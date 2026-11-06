//! LA REPONSE DE L'IMPORT (`importation-apercu`, `importer`) : ce que l'apercu reprendrait, ou ce que l'import a repris,
//! telle que la page la lit -- le dossier fichier par fichier (`TriVu`), les declarations, les bulletins, les certificats
//! et les AEM en PDF lus, chaque fiche que l'import cree, complete ou laisse (`FicheImportee`), et leurs phrases
//! (`Importation::dire`, une fois tout ecrit).
//!
//! Restent des formes ouvertes : ce que porte une fiche (`donnees`, la case de la fiche d'un ecart : les cases de
//! `champs-base.json`, lues par `declarations::champs::ParCase`) et ses anomalies (`declarations::champs::verifier_fiche`).

use serde::Serialize;
use serde_json::{Map, Value};

use crate::routes::{couleur_de_l_etat, Pastille};
use engagement::importe::{ContratLu, PaieLue};
use socle::mois::Mois;

// ------------------------------------------------------------------ les sorts

/// LE SORT D'UN FICHIER OU D'UNE PAGE DU DOSSIER (`import::bulletins`, la table de ses sorts).
///
/// | sort | ce qu'il veut dire |
/// |---|---|
/// | `repris` | ce que le fichier porte entre dans l'apercu |
/// | `deja_repris` | tout ce qu'il porte est deja dans l'historique (ou dans un autre fichier du dossier) |
/// | `libelle_inconnu` | repris, mais une ligne au moins a un libelle que la table ne connait pas : gardee telle quelle |
/// | `deja_saisi` | un engagement saisi paie ce salarie ce mois-la, et aucune paie declaree n'est a completer : non repris |
/// | `incoherent` | une DSN ou une AEM d'une autre entreprise (rien n'en est lu) ; la page contredit le reste (bulletin d'une autre entreprise, NIR qui contredit sa cle, plusieurs paies possibles, deux bulletins qui different) : non repris -- une page qui ne retombe pas sur ses propres totaux est reprise (le bulletin fait foi) |
/// | `incomplet` | ce qui identifie le bulletin ne se lit pas : non repris |
/// | `sans_texte` | aucun mot (lettres en traces, scan) : non repris |
/// | `sans_entetes` | le rang d'en-tetes de son tableau (« Libellé », « Base », les parts) ne se trouve pas : non repris |
/// | `ambigu` | la page se lit de deux facons (deux en-tetes, une colonne sans sa part, deux adresses de salarie) : non repris |
/// | `annexe` | l'annexe d'un bulletin, qui en detaille les cotisations : rien a reprendre |
/// | `non_reconnu` | ni DSN, ni AEM, ni PDF lisible, ni archive |
/// | `doublon` | la copie exacte d'un autre fichier du dossier |
/// | `archive` | un zip, ouvert : son contenu a son propre sort |
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Sort {
    Repris,
    DejaRepris,
    LibelleInconnu,
    DejaSaisi,
    Incoherent,
    Incomplet,
    SansTexte,
    SansEntetes,
    Ambigu,
    Annexe,
    NonReconnu,
    Doublon,
    Archive,
}

impl Sort {
    /// Les sorts d'une page, du plus grave au moins grave : un PDF de plusieurs pages prend le plus grave.
    pub const GRAVITE: [Sort; 10] = [
        Sort::Incoherent,
        Sort::Incomplet,
        Sort::Ambigu,
        Sort::SansEntetes,
        Sort::SansTexte,
        Sort::LibelleInconnu,
        Sort::Repris,
        Sort::DejaSaisi,
        Sort::DejaRepris,
        Sort::Annexe,
    ];

    /// Les sorts qui laissent quelque chose a voir (l'editeur, la table de lecture) : le fichier n'est pas repris.
    pub fn non_repris(self) -> bool {
        matches!(self, Sort::Incoherent | Sort::Incomplet | Sort::Ambigu | Sort::SansEntetes | Sort::SansTexte | Sort::NonReconnu)
    }

