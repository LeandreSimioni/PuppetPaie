//! Le journal des incidents : ce que le logiciel n'a pas su faire
//! (`moteur/incidents.py`).
//!
//! `journal.py` note ce qu'un fichier produit est DEVENU ; celui-ci note les
//! moments ou le logiciel s'est arrete, a refuse, ou s'est fait rejeter. Il
//! porte le diagnostic a distance : tout s'ecrit ICI, toujours, et une ligne
//! porte son etat d'envoi (`en-attente`, `depose`, `sans-objet`), c'est tout
//! ce que l'envoi ajoute. `prive/journal/incidents.json` : une liste, on
//! n'ajoute qu'a la fin.
//!
//! Quatre natures (`logiciel`, `refus`, `dpae`, `dsn`) ; les deux premieres se
//! regroupent par empreinte. AUCUNE VALEUR PERSONNELLE : un message nomme le
//! champ, jamais la valeur, et `noter` CAVIARDE tout ce qui a la forme d'un
//! NIR, d'un IBAN, d'un BIC ou du jeton de session -- c'est l'alarme, pas le
//! garde-fou.
//!
//! ET IL NE LEVE JAMAIS : `noter`, `depuis_exception`, `depuis_navigateur`,
//! `marquer_envoye`, `oublier` et `envoyer` ne remontent aucun refus a
//! l'appelant. Ce journal est appele depuis le gestionnaire de plantage : s'il
//! levait, il remplacerait l'erreur qu'on cherche a comprendre par la sienne.
//!
//! Format sur le disque identique au Python (`util.ecrire_json`, indent 2) :
//! les deux cohabitent tant que du Python note encore des incidents
//! (`operations_declarations`, `operations_paies`, `web.py`).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde_json::{json, Map, Value};

use crate::editeur::{Client, ErreurFilet};
use crate::ecriture::ecrire_json;
use crate::etats::{eq_py, erreur, get, nom_type, py_str, R};
use crate::fichiers::{dossier_du_code, vrai};
use regex::Regex;
use crate::verrou::Verrou;

pub const LOGICIEL: &str = "logiciel";
pub const REFUS: &str = "refus";
pub const DPAE: &str = "dpae";
pub const DSN: &str = "dsn";

pub const EN_ATTENTE: &str = "en-attente";
pub const DEPOSE: &str = "depose";
pub const SANS_OBJET: &str = "sans-objet";

/// `NATURES` : (nature, se regroupe).
const NATURES: [(&str, bool); 4] = [(LOGICIEL, true), (REFUS, true), (DPAE, false), (DSN, false)];

/// Les natures que le journal connait, dans l'ordre de `NATURES`.
pub fn natures() -> Vec<&'static str> {
    NATURES.iter().map(|(n, _)| *n).collect()
}

fn nature_connue(n: &str) -> bool {
    NATURES.iter().any(|(x, _)| *x == n)
}

/// `NATURES.get(quoi, {}).get("regroupe", False)`.
fn se_regroupe(quoi: &Value) -> bool {
    quoi.as_str().is_some_and(|q| NATURES.iter().any(|(n, r)| *n == q && *r))
}

/// Au-dela, on tronque : une trace d'exception peut faire des pages.
const TRACE_MAX: usize = 4000;
const TEXTE_MAX: usize = 600;
const JS_PILE_MAX: usize = 2000;

/// `incidents.LOT` : la borne du serveur ; `MAX_PAR_PASSAGE` : au-dela, le
/// passage suivant reprend.
const LOT: usize = 20;
const MAX_PAR_PASSAGE: usize = 200;

const POSTE_FICHIER: &str = "diagnostic.json";
const FICHIER_EDITEUR: &str = "EDITEUR.json";

// ------------------------------------------------------------------- les motifs

