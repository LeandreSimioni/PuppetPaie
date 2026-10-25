//! Le prelevement a la source des paies d'un mois : le taux de chacune (`droit::pas`), ou ce qu'une DSN deposee a deja
//! declare pour elle.

use serde_json::Value;

use engagement::calcul::Paie;
use droit::contexte::Contexte;
use droit::natures::contrat_court;
use droit::pas::{du_versement, plancher, taux_dgfip, ErreurPas, Triplet, Versement, ZONE_DEFAUT};
use socle::date::{en_texte, Jour};
use socle::fichiers::vrai;
use socle::num::{eur, sub, Decimal};

/// Le prelevement d'une paie qu'une DSN deposee a deja declare, ou `None` : le mois n'est pas depose (ni au journal, ni
/// repris de l'historique), la paie n'a pas de NIR ou pas de numero de contrat (la DSN ne se lit pas par eux), ou la DSN
/// deposee ne porte pas son versement (une paie ajoutee apres le depot, que la correction du mois suivant declarera).
///
/// LE PAS D'UN MOIS DEPOSE SE LIT, IL NE SE RECALCULE PAS (lot D2 ; regles 1 et 4). CGI art. 1671, 1 et 2
/// (`atelier/reference/sources/code-general-des-impots/articles-20261013.md`, l. 99 et 102) : la retenue « est effectuée
/// par le débiteur lors du paiement », au taux que l'administration a transmis et que l'employeur applique « au plus
/// tard le deuxième mois suivant sa transmission ». Ce qui a ete retenu l'a ete au paiement, au taux de ce jour-la, et
/// la DSN deposee le dit (S21.G00.50.006 a 50.013) ; le recalculer au taux du jour (celui que le magasin porte quand on
/// relit le mois : `taux_dgfip(ctx, cle, None)`) imprimait sur le bulletin relu, le livre de paie, l'etat du
/// prelevement, la fiche recapitulative et les ecritures un impot qui n'a pas ete preleve (PASSATION-R3, « Resté
/// ouvert »). Le mois de la DSN est celui de la fin de la periode de la paie, comme le declare `dsn::bloc_versement`
/// (50.001) ; l'etablissement, celui du salarie.
fn deja_declare(ctx: &Contexte, pa: &Paie) -> Result<Option<Versement>, String> {
    let e = &pa.engagement;
    let nir = pa.salarie.get("nir").filter(|n| vrai(n)).map(en_texte).unwrap_or_default();
    let numero = e.numero.clone().unwrap_or_default();
    if nir.is_empty() || numero.is_empty() {
        return Ok(None);
    }
    let fin = e.fin_periode()?;
    let mois = format!("{:04}-{:02}", fin.an, fin.mois);
    let nic = droit::bareme::etablissement_du_salarie(&ctx.entreprise, &pa.salarie)?;
    let Some(chemin) = crate::casc::dsn_deposee(ctx, &mois, Some(&nic))? else { return Ok(None) };
    let lu = crate::versement_declare::pas_declare(&chemin, &mois, &nir, &numero)
        .map_err(|m| format!("Le prelevement a la source de {} (contrat {}) se lit dans la DSN deposee : {}. Rien n'est ecrit.", mois, numero, m))?;
    let Some(d) = lu else { return Ok(None) };
    let zero = Decimal::new(0, 2);
    let abattement = d.abattement.unwrap_or_else(|| eur(plancher(sub(d.net_imposable, d.soumis), zero)));
    Ok(Some(Versement {
        taux: Value::String(d.taux),
        type_taux: Value::String(d.type_taux),
        identifiant: d.identifiant.map_or(Value::Null, Value::String),
        abattement,
        montant_soumis: d.soumis,
        prelevement: d.prelevement,
    }))
}

/// Ce que `du_versement` dit de chaque paie, par POSITION dans `paies` (`None` :
/// paie sans bulletin, ou bareme sans grille -- on n'imprime pas un impot qu'on
/// ne sait pas calculer). Le taux DGFiP se lit une fois par salarie ;
/// l'abattement se calcule une fois par versement. Une paie qu'une DSN deposee a
/// deja declaree garde le prelevement que la DSN dit (`deja_declare`).
pub fn des_paies(ctx: &Contexte, paies: &[Paie]) -> Result<Vec<Option<Versement>>, String> {
    let mut taux: Vec<(String, Option<Triplet>)> = Vec::new();
    let mut out = Vec::new();
    for pa in paies {
        let e = &pa.engagement;
        if e.sans_paie() {
            out.push(None);
            continue;
        }
        if let Some(v) = deja_declare(ctx, pa)? {
            out.push(Some(v));
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