    /// L'etat d'un sort (sa couleur) et ses mots, tels qu'on les lit : sur la pastille et dans le resume d'un PDF. Repris,
    /// fait ; repris sous reserve (un libelle que la table ne connait pas), a voir ; non repris parce que quelque chose
    /// manque ou se contredit (`non_repris`), refuse ; laisse expres (deja saisi, annexe, doublon, archive ouverte), une
    /// information.
    pub fn en_mots(self) -> (&'static str, &'static str) {
        match self {
            Sort::Repris => ("fait", "repris"),
            Sort::DejaRepris => ("fait", "déjà repris"),
            Sort::LibelleInconnu => ("a_faire", "libellé inconnu"),
            Sort::DejaSaisi => ("information", "déjà saisi"),
            Sort::Incoherent => ("refuse", "incohérent"),
            Sort::Incomplet => ("refuse", "incomplet"),
            Sort::SansTexte => ("refuse", "sans texte"),
            Sort::SansEntetes => ("refuse", "tableau introuvable"),
            Sort::Ambigu => ("refuse", "ambigu"),
            Sort::Annexe => ("information", "annexe"),
            Sort::NonReconnu => ("refuse", "non reconnu"),
            Sort::Doublon => ("information", "doublon"),
            Sort::Archive => ("information", "archive"),
        }
    }
}

/// Une valeur par sort (un compte, une pastille), dans l'ordre des sorts.
#[derive(Debug, Clone, Serialize)]
pub struct ParSort<T> {
    pub repris: T,
    pub deja_repris: T,
    pub libelle_inconnu: T,
    pub deja_saisi: T,
    pub incoherent: T,
    pub incomplet: T,
    pub sans_texte: T,
    pub sans_entetes: T,
    pub ambigu: T,
    pub annexe: T,
    pub non_reconnu: T,
    pub doublon: T,
    pub archive: T,
}

impl<T> ParSort<T> {
    pub fn de(&mut self, s: Sort) -> &mut T {
        match s {
            Sort::Repris => &mut self.repris,
            Sort::DejaRepris => &mut self.deja_repris,
            Sort::LibelleInconnu => &mut self.libelle_inconnu,
            Sort::DejaSaisi => &mut self.deja_saisi,
            Sort::Incoherent => &mut self.incoherent,
            Sort::Incomplet => &mut self.incomplet,
            Sort::SansTexte => &mut self.sans_texte,
            Sort::SansEntetes => &mut self.sans_entetes,
            Sort::Ambigu => &mut self.ambigu,
            Sort::Annexe => &mut self.annexe,
            Sort::NonReconnu => &mut self.non_reconnu,
            Sort::Doublon => &mut self.doublon,
            Sort::Archive => &mut self.archive,
        }
    }
}

impl Default for ParSort<usize> {
    fn default() -> Self {
        ParSort { repris: 0, deja_repris: 0, libelle_inconnu: 0, deja_saisi: 0, incoherent: 0, incomplet: 0, sans_texte: 0, sans_entetes: 0, ambigu: 0, annexe: 0, non_reconnu: 0, doublon: 0, archive: 0 }
    }
}

/// Les pastilles, par defaut : celle de chaque sort.
impl Default for ParSort<Pastille> {
    fn default() -> Self {
        pastilles_des_sorts()
    }
}

/// LA PASTILLE DE CHAQUE SORT, telle que la page la dessine (`Sort::en_mots`).
pub fn pastilles_des_sorts() -> ParSort<Pastille> {
    let p = |s: Sort| Pastille::de(s.en_mots().0, s.en_mots().1);
    ParSort {
        repris: p(Sort::Repris),
        deja_repris: p(Sort::DejaRepris),
        libelle_inconnu: p(Sort::LibelleInconnu),
        deja_saisi: p(Sort::DejaSaisi),
        incoherent: p(Sort::Incoherent),
        incomplet: p(Sort::Incomplet),
        sans_texte: p(Sort::SansTexte),
        sans_entetes: p(Sort::SansEntetes),
        ambigu: p(Sort::Ambigu),
        annexe: p(Sort::Annexe),
        non_reconnu: p(Sort::NonReconnu),
        doublon: p(Sort::Doublon),
        archive: p(Sort::Archive),
    }
}

// ------------------------------------------------------------------ d'ou vient une valeur

/// La piece d'ou vient une valeur reprise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Source {
    #[serde(rename = "dsn")]
    Dsn,
    #[serde(rename = "bulletin")]
    Bulletin,
    #[serde(rename = "certificat")]
    Certificat,
    #[serde(rename = "AEM")]
    Aem,
}

/// D'ou vient une valeur reprise : la DSN d'un mois, un bulletin (fichier, page), un certificat Congés Spectacles ou une
/// AEM (fichier, page, mois).
#[derive(Debug, Clone, Serialize)]
pub struct Origine {
    pub source: Source,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fichier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mois: Option<String>,
}

impl Origine {
    pub fn dsn(mois: &str) -> Origine {
        Origine { source: Source::Dsn, fichier: None, page: None, mois: Some(mois.to_string()) }
    }

