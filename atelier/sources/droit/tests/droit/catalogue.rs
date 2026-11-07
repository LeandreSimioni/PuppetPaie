//! Le catalogue des rubriques de chaque racine de `tests/donnees/racines/` (ce que la compagnie ajuste et ajoute ; ses
//! rubriques datees, lues a la date ; ses refus) et les heures d'une rubrique (`catalogue-heures.json`).
//! `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit la reference des heures.

use crate::outils::typee::{cas_de, chemin_donnee, verifier, Ligne};
use droit::catalogue::{catalogue, rubrique};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use socle::date::Jour;

fn jour(s: &str) -> Jour {
    Jour::lire(s).unwrap()
}

fn dec(x: f64) -> Decimal {
    x.to_string().parse().unwrap()
}

#[test]
fn les_catalogues_et_les_refus() {
    // Une rubrique de la compagnie changee a une date : la nouvelle valeur a partir d'elle, l'ancienne avant.
    let a = chemin_donnee("racines").join("a");
    let libelle = |j: &str| rubrique(&a, "cachet", &jour(j)).unwrap().libelle;
    assert_eq!((libelle("2026-12-31"), libelle("2027-01-01")), ("Cachet maison".to_string(), "Cachet maison 2027".to_string()));
    // Un catalogue de compagnie mal decrit se refuse, sans rien remplir a la place.
    let mut refus = 0;
    for entree in std::fs::read_dir(chemin_donnee("racines")).unwrap() {
        let entree = entree.unwrap();
        if entree.file_name().to_string_lossy().starts_with("refus_") {
            assert!(catalogue(&entree.path(), &jour("2026-10-28")).is_err(), "{:?}", entree.file_name());
            refus += 1;
        }
    }
    assert_eq!(refus, 9);
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
