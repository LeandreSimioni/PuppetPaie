//! LA LECTURE D'UNE FORME : une donnee lue (un fichier, un bareme, la charge d'une route) est une structure derivee
//! (`serde::Deserialize`), dont chaque type porte `#[serde(deny_unknown_fields)]` (garde par
//! `moteur/tests/fichiers/structure.rs`) : une cle que la forme ne connait pas est refusee, sans exception. `lire` est la
//! seule fonction qui la lit ; son refus est une phrase du moteur, la cle fautive (son chemin dans la donnee) et ce qui ne
//! va pas a part (`echec::APart`).
//!
//! C'est un des trois modules du BORD (avec `ecriture` et `json_ascii`, qui ecrivent) : le seul endroit d'une couche
//! fermee ou le JSON se manipule sans forme (`moteur/tests/fichiers/structure.rs`, `une_couche_fermee_ne_lit_que_des_formes`).

use std::path::Path;

use crate::echec::APart;
use crate::num::Resultat;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// La donnee `v` lue en `T`. `phrase` est celle du refus, pour l'ecran (« Le barème livré ne se lit pas : le signaler au
/// développeur. ») ; le chemin de la cle et le detail vont a part.
pub fn lire<T: DeserializeOwned>(v: &Value, phrase: &str) -> Resultat<T> {
    serde_path_to_error::deserialize(v).map_err(|e| refus(phrase, None, &e))
}

/// Le texte JSON `texte` lu en `T` (`lire`, sans passer par une `Value`).
pub fn lire_texte<T: DeserializeOwned>(texte: &str, phrase: &str) -> Resultat<T> {
    let mut d = serde_json::Deserializer::from_str(texte);
    serde_path_to_error::deserialize(&mut d).map_err(|e| refus(phrase, None, &e))
}

/// Le fichier JSON `fichier` lu en `T` (`lire_texte`, sa lecture notee au registre d'un calcul suivi :
/// `fichiers::lire_texte`). Le disque qui refuse se dit par `echec::du_disque` ; une forme fausse par `phrase`, le
/// chemin du fichier, la cle et le detail a part.
pub fn lire_fichier<T: DeserializeOwned>(fichier: &Path, phrase: &str) -> Resultat<T> {
    let texte = crate::fichiers::lire_texte(fichier).map_err(|e| crate::echec::du_disque(fichier, &e))?;
    let mut d = serde_json::Deserializer::from_str(&texte);
    serde_path_to_error::deserialize(&mut d).map_err(|e| refus(phrase, Some(fichier), &e))
}

/// La longueur, en octets, de l'objet JSON qui ouvre `texte` (ce qu'un refus porte a part, `echec::avec`), s'il en
/// ouvre un.
pub fn objet_en_tete(texte: &str) -> Option<usize> {
    let mut lu = serde_json::Deserializer::from_str(texte).into_iter::<serde::de::IgnoredAny>();
    match (texte.starts_with('{'), lu.next()) {
        (true, Some(Ok(_))) => Some(lu.byte_offset()),
        _ => None,
    }
}

fn refus(phrase: &str, fichier: Option<&Path>, e: &serde_path_to_error::Error<serde_json::Error>) -> String {
    let cle = e.path().to_string();
    let detail = en_clair(&e.inner().to_string());
    crate::echec::avec(
        phrase,
        APart {
            chemin: fichier.map(|f| f.to_string_lossy().into_owned()),
            cle: Some(if cle == "." { String::new() } else { cle }),
            detail: Some(detail).filter(|d| !d.is_empty()),
            ..APart::default()
        },
    )
}

/// Le message de serde, dit en francais pour ce qu'il a de courant ; le reste tel quel.
fn en_clair(m: &str) -> String {
    let m = m.split(" at line ").next().unwrap_or(m);
    if let Some(r) = m.strip_prefix("unknown field ") {
        let cle = r.split(',').next().unwrap_or(r);
        return match r.split_once("expected ") {
            Some((_, a)) => format!("la clé {} n'est pas connue ici (attendues : {})", cle, a.replace("one of ", "")),
            None => format!("la clé {} n'est pas connue ici : aucune n'est attendue", cle),
        };
    }
    if let Some(r) = m.strip_prefix("missing field ") {
        return format!("la clé {} manque", r);
    }
    if let Some(r) = m.strip_prefix("invalid type: ") {
        return format!("la valeur n'a pas la forme attendue : {}", r.replace(", expected ", ", attendu "));
    }
    if let Some(r) = m.strip_prefix("unknown variant ") {
        return format!("la valeur {} n'est pas connue ici", r.replace(", expected one of ", " (attendues : ").replace(", expected ", " (attendue : ").replace("` or `", "` ou `") + ")");
    }
    m.to_string()
}

