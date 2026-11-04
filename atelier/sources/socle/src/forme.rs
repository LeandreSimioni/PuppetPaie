//! LA LECTURE D'UNE FORME : une donnee lue (un fichier, un bareme, la charge d'une route) est une structure derivee
//! (`serde::Deserialize`), dont chaque type porte `#[serde(deny_unknown_fields)]` (garde par
//! `moteur/tests/fichiers/structure.rs`) : une cle que la forme ne connait pas est refusee, sans exception. `lire` est la
//! seule fonction qui la lit ; son refus est une phrase du moteur, la cle fautive (son chemin dans la donnee) et ce qui ne
//! va pas a part (`echec::a_la_cle`).

use crate::num::Resultat;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// La donnee `v` lue en `T`. `phrase` est celle du refus, pour l'ecran (« Le barème livré ne se lit pas : le signaler au
/// développeur. ») ; le chemin de la cle et le detail vont a part.
pub fn lire<T: DeserializeOwned>(v: &Value, phrase: &str) -> Resultat<T> {
    serde_path_to_error::deserialize(v).map_err(|e| refus(phrase, &e))
}

/// Le texte JSON `texte` lu en `T` (`lire`, sans passer par une `Value`).
pub fn lire_texte<T: DeserializeOwned>(texte: &str, phrase: &str) -> Resultat<T> {
    let mut d = serde_json::Deserializer::from_str(texte);
    serde_path_to_error::deserialize(&mut d).map_err(|e| refus(phrase, &e))
}

fn refus(phrase: &str, e: &serde_path_to_error::Error<serde_json::Error>) -> String {
    let chemin = e.path().to_string();
    crate::echec::a_la_cle(phrase, if chemin == "." { "" } else { &chemin }, en_clair(&e.inner().to_string()))
}

/// Le message de serde, dit en francais pour ce qu'il a de courant ; le reste tel quel.
fn en_clair(m: &str) -> String {
    let m = m.split(" at line ").next().unwrap_or(m);
    if let Some(r) = m.strip_prefix("unknown field ") {
        let cle = r.split(',').next().unwrap_or(r);
        let attendues = r.split_once("expected ").map(|(_, a)| a.replace("one of ", "")).unwrap_or_default();
        return format!("la clé {} n'est pas connue ici (attendues : {})", cle, attendues);
    }
    if let Some(r) = m.strip_prefix("missing field ") {
        return format!("la clé {} manque", r);
    }
    if let Some(r) = m.strip_prefix("invalid type: ") {
        return format!("la valeur n'a pas la forme attendue : {}", r.replace(", expected ", ", attendu "));
    }
    if let Some(r) = m.strip_prefix("unknown variant ") {
        return format!("la valeur {} n'est pas connue ici", r.replace(", expected one of ", " (attendues : ").replace(", expected ", " (attendue : ") + ")");
    }
    m.to_string()
}

/// UN MONTANT OU UN TAUX EN DECIMAL, lu comme `num::dec` le lit d'une `Value` : un entier garde son echelle nulle, un
/// flottant sa representation la plus courte (`1.0` reste `1.0`). `#[serde(deserialize_with = "socle::forme::decimal")]`.
pub fn decimal<'de, D: serde::Deserializer<'de>>(d: D) -> Result<crate::num::Decimal, D::Error> {
    let v: Value = serde::Deserialize::deserialize(d)?;
    crate::num::dec(&v).map_err(serde::de::Error::custom)
}

/// `decimal`, quand la valeur peut manquer ou etre nulle (avec `#[serde(default)]`).
pub fn decimal_opt<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<crate::num::Decimal>, D::Error> {
    let v: Value = serde::Deserialize::deserialize(d)?;
    if v.is_null() {
        return Ok(None);
    }
    crate::num::dec(&v).map(Some).map_err(serde::de::Error::custom)
}

/// Une cle exigee meme nulle (`Option` la tiendrait pour absente) : `#[serde(deserialize_with = "socle::forme::presente")]`,
/// sans `default`. La part d'une rubrique s'ecrit, nulle quand il n'y en a pas.
pub fn presente<'de, D: serde::Deserializer<'de>, T: serde::Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    <Option<T> as serde::Deserialize>::deserialize(d)
}

/// Un nombre decimal, lu comme `decimal`, la ou un champ ne se pose pas (une valeur de table).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Nombre(pub crate::num::Decimal);

impl<'de> serde::Deserialize<'de> for Nombre {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        decimal(d).map(Nombre)
    }
}
