//! LES HEURES EN PLUS D'UN SALARIE PAYE AU MOIS : ses heures supplementaires (temps plein) ou complementaires (temps
//! partiel) du mois, des faits de sa carte (`HEURES_SUPPLEMENTAIRES`, `HEURES_SUPPLEMENTAIRES_A_50`,
//! `HEURES_COMPLEMENTAIRES`), dont le moteur fait les lignes du bulletin (`lignes`, une par majoration :
//! `droit::heures_en_plus`) et ce qui les suit au calcul (`apres_les_cotisations`) : la reduction de cotisations
//! salariales (CSS L241-17, D241-21), la deduction forfaitaire patronale des heures supplementaires (L241-18, L241-18-1,
//! D241-24) et l'exoneration d'impot sur le revenu (CGI 81 quater) qui sort leur montant net de la remuneration nette
//! fiscale. Un contrat d'usage n'en a pas : ses heures sont ses lignes. Textes : `logiciel/donnees/permanents-base.json`,
//! `heures_en_plus`, qui les cite.

use socle::forme::NombreLu;

use crate::calcul::{LigneBrut, LigneCotisation};
use crate::conges::{avertir, flot, info, qui};
use crate::saisie::Carte;
use crate::Avis;
use droit::contexte::Contexte;
use socle::texte::{euros, virgule};
use socle::num::{dec_f64, eur2, Decimal, Resultat};

/// Les faits du mois, sur la carte d'un salarie paye au mois.
pub const HEURES_SUPPLEMENTAIRES: &str = "heures_supplementaires";
pub const HEURES_SUPPLEMENTAIRES_A_50: &str = "heures_supplementaires_a_50";
pub const HEURES_COMPLEMENTAIRES: &str = "heures_complementaires";

/// Les rubriques que le moteur compose (`rubriques-base.json`).
pub const HEURE_SUPPLEMENTAIRE: &str = "heure_supplementaire";
pub const HEURE_COMPLEMENTAIRE: &str = "heure_complementaire";

/// Dans les bases d'une paie (`Paie::bases`) : la remuneration des heures en plus, leur nombre, et leur montant net
/// exonere d'impot (S21.G00.58.003 = 01).
pub const REMUNERATION: &str = "heures_en_plus";
pub const HEURES: &str = "heures_en_plus_heures";
pub const EXONERE: &str = "heures_en_plus_exonere";

/// Les lignes du bulletin qui portent la reduction salariale et la deduction patronale.
pub const REDUCTION: &str = "reduction_heures_en_plus";
pub const DEDUCTION: &str = "deduction_heures_supplementaires";

/// LES FAITS QU'APPELLE UN CONTRAT PAYE AU MOIS, ceux que sa carte montre sous « Avancé » : un temps partiel ses heures
/// complementaires ; un temps plein ses heures supplementaires, et, quand la convention ne fixe pas leurs paliers (la
/// loi les decompte par semaine, C. trav. L3121-29 et L3121-36), celles qui passent la huitieme d'une semaine.
pub fn faits_appeles(ctx: &Contexte, partiel: bool) -> Resultat<Vec<&'static str>> {
    if partiel {
        return Ok(vec![HEURES_COMPLEMENTAIRES]);
    }
    let idcc = ctx.entreprise.idcc.texte();
    Ok(if droit::heures_en_plus::paliers_conventionnels(&idcc)? {
        vec![HEURES_SUPPLEMENTAIRES]
    } else {
        vec![HEURES_SUPPLEMENTAIRES, HEURES_SUPPLEMENTAIRES_A_50]
    })
}

/// Les heures d'un fait de la carte, zero s'il n'est pas dit ou s'il est negatif.
fn heures(x: &Option<NombreLu>) -> Resultat<Decimal> {
    crate::conges::dec_ou(x.as_ref(), Decimal::ZERO).map(|h| h.max(Decimal::ZERO))
}

