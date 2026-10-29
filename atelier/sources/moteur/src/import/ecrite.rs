//! `importer` : ecrit ce que `importation-apercu` annonce.
//!
//! `{"dossier": ...}` : la boite aux lettres `import/` entiere, comme l'apercu (`import::bulletins`). Dans l'ordre : ce
//! qui est repris (DSN, AEM, PDF dont une page au moins est reprise) est COPIE dans `historique/bulletins/`, sous le nom
//! de sa copie (`<empreinte>.<pdf|dsn|txt>` : rien de personnel ; `import::bulletins::nom_copie`) -- rien ne s'ecrase ;
//! l'apercu est relu DANS ces copies, et c'est lui qui s'ecrit ; l'alerte CASC-SVP (`alerte_apres_import`) ; les fiches
//! a creer (entreprise, salaries, spectacles), ecrites comme celles de l'ecran (`crate::routes::fiches::ecrire_fiche`,
//! `forcer`) ; les ENGAGEMENTS IMPORTES (`engagement::importe`), chacun dans `engagements/AAAA-MM.json`, le fichier du
//! mois ou il commence : le contrat qu'une AEM ou un bulletin atteste, et ce qui a ete verse, mois par mois (la DSN
//! deposee, completee par le bulletin). Un import se rejoue sans dommage. La reponse est celle de l'apercu, completee de
//! ce qui a ete ecrit (`ecrit`, `fichiers`, `copie`) ; elle garde le compte rendu (`tri`) du dossier lu, pour l'ecran ;
//! rien n'en part (l'incident est `import::incident`, sur le clic de la personne). Les pieces copiees restent sous
//! `historique/` : le dossier du salarie montre les pages des bulletins, l'alerte CASC-SVP relit les DSN.
//!
//! UNE PAIE, DEUX SOURCES, UNE LIGNE (les deux chemins) : une ligne de DSN qui arrive sur une ligne que des bulletins
//! ont deja completee garde ce que le bulletin a ecrit (le bulletin fait foi) ; une ligne qu'un bulletin seul declare
//! s'efface devant la ligne de DSN des memes salarie, debut et fin, qu'elle complete (`rapprocher`) -- l'ordre DSN
//! puis bulletins, ou bulletins puis DSN, donne les memes engagements.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use socle::valeur::valeur_en_texte;
use socle::valeur::nom_type;
use socle::echec::erreur;
use socle::fichiers::vrai;
use socle::num::Decimal;
use socle::echec::Echec;

type R<T> = Result<T, Echec>;


/// Les colonnes qu'un bulletin ajoute a une paie (`import::bulletins::completer`).
const COLONNES_BULLETIN: [&str; 12] = [
    "spectacle",
    "analytique",
    "cotisations_salariales",
    "cotisations_patronales",
    "cout_employeur",
    "net_social",
    "net_avant_impot",
    "lignes",
    "dates",
    "cotisations",
    "emploi_imprime",
    "bulletin",
];
/// Les montants sur lesquels le bulletin l'emporte.
const MONTANTS_BULLETIN: [&str; 4] = ["brut", "net_imposable", "net_a_payer", "pas"];

fn en_os(e: &std::io::Error, p: &Path) -> Echec {
    erreur("OSError", format!("{} : {}", p.display(), e))
}

fn jj_mm_aaaa(j: &socle::date::Jour) -> String {
    format!("{:02}/{:02}/{:04}", j.jour, j.mois, j.an)
}

/// Les cles qui commencent par `_` s'en vont.
fn sans_prives(v: &Value) -> Value {
    match v {
        Value::Object(o) => Value::Object(o.iter().filter(|(k, _)| !k.starts_with('_')).map(|(k, x)| (k.clone(), sans_prives(x))).collect()),
        Value::Array(a) => Value::Array(a.iter().map(sans_prives).collect()),
        autre => autre.clone(),
    }
}

// ------------------------------------------------------------------ l'alerte CASC-SVP

/// Un montant de l'alerte CASC-SVP : `1 234,50 €`.
fn eur_fr(v: Decimal) -> String {
    let t = format!("{:.2}", v);
    let (entier, dec) = t.split_once('.').unwrap_or((&t, "00"));
    let (signe, chiffres) = match entier.strip_prefix('-') {
        Some(r) => ("-", r),
        None => ("", entier),
    };
    let mut groupes: Vec<String> = Vec::new();
    let c: Vec<char> = chiffres.chars().collect();
    let mut fin = c.len();
    while fin > 3 {
        groupes.push(c[fin - 3..fin].iter().collect());
        fin -= 3;
    }
    groupes.push(c[..fin].iter().collect());
    groupes.reverse();
    format!("{}{},{} €", signe, groupes.join(" "), dec)
}

/// « janvier 2025 » : le nom d'un mois de l'alerte CASC-SVP. « 00 » n'est aucun mois et se refuse
/// (`date::numero_du_mois`), comme un mois illisible.
fn libelle_casc(m: &str) -> R<String> {
    let n = socle::date::numero_du_mois(m).map_err(|e| erreur("ValueError", e))?;
    let nom = socle::date::nom_du_mois(n as i64, socle::date::Graphie::Accentuee).map_err(|e| erreur("ValueError", e))?;
    Ok(format!("{} {}", nom, m.chars().take(4).collect::<String>()))
}

