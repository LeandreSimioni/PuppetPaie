//! Ce que les commandes ecrivent : l'enveloppe `--json`, et les petits outils de mise en forme d'une ligne en colonnes
//! (`%-10s %6d` : un texte cale a gauche, un nombre a droite).

use serde_json::{json, Value};

/// La charge en JSON indente de deux espaces, suivi d'un saut de ligne.
/// Rend le code de sortie : 0 si `ok`, 2 sinon.
pub fn rendre(charge: &Value) -> i32 {
    dire!("{}", socle::ecriture::json_indente(charge, 2));
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

/// Le texte d'une valeur JSON : `None` pour null, `True`, `0.5`.
pub fn texte(v: &Value) -> String {
    documents::valeur::valeur_en_texte(v)
}

/// Le texte d'une cle d'un objet JSON, `None` quand elle manque.
pub fn champ(v: &Value, cle: &str) -> String {
    v.get(cle).map(texte).unwrap_or_else(|| "None".into())
}

/// Les `n` premiers caracteres (`texte[:n]`).
pub fn debut(t: &str, n: usize) -> String {
    t.chars().take(n).collect()
}


/// Le texte de la cle, ou le refus `KeyError` quand elle manque.
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
