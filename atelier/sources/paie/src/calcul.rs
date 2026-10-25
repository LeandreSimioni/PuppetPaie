//! Les pieces de `calculer` (PORTAGE-RUST §6.4, §7.2, §7.9, §7.10) : les
//! lignes de remuneration et leur designation, les structs de sortie
//! (`LigneCotisation`, `Paie`), la reduction generale, le seuil de SMIC et la
//! deduction forfaitaire, puis `calculer` lui-meme (§7).

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, OnceLock};

use rust_decimal::prelude::*;
use serde_json::{Map, Value};

use droit::bareme::{bareme_applique_au_contrat, SANTE_SUSPENDUE, SANTE_SUSPENSION, SANTE_SUSPENSION_MAINTENUE, SANTE_SUSPENSION_SUSPENDUE};
use droit::natures::Nature;
use droit::catalogue::{rubrique, HorsAssiette, Rubrique, AVANTAGES_EN_NATURE};
use droit::contexte::{telle_qu_au, Contexte};
use socle::date::{en_texte, repr_texte, Jour};
use droit::emplois::{emploi_de, EmploiEngagement};
use crate::engagement::{nombre, Avis, Engagement};
use socle::fichiers::vrai;
use socle::num::{add, dec, dec_f64, div, eur, flottant, mul, pct, round3, sub, Decimal, Resultat};
use droit::plafond::{plafond_permanent, pour_paie, EngagementPlafond};

pub use crate::engagement::ligne_en_pourcentage;

/// `d.get(cle)` ; un `KeyError` Python se lit par son message : le repr de la cle.
fn requis<'a>(o: &'a Value, cle: &str) -> Resultat<&'a Value> {
    o.get(cle).ok_or_else(|| socle::fichiers::absent(cle))
}

/// `Decimal(str(x))` d'une valeur facultative, `defaut` si la cle manque.
fn dec_ou(v: Option<&Value>, defaut: i64) -> Resultat<Decimal> {
    v.map_or(Ok(Decimal::from(defaut)), dec)
}

/// `float(Decimal)` : correctement arrondi, via le texte.
fn en_f64(x: Decimal) -> f64 {
    x.to_string().parse().unwrap_or(f64::NAN)
}

/// `quantize(Decimal("0.0001"), ROUND_HALF_UP)`.
fn quantifier4(x: Decimal) -> Decimal {
    let mut r = x.round_dp_with_strategy(4, RoundingStrategy::MidpointAwayFromZero);
    r.rescale(4);
    r
}

/// Un quotient Decimal ; diviser une valeur non nulle par zero est un refus.
fn diviser(a: Decimal, b: Decimal) -> Resultat<Decimal> {
    if b.is_zero() {
        return Err(if a.is_zero() { "zéro divisé par zéro" } else { "division par zéro" }.into());
    }
    Ok(div(a, b))
}

// ------------------------------------------------------ les lignes de brut

/// Une ligne de remuneration : une rubrique, une quantite, un taux.
#[derive(Debug, Clone, PartialEq)]
pub struct LigneBrut {
    pub code: String,
    pub libelle: String,
    pub unite: String,
    pub quantite: Decimal,
    pub taux: Decimal,
    pub montant: Decimal,
    pub heures_annexes: Decimal,
    pub soumis: bool,
    pub imposable: bool,
    pub conges_spectacles: bool,
    pub salaire_de_base: bool,
    pub retabli: bool,
    pub conges_pris: bool,
    pub dsn_bloc52: Option<String>,
    pub arret_de_travail: bool,
    pub dsn_autres_bruts: Option<String>,
}

impl LigneBrut {
    /// La ligne en francais : « 2 Cachets », « 1 Journee de travail »,
    /// « 6 h de Masterclass ».
    pub fn designation(&self) -> String {
        let q = self.quantite.normalize();
        let entier = q == q.trunc();
        let nb = if entier { q.trunc().to_string() } else { q.to_string().replace('.', ",") };
        if self.unite == "heure" && !self.libelle.to_lowercase().starts_with("heure") {
            return format!("{} h de {}", nb, self.libelle);
        }
        // Le pluriel porte sur le premier mot.
        let texte = if q <= Decimal::ONE {
            self.libelle.clone()
        } else {
            let (tete, reste) = match self.libelle.split_once(' ') {
                Some((t, r)) => (t, Some(r)),
                None => (self.libelle.as_str(), None),
            };
            let tete = if tete.ends_with('s') { tete.to_string() } else { format!("{}s", tete) };
            match reste {
                Some(r) => format!("{} {}", tete, r),
                None => tete,
            }
        };
        format!("{} {}", nb, texte)
    }

    /// La forme canonique de l'oracle (les champs de la dataclass, dans l'ordre).
    pub fn canon(&self) -> Value {
        let texte = |o: &Option<String>| o.clone().map_or(Value::Null, Value::String);
        let mut m = Map::new();
        m.insert("code".into(), Value::String(self.code.clone()));
        m.insert("libelle".into(), Value::String(self.libelle.clone()));
        m.insert("unite".into(), Value::String(self.unite.clone()));
        m.insert("quantite".into(), Value::String(nombre(self.quantite)));
        m.insert("taux".into(), Value::String(nombre(self.taux)));
        m.insert("montant".into(), Value::String(nombre(self.montant)));
        m.insert("heures_annexes".into(), Value::String(nombre(self.heures_annexes)));
        m.insert("soumis".into(), Value::Bool(self.soumis));
        m.insert("imposable".into(), Value::Bool(self.imposable));
        m.insert("conges_spectacles".into(), Value::Bool(self.conges_spectacles));
        m.insert("salaire_de_base".into(), Value::Bool(self.salaire_de_base));
        m.insert("retabli".into(), Value::Bool(self.retabli));
        m.insert("conges_pris".into(), Value::Bool(self.conges_pris));
        m.insert("dsn_bloc52".into(), texte(&self.dsn_bloc52));
        m.insert("arret_de_travail".into(), Value::Bool(self.arret_de_travail));
        m.insert("dsn_autres_bruts".into(), texte(&self.dsn_autres_bruts));
        Value::Object(m)
    }
}

/// Les lignes de remuneration d'un engagement : une par ligne de paie, le
/// montant arrondi ligne par ligne.
pub fn lignes_brut(eng: &Engagement, ctx: &Contexte) -> Resultat<Vec<LigneBrut>> {
    let mut out = Vec::new();
    for l in &eng.lignes {
        let r = rubrique(&ctx.racine, &en_texte(requis(l, "rubrique")?))?;
        let q = dec(requis(l, "quantite")?)?;
        let taux = dec(requis(l, "taux")?)?;
        let heures = match l.get("heures") {
            None | Some(Value::Null) => None,
            Some(h) => Some(flottant(h)?),
        };
        let signe = if r.sens == "-" { -Decimal::ONE } else { Decimal::ONE };
        // Le signe porte aussi sur les heures : une absence en retire.
        let h = dec_f64(r.heures(en_f64(q), heures))?;
        let dsn_autres_bruts = code_54(&r, l)?;
        out.push(LigneBrut {
            code: r.code,
            libelle: r.libelle,
            unite: r.unite,
            quantite: q,
            taux,
            montant: signe * eur(mul(taux, q)),
            // `Decimal("-1") * Decimal("0.0")` garde l'echelle en Python.
            heures_annexes: if h.is_zero() { if signe.is_sign_negative() { -h } else { h } } else { signe * h },
            soumis: r.soumis,
            imposable: r.imposable,
            conges_spectacles: r.conges_spectacles,
            salaire_de_base: r.salaire_de_base,
            retabli: r.retabli,
            conges_pris: r.conges_pris,
            dsn_bloc52: r.dsn_bloc52,
            arret_de_travail: r.arret_de_travail,
            dsn_autres_bruts,
        });
    }
    Ok(out)
}

/// Le code S21.G00.54.001 de cette ligne de paie, ou `None` : `genre` de la
/// saisie precise lequel des cinq avantages en nature.
pub fn code_54(r: &Rubrique, ligne: &Value) -> Resultat<Option<String>> {
    let Some(code) = &r.dsn_autres_bruts else { return Ok(None) };
    let genre = match ligne.get("genre") {
        Some(g) if vrai(g) => en_texte(g).trim().to_string(),
        _ => String::new(),
    };
    if genre.is_empty() {
        return Ok(Some(code.clone()));
    }
    if !AVANTAGES_EN_NATURE.contains(&code.as_str()) {
        return Err(format!(
            "Ligne << {} >> : `genre` ({}) ne se saisit que sur un avantage \
             en nature. Cette rubrique declare {} en S21.G00.54.001.",
            r.libelle, repr_texte(&genre), code));
    }
    if !AVANTAGES_EN_NATURE.contains(&genre.as_str()) {
        return Err(format!(
            "Ligne << {} >> : `genre` vaut {}. Les avantages en nature du \
             bloc 54 sont {} -- voir catalogue.AUTRES_BRUTS.",
            r.libelle, repr_texte(&genre), AVANTAGES_EN_NATURE.join(", ")));
    }
    Ok(Some(genre))
}

// ----------------------------------------------------------------- sortie

#[derive(Debug, Clone, PartialEq)]
pub struct LigneCotisation {
    pub id: String,
    pub famille: String,
    pub libelle: String,
    pub base: Decimal,
    pub taux_pat: Option<f64>,
    pub montant_pat: Decimal,
    pub taux_sal: Option<f64>,
    pub montant_sal: Decimal,
    pub masquer_base_et_taux: bool,
}

impl LigneCotisation {
    pub fn canon(&self) -> Resultat<Value> {
        let taux = |t: Option<f64>| -> Resultat<Value> {
            t.map_or(Ok(Value::Null), |x| Ok(Value::String(nombre(dec_f64(x)?))))
        };
        let mut m = Map::new();
        m.insert("id".into(), Value::String(self.id.clone()));
        m.insert("famille".into(), Value::String(self.famille.clone()));
        m.insert("libelle".into(), Value::String(self.libelle.clone()));
        m.insert("base".into(), Value::String(nombre(self.base)));
        m.insert("taux_pat".into(), taux(self.taux_pat)?);
        m.insert("montant_pat".into(), Value::String(nombre(self.montant_pat)));
        m.insert("taux_sal".into(), taux(self.taux_sal)?);
        m.insert("montant_sal".into(), Value::String(nombre(self.montant_sal)));
        m.insert("masquer_base_et_taux".into(), Value::Bool(self.masquer_base_et_taux));
        Ok(Value::Object(m))
    }
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

/// La deduction forfaitaire specifique (DFS) appliquee a un contrat : arrete du 4 septembre 2025
/// relatif aux frais professionnels, art. 9 (`atelier/reference/sources/dfs/LISEZ-MOI.md`).
#[derive(Debug, Clone, PartialEq)]
pub struct DeductionForfaitaire {
    /// « 20 » ou « 25 » : le taux de l'article 5 de l'annexe IV du CGI (redaction du 31/12/2000) que la profession porte.
    pub categorie: String,
    /// La profession de la fiche (`deduction_forfaitaire.professions` du bareme) ; vide pour un calcul stocke avant le
    /// lot R9, qui ne gardait que la categorie.
    pub profession: String,
    /// Le taux de l'annee (bareme), en pourcent.
    pub taux: Decimal,
    /// Les lignes du bulletin dont l'assiette est abattue.
    pub rubriques: Vec<String>,
    /// La deduction retenue sur ce contrat : le brut moins l'assiette abattue, apres le plafond annuel et l'assiette
    /// minimum. C'est ce que la DSN declare en bloc 54 de type 10.
    pub montant: Decimal,
    /// Ce que l'annee civile a deja deduit pour ce salarie avant ce contrat (les mois anterieurs, les contrats
    /// precedents du mois).
    pub deja: Decimal,
}

impl DeductionForfaitaire {
    pub fn canon(&self) -> Value {
        let mut m = Map::new();
        m.insert("categorie".into(), Value::String(self.categorie.clone()));
        m.insert("profession".into(), Value::String(self.profession.clone()));
        m.insert("taux".into(), Value::String(nombre(self.taux)));
        m.insert("rubriques".into(), Value::Array(self.rubriques.iter().cloned().map(Value::String).collect()));
        m.insert("montant".into(), Value::String(nombre(self.montant)));
        m.insert("deja".into(), Value::String(nombre(self.deja)));
        Value::Object(m)
    }
}

/// Ce que la remuneration nette fiscale reintegre au net des cotisations : la CSG et la CRDS non deductibles (part
/// salariale de `csg_crds_nd`) et la part patronale des garanties frais de sante (`mutuelle`) ; norme DSN 2026.1,
/// p. 60-61 (« AJOUTER ») et p. 244 (50.004). Un seul endroit : `calculer` en tire 50.002, `Paie::net_verse` 50.004.
fn reintegrations(lignes: &[LigneCotisation]) -> Decimal {
    let zero = Decimal::new(0, 2);
    let csg_nd = lignes.iter().find(|l| l.id == "csg_crds_nd").map_or(zero, |l| l.montant_sal);
    csg_nd + lignes.iter().filter(|l| l.id == "mutuelle").fold(zero, |a, l| a + l.montant_pat)
}

#[derive(Debug, Clone)]
pub struct Paie {
    pub engagement: Engagement,
    pub salarie: Value,
    pub spectacle: Value,
    pub brut: Decimal,
    pub base_csg: Decimal,
    pub plafond: Decimal,
    pub smic: f64,
    pub heures: Decimal,
    pub lignes: Vec<LigneCotisation>,
    pub total_pat: Decimal,
    pub total_sal: Decimal,
    /// Ce qui se paie au salarie avant impot : le brut, moins les cotisations salariales, plus ou moins TOUTES les
    /// lignes hors assiette (defraiements, acompte retenu, indemnites).
    pub net_a_payer: Decimal,
    /// Le montant net social (S21.G00.58.004 de type 03, « Montant net social » du bulletin) : voir `calculer`
    /// (7.10). Le brut, moins les cotisations salariales, plus les lignes hors assiette qui sont un revenu ; ni
    /// remboursement de frais, ni mouvement de tresorerie (acompte).
    pub net_social: Decimal,
    /// La remuneration nette fiscale (S21.G00.50.002, « Net imposable » du bulletin, base du PAS) : voir
    /// `calculer` (7.10) ; les lignes hors assiette n'y entrent que si le catalogue les dit imposables.
    pub net_imposable: Decimal,
    pub cout_employeur: Decimal,
    pub numero_bulletin: Option<String>,
    /// Les assiettes du calcul, que la DSN declare telles quelles.
    pub bases: BTreeMap<String, Decimal>,
    /// Les lignes de remuneration (le haut du bulletin), pas les cotisations.
    pub remuneration: Vec<LigneBrut>,
    pub reduction_generale: Option<ReductionGenerale>,
    pub deduction_forfaitaire: Option<DeductionForfaitaire>,
    pub avertissements: Vec<Avis>,
    pub salaire_de_base: Decimal,
    pub salaire_retabli: Decimal,
    pub assiette_chomage: Decimal,
    pub heures_travaillees: Decimal,
    pub heures_absence: Decimal,
    /// Les periodes elementaires de paie d'un mois decoupe (lot R10, `periodes`), chacune une paie calculee sur ses
    /// bornes, sa fiche et sa pile ; vide pour un mois d'un seul tenant. La paie du mois en est la somme ; la DSN declare
    /// leurs remunerations (51) et leurs bases (78, 79, 81) une par une.
    pub periodes: Vec<Paie>,
}

impl Paie {
    /// Les champs sans valeur par defaut ; les autres partent a zero ou vides.
    pub fn nouveau(
        engagement: Engagement, salarie: Value, spectacle: Value, brut: Decimal,
        base_csg: Decimal, plafond: Decimal, smic: f64, heures: Decimal,
    ) -> Paie {
        let zero = Decimal::new(0, 2);
        Paie {
            engagement, salarie, spectacle, brut, base_csg, plafond, smic, heures,
            lignes: Vec::new(), total_pat: zero, total_sal: zero, net_a_payer: zero, net_social: zero,
            net_imposable: zero, cout_employeur: zero, numero_bulletin: None,
            bases: BTreeMap::new(), remuneration: Vec::new(), reduction_generale: None,
            deduction_forfaitaire: None, avertissements: Vec::new(), salaire_de_base: zero,
            salaire_retabli: zero, assiette_chomage: zero, heures_travaillees: zero,
            heures_absence: zero, periodes: Vec::new(),
        }
    }

