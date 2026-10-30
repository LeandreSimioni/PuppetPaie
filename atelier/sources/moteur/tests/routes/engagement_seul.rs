//! UN ENGAGEMENT ENVOYE SEUL, A COTE D'UN ENGAGEMENT VALIDE DU MEME SALARIE : `enregistrer-saisie` recoit le mois que
//! l'ecran montre ; un appel qui n'envoie qu'un nouvel engagement (la ligne de commande) ne le perd pas en l'appariant
//! au contrat valide du meme salarie, qui ne change pas par ce chemin. Le nouveau s'ecrit, le valide reste tel quel.

use serde_json::{json, Value};

use crate::outils::rejeu::{rejouer_dossier, scenario};
use crate::outils::{dossier_de_test, reussi, route, saisie};

#[test]
fn un_nouvel_engagement_envoye_seul_ne_se_perd_pas_sur_le_valide() {
    let d = dossier_de_test("engagement_seul", "a_cote_du_valide");
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    let gestes = std::fs::read_to_string(scenario("treteaux-du-banc").join("gestes.jsonl")).unwrap();
    let debut: Vec<&str> = gestes.lines().take_while(|l| !l.contains("\"geste\":\"jour\"")).collect();
    std::fs::write(sc.join("gestes.jsonl"), debut.join("\n") + "\n").unwrap();
    let (_, c) = rejouer_dossier(&sc, &d);
    let jour = "2026-11-02";

    // Le contrat de FULLER, valide.
    let s = saisie(&c, "2026-11");
    let fuller: Value = s["engagements"].as_array().unwrap().iter().find(|e| e["salarie"] == "FULLER").cloned().unwrap();
    let debut = fuller["debut"].as_str().map(str::to_string).unwrap_or_else(|| engagement::debut_brut(&fuller));
    let v = route(&c, "valider-engagement", json!({"mois": "2026-11", "salarie": "FULLER", "debut": debut, "numero": "", "rang": 0, "valide": true}), jour);
    assert!(reussi(&v)["valide"].is_string(), "le contrat de FULLER ne se valide pas : {}", v);

    // Un cachet de plus pour FULLER, un autre jour, envoye seul.
    let nouveau = json!({"salarie": "FULLER", "spectacle": fuller["spectacle"], "lignes": [{"rubrique": "cachet", "quantite": 1, "taux": 150, "dates": ["2026-11-28"]}]});
    reussi(&route(&c, "enregistrer-saisie", json!({"mois": "2026-11", "saisie": {"engagements": [nouveau]}}), jour));

    let apres = saisie(&c, "2026-11");
    let siens: Vec<&Value> = apres["engagements"].as_array().unwrap().iter().filter(|e| e["salarie"] == "FULLER").collect();
    assert!(siens.iter().any(|e| e["valide"].is_object()), "le contrat validé de FULLER a disparu : {:?}", siens);
    assert!(
        siens.iter().any(|e| e["valide"].is_null() && e["lignes"][0]["dates"] == json!(["2026-11-28"])),
        "le nouvel engagement de FULLER s'est perdu : {:?}",
        siens
    );
}
