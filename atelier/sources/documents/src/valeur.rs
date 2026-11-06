//! UNE VALEUR JSON LUE PAR CLE, au-dessus des couches fermees : ce que les parties qui n'ont pas encore leurs formes
//! (documents, declarations, envois, moteur) lisent dans une `Value` prise ou rendue. Le socle, le droit et l'engagement
//! ne lisent que des formes (`socle::forme`) : ce module ne leur sert pas, et tombe avec son dernier lecteur.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use rust_decimal::prelude::*;
use serde_json::{Map, Value};

use socle::date::{repr_flottant, repr_texte, Jour};
use socle::echec::{erreur, Echec, A_PART, R};
use socle::fichiers::{attendu, lire_texte, lire_texte_echec, noter_si_besoin, repr, signature};
use socle::num::{dec_f64, dec_texte, illisible, Resultat};

// ------------------------------------------------------------ une date

/// Une date lue dans une valeur JSON : une chaine, un nombre (lu en texte), ou la date detaillee
/// `{"jour": ..., "lieu": ...}` qui se lit par son `jour`.
pub fn d(v: &Value) -> Resultat<Jour> {
    match v {
        Value::Object(o) => match o.get("jour") {
            Some(j) => d(j),
            None => Err(socle::echec::a_la_cle("Une date ne dit pas son jour.", "jour", forme_ecrite(v))),
        },
        Value::String(s) => Jour::lire(s),
        autre => Jour::lire(&en_texte(autre)),
    }
}

/// La forme ecrite d'une valeur dans un message : objet, liste, texte cite, nombre, `True`, `None`.
pub fn forme_ecrite(v: &Value) -> String {
    match v {
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.to_string()
            } else if let Some(u) = n.as_u64() {
                u.to_string()
            } else {
                repr_flottant(n.as_f64().unwrap_or(f64::NAN))
            }
        }
        Value::String(s) => repr_texte(s),
        Value::Array(a) => format!("[{}]", a.iter().map(forme_ecrite).collect::<Vec<_>>().join(", ")),
        Value::Object(o) => format!(
            "{{{}}}",
            o.iter().map(|(k, v)| format!("{}: {}", repr_texte(k), forme_ecrite(v))).collect::<Vec<_>>().join(", ")
        ),
    }
}

/// Une valeur en texte : un texte tel quel, le reste sous sa forme ecrite.
pub fn en_texte(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        autre => forme_ecrite(autre),
    }
}

// ------------------------------------------------------------ un refus

/// La phrase d'un refus, et ce qu'il porte a part (`socle::echec::avec`), reunis d'un bout a l'autre du message.
pub fn separer(message: &str) -> (String, Map<String, Value>) {
    let mut morceaux = message.split(A_PART);
    let mut phrase = morceaux.next().unwrap_or_default().to_string();
    let mut plus = Map::new();
    for m in morceaux {
        let mut lu = serde_json::Deserializer::from_str(m).into_iter::<Value>();
        match lu.next() {
            Some(Ok(Value::Object(o))) => {
                for (k, v) in o {
                    plus.entry(k).or_insert(v);
                }
                phrase.push_str(&m[lu.byte_offset()..]);
            }
            _ => phrase.push_str(m),
        }
    }
    (phrase, plus)
}

// ------------------------------------------------------------ un fichier

type Cache = Mutex<HashMap<PathBuf, ((u128, u64), Arc<Value>)>>;

/// Un fichier JSON lu une fois tant qu'il ne change pas (objet partage : on ne le modifie pas).
pub fn json_en_cache(chemin: &Path) -> Resultat<Arc<Value>> {
    json_en_cache_ou(chemin, lire_json)
}

/// `json_en_cache`, lu par `lire` quand il faut le lire : le refus d'un fichier illisible reste celui de l'appelant (sa
/// classe, son message), et ne se garde jamais -- seul un JSON lu se garde, sous la signature du fichier (date, taille).
/// `lire` doit noter sa lecture (`lire_texte`) ; servi du cache, le fichier se note ici (`noter_si_besoin`).
pub fn json_en_cache_ou<E>(chemin: &Path, lire: impl FnOnce(&Path) -> Result<Value, E>) -> Result<Arc<Value>, E> {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let sig = signature(chemin);
    if let Some(s) = sig {
        if let Some((vue, v)) = cache.lock().unwrap().get(chemin) {
            if *vue == s {
                noter_si_besoin(chemin);
                return Ok(v.clone());
            }
        }
    }
    let v = Arc::new(lire(chemin)?);
    if let Some(s) = sig {
        cache.lock().unwrap().insert(chemin.to_path_buf(), (s, v.clone()));
    }
    Ok(v)
}

/// Le JSON d'un fichier UTF-8, sans cache.
pub fn lire_json(chemin: &Path) -> Resultat<Value> {
    let texte = lire_texte(chemin).map_err(|e| socle::echec::du_disque(&chemin, &e))?;
    serde_json::from_str(&texte).map_err(|e| socle::echec::du_disque(&chemin, &e))
}