/// La DSN qu'un mois importe a deposee hors de PuppetPaie : le fichier que l'import a retenu pour ce mois (la paie
/// versee d'un engagement importe), sa copie sous `historique/bulletins/`, ou qu'est la compagnie. `None` : un mois que
/// l'import n'a pas retenu ; un fichier retenu introuvable refuse.
pub(crate) fn dsn_importee(ctx: &Contexte, mois: &str) -> Result<Option<PathBuf>, String> {
    let lignes: Vec<Value> = engagement::importe::paies_lues(&ctx.racine)?
        .into_iter()
        .filter(|x| x.get("mois").and_then(Value::as_str) == Some(mois) && x.get("fichier").is_some_and(vrai))
        .collect();
    let Some(ligne) = lignes.last() else { return Ok(None) };
    let chemin = dossier_bulletins(ctx).join(valeur_en_texte(&ligne["fichier"]));
    if chemin.exists() {
        return Ok(Some(chemin));
    }
    let nom = valeur_en_texte(&ligne["fichier"]);
    Err(format!("La DSN de {} ({}) a ete importee, mais le fichier n'est plus sous historique/.", mois, nom))
}

/// `lire_dsn_casc` : la DSN deposee, et si elle porte la CASC-SVP (un contrat rattache, S21.G00.70.013, a
/// l'adhesion que la DSN nomme `contrat` en bloc 15). `None` : illisible.
pub(crate) fn lire_dsn_casc(chemin: &Path, contrat: &str) -> Option<(Value, bool)> {
    let d = crate::import::dsn_aem::lire_une_dsn(chemin)?;
    let octets = std::fs::read(chemin).ok()?;
    let (mut refs, mut rattachees): (HashSet<String>, HashSet<String>) = (HashSet::new(), HashSet::new());
    let mut libelle: Option<String> = None;
    for ligne in socle::texte::lignes_du_texte(&socle::texte::texte_latin1(&octets)) {
        let (k, v) = ligne.split_once(',').unwrap_or((&ligne, ""));
        let v = v.trim_matches(socle::texte::blanc).trim_matches('\'');
        if k == "S21.G00.15.001" {
            libelle = Some(v.to_string());
        } else if k == "S21.G00.15.005" && libelle.as_deref() == Some(contrat) {
            refs.insert(v.to_string());
        } else if k == "S21.G00.70.013" {
            rattachees.insert(v.to_string());
        }
    }
    let porte = refs.iter().any(|r| rattachees.contains(r));
    Some((d, porte))
}

/// L'alerte apres l'import : une compagnie de la 3090 qui arrive de CoolPaie avec des DSN de 2025
/// ou apres : la CASC-SVP y manque peut-etre. Rien si ce n'est pas la 3090, ou si tout est regularise ; toute
/// `ErreurOperation` de la regularisation (`regularisation_casc_svp(ecrire=False)`) vaut « rien ». Seuls comptent
/// le nombre de mois, la periode et le total, que cette fonction calcule comme la regularisation.
fn alerte_apres_import(ctx: &Contexte) -> R<Option<String>> {
    let base = declarations::casc::base().map_err(Echec::from)?;
    let ent = &ctx.entreprise;
    let idcc = ent.get("idcc").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
    if idcc != valeur_en_texte(&base["idcc"]) {
        return Ok(None);
    }
    if !matches!(droit::bareme::casc_exemptee(ent), Ok(false)) {
        return Ok(None);
    }
    let faits: HashSet<String> = declarations::casc::mois_regularises(ctx).map_err(Echec::from)?.into_iter().collect();
    // `mois_disponibles` : les mois des paies declarees, et les mois saisis.
    let mut dispo: BTreeSet<String> = BTreeSet::new();
    for x in engagement::importe::paies_lues(&ctx.racine).map_err(Echec::from)? {
        if let Some(m) = x.get("mois").filter(|m| vrai(m)) {
            dispo.insert(valeur_en_texte(m));
        }
    }
    dispo.extend(crate::routes::etapes::mois_saisis(&ctx.racine));
    let depuis = valeur_en_texte(&base["depuis"]);
    let contrat = valeur_en_texte(&base["dsn"]["contrat"]);
    let mut par_mois: Vec<(String, Decimal)> = Vec::new();
    for m in dispo.iter().filter(|m| **m >= depuis && !faits.contains(*m)) {
        let chemin = match dsn_importee(ctx, m) {
            Ok(Some(c)) => c,
            Ok(None) => continue,
            Err(_) => return Ok(None),
        };
        let Some((d, porte)) = lire_dsn_casc(&chemin, &contrat) else { return Ok(None) };
        if porte {
            continue;
        }
        let mut masse = socle::num::zero2();
        for v in d["versements"].as_array().into_iter().flatten() {
            let Some(brut) = v.get("brut").filter(|b| vrai(b)) else { continue };
            let tb = valeur_en_texte(brut);
            let b = socle::num::decimal_lu(&tb).ok_or_else(|| erreur("InvalidOperation", socle::num::illisible(&tb)))?;
            masse += b;
        }
        let an: String = m.chars().take(4).collect();
        let Some(taux) = base["taux"].get(&an) else { return Ok(None) };
        let taux = socle::num::dec(taux).map_err(Echec::from)?;
        let cot = (masse * taux / Decimal::from(100)).round_dp_with_strategy(2, rust_decimal::RoundingStrategy::MidpointAwayFromZero);
        par_mois.push((m.clone(), cot));
    }
    if par_mois.is_empty() {
        return Ok(None);
    }
    let periode = format!("{} à {}", libelle_casc(&par_mois[0].0)?, libelle_casc(&par_mois[par_mois.len() - 1].0)?);
    let total = par_mois.iter().fold(Decimal::ZERO, |a, (_, c)| a + *c);
    Ok(Some(format!(
        "CASC-SVP : {} DSN deposee(s) depuis janvier 2025 ({}) ne portent peut-etre pas la cotisation CASC-SVP -- un logiciel qui n'etait pas parametre l'a omise. Si Audiens ne l'a pas encaissee, {} sont a regulariser : Exports > « regularisation-casc-svp » fait le dossier (virements, courriels, liste nominative).",
        par_mois.len(),
        periode,
        eur_fr(total)
    )))
}