    /// S21.G00.50.004, le montant net verse : « la remuneration nette fiscale (S21.G00.50.002), de laquelle sont
    /// deduits le montant de la CSG non deductible, le montant de la CRDS et le montant des contributions des employeurs
    /// destinees a financer des garanties « frais de sante » reintegrees dans la base fiscale » (norme DSN 2026.1,
    /// p. 244). Ce n'est PAS le net a payer : ni defraiement, ni acompte, ni indemnite non imposable n'y entrent.
    pub fn net_verse(&self) -> Decimal {
        self.net_imposable - reintegrations(&self.lignes)
    }

    /// Le titre du spectacle, ou pour un permanent le libelle de sa nature de contrat.
    pub fn titre(&self) -> Resultat<String> {
        if vrai(&self.spectacle) {
            Ok(en_texte(requis(&self.spectacle, "titre")?))
        } else {
            Ok(self.engagement.nat()?.libelle.clone())
        }
    }

    /// Les paies que la DSN declare une par une : les periodes elementaires d'un mois decoupe, sinon la paie elle-meme.
    pub fn elementaires(&self) -> Vec<&Paie> {
        if self.periodes.is_empty() {
            vec![self]
        } else {
            self.periodes.iter().collect()
        }
    }

    /// La paie est-elle decoupee au jour `j` (une periode elementaire commence ce jour-la, apres le debut du mois) ?
    pub fn decoupee_le(&self, j: &Jour) -> bool {
        self.periodes.iter().skip(1).any(|p| p.engagement.debut_periode().map_or(false, |d| d == *j))
    }

    pub fn ligne(&self, id: &str) -> Option<&LigneCotisation> {
        self.lignes.iter().find(|l| l.id == id)
    }

