//! UN ARRET SE PROLONGE, PUIS LE SALARIE REPREND PLUS TOT : la page lit le genre de chaque imprevu (`imprevus`) pour lui
//! offrir ses gestes, et un arret se cherche par ce genre pour etre prolonge. La prolongation ne change que la fin
//! prescrite (fiche net-entreprises 1983 : « il suffit de declarer la nouvelle date de fin de l'arret en DSN mensuelle »,
//! `atelier/reference/sources/net-entreprises-base-connaissances/net-entreprises-fiche1983-prolongation-arret-dates-de-subrogation-20250322.txt`) ;
//! la reprise anticipee se saisit ensuite sur le meme arret. Un accident du travail fait naitre sa declaration, dans les
//! quarante-huit heures (R. 441-3, `atelier/reference/sources/code-de-la-securite-sociale/articles-20261029.md`), et le
//! bulletin nomme l'absence d'apres le motif (S21.G00.60.001, `atelier/reference/norme-dsn/texte/bloc-S21.G00.60.txt`).

use serde_json::{json, Value};

use crate::outils::rejeu::{rejouer_dossier, scenario};
use crate::outils::{dossier_de_test, reussi, route};

#[test]
fn un_accident_du_travail_se_prolonge_puis_se_reprend_plus_tot() {
    let d = dossier_de_test("arret_prolonge", "accident_du_travail");
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    // Les Faisceaux : SVOBODA, en CDI depuis le 05/01/2026, payee au mois.
    std::fs::copy(scenario("faisceaux-sans-civilite").join("gestes.jsonl"), sc.join("gestes.jsonl")).unwrap();
    let (_, c) = rejouer_dossier(&sc, &d);
    let jour = "2026-02-20";

    reussi(&route(
        &c,
        "arret-enregistrer",
        json!({"salarie": "SVOBODA", "dernier_jour": "2026-02-10", "fin": "2026-02-15", "motif": "06", "subrogation": "02",
               "date_accident": "2026-02-10"}),
        jour,
    ));
    let l_arret = || -> Value {
        let r = reussi(&route(&c, "imprevus", json!({"mois": "2026-02", "salarie": "SVOBODA"}), jour)).clone();
        r["imprevus"].as_array().unwrap().iter().find(|i| i["dernier_jour_travaille"] == "2026-02-10").cloned().unwrap_or(r)
    };
    let a = l_arret();
    assert_eq!(a["genre"], json!("arret"), "{}", a);
    let declaration = a["obligations"].as_array().unwrap().iter().find(|o| o["cle"] == "declaration_accident").cloned();
    let declaration = declaration.unwrap_or_else(|| panic!("pas de déclaration d'accident : {}", a["obligations"]));
    assert_eq!(declaration["du"], json!(true), "{}", declaration);
    // Le mardi 10/02, connu des minuit : 48 heures, sans dimanche ni jour ferie, menent au mercredi 11/02 a minuit.
    assert_eq!(declaration["butoir"], json!("2026-02-11"), "{}", declaration);

    reussi(&route(&c, "arret-prolonger", json!({"salarie": "SVOBODA", "dernier_jour": "2026-02-10", "fin": "2026-02-22"}), jour));
    assert_eq!(l_arret()["fin_previsionnelle"], json!("2026-02-22"));
    reussi(&route(&c, "arret-reprise", json!({"salarie": "SVOBODA", "dernier_jour": "2026-02-10", "reprise": "2026-02-19"}), jour));
    let a = l_arret();
    assert_eq!((&a["fin_previsionnelle"], &a["reprise"]), (&json!("2026-02-22"), &json!("2026-02-19")), "{}", a);

    // Le bulletin de fevrier retire les jours d'arret sous le nom de l'accident du travail.
    let saisie = reussi(&route(&c, "saisie", json!({"mois": "2026-02"}), jour))["saisie"].clone();
    let apercu = reussi(&route(&c, "apercu", json!({"saisie": saisie, "lignes": true}), jour)).to_string().to_lowercase();
    assert!(apercu.contains("absence accident du travail"), "{}", apercu);
    assert!(!apercu.contains("absence maladie"), "{}", apercu);
}