    /// Un bulletin : son fichier et sa page, quand on les sait.
    pub fn bulletin(fichier: Option<String>, page: Option<usize>) -> Origine {
        Origine { source: Source::Bulletin, fichier, page, mois: None }
    }

    /// Une piece : ce qu'elle est, son fichier, sa page, son mois.
    pub fn piece(source: Source, fichier: String, page: usize, mois: Option<String>) -> Origine {
        Origine { source, fichier: Some(fichier), page: Some(page), mois }
    }

    /// « la DSN de mars 2025 », « le bulletin (x.pdf, page 2) », « l'AEM de mars 2025 (x.pdf) ».
    fn en_mots(&self) -> String {
        let fichier = match (&self.fichier, self.page) {
            (Some(f), Some(p)) => format!(" ({}, page {})", f, p),
            (Some(f), None) => format!(" ({})", f),
            _ => String::new(),
        };
        let de_mois = |m: &Option<String>| m.as_deref().filter(|m| !m.is_empty()).map(|m| format!(" {}", socle::texte::de(&Mois::en_clair(m)))).unwrap_or_default();
        match self.source {
            Source::Aem => format!("l'AEM{}{}", de_mois(&self.mois), fichier),
            Source::Certificat => format!("le certificat Congés Spectacles{}{}", de_mois(&self.mois), fichier),
            Source::Bulletin => format!("le bulletin{}", fichier),
            Source::Dsn => format!("la DSN {}", socle::texte::de(&Mois::en_clair(self.mois.as_deref().unwrap_or("")))),
        }
    }
}

/// Une valeur, en mots dans une phrase : une liste de structures, une structure (`cle valeur`), un texte.
fn en_mots(v: &Value) -> String {
    let mot = crate::routes::mot;
    match v {
        Value::Array(l) => l.iter().map(|x| if let Value::Object(o) = x { o.values().map(mot).collect::<Vec<_>>().join(" ") } else { mot(x) }).collect::<Vec<_>>().join(", "),
        Value::Object(o) => o.iter().map(|(k, x)| format!("{} {}", k, mot(x))).collect::<Vec<_>>().join(", "),
        autre => mot(autre),
    }
}

// ------------------------------------------------------------------ une fiche

/// Ou en est une fiche que l'import reprend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EtatFiche {
    ACreer,
    Existante,
    Creee,
    Completee,
}

/// Une case vide d'une fiche qu'une piece remplit : sa valeur et d'ou elle vient.
#[derive(Debug, Clone, Serialize)]
pub struct Completee {
    pub champ: String,
    pub valeur: Value,
    #[serde(flatten)]
    pub origine: Origine,
    pub dit: String,
}

/// Ce qu'une piece dit d'une case autrement que la fiche, et ou.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum DitPar {
    Dsn { dsn: Value, mois: String },
    Bulletin { bulletin: String, fichier: String, page: usize },
    Certificat { certificat: String, fichier: String, page: usize },
    Aem { aem: String, fichier: String, page: usize },
}

/// UN ECART : une case que la fiche dit, qu'une piece disait autrement ; la fiche fait foi.
#[derive(Debug, Clone, Serialize)]
pub struct Ecart {
    pub champ: String,
    /// La case de la fiche, telle qu'elle est (un texte, ou une structure : la banque).
    pub fiche: Value,
    #[serde(flatten)]
    pub par: DitPar,
    pub dit: String,
}

impl Ecart {
    pub fn new(champ: &str, fiche: Value, par: DitPar) -> Ecart {
        let mot = crate::routes::mot;
        let (fiche, par) = match par {
            DitPar::Dsn { dsn, mois } => (brut(&fiche), DitPar::Dsn { dsn: brut(&dsn), mois }),
            autre => (brut(&fiche), autre),
        };
        let (ou, lu) = match &par {
            DitPar::Certificat { certificat, fichier, page } => (format!("le certificat ({}, page {}) disait", fichier, page), certificat.clone()),
            DitPar::Aem { aem, fichier, page } => (format!("l'AEM ({}, page {}) disait", fichier, page), aem.clone()),
            DitPar::Bulletin { bulletin, fichier, page } => (format!("le bulletin ({}, page {}) disait", fichier, page), bulletin.clone()),
            DitPar::Dsn { dsn, mois } => (format!("la DSN {} disait", socle::texte::de(&Mois::en_clair(mois))), mot(dsn)),
        };
        let dit = format!("{} « {} », la fiche dit « {} » : la fiche fait foi", ou, lu, mot(&fiche));
        Ecart { champ: champ.to_string(), fiche, par, dit }
    }
}

