// Fichier modifié pour PuppetPaie (copie locale de genpdfi 0.2.7, Apache-2.0 OU MIT) :
// voir MODIFICATIONS.md à la racine de cette copie.
//! Polices embarquées en sous-ensemble (modification PuppetPaie, nouveau module).
//!
//! printpdf embarque la police entière (350 Ko par fichier de Liberation) et une table
//! `ToUnicode` de tous ses glyphes — en parcourant tout l'espace Unicode, ce qui prend 70 ms par
//! police. On lui donne donc à la place une **doublure** (`doublure`) : la police réduite au seul
//! `.notdef`, marquée d'un numéro. `embed_subsets` la retrouve dans le fichier écrit et réécrit
//! la police pour que le PDF ne porte que les glyphes réellement imprimés :
//!
//! - le texte est écrit avec des codes de deux octets qui sont le **point de code Unicode**
//!   (`Identity-H`) ; `CIDToGIDMap` les traduit vers les glyphes du sous-ensemble ;
//! - la table `ToUnicode` rend le code lui-même : le texte extrait est exactement le texte
//!   imprimé (l'espace insécable reste insécable) ;
//! - les glyphes sont numérotés dans l'ordre des caractères (triés), et le nom du sous-ensemble
//!   (`ABCDEF+NomPostScript`) est tiré d'une empreinte du nom de la police et des caractères :
//!   même texte, mêmes octets.

use std::collections::BTreeSet;
use std::convert::TryInto;
use std::sync::Arc;

use printpdf::lopdf::{Dictionary, Document, Object, ObjectId, Stream, StringFormat};

use crate::error::{Error, ErrorKind};

fn interne(message: String) -> Error {
    Error::new(message, ErrorKind::Internal)
}

/// Une police en sous-ensemble : la doublure donnée à printpdf, le fichier de police entier et
/// les caractères imprimés (vide : aucun texte ne l'a employée).
pub struct SubsetFace {
    /// La doublure (`doublure`).
    pub stub: Vec<u8>,
    /// Le fichier de police (TTF).
    pub data: Arc<Vec<u8>>,
    /// Les caractères imprimés, triés.
    pub chars: BTreeSet<char>,
}

/// La doublure du fichier de police `donnees`, numéro `numero` : son seul `.notdef`, suivi d'une
/// marque (les octets en trop après les tables sont ignorés par les lecteurs de polices). Deux
/// polices n'ont jamais la même doublure, grâce au numéro.
pub(crate) fn doublure(donnees: &[u8], numero: u32) -> Result<Vec<u8>, Error> {
    let (mut octets, _) = decouper_ttf(donnees, &[])?;
    octets.extend_from_slice(b"\0puppetpaie-sous-ensemble\0");
    octets.extend_from_slice(&numero.to_be_bytes());
    Ok(octets)
}

/// Les trois objets que printpdf écrit pour une police externe : la police `Type0`, son
/// descripteur et son flux de police.
struct PolicePrintpdf {
    type0: ObjectId,
    descripteur: ObjectId,
    fichier: ObjectId,
    tounicode: ObjectId,
}

fn objets_de(pdf: &Document, id: ObjectId) -> Option<PolicePrintpdf> {
    let type0 = pdf.get_dictionary(id).ok()?;
    if type0.get(b"Subtype").ok()?.as_name().ok()? != b"Type0" {
        return None;
    }
    let descendante = match type0.get(b"DescendantFonts").ok()?.as_array().ok()?.first()? {
        Object::Dictionary(d) => d.clone(),
        Object::Reference(r) => pdf.get_dictionary(*r).ok()?.clone(),
        _ => return None,
    };
    let descripteur = descendante.get(b"FontDescriptor").ok()?.as_reference().ok()?;
    let fichier = pdf
        .get_dictionary(descripteur)
        .ok()?
        .get(b"FontFile2")
        .ok()?
        .as_reference()
        .ok()?;
    let tounicode = type0.get(b"ToUnicode").ok()?.as_reference().ok()?;
    Some(PolicePrintpdf {
        type0: id,
        descripteur,
        fichier,
        tounicode,
    })
}

