//! L'extraction du texte d'un PDF (`fabrique-pdf --texte`) : le texte de chaque page, dans
//! l'ordre où le PDF l'écrit, au plus près de `page.get_text()` de PyMuPDF (MuPDF).
//!
//! Trois étages :
//!
//! 1. un interpréteur du flux de contenu (états graphiques, matrices, opérateurs de texte,
//!    objets formulaire, apparences des annotations — MuPDF dessine les champs de formulaire, et
//!    leur texte sort avec celui de la page) qui rend des caractères positionnés ;
//! 2. les polices : table `ToUnicode` quand la police en a une (elle l'emporte, comme dans
//!    MuPDF), sinon l'encodage de la police simple (WinAnsi, MacRoman, Standard, `/Differences`) ;
//!    les largeurs viennent du PDF, ou de Liberation pour les polices standard sans `/Widths` ;
//! 3. la mise en lignes : un caractère prolonge la ligne courante s'il part du bout du précédent
//!    (à un espace près) sur la même ligne de base ; sinon il ouvre une ligne. C'est la règle de
//!    MuPDF, pas un tri géométrique : le texte sort dans l'ordre du flux.
//!
//! Le résultat est le texte de chaque page, les lignes séparées par `\n`, un `\n` final.

use std::collections::HashMap;
use std::rc::Rc;

use lopdf::content::Content;
use lopdf::{Dictionary, Document, Object, ObjectId};

use crate::glyphes::GLYPHES;

type Mat = [f64; 6];
const ID: Mat = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
static NUL: Object = Object::Null;

/// `a` puis `b` (convention PDF des vecteurs lignes).
fn mul(a: &Mat, b: &Mat) -> Mat {
    [
        a[0] * b[0] + a[1] * b[2],
        a[0] * b[1] + a[1] * b[3],
        a[2] * b[0] + a[3] * b[2],
        a[2] * b[1] + a[3] * b[3],
        a[4] * b[0] + a[5] * b[2] + b[4],
        a[4] * b[1] + a[5] * b[3] + b[5],
    ]
}