    pub fn resume(&self) -> Resultat<String> {
        let s = &self.salarie;
        let quoi = self.remuneration.iter().map(|l| l.designation()).collect::<Vec<_>>().join(", ");
        let quoi = if quoi.is_empty() { "sans ligne".to_string() } else { quoi };
        Ok(format!(
            "{} {} - {} - {} = {} brut, net {}, cout employeur {}",
            en_texte(requis(s, "nom")?), en_texte(requis(s, "prenom_affichage")?), self.titre()?,
            quoi, self.brut, self.net_a_payer, self.cout_employeur))
    }
}

// ------------------------------------------------------ les allegements

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
/// decoupe aussi, au coefficient du mois (`periodes::assembler`, lot R10).
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

/// La ligne du bulletin qui porte la reduction generale `rg` d'un brut `brut`.
pub(crate) fn ligne_de_reduction(rg: &ReductionGenerale, brut: Decimal) -> LigneCotisation {
    LigneCotisation {
        id: "reduction_generale".into(),
        famille: FAMILLE_ALLEGEMENTS.into(),
        libelle: "Réduction générale dégressive unique".into(),
        base: brut,
        taux_pat: None,
        montant_pat: -rg.montant,
        taux_sal: None,
        montant_sal: Decimal::new(0, 2),
        masquer_base_et_taux: true,
    }
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

/// Les bases de l'option de l'employeur (fiche entreprise, `deduction_forfaitaire_option`) : arrete du 4 septembre 2025
/// relatif aux frais professionnels, art. 9, I, al. 2 et 3 (`atelier/reference/sources/dfs/`). « L'employeur peut opter
/// pour la deduction forfaitaire specifique lorsqu'une convention ou un accord collectif l'a explicitement prevu ou
/// lorsque le comite d'entreprise, le comite social et economique ou les delegues du personnel ont donne leur accord. A
/// defaut, il appartient a chaque salarie d'accepter ou non cette option. »
pub const DFS_ACCORD_COLLECTIF: &str = "accord_collectif";
pub const DFS_ACCORD_CSE: &str = "accord_cse";
pub const DFS_ACCORD_INDIVIDUEL: &str = "accord_individuel";
/// L'accord du salarie, quand l'option repose sur lui (fiche salarie, `deduction_forfaitaire_accord`) : art. 9, I,
/// al. 3, « Celle-ci peut alors figurer soit dans le contrat de travail ou soit dans un avenant au contrat de travail. A
/// defaut, l'employeur informe et recueille le consentement annuel du salarie par tout moyen. Lorsque le travailleur
/// salarie et assimile ne repond pas dans un delai raisonnable a cette consultation, son silence vaut accord. Le salarie
/// peut toutefois demander a tout moment a son employeur a beneficier ou ne plus beneficier de la deduction, avec
/// application au 1er janvier de l'annee suivante. » Le consentement est annuel : la fiche le date (son historique).
pub const DFS_SALARIE_CONTRAT: &str = "contrat";
pub const DFS_SALARIE_CONSENTEMENT: &str = "consentement_annuel";
pub const DFS_SALARIE_REFUS: &str = "refus";

/// L'option de DFS qui vaut pour ce salarie : sa profession, son taux de l'annee, les lignes abattues, le plafond annuel,
/// les remboursements de frais que l'annexe 2 de l'arrete laisse hors de l'assiette et la tolerance du BOSS qui y laisse
/// les autres.
#[derive(Debug, Clone, PartialEq)]
pub struct OptionDfs {
    pub profession: String,
    pub categorie: String,
    pub taux: Decimal,
    pub rubriques: Vec<String>,
    pub plafond_annuel: Decimal,
    /// Les rubriques de frais (`defraiement_forfait`, `defraiement_reel`) que l'annexe 2 de l'arrete (I, 2° et 5°) laisse
    /// hors de l'assiette pour cette profession ; tout autre remboursement de frais se reintegrerait (art. 9, I, al. 4).
    pub frais_hors_assiette: Vec<String>,
    /// La tolerance du BOSS, Frais professionnels, § 2320 (bareme, `frais_cumulables`) : pour les huit secteurs, dont le
    /// spectacle vivant et le spectacle enregistre, << l'ensemble des remboursements de frais professionnels definis par
    /// l'arrete du 4 septembre 2025 peut faire l'objet d'un cumul avec la deduction forfaitaire specifique >>
    /// (`atelier/reference/sources/boss/boss-frais-professionnels-20261013.txt`, l. 2917-2921).
    pub frais_cumulables: bool,
}

fn qui_salarie(sal: &Value) -> String {
    let nom = sal.get("nom").map_or(String::new(), en_texte);
    let prenom = match sal.get("prenom_affichage") {
        Some(p) if vrai(p) => en_texte(p),
        _ => sal.get("prenom").map_or(String::new(), en_texte),
    };
    format!("{} {}", nom, prenom).trim().to_string()
}

fn texte_de(v: &Value, cle: &str) -> String {
    match v.get(cle) {
        Some(c) if vrai(c) => en_texte(c).trim().to_string(),
        _ => String::new(),
    }
}

/// La DFS que la fiche exerce, ou `None`. La decision vit ici (regle 2) : la profession de la fiche salarie
/// (`deduction_forfaitaire`), l'option de l'employeur (fiche entreprise, `deduction_forfaitaire_option`) et, quand
/// l'option repose sur chaque salarie, son accord (`deduction_forfaitaire_accord`). Arrete du 4 septembre 2025, art. 9
/// (`atelier/reference/sources/dfs/arrete-2025-frais-professionnels/`) ; les professions, l'article 5 de l'annexe IV du
/// CGI dans sa redaction en vigueur au 31/12/2000 (`atelier/reference/sources/dfs/cgi-annexe4-art5/`) et l'annexe 1 de
/// l'arrete ; leur taux de l'annee, le bareme (art. 9, II et III). Une section du bareme non marquee verifiee refuse.
pub fn deduction_forfaitaire(bar: &Value, sal: &Value, ent: &Value) -> Resultat<Option<OptionDfs>> {
    let profession = texte_de(sal, "deduction_forfaitaire");
    if profession.is_empty() {
        return Ok(None);
    }
    let qui = qui_salarie(sal);
    let annee = bar.get("annee").map_or("?".to_string(), en_texte);
    let dfs = bar.get("deduction_forfaitaire").filter(|d| vrai(d));
    let professions = dfs.and_then(|d| d.get("professions")).and_then(Value::as_object);
    let connues = || professions.map(|p| p.keys().cloned().collect::<Vec<_>>().join(", ")).unwrap_or_else(|| "aucune".into());
    let Some(fiche_prof) = professions.and_then(|p| p.get(&profession)) else {
        return Err(format!(
            "Deduction forfaitaire specifique de {} : la fiche dit << {} >>, que le bareme {} ne connait pas. La deduction \
est attachee a une PROFESSION (arrete du 4 septembre 2025, art. 9, I : << les professions, prevues a l'article 5 de \
l'annexe IV du code general des impots dans sa redaction en vigueur au 31 decembre 2000 >>), et c'est elle qui dit le \
taux et les frais que l'annexe 2 de l'arrete laisse hors de l'assiette : choisir la profession ({}).",
            qui, profession, annee, connues()
        ));
    };
    let dfs = dfs.ok_or("deduction_forfaitaire")?;
    // L'option de l'employeur, et l'accord qu'elle demande.
    let option = texte_de(ent, "deduction_forfaitaire_option");
    match option.as_str() {
        "" => {
            return Err(format!(
                "Deduction forfaitaire specifique de {} : sa fiche dit qu'il l'exerce ({}), mais la compagnie n'a pas \
opte pour elle (fiche entreprise, << Deduction forfaitaire specifique >>). C'est une option de l'EMPLOYEUR (arrete du \
4 septembre 2025, art. 9, I, al. 2) : par un accord collectif qui l'a explicitement prevue, avec l'accord du comite \
social et economique, ou, a defaut, salarie par salarie. Rien n'est calcule.",
                qui, profession
            ))
        }
        DFS_ACCORD_COLLECTIF | DFS_ACCORD_CSE => {}
        DFS_ACCORD_INDIVIDUEL => match texte_de(sal, "deduction_forfaitaire_accord").as_str() {
            DFS_SALARIE_CONTRAT | DFS_SALARIE_CONSENTEMENT => {}
            DFS_SALARIE_REFUS => return Ok(None),
            "" => {
                return Err(format!(
                    "Deduction forfaitaire specifique de {} : la compagnie l'exerce salarie par salarie, et la fiche ne \
dit pas l'accord de celui-ci. L'arrete du 4 septembre 2025 (art. 9, I, al. 3) veut qu'il l'accepte, << soit dans le \
contrat de travail ou soit dans un avenant >>, a defaut par un consentement annuel recueilli << par tout moyen >> (<< son \
silence vaut accord >>). Dire lequel ({}, {}) ou son refus ({}). Rien n'est calcule.",
                    qui, DFS_SALARIE_CONTRAT, DFS_SALARIE_CONSENTEMENT, DFS_SALARIE_REFUS
                ))
            }
            autre => {
                return Err(format!(
                    "Deduction forfaitaire specifique de {} : l'accord du salarie vaut << {} >>, que le logiciel ne connait \
pas ({}, {} ou {} ; arrete du 4 septembre 2025, art. 9, I, al. 3).",
                    qui, autre, DFS_SALARIE_CONTRAT, DFS_SALARIE_CONSENTEMENT, DFS_SALARIE_REFUS
                ))
            }
        },
        autre => {
            return Err(format!(
                "Fiche entreprise, `deduction_forfaitaire_option` = << {} >> : ce choix n'est pas connu. L'arrete du \
4 septembre 2025 (art. 9, I, al. 2 et 3) en connait trois : un accord collectif qui l'a explicitement prevue ({}), \
l'accord du comite social et economique ({}), ou, a defaut, l'accord de chaque salarie ({}).",
                autre, DFS_ACCORD_COLLECTIF, DFS_ACCORD_CSE, DFS_ACCORD_INDIVIDUEL
            ))
        }
    }
    if !dfs.get("_verifie").map_or(false, vrai) {
        return Err(format!(
            "Deduction forfaitaire specifique de {} ({}) : la section deduction_forfaitaire du bareme {} n'est pas \
marquee verifiee (\"_verifie\": true). Le moteur refuse de payer plutot que d'inventer -- comme pour un statut sans \
section verifiee.",
            qui, profession, annee
        ));
    }
    let categorie = texte_de(fiche_prof, "categorie");
    // Le taux de l'annee : celui de la profession quand l'arrete lui donne un calendrier a part (art. 9, III, pour les
    // professions hors spectacle vivant), sinon celui de sa categorie (art. 9, II).
    let taux = fiche_prof.get("taux").filter(|t| !t.is_null()).or_else(|| dfs.get("taux").and_then(Value::as_object).and_then(|t| t.get(&categorie))).ok_or_else(|| {
        format!(
            "Deduction forfaitaire specifique de {} ({}) : le bareme {} n'a pas de taux pour la categorie << {} >>.",
            qui, profession, annee, categorie
        )
    })?;
    let liste = |v: Option<&Value>| -> Vec<String> {
        match v {
            Some(Value::Array(a)) => a.iter().map(en_texte).collect(),
            _ => Vec::new(),
        }
    };
    Ok(Some(OptionDfs {
        profession,
        categorie,
        taux: dec(taux)?,
        rubriques: liste(dfs.get("rubriques_abattues")),
        plafond_annuel: dec(requis(dfs, "plafond_annuel")?)?,
        frais_hors_assiette: liste(fiche_prof.get("frais_hors_assiette")),
        frais_cumulables: dfs.get("frais_cumulables").map_or(false, vrai),
    }))
}

/// Ce que le moteur sait calculer sous DFS, et ce qu'il refuse en disant pourquoi.
/// - un contrat paye au mois (permanent, CDD de droit commun) : l'assiette de son chomage (reglement general, art. 49,
///   << entrant dans l'assiette des cotisations de securite sociale >>, sans le << avant application de l'abattement >>
///   des annexes VIII et X), sa prevoyance conventionnelle (3090 : << servant de base aux cotisations de la securite
///   sociale >> ; 1285 : rien), ses tranches de retraite et son assiette minimum mensuelle ne sont pas portees ;
/// - un cadre : l'Apec n'a pas de texte depose qui dise son assiette sous DFS ;
/// - un remboursement de frais que l'annexe 2 de l'arrete ne laisse pas hors de l'assiette, quand le bareme ne porte pas
///   la tolerance du BOSS (`frais_cumulables`) : l'article 9 (I, al. 4) le reintegre << y compris, le cas echeant, les
///   indemnites versees [...] a titre de remboursement des frais professionnels >>, et ce que la reintegration fait a
///   l'assiette chomage et a la DSN (type 35) n'est pas porte. Avec la tolerance (BOSS, Frais professionnels, § 2320,
///   `atelier/reference/sources/boss/boss-frais-professionnels-20261013.txt`, l. 2917-2921 : << l'ensemble des
///   remboursements de frais professionnels definis par l'arrete du 4 septembre 2025 peut faire l'objet d'un cumul avec
///   la deduction forfaitaire specifique >>), tout remboursement de frais reste hors de l'assiette abattue.
/// Une ligne hors assiette se classe par ce qu'elle est, la nature de sa rubrique (`Rubrique::hors_assiette`), pas par
/// son code : un remboursement de frais d'une rubrique de la compagnie est un remboursement de frais.
/// - une ligne hors assiette qui n'est pas un remboursement de frais (une indemnite) : la tolerance du BOSS ne la couvre
///   pas, et le rescrit BOSS-RES-000010 (meme page, § 2325, l. 2923-2934) veut une indemnite de transport conventionnelle
///   << dans l'assiette des cotisations et contributions sociales des le 1er euro prealablement a l'application de
///   l'abattement >> ; l'art. 9, I, al. 4 compose l'assiette du << montant global des remunerations, indemnites, primes
///   [...] >>. Aucun texte depose ne dit si une autre indemnite hors assiette (une indemnite de rupture exoneree par sa
///   propre regle) s'y reintegre : refus (regle 1). Un acompte, un mouvement de tresorerie, n'est rien de tout cela.
fn dfs_applicable(
    eng: &Engagement, nat: &Nature, sal: &Value, o: &OptionDfs, lb: &[LigneBrut], rubriques_bulletin: &[&Value], racine: &std::path::Path,
) -> Resultat<()> {
    let qui = format!("Contrat {} de {}", eng.numero.clone().unwrap_or_else(|| "?".into()), qui_salarie(sal));
    if nat.mensuelle() {
        return Err(format!(
            "{} : une deduction forfaitaire specifique sur un contrat paye au mois ({}). Le logiciel ne la porte que sur \
un contrat d'usage : pour un salarie au mois, l'assiette du chomage (reglement general d'assurance chomage, art. 49), sa \
prevoyance conventionnelle, ses tranches de retraite et son assiette minimum ne sont pas encore calculees sous deduction \
(atelier/reference/sources/dfs/LISEZ-MOI.md, << Ce qui manque >>). Rien n'est calcule.",
            qui, nat.libelle
        ));
    }
    if let Some(r) = rubriques_bulletin.iter().find(|r| r.get("id").and_then(Value::as_str).map_or(false, |i| i.starts_with("apec"))) {
        return Err(format!(
            "{} : une deduction forfaitaire specifique sur la paie d'un cadre (ligne << {} >>). L'assiette de l'Apec sous \
deduction n'est dans aucun texte depose (atelier/reference/sources/dfs/LISEZ-MOI.md, << Ce qui manque >>) : le logiciel \
ne la devine pas. Rien n'est calcule.",
            qui,
            r.get("libelle").map_or_else(String::new, en_texte)
        ));
    }
    for l in lb.iter().filter(|l| !l.soumis && !l.montant.is_zero()) {
        let genre = rubrique(racine, &l.code)?.hors_assiette()?;
        if genre == HorsAssiette::Revenu {
            return Err(format!(
                "{} : la ligne << {} >> est hors assiette et n'est pas un remboursement de frais, pour un salarie sous \
deduction forfaitaire specifique ({}). La tolerance du BOSS (Frais professionnels, § 2320) ne laisse hors de l'assiette \
que les remboursements de frais au sens de l'arrete du 4 septembre 2025 ; le rescrit BOSS-RES-000010 (§ 2325) veut une \
indemnite de transport conventionnelle dans l'assiette << des le 1er euro prealablement a l'application de \
l'abattement >>, et aucun texte depose ne dit ce que devient une autre indemnite hors assiette. Le logiciel ne le \
devine pas : si c'est un remboursement de frais, sa rubrique est de nature `frais` ; si elle entre dans l'assiette, \
elle est soumise. Rien n'est calcule.",
                qui, l.libelle, o.profession
            ));
        }
        let frais = genre == HorsAssiette::Frais;
        if frais && !o.frais_cumulables && !o.frais_hors_assiette.contains(&l.code) {
            return Err(format!(
                "{} : la ligne << {} >> rembourse des frais a un salarie sous deduction forfaitaire specifique ({}). \
L'arrete du 4 septembre 2025 (art. 9, I, al. 4) les reintegre alors dans l'assiette avant l'abattement, sauf ceux de \
la liste de son annexe 2 : pour cette profession, {} ; et le bareme ne porte pas la tolerance du BOSS (Frais \
professionnels, § 2320, `frais_cumulables`) qui les laisserait tous hors de l'assiette. Le logiciel ne porte pas encore \
cette reintegration (assiette du chomage, bloc 54 de type 35) : rien n'est calcule.",
                qui,
                l.libelle,
                o.profession,
                if o.frais_hors_assiette.is_empty() {
                    "aucun".to_string()
                } else {
                    o.frais_hors_assiette.join(", ")
                }
            ));
        }
    }
    Ok(())
}

/// Ce que l'annee civile a deja deduit pour ce salarie avant ce contrat : les mois anterieurs de la meme annee
/// (`conges::dfs_du_mois`, recalcules depuis leurs saisies) et les contrats precedents du meme mois (`precedentes`).
/// Arrete du 4 septembre 2025, art. 9, I, al. 1 : << dans la limite de 7 600 euros par annee civile >>. INFERENCE, ecrite comme
/// telle : l'annee est celle de la periode de paie, et les contrats d'un mois se suivent dans l'ordre de la saisie.
fn dfs_deja_deduite(ctx: &Contexte, eng: &Engagement, sal: &Value, precedentes: &[Paie]) -> Resultat<Decimal> {
    let cle = sal.get("_cle").and_then(Value::as_str).map(String::from).unwrap_or_else(|| eng.salarie.clone());
    let mois = eng.fin_periode()?.mois_iso();
    let mut deja = crate::conges::dfs_des_mois_anterieurs(ctx, &mois, &cle)?;
    for p in precedentes {
        let sienne = p.salarie.get("_cle").and_then(Value::as_str) == Some(cle.as_str()) || p.engagement.salarie == eng.salarie;
        if sienne {
            if let Some(d) = &p.deduction_forfaitaire {
                deja += d.montant;
            }
        }
    }
    Ok(deja)
}

/// Ce que la deduction retire d'un contrat, et les assiettes abattues de ses rubriques de salaire.
#[derive(Debug, Clone, PartialEq)]
pub struct AbattementDfs {
    pub deduction: Decimal,
    pub assiettes: Vec<Decimal>,
    pub avis: Vec<String>,
}

/// La deduction d'un contrat : chaque rubrique de salaire abattue du taux de l'annee (arrondie au centime, comme
/// avant le lot R9), puis
/// - le plafond annuel : << dans la limite de 7 600 euros par annee civile >> (arrete du 4 septembre 2025, art. 9, I,
///   al. 1) ;
///   `deja` est ce que l'annee a deja deduit ;
/// - l'assiette minimum : << L'application de ces dispositions s'entend sans prejudice du sixieme alinea de l'article
///   R. 242-1 du code de la securite sociale >> (art. 9, I, dernier alinea) ; R. 242-1, al. 6 : la base << ne peut etre inferieure,
///   en aucun cas, au montant cumule [...] du salaire minimum de croissance >> ; BOSS, assiette generale, § 410 : le
///   SMIC, sans les elements conventionnels. `assiette_minimale` est le SMIC du contrat, celui que la DSN retient (SMIC
///   horaire x heures, `calculer` 7.5). INFERENCE pour un cachet : ses heures sont celles que le moteur lui donne pour
///   la DSN (`heures_par_cachet`), aucun texte depose ne dit les heures travaillees d'un cachet.
/// Quand l'une ou l'autre ramene la deduction, l'assiette abattue se repartit entre les rubriques au prorata de leur
/// montant, le dernier centime sur la derniere.
pub fn abattement_dfs(o: &OptionDfs, montants: &[Decimal], brut: Decimal, deja: Decimal, assiette_minimale: Decimal) -> AbattementDfs {
    let zero = Decimal::new(0, 2);
    let facteur = sub(Decimal::ONE, div(o.taux, Decimal::ONE_HUNDRED));
    let theoriques: Vec<Decimal> = montants.iter().map(|m| eur(mul(*m, facteur))).collect();
    let pleine = brut - theoriques.iter().fold(zero, |a, b| a + *b);
    let mut avis = Vec::new();
    let reste = zero.max(o.plafond_annuel - deja);
    let mut retenue = pleine.min(reste);
    if retenue < pleine {
        avis.push(format!(
            "Deduction forfaitaire specifique ramenee de {} a {} EUR : le plafond de {} EUR par annee civile (arrete du \
4 septembre 2025, art. 9) est atteint, {} EUR deja deduits cette annee.",
            pleine, retenue, o.plafond_annuel, deja
        ));
    }
    if brut - retenue < assiette_minimale {
        let plancher = zero.max(brut - assiette_minimale).min(retenue);
        avis.push(format!(
            "Deduction forfaitaire specifique ramenee de {} a {} EUR : l'assiette abattue ne descend pas sous le SMIC du \
contrat, {} EUR (CSS R. 242-1, al. 6 ; arrete du 4 septembre 2025, art. 9, I, dernier alinea).",
            retenue, plancher, assiette_minimale
        ));
        retenue = plancher;
    }
    let assiettes = if retenue == pleine || brut <= Decimal::ZERO {
        theoriques
    } else {
        let cible = brut - retenue;
        let mut out = Vec::with_capacity(montants.len());
        let mut cumul = zero;
        for (i, m) in montants.iter().enumerate() {
            let a = if i + 1 == montants.len() { cible - cumul } else { eur(div(mul(*m, cible), brut)) };
            cumul += a;
            out.push(a);
        }
        out
    };
    let deduction = brut - assiettes.iter().fold(zero, |a, b| a + *b);
    AbattementDfs { deduction, assiettes, avis }
}

// ---------------------------------------------------------------- calculer

const FAMILLE_ALLEGEMENTS: &str = "EXONÉRATIONS ET ALLÈGEMENTS DE COTISATIONS";

/// Les bases lues par cle : une valeur par rubrique de salaire, sinon globale.
const GLOBALES: [&str; 19] = [
    "csg", "t1_journaliere", "t2_journaliere", "t2_prevoyance_journaliere", "apec", "cet", "t1_mensuelle",
    "t2_mensuelle", "apec_mensuelle", "cet_mensuelle", "pss_periode", "t2_conventionnelle", "t1_t2_mensuelle",
    "brut_plafonne", "brut_x115_plafonne", "brut_chomage", "brut_plafonne_dfs", "brut_x115_plafonne_dfs",
    "t1_mensuelle_x115",
];

/// Les assiettes d'un montant de salaire (§7.4).
struct Assiettes {
    brut: Decimal,
    brut_cs: Decimal,
    csg: Decimal,
    x115: Decimal,
    x110: Decimal,
    dfs: Option<Decimal>,
    /// Les assiettes majorees apres la deduction forfaitaire specifique : la majoration s'applique au brut abattu
    /// (lettre circulaire Acoss 2013-019, annexe 1, R6 ; fiche Audiens « Les Conges Spectacles », 2023 : 334,50 =
    /// 300 x 1,115 ; atelier/reference/sources/majoration-caisses-conges/).
    x115_dfs: Option<Decimal>,
    x110_dfs: Option<Decimal>,
}

impl Assiettes {
    fn lire(&self, cle: &str) -> Option<Decimal> {
        match cle {
            "brut" => Some(self.brut),
            "brut_cs" => Some(self.brut_cs),
            "brut_x115" => Some(self.x115),
            "brut_x110" => Some(self.x110),
            "brut_dfs" => self.dfs,
            "brut_x115_dfs" => self.x115_dfs,
            "brut_x110_dfs" => self.x110_dfs,
            _ => None,
        }
    }
}

fn pss_du_contexte(ctx: &Contexte) -> impl Fn(i32) -> Resultat<(Decimal, Decimal)> + '_ {
    move |a| ctx.plafond_securite_sociale(a).map(|(m, h)| (Decimal::from(m), Decimal::from(h)))
}

fn tuple(j: &Jour) -> (i32, u32, u32) {
    (j.an, j.mois, j.jour)
}

/// Une liste de taux, `None` si absente ou vide (le faux Python).
pub(crate) fn taux_liste(v: Option<&Value>) -> Resultat<Option<Vec<f64>>> {
    match v {
        Some(Value::Array(a)) if !a.is_empty() => Ok(Some(a.iter().map(flottant).collect::<Resultat<_>>()?)),
        _ => Ok(None),
    }
}

pub(crate) fn somme_pct(a: Decimal, taux: &[f64]) -> Resultat<Decimal> {
    let mut m = Decimal::new(0, 2);
    for t in taux {
        m += pct(a, dec_f64(*t)?);
    }
    Ok(m)
}

/// `bareme_applique_au_contrat(ctx.bareme(fin), sal, nat, fin, caisse)`, garde : la pile ne
/// depend que des fichiers du bareme, de la fiche entreprise, du salarie, de
/// la nature, de la caisse de conges et du jour. Un mois et ses mois anterieurs la recomposaient a
/// chaque paie, et c'etait l'essentiel du temps de calcul.
fn bareme_du_salarie(ctx: &Contexte, sal: &Value, nat: &Nature, fin: &Jour, caisse: bool, cpf_cdd: bool) -> Resultat<Arc<Value>> {
    type Cache = Mutex<HashMap<String, Arc<Value>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let cle = format!("{}|{}|{}|{}|{}|{}|{}", ctx.empreinte_bareme(fin.an), fin.iso(), nat.cle, sal, nat.mensuelle(), caisse, cpf_cdd);
    if let Some(v) = cache.lock().unwrap().get(&cle) {
        return Ok(v.clone());
    }
    let bar = Arc::new(bareme_applique_au_contrat(ctx.bareme(fin)?, sal, nat, Some(fin), caisse, cpf_cdd)?);
    socle::fichiers::ranger(cache, cle, bar.clone());
    Ok(bar)
}

/// La fiche salarie telle qu'elle vaut pour l'engagement `eng`, emploi compris (`emplois::emploi_de`). Un contrat
/// d'usage la lit telle qu'au premier jour du contrat (un nouveau contrat est un nouveau contrat) ; un contrat paye au
/// mois, telle qu'au mois paye (`emplois::emploi_du_mois`, vague V3c). La paie (`calculer`) et ce qui lit un contrat
/// sans le payer (`amorcage::embauche` : la DPAE, le signalement d'amorcage, les emplois multiples de la DSN) la
/// lisent ici, et pas autrement : deux lectures d'un meme contrat feraient deux emplois.
pub fn fiche_de_l_engagement(ctx: &Contexte, eng: &Engagement) -> Resultat<Value> {
    let nat = eng.nat()?;
    let fiche = ctx.salarie(&eng.salarie)?;
    let tqa = |f: &Value, j: &str| telle_qu_au(f, &Jour::lire(j)?);
    let emploi = EmploiEngagement {
        emploi: eng.emploi.clone(),
        emploi_affichage: eng.emploi_affichage.clone(),
        pcs_ese: eng.pcs_ese.clone(),
        code_emploi_ft: eng.code_emploi_ft.clone(),
        statut: eng.statut.clone(),
        statut_conventionnel: eng.statut_conventionnel.clone(),
        statut_categoriel_rc: eng.statut_categoriel_rc.clone(),
    };
    if nat.mensuelle() {
        let brute = fiche.get("_cle").and_then(Value::as_str).and_then(|c| ctx.salarie_brut(c));
        // La fiche de la derniere periode elementaire : un mois decoupe (lot R10) declare la derniere valeur connue
        // (norme DSN 2026.1, § 2.1.2.4, p. 22) ; un mois d'un seul tenant, la periode de paie entiere. La dispense de
        // sante se lit sur ces memes bornes (`dispense_de_la_periode`, lot R7).
        let (debut, fin) = *crate::periodes::bornes(ctx, eng)?.last().ok_or("aucune periode de paie")?;
        let du_mois = droit::emplois::emploi_du_mois(&fiche, brute, &debut.iso(), &fin.iso(), &tqa)?;
        let fiche = dispense_de_la_periode(emploi_de(&du_mois, None, &emploi, &tqa)?, &debut, &fin)?;
        sante_de_la_suspension(ctx, eng, fiche)
    } else {
        emploi_de(&fiche, Some(&eng.debut.iso()), &emploi, &tqa)
    }
}

/// `calcul.calculer` : la paie d'un engagement (`calculer_apres`, sans contrat precedent dans le mois).
pub fn calculer(ctx: &Contexte, eng: &Engagement) -> Resultat<Paie> {
    calculer_apres(ctx, eng, &[])
}

/// `calculer`, les paies deja calculees du meme mois en main (`engagement::charger_donnees`, dans l'ordre de la
/// saisie) : la deduction forfaitaire specifique d'un salarie se plafonne par annee civile, contrats precedents du mois
/// compris (`dfs_deja_deduite`, lot R9). Un mois d'un seul tenant se calcule en une fois (`calculer_periode`) ; un mois ou
/// le statut, la categorie ou la quotite d'un contrat paye au mois change en cours de periode se decoupe en periodes
/// elementaires (`periodes::bornes`, lot R10), chacune calculee de meme, les periodes deja calculees comptant parmi les
/// precedentes, et la paie du mois en est la somme (`periodes::assembler`).
pub fn calculer_apres(ctx: &Contexte, eng: &Engagement, precedentes: &[Paie]) -> Resultat<Paie> {
    let bornes = crate::periodes::bornes(ctx, eng)?;
    if bornes.len() <= 1 {
        return calculer_periode(ctx, eng, precedentes);
    }
    let mut parts: Vec<Paie> = Vec::new();
    for e in crate::periodes::engagements_des_periodes(ctx, eng, &bornes)? {
        let avant: Vec<Paie> = precedentes.iter().chain(parts.iter()).cloned().collect();
        parts.push(calculer_periode(ctx, &e, &avant)?);
    }
    crate::periodes::assembler(ctx, eng, parts)
}

/// La pile de bareme sur laquelle se calcule la paie `p` (celle de `calculer_periode`, du meme cache).
pub(crate) fn pile_de_la_paie(ctx: &Contexte, p: &Paie) -> Resultat<Arc<Value>> {
    let eng = &p.engagement;
    let nat = eng.nat()?;
    let caisse = droit::caisse_conges::affilie(&nat, &p.salarie, &eng.debut, eng.fin.as_ref())?;
    let cpf_cdd = crate::assujettissement::contrat_ouvre_cpf_cdd(
        &nat,
        eng.motif_recours.as_deref(),
        &format!("Contrat {} de {}", eng.numero.as_deref().unwrap_or("sans numero"), eng.salarie),
    )?;
    bareme_du_salarie(ctx, &p.salarie, &nat, &eng.fin_periode()?, caisse, cpf_cdd)
}

/// Une date de la fiche (`AAAA-MM-JJ`), ou `None` quand le champ est vide.
fn date_de_la_fiche(fiche: &Value, cle: &str) -> Resultat<Option<Jour>> {
    match fiche.get(cle).filter(|v| vrai(v)) {
        None => Ok(None),
        Some(v) => Jour::lire(&en_texte(v)).map(Some).map_err(|_| {
            format!("Fiche salarie, `{}` : « {} » n'est pas une date (AAAA-MM-JJ).", cle, en_texte(v))
        }),
    }
}

/// LA DISPENSE DE SANTE A DES BORNES (Q13, tranchee par Leandre le 11/10/2026). Les accords de branche deposes en
/// donnent : la dispense de l'assure individuel « ne peut jouer que jusqu'a l'echeance du contrat individuel », celle
/// d'une couverture collective se justifie chaque annee, et le salarie « cessant de justifier » de son cas adhere
/// (CCN 3090, art. 9.2 et 9.2.2 b et d, `atelier/reference/sources/kali-3090-accord-sante-permanents-20240719-20261011.md` ;
/// CCNEAC XII.2.1.8.1 b, `kali-1285-accord-frais-de-sante-20260130-20261011.md` l. 78). La fiche porte le motif (`sante_dispense`) et, s'il y a lieu, `sante_dispense_depuis` et
/// `sante_dispense_jusqu_au` ; hors de ces bornes, le salarie est affilie : la fiche que la paie lit perd le motif, et
/// le bareme (`bareme_sante`) comme la DSN (`dsn::affiliations_du_permanent`) le traitent en affilie.
///
/// La periode payee entiere dans la dispense : dispense ; entiere dehors : affilie. Une borne en cours de periode
/// (une dispense qui commence avec le contrat n'en est pas une) refuse : le partage de la cotisation du mois entre
/// dispense et affiliation n'est dit par aucun texte depose (regle 1).
pub(crate) fn dispense_de_la_periode(mut fiche: Value, debut: &Jour, fin: &Jour) -> Resultat<Value> {
    if !fiche.get("sante_dispense").is_some_and(vrai) {
        return Ok(fiche);
    }
    let depuis = date_de_la_fiche(&fiche, "sante_dispense_depuis")?;
    let jusqu_au = date_de_la_fiche(&fiche, "sante_dispense_jusqu_au")?;
    if depuis.is_some_and(|d| d > *fin) || jusqu_au.is_some_and(|j| j < *debut) {
        if let Some(o) = fiche.as_object_mut() {
            o.remove("sante_dispense");
        }
        return Ok(fiche);
    }
    let borne = match (depuis.filter(|d| d > debut), jusqu_au.filter(|j| j < fin)) {
        (Some(d), _) => Some(format!("commence le {}", d.jj_mm_aaaa())),
        (None, Some(j)) => Some(format!("finit le {}", j.jj_mm_aaaa())),
        (None, None) => None,
    };
    if let Some(borne) = borne {
        let qui = [fiche.get("prenom"), fiche.get("nom")].into_iter().flatten().filter(|v| vrai(v)).map(en_texte).collect::<Vec<_>>().join(" ");
        return Err(format!(
            "{} : sa dispense de complementaire sante {}, en cours de periode payee (du {} au {}). Le moteur ne partage \
pas la cotisation sante d'un mois entre dispense et affiliation : aucun texte depose ne dit comment. Dater la borne de \
la dispense au premier jour d'un mois (fiche salarie, « Dispense depuis ») ou au dernier jour d'un mois (« Dispense \
jusqu'au »), si l'organisme et la convention le permettent ; sinon c'est a trancher. Rien n'est calcule.",
            if qui.is_empty() { "Ce salarie".to_string() } else { qui },
            borne,
            debut.jj_mm_aaaa(),
            fin.jj_mm_aaaa()
        ));
    }
    Ok(fiche)
}

/// LA COMPLEMENTAIRE SANTE D'UN MOIS ENTIEREMENT SUSPENDU SANS INDEMNISATION (15/10/2026). Un permanent dont toute la
/// periode payee est couverte par des suspensions non remunerees (conge parental 632, sabbatique 639, sans solde 501... :
/// `imprevus::suspendu_sans_remuneration` ; un arret de travail n'en est pas une) :
///
/// - **3090** : « Le benefice des garanties [...] est suspendu pour les salaries permanents dont le contrat de travail est
///   suspendu et ne donne lieu a aucune indemnisation » ; le salarie qui veut rester affilie paie « integralement » les
///   deux parts, que « l'organisme assureur preleve [...] directement aupres du salarie » (accord du 19/07/2024, etendu,
///   art. 4.2 et 15, `atelier/reference/sources/kali-3090-accord-sante-permanents-20240719-20261011.md`). Ni cotisation ni
///   part patronale sur la paie de ce mois, donc rien a reintegrer au net fiscal : la fiche porte `SANTE_SUSPENDUE`.
/// - **toute autre convention** (1285 : l'accord du 30/01/2026 ne parle que de la suspension « indemnisee » ; un contrat
///   d'entreprise) : AUCUN TEXTE DEPOSE ne dit ce que devient la sante ; le droit commun laisse le maintien facultatif
///   (instruction DSS/3C/5B/2021/127, II.B, `atelier/reference/sources/suspension-non-remuneree/dss-instruction-3c5b-2021-
///   127-suspension-contrat-prevoyance-20210617.txt`) et c'est le contrat d'adhesion de la compagnie qui tranche. La fiche
///   entreprise le dit (`SANTE_SUSPENSION`) : « suspendue », comme la 3090 ; « maintenue », la paie cotise comme un mois
///   ordinaire (la part patronale se reintegre alors au net fiscal, CGI 83, 1° quater, et le montant soumis au PAS reste a
///   0.00 : `pas::du_versement`). Sans reponse, refus (regle 1).
///
/// La prevoyance n'a rien a suspendre ici : 3090 et 1285 l'assoient sur la remuneration (tranche A, tranche 1 ;
/// `legifrance-3090-prevoyance-titre12-20260919.md`, `legifrance-1285-prevoyance-titre12-20260919.md`), nulle ce
/// mois-la -- inference : aucun texte depose ne traite de la suspension non indemnisee pour la prevoyance.
fn sante_de_la_suspension(ctx: &Contexte, eng: &Engagement, mut fiche: Value) -> Resultat<Value> {
    if fiche.get("sante_dispense").is_some_and(vrai) {
        return Ok(fiche);
    }
    let cle = fiche.get("_cle").map(en_texte).unwrap_or_default();
    let (debut, fin) = (eng.debut_periode()?, eng.fin_periode()?);
    let suspensions = crate::imprevus::suspendu_sans_remuneration(&ctx.racine, &cle, eng.numero.as_deref(), &eng.debut, &debut, &fin)?;
    let Some(premiere) = suspensions.first() else { return Ok(fiche) };
    let motifs: Vec<String> = suspensions.iter().map(|s| s.get("motif").map(en_texte).unwrap_or_default()).collect();
    let bar = ctx.bareme(&fin)?;
    let ent = crate::dsn::entreprise_du(&bar, &ctx.entreprise);
    let idcc = ent.get("idcc").map(en_texte).unwrap_or_default();
    let pourquoi = if idcc == "3090" {
        format!(
            "contrat suspendu sans indemnisation tout le mois (motif {}) : les garanties de sante sont suspendues (CCN 3090, \
accord sante du 19/07/2024, art. 4.2 et 15)",
            motifs.join(", ")
        )
    } else {
        match ent.get(SANTE_SUSPENSION).map(en_texte).unwrap_or_default().trim() {
            SANTE_SUSPENSION_SUSPENDUE => format!(
                "contrat suspendu sans indemnisation tout le mois (motif {}) : les garanties de sante sont suspendues (fiche \
entreprise, « {} »)",
                motifs.join(", "),
                SANTE_SUSPENSION_SUSPENDUE
            ),
            SANTE_SUSPENSION_MAINTENUE => return Ok(fiche),
            autre => {
                let qui = [fiche.get("prenom"), fiche.get("nom")].into_iter().flatten().filter(|v| vrai(v)).map(en_texte).collect::<Vec<_>>().join(" ");
                return Err(format!(
                    "{} : son contrat est suspendu sans indemnisation sur toute la periode payee (du {} au {} ; suspension {} \
depuis le {}). Que devient sa complementaire sante ce mois-la ? Sous la convention {}, aucun texte ne le dit : c'est le \
contrat de sante de la compagnie qui tranche (le maintien est facultatif, instruction DSS du 17/06/2021, II.B). {}A dire \
une fois pour toutes : Configuration › Entreprise, « Santé d'un permanent suspendu sans indemnisation » (`{}`) -- « {} » \
si le contrat suspend les garanties (rien sur la paie), « {} » s'il les maintient avec les cotisations. Rien n'est calcule.",
                    if qui.is_empty() { cle.clone() } else { qui },
                    debut.jj_mm_aaaa(),
                    fin.jj_mm_aaaa(),
                    motifs.join(", "),
                    premiere.get("debut").and_then(|d| Jour::lire(&en_texte(d)).ok()).map(|d| d.jj_mm_aaaa()).unwrap_or_default(),
                    if idcc.is_empty() { "de la compagnie".to_string() } else { idcc.clone() },
                    if autre.is_empty() { String::new() } else { format!("La fiche dit « {} », qui n'est pas une reponse. ", autre) },
                    SANTE_SUSPENSION,
                    SANTE_SUSPENSION_SUSPENDUE,
                    SANTE_SUSPENSION_MAINTENUE
                ));
            }
        }
    };
    if let Some(o) = fiche.as_object_mut() {
        o.insert(SANTE_SUSPENDUE.into(), Value::String(pourquoi));
    }
    Ok(fiche)
}

/// Le contrat de cette paie est-il declare a la caisse des Conges Spectacles (`caisse_conges::affilie`, D. 7121-41) :
/// son contrat et la fiche qu'elle a lue.
pub fn declaree_a_la_caisse_des_conges(p: &Paie) -> Resultat<bool> {
    let e = &p.engagement;
    droit::caisse_conges::affilie(&*e.nat()?, &p.salarie, &e.debut, e.fin.as_ref())
}

/// La paie d'un engagement sur sa periode (la periode de paie, ou une periode elementaire), dans l'ordre du §7.
pub fn calculer_periode(ctx: &Contexte, eng: &Engagement, precedentes: &[Paie]) -> Resultat<Paie> {
    let nat = eng.nat()?;
    let mut avis_dfs: Vec<String> = Vec::new();
    let fin_periode = eng.fin_periode()?;
    let sal = fiche_de_l_engagement(ctx, eng)?;
    let spec = if nat.mensuelle() {
        Value::Null
    } else {
        ctx.spectacle(eng.spectacle.as_deref().ok_or_else(|| socle::fichiers::absent("spectacle"))?)?
    };
    // Le contrat est-il declare a la caisse des Conges Spectacles ? (`caisse_conges::affilie` : un CDDU de moins de
    // douze mois, un CDD de droit commun de moins de douze mois d'un artiste.)
    let caisse = droit::caisse_conges::affilie(&nat, &sal, &eng.debut, eng.fin.as_ref())?;
    // Le contrat ouvre-t-il la contribution CPF-CDD (C. trav. L. 6331-6 ; lot R2) ? Un CDD de droit commun, ni saisonnier,
    // ni contrat aide (`assujettissement::contrat_ouvre_cpf_cdd`, la regle du bloc 44).
    let cpf_cdd = crate::assujettissement::contrat_ouvre_cpf_cdd(
        &nat,
        eng.motif_recours.as_deref(),
        &format!("Contrat {} de {}", eng.numero.as_deref().unwrap_or("sans numero"), eng.salarie),
    )?;
    let bar = bareme_du_salarie(ctx, &sal, &nat, &fin_periode, caisse, cpf_cdd)?;
    let rubriques_bulletin: Vec<&Value> = requis(&bar, "rubriques_bulletin")?
        .as_array()
        .ok_or("rubriques_bulletin : une liste est attendue")?
        .iter()
        .collect();
    let jours = Decimal::from(eng.jours);

    // 7.2 : les lignes de remuneration.
    let lb = lignes_brut(eng, ctx)?;
    let soumises: Vec<&LigneBrut> = lb.iter().filter(|l| l.soumis).collect();
    if caisse {
        // La caisse paie ses conges (D. 7121-41) : l'employeur ne les paie pas en plus.
        if let Some(l) = lb.iter().find(|l| matches!(l.code.as_str(), "absence_conges_payes" | "indemnite_conges_payes" | "indemnite_compensatrice_conges")) {
            return Err(format!(
                "Contrat {} : la ligne << {} >> paie des conges a un salarie que l'employeur declare a la caisse des \
Conges Spectacles (code du travail D. 7121-41 ; `caisse_conges::affilie`) : c'est la caisse qui les lui verse. Les payer aussi au bulletin serait les payer deux fois. Refus.",
                eng.numero.clone().unwrap_or_else(|| "?".into()),
                l.code
            ));
        }
    }
    let rubriques: Vec<Decimal> = soumises.iter().map(|l| l.montant).collect();
    let brut = rubriques.iter().fold(Decimal::new(0, 2), |a, b| a + *b);

    // 7.3 : la prevoyance dans l'assiette CSG.
    let prev = rubriques_bulletin.iter().find(|r| r.get("id").and_then(Value::as_str) == Some("prevoyance"));
    let prev = match prev {
        Some(p) if p.get("taux_pat").map_or(false, |t| !t.is_null()) => *p,
        _ => {
            return Err(format!(
                "Bareme {}{} : aucune rubrique `prevoyance` avec un taux \
                 patronal. Elle entre dans l'assiette CSG (98,25 % du brut + \
                 part patronale de prevoyance) : sans elle la CSG serait \
                 sous-declaree sans que rien ne le signale.",
                bar.get("annee").map_or("?".to_string(), en_texte),
                if nat.bareme.is_some() { format!(", nature '{}'", nat.cle) } else { String::new() }
            ))
        }
    };
    let taux_prev = dec(&prev["taux_pat"])?;
    let plafond_journalier = bar.get("plafond_journalier").filter(|v| vrai(v));
    let t1_du_contrat = |p: &Value| -> Resultat<Decimal> { Ok(eur(mul(dec(p)?, jours))) };
    let mut plafond_prev = None;
    if prev.get("base").and_then(Value::as_str) == Some("t1_journaliere") {
        if let Some(p) = plafond_journalier {
            plafond_prev = Some(t1_du_contrat(p)?);
        }
    }
    let mut pss_prev = None;
    if nat.mensuelle() {
        // Le plafond de la periode, le meme partout (`plafond_de_l_engagement`) : corrige a la quotite d'un temps
        // partiel, sauf renonciation de l'etablissement ou artiste (`plafond_temps_partiel`).
        let p = plafond_de_l_engagement(ctx, eng, &sal)?;
        pss_prev = Some(p);
        if prev.get("base").and_then(Value::as_str) == Some("t1_mensuelle") {
            plafond_prev = Some(p);
        }
    }
    // Les autres lignes de prevoyance, sur une tranche 2 : leur part patronale entre aussi dans l'assiette.
    let mut tranches_prev = Decimal::new(0, 2);
    for r in &rubriques_bulletin {
        let id = en_texte(requis(r, "id")?);
        if id == "prevoyance" || !id.starts_with("prevoyance") || !r.get("taux_pat").map_or(false, vrai) {
            continue;
        }
        let base = en_texte(requis(r, "base")?);
        let t1 = if base == "t2_prevoyance_journaliere" && plafond_journalier.is_some() {
            Some(t1_du_contrat(plafond_journalier.unwrap())?)
        } else if base == "t2_conventionnelle" {
            pss_prev
        } else {
            None
        };
        let Some(t1) = t1 else {
            return Err(format!(
                "Bareme {} : la ligne de prevoyance `{}` porte l'assiette \
                 `{}`, que le calcul de l'assiette CSG ne sait pas lire. Sa \
                 part patronale doit y entrer : sans elle la CSG serait \
                 sous-declaree.",
                bar.get("annee").map_or("?".to_string(), en_texte), id, base
            ));
        };
        let base_t2 = Decimal::new(0, 2).max(brut.min(Decimal::from(4) * t1) - t1);
        tranches_prev = add(tranches_prev, div(mul(base_t2, dec(&r["taux_pat"])?), Decimal::ONE_HUNDRED));
    }
    // La part patronale de la mutuelle : un forfait sur le plafond MENSUEL de la periode, sans la correction du temps
    // partiel (`plafond_de_la_sante`).
    let mutuelle = rubriques_bulletin
        .iter()
        .find(|r| r.get("id").and_then(Value::as_str) == Some("mutuelle") && r.get("taux_pat").map_or(false, vrai));
    let pss_sante = if nat.mensuelle() { Some(plafond_de_la_sante(ctx, eng)?) } else { None };
    if let (Some(m), Some(pss)) = (mutuelle, pss_sante) {
        tranches_prev += pct(pss, dec(&m["taux_pat"])?);
    }

    // 7.4 : les assiettes, par rubrique de salaire.
    let abattement = dec(requis(&bar, "abattement_csg")?)?;
    let maj115 = dec(requis(&bar, "majoration_conges_115")?)?;
    let maj110 = dec(requis(&bar, "majoration_conges_110")?)?;
    let assiettes = |montant: Decimal, cs: bool| -> Assiettes {
        let mut base_prev = montant;
        if let Some(pp) = plafond_prev {
            if brut > Decimal::ZERO {
                base_prev = div(mul(brut.min(pp), montant), brut);
            }
        }
        let base_prev_t2 = if !tranches_prev.is_zero() && brut > Decimal::ZERO {
            div(mul(tranches_prev, montant), brut)
        } else {
            Decimal::ZERO
        };
        Assiettes {
            brut: montant,
            brut_cs: if cs { montant } else { Decimal::new(0, 2) },
            csg: eur(add(add(mul(montant, abattement), div(mul(base_prev, taux_prev), Decimal::ONE_HUNDRED)), base_prev_t2)),
            x115: eur(mul(montant, maj115)),
            x110: eur(mul(montant, maj110)),
            dfs: None,
            x115_dfs: None,
            x110_dfs: None,
        }
    };
    // L'assiette des Conges Spectacles : les rubriques que le catalogue y met, d'un contrat que l'employeur declare a la
    // caisse -- et d'aucun autre (un CDI, un contrat de douze mois ou plus : `caisse_conges::affilie`).
    let mut par_rubrique: Vec<Assiettes> = soumises.iter().map(|l| assiettes(l.montant, l.conges_spectacles && caisse)).collect();
    // UN MOIS SANS SALAIRE OU LA PART PATRONALE RESTE DUE (une sante maintenue pendant une suspension non indemnisee :
    // `sante_de_la_suspension`, « maintenue »). Les contributions de l'employeur aux garanties collectives entrent dans
    // l'assiette de la CSG (CSS L136-1-1, I, « toutes les sommes [...] dues [...] a l'occasion d'un travail », dont le III, 2°,
    // n'exclut pas les contributions aux garanties collectives : `atelier/reference/sources/suspension-non-remuneree/legi-extrait-code-de-la-
    // securite-sociale-20261015.md`) ; elles se repartissent au prorata du brut de chaque rubrique, et un brut nul ne les
    // repartit nulle part : elles vont entieres a la premiere rubrique soumise, plutot que de sortir de l'assiette.
    if brut.is_zero() && !tranches_prev.is_zero() {
        if let Some(premiere) = par_rubrique.first_mut() {
            premiere.csg = eur(add(premiere.csg, tranches_prev));
        }
    }
    let zero = Decimal::new(0, 2);
    let base_csg = par_rubrique.iter().fold(zero, |a, b| a + b.csg);

    // 7.5 : le SMIC retenu pour la DSN.
    let smic = ctx.smic(&fin_periode)?;
    let hpc = dec(requis(&bar, "heures_par_cachet")?)?;
    let heures_annexes = lb.iter().fold(Decimal::ZERO, |a, l| a + l.heures_annexes);
    let heures = if nat.mensuelle() { Decimal::ZERO.max(heures_annexes) } else { heures_annexes.min(mul(hpc, jours)) };
    let plafond = eur(mul(dec_f64(smic)?, heures));

    // 7.5 bis : la deduction forfaitaire specifique (arrete du 4 septembre 2025, art. 9 ; `abattement_dfs`). Elle abat
    // l'assiette des lignes que le bareme nomme, rubrique de salaire par rubrique de salaire, apres le plafond annuel
    // et sans descendre sous l'assiette minimum ; les assiettes majorees partent du brut abattu (Acoss 2013-019, R6).
    let mut dfs: Option<DeductionForfaitaire> = None;
    if let Some(o) = deduction_forfaitaire(&bar, &sal, &ctx.entreprise)? {
        dfs_applicable(eng, &nat, &sal, &o, &lb, &rubriques_bulletin, &ctx.racine)?;
        let deja = dfs_deja_deduite(ctx, eng, &sal, precedentes)?;
        let montants: Vec<Decimal> = soumises.iter().map(|l| l.montant).collect();
        let a = abattement_dfs(&o, &montants, brut, deja, plafond);
        for (r, abattu) in par_rubrique.iter_mut().zip(&a.assiettes) {
            r.dfs = Some(*abattu);
            r.x115_dfs = Some(eur(mul(*abattu, maj115)));
            r.x110_dfs = Some(eur(mul(*abattu, maj110)));
        }
        avis_dfs.extend(a.avis);
        dfs = Some(DeductionForfaitaire {
            categorie: o.categorie.clone(),
            profession: o.profession.clone(),
            taux: o.taux,
            rubriques: o.rubriques.clone(),
            montant: a.deduction,
            deja,
        });
    }

    // 7.6 : les bases globales.
    let mut plafond_intermittent = None;
    let mut bases: BTreeMap<String, Decimal> = BTreeMap::new();
    let somme_de = |f: &dyn Fn(&Assiettes) -> Decimal| par_rubrique.iter().fold(zero, |a, b| a + f(b));
    bases.insert("brut".into(), brut);
    bases.insert("brut_cs".into(), somme_de(&|b| b.brut_cs));
    bases.insert("csg".into(), base_csg);
    bases.insert("brut_x115".into(), somme_de(&|b| b.x115));
    bases.insert("brut_x110".into(), somme_de(&|b| b.x110));
    if let Some(pj) = plafond_journalier {
        let t1 = t1_du_contrat(pj)?;
        let mult = |cle: &str| -> Resultat<Decimal> { Ok(Decimal::from(dec(requis(&bar, cle)?)?.trunc().to_i64().unwrap_or(0))) };
        bases.insert("t1_journaliere".into(), brut.min(t1));
        bases.insert("t2_journaliere".into(), zero.max(brut.min(mult("t2_multiple")? * t1) - t1));
        bases.insert("apec".into(), brut.min(mult("apec_multiple")? * t1));
        bases.insert("t2_prevoyance_journaliere".into(), zero.max(brut.min(Decimal::from(4) * t1) - t1));
        bases.insert("cet".into(), if brut > t1 { brut } else { zero });
    }
    if nat.mensuelle() {
        let pss = plafond_de_l_engagement(ctx, eng, &sal)?;
        // `pss_periode` : l'assiette du forfait de sante (rubrique `mutuelle`, base 18 du bloc 79), le plafond de la
        // periode sans la correction du temps partiel (`plafond_de_la_sante`). Les tranches (`t1_mensuelle`...) suivent,
        // elles, le plafond corrige.
        bases.insert("pss_periode".into(), pss_sante.unwrap_or(pss));
        bases.insert("t1_mensuelle".into(), brut.min(pss));
        // Le FNAL plafonne d'un permanent declare a la caisse (CDD de moins de douze mois d'un
        // artiste) : seul le montant plafonne se majore (lettre circulaire Acoss 2013-019, annexe 1, R2), comme
        // `brut_x115_plafonne` d'un intermittent. Sans majoration (1,0), c'est la tranche 1.
        let x115 = bases["brut_x115"];
        bases.insert("t1_mensuelle_x115".into(), if brut <= pss { x115 } else { eur(mul(pss, maj115)) });
        bases.insert("t2_mensuelle".into(), zero.max(brut.min(Decimal::from(8) * pss) - pss));
        // Quatre plafonds de la periode : l'Apec d'un cadre, et le chomage et l'AGS d'un permanent. Les contributions
        // d'assurance chomage « sont assises sur les remunerations brutes dans la limite d'un plafond » (C. trav.
        // L. 5422-9), « fixe a quatre fois le plafond mentionne a l'article L. 241-3 du code de la securite sociale »
        // (reglement general annexe a la convention du 15/11/2024, art. 49, al. 2) ; l'AGS suit la meme assiette
        // (L. 3253-18). Le plafond est celui de la periode de paie, reduit au prorata des jours d'emploi quand le contrat
        // ne couvre pas le mois (CSS R. 242-2, al. 3) : `pss`. Textes : atelier/reference/sources/chomage-permanents/.
        let quatre_plafonds = brut.min(Decimal::from(4) * pss);
        bases.insert("apec_mensuelle".into(), quatre_plafonds);
        bases.insert("brut_chomage".into(), quatre_plafonds);
        bases.insert("t2_conventionnelle".into(), zero.max(brut.min(Decimal::from(4) * pss) - pss));
        bases.insert("cet_mensuelle".into(), if brut > pss { brut } else { zero });
        bases.insert("t1_t2_mensuelle".into(), brut.min(Decimal::from(8) * pss));
    } else {
        // Le plafond d'un intermittent : celui que le bas du bulletin imprime (`plafond_de_l_engagement`), et la seule
        // source. Textes : atelier/reference/sources/plafond-intermittents/LISEZ-MOI.md. La vieillesse plafonnee et le
        // FNAL s'assoient « dans la limite » du plafond de L. 241-3 CSS ; le chomage des annexes VIII et X « dans la
        // limite d'un plafond fixe a quatre fois le plafond mentionne a l'article L. 241-3 » (annexes VIII et X du
        // 15/11/2024, art. 49). Sous le plafond, rien ne bouge : les bases sont le brut, et le brut majore tel qu'il
        // etait.
        let plafond_ss = plafond_de_l_engagement(ctx, eng, &sal)?;
        plafond_intermittent = Some(plafond_ss);
        let x115 = bases["brut_x115"];
        bases.insert("brut_plafonne".into(), brut.min(plafond_ss));
        // La majoration de 11,5 % (CSS L. 243-1-3, D. 243-0-2) s'applique a l'assiette plafonnee : au-dela du plafond,
        // c'est le plafond qu'elle majore (lettre circulaire Acoss 2013-019, annexe 1, R2 ;
        // atelier/reference/sources/majoration-caisses-conges/LISEZ-MOI.md).
        bases.insert("brut_x115_plafonne".into(), if brut <= plafond_ss { x115 } else { eur(mul(plafond_ss, maj115)) });
        bases.insert("brut_chomage".into(), brut.min(Decimal::from(4) * plafond_ss));
    }
    if dfs.is_some() {
        let s = par_rubrique.iter().fold(zero, |a, b| a + b.dfs.unwrap_or(zero));
        bases.insert("brut_dfs".into(), s);
        let x115 = par_rubrique.iter().fold(zero, |a, b| a + b.x115_dfs.unwrap_or(zero));
        bases.insert("brut_x115_dfs".into(), x115);
        bases.insert("brut_x110_dfs".into(), par_rubrique.iter().fold(zero, |a, b| a + b.x110_dfs.unwrap_or(zero)));
        if let Some(plafond_ss) = plafond_intermittent {
            // Le plafond limite l'assiette abattue : la vieillesse plafonnee et la base 02 sur le brut abattu dans la
            // limite du plafond du contrat (V1n l'avait laisse sur le brut entier).
            bases.insert("brut_plafonne_dfs".into(), s.min(plafond_ss));
            bases.insert("brut_x115_plafonne_dfs".into(), if s <= plafond_ss { x115 } else { eur(mul(plafond_ss, maj115)) });
        }
    }

    // 7.7 : la boucle des cotisations.
    let mut lignes: Vec<LigneCotisation> = Vec::new();
    let (mut tot_pat, mut tot_sal) = (zero, zero);
    for r in &rubriques_bulletin {
        let id = en_texte(requis(r, "id")?);
        let mut cle_base = en_texte(requis(r, "base")?);
        if let Some(d) = &dfs {
            // Une rubrique abattue s'assoit sur le brut abattu ; ses assiettes majorees aussi (Acoss 2013-019, R6).
            if d.rubriques.contains(&id) {
                if let Some(abattue) = match cle_base.as_str() {
                    "brut" => Some("brut_dfs"),
                    "brut_x115" => Some("brut_x115_dfs"),
                    "brut_x110" => Some("brut_x110_dfs"),
                    "brut_plafonne" if bases.contains_key("brut_plafonne_dfs") => Some("brut_plafonne_dfs"),
                    "brut_x115_plafonne" if bases.contains_key("brut_x115_plafonne_dfs") => Some("brut_x115_plafonne_dfs"),
                    _ => None,
                } {
                    cle_base = abattue.into();
                }
            }
        }
        let base = *bases.get(&cle_base).ok_or_else(|| socle::fichiers::absent(&cle_base))?;
        let assiettes_r: Vec<Decimal> = if GLOBALES.contains(&cle_base.as_str()) {
            vec![base]
        } else {
            par_rubrique
                .iter()
                .map(|b| b.lire(&cle_base).ok_or_else(|| socle::fichiers::absent(&cle_base)))
                .collect::<Resultat<_>>()?
        };
        let (mut m_pat, mut m_sal) = (zero, zero);
        let lire_taux = |cle: &str| -> Resultat<Option<f64>> {
            match requis(r, cle)? {
                Value::Null => Ok(None),
                v => flottant(v).map(Some),
            }
        };
        let mut taux_pat = lire_taux("taux_pat")?;
        let mut taux_sal = lire_taux("taux_sal")?;
        let mut comp_pat = taux_liste(r.get("composants_pat"))?;
        let mut comp_sal = taux_liste(r.get("composants_sal"))?;
        let manque = || "taux absent (taux_pat ou taux_sal) : le complément ne s'ajoute à rien".to_string();
        // Le complement au-dela d'un seuil : le taux reduit, plus le complement.
        if let Some(compl) = r.get("complement").filter(|c| vrai(c)) {
            if au_dela_du_seuil(&bar, brut, Decimal::ZERO.max(heures_annexes), compl.get("seuil_smic"))? {
                taux_pat = Some(round3(taux_pat.ok_or_else(manque)? + flottant(requis(compl, "taux")?)?));
            }
        }
        // La CET des cadres, fondue dans T1 et T2 : l'arrondi se fait en deux parts.
        if let Some(cet) = r.get("cet_si_depasse").filter(|c| vrai(c)) {
            if bases.get("cet").map_or(false, |c| !c.is_zero()) {
                let (cet_pat, cet_sal) = (flottant(requis(cet, "pat")?)?, flottant(requis(cet, "sal")?)?);
                let mut cp = comp_pat.take().map_or_else(|| taux_pat.map(|t| vec![t]).ok_or_else(manque), Ok)?;
                *cp.last_mut().unwrap() += cet_pat;
                let mut cs = comp_sal.take().map_or_else(|| taux_sal.map(|t| vec![t]).ok_or_else(manque), Ok)?;
                *cs.last_mut().unwrap() += cet_sal;
                comp_pat = Some(cp);
                comp_sal = Some(cs);
                taux_pat = Some(round3(taux_pat.ok_or_else(manque)? + cet_pat));
                taux_sal = Some(round3(taux_sal.ok_or_else(manque)? + cet_sal));
            }
        }
        let composants = taux_liste(r.get("composants"))?;
        for a in &assiettes_r {
            if let Some(c) = &composants {
                m_pat += somme_pct(*a, c)?;
            } else if let Some(c) = &comp_pat {
                m_pat += somme_pct(*a, c)?;
            } else if let Some(t) = taux_pat {
                m_pat += pct(*a, dec_f64(t)?);
            }
            if let Some(c) = &comp_sal {
                m_sal += somme_pct(*a, c)?;
            } else if let Some(t) = taux_sal {
                m_sal += pct(*a, dec_f64(t)?);
            }
        }
        lignes.push(LigneCotisation {
            id,
            famille: en_texte(requis(r, "famille")?),
            libelle: en_texte(requis(r, "libelle")?),
            base,
            taux_pat,
            montant_pat: m_pat,
            taux_sal,
            montant_sal: m_sal,
            masquer_base_et_taux: r.get("masquer_base_et_taux").map_or(false, vrai),
        });
        tot_pat += m_pat;
        tot_sal += m_sal;
    }
    lignes.retain(|l| {
        !(l.base.is_zero() && l.montant_pat.is_zero() && l.montant_sal.is_zero() && (l.id == "retraite_t2" || l.id == "apec_t2"))
    });

    // 7.9 : la reduction generale, apres les cotisations.
    let statut = sal.get("statut").and_then(Value::as_str);
    let caisse_conges = bases["brut_cs"] > Decimal::ZERO;
    // Sous DFS, la reduction se calcule sur le brut abattu, dans la limite de 130 % de celle du brut entier
    // (`reduction_generale_sous_dfs`).
    let rg = match bases.get("brut_dfs").filter(|_| dfs.is_some()) {
        Some(abattu) => reduction_generale_sous_dfs(&bar, statut, brut, *abattu, heures_annexes, caisse_conges)?,
        None => reduction_generale(&bar, statut, brut, heures_annexes, caisse_conges)?,
    };
    if let Some(rg) = rg.as_ref().filter(|rg| !rg.montant.is_zero()) {
        lignes.push(ligne_de_reduction(rg, brut));
        tot_pat -= rg.montant;
    }

    // 7.10 : la sortie. Ce qui est hors assiette se paie ou se retient quand meme.
    let hors_assiette = lb.iter().filter(|l| !l.soumis).fold(zero, |a, l| a + l.montant);
    let net = brut - tot_sal + hors_assiette;
    // La remuneration nette fiscale (S21.G00.50.002) part du brut soumis, deduit les cotisations salariales et
    // reintegre la CSG/CRDS non deductible et la sante patronale (`reintegrations`). Des lignes hors assiette, elle ne
    // prend que celles que le catalogue dit imposables : la norme (2026.1, § 2.3.2, p. 60-61) fait « DEDUIRE les sommes
    // versees a titre de remboursement de frais professionnels » et « AJOUTER les indemnites imposables a l'impot sur
    // le revenu ». Un defraiement, une indemnite exoneree (CGI 80 duodecies) n'y entrent pas ; un acompte non plus : il
    // avance le net du mois, il ne change pas le revenu.
    let hors_assiette_imposable = lb.iter().filter(|l| !l.soumis && l.imposable).fold(zero, |a, l| a + l.montant);
    // Le montant net social (S21.G00.58.004, type 03). C. trav. R. 3243-1, 9° bis, qui renvoie au CSS R. 844-1, II
    // (redaction du decret n° 2023-1378, que le decret n° 2026-535 n'a pas touchee) ; page du BOSS « Montant net
    // social » du 01/10/2026, II.A.1 et Q 20, 27 (textes : atelier/reference/sources/montant-net-social/LISEZ-MOI.md). La
    // totalite des sommes versees en contrepartie ou a l'occasion du travail, exonerees ou non, moins les cotisations et
    // contributions sociales du salarie (`tot_sal`, parts salariales de sante et de prevoyance comprises). Le
    // financement patronal des garanties collectives (sante, prevoyance, Fonds sante des intermittents) n'y entre pas :
    // R. 844-1, II, 1° l'excepte. La page du BOSS l'y range (II.A.1, Q 13, 14, 29) sans texte qui l'appuie ; le decret
    // prime (vague 1m, meme LISEZ-MOI, derniere section). Des lignes hors assiette, seules
    // celles qui sont un revenu (indemnites, imposables ou non) ; ni remboursement de frais dans ses limites, ni
    // mouvement de tresorerie (acompte) : la nature de la rubrique le dit, et une nature qui ne le dit pas refuse
    // (`Rubrique::hors_assiette`). Pas de prelevement a la source : un net SOCIAL.
    let mut hors_assiette_revenu = zero;
    for l in lb.iter().filter(|l| !l.soumis) {
        if rubrique(&ctx.racine, &l.code)?.hors_assiette()? == HorsAssiette::Revenu {
            hors_assiette_revenu += l.montant;
        }
    }
    lignes.iter().find(|l| l.id == "csg_crds_nd").ok_or("StopIteration")?;
    let (base_51, retabli, chomage_51) = if nat.remunerations_au_brut() {
        (brut, brut, brut)
    } else {
        let base_51 = lb.iter().filter(|l| l.salaire_de_base).fold(zero, |a, l| a + l.montant);
        // S21.G00.51.011 = 003, « Salaire rétabli – reconstitué », qui « doit toujours être déclaré pour la période de paie
        // courante dans la DSN mensuelle » (cahier technique 2026.1, bloc S21.G00.51, Type :
        // `atelier/reference/norme-dsn/texte/bloc-S21.G00.51.txt`) : le salaire RECONSTITUE, le brut sans les absences qui
        // l'ont reduit (les rubriques `retabli`). Un mois entierement suspendu declare donc 001 a 0.00 et 003 au salaire du
        // mois.
        let retabli = brut - lb.iter().filter(|l| l.retabli).fold(zero, |a, l| a + l.montant);
        let chomage = brut - lb.iter().filter(|l| l.dsn_bloc52.is_some() && l.soumis).fold(zero, |a, l| a + l.montant);
        (base_51, retabli, chomage)
    };
    let mut p = Paie::nouveau(eng.clone(), sal, spec, brut, base_csg, plafond, smic, heures);
    p.heures_absence = -lb.iter().filter(|l| l.heures_annexes < Decimal::ZERO).fold(Decimal::ZERO, |a, l| a + l.heures_annexes);
    p.lignes = lignes;
    p.total_pat = tot_pat;
    p.total_sal = tot_sal;
    p.net_a_payer = net;
    p.net_social = brut - tot_sal + hors_assiette_revenu;
    p.net_imposable = brut - tot_sal + hors_assiette_imposable + reintegrations(&p.lignes);
    p.cout_employeur = brut + tot_pat;
    p.bases = bases;
    p.reduction_generale = rg;
    p.deduction_forfaitaire = dfs;
    p.avertissements.extend(avis_dfs.into_iter().map(|texte| Avis { info: true, texte }));
    p.salaire_de_base = base_51;
    p.salaire_retabli = retabli;
    p.assiette_chomage = chomage_51;
    p.heures_travaillees = Decimal::ZERO.max(heures_annexes);
    p.remuneration = lb;
    Ok(p)
}

/// Le plafond de securite sociale d'un engagement (`plafond::pour_paie`) : celui que le bas du bulletin imprime, et
/// celui sur lequel s'assoient la vieillesse plafonnee, le FNAL et le chomage d'un intermittent (`calculer`, 7.6).
/// `sal` : la fiche du salarie telle que la paie la lit (`emploi_de`), pour son statut : l'arrete du 24/01/1975 ne
/// vise que les artistes ; un technicien, comme un artiste engage cinq jours ou plus, a le plafond proratise sur les
/// jours remuneres de la paie (BOSS, assiette generale, § 1090, `atelier/reference/sources/boss/`). Textes :
/// atelier/reference/sources/plafond-intermittents/LISEZ-MOI.md.
pub fn plafond_de_l_engagement(ctx: &Contexte, e: &Engagement, sal: &Value) -> Resultat<Decimal> {
    let nat = e.nat()?;
    let statut = sal.get("statut").filter(|s| vrai(s)).map(en_texte).unwrap_or_else(|| "artiste".into());
    let artiste = match statut.as_str() {
        "artiste" => true,
        "technicien" => false,
        autre if !nat.mensuelle() => {
            return Err(format!(
                "Statut << {} >> : le plafond de securite sociale d'un intermittent depend de son statut (artiste : \
                 arrete du 24/01/1975, art. 3 ; technicien : R. 242-2 CSS), et ce statut n'est ni l'un ni l'autre.",
                autre))
        }
        _ => false,
    };
    if nat.mensuelle() {
        let tp = match plafond_temps_partiel(&ctx.entreprise, e, sal)? {
            PlafondTempsPartiel::Proratise => Some(duree_du_temps_partiel(e)?),
            _ => None,
        };
        return plafond_permanent(tuple(&e.debut_periode()?), tuple(&e.fin_periode()?), tp, &pss_du_contexte(ctx));
    }
    let ep = EngagementPlafond {
        mensuelle: nat.mensuelle(),
        artiste,
        debut_periode: tuple(&e.debut_periode()?),
        fin_periode: tuple(&e.fin_periode()?),
        debut: tuple(&e.debut),
        fin: tuple(&e.fin.unwrap_or(e.debut)),
        dates_travail: e.dates_travail.iter().map(tuple).collect(),
        jours: e.jours,
    };
    pour_paie(&ep, &pss_du_contexte(ctx))
}

/// Le plafond sur lequel se pose le forfait de sante d'un salarie au mois (`pss_periode`, la rubrique `mutuelle` et la
/// base 18 du bloc 79) : celui de la periode de paie, reduit au prorata des jours quand le contrat ne la couvre pas
/// (`plafond::plafond_permanent`), mais SANS la correction du temps partiel (lot D1, 13/10/2026).
///
/// LES TEXTES. Les accords de sante fixent une cotisation « au titre de la couverture du seul salarie » en pourcentage
/// du « plafond mensuel de la securite sociale » (IDCC 1285, accord du 30/01/2026, art. 6 ; IDCC 3090, accord du
/// 19/07/2024, art. 13 : `atelier/reference/sources/kali-1285-accord-frais-de-sante-20260130-20261011.md`,
/// `kali-3090-accord-sante-permanents-20240719-20261011.md`) : c'est la valeur mensuelle que l'arrete fixe chaque annee
/// (CSS L. 241-3, D. 242-17, D. 242-18). La correction du temps partiel est autre chose : R. 242-2, I, dernier alinea,
/// corrige « le plafond » retenu pour le calcul des cotisations plafonnees du salarie, a la duree de son contrat ; un
/// forfait n'est pas plafonne. Et les memes accords, apres CSS R. 242-1-6, 2°, c, ouvrent une dispense aux « salaries a
/// temps partiel [...] dont l'adhesion au systeme de garanties les conduirait a s'acquitter d'une cotisation au moins
/// egale a 10 % de leur remuneration brute » : la cotisation d'un temps partiel ne baisse donc pas avec son temps de
/// travail. Le prorata des jours d'un contrat qui ne couvre pas le mois reste une INFERENCE plus ancienne (V3e), qu'aucun
/// de ces textes ne tranche. La prevoyance, elle, porte sur des tranches : « la tranche 1 porte sur la remuneration egale
/// au plafond de la securite sociale vise a l'article L. 241-3 » (IDCC 1285, titre XII, `kali-1285-titres-ii-iii-x-xi-
/// xii-20260923.md`, l. 73), le plafond que R. 242-2 corrige ; elle suit le plafond corrige, comme la retraite
/// complementaire (`plafond_de_l_engagement`). Textes : `atelier/reference/sources/plafond-temps-partiel/LISEZ-MOI.md`.
pub fn plafond_de_la_sante(ctx: &Contexte, e: &Engagement) -> Resultat<Decimal> {
    plafond_permanent(tuple(&e.debut_periode()?), tuple(&e.fin_periode()?), None, &pss_du_contexte(ctx))
}

/// Le plafond d'un salarie a temps partiel, et ce que S21.G00.40.084 en declare.
///
/// LA PRORATISATION EST LA REGLE, Y RENONCER EST LE CHOIX (11/10/2026, lot R5a ; le lot V4b avait code l'inverse).
/// - CSS L. 242-8 : pour un salarie a temps partiel (L. 3123-1 C. trav.), « il est opere un abattement d'assiette »
///   destine a compenser l'ecart avec un temps complet -- un imperatif ; R. 242-2, I, dernier alinea, en donne la
///   mesure (`plafond::plafond_permanent`) ;
/// - fiche net-entreprises 3369 (03/04/2026) : « Par defaut, le salarie a temps partiel cotise uniquement a hauteur de
///   la quotite de travail de son contrat. L'employeur peut choisir de ne pas proratiser le plafond de Securite sociale
///   pour tous les salaries a temps partiel de son etablissement » ; BOSS, assiette generale, § 760 : « Le salarie a
///   temps partiel et l'employeur ont la possibilite de renoncer a cet ajustement » ;
/// - L. 242-10, 1° : L. 242-8 « n'est pas applicable » aux salaries dont l'emploi donne lieu a des taux, assiettes ou
///   montants specifiques ; BOSS § 1090 : ils « ne sont pas eligibles a la reduction du plafond », et « sont notamment
///   vises les artistes du spectacle ».
/// Textes : `atelier/reference/sources/plafond-temps-partiel/LISEZ-MOI.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlafondTempsPartiel {
    /// Un intermittent (contrat d'usage, sans duree du travail : 40.014 = 99), ou un temps plein : la question ne se
    /// pose pas, le plafond n'est pas corrige a la quotite, et 40.084 ne s'ecrit pas (norme 2027.1.2 : conditionnelle).
    NonConcerne,
    /// Temps partiel : le plafond se corrige a la duree du contrat (la regle).
    Proratise,
    /// Temps partiel, et l'etablissement a renonce a la proratisation (la fiche entreprise le dit) : le plafond entier.
    Entier,
    /// Temps partiel d'un artiste du spectacle : exclu de la proratisation (L. 242-10, 1° ; BOSS § 1090), le plafond
    /// entier quel que soit le choix de l'etablissement.
    Exclu,
}

