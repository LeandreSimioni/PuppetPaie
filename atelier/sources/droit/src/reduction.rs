//! La reduction generale degressive unique (CSS L. 241-13, D. 241-7 ; parametres dans la section `reduction_generale`
//! du bareme) : le coefficient d'un brut paye pour des heures, son montant et ses parts Urssaf, chomage et
//! Agirc-Arrco, sa limite sous deduction forfaitaire specifique, et le seuil de SMIC d'une cotisation qui ne s'applique
//! qu'au-dela. Le droit recoit le bareme, le brut et les heures ; l'engagement l'appelle a son calcul.

use std::str::FromStr;

use rust_decimal::prelude::*;
use serde_json::{Map, Value};

use socle::date::en_texte;
use socle::fichiers::vrai;
use socle::num::{add, dec, dec_f64, div, eur, mul, nombre, sub, Decimal, Resultat};

fn requis<'a>(o: &'a Value, cle: &str) -> Resultat<&'a Value> {
    o.get(cle).ok_or_else(|| socle::fichiers::absent(cle))
}

/// La valeur decimale d'une cle facultative, `defaut` si elle manque.
fn dec_ou(v: Option<&Value>, defaut: i64) -> Resultat<Decimal> {
    v.map_or(Ok(Decimal::from(defaut)), dec)
}

/// Le flottant d'un decimal, correctement arrondi, via le texte.
fn en_f64(x: Decimal) -> f64 {
    x.to_string().parse().unwrap_or(f64::NAN)
}

/// Au dix-millieme, la moitie s'eloignant de zero.
fn quantifier4(x: Decimal) -> Decimal {
    let mut r = x.round_dp_with_strategy(4, RoundingStrategy::MidpointAwayFromZero);
    r.rescale(4);
    r
}

/// Un quotient decimal ; diviser une valeur non nulle par zero est un refus.
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

/// La reduction generale degressive unique d'un contrat, ou `None`
/// (art. D. 241-7 CSS ; parametres dans la section `reduction_generale`).
pub fn reduction_generale(
    bar: &Value, statut: Option<&str>, brut: Decimal, heures: Decimal, caisse_conges: bool,
) -> Resultat<Option<ReductionGenerale>> {
    let rg = match bar.get("reduction_generale") {
        Some(rg) if vrai(rg) => rg,
        _ => return Ok(None),
    };
    let statut = statut.filter(|s| !s.is_empty()).unwrap_or("artiste");
    let connu = rg.get("statuts").and_then(Value::as_array).map_or(false, |a| a.iter().any(|s| s.as_str() == Some(statut)));
    if !connu || brut <= Decimal::ZERO || heures <= Decimal::ZERO {
        return Ok(None);
    }
    let smic_ref = eur(mul(dec(requis(rg, "smic_horaire_reference")?)?, heures));
    let plafond = dec_ou(rg.get("plafond_smic"), 3)?;
    let fillon = rg.get("formule").and_then(Value::as_str) == Some("fillon");
    let mut coefficient;
    if brut >= mul(plafond, smic_ref) {
        coefficient = Decimal::ZERO;
    } else if fillon {
        // T / 0,6 x (1,6 x SMIC / remuneration - 1), borne a T.
        let t = dec(requis(rg, "t")?)?;
        let x = mul(diviser(t, Decimal::new(6, 1))?, sub(diviser(mul(plafond, smic_ref), brut)?, Decimal::ONE));
        coefficient = quantifier4(t.min(x));
    } else {
        // Le crochet est borne a 1 ; seul le terme a la puissance passe par le flottant.
        let crochet = Decimal::ONE.min(div(sub(diviser(mul(plafond, smic_ref), brut)?, Decimal::ONE), Decimal::TWO));
        let puissance = en_f64(dec(requis(rg, "puissance")?)?);
        let terme = dec_f64(en_f64(crochet).powf(puissance))?;
        coefficient = quantifier4(add(dec(requis(rg, "tmin")?)?, mul(dec(requis(rg, "tdelta")?)?, terme)));
    }
    // Majoration des salaries d'une caisse de conges payes (D. 241-10 CSS).
    if let Some(b) = rg.get("majoration_caisse_conges") {
        if caisse_conges && vrai(b) && !coefficient.is_zero() {
            let t = en_texte(b);
            let parts: Vec<&str> = t.split('/').collect();
            if parts.len() != 2 {
                return Err(format!("fraction illisible : {}", socle::fichiers::repr(&t)));
            }
            let lire = |s: &str| Decimal::from_str(s).map_err(|_| socle::num::illisible(s));
            let (num, den) = (lire(parts[0])?, lire(parts[1])?);
            coefficient = quantifier4(diviser(mul(coefficient, num), den)?);
        }
    }
    reduction_au_coefficient(bar, coefficient, brut, heures).map(Some)
}

