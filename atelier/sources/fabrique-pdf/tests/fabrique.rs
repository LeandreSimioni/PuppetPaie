//! Tests de la fabrique de PDF : un test par type de bloc, les refus, le déterminisme, la
//! justification et l'exemple `exemples/contrat.json`.
//!
//! Les PDF produits sont relus avec lopdf : on décode le flux de contenu de chaque page pour y
//! retrouver le texte, sa position et ses largeurs. La bibliothèque de la fabrique (`lecture::texte`
//! et `inspection::inspecter`, le repère de PyMuPDF) donne une seconde lecture, indépendante de
//! celle-là : texte extrait, boîtes des mots, des images et des traits. Rien n'est sauté.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use lopdf::content::Content;
use lopdf::{Document, Object};
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Outils.
// ---------------------------------------------------------------------------

fn exemples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("exemples")
}

fn dossier_temp(nom: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(nom);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Un chemin en UTF-8 avec des barres obliques (valable aussi sous Windows).
fn chemin(p: &Path) -> String {
    p.to_str().unwrap().replace('\\', "/")
}

fn image(nom: &str) -> String {
    chemin(&exemples().join(nom))
}

fn fabriquer(v: &Value) -> Result<Vec<u8>, String> {
    fabrique_pdf::fabriquer(&v.to_string()).map_err(|r| r.0)
}

fn ok(v: &Value) -> Vec<u8> {
    fabriquer(v).unwrap_or_else(|e| panic!("refus inattendu : {}", e))
}

fn refus(v: &Value) -> String {
    match fabriquer(v) {
        Ok(_) => panic!("la description aurait dû être refusée"),
        Err(e) => {
            assert!(!e.contains('\n'), "le message doit tenir sur une ligne : {:?}", e);
            e
        }
    }
}

/// Une description minimale, marges de 50 sur une page de 400 x 500.
fn base(blocs: Vec<Value>) -> Value {
    json!({
        "format": 1,
        "page": {"largeur": 400.0, "hauteur": 500.0,
                 "marges": {"haut": 50.0, "droite": 50.0, "bas": 50.0, "gauche": 50.0}},
        "police": {"taille": 10.0, "interligne": 1.25, "couleur": "#000000"},
        "documents": [{"blocs": blocs}]
    })
}

fn para(texte: &str) -> Value {
    json!({"type": "paragraphe", "morceaux": [{"texte": texte}]})
}

fn para_align(texte: &str, alignement: &str) -> Value {
    json!({"type": "paragraphe", "morceaux": [{"texte": texte}], "alignement": alignement})
}

/// Une ligne de texte relue dans le flux de contenu.
#[derive(Debug, Clone)]
struct Ligne {
    page: usize,
    x: f32,
    y: f32,
    /// Largeur de la ligne, lettres et espacement des mots compris (points).
    largeur: f32,
    texte: String,
    mot_espace: f32,
}

fn face(nom: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("polices")
            .join(nom),
    )
    .unwrap()
}

fn nombre(o: &Object) -> f32 {
    match o {
        Object::Integer(i) => *i as f32,
        Object::Real(r) => *r,
        _ => panic!("nombre attendu : {:?}", o),
    }
}

/// Le fichier de Liberation d'une police du PDF, d'après son `BaseFont` (« ABCDEF+LiberationSans-Bold »).
fn fichier_de_police(base_font: &str) -> String {
    let nom = base_font.split('+').last().unwrap();
    if nom.contains('-') {
        format!("{}.ttf", nom)
    } else {
        format!("{}-Regular.ttf", nom)
    }
}

/// Les polices d'une page : nom de ressource (F0, F1…) -> dictionnaire de la police. Les
/// ressources et le dictionnaire `Font` sont des objets à part (`lopdf::get_page_fonts` ne les suit pas).
fn polices_de_page(doc: &Document, page: lopdf::ObjectId) -> Vec<(Vec<u8>, &lopdf::Dictionary)> {
    fn suivre<'a>(doc: &'a Document, o: &'a Object) -> Option<&'a lopdf::Dictionary> {
        match o {
            Object::Reference(r) => doc.get_dictionary(*r).ok(),
            Object::Dictionary(d) => Some(d),
            _ => None,
        }
    }
    let page = doc.get_dictionary(page).unwrap();
    let ressources = suivre(doc, page.get(b"Resources").unwrap()).unwrap();
    let polices = match ressources.get(b"Font").ok().and_then(|f| suivre(doc, f)) {
        Some(p) => p,
        None => return Vec::new(),
    };
    polices
        .iter()
        .map(|(nom, o)| (nom.clone(), suivre(doc, o).unwrap()))
        .collect()
}

/// Les polices du PDF, par nom PostScript sans l'étiquette du sous-ensemble, triées : chacune est
/// embarquée (`FontFile2`) et le fichier ne porte que les glyphes imprimés.
fn noms_de_polices(pdf: &[u8]) -> Vec<String> {
    let doc = Document::load_mem(pdf).unwrap();
    let mut noms: Vec<String> = doc
        .objects
        .values()
        .filter_map(|o| match o {
            Object::Dictionary(d) if d.get(b"Subtype").ok().and_then(|s| s.as_name().ok()) == Some(b"Type0") => {
                let base = d.get(b"BaseFont").unwrap().as_name_str().unwrap();
                let (etiquette, nom) = base.split_once('+').expect("nom de sous-ensemble ABCDEF+Nom");
                assert!(etiquette.len() == 6 && etiquette.chars().all(|c| c.is_ascii_uppercase()), "{}", base);
                Some(nom.to_string())
            }
            _ => None,
        })
        .collect();
    noms.sort();
    noms
}

/// Le nom de ressource (F0, F1…) de la police dont le nom PostScript est `nom`, sur la page.
fn nom_de_ressource(pdf: &[u8], page: usize, nom: &str) -> Vec<u8> {
    let doc = Document::load_mem(pdf).unwrap();
    let (_, id) = doc.get_pages().into_iter().nth(page - 1).unwrap();
    let trouvee = polices_de_page(&doc, id)
        .into_iter()
        .find(|(_, d)| d.get(b"BaseFont").unwrap().as_name_str().unwrap().ends_with(&format!("+{}", nom)))
        .map(|(n, _)| n);
    trouvee.unwrap_or_else(|| panic!("police {} absente", nom))
}

/// Le texte d'une chaîne du PDF écrite avec une police en sous-ensemble : deux octets par
/// caractère, le point de code Unicode lui-même.
fn caracteres(octets: &[u8]) -> Vec<char> {
    assert_eq!(octets.len() % 2, 0, "codes de deux octets attendus : {:?}", octets);
    octets
        .chunks(2)
        .map(|c| char::from_u32(((c[0] as u32) << 8) | c[1] as u32).unwrap())
        .collect()
}

