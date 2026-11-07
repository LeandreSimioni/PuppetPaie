//! Les formulaires AcroForm d'un PDF existant (FORMAT.md, « Remplir un formulaire ») :
//!
//! - `champs` (`fabrique-pdf --champs a.pdf`) : chaque champ, son nom complet, sa valeur, sa page
//!   et son rectangle — ce que `PdfReader(...).get_fields()` de PyPDF et `page.widgets()` de PyMuPDF
//!   rendaient ;
//! - `remplir` (`fabrique-pdf --remplir entree.pdf sortie.pdf`, JSON sur l'entrée standard) :
//!   écrit des champs texte avec une apparence (`/AP`) **régénérée** — le PDF se lit pareil dans
//!   tous les lecteurs, même ceux qui ne refont pas les apparences — et pose des images (SVG, PNG,
//!   JPEG) dans des rectangles de pages existantes, marges et proportions gardées.
//!
//! C'est ce que faisait PyMuPDF pour l'attestation employeur rematérialisée (AER) de France
//! Travail : remplir le cadre 5, poser la signature dans la case signature et le cachet dans la
//! case cachet.
//!
//! L'apparence d'un champ texte suit celle que MuPDF écrivait (mesurée sur les AER) : découpe à
//! 1 pt du bord, texte à 2 pt du bord, ligne de base à `(h − corps) / 2 + 0,2 corps` du bas pour
//! une ligne, cases en peigne (`Comb`) d'un caractère centré chacune, lignes de `1,116 corps` en
//! multiligne en partant du haut. La police est Liberation Sans (les métriques d'Helvetica)
//! embarquée en sous-ensemble : tout caractère qu'elle a s'écrit, et le texte se lit pareil
//! partout ; `/DA` n'est pas touché (un lecteur qui refait l'apparence garde la police du
//! formulaire).
//!
//! Les flux de contenu des pages ne sont pas touchés : une page qui reçoit une image voit son
//! contenu devenir `q`, les anciens flux, les `Q` qui les referment, puis le flux des images
//! (comme `--marquer`).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Write;

use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use serde::Serialize;

use crate::description::Refus;
use crate::forme::{AjoutDeChamps, Alignement, Remplissage};
use crate::marquage::{cadre, flux_existants, q_ouverts};
use crate::texte::{dico, herite, nombres};

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

/// Arrondi à deux décimales, pour le JSON rendu.
fn r2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn suivre<'a>(doc: &'a Document, o: &'a Object) -> &'a Object {
    let mut o = o;
    for _ in 0..32 {
        match o {
            Object::Reference(id) => match doc.objects.get(id) {
                Some(x) => o = x,
                None => return &Object::Null,
            },
            _ => return o,
        }
    }
    o
}

// ------------------------------------------------------------------ les chaînes de texte PDF

/// PDFDocEncoding : les octets 0x18–0x1F et 0x80–0xA0 qui diffèrent de Latin-1.
const PDFDOC: [(u8, char); 41] = [
    (0x18, '\u{02D8}'), (0x19, '\u{02C7}'), (0x1A, '\u{02C6}'), (0x1B, '\u{02D9}'),
    (0x1C, '\u{02DD}'), (0x1D, '\u{02DB}'), (0x1E, '\u{02DA}'), (0x1F, '\u{02DC}'),
    (0x80, '\u{2022}'), (0x81, '\u{2020}'), (0x82, '\u{2021}'), (0x83, '\u{2026}'),
    (0x84, '\u{2014}'), (0x85, '\u{2013}'), (0x86, '\u{0192}'), (0x87, '\u{2044}'),
    (0x88, '\u{2039}'), (0x89, '\u{203A}'), (0x8A, '\u{2212}'), (0x8B, '\u{2030}'),
    (0x8C, '\u{201E}'), (0x8D, '\u{201C}'), (0x8E, '\u{201D}'), (0x8F, '\u{2018}'),
    (0x90, '\u{2019}'), (0x91, '\u{201A}'), (0x92, '\u{2122}'), (0x93, '\u{FB01}'),
    (0x94, '\u{FB02}'), (0x95, '\u{0141}'), (0x96, '\u{0152}'), (0x97, '\u{0160}'),
    (0x98, '\u{0178}'), (0x99, '\u{017D}'), (0x9A, '\u{0131}'), (0x9B, '\u{0142}'),
    (0x9C, '\u{0153}'), (0x9D, '\u{0161}'), (0x9E, '\u{017E}'), (0x9F, '\u{FFFD}'),
    (0xA0, '\u{20AC}'),
];

/// Une chaîne de texte PDF en `String` : UTF-16BE si elle commence par la marque `FE FF`, UTF-8
/// si elle commence par `EF BB BF` (PDF 2.0), PDFDocEncoding sinon.
fn chaine_pdf(s: &[u8]) -> String {
    if let Some(r) = s.strip_prefix(&[0xFE, 0xFF]) {
        let unites: Vec<u16> = r.chunks(2).map(|c| u16::from_be_bytes([c[0], *c.get(1).unwrap_or(&0)])).collect();
        return String::from_utf16_lossy(&unites);
    }
    if let Some(r) = s.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(r).into_owned();
    }
    s.iter()
        .map(|&b| PDFDOC.iter().find(|(o, _)| *o == b).map(|(_, c)| *c).unwrap_or(b as char))
        .collect()
}

/// Une `String` en chaîne de texte PDF : PDFDocEncoding quand tous ses caractères y sont (ce
/// qu'écrivent les autres outils pour du français), UTF-16BE avec sa marque sinon.
fn vers_chaine_pdf(t: &str) -> Vec<u8> {
    let mut octets = Vec::with_capacity(t.len());
    for c in t.chars() {
        let direct = (c as u32) < 0x100
            && !(0x18..=0x1F).contains(&(c as u32))
            && !(0x7F..=0xA0).contains(&(c as u32));
        if direct {
            octets.push(c as u8);
        } else if let Some((o, _)) = PDFDOC.iter().find(|(_, x)| *x == c && c != '\u{FFFD}') {
            octets.push(*o);
        } else {
            let mut u = vec![0xFE, 0xFF];
            for unite in t.encode_utf16() {
                u.extend_from_slice(&unite.to_be_bytes());
            }
            return u;
        }
    }
    octets
}

// ------------------------------------------------------------------ l'arbre des champs

/// Un champ terminal du formulaire, avec ce qu'il hérite de ses parents.
struct Champ {
    /// Le nœud qui porte `/V` (le champ lui-même).
    id: ObjectId,
    nom: String,
    ft: Option<Vec<u8>>,
    ff: i64,
    v: Option<Object>,
    da: Option<Vec<u8>>,
    q: i64,
    maxlen: Option<i64>,
    /// Ses annotations (widgets) : lui-même s'il est fusionné avec son widget.
    widgets: Vec<ObjectId>,
}

#[derive(Clone, Default)]
struct Herite {
    ft: Option<Vec<u8>>,
    ff: Option<i64>,
    v: Option<Object>,
    da: Option<Vec<u8>>,
    q: Option<i64>,
    maxlen: Option<i64>,
}

fn entier(doc: &Document, o: Option<&Object>) -> Option<i64> {
    match o.map(|o| suivre(doc, o)) {
        Some(Object::Integer(i)) => Some(*i),
        Some(Object::Real(r)) => Some(*r as i64),
        _ => None,
    }
}

fn chaine_brute(doc: &Document, o: Option<&Object>) -> Option<Vec<u8>> {
    match o.map(|o| suivre(doc, o)) {
        Some(Object::String(s, _)) => Some(s.clone()),
        _ => None,
    }
}

/// Les champs terminaux du formulaire, dans l'ordre de `/Fields`. Un PDF sans formulaire n'en a
/// aucun.
fn lister(doc: &Document) -> (Vec<Champ>, Option<Vec<u8>>) {
    let catalogue = match doc.catalog() {
        Ok(c) => c,
        Err(_) => return (Vec::new(), None),
    };
    let acro = match catalogue.get(b"AcroForm").ok().and_then(|o| dico(doc, o)) {
        Some(a) => a,
        None => return (Vec::new(), None),
    };
    let da_formulaire = chaine_brute(doc, acro.get(b"DA").ok());
    let racines: Vec<Object> = match acro.get(b"Fields").ok().map(|o| suivre(doc, o)) {
        Some(Object::Array(v)) => v.clone(),
        _ => Vec::new(),
    };
    let mut out = Vec::new();
    let mut vus = BTreeSet::new();
    let base = Herite { da: da_formulaire.clone(), ..Herite::default() };
    for r in &racines {
        if let Object::Reference(id) = r {
            parcourir(doc, *id, "", &base, &mut out, &mut vus, 0);
        }
    }
    (out, da_formulaire)
}

