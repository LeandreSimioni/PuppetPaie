//! Tests de `--champs` et `--remplir` (FORMAT.md, « Remplir un formulaire ») sur un petit
//! formulaire AcroForm FICTIF, écrit à la main ici (comme un outil tiers l'écrirait) : champs texte
//! simples, lecture seule, peigne, multiligne, centré, hiérarchie parent/enfant avec un widget
//! séparé, case à cocher. Les images (PNG, JPEG) sont fabriquées par la crate `image`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use lopdf::{Document, Object, ObjectId};
use serde_json::{json, Value};

mod commun;
use commun::{chemin, dossier_temp, flux, lancer, octets_fabrique, pdf_a_la_main, textes};

// Les numéros d'objets du formulaire d'essai.
const PAGE1: ObjectId = (3, 0);
const PAGE2: ObjectId = (4, 0);
const ACROFORM: ObjectId = (8, 0);
const NOM: ObjectId = (9, 0);
const CODE: ObjectId = (10, 0);
const PEIGNE: ObjectId = (11, 0);
const NOTE: ObjectId = (12, 0);
const CENTRE: ObjectId = (13, 0);
const ENFANT: ObjectId = (15, 0);
const WIDGET_ENFANT: ObjectId = (16, 0);

const CONTENU_PAGE1: &str = "q 1 0 0 1 0 0 cm\nBT /F1 12 Tf 50 780 Td (Page un) Tj ET\n";
const CONTENU_PAGE2: &str = "BT /F1 12 Tf 50 780 Td (Page deux) Tj ET\n";
const HAUTEUR: f64 = 842.0;

/// Le formulaire d'essai : deux pages 595 × 842 (`rotation` : le `/Rotate` de la page 2).
///
/// | champ | page | rect | particularité |
/// |---|---|---|---|
/// | NOM | 1 | 100 700 300 716 | texte, `/DA` à lui |
/// | CODE | 1 | 100 680 200 696 | lecture seule, `/V (Été)` |
/// | PEIGNE | 1 | 100 650 140 666 | peigne, `/MaxLen 2` |
/// | NOTE | 1 | 100 500 200 560 | multiligne |
/// | CENTRE | 2 | 100 700 200 716 | `/Q 1`, `/V` en UTF-16BE « Łuca » |
/// | PARENT.ENFANT | 2 | 100 600 300 620 | widget séparé du champ |
/// | CASE | 2 | 100 500 112 512 | case à cocher cochée |
fn formulaire(rotation: Option<i64>) -> Vec<u8> {
    let rotate = rotation.map(|r| format!(" /Rotate {}", r)).unwrap_or_default();
    let widget = |ft: &str, ff: &str, rect: &str, extra: &str| -> String {
        format!("<< /Type /Annot /Subtype /Widget /F 4 /FT /{} {} /Rect [{}] {} >>", ft, ff, rect, extra)
    };
    let objets = vec![
        // 1
        "<< /Type /Catalog /Pages 2 0 R /AcroForm 8 0 R >>".to_string(),
        // 2 : la police et le cadre sont hérités par les pages
        "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 /MediaBox [0 0 595 842] \
         /Resources << /Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >> >> >>"
            .to_string(),
        // 3
        "<< /Type /Page /Parent 2 0 R /Contents 5 0 R /Annots [9 0 R 10 0 R 11 0 R 12 0 R] >>".to_string(),
        // 4
        format!("<< /Type /Page /Parent 2 0 R /Contents 6 0 R /Annots [13 0 R 16 0 R 17 0 R]{} >>", rotate),
        // 5, 6
        flux("", CONTENU_PAGE1),
        flux("", CONTENU_PAGE2),
        // 7
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_string(),
        // 8
        "<< /Fields [9 0 R 10 0 R 11 0 R 12 0 R 13 0 R 14 0 R 17 0 R] /DA (/Helv 7 Tf 0 g) \
         /DR << /Font << /Helv 7 0 R >> >> >>"
            .to_string(),
        // 9 NOM
        widget("Tx", "/T (NOM) /DA (/Helv 7 Tf 0 g)", "100 700 300 716", ""),
        // 10 CODE (lecture seule)
        widget("Tx", "/T (CODE) /Ff 1", "100 680 200 696", "/V (\\311t\\351)"),
        // 11 PEIGNE (bit 25)
        widget("Tx", "/T (PEIGNE) /Ff 16777216 /MaxLen 2", "100 650 140 666", ""),
        // 12 NOTE (multiligne, bit 13)
        widget("Tx", "/T (NOTE) /Ff 4096", "100 500 200 560", ""),
        // 13 CENTRE
        widget("Tx", "/T (CENTRE) /Q 1", "100 700 200 716", "/V <FEFF0141007500630061>"),
        // 14 PARENT
        "<< /T (PARENT) /FT /Tx /Kids [15 0 R] >>".to_string(),
        // 15 ENFANT
        "<< /T (ENFANT) /Parent 14 0 R /Kids [16 0 R] >>".to_string(),
        // 16 le widget d'ENFANT : sans nom
        "<< /Type /Annot /Subtype /Widget /F 4 /Parent 15 0 R /Rect [100 600 300 620] >>".to_string(),
        // 17 CASE
        widget("Btn", "/T (CASE)", "100 500 112 512", "/V /Oui /AS /Oui"),
    ];
    pdf_a_la_main(&objets, 0)
}

fn charge(o: &[u8]) -> Document {
    Document::load_mem(o).unwrap()
}

fn num(o: &Object) -> f64 {
    match o {
        Object::Integer(i) => *i as f64,
        Object::Real(r) => *r as f64,
        autre => panic!("un nombre est attendu : {:?}", autre),
    }
}

fn proche(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.011
}

fn champs(o: &[u8]) -> Value {
    fabrique_pdf::formulaire::champs(o).unwrap_or_else(|e| panic!("{}", e.0))
}

fn remplir(o: &[u8], d: Value) -> Result<Vec<u8>, String> {
    fabrique_pdf::formulaire::remplir(o, &d.to_string()).map_err(|e| e.0)
}

fn remplir_ok(o: &[u8], d: Value) -> Vec<u8> {
    remplir(o, d).unwrap_or_else(|e| panic!("refus inattendu : {}", e))
}

fn refus(o: &[u8], d: Value) -> String {
    match remplir(o, d) {
        Ok(_) => panic!("un refus était attendu"),
        Err(m) => m,
    }
}

/// `--remplir` en ligne de commande, la description sur l'entrée standard.
fn remplir_cli(entree: &str, sortie: &str, d: &Value) -> std::process::Output {
    let mut p = Command::new(env!("CARGO_BIN_EXE_fabrique-pdf"))
        .args(["--remplir", entree, sortie])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    p.stdin.take().unwrap().write_all(d.to_string().as_bytes()).unwrap();
    p.wait_with_output().unwrap()
}

/// Le flux d'apparence (`/AP /N`) d'un widget, décodé, et le dictionnaire de cet objet formulaire.
fn apparence(doc: &Document, widget: ObjectId) -> (String, lopdf::Dictionary) {
    let w = doc.get_dictionary(widget).unwrap();
    let ap = w.get(b"AP").expect("pas de /AP").as_dict().unwrap();
    let n = ap.get(b"N").unwrap().as_reference().unwrap();
    let s = doc.get_object(n).unwrap().as_stream().unwrap();
    let contenu = s.decompressed_content().unwrap_or_else(|_| s.content.clone());
    (String::from_utf8(contenu).unwrap(), s.dict.clone())
}

