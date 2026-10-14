//! Tests de `--inspecter` : la taille des pages, les mots et leurs boîtes, les images, les traits,
//! les polices, les métadonnées et le chiffrement, en une ligne de JSON (repère de PyMuPDF : origine
//! en haut à gauche, y vers le bas). Les PDF d'essai sont fabriqués par la fabrique elle-même, ou
//! écrits à la main (page tournée, image en ligne, chemins non peints, `/Info` en UTF-16).

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

mod commun;
use commun::{chemin, dossier_temp, flux, lancer, pdf_a_la_main, textes};

fn exemples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("exemples")
}

fn inspecter(pdf: &str) -> Value {
    let r = lancer(&["--inspecter", pdf]);
    assert_eq!(r.status.code(), Some(0), "{}", String::from_utf8_lossy(&r.stderr));
    let sortie = String::from_utf8(r.stdout).unwrap();
    assert_eq!(sortie.trim_end().lines().count(), 1, "une seule ligne de JSON");
    serde_json::from_str(&sortie).unwrap()
}

fn ecrire(dossier: &Path, nom: &str, octets: &[u8]) -> String {
    let p = dossier.join(nom);
    std::fs::write(&p, octets).unwrap();
    chemin(&p)
}

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap()
}

fn boite(v: &Value) -> [f64; 4] {
    let a = v.as_array().unwrap();
    assert_eq!(a.len(), 4);
    [f(&a[0]), f(&a[1]), f(&a[2]), f(&a[3])]
}

fn proche(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance
}

/// Un contrat de la fabrique : un titre, un paragraphe, une image, un tableau cadré, sur 2 pages.
fn contrat(largeur: f64, hauteur: f64) -> Vec<u8> {
    let para = |t: &str| json!({"type": "paragraphe", "morceaux": [{"texte": t}]});
    let desc = json!({
        "format": 1,
        "page": {"largeur": largeur, "hauteur": hauteur,
                 "marges": {"haut": 50.0, "droite": 50.0, "bas": 50.0, "gauche": 50.0}},
        "police": {"taille": 10.0, "interligne": 1.25, "couleur": "#000000"},
        "titre": "Contrat éàç — DUPONT",
        "documents": [{"blocs": [
            para("Bonjour le monde"),
            json!({"type": "image", "chemin": chemin(&exemples().join("signature.png")), "largeur": 92.0}),
            json!({"type": "tableau",
                   "colonnes": [{"largeur": 0.6}, {"largeur": 0.4, "alignement": "droite"}],
                   "filets_cellules": {"couleur": "#000000", "epaisseur": 0.5},
                   "lignes": [{"cellules": ["Salaire brut", "1 234,56"]}, {"cellules": ["Net", "987,65"]}]}),
        ]}, {"blocs": [para("Deuxième page")]}]
    });
    fabrique_pdf::fabriquer(&desc.to_string()).unwrap_or_else(|e| panic!("{}", e.0))
}