/// Les octets du fichier de police que printpdf a embarqué.
fn octets_du_flux(pdf: &Document, id: ObjectId) -> Option<Vec<u8>> {
    let flux = pdf.get_object(id).ok()?.as_stream().ok()?;
    Some(
        flux.decompressed_content()
            .unwrap_or_else(|_| flux.content.clone()),
    )
}

/// Empreinte FNV-1a de 64 bits, pour le nom du sous-ensemble.
fn empreinte(nom: &str, caracteres: &BTreeSet<char>) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    let mut octet = |b: u8| {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    };
    for b in nom.bytes() {
        octet(b);
    }
    for c in caracteres {
        for b in (*c as u32).to_be_bytes() {
            octet(b);
        }
    }
    h
}

fn nom_postscript(face: &ttf_parser::Face<'_>) -> String {
    face.names()
        .into_iter()
        .find(|n| n.name_id == ttf_parser::name_id::POST_SCRIPT_NAME && n.is_unicode())
        .and_then(|n| n.to_string())
        .filter(|n| !n.is_empty() && n.is_ascii() && !n.contains(char::is_whitespace))
        .unwrap_or_else(|| "Police".to_string())
}

/// La table `ToUnicode` : chaque code (le point de code) rend le caractère lui-même.
fn table_tounicode(caracteres: &BTreeSet<char>) -> String {
    let mut t = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n\
         /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
         /CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n\
         1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    let codes: Vec<u32> = caracteres.iter().map(|c| *c as u32).collect();
    for bloc in codes.chunks(100) {
        t.push_str(&format!("{} beginbfchar\n", bloc.len()));
        for c in bloc {
            t.push_str(&format!("<{:04X}> <{:04X}>\n", c, c));
        }
        t.push_str("endbfchar\n");
    }
    t.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    t
}

/// Coupe chaque doublure de `pdf` aux caractères imprimés de sa police. Une police externe qui
/// n'est la doublure d'aucune reste telle que printpdf l'a écrite.
pub(crate) fn embed_subsets(pdf: &mut Document, sous_ensembles: &[SubsetFace]) -> Result<(), Error> {
    if sous_ensembles.is_empty() {
        return Ok(());
    }
    let ids: Vec<ObjectId> = pdf.objects.keys().copied().collect();
    for id in ids {
        let objets = match objets_de(pdf, id) {
            Some(o) => o,
            None => continue,
        };
        let doublure = match octets_du_flux(pdf, objets.fichier) {
            Some(d) => d,
            None => continue,
        };
        let face = match sous_ensembles.iter().find(|f| f.stub == doublure) {
            Some(f) => f,
            None => continue,
        };
        if face.chars.is_empty() {
            // Une police qu'aucun texte n'emploie : elle sort du fichier.
            retirer_des_ressources(pdf, objets.type0);
        } else {
            couper(pdf, &objets, &face.data, &face.chars)?;
        }
    }
    Ok(())
}

