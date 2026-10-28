//! Les aides des tests des documents : les oracles (communs aux crates) et la lecture d'un montant.

#![allow(dead_code)]

use std::str::FromStr;

use rust_decimal::Decimal;

#[path = "../../../socle/tests/outils/oracles.rs"]
mod oracles;
pub use oracles::*;

pub fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap_or_else(|_| panic!("montant illisible : {:?}", s))
}
