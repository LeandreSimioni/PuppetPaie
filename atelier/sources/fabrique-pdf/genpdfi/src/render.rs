// Fichier modifié pour PuppetPaie (copie locale de genpdfi 0.2.7, Apache-2.0 OU MIT) :
// voir MODIFICATIONS.md à la racine de cette copie.
//! Low-level PDF rendering utilities.
//!
//! This module provides low-level abstractions over [`printpdf`][]:  A [`Renderer`][] creates a
//! document with one or more pages with different sizes.  A [`Page`][] has one or more layers, all
//! of the same size.  A [`Layer`][] can be used to access its [`Area`][].
//!
//! An [`Area`][] is a view on a full layer or on a part of a layer.  It can be used to print
//! lines and text.  For more advanced text formatting, you can create a [`TextSection`][] from an
//! [`Area`][].
//!
//! [`printpdf`]: https://docs.rs/printpdf/latest/printpdf
//! [`Renderer`]: struct.Renderer.html
//! [`Page`]: struct.Page.html
//! [`Layer`]: struct.Layer.html
//! [`Area`]: struct.Area.html
//! [`TextSection`]: struct.TextSection.html

use std::cell;
use std::io;
use std::ops;
use std::rc;

use crate::error::{Context as _, Error, ErrorKind};
use crate::fonts;
use crate::style::{Color, LineStyle, Style};
use crate::{Margins, Mm, Position, Size};

/// A position relative to the top left corner of a layer.
struct LayerPosition(Position);

impl LayerPosition {
    pub fn from_area(area: &Area<'_>, position: Position) -> Self {
        Self(position + area.origin)
    }
}

/// A position relative to the bottom left corner of a layer (“user space” in PDF terms).
struct UserSpacePosition(Position);

impl UserSpacePosition {
    pub fn from_layer(layer: &Layer<'_>, position: LayerPosition) -> Self {
        Self(Position::new(
            position.0.x,
            layer.page.size.height - position.0.y,
        ))
    }
}

impl From<UserSpacePosition> for printpdf::Point {
    fn from(pos: UserSpacePosition) -> printpdf::Point {
        printpdf::Point::new(pos.0.x.into(), pos.0.y.into())
    }
}

impl ops::Deref for UserSpacePosition {
    type Target = Position;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Renders a PDF document with one or more pages.
///
/// This is a wrapper around a [`printpdf::PdfDocumentReference`][].
///
/// [`printpdf::PdfDocumentReference`]: https://docs.rs/printpdf/0.3.2/printpdf/types/pdf_document/struct.PdfDocumentReference.html
pub struct Renderer {
    doc: printpdf::PdfDocumentReference,
    // invariant: pages.len() >= 1
    pages: Vec<Page>,
    // Modification PuppetPaie : les polices en sous-ensemble (données du fichier de police) et
    // les caractères imprimés avec chacune, que `write` coupe à ces seuls glyphes.
    subsets: Vec<crate::pdf_subset::SubsetFace>,
}

impl Renderer {
    /// Creates a new PDF document renderer with one page of the given size and the given title.
    pub fn new(size: impl Into<Size>, title: impl AsRef<str>) -> Result<Renderer, Error> {
        let size = size.into();
        let (doc, page_idx, layer_idx) = printpdf::PdfDocument::new(
            title.as_ref(),
            size.width.into(),
            size.height.into(),
            "Layer 1",
        );
        // Modification PuppetPaie : sortie déterministe.  printpdf date le document de
        // l'instant présent et tire des identifiants au hasard ; on fixe les dates ici et les
        // identifiants du fichier dans `write`.
        let fixed = fixed_date();
        let doc = doc
            .with_creation_date(fixed)
            .with_mod_date(fixed)
            .with_metadata_date(fixed)
            .with_document_id(FIXED_ID.to_string());
        let page_ref = doc.get_page(page_idx);
        let layer_ref = page_ref.get_layer(layer_idx);
        let page = Page::new(page_ref, layer_ref, size);

        Ok(Renderer {
            doc,
            pages: vec![page],
            subsets: Vec::new(),
        })
    }

    /// Sets the PDF conformance for the generated PDF document.
    pub fn with_conformance(mut self, conformance: printpdf::PdfConformance) -> Self {
        self.doc = self.doc.with_conformance(conformance);
        self
    }

    /// Sets the creation date for the generated PDF document.
    pub fn with_creation_date(mut self, date: printpdf::OffsetDateTime) -> Self {
        self.doc = self.doc.with_creation_date(date);
        self
    }

    /// Sets the modification date for the generated PDF document.
    pub fn with_modification_date(mut self, date: printpdf::OffsetDateTime) -> Self {
        self.doc = self.doc.with_mod_date(date);
        self
    }

    /// Adds a new page with the given size to the document.
    pub fn add_page(&mut self, size: impl Into<Size>) {
        let size = size.into();
        let (page_idx, layer_idx) =
            self.doc
                .add_page(size.width.into(), size.height.into(), "Layer 1");
        let page_ref = self.doc.get_page(page_idx);
        let layer_ref = page_ref.get_layer(layer_idx);
        self.pages.push(Page::new(page_ref, layer_ref, size))
    }

    /// Returns the number of pages in this document.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Returns a page of this document.
    pub fn get_page(&self, idx: usize) -> Option<&Page> {
        self.pages.get(idx)
    }

    /// Returns a mutable reference to a page of this document.
    pub fn get_page_mut(&mut self, idx: usize) -> Option<&mut Page> {
        self.pages.get_mut(idx)
    }

    /// Returns a mutable reference to the first page of this document.
    pub fn first_page(&self) -> &Page {
        &self.pages[0]
    }

    /// Returns the first page of this document.
    pub fn first_page_mut(&mut self) -> &mut Page {
        &mut self.pages[0]
    }

    /// Returns the last page of this document.
    pub fn last_page(&self) -> &Page {
        &self.pages[self.pages.len() - 1]
    }

    /// Returns a mutable reference to the last page of this document.
    pub fn last_page_mut(&mut self) -> &mut Page {
        let idx = self.pages.len() - 1;
        &mut self.pages[idx]
    }

    /// Loads the font from the given data, adds it to the generated document and returns a
    /// reference to it.
    pub fn add_builtin_font(
        &self,
        builtin: printpdf::BuiltinFont,
    ) -> Result<printpdf::IndirectFontRef, Error> {
        self.doc
            .add_builtin_font(builtin)
            .context("Failed to load PDF font")
    }

    /// Loads the font from the given data, adds it to the generated document and returns a
    /// reference to it.
    pub fn add_embedded_font(&self, data: &[u8]) -> Result<printpdf::IndirectFontRef, Error> {
        self.doc
            .add_external_font(data)
            .context("Failed to load PDF font")
    }

    /// Tells which characters were printed with each font embedded as a subset (PuppetPaie
    /// modification, see `fonts::FontData::new_subset`).  `write` keeps only their glyphs.
    pub fn set_subsets(&mut self, subsets: Vec<crate::pdf_subset::SubsetFace>) {
        self.subsets = subsets;
    }

