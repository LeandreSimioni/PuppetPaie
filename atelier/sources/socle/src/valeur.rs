//! Une valeur JSON lue telle que les fichiers la portent : son nom de type, son egalite (`1 == 1.0 == true`), sa forme
//! ecrite dans une phrase de refus.

use serde_json::Value;

use crate::ecriture::repr_flottant;
use crate::fichiers::repr;

/// Le nom du type d'une valeur, tel que les phrases de refus le disent (`fichiers::attendu`).
pub fn nom_type(v: &Value) -> &'static str {
    match v {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::Number(n) if n.is_f64() => "float",
        Value::Number(_) => "int",
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
    }
}

/// Un nombre, ou un booleen compte comme 1 ou 0.
pub fn nombre(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    }
}

/// L'egalite des valeurs lues : `1 == 1.0 == true`, listes et dictionnaires en profondeur.
pub fn valeurs_egales(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(u, v)| valeurs_egales(u, v)),
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| valeurs_egales(v, w)))
        }
        _ => match (nombre(a), nombre(b)) {
            (Some(x), Some(y)) => x == y,
            _ => a == b,
        },
    }
}

/// La forme ecrite d'une valeur : `None`, `True`, un texte entre apostrophes, une liste entre crochets.
pub fn valeur_ecrite(v: &Value) -> String {
    match v {
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::Number(n) => match (n.as_i64(), n.as_u64()) {
            (Some(i), _) => i.to_string(),
            (_, Some(u)) => u.to_string(),
            _ => repr_flottant(n.as_f64().unwrap_or(0.0)),
        },
        Value::String(s) => repr(s),
        Value::Array(a) => format!("[{}]", a.iter().map(valeur_ecrite).collect::<Vec<_>>().join(", ")),
        Value::Object(o) => format!(
            "{{{}}}",
            o.iter().map(|(k, v)| format!("{}: {}", repr(k), valeur_ecrite(v))).collect::<Vec<_>>().join(", ")
        ),
    }
}

/// Une valeur en texte : un texte tel quel, le reste sous sa forme ecrite (`valeur_ecrite`).
pub fn valeur_en_texte(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        autre => valeur_ecrite(autre),
    }
}
