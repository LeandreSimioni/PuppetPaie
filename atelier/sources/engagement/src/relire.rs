//! UNE PAIE ECRITE SE RELIT, ELLE NE SE RECALCULE PAS. La validation ecrit chaque paie d'un engagement (`etats`) sous
//! sa forme ecrite (`paie_ecrite::FormeDePaie`, que `calcul::Paie::forme` donne) ; ce qui fabrique un document la relit
//! ici, telle qu'elle a ete calculee : l'engagement de la saisie du mois, la fiche et le spectacle comme le calcul les lit
//! (`calcul::fiche_de_l_engagement`), les montants ecrits, et les periodes elementaires d'un mois decoupe.

use std::cell::Cell;

use crate::calcul::{fiche_de_l_engagement, DeductionForfaitaire, LigneBrut, LigneCotisation, Paie, PlafondTempsPartiel};
use crate::paie_ecrite::FormeDePaie;
use crate::{Avis, Engagement, LignePaie};
use droit::contexte::Contexte;
use socle::num::{Decimal, Resultat};

/// La forme ecrite d'une paie : sa forme canonique, ses avertissements (`{"texte", "info"}`), les lignes de son
/// engagement telles quelles (celles d'un permanent, deduites) et le plafond d'un temps partiel.
pub fn ecrire(ctx: &Contexte, p: &Paie) -> Resultat<FormeDePaie<Avis>> {
    p.forme(ctx, &Avis::clone, true)
}

/// Le flottant d'un decimal ecrit.
fn en_f64(x: Decimal) -> f64 {
    x.to_string().parse().unwrap_or(f64::NAN)
}

/// Les engagements de la saisie `data`, construits sans rien chiffrer, puis ce que le calcul a pose dessus : le numero
/// (quand l'engagement n'en porte pas),
/// les jours du contrat, les suspensions, et, pour un permanent, les lignes et les jours que le moteur a deduits. `paies` : la forme
/// ecrite de chaque paie, dans l'ordre de la saisie.
pub fn engagements_ecrits(ctx: &Contexte, data: &crate::saisie::VueDuMois, paies: &[&FormeDePaie<Avis>]) -> Resultat<Vec<Engagement>> {
    let mensuels: Vec<usize> = data.engagements.iter().enumerate().filter(|(_, e)| e.mensuelle()).map(|(i, _)| i).collect();
    let appel = Cell::new(0usize);
    let lignes = |_: &Contexte, _: &crate::saisie::Carte, _: &str| -> Resultat<(Vec<LignePaie>, Vec<Avis>, Option<i64>)> {
        let rang = *mensuels.get(appel.get()).ok_or("rang d'engagement inconnu")?;
        appel.set(appel.get() + 1);
        let ecrit = &paies.get(rang).ok_or_else(|| socle::echec::cle(socle::echec::PAIE_ILLISIBLE, "lignes"))?.engagement;
        Ok((ecrit.lignes.clone(), Vec::new(), Some(ecrit.jours)))
    };
    let (mut engs, _) = crate::engagements(ctx, data, &lignes)?;
    if engs.len() != paies.len() {
        return Err(socle::echec::cle(socle::echec::PAIE_ILLISIBLE, "paies"));
    }
    for (e, p) in engs.iter_mut().zip(paies) {
        // Le numero est celui de l'engagement, pose quand le contrat part ; une paie validee avant ne le porte pas.
        if e.numero.is_none() {
            e.numero = p.engagement.numero.clone();
        }
        e.jours_contrat = p.engagement.jours_contrat;
        e.suspensions = p.engagement.suspensions.clone();
    }
    Ok(engs)
}

/// Une `Paie` du moteur, sur l'engagement `eng`, depuis sa forme ecrite `v`.
pub fn paie(ctx: &Contexte, eng: Engagement, v: &FormeDePaie<Avis>) -> Resultat<Paie> {
    paie_lue(ctx, eng, v, None, &[])
}

/// LES FICHES DU SALARIE TELLES QUE LE CALCUL D'UNE PAIE LES A LUES : celle de la paie, et celle de chaque periode
/// elementaire d'un mois decoupe, sans les taux recus (`droit::pas::sans_taux_recus` : le taux de la paie est son `pas`).
/// Elles s'ecrivent a cote de la paie, et ce qui relit ce qui a ete declare les relit (`lire`).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FichesDuCalcul {
    pub paie: droit::fiche::SalarieDuContrat,
    pub periodes: Vec<droit::fiche::SalarieDuContrat>,
}