/// LES LIGNES DES HEURES EN PLUS DU MOIS de la saisie `e` d'un contrat paye au mois de `salaire` pour `quotite` heures
/// par mois, a temps `partiel` ou non : une ligne par majoration, au taux horaire du contrat majore. INFERENCE, ecrite
/// comme telle : le taux horaire est le salaire mensuel du contrat sur ses heures par mois ; le BOSS (§ 310) y ajoute les
/// primes qui sont la contrepartie du travail, que la carte ne distingue pas des autres.
pub(crate) fn lignes(ctx: &Contexte, e: &Carte, mois: &str, salaire: Decimal, quotite: Decimal, partiel: bool) -> Resultat<(Vec<crate::LignePaie>, Vec<Avis>)> {
    let (sup, a_50, comp) = (heures(&e.heures_supplementaires)?, heures(&e.heures_supplementaires_a_50)?, heures(&e.heures_complementaires)?);
    if sup.is_zero() && comp.is_zero() && a_50.is_zero() {
        return Ok((Vec::new(), Vec::new()));
    }
    if partiel && !(sup.is_zero() && a_50.is_zero()) {
        return Err(format!(
            "{} : son contrat est à temps partiel ({} h par mois) ; les heures au-delà de son contrat sont des heures \
             complémentaires (C. trav. L3123-20). Les dire dans « Heures complémentaires ». Rien n'est calculé.",
            qui(ctx, e),
            virgule(quotite.round_dp(2).normalize())
        ));
    }
    if !partiel && !comp.is_zero() {
        return Err(format!(
            "{} : son contrat est à temps plein ; les heures au-delà de la durée légale sont des heures supplémentaires (C. trav. \
             L3121-28). Les dire dans « Heures supplémentaires ». Rien n'est calculé.",
            qui(ctx, e)
        ));
    }
    if quotite.is_zero() {
        return Err(format!("{} : ses heures par mois sont nulles ; le taux horaire de ses heures en plus ne se calcule pas.", qui(ctx, e)));
    }
    let idcc = ctx.entreprise.idcc.texte();
    let (rubrique, reparties) = if partiel {
        (HEURE_COMPLEMENTAIRE, droit::heures_en_plus::complementaires(&idcc, comp, quotite)?)
    } else {
        let salarie = crate::conges::salarie_de(e)?;
        let deja = if droit::heures_en_plus::paliers_conventionnels(&idcc)? || droit::clauses::releve(&idcc)?.is_some_and(|c| c.heures_supplementaires.is_some()) {
            crate::conges::heures_supplementaires_des_mois_anterieurs(ctx, mois, &salarie)?
        } else {
            Decimal::ZERO
        };
        (HEURE_SUPPLEMENTAIRE, droit::heures_en_plus::supplementaires(&idcc, sup, a_50, deja)?)
    };
    let horaire = salaire / quotite;
    let mut out = Vec::new();
    for p in &reparties.paliers {
        let taux = (horaire * (Decimal::ONE_HUNDRED + p.majoration) / Decimal::ONE_HUNDRED).round_dp(4);
        out.push(crate::LignePaie::Composee(crate::ligne::Composee {
            rubrique: rubrique.to_string(),
            quantite: dec_f64(flot(p.heures))?,
            taux: dec_f64(flot(taux))?,
            heures: None,
            precision: Some(format!("majorées de {} %", virgule(p.majoration.normalize()))),
            dates: Vec::new(),
        }));
    }
    let mut avis = vec![info(format!(
        "{} : {} heures {} au taux horaire de son contrat ({}, le salaire mensuel sur ses {} h), {}.",
        qui(ctx, e),
        virgule(reparties.paliers.iter().fold(Decimal::ZERO, |a, p| a + p.heures).normalize()),
        if partiel { "complémentaires" } else { "supplémentaires" },
        euros(horaire.round_dp(2)),
        virgule(quotite.round_dp(2).normalize()),
        reparties.paliers.iter().map(|p| format!("{} h majorées de {} %", virgule(p.heures.normalize()), virgule(p.majoration.normalize()))).collect::<Vec<_>>().join(", ")
    ))];
    avis.extend(reparties.avis.into_iter().map(avertir));
    Ok((out, avis))
}

/// Ce que les heures en plus d'une paie font apres ses cotisations.
pub(crate) struct Suites {
    pub lignes: Vec<LigneCotisation>,
    pub reduction: Decimal,
    pub deduction: Decimal,
    pub remuneration: Decimal,
    pub heures: Decimal,
    pub exonere: Decimal,
    pub avis: Vec<String>,
}

