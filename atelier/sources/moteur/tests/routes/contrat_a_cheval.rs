//! UN CONTRAT D'USAGE A CHEVAL SUR DEUX MOIS EST UNE SEULE EMBAUCHE : sa DPAE et son contrat se font a son premier jour
//! (code du travail, L. 1221-10 et R. 1221-1, 3°, `atelier/reference/sources/code-du-travail/xml/`), et la DSN de son
//! second mois le declare depuis ce jour-la (S21.G00.40.001, `norme-dsn/texte/bloc-S21.G00.40.txt`). Le banc, FULLER
//! engagee les 20 et 21/11 avec deux jours planifies les 02 et 03/12, vit jusqu'au 16/12 : decembre ne refait ni DPAE ni
//! contrat, et sa DSN dit le contrat du 20/11.

use serde_json::json;

use crate::outils::rejeu::{rejouer_dossier, scenario};
use crate::outils::{dossier_de_test, noms, reussi, route};

#[test]
fn le_second_mois_d_un_contrat_d_usage_n_est_pas_une_embauche() {
    let d = dossier_de_test("contrat_a_cheval", "second_mois");
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    let gestes = std::fs::read_to_string(scenario("treteaux-du-banc").join("gestes.jsonl")).unwrap();
    let novembre = r#""dates":["2026-11-20","2026-11-21"]}"#;
    assert!(gestes.contains(novembre));
    let gestes = gestes.replace(novembre, r#""dates":["2026-11-20","2026-11-21"],"planning":["2026-12-02","2026-12-03"]}"#);
    std::fs::write(sc.join("gestes.jsonl"), gestes).unwrap();
    let (_, c) = rejouer_dossier(&sc, &d);

    let dpae: Vec<String> = noms(&c.join("retours/depots")).into_iter().filter(|n| n.contains("depot-dpae")).collect();
    assert!(dpae.iter().all(|n| n.contains("_202611")), "une DPAE est repartie en decembre : {:?}", dpae);
    assert!(!c.join("sorties/2026-12/envoi").exists(), "un contrat est refait en decembre : {:?}", noms(&c.join("sorties/2026-12/envoi")));
    let jour = "2026-12-20";
    assert_eq!(reussi(&route(&c, "embauches-a-declarer", json!({}), jour))["embauches"], json!([]));

    let ecrite = reussi(&route(&c, "dsn-a-deposer", json!({"mois": "2026-12"}), jour)).clone();
    let decembre = String::from_utf8_lossy(&std::fs::read(ecrite["chemin"].as_str().unwrap()).unwrap()).to_string();
    for attendu in ["S21.G00.40.001,'20112026'", "S21.G00.40.009,'FU1.26.01'", "S21.G00.40.010,'03122026'", "S21.G00.40.013,'4.00'"] {
        assert!(decembre.contains(attendu), "{} manque :\n{}", attendu, decembre);
    }
}
