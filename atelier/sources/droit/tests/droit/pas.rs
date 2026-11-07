//! Le prelevement a la source contre `pas/` : la grille de l'article 204 H du CGI (`taux_par_defaut`, `du_versement`),
//! de 0 a 10 000 EUR, de part et d'autre des changements de bareme. Un cas est un bareme, un jour, une zone (et, pour un
//! versement, un taux recu et un contrat court) ; il rend un resultat par montant de `MONTANTS`, ecrit une fois quand ils
//! sont tous le meme. `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit les references.

use crate::outils::typee::{cas_de, code_livre, jouer, lire};
use droit::forme_bareme::{GrillePas, NombreDate, PasParDefaut, Pile};
use droit::fiche::Montant;
use droit::pas::{du_versement, taux_par_defaut, ErreurPas, Triplet, Versement};
use serde::{Deserialize, Serialize};
use socle::date::Jour;
use socle::forme::Nombre;
use socle::num::Decimal;
use std::str::FromStr;

/// Les montants de la grille, de part et d'autre de chaque seuil des baremes de 2025 et 2026.
const MONTANTS: [&str; 40] = [
    "0", "0.01", "100", "747.99", "748", "765.99", "766", "1000", "1590.99", "1591", "1619.99", "1620", "1634.99", "1635",
    "1635.01", "1650.50", "1697.99", "1698", "1806.995", "1807", "2000", "2059.99", "2060", "2500", "2737.5", "2738",
    "3134.99", "3135", "3570.99", "3571", "4018.5", "4019", "5000", "5623.995", "5624", "7036.99", "7037", "8789", "10000",
    "-50",
];

fn montants() -> Vec<Decimal> {
    MONTANTS.iter().map(|m| Decimal::from_str(m).unwrap()).collect()
}

/// Le bareme d'un cas : un socle livre (`brut-2025`, `brut-2026`), sans section du PAS (`vide`), ou sans grille.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
enum Bareme {
    #[serde(rename = "brut-2025")]
    Brut2025,
    #[serde(rename = "brut-2026")]
    Brut2026,
    #[serde(rename = "vide")]
    Vide,
    #[serde(rename = "sans-grille")]
    SansGrille,
}

/// Ce que le PAS lit d'un socle livre : sa section `pas_taux_par_defaut`, l'abattement en liste datee.
#[derive(Deserialize)]
struct SocleDuPas {
    pas_taux_par_defaut: SectionDuPas,
}

#[derive(Deserialize)]
struct SectionDuPas {
    abattement_contrats_courts: Vec<AbattementDate>,
    grilles: Vec<GrillePas>,
}

#[derive(Deserialize)]
struct AbattementDate {
    depuis: Jour,
    valeur: Nombre,
}

/// La pile d'un bareme au jour `le`, pour ce que le PAS en lit : l'abattement resolu a ce jour (la derniere valeur dont
/// le `depuis` ne le depasse pas, toutes sans jour), comme `Contexte::bareme` le resout.
fn pile(b: Bareme, le: Option<Jour>) -> Pile {
    let socle = |an: i32| -> SocleDuPas { lire(&code_livre(), &format!("baremes/{}.json", an)) };
    let section = match b {
        Bareme::Vide => None,
        Bareme::SansGrille => Some(PasParDefaut::default()),
        Bareme::Brut2025 | Bareme::Brut2026 => {
            let s = socle(if b == Bareme::Brut2025 { 2025 } else { 2026 }).pas_taux_par_defaut;
            let mut vues: Vec<&AbattementDate> = s.abattement_contrats_courts.iter().filter(|e| le.is_none_or(|l| e.depuis <= l)).collect();
            vues.sort_by_key(|e| e.depuis);
            Some(PasParDefaut { abattement_contrats_courts: vues.last().map(|e| NombreDate(e.valeur.0)), grilles: s.grilles, ..PasParDefaut::default() })
        }
    };
    Pile { pas_taux_par_defaut: section, ..Pile::default() }
}

/// Ce que rend le PAS pour un montant : sa valeur, ou son refus (la phrase).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Rendu<T> {
    Ok(T),
    SansGrille(String),
    Erreur(String),
}

impl<T> From<Result<T, ErreurPas>> for Rendu<T> {
    fn from(r: Result<T, ErreurPas>) -> Rendu<T> {
        match r {
            Ok(v) => Rendu::Ok(v),
            Err(ErreurPas::SansGrille(m)) => Rendu::SansGrille(socle::echec::phrase_de(&m).to_string()),
            Err(autre) => Rendu::Erreur(socle::echec::phrase_de(autre.message()).to_string()),
        }
    }
}

/// Les resultats d'un cas, un par montant : un seul quand ils sont tous le meme.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Suite<R> {
    Tous(R),
    UnParMontant(Vec<R>),
}

fn suite<R: PartialEq>(r: Vec<R>) -> Suite<R> {
    if r.iter().all(|x| *x == r[0]) {
        Suite::Tous(r.into_iter().next().unwrap())
    } else {
        Suite::UnParMontant(r)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CasDuTaux {
    bareme: Bareme,
    le: Option<Jour>,
    zone: String,
}

/// Le taux recu d'un versement, tel que le compte rendu le donne.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TauxRecu {
    taux: Option<Montant>,
    type_taux: Option<String>,
    identifiant: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CasDuVersement {
    bareme: Bareme,
    le: Option<Jour>,
    zone: String,
    contrat_court: bool,
    recu: Option<TauxRecu>,
}

fn taux_du_cas(c: &CasDuTaux) -> Suite<Rendu<(String, String)>> {
    suite(montants().into_iter().map(|m| taux_par_defaut(&pile(c.bareme, c.le), m, c.le, &c.zone).into()).collect())
}

fn versement_du_cas(c: &CasDuVersement) -> Suite<Rendu<Versement>> {
    let t = c.recu.as_ref().map(|r| Triplet { taux: r.taux.clone(), type_taux: r.type_taux.clone(), identifiant: r.identifiant.clone() });
    suite(montants().into_iter().map(|m| du_versement(t.as_ref(), &pile(c.bareme, c.le), m, c.le, c.contrat_court, &c.zone).into()).collect())
}

#[test]
fn le_taux_par_defaut_de_zero_a_dix_mille_euros() {
    let cas: Vec<CasDuTaux> = cas_de("pas/taux_par_defaut.json");
    assert!(cas.len() * MONTANTS.len() > 8000);
    jouer("pas/taux_par_defaut.json", cas, taux_du_cas);
}

#[test]
fn le_prelevement_d_un_versement() {
    let cas: Vec<CasDuVersement> = cas_de("pas/du_versement.json");
    assert!(cas.len() > 40);
    jouer("pas/du_versement.json", cas, versement_du_cas);
}

