//! La route `courriels-fictifs` : les courriels joues de la compagnie fictive (`envois::simulation_courriels`), que la
//! page montre. Une compagnie qui n'est pas fictive n'en a pas : la route refuse.

use droit::contexte::Contexte;
use envois::simulation_courriels::CourrielsJoues;
use socle::echec::{erreur, R};

/// La charge de `courriels-fictifs` : seulement ceux d'apres ce fichier (vide ou absent : tous).
#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChargeCourrielsFictifs {
    #[serde(default)]
    pub apres: Option<String>,
}

/// Les courriels joues de la compagnie fictive ; refuse une compagnie reelle.
pub fn courriels_fictifs(ctx: &Contexte, charge: &ChargeCourrielsFictifs) -> R<CourrielsJoues> {
    if !crate::compagnie::bridage::branchement(&ctx.racine).joue() {
        return Err(erreur("ErreurOperation", "Seulement dans la compagnie fictive."));
    }
    Ok(envois::simulation_courriels::courriels_joues(&ctx.racine, charge.apres.as_deref().unwrap_or("")))
}