    /// Writes this PDF document to a writer.
    pub fn write(self, w: impl io::Write) -> Result<(), Error> {
        use io::Write as _;
        let bytes = self
            .doc
            .save_to_bytes()
            .context("Failed to save document")?;
        let bytes = finish(bytes, &self.subsets)?;
        let mut w = io::BufWriter::new(w);
        w.write_all(&bytes).context("Failed to write document")?;
        w.flush().context("Failed to write document")
    }
}

/// The fixed identifier written in the file trailer (PuppetPaie modification).
const FIXED_ID: &str = "50757070657450616965000000000001";

/// The fixed date of creation and modification (PuppetPaie modification): 1970-01-01T00:00:00Z.
fn fixed_date() -> printpdf::OffsetDateTime {
    printpdf::OffsetDateTime::from_unix_timestamp(0).expect("date fixe valide")
}

/// Registry of the alpha planes of the raster images (PuppetPaie modification).
///
/// printpdf 0.7.0 writes the soft mask of a transparent image as an inline stream with the wrong
/// height, which is not a valid PDF.  The pictures therefore hand printpdf an opaque RGB image
/// marked by an identifier, and `finish` attaches the real soft mask (an indirect stream) to
/// the marked image objects once the document is written.
static ALPHA_PLANES: std::sync::Mutex<Vec<(u32, Vec<u8>)>> = std::sync::Mutex::new(Vec::new());

/// Stores the alpha plane (one byte per pixel) of an image and returns the marker that must be
/// put in the image's `clipping_bbox` as `CurTransMat::Translate(Pt(marker), Pt(0))`.
pub fn register_alpha_plane(alpha: Vec<u8>) -> u32 {
    let mut planes = ALPHA_PLANES.lock().unwrap_or_else(|e| e.into_inner());
    let id = planes.len() as u32 + 1;
    planes.push((id, alpha));
    id
}

/// Finishes the file written by printpdf (PuppetPaie modification):
///
/// - fixes the images: attaches the soft masks, removes the `BBox` and the null `SMask` that
///   printpdf puts in every image dictionary (they are not valid there);
/// - cuts the fonts embedded as subsets down to the glyphs that were printed, and writes their
///   `ToUnicode` table (`crate::pdf_subset`);
/// - replaces the identifiers of the trailer, drawn at random by printpdf, by a fixed one;
/// - prunes, removes empty streams and compresses, in debug builds as well as in release builds
///   (printpdf only does it in release), so that the file does not depend on the profile.
fn finish(bytes: Vec<u8>, subsets: &[crate::pdf_subset::SubsetFace]) -> Result<Vec<u8>, Error> {
    use printpdf::lopdf::{Dictionary, Document, Object, Stream, StringFormat};

    let internal = |what: &str, e: printpdf::lopdf::Error| {
        Error::new(format!("{}: {}", what, e), ErrorKind::Internal)
    };
    let mut pdf =
        Document::load_mem(&bytes).map_err(|e| internal("Failed to reread the document", e))?;
    let ids: Vec<_> = pdf.objects.keys().copied().collect();
    let mut masks: Vec<(printpdf::lopdf::ObjectId, u32, i64, i64)> = Vec::new();
    for id in ids {
        if let Some(Object::Stream(s)) = pdf.objects.get_mut(&id) {
            let is_image = s
                .dict
                .get(b"Subtype")
                .ok()
                .and_then(|o| o.as_name().ok())
                .map(|n| n == b"Image")
                .unwrap_or(false);
            if !is_image {
                continue;
            }
            let marker = s
                .dict
                .get(b"BBox")
                .ok()
                .and_then(|o| o.as_array().ok())
                .and_then(|a| a.get(4))
                .and_then(|o| match o {
                    Object::Real(r) => Some(*r as u32),
                    Object::Integer(i) => Some(*i as u32),
                    _ => None,
                })
                .unwrap_or(0);
            s.dict.remove(b"BBox");
            if matches!(s.dict.get(b"SMask"), Ok(Object::Null)) {
                s.dict.remove(b"SMask");
            }
            if marker > 0 {
                let w = s.dict.get(b"Width").ok().and_then(|o| o.as_i64().ok()).unwrap_or(0);
                let h = s.dict.get(b"Height").ok().and_then(|o| o.as_i64().ok()).unwrap_or(0);
                masks.push((id, marker, w, h));
            }
        }
    }
    // Une image posée sur plusieurs pages (le logo, en haut de chacune) : un seul masque par plan
    // alpha, puis (plus bas) un seul objet image (ticket #228).
    let mut mask_of_marker: std::collections::HashMap<u32, printpdf::lopdf::ObjectId> =
        std::collections::HashMap::new();
    for (id, marker, w, h) in masks {
        if let Some(mask_id) = mask_of_marker.get(&marker).copied() {
            if let Some(Object::Stream(s)) = pdf.objects.get_mut(&id) {
                s.dict.set("SMask", Object::Reference(mask_id));
            }
            continue;
        }
        let alpha = {
            let planes = ALPHA_PLANES.lock().unwrap_or_else(|e| e.into_inner());
            planes.iter().find(|(m, _)| *m == marker).map(|(_, a)| a.clone())
        };
        let alpha = alpha.ok_or_else(|| {
            Error::new("Unknown alpha plane marker", ErrorKind::Internal)
        })?;
        let mut dict = Dictionary::new();
        dict.set("Type", Object::Name(b"XObject".to_vec()));
        dict.set("Subtype", Object::Name(b"Image".to_vec()));
        dict.set("Width", Object::Integer(w));
        dict.set("Height", Object::Integer(h));
        dict.set("ColorSpace", Object::Name(b"DeviceGray".to_vec()));
        dict.set("BitsPerComponent", Object::Integer(8));
        let mask_id = pdf.add_object(Object::Stream(Stream::new(dict, alpha)));
        mask_of_marker.insert(marker, mask_id);
        if let Some(Object::Stream(s)) = pdf.objects.get_mut(&id) {
            s.dict.set("SMask", Object::Reference(mask_id));
        }
    }
    merge_identical_images(&mut pdf);

    // printpdf copies a converted SVG (a form XObject) into the document without the objects
    // that its resources point to (gradients, patterns, transparency states, embedded images),
    // so that those references point to nothing or, worse, to another object.  A form whose
    // resources hold a reference is therefore refused.
    for o in pdf.objects.values() {
        if let Object::Stream(s) = o {
            let is_form = s
                .dict
                .get(b"Subtype")
                .ok()
                .and_then(|o| o.as_name().ok())
                .map(|n| n == b"Form")
                .unwrap_or(false);
            if is_form {
                let mut refs = Vec::new();
                if let Ok(res) = s.dict.get(b"Resources") {
                    collect_references(res, &mut refs);
                }
                if !refs.is_empty() {
                    return Err(Error::new(
                        crate::elements::SVG_NON_PRIS_EN_CHARGE,
                        ErrorKind::InvalidData,
                    ));
                }
            }
        }
    }

    crate::pdf_subset::embed_subsets(&mut pdf, subsets)?;

    let fixed = |_: usize| Object::String(FIXED_ID.as_bytes().to_vec(), StringFormat::Literal);
    pdf.trailer
        .set("ID", Object::Array(vec![fixed(0), fixed(1)]));
    pdf.prune_objects();
    pdf.delete_zero_length_streams();
    predict_images(&mut pdf)?;
    pdf.compress();
    canonicalize(&mut pdf);

    let mut out = Vec::new();
    pdf.save_to(&mut out)
        .map_err(|e| Error::new(format!("Failed to write the document: {}", e), ErrorKind::Internal))?;
    Ok(out)
}

/// Compresses the raw pixels of the images (8 bits per component, grey or RGB, soft masks
/// included) with the PNG predictors before Flate (PuppetPaie modification, ticket #228): each row
/// is written with the PNG filter that gives the smallest sum of absolute differences, and the
/// stream says it (`DecodeParms`, `Predictor 15`, PDF 1.7 § 7.4.4.4).  A plain Flate of the pixels,
/// which is what `compress` does, keeps a gradient or an anti-aliased edge almost whole.
fn predict_images(pdf: &mut printpdf::lopdf::Document) -> Result<(), Error> {
    use printpdf::lopdf::{Dictionary, Object};
    use std::io::Write as _;
    for o in pdf.objects.values_mut() {
        let Object::Stream(s) = o else { continue };
        let name = |k: &[u8]| s.dict.get(k).ok().and_then(|o| o.as_name().ok()).map(|n| n.to_vec());
        let int = |k: &[u8]| s.dict.get(k).ok().and_then(|o| o.as_i64().ok());
        if name(b"Subtype").as_deref() != Some(b"Image") || s.dict.get(b"DecodeParms").is_ok() {
            continue;
        }
        // printpdf comprime déjà en `release` (Flate seul) : on repart des pixels.
        let raw = match s.dict.get(b"Filter").ok() {
            None => s.content.clone(),
            Some(f) => {
                let flate = match f {
                    Object::Name(n) => n.as_slice() == b"FlateDecode",
                    Object::Array(a) => a.len() == 1 && a[0].as_name().ok() == Some(&b"FlateDecode"[..]),
                    _ => false,
                };
                if !flate {
                    continue;
                }
                let mut out = Vec::new();
                use std::io::Read as _;
                if flate2::read::ZlibDecoder::new(&s.content[..]).read_to_end(&mut out).is_err() {
                    continue;
                }
                out
            }
        };
        let colors = match name(b"ColorSpace").as_deref() {
            Some(b"DeviceRGB") => 3usize,
            Some(b"DeviceGray") => 1,
            _ => continue,
        };
        let (Some(w), Some(h), Some(8)) = (int(b"Width"), int(b"Height"), int(b"BitsPerComponent")) else { continue };
        let (w, h) = (w as usize, h as usize);
        let row = w * colors;
        if w == 0 || h == 0 || raw.len() != row * h {
            continue;
        }
        let mut filtered = Vec::with_capacity((row + 1) * h);
        let zero = vec![0u8; row];
        let mut candidate = vec![0u8; row];
        let mut best = vec![0u8; row];
        for y in 0..h {
            let cur = &raw[y * row..(y + 1) * row];
            let up = if y == 0 { &zero[..] } else { &raw[(y - 1) * row..y * row] };
            let mut best_type = 0u8;
            let mut best_score = u64::MAX;
            for t in 0..5u8 {
                for i in 0..row {
                    let a = if i >= colors { cur[i - colors] } else { 0 };
                    let b = up[i];
                    let c = if i >= colors { up[i - colors] } else { 0 };
                    let p = match t {
                        0 => 0,
                        1 => a,
                        2 => b,
                        3 => ((a as u16 + b as u16) / 2) as u8,
                        _ => paeth(a, b, c),
                    };
                    candidate[i] = cur[i].wrapping_sub(p);
                }
                let score: u64 = candidate.iter().map(|&v| (v as i8).unsigned_abs() as u64).sum();
                if score < best_score {
                    best_score = score;
                    best_type = t;
                    best.copy_from_slice(&candidate);
                }
            }
            filtered.push(best_type);
            filtered.extend_from_slice(&best);
        }
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
        z.write_all(&filtered)
            .and_then(|_| z.flush())
            .map_err(|e| Error::new(format!("Failed to compress an image: {}", e), ErrorKind::Internal))?;
        let packed = z
            .finish()
            .map_err(|e| Error::new(format!("Failed to compress an image: {}", e), ErrorKind::Internal))?;
        if s.dict.get(b"Filter").is_ok() && packed.len() >= s.content.len() {
            continue; // le Flate seul de printpdf était déjà plus court
        }
        let mut parms = Dictionary::new();
        parms.set("Predictor", Object::Integer(15));
        parms.set("Colors", Object::Integer(colors as i64));
        parms.set("BitsPerComponent", Object::Integer(8));
        parms.set("Columns", Object::Integer(w as i64));
        s.dict.set("Filter", Object::Name(b"FlateDecode".to_vec()));
        s.dict.set("DecodeParms", Object::Dictionary(parms));
        s.set_content(packed);
    }
    Ok(())
}

/// The PNG Paeth predictor.
fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = a as i16 + b as i16 - c as i16;
    let (pa, pb, pc) = ((p - a as i16).abs(), (p - b as i16).abs(), (p - c as i16).abs());
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

/// Merges the image objects that are identical, dictionary and pixels (PuppetPaie modification,
/// ticket #228): printpdf writes a new image object each time a picture is placed, so the logo
/// drawn at the top of every page was stored once per page.  The duplicates are removed and every
/// reference to them points to the first one.
fn merge_identical_images(pdf: &mut printpdf::lopdf::Document) {
    use printpdf::lopdf::{Object, ObjectId};
    let mut first: std::collections::HashMap<(String, Vec<u8>), ObjectId> =
        std::collections::HashMap::new();
    let mut replace: std::collections::HashMap<ObjectId, ObjectId> =
        std::collections::HashMap::new();
    let ids: Vec<ObjectId> = pdf.objects.keys().copied().collect();
    for id in ids {
        if let Some(Object::Stream(s)) = pdf.objects.get(&id) {
            let is_image = s
                .dict
                .get(b"Subtype")
                .ok()
                .and_then(|o| o.as_name().ok())
                .map(|n| n == b"Image")
                .unwrap_or(false);
            if !is_image {
                continue;
            }
            let mut keys: Vec<String> = s
                .dict
                .iter()
                .map(|(k, v)| format!("{:?}={:?}", k, v))
                .collect();
            keys.sort();
            let key = (keys.join(";"), s.content.clone());
            match first.get(&key) {
                Some(kept) => {
                    replace.insert(id, *kept);
                }
                None => {
                    first.insert(key, id);
                }
            }
        }
    }
    if replace.is_empty() {
        return;
    }
    for id in replace.keys() {
        pdf.objects.remove(id);
    }
    fn redirect(o: &mut Object, replace: &std::collections::HashMap<ObjectId, ObjectId>) {
        match o {
            Object::Reference(r) => {
                if let Some(n) = replace.get(r) {
                    *r = *n;
                }
            }
            Object::Array(v) => v.iter_mut().for_each(|x| redirect(x, replace)),
            Object::Dictionary(d) => d.iter_mut().for_each(|(_, v)| redirect(v, replace)),
            Object::Stream(s) => s.dict.iter_mut().for_each(|(_, v)| redirect(v, replace)),
            _ => {}
        }
    }
    for o in pdf.objects.values_mut() {
        redirect(o, &replace);
    }
}

fn collect_references(o: &printpdf::lopdf::Object, out: &mut Vec<printpdf::lopdf::ObjectId>) {
    use printpdf::lopdf::Object;
    match o {
        Object::Reference(id) => out.push(*id),
        Object::Dictionary(d) => d.iter().for_each(|(_, v)| collect_references(v, out)),
        Object::Stream(s) => s.dict.iter().for_each(|(_, v)| collect_references(v, out)),
        Object::Array(a) => a.iter().for_each(|v| collect_references(v, out)),
        _ => {}
    }
}

/// Puts the objects of the document in a canonical order (PuppetPaie modification).
///
/// printpdf builds some dictionaries from `HashMap`s, whose order changes from one run to the
/// next: the same document came out with its objects numbered differently.  The keys of every
/// dictionary are therefore sorted, and the objects renumbered in the order in which a walk
/// from the trailer meets them.
fn canonicalize(pdf: &mut printpdf::lopdf::Document) {
    use printpdf::lopdf::{Dictionary, Object, ObjectId};
    use std::collections::{BTreeMap, HashMap};

    fn sort_dict(d: &mut Dictionary) {
        for (_, v) in d.iter_mut() {
            sort_object(v);
        }
        let mut entries: Vec<(Vec<u8>, Object)> =
            d.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        let mut sorted = Dictionary::new();
        for (k, v) in entries {
            sorted.set(k, v);
        }
        *d = sorted;
    }

    fn sort_object(o: &mut Object) {
        match o {
            Object::Dictionary(d) => sort_dict(d),
            Object::Stream(s) => sort_dict(&mut s.dict),
            Object::Array(a) => a.iter_mut().for_each(sort_object),
            _ => {}
        }
    }

    fn remap(o: &mut Object, map: &HashMap<ObjectId, ObjectId>) {
        match o {
            Object::Reference(id) => {
                if let Some(new) = map.get(id) {
                    *id = *new;
                }
            }
            Object::Dictionary(d) => d.iter_mut().for_each(|(_, v)| remap(v, map)),
            Object::Stream(s) => s.dict.iter_mut().for_each(|(_, v)| remap(v, map)),
            Object::Array(a) => a.iter_mut().for_each(|v| remap(v, map)),
            _ => {}
        }
    }

    sort_dict(&mut pdf.trailer);
    for o in pdf.objects.values_mut() {
        sort_object(o);
    }

    // The walk, in an explicit stack to keep the order of a depth-first search.
    let mut order: Vec<ObjectId> = Vec::new();
    let mut seen: HashMap<ObjectId, ()> = HashMap::new();
    let mut roots = Vec::new();
    pdf.trailer
        .iter()
        .for_each(|(_, v)| collect_references(v, &mut roots));
    let mut stack: Vec<ObjectId> = roots.into_iter().rev().collect();
    while let Some(id) = stack.pop() {
        if seen.insert(id, ()).is_some() {
            continue;
        }
        if let Some(obj) = pdf.objects.get(&id) {
            order.push(id);
            let mut children = Vec::new();
            collect_references(obj, &mut children);
            stack.extend(children.into_iter().rev());
        }
    }
    let map: HashMap<ObjectId, ObjectId> = order
        .iter()
        .enumerate()
        .map(|(i, id)| (*id, (i as u32 + 1, 0)))
        .collect();
    let mut objects = BTreeMap::new();
    for (old, mut obj) in std::mem::take(&mut pdf.objects) {
        if let Some(new) = map.get(&old) {
            remap(&mut obj, &map);
            objects.insert(*new, obj);
        }
    }
    pdf.objects = objects;
    pdf.trailer.iter_mut().for_each(|(_, v)| remap(v, &map));
    pdf.max_id = order.len() as u32;
}

/// A page of a PDF document.
///
/// This is a wrapper around a [`printpdf::PdfPageReference`][].
///
/// [`printpdf::PdfPageReference`]: https://docs.rs/printpdf/0.3.2/printpdf/types/pdf_page/struct.PdfPageReference.html
pub struct Page {
    page: printpdf::PdfPageReference,
    size: Size,
    layers: Layers,
}

impl Page {
    fn new(
        page: printpdf::PdfPageReference,
        layer: printpdf::PdfLayerReference,
        size: Size,
    ) -> Page {
        Page {
            page,
            size,
            layers: Layers::new(layer),
        }
    }

