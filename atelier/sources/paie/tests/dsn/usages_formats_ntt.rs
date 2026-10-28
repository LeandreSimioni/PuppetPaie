//! Les controles avant depot (regle 7) contre `usages-formats-ntt.json` : le tableau des usages (P26V01), le moteur
//! d'expressions des formats (motif par motif), la mauvaise valeur et les caracteres refuses d'une rubrique, le NIR et
//! le NTT. `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit l'oracle.

use crate::outils::{lire_oracle, oracle};
use socle::date::Jour;
use paie::dsn::Fichier;
use paie::formats::{self, Regex};
use paie::{ntt, usages};
use serde_json::{json, Value};

const ORACLE: &str = "usages-formats-ntt.json";
const NATURES: [&str; 7] = ["01", "04", "05", "07", "08", "09", "10"];

/// L'oracle est celui des tables 2026 : ses comparaisons se font dans la version P26V01 (la table se choisit par version
/// de la norme).
fn p26() -> paie::norme::Norme {
    paie::norme::Norme::connue("P26V01").unwrap()
}




#[test]
fn tableau_des_usages() {
    let mut tableau = lire_oracle(ORACLE)["usages"].clone();
    for (r, codes) in tableau.as_object_mut().unwrap() {
        *codes = Value::String(NATURES.iter().map(|nat| usages::usage_en(&p26(), r, nat).unwrap()).map(|u| if u.is_empty() { ".".to_string() } else { u }).collect());
    }
    assert_eq!(usages::usage_en(&p26(), "S10.G00.00.001", "99").unwrap(), "");
    assert!(tableau.as_object().unwrap().len() * NATURES.len() > 4000);
    oracle(ORACLE, "/usages", &tableau);
}

#[test]
fn expressions_des_formats() {
    let mut cas = lire_oracle(ORACLE)["regex"].clone();
    let mut n = 0;
    for c in cas.as_array_mut().unwrap() {
        let re = Regex::compiler(c[0].as_str().unwrap()).unwrap();
        for x in c[1].as_array_mut().unwrap() {
            x[1] = Value::Bool(re.fullmatch(x[0].as_str().unwrap()));
            n += 1;
        }
    }
    assert!(n > 4000, "{}", n);
    oracle(ORACLE, "/regex", &cas);
}

#[test]
fn toutes_les_expressions_de_la_table_se_compilent() {
    // Les tables de toutes les versions livrees (`norme-dsn-base.json`) : celle de 2027 porte des expressions que celle
    // de 2026 n'avait pas (S89.G00.88.008, entiere en 2027).
    for norme in paie::norme::versions().unwrap() {
        let t = socle::fichiers::lire_json(&socle::fichiers::dossier_du_code().join(norme.table_formats().unwrap())).unwrap();
        let mut n = 0;
        for (_, f) in t["formats"].as_object().unwrap() {
            for c in f.get("csl").and_then(|c| c.as_array()).into_iter().flatten() {
                Regex::compiler(c.as_str().unwrap()).unwrap();
                n += 1;
            }
        }
        assert!(n > 100, "{} : {}", norme.version, n);
    }
}

#[test]
fn mauvaise_valeur_et_caracteres_refuses() {
    let mut cas = lire_oracle(ORACLE)["rubriques"].clone();
    for c in cas.as_array_mut().unwrap() {
        let (r, v) = (c[0].as_str().unwrap().to_string(), c[1].as_str().unwrap().to_string());
        c[2] = json!(formats::mauvaise_en(&p26(), &r, &v).unwrap());
        c[3] = json!(formats::caracteres_refuses_en(&p26(), &r, &v).unwrap().iter().map(|c| c.to_string()).collect::<Vec<_>>());
    }
    assert!(cas.as_array().unwrap().len() > 5000);
    oracle(ORACLE, "/rubriques", &cas);
}



fn lignes_ou_erreur(r: Result<(), String>, f: &Fichier) -> Value {
    match r {
        Ok(()) => json!({ "lignes": f.lignes }),
        Err(e) => json!({ "erreur": e }),
    }
}

#[test]
fn nir_et_ntt() {
    let mut cas = lire_oracle(ORACLE)["ntt"].clone();
    for e in cas.as_array_mut().unwrap() {
        let (ent, sal) = (e["ent"].clone(), e["sal"].clone());
        let date = Jour::lire(e["date"].as_str().unwrap()).unwrap();
        let mut f = Fichier::nouveau();
        e["nir"] = lignes_ou_erreur(ntt::poser_nir(&mut f, &sal), &f);
        let mut f = Fichier::nouveau();
        e["ntt"] = lignes_ou_erreur(ntt::poser_ntt(&mut f, &ent, &sal, &date), &f);
        e["sexe"] = json!(ntt::sexe(&sal));
        e["nir_de"] = json!(ntt::nir_de(&sal));
        e["besoin"] = ntt::besoin(&sal, &date).map_or_else(|m| Value::String(format!("ERR:{}", m)), Value::Bool);
    }
    assert!(cas.as_array().unwrap().len() > 500);
    oracle(ORACLE, "/ntt", &cas);
}
