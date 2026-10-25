//! `calculer` contre des valeurs payees (§7.11 de PORTAGE-RUST) : des bulletins reels, rejoues sur une compagnie de
//! fictive ; un attendu independant du moteur. Le taux AT/MP et le parametrage de la reduction generale de la fictive
//! ne sont pas ceux de ces bulletins : les totaux patronaux, le cout et le coefficient de la RGDU ne s'y comparent pas,
//! le reste (assiettes, salarie, net, SMIC retenu) si.

use paie::calcul::Paie;
use droit::contexte::Contexte;
use socle::date::d;
use paie::engagement::charger;
use serde_json::{json, Value};
use crate::outils::{dec, fictive};

/// Une paie d'un seul engagement : `mois`, salarie, spectacle, jours, lignes.
fn paie(racine: &str, mois: &str, salarie: &str, extra: Value, jours: &[&str], lignes: Value) -> Paie {
    let mut e = json!({
        "salarie": salarie, "spectacle": "ubu-roi",
        "debut": jours[0], "fin": jours[jours.len() - 1], "date_paiement": jours[jours.len() - 1],
        "lignes": lignes,
    });
    for (k, v) in extra.as_object().into_iter().flatten() {
        e[k] = v.clone();
    }
    let saisie = json!({"mois": mois, "taux_cachet": 130.0, "engagements": [e]});
    let chemin = crate::outils::chemin_unique("calculer.json");
    std::fs::write(&chemin, saisie.to_string()).unwrap();
    let ctx = Contexte::nouveau(&fictive(racine), Some(d(&json!(format!("{}-28", mois))).unwrap())).unwrap();
    let mut s = charger(&ctx, &chemin).unwrap();
    s.paies.remove(0)
}

fn cachets(n: usize, taux: f64, jours: &[&str]) -> Value {
    json!([{"rubrique": "cachet", "quantite": n, "taux": taux, "dates": &jours[..n]}])
}

const J: [&str; 2] = ["2026-06-10", "2026-06-11"];

#[test]
fn favart_deux_cachets_a_130() {
    let p = paie("fictive-3090-petite", "2026-06", "Brioche", json!({}), &J, cachets(2, 130.0, &J));
    assert_eq!(p.brut, dec("260"));
    assert_eq!(p.base_csg, dec("258.34"));
    assert_eq!(p.total_sal, dec("58.68"));
    assert_eq!(p.net_a_payer, dec("201.32"));
    assert_eq!(p.net_imposable, dec("208.81"));
    assert_eq!(p.plafond, dec("172.34"));
    assert_eq!(p.ligne("ccn").unwrap().montant_pat, dec("1.30"));
    assert_eq!(p.ligne("csg_ded").unwrap().montant_sal, dec("17.57"));
}

#[test]
fn bulletin_27_cachet_et_heures_arrondis_par_rubrique() {
    let jours = ["2026-06-09", "2026-06-10", "2026-06-11", "2026-06-12"];
    let lignes = json!([
        {"rubrique": "cachet", "quantite": 1, "taux": 130.0, "dates": [jours[0]]},
        {"rubrique": "heure", "quantite": 12, "taux": 20.0, "dates": &jours[1..]},
    ]);
    let p = paie("fictive-3090-petite", "2026-06", "Brioche", json!({}), &jours, lignes);
    assert_eq!(p.brut, dec("370"));
    assert_eq!(p.base_csg, dec("367.63"));
    assert_eq!(p.total_sal, dec("83.50"));
    assert_eq!(p.net_a_payer, dec("286.50"));
}

#[test]
fn smic_retenu_fevrier_et_juillet() {
    let f = ["2026-02-10"];
    let p = paie("fictive-3090-petite", "2026-02", "Brioche", json!({}), &f, cachets(1, 130.0, &f));
    assert_eq!(p.plafond, dec("84.14"));
    let j = ["2026-07-10"];
    let p = paie("fictive-3090-petite", "2026-07", "Brioche", json!({}), &j, cachets(1, 130.0, &j));
    assert_eq!(p.plafond, dec("86.17"));
}

#[test]
fn artiste_cadre_a_250() {
    let j = ["2026-06-10"];
    let p = paie("fictive-3090-petite", "2026-06", "Brioche", json!({"statut_categoriel_rc": "01"}), &j, cachets(1, 250.0, &j));
    assert_eq!(p.brut, dec("250"));
    assert_eq!(p.base_csg, dec("249.34"));
    assert_eq!(p.total_sal, dec("57.10"));
    assert_eq!(p.net_a_payer, dec("192.90"));
    assert_eq!(p.net_imposable, dec("200.13"));
    assert!(p.reduction_generale.is_none());
}

#[test]
fn cadre_sous_la_t1_sans_t2_ni_cet() {
    let j = ["2026-06-10", "2026-06-11"];
    let p = paie("fictive-3090-petite", "2026-06", "Brioche", json!({"statut_categoriel_rc": "01"}), &j, cachets(2, 160.0, &j));
    assert!(p.ligne("retraite_t2").is_none() && p.ligne("apec_t2").is_none());
    assert_eq!(p.base_csg, dec("319.81"));
    let t1 = p.ligne("retraite_t1").unwrap();
    assert_eq!((t1.montant_pat, t1.montant_sal), (dec("16.74"), dec("15.33")));
    let apec = p.ligne("apec_t1").unwrap();
    assert_eq!((apec.montant_pat, apec.montant_sal), (dec("0.12"), dec("0.08")));
}

#[test]
fn technicien_une_journee_de_7_h_a_100() {
    let j = ["2026-06-10"];
    let lignes = json!([{"rubrique": "heure", "quantite": 7, "taux": 100.0/7.0, "dates": j}]);
    let p = paie("fictive-3090-petite", "2026-06", "Lemercier", json!({}), &j, lignes);
    // 7 h a 100/7 : le brut est arrondi ligne par ligne, 100,00.
    assert_eq!(p.brut, dec("100"));
    assert_eq!(p.base_csg, dec("99.36"));
    assert_eq!(p.total_sal, dec("24.25"));
    assert_eq!(p.net_a_payer, dec("75.75"));
    assert_eq!(p.net_imposable, dec("78.63"));
    let rg = p.reduction_generale.as_ref().unwrap();
    assert_eq!(rg.smic_reference, dec("84.14"));
    assert_eq!(p.ligne("reduction_generale").unwrap().montant_pat, -rg.montant);
}

#[test]
fn technicien_quatre_heures_a_14() {
    let j = ["2026-06-10"];
    let lignes = json!([{"rubrique": "heure", "quantite": 4, "taux": 14.0, "dates": j}]);
    let p = paie("fictive-3090-petite", "2026-06", "Lemercier", json!({}), &j, lignes);
    let rg = p.reduction_generale.as_ref().unwrap();
    assert_eq!(rg.smic_reference, dec("48.08"));
    assert_eq!(p.ligne("maladie").unwrap().montant_pat, dec("7.28"));
}