/// Toutes les lignes de texte du PDF (un bloc BT..ET = une ligne), page par page. Les polices sont
/// celles de Liberation, embarquées en sous-ensemble ; les largeurs viennent des fichiers de
/// `polices/` ; le texte justifié est écrit en `TJ` (un décalage après chaque espace).
fn lignes(pdf: &[u8]) -> Vec<Ligne> {
    let doc = Document::load_mem(pdf).unwrap();
    let mut out = Vec::new();
    for (n, (_, id)) in doc.get_pages().iter().enumerate() {
        // Nom de ressource (F0, F1…) -> fichier de police.
        let donnees: Vec<(Vec<u8>, Vec<u8>)> = polices_de_page(&doc, *id)
            .into_iter()
            .map(|(nom, d)| {
                let base = d.get(b"BaseFont").unwrap().as_name_str().unwrap().to_string();
                (nom, face(&fichier_de_police(&base)))
            })
            .collect();
        let faces: Vec<(&Vec<u8>, ttf_parser::Face<'_>)> = donnees
            .iter()
            .map(|(nom, d)| (nom, ttf_parser::Face::parse(d, 0).unwrap()))
            .collect();
        let contenu = doc.get_page_content(*id).unwrap();
        let ops = Content::decode(&contenu).unwrap().operations;
        let (mut x, mut y) = (0.0f32, 0.0f32);
        let mut police = Vec::new();
        let mut taille = 10.0f32;
        let mut largeur = 0.0f32;
        let mut mot = 0.0f32;
        let mut texte = String::new();
        for op in ops {
            match op.operator.as_str() {
                "BT" => {
                    largeur = 0.0;
                    texte.clear();
                    mot = 0.0;
                }
                "Td" => {
                    x = nombre(&op.operands[0]);
                    y = nombre(&op.operands[1]);
                }
                "Tf" => {
                    police = op.operands[0].as_name().unwrap().to_vec();
                    taille = nombre(&op.operands[1]);
                }
                "Tj" | "TJ" => {
                    let f = &faces.iter().find(|(n, _)| **n == police).unwrap().1;
                    let upem = f.units_per_em() as f32;
                    let morceaux: Vec<&Object> = match &op.operands[0] {
                        Object::Array(a) => a.iter().collect(),
                        o => vec![o],
                    };
                    for m in morceaux {
                        match m {
                            Object::String(b, _) => {
                                for c in caracteres(b) {
                                    texte.push(c);
                                    let adv = f
                                        .glyph_index(c)
                                        .and_then(|g| f.glyph_hor_advance(g))
                                        .unwrap_or(0) as f32;
                                    largeur += adv / upem * taille;
                                }
                            }
                            // Un décalage de TJ, en millièmes de la taille : positif = vers la
                            // gauche. Celui qui suit une espace étire l'espace (justification).
                            n => {
                                let d = -nombre(n) / 1000.0 * taille;
                                largeur += d;
                                if d != 0.0 {
                                    mot = d;
                                }
                            }
                        }
                    }
                }
                "ET" => {
                    if !texte.is_empty() {
                        out.push(Ligne {
                            page: n + 1,
                            x,
                            y,
                            largeur,
                            texte: texte.clone(),
                            mot_espace: mot,
                        });
                    }
                }
                _ => {}
            }
        }
    }
    out
}

fn nb_pages(pdf: &[u8]) -> usize {
    Document::load_mem(pdf).unwrap().get_pages().len()
}

/// Le nom des XObjects de la page (1 = première) dont le sous-type est `sous_type`.
fn xobjects(pdf: &[u8], page: usize, sous_type: &str) -> usize {
    let doc = Document::load_mem(pdf).unwrap();
    let (_, id) = doc.get_pages().into_iter().nth(page - 1).unwrap();
    let (res, ids) = doc.get_page_resources(id);
    let res = match res {
        Some(r) => r,
        None => match ids.first().and_then(|i| doc.get_dictionary(*i).ok()) {
            Some(r) => r,
            None => return 0,
        },
    };
    let xo = match res.get(b"XObject").ok().and_then(|o| o.as_dict().ok()) {
        Some(x) => x,
        None => return 0,
    };
    xo.iter()
        .filter(|(_, o)| {
            let obj = match o {
                Object::Reference(r) => doc.get_object(*r).unwrap(),
                o => o,
            };
            match obj {
                Object::Stream(s) => s
                    .dict
                    .get(b"Subtype")
                    .ok()
                    .and_then(|s| s.as_name().ok())
                    .map(|n| n == sous_type.as_bytes())
                    .unwrap_or(false),
                _ => false,
            }
        })
        .count()
}

fn texte_complet(pdf: &[u8]) -> String {
    lignes(pdf)
        .iter()
        .map(|l| l.texte.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

/// L'inspection de la fabrique (`--inspecter`, le repère de PyMuPDF : origine en haut à gauche,
/// y vers le bas) d'un PDF fabriqué : une seconde lecture, indépendante de celle de `lignes`.
fn inspecter(pdf: &[u8], nom: &str) -> Value {
    let p = ecrire(nom, pdf);
    let (json, avertissements) =
        fabrique_pdf::inspection::inspecter(&chemin(&p)).unwrap_or_else(|r| panic!("inspection : {}", r.0));
    assert!(avertissements.is_empty(), "{:?}", avertissements);
    serde_json::from_str(&json).unwrap()
}

/// Le texte de chaque page, extrait par `lecture::texte` de la fabrique (ce que faisait `get_text()`).
fn texte_des_pages(pdf: &[u8], nom: &str) -> Vec<String> {
    let p = ecrire(nom, pdf);
    let (pages, avertissements) =
        fabrique_pdf::lecture::texte(&chemin(&p)).unwrap_or_else(|r| panic!("lecture : {}", r.0));
    assert!(avertissements.is_empty(), "{:?}", avertissements);
    pages
}

fn nombres(v: &Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect()
}

fn ecrire(nom: &str, pdf: &[u8]) -> PathBuf {
    let p = dossier_temp("pdf").join(nom);
    std::fs::write(&p, pdf).unwrap();
    p
}

/// Lance le binaire : (code, sortie d'erreur).
fn binaire(entree: &str, sortie: &Path) -> (i32, String) {
    let mut enfant = Command::new(env!("CARGO_BIN_EXE_fabrique-pdf"))
        .arg(sortie)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    enfant
        .stdin
        .take()
        .unwrap()
        .write_all(entree.as_bytes())
        .unwrap();
    let r = enfant.wait_with_output().unwrap();
    (
        r.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&r.stderr).to_string(),
    )
}

// ---------------------------------------------------------------------------
// La ligne de commande.
// ---------------------------------------------------------------------------

#[test]
fn version_est_le_numero_du_format() {
    let r = Command::new(env!("CARGO_BIN_EXE_fabrique-pdf"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(r.status.success());
    assert_eq!(String::from_utf8_lossy(&r.stdout).trim(), "fabrique-pdf 3");
}

#[test]
fn l_aide_nomme_chaque_mode() {
    // C'est ce que le Python peut lire pour savoir ce que le programme sait faire.
    let r = Command::new(env!("CARGO_BIN_EXE_fabrique-pdf")).arg("--aide").output().unwrap();
    assert!(r.status.success());
    let aide = String::from_utf8_lossy(&r.stdout).to_string();
    for mode in ["--assembler", "--pages", "--texte", "--marquer", "--verrouiller", "--version"] {
        assert!(aide.contains(mode), "l'aide ne nomme pas {} : {}", mode, aide);
    }
}

#[test]
fn binaire_ecrit_le_pdf_ou_rien() {
    let dossier = dossier_temp("binaire");
    let sortie = dossier.join("ok.pdf");
    let _ = std::fs::remove_file(&sortie);
    let (code, err) = binaire(&base(vec![para("Bonjour")]).to_string(), &sortie);
    assert_eq!(code, 0, "{}", err);
    assert!(err.is_empty());
    assert!(std::fs::read(&sortie).unwrap().starts_with(b"%PDF-"));

    let refuse = dossier.join("refus.pdf");
    let _ = std::fs::remove_file(&refuse);
    let (code, err) = binaire(
        &base(vec![json!({"type": "tablo"})]).to_string(),
        &refuse,
    );
    assert_ne!(code, 0);
    assert!(!refuse.exists(), "rien ne doit être écrit sur un refus");
    assert_eq!(err.trim().lines().count(), 1, "une seule ligne : {:?}", err);
    assert!(err.contains("bloc 1 : type inconnu \"tablo\""), "{}", err);
}

#[test]
fn binaire_sans_argument_dit_l_usage() {
    let r = Command::new(env!("CARGO_BIN_EXE_fabrique-pdf"))
        .output()
        .unwrap();
    assert!(!r.status.success());
    let err = String::from_utf8_lossy(&r.stderr);
    assert!(err.contains("usage"), "{}", err);
    assert_eq!(err.trim().lines().count(), 1);
}

// ---------------------------------------------------------------------------
// Chaque type de bloc.
// ---------------------------------------------------------------------------

#[test]
fn paragraphe_accents_euro_et_ponctuation() {
    let phrase = "é à ç ô € œ « » — ’ Œ É ÿ ü 1 249,35 € & <b> \"guillemets\"";
    let pdf = ok(&base(vec![para(phrase)]));
    assert_eq!(nb_pages(&pdf), 1);
    // Ce qui est écrit dans le flux : les codes de la police en sous-ensemble, relus.
    assert_eq!(texte_complet(&pdf), phrase);

    // Ce qu'en extrait la lecture de la fabrique.
    let sortie = texte_des_pages(&pdf, "accents.pdf");
    assert_eq!(sortie[0].trim(), phrase);
}

#[test]
fn paragraphe_morceaux_gras_et_retour_force() {
    let v = base(vec![json!({"type": "paragraphe", "morceaux": [
        {"texte": "Entre la société "}, {"texte": "FIL TENDU", "gras": true}, {"texte": ", ci-après"},
        {"texte": " la première ligne\nla seconde"}]})]);
    let pdf = ok(&v);
    let l = lignes(&pdf);
    assert_eq!(l.len(), 2, "{:?}", l);
    assert_eq!(l[0].texte, "Entre la société FIL TENDU, ci-après la première ligne");
    assert_eq!(l[1].texte, "la seconde");
    // Les deux lignes sont l'une sous l'autre, à l'interligne de 1,25 x 10 pt.
    assert!((l[0].y - l[1].y - 12.5).abs() < 0.01, "{:?}", l);
    // Le gras est Liberation Sans Bold, embarquée en sous-ensemble : romain et gras, rien d'autre.
    let noms = noms_de_polices(&pdf);
    assert_eq!(noms, ["LiberationSans", "LiberationSans-Bold"], "{:?}", noms);
}

#[test]
fn paragraphe_majuscules_taille_couleur_et_espaces() {
    let v = base(vec![
        json!({"type": "paragraphe", "morceaux": [{"texte": "Titre éàç œuvre"}],
               "majuscules": true, "taille": 20.0, "couleur": "#FF0000",
               "espace_avant": 7.0, "espace_apres": 13.0}),
        para("Suite"),
    ]);
    let pdf = ok(&v);
    let l = lignes(&pdf);
    assert_eq!(l[0].texte, "TITRE ÉÀÇ ŒUVRE");
    assert_eq!(l[1].texte, "Suite");
    // Haut de page 450 ; espace avant 7 ; ligne de 25 pt (20 x 1,25) ; la ligne de base du
    // titre est au milieu de l'interligne (demi-interligne) puis l'ascendante.
    assert!(l[0].y < 450.0 - 7.0, "{:?}", l);
    // Entre les deux lignes de base : le reste du titre + 13 pt d'espace + le début de la suite.
    assert!(l[0].y - l[1].y > 13.0 + 10.0, "{:?}", l);
    let brut = String::from_utf8_lossy(&pdf).to_string();
    let contenu = Document::load_mem(&pdf).unwrap();
    let (_, id) = contenu.get_pages().into_iter().next().unwrap();
    let flux = String::from_utf8_lossy(&contenu.get_page_content(id).unwrap()).to_string();
    assert!(flux.contains("1 0 0 rg"), "couleur #FF0000 : {}", flux);
    assert!(flux.contains(" 20 Tf"), "taille 20 : {}", flux);
    let _ = brut;
}

#[test]
fn bloc_espace_ecarte_les_paragraphes() {
    let sans = ok(&base(vec![para("A"), para("B")]));
    let avec = ok(&base(vec![
        para("A"),
        json!({"type": "espace", "hauteur": 6.0}),
        para("B"),
    ]));
    let (a, b) = (lignes(&sans), lignes(&avec));
    let ecart_sans = a[0].y - a[1].y;
    let ecart_avec = b[0].y - b[1].y;
    assert!((ecart_avec - ecart_sans - 6.0).abs() < 0.01, "{} {}", ecart_sans, ecart_avec);
}

#[test]
fn bloc_colonnes_filet_et_positions() {
    let v = base(vec![json!({
        "type": "colonnes",
        "filet_haut": {"couleur": "#CCCCCC", "epaisseur": 0.5},
        "espace_avant": 4.0, "marge_interne_haut": 5.0,
        "colonnes": [
            {"largeur": 0.25, "blocs": [para("Gauche")]},
            {"largeur": 0.75, "blocs": [para("Droite"), para("Suite de droite")]}
        ]})]);
    let pdf = ok(&v);
    let l = lignes(&pdf);
    assert_eq!(l.len(), 3, "{:?}", l);
    // Largeur utile 300 : la première colonne commence à 50, la seconde à 50 + 75.
    assert!((l[0].x - 50.0).abs() < 0.01, "{:?}", l);
    assert!((l[1].x - 125.0).abs() < 0.01, "{:?}", l);
    // Le filet : un trait par colonne, gris clair.
    let doc = Document::load_mem(&pdf).unwrap();
    let (_, id) = doc.get_pages().into_iter().next().unwrap();
    let ops = Content::decode(&doc.get_page_content(id).unwrap())
        .unwrap()
        .operations;
    let traits = ops.iter().filter(|o| o.operator == "S").count();
    assert_eq!(traits, 2, "un filet au-dessus de chaque colonne");
    let gris = ops
        .iter()
        .find(|o| o.operator == "RG")
        .map(|o| o.operands.iter().map(nombre).collect::<Vec<_>>())
        .unwrap();
    assert!(gris.iter().all(|c| (c - 0.8).abs() < 0.01), "{:?}", gris);
}

#[test]
fn colonnes_ne_se_coupent_pas() {
    // Douze paragraphes remplissent presque la page ; les colonnes (quatre lignes) ne tiennent
    // pas dans ce qui reste et passent entières à la page suivante.
    let mut blocs: Vec<Value> = (1..=31).map(|i| para(&format!("Ligne {}", i))).collect();
    blocs.push(json!({"type": "colonnes", "colonnes": [
        {"largeur": 0.5, "blocs": [para("GA1"), para("GA2"), para("GA3"), para("GA4")]},
        {"largeur": 0.5, "blocs": [para("DR1"), para("DR2")]}]}));
    let pdf = ok(&base(blocs));
    assert_eq!(nb_pages(&pdf), 2);
    let l = lignes(&pdf);
    let page = |t: &str| l.iter().find(|x| x.texte == t).unwrap().page;
    assert_eq!(page("Ligne 31"), 1);
    for t in ["GA1", "GA2", "GA3", "GA4", "DR1", "DR2"] {
        assert_eq!(page(t), 2, "{} doit être à la page 2", t);
    }
    // Un paragraphe ordinaire, lui, se coupe : 60 lignes font deux pages avec la coupure au
    // milieu.
    let long = (1..=60).map(|i| format!("mot{}", i)).collect::<Vec<_>>().join("\n");
    let pdf = ok(&base(vec![para(&long)]));
    assert_eq!(nb_pages(&pdf), 2);
}

#[test]
fn colonnes_trop_hautes_pour_une_page_sont_refusees() {
    let haut: Vec<Value> = (1..=50).map(|i| para(&format!("L{}", i))).collect();
    let e = refus(&base(vec![json!({"type": "colonnes", "colonnes": [
        {"largeur": 1.0, "blocs": haut}]})]));
    assert!(e.starts_with("bloc 1 : "), "{}", e);
    assert!(e.contains("colonnes"), "{}", e);
}

#[test]
fn bloc_image_png_avec_transparence() {
    let pdf = ok(&base(vec![
        para("avant"),
        json!({"type": "image", "chemin": image("signature.png"), "largeur": 92.0}),
    ]));
    assert_eq!(xobjects(&pdf, 1, "Image"), 1);
    // Le masque de transparence est un objet à part, référencé par l'image.
    let doc = Document::load_mem(&pdf).unwrap();
    let mut vu = false;
    for (_, o) in doc.objects.iter() {
        if let Object::Stream(s) = o {
            if s.dict.get(b"Subtype").ok().and_then(|x| x.as_name().ok()) == Some(b"Image") {
                if let Ok(Object::Reference(r)) = s.dict.get(b"SMask") {
                    let m = doc.get_object(*r).unwrap().as_stream().unwrap();
                    assert_eq!(m.dict.get(b"Height").unwrap().as_i64().unwrap(), 120);
                    assert_eq!(m.dict.get(b"Width").unwrap().as_i64().unwrap(), 368);
                    vu = true;
                }
            }
        }
    }
    assert!(vu, "masque alpha absent");
    // La hauteur suit les proportions : 92 x 120/368 = 30 pt. L'image est juste sous le texte.
    let ins = inspecter(&pdf, "png.pdf");
    let r = nombres(&ins["pages"][0]["images"][0]);
    let (largeur, hauteur) = (r[2] - r[0], r[3] - r[1]);
    assert!((largeur - 92.0).abs() < 0.1 && (hauteur - 30.0).abs() < 0.1, "{:?}", r);
}

#[test]
fn bloc_image_jpeg_recopie_tel_quel() {
    let jpeg = std::fs::read(exemples().join("scan.jpg")).unwrap();
    let pdf = ok(&base(vec![
        json!({"type": "image", "chemin": image("scan.jpg"), "largeur": 150.0}),
    ]));
    assert_eq!(xobjects(&pdf, 1, "Image"), 1);
    // Les octets du JPEG sont dans le PDF, sans décompression.
    assert!(
        pdf.windows(jpeg.len().min(4000)).any(|w| w == &jpeg[..jpeg.len().min(4000)]),
        "le JPEG doit être recopié tel quel (DCTDecode)"
    );
    assert!(String::from_utf8_lossy(&pdf).contains("DCTDecode"));
}

#[test]
fn bloc_image_svg_est_vectoriel() {
    let pdf = ok(&base(vec![
        json!({"type": "image", "chemin": image("logo.svg"), "largeur": 120.0}),
    ]));
    assert_eq!(xobjects(&pdf, 1, "Form"), 1, "un SVG devient un objet de formulaire vectoriel");
    assert_eq!(xobjects(&pdf, 1, "Image"), 0, "pas de pixels");
}

#[test]
fn images_reconnues_par_le_contenu_pas_par_l_extension() {
    let dossier = dossier_temp("extensions");
    let png = std::fs::read(exemples().join("signature.png")).unwrap();
    let jpg = std::fs::read(exemples().join("scan.jpg")).unwrap();
    let svg = std::fs::read(exemples().join("logo.svg")).unwrap();
    let cas: Vec<(&str, &Vec<u8>, &str)> = vec![
        ("a.jpg", &jpg, "Image"),
        ("b.JPG", &jpg, "Image"),
        ("c.jpeg", &jpg, "Image"),
        ("d.JPEG", &jpg, "Image"),
        ("e.png", &png, "Image"),
        ("f.PNG", &png, "Image"),
        // Un PNG qui s'appelle .jpg est lu comme un PNG, et un JPEG nommé .png comme un JPEG.
        ("g.jpg", &png, "Image"),
        ("h.png", &jpg, "Image"),
        ("i.svg", &svg, "Form"),
        ("j.SVG", &svg, "Form"),
        ("k.image", &jpg, "Image"),
    ];
    for (nom, donnees, genre) in cas {
        let p = dossier.join(nom);
        std::fs::write(&p, donnees).unwrap();
        let pdf = ok(&base(vec![
            json!({"type": "image", "chemin": chemin(&p), "largeur": 100.0}),
        ]));
        assert_eq!(xobjects(&pdf, 1, genre), 1, "{}", nom);
        // Le même fichier en logo.
        let mut v = base(vec![para("x")]);
        v["logo"] = json!({"chemin": chemin(&p), "largeur": 50.0, "marge_haut": 5.0, "marge_droite": 5.0});
        let pdf = ok(&v);
        assert_eq!(xobjects(&pdf, 1, genre), 1, "logo {}", nom);
    }
}

// ---------------------------------------------------------------------------
// Le bloc `tableau`.
// ---------------------------------------------------------------------------

/// Les opérateurs du flux de contenu d'une page (1 = première).
fn operateurs(pdf: &[u8], page: usize) -> Vec<lopdf::content::Operation> {
    let doc = Document::load_mem(pdf).unwrap();
    let (_, id) = doc.get_pages().into_iter().nth(page - 1).unwrap();
    Content::decode(&doc.get_page_content(id).unwrap()).unwrap().operations
}

/// Un tableau de trois colonnes (libellé à gauche, deux nombres à droite), cellules de 4 pt de
/// marge à gauche et à droite, sur la largeur utile de 300 pt de `base`.
fn tableau3(entete: Vec<Value>, lignes: Vec<Value>) -> Value {
    json!({"type": "tableau",
           "colonnes": [{"largeur": 0.5},
                        {"largeur": 0.25, "alignement": "droite"},
                        {"largeur": 0.25, "alignement": "droite"}],
           "marge_cellule": {"haut": 1.0, "droite": 4.0, "bas": 1.0, "gauche": 4.0},
           "entete": entete, "lignes": lignes})
}

fn ligne3(a: &str, b: &str, c: &str) -> Value {
    json!({"cellules": [a, b, c]})
}

#[test]
fn tableau_colonnes_alignees_nombres_a_droite() {
    let v = base(vec![tableau3(
        vec![ligne3("Libellé", "Base", "Montant")],
        vec![ligne3("Salaire", "1 234,56", "12,00"), ligne3("Total", "9,00", "1 000 000,00")],
    )]);
    let l = lignes(&ok(&v));
    let trouve = |t: &str| l.iter().find(|x| x.texte == t).unwrap_or_else(|| panic!("{} dans {:?}", t, l)).clone();
    // Colonne 1 : de 50 à 200, texte collé à gauche (marge de 4).
    assert!((trouve("Salaire").x - 54.0).abs() < 0.01, "{:?}", l);
    // Colonnes 2 et 3 : de 200 à 275 et de 275 à 350 ; le bord droit du texte est à la marge près.
    for (t, bord) in [("1 234,56", 271.0), ("Base", 271.0), ("9,00", 271.0), ("12,00", 346.0), ("1 000 000,00", 346.0)] {
        let c = trouve(t);
        assert!((c.x + c.largeur - bord).abs() < 0.05, "{} finit à {} au lieu de {}", t, c.x + c.largeur, bord);
    }
    // L'ordre de lecture : ligne après ligne, de gauche à droite.
    let ordre: Vec<&str> = l.iter().map(|x| x.texte.as_str()).collect();
    assert_eq!(ordre, ["Libellé", "Base", "Montant", "Salaire", "1 234,56", "12,00", "Total", "9,00", "1 000 000,00"]);
}

#[test]
fn tableau_cellule_longue_passe_a_la_ligne_et_la_ligne_grandit() {
    let long = "un libellé bien trop long pour tenir sur une seule ligne dans une colonne de cent cinquante points";
    let v = base(vec![tableau3(vec![], vec![ligne3(long, "1", "2"), ligne3("suite", "3", "4")])]);
    let l = lignes(&ok(&v));
    let morceaux: Vec<&Ligne> = l.iter().filter(|x| long.contains(x.texte.as_str()) && x.texte.len() > 5).collect();
    assert!(morceaux.len() >= 2, "le libellé doit se replier : {:?}", l);
    // Les nombres restent sur la première ligne, la ligne suivante est sous la dernière du libellé.
    let un = l.iter().find(|x| x.texte == "1").unwrap();
    assert!((un.y - morceaux[0].y).abs() < 0.01, "{:?}", l);
    let suite = l.iter().find(|x| x.texte == "suite").unwrap();
    let plus_bas = morceaux.iter().map(|m| m.y).fold(f32::MAX, f32::min);
    assert!(suite.y < plus_bas, "la ligne suivante passe sous le libellé replié : {:?}", l);
}

#[test]
fn tableau_etendue_gras_fond_filets_et_couleurs() {
    let v = base(vec![json!({
        "type": "tableau",
        "colonnes": [{"largeur": 0.5}, {"largeur": 0.5, "alignement": "droite"}],
        "filet_lignes": {"couleur": "#E4E4E4", "epaisseur": 0.3},
        "entete": [{"cellules": ["A", "B"], "fond": "#B22222", "couleur": "#FFFFFF", "gras": true}],
        "lignes": [
            {"cellules": [{"texte": "Total", "etendue": 2}], "filet_haut": {"couleur": "#B22222", "epaisseur": 0.6}},
            {"cellules": [{"morceaux": [{"texte": "gris", "couleur": "#777777"}, {"texte": " gras", "gras": true}]}, "9"],
             "fond": "#F4F0F0"}
        ]})]);
    let pdf = ok(&v);
    let ops = operateurs(&pdf, 1);
    // Deux fonds pleins, une couleur de remplissage rouge puis une grise claire.
    assert_eq!(ops.iter().filter(|o| matches!(o.operator.as_str(), "f" | "f*")).count(), 2);
    let remplissages: Vec<Vec<f32>> = ops
        .iter()
        .filter(|o| o.operator == "rg")
        .map(|o| o.operands.iter().map(nombre).collect())
        .collect();
    assert!(remplissages.iter().any(|c| (c[0] - 0xB2 as f32 / 255.0).abs() < 0.01 && c[1] < 0.2), "{:?}", remplissages);
    assert!(remplissages.iter().any(|c| c.iter().all(|x| (x - 0x77 as f32 / 255.0).abs() < 0.01)), "{:?}", remplissages);
    // Filets : un par ligne du corps (2) plus le filet haut explicite de « Total » (3 traits).
    assert_eq!(ops.iter().filter(|o| o.operator == "S").count(), 3);
    // Le texte du bloc, dans l'ordre ; l'en-tête et « gras » sont en Liberation Sans Bold.
    let t = texte_complet(&pdf);
    assert_eq!(t, "A\nB\nTotal\ngris gras\n9");
    let gras = nom_de_ressource(&pdf, 1, "LiberationSans-Bold");
    let polices: Vec<Vec<u8>> = ops
        .iter()
        .filter(|o| o.operator == "Tf")
        .map(|o| o.operands[0].as_name().unwrap().to_vec())
        .collect();
    assert!(polices.iter().filter(|p| **p == gras).count() >= 3, "{:?}", polices);
    // « Total » s'étend sur les deux colonnes : un texte plus long que la colonne tient sur une ligne.
    let v = base(vec![json!({
        "type": "tableau", "colonnes": [{"largeur": 0.2}, {"largeur": 0.8}],
        "lignes": [{"cellules": [{"texte": "ce libellé ne tient que sur la largeur des deux colonnes réunies", "etendue": 2}]}]})]);
    assert_eq!(lignes(&ok(&v)).len(), 1);
}

#[test]
fn tableau_entete_repetee_et_ligne_jamais_coupee() {
    // 60 lignes de deux lignes de texte chacune : plusieurs pages (400 x 500, marges de 50).
    let corps: Vec<Value> = (1..=60).map(|k| ligne3(&format!("L{}a\nL{}b", k, k), "1", "2")).collect();
    let v = base(vec![
        para("Avant"),
        tableau3(vec![ligne3("EN", "TETE", "ICI")], corps),
        para("Après"),
    ]);
    let pdf = ok(&v);
    let n = nb_pages(&pdf);
    assert!(n >= 3, "{} pages", n);
    let l = lignes(&pdf);
    for page in 1..=n {
        let a = l.iter().filter(|x| x.page == page && x.texte == "EN").count();
        assert_eq!(a, 1, "l'en-tête ouvre la page {}", page);
    }
    // Une ligne ne se coupe pas : ses deux lignes de texte sont sur la même page.
    for k in 1..=60 {
        let pa = l.iter().find(|x| x.texte == format!("L{}a", k)).unwrap().page;
        let pb = l.iter().find(|x| x.texte == format!("L{}b", k)).unwrap().page;
        assert_eq!(pa, pb, "la ligne {} est coupée", k);
    }
    // Le paragraphe suivant vient après la dernière ligne ; le texte garde l'ordre.
    let derniere = l.iter().position(|x| x.texte == "L60b").unwrap();
    assert_eq!(l[derniere + 3].texte, "Après", "{:?}", &l[derniere..]);
    // Même description, mêmes octets.
    assert_eq!(pdf, ok(&v));
}

#[test]
fn tableau_sans_ligne_n_ecrit_que_l_entete_et_se_marge() {
    let v = base(vec![
        para("Avant"),
        json!({"type": "tableau", "colonnes": [{"largeur": 1.0}],
               "entete": [{"cellules": ["Seul"]}], "espace_avant": 20.0, "espace_apres": 20.0}),
        para("Après"),
    ]);
    let l = lignes(&ok(&v));
    assert_eq!(l.iter().map(|x| x.texte.as_str()).collect::<Vec<_>>(), ["Avant", "Seul", "Après"]);
    // 20 pt avant et après (les marges voisines se fondent avec celles des paragraphes, nulles ici).
    assert!(l[0].y - l[1].y > 20.0 && l[1].y - l[2].y > 20.0, "{:?}", l);
}

#[test]
fn tableau_ne_laisse_pas_son_entete_seul_en_bas_de_page() {
    // Presque toute la page est prise : l'en-tête et la première ligne ne tiennent pas ensemble,
    // le tableau entier passe à la page suivante (pas d'en-tête orphelin).
    let mut blocs: Vec<Value> = (1..=31).map(|i| para(&format!("Ligne {}", i))).collect();
    blocs.push(tableau3(vec![ligne3("EN", "TETE", "ICI")], vec![ligne3("a", "b", "c")]));
    let pdf = ok(&base(blocs));
    let l = lignes(&pdf);
    let page = |t: &str| l.iter().find(|x| x.texte == t).unwrap().page;
    assert_eq!(page("Ligne 31"), 1);
    assert_eq!(page("EN"), page("a"));
    assert_eq!(l.iter().filter(|x| x.texte == "EN").count(), 1);
}

#[test]
fn tableau_chaque_document_recommence_sur_une_page_neuve() {
    // Chaque document recommence sur une page neuve, tableau compris.
    let mut v = base(vec![tableau3(vec![ligne3("EN", "TETE", "ICI")], vec![ligne3("a", "b", "c")])]);
    let second = v["documents"][0].clone();
    v["documents"].as_array_mut().unwrap().push(second);
    let pdf = ok(&v);
    assert_eq!(nb_pages(&pdf), 2);
    assert_eq!(lignes(&pdf).iter().filter(|x| x.texte == "EN").count(), 2);
}

#[test]
fn paragraphe_couleur_par_morceau() {
    let v = base(vec![json!({"type": "paragraphe", "couleur": "#000000",
        "morceaux": [{"texte": "noir "}, {"texte": "rouge", "couleur": "#FF0000"}]})]);
    let ops = operateurs(&ok(&v), 1);
    let rg: Vec<Vec<f32>> = ops.iter().filter(|o| o.operator == "rg").map(|o| o.operands.iter().map(nombre).collect()).collect();
    assert!(rg.iter().any(|c| c[0] > 0.99 && c[1] < 0.01 && c[2] < 0.01), "{:?}", rg);
}

#[test]
fn refus_tableau() {
    let col1 = json!([{"largeur": 1.0}]);
    let cas: Vec<(Value, &str)> = vec![
        (json!({"type": "tableau", "colonnes": []}), "un tableau sans colonne"),
        (json!({"type": "tableau", "colonnes": [{"largeur": 0.5}]}), "la somme des largeurs"),
        (json!({"type": "tableau", "colonnes": col1, "lignes": [{"cellules": ["a", "b"]}]}),
         "ligne 1, cellule 2 : la ligne occupe plus que les 1 colonne(s)"),
        (json!({"type": "tableau", "colonnes": col1, "entete": [{"cellules": [{"texte": "a", "etendue": 0}]}]}),
         "en-tête, ligne 1, cellule 1 : etendue doit valoir au moins 1"),
        (json!({"type": "tableau", "colonnes": col1, "lignes": [{"cellules": [{"texte": "a", "morceaux": []}]}]}),
         "\"texte\" et \"morceaux\" s'excluent"),
        (json!({"type": "tableau", "colonnes": col1, "lignes": [{"cellules": [3]}]}),
         "une cellule est une chaîne ou un objet"),
        (json!({"type": "tableau", "colonnes": col1, "lignes": [{"cellules": ["a"], "fond": "rouge"}]}),
         "couleur \"rouge\" invalide"),
        (json!({"type": "tableau", "colonnes": col1, "lignes": [{"cellules": ["a"], "bordure": 1}]}),
         "champ inconnu \"bordure\""),
        (json!({"type": "tableau", "colonnes": col1, "lignes": [{"cellules": [{"texte": "a", "fond": "#000000"}]}]}),
         "champ inconnu \"fond\""),
        (json!({"type": "tableau", "colonnes": [{"largeur": 1.0, "couleur": "#000000"}]}), "champ inconnu \"couleur\""),
        (json!({"type": "tableau", "colonnes": col1, "marge_cellule": {"haut": -1.0}}), "haut ne peut pas être négatif"),
        (json!({"type": "tableau", "colonnes": col1, "marge_cellule": {"gauche": 200.0, "droite": 200.0}, "lignes": [{"cellules": ["a"]}]}),
         "plus étroite que les marges"),
        (json!({"type": "tableau", "colonnes": col1, "filet_lignes": {"couleur": "#000000", "epaisseur": 0.0}}),
         "epaisseur doit être positif"),
        (json!({"type": "tableau", "colonnes": col1, "lignes": [{"cellules": ["漢"]}]}), "n'existe pas dans la police"),
        (json!({"type": "colonnes", "colonnes": [{"largeur": 1.0, "blocs": [
            {"type": "tableau", "colonnes": col1}]}]}), "un tableau ne se place pas dans des colonnes"),
    ];
    for (bloc, attendu) in cas {
        let e = refus(&base(vec![para("x"), bloc.clone()]));
        assert!(e.starts_with("bloc 2"), "{} / {}", bloc, e);
        assert!(e.contains(attendu), "{} / attendu « {} » / {}", bloc, attendu, e);
    }
    // Une ligne plus haute que la page ne se coupe pas : refus, avec le numéro du bloc.
    let haute = (1..=60).map(|i| format!("m{}", i)).collect::<Vec<_>>().join("\n");
    let e = refus(&base(vec![json!({"type": "tableau", "colonnes": col1, "lignes": [{"cellules": [haute]}]})]));
    assert!(e.starts_with("bloc 1 : l'en-tête et la plus haute ligne du tableau occupent"), "{}", e);
}

// ---------------------------------------------------------------------------
// Les refus.
// ---------------------------------------------------------------------------

#[test]
fn refus_champ_inconnu_partout() {
    // À la racine.
    let mut v = base(vec![para("x")]);
    v["couleur_fond"] = json!("#FFFFFF");
    let e = refus(&v);
    assert!(e.contains("champ inconnu \"couleur_fond\""), "{}", e);
    // Dans la page, les marges, la police.
    for (chemin_json, champ) in [
        ("/page", "epaisseur"),
        ("/page/marges", "haute"),
        ("/police", "gras"),
    ] {
        let mut v = base(vec![para("x")]);
        v.pointer_mut(chemin_json).unwrap()[champ] = json!(1);
        let e = refus(&v);
        assert!(e.contains(&format!("champ inconnu \"{}\"", champ)), "{}", e);
    }
    // Dans chaque type de bloc, et dans un morceau.
    for b in [
        json!({"type": "paragraphe", "morceaux": [], "italique": true}),
        json!({"type": "paragraphe", "morceaux": [{"texte": "a", "souligne": true}]}),
        json!({"type": "espace", "hauteur": 1.0, "couleur": "#000000"}),
        json!({"type": "image", "chemin": image("logo.svg"), "largeur": 10.0, "hauteur": 10.0}),
        json!({"type": "colonnes", "colonnes": [{"largeur": 1.0, "blocs": [], "titre": "x"}]}),
        json!({"type": "colonnes", "bordure": 1, "colonnes": [{"largeur": 1.0, "blocs": []}]}),
        json!({"type": "colonnes", "filet_haut": {"couleur": "#000000", "style": "x"},
               "colonnes": [{"largeur": 1.0, "blocs": []}]}),
    ] {
        let e = refus(&base(vec![b.clone()]));
        assert!(e.starts_with("bloc 1"), "{} / {}", b, e);
        assert!(e.contains("champ inconnu"), "{} / {}", b, e);
    }
    let mut v = base(vec![para("x")]);
    v["logo"] = json!({"chemin": image("logo.svg"), "largeur": 10.0, "coin": "haut"});
    assert!(refus(&v).contains("champ inconnu \"coin\""));
    let mut v = base(vec![para("x")]);
    v["documents"][0]["titre"] = json!("x");
    assert!(refus(&v).contains("champ inconnu \"titre\""));
}

#[test]
fn refus_type_de_bloc_avec_son_numero() {
    let mut blocs: Vec<Value> = (0..11).map(|i| para(&format!("p{}", i))).collect();
    blocs.push(json!({"type": "tablo", "lignes": []}));
    let e = refus(&base(blocs));
    assert!(e.starts_with("bloc 12 : type inconnu \"tablo\""), "{}", e);
    let e = refus(&base(vec![json!({"morceaux": []})]));
    assert!(e.starts_with("bloc 1 : champ absent \"type\""), "{}", e);
    // Avec plusieurs documents, le numéro du document précède.
    let v = json!({"format": 1, "documents": [
        {"blocs": [para("a")]}, {"blocs": [para("b"), {"type": "zzz"}]}]});
    let e = refus(&v);
    assert!(e.starts_with("document 2, bloc 2 : type inconnu \"zzz\""), "{}", e);
    // Dans une colonne.
    let e = refus(&base(vec![json!({"type": "colonnes", "colonnes": [
        {"largeur": 0.5, "blocs": []},
        {"largeur": 0.5, "blocs": [para("a"), {"type": "zzz"}]}]})]));
    assert!(e.starts_with("bloc 1, colonne 2, bloc 2 : type inconnu \"zzz\""), "{}", e);
}

#[test]
fn refus_format_absent_ou_inconnu() {
    // Les formats 1, 2 et 3 se lisent ; le 4 n'existe pas.
    let mut v = base(vec![para("x")]);
    v["format"] = json!(4);
    let e = refus(&v);
    assert!(e.contains("format 4") && e.contains("formats 1, 2 et 3"), "{}", e);
    ok(&base(vec![para("x")]));
    let mut v = base(vec![para("x")]);
    v["format"] = json!(2);
    ok(&v);
    v["format"] = json!(3);
    ok(&v);
    v["format"] = json!("1");
    assert!(refus(&v).contains("format"));
    let mut v = base(vec![para("x")]);
    v.as_object_mut().unwrap().remove("format");
    assert!(refus(&v).contains("champ absent \"format\""));
    // Un format futur n'est pas refusé pour ses champs nouveaux mais pour son numéro.
    let e = refus(&json!({"format": 4, "champ_du_futur": true, "documents": []}));
    assert!(e.contains("format 4"), "{}", e);
}

#[test]
fn refus_image_absente_illisible_ou_inconnue() {
    let e = refus(&base(vec![json!({"type": "image", "chemin": "/nulle/part/absente.png", "largeur": 50.0})]));
    assert!(e.starts_with("bloc 1 : image introuvable : /nulle/part/absente.png"), "{}", e);
    let mut v = base(vec![para("x")]);
    v["logo"] = json!({"chemin": "/nulle/part/logo.svg", "largeur": 50.0});
    let e = refus(&v);
    assert!(e.contains("logo") && e.contains("introuvable"), "{}", e);

    let dossier = dossier_temp("illisibles");
    let faux = dossier.join("faux.png");
    std::fs::write(&faux, b"ceci n'est pas une image").unwrap();
    let e = refus(&base(vec![json!({"type": "image", "chemin": chemin(&faux), "largeur": 50.0})]));
    assert!(e.contains("format d'image non reconnu"), "{}", e);

    // Un PNG tronqué : la signature est bonne, le reste est illisible.
    let png = std::fs::read(exemples().join("signature.png")).unwrap();
    let tronque = dossier.join("tronque.png");
    std::fs::write(&tronque, &png[..png.len() / 2]).unwrap();
    let e = refus(&base(vec![json!({"type": "image", "chemin": chemin(&tronque), "largeur": 50.0})]));
    assert!(e.contains("PNG illisible"), "{}", e);

    // Un JPEG tronqué.
    let jpg = std::fs::read(exemples().join("scan.jpg")).unwrap();
    let tronque = dossier.join("tronque.jpg");
    std::fs::write(&tronque, &jpg[..jpg.len() / 3]).unwrap();
    let e = refus(&base(vec![json!({"type": "image", "chemin": chemin(&tronque), "largeur": 50.0})]));
    assert!(e.contains("JPEG illisible"), "{}", e);

    // Un SVG mal formé, un SVG avec texte, un SVG avec dégradé.
    let svg = dossier.join("casse.svg");
    std::fs::write(&svg, "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\"><rect").unwrap();
    let e = refus(&base(vec![json!({"type": "image", "chemin": chemin(&svg), "largeur": 50.0})]));
    assert!(e.contains("SVG illisible"), "{}", e);
    let svg = dossier.join("texte.svg");
    std::fs::write(&svg, "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\"><text x=\"1\" y=\"5\">A</text></svg>").unwrap();
    let e = refus(&base(vec![json!({"type": "image", "chemin": chemin(&svg), "largeur": 50.0})]));
    assert!(e.contains("texte"), "{}", e);
    let svg = dossier.join("degrade.svg");
    std::fs::write(&svg, "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\"><defs><linearGradient id=\"g\"><stop offset=\"0\" stop-color=\"red\"/><stop offset=\"1\" stop-color=\"blue\"/></linearGradient></defs><rect width=\"10\" height=\"10\" fill=\"url(#g)\"/></svg>").unwrap();
    let e = refus(&base(vec![json!({"type": "image", "chemin": chemin(&svg), "largeur": 50.0})]));
    assert!(e.contains("dégradé"), "{}", e);
}

#[test]
fn refus_valeurs_invalides() {
    let e = refus(&base(vec![json!({"type": "paragraphe", "morceaux": [{"texte": "a"}], "couleur": "rouge"})]));
    assert!(e.contains("couleur") && e.contains("#RRGGBB"), "{}", e);
    let e = refus(&base(vec![json!({"type": "paragraphe", "morceaux": [{"texte": "a"}], "alignement": "milieu"})]));
    assert!(e.contains("valeur inconnue \"milieu\"") && e.contains("justifie"), "{}", e);
    let e = refus(&base(vec![json!({"type": "paragraphe", "morceaux": [{"texte": "a"}], "taille": -3.0})]));
    assert!(e.contains("taille"), "{}", e);
    let e = refus(&base(vec![json!({"type": "espace", "hauteur": -1.0})]));
    assert!(e.contains("hauteur"), "{}", e);
    let e = refus(&base(vec![json!({"type": "espace"})]));
    assert!(e.contains("champ absent \"hauteur\""), "{}", e);
    let e = refus(&base(vec![json!({"type": "paragraphe"})]));
    assert!(e.contains("champ absent \"morceaux\""), "{}", e);
    let e = refus(&base(vec![json!({"type": "paragraphe", "morceaux": "texte"})]));
    assert!(e.starts_with("bloc 1 : "), "{}", e);
    let e = refus(&base(vec![json!({"type": "colonnes", "colonnes": [
        {"largeur": 0.5, "blocs": []}, {"largeur": 0.4, "blocs": []}]})]));
    assert!(e.contains("somme des largeurs"), "{}", e);
    let e = refus(&json!({"format": 1, "documents": []}));
    assert!(e.contains("documents"), "{}", e);
    let e = refus(&json!({"format": 1}));
    assert!(e.contains("champ absent \"documents\""), "{}", e);
    let e = refus(&json!({"format": 1, "page": {"marges": {"haut": 500.0, "bas": 500.0}}, "documents": [{"blocs": []}]}));
    assert!(e.contains("marges"), "{}", e);
    let e = fabrique_pdf::fabriquer("{\"format\": 1,").unwrap_err().0;
    assert!(e.contains("illisible") && !e.contains('\n'), "{}", e);
}

#[test]
fn refus_caractere_absent_de_la_police() {
    let e = refus(&base(vec![para("un idéogramme : 漢")]));
    assert!(e.starts_with("bloc 1 : le caractère « 漢 » (U+6F22) n'existe pas dans la police Liberation"), "{}", e);
    // Un emoji, hors du plan de base : même refus.
    let e = refus(&base(vec![para("sourire 😀")]));
    assert!(e.contains("U+1F600"), "{}", e);
    let e = refus(&base(vec![para("tab\there")]));
    assert!(e.contains("U+0009"), "{}", e);
    // Et le texte reste intact pour ce qui est permis : l'espace insécable.
    let pdf = ok(&base(vec![para("10\u{a0}000\u{a0}€")]));
    assert_eq!(texte_complet(&pdf), "10\u{a0}000\u{a0}€");
}

// ---------------------------------------------------------------------------
// Le logo et les documents.
// ---------------------------------------------------------------------------

#[test]
fn logo_sur_chaque_page_en_haut_a_droite_et_un_document_par_page() {
    let mut v = json!({
        "format": 1,
        "page": {"largeur": 400.0, "hauteur": 500.0,
                 "marges": {"haut": 100.0, "droite": 50.0, "bas": 50.0, "gauche": 50.0}},
        "police": {"taille": 10.0, "interligne": 1.25},
        "logo": {"chemin": image("logo.svg"), "largeur": 80.0, "marge_haut": 20.0, "marge_droite": 30.0},
        "documents": [
            {"blocs": [para("Premier document")]},
            {"blocs": [para("Deuxième document")]},
            {"blocs": [para(&(1..=70).map(|i| format!("ligne {}", i)).collect::<Vec<_>>().join("\n"))]}
        ]});
    let pdf = ok(&v);
    // 1 page + 1 page + (70 lignes à 12,5 pt dans 350 pt = 28 lignes par page -> 3 pages).
    assert_eq!(nb_pages(&pdf), 5);
    let l = lignes(&pdf);
    assert_eq!(l.iter().find(|x| x.texte == "Premier document").unwrap().page, 1);
    assert_eq!(l.iter().find(|x| x.texte == "Deuxième document").unwrap().page, 2);
    assert_eq!(l.iter().find(|x| x.texte == "ligne 1").unwrap().page, 3);
    for page in 1..=5 {
        assert_eq!(xobjects(&pdf, page, "Form"), 1, "le logo manque à la page {}", page);
    }
    // Position du logo (80 x 26,67 pt, à 30 pt du bord droit et 20 pt du haut), par l'inspection.
    let ins = inspecter(&pdf, "logo.pdf");
    let r = nombres(&ins["pages"][4]["traits"][0]);
    assert!((r[0] - 290.0).abs() < 0.1, "{:?}", r);
    assert!((r[1] - 20.0).abs() < 0.1, "{:?}", r);
    assert!((r[2] - 370.0).abs() < 0.1, "{:?}", r);
    assert!((r[3] - 46.667).abs() < 0.1, "{:?}", r);
    v["documents"] = json!([{"blocs": []}]);
    assert_eq!(nb_pages(&ok(&v)), 1, "un document vide fait une page (avec son logo)");
}

// ---------------------------------------------------------------------------
// La justification.
// ---------------------------------------------------------------------------

const LOREM: &str = "Le salarié est engagé en qualité de comédien-marionnettiste pour les représentations du spectacle « Le Fou & la Marotte » les douze, treize et quatorze novembre deux mille vingt-six au théâtre de la Halle, à Villeneuve-sur-Seine. Le présent contrat est conclu pour la durée de ces représentations et de leurs répétitions, conformément au code du travail, et il prend fin de plein droit à l’issue de la dernière représentation.";

#[test]
fn justification_les_lignes_pleines_finissent_au_bord_droit() {
    let pdf = ok(&base(vec![para_align(LOREM, "justifie")]));
    let l = lignes(&pdf);
    assert!(l.len() >= 4, "{:?}", l);
    // Largeur utile : 300 pt, de 50 à 350.
    for ligne in &l[..l.len() - 1] {
        assert!((ligne.x - 50.0).abs() < 0.01, "{:?}", ligne);
        assert!(
            (ligne.x + ligne.largeur - 350.0).abs() < 0.05,
            "la ligne ne finit pas au bord droit : {:?}",
            ligne
        );
        assert!(ligne.mot_espace > 0.0, "{:?}", ligne);
    }
    // La dernière ligne reste à gauche, à son espacement naturel.
    let derniere = l.last().unwrap();
    assert_eq!(derniere.mot_espace, 0.0);
    assert!(derniere.x + derniere.largeur < 350.0 - 1.0, "{:?}", derniere);
    // Le texte, lui, est celui du paragraphe : on recolle les lignes avec une espace.
    let recolle = l.iter().map(|x| x.texte.clone()).collect::<Vec<_>>().join(" ");
    assert_eq!(recolle, LOREM);
    // Et l'inspection voit les mêmes mots, le dernier mot des lignes pleines au bord droit.
    let ins = inspecter(&pdf, "justifie.pdf");
    let mut par_ligne: BTreeMap<i64, Vec<Vec<f64>>> = BTreeMap::new();
    for w in ins["pages"][0]["mots"].as_array().unwrap() {
        let r = nombres(&w["rect"]);
        par_ligne.entry(r[3].round() as i64).or_default().push(r);
    }
    let v: Vec<(f64, f64)> = par_ligne
        .values()
        .map(|ws| {
            (
                ws.iter().map(|w| w[0]).fold(f64::INFINITY, f64::min),
                ws.iter().map(|w| w[2]).fold(f64::NEG_INFINITY, f64::max),
            )
        })
        .collect();
    assert_eq!(v.len(), l.len(), "{:?}", v);
    for (x0, x1) in &v[..v.len() - 1] {
        assert!((x0 - 50.0).abs() < 0.6 && (x1 - 350.0).abs() < 0.6, "{:?}", v);
    }
}

#[test]
fn justification_les_lignes_finies_par_retour_forcé_restent_a_gauche() {
    let texte = "Première ligne assez courte\nSeconde ligne plus longue qui doit passer à la ligne puisqu'elle dépasse largement la largeur utile disponible ici\nFin";
    let pdf = ok(&base(vec![para_align(texte, "justifie")]));
    let l = lignes(&pdf);
    let finie_par_n = &l[0];
    assert_eq!(finie_par_n.texte, "Première ligne assez courte");
    assert_eq!(finie_par_n.mot_espace, 0.0, "{:?}", finie_par_n);
    // La deuxième partie est justifiée sur ses lignes pleines et pas sur sa dernière.
    let milieu: Vec<&Ligne> = l.iter().filter(|x| x.texte != "Fin" && x.texte != finie_par_n.texte).collect();
    assert!(milieu.len() >= 2);
    assert!(milieu[0].mot_espace > 0.0);
    assert_eq!(milieu.last().unwrap().mot_espace, 0.0);
    assert_eq!(l.last().unwrap().texte, "Fin");
}

#[test]
fn alignements_gauche_centre_droite() {
    let cas = [("gauche", 50.0), ("centre", -1.0), ("droite", -2.0)];
    for (al, _) in cas {
        let pdf = ok(&base(vec![para_align("Un texte court", al)]));
        let l = &lignes(&pdf)[0];
        match al {
            "gauche" => assert!((l.x - 50.0).abs() < 0.01),
            "centre" => assert!((l.x + l.largeur / 2.0 - 200.0).abs() < 0.05, "{:?}", l),
            _ => assert!((l.x + l.largeur - 350.0).abs() < 0.05, "{:?}", l),
        }
    }
}

// ---------------------------------------------------------------------------
// Le déterminisme.
// ---------------------------------------------------------------------------

#[test]
fn meme_description_meme_pdf_a_l_octet_pres() {
    let mut v = base(vec![
        para_align(LOREM, "justifie"),
        json!({"type": "image", "chemin": image("signature.png"), "largeur": 60.0}),
        json!({"type": "image", "chemin": image("scan.jpg"), "largeur": 60.0}),
    ]);
    v["logo"] = json!({"chemin": image("logo.svg"), "largeur": 60.0, "marge_haut": 5.0, "marge_droite": 5.0});
    v["documents"].as_array_mut().unwrap().push(json!({"blocs": [para("Deuxième")]}));
    // Dans le processus : dix fabrications (les tables de hachage tirent un ordre différent à
    // chaque fois).
    let premier = ok(&v);
    for _ in 0..10 {
        assert!(ok(&v) == premier, "deux fabrications donnent des fichiers différents");
    }
    // Entre deux processus, à une seconde d'écart.
    let dossier = dossier_temp("determinisme");
    let (a, b) = (dossier.join("a.pdf"), dossier.join("b.pdf"));
    assert_eq!(binaire(&v.to_string(), &a).0, 0);
    std::thread::sleep(std::time::Duration::from_millis(1100));
    assert_eq!(binaire(&v.to_string(), &b).0, 0);
    assert!(std::fs::read(&a).unwrap() == std::fs::read(&b).unwrap());
    assert!(std::fs::read(&a).unwrap() == premier);
    // Aucune date du jour dans le fichier.
    let brut = String::from_utf8_lossy(&premier).to_string();
    assert!(brut.contains("D:19700101000000"), "date fixe attendue");
    let annee = "2026";
    assert!(!brut.contains(&format!("D:{}", annee)), "aucune date variable");
}

// ---------------------------------------------------------------------------
// L'exemple du contrat.
// ---------------------------------------------------------------------------

fn description_exemple() -> Value {
    let texte = std::fs::read_to_string(exemples().join("contrat.json")).unwrap();
    let texte = texte.replace("@EXEMPLES@", &chemin(&exemples()));
    serde_json::from_str(&texte).unwrap()
}

#[test]
fn exemple_contrat_deux_pages_texte_logo_et_signatures() {
    let v = description_exemple();
    let pdf = ok(&v);
    assert_eq!(nb_pages(&pdf), 2, "le contrat d'exemple fait deux pages");
    // Le logo vectoriel sur chaque page, la signature PNG sur la dernière.
    assert_eq!(xobjects(&pdf, 1, "Form"), 1);
    assert_eq!(xobjects(&pdf, 2, "Form"), 1);
    assert_eq!(xobjects(&pdf, 1, "Image"), 0);
    assert_eq!(xobjects(&pdf, 2, "Image"), 1);

    // Le texte du PDF est celui des morceaux, dans l'ordre (les lignes recollées par une espace,
    // les retours forcés par un saut de ligne).
    let l = lignes(&pdf);
    let mut attendu: Vec<String> = Vec::new();
    collecter(&v["documents"][0]["blocs"], &mut attendu);
    let obtenu = l.iter().map(|x| x.texte.clone()).collect::<Vec<_>>().join(" ");
    let attendu = attendu.join(" ");
    let norme = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    assert_eq!(norme(&obtenu), norme(&attendu));

    // Les deux colonnes de signature sont côte à côte, la seconde à mi-page.
    let gauche = l.iter().find(|x| x.texte == "L’employeur").unwrap();
    let droite = l.iter().find(|x| x.texte == "Le salarié").unwrap();
    assert!((gauche.x - 52.0).abs() < 0.01);
    assert!((droite.x - (52.0 + (595.28 - 104.0) / 2.0)).abs() < 0.1, "{:?}", droite);
    assert_eq!(gauche.page, droite.page);

    let ins = inspecter(&pdf, "contrat.pdf");
    let pages = ins["pages"].as_array().unwrap();
    assert_eq!(pages.len(), 2);
    assert!(!pages[0]["traits"].as_array().unwrap().is_empty(), "logo page 1");
    assert!(!pages[1]["traits"].as_array().unwrap().is_empty(), "logo page 2");
    let s = texte_des_pages(&pdf, "contrat.pdf").join("\n");
    for attendu in [
        "CONTRAT D’ENGAGEMENT À DURÉE DÉTERMINÉE D’USAGE",
        "COMPAGNIE DES PETITS ŒUFS",
        "152,75 €",
        // L'espace insécable ressort telle quelle (la table ToUnicode la donne pour ce qu'elle est).
        "«\u{a0}l’employeur\u{a0}»",
        "Article 1 — Objet et durée",
        "signature",
    ] {
        assert!(s.contains(attendu), "la lecture n'a pas extrait {:?}", attendu);
    }
}

fn collecter(blocs: &Value, sortie: &mut Vec<String>) {
    for b in blocs.as_array().unwrap() {
        match b["type"].as_str().unwrap() {
            "paragraphe" => {
                let t: String = b["morceaux"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|m| m["texte"].as_str().unwrap())
                    .collect();
                sortie.push(t);
            }
            "colonnes" => {
                for c in b["colonnes"].as_array().unwrap() {
                    collecter(&c["blocs"], sortie);
                }
            }
            _ => {}
        }
    }
}

#[test]
fn exemple_contrat_par_le_binaire() {
    let texte = std::fs::read_to_string(exemples().join("contrat.json"))
        .unwrap()
        .replace("@EXEMPLES@", &chemin(&exemples()));
    let sortie = dossier_temp("exemple").join("contrat.pdf");
    let (code, err) = binaire(&texte, &sortie);
    assert_eq!(code, 0, "{}", err);
    assert_eq!(nb_pages(&std::fs::read(&sortie).unwrap()), 2);
}

#[test]
fn les_objets_du_pdf_sont_ordonnes_et_le_fichier_se_relit() {
    // Le fichier passe lopdf sans objet manquant : toute référence pointe un objet existant.
    let pdf = ok(&description_exemple());
    let doc = Document::load_mem(&pdf).unwrap();
    let mut vus: BTreeMap<u32, ()> = BTreeMap::new();
    for ((n, _), _) in doc.objects.iter() {
        vus.insert(*n, ());
    }
    fn refs(o: &Object, out: &mut Vec<u32>) {
        match o {
            Object::Reference((n, _)) => out.push(*n),
            Object::Array(a) => a.iter().for_each(|x| refs(x, out)),
            Object::Dictionary(d) => d.iter().for_each(|(_, x)| refs(x, out)),
            Object::Stream(s) => s.dict.iter().for_each(|(_, x)| refs(x, out)),
            _ => {}
        }
    }
    let mut toutes = Vec::new();
    for o in doc.objects.values() {
        refs(o, &mut toutes);
    }
    for r in doc.trailer.iter().flat_map(|(_, o)| {
        let mut v = Vec::new();
        refs(o, &mut v);
        v
    }) {
        toutes.push(r);
    }
    assert!(!toutes.is_empty());
    for r in toutes {
        assert!(vus.contains_key(&r), "référence pendante vers {}", r);
    }
}

// ---------------------------------------------------------------------------
// Les blocs à position fixe : texte_fixe, trait, image_fixe (FORMAT.md).
// ---------------------------------------------------------------------------

/// Une page de 400 x 500 sans marge, un document par page fixe.
fn fixe(documents: Vec<Vec<Value>>) -> Value {
    json!({
        "format": 2,
        "page": {"largeur": 400.0, "hauteur": 500.0,
                 "marges": {"haut": 0.0, "droite": 0.0, "bas": 0.0, "gauche": 0.0}},
        "police": {"taille": 10.0, "interligne": 1.25, "couleur": "#000000"},
        "documents": documents.into_iter().map(|b| json!({"blocs": b})).collect::<Vec<_>>()
    })
}

fn texte_fixe(texte: &str, x: f64, y: f64) -> Value {
    json!({"type": "texte_fixe", "texte": texte, "x": x, "y": y})
}

/// Les mots (x0, y0, x1, y1, texte) de la page 1, relevés par l'inspection de la fabrique.
fn mots(pdf: &[u8], nom: &str) -> Vec<(f64, f64, f64, f64, String)> {
    let ins = inspecter(pdf, nom);
    ins["pages"][0]["mots"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| {
            let r = nombres(&w["rect"]);
            (r[0], r[1], r[2], r[3], w["texte"].as_str().unwrap().to_string())
        })
        .collect()
}

#[test]
fn texte_fixe_se_pose_sur_sa_ligne_de_base_en_times_et_courier() {
    let v = fixe(vec![vec![
        texte_fixe("Bonjour", 100.0, 200.0),
        json!({"type": "texte_fixe", "texte": "Times", "x": 100.0, "y": 250.0,
               "police": "times", "gras": true, "taille": 12.0}),
        json!({"type": "texte_fixe", "texte": "Mono", "x": 100.0, "y": 300.0,
               "police": "courier", "taille": 10.0}),
    ]]);
    let m = mots(&ok(&v), "fixe_polices.pdf");
    assert_eq!(m.len(), 3, "{:?}", m);
    for (mot, x, y) in [("Bonjour", 100.0, 200.0), ("Times", 100.0, 250.0), ("Mono", 100.0, 300.0)] {
        let w = m.iter().find(|w| w.4 == mot).unwrap();
        assert!((w.0 - x).abs() < 0.1, "{} : x0 = {}", mot, w.0);
        // La ligne de base est entre le haut et le bas de la boîte du mot.
        assert!(w.1 < y && w.3 > y - 0.01, "{} : boîte {}..{} pour la base {}", mot, w.1, w.3, y);
    }
    // Courier : 0,6 em par lettre, 4 lettres de 10 pt.
    let mono = m.iter().find(|w| w.4 == "Mono").unwrap();
    assert!((mono.2 - mono.0 - 24.0).abs() < 0.1, "{:?}", mono);
    let noms = noms_de_polices(&ok(&v));
    assert_eq!(noms, ["LiberationMono", "LiberationSans", "LiberationSerif-Bold"], "{:?}", noms);
}

#[test]
fn texte_fixe_s_aligne_a_droite_et_au_centre_de_x() {
    let v = fixe(vec![vec![
        json!({"type": "texte_fixe", "texte": "Droite", "x": 300.0, "y": 100.0, "alignement": "droite"}),
        json!({"type": "texte_fixe", "texte": "Centre", "x": 200.0, "y": 150.0, "alignement": "centre",
               "police": "times"}),
    ]]);
    let m = mots(&ok(&v), "fixe_alignement.pdf");
    let d = m.iter().find(|w| w.4 == "Droite").unwrap();
    assert!((d.2 - 300.0).abs() < 0.3, "bord droit {}", d.2);
    let c = m.iter().find(|w| w.4 == "Centre").unwrap();
    assert!(((c.0 + c.2) / 2.0 - 200.0).abs() < 0.3, "centre {}", (c.0 + c.2) / 2.0);
}

#[test]
fn les_coordonnees_partent_du_coin_de_la_page_meme_avec_des_marges() {
    let mut v = fixe(vec![vec![texte_fixe("Coin", 20.0, 60.0)]]);
    v["page"]["marges"] = json!({"haut": 40.0, "droite": 30.0, "bas": 40.0, "gauche": 30.0});
    let m = mots(&ok(&v), "fixe_marges.pdf");
    assert!((m[0].0 - 20.0).abs() < 0.1 && m[0].3 > 59.9 && m[0].1 < 60.0, "{:?}", m);
}

#[test]
fn trait_et_image_fixes_se_posent_aux_coordonnees_donnees() {
    let v = fixe(vec![vec![
        json!({"type": "trait", "x1": 10.0, "y1": 20.0, "x2": 110.0, "y2": 20.0,
               "epaisseur": 3.0, "couleur": "#FF0000"}),
        json!({"type": "image_fixe", "chemin": image("signature.png"),
               "x": 200.0, "y": 300.0, "largeur": 80.0, "hauteur": 20.0}),
        json!({"type": "image_fixe", "chemin": image("signature.png"), "centree": true,
               "x": 200.0, "y": 400.0, "largeur": 80.0, "hauteur": 20.0}),
    ]]);
    let pdf = ok(&v);
    let ins = inspecter(&pdf, "fixe_trait_image.pdf");
    let page = &ins["pages"][0];
    // Le trait de (10, 20) à (110, 20) : sa boîte (l'inspection ne compte pas l'épaisseur)...
    let traits = page["traits"].as_array().unwrap();
    assert_eq!(traits.len(), 1, "{:?}", traits);
    let t = nombres(&traits[0]);
    for (obtenu, attendu) in t.iter().zip([10.0, 20.0, 110.0, 20.0]) {
        assert!((obtenu - attendu).abs() < 0.05, "{:?}", t);
    }
    // ... et, dans le flux, son épaisseur (3) et sa couleur (rouge), que l'inspection ne rend pas.
    let doc = Document::load_mem(&pdf).unwrap();
    let (_, id) = doc.get_pages().into_iter().next().unwrap();
    let ops = Content::decode(&doc.get_page_content(id).unwrap()).unwrap().operations;
    let operandes = |op: &str| -> Vec<Vec<f32>> {
        ops.iter()
            .filter(|o| o.operator == op)
            .map(|o| o.operands.iter().map(nombre).collect())
            .collect()
    };
    let w = operandes("w");
    assert!(w.len() == 1 && (w[0][0] - 3.0).abs() < 0.001, "{:?}", w);
    assert_eq!(operandes("RG"), [[1.0, 0.0, 0.0]]);
    let images: Vec<Vec<f64>> = page["images"].as_array().unwrap().iter().map(nombres).collect();
    assert_eq!(images.len(), 2, "{:?}", images);
    // Dans la boîte 80 x 20, proportions gardées : la hauteur limite, donc plus étroite que 80.
    for (b, haut) in images.iter().zip([300.0, 400.0]) {
        assert!(b[2] - b[0] < 80.0 - 0.5 && (b[3] - b[1] - 20.0).abs() < 0.3, "{:?}", b);
        assert!(b[1] >= haut - 0.2 && b[3] <= haut + 20.2, "{:?}", b);
    }
    // Calée en haut à gauche, ou centrée dans la boîte.
    assert!((images[0][0] - 200.0).abs() < 0.2, "{:?}", images[0]);
    assert!(((images[1][0] + images[1][2]) / 2.0 - 240.0).abs() < 0.2, "{:?}", images[1]);
}

#[test]
fn un_document_par_page_fixe() {
    let v = fixe(vec![
        vec![texte_fixe("Page un", 10.0, 50.0)],
        vec![texte_fixe("Page deux", 10.0, 50.0), texte_fixe("encore", 10.0, 80.0)],
        vec![texte_fixe("Page trois", 10.0, 50.0)],
    ]);
    let pdf = ok(&v);
    assert_eq!(nb_pages(&pdf), 3);
    let l = lignes(&pdf);
    assert_eq!(l.iter().filter(|x| x.page == 2).count(), 2, "{:?}", l);
}

#[test]
fn blocs_fixes_refus_en_une_ligne() {
    let bien = texte_fixe("x", 1.0, 1.0);
    let e = refus(&fixe(vec![vec![json!({"type": "texte_fixe", "texte": "a", "x": 1.0, "y": 2.0,
                                          "police": "comic"})]]));
    assert!(e.contains("bloc 1") && e.contains("comic") && e.contains("helvetica, times, courier"), "{}", e);
    let e = refus(&fixe(vec![vec![json!({"type": "texte_fixe", "texte": "a", "x": 1.0, "y": 2.0,
                                          "alignement": "justifie"})]]));
    assert!(e.contains("ne se justifie pas"), "{}", e);
    let e = refus(&fixe(vec![vec![texte_fixe("deux\nlignes", 1.0, 2.0)]]));
    assert!(e.contains("une ligne"), "{}", e);
    // Un caractère que la police elle-même n'a pas : un idéogramme.
    let e = refus(&fixe(vec![vec![texte_fixe("日", 1.0, 2.0)]]));
    assert!(e.contains("日") && e.contains("U+65E5"), "{}", e);
    // Un champ manquant ou inconnu.
    let e = refus(&fixe(vec![vec![json!({"type": "texte_fixe", "texte": "a", "x": 1.0})]]));
    assert!(e.contains("\"y\""), "{}", e);
    let e = refus(&fixe(vec![vec![json!({"type": "trait", "x1": 0, "y1": 0, "x2": 1, "y2": 1, "largeur": 2})]]));
    assert!(e.contains("largeur"), "{}", e);
    let e = refus(&fixe(vec![vec![json!({"type": "trait", "x1": 0, "y1": 0, "x2": 1, "y2": 1,
                                          "epaisseur": 0})]]));
    assert!(e.contains("epaisseur"), "{}", e);
    // Fixes et blocs qui coulent ne se mêlent pas dans un document.
    let e = refus(&fixe(vec![vec![bien.clone(), para("coule")]]));
    assert!(e.contains("position fixe") && e.contains("bloc 2"), "{}", e);
    // Ni dans des colonnes.
    let e = refus(&fixe(vec![vec![json!({"type": "colonnes", "colonnes": [
        {"largeur": 1.0, "blocs": [bien]}]})]]));
    assert!(e.contains("colonnes") && e.contains("position fixe"), "{}", e);
    // Une image introuvable est un refus, pas un blanc.
    let e = refus(&fixe(vec![vec![json!({"type": "image_fixe", "chemin": "/nulle/part.png",
                                          "x": 0, "y": 0, "largeur": 10, "hauteur": 10})]]));
    assert!(e.contains("introuvable"), "{}", e);
}

#[test]
fn meme_description_fixe_meme_pdf() {
    let v = fixe(vec![vec![
        texte_fixe("Fixe", 10.0, 50.0),
        json!({"type": "trait", "x1": 0, "y1": 60, "x2": 100, "y2": 60}),
        json!({"type": "texte_fixe", "texte": "T", "x": 10, "y": 90, "police": "times"}),
    ]]);
    assert_eq!(ok(&v), ok(&v));
}

// ---------------------------------------------------------------------------
// L'en-tête et le pied de page répétés.
// ---------------------------------------------------------------------------

/// Trois pages de paragraphes (un par ligne de la page de 400 x 500, marges de 50).
fn pages_repetees(extra: Value) -> Value {
    let mut v = base((0..120).map(|i| para(&format!("ligne {}", i))).collect());
    for (k, x) in extra.as_object().unwrap() {
        v[k] = x.clone();
    }
    v
}

#[test]
fn pied_et_entete_repetes_avec_numero_et_total() {
    let v = pages_repetees(json!({
        "entete_page": {"gauche": "Titre de l'état", "droite": "(suite)", "premiere_page": false},
        "pied_page": {"gauche": "Période 2026", "centre": "au centre",
                      "droite": "page {page} sur {pages}", "taille": 6.5, "couleur": "#777777"}
    }));
    let pdf = ok(&v);
    let n = nb_pages(&pdf);
    assert!(n >= 3, "{} pages", n);
    let l = lignes(&pdf);
    for p in 1..=n {
        let de = |t: &str| l.iter().filter(|x| x.page == p && x.texte == t).count();
        let pied = format!("page {} sur {}", p, n);
        assert_eq!(de(&pied), 1, "page {} : {:?}", p, l.iter().filter(|x| x.page == p).map(|x| &x.texte).collect::<Vec<_>>());
        assert_eq!(de("Période 2026"), 1);
        assert_eq!(de("au centre"), 1);
        // L'en-tête saute la première page.
        assert_eq!(de("Titre de l'état"), if p == 1 { 0 } else { 1 });
        let d = l.iter().find(|x| x.page == p && x.texte == pied).unwrap();
        assert!((d.x + d.largeur - 350.0).abs() < 0.05, "bord droit {}", d.x + d.largeur);
        assert!((d.y - 20.0).abs() < 0.2, "ligne de base du pied : {}", d.y);
        let g = l.iter().find(|x| x.page == p && x.texte == "Période 2026").unwrap();
        assert!((g.x - 50.0).abs() < 0.05);
        let c = l.iter().find(|x| x.page == p && x.texte == "au centre").unwrap();
        assert!((c.x + c.largeur / 2.0 - 200.0).abs() < 0.05);
    }
    let e = l.iter().find(|x| x.page == 2 && x.texte == "Titre de l'état").unwrap();
    assert!((e.y - 480.0).abs() < 0.2, "ligne de base de l'en-tête : {}", e.y);
    // Le corps est inchangé : même nombre de pages qu'avec ni pied.
    assert_eq!(n, nb_pages(&ok(&pages_repetees(json!({})))));
    // Deux passes : même description, même PDF.
    assert_eq!(pdf, ok(&v));
}

#[test]
fn pied_sans_total_ne_fait_qu_une_passe_et_numerote() {
    let pdf = ok(&pages_repetees(json!({"pied_page": {"droite": "{page}"}})));
    let l = lignes(&pdf);
    for p in 1..=nb_pages(&pdf) {
        assert!(l.iter().any(|x| x.page == p && x.texte == p.to_string()), "page {}", p);
    }
}

#[test]
fn refus_entete_et_pied() {
    let e = refus(&pages_repetees(json!({"pied_page": {"gauche": "a\nb"}})));
    assert!(e.contains("pied_page") && e.contains("une ligne"), "{}", e);
    let e = refus(&pages_repetees(json!({"entete_page": {"gauche": "日"}})));
    assert!(e.contains("entete_page") && e.contains("日"), "{}", e);
    let e = refus(&pages_repetees(json!({"pied_page": {"gauche": "a", "couleurs": 1}})));
    assert!(e.contains("couleurs"), "{}", e);
    let e = refus(&pages_repetees(json!({"pied_page": {"gauche": "a", "taille": 0}})));
    assert!(e.contains("taille"), "{}", e);
}

// ---------------------------------------------------------------------------
// Les polices embarquées en sous-ensemble (étape 8) : tout caractère de Liberation s'imprime.
// ---------------------------------------------------------------------------

const NOMS: [&str; 3] = ["Łukasz Wójcik", "Zoë", "Ştefan"];

/// Les objets d'une police du PDF (nom PostScript sans l'étiquette) : (dictionnaire de la police
/// composée, octets du fichier de police, `CIDToGIDMap`, table `ToUnicode`).
fn police_embarquee(pdf: &[u8], nom: &str) -> (lopdf::Dictionary, Vec<u8>, Vec<u8>, String) {
    let doc = Document::load_mem(pdf).unwrap();
    let flux = |o: &Object| -> Vec<u8> {
        let id = o.as_reference().unwrap();
        let s = doc.get_object(id).unwrap().as_stream().unwrap();
        s.decompressed_content().unwrap_or_else(|_| s.content.clone())
    };
    for o in doc.objects.values() {
        let Object::Dictionary(d) = o else { continue };
        if d.get(b"Subtype").ok().and_then(|s| s.as_name().ok()) != Some(b"Type0") {
            continue;
        }
        let base = d.get(b"BaseFont").unwrap().as_name_str().unwrap();
        if !base.ends_with(&format!("+{}", nom)) {
            continue;
        }
        let descendante = d.get(b"DescendantFonts").unwrap().as_array().unwrap()[0].as_dict().unwrap();
        let descripteur = doc
            .get_dictionary(descendante.get(b"FontDescriptor").unwrap().as_reference().unwrap())
            .unwrap();
        let fichier = flux(descripteur.get(b"FontFile2").unwrap());
        let cid_gid = flux(descendante.get(b"CIDToGIDMap").unwrap());
        let tounicode = String::from_utf8(flux(d.get(b"ToUnicode").unwrap())).unwrap();
        return (d.clone(), fichier, cid_gid, tounicode);
    }
    panic!("police {} absente", nom);
}

/// Pour chaque caractère de `texte`, le glyphe du sous-ensemble (par `CIDToGIDMap`) a la même
/// largeur et le même dessin que celui de la police d'origine, et la table `ToUnicode` rend le
/// caractère : c'est ce qui fait que le texte extrait est exact.
fn verifier_police(pdf: &[u8], nom: &str, fichier_origine: &str, texte: &str) {
    let (_, sous, cid_gid, tounicode) = police_embarquee(pdf, nom);
    let origine = face(fichier_origine);
    let f0 = ttf_parser::Face::parse(&origine, 0).unwrap();
    let f1 = ttf_parser::Face::parse(&sous, 0).expect("le sous-ensemble est un TTF lisible");
    let distincts: std::collections::BTreeSet<char> = texte.chars().collect();
    // Le sous-ensemble ne porte que les glyphes employés, le .notdef et les composants des lettres
    // accentuées (la police en a 2 600).
    assert!(
        (f1.number_of_glyphs() as usize) < 2 * distincts.len() + 4,
        "{} glyphes pour {} caractères",
        f1.number_of_glyphs(),
        distincts.len()
    );
    assert!(sous.len() < 40_000, "sous-ensemble de {} octets", sous.len());
    for c in distincts {
        let g0 = f0.glyph_index(c).unwrap_or_else(|| panic!("{:?} absent de {}", c, fichier_origine));
        let i = 2 * c as usize;
        let g1 = ttf_parser::GlyphId(((cid_gid[i] as u16) << 8) | cid_gid[i + 1] as u16);
        assert_ne!(g1.0, 0, "{:?} sans glyphe", c);
        assert_eq!(f0.glyph_hor_advance(g0), f1.glyph_hor_advance(g1), "largeur de {:?}", c);
        assert_eq!(f0.glyph_bounding_box(g0), f1.glyph_bounding_box(g1), "dessin de {:?}", c);
        assert!(
            tounicode.contains(&format!("<{:04X}> <{:04X}>", c as u32, c as u32)),
            "ToUnicode ne rend pas {:?}",
            c
        );
    }
}

#[test]
fn les_noms_hors_windows_1252_s_impriment_et_s_extraient_justes() {
    let phrase = format!("Entre la compagnie et {}, {} et {}.", NOMS[0], NOMS[1], NOMS[2]);
    let pdf = ok(&base(vec![
        para(&phrase),
        json!({"type": "paragraphe", "morceaux": [{"texte": "Gras : "}, {"texte": NOMS[0], "gras": true}]}),
    ]));
    // Le flux : ce que le fichier dit imprimer.
    assert_eq!(texte_complet(&pdf), format!("{}\nGras : {}", phrase, NOMS[0]));
    // Chaque glyphe est le bon, la table ToUnicode rend le caractère.
    verifier_police(&pdf, "LiberationSans", "LiberationSans-Regular.ttf", &format!("{}Gras : ", phrase));
    verifier_police(&pdf, "LiberationSans-Bold", "LiberationSans-Bold.ttf", NOMS[0]);
    // Rien d'autre n'est embarqué (ni l'italique, ni Serif, ni Mono) : un contrat de cette taille
    // pèse quelques dizaines de Ko, pas les 1,6 Mo des fichiers.
    assert_eq!(noms_de_polices(&pdf), ["LiberationSans", "LiberationSans-Bold"]);
    assert!(pdf.len() < 60_000, "{} octets", pdf.len());

    let sortie = texte_des_pages(&pdf, "noms_exotiques.pdf")[0].clone();
    // Le texte extrait par la fabrique est exactement le texte imprimé (les retours à la ligne
    // de la mise en page mis à part).
    let attendu = format!("{} Gras : {}", phrase, NOMS[0]);
    let obtenu = sortie.split_whitespace().collect::<Vec<_>>().join(" ");
    assert_eq!(obtenu, attendu);
    for n in NOMS {
        assert!(sortie.contains(n), "la lecture n'a pas extrait {:?} : {}", n, sortie);
    }
}

#[test]
fn le_texte_justifie_s_extrait_juste_avec_des_caracteres_exotiques() {
    // Un paragraphe justifié (décalages `TJ` après les espaces) : l'extraction rend les mêmes mots.
    let mots: Vec<String> = (0..40).map(|i| format!("{}{}", NOMS[i % 3], i)).collect();
    let phrase = mots.join(" ");
    let pdf = ok(&base(vec![para_align(&phrase, "justifie")]));
    let l = lignes(&pdf);
    assert!(l.len() > 2, "{:?}", l);
    assert!(l[0].mot_espace > 0.0, "la première ligne est justifiée : {:?}", l[0]);
    let relu = l.iter().map(|x| x.texte.clone()).collect::<Vec<_>>().join(" ");
    assert_eq!(relu, phrase);
    let sortie = texte_des_pages(&pdf, "justifie_exotique.pdf")[0].clone();
    assert_eq!(sortie.split_whitespace().collect::<Vec<_>>().join(" "), phrase);
}

#[test]
fn italique_et_gras_italique_sont_de_vraies_polices() {
    let v = base(vec![json!({"type": "paragraphe", "morceaux": [
        {"texte": "droit "}, {"texte": "Łukasz", "italique": true},
        {"texte": " Zoë", "italique": true, "gras": true}]})]);
    let pdf = ok(&v);
    assert_eq!(texte_complet(&pdf), "droit Łukasz Zoë");
    assert_eq!(
        noms_de_polices(&pdf),
        ["LiberationSans", "LiberationSans-BoldItalic", "LiberationSans-Italic"]
    );
    verifier_police(&pdf, "LiberationSans-Italic", "LiberationSans-Italic.ttf", "Łukasz");
    verifier_police(&pdf, "LiberationSans-BoldItalic", "LiberationSans-BoldItalic.ttf", " Zoë");
}

#[test]
fn serif_et_mono_impriment_aussi_les_caracteres_exotiques() {
    let v = fixe(vec![vec![
        json!({"type": "texte_fixe", "texte": "Łukasz Wójcik", "x": 100.0, "y": 200.0,
               "police": "times", "gras": true, "taille": 12.0}),
        json!({"type": "texte_fixe", "texte": "Ştefan Zoë", "x": 100.0, "y": 250.0,
               "police": "courier", "gras": true, "taille": 10.0}),
        json!({"type": "texte_fixe", "texte": "Zoë", "x": 100.0, "y": 300.0, "police": "courier"}),
    ]]);
    let pdf = ok(&v);
    assert_eq!(texte_complet(&pdf), "Łukasz Wójcik\nŞtefan Zoë\nZoë");
    assert_eq!(
        noms_de_polices(&pdf),
        ["LiberationMono", "LiberationMono-Bold", "LiberationSerif-Bold"]
    );
    verifier_police(&pdf, "LiberationSerif-Bold", "LiberationSerif-Bold.ttf", "Łukasz Wójcik");
    verifier_police(&pdf, "LiberationMono-Bold", "LiberationMono-Bold.ttf", "Ştefan Zoë");
    verifier_police(&pdf, "LiberationMono", "LiberationMono-Regular.ttf", "Zoë");
    let m = mots(&pdf, "fixe_exotique.pdf");
    let t: Vec<&str> = m.iter().map(|w| w.4.as_str()).collect();
    assert_eq!(t, ["Łukasz", "Wójcik", "Ştefan", "Zoë", "Zoë"]);
}

#[test]
fn des_caracteres_de_toutes_les_ecritures_de_liberation_s_impriment() {
    // Latin étendu, grec, cyrillique, ponctuation et monnaies : tout ce que la police possède.
    let phrase = "ő č ğ ī ŵ ǆ Ǹ ß · Ελληνικά Кириллица № − ≤ → ½ ‰ ¶";
    let pdf = ok(&base(vec![para(phrase)]));
    assert_eq!(texte_complet(&pdf), phrase);
    verifier_police(&pdf, "LiberationSans", "LiberationSans-Regular.ttf", phrase);
    let sortie = texte_des_pages(&pdf, "ecritures.pdf");
    assert_eq!(sortie[0].trim(), phrase);
}

#[test]
fn le_sous_ensemble_est_deterministe_et_ne_depend_que_du_texte() {
    let v = base(vec![para(&format!("Bonjour {} et {}", NOMS[0], NOMS[2]))]);
    let (a, b) = (ok(&v), ok(&v));
    assert_eq!(a, b);
    // Un autre texte : d'autres glyphes, donc un autre fichier -- et une autre étiquette de nom.
    let c = ok(&base(vec![para(&format!("Bonjour {} et {}", NOMS[0], NOMS[1]))]));
    assert_ne!(a, c);
    let nom = |pdf: &[u8]| {
        let doc = Document::load_mem(pdf).unwrap();
        doc.objects
            .values()
            .find_map(|o| match o {
                Object::Dictionary(d) if d.get(b"Subtype").ok().and_then(|s| s.as_name().ok()) == Some(b"Type0") => {
                    Some(d.get(b"BaseFont").unwrap().as_name_str().unwrap().to_string())
                }
                _ => None,
            })
            .unwrap()
    };
    assert_ne!(nom(&a), nom(&c));
    // Ni date ni identifiant tirés au hasard : on le vérifie en lançant le binaire deux fois.
    let d = serde_json::to_string(&v).unwrap();
    let (s1, s2) = (dossier_temp("det1").join("a.pdf"), dossier_temp("det2").join("a.pdf"));
    assert_eq!(binaire(&d, &s1).0, 0);
    assert_eq!(binaire(&d, &s2).0, 0);
    assert_eq!(std::fs::read(&s1).unwrap(), std::fs::read(&s2).unwrap());
    assert_eq!(std::fs::read(&s1).unwrap(), a);
}

#[test]
fn le_binaire_refuse_en_une_ligne_un_caractere_que_la_police_n_a_pas() {
    let d = serde_json::to_string(&base(vec![para("Zoë 漢")])).unwrap();
    let sortie = dossier_temp("refus_police").join("a.pdf");
    let (code, err) = binaire(&d, &sortie);
    assert_eq!(code, 1);
    assert_eq!(err.trim().lines().count(), 1, "{}", err);
    assert!(err.contains("« 漢 » (U+6F22) n'existe pas dans la police Liberation"), "{}", err);
    assert!(!sortie.exists());
}
