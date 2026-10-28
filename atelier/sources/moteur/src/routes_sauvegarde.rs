//! Les deux routes de la veille qui regardent la sauvegarde SANS RESEAU (lot H4b) : `sauvegarde-derniere` et
//! `sauvegarde-rien-n-a-bouge`.
//!
//! `web.py` n'en lit que ce que l'index local (`config/.sauvegarde.json`) dit du dernier passage : quand, combien de
//! fichiers, vers quelle racine du Drive (`Derniere::en_json`), et si le disque n'a pas bouge depuis
//! (`sauvegarde::rien_n_a_bouge`). Ni l'une ni l'autre ne construit de client Drive, ni n'ouvre le coffre ou le jeton :
//! elles ne lisent que cet index et les tailles et dates des fichiers.
//!
//! `sauvegarder` et `archiver` ne sont PAS des routes ici : l'ouverture (lot I) et la ligne de commande (lot J) les appellent.

use std::collections::HashMap;

use serde_json::{json, Value};

use droit::contexte::Contexte;
use socle::echec::R;
use socle::echec::erreur;
use crate::sauvegarde::{derniere, rien_n_a_bouge, RACINE_DRIVE};

/// Les routes que ce module sert.
pub const ROUTES: [&str; 2] = ["sauvegarde-derniere", "sauvegarde-rien-n-a-bouge"];

/// Une route de ce module.
///
/// - `sauvegarde-derniere` : `{"quand", "fichiers", "racine_drive"}`, comme `sauvegarde.derniere` du Python.
/// - `sauvegarde-rien-n-a-bouge` : `{"rien_n_a_bouge": bool}` ; la charge peut porter `racine_drive` (un texte ; `PuppetPaie`
///   par defaut, la seule que la veille utilise).
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "sauvegarde-derniere" => Ok(derniere(&ctx.racine).en_json()),
        "sauvegarde-rien-n-a-bouge" => {
            let racine_drive = match charge.get("racine_drive") {
                None | Some(Value::Null) => RACINE_DRIVE,
                Some(Value::String(t)) if !t.is_empty() => t.as_str(),
                Some(_) => return Err(erreur("ErreurOperation", "racine_drive : un texte est attendu.")),
            };
            Ok(json!({"rien_n_a_bouge": rien_n_a_bouge(&ctx.racine, racine_drive)}))
        }
        autre => Err(erreur("ErreurOperation", format!("route inconnue : {}", autre))),
    }
}
