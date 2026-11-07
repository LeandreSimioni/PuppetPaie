//! La forme de ce qu'on donne à la fabrique (FORMAT.md), déclarée une fois : la description d'un
//! document (`Description`), le remplissage d'un formulaire (`Remplissage`), les champs qu'on lui
//! ajoute (`AjoutDeChamps`). Le moteur les construit en Rust ; la ligne de commande les lit d'un texte
//! JSON, une fois, strictement (`lire`) : aucune clé inconnue.
//!
//! Un champ `Option` absent est vide : il ne dit rien. Ce que la fabrique met alors à sa place est
//! nommé sur le champ — la valeur de la police, du bloc, de la colonne, ou une des constantes
//! ci-dessous — et ne se décide qu'à un seul endroit, la validation (`description.rs`).

use serde::Deserialize;
use std::collections::BTreeMap;

use crate::description::{francais, Refus};

/// Une page sans largeur dite : A4 (en points).
pub const PAGE_LARGEUR: f32 = 595.28;
/// Une page sans hauteur dite : A4 (en points).
pub const PAGE_HAUTEUR: f32 = 841.89;
/// Une marge de page non dite (en points, 2 cm).
pub const MARGE: f32 = 56.7;
/// Le corps d'une police non dite.
pub const TAILLE: f32 = 10.0;
/// L'interligne d'une police non dite.
pub const INTERLIGNE: f32 = 1.25;
/// La couleur d'une police, d'un filet ou d'un trait non dite.
pub const COULEUR: &str = "#000000";
/// L'épaisseur d'un filet ou d'un trait non dite (en points).
pub const EPAISSEUR: f32 = 0.5;
/// La distance d'un repère au bord de la page non dite (en points, jusqu'à la ligne de base).
pub const DISTANCE_REPERE: f32 = 20.0;

/// Lit `texte` en `T`, strictement ; un refus est une ligne de français.
pub fn lire<T: serde::de::DeserializeOwned>(texte: &str, quoi: &str) -> Result<T, Refus> {
    serde_json::from_str(texte).map_err(|e| Refus(format!("{} illisible : {}", quoi, francais(&e))))
}

// ---------------------------------------------------------------------------
// La description d'un document.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Description {
    /// 1, 2 ou 3 (FORMAT.md) ; un `modele` demande 2 au moins.
    pub format: i64,
    /// Sans elle : celle du style, sinon A4 aux marges `MARGE`.
    pub page: Option<Page>,
    /// Sans elle : celle du style, sinon `TAILLE`, `INTERLIGNE`, `COULEUR`.
    pub police: Option<Police>,
    /// La mise en page des documents « modèle ».
    pub style: Option<Style>,
    pub logo: Option<Logo>,
    /// Sans lui : celui du premier modèle, sinon aucun.
    pub titre: Option<String>,
    /// Chacun commence sur une nouvelle page.
    pub documents: Vec<Document>,
    pub entete_page: Option<Repere>,
    pub pied_page: Option<Repere>,
}

impl Description {
    /// La porte du texte : une description JSON (FORMAT.md) lue une fois, strictement.
    pub fn lire(texte: &str) -> Result<Description, Refus> {
        lire(texte, "description")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Style {
    Contrat,
    Avenant,
    Attestation,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Page {
    /// Sans elle : `PAGE_LARGEUR`.
    pub largeur: Option<f32>,
    /// Sans elle : `PAGE_HAUTEUR`.
    pub hauteur: Option<f32>,
    /// Sans elles : `MARGE` de chaque côté.
    pub marges: Option<Marges>,
}

/// Chaque marge non dite : `MARGE`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Marges {
    pub haut: Option<f32>,
    pub droite: Option<f32>,
    pub bas: Option<f32>,
    pub gauche: Option<f32>,
}

/// Chaque valeur non dite : `TAILLE`, `INTERLIGNE`, `COULEUR`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Police {
    pub taille: Option<f32>,
    pub interligne: Option<f32>,
    pub couleur: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Logo {
    pub chemin: String,
    pub largeur: f32,
    /// Sans elle : 0.
    pub marge_haut: Option<f32>,
    /// Sans elle : 0.
    pub marge_droite: Option<f32>,
}

/// Un en-tête ou un pied de page répété sur chaque page (FORMAT.md, « En-tête et pied de page »).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Repere {
    /// Chaque texte non dit : rien à cette place.
    pub gauche: Option<String>,
    pub centre: Option<String>,
    pub droite: Option<String>,
    /// Sans elle : celle de la police.
    pub taille: Option<f32>,
    /// Sans elle : celle de la police.
    pub couleur: Option<String>,
    /// Sans lui : maigre.
    pub gras: Option<bool>,
    /// Sans elle : `DISTANCE_REPERE`.
    pub distance: Option<f32>,
    /// Sans lui : le repère est aussi sur la première page.
    pub premiere_page: Option<bool>,
    pub image: Option<ImageRepere>,
}

/// L'image qui suit le texte de droite d'un repère.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageRepere {
    pub chemin: String,
    pub largeur: f32,
    /// Sans elle : 0 (le bas de l'image sur la ligne de base).
    pub descente: Option<f32>,
}

