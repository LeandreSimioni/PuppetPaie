//! `paie releve-conventions mettre-a-jour|article|chercher` : le texte complet des conventions collectives suivies, tenu a
//! jour en local. Porte de `atelier/outils/relever_les_conventions.py` (retire le 15/10/2026) ; meme entrepot
//! (`kali-entrepot/`), memes champs. Un outil de l'atelier : `<sources>` est `atelier/reference/sources` du depot.
//!
//! POURQUOI (24/09/2026, journal §122). La veille des conventions (`kali.rs`, `paie sources --verifier`) est
//! incrementale, mais elle ne garde que la LISTE des textes de chaque convention -- pas leurs articles. Le 24/09/2026,
//! relever les clauses de rupture de la 3090 puis de la 1285 a demande de retelecharger deux fois la base KALI entiere
//! (180 Mo) et ses 246 livraisons, et de filtrer a la main. L'entrepot garde, dans `kali-entrepot/`, TOUS les articles des
//! conventions citees par `donnees/sources.json` -- clauses communes, annexes, avenants, toutes versions --, et la liste
//! des livraisons deja appliquees : un article se lit en une seconde, et une mise a jour ne telecharge que les livraisons
//! nouvelles.
//!
//! Un article se rend avec son identifiant KALI, son etat (VIGUEUR_ETEN, VIGUEUR_NON_ETEN, ABROGE...), ses dates et le
//! texte auquel il appartient : c'est ce qu'une transcription dans `atelier/reference/sources/` recopie.
//!
//! La base entiere ne se relit que pour construire l'entrepot, ou avec `--reconstruire` (une convention nouvelle a suivre,
//! par exemple -- elle se relit d'elle-meme quand le manifeste en cite une que l'entrepot ne tient pas).

use std::collections::HashSet;
use std::io::Read;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Instant;

use regex::Regex;
use serde_json::{json, Map, Value};

use crate::date::Jour;
use crate::entrepot_dila::{self as dila, dire, Entrepot, LecteurTar, Source};

type R<T> = Result<T, String>;

pub const BASE: &str = crate::kali::INDEX;
pub const DOSSIER: &str = "kali-entrepot";

const USAGE: &str = "usage : paie releve-conventions mettre-a-jour <sources> [--reconstruire]
       paie releve-conventions article <sources> <IDCC> <numero> [...]
       paie releve-conventions chercher <sources> <IDCC> <mot>

Le texte complet des conventions collectives suivies (celles que cite logiciel/donnees/sources.json), tenu a jour dans
<sources>/kali-entrepot/ depuis la base KALI de la DILA (open data). `mettre-a-jour` n'applique que les livraisons
nouvelles (`--reconstruire` relit la base entiere, 180 Mo) ; `article` rend les articles en vigueur d'un numero ;
`chercher` liste les articles en vigueur dont le texte porte le mot.";

/// `{conteneur KALICONT: IDCC}` -- les conventions que `sources.json` cite, et celles qu'il met de cote a relever. Dans
/// l'ordre du manifeste : c'est lui qui decide a quel conteneur un article se rattache quand il en cite plusieurs.
pub fn suivies(manifeste: &Value) -> Map<String, Value> {
    let mut out = Map::new();
    let sources: Vec<&Value> = match manifeste.get("sources") {
        Some(Value::Object(o)) => o.values().collect(),
        Some(Value::Array(a)) => a.iter().collect(),
        _ => Vec::new(),
    };
    for s in sources {
        let Some(k) = s.get("kali").filter(|k| dila::vrai(k)) else {
            continue;
        };
        if let Some(c) = k.get("conteneur").filter(|c| dila::vrai(c)) {
            let idcc = k.get("idcc").filter(|i| dila::vrai(i)).map(dila::texte).unwrap_or_default();
            out.insert(dila::texte(c), json!(idcc));
        }
    }
    // Les conventions que le moteur ne porte pas encore, gardees pour etre relues avant d'ecrire leur couche (25/09/2026,
    // journal §130).
    for (idcc, c) in manifeste.get("conventions_a_relever").and_then(Value::as_object).into_iter().flatten() {
        if idcc.starts_with('_') {
            continue;
        }
        if let Some(cont) = c.as_object().and_then(|o| o.get("conteneur")).filter(|c| dila::vrai(c)) {
            out.insert(dila::texte(cont), json!(idcc));
        }
    }
    out
}

