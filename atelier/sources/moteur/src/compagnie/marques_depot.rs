//! Les arrets que la DSN d'un mois reporte (`arrets_pour`). Rien d'ici ne fait partir quoi que ce soit.

use serde_json::Value;

use droit::contexte::Contexte;
use socle::echec::R;
use socle::echec::erreur;

/// Les arrets que la DSN de `mois` doit reporter -- ceux du magasin des imprevus dont la periode croise le mois principal
/// declare (`imprevus::du_mois`).
pub(crate) fn arrets_pour(ctx: &Contexte, mois: &str) -> R<Vec<Value>> {
    socle::date::premier_du_mois(mois).map_err(|m| erreur("ValueError", m))?;
    engagement::imprevus::du_mois(&ctx.racine, mois, Some(engagement::imprevus::ARRET)).map_err(crate::routes::imprevus::refus_magasin)
}
