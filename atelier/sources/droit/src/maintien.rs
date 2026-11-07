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
use crate::clauses;
use socle::num::{div, Decimal, Resultat};

/// Ce qui a cause l'arret, quand c'est une maladie ou un accident : le champ `origine` d'un motif S21.G00.60.001 de
/// `arrets-base.json` (`crate::arrets`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Origine {
    Maladie,
    AccidentDuTravail,
    MaladieProfessionnelle,
    Trajet,
}

impl Origine {
    /// L'origine d'un motif d'arret (`arrets-base.json`), ou `None` pour un motif qui n'appelle pas de maintien (ou
    /// que la norme ne connait pas).
    pub fn du_motif(code: &str) -> Resultat<Option<Origine>> {
        Ok(crate::arrets::motif_d_arret(code)?.and_then(|m| m.origine))
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

fn manque(ou: &str, cle: &str) -> String {
    socle::echec::livree(ou, cle)
}

fn tranches(l: &[clauses::TrancheDeMaintien], ou: &str) -> Resultat<Vec<Tranche>> {
    l.iter()
        .map(|t| {
            let duree = match (t.pendant_ij, t.mois, t.jours) {
                (Some(true), _, _) => Duree::PendantLesIndemnites,
                (_, Some(m), _) => Duree::Mois(m),
                (_, None, Some(j)) => Duree::Jours(j),
                (_, None, None) => return Err(manque(ou, "jours")),
            };
            Ok(Tranche { duree, taux: t.taux.0 })
        })
        .collect()
}

/// Ce que la convention `idcc` garantit pour un arret de cette origine a cette anciennete (au premier jour de l'absence) ;
/// `None` quand elle ne prevoit rien (la loi seule s'applique). La clause de l'accident du travail vaut quand elle couvre
/// l'origine (`couvre` ; sans lui, l'accident du travail et la maladie professionnelle), sauf passe
/// `comme_maladie_apres_mois` d'anciennete ; celle de la maladie sinon (une maladie, un accident « non professionnel »).
pub fn conventionnel(idcc: &str, anciennete_mois: Decimal, origine: Origine) -> Resultat<Option<Droit>> {
    let Some(sec) = clauses::releve(idcc)?.and_then(|c| c.maintien_de_salaire) else { return Ok(None) };
    let accident = sec.accident_du_travail.filter(|at| {
        let couvre = at.couvre.as_ref().map_or(origine.professionnelle(), |l| l.contains(&origine));
        couvre && at.comme_maladie_apres_mois.is_none_or(|apres| anciennete_mois < apres.0)
    });
    let nom = clauses::nom(idcc);
    // (article, minimum d'anciennete, base, carence, plafond net, subrogation, tranches)
    let (article, minimum, base, carence, plafond_net, subrogation, liste) = match (accident, sec.maladie) {
        (Some(at), _) => {
            let ou = format!("clauses-conventionnelles-base.json, {}, art. {}", nom, at.article);
            let mut tr = at.tranches.as_slice();
            if let Some(deux_ans) = at.tranches_deux_ans.as_deref().filter(|t| !t.is_empty() && anciennete_mois >= Decimal::from(24)) {
                tr = deux_ans;
            }
            if let Some(apres) = at.tranches_apres.as_ref().filter(|a| anciennete_mois > a.anciennete_mois.0) {
                tr = apres.tranches.as_slice();
            }
            let liste = tranches(tr, &ou)?;
            let minimum = at.anciennete_mois_minimum.0;
            (at.article, minimum, at.base, at.carence_jours, at.plafond_net == Some(true), at.subrogation, Ok(liste))
        }
        (None, Some(m)) => {
            let ou = format!("clauses-conventionnelles-base.json, {}, art. {}", nom, m.article);
            let annees = div(anciennete_mois, Decimal::from(12));
            let mut lignes: Vec<&clauses::LigneDeMaintien> = m.par_anciennete.iter().collect();
            lignes.sort_by_key(|l| l.anciennete_annees_min.0);
            let liste = match lignes.into_iter().filter(|l| annees >= l.anciennete_annees_min.0).last() {
                None => Err(format!("{} (art. {}) : aucune tranche a cette anciennete.", nom, m.article)),
                Some(l) => match l.tranches.as_deref() {
                    Some(t) if !t.is_empty() => Ok(tranches(t, &ou)?),
                    _ => Ok(vec![
                        Tranche { duree: Duree::Jours(l.jours_90.ok_or_else(|| manque(&ou, "jours_90"))?), taux: Decimal::from(90) },
                        Tranche { duree: Duree::Jours(l.jours_66.ok_or_else(|| manque(&ou, "jours_66"))?), taux: Decimal::new(6666, 2) },
                    ]),
                },
            };
            let minimum = m.anciennete_mois_minimum.0;
            (m.article, minimum, m.base, m.carence_jours, m.plafond_net == Some(true), m.subrogation, liste)
        }
        (None, None) => return Ok(None),
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
    let tranches = match liste {
        Ok(t) => t,
        Err(pas_du) => return Ok(Some(Droit::PasDu(pas_du))),
    };
    Ok(Some(Droit::Du(Regime {
        texte: format!("{} (art. {})", nom, article),
        base: match base {
            clauses::BaseDuMaintien::Brut => Base::Brut,
            clauses::BaseDuMaintien::Net => Base::Net,
        },
        carence,
        tranches,
        plafond_net,
        subrogation: subrogation == Some(clauses::Subrogation::ToutSalarie),
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
