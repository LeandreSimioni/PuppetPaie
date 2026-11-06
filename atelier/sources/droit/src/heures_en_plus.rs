//! LES HEURES EN PLUS D'UN SALARIE PAYE AU MOIS : les heures supplementaires d'un temps plein (C. trav. L3121-28 : toute
//! heure au-dela de la duree legale), les heures complementaires d'un temps partiel (L3123-20). Leur majoration se lit
//! dans la convention de la compagnie (`clauses-conventionnelles-base.json`, `heures_supplementaires`,
//! `heures_complementaires`), a defaut dans la loi (`permanents-base.json`, `heures_en_plus`) : L3121-33, I, 1° et
//! L3123-21 donnent la main a l'accord, L3121-36 et L3123-29 disent le taux « a defaut ». Ce qui suit les heures (la
//! reduction de cotisations salariales de L241-17, la deduction patronale de L241-18 et L241-18-1, l'exoneration
//! d'impot de 81 quater) se lit ici aussi ; l'engagement en fait les lignes et les montants (`engagement::heures_en_plus`).
//! Textes : `atelier/reference/sources/code-du-travail/articles-20261102.md`,
//! `code-de-la-securite-sociale/articles-20261102.md`, `code-general-des-impots/articles-20261102.md`,
//! `kali-1285-titre-vi-duree-du-travail-20261102.md`, `kali-3090-titre-viii-duree-du-travail-20261102.md`,
//! `boss/boss-heures-supplementaires-20260601.txt`.

use socle::texte::virgule;
use socle::num::{Decimal, Resultat};

/// Des heures a une majoration (en pourcent).
#[derive(Debug, Clone, PartialEq)]
pub struct Palier {
    pub heures: Decimal,
    pub majoration: Decimal,
}

/// Les heures du mois reparties par majoration, et ce qu'il y a a en dire (un contingent, un plafond depasse).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Reparties {
    pub paliers: Vec<Palier>,
    pub avis: Vec<String>,
}

/// La loi, a defaut d'accord (`permanents-base.json`, `heures_en_plus`).
fn loi() -> Resultat<crate::permanents::HeuresEnPlus> {
    Ok(crate::permanents::regles()?.heures_en_plus.clone())
}

/// Ce que la convention `idcc` dit des heures supplementaires, s'il est releve.
fn supplementaires_de_la_convention(idcc: &str) -> Resultat<Option<crate::clauses::HeuresSupplementaires>> {
    Ok(crate::clauses::releve(idcc)?.and_then(|c| c.heures_supplementaires))
}

/// LA CONVENTION FIXE-T-ELLE LES PALIERS DES HEURES SUPPLEMENTAIRES ? Sinon la loi : 25 % pour les huit premieres
/// heures de la semaine, 50 % au-dela (L3121-36), et le mois ne dit pas lesquelles sont au-dela de la huitieme d'une
/// semaine (elles se decomptent par semaine, L3121-29) : la carte le demande (`heures_supplementaires_a_50`).
pub fn paliers_conventionnels(idcc: &str) -> Resultat<bool> {
    Ok(supplementaires_de_la_convention(idcc)?.is_some_and(|c| c.paliers_annuels.as_ref().is_some_and(|p| !p.is_empty())))
}

