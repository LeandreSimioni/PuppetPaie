//! `fabrique-pdf --inspecter a.pdf` : ce que les tests Python demandaient à PyMuPDF (`page.rect`,
//! `search_for`, `get_drawings`, `get_image_bbox`, `get_fonts`, `doc.metadata`, `doc.permissions`),
//! en une ligne de JSON. Le relevé est fait par l'interpréteur de `texte.rs` (le même que pour
//! `--texte` : mêmes lignes, mêmes mots), ici on le met en forme dans le repère de PyMuPDF
//! (origine en haut à gauche de la page visible, y vers le bas). Voir FORMAT.md, « Inspecter un PDF ».

use lopdf::{Document, Object};
use serde::Serialize;

use crate::description::Refus;
use crate::texte::{cadre_de, dico, herite, inspecter_page, nombre, suivre, Boite};

#[derive(Serialize)]
struct Chiffrement {
    filtre: String,
    #[serde(rename = "V")]
    v: i64,
    #[serde(rename = "R")]
    r: i64,
    longueur: i64,
    #[serde(rename = "P")]
    p: i64,
    methode: String,
}

#[derive(Serialize)]
struct Mot {
    texte: String,
    rect: Boite,
    ligne: usize,
}

#[derive(Serialize)]
struct Page {
    largeur: f64,
    hauteur: f64,
    rotation: i64,
    polices: Vec<String>,
    images: Vec<Boite>,
    traits: Vec<Boite>,
    mots: Vec<Mot>,
}

#[derive(Serialize)]
struct Inspection {
    chiffrement: Option<Chiffrement>,
    metadonnees: std::collections::BTreeMap<String, String>,
    pages: Vec<Page>,
}

/// Arrondi à 2 décimales (et jamais « -0.0 »).
fn arrondi(x: f64) -> f64 {
    if !x.is_finite() {
        return 0.0;
    }
    (x * 100.0).round() / 100.0 + 0.0
}

/// Le passage du repère du PDF (origine en bas à gauche) à celui de PyMuPDF : origine en haut à
/// gauche de la page visible, y vers le bas. Comme PyMuPDF, les **boîtes** (mots, images, traits)
/// restent dans le repère de la page **non tournée**, alors que `largeur` et `hauteur` (`page.rect`)
/// sont ceux de la page tournée de `/Rotate`.
struct Repere {
    cadre: [f64; 4],
    rotation: i64,
}

impl Repere {
    fn boite(&self, b: &Boite) -> Boite {
        let [x0, _, _, y1] = self.cadre;
        [arrondi(b[0].min(b[2]) - x0), arrondi(y1 - b[1].max(b[3])), arrondi(b[0].max(b[2]) - x0), arrondi(y1 - b[1].min(b[3]))]
    }

    fn dimensions(&self) -> (f64, f64) {
        let (l, h) = (self.cadre[2] - self.cadre[0], self.cadre[3] - self.cadre[1]);
        if self.rotation % 180 == 90 {
            (h, l)
        } else {
            (l, h)
        }
    }
}

/// Les mots d'une ligne : les suites de caractères séparées par des espaces, avec leur boîte.
fn mots_de(ligne: usize, texte: &str, boites: &[Option<Boite>], repere: &Repere, sortie: &mut Vec<Mot>) {
    let mut mot = String::new();
    let mut boite: Option<Boite> = None;
    let mut finir = |mot: &mut String, boite: &mut Option<Boite>| {
        if let (false, Some(b)) = (mot.is_empty(), *boite) {
            sortie.push(Mot { texte: std::mem::take(mot), rect: repere.boite(&b), ligne });
        }
        mot.clear();
        *boite = None;
    };
    for (c, b) in texte.chars().zip(boites.iter()) {
        if c.is_whitespace() {
            finir(&mut mot, &mut boite);
            continue;
        }
        mot.push(c);
        if let Some(b) = b {
            boite = Some(match boite {
                Some(u) => [u[0].min(b[0]), u[1].min(b[1]), u[2].max(b[2]), u[3].max(b[3])],
                None => *b,
            });
        }
    }
    finir(&mut mot, &mut boite);
}