fn parcourir(
    doc: &Document,
    id: ObjectId,
    parent: &str,
    herite: &Herite,
    out: &mut Vec<Champ>,
    vus: &mut BTreeSet<ObjectId>,
    prof: usize,
) {
    if prof > 32 || !vus.insert(id) {
        return;
    }
    let d = match doc.get_dictionary(id) {
        Ok(d) => d,
        Err(_) => return,
    };
    let partiel = chaine_brute(doc, d.get(b"T").ok()).map(|t| chaine_pdf(&t));
    let nom = match (&partiel, parent.is_empty()) {
        (Some(t), true) => t.clone(),
        (Some(t), false) => format!("{}.{}", parent, t),
        (None, _) => parent.to_string(),
    };
    let mut h = herite.clone();
    if let Ok(Object::Name(n)) = d.get(b"FT").map(|o| suivre(doc, o)) {
        h.ft = Some(n.clone());
    }
    if let Some(f) = entier(doc, d.get(b"Ff").ok()) {
        h.ff = Some(f);
    }
    if let Ok(v) = d.get(b"V") {
        h.v = Some(suivre(doc, v).clone());
    }
    if let Some(da) = chaine_brute(doc, d.get(b"DA").ok()) {
        h.da = Some(da);
    }
    if let Some(q) = entier(doc, d.get(b"Q").ok()) {
        h.q = Some(q);
    }
    if let Some(m) = entier(doc, d.get(b"MaxLen").ok()) {
        h.maxlen = Some(m);
    }
    let enfants: Vec<ObjectId> = match d.get(b"Kids").ok().map(|o| suivre(doc, o)) {
        Some(Object::Array(v)) => v.iter().filter_map(|o| o.as_reference().ok()).collect(),
        _ => Vec::new(),
    };
    // Les enfants qui ont un nom sont des champs ; les autres sont les widgets de celui-ci.
    let (champs_enfants, widgets): (Vec<ObjectId>, Vec<ObjectId>) = enfants
        .into_iter()
        .partition(|k| doc.get_dictionary(*k).map(|kd| kd.has(b"T")).unwrap_or(false));
    for k in &champs_enfants {
        parcourir(doc, *k, &nom, &h, out, vus, prof + 1);
    }
    let mut widgets = widgets;
    if champs_enfants.is_empty() && widgets.is_empty() {
        widgets.push(id); // champ et widget fusionnés
    }
    if !champs_enfants.is_empty() && widgets.is_empty() {
        return; // un nœud intermédiaire, pas un champ
    }
    out.push(Champ {
        id,
        nom,
        ft: h.ft,
        ff: h.ff.unwrap_or(0),
        v: h.v,
        da: h.da,
        q: h.q.unwrap_or(0),
        maxlen: h.maxlen,
        widgets,
    });
}

/// L'annotation → (numéro de page à partir de 1, page).
fn pages_des_annotations(doc: &Document) -> HashMap<ObjectId, (usize, ObjectId)> {
    let mut m = HashMap::new();
    for (n, id) in doc.get_pages() {
        let annots = doc.get_dictionary(id).ok().and_then(|d| d.get(b"Annots").ok()).map(|o| suivre(doc, o));
        if let Some(Object::Array(v)) = annots {
            for a in v {
                if let Object::Reference(r) = a {
                    m.entry(*r).or_insert((n as usize, id));
                }
            }
        }
    }
    m
}

/// Le rectangle d'un widget dans le repère de PyMuPDF : en points, origine en haut à gauche de la
/// page visible, y vers le bas.
fn rect_vu(doc: &Document, widget: ObjectId, page: ObjectId) -> Option<[f64; 4]> {
    let r = nombres(doc, doc.get_dictionary(widget).ok()?.get(b"Rect").ok());
    if r.len() != 4 {
        return None;
    }
    let c = cadre(doc, page).unwrap_or([0.0, 0.0, 595.0, 842.0]);
    let (x0, x1) = (r[0].min(r[2]), r[0].max(r[2]));
    let (y0, y1) = (r[1].min(r[3]), r[1].max(r[3]));
    Some([r2(x0 - c[0]), r2(c[3] - y1), r2(x1 - c[0]), r2(c[3] - y0)])
}

fn type_de(ft: &Option<Vec<u8>>) -> &'static str {
    match ft.as_deref() {
        Some(b"Tx") => "texte",
        Some(b"Btn") => "bouton",
        Some(b"Ch") => "choix",
        Some(b"Sig") => "signature",
        _ => "inconnu",
    }
}

/// La valeur d'un champ (`/V`), telle que le PDF la porte.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ValeurLue {
    /// Pas de `/V` (ou un objet qu'on ne lit pas).
    Rien,
    Texte(String),
    Entier(i64),
    Reel(f32),
    Booleen(bool),
    Liste(Vec<ValeurLue>),
}

impl ValeurLue {
    /// Le texte du champ, s'il en porte un.
    pub fn texte(&self) -> Option<&str> {
        match self {
            ValeurLue::Texte(t) => Some(t),
            _ => None,
        }
    }
}

/// Un champ du formulaire, tel que `--champs` le rend.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChampLu {
    /// Le nom complet (`parent.enfant`) : la clé de l'objet que rend `--champs`.
    #[serde(skip)]
    pub nom: String,
    pub valeur: ValeurLue,
    /// texte, bouton, choix, signature ou inconnu.
    #[serde(rename = "type")]
    pub genre: &'static str,
    pub lecture_seule: bool,
    /// La page de son premier widget (à partir de 1) ; aucune si aucune page ne le porte.
    pub page: Option<usize>,
    /// [x0, y0, x1, y1], origine en haut à gauche de la page visible.
    pub rect: Option<[f64; 4]>,
}

fn valeur_lue(doc: &Document, v: &Option<Object>) -> ValeurLue {
    match v.as_ref().map(|o| suivre(doc, o)) {
        None | Some(Object::Null) => ValeurLue::Rien,
        Some(Object::String(s, _)) => ValeurLue::Texte(chaine_pdf(s)),
        Some(Object::Name(n)) => ValeurLue::Texte(String::from_utf8_lossy(n).into_owned()),
        Some(Object::Integer(i)) => ValeurLue::Entier(*i),
        Some(Object::Real(r)) => ValeurLue::Reel(*r),
        Some(Object::Boolean(b)) => ValeurLue::Booleen(*b),
        Some(Object::Array(a)) => ValeurLue::Liste(a.iter().map(|x| valeur_lue(doc, &Some(x.clone()))).collect()),
        Some(_) => ValeurLue::Rien,
    }
}

/// La sortie de `--champs` : une ligne de JSON, `{"NOM": {"valeur", "type", "lecture_seule", "page", "rect"}}`,
/// les champs **dans l'ordre de `/AcroForm /Fields`**. Un PDF sans formulaire rend `{}`.
pub fn champs_json(octets: &[u8]) -> Result<String, Refus> {
    let mut morceaux: Vec<String> = Vec::new();
    for c in champs(octets)? {
        let nom = serde_json::to_string(&c.nom).map_err(|e| Refus(e.to_string()))?;
        let champ = serde_json::to_string(&c).map_err(|e| Refus(e.to_string()))?;
        morceaux.push(format!("{}:{}", nom, champ));
    }
    Ok(format!("{{{}}}", morceaux.join(",")))
}

/// Les champs, dans l'ordre de `/AcroForm /Fields` ; un nom porté par deux champs : le premier.
pub fn champs(octets: &[u8]) -> Result<Vec<ChampLu>, Refus> {
    let doc = crate::lecture::charger(octets)?;
    let doc = if doc.is_encrypted() {
        let mut d = doc;
        if d.decrypt("").is_err() {
            return refus("le PDF ne s'ouvre pas sans mot de passe");
        }
        d
    } else {
        doc
    };
    let (liste, _) = lister(&doc);
    let pages = pages_des_annotations(&doc);
    let mut out: Vec<ChampLu> = Vec::new();
    let mut vus = BTreeSet::new();
    for c in liste {
        let premier = c.widgets.iter().find_map(|w| pages.get(w).map(|p| (*w, *p)));
        let (page, rect) = match premier {
            Some((w, (n, page))) => (Some(n), rect_vu(&doc, w, page)),
            None => (None, None),
        };
        // Un nom porté par deux champs : le premier, comme PyPDF.
        if vus.insert(c.nom.clone()) {
            out.push(ChampLu {
                valeur: valeur_lue(&doc, &c.v),
                genre: type_de(&c.ft),
                lecture_seule: c.ff & 1 != 0,
                page,
                rect,
                nom: c.nom,
            });
        }
    }
    Ok(out)
}

