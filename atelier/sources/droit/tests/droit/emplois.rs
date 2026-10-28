//! Les emplois : `resoudre`, `metier` et `statut_non_cadre` (`emplois.json`) sur les codes de la fictive, du catalogue et
//! de la nomenclature, et un code inconnu ; contre les
//! oracles, que `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit.

use crate::outils::{lire_oracle, oracle};
use droit::emplois::{metier, resoudre, statut_non_cadre};
use serde_json::{json, Value};

#[test]
fn resoudre_metier_et_statut_non_cadre() {
    let mut cas = lire_oracle("emplois.json");
    let (mut resolus, mut refus) = (0, 0);
    for e in cas.as_array_mut().unwrap() {
        let code = e["code"].as_str().unwrap().to_string();
        let m = metier(&code).unwrap();
        if e.get("non_cadre").is_some() {
            e["non_cadre"] = statut_non_cadre(m.as_ref().unwrap()).map_or(Value::Null, Value::String);
        }
        // Un code de la seule nomenclature : l'oracle ne garde que son statut de non-cadre.
        if !(e["metier"].is_null() && e.get("non_cadre").is_some()) {
            e["metier"] = m.unwrap_or(Value::Null);
        }
        for (cle, voulu) in e["resolu"].as_object_mut().unwrap() {
            let (cadre, fem) = cle.split_once('|').unwrap();
            let cadre = match cadre {
                "None" => None,
                "True" => Some(true),
                _ => Some(false),
            };
            *voulu = resoudre(&code, cadre, fem == "True").unwrap_or_else(|m| {
                refus += 1;
                json!({ "refus": m })
            });
            resolus += 1;
        }
    }
    oracle("emplois.json", "", &cas);
    assert!(resolus - refus > 700, "{}", resolus);
    assert_eq!(refus, 8); // XXX999 et le code vide : quatre variantes chacun
}