/// UNE FICHE QUE L'IMPORT CREE, COMPLETE OU LAISSE, ou sur laquelle il a a redire.
#[derive(Debug, Clone, Serialize)]
pub struct FicheImportee {
    /// `entreprise`, `salarie` ou `spectacle`.
    pub nom: &'static str,
    pub cle: Option<String>,
    pub libelle: String,
    pub etat: EtatFiche,
    pub ecarts: Vec<Ecart>,
    pub anomalies: Vec<Value>,
    /// Ce que porte une fiche a creer (sans ses cles `_…`).
    pub donnees: Map<String, Value>,
    pub completer: Vec<Completee>,
    /// La piece d'ou vient une fiche a creer, quand ce n'est ni une DSN ni un bulletin.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lu_dans: Option<Origine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lu_dans_dit: Option<String>,
    /// Creee, completee ou deja la, faite ; a creer ou a completer, a faire.
    pub pastille: Pastille,
}

/// `brut(valeur)` : un dictionnaire perd ses cles qui commencent par `_`.
pub(crate) fn brut(v: &Value) -> Value {
    match v {
        Value::Object(o) => Value::Object(o.iter().filter(|(k, _)| !k.starts_with('_')).map(|(k, x)| (k.clone(), brut(x))).collect()),
        Value::Array(a) => Value::Array(a.iter().map(brut).collect()),
        autre => autre.clone(),
    }
}

impl FicheImportee {
    pub fn new(nom: &'static str, cle: Option<&str>, libelle: &str, etat: EtatFiche, ecarts: Vec<Ecart>, anomalies: Vec<Value>, donnees: &Map<String, Value>) -> FicheImportee {
        let mut f = FicheImportee {
            nom,
            cle: cle.map(String::from),
            libelle: libelle.to_string(),
            etat,
            ecarts: Vec::new(),
            anomalies: Vec::new(),
            donnees: Map::new(),
            completer: Vec::new(),
            lu_dans: None,
            lu_dans_dit: None,
            pastille: Pastille::de("a_faire", "à créer"),
        };
        f.ajouter_ecarts(ecarts);
        f.poser(donnees, anomalies);
        f
    }

    /// Ce que porte la fiche a creer, et ses anomalies, sans les cles `_…`.
    pub fn poser(&mut self, donnees: &Map<String, Value>, anomalies: Vec<Value>) {
        self.donnees = donnees.iter().filter(|(k, _)| !k.starts_with('_') && !droit::fiche::est_une_note(k)).map(|(k, v)| (k.clone(), brut(v))).collect();
        self.anomalies = anomalies.iter().map(brut).collect();
    }

    /// Les ecarts d'une piece de plus : une case deja notee garde le sien (la DSN passe avant le bulletin).
    pub fn ajouter_ecarts(&mut self, ecarts: Vec<Ecart>) {
        for e in ecarts {
            if !e.champ.starts_with('_') && !self.ecarts.iter().any(|x| x.champ == e.champ) {
                self.ecarts.push(e);
            }
        }
    }

    /// Note ce qui serait complete et d'ou ca vient. Un champ deja note garde sa premiere origine
    /// (`completer::ajouter`).
    pub fn noter(&mut self, champs: &Map<String, Value>, origine: &Origine) {
        for (k, v) in champs {
            if !self.completer.iter().any(|c| &c.champ == k) {
                self.completer.push(Completee { champ: k.clone(), valeur: brut(v), origine: origine.clone(), dit: String::new() });
            }
        }
    }

    /// Le numero d'objet d'un spectacle a creer, en capitales.
    pub fn numero_objet(&self) -> Option<String> {
        self.donnees.get("numero_objet").map(documents::valeur::valeur_en_texte).map(|n| n.to_uppercase())
    }

    /// La fiche s'ecrit-elle : a creer, ou existante avec des cases a completer.
    fn a_ecrire(&self) -> bool {
        self.etat == EtatFiche::ACreer || (self.etat == EtatFiche::Existante && !self.completer.is_empty())
    }

