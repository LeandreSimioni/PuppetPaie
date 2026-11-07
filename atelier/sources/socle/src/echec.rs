//! Le refus du moteur, et la memoire de l'endroit ou il se construit.
//!
//! Un `Echec` porte sa phrase et, s'il y a lieu, sa classe (`ErreurOperation`, `BadZipFile`...) que les
//! routes rendent a la page. Chaque echec construit se note ici (l'endroit dans le code, le fil, l'instant) : la trace
//! d'un refus de route (`diagnostic::trace_du_refus`) retrouve ou il est ne et ce qui l'a precede sur le meme fil.

use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard, OnceLock};
use crate::temps::maintenant_ms;

/// Un refus : sa phrase, et sa classe ; `None` pour un refus ordinaire.
#[derive(Debug)]
pub struct Echec {
    pub classe: Option<&'static str>,
    pub message: String,
}

pub type R<T> = Result<T, Echec>;

impl From<String> for Echec {
    #[track_caller]
    fn from(message: String) -> Self {
        noter_echec(None, &message);
        Echec { classe: None, message }
    }
}

impl From<&str> for Echec {
    #[track_caller]
    fn from(m: &str) -> Self {
        noter_echec(None, m);
        Echec { classe: None, message: m.to_string() }
    }
}

/// Un echec de classe `classe`, son endroit note.
#[track_caller]
pub fn erreur(classe: &'static str, message: impl Into<String>) -> Echec {
    let message = message.into();
    noter_echec(Some(classe), &message);
    Echec { classe: Some(classe), message }
}

/// Un echec de classe quelconque (`None` : un refus ordinaire), son endroit note comme pour `erreur`.
#[track_caller]
pub fn noter_et(classe: Option<&'static str>, message: impl Into<String>) -> Echec {
    let message = message.into();
    noter_echec(classe, &message);
    Echec { classe, message }
}

// ===================================================================================================== la memoire

/// La fenetre de la memoire : les trois minutes d'avant.
pub const FENETRE_MS: u64 = 180_000;
/// Au plus tant d'echecs gardes.
const ECHECS_MAX: usize = 80;

/// Un echec note : quand, sur quel fil, ou dans le code, sa classe et le debut de sa phrase.
#[derive(Clone, Debug)]
pub struct Echoue {
    pub t: u64,
    pub fil: String,
    pub ou: String,
    pub classe: String,
    pub message: String,
}

fn les_echecs() -> &'static Mutex<VecDeque<Echoue>> {
    static E: OnceLock<Mutex<VecDeque<Echoue>>> = OnceLock::new();
    E.get_or_init(|| Mutex::new(VecDeque::new()))
}


/// Le fil courant, tel que la memoire le nomme.
pub fn fil_courant() -> String {
    let t = std::thread::current();
    match t.name() {
        Some(n) => format!("{} {:?}", n, t.id()),
        None => format!("{:?}", t.id()),
    }
}

/// Un echec vient d'etre construit : son endroit dans le code, garde en memoire (les 80 derniers, trois minutes).
#[track_caller]
pub fn noter_echec(classe: Option<&str>, message: &str) {
    let l = std::panic::Location::caller();
    noter_echec_a(&format!("{}:{}", l.file(), l.line()), classe, message);
}

/// Un echec ne a l'endroit `ou` (une panique donne le sien), garde en memoire comme `noter_echec`.
pub fn noter_echec_a(ou: &str, classe: Option<&str>, message: &str) {
    let e = Echoue {
        t: maintenant_ms(),
        fil: fil_courant(),
        ou: ou.to_string(),
        classe: match classe {
            Some(c) => c.to_string(),
            None => "-".to_string(),
        },
        message: message.chars().take(300).collect(),
    };
    let mut l = echecs_notes();
    while l.len() >= ECHECS_MAX || l.front().is_some_and(|x| x.t + FENETRE_MS < e.t) {
        l.pop_front();
    }
    l.push_back(e);
}

/// Les echecs notes, du plus ancien au plus recent.
pub fn echecs_notes() -> MutexGuard<'static, VecDeque<Echoue>> {
    crate::memoire::tenu(les_echecs())
}

/// Un refus et, quand il porte un nom, sa classe
/// (`SaisieIncomplete`, `KeyError`...).
#[derive(Debug)]
pub struct Refus {
    pub classe: Option<&'static str>,
    pub message: String,
}


impl From<String> for Refus {
    fn from(message: String) -> Self {
        Refus { classe: None, message }
    }
}

impl From<&str> for Refus {
    fn from(m: &str) -> Self {
        Refus { classe: None, message: m.to_string() }
    }
}

impl From<Echec> for Refus {
    fn from(e: Echec) -> Self {
        Refus { classe: e.classe, message: e.message }
    }
}

impl From<Refus> for Echec {
    fn from(r: Refus) -> Self {
        Echec { classe: r.classe, message: r.message }
    }
}

// ============================================================================================ ce qui va a part

/// Entre la phrase d'un refus et ce qu'il porte a part : un chemin, une cle, un fichier, une donnee pour la page.
pub const A_PART: char = '\u{1e}';