    /// Adds a new layer with the given name to the page.
    pub fn add_layer(&mut self, name: impl Into<String>) {
        let layer = self.page.add_layer(name);
        self.layers.push(layer);
    }

    /// Returns the number of layers on this page.
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    /// Returns a layer of this page.
    pub fn get_layer(&self, idx: usize) -> Option<Layer<'_>> {
        self.layers.get(idx).map(|l| Layer::new(self, l))
    }

    /// Returns the first layer of this page.
    pub fn first_layer(&self) -> Layer<'_> {
        Layer::new(self, self.layers.first())
    }

    /// Returns the last layer of this page.
    pub fn last_layer(&self) -> Layer<'_> {
        Layer::new(self, self.layers.last())
    }

    fn next_layer(&self, layer: &printpdf::PdfLayerReference) -> Layer<'_> {
        let layer = self.layers.next(layer).unwrap_or_else(|| {
            let layer = self
                .page
                .add_layer(format!("Layer {}", self.layers.len() + 1));
            self.layers.push(layer)
        });
        Layer::new(self, layer)
    }
}

#[derive(Debug)]
struct Layers(cell::RefCell<Vec<rc::Rc<LayerData>>>);

impl Layers {
    pub fn new(layer: printpdf::PdfLayerReference) -> Self {
        Self(vec![LayerData::from(layer).into()].into())
    }