/// Les valeurs de `plafond_temps_partiel` (fiche entreprise, `logiciel/donnees/champs-base.json`). Vide : la regle, le
/// plafond proratise. « entier » : l'etablissement renonce a la proratisation (fiche 3369).
pub const PLAFOND_PRORATISE: &str = "proratisé";
pub const PLAFOND_ENTIER: &str = "entier";

/// Le plafond du salarie `sal` (sa fiche telle que la paie la lit, pour son statut) a temps partiel de l'engagement `e`,
/// d'apres la fiche entreprise `ent` (`plafond_temps_partiel`). Le temps partiel est celui que la DSN declare en
/// S21.G00.40.014 (20) : une duree du contrat inferieure a la duree de reference (L. 3123-1 du code du travail,
/// `engagement::modalite_temps_travail`). Sans reponse dans la fiche, la regle : proratise (L. 242-8, fiche 3369). Un
/// artiste est exclu (L. 242-10, 1° ; BOSS § 1090) : la reponse de la fiche ne le concerne pas.
///
/// UNE DONNEE QUI A CHANGE DE SENS SANS CHANGER DE VALEURS : « proratisé » et « entier » disent la meme chose qu'au lot
/// V4b ; seule l'absence de reponse change (V4b : « l'employeur n'a pas choisi », le plafond entier et la DSN de 2027
/// refusee ; desormais : la regle, le plafond proratise). Rien n'est a convertir dans une fiche.
pub fn plafond_temps_partiel(ent: &Value, e: &Engagement, sal: &Value) -> Resultat<PlafondTempsPartiel> {
    if !e.nat()?.mensuelle() || e.modalite_temps_travail()? != "20" {
        return Ok(PlafondTempsPartiel::NonConcerne);
    }
    let renonce = match ent.get("plafond_temps_partiel") {
        None | Some(Value::Null) => false,
        Some(Value::String(t)) if t.trim().is_empty() || t.trim() == PLAFOND_PRORATISE => false,
        Some(Value::String(t)) if t.trim() == PLAFOND_ENTIER => true,
        Some(autre) => {
            return Err(format!(
                "Fiche entreprise, `plafond_temps_partiel` = {} : ce choix n'est pas connu. Le plafond de Securite sociale \
d'un salarie a temps partiel se proratise a la duree de son contrat (CSS L. 242-8 ; fiche net-entreprises 3369 : << par \
defaut >>) : laisser vide, ou << {} >>. << {} >> : l'etablissement renonce a la proratisation pour tous ses salaries a \
temps partiel (fiche 3369 ; BOSS, assiette generale, § 760).",
                autre, PLAFOND_PRORATISE, PLAFOND_ENTIER
            ))
        }
    };
    // Le statut se lit comme le bareme le lit (`plafond_de_l_engagement`, `reduction_generale`) : vide, un artiste.
    let statut = sal.get("statut").filter(|s| vrai(s)).map(en_texte).unwrap_or_else(|| "artiste".into());
    if statut == "artiste" {
        return Ok(PlafondTempsPartiel::Exclu);
    }
    Ok(if renonce { PlafondTempsPartiel::Entier } else { PlafondTempsPartiel::Proratise })
}

