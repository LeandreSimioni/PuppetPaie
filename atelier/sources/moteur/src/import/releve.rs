//! LE RELEVE D'UN PDF, UNE FOIS PAR GESTE. Le tri (`tri.rs`) et chaque lecteur (`bulletin_lu`, `certificat_lu`,
//! `aem_lu`) lisent les pages que la fabrique releve (`fabrique_pdf::inspection::inspecter`) : pendant une route d'import
//! (`Releves::ouvrir`), chaque fichier se releve une fois, et tous reprennent ce releve tant que le fichier est le meme
//! (son chemin, sa taille, sa date). Hors d'une route d'import, chaque lecture releve le fichier.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::SystemTime;

use serde_json::Value;

/// Ce que la fabrique releve d'un PDF : ses pages (`pages`, ...) et ce qu'elle en signale, page par page.
pub struct Releve {
    pub releve: Value,
    pub avertissements: Vec<String>,
}

/// Pourquoi un PDF ne se releve pas.
pub enum Refus {
    /// La fabrique ne l'ouvre pas (sa raison, qui commence par le chemin).
    Fabrique(String),
    /// Le releve rendu ne se lit pas.
    Releve(String),
}

impl Refus {
    /// La phrase des lecteurs.
    pub fn phrase(&self) -> String {
        match self {
            Refus::Fabrique(r) => format!("PDF que la fabrique n'ouvre pas : {}", r),
            Refus::Releve(e) => format!("releve du PDF illisible ({})", e),
        }
    }
}

type Cle = (PathBuf, u64, Option<SystemTime>);

thread_local! {
    static MEMO: RefCell<(u32, HashMap<Cle, Rc<Releve>>)> = RefCell::new((0, HashMap::new()));
}

/// Une route d'import ouverte : chaque fichier s'y releve une fois ; le memo se vide quand la derniere se ferme.
pub struct Releves;

impl Releves {
    pub fn ouvrir() -> Releves {
        MEMO.with(|m| m.borrow_mut().0 += 1);
        Releves
    }
}

impl Drop for Releves {
    fn drop(&mut self) {
        MEMO.with(|m| {
            let mut m = m.borrow_mut();
            m.0 -= 1;
            if m.0 == 0 {
                m.1.clear();
            }
        });
    }
}

/// Le releve du PDF `chemin`.
pub fn du_pdf(chemin: &Path) -> Result<Rc<Releve>, Refus> {
    let meta = std::fs::metadata(chemin).ok();
    let cle: Cle = (chemin.to_path_buf(), meta.as_ref().map_or(0, |m| m.len()), meta.and_then(|m| m.modified().ok()));
    if let Some(r) = MEMO.with(|m| m.borrow().1.get(&cle).cloned()) {
        return Ok(r);
    }
    let (texte, avertissements) = fabrique_pdf::inspection::inspecter(&chemin.to_string_lossy()).map_err(|r| Refus::Fabrique(r.0))?;
    let releve: Value = serde_json::from_str(&texte).map_err(|e| Refus::Releve(e.to_string()))?;
    let r = Rc::new(Releve { releve, avertissements });
    MEMO.with(|m| {
        let mut m = m.borrow_mut();
        if m.0 > 0 {
            m.1.insert(cle, r.clone());
        }
    });
    Ok(r)
}