/// LES HEURES SUPPLEMENTAIRES DU MOIS, par majoration. `heures` : celles du mois ; `a_50` : celles d'entre elles qui
/// passent la huitieme d'une semaine (la loi seule les lit) ; `deja` : celles des mois d'avant dans l'annee, que les
/// paliers annuels d'une convention et son contingent comptent (1285, VI.9 et VI.12).
pub fn supplementaires(idcc: &str, heures: Decimal, a_50: Decimal, deja: Decimal) -> Resultat<Reparties> {
    let mut r = Reparties::default();
    if heures <= Decimal::ZERO {
        return Ok(r);
    }
    let conv = supplementaires_de_la_convention(idcc)?;
    match &conv {
        Some(c) if c.paliers_annuels.as_ref().is_some_and(|p| !p.is_empty()) => {
            // Les paliers annuels de la convention : la n-ieme heure de l'annee prend la majoration de son palier ; au-dela
            // du dernier palier (le contingent), `au_dela_du_contingent`.
            let mut debut = Decimal::ZERO;
            let mut reste = heures;
            let mut rang = deja;
            for p in c.paliers_annuels.iter().flatten() {
                let fin = debut + p.heures.0;
                let ici = (fin - rang).max(Decimal::ZERO).min(reste);
                if ici > Decimal::ZERO {
                    ajouter(&mut r.paliers, ici, p.majoration.0);
                    reste -= ici;
                    rang += ici;
                }
                debut = fin;
            }
            if reste > Decimal::ZERO {
                let au_dela = c.au_dela_du_contingent.ok_or_else(|| socle::echec::livree("clauses-conventionnelles-base.json", "heures_supplementaires.au_dela_du_contingent"))?;
                ajouter(&mut r.paliers, reste, au_dela.0);
            }
        }
        _ => {
            if a_50 > heures {
                return Err(format!(
                    "{} heures supplémentaires au-delà de la huitième d'une semaine, pour {} heures supplémentaires dans le mois : \
                     les premières sont une part des secondes. Rien n'est calculé.",
                    virgule(a_50), virgule(heures)
                ));
            }
            let hebdo = loi()?.heures_supplementaires.paliers_hebdomadaires;
            let (premier, second) = match (hebdo.first(), hebdo.get(1)) {
                (Some(a), Some(b)) => (a.majoration, b.majoration),
                _ => return Err(socle::echec::livree("permanents-base.json", "heures_en_plus.heures_supplementaires.paliers_hebdomadaires")),
            };
            ajouter(&mut r.paliers, heures - a_50, premier);
            ajouter(&mut r.paliers, a_50, second);
        }
    }
    if let Some(contingent) = conv.as_ref().and_then(|c| c.contingent_annuel).map(|c| c.0).filter(|c| !c.is_zero()) {
        if deja + heures > contingent {
            r.avis.push(format!(
                "Heures supplémentaires de l'année : {} avec ce mois, au-delà du contingent annuel de {} heures de la convention. \
                 Les heures au-delà ouvrent droit à une contrepartie obligatoire en repos (C. trav. L3121-30), à poser hors de la paie.",
                virgule(deja + heures),
                virgule(contingent)
            ));
        }
    }
    Ok(r)
}

/// LES HEURES COMPLEMENTAIRES DU MOIS, par majoration, sur un contrat de `contrat` heures par mois. La loi (L3123-29) :
/// 10 % dans le dixieme des heures du contrat, 25 % jusqu'au tiers ; une convention qui ne fixe pas d'autre taux la
/// laisse (1285, VI.13 bis : 25 % au-dela du dixieme, rien dans le dixieme). Le plafond : le dixieme sans accord
/// (L3123-28), le tiers quand la convention le permet ; au-dela, un avis.
pub fn complementaires(idcc: &str, heures: Decimal, contrat: Decimal) -> Resultat<Reparties> {
    let mut r = Reparties::default();
    if heures <= Decimal::ZERO {
        return Ok(r);
    }
    let hc = loi()?.heures_complementaires;
    let part = |nom: &str| -> Resultat<Decimal> {
        match nom {
            "dixieme" => Ok(contrat / Decimal::from(10)),
            "tiers" => Ok(contrat / Decimal::from(3)),
            autre => Err(format!("heures complémentaires : une part du contrat inconnue, {}", autre)),
        }
    };
    let mut debut = Decimal::ZERO;
    let mut reste = heures;
    let mut derniere = Decimal::ZERO;
    for p in &hc.paliers {
        let fin = part(p.jusqu_au.nom())?;
        derniere = p.majoration;
        let ici = (fin - debut).max(Decimal::ZERO).min(reste);
        if ici > Decimal::ZERO {
            ajouter(&mut r.paliers, ici, derniere);
            reste -= ici;
        }
        debut = fin;
    }
    if reste > Decimal::ZERO {
        ajouter(&mut r.paliers, reste, derniere);
    }
    let plafond = crate::clauses::releve(idcc)?.and_then(|c| c.heures_complementaires).map_or(hc.plafond_sans_accord, |c| c.plafond).nom();
    let limite = part(plafond)?;
    if heures > limite {
        r.avis.push(format!(
            "{} heures complémentaires pour un contrat de {} heures par mois : au-delà du {} de la durée du contrat que la loi ou la \
             convention permettent (C. trav. L3123-20, L3123-28). Le logiciel les paie ; au-delà, la durée du contrat se revoit.",
            virgule(heures),
            virgule(contrat.round_dp(2)),
            plafond.replace("dixieme", "dixième")
        ));
    }
    Ok(r)
}

