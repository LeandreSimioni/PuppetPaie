//! Le `Contexte` des quatre compagnies fictives contre `contexte.json` : SMIC, licence, PSS et l'empreinte du bareme resolu a
//! quatre dates (le bareme lui-meme est prouve par `bareme.rs`), la recherche des fiches et des spectacles, refus
//! compris. `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit l'oracle.

use crate::outils::{fictive, egal, lire_oracle, oracle_selon, sceller};
use droit::contexte::Contexte;
use socle::date::d;
use serde_json::json;

#[test]
fn le_contexte_des_fictives() {
    let mut cas = lire_oracle("contexte.json");
    for c in cas.as_array_mut().unwrap() {
        let ctx = Contexte::nouveau(&fictive(c["racine"].as_str().unwrap()), Some(d(&json!("2026-09-27")).unwrap())).unwrap();
        let o = c.as_object_mut().unwrap();
        let r = if let Some(j) = o.get("jour") {
            let jour = d(j).unwrap();
            let (m, h) = ctx.plafond_securite_sociale(jour.an).unwrap();
            o.remove("bareme");
            json!({"empreinte_bareme": sceller(&ctx.bareme(&jour).unwrap()), "smic": ctx.smic(&jour).unwrap(),
                   "licence": ctx.licence(&jour).unwrap(), "pss": [m, h]})
        } else {
            let r = match o.get("salarie") {
                Some(n) => ctx.salarie(n.as_str().unwrap()),
                None => ctx.spectacle(o["spectacle"].as_str().unwrap()),
            };
            o.remove("ok");
            o.remove("erreur");
            r.map_or_else(|e| json!({"erreur": e}), |v| json!({"ok": v["_cle"]}))
        };
        o.extend(r.as_object().unwrap().clone());
    }
    oracle_selon("contexte.json", "", &cas, egal);
}

