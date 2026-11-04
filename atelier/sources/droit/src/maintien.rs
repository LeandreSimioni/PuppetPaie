//! LE MAINTIEN DE SALAIRE D'UN ARRET DE TRAVAIL : ce que la loi et la convention garantissent, lu une fois, sous une seule
//! forme (`Regime`). Le module dit le droit ; il ne compte pas les jours d'un arret ni ne fait la paie
//! (`engagement::maintien`).
//!
//! - La loi : L1226-1 (une annee d'anciennete ; pas pour les intermittents), D1226-1 (90 % puis deux tiers de la
//!   remuneration brute), D1226-2 (dix jours de plus par cinq ans au-dela de la premiere annee, quatre-vingt-dix au plus),
//!   D1226-3 (sept jours de carence, sauf accident du travail et maladie professionnelle), D1226-4 (les douze mois
//!   anterieurs), D1226-5 (les indemnites journalieres se deduisent), D1226-8 (l'anciennete au premier jour de l'absence) :
//!   `atelier/reference/sources/code-du-travail/articles-20261015.md` ; les nombres dans `permanents-base.json`.
//! - La convention : `clauses-conventionnelles-base.json`, section `maintien_de_salaire` de chaque IDCC, chaque article lu
//!   dans le texte de la DILA (`atelier/reference/sources/kali-entrepot/`, `moteur releve-conventions article`). Une
//!   tranche dure des jours (`jours`), des mois de calendrier (`mois` : la 1285, X.4.6) ou tant que la caisse verse ses
//!   indemnites (`pendant_ij` : la 1285, X.4.7). `base` dit si le pourcentage porte sur le salaire brut ou net ;
//!   `plafond_net`, que « les sommes de toutes provenances » ne depassent pas le net qu'il aurait percu ; `subrogation`
//!   (`tout_salarie`), que la convention la donne a tout salarie qui a droit au maintien.

use crate::natures::Nature;
use socle::fichiers::vrai;
use socle::num::{dec_avec_echelle, div, Decimal, Resultat};
use serde_json::Value;

/// Ce qui a cause l'arret, lu sur son motif (S21.G00.60.001 : 01 maladie, 04 trajet, 05 maladie professionnelle, 06
/// accident du travail).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origine {
    Maladie,
    AccidentDuTravail,
    MaladieProfessionnelle,
    Trajet,
}

impl Origine {
    /// L'origine d'un motif d'arret (`arrets-base.json`), ou `None` pour un motif qui n'appelle pas de maintien.
    pub fn du_motif(code: &str) -> Option<Origine> {
        match code {
            "01" => Some(Origine::Maladie),
            "04" => Some(Origine::Trajet),
            "05" => Some(Origine::MaladieProfessionnelle),
            "06" => Some(Origine::AccidentDuTravail),
            _ => None,
        }
    }

    /// Le mot qui la nomme dans `couvre` d'une clause d'accident du travail.
    fn mot(self) -> &'static str {
        match self {
            Origine::Maladie => "maladie",
            Origine::AccidentDuTravail => "accident_du_travail",
            Origine::MaladieProfessionnelle => "maladie_professionnelle",
            Origine::Trajet => "trajet",
        }
    }

    /// L'accident du travail et la maladie professionnelle : pas de carence legale (D1226-3), et l'indemnite journaliere
    /// de la caisse court des le lendemain de l'arret (CSS L433-1), au lieu du quatrieme jour (R323-1).
    pub fn professionnelle(self) -> bool {
        matches!(self, Origine::AccidentDuTravail | Origine::MaladieProfessionnelle)
    }
}

/// Combien dure une tranche.
#[derive(Debug, Clone, PartialEq)]
pub enum Duree {
    Jours(i64),
    /// Des mois de calendrier, de date a date depuis le premier jour qu'elle indemnise.
    Mois(i64),
    /// Tant que la caisse verse ses indemnites journalieres : jusqu'a la fin de l'arret.
    PendantLesIndemnites,
}

/// Une tranche : sa duree et son pourcentage (66,66 se lit deux tiers : `fraction`).
#[derive(Debug, Clone, PartialEq)]
pub struct Tranche {
    pub duree: Duree,
    pub taux: Decimal,
}

/// Sur quoi porte le pourcentage garanti.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Base {
    Brut,
    Net,
}

/// Ce qu'un texte garantit pour un arret, a l'anciennete du salarie.
#[derive(Debug, Clone, PartialEq)]
pub struct Regime {
    /// Le texte, pour les phrases : « la loi (L1226-1) », « la convention 1285 (art. X.4.6) ».
    pub texte: String,
    pub base: Base,
    pub carence: i64,
    pub tranches: Vec<Tranche>,
    /// « un montant total superieur a la remuneration nette qu'il aurait effectivement percue » est exclu.
    pub plafond_net: bool,
    /// La convention donne la subrogation a tout salarie qui a droit au maintien.
    pub subrogation: bool,
}