/// UN MONTANT OU UN TAUX EN DECIMAL : un entier garde son echelle nulle, un flottant sa representation la plus courte
/// (`1.0` reste `1.0`, `num::dec_f64`), un nombre ecrit en texte se lit (`num::dec_texte`).
/// `#[serde(deserialize_with = "socle::forme::decimal")]`.
pub fn decimal<'de, D: serde::Deserializer<'de>>(d: D) -> Result<crate::num::Decimal, D::Error> {
    struct EnDecimal;
    impl serde::de::Visitor<'_> for EnDecimal {
        type Value = crate::num::Decimal;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("un nombre")
        }
        fn visit_i64<E: serde::de::Error>(self, i: i64) -> Result<Self::Value, E> {
            Ok(i.into())
        }
        fn visit_u64<E: serde::de::Error>(self, u: u64) -> Result<Self::Value, E> {
            Ok(u.into())
        }
        fn visit_f64<E: serde::de::Error>(self, x: f64) -> Result<Self::Value, E> {
            crate::num::dec_f64(x).map_err(E::custom)
        }
        fn visit_str<E: serde::de::Error>(self, s: &str) -> Result<Self::Value, E> {
            crate::num::dec_texte(s).map_err(E::custom)
        }
    }
    d.deserialize_any(EnDecimal)
}

/// `decimal`, quand la valeur peut manquer ou etre nulle (avec `#[serde(default)]`).
pub fn decimal_opt<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<crate::num::Decimal>, D::Error> {
    <Option<Nombre> as serde::Deserialize>::deserialize(d).map(|n| n.map(|n| n.0))
}

/// Une cle exigee meme nulle (`Option` la tiendrait pour absente) : `#[serde(deserialize_with = "socle::forme::presente")]`,
/// sans `default`. La part d'une rubrique s'ecrit, nulle quand il n'y en a pas.
pub fn presente<'de, D: serde::Deserializer<'de>, T: serde::Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    <Option<T> as serde::Deserialize>::deserialize(d)
}

/// Une cle qui peut manquer (rien ne change) ou etre nulle (elle s'efface) : `#[serde(default, deserialize_with =
/// "socle::forme::dite")]` sur un `Option<Option<T>>` -- absente, `None` ; nulle, `Some(None)`.
pub fn dite<'de, D: serde::Deserializer<'de>, T: serde::Deserialize<'de>>(d: D) -> Result<Option<Option<T>>, D::Error> {
    <Option<T> as serde::Deserialize>::deserialize(d).map(Some)
}

/// UN OBJET DONT L'ORDRE COMPTE (un catalogue montre dans l'ordre du fichier), lu en paires `(cle, valeur)` :
/// `#[serde(deserialize_with = "socle::forme::en_ordre")]` sur un `Vec<(String, T)>`. Une cle repetee refuse.
pub fn en_ordre<'de, D: serde::Deserializer<'de>, T: serde::Deserialize<'de>>(d: D) -> Result<Vec<(String, T)>, D::Error> {
    struct Visiteur<T>(std::marker::PhantomData<T>);
    impl<'de, T: serde::Deserialize<'de>> serde::de::Visitor<'de> for Visiteur<T> {
        type Value = Vec<(String, T)>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("un objet")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(self, mut m: A) -> Result<Self::Value, A::Error> {
            let mut out: Vec<(String, T)> = Vec::new();
            while let Some((k, v)) = m.next_entry::<String, T>()? {
                if out.iter().any(|(x, _)| *x == k) {
                    return Err(serde::de::Error::custom(format!("la clé {} est répétée", k)));
                }
                out.push((k, v));
            }
            Ok(out)
        }
    }
    d.deserialize_map(Visiteur(std::marker::PhantomData))
}

/// Un nombre decimal, lu comme `decimal`, la ou un champ ne se pose pas (une valeur de table).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Nombre(pub crate::num::Decimal);

impl<'de> serde::Deserialize<'de> for Nombre {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        decimal(d).map(Nombre)
    }
}

/// `lire`, quand la phrase du refus nomme elle-meme la cle fautive : `phrase(chemin, detail)` la compose (le fichier
/// d'une compagnie, qu'elle corrige : `droit::fiche`).
pub fn lire_en_phrase<T: DeserializeOwned>(v: &Value, phrase: impl Fn(&str, &str) -> String) -> Resultat<T> {
    serde_path_to_error::deserialize(v).map_err(|e| refus_en_phrase(&phrase, &e))
}

/// `lire_en_phrase`, du fichier JSON `fichier` (sa lecture notee au registre d'un calcul suivi, `fichiers::lire_texte`) ;
/// le disque qui refuse se dit par `echec::du_disque`.
pub fn lire_fichier_en_phrase<T: DeserializeOwned>(fichier: &Path, phrase: impl Fn(&str, &str) -> String) -> Resultat<T> {
    let texte = crate::fichiers::lire_texte(fichier).map_err(|e| crate::echec::du_disque(fichier, &e))?;
    let mut d = serde_json::Deserializer::from_str(&texte);
    serde_path_to_error::deserialize(&mut d).map_err(|e| refus_en_phrase(&phrase, &e))
}

fn refus_en_phrase(phrase: &impl Fn(&str, &str) -> String, e: &serde_path_to_error::Error<serde_json::Error>) -> String {
    let chemin = e.path().to_string();
    let chemin = if chemin == "." { String::new() } else { chemin };
    let detail = en_clair(&e.inner().to_string());
    crate::echec::avec(phrase(&chemin, &detail), APart { cle: Some(chemin), detail: Some(detail).filter(|d| !d.is_empty()), ..APart::default() })
}