fn motifs_a_caviarder() -> &'static [(Regex, &'static str)] {
    static M: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    M.get_or_init(|| {
        vec![
            // Avec ou sans les espaces de la carte Vitale (1 84 12 76 451 089 46).
            (motif(r"\b[12] ?[0-9]{2} ?(?:0[1-9]|1[0-2]) ?[0-9]{2} ?[0-9]{3} ?[0-9]{3}(?: ?[0-9]{1,2})?\b"), "NIR"),
            (motif(r"\bFR\d{2} ?(?:\d{4} ?){5}\d{3}\b"), "IBAN"),
            (motif(r"\b[A-Z]{4}FR[A-Z0-9]{2}[0-9][A-Z0-9]{0,2}\b"), "BIC"),
            // Le jeton de session de l'interface : le secret du serveur local.
            (motif(r#"(?i)(jeton=)[^&\s\"'\\]+"#), "JETON"),
        ]
    })
}

/// Les motifs sont ecrits ici, une fois : un motif faux est un defaut du programme.
fn motif(source: &str) -> Regex {
    Regex::new(source).expect("motif du journal des incidents")
}

/// Le chemin jusqu'au dossier du logiciel (`_CHEMIN` du Python). `regex` ne
/// connait pas `(?=...)` : le dossier qui suit est capture (groupe 2), et la
/// recherche suivante repart de lui, comme apres une anticipation.
fn motif_chemin() -> &'static Regex {
    static M: OnceLock<Regex> = OnceLock::new();
    M.get_or_init(|| {
        motif(r#"(?:\b[A-Za-z]:)?[\\/][^"'\n<>|;]*?[\\/]((?:logiciel|atelier|moteur|prive|prive-dev|demo|Lib|lib|site-packages)[\\/])"#)
    })
}

fn motif_adresse() -> &'static Regex {
    static M: OnceLock<Regex> = OnceLock::new();
    M.get_or_init(|| motif(r#"[^\s<>()\[\]"',;:]+@[^\s<>()\[\]"',;:]+\.[A-Za-z]{2,}"#))
}

fn motif_maison() -> &'static Regex {
    static M: OnceLock<Regex> = OnceLock::new();
    M.get_or_init(|| motif(r#"(?i)([A-Z]:\\+Users\\+|/home/|/Users/)[^\\/"'\s]+"#))
}

/// `_CHEMIN.sub("…/", s)` : chaque chemin jusqu'au dossier du logiciel devient
/// `…/`, le dossier lui-meme reste et la recherche repart de lui.
fn sans_chemin(s: &str) -> String {
    let re = motif_chemin();
    let (mut out, mut dernier, mut pos) = (String::new(), 0, 0);
    while let Some(c) = re.captures_at(s, pos) {
        let (debut, suite) = (c.get(0).map_or(0, |m| m.start()), c.get(1).map_or(0, |m| m.start()));
        out.push_str(&s[dernier..debut]);
        out.push_str("…/");
        dernier = suite;
        pos = suite;
    }
    out.push_str(&s[dernier..]);
    out
}

// ---------------------------------------------------------------------- le chemin

/// `incidents.chemin` : ou vit le journal. `PUPPETPAIE_INCIDENTS` le deplace
/// (les essais).
pub fn chemin(racine: &Path) -> PathBuf {
    match std::env::var("PUPPETPAIE_INCIDENTS").ok().filter(|v| !v.is_empty()) {
        Some(autre) => PathBuf::from(autre).join("incidents.json"),
        None => racine.join("journal").join("incidents.json"),
    }
}

/// `str.strip()` de Python.
fn py_strip(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c))
}

/// `s[:n]` sur des points de code.
fn tete(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// L'heure de Paris, `datetime.now().isoformat(timespec="seconds")`.
pub fn maintenant() -> String {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64());
    crate::paris::iso_secondes(t)
}

// -------------------------------------------------------------------- caviardage

/// `incidents.caviarder` : le texte nettoye, et ce qui a ete trouve. Le
/// marqueur dit CE QUI a ete retire (`[NIR]`), pas ce que c'etait.
pub fn caviarder(texte: &str) -> (String, Vec<&'static str>) {
    let mut s = texte.to_string();
    let mut trouve = Vec::new();
    for (motif, quoi) in motifs_a_caviarder() {
        let n = motif.find_iter(&s).count();
        if n > 0 {
            s = motif.replace_all(&s, format!("[{}]", quoi).as_str()).into_owned();
        }
        for _ in 0..n {
            trouve.push(*quoi);
        }
    }
    (s, trouve)
}

/// `incidents._nettoyer` : caviarde en profondeur (chaines, listes,
/// dictionnaires) ; une chaine est tronquee a `limite` points de code.
pub fn nettoyer(v: &Value, trouve: &mut Vec<&'static str>, limite: usize) -> Value {
    match v {
        Value::String(s) => {
            let (propre, vus) = caviarder(s);
            trouve.extend(vus);
            Value::String(tete(&propre, limite))
        }
        Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), nettoyer(x, trouve, limite))).collect()),
        Value::Array(a) => Value::Array(a.iter().map(|x| nettoyer(x, trouve, limite)).collect()),
        autre => autre.clone(),
    }
}

/// `incidents.empreinte` : ce qui fait que deux incidents sont LE MEME -- la
/// nature, l'endroit et le genre, pas le message.
pub fn empreinte(nature: &str, ou: &str, quoi: &str) -> String {
    use sha2::{Digest, Sha256};
    let graine = format!("{}|{}|{}", nature, ou, quoi);
    let h = Sha256::digest(graine.as_bytes());
    h.iter().map(|o| format!("{:02x}", o)).collect::<String>()[..12].to_string()
}

// ------------------------------------------------------------------------ ecrire

/// La version du logiciel (`incidents._version`) : `VERSION`, a cote du code.
pub fn version() -> String {
    let p = dossier_du_code().parent().map(|d| d.join("VERSION")).unwrap_or_default();
    if !p.exists() {
        return "0.0.0".to_string();
    }
    match std::fs::read(&p).ok().and_then(|o| String::from_utf8(o).ok()) {
        Some(t) => py_strip(&t).to_string(),
        None => "?".to_string(),
    }
}

/// Ce que `noter` recoit.
#[derive(Default)]
pub struct Note<'a> {
    pub nature: &'a str,
    pub resume: &'a str,
    /// L'endroit : « calcul.py:614 », « depot-dsn », « fiche/nir ».
    pub ou: &'a str,
    /// Le genre : le type d'exception, le code de controle, le motif du refus.
    pub quoi: &'a str,
    pub detail: Value,
    pub trace: Option<&'a str>,
    /// `en-attente` par defaut.
    pub envoi: Option<&'a str>,
    pub quand: Option<&'a str>,
}

/// `_lignes` : la liste du fichier ; un fichier absent, illisible ou qui n'est
/// pas une liste n'en dit aucune.
fn lignes(p: &Path) -> Vec<Value> {
    if !p.exists() {
        return Vec::new();
    }
    match std::fs::read(p).ok().and_then(|o| String::from_utf8(o).ok()).and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
        Some(Value::Array(a)) => a,
        _ => Vec::new(),
    }
}

