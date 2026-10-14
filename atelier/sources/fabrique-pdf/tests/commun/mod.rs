//! Ce que les tests des modes de lecture et de retouche de PDF (`lecture`, `marquage`,
//! `verrouillage`) ont en commun : un dossier temporaire, le lancement du programme, un PDF
//! fabriqué par la fabrique, un PDF écrit à la main (comme un outil tiers le ferait).
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

pub fn dossier_temp(nom: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(nom);
    std::fs::create_dir_all(&d).unwrap();
    d
}

pub fn chemin(p: &Path) -> String {
    p.to_str().unwrap().replace('\\', "/")
}

pub fn lancer(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_fabrique-pdf")).args(args).output().unwrap()
}

/// Les octets d'un PDF de `n` pages fabriqué par la fabrique : une ligne « Page k éàç œ » par page.
pub fn octets_fabrique(n: usize) -> Vec<u8> {
    let documents: Vec<Value> = (1..=n)
        .map(|k| json!({"blocs": [{"type": "paragraphe", "morceaux": [{"texte": format!("Page {} éàç œ", k)}]}]}))
        .collect();
    let desc = json!({
        "format": 1,
        "page": {"largeur": 400.0, "hauteur": 500.0,
                 "marges": {"haut": 50.0, "droite": 50.0, "bas": 50.0, "gauche": 50.0}},
        "police": {"taille": 10.0, "interligne": 1.25, "couleur": "#000000"},
        "documents": documents
    });
    fabrique_pdf::fabriquer(&desc.to_string()).unwrap_or_else(|e| panic!("{}", e.0))
}

/// Le même PDF, écrit dans `dossier/nom` ; rend son chemin.
pub fn pdf_fabrique(dossier: &Path, nom: &str, n: usize) -> String {
    let p = dossier.join(nom);
    std::fs::write(&p, octets_fabrique(n)).unwrap();
    chemin(&p)
}

/// Un flux : son dictionnaire (sans `/Length`) et son contenu.
pub fn flux(dict: &str, contenu: &str) -> String {
    format!("<< {} /Length {} >>\nstream\n{}\nendstream", dict, contenu.len(), contenu)
}

/// Un PDF écrit à la main : `objets[k]` est le corps de l'objet `k + 1`, l'objet 1 est la racine.
/// `decalage` fausse la table de références (les positions de tous les objets, d'autant d'octets),
/// comme le fait « Microsoft Print To PDF ».
pub fn pdf_a_la_main(objets: &[String], decalage: usize) -> Vec<u8> {
    let mut o: Vec<u8> = b"%PDF-1.7\n\n".to_vec();
    let mut positions = Vec::new();
    for (k, corps) in objets.iter().enumerate() {
        positions.push(o.len() - decalage);
        o.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", k + 1, corps).as_bytes());
    }
    let debut = o.len();
    o.extend_from_slice(format!("xref\n0 {}\r\n0000000000 65535 f\r\n", objets.len() + 1).as_bytes());
    for p in positions {
        o.extend_from_slice(format!("{:010} 00000 n\r\n", p).as_bytes());
    }
    o.extend_from_slice(
        format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n", objets.len() + 1, debut).as_bytes(),
    );
    o
}

/// Le texte de chaque page d'un PDF, par `fabrique-pdf --texte`.
pub fn textes(chemin_pdf: &str) -> Vec<String> {
    let r = lancer(&["--texte", chemin_pdf]);
    assert_eq!(r.status.code(), Some(0), "{}", String::from_utf8_lossy(&r.stderr));
    serde_json::from_slice(&r.stdout).unwrap()
}
