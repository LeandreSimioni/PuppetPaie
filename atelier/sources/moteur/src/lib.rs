//! Le moteur de PuppetPaie au-dessus des envois (`envois`), des documents et des declarations : la ligne de commande
//! `moteur`, les routes de l'interface, la compagnie (`compagnie`), l'import (`import`) et l'atelier (`atelier`).
//!
//! Les fichiers de donnees se lisent en `serde_json::Value` : une cle inconnue survit, rien a migrer.

pub mod apercu_dsn;
pub mod atelier;
pub mod commandes;
pub mod compagnie;
pub mod etat_garde;
pub mod historique_mois;
pub mod horloge;
pub mod import;
pub mod interface;
pub mod obligations;
pub mod regularisation;
pub mod routes;
pub mod service;
pub mod simulation_courriels;