/// `noter` : ajoute une ligne. Ne leve jamais ; rend la ligne, ou `None` si
/// rien n'a pu etre ecrit.
pub fn noter(racine: &Path, n: &Note) -> Option<Value> {
    let nature = if nature_connue(n.nature) { n.nature } else { LOGICIEL };
    if (nature == DPAE || nature == DSN) && crate::routes_accueil::simulation_active(racine) {
        // LES FAUX ENVOIS NE SONT PAS DES INCIDENTS : une fictive joue ses depots.
        return None;
    }
    let mut trouve = Vec::new();
    let texte = |s: &str, limite: usize, trouve: &mut Vec<&'static str>| nettoyer(&Value::String(s.to_string()), trouve, limite);
    let ou = texte(n.ou, 200, &mut trouve);
    let genre = texte(n.quoi, 200, &mut trouve);
    let resume = texte(n.resume, TEXTE_MAX, &mut trouve);
    let detail = if vrai(&n.detail) { nettoyer(&n.detail, &mut trouve, TEXTE_MAX) } else { json!({}) };
    let trace = match n.trace {
        Some(t) if !t.is_empty() => texte(t, TRACE_MAX, &mut trouve),
        _ => Value::Null,
    };
    trouve.sort_unstable();
    trouve.dedup();
    let ligne = json!({
        "quoi": nature,
        "quand": n.quand.map_or_else(maintenant, str::to_string),
        "version": version(),
        "empreinte": empreinte(nature, n.ou, n.quoi),
        "ou": ou,
        "genre": genre,
        "resume": resume,
        "detail": detail,
        "trace": trace,
        "caviarde": trouve,
        "envoi": {"etat": n.envoi.unwrap_or(EN_ATTENTE), "quand": Value::Null},
    });
    let p = chemin(racine);
    let mut toutes = lignes(&p);
    toutes.push(ligne.clone());
    ecrire_json(&p, &Value::Array(toutes), 2).ok()?;
    Some(ligne)
}

/// Les classes que le Python tenait pour un refus (`nature_de`) : une
/// `ErreurOperation` et ses filles, `KeyError`, `ValueError`,
/// `FileNotFoundError` et leurs filles.
// `ErreurCoffre` et `ErreurGoogle` y sont, et c'est voulu (decision de Leandre, 08/10/2026) : le Python les classait en defaut du logiciel
// (`Exception` simples), mais un coffre ferme ou un refus de Google est une situation d'usage. Un defaut du Python ne se reproduit pas.
const REFUS_PAR_CLASSE: [&str; 20] = [
    "ErreurOperation", "SaisieIncomplete", "PreuveInsuffisante", "CalculRefuse", "FabriqueRefus", "FicheRefusee",
    "DsnDejaDeposee", "MoisPasPret", "RepetitionManquante", "DpaeDejaDeclaree", "VersionExigee", "CompteursVerrouilles",
    "KeyError", "ValueError", "FileNotFoundError", "UnicodeDecodeError", "JSONDecodeError", "UnicodeEncodeError",
    "ErreurCoffre", "ErreurGoogle",
];

/// `nature_de` d'apres le NOM de la classe de l'exception.
pub fn nature_de(classe: &str) -> &'static str {
    if REFUS_PAR_CLASSE.contains(&classe) {
        REFUS
    } else {
        LOGICIEL
    }
}

/// `depuis_exception` : note une exception (son type, l'endroit, sa trace).
/// `nature` absente : elle se deduit du type. Les copies de developpement ne
/// notent rien (« rien ne part d'ici », le moteur tient sa porte).
pub fn depuis_exception(racine: &Path, classe: &str, message: &str, nature: Option<&str>, ou: &str, detail: Value, trace: Option<&str>) -> Option<Value> {
    if classe == "CopieDeDeveloppement" || classe == "EditionDemo" {
        return None;
    }
    let nature = nature.unwrap_or_else(|| nature_de(classe));
    noter(
        racine,
        &Note {
            nature,
            ou: if ou.is_empty() { "?" } else { ou },
            quoi: classe,
            resume: &format!("{}: {}", classe, message),
            detail,
            trace,
            ..Default::default()
        },
    )
}

/// `str(x)` (`None` n'est pas ici un texte : voir l'appelant).
fn texte_de(v: &Value) -> String {
    py_str(v)
}

/// `_fichier_court` : « http://hote:7331/interface.js?v=3 » -> « interface.js ».
/// L'hote et le port varient ; la requete porte le jeton, elle ne se garde jamais.
fn fichier_court(source: &Value) -> String {
    let s = if vrai(source) { texte_de(source) } else { String::new() };
    let s = s.split('?').next().unwrap_or("");
    let s = s.split('#').next().unwrap_or("");
    let s = s.trim_end_matches('/');
    let s = s.rsplit('/').next().unwrap_or("");
    tete(s, 80)
}

