//! Tests du champ `largeur_max` de `texte_fixe` (FORMAT.md) : le corps est réduit pour que le texte
//! tienne, exactement selon `corps_qui_tient` de `moteur/conges_spectacles.py` — le corps d'origine
//! tant que le texte y tient, sinon `corps × place ÷ largeur`, jamais sous 6 pt.

use lopdf::content::Content;
use lopdf::{Document, Object};
use serde_json::{json, Value};

fn description(blocs: Vec<Value>) -> Value {
    json!({
        "format": 2,
        "page": {"largeur": 400.0, "hauteur": 500.0,
                 "marges": {"haut": 0.0, "droite": 0.0, "bas": 0.0, "gauche": 0.0}},
        "police": {"taille": 10.0, "interligne": 1.25, "couleur": "#000000"},
        "documents": [{"blocs": blocs}]
    })
}

fn texte_fixe(texte: &str, taille: f64, extra: Value) -> Value {
    let mut b = json!({"type": "texte_fixe", "texte": texte, "x": 20.0, "y": 100.0, "taille": taille});
    for (k, v) in extra.as_object().unwrap() {
        b[k] = v.clone();
    }
    b
}

/// Les corps (opérateur `Tf`) des textes de la première page, dans l'ordre.
fn corps_poses(pdf: &[u8]) -> Vec<f32> {
    let doc = Document::load_mem(pdf).unwrap();
    let page = *doc.get_pages().values().next().unwrap();
    let flux = doc.get_page_content(page).unwrap();
    Content::decode(&flux)
        .unwrap()
        .operations
        .iter()
        .filter(|o| o.operator == "Tf")
        .map(|o| match &o.operands[1] {
            Object::Integer(i) => *i as f32,
            Object::Real(r) => *r,
            autre => panic!("corps inattendu {:?}", autre),
        })
        .collect()
}

/// La largeur d'un texte en points, d'après les avances de Liberation (les métriques d'Helvetica,
/// de Times ou de Courier) — ce que le `get_text_length` de PyMuPDF donne pour la police standard.
fn largeur(fichier: &str, texte: &str, corps: f32) -> f32 {
    let donnees = std::fs::read(format!("{}/polices/{}", env!("CARGO_MANIFEST_DIR"), fichier)).unwrap();
    let face = ttf_parser::Face::parse(&donnees, 0).unwrap();
    let upem = face.units_per_em() as f32;
    texte
        .chars()
        .map(|c| face.glyph_hor_advance(face.glyph_index(c).unwrap()).unwrap() as f32 / upem * corps)
        .sum()
}

/// L'algorithme de `corps_qui_tient`, tel qu'il est écrit en Python.
fn corps_qui_tient(fichier: &str, texte: &str, corps: f32, place: f32) -> f32 {
    let l = largeur(fichier, texte, corps);
    if l <= place {
        corps
    } else {
        (corps * place / l).max(6.0)
    }
}

fn fabriquer(blocs: Vec<Value>) -> Vec<u8> {
    fabrique_pdf::fabriquer(&description(blocs).to_string()).unwrap_or_else(|e| panic!("{}", e.0))
}

#[test]
fn le_corps_demande_est_gardé_quand_le_texte_tient() {
    let t = "Société Fictive";
    let l = largeur("LiberationSans-Regular.ttf", t, 13.92);
    let pdf = fabriquer(vec![
        texte_fixe(t, 13.92, json!({"largeur_max": l + 0.5})),
        texte_fixe(t, 13.92, json!({})),
    ]);
    let c = corps_poses(&pdf);
    assert_eq!(c.len(), 2);
    assert!((c[0] - 13.92).abs() < 1e-3 && (c[1] - 13.92).abs() < 1e-3, "{:?}", c);
}

#[test]
fn le_corps_est_reduit_au_prorata_de_la_place() {
    let t = "COMPAGNIE FICTIVE DES ESSAIS RÉUNIS";
    for (place, corps) in [(200.0f32, 13.92f32), (150.0, 10.0), (180.0, 12.0)] {
        let attendu = corps_qui_tient("LiberationSans-Regular.ttf", t, corps, place);
        assert!(attendu < corps && attendu > 6.0, "le cas ne réduit pas : {}", attendu);
        let pdf = fabriquer(vec![texte_fixe(t, corps as f64, json!({"largeur_max": place}))]);
        let c = corps_poses(&pdf);
        assert!((c[0] - attendu).abs() < 0.01, "place {} : corps {} au lieu de {}", place, c[0], attendu);
        // et le texte réduit tient dans la place
        assert!(largeur("LiberationSans-Regular.ttf", t, c[0]) <= place + 0.05);
    }
}

#[test]
fn le_corps_ne_descend_pas_sous_six_points() {
    let t = "UN TEXTE BIEN TROP LONG POUR UNE PLACE SI ÉTROITE";
    let pdf = fabriquer(vec![texte_fixe(t, 12.0, json!({"largeur_max": 20.0}))]);
    let c = corps_poses(&pdf);
    assert!((c[0] - 6.0).abs() < 1e-3, "{:?}", c);
    // Comme dans corps_qui_tient : le plancher vaut aussi pour un corps demandé plus petit.
    assert_eq!(corps_qui_tient("LiberationSans-Regular.ttf", t, 5.0, 20.0), 6.0);
    let pdf = fabriquer(vec![texte_fixe(t, 5.0, json!({"largeur_max": 20.0}))]);
    assert!((corps_poses(&pdf)[0] - 6.0).abs() < 1e-3);
}

#[test]
fn la_mesure_suit_la_police_et_le_gras() {
    let t = "Raison sociale fictive et trop longue";
    for (police, gras, fichier) in [
        ("times", false, "LiberationSerif-Regular.ttf"),
        ("times", true, "LiberationSerif-Bold.ttf"),
        ("courier", false, "LiberationMono-Regular.ttf"),
        ("helvetica", true, "LiberationSans-Bold.ttf"),
    ] {
        let attendu = corps_qui_tient(fichier, t, 11.0, 90.0);
        assert!(attendu < 11.0, "{} : le cas ne réduit pas", fichier);
        let pdf = fabriquer(vec![texte_fixe(t, 11.0, json!({"police": police, "gras": gras, "largeur_max": 90.0}))]);
        let c = corps_poses(&pdf);
        assert!((c[0] - attendu).abs() < 0.01, "{} : {} au lieu de {}", fichier, c[0], attendu);
    }
}

#[test]
fn largeur_max_ne_change_ni_l_alignement_ni_les_autres_blocs() {
    // Le texte réduit s'aligne sur sa largeur réduite (bord droit en x) ; un bloc voisin sans
    // largeur_max garde son corps.
    let pdf = fabriquer(vec![
        texte_fixe("TEXTE ASSEZ LONG POUR DEBORDER", 12.0, json!({"alignement": "droite", "x": 300.0, "largeur_max": 60.0})),
        texte_fixe("VOISIN", 12.0, json!({"y": 150.0})),
    ]);
    let c = corps_poses(&pdf);
    assert_eq!(c.len(), 2);
    assert!(c[0] < 12.0 && (c[1] - 12.0).abs() < 1e-3, "{:?}", c);
}

#[test]
fn une_largeur_max_nulle_ou_negative_est_refusee() {
    for v in [0.0, -5.0] {
        let e = fabrique_pdf::fabriquer(&description(vec![texte_fixe("x", 10.0, json!({"largeur_max": v}))]).to_string())
            .expect_err("refus attendu");
        assert!(e.0.contains("largeur_max") && !e.0.contains('\n'), "{:?}", e.0);
    }
}
