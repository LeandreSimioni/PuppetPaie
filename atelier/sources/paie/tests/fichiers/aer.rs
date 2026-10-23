//! L'AER remplie et signee par la fabrique : `paie::aer` et `paie::cachet`, sur une attestation FICTIVE. On refuse de
//! signer une attestation qui ne concorde pas avec la paie, on ne retouche pas un chiffre de France Travail, la signature
//! est deterministe. La lecture d'une date, d'un entier et le cadre de la signature : `src/aer.rs`.
//!
//! Le PDF d'essai est ecrit ici en octets bruts (catalogue, AcroForm, trois
//! pages, champs widgets, table xref) : il imite l'AER de France Travail --
//! quantites imprimees en LECTURE SEULE a la page 2, cadre 5 a la page 3 (champs
//! texte, dates et telephone en peigne, courriel multiligne, cases signature et
//! cachet) -- sans emprunter un octet a une vraie attestation. La compagnie est
//! inventee de bout en bout.

use paie::aer::{self, Authentification, Echec, PaieAer};
use paie::cachet;
use paie::date::Jour;
use serde_json::{json, Value};

const HAUTEUR: f64 = 842.0;

fn jour(a: i32, m: u32, j: u32) -> Jour {
    Jour::nouveau(a, m, j).unwrap()
}

fn fiche() -> Value {
    json!({
        "raison_sociale": "Compagnie du Chat Perche",
        "siret": "12345678900019",
        "naf": "9001Z",
        "adresse": "12 rue des Lilas",
        "code_postal": "69003",
        "commune": "Lyon",
        "licence_spectacle": "PLATESV-R-2026-000000",
        "signataire": {"nom": "MARTIN", "prenom": "Camille", "fonction": "Presidente"},
        "declarant_dsn": {"nom_prenom": "DURAND Lou", "tel": "0102030405", "email": "Lou.Durand@Exemple.TEST"},
    })
}

// ------------------------------------------------------------------ le PDF d'essai

struct Champ {
    nom: String,
    page: usize,
    rect: [f64; 4],
    valeur: Option<String>,
    ff: u32,
    maxlen: Option<u32>,
}

fn champ(nom: &str, page: usize, rect: [f64; 4], valeur: Option<&str>, ff: u32, maxlen: Option<u32>) -> Champ {
    Champ { nom: nom.into(), page, rect, valeur: valeur.map(str::to_string), ff, maxlen }
}

/// Les champs de l'AER fictive. Les chiffres imprimes sont en lecture seule
/// (bit 1), le cadre 5 est vide.
fn champs_aer(cachets: u32, heures: u32) -> Vec<Champ> {
    let mut c = Vec::new();
    let contrats = [("01/08/2026", "31/08/2026", "FA1.26.15", cachets, heures), ("05/08/2026", "06/08/2026", "MA2.26.15", 1, 0)];
    for (i, (deb, fin, num, n, h)) in contrats.iter().enumerate() {
        let rang = i + 1;
        let y = 100.0 + 30.0 * rang as f64;
        c.push(champ(&format!("SAL_DT_DEB_{:02}", rang), 2, [60.0, y, 140.0, y + 14.0], Some(deb), 1, None));
        c.push(champ(&format!("SAL_DT_FIN_{:02}", rang), 2, [150.0, y, 230.0, y + 14.0], Some(fin), 1, None));
        c.push(champ(&format!("SAL_NB_CACHET_{:02}", rang), 2, [240.0, y, 280.0, y + 14.0], Some(&n.to_string()), 1, None));
        c.push(champ(&format!("SAL_NB_HEURES_{:02}", rang), 2, [290.0, y, 330.0, y + 14.0], Some(&h.to_string()), 1, None));
        c.push(champ(&format!("CON_NUMERO_{:02}", rang), 2, [340.0, y, 440.0, y + 14.0], Some(num), 1, None));
    }
    let comb = 1 << 24;
    let multiligne = 1 << 12;
    let mut y = 541.82;
    for nom in ["EMPLOYEUR_NOM", "EMPLOYEUR_PRENOM", "EMPLOYEUR_QUALITE_LIB", "EMPLOYEUR_LIEU_SIGN", "EMPLOYEUR_CORRES_NOM"] {
        c.push(champ(nom, 3, [131.1, y, 569.27, y + 16.02], None, 0, None));
        y += 22.0;
    }
    for (nom, n) in [("EMPLOYEUR_DATE_SIGN_J", 2), ("EMPLOYEUR_DATE_SIGN_M", 2), ("EMPLOYEUR_DATE_SIGN_A", 4)] {
        c.push(champ(nom, 3, [131.1, y, 131.1 + 14.0 * n as f64, y + 16.02], None, comb, Some(n)));
        y += 22.0;
    }
    c.push(champ("EMPLOYEUR_CORRES_TEL", 3, [131.1, y, 131.1 + 14.0 * 20.0, y + 16.02], None, comb, Some(20)));
    y += 22.0;
    c.push(champ("EMPLOYEUR_CORRES_EMAIL", 3, [131.1, y, 400.0, y + 34.0], None, multiligne, None));
    c.push(champ("EMPLOYEUR_SIGNATURE", 3, [35.1, 687.4, 135.8, 735.6], None, 0, None));
    c.push(champ("EMPLOYEUR_CACHET", 3, [401.4, 685.4, 562.1, 764.7], None, 0, None));
    c
}

