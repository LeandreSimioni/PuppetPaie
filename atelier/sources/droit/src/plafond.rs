//! Le « Plafond Urssaf » du bas de bulletin : le plafond de securite sociale d'une paie (L. 241-3 CSS), celui sur
//! lequel s'assoient la vieillesse plafonnee, le FNAL et le chomage d'un intermittent (`calcul::calculer`, 7.6).
//!
//! ```text
//! artiste, periode d'engagement continu de moins de cinq jours (du debut a la fin du contrat, jours calendaires) :
//!     plafond = PSS horaire x 12 x journees de travail accomplies dans la paie (dates distinctes)
//! autre intermittent (artiste engage cinq jours ou plus, technicien) :
//!     plafond = eur( somme, pour chaque jour ayant donne lieu a remuneration dans la paie (dates distinctes),
//!                    de PSS mensuel de son annee / jours de son mois )
//! permanent (mensualise) :
//!     plafond = eur( PSS mensuel x jours de la periode de paie couverts par le contrat / jours du mois )
//! ```
//!
//! Textes (`atelier/reference/sources/`) :
//! - arrete du 24/01/1975, art. 3, al. 2 (`arrete-1975-artistes-du-spectacle/articles-20261010.md`) : « Pour les
//!   periodes d'engagement continu inferieures a cinq jours, le plafond [...] applicable a chaque journee de travail
//!   accomplie par un artiste du spectacle pour un meme employeur est egal a douze fois le plafond horaire, quels que
//!   soient le nombre d'heures et la nature du travail effectues dans ladite journee. » Une journee, pas un cachet ;
//!   « est egal », pas un plancher ; la duree de l'engagement, pas une suite de jours travailles. L'arrete ne vise que
//!   les artistes (art. 1 : 15° de L. 311-3 CSS) ;
//! - BOSS, assiette generale, version du 01/08/2026, § 1090 (`boss/boss-assiette-generale-20260801.txt`,
//!   l. 1472-1483) : les artistes du spectacle « ne sont pas eligibles a la reduction du plafond » ; « Toutefois, il est
//!   admis que : pour les salaries intermittents, le plafond soit proratise pour ne tenir compte que des jours ayant
//!   donne lieu a remuneration ». Tolerance appliquee (le BOSS est opposable) a tout
//!   intermittent hors de l'arrete de 1975 ; le prorata se fait comme au § 700 (« valeur mens. du plafond x (nombre de
//!   jours de la periode d'emploi / nombre de jours calendaires du mois) »), les jours de la periode d'emploi etant
//!   ici les jours remuneres de la paie. Un jour remunere se lit sur les lignes datees ; il ne se devine pas ;
//! - CSS R. 242-2, I, al. 2 et 3 (`code-de-la-securite-sociale/articles-20261010.md`) : le plafond « retenu pour
//!   chaque paie », « reduit[s] a due proportion du nombre de jours de la periode pendant laquelle les personnes sont
//!   employees » : la regle d'un permanent, sur la periode de sa paie ;
//! - CSS D. 242-19 : la valeur horaire (dans le bareme, `plafond_securite_sociale`).
//!
//! Les plafonds (PSS) ne sont pas dans le code : le module recoit un resolveur `annee -> (mensuel, horaire)`
//! (`Contexte::plafond_securite_sociale` en production). Un refus du resolveur passe tel quel. Les dates sont des
//! `(an, mois, jour)`.

use socle::num::{add, div, eur, mul, Decimal, Resultat};
use std::collections::{BTreeSet, HashMap};

/// Une date civile : (an, mois, jour).
pub type Jour = (i32, u32, u32);

/// Un resolveur de PSS : annee -> (mensuel, horaire). `Err` = `PlafondInconnu`.
pub type Pss<'a> = &'a dyn Fn(i32) -> Resultat<(Decimal, Decimal)>;

/// Arrete du 24/01/1975, art. 3, al. 2 : « periodes d'engagement continu inferieures a cinq jours ».
pub const JOURS_ENGAGEMENT_CONTINU: i64 = 5;

