//! Configuration › Signature & tampon : la signature, le cachet et le logo de la compagnie, poses ou dessines.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::routes::catalogue::{Acte, Affiche, Champ, Forme, Ecran, Fiche, Place, Refus, FORME, MANQUE, OPERATION};
use crate::routes::catalogue::Route;
use crate::routes::fiches::{aujourd_hui, creer_dossier, ecrire, en_os, refus};
use crate::routes::ouverture::{EcranDeLImage, Identite, ImageIdentite};
use droit::contexte::Contexte;
use socle::echec::{Echec, R};

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
        Route::Identite => {
            let super::SansCharge {} = super::lire_charge(charge)?;
            let mut identite = identite(ctx)?;
            ecran_de_l_identite(&mut identite);
            super::en_json(IdentiteDite {
                phrase: dire_l_identite(&identite),
                identite,
                intro: Some("Ce qui se pose sur les documents de la compagnie : PNG, JPEG ou SVG, 2 Mo au plus. Une image posée part dans chaque document qui la porte : le logiciel l'allège au poids visé."),
                poids_vise_ko: Some(POIDS_VISE_KO),
            })
        }
        Route::PoserIdentite => {
            let (identite, poids) = poser_identite(ctx, super::lire_charge(charge)?)?;
            super::en_json(IdentiteDite { phrase: format!("Posé. {} {}", poids, dire_l_identite(&identite)), identite, intro: None, poids_vise_ko: None })
        }
        Route::RetirerIdentite => {
            let identite = retirer_identite(ctx, super::lire_charge(charge)?)?;
            super::en_json(IdentiteDite { phrase: format!("Retiré. {}", dire_l_identite(&identite)), identite, intro: None, poids_vise_ko: None })
        }
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

/// L'IDENTITE DE LA COMPAGNIE telle que ses routes la rendent : sa phrase, les trois images, et pour l'ecran (`identite`)
/// ce qu'il dit en tete et le poids vise.
#[derive(Debug, Clone, serde::Serialize)]
struct IdentiteDite {
    phrase: String,
    identite: Identite,
    #[serde(skip_serializing_if = "Option::is_none")]
    intro: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    poids_vise_ko: Option<f64>,
}

/// La charge de `poser-identite` : quoi, l'image en base64 (une adresse data: aussi), et le poids vise en Ko.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargePoser {
    quoi: String,
    #[serde(deserialize_with = "socle::forme::presente")]
    contenu: Option<String>,
    #[serde(default)]
    poids: Option<PoidsVise>,
}

/// Le poids vise : un nombre, ou un texte qui en dit un (« 20,5 »).
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(untagged)]
enum PoidsVise {
    Nombre(f64),
    Texte(String),
}

/// La charge de `retirer-identite` : quoi.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeQuoi {
    quoi: String,
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

