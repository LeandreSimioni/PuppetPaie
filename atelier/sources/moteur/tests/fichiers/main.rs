//! Les fichiers de la compagnie et ce qui en part : ecriture, journal, coffre, sauvegarde, lecture des courriels, Drive, et la
//! ligne de commande.
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

mod archivage;
mod bridage;
mod bulletin_lu;
mod caviardage;
mod cle_de_signature;
mod coffre;
mod conseils_banque;
mod donnees_reelles;
mod drive;
mod essai;
mod referentiels;
mod garde_reseau;
mod installations;
mod mise_a_jour;
mod restauration;
mod sauvegarde;
mod verifier_avant_commit;
