//! Les jours ouvrables de `conges` contre `conges.json`, selon les feries chomes. `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit l'oracle.

use crate::outils::{lire_oracle, oracle};
use droit::conges::jours_ouvrables;
use serde_json::{json, Value};
use socle::date::Jour;

fn jour(s: &str) -> Jour {
    Jour::lire(s).unwrap()
}

fn err(m: String) -> Value {
    json!({"err": m})
}

#[test]
fn jours_ouvrables_selon_les_feries_chomes() {
    let mut cas = lire_oracle("conges.json")["jours_ouvrables"].clone();
    for c in cas.as_array_mut().unwrap() {
        let ent = if c[0] == "<absent>" { None } else { Some(json!({"jours_feries_chomes": c[0]})) };
        c[3] = jours_ouvrables(&jour(c[1].as_str().unwrap()), &jour(c[2].as_str().unwrap()), ent.as_ref()).map_or_else(err, |n| json!(n));
    }
    assert!(!cas.as_array().unwrap().is_empty());
    oracle("conges.json", "/jours_ouvrables", &cas);
}