// ------------------------------------------------------------------ l'apparence d'un champ texte

/// Ce que `/DA` dit : le corps (0 = automatique) et l'opérateur de couleur (`0 g`).
fn lire_da(da: &[u8]) -> (f64, String) {
    let texte = String::from_utf8_lossy(da);
    let jetons: Vec<&str> = texte.split_whitespace().collect();
    let mut corps = 0.0;
    let mut couleur = "0 g".to_string();
    for (i, j) in jetons.iter().enumerate() {
        let operandes = |n: usize| -> Option<Vec<f64>> {
            if i < n {
                return None;
            }
            jetons[i - n..i].iter().map(|x| x.parse::<f64>().ok()).collect()
        };
        match *j {
            "Tf" => {
                if let Some(v) = operandes(1) {
                    corps = v[0];
                }
            }
            "g" | "rg" | "k" => {
                let n = match *j {
                    "g" => 1,
                    "rg" => 3,
                    _ => 4,
                };
                if let Some(v) = operandes(n) {
                    couleur = format!("{} {}", v.iter().map(|x| nb(*x)).collect::<Vec<_>>().join(" "), j);
                }
            }
            _ => {}
        }
    }
    (corps, couleur)
}

const COMB: i64 = 1 << 24;
const MULTILIGNE: i64 = 1 << 12;
const MOT_DE_PASSE: i64 = 1 << 13;
/// L'interligne des champs multilignes, en corps (celui de MuPDF).
const INTERLIGNE: f64 = 1.116;

struct Metriques<'a> {
    face: ttf_parser::Face<'a>,
}

impl<'a> Metriques<'a> {
    fn largeur(&self, c: char, corps: f64) -> f64 {
        let em = self.face.units_per_em() as f64;
        self.face
            .glyph_index(c)
            .and_then(|g| self.face.glyph_hor_advance(g))
            .map(|a| a as f64 / em * corps)
            .unwrap_or(0.0)
    }
    fn largeur_texte(&self, t: &str, corps: f64) -> f64 {
        t.chars().map(|c| self.largeur(c, corps)).sum()
    }
}

fn hex(t: &str) -> String {
    t.chars().map(|c| format!("{:04X}", c as u32)).collect()
}

/// Les lignes d'un champ multiligne : les sauts de ligne du texte, puis des coupures aux espaces
/// pour tenir dans `large` (un mot plus long que la case se coupe où il déborde).
fn couper(m: &Metriques, texte: &str, corps: f64, large: f64) -> Vec<String> {
    let mut lignes = Vec::new();
    for paragraphe in texte.split('\n') {
        let mut courante = String::new();
        for mot in paragraphe.split(' ') {
            let essai = if courante.is_empty() { mot.to_string() } else { format!("{} {}", courante, mot) };
            if m.largeur_texte(&essai, corps) <= large || courante.is_empty() {
                courante = essai;
            } else {
                lignes.push(std::mem::take(&mut courante));
                courante = mot.to_string();
            }
            // Un mot seul trop large : coupé au caractère.
            while m.largeur_texte(&courante, corps) > large && courante.chars().count() > 1 {
                let mut tete = String::new();
                for c in courante.chars() {
                    if m.largeur_texte(&format!("{}{}", tete, c), corps) > large && !tete.is_empty() {
                        break;
                    }
                    tete.push(c);
                }
                let reste: String = courante.chars().skip(tete.chars().count()).collect();
                lignes.push(tete);
                courante = reste;
            }
        }
        lignes.push(courante);
    }
    lignes
}

/// Le flux de l'apparence d'un champ texte de `l` × `h` points.
fn flux_apparence(m: &Metriques, c: &Champ, texte: &str, l: f64, h: f64, police: &str) -> Vec<u8> {
    let (mut corps, couleur) = lire_da(c.da.as_deref().unwrap_or(b"0 g /Helv 0 Tf"));
    let multiligne = c.ff & MULTILIGNE != 0;
    let comb = c.ff & COMB != 0 && c.maxlen.unwrap_or(0) > 0 && !multiligne;
    if corps <= 0.0 {
        // Corps automatique : ce qui tient en hauteur (12 au plus), puis en largeur sur une ligne.
        corps = ((h - 4.0) / INTERLIGNE).clamp(4.0, 12.0);
        if !multiligne && !comb {
            let lt = m.largeur_texte(texte, corps);
            if lt > l - 4.0 && lt > 0.0 {
                corps = (corps * (l - 4.0) / lt).max(4.0);
            }
        }
    }
    let mut s = String::new();
    s.push_str("/Tx BMC\nq\n");
    s.push_str(&format!("1 1 {} {} re\nW\nn\nBT\n{}\n/{} {} Tf\n", nb(l - 2.0), nb(h - 2.0), couleur, police, nb(corps)));
    let base_une_ligne = (h - corps) / 2.0 + 0.2 * corps;
    if comb {
        let n = c.maxlen.unwrap_or(1) as f64;
        let case = l / n;
        for (i, ch) in texte.chars().enumerate() {
            let x = i as f64 * case + (case - m.largeur(ch, corps)) / 2.0;
            s.push_str(&format!("1 0 0 1 {} {} Tm\n<{}> Tj\n", nb(x), nb(base_une_ligne), hex(&ch.to_string())));
        }
    } else if multiligne {
        let lignes = couper(m, texte, corps, l - 4.0);
        for (i, ligne) in lignes.iter().enumerate() {
            let y = h - 2.0 - INTERLIGNE * corps * (i as f64 + 1.0);
            let x = aligner(c.q, m.largeur_texte(ligne, corps), l);
            s.push_str(&format!("1 0 0 1 {} {} Tm\n<{}> Tj\n", nb(x), nb(y), hex(ligne)));
        }
    } else {
        let x = aligner(c.q, m.largeur_texte(texte, corps), l);
        s.push_str(&format!("1 0 0 1 {} {} Tm\n<{}> Tj\n", nb(x), nb(base_une_ligne), hex(texte)));
    }
    s.push_str("ET\nQ\nEMC\n");
    s.into_bytes()
}

/// L'abscisse du texte selon `/Q` : 0 à gauche, 1 centré, 2 à droite (à 2 pt du bord).
fn aligner(q: i64, largeur: f64, l: f64) -> f64 {
    match q {
        1 => (l - largeur) / 2.0,
        2 => l - 2.0 - largeur,
        _ => 2.0,
    }
}

// ------------------------------------------------------------------ les images

/// Une image prête à poser : son XObject (dessiné dans le carré unité, comme une image PDF) et sa
/// taille naturelle (pixels, ou points du SVG), qui en donne les proportions.
struct Image {
    xobjet: ObjectId,
    largeur: f64,
    hauteur: f64,
}

/// Ce qu'est le fichier, d'après ses premiers octets (pas son extension).
fn sorte(octets: &[u8]) -> Option<&'static str> {
    if octets.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("png");
    }
    if octets.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("jpeg");
    }
    let tete = &octets[..octets.len().min(4096)];
    let tete = String::from_utf8_lossy(tete).to_ascii_lowercase();
    if tete.contains("<svg") {
        return Some("svg");
    }
    None
}

fn comprimer(donnees: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    let _ = e.write_all(donnees);
    e.finish().unwrap_or_default()
}

fn flux_image(largeur: u32, hauteur: u32, espace: &str, filtre: &str, donnees: Vec<u8>) -> Stream {
    let mut d = Dictionary::new();
    d.set("Type", Object::Name(b"XObject".to_vec()));
    d.set("Subtype", Object::Name(b"Image".to_vec()));
    d.set("Width", Object::Integer(largeur as i64));
    d.set("Height", Object::Integer(hauteur as i64));
    d.set("ColorSpace", Object::Name(espace.as_bytes().to_vec()));
    d.set("BitsPerComponent", Object::Integer(8));
    d.set("Filter", Object::Name(filtre.as_bytes().to_vec()));
    Stream::new(d, donnees).with_compression(false)
}