/// Décompresse un flux Flate (zlib) à la main : ce que fait un lecteur.
fn deflate(octets: &[u8]) -> Vec<u8> {
    use std::io::Read;
    let mut sortie = Vec::new();
    flate2::read::ZlibDecoder::new(octets).read_to_end(&mut sortie).expect("flux Flate valide");
    sortie
}

fn bbox(dict: &lopdf::Dictionary) -> Vec<f64> {
    dict.get(b"BBox").unwrap().as_array().unwrap().iter().map(num).collect()
}

fn valeur_pdf(doc: &Document, id: ObjectId) -> Vec<u8> {
    match doc.get_dictionary(id).unwrap().get(b"V").unwrap() {
        Object::String(s, _) => s.clone(),
        autre => panic!("/V n'est pas une chaîne : {:?}", autre),
    }
}

/// Les `Tm` (x, y) et le texte du `Tj` qui suit, d'un flux d'apparence (codes de 4 chiffres hexa).
fn lignes(flux: &str) -> Vec<(f64, f64, String)> {
    let mut out = Vec::new();
    let mut tm: Option<(f64, f64)> = None;
    for l in flux.lines() {
        if let Some(r) = l.strip_suffix(" Tm") {
            let v: Vec<f64> = r.split_whitespace().map(|x| x.parse().unwrap()).collect();
            assert_eq!(v.len(), 6, "{}", l);
            assert_eq!((v[0], v[1], v[2], v[3]), (1.0, 0.0, 0.0, 1.0), "{}", l);
            tm = Some((v[4], v[5]));
        } else if let Some(h) = l.strip_suffix("> Tj") {
            let h = h.strip_prefix('<').unwrap();
            let t: String = h
                .as_bytes()
                .chunks(4)
                .map(|c| char::from_u32(u32::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap()).unwrap())
                .collect();
            let (x, y) = tm.take().expect("Tj sans Tm");
            out.push((x, y, t));
        }
    }
    out
}

fn hex(t: &str) -> String {
    t.chars().map(|c| format!("{:04X}", c as u32)).collect()
}

fn largeur(t: &str, corps: f64) -> f64 {
    let donnees = fabrique_pdf::polices::sans_regulier().unwrap();
    let face = ttf_parser::Face::parse(&donnees, 0).unwrap();
    let em = face.units_per_em() as f64;
    t.chars().map(|c| face.glyph_index(c).and_then(|g| face.glyph_hor_advance(g)).unwrap_or(0) as f64 / em * corps).sum()
}


// ------------------------------------------------------------------------------ --champs

#[test]
fn champs_noms_complets_types_pages_et_rectangles() {
    let c = champs(&formulaire(None));
    let o = c.as_object().unwrap();
    // tous les champs terminaux
    let mut noms: Vec<&str> = o.keys().map(|s| s.as_str()).collect();
    noms.sort();
    assert_eq!(noms, vec!["CASE", "CENTRE", "CODE", "NOM", "NOTE", "PARENT.ENFANT", "PEIGNE"]);
    // la sortie de --champs les écrit dans l'ordre de /AcroForm /Fields
    let ligne = fabrique_pdf::formulaire::champs_json(&formulaire(None)).unwrap();
    let positions: Vec<usize> = ["NOM", "CODE", "PEIGNE", "NOTE", "CENTRE", "PARENT.ENFANT", "CASE"]
        .iter()
        .map(|n| ligne.find(&format!("\"{}\":", n)).unwrap())
        .collect();
    assert!(positions.windows(2).all(|w| w[0] < w[1]), "{}", ligne);
    assert_eq!(serde_json::from_str::<Value>(&ligne).unwrap(), c);

    // y0 = hauteur de la page - y1 du /Rect
    assert_eq!(c["NOM"]["rect"], json!([100.0, 126.0, 300.0, 142.0]));
    assert_eq!(c["NOM"]["page"], json!(1));
    assert_eq!(c["NOM"]["type"], json!("texte"));
    assert_eq!(c["NOM"]["lecture_seule"], json!(false));
    assert_eq!(c["NOM"]["valeur"], Value::Null);

    assert_eq!(c["CODE"]["valeur"], json!("Été"));
    assert_eq!(c["CODE"]["lecture_seule"], json!(true));
    assert_eq!(c["CODE"]["rect"], json!([100.0, HAUTEUR - 696.0, 200.0, HAUTEUR - 680.0]));

    assert_eq!(c["PEIGNE"]["valeur"], Value::Null);
    assert_eq!(c["PEIGNE"]["rect"], json!([100.0, 176.0, 140.0, 192.0]));
    assert_eq!(c["NOTE"]["rect"], json!([100.0, 282.0, 200.0, 342.0]));

    // page 2, /V en UTF-16BE
    assert_eq!(c["CENTRE"]["page"], json!(2));
    assert_eq!(c["CENTRE"]["valeur"], json!("Łuca"));

    // la hiérarchie : le nom complet, et le widget séparé donne page et rectangle
    assert_eq!(c["PARENT.ENFANT"]["type"], json!("texte"));
    assert_eq!(c["PARENT.ENFANT"]["page"], json!(2));
    assert_eq!(c["PARENT.ENFANT"]["rect"], json!([100.0, 222.0, 300.0, 242.0]));
    assert!(o.get("PARENT").is_none() && o.get("ENFANT").is_none());

    // la case : un bouton, dont la valeur est un nom
    assert_eq!(c["CASE"]["type"], json!("bouton"));
    assert_eq!(c["CASE"]["valeur"], json!("Oui"));
    assert_eq!(c["CASE"]["page"], json!(2));
}

#[test]
fn champs_le_rect_suit_le_cadre_de_la_page_visible() {
    // CropBox [10 20 310 420] dans une MediaBox 595 × 842 : l'origine est le coin haut-gauche du cadre.
    let objets = vec![
        "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [5 0 R] >> >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [0 0 595 842] >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /Contents 4 0 R /CropBox [10 20 310 420] /Annots [5 0 R] >>".to_string(),
        flux("", "q Q\n"),
        "<< /Type /Annot /Subtype /Widget /FT /Tx /T (A) /Rect [50 300 150 320] >>".to_string(),
    ];
    let c = champs(&pdf_a_la_main(&objets, 0));
    // x : 50 - 10 ; y0 = 420 - 320 ; y1 = 420 - 300
    assert_eq!(c["A"]["rect"], json!([40.0, 100.0, 140.0, 120.0]));
}

#[test]
fn champs_un_pdf_sans_formulaire_rend_un_objet_vide() {
    assert_eq!(champs(&octets_fabrique(2)), json!({}));
    let d = dossier_temp("formulaire-sans");
    let p = d.join("sans.pdf");
    std::fs::write(&p, octets_fabrique(1)).unwrap();
    let r = lancer(&["--champs", &chemin(&p)]);
    assert_eq!(r.status.code(), Some(0), "{}", String::from_utf8_lossy(&r.stderr));
    let v: Value = serde_json::from_slice(&r.stdout).unwrap();
    assert_eq!(v, json!({}));
}

#[test]
fn champs_en_ligne_de_commande() {
    let d = dossier_temp("formulaire-champs-cli");
    let p = d.join("f.pdf");
    std::fs::write(&p, formulaire(None)).unwrap();
    let r = lancer(&["--champs", &chemin(&p)]);
    assert_eq!(r.status.code(), Some(0), "{}", String::from_utf8_lossy(&r.stderr));
    let sortie = String::from_utf8(r.stdout).unwrap();
    assert_eq!(sortie.lines().count(), 1, "une ligne de JSON");
    let v: Value = serde_json::from_str(&sortie).unwrap();
    assert_eq!(v["CODE"]["valeur"], json!("Été"));
    // fichier absent : refus d'une ligne qui le nomme ; argument manquant : 2
    let r = lancer(&["--champs", &chemin(&d.join("absent.pdf"))]);
    assert_ne!(r.status.code(), Some(0));
    let r = lancer(&["--champs"]);
    assert_eq!(r.status.code(), Some(2));
}

