//! Les formats du bulletin contre `bulletin.json`. `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit l'oracle.

use crate::outils::{lire_oracle, oracle};
use documents::bulletin::{fmt_eur, fmt_heures, fmt_taux, jj_mm_aaaa, jours};
use serde_json::json;
use socle::num::{dec, Decimal};

#[test]
fn les_formats() {
    let mut f = lire_oracle("bulletin.json")["formats"].clone();
    let d = |s: &serde_json::Value| dec(s).unwrap();
    for (cle, fonction) in [("eur", fmt_eur as fn(Decimal) -> String), ("taux", fmt_taux), ("heures", fmt_heures), ("jours", jours)] {
        assert!(f[cle].as_array().unwrap().len() > 10);
        for c in f[cle].as_array_mut().unwrap() {
            c[1] = json!(fonction(d(&c[0])));
        }
    }
    for c in f["dates"].as_array_mut().unwrap() {
        c[1] = json!(jj_mm_aaaa(&c[0]).unwrap());
    }
    oracle("bulletin.json", "/formats", &f);
}
