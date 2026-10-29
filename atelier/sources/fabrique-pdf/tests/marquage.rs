//! Tests de `--marquer` : un texte en pied de chaque page d'un PDF existant (6 pt, gris 0,45, à
//! 34 pt du bord gauche, ligne de base à 12 pt du bas), en Liberation Sans sous-ensemble avec sa table `ToUnicode`, sans toucher aux flux existants.

use lopdf::content::Content;
use lopdf::{Document, Object, ObjectId};

mod commun;
use commun::{chemin, dossier_temp, flux, lancer, octets_fabrique, pdf_a_la_main, pdf_fabrique, textes};

const MARQUE: &str = "Réédité par Test le 01/10/2026 — numéros identiques ; mise en page d'aujourd'hui.";

fn marquer(entree: &str, sortie: &str, texte: &str) -> std::process::Output {
    lancer(&["--marquer", entree, sortie, texte])
}

/// Les identifiants des flux de `/Contents` d'une page.
fn contenus(doc: &Document, page: ObjectId) -> Vec<ObjectId> {
    let c = doc.get_dictionary(page).unwrap().get(b"Contents").unwrap();
    match c {
        Object::Array(v) => v.iter().map(|o| o.as_reference().unwrap()).collect(),
        Object::Reference(id) => match doc.get_object(*id).unwrap() {
            Object::Array(v) => v.iter().map(|o| o.as_reference().unwrap()).collect(),
            _ => vec![*id],
        },
        _ => panic!("contenu inattendu"),
    }
}

fn flux_decode(doc: &Document, id: ObjectId) -> String {
    let s = doc.get_object(id).unwrap().as_stream().unwrap();
    String::from_utf8(s.decompressed_content().unwrap_or_else(|_| s.content.clone())).unwrap()
}

#[test]
fn le_texte_se_pose_en_pied_de_chaque_page_et_s_extrait() {
    let d = dossier_temp("marquage-texte");
    let a = pdf_fabrique(&d, "a.pdf", 3);
    let sortie = chemin(&d.join("marque.pdf"));
    let r = marquer(&a, &sortie, MARQUE);
    assert_eq!(r.status.code(), Some(0), "{}", String::from_utf8_lossy(&r.stderr));
    let avant = textes(&a);
    let apres = textes(&sortie);
    assert_eq!(apres.len(), 3);
    for (av, ap) in avant.iter().zip(&apres) {
        // le texte d'origine d'abord, la marque ensuite, sur sa propre ligne
        assert_eq!(ap, &format!("{}{}\n", av, MARQUE), "{:?}", ap);
    }
}

#[test]
fn la_marque_est_en_liberation_sans_sous_ensemble_avec_tounicode() {
    let d = dossier_temp("marquage-police");
    let a = pdf_fabrique(&d, "a.pdf", 2);
    let sortie = d.join("marque.pdf");
    assert_eq!(marquer(&a, &chemin(&sortie), MARQUE).status.code(), Some(0));
    let doc = Document::load(&sortie).unwrap();
    let page = *doc.get_pages().values().next().unwrap();
    // la police de la marque : Type0, Identity-H, ToUnicode, sous-ensemble de Liberation Sans
    let ressources = doc.get_dictionary(page).unwrap().get(b"Resources").unwrap().as_dict().unwrap();
    let polices = ressources.get(b"Font").unwrap().as_dict().unwrap();
    let (nom, reference) = polices.iter().find(|(n, _)| n.starts_with(b"PPMarque")).expect("police de la marque");
    let police = doc.get_dictionary(reference.as_reference().unwrap()).unwrap();
    assert_eq!(police.get(b"Subtype").unwrap().as_name().unwrap(), b"Type0");
    assert_eq!(police.get(b"Encoding").unwrap().as_name().unwrap(), b"Identity-H");
    let base = String::from_utf8(police.get(b"BaseFont").unwrap().as_name().unwrap().to_vec()).unwrap();
    assert!(base.contains("+LiberationSans"), "{}", base);
    let tounicode = doc.get_object(police.get(b"ToUnicode").unwrap().as_reference().unwrap()).unwrap().as_stream().unwrap();
    let table = String::from_utf8(tounicode.decompressed_content().unwrap()).unwrap();
    assert!(table.contains("beginbfchar") && table.contains("<2014> <2014>"), "{}", table);
    // le flux de la marque : couleur, corps, texte en codes de deux octets (le point de code)
    let dernier = *contenus(&doc, page).last().unwrap();
    let flux = flux_decode(&doc, dernier);
    let nom = String::from_utf8(nom.clone()).unwrap();
    assert!(flux.contains("0.45 0.45 0.45 rg") && flux.contains(&format!("/{} 6 Tf", nom)), "{}", flux);
    let attendu: String = MARQUE.chars().map(|c| format!("{:04X}", c as u32)).collect();
    assert!(flux.contains(&format!("<{}> Tj", attendu)), "{}", flux);
    // un sous-ensemble : le fichier reste léger (la police entière pèse 350 Ko)
    let taille_police = doc
        .objects
        .values()
        .filter_map(|o| o.as_stream().ok())
        .filter(|s| s.dict.get(b"Length1").is_ok())
        .map(|s| s.content.len())
        .max()
        .unwrap_or(0);
    assert!(taille_police > 0 && taille_police < 40_000, "police de {} octets", taille_police);
}