/// Un document : ses blocs, ou un modèle que la fabrique met en page selon le style. En JSON,
/// `{"blocs": [...]}` ou `{"modele": {...}}`.
#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "DocumentDit")]
pub enum Document {
    Blocs(Vec<Bloc>),
    Modele(Modele),
}

/// Un document tel que le texte le dit : l'un des deux, que `Document` départage.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentDit {
    blocs: Option<Vec<Bloc>>,
    modele: Option<Modele>,
}

impl TryFrom<DocumentDit> for Document {
    type Error = String;
    fn try_from(d: DocumentDit) -> Result<Document, String> {
        match (d.blocs, d.modele) {
            (Some(b), None) => Ok(Document::Blocs(b)),
            (None, Some(m)) => Ok(Document::Modele(m)),
            (Some(_), Some(_)) => Err("un document : \"blocs\" et \"modele\" s'excluent".to_string()),
            (None, None) => Err("un document dit ses \"blocs\" ou son \"modele\"".to_string()),
        }
    }
}

/// Un document dont le texte est dans un fichier `.modele` (`chemin`) ou donné tel quel (`texte`) :
/// l'un des deux.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Modele {
    pub chemin: Option<String>,
    pub texte: Option<String>,
    pub valeurs: BTreeMap<String, String>,
    /// Le contrat que celui-ci annule et remplace (style « contrat »).
    pub remplace: Option<Remplace>,
    /// L'image de la signature de l'employeur dans sa case (styles « contrat » et « avenant »).
    pub signature: Option<String>,
    /// L'image de la signature de l'employeur en fin de document (style « attestation »).
    pub signature_employeur: Option<String>,
    /// Les articles que l'avenant modifie (style « avenant »).
    pub modifications: Option<Vec<Modification>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Remplace {
    pub numero: String,
    pub date: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Modification {
    /// Le numéro de l'article cité ; aucun pour une modification dite en clair (`phrase`).
    pub numero: Option<i64>,
    pub article: String,
    pub avant: Vec<String>,
    pub apres: Vec<String>,
    /// La modification dite en clair, sans article cité : le paragraphe `{{modification.phrase}}` du modèle la porte.
    pub phrase: Option<String>,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Alignement {
    Gauche,
    Justifie,
    Centre,
    Droite,
}

/// Les blocs d'un document, dans l'ordre (FORMAT.md, « Les blocs »).
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Bloc {
    Paragraphe(Paragraphe),
    Espace(Espace),
    Colonnes(Colonnes),
    Image(Image),
    Tableau(Tableau),
    TexteFixe(TexteFixe),
    Trait(Trait),
    ImageFixe(ImageFixe),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Morceau {
    pub texte: String,
    /// Sans lui : maigre (dans un tableau, la graisse de la cellule ou de la ligne).
    pub gras: Option<bool>,
    /// Sans lui : droit.
    pub italique: Option<bool>,
    /// Sans elle : celle du bloc (ou de la cellule, de la ligne).
    pub couleur: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Paragraphe {
    pub morceaux: Vec<Morceau>,
    /// Sans elle : celle de la police.
    pub taille: Option<f32>,
    /// Sans elle : celle de la police.
    pub couleur: Option<String>,
    /// Sans lui : à gauche.
    pub alignement: Option<Alignement>,
    /// Sans lui : le texte tel quel.
    pub majuscules: Option<bool>,
    /// Sans lui : 0.
    pub espace_avant: Option<f32>,
    /// Sans lui : 0.
    pub espace_apres: Option<f32>,
    /// Marge à gauche du paragraphe, en points (une citation, un alinéa). Sans lui : 0.
    pub retrait: Option<f32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Espace {
    pub hauteur: f32,
}

/// Un trait : sans couleur, `COULEUR` ; sans épaisseur, `EPAISSEUR`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filet {
    pub couleur: Option<String>,
    pub epaisseur: Option<f32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Colonnes {
    pub filet_haut: Option<Filet>,
    /// Sans lui : 0.
    pub espace_avant: Option<f32>,
    /// Sans elle : 0.
    pub marge_interne_haut: Option<f32>,
    pub colonnes: Vec<Colonne>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Colonne {
    /// La fraction de la largeur utile ; les colonnes font 1 à elles toutes.
    pub largeur: f32,
    /// Sans eux : une colonne vide.
    pub blocs: Option<Vec<Bloc>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Image {
    pub chemin: String,
    pub largeur: f32,
    /// Posée à l'endroit courant sans occuper de hauteur. Sans lui : elle occupe sa hauteur.
    pub flottante: Option<bool>,
}

/// La police d'un `texte_fixe` : Liberation Sans (Helvetica), Serif (Times) ou Mono (Courier).
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PoliceFixe {
    Helvetica,
    Times,
    Courier,
}

/// Un texte posé à une position absolue de la page.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TexteFixe {
    pub texte: String,
    pub x: f32,
    /// L'ordonnée de la ligne de base, depuis le haut de la page.
    pub y: f32,
    /// Sans elle : celle de la police.
    pub taille: Option<f32>,
    /// Sans elle : Helvetica.
    pub police: Option<PoliceFixe>,
    /// Sans lui : maigre.
    pub gras: Option<bool>,
    /// Sans elle : celle de la police.
    pub couleur: Option<String>,
    /// Par rapport à `x` ; sans lui : à gauche.
    pub alignement: Option<Alignement>,
    /// La largeur que le texte ne doit pas dépasser : sinon son corps est réduit (FORMAT.md).
    pub largeur_max: Option<f32>,
}

/// Un trait : sans épaisseur, `EPAISSEUR` ; sans couleur, `COULEUR`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trait {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub epaisseur: Option<f32>,
    pub couleur: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageFixe {
    pub chemin: String,
    pub x: f32,
    pub y: f32,
    pub largeur: f32,
    pub hauteur: f32,
    /// Sans lui : posée au coin (x, y).
    pub centree: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColonneTableau {
    /// La fraction de la largeur utile ; les colonnes font 1 à elles toutes.
    pub largeur: f32,
    /// Sans lui : à gauche.
    pub alignement: Option<Alignement>,
}

/// L'espace entre le bord d'une cellule et son texte ; chaque côté non dit : 0.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MargeCellule {
    pub haut: Option<f32>,
    pub droite: Option<f32>,
    pub bas: Option<f32>,
    pub gauche: Option<f32>,
}

/// Une cellule : son texte seul (en JSON, une chaîne), ou une cellule décrite (un objet).
#[derive(Debug, Clone)]
pub enum Cellule {
    Texte(String),
    Decrite(CelluleDecrite),
}

/// Lue à la main plutôt que sans étiquette : l'objet garde le refus de sa clé inconnue, et le reste dit
/// « une chaîne ou un objet ».
impl<'de> Deserialize<'de> for Cellule {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Cellule, D::Error> {
        struct Lecteur;
        impl<'de> serde::de::Visitor<'de> for Lecteur {
            type Value = Cellule;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("une chaîne ou un objet (une cellule)")
            }
            fn visit_str<E: serde::de::Error>(self, s: &str) -> Result<Cellule, E> {
                Ok(Cellule::Texte(s.to_string()))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, m: A) -> Result<Cellule, A::Error> {
                CelluleDecrite::deserialize(serde::de::value::MapAccessDeserializer::new(m)).map(Cellule::Decrite)
            }
        }
        d.deserialize_any(Lecteur)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CelluleDecrite {
    /// `texte` et `morceaux` s'excluent ; sans l'un ni l'autre, la cellule est vide.
    pub texte: Option<String>,
    pub morceaux: Option<Vec<Morceau>>,
    /// Sans lui : la graisse de la ligne.
    pub gras: Option<bool>,
    /// Sans elle : celle de la ligne.
    pub couleur: Option<String>,
    /// Sans lui : celui de la colonne où elle commence.
    pub alignement: Option<Alignement>,
    /// Le nombre de colonnes qu'elle couvre ; sans lui : une.
    pub etendue: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ligne {
    pub cellules: Vec<Cellule>,
    /// Sans lui : maigre.
    pub gras: Option<bool>,
    pub fond: Option<String>,
    /// Sans elle : celle du tableau.
    pub couleur: Option<String>,
    /// Sans elle : celle du tableau.
    pub taille: Option<f32>,
    pub filet_haut: Option<Filet>,
    pub filet_bas: Option<Filet>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tableau {
    pub colonnes: Vec<ColonneTableau>,
    /// Répétées en haut de chaque page ; sans elles : aucune.
    pub entete: Option<Vec<Ligne>>,
    /// Sans elles : aucune.
    pub lignes: Option<Vec<Ligne>>,
    /// Sans elle : celle de la police.
    pub taille: Option<f32>,
    /// Sans elle : celle de la police.
    pub couleur: Option<String>,
    /// Sans lui : celui de la police.
    pub interligne: Option<f32>,
    /// Sans elle : 0 de chaque côté.
    pub marge_cellule: Option<MargeCellule>,
    pub filet_lignes: Option<Filet>,
    pub filets_cellules: Option<Filet>,
    /// Sans lui : 0.
    pub espace_avant: Option<f32>,
    /// Sans lui : 0.
    pub espace_apres: Option<f32>,
}

// ---------------------------------------------------------------------------
// Le remplissage d'un formulaire (FORMAT.md, « Remplir un formulaire »).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Remplissage {
    /// `FORMAT`.
    pub format: i64,
    /// Nom du champ → texte à y écrire ; sans eux : aucun.
    pub champs: Option<BTreeMap<String, String>>,
    /// Sans elles : aucune.
    pub images: Option<Vec<ImagePosee>>,
}

impl Remplissage {
    /// La porte du texte (`--remplir`).
    pub fn lire(texte: &str) -> Result<Remplissage, Refus> {
        lire(texte, "description")
    }
}

/// Une image posée dans un rectangle d'une page : le plus grand rectangle à ses proportions qui tient, centré.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImagePosee {
    /// À partir de 1.
    pub page: usize,
    /// [x0, y0, x1, y1], en points, origine en haut à gauche (comme `--champs`).
    pub rect: [f64; 4],
    /// Sans elle : 0.
    pub marge: Option<f64>,
    /// `fichier` (PNG, JPEG ou SVG) et `svg` (le texte du SVG) s'excluent.
    pub fichier: Option<String>,
    pub svg: Option<String>,
}

// ---------------------------------------------------------------------------
// Les champs ajoutés à un PDF (`formulaire::ajouter_champs`).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AjoutDeChamps {
    /// `FORMAT`.
    pub format: i64,
    pub champs: Vec<ChampAjoute>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChampAjoute {
    /// Non vide, sans point.
    pub nom: String,
    /// À partir de 1.
    pub page: usize,
    /// [x0, y0, x1, y1], origine en haut à gauche.
    pub rect: [f64; 4],
    pub valeur: Option<String>,
    /// Sans lui : on peut y écrire.
    pub lecture_seule: Option<bool>,
    /// Le corps ; sans lui : celui qui tient.
    pub taille: Option<f64>,
    /// Gauche, centre ou droite ; sans lui : à gauche.
    pub alignement: Option<Alignement>,
}