// ------------------------------------------------------------------------------ --remplir : champs

#[test]
fn remplir_ecrit_v_et_une_apparence_a_la_taille_du_rect() {
    let entree = formulaire(None);
    let sortie = remplir_ok(
        &entree,
        json!({"format": 3, "champs": {"NOM": "Bonjour", "NOTE": "Une note", "CENTRE": "AB", "PARENT.ENFANT": "Paris"}}),
    );
    let doc = charge(&sortie);
    assert_eq!(valeur_pdf(&doc, NOM), b"Bonjour");
    assert_eq!(valeur_pdf(&doc, CENTRE), b"AB");
    // la valeur du champ hiérarchique est portée par le champ (ENFANT), l'apparence par le widget
    assert_eq!(valeur_pdf(&doc, ENFANT), b"Paris");
    assert!(doc.get_dictionary(WIDGET_ENFANT).unwrap().get(b"V").is_err());

    for (w, l, h, texte) in [
        (NOM, 200.0, 16.0, "Bonjour"),
        (NOTE, 100.0, 60.0, "Une note"),
        (CENTRE, 100.0, 16.0, "AB"),
        (WIDGET_ENFANT, 200.0, 20.0, "Paris"),
    ] {
        let (flux, dict) = apparence(&doc, w);
        let b = bbox(&dict);
        assert_eq!(b, vec![0.0, 0.0, l, h], "BBox de {:?}", w);
        assert_eq!(dict.get(b"Subtype").unwrap().as_name().unwrap(), b"Form");
        // le texte en codes de la police embarquée
        assert!(flux.contains(&format!("<{}> Tj", hex(texte))), "{:?} : {}", w, flux);
        assert!(flux.contains("/PPChamp 7 Tf") && flux.contains("0 g"), "{}", flux);
        assert!(flux.starts_with("/Tx BMC") && flux.trim_end().ends_with("EMC"), "{}", flux);
        // la police est dans les ressources de l'apparence
        let res = dict.get(b"Resources").unwrap().as_dict().unwrap();
        assert!(res.get(b"Font").unwrap().as_dict().unwrap().has(b"PPChamp"));
    }

    // la position d'un texte simple : 2 pt du bord gauche, ligne de base (h - corps) / 2 + 0,2 corps
    let (flux, _) = apparence(&doc, NOM);
    assert!(flux.contains("1 0 0 1 2 5.9 Tm"), "{}", flux);

    // /DA et /DR du formulaire ne sont pas touchés
    let acro = doc.get_dictionary(ACROFORM).unwrap();
    assert!(matches!(acro.get(b"DA").unwrap(), Object::String(s, _) if s == b"/Helv 7 Tf 0 g"));
    assert!(acro.get(b"DR").is_ok());
    let nom = doc.get_dictionary(NOM).unwrap();
    assert!(matches!(nom.get(b"DA").unwrap(), Object::String(s, _) if s == b"/Helv 7 Tf 0 g"));

    // lus par --champs : les valeurs reviennent
    let c = champs(&sortie);
    assert_eq!(c["NOM"]["valeur"], json!("Bonjour"));
    assert_eq!(c["PARENT.ENFANT"]["valeur"], json!("Paris"));
    // les champs non écrits, la lecture seule et la case sont inchangés
    assert_eq!(c["CODE"]["valeur"], json!("Été"));
    assert_eq!(c["CASE"]["valeur"], json!("Oui"));
    assert_eq!(c["PEIGNE"]["valeur"], Value::Null);
    assert!(doc.get_dictionary(CODE).unwrap().get(b"AP").is_err());
}

#[test]
fn remplir_le_texte_s_extrait_du_resultat() {
    let d = dossier_temp("formulaire-texte");
    let entree = formulaire(None);
    let sortie = remplir_ok(&entree, json!({"format": 3, "champs": {"NOM": "Dupont Élodie", "CENTRE": "Łukasz"}}));
    let p = d.join("rempli.pdf");
    std::fs::write(&p, &sortie).unwrap();
    let t = textes(&chemin(&p));
    assert_eq!(t.len(), 2);
    assert!(t[0].contains("Dupont Élodie"), "{:?}", t[0]);
    assert!(t[1].contains("Łukasz"), "{:?}", t[1]);
    // la bibliothèque dit la même chose
    let (pages, _) = fabrique_pdf::lecture::texte(&chemin(&p)).unwrap();
    assert!(pages[0].contains("Dupont Élodie"));
    // Łukasz s'écrit en PDFDocEncoding (Ł = 0x95) ; un caractère hors de PDFDocEncoding passe en UTF-16BE
    assert_eq!(valeur_pdf(&charge(&sortie), CENTRE), b"\x95ukasz");
    assert_eq!(champs(&sortie)["CENTRE"]["valeur"], json!("Łukasz"));
    let hongrois = remplir_ok(&formulaire(None), json!({"format": 3, "champs": {"CENTRE": "Őrs"}}));
    let v = valeur_pdf(&charge(&hongrois), CENTRE);
    assert_eq!(&v[..2], &[0xFE, 0xFF]);
    assert_eq!(champs(&hongrois)["CENTRE"]["valeur"], json!("Őrs"));
}

#[test]
fn remplir_une_valeur_en_latin_1_s_ecrit_en_pdfdoc() {
    let sortie = remplir_ok(&formulaire(None), json!({"format": 3, "champs": {"NOM": "Été"}}));
    assert_eq!(valeur_pdf(&charge(&sortie), NOM), b"\xC9t\xE9");
}

#[test]
fn remplir_peigne_un_tm_par_caractere_centre_dans_sa_case() {
    let sortie = remplir_ok(&formulaire(None), json!({"format": 3, "champs": {"PEIGNE": "AB"}}));
    let doc = charge(&sortie);
    let (flux, dict) = apparence(&doc, PEIGNE);
    assert_eq!(bbox(&dict), vec![0.0, 0.0, 40.0, 16.0]);
    assert_eq!(flux.matches(" Tm\n").count(), 2, "{}", flux);
    let l = lignes(&flux);
    assert_eq!(l.len(), 2);
    let case = 40.0 / 2.0;
    for (i, (x, y, t)) in l.iter().enumerate() {
        let c = "AB".chars().nth(i).unwrap();
        assert_eq!(t, &c.to_string());
        // centré : le milieu du caractère est le milieu de sa case
        let milieu = x + largeur(t, 7.0) / 2.0;
        assert!(proche(milieu, i as f64 * case + case / 2.0), "caractère {} : milieu {}", i, milieu);
        assert!(proche(*y, (16.0 - 7.0) / 2.0 + 0.2 * 7.0), "y = {}", y);
    }
    // un seul caractère : il va dans la première case
    let un = remplir_ok(&formulaire(None), json!({"format": 3, "champs": {"PEIGNE": "A"}}));
    let l = lignes(&apparence(&charge(&un), PEIGNE).0);
    assert_eq!(l.len(), 1);
    assert!(proche(l[0].0 + largeur("A", 7.0) / 2.0, 10.0));
}

