//! LA BOITE COURRIEL DE LA COMPAGNIE : son adresse et son mot de passe d'application se lisent au coffre (service
//! `courriel`, en CONSULTATION : un courriel n'est pas une declaration, le garde-fou d'un envoi est le mot de passe du
//! logiciel exige avant l'envoi), et le client d'`envois::courriel` les recoit (regle 3 : le transport n'ouvre jamais le
//! coffre).

use std::path::Path;

use serde_json::{json, Value};

use crate::coffre::{Coffre, CONSULTATION};
use droit::contexte::Contexte;
use envois::courriel::{Client, ErreurCourriel};

/// Le service du coffre qui porte le mot de passe de la boite et son adresse.
pub const SERVICE: &str = "courriel";

fn coffre_de(racine: &Path) -> Coffre {
    Coffre::du_poste(racine, "")
}

fn refus(m: impl Into<String>) -> ErreurCourriel {
    ErreurCourriel(m.into())
}

/// L'adresse de la boite, posee avec le mot de passe (`COURRIEL_COMPTE`, puis le coffre). "" si rien.
pub fn adresse(racine: &Path) -> Result<String, ErreurCourriel> {
    if let Ok(v) = std::env::var("COURRIEL_COMPTE") {
        if !v.is_empty() {
            return Ok(v.trim().to_lowercase());
        }
    }
    let du_coffre = coffre_de(racine).compte(SERVICE).map_err(|e| refus(e.message))?;
    Ok(du_coffre.trim().to_lowercase())
}

/// Peut-on ouvrir la boite sans rien demander ? Adresse ET secret.
pub fn disponible(racine: &Path) -> bool {
    match adresse(racine) {
        Ok(a) if !a.is_empty() => coffre_de(racine).disponible(SERVICE).unwrap_or(false),
        _ => false,
    }
}

/// Le mot de passe, lu en CONSULTATION.
pub fn mot_de_passe(racine: &Path) -> Result<String, ErreurCourriel> {
    let lu = coffre_de(racine)
        .lire(&Value::String(SERVICE.into()), &Value::String(CONSULTATION.into()), false, None)
        .map_err(|e| refus(e.message))?;
    match lu {
        Some(m) if !m.is_empty() => Ok(m.replace(' ', "")),
        _ => Err(refus("Aucun mot de passe de boite courriel dans le coffre.\n  paie coffre poser courriel --compte adresse@...")),
    }
}

/// Le vrai client de la boite, adresse et mot de passe lus au coffre.
pub fn client(racine: &Path) -> Result<Client, ErreurCourriel> {
    Client::nouveau(racine, &adresse(racine)?, &mot_de_passe(racine)?)
}

/// Le vrai client de la boite de la compagnie.
pub fn client_du_contexte(ctx: &Contexte) -> Result<Client, ErreurCourriel> {
    client(&ctx.racine)
}

/// La boite s'ouvre-t-elle avec ce qui est pose ? UNE tentative IMAP (`envois::courriel::verifier`). Rend
/// `{"ok", "message", "conseil"}` -- jamais le mot de passe.
pub fn verifier(racine: &Path) -> Result<Value, ErreurCourriel> {
    if adresse(racine)?.is_empty() {
        return Ok(json!({
            "ok": false,
            "message": "Aucune adresse posee avec le mot de passe.",
            "conseil": "paie coffre poser courriel --compte adresse@..."
        }));
    }
    match client(racine) {
        Ok(c) => Ok(envois::courriel::verifier(c)),
        Err(e) => Ok(json!({"ok": false, "message": e.0.lines().next().unwrap_or(""), "conseil": ""})),
    }
}