/// La table PDFDocEncoding au-delà de Latin-1 (ISO 32000-1, annexe D.2).
fn pdfdoc(b: u8) -> char {
    const BAS: [char; 8] = ['\u{2D8}', '\u{2C7}', '\u{2C6}', '\u{2D9}', '\u{2DD}', '\u{2DB}', '\u{2DA}', '\u{2DC}'];
    const HAUT: [char; 33] = [
        '\u{2022}', '\u{2020}', '\u{2021}', '\u{2026}', '\u{2014}', '\u{2013}', '\u{192}', '\u{2044}', '\u{2039}', '\u{203A}',
        '\u{2212}', '\u{2030}', '\u{201E}', '\u{201C}', '\u{201D}', '\u{2018}', '\u{2019}', '\u{201A}', '\u{2122}', '\u{FB01}',
        '\u{FB02}', '\u{141}', '\u{152}', '\u{160}', '\u{178}', '\u{17D}', '\u{131}', '\u{142}', '\u{153}', '\u{161}', '\u{17E}',
        '\u{FFFD}', '\u{20AC}',
    ];
    match b {
        0x18..=0x1F => BAS[(b - 0x18) as usize],
        0x80..=0xA0 => HAUT[(b - 0x80) as usize],
        _ => b as char,
    }
}

/// Une chaîne de texte PDF : UTF-16BE ou UTF-16LE avec marque, UTF-8 avec marque, sinon PDFDocEncoding.
fn chaine_pdf(s: &[u8]) -> String {
    let utf16 = |o: &[u8], be: bool| {
        let u: Vec<u16> = o
            .chunks(2)
            .map(|p| {
                let (a, b) = (p[0] as u16, *p.get(1).unwrap_or(&0) as u16);
                if be { (a << 8) | b } else { (b << 8) | a }
            })
            .collect();
        String::from_utf16_lossy(&u)
    };
    if s.starts_with(&[0xFE, 0xFF]) {
        utf16(&s[2..], true)
    } else if s.starts_with(&[0xFF, 0xFE]) {
        utf16(&s[2..], false)
    } else if s.starts_with(&[0xEF, 0xBB, 0xBF]) {
        String::from_utf8_lossy(&s[3..]).into_owned()
    } else if s.iter().any(|b| *b >= 0x80) && std::str::from_utf8(s).is_ok() {
        // Des octets qui forment de l'UTF-8 valide (printpdf l'écrit ainsi, sans marque) : MuPDF
        // les lit comme tels, et un texte en PDFDocEncoding n'a presque jamais cette forme.
        String::from_utf8_lossy(s).into_owned()
    } else {
        s.iter().map(|b| pdfdoc(*b)).collect()
    }
}

/// Le dictionnaire `/Encrypt` du fichier (à lire AVANT de déchiffrer : le déchiffrement l'efface).
fn chiffrement(doc: &Document) -> Option<Chiffrement> {
    let e = dico(doc, doc.trailer.get(b"Encrypt").ok()?)?;
    let entier = |cle: &[u8]| e.get(cle).ok().and_then(|o| nombre(doc, o));
    let v = entier(b"V").unwrap_or(0.0) as i64;
    let r = entier(b"R").unwrap_or(0.0) as i64;
    let longueur = match v {
        5 => 256,
        4 => 128,
        _ => entier(b"Length").map(|l| l as i64).unwrap_or(40),
    };
    let mut p = entier(b"P").unwrap_or(0.0) as i64;
    if p > i32::MAX as i64 {
        p -= 1 << 32;
    }
    let nom = |o: Option<&Object>| o.and_then(|o| match suivre(doc, o) {
        Object::Name(n) => Some(String::from_utf8_lossy(n).into_owned()),
        _ => None,
    });
    let methode = if v >= 4 {
        let filtre = nom(e.get(b"StmF").ok()).unwrap_or_else(|| "Identity".into());
        let cfm = e
            .get(b"CF")
            .ok()
            .and_then(|o| dico(doc, o))
            .and_then(|cf| cf.get(filtre.as_bytes()).ok())
            .and_then(|o| dico(doc, o))
            .and_then(|f| nom(f.get(b"CFM").ok()));
        match cfm {
            Some(m) => m,
            None => filtre,
        }
    } else {
        "RC4".to_string()
    };
    Some(Chiffrement { filtre: nom(e.get(b"Filter").ok()).unwrap_or_else(|| "Standard".into()), v, r, longueur, p, methode })
}