/// Des pixels bruts (8 bits), comprimés avec les prédicteurs PNG (`genpdfi::render::predict_image`).
fn flux_pixels(largeur: u32, hauteur: u32, espace: &str, pixels: Vec<u8>) -> Stream {
    let mut d = Dictionary::new();
    d.set("Type", Object::Name(b"XObject".to_vec()));
    d.set("Subtype", Object::Name(b"Image".to_vec()));
    d.set("Width", Object::Integer(largeur as i64));
    d.set("Height", Object::Integer(hauteur as i64));
    d.set("ColorSpace", Object::Name(espace.as_bytes().to_vec()));
    d.set("BitsPerComponent", Object::Integer(8));
    let mut s = Stream::new(d, pixels).with_compression(false);
    let _ = genpdfi::render::predict_image(&mut s);
    if s.dict.get(b"Filter").is_err() {
        let brut = std::mem::take(&mut s.content);
        s.dict.set("Filter", Object::Name(b"FlateDecode".to_vec()));
        s.set_content(comprimer(&brut));
    }
    s
}

/// Les composantes et la taille d'un JPEG, lues dans son en-tête de trame (SOFn).
fn entete_jpeg(o: &[u8]) -> Option<(u32, u32, u8)> {
    let mut i = 2;
    while i + 9 < o.len() {
        if o[i] != 0xFF {
            return None;
        }
        let marque = o[i + 1];
        if marque == 0xD8 || (0xD0..=0xD7).contains(&marque) || marque == 0x01 || marque == 0xFF {
            i += if marque == 0xFF { 1 } else { 2 };
            continue;
        }
        let longueur = u16::from_be_bytes([o[i + 2], o[i + 3]]) as usize;
        if matches!(marque, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF) {
            let h = u16::from_be_bytes([o[i + 5], o[i + 6]]) as u32;
            let l = u16::from_be_bytes([o[i + 7], o[i + 8]]) as u32;
            return Some((l, h, o[i + 9]));
        }
        i += 2 + longueur;
    }
    None
}

fn image_matricielle(doc: &mut Document, octets: &[u8], sorte: &str, nom: &str) -> Result<Image, Refus> {
    // Un JPEG en niveaux de gris ou en RVB part tel quel (DCTDecode) : pas de recompression.
    if sorte == "jpeg" {
        if let Some((l, h, n)) = entete_jpeg(octets) {
            if (n == 1 || n == 3) && l > 0 && h > 0 {
                let espace = if n == 1 { "DeviceGray" } else { "DeviceRGB" };
                let id = doc.add_object(Object::Stream(flux_image(l, h, espace, "DCTDecode", octets.to_vec())));
                return Ok(Image { xobjet: id, largeur: l as f64, hauteur: h as f64 });
            }
        }
    }
    let format = if sorte == "png" { image::ImageFormat::Png } else { image::ImageFormat::Jpeg };
    let img = image::load_from_memory_with_format(octets, format)
        .map_err(|e| Refus(format!("l'image {} est illisible ({})", nom, e)))?;
    let (l, h) = (img.width(), img.height());
    if l == 0 || h == 0 {
        return refus(format!("l'image {} est vide", nom));
    }
    let rgba = img.to_rgba8();
    let opaque = rgba.pixels().all(|p| p[3] == 255);
    let gris = img.color().channel_count() <= 2;
    let couleurs: Vec<u8> = if gris {
        rgba.pixels().map(|p| p[0]).collect()
    } else {
        rgba.pixels().flat_map(|p| [p[0], p[1], p[2]]).collect()
    };
    let espace = if gris { "DeviceGray" } else { "DeviceRGB" };
    let mut flux = flux_pixels(l, h, espace, couleurs);
    if !opaque {
        let alpha: Vec<u8> = rgba.pixels().map(|p| p[3]).collect();
        let masque = doc.add_object(Object::Stream(flux_pixels(l, h, "DeviceGray", alpha)));
        flux.dict.set("SMask", Object::Reference(masque));
    }
    let id = doc.add_object(Object::Stream(flux));
    Ok(Image { xobjet: id, largeur: l as f64, hauteur: h as f64 })
}

/// Un morceau de texte d'un SVG, posé tel quel en texte PDF : sa police (gras ou non), son corps, sa couleur et
/// ses caractères.
struct Morceau {
    gras: bool,
    corps: f64,
    couleur: [u8; 3],
    texte: String,
}

/// Une ligne de texte d'un SVG (un « chunk » d'usvg) : son point d'ancrage, son alignement et ses morceaux.
struct LigneSvg {
    matrice: [f64; 6],
    x: f64,
    y: f64,
    ancre: usvg::TextAnchor,
    morceaux: Vec<Morceau>,
}

/// Le texte d'un SVG, s'il s'écrit en texte PDF sans rien changer au dessin : Liberation Sans romain ou gras,
/// droit, sans contour, décoration, espacement, décalage ni position par caractère, d'une couleur pleine, et
/// la boîte de vue du SVG à sa taille. `None` dès qu'un texte ne s'y prête pas (il devient alors des tracés,
/// comme le reste du dessin).
fn textes_simples(arbre: &usvg::Tree) -> Option<Vec<(usvg::Node, Vec<LigneSvg>)>> {
    use usvg::NodeExt;
    let vb = arbre.view_box.rect;
    if vb.x() != 0.0 || vb.y() != 0.0 || vb.width() != arbre.size.width() || vb.height() != arbre.size.height() {
        return None;
    }
    let mut textes = Vec::new();
    for noeud in arbre.root.descendants() {
        let usvg::NodeKind::Text(ref t) = *noeud.borrow() else { continue };
        if t.writing_mode != usvg::WritingMode::LeftToRight
            || t.rotate.iter().any(|r| *r != 0.0)
            || t.positions.iter().any(|p| p.dx.is_some() || p.dy.is_some())
            || t.positions.iter().filter(|p| p.x.is_some() || p.y.is_some()).count() > t.chunks.len()
        {
            return None;
        }
        let m = noeud.abs_transform();
        let matrice = [m.sx as f64, m.ky as f64, m.kx as f64, m.sy as f64, m.tx as f64, m.ty as f64];
        let mut lignes = Vec::new();
        for c in &t.chunks {
            if !matches!(c.text_flow, usvg::TextFlow::Linear) {
                return None;
            }
            let mut morceaux = Vec::new();
            for s in &c.spans {
                let couleur = match &s.fill {
                    Some(usvg::Fill { paint: usvg::Paint::Color(c), opacity, .. }) if opacity.get() == 1.0 => [c.red, c.green, c.blue],
                    _ => return None,
                };
                let d = &s.decoration;
                if s.stroke.is_some()
                    || d.underline.is_some()
                    || d.overline.is_some()
                    || d.line_through.is_some()
                    || s.small_caps
                    || s.letter_spacing != 0.0
                    || s.word_spacing != 0.0
                    || s.text_length.is_some()
                    || !s.baseline_shift.is_empty()
                    || s.dominant_baseline != usvg::DominantBaseline::Auto
                    || s.alignment_baseline != usvg::AlignmentBaseline::Auto
                    || s.visibility != usvg::Visibility::Visible
                    || s.font.style != usvg::FontStyle::Normal
                {
                    return None;
                }
                // La famille que la base de polices retiendrait : la première qu'elle connaît (Liberation Sans
                // et `sans-serif` ici ; une autre famille de Liberation se dessine en tracés), Liberation Sans
                // sinon.
                for f in &s.font.families {
                    let f = f.to_ascii_lowercase();
                    if f == "liberation sans" || f == "sans-serif" {
                        break;
                    }
                    if f.starts_with("liberation") || matches!(f.as_str(), "serif" | "monospace" | "cursive" | "fantasy") {
                        return None;
                    }
                }
                let texte = c.text.get(s.start..s.end)?.to_string();
                if texte.chars().any(|ch| ch.is_control() || ch as u32 > 0xFFFF) {
                    return None;
                }
                morceaux.push(Morceau { gras: s.font.weight > 500, corps: s.font_size.get() as f64, couleur, texte });
            }
            lignes.push(LigneSvg { matrice, x: c.x.unwrap_or(0.0) as f64, y: c.y.unwrap_or(0.0) as f64, ancre: c.anchor, morceaux });
        }
        textes.push((noeud.clone(), lignes));
    }
    Some(textes)
}