impl FichesDuCalcul {
    /// Celles que le calcul de `p` a lues.
    pub fn de(p: &Paie) -> FichesDuCalcul {
        let sans = |s: &droit::fiche::SalarieDuContrat| {
            let mut s = s.clone();
            s.fiche.champs = droit::pas::sans_taux_recus(&s.fiche.champs);
            s
        };
        FichesDuCalcul { paie: sans(&p.salarie), periodes: p.periodes.iter().map(|q| sans(&q.salarie)).collect() }
    }
}

/// Une `Paie` du moteur, sur l'engagement `eng`, depuis sa forme ecrite `v`, sur les fiches du salarie telles que son
/// calcul les a lues plutot que sur la fiche d'aujourd'hui.
pub fn paie_sur_ses_fiches(ctx: &Contexte, eng: Engagement, v: &FormeDePaie<Avis>, fiches: &FichesDuCalcul) -> Resultat<Paie> {
    paie_lue(ctx, eng, v, Some(&fiches.paie), &fiches.periodes)
}

fn paie_lue(ctx: &Contexte, eng: Engagement, v: &FormeDePaie<Avis>, fiche: Option<&droit::fiche::SalarieDuContrat>, periodes: &[droit::fiche::SalarieDuContrat]) -> Resultat<Paie> {
    let sal = match fiche {
        Some(f) => f.clone(),
        None => fiche_de_l_engagement(ctx, &eng)?,
    };
    let spectacle = if eng.nat()?.mensuelle() {
        None
    } else {
        Some(ctx.spectacle(eng.spectacle.as_deref().ok_or_else(|| socle::fichiers::absent("spectacle"))?)?)
    };
    let tp = PlafondTempsPartiel::lire(v.plafond_temps_partiel.as_deref().ok_or_else(|| socle::echec::cle(socle::echec::PAIE_ILLISIBLE, "plafond_temps_partiel"))?)?;
    let mut paie = Paie::nouveau(eng.clone(), sal, spectacle, v.brut, v.base_csg, v.plafond, en_f64(v.smic), v.heures, tp);
    let p = paie.ecrire();
    p.total_pat = v.total_pat;
    p.total_sal = v.total_sal;
    p.net_a_payer = v.net_a_payer;
    p.net_social = v.net_social;
    p.net_imposable = v.net_imposable;
    p.cout_employeur = v.cout_employeur;
    p.salaire_de_base = v.salaire_de_base;
    p.salaire_retabli = v.salaire_retabli;
    p.assiette_chomage = v.assiette_chomage;
    p.heures_travaillees = v.heures_travaillees;
    p.heures_absence = v.heures_absence;
    p.numero_bulletin = v.numero_bulletin.clone();
    p.lignes = v.lignes.iter().map(LigneCotisation::de_la_forme).collect();
    p.remuneration = v.remuneration.iter().map(LigneBrut::de_la_forme).collect();
    p.bases = v.bases.clone();
    p.reduction_generale = v.reduction_generale.clone();
    p.deduction_forfaitaire = v.deduction_forfaitaire.as_ref().map(DeductionForfaitaire::de_la_forme);
    p.avertissements = v.avertissements.clone();
    // Les periodes elementaires d'un mois decoupe : chacune sur l'engagement borne a sa periode, aux termes qu'elle porte.
    for (n, x) in v.periodes.iter().enumerate() {
        let e = &x.engagement;
        let mut sub = eng.clone();
        sub.periode = Some((e.debut_periode, e.fin_periode));
        sub.lignes = e.lignes.clone();
        sub.quotite_contrat = Some(en_f64(e.quotite));
        sub.heures = e.heures.map(en_f64);
        p.periodes.push(paie_lue(ctx, sub, x, periodes.get(n), &[])?);
    }
    Ok(paie)
}

/// Les paies ecrites d'une saisie de mois, dans son ordre : `ecrites[i]` est la forme ecrite de l'engagement `i`.
pub fn paies(ctx: &Contexte, data: &crate::saisie::VueDuMois, ecrites: &[&FormeDePaie<Avis>]) -> Resultat<Vec<Paie>> {
    let engs = engagements_ecrits(ctx, data, ecrites)?;
    engs.into_iter().zip(ecrites).map(|(e, v)| paie(ctx, e, v)).collect()
}

/// La saisie `data` reduite a son engagement `i` : ce que la relecture d'une seule paie lit.
pub fn saisie_d_un_engagement(data: &crate::saisie::VueDuMois, i: usize) -> Resultat<crate::saisie::VueDuMois> {
    let e = data.engagements.get(i).cloned().ok_or_else(|| socle::echec::cle(socle::echec::PAIE_ILLISIBLE, "engagements"))?;
    Ok(crate::saisie::VueDuMois { mois: data.mois, taux_cachet: data.taux_cachet.clone(), engagements: vec![e] })
}
