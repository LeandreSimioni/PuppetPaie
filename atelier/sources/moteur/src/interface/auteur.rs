//! L'AUTEUR DU LOGICIEL, la seule place ou l'editeur signe : la mention du pied de l'accueil et de la page d'une
//! compagnie (rendue par les routes `compagnies` et `etat`, lue aussi par un LLM), et les sites de l'editeur, les seuls
//! que la fenetre fait ouvrir au navigateur du poste (`poste::ouvrir_site`). Le verrou du nom de la compagnie du poste
//! (`tests/fichiers/donnees_reelles.rs`) ne laisse passer que ce fichier.

use serde_json::{json, Value};

/// Les sites de l'editeur.
pub const SITES: [&str; 2] = ["ueueue.eu", "puppetpaie.fr"];

/// La mention : la phrase, le nom qui y porte le lien, l'adresse du lien.
pub fn auteur() -> Value {
    json!({"phrase": "Codé et maintenu par la Compagnie UEUEUE", "nom": "Compagnie UEUEUE", "adresse": "https://ueueue.eu"})
}
