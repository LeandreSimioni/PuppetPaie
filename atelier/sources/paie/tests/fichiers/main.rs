//! Les fichiers de la compagnie et ce qui en part : ecriture, journal, coffre, depots, retours, sauvegarde, courriels, Drive,
//! transport, et la ligne de commande.
//!
//! Un binaire pour tout le groupe (11/10/2026, séance « tests plus rapides ») : moins d'éditions de liens, moins de
//! disque, et les tests des différents fichiers se répartissent sur les cœurs. Un fichier seul :
//! `cargo nextest run --cargo-profile essais -E 'binary(fichiers) & test(/^<fichier>::/)'` (le nom du fichier est le nom du module).
//! Aucun de ces fichiers ne change l'environnement du processus (`set_var`) : ceux qui le font restent seuls dans `tests/`.

#[path = "../outils/mod.rs"]
mod outils;

#[allow(dead_code)]
#[path = "../bulletins_fictifs/mod.rs"]
mod bulletins_fictifs;
#[allow(dead_code)]
#[path = "../faux_drive/mod.rs"]
mod faux_drive;

mod aer;
mod archivage;
mod bulletin_lu;
mod caviardage;
mod cle_de_signature;
mod coffre;
mod commandes;
mod courriel;
mod courriel_lecture;
mod depot;
mod donnees_reelles;
mod drive;
mod ecriture;
mod envoi;
mod envoi_mime;
mod essai;
mod importation_tri;
mod incidents_filet;
mod mise_a_jour;
mod restauration;
mod retours;
mod sauvegarde;
mod tables_generees;
mod transport;
