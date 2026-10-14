//! La description d'un document : lecture du JSON (FORMAT.md) et validation.
//!
//! Tout champ inconnu est un refus (`deny_unknown_fields`) ; toute erreur sort en une ligne de
//! français, avec le numéro du bloc quand on le connaît.

use serde::de::IgnoredAny;
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

use crate::modele;
use crate::style_contrat;

/// Un refus : une ligne de français, prête à être affichée telle quelle.
#[derive(Debug, Clone, PartialEq)]
pub struct Refus(pub String);

impl std::fmt::Display for Refus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Refus {}

pub fn refus<T>(message: impl Into<String>) -> Result<T, Refus> {
    Err(Refus(message.into()))
}

/// Le numéro du format que cette fabrique parle (`--version`). Le 2 ajoute les documents « modèle »
/// (FORMAT.md) ; les descriptions du format 1 sont toujours lues telles quelles. Le 3 ajoute les
/// modes de lecture et de retouche de PDF existants (`--pages`, `--texte`, `--marquer`,
/// `--verrouiller`) et le champ facultatif `largeur_max` de `texte_fixe` ; une description du
/// format 2 reste lue telle quelle.
pub const FORMAT: i64 = 3;

/// Le corps en dessous duquel `largeur_max` ne réduit pas un texte (`corps_qui_tient`, 6 pt : sous
/// cette taille la case est illisible, et c'est le signe que la donnée ne va pas).
pub const CORPS_PLANCHER: f32 = 6.0;

// ---------------------------------------------------------------------------
// Ce qui est lu tel quel dans le JSON.
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DescriptionBrute {
    #[allow(dead_code)]
    format: IgnoredAny,
    page: Option<PageBrute>,
    police: Option<PoliceBrute>,
    style: Option<String>,
    logo: Option<LogoBrut>,
    titre: Option<String>,
    documents: Option<Vec<DocumentBrut>>,
    entete_page: Option<RepereBrut>,
    pied_page: Option<RepereBrut>,
}

/// Un en-tête ou un pied de page répété sur chaque page (FORMAT.md, « En-tête et pied de page »).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RepereBrut {
    #[serde(default)]
    gauche: String,
    #[serde(default)]
    centre: String,
    #[serde(default)]
    droite: String,
    taille: Option<f32>,
    couleur: Option<String>,
    #[serde(default)]
    gras: bool,
    #[serde(default = "distance_defaut")]
    distance: f32,
    #[serde(default = "vrai")]
    premiere_page: bool,
}

fn distance_defaut() -> f32 {
    20.0
}
fn vrai() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PageBrute {
    #[serde(default = "a4_largeur")]
    largeur: f32,
    #[serde(default = "a4_hauteur")]
    hauteur: f32,
    #[serde(default)]
    marges: MargesBrutes,
}

fn a4_largeur() -> f32 {
    595.28
}
fn a4_hauteur() -> f32 {
    841.89
}

