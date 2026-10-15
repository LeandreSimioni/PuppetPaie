//! `paie releve-codes moissonner|rapport|comparer <sources> [code] [--reconstruire]` : les articles de loi dont la paie a
//! besoin, releves chez la DILA. Porte de `atelier/outils/relever_les_codes.py` (retire le 15/10/2026) ; memes fichiers,
//! memes champs, meme ordre. Un outil de l'atelier : `<sources>` est `atelier/reference/sources` du depot.
//!
//! POURQUOI. Legifrance refuse les robots (403) : les articles du §38.3 avaient ete releves A LA MAIN, un par un, ce qui
//! marche une fois et se perime en silence. La base LEGI est publique en open data (`echanges.dila.gouv.fr/OPENDATA/LEGI/`),
//! exactement comme KALI pour les conventions collectives : une archive globale et une livraison par soir. Le releve
//! devient donc RE-VERIFIABLE automatiquement, comme la table des parametres du BOSS et celle des CTP de l'Urssaf.
//!
//! PLUSIEURS TEXTES, UN SEUL PASSAGE (19/09/2026). Le code du travail dit ce qu'une indemnite de rupture VAUT ; le code de
//! la securite sociale dit ce qu'elle SUPPORTE, et le code general des impots ce qu'elle coute a celui qui la recoit. Tous
//! se lisent dans la meme archive : ce qui coute, c'est de faire defiler 1,1 Go, pas de reconnaitre un article de plus au
//! passage. Le dossier donne en argument est celui des SOURCES ; chaque texte y ecrit dans le sien (`textes::TEXTES`).
//!
//! `moissonner` TIENT UN ENTREPOT A JOUR (journal §122) : `legi-entrepot/` dans le dossier des sources garde TOUS les
//! articles en vigueur des textes suivis et la liste des livraisons deja appliquees (`entrepot_dila`). La premiere fois
//! (ou avec `--reconstruire`), l'archive globale defile -- 1,1 Go, un quart d'heure --, puis les livraisons depuis sa date ;
//! ensuite, seules les livraisons nouvelles se telechargent (quelques secondes pour une semaine). Ajouter un article a
//! `TEXTES` ne demande plus aucun telechargement : il est deja dans l'entrepot. `rapport` ecrit le document lisible a
//! partir de ce qui a ete moissonne.
//!
//! `comparer` REPOND A LA QUESTION QUE `moissonner` POSE (21/09/2026). « Huit articles ont change de version » ne dit pas
//! si le DROIT a change : un article dont un alinea etranger a la paie est reecrit recoit un identifiant neuf sans que rien
//! de ce qui nous concerne bouge. Il faut donc relire chaque article contre sa version d'AVANT -- et cette version-la ne se
//! trouve nulle part ailleurs que dans l'archive globale. Le releve ne garde que la version en vigueur ; l'historique Git
//! ne l'a pas non plus, parce qu'un releve precedent portait deja, lui aussi, la version d'apres. Comptez un quart
//! d'heure : `comparer` s'arrete des qu'il les a toutes.
//!
//! LE PIEGE PROPRE A `comparer` : on cherche par IDENTIFIANT, jamais par numero. L'archive globale porte plusieurs
//! versions du meme article ; demander « L242-1 » y rapporte un paquet de redactions dont on ne saurait laquelle etait en
//! vigueur. Ce qu'on veut n'est pas « une vieille version », c'est CELLE que le releve precedent tenait pour en vigueur --
//! celle que `_changes_depuis_le_global` nomme sous `avant` (`avant`, plus bas).
//!
//! TROIS PIEGES, payes tous les trois (chacun est rappele la ou le code l'evite, dans `passer`) :
//!
//!   1. le dossier du dump s'appelle `code_en_vigueur`, mais il porte AUSSI les versions anterieures de chaque article.
//!      La version qui vaut est celle dont `<ETAT>` est VIGUEUR et `<DATE_FIN>` 2999-01-01 -- et `etat` existe aussi
//!      comme ATTRIBUT, sur d'autres balises, ce qui fait attraper n'importe quoi. Premiere version du moissonneur : sept
//!      articles sur cinquante-cinq gardes dans une version perimee, dont L3141-3 (les 2,5 jours de conges) et R1234-2
//!      (l'indemnite de licenciement) ;
//!   2. un article modifie recoit un NOUVEL identifiant de version. On range donc l'entrepot par NUMERO d'article, pas
//!      par identifiant ;
//!   3. un numero d'article n'appartient a aucun code par lui-meme. `L242-1` est un article du code de la securite
//!      sociale, et rien n'interdit a un autre code d'en porter un du meme nom. Ce qui tranche, c'est le LEGITEXT dans le
//!      chemin du fichier -- pas le numero.
//!
//! CE QUI DIFFERE DU PYTHON : les documents ecrits nomment la commande (`paie releve-codes`) la ou ils nommaient le
//! script ; l'archive compressee de l'entrepot (`entrepot_dila`) ; et la date du jour, prise dans
//! `PUPPETPAIE_AUJOURDHUI` quand elle est posee (`contexte::aujourdhui_sans_note`, en temps universel sinon).

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Instant;

use regex::Regex;
use serde_json::{json, Map, Value};

use crate::date::Jour;
use crate::entrepot_dila::{self as dila, dire, Entrepot, LecteurTar, Source};

mod difflib;
pub mod textes;

pub use textes::{Domaine, Texte, TEXTES};

type R<T> = Result<T, String>;

pub const BASE: &str = "https://echanges.dila.gouv.fr/OPENDATA/LEGI/";
pub const GLOBAL: &str = "Freemium_legi_global_20250713-140000.tar.gz";
pub const DEPUIS: &str = "20250713";
/// Le sous-dossier des pieces justificatives.
pub const XML: &str = "xml";
/// Les versions d'avant, a cote des pieces (`comparer`).
pub const ANTERIEUR: &str = "xml-anterieur";
/// L'entrepot des textes suivis, a cote des releves (journal §122).
pub const ENTREPOT: &str = "legi-entrepot";

