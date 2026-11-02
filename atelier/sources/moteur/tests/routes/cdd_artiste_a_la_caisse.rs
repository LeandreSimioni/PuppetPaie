//! L'ARTISTE EN CDD DECLARE A LA CAISSE DES CONGES SPECTACLES N'A PAS D'INDEMNITE COMPENSATRICE DE L'EMPLOYEUR : « Les
//! articles L. 3141-28 et L. 3141-29 ne sont pas applicables lorsque l'employeur est tenu d'adherer a une caisse de
//! conges » (C. trav. L. 3141-30, `atelier/reference/sources/code-du-travail/articles-20261102.md` ; la caisse « assure
//! le service des conges », D. 7121-38). Le dernier mois d'un CDD au mois de moins de douze mois d'un artiste se calcule,
//! sans ligne `indemnite_compensatrice_conges`.

use serde_json::json;

use crate::outils::rejeu::{rejouer_dossier, scenario};
use crate::outils::{reussi, route};

#[test]
fn le_dernier_mois_n_a_pas_d_indemnite_compensatrice() {
    let d = crate::outils::dossier_de_test("cdd_artiste_a_la_caisse", "dernier_mois");
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    // Les Faisceaux jusqu'a la carte de janvier.
    let gestes = std::fs::read_to_string(scenario("faisceaux-sans-civilite").join("gestes.jsonl")).unwrap();
    let gestes: Vec<&str> = gestes.lines().take_while(|l| !l.contains("\"geste\":\"jour\"")).collect();
    std::fs::write(sc.join("gestes.jsonl"), gestes.join("\n") + "\n").unwrap();
    let (_, c) = rejouer_dossier(&sc, &d);

    // FULLER engagee comme artiste choregraphique (annexe 10), en CDD de droit commun de trois mois.
    let saisie = json!({"salarie": "FULLER", "nature": "cdd", "debut": "2026-03-02", "fin": "2026-05-29", "salaire_mensuel": 2500,
        "quotite_contrat": 151.67, "metier": "ART040", "motif_recours": "02"});
    reussi(&route(&c, "valider-engagement", json!({"mois": "2026-03", "saisie": saisie}), "2026-02-25"));
    let ap = route(&c, "apercu", json!({"mois": "2026-05"}), "2026-05-20");
    let paies = reussi(&ap)["paies"].as_array().unwrap().clone();
    let fuller = paies.iter().find(|p| p.to_string().contains("FULLER")).expect("la paie de mai de FULLER");
    assert!(!fuller.to_string().contains("indemnite_compensatrice_conges"), "une indemnite compensatrice : {}", fuller);
}
