//! Tests de l'assemblage (`FORMAT.md`, « L'assemblage ») : les PDF à fusionner sont fabriqués par
//! la fabrique elle-même, puis relus avec lopdf (nombre de pages, texte page par page).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::io::Write;

use lopdf::content::Content;
use lopdf::{Document, Object};
use serde_json::{json, Value};

fn dossier_temp(nom: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(nom);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn chemin(p: &Path) -> String {
    p.to_str().unwrap().replace('\\', "/")
}

/// Un PDF d'une page par texte, écrit dans `dossier` ; rend son chemin.
fn source(dossier: &Path, nom: &str, textes: &[&str]) -> String {
    let documents: Vec<Value> = textes
        .iter()
        .map(|t| json!({"blocs": [{"type": "paragraphe", "morceaux": [{"texte": t}]}]}))
        .collect();
    let desc = json!({
        "format": 1,
        "page": {"largeur": 400.0, "hauteur": 500.0,
                 "marges": {"haut": 50.0, "droite": 50.0, "bas": 50.0, "gauche": 50.0}},
        "police": {"taille": 10.0, "interligne": 1.25, "couleur": "#000000"},
        "documents": documents
    });
    let octets = fabrique_pdf::fabriquer(&desc.to_string()).unwrap_or_else(|e| panic!("{}", e.0));
    let p = dossier.join(nom);
    std::fs::write(&p, octets).unwrap();
    chemin(&p)
}

fn assembler(v: &Value) -> Result<Vec<u8>, String> {
    fabrique_pdf::assembler(&v.to_string()).map_err(|r| {
        assert!(!r.0.contains('\n'), "le message doit tenir sur une ligne : {:?}", r.0);
        r.0
    })
}

/// Le texte de chaque page : les chaînes des opérateurs Tj / TJ, mises bout à bout.
fn textes(octets: &[u8]) -> Vec<String> {
    let doc = Document::load_mem(octets).unwrap();
    doc.get_pages()
        .values()
        .map(|id| {
            let flux = doc.get_page_content(*id).unwrap();
            let contenu = Content::decode(&flux).unwrap();
            let mut t = String::new();
            for op in contenu.operations {
                if op.operator == "Tj" || op.operator == "TJ" {
                    for o in &op.operands {
                        match o {
                            Object::String(s, _) => t.extend(caracteres(s)),
                            Object::Array(a) => {
                                for x in a {
                                    if let Object::String(s, _) = x {
                                        t.extend(caracteres(s));
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            t
        })
        .collect()
}

#[test]
fn les_pages_se_suivent_dans_l_ordre_des_pieces() {
    let d = dossier_temp("assemblage-ordre");
    let a = source(&d, "a.pdf", &["AAA1", "AAA2", "AAA3"]);
    let b = source(&d, "b.pdf", &["BBB1"]);
    let sortie = assembler(&json!({"format": 1, "pieces": [{"fichier": b}, {"fichier": a}]})).unwrap();
    let t = textes(&sortie);
    assert_eq!(t.len(), 4);
    for (page, attendu) in t.iter().zip(["BBB1", "AAA1", "AAA2", "AAA3"]) {
        assert!(page.contains(attendu), "{:?} ne contient pas {}", page, attendu);
    }
}

#[test]
fn les_pages_choisies_sont_prises_dans_l_ordre_demande_et_peuvent_revenir() {
    let d = dossier_temp("assemblage-pages");
    let a = source(&d, "a.pdf", &["AAA1", "AAA2", "AAA3"]);
    let sortie = assembler(&json!({"pieces": [{"fichier": a, "pages": [3, 1, 1]}]})).unwrap();
    let t = textes(&sortie);
    assert_eq!(t.len(), 3);
    assert!(t[0].contains("AAA3") && t[1].contains("AAA1") && t[2].contains("AAA1"), "{:?}", t);
}

#[test]
fn le_resultat_est_un_pdf_lisible_dont_les_pages_pendent_a_un_seul_arbre() {
    let d = dossier_temp("assemblage-arbre");
    let a = source(&d, "a.pdf", &["AAA1", "AAA2"]);
    let sortie = assembler(&json!({"pieces": [{"fichier": a}, {"fichier": a}]})).unwrap();
    let doc = Document::load_mem(&sortie).unwrap();
    assert_eq!(doc.get_pages().len(), 4);
    let racine = doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let pages = doc.get_dictionary(racine).unwrap().get(b"Pages").unwrap().as_reference().unwrap();
    for (_, id) in doc.get_pages() {
        let p = doc.get_dictionary(id).unwrap();
        assert_eq!(p.get(b"Parent").unwrap().as_reference().unwrap(), pages);
        assert!(p.get(b"MediaBox").is_ok() && p.get(b"Resources").is_ok(), "attributs hérités perdus");
    }
}

#[test]
fn les_objets_identiques_ne_sont_ecrits_qu_une_fois() {
    // Quatre copies du même fichier : la police embarquée (un flux de plusieurs dizaines de
    // kilo-octets) ne doit pas être écrite quatre fois.
    let d = dossier_temp("assemblage-doublons");
    let a = source(&d, "a.pdf", &["AAA1"]);
    let une = assembler(&json!({"pieces": [{"fichier": a}]})).unwrap();
    let quatre = assembler(&json!({"pieces": [{"fichier": a}, {"fichier": a}, {"fichier": a}, {"fichier": a}]})).unwrap();
    assert_eq!(textes(&quatre).len(), 4);
    assert!(quatre.len() < 2 * une.len(), "{} octets pour une copie, {} pour quatre", une.len(), quatre.len());
}

#[test]
fn meme_description_memes_octets() {
    let d = dossier_temp("assemblage-determinisme");
    let a = source(&d, "a.pdf", &["AAA1", "AAA2"]);
    let b = source(&d, "b.pdf", &["BBB1"]);
    let desc = json!({"format": 1, "pieces": [{"fichier": a, "pages": [2]}, {"fichier": b}, {"fichier": a}]});
    assert_eq!(assembler(&desc).unwrap(), assembler(&desc).unwrap());
}

#[test]
fn les_refus_tiennent_en_une_ligne() {
    let d = dossier_temp("assemblage-refus");
    let a = source(&d, "a.pdf", &["AAA1"]);
    let faux = d.join("faux.pdf");
    std::fs::write(&faux, b"ceci n'est pas un PDF").unwrap();
    let cas = [
        (json!({"pieces": []}), "aucune pièce"),
        (json!({"format": 2, "pieces": [{"fichier": a}]}), "format 2"),
        (json!({"pieces": [{"fichier": a, "pages": [2]}]}), "page 2 n'existe pas"),
        (json!({"pieces": [{"fichier": a, "pages": [0]}]}), "page 0 n'existe pas"),
        (json!({"pieces": [{"fichier": a, "pages": []}]}), "aucune page"),
        (json!({"pieces": [{"fichier": chemin(&d.join("absent.pdf"))}]}), "pièce 1"),
        (json!({"pieces": [{"fichier": chemin(&faux)}]}), "pas un PDF"),
        (json!({"pieces": [{"fichier": a, "rotation": 90}]}), "illisible"),
    ];
    for (desc, mot) in cas {
        let e = assembler(&desc).expect_err(&format!("{} aurait dû être refusé", desc));
        assert!(e.contains(mot), "{:?} ne contient pas {:?}", e, mot);
    }
}

#[test]
fn le_programme_n_ecrit_rien_quand_il_refuse() {
    let d = dossier_temp("assemblage-programme");
    let a = source(&d, "a.pdf", &["AAA1"]);
    let exe = env!("CARGO_BIN_EXE_fabrique-pdf");
    let sortie = d.join("sortie.pdf");
    let _ = std::fs::remove_file(&sortie);
    let lancer = |desc: &Value| {
        let mut p = Command::new(exe)
            .arg("--assembler")
            .arg(&sortie)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        p.stdin.take().unwrap().write_all(desc.to_string().as_bytes()).unwrap();
        p.wait_with_output().unwrap()
    };
    let r = lancer(&json!({"pieces": [{"fichier": a, "pages": [9]}]}));
    assert_eq!(r.status.code(), Some(1));
    let ligne = String::from_utf8(r.stderr).unwrap();
    assert!(ligne.trim().lines().count() == 1 && ligne.contains("page 9"), "{:?}", ligne);
    assert!(!sortie.exists(), "rien ne doit être écrit en cas de refus");
    let r = lancer(&json!({"pieces": [{"fichier": a}]}));
    assert_eq!(r.status.code(), Some(0));
    assert_eq!(textes(&std::fs::read(&sortie).unwrap()).len(), 1);
    // l'aide nomme le mode
    let aide = Command::new(exe).arg("--aide").output().unwrap();
    assert!(String::from_utf8(aide.stdout).unwrap().contains("--assembler"));
}

/// Le texte d'une chaîne du PDF : deux octets par caractère (le point de code Unicode), la police
/// étant embarquée en sous-ensemble.
fn caracteres(octets: &[u8]) -> Vec<char> {
    octets
        .chunks(2)
        .map(|c| char::from_u32(((c[0] as u32) << 8) | c[1] as u32).unwrap())
        .collect()
}