const USAGE: &str = "usage : paie releve-codes moissonner <sources> [--reconstruire]
       paie releve-codes rapport <sources> [code]
       paie releve-codes comparer <sources>

Les articles de loi dont la paie a besoin, releves dans la base LEGI de la DILA (open data). <sources> est le dossier des
sources du depot (atelier/reference/sources) ; chaque texte y ecrit dans le sien. `moissonner` tient `legi-entrepot/` a
jour (seules les livraisons nouvelles se telechargent ; `--reconstruire` refait defiler l'archive globale, 1,1 Go) puis
ecrit `releve.json` et `xml/` ; `rapport` ecrit `articles-<date>.md` ; `comparer` relit chaque article change contre sa
version d'avant, dans l'archive globale.";

fn texte_de(cle: &str) -> Option<&'static Texte> {
    TEXTES.iter().find(|t| t.cle == cle)
}

/// LEGITEXT -> cle du texte.
fn par_legitext() -> Vec<(&'static str, &'static str)> {
    TEXTES.iter().map(|t| (t.legitext, t.cle)).collect()
}

/// Cle du texte -> les numeros qu'on y cherche.
fn tous() -> &'static HashMap<&'static str, HashSet<&'static str>> {
    static T: OnceLock<HashMap<&'static str, HashSet<&'static str>>> = OnceLock::new();
    T.get_or_init(|| TEXTES.iter().map(|t| (t.cle, t.domaines.iter().flat_map(|d| d.numeros.iter().copied()).collect())).collect())
}

fn voulu(texte: &str, num: &str) -> bool {
    tous().get(texte).is_some_and(|n| n.contains(num))
}

fn motif(m: &'static OnceLock<Regex>, s: &str) -> &'static Regex {
    m.get_or_init(|| Regex::new(s).expect("motif"))
}

fn re_num() -> &'static Regex {
    static M: OnceLock<Regex> = OnceLock::new();
    motif(&M, r"<NUM>([^<]*)</NUM>")
}
fn re_etat() -> &'static Regex {
    static M: OnceLock<Regex> = OnceLock::new();
    motif(&M, r"<ETAT>([^<]*)</ETAT>")
}
fn re_debut() -> &'static Regex {
    static M: OnceLock<Regex> = OnceLock::new();
    motif(&M, r"<DATE_DEBUT>([^<]*)</DATE_DEBUT>")
}
fn re_fin() -> &'static Regex {
    static M: OnceLock<Regex> = OnceLock::new();
    motif(&M, r"<DATE_FIN>([^<]*)</DATE_FIN>")
}
fn re_id() -> &'static Regex {
    static M: OnceLock<Regex> = OnceLock::new();
    motif(&M, r"(LEGIARTI\d+)\.xml$")
}
fn re_texte() -> &'static Regex {
    static M: OnceLock<Regex> = OnceLock::new();
    motif(&M, r"(?s)<BLOC_TEXTUEL>(.*?)</BLOC_TEXTUEL>")
}

fn groupe<'a>(m: &Regex, t: &'a str) -> Option<&'a str> {
    m.captures(t).and_then(|c| c.get(1)).map(|g| g.as_str())
}

// ------------------------------------------------------------------ la ligne de commande

/// `paie releve-codes ...` : rend le code de sortie.
pub fn commande(args: &[String]) -> i32 {
    if args.len() < 2 || args.iter().any(|a| a == "-h" || a == "--help" || a == "--aide") {
        eprintln!("{}", USAGE);
        return 2;
    }
    let racine = Path::new(&args[1]);
    let aujourdhui = match crate::contexte::aujourdhui_sans_note() {
        Ok(j) => j,
        Err(m) => {
            eprintln!("{}", m);
            return 1;
        }
    };
    let r = match args[0].as_str() {
        "moissonner" => moissonner(racine, args.iter().any(|a| a == "--reconstruire"), &dila::Reseau, &aujourdhui),
        "rapport" => rapport(racine, args.get(2).map(String::as_str).unwrap_or(""), &aujourdhui),
        "comparer" => comparer(racine, &dila::Reseau, &aujourdhui),
        v => {
            eprintln!("Verbe inconnu : {}\n\n{}", v, USAGE);
            return 2;
        }
    };
    match r {
        Ok(()) => 0,
        Err(m) => {
            eprintln!("{}", m);
            1
        }
    }
}

// ------------------------------------------------------------------ moissonner

