//! LE SERVICE EST L'UNITE DE PAIE DES ARTISTES : les conventions payent au service les repetitions des artistes
//! (1285, art. X.3.2 C et X.3.3, `atelier/reference/sources/kali-1285-x3-remuneration-artistes-20260929.md`) ; un
//! technicien s'y paie a l'heure (1285, titre VI, `atelier/reference/sources/kali-1285-titre-vi-duree-du-travail-20261102.md` ;
//! 3252, ses grilles de techniciens a l'heure, au jour ou au mois, `logiciel/donnees/minima-conventionnels.json`). Un
//! engagement neuf qui paie un technicien au service se refuse, et rien n'est ecrit.

use serde_json::{json, Value};

use crate::outils::rejeu::rejouer;
use crate::outils::{refus, reussi, route, saisir, valider};

#[test]
fn un_technicien_ne_se_paie_pas_au_service() {
    let d = crate::outils::dossier_de_test("technicien_au_service", "craig");
    // Les Faisceaux le 20/02/2026 : CRAIG est technicien.
    let (_, c) = rejouer("faisceaux-sans-civilite", &d);
    let carte_de = |ligne: Value| json!({"salarie": "CRAIG", "nature": "cddu_spectacle", "spectacle": "nuits-d-ete", "lignes": [ligne]});
    let repetition = json!({"rubrique": "repetition", "quantite": 2, "taux": 70.0, "heures": 4, "lieu": null, "dates": ["2026-03-10"], "planning": []});
    let heures = json!({"rubrique": "heure", "quantite": 8, "taux": 17.5, "lieu": null, "dates": ["2026-03-10"], "planning": []});
    let id = reussi(&saisir(&c, "2026-03", None, &carte_de(repetition), "2026-02-25"))["engagement"].clone();
    let (_, phrase) = refus(&route(&c, "valider-engagement", json!({"mois": "2026-03", "engagement": id}), "2026-02-25"));
    assert!(phrase.contains("heure"), "le refus dit de le payer a l'heure : {}", phrase);
    let mars = crate::outils::saisie(&c, "2026-03");
    let carte = mars["engagements"].as_array().unwrap().iter().find(|e| e["salarie"] == "CRAIG").unwrap();
    assert!(carte["valide"].is_null(), "un refus ne valide rien : {}", carte);
    let etat = route(&c, "etat", json!({}), "2026-02-25");
    let craig = reussi(&etat)["salaries"].as_array().unwrap().iter().find(|s| s["cle"] == "CRAIG").cloned().unwrap();
    let service = reussi(&etat)["rubriques"].as_array().unwrap().iter().find(|r| r["code"] == "repetition").cloned().unwrap();
    assert_ne!(craig["statut"], service["reservee_au_statut"], "la liste ne propose pas le service a un technicien");
    reussi(&valider(&c, "2026-03", Some(&id), &carte_de(heures), "2026-02-25"));
    // UN ARTISTE CADRE AU CACHET SE PAIE (le bug : « Il manque une donnée attendue », cle apec_t2_journaliere) : le metteur
    // en scene, cadre (3090, art. 6.3), paie l'Apec au-dela de la T1 journaliere, une base de tout le contrat.
    let mes = json!({"salarie": "CRAIG", "nature": "cddu_spectacle", "spectacle": "nuits-d-ete", "metier": "MET040",
        "lignes": [{"rubrique": "cachet", "quantite": 2, "taux": 400.0, "lieu": null, "dates": ["2026-04-10", "2026-04-11"], "planning": []}]});
    reussi(&valider(&c, "2026-04", None, &mes, "2026-02-25"));
}