/// La reduction generale d'un brut `brut` paye pour `heures`, au coefficient `coefficient` deja determine : le montant
/// (brut x coefficient), le SMIC de reference de ces heures (S21.G00.79.004) et ses parts Urssaf, chomage et Agirc-Arrco
/// (section `reduction_generale.imputation` du bareme). `reduction_generale` y aboutit ; une periode elementaire d'un mois
/// decoupe aussi, au coefficient du mois (`engagement::periodes`).
pub fn reduction_au_coefficient(bar: &Value, coefficient: Decimal, brut: Decimal, heures: Decimal) -> Resultat<ReductionGenerale> {
    let rg = match bar.get("reduction_generale") {
        Some(rg) if vrai(rg) => rg,
        _ => return Err("reduction_au_coefficient : le bareme n'a pas de section `reduction_generale`".into()),
    };
    let smic_ref = eur(mul(dec(requis(rg, "smic_horaire_reference")?)?, heures));
    let montant = eur(mul(brut, coefficient));
    repartir_la_reduction(rg, coefficient, montant, smic_ref, heures)
}

/// La reduction d'un `montant` repartie entre l'Urssaf, le chomage et l'Agirc-Arrco selon l'`imputation` de la section.
fn repartir_la_reduction(
    rg: &Value, coefficient: Decimal, montant: Decimal, smic_ref: Decimal, heures: Decimal,
) -> Resultat<ReductionGenerale> {
    let fillon = rg.get("formule").and_then(Value::as_str) == Some("fillon");
    let total_points = if fillon {
        dec(requis(rg, "t")?)?
    } else {
        dec(requis(rg, "tmin")?)? + dec(requis(rg, "tdelta")?)?
    };
    let vide = Value::Object(Map::new());
    let imput = match rg.get("imputation") {
        Some(i) if vrai(i) => i,
        _ => &vide,
    };
    let part = |cle: &str| -> Resultat<Decimal> {
        if montant.is_zero() {
            return Ok(Decimal::new(0, 2));
        }
        let taux = dec_ou(imput.get(cle), 0)?;
        Ok(eur(diviser(div(mul(montant, taux), Decimal::ONE_HUNDRED), total_points)?))
    };
    let chomage = part("chomage")?;
    let agirc = part("agirc_arrco")?;
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
    bar: &Value, statut: Option<&str>, brut: Decimal, abattu: Decimal, heures: Decimal, caisse_conges: bool,
) -> Resultat<Option<ReductionGenerale>> {
    let Some(apres) = reduction_generale(bar, statut, abattu, heures, caisse_conges)? else { return Ok(None) };
    let sans = reduction_generale(bar, statut, brut, heures, caisse_conges)?.map_or(Decimal::new(0, 2), |r| r.montant);
    let limite = eur(mul(sans, Decimal::new(130, 2)));
    if apres.montant <= limite {
        return Ok(Some(apres));
    }
    let rg = bar.get("reduction_generale").ok_or_else(|| socle::fichiers::absent("reduction_generale"))?;
    Ok(Some(repartir_la_reduction(rg, apres.coefficient, limite, apres.smic_reference, apres.heures)?))
}

/// La remuneration du contrat depasse-t-elle `seuil_smic` fois le SMIC de
/// reference (SMIC horaire x heures remunerees) ? Sans seuil : toujours.
pub fn au_dela_du_seuil(bar: &Value, brut: Decimal, heures: Decimal, seuil_smic: Option<&Value>) -> Resultat<bool> {
    let seuil = match seuil_smic {
        None | Some(Value::Null) => return Ok(true),
        Some(s) => s,
    };
    let smic = match bar.get("reduction_generale") {
        Some(rg) if vrai(rg) => rg.get("smic_horaire_reference").filter(|v| vrai(v)),
        _ => None,
    };
    let Some(smic) = smic else { return Ok(true) };
    if heures <= Decimal::ZERO {
        return Ok(true);
    }
    let smic_ref = eur(mul(dec(smic)?, heures));
    Ok(brut > mul(dec(seuil)?, smic_ref))
}