/// `int(x)` de Python, ecrit en chiffres : l'entier, le flottant tronque (sans
/// limite de taille), le booleen, le texte (espaces et signe permis, `_` entre
/// deux chiffres) ; `None` quand Python leve `TypeError` ou `ValueError`.
fn entier_py(v: &Value) -> Option<String> {
    fn sans_zeros(neg: bool, chiffres: &str) -> String {
        let t = chiffres.trim_start_matches('0');
        if t.is_empty() {
            "0".to_string()
        } else if neg {
            format!("-{}", t)
        } else {
            t.to_string()
        }
    }
    match v {
        Value::Bool(b) => Some(if *b { "1" } else { "0" }.to_string()),
        Value::Number(n) => match n.as_i64() {
            Some(i) => Some(i.to_string()),
            None => {
                let x = n.as_f64()?;
                if !x.is_finite() {
                    return None;
                }
                let t = format!("{:.0}", x.trunc());
                let (neg, chiffres) = match t.strip_prefix('-') {
                    Some(r) => (true, r.to_string()),
                    None => (false, t),
                };
                Some(sans_zeros(neg, &chiffres))
            }
        },
        Value::String(t) => {
            let u = py_strip(t);
            let (neg, reste) = match u.strip_prefix('-') {
                Some(r) => (true, r),
                None => (false, u.strip_prefix('+').unwrap_or(u)),
            };
            let c: Vec<char> = reste.chars().collect();
            let chiffre = |x: char| crate::routes_ecrites::chiffre_py(x);
            let bien_forme = !c.is_empty()
                && chiffre(c[0]).is_some()
                && chiffre(c[c.len() - 1]).is_some()
                && c.iter().all(|x| chiffre(*x).is_some() || *x == '_')
                && !reste.contains("__");
            if !bien_forme {
                return None;
            }
            let ascii: String = c.iter().filter_map(|x| chiffre(*x)).map(|d| char::from(b'0' + d as u8)).collect();
            Some(sans_zeros(neg, &ascii))
        }
        _ => None,
    }
}

/// `depuis_navigateur` : note ce qui a casse dans l'interface. La page n'envoie
/// que ce qu'elle a vu ; la nature, l'endroit qui regroupe et le caviardage se
/// decident ICI. La nature est TOUJOURS `logiciel`.
pub fn depuis_navigateur(racine: &Path, rapport: &Value) -> Option<Value> {
    let champ = |c: &str| rapport.get(c).unwrap_or(&Value::Null);
    let genre = {
        let g = champ("genre");
        tete(&if vrai(g) { texte_de(g) } else { "Erreur".to_string() }, 80)
    };
    let fichier = fichier_court(champ("fichier"));
    let ligne = entier_py(champ("ligne")).filter(|l| l != "0").unwrap_or_else(|| "?".to_string());
    let ou = format!("{}:{}", if fichier.is_empty() { "interface".to_string() } else { fichier }, ligne);
    let mut detail = Map::new();
    detail.insert("face".into(), json!("interface"));
    let canal = champ("canal");
    detail.insert("canal".into(), json!(tete(&if vrai(canal) { texte_de(canal) } else { "erreur".to_string() }, 20)));
    let colonne = champ("colonne");
    if !colonne.is_null() && colonne.as_str() != Some("") {
        detail.insert("colonne".into(), json!(tete(&texte_de(colonne), 10)));
    }
    let dans_la_page = champ("ou_dans_la_page");
    if vrai(dans_la_page) {
        detail.insert("ou_dans_la_page".into(), json!(tete(&texte_de(dans_la_page), 120)));
    }
    let message = champ("message");
    let resume = format!("{}: {}", genre, if vrai(message) { texte_de(message) } else { String::new() });
    let pile = champ("pile");
    let trace = if vrai(pile) { Some(tete(&texte_de(pile), JS_PILE_MAX)) } else { None };
    noter(
        racine,
        &Note { nature: LOGICIEL, ou: &ou, quoi: &genre, resume: &resume, detail: Value::Object(detail), trace: trace.as_deref(), ..Default::default() },
    )
}

// ------------------------------------------------------------------------- lire

/// `lire` : les lignes, les plus recentes en dernier. Une ligne qui n'est pas
/// un objet leve `AttributeError`, comme `l.get`.
pub fn lire(racine: &Path, nature: Option<&str>, en_attente_seulement: bool) -> R<Vec<Value>> {
    let mut out = lignes(&chemin(racine));
    if let Some(n) = nature.filter(|n| !n.is_empty()) {
        let mut gardees = Vec::new();
        for l in out {
            if get(&l, "quoi")?.as_str() == Some(n) {
                gardees.push(l);
            }
        }
        out = gardees;
    }
    if en_attente_seulement {
        let mut gardees = Vec::new();
        for l in out {
            if etat_d_envoi(&l)? == Some(EN_ATTENTE.to_string()) {
                gardees.push(l);
            }
        }
        out = gardees;
    }
    Ok(out)
}

/// `(l.get("envoi") or {}).get("etat")`.
fn etat_d_envoi(l: &Value) -> R<Option<String>> {
    let envoi = get(l, "envoi")?;
    let envoi = if vrai(envoi) { envoi } else { return Ok(None) };
    Ok(get(envoi, "etat")?.as_str().map(str::to_string))
}

