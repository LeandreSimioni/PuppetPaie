//! La reduction generale degressive unique (CSS L. 241-13, D. 241-7 ; parametres dans la section `reduction_generale`
//! du bareme) : le coefficient d'un brut paye pour des heures, son montant et ses parts Urssaf, chomage et
//! Agirc-Arrco, sa limite sous deduction forfaitaire specifique, et le seuil de SMIC d'une cotisation qui ne s'applique
//! qu'au-dela. Le droit recoit le bareme, le brut et les heures ; l'engagement l'appelle a son calcul.

use std::str::FromStr;

use rust_decimal::prelude::*;
use serde_json::{Map, Value};

use crate::forme_bareme::ReductionGenerale as Parametres;
use socle::num::{add, dec_f64, div, eur, mul, nombre, sub, Decimal, Resultat};

/// Un parametre exige de la section.
fn requis(v: Option<Decimal>, cle: &str) -> Resultat<Decimal> {
    v.ok_or_else(|| socle::fichiers::absent(cle))
}

fn en_f64(x: Decimal) -> f64 {
    x.to_string().parse().unwrap_or(f64::NAN)
}

/// Quatre decimales, la moitie vers le haut, l'echelle gardee.
fn quantifier4(x: Decimal) -> Decimal {
    let mut r = x.round_dp_with_strategy(4, RoundingStrategy::MidpointAwayFromZero);
    r.rescale(4);
    r
}

fn diviser(a: Decimal, b: Decimal) -> Resultat<Decimal> {
    if b.is_zero() {
        return Err(if a.is_zero() { "zéro divisé par zéro" } else { "division par zéro" }.into());
    }
    Ok(div(a, b))
}

/// La reduction generale appliquee : ce que la DSN declare.
#[derive(Debug, Clone, PartialEq)]
pub struct ReductionGenerale {
    pub coefficient: Decimal,
    pub montant: Decimal,
    pub smic_reference: Decimal,
    pub heures: Decimal,
    pub urssaf: Decimal,
    pub chomage: Decimal,
    pub agirc_arrco: Decimal,
}

impl ReductionGenerale {
    pub fn canon(&self) -> Value {
        let mut m = Map::new();
        for (k, v) in [
            ("coefficient", self.coefficient), ("montant", self.montant),
            ("smic_reference", self.smic_reference), ("heures", self.heures),
            ("urssaf", self.urssaf), ("chomage", self.chomage), ("agirc_arrco", self.agirc_arrco),
        ] {
            m.insert(k.into(), Value::String(nombre(v)));
        }
        Value::Object(m)
    }
}

/// La reduction generale degressive unique d'un contrat, ou `None` (art. D. 241-7 CSS ; `rg` : la section
/// `reduction_generale` de la pile).
pub fn reduction_generale(
    rg: Option<&Parametres>, statut: Option<&str>, brut: Decimal, heures: Decimal, caisse_conges: bool,
) -> Resultat<Option<ReductionGenerale>> {
    let Some(rg) = rg else { return Ok(None) };
    let statut = statut.filter(|s| !s.is_empty()).unwrap_or("artiste");
    if !rg.statuts.iter().any(|s| s == statut) || brut <= Decimal::ZERO || heures <= Decimal::ZERO {
        return Ok(None);
    }
    let smic_ref = eur(mul(requis(rg.smic_horaire_reference, "smic_horaire_reference")?, heures));
    let plafond = rg.plafond_smic.unwrap_or(Decimal::from(3));
    let fillon = rg.formule.as_deref() == Some("fillon");
    let mut coefficient;
    if brut >= mul(plafond, smic_ref) {
        coefficient = Decimal::ZERO;
    } else if fillon {
        let t = requis(rg.t, "t")?;
        let x = mul(diviser(t, Decimal::new(6, 1))?, sub(diviser(mul(plafond, smic_ref), brut)?, Decimal::ONE));
        coefficient = quantifier4(t.min(x));
    } else {
        let crochet = Decimal::ONE.min(div(sub(diviser(mul(plafond, smic_ref), brut)?, Decimal::ONE), Decimal::TWO));
        let puissance = en_f64(requis(rg.puissance, "puissance")?);
        let terme = dec_f64(en_f64(crochet).powf(puissance))?;
        coefficient = add(requis(rg.tmin, "tmin")?, mul(requis(rg.tdelta, "tdelta")?, terme));
    }
    if let Some(t) = rg.majoration_caisse_conges.as_deref().filter(|t| !t.is_empty()) {
        if caisse_conges && !coefficient.is_zero() {
            let parts: Vec<&str> = t.split('/').collect();
            if parts.len() != 2 {
                return Err(format!("fraction illisible : {}", socle::fichiers::repr(t)));
            }
            let lire = |s: &str| Decimal::from_str(s).map_err(|_| socle::num::illisible(s));
            let (num, den) = (lire(parts[0])?, lire(parts[1])?);
            coefficient = diviser(mul(coefficient, num), den)?;
        }
    }
    let coefficient = if coefficient.is_zero() { coefficient } else { quantifier4(coefficient) };
    reduction_au_coefficient(Some(rg), coefficient, brut, heures).map(Some)
}