fn ajouter(v: &mut Vec<Palier>, heures: Decimal, majoration: Decimal) {
    if heures <= Decimal::ZERO {
        return;
    }
    match v.iter_mut().find(|p| p.majoration == majoration) {
        Some(p) => p.heures += heures,
        None => v.push(Palier { heures, majoration }),
    }
}

/// Le taux maximum de la reduction de cotisations salariales (D241-21 : 11,31 %).
pub fn taux_maximum_de_reduction() -> Resultat<Decimal> {
    Ok(loi()?.reduction_salariale.taux_maximum)
}

/// La deduction patronale par heure supplementaire (D241-24) : 1,50 EUR sous vingt salaries, 0,50 EUR a partir de vingt.
pub fn deduction_par_heure(au_moins_20: bool) -> Resultat<Decimal> {
    let d = loi()?.deduction_patronale;
    Ok(if au_moins_20 { d.au_moins_20_salaries } else { d.moins_de_20_salaries })
}

/// La limite annuelle de l'exoneration d'impot sur le revenu (CGI 81 quater : 7 500 EUR).
pub fn limite_de_l_exoneration() -> Resultat<Decimal> {
    Ok(loi()?.exoneration_impot.limite_annuelle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Decimal {
        s.parse().unwrap()
    }

    #[test]
    fn la_loi_majore_de_25_les_huit_premieres_heures_d_une_semaine_et_de_50_les_suivantes() {
        // L3121-36, sans convention relevee (une compagnie sans IDCC).
        let r = supplementaires("", d("12"), d("3"), Decimal::ZERO).unwrap();
        assert_eq!(r.paliers, vec![Palier { heures: d("9"), majoration: d("25") }, Palier { heures: d("3"), majoration: d("50") }]);
    }

    #[test]
    fn la_1285_majore_les_80_premieres_heures_de_l_annee_de_25_puis_de_50() {
        // CCNEAC VI.12 : 75 heures deja faites dans l'annee, 10 ce mois : 5 a 25 %, 5 a 50 % ; la semaine ne compte pas.
        let r = supplementaires("1285", d("10"), d("4"), d("75")).unwrap();
        assert_eq!(r.paliers, vec![Palier { heures: d("5"), majoration: d("25") }, Palier { heures: d("5"), majoration: d("50") }]);
    }

    #[test]
    fn la_loi_majore_de_10_les_heures_complementaires_du_dixieme_et_de_25_les_suivantes() {
        // L3123-29 : un contrat de 100 heures par mois, 14 heures complementaires : 10 a 10 %, 4 a 25 %.
        let r = complementaires("", d("14"), d("100")).unwrap();
        assert_eq!(r.paliers, vec![Palier { heures: d("10"), majoration: d("10") }, Palier { heures: d("4"), majoration: d("25") }]);
    }
}
