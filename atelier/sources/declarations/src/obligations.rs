//! LA BASE DES OBLIGATIONS (`logiciel/donnees/obligations-base.json`) : chaque obligation qu'un arret ou une fin de
//! contrat fait naitre, son delai, son point de depart et ses textes. Sa forme est declaree ici et `base` est la seule
//! fonction qui lit le fichier ; le signalement (`signalement::avertissement_hors_dsn`) et le moteur (`obligations`, qui
//! compte les obligations) la lisent par elle.

use serde::{Deserialize, Serialize};

use socle::fichiers::{dossier_du_code, json_en_cache};
use socle::num::Resultat;

/// `obligations-base.json` : chaque obligation, son delai, son point de depart et ses textes. Les notes du fichier
/// (`_quoi`, `_pourquoi_ici`, `_textes`, `jours_ouvres`) ne servent pas au calcul.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaseDesObligations {
    _quoi: String,
    _pourquoi_ici: String,
    _textes: String,
    #[serde(rename = "jours_ouvres")]
    _jours_ouvres: NoteDesJoursOuvres,
    pub obligations: LesObligations,
}

/// Ce que le fichier dit d'un jour ouvre (la regle est dans le calendrier du moteur, `jours_ouvres_apres`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NoteDesJoursOuvres {
    _regle: String,
    _inference: String,
}

/// Les obligations que le moteur sait compter, une par cle.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LesObligations {
    pub signalement_arret: DeBase,
    pub signalement_arret_rectificatif: DeBase,
    pub signalement_reprise: DeBase,
    pub signalement_fin_contrat: DeBase,
    pub declaration_accident: DeBase,
    pub declaration_maladie_professionnelle: DeBase,
    pub attestation_salaire: DeBase,
}

/// Une obligation telle que le fichier la livre : ce qu'elle est, ou elle se fait, qui la recoit, son delai (sans
/// subrogation, avec, ou unique), les motifs qui l'appellent ou n'en font pas naitre, ses suites et ses textes.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeBase {
    pub libelle: String,
    pub nait_de: String,
    pub qui: String,
    #[serde(default)]
    pub ou: Option<Ou>,
    pub destinataire: String,
    #[serde(default)]
    pub motifs: Vec<String>,
    #[serde(default)]
    pub sauf_motifs: Option<SaufMotifs>,
    #[serde(default)]
    pub delai: Option<Delai>,
    #[serde(default)]
    pub delai_sans_subrogation: Option<Delai>,
    #[serde(default)]
    pub delai_avec_subrogation: Option<Delai>,
    #[serde(default)]
    pub connaissance_presumee: Option<String>,
    #[serde(default)]
    pub exceptions: Vec<String>,
    #[serde(default)]
    pub inferences: Vec<String>,
    #[serde(default)]
    pub a_avoir: Vec<String>,
    #[serde(default)]
    _a_avoir_source: Option<String>,
    #[serde(default)]
    pub suites: Option<Suites>,
    pub textes: Vec<TexteDepose>,
}

/// Ou une obligation se fait : une phrase (un signalement, que le logiciel ecrit), ou les lieux ou la faire soi-meme
/// (hors DSN), que la page propose et que `accident-declare` et `attestation-declare` acceptent.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Ou {
    Phrase(String),
    Lieux(Vec<Lieu>),
}

/// Un lieu ou faire une obligation hors DSN : son code, ce qu'en dit l'ecran (court, puis en entier), le texte qui le
/// decrit, et pour net-entreprises ce qu'il faudrait pour y deposer un fichier.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Lieu {
    pub code: String,
    pub court: String,
    libelle: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    edi: Option<String>,
    texte: String,
}

/// Un texte depose qui fonde une obligation : sa reference, son fichier, ce qu'il dit.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TexteDepose {
    reference: String,
    fichier: String,
    dit: String,
}

/// Les motifs d'arret que la DSN ne couvre pas, pourquoi, et l'avertissement que le signalement en donne.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaufMotifs {
    pub codes: Vec<String>,
    #[serde(rename = "pourquoi")]
    _pourquoi: String,
    pub avertissement: String,
    #[serde(rename = "texte")]
    _texte: TexteDepose,
}