// ------------------------------------------------------------------ l'historique

type CleEngagement = (String, String, String, String, i128, i128, i128, String);

/// `_cle_engagement` d'un engagement deja ecrit : `None` quand une colonne n'est pas un texte (elle ne peut
/// egaler celle d'un contrat neuf).
fn cle_engagement(e: &Value) -> R<Option<CleEngagement>> {
    for k in ["salarie", "debut", "fin"] {
        if e.get(k).is_none() {
            return Err(erreur("KeyError", socle::fichiers::absent(k)));
        }
    }
    let entier = |k: &str| -> R<i128> {
        let v = e.get(k).cloned().unwrap_or_else(|| json!(0));
        match &v {
            Value::Number(n) => Ok(n.as_i64().map(i128::from).unwrap_or_else(|| n.as_f64().unwrap_or(0.0) as i128)),
            Value::Bool(b) => Ok(i128::from(*b)),
            Value::String(s) => socle::num::entier_lu(s).ok_or_else(|| erreur("ValueError", format!("nombre entier illisible : {}", socle::fichiers::repr(s)))),
            autre => Err(erreur("TypeError", socle::fichiers::attendu("un nombre entier", nom_type(autre)))),
        }
    };
    let (h, c, j) = (entier("heures")?, entier("cachets")?, entier("jours")?);
    let texte = |v: Option<&Value>| match v {
        Some(Value::String(t)) => Some(t.clone()),
        _ => None,
    };
    let objet = match e.get("numero_objet") {
        None => Some(String::new()),
        autre => texte(autre),
    };
    let brut = e.get("brut").map_or("0".to_string(), valeur_en_texte);
    Ok(match (texte(e.get("salarie")), texte(e.get("debut")), texte(e.get("fin")), objet) {
        (Some(a), Some(b), Some(c2), Some(d)) => Some((a, b, c2, d, h, c, j, brut)),
        _ => None,
    })
}

fn cle_paie(x: &Value) -> R<(String, String, String)> {
    let k = |c: &str| x.get(c).map(valeur_en_texte).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(c)));
    Ok((k("mois")?, k("salarie")?, k("contrat")?))
}

// ------------------------------------------------------------------ une paie, deux sources

/// Une valeur qui dit quelque chose (ni `null`, ni un texte blanc).
fn plein(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::String(s) => !s.trim().is_empty(),
        _ => true,
    }
}

/// Une paie que des bulletins ont deja completee (ou ecrite).
fn porte_un_bulletin(x: &Value) -> bool {
    x.get("bulletin").is_some_and(Value::is_object)
}

fn declaree_par_bulletin(x: &Value) -> bool {
    x.get("declare_par").and_then(Value::as_str) == Some("bulletin")
}

/// Complete une paie par ce qu'un bulletin a ecrit d'elle (`b`), comme `import::bulletins::completer` : sur le
/// brut, le net imposable, le net a payer et le PAS, le bulletin l'emporte ; le numero d'objet et le jour du versement
/// remplissent ce qui est vide ; les colonnes du bulletin s'ajoutent.
fn completer_par(ligne: &mut Value, b: &Value) {
    for k in MONTANTS_BULLETIN {
        if let Some(v) = b.get(k).filter(|v| plein(v)) {
            ligne[k] = v.clone();
        }
    }
    for k in ["numero_objet", "verse_le"] {
        if !ligne.get(k).is_some_and(plein) {
            if let Some(v) = b.get(k).filter(|v| plein(v)) {
                ligne[k] = v.clone();
            }
        }
    }
    for k in COLONNES_BULLETIN {
        if let Some(v) = b.get(k).filter(|v| !v.is_null()) {
            ligne[k] = v.clone();
        }
    }
}

