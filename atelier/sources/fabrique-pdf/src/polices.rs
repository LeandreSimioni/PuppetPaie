//! Les polices : Liberation Sans, et pour les blocs à position fixe Liberation Serif et Mono,
//! embarquées dans le PDF **en sous-ensemble** : seuls les glyphes des caractères imprimés
//! (`genpdfi/src/pdf_subset.rs`), avec la table `ToUnicode` qui fait que le texte extrait est
//! exactement le texte imprimé.
//!
//! Liberation (licence SIL OFL 1.1, voir `polices/LICENCE-Liberation-OFL.txt`) a les métriques
//! d'Helvetica/Arial (Sans), de Times New Roman (Serif) et de Courier New (Mono) : la mise en page
//! est celle qu'avaient les polices standard, que le PDF nommait sans les embarquer. Tout
//! caractère que la police possède s'imprime (« Ł », « ő », « č », cyrillique, grec…) ; un
//! caractère qu'elle n'a pas est un refus (`mise_en_page::verifier_texte`).
//!
//! Le romain, le gras, l'italique et le gras italique de Sans sont quatre fichiers. Le Serif et
//! le Mono n'ont ni italique ni gras italique dans les blocs fixes (le format ne les offre pas) :
//! la case reprend le romain, ou le gras, du même fichier.

use std::sync::Arc;

use genpdfi::fonts::{FontData, FontFamily};

use crate::description::{refus, Refus};

// Comprimées par build.rs (zlib) : 0,4 Mo de moins dans le programme.
static REGULIER_Z: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/LiberationSans-Regular.ttf.z"));
static GRAS_Z: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/LiberationSans-Bold.ttf.z"));
static ITALIQUE_Z: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/LiberationSans-Italic.ttf.z"));
static GRAS_ITALIQUE_Z: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/LiberationSans-BoldItalic.ttf.z"));
static SERIF_Z: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/LiberationSerif-Regular.ttf.z"));
static SERIF_GRAS_Z: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/LiberationSerif-Bold.ttf.z"));
static MONO_Z: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/LiberationMono-Regular.ttf.z"));
static MONO_GRAS_Z: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/LiberationMono-Bold.ttf.z"));

fn decomprimer(z: &[u8]) -> Result<Arc<Vec<u8>>, Refus> {
    use std::io::Read;
    let mut ttf = Vec::new();
    flate2::read::ZlibDecoder::new(z)
        .read_to_end(&mut ttf)
        .or_else(|e| refus(format!("police interne illisible : {}", e)))?;
    Ok(Arc::new(ttf))
}

fn police(donnees: &Arc<Vec<u8>>) -> Result<FontData, Refus> {
    FontData::new_subset(donnees.clone())
        .or_else(|e| refus(format!("police interne illisible : {}", e)))
}

/// Liberation Sans : romain, gras, italique, gras italique.
pub fn famille() -> Result<FontFamily<FontData>, Refus> {
    Ok(FontFamily {
        regular: police(&decomprimer(REGULIER_Z)?)?,
        bold: police(&decomprimer(GRAS_Z)?)?,
        italic: police(&decomprimer(ITALIQUE_Z)?)?,
        bold_italic: police(&decomprimer(GRAS_ITALIQUE_Z)?)?,
    })
}

/// Le fichier de Liberation Sans romain (celui dont la fabrique tire ses sous-ensembles), pour
/// l'embarquer dans un PDF existant (`marquage.rs`).
pub fn sans_regulier() -> Result<Arc<Vec<u8>>, Refus> {
    decomprimer(REGULIER_Z)
}

/// Tous les fichiers de Liberation (Sans en quatre styles, Serif et Mono en romain et gras) : ce
/// que le texte d'un SVG peut demander quand on le pose dans un PDF (`formulaire.rs`). Aucune
/// police du système n'est lue : le même SVG donne le même dessin sur tous les postes.
pub fn toutes() -> Result<Vec<Arc<Vec<u8>>>, Refus> {
    [REGULIER_Z, GRAS_Z, ITALIQUE_Z, GRAS_ITALIQUE_Z, SERIF_Z, SERIF_GRAS_Z, MONO_Z, MONO_GRAS_Z]
        .iter()
        .map(|z| decomprimer(z))
        .collect()
}

/// Liberation Serif (Times), pour les textes à position fixe. L'italique n'est pas offert par
/// les blocs ; la case sert de repli (le romain, le gras).
pub fn famille_times() -> Result<FontFamily<FontData>, Refus> {
    let regulier = decomprimer(SERIF_Z)?;
    let gras = decomprimer(SERIF_GRAS_Z)?;
    Ok(FontFamily {
        regular: police(&regulier)?,
        bold: police(&gras)?,
        italic: police(&regulier)?,
        bold_italic: police(&gras)?,
    })
}

/// Liberation Mono (Courier) : chasse fixe de 0,6 em, gras compris.
pub fn famille_courier() -> Result<FontFamily<FontData>, Refus> {
    let regulier = decomprimer(MONO_Z)?;
    let gras = decomprimer(MONO_GRAS_Z)?;
    Ok(FontFamily {
        regular: police(&regulier)?,
        bold: police(&gras)?,
        italic: police(&regulier)?,
        bold_italic: police(&gras)?,
    })
}

/// Les largeurs d'une police standard (Helvetica, Times, Courier et leurs variantes) qu'un PDF
/// cite sans `/Widths` : celles de Liberation, qui a les métriques de ces polices. Sert à
/// l'extraction de texte (`texte.rs`) et au marquage (`marquage.rs`).
pub struct Metrique {
    donnees: Arc<Vec<u8>>,
}

impl Metrique {
    /// La métrique de la police standard que `base_font` nomme (`None` : aucune police connue).
    pub fn pour(base_font: &str) -> Option<Metrique> {
        let n = base_font.to_ascii_lowercase();
        let n = n.rsplit('+').next().unwrap_or("");
        let gras = n.contains("bold") || n.contains("black") || n.contains("heavy");
        let z = if n.contains("times") || (n.contains("serif") && !n.contains("sans")) {
            if gras { SERIF_GRAS_Z } else { SERIF_Z }
        } else if n.contains("courier") || n.contains("mono") || n.contains("consolas") {
            if gras { MONO_GRAS_Z } else { MONO_Z }
        } else if n.contains("helvetica") || n.contains("arial") || n.contains("sans") || n.is_empty() {
            if gras { GRAS_Z } else { REGULIER_Z }
        } else {
            return None;
        };
        Some(Metrique { donnees: decomprimer(z).ok()? })
    }

    /// La largeur du caractère, en fraction d'em (`None` : la police ne le dessine pas).
    pub fn largeur(&self, c: char) -> Option<f64> {
        let face = ttf_parser::Face::parse(&self.donnees, 0).ok()?;
        let g = face.glyph_index(c)?;
        Some(face.glyph_hor_advance(g)? as f64 / face.units_per_em() as f64)
    }
}
