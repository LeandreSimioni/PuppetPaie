//! LE BRIDAGE (lot U1, 14/10/2026). Il n'y a qu'un logiciel, BRIDE ou DEBLOQUE ; Leandre : « Il n'y a pas de version
//! demonstration. C'est toujours le meme logiciel qui est bride ou pas bride. » Bride, l'accueil ne propose que les
//! compagnies fictives et n'en ajoute pas ; debloque, il propose aussi les compagnies reelles. Rien d'autre ne change :
//! le meme moteur, la meme mise a jour (le bridage n'y joue aucun role, `maj.rs`).
//!
//! Deux fichiers le disent, lus ici et nulle part ailleurs :
//! - `logiciel/EDITION.json` (pose par l'installeur, propre a l'installation : la mise a jour ne le touche pas) :
//!   `"defaut": "bride"` -- l'etat de depart, que le geste de l'accueil peut changer ; ou `"edition": "bride"` -- le
//!   PAQUET SOUDE, bride pour toujours, dont les envois sont joues et qui ne poste a l'editeur que ses incidents.
//! - `.edition.json`, a cote de `logiciel/` : ce que le geste de l'accueil a retenu sur ce poste,
//!   `"edition": "bride"` ou `"debloque"` ; il l'emporte sur le defaut.
//!
//! LES INSTALLATIONS DEJA POSEES gardent leurs fichiers : `"demo"` s'y lit « bride », `"complete"` « debloque » (les mots
//! d'avant le lot U1). Le geste ecrit les mots neufs.

use std::path::Path;

use serde_json::{json, Value};

const FICHIER_GESTE: &str = ".edition.json";
const FICHIER_INSTALLATION: &str = "EDITION.json";
pub const BRIDE: &str = "bride";
pub const DEBLOQUE: &str = "debloque";

/// Un mot du bridage : `Some(true)` bride, `Some(false)` debloque ; les mots d'avant le lot U1 se lisent encore.
fn bride_selon(mot: &str) -> Option<bool> {
    match mot {
        BRIDE | "demo" => Some(true),
        DEBLOQUE | "complete" => Some(false),
        _ => None,
    }
}

fn lire(p: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()
}

fn mot(v: &Option<Value>, cle: &str) -> Option<bool> {
    v.as_ref().and_then(|d| d.get(cle)).and_then(Value::as_str).and_then(bride_selon)
}

/// Le PAQUET SOUDE : `logiciel/EDITION.json` dit `"edition": "bride"` (ou `"demo"`). Bride pour toujours ; ses envois
/// sont joues.
pub fn paquet_soude() -> bool {
    mot(&lire(&crate::interface::compagnies::logiciel().join(FICHIER_INSTALLATION)), "edition") == Some(true)
}

/// Bride ou debloque : le paquet soude reste bride ; sinon ce que le geste a retenu sur le poste, sinon le defaut de
/// l'installation, sinon debloque.
pub fn est_bride() -> bool {
    if paquet_soude() {
        return true;
    }
    if let Some(b) = mot(&lire(&crate::interface::compagnies::base().join(FICHIER_GESTE)), "edition") {
        return b;
    }
    mot(&lire(&crate::interface::compagnies::logiciel().join(FICHIER_INSTALLATION)), "defaut").unwrap_or(false)
}

/// Le mot qui le dit (le journal des incidents, la description du poste) : `bride` ou `debloque`.
pub fn etat() -> &'static str {
    if est_bride() {
        BRIDE
    } else {
        DEBLOQUE
    }
}

/// Le geste secret de l'accueil : bride <-> debloque, retenu sur le poste. Sans effet sur un paquet soude.
pub fn basculer() -> Result<(), String> {
    if paquet_soude() {
        return Ok(());
    }
    let nouveau = if est_bride() { DEBLOQUE } else { BRIDE };
    crate::ecriture::ecrire_json(
        &crate::interface::compagnies::base().join(FICHIER_GESTE),
        &json!({"edition": nouveau, "_quoi": "Le bridage de PuppetPaie sur ce poste : bride (l'accueil ne propose que les compagnies fictives) ou debloque. Change par le geste de l'accueil."}),
        2,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_mots_d_avant_se_lisent_encore() {
        assert_eq!(bride_selon("bride"), Some(true));
        assert_eq!(bride_selon("demo"), Some(true), "une installation posee avant le lot U1");
        assert_eq!(bride_selon("debloque"), Some(false));
        assert_eq!(bride_selon("complete"), Some(false), "un geste retenu avant le lot U1");
        assert_eq!(bride_selon("autre"), None);
    }
}