#[test]
fn remplir_multiligne_coupe_la_valeur_longue_sur_plusieurs_lignes() {
    let longue = "Ceci est une assez longue note qui ne tient pas sur une seule ligne de la case de cent points";
    let sortie = remplir_ok(&formulaire(None), json!({"format": 3, "champs": {"NOTE": longue}}));
    let (flux, _) = apparence(&charge(&sortie), NOTE);
    let l = lignes(&flux);
    assert!(l.len() >= 3, "{} lignes : {:?}", l.len(), l);
    // recollées aux espaces, les lignes redonnent la valeur
    let recolle: Vec<&str> = l.iter().map(|(_, _, t)| t.as_str()).collect();
    assert_eq!(recolle.join(" "), longue);
    for (i, (x, y, t)) in l.iter().enumerate() {
        assert!(proche(*x, 2.0), "x = {}", x);
        // depuis le haut, lignes de 1,116 × corps
        assert!(proche(*y, 60.0 - 2.0 - 1.116 * 7.0 * (i as f64 + 1.0)), "ligne {} : y = {}", i, y);
        // chaque ligne tient à 2 pt des bords
        assert!(largeur(t, 7.0) <= 96.0 + 0.01, "{:?} : {}", t, largeur(t, 7.0));
    }
    // un saut de ligne de la valeur fait une ligne
    let sauts = remplir_ok(&formulaire(None), json!({"format": 3, "champs": {"NOTE": "ab\ncd"}}));
    let l = lignes(&apparence(&charge(&sauts), NOTE).0);
    assert_eq!(l.iter().map(|x| x.2.as_str()).collect::<Vec<_>>(), vec!["ab", "cd"]);
}

#[test]
fn remplir_centre_selon_q() {
    let sortie = remplir_ok(&formulaire(None), json!({"format": 3, "champs": {"CENTRE": "AB"}}));
    let l = lignes(&apparence(&charge(&sortie), CENTRE).0);
    let attendu = (100.0 - largeur("AB", 7.0)) / 2.0;
    assert!(proche(l[0].0, attendu), "x = {}, attendu {}", l[0].0, attendu);
}

#[test]
fn remplir_lukasz_est_accepte_et_un_ideogramme_est_refuse_avec_le_nom_du_champ() {
    let e = formulaire(None);
    remplir_ok(&e, json!({"format": 3, "champs": {"NOM": "Łukasz"}}));
    let m = refus(&e, json!({"format": 3, "champs": {"NOM": "Dupont", "NOTE": "漢字"}}));
    assert!(m.contains("NOTE") && m.contains("Liberation"), "{}", m);
}

#[test]
fn remplir_les_refus_de_champs() {
    let e = formulaire(None);
    let m = refus(&e, json!({"format": 3, "champs": {"INCONNU": "x"}}));
    assert!(m.contains("INCONNU"), "{}", m);
    // un champ en lecture seule : les chiffres de France Travail
    let m = refus(&e, json!({"format": 3, "champs": {"CODE": "x"}}));
    assert!(m.contains("lecture seule") && m.contains("CODE"), "{}", m);
    // un champ qui n'est pas du texte
    let m = refus(&e, json!({"format": 3, "champs": {"CASE": "Off"}}));
    assert!(m.contains("CASE") && m.contains("champ texte"), "{}", m);
    // plus long que /MaxLen
    let m = refus(&e, json!({"format": 3, "champs": {"PEIGNE": "ABC"}}));
    assert!(m.contains("PEIGNE") && m.contains("2"), "{}", m);
    // le nom d'un parent n'est pas celui d'un champ
    refus(&e, json!({"format": 3, "champs": {"PARENT": "x"}}));
    refus(&e, json!({"format": 3, "champs": {"ENFANT": "x"}}));
    // une valeur qui n'est pas une chaîne
    refus(&e, json!({"format": 3, "champs": {"NOM": 12}}));
    // clé inconnue
    let m = refus(&e, json!({"format": 3, "champ": {"NOM": "x"}}));
    assert!(m.contains("champ"), "{}", m);
    // format absent ou autre que 3
    let m = refus(&e, json!({"champs": {"NOM": "x"}}));
    assert!(m.contains("format"), "{}", m);
    for f in [1, 2, 4] {
        let m = refus(&e, json!({"format": f, "champs": {"NOM": "x"}}));
        assert!(m.contains("format"), "{}", m);
    }
    // une description qui n'est pas du JSON
    assert!(fabrique_pdf::formulaire::remplir(&e, "pas du json").is_err());
    // un PDF sans formulaire n'a aucun champ
    let m = refus(&octets_fabrique(1), json!({"format": 3, "champs": {"NOM": "x"}}));
    assert!(m.contains("NOM"), "{}", m);
}

#[test]
fn remplir_un_refus_n_ecrit_rien_en_ligne_de_commande() {
    let d = dossier_temp("formulaire-refus-cli");
    let entree = d.join("f.pdf");
    std::fs::write(&entree, formulaire(None)).unwrap();
    let octets_avant = std::fs::read(&entree).unwrap();
    let cas = [
        json!({"format": 3, "champs": {"NOM": "bon", "CODE": "refusé"}}),
        json!({"format": 3, "champs": {"NOM": "bon"}, "images": [{"page": 9, "rect": [0, 0, 10, 10], "svg": "<svg xmlns='http://www.w3.org/2000/svg' width='10' height='10'/>"}]}),
        json!({"format": 2, "champs": {"NOM": "bon"}}),
        json!({"champs": {"NOM": "bon"}}),
        json!({"format": 3, "bidule": 1}),
    ];
    for (k, c) in cas.iter().enumerate() {
        let sortie = d.join(format!("s{}.pdf", k));
        let r = remplir_cli(&chemin(&entree), &chemin(&sortie), c);
        assert_eq!(r.status.code(), Some(1), "cas {} : {}", k, String::from_utf8_lossy(&r.stderr));
        let erreur = String::from_utf8_lossy(&r.stderr).to_string();
        assert_eq!(erreur.trim_end().lines().count(), 1, "une ligne : {:?}", erreur);
        assert!(erreur.contains("f.pdf"), "le fichier d'entrée est nommé : {:?}", erreur);
        assert!(!sortie.exists(), "cas {} : la sortie existe", k);
    }
    // et sur place : l'entrée reste ce qu'elle était
    let r = remplir_cli(&chemin(&entree), &chemin(&entree), &cas[0]);
    assert_eq!(r.status.code(), Some(1));
    assert_eq!(std::fs::read(&entree).unwrap(), octets_avant);
    // un argument manquant : code 2
    let r = lancer(&["--remplir", &chemin(&entree)]);
    assert_eq!(r.status.code(), Some(2));
}

#[test]
fn remplir_en_ligne_de_commande_ecrit_le_pdf() {
    let d = dossier_temp("formulaire-cli");
    let entree = d.join("f.pdf");
    let sortie = d.join("rempli.pdf");
    std::fs::write(&entree, formulaire(None)).unwrap();
    let r = remplir_cli(&chemin(&entree), &chemin(&sortie), &json!({"format": 3, "champs": {"NOM": "Martin"}}));
    assert_eq!(r.status.code(), Some(0), "{}", String::from_utf8_lossy(&r.stderr));
    let v = lancer(&["--champs", &chemin(&sortie)]);
    let v: Value = serde_json::from_slice(&v.stdout).unwrap();
    assert_eq!(v["NOM"]["valeur"], json!("Martin"));
    assert!(textes(&chemin(&sortie))[0].contains("Martin"));
}

