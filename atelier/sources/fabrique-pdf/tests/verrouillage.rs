//! Tests de `--verrouiller` : chiffrement AES-256 de PDF 2.0 (`/V 5 /R 6`), mot de passe utilisateur
//! vide, mot de passe propriétaire tiré au hasard et jeté, permissions « impression + copie +
//! accessibilité » (celles de `verrouiller` dans moteur/contrat.py, `/P` = -3372).
//!
//! Le déchiffrement des tests est **réécrit ici, à part** du code de la fabrique (algorithme 2.B de
//! l'ISO 32000-2, AES-CBC) : un fichier chiffré par la fabrique doit s'ouvrir avec lui. Les
//! vérifications avec d'autres lecteurs (PyMuPDF, qpdf) sont décrites dans README.md.

use aes::cipher::block_padding::{NoPadding, Pkcs7};
use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use lopdf::{Dictionary, Document, Object};
use sha2::{Digest, Sha256, Sha384, Sha512};

mod commun;
use commun::{chemin, dossier_temp, lancer, pdf_fabrique, textes};

fn verrouiller(entree: &str, sortie: &str) -> std::process::Output {
    lancer(&["--verrouiller", entree, sortie])
}

/// L'algorithme 2.B de l'ISO 32000-2, écrit à part de `src/chiffrement.rs`.
fn hachage_r6(mot_de_passe: &[u8], sel: &[u8], u: &[u8]) -> Vec<u8> {
    let mut k = Sha256::digest([mot_de_passe, sel, u].concat()).to_vec();
    let mut i = 0usize;
    loop {
        let unite = [mot_de_passe, &k[..], u].concat();
        let k1 = unite.repeat(64);
        let e = cbc::Encryptor::<aes::Aes128>::new(k[..16].into(), k[16..32].into()).encrypt_padded_vec_mut::<NoPadding>(&k1);
        let m = e[..16].iter().map(|b| *b as u32).sum::<u32>() % 3;
        k = match m {
            0 => Sha256::digest(&e).to_vec(),
            1 => Sha384::digest(&e).to_vec(),
            _ => Sha512::digest(&e).to_vec(),
        };
        i += 1;
        if i >= 64 && *e.last().unwrap() as usize <= i - 32 {
            break;
        }
    }
    k[..32].to_vec()
}

fn dechiffrer_flux(cle: &[u8], donnees: &[u8]) -> Vec<u8> {
    cbc::Decryptor::<aes::Aes256>::new(cle.into(), donnees[..16].into())
        .decrypt_padded_vec_mut::<Pkcs7>(&donnees[16..])
        .expect("bourrage PKCS#7")
}

fn octets(d: &Dictionary, cle: &[u8]) -> Vec<u8> {
    d.get(cle).unwrap().as_str().unwrap().to_vec()
}

/// Le dictionnaire `/Encrypt` du fichier, et la clé de fichier retrouvée avec le mot de passe vide.
fn ouvrir_a_la_main(octets_pdf: &[u8]) -> (Document, Dictionary, Vec<u8>) {
    let doc = Document::load_mem(octets_pdf).unwrap();
    let e = doc.get_dictionary(doc.trailer.get(b"Encrypt").unwrap().as_reference().unwrap()).unwrap().clone();
    let u = octets(&e, b"U");
    assert_eq!(u.len(), 48);
    // le mot de passe utilisateur (vide) valide ; la clé de fichier sort de /UE
    assert_eq!(hachage_r6(b"", &u[32..40], b""), u[..32], "le mot de passe vide ne valide pas /U");
    let derivee = hachage_r6(b"", &u[40..48], b"");
    let cle = cbc::Decryptor::<aes::Aes256>::new(derivee.as_slice().into(), (&[0u8; 16]).into())
        .decrypt_padded_vec_mut::<NoPadding>(&octets(&e, b"UE"))
        .unwrap();
    assert_eq!(cle.len(), 32);
    (doc, e, cle)
}

