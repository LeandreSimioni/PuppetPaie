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

mod arret_prolonge;
mod administrateur;
mod avenant;
mod calcul_du_mois;
mod carte_figee;
mod conseils;
mod demarrage;
mod echeance_dsn;
mod historique_mois;
mod modifier_apres_echeance;
mod etat_garde;
mod fictive_ajoutee;
mod compagnie_supprimee;
mod prechauffage;
mod routes_accueil;
mod routes_actes;
mod routes_apercus;
mod routes_arrets;
mod routes_imprevus;
mod routes_bulletin_modele;
mod routes_cloture_compta;
mod routes_coffre;
mod routes_contrats;
mod routes_couts;
mod routes_dossiers;
mod routes_ecritures;
mod routes_etats;
mod routes_fiches;
mod routes_importation;
mod routes_listes;
mod routes_mois;
mod routes_obligations;
mod routes_organismes;
mod routes_ouverture;
mod routes_produire;
mod routes_regularisations;
mod routes_reprise;
mod routes_saisie;
mod routes_sauvegarde;
mod routes_validation;
