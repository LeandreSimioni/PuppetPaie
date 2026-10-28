//! Les aides des tests des declarations : les oracles (communs aux crates) et les donnees livrees.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use serde_json::Value;

#[path = "../../../socle/tests/outils/oracles.rs"]
mod oracles;
pub use oracles::*;

/// Les donnees livrees (`logiciel/donnees`).
pub fn code_livre() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../logiciel/donnees")
}

pub fn lire(racine: &Path, rel: &str) -> Value {
    serde_json::from_slice(&std::fs::read(racine.join(rel)).unwrap()).unwrap_or_else(|e| panic!("{} illisible : {}", rel, e))
}