#[test]
fn le_dictionnaire_de_chiffrement_est_celui_de_pdf_2_0_aes_256() {
    let d = dossier_temp("verrou-dico");
    let a = pdf_fabrique(&d, "a.pdf", 2);
    let sortie = d.join("verrouille.pdf");
    let r = verrouiller(&a, &chemin(&sortie));
    assert_eq!(r.status.code(), Some(0), "{}", String::from_utf8_lossy(&r.stderr));
    let brut = std::fs::read(&sortie).unwrap();
    assert!(brut.starts_with(b"%PDF-2.0"), "{:?}", &brut[..12]);
    let (doc, e, _) = ouvrir_a_la_main(&brut);
    let nom = |c: &[u8]| String::from_utf8(e.get(c).unwrap().as_name().unwrap().to_vec()).unwrap();
    assert_eq!(nom(b"Filter"), "Standard");
    assert_eq!(e.get(b"V").unwrap().as_i64().unwrap(), 5);
    assert_eq!(e.get(b"R").unwrap().as_i64().unwrap(), 6);
    assert_eq!(e.get(b"Length").unwrap().as_i64().unwrap(), 256);
    // impression (bit 3), copie (bit 5) et accessibilité (bit 10) : -3372, comme MuPDF l'écrivait
    assert_eq!(e.get(b"P").unwrap().as_i64().unwrap(), -3372);
    let p = (e.get(b"P").unwrap().as_i64().unwrap() as i32) as u32;
    for (bit, voulu) in [(3, true), (4, false), (5, true), (6, false), (9, false), (10, true), (11, false), (12, false)] {
        assert_eq!(p & (1 << (bit - 1)) != 0, voulu, "bit {} de /P", bit);
    }
    assert_eq!(e.get(b"EncryptMetadata").unwrap().as_bool().unwrap(), true);
    assert_eq!((nom(b"StmF"), nom(b"StrF")), ("StdCF".to_string(), "StdCF".to_string()));
    let cf = e.get(b"CF").unwrap().as_dict().unwrap().get(b"StdCF").unwrap().as_dict().unwrap();
    assert_eq!(cf.get(b"CFM").unwrap().as_name().unwrap(), b"AESV3");
    assert_eq!(cf.get(b"AuthEvent").unwrap().as_name().unwrap(), b"DocOpen");
    assert_eq!((octets(&e, b"O").len(), octets(&e, b"U").len()), (48, 48));
    assert_eq!((octets(&e, b"OE").len(), octets(&e, b"UE").len(), octets(&e, b"Perms").len()), (32, 32, 16));
    assert!(doc.trailer.get(b"ID").is_ok());
}

#[test]
fn le_contenu_d_une_page_est_chiffre_puis_se_dechiffre_avec_le_mot_de_passe_vide() {
    let d = dossier_temp("verrou-page");
    let a = pdf_fabrique(&d, "a.pdf", 2);
    let sortie = d.join("verrouille.pdf");
    assert_eq!(verrouiller(&a, &chemin(&sortie)).status.code(), Some(0));

    let original = Document::load(&a).unwrap();
    let (chiffre, e, cle) = ouvrir_a_la_main(&std::fs::read(&sortie).unwrap());
    let id_e = chiffre.trailer.get(b"Encrypt").unwrap().as_reference().unwrap();
    let _ = e;
    for ((_, p_orig), (_, p_chif)) in original.get_pages().iter().zip(chiffre.get_pages().iter()) {
        let clair = original.get_page_content(*p_orig).unwrap();
        // le(s) flux de la page : chiffrés, donc différents du clair ; déchiffrés à la main, ils le rendent
        let mut rendu = Vec::new();
        for id in chiffre.get_page_contents(*p_chif) {
            let s = chiffre.get_object(id).unwrap().as_stream().unwrap();
            assert_ne!(s.content, clair, "le flux n'est pas chiffré");
            let en_clair = dechiffrer_flux(&cle, &s.content);
            // le contenu est comprimé : on le décomprime comme le fait lopdf
            let mut ds = s.clone();
            ds.set_content(en_clair);
            rendu.extend(ds.decompressed_content().unwrap_or_else(|_| ds.content.clone()));
        }
        assert_eq!(rendu, clair, "le contenu déchiffré à la main n'est pas celui de la page");
    }
    // le dictionnaire de chiffrement n'est pas chiffré, tout le reste l'est : aucune chaîne en clair
    for (id, o) in chiffre.objects.iter() {
        if *id == id_e {
            continue;
        }
        let mut chaines = Vec::new();
        collecter_chaines(o, &mut chaines);
        for c in chaines {
            assert!(c.len() >= 32 && c.len() % 16 == 0, "chaîne de {} octets non chiffrée dans l'objet {:?}", c.len(), id);
        }
    }
}