/// Une paie qu'un bulletin seul declare (`declare_par: "bulletin"`) s'efface devant la paie declaree des memes
/// salarie, debut et fin, qu'elle complete (une seule ; plusieurs : celle du meme mois, sinon rien ne se rapproche).
/// Rend le nombre de paies de bulletin laissees a cote de plusieurs paies declarees.
fn rapprocher(par_cle: &mut BTreeMap<(String, String, String), Value>) -> usize {
    let t = |x: &Value, k: &str| x.get(k).map(valeur_en_texte).unwrap_or_default();
    let de_bulletin: Vec<(String, String, String)> = par_cle.iter().filter(|(_, x)| declaree_par_bulletin(x)).map(|(k, _)| k.clone()).collect();
    let mut ambigues = 0usize;
    for kb in de_bulletin {
        let b = par_cle[&kb].clone();
        let (salarie, debut, fin) = (t(&b, "salarie"), t(&b, "debut"), t(&b, "fin"));
        if salarie.is_empty() || salarie == "<a creer>" || debut.is_empty() || fin.is_empty() {
            continue;
        }
        let mut cibles: Vec<(String, String, String)> = par_cle
            .iter()
            .filter(|(_, x)| !declaree_par_bulletin(x) && t(x, "salarie") == salarie && t(x, "debut") == debut && t(x, "fin") == fin)
            .map(|(k, _)| k.clone())
            .collect();
        if cibles.len() > 1 {
            cibles.retain(|k| k.0 == kb.0);
        }
        // UN CONTRAT A CHEVAL SUR DEUX MOIS : le bulletin imprime la periode du mois, la DSN le contrat entier
        // (S21.G00.40.001 et .010, `norme-dsn/texte/bloc-S21.G00.40.txt`). Le bulletin complete la paie declaree du meme
        // mois dont le contrat contient sa periode, quand il n'y en a qu'une.
        if cibles.is_empty() {
            cibles = par_cle
                .iter()
                .filter(|(k, x)| {
                    let (d, f) = (t(x, "debut"), t(x, "fin"));
                    !declaree_par_bulletin(x) && k.0 == kb.0 && t(x, "salarie") == salarie && !d.is_empty() && d <= debut && (f.is_empty() || fin <= f)
                })
                .map(|(k, _)| k.clone())
                .collect();
        }
        match cibles.as_slice() {
            [] => {}
            [kd] => {
                if let Some(d) = par_cle.get_mut(kd) {
                    if !porte_un_bulletin(d) {
                        completer_par(d, &b);
                    }
                }
                par_cle.remove(&kb);
            }
            _ => ambigues += 1,
        }
    }
    ambigues
}

/// Une ligne sans ce qui dit quand elle a ete importee : ce qui compare deux lectures des memes pieces.
fn sans_dates_d_import(x: &Value) -> Value {
    let mut x = x.clone();
    if let Some(o) = x.as_object_mut() {
        o.remove("importe_le");
    }
    x
}

/// Une ligne sans les cles que l'engagement importe porte lui-meme.
fn sans(x: &Value, cles: &[&str]) -> Map<String, Value> {
    let mut o = x.as_object().cloned().unwrap_or_default();
    for k in cles {
        o.remove(*k);
    }
    o
}

