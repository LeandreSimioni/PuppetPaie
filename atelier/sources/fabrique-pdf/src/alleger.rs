//! Une image de la compagnie (logo, signature, tampon) allégée avant d'être posée : elle part dans chaque document
//! qui la porte, et chaque document part par courriel et se garde. Le dessin ne change pas à l'œil : l'image est
//! ramenée à la largeur qu'un document imprime, à `RESOLUTION`, puis écrite dans le format qui lui va (PNG pour un
//! dessin à aplats ou une image transparente, JPEG pour une photo) ; la résolution ne baisse, jusqu'à
//! `RESOLUTION_MIN`, que si l'objectif de poids l'exige. Un SVG reste tel quel (un dessin vectoriel, net à toute
//! taille). Une image déjà assez fine et assez légère reste telle quelle, octet pour octet.

use image::{DynamicImage, GenericImageView};

use crate::description::Refus;

/// La résolution visée, en points par pouce : ce qu'une imprimante de bureau rend d'une image posée.
pub const RESOLUTION: f64 = 200.0;
/// La résolution au-dessous de laquelle on ne descend pas pour tenir l'objectif de poids.
pub const RESOLUTION_MIN: f64 = 120.0;
/// Les qualités JPEG essayées, de la meilleure à la plus légère.
const QUALITES: [u8; 4] = [85, 75, 65, 55];

/// L'image allégée.
pub struct Allegee {
    /// Les octets à poser.
    pub octets: Vec<u8>,
    /// `png`, `jpeg` ou `svg`.
    pub format: &'static str,
    /// Sa largeur en pixels (0 pour un SVG).
    pub largeur: u32,
    /// Le poids de l'image reçue.
    pub avant: usize,
    /// L'image reçue est gardée telle quelle.
    pub telle_quelle: bool,
}

fn format_de(o: &[u8]) -> Option<&'static str> {
    if o.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if o.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpeg")
    } else if String::from_utf8_lossy(&o[..o.len().min(8192)]).to_ascii_lowercase().contains("<svg") {
        Some("svg")
    } else {
        None
    }
}

fn png(image: &DynamicImage) -> Result<Vec<u8>, Refus> {
    use image::codecs::png::{CompressionType, FilterType, PngEncoder};
    use image::ImageEncoder;
    let mut o = Vec::new();
    let image = match image {
        DynamicImage::ImageLuma8(_) | DynamicImage::ImageLumaA8(_) | DynamicImage::ImageRgb8(_) | DynamicImage::ImageRgba8(_) => image.clone(),
        i if i.color().has_alpha() => DynamicImage::ImageRgba8(i.to_rgba8()),
        i => DynamicImage::ImageRgb8(i.to_rgb8()),
    };
    PngEncoder::new_with_quality(&mut o, CompressionType::Best, FilterType::Adaptive)
        .write_image(image.as_bytes(), image.width(), image.height(), image.color())
        .map_err(|e| Refus(format!("l'image n'a pas pu être écrite en PNG ({})", e)))?;
    Ok(o)
}

fn jpeg(image: &DynamicImage, qualite: u8) -> Result<Vec<u8>, Refus> {
    let mut o = Vec::new();
    let image = if image.color().has_color() { DynamicImage::ImageRgb8(image.to_rgb8()) } else { DynamicImage::ImageLuma8(image.to_luma8()) };
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut o, qualite)
        .encode_image(&image)
        .map_err(|e| Refus(format!("l'image n'a pas pu être écrite en JPEG ({})", e)))?;
    Ok(o)
}

/// La largeur en pixels d'une image imprimée sur `points` à `resolution`.
fn pixels(points: f64, resolution: f64) -> u32 {
    (points / 72.0 * resolution).ceil().max(1.0) as u32
}

/// L'écriture de `image` (à sa taille) : PNG, sans perte, si l'image est transparente, si le PNG tient dans `objectif`
/// ou s'il pèse moins que le JPEG de qualité 85 ; sinon le JPEG de la meilleure qualité qui tient dans `objectif`, ou
/// le plus léger.
fn ecrire(image: &DynamicImage, objectif: usize) -> Result<(Vec<u8>, &'static str), Refus> {
    let transparente = image.color().has_alpha() && image.pixels().any(|(_, _, p)| p.0[3] < 255);
    // Une image opaque perd son canal alpha : il ne porterait que des 255, et un masque de plus dans chaque document.
    let opaque;
    let image = if !transparente && image.color().has_alpha() {
        opaque = if image.color().has_color() { DynamicImage::ImageRgb8(image.to_rgb8()) } else { DynamicImage::ImageLuma8(image.to_luma8()) };
        &opaque
    } else {
        image
    };
    let en_png = png(image)?;
    if transparente {
        return Ok((en_png, "png"));
    }
    let mut meilleur = jpeg(image, QUALITES[0])?;
    if en_png.len() <= meilleur.len() || en_png.len() <= objectif {
        return Ok((en_png, "png"));
    }
    for q in &QUALITES[1..] {
        if meilleur.len() <= objectif {
            break;
        }
        meilleur = jpeg(image, *q)?;
    }
    Ok((meilleur, "jpeg"))
}

/// `octets` (PNG, JPEG ou SVG) allégée pour être imprimée sur `largeur_points` au plus, à `objectif` octets au
/// plus si la résolution le permet (voir l'en-tête du module).
pub fn alleger(octets: &[u8], largeur_points: f64, objectif: usize) -> Result<Allegee, Refus> {
    let format = format_de(octets).ok_or_else(|| Refus("ni un PNG, ni un JPEG, ni un SVG".into()))?;
    if format == "svg" {
        return Ok(Allegee { octets: octets.to_vec(), format, largeur: 0, avant: octets.len(), telle_quelle: true });
    }
    let image = image::load_from_memory(octets).map_err(|e| Refus(format!("l'image est illisible ({})", e)))?;
    if image.width() == 0 || image.height() == 0 {
        return Err(Refus("l'image est vide".into()));
    }
    let cible = pixels(largeur_points, RESOLUTION);
    if image.width() <= cible + cible / 4 && octets.len() <= objectif {
        return Ok(Allegee { octets: octets.to_vec(), format, largeur: image.width(), avant: octets.len(), telle_quelle: true });
    }
    let mut resolution = RESOLUTION;
    loop {
        let cible = pixels(largeur_points, resolution);
        let reduite = if image.width() > cible { genpdfi::elements::reduire_a(image.clone(), cible) } else { image.clone() };
        let (o, f) = ecrire(&reduite, objectif)?;
        let plus_bas = resolution * 0.85;
        if o.len() <= objectif || plus_bas < RESOLUTION_MIN {
            // Une écriture plus lourde que l'image reçue, à la même taille, ne sert à rien.
            if o.len() >= octets.len() && reduite.width() == image.width() {
                return Ok(Allegee { octets: octets.to_vec(), format, largeur: image.width(), avant: octets.len(), telle_quelle: true });
            }
            return Ok(Allegee { octets: o, format: f, largeur: reduite.width(), avant: octets.len(), telle_quelle: false });
        }
        resolution = plus_bas;
    }
}