    /// Ou elle en est (`pastille`), d'ou elle vient (`lu_dans_dit`), chaque case qu'une piece remplit (`completer.dit`).
    fn dire(&mut self) {
        self.pastille = match self.etat {
            EtatFiche::Completee => Pastille::de("fait", "complétée"),
            EtatFiche::Existante if !self.completer.is_empty() => Pastille::de("a_faire", "à compléter"),
            EtatFiche::Existante => Pastille::de("fait", "existe déjà"),
            EtatFiche::Creee => Pastille::de("fait", "créée"),
            EtatFiche::ACreer => Pastille::de("a_faire", "à créer"),
        };
        self.lu_dans_dit = self.lu_dans.as_ref().map(|o| format!("Lu dans {}", o.en_mots()));
        let completee = self.etat == EtatFiche::Completee;
        for c in &mut self.completer {
            c.dit = format!("{} « {} », lu dans {} : la case était vide", if completee { "a reçu" } else { "recevra" }, en_mots(&c.valeur), c.origine.en_mots());
        }
    }
}

// ------------------------------------------------------------------ le dossier, fichier par fichier

/// Ou en est la paie qu'un bulletin complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EtatPaie {
    Deja,
    Nouvelle,
    Completee,
}

/// Ou en est le contrat qu'une piece atteste.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EtatContrat {
    Inconnu,
    Deja,
    Ecart,
    Nouveau,
}

/// UN ECART ENTRE UN BULLETIN ET LA PAIE QU'IL COMPLETE : le bulletin l'emporte (a voir), ou l'ecart est note.
#[derive(Debug, Clone, Serialize)]
pub struct EcartPaie {
    pub champ: &'static str,
    /// D'ou vient la paie : `dsn`, `historique` ou `bulletin`.
    pub source: &'static str,
    pub valeur: String,
    pub bulletin: String,
    pub le_bulletin_l_emporte: bool,
    pub champ_dit: &'static str,
    pub pastille: Pastille,
}

impl EcartPaie {
    pub fn new(champ: &'static str, source: &'static str, valeur: String, gagne: bool, bulletin: String) -> EcartPaie {
        let champ_dit = match champ {
            "brut" => "brut",
            "net_imposable" => "net imposable",
            "net_a_payer" => "net à payer",
            "pas" => "prélèvement à la source",
            autre => autre,
        };
        let pastille = if gagne { Pastille::de("a_faire", "le bulletin l'emporte") } else { Pastille::de("information", "noté") };
        EcartPaie { champ, source, valeur, bulletin, le_bulletin_l_emporte: gagne, champ_dit, pastille }
    }
}

/// Ce qu'une page dit d'elle-meme dans le compte rendu.
#[derive(Debug, Clone, Default, Serialize)]
pub struct InfosPage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logiciel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub numero: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aem: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub certificat: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub salarie: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debut: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mois: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paie: Option<EtatPaie>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contrat: Option<EtatContrat>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ecarts: Vec<EcartPaie>,
    /// Ce que le compte des heures, des cachets et des jours a releve.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub compteurs: Vec<String>,
    /// Ce que la page dit d'elle-meme qui ne se retrouve pas : le bulletin fait foi.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub avertissements: Vec<String>,
    /// Le contrat du bulletin, quand un autre contrat atteste aux memes dates est garde.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contrat_bulletin: Option<ContratLu>,
}

/// Le compte rendu d'une page de PDF.
#[derive(Debug, Clone, Serialize)]
pub struct PageVu {
    pub page: usize,
    pub sort: Sort,
    pub raison: String,
    #[serde(flatten)]
    pub infos: InfosPage,
    /// « page 2 (mars 2025) ».
    pub page_dit: String,
    /// Ou se lit un ecart de la page : le fichier, et sa page s'il en a plusieurs.
    pub ou_dit: String,
}

impl PageVu {
    pub fn new(page: usize, sort: Sort, raison: String, infos: InfosPage) -> PageVu {
        PageVu { page, sort, raison, infos, page_dit: String::new(), ou_dit: String::new() }
    }
}

/// Le compte rendu d'un fichier du dossier.
#[derive(Debug, Clone, Serialize)]
pub struct FichierVu {
    pub chemin: String,
    /// La sorte du fichier, au tri (`tri::Sorte::nom`).
    pub sorte: &'static str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pages: Vec<PageVu>,
    pub sort: Sort,
    pub raison: String,
    /// Le nom de sa copie sous `historique/bulletins/`, s'il est repris.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copie: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doublon_de: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mois: Option<String>,
    /// D'ou il vient : une copie, une archive.
    pub de_dit: Option<String>,
    /// « 3 pages ».
    pub pages_dit: String,
}

