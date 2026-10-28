//! Les compteurs des documents : la sequence continue des N CE (`prochain_continu`) et le verrou sous lequel un numero se
//! reserve puis s'ecrit (`prendre_le_verrou`).

use serde_json::Value;

use droit::contexte::Contexte;
use socle::echec::erreur;
use socle::echec::Echec;
use socle::echec::R;
use socle::fichiers::lire_json;
use socle::verrou::Verrou;

/// `Contexte.prochain_continu` : une sequence qui ne repart pas a 1 chaque an. La LECTURE balaie toutes les
/// annees de `compteurs.json` et retient le maximum.
pub(crate) fn prochain_continu(ctx: &Contexte, cle: &str, defaut: i64) -> R<i64> {
    let p = ctx.racine.join("config").join("compteurs.json");
    let mut vu = defaut - 1;
    if p.exists() {
        let c = lire_json(&p).map_err(|m| Echec::from(m))?;
        if let Value::Object(annees) = &c {
            for valeurs in annees.values() {
                if let Some(v) = valeurs.as_object().and_then(|o| o.get(cle)) {
                    let n = match v {
                        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
                        Value::String(s) => s.trim().parse().ok(),
                        Value::Bool(b) => Some(*b as i64),
                        _ => None,
                    }
                    .ok_or_else(|| erreur("ValueError", format!("nombre entier illisible : {}", socle::fichiers::repr_valeur(v))))?;
                    vu = vu.max(n);
                }
            }
        }
    }
    Ok(vu + 1)
}

/// `verrou_compteurs(ctx, qui=...)` : refuse si un autre tient les compteurs.
pub(crate) fn prendre_le_verrou(ctx: &Contexte, qui: &str) -> R<Verrou> {
    let chemin = socle::verrou::chemin_compteurs(&ctx.racine);
    let maintenant = socle::paris::iso_secondes(socle::temps::secondes()).replace('T', " ");
    let marque = socle::verrou::marque(qui, std::process::id(), &maintenant);
    match Verrou::prendre(&chemin, &marque, socle::verrou::DELAI_VERROU).map_err(|m| erreur("OSError", m))? {
        Ok(v) => Ok(v),
        Err(v) => Err(erreur("CompteursVerrouilles", v.message())),
    }
}