/// Arrete du 24/01/1975, art. 3, al. 2 : « douze fois le plafond horaire » par journee de travail.
pub const HEURES_PAR_JOURNEE: i64 = 12;

/// `PlafondInconnu` : le message quand une annee manque a la table.
pub fn plafond_inconnu(annee: i32) -> String {
    format!(
        "Pas de plafond de Securite sociale pour {a}.\n\
         Le relever sur la table des parametres du BOSS (codes \
         PSS_M_METROP et PSS_H_METROP) et le poser dans \
         config/baremes/{a}.json, section plafond_securite_sociale \
         ({{\"mensuel\": ..., \"horaire\": ...}}).",
        a = annee)
}

/// Un resolveur tire d'une table `{annee: (mensuel, horaire)}` (celle des tests).
pub fn pss_depuis_table(table: &HashMap<i32, (i64, i64)>) -> impl Fn(i32) -> Resultat<(Decimal, Decimal)> + '_ {
    move |annee| match table.get(&annee) {
        Some((m, h)) => Ok((Decimal::from(*m), Decimal::from(*h))),
        None => Err(plafond_inconnu(annee)),
    }
}

// ---------------------------------------------------------------- les dates

fn jours_du_mois(an: i32, mois: u32) -> u32 {
    match mois {
        4 | 6 | 9 | 11 => 30,
        2 if (an % 4 == 0 && an % 100 != 0) || an % 400 == 0 => 29,
        2 => 28,
        _ => 31,
    }
}

/// Numero du jour (algorithme du jour civil) : la difference de deux numeros
/// est le nombre de jours entre les dates.
fn numero(j: Jour) -> i64 {
    let (a, m, d) = (j.0 as i64, j.1 as i64, j.2 as i64);
    let a = if m <= 2 { a - 1 } else { a };
    let ere = a.div_euclid(400);
    let aa = a - ere * 400;
    let mp = (m + 9) % 12;
    let jda = (153 * mp + 2) / 5 + d - 1;
    let jj = aa * 365 + aa / 4 - aa / 100 + jda;
    ere * 146097 + jj
}

fn iso(j: Jour) -> String {
    format!("{:04}-{:02}-{:02}", j.0, j.1, j.2)
}

// ----------------------------------------------------------------- le calcul

/// (PSS mensuel, PSS horaire) de l'annee, tronques a l'entier.
fn valeurs(annee: i32, pss: Pss) -> Resultat<(Decimal, Decimal)> {
    let (m, h) = pss(annee)?;
    Ok((m.trunc(), h.trunc()))
}

/// PSS mensuel x jours calendaires du contrat / jours du mois, mois par mois,
/// somme. Les deux bornes sont incluses.
pub fn prorata_calendaire(debut: Jour, fin: Jour, pss: Pss) -> Resultat<Decimal> {
    if fin < debut {
        return Err(format!("fin {} anterieure au debut {}", iso(fin), iso(debut)));
    }
    let mut total = Decimal::ZERO;
    let mut jour = debut;
    while jour <= fin {
        let dans_le_mois = jours_du_mois(jour.0, jour.1);
        let derniere = (jour.0, jour.1, dans_le_mois);
        let borne = fin.min(derniere);
        let jours = borne.2 - jour.2 + 1;
        let (mensuel, _) = valeurs(jour.0, pss)?;
        total = add(total, div(mul(mensuel, Decimal::from(jours)), Decimal::from(dans_le_mois)));
        jour = if borne.1 == 12 {
            (borne.0 + 1, 1, 1)
        } else {
            (borne.0, borne.1 + 1, 1)
        };
    }
    Ok(total)
}


/// Le nombre de jours calendaires de la periode d'engagement continu, bornes incluses (du 24 au 24 fait un jour).
pub fn duree_engagement(debut: Jour, fin: Jour) -> Resultat<i64> {
    if fin < debut {
        return Err(format!("fin {} anterieure au debut {}", iso(fin), iso(debut)));
    }
    Ok(numero(fin) - numero(debut) + 1)
}