#[test]
fn position_pied_a_34_et_12_du_bas_selon_le_cadre_et_la_rotation() {
    let d = dossier_temp("marquage-position");
    let page = "BT /F1 12 Tf 50 700 Td (Texte) Tj ET\n";
    let ressources = "/Resources << /Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >> >>";
    let cas = [
        // (attributs de la page, la matrice et la position attendues)
        ("/MediaBox [0 0 595 842]", "1 0 0 1 34 12 Tm"),
        ("/MediaBox [0 0 595 842] /CropBox [10 20 310 420]", "1 0 0 1 44 32 Tm"),
        ("/MediaBox [0 0 300 400] /Rotate 90", "0 1 -1 0 288 34 Tm"),
        ("/MediaBox [0 0 300 400] /Rotate 180", "-1 0 0 -1 266 388 Tm"),
        ("/MediaBox [0 0 300 400] /Rotate 270", "0 -1 1 0 12 366 Tm"),
    ];
    for (k, (attributs, attendu)) in cas.iter().enumerate() {
        let objets = vec![
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            format!("<< /Type /Page /Parent 2 0 R /Contents 4 0 R {} {} >>", attributs, ressources),
            flux("", page),
        ];
        let entree = d.join(format!("p{}.pdf", k));
        std::fs::write(&entree, pdf_a_la_main(&objets, 0)).unwrap();
        let sortie = d.join(format!("m{}.pdf", k));
        let r = marquer(&chemin(&entree), &chemin(&sortie), "Pied");
        assert_eq!(r.status.code(), Some(0), "{}", String::from_utf8_lossy(&r.stderr));
        let doc = Document::load(&sortie).unwrap();
        let id = *doc.get_pages().values().next().unwrap();
        let marque = flux_decode(&doc, *contenus(&doc, id).last().unwrap());
        assert!(marque.contains(attendu), "{} : {:?} ne contient pas {}", attributs, marque, attendu);
    }
}

#[test]
fn les_flux_existants_ne_sont_pas_touches_et_l_etat_graphique_est_rendu() {
    // Un contenu qui change la matrice et laisse un `q` ouvert : la marque doit tout de même
    // partir d'un état neuf.
    let d = dossier_temp("marquage-etat");
    let page = "q 2 0 0 2 0 0 cm\nBT /F1 12 Tf 10 100 Td (Dessus) Tj ET\n";
    let objets = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [0 0 595 842] >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /Contents 4 0 R /Resources << /Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >> >> >>".to_string(),
        flux("", page),
    ];
    let entree = d.join("ouvert.pdf");
    std::fs::write(&entree, pdf_a_la_main(&objets, 0)).unwrap();
    let sortie = d.join("ouvert-marque.pdf");
    assert_eq!(marquer(&chemin(&entree), &chemin(&sortie), "Pied").status.code(), Some(0));

    let avant = Document::load(&entree).unwrap();
    let apres = Document::load(&sortie).unwrap();
    let id_avant = *avant.get_pages().values().next().unwrap();
    let id_apres = *apres.get_pages().values().next().unwrap();
    let anciens = contenus(&avant, id_avant);
    let nouveaux = contenus(&apres, id_apres);
    // q, l'ancien flux (même objet, mêmes octets), les Q, la marque
    assert_eq!(nouveaux.len(), anciens.len() + 3, "{:?}", nouveaux);
    assert_eq!(flux_decode(&apres, nouveaux[0]).trim(), "q");
    assert_eq!(nouveaux[1], anciens[0]);
    assert_eq!(flux_decode(&apres, nouveaux[1]), flux_decode(&avant, anciens[0]));
    assert_eq!(flux_decode(&apres, nouveaux[2]), "Q\nQ\n", "un q resté ouvert, plus le nôtre");
    // le contenu entier est équilibré : autant de q que de Q
    let tout: String = nouveaux.iter().map(|i| flux_decode(&apres, *i)).collect::<Vec<_>>().join("\n");
    let ops = Content::decode(tout.as_bytes()).unwrap().operations;
    let q = ops.iter().filter(|o| o.operator == "q").count();
    let grand_q = ops.iter().filter(|o| o.operator == "Q").count();
    assert_eq!(q, grand_q, "{}", tout);
    // l'extraction : le texte d'origine puis la marque
    assert_eq!(textes(&chemin(&sortie)), vec!["Dessus\nPied\n"]);
}

