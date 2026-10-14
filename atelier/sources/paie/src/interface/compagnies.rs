//! Les compagnies du poste, telles que l'accueil les propose (`moteur/compagnies.py`, lot J3b).
//!
//! A cote du code : `prive/` (la vraie), `compagnies/*` (les autres vraies), `demo/*` (les fictives). La copie de
//! developpement (`prive-dev/`) n'est jamais proposee. Le mode « demo » (le geste secret de l'accueil) ne montre que
//! les fictives.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::fichiers::dossier_du_code;

const MARQUEUR: &str = "COPIE-DE-DEVELOPPEMENT.json";
const FICHIER_MODE_EDITION: &str = ".edition.json";
const COULEUR_DEFAUT: &str = "#B22222";
const DOSSIER_ORIGINE: &str = ".origine";
/// Ce qui appartient au poste et survit a une remise a zero.
const PROPRES_AU_POSTE: [&str; 4] =
    ["config/diagnostic.json", "journal/signalements.json", "journal/incidents.json", "journal/interface.log"];

/// Le dossier du logiciel (`logiciel/`) : le `CODE` du Python.
pub fn logiciel() -> PathBuf {
    // Le dossier du code peut porter un `..` (`paie/../donnees`) : on le resout avant d'en prendre le parent.
    let code = dossier_du_code();
    // Sous Windows, `canonicalize` rend un chemin `\\?\C:\...` que l'explorateur ne sait pas ouvrir : on retire le prefixe.
    let code = std::fs::canonicalize(&code)
        .map(|c| c.to_string_lossy().strip_prefix(r"\\?\").map(PathBuf::from).unwrap_or(c))
        .unwrap_or(code);
    code.parent().map(Path::to_path_buf).unwrap_or_default()
}

/// Ce qui est a cote du logiciel : `prive/`, `demo/`, `compagnies/`.
pub fn base() -> PathBuf {
    logiciel().parent().map(Path::to_path_buf).unwrap_or_default()
}

fn lire(p: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()
}

fn ecrire(p: &Path, v: &Value) -> Result<(), String> {
    crate::ecriture::ecrire_json(p, v, 2)
}

/// La vraie compagnie : `prive/` a cote du code, sinon le code lui-meme.
pub fn dossier_reel() -> PathBuf {
    let voisin = base().join("prive");
    if voisin.join("config").is_dir() {
        voisin
    } else {
        logiciel()
    }
}

pub fn est_copie(racine: &Path) -> bool {
    racine.join(MARQUEUR).is_file()
}

pub fn est_demonstration(racine: &Path) -> bool {
    crate::editeur::est_demonstration(racine)
}

/// demo ou complete : le paquet soude reste demo ; sinon ce que le poste a retenu, sinon le defaut du paquet.
pub fn est_demo() -> bool {
    if crate::editeur::paquet_soude() {
        return true;
    }
    if let Some(e) = lire(&base().join(FICHIER_MODE_EDITION)).and_then(|d| d.get("edition").and_then(Value::as_str).map(String::from)) {
        if e == "demo" || e == "complete" {
            return e == "demo";
        }
    }
    lire(&logiciel().join("EDITION.json")).and_then(|d| d.get("defaut").and_then(Value::as_str).map(|s| s == "demo")).unwrap_or(false)
}

/// Le geste secret de l'accueil : demo <-> complete, retenu sur le poste.
pub fn basculer_mode() -> Result<(), String> {
    if crate::editeur::paquet_soude() {
        return Ok(());
    }
    let nouveau = if est_demo() { "complete" } else { "demo" };
    ecrire(
        &base().join(FICHIER_MODE_EDITION),
        &json!({"edition": nouveau, "_quoi": "Ce que montre l'accueil de PuppetPaie sur ce poste : demo (les compagnies fictives) ou complete. Change par le geste de l'accueil."}),
    )
}

fn fiche(racine: &Path) -> Value {
    lire(&racine.join("config").join("entreprise.json")).filter(Value::is_object).unwrap_or_else(|| json!({}))
}

/// Une valeur de fiche, datee ou non : la plus recente si c'est une liste de periodes.
fn texte(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(a) => a.last().filter(|d| d.is_object()).map(|d| texte(&d["valeur"])).unwrap_or_default(),
        _ => String::new(),
    }
}

fn nom(racine: &Path) -> String {
    let ent = fiche(racine);
    let repli = racine.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let trouve = [&ent["nom_affichage"], &ent["raison_sociale"]]
        .into_iter()
        .find_map(|v| v.as_str().filter(|s| !s.is_empty()).map(String::from));
    trouve.unwrap_or(repli)
}

fn titre(s: &str) -> String {
    let mut sortie = String::new();
    let mut debut = true;
    for c in s.chars() {
        if c.is_alphabetic() {
            if debut {
                sortie.extend(c.to_uppercase());
            } else {
                sortie.extend(c.to_lowercase());
            }
            debut = false;
        } else {
            sortie.push(c);
            debut = true;
        }
    }
    sortie
}

fn lieu(racine: &Path) -> String {
    let ent = fiche(racine);
    let mut commune = texte(&ent["commune_affichage"]);
    if commune.is_empty() {
        commune = titre(&texte(&ent["commune"]));
    }
    let idcc = texte(&ent["idcc"]);
    let idcc = if idcc.is_empty() { String::new() } else { format!("IDCC {}", idcc) };
    [commune, idcc].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" · ")
}