impl FichierVu {
    /// Ses phrases : d'ou il vient, ses pages, ou se lit l'ecart d'une page.
    pub fn dire(&mut self) {
        self.de_dit = match (&self.doublon_de, &self.archive) {
            (Some(d), _) => Some(format!("(copie de {})", d)),
            (None, Some(a)) => Some(format!("(dans {})", a)),
            _ => None,
        };
        let nb = self.pages.len();
        self.pages_dit = crate::routes::compte(nb, "page", "pages");
        for p in &mut self.pages {
            p.page_dit = match p.infos.mois.as_deref().filter(|m| !m.is_empty()) {
                Some(m) => format!("page {} ({})", p.page, Mois::en_clair(m)),
                None => format!("page {}", p.page),
            };
            p.ou_dit = if nb > 1 { format!("{}, page {}", self.chemin, p.page) } else { self.chemin.clone() };
        }
    }
}

/// Un fichier qui n'est pas repris, pour l'encart.
#[derive(Debug, Clone, Serialize)]
pub struct NonReprisVu {
    pub chemin: String,
    pub sort: Sort,
    pub raison: String,
}

/// CE QUI N'EST PAS REPRIS : chaque fichier et sa raison ; un import qui en laisse se rejoue (`phrase`).
#[derive(Debug, Clone, Serialize)]
pub struct NonRepris {
    pub fichiers: Vec<NonReprisVu>,
    pub phrase: &'static str,
}

/// Le dossier lu, fichier par fichier.
#[derive(Debug, Clone, Default, Serialize)]
pub struct TriVu {
    pub dossier: String,
    pub fichiers: Vec<FichierVu>,
    /// Les fichiers, par sort.
    pub comptes: ParSort<usize>,
    pub total: usize,
    pub non_repris: Option<NonRepris>,
    /// « 12 fichiers dans … ».
    pub dit: String,
}

// ------------------------------------------------------------------ les pieces lues

/// Les bulletins lus : combien, de quand a quand, ce qu'ils ajoutent.
#[derive(Debug, Clone, Default, Serialize)]
pub struct BlocBulletins {
    pub pages: usize,
    pub lus: usize,
    pub repris: usize,
    pub par_logiciel: std::collections::BTreeMap<String, usize>,
    pub du: Option<String>,
    pub au: Option<String>,
    pub fiches_a_creer: usize,
    pub contrats_nouveaux: usize,
    pub contrats: Vec<ContratLu>,
    pub paies_nouvelles: usize,
    pub paies_completees: usize,
    pub paies: Vec<PaieLue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dit: Option<String>,
}

/// Les certificats Congés Spectacles, ou les AEM en PDF, lus : combien, de quand a quand, ce qu'ils ajoutent, et ce qui
/// reste a saisir dans une fiche qu'ils creent.
#[derive(Debug, Clone, Default, Serialize)]
pub struct BlocPieces {
    pub lus: usize,
    pub du: Option<String>,
    pub au: Option<String>,
    pub contrats_nouveaux: usize,
    pub contrats: Vec<ContratLu>,
    pub confirmes: usize,
    pub fiches_a_creer: usize,
    pub reste_a_saisir: Vec<&'static str>,
    pub dit: String,
}

/// La copie de ce que l'import reprend, dans `historique/bulletins/`.
#[derive(Debug, Clone, Serialize)]
pub struct Copie {
    pub copies: usize,
    pub identiques: usize,
    /// Les noms deja copies avec un autre contenu : la copie est gardee telle quelle.
    pub differents: Vec<String>,
    pub dossier: String,
}

// ------------------------------------------------------------------ la reponse

/// CE QUE L'APERCU OU LA REPRISE D'UN DOSSIER REND (`importation-apercu`, `importer`).
#[derive(Debug, Clone, Default, Serialize)]
pub struct Importation {
    pub phrase: String,
    pub source: String,
    /// Les DSN lues, les fichiers d'AEM lus.
    pub dsn: usize,
    pub aem: usize,
    pub premier_mois: String,
    pub dernier_mois: String,
    pub entreprise: Option<FicheImportee>,
    pub salaries: Vec<FicheImportee>,
    pub spectacles: Vec<FicheImportee>,
    /// Les contrats attestes par les AEM deposees.
    pub contrats: usize,
    pub contrats_nouveaux: usize,
    pub contrats_du: String,
    pub contrats_au: String,
    pub contrats_sans_fiche: usize,
    pub avertissements: Vec<String>,
    pub rien_a_faire: bool,
    pub ecrit: bool,
    /// Les fichiers que l'import a ecrits.
    pub fichiers: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copie: Option<Copie>,
    pub tri: TriVu,
    pub bulletins: BlocBulletins,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub certificats: Option<BlocPieces>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aem_pdf: Option<BlocPieces>,
    /// Les depots DSN que les retours deja releves attestent, entres au journal (`importer`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depots_repris: Option<usize>,
    /// Ce que les DSN reprises ont pose dans la configuration (`importer`).
    pub parametrage: Vec<String>,
    pub declarations_dit: String,
    pub titre_dit: &'static str,
    /// Ce que le bouton reprendrait.
    pub reprendre: String,
    pub pastilles: ParSort<Pastille>,
    /// L'encart de ce qui est repris, ou qu'il n'y a rien a reprendre : fait.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encart: Option<Pastille>,
    /// La couleur de la phrase (`importer`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub couleur: Option<&'static str>,
}