/// Le droit d'un salarie a un regime, ou pourquoi il n'en a pas.
#[derive(Debug, Clone, PartialEq)]
pub enum Droit {
    Du(Regime),
    PasDu(String),
}

/// Le pourcentage en fraction : deux tiers exacts pour 66,66 (D1226-1, 2°, « deux tiers ») et 66,66 de la 3090 et de la
/// 2642 (« 2/3 », « 66,66 % » : le meme deux tiers ecrit au centieme) ; le taux sur cent sinon.
pub fn fraction(taux: Decimal) -> Decimal {
    if taux == Decimal::new(6666, 2) {
        div(Decimal::from(2), Decimal::from(3))
    } else {
        div(taux, Decimal::ONE_HUNDRED)
    }
}

// ------------------------------------------------------------------ la loi

/// Ce que la loi garantit (L1226-1, D1226-1 a D1226-3, D1226-8) a un salarie de cette nature et de cette anciennete au
/// premier jour de l'absence.
pub fn legal(nature: &Nature, anciennete_mois: Decimal, origine: Origine) -> Resultat<Droit> {
    if let Some(refus) = crate::permanents::droit_au_maintien(Some(nature), anciennete_mois)? {
        return Ok(Droit::PasDu(refus));
    }
    let carence = crate::permanents::jours_de_carence(origine.professionnelle(), origine == Origine::Trajet)?;
    let tranches = crate::permanents::duree_du_maintien(anciennete_mois)?
        .into_iter()
        .map(|t| Tranche { duree: Duree::Jours(t.jours), taux: t.taux })
        .collect();
    Ok(Droit::Du(Regime {
        texte: "la loi (L1226-1)".into(),
        base: Base::Brut,
        carence,
        tranches,
        plafond_net: false,
        subrogation: false,
    }))
}

// ------------------------------------------------------------------ la convention

fn get<'a>(v: &'a Value, k: &str) -> &'a Value {
    v.get(k).unwrap_or(&Value::Null)
}

fn nombre(v: &Value, k: &str, ou: &str) -> Resultat<Decimal> {
    match v.get(k) {
        Some(x) if !x.is_null() => dec_avec_echelle(x).map_err(|_| socle::echec::livree(ou, k)),
        _ => Err(socle::echec::livree(ou, k)),
    }
}

fn entier(v: &Value, k: &str, ou: &str) -> Resultat<i64> {
    use rust_decimal::prelude::ToPrimitive;
    nombre(v, k, ou)?.trunc().to_i64().ok_or_else(|| socle::echec::livree(ou, k))
}

fn tranche(t: &Value, ou: &str) -> Resultat<Tranche> {
    let taux = nombre(t, "taux", ou)?;
    let duree = if get(t, "pendant_ij").as_bool() == Some(true) {
        Duree::PendantLesIndemnites
    } else if t.get("mois").is_some() {
        Duree::Mois(entier(t, "mois", ou)?)
    } else {
        Duree::Jours(entier(t, "jours", ou)?)
    };
    Ok(Tranche { duree, taux })
}

fn tranches(v: &Value, ou: &str) -> Resultat<Vec<Tranche>> {
    v.as_array().ok_or_else(|| format!("{} : les tranches ne sont pas une liste.", ou))?.iter().map(|t| tranche(t, ou)).collect()
}

/// La section de la convention qui vaut pour cette origine : `accident_du_travail` quand elle couvre l'origine (`couvre` ;
/// sans lui, l'accident du travail et la maladie professionnelle), sauf passe `comme_maladie_apres_mois` d'anciennete ;
/// `maladie` sinon (une maladie, un accident « non professionnel »).
fn section<'a>(sec: &'a Value, origine: Origine, anciennete_mois: Decimal) -> Resultat<(&'a Value, bool)> {
    let at = get(sec, "accident_du_travail");
    if vrai(at) {
        let couvre = match at.get("couvre").and_then(Value::as_array) {
            Some(l) => l.iter().any(|x| x.as_str() == Some(origine.mot())),
            None => origine.professionnelle(),
        };
        let apres = get(at, "comme_maladie_apres_mois");
        let devenu_maladie = !apres.is_null() && anciennete_mois >= dec_avec_echelle(apres)?;
        if couvre && !devenu_maladie {
            return Ok((at, true));
        }
    }
    Ok((get(sec, "maladie"), false))
}

