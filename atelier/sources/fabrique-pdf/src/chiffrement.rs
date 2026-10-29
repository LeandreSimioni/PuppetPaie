//! Le chiffrement d'un PDF : `fabrique-pdf --verrouiller`, et la lecture d'un PDF ainsi verrouillé.
//!
//! C'est le chiffrement standard de PDF 2.0 (ISO 32000-2) : `/V 5`, `/R 6`, AES-256 en mode CBC
//! (`/CFM /AESV3`), clé de fichier de 32 octets tirée au hasard. Le mot de passe utilisateur est
//! vide (le fichier s'ouvre partout sans mot de passe) ; le mot de passe propriétaire est tiré au
//! hasard et jeté : personne ne peut plus lever les restrictions. Les permissions sont :
//! impression, copie et accessibilité, rien d'autre.
//!
//! Chaînes et flux sont chiffrés un à un (IV de 16 octets tiré au hasard, en tête ; bourrage
//! PKCS#7), à l'exception du dictionnaire `/Encrypt` et des flux de table de références.
//!
//! Le déchiffrement (`dechiffrer`) lit ce même chiffrement avec un mot de passe utilisateur donné
//! (vide, ici) ; pour les anciens chiffrements (RC4, `/V` 1 ou 2) il s'en remet à lopdf.

use aes::cipher::block_padding::{NoPadding, Pkcs7};
use aes::cipher::{BlockDecryptMut, BlockEncrypt, BlockEncryptMut, KeyInit, KeyIvInit};
use lopdf::{Dictionary, Document, Object, ObjectId, StringFormat};
use sha2::{Digest, Sha256, Sha384, Sha512};

use crate::description::Refus;

type CbcChiffre256 = cbc::Encryptor<aes::Aes256>;
type CbcDechiffre256 = cbc::Decryptor<aes::Aes256>;
type CbcChiffre128 = cbc::Encryptor<aes::Aes128>;

/// Les permissions, comme `PDF_PERM_PRINT | PDF_PERM_COPY | PDF_PERM_ACCESSIBILITY` de MuPDF :
/// bits 3 (imprimer), 5 (copier) et 10 (accessibilité), les bits réservés à 1 (7, 8 et 13 à 32).
/// Vaut -3372 en entier signé, comme dans les fichiers que PyMuPDF verrouillait.
pub const PERMISSIONS: u32 = 0xFFFF_F0C0 | 4 | 16 | 512;

fn refus<T>(m: impl Into<String>) -> Result<T, Refus> {
    Err(Refus(m.into()))
}

fn hasard(n: usize) -> Result<Vec<u8>, Refus> {
    let mut v = vec![0u8; n];
    getrandom::getrandom(&mut v).map_err(|e| Refus(format!("tirage au hasard impossible ({})", e)))?;
    Ok(v)
}

// ---------------------------------------------------------------------------------------------
// Les empreintes de mot de passe.
// ---------------------------------------------------------------------------------------------

/// L'algorithme 2.B de la norme ISO 32000-2 (révision 6) : SHA-256, puis au moins 64 tours de
/// AES-128-CBC dont la somme des 16 premiers octets (modulo 3) choisit SHA-256, 384 ou 512.
fn empreinte_r6(mot_de_passe: &[u8], sel: &[u8], u: &[u8]) -> Vec<u8> {
    let mut k: Vec<u8> = Sha256::new()
        .chain_update(mot_de_passe)
        .chain_update(sel)
        .chain_update(u)
        .finalize()
        .to_vec();
    let mut tour = 0usize;
    loop {
        let mut k1 = Vec::with_capacity(64 * (mot_de_passe.len() + k.len() + u.len()));
        for _ in 0..64 {
            k1.extend_from_slice(mot_de_passe);
            k1.extend_from_slice(&k);
            k1.extend_from_slice(u);
        }
        // k1 est de longueur multiple de 16 (64 répétitions) : pas de bourrage.
        let e = CbcChiffre128::new_from_slices(&k[..16], &k[16..32])
            .expect("clé et IV de 16 octets")
            .encrypt_padded_vec_mut::<NoPadding>(&k1);
        let reste: u32 = e[..16].iter().map(|b| *b as u32).sum::<u32>() % 3;
        k = match reste {
            0 => Sha256::digest(&e).to_vec(),
            1 => Sha384::digest(&e).to_vec(),
            _ => Sha512::digest(&e).to_vec(),
        };
        tour += 1;
        if tour >= 64 && (*e.last().unwrap_or(&0) as usize) <= tour - 32 {
            break;
        }
    }
    k.truncate(32);
    k
}