#[test]
fn remplir_sans_champs_ni_images_ne_change_rien_d_autre() {
    let e = formulaire(None);
    let sortie = remplir_ok(&e, json!({"format": 3}));
    assert_eq!(champs(&sortie), champs(&e));
    let doc = charge(&sortie);
    // les pages gardent leur contenu : toujours une seule référence
    assert!(matches!(doc.get_dictionary(PAGE1).unwrap().get(b"Contents").unwrap(), Object::Reference(_)));
}

// ------------------------------------------------------------------------------ --remplir : images

const SVG_TEXTE: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"100\" height=\"40\" viewBox=\"0 0 100 40\">\
    <rect x=\"1\" y=\"1\" width=\"98\" height=\"38\" fill=\"none\" stroke=\"#0000ff\"/>\
    <text x=\"8\" y=\"26\" font-family=\"Helvetica\" font-size=\"18\" fill=\"#0000ff\">Cachet</text></svg>";

fn png(d: &Path, nom: &str, l: u32, h: u32, transparent: bool) -> String {
    let p = d.join(nom);
    if transparent {
        let img = image::RgbaImage::from_fn(l, h, |x, y| {
            image::Rgba([(x * 5) as u8, (y * 9) as u8, 128, if x < 4 { 0 } else { 255 }])
        });
        img.save(&p).unwrap();
    } else {
        let img = image::RgbImage::from_fn(l, h, |x, y| image::Rgb([(x * 5) as u8, (y * 9) as u8, 128]));
        img.save(&p).unwrap();
    }
    chemin(&p)
}

fn jpeg(d: &Path, nom: &str, l: u32, h: u32) -> (String, Vec<u8>) {
    let p: PathBuf = d.join(nom);
    let img = image::RgbImage::from_fn(l, h, |x, y| image::Rgb([(x * 7) as u8, (y * 11) as u8, 200]));
    img.save(&p).unwrap();
    (chemin(&p), std::fs::read(&p).unwrap())
}

/// Ce qu'on attend de `cm` : le plus grand rectangle aux proportions de l'image (`l` × `h`) qui
/// tient dans le `rect` moins la `marge`, centré ; origine du PDF en bas à gauche.
fn attendu_cm(rect: [f64; 4], marge: f64, l: f64, h: f64) -> [f64; 6] {
    let (x0, y0, x1, y1) = (rect[0] + marge, rect[1] + marge, rect[2] - marge, rect[3] - marge);
    let (cl, ch) = (x1 - x0, y1 - y0);
    let k = (cl / l).min(ch / h);
    let (dl, dh) = (l * k, h * k);
    let x = x0 + (cl - dl) / 2.0;
    let haut = y0 + (ch - dh) / 2.0;
    [dl, 0.0, 0.0, dh, x, HAUTEUR - haut - dh]
}

/// Les identifiants du `/Contents` d'une page (toujours un tableau après une pose d'image).
fn contenus(doc: &Document, page: ObjectId) -> Vec<ObjectId> {
    match doc.get_dictionary(page).unwrap().get(b"Contents").unwrap() {
        Object::Array(v) => v.iter().map(|o| o.as_reference().unwrap()).collect(),
        Object::Reference(id) => vec![*id],
        _ => panic!("contenu inattendu"),
    }
}

fn texte_flux(doc: &Document, id: ObjectId) -> String {
    let s = doc.get_object(id).unwrap().as_stream().unwrap();
    String::from_utf8(s.decompressed_content().unwrap_or_else(|_| s.content.clone())).unwrap()
}

/// Le dernier flux de la page (les images) → les `cm` posées, dans l'ordre, et les noms `Do`.
fn poses(doc: &Document, page: ObjectId) -> Vec<([f64; 6], String)> {
    let flux = texte_flux(doc, *contenus(doc, page).last().unwrap());
    let mut out = Vec::new();
    let mut cm: Option<[f64; 6]> = None;
    for l in flux.lines() {
        if let Some(r) = l.strip_suffix(" cm") {
            let v: Vec<f64> = r.split_whitespace().map(|x| x.parse().unwrap()).collect();
            cm = Some([v[0], v[1], v[2], v[3], v[4], v[5]]);
        } else if let Some(n) = l.strip_prefix('/').and_then(|r| r.strip_suffix(" Do")) {
            out.push((cm.take().unwrap(), n.to_string()));
        }
    }
    out
}

/// L'XObject nommé dans les ressources de la page.
fn xobjet(doc: &Document, page: ObjectId, nom: &str) -> (ObjectId, Object) {
    let res = doc.get_dictionary(page).unwrap().get(b"Resources").unwrap().as_dict().unwrap();
    let id = res
        .get(b"XObject")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(nom.as_bytes())
        .unwrap()
        .as_reference()
        .unwrap();
    (id, doc.get_object(id).unwrap().clone())
}

fn verifie_cm(obtenu: [f64; 6], attendu: [f64; 6]) {
    for (a, b) in obtenu.iter().zip(attendu.iter()) {
        assert!(proche(*a, *b), "cm {:?} attendu {:?}", obtenu, attendu);
    }
}

#[test]
fn image_png_avec_transparence_a_un_smask() {
    let d = dossier_temp("formulaire-png-alpha");
    let f = png(&d, "alpha.png", 40, 20, true);
    let rect = [100.0, 100.0, 200.0, 180.0];
    let sortie = remplir_ok(&formulaire(None), json!({"format": 3, "images": [{"page": 1, "rect": rect, "marge": 5, "fichier": f}]}));
    let doc = charge(&sortie);
    let (_, x) = xobjet(&doc, PAGE1, "PPImage1");
    let s = x.as_stream().unwrap();
    assert_eq!(s.dict.get(b"Subtype").unwrap().as_name().unwrap(), b"Image");
    assert_eq!(s.dict.get(b"Filter").unwrap().as_name().unwrap(), b"FlateDecode");
    assert_eq!(s.dict.get(b"ColorSpace").unwrap().as_name().unwrap(), b"DeviceRGB");
    assert_eq!(num(s.dict.get(b"Width").unwrap()), 40.0);
    assert_eq!(num(s.dict.get(b"Height").unwrap()), 20.0);
    let masque = s.dict.get(b"SMask").expect("la transparence passe en /SMask").as_reference().unwrap();
    let m = doc.get_object(masque).unwrap().as_stream().unwrap();
    assert_eq!(m.dict.get(b"ColorSpace").unwrap().as_name().unwrap(), b"DeviceGray");
    let alpha = deflate(&m.content);
    assert_eq!(alpha.len(), 40 * 20);
    assert_eq!(alpha[0], 0);
    assert_eq!(alpha[10], 255);
    // les couleurs : 3 octets par pixel
    assert_eq!(deflate(&s.content).len(), 40 * 20 * 3);
    // 2:1 dans une case de 90 × 70 : 90 × 45, calé à gauche, centré en hauteur
    let p = poses(&doc, PAGE1);
    assert_eq!(p.len(), 1);
    assert_eq!(p[0].1, "PPImage1");
    verifie_cm(p[0].0, attendu_cm(rect, 5.0, 40.0, 20.0));
    verifie_cm(p[0].0, [90.0, 0.0, 0.0, 45.0, 105.0, 679.5]);
}