fn couleur_valide(c: &str) -> bool {
    c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|x| x.is_ascii_hexdigit())
}

/// La couleur d'une compagnie (`config/affichage.json`), rouge par defaut.
pub fn couleur(racine: &Path) -> String {
    lire(&racine.join("config").join("affichage.json"))
        .and_then(|d| d.get("couleur").and_then(Value::as_str).map(String::from))
        .filter(|c| couleur_valide(c))
        .unwrap_or_else(|| COULEUR_DEFAUT.to_string())
}

/// Le logo de la compagnie, s'il est pose (svg d'abord).
pub fn logo(racine: &Path) -> Option<PathBuf> {
    ["logo.svg", "logo.png"].iter().map(|n| racine.join("gabarits").join("identite").join(n)).find(|p| p.is_file())
}

#[derive(Clone, Debug)]
pub struct Compagnie {
    pub id: String,
    pub nom: String,
    pub lieu: String,
    pub racine: PathBuf,
    pub reelle: bool,
    pub couleur: String,
}

fn entree(id: String, racine: &Path, reelle: bool) -> Compagnie {
    let mut n = nom(racine);
    if !reelle {
        n = n.replace(" (démonstration)", "").replace(" (demonstration)", "");
    }
    Compagnie { id, nom: n, lieu: lieu(racine), racine: racine.to_path_buf(), reelle, couleur: couleur(racine) }
}

fn sous_dossiers(dossier: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dossier)
        .map(|it| it.filter_map(|e| e.ok().map(|e| e.path())).filter(|d| d.join("config").join("entreprise.json").is_file()).collect())
        .unwrap_or_default();
    v.sort();
    v
}