/// L'empreinte de la révision `r` (5 : un simple SHA-256, ancienne extension d'Adobe ; 6 : 2.B).
fn empreinte(r: i64, mot_de_passe: &[u8], sel: &[u8], u: &[u8]) -> Vec<u8> {
    if r >= 6 {
        empreinte_r6(mot_de_passe, sel, u)
    } else {
        Sha256::new().chain_update(mot_de_passe).chain_update(sel).chain_update(u).finalize().to_vec()
    }
}

// ---------------------------------------------------------------------------------------------
// Les chiffrements de données.
// ---------------------------------------------------------------------------------------------

/// AES-256-CBC, IV tiré au hasard et placé en tête, bourrage PKCS#7.
fn chiffrer_donnees(cle: &[u8], donnees: &[u8]) -> Result<Vec<u8>, Refus> {
    let iv = hasard(16)?;
    let mut sortie = iv.clone();
    sortie.extend(
        CbcChiffre256::new_from_slices(cle, &iv)
            .map_err(|_| Refus("clé de chiffrement invalide".into()))?
            .encrypt_padded_vec_mut::<Pkcs7>(donnees),
    );
    Ok(sortie)
}

/// L'inverse : les 16 premiers octets sont l'IV. Des données trop courtes ou mal bourrées sont
/// laissées telles quelles (un objet que l'auteur du fichier n'a pas chiffré).
fn dechiffrer_donnees(cle: &[u8], donnees: &[u8]) -> Vec<u8> {
    if donnees.len() < 32 || donnees.len() % 16 != 0 {
        return donnees.to_vec();
    }
    match CbcDechiffre256::new_from_slices(cle, &donnees[..16]) {
        Ok(d) => d.decrypt_padded_vec_mut::<Pkcs7>(&donnees[16..]).unwrap_or_else(|_| donnees.to_vec()),
        Err(_) => donnees.to_vec(),
    }
}

/// Applique `f` à chaque chaîne de l'objet, en profondeur, et aux flux : `fl` à leur contenu.
fn parcourir(
    o: &mut Object,
    chaine: &mut dyn FnMut(&[u8]) -> Result<Vec<u8>, Refus>,
    flux: &mut dyn FnMut(&[u8]) -> Result<Vec<u8>, Refus>,
) -> Result<(), Refus> {
    match o {
        // Le format (littéral ou hexadécimal) reste celui de la chaîne : l'écriture de lopdf
        // échappe ce qu'il faut dans une chaîne littérale, quels que soient les octets.
        Object::String(s, _) => *s = chaine(s)?,
        Object::Array(v) => {
            for x in v {
                parcourir(x, chaine, flux)?;
            }
        }
        Object::Dictionary(d) => {
            for (_, x) in d.iter_mut() {
                parcourir(x, chaine, flux)?;
            }
        }
        Object::Stream(s) => {
            for (_, x) in s.dict.iter_mut() {
                parcourir(x, chaine, flux)?;
            }
            let neuf = flux(&s.content)?;
            s.set_content(neuf);
        }
        _ => {}
    }
    Ok(())
}

fn est_de_structure(o: &Object) -> bool {
    matches!(o.type_name(), Ok(t) if t == "XRef" || t == "ObjStm" || t == "Linearized")
}

// ---------------------------------------------------------------------------------------------
// Verrouiller.
// ---------------------------------------------------------------------------------------------

