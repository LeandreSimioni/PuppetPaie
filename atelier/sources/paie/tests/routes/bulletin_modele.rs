//! L'APERCU DES MODELES DE BULLETIN (`bulletin-modele-apercu`) : la paie de la compagnie fictive livree
//! (`logiciel/donnees/apercu-bulletin/`, configuration datee) se calcule et s'imprime sous chaque modele.

use paie::interface::compagnies::ajouter_dans;
use serde_json::json;

use crate::outils::{dossier_de_test, reussi, route};

#[test]
fn l_apercu_de_chaque_modele_rend_un_bulletin() {
    let racine = ajouter_dans(&dossier_de_test("bulletin_modele", "apercu"), "Les Tréteaux de l'Aperçu").unwrap().racine;
    for modele in ["simplifié", "complet"] {
        let r = route(&racine, "bulletin-modele-apercu", json!({"modele": modele}), "2026-11-02");
        let pdf = reussi(&r)["pdf"].as_str().unwrap_or_default().to_string();
        assert!(pdf.starts_with("JVBERi0"), "{} : pas un PDF", modele);
    }
}
