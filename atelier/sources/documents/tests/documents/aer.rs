//! L'AER remplie et signee par la fabrique (`documents::aer`), sur une attestation FICTIVE : la forme relue (le cadre 5
//! rempli, les chiffres de France Travail intacts), les chiffres egaux aux paies (une periode en trop ou en moins est un
//! ecart, un contrat sur deux mois se compare colonne par colonne), et le refus de signer ce qui ne concorde pas.
//!
//! Le PDF d'essai est ecrit ici en octets bruts (catalogue, AcroForm, trois
//! pages, champs widgets, table xref) : il imite l'AER de France Travail --
//! quantites imprimees en LECTURE SEULE a la page 2, cadre 5 a la page 3 (champs
//! texte, dates et telephone en peigne, courriel multiligne, cases signature et
//! cachet) -- sans emprunter un octet a une vraie attestation. La compagnie est
//! inventee de bout en bout.

use documents::aer::{self, Authentification, Echec, PaieAer};
use socle::date::Jour;
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


fn valeur_de(champs: &Value, nom: &str) -> Value {
    champs[nom]["valeur"].clone()
}

// ------------------------------------------------------------------ verifier

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
