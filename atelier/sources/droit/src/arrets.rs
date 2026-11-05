//! LES NOMENCLATURES DU BLOC « Arret de travail - S21.G00.60 » (`logiciel/donnees/arrets-base.json`) : motif de l'arret,
//! motif de la reprise, subrogation. Leur forme est declaree ici et `bloc_60` est la seule fonction qui lit le fichier ;
//! l'origine d'un arret et son maintien (`maintien::Origine::du_motif`), le bulletin (`engagement::imprevus::lignes_du_mois`),
//! le signalement et la DSN (`declarations`), les obligations et la page des imprevus (le moteur) la lisent par elle.

use crate::maintien::Origine;
use socle::num::Resultat;
use socle::fichiers::{forme_en_cache, FormeGardee};

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bloc60 {
    _quoi: String,
    _pourquoi_ici: String,
    _source: SourceDuBloc60,
    pub motifs_arret: Nomenclature,
    pub motifs_reprise: Nomenclature,
    pub subrogation: Nomenclature,
}

/// D'ou viennent les codes : le cahier technique, sa copie deposee, quand il a ete relu.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceDuBloc60 {
    #[serde(rename = "titre")]
    _titre: String,
    #[serde(rename = "copie")]
    _copie: String,
    #[serde(rename = "relu_le")]
    _relu_le: String,
    #[serde(rename = "conformite_2027")]
    _conformite_2027: String,
}

/// Une nomenclature : sa rubrique, ses codes dans l'ordre du fichier, et les notes qui disent ses drapeaux.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Nomenclature {
    _rubrique: String,
    pub codes: Vec<CodeDuBloc60>,
    #[serde(default)]
    _quoi: Option<String>,
    #[serde(default)]
    _ligne_du_bulletin: Option<String>,
    #[serde(default)]
    _regle: Option<String>,
    #[serde(default)]
    _date_accident: Option<String>,
    #[serde(default)]
    _origine: Option<String>,
    #[serde(default)]
    _temps_partiel_therapeutique: Option<String>,
    #[serde(default)]
    _annulation: Option<String>,
}

/// Un code de la norme, tel que la page le propose : son libelle, la ligne du bulletin qui nomme l'absence
/// (`rubrique`), s'il se saisit, et ce qu'il emporte (la date de l'accident, l'origine qui ouvre le maintien de la loi,
/// un temps partiel therapeutique, une valeur de reserve, l'annulation).
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct CodeDuBloc60 {
    pub code: String,
    pub libelle: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rubrique: Option<String>,
    pub saisissable: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub date_accident: bool,
    /// Ce qui a cause l'arret, quand c'est une maladie ou un accident (L1226-1) : le seul motif qui appelle le maintien.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origine: Option<Origine>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub temps_partiel_therapeutique: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reserve: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub annulation: bool,
    #[serde(default, rename = "_norme", skip_serializing)]
    _norme: Option<String>,
}

impl Nomenclature {
    /// Le code `c` de la nomenclature, s'il y est.
    pub fn code(&self, c: &str) -> Option<&CodeDuBloc60> {
        self.codes.iter().find(|x| x.code == c)
    }

    /// Ceux que la page propose (`saisissable`).
    pub fn saisissables(&self) -> Vec<CodeDuBloc60> {
        self.codes.iter().filter(|c| c.saisissable).cloned().collect()
    }
}

impl Bloc60 {
    /// Le code « oui » de la subrogation (S21.G00.60.004), que le fichier doit porter.
    pub fn subrogation_oui(&self) -> Resultat<&str> {
        self.subrogation.codes.iter().find(|c| c.libelle == "oui").map(|c| c.code.as_str()).ok_or_else(|| socle::echec::livree("arrets-base.json", "subrogation.oui"))
    }
}

/// `arrets-base.json`, lu par sa forme (le texte est lu une fois tant qu'il ne change pas).
pub fn bloc_60() -> Resultat<Bloc60> {
    static CACHE: std::sync::OnceLock<FormeGardee<Bloc60>> = std::sync::OnceLock::new();
    let b = forme_en_cache(&CACHE, "arrets-base.json", "Les codes du bloc arrêt de travail livrés avec le logiciel ne se lisent pas : le signaler au développeur.", Ok)?;
    Ok((*b).clone())
}

/// Le motif d'arret (`arrets-base.json`, `motifs_arret`), s'il existe.
pub fn motif_d_arret(code: &str) -> Resultat<Option<CodeDuBloc60>> {
    Ok(bloc_60()?.motifs_arret.codes.into_iter().find(|c| c.code == code))
}
