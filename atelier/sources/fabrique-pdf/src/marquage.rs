//! Le marquage d'un PDF existant (`fabrique-pdf --marquer entree.pdf sortie.pdf "texte"`) : un
//! texte posé en pied de chaque page, comme `_marquer` de `moteur/operations_dossier.py`
//! (`page.insert_text((34, hauteur - 12), texte, fontname="helv", fontsize=6, color=(0.45, 0.45,
//! 0.45))`) : à 34 pt du bord gauche, ligne de base à 12 pt du bas de la page, Helvetica 6 pt, gris
//! 0,45, aligné à gauche et sur une ligne.
//!
//! La police est Liberation Sans (les métriques d'Helvetica) en sous-ensemble, comme dans tous les
//! PDF de la fabrique : seuls les glyphes du texte, avec leur table `ToUnicode` (le texte extrait
//! est le texte posé).
//!
//! Les flux de contenu existants ne sont pas touchés : le contenu de la page devient
//! `[ "q", anciens flux…, "Q" (autant que d'`q` restés ouverts, plus un), la marque ]`. L'état
//! graphique de la marque est donc toujours celui d'une page neuve, quoi que l'ancien contenu ait
//! laissé (matrice, couleur, découpe). Les ressources de la page sont recopiées dans la page
//! (jamais modifiées sur place : un nœud `Pages` les partage entre plusieurs pages) avec la
//! police en plus. Une page tournée (`/Rotate`) reçoit la marque dans son sens de lecture.

use std::collections::BTreeSet;

use lopdf::content::Content;
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};

use crate::description::Refus;
use crate::texte::{contenu_de_page, dico, herite, nombres};

/// Le corps de la marque, en points.
pub const CORPS: f64 = 6.0;
/// Sa distance au bord gauche de la page, en points.
pub const X: f64 = 34.0;
/// La distance de sa ligne de base au bord bas de la page, en points.
pub const DISTANCE_DU_BAS: f64 = 12.0;
/// Son gris (rouge = vert = bleu).
pub const GRIS: f64 = 0.45;

fn refus<T>(m: impl Into<String>) -> Result<T, Refus> {
    Err(Refus(m.into()))
}

