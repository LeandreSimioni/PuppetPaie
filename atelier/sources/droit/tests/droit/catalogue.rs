//! Le catalogue des rubriques contre `catalogue.json` : le catalogue de chaque racine de `tests/donnees/racines/` (ce que
//! la compagnie ajuste et ajoute ; ses rubriques datees, lues a la date), ses refus, et les heures d'une rubrique
//! (`catalogue-heures.json`). `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit les references.

use crate::outils::typee::{cas_de, chemin_donnee, verifier, Ligne};
use droit::catalogue::{annexes, catalogue, rubrique, PlafondAnnuelHeures, Rubrique};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use socle::date::Jour;
use std::collections::BTreeMap;

fn jour(s: &str) -> Jour {
    Jour::lire(s).unwrap()
}

fn dec(x: f64) -> Decimal {
    x.to_string().parse().unwrap()
}

/// Une rubrique telle que le calcul la lit.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RubriqueVue {
    code: String,
    libelle: String,
    unite: String,
    nature: String,
    saisie: Vec<String>,
    soumis: bool,
    imposable: bool,
    conges_spectacles: bool,
    source: String,
    sens: String,
    heures_forfait: Option<Decimal>,
    plafond: Option<String>,
    aem: Option<String>,
    dsn_type: Option<String>,
    dsn_autres_bruts: Option<String>,
    annexe: Option<String>,
    plafond_mensuel: Option<i64>,
    plafond_annuel_heures: Option<PlafondAnnuelHeures>,
    compte_pour: String,
    salaire_de_base: bool,
    retabli: bool,
    suspension: bool,
    arret_de_travail: bool,
    conges_pris: bool,
    dsn_bloc52: Option<String>,
    origine: String,
}

fn vue(r: &Rubrique) -> RubriqueVue {
    RubriqueVue {
        code: r.code.clone(),
        libelle: r.libelle.clone(),
        unite: r.unite.clone(),
        nature: r.nature.clone(),
        saisie: r.saisie.clone(),
        soumis: r.soumis,
        imposable: r.imposable,
        conges_spectacles: r.conges_spectacles,
        source: r.source.clone(),
        sens: r.sens.clone(),
        heures_forfait: r.heures_forfait.map(dec),
        plafond: r.plafond.clone(),
        aem: r.aem.clone(),
        dsn_type: r.dsn_type.clone(),
        dsn_autres_bruts: r.dsn_autres_bruts.clone(),
        annexe: r.annexe.clone(),
        plafond_mensuel: r.plafond_mensuel,
        plafond_annuel_heures: r.plafond_annuel_heures,
        compte_pour: r.compte_pour.clone(),
        salaire_de_base: r.salaire_de_base,
        retabli: r.retabli,
        suspension: r.suspension,
        arret_de_travail: r.arret_de_travail,
        conges_pris: r.conges_pris,
        dsn_bloc52: r.dsn_bloc52.clone(),
        origine: r.origine.clone(),
    }
}

/// Le catalogue d'une racine, ou le refus qu'elle rend.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
enum Catalogue {
    #[serde(rename = "refus")]
    Refus(String),
    #[serde(rename = "catalogue")]
    Rubriques {
        catalogue: Vec<RubriqueVue>,
        inconnue: String,
        reservee_a: BTreeMap<String, Option<String>>,
    },
}

fn de_la_racine(nom: &str) -> Catalogue {
    let racine = chemin_donnee("racines").join(nom);
    match catalogue(&racine, &jour("2026-10-28")) {
        Err(e) => Catalogue::Refus(socle::echec::phrase_de(&e)),
        Ok(cat) => Catalogue::Rubriques {
            catalogue: cat.iter().map(vue).collect(),
            inconnue: rubrique(&racine, "zzz", &jour("2026-10-28")).unwrap_err(),
            reservee_a: cat.iter().map(|r| (r.code.clone(), r.reservee_a().map(String::from))).collect(),
        },
    }
}

#[test]
fn les_catalogues_et_les_refus() {
    let racines: Vec<String> = cas_de("catalogue.json");
    assert!(racines.len() >= 12);
    let obtenu: Vec<Ligne<String, Catalogue>> = racines.into_iter().map(|n| Ligne { attendu: de_la_racine(&n), cas: n }).collect();
    assert_eq!(obtenu.iter().filter(|l| matches!(l.attendu, Catalogue::Refus(_))).count(), 9);
    // Une rubrique de la compagnie changee a une date : la nouvelle valeur a partir d'elle, l'ancienne avant.
    let a = chemin_donnee("racines").join("a");
    let libelle = |j: &str| rubrique(&a, "cachet", &jour(j)).unwrap().libelle;
    assert_eq!((libelle("2026-12-31"), libelle("2027-01-01")), ("Cachet maison".to_string(), "Cachet maison 2027".to_string()));
    verifier("catalogue.json", obtenu);
}

#[test]
fn les_annexes() {
    let a = annexes().unwrap();
    assert_eq!((a.seuil_heures, a.fenetre_jours), (507, 365));
}

/// Une rubrique, la quantite d'une ligne et les heures qu'elle saisit.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CasHeures {
    rubrique: String,
    quantite: Decimal,
    heures_saisies: Option<Decimal>,
}

fn flottant(d: Decimal) -> f64 {
    d.to_string().parse().unwrap()
}

#[test]
fn les_heures() {
    let racine = chemin_donnee("racines/a");
    let cas: Vec<CasHeures> = cas_de("catalogue-heures.json");
    assert!(cas.len() > 600);
    let obtenu = cas
        .into_iter()
        .map(|c| {
            let r = rubrique(&racine, &c.rubrique, &jour("2026-10-28")).unwrap();
            let heures = dec(r.heures(flottant(c.quantite), c.heures_saisies.map(flottant)));
            Ligne { cas: c, attendu: heures }
        })
        .collect();
    verifier("catalogue-heures.json", obtenu);
}