/// Un fichier JSON (`lire_texte_echec`, puis `JSONDecodeError` s'il ne se lit pas : la phrase en mots, le chemin et ce
/// qui ne va pas a part, `echec::du_disque`).
pub fn lire_json_echec(p: &Path) -> Result<Value, Echec> {
    serde_json::from_str(&lire_texte_echec(p)?).map_err(|e| erreur("JSONDecodeError", socle::echec::du_disque(p, &e)))
}

// ------------------------------------------------------------ un nombre

/// Le decimal d'un nombre JSON (entier ou flottant, par sa forme ecrite la plus courte) ou d'une chaine.
pub fn dec(v: &Value) -> Resultat<Decimal> {
    match v {
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Decimal::from(i))
            } else if let Some(u) = n.as_u64() {
                Ok(Decimal::from(u))
            } else {
                dec_f64(n.as_f64().unwrap_or(f64::NAN))
            }
        }
        Value::String(s) => dec_texte(s),
        autre => Err(format!("nombre attendu, lu {}", autre)),
    }
}

/// Le decimal d'un nombre JSON dont un flottant garde son echelle (`130.0`).
pub fn dec_avec_echelle(v: &Value) -> Resultat<Decimal> {
    match v {
        Value::Number(n) if n.as_i64().is_none() && n.as_u64().is_none() => {
            let s = repr_flottant(n.as_f64().unwrap_or(f64::NAN));
            Decimal::from_str(&s)
                .or_else(|_| Decimal::from_scientific(&s))
                .map_err(|_| format!("nombre hors de portee : {}", s))
        }
        autre => dec(autre),
    }
}

/// Le flottant d'une valeur JSON (un nombre, ou un nombre ecrit en texte).
pub fn flottant(v: &Value) -> Resultat<f64> {
    match v {
        Value::Number(n) => n.as_f64().ok_or_else(|| format!("nombre illisible : {}", n)),
        Value::String(s) => s.trim().parse::<f64>().map_err(|_| illisible(s)),
        autre => Err(format!("nombre attendu, lu {}", autre)),
    }
}

/// `socle::texte::euros` d'une valeur (`12.8`, `"12,80"`) ; rien pour une valeur absente, le texte tel quel s'il n'est
/// pas un nombre.
pub fn euros_de(v: &Value) -> String {
    let x = match v {
        Value::Null => return String::new(),
        Value::String(s) => s.trim().replace(',', ".").parse().ok(),
        Value::Number(_) => dec(v).ok(),
        _ => None,
    };
    x.map_or_else(|| valeur_en_texte(v), socle::texte::euros)
}

// ------------------------------------------------------------ une valeur

/// Une valeur est-elle vraie ? Faux : `null`, `false`, zero, un texte, une liste ou un objet vides.
pub fn vrai(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|x| x != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// Une valeur JSON citee dans un message (`None`, `True`, `12`, `'x'`).
pub fn repr_valeur(v: &Value) -> String {
    match v {
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::String(s) => repr(s),
        Value::Array(a) => format!("[{}]", a.iter().map(repr_valeur).collect::<Vec<_>>().join(", ")),
        Value::Object(o) => format!("{{{}}}", o.iter().map(|(k, v)| format!("{}: {}", repr(k), repr_valeur(v))).collect::<Vec<_>>().join(", ")),
        autre => autre.to_string(),
    }
}

/// Le nom du type d'une valeur, tel que les phrases de refus le disent (`socle::fichiers::attendu`).
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
            _ => socle::ecriture::repr_flottant(n.as_f64().unwrap_or(0.0)),
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

/// `{}`, a rendre par reference.
pub fn objet_vide() -> &'static Value {
    static V: OnceLock<Value> = OnceLock::new();
    V.get_or_init(|| Value::Object(Map::new()))
}

/// `""`, a rendre par reference.
pub fn texte_vide() -> &'static Value {
    static V: OnceLock<Value> = OnceLock::new();
    V.get_or_init(|| Value::String(String::new()))
}

/// `v.get(k)`.
pub fn get<'a>(v: &'a Value, k: &str) -> R<&'a Value> {
    static NUL: Value = Value::Null;
    getd(v, k, &NUL)
}

/// Sur autre chose qu'un dictionnaire, `AttributeError`.
pub fn getd<'a>(v: &'a Value, k: &str, defaut: &'a Value) -> R<&'a Value> {
    match v {
        Value::Object(o) => Ok(o.get(k).unwrap_or(defaut)),
        _ => Err(erreur("AttributeError", attendu("un objet", nom_type(v)))),
    }
}

/// `v or defaut`.
pub fn ou<'a>(v: &'a Value, defaut: &'a Value) -> &'a Value {
    if vrai(v) {
        v
    } else {
        defaut
    }
}
