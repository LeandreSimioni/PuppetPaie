//! Les oracles des tests, ecrits une fois pour les trois crates : un oracle est un fichier de `tests/donnees/` de la
//! crate qui l'inclut (`CARGO_MANIFEST_DIR`), que `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit ; et l'echantillon a graine
//! fixe d'une grille (`tire`). Inclus par les aides de chaque crate (`#[path = ...] mod oracles;`).

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::str::FromStr;

use rust_decimal::Decimal;
use serde_json::Value;


/// `tests/donnees/<nom>`.
pub fn chemin_oracle(nom: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/donnees").join(nom)
}

pub fn lire_oracle(nom: &str) -> Value {
    serde_json::from_slice(&std::fs::read(chemin_oracle(nom)).unwrap()).unwrap_or_else(|e| panic!("{} illisible : {}", nom, e))
}

/// `PUPPETPAIE_REECRIRE_ORACLES=1` : les tests réécrivent leurs oracles au lieu de les comparer. Lue, jamais posée par un test.
pub fn reecrire_oracles() -> bool {
    std::env::var("PUPPETPAIE_REECRIRE_ORACLES").is_ok_and(|v| v == "1")
}

/// Deux nombres égaux en valeur (`2.0` et `2`, `"2.50"` et `"2.5"` à 1e-25 près) ; tout le reste au caractère près.
pub fn egal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        (Value::String(x), Value::String(y)) => {
            x == y
                || match (Decimal::from_str(x), Decimal::from_str(y)) {
                    (Ok(x), Ok(y)) => (x - y).abs() < Decimal::from_str("0.0000000000000000000000001").unwrap(),
                    _ => false,
                }
        }
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(a, b)| egal(a, b)),
        (Value::Object(x), Value::Object(y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| egal(v, w))),
        _ => a == b,
    }
}

fn ecarts(attendu: &Value, obtenu: &Value, chemin: &str, egal: &dyn Fn(&Value, &Value) -> bool, out: &mut Vec<String>) {
    if egal(attendu, obtenu) {
        return;
    }
    match (attendu, obtenu) {
        (Value::Array(a), Value::Array(o)) if a.len() == o.len() => {
            a.iter().zip(o).enumerate().for_each(|(i, (x, y))| ecarts(x, y, &format!("{}/{}", chemin, i), egal, out))
        }
        (Value::Object(a), Value::Object(o)) if a.len() == o.len() && a.keys().all(|k| o.contains_key(k)) => {
            a.iter().for_each(|(k, x)| ecarts(x, &o[k], &format!("{}/{}", chemin, k), egal, out))
        }
        _ => {
            let court = |v: &Value| v.to_string().chars().take(300).collect::<String>();
            out.push(format!("{}\n  attendu : {}\n  obtenu  : {}", chemin, court(attendu), court(obtenu)));
        }
    }
}

/// Ce qui est égal garde la forme de l'oracle ; le reste prend celle du moteur.
fn fondre(ancien: &Value, neuf: &Value, egal: &dyn Fn(&Value, &Value) -> bool) -> Value {
    if egal(ancien, neuf) {
        return ancien.clone();
    }
    match (ancien, neuf) {
        (Value::Array(a), Value::Array(n)) if a.len() == n.len() => Value::Array(a.iter().zip(n).map(|(x, y)| fondre(x, y, egal)).collect()),
        (Value::Object(a), Value::Object(n)) => {
            Value::Object(n.iter().map(|(k, y)| (k.clone(), a.get(k).map_or_else(|| y.clone(), |x| fondre(x, y, egal)))).collect())
        }
        _ => neuf.clone(),
    }
}

/// Le texte d'un oracle dans la forme du fichier qu'il remplace : d'un bloc s'il l'était, sinon indenté comme lui.
fn texte_oracle(v: &Value, ancien: &str) -> Vec<u8> {
    let mut lignes = ancien.lines();
    let retrait = match (lignes.next(), lignes.next()) {
        (Some("{" | "["), Some(l)) => Some(l.len() - l.trim_start().len()),
        _ => None,
    };
    let mut out = Vec::new();
    match retrait {
        None => serde_json::to_writer(&mut out, v).unwrap(),
        Some(n) => {
            // `to_string_pretty` indente de deux espaces ; une chaîne JSON ne contient aucun saut de ligne.
            let lignes: Vec<String> = serde_json::to_string_pretty(v).unwrap().lines().map(|l| {
                let t = l.trim_start();
                format!("{}{}", " ".repeat((l.len() - t.len()) / 2 * n), t)
            }).collect();
            out.extend_from_slice(lignes.join("\n").as_bytes());
        }
    }
    if ancien.ends_with('\n') {
        out.push(b'\n');
    }
    out
}

