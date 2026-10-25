//! Le catalogue des rubriques contre `catalogue.json` : le catalogue de chaque racine de `tests/donnees/racines/` (ce que
//! la compagnie ajuste, ajoute, ordonne), ses refus, les heures d'une rubrique, les annexes.
//! `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit l'oracle.

use crate::outils::{chemin_oracle, egal, lire_oracle, oracle_selon};
use droit::catalogue::{annexes, catalogue, rubrique, Rubrique};
use serde_json::{json, Value};

fn en_json(r: &Rubrique) -> Value {
    json!({
        "code": r.code, "libelle": r.libelle, "unite": r.unite, "nature": r.nature,
        "saisie": r.saisie, "soumis": r.soumis, "imposable": r.imposable,
        "conges_spectacles": r.conges_spectacles, "source": r.source, "sens": r.sens,
        "heures_forfait": r.heures_forfait, "plafond": r.plafond, "aem": r.aem,
        "dsn_type": r.dsn_type, "dsn_autres_bruts": r.dsn_autres_bruts,
        "annexe": r.annexe, "plafond_mensuel": r.plafond_mensuel,
        "plafond_annuel_heures": r.plafond_annuel_heures, "compte_pour": r.compte_pour,
        "salaire_de_base": r.salaire_de_base, "retabli": r.retabli,
        "suspension": r.suspension, "arret_de_travail": r.arret_de_travail,
        "conges_pris": r.conges_pris, "dsn_bloc52": r.dsn_bloc52,
        "origine": r.origine, "usages": r.usages, "active": r.active,
    })
}

#[test]
fn les_catalogues_et_les_refus() {
    let mut cas = lire_oracle("catalogue.json")["cas"].clone();
    assert!(cas.as_object().unwrap().len() >= 12);
    let mut refus = 0;
    for (nom, attendu) in cas.as_object_mut().unwrap() {
        let racine = chemin_oracle("racines").join(nom);
        *attendu = match catalogue(&racine) {
            Err(e) => {
                refus += 1;
                json!({ "refus": e })
            }
            Ok(cat) => json!({
                "catalogue": cat.iter().map(en_json).collect::<Vec<_>>(),
                "inconnue": rubrique(&racine, "zzz").unwrap_err(),
                "reservee_a": cat.iter().map(|r| (r.code.clone(), json!(r.reservee_a()))).collect::<serde_json::Map<_, _>>(),
            }),
        };
    }
    assert_eq!(refus, 9);
    oracle_selon("catalogue.json", "/cas", &cas, egal);
}

#[test]
fn les_heures() {
    let racine = chemin_oracle("racines/a");
    let mut heures = lire_oracle("catalogue.json")["heures"].clone();
    for h in heures.as_array_mut().unwrap() {
        let r = rubrique(&racine, h[0].as_str().unwrap()).unwrap();
        h[3] = json!(r.heures(h[1].as_f64().unwrap(), h[2].as_f64()));
    }
    assert!(heures.as_array().unwrap().len() > 600);
    oracle_selon("catalogue.json", "/heures", &heures, egal);
}

#[test]
fn les_annexes() {
    oracle_selon("catalogue.json", "/annexes", &annexes().unwrap(), egal);
}
