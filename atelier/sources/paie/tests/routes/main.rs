//! Les routes de l'interface (`paie route`) : chaque test monte sa compagnie fictive dans un dossier a lui.
//!
//! Un binaire pour tout le groupe (11/10/2026, séance « tests plus rapides ») : moins d'éditions de liens, moins de
//! disque, et les tests des différents fichiers se répartissent sur les cœurs. Un fichier seul :
//! `cargo nextest run --cargo-profile essais -E 'binary(routes) & test(/^<fichier>::/)'` (le nom du fichier est le nom du module).
//! Aucun de ces fichiers ne change l'environnement du processus (`set_var`) : ceux qui le font restent seuls dans `tests/`.

#[path = "../outils/mod.rs"]
mod outils;

#[allow(dead_code)]
#[path = "../bulletins_fictifs/mod.rs"]
mod bulletins_fictifs;
#[allow(dead_code)]
#[path = "../faux_drive/mod.rs"]
mod faux_drive;

mod demarrage;
mod import_neuve;
mod fictive_ajoutee;
mod compagnie_supprimee;