/// Un nombre pour un flux de contenu : quatre décimales au plus, sans zéros inutiles.
fn nb(v: f64) -> String {
    let s = format!("{:.4}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

/// Le cadre visible de la page (`CropBox` dans `MediaBox`) : x0, y0, x1, y1.
pub(crate) fn cadre(doc: &Document, page: ObjectId) -> Option<[f64; 4]> {
    let v = nombres(doc, herite(doc, page, b"MediaBox"));
    if v.len() != 4 {
        return None;
    }
    let mut c = [v[0].min(v[2]), v[1].min(v[3]), v[0].max(v[2]), v[1].max(v[3])];
    let r = nombres(doc, herite(doc, page, b"CropBox"));
    if r.len() == 4 {
        let r = [r[0].min(r[2]), r[1].min(r[3]), r[0].max(r[2]), r[1].max(r[3])];
        let croise = [c[0].max(r[0]), c[1].max(r[1]), c[2].min(r[2]), c[3].min(r[3])];
        if croise[2] > croise[0] && croise[3] > croise[1] {
            c = croise;
        }
    }
    Some(c)
}

/// Les flux de contenu de la page, par référence (`/Contents` est un flux, un tableau de flux, ou
/// la référence d'un tableau).
pub(crate) fn flux_existants(doc: &Document, page: ObjectId) -> Vec<Object> {
    let contenu = match doc.get_dictionary(page).ok().and_then(|d| d.get(b"Contents").ok()) {
        Some(c) => c.clone(),
        None => return Vec::new(),
    };
    match &contenu {
        Object::Array(v) => v.clone(),
        Object::Reference(id) => match doc.objects.get(id) {
            Some(Object::Array(v)) => v.clone(),
            Some(Object::Stream(_)) => vec![contenu.clone()],
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// Combien de `q` l'ancien contenu laisse ouverts (0 s'il est équilibré ou illisible).
pub(crate) fn q_ouverts(doc: &Document, page: ObjectId) -> usize {
    let flux = contenu_de_page(doc, page);
    let contenu = match Content::decode(&flux) {
        Ok(c) => c,
        Err(_) => return 0,
    };
    let mut profondeur: i64 = 0;
    for op in &contenu.operations {
        match op.operator.as_str() {
            "q" => profondeur += 1,
            "Q" => profondeur = (profondeur - 1).max(0),
            _ => {}
        }
    }
    profondeur as usize
}

/// Le flux de la marque : position et sens de lecture selon la rotation de la page.
fn flux_de_marque(nom_police: &str, codes: &str, c: [f64; 4], rotation: i64) -> Vec<u8> {
    let (largeur, hauteur) = (c[2] - c[0], c[3] - c[1]);
    // La hauteur de la page telle qu'on la voit (celle de `page.rect` dans PyMuPDF).
    let vue = if rotation % 180 == 0 { hauteur } else { largeur };
    let vy = vue - DISTANCE_DU_BAS; // la ligne de base, depuis le haut de la page vue
    let (m, e, f) = match rotation {
        90 => ([0.0, 1.0, -1.0, 0.0], c[0] + vy, c[1] + X),
        180 => ([-1.0, 0.0, 0.0, -1.0], c[0] + largeur - X, c[1] + vy),
        270 => ([0.0, -1.0, 1.0, 0.0], c[0] + largeur - vy, c[1] + hauteur - X),
        _ => ([1.0, 0.0, 0.0, 1.0], c[0] + X, c[1] + hauteur - vy),
    };
    format!(
        "q\n{g} {g} {g} rg\nBT\n/{police} {corps} Tf\n{a} {b} {c} {d} {e} {f} Tm\n<{codes}> Tj\nET\nQ\n",
        g = nb(GRIS),
        police = nom_police,
        corps = nb(CORPS),
        a = nb(m[0]),
        b = nb(m[1]),
        c = nb(m[2]),
        d = nb(m[3]),
        e = nb(e),
        f = nb(f),
        codes = codes
    )
    .into_bytes()
}

/// Le PDF `octets`, avec `texte` en pied de chaque page. Un texte vide laisse le fichier tel quel.
pub fn marquer(octets: &[u8], texte: &str) -> Result<Vec<u8>, Refus> {
    let texte = texte.replace("\r\n", " ").replace(['\r', '\n'], " ");
    let mut doc = crate::lecture::charger(octets)?;
    if doc.trailer.get(b"Encrypt").is_ok() {
        return refus("le PDF est chiffré : on ne le marque pas");
    }
    if texte.is_empty() {
        return Ok(octets.to_vec());
    }

    // La police : Liberation Sans, réduite aux caractères du texte.
    let donnees = crate::polices::sans_regulier()?;
    let face = ttf_parser::Face::parse(&donnees, 0).map_err(|e| Refus(format!("police interne illisible : {:?}", e)))?;
    let mut caracteres = BTreeSet::new();
    for c in texte.chars() {
        if c as u32 > 0xFFFF || face.glyph_index(c).is_none() {
            return refus(format!("le caractère « {} » (U+{:04X}) n'existe pas dans la police Liberation", c, c as u32));
        }
        caracteres.insert(c);
    }
    let police = genpdfi::pdf_subset::embarquer(&mut doc, &donnees, &caracteres)
        .map_err(|e| Refus(format!("police impossible à embarquer : {}", e)))?;
    let codes: String = texte.chars().map(|c| format!("{:04X}", c as u32)).collect();

    let pages: Vec<ObjectId> = doc.get_pages().values().copied().collect();
    if pages.is_empty() {
        return refus("PDF sans page");
    }
    for id in pages {
        let c = match cadre(&doc, id) {
            Some(c) => c,
            None => [0.0, 0.0, 595.0, 842.0],
        };
        let rotation = match herite(&doc, id, b"Rotate") {
            Some(Object::Integer(r)) => r.rem_euclid(360),
            _ => 0,
        };

        // Les ressources de la page, recopiées, avec la police de la marque.
        let mut ressources: Dictionary =
            herite(&doc, id, b"Resources").and_then(|o| dico(&doc, o)).cloned().unwrap_or_default();
        let mut polices: Dictionary =
            ressources.get(b"Font").ok().and_then(|o| dico(&doc, o)).cloned().unwrap_or_default();
        let mut nom_police = "PPMarque".to_string();
        while polices.has(nom_police.as_bytes()) {
            nom_police.push('x');
        }
        polices.set(nom_police.as_str(), Object::Reference(police));
        ressources.set("Font", Object::Dictionary(polices));

        // Le contenu : q, l'ancien, les Q qui le referment, la marque.
        let anciens = flux_existants(&doc, id);
        let ouverts = q_ouverts(&doc, id);
        let avant = doc.add_object(Object::Stream(Stream::new(Dictionary::new(), b"q\n".to_vec())));
        let apres = doc.add_object(Object::Stream(Stream::new(Dictionary::new(), "Q\n".repeat(ouverts + 1).into_bytes())));
        let marque = doc.add_object(Object::Stream(Stream::new(
            Dictionary::new(),
            flux_de_marque(&nom_police, &codes, c, rotation),
        )));
        let mut contenu = vec![Object::Reference(avant)];
        contenu.extend(anciens);
        contenu.push(Object::Reference(apres));
        contenu.push(Object::Reference(marque));

        let page = doc
            .get_object_mut(id)
            .and_then(Object::as_dict_mut)
            .map_err(|e| Refus(format!("page illisible ({})", e)))?;
        page.set("Resources", Object::Dictionary(ressources));
        page.set("Contents", Object::Array(contenu));
    }

    let mut sortie = Vec::new();
    doc.save_to(&mut sortie).map_err(|e| Refus(format!("écriture du PDF impossible ({})", e)))?;
    Ok(sortie)
}
