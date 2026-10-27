//! Le prelevement a la source contre `pas.json` : la grille de l'article 204 H du CGI (`taux_par_defaut`, `du_versement`,
//! l'abattement des contrats courts), de 0 a 10 000 EUR, de part et d'autre des changements de bareme.
//! `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit l'oracle.

use crate::outils::{code_livre, lire_oracle, oracle};
use droit::bareme::canon;
use socle::date::Jour;
use socle::num::dec;
use droit::pas::{abattement, du_versement, taux_par_defaut, ErreurPas, Triplet, Versement};
use serde_json::{json, Value};
use std::collections::HashMap;

fn attendus() -> Value {
    lire_oracle("pas.json")
}

fn jour(s: &str) -> Option<Jour> {
    if s == "None" {
        None
    } else {
        Some(Jour::lire(s).unwrap())
    }
}

/// Une ligne de resultats, un par montant de la grille : `*x` quand ils sont tous x.
fn ligne(r: Vec<String>) -> Value {
    Value::String(if r.iter().all(|x| *x == r[0]) { format!("*{}", r[0]) } else { r.join(";") })
}

fn montants(a: &Value) -> Vec<socle::num::Decimal> {
    a["grille"]["montants"].as_array().unwrap().iter().map(|m| dec(m).unwrap()).collect()
}

fn erreur(e: ErreurPas) -> String {
    match e {
        ErreurPas::SansGrille(m) => format!("SANS_GRILLE:{}", m),
        autre => format!("ERREUR:{}", autre.message()),
    }
}

fn texte(v: &Value) -> String {
    match canon(v) {
        Value::Null => "None".to_string(),
        Value::String(s) => s,
        autre => autre.to_string(),
    }
}

fn ligne_versement(r: Result<Versement, ErreurPas>) -> String {
    match r {
        Err(e) => erreur(e),
        Ok(v) => {
            let c = v.canon();
            ["taux", "type", "identifiant", "abattement", "montant_soumis", "prelevement"]
                .iter()
                .map(|k| texte(&c[*k]))
                .collect::<Vec<_>>()
                .join("|")
        }
    }
}

fn baremes() -> HashMap<&'static str, Value> {
    let socle = |an: i32| crate::outils::lire(&code_livre(), &format!("baremes/{}.json", an));
    HashMap::from([
        ("brut-2025", socle(2025)),
        ("brut-2026", socle(2026)),
        ("vide", json!({})),
        ("sans-grille", json!({"pas_taux_par_defaut": {"grilles": []}})),
    ])
}

fn triplet(v: &Value) -> Option<Triplet> {
    v.as_array().map(|a| (a[0].clone(), a[1].clone(), a[2].clone()))
}

#[test]
fn l_abattement_suit_la_date() {
    let b = baremes();
    let mut cas = attendus()["grille"]["abattement"].clone();
    for (cle, v) in cas.as_object_mut().unwrap() {
        let p: Vec<&str> = cle.split('|').collect();
        *v = Value::String(abattement(&b[p[0]], jour(p[1])).map_or_else(erreur, socle::num::nombre));
    }
    assert!(cas.as_object().unwrap().len() > 50);
    oracle("pas.json", "/grille/abattement", &cas);
}

#[test]
fn le_taux_par_defaut_de_zero_a_dix_mille_euros() {
    let (a, b) = (attendus(), baremes());
    let mut cas = a["grille"]["taux_par_defaut"].clone();
    for (cle, v) in cas.as_object_mut().unwrap() {
        let p: Vec<&str> = cle.split('|').collect();
        *v = ligne(montants(&a).into_iter().map(|m| taux_par_defaut(&b[p[0]], m, jour(p[1]), p[2]).map_or_else(erreur, |(t, ty)| format!("{},{}", t, ty))).collect());
    }
    assert!(cas.as_object().unwrap().len() * montants(&a).len() > 8000);
    oracle("pas.json", "/grille/taux_par_defaut", &cas);
}

#[test]
fn le_prelevement_d_un_versement() {
    let (a, b) = (attendus(), baremes());
    let triplets = a["grille"]["triplets"].as_array().unwrap();
    let mut cas = a["grille"]["du_versement"].clone();
    for (cle, v) in cas.as_object_mut().unwrap() {
        let p: Vec<&str> = cle.split('|').collect();
        let t = triplet(&triplets[p[4].parse::<usize>().unwrap()]);
        *v = ligne(montants(&a).into_iter().map(|m| ligne_versement(du_versement(t.as_ref(), &b[p[0]], m, jour(p[1]), p[3] == "True", p[2]))).collect());
    }
    assert!(cas.as_object().unwrap().len() > 40);
    oracle("pas.json", "/grille/du_versement", &cas);
}