/// Le texte `lignes` d'un SVG ajouté à son objet formulaire `forme` (écrit par svg2pdf, dans le repère du SVG
/// retourné, `hauteur` sa hauteur), en texte PDF avec les sous-ensembles de Liberation Sans de `doc`
/// (`police_pour`). Refus : un caractère que la police n'a pas.
fn ecrire_textes(doc: &mut Document, forme: ObjectId, lignes: &[LigneSvg], hauteur: f64, avant: Option<&Document>) -> Result<(), Refus> {
    let romain = crate::polices::sans_regulier()?;
    let gras = crate::polices::sans_gras()?;
    let faces = [
        ttf_parser::Face::parse(&romain, 0).map_err(|e| Refus(format!("police interne illisible : {:?}", e)))?,
        ttf_parser::Face::parse(&gras, 0).map_err(|e| Refus(format!("police interne illisible : {:?}", e)))?,
    ];
    let mut caracteres = [BTreeSet::new(), BTreeSet::new()];
    for m in lignes.iter().flat_map(|l| &l.morceaux) {
        for c in m.texte.chars() {
            if faces[m.gras as usize].glyph_index(c).is_none() {
                return refus(format!("le texte du SVG : le caractère « {} » (U+{:04X}) n'existe pas dans la police Liberation", c, c as u32));
            }
            caracteres[m.gras as usize].insert(c);
        }
    }
    let mut polices = Dictionary::new();
    let mut noms = ["", ""];
    for (i, (donnees, nom)) in [(&romain, "PPTexte"), (&gras, "PPTexteGras")].into_iter().enumerate() {
        if !caracteres[i].is_empty() {
            polices.set(nom, Object::Reference(police_pour(doc, donnees, &caracteres[i], avant)?));
            noms[i] = nom;
        }
    }
    let avance = |m: &Morceau| -> f64 {
        let f = &faces[m.gras as usize];
        let em = f.units_per_em() as f64;
        m.texte.chars().filter_map(|c| f.glyph_index(c)).map(|g| f.glyph_hor_advance(g).unwrap_or(0) as f64 / em * m.corps).sum()
    };
    let srgb = match doc.get_object(forme) {
        Ok(Object::Stream(s)) => s.dict.get(b"Resources").ok().and_then(|r| dico(doc, r)).and_then(|r| r.get(b"ColorSpace").ok()).and_then(|c| dico(doc, c)).map_or(false, |c| c.has(b"srgb")),
        _ => false,
    };
    // Le repère du SVG (y vers le bas) comme svg2pdf le pose, puis chaque ligne dans le sien.
    let mut s = format!("q\n1 0 0 -1 0 {} cm\n", nb(hauteur));
    for l in lignes {
        let largeur: f64 = l.morceaux.iter().map(|m| avance(m)).sum();
        let mut x = match l.ancre {
            usvg::TextAnchor::Start => l.x,
            usvg::TextAnchor::Middle => l.x - largeur / 2.0,
            usvg::TextAnchor::End => l.x - largeur,
        };
        let m = l.matrice;
        s.push_str(&format!("q\n{} {} {} {} {} {} cm\nBT\n", nb(m[0]), nb(m[1]), nb(m[2]), nb(m[3]), nb(m[4]), nb(m[5])));
        for mo in &l.morceaux {
            let [r, v, b] = mo.couleur.map(|c| nb(c as f64 / 255.0));
            if srgb {
                s.push_str(&format!("/srgb cs {} {} {} scn\n", r, v, b));
            } else {
                s.push_str(&format!("{} {} {} rg\n", r, v, b));
            }
            // Les glyphes debout dans le repère retourné : la matrice de texte retourne aussi.
            s.push_str(&format!("/{} {} Tf\n1 0 0 -1 {} {} Tm\n<{}> Tj\n", noms[mo.gras as usize], nb(mo.corps), nb(x), nb(l.y), hex(&mo.texte)));
            x += avance(mo);
        }
        s.push_str("ET\nQ\n");
    }
    s.push_str("Q\n");
    let mut ressources: Dictionary = match doc.get_object(forme) {
        Ok(Object::Stream(f)) => f.dict.get(b"Resources").ok().and_then(|r| dico(doc, r)).cloned().unwrap_or_default(),
        _ => return refus("le SVG converti n'a pas d'objet formulaire"),
    };
    ressources.set("Font", Object::Dictionary(polices));
    let Ok(Object::Stream(flux)) = doc.get_object_mut(forme) else {
        return refus("le SVG converti n'a pas d'objet formulaire");
    };
    let mut contenu = flux.decompressed_content().unwrap_or_else(|_| flux.content.clone());
    contenu.push(b'\n');
    contenu.extend_from_slice(s.as_bytes());
    flux.dict.set("Resources", Object::Dictionary(ressources));
    flux.dict.remove(b"Filter");
    flux.dict.remove(b"DecodeParms");
    flux.set_content(comprimer(&contenu));
    flux.dict.set("Filter", Object::Name(b"FlateDecode".to_vec()));
    Ok(())
}

/// Un SVG en objet formulaire PDF, **vectoriel** : usvg le lit (aucune police du système : même dessin partout ;
/// `Helvetica`, `Arial` et `sans-serif` sont Liberation Sans), svg2pdf l'écrit, et les objets écrits sont recopiés
/// dans le document. Son texte s'écrit en texte PDF quand il le peut (`textes_simples` : un sous-ensemble de
/// police par document, `police_pour`, quelques octets par caractère, et le texte se lit) ; sinon, il devient des
/// tracés (chaque lettre dessinée, des milliers de nombres). `avant` : voir `police_pour`.
fn image_svg(doc: &mut Document, octets: &[u8], nom: &str, avant: Option<&Document>) -> Result<Image, Refus> {
    use usvg::{TreeParsing, TreeTextToPath};
    let mut polices = usvg::fontdb::Database::new();
    for donnees in crate::polices::toutes()? {
        polices.load_font_data(donnees.as_ref().clone());
    }
    polices.set_sans_serif_family("Liberation Sans");
    polices.set_serif_family("Liberation Serif");
    polices.set_monospace_family("Liberation Mono");
    // Le SVG seul fait foi : une image qu'il citerait par un chemin n'est pas lue (une image
    // `data:` dans le SVG l'est).
    let options = usvg::Options {
        font_family: "Liberation Sans".to_string(),
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_string: Box::new(|_, _| None),
            ..usvg::ImageHrefResolver::default()
        },
        ..usvg::Options::default()
    };
    let mut arbre = usvg::Tree::from_data(octets, &options)
        .map_err(|e| Refus(format!("le SVG {} est illisible ({})", nom, e)))?;
    let textes = match textes_simples(&arbre) {
        Some(t) => {
            for (noeud, _) in &t {
                noeud.clone().detach();
            }
            t.into_iter().flat_map(|(_, l)| l).collect()
        }
        None => {
            arbre.convert_text(&polices);
            Vec::new()
        }
    };
    let (l, h) = (arbre.size.width() as f64, arbre.size.height() as f64);
    if !(l > 0.0 && h > 0.0) {
        return refus(format!("le SVG {} n'a pas de taille", nom));
    }

    // svg2pdf écrit dans un PDF minimal, que lopdf relit ; on en garde l'objet formulaire et ce
    // qu'il cite.
    let mut pdf = pdf_writer::Pdf::new();
    let catalogue = pdf_writer::Ref::new(1);
    let arbre_pages = pdf_writer::Ref::new(2);
    pdf.catalog(catalogue).pages(arbre_pages);
    pdf.pages(arbre_pages).count(0);
    // L'objet formulaire est écrit au numéro de départ ; `convert_tree_into` rend le numéro libre
    // qui suit ses objets, pas le sien.
    let depart = pdf_writer::Ref::new(3);
    let mut morceau = pdf_writer::Chunk::new();
    svg2pdf::convert_tree_into(&arbre, svg2pdf::Options::default(), &mut morceau, depart);
    pdf.extend(&morceau);
    let source = Document::load_mem(&pdf.finish()).map_err(|e| Refus(format!("le SVG {} n'a pas pu être converti ({})", nom, e)))?;
    let id = importer(doc, &source, (depart.get() as u32, 0))?;
    if !textes.is_empty() {
        ecrire_textes(doc, id, &textes, h, avant)?;
    }
    Ok(Image { xobjet: id, largeur: l, hauteur: h })
}

/// Recopie dans `doc` l'objet `racine` de `source` et tout ce qu'il cite, sous de nouveaux
/// numéros ; rend le numéro de la racine.
fn importer(doc: &mut Document, source: &Document, racine: ObjectId) -> Result<ObjectId, Refus> {
    let mut nouveaux: BTreeMap<ObjectId, ObjectId> = BTreeMap::new();
    let mut a_faire = vec![racine];
    while let Some(id) = a_faire.pop() {
        if nouveaux.contains_key(&id) {
            continue;
        }
        let o = source.get_object(id).map_err(|_| Refus("objet manquant dans le SVG converti".into()))?;
        nouveaux.insert(id, doc.new_object_id());
        references(o, &mut a_faire);
    }
    for (ancien, nouveau) in &nouveaux {
        let mut o = source.get_object(*ancien).map_err(|_| Refus("objet manquant dans le SVG converti".into()))?.clone();
        renumeroter(&mut o, &nouveaux);
        doc.objects.insert(*nouveau, o);
    }
    Ok(nouveaux[&racine])
}

