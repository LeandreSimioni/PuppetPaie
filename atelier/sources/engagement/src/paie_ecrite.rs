//! LA PAIE ECRITE : ce que la validation d'un engagement ecrit de sa paie d'un mois (`etats::calculer_les_rangs`), dans le
//! fichier de ce mois (`paies`), et ce que ses envois y posent ensuite (le numero du bulletin, le premier N CE, le
//! bulletin qu'elle rectifie, pourquoi elle s'est recalculee). Une seule forme, lue par `PaieEcrite::lire` seule et ecrite
//! par serde ; une cle inconnue refuse en se nommant.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use socle::num::Resultat;

/// UNE PAIE ECRITE.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaieEcrite {
    /// La paie, sous sa forme canonique (`relire::ecrire`). TRAVAIL A FAIRE (chaine A) : la forme canonique de la paie
    /// (`calcul::Paie::canon`, relue par `relire::paie_lue`) se declare.
    pub paie: Value,
    /// Les fiches du salarie telles que le calcul les a lues.
    pub fiches: crate::relire::FichesDuCalcul,
    /// Ce que la saisie du mois posait pour tous ses engagements.
    pub saisie_du_mois: crate::saisie::CeQueLeMoisDit,
    /// Ce dont le calcul dependait (`empreinte`).
    pub empreinte: Empreinte,
    /// Le prelevement a la source (`etats::prelevement`).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "pas")]
    pub pas: Option<droit::pas::Versement>,
    /// Le compteur de conges d'un permanent a la fin du mois, et ce qu'il tient pour le mois suivant.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "compteur")]
    pub conges: Option<crate::conges::CompteurConges>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "avant")]
    pub conges_avant: Option<crate::conges::Avant>,
    /// Le numero du bulletin et le premier N CE, ecrits quand ses documents sont partis (`etats::poser_numeros_des_documents`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bulletin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ce: Option<i64>,
    /// Le numero du bulletin d'une paie partie que celle-ci rectifie.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rectifie: Option<String>,
    /// Pourquoi la paie s'est recalculee d'elle-meme, et quand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recalcul: Option<Recalcul>,
}

/// CE DONT LE CALCUL D'UNE PAIE DEPENDAIT : l'empreinte de chaque chose lue, les regles datees et les valeurs datees de la
/// compagnie (`empreinte`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Empreinte {
    pub entrees: BTreeMap<String, String>,
    pub regles: Vec<String>,
    pub configuration: Vec<String>,
}

/// Une paie recalculee d'elle-meme : quand, et pourquoi (`etats::raison`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recalcul {
    pub le: String,
    pub raison: String,
}

impl PaieEcrite {
    /// LA SEULE LECTURE D'UNE PAIE ECRITE.
    pub fn lire(v: &Value) -> Resultat<PaieEcrite> {
        socle::forme::lire(v, socle::echec::PAIE_ILLISIBLE)
    }

    /// Le compteur de conges sans ses avertissements, pour comparer deux paies sur leurs chiffres.
    pub fn conges_sans_avis(&self) -> Option<crate::conges::CompteurConges> {
        self.conges.clone().map(|c| crate::conges::CompteurConges { avertissements: Vec::new(), ..c })
    }
}

/// Le prelevement, tel que `droit::pas::Versement` l'ecrit et le relit.
mod pas {
    use super::*;
    pub fn serialize<S: Serializer>(v: &Option<droit::pas::Versement>, s: S) -> Result<S::Ok, S::Error> {
        v.as_ref().map(droit::pas::Versement::canon).serialize(s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<droit::pas::Versement>, D::Error> {
        Option::<Value>::deserialize(d)?.map(|v| droit::pas::Versement::lire(&v)).transpose().map_err(serde::de::Error::custom)
    }
}

/// Le compteur de conges, tel que `conges::CompteurConges` l'ecrit et le relit.
mod compteur {
    use super::*;
    pub fn serialize<S: Serializer>(v: &Option<crate::conges::CompteurConges>, s: S) -> Result<S::Ok, S::Error> {
        v.as_ref().map(crate::conges::CompteurConges::en_json).serialize(s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<crate::conges::CompteurConges>, D::Error> {
        Option::<Value>::deserialize(d)?.map(|v| crate::conges::CompteurConges::lire(&v)).transpose().map_err(serde::de::Error::custom)
    }
}

/// Le compteur tenu pour le mois suivant, tel que `conges::Avant` l'ecrit et le relit.
mod avant {
    use super::*;
    pub fn serialize<S: Serializer>(v: &Option<crate::conges::Avant>, s: S) -> Result<S::Ok, S::Error> {
        v.as_ref().map(crate::conges::Avant::en_json).serialize(s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<crate::conges::Avant>, D::Error> {
        Option::<Value>::deserialize(d)?.map(|v| crate::conges::Avant::lire(&v)).transpose().map_err(serde::de::Error::custom)
    }
}