/// Le PDF `octets`, chiffré en AES-256 (PDF 2.0, `/V 5 /R 6`), mot de passe utilisateur vide.
pub fn verrouiller(octets: &[u8]) -> Result<Vec<u8>, Refus> {
    let mut doc = crate::lecture::charger(octets)?;
    // Un fichier déjà chiffré (mot de passe vide) est d'abord ouvert : on le verrouille de neuf.
    dechiffrer(&mut doc, b"")?;
    if doc.get_pages().is_empty() {
        return refus("PDF sans page");
    }
    // Les flux de structure (objets compressés, table de références) sont réécrits par lopdf, et
    // leurs objets ont déjà été lus : on ne les garde pas.
    doc.objects.retain(|_, o| !est_de_structure(o));
    doc.compress();
    doc.reference_table.cross_reference_type = lopdf::xref::XrefType::CrossReferenceTable;
    doc.version = "2.0".to_string();
    doc.trailer.remove(b"Encrypt");

    // La clé de fichier, et tout ce qu'il faut pour que l'utilisateur (mot de passe vide) et le
    // propriétaire (mot de passe jeté) la retrouvent.
    let cle = hasard(32)?;
    let utilisateur: &[u8] = b"";
    let proprietaire = hasard(32)?;

    let sel_u = hasard(16)?;
    let (sel_valid_u, sel_cle_u) = sel_u.split_at(8);
    let mut u = empreinte_r6(utilisateur, sel_valid_u, b"");
    u.extend_from_slice(sel_valid_u);
    u.extend_from_slice(sel_cle_u);
    let ue = chiffrer_cle(&empreinte_r6(utilisateur, sel_cle_u, b""), &cle)?;

    let sel_o = hasard(16)?;
    let (sel_valid_o, sel_cle_o) = sel_o.split_at(8);
    let mut o = empreinte_r6(&proprietaire, sel_valid_o, &u);
    o.extend_from_slice(sel_valid_o);
    o.extend_from_slice(sel_cle_o);
    let oe = chiffrer_cle(&empreinte_r6(&proprietaire, sel_cle_o, &u), &cle)?;

    // /Perms : les permissions, une marque et 4 octets quelconques, en un bloc AES-256 (BCE).
    let mut perms = [0u8; 16];
    perms[0..4].copy_from_slice(&PERMISSIONS.to_le_bytes());
    perms[4..8].copy_from_slice(&[0xFF; 4]);
    perms[8] = b'T'; // /EncryptMetadata true
    perms[9..12].copy_from_slice(b"adb");
    perms[12..16].copy_from_slice(&hasard(4)?);
    let aes = aes::Aes256::new_from_slice(&cle).map_err(|_| Refus("clé de chiffrement invalide".into()))?;
    aes.encrypt_block((&mut perms).into());

    // Chaînes et flux, tous chiffrés (même les métadonnées, comme /EncryptMetadata true le dit).
    let mut erreur: Option<Refus> = None;
    for (_, objet) in doc.objects.iter_mut() {
        let mut chiffre = |d: &[u8]| chiffrer_donnees(&cle, d);
        let mut chiffre2 = |d: &[u8]| chiffrer_donnees(&cle, d);
        if let Err(e) = parcourir(objet, &mut chiffre, &mut chiffre2) {
            erreur = Some(e);
            break;
        }
    }
    if let Some(e) = erreur {
        return Err(e);
    }

    let mut cf = Dictionary::new();
    cf.set("AuthEvent", Object::Name(b"DocOpen".to_vec()));
    cf.set("CFM", Object::Name(b"AESV3".to_vec()));
    cf.set("Length", Object::Integer(32));
    let mut filtres = Dictionary::new();
    filtres.set("StdCF", Object::Dictionary(cf));
    let chaine = |v: Vec<u8>| Object::String(v, StringFormat::Hexadecimal);
    let mut e = Dictionary::new();
    e.set("Filter", Object::Name(b"Standard".to_vec()));
    e.set("V", Object::Integer(5));
    e.set("R", Object::Integer(6));
    e.set("Length", Object::Integer(256));
    e.set("P", Object::Integer(PERMISSIONS as i32 as i64));
    e.set("EncryptMetadata", Object::Boolean(true));
    e.set("CF", Object::Dictionary(filtres));
    e.set("StmF", Object::Name(b"StdCF".to_vec()));
    e.set("StrF", Object::Name(b"StdCF".to_vec()));
    e.set("O", chaine(o));
    e.set("U", chaine(u));
    e.set("OE", chaine(oe));
    e.set("UE", chaine(ue));
    e.set("Perms", chaine(perms.to_vec()));
    let id_chiffrement = doc.add_object(Object::Dictionary(e));
    doc.trailer.set("Encrypt", Object::Reference(id_chiffrement));
    // L'identifiant du fichier (deux chaînes de 16 octets, jamais chiffrées).
    let id = chaine(hasard(16)?);
    doc.trailer.set("ID", Object::Array(vec![id.clone(), id]));

    let mut sortie = Vec::new();
    doc.save_to(&mut sortie).map_err(|e| Refus(format!("écriture du PDF impossible ({})", e)))?;
    Ok(sortie)
}

/// La clé de fichier chiffrée par la clé dérivée du mot de passe (AES-256-CBC, IV nul, sans bourrage).
fn chiffrer_cle(cle_derivee: &[u8], cle_fichier: &[u8]) -> Result<Vec<u8>, Refus> {
    Ok(CbcChiffre256::new_from_slices(cle_derivee, &[0u8; 16])
        .map_err(|_| Refus("clé de chiffrement invalide".into()))?
        .encrypt_padded_vec_mut::<NoPadding>(cle_fichier))
}

// ---------------------------------------------------------------------------------------------
// Déchiffrer.
// ---------------------------------------------------------------------------------------------

