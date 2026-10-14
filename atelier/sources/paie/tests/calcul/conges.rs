//! `conges` contre `conges.json` : les jours ouvrables selon les feries chomes, les lignes d'un mois de permanent avec
//! leurs avertissements, le compteur de conges. `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit l'oracle.

use crate::outils::{fictive, lire_oracle, oracle};
use paie::conges::{compteur_conges, jours_ouvrables, lignes_du_mois};
use paie::contexte::Contexte;
use paie::date::Jour;
use serde_json::{json, Value};

fn lire() -> Value {
    lire_oracle("conges.json")
}

fn jour(s: &str) -> Jour {
    Jour::lire(s).unwrap()
}

/// La compagnie de la fictive, a la date du relevé, avec les feries chomes du cas.
fn contexte(feries: &Value) -> Contexte {
    let mut ctx = Contexte::nouveau(&fictive("fictive-1285-grande"), Some(jour("2026-09-27"))).unwrap();
    let ent = ctx.entreprise.as_object_mut().unwrap();
    if feries == "<absent>" {
        ent.remove("jours_feries_chomes");
    } else {
        ent.insert("jours_feries_chomes".into(), feries.clone());
    }
    ctx
}

fn avis(a: &[paie::engagement::Avis]) -> Value {
    Value::Array(a.iter().map(|x| json!([x.info, x.texte])).collect())
}

fn err(m: String) -> Value {
    json!({"err": m})
}

#[test]
fn jours_ouvrables_selon_les_feries_chomes() {
    let mut cas = lire()["jours_ouvrables"].clone();
    for c in cas.as_array_mut().unwrap() {
        let ent = if c[0] == "<absent>" { None } else { Some(json!({"jours_feries_chomes": c[0]})) };
        c[3] = jours_ouvrables(&jour(c[1].as_str().unwrap()), &jour(c[2].as_str().unwrap()), ent.as_ref()).map_or_else(err, |n| json!(n));
    }
    assert!(!cas.as_array().unwrap().is_empty());
    oracle("conges.json", "/jours_ouvrables", &cas);
}

#[test]
fn les_lignes_d_un_mois_de_permanent() {
    let mut cas = lire()["lignes"].clone();
    for c in cas.as_array_mut().unwrap() {
        let ctx = contexte(&c["feries"]);
        c["attendu"] = lignes_du_mois(&ctx, &c["e"], c["mois"].as_str().unwrap())
            .map_or_else(err, |(lignes, dits)| json!({"lignes": lignes, "avertissements": avis(&dits)}));
    }
    oracle("conges.json", "/lignes", &cas);
}

#[test]
fn le_compteur_de_conges() {
    let mut cas = lire()["compteurs"].clone();
    let ctx = contexte(&json!("tous"));
    for c in cas.as_array_mut().unwrap() {
        c["attendu"] = compteur_conges(&ctx, c["cle"].as_str().unwrap(), &jour(c["date"].as_str().unwrap()), c["jusqu"].as_str()).map_or_else(err, |k| {
            json!({
                "resume": k.resume(), "solde": k.solde().to_string(),
                "report_restant": k.report_restant().to_string(),
                "acquis": k.acquis.to_string(), "pris": k.pris.to_string(),
                "reports": k.reports.to_string(),
                "remuneration": k.remuneration_periode.to_string(),
                "avertissements": avis(&k.avertissements),
            })
        });
    }
    oracle("conges.json", "/compteurs", &cas);
}