/// La reduction d'un coefficient deja fixe (celui de la regularisation annuelle d'un permanent).
pub fn reduction_au_coefficient(rg: Option<&Parametres>, coefficient: Decimal, brut: Decimal, heures: Decimal) -> Resultat<ReductionGenerale> {
    let rg = rg.ok_or_else(|| socle::echec::a_la_cle("Le barème de l'année ne porte pas la réduction générale des cotisations : le signaler au développeur.", "reduction_generale", ""))?;
    let smic_ref = eur(mul(requis(rg.smic_horaire_reference, "smic_horaire_reference")?, heures));
    let montant = eur(mul(brut, coefficient));
    repartir_la_reduction(rg, coefficient, montant, smic_ref, heures)
}

fn repartir_la_reduction(rg: &Parametres, coefficient: Decimal, montant: Decimal, smic_ref: Decimal, heures: Decimal) -> Resultat<ReductionGenerale> {
    let total_points = if rg.formule.as_deref() == Some("fillon") { requis(rg.t, "t")? } else { requis(rg.tmin, "tmin")? + requis(rg.tdelta, "tdelta")? };
    let part = |taux: Option<Decimal>| -> Resultat<Decimal> {
        if montant.is_zero() {
            return Ok(Decimal::new(0, 2));
        }
        Ok(eur(diviser(div(mul(montant, taux.unwrap_or_default()), Decimal::ONE_HUNDRED), total_points)?))
    };
    let chomage = part(rg.imputation.as_ref().and_then(|i| i.chomage))?;
    let agirc = part(rg.imputation.as_ref().and_then(|i| i.agirc_arrco))?;
    Ok(ReductionGenerale {
        coefficient, montant, smic_reference: smic_ref, heures,
        urssaf: montant - chomage - agirc, chomage, agirc_arrco: agirc,
    })
}

/// La reduction generale d'une paie sous deduction forfaitaire specifique. CSS L. 241-13, III : elle << ne tient compte
/// des deductions au titre de frais professionnels calculees forfaitairement en pourcentage de cette remuneration que
/// dans des limites et conditions fixees par arrete >> ; arrete du 4 decembre 2019, art. 1er (redaction de l'arrete du
/// 4 septembre 2025, art. 11 ; `atelier/reference/sources/dfs/arrete-2019-reduction-generale/`) : << Le montant de la
/// reduction generale [...] calcule pour les remunerations apres application de l'abattement forfaitaire [...] ne peut
/// exceder, pour ces remunerations, 130 % du montant la reduction calcule sans application du meme abattement. >> La
/// reduction se calcule donc sur le brut abattu (`abattu`), dans la limite de 130 % de celle du brut entier.
pub fn reduction_generale_sous_dfs(
    rg: Option<&Parametres>, statut: Option<&str>, brut: Decimal, abattu: Decimal, heures: Decimal, caisse_conges: bool,
) -> Resultat<Option<ReductionGenerale>> {
    let Some(apres) = reduction_generale(rg, statut, abattu, heures, caisse_conges)? else { return Ok(None) };
    let sans = reduction_generale(rg, statut, brut, heures, caisse_conges)?.map_or(Decimal::new(0, 2), |r| r.montant);
    let limite = eur(mul(sans, Decimal::new(130, 2)));
    if apres.montant <= limite {
        return Ok(Some(apres));
    }
    let rg = rg.ok_or_else(|| socle::fichiers::absent("reduction_generale"))?;
    Ok(Some(repartir_la_reduction(rg, apres.coefficient, limite, apres.smic_reference, apres.heures)?))
}

/// La remuneration du contrat depasse-t-elle `seuil_smic` fois le SMIC de reference (SMIC horaire de la reduction x
/// heures remunerees) ? Sans seuil, sans SMIC de reference ou sans heures : toujours.
pub fn au_dela_du_seuil(rg: Option<&Parametres>, brut: Decimal, heures: Decimal, seuil_smic: Option<Decimal>) -> bool {
    let (Some(seuil), Some(smic)) = (seuil_smic, rg.and_then(|r| r.smic_horaire_reference)) else { return true };
    if heures <= Decimal::ZERO || smic.is_zero() {
        return true;
    }
    brut > mul(seuil, eur(mul(smic, heures)))
}
