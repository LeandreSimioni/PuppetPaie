use serde_json::json;

use crate::outils::rejeu::{rejouer_dossier, scenario};
use crate::outils::{dossier_de_test, reussi, route, valider_la_carte};

/// LA DEMANDE DE TAUX PART AVEC LA DPAE : deposee en test (route `deposer`, la DPAE du mois designee), la DPAE d'une
/// embauche sans taux de PAS emporte, dans le meme geste et le meme mode, le signalement d'amorcage (nature 08, motif 01) que sa
/// production a ecrit.
#[test]
fn la_dpae_deposee_emporte_la_demande_de_taux() {
    let d = dossier_de_test("case_de_depot", "dpae_amorcage");
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    let gestes = std::fs::read_to_string(scenario("treteaux-du-banc").join("gestes.jsonl")).unwrap();
    let debut: Vec<&str> = gestes.lines().take_while(|l| !l.contains("\"geste\":\"jour\"")).collect();
    std::fs::write(sc.join("gestes.jsonl"), debut.join("\n") + "\n").unwrap();
    let (_, c) = rejouer_dossier(&sc, &d);
    let jour = "2026-11-20";
    reussi(&valider_la_carte(&c, "2026-11", "FULLER", jour));

    reussi(&route(&c, "deposer", json!({"quoi": "dpae", "mois": "2026-11", "reel": false}), jour));

    let journal: serde_json::Value = serde_json::from_slice(&std::fs::read(c.join("journal/depots.json")).unwrap()).unwrap();
    let lignes = journal.as_array().or_else(|| journal["lignes"].as_array()).cloned().unwrap_or_default();
    let parti = lignes.iter().any(|l| l["quoi"] == "dsn" && l["mode"] == "test" && l["fichier"] == "Amorcage01_FULLER_20261120.dsn");
    assert!(parti, "le signalement d'amorcage n'est pas parti avec la DPAE : {:?}", lignes.iter().map(|l| (&l["quoi"], &l["mode"], &l["fichier"])).collect::<Vec<_>>());
}