fn re_champ(k: &str) -> &'static Regex {
    static M: OnceLock<Vec<(&'static str, Regex)>> = OnceLock::new();
    let t = M.get_or_init(|| {
        ["ID", "NUM", "ETAT", "DATE_DEBUT", "DATE_FIN", "CONTENU"]
            .into_iter()
            .map(|k| (k, Regex::new(&format!(r"(?s)<{}>(.*?)</{}>", k, k)).expect("motif")))
            .collect()
    });
    &t.iter().find(|(n, _)| *n == k).expect("champ connu").1
}

fn motif(m: &'static OnceLock<Regex>, s: &str) -> &'static Regex {
    m.get_or_init(|| Regex::new(s).expect("motif"))
}

/// Un fragment de XML rendu lisible : `<br/>` et `</p>` en sauts de ligne, les autres balises retirees, les entites
/// decodees, les lignes vides ramenees a une.
pub fn nettoyer(x: &str) -> String {
    static SAUTS: OnceLock<Regex> = OnceLock::new();
    static BALISES: OnceLock<Regex> = OnceLock::new();
    static VIDES: OnceLock<Regex> = OnceLock::new();
    let x = motif(&SAUTS, r"<br[\s\x1C-\x1F]*/?>|</p>").replace_all(x, "\n");
    let x = dila::unescape_html(&motif(&BALISES, r"<[^>]+>").replace_all(&x, ""));
    let x = motif(&VIDES, r"\n[\s\x1C-\x1F]*\n+").replace_all(&x, "\n\n");
    dila::strip_py(&x).to_string()
}

/// (cle, article) d'un fichier d'article, ou `None` s'il n'est d'aucune convention suivie. La cle est (IDCC, identifiant
/// KALIARTI).
fn lire(xml: &str, conteneurs: &Map<String, Value>) -> Option<(String, Value)> {
    static TXT: OnceLock<Regex> = OnceLock::new();
    static TM: OnceLock<Regex> = OnceLock::new();
    let cont = conteneurs.keys().find(|c| xml.contains(c.as_str()))?;
    let g = |k: &str| re_champ(k).captures(xml).and_then(|c| c.get(1)).map(|m| dila::strip_py(m.as_str()).to_string()).unwrap_or_default();
    let txt = motif(&TXT, r#"(?s)<TITRE_TXT[^>]*id_txt="([^"]*)"[^>]*>(.*?)</TITRE_TXT>"#).captures(xml);
    let ident = g("ID");
    if ident.is_empty() {
        return None;
    }
    let idcc = dila::texte(&conteneurs[cont.as_str()]);
    let chemin: Vec<String> =
        motif(&TM, r"(?s)<TITRE_TM[^>]*>(.*?)</TITRE_TM>").captures_iter(xml).map(|c| nettoyer(&c[1])).collect();
    let a = json!({
        "id": ident, "num": g("NUM"), "etat": g("ETAT"),
        "debut": g("DATE_DEBUT"), "fin": g("DATE_FIN"),
        "conteneur": cont,
        "texte": txt.as_ref().map(|c| c[1].to_string()).unwrap_or_default(),
        "titre_texte": txt.as_ref().map(|c| nettoyer(&c[2])).unwrap_or_default(),
        "chemin": chemin.join(" > "),
        "contenu": nettoyer(&g("CONTENU")),
    });
    Some((dila::cle(&[&idcc, &ident]), a))
}

/// Fait defiler une archive (la base entiere, ou une livraison quand `livraison` est nommee) et range les articles des
/// conventions suivies ; rend le nombre d'articles gardes. Une livraison qui change l'etat ou le texte d'un article deja
/// tenu le note dans `etat["changes"]`.
fn passer<L: Read>(tar: &mut LecteurTar<L>, ent: &mut Entrepot, conteneurs: &Map<String, Value>, livraison: &str) -> R<usize> {
    let mut n = 0;
    while let Some(m) = tar.suivant()? {
        if !m.fichier {
            continue;
        }
        if m.nom.ends_with(".dat") && m.nom.contains("suppression") {
            let liste = String::from_utf8_lossy(&tar.lire()?).into_owned();
            let retires: HashSet<&str> = dila::lignes_py(&liste)
                .into_iter()
                .map(dila::strip_py)
                .filter(|l| !l.is_empty())
                .map(|l| l.rsplit('/').next().unwrap_or(l))
                .collect();
            ent.articles.retain(|k, _| !dila::parties(k).get(1).is_some_and(|id| retires.contains(id)));
            continue;
        }
        if !m.nom.contains("/article/") {
            continue;
        }
        let xml = String::from_utf8_lossy(&tar.lire()?).into_owned();
        let Some((k, a)) = lire(&xml, conteneurs) else {
            continue;
        };
        if let Some(ancien) = ent.articles.get(&k).filter(|v| dila::vrai(v)) {
            if !livraison.is_empty() && (&ancien["etat"], &ancien["contenu"]) != (&a["etat"], &a["contenu"]) {
                let p = dila::parties(&k);
                let change = json!({
                    "idcc": p[0], "id": p[1], "num": a["num"], "livraison": livraison,
                    "avant": ancien["etat"], "apres": a["etat"],
                    "texte_change": ancien["contenu"] != a["contenu"],
                });
                let v = ent.etat.entry("changes").or_insert_with(|| json!([]));
                if !v.is_array() {
                    *v = json!([]);
                }
                if let Some(l) = v.as_array_mut() {
                    l.push(change);
                }
            }
        }
        ent.articles.insert(k, a);
        n += 1;
    }
    Ok(n)
}

/// L'archive globale la plus recente de l'index.
fn globale(source: &dyn Source) -> R<String> {
    let page = dila::page(source, BASE)?;
    let m = Regex::new(r"(Freemium_kali_global_\d{8}-\d{6}\.tar\.gz)").expect("motif");
    let mut noms: Vec<String> = m.captures_iter(&page).map(|c| c[1].to_string()).collect();
    noms.sort();
    noms.dedup();
    noms.pop().ok_or_else(|| format!("aucune archive globale KALI a {}", BASE))
}

/// Tient l'entrepot a jour : la base entiere la premiere fois (ou une convention nouvelle a suivre, ou `reconstruire`),
/// puis les livraisons pas encore appliquees, dans l'ordre.
pub fn mettre_a_jour(racine: &Path, reconstruire: bool, manifeste: &Value, source: &dyn Source, aujourdhui: &Jour) -> R<Entrepot> {
    let jour = aujourdhui.iso();
    let conteneurs = suivies(manifeste);
    let mut ent = Entrepot::nouveau(&racine.join(DOSSIER));
    let tenus = |e: &Entrepot| -> HashSet<String> { e.etat.get("conteneurs").and_then(Value::as_object).map(|o| o.keys().cloned().collect()).unwrap_or_default() };
    let mut manquantes: Vec<&String> = conteneurs.keys().filter(|c| !tenus(&ent).contains(*c)).collect();
    let existait = ent.existe();
    if existait && !reconstruire {
        ent = ent.charger()?;
        let t = tenus(&ent);
        manquantes = conteneurs.keys().filter(|c| !t.contains(*c)).collect();
    }
    if !existait || reconstruire || !manquantes.is_empty() {
        // UNE CONVENTION NOUVELLE A SUIVRE, ou la premiere fois : la base entiere defile une fois (180 Mo), puis les
        // livraisons depuis sa date.
        let glob = globale(source)?;
        dire(&format!("== base KALI entiere : {} ==", glob));
        ent.articles = Map::new();
        let t0 = Instant::now();
        let n = passer(&mut dila::tar_gz(source.flux(&format!("{}{}", BASE, glob))?), &mut ent, &conteneurs, "")?;
        let mut etat = Map::new();
        etat.insert("global".into(), json!(glob));
        etat.insert("lues".into(), json!([]));
        etat.insert("changes".into(), json!([]));
        etat.insert("conteneurs".into(), Value::Object(conteneurs.clone()));
        etat.insert("construit_le".into(), json!(jour));
        ent.etat = etat;
        dire(&format!("  {} articles gardes en {:.0}s", n, t0.elapsed().as_secs_f64()));
        ent.sauver(&jour)?;
    }
    let global = dila::texte(ent.etat.get("global").unwrap_or(&Value::Null));
    let date_globale = Regex::new(r"(\d{8})")
        .expect("motif")
        .captures(&global)
        .map(|c| c[1].to_string())
        .ok_or_else(|| format!("{}/etat.json : « global » sans date ({})", DOSSIER, global))?;
    let toutes: Vec<String> = dila::livraisons(&dila::page(source, BASE)?, "KALI").into_iter().filter(|l| l.get(..8).unwrap_or("") >= date_globale.as_str()).collect();
    let lues = ent.lues();
    let neuves: Vec<String> = toutes.iter().filter(|l| !lues.contains(*l)).cloned().collect();
    dire(&format!("== {} livraison(s) nouvelle(s) sur {} ==", neuves.len(), toutes.len()));
    let urls: Vec<String> = neuves.iter().map(|l| format!("{}KALI_{}.tar.gz", BASE, l)).collect();
    let mut panne: Option<String> = None;
    dila::en_ordre(source, &urls, 8, |i, octets| {
        let nom = &neuves[i];
        let octets = match octets {
            Ok(o) => o,
            Err(e) => {
                // Une livraison manquee arrete la suite : la suivante ne s'applique pas par-dessus un trou.
                dire(&format!("  ! livraison {} : {} -- arret, reprise au prochain passage", nom, e));
                return false;
            }
        };
        if let Err(e) = passer(&mut dila::archive(&octets), &mut ent, &conteneurs, nom) {
            panne = Some(format!("livraison {} : {}", nom, e));
            return false;
        }
        ent.noter_lue(nom);
        if (i + 1) % 50 == 0 {
            if let Err(e) = ent.sauver(&jour) {
                panne = Some(e);
                return false;
            }
            dire(&format!("  [{}/{}] {}", i + 1, neuves.len(), nom));
        }
        true
    });
    if let Some(e) = panne {
        return Err(e);
    }
    ent.sauver(&jour)?;
    dire(&format!("== {} articles, {} livraisons appliquees ==", ent.articles.len(), ent.lues().len()));
    Ok(ent)
}

/// L'entrepot, qui doit exister.
pub fn charger(racine: &Path) -> R<Entrepot> {
    let ent = Entrepot::nouveau(&racine.join(DOSSIER));
    if !ent.existe() {
        return Err(format!(
            "Pas d'entrepot KALI dans {} : lancer d'abord `paie releve-conventions mettre-a-jour`.",
            racine.display()
        ));
    }
    ent.charger()
}

/// Les articles d'une convention, d'un numero donne s'il y en a un ; les versions en vigueur seulement, par defaut. Ranges
/// par texte, numero, date de debut.
pub fn articles<'a>(ent: &'a Entrepot, idcc: &str, num: Option<&str>, en_vigueur: bool) -> Vec<&'a Value> {
    let s = |a: &Value, k: &str| a.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let mut out: Vec<&Value> = ent
        .articles
        .iter()
        .filter(|(k, a)| {
            dila::parties(k)[0] == idcc
                && num.is_none_or(|n| a.get("num").and_then(Value::as_str) == Some(n))
                && (!en_vigueur || s(a, "etat").starts_with("VIGUEUR"))
        })
        .map(|(_, a)| a)
        .collect();
    out.sort_by_key(|a| (s(a, "texte"), s(a, "num"), s(a, "debut")));
    out
}

