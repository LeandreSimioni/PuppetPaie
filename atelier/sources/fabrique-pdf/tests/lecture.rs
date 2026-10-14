//! Tests de la lecture des PDF existants : `--pages` (nombre de pages) et `--texte` (texte de
//! chaque page). Les PDF d'essai sont fictifs : écrits à la main ici (comme le ferait un outil
//! tiers : polices standard, flux de contenu en tableau, `/ActualText`, champ de formulaire) ou
//! fabriqués par la fabrique elle-même.

use serde_json::json;

mod commun;
use commun::{dossier_temp, flux, lancer, pdf_a_la_main, pdf_fabrique};

/// Le PDF « tiers » de ces tests : une page dont `/Contents` renvoie à un tableau de deux flux,
/// une police standard sans largeurs, un objet formulaire, un `/ActualText` et un champ de
/// formulaire (apparence dans `/AP`).
fn pdf_tiers(decalage: usize) -> Vec<u8> {
    let page1 = "BT /F1 12 Tf 50 700 Td (Bonjour) Tj ( le monde) Tj ET\n\
                 BT /F1 12 Tf 50 680 Td [(Ligne) -300 (deux)] TJ ET\n";
    let page2 = "BT /F1 12 Tf 50 660 Td (a) Tj 200 0 Td (b) Tj ET\n\
                 /Span << /ActualText (Reel) >> BDC BT /F1 12 Tf 50 640 Td (xxxx) Tj ET EMC\n\
                 q 1 0 0 1 50 600 cm /Fm1 Do Q\n\
                 BT /F1 12 Tf 50 560 Td (\\351t\\351 caf\\351) Tj ET\n";
    let objets = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [0 0 595 842] >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /Contents 4 0 R /Annots [8 0 R] \
         /Resources << /Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >> >> \
         /XObject << /Fm1 7 0 R >> >> >>"
            .to_string(),
        "[5 0 R 6 0 R]".to_string(),
        flux("", page1),
        flux("", page2),
        flux(
            "/Type /XObject /Subtype /Form /BBox [0 0 100 20] /Resources << /Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >> >>",
            "BT /F1 12 Tf 0 5 Td (FORMULAIRE) Tj ET",
        ),
        "<< /Type /Annot /Subtype /Widget /FT /Tx /Rect [300 300 400 320] /F 4 /AP << /N 9 0 R >> >>".to_string(),
        flux(
            "/Type /XObject /Subtype /Form /BBox [0 0 100 20] /Resources << /Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >> >>",
            "BT /F1 10 Tf 2 5 Td (CHAMP) Tj ET",
        ),
    ];
    pdf_a_la_main(&objets, decalage)
}

#[test]
fn pages_donne_le_nombre_de_pages_de_chaque_fichier() {
    let d = dossier_temp("lecture-pages");
    let a = pdf_fabrique(&d, "a.pdf", 3);
    let b = pdf_fabrique(&d, "b.pdf", 1);
    let r = lancer(&["--pages", &a, &b]);
    assert_eq!(r.status.code(), Some(0));
    let n: Vec<u64> = serde_json::from_slice(&r.stdout).unwrap();
    assert_eq!(n, vec![3, 1]);
    // un seul fichier : un tableau d'un nombre
    let r = lancer(&["--pages", &a]);
    assert_eq!(String::from_utf8(r.stdout).unwrap().trim(), "[3]");
}

#[test]
fn pages_refuse_en_une_ligne_et_nomme_le_fichier() {
    let d = dossier_temp("lecture-pages-refus");
    let a = pdf_fabrique(&d, "a.pdf", 2);
    let faux = d.join("faux.pdf");
    std::fs::write(&faux, b"ceci n'est pas un PDF").unwrap();
    let faux = faux.to_str().unwrap().replace('\\', "/");
    for (args, mot) in [
        (vec!["--pages", &a, &faux], "faux.pdf"),
        (vec!["--pages", "/introuvable/absent.pdf"], "absent.pdf"),
    ] {
        let r = lancer(&args);
        assert_eq!(r.status.code(), Some(1));
        assert!(r.stdout.is_empty(), "rien sur la sortie en cas de refus");
        let ligne = String::from_utf8(r.stderr).unwrap();
        assert!(ligne.trim().lines().count() == 1 && ligne.contains(mot), "{:?}", ligne);
    }
    // sans fichier : l'usage, code 2
    assert_eq!(lancer(&["--pages"]).status.code(), Some(2));
}

