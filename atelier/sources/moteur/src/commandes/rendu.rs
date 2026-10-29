//! Ce que les commandes ecrivent : l'enveloppe `--json` de `moteur/sortie_json.py`, et les petits outils de mise en
//! forme qui font ressembler une ligne a celle de `%-10s %6d` du Python.

use serde_json::{json, Value};

/// `json.dumps(charge, ensure_ascii=False, indent=2)` suivi d'un saut de ligne, comme `cli._rendre`.
/// Rend le code de sortie : 0 si `ok`, 2 sinon.
pub fn rendre(charge: &Value) -> i32 {
    dire!("{}", socle::ecriture::json_indente(charge, 2));
    if charge.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        0
    } else {
        2
    }
}

/// Comme `rendre`, avec l'indentation d'un espace (`arrivee --json`).
pub fn rendre_indent(charge: &Value, indent: usize) -> i32 {
    dire!("{}", socle::ecriture::json_indente(charge, indent));
    if charge.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        0
    } else {
        2
    }
}

/// `sortie_json.enveloppe`.
pub fn enveloppe(commande: &str, resultat: Value) -> Value {
    json!({"commande": commande, "ok": true, "resultat": resultat})
}

/// Une erreur metier est une reponse. `extra` s'ajoute au corps (`type`, `message`).
pub fn erreur(commande: &str, classe: &str, message: &str, extra: &[(&str, Value)]) -> Value {
    let mut corps = serde_json::Map::new();
    corps.insert("type".into(), json!(classe));
    corps.insert("message".into(), json!(message));
    for (k, v) in extra {
        corps.insert((*k).to_string(), v.clone());
    }
    json!({"commande": commande, "ok": false, "erreur": Value::Object(corps)})
}

/// Le texte d'une valeur JSON comme `str(x)` de Python le donnerait : `None`, `True`, `0.5`.
pub fn texte(v: &Value) -> String {
    socle::valeur::valeur_en_texte(v)
}

/// Le texte d'une cle d'un objet JSON, `None` quand elle manque (Python leverait `KeyError` ; ici on est plus doux).
pub fn champ(v: &Value, cle: &str) -> String {
    v.get(cle).map(texte).unwrap_or_else(|| "None".into())
}

/// Le texte d'une cle quand elle est vraie, sinon `defaut` (`x.get(cle) or defaut`).
pub fn champ_ou(v: &Value, cle: &str, defaut: &str) -> String {
    match v.get(cle) {
        Some(x) if socle::fichiers::vrai(x) => texte(x),
        _ => defaut.to_string(),
    }
}

/// Les `n` premiers caracteres (`texte[:n]`).
pub fn debut(t: &str, n: usize) -> String {
    t.chars().take(n).collect()
}

/// `jj_mm_aaaa` d'un `AAAA-MM-JJ` (les dix premiers caracteres).
pub fn jj_mm_aaaa_de_iso(iso: &str) -> String {
    socle::date::Jour::lire(&debut(iso, 10)).map(|j| j.jj_mm_aaaa()).unwrap_or_else(|_| iso.to_string())
}

/// `x.get("quand")[:16].replace("T", " ")` -- l'heure d'une ligne de journal.
pub fn quand_court(v: Option<&Value>) -> String {
    let t = v.filter(|x| socle::fichiers::vrai(x)).map(texte).unwrap_or_default();
    debut(&t, 16).replace('T', " ")
}

/// Une seule ligne, separateurs `, ` et `: `.
pub fn json_ligne(v: &Value) -> String {
    fn chaine(t: &str, s: &mut String) {
        s.push('"');
        for c in t.chars() {
            match c {
                '"' => s.push_str("\\\""),
                '\\' => s.push_str("\\\\"),
                '\u{8}' => s.push_str("\\b"),
                '\u{c}' => s.push_str("\\f"),
                '\n' => s.push_str("\\n"),
                '\r' => s.push_str("\\r"),
                '\t' => s.push_str("\\t"),
                c if (c as u32) < 0x20 => s.push_str(&format!("\\u{:04x}", c as u32)),
                c => s.push(c),
            }
        }
        s.push('"');
    }
    fn ecrire(v: &Value, s: &mut String) {
        match v {
            Value::Null => s.push_str("null"),
            Value::Bool(b) => s.push_str(if *b { "true" } else { "false" }),
            Value::Number(n) => match (n.as_i64(), n.as_u64()) {
                (Some(i), _) => s.push_str(&i.to_string()),
                (None, Some(u)) => s.push_str(&u.to_string()),
                _ => s.push_str(&socle::ecriture::repr_flottant(n.as_f64().unwrap_or(0.0))),
            },
            Value::String(t) => chaine(t, s),
            Value::Array(a) => {
                s.push('[');
                for (i, x) in a.iter().enumerate() {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    ecrire(x, s);
                }
                s.push(']');
            }
            Value::Object(o) => {
                s.push('{');
                for (i, (k, x)) in o.iter().enumerate() {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    chaine(k, s);
                    s.push_str(": ");
                    ecrire(x, s);
                }
                s.push('}');
            }
        }
    }
    let mut s = String::new();
    ecrire(v, &mut s);
    s
}

/// `d[cle]` : le texte de la cle, ou le `KeyError` que Python leverait.
pub fn exiger(v: &Value, cle: &str) -> Result<String, socle::echec::Echec> {
    match v.get(cle) {
        Some(x) => Ok(texte(x)),
        None => Err(socle::echec::Echec { classe: Some("KeyError"), message: socle::fichiers::repr(cle) }),
    }
}

/// `sortie_json.brut` d'un JSON : les cles qui commencent par `_` s'en vont, a tous les niveaux.
pub fn brut(v: &Value) -> Value {
    match v {
        Value::Object(o) => Value::Object(o.iter().filter(|(k, _)| !k.starts_with('_')).map(|(k, x)| (k.clone(), brut(x))).collect()),
        Value::Array(a) => Value::Array(a.iter().map(brut).collect()),
        autre => autre.clone(),
    }
}
