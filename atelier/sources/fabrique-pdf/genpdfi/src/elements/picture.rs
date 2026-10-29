// Fichier modifié pour PuppetPaie (copie locale de genpdfi 0.2.7, Apache-2.0 OU MIT) :
// voir MODIFICATIONS.md à la racine de cette copie.
//! Images : PNG, JPEG et SVG.
//!
//! Modification PuppetPaie : ce module remplace l'élément `Image` de genpdfi 0.2.7 (voir
//! MODIFICATIONS.md).  Différences :
//!
//! - le format se reconnaît à l'**octet de tête** du contenu (signature PNG, signature JPEG, balise
//!   `<svg`), jamais à l'extension ;
//! - la taille se donne par la **largeur** ; la hauteur suit les proportions ;
//! - les PNG avec canal alpha sont acceptés (masque de transparence) ;
//! - les JPEG en niveaux de gris ou en couleurs sont recopiés tels quels dans le PDF (filtre
//!   `DCTDecode`), sans décompression ;
//! - une image plus fine que `RESOLUTION` à sa largeur posée y est ramenée ;
//! - le SVG est posé en vectoriel (`printpdf::svg`, donc svg2pdf) ;
//! - l'image qui ne tient pas dans la place restante passe à la page suivante.

#[cfg(feature = "svg")]
use usvg::TreeParsing as _;

use crate::error::{Error, ErrorKind};
use crate::{render, style};
use crate::{Alignment, Context, Element, Mm, Position, RenderResult, Size};

/// Le format d'une image, reconnu à son contenu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// PNG.
    Png,
    /// JPEG.
    Jpeg,
    /// SVG.
    Svg,
}

/// Reconnaît le format d'après le contenu : signature PNG, signature JPEG, sinon balise `<svg`
/// dans les premiers octets.
pub fn sniff(data: &[u8]) -> Option<Format> {
    if data.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some(Format::Png)
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(Format::Jpeg)
    } else {
        let head = &data[..data.len().min(4096)];
        if head.windows(4).any(|w| w.eq_ignore_ascii_case(b"<svg")) {
            Some(Format::Svg)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug)]
enum Kind {
    #[cfg(feature = "images")]
    Raster(printpdf::xobject::ImageXObject),
    #[cfg(feature = "svg")]
    Svg(printpdf::svg::Svg),
}

/// Une image posée à une largeur donnée.
#[derive(Clone, Debug)]
pub struct Picture {
    kind: Kind,
    width: Mm,
    height: Mm,
    alignment: Alignment,
    drawn: bool,
}

/// Le message des SVG qui demandent plus que des tracés et des aplats unis.
pub const SVG_NON_PRIS_EN_CHARGE: &str = "SVG avec dégradé, motif, transparence ou image intégrée : seuls les tracés et les aplats unis sont pris en charge";

fn invalid(msg: impl Into<String>) -> Error {
    Error::new(msg.into(), ErrorKind::InvalidData)
}

impl Picture {
    /// Crée une image à partir du contenu d'un fichier, à la largeur `width`.
    ///
    /// Le format est reconnu à l'octet de tête ; si rien ne correspond et que `svg_hint` est
    /// vrai (l'extension du fichier est `.svg`), le contenu est lu comme un SVG.
    pub fn from_bytes(data: &[u8], width: Mm, svg_hint: bool) -> Result<Picture, Error> {
        let format = match sniff(data) {
            Some(f) => f,
            None if svg_hint => Format::Svg,
            None => return Err(invalid("format d'image non reconnu (ni PNG, ni JPEG, ni SVG)")),
        };
        match format {
            Format::Png | Format::Jpeg => Self::raster(data, format, width),
            Format::Svg => Self::svg(data, width),
        }
    }

    #[cfg(not(feature = "images"))]
    fn raster(_data: &[u8], _format: Format, _width: Mm) -> Result<Picture, Error> {
        Err(invalid("PNG et JPEG non pris en charge (feature `images`)"))
    }