fn references(o: &Object, sortie: &mut Vec<ObjectId>) {
    match o {
        Object::Reference(r) => sortie.push(*r),
        Object::Array(v) => v.iter().for_each(|x| references(x, sortie)),
        Object::Dictionary(d) => d.iter().for_each(|(_, x)| references(x, sortie)),
        Object::Stream(s) => s.dict.iter().for_each(|(_, x)| references(x, sortie)),
        _ => {}
    }
}

fn renumeroter(o: &mut Object, table: &BTreeMap<ObjectId, ObjectId>) {
    match o {
        Object::Reference(r) => {
            if let Some(n) = table.get(r) {
                *r = *n;
            }
        }
        Object::Array(v) => v.iter_mut().for_each(|x| renumeroter(x, table)),
        Object::Dictionary(d) => d.iter_mut().for_each(|(_, x)| renumeroter(x, table)),
        Object::Stream(s) => s.dict.iter_mut().for_each(|(_, x)| renumeroter(x, table)),
        _ => {}
    }
}

// ------------------------------------------------------------------ remplir

/// Un rectangle [x0, y0, x1, y1] de nombres finis, x1 > x0 et y1 > y0.
fn rect_valide(r: &[f64; 4]) -> bool {
    r.iter().all(|x| x.is_finite()) && r[2] > r[0] && r[3] > r[1]
}

struct Pose {
    page: usize,
    rect: [f64; 4],
    marge: f64,
    octets: Vec<u8>,
    nom: String,
}

/// `--remplir` : le remplissage (FORMAT.md, « Remplir un formulaire ») appliqué au PDF
/// `octets`. Rend les octets du PDF rempli. Tout est vérifié avant d'écrire : un champ inconnu,
/// en lecture seule ou trop long, une image illisible, une page qui n'existe pas refusent
/// l'ensemble.
pub fn remplir(octets: &[u8], d: &Remplissage) -> Result<Vec<u8>, Refus> {
    if d.format != crate::FORMAT {
        return refus(format!("format {} : cette fabrique lit le format {}", d.format, crate::FORMAT));
    }
    let valeurs: Vec<(String, String)> =
        d.champs.iter().flatten().map(|(nom, texte)| (nom.clone(), texte.clone())).collect();
    let mut poses = Vec::new();
    for (i, im) in d.images.iter().flatten().enumerate() {
        poses.push(lire_pose(im, i + 1)?);
    }

    // Le formulaire reçu reste tel quel, octet pour octet : ce qu'on y écrit (les valeurs, leur apparence,
    // la police, les images) s'ajoute à la fin, en mise à jour incrémentale (ISO 32000-1, 7.5.6).
    let (avant, lus) = crate::lecture::charger_avec_octets(octets)?;
    if avant.trailer.get(b"Encrypt").is_ok() {
        return refus("le PDF est chiffré : on ne le remplit pas");
    }
    let mut doc = avant.clone();
    if !valeurs.is_empty() {
        ecrire_champs(&mut doc, &valeurs, Some(&avant))?;
    }
    if !poses.is_empty() {
        poser_images(&mut doc, &poses, Some(&avant))?;
    }
    doc.prune_objects();
    Ok(genpdfi::pdf_ecriture::ajouter(&lus, &avant, &doc))
}

fn lire_pose(im: &crate::forme::ImagePosee, rang: usize) -> Result<Pose, Refus> {
    if im.page < 1 {
        return refus(format!("image {} : « page » doit être un entier à partir de 1", rang));
    }
    if !rect_valide(&im.rect) {
        return refus(format!("image {} : « rect » doit être [x0, y0, x1, y1] avec x1 > x0 et y1 > y0", rang));
    }
    let marge = match im.marge {
        None => 0.0,
        Some(m) if m.is_finite() && m >= 0.0 => m,
        Some(_) => return refus(format!("image {} : « marge » doit être un nombre positif", rang)),
    };
    let (octets, nom) = match (&im.fichier, &im.svg) {
        (Some(f), None) => (std::fs::read(f).map_err(|e| Refus(format!("image {} illisible ({})", f, e)))?, f.clone()),
        (None, Some(s)) => (s.as_bytes().to_vec(), format!("de l'image {}", rang)),
        _ => return refus(format!("image {} : il faut « fichier » ou « svg », l'un des deux", rang)),
    };
    Ok(Pose { page: im.page, rect: im.rect, marge, octets, nom })
}

/// La police (sous-ensemble de `donnees`) qui écrit `caracteres` dans `doc` : une seule par document et par
/// fichier de police. Une de celles que la fabrique y a déjà mises, si elle porte ces caractères ; sinon une
/// qu'on peut élargir à ce qui lui manque : toute police quand le document se réécrit en entier (`avant` vide),
/// une police ajoutée par cette écriture-ci quand il s'écrit en mise à jour incrémentale (`avant` : le document
/// reçu, dont les objets restent tels quels) ; sinon, un sous-ensemble neuf de ces seuls caractères.
pub(crate) fn police_pour(doc: &mut Document, donnees: &[u8], caracteres: &BTreeSet<char>, avant: Option<&Document>) -> Result<ObjectId, Refus> {
    let echec = |e: genpdfi::error::Error| Refus(format!("police impossible à embarquer : {}", e));
    let candidates = genpdfi::pdf_subset::polices_de_la_fabrique(doc, donnees);
    if let Some((id, _)) = candidates.iter().find(|(_, portes)| caracteres.is_subset(portes)) {
        return Ok(*id);
    }
    if let Some((id, portes)) = candidates.into_iter().find(|(id, _)| avant.map_or(true, |a| !a.objects.contains_key(id))) {
        let tous: BTreeSet<char> = portes.union(caracteres).copied().collect();
        genpdfi::pdf_subset::elargir(doc, id, donnees, &tous).map_err(echec)?;
        return Ok(id);
    }
    genpdfi::pdf_subset::embarquer(doc, donnees, caracteres).map_err(echec)
}

/// Les polices qui écrivent `caracteres` dans une mise à jour incrémentale de `avant` (le document reçu, qui reste
/// tel quel), chacune avec les caractères qu'elle écrit : celle que la fabrique y a déjà mise et qui en porte le plus,
/// puis, pour ce qui lui manque, un sous-ensemble neuf de ces seuls caractères (`police_pour`). Un texte qui passe de
/// l'une à l'autre change de police entre deux `Tj`, sans changer de place.
pub(crate) fn polices_pour_ajout(doc: &mut Document, donnees: &[u8], caracteres: &BTreeSet<char>, avant: &Document) -> Result<Vec<(ObjectId, BTreeSet<char>)>, Refus> {
    let meilleure = genpdfi::pdf_subset::polices_de_la_fabrique(doc, donnees)
        .into_iter()
        .filter(|(id, _)| avant.objects.contains_key(id))
        .map(|(id, portes)| (id, caracteres.intersection(&portes).copied().collect::<BTreeSet<char>>()))
        .max_by_key(|(_, communs)| communs.len());
    let mut polices = Vec::new();
    let mut reste = caracteres.clone();
    if let Some((id, communs)) = meilleure.filter(|(_, c)| !c.is_empty()) {
        reste = caracteres.difference(&communs).copied().collect();
        polices.push((id, communs));
    }
    if !reste.is_empty() {
        polices.push((police_pour(doc, donnees, &reste, Some(avant))?, reste));
    }
    Ok(polices)
}