/// Ce que la convention `idcc` garantit pour un arret de cette origine a cette anciennete (au premier jour de l'absence) ;
/// `None` quand elle ne prevoit rien (la loi seule s'applique).
pub fn conventionnel(idcc: &str, anciennete_mois: Decimal, origine: Origine) -> Resultat<Option<Droit>> {
    let Some(conv) = crate::clauses::releve(idcc)? else { return Ok(None) };
    let sec = get(&conv, "maintien_de_salaire");
    let (s, at) = section(sec, origine, anciennete_mois)?;
    if !vrai(s) {
        return Ok(None);
    }
    let nom = crate::clauses::nom(idcc);
    let article = socle::date::en_texte(get(s, "article"));
    let ou = format!("clauses-conventionnelles-base.json, {}, art. {}", nom, article);
    // Sans minimum ecrit, l'accident du travail garde l'annee de L1226-1 : l'extension de ces clauses se fait « sous
    // reserve du respect des dispositions des articles L. 1226-1, D. 1226-1 et D. 1226-2 » (3090, 11.4 ; 2642, VIII.3).
    let minimum = match s.get("anciennete_mois_minimum") {
        Some(m) if !m.is_null() => dec_avec_echelle(m)?,
        _ if at => Decimal::from(12),
        _ => Decimal::ZERO,
    };
    if anciennete_mois < minimum {
        return Ok(Some(Droit::PasDu(format!(
            "{} (art. {}) demande {} mois d'anciennete ; il y en a {}.",
            nom,
            article,
            socle::texte::virgule(minimum),
            socle::texte::virgule(anciennete_mois.round_dp(1))
        ))));
    }
    let liste = if at {
        let mut tr = get(s, "tranches");
        if vrai(get(s, "tranches_deux_ans")) && anciennete_mois >= Decimal::from(24) {
            tr = get(s, "tranches_deux_ans");
        }
        let apres = get(s, "tranches_apres");
        if vrai(apres) && anciennete_mois > nombre(apres, "anciennete_mois", &ou)? {
            tr = get(apres, "tranches");
        }
        tranches(tr, &ou)?
    } else {
        let annees = div(anciennete_mois, Decimal::from(12));
        let mut lignes: Vec<&Value> = get(s, "par_anciennete").as_array().map_or_else(Vec::new, |a| a.iter().collect());
        let mut cles = Vec::new();
        for l in &lignes {
            cles.push(nombre(l, "anciennete_annees_min", &ou)?);
        }
        let mut ordre: Vec<usize> = (0..lignes.len()).collect();
        ordre.sort_by(|a, b| cles[*a].cmp(&cles[*b]));
        lignes = ordre.into_iter().map(|i| lignes[i]).collect();
        let mut ligne: Option<&Value> = None;
        for l in lignes {
            if annees >= nombre(l, "anciennete_annees_min", &ou)? {
                ligne = Some(l);
            }
        }
        let Some(l) = ligne else {
            return Ok(Some(Droit::PasDu(format!("{} (art. {}) : aucune tranche a cette anciennete.", nom, article))));
        };
        if vrai(get(l, "tranches")) {
            tranches(get(l, "tranches"), &ou)?
        } else {
            vec![
                Tranche { duree: Duree::Jours(entier(l, "jours_90", &ou)?), taux: Decimal::from(90) },
                Tranche { duree: Duree::Jours(entier(l, "jours_66", &ou)?), taux: Decimal::new(6666, 2) },
            ]
        }
    };
    let base = match get(s, "base").as_str() {
        Some("net") => Base::Net,
        Some("brut") => Base::Brut,
        _ => return Err(socle::echec::livree(&ou, "base")),
    };
    Ok(Some(Droit::Du(Regime {
        texte: format!("{} (art. {})", nom, article),
        base,
        carence: match s.get("carence_jours") {
            Some(c) if !c.is_null() => entier(s, "carence_jours", &ou)?,
            _ => 0,
        },
        tranches: liste,
        plafond_net: get(s, "plafond_net").as_bool() == Some(true),
        subrogation: get(s, "subrogation").as_str() == Some("tout_salarie"),
    })))
}

// ------------------------------------------------------------------ la phrase

/// Le regime en une phrase : « la convention 1285 (art. X.4.6) : 3 mois à 100 % puis 3 mois à 75 % du salaire brut,
/// sans carence ».
pub fn phrase(r: &Regime) -> String {
    let tr: Vec<String> = r
        .tranches
        .iter()
        .map(|t| {
            let pct = socle::texte::virgule(t.taux);
            match t.duree {
                Duree::Jours(j) => format!("{} jours à {} %", j, pct),
                Duree::Mois(m) => format!("{} mois à {} %", m, pct),
                Duree::PendantLesIndemnites => format!("{} % tant que la caisse indemnise", pct),
            }
        })
        .collect();
    format!(
        "{} : {} du salaire {}, {}",
        r.texte,
        tr.join(" puis "),
        if r.base == Base::Net { "net" } else { "brut" },
        if r.carence == 0 { "sans carence".to_string() } else { format!("après {} jours de carence", r.carence) }
    )
}