fn echappe(t: &str) -> String {
    t.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)")
}

/// Un PDF valide de trois pages A4 portant ces champs. Table xref exacte.
fn formulaire(champs: &[Champ]) -> Vec<u8> {
    // numeros : 1 catalogue, 2 pages, 3..5 pages, 6..8 contenus, 9 police,
    // 10 AcroForm, 11.. les widgets
    let premier = 11;
    let mut par_page: Vec<Vec<usize>> = vec![Vec::new(); 4];
    for (i, ch) in champs.iter().enumerate() {
        par_page[ch.page].push(premier + i);
    }
    let mut objets: Vec<(usize, Vec<u8>)> = Vec::new();
    objets.push((1, b"<< /Type /Catalog /Pages 2 0 R /AcroForm 10 0 R >>".to_vec()));
    objets.push((
        2,
        b"<< /Type /Pages /Kids [3 0 R 4 0 R 5 0 R] /Count 3 /MediaBox [0 0 595 842] /Resources << /Font << /F1 9 0 R >> >> >>".to_vec(),
    ));
    for p in 1..=3 {
        let annots: Vec<String> = par_page[p].iter().map(|n| format!("{} 0 R", n)).collect();
        objets.push((2 + p, format!("<< /Type /Page /Parent 2 0 R /Contents {} 0 R /Annots [{}] >>", 5 + p, annots.join(" ")).into_bytes()));
        let texte = format!("BT /F1 12 Tf 50 800 Td (Attestation employeur page {}) Tj ET\n", p);
        let mut o = format!("<< /Length {} >>\nstream\n", texte.len()).into_bytes();
        o.extend(texte.as_bytes());
        o.extend(b"endstream");
        objets.push((5 + p, o));
    }
    objets.push((9, b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_vec()));
    let refs: Vec<String> = (0..champs.len()).map(|i| format!("{} 0 R", premier + i)).collect();
    objets.push((10, format!("<< /Fields [{}] /DA (/Helv 7 Tf 0 g) /DR << /Font << /Helv 9 0 R >> >> >>", refs.join(" ")).into_bytes()));
    for (i, ch) in champs.iter().enumerate() {
        let [x0, y0, x1, y1] = ch.rect;
        let mut d = format!(
            "<< /Type /Annot /Subtype /Widget /F 4 /FT /Tx /T ({}) /Rect [{:.2} {:.2} {:.2} {:.2}] /P {} 0 R",
            ch.nom,
            x0,
            HAUTEUR - y1,
            x1,
            HAUTEUR - y0,
            2 + ch.page
        );
        if ch.ff != 0 {
            d += &format!(" /Ff {}", ch.ff);
        }
        if let Some(m) = ch.maxlen {
            d += &format!(" /MaxLen {}", m);
        }
        if let Some(v) = &ch.valeur {
            d += &format!(" /V ({})", echappe(v));
        }
        objets.push((premier + i, (d + " >>").into_bytes()));
    }
    objets.sort_by_key(|(n, _)| *n);
    let mut sortie: Vec<u8> = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut positions = std::collections::HashMap::new();
    for (n, o) in &objets {
        positions.insert(*n, sortie.len());
        sortie.extend(format!("{} 0 obj\n", n).as_bytes());
        sortie.extend(o);
        sortie.extend(b"\nendobj\n");
    }
    let debut_xref = sortie.len();
    let total = objets.last().unwrap().0 + 1;
    sortie.extend(format!("xref\n0 {}\n0000000000 65535 f \n", total).as_bytes());
    for n in 1..total {
        sortie.extend(format!("{:010} 00000 n \n", positions[&n]).as_bytes());
    }
    sortie.extend(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n", total, debut_xref).as_bytes());
    sortie
}

fn attestation() -> Vec<u8> {
    formulaire(&champs_aer(2, 12))
}

fn crc32(octets: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in octets {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

fn adler32(octets: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &o in octets {
        a = (a + o as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

/// Un PNG RVBA (une moitie transparente), en octets bruts : zlib en blocs
/// non compresses, pour ne rien ajouter aux dependances.
fn png(largeur: u32, hauteur: u32) -> Vec<u8> {
    fn morceau(nom: &[u8], corps: &[u8]) -> Vec<u8> {
        let mut o = (corps.len() as u32).to_be_bytes().to_vec();
        let mut ncorps = nom.to_vec();
        ncorps.extend(corps);
        o.extend(&ncorps);
        o.extend(crc32(&ncorps).to_be_bytes());
        o
    }
    let mut lignes = Vec::new();
    for _ in 0..hauteur {
        lignes.push(0u8);
        for x in 0..largeur {
            lignes.extend([20, 40, 160, if x >= largeur / 2 { 255 } else { 0 }]);
        }
    }
    let mut z = vec![0x78, 0x01];
    let blocs: Vec<&[u8]> = lignes.chunks(65535).collect();
    for (i, b) in blocs.iter().enumerate() {
        z.push(if i + 1 == blocs.len() { 1 } else { 0 });
        z.extend((b.len() as u16).to_le_bytes());
        z.extend((!(b.len() as u16)).to_le_bytes());
        z.extend(*b);
    }
    z.extend(adler32(&lignes).to_be_bytes());
    let mut ihdr = Vec::new();
    ihdr.extend(largeur.to_be_bytes());
    ihdr.extend(hauteur.to_be_bytes());
    ihdr.extend([8, 6, 0, 0, 0]);
    let mut o = b"\x89PNG\r\n\x1a\n".to_vec();
    o.extend(morceau(b"IHDR", &ihdr));
    o.extend(morceau(b"IDAT", &z));
    o.extend(morceau(b"IEND", b""));
    o
}

const SVG_SIGNATURE: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"100\" height=\"50\" viewBox=\"0 0 100 50\"><path d=\"M5 40 C 20 5, 40 5, 50 30 S 80 45, 95 10\" fill=\"none\" stroke=\"#123\" stroke-width=\"2\"/></svg>";

fn paie(numero: &str, debut: Jour, fin: Jour, cachets: i128, heures: i128) -> PaieAer {
    PaieAer { numero: numero.into(), debut, fin, cachets, heures }
}

fn paies_justes() -> Vec<PaieAer> {
    vec![paie("FA1.26.15", jour(2026, 8, 1), jour(2026, 8, 31), 2, 12), paie("MA2.26.15", jour(2026, 8, 5), jour(2026, 8, 6), 1, 0)]
}

fn paies_fausses() -> Vec<PaieAer> {
    vec![paie("FA1.26.15", jour(2026, 8, 1), jour(2026, 8, 31), 2, 24), paie("MA2.26.15", jour(2026, 8, 5), jour(2026, 8, 6), 1, 0)]
}

fn signer(pdf: &[u8], ent: &Value, paies: Option<&[PaieAer]>, forcer: bool, signature: Option<&str>, cachet: Option<&str>) -> Result<Vec<u8>, Echec> {
    aer::authentifier(pdf, &Authentification { ent, aujourdhui: jour(2026, 10, 1), signature, paies, forcer, cachet_fichier: cachet })
}

fn fichier(nom: &str, octets: &[u8]) -> String {
    let f = crate::outils::chemin_unique(nom);
    std::fs::write(&f, octets).unwrap();
    f.to_string_lossy().into_owned()
}

fn valeur_de(champs: &Value, nom: &str) -> Value {
    champs[nom]["valeur"].clone()
}

fn contient(h: &[u8], aiguille: &str) -> bool {
    h.windows(aiguille.len()).any(|w| w == aiguille.as_bytes())
}

// ------------------------------------------------------------------ le PDF d'essai lui-meme

#[test]
fn le_pdf_d_essai_est_lu_par_la_fabrique() {
    let c = aer::champs(&attestation()).unwrap();
    assert_eq!(c["SAL_NB_CACHET_01"]["valeur"], "2");
    assert_eq!(c["SAL_NB_CACHET_01"]["lecture_seule"], true);
    assert_eq!(c["SAL_NB_CACHET_01"]["page"], 2);
    assert_eq!(c["EMPLOYEUR_NOM"]["page"], 3);
    assert!(c["EMPLOYEUR_NOM"]["valeur"].is_null());
    assert_eq!(c["EMPLOYEUR_NOM"]["lecture_seule"], false);
    let r: Vec<f64> = c["EMPLOYEUR_CACHET"]["rect"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
    for (a, b) in r.iter().zip([401.4, 685.4, 562.1, 764.7]) {
        assert!((a - b).abs() < 0.01, "{:?}", r);
    }
}

// ------------------------------------------------------------------ lire_quantites

#[test]
fn lire_quantites_dates_et_entiers() {
    let q = aer::lire_quantites(&aer::champs(&attestation()).unwrap()).unwrap();
    assert_eq!(q.len(), 2);
    assert_eq!(
        q[0],
        aer::Ligne { debut: Some(jour(2026, 8, 1)), fin: Some(jour(2026, 8, 31)), numero: "FA1.26.15".into(), cachets: 2, heures: 12 }
    );
    assert_eq!(
        q[1],
        aer::Ligne { debut: Some(jour(2026, 8, 5)), fin: Some(jour(2026, 8, 6)), numero: "MA2.26.15".into(), cachets: 1, heures: 0 }
    );
}

#[test]
fn lire_quantites_un_contrat_sans_dates_est_ignore() {
    let sans_02: Vec<Champ> = champs_aer(2, 12).into_iter().filter(|c| !c.nom.ends_with("_02")).collect();
    let q = aer::lire_quantites(&aer::champs(&formulaire(&sans_02)).unwrap()).unwrap();
    assert_eq!(q.len(), 1);
    assert_eq!(q[0].debut, Some(jour(2026, 8, 1)));
    assert!(aer::lire_quantites(&aer::champs(&formulaire(&[])).unwrap()).unwrap().is_empty());
}

#[test]
fn lire_quantites_deux_rangs_de_meme_cle_le_dernier_l_emporte_a_la_place_du_premier() {
    let mut c = champs_aer(2, 12);
    for ch in c.iter_mut() {
        match ch.nom.as_str() {
            "SAL_DT_DEB_02" => ch.valeur = Some("01/08/2026".into()),
            "SAL_DT_FIN_02" => ch.valeur = Some("31/08/2026".into()),
            "CON_NUMERO_02" => ch.valeur = Some("FA1.26.15".into()),
            _ => {}
        }
    }
    let q = aer::lire_quantites(&aer::champs(&formulaire(&c)).unwrap()).unwrap();
    assert_eq!(q.len(), 1);
    assert_eq!((q[0].numero.as_str(), q[0].cachets, q[0].heures), ("FA1.26.15", 1, 0));
}

// ------------------------------------------------------------------ verifier

#[test]
fn verifier_concordance_rend_une_liste_vide() {
    assert_eq!(aer::verifier(&attestation(), &paies_justes()), Ok(vec![]));
}

#[test]
fn verifier_un_ecart_donne_un_message_qui_dit_quoi() {
    let e = aer::verifier(&attestation(), &paies_fausses()).unwrap();
    // Le contrat, sa periode, et les deux heures qui different.
    assert_eq!(e.len(), 1);
    assert!(["FA1.26.15", "2026-08-01", "2026-08-31", "12 heures", "24"].iter().all(|x| e[0].contains(x)), "{:?}", e);
    let mut trois = paies_justes();
    trois[0].cachets = 3;
    let e = aer::verifier(&attestation(), &trois).unwrap();
    assert_eq!(e.len(), 1);
    assert!(e[0].contains("2 cachets sur l'attestation, 3 dans la paie"), "{:?}", e);
}

/// Une periode imprimee qu'aucune paie n'explique, ou une periode de la paie que l'attestation ne porte pas : signer
/// certifierait exacte une attestation qui ajoute ou retire une periode d'emploi. C'est un ecart, jamais un silence.
#[test]
fn verifier_une_periode_en_trop_ou_en_moins_est_un_ecart() {
    let e = aer::verifier(&attestation(), &paies_justes()[..1]).unwrap();
    assert_eq!(e.len(), 1, "{:?}", e);
    assert!(e[0].contains("MA2.26.15") && e[0].contains("aucune paie"), "{:?}", e);
    let mut plus = paies_justes();
    plus.push(paie("FA1.26.15", jour(2026, 7, 1), jour(2026, 7, 31), 9, 99));
    let e = aer::verifier(&attestation(), &plus).unwrap();
    assert_eq!(e.len(), 1, "{:?}", e);
    assert!(e[0].contains("2026-07-01") && e[0].contains("ne porte pas"), "{:?}", e);
    // Le meme contrat sous un autre numero (le compteur d'aujourd'hui, pas celui de la DSN) ne s'explique pas.
    let e = aer::verifier(&attestation(), &[paie("FA1.26.18", jour(2026, 8, 1), jour(2026, 8, 31), 2, 12), paies_justes()[1].clone()]).unwrap();
    assert_eq!(e.len(), 2, "{:?}", e);
}

#[test]
fn verifier_un_contrat_sans_numero_s_appelle_contrat() {
    let sans_numero: Vec<Champ> = champs_aer(2, 12).into_iter().filter(|c| c.nom != "CON_NUMERO_01").collect();
    let e = aer::verifier(&formulaire(&sans_numero), &paies_fausses()).unwrap();
    assert!(e[0].starts_with("contrat (2026-08-01 au 2026-08-31)"), "{:?}", e);
}

/// UN CONTRAT SUR DEUX MOIS (la forme d'une vraie AER d'octobre 2026, depot prive `reference-privee/exemples/`) : une
/// colonne par mois, le numero du contrat imprime dans la premiere seulement. Chaque colonne se compare a la paie de
/// son mois, sous le numero declare.
fn deux_mois(heures_oct: u32) -> Vec<u8> {
    let mut c = Vec::new();
    let colonnes = [("29/09/2026", "30/09/2026", Some("SI2.26.19"), 8), ("01/10/2026", "16/10/2026", None, heures_oct)];
    for (i, (deb, fin, num, h)) in colonnes.iter().enumerate() {
        let rang = i + 1;
        let x = 150.0 + 85.0 * i as f64;
        c.push(champ(&format!("SAL_DT_DEB_{:02}", rang), 2, [x, 218.0, x + 80.0, 233.0], Some(deb), 1, None));
        c.push(champ(&format!("SAL_DT_FIN_{:02}", rang), 2, [x, 236.0, x + 80.0, 251.0], Some(fin), 1, None));
        c.push(champ(&format!("SAL_NB_HEURES_{:02}", rang), 2, [x, 258.0, x + 80.0, 273.0], Some(&h.to_string()), 1, None));
        if let Some(n) = num {
            c.push(champ(&format!("CON_NUMERO_{:02}", rang), 2, [x, 82.0, x + 80.0, 100.0], Some(n), 1, None));
        }
    }
    formulaire(&c)
}

#[test]
fn verifier_un_contrat_sur_deux_mois_colonne_par_colonne() {
    let paies = [paie("SI2.26.19", jour(2026, 9, 29), jour(2026, 9, 30), 0, 8), paie("SI2.26.19", jour(2026, 10, 1), jour(2026, 10, 16), 0, 10)];
    let q = aer::lire_quantites(&aer::champs(&deux_mois(10)).unwrap()).unwrap();
    assert_eq!(q.iter().map(|l| l.numero.as_str()).collect::<Vec<_>>(), ["SI2.26.19", "SI2.26.19"], "la suite du contrat garde son numero");
    assert_eq!(aer::verifier(&deux_mois(10), &paies), Ok(vec![]));
    let e = aer::verifier(&deux_mois(12), &paies).unwrap();
    assert_eq!(e.len(), 1, "{:?}", e);
    assert!(["SI2.26.19", "2026-10-01", "12 heures", "10"].iter().all(|x| e[0].contains(x)), "{:?}", e);
    // La paie d'octobre seule (septembre oublie) : la colonne de septembre ne s'explique pas.
    let e = aer::verifier(&deux_mois(10), &paies[1..]).unwrap();
    assert_eq!(e.len(), 1, "{:?}", e);
    assert!(e[0].contains("2026-09-29"), "{:?}", e);
}

// ------------------------------------------------------------------ authentifier

#[test]
fn authentifier_refuse_une_attestation_qui_ne_concorde_pas() {
    let r = signer(&attestation(), &fiche(), Some(&paies_fausses()), false, None, None);
    match r {
        Err(Echec::Ecarts(e)) => assert!(e[0].contains("FA1.26.15"), "{:?}", e),
        autre => panic!("une attestation fausse a ete signee : {:?}", autre.map(|o| o.len())),
    }
}

#[test]
fn authentifier_signe_malgre_l_ecart_avec_forcer() {
    let signe = signer(&attestation(), &fiche(), Some(&paies_fausses()), true, None, None).unwrap();
    assert!(signe.starts_with(b"%PDF"));
    assert_eq!(valeur_de(&aer::champs(&signe).unwrap(), "EMPLOYEUR_NOM"), "MARTIN");
}

#[test]
fn authentifier_remplit_le_cadre_5_et_ne_touche_pas_aux_chiffres() {
    let avant = aer::champs(&attestation()).unwrap();
    let signe = signer(&attestation(), &fiche(), Some(&paies_justes()), false, None, None).unwrap();
    let apres = aer::champs(&signe).unwrap();
    let attendu = [
        ("EMPLOYEUR_NOM", "MARTIN"),
        ("EMPLOYEUR_PRENOM", "Camille"),
        ("EMPLOYEUR_QUALITE_LIB", "Presidente"),
        ("EMPLOYEUR_LIEU_SIGN", "Lyon"),
        ("EMPLOYEUR_DATE_SIGN_J", "01"),
        ("EMPLOYEUR_DATE_SIGN_M", "10"),
        ("EMPLOYEUR_DATE_SIGN_A", "2026"),
        ("EMPLOYEUR_CORRES_NOM", "DURAND Lou"),
        ("EMPLOYEUR_CORRES_TEL", "0102030405"),
        ("EMPLOYEUR_CORRES_EMAIL", "lou.durand@exemple.test"), // en minuscules
    ];
    for (nom, v) in attendu {
        assert_eq!(valeur_de(&apres, nom), v, "{}", nom);
    }
    assert_eq!(attendu.map(|(n, _)| n), aer::CHAMPS_TEXTE);
    // les chiffres de France Travail : identiques, y compris leur lecture seule
    for (nom, c) in avant.as_object().unwrap() {
        if nom.starts_with("SAL_") || nom.starts_with("CON_") {
            assert_eq!(&apres[nom], c, "{}", nom);
            assert_eq!(apres[nom]["lecture_seule"], true);
        }
    }
    // la signature et le cachet ne sont pas des textes tapes
    assert!(apres["EMPLOYEUR_SIGNATURE"]["valeur"].is_null());
    assert!(apres["EMPLOYEUR_CACHET"]["valeur"].is_null());
    // le contenu qu'on lit toujours : l'attestation ne se lit pas autrement
    assert_eq!(aer::lire_quantites(&apres), aer::lire_quantites(&avant));
    assert_eq!(aer::verifier(&signe, &paies_justes()), Ok(vec![]));
}

#[test]
fn authentifier_sans_paies_signe_sans_regarder() {
    assert!(signer(&attestation(), &fiche(), None, false, None, None).unwrap().starts_with(b"%PDF"));
    // une liste vide n'est pas « sans paies » : rien n'explique les periodes imprimees, et rien ne se signe
    assert!(matches!(signer(&attestation(), &fiche(), Some(&[]), false, None, None), Err(Echec::Ecarts(_))));
}

#[test]
fn authentifier_est_deterministe() {
    let sig = fichier("signature.svg", SVG_SIGNATURE.as_bytes());
    let a = signer(&attestation(), &fiche(), None, false, Some(&sig), None).unwrap();
    let b = signer(&attestation(), &fiche(), None, false, Some(&sig), None).unwrap();
    assert_eq!(a, b);
}

#[test]
fn authentifier_refuse_une_fiche_sans_signataire_apres_la_verification() {
    let mut ent = fiche();
    ent.as_object_mut().unwrap().remove("signataire");
    for pdf in [attestation(), b"pas un pdf".to_vec()] {
        match signer(&pdf, &ent, None, false, None, None) {
            Err(Echec::Autre(m)) => assert!(m.contains("signataire"), "{}", m),
            autre => panic!("une fiche sans signataire a signe : {:?}", autre.map(|o| o.len())),
        }
    }
    // la verification passe d'abord : l'ecart est dit avant la cle absente
    assert!(matches!(signer(&attestation(), &ent, Some(&paies_fausses()), false, None, None), Err(Echec::Ecarts(_))));
}

#[test]
fn authentifier_le_texte_de_la_page_3_porte_le_cadre_5() {
    let signe = signer(&attestation(), &fiche(), None, false, None, None).unwrap();
    let doc = fabrique_pdf::lecture::charger(&signe).unwrap();
    let (pages, _) = fabrique_pdf::texte::texte_des_pages(&doc);
    assert_eq!(pages.len(), 3);
    for v in ["MARTIN", "Camille", "Presidente", "Lyon", "DURAND Lou", "lou.durand@exemple.test"] {
        assert!(pages[2].contains(v), "{}", v);
    }
    assert!(pages[2].contains("Attestation employeur page 3"));
    assert!(!pages[1].contains("MARTIN"));
}

// ------------------------------------------------------------------ signature et cachet

#[test]
fn signature_png_et_cachet_dessine_alourdissent_le_pdf() {
    let sans = signer(&attestation(), &fiche(), None, false, None, None).unwrap();
    let sig = fichier("signature.png", &png(40, 20));
    let avec = signer(&attestation(), &fiche(), None, false, Some(&sig), None).unwrap();
    assert!(avec.len() > sans.len() && sans.len() > attestation().len());
    // deux images posees sur la page 3 (la signature, puis le cachet dessine)
    assert!(contient(&avec, "/PPImage1 Do") && contient(&avec, "/PPImage2 Do"));
    assert!(!contient(&sans, "/PPImage2 Do") && contient(&sans, "/PPImage1 Do"));
    assert!(aer::champs(&avec).unwrap()["EMPLOYEUR_SIGNATURE"]["valeur"].is_null());
}

#[test]
fn signature_svg() {
    let sig = fichier("signature2.svg", SVG_SIGNATURE.as_bytes());
    let sans = signer(&attestation(), &fiche(), None, false, None, None).unwrap();
    let avec = signer(&attestation(), &fiche(), None, false, Some(&sig), None).unwrap();
    assert!(avec.len() > sans.len());
    assert!(contient(&avec, "/PPImage2 Do"));
}

#[test]
fn cachet_fourni_par_la_compagnie_l_emporte_sur_le_dessin() {
    let tampon = fichier("tampon.png", &png(60, 30));
    let dessine = signer(&attestation(), &fiche(), None, false, None, None).unwrap();
    let fourni = signer(&attestation(), &fiche(), None, false, None, Some(&tampon)).unwrap();
    assert_ne!(dessine, fourni);
    assert!(contient(&fourni, "/PPImage1 Do"));
}

#[test]
fn signature_absente_n_est_pas_une_erreur() {
    let sans = signer(&attestation(), &fiche(), None, false, Some("/nulle/part.png"), None).unwrap();
    // seul le cachet est pose
    assert!(contient(&sans, "/PPImage1 Do") && !contient(&sans, "/PPImage2 Do"));
    assert_eq!(signer(&attestation(), &fiche(), None, false, None, None).unwrap(), sans);
}

#[test]
fn une_signature_qui_n_est_pas_une_image_est_refusee() {
    let sig = fichier("fausse-signature.png", b"ceci n'est pas une image");
    match signer(&attestation(), &fiche(), None, false, Some(&sig), None) {
        Err(Echec::Fabrique(m)) => assert!(m.contains("fausse-signature.png"), "{}", m),
        autre => panic!("{:?}", autre.map(|o| o.len())),
    }
}

#[test]
fn un_pdf_sans_les_cases_n_a_pas_d_image() {
    let sans: Vec<Champ> = champs_aer(2, 12).into_iter().filter(|c| c.nom != "EMPLOYEUR_SIGNATURE" && c.nom != "EMPLOYEUR_CACHET").collect();
    let signe = signer(&formulaire(&sans), &fiche(), None, false, None, None).unwrap();
    assert!(!contient(&signe, "PPImage"));
    assert_eq!(valeur_de(&aer::champs(&signe).unwrap(), "EMPLOYEUR_NOM"), "MARTIN");
}

#[test]
fn un_pdf_qui_n_en_est_pas_un_est_refuse_par_la_fabrique() {
    assert!(matches!(aer::verifier(b"ceci n'est pas un pdf", &[]), Err(Echec::Fabrique(_))));
}

// ------------------------------------------------------------------ remplir

#[test]
fn remplir_un_champ_de_france_travail_est_refuse() {
    for nom in ["SAL_NB_CACHET_01", "SAL_NB_HEURES_01", "CON_NUMERO_01"] {
        let mut v = serde_json::Map::new();
        v.insert(nom.into(), Value::String("999".into()));
        match aer::remplir(&attestation(), &v, &[]) {
            Err(Echec::Fabrique(m)) => assert!(m.contains("lecture seule"), "{}", m),
            autre => panic!("un chiffre de France Travail a ete retouche : {:?}", autre.map(|o| o.len())),
        }
    }
}

#[test]
fn remplir_un_peigne_trop_long_est_refuse() {
    let mut v = serde_json::Map::new();
    v.insert("EMPLOYEUR_DATE_SIGN_J".into(), Value::String("123".into()));
    match aer::remplir(&attestation(), &v, &[]) {
        Err(Echec::Fabrique(m)) => assert!(m.contains("EMPLOYEUR_DATE_SIGN_J"), "{}", m),
        autre => panic!("{:?}", autre.map(|o| o.len())),
    }
}

// ------------------------------------------------------------------ cachet

const CASE_CACHET: [f64; 4] = [401.4, 685.4, 562.1, 764.7];

#[test]
fn cachet_image_sans_fichier_rend_le_dessin() {
    let im = cachet::image(&json!(3), &json!(CASE_CACHET), &fiche(), Some(&jour(2026, 10, 1)), None).unwrap();
    let mut cles: Vec<&String> = im.as_object().unwrap().keys().collect();
    cles.sort();
    assert_eq!(cles, ["marge", "page", "rect", "svg"]);
    assert_eq!(im["page"], 3);
    assert_eq!(im["marge"], cachet::MARGE);
    let svg = im["svg"].as_str().unwrap();
    assert!(svg.starts_with("<svg") && svg.contains("COMPAGNIE DU CHAT PERCHE"));
    assert!(svg.contains("SIRET 12345678900019"));
    // un fichier qui n'existe pas : le dessin reprend la main
    let im2 = cachet::image(&json!(3), &json!([0, 0, 100, 50]), &fiche(), Some(&jour(2026, 10, 1)), Some("/nulle/part.png")).unwrap();
    assert!(im2.get("svg").is_some() && im2.get("fichier").is_none());
}

#[test]
fn cachet_image_avec_un_fichier_existant_rend_le_fichier() {
    let f = fichier("tampon2.png", &png(10, 10));
    let im = cachet::image(&json!(3), &json!([1, 2, 3, 4]), &fiche(), Some(&jour(2026, 10, 1)), Some(&f)).unwrap();
    assert_eq!(im, json!({"page": 3, "rect": [1, 2, 3, 4], "marge": cachet::MARGE, "fichier": f}));
}

#[test]
fn cachet_image_sans_rien_a_dessiner_rend_none() {
    assert_eq!(cachet::image(&json!(3), &json!([0, 0, 100, 50]), &json!({}), Some(&jour(2026, 10, 1)), None), None);
}

#[test]
fn le_cachet_dessine_se_pose_sans_police_du_systeme() {
    // le SVG du cachet porte du texte (Helvetica) : pose par la fabrique en
    // traces, il ne demande aucune police -- et sort identique deux fois
    let im = cachet::image(&json!(3), &json!(CASE_CACHET), &fiche(), Some(&jour(2026, 10, 1)), None).unwrap();
    let a = aer::remplir(&attestation(), &serde_json::Map::new(), &[im.clone()]).unwrap();
    let b = aer::remplir(&attestation(), &serde_json::Map::new(), &[im]).unwrap();
    assert_eq!(a, b);
    assert!(contient(&a, "/PPImage1 Do"));
}

// ------------------------------------------------------------------ le dessin du cachet

/// La taille de police de chaque texte du dessin, dans l'ordre.
fn tailles(svg: &str) -> Vec<f64> {
    svg.split("font-size=\"").skip(1).map(|t| t[..t.find('"').unwrap()].parse().unwrap()).collect()
}

#[test]
fn le_cachet_retrecit_le_texte_qui_deborde_echappe_les_chevrons_et_choisit_la_licence_du_jour() {
    let svg = cachet::svg(&fiche(), Some(&jour(2026, 10, 1))).unwrap();
    assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"));
    for t in [">COMPAGNIE DU CHAT PERCHE<", ">12 rue des Lilas<", ">69003 Lyon<", ">SIRET 12345678900019<", ">APE 9001Z<", ">Licence PLATESV-R-2026-000000<"] {
        assert!(svg.contains(t), "{} : {}", t, svg);
    }
    assert_eq!(cachet::svg(&json!({}), None), None);

    let mut debordante = fiche();
    debordante["raison_sociale"] = json!("Compagnie du Chat Perche & Fils <Spectacles> l'ete Compagnie du Chat Perche & Fils <Spectacles> l'ete ");
    debordante["adresse"] = json!("Batiment C, escalier 4, residence des grands lilas bleus, 12 bis rue des Lilas");
    debordante["licence_spectacle"] = json!([{"depuis": "2024-01-01", "numero": "L-2024"}, {"depuis": "2026-06-01", "numero": "L-2026"}]);
    let deborde = cachet::svg(&debordante, Some(&jour(2026, 10, 1))).unwrap();
    assert!(deborde.contains("CHAT PERCHE &amp; FILS &lt;SPECTACLES&gt; L'ETE") && deborde.contains(">Licence L-2026<"), "{}", deborde);
    let (normal, petit) = (tailles(&svg), tailles(&deborde));
    assert!(petit[0] < normal[0] && petit[1] < normal[1], "le nom et l'adresse qui debordent retrecissent : {:?} {:?}", normal, petit);
    assert_eq!(petit[2..], normal[2..], "le reste garde sa taille");
}

#[test]
fn les_lignes_du_cachet_sautent_ce_que_la_fiche_ne_dit_pas() {
    let l = cachet::lignes(&fiche(), Some(&jour(2026, 10, 1)));
    assert_eq!(l.nom, "Compagnie du Chat Perche");
    assert_eq!(l.lieu, ["12 rue des Lilas", "69003 Lyon"]);
    assert_eq!(l.numeros, ["SIRET 12345678900019", "APE 9001Z", "Licence PLATESV-R-2026-000000"]);
    let l = cachet::lignes(&json!({"siret": "123", "commune": "Lyon"}), None);
    assert_eq!((l.nom.as_str(), l.lieu.clone(), l.numeros.clone()), ("", vec!["Lyon".to_string()], vec!["SIRET 123".to_string()]));
}

#[test]
fn la_licence_du_jour_suit_les_dates() {
    let mut e = fiche();
    e["licence_spectacle"] = json!([
        {"depuis": "2024-01-01", "numero": "L-2024"},
        {"depuis": "2026-06-01", "numero": "L-2026"},
        {"depuis": "2025-01-01", "numero": "L-2025"},
    ]);
    let a = |j: Option<Jour>| cachet::licence_du_jour(&e, j.as_ref());
    assert_eq!(a(Some(jour(2025, 3, 1))).as_deref(), Some("L-2025"));
    assert_eq!(a(Some(jour(2026, 10, 3))).as_deref(), Some("L-2026"));
    // avant la premiere : celle qui reste est la derniere de la liste telle qu'ecrite
    assert_eq!(a(Some(jour(2020, 1, 1))).as_deref(), Some("L-2025"));
    // sans jour : la derniere dans l'ordre des dates
    assert_eq!(a(None).as_deref(), Some("L-2026"));
    // une date illisible : pas de licence, sans lever
    e["licence_spectacle"] = json!([{"depuis": "pas une date", "numero": "X"}]);
    assert_eq!(cachet::licence_du_jour(&e, Some(&jour(2026, 1, 1))), None);
    // l'ampersand et les chevrons sont echappes
    e["raison_sociale"] = json!("Chat & <Fils>");
    assert!(cachet::svg(&e, None).unwrap().contains("CHAT &amp; &lt;FILS&gt;"));
}
