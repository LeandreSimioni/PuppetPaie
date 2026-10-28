//! Les tests des declarations sur des valeurs, sans compagnie : l'echeance de la DSN, les usages et les formats de la
//! norme, les changements d'individu, la nationalite, les options sante, la FCTU et l'amorcage, l'OETH et la CASC, la
//! DFS. Les oracles sont dans `declarations/tests/donnees/` (`PUPPETPAIE_REECRIRE_ORACLES=1` les reecrit). Un fichier
//! seul : `cargo nextest run --cargo-profile essais -p declarations -E 'test(/^<fichier>::/)'`.

#[path = "../outils/mod.rs"]
mod outils;

mod changements_individu;
mod dfs;
mod echeance_dsn;
mod fctu_amorcage;
mod nationalite;
mod oeth_casc;
mod options_sante;
mod usages_formats_ntt;
