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
    // La fiche dit un choix (`droit::fiche::JoursFeriesChomes`) : « tous », « 1er mai seulement », ou rien.
    cas.as_array_mut().unwrap().retain(|c| c[0].is_string());
    for c in cas.as_array_mut().unwrap() {
        let dits = c[0].as_str().filter(|d| *d != "<absent>");
        c[3] = jours_ouvrables(&jour(c[1].as_str().unwrap()), &jour(c[2].as_str().unwrap()), dits).map_or_else(err, |n| json!(n));
    }
    assert!(!cas.as_array().unwrap().is_empty());
    oracle("conges.json", "/jours_ouvrables", &cas);
}
