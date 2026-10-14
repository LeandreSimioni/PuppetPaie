//! Les controles avant depot (regle 7) contre `usages-formats-ntt.json` : le tableau des usages (P26V01), le moteur
//! d'expressions des formats (motif par motif), la mauvaise valeur et les caracteres refuses d'une rubrique, les DSN des
//! demos et leurs variantes mutees passees aux trois controles, le NIR et le NTT. `PUPPETPAIE_REECRIRE_ORACLES=1`
//! reecrit l'oracle.

use crate::outils::{chemin_oracle, lire_oracle, oracle};
use paie::date::Jour;
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

/// Les chemins de l'oracle (`demo/...`) se lisent dans la copie figee des demonstrations.
fn latin1(chemin: &str) -> String {
    std::fs::read(chemin_oracle(chemin)).unwrap().iter().map(|&b| b as char).collect()
}

fn texte_mute(base: &str, ops: &Value) -> String {
    let mut l: Vec<String> = base.split("\r\n").map(String::from).collect();
    for op in ops.as_array().unwrap() {
        let op = op.as_array().unwrap();
        let i = op[1].as_u64().unwrap() as usize;
        match op[0].as_str().unwrap() {
            "set" => l[i] = op[2].as_str().unwrap().to_string(),
            "del" => {
                l.remove(i);
            }
            "ins" => l.insert(i, op[2].as_str().unwrap().to_string()),
            autre => panic!("{}", autre),
        }
    }
    l.join("\r\n")
}

/// Les trois controles d'un fichier : `null`, ou le refus.
fn controler(texte: &str, e: &mut Value) {
    let en_json = |r: Result<(), String>| r.err().map_or(Value::Null, Value::String);
    e["usages"] = en_json(usages::verifier_fichier(texte));
    e["formats"] = en_json(formats::verifier_fichier(texte));
    e["ntt"] = en_json(ntt::verifier_fichier(texte));
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
        let t = paie::fichiers::lire_json(&paie::fichiers::dossier_du_code().join(norme.table_formats().unwrap())).unwrap();
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

#[test]
fn fichiers_des_demos() {
    let mut fichiers = lire_oracle(ORACLE)["fichiers"].clone();
    for e in fichiers.as_array_mut().unwrap() {
        let texte = latin1(e["chemin"].as_str().unwrap());
        controler(&texte, e);
    }
    assert!(fichiers.as_array().unwrap().len() >= 80);
    oracle(ORACLE, "/fichiers", &fichiers);
}

#[test]
fn fichiers_mutes() {
    let mut muts = lire_oracle(ORACLE)["mutations"].clone();
    let mut hors_version = 0;
    for e in muts.as_array_mut().unwrap() {
        let texte = texte_mute(&latin1(e["base"].as_str().unwrap()), &e["ops"]);
        controler(&texte, e);
        // La version de la norme se lit dans le fichier (S10.G00.00.006) et choisit les tables : une variante qui y met
        // autre chose qu'une version livree (« a-b », « 99999999 »...) ne se controle pas.
        let version = texte.lines().find_map(|l| l.strip_prefix("S10.G00.00.006,'").map(|v| v.trim_end_matches('\'').to_string()));
        if version.as_deref().is_some_and(|v| paie::norme::Norme::connue(v).is_err()) {
            for c in ["usages", "formats"] {
                assert!(e[c].as_str().is_some_and(|m| m.starts_with("S10.G00.00.006")), "une version inconnue refuse : {} {}", c, e[c]);
            }
            hors_version += 1;
        }
    }
    assert!(muts.as_array().unwrap().len() > 200);
    assert_eq!(hors_version, 7, "les variantes qui changent la version de la norme");
    oracle(ORACLE, "/mutations", &muts);
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