/// Un article comme les transcriptions de `atelier/reference/sources/` le recopient.
pub fn transcrire(a: &Value) -> String {
    let s = |k: &str| a.get(k).map(dila::texte).unwrap_or_default();
    let num = s("num");
    format!(
        "{}\n{} -- article {}\n{} -- {} du {} au {}\ntexte : {} ({})\n{}\n{}\n",
        "=".repeat(78),
        dila::debut_py(&s("chemin"), 120),
        if num.is_empty() { "(chapeau)".to_string() } else { num },
        s("id"),
        s("etat"),
        s("debut"),
        s("fin"),
        s("titre_texte"),
        s("texte"),
        "-".repeat(78),
        s("contenu")
    )
}

/// `article <sources> <IDCC> <numero>...` : ce que la commande affiche.
pub fn rendre_articles(ent: &Entrepot, idcc: &str, nums: &[String]) -> String {
    let mut out = String::new();
    for num in nums {
        let trouves = articles(ent, idcc, Some(num), true);
        if trouves.is_empty() {
            out.push_str(&format!("(aucun article {} en vigueur pour l'IDCC {})\n", num, idcc));
        }
        for a in trouves {
            out.push_str(&transcrire(a));
            out.push('\n');
        }
    }
    out
}

/// `chercher <sources> <IDCC> <mot>` : une ligne par article en vigueur dont le texte porte le mot (sans egard a la casse).
pub fn rendre_recherche(ent: &Entrepot, idcc: &str, mot: &str) -> String {
    let mot = mot.to_lowercase();
    let mut out = String::new();
    for a in articles(ent, idcc, None, true) {
        let s = |k: &str| a.get(k).map(dila::texte).unwrap_or_default();
        if s("contenu").to_lowercase().contains(&mot) {
            out.push_str(&format!("{}  {:<8} {:<22} {}\n", s("id"), s("num"), s("etat"), dila::debut_py(&s("chemin"), 90)));
        }
    }
    out
}

