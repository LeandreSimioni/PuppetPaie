//! Le type de preavis d'une fin de contrat (S21.G00.63.001) et l'indemnite compensatrice que la paie verse : deux
//! choses que la norme veut d'accord. Les types et leurs drapeaux sont ceux de `fctu-base.json` (cahier technique DSN
//! 2026.1, p. 253), lu par sa forme (`BaseFctu`, `base`) : le signalement de fin de contrat et la page le lisent ainsi.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use socle::fichiers::{forme_en_cache, repr, FormeGardee};
use socle::num::Resultat;

/// `fctu-base.json` : les codes de la norme propres au signalement de fin de contrat de travail unique.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaseFctu {
    #[serde(rename = "_quoi")]
    _quoi: String,
    #[serde(rename = "_source")]
    _source: String,
    /// Les cardinalites du modele, bloc par bloc (cahier technique p. 122), et ce qu'elles disent.
    #[serde(rename = "_arborescence")]
    _arborescence: BTreeMap<String, String>,
    pub types_preavis: TypesPreavis,
    pub transaction: Nomenclature,
    pub rattachement_versement: Nomenclature,
    pub notification_rupture: NotificationRupture,
    #[serde(rename = "_delai")]
    _delai: String,
    #[serde(rename = "_report_en_mensuelle")]
    _report_en_mensuelle: String,
}

/// Les types de preavis (S21.G00.63.001), les codes d'une indemnite compensatrice, et les motifs qui limitent le type.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypesPreavis {
    #[serde(rename = "_rubrique")]
    _rubrique: String,
    #[serde(rename = "_quoi")]
    _quoi: String,
    pub codes: Vec<TypePreavis>,
    #[serde(rename = "_indemnite_de_preavis")]
    _indemnite_de_preavis: String,
    pub codes_52_de_l_indemnite_de_preavis: Vec<String>,
    #[serde(rename = "_codes_52_de_l_indemnite_de_preavis")]
    _codes_52: String,
    #[serde(rename = "_non_effectue")]
    _non_effectue: String,
    #[serde(rename = "_dates")]
    _dates: String,
    #[serde(rename = "_periode_essai")]
    _periode_essai: String,
    #[serde(rename = "_rupture_conventionnelle")]
    _rupture_conventionnelle: String,
    /// Par motif de rupture, les types permis ; les notes de la table sont des phrases.
    types_permis_par_motif: BTreeMap<String, PermisOuNote>,
    #[serde(rename = "_contigues")]
    _contigues: String,
}

/// Un type de preavis : son code, son libelle, s'il porte ses dates, si l'ecran le propose, s'il est non effectue, et ce
/// qu'il dit de l'indemnite compensatrice. La page le recoit tel quel.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TypePreavis {
    pub code: String,
    pub libelle: String,
    pub dates: bool,
    pub saisissable: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub non_effectue: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indemnite_de_preavis: Option<IndemniteDuType>,
}

/// Ce qu'un type de preavis dit de l'indemnite compensatrice que la paie verse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IndemniteDuType {
    Versee,
    Aucune,
}

/// Une ligne de `types_permis_par_motif` : la limite d'un motif, ou une note.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum PermisOuNote {
    Permis(Permis),
    Note(#[allow(dead_code)] String),
}

/// Les types permis a un motif, et le controle qui le veut.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Permis {
    pub types: Vec<String>,
    pub controle: String,
}

/// Une nomenclature a codes (`transaction`, `rattachement_versement`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Nomenclature {
    #[serde(rename = "_rubrique")]
    _rubrique: String,
    #[serde(rename = "_quoi")]
    _quoi: String,
    pub codes: Vec<Code>,
}

/// Un code d'une nomenclature, et s'il est celui par defaut. La page le recoit tel quel.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Code {
    pub code: String,
    pub libelle: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub defaut: bool,
}

/// Les motifs de rupture pour lesquels la date de notification (S21.G00.62.003) est obligatoire dans un FCTU.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationRupture {
    #[serde(rename = "_rubrique")]
    _rubrique: String,
    #[serde(rename = "_champ")]
    _champ: String,
    #[serde(rename = "_quoi")]
    _quoi: String,
    #[serde(rename = "_source")]
    _source: String,
    pub motifs: Vec<String>,
}