#[test]
fn contrat_de_la_fabrique_taille_mots_image_traits_polices_metadonnees() {
    let d = dossier_temp("inspection-contrat");
    let pdf = ecrire(&d, "contrat.pdf", &contrat(400.0, 500.0));
    let j = inspecter(&pdf);
    assert!(j["chiffrement"].is_null());
    assert_eq!(j["metadonnees"]["Title"], "Contrat éàç — DUPONT");
    let pages = j["pages"].as_array().unwrap();
    assert_eq!(pages.len(), 2);
    let p = &pages[0];
    assert_eq!((f(&p["largeur"]), f(&p["hauteur"])), (400.0, 500.0));
    assert_eq!(p["rotation"], 0);

    // Les polices : Liberation Sans, avec son préfixe de sous-ensemble.
    let polices: Vec<&str> = p["polices"].as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
    assert!(!polices.is_empty() && polices.iter().all(|n| n.contains("LiberationSans")), "{:?}", polices);

    // Un mot attendu, avec une boîte plausible : en haut de la page (y vers le bas), à droite de la
    // marge gauche, haute d'à peu près le corps (10 pt) : ascendant + descendant de Liberation.
    let mots = p["mots"].as_array().unwrap();
    let bonjour = mots.iter().find(|m| m["texte"] == "Bonjour").expect("le mot Bonjour");
    let b = boite(&bonjour["rect"]);
    assert!(proche(b[0], 50.0, 1.0), "x0 = marge gauche : {:?}", b);
    assert!(b[1] >= 50.0 && b[3] < 80.0, "juste sous la marge haute : {:?}", b);
    assert!(b[3] - b[1] > 9.0 && b[3] - b[1] < 13.0, "hauteur d'un corps de 10 : {:?}", b);
    assert!(b[2] - b[0] > 25.0 && b[2] - b[0] < 45.0, "largeur de « Bonjour » : {:?}", b);
    assert_eq!(bonjour["ligne"], 0);
    let le = mots.iter().find(|m| m["texte"] == "le").unwrap();
    let monde = mots.iter().find(|m| m["texte"] == "monde").unwrap();
    assert!(boite(&le["rect"])[0] > b[2] && boite(&monde["rect"])[0] > boite(&le["rect"])[2], "les mots vont de gauche à droite");
    assert_eq!(le["ligne"], 0);

    // L'image : 92 pt de large, marge gauche, sous le paragraphe.
    let images = p["images"].as_array().unwrap();
    assert_eq!(images.len(), 1);
    let i = boite(&images[0]);
    assert!(proche(i[2] - i[0], 92.0, 0.1) && proche(i[0], 50.0, 0.1), "{:?}", i);
    assert!(i[1] > b[3] && i[3] > i[1], "sous le paragraphe : {:?}", i);

    // Les traits : le cadre de chaque cellule (4 cellules, filets de 0,5 pt dessinés en rectangles
    // ou en traits), tous dans la page.
    let traits = p["traits"].as_array().unwrap();
    assert!(traits.len() >= 4, "{} traits", traits.len());
    for t in traits {
        let t = boite(t);
        assert!(t[0] >= 0.0 && t[2] <= 400.0 && t[1] >= 0.0 && t[3] <= 500.0 && t[0] <= t[2] && t[1] <= t[3], "{:?}", t);
    }

    // Le texte des mots, ligne par ligne, redonne celui de --texte (espaces ramenés à un).
    let lus = textes(&pdf);
    assert_eq!(lus.len(), 2);
    for (page, texte) in pages.iter().zip(&lus) {
        let attendu: Vec<String> = texte.lines().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).collect();
        let mut lignes: Vec<String> = Vec::new();
        let mut derniere = None;
        for m in page["mots"].as_array().unwrap() {
            let n = m["ligne"].as_u64().unwrap();
            if derniere == Some(n) {
                let l = lignes.last_mut().unwrap();
                l.push(' ');
                l.push_str(m["texte"].as_str().unwrap());
            } else {
                lignes.push(m["texte"].as_str().unwrap().to_string());
                derniere = Some(n);
            }
        }
        assert_eq!(lignes, attendu.into_iter().filter(|l| !l.is_empty()).collect::<Vec<_>>());
    }
    assert!(pages[1]["mots"].as_array().unwrap().iter().any(|m| m["texte"] == "Deuxième"));
}

#[test]
fn page_paysage() {
    let d = dossier_temp("inspection-paysage");
    let pdf = ecrire(&d, "paysage.pdf", &contrat(841.89, 595.28));
    let j = inspecter(&pdf);
    let p = &j["pages"][0];
    assert_eq!((f(&p["largeur"]), f(&p["hauteur"])), (841.89, 595.28));
    assert_eq!(p["rotation"], 0);
}

