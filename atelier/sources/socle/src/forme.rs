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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Deserialize, Debug)]
    #[serde(deny_unknown_fields)]
    #[allow(dead_code)]
    struct Ligne {
        id: String,
        taux: f64,
    }

    #[derive(serde::Deserialize, Debug)]
    #[serde(deny_unknown_fields)]
    #[allow(dead_code)]
    struct Fichier {
        lignes: Vec<Ligne>,
    }

    #[test]
    fn une_cle_inconnue_se_refuse_avec_son_chemin() {
        let v = serde_json::json!({"lignes": [{"id": "a", "taux": 1.0}, {"id": "b", "taux": 2.0, "tuax": 3}]});
        let e = lire::<Fichier>(&v, "Le fichier ne se lit pas.").unwrap_err();
        let (phrase, plus) = crate::echec::separer(&e);
        assert_eq!(phrase, "Le fichier ne se lit pas.");
        assert_eq!(plus["cle"], "lignes[1].tuax", "{}", e);
        assert!(plus["detail"].as_str().unwrap().contains("`tuax` n'est pas connue"), "{}", e);
    }
}