/// Vrai quand l'arrete du 24/01/1975, art. 3, al. 2 s'applique : un artiste engage moins de cinq jours continus.
pub fn engagement_court(debut: Jour, fin: Jour, artiste: bool) -> Resultat<bool> {
    Ok(artiste && duree_engagement(debut, fin)? < JOURS_ENGAGEMENT_CONTINU)
}

/// PSS horaire x 12 par journee de travail accomplie (dates distinctes : deux cachets le meme jour font une
/// journee), au PSS horaire de l'annee de chaque journee. Arrete du 24/01/1975, art. 3, al. 2.
pub fn plafond_journees(dates: &[Jour], pss: Pss) -> Resultat<Decimal> {
    let journees: BTreeSet<Jour> = dates.iter().copied().collect();
    let mut total = Decimal::ZERO;
    for j in journees {
        let (_, horaire) = valeurs(j.0, pss)?;
        total = add(total, mul(horaire, Decimal::from(HEURES_PAR_JOURNEE)));
    }
    Ok(total)
}

/// BOSS, assiette generale, § 1090 (tolerance) et § 700 (la formule) : pour chaque jour ayant donne lieu a
/// remuneration (dates distinctes), mois par mois : PSS mensuel de l'annee x jours remuneres du mois / jours du mois ;
/// la somme, non arrondie (la forme de `prorata_calendaire`).
pub fn plafond_jours_remuneres(dates: &[Jour], pss: Pss) -> Resultat<Decimal> {
    let jours: BTreeSet<Jour> = dates.iter().copied().collect();
    let mut par_mois: std::collections::BTreeMap<(i32, u32), i64> = Default::default();
    for j in jours {
        *par_mois.entry((j.0, j.1)).or_default() += 1;
    }
    let mut total = Decimal::ZERO;
    for ((an, mois), n) in par_mois {
        let (mensuel, _) = valeurs(an, pss)?;
        total = add(total, div(mul(mensuel, Decimal::from(n)), Decimal::from(jours_du_mois(an, mois))));
    }
    Ok(total)
}

/// Le refus d'un intermittent dont les journees de travail ne se lisent pas.
fn journees_illisibles(debut: Jour, fin: Jour, court: bool, dites: i64, datees: usize) -> String {
    let regle = if court {
        "le plafond d'un artiste engage moins de cinq jours se compte par journee de travail accomplie, repetitions \
         comprises (arrete du 24/01/1975, art. 3, al. 2)"
    } else {
        "le plafond d'un intermittent se proratise sur les jours ayant donne lieu a remuneration (BOSS, assiette \
         generale, § 1090)"
    };
    format!(
        "Contrat du {} au {} : {}, et la saisie dit {} jour(s) travaille(s) pour {} date(s). Dater chaque journee de \
         travail sur les lignes de paie : elle ne se devine pas.",
        iso(debut), iso(fin), regle, dites, datees)
}

/// Le plafond d'un contrat d'intermittent, arrondi au centime, pour les journees `dates` (celles d'une paie, ou de
/// tout le contrat). `artiste` : le statut (l'arrete de 1975 ne vise que les artistes). Artiste engage moins de cinq
/// jours : 12 x PSS horaire par journee (arrete de 1975). Sinon : le prorata du PSS mensuel sur les jours remuneres
/// (BOSS § 1090). Sans date, ou avec une date hors du contrat : refus.
pub fn plafond_urssaf(debut: Jour, fin: Jour, artiste: bool, pss: Pss, dates: &[Jour]) -> Resultat<Decimal> {
    let court = engagement_court(debut, fin, artiste)?;
    if dates.is_empty() {
        return Err(journees_illisibles(debut, fin, court, 0, 0));
    }
    if let Some(j) = dates.iter().find(|j| **j < debut || **j > fin) {
        return Err(format!("journee de travail le {} hors du contrat du {} au {}", iso(*j), iso(debut), iso(fin)));
    }
    if court {
        Ok(eur(plafond_journees(dates, pss)?))
    } else {
        Ok(eur(plafond_jours_remuneres(dates, pss)?))
    }
}

