//! Les aides des tests des envois : un dossier par test, les oracles (communs aux crates), le faux serveur HTTP.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

#[path = "../../../socle/tests/outils/oracles.rs"]
mod oracles;
mod serveur;
pub use oracles::*;
pub use serveur::*;

/// Un dossier vide pour un test, sous le dossier temporaire de Cargo.
pub fn dossier_de_test(fichier: &str, test: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("envois").join(fichier).join(test);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Une piece de `envois/tests/donnees/`.
pub fn donnee(rel: &str) -> Vec<u8> {
    std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/donnees").join(rel)).unwrap()
}