/// LES SUITES DES HEURES EN PLUS d'une paie de `brut`, ses lignes de remuneration `lb` et ses cotisations `lignes` (la
/// reduction generale comprise), `tot_pat` ses cotisations patronales ; `None` sans heure en plus.
/// - LA REDUCTION DE COTISATIONS SALARIALES (L241-17, II ; D241-21 ; BOSS § 350-370) : la remuneration des heures,
///   majorations comprises, fois le taux moyen des cotisations salariales d'assurance vieillesse (la famille RETRAITE du
///   bulletin : vieillesse, Agirc-Arrco, CET ; pas l'Apec) sur le brut, dans la limite de 11,31 % ; imputee sur la
///   vieillesse salariale de base (L241-3) et dans sa limite (L241-17, II ; BOSS § 450-480).
/// - LA DEDUCTION PATRONALE (L241-18 et L241-18-1 ; D241-24) : par heure supplementaire (pas complementaire, BOSS § 570),
///   1,50 EUR sous vingt salaries, 0,50 EUR a partir de vingt (l'effectif de L130-1), dans la limite des cotisations
///   patronales du salarie.
/// - L'EXONERATION D'IMPOT (CGI 81 quater) : le montant net des heures sort de la remuneration nette fiscale et se declare
///   a part (fiche net-entreprises 2066, S21.G00.58.003 = 01). INFERENCE, ecrite comme telle : le net est la remuneration
///   des heures moins la CSG deductible qui la frappe (l'exemple de la fiche : 400 EUR, 373,28 EUR) et moins ce que ses
///   autres cotisations salariales, a leur taux moyen sur le brut, laissent apres la reduction ; la limite annuelle de
///   7 500 EUR se compte sur ce net, dans l'annee civile, chez l'employeur (`deja`).
pub(crate) fn apres_les_cotisations(
    ctx: &Contexte, bar: &droit::forme_bareme::Pile, lb: &[LigneBrut], lignes: &[LigneCotisation], brut: Decimal, tot_pat: Decimal, deja: impl FnOnce() -> Resultat<Decimal>,
) -> Resultat<Option<Suites>> {
    let siennes: Vec<&LigneBrut> = lb.iter().filter(|l| l.code == HEURE_SUPPLEMENTAIRE || l.code == HEURE_COMPLEMENTAIRE).collect();
    let remuneration = siennes.iter().fold(Decimal::new(0, 2), |a, l| a + l.montant);
    if remuneration <= Decimal::ZERO || brut <= Decimal::ZERO {
        return Ok(None);
    }
    let heures = siennes.iter().fold(Decimal::ZERO, |a, l| a + l.quantite);
    let supplementaires = siennes.iter().filter(|l| l.code == HEURE_SUPPLEMENTAIRE).fold(Decimal::ZERO, |a, l| a + l.quantite);
    let sal = |f: &dyn Fn(&LigneCotisation) -> bool| lignes.iter().filter(|l| f(l)).fold(Decimal::ZERO, |a, l| a + l.montant_sal);
    let mut avis = Vec::new();

    // La reduction salariale.
    let vieillesse = sal(&|l| l.famille == "RETRAITE");
    let taux = (vieillesse / brut * Decimal::ONE_HUNDRED).min(droit::heures_en_plus::taux_maximum_de_reduction()?).round_dp(2);
    let de_base = sal(&|l| l.id == "vieil_plaf" || l.id == "vieil_deplaf");
    let reduction = eur2(remuneration * taux / Decimal::ONE_HUNDRED).min(de_base).max(Decimal::new(0, 2));
    let genre = if supplementaires.is_zero() { "complémentaires" } else { "supplémentaires" };
    let mut out = vec![LigneCotisation {
        id: REDUCTION.into(),
        famille: crate::calcul::FAMILLE_ALLEGEMENTS.into(),
        libelle: format!("Réduction de cotisations salariales sur heures {}", genre),
        base: remuneration,
        taux_pat: None,
        montant_pat: Decimal::new(0, 2),
        taux_sal: Some(crate::conges::flot(taux)),
        montant_sal: -reduction,
        masquer_base_et_taux: false,
    }];

    // La deduction patronale, des heures supplementaires seules.
    let mut deduction = Decimal::new(0, 2);
    if !supplementaires.is_zero() {
        let annee = ctx.lue_au.an;
        let au_moins_20 = ctx.effectif()?.atteint(20, annee).map_err(|e| {
            format!("La déduction patronale des heures supplémentaires dépend de l'effectif (CSS L241-18, L241-18-1 : moins de vingt salariés ou au moins vingt). {}", e)
        })?;
        deduction = eur2(supplementaires * droit::heures_en_plus::deduction_par_heure(au_moins_20)?).min(tot_pat.max(Decimal::new(0, 2)));
        if !deduction.is_zero() {
            out.push(LigneCotisation {
                id: DEDUCTION.into(),
                famille: crate::calcul::FAMILLE_ALLEGEMENTS.into(),
                libelle: "Déduction patronale sur heures supplémentaires".into(),
                base: supplementaires,
                taux_pat: None,
                montant_pat: -deduction,
                taux_sal: None,
                montant_sal: Decimal::new(0, 2),
                masquer_base_et_taux: true,
            });
        }
    }

    // L'exoneration d'impot : le net des heures.
    let abattement = bar.abattement_csg;
    // La CSG deductible que la paie retient (une paie sans CSG n'en deduit rien) ; sa ligne porte son taux.
    let taux_csg = lignes
        .iter()
        .filter(|l| l.id == "csg_ded")
        .take(1)
        .map(|l| socle::num::dec_f64(l.taux_sal.ok_or("la ligne de CSG déductible n'a pas de taux salarial")?))
        .sum::<Resultat<Decimal>>()?;
    let csg_deductible = eur2(remuneration * abattement * taux_csg / Decimal::ONE_HUNDRED);
    let autres = sal(&|l| !matches!(l.id.as_str(), "csg_ded" | "csg_crds_nd" | "mutuelle"));
    let leurs_cotisations = eur2(autres * remuneration / brut);
    let net = (remuneration - csg_deductible - (leurs_cotisations - reduction).max(Decimal::ZERO)).max(Decimal::new(0, 2));
    let deja = deja()?;
    let reste = (droit::heures_en_plus::limite_de_l_exoneration()? - deja).max(Decimal::new(0, 2));
    let exonere = net.min(reste);
    if exonere < net {
        avis.push(format!(
            "Heures en plus : {} nets exonérés d'impôt cette année avant ce mois ; la limite de {} (CGI 81 quater) laisse {} \
             exonérés ce mois, le reste ({}) est imposable.",
            euros(deja),
            euros(droit::heures_en_plus::limite_de_l_exoneration()?),
            euros(exonere),
            euros(net - exonere)
        ));
    }
    Ok(Some(Suites { lignes: out, reduction, deduction, remuneration, heures, exonere, avis }))
}
