//! Ce que la DSN et les signalements ecrivent, bloc par bloc : les corrections DSN, les rubriques 2027, le rejeu
//! des mois deposes des compagnies fictives.
//!
//! Un binaire pour tout le groupe : moins d'éditions de liens, moins de
//! disque, et les tests des différents fichiers se répartissent sur les cœurs. Un fichier seul :
//! `cargo nextest run --cargo-profile essais -E 'binary(dsn) & test(/^<fichier>::/)'` (le nom du fichier est le nom du module).
//! Aucun de ces fichiers ne change l'environnement du processus (`set_var`) : ceux qui le font restent seuls dans `tests/`.

#[path = "../outils/mod.rs"]
mod outils;

mod agirc_arrco;
mod preavis;