/// (duree du contrat, duree de reference) de la correction du plafond d'un temps partiel (R. 242-2, I, dernier alinea) :
/// la duree inscrite au contrat (S21.G00.40.013), rapportee a « la plus courte des durees mentionnees au 1° de
/// l'article L. 3123-1 du code du travail » -- la duree legale (151,67 h, `contrats-base.json`, L. 3121-27) ou, si
/// elle est plus courte, la duree collective de l'entreprise (S21.G00.40.012). Le texte ajoute a la duree du contrat
/// « le nombre d'heures complementaires » du mois : le moteur ne les distingue pas d'autres heures payees, et une
/// ligne d'heures en plus du mois refuse plutot que de les deviner.
pub fn duree_du_temps_partiel(e: &Engagement) -> Resultat<(Decimal, Decimal)> {
    let nat = e.nat()?;
    if let Some(l) = e.lignes.iter().find(|l| l.get("rubrique").and_then(Value::as_str) == Some("heure")) {
        return Err(format!(
            "Contrat {} : le plafond de Securite sociale de ce salarie a temps partiel se corrige a la duree de son \
contrat << majoree du nombre d'heures complementaires >> du mois (CSS R. 242-2, I, dernier alinea), et la paie porte des \
heures en plus du salaire mensuel ({} h) : heures complementaires ou non, le logiciel ne le sait pas. Refus : il ne les \
devine pas.",
            e.numero.clone().unwrap_or_else(|| "?".into()),
            l.get("quantite").map(en_texte).unwrap_or_default()
        ));
    }
    let legale = dec_f64(nat.quotite_reference_defaut.ok_or_else(|| format!("{} : pas de duree legale (`quotite_reference_defaut`)", nat.libelle))?)?;
    let reference = match e.quotite_reference()? {
        Some(r) => legale.min(dec_f64(r)?),
        None => legale,
    };
    Ok((e.quotite()?, reference))
}

