//! L'obligation d'emploi des travailleurs handicapes (OETH) : la declaration
//! annuelle que la DSN d'avril porte pour l'annee precedente
//! (`moteur/oeth.py`). Les chiffres viennent de `config/oeth.json`, par
//! millesime ; le calcul applique D5212-2, -20 a -24.

use droit::contexte::Contexte;
use socle::date::Jour;
use socle::fichiers::{lire_json, repr, repr_valeur, vrai};
use socle::num::{add, dec_f64, dec_py, eur2, mul, sub, Decimal, Resultat};
use serde_json::Value;
use std::collections::BTreeMap;

const SEUIL: i64 = 20;
const MOIS_DE_DECLARATION: &str = "04";
const CTP_CONTRIBUTION: &str = "730";
const CTP_ACCORD: &str = "740";

/// Les categories de D5212-23 et leur code en S21.G00.82.002.
const DEPENSES: [(&str, &str); 3] = [("accessibilite", "062"), ("maintien", "063"), ("accompagnement", "064")];

/// S21.G00.13.002 : les types de beneficiaires mis a disposition.
const TYPES_EXTERNES: [&str; 6] = ["01", "02", "03", "04", "05", "06"];

fn taux() -> Decimal {
    Decimal::from(6)
}

fn zero2() -> Decimal {
    socle::num::zero2()
}

/// Ce que la DSN d'avril porte pour un millesime.
#[derive(Clone, Debug)]
pub struct Declaration {
    pub millesime: i32,
    pub effectif: Decimal,
    pub obligation: Decimal,
    pub beneficiaires: Decimal,
    pub manquants: Decimal,
    pub coefficient: i64,
    pub smic: Decimal,
    pub brute: Decimal,
    pub nette: Decimal,
    /// Les deductions non plafonnees, telles qu'elles se declarent (code -> montant).
    pub deductions: BTreeMap<String, Decimal>,
    /// [(type, nombre)] des beneficiaires mis a disposition.
    pub externes: Vec<(String, Decimal)>,
    /// Le code de l'accord agree, ou `None`.
    pub accord: Option<String>,
    pub non_realisees: Decimal,
    pub source: String,
}

impl Declaration {
    /// La contribution reelle due (068), celle du CTP 730.
    pub fn due(&self) -> Decimal {
        self.nette
    }

    /// [(code, montant)] du bloc S21.G00.82, dans l'ordre des codes.
    pub fn blocs_82(&self) -> Vec<(String, Decimal)> {
        if self.accord.is_some() {
            return if self.non_realisees.is_zero() { vec![] } else { vec![("069".to_string(), self.non_realisees)] };
        }
        let mut l: Vec<(String, Decimal)> =
            self.deductions.iter().filter(|(_, m)| !m.is_zero()).map(|(c, m)| (c.clone(), *m)).collect();
        l.push(("065".into(), self.brute));
        l.push(("066".into(), self.nette));
        l.push(("068".into(), self.due()));
        l.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        l
    }

    /// Ce que la declaration ajoute au bordereau Urssaf, en euros (`Declaration.a_verser`).
    pub fn a_verser(&self) -> Decimal {
        self.bordereau().iter().fold(zero2(), |a, (_, m)| a + *m)
    }

    /// `Declaration.resume` : la phrase que `produire` dit au mois d'avril.
    pub fn resume(&self) -> String {
        if let Some(accord) = &self.accord {
            let non = if self.non_realisees.is_zero() {
                String::new()
            } else {
                format!(", {} EUR de depenses non realisees (CTP 740)", self.non_realisees)
            };
            return format!("OETH {} : accord agree {}{}.", self.millesime, accord, non);
        }
        format!(
            "OETH {} : effectif {}, {} beneficiaire(s) du(s), {} employe(s), {} manquant(s) x {} x SMIC {} = {} EUR brute, {} EUR nette (CTP 730).",
            self.millesime, self.effectif, self.obligation, self.beneficiaires, self.manquants, self.coefficient, self.smic, self.brute, self.nette
        )
    }

    /// [(ctp, montant)] a ajouter au bordereau Urssaf.
    pub fn bordereau(&self) -> Vec<(String, Decimal)> {
        if self.accord.is_some() {
            return if self.non_realisees.is_zero() { vec![] } else { vec![(CTP_ACCORD.to_string(), self.non_realisees)] };
        }
        if self.due().is_zero() {
            vec![]
        } else {
            vec![(CTP_CONTRIBUTION.to_string(), self.due())]
        }
    }
}

/// `Decimal(str(v))` d'une valeur JSON ; `None` si Python lèverait `InvalidOperation`.
fn decimal_de(v: &Value) -> Option<Decimal> {
    match v {
        Value::Number(_) | Value::String(_) => dec_py(v).ok(),
        _ => None,
    }
}