/// `paie releve-conventions ...` : rend le code de sortie.
pub fn commande(args: &[String]) -> i32 {
    if args.len() < 2 || args.iter().any(|a| a == "-h" || a == "--help" || a == "--aide") {
        eprintln!("{}", USAGE);
        return 2;
    }
    let racine = Path::new(&args[1]);
    let sortie = |r: R<String>| match r {
        Ok(t) => {
            use std::io::Write;
            let mut o = std::io::stdout().lock();
            let _ = o.write_all(t.as_bytes());
            let _ = o.flush();
            0
        }
        Err(m) => {
            eprintln!("{}", m);
            1
        }
    };
    match args[0].as_str() {
        "mettre-a-jour" => {
            crate::commandes::compagnie::fixer_le_code();
            let r = crate::sources::manifeste(&crate::fichiers::dossier_du_code()).and_then(|m| {
                let jour = crate::contexte::aujourdhui_sans_note()?;
                mettre_a_jour(racine, args.iter().any(|a| a == "--reconstruire"), &m, &dila::Reseau, &jour).map(|_| String::new())
            });
            sortie(r)
        }
        "article" if args.len() >= 4 => sortie(charger(racine).map(|e| rendre_articles(&e, &args[2], &args[3..]))),
        "chercher" if args.len() >= 4 => sortie(charger(racine).map(|e| rendre_recherche(&e, &args[2], &args[3]))),
        _ => {
            eprintln!("{}", USAGE);
            2
        }
    }
}
