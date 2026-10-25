//! Le refus du moteur, et la memoire de l'endroit ou il se construit.
//!
//! Un `Echec` porte sa phrase et, s'il y a lieu, sa classe (`KeyError`, `ErreurOperation`, `BadZipFile`...) que les
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
        classe: classe.unwrap_or("-").to_string(),
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
    les_echecs().lock().unwrap_or_else(|p| p.into_inner())
}