    #[cfg(feature = "images")]
    fn raster(data: &[u8], format: Format, width: Mm) -> Result<Picture, Error> {
        use printpdf::{ColorBits, ColorSpace, Px};

        let (image_format, name) = match format {
            Format::Png => (image::ImageFormat::Png, "PNG"),
            _ => (image::ImageFormat::Jpeg, "JPEG"),
        };
        // On décode toujours : un fichier tronqué ou corrompu est un refus, pas un blanc.
        let decoded = image::load_from_memory_with_format(data, image_format)
            .map_err(|e| invalid(format!("{} illisible : {}", name, e)))?;
        if decoded.width() == 0 || decoded.height() == 0 {
            return Err(invalid(format!("{} vide", name)));
        }
        // Une image plus fine que ce qui s'imprime est ramenée à `RESOLUTION` à sa largeur posée
        // (un logo de 1 200 pixels posé sur 26 mm pesait l'essentiel du bulletin).
        // Les proportions posées sont celles de l'original : la réduction arrondit les pixels.
        let proportions = decoded.height() as f32 / decoded.width() as f32;
        let (decoded, reduite) = reduire(decoded, width);
        let (w_px, h_px) = (decoded.width() as usize, decoded.height() as usize);

        let xobject = match (format, jpeg_components(data)) {
            // JPEG ramené à la résolution d'impression : réencodé en JPEG (filtre `DCTDecode`).
            (Format::Jpeg, Some(components)) if reduite && (components == 1 || components == 3) => {
                let (pixels, color_space, color) = if components == 1 {
                    (decoded.to_luma8().into_raw(), ColorSpace::Greyscale, image::ColorType::L8)
                } else {
                    (decoded.to_rgb8().into_raw(), ColorSpace::Rgb, image::ColorType::Rgb8)
                };
                let mut jpeg = Vec::new();
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, QUALITE_JPEG)
                    .encode(&pixels, w_px as u32, h_px as u32, color)
                    .map_err(|e| invalid(format!("JPEG : réduction impossible : {}", e)))?;
                printpdf::xobject::ImageXObject {
                    width: Px(w_px),
                    height: Px(h_px),
                    color_space,
                    bits_per_component: ColorBits::Bit8,
                    interpolate: true,
                    image_data: jpeg,
                    image_filter: Some(printpdf::xobject::ImageFilter::DCT),
                    smask: None,
                    clipping_bbox: None,
                }
            }
            // JPEG en niveaux de gris ou en couleurs, déjà à la bonne résolution : recopié tel quel.
            (Format::Jpeg, Some(components)) if components == 1 || components == 3 => {
                printpdf::xobject::ImageXObject {
                    width: Px(w_px),
                    height: Px(h_px),
                    color_space: if components == 1 {
                        ColorSpace::Greyscale
                    } else {
                        ColorSpace::Rgb
                    },
                    bits_per_component: ColorBits::Bit8,
                    interpolate: true,
                    image_data: data.to_vec(),
                    image_filter: Some(printpdf::xobject::ImageFilter::DCT),
                    smask: None,
                    clipping_bbox: None,
                }
            }
            // Le reste (PNG, JPEG CMJN ou exotique) : pixels décodés, ramenés à un format que
            // printpdf sait écrire (niveaux de gris, RVB, RVB + masque alpha, 8 bits).
            _ => {
                if decoded.color().has_alpha() {
                    // RVB opaque + plan alpha à part : le masque de transparence est attaché
                    // à l'écriture du fichier (voir `render::finish`).
                    let rgba = decoded.to_rgba8();
                    let mut rgb = Vec::with_capacity(w_px * h_px * 3);
                    let mut alpha = Vec::with_capacity(w_px * h_px);
                    for px in rgba.pixels() {
                        rgb.extend_from_slice(&px.0[..3]);
                        alpha.push(px.0[3]);
                    }
                    let marker = crate::render::register_alpha_plane(alpha);
                    printpdf::xobject::ImageXObject {
                        width: Px(w_px),
                        height: Px(h_px),
                        color_space: ColorSpace::Rgb,
                        bits_per_component: ColorBits::Bit8,
                        interpolate: true,
                        image_data: rgb,
                        image_filter: None,
                        smask: None,
                        clipping_bbox: Some(printpdf::CurTransMat::Translate(
                            printpdf::Pt(marker as f32),
                            printpdf::Pt(0.0),
                        )),
                    }
                } else {
                    let normalized = if decoded.color().has_color() {
                        image::DynamicImage::ImageRgb8(decoded.to_rgb8())
                    } else {
                        image::DynamicImage::ImageLuma8(decoded.to_luma8())
                    };
                    printpdf::Image::from_dynamic_image(&normalized).image
                }
            }
        };
        let height = width * proportions;
        Ok(Picture {
            kind: Kind::Raster(xobject),
            width,
            height,
            alignment: Alignment::Left,
            drawn: false,
        })
    }

    #[cfg(not(feature = "svg"))]
    fn svg(_data: &[u8], _width: Mm) -> Result<Picture, Error> {
        Err(invalid("SVG non pris en charge (feature `svg`)"))
    }

    #[cfg(feature = "svg")]
    fn svg(data: &[u8], width: Mm) -> Result<Picture, Error> {
        let text = std::str::from_utf8(data).map_err(|_| invalid("SVG illisible : pas de l'UTF-8"))?;
        if text.contains("<text") {
            return Err(invalid(
                "SVG avec du texte : le texte n'est pas pris en charge (à convertir en tracés)",
            ));
        }
        // Les proportions exactes viennent de usvg : printpdf arrondit la taille à l'entier.
        let tree = usvg::Tree::from_str(text, &usvg::Options::default())
            .map_err(|e| invalid(format!("SVG illisible : {}", e)))?;
        let (w, h) = (tree.size.width() as f32, tree.size.height() as f32);
        if !(w > 0.0 && h > 0.0) {
            return Err(invalid("SVG sans dimensions"));
        }
        let svg = printpdf::svg::Svg::parse(text)
            .map_err(|e| invalid(format!("SVG illisible : {}", e)))?;
        // printpdf ne recopie pas les objets auxquels renvoie le SVG converti (dégradés, motifs,
        // états de transparence, images intégrées) : une référence dans l'image en trahit un.
        if format!("{:?}", svg).contains(" 0 R") {
            return Err(invalid(SVG_NON_PRIS_EN_CHARGE));
        }
        Ok(Picture {
            kind: Kind::Svg(svg),
            width,
            height: width * (h / w),
            alignment: Alignment::Left,
            drawn: false,
        })
    }

    /// Sets the horizontal alignment inside the area (left by default).
    pub fn with_alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    /// Returns the width of the picture.
    pub fn width(&self) -> Mm {
        self.width
    }

    /// Returns the height of the picture.
    pub fn height(&self) -> Mm {
        self.height
    }

    /// Draws the picture with its upper left corner at `position` in `area`, without any
    /// flow logic (used for the logo of each page).
    pub fn draw_at(&self, area: &render::Area<'_>, position: Position) {
        match &self.kind {
            #[cfg(feature = "images")]
            Kind::Raster(x) => area.add_raster(
                printpdf::Image { image: x.clone() },
                position,
                self.width,
                self.height,
            ),
            #[cfg(feature = "svg")]
            Kind::Svg(svg) => area.add_svg(svg.clone(), position, self.width, self.height),
        }
    }
}