impl Default for PageBrute {
    fn default() -> Self {
        PageBrute {
            largeur: a4_largeur(),
            hauteur: a4_hauteur(),
            marges: MargesBrutes::default(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MargesBrutes {
    #[serde(default = "marge_defaut")]
    haut: f32,
    #[serde(default = "marge_defaut")]
    droite: f32,
    #[serde(default = "marge_defaut")]
    bas: f32,
    #[serde(default = "marge_defaut")]
    gauche: f32,
}

fn marge_defaut() -> f32 {
    56.7
}

impl Default for MargesBrutes {
    fn default() -> Self {
        MargesBrutes {
            haut: marge_defaut(),
            droite: marge_defaut(),
            bas: marge_defaut(),
            gauche: marge_defaut(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PoliceBrute {
    #[serde(default = "taille_defaut")]
    taille: f32,
    #[serde(default = "interligne_defaut")]
    interligne: f32,
    #[serde(default = "couleur_defaut")]
    couleur: String,
}

fn taille_defaut() -> f32 {
    10.0
}
fn interligne_defaut() -> f32 {
    1.25
}
fn couleur_defaut() -> String {
    "#000000".to_string()
}

impl Default for PoliceBrute {
    fn default() -> Self {
        PoliceBrute {
            taille: taille_defaut(),
            interligne: interligne_defaut(),
            couleur: couleur_defaut(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LogoBrut {
    chemin: String,
    largeur: f32,
    #[serde(default)]
    marge_haut: f32,
    #[serde(default)]
    marge_droite: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentBrut {
    #[serde(default)]
    blocs: Vec<Value>,
    modele: Option<ModeleBrut>,
}

/// Un document dont le texte est dans un fichier `.modele` que la fabrique lit elle-même.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ModeleBrut {
    /// Le fichier `.modele`, ou bien (exclusifs) son texte même : `texte`.
    chemin: Option<String>,
    texte: Option<String>,
    valeurs: BTreeMap<String, String>,
    remplace: Option<RemplaceBrut>,
    signature: Option<String>,
    /// L'image de la signature de l'employeur en fin de document (style « attestation »).
    signature_employeur: Option<String>,
    /// Ce que l'artiste a signé d'un clic, posé dans sa case (styles contrat et avenant).
    mention_artiste: Option<String>,
    /// Les articles que l'avenant modifie (style « avenant »).
    #[serde(default)]
    modifications: Vec<ModificationBrute>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ModificationBrute {
    numero: i64,
    article: String,
    avant: Vec<String>,
    apres: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RemplaceBrut {
    numero: String,
    date: String,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Alignement {
    Gauche,
    Justifie,
    Centre,
    Droite,
}

impl Default for Alignement {
    fn default() -> Self {
        Alignement::Gauche
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MorceauBrut {
    texte: String,
    #[serde(default)]
    gras: bool,
    #[serde(default)]
    italique: bool,
    /// Couleur de ce morceau seul ; absente : celle du bloc (ou de la cellule, de la ligne).
    couleur: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ParagrapheBrut {
    morceaux: Vec<MorceauBrut>,
    taille: Option<f32>,
    couleur: Option<String>,
    #[serde(default)]
    alignement: Alignement,
    #[serde(default)]
    majuscules: bool,
    #[serde(default)]
    espace_avant: f32,
    #[serde(default)]
    espace_apres: f32,
    /// Marge à gauche du paragraphe, en points (une citation, un alinéa).
    #[serde(default)]
    retrait: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EspaceBrut {
    hauteur: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FiletBrut {
    #[serde(default = "couleur_defaut")]
    couleur: String,
    #[serde(default = "epaisseur_defaut")]
    epaisseur: f32,
}

fn epaisseur_defaut() -> f32 {
    0.5
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ColonneBrute {
    largeur: f32,
    #[serde(default)]
    blocs: Vec<Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ColonnesBrut {
    filet_haut: Option<FiletBrut>,
    #[serde(default)]
    espace_avant: f32,
    #[serde(default)]
    marge_interne_haut: f32,
    colonnes: Vec<ColonneBrute>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImageBrute {
    chemin: String,
    largeur: f32,
    #[serde(default)]
    flottante: bool,
}

/// Un texte posé à une position absolue de la page (FORMAT.md, `texte_fixe`).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TexteFixeBrut {
    texte: String,
    x: f32,
    /// L'ordonnée de la ligne de base, depuis le haut de la page.
    y: f32,
    taille: Option<f32>,
    police: Option<String>,
    #[serde(default)]
    gras: bool,
    couleur: Option<String>,
    #[serde(default)]
    alignement: Alignement,
    /// La largeur que le texte ne doit pas dépasser : sinon son corps est réduit (FORMAT.md).
    largeur_max: Option<f32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TraitBrut {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    #[serde(default = "epaisseur_defaut")]
    epaisseur: f32,
    #[serde(default = "couleur_defaut")]
    couleur: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImageFixeBrute {
    chemin: String,
    x: f32,
    y: f32,
    largeur: f32,
    hauteur: f32,
    #[serde(default)]
    centree: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ColonneTableauBrute {
    largeur: f32,
    #[serde(default)]
    alignement: Alignement,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct MargeCelluleBrute {
    #[serde(default)]
    haut: f32,
    #[serde(default)]
    droite: f32,
    #[serde(default)]
    bas: f32,
    #[serde(default)]
    gauche: f32,
}

/// Une cellule : une simple chaîne (le texte), ou un objet. Lue à la main, pour que le refus dise
/// « ligne 3, cellule 2 » plutôt que le « no variant matched » d'un enum sans étiquette.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CelluleBrute {
    texte: Option<String>,
    morceaux: Option<Vec<MorceauBrut>>,
    #[serde(default)]
    gras: bool,
    couleur: Option<String>,
    alignement: Option<Alignement>,
    etendue: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LigneBrute {
    cellules: Vec<Value>,
    #[serde(default)]
    gras: bool,
    fond: Option<String>,
    couleur: Option<String>,
    taille: Option<f32>,
    filet_haut: Option<FiletBrut>,
    filet_bas: Option<FiletBrut>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TableauBrut {
    colonnes: Vec<ColonneTableauBrute>,
    #[serde(default)]
    entete: Vec<LigneBrute>,
    #[serde(default)]
    lignes: Vec<LigneBrute>,
    taille: Option<f32>,
    couleur: Option<String>,
    interligne: Option<f32>,
    #[serde(default)]
    marge_cellule: MargeCelluleBrute,
    filet_lignes: Option<FiletBrut>,
    filets_cellules: Option<FiletBrut>,
    #[serde(default)]
    espace_avant: f32,
    #[serde(default)]
    espace_apres: f32,
}

// ---------------------------------------------------------------------------
// Ce qui est validé.
// ---------------------------------------------------------------------------

pub type Rvb = (u8, u8, u8);

#[derive(Debug, Clone)]
pub struct Page {
    pub largeur: f32,
    pub hauteur: f32,
    pub haut: f32,
    pub droite: f32,
    pub bas: f32,
    pub gauche: f32,
}

impl Page {
    pub fn largeur_utile(&self) -> f32 {
        self.largeur - self.gauche - self.droite
    }
    pub fn hauteur_utile(&self) -> f32 {
        self.hauteur - self.haut - self.bas
    }
}

#[derive(Debug, Clone)]
pub struct Police {
    pub taille: f32,
    pub interligne: f32,
    pub couleur: Rvb,
}

#[derive(Debug, Clone)]
pub struct Logo {
    pub chemin: String,
    pub largeur: f32,
    pub marge_haut: f32,
    pub marge_droite: f32,
}

/// Un en-tête ou un pied de page : trois textes d'une ligne (`{page}` et `{pages}` y sont remplacés
/// par le numéro de la page et par le nombre de pages), à `distance` points du bord de la page (la
/// ligne de base).
#[derive(Debug, Clone)]
pub struct Repere {
    pub gauche: String,
    pub centre: String,
    pub droite: String,
    pub taille: f32,
    pub couleur: Rvb,
    pub gras: bool,
    pub distance: f32,
    pub premiere_page: bool,
}

impl Repere {
    /// Le total de pages ne se connaît qu'après une première mise en page (FORMAT.md).
    pub fn emploie_total(&self) -> bool {
        [&self.gauche, &self.centre, &self.droite].iter().any(|t| t.contains("{pages}"))
    }
}

#[derive(Debug, Clone)]
pub struct Morceau {
    pub texte: String,
    pub gras: bool,
    pub italique: bool,
    /// None : la couleur du bloc.
    pub couleur: Option<Rvb>,
}

/// Un trait : couleur et épaisseur.
pub type Filet = (Rvb, f32);

#[derive(Debug, Clone)]
pub struct CelluleTableau {
    /// Couleur et graisse déjà résolues (morceau, cellule, ligne, tableau).
    pub morceaux: Vec<Morceau>,
    pub etendue: usize,
    pub alignement: Alignement,
}

#[derive(Debug, Clone)]
pub struct LigneTableau {
    pub cellules: Vec<CelluleTableau>,
    pub fond: Option<Rvb>,
    pub taille: f32,
    pub filet_haut: Option<Filet>,
    pub filet_bas: Option<Filet>,
}

/// La police d'un `texte_fixe` : Liberation Sans (Helvetica), Serif (Times) ou Mono (Courier),
/// embarquées en sous-ensemble.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PoliceFixe {
    Helvetica,
    Times,
    Courier,
}

#[derive(Debug, Clone)]
pub enum Bloc {
    Paragraphe {
        morceaux: Vec<Morceau>,
        taille: f32,
        couleur: Rvb,
        alignement: Alignement,
        espace_avant: f32,
        espace_apres: f32,
        /// Marge à gauche, en points : le texte se met en page dans une largeur d'autant réduite.
        retrait: f32,
    },
    Espace(f32),
    /// Une case encadrée avec quelques lignes de texte, posée à l'endroit courant sans occuper de
    /// hauteur (comme une image flottante) : la mention de signature de l'artiste.
    Mention {
        lignes: Vec<String>,
        taille: f32,
        interligne: f32,
        couleur: Rvb,
        filet: Rvb,
        /// Hauteur de la case ; sa largeur est celle de la colonne où elle se trouve.
        hauteur: f32,
        /// Espace entre le filet et le texte : gauche/droite, haut.
        marge: (f32, f32),
    },
    Colonnes {
        filet: Option<(Rvb, f32)>,
        espace_avant: f32,
        marge_interne_haut: f32,
        colonnes: Vec<(f32, Vec<BlocEtiquete>)>,
    },
    Image {
        chemin: String,
        largeur: f32,
        /// Posée à l'endroit courant sans occuper de hauteur (FORMAT.md).
        flottante: bool,
    },
    /// Position absolue sur la page, en points depuis son coin haut-gauche : ne coule pas.
    TexteFixe {
        texte: String,
        x: f32,
        y: f32,
        taille: f32,
        police: PoliceFixe,
        gras: bool,
        couleur: Rvb,
        /// Gauche, Droite ou Centre, par rapport à `x`.
        alignement: Alignement,
        /// Largeur à ne pas dépasser, en points : au-delà, le corps est réduit pour que le texte
        /// tienne, sans descendre sous `CORPS_PLANCHER` (comme `corps_qui_tient` de
        /// `moteur/conges_spectacles.py`).
        largeur_max: Option<f32>,
    },
    Trait {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        epaisseur: f32,
        couleur: Rvb,
    },
    ImageFixe {
        chemin: String,
        x: f32,
        y: f32,
        largeur: f32,
        hauteur: f32,
        centree: bool,
    },
    Tableau {
        /// Fraction de la largeur utile et alignement de chaque colonne.
        colonnes: Vec<(f32, Alignement)>,
        interligne: f32,
        /// haut, droite, bas, gauche : l'espace entre le bord d'une cellule et son texte.
        marge_cellule: [f32; 4],
        filet_lignes: Option<Filet>,
        filets_cellules: Option<Filet>,
        entete: Vec<LigneTableau>,
        lignes: Vec<LigneTableau>,
        espace_avant: f32,
        espace_apres: f32,
    },
}

impl Bloc {
    /// Un bloc à position absolue sur la page (il ne coule pas, n'occupe aucune hauteur).
    pub fn est_fixe(&self) -> bool {
        matches!(self, Bloc::TexteFixe { .. } | Bloc::Trait { .. } | Bloc::ImageFixe { .. })
    }
}

/// Un bloc et son étiquette d'erreur (« bloc 3, colonne 2, bloc 1 »).
#[derive(Debug, Clone)]
pub struct BlocEtiquete {
    pub etiquette: String,
    pub bloc: Bloc,
}

#[derive(Debug, Clone)]
pub struct Description {
    pub page: Page,
    pub police: Police,
    pub logo: Option<Logo>,
    pub titre: String,
    pub documents: Vec<Vec<BlocEtiquete>>,
    pub entete_page: Option<Repere>,
    pub pied_page: Option<Repere>,
}

// ---------------------------------------------------------------------------
// La lecture.
// ---------------------------------------------------------------------------

/// Met une erreur de serde en français.
pub fn francais(e: &serde_json::Error) -> String {
    let brut = e.to_string();
    // serde_json ajoute « at line L column C » : on le retire du message et on le range à part.
    let (msg, lieu) = match brut.rfind(" at line ") {
        Some(i) => (brut[..i].to_string(), Some((e.line(), e.column()))),
        None => (brut.clone(), None),
    };
    let liste = |reste: &str| -> String {
        reste
            .split('`')
            .enumerate()
            .filter(|(i, _)| i % 2 == 1)
            .map(|(_, s)| s.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let nom = |reste: &str| -> String { reste.split('`').next().unwrap_or("").to_string() };
    let corps = if let Some(reste) = msg.strip_prefix("unknown field `") {
        let n = nom(reste);
        match reste.find("expected ") {
            Some(i) if !reste[i..].contains("there are no") => format!(
                "champ inconnu \"{}\" (champs connus : {})",
                n,
                liste(&reste[i..])
            ),
            _ => format!("champ inconnu \"{}\"", n),
        }
    } else if let Some(reste) = msg.strip_prefix("missing field `") {
        format!("champ absent \"{}\"", nom(reste))
    } else if let Some(reste) = msg.strip_prefix("unknown variant `") {
        let n = nom(reste);
        match reste.find("expected ") {
            Some(i) => format!("valeur inconnue \"{}\" (attendu : {})", n, liste(&reste[i..])),
            None => format!("valeur inconnue \"{}\"", n),
        }
    } else if let Some(reste) = msg.strip_prefix("invalid type: ") {
        match reste.split_once(", expected ") {
            Some((recu, attendu)) => format!(
                "valeur de mauvais type : {} reçu, {} attendu",
                traduit_type(recu),
                traduit_type(attendu)
            ),
            None => format!("valeur de mauvais type : {}", reste),
        }
    } else if let Some(reste) = msg.strip_prefix("invalid value: ") {
        format!("valeur invalide : {}", reste)
    } else if msg.starts_with("EOF while parsing") {
        "le JSON s'arrête trop tôt".to_string()
    } else if e.is_syntax() {
        "JSON mal formé".to_string()
    } else {
        msg
    };
    match lieu {
        Some((l, c)) if l > 0 => format!("{} (ligne {}, colonne {})", corps, l, c),
        _ => corps,
    }
}

fn traduit_type(t: &str) -> String {
    let t = t
        .replace("floating point", "nombre décimal")
        .replace("integer", "entier")
        .replace("string", "chaîne")
        .replace("boolean", "booléen")
        .replace("sequence", "liste")
        .replace("map", "objet")
        .replace("null", "null")
        .replace("unit value", "null")
        .replace("a nombre décimal", "un nombre décimal")
        .replace("f32", "un nombre")
        .replace("a chaîne", "une chaîne")
        .replace("a liste", "une liste")
        .replace("a booléen", "un booléen")
        .replace("a struct", "un objet")
        .replace("struct", "un objet");
    t
}

/// Lit « #RRGGBB » (majuscules ou minuscules).
pub fn couleur(texte: &str, ou: &str) -> Result<Rvb, Refus> {
    let bon = texte.len() == 7
        && texte.starts_with('#')
        && texte[1..].chars().all(|c| c.is_ascii_hexdigit());
    if !bon {
        return refus(format!(
            "{} : couleur \"{}\" invalide (attendu : \"#RRGGBB\")",
            ou, texte
        ));
    }
    let v = |i: usize| u8::from_str_radix(&texte[i..i + 2], 16).unwrap_or(0);
    Ok((v(1), v(3), v(5)))
}

fn fini(valeur: f32, nom: &str, ou: &str) -> Result<f32, Refus> {
    if valeur.is_finite() {
        Ok(valeur)
    } else {
        refus(format!("{} : {} n'est pas un nombre fini", ou, nom))
    }
}

fn positif_ou_nul(valeur: f32, nom: &str, ou: &str) -> Result<f32, Refus> {
    let v = fini(valeur, nom, ou)?;
    if v < 0.0 {
        refus(format!("{} : {} ne peut pas être négatif ({})", ou, nom, v))
    } else {
        Ok(v)
    }
}

fn strictement_positif(valeur: f32, nom: &str, ou: &str) -> Result<f32, Refus> {
    let v = fini(valeur, nom, ou)?;
    if v <= 0.0 {
        refus(format!("{} : {} doit être positif ({})", ou, nom, v))
    } else {
        Ok(v)
    }
}

fn lire<T: serde::de::DeserializeOwned>(valeur: &Value, ou: &str) -> Result<T, Refus> {
    serde_json::from_value::<T>(valeur.clone())
        .map_err(|e| Refus(format!("{} : {}", ou, francais(&e))))
}

/// Les types de bloc connus, pour le message d'erreur.
const TYPES: &str =
    "paragraphe, espace, colonnes, image, tableau, texte_fixe, trait, image_fixe";

/// Les styles de modèle connus, pour le message d'erreur.
const STYLES: &str = "contrat, avenant, attestation";

fn lire_bloc(valeur: &Value, etiquette: &str, police: &Police) -> Result<Bloc, Refus> {
    let objet = match valeur.as_object() {
        Some(o) => o,
        None => return refus(format!("{} : un bloc doit être un objet", etiquette)),
    };
    let type_ = match objet.get("type") {
        Some(Value::String(t)) => t.clone(),
        Some(_) => return refus(format!("{} : le champ \"type\" doit être une chaîne", etiquette)),
        None => {
            return refus(format!(
                "{} : champ absent \"type\" (types connus : {})",
                etiquette, TYPES
            ))
        }
    };
    let mut sans_type = objet.clone();
    sans_type.remove("type");
    let sans_type = Value::Object(sans_type);
    match type_.as_str() {
        "paragraphe" => {
            let p: ParagrapheBrut = lire(&sans_type, etiquette)?;
            let taille = match p.taille {
                Some(t) => strictement_positif(t, "taille", etiquette)?,
                None => police.taille,
            };
            let couleur_p = match &p.couleur {
                Some(c) => couleur(c, etiquette)?,
                None => police.couleur,
            };
            let majuscules = p.majuscules;
            let mut morceaux = Vec::new();
            for m in p.morceaux {
                morceaux.push(Morceau {
                    texte: if majuscules {
                        m.texte.to_uppercase()
                    } else {
                        m.texte
                    },
                    gras: m.gras,
                    italique: m.italique,
                    couleur: match &m.couleur {
                        Some(c) => Some(couleur(c, etiquette)?),
                        None => None,
                    },
                });
            }
            Ok(Bloc::Paragraphe {
                morceaux,
                taille,
                couleur: couleur_p,
                alignement: p.alignement,
                espace_avant: positif_ou_nul(p.espace_avant, "espace_avant", etiquette)?,
                espace_apres: positif_ou_nul(p.espace_apres, "espace_apres", etiquette)?,
                retrait: positif_ou_nul(p.retrait, "retrait", etiquette)?,
            })
        }
        "espace" => {
            let e: EspaceBrut = lire(&sans_type, etiquette)?;
            Ok(Bloc::Espace(positif_ou_nul(e.hauteur, "hauteur", etiquette)?))
        }
        "image" => {
            let i: ImageBrute = lire(&sans_type, etiquette)?;
            Ok(Bloc::Image {
                largeur: strictement_positif(i.largeur, "largeur", etiquette)?,
                chemin: i.chemin,
                flottante: i.flottante,
            })
        }
        "colonnes" => {
            let c: ColonnesBrut = lire(&sans_type, etiquette)?;
            if c.colonnes.is_empty() {
                return refus(format!("{} : une liste de colonnes vide", etiquette));
            }
            let filet = match c.filet_haut {
                Some(f) => Some((
                    couleur(&f.couleur, etiquette)?,
                    strictement_positif(f.epaisseur, "epaisseur", etiquette)?,
                )),
                None => None,
            };
            let mut somme = 0.0f32;
            let mut colonnes = Vec::new();
            for (k, col) in c.colonnes.iter().enumerate() {
                let ou_col = format!("{}, colonne {}", etiquette, k + 1);
                let l = strictement_positif(col.largeur, "largeur", &ou_col)?;
                somme += l;
                let mut blocs = Vec::new();
                for (j, b) in col.blocs.iter().enumerate() {
                    let et = format!("{}, bloc {}", ou_col, j + 1);
                    let bloc = lire_bloc(b, &et, police)?;
                    if matches!(bloc, Bloc::Tableau { .. }) {
                        return refus(format!(
                            "{} : un tableau ne se place pas dans des colonnes (il se coupe entre deux pages, elles non)",
                            et
                        ));
                    }
                    if bloc.est_fixe() {
                        return refus(format!(
                            "{} : un bloc à position fixe (texte_fixe, trait, image_fixe) ne se place pas dans des colonnes",
                            et
                        ));
                    }
                    blocs.push(BlocEtiquete { bloc, etiquette: et });
                }
                colonnes.push((l, blocs));
            }
            if (somme - 1.0).abs() > 0.001 {
                return refus(format!(
                    "{} : la somme des largeurs de colonnes vaut {}, elle doit valoir 1",
                    etiquette, somme
                ));
            }
            Ok(Bloc::Colonnes {
                filet,
                espace_avant: positif_ou_nul(c.espace_avant, "espace_avant", etiquette)?,
                marge_interne_haut: positif_ou_nul(
                    c.marge_interne_haut,
                    "marge_interne_haut",
                    etiquette,
                )?,
                colonnes,
            })
        }
        "tableau" => {
            let t: TableauBrut = lire(&sans_type, etiquette)?;
            lire_tableau(t, etiquette, police)
        }
        "texte_fixe" => {
            let t: TexteFixeBrut = lire(&sans_type, etiquette)?;
            let police_fixe = match t.police.as_deref() {
                None | Some("helvetica") => PoliceFixe::Helvetica,
                Some("times") => PoliceFixe::Times,
                Some("courier") => PoliceFixe::Courier,
                Some(autre) => {
                    return refus(format!(
                        "{} : police \"{}\" inconnue (polices connues : helvetica, times, courier)",
                        etiquette, autre
                    ))
                }
            };
            if t.alignement == Alignement::Justifie {
                return refus(format!(
                    "{} : un texte à position fixe ne se justifie pas (gauche, droite ou centre)",
                    etiquette
                ));
            }
            Ok(Bloc::TexteFixe {
                x: fini(t.x, "x", etiquette)?,
                y: fini(t.y, "y", etiquette)?,
                taille: match t.taille {
                    Some(v) => strictement_positif(v, "taille", etiquette)?,
                    None => police.taille,
                },
                couleur: match &t.couleur {
                    Some(c) => couleur(c, etiquette)?,
                    None => police.couleur,
                },
                texte: t.texte,
                police: police_fixe,
                gras: t.gras,
                alignement: t.alignement,
                largeur_max: match t.largeur_max {
                    Some(v) => Some(strictement_positif(v, "largeur_max", etiquette)?),
                    None => None,
                },
            })
        }
        "trait" => {
            let t: TraitBrut = lire(&sans_type, etiquette)?;
            Ok(Bloc::Trait {
                x1: fini(t.x1, "x1", etiquette)?,
                y1: fini(t.y1, "y1", etiquette)?,
                x2: fini(t.x2, "x2", etiquette)?,
                y2: fini(t.y2, "y2", etiquette)?,
                epaisseur: strictement_positif(t.epaisseur, "epaisseur", etiquette)?,
                couleur: couleur(&t.couleur, etiquette)?,
            })
        }
        "image_fixe" => {
            let i: ImageFixeBrute = lire(&sans_type, etiquette)?;
            Ok(Bloc::ImageFixe {
                x: fini(i.x, "x", etiquette)?,
                y: fini(i.y, "y", etiquette)?,
                largeur: strictement_positif(i.largeur, "largeur", etiquette)?,
                hauteur: strictement_positif(i.hauteur, "hauteur", etiquette)?,
                centree: i.centree,
                chemin: i.chemin,
            })
        }
        autre => refus(format!(
            "{} : type inconnu \"{}\" (types connus : {})",
            etiquette, autre, TYPES
        )),
    }
}

fn lire_filet(f: &Option<FiletBrut>, ou: &str) -> Result<Option<Filet>, Refus> {
    match f {
        Some(f) => Ok(Some((
            couleur(&f.couleur, ou)?,
            strictement_positif(f.epaisseur, "epaisseur", ou)?,
        ))),
        None => Ok(None),
    }
}

/// Une ligne de tableau : les couleurs, la graisse et la taille de la ligne descendent dans ses
/// cellules (celle du morceau, puis celle de la cellule, l'emportent).
fn lire_ligne(
    l: &LigneBrute,
    ou: &str,
    colonnes: &[(f32, Alignement)],
    taille: f32,
    couleur_table: Rvb,
) -> Result<LigneTableau, Refus> {
    let taille = match l.taille {
        Some(t) => strictement_positif(t, "taille", ou)?,
        None => taille,
    };
    let couleur_ligne = match &l.couleur {
        Some(c) => couleur(c, ou)?,
        None => couleur_table,
    };
    let fond = match &l.fond {
        Some(c) => Some(couleur(c, ou)?),
        None => None,
    };
    let mut cellules = Vec::new();
    let mut occupe = 0usize;
    for (k, v) in l.cellules.iter().enumerate() {
        let ou_c = format!("{}, cellule {}", ou, k + 1);
        let brute = match v {
            Value::String(s) => CelluleBrute {
                texte: Some(s.clone()),
                morceaux: None,
                gras: false,
                couleur: None,
                alignement: None,
                etendue: None,
            },
            Value::Object(_) => lire::<CelluleBrute>(v, &ou_c)?,
            _ => return refus(format!("{} : une cellule est une chaîne ou un objet", ou_c)),
        };
        let etendue = brute.etendue.unwrap_or(1);
        if etendue == 0 {
            return refus(format!("{} : etendue doit valoir au moins 1", ou_c));
        }
        if occupe + etendue > colonnes.len() {
            return refus(format!(
                "{} : la ligne occupe plus que les {} colonne(s) du tableau",
                ou_c,
                colonnes.len()
            ));
        }
        let couleur_cellule = match &brute.couleur {
            Some(c) => couleur(c, &ou_c)?,
            None => couleur_ligne,
        };
        let bruts = match (brute.texte, brute.morceaux) {
            (Some(_), Some(_)) => {
                return refus(format!("{} : \"texte\" et \"morceaux\" s'excluent", ou_c))
            }
            (Some(t), None) => vec![MorceauBrut { texte: t, gras: false, italique: false, couleur: None }],
            (None, Some(m)) => m,
            (None, None) => Vec::new(),
        };
        let mut morceaux = Vec::new();
        for m in bruts {
            morceaux.push(Morceau {
                texte: m.texte,
                gras: m.gras || brute.gras || l.gras,
                italique: m.italique,
                couleur: Some(match &m.couleur {
                    Some(c) => couleur(c, &ou_c)?,
                    None => couleur_cellule,
                }),
            });
        }
        cellules.push(CelluleTableau {
            morceaux,
            etendue,
            // Sans mot de la cellule, celui de la colonne où elle commence.
            alignement: brute.alignement.unwrap_or(colonnes[occupe].1),
        });
        occupe += etendue;
    }
    Ok(LigneTableau {
        cellules,
        fond,
        taille,
        filet_haut: lire_filet(&l.filet_haut, ou)?,
        filet_bas: lire_filet(&l.filet_bas, ou)?,
    })
}

fn lire_tableau(t: TableauBrut, etiquette: &str, police: &Police) -> Result<Bloc, Refus> {
    if t.colonnes.is_empty() {
        return refus(format!("{} : un tableau sans colonne", etiquette));
    }
    let mut colonnes = Vec::new();
    let mut somme = 0.0f32;
    for (k, c) in t.colonnes.iter().enumerate() {
        let ou_col = format!("{}, colonne {}", etiquette, k + 1);
        let l = strictement_positif(c.largeur, "largeur", &ou_col)?;
        somme += l;
        colonnes.push((l, c.alignement));
    }
    if (somme - 1.0).abs() > 0.001 {
        return refus(format!(
            "{} : la somme des largeurs de colonnes vaut {}, elle doit valoir 1",
            etiquette, somme
        ));
    }
    let taille = match t.taille {
        Some(x) => strictement_positif(x, "taille", etiquette)?,
        None => police.taille,
    };
    let couleur_table = match &t.couleur {
        Some(c) => couleur(c, etiquette)?,
        None => police.couleur,
    };
    let interligne = match t.interligne {
        Some(x) => strictement_positif(x, "interligne", etiquette)?,
        None => police.interligne,
    };
    let m = &t.marge_cellule;
    let ou_m = format!("{}, marge_cellule", etiquette);
    let marge_cellule = [
        positif_ou_nul(m.haut, "haut", &ou_m)?,
        positif_ou_nul(m.droite, "droite", &ou_m)?,
        positif_ou_nul(m.bas, "bas", &ou_m)?,
        positif_ou_nul(m.gauche, "gauche", &ou_m)?,
    ];
    let mut entete = Vec::new();
    for (i, l) in t.entete.iter().enumerate() {
        let ou = format!("{}, en-tête, ligne {}", etiquette, i + 1);
        entete.push(lire_ligne(l, &ou, &colonnes, taille, couleur_table)?);
    }
    let mut lignes = Vec::new();
    for (i, l) in t.lignes.iter().enumerate() {
        let ou = format!("{}, ligne {}", etiquette, i + 1);
        lignes.push(lire_ligne(l, &ou, &colonnes, taille, couleur_table)?);
    }
    Ok(Bloc::Tableau {
        colonnes,
        interligne,
        marge_cellule,
        filet_lignes: lire_filet(&t.filet_lignes, etiquette)?,
        filets_cellules: lire_filet(&t.filets_cellules, etiquette)?,
        entete,
        lignes,
        espace_avant: positif_ou_nul(t.espace_avant, "espace_avant", etiquette)?,
        espace_apres: positif_ou_nul(t.espace_apres, "espace_apres", etiquette)?,
    })
}

/// Lit et valide une description JSON.
fn lire_repere(r: Option<RepereBrut>, nom: &str, police: &Police) -> Result<Option<Repere>, Refus> {
    let r = match r {
        Some(r) => r,
        None => return Ok(None),
    };
    for t in [&r.gauche, &r.centre, &r.droite] {
        if t.contains('\n') || t.contains('\r') {
            return refus(format!("{} : chaque texte tient sur une ligne (pas de \\n)", nom));
        }
    }
    Ok(Some(Repere {
        taille: strictement_positif(r.taille.unwrap_or(police.taille), "taille", nom)?,
        couleur: match &r.couleur {
            Some(c) => couleur(c, nom)?,
            None => police.couleur,
        },
        distance: positif_ou_nul(r.distance, "distance", nom)?,
        gauche: r.gauche,
        centre: r.centre,
        droite: r.droite,
        gras: r.gras,
        premiere_page: r.premiere_page,
    }))
}

pub fn lire_description(texte: &str) -> Result<Description, Refus> {
    let valeur: Value = serde_json::from_str(texte).map_err(|e| {
        Refus(format!(
            "description illisible : {}",
            francais(&e)
        ))
    })?;
    let objet = match valeur.as_object() {
        Some(o) => o,
        None => return refus("description illisible : l'objet JSON de la description est attendu"),
    };
    match objet.get("format") {
        None => return refus("champ absent \"format\" (la description doit porter \"format\": 1)"),
        // Le 3 est le numéro de la fabrique (`--version`) : une description qui le porte est celle
        // du format 2, avec `largeur_max` en plus. Les trois se lisent de la même façon.
        Some(Value::Number(n)) if matches!(n.as_i64(), Some(1) | Some(2) | Some(3)) => {}
        Some(autre) => {
            return refus(format!(
                "format {} non pris en charge : cette fabrique lit les formats 1, 2 et 3",
                autre
            ))
        }
    }
    let format = objet.get("format").and_then(Value::as_i64).unwrap_or(1);
    let brute: DescriptionBrute = serde_json::from_value(valeur.clone())
        .map_err(|e| Refus(format!("description : {}", francais(&e))))?;

    // Le style (« contrat ») donne la page et la police quand la description ne les dit pas.
    let style = match brute.style.as_deref() {
        None => None,
        Some(nom) => match style_contrat::Variante::depuis_style(nom) {
            Some(v) => Some(v),
            None => {
                return refus(format!(
                    "description : style inconnu \"{}\" (styles connus : {})",
                    nom, STYLES
                ))
            }
        },
    };
    let (page_brute, police_brute) = match style {
        Some(_) => {
            let (p, q) = (style_contrat::page(), style_contrat::police());
            (
                brute.page.unwrap_or(PageBrute {
                    largeur: p.largeur,
                    hauteur: p.hauteur,
                    marges: MargesBrutes { haut: p.haut, droite: p.droite, bas: p.bas, gauche: p.gauche },
                }),
                brute.police.unwrap_or(PoliceBrute {
                    taille: q.taille,
                    interligne: q.interligne,
                    couleur: format!("#{:02X}{:02X}{:02X}", q.couleur.0, q.couleur.1, q.couleur.2),
                }),
            )
        }
        None => (brute.page.unwrap_or_default(), brute.police.unwrap_or_default()),
    };
    let m = &page_brute.marges;
    let page = Page {
        largeur: strictement_positif(page_brute.largeur, "largeur", "page")?,
        hauteur: strictement_positif(page_brute.hauteur, "hauteur", "page")?,
        haut: positif_ou_nul(m.haut, "haut", "page, marges")?,
        droite: positif_ou_nul(m.droite, "droite", "page, marges")?,
        bas: positif_ou_nul(m.bas, "bas", "page, marges")?,
        gauche: positif_ou_nul(m.gauche, "gauche", "page, marges")?,
    };
    if page.largeur_utile() <= 0.0 || page.hauteur_utile() <= 0.0 {
        return refus("page : les marges ne laissent aucune place au texte");
    }
    let police = Police {
        taille: strictement_positif(police_brute.taille, "taille", "police")?,
        interligne: strictement_positif(police_brute.interligne, "interligne", "police")?,
        couleur: couleur(&police_brute.couleur, "police")?,
    };
    let logo = match brute.logo {
        Some(l) => Some(Logo {
            largeur: strictement_positif(l.largeur, "largeur", "logo")?,
            marge_haut: positif_ou_nul(l.marge_haut, "marge_haut", "logo")?,
            marge_droite: positif_ou_nul(l.marge_droite, "marge_droite", "logo")?,
            chemin: l.chemin,
        }),
        None => None,
    };
    let documents_bruts = match brute.documents {
        Some(d) if !d.is_empty() => d,
        Some(_) => return refus("description : la liste \"documents\" est vide"),
        None => return refus("description : champ absent \"documents\""),
    };
    let plusieurs = documents_bruts.len() > 1;
    let mut documents = Vec::new();
    let mut titre_modele: Option<String> = None;
    for (d, doc) in documents_bruts.iter().enumerate() {
        let mut blocs = Vec::new();
        if let Some(mb) = &doc.modele {
            let ou = if plusieurs { format!("document {}", d + 1) } else { "document".to_string() };
            if !doc.blocs.is_empty() {
                return refus(format!("{} : \"blocs\" et \"modele\" s'excluent", ou));
            }
            if format < 2 {
                return refus(format!("{} : \"modele\" demande \"format\": 2", ou));
            }
            if style.is_none() {
                return refus(format!(
                    "{} : \"modele\" demande un \"style\" au niveau de la description (styles connus : {})",
                    ou, STYLES
                ));
            }
            let variante = style.expect("style vérifié plus haut");
            // Le modèle vient d'un fichier (`chemin`) ou de son texte (`texte`) : l'un des deux.
            let (m, nom_modele) = match (&mb.chemin, &mb.texte) {
                (Some(_), Some(_)) => {
                    return refus(format!("{} : \"chemin\" et \"texte\" s'excluent (le modèle vient de l'un ou de l'autre)", ou))
                }
                (None, None) => {
                    return refus(format!("{} : \"modele\" demande \"chemin\" (le fichier) ou \"texte\" (le modèle même)", ou))
                }
                (Some(c), None) => (modele::lire_fichier(c)?, c.clone()),
                (None, Some(t)) => (modele::lire_modele(t, "le modèle donné en texte")?, "le modèle donné en texte".to_string()),
            };
            use style_contrat::Variante;
            if variante != Variante::Contrat && mb.remplace.is_some() {
                return refus(format!("{} : \"remplace\" ne va qu'avec le style \"contrat\"", ou));
            }
            if variante == Variante::Attestation && mb.signature.is_some() {
                return refus(format!(
                    "{} : le style \"attestation\" n'a pas de case de signature (\"signature\")",
                    ou
                ));
            }
            if variante != Variante::Attestation && mb.signature_employeur.is_some() {
                return refus(format!(
                    "{} : \"signature_employeur\" ne va qu'avec le style \"attestation\"",
                    ou
                ));
            }
            if variante == Variante::Attestation && mb.mention_artiste.is_some() {
                return refus(format!(
                    "{} : le style \"attestation\" n'a pas de case de l'artiste (\"mention_artiste\")",
                    ou
                ));
            }
            if variante != Variante::Avenant && !mb.modifications.is_empty() {
                return refus(format!("{} : \"modifications\" ne va qu'avec le style \"avenant\"", ou));
            }
            if variante == Variante::Avenant {
                if mb.modifications.is_empty() {
                    return refus(format!("{} : rien n'a changé, pas d'avenant à écrire (\"modifications\" est vide)", ou));
                }
                if m.modification.is_empty() {
                    return refus(format!("{} n'a pas de bloc MODIFICATION:.", nom_modele));
                }
                for (i, x) in mb.modifications.iter().enumerate() {
                    if x.avant.len() != x.apres.len() {
                        return refus(format!(
                            "{} : modification {} : \"avant\" et \"apres\" n'ont pas le même nombre de paragraphes",
                            ou,
                            i + 1
                        ));
                    }
                }
            }
            let modifications: Vec<style_contrat::Modification> = mb
                .modifications
                .iter()
                .map(|x| style_contrat::Modification {
                    numero: x.numero,
                    article: x.article.clone(),
                    avant: x.avant.clone(),
                    apres: x.apres.clone(),
                })
                .collect();
            let c = style_contrat::Contrat {
                variante,
                valeurs: &mb.valeurs,
                remplace: mb.remplace.as_ref().map(|r| (r.numero.as_str(), r.date.as_str())),
                signature: mb.signature.as_deref(),
                modifications: &modifications,
                signature_employeur: mb.signature_employeur.as_deref(),
                mention_artiste: mb.mention_artiste.as_deref(),
            };
            if titre_modele.is_none() {
                titre_modele = Some(modele::texte_brut(&m.titre, &mb.valeurs)?);
            }
            let prefixe = if plusieurs { format!("document {}, ", d + 1) } else { String::new() };
            documents.push(style_contrat::blocs(&m, &c, &prefixe)?);
            continue;
        }
        for (i, b) in doc.blocs.iter().enumerate() {
            let etiquette = if plusieurs {
                format!("document {}, bloc {}", d + 1, i + 1)
            } else {
                format!("bloc {}", i + 1)
            };
            blocs.push(BlocEtiquete {
                bloc: lire_bloc(b, &etiquette, &police)?,
                etiquette,
            });
        }
        // La page d'un document à position fixe est fixe : ses blocs ne se mêlent pas à ceux qui
        // coulent (leur origine serait celle de l'endroit où l'on en est, pas celle de la page).
        if let Some(premier_fixe) = blocs.iter().position(|b| b.bloc.est_fixe()) {
            if let Some(autre) = blocs.iter().find(|b| !b.bloc.est_fixe()) {
                return refus(format!(
                    "{} : un document à position fixe ne mêle pas les blocs fixes (par exemple {}) et ceux qui coulent (par exemple {}) ; un document par page fixe",
                    autre.etiquette, blocs[premier_fixe].etiquette, autre.etiquette
                ));
            }
        }
        documents.push(blocs);
    }
    let entete_page = lire_repere(brute.entete_page, "entete_page", &police)?;
    let pied_page = lire_repere(brute.pied_page, "pied_page", &police)?;
    Ok(Description {
        page,
        police,
        logo,
        // Sans titre dit, celui du premier modèle (`texte_brut`, sans gras).
        titre: brute.titre.or(titre_modele).unwrap_or_default(),
        documents,
        entete_page,
        pied_page,
    })
}