/// `regrouper` : les incidents regroupes par empreinte, le plus frequent en
/// tete. Les natures qui ne se regroupent pas rendent une entree par ligne.
pub fn regrouper(racine: &Path, nature: Option<&str>) -> R<Vec<Value>> {
    // (cle de regroupement, paquet) dans l'ordre d'insertion.
    let mut paquets: Vec<(Option<Value>, Map<String, Value>)> = Vec::new();
    for l in lire(racine, nature, false)? {
        let regroupe = se_regroupe(get(&l, "quoi")?);
        let cle = if regroupe { Some(get(&l, "empreinte")?.clone()) } else { None };
        let existant = match &cle {
            Some(c) => paquets.iter().position(|(k, _)| k.as_ref().is_some_and(|k| eq_py(k, c))),
            None => None,
        };
        let quand = get(&l, "quand")?.clone();
        match existant {
            None => {
                let mut p = Map::new();
                p.insert("empreinte".into(), get(&l, "empreinte")?.clone());
                p.insert("quoi".into(), get(&l, "quoi")?.clone());
                p.insert("ou".into(), get(&l, "ou")?.clone());
                p.insert("genre".into(), get(&l, "genre")?.clone());
                p.insert("resume".into(), get(&l, "resume")?.clone());
                p.insert("combien".into(), json!(1));
                p.insert("premiere".into(), quand.clone());
                p.insert("derniere".into(), quand);
                p.insert("versions".into(), json!([get(&l, "version")?.clone()]));
                let cav = get(&l, "caviarde")?;
                p.insert("caviarde".into(), Value::Array(if vrai(cav) { crate::etats::items(cav)?.into_iter().map(|c| c.into_owned()).collect() } else { Vec::new() }));
                paquets.push((cle, p));
            }
            Some(i) => {
                let p = &mut paquets[i].1;
                let n = p["combien"].as_i64().unwrap_or(0) + 1;
                p.insert("combien".into(), json!(n));
                p.insert("derniere".into(), quand);
                let version = get(&l, "version")?.clone();
                if let Some(Value::Array(vs)) = p.get_mut("versions") {
                    if !vs.iter().any(|v| eq_py(v, &version)) {
                        vs.push(version);
                    }
                }
                let cav = get(&l, "caviarde")?;
                if vrai(cav) {
                    for c in crate::etats::items(cav)? {
                        if let Some(Value::Array(cs)) = p.get_mut("caviarde") {
                            if !cs.iter().any(|x| eq_py(x, &c)) {
                                cs.push(c.into_owned());
                            }
                        }
                    }
                }
            }
        }
    }
    // `sorted(..., key=(combien, derniere or ""), reverse=True)` : stable.
    let cle_de = |p: &Map<String, Value>| (p["combien"].as_i64().unwrap_or(0), if vrai(&p["derniere"]) { texte_de(&p["derniere"]) } else { String::new() });
    let mut v: Vec<Map<String, Value>> = paquets.into_iter().map(|(_, p)| p).collect();
    v.sort_by(|a, b| cle_de(b).cmp(&cle_de(a)));
    Ok(v.into_iter().map(Value::Object).collect())
}

/// `marquer_envoye` : passe les lignes en attente qui portent l'une de ces
/// empreintes a `depose`. Rend le nombre touche ; ne leve jamais.
pub fn marquer_envoye(racine: &Path, empreintes: &[Value]) -> usize {
    let p = chemin(racine);
    let mut toutes = lignes(&p);
    let quand = maintenant();
    let mut n = 0;
    for l in toutes.iter_mut() {
        let porte = match l.get("empreinte") {
            Some(e) => empreintes.iter().any(|v| eq_py(v, e)),
            None => false,
        };
        if l.is_object() && porte && matches!(etat_d_envoi(l), Ok(Some(e)) if e == EN_ATTENTE) {
            l["envoi"] = json!({"etat": DEPOSE, "quand": quand});
            n += 1;
        } else if !l.is_object() {
            // `l.get` sur autre chose qu'un objet leve : l'appel entier s'arrete a 0.
            return 0;
        }
    }
    if n > 0 && ecrire_json(&p, &Value::Array(toutes), 2).is_err() {
        return 0;
    }
    n
}

/// `oublier` : vide le journal, en entier ou d'une nature. Rend le nombre retire.
pub fn oublier(racine: &Path, nature: Option<&str>) -> usize {
    let p = chemin(racine);
    let toutes = lignes(&p);
    let gardees: Vec<Value> = match nature.filter(|n| !n.is_empty()) {
        Some(n) => {
            let mut g = Vec::new();
            for l in &toutes {
                match l.get("quoi") {
                    Some(q) if q.as_str() == Some(n) => {}
                    _ if l.is_object() => g.push(l.clone()),
                    _ => return 0,
                }
            }
            g
        }
        None => Vec::new(),
    };
    let retires = toutes.len() - gardees.len();
    if ecrire_json(&p, &Value::Array(gardees), 2).is_err() {
        return 0;
    }
    retires
}

// -------------------------------------------------------- ce qui part chez l'editeur

/// `_pour_texte` : ni le chemin du dossier de l'utilisateur (qui porte son nom
/// de session), ni une adresse courriel.
fn pour_texte(v: &Value) -> Value {
    match v {
        Value::String(s) => {
            let s = sans_chemin(s);
            let s = motif_maison().replace_all(&s, "${1}[session]");
            let s = motif_adresse().replace_all(&s, "[adresse]");
            Value::String(s.into_owned())
        }
        Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), pour_texte(x))).collect()),
        Value::Array(a) => Value::Array(a.iter().map(pour_texte).collect()),
        autre => autre.clone(),
    }
}

/// `pour_l_editeur` : ce qui part d'une ligne -- moins son etat d'envoi --, plus
/// la machine qui l'envoie.
pub fn pour_l_editeur(ligne: &Value) -> R<Value> {
    let mut d = Map::new();
    for k in ["quoi", "quand", "version", "empreinte", "ou", "genre", "resume", "detail", "trace", "caviarde"] {
        d.insert(k.to_string(), pour_texte(get(ligne, k)?));
    }
    d.insert("machine".into(), json!(machine()));
    Ok(Value::Object(d))
}

