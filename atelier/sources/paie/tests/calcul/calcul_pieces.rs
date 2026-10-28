//! Les pieces de `calculer` contre `calcul-pieces.json` : les designations, la reduction generale et le seuil de SMIC sur
//! une grille (par empreinte, en entier pour quelques groupes). Les refus sont des attendus.
//! `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit l'oracle.

use crate::outils::{dec, lire_oracle, oracle, sceller};
use engagement::calcul::{au_dela_du_seuil, reduction_generale, LigneBrut};
use engagement::canon_valeur;
use socle::num::Decimal;
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
fn les_designations() {
    let mut cas = lire_oracle(ORACLE)["designations"].clone();
    for c in cas.as_array_mut().unwrap() {
        let (unite, libelle, q) = (c[0].as_str().unwrap(), c[1].as_str().unwrap(), c[2].as_str().unwrap());
        let l = LigneBrut {
            code: "x".into(), libelle: libelle.into(), unite: unite.into(), quantite: dec(q),
            taux: Decimal::ONE, montant: Decimal::ONE, heures_annexes: Decimal::ZERO, soumis: true,
            imposable: true, conges_spectacles: true, salaire_de_base: false, retabli: false,
            conges_pris: false, dsn_bloc52: None, arret_de_travail: false, dsn_autres_bruts: None,
        };
        c[3] = json!(l.designation());
    }
    oracle(ORACLE, "/designations", &cas);
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

