//! LES ORGANISMES DE RECOUVREMENT (`urssaf-dpae-base.json`, extrait par `moteur extraire urssaf`) : le code a trois
//! chiffres que la DPAE attend en C_URSSAF (positions 133-136, annexe II.1 de la norme), et l'organisme du territoire
//! de chaque departement. `base` est sa seule lecture.

use std::collections::BTreeMap;

use serde::Deserialize;

use socle::fichiers::{dossier_du_code, json_en_cache};
use socle::num::Resultat;

/// `urssaf-dpae-base.json`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaseUrssaf {
    #[serde(rename = "_quoi")]
    _quoi: String,
    #[serde(rename = "_source")]
    _source: String,
    #[serde(rename = "_par_departement_est_une_suggestion")]
    _suggestion: String,
    #[serde(rename = "_defaut")]
    _defaut: String,
    pub organismes: Vec<Organisme>,
    /// L'organisme de chaque ancienne region (2015) : ce d'ou l'extraction tire `par_departement`.
    #[serde(rename = "par_ancienne_region")]
    _par_ancienne_region: BTreeMap<String, String>,
    /// L'organisme du territoire de chaque departement (deux chiffres, trois outre-mer, 2A et 2B par 20).
    par_departement: BTreeMap<String, String>,
}

/// Un organisme de recouvrement : son code, sa codification, son nom, son SIRET et son code emetteur.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct Organisme {
    pub code: String,
    pub codification: String,
    pub nom: String,
    pub siret: String,
    pub emetteur: String,
}

/// La table livree, lue par sa forme.
pub fn base() -> Resultat<BaseUrssaf> {
    let v = json_en_cache(&dossier_du_code().join("urssaf-dpae-base.json"))?;
    socle::forme::lire(&v, "La table des Urssaf livrée avec le logiciel ne se lit pas : le signaler au développeur.")
}

impl BaseUrssaf {
    /// Ce code est-il celui d'un organisme connu ?
    pub fn existe(&self, code: &str) -> bool {
        let c = socle::texte::strip(code);
        self.organismes.iter().any(|o| o.code == c)
    }

    /// L'organisme du TERRITOIRE de ce departement, ou "". Une suggestion, pas une decision.
    pub fn du_departement(&self, dep: &str) -> String {
        let d = socle::texte::strip(dep).to_uppercase();
        let lu = |k: &str| self.par_departement.get(k).filter(|v| !v.is_empty()).cloned();
        lu(&d).or_else(|| if d == "2A" || d == "2B" { lu("20") } else { None }).unwrap_or_default()
    }

    /// Idem, depuis un code postal francais. L'outre-mer se lit sur TROIS chiffres (97139 -> 971), la metropole sur deux.
    pub fn du_code_postal(&self, cp: &str) -> String {
        let v: Vec<char> = cp.chars().filter(|c| c.is_ascii_digit()).collect();
        if v.len() < 2 {
            return String::new();
        }
        let n = if v[0] == '9' && (v[1] == '7' || v[1] == '8') { 3 } else { 2 };
        self.du_departement(&v.iter().take(n).collect::<String>())
    }
}