/// `incidents.machine` : la machine, en une ligne -- « Linux Ubuntu 24.04 ·
/// x86_64 · moteur Rust · complete » : le systeme, l'architecture, le moteur et
/// l'edition. Rien qui designe la personne. Pas de virgule : le serveur les
/// regroupe par elle.
pub fn machine() -> String {
    static M: OnceLock<String> = OnceLock::new();
    M.get_or_init(|| {
        let morceaux = [systeme(), architecture(), "moteur Rust".to_string(), edition()];
        let m: Vec<String> = morceaux.iter().filter(|m| !m.is_empty()).map(|m| m.replace(',', " ")).collect();
        tete(&m.join(" · "), 120)
    })
    .clone()
}

fn architecture() -> String {
    let a = std::env::consts::ARCH;
    let nom = match (std::env::consts::OS, a) {
        ("windows", "x86_64") => "AMD64",
        ("windows", "aarch64") => "ARM64",
        ("macos", "aarch64") => "arm64",
        (_, a) => a,
    };
    nom.to_string()
}

#[cfg(unix)]
fn systeme() -> String {
    match std::env::consts::OS {
        "macos" => {
            let v = std::process::Command::new("sw_vers").arg("-productVersion").output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
            format!("macOS {}", v)
        }
        _ => {
            if let Some(nom) = std::fs::read_to_string("/etc/os-release").ok().and_then(|t| {
                t.lines().find_map(|l| l.strip_prefix("PRETTY_NAME=").map(|v| v.trim().trim_matches('"').to_string()))
            }) {
                return nom;
            }
            let release = std::fs::read_to_string("/proc/sys/kernel/osrelease").map(|t| t.trim().to_string()).unwrap_or_default();
            format!("Linux {}", release)
        }
    }
}

#[cfg(not(unix))]
fn systeme() -> String {
    let sortie = std::process::Command::new("cmd").args(["/C", "ver"]).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
    let version = sortie.split("Version ").nth(1).and_then(|r| r.split(']').next()).unwrap_or("").trim().to_string();
    let build: u32 = version.split('.').nth(2).and_then(|b| b.parse().ok()).unwrap_or(0);
    format!("Windows {} ({})", if build >= 22000 { "11" } else { "10" }, version)
}

/// `compagnies.edition()` : `demo` pour le paquet soude, sinon le mode retenu
/// sur le poste (`.edition.json` a cote de `logiciel/`), sinon le defaut du
/// paquet, sinon `complete`.
pub fn edition() -> String {
    if crate::editeur::paquet_soude() {
        return "demo".to_string();
    }
    let logiciel = dossier_du_code().parent().map(Path::to_path_buf).unwrap_or_default();
    let lu = |p: PathBuf| std::fs::read(p).ok().and_then(|o| String::from_utf8(o).ok()).and_then(|t| serde_json::from_str::<Value>(&t).ok());
    if let Some(Value::Object(d)) = logiciel.parent().and_then(|r| lu(r.join(".edition.json"))) {
        if let Some(e) = d.get("edition").and_then(Value::as_str).filter(|e| *e == "complete" || *e == "demo") {
            return e.to_string();
        }
    }
    match lu(logiciel.join("EDITION.json")) {
        Some(Value::Object(d)) if d.get("defaut").and_then(Value::as_str) == Some("demo") => "demo".to_string(),
        _ => "complete".to_string(),
    }
}

/// L'ENVOI A L'EDITEUR SE COUPE (Leandre, 08/10/2026 ; c'est aussi une condition de SignPath). `config/diagnostic.json`,
/// cle `envoi` : absente ou vraie, les incidents partent ; `false`, ils restent notes sur le poste et rien ne part.
pub fn envoi_actif(racine: &Path) -> bool {
    let p = racine.join("config").join(POSTE_FICHIER);
    match crate::routes_fiches::lire_tolerant(&p) {
        Ok(Some(v)) => v.get("envoi") != Some(&Value::Bool(false)),
        _ => true,
    }
}

/// Active ou coupe l'envoi a l'editeur ; le numero du poste reste.
pub fn regler_envoi(racine: &Path, actif: bool) -> R<()> {
    let p = racine.join("config").join(POSTE_FICHIER);
    let mut d = match crate::routes_fiches::lire_tolerant(&p)? {
        Some(Value::Object(o)) => o,
        _ => Map::new(),
    };
    d.insert("envoi".into(), Value::Bool(actif));
    ecrire_json(&p, &Value::Object(d), 2).map_err(|m| erreur("OSError", m))
}

/// Un numero de poste tel que le serveur les donne : 32 hexadecimaux.
fn numero_de_poste(v: &Value) -> Option<String> {
    let s = v.as_str()?;
    (s.len() == 32 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))).then(|| s.to_string())
}

/// Une cle de poste telle que le serveur les donne : 64 hexadecimaux.
fn cle_de_poste(v: &Value) -> Option<String> {
    let s = v.as_str()?;
    (s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))).then(|| s.to_string())
}

/// `incidents.poste` : le numero de ce poste chez l'editeur, tel que le serveur
/// l'a donne (`config/diagnostic.json`, dans la compagnie dont vient la
/// configuration) ; vide tant qu'aucune identite n'a ete demandee. Ne demande
/// rien : c'est `client_du_poste` qui le fait, a l'envoi.
pub fn poste(racine: &Path) -> R<String> {
    let ou = match configuration(racine)? {
        Some((_, ou)) => ou,
        None => racine.to_path_buf(),
    };
    let p = ou.join("config").join(POSTE_FICHIER);
    Ok(crate::routes_fiches::lire_tolerant(&p)?.as_ref().and_then(|d| d.get("poste")).and_then(numero_de_poste).unwrap_or_default())
}

