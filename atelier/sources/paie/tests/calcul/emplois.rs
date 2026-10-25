//! Les emplois : `resoudre`, `metier`, `statut_non_cadre` (`emplois.json`), `emploi_a_la_date` et `emploi_de`
//! (`emploi_de.json`) sur les codes de la fictive, du catalogue et de la nomenclature, et un code inconnu ; contre les
//! oracles, que `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit.

use crate::outils::{lire_oracle, oracle};
use socle::date::Jour;
use droit::emplois::{emploi_a_la_date, emploi_de, metier, resoudre, statut_non_cadre, EmploiEngagement};
use serde_json::{json, Map, Value};

#[test]
fn resoudre_metier_et_statut_non_cadre() {
    let mut cas = lire_oracle("emplois.json");
    let (mut resolus, mut refus) = (0, 0);
    for e in cas.as_array_mut().unwrap() {
        let code = e["code"].as_str().unwrap().to_string();
        let m = metier(&code).unwrap();
        if e.get("non_cadre").is_some() {
            e["non_cadre"] = statut_non_cadre(m.as_ref().unwrap()).map_or(Value::Null, Value::String);
        }
        // Un code de la seule nomenclature : l'oracle ne garde que son statut de non-cadre.
        if !(e["metier"].is_null() && e.get("non_cadre").is_some()) {
            e["metier"] = m.unwrap_or(Value::Null);
        }
        for (cle, voulu) in e["resolu"].as_object_mut().unwrap() {
            let (cadre, fem) = cle.split_once('|').unwrap();
            let cadre = match cadre {
                "None" => None,
                "True" => Some(true),
                _ => Some(false),
            };
            *voulu = resoudre(&code, cadre, fem == "True").unwrap_or_else(|m| {
                refus += 1;
                json!({ "refus": m })
            });
            resolus += 1;
        }
    }
    oracle("emplois.json", "", &cas);
    assert!(resolus - refus > 700, "{}", resolus);
    assert_eq!(refus, 8); // XXX999 et le code vide : quatre variantes chacun
}

#[test]
fn les_codes_de_la_fictive() {
    for c in ["ADM020", "ATT010", "CHA080", "COM010", "DAN010", "MAR010", "MUS010", "REG010", "REG120", "REG130", "REG150"] {
        let r = resoudre(c, None, false).unwrap();
        assert_eq!(r["code_emploi_ft"], c);
        assert!(r["emploi"].is_string());
    }
    let r = resoudre("MAR010", None, false).unwrap();
    assert_eq!(r["emploi"], "MARIONNETTISTE");
    assert_eq!(r["pcs_ese"], "354c");
    assert_eq!(r["statut_conventionnel"], "05");
    assert_eq!(r["statut_categoriel_rc"], "04");
    let r = resoudre("MAR010", Some(true), false).unwrap();
    assert_eq!(r["statut_conventionnel"], "04");
    assert_eq!(r["statut_categoriel_rc"], "01");
}

#[test]
fn un_code_inconnu_est_refuse() {
    let e = resoudre("XXX999", None, false).unwrap_err();
    assert!(e.contains("XXX999") && e.contains("inconnu"), "{}", e);
    assert!(metier("XXX999").unwrap().is_none());
    assert!(metier("").unwrap().is_none());
}

fn champ(e: &Map<String, Value>, c: &str) -> Option<String> {
    e.get(c).and_then(|v| v.as_str()).map(String::from)
}

#[test]
fn emploi_de_et_emploi_a_la_date() {
    let mut cas = lire_oracle("emploi_de.json");
    assert!(cas.as_array().unwrap().len() > 100);
    let telle_qu_au = |f: &Value, j: &str| droit::contexte::telle_qu_au(f, &Jour::lire(j)?);
    for c in cas.as_array_mut().unwrap() {
        let (fiche, debut) = (&c["fiche"], c["debut"].as_str());
        let e = c["eng"].as_object().unwrap();
        let eng = EmploiEngagement {
            emploi: champ(e, "emploi"),
            emploi_affichage: champ(e, "emploi_affichage"),
            pcs_ese: champ(e, "pcs_ese"),
            code_emploi_ft: champ(e, "code_emploi_ft"),
            statut: champ(e, "statut"),
            statut_conventionnel: champ(e, "statut_conventionnel"),
            statut_categoriel_rc: champ(e, "statut_categoriel_rc"),
        };
        let a_la_date = emploi_a_la_date(fiche, debut, &telle_qu_au).unwrap();
        let de = emploi_de(fiche, debut, &eng, &telle_qu_au).unwrap();
        c["a_la_date"] = a_la_date;
        c["emploi_de"] = de;
    }
    oracle("emploi_de.json", "", &cas);
}
