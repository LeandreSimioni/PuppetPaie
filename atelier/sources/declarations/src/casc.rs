//! La CASC-SVP que lit la DSN : les mois regularises hors DSN (sa base livree : `droit::adhesions::casc_svp_base`).

use droit::contexte::Contexte;
use socle::fichiers::lire_json;
use socle::num::Resultat;
use serde_json::Value;

/// Ce que la compagnie dit de ses mois Conges Spectacles regularises hors DSN, sous sa racine.
pub const CASC_SVP: &str = "config/casc-svp.json";

/// `mois_regularises` : les mois de `config/casc-svp.json` regularises hors DSN
/// (un fichier absent ou illisible n'en regularise aucun).
pub fn mois_regularises(ctx: &Contexte) -> Resultat<Vec<String>> {
    let Ok(d) = lire_json(&ctx.racine.join(CASC_SVP)) else {
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