/// `client_du_poste` : le client des actions de l'editeur chez `serveur`, avec
/// l'identite de ce poste. Elle se lit dans `config/diagnostic.json` de `ou`
/// (`poste`, `cle`, `serveur`) ; absente, incomplete, ou donnee par un autre
/// serveur, elle se DEMANDE au serveur (`POST /poste`) et s'ecrit, une fois pour
/// toutes. Le numero que les versions d'avant tiraient elles-memes n'est connu
/// de personne : il se remplace. Une panne du serveur (reseau, 429) se rend
/// telle quelle : l'appelant laisse tout en attente.
pub fn client_du_poste(ou: &Path, serveur: &str) -> R<Result<Client, ErreurFilet>> {
    let p = ou.join("config").join(POSTE_FICHIER);
    let mut d = match crate::routes_fiches::lire_tolerant(&p)? {
        Some(Value::Object(o)) => o,
        _ => Map::new(),
    };
    let serveur = serveur.trim_end_matches('/');
    let connue = match (d.get("poste").and_then(numero_de_poste), d.get("cle").and_then(cle_de_poste), d.get("serveur").and_then(Value::as_str)) {
        (Some(n), Some(c), Some(s)) if s.trim_end_matches('/') == serveur => Some((n, c)),
        _ => None,
    };
    let (numero, cle) = match connue {
        Some(id) => id,
        None => {
            let r = match Client::demander_poste(serveur) {
                Ok(r) => r,
                Err(e) => return Ok(Err(e)),
            };
            let (Some(n), Some(c)) = (r.get("poste").and_then(numero_de_poste), r.get("cle").and_then(cle_de_poste)) else {
                return Ok(Err(ErreurFilet::filet(format!("Filet DPAE : {}/poste n'a pas rendu une identite de poste.", serveur), 0)));
            };
            d.insert("poste".into(), json!(n));
            d.insert("cle".into(), json!(c));
            d.insert("serveur".into(), json!(serveur));
            ecrire_json(&p, &Value::Object(d), 2).map_err(|m| erreur("OSError", m))?;
            (n, c)
        }
    };
    Ok(Ok(Client::du_poste(serveur, &numero, &cle)))
}

/// Le serveur de l'editeur de cette compagnie (`_configuration_propre`) :
/// `config/filet-dpae.json`, `serveur`, sinon celui du compte du filet.
fn serveur_propre(racine: &Path) -> R<String> {
    let e = crate::filet::lire_etat(racine)?;
    let compte = get(&e, "compte")?;
    let compte = if vrai(compte) { compte.clone() } else { json!({}) };
    let texte = |v: &Value| if vrai(v) { py_str(v) } else { String::new() };
    let s = get(&e, "serveur")?;
    Ok(if vrai(s) { texte(s) } else { texte(get(&compte, "serveur")?) })
}

/// `incidents._configuration` : le serveur de l'editeur, pour les incidents, les
/// retours et leurs reponses, et la compagnie qui porte l'identite du poste
/// (son `config/diagnostic.json`) -- celui de la compagnie ouverte ; sinon celui
/// de la vraie compagnie du poste (depuis une FICTIVE seulement : un poste, une
/// identite) ; sinon celui que le paquet livre (`EDITEUR.json`), l'identite
/// restant dans la compagnie ouverte ; sinon celui du moteur
/// (`editeur::SERVEUR_PAR_DEFAUT`, lot S1). Toujours un serveur, depuis : `None`
/// ne se rend plus (le type reste, pour les appelants).
pub fn configuration(racine: &Path) -> R<Option<(String, PathBuf)>> {
    let serveur = serveur_propre(racine)?;
    if !serveur.is_empty() {
        return Ok(Some((serveur, racine.to_path_buf())));
    }
    let logiciel = dossier_du_code().parent().map(Path::to_path_buf).unwrap_or_default();
    let voisin = logiciel.parent().map(|r| r.join("prive")).unwrap_or_default();
    let reel = if voisin.join("config").is_dir() { voisin } else { logiciel.clone() };
    if crate::editeur::est_demonstration(racine) && reel != racine && reel.join("config").is_dir() {
        let serveur = serveur_propre(&reel)?;
        if !serveur.is_empty() {
            return Ok(Some((serveur, reel)));
        }
    }
    let e = std::fs::read(logiciel.join(FICHIER_EDITEUR)).ok().and_then(|o| String::from_utf8(o).ok()).and_then(|t| serde_json::from_str::<Value>(&t).ok());
    match e {
        Some(Value::Object(d)) => match d.get("serveur").filter(|v| vrai(v)).map(py_str) {
            Some(s) => Ok(Some((s, racine.to_path_buf()))),
            None => Ok(Some((crate::editeur::SERVEUR_PAR_DEFAUT.to_string(), racine.to_path_buf()))),
        },
        _ => Ok(Some((crate::editeur::SERVEUR_PAR_DEFAUT.to_string(), racine.to_path_buf()))),
    }
}

/// Le canal vers l'editeur, pret a envoyer.
pub enum Canal {
    /// Aucun serveur configure : rien ne peut partir.
    Aucun,
    /// Le client, avec l'identite du poste.
    Pret(Client),
    /// Un serveur configure, mais l'identite n'a pas pu etre obtenue (reseau, 429).
    Panne(ErreurFilet),
}