/// `_dec` : un nombre positif ou nul de `config/oeth.json`.
fn dec(v: &Value, nom: &str, millesime: i32) -> Resultat<Decimal> {
    let Some(x) = decimal_de(v) else {
        return Err(format!("config/oeth.json, {} : << {} >> n'est pas un nombre ({}).", millesime, nom, repr_valeur(v)));
    };
    if x.is_sign_negative() && !x.is_zero() {
        return Err(format!(
            "config/oeth.json, {} : << {} >> est negatif ({}).",
            millesime,
            nom,
            socle::date::en_texte(v)
        ));
    }
    Ok(x)
}

/// `donnees.get(cle, 0)`.
fn ou_zero(donnees: &Value, cle: &str) -> Value {
    donnees.get(cle).cloned().unwrap_or_else(|| Value::from(0))
}

/// D5212-20 et D5212-21.
pub fn coefficient(effectif: Decimal, sans_action: bool) -> i64 {
    if sans_action {
        1500
    } else if effectif >= Decimal::from(750) {
        600
    } else if effectif >= Decimal::from(250) {
        500
    } else {
        400
    }
}

/// La declaration d'un millesime, depuis son entree de `config/oeth.json` et le
/// SMIC horaire brut au 31/12 de ce millesime.
pub fn calculer(millesime: i32, donnees: &Value, smic: Decimal) -> Resultat<Declaration> {
    let ou = format!("config/oeth.json, {}", millesime);
    let manque: Vec<&str> = ["effectif_assujettissement", "boeth", "ecap", "source"]
        .into_iter()
        .filter(|k| matches!(donnees.get(*k), None | Some(Value::Null)) || donnees.get(*k) == Some(&Value::String(String::new())))
        .collect();
    if !manque.is_empty() {
        return Err(format!(
            "{} : il manque {}. Ce sont les chiffres que l'Urssaf transmet avant le 15 mars (D5212-5) -- \
effectif d'assujettissement, beneficiaires employes, effectif ECAP -- et d'ou ils viennent. \
Rien n'est ecrit.",
            ou,
            manque.join(", ")
        ));
    }
    let eff = dec(&donnees["effectif_assujettissement"], "effectif_assujettissement", millesime)?;
    let mut d = Declaration {
        millesime,
        effectif: eff,
        obligation: Decimal::ZERO,
        beneficiaires: Decimal::ZERO,
        manquants: Decimal::ZERO,
        coefficient: 0,
        smic,
        brute: zero2(),
        nette: zero2(),
        deductions: BTreeMap::new(),
        externes: vec![],
        accord: None,
        non_realisees: zero2(),
        source: socle::date::en_texte(&donnees["source"]),
    };

    let mut externes = vec![];
    match donnees.get("boeth_externes") {
        Some(Value::Array(l)) => {
            for x in l {
                let t = match x.get("type") {
                    None => String::new(),
                    Some(v) => socle::date::en_texte(v),
                };
                if !TYPES_EXTERNES.contains(&t.as_str()) {
                    return Err(format!(
                        "{} : type de beneficiaire externe {} inconnu (S21.G00.13.002 : {}).",
                        ou,
                        repr(&t),
                        TYPES_EXTERNES.join(", ")
                    ));
                }
                let nul = Value::Null;
                let n = dec(x.get("nombre").unwrap_or(&nul), "boeth_externes.nombre", millesime)?;
                if !n.is_zero() {
                    externes.push((t, n));
                }
            }
        }
        Some(v) if vrai(v) => return Err(format!("{} : 'boeth_externes' : {}", ou, socle::fichiers::attendu("une liste", socle::valeur::nom_type(v)))),
        _ => {}
    }
    d.externes = externes;

    let accord = match donnees.get("accord") {
        Some(v) if vrai(v) => socle::date::en_texte(v).trim().to_string(),
        _ => String::new(),
    };
    if !accord.is_empty() {
        if accord.chars().count() != 12 {
            return Err(format!(
                "{} : le code d'accord agree {} n'a pas douze caracteres (table AAETH, S21.G00.13.001).",
                ou,
                repr(&accord)
            ));
        }
        d.accord = Some(accord);
        d.non_realisees = eur2(dec(&ou_zero(donnees, "depenses_non_realisees"), "depenses_non_realisees", millesime)?);
        return Ok(d);
    }

    d.obligation = div_cent(mul(eff, taux())).trunc();
    if let Some(dus) = donnees.get("boeth_dus") {
        if !matches!(dus, Value::Null) && dus != &Value::String(String::new()) {
            let x = dec(dus, "boeth_dus", millesime)?;
            if x != d.obligation {
                return Err(format!(
                    "{} : l'Urssaf transmet {} beneficiaire(s) du(s), et 6 % de {} arrondi a l'entier \
inferieur (D5212-2) en donne {}. L'un des deux est mal recopie -- ou une regle que le moteur ne \
connait pas s'applique. Rien n'est ecrit.",
                    ou,
                    socle::date::en_texte(dus),
                    eff,
                    d.obligation
                ));
            }
        }
    }
    let mut ext_total = Decimal::ZERO;
    for (_, n) in &d.externes {
        ext_total = add(ext_total, *n);
    }
    d.beneficiaires = add(dec(&donnees["boeth"], "boeth", millesime)?, ext_total);
    d.manquants = if sub(d.obligation, d.beneficiaires) > Decimal::ZERO {
        sub(d.obligation, d.beneficiaires)
    } else {
        Decimal::ZERO
    };

    let sous_traitance = eur2(dec(&ou_zero(donnees, "sous_traitance"), "sous_traitance", millesime)?);
    let vide = Value::Object(Default::default());
    let depenses = match donnees.get("depenses") {
        Some(v) if vrai(v) => v,
        _ => &vide,
    };
    let Some(dep_obj) = depenses.as_object() else {
        return Err(format!("'depenses' : {}", socle::fichiers::attendu("un objet", socle::valeur::nom_type(depenses))));
    };
    let mut inconnues: Vec<&str> =
        dep_obj.keys().map(String::as_str).filter(|k| !DEPENSES.iter().any(|(c, _)| c == k)).collect();
    inconnues.sort();
    if !inconnues.is_empty() {
        return Err(format!(
            "{} : depenses {} inconnues. D5212-23 en vigueur n'en connait que trois (accessibilite, maintien, \
accompagnement) ; les codes 071 a 073 du cahier technique ne correspondent a aucun texte lu. \
Rien n'est ecrit.",
            ou,
            inconnues.join(", ")
        ));
    }

    let mut sans_action = donnees.get("aucune_action_depuis_plus_de_trois_ans").and_then(Value::as_bool);
    if d.beneficiaires.is_zero() && sous_traitance.is_zero() {
        if sans_action.is_none() {
            return Err(format!(
                "{} : aucun beneficiaire et aucune sous-traitance cette annee. La contribution passe a 1 500 \
fois le SMIC par manquant si cela dure depuis PLUS DE TROIS ANS (D5212-21), et 400 fois sinon : ecrire \
\"aucune_action_depuis_plus_de_trois_ans\": true ou false. Le logiciel ne connait pas assez le passe \
pour le dire. Rien n'est ecrit.",
                ou
            ));
        }
    } else {
        sans_action = Some(false);
    }
    d.coefficient = coefficient(eff, sans_action.unwrap_or(false));
    d.brute = eur2(mul(mul(d.manquants, Decimal::from(d.coefficient)), smic));

    let ecap = eur2(mul(mul(dec(&donnees["ecap"], "ecap", millesime)?, Decimal::from(17)), smic));
    d.deductions.insert("060".into(), ecap);
    d.deductions.insert("061".into(), sous_traitance);
    for (cle, code) in DEPENSES {
        let v = dep_obj.get(cle).cloned().unwrap_or_else(|| Value::from(0));
        d.deductions.insert(code.into(), eur2(dec(&v, &format!("depenses.{}", cle), millesime)?));
    }

    // Les plafonds, sur la contribution brute (D5212-22 et -23).
    let moitie_du_taux = mul(d.beneficiaires, Decimal::ONE_HUNDRED) >= socle::num::div(mul(eff, taux()), Decimal::from(2));
    let part = if moitie_du_taux { Decimal::new(75, 2) } else { Decimal::new(50, 2) };
    let plafond_st = eur2(mul(d.brute, part));
    let plafond_dep = eur2(mul(d.brute, Decimal::new(10, 2)));
    let mut total_dep = zero2();
    for (_, c) in DEPENSES {
        total_dep = add(total_dep, d.deductions[c]);
    }
    let retenu = add(add(ecap, sous_traitance.min(plafond_st)), total_dep.min(plafond_dep));
    let reste = sub(d.brute, retenu);
    d.nette = if reste > zero2() { reste } else { zero2() };
    Ok(d)
}

