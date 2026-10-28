//! Les aides des tests du droit : les oracles (communs aux trois crates) et les donnees livrees.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::str::FromStr;

use rust_decimal::Decimal;
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

pub fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap_or_else(|_| panic!("montant illisible : {:?}", s))
}