/// `canal` : le client des actions de l'editeur pour cette compagnie.
pub fn canal(racine: &Path) -> R<Canal> {
    Ok(match configuration(racine)? {
        None => Canal::Aucun,
        Some((serveur, ou)) => match client_du_poste(&ou, &serveur)? {
            Ok(c) => Canal::Pret(c),
            Err(e) => Canal::Panne(e),
        },
    })
}

/// `_marquer` : passe a `depose` les lignes nommees par (empreinte, quand).
fn marquer(racine: &Path, cles: &[(Value, Value)]) -> R<usize> {
    if cles.is_empty() {
        return Ok(0);
    }
    let p = chemin(racine);
    let mut toutes = lignes(&p);
    let quand = maintenant();
    let mut n = 0;
    for l in toutes.iter_mut() {
        let empreinte = get(l, "empreinte")?.clone();
        let q = get(l, "quand")?.clone();
        if cles.iter().any(|(e, w)| eq_py(e, &empreinte) && eq_py(w, &q)) && etat_d_envoi(l)? == Some(EN_ATTENTE.to_string()) {
            l["envoi"] = json!({"etat": DEPOSE, "quand": quand});
            n += 1;
        }
    }
    if n > 0 {
        ecrire_json(&p, &Value::Array(toutes), 2).map_err(|m| erreur("OSError", m))?;
    }
    Ok(n)
}

/// `a + b` de deux listes `recus` et `ecartes` du serveur (`None` : absent).
fn concatener(a: &Value, b: &Value) -> R<Vec<Value>> {
    let vide = Value::Array(Vec::new());
    let (a, b) = (if vrai(a) { a } else { &vide }, if vrai(b) { b } else { &vide });
    match (a, b) {
        (Value::Array(x), Value::Array(y)) => Ok(x.iter().chain(y).cloned().collect()),
        (Value::Array(_), autre) | (autre, _) => Err(erreur("TypeError", format!("'recus' et 'ecartes' : {}", crate::fichiers::attendu("une liste", nom_type(autre))))),
    }
}

/// `tuple(c)` d'un element de `recus` : (empreinte, quand).
fn paire(c: &Value) -> R<(Value, Value)> {
    let v: Vec<Value> = crate::etats::items(c)?.into_iter().map(|x| x.into_owned()).collect();
    Ok((v.first().cloned().unwrap_or(Value::Null), v.get(1).cloned().unwrap_or(Value::Null)))
}

/// Le bilan d'un passage, rempli au fur et a mesure : ce que le Python
/// mettait dans son dictionnaire.
#[derive(Default)]
struct Bilan {
    envoyes: usize,
    reste: usize,
    erreur: String,
    sans_serveur: bool,
}

impl Bilan {
    fn json(&self) -> Value {
        json!({"envoyes": self.envoyes, "reste": self.reste, "erreur": self.erreur, "sans_serveur": self.sans_serveur})
    }
}

fn passage(racine: &Path, bilan: &mut Bilan) -> R<()> {
    let attente = lire(racine, None, true)?;
    bilan.reste = attente.len();
    if attente.is_empty() {
        return Ok(());
    }
    let client = match canal(racine)? {
        Canal::Aucun => {
            bilan.sans_serveur = true;
            return Ok(());
        }
        Canal::Panne(e) => {
            bilan.erreur = e.message;
            return Ok(());
        }
        Canal::Pret(c) => c,
    };
    let a_faire = &attente[..attente.len().min(MAX_PAR_PASSAGE)];
    for lot in a_faire.chunks(LOT) {
        let mut contenu = Vec::new();
        for l in lot {
            contenu.push(pour_l_editeur(l)?);
        }
        let r = match client.rapporter(&contenu) {
            Ok(r) => r,
            Err(e) => {
                bilan.erreur = e.message;
                break;
            }
        };
        let cles: Vec<(Value, Value)> = concatener(get(&r, "recus")?, get(&r, "ecartes")?)?.iter().map(paire).collect::<R<_>>()?;
        bilan.envoyes += marquer(racine, &cles)?;
    }
    bilan.reste = lire(racine, None, true)?.len();
    Ok(())
}

/// `envoyer` : depose chez l'editeur ce qui attend. Rend un bilan ; ne leve
/// jamais. `{"envoyes", "reste", "erreur", "sans_serveur"}` -- `erreur` vide
/// quand tout s'est bien passe, ou quand il n'y a nulle part ou deposer. Une
/// panne laisse les lignes `en-attente` : le passage suivant reessaie.
pub fn envoyer(racine: &Path) -> Value {
    let mut bilan = Bilan::default();
    let p = chemin(racine);
    if !p.exists() || !envoi_actif(racine) {
        return bilan.json();
    }
    // Un autre passage est en cours : celui-ci n'a rien a faire.
    let marque = crate::verrou::marque("incidents", std::process::id(), &maintenant());
    let _verrou = match Verrou::prendre(&p.with_file_name(".envoi.verrou"), &marque, crate::verrou::DELAI_VERROU) {
        Ok(Ok(v)) => v,
        Ok(Err(_)) => return bilan.json(),
        Err(m) => {
            bilan.erreur = m;
            return bilan.json();
        }
    };
    if let Err(e) = passage(racine, &mut bilan) {
        bilan.erreur = e.message;
    }
    bilan.json()
}