    pub fn len(&self) -> usize {
        self.0.borrow().len()
    }

    pub fn first(&self) -> rc::Rc<LayerData> {
        self.0.borrow().first().unwrap().clone()
    }

    pub fn last(&self) -> rc::Rc<LayerData> {
        self.0.borrow().last().unwrap().clone()
    }

    pub fn get(&self, idx: usize) -> Option<rc::Rc<LayerData>> {
        self.0.borrow().get(idx).cloned()
    }

    pub fn push(&self, layer: printpdf::PdfLayerReference) -> rc::Rc<LayerData> {
        let layer_data = rc::Rc::from(LayerData::from(layer));
        self.0.borrow_mut().push(layer_data.clone());
        layer_data
    }

    pub fn next(&self, layer: &printpdf::PdfLayerReference) -> Option<rc::Rc<LayerData>> {
        self.0
            .borrow()
            .iter()
            .skip_while(|l| l.layer.layer != layer.layer)
            .nth(1)
            .cloned()
    }
}

/// A layer of a page of a PDF document.
///
/// This is a wrapper around a [`printpdf::PdfLayerReference`][].
///
/// [`printpdf::PdfLayerReference`]: https://docs.rs/printpdf/0.3.2/printpdf/types/pdf_layer/struct.PdfLayerReference.html
#[derive(Clone)]
pub struct Layer<'p> {
    page: &'p Page,
    data: rc::Rc<LayerData>,
}

