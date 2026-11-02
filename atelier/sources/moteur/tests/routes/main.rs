//! Les routes de l'interface (`moteur route`) : chaque test monte sa compagnie fictive dans un dossier a lui.
//!
//! Un binaire pour tout le groupe : moins d'éditions de liens, moins de
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
mod compagnie_supprimee;
mod contrat_imprime;
mod contrat_a_cheval;
mod homonymes;
mod case_de_depot;
mod notification_atmp;
mod net_a_virer;
mod duree_du_travail;
mod arret_prolonge;
mod signalement_apres_echeance;
mod tarif_conventionnel;
mod cdd_d_artiste;
mod dsn_sans_salarie;
mod une_liste_de_ce_qui_est_du;
mod nuitee_au_dela;
mod second_contrat_au_mois;
mod identite_qui_arrive;
