//! LA CASE D'UN FICHIER DEJA ECRIT OUVRE SON DEPOT : une fois la DSN du mois ecrite, la case DSN du panneau
//! (`etat-du-mois`) rend le geste `deposer_fichier`, et le fichier qu'il nomme est celui que le recapitulatif du depot
//! (`depot-apercu`) ouvre.

use serde_json::json;

use crate::outils::rejeu::{rejouer_dossier, scenario};
use crate::outils::{dossier_de_test, reussi, route};

#[test]
fn la_case_de_la_dsn_ecrite_ouvre_son_recapitulatif() {
    let d = dossier_de_test("case_de_depot", "dsn_ecrite");
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    let gestes = std::fs::read_to_string(scenario("treteaux-du-banc").join("gestes.jsonl")).unwrap();
    let debut: Vec<&str> = gestes.lines().take_while(|l| !l.contains("\"geste\":\"jour\"")).collect();
    std::fs::write(sc.join("gestes.jsonl"), debut.join("\n") + "\n").unwrap();
    let (_, c) = rejouer_dossier(&sc, &d);
    let jour = "2026-12-01";
    reussi(&route(&c, "valider-engagement", json!({"mois": "2026-11", "salarie": "FULLER", "debut": "2026-11-20"}), jour));
    reussi(&route(&c, "dsn-a-deposer", json!({"mois": "2026-11"}), jour));

    let e = route(&c, "etat-du-mois", json!({"mois": "2026-11"}), jour);
    let dsn = reussi(&e)["etapes"].as_array().unwrap().iter().find(|x| x["cle"] == "dsn").unwrap().clone();
    let action = dsn["groupes"].as_array().unwrap().iter().map(|g| &g["action"]).find(|a| a["geste"] == "deposer_fichier").cloned();
    let action = action.unwrap_or_else(|| panic!("la DSN écrite n'a pas de geste de dépôt : {}", dsn));
    reussi(&route(&c, "depot-apercu", json!({"quoi": action["quoi"], "fichier": action["fichier"]}), jour));
}
