//! L'OETH (`oeth::declaration`) et ce que la DSN lit de la CASC-SVP (`casc::dsn_deposee`, `casc::mois_regularises`)
//! contre `oeth_casc.json` : les quatre demonstrations mois par mois, et des variantes montees depuis la config d'une
//! demo. Les refus sont des attendus. `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit l'oracle.

use crate::outils::{code_livre, copier, demo, dossier_de_test, lire, lire_oracle, oracle};
use paie::casc::{base, dsn_deposee, mois_regularises};
use paie::contexte::Contexte;
use paie::date::Jour;
use paie::num::Decimal;
use paie::oeth::{declaration, Declaration};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const ORACLE: &str = "oeth_casc.json";

fn contexte(racine: &Path) -> Contexte {
    Contexte::nouveau(racine, Some(Jour::lire("2026-09-27").unwrap())).unwrap()
}

/// La config de la demo, puis les fichiers de la variante.
fn variante(spec: &Value, test: &str, n: usize) -> PathBuf {
    let racine = dossier_de_test("oeth_casc", &format!("{}-{}", test, n));
    copier(&demo(spec["racine"].as_str().unwrap()).join("config"), &racine.join("config"));
    for (chemin, v) in spec["fichiers"].as_object().unwrap() {
        let p = racine.join(chemin);
        if v.get("supprimer").is_some() {
            let _ = std::fs::remove_file(&p);
            continue;
        }
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        let texte = match v.get("json") {
            Some(j) => serde_json::to_string(j).unwrap(),
            None => v["texte"].as_str().unwrap().to_string(),
        };
        std::fs::write(p, texte).unwrap();
    }
    racine
}

fn en_json(d: &Declaration) -> Value {
    let paires = |l: &[(String, Decimal)]| json!(l.iter().map(|(c, m)| json!([c, m.to_string()])).collect::<Vec<_>>());
    let a_verser: Decimal = d.bordereau().iter().map(|(_, m)| *m).fold(Decimal::new(0, 2), |x, y| x + y);
    json!({
        "millesime": d.millesime, "effectif": d.effectif.to_string(), "obligation": d.obligation.to_string(),
        "beneficiaires": d.beneficiaires.to_string(), "manquants": d.manquants.to_string(), "coefficient": d.coefficient,
        "smic": d.smic.to_string(), "brute": d.brute.to_string(), "nette": d.nette.to_string(),
        "deductions": d.deductions.iter().map(|(c, m)| (c.clone(), json!(m.to_string()))).collect::<serde_json::Map<_, _>>(),
        "externes": paires(&d.externes), "accord": d.accord.clone().unwrap_or_default(),
        "non_realisees": d.non_realisees.to_string(), "source": d.source, "blocs_82": paires(&d.blocs_82()),
        "bordereau": paires(&d.bordereau()), "a_verser": a_verser.to_string(),
    })
}

fn oeth(ctx: &Contexte, mois: &str) -> Value {
    match declaration(ctx, mois) {
        Ok(None) => Value::Null,
        Ok(Some(d)) => en_json(&d),
        Err(e) => json!({ "erreur": e }),
    }
}

fn dsn(ctx: &Contexte, mois: &str, nic: Option<&str>) -> Value {
    match dsn_deposee(ctx, mois, nic) {
        Ok(None) => Value::Null,
        Ok(Some(p)) => json!({"chemin": p.strip_prefix(&ctx.racine).unwrap().to_string_lossy().replace('\\', "/")}),
        Err(e) => json!({ "erreur": e }),
    }
}

#[test]
fn les_demos_mois_par_mois() {
    let mut demos = lire_oracle(ORACLE)["demos"].clone();
    let mut n = 0;
    for (nom, x) in demos.as_object_mut().unwrap() {
        let ctx = contexte(&demo(nom));
        x["regularises"] = json!(mois_regularises(&ctx).unwrap());
        for (mois, r) in x["mois"].as_object_mut().unwrap() {
            n += 1;
            r["oeth"] = oeth(&ctx, mois);
            r["dsn"] = dsn(&ctx, mois, None);
            r["dsn_nic"] = dsn(&ctx, mois, Some("00099"));
        }
    }
    assert!(n >= 100, "{} mois", n);
    oracle(ORACLE, "/demos", &demos);
}

#[test]
fn l_oeth_des_variantes() {
    let mut cas = lire_oracle(ORACLE)["oeth"].clone();
    for (i, c) in cas.as_array_mut().unwrap().iter_mut().enumerate() {
        c["attendu"] = oeth(&contexte(&variante(c, "oeth", i)), c["mois"].as_str().unwrap());
    }
    assert!(cas.as_array().unwrap().len() >= 40);
    oracle(ORACLE, "/oeth", &cas);
}

#[test]
fn la_dsn_deposee_des_variantes() {
    let mut cas = lire_oracle(ORACLE)["dsn"].clone();
    for (i, c) in cas.as_array_mut().unwrap().iter_mut().enumerate() {
        c["attendu"] = dsn(&contexte(&variante(c, "dsn", i)), c["mois"].as_str().unwrap(), c["nic"].as_str());
    }
    oracle(ORACLE, "/dsn", &cas);
}

#[test]
fn les_mois_regularises_des_variantes() {
    let mut cas = lire_oracle(ORACLE)["regularises"].clone();
    for (i, c) in cas.as_array_mut().unwrap().iter_mut().enumerate() {
        c["attendu"] = json!(mois_regularises(&contexte(&variante(c, "regularises", i))).unwrap());
    }
    oracle(ORACLE, "/regularises", &cas);
}

#[test]
fn la_base_de_la_casc_est_le_fichier_livre() {
    assert_eq!(base().unwrap(), lire(&code_livre(), "casc-svp-base.json"));
}
