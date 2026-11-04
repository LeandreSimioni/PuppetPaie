//! Configuration › Signature & tampon : la signature, le cachet et le logo de la compagnie, poses ou dessines.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::routes::catalogue::{Acte, Affiche, Champ, Forme, Ecran, Fiche, Place, Refus, FORME, MANQUE, OPERATION};
use crate::routes::catalogue::Route;
use crate::routes::fiches::{aujourd_hui, creer_dossier, ecrire, en_os, exiger, objet_du_fichier, refus, texte_requis};
use droit::contexte::Contexte;
use socle::echec::{erreur, Echec, R};
use socle::fichiers::vrai;

const QUOI_IDENTITE: Champ = Champ { nom: "quoi", libelle: Affiche::new("Signature, cachet ou logo"), forme: Forme::Texte, sens: "signature, cachet ou logo", avance: false };

const IDENTITE_REND: &str = "signature, cachet et logo : chacun posé ou dessiné par le logiciel, et sa pastille";

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Identite);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::Identite,
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
        route: Route::PoserIdentite,
        geste: "Poser la signature, le cachet ou le logo (une image PNG, JPEG ou SVG).",
        charge: &[
            QUOI_IDENTITE,
            Champ { nom: "contenu", libelle: Affiche::new("Image"), forme: Forme::Texte, sens: "l'image en base64 (une adresse data: aussi)", avance: false },
            Champ { nom: "poids", libelle: Affiche::new("Poids visé en Ko"), forme: Forme::Nombre, sens: "le poids visé de l'image dans chaque document, en Ko (20 si absent)", avance: true },
        ],
        exemple: r#"{"quoi":"logo","contenu":"PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIxIiBoZWlnaHQ9IjEiLz4="}"#,
        rend: IDENTITE_REND,
        refus: &[Refus { classe: "ErreurOperation", corriger: "une image lisible (PNG, JPEG, SVG), de 2 Mo au plus" }, FORME, MANQUE],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::RetirerIdentite,
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
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: Route, charge: &Value) -> R<Value> {
    match nom {
        Route::Identite => identite(ctx).map(|mut v| {
            ecran_de_l_identite(&mut v);
            super::phrase_en_tete(v.clone(), dire_l_identite(&v))
        }),
        Route::PoserIdentite => poser_identite(ctx, charge).map(|(v, poids)| super::phrase_en_tete(v.clone(), format!("Posé. {} {}", poids, dire_l_identite(&v)))),
        Route::RetirerIdentite => retirer_identite(ctx, charge).map(|v| super::phrase_en_tete(v.clone(), format!("Retiré. {}", dire_l_identite(&v)))),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

// ------------------------------------------------------------------ l'identite

const IDENTITES: [&str; 3] = ["signature", "cachet", "logo"];
const OCTETS_MAX: usize = 2 * 1024 * 1024;
/// Le poids visé d'une image dans chaque document, en Ko, quand la compagnie n'en dit pas d'autre.
const POIDS_VISE_KO: f64 = 20.0;

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
    std::fs::rename(f, PathBuf::from(nom)).map_err(|e| en_os(socle::echec::du_disque(&f, &e)))
}

fn poser_identite(ctx: &Contexte, charge: &Value) -> R<(Value, String)> {
    let quoi = texte_requis(charge, "quoi")?;
    let contenu = match exiger(charge, "contenu")? {
        Value::Null => "",
        Value::String(s) => s.as_str(),
        _ => return Err(erreur("TypeError", socle::echec::charge("contenu", "un texte"))),
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
    format_de(&octets).ok_or_else(|| {
        refus("Ce fichier n'est ni un PNG, ni un JPEG, ni un SVG (ses premiers octets ne sont ceux d'aucun des trois). Rien n'a ete ecrit.")
    })?;
    let objectif_ko = match charge.get("poids") {
        None | Some(Value::Null) => POIDS_VISE_KO,
        Some(v) => match v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim().replace(',', ".").parse().ok())) {
            Some(k) if (1.0..=OCTETS_MAX as f64 / 1024.0).contains(&k) => k,
            _ => return Err(refus(format!("Le poids visé est un nombre de Ko, de 1 à {} : {} ne l'est pas.", OCTETS_MAX / 1024, socle::fichiers::repr(&socle::valeur::valeur_en_texte(v))))),
        },
    };
    // L'image part dans chaque document qui la porte : elle est allégée avant d'être posée, à la largeur que les
    // documents impriment (`documents::largeur_imprimee`) et au poids visé.
    let largeur = documents::largeur_imprimee(quoi).ok_or_else(|| identite_inconnue(quoi))?;
    let allegee = fabrique_pdf::alleger::alleger(&octets, largeur, (objectif_ko * 1024.0) as usize)
        .map_err(|r| refus(format!("Le fichier n'est pas lisible : {}. Rien n'a ete ecrit.", r.0)))?;
    let poids = dire_le_poids(quoi, &allegee, objectif_ko);
    let (octets, format) = (allegee.octets, allegee.format);
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
    std::fs::write(&cible, &octets).map_err(|e| en_os(socle::echec::du_disque(&cible, &e)))?;
    if quoi == "signature" {
        // Pour qui elle est posee : la personne qui signe CE JOUR-LA.
        let registre = dossier.join("identite.json");
        let mut tenu = objet_du_fichier(&registre)?;
        let signataire = crate::routes::ouverture::signataire_de(&ctx.entreprise)?;
        tenu.insert("signature".into(), json!({"de": signataire["nom"], "le": aujourd_hui().iso()}));
        ecrire(&registre, Value::Object(tenu))?;
    }
    Ok((identite(ctx)?, poids))
}

