//! Les tests des documents sur des valeurs, sans compagnie : le bordereau contre la table des CTP et les bordereaux
//! preleves, les formats du bulletin, l'attestation employeur, le texte compare d'un avenant. Les oracles sont dans
//! `documents/tests/donnees/` (`PUPPETPAIE_REECRIRE_ORACLES=1` les reecrit). Un fichier seul :
//! `cargo nextest run --cargo-profile essais -p documents -E 'test(/^<fichier>::/)'`.

#[path = "../outils/mod.rs"]
mod outils;

mod aer;
mod avenant;
mod bordereau;
mod bulletin;
