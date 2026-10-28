//! Les aides des tests des documents : les oracles (communs aux crates) et un fichier de travail unique.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};

use rust_decimal::Decimal;

#[path = "../../../socle/tests/outils/oracles.rs"]
mod oracles;
pub use oracles::*;

pub fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap_or_else(|_| panic!("montant illisible : {:?}", s))
}

/// Un chemin de travail que nul autre test ne prend.
pub fn chemin_unique(nom: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("uniques");
    std::fs::create_dir_all(&d).unwrap();
    d.join(format!("{}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst), nom))
}