impl Paie {
    /// `plafond.pour_paie(paie, ctx)` : le « Plafond Urssaf » du bas de bulletin.
    pub fn plafond_urssaf(&self, ctx: &Contexte) -> Resultat<Decimal> {
        // Un mois decoupe : la somme des plafonds de ses periodes (BOSS, assiette generale, § 730 : « les differentes
        // regles d'ajustement du plafond sont cumulables »).
        if !self.periodes.is_empty() {
            let mut total = Decimal::new(0, 2);
            for p in &self.periodes {
                total += p.plafond_urssaf(ctx)?;
            }
            return Ok(total);
        }
        plafond_de_l_engagement(ctx, &self.engagement, &self.salarie)
    }

    /// `oracle._paie` : la forme canonique d'une paie (`ctx` pour le plafond Urssaf).
    pub fn canon(&self, ctx: &Contexte) -> Resultat<Value> {
        let cle = |f: &Value| {
            let mut m = Map::new();
            m.insert("_cle".into(), f.get("_cle").cloned().unwrap_or(Value::Null));
            Value::Object(m)
        };
        let n = |x: Decimal| Value::String(nombre(x));
        let mut m = Map::new();
        m.insert("assiette_chomage".into(), n(self.assiette_chomage));
        m.insert(
            "avertissements".into(),
            Value::Array(self.avertissements.iter().map(|a| Value::String(a.texte.clone())).collect()),
        );
        m.insert("base_csg".into(), n(self.base_csg));
        m.insert("bases".into(), Value::Object(self.bases.iter().map(|(k, v)| (k.clone(), n(*v))).collect()));
        m.insert("brut".into(), n(self.brut));
        m.insert("cout_employeur".into(), n(self.cout_employeur));
        m.insert(
            "deduction_forfaitaire".into(),
            self.deduction_forfaitaire.as_ref().map_or(Value::Null, |d| d.canon()),
        );
        m.insert("engagement".into(), self.engagement.canon()?);
        m.insert("heures".into(), n(self.heures));
        m.insert("heures_absence".into(), n(self.heures_absence));
        m.insert("heures_travaillees".into(), n(self.heures_travaillees));
        m.insert("lignes".into(), Value::Array(self.lignes.iter().map(|l| l.canon()).collect::<Resultat<_>>()?));
        m.insert("net_a_payer".into(), n(self.net_a_payer));
        m.insert("net_imposable".into(), n(self.net_imposable));
        m.insert("net_social".into(), n(self.net_social));
        m.insert("numero_bulletin".into(), self.numero_bulletin.clone().map_or(Value::Null, Value::String));
        m.insert("reduction_generale".into(), self.reduction_generale.as_ref().map_or(Value::Null, |r| r.canon()));
        let remuneration = self.remuneration.iter().map(|l| {
            let mut c = l.canon();
            c["designation"] = Value::String(l.designation());
            c
        });
        m.insert("remuneration".into(), Value::Array(remuneration.collect()));
        m.insert("salaire_de_base".into(), n(self.salaire_de_base));
        m.insert("salaire_retabli".into(), n(self.salaire_retabli));
        m.insert("salarie".into(), cle(&self.salarie));
        m.insert("smic".into(), n(dec_f64(self.smic)?));
        m.insert("spectacle".into(), if vrai(&self.spectacle) { cle(&self.spectacle) } else { Value::Null });
        m.insert("total_pat".into(), n(self.total_pat));
        m.insert("total_sal".into(), n(self.total_sal));
        m.insert("plafond".into(), n(self.plafond));
        let urssaf = self.plafond_urssaf(ctx)?;
        m.insert("plafond_urssaf".into(), n(urssaf));
        // Un mois decoupe (lot R10) : ses periodes elementaires, chacune une paie. Absent d'un mois d'un seul tenant.
        if !self.periodes.is_empty() {
            m.insert("periodes".into(), Value::Array(self.periodes.iter().map(|p| p.canon(ctx)).collect::<Resultat<_>>()?));
        }
        Ok(Value::Object(m))
    }
}