impl Element for Picture {
    fn render(
        &mut self,
        _context: &Context,
        area: render::Area<'_>,
        _style: style::Style,
    ) -> Result<RenderResult, Error> {
        if self.drawn {
            return Ok(RenderResult::default());
        }
        if self.height > area.size().height {
            // Ne tient pas ici : à la page suivante.
            return Ok(RenderResult {
                size: Size::new(0, 0),
                has_more: true,
            });
        }
        let x = match self.alignment {
            Alignment::Center => (area.size().width - self.width) / 2.0,
            Alignment::Right => area.size().width - self.width,
            _ => Mm(0.0),
        };
        self.draw_at(&area, Position::new(x, 0));
        self.drawn = true;
        Ok(RenderResult {
            size: Size::new(self.width, self.height),
            has_more: false,
        })
    }
}

/// La résolution, en pixels par pouce, à laquelle une image est ramenée à sa largeur posée : celle
/// de l'impression courante. Au-delà, les pixels ne s'impriment ni ne se voient, ils ne font que
/// peser.
pub const RESOLUTION: f32 = 300.0;

/// La qualité d'un JPEG réencodé après réduction (sur 100).
#[cfg(feature = "images")]
const QUALITE_JPEG: u8 = 90;

/// L'image ramenée à `RESOLUTION` à la largeur `width`, si elle la dépasse de plus d'un quart
/// (en deçà, l'écart ne vaut pas un rééchantillonnage) ; `true` si elle a été réduite. Les
/// proportions sont gardées. Une image avec transparence est réduite en couleurs prémultipliées :
/// la couleur d'un pixel transparent ne déteint pas sur le bord du dessin.
#[cfg(feature = "images")]
fn reduire(image: image::DynamicImage, width: Mm) -> (image::DynamicImage, bool) {
    use image::imageops::FilterType;
    // (le millième retire l'erreur d'arrondi des points convertis en millimètres : 48 pt font 200 pixels, pas 201)
    let cible = ((width.0 / 25.4) * RESOLUTION - 0.001).ceil().max(1.0) as u32;
    let (w, h) = (image.width(), image.height());
    if w <= cible + cible / 4 {
        return (image, false);
    }
    let hauteur = ((h as f64) * (cible as f64) / (w as f64)).round().max(1.0) as u32;
    if !image.color().has_alpha() {
        return (image.resize_exact(cible, hauteur, FilterType::Lanczos3), true);
    }
    let mut f = image.to_rgba32f();
    for p in f.pixels_mut() {
        let a = p.0[3];
        p.0[0] *= a;
        p.0[1] *= a;
        p.0[2] *= a;
    }
    let mut r = image::imageops::resize(&f, cible, hauteur, FilterType::Lanczos3);
    for p in r.pixels_mut() {
        let a = p.0[3].clamp(0.0, 1.0);
        for c in 0..3 {
            p.0[c] = if a > 0.0 { (p.0[c] / a).clamp(0.0, 1.0) } else { 0.0 };
        }
        p.0[3] = a;
    }
    let reduite = image::DynamicImage::ImageRgba32F(r);
    // Revenir aux huit bits par composante de l'original (couleur ou niveaux de gris).
    let reduite = if image.color().has_color() {
        image::DynamicImage::ImageRgba8(reduite.to_rgba8())
    } else {
        image::DynamicImage::ImageLumaA8(reduite.to_luma_alpha8())
    };
    (reduite, true)
}

/// Retourne le nombre de composantes d'un JPEG recopiable tel quel (DCT de base ou progressive,
/// 8 bits), ou `None` si ce n'en est pas un.
#[cfg(feature = "images")]
fn jpeg_components(data: &[u8]) -> Option<u8> {
    let mut i = 2; // après SOI
    while i + 4 <= data.len() {
        if data[i] != 0xFF {
            return None;
        }
        let marker = data[i + 1];
        match marker {
            0xFF => {
                i += 1;
                continue;
            }
            0x01 | 0xD0..=0xD8 => {
                i += 2;
                continue;
            }
            _ => {}
        }
        let len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
        if matches!(marker, 0xC0 | 0xC1 | 0xC2) {
            // SOFn : longueur, précision, hauteur, largeur, composantes.
            if i + 10 > data.len() || data[i + 4] != 8 {
                return None;
            }
            return Some(data[i + 9]);
        }
        if marker == 0xDA {
            return None;
        }
        i += 2 + len;
    }
    None
}
