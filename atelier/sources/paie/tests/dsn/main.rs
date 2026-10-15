//! Ce que la DSN et les signalements ecrivent, bloc par bloc : les corrections DSN par vague, les rubriques 2027, le rejeu
//! des mois deposes des compagnies fictives.
//!
//! Un binaire pour tout le groupe (11/10/2026, séance « tests plus rapides ») : moins d'éditions de liens, moins de
//! disque, et les tests des différents fichiers se répartissent sur les cœurs. Un fichier seul :
//! `cargo nextest run --cargo-profile essais -E 'binary(dsn) & test(/^<fichier>::/)'` (le nom du fichier est le nom du module).
//! Aucun de ces fichiers ne change l'environnement du processus (`set_var`) : ceux qui le font restent seuls dans `tests/`.

#[path = "../outils/mod.rs"]
mod outils;

mod agirc_arrco;
mod bloc_40_86;
mod bloc_41;
mod bloc_44;
mod changements_individu;
mod composants_versement;
mod cpf_cdd;
mod fictives;
mod dfs_dsn;
mod fctu_amorcage;
mod fin_cddu;
mod imprevus;
mod naf_2025;
mod nationalite;
mod net_deja_verse;
mod norme_par_fichier;
mod oeth_casc;
mod ordre_signalements;
mod options_sante;
mod preavis;
mod regularisation_declaree;
mod rubriques_2027;
mod signalement;
mod suspension;
mod usages_formats_ntt;
mod versement_dgfip;