/// Ajoute à un PDF **existant** la police `donnees` réduite aux caractères `caracteres` (même
/// sous-ensemble, mêmes codes `Identity-H` = point de code, même table `ToUnicode` que dans les PDF
/// de la fabrique) et rend l'objet de la police `Type0`, à mettre dans les `/Font` d'une page.
/// Les flux (police, `CIDToGIDMap`, `ToUnicode`) sont comprimés.
pub fn embarquer(pdf: &mut Document, donnees: &[u8], caracteres: &BTreeSet<char>) -> Result<ObjectId, Error> {
    if caracteres.is_empty() {
        return Err(interne("aucun caractère à embarquer".to_string()));
    }
    // Les trois objets que `couper` remplit, et la police composée qui les relie.
    let fichier = pdf.add_object(Object::Stream(Stream::new(Dictionary::new(), Vec::new())));
    let tounicode = pdf.add_object(Object::Stream(Stream::new(Dictionary::new(), Vec::new())));
    let descripteur = pdf.add_object(Object::Dictionary(Dictionary::new()));
    let mut t0 = Dictionary::new();
    t0.set("Type", Object::Name(b"Font".to_vec()));
    t0.set("Subtype", Object::Name(b"Type0".to_vec()));
    t0.set("BaseFont", Object::Name(b"Police".to_vec()));
    t0.set("Encoding", Object::Name(b"Identity-H".to_vec()));
    t0.set("ToUnicode", Object::Reference(tounicode));
    let type0 = pdf.add_object(Object::Dictionary(t0));
    let objets = PolicePrintpdf { type0, descripteur, fichier, tounicode };
    couper(pdf, &objets, donnees, caracteres)?;
    // La table des glyphes (`CIDToGIDMap`) est dans la police descendante, que `couper` vient d'écrire.
    let gid = pdf
        .get_dictionary(type0)
        .ok()
        .and_then(|d| d.get(b"DescendantFonts").ok())
        .and_then(|o| o.as_array().ok())
        .and_then(|a| a.first())
        .and_then(|o| o.as_dict().ok())
        .and_then(|d| d.get(b"CIDToGIDMap").ok())
        .and_then(|o| o.as_reference().ok());
    for id in [Some(fichier), Some(tounicode), gid].iter().flatten() {
        if let Ok(Object::Stream(s)) = pdf.get_object_mut(*id) {
            let _ = s.compress();
        }
    }
    Ok(type0)
}

/// Retire la police `id` des ressources (`/Font`) des pages : toute entrée de dictionnaire qui y
/// renvoie (seuls les dictionnaires de polices le font). Ses objets, que plus rien n'atteint,
/// sont élagués ensuite.
fn retirer_des_ressources(pdf: &mut Document, id: ObjectId) {
    fn parcourir(o: &mut Object, id: ObjectId) {
        match o {
            Object::Dictionary(d) => nettoyer(d, id),
            Object::Stream(s) => nettoyer(&mut s.dict, id),
            Object::Array(a) => a.iter_mut().for_each(|x| parcourir(x, id)),
            _ => {}
        }
    }
    fn nettoyer(d: &mut Dictionary, id: ObjectId) {
        let noms: Vec<Vec<u8>> = d
            .iter()
            .filter(|(_, v)| matches!(v, Object::Reference(r) if *r == id))
            .map(|(k, _)| k.clone())
            .collect();
        for n in noms {
            d.remove(&n);
        }
        for (_, v) in d.iter_mut() {
            parcourir(v, id);
        }
    }
    for objet in pdf.objects.values_mut() {
        parcourir(objet, id);
    }
}