fn nom_dossier(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Ce que propose l'accueil, dans l'ordre des tuiles : les vraies, puis les fictives.
pub fn liste() -> Vec<Compagnie> {
    let b = base();
    let mut out = Vec::new();
    if !est_demo() {
        let reel = dossier_reel();
        if reel.join("config").is_dir() {
            out.push(entree("reelle".into(), &reel, true));
        }
        for d in sous_dossiers(&b.join("compagnies")) {
            out.push(entree(format!("cie:{}", nom_dossier(&d)), &d, true));
        }
    }
    for d in sous_dossiers(&b.join("demo")) {
        out.push(entree(format!("demo:{}", nom_dossier(&d)), &d, false));
    }
    out
}

/// Le dossier d'une tuile -- et seulement d'une tuile : l'identifiant ne designe jamais un chemin qu'on n'a pas propose.
pub fn racine_choisie(id: &str) -> Result<PathBuf, String> {
    liste()
        .into_iter()
        .find(|c| c.id == id)
        .map(|c| c.racine)
        .ok_or_else(|| format!("Compagnie inconnue ou absente de cette version : {}", id))
}

pub fn poser_couleur(id: &str, valeur: &str) -> Result<String, String> {
    if !couleur_valide(valeur) {
        return Err(format!("Couleur attendue sous la forme #RRGGBB : '{}'", valeur));
    }
    let c = liste().into_iter().find(|c| c.id == id && c.reelle).ok_or("Seule une compagnie reelle a une couleur a choisir.")?;
    let v = valeur.to_uppercase();
    ecrire(&c.racine.join("config").join("affichage.json"), &json!({"couleur": v}))?;
    Ok(v)
}

fn nom_de_dossier(nom: &str) -> String {
    let mut s = String::new();
    for c in nom.to_lowercase().chars() {
        let pli = match c {
            'à' | 'â' | 'ä' | 'á' | 'ã' | 'å' => "a",
            'é' | 'è' | 'ê' | 'ë' => "e",
            'î' | 'ï' | 'í' | 'ì' => "i",
            'ô' | 'ö' | 'ó' | 'ò' | 'õ' => "o",
            'ù' | 'û' | 'ü' | 'ú' => "u",
            'ç' => "c",
            'ñ' => "n",
            'ÿ' => "y",
            'œ' => "oe",
            'æ' => "ae",
            c if c.is_ascii_alphanumeric() => {
                s.push(c);
                continue;
            }
            c if !c.is_ascii() => "",
            _ => "-",
        };
        s.push_str(pli);
    }
    let mut net = String::new();
    for c in s.chars() {
        if c == '-' && (net.is_empty() || net.ends_with('-')) {
            continue;
        }
        net.push(c);
    }
    let net: String = net.trim_end_matches('-').chars().take(40).collect();
    if net.is_empty() {
        "compagnie".into()
    } else {
        net
    }
}

/// « Ajouter une compagnie » : le dossier et les trois fiches sans lesquelles le contexte ne s'ouvre pas.
pub fn ajouter(nom: &str) -> Result<Compagnie, String> {
    if est_demo() {
        return Err("La version demo n'ajoute pas de compagnie.".into());
    }
    let nom = nom.split_whitespace().collect::<Vec<_>>().join(" ");
    if nom.chars().count() < 2 {
        return Err("Le nom de la compagnie, s'il vous plait.".into());
    }
    let parent = base().join("compagnies");
    let racine_nom = nom_de_dossier(&nom);
    let mut dossier = parent.join(&racine_nom);
    let mut n = 2;
    while dossier.exists() {
        dossier = parent.join(format!("{}-{}", racine_nom, n));
        n += 1;
    }
    let cfg = dossier.join("config");
    std::fs::create_dir_all(&cfg).map_err(|e| e.to_string())?;
    ecrire(&cfg.join("entreprise.json"), &json!({"nom_affichage": nom, "raison_sociale": nom.to_uppercase()}))?;
    ecrire(&cfg.join("salaries.json"), &json!({}))?;
    ecrire(&cfg.join("spectacles.json"), &json!({}))?;
    Ok(entree(format!("cie:{}", nom_dossier(&dossier)), &dossier, true))
}

fn origine(racine: &Path) -> PathBuf {
    racine.parent().unwrap_or(racine).join(DOSSIER_ORIGINE).join(nom_dossier(racine))
}

fn copier_arbre(de: &Path, vers: &Path, rel: &Path, sauf: &[&str]) -> std::io::Result<()> {
    std::fs::create_dir_all(vers.join(rel))?;
    for e in std::fs::read_dir(de.join(rel))? {
        let e = e?;
        let r = rel.join(e.file_name());
        let rs = r.to_string_lossy().replace('\\', "/");
        if sauf.contains(&rs.as_str()) || e.file_name() == "__pycache__" {
            continue;
        }
        if e.file_type()?.is_dir() {
            copier_arbre(de, vers, &r, sauf)?;
        } else {
            std::fs::copy(e.path(), vers.join(&r))?;
        }
    }
    Ok(())
}

/// La photo d'origine d'une fictive, prise avant le premier geste (un dossier `.origine/<nom>/`).
pub fn figer_origine(racine: &Path) -> Result<(), String> {
    if !est_demonstration(racine) {
        return Ok(());
    }
    let o = origine(racine);
    if o.is_dir() {
        return Ok(());
    }
    let provisoire = o.with_extension("part");
    let _ = std::fs::remove_dir_all(&provisoire);
    copier_arbre(racine, &provisoire, Path::new(""), &PROPRES_AU_POSTE).map_err(|e| e.to_string())?;
    std::fs::rename(&provisoire, &o).map_err(|e| e.to_string())
}

/// La fictive revient a sa photo d'origine ; ce qui appartient au poste est garde. Rend (nom, deja).
pub fn remettre_a_zero(id: &str) -> Result<(String, bool), String> {
    let c = liste()
        .into_iter()
        .find(|c| c.id == id && !c.reelle && est_demonstration(&c.racine))
        .ok_or("Seule une compagnie de démonstration se remet à zéro.")?;
    let o = origine(&c.racine);
    if !o.is_dir() {
        return Ok((c.nom, true));
    }
    let parent = c.racine.parent().unwrap_or(&c.racine).to_path_buf();
    let n = nom_dossier(&c.racine);
    let neuve = parent.join(format!(".{}.neuve", n));
    let ancienne = parent.join(format!(".{}.ancienne", n));
    for d in [&neuve, &ancienne] {
        let _ = std::fs::remove_dir_all(d);
    }
    copier_arbre(&o, &neuve, Path::new(""), &[]).map_err(|e| e.to_string())?;
    for rel in PROPRES_AU_POSTE {
        if let Ok(octets) = std::fs::read(c.racine.join(rel)) {
            let cible = neuve.join(rel);
            if let Some(p) = cible.parent() {
                let _ = std::fs::create_dir_all(p);
            }
            std::fs::write(cible, octets).map_err(|e| e.to_string())?;
        }
    }
    std::fs::rename(&c.racine, &ancienne).map_err(|e| e.to_string())?;
    std::fs::rename(&neuve, &c.racine).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&ancienne);
    Ok((c.nom, false))
}

/// L'etiquette du bandeau d'une compagnie qui n'est pas reelle (deja en HTML).
pub fn texte_copie(racine: &Path) -> String {
    if est_demonstration(racine) {
        return "d&eacute;monstration &mdash; les envois sont simul&eacute;s".into();
    }
    if racine.parent().and_then(Path::file_name).is_some_and(|n| n == "demo") {
        return "compagnie fictive &mdash; rien ne part".into();
    }
    "copie de test &mdash; rien ne part en r&eacute;el".into()
}