/// Le plafond d'un permanent (mensualise) pour sa periode de paie, arrondi au centime : le prorata calendaire de
/// R. 242-2, I, al. 3 (`prorata_calendaire`), et, quand l'employeur l'a choisi pour un salarie a temps partiel, la
/// correction du dernier alinea du meme I : « l'employeur est en droit de corriger le plafond, sans pouvoir augmenter sa
/// valeur mensuelle, a due proportion de la duree de travail inscrite a leur contrat de travail [...], majoree du
/// nombre d'heures complementaires [...], rapportee a celle correspondant a la plus courte des durees mentionnees au 1°
/// de l'article L. 3123-1 du code du travail » (`code-de-la-securite-sociale/articles-20261010.md`, R242-2 ; BOSS,
/// assiette generale, § 760-770, `boss/boss-assiette-generale-20260801.txt` l. 1033-1072 : « valeur mens. du plafond x
/// (duree contractuelle + heures complementaires) / duree legale du travail », et l'exemple « Plafond mensuel x
/// (121,33 h / 151,67 h) x (12 jours / 31 jours) »). `temps_partiel` : (duree du contrat, duree de reference), `None`
/// sans correction. Le rapport ne depasse jamais 1. Textes : `atelier/reference/sources/plafond-temps-partiel/`.
pub fn plafond_permanent(debut_periode: Jour, fin_periode: Jour, temps_partiel: Option<(Decimal, Decimal)>, pss: Pss) -> Resultat<Decimal> {
    let prorata = prorata_calendaire(debut_periode, fin_periode, pss)?;
    match temps_partiel {
        Some((contrat, reference)) if contrat < reference => {
            if reference <= Decimal::ZERO {
                return Err(format!("duree de reference du temps partiel nulle ou negative : {}", reference));
            }
            Ok(eur(div(mul(prorata, contrat), reference)))
        }
        _ => Ok(eur(prorata)),
    }
}

/// Ce que `pour_paie` lit de l'engagement.
#[derive(Debug, Clone)]
pub struct EngagementPlafond {
    /// La nature du contrat est payee au mois (`Nature::mensuelle`).
    pub mensuelle: bool,
    /// Le statut du salarie est `artiste` (vide compris) ; `false` : `technicien`.
    pub artiste: bool,
    pub debut_periode: Jour,
    pub fin_periode: Jour,
    pub debut: Jour,
    pub fin: Jour,
    /// Les jours travailles de la paie, dates (les dates distinctes de toutes les lignes).
    pub dates_travail: Vec<Jour>,
    /// Les jours travailles que la saisie dit (saisis, sinon les dates distinctes).
    pub jours: i64,
}

/// Le plafond d'UNE paie (R. 242-2 : le plafond « retenu pour chaque paie »). Un permanent : le prorata de droit
/// commun sur la periode de paie du mois. Un intermittent : les journees datees de CETTE paie (aucune journee dans le
/// mois, aucun plafond) : 12 x horaire chacune pour un artiste engage moins de cinq jours, sinon le prorata mensuel
/// des jours remuneres (BOSS § 1090). Une journee dite et non datee refuse, une date hors de la periode de paie aussi.
pub fn pour_paie(e: &EngagementPlafond, pss: Pss) -> Resultat<Decimal> {
    if e.mensuelle {
        return Ok(eur(prorata_calendaire(e.debut_periode, e.fin_periode, pss)?));
    }
    let court = engagement_court(e.debut, e.fin, e.artiste)?;
    let datees = e.dates_travail.iter().collect::<BTreeSet<_>>().len();
    if e.jours != datees as i64 {
        return Err(journees_illisibles(e.debut, e.fin, court, e.jours, datees));
    }
    if datees == 0 {
        return Ok(eur(Decimal::ZERO));
    }
    if let Some(j) = e.dates_travail.iter().find(|j| **j < e.debut_periode || **j > e.fin_periode) {
        return Err(format!(
            "journee de travail le {} hors de la periode de paie du {} au {} : le plafond se retient pour chaque paie \
             (R. 242-2 CSS), sur les jours de cette paie",
            iso(*j), iso(e.debut_periode), iso(e.fin_periode)));
    }
    plafond_urssaf(e.debut, e.fin, e.artiste, pss, &e.dates_travail)
}