#[test]
fn pdf_verrouille_se_lit_et_dit_son_chiffrement() {
    let d = dossier_temp("inspection-verrou");
    let clair = ecrire(&d, "clair.pdf", &contrat(400.0, 500.0));
    let verrouille = chemin(&d.join("verrouille.pdf"));
    assert_eq!(lancer(&["--verrouiller", &clair, &verrouille]).status.code(), Some(0));
    let (a, b) = (inspecter(&clair), inspecter(&verrouille));
    assert_eq!(
        b["chiffrement"],
        json!({"filtre": "Standard", "V": 5, "R": 6, "longueur": 256, "P": -3372, "methode": "AESV3"})
    );
    assert!(a["chiffrement"].is_null());
    // Une fois déchiffré, le contenu est le même, métadonnées comprises.
    assert_eq!(a["pages"], b["pages"]);
    assert_eq!(a["metadonnees"]["Title"], b["metadonnees"]["Title"]);
    assert_eq!(b["metadonnees"]["Title"], "Contrat éàç — DUPONT");
}

/// Une page 200 × 100 tournée de 90° : `largeur` et `hauteur` sont celles de la page affichée
/// (100 × 200), mais les boîtes restent dans le repère de la page non tournée, comme PyMuPDF
/// (`search_for`, `get_drawings`) : le texte posé en (30, 10) est à x = 30, en bas de la page.
#[test]
fn page_tournee() {
    let d = dossier_temp("inspection-rotation");
    let objets = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [0 0 200 100] /Rotate 90 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /Contents 4 0 R /Resources << /Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >> >> >>"
            .to_string(),
        flux("", "BT /F1 10 Tf 30 10 Td (Tourne) Tj ET 20 60 40 20 re S"),
    ];
    let pdf = ecrire(&d, "tournee.pdf", &pdf_a_la_main(&objets, 0));
    let j = inspecter(&pdf);
    let p = &j["pages"][0];
    assert_eq!((f(&p["largeur"]), f(&p["hauteur"])), (100.0, 200.0));
    assert_eq!(p["rotation"], 90);
    assert_eq!(p["polices"], json!(["Helvetica"]));
    let b = boite(&p["mots"][0]["rect"]);
    // Helvetica non incorporée : ascendant 1,075 et descendant 0,299 (ceux de MuPDF), corps 10
    assert!(proche(b[0], 30.0, 0.01) && proche(b[1], 79.25, 0.01) && proche(b[3], 92.99, 0.01), "{:?}", b);
    assert_eq!(boite(&p["traits"][0]), [20.0, 20.0, 60.0, 40.0]);
}

