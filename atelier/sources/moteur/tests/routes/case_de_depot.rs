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

/// UNE DPAE QUI MANQUE D'UNE CASE REFUSE ET LE DIT, MEME QUAND UNE DPAE DU MOIS EST DEJA PARTIE : le depot ne reprend la
/// DPAE partie que quand rien ne reste a declarer. (Un salarie sans NIR ni sexe : la DPAE refusait en silence et l'ancien
/// fichier repartait a sa place.)
#[test]
fn une_dpae_a_qui_il_manque_une_case_refuse_au_lieu_de_reprendre_la_partie() {
    let d = dossier_de_test("case_de_depot", "dpae_manque");
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    let gestes = std::fs::read_to_string(scenario("treteaux-du-banc").join("gestes.jsonl")).unwrap();
    let debut: Vec<&str> = gestes.lines().take_while(|l| !l.contains("\"geste\":\"jour\"")).collect();
    std::fs::write(sc.join("gestes.jsonl"), debut.join("\n") + "\n").unwrap();
    let (_, c) = rejouer_dossier(&sc, &d);
    let jour = "2026-11-20";
    reussi(&valider_la_carte(&c, "2026-11", "FULLER", jour));
    reussi(&route(&c, "deposer", json!({"quoi": "dpae", "mois": "2026-11", "reel": false}), jour));
    let lignes = || -> usize {
        let j: serde_json::Value = serde_json::from_slice(&std::fs::read(c.join("journal/depots.json")).unwrap()).unwrap();
        j.as_array().or_else(|| j["lignes"].as_array()).map_or(0, |l| l.iter().filter(|x| x["quoi"] == "dpae").count())
    };
    let avant = lignes();

    let chemin = c.join("config/salaries.json");
    let mut s: serde_json::Value = serde_json::from_slice(&std::fs::read(&chemin).unwrap()).unwrap();
    s["FULLER"].as_object_mut().unwrap().remove("date_naissance");
    std::fs::write(&chemin, serde_json::to_vec_pretty(&s).unwrap()).unwrap();

    let r = route(&c, "deposer", json!({"quoi": "dpae", "mois": "2026-11", "reel": false}), jour);
    assert_eq!(r["ok"], json!(false), "la DPAE est repartie sans sa case : {}", r);
    assert_eq!(lignes(), avant, "rien ne part d'un refus");
}
