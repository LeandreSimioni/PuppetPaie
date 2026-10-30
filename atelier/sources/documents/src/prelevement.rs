//! Le prelevement a la source des paies d'un mois : celui de la paie declaree, sinon celui que l'engagement a ecrit
//! dans la paie (`engagement::etats`). Rien ne s'y calcule.

use serde_json::Value;

use engagement::calcul::Paie;
use droit::contexte::Contexte;
use droit::pas::Versement;
use socle::date::en_texte;

/// Le prelevement de la paie declaree (`engagement::etats::paie_declaree`), ou `None` : aucune DSN partie n'a declare
/// le mois de sa paie pour son engagement (une paie ajoutee apres le depot, que la correction du mois suivant
/// declarera), ou la paie n'a pas de numero de contrat.
///
/// LE PAS D'UN MOIS DECLARE SE LIT, IL NE SE RECALCULE PAS (regles 1 et 4). CGI art. 1671, 1 et 2
/// (`atelier/reference/sources/code-general-des-impots/articles-20261013.md`, l. 99 et 102) : la retenue « est effectuée
/// par le débiteur lors du paiement », au taux que l'administration a transmis et que l'employeur applique « au plus
/// tard le deuxième mois suivant sa transmission ». Ce qui a ete retenu l'a ete au paiement, au taux de ce jour-la : le
/// prelevement de la paie que la DSN a declaree (`engagement::etats::paie_declaree`), pas celui d'un recalcul au taux du
/// jour. Le mois declare est celui de la fin de la periode de la paie, comme le declare `dsn::bloc_versement` (50.001).
fn deja_declare(ctx: &Contexte, pa: &Paie, lus: &mut Lus, departs: &engagement::departs::Departs) -> Result<Option<Versement>, String> {
    let e = &pa.engagement;
    let Some(numero) = e.numero.clone().filter(|n| !n.is_empty()) else { return Ok(None) };
    let cle = en_texte(pa.salarie.get("_cle").unwrap_or(&Value::Null));
    let fin = e.fin_periode()?;
    let mois = format!("{:04}-{:02}", fin.an, fin.mois);
    if !lus.objets.contains_key(&mois) {
        lus.objets.insert(mois.clone(), engagement::stock::objets_du_mois(&ctx.racine, &mois)?);
    }
    let objet = lus.objets[&mois].iter().find(|o| {
        let b = Value::Object(o.brut.clone());
        engagement::cle_de_l_engagement(ctx, &b).as_deref() == Some(cle.as_str()) && b.get("numero").map(en_texte).as_deref() == Some(numero.as_str())
    });
    let Some(o) = objet else { return Ok(None) };
    Ok(engagement::etats::paie_declaree(ctx, o, departs, &mois)?.and_then(|p| p.pas.clone()))
}

/// Ce que `deja_declare` a lu une fois pour toutes les paies : les engagements de chaque mois.
#[derive(Default)]
struct Lus {
    objets: std::collections::HashMap<String, Vec<engagement::stock::Objet>>,
}

/// Le prelevement de chaque paie, par POSITION dans `paies` (`None` : un mois sans paie, sans bulletin). Une paie deja
/// declaree garde le prelevement de la paie declaree (`deja_declare`) ; une autre, celui que l'engagement a ecrit ou
/// calcule pour elle (`Paie::pas`). Aucun ne se calcule ici : une paie qui n'en porte pas se refuse.
pub fn des_paies(ctx: &Contexte, paies: &[Paie], departs: &engagement::departs::Departs) -> Result<Vec<Option<Versement>>, String> {
    let mut lus = Lus::default();
    let mut out = Vec::new();
    for pa in paies {
        if pa.engagement.sans_paie() {
            out.push(None);
            continue;
        }
        if let Some(v) = deja_declare(ctx, pa, &mut lus, departs)? {
            out.push(Some(v));
            continue;
        }
        let v = pa.pas.clone().ok_or_else(|| {
            format!(
                "{} : la paie de {} ne porte pas de prelevement a la source (paie ecrite illisible : `pas`). Revalider l'engagement.",
                en_texte(pa.salarie.get("_cle").unwrap_or(&Value::Null)),
                pa.engagement.mois
            )
        })?;
        out.push(Some(v));
    }
    Ok(out)
}