#[test]
fn les_ressources_partagees_par_un_noeud_pages_ne_sont_pas_modifiees() {
    let d = dossier_temp("marquage-ressources");
    let objets = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 /MediaBox [0 0 595 842] \
         /Resources << /Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >> >> >>"
            .to_string(),
        "<< /Type /Page /Parent 2 0 R /Contents 5 0 R >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /Contents 6 0 R >>".to_string(),
        flux("", "BT /F1 12 Tf 10 100 Td (Un) Tj ET"),
        flux("", "BT /F1 12 Tf 10 100 Td (Deux) Tj ET"),
    ];
    let entree = d.join("partage.pdf");
    std::fs::write(&entree, pdf_a_la_main(&objets, 0)).unwrap();
    let sortie = d.join("partage-marque.pdf");
    assert_eq!(marquer(&chemin(&entree), &chemin(&sortie), "Pied").status.code(), Some(0));
    assert_eq!(textes(&chemin(&sortie)), vec!["Un\nPied\n", "Deux\nPied\n"]);
    let doc = Document::load(&sortie).unwrap();
    // le nœud Pages garde ses ressources d'origine : pas de police de marque
    let racine = doc.catalog().unwrap().get(b"Pages").unwrap().as_reference().unwrap();
    let pages = doc.get_dictionary(racine).unwrap();
    let polices = pages.get(b"Resources").unwrap().as_dict().unwrap().get(b"Font").unwrap().as_dict().unwrap();
    assert_eq!(polices.len(), 1);
    // chaque page a les siennes, avec la police d'origine et celle de la marque
    for (_, id) in doc.get_pages() {
        let p = doc.get_dictionary(id).unwrap().get(b"Resources").unwrap().as_dict().unwrap();
        let f = p.get(b"Font").unwrap().as_dict().unwrap();
        assert!(f.has(b"F1") && f.has(b"PPMarque"), "{:?}", f);
    }
}

#[test]
fn un_texte_vide_laisse_le_fichier_tel_quel_et_le_meme_nom_marque_en_place() {
    let d = dossier_temp("marquage-vide");
    let a = pdf_fabrique(&d, "a.pdf", 1);
    let sortie = chemin(&d.join("vide.pdf"));
    assert_eq!(marquer(&a, &sortie, "").status.code(), Some(0));
    assert_eq!(std::fs::read(&sortie).unwrap(), std::fs::read(&a).unwrap());
    // en place : l'entrée est aussi la sortie
    assert_eq!(marquer(&a, &a, "En place").status.code(), Some(0));
    assert_eq!(textes(&a), vec!["Page 1 éàç œ\nEn place\n"]);
}

#[test]
fn meme_entree_meme_texte_memes_octets() {
    let d = dossier_temp("marquage-determinisme");
    let a = pdf_fabrique(&d, "a.pdf", 2);
    let (x, y) = (chemin(&d.join("x.pdf")), chemin(&d.join("y.pdf")));
    assert_eq!(marquer(&a, &x, MARQUE).status.code(), Some(0));
    assert_eq!(marquer(&a, &y, MARQUE).status.code(), Some(0));
    assert_eq!(std::fs::read(&x).unwrap(), std::fs::read(&y).unwrap());
}

#[test]
fn les_refus_tiennent_en_une_ligne_et_n_ecrivent_rien() {
    let d = dossier_temp("marquage-refus");
    let a = pdf_fabrique(&d, "a.pdf", 1);
    let faux = d.join("faux.pdf");
    std::fs::write(&faux, b"ceci n'est pas un PDF").unwrap();
    let sortie = d.join("jamais.pdf");
    let _ = std::fs::remove_file(&sortie);
    for (entree, texte, mot) in [
        (chemin(&faux), "Pied", "faux.pdf"),
        (chemin(&d.join("absent.pdf")), "Pied", "absent.pdf"),
        (a.clone(), "日本語", "n'existe pas dans la police"),
    ] {
        let r = marquer(&entree, &chemin(&sortie), texte);
        assert_eq!(r.status.code(), Some(1), "{}", entree);
        let ligne = String::from_utf8(r.stderr).unwrap();
        assert!(ligne.trim().lines().count() == 1 && ligne.contains(mot), "{:?}", ligne);
        assert!(!sortie.exists(), "rien ne doit être écrit en cas de refus");
    }
    // sans les trois arguments : l'usage, code 2
    assert_eq!(lancer(&["--marquer", &a, &chemin(&sortie)]).status.code(), Some(2));
    let _ = octets_fabrique(1);
}
