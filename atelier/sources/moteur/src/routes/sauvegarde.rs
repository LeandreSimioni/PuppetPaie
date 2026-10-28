//! La route de la veille qui regarde la sauvegarde SANS RESEAU : `sauvegarde-derniere`, ce que l'index local
//! (`config/.sauvegarde.json`) dit du dernier passage : quand, combien de fichiers, vers quelle racine du Drive
//! (`Derniere::en_json`). Elle ne construit aucun client Drive, n'ouvre ni le coffre ni le jeton.
//!
//! `sauvegarder` et `archiver` ne sont PAS des routes : l'ouverture et la ligne de commande les appellent.

use std::collections::HashMap;

use serde_json::Value;

use droit::contexte::Contexte;
use socle::echec::R;
use socle::echec::erreur;
use crate::sauvegarde::derniere;

/// Les routes que ce module sert.
pub const ROUTES: [&str; 1] = ["sauvegarde-derniere"];

/// Une route de ce module : `sauvegarde-derniere`, `{"quand", "fichiers", "racine_drive"}`.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, _charge: &Value) -> R<Value> {
    match nom {
        "sauvegarde-derniere" => Ok(derniere(&ctx.racine).en_json()),
        autre => Err(erreur("ErreurOperation", format!("route inconnue : {}", autre))),
    }
}
