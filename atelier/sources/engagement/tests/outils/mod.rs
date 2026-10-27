//! Les aides des tests de l'engagement : une compagnie montee de la copie figee d'une fictive, dans le dossier du test.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use serde_json::Value;

/// La copie figee d'une compagnie fictive, celle de toutes les crates (`paie/tests/donnees/fictives/<nom>`), qu'aucun
/// test n'ecrit.
pub fn fictive(nom: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../paie/tests/donnees/fictives").join(nom)
}

/// `CARGO_TARGET_TMPDIR/<fichier>/<test>`, vide puis cree : le dossier d'un test, a lui seul.
pub fn dossier_de_test(fichier: &str, test: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("engagement").join(fichier).join(test);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn copier(de: &Path, vers: &Path) {
    std::fs::create_dir_all(vers).unwrap();
    for e in std::fs::read_dir(de).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            copier(&p, &vers.join(e.file_name()));
        } else {
            std::fs::copy(&p, vers.join(e.file_name())).unwrap();
        }
    }
}

/// La config et le journal de la fictive `nom`, sans engagement, dans `racine`.
pub fn config_et_journal(nom: &str, racine: &Path) -> PathBuf {
    let d = fictive(nom);
    for sous in ["config", "journal"] {
        if d.join(sous).is_dir() {
            copier(&d.join(sous), &racine.join(sous));
        }
    }
    racine.to_path_buf()
}

pub fn lire(racine: &Path, rel: &str) -> Value {
    serde_json::from_slice(&std::fs::read(racine.join(rel)).unwrap()).unwrap_or_else(|e| panic!("{} illisible : {}", rel, e))
}

pub fn ecrire(racine: &Path, rel: &str, v: &Value) {
    let p = racine.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, serde_json::to_vec_pretty(v).unwrap()).unwrap();
}