/// Les valeurs écrites dans leurs champs, l'apparence régénérée ; `avant` : voir `police_pour`.
fn ecrire_champs(doc: &mut Document, valeurs: &[(String, String)], avant: Option<&Document>) -> Result<(), Refus> {
    let (liste, _) = lister(doc);
    let mut par_nom: HashMap<&str, &Champ> = HashMap::new();
    for c in &liste {
        par_nom.entry(c.nom.as_str()).or_insert(c);
    }
    // Tout se vérifie avant d'écrire quoi que ce soit.
    let donnees = crate::polices::sans_regulier()?;
    let face = ttf_parser::Face::parse(&donnees, 0).map_err(|e| Refus(format!("police interne illisible : {:?}", e)))?;
    let mut caracteres = BTreeSet::new();
    for (nom, texte) in valeurs {
        let c = match par_nom.get(nom.as_str()) {
            Some(c) => *c,
            None => return refus(format!("le formulaire n'a pas de champ « {} »", nom)),
        };
        if c.ft.as_deref() != Some(b"Tx") {
            return refus(format!("le champ « {} » n'est pas un champ texte ({})", nom, type_de(&c.ft)));
        }
        if c.ff & 1 != 0 {
            return refus(format!("le champ « {} » est en lecture seule : on n'y écrit pas", nom));
        }
        if c.ff & MOT_DE_PASSE != 0 {
            return refus(format!("le champ « {} » est un champ de mot de passe : on n'y écrit pas", nom));
        }
        if let Some(m) = c.maxlen {
            if m > 0 && texte.chars().count() as i64 > m {
                return refus(format!("le champ « {} » prend {} caractères au plus : « {} » en a {}", nom, m, texte, texte.chars().count()));
            }
        }
        for ch in texte.chars() {
            if ch == '\n' || ch == '\r' {
                continue;
            }
            if ch as u32 > 0xFFFF || face.glyph_index(ch).is_none() {
                return refus(format!(
                    "champ « {} » : le caractère « {} » (U+{:04X}) n'existe pas dans la police Liberation",
                    nom, ch, ch as u32
                ));
            }
            caracteres.insert(ch);
        }
    }
    let police = if caracteres.is_empty() { None } else { Some(police_pour(doc, &donnees, &caracteres, avant)?) };
    let metriques = Metriques { face };
    for (nom, texte) in valeurs {
        let texte = texte.replace("\r\n", "\n").replace('\r', "\n");
        let c = par_nom[nom.as_str()];
        let mut apparences = Vec::new();
        for w in &c.widgets {
            let r = nombres(doc, doc.get_dictionary(*w).ok().and_then(|d| d.get(b"Rect").ok()));
            if r.len() != 4 {
                continue;
            }
            let (l, h) = ((r[2] - r[0]).abs(), (r[3] - r[1]).abs());
            let flux = flux_apparence(&metriques, c, &texte, l, h, "PPChamp");
            let mut dict = Dictionary::new();
            dict.set("Type", Object::Name(b"XObject".to_vec()));
            dict.set("Subtype", Object::Name(b"Form".to_vec()));
            dict.set("BBox", Object::Array(vec![0.into(), 0.into(), Object::Real(l as f32), Object::Real(h as f32)]));
            let mut ressources = Dictionary::new();
            if let Some(p) = police {
                let mut polices = Dictionary::new();
                polices.set("PPChamp", Object::Reference(p));
                ressources.set("Font", Object::Dictionary(polices));
            }
            dict.set("Resources", Object::Dictionary(ressources));
            let mut s = Stream::new(dict, flux);
            let _ = s.compress();
            apparences.push((*w, doc.add_object(Object::Stream(s))));
        }
        if let Ok(o) = doc.get_object_mut(c.id).and_then(Object::as_dict_mut) {
            o.set("V", Object::String(vers_chaine_pdf(&texte), lopdf::StringFormat::Literal));
        }
        for (w, ap) in apparences {
            if let Ok(o) = doc.get_object_mut(w).and_then(Object::as_dict_mut) {
                let mut a = Dictionary::new();
                a.set("N", Object::Reference(ap));
                o.set("AP", Object::Dictionary(a));
            }
        }
    }
    Ok(())
}

fn poser_images(doc: &mut Document, poses: &[Pose], avant: Option<&Document>) -> Result<(), Refus> {
    let pages: BTreeMap<u32, ObjectId> = doc.get_pages();
    let mut par_page: BTreeMap<usize, Vec<(ObjectId, [f64; 6])>> = BTreeMap::new();
    for p in poses {
        let page = match pages.get(&(p.page as u32)) {
            Some(id) => *id,
            None => return refus(format!("pas de page {} : le PDF en a {}", p.page, pages.len())),
        };
        let rotation = match herite(doc, page, b"Rotate") {
            Some(Object::Integer(r)) => r.rem_euclid(360),
            _ => 0,
        };
        if rotation != 0 {
            return refus(format!("la page {} est tournée ({}°) : on n'y pose pas d'image", p.page, rotation));
        }
        let c = cadre(doc, page).unwrap_or([0.0, 0.0, 595.0, 842.0]);
        let dedans = [p.rect[0] + p.marge, p.rect[1] + p.marge, p.rect[2] - p.marge, p.rect[3] - p.marge];
        let (cl, ch) = (dedans[2] - dedans[0], dedans[3] - dedans[1]);
        if cl <= 0.0 || ch <= 0.0 {
            return refus(format!("image {} : la marge ne laisse rien du rectangle", p.nom));
        }
        let image = match sorte(&p.octets) {
            Some("svg") => image_svg(doc, &p.octets, &p.nom, avant)?,
            Some(s) => image_matricielle(doc, &p.octets, s, &p.nom)?,
            None => return refus(format!("l'image {} n'est ni un SVG, ni un PNG, ni un JPEG", p.nom)),
        };
        // Le plus grand rectangle aux proportions de l'image qui tient dans la case, centré.
        let k = (cl / image.largeur).min(ch / image.hauteur);
        let (dl, dh) = (image.largeur * k, image.hauteur * k);
        let x = c[0] + dedans[0] + (cl - dl) / 2.0;
        let haut = dedans[1] + (ch - dh) / 2.0;
        let y = c[3] - haut - dh;
        par_page.entry(p.page).or_default().push((image.xobjet, [dl, 0.0, 0.0, dh, x, y]));
    }
    for (n, liste) in par_page {
        let id = pages[&(n as u32)];
        let mut ressources: Dictionary =
            herite(doc, id, b"Resources").and_then(|o| dico(doc, o)).cloned().unwrap_or_default();
        let mut xobjets: Dictionary =
            ressources.get(b"XObject").ok().and_then(|o| dico(doc, o)).cloned().unwrap_or_default();
        let mut flux = String::new();
        let mut rang = 1;
        for (objet, m) in liste {
            let mut nom = format!("PPImage{}", rang);
            while xobjets.has(nom.as_bytes()) {
                rang += 1;
                nom = format!("PPImage{}", rang);
            }
            rang += 1;
            xobjets.set(nom.as_str(), Object::Reference(objet));
            flux.push_str(&format!(
                "q\n{} {} {} {} {} {} cm\n/{} Do\nQ\n",
                nb(m[0]), nb(m[1]), nb(m[2]), nb(m[3]), nb(m[4]), nb(m[5]), nom
            ));
        }
        ressources.set("XObject", Object::Dictionary(xobjets));
        let anciens = flux_existants(doc, id);
        let ouverts = q_ouverts(doc, id);
        let avant = doc.add_object(Object::Stream(Stream::new(Dictionary::new(), b"q\n".to_vec())));
        let apres = doc.add_object(Object::Stream(Stream::new(Dictionary::new(), "Q\n".repeat(ouverts + 1).into_bytes())));
        let images = doc.add_object(Object::Stream(Stream::new(Dictionary::new(), flux.into_bytes())));
        let mut contenu = vec![Object::Reference(avant)];
        contenu.extend(anciens);
        contenu.push(Object::Reference(apres));
        contenu.push(Object::Reference(images));
        let page = doc
            .get_object_mut(id)
            .and_then(Object::as_dict_mut)
            .map_err(|e| Refus(format!("page illisible ({})", e)))?;
        page.set("Resources", Object::Dictionary(ressources));
        page.set("Contents", Object::Array(contenu));
    }
    Ok(())
}

// ------------------------------------------------------------------ ajouter des champs

/// Un champ à ajouter (`ajouter_champs`).
struct Ajout {
    nom: String,
    page: usize,
    rect: [f64; 4],
    valeur: Option<String>,
    lecture_seule: bool,
    taille: f64,
    q: i64,
}

