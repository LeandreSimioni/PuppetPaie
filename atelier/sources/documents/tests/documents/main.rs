//! Les tests des documents sur des valeurs, sans compagnie : le bordereau contre les bordereaux preleves, l'attestation
//! employeur contre les paies. Les oracles sont dans `documents/tests/donnees/`. Un fichier seul :
//! `cargo nextest run --cargo-profile essais -p documents -E 'test(/^<fichier>::/)'`.

#[path = "../outils/mod.rs"]
mod outils;

mod aer;
mod bordereau;