fn couper(
    pdf: &mut Document,
    objets: &PolicePrintpdf,
    donnees: &[u8],
    caracteres: &BTreeSet<char>,
) -> Result<(), Error> {
    let face = ttf_parser::Face::parse(donnees, 0)
        .map_err(|e| interne(format!("Failed to parse font: {:?}", e)))?;
    let echelle = 1000.0 / face.units_per_em() as f32;
    let en_mille = |v: f32| (v * echelle).round() as i64;

    // Les glyphes dans l'ordre des caractères : le .notdef d'abord, puis un par caractère.
    let mut voulus: Vec<u16> = Vec::new();
    for c in caracteres {
        let origine = face
            .glyph_index(*c)
            .ok_or_else(|| interne(format!("Character {:?} (U+{:04X}) is not in the font", c, *c as u32)))?;
        voulus.push(origine.0);
    }
    let (sous_ensemble, nouveaux) = decouper_ttf(donnees, &voulus)?;
    // (code, glyphe d'origine, glyphe du sous-ensemble)
    let glyphes: Vec<(u16, u16, u16)> = caracteres
        .iter()
        .zip(voulus.iter().zip(nouveaux.iter()))
        .map(|(c, (o, n))| (*c as u32 as u16, *o, *n))
        .collect();

    // Le nom : six lettres tirées de l'empreinte, puis le nom PostScript.
    let ps = nom_postscript(&face);
    let mut h = empreinte(&ps, caracteres);
    let mut etiquette = String::new();
    for _ in 0..6 {
        etiquette.push((b'A' + (h % 26) as u8) as char);
        h /= 26;
    }
    let nom = format!("{}+{}", etiquette, ps);

    // Les largeurs, par code, en millièmes d'em.
    let mut largeurs: Vec<Object> = Vec::new();
    for (code, origine, _) in &glyphes {
        let l = face
            .glyph_hor_advance(ttf_parser::GlyphId(*origine))
            .unwrap_or(0) as f32;
        largeurs.push(Object::Integer(*code as i64));
        largeurs.push(Object::Array(vec![Object::Integer(en_mille(l))]));
    }

    // Code -> glyphe du sous-ensemble (deux octets par code, 0 pour les codes inemployés).
    let plus_grand = glyphes.iter().map(|(c, _, _)| *c as usize).max().unwrap_or(0);
    let mut cid_vers_gid = vec![0u8; 2 * (plus_grand + 1)];
    for (code, _, nouveau) in &glyphes {
        let i = 2 * *code as usize;
        cid_vers_gid[i..i + 2].copy_from_slice(&nouveau.to_be_bytes());
    }
    let mut dict_gid = Dictionary::new();
    dict_gid.set("Length", Object::Integer(cid_vers_gid.len() as i64));
    // Un flux d'à côté : son objet est ajouté, l'ancien est élagué avec le reste.
    let id_gid = pdf.add_object(Object::Stream(Stream::new(dict_gid, cid_vers_gid)));

    // Le fichier de police.
    let mut dict_fichier = Dictionary::new();
    dict_fichier.set("Length1", Object::Integer(sous_ensemble.len() as i64));
    pdf.objects.insert(
        objets.fichier,
        Object::Stream(Stream::new(dict_fichier, sous_ensemble)),
    );

    // La table ToUnicode.
    pdf.objects.insert(
        objets.tounicode,
        Object::Stream(Stream::new(
            Dictionary::new(),
            table_tounicode(caracteres).into_bytes(),
        )),
    );

    // Le descripteur.
    let bbox = face.global_bounding_box();
    let angle = face.italic_angle().unwrap_or(0.0);
    let mut drapeaux = 32; // jeu de caractères non symbolique
    if face.is_monospaced() {
        drapeaux |= 1;
    }
    if face.is_italic() || angle != 0.0 {
        drapeaux |= 64;
    }
    let hauteur_capitale = face.capital_height().map(|h| h as f32).unwrap_or(face.ascender() as f32);
    let mut descripteur = Dictionary::new();
    descripteur.set("Type", Object::Name(b"FontDescriptor".to_vec()));
    descripteur.set("FontName", Object::Name(nom.clone().into_bytes()));
    descripteur.set("Flags", Object::Integer(drapeaux));
    descripteur.set(
        "FontBBox",
        Object::Array(vec![
            Object::Integer(en_mille(bbox.x_min as f32)),
            Object::Integer(en_mille(bbox.y_min as f32)),
            Object::Integer(en_mille(bbox.x_max as f32)),
            Object::Integer(en_mille(bbox.y_max as f32)),
        ]),
    );
    descripteur.set("ItalicAngle", Object::Integer(angle.round() as i64));
    descripteur.set("Ascent", Object::Integer(en_mille(face.ascender() as f32)));
    descripteur.set("Descent", Object::Integer(en_mille(face.descender() as f32)));
    descripteur.set("CapHeight", Object::Integer(en_mille(hauteur_capitale)));
    descripteur.set("StemV", Object::Integer(if face.is_bold() { 140 } else { 80 }));
    descripteur.set("FontFile2", Object::Reference(objets.fichier));
    pdf.objects
        .insert(objets.descripteur, Object::Dictionary(descripteur));

    // La police descendante, puis la police composée.
    let mut systeme = Dictionary::new();
    systeme.set("Registry", Object::String(b"Adobe".to_vec(), StringFormat::Literal));
    systeme.set("Ordering", Object::String(b"Identity".to_vec(), StringFormat::Literal));
    systeme.set("Supplement", Object::Integer(0));
    let mut descendante = Dictionary::new();
    descendante.set("Type", Object::Name(b"Font".to_vec()));
    descendante.set("Subtype", Object::Name(b"CIDFontType2".to_vec()));
    descendante.set("BaseFont", Object::Name(nom.clone().into_bytes()));
    descendante.set("CIDSystemInfo", Object::Dictionary(systeme));
    descendante.set("FontDescriptor", Object::Reference(objets.descripteur));
    descendante.set("CIDToGIDMap", Object::Reference(id_gid));
    descendante.set("DW", Object::Integer(1000));
    descendante.set("W", Object::Array(largeurs));
    let type0 = pdf
        .get_object_mut(objets.type0)
        .and_then(Object::as_dict_mut)
        .map_err(|e| interne(format!("Failed to rewrite the font: {}", e)))?;
    type0.set("BaseFont", Object::Name(nom.into_bytes()));
    type0.set(
        "DescendantFonts",
        Object::Array(vec![Object::Dictionary(descendante)]),
    );
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Le découpage d'un fichier TrueType.
// ---------------------------------------------------------------------------------------------

fn u16_en(d: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_be_bytes(d.get(i..i + 2)?.try_into().ok()?))
}