/// `fctu-base.json`, lu par sa forme.
pub fn base() -> Resultat<BaseFctu> {
    static CACHE: std::sync::OnceLock<FormeGardee<BaseFctu>> = std::sync::OnceLock::new();
    let b = forme_en_cache(&CACHE, "fctu-base.json", "Les codes du signalement de fin de contrat livrés avec le logiciel ne se lisent pas : le signaler au développeur.", Ok)?;
    Ok((*b).clone())
}

impl TypesPreavis {
    /// Le type de ce code, ou `None`.
    pub fn du_code(&self, code: &str) -> Option<&TypePreavis> {
        self.codes.iter().find(|c| c.code == code)
    }

    /// Les types permis au motif `motif`, ou `None` s'il ne limite pas le type.
    pub fn permis_au_motif(&self, motif: &str) -> Option<&Permis> {
        match self.types_permis_par_motif.get(motif) {
            Some(PermisOuNote::Permis(p)) => Some(p),
            _ => None,
        }
    }
}

impl Nomenclature {
    /// Le code par defaut.
    pub fn defaut(&self) -> Option<&str> {
        self.codes.iter().find(|c| c.defaut).map(|c| c.code.as_str())
    }
}

/// Les codes S21.G00.52.001 d'une indemnite compensatrice de preavis (`types_preavis.codes_52_de_l_indemnite_de_preavis` :
/// 023, 032 ; cahier technique 2026.1, p. 253).
pub fn codes_52_de_l_indemnite_de_preavis() -> Resultat<Vec<String>> {
    Ok(base()?.types_preavis.codes_52_de_l_indemnite_de_preavis)
}

/// Le type de preavis (S21.G00.63.001) et l'indemnite compensatrice de preavis que la paie verse disent-ils la meme
/// chose ? `versees` : les lignes de la paie declarees sous un code de `codes_52_de_l_indemnite_de_preavis`, en (code,
/// libelle). `None` s'ils s'accordent, sinon pourquoi non. Le drapeau `indemnite_de_preavis` du type (`fctu-base.json`) :
/// `versee` (02) veut une ligne, `aucune` (01, 03, 10, 60, 61, 90) n'en veut pas, un type sans
/// drapeau (50, 51) n'est pas confronte. Un type inconnu refuse.
pub fn desaccord_du_preavis(type_preavis: &str, versees: &[(String, String)]) -> Resultat<Option<String>> {
    let types = base()?.types_preavis;
    let t = types.du_code(type_preavis).ok_or_else(|| {
        format!("Type de preavis {} inconnu de la norme (S21.G00.63.001). Les types sont dans fctu-base.json.", repr(type_preavis))
    })?;
    let nom = format!("« {} - {} »", type_preavis, t.libelle);
    let codes = types.codes_52_de_l_indemnite_de_preavis.join(" ou ");
    Ok(match t.indemnite_de_preavis {
        Some(IndemniteDuType::Versee) if versees.is_empty() => Some(format!(
            "le preavis se declare {} (S21.G00.63.001), et la paie de ce contrat ne verse aucune indemnite compensatrice \
de preavis (S21.G00.52.001 = {}) : le signalement dirait paye un preavis qui ne l'est pas. Si aucune indemnite n'est due \
pour ce motif (le solde de tout compte le dit), le type est « 03 - preavis non effectue et non paye » ; si elle est due, \
c'est la saisie qui le dit (`preavis_non_effectue`), et le solde la paie.",
            nom, codes
        )),
        Some(IndemniteDuType::Aucune) if !versees.is_empty() => {
            let lignes: Vec<String> = versees.iter().map(|(c, l)| format!("« {} » ({})", l, c)).collect();
            Some(format!(
                "le preavis se declare {} (S21.G00.63.001), et la paie de ce contrat verse {} : l'indemnite d'un preavis \
non effectue. Un preavis non effectue dont l'indemnite est versee se declare « 02 - preavis non effectue et paye ».",
                nom,
                lignes.join(", ")
            ))
        }
        _ => None,
    })
}