/// Compare `obtenu` à la partie `pointeur` (RFC 6901 ; `""` : le fichier entier) de l'oracle `tests/donnees/<nom>`, selon
/// `egal`. Sous `PUPPETPAIE_REECRIRE_ORACLES=1`, l'y écrit à la place (ce qui reste égal garde sa forme) : le diff de
/// l'oracle se relit au commit.
pub fn oracle_selon(nom: &str, pointeur: &str, obtenu: &Value, egal: impl Fn(&Value, &Value) -> bool) {
    let chemin = chemin_oracle(nom);
    if reecrire_oracles() {
        // Plusieurs tests (et processus) écrivent des parties du même fichier : chacun à son tour.
        let verrou = chemin.with_extension("verrou");
        while std::fs::OpenOptions::new().write(true).create_new(true).open(&verrou).is_err() {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let ancien = std::fs::read_to_string(&chemin).unwrap_or_default();
        let mut tout: Value = serde_json::from_str(&ancien).unwrap_or(Value::Null);
        match tout.pointer_mut(pointeur) {
            Some(p) => *p = fondre(p, obtenu, &egal),
            None if pointeur.is_empty() => tout = obtenu.clone(),
            None => {
                let (parent, cle) = pointeur.rsplit_once('/').unwrap();
                tout.pointer_mut(parent).and_then(Value::as_object_mut).unwrap_or_else(|| panic!("{} : pas de {}", nom, parent)).insert(cle.to_string(), obtenu.clone());
            }
        }
        // Un test qui lit l'oracle sans verrou ne voit jamais un fichier a moitie ecrit.
        let provisoire = chemin.with_extension(format!("{}.json", std::process::id()));
        std::fs::write(&provisoire, texte_oracle(&tout, &ancien)).unwrap();
        std::fs::rename(&provisoire, &chemin).unwrap();
        std::fs::remove_file(&verrou).unwrap();
        return;
    }
    let tout = lire_oracle(nom);
    let attendu = tout.pointer(pointeur).unwrap_or_else(|| panic!("{} : rien en {:?} (PUPPETPAIE_REECRIRE_ORACLES=1 l'écrit)", nom, pointeur));
    let mut out = Vec::new();
    ecarts(attendu, obtenu, pointeur, &egal, &mut out);
    assert!(out.is_empty(), "{} écarts avec {}{} :\n{}", out.len(), nom, pointeur, out.iter().take(8).cloned().collect::<Vec<_>>().join("\n"));
}

/// `oracle_selon` à l'égalité stricte.
pub fn oracle(nom: &str, pointeur: &str, obtenu: &Value) {
    oracle_selon(nom, pointeur, obtenu, |a, b| a == b)
}

/// FNV-1a 64 bits, en hexadécimal.
pub fn fnv(texte: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for o in texte.bytes() {
        h = (h ^ u64::from(o)).wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", h)
}

/// Les clés triées à tous les niveaux.
pub fn trie(v: &Value) -> Value {
    match v {
        Value::Object(o) => {
            let mut cles: Vec<&String> = o.keys().collect();
            cles.sort();
            Value::Object(cles.into_iter().map(|k| (k.clone(), trie(&o[k]))).collect())
        }
        Value::Array(a) => Value::Array(a.iter().map(trie).collect()),
        autre => autre.clone(),
    }
}

/// L'empreinte du JSON compact, clés triées.
pub fn sceller(v: &Value) -> String {
    fnv(&serde_json::to_string(&trie(v)).unwrap())
}

/// L'échantillon d'une grille (décision E) : un cas sur `un_sur`, tiré de son texte et d'une graine fixe. Le tirage ne
/// dépend que du cas : un oracle déjà réduit se réécrit sans rien perdre de plus.
pub fn tire(graine: u64, cas: &Value, un_sur: u64) -> bool {
    u64::from_str_radix(&fnv(&format!("{}|{}", graine, cas)), 16).unwrap() % un_sur == 0
}

