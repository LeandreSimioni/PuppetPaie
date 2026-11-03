//! LE SERVICE EST L'UNITE DE PAIE DES ARTISTES : les conventions payent au service les repetitions des artistes
//! (1285, art. X.3.2 C et X.3.3, `atelier/reference/sources/kali-1285-x3-remuneration-artistes-20260929.md`) ; un
//! technicien s'y paie a l'heure (1285, titre VI, `atelier/reference/sources/kali-1285-titre-vi-duree-du-travail-20261102.md` ;
//! 3252, ses grilles de techniciens a l'heure, au jour ou au mois, `logiciel/donnees/minima-conventionnels.json`). Un
//! engagement neuf qui paie un technicien au service se refuse, et rien n'est ecrit.

use serde_json::json;

use crate::outils::rejeu::rejouer;
use crate::outils::{refus, reussi, route};

#[test]
fn un_technicien_ne_se_paie_pas_au_service() {
    let d = crate::outils::dossier_de_test("technicien_au_service", "craig");
    // Les Faisceaux le 20/02/2026 : CRAIG est technicien.
    let (_, c) = rejouer("faisceaux-sans-civilite", &d);
    let ligne = |rubrique: &str, quantite: u32, taux: f64, heures: u32| {
        json!({"mois": "2026-03", "saisie": {"salarie": "CRAIG", "spectacle": "nuits-d-ete",
            "lignes": [{"rubrique": rubrique, "quantite": quantite, "taux": taux, "heures": heures, "dates": ["2026-03-10"]}]}})
    };
    let (_, phrase) = refus(&route(&c, "valider-engagement", ligne("repetition", 2, 70.0, 4), "2026-02-25"));
    assert!(phrase.contains("heure"), "le refus dit de le payer a l'heure : {}", phrase);
    assert!(!c.join("engagements").join("2026-03.json").exists(), "un refus n'ecrit rien");
    let etat = route(&c, "etat", json!({}), "2026-02-25");
    let craig = reussi(&etat)["salaries"].as_array().unwrap().iter().find(|s| s["cle"] == "CRAIG").cloned().unwrap();
    let service = reussi(&etat)["rubriques"].as_array().unwrap().iter().find(|r| r["code"] == "repetition").cloned().unwrap();
    assert_ne!(craig["statut"], service["reservee_au_statut"], "la liste ne propose pas le service a un technicien");
    reussi(&route(&c, "valider-engagement", ligne("heure", 8, 17.5, 1), "2026-02-25"));
}