#[test]
fn images_en_ligne_chemins_peints_et_formulaires() {
    let d = dossier_temp("inspection-dessins");
    // Page 200 × 300. Un chemin non peint (`n`) ne compte pas ; un rectangle tracé (`S`), un
    // triangle rempli (`f`) oui ; un carré rempli puis tracé (`f` puis `S`, même chemin) ne compte
    // que pour un seul dessin, comme le « fs » de PyMuPDF ; une image en ligne et une image `Do` ; du
    // texte après l'image en ligne se lit toujours.
    let page = "150 150 20 20 re f\n150 150 20 20 re S\n\
                0 0 5 5 re n\n\
                10 10 100 50 re S\n\
                1 0 0 rg 20 20 m 60 20 l 40 80 l f\n\
                q 50 0 0 40 20 30 cm\nBI /W 1 /H 1 /CS /G /BPC 8 ID A\nEI Q\n\
                q 30 0 0 20 100 200 cm /Im1 Do Q\n\
                q 1 0 0 1 0 100 cm /Fm1 Do Q\n\
                BT /F1 10 Tf 20 250 Td (Apres BI) Tj ET\n";
    let objets = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [0 0 200 300] >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /Contents 4 0 R /Resources << /Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >> \
         /XObject << /Im1 5 0 R /Fm1 6 0 R >> >> >>"
            .to_string(),
        flux("", page),
        flux("/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8", "A"),
        flux(
            "/Type /XObject /Subtype /Form /BBox [0 0 100 20] /Resources << /Font << /F2 << /Type /Font /Subtype /Type1 /BaseFont /Courier >> >> >>",
            "0 0 10 10 re f BT /F2 8 Tf 5 5 Td (Form) Tj ET",
        ),
    ];
    let pdf = ecrire(&d, "dessins.pdf", &pdf_a_la_main(&objets, 0));
    let p = &inspecter(&pdf)["pages"][0];

    let traits: Vec<[f64; 4]> = p["traits"].as_array().unwrap().iter().map(boite).collect();
    // y vers le bas : hauteur 300, donc y_haut = 300 − y_pdf
    assert_eq!(traits.len(), 4, "{:?}", traits);
    assert_eq!(traits[0], [150.0, 130.0, 170.0, 150.0], "rempli puis tracé : un seul dessin");
    assert_eq!(traits[1], [10.0, 240.0, 110.0, 290.0]);
    assert_eq!(traits[2], [20.0, 220.0, 60.0, 280.0]);
    assert_eq!(traits[3], [0.0, 190.0, 10.0, 200.0], "le rectangle du formulaire, décalé de 100 vers le haut");

    let images: Vec<[f64; 4]> = p["images"].as_array().unwrap().iter().map(boite).collect();
    assert_eq!(images, vec![[20.0, 230.0, 70.0, 270.0], [100.0, 80.0, 130.0, 100.0]]);

    assert_eq!(p["polices"], json!(["Helvetica", "Courier"]));
    let mots: Vec<&str> = p["mots"].as_array().unwrap().iter().map(|m| m["texte"].as_str().unwrap()).collect();
    assert_eq!(mots, vec!["Form", "Apres", "BI"]);
}

#[test]
fn metadonnees_en_utf16_et_pdfdocencoding() {
    let d = dossier_temp("inspection-info");
    let objets = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [0 0 100 100] >>".to_string(),
        "<< /Type /Page /Parent 2 0 R >>".to_string(),
        "<< /Title <FEFF00E9007400E9> /Author (Caf\\351) /Producer (Test) /Trapped /False >>".to_string(),
    ];
    let mut octets = pdf_a_la_main(&objets, 0);
    let texte = String::from_utf8(octets.clone()).unwrap().replace("/Root 1 0 R", "/Root 1 0 R /Info 4 0 R");
    octets = texte.into_bytes();
    let pdf = ecrire(&d, "info.pdf", &octets);
    let j = inspecter(&pdf);
    assert_eq!(j["metadonnees"], json!({"Title": "été", "Author": "Café", "Producer": "Test", "Trapped": "False"}));
    // sans /Info : un dictionnaire vide ; sans contenu : une page sans mots
    let sans = ecrire(&d, "sans.pdf", &pdf_a_la_main(&objets[..3], 0));
    let j = inspecter(&sans);
    assert_eq!(j["metadonnees"], json!({}));
    assert_eq!(j["pages"][0]["mots"], json!([]));
    assert_eq!((f(&j["pages"][0]["largeur"]), f(&j["pages"][0]["hauteur"])), (100.0, 100.0));
}

#[test]
fn refus_en_une_ligne_et_arguments_faux() {
    let d = dossier_temp("inspection-refus");
    let faux = ecrire(&d, "faux.pdf", b"ceci n'est pas un PDF");
    for (arg, mot) in [(faux.as_str(), "faux.pdf"), ("/introuvable/absent.pdf", "absent.pdf")] {
        let r = lancer(&["--inspecter", arg]);
        assert_eq!(r.status.code(), Some(1));
        assert!(r.stdout.is_empty());
        let ligne = String::from_utf8(r.stderr).unwrap();
        assert!(ligne.trim().lines().count() == 1 && ligne.contains(mot), "{:?}", ligne);
    }
    assert_eq!(lancer(&["--inspecter"]).status.code(), Some(2));
    assert_eq!(lancer(&["--inspecter", &faux, &faux]).status.code(), Some(2));
}