impl<'p> Layer<'p> {
    fn new(page: &'p Page, data: rc::Rc<LayerData>) -> Layer<'p> {
        Layer { page, data }
    }

    /// Returns the underlying `PdfLayerReference` for this layer.
    pub fn layer(&self) -> &printpdf::PdfLayerReference {
        &self.data.layer
    }

    /// Returns the next layer of this page.
    ///
    /// If this layer is not the last layer, the existing next layer is used.  If it is the last
    /// layer, a new layer is created and added to the page.
    pub fn next(&self) -> Layer<'p> {
        self.page.next_layer(&self.data.layer)
    }

    /// Returns a drawable area for this layer.
    pub fn area(&self) -> Area<'p> {
        Area::new(self.clone(), Position::default(), self.page.size)
    }

    /// Places a raster image whose lower left corner is at `position` and that measures
    /// `width` x `height` (PuppetPaie modification: replaces genpdfi's scale/rotation/dpi
    /// image placement).
    #[cfg(feature = "images")]
    fn add_raster(&self, image: printpdf::Image, position: LayerPosition, width: Mm, height: Mm) {
        let position = self.transform_position(position);
        let px_w = image.image.width.0 as f32;
        let px_h = image.image.height.0 as f32;
        let to_pt = |mm: Mm| printpdf::Pt::from(mm).0;
        image.add_to_layer(
            self.data.layer.clone(),
            printpdf::ImageTransform {
                translate_x: Some(position.x.into()),
                translate_y: Some(position.y.into()),
                rotate: None,
                // With a resolution of 72 dpi, one pixel measures one point.
                scale_x: Some(to_pt(width) / px_w),
                scale_y: Some(to_pt(height) / px_h),
                dpi: Some(72.0),
            },
        );
    }

    /// Places a vector image whose lower left corner is at `position`.  `natural` is the size
    /// (in points) that printpdf gives to the converted image.
    #[cfg(feature = "svg")]
    fn add_svg(
        &self,
        svg: printpdf::svg::Svg,
        position: LayerPosition,
        width: Mm,
        height: Mm,
    ) {
        let position = self.transform_position(position);
        let to_pt = |mm: Mm| printpdf::Pt::from(mm);
        let nat_w = svg.width.0 as f32;
        let nat_h = svg.height.0 as f32;
        svg.add_to_layer(
            &self.data.layer,
            printpdf::svg::SvgTransform {
                translate_x: Some(printpdf::Pt::from(position.x)),
                translate_y: Some(printpdf::Pt::from(position.y)),
                rotate: None,
                scale_x: Some(to_pt(width).0 / nat_w),
                scale_y: Some(to_pt(height).0 / nat_h),
                dpi: Some(72.0),
            },
        );
    }

    fn add_line_shape<I>(&self, points: I)
    where
        I: IntoIterator<Item = LayerPosition>,
    {
        let line_points: Vec<_> = points
            .into_iter()
            .map(|pos| (self.transform_position(pos).into(), false))
            .collect();
        let line = printpdf::Line {
            points: line_points,
            is_closed: false,
        };
        self.data.layer.add_line(line);
    }

    /// Remplit un rectangle (PuppetPaie : fond des lignes d'un tableau). `coin` est le coin haut
    /// gauche, en coordonnées de la couche.
    fn add_fill_rect(&self, coin: LayerPosition, width: Mm, height: Mm, color: Color) {
        self.set_fill_color(Some(color));
        let (x, y) = (coin.0.x, coin.0.y);
        let coins = [
            Position::new(x, y),
            Position::new(x + width, y),
            Position::new(x + width, y + height),
            Position::new(x, y + height),
        ];
        let anneau: Vec<_> = coins
            .iter()
            .map(|c| (self.transform_position(LayerPosition(*c)).into(), false))
            .collect();
        self.data.layer.add_polygon(printpdf::Polygon {
            rings: vec![anneau],
            mode: printpdf::path::PaintMode::Fill,
            winding_order: printpdf::path::WindingOrder::NonZero,
        });
    }

    fn set_fill_color(&self, color: Option<Color>) {
        if self.data.update_fill_color(color) {
            self.data
                .layer
                .set_fill_color(color.unwrap_or(Color::Rgb(0, 0, 0)).into());
        }
    }

    fn set_outline_thickness(&self, thickness: Mm) {
        if self.data.update_outline_thickness(thickness) {
            self.data
                .layer
                .set_outline_thickness(printpdf::Pt::from(thickness).0);
        }
    }

    fn set_outline_color(&self, color: Color) {
        if self.data.update_outline_color(color) {
            self.data.layer.set_outline_color(color.into());
        }
    }

    fn set_text_cursor(&self, cursor: LayerPosition) {
        let cursor = self.transform_position(cursor);
        self.data
            .layer
            .set_text_cursor(cursor.x.into(), cursor.y.into());
    }

    fn begin_text_section(&self) {
        self.data.layer.begin_text_section();
    }

    fn end_text_section(&self) {
        self.data.layer.end_text_section();
    }

    /// Writes text already encoded in Windows-1252 with the current font (PuppetPaie
    /// modification: printpdf's own encoding silently drops what it does not know).
    fn write_builtin_text(&self, bytes: &[u8]) {
        use printpdf::lopdf::content::Operation;
        use printpdf::lopdf::{Object, StringFormat};
        self.data.layer.add_operation(Operation::new(
            "Tj",
            vec![Object::String(bytes.to_vec(), StringFormat::Hexadecimal)],
        ));
    }

    /// Writes text of a font embedded as a subset (PuppetPaie): two-byte codes (the Unicode code
    /// point).  The operator `Tw` does not apply to two-byte codes, so a justified line is
    /// written as a `TJ` array: after each ordinary space, the shift that stretches it by
    /// `word_spacing` points.
    fn write_subset_text(&self, codes: &[u16], word_spacing: f32, font_size: f32) {
        use printpdf::lopdf::content::Operation;
        use printpdf::lopdf::{Object, StringFormat};
        let hex = |codes: &[u16]| {
            let mut octets = Vec::with_capacity(codes.len() * 2);
            for c in codes {
                octets.extend_from_slice(&c.to_be_bytes());
            }
            Object::String(octets, StringFormat::Hexadecimal)
        };
        if word_spacing == 0.0 || !codes.contains(&0x20) {
            self.data
                .layer
                .add_operation(Operation::new("Tj", vec![hex(codes)]));
            return;
        }
        // A positive shift moves the next glyph to the left: the stretch is its opposite.
        let shift = (-(word_spacing * 1000.0 / font_size) * 100.0).round() / 100.0;
        let mut list = Vec::new();
        let mut debut = 0;
        for (i, c) in codes.iter().enumerate() {
            if *c == 0x20 {
                list.push(hex(&codes[debut..=i]));
                list.push(Object::Real(shift));
                debut = i + 1;
            }
        }
        if debut < codes.len() {
            list.push(hex(&codes[debut..]));
        }
        self.data
            .layer
            .add_operation(Operation::new("TJ", vec![Object::Array(list)]));
    }

    fn add_line_break(&self) {
        self.data.layer.add_line_break();
    }

    fn set_line_height(&self, line_height: Mm) {
        self.data.layer.set_line_height(line_height.0);
    }

    fn set_font(&self, font: &printpdf::IndirectFontRef, font_size: f32) {
        self.data.layer.set_font(font, font_size);
    }

    fn write_positioned_codepoints<P, C>(&self, positions: P, codepoints: C)
    where
        P: IntoIterator<Item = i64>,
        C: IntoIterator<Item = u16>,
    {
        self.data
            .layer
            .write_positioned_codepoints(positions.into_iter().zip(codepoints.into_iter()));
    }

    /// Transforms the given position that is relative to the upper left corner of the layer to a
    /// position that is relative to the lower left corner of the layer (as used by `printpdf`).
    fn transform_position(&self, position: LayerPosition) -> UserSpacePosition {
        UserSpacePosition::from_layer(self, position)
    }

    /// Adds a link annotation to the layer.
    pub fn add_annotation(&mut self, annotation: printpdf::LinkAnnotation) {
        self.data.layer.add_link_annotation(annotation);
    }
}

