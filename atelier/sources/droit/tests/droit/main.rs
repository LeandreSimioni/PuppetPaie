//! Les tests du droit : une regle tiree d'un texte depose (qui le cite), une grille a graine fixe par bareme, un bug reel
//! qui a echappe. Les oracles sont dans `droit/tests/donnees/` (`PUPPETPAIE_REECRIRE_ORACLES=1` les reecrit). Un fichier
//! seul : `cargo nextest run --cargo-profile essais -p droit -E 'test(/^<fichier>::/)'`.

#[path = "../outils/mod.rs"]
mod outils;

mod caisse_conges;
mod catalogue;
mod clauses;
mod emplois;
mod natures;
mod pas;
mod permanents;
mod plafond;