/// LES ENGAGEMENTS IMPORTES, tires des contrats attestes (`contrats`, une AEM ou un bulletin chacun) et des paies
/// versees (`paies`, une par mois et par contrat). Un engagement, c'est un salarie, un debut et une fin : le contrat
/// atteste qui les porte (plusieurs : celui du meme numero d'objet que la paie), et ses paies, une par mois. Une paie
/// qu'aucun contrat atteste ne porte fait son engagement avec celles des memes salarie, debut, fin et numero de contrat.
/// Son numero est celui que la DSN deposee a declare (S21.G00.40.009), sinon celui que le bulletin imprime ; jamais un
/// numero tire.
fn engagements_importes(contrats: Vec<Value>, paies: Vec<Value>) -> Vec<Map<String, Value>> {
    let t = |x: &Value, k: &str| x.get(k).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
    struct Groupe {
        salarie: String,
        debut: String,
        fin: String,
        contrat: Option<Value>,
        paies: Vec<Value>,
    }
    let mut groupes: Vec<Groupe> = Vec::new();
    for c in contrats {
        groupes.push(Groupe { salarie: t(&c, "salarie"), debut: t(&c, "debut"), fin: t(&c, "fin"), contrat: Some(c), paies: Vec::new() });
    }
    for p in paies {
        let (s, d, f, o) = (t(&p, "salarie"), t(&p, "debut"), t(&p, "fin"), t(&p, "numero_objet"));
        let mut candidats: Vec<usize> = (0..groupes.len()).filter(|&i| groupes[i].contrat.is_some() && groupes[i].salarie == s && groupes[i].debut == d && groupes[i].fin == f).collect();
        // Le contrat qu'un bulletin atteste porte la periode du mois ; la paie declaree, le contrat entier : la periode
        // du bulletin dans le contrat declare, et l'engagement prend les dates du contrat.
        if candidats.is_empty() && !d.is_empty() && !declaree_par_bulletin(&p) {
            candidats = (0..groupes.len())
                .filter(|&i| {
                    let g = &groupes[i];
                    g.paies.is_empty()
                        && g.contrat.as_ref().is_some_and(|c| t(c, "atteste_par") == "bulletin")
                        && g.salarie == s
                        && d <= g.debut
                        && (f.is_empty() || g.fin <= f)
                })
                .collect();
            if let [i] = candidats.as_slice() {
                groupes[*i].debut = d.clone();
                groupes[*i].fin = f.clone();
            }
        }
        let atteste = match candidats.as_slice() {
            [i] => Some(*i),
            [] => None,
            plusieurs => {
                let memes: Vec<usize> = plusieurs.iter().copied().filter(|&i| groupes[i].contrat.as_ref().is_some_and(|c| t(c, "numero_objet").eq_ignore_ascii_case(&o))).collect();
                (memes.len() == 1).then(|| memes[0])
            }
        };
        let mois = t(&p, "mois");
        let libre = |g: &Groupe| !g.paies.iter().any(|x| t(x, "mois") == mois);
        let i = match atteste.filter(|&i| libre(&groupes[i])) {
            Some(i) => i,
            None => match (0..groupes.len()).find(|&i| {
                let g = &groupes[i];
                g.contrat.is_none() && g.salarie == s && g.debut == d && g.fin == f && g.paies.first().map(|x| t(x, "contrat")) == Some(t(&p, "contrat")) && libre(g)
            }) {
                Some(i) => i,
                None => {
                    groupes.push(Groupe { salarie: s, debut: d, fin: f, contrat: None, paies: Vec::new() });
                    groupes.len() - 1
                }
            },
        };
        groupes[i].paies.push(p);
    }
    let mut out = Vec::new();
    for g in groupes {
        let mut lu = Map::new();
        let lignes: Vec<&Value> = g.contrat.iter().chain(g.paies.iter()).collect();
        let le = lignes.iter().map(|x| t(x, "importe_le")).filter(|x| !x.is_empty()).min().unwrap_or_default();
        lu.insert("le".into(), json!(le));
        if let Some(c) = &g.contrat {
            lu.insert("contrat".into(), Value::Object(sans(c, &["salarie", "debut", "fin", "contrat"])));
        }
        let mut par_mois = Map::new();
        for p in &g.paies {
            par_mois.insert(t(p, "mois"), Value::Object(sans(p, &["salarie", "mois"])));
        }
        lu.insert("paies".into(), Value::Object(par_mois));
        let mut e = Map::new();
        e.insert("salarie".into(), json!(g.salarie));
        e.insert("debut".into(), json!(g.debut));
        e.insert("fin".into(), json!(g.fin));
        let declare = g.paies.iter().filter(|p| !declaree_par_bulletin(p)).map(|p| t(p, "contrat")).find(|n| !n.is_empty());
        let imprime = g.contrat.iter().map(|c| t(c, "numero_contrat")).find(|n| !n.is_empty());
        if let Some(n) = declare.or(imprime) {
            e.insert("numero".into(), json!(n));
        }
        e.insert(engagement::importe::IMPORTE.into(), Value::Object(lu));
        out.push(e);
    }
    out.sort_by(|a, b| {
        let k = |e: &Map<String, Value>| (e["debut"].as_str().unwrap_or("").to_string(), e["fin"].as_str().unwrap_or("").to_string(), e["salarie"].as_str().unwrap_or("").to_string());
        k(a).cmp(&k(b))
    });
    out
}

// ------------------------------------------------------------------ l'ecriture

/// Ce qu'une ecriture a fait.
struct Ecrit {
    fichiers: Vec<Value>,
    contrats_aem: usize,
    contrats_bulletins: usize,
    paies: usize,
    restent_a_creer: usize,
    /// Les fiches existantes completees.
    completees: usize,
    avertissements: Vec<String>,
}

impl Ecrit {
    fn rien(&self) -> bool {
        self.restent_a_creer == 0 && self.completees == 0 && self.contrats_aem == 0 && self.contrats_bulletins == 0 && self.paies == 0
    }
}