fn u32_en(d: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(i..i + 4)?.try_into().ok()?))
}

/// La somme de contrôle d'une table (somme des mots de 32 bits, la fin complétée de zéros).
fn somme_de_controle(table: &[u8]) -> u32 {
    let mut somme: u32 = 0;
    for bloc in table.chunks(4) {
        let mut mot = [0u8; 4];
        mot[..bloc.len()].copy_from_slice(bloc);
        somme = somme.wrapping_add(u32::from_be_bytes(mot));
    }
    somme
}

/// Ce qu'un PDF demande à une police TrueType embarquée (`FontFile2`) : ces tables-là, et pas
/// `cmap`, `name`, `post`, `OS/2`, ni les tables de crénage et de substitution.
const TABLES_GARDEES: [&[u8; 4]; 3] = [b"cvt ", b"fpgm", b"prep"];

/// Coupe le fichier TrueType `donnees` aux glyphes `voulus` (numéros du fichier d'origine) : le
/// `.notdef` en premier, puis les `voulus` dans l'ordre donné, puis les composants des glyphes
/// composés. Rend le fichier découpé et, pour chaque glyphe voulu, son numéro dans le découpé.
///
/// Seules les polices à contours TrueType (`glyf`) sont prises en charge ; les contours, les
/// instructions de hinting et les tables `cvt `, `fpgm`, `prep` sont recopiés tels quels. Le
/// résultat ne dépend que de l'entrée : mêmes glyphes, mêmes octets.
pub(crate) fn decouper_ttf(donnees: &[u8], voulus: &[u16]) -> Result<(Vec<u8>, Vec<u16>), Error> {
    let illisible = || interne("Failed to read the TrueType font".to_string());
    let nombre = u16_en(donnees, 4).ok_or_else(illisible)? as usize;
    let table = |nom: &[u8; 4]| -> Option<&[u8]> {
        for i in 0..nombre {
            let e = 12 + 16 * i;
            if donnees.get(e..e + 4)? == nom {
                let debut = u32_en(donnees, e + 8)? as usize;
                let longueur = u32_en(donnees, e + 12)? as usize;
                return donnees.get(debut..debut.checked_add(longueur)?);
            }
        }
        None
    };
    if table(b"glyf").is_none() {
        return Err(interne("Only TrueType (glyf) fonts can be embedded".to_string()));
    }
    let head = table(b"head").ok_or_else(illisible)?;
    let hhea = table(b"hhea").ok_or_else(illisible)?;
    let maxp = table(b"maxp").ok_or_else(illisible)?;
    let loca = table(b"loca").ok_or_else(illisible)?;
    let glyf = table(b"glyf").ok_or_else(illisible)?;
    let hmtx = table(b"hmtx").ok_or_else(illisible)?;
    let long = u16_en(head, 50).ok_or_else(illisible)? != 0;
    let nb_glyphes = u16_en(maxp, 4).ok_or_else(illisible)? as usize;
    let nb_metriques = u16_en(hhea, 34).ok_or_else(illisible)? as usize;
    if nb_metriques == 0 {
        return Err(illisible());
    }

    // Les octets du glyphe `g` (vide pour un glyphe sans contour).
    let contours = |g: usize| -> Option<&[u8]> {
        if g >= nb_glyphes {
            return None;
        }
        let (a, b) = if long {
            (u32_en(loca, 4 * g)? as usize, u32_en(loca, 4 * g + 4)? as usize)
        } else {
            (2 * u16_en(loca, 2 * g)? as usize, 2 * u16_en(loca, 2 * g + 2)? as usize)
        };
        glyf.get(a..b)
    };
    // Largeur d'avance et approche gauche du glyphe `g`.
    let metriques = |g: usize| -> Option<(u16, i16)> {
        let avance = u16_en(hmtx, 4 * g.min(nb_metriques - 1))?;
        let approche = if g < nb_metriques {
            u16_en(hmtx, 4 * g + 2)?
        } else {
            u16_en(hmtx, 4 * nb_metriques + 2 * (g - nb_metriques))?
        };
        Some((avance, approche as i16))
    };

    // La numérotation : le .notdef, les glyphes voulus, puis les composants trouvés en chemin.
    let mut anciens: Vec<usize> = vec![0];
    let mut nouveau_de: std::collections::BTreeMap<usize, u16> = Default::default();
    nouveau_de.insert(0, 0);
    let mut ajouter = |g: usize, anciens: &mut Vec<usize>| -> u16 {
        *nouveau_de.entry(g).or_insert_with(|| {
            anciens.push(g);
            (anciens.len() - 1) as u16
        })
    };
    let nouveaux: Vec<u16> = voulus.iter().map(|g| ajouter(*g as usize, &mut anciens)).collect();
    let mut i = 0;
    let mut glyphes_ecrits: Vec<Vec<u8>> = Vec::new();
    while i < anciens.len() {
        let mut octets = contours(anciens[i]).ok_or_else(illisible)?.to_vec();
        // Un glyphe composé : ses composants entrent dans le découpé et leur numéro est réécrit.
        if octets.len() >= 10 && i16::from_be_bytes([octets[0], octets[1]]) < 0 {
            let mut p = 10;
            loop {
                let drapeaux = u16_en(&octets, p).ok_or_else(illisible)?;
                let g = u16_en(&octets, p + 2).ok_or_else(illisible)? as usize;
                let n = ajouter(g, &mut anciens);
                octets[p + 2..p + 4].copy_from_slice(&n.to_be_bytes());
                p += 4 + if drapeaux & 0x0001 != 0 { 4 } else { 2 };
                if drapeaux & 0x0008 != 0 {
                    p += 2;
                } else if drapeaux & 0x0040 != 0 {
                    p += 4;
                } else if drapeaux & 0x0080 != 0 {
                    p += 8;
                }
                if drapeaux & 0x0020 == 0 {
                    break;
                }
            }
        }
        glyphes_ecrits.push(octets);
        i += 1;
    }

    // glyf, loca (format long), hmtx.
    let mut glyf_neuf = Vec::new();
    let mut loca_neuf = Vec::new();
    let mut hmtx_neuf = Vec::new();
    for (octets, ancien) in glyphes_ecrits.iter().zip(anciens.iter()) {
        loca_neuf.extend_from_slice(&(glyf_neuf.len() as u32).to_be_bytes());
        glyf_neuf.extend_from_slice(octets);
        while glyf_neuf.len() % 4 != 0 {
            glyf_neuf.push(0);
        }
        let (avance, approche) = metriques(*ancien).ok_or_else(illisible)?;
        hmtx_neuf.extend_from_slice(&avance.to_be_bytes());
        hmtx_neuf.extend_from_slice(&approche.to_be_bytes());
    }
    loca_neuf.extend_from_slice(&(glyf_neuf.len() as u32).to_be_bytes());
    let total = anciens.len() as u16;
    let mut head_neuf = head.to_vec();
    head_neuf[8..12].copy_from_slice(&[0; 4]); // checkSumAdjustment
    head_neuf[50..52].copy_from_slice(&1u16.to_be_bytes());
    let mut hhea_neuf = hhea.to_vec();
    hhea_neuf[34..36].copy_from_slice(&total.to_be_bytes());
    let mut maxp_neuf = maxp.to_vec();
    maxp_neuf[4..6].copy_from_slice(&total.to_be_bytes());

    let mut tables: Vec<([u8; 4], Vec<u8>)> = vec![
        (*b"glyf", glyf_neuf),
        (*b"head", head_neuf),
        (*b"hhea", hhea_neuf),
        (*b"hmtx", hmtx_neuf),
        (*b"loca", loca_neuf),
        (*b"maxp", maxp_neuf),
    ];
    for nom in TABLES_GARDEES {
        if let Some(t) = table(nom) {
            tables.push((*nom, t.to_vec()));
        }
    }
    tables.sort_by(|a, b| a.0.cmp(&b.0));

    // Le répertoire des tables, puis les tables, chacune alignée sur 4 octets.
    let n = tables.len();
    let mut puissance = 1;
    let mut log = 0;
    while puissance * 2 <= n {
        puissance *= 2;
        log += 1;
    }
    let mut sortie = Vec::new();
    sortie.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    sortie.extend_from_slice(&(n as u16).to_be_bytes());
    sortie.extend_from_slice(&((puissance * 16) as u16).to_be_bytes());
    sortie.extend_from_slice(&(log as u16).to_be_bytes());
    sortie.extend_from_slice(&((n * 16 - puissance * 16) as u16).to_be_bytes());
    let mut debut = 12 + 16 * n;
    let mut corps = Vec::new();
    for (nom, t) in &tables {
        sortie.extend_from_slice(nom);
        sortie.extend_from_slice(&somme_de_controle(t).to_be_bytes());
        sortie.extend_from_slice(&(debut as u32).to_be_bytes());
        sortie.extend_from_slice(&(t.len() as u32).to_be_bytes());
        corps.extend_from_slice(t);
        while corps.len() % 4 != 0 {
            corps.push(0);
        }
        debut = 12 + 16 * n + corps.len();
    }
    sortie.extend_from_slice(&corps);
    // checkSumAdjustment : 0xB1B0AFBA moins la somme de tout le fichier.
    let ajustement = 0xB1B0_AFBAu32.wrapping_sub(somme_de_controle(&sortie));
    let (debut_head, _) = {
        let mut trouve = (0, 0);
        for i in 0..n {
            let e = 12 + 16 * i;
            if &sortie[e..e + 4] == b"head" {
                trouve = (
                    u32_en(&sortie, e + 8).unwrap() as usize,
                    u32_en(&sortie, e + 12).unwrap() as usize,
                );
            }
        }
        trouve
    };
    sortie[debut_head + 8..debut_head + 12].copy_from_slice(&ajustement.to_be_bytes());
    Ok((sortie, nouveaux))
}