#[test]
fn texte_d_un_pdf_de_la_fabrique() {
    let d = dossier_temp("lecture-texte-fabrique");
    let a = pdf_fabrique(&d, "a.pdf", 3);
    let r = lancer(&["--texte", &a]);
    assert_eq!(r.status.code(), Some(0), "{}", String::from_utf8_lossy(&r.stderr));
    let t: Vec<String> = serde_json::from_slice(&r.stdout).unwrap();
    assert_eq!(t, vec!["Page 1 éàç œ\n", "Page 2 éàç œ\n", "Page 3 éàç œ\n"]);
}

#[test]
fn texte_d_un_paragraphe_long_une_ligne_par_ligne_du_pdf() {
    let d = dossier_temp("lecture-texte-lignes");
    let desc = json!({
        "format": 1,
        "page": {"largeur": 200.0, "hauteur": 300.0,
                 "marges": {"haut": 20.0, "droite": 20.0, "bas": 20.0, "gauche": 20.0}},
        "police": {"taille": 10.0, "interligne": 1.25, "couleur": "#000000"},
        "documents": [{"blocs": [{"type": "paragraphe", "morceaux": [{"texte":
            "Un paragraphe assez long pour passer à la ligne plusieurs fois dans une page étroite, sans rien perdre."}]}]}]
    });
    let octets = fabrique_pdf::fabriquer(&desc.to_string()).unwrap();
    let p = d.join("long.pdf");
    std::fs::write(&p, octets).unwrap();
    let r = lancer(&["--texte", p.to_str().unwrap()]);
    let t: Vec<String> = serde_json::from_slice(&r.stdout).unwrap();
    assert_eq!(t.len(), 1);
    let lignes: Vec<&str> = t[0].lines().collect();
    assert!(lignes.len() >= 3, "{:?}", lignes);
    // les lignes recollées donnent la phrase, sans mot coupé ni perdu
    assert_eq!(
        lignes.join(" "),
        "Un paragraphe assez long pour passer à la ligne plusieurs fois dans une page étroite, sans rien perdre."
    );
    assert!(t[0].ends_with('\n'));
}

#[test]
fn texte_d_un_pdf_tiers_contenus_en_tableau_actualtext_formulaire_et_champ() {
    let d = dossier_temp("lecture-texte-tiers");
    let p = d.join("tiers.pdf");
    std::fs::write(&p, pdf_tiers(0)).unwrap();
    let r = lancer(&["--texte", p.to_str().unwrap()]);
    assert_eq!(r.status.code(), Some(0), "{}", String::from_utf8_lossy(&r.stderr));
    let t: Vec<String> = serde_json::from_slice(&r.stdout).unwrap();
    assert_eq!(t.len(), 1);
    // dans l'ordre du flux : le tableau de flux (/Contents -> [5 6]), puis les champs
    assert_eq!(
        t[0],
        "Bonjour le monde\nLigne deux\na\nb\nReel\nFORMULAIRE\nété café\nCHAMP\n",
        "{:?}",
        t[0]
    );
}

#[test]
fn texte_d_un_pdf_dont_la_table_de_references_est_fausse() {
    // « Microsoft Print To PDF » écrit des positions décalées d'un octet : le fichier se lit
    // quand même, comme le font les lecteurs de PDF.
    let d = dossier_temp("lecture-texte-repare");
    let p = d.join("decale.pdf");
    std::fs::write(&p, pdf_tiers(1)).unwrap();
    let r = lancer(&["--pages", p.to_str().unwrap()]);
    assert_eq!(String::from_utf8(r.stdout).unwrap().trim(), "[1]");
    let r = lancer(&["--texte", p.to_str().unwrap()]);
    let t: Vec<String> = serde_json::from_slice(&r.stdout).unwrap();
    assert!(t[0].starts_with("Bonjour le monde\n") && t[0].contains("CHAMP"), "{:?}", t);
}