/// Ecrit ce que l'apercu `imp` annonce : les fiches (et leur place dans `reponse`), puis les engagements importes
/// (`engagements_importes` : les contrats des AEM et des bulletins, les paies versees).
fn ecrire(ctx: &Contexte, imp: &crate::routes::import::Apercu, reponse: &mut Value, note: &Value) -> R<Ecrit> {
    let aujourdhui = ctx.date;
    // Les fiches, comme celles de l'ecran (`forcer` : une fiche importee avec des anomalies est ecrite quand meme).
    let mut ecrits: BTreeSet<String> = BTreeSet::new();
    let mut restent_a_creer = 0usize;
    for a in &imp.a_creer {
        let e = crate::routes::fiches::ecrire_fiche(ctx, a.nom, None, &Value::Object(a.donnees.clone()), crate::routes::fiches::Forcer::Tout, &Value::Null, note)?;
        let emplacement: &mut Value = match (a.nom, a.rang) {
            ("entreprise", _) => &mut reponse["entreprise"],
            ("salarie", Some(i)) => &mut reponse["salaries"][i],
            (_, Some(i)) => &mut reponse["spectacles"][i],
            _ => {
                restent_a_creer += 1;
                continue;
            }
        };
        emplacement["cle"] = e.cle.map_or(Value::Null, Value::String);
        emplacement["etat"] = json!("creee");
        let mut anomalies = e.anomalies;
        crate::routes::fiches::avec_libelles(a.nom, &mut anomalies)?;
        emplacement["anomalies"] = sans_prives(&Value::Array(anomalies));
        let fichier = match a.nom {
            "entreprise" => "entreprise.json",
            "salarie" => "salaries.json",
            _ => "spectacles.json",
        };
        ecrits.insert(ctx.racine.join("config").join(fichier).display().to_string());
    }
    // Les fiches qui existent : ce qui y est vide ou « A COMPLETER » (`import::completer`), rien d'autre.
    let mut completees = 0usize;
    for a in &imp.a_completer {
        let faits = crate::routes::fiches::completer_fiche(ctx, a.nom, a.cle.as_deref(), &a.champs)?;
        if faits.is_empty() {
            continue;
        }
        let liste = match a.nom {
            "entreprise" => None,
            "salarie" => Some("salaries"),
            _ => Some("spectacles"),
        };
        let emplacement = match liste {
            None => Some(&mut reponse["entreprise"]),
            Some(l) => reponse[l].as_array_mut().and_then(|v| v.iter_mut().find(|e| e.get("cle").and_then(Value::as_str) == a.cle.as_deref())),
        };
        if let Some(e) = emplacement.filter(|e| e.is_object()) {
            e["etat"] = json!("completee");
            if let Some(Value::Object(c)) = e.get_mut("completer") {
                c.retain(|k, _| faits.contains_key(k));
            }
        }
        let fichier = match a.nom {
            "entreprise" => "entreprise.json",
            "salarie" => "salaries.json",
            _ => "spectacles.json",
        };
        ecrits.insert(ctx.racine.join("config").join(fichier).display().to_string());
        completees += 1;
    }
    let mut fichiers: Vec<Value> = ecrits.into_iter().map(Value::String).collect();
    let mut avertissements: Vec<String> = Vec::new();

    // Les engagements importes, avec les fiches telles qu'elles sont MAINTENANT.
    let frais = Contexte::nouveau(&ctx.racine, None).map_err(Echec::from)?;
    let mut par_nir: HashMap<String, String> = HashMap::new();
    for (cle, s) in &frais.salaries {
        if let Some(n) = s.get("nir").filter(|v| vrai(v)) {
            par_nir.insert(valeur_en_texte(n), cle.clone());
        }
    }
    // CE QUE L'IMPORT A DEJA ECRIT, relu de ses engagements importes : les contrats attestes, les paies versees.
    let importes_avant = engagement::importe::tous(&ctx.racine).map_err(Echec::from)?;
    let mut engagements: Vec<Value> = engagement::importe::contrats_lus(&ctx.racine).map_err(Echec::from)?;
    let mut deja: HashSet<CleEngagement> = HashSet::new();
    for e in &engagements {
        if let Some(k) = cle_engagement(e)? {
            deja.insert(k);
        }
    }
    let le = json!(aujourdhui.iso());
    let mut contrats_aem = 0usize;
    for c in &imp.contrats {
        let Some(cle) = par_nir.get(&c.nir) else { continue };
        let k: CleEngagement = (cle.clone(), c.debut.iso(), c.fin.iso(), c.numero_objet.clone(), c.heures, c.cachets, c.jours, c.brut.clone());
        if deja.contains(&k) {
            continue;
        }
        deja.insert(k);
        engagements.push(json!({
            "salarie": cle, "debut": c.debut.iso(), "fin": c.fin.iso(), "numero_objet": c.numero_objet,
            "heures": c.heures as i64, "cachets": c.cachets as i64, "jours": c.jours as i64, "brut": c.brut,
            "atteste_par": "AEM", "numero_attestation": c.numero_attestation,
            "atteste_le": c.date_attestation, "fichier": c.fichier, "importe_le": le,
        }));
        contrats_aem += 1;
    }
    // Les contrats que les bulletins attestent, rattaches par NIR a la fiche qui existe maintenant.
    let mut contrats_bulletins = 0usize;
    for (nir, ligne) in &imp.engagements_bulletins {
        let Some(cle) = par_nir.get(nir) else { continue };
        let mut e = ligne.clone();
        e["salarie"] = json!(cle);
        let Some(k) = cle_engagement(&e)? else { continue };
        if !deja.insert(k) {
            continue;
        }
        e["importe_le"] = le.clone();
        engagements.push(e);
        contrats_bulletins += 1;
    }

    // LES PAIES : on remplace ligne a ligne par la cle (mois, salarie, contrat). Une ligne de DSN qui arrive sur une ligne
    // completee par un bulletin garde ce que le bulletin a ecrit ; une paie de bulletin seul s'efface devant la paie
    // declaree qu'elle complete (`rapprocher`).
    let mut par_cle: BTreeMap<(String, String, String), Value> = BTreeMap::new();
    for x in engagement::importe::paies_lues(&ctx.racine).map_err(Echec::from)? {
        par_cle.insert(cle_paie(&x)?, x);
    }
    let avant = par_cle.clone();
    for x in &imp.paies {
        let Some(cle) = x.get("nir").and_then(Value::as_str).and_then(|n| par_nir.get(n)) else { continue };
        let mut x = x.clone();
        x["salarie"] = json!(cle);
        x["importe_le"] = le.clone();
        let k = cle_paie(&x)?;
        if let Some(a) = par_cle.get(&k).filter(|a| porte_un_bulletin(a) && !porte_un_bulletin(&x)) {
            completer_par(&mut x, &a.clone());
        }
        par_cle.insert(k, x);
    }
    let ambigues = rapprocher(&mut par_cle);
    if ambigues > 0 {
        avertissements.push(format!(
            "{} paie(s) écrite(s) d'un bulletin seul ont les dates de plusieurs paies déclarées en DSN : aucune n'est rapprochée, les engagements importés restent côte à côte, à regarder.",
            ambigues
        ));
    }
    let neuves = par_cle.iter().filter(|(k, v)| avant.get(*k).map(sans_dates_d_import) != Some(sans_dates_d_import(v))).count()
        + avant.keys().filter(|k| !par_cle.contains_key(*k)).count();
    if contrats_aem + contrats_bulletins + neuves > 0 || importes_avant.is_empty() {
        let importes = engagements_importes(engagements, par_cle.into_values().collect());
        for p in engagement::importe::ecrire_tous(&ctx.racine, importes).map_err(Echec::from)? {
            fichiers.push(json!(p.display().to_string()));
        }
    }
    Ok(Ecrit { fichiers, contrats_aem, contrats_bulletins, paies: neuves, restent_a_creer, completees, avertissements })
}

