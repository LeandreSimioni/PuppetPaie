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
    pub quoi: String,
    pub note: String,
    pub motifs_arret: Nomenclature,
    pub motifs_reprise: Nomenclature,
    pub subrogation: Nomenclature,
}

/// Une nomenclature : ses codes dans l'ordre du fichier, et ses notes (rubrique, drapeaux).
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Nomenclature {
    pub codes: Vec<CodeDuBloc60>,
    pub quoi: Option<String>,
    pub note: Option<String>,
}

/// Un code de la norme, tel que la page le propose : son libelle, la ligne du bulletin qui nomme l'absence
/// (`rubrique`), s'il se saisit, et ce qu'il emporte (la date de l'accident, l'origine qui ouvre le maintien de la loi,
/// un temps partiel therapeutique, une valeur de reserve, l'annulation).
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct CodeDuBloc60 {
    pub code: String,
    pub libelle: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rubrique: Option<String>,
    pub saisissable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_accident: Option<bool>,
    /// Ce qui a cause l'arret, quand c'est une maladie ou un accident (L1226-1) : le seul motif qui appelle le maintien.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origine: Option<Origine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temps_partiel_therapeutique: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reserve: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annulation: Option<bool>,
    #[serde(skip_serializing)]
    pub note: Option<String>,
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
