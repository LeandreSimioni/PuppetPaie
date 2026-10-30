//! CE QUE LA COMPAGNIE VIRE EST LA SOMME EFFECTIVEMENT RECUE PAR LE SALARIE (C. trav. R. 3243-1, 10°,
//! `atelier/reference/sources/code-du-travail/articles-20261029.md`) : le « MONTANT NET À PAYER » du bulletin, le net a
//! payer moins l'impot preleve a la source (9° du meme article). Le banc des Treteaux, FULLER a un taux DGFiP de 10 % :
//! le net du mois, celui de sa carte et celui de sa ligne dans « Le mois » sont son net apres impot.

use serde_json::{json, Value};

use crate::outils::rejeu::{rejouer_dossier, scenario};
use crate::outils::{dec, dossier_de_test, ecrire, reussi, route, saisie};

#[test]
fn le_net_a_virer_est_le_net_apres_impot() {
    let d = dossier_de_test("net_a_virer", "apres_impot");
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    let gestes = std::fs::read_to_string(scenario("treteaux-du-banc").join("gestes.jsonl")).unwrap();
    let debut: Vec<&str> = gestes.lines().take_while(|l| !l.contains("\"geste\":\"jour\"")).collect();
    std::fs::write(sc.join("gestes.jsonl"), debut.join("\n") + "\n").unwrap();
    let (_, c) = rejouer_dossier(&sc, &d);
    let jour = "2026-11-02";
    ecrire(&c, "config/pas.json", &json!({"salaries": {"FULLER": {
        "taux": "10.00", "type": "01", "identifiant": "190151266", "periode": "202610",
        "mis_a_disposition": "20261015100000", "source": "CRM DGFiP nature 94",
    }}}));

    let s = saisie(&c, "2026-11");
    let r = route(&c, "apercu", json!({"saisie": s}), jour);
    let a = reussi(&r);
    let paies = a["paies"].as_array().unwrap();
    let fuller: &Value = paies.iter().find(|p| p["salarie"]["cle"] == "FULLER").expect("FULLER n'est pas dans l'apercu");
    let avant = dec(fuller["net_a_payer"].as_str().unwrap());
    let impot = (dec(fuller["net_imposable"].as_str().unwrap()) * dec("0.10")).round_dp(2);
    assert!(impot > dec("0"), "aucun impot a prelever : {}", fuller);
    let paye = avant - impot;
    assert_eq!(dec(fuller["net_paye"].as_str().unwrap()), paye, "{}", fuller);
    let ligne = a["par_salarie"].as_array().unwrap().iter().find(|g| g["salarie"]["cle"] == "FULLER").unwrap();
    assert_eq!(dec(ligne["net_paye"].as_str().unwrap()), paye, "{}", ligne);
    let total: rust_decimal::Decimal = paies.iter().map(|p| dec(p["net_paye"].as_str().unwrap())).sum();
    assert_eq!(dec(a["total_net"].as_str().unwrap()), total);
}