#[test]
fn image_png_opaque_n_a_pas_de_smask() {
    let d = dossier_temp("formulaire-png-opaque");
    let rgb = png(&d, "rgb.png", 30, 30, false);
    // un PNG RVBA entièrement opaque n'a pas de masque non plus
    let p = d.join("rgba-opaque.png");
    image::RgbaImage::from_fn(10, 10, |x, y| image::Rgba([x as u8, y as u8, 0, 255])).save(&p).unwrap();
    let sortie = remplir_ok(
        &formulaire(None),
        json!({"format": 3, "images": [
            {"page": 2, "rect": [50, 50, 150, 100], "fichier": rgb},
            {"page": 2, "rect": [50, 120, 150, 170], "fichier": chemin(&p)}]}),
    );
    let doc = charge(&sortie);
    for nom in ["PPImage1", "PPImage2"] {
        let (_, x) = xobjet(&doc, PAGE2, nom);
        let s = x.as_stream().unwrap();
        assert!(s.dict.get(b"SMask").is_err(), "{} a un masque", nom);
        assert_eq!(s.dict.get(b"Filter").unwrap().as_name().unwrap(), b"FlateDecode");
    }
    // carré 30 × 30 dans 100 × 50 : 50 × 50, centré
    let p = poses(&doc, PAGE2);
    assert_eq!(p.len(), 2);
    verifie_cm(p[0].0, attendu_cm([50.0, 50.0, 150.0, 100.0], 0.0, 30.0, 30.0));
    verifie_cm(p[0].0, [50.0, 0.0, 0.0, 50.0, 75.0, 742.0]);
    verifie_cm(p[1].0, attendu_cm([50.0, 120.0, 150.0, 170.0], 0.0, 10.0, 10.0));
}

#[test]
fn image_jpeg_rvb_passe_tel_quel() {
    let d = dossier_temp("formulaire-jpeg");
    let (f, octets) = jpeg(&d, "tampon.jpg", 30, 20);
    let rect = [300.0, 200.0, 400.0, 300.0];
    let sortie = remplir_ok(&formulaire(None), json!({"format": 3, "images": [{"page": 1, "rect": rect, "marge": 2, "fichier": f}]}));
    let doc = charge(&sortie);
    let (_, x) = xobjet(&doc, PAGE1, "PPImage1");
    let s = x.as_stream().unwrap();
    assert_eq!(s.dict.get(b"Filter").unwrap().as_name().unwrap(), b"DCTDecode");
    assert_eq!(s.dict.get(b"ColorSpace").unwrap().as_name().unwrap(), b"DeviceRGB");
    assert_eq!(num(s.dict.get(b"Width").unwrap()), 30.0);
    assert_eq!(num(s.dict.get(b"Height").unwrap()), 20.0);
    assert_eq!(s.content, octets, "aucune recompression");
    assert!(s.dict.get(b"SMask").is_err());
    // 3:2 dans 96 × 96 : 96 × 64
    let p = poses(&doc, PAGE1);
    verifie_cm(p[0].0, attendu_cm(rect, 2.0, 30.0, 20.0));
    verifie_cm(p[0].0, [96.0, 0.0, 0.0, 64.0, 302.0, 842.0 - 202.0 - 16.0 - 64.0]);
}

#[test]
fn image_le_type_se_lit_dans_les_octets_pas_dans_l_extension() {
    let d = dossier_temp("formulaire-extension");
    // `save` choisit le format d'après l'extension : on écrit un vrai PNG sous un faux nom
    let vrai = d.join("vrai.dat");
    image::RgbImage::from_fn(8, 8, |x, y| image::Rgb([x as u8, y as u8, 0]))
        .save_with_format(&vrai, image::ImageFormat::Png)
        .unwrap();
    let sortie = remplir_ok(
        &formulaire(None),
        json!({"format": 3, "images": [{"page": 1, "rect": [10, 10, 60, 60], "fichier": chemin(&vrai)}]}),
    );
    let doc = charge(&sortie);
    let (_, x) = xobjet(&doc, PAGE1, "PPImage1");
    assert_eq!(x.as_stream().unwrap().dict.get(b"Filter").unwrap().as_name().unwrap(), b"FlateDecode");
}

#[test]
fn image_svg_avec_du_texte_est_un_objet_formulaire_sans_police() {
    let d = dossier_temp("formulaire-svg");
    let fichier = d.join("cachet.svg");
    std::fs::write(&fichier, SVG_TEXTE).unwrap();
    let rect = [300.0, 100.0, 400.0, 160.0];
    for (k, image) in [json!({"fichier": chemin(&fichier)}), json!({"svg": SVG_TEXTE})].iter().enumerate() {
        let mut im = image.clone();
        im["page"] = json!(1);
        im["rect"] = json!(rect);
        im["marge"] = json!(2);
        let sortie = remplir_ok(&formulaire(None), json!({"format": 3, "images": [im]}));
        let doc = charge(&sortie);
        let (_, x) = xobjet(&doc, PAGE1, "PPImage1");
        let s = x.as_stream().unwrap();
        assert_eq!(s.dict.get(b"Subtype").unwrap().as_name().unwrap(), b"Form", "variante {}", k);
        // du vectoriel : le texte est en tracés, il n'y a pas de police à chercher
        let contenu = String::from_utf8_lossy(&s.decompressed_content().unwrap()).to_string();
        assert!(!contenu.contains("Tj") && !contenu.contains(" Tf"), "{}", contenu);
        if let Ok(res) = s.dict.get(b"Resources").and_then(|r| r.as_dict()) {
            assert!(res.get(b"Font").is_err(), "le SVG ne cite aucune police");
        }
        // 100 × 40 dans 96 × 56 : facteur 0,96
        let p = poses(&doc, PAGE1);
        verifie_cm(p[0].0, attendu_cm(rect, 2.0, 100.0, 40.0));
        verifie_cm(p[0].0, [96.0, 0.0, 0.0, 38.4, 302.0, 842.0 - 110.8 - 38.4]);
    }
}

#[test]
fn image_svg_le_meme_dessin_chaque_fois() {
    let a = remplir_ok(&formulaire(None), json!({"format": 3, "images": [{"page": 1, "rect": [0, 0, 100, 40], "svg": SVG_TEXTE}]}));
    let b = remplir_ok(&formulaire(None), json!({"format": 3, "images": [{"page": 1, "rect": [0, 0, 100, 40], "svg": SVG_TEXTE}]}));
    assert_eq!(a, b);
}

#[test]
fn image_les_proportions_sont_gardees_dans_les_deux_sens() {
    let d = dossier_temp("formulaire-proportions");
    let large = png(&d, "large.png", 60, 10, false);
    let haute = png(&d, "haute.png", 10, 60, false);
    let sortie = remplir_ok(
        &formulaire(None),
        json!({"format": 3, "images": [
            {"page": 1, "rect": [10, 10, 110, 90], "marge": 0, "fichier": large},
            {"page": 1, "rect": [10, 100, 110, 180], "marge": 0, "fichier": haute}]}),
    );
    let p = poses(&charge(&sortie), PAGE1);
    verifie_cm(p[0].0, attendu_cm([10.0, 10.0, 110.0, 90.0], 0.0, 60.0, 10.0));
    verifie_cm(p[0].0, [100.0, 0.0, 0.0, 100.0 / 6.0, 10.0, 842.0 - 10.0 - (80.0 - 100.0 / 6.0) / 2.0 - 100.0 / 6.0]);
    verifie_cm(p[1].0, attendu_cm([10.0, 100.0, 110.0, 180.0], 0.0, 10.0, 60.0));
    // le rapport largeur / hauteur de la pose est celui de l'image
    assert!(proche(p[0].0[0] / p[0].0[3], 6.0) && proche(p[1].0[0] / p[1].0[3], 1.0 / 6.0));
}