/// Un delai : en jours ouvres, en jours, en heures (dimanches et feries non compris), ou avec la DSN mensuelle ; et la
/// phrase qui le dit.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Delai {
    #[serde(default)]
    pub jours_ouvres: Option<i64>,
    #[serde(default)]
    pub jours: Option<i64>,
    #[serde(default)]
    pub heures: Option<i64>,
    #[serde(default, rename = "sans_dimanches")]
    _sans_dimanches: Option<bool>,
    #[serde(default, rename = "sans_feries")]
    _sans_feries: Option<bool>,
    #[serde(default, rename = "avec_la_mensuelle")]
    _avec_la_mensuelle: Option<bool>,
    #[serde(default, rename = "depart")]
    _depart: Option<String>,
    pub dit: String,
}

/// Ce qui suit une declaration d'accident.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Suites {
    #[serde(default)]
    pub reserves: Option<Reserves>,
    #[serde(default)]
    pub feuille_accident: Option<FeuilleDAccident>,
}

/// Les reserves motivees (R. 441-6) : dix jours francs apres la declaration.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reserves {
    pub libelle: String,
    pub jours_francs: i64,
    #[serde(rename = "depuis")]
    _depuis: String,
    pub texte: TexteDepose,
    pub inference: String,
}

/// La feuille d'accident a remettre au salarie (L. 441-5).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeuilleDAccident {
    pub libelle: String,
    pub texte: TexteDepose,
}

impl LesObligations {
    /// L'obligation dont le moteur dit la cle (`Obligation::cle`).
    pub fn de(&self, cle: &str) -> Resultat<&DeBase> {
        Ok(match cle {
            "signalement_arret" => &self.signalement_arret,
            "signalement_arret_rectificatif" => &self.signalement_arret_rectificatif,
            "signalement_reprise" => &self.signalement_reprise,
            "signalement_fin_contrat" => &self.signalement_fin_contrat,
            "declaration_accident" => &self.declaration_accident,
            "declaration_maladie_professionnelle" => &self.declaration_maladie_professionnelle,
            "attestation_salaire" => &self.attestation_salaire,
            autre => return Err(livree(autre)),
        })
    }
}

impl DeBase {
    /// Les motifs que la DSN ne couvre pas (`sauf_motifs.codes`), aucun sinon.
    pub fn sauf(&self) -> &[String] {
        self.sauf_motifs.as_ref().map_or(&[], |s| &s.codes)
    }

    /// Le delai `quoi` (`delai`, `delai_sans_subrogation`, `delai_avec_subrogation`), que l'obligation doit porter.
    pub fn delai_de<'a>(&self, d: &'a Option<Delai>, quoi: &str) -> Resultat<&'a Delai> {
        d.as_ref().ok_or_else(|| livree(&format!("{}.{}", self.libelle, quoi)))
    }

    /// Le point de depart presume, que l'obligation doit porter quand elle court de la connaissance.
    pub fn presume(&self) -> Resultat<String> {
        self.connaissance_presumee.clone().ok_or_else(|| livree("connaissance_presumee"))
    }

    /// Les lieux ou la faire soi-meme (aucun pour un signalement).
    pub fn lieux(&self) -> &[Lieu] {
        match &self.ou {
            Some(Ou::Lieux(l)) => l,
            _ => &[],
        }
    }
}

/// Une valeur que la base doit porter, et qu'elle ne porte pas.
pub fn livree(cle: &str) -> String {
    socle::echec::livree("obligations-base.json", cle)
}

/// `obligations-base.json`, lu par sa forme (le texte est lu une fois tant qu'il ne change pas).
pub fn base() -> Resultat<BaseDesObligations> {
    let v = json_en_cache(&dossier_du_code().join("obligations-base.json"))?;
    socle::forme::lire(&v, "Les obligations livrées avec le logiciel ne se lisent pas : le signaler au développeur.")
}
