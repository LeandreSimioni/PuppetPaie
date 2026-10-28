//! `regularisation-apercu` refuse un mois illisible, sur une compagnie neuve (`compagnies::ajouter_dans`).

use crate::outils::{dossier_de_test, route};
use paie::interface::compagnies::ajouter_dans;
use serde_json::json;

#[test]
fn un_apercu_d_un_mois_illisible_refuse() {
    let racine = ajouter_dans(&dossier_de_test("routes_regularisations", "apercu-mois"), "Les Tréteaux Neufs").unwrap().racine;
    let r = route(&racine, "regularisation-apercu", json!({"mois": "abc", "salarie": "B", "debut": "d", "engagement": {}}), "2026-10-03");
    assert_eq!(r["ok"], json!(false), "{}", r);
}