/// « 3 bulletins » : `n` et le mot, au pluriel quand il le faut ; rien pour zero.
pub(crate) fn n_mots(n: usize, mot: &str) -> String {
    match n {
        0 => String::new(),
        1 => format!("1 {}", mot),
        n => format!("{} {}s", n, mot),
    }
}

/// Un jour (`2026-11-02`) tel qu'une phrase le dit : « 02/11/2026 » ; ce qui n'en est pas un, tel quel.
fn jour(t: &str) -> String {
    socle::date::Jour::lire(t).map(|j| j.jj_mm_aaaa()).unwrap_or_else(|_| t.to_string())
}

impl Importation {
    /// Les fiches de la reponse : l'entreprise, les salaries, les spectacles.
    pub fn fiches_mut(&mut self) -> impl Iterator<Item = &mut FicheImportee> {
        self.entreprise.iter_mut().chain(self.salaries.iter_mut()).chain(self.spectacles.iter_mut())
    }

    /// LES PHRASES DE CE QUI EST REPRIS : les declarations, les bulletins, les certificats et les AEM en PDF lus, le
    /// dossier, ce que le bouton reprendrait, et pour chaque fiche ce qu'elle recoit d'une piece ; la pastille de chaque
    /// sort et l'encart. Une fois tout ecrit.
    pub fn dire(&mut self) {
        let ecrit = self.ecrit;
        let s = |k: usize| if k > 1 { "s" } else { "" };
        let creer = |k: usize| if ecrit { if k > 1 { " créées" } else { " créée" } } else { " à créer" };
        // Les declarations.
        let periode = if self.premier_mois.is_empty() { String::new() } else { format!(" ({} → {})", Mois::en_clair(&self.premier_mois), Mois::en_clair(&self.dernier_mois)) };
        let contrats = if self.contrats > 0 {
            format!(
                "{} contrat(s) attesté(s) du {} au {} : {} {} à l'historique, {} déjà connu(s){}",
                self.contrats,
                jour(&self.contrats_du),
                jour(&self.contrats_au),
                self.contrats_nouveaux,
                if ecrit { "ajouté(s)" } else { "à ajouter" },
                self.contrats.saturating_sub(self.contrats_nouveaux + self.contrats_sans_fiche),
                if self.contrats_sans_fiche > 0 { format!(", {} sans fiche", self.contrats_sans_fiche) } else { String::new() }
            )
        } else {
            "aucun contrat attesté par une AEM".to_string()
        };
        self.declarations_dit = if self.dsn + self.aem > 0 {
            format!("{} DSN{}, {} fichier(s) AEM : {}.", self.dsn, periode, self.aem, contrats)
        } else {
            "Aucune DSN ni fichier d'AEM déposé dans le dossier.".to_string()
        };
        // Les bulletins.
        let b = &mut self.bulletins;
        b.dit = (b.pages > 0).then(|| {
            let ajouts: Vec<String> = [
                if b.paies_nouvelles > 0 { format!("{} paie{} qu'aucune DSN ne déclare", b.paies_nouvelles, s(b.paies_nouvelles)) } else { String::new() },
                if b.paies_completees > 0 {
                    let k = b.paies_completees;
                    format!("{} paie{} déclarée{} complétée{} (cotisations, coût employeur, lignes)", k, s(k), s(k), s(k))
                } else {
                    String::new()
                },
                if b.contrats_nouveaux > 0 { format!("{} contrat{} attesté{}", b.contrats_nouveaux, s(b.contrats_nouveaux), s(b.contrats_nouveaux)) } else { String::new() },
                if b.fiches_a_creer > 0 { format!("{} fiche{}{}", b.fiches_a_creer, s(b.fiches_a_creer), creer(b.fiches_a_creer)) } else { String::new() },
            ]
            .into_iter()
            .filter(|x| !x.is_empty())
            .collect();
            let du = match (&b.du, &b.au) {
                (Some(d), Some(a)) => format!(" du {} au {}", jour(d), jour(a)),
                _ => String::new(),
            };
            format!(
                "{} bulletin{} lu{} sur {} page{}{} ; {} {}{}.",
                b.lus,
                s(b.lus),
                s(b.lus),
                b.pages,
                s(b.pages),
                du,
                b.repris,
                if ecrit { "repris" } else { "à reprendre" },
                if ajouts.is_empty() { String::new() } else { format!(" : {}", ajouts.join(", ")) }
            )
        });
        // Les certificats Congés Spectacles et les AEM en PDF.
        for (c, piece, pieces, aucune) in [
            (&mut self.certificats, "certificat", "certificats", "Aucun certificat ne porte"),
            (&mut self.aem_pdf, "attestation", "attestations", "Aucune AEM ne porte"),
        ] {
            let Some(c) = c.as_mut() else { continue };
            let ajouts: Vec<String> = [
                if c.contrats_nouveaux > 0 { format!("{} contrat{} attesté{}", c.contrats_nouveaux, s(c.contrats_nouveaux), s(c.contrats_nouveaux)) } else { String::new() },
                if c.confirmes > 0 { format!("{} déjà attesté{} par une autre pièce", c.confirmes, s(c.confirmes)) } else { String::new() },
                if c.fiches_a_creer > 0 { format!("{} fiche{}{}", c.fiches_a_creer, s(c.fiches_a_creer), creer(c.fiches_a_creer)) } else { String::new() },
            ]
            .into_iter()
            .filter(|x| !x.is_empty())
            .collect();
            let du = match (&c.du, &c.au) {
                (Some(d), Some(a)) => format!(" du {} au {}", jour(d), jour(a)),
                _ => String::new(),
            };
            c.dit = format!(
                "{} {} lu{}{}{}.{}",
                c.lus,
                if c.lus > 1 { pieces } else { piece },
                s(c.lus),
                du,
                if ajouts.is_empty() { String::new() } else { format!(" : {}", ajouts.join(", ")) },
                if c.reste_a_saisir.is_empty() { String::new() } else { format!(" {}, et reste à saisir dans chaque fiche : {}.", aucune, c.reste_a_saisir.join(" ; ")) }
            );
        }
        // Le dossier, fichier par fichier.
        self.tri.dit = format!("{} fichier{} dans {}", self.tri.total, s(self.tri.total), self.tri.dossier);
        self.tri.fichiers.iter_mut().for_each(FichierVu::dire);
        self.titre_dit = if ecrit { "Repris" } else { "Serait repris" };
        // Ce que le bouton reprendrait.
        let nc = self.contrats_nouveaux + self.bulletins.contrats_nouveaux;
        let np = self.bulletins.paies_nouvelles + self.bulletins.paies_completees;
        let compte = |l: &[FicheImportee]| l.iter().filter(|f| f.a_ecrire()).count();
        let quoi: Vec<String> = [
            if self.entreprise.as_ref().is_some_and(FicheImportee::a_ecrire) { "la fiche de la compagnie".to_string() } else { String::new() },
            n_mots(compte(&self.salaries), "salarié"),
            n_mots(compte(&self.spectacles), "spectacle"),
            n_mots(nc, "contrat"),
            n_mots(np, "bulletin"),
        ]
        .into_iter()
        .filter(|x| !x.is_empty())
        .collect();
        self.reprendre = quoi.join(", ");
        self.fiches_mut().for_each(FicheImportee::dire);
        self.encart = (self.ecrit || self.rien_a_faire).then(|| Pastille::de("fait", "repris"));
    }

    /// La phrase de l'apercu : ce que le dossier lu donne ; rien n'est ecrit.
    pub fn dire_l_apercu(&mut self) {
        self.dire();
        let b = &self.bulletins;
        let quand = |x: &Option<String>| crate::routes::quand(x.as_deref().unwrap_or(""));
        self.phrase = format!(
            "Dans le dossier lu : {} bulletins lus (du {} au {}), {} à reprendre, fiches à créer : {} ; DSN : {} ; AEM : {}. Rien n'est écrit.",
            b.lus,
            quand(&b.du),
            quand(&b.au),
            b.repris,
            b.fiches_a_creer,
            self.dsn,
            self.aem
        );
    }

    /// La couleur de la phrase d'un import fait.
    pub fn fait(&mut self) {
        self.couleur = Some(couleur_de_l_etat("fait"));
    }
}