// ------------------------------------------------------------------ la route

/// `importer` : `{"dossier": ...}`, la boite aux lettres `import/` (vide ou absent : celle de la compagnie).
pub(crate) fn importer(ctx: &Contexte, charge: &Value) -> R<Value> {
    importer_dossier(ctx, charge)
}

/// LES MOIS IMPORTES SONT DECLARES S'ILS L'ONT ETE : les depots DSN que les retours deja releves attestent
/// entrent au journal avec l'historique, pour qu'un mois tenu avant ne se dise pas « en retard »
/// (`reprise_journal::reprendre_les_depots`). Rend le nombre de depots repris.
fn depots_attestes_au_journal(ctx: &Contexte, charge: &Value) -> R<usize> {
    let qui = crate::routes::qui_de(charge);
    crate::compagnie::incidents::noter_ceux_des_retours(&ctx.racine, envois::reprise_journal::reprendre_les_depots(ctx, &qui, &crate::routes::maintenant()))
}


/// LES ECARTS DE CALCUL PARTENT A CHAQUE IMPORT (`import::ecarts`), lus avec les fiches telles que l'import les a
/// ecrites ; rien ne s'en dit a l'ecran.
fn rapporter_les_ecarts(ctx: &Contexte) {
    if let Ok(frais) = Contexte::nouveau(&ctx.racine, Some(ctx.date)) {
        crate::import::ecarts::rapporter(&frais);
    }
}
fn avertissement_differents(differents: &[String], ou: &str) -> String {
    format!(
        "{} fichier(s) de {} portent le nom d'un fichier déjà copié avec un autre contenu : la copie est gardée telle quelle, à comparer à la main ({}).",
        differents.len(),
        ou,
        differents.iter().take(5).cloned().collect::<Vec<_>>().join(", ")
    )
}

/// Ou l'import du dossier copie ce qu'il reprend.
fn dossier_bulletins(ctx: &Contexte) -> PathBuf {
    ctx.racine.join("historique").join("bulletins")
}

/// La copie de ce que le dossier lu reprend, dans `historique/bulletins/`, sous le nom de sa copie. RIEN NE S'ECRASE :
/// un fichier deja copie et identique est laisse ; un autre contenu sous le meme nom est laisse aussi, et SIGNALE (il
/// n'est pas relu). Rend le compte, et les noms a relire.
fn copier_repris(ctx: &Contexte, lu: &crate::import::bulletins::DossierLu) -> R<(Value, Vec<String>)> {
    let dossier = dossier_bulletins(ctx);
    let (mut copies, mut identiques) = (0usize, 0usize);
    let (mut differents, mut noms): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    for (i, nom) in &lu.copies {
        let source = &lu.tri.fichiers[*i].disque;
        let cible = dossier.join(nom);
        if cible.exists() {
            let a = std::fs::read(source).map_err(|e| en_os(&e, source))?;
            let b = std::fs::read(&cible).map_err(|e| en_os(&e, &cible))?;
            if a == b {
                identiques += 1;
                noms.push(nom.clone());
            } else {
                differents.push(nom.clone());
            }
            continue;
        }
        std::fs::create_dir_all(&dossier).map_err(|e| en_os(&e, &dossier))?;
        std::fs::copy(source, &cible).map_err(|e| en_os(&e, source))?;
        if let Ok(m) = std::fs::metadata(source).and_then(|m| m.modified()) {
            if let Ok(f) = std::fs::OpenOptions::new().write(true).open(&cible) {
                let _ = f.set_modified(m);
            }
        }
        copies += 1;
        noms.push(nom.clone());
    }
    Ok((json!({"copies": copies, "identiques": identiques, "differents": differents, "dossier": dossier.display().to_string()}), noms))
}