#[derive(Debug)]
struct LayerData {
    layer: printpdf::PdfLayerReference,
    fill_color: cell::Cell<Color>,
    outline_color: cell::Cell<Color>,
    outline_thickness: cell::Cell<Mm>,
}

impl LayerData {
    pub fn update_fill_color(&self, color: Option<Color>) -> bool {
        let color = color.unwrap_or(Color::Rgb(0, 0, 0));
        self.fill_color.replace(color) != color
    }

    pub fn update_outline_color(&self, color: Color) -> bool {
        self.outline_color.replace(color) != color
    }

    pub fn update_outline_thickness(&self, thickness: Mm) -> bool {
        self.outline_thickness.replace(thickness) != thickness
    }
}

impl From<printpdf::PdfLayerReference> for LayerData {
    fn from(layer: printpdf::PdfLayerReference) -> Self {
        Self {
            layer,
            fill_color: Color::Rgb(0, 0, 0).into(),
            outline_color: Color::Rgb(0, 0, 0).into(),
            outline_thickness: Mm::from(printpdf::Pt(1.0)).into(),
        }
    }
}

/// A view on an area of a PDF layer that can be drawn on.
///
/// This struct provides access to the drawing methods of a [`printpdf::PdfLayerReference`][].  It
/// is defined by the layer that is drawn on and the origin and the size of the area.
///
/// [`printpdf::PdfLayerReference`]: https://docs.rs/printpdf/0.3.2/printpdf/types/pdf_layer/struct.PdfLayerReference.html
#[derive(Clone)]
pub struct Area<'p> {
    layer: Layer<'p>,
    origin: Position,
    size: Size,
}

impl<'p> Area<'p> {
    fn new(layer: Layer<'p>, origin: Position, size: Size) -> Area<'p> {
        Area {
            layer,
            origin,
            size,
        }
    }

    /// Returns a copy of this area on the next layer of the page.
    ///
    /// If this area is not on the last layer, the existing next layer is used.  If it is on the
    /// last layer, a new layer is created and added to the page.
    pub fn next_layer(&self) -> Self {
        let layer = self.layer.next();
        Self {
            layer,
            origin: self.origin,
            size: self.size,
        }
    }

    /// Reduces the size of the drawable area by the given margins.
    pub fn add_margins(&mut self, margins: impl Into<Margins>) {
        let margins = margins.into();
        self.origin.x += margins.left;
        self.origin.y += margins.top;
        self.size.width -= margins.left + margins.right;
        self.size.height -= margins.top + margins.bottom;
    }

    /// Returns the size of this area.
    pub fn size(&self) -> Size {
        self.size
    }

    /// Adds the given offset to the area, reducing the drawable area.
    pub fn add_offset(&mut self, offset: impl Into<Position>) {
        let offset = offset.into();
        self.origin.x += offset.x;
        self.origin.y += offset.y;
        self.size.width -= offset.x;
        self.size.height -= offset.y;
    }

    /// Sets the size of this area.
    pub fn set_size(&mut self, size: impl Into<Size>) {
        self.size = size.into();
    }

    /// Sets the width of this area.
    pub fn set_width(&mut self, width: Mm) {
        self.size.width = width;
    }

    /// Sets the height of this area.
    pub fn set_height(&mut self, height: Mm) {
        self.size.height = height;
    }

    /// Splits this area horizontally using the given weights.
    ///
    /// The returned vector has the same number of elements as the provided slice.  The width of
    /// the *i*-th area is *width \* weights[i] / total_weight*, where *width* is the width of this
    /// area, and *total_weight* is the sum of all given weights.
    pub fn split_horizontally(&self, weights: &[usize]) -> Vec<Area<'p>> {
        let total_weight: usize = weights.iter().sum();
        let factor = self.size.width / total_weight as f32;
        let widths = weights.iter().map(|weight| factor * *weight as f32);
        let mut offset = Mm(0.0);
        let mut areas = Vec::new();
        for width in widths {
            let mut area = self.clone();
            area.origin.x += offset;
            area.size.width = width;
            areas.push(area);
            offset += width;
        }
        areas
    }

    /// Inserts a raster image (PuppetPaie modification).  `position` is the upper left corner of
    /// the image, relative to the upper left corner of the area.
    #[cfg(feature = "images")]
    pub fn add_raster(&self, image: printpdf::Image, position: Position, width: Mm, height: Mm) {
        let lower_left = Position::new(position.x, position.y + height);
        self.layer
            .add_raster(image, self.position(lower_left), width, height);
    }

    /// Inserts a vector (SVG) image (PuppetPaie modification).  `position` is the upper left
    /// corner of the image, relative to the upper left corner of the area.
    #[cfg(feature = "svg")]
    pub fn add_svg(&self, svg: printpdf::svg::Svg, position: Position, width: Mm, height: Mm) {
        let lower_left = Position::new(position.x, position.y + height);
        self.layer
            .add_svg(svg, self.position(lower_left), width, height);
    }

    /// Draws a line with the given points and the given line style.
    ///
    /// The points are relative to the upper left corner of the area.
    pub fn draw_line<I>(&self, points: I, line_style: LineStyle)
    where
        I: IntoIterator<Item = Position>,
    {
        self.layer.set_outline_thickness(line_style.thickness());
        self.layer.set_outline_color(line_style.color());
        self.layer
            .add_line_shape(points.into_iter().map(|pos| self.position(pos)));
    }

    /// Remplit un rectangle de la couleur donnée (PuppetPaie). `position` est le coin haut gauche,
    /// relatif au coin haut gauche de la zone.
    pub fn fill_rect(&self, position: Position, size: Size, color: Color) {
        self.layer
            .add_fill_rect(self.position(position), size.width, size.height, color);
    }