/// CE QUE LE SOCLE MET A PART D'UN REFUS : le chemin d'un fichier, le nom d'un fichier livre, la cle d'une donnee, ce
/// qui a ete lu ou ce que le systeme a dit. Ce qui n'est pas dit ne s'ecrit pas. Une couche du dessus porte a part ce
/// qu'elle a d'autre (l'etat d'un mois, les anomalies d'une fiche) dans sa propre forme (`avec`).
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct APart {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chemin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fichier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// UN REFUS QUI PORTE QUELQUE CHOSE A PART. Sa phrase se lit a l'ecran, en mots ; ce qui n'est pas fait pour l'ecran
/// (le chemin d'un fichier, la cle d'une donnee, le nom d'un fichier livre) ou ce que la page lit d'autre (l'etat d'un
/// mois, les anomalies d'une fiche) va dans `plus`, une forme (`APart`, ou celle de la couche qui refuse) qui s'ecrit en
/// objet, que la reponse de la route rend a cote de la phrase (`separer`) : la fenetre l'ouvre, le LLM le lit. Une
/// phrase qu'on prefixe garde ce qu'elle porte.
pub fn avec(phrase: impl Into<String>, plus: impl serde::Serialize) -> String {
    format!("{}{}{}", phrase.into(), A_PART, crate::ecriture::json_en_ligne(&plus))
}

/// La phrase d'un refus, sans ce qu'il porte a part (`avec`) : les morceaux du message, chacun sans l'objet qui
/// l'ouvre.
pub fn phrase_de(message: &str) -> String {
    let mut phrase = String::new();
    for (rang, m) in message.split(A_PART).enumerate() {
        let debut = match (rang, crate::forme::objet_en_tete(m)) {
            (0, _) | (_, None) => 0,
            (_, Some(n)) => n,
        };
        phrase.push_str(&m[debut..]);
    }
    phrase
}

impl Echec {
    /// La phrase du refus, sans ce qu'il porte a part (`phrase_de`).
    pub fn phrase(&self) -> String {
        phrase_de(&self.message)
    }
}

/// UN FICHIER QUE LE DISQUE REFUSE (absent, interdit, illisible, mal forme) : la phrase dit ce qui arrive, en mots ; le
/// chemin et la raison du systeme vont a part (`chemin`, `detail`). Un refus du moteur sur ce que le fichier porte (une
/// phrase, `String`) garde sa phrase, et le chemin va a part.
pub fn du_disque<E: std::fmt::Display + 'static>(chemin: &std::path::Path, e: &E) -> String {
    let a_part = |phrase: &str, detail: Option<String>| {
        avec(phrase, APart { chemin: Some(chemin.to_string_lossy().into_owned()), detail, ..APart::default() })
    };
    let e_ = e as &dyn std::any::Any;
    if let Some(phrase) = e_.downcast_ref::<String>() {
        return a_part(phrase, None);
    }
    let phrase = match e_.downcast_ref::<std::io::Error>().map(std::io::Error::kind) {
        Some(std::io::ErrorKind::NotFound) => "Un fichier de la compagnie manque.",
        Some(std::io::ErrorKind::PermissionDenied) => "Le poste refuse l'accès à un fichier de la compagnie : un autre programme le tient peut-être ouvert.",
        Some(std::io::ErrorKind::StorageFull) => "Le disque est plein : un fichier de la compagnie ne s'écrit pas.",
        Some(_) => "Le disque refuse un fichier de la compagnie.",
        None => "Un fichier de la compagnie ne se lit pas : il est abîmé.",
    };
    a_part(phrase, Some(e.to_string()))
}

/// UNE DONNEE QUI NE SE LIT PAS LA OU ELLE DEVRAIT : `quoi` la dit en mots (« Une paie écrite ne se relit pas. ») ; la cle
/// et ce qui a ete lu vont a part (`cle`, `detail`).
pub fn a_la_cle(quoi: &str, cle: &str, detail: impl std::fmt::Display) -> String {
    let detail = detail.to_string();
    avec(quoi, APart { cle: Some(cle.to_string()), detail: Some(detail).filter(|d| !d.is_empty()), ..APart::default() })
}

/// UNE DONNEE LIVREE AVEC LE LOGICIEL QUI MANQUE OU NE SE LIT PAS (un referentiel de `donnees/`) : rien que la compagnie
/// puisse corriger ; le fichier et la cle vont a part.
pub fn livree(fichier: &str, cle: &str) -> String {
    avec(
        "Une donnée livrée avec le logiciel manque ou ne se lit pas : le signaler au développeur.",
        APart { fichier: Some(fichier.to_string()), cle: Some(cle.to_string()), ..APart::default() },
    )
}

/// UNE DEMANDE MAL FORMEE : la charge d'une route n'a pas, a `cle`, ce qu'il faut (`attendu` : « un texte », « une
/// liste »). La cle va a part.
pub fn charge(cle: &str, attendu: &str) -> String {
    self::cle(format!("La demande est mal formée : {} est attendu.", attendu), cle)
}

/// Un refus en mots, et la cle de la donnee qu'il vise, a part.
pub fn cle(phrase: impl Into<String>, cle: &str) -> String {
    avec(phrase, APart { cle: Some(cle.to_string()), ..APart::default() })
}

/// Une paie ecrite (ce que la validation d'un engagement a garde) qui ne se relit pas : la cle va a part (`a_la_cle`).
pub const PAIE_ILLISIBLE: &str = "Une paie écrite ne se relit pas : revalider l'engagement.";

/// UN FICHIER DE LA COMPAGNIE QUI NE PORTE PAS CE QU'IL DEVRAIT (une forme inattendue) : la phrase en mots, le chemin et ce
/// qui ne va pas a part.
pub fn abime(chemin: &std::path::Path, detail: impl std::fmt::Display) -> String {
    avec(
        "Un fichier de la compagnie ne se lit pas : il est abîmé.",
        APart { chemin: Some(chemin.to_string_lossy().into_owned()), detail: Some(detail.to_string()), ..APart::default() },
    )
}