/// L'avertissement de l'apercu qui compte les fichiers non repris du dossier.
fn dit_les_non_repris(a: &Value) -> bool {
    a.as_str().is_some_and(|t| t.ends_with("dit pourquoi, fichier par fichier."))
}

/// `importer` avec `{"dossier": ...}` : la copie de ce qui est repris, l'apercu relu dans la copie, l'ecriture.
fn importer_dossier(ctx: &Contexte, charge: &Value) -> R<Value> {
    let aujourdhui = ctx.date;
    let dossier = crate::routes::import::dossier_import(ctx, charge)?;
    // 1. Le dossier tel qu'il est : son compte rendu (pour l'ecran), et la copie de ce qu'il reprend.
    let (tri, non_repris, copie, noms) = crate::import::bulletins::lire_dossier(ctx, &dossier, |lu| {
        let (copie, noms) = copier_repris(ctx, &lu)?;
        let tri = lu.ap.reponse.get("tri").cloned().unwrap_or(Value::Null);
        let non_repris: Vec<Value> =
            lu.ap.reponse.get("avertissements").and_then(Value::as_array).into_iter().flatten().filter(|a| dit_les_non_repris(a)).cloned().collect();
        Ok((tri, non_repris, copie, noms))
    })?;

    // 2. D'ABORD LA COPIE, ENSUITE LA LECTURE : l'apercu relu dans les copies de ce dossier (et d'elles seules), prises
    // dans `historique/bulletins/` ; c'est lui qui s'ecrit.
    let copies = dossier_bulletins(ctx);
    let relu = PathBuf::from(format!("{}-copie", crate::routes::import::dossier_de_travail().display()));
    let relire = || -> R<crate::routes::import::Apercu> {
        std::fs::create_dir_all(&relu).map_err(|e| en_os(&e, &relu))?;
        for nom in &noms {
            std::fs::copy(copies.join(nom), relu.join(nom)).map_err(|e| en_os(&e, &copies.join(nom)))?;
        }
        let frais_avant = Contexte::nouveau(&ctx.racine, None).map_err(Echec::from)?;
        crate::import::bulletins::lire_dossier(&frais_avant, &relu, |lu| Ok(lu.ap))
    };
    let rendu = relire();
    let _ = std::fs::remove_dir_all(&relu);
    let mut imp = rendu?;

    let mut reponse = Value::Object(std::mem::take(&mut imp.reponse));
    // Le compte rendu est celui du dossier lu : ses chemins, ses fichiers non repris (la relecture ne les voit pas).
    let mut avertissements: Vec<Value> =
        reponse["avertissements"].as_array().into_iter().flatten().filter(|a| !dit_les_non_repris(a)).cloned().collect();
    avertissements.extend(non_repris);
    let differents: Vec<String> = copie["differents"].as_array().into_iter().flatten().map(valeur_en_texte).collect();
    if !differents.is_empty() {
        avertissements.push(json!(avertissement_differents(&differents, "ce dossier")));
    }
    if let Some(alerte) = alerte_apres_import(ctx)? {
        avertissements.push(json!(alerte));
    }
    reponse["tri"] = tri;
    reponse["source"] = json!(dossier.display().to_string());
    let note = json!(format!("Importe du dossier import/ le {}", jj_mm_aaaa(&aujourdhui)));
    let e = ecrire(ctx, &imp, &mut reponse, &note)?;
    avertissements.extend(e.avertissements.iter().map(|a| json!(a)));
    reponse["avertissements"] = Value::Array(avertissements);
    reponse["contrats_nouveaux"] = json!(e.contrats_aem);
    if reponse.get("bulletins").is_some_and(Value::is_object) {
        reponse["bulletins"]["contrats_nouveaux"] = json!(e.contrats_bulletins);
    }
    reponse["rien_a_faire"] = json!(e.rien());
    reponse["ecrit"] = json!(true);
    reponse["fichiers"] = Value::Array(e.fichiers);
    reponse["copie"] = copie;
    reponse["depots_repris"] = json!(depots_attestes_au_journal(ctx, charge)?);
    rapporter_les_ecarts(ctx);
    Ok(reponse)
}

#[cfg(test)]
mod tests_mois {
    /// Le mois « 00 » d'une alerte CASC-SVP n'est aucun mois et se refuse (ce n'est pas janvier).
    #[test]
    fn le_mois_00_de_l_alerte_casc_se_refuse() {
        let refus = super::libelle_casc("2026-00").unwrap_err();
        assert!(refus.message.contains(&socle::date::mois_impossible(0)), "{}", refus.message);
        assert_eq!(super::libelle_casc("2025-03").unwrap(), "mars 2025");
    }
}
