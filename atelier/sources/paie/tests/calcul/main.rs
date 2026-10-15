//! Le calcul de la paie contre les textes, des valeurs payees et les oracles de `tests/donnees/` (que
//! `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit) : bareme, clauses, minima, plafond, pieces du calcul, bulletin, bordereau.
//!
//! Un binaire pour tout le groupe (11/10/2026, séance « tests plus rapides ») : moins d'éditions de liens, moins de
//! disque, et les tests des différents fichiers se répartissent sur les cœurs. Un fichier seul :
//! `cargo nextest run --cargo-profile essais -E 'binary(calcul) & test(/^<fichier>::/)'` (le nom du fichier est le nom du module).
//! Aucun de ces fichiers ne change l'environnement du processus (`set_var`) : ceux qui le font restent seuls dans `tests/`.

#[path = "../outils/mod.rs"]
mod outils;

mod bareme;
mod bordereau;
mod bulletin;
mod bulletin_modeles;
mod caisse_conges_affiliation;
mod calcul_pieces;
mod calculer;
mod catalogue;
mod cumul;
mod clauses;
mod conges;
mod contexte;
mod contrats;
mod dfs;
mod emplois;
mod echeance_dsn;
mod engagement;
mod imprevus;
mod majoration_caisses_conges;
mod minima;
mod net_social;
mod pas;
mod periodes;
mod permanents;
mod plafond;
mod plafond_intermittents;
mod plafond_texte;
mod seuils_effectif;
