//! UNE JOURNEE DE TRAVAIL SAISIE : au-dela de 24 heures, elle se refuse (une journee n'en a que 24) ; au-dela de la duree
//! quotidienne maximale de 10 heures (code du travail, L. 3121-18 ; jusqu'a 12 heures par convention ou accord,
//! L. 3121-19 : `atelier/reference/sources/code-du-travail/articles-20261030.md`), elle se dit sur sa carte sans se
//! refuser. Le banc des Treteaux, FULLER payee a l'heure le 28/11.

use serde_json::{json, Value};

use crate::outils::rejeu::{rejouer_dossier, scenario};
use crate::outils::{dossier_de_test, refus, reussi, route, saisie};

#[test]
fn une_journee_de_plus_de_24_heures_se_refuse_et_plus_de_10_heures_se_dit() {
    let d = dossier_de_test("duree_du_travail", "journee");
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    let gestes = std::fs::read_to_string(scenario("treteaux-du-banc").join("gestes.jsonl")).unwrap();
    let debut: Vec<&str> = gestes.lines().take_while(|l| !l.contains("\"geste\":\"jour\"")).collect();
    std::fs::write(sc.join("gestes.jsonl"), debut.join("\n") + "\n").unwrap();
    let (_, c) = rejouer_dossier(&sc, &d);
    let jour = "2026-11-02";
    let spectacle = saisie(&c, "2026-11")["engagements"].as_array().unwrap().iter().find(|e| e["salarie"] == "FULLER").unwrap()["spectacle"].clone();
    let regie = |heures: i64| json!({"salarie": "FULLER", "spectacle": spectacle, "lignes": [{"rubrique": "heure", "quantite": heures, "taux": 19, "dates": ["2026-11-28"]}]});
    let carte = |heures: i64, id: &Value| json!({"mois": "2026-11", "saisie": regie(heures), "engagement": id});

    let id = reussi(&route(&c, "enregistrer-saisie", carte(25, &Value::Null), jour))["engagement"].clone();
    let (_, r) = refus(&route(&c, "valider-engagement", carte(25, &id), jour));
    assert!(r.contains("25 h") && r.contains("24 heures"), "{}", r);

    reussi(&route(&c, "enregistrer-saisie", carte(11, &id), jour));
    let a = route(&c, "apercu", json!({"mois": "2026-11"}), jour);
    let dits: Vec<Value> = reussi(&a)["paies"].as_array().unwrap().iter().filter(|p| p["debut"] == "2026-11-28").flat_map(|p| p["avertissements"].as_array().cloned().unwrap_or_default()).collect();
    assert!(dits.iter().any(|t| t.as_str().is_some_and(|t| t.contains("11 h le 28/11") && t.contains("L. 3121-18"))), "{:?}", dits);
    assert!(reussi(&route(&c, "valider-engagement", carte(11, &id), jour))["valide"].is_string());
}