fn div_cent(x: Decimal) -> Decimal {
    socle::num::div(x, Decimal::ONE_HUNDRED)
}

/// La declaration OETH que la DSN de `mois` doit porter, ou `None` : seulement
/// en avril, pour l'annee precedente, et si l'employeur est assujetti. Refuse si
/// l'effectif dit qu'il l'est et que les chiffres de l'Urssaf ne sont pas ecrits.
pub fn declaration(ctx: &Contexte, mois: &str) -> Resultat<Option<Declaration>> {
    let car: Vec<char> = mois.chars().collect();
    let tranche = |a: usize, b: usize| -> String { car.iter().skip(a).take(b - a).collect() };
    if tranche(5, 7) != MOIS_DE_DECLARATION {
        return Ok(None);
    }
    let millesime = tranche(0, 4)
        .trim()
        .parse::<i32>()
        .map_err(|_| format!("nombre entier illisible : {}", repr(&tranche(0, 4))))?
        - 1;
    let chemin = ctx.racine.join("config").join("oeth.json");
    let fichier = if socle::fichiers::existe(&chemin) { lire_json(&chemin)? } else { Value::Object(Default::default()) };
    let donnees = match fichier.get(millesime.to_string()) {
        None | Some(Value::Null) => None,
        Some(v) => Some(v),
    };
    let Some(donnees) = donnees else {
        if !assujetti_selon_l_effectif(ctx, millesime)? {
            return Ok(None);
        }
        return Err(format!(
            "DSN de {} : l'entreprise a atteint {} salaries cinq annees de suite, elle doit la declaration \
OETH de {} (L5212-1, D5212-8), et config/oeth.json ne porte rien pour {}. Y ecrire les chiffres que \
l'Urssaf transmet avant le 15 mars : \"{}\": {{\"effectif_assujettissement\": ..., \"boeth\": ..., \
\"ecap\": ..., \"source\": \"...\"}}. Rien n'est ecrit.",
            mois, SEUIL, millesime, millesime, millesime
        ));
    };
    if donnees.get("assujetti") == Some(&Value::Bool(false)) {
        let pourquoi = match donnees.get("pourquoi") {
            Some(v) if vrai(v) => socle::date::en_texte(v),
            _ => String::new(),
        };
        if pourquoi.trim().is_empty() {
            return Err(format!(
                "config/oeth.json, {} : \"assujetti\": false sans \"pourquoi\". Dire la raison (delai de cinq \
ans d'une entreprise creee a vingt salaries, L5212-4 ; seuil pas encore franchi, L. 130-1).",
                millesime
            ));
        }
        return Ok(None);
    }
    if let Some(eff) = donnees.get("effectif_assujettissement") {
        if !matches!(eff, Value::Null) && eff != &Value::String(String::new()) {
            let x = decimal_de(eff).ok_or_else(|| format!("config/oeth.json, {} : effectif d'assujettissement illisible : {}", millesime, eff))?;
            let accord = donnees.get("accord").map_or(false, vrai);
            if x < Decimal::from(SEUIL) && !accord {
                return Ok(None);
            }
        }
    }
    Ok(Some(calculer(millesime, donnees, smic_au_31_decembre(ctx, millesime)?)?))
}