#[test]
fn texte_refuse_un_fichier_illisible_en_une_ligne() {
    let d = dossier_temp("lecture-texte-refus");
    let faux = d.join("faux.pdf");
    std::fs::write(&faux, b"ceci n'est pas un PDF").unwrap();
    let r = lancer(&["--texte", faux.to_str().unwrap()]);
    assert_eq!(r.status.code(), Some(1));
    assert!(r.stdout.is_empty());
    let ligne = String::from_utf8(r.stderr).unwrap();
    assert!(ligne.trim().lines().count() == 1 && ligne.contains("faux.pdf"), "{:?}", ligne);
    assert_eq!(lancer(&["--texte"]).status.code(), Some(2));
}

#[test]
fn les_lignes_distinctes_ne_se_collent_pas_et_l_exposant_reste_dans_sa_ligne() {
    // Règle de MuPDF : un exposant (décalage de ligne de base < 0,8 corps) reste dans la ligne ;
    // une ligne plus bas, ou un saut vers la droite de plus de 0,8 corps, ouvre une ligne.
    let d = dossier_temp("lecture-texte-exposant");
    let page = "BT /F1 10 Tf 50 700 Td (1) Tj /F1 6 Tf 1 0 0 1 55.56 704 Tm (er) Tj \
                /F1 10 Tf 1 0 0 1 60.9 700 Tm ( mai) Tj ET\n\
                BT /F1 10 Tf 50 680 Td (Total) Tj 100 0 Td (42) Tj ET\n";
    let objets = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [0 0 595 842] >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /Contents 4 0 R /Resources << /Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >> >> >>".to_string(),
        flux("", page),
    ];
    let p = d.join("exposant.pdf");
    std::fs::write(&p, pdf_a_la_main(&objets, 0)).unwrap();
    let r = lancer(&["--texte", p.to_str().unwrap()]);
    let t: Vec<String> = serde_json::from_slice(&r.stdout).unwrap();
    assert_eq!(t[0], "1er mai\nTotal\n42\n", "{:?}", t[0]);
}

#[test]
fn les_polices_composees_lisent_tounicode_meme_abimee_et_les_polices_simples_leurs_differences() {
    let d = dossier_temp("lecture-texte-polices");
    // Une police Type0 (Identity-H, codes de deux octets) : `ToUnicode` donne les caractères
    // (une plage et un code isolé) ; le code 5 n'y est pas (U+FFFD) ; un « > » isolé, comme en
    // met un fichier abîmé, ne bloque pas la lecture.
    let tounicode = flux(
        "",
        "/CIDInit /ProcSet findresource begin 12 dict begin begincmap\n\
         1 begincodespacerange <0000> <FFFF> endcodespacerange\n\
         > > 1 beginbfrange <0001> <0003> <0041> endbfrange\n\
         1 beginbfchar <0004> <00E9> endbfchar\nendcmap",
    );
    let page = "BT /F1 12 Tf 50 700 Td <00010002000300040005> Tj ET\n\
                BT /F2 12 Tf 50 680 Td (ABC \\200) Tj ET\n";
    let objets = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [0 0 595 842] >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /Contents 4 0 R /Resources << /Font << /F1 5 0 R /F2 7 0 R >> >> >>".to_string(),
        flux("", page),
        "<< /Type /Font /Subtype /Type0 /BaseFont /Test /Encoding /Identity-H /DescendantFonts [6 0 R] /ToUnicode 8 0 R >>".to_string(),
        "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /Test /DW 500 /W [1 [600 600] 3 3 700] >>".to_string(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding << /Type /Encoding /BaseEncoding /WinAnsiEncoding \
         /Differences [65 /eacute /uni00E0] >> >>"
            .to_string(),
        tounicode,
    ];
    let p = d.join("polices.pdf");
    std::fs::write(&p, pdf_a_la_main(&objets, 0)).unwrap();
    let r = lancer(&["--texte", p.to_str().unwrap()]);
    assert_eq!(r.status.code(), Some(0), "{}", String::from_utf8_lossy(&r.stderr));
    let t: Vec<String> = serde_json::from_slice(&r.stdout).unwrap();
    // Les codes 1 à 5 : A B C é ; le 5 n'a pas de texte. Les différences : A -> é, B -> à, C reste
    // « C » (WinAnsi), \200 -> « € » (WinAnsi).
    assert_eq!(t[0], "ABCé\u{FFFD}\néàC €\n", "{:?}", t[0]);
}
