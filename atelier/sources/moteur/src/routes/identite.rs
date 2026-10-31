//! Configuration › Signature & tampon : la signature, le cachet et le logo de la compagnie, poses ou dessines.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::routes::catalogue::{Acte, Champ, Ecran, Fiche, Place, Refus, FORME, MANQUE, OPERATION};
use crate::routes::fiches::{aujourd_hui, creer_dossier, ecrire, en_os, exiger, objet_du_fichier, refus, texte_requis};
use droit::contexte::Contexte;
use socle::echec::{erreur, Echec, R};
use socle::fichiers::vrai;

const QUOI_IDENTITE: Champ = Champ { nom: "quoi", sens: "signature, cachet ou logo", avance: false };

const IDENTITE_REND: &str = "signature, cachet et logo : chacun posé ou dessiné par le logiciel, et sa pastille";

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Identite);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        nom: "identite",
        geste: "Dire la signature, le cachet et le logo de la compagnie.",
        charge: &[],
        exemple: r#"{}"#,
        rend: IDENTITE_REND,
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        nom: "poser-identite",
        geste: "Poser la signature, le cachet ou le logo (une image PNG, JPEG ou SVG).",
        charge: &[QUOI_IDENTITE, Champ { nom: "contenu", sens: "l'image en base64 (une adresse data: aussi)", avance: false }],
        exemple: r#"{"quoi":"logo","contenu":"PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIxIiBoZWlnaHQ9IjEiLz4="}"#,
        rend: IDENTITE_REND,
        refus: &[Refus { classe: "ErreurOperation", corriger: "une image lisible (PNG, JPEG, SVG), de 2 Mo au plus" }, FORME, MANQUE],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        nom: "retirer-identite",
        geste: "Retirer la signature, le cachet ou le logo posé.",
        charge: &[QUOI_IDENTITE],
        exemple: r#"{"quoi":"logo"}"#,
        rend: IDENTITE_REND,
        refus: &[OPERATION, MANQUE],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
];

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "identite" => identite(ctx),
        "poser-identite" => poser_identite(ctx, charge),
        "retirer-identite" => retirer_identite(ctx, charge),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

// ------------------------------------------------------------------ l'identite

const IDENTITES: [&str; 3] = ["signature", "cachet", "logo"];
const OCTETS_MAX: usize = 2 * 1024 * 1024;

#[track_caller]
fn identite_inconnue(quoi: &str) -> Echec {
    refus(format!("Identite inconnue : {}. Connues : cachet, logo, signature.", socle::fichiers::repr(quoi)))
}

fn dossier_identite(ctx: &Contexte) -> PathBuf {
    ctx.gabarits().join("identite")
}

/// Le fichier pose pour `quoi`, quelle que soit son extension.
fn chemin_identite(dossier: &Path, quoi: &str) -> Option<PathBuf> {
    [".png", ".jpg", ".svg"].iter().map(|e| dossier.join(format!("{}{}", quoi, e))).find(|f| f.exists())
}

/// Le format de ces octets, reconnu sur l'en-tete et non sur l'extension.
fn format_de(o: &[u8]) -> Option<&'static str> {
    if o.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        return Some("png");
    }
    if o.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some("jpeg");
    }
    let tete = &o[..o.len().min(2048)];
    let debut = tete.iter().position(|c| !c.is_ascii_whitespace() && *c != 0x0b).unwrap_or(tete.len());
    let tete = &tete[debut..];
    if tete.starts_with(b"<?xml") || tete.starts_with(b"<svg") || tete.starts_with(b"<!--") {
        let long = o[..o.len().min(8192)].to_ascii_lowercase();
        if long.windows(4).any(|w| w == b"<svg") {
            return Some("svg");
        }
    }
    None
}

fn avant(f: &Path) -> R<()> {
    let mut nom = f.as_os_str().to_owned();
    nom.push(".avant");
    std::fs::rename(f, PathBuf::from(nom)).map_err(|e| en_os(format!("{} : {}", f.display(), e)))
}

fn poser_identite(ctx: &Contexte, charge: &Value) -> R<Value> {
    let quoi = texte_requis(charge, "quoi")?;
    let contenu = match exiger(charge, "contenu")? {
        Value::Null => "",
        Value::String(s) => s.as_str(),
        _ => return Err(erreur("TypeError", "`contenu` : un texte est attendu")),
    };
    if !IDENTITES.contains(&quoi) {
        return Err(identite_inconnue(quoi));
    }
    let utile = contenu.rsplit(',').next().unwrap_or("");
    let octets = crate::compagnie::problemes::base64_strict(utile).ok_or_else(|| refus("Le fichier n'est pas lisible : base64 invalide."))?;
    if octets.is_empty() {
        return Err(refus("Le fichier est vide."));
    }
    if octets.len() > OCTETS_MAX {
        return Err(refus(format!("Le fichier fait {:.1} Mo ; la limite est de 2 Mo.", octets.len() as f64 / 1024.0 / 1024.0)));
    }
    let format = format_de(&octets).ok_or_else(|| {
        refus("Ce fichier n'est ni un PNG, ni un JPEG, ni un SVG (ses premiers octets ne sont ceux d'aucun des trois). Rien n'a ete ecrit.")
    })?;
    let dossier = dossier_identite(ctx);
    creer_dossier(&dossier)?;
    // Les autres extensions s'en vont : une identite, un fichier.
    if let Some(ancien) = chemin_identite(&dossier, quoi) {
        avant(&ancien)?;
    }
    let extension = match format {
        "png" => ".png",
        "jpeg" => ".jpg",
        _ => ".svg",
    };
    let cible = dossier.join(format!("{}{}", quoi, extension));
    std::fs::write(&cible, &octets).map_err(|e| en_os(format!("{} : {}", cible.display(), e)))?;
    if quoi == "signature" {
        // Pour qui elle est posee : la personne qui signe CE JOUR-LA.
        let registre = dossier.join("identite.json");
        let mut tenu = objet_du_fichier(&registre)?;
        let signataire = crate::routes::ouverture::signataire_de(&ctx.entreprise)?;
        tenu.insert("signature".into(), json!({"de": signataire["nom"], "le": aujourd_hui().iso()}));
        ecrire(&registre, Value::Object(tenu))?;
    }
    identite(ctx)
}

fn retirer_identite(ctx: &Contexte, charge: &Value) -> R<Value> {
    let quoi = texte_requis(charge, "quoi")?;
    if !IDENTITES.contains(&quoi) {
        return Err(identite_inconnue(quoi));
    }
    if let Some(f) = chemin_identite(&dossier_identite(ctx), quoi) {
        avant(&f)?;
    }
    identite(ctx)
}

fn identite(ctx: &Contexte) -> R<Value> {
    let mut v = crate::routes::ouverture::identite(ctx, &ctx.gabarits())?;
    // La pastille de chaque image : la sienne, posee, ou celle que le logiciel dessine, faites ; rien de pose, a faire.
    for e in v.as_object_mut().into_iter().flat_map(|o| o.values_mut()) {
        let Some(o) = e.as_object_mut() else { continue };
        let p = if o.get("pose").is_some_and(vrai) {
            super::pastille("fait", "le vôtre")
        } else if o.get("automatique").is_some_and(vrai) {
            super::pastille("fait", "celui du logiciel")
        } else {
            super::pastille("a_faire", "rien de posé")
        };
        o.insert("pastille".into(), p);
    }
    Ok(json!({"identite": v}))
}

