//! Le choix de la compagnie, comme dans le Python (`moteur/contexte.py` : `racine_donnees`, `moteur/__main__.py` :
//! `--dev`, `moteur/compagnies.py`) : rien de neuf.
//!
//!   - `PUPPETPAIE_DONNEES` si elle est posee ;
//!   - sinon le dossier `prive/` voisin du code, s'il porte un `config/` ;
//!   - sinon le code lui-meme (installation d'un seul tenant) ;
//!   - `paie --dev COMMANDE` : la copie de developpement `prive-dev/`, si le mode developpeur est allume et la copie
//!     faite (le Python pose `PUPPETPAIE_DONNEES` ; ici aussi, pour le reste du moteur).
//!
//! Et le jour : celui de Paris (le Python pose `TZ=Europe/Paris`), pas celui d'UTC. `PUPPETPAIE_AUJOURDHUI`, quand elle
//! est posee, regle `aujourdhui` ; sinon on la pose a l'heure de Paris pour que tout le moteur lise le meme jour.

use std::path::{Path, PathBuf};

use socle::date::Jour;
use socle::fichiers::dossier_du_code;

const MARQUEUR: &str = "COPIE-DE-DEVELOPPEMENT.json";
const FICHIER_MODE: &str = ".mode-developpeur.json";
const NOM_DEV: &str = "prive-dev";

/// `CODE.parent` du Python : le dossier qui porte `logiciel/` (celui du depot). `fixer_le_code` a rendu le chemin du
/// code absolu et sans `..`.
fn parent_du_logiciel() -> PathBuf {
    // `<...>/logiciel/donnees` -> `<...>/logiciel` -> `<...>`
    dossier_du_code().parent().and_then(Path::parent).map(Path::to_path_buf).unwrap_or_default()
}

/// Le dossier `donnees/` (le code livre : baremes, echeancier...) : `PUPPETPAIE_CODE` si elle est posee, sinon celui que
/// l'on trouve a cote du programme (`logiciel/puppetpaie.exe` -> `logiciel/donnees` ; `atelier/sources/paie/bin/<systeme>/paie`
/// -> `logiciel/donnees` en remontant), a defaut celui du depot qui a compile le programme.
/// Le chemin est rendu absolu et sans `..` : le moteur remonte de `moteur/` a `logiciel/` puis au depot par `parent()`.
pub fn fixer_le_code() {
    if std::env::var_os("PUPPETPAIE_CODE").filter(|v| !v.is_empty()).is_some() {
        return;
    }
    let valide = |p: &Path| p.join("echeancier-base.json").is_file();
    let mut candidats: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        for a in exe.ancestors().skip(1) {
            candidats.push(a.join("donnees"));
            candidats.push(a.join("logiciel").join("donnees"));
        }
    }
    candidats.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../logiciel/donnees"));
    for c in candidats {
        if valide(&c) {
            if let Ok(abs) = c.canonicalize() {
                // Sous Windows, `canonicalize` rend un chemin `\\?\C:\...` que le reste du moteur ne compare pas.
                let abs = abs.to_string_lossy().strip_prefix(r"\\?\").map(PathBuf::from).unwrap_or(abs);
                std::env::set_var("PUPPETPAIE_CODE", abs);
                return;
            }
        }
    }
}

/// `compagnies.dossier_dev` : dans l'atelier (`atelier/prive-dev`), pas a cote de ce que les gens recoivent (rangement
/// du 09/10/2026 : a la racine, `logiciel/`, `fictives/`, `prive/` et `atelier/`).
pub fn dossier_dev() -> PathBuf {
    parent_du_logiciel().join("atelier").join(NOM_DEV)
}

/// `compagnies._chemin_mode` : `PUPPETPAIE_CLE` le deplace (c'est ainsi que les tests le tiennent hors du poste).
fn chemin_mode() -> PathBuf {
    match std::env::var_os("PUPPETPAIE_CLE").filter(|v| !v.is_empty()) {
        Some(d) => PathBuf::from(d).join(FICHIER_MODE),
        // Le mode developpeur est un reglage de l'atelier : il vit dans `atelier/` (rangement du 09/10/2026).
        None => parent_du_logiciel().join("atelier").join(FICHIER_MODE),
    }
}

/// `compagnies.mode_actif`.
pub fn mode_actif() -> bool {
    std::fs::read_to_string(chemin_mode())
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.get("actif").map(socle::fichiers::vrai))
        .unwrap_or(false)
}

/// `compagnies.copie_faite`.
pub fn copie_faite() -> bool {
    std::fs::read_to_string(dossier_dev().join(MARQUEUR)).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()).is_some()
}

/// Le jour de Paris, aujourd'hui.
pub fn jour_de_paris() -> Jour {
    socle::temps::jour_de_paris()
}

/// Pose `PUPPETPAIE_AUJOURDHUI` a l'heure de Paris quand rien ne l'a posee (le Python lit `date.today()`, a Paris).
pub fn poser_le_jour() -> Jour {
    let paris = jour_de_paris();
    if std::env::var_os("PUPPETPAIE_AUJOURDHUI").filter(|v| !v.is_empty()).is_none() {
        std::env::set_var("PUPPETPAIE_AUJOURDHUI", paris.iso());
    }
    paris
}

/// La racine de la compagnie de cette commande. `dev` : la copie de developpement, ou la phrase du refus
/// (`moteur/__main__.py` sort avec ces phrases).
pub fn racine(dev: bool) -> Result<PathBuf, String> {
    if dev {
        if !mode_actif() {
            return Err("Mode developpeur eteint : `--dev` est refuse sur ce poste (`paie dev activer`, commande de l'atelier).".into());
        }
        if !copie_faite() {
            return Err("Pas de copie de developpement (`paie dev copier`, commande de l'atelier).".into());
        }
        let d = dossier_dev();
        std::env::set_var("PUPPETPAIE_DONNEES", &d);
        return Ok(d);
    }
    Ok(crate::routes_accueil::racine_du_poste())
}

/// La compagnie est-elle une copie (developpement ou compagnie fictive) ?
pub fn est_copie(racine: &Path) -> bool {
    racine.join(MARQUEUR).is_file()
}