/// Le dictionnaire `/Info`, chaînes décodées (`{}` s'il n'y en a pas).
fn metadonnees(doc: &Document) -> std::collections::BTreeMap<String, String> {
    let mut m = std::collections::BTreeMap::new();
    let info = match doc.trailer.get(b"Info").ok().and_then(|o| dico(doc, o)) {
        Some(i) => i,
        None => return m,
    };
    for (cle, valeur) in info.iter() {
        let texte = match suivre(doc, valeur) {
            Object::String(s, _) => chaine_pdf(s),
            Object::Name(n) => String::from_utf8_lossy(n).into_owned(),
            Object::Integer(i) => i.to_string(),
            Object::Real(r) => r.to_string(),
            Object::Boolean(b) => b.to_string(),
            _ => continue,
        };
        m.insert(String::from_utf8_lossy(cle).into_owned(), texte);
    }
    m
}

fn page_de(doc: &Document, numero: u32, id: lopdf::ObjectId, avertissements: &mut Vec<String>) -> Page {
    // Sans MediaBox, PyMuPDF prend le format Lettre.
    let cadre = cadre_de(doc, id).unwrap_or([0.0, 0.0, 612.0, 792.0]);
    let tour = herite(doc, id, b"Rotate").and_then(|o| nombre(doc, o)).unwrap_or(0.0) as i64;
    let rotation = if tour % 90 == 0 { tour.rem_euclid(360) } else { 0 };
    let repere = Repere { cadre, rotation };
    let (largeur, hauteur) = repere.dimensions();
    let mut page = Page {
        largeur: arrondi(largeur),
        hauteur: arrondi(hauteur),
        rotation,
        polices: Vec::new(),
        images: Vec::new(),
        traits: Vec::new(),
        mots: Vec::new(),
    };
    match inspecter_page(doc, id) {
        Ok(lue) => {
            page.polices = lue.polices;
            page.images = lue.images.iter().map(|b| repere.boite(b)).collect();
            page.traits = lue.traits.iter().map(|b| repere.boite(b)).collect();
            for (k, (texte, boites)) in lue.lignes.iter().enumerate() {
                mots_de(k, texte, boites, &repere, &mut page.mots);
            }
        }
        Err(m) => avertissements.push(format!("page {} : {}", numero, m)),
    }
    page
}

/// L'inspection de `chemin` : la ligne de JSON, et les avertissements (pages illisibles).
pub fn inspecter(chemin: &str) -> Result<(String, Vec<String>), Refus> {
    let octets = std::fs::read(chemin).map_err(|e| Refus(format!("{} illisible ({})", chemin, e)))?;
    let mut doc = crate::lecture::charger(&octets).map_err(|r| Refus(format!("{} {}", chemin, r.0)))?;
    let chiffre = chiffrement(&doc);
    crate::chiffrement::dechiffrer(&mut doc, b"").map_err(|r| Refus(format!("{} : {}", chemin, r.0)))?;
    let mut avertissements = Vec::new();
    let pages = doc.get_pages().into_iter().map(|(n, id)| page_de(&doc, n, id, &mut avertissements)).collect();
    let inspection = Inspection { chiffrement: chiffre, metadonnees: metadonnees(&doc), pages };
    let json = serde_json::to_string(&inspection).map_err(|e| Refus(format!("{} : {}", chemin, e)))?;
    Ok((json, avertissements))
}
