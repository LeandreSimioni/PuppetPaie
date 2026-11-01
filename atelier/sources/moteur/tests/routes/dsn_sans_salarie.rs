//! UN MOIS SANS ENGAGEMENT A SA DSN, ET L'HORLOGE DE LA COMPAGNIE FICTIVE LA DEPOSE A SON ECHEANCE. « La DSN est
//! mensuelle et produite par établissement employeur. [...] A défaut, une DSN Sans individu devra être produite » (cahier
//! technique DSN 2026.1, § 2.4, `atelier/reference/norme-dsn/texte/chapitre-2-principes-de-constitution.txt`
//! l. 2291-2309). Les Faisceaux : CRAIG joue les 29 et 30 janvier 2026, personne en fevrier ; au 20 mars, l'horloge a
//! depose la DSN de fevrier, sans individu.

use serde_json::json;

use crate::outils::rejeu::{rejouer_dossier, scenario};
use crate::outils::{dossier_de_test, reussi, route};

#[test]
fn l_horloge_depose_la_dsn_d_un_mois_sans_engagement() {
    let d = dossier_de_test("dsn_sans_salarie", "fevrier");
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    let gestes = std::fs::read_to_string(scenario("faisceaux-sans-civilite").join("gestes.jsonl")).unwrap();
    let gestes: Vec<String> = gestes
        .lines()
        .take_while(|l| !l.contains("\"enregistrer-saisie\""))
        .map(|l| l.replacen("\"declarant_dsn\":{", "\"declarant_dsn\":{\"civilite\":\"01\",", 1))
        .map(|l| if l.contains("\"cle\":\"CRAIG\"") { l.replacen("\"date_anciennete\":\"2026-02-02\"", "\"date_anciennete\":\"2026-01-29\"", 1) } else { l })
        .collect();
    std::fs::write(sc.join("gestes.jsonl"), gestes.join("\n") + "\n").unwrap();
    let (_, c) = rejouer_dossier(&sc, &d);
    reussi(&route(
        &c,
        "valider-engagement",
        json!({"mois": "2026-01", "saisie": {"salarie": "CRAIG", "spectacle": "nuits-d-ete",
            "lignes": [{"rubrique": "cachet", "quantite": 2, "taux": 150, "dates": ["2026-01-29", "2026-01-30"]}]}}),
        "2026-01-05",
    ));
    let r = route(&c, "horloge-rattraper", json!({"aujourdhui": "2026-03-20"}), "2026-03-20");
    let fait: Vec<String> = reussi(&r).as_array().unwrap().iter().map(|p| p.as_str().unwrap_or_default().to_string()).collect();
    assert!(fait.iter().any(|p| p.contains("DSN de février 2026 déposée") && !p.contains("non joué")), "{:?}", fait);
    let dsn = std::fs::read_to_string(c.join("sorties/2026-02/DSN_Fevrier2026.dsn")).unwrap_or_else(|e| panic!("{:?} : {}", fait, e));
    assert!(dsn.contains("S20.G00.05.002,'02'"), "la DSN de fevrier n'est pas sans individu");
}