fn poser_identite(ctx: &Contexte, charge: ChargePoser) -> R<(Identite, String)> {
    let quoi = charge.quoi.as_str();
    let contenu = charge.contenu.as_deref().unwrap_or("");
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
    let objectif_ko = match &charge.poids {
        None => POIDS_VISE_KO,
        Some(p) => {
            let (lu, dit) = match p {
                PoidsVise::Nombre(k) => (Some(*k), k.to_string()),
                PoidsVise::Texte(s) => (s.trim().replace(',', ".").parse::<f64>().ok(), s.clone()),
            };
            match lu {
                Some(k) if (1.0..=OCTETS_MAX as f64 / 1024.0).contains(&k) => k,
                _ => return Err(refus(format!("Le poids visé est un nombre de Ko, de 1 à {} : {} ne l'est pas.", OCTETS_MAX / 1024, socle::fichiers::repr(&dit)))),
            }
        }
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
        let mut registre = crate::routes::ouverture::registre_identite(&dossier)?;
        let signataire = crate::routes::ouverture::signataire_de(&ctx.entreprise);
        registre.signature = Some(crate::routes::ouverture::PoseePour { de: signataire.nom, le: aujourd_hui().iso() });
        ecrire(&dossier.join("identite.json"), super::en_json(&registre)?)?;
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

fn retirer_identite(ctx: &Contexte, charge: ChargeQuoi) -> R<Identite> {
    let quoi = charge.quoi.as_str();
    if !IDENTITES.contains(&quoi) {
        return Err(identite_inconnue(quoi));
    }
    if let Some(f) = chemin_identite(&dossier_identite(ctx), quoi) {
        avant(&f)?;
    }
    identite(ctx)
}

/// L'identite, chaque image avec sa pastille : la sienne, posee, ou celle que le logiciel dessine, faites ; rien de pose,
/// a faire.
fn identite(ctx: &Contexte) -> R<Identite> {
    let mut v = crate::routes::ouverture::identite(ctx, &ctx.gabarits())?;
    for (_, e) in v.images_mut() {
        e.pastille = Some(if e.pose {
            super::Pastille::de("fait", "le vôtre")
        } else if e.automatique {
            super::Pastille::de("fait", "celui du logiciel")
        } else {
            super::Pastille::de("a_faire", "rien de posé")
        });
    }
    Ok(v)
}

/// L'identite de la compagnie en une phrase : la signature, le tampon et le logo, chacun avec ce que l'ecran en dit.
fn dire_l_identite(v: &Identite) -> String {
    let mot = |e: &ImageIdentite| e.pastille.as_ref().map(|p| p.mots.clone()).unwrap_or_default();
    format!("Signature : {} ; tampon : {} ; logo : {}.", mot(&v.signature), mot(&v.cachet), mot(&v.logo))
}

/// CE QUE L'ECRAN DE LA SIGNATURE ET DU TAMPON DIT (pour chaque image son `titre` et ce qu'elle `dit` : ou elle va, ce
/// qu'elle engage, et ce qui la contredit dans la fiche Entreprise ; ce que dit l'image posee, et les mots de ses gestes).
fn ecran_de_l_identite(v: &mut Identite) {
    let ou = |i: &ImageIdentite| if i.documents.is_empty() { String::new() } else { format!("Sur {}", i.documents.join(", ")) };
    let sig = &v.signature;
    let (nom, fonction) = sig.signataire.as_ref().map(|s| (s.nom.clone(), s.fonction.clone())).unwrap_or_default();
    let titre_signature = if nom.is_empty() { "La signature".to_string() } else { format!("La signature de {}{}", nom, if fonction.is_empty() { String::new() } else { format!(", {}", fonction) }) };
    let mut dit = format!("{}. Elle engage la personne qui signe au nom de la compagnie.", ou(sig));
    if nom.is_empty() {
        dit.push_str(" La fiche Entreprise ne dit pas qui signe.");
    }
    if sig.autre_personne == Some(true) {
        dit.push_str(&format!(" Cette image a été posée pour {} ; la personne qui signe est maintenant {}. Importer sa signature à elle, ou corriger la fiche Entreprise.", sig.posee_pour.as_deref().unwrap_or_default(), nom));
    }
    let dits = [
        (titre_signature, dit, None),
        ("Le tampon".to_string(), format!("{}. Il identifie la compagnie, il n'engage personne.", ou(&v.cachet)), Some("dessiné depuis la fiche Entreprise, il la suit : rien à faire")),
        ("Le logo".to_string(), format!("{}, en haut à droite ; sans lui, rien n'y est imprimé.", ou(&v.logo)), None),
    ];
    for ((_, e), (titre, dit, automatique_dit)) in v.images_mut().into_iter().zip(dits) {
        // Ce que dit l'image posee (sa forme, son poids, son jour), et les mots de ses gestes.
        let pose_dit = e.pose.then(|| {
            let jour: String = e.quand.as_deref().unwrap_or_default().chars().take(10).collect();
            let jour = socle::date::Jour::lire(&jour).map(|j| j.jj_mm_aaaa()).unwrap_or(jour);
            format!("{}, {} dans chaque document, posé le {}", e.format.as_deref().unwrap_or_default().to_uppercase(), ko(e.octets.unwrap_or(0) as usize), jour)
        });
        e.ecran = Some(EcranDeLImage {
            titre,
            dit,
            automatique_dit,
            pose_dit,
            retirer_dit: if e.automatique { "Tampon du logiciel" } else { "Retirer" },
            importer_dit: if e.pose { "Modifier…" } else { "Importer…" },
        });
    }
}