fn collecter_chaines(o: &Object, sortie: &mut Vec<Vec<u8>>) {
    match o {
        Object::String(s, _) => sortie.push(s.clone()),
        Object::Array(v) => v.iter().for_each(|x| collecter_chaines(x, sortie)),
        Object::Dictionary(d) => d.iter().for_each(|(_, x)| collecter_chaines(x, sortie)),
        Object::Stream(s) => s.dict.iter().for_each(|(_, x)| collecter_chaines(x, sortie)),
        _ => {}
    }
}

#[test]
fn la_fabrique_relit_ce_qu_elle_verrouille_pages_texte_et_second_verrou() {
    let d = dossier_temp("verrou-relecture");
    let a = pdf_fabrique(&d, "a.pdf", 3);
    let v1 = chemin(&d.join("v1.pdf"));
    assert_eq!(verrouiller(&a, &v1).status.code(), Some(0));
    // le nombre de pages et le texte, identiques avant et après
    assert_eq!(String::from_utf8(lancer(&["--pages", &v1]).stdout).unwrap().trim(), "[3]");
    assert_eq!(textes(&v1), textes(&a));
    // verrouiller un fichier déjà verrouillé : le résultat s'ouvre de même, avec une autre clé
    let v2 = chemin(&d.join("v2.pdf"));
    assert_eq!(verrouiller(&v1, &v2).status.code(), Some(0));
    assert_eq!(textes(&v2), textes(&a));
    assert_ne!(std::fs::read(&v1).unwrap(), std::fs::read(&v2).unwrap());
    // la clé, les sels et les IV sont tirés au hasard : deux verrous du même fichier diffèrent
    let v3 = chemin(&d.join("v3.pdf"));
    assert_eq!(verrouiller(&a, &v3).status.code(), Some(0));
    assert_ne!(std::fs::read(&v1).unwrap(), std::fs::read(&v3).unwrap());
}

#[test]
fn un_autre_mot_de_passe_que_le_vide_est_refuse() {
    let d = dossier_temp("verrou-mot-de-passe");
    let a = pdf_fabrique(&d, "a.pdf", 1);
    let v = d.join("v.pdf");
    assert_eq!(verrouiller(&a, &chemin(&v)).status.code(), Some(0));
    let mut doc = Document::load(&v).unwrap();
    let e = fabrique_pdf::chiffrement::dechiffrer(&mut doc, b"autre").expect_err("refus attendu");
    assert!(e.0.contains("mot de passe"), "{:?}", e.0);
    // le mot de passe propriétaire, lui, est jeté : rien dans le fichier ne le donne, et /O n'est
    // pas celui d'un mot de passe vide
    let mut doc = Document::load(&v).unwrap();
    let id = doc.trailer.get(b"Encrypt").unwrap().as_reference().unwrap();
    let o = doc.get_dictionary(id).unwrap().get(b"O").unwrap().as_str().unwrap().to_vec();
    assert_ne!(hachage_r6(b"", &o[32..40], &octets(doc.get_dictionary(id).unwrap(), b"U")), o[..32]);
    fabrique_pdf::chiffrement::dechiffrer(&mut doc, b"").unwrap();
    assert!(doc.trailer.get(b"Encrypt").is_err(), "le dictionnaire est retiré une fois déchiffré");
}

#[test]
fn les_refus_tiennent_en_une_ligne_et_n_ecrivent_rien() {
    let d = dossier_temp("verrou-refus");
    let faux = d.join("faux.pdf");
    std::fs::write(&faux, b"ceci n'est pas un PDF").unwrap();
    let sortie = d.join("jamais.pdf");
    let _ = std::fs::remove_file(&sortie);
    for entree in [chemin(&faux), chemin(&d.join("absent.pdf"))] {
        let r = verrouiller(&entree, &chemin(&sortie));
        assert_eq!(r.status.code(), Some(1));
        assert!(r.stdout.is_empty());
        let ligne = String::from_utf8(r.stderr).unwrap();
        assert!(ligne.trim().lines().count() == 1 && ligne.contains(".pdf"), "{:?}", ligne);
        assert!(!sortie.exists());
    }
    assert_eq!(lancer(&["--verrouiller", &chemin(&faux)]).status.code(), Some(2));
}

#[test]
fn verrouiller_en_place_garde_le_texte() {
    let d = dossier_temp("verrou-en-place");
    let a = pdf_fabrique(&d, "a.pdf", 2);
    let avant = textes(&a);
    assert_eq!(verrouiller(&a, &a).status.code(), Some(0));
    assert_eq!(textes(&a), avant);
    let doc = Document::load(&a).unwrap();
    assert!(doc.trailer.get(b"Encrypt").is_ok());
}
