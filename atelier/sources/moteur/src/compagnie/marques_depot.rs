//! Les arrets que la DSN d'un mois reporte (`arrets_pour`). Rien d'ici ne fait partir quoi que ce soit.

use serde_json::Value;

use droit::contexte::Contexte;
use socle::echec::R;
use socle::echec::erreur;

/// Les arrets que la DSN de `mois` doit reporter : ceux du magasin des imprevus que la regle du bloc 60 y met
/// (`declarations::dsn::Arret::a_reporter` : les arrets qui croisent le mois, et ceux d'un mois anterieur saisis apres le
/// depart de sa DSN).
pub(crate) fn arrets_pour(ctx: &Contexte, mois: &str, departs: &engagement::departs::Departs) -> R<Vec<Value>> {
    socle::mois::Mois::lire(mois).map(socle::mois::Mois::premier_jour).map_err(|m| erreur("ValueError", m))?;
    let mut out = Vec::new();
    for imp in engagement::imprevus::tous(&ctx.racine).map_err(crate::routes::imprevus::refus_magasin)? {
        if !engagement::imprevus::est_un_arret(&imp) {
            continue;
        }
        let a = socle::ecriture::en_valeur(&imp);
        let arret = declarations::dsn::Arret { v: a.clone() };
        if arret.a_reporter(mois, departs).map_err(|m| erreur("ValueError", m))? {
            out.push(a);
        }
    }
    Ok(out)
}
