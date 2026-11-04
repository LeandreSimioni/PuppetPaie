//! LE FONDEMENT D'UNE VALEUR : le texte depose qui la donne (`Source`), ou plusieurs textes d'emetteurs distincts qui
//! convergent (`Fondement::Convergence`). Toute regle vient d'un texte depose dans `atelier/reference/` (atelier/AGENTS.md,
//! « Les sources ») ; dans un fichier, le champ s'appelle `source`. Le bareme le porte a chaque valeur qu'un texte fonde,
//! la banque des conseils a chaque conseil.
//!
//! Un seul verificateur, en test (`moteur/tests/fichiers/sources.rs`) : chaque fichier est depose, chaque citation s'y
//! lit, une convergence a au moins deux emetteurs distincts.

use serde::Deserialize;

/// Le chemin d'un texte depose, depuis la racine du depot : sous `atelier/reference/`, ou le mode d'emploi du logiciel
/// (`atelier/README.md`, ce que le logiciel fait). Un article d'un entrepot de la DILA (`articles.jsonl.gz`) se nomme
/// `<entrepot>#<code>` (`legi-entrepot/articles.jsonl.gz#travail`, `kali-entrepot/articles.jsonl.gz#3090`), et
/// l'article de la source est sa cle dans l'entrepot (`D7121-41`, `KALIARTI000027609780`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize)]
#[serde(try_from = "String")]
pub struct CheminDepose(String);

/// Le mode d'emploi, la seule source hors de `atelier/reference/`.
pub const MODE_D_EMPLOI: &str = "atelier/README.md";

impl TryFrom<String> for CheminDepose {
    type Error = String;
    fn try_from(s: String) -> Result<Self, String> {
        let fichier = s.split('#').next().unwrap_or_default();
        if !(fichier.starts_with("atelier/reference/") || fichier == MODE_D_EMPLOI) || fichier.contains("..") {
            return Err(format!("« {} » n'est pas un texte déposé sous atelier/reference/", s));
        }
        if s.contains('#') != fichier.ends_with(".jsonl.gz") {
            return Err(format!("« {} » : un article d'entrepôt se nomme <entrepôt>#<code>, et lui seul", s));
        }
        Ok(CheminDepose(s))
    }
}

impl CheminDepose {
    /// Le chemin tel qu'ecrit.
    pub fn texte(&self) -> &str {
        &self.0
    }
    /// Le fichier depose, sans le code d'entrepot.
    pub fn fichier(&self) -> &str {
        self.0.split('#').next().unwrap_or_default()
    }
    /// Le code d'un entrepot de la DILA (`travail`, `3090`), si c'en est un.
    pub fn code(&self) -> Option<&str> {
        self.0.split_once('#').map(|(_, c)| c)
    }
}

/// Qui a ecrit le texte : sa force se lit la (la loi l'emporte sur la convention, la convention etendue sur la plaquette
/// de l'organisme qui collecte).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Emetteur {
    /// Codes, lois, decrets, arretes, ordonnances (Legifrance).
    Loi,
    /// La convention collective et ses accords (les partenaires sociaux ; KALI, BOCC).
    Convention,
    /// Le Bulletin officiel de la securite sociale.
    Boss,
    /// La DGFiP : BOFiP, notices des formulaires.
    Dgfip,
    Urssaf,
    Unedic,
    FranceTravail,
    AgircArrco,
    Audiens,
    Afdas,
    CascSvp,
    /// Le GIP-MDS : la norme DSN et son cahier technique.
    GipMds,
    NetEntreprises,
    /// Le mode d'emploi du logiciel : ce que PuppetPaie fait, pas une regle.
    Puppetpaie,
}

/// Un texte depose, l'endroit, et ce qu'on y lit a l'identique.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub fichier: CheminDepose,
    pub emetteur: Emetteur,
    pub article: Option<String>,
    pub citation: String,
}

/// Ce qui fonde une valeur : les textes d'un meme emetteur (`Texte`), ou des textes d'emetteurs distincts qui convergent,
/// avec le raisonnement qui les joint. Dans un fichier, une source seule s'ecrit a plat (`{fichier, emetteur, article,
/// citation}`), plusieurs textes d'un emetteur `{sources}`, une convergence `{sources, raisonnement}`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(try_from = "FondementLu")]
pub enum Fondement {
    Texte(Vec<Source>),
    Convergence { sources: Vec<Source>, raisonnement: String },
}

/// La forme ecrite d'un fondement, avant de savoir lequel c'est.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FondementLu {
    fichier: Option<CheminDepose>,
    emetteur: Option<Emetteur>,
    article: Option<String>,
    citation: Option<String>,
    sources: Option<Vec<Source>>,
    raisonnement: Option<String>,
}

impl TryFrom<FondementLu> for Fondement {
    type Error = String;
    fn try_from(l: FondementLu) -> Result<Self, String> {
        match (l.sources, l.raisonnement) {
            (Some(sources), Some(raisonnement)) if l.fichier.is_none() && l.emetteur.is_none() && l.article.is_none() && l.citation.is_none() => {
                Ok(Fondement::Convergence { sources, raisonnement })
            }
            (Some(sources), None) if l.fichier.is_none() && l.emetteur.is_none() && l.article.is_none() && l.citation.is_none() && !sources.is_empty() => {
                Ok(Fondement::Texte(sources))
            }
            (None, None) => match (l.fichier, l.emetteur, l.citation) {
                (Some(fichier), Some(emetteur), Some(citation)) => Ok(Fondement::Texte(vec![Source { fichier, emetteur, article: l.article, citation }])),
                _ => Err("une source nomme son fichier, son émetteur et sa citation".to_string()),
            },
            _ => Err("une convergence porte ses sources et son raisonnement, et rien d'autre".to_string()),
        }
    }
}

impl Fondement {
    /// Les textes qui fondent la valeur.
    pub fn sources(&self) -> Vec<&Source> {
        match self {
            Fondement::Texte(sources) | Fondement::Convergence { sources, .. } => sources.iter().collect(),
        }
    }
}
