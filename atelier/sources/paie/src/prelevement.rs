//! Le prelevement a la source des paies d'un mois : le taux de chacune (`droit::pas`), ou ce qu'une DSN a deja
//! declare pour elle.

use serde_json::Value;

use engagement::calcul::Paie;
use droit::contexte::Contexte;
use droit::natures::contrat_court;
use droit::pas::{du_versement, taux_dgfip, ErreurPas, Triplet, Versement, ZONE_DEFAUT};
use socle::date::{en_texte, Jour};

/// Le prelevement d'une paie qu'une DSN a deja declare, ou `None` : aucune DSN partie n'a declare le mois de sa paie
/// pour son engagement (une paie ajoutee apres le depot, que la correction du mois suivant declarera), ou la paie n'a
/// pas de numero de contrat.
///
/// LE PAS D'UN MOIS DECLARE SE LIT, IL NE SE RECALCULE PAS (regles 1 et 4). CGI art. 1671, 1 et 2
/// (`atelier/reference/sources/code-general-des-impots/articles-20261013.md`, l. 99 et 102) : la retenue « est effectuée
/// par le débiteur lors du paiement », au taux que l'administration a transmis et que l'employeur applique « au plus
/// tard le deuxième mois suivant sa transmission ». Ce qui a ete retenu l'a ete au paiement, au taux de ce jour-la : le
/// prelevement de la paie que la DSN a declaree (`engagement::etats::paie_declaree`), pas celui d'un recalcul au taux du
/// jour. Le mois declare est celui de la fin de la periode de la paie, comme le declare `dsn::bloc_versement` (50.001).
fn deja_declare(ctx: &Contexte, pa: &Paie, lus: &mut Lus) -> Result<Option<Versement>, String> {
    let e = &pa.engagement;
    let Some(numero) = e.numero.clone().filter(|n| !n.is_empty()) else { return Ok(None) };
    let cle = en_texte(pa.salarie.get("_cle").unwrap_or(&Value::Null));
    let fin = e.fin_periode()?;
    let mois = format!("{:04}-{:02}", fin.an, fin.mois);
    if lus.departs.is_none() {
        lus.departs = Some(crate::journal::departs(&ctx.racine)?);
    }
    if !lus.objets.contains_key(&mois) {
        lus.objets.insert(mois.clone(), engagement::stock::objets_du_mois(&ctx.racine, &mois)?);
    }
    let departs = lus.departs.as_ref().expect("lus");
    let objet = lus.objets[&mois].iter().find(|o| {
        let b = Value::Object(o.brut.clone());
        engagement::cle_de_l_engagement(ctx, &b).as_deref() == Some(cle.as_str()) && b.get("numero").map(en_texte).as_deref() == Some(numero.as_str())
    });
    let Some(o) = objet else { return Ok(None) };
    Ok(engagement::etats::paie_declaree(ctx, o, departs, &mois)?.and_then(|p| p.pas))
}

/// Ce que `deja_declare` a lu une fois pour toutes les paies : ce qui est parti, et les engagements de chaque mois.
#[derive(Default)]
struct Lus {
    departs: Option<engagement::departs::Departs>,
    objets: std::collections::HashMap<String, Vec<engagement::stock::Objet>>,
}

/// Ce que `du_versement` dit de chaque paie, par POSITION dans `paies` (`None` :
/// paie sans bulletin, ou bareme sans grille -- on n'imprime pas un impot qu'on
/// ne sait pas calculer). Une paie qu'une DSN deposee a deja declaree garde le prelevement que la DSN dit
/// (`deja_declare`) ; une paie validee, celui qu'elle a ecrit (`Paie::pas`). Sinon le taux DGFiP se lit une fois par
/// salarie, et l'abattement se calcule une fois par versement.
pub fn des_paies(ctx: &Contexte, paies: &[Paie]) -> Result<Vec<Option<Versement>>, String> {
    let mut taux: Vec<(String, Option<Triplet>)> = Vec::new();
    let mut lus = Lus::default();
    let mut out = Vec::new();
    for pa in paies {
        let e = &pa.engagement;
        if e.sans_paie() {
            out.push(None);
            continue;
        }
        if let Some(v) = deja_declare(ctx, pa, &mut lus)? {
            out.push(Some(v));
            continue;
        }
        if let Some(v) = &pa.pas {
            out.push(Some(v.clone()));
            continue;
        }
        let cle = en_texte(pa.salarie.get("_cle").ok_or_else(|| socle::fichiers::absent("_cle"))?);
        let i = match taux.iter().position(|(c, _)| *c == cle) {
            Some(i) => i,
            None => {
                taux.push((cle.clone(), taux_dgfip(ctx, &cle, None)?));
                taux.len() - 1
            }
        };
        let fin = e.fin_periode()?;
        let bareme = ctx.bareme(&Jour { an: fin.an, mois: fin.mois, jour: 1 })?;
        let t = |j: Jour| (j.an, j.mois, j.jour);
        let court = contrat_court(&*e.nat()?, t(e.debut), e.fin.map(t), t(fin));
        match du_versement(taux[i].1.as_ref(), &bareme, pa.net_imposable, Some(fin), court, ZONE_DEFAUT) {
            Ok(v) => out.push(Some(v)),
            Err(ErreurPas::SansGrille(_)) => out.push(None),
            Err(autre) => return Err(autre.message().to_string()),
        }
    }
    Ok(out)
}
