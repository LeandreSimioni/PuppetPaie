//! La CASC-SVP que lit la DSN : sa base livree, et les mois regularises hors DSN.

use droit::contexte::Contexte;
use socle::fichiers::{dossier_du_code, lire_json};
use socle::num::Resultat;
use serde_json::Value;

/// `_base` : `casc-svp-base.json`, livre avec le code.
pub fn base() -> Resultat<Value> {
    lire_json(&dossier_du_code().join("casc-svp-base.json"))
}

/// `mois_regularises` : les mois de `config/casc-svp.json` regularises hors DSN
/// (un fichier absent ou illisible n'en regularise aucun).
pub fn mois_regularises(ctx: &Contexte) -> Resultat<Vec<String>> {
    let Ok(d) = lire_json(&ctx.racine.join("config").join("casc-svp.json")) else {
        return Ok(vec![]);
    };
    let mut mois: Vec<String> = vec![];
    for reg in d.get("regularisations").and_then(Value::as_array).into_iter().flatten() {
        for m in reg.get("mois").and_then(Value::as_array).into_iter().flatten() {
            let m = socle::date::en_texte(m);
            if !mois.contains(&m) {
                mois.push(m);
            }
        }
    }
    mois.sort();
    Ok(mois)
}