/// Le dictionnaire `/Encrypt` du fichier (direct dans la queue, ou par référence), et l'objet qui
/// le porte s'il est indirect.
fn dictionnaire_de_chiffrement(doc: &Document) -> Option<(Dictionary, Option<ObjectId>)> {
    match doc.trailer.get(b"Encrypt").ok()? {
        Object::Dictionary(d) => Some((d.clone(), None)),
        Object::Reference(id) => match doc.get_object(*id).ok()? {
            Object::Dictionary(d) => Some((d.clone(), Some(*id))),
            _ => None,
        },
        _ => None,
    }
}

fn octets_de(d: &Dictionary, cle: &[u8]) -> Option<Vec<u8>> {
    match d.get(cle).ok()? {
        Object::String(s, _) => Some(s.clone()),
        _ => None,
    }
}

/// Déchiffre `doc` avec ce mot de passe utilisateur (sans effet si le fichier n'est pas chiffré).
pub fn dechiffrer(doc: &mut Document, mot_de_passe: &[u8]) -> Result<(), Refus> {
    let (e, id_e) = match dictionnaire_de_chiffrement(doc) {
        Some(x) => x,
        None => return Ok(()),
    };
    let v = e.get(b"V").ok().and_then(|o| o.as_i64().ok()).unwrap_or(0);
    let r = e.get(b"R").ok().and_then(|o| o.as_i64().ok()).unwrap_or(0);
    if v < 5 {
        // RC4 (V 1 et 2) : lopdf, qui veut le dictionnaire en objet indirect.
        if id_e.is_none() {
            let id = doc.add_object(Object::Dictionary(e));
            doc.trailer.set("Encrypt", Object::Reference(id));
        }
        return doc.decrypt(mot_de_passe).map_err(|err| {
            Refus(format!("chiffrement non pris en charge ou mot de passe refusé (V {}, R {} : {})", v, r, err))
        });
    }
    let (u, ue) = match (octets_de(&e, b"U"), octets_de(&e, b"UE")) {
        (Some(u), Some(ue)) if u.len() >= 48 && ue.len() >= 32 => (u, ue),
        _ => return refus("chiffrement illisible : /U ou /UE manque"),
    };
    // Le mot de passe utilisateur doit rendre l'empreinte de validation.
    let (empreinte_u, sel_valid, sel_cle) = (&u[..32], &u[32..40], &u[40..48]);
    if empreinte(r, mot_de_passe, sel_valid, b"") != empreinte_u {
        return refus("le PDF est chiffré et ne s'ouvre pas sans mot de passe");
    }
    let cle_derivee = empreinte(r, mot_de_passe, sel_cle, b"");
    let cle = CbcDechiffre256::new_from_slices(&cle_derivee, &[0u8; 16])
        .map_err(|_| Refus("clé de chiffrement invalide".into()))?
        .decrypt_padded_vec_mut::<NoPadding>(&ue[..32])
        .map_err(|_| Refus("clé de fichier illisible".into()))?;

    // Les filtres de chiffrement : seul AESV3 est pris en charge ; `Identity` laisse en clair.
    let filtre = |nom: &[u8]| -> Result<bool, Refus> {
        let n = match e.get(nom).ok().and_then(|o| o.as_name().ok()) {
            Some(n) => n.to_vec(),
            None => return Ok(false),
        };
        if n == b"Identity" {
            return Ok(false);
        }
        let cfm = e
            .get(b"CF")
            .ok()
            .and_then(|o| o.as_dict().ok())
            .and_then(|cf| cf.get(&n).ok())
            .and_then(|o| o.as_dict().ok())
            .and_then(|f| f.get(b"CFM").ok())
            .and_then(|o| o.as_name().ok());
        if cfm == Some(b"AESV3") {
            Ok(true)
        } else {
            refus("chiffrement non pris en charge (seul AES-256 l'est)")
        }
    };
    let chiffre_flux = filtre(b"StmF")?;
    let chiffre_chaines = filtre(b"StrF")?;
    let metadonnees = e.get(b"EncryptMetadata").ok().and_then(|o| o.as_bool().ok()).unwrap_or(true);

    for (id, objet) in doc.objects.iter_mut() {
        if Some(*id) == id_e || est_de_structure(objet) {
            continue;
        }
        if !metadonnees && matches!(objet.type_name(), Ok("Metadata")) {
            continue;
        }
        let mut chaine = |d: &[u8]| Ok(if chiffre_chaines { dechiffrer_donnees(&cle, d) } else { d.to_vec() });
        let mut flux = |d: &[u8]| Ok(if chiffre_flux { dechiffrer_donnees(&cle, d) } else { d.to_vec() });
        parcourir(objet, &mut chaine, &mut flux)?;
    }
    if let Some(id) = id_e {
        doc.objects.remove(&id);
    }
    doc.trailer.remove(b"Encrypt");
    Ok(())
}