fn lire_ajout(c: &crate::forme::ChampAjoute, rang: usize) -> Result<Ajout, Refus> {
    if c.nom.is_empty() || c.nom.contains('.') {
        return refus(format!("champ {} : « nom » doit être un texte non vide, sans point", rang));
    }
    let nom = c.nom.clone();
    if c.page < 1 {
        return refus(format!("champ « {} » : « page » doit être un entier à partir de 1", nom));
    }
    if !rect_valide(&c.rect) {
        return refus(format!("champ « {} » : « rect » doit être [x0, y0, x1, y1] avec x1 > x0 et y1 > y0", nom));
    }
    let lecture_seule = c.lecture_seule.unwrap_or(false);
    // Le corps 0 : celui qui tient (`/DA`).
    let taille = match c.taille {
        None => 0.0,
        Some(t) if t.is_finite() && t >= 0.0 => t,
        Some(_) => return refus(format!("champ « {} » : « taille » doit être un nombre positif ou nul", nom)),
    };
    let q = match c.alignement {
        None | Some(Alignement::Gauche) => 0,
        Some(Alignement::Centre) => 1,
        Some(Alignement::Droite) => 2,
        Some(Alignement::Justifie) => {
            return refus(format!("champ « {} » : alignement « justifie » inconnu (gauche, centre, droite)", nom))
        }
    };
    if c.valeur.is_none() && lecture_seule {
        return refus(format!("champ « {} » : un champ en lecture seule sans valeur ne servirait à rien", nom));
    }
    Ok(Ajout { nom, page: c.page, rect: c.rect, valeur: c.valeur.clone(), lecture_seule, taille, q })
}

/// `ajouter_champs` : des champs texte AcroForm ajoutés à un PDF existant (le plus souvent un document de la fabrique,
/// dessiné en blocs fixes), chacun à sa page et dans son rectangle (origine en haut à gauche, comme `--champs`). Un champ
/// qui a une valeur la reçoit avec son apparence régénérée (celle de `remplir`) ; `lecture_seule` pose ensuite le bit 1
/// de `/Ff`, que `remplir` refuse de franchir ; sans `taille`, le corps qui tient. Tout se vérifie avant d'écrire :
/// un nom déjà porté par le formulaire, une page qui n'existe pas, un caractère que la police n'a pas, et rien n'est rendu.
pub fn ajouter_champs(octets: &[u8], d: &AjoutDeChamps) -> Result<Vec<u8>, Refus> {
    if d.format != crate::FORMAT {
        return refus(format!("format {} : cette fabrique lit le format {}", d.format, crate::FORMAT));
    }
    if d.champs.is_empty() {
        return refus("« champs » doit être une liste non vide");
    }
    let mut ajouts = Vec::new();
    for (i, c) in d.champs.iter().enumerate() {
        ajouts.push(lire_ajout(c, i + 1)?);
    }
    let mut doc = crate::lecture::charger(octets)?;
    if doc.trailer.get(b"Encrypt").is_ok() {
        return refus("le PDF est chiffré : on n'y ajoute rien");
    }
    let (existants, _) = lister(&doc);
    let mut noms: BTreeSet<String> = existants.into_iter().map(|c| c.nom).collect();
    for a in &ajouts {
        if !noms.insert(a.nom.clone()) {
            return refus(format!("le formulaire a déjà un champ « {} »", a.nom));
        }
    }
    let pages: BTreeMap<u32, ObjectId> = doc.get_pages();
    for a in &ajouts {
        if !pages.contains_key(&(a.page as u32)) {
            return refus(format!("champ « {} » : pas de page {} (le PDF en a {})", a.nom, a.page, pages.len()));
        }
    }

    // Le formulaire du catalogue : créé s'il n'existe pas, avec Helvetica comme police par défaut (`/DR`).
    let helv = doc.add_object(Object::Dictionary({
        let mut f = Dictionary::new();
        f.set("Type", Object::Name(b"Font".to_vec()));
        f.set("Subtype", Object::Name(b"Type1".to_vec()));
        f.set("BaseFont", Object::Name(b"Helvetica".to_vec()));
        f.set("Encoding", Object::Name(b"WinAnsiEncoding".to_vec()));
        f
    }));
    let racine = doc.trailer.get(b"Root").and_then(Object::as_reference).map_err(|_| Refus("PDF sans catalogue".into()))?;
    let acro_existant: Option<Object> = doc.get_dictionary(racine).ok().and_then(|c| c.get(b"AcroForm").ok()).cloned();
    let acro_id = match acro_existant {
        Some(Object::Reference(id)) => id,
        Some(Object::Dictionary(dico_inline)) => doc.add_object(Object::Dictionary(dico_inline)),
        _ => doc.add_object(Object::Dictionary(Dictionary::new())),
    };
    {
        let acro = doc.get_object_mut(acro_id).and_then(Object::as_dict_mut).map_err(|e| Refus(format!("formulaire illisible ({})", e)))?;
        if !acro.has(b"DA") {
            acro.set("DA", Object::String(b"/Helv 0 Tf 0 g".to_vec(), lopdf::StringFormat::Literal));
        }
        if !acro.has(b"DR") {
            let mut polices = Dictionary::new();
            polices.set("Helv", Object::Reference(helv));
            let mut dr = Dictionary::new();
            dr.set("Font", Object::Dictionary(polices));
            acro.set("DR", Object::Dictionary(dr));
        }
    }
    if let Ok(c) = doc.get_object_mut(racine).and_then(Object::as_dict_mut) {
        c.set("AcroForm", Object::Reference(acro_id));
    }

    // Les widgets (champ et widget fusionnés), dans `/Annots` de leur page et dans `/Fields`.
    let mut nouveaux: Vec<ObjectId> = Vec::new();
    for a in &ajouts {
        let page = pages[&(a.page as u32)];
        let c = cadre(&doc, page).unwrap_or([0.0, 0.0, 595.0, 842.0]);
        let r = [c[0] + a.rect[0], c[3] - a.rect[3], c[0] + a.rect[2], c[3] - a.rect[1]];
        let mut w = Dictionary::new();
        w.set("Type", Object::Name(b"Annot".to_vec()));
        w.set("Subtype", Object::Name(b"Widget".to_vec()));
        w.set("FT", Object::Name(b"Tx".to_vec()));
        w.set("T", Object::String(vers_chaine_pdf(&a.nom), lopdf::StringFormat::Literal));
        w.set("Rect", Object::Array(r.iter().map(|x| Object::Real(*x as f32)).collect()));
        w.set("P", Object::Reference(page));
        w.set("F", Object::Integer(4));
        w.set("Ff", Object::Integer(0));
        w.set("Q", Object::Integer(a.q));
        w.set("DA", Object::String(format!("/Helv {} Tf 0 g", nb(a.taille)).into_bytes(), lopdf::StringFormat::Literal));
        let id = doc.add_object(Object::Dictionary(w));
        nouveaux.push(id);
        let annots: Option<Object> = doc.get_dictionary(page).ok().and_then(|p| p.get(b"Annots").ok()).cloned();
        match annots {
            Some(Object::Reference(tab)) => {
                if let Ok(Object::Array(v)) = doc.get_object_mut(tab) {
                    v.push(Object::Reference(id));
                }
            }
            Some(Object::Array(mut v)) => {
                v.push(Object::Reference(id));
                if let Ok(p) = doc.get_object_mut(page).and_then(Object::as_dict_mut) {
                    p.set("Annots", Object::Array(v));
                }
            }
            _ => {
                if let Ok(p) = doc.get_object_mut(page).and_then(Object::as_dict_mut) {
                    p.set("Annots", Object::Array(vec![Object::Reference(id)]));
                }
            }
        }
    }
    let champs_existants: Option<Object> = doc.get_dictionary(acro_id).ok().and_then(|a| a.get(b"Fields").ok()).cloned();
    let refs: Vec<Object> = nouveaux.iter().map(|id| Object::Reference(*id)).collect();
    match champs_existants {
        Some(Object::Reference(tab)) => {
            if let Ok(Object::Array(v)) = doc.get_object_mut(tab) {
                v.extend(refs);
            }
        }
        Some(Object::Array(mut v)) => {
            v.extend(refs);
            if let Ok(a) = doc.get_object_mut(acro_id).and_then(Object::as_dict_mut) {
                a.set("Fields", Object::Array(v));
            }
        }
        _ => {
            if let Ok(a) = doc.get_object_mut(acro_id).and_then(Object::as_dict_mut) {
                a.set("Fields", Object::Array(refs));
            }
        }
    }

    // Les valeurs, avec leur apparence (le même chemin que `remplir`), puis la lecture seule.
    let valeurs: Vec<(String, String)> = ajouts.iter().filter_map(|a| a.valeur.clone().map(|v| (a.nom.clone(), v))).collect();
    if !valeurs.is_empty() {
        ecrire_champs(&mut doc, &valeurs, None)?;
    }
    for (a, id) in ajouts.iter().zip(&nouveaux) {
        if a.lecture_seule {
            if let Ok(w) = doc.get_object_mut(*id).and_then(Object::as_dict_mut) {
                w.set("Ff", Object::Integer(1));
            }
        }
    }
    doc.prune_objects();
    Ok(genpdfi::pdf_ecriture::compact(&doc))
}