fn point(m: &Mat, x: f64, y: f64) -> (f64, f64) {
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

// ---------------------------------------------------------------------------------------------
// Lecture des objets.
// ---------------------------------------------------------------------------------------------

pub(crate) fn suivre<'a>(doc: &'a Document, o: &'a Object) -> &'a Object {
    doc.dereference(o).map(|(_, o)| o).unwrap_or(&NUL)
}

pub(crate) fn nombre(doc: &Document, o: &Object) -> Option<f64> {
    match suivre(doc, o) {
        Object::Integer(i) => Some(*i as f64),
        Object::Real(r) => Some(*r as f64),
        _ => None,
    }
}

pub(crate) fn dico<'a>(doc: &'a Document, o: &'a Object) -> Option<&'a Dictionary> {
    match suivre(doc, o) {
        Object::Dictionary(d) => Some(d),
        Object::Stream(s) => Some(&s.dict),
        _ => None,
    }
}

fn nom<'a>(doc: &'a Document, o: &'a Object) -> Option<&'a [u8]> {
    match suivre(doc, o) {
        Object::Name(n) => Some(n.as_slice()),
        _ => None,
    }
}

pub(crate) fn nombres(doc: &Document, o: Option<&Object>) -> Vec<f64> {
    match o.map(|o| suivre(doc, o)) {
        Some(Object::Array(v)) => v.iter().filter_map(|x| nombre(doc, x)).collect(),
        _ => Vec::new(),
    }
}

fn matrice(doc: &Document, o: Option<&Object>) -> Mat {
    let v = nombres(doc, o);
    if v.len() == 6 {
        [v[0], v[1], v[2], v[3], v[4], v[5]]
    } else {
        ID
    }
}

fn contenu_du_flux(o: &Object) -> Vec<u8> {
    match o {
        Object::Stream(s) => s.decompressed_content().unwrap_or_else(|_| s.content.clone()),
        _ => Vec::new(),
    }
}

// ---------------------------------------------------------------------------------------------
// Les tables CMap : ToUnicode, et codes des polices composées.
// ---------------------------------------------------------------------------------------------

#[derive(Debug, PartialEq)]
enum Jeton {
    Hex(Vec<u8>),
    Mot(String),
    Ouvre,
    Ferme,
}

fn jetons(octets: &[u8]) -> Vec<Jeton> {
    let mut v = Vec::new();
    let mut i = 0;
    while i < octets.len() {
        let c = octets[i];
        match c {
            b'%' => {
                while i < octets.len() && octets[i] != b'\n' && octets[i] != b'\r' {
                    i += 1;
                }
            }
            b'<' if octets.get(i + 1) == Some(&b'<') => i += 2,
            b'>' if octets.get(i + 1) == Some(&b'>') => i += 2,
            b'>' => i += 1,
            b'<' => {
                let mut chiffres = Vec::new();
                i += 1;
                while i < octets.len() && octets[i] != b'>' {
                    if octets[i].is_ascii_hexdigit() {
                        chiffres.push(octets[i]);
                    }
                    i += 1;
                }
                i += 1;
                if chiffres.len() % 2 == 1 {
                    chiffres.push(b'0');
                }
                let hex = |b: u8| (b as char).to_digit(16).unwrap_or(0) as u8;
                v.push(Jeton::Hex(chiffres.chunks(2).map(|p| hex(p[0]) * 16 + hex(p[1])).collect()));
            }
            b'[' => {
                v.push(Jeton::Ouvre);
                i += 1;
            }
            b']' => {
                v.push(Jeton::Ferme);
                i += 1;
            }
            b'(' => {
                // Une chaîne littérale : ignorée (noms de CMap, registre).
                let mut prof = 0;
                while i < octets.len() {
                    match octets[i] {
                        b'\\' => i += 1,
                        b'(' => prof += 1,
                        b')' => {
                            prof -= 1;
                            if prof == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
                i += 1;
            }
            c if c.is_ascii_whitespace() => i += 1,
            _ => {
                let debut = i;
                while i < octets.len()
                    && !octets[i].is_ascii_whitespace()
                    && !matches!(octets[i], b'<' | b'>' | b'[' | b']' | b'(' | b'%')
                {
                    i += 1;
                }
                v.push(Jeton::Mot(String::from_utf8_lossy(&octets[debut..i]).into_owned()));
            }
        }
    }
    v
}

fn entier(octets: &[u8]) -> u32 {
    octets.iter().take(4).fold(0u32, |a, b| (a << 8) | *b as u32)
}

/// UTF-16BE (ou un seul octet : Latin-1) d'une destination de `ToUnicode`.
fn utf16(octets: &[u8]) -> String {
    if octets.len() == 1 {
        return (octets[0] as char).to_string();
    }
    let unites: Vec<u16> = octets.chunks(2).map(|p| ((p[0] as u16) << 8) | *p.get(1).unwrap_or(&0) as u16).collect();
    String::from_utf16_lossy(&unites)
}

/// Une table CMap lue : espaces de codes, `ToUnicode` (code -> texte) et `cid` (code -> CID).
#[derive(Default)]
struct Cmap {
    espaces: Vec<(usize, u32, u32)>,
    unicode: HashMap<u32, String>,
    cid: HashMap<u32, u32>,
    plages_cid: Vec<(u32, u32, u32)>,
}

fn lire_cmap(octets: &[u8]) -> Cmap {
    let t = jetons(octets);
    let mut c = Cmap::default();
    let mut i = 0;
    while i < t.len() {
        let mot = match &t[i] {
            Jeton::Mot(m) => m.as_str(),
            _ => {
                i += 1;
                continue;
            }
        };
        i += 1;
        match mot {
            "begincodespacerange" => {
                while let (Some(Jeton::Hex(a)), Some(Jeton::Hex(b))) = (t.get(i), t.get(i + 1)) {
                    c.espaces.push((a.len(), entier(a), entier(b)));
                    i += 2;
                }
            }
            "beginbfchar" => {
                while let (Some(Jeton::Hex(a)), Some(Jeton::Hex(b))) = (t.get(i), t.get(i + 1)) {
                    c.unicode.insert(entier(a), utf16(b));
                    i += 2;
                }
            }
            "beginbfrange" => {
                while let (Some(Jeton::Hex(a)), Some(Jeton::Hex(b))) = (t.get(i), t.get(i + 1)) {
                    let (debut, fin) = (entier(a), entier(b));
                    i += 2;
                    match t.get(i) {
                        Some(Jeton::Hex(d)) => {
                            i += 1;
                            // La destination est celle du premier code ; chacun des suivants
                            // avance son dernier caractère d'autant.
                            let chars: Vec<char> = utf16(d).chars().collect();
                            for k in 0..=fin.saturating_sub(debut).min(0xFFFF) {
                                if let Some(dernier) = chars.last() {
                                    let mut s = chars.clone();
                                    let n = (*dernier as u32).wrapping_add(k);
                                    *s.last_mut().unwrap() = char::from_u32(n).unwrap_or('\u{FFFD}');
                                    c.unicode.insert(debut + k, s.into_iter().collect());
                                }
                            }
                        }
                        Some(Jeton::Ouvre) => {
                            i += 1;
                            let mut k = 0;
                            while let Some(Jeton::Hex(d)) = t.get(i) {
                                c.unicode.insert(debut + k, utf16(d));
                                k += 1;
                                i += 1;
                            }
                            if t.get(i) == Some(&Jeton::Ferme) {
                                i += 1;
                            }
                        }
                        _ => break,
                    }
                }
            }
            "begincidchar" => {
                while let (Some(Jeton::Hex(a)), Some(Jeton::Mot(n))) = (t.get(i), t.get(i + 1)) {
                    c.cid.insert(entier(a), n.parse().unwrap_or(0));
                    i += 2;
                }
            }
            "begincidrange" => {
                while let (Some(Jeton::Hex(a)), Some(Jeton::Hex(b)), Some(Jeton::Mot(n))) =
                    (t.get(i), t.get(i + 1), t.get(i + 2))
                {
                    c.plages_cid.push((entier(a), entier(b), n.parse().unwrap_or(0)));
                    i += 3;
                }
            }
            _ => {}
        }
    }
    c
}

// ---------------------------------------------------------------------------------------------
// Les polices.
// ---------------------------------------------------------------------------------------------

/// Ce que l'interpréteur sait d'une police : décoder une chaîne en codes, et dire pour chaque code
/// son texte et sa largeur (millièmes d'em).
struct Police {
    composee: bool,
    /// Polices composées : les espaces de codes (longueur en octets, plus petit, plus grand), et
    /// la table des CID d'un CMap incorporé (`None` : Identity).
    espaces: Vec<(usize, u32, u32)>,
    cmap_cid: Option<Cmap>,
    unicode: HashMap<u32, String>,
    /// Polices simples : le caractère de chaque code d'après l'encodage.
    encodage: Vec<Option<char>>,
    largeurs: HashMap<u32, f64>,
    largeur_defaut: f64,
    /// Largeurs tirées de Liberation pour une police standard sans `/Widths`.
    standard: Option<crate::polices::Metrique>,
    vertical: bool,
    /// Ascendant et descendant (fractions du corps), qui bornent la boîte d'un glyphe pour
    /// `--inspecter` : ceux du fichier de police incorporé, sinon (polices standard) ceux de MuPDF,
    /// sinon ceux du `/FontDescriptor`, sinon 0,8 et 0,2. Laissés à 0,8 et 0,2 pour `--texte`.
    asc: f64,
    desc: f64,
}

/// Une boîte (x0, y0, x1, y1), dans le repère de la page du PDF (origine en bas à gauche).
pub(crate) type Boite = [f64; 4];

/// La boîte englobante du rectangle (x0, y0)-(x1, y1) transformé par `m`.
fn englobante(m: &Mat, x0: f64, y0: f64, x1: f64, y1: f64) -> Boite {
    let p = [point(m, x0, y0), point(m, x1, y0), point(m, x0, y1), point(m, x1, y1)];
    let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for (x, y) in p {
        b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
    }
    b
}

/// L'ascendant et le descendant d'une police (fractions du corps). Comme MuPDF : ceux du fichier de
/// police incorporé (`hhea`) ; pour une police standard non incorporée, ceux des polices de
/// remplacement de MuPDF (mesurés) ; sinon le `/FontDescriptor` ; sinon 0,8 et 0,2.
fn ascendant_descendant(doc: &Document, fd: Option<&Dictionary>, base_font: &str) -> (f64, f64) {
    let fichier = fd.and_then(|d| {
        [b"FontFile2".as_slice(), b"FontFile3", b"FontFile"].iter().find_map(|cle| d.get(cle).ok().map(|o| suivre(doc, o)))
    });
    if let Some(flux) = fichier {
        let octets = contenu_du_flux(flux);
        if let Ok(face) = ttf_parser::Face::parse(&octets, 0) {
            let em = face.units_per_em() as f64;
            let (a, d) = (face.ascender() as f64 / em, -(face.descender() as f64) / em);
            if a > 0.0 && a <= 2.0 && (0.0..=1.0).contains(&d) {
                return (a, d);
            }
        }
    } else {
        let n = base_font.to_ascii_lowercase();
        let n = n.rsplit('+').next().unwrap_or("");
        let gras = n.contains("bold");
        if n.contains("helvetica") || n == "arial" || n == "arialmt" || n.starts_with("arial,") || n.starts_with("arial-") {
            return if gras { (1.07, 0.307) } else { (1.075, 0.299) };
        }
        if n.contains("times") {
            return if gras { (1.044, 0.341) } else { (1.053, 0.281) };
        }
        if n.contains("courier") {
            return (0.932, 0.317);
        }
    }
    let lire = |cle: &[u8]| fd.and_then(|d| d.get(cle).ok()).and_then(|o| nombre(doc, o));
    match (lire(b"Ascent"), lire(b"Descent")) {
        (Some(a), Some(d)) if a > 0.0 && a <= 1500.0 && d.abs() <= 1000.0 => (a / 1000.0, d.abs() / 1000.0),
        _ => (0.8, 0.2),
    }
}

impl Police {
    /// Les codes d'une chaîne : (code, nombre d'octets).
    fn codes(&self, s: &[u8]) -> Vec<(u32, usize)> {
        if !self.composee {
            return s.iter().map(|b| (*b as u32, 1)).collect();
        }
        let mut v = Vec::new();
        let mut i = 0;
        while i < s.len() {
            let mut pris = None;
            for n in 1..=4usize.min(s.len() - i) {
                let code = entier(&s[i..i + n]);
                if self.espaces.iter().any(|(l, a, b)| *l == n && code >= *a && code <= *b) {
                    pris = Some((code, n));
                    break;
                }
            }
            // Hors de tout espace de codes : on consomme l'octet-plus-petit de l'espace le plus court.
            let (code, n) = pris.unwrap_or_else(|| {
                let n = self.espaces.iter().map(|e| e.0).min().unwrap_or(2).min(s.len() - i);
                (entier(&s[i..i + n]), n)
            });
            v.push((code, n));
            i += n;
        }
        v
    }

    fn cid(&self, code: u32) -> u32 {
        match &self.cmap_cid {
            None => code,
            Some(c) => {
                if let Some(v) = c.cid.get(&code) {
                    return *v;
                }
                c.plages_cid
                    .iter()
                    .find(|(a, b, _)| code >= *a && code <= *b)
                    .map(|(a, _, n)| n + (code - a))
                    .unwrap_or(0)
            }
        }
    }

    fn texte(&self, code: u32) -> String {
        if let Some(t) = self.unicode.get(&code) {
            return t.clone();
        }
        if self.composee {
            return "\u{FFFD}".to_string();
        }
        match self.encodage.get(code as usize).copied().flatten() {
            Some(c) => c.to_string(),
            None => String::new(),
        }
    }

    /// La largeur du code, en fraction d'em.
    fn largeur(&self, code: u32) -> f64 {
        let cle = if self.composee { self.cid(code) } else { code };
        if let Some(w) = self.largeurs.get(&cle) {
            return w / 1000.0;
        }
        if let Some(m) = &self.standard {
            if let Some(c) = self.encodage.get(code as usize).copied().flatten() {
                if let Some(w) = m.largeur(c) {
                    return w;
                }
            }
        }
        self.largeur_defaut / 1000.0
    }
}

fn nom_de_glyphe(n: &str) -> Option<char> {
    if let Ok(i) = GLYPHES.binary_search_by(|(g, _)| (*g).cmp(n)) {
        return char::from_u32(GLYPHES[i].1 as u32);
    }
    if let Some(h) = n.strip_prefix("uni") {
        if h.len() >= 4 {
            if let Ok(v) = u32::from_str_radix(&h[..4], 16) {
                return char::from_u32(v);
            }
        }
    }
    if let Some(h) = n.strip_prefix('u') {
        if (4..=6).contains(&h.len()) {
            if let Ok(v) = u32::from_str_radix(h, 16) {
                return char::from_u32(v);
            }
        }
    }
    None
}

fn base_encodage(nom: &str) -> Vec<Option<char>> {
    (0..=255u8)
        .map(|b| {
            // WinAnsi met un trait d'union au code 0xAD ; MuPDF, lui, y lit le trait d'union
            // conditionnel (U+00AD) de Windows-1252, et c'est ce qui se retrouve dans son texte.
            if b == 0xAD && nom == "WinAnsiEncoding" {
                return Some('\u{AD}');
            }
            let s = Document::decode_text(Some(nom), &[b]);
            s.chars().next()
        })
        .collect()
}

/// Le `/BaseFont` d'une police (vide s'il n'y en a pas).
pub(crate) fn nom_de_base(doc: &Document, d: &Dictionary) -> String {
    d.get(b"BaseFont").ok().and_then(|o| nom(doc, o)).map(|n| String::from_utf8_lossy(n).into_owned()).unwrap_or_default()
}

fn charger_police(doc: &Document, d: &Dictionary, inspecter: bool) -> Police {
    let sous_type = d.get(b"Subtype").ok().and_then(|o| nom(doc, o)).unwrap_or(b"");
    let composee = sous_type == b"Type0";
    let mut p = Police {
        composee,
        espaces: vec![(2, 0, 0xFFFF)],
        cmap_cid: None,
        unicode: HashMap::new(),
        encodage: Vec::new(),
        largeurs: HashMap::new(),
        largeur_defaut: 0.0,
        standard: None,
        vertical: false,
        asc: 0.8,
        desc: 0.2,
    };
    if let Some(o) = d.get(b"ToUnicode").ok() {
        let c = lire_cmap(&contenu_du_flux(suivre(doc, o)));
        p.unicode = c.unicode;
    }
    if composee {
        match d.get(b"Encoding").ok().map(|o| suivre(doc, o)) {
            Some(Object::Name(n)) => p.vertical = n.ends_with(b"-V"),
            Some(s @ Object::Stream(_)) => {
                let c = lire_cmap(&contenu_du_flux(s));
                if !c.espaces.is_empty() {
                    p.espaces = c.espaces.clone();
                }
                p.cmap_cid = Some(c);
            }
            _ => {}
        }
        let desc = d
            .get(b"DescendantFonts")
            .ok()
            .and_then(|o| match suivre(doc, o) {
                Object::Array(v) => v.first(),
                _ => None,
            })
            .and_then(|o| dico(doc, o));
        if let Some(desc) = desc {
            if inspecter {
                let fd = desc.get(b"FontDescriptor").ok().and_then(|o| dico(doc, o));
                (p.asc, p.desc) = ascendant_descendant(doc, fd, &nom_de_base(doc, d));
            }
            p.largeur_defaut = desc.get(b"DW").ok().and_then(|o| nombre(doc, o)).unwrap_or(1000.0);
            if let Some(Object::Array(w)) = desc.get(b"W").ok().map(|o| suivre(doc, o)) {
                let mut i = 0;
                while i < w.len() {
                    let premier = match nombre(doc, &w[i]) {
                        Some(n) => n as u32,
                        None => break,
                    };
                    match w.get(i + 1).map(|o| suivre(doc, o)) {
                        Some(Object::Array(liste)) => {
                            for (k, x) in liste.iter().enumerate() {
                                if let Some(v) = nombre(doc, x) {
                                    p.largeurs.insert(premier + k as u32, v);
                                }
                            }
                            i += 2;
                        }
                        Some(_) => {
                            let dernier = w.get(i + 1).and_then(|o| nombre(doc, o)).unwrap_or(0.0) as u32;
                            let v = w.get(i + 2).and_then(|o| nombre(doc, o)).unwrap_or(0.0);
                            for c in premier..=dernier.min(premier + 0xFFFF) {
                                p.largeurs.insert(c, v);
                            }
                            i += 3;
                        }
                        None => break,
                    }
                }
            }
        }
        return p;
    }

    // Police simple : l'encodage.
    if inspecter {
        (p.asc, p.desc) = ascendant_descendant(doc, d.get(b"FontDescriptor").ok().and_then(|o| dico(doc, o)), &nom_de_base(doc, d));
    }
    let base_nom = d.get(b"BaseFont").ok().and_then(|o| nom(doc, o)).map(|n| String::from_utf8_lossy(n).into_owned());
    let mut base: Option<String> = None;
    let mut differences: Vec<(usize, char)> = Vec::new();
    match d.get(b"Encoding").ok().map(|o| suivre(doc, o)) {
        Some(Object::Name(n)) => base = Some(String::from_utf8_lossy(n).into_owned()),
        Some(Object::Dictionary(e)) => {
            if let Some(n) = e.get(b"BaseEncoding").ok().and_then(|o| nom(doc, o)) {
                base = Some(String::from_utf8_lossy(n).into_owned());
            }
            if let Some(Object::Array(v)) = e.get(b"Differences").ok().map(|o| suivre(doc, o)) {
                let mut code = 0usize;
                for x in v {
                    match suivre(doc, x) {
                        Object::Integer(n) => code = *n as usize,
                        Object::Real(n) => code = *n as usize,
                        Object::Name(g) => {
                            if let Some(c) = nom_de_glyphe(&String::from_utf8_lossy(g)) {
                                differences.push((code, c));
                            }
                            code += 1;
                        }
                        _ => {}
                    }
                }
            }
        }
        _ => {}
    }
    let base = base.unwrap_or_else(|| {
        if sous_type == b"TrueType" { "WinAnsiEncoding" } else { "StandardEncoding" }.to_string()
    });
    p.encodage = base_encodage(&base);
    for (code, c) in differences {
        if code < 256 {
            p.encodage[code] = Some(c);
        }
    }

    // Les largeurs.
    let premier = d.get(b"FirstChar").ok().and_then(|o| nombre(doc, o)).unwrap_or(0.0) as u32;
    let mut a_des_largeurs = false;
    if let Some(Object::Array(w)) = d.get(b"Widths").ok().map(|o| suivre(doc, o)) {
        a_des_largeurs = true;
        for (k, x) in w.iter().enumerate() {
            if let Some(v) = nombre(doc, x) {
                p.largeurs.insert(premier + k as u32, v);
            }
        }
    }
    p.largeur_defaut = d
        .get(b"FontDescriptor")
        .ok()
        .and_then(|o| dico(doc, o))
        .and_then(|fd| fd.get(b"MissingWidth").ok())
        .and_then(|o| nombre(doc, o))
        .unwrap_or(0.0);
    if !a_des_largeurs {
        p.standard = crate::polices::Metrique::pour(base_nom.as_deref().unwrap_or(""));
    }
    p
}

// ---------------------------------------------------------------------------------------------
// La mise en lignes (la règle de MuPDF).
// ---------------------------------------------------------------------------------------------

// Les seuils sont ceux de MuPDF, retrouvés sur 370 000 couples de caractères consécutifs de nos
// PDF (aucun couple mal classé) : un écart se mesure en corps de la police du caractère posé.

/// Au-dessous de cette avance, le caractère suivant colle au précédent.
const ESPACE_MIN: f64 = 0.15;
/// Entre `ESPACE_MIN` et celle-ci, un espace est inséré ; au-delà, c'est une autre ligne. Un recul
/// de plus que cela (caractère posé avant le bout du précédent) ouvre aussi une ligne.
const ESPACE_MAX: f64 = 0.8;
/// Écart de ligne de base toléré (en corps) pour rester sur la même ligne : les exposants
/// (« 1er ») et les indices restent dans la ligne.
const DECALAGE_MAX: f64 = 0.8;

struct LigneCourante {
    dir: (f64, f64),
    texte: String,
    /// Une boîte par caractère de `texte` (`None` : un espace ajouté), si `Mise::garder`.
    boites: Vec<Option<Boite>>,
}

#[derive(Default)]
struct Mise {
    lignes: Vec<String>,
    /// Garder la boîte de chaque caractère (`--inspecter`) : `boites[k]` est celle de `lignes[k]`.
    garder: bool,
    boites: Vec<Vec<Option<Boite>>>,
    courante: Option<LigneCourante>,
    /// Le bout du dernier caractère posé.
    stylo: (f64, f64),
}

impl Mise {
    fn fermer(&mut self) {
        if let Some(l) = self.courante.take() {
            self.lignes.push(l.texte);
            if self.garder {
                self.boites.push(l.boites);
            }
        }
    }

    /// Pose un caractère : origine `p`, vecteur de direction `dir` (déjà à l'échelle du corps),
    /// avance `adv` (fraction d'em).
    fn caractere(&mut self, c: char, p: (f64, f64), dir: (f64, f64), taille: f64, adv: Option<f64>, boite: Boite) {
        let longueur = dir.0.hypot(dir.1);
        if longueur == 0.0 || taille == 0.0 {
            return;
        }
        let nd = (dir.0 / longueur, dir.1 / longueur);
        let mut nouvelle = true;
        let mut espace = false;
        if let Some(l) = &self.courante {
            if (nd.0 - l.dir.0).abs() + (nd.1 - l.dir.1).abs() < 0.05 {
                let d = (p.0 - self.stylo.0, p.1 - self.stylo.1);
                let le_long = d.0 * l.dir.0 + d.1 * l.dir.1;
                let en_travers = -d.0 * l.dir.1 + d.1 * l.dir.0;
                let ecart = le_long / taille;
                if en_travers.abs() <= taille * DECALAGE_MAX {
                    if ecart >= -ESPACE_MAX && ecart < ESPACE_MIN {
                        nouvelle = false;
                    } else if ecart >= ESPACE_MIN && ecart < ESPACE_MAX {
                        nouvelle = false;
                        espace = true;
                    }
                }
            }
        }
        if nouvelle {
            self.fermer();
            self.courante = Some(LigneCourante { dir: nd, texte: String::new(), boites: Vec::new() });
        }
        let garder = self.garder;
        let l = self.courante.as_mut().unwrap();
        if espace && !l.texte.ends_with(' ') && c != ' ' {
            l.texte.push(' ');
            if garder {
                l.boites.push(None);
            }
        }
        l.texte.push(c);
        if garder {
            l.boites.push(Some(boite));
        }
        // Le 2e caractère d'un glyphe qui en vaut plusieurs (« fi ») n'avance pas le stylo.
        if let Some(adv) = adv {
            self.stylo = (p.0 + adv * dir.0, p.1 + adv * dir.1);
        }
    }

    /// Les lignes avec la boîte de chacun de leurs caractères (`Mise::garder`).
    fn finir_avec_boites(mut self) -> Vec<(String, Vec<Option<Boite>>)> {
        self.fermer();
        self.lignes.into_iter().zip(self.boites).collect()
    }

    fn finir(mut self) -> String {
        self.fermer();
        let mut t = String::new();
        for l in self.lignes {
            t.push_str(&l);
            t.push('\n');
        }
        t
    }
}

// ---------------------------------------------------------------------------------------------
// L'interpréteur.
// ---------------------------------------------------------------------------------------------

#[derive(Clone)]
struct Etat {
    ctm: Mat,
    tc: f64,
    tw: f64,
    th: f64,
    tl: f64,
    police: Option<Rc<Police>>,
    taille: f64,
    rendu: i64,
    rise: f64,
}

struct Interprete<'a> {
    doc: &'a Document,
    polices: HashMap<ObjectId, Rc<Police>>,
    mise: Mise,
    /// Le cadre de la page (x0, y0, x1, y1) : le texte tout entier hors de lui n'est pas lu.
    cadre: Option<[f64; 4]>,
    /// Le `/ActualText` en cours : son texte, le premier glyphe (origine, direction, corps) et
    /// l'avance totale des glyphes qu'il remplace.
    actuel: Option<Actuel>,
    /// Ce que `--inspecter` relève en plus (images, chemins peints, polices) ; `None` pour `--texte`.
    releve: Option<Releve>,
    /// Vrai pendant la lecture des apparences d'annotations (leurs polices ne sont pas relevées).
    annotation: bool,
}

/// Ce que l'interpréteur relève pour `--inspecter`, dans le repère de la page du PDF.
#[derive(Default)]
pub(crate) struct Releve {
    pub images: Vec<Boite>,
    pub traits: Vec<Boite>,
    pub polices: Vec<String>,
    /// Les points du dernier chemin peint s'il n'a été que rempli (voir `peindre`).
    dernier_rempli: Option<Vec<(f64, f64)>>,
}

impl Releve {
    /// Un chemin peint : sa boîte (épaisseur du trait non comptée). Comme PyMuPDF, le tracé d'un
    /// chemin que l'opération de dessin précédente vient de remplir (même points) n'ajoute rien : les
    /// deux ne font qu'un seul dessin (« fs »).
    fn peindre(&mut self, points: &[(f64, f64)], trace: bool, rempli: bool) {
        let precedent = self.dernier_rempli.take();
        if points.len() < 2 {
            return;
        }
        if trace && precedent.as_deref() == Some(points) {
            return;
        }
        let mut b = [points[0].0, points[0].1, points[0].0, points[0].1];
        for p in points {
            b = [b[0].min(p.0), b[1].min(p.1), b[2].max(p.0), b[3].max(p.1)];
        }
        self.traits.push(b);
        if rempli {
            self.dernier_rempli = Some(points.to_vec());
        }
    }
}

struct Actuel {
    texte: String,
    premier: Option<((f64, f64), (f64, f64), f64)>,
    adv: f64,
    /// La matrice de rendu du premier glyphe, son ascendant et son descendant.
    trm: Mat,
    asc: f64,
    desc: f64,
}

/// Remplace chaque image en ligne (`BI … ID données EI`) par l'opérateur `PPBI`, que l'analyseur
/// de lopdf (qui ne connaît pas les données binaires) lit sans broncher. Sans `BI`, le flux est
/// rendu tel quel.
fn remplacer_images_en_ligne(flux: &[u8]) -> std::borrow::Cow<'_, [u8]> {
    if !flux.windows(2).any(|w| w == b"BI") {
        return std::borrow::Cow::Borrowed(flux);
    }
    let blanc = |c: u8| matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0c | 0);
    let delim = |c: u8| matches!(c, b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%');
    let mut sortie = Vec::with_capacity(flux.len());
    let mut i = 0;
    let mut copie = 0; // ce qui précède a été recopié
    while i < flux.len() {
        match flux[i] {
            b'%' => {
                while i < flux.len() && flux[i] != b'\n' && flux[i] != b'\r' {
                    i += 1;
                }
            }
            b'(' => {
                let mut prof = 0;
                while i < flux.len() {
                    match flux[i] {
                        b'\\' => i += 1,
                        b'(' => prof += 1,
                        b')' => {
                            prof -= 1;
                            if prof == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
                i += 1;
            }
            c if blanc(c) || delim(c) => i += 1,
            _ => {
                let debut = i;
                while i < flux.len() && !blanc(flux[i]) && !delim(flux[i]) {
                    i += 1;
                }
                if &flux[debut..i] != b"BI" {
                    continue;
                }
                // Les données commencent après « ID » et un blanc ; elles finissent à « EI ».
                let id = flux[i..].windows(3).position(|w| blanc(w[0]) && w[1] == b'I' && w[2] == b'D').map(|p| i + p + 3);
                let donnees = match id {
                    Some(d) => d + 1,
                    None => continue,
                };
                let fin = (donnees..flux.len().saturating_sub(1)).find(|&k| {
                    flux[k] == b'E'
                        && flux[k + 1] == b'I'
                        && (k == 0 || blanc(flux[k - 1]))
                        && flux.get(k + 2).map_or(true, |c| blanc(*c) || delim(*c))
                });
                if let Some(k) = fin {
                    sortie.extend_from_slice(&flux[copie..debut]);
                    sortie.extend_from_slice(b" PPBI ");
                    i = k + 2;
                    copie = i;
                }
            }
        }
    }
    sortie.extend_from_slice(&flux[copie.min(flux.len())..]);
    std::borrow::Cow::Owned(sortie)
}

/// Le texte d'une chaîne PDF : UTF-16BE s'il commence par la marque, sinon Latin-1 (approchant
/// PDFDocEncoding).
fn texte_pdf(s: &[u8]) -> String {
    if s.starts_with(&[0xFE, 0xFF]) {
        utf16(&s[2..])
    } else {
        s.iter().map(|b| *b as char).collect()
    }
}

impl<'a> Interprete<'a> {
    /// Pose le texte réel d'un `/ActualText` refermé, réparti sur la place des glyphes remplacés.
    fn finir_actuel(&mut self) {
        let a = match self.actuel.take() {
            Some(a) => a,
            None => return,
        };
        let ((x, y), dir, taille) = match a.premier {
            Some(p) => p,
            None => return,
        };
        let n = a.texte.chars().count().max(1) as f64;
        for (k, c) in a.texte.chars().enumerate() {
            let decalage = k as f64 * a.adv / n;
            let boite = englobante(&a.trm, decalage, -a.desc, decalage + a.adv / n, a.asc);
            self.mise.caractere(c, (x + decalage * dir.0, y + decalage * dir.1), dir, taille, Some(a.adv / n), boite);
        }
    }

    fn police(&mut self, res: &Dictionary, nom_police: &[u8]) -> Option<Rc<Police>> {
        let doc = self.doc;
        let polices = res.get(b"Font").ok().and_then(|o| dico(doc, o))?;
        let entree = polices.get(nom_police).ok()?;
        if let Object::Reference(id) = entree {
            if let Some(p) = self.polices.get(id) {
                return Some(p.clone());
            }
            let d = dico(doc, entree)?;
            let p = Rc::new(charger_police(doc, d, self.releve.is_some()));
            self.polices.insert(*id, p.clone());
            return Some(p);
        }
        let d = dico(doc, entree)?;
        Some(Rc::new(charger_police(doc, d, self.releve.is_some())))
    }

    /// Relève les `/BaseFont` des polices d'un dictionnaire de ressources (page ou formulaire suivi),
    /// dans l'ordre, sans doublon.
    fn relever_polices(&mut self, res: &Dictionary) {
        if self.releve.is_none() || self.annotation {
            return;
        }
        let doc = self.doc;
        let polices = match res.get(b"Font").ok().and_then(|o| dico(doc, o)) {
            Some(p) => p,
            None => return,
        };
        for (_, entree) in polices.iter() {
            let nom = dico(doc, entree).map(|d| nom_de_base(doc, d)).unwrap_or_default();
            if let Some(r) = self.releve.as_mut() {
                if !nom.is_empty() && !r.polices.contains(&nom) {
                    r.polices.push(nom);
                }
            }
        }
    }

    /// Le flux tel que l'analyseur le lira : pour `--inspecter`, les images en ligne remplacées.
    fn preparer<'f>(&self, flux: &'f [u8]) -> std::borrow::Cow<'f, [u8]> {
        if self.releve.is_some() {
            remplacer_images_en_ligne(flux)
        } else {
            std::borrow::Cow::Borrowed(flux)
        }
    }

    fn executer(&mut self, flux: &[u8], res: &Dictionary, ctm: Mat, profondeur: usize) {
        let flux = self.preparer(flux);
        let contenu = match Content::decode(&flux) {
            Ok(c) => c,
            Err(_) => return,
        };
        let mut e = Etat { ctm, tc: 0.0, tw: 0.0, th: 1.0, tl: 0.0, police: None, taille: 0.0, rendu: 0, rise: 0.0 };
        let mut pile: Vec<Etat> = Vec::new();
        let mut tm = ID;
        let mut tlm = ID;
        // Pour chaque contenu marqué ouvert : a-t-il ouvert un `/ActualText` ?
        let mut marques: Vec<bool> = Vec::new();
        // Le chemin en construction (ses points, dans le repère de la page), pour `--inspecter`.
        let mut points: Vec<(f64, f64)> = Vec::new();
        for op in &contenu.operations {
            let a = &op.operands;
            let num = |i: usize| a.get(i).and_then(|o| nombre(self.doc, o)).unwrap_or(0.0);
            match op.operator.as_str() {
                "q" => pile.push(e.clone()),
                "Q" => {
                    if let Some(s) = pile.pop() {
                        e = s;
                    }
                }
                "cm" => e.ctm = mul(&[num(0), num(1), num(2), num(3), num(4), num(5)], &e.ctm),
                "BT" => {
                    tm = ID;
                    tlm = ID;
                }
                "Tc" => e.tc = num(0),
                "Tw" => e.tw = num(0),
                "Tz" => e.th = num(0) / 100.0,
                "TL" => e.tl = num(0),
                "Tr" => e.rendu = num(0) as i64,
                "Ts" => e.rise = num(0),
                "Tf" => {
                    if let Some(Object::Name(n)) = a.first() {
                        e.police = self.police(res, n);
                    }
                    e.taille = num(1);
                }
                "Td" => {
                    tlm = mul(&[1.0, 0.0, 0.0, 1.0, num(0), num(1)], &tlm);
                    tm = tlm;
                }
                "TD" => {
                    e.tl = -num(1);
                    tlm = mul(&[1.0, 0.0, 0.0, 1.0, num(0), num(1)], &tlm);
                    tm = tlm;
                }
                "Tm" => {
                    tlm = [num(0), num(1), num(2), num(3), num(4), num(5)];
                    tm = tlm;
                }
                "T*" => {
                    tlm = mul(&[1.0, 0.0, 0.0, 1.0, 0.0, -e.tl], &tlm);
                    tm = tlm;
                }
                "Tj" | "'" | "\"" => {
                    if op.operator != "Tj" {
                        if op.operator == "\"" {
                            e.tw = num(0);
                            e.tc = num(1);
                        }
                        tlm = mul(&[1.0, 0.0, 0.0, 1.0, 0.0, -e.tl], &tlm);
                        tm = tlm;
                    }
                    if let Some(Object::String(s, _)) = a.last() {
                        self.montrer(&e, &mut tm, s);
                    }
                }
                "TJ" => {
                    if let Some(Object::Array(v)) = a.first() {
                        for x in v {
                            match x {
                                Object::String(s, _) => self.montrer(&e, &mut tm, s),
                                Object::Integer(_) | Object::Real(_) => {
                                    let n = nombre(self.doc, x).unwrap_or(0.0);
                                    let vertical = e.police.as_ref().map(|p| p.vertical).unwrap_or(false);
                                    let t = -n / 1000.0 * e.taille;
                                    let d = if vertical { [1.0, 0.0, 0.0, 1.0, 0.0, t] } else { [1.0, 0.0, 0.0, 1.0, t * e.th, 0.0] };
                                    tm = mul(&d, &tm);
                                }
                                _ => {}
                            }
                        }
                    }
                }
                // Le contenu marqué : un `/ActualText` remplace les glyphes qu'il enveloppe (MuPDF
                // fait de même : Word écrit ainsi ses espaces, par exemple).
                "BMC" => marques.push(false),
                "BDC" => {
                    let propres = match a.get(1) {
                        Some(Object::Name(n)) => res
                            .get(b"Properties")
                            .ok()
                            .and_then(|o| dico(self.doc, o))
                            .and_then(|p| p.get(n).ok())
                            .and_then(|o| dico(self.doc, o)),
                        Some(o) => dico(self.doc, o),
                        None => None,
                    };
                    let reel = propres.and_then(|d| d.get(b"ActualText").ok()).and_then(|o| match suivre(self.doc, o) {
                        Object::String(s, _) => Some(texte_pdf(s)),
                        _ => None,
                    });
                    match reel {
                        Some(t) if self.actuel.is_none() => {
                            self.actuel = Some(Actuel { texte: t, premier: None, adv: 0.0, trm: ID, asc: 0.8, desc: 0.2 });
                            marques.push(true);
                        }
                        _ => marques.push(false),
                    }
                }
                "EMC" => {
                    if marques.pop() == Some(true) {
                        self.finir_actuel();
                    }
                }
                // Les chemins : leur boîte, relevée quand ils sont peints.
                "m" | "l" => points.push(point(&e.ctm, num(0), num(1))),
                "c" => {
                    for k in 0..3 {
                        points.push(point(&e.ctm, num(2 * k), num(2 * k + 1)));
                    }
                }
                "v" | "y" => {
                    for k in 0..2 {
                        points.push(point(&e.ctm, num(2 * k), num(2 * k + 1)));
                    }
                }
                "re" => {
                    let (x, y, w, h) = (num(0), num(1), num(2), num(3));
                    for (px, py) in [(x, y), (x + w, y), (x + w, y + h), (x, y + h)] {
                        points.push(point(&e.ctm, px, py));
                    }
                }
                "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" => {
                    if let Some(r) = self.releve.as_mut() {
                        r.peindre(&points, matches!(op.operator.as_str(), "S" | "s"), matches!(op.operator.as_str(), "f" | "F" | "f*"));
                    }
                    points.clear();
                }
                "n" => points.clear(),
                // Une image en ligne (voir `remplacer_images_en_ligne`) : le carré unité sous la CTM.
                "PPBI" => {
                    if let Some(r) = self.releve.as_mut() {
                        r.images.push(englobante(&e.ctm, 0.0, 0.0, 1.0, 1.0));
                    }
                }
                "Do" => {
                    if profondeur < 12 {
                        if let Some(Object::Name(n)) = a.first() {
                            self.objet(res, n, &e.ctm, profondeur);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Un objet formulaire (`Do`) : son flux, sous sa matrice, avec ses ressources (ou celles de
    /// l'appelant).
    fn objet(&mut self, res: &Dictionary, nom_obj: &[u8], ctm: &Mat, profondeur: usize) {
        let doc = self.doc;
        let xo = match res.get(b"XObject").ok().and_then(|o| dico(doc, o)) {
            Some(x) => x,
            None => return,
        };
        let o = match xo.get(nom_obj) {
            Ok(o) => suivre(doc, o),
            Err(_) => return,
        };
        let s = match o {
            Object::Stream(s) => s,
            _ => return,
        };
        let sous_type = s.dict.get(b"Subtype").ok().and_then(|o| nom(doc, o));
        if sous_type == Some(b"Image") {
            if let Some(r) = self.releve.as_mut() {
                r.images.push(englobante(ctm, 0.0, 0.0, 1.0, 1.0));
            }
            return;
        }
        if sous_type != Some(b"Form") {
            return;
        }
        let m = matrice(doc, s.dict.get(b"Matrix").ok());
        let propres = s.dict.get(b"Resources").ok().and_then(|o| dico(doc, o)).cloned();
        if let Some(r) = &propres {
            self.relever_polices(r);
        }
        let res_form = propres.unwrap_or_else(|| res.clone());
        let flux = contenu_du_flux(o);
        self.executer(&flux, &res_form, mul(&m, ctm), profondeur + 1);
    }

    /// Montre une chaîne : un caractère par code, le texte avance de la largeur de chaque glyphe.
    fn montrer(&mut self, e: &Etat, tm: &mut Mat, chaine: &[u8]) {
        let police = match &e.police {
            Some(p) => p.clone(),
            None => return,
        };
        for (code, n) in police.codes(chaine) {
            let w0 = police.largeur(code);
            let trm = mul(&[e.taille * e.th, 0.0, 0.0, e.taille, 0.0, e.rise], &mul(tm, &e.ctm));
            let texte = police.texte(code);
            let visible = self.dans_le_cadre(&trm, w0);
            let boite = englobante(&trm, 0.0, -police.desc, w0, police.asc);
            if let Some(a) = &mut self.actuel {
                // Sous un `/ActualText` : on ne garde que la position du premier glyphe et
                // l'avance totale ; le texte réel se pose à la fermeture.
                if a.premier.is_none() {
                    let taille = (trm[0] * trm[3] - trm[1] * trm[2]).abs().sqrt();
                    a.premier = Some(((trm[4], trm[5]), (trm[0], trm[1]), taille));
                    (a.trm, a.asc, a.desc) = (trm, police.asc, police.desc);
                }
                a.adv += w0;
            } else if visible {
                let taille = (trm[0] * trm[3] - trm[1] * trm[2]).abs().sqrt();
                let dir = if police.vertical { (trm[2], trm[3]) } else { (trm[0], trm[1]) };
                for (k, c) in texte.chars().enumerate() {
                    self.mise.caractere(c, (trm[4], trm[5]), dir, taille, if k == 0 { Some(w0) } else { None }, boite);
                }
            }
            let mut tx = w0 * e.taille + e.tc;
            if n == 1 && code == 32 {
                tx += e.tw;
            }
            *tm = if police.vertical {
                mul(&[1.0, 0.0, 0.0, 1.0, 0.0, -tx.abs()], tm)
            } else {
                mul(&[1.0, 0.0, 0.0, 1.0, tx * e.th, 0.0], tm)
            };
        }
    }

    fn dans_le_cadre(&self, trm: &Mat, w0: f64) -> bool {
        let c = match &self.cadre {
            Some(c) => c,
            None => return true,
        };
        let p = (trm[4], trm[5]);
        let q = (p.0 + w0 * trm[0], p.1 + w0 * trm[1]);
        let (x0, x1) = (p.0.min(q.0), p.0.max(q.0));
        let (y0, y1) = (p.1.min(q.1) - trm[3].abs() * 0.25, p.1.max(q.1) + trm[3].abs() * 0.75);
        !(x1 < c[0] || x0 > c[2] || y1 < c[1] || y0 > c[3])
    }
}

// ---------------------------------------------------------------------------------------------
// Les pages.
// ---------------------------------------------------------------------------------------------

/// Un attribut hérité de l'arbre des pages.
pub(crate) fn herite<'a>(doc: &'a Document, page: ObjectId, cle: &[u8]) -> Option<&'a Object> {
    let mut courant = page;
    for _ in 0..64 {
        let d = doc.get_dictionary(courant).ok()?;
        if let Ok(v) = d.get(cle) {
            return Some(suivre(doc, v));
        }
        courant = d.get(b"Parent").ok()?.as_reference().ok()?;
    }
    None
}

pub(crate) fn cadre_de(doc: &Document, page: ObjectId) -> Option<[f64; 4]> {
    let v = nombres(doc, herite(doc, page, b"MediaBox"));
    if v.len() != 4 {
        return None;
    }
    let mut c = [v[0].min(v[2]), v[1].min(v[3]), v[0].max(v[2]), v[1].max(v[3])];
    let r = nombres(doc, herite(doc, page, b"CropBox"));
    if r.len() == 4 {
        c = [c[0].max(r[0].min(r[2])), c[1].max(r[1].min(r[3])), c[2].min(r[0].max(r[2])), c[3].min(r[1].max(r[3]))];
    }
    if c[2] <= c[0] || c[3] <= c[1] {
        return None;
    }
    Some(c)
}

/// Les flux de contenu d'une page, bout à bout (un saut de ligne entre deux : un opérateur ne
/// s'étend jamais sur deux flux). `/Contents` est un flux, un tableau de flux, ou une référence à
/// un tableau (ce que `Document::get_page_content` de lopdf ne lit pas).
pub(crate) fn contenu_de_page(doc: &Document, id: ObjectId) -> Vec<u8> {
    fn ramasser(doc: &Document, o: &Object, sortie: &mut Vec<u8>, prof: usize) {
        match suivre(doc, o) {
            s @ Object::Stream(_) => {
                sortie.extend_from_slice(&contenu_du_flux(s));
                sortie.push(b'\n');
            }
            Object::Array(v) if prof < 4 => v.iter().for_each(|x| ramasser(doc, x, sortie, prof + 1)),
            _ => {}
        }
    }
    let mut sortie = Vec::new();
    if let Some(c) = doc.get_dictionary(id).ok().and_then(|d| d.get(b"Contents").ok()) {
        ramasser(doc, c, &mut sortie, 0);
    }
    sortie
}

/// Lit une page : son contenu, puis l'apparence de ses annotations (champs de formulaire). Avec
/// `inspecter`, l'interpréteur garde en plus la boîte des caractères, les images, les chemins
/// peints et les polices.
fn lire_la_page(doc: &Document, id: ObjectId, inspecter: bool) -> Result<Interprete<'_>, String> {
    let res = herite(doc, id, b"Resources").and_then(|o| dico(doc, o)).cloned().unwrap_or_default();
    let mise = Mise { garder: inspecter, ..Mise::default() };
    let releve = if inspecter { Some(Releve::default()) } else { None };
    let mut it = Interprete { doc, polices: HashMap::new(), mise, cadre: cadre_de(doc, id), actuel: None, releve, annotation: false };
    it.relever_polices(&res);
    let flux = contenu_de_page(doc, id);
    if !flux.is_empty() {
        if Content::decode(&it.preparer(&flux)).is_err() {
            return Err("contenu de page illisible".to_string());
        }
        it.executer(&flux, &res, ID, 0);
    }
    // Les annotations (Popup, cachées ou sans apparence exclues).
    let annots: Vec<Object> = match doc.get_dictionary(id).ok().and_then(|d| d.get(b"Annots").ok()) {
        Some(o) => match suivre(doc, o) {
            Object::Array(v) => v.clone(),
            _ => Vec::new(),
        },
        None => Vec::new(),
    };
    it.annotation = true;
    for a in &annots {
        let d = match dico(doc, a) {
            Some(d) => d,
            None => continue,
        };
        let drapeaux = d.get(b"F").ok().and_then(|o| nombre(doc, o)).unwrap_or(0.0) as i64;
        if drapeaux & (2 | 32) != 0 || d.get(b"Subtype").ok().and_then(|o| nom(doc, o)) == Some(b"Popup") {
            continue;
        }
        let ap = match d.get(b"AP").ok().and_then(|o| dico(doc, o)).and_then(|ap| ap.get(b"N").ok()) {
            Some(n) => suivre(doc, n),
            None => continue,
        };
        // L'apparence : un flux, ou un dictionnaire d'états choisi par /AS.
        let flux_ap = match ap {
            Object::Stream(_) => ap,
            Object::Dictionary(etats) => {
                let etat = d.get(b"AS").ok().and_then(|o| nom(doc, o));
                match etat.and_then(|n| etats.get(n).ok()).map(|o| suivre(doc, o)) {
                    Some(s @ Object::Stream(_)) => s,
                    _ => continue,
                }
            }
            _ => continue,
        };
        let s = match flux_ap {
            Object::Stream(s) => s,
            _ => continue,
        };
        let rect = nombres(doc, d.get(b"Rect").ok());
        let bbox = nombres(doc, s.dict.get(b"BBox").ok());
        if rect.len() != 4 || bbox.len() != 4 {
            continue;
        }
        let m = matrice(doc, s.dict.get(b"Matrix").ok());
        let coins = [point(&m, bbox[0], bbox[1]), point(&m, bbox[2], bbox[1]), point(&m, bbox[0], bbox[3]), point(&m, bbox[2], bbox[3])];
        let (bx0, bx1) = coins.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.0), b.max(p.0)));
        let (by0, by1) = coins.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.1), b.max(p.1)));
        let (rx0, rx1) = (rect[0].min(rect[2]), rect[0].max(rect[2]));
        let (ry0, ry1) = (rect[1].min(rect[3]), rect[1].max(rect[3]));
        let sx = if bx1 > bx0 { (rx1 - rx0) / (bx1 - bx0) } else { 1.0 };
        let sy = if by1 > by0 { (ry1 - ry0) / (by1 - by0) } else { 1.0 };
        let a_m = [sx, 0.0, 0.0, sy, rx0 - bx0 * sx, ry0 - by0 * sy];
        let res_ap = s.dict.get(b"Resources").ok().and_then(|o| dico(doc, o)).cloned().unwrap_or_else(|| res.clone());
        let contenu = contenu_du_flux(flux_ap);
        it.executer(&contenu, &res_ap, mul(&m, &a_m), 1);
    }
    Ok(it)
}

/// Le texte d'une page : son contenu, puis l'apparence de ses annotations (champs de formulaire).
fn texte_de_page(doc: &Document, id: ObjectId) -> Result<String, String> {
    Ok(lire_la_page(doc, id, false)?.mise.finir())
}

/// Ce que `--inspecter` relève d'une page, dans le repère du PDF (origine en bas à gauche).
pub(crate) struct PageLue {
    /// Les lignes de texte (celles de `--texte`) avec la boîte de chacun de leurs caractères.
    pub lignes: Vec<(String, Vec<Option<Boite>>)>,
    pub images: Vec<Boite>,
    pub traits: Vec<Boite>,
    pub polices: Vec<String>,
}

pub(crate) fn inspecter_page(doc: &Document, id: ObjectId) -> Result<PageLue, String> {
    let it = lire_la_page(doc, id, true)?;
    let releve = it.releve.unwrap_or_default();
    Ok(PageLue { lignes: it.mise.finir_avec_boites(), images: releve.images, traits: releve.traits, polices: releve.polices })
}

/// Le texte de chaque page du document. Une page dont le contenu est illisible donne un texte vide
/// et un avertissement (rendu à l'appelant) : le reste du document se lit.
pub fn texte_des_pages(doc: &Document) -> (Vec<String>, Vec<String>) {
    let mut textes = Vec::new();
    let mut avertissements = Vec::new();
    for (n, id) in doc.get_pages() {
        match texte_de_page(doc, id) {
            Ok(t) => textes.push(t),
            Err(m) => {
                avertissements.push(format!("page {} : {}", n, m));
                textes.push(String::new());
            }
        }
    }
    (textes, avertissements)
}