/// Des octets en Ko, arrondis au-dessus (« 1 Ko » au moins), en Mo à une décimale au-delà de 1 000 Ko.
fn ko(octets: usize) -> String {
    let k = octets.div_ceil(1024).max(1);
    if k < 1000 {
        format!("{} Ko", k)
    } else {
        format!("{:.1} Mo", octets as f64 / 1024.0 / 1024.0).replace('.', ",")
    }
}

/// Ce que l'image posée pèsera dans chaque document, et pourquoi on l'a allégée.
fn dire_le_poids(quoi: &str, a: &fabrique_pdf::alleger::Allegee, objectif_ko: f64) -> String {
    let nom = match quoi {
        "signature" => "La signature",
        "cachet" => "Le tampon",
        _ => "Le logo",
    };
    let pourquoi = "Chaque document part par courriel et se garde : une image lourde l'alourdit chaque fois.";
    let poids = if a.format == "svg" { format!("environ {}", ko(a.octets.len())) } else { ko(a.octets.len()) };
    let mut dit = format!("{} pèsera {} dans chaque document qui le porte", nom, poids);
    if quoi == "signature" {
        dit = dit.replace("qui le porte", "qui la porte");
    }
    if !a.telle_quelle {
        dit.push_str(&format!(" (l'image reçue : {} ; ramenée à {} pixels de large, ce que les documents impriment)", ko(a.avant), a.largeur));
    }
    dit.push('.');
    if a.octets.len() as f64 > objectif_ko * 1024.0 {
        dit.push_str(&format!(" C'est plus que les {} Ko visés : une image plus simple (moins de détails, moins de couleurs) serait plus légère.", objectif_ko));
    }
    format!("{} {}", dit, pourquoi)
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


/// L'identite de la compagnie en une phrase : la signature, le tampon et le logo, chacun avec ce que l'ecran en dit.
fn dire_l_identite(v: &Value) -> String {
    let mot = |k: &str| socle::valeur::valeur_en_texte(&v["identite"][k]["pastille"]["mots"]);
    format!("Signature : {} ; tampon : {} ; logo : {}.", mot("signature"), mot("cachet"), mot("logo"))
}

/// CE QUE L'ECRAN DE LA SIGNATURE ET DU TAMPON DIT (`intro`, et pour chaque image son `titre` et ce qu'elle `dit` : ou elle
/// va, ce qu'elle engage, et ce qui la contredit dans la fiche Entreprise).
fn ecran_de_l_identite(v: &mut Value) {
    v["intro"] = json!("Ce qui se pose sur les documents de la compagnie : PNG, JPEG ou SVG, 2 Mo au plus. Une image posée part dans chaque document qui la porte : le logiciel l'allège au poids visé.");
    v["poids_vise_ko"] = json!(POIDS_VISE_KO);
    let ou = |i: &Value| {
        let docs: Vec<String> = i["documents"].as_array().into_iter().flatten().map(super::mot).collect();
        if docs.is_empty() { String::new() } else { format!("Sur {}", docs.join(", ")) }
    };
    let i = &mut v["identite"];
    let sig = i["signature"].clone();
    let qui = &sig["signataire"];
    let nom = super::mot(&qui["nom"]);
    i["signature"]["titre"] = json!(if nom.is_empty() { "La signature".to_string() } else {
        format!("La signature de {}{}", nom, if super::mot(&qui["fonction"]).is_empty() { String::new() } else { format!(", {}", super::mot(&qui["fonction"])) })
    });
    let mut dit = format!("{}. Elle engage la personne qui signe au nom de la compagnie.", ou(&sig));
    if nom.is_empty() {
        dit.push_str(" La fiche Entreprise ne dit pas qui signe.");
    }
    if vrai(&sig["autre_personne"]) {
        dit.push_str(&format!(" Cette image a été posée pour {} ; la personne qui signe est maintenant {}. Importer sa signature à elle, ou corriger la fiche Entreprise.", super::mot(&sig["posee_pour"]), nom));
    }
    i["signature"]["dit"] = json!(dit);
    let cachet = ou(&i["cachet"]);
    i["cachet"]["titre"] = json!("Le tampon");
    i["cachet"]["dit"] = json!(format!("{}. Il identifie la compagnie, il n'engage personne.", cachet));
    i["cachet"]["automatique_dit"] = json!("dessiné depuis la fiche Entreprise, il la suit : rien à faire");
    let logo = ou(&i["logo"]);
    i["logo"]["titre"] = json!("Le logo");
    i["logo"]["dit"] = json!(format!("{}, en haut à droite ; sans lui, rien n'y est imprimé.", logo));
    // Ce que dit l'image posee (sa forme, son poids, son jour), et les mots de ses gestes.
    for quoi in ["signature", "cachet", "logo"] {
        let e = &mut i[quoi];
        let pose = vrai(&e["pose"]);
        let jour: String = super::mot(&e["quand"]).chars().take(10).collect();
        e["pose_dit"] = if pose {
            json!(format!("{}, {} dans chaque document, posé le {}", super::mot(&e["format"]).to_uppercase(), ko(e["octets"].as_u64().unwrap_or(0) as usize), super::jour_en_clair(&json!(jour))))
        } else {
            Value::Null
        };
        e["retirer_dit"] = json!(if vrai(&e["automatique"]) { "Tampon du logiciel" } else { "Retirer" });
        e["importer_dit"] = json!(if pose { "Modifier…" } else { "Importer…" });
    }
}
