//! Les pieces de `calculer` contre `calcul-pieces.json` : le code du bloc 54, la reduction generale et le seuil de SMIC sur
//! une grille (par empreinte, en entier pour quelques groupes). Les refus sont des attendus.
//! `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit l'oracle.

use crate::outils::{dec, dossier_de_test, lire_oracle, oracle, sceller};
use droit::catalogue::rubrique;
use droit::catalogue::code_54;
use droit::reduction::{au_dela_du_seuil, reduction_generale};
use engagement::canon_valeur;
use serde_json::{json, Value};

const ORACLE: &str = "calcul-pieces.json";


/// `{"ok": canon(v)}` ou `{"refus": message}`.
fn essai(r: Result<Value, String>) -> Value {
    match r {
        Ok(v) => json!({"ok": canon_valeur(&v).unwrap()}),
        Err(e) => json!({"refus": e}),
    }
}



#[test]
fn le_code_du_bloc_54() {
    let mut cas = lire_oracle(ORACLE)["code_54"].clone();
    let racine = dossier_de_test("calcul_pieces", "code_54");
    let mut refus = 0;
    for c in cas.as_array_mut().unwrap() {
        let r = rubrique(&racine, c[0].as_str().unwrap(), &socle::date::Jour::lire("2026-10-28").unwrap()).unwrap();
        let vu = code_54(&r, &c[1]);
        refus += vu.is_err() as usize;
        c[2] = essai(vu.map(|o| o.map_or(Value::Null, Value::String)));
    }
    assert!(refus > 10, "{}", refus);
    oracle(ORACLE, "/code_54", &cas);
}

// ----------------------------------------------------------------- la grille
/// Un groupe de la grille : l'empreinte de ses resultats, et ses resultats entiers quand l'oracle les garde.
fn groupe(g: &mut Value, vus: Vec<Value>) {
    let vus = Value::Array(vus);
    g["empreinte"] = Value::String(sceller(&vus));
    if g.get("attendu").is_some() {
        g["attendu"] = vus;
    }
}

#[test]
fn la_reduction_generale() {
    let rel = lire_oracle(ORACLE);
    let baremes = rel["baremes"].as_array().unwrap();
    let mut rg = rel["rg"].clone();
    let (mut points, mut appliquees) = (0, 0);
    for g in rg.as_array_mut().unwrap() {
        let cle = g["grille"].as_str().unwrap().to_string();
        let (rang, h) = cle.split_once('|').unwrap();
        let bar = &baremes[rang.parse::<usize>().unwrap()];
        let (statut, caisse) = (g["statut"].as_str().map(String::from), g["caisse"].as_bool().unwrap());
        let mut vus = Vec::new();
        for b in rel["grilles"][&cle].as_array().unwrap() {
            points += 1;
            let r = reduction_generale(bar, statut.as_deref(), dec(b.as_str().unwrap()), dec(h), caisse);
            appliquees += matches!(r, Ok(Some(_))) as usize;
            vus.push(essai(r.map(|o| o.map_or(Value::Null, |x| x.canon()))));
        }
        groupe(g, vus);
    }
    assert!(points > 30000 && appliquees > 5000, "{} {}", points, appliquees);
    oracle(ORACLE, "/rg", &rg);
}

#[test]
fn le_seuil_de_smic() {
    let rel = lire_oracle(ORACLE);
    let baremes = rel["baremes"].as_array().unwrap();
    let mut seuil = rel["seuil"].clone();
    let mut points = 0;
    for g in seuil.as_array_mut().unwrap() {
        let cle = g["grille"].as_str().unwrap().to_string();
        let (rang, h) = cle.split_once('|').unwrap();
        let bar = &baremes[rang.parse::<usize>().unwrap()];
        let mut vus = Vec::new();
        for b in rel["grilles"][&cle].as_array().unwrap() {
            points += 1;
            vus.push(essai(au_dela_du_seuil(bar, dec(b.as_str().unwrap()), dec(h), Some(&g["seuil"])).map(Value::Bool)));
        }
        groupe(g, vus);
    }
    assert!(points > 30000, "{}", points);
    oracle(ORACLE, "/seuil", &seuil);
}