/// Fait defiler une archive et garde les articles en vigueur ; rend (fichiers d'article des textes suivis, secondes).
///
/// `tar` : une archive ouverte (en flux pour la globale, en memoire pour une livraison). `tous` : garder TOUS les articles
/// des textes suivis -- c'est ce que fait l'entrepot (journal §122) --, et non les seuls voulus.
///
/// `changes` : chaque fois qu'une livraison remplace la version d'un article, on note laquelle et quand. C'est la seule
/// chose qui compte au deuxieme passage -- « aucun des articles n'a bouge » est une phrase qui doit se prouver, pas se
/// supposer.
///
/// `seuls` : ne regarder que ces textes-la (des cles de `TEXTES`) -- c'est ce que fait `completer` pour un texte ajoute
/// apres la construction de l'entrepot.
#[allow(clippy::too_many_arguments)]
fn passer<L: Read>(
    tar: &mut LecteurTar<L>,
    articles: &mut Map<String, Value>,
    journal: &mut Vec<String>,
    mut changes: Option<&mut Vec<Value>>,
    livraison: &str,
    tous_les_articles: bool,
    seuls: Option<&[&str]>,
) -> R<(usize, f64)> {
    let suivis: Vec<(&str, &str)> = par_legitext().into_iter().filter(|(_, cle)| seuls.is_none_or(|s| s.contains(cle))).collect();
    let (mut vus, t0) = (0usize, Instant::now());
    while let Some(m) = tar.suivant()? {
        if !m.fichier {
            continue;
        }
        if m.nom.ends_with(".dat") && m.nom.contains("suppression") {
            // CE QUE LA LIVRAISON RETIRE : un article dont la version gardee est dans la liste sort de l'entrepot.
            let liste = String::from_utf8_lossy(&tar.lire()?).into_owned();
            let retires: HashSet<&str> = dila::lignes_py(&liste)
                .into_iter()
                .map(dila::strip_py)
                .filter(|l| !l.is_empty())
                .map(|l| l.rsplit('/').next().unwrap_or(l))
                .collect();
            let partants: Vec<String> = articles
                .iter()
                .filter(|(_, a)| a.get("id").and_then(Value::as_str).is_some_and(|i| retires.contains(i)))
                .map(|(k, _)| k.clone())
                .collect();
            for k in partants {
                let p = dila::parties(&k);
                journal.push(format!("retire par {} : {} {}", livraison, p[0], p.get(1).unwrap_or(&"")));
                articles.shift_remove(&k);
            }
            continue;
        }
        if !m.nom.contains("/article/") {
            continue;
        }
        // LE NUMERO NE SUFFIT PAS A DIRE DE QUEL CODE IL EST (piege 3) : "L242-1" existe au code de la securite sociale, et
        // rien n'interdit qu'il existe ailleurs. C'est le chemin du fichier, qui porte le LEGITEXT, qui tranche -- et il
        // tranche AVANT qu'on lise le contenu.
        let Some(texte) = suivis.iter().find(|(lt, _)| m.nom.contains(lt)).map(|(_, cle)| *cle) else {
            continue;
        };
        vus += 1;
        let txt = String::from_utf8_lossy(&tar.lire()?).into_owned();
        let Some(num) = groupe(re_num(), &txt).map(|n| dila::strip_py(n).to_string()) else {
            continue;
        };
        if !(tous_les_articles || voulu(texte, &num)) {
            continue;
        }
        // PIEGE 1 : `code_en_vigueur` porte aussi les versions anterieures. La version qui vaut a `<ETAT>` VIGUEUR et
        // `<DATE_FIN>` 2999-01-01 -- la BALISE, pas l'attribut `etat` d'autres balises.
        let en_vigueur = groupe(re_etat(), &txt) == Some("VIGUEUR") && groupe(re_fin(), &txt) == Some("2999-01-01");
        let debut = groupe(re_debut(), &txt).unwrap_or("").to_string();
        // PIEGE 2 : l'entrepot range par (texte, NUMERO) ; une version neuve remplace l'ancienne sous la meme cle.
        let k = dila::cle(&[texte, &num]);
        let ancien = articles.get(&k).and_then(Value::as_object);
        if ancien.is_some_and(|a| a.get("en_vigueur") == Some(&json!(true))) && !en_vigueur {
            continue;
        }
        let id = groupe(re_id(), &m.nom).unwrap_or("").to_string();
        if let (Some(ch), Some(a)) = (changes.as_deref_mut(), ancien) {
            if a.get("id").and_then(Value::as_str).unwrap_or("") != id {
                // L'ordre des cles est celui du Python : il se voit dans `releve.json`, qui ne trie pas.
                ch.push(json!({
                    "texte": texte, "article": num, "avant": a.get("id").cloned().unwrap_or(json!("")),
                    "apres": id, "depuis": debut, "livraison": livraison,
                }));
            }
        }
        articles.insert(k, json!({"id": id, "debut": debut, "en_vigueur": en_vigueur, "xml": txt}));
    }
    Ok((vus, t0.elapsed().as_secs_f64()))
}

/// Les livraisons de l'index posterieures a l'archive globale.
fn livraisons(source: &dyn Source) -> R<Vec<String>> {
    let page = dila::page(source, BASE)?;
    Ok(dila::livraisons(&page, "LEGI").into_iter().filter(|n| n.get(..8).unwrap_or("") > DEPUIS).collect())
}

/// L'archive globale en flux, rejouee trois fois : elle fait 1,1 Go, et une coupure de transport n'est pas une reponse.
fn passer_le_global(
    ent: &mut Entrepot,
    journal: &mut Vec<String>,
    source: &dyn Source,
    seuls: Option<&[&str]>,
) -> R<(usize, f64)> {
    let mut essai = 1;
    loop {
        let r = source.flux(&format!("{}{}", BASE, GLOBAL)).and_then(|f| passer(&mut dila::tar_gz(f), &mut ent.articles, journal, None, "", true, seuls));
        match r {
            Ok(v) => return Ok(v),
            Err(e) => {
                dire(&format!("  archive globale, essai {} : {}", essai, e));
                if essai == 3 {
                    return Err(e);
                }
                // Ce qui a ete garde de ce passage-la ne vaut rien : on le retire avant de rejouer.
                match seuls {
                    None => ent.articles.clear(),
                    Some(s) => ent.articles.retain(|k, _| !s.contains(&dila::parties(k)[0])),
                }
                source.pause(10 * essai);
                essai += 1;
            }
        }
    }
}