/// Le SMIC horaire brut au 31/12 du millesime (D5212-20, dernier alinea) : national,
/// lu dans le socle livre ; une annee sans socle se lit chez la compagnie.
pub fn smic_au_31_decembre(ctx: &Contexte, millesime: i32) -> Resultat<Decimal> {
    let jour = Jour::nouveau(millesime, 12, 31).ok_or_else(|| format!("annee hors limites : {}", millesime))?;
    let socle = ctx.socle_livre(millesime)?;
    let liste = match socle.as_ref().and_then(|s| s.get("smic_horaire")) {
        Some(l) if vrai(l) => l,
        _ => return dec_f64(ctx.smic(&jour)?),
    };
    let mut entrees: Vec<&Value> = liste.as_array().map(|a| a.iter().collect()).unwrap_or_default();
    entrees.sort_by_key(|e| socle::date::en_texte(&e["depuis"]));
    let mut val = None;
    for e in entrees {
        if Jour::lire(&socle::date::en_texte(&e["depuis"]))? <= jour {
            val = Some(&e["valeur"]);
        }
    }
    match val {
        Some(v) => dec_py(v),
        None => Err(format!("Aucun SMIC au {} dans le socle {}.", jour.iso(), millesime)),
    }
}

/// L'effectif dit-il l'entreprise assujettie pour ce millesime ? Le code A04 du
/// 118 l'emporte s'il est la et sous le seuil ; un effectif inconnu reste sous le seuil.
fn assujetti_selon_l_effectif(ctx: &Contexte, millesime: i32) -> Resultat<bool> {
    let chemin = ctx.racine.join("config").join("effectifs.json");
    let brut = if socle::fichiers::existe(&chemin) { lire_json(&chemin)? } else { Value::Object(Default::default()) };
    let a04 = brut
        .get("annees")
        .and_then(|a| a.get(millesime.to_string()))
        .and_then(|a| a.get("A04"))
        .and_then(|a| a.get("valeur"))
        .filter(|v| !v.is_null());
    if let Some(v) = a04 {
        if socle::num::flottant(v)? < SEUIL as f64 {
            return Ok(false);
        }
    }
    Ok(ctx.effectif().and_then(|e| e.atteint(SEUIL, millesime + 1)).unwrap_or(false))
}