#[test]
fn image_le_contenu_ancien_est_intact_et_les_images_viennent_par_dessus() {
    let d = dossier_temp("formulaire-ordre");
    let f = png(&d, "a.png", 10, 10, true);
    let e = formulaire(None);
    let avant = charge(&e);
    let sortie = remplir_ok(
        &e,
        json!({"format": 3, "champs": {"NOM": "Bonjour"}, "images": [
            {"page": 1, "rect": [10, 10, 60, 60], "fichier": f},
            {"page": 1, "rect": [70, 10, 120, 60], "svg": SVG_TEXTE}]}),
    );
    let doc = charge(&sortie);
    let ids = contenus(&doc, PAGE1);
    // q, le flux d'origine (le même objet), les Q qui referment (celui de la page + 1 resté ouvert),
    // puis les images
    assert_eq!(ids.len(), 4);
    assert_eq!(ids[1], (5, 0), "le flux ancien est le même objet");
    assert_eq!(texte_flux(&doc, ids[0]).trim(), "q");
    assert_eq!(texte_flux(&doc, ids[1]), CONTENU_PAGE1);
    assert_eq!(texte_flux(&doc, ids[2]).trim(), "Q\nQ");
    let origine = avant.get_object((5, 0)).unwrap().as_stream().unwrap();
    assert_eq!(doc.get_object((5, 0)).unwrap().as_stream().unwrap().content, origine.content);
    let images = texte_flux(&doc, ids[3]);
    assert!(images.matches(" cm\n").count() == 2 && images.contains("/PPImage1 Do") && images.contains("/PPImage2 Do"), "{}", images);
    assert!(images.starts_with("q\n") && images.trim_end().ends_with('Q'), "{}", images);
    // les ressources héritées (la police F1) sont recopiées dans la page avec les images en plus
    let res = doc.get_dictionary(PAGE1).unwrap().get(b"Resources").unwrap().as_dict().unwrap();
    assert!(res.get(b"Font").unwrap().as_dict().unwrap().has(b"F1"));
    let xo = res.get(b"XObject").unwrap().as_dict().unwrap();
    assert!(xo.has(b"PPImage1") && xo.has(b"PPImage2"));
    // la page 2 n'a pas bougé
    assert!(matches!(doc.get_dictionary(PAGE2).unwrap().get(b"Contents").unwrap(), Object::Reference((6, 0))));
    // le texte de la page et le champ rempli s'extraient toujours
    let p = d.join("sortie.pdf");
    std::fs::write(&p, &sortie).unwrap();
    let t = textes(&chemin(&p));
    assert!(t[0].contains("Page un") && t[0].contains("Bonjour"), "{:?}", t[0]);
    assert!(t[1].contains("Page deux"));
}

#[test]
fn image_une_page_a_une_seule_image_garde_la_cle_xobject_existante() {
    // deux poses successives sur deux pages : chaque page a ses ressources, nommées PPImage1
    let d = dossier_temp("formulaire-deux-pages");
    let f = png(&d, "a.png", 10, 10, false);
    let sortie = remplir_ok(
        &formulaire(None),
        json!({"format": 3, "images": [
            {"page": 1, "rect": [10, 10, 60, 60], "fichier": f},
            {"page": 2, "rect": [10, 10, 60, 60], "fichier": f}]}),
    );
    let doc = charge(&sortie);
    assert_eq!(poses(&doc, PAGE1).len(), 1);
    assert_eq!(poses(&doc, PAGE2).len(), 1);
}

#[test]
fn image_les_refus() {
    let d = dossier_temp("formulaire-image-refus");
    let bon = png(&d, "bon.png", 10, 10, false);
    let e = formulaire(None);
    let une = |im: Value| json!({"format": 3, "images": [im]});

    // une page qui n'existe pas
    let m = refus(&e, une(json!({"page": 5, "rect": [10, 10, 60, 60], "fichier": bon})));
    assert!(m.contains("page 5"), "{}", m);
    refus(&e, une(json!({"page": 0, "rect": [10, 10, 60, 60], "fichier": bon})));
    // un rectangle inversé, ou vide
    let m = refus(&e, une(json!({"page": 1, "rect": [60, 10, 10, 60], "fichier": bon})));
    assert!(m.contains("rect"), "{}", m);
    refus(&e, une(json!({"page": 1, "rect": [10, 60, 60, 10], "fichier": bon})));
    refus(&e, une(json!({"page": 1, "rect": [10, 10, 10, 60], "fichier": bon})));
    refus(&e, une(json!({"page": 1, "rect": [10, 10, 60], "fichier": bon})));
    // une marge qui mange la case
    let m = refus(&e, une(json!({"page": 1, "rect": [10, 10, 60, 60], "marge": 25, "fichier": bon})));
    assert!(m.contains("marge"), "{}", m);
    refus(&e, une(json!({"page": 1, "rect": [10, 10, 60, 60], "marge": -1, "fichier": bon})));
    // un fichier qui n'est pas une image
    let texte = d.join("note.txt");
    std::fs::write(&texte, "ceci n'est pas une image").unwrap();
    let m = refus(&e, une(json!({"page": 1, "rect": [10, 10, 60, 60], "fichier": chemin(&texte)})));
    assert!(m.contains("note.txt"), "{}", m);
    // un fichier absent
    refus(&e, une(json!({"page": 1, "rect": [10, 10, 60, 60], "fichier": chemin(&d.join("absent.png"))})));
    // un PNG tronqué
    let tronque = d.join("tronque.png");
    let octets = std::fs::read(&bon).unwrap();
    std::fs::write(&tronque, &octets[..octets.len() / 2]).unwrap();
    refus(&e, une(json!({"page": 1, "rect": [10, 10, 60, 60], "fichier": chemin(&tronque)})));
    // ni « fichier » ni « svg », ou les deux
    refus(&e, une(json!({"page": 1, "rect": [10, 10, 60, 60]})));
    refus(&e, une(json!({"page": 1, "rect": [10, 10, 60, 60], "fichier": bon, "svg": SVG_TEXTE})));
    // une clé inconnue dans l'image
    let m = refus(&e, une(json!({"page": 1, "rect": [10, 10, 60, 60], "fichier": bon, "angle": 90})));
    assert!(m.contains("angle"), "{}", m);
    // un SVG qui n'en est pas un
    refus(&e, une(json!({"page": 1, "rect": [10, 10, 60, 60], "svg": "<svg>pas fermé"})));
    // « images » n'est pas une liste
    refus(&e, json!({"format": 3, "images": {"page": 1}}));
}

#[test]
fn image_une_page_tournee_est_refusee() {
    let d = dossier_temp("formulaire-rotate");
    let bon = png(&d, "bon.png", 10, 10, false);
    let tournee = formulaire(Some(90));
    // la page 1 n'est pas tournée : elle reçoit l'image
    remplir_ok(&tournee, json!({"format": 3, "images": [{"page": 1, "rect": [10, 10, 60, 60], "fichier": bon}]}));
    // la page 2 l'est : refus, même avec des champs bons à côté
    let m = refus(
        &tournee,
        json!({"format": 3, "champs": {"NOM": "x"}, "images": [{"page": 2, "rect": [10, 10, 60, 60], "fichier": bon}]}),
    );
    assert!(m.contains("page 2") && m.contains("tournée"), "{}", m);
    // 0 et 360 ne sont pas des rotations
    remplir_ok(&formulaire(Some(360)), json!({"format": 3, "images": [{"page": 2, "rect": [10, 10, 60, 60], "fichier": bon}]}));
    refus(&formulaire(Some(-90)), json!({"format": 3, "images": [{"page": 2, "rect": [10, 10, 60, 60], "fichier": bon}]}));
}