    /// Tries to draw the given string at the given position and returns `true` if the area was
    /// large enough to draw the string.
    ///
    /// The font cache must contain the PDF font for the font set in the style.  The position is
    /// relative to the upper left corner of the area.
    pub fn print_str<S: AsRef<str>>(
        &self,
        font_cache: &fonts::FontCache,
        position: Position,
        style: Style,
        s: S,
    ) -> Result<bool, Error> {
        if let Some(mut section) =
            self.text_section(font_cache, position, style.metrics(font_cache))
        {
            section.print_str(s, style)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Creates a new text section at the given position if the text section fits in this area.
    ///
    /// The given style is only used to calculate the line height of the section.  The position is
    /// relative to the upper left corner of the area.  The font cache must contain the PDF font
    /// for all fonts printed with the text section.
    pub fn text_section<'f>(
        &self,
        font_cache: &'f fonts::FontCache,
        position: Position,
        metrics: fonts::Metrics,
    ) -> Option<TextSection<'f, 'p>> {
        let mut area = self.clone();
        area.add_offset(position);
        TextSection::new(font_cache, area, metrics)
    }

    /// Returns a position relative to the top left corner of this area.
    fn position(&self, position: Position) -> LayerPosition {
        LayerPosition::from_area(self, position)
    }

    /// Adds a clickable link to the document.
    ///
    /// The font cache must contain the PDF font for the font set in the style.  The position is
    /// relative to the upper left corner of the area.
    pub fn add_link<S: AsRef<str>>(
        &self,
        font_cache: &fonts::FontCache,
        position: Position,
        style: Style,
        text: S,
        uri: S,
    ) -> Result<bool, Error> {
        if let Some(mut section) =
            self.text_section(font_cache, position, style.metrics(font_cache))
        {
            section.add_link(text, uri, style)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

/// A text section that is drawn on an area of a PDF layer.
pub struct TextSection<'f, 'p> {
    font_cache: &'f fonts::FontCache,
    area: Area<'p>,
    is_first: bool,
    metrics: fonts::Metrics,
    font: Option<(printpdf::IndirectFontRef, f32)>,
    current_x_offset: Mm,
    cumulative_kerning: Mm,
    // `Tw` was written (it must be reset when the section ends).
    word_spacing: bool,
    // The word spacing asked for, in points (0 = none).
    word_spacing_pt: f32,
}

impl<'f, 'p> TextSection<'f, 'p> {
    fn new(
        font_cache: &'f fonts::FontCache,
        area: Area<'p>,
        metrics: fonts::Metrics,
    ) -> Option<TextSection<'f, 'p>> {
        if metrics.glyph_height > area.size.height {
            return None;
        }

        area.layer.begin_text_section();
        area.layer.set_line_height(metrics.line_height);

        Some(TextSection {
            font_cache,
            area,
            is_first: true,
            metrics,
            font: None,
            current_x_offset: Mm(0.0),
            cumulative_kerning: Mm(0.0),
            word_spacing: false,
            word_spacing_pt: 0.0,
        })
    }

    /// Sets the word spacing in points, for the rest of this section (PuppetPaie modification,
    /// used by justified paragraphs).  It applies to the ordinary space (U+0020): for built-in
    /// fonts by the operator `Tw`, written when the first text is, and reset when the section
    /// ends; for fonts embedded as subsets by a shift in the `TJ` array (`Tw` does not apply to
    /// two-byte codes).
    pub fn set_word_spacing(&mut self, points: f32) {
        self.word_spacing_pt = points;
    }

    /// Writes the text with the font (PuppetPaie modification): built-in fonts in Windows-1252,
    /// fonts embedded as subsets in their own codes.
    fn write_text(&mut self, font: fonts::Font, s: &str, font_size: f32) -> Result<(), Error> {
        if font.is_subset() {
            let codes = self.font_cache.subset_codes(font, s)?;
            self.area
                .layer
                .write_subset_text(&codes, self.word_spacing_pt, font_size);
        } else {
            let octets = encode_win1252(s)?;
            if self.word_spacing_pt != 0.0 && !self.word_spacing {
                self.area
                    .layer
                    .data
                    .layer
                    .set_word_spacing(self.word_spacing_pt);
                self.word_spacing = true;
            }
            self.area.layer.write_builtin_text(&octets);
        }
        Ok(())
    }

    fn set_text_cursor(&self, x_offset: Mm) {
        let cursor = self
            .area
            .position(Position::new(x_offset, self.metrics.ascent));
        self.area.layer.set_text_cursor(cursor);
    }

    fn set_font(&mut self, font: &printpdf::IndirectFontRef, font_size: f32) {
        let font_is_set = self
            .font
            .as_ref()
            .map(|(font, font_size)| (font, *font_size))
            .map(|data| data == (font, font_size))
            .unwrap_or_default();
        if !font_is_set {
            self.font = Some((font.clone(), font_size));
            self.area.layer.set_font(font, font_size);
        }
    }

    /// Tries to add a new line and returns `true` if the area was large enough to fit the new
    /// line.
    #[must_use]
    pub fn add_newline(&mut self) -> bool {
        if self.metrics.line_height > self.area.size.height {
            false
        } else {
            self.area.layer.add_line_break();
            self.area.add_offset((0, self.metrics.line_height));
            true
        }
    }

    /// Prints the given string with the given style.
    ///
    /// The font cache for this text section must contain the PDF font for the given style.
    pub fn print_str(&mut self, s: impl AsRef<str>, style: Style) -> Result<(), Error> {
        let font = style.font(self.font_cache);
        let s = s.as_ref();

        if self.is_first {
            if let Some(first_c) = s.chars().next() {
                let x_offset = style.char_left_side_bearing(self.font_cache, first_c) * -1.0;
                self.set_text_cursor(x_offset);
            }
            self.is_first = false;
        }

        let pdf_font = self
            .font_cache
            .get_pdf_font(font)
            .expect("Could not find PDF font in font cache");
        self.area.layer.set_fill_color(style.color());
        self.set_font(pdf_font, style.font_size());

        // Store starting position for underline/strikethrough
        let start_x = self.current_x_offset + self.cumulative_kerning;
        let text_width = style.text_width(self.font_cache, s);

        // For built-in fonts, emit text as whole words/strings to avoid character-by-character spacing
        if font.is_builtin() {
            // Use simple text emission for built-in fonts
            // This avoids the character-by-character positioning that causes spacing issues
            self.write_text(font, s, style.font_size())?;
        } else {
            // For embedded fonts, we still need precise positioning for proper kerning
            let kerning_positions = font.kerning(self.font_cache, s.chars());
            let positions = kerning_positions
                .clone()
                .into_iter()
                .map(|pos| (-pos * 1000.0) as i64);
            let codepoints = font.glyph_ids(&self.font_cache, s.chars());

            self.area
                .layer
                .write_positioned_codepoints(positions, codepoints);
        }

        // Draw underline if enabled
        if style.is_underline() {
            let line_thickness = Mm(style.font_size() as f32 * 0.05); // 5% of font size
            // Position just below baseline
            let underline_y = self.metrics.ascent + Mm(style.font_size() as f32 * 0.06);
            let line_style = LineStyle::new()
                .with_thickness(line_thickness)
                .with_color(style.color().unwrap_or(Color::Rgb(0, 0, 0)));

            self.area.draw_line(
                vec![
                    Position::new(start_x, underline_y),
                    Position::new(start_x + text_width, underline_y),
                ],
                line_style,
            );
        }

        // Draw strikethrough if enabled
        if style.is_strikethrough() {
            let line_thickness = Mm(style.font_size() as f32 * 0.05); // 5% of font size
            // Position at middle of x-height (roughly middle of lowercase letters)
            let strikethrough_y = self.metrics.ascent * 0.75;
            let line_style = LineStyle::new()
                .with_thickness(line_thickness)
                .with_color(style.color().unwrap_or(Color::Rgb(0, 0, 0)));

            self.area.draw_line(
                vec![
                    Position::new(start_x, strikethrough_y),
                    Position::new(start_x + text_width, strikethrough_y),
                ],
                line_style,
            );
        }

        // Update position tracking
        self.current_x_offset += text_width;

        // For built-in fonts, we don't need kerning tracking since PDF viewers handle it
        if !font.is_builtin() {
            let kerning_positions = font.kerning(self.font_cache, s.chars());
            let kerning_sum = Mm(kerning_positions.iter().sum::<f32>());
            self.cumulative_kerning += kerning_sum;
        }

        Ok(())
    }

    /// Adds a clickable link with the given text, URI, and style.
    ///
    /// The font cache for this text section must contain the PDF font for the given style.
    pub fn add_link(
        &mut self,
        text: impl AsRef<str>,
        uri: impl AsRef<str>,
        style: Style,
    ) -> Result<(), Error> {
        let font = style.font(self.font_cache);
        let text = text.as_ref();
        let uri = uri.as_ref();

        let kerning_positions: Vec<f32> = font.kerning(self.font_cache, text.chars());

        // Get current cursor position, including all accumulated offsets
        let start_x = self.current_x_offset + self.cumulative_kerning;
        let current_pos = self.area.position(Position::new(start_x, 0.0));

        let pdf_pos = self.area.layer.transform_position(current_pos);
        let text_width = style.text_width(self.font_cache, text);
        let rect = printpdf::Rect::new(
            printpdf::Mm(pdf_pos.x.0),                                     // left
            printpdf::Mm(pdf_pos.y.0 - font.ascent(style.font_size()).0),  // bottom
            printpdf::Mm(pdf_pos.x.0 + text_width.0),                      // right
            printpdf::Mm(pdf_pos.y.0 + font.descent(style.font_size()).0), // top
        );

        let annotation = printpdf::LinkAnnotation::new(
            rect,
            Some(printpdf::BorderArray::Solid([0.0, 0.0, 0.0])), // No border
            Some(printpdf::ColorArray::Transparent),             // Transparent color
            printpdf::Actions::uri(uri.to_string()),
            None,
        );
        self.area.layer.add_annotation(annotation);

        // Handle first character positioning
        if self.is_first {
            if let Some(first_c) = text.chars().next() {
                let x_offset = style.char_left_side_bearing(self.font_cache, first_c) * -1.0;
                self.set_text_cursor(x_offset);
            }
            self.is_first = false;
        }

        let positions = kerning_positions
            .clone()
            .into_iter()
            .map(|pos| (-pos * 1000.0) as i64);

        let codepoints = if font.is_builtin() {
            Vec::new()
        } else {
            font.glyph_ids(&self.font_cache, text.chars())
        };

        let pdf_font = self
            .font_cache
            .get_pdf_font(font)
            .expect("Could not find PDF font in font cache");

        self.area.layer.set_fill_color(style.color());
        self.set_font(pdf_font, style.font_size());

        // For built-in fonts, emit text as whole words/strings to avoid character-by-character spacing
        if font.is_builtin() {
            // Use simple text emission for built-in fonts
            // This avoids the character-by-character positioning that causes spacing issues
            self.write_text(font, text, style.font_size())?;
        } else {
            // For embedded fonts, we still need precise positioning for proper kerning
            self.area
                .layer
                .write_positioned_codepoints(positions, codepoints);
        }

        // Draw underline if enabled
        if style.is_underline() {
            let line_thickness = Mm(style.font_size() as f32 * 0.05); // 5% of font size
            // Position just below baseline
            let underline_y = self.metrics.ascent + Mm(style.font_size() as f32 * 0.06);
            let line_style = LineStyle::new()
                .with_thickness(line_thickness)
                .with_color(style.color().unwrap_or(Color::Rgb(0, 0, 0)));

            self.area.draw_line(
                vec![
                    Position::new(start_x, underline_y),
                    Position::new(start_x + text_width, underline_y),
                ],
                line_style,
            );
        }

        // Draw strikethrough if enabled
        if style.is_strikethrough() {
            let line_thickness = Mm(style.font_size() as f32 * 0.05); // 5% of font size
            // Position at middle of x-height (roughly middle of lowercase letters)
            let strikethrough_y = self.metrics.ascent * 0.75;
            let line_style = LineStyle::new()
                .with_thickness(line_thickness)
                .with_color(style.color().unwrap_or(Color::Rgb(0, 0, 0)));

            self.area.draw_line(
                vec![
                    Position::new(start_x, strikethrough_y),
                    Position::new(start_x + text_width, strikethrough_y),
                ],
                line_style,
            );
        }

        // Update position tracking
        self.current_x_offset += text_width;

        // For built-in fonts, we don't need kerning tracking since PDF viewers handle it
        if !font.is_builtin() {
            let kerning_sum = Mm(kerning_positions.iter().sum::<f32>());
            self.cumulative_kerning += kerning_sum;
        }

        Ok(())
    }
}

impl<'f, 'p> Drop for TextSection<'f, 'p> {
    fn drop(&mut self) {
        if self.word_spacing {
            self.area.layer.data.layer.set_word_spacing(0.0);
        }
        self.area.layer.end_text_section();
    }
}

/// Encodes the given string in Windows-1252 (the `WinAnsiEncoding` of the PDF standard fonts),
/// returning an error naming the first character that the encoding does not have.
///
/// Modification PuppetPaie: own table instead of lopdf's, which dropped unknown characters
/// without a word.
pub fn encode_win1252(s: &str) -> Result<Vec<u8>, Error> {
    s.chars()
        .map(|c| {
            win1252_byte(c).ok_or_else(|| {
                Error::new(
                    format!(
                        "Character {:?} (U+{:04X}) is not in the Windows-1252 encoding of the built-in fonts",
                        c, c as u32
                    ),
                    ErrorKind::UnsupportedEncoding,
                )
            })
        })
        .collect()
}

/// Returns the Windows-1252 byte of a character, if it has one.
pub fn win1252_byte(c: char) -> Option<u8> {
    let u = c as u32;
    match u {
        0x20..=0x7E | 0xA0..=0xFF => Some(u as u8),
        _ => {
            const HIGH: [(u32, u8); 27] = [
                (0x20AC, 0x80),
                (0x201A, 0x82),
                (0x0192, 0x83),
                (0x201E, 0x84),
                (0x2026, 0x85),
                (0x2020, 0x86),
                (0x2021, 0x87),
                (0x02C6, 0x88),
                (0x2030, 0x89),
                (0x0160, 0x8A),
                (0x2039, 0x8B),
                (0x0152, 0x8C),
                (0x017D, 0x8E),
                (0x2018, 0x91),
                (0x2019, 0x92),
                (0x201C, 0x93),
                (0x201D, 0x94),
                (0x2022, 0x95),
                (0x2013, 0x96),
                (0x2014, 0x97),
                (0x02DC, 0x98),
                (0x2122, 0x99),
                (0x0161, 0x9A),
                (0x203A, 0x9B),
                (0x0153, 0x9C),
                (0x017E, 0x9E),
                (0x0178, 0x9F),
            ];
            HIGH.iter().find(|(cp, _)| *cp == u).map(|(_, b)| *b)
        }
    }
}
