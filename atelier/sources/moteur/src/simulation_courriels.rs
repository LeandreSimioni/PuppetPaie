//! La route `courriels-fictifs` : les courriels joues de la compagnie fictive (`envois::simulation_courriels`), que la
//! page montre. Une compagnie qui n'est pas fictive n'en a pas : la route refuse.

use serde_json::Value;

use droit::contexte::Contexte;
use socle::echec::{erreur, R};

pub fn courriels_fictifs(ctx: &Contexte, charge: &Value) -> R<Value> {
    if !crate::compagnie::bridage::branchement(&ctx.racine).joue() {
        return Err(erreur("ErreurOperation", "Seulement dans la compagnie fictive."));
    }
    let apres = charge.get("apres").filter(|v| socle::fichiers::vrai(v)).map(socle::valeur::valeur_en_texte).unwrap_or_default();
    Ok(envois::simulation_courriels::courriels_joues(&ctx.racine, &apres))
}