/// Tenir l'entrepot a jour, puis en tirer les articles voulus. `racine` est le dossier des sources ; chaque texte y ecrit
/// dans le sien.
///
/// LA PREMIERE FOIS SEULEMENT (ou avec `reconstruire`), l'archive globale defile -- 1,1 Go, un quart d'heure -- et
/// l'entrepot garde TOUS les articles en vigueur des textes suivis, pas seulement les voulus. Ensuite, seules les
/// livraisons pas encore appliquees se telechargent, plusieurs a la fois, et s'appliquent dans l'ordre. Ajouter un article
/// a `TEXTES` ne coute plus rien : il est deja dans l'entrepot (journal §122).
pub fn moissonner(racine: &Path, reconstruire: bool, source: &dyn Source, aujourdhui: &Jour) -> R<()> {
    let jour = aujourdhui.iso();
    std::fs::create_dir_all(racine).map_err(|e| format!("{} : {}", racine.display(), e))?;
    let mut ent = Entrepot::nouveau(&racine.join(ENTREPOT));
    let mut journal: Vec<String> = Vec::new();
    if ent.existe() && !reconstruire {
        ent = ent.charger()?;
        dire(&format!("== entrepot : {} articles, {} livraisons deja appliquees ==", ent.articles.len(), ent.lues().len()));
        // UN TEXTE AJOUTE A `TEXTES` APRES LA CONSTRUCTION (11/10/2026, lot R9 : l'arrete du 4 septembre 2025 sur les frais
        // professionnels) n'a aucun article dans l'entrepot : l'archive globale ne l'a pas garde. On la refait defiler pour
        // lui seul, puis les livraisons deja lues.
        let presents: HashSet<&str> = ent.articles.keys().map(|k| dila::parties(k)[0]).collect();
        let absents: Vec<&str> = TEXTES.iter().map(|t| t.cle).filter(|c| !presents.contains(c)).collect();
        if !absents.is_empty() {
            completer(&mut ent, &absents, &mut journal, source, &jour)?;
        }
    } else {
        dire("== archive globale (construction de l'entrepot) ==");
        // L'ARCHIVE GLOBALE EST LE SOCLE : sans elle il n'y a rien a comparer.
        let (vus, sec) = passer_le_global(&mut ent, &mut journal, source, None)?;
        let mut etat = Map::new();
        etat.insert("global".into(), json!(GLOBAL));
        etat.insert("lues".into(), json!([]));
        etat.insert("changes".into(), json!([]));
        etat.insert("construit_le".into(), json!(jour));
        ent.etat = etat;
        dire(&format!("  {} fichiers d'article lus en {:.0}s ; {} articles gardes", vus, sec, ent.articles.len()));
        ent.sauver(&jour)?;
    }
    let toutes = livraisons(source)?;
    let lues = ent.lues();
    let neuves: Vec<String> = toutes.iter().filter(|n| !lues.contains(*n)).cloned().collect();
    dire(&format!("== {} livraison(s) nouvelle(s) sur {} ==", neuves.len(), toutes.len()));
    let urls: Vec<String> = neuves.iter().map(|n| format!("{}LEGI_{}.tar.gz", BASE, n)).collect();
    let mut changes: Vec<Value> = Vec::new();
    let mut panne: Option<String> = None;
    dila::en_ordre(source, &urls, 8, |i, octets| {
        let nom = &neuves[i];
        let octets = match octets {
            Ok(o) => o,
            Err(e) => {
                // UNE LIVRAISON MANQUEE ARRETE LA SUITE : appliquer celle du lendemain par-dessus un trou ferait un
                // entrepot faux sans le dire. La prochaine moisson reprendra ici.
                journal.push(format!("livraison {} : {} -- arret, reprise au prochain passage", nom, e));
                return false;
            }
        };
        if let Err(e) = passer(&mut dila::archive(&octets), &mut ent.articles, &mut journal, Some(&mut changes), nom, true, None) {
            panne = Some(format!("livraison {} : {}", nom, e));
            return false;
        }
        ent.noter_lue(nom);
        // ON ECRIT EN CHEMIN, toutes les vingt-cinq. Un releve qui ne dit pas ou il s'est arrete est un releve qu'on croit
        // complet.
        if (i + 1) % 25 == 0 {
            ajouter_changes(&mut ent, std::mem::take(&mut changes));
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
    ajouter_changes(&mut ent, changes);
    ent.sauver(&jour)?;
    let articles: Map<String, Value> = ent
        .articles
        .iter()
        .filter(|(k, _)| {
            let p = dila::parties(k);
            voulu(p[0], p.get(1).unwrap_or(&""))
        })
        .map(|(k, a)| (k.clone(), a.clone()))
        .collect();
    let changes_voulus: Vec<Value> = ent
        .etat
        .get("changes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|c| voulu(c["texte"].as_str().unwrap_or(""), c["article"].as_str().unwrap_or("")))
        .cloned()
        .collect();
    let mut lues: Vec<String> = ent.lues().into_iter().collect();
    lues.sort();
    ecrire(racine, &articles, &lues, toutes.len(), &changes_voulus, &jour)?;
    let mut manque: Vec<String> = TEXTES
        .iter()
        .flat_map(|t| t.domaines.iter().flat_map(|d| d.numeros.iter()).map(move |n| (t.cle, *n)))
        .filter(|(c, n)| !articles.contains_key(&dila::cle(&[c, n])))
        .map(|(c, n)| format!("{} {}", c, n))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    manque.sort();
    let mut perimes: Vec<String> = articles
        .iter()
        .filter(|(_, a)| a.get("en_vigueur") != Some(&json!(true)))
        .map(|(k, _)| dila::parties(k).join(" "))
        .collect();
    perimes.sort();
    let mut vus_changes: Vec<String> = changes_voulus
        .iter()
        .map(|c| format!("{} ({})", c["article"].as_str().unwrap_or(""), c["livraison"].as_str().unwrap_or("")))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    vus_changes.sort();
    let toutes_ens: HashSet<&String> = toutes.iter().collect();
    dire("== fini ==");
    dire(&format!("  livraisons : {} appliquees sur {}", ent.lues().iter().filter(|l| toutes_ens.contains(l)).count(), toutes.len()));
    dire(&format!("  manquants : {}", if manque.is_empty() { "aucun".to_string() } else { manque.join(" ") }));
    // LE GARDE-FOU : un article garde dans une version perimee ferait appliquer un droit abroge sans que rien ne le signale.
    dire(&format!("  perimes   : {}", if perimes.is_empty() { "aucun".to_string() } else { perimes.join(" ") }));
    dire(&format!("  changes   : {}", if vus_changes.is_empty() { "aucun".to_string() } else { vus_changes.join(" ") }));
    for l in journal.iter().take(20) {
        dire(&format!("  ! {}", l));
    }
    Ok(())
}

/// `ent.etat["changes"] += changes`.
fn ajouter_changes(ent: &mut Entrepot, changes: Vec<Value>) {
    let v = ent.etat.entry("changes").or_insert_with(|| json!([]));
    if !v.is_array() {
        *v = json!([]);
    }
    if let Some(a) = v.as_array_mut() {
        a.extend(changes);
    }
}

/// Les textes de `absents`, lus dans l'archive globale puis dans chaque livraison deja appliquee a l'entrepot, dans
/// l'ordre : l'entrepot les tient ensuite comme s'il les avait gardes des le depart. Le cout est celui d'une construction
/// (l'archive globale en flux, puis les livraisons), mais une fois par texte ajoute.
fn completer(ent: &mut Entrepot, absents: &[&str], journal: &mut Vec<String>, source: &dyn Source, jour: &str) -> R<()> {
    dire(&format!("== textes ajoutes depuis la construction : {} ==", absents.join(", ")));
    let (vus, sec) = passer_le_global(ent, journal, source, Some(absents))?;
    dire(&format!("  {} fichiers d'article de ces textes lus en {:.0}s", vus, sec));
    let mut lues: Vec<String> = ent.lues().into_iter().collect();
    lues.sort();
    let urls: Vec<String> = lues.iter().map(|n| format!("{}LEGI_{}.tar.gz", BASE, n)).collect();
    let mut panne: Option<String> = None;
    dila::en_ordre(source, &urls, 8, |i, octets| {
        let r = octets.and_then(|o| passer(&mut dila::archive(&o), &mut ent.articles, journal, None, &lues[i], true, Some(absents)));
        if let Err(e) = r {
            // Une livraison manquee laisserait ces textes dans une version ancienne sans le dire : on s'arrete, et on
            // n'ecrit rien.
            panne = Some(format!("livraison {} : {} -- les textes {} ne sont pas completes", lues[i], e, absents.join(", ")));
            return false;
        }
        true
    });
    if let Some(e) = panne {
        return Err(e);
    }
    ent.sauver(jour)?;
    dire(&format!("  {} livraisons rejouees pour ces textes", lues.len()));
    Ok(())
}

/// Un releve par texte, chacun dans son dossier.
///
/// CHAQUE DOSSIER SE LIT SEUL : son `releve.json` ne parle que de ses articles, et le nombre de livraisons lues y est
/// repete. Un texte dont le releve ne dirait ou il en est que par le dossier d'a cote serait un releve qu'on croit complet.
fn ecrire(racine: &Path, articles: &Map<String, Value>, lues: &[String], totales: usize, changes: &[Value], jour: &str) -> R<()> {
    let totales = if totales == 0 { lues.len() } else { totales };
    for texte in TEXTES {
        let dossier = racine.join(texte.dossier);
        // Le XML de chaque article dans `xml/`, le releve a cote : c'est la piece justificative qui reste a cote du compte
        // rendu.
        let px = dossier.join(XML);
        std::fs::create_dir_all(&px).map_err(|e| format!("{} : {}", px.display(), e))?;
        let mut siens = Map::new();
        for (k, a) in articles {
            let p = dila::parties(k);
            if p[0] != texte.cle {
                continue;
            }
            let num = p.get(1).copied().unwrap_or("");
            let f = px.join(format!("{}.xml", fichier(num)));
            std::fs::write(&f, a["xml"].as_str().unwrap_or("")).map_err(|e| format!("{} : {}", f.display(), e))?;
            let sans_xml: Map<String, Value> = a.as_object().into_iter().flatten().filter(|(c, _)| *c != "xml").map(|(c, v)| (c.clone(), v.clone())).collect();
            siens.insert(num.to_string(), Value::Object(sans_xml));
        }
        let siens_changes: Vec<Value> =
            changes.iter().filter(|c| c.get("texte").and_then(Value::as_str).unwrap_or("travail") == texte.cle).cloned().collect();
        let releve = json!({
            "_source": BASE, "_global": GLOBAL,
            "_texte": texte.nom, "_legitext": texte.legitext,
            "_livraisons_lues": lues.len(),
            "_livraisons_totales": totales,
            // Ce qui reste a lire quand un passage s'interrompt : le document le dira, plutot que de se donner pour complet.
            "_complet": lues.len() == totales,
            "_derniere_livraison": lues.last().cloned().unwrap_or_default(),
            "_changes_depuis_le_global": siens_changes,
            "_releve_le": jour,
            "articles": siens,
        });
        let p = dossier.join("releve.json");
        std::fs::write(&p, crate::ecriture::json_py(&releve, 1)).map_err(|e| format!("{} : {}", p.display(), e))?;
    }
    Ok(())
}

/// Le nom de fichier d'un article. « 80 duodecies » en est un aussi.
pub fn fichier(num: &str) -> String {
    num.replace(' ', "_").replace('/', "-")
}

// ------------------------------------------------------------------ rapport

/// Le texte d'un article (son `BLOC_TEXTUEL`), balises en sauts de ligne, entites decodees, lignes vides retirees.
fn contenu(xml: &str) -> String {
    static BALISE: OnceLock<Regex> = OnceLock::new();
    static VIDES: OnceLock<Regex> = OnceLock::new();
    let Some(t) = groupe(re_texte(), xml) else {
        return String::new();
    };
    let t = motif(&BALISE, r"<[^>]+>").replace_all(t, "\n");
    let t = dila::unescape_html(&t);
    // `\s` du Python : le blanc d'Unicode, plus \x1c a \x1f.
    let t = motif(&VIDES, r"\n[\s\x1C-\x1F]*\n+").replace_all(&t, "\n");
    dila::strip_py(&t).to_string()
}

/// 20250713 -> 13/07/2025. Une date se lit, elle ne se dechiffre pas.
fn jour_lisible(aaaammjj: &str) -> String {
    let c: Vec<char> = aaaammjj.chars().collect();
    if c.len() < 8 || !c[..8].iter().all(|x| x.is_ascii_digit()) {
        return aaaammjj.to_string(); // pas une date : on la rend telle quelle
    }
    let s = |a: usize, b: usize| c[a..b].iter().collect::<String>();
    format!("{}/{}/{}", s(6, 8), s(4, 6), s(0, 4))
}

/// La seule phrase qui compte au deuxieme passage.
///
/// « Aucun n'a change » n'a de valeur que dit par une machine qui a reellement passe les livraisons -- et il faut alors
/// dire combien.
fn ce_qui_a_change(meta: &Value) -> String {
    let changes: Vec<&Value> = meta.get("_changes_depuis_le_global").and_then(Value::as_array).into_iter().flatten().collect();
    if changes.is_empty() {
        let n = meta["articles"].as_object().map_or(0, Map::len);
        return format!("**aucun de ces {} articles n'a change de version** depuis l'archive globale", n);
    }
    format!(
        "**{} article(s) ont change de version** : {}",
        changes.len(),
        changes
            .iter()
            .map(|c| format!(
                "{} ({} -> {}, livraison du {})",
                dila::texte(&c["article"]),
                dila::texte(&c["avant"]),
                dila::texte(&c["apres"]),
                jour_lisible(dila::debut_py(&dila::texte(&c["livraison"]), 8))
            ))
            .collect::<Vec<_>>()
            .join(" ; ")
    )
}

/// Le document lisible de chaque texte : un titre par domaine, un article par entree. `quel` limite a un seul texte.
pub fn rapport(racine: &Path, quel: &str, aujourdhui: &Jour) -> R<()> {
    for texte in TEXTES {
        if !quel.is_empty() && texte.cle != quel {
            continue;
        }
        let dossier = racine.join(texte.dossier);
        let cible = dossier.join(format!("articles-{}.md", aujourdhui.aaaammjj()));
        rapport_d_un_texte(&dossier, &cible, texte)?;
    }
    Ok(())
}

/// Un fichier texte lu comme le Python l'ouvre (`open(..., encoding="utf-8")`) : fins de ligne universelles.
fn lire_texte(p: &Path) -> R<String> {
    let t = std::fs::read_to_string(p).map_err(|e| format!("{} : {}", p.display(), e))?;
    Ok(if t.contains('\r') { t.replace("\r\n", "\n").replace('\r', "\n") } else { t })
}

fn lire_json(p: &Path) -> R<Value> {
    serde_json::from_str(&lire_texte(p)?).map_err(|e| format!("{} illisible : {}", p.display(), e))
}

fn rapport_d_un_texte(dossier: &Path, cible: &Path, texte: &Texte) -> R<()> {
    let meta = lire_json(&dossier.join("releve.json"))?;
    let n_articles = meta["articles"].as_object().map_or(0, Map::len);
    let lues = meta["_livraisons_lues"].as_u64().unwrap_or(0);
    let totales = meta.get("_livraisons_totales").and_then(Value::as_u64).unwrap_or(lues);
    let mut lignes: Vec<String> = vec![
        format!("# {}", texte.titre),
        String::new(),
        "**Releve automatiquement**, et c'est l'interet : un article lu a la main sur Legifrance -- qui refuse les robots \
         (403) -- se perime en silence. Ceux-ci viennent de la base **LEGI** de la DILA, en open data, par \
         `paie releve-codes` -- la meme voie que KALI pour les conventions collectives (§29). Le releve se REFAIT, et \
         c'est tout le sujet : il dit ce qui a bouge depuis la derniere fois."
            .to_string(),
        String::new(),
        "| | |".into(),
        "|---|---|".into(),
        format!("| source | `{}` |", dila::texte(&meta["_source"])),
        format!("| archive globale | `{}` |", dila::texte(&meta["_global"])),
        format!("| code | {} (`{}`) |", texte.nom, texte.legitext),
        format!("| livraisons lues | **{} sur {}**, dans l'ordre depuis le {} |", lues, totales, jour_lisible(DEPUIS)),
        format!("| ce qu'elles disent | {} |", ce_qui_a_change(&meta)),
        format!("| releve le | {} |", dila::texte(&meta["_releve_le"])),
        format!("| articles | {} |", n_articles),
        String::new(),
    ];
    // CE QUI RESTE A LIRE, quand un passage s'est interrompu. Un releve qui tairait la difference se donnerait pour
    // complet, et c'est precisement ce qu'on ne veut pas d'une source. Le defaut se DEDUIT des nombres plutot que d'un
    // booleen absent : un releve ecrit par une version plus ancienne du moissonneur ne doit pas se donner pour complet par
    // omission.
    let complet = match meta.get("_complet") {
        Some(v) => dila::vrai(v),
        None => lues >= meta.get("_livraisons_totales").and_then(Value::as_u64).unwrap_or(0),
    };
    if !complet {
        let reste = totales as i64 - lues as i64;
        lignes.push(format!(
            "**PASSAGE INCOMPLET.** Il reste **{} livraisons** a passer -- les plus recentes. Ce qui suit est donc vrai au {} \
             pour les {} premieres ; `paie releve-codes moissonner` le refait en entier, et c'est tout l'interet : un \
             releve a la main ne se refait pas.",
            reste,
            jour_lisible(dila::debut_py(&dila::texte(&meta["_derniere_livraison"]), 8)),
            lues
        ));
        lignes.push(String::new());
    }
    lignes.push(
        "**ET LA VERSION EST VERIFIEE.** Le dossier du dump s'appelle `code_en_vigueur`, mais il porte AUSSI les versions \
         anterieures de chaque article : la premiere version du moissonneur en a garde sept sur cinquante-cinq dans une \
         version PERIMEE, dont L3141-3 du code du travail (les 2,5 jours de conges) et R1234-2 (l'indemnite de \
         licenciement). Chaque article ci-dessous est verifie en vigueur -- `<ETAT>` a VIGUEUR et `<DATE_FIN>` a \
         2999-01-01 --, et celui qui ne le serait pas est signale a sa ligne."
            .into(),
    );
    lignes.push(String::new());
    lignes.push(
        "Chaque article porte l'identifiant de SA VERSION (`LEGIARTI...`) et la date depuis laquelle elle est en vigueur : \
         c'est ce qui permet de dire, au prochain relevé, si quelque chose a bouge."
            .into(),
    );
    lignes.push(String::new());
    for domaine in texte.domaines {
        lignes.extend(["---".to_string(), String::new(), format!("## {}", domaine.titre), String::new()]);
        for num in domaine.numeros {
            let a = meta["articles"].get(*num).filter(|a| a.as_object().is_some_and(|o| !o.is_empty()));
            let Some(a) = a else {
                lignes.extend([format!("### {} -- ABSENT DU RELEVE", num), String::new()]);
                continue;
            };
            let xml = lire_texte(&dossier.join(XML).join(format!("{}.xml", fichier(num))))?;
            let etat = if dila::vrai(&a["en_vigueur"]) { "" } else { "  **VERSION PERIMEE**" };
            lignes.push(format!("### {} (`{}`, en vigueur depuis le {}){}", num, dila::texte(&a["id"]), dila::texte(&a["debut"]), etat));
            lignes.push(String::new());
            lignes.push(format!("> {}", contenu(&xml).replace('\n', "\n> ")));
            lignes.push(String::new());
        }
    }
    let mut t = lignes.join("\n");
    t.push('\n');
    std::fs::write(cible, t).map_err(|e| format!("{} : {}", cible.display(), e))?;
    dire(&format!("{} articles -> {}", n_articles, cible.display()));
    Ok(())
}

// ------------------------------------------------------------------ comparer

/// Tous les changements de version notes par le dernier `moissonner`, tous textes confondus. C'est le releve qui dit ce
/// qu'il y a a comparer : on ne redecide pas ici quels articles ont bouge.
fn changements(racine: &Path) -> R<Vec<Map<String, Value>>> {
    let mut tout = Vec::new();
    for texte in TEXTES {
        let p = racine.join(texte.dossier).join("releve.json");
        if !p.exists() {
            continue;
        }
        let meta = lire_json(&p)?;
        for c in meta.get("_changes_depuis_le_global").and_then(Value::as_array).into_iter().flatten() {
            let mut c = c.as_object().cloned().unwrap_or_default();
            let t = c.get("texte").cloned().unwrap_or_else(|| json!(texte.cle));
            c.insert("texte".into(), t);
            tout.push(c);
        }
    }
    Ok(tout)
}

fn champ(c: &Map<String, Value>, k: &str) -> String {
    c.get(k).map(dila::texte).unwrap_or_default()
}

/// Va chercher dans l'archive globale les versions d'AVANT, par leur IDENTIFIANT (le piege propre a `comparer`). Rend
/// {(texte, article): xml} et ecrit chaque piece a cote des autres, dans `<texte>/xml-anterieur/`.
///
/// ON S'ARRETE DES QU'ON LES A TOUTES. L'archive porte 1,75 million d'articles et les versions cherchees ne sont qu'une
/// poignee : sortir au dernier trouve a economise un cinquieme du defilement, le 21/09/2026.
fn avant(racine: &Path, changements: &[Map<String, Value>], source: &dyn Source) -> R<HashMap<(String, String), String>> {
    let mut vise: HashMap<String, (String, String)> = HashMap::new();
    for c in changements {
        vise.insert(champ(c, "avant"), (champ(c, "texte"), champ(c, "article")));
    }
    let (mut trouves, mut vus, t0) = (HashMap::new(), 0usize, Instant::now());
    let mut tar = dila::tar_gz(source.flux(&format!("{}{}", BASE, GLOBAL))?);
    while let Some(m) = tar.suivant()? {
        if !m.fichier || !m.nom.contains("/article/") {
            continue;
        }
        vus += 1;
        let Some(id) = groupe(re_id(), &m.nom).map(String::from) else {
            continue;
        };
        let Some((cle, num)) = vise.get(&id).cloned() else {
            continue;
        };
        let xml = String::from_utf8_lossy(&tar.lire()?).into_owned();
        let dossier = racine.join(texte_de(&cle).map(|t| t.dossier).ok_or_else(|| format!("texte inconnu : {}", cle))?).join(ANTERIEUR);
        std::fs::create_dir_all(&dossier).map_err(|e| format!("{} : {}", dossier.display(), e))?;
        let f = dossier.join(format!("{}.xml", fichier(&num)));
        std::fs::write(&f, &xml).map_err(|e| format!("{} : {}", f.display(), e))?;
        trouves.insert((cle, num.clone()), xml);
        dire(&format!("  {} {} ({}/{})", num, id, trouves.len(), vise.len()));
        if trouves.len() == vise.len() {
            break;
        }
    }
    dire(&format!(
        "  {} fichiers parcourus en {:.0}s ; {}/{} versions anterieures retrouvees",
        vus,
        t0.elapsed().as_secs_f64(),
        trouves.len(),
        vise.len()
    ));
    Ok(trouves)
}

/// Le texte d'un article, alinea par alinea, espaces normalises.
///
/// On compare le BLOC_TEXTUEL et lui seul : ni les visas, ni les liens, ni les notes. Un changement qui n'y figurerait pas
/// ECHAPPE a cette lecture, et le document le dit plutot que de le taire.
fn alineas(xml: &str) -> Vec<String> {
    static ESPACES: OnceLock<Regex> = OnceLock::new();
    contenu(xml)
        .split('\n')
        .filter(|l| !dila::strip_py(l).is_empty())
        .map(|l| dila::strip_py(&motif(&ESPACES, r"[\s\x1C-\x1F]+").replace_all(l, " ")).to_string())
        .collect()
}

/// Relit chaque article qui a change contre sa version d'avant, et ecrit le document des differences.
pub fn comparer(racine: &Path, source: &dyn Source, aujourdhui: &Jour) -> R<()> {
    let changements = changements(racine)?;
    let cible = racine.join(format!("changements-de-version-{}.md", aujourdhui.aaaammjj()));
    if changements.is_empty() {
        // RIEN A COMPARER N'EST PAS RIEN A DIRE : le document existe quand meme, et il date le passage qui n'a rien trouve.
        let t = format!(
            "# Les changements de version : aucun\n\nLe dernier `moissonner` n'a note **aucun** changement de version depuis \
             l'archive globale `{}`. Il n'y a donc rien a relire.\n\n*Etabli le {}.*\n",
            GLOBAL,
            aujourdhui.jj_mm_aaaa()
        );
        std::fs::write(&cible, t).map_err(|e| format!("{} : {}", cible.display(), e))?;
        dire(&format!("aucun changement -> {}", cible.display()));
        return Ok(());
    }
    dire(&format!("== {} article(s) a relire contre leur version d'avant ==", changements.len()));
    let mut essai = 1;
    let anterieures = loop {
        match avant(racine, &changements, source) {
            Ok(a) => break a,
            Err(e) => {
                dire(&format!("  archive globale, essai {} : {}", essai, e));
                if essai == 3 {
                    return Err(e);
                }
                source.pause(10 * essai);
                essai += 1;
            }
        }
    };
    ecrire_comparaison(racine, &cible, &changements, &anterieures, aujourdhui)
}

fn ecrire_comparaison(
    racine: &Path,
    cible: &Path,
    changements: &[Map<String, Value>],
    anterieures: &HashMap<(String, String), String>,
    aujourdhui: &Jour,
) -> R<()> {
    let mut lignes: Vec<String> = vec![
        "# Ce qui a change DANS LE TEXTE des articles qui ont change de version".into(),
        String::new(),
        format!("*Etabli le {} par `paie releve-codes comparer`.*", aujourdhui.jj_mm_aaaa()),
        String::new(),
        "Changer de version n'est pas changer de sens : un article dont un alinea etranger a la paie est reecrit recoit un \
         identifiant neuf sans que rien de ce qui nous concerne bouge. Ce document montre, article par article, **ce qui a \
         reellement change dans le texte** -- il ne dit pas ce qu'il faut en faire, ce qui demande une lecture."
            .into(),
        String::new(),
        format!(
            "Les versions d'avant viennent de l'archive globale `{}`, cherchees **par leur identifiant** et non par leur \
             numero : on ne voulait pas une vieille version, on voulait celle que le relevé precedent tenait pour en \
             vigueur. Elles sont gardees dans `<code>/{}/`, a cote des pieces justificatives.",
            GLOBAL, ANTERIEUR
        ),
        String::new(),
        "La comparaison porte sur le `BLOC_TEXTUEL` seul -- ni visas, ni liens, ni notes. **Un changement qui n'y \
         figurerait pas echappe a cette lecture.**"
            .into(),
        String::new(),
    ];
    let mut tries: Vec<&Map<String, Value>> = changements.iter().collect();
    tries.sort_by(|a, b| (champ(a, "texte"), champ(a, "article")).cmp(&(champ(b, "texte"), champ(b, "article"))));
    for c in tries {
        let (cle, article) = (champ(c, "texte"), champ(c, "article"));
        let texte = texte_de(&cle).ok_or_else(|| format!("texte inconnu : {}", cle))?;
        let depuis = champ(c, "depuis");
        lignes.extend([
            "---".to_string(),
            String::new(),
            format!("## {} ({})", article, texte.nom),
            String::new(),
            format!(
                "`{}` -> `{}`, en vigueur depuis le {}, livraison du {}.",
                champ(c, "avant"),
                champ(c, "apres"),
                if depuis.is_empty() { "?".to_string() } else { jour_lisible(dila::debut_py(&depuis.replace('-', ""), 8)) },
                jour_lisible(dila::debut_py(&champ(c, "livraison"), 8))
            ),
            String::new(),
        ]);
        let Some(xml_a) = anterieures.get(&(cle.clone(), article.clone())) else {
            // UNE VERSION INTROUVABLE SE DIT. La taire laisserait croire que l'article a ete relu, alors qu'il ne l'a pas ete.
            lignes.extend([
                "**Version anterieure INTROUVABLE dans l'archive globale.** Cet article n'a donc PAS ete relu contre son passe."
                    .to_string(),
                String::new(),
            ]);
            continue;
        };
        let xml_b = lire_texte(&racine.join(texte.dossier).join(XML).join(format!("{}.xml", fichier(&article))))?;
        let (a, b) = (alineas(xml_a), alineas(&xml_b));
        if a == b {
            lignes.extend([format!("**Texte identique** : {} alineas de part et d'autre. Seule la version a change.", a.len()), String::new()]);
            continue;
        }
        lignes.extend([format!("{} alineas -> {}. Ce qui differe :", a.len(), b.len()), String::new(), "```diff".into()]);
        for l in difflib::unifie(&a, &b, "avant", "apres", 1) {
            if !(l.starts_with("---") || l.starts_with("+++") || l.starts_with("@@")) {
                lignes.push(l);
            }
        }
        lignes.extend(["```".to_string(), String::new()]);
    }
    let mut t = lignes.join("\n");
    t.push('\n');
    std::fs::write(cible, t).map_err(|e| format!("{} : {}", cible.display(), e))?;
    dire(&format!("{} article(s) -> {}", changements.len(), cible.display()));
    Ok(())
}

/// `difflib.unified_diff` du Python, pour les essais.
pub fn diff_unifie(a: &[String], b: &[String], n: usize) -> Vec<String> {
    difflib::unifie(a, b, "avant", "apres", n)
}