#[test]
fn image_dans_le_cadre_d_une_page_decalee() {
    // CropBox [10 20 310 420] : le rect est compté depuis le coin haut-gauche du cadre
    let objets = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [0 0 595 842] >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /Contents 4 0 R /CropBox [10 20 310 420] >>".to_string(),
        flux("", "q Q\n"),
    ];
    let d = dossier_temp("formulaire-cadre");
    let f = png(&d, "a.png", 10, 10, false);
    let sortie = remplir_ok(
        &pdf_a_la_main(&objets, 0),
        json!({"format": 3, "images": [{"page": 1, "rect": [40, 100, 140, 120], "fichier": f}]}),
    );
    let doc = charge(&sortie);
    let p = poses(&doc, (3, 0));
    // 20 × 20 centré dans 100 × 20 : x = 10 + 40 + 40, haut = 100 → y = 420 - 100 - 20
    verifie_cm(p[0].0, [20.0, 0.0, 0.0, 20.0, 90.0, 300.0]);
}

// ------------------------------------------------------------------------------ déterminisme

#[test]
fn deux_appels_identiques_donnent_les_memes_octets() {
    let d = dossier_temp("formulaire-determinisme");
    let f = png(&d, "a.png", 12, 12, true);
    let description = json!({"format": 3,
        "champs": {"NOM": "Bonjour", "PEIGNE": "AB", "NOTE": "Une note assez longue pour passer à la ligne dans la case", "CENTRE": "Łukasz", "PARENT.ENFANT": "Paris"},
        "images": [{"page": 1, "rect": [10, 10, 80, 80], "marge": 3, "fichier": f},
                   {"page": 2, "rect": [10, 10, 130, 70], "svg": SVG_TEXTE}]});
    let e = formulaire(None);
    let a = remplir_ok(&e, description.clone());
    let b = remplir_ok(&e, description.clone());
    assert_eq!(a, b);
    // en ligne de commande aussi
    let entree = d.join("f.pdf");
    std::fs::write(&entree, &e).unwrap();
    let (s1, s2) = (d.join("s1.pdf"), d.join("s2.pdf"));
    assert_eq!(remplir_cli(&chemin(&entree), &chemin(&s1), &description).status.code(), Some(0));
    assert_eq!(remplir_cli(&chemin(&entree), &chemin(&s2), &description).status.code(), Some(0));
    assert_eq!(std::fs::read(&s1).unwrap(), std::fs::read(&s2).unwrap());
    assert_eq!(std::fs::read(&s1).unwrap(), a);
}

#[test]
fn remplir_deux_fois_de_suite_remplace_la_valeur() {
    let e = formulaire(None);
    let une = remplir_ok(&e, json!({"format": 3, "champs": {"NOM": "Premier"}}));
    let deux = remplir_ok(&une, json!({"format": 3, "champs": {"NOM": "Second"}}));
    let doc = charge(&deux);
    assert_eq!(valeur_pdf(&doc, NOM), b"Second");
    let (flux, _) = apparence(&doc, NOM);
    assert!(flux.contains(&hex("Second")) && !flux.contains(&hex("Premier")), "{}", flux);
}

// ------------------------------------------------------------------ ajouter des champs

fn ajouter(o: &[u8], d: Value) -> Result<Vec<u8>, String> {
    fabrique_pdf::formulaire::ajouter_champs(o, &d.to_string()).map_err(|e| e.0)
}

#[test]
fn ajouter_champs_a_un_document_de_la_fabrique_se_relit_et_se_remplit() {
    let base = octets_fabrique(2);
    let d = json!({"format": 3, "champs": [
        {"nom": "IMPRIME", "page": 1, "rect": [60, 100, 160, 116], "valeur": "12/10/2026", "lecture_seule": true, "taille": 9},
        {"nom": "A_REMPLIR", "page": 2, "rect": [60, 200, 260, 216], "taille": 9},
        {"nom": "CENTRE", "page": 1, "rect": [60, 130, 160, 146], "valeur": "2", "alignement": "centre"}
    ]});
    let o = ajouter(&base, d).unwrap();
    let c = champs(&o);
    // Ce que `--champs` relit : la valeur, la lecture seule, la page et le rectangle donnés (origine en haut à gauche).
    assert_eq!(c["IMPRIME"]["valeur"], json!("12/10/2026"));
    assert_eq!(c["IMPRIME"]["lecture_seule"], json!(true));
    assert_eq!(c["IMPRIME"]["page"], json!(1));
    let r: Vec<f64> = c["IMPRIME"]["rect"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
    assert!(proche(r[0], 60.0) && proche(r[1], 100.0) && proche(r[2], 160.0) && proche(r[3], 116.0), "{:?}", r);
    assert_eq!(c["A_REMPLIR"]["valeur"], Value::Null);
    assert_eq!(c["A_REMPLIR"]["lecture_seule"], json!(false));
    assert_eq!(c["A_REMPLIR"]["page"], json!(2));
    // La valeur imprimée a son apparence : elle se lit dans tous les lecteurs.
    let doc = charge(&o);
    let id = doc
        .objects
        .iter()
        .find(|(_, v)| v.as_dict().ok().and_then(|d| d.get(b"T").ok()).and_then(|t| t.as_str().ok()) == Some(&b"IMPRIME"[..]))
        .map(|(id, _)| *id)
        .unwrap();
    let (texte, _) = apparence(&doc, id);
    assert!(texte.contains(&hex("12/10/2026")), "{}", texte);
    // Le champ en lecture seule ne se remplit pas ; l'autre, si.
    assert!(remplir(&o, json!({"format": 3, "champs": {"IMPRIME": "13/10/2026"}})).unwrap_err().contains("lecture seule"));
    let rempli = remplir_ok(&o, json!({"format": 3, "champs": {"A_REMPLIR": "Lyon"}}));
    assert_eq!(champs(&rempli)["A_REMPLIR"]["valeur"], json!("Lyon"));
}

#[test]
fn ajouter_champs_refuse_sans_rien_rendre() {
    let base = octets_fabrique(1);
    let un = |c: Value| ajouter(&base, json!({"format": 3, "champs": [c]})).unwrap_err();
    assert!(un(json!({"nom": "X", "page": 2, "rect": [0, 0, 10, 10], "valeur": "a"})).contains("pas de page 2"));
    assert!(un(json!({"nom": "X", "page": 1, "rect": [10, 0, 0, 10], "valeur": "a"})).contains("rect"));
    assert!(un(json!({"nom": "X", "page": 1, "rect": [0, 0, 10, 10], "lecture_seule": true})).contains("sans valeur"));
    assert!(un(json!({"nom": "X", "page": 1, "rect": [0, 0, 10, 10], "valeur": "漢"})).contains("police"));
    let deux = ajouter(&base, json!({"format": 3, "champs": [
        {"nom": "X", "page": 1, "rect": [0, 0, 10, 10]}, {"nom": "X", "page": 1, "rect": [0, 20, 10, 30]}]}));
    assert!(deux.unwrap_err().contains("déjà un champ"));
    let deja = ajouter(&base, json!({"format": 3, "champs": [{"nom": "X", "page": 1, "rect": [0, 0, 10, 10]}]})).unwrap();
    assert!(ajouter(&deja, json!({"format": 3, "champs": [{"nom": "X", "page": 1, "rect": [0, 20, 10, 30]}]})).unwrap_err().contains("déjà un champ"));
}
