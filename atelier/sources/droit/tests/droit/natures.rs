//! `contrat_court` des natures de contrat (`contrats-base.json`), contre `natures-court.json`.
//! `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit la reference.

use crate::outils::typee::{reference, verifier, Ligne};
use droit::natures::{contrat_court, nature, Jour};
use serde::{Deserialize, Serialize};

/// Un contrat et le jour ou on le lit : est-il court ?
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CasCourt {
    nature: String,
    debut: Jour,
    fin: Option<Jour>,
    le: Jour,
}

#[test]
fn contrat_court_des_natures() {
    let lignes: Vec<Ligne<CasCourt, bool>> = reference("natures-court.json");
    assert!(lignes.len() >= 39);
    let obtenu = lignes
        .into_iter()
        .map(|l| {
            let n = nature(&l.cas.nature).unwrap();
            let court = contrat_court(&n, l.cas.debut, l.cas.fin, l.cas.le);
            Ligne { cas: l.cas, attendu: court }
        })
        .collect();
    verifier("natures-court.json", obtenu);
}
