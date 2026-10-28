//! Le calcul de la paie contre les textes, des valeurs payees et les oracles de `tests/donnees/` (que
//! `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit) : les pieces du calcul, le bulletin, le bordereau, et les regles du droit qui
//! ne se prouvent qu'au calcul d'un engagement (plafond, minima, conges, cumul, caisse des conges, fin de contrat).
//!
//! Un binaire pour tout le groupe : moins d'éditions de liens, moins de
//! disque, et les tests des différents fichiers se répartissent sur les cœurs. Un fichier seul :
//! `cargo nextest run --cargo-profile essais -E 'binary(calcul) & test(/^<fichier>::/)'` (le nom du fichier est le nom du module).
//! Aucun de ces fichiers ne change l'environnement du processus (`set_var`) : ceux qui le font restent seuls dans `tests/`.

#[path = "../outils/mod.rs"]
mod outils;

mod bordereau;
mod bulletin;
mod calcul_pieces;
mod conges;
mod dfs;
mod echeance_dsn;
