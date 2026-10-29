//! LE BRIDAGE PAR DEFAUT (le logiciel n'est jamais debloque par defaut : bride tant que rien ne dit qu'il est
//! debloque) : `moteur/src/compagnie/bridage.rs`, sur des installations de test.
//!
//! - une installation neuve (une archive de la Release decompressee : `logiciel/` sans `EDITION.json`, rien a cote) est
//!   bridee, et le geste de l'accueil la debloque (`.edition.json`), puis la rebride ;
//! - seul `.edition.json` « debloque » debloque : un `EDITION.json` qui ne dit rien de soude ne debloque pas ;
//! - le paquet soude (`logiciel/EDITION.json`, `"edition": "bride"`) l'emporte toujours, et le geste n'y fait rien ;
//! - l'accueil bride ne propose pas les compagnies reelles de l'installation, debloque si.
//! - le branchement : une fictive livree joue ses envois, une fictive ajoutee recoit le message, toute autre envoie.

use std::path::{Path, PathBuf};

use moteur::compagnie::bridage::{est_bride_de, paquet_soude_de};
use moteur::interface::compagnies::liste_de;

use crate::outils::dossier_de_test;

/// Une installation telle qu'une archive la pose : `logiciel/` (sans `EDITION.json`), rien a cote.
fn installation(nom: &str) -> PathBuf {
    let d = dossier_de_test("bridage", nom);
    std::fs::create_dir_all(d.join("logiciel")).unwrap();
    std::fs::write(d.join("logiciel").join("VERSION"), "2026.10.22\n").unwrap();
    d
}

fn geste(inst: &Path, mot: &str) {
    std::fs::write(inst.join(".edition.json"), format!("{{\"edition\": \"{}\"}}", mot)).unwrap();
}

#[test]
fn sans_rien_qui_dise_debloque_le_logiciel_est_bride() {
    let inst = installation("neuve");
    assert!(!inst.join("logiciel/EDITION.json").exists() && !inst.join(".edition.json").exists());
    assert!(est_bride_de(&inst), "une archive decompressee dans un dossier neuf s'ouvre bridee");
    assert!(!paquet_soude_de(&inst));
}

#[test]
fn seul_le_geste_debloque() {
    let inst = installation("seul-le-geste");
    geste(&inst, "debloque");
    assert!(!est_bride_de(&inst), "un .edition.json « debloque »");
    geste(&inst, "complete");
    assert!(!est_bride_de(&inst), "le mot d'avant le lot U1");
    geste(&inst, "bride");
    assert!(est_bride_de(&inst));
    std::fs::write(inst.join(".edition.json"), "pas du json").unwrap();
    assert!(est_bride_de(&inst), "un .edition.json illisible ne debloque pas");
    std::fs::remove_file(inst.join(".edition.json")).unwrap();
    // L'ancienne cle `defaut` de l'installeur ne se lit plus : elle ne debloque rien.
    std::fs::write(inst.join("logiciel").join("EDITION.json"), r#"{"defaut": "debloque"}"#).unwrap();
    assert!(est_bride_de(&inst), "seul .edition.json debloque");
}

#[test]
fn l_accueil_bride_ne_propose_que_les_fictives() {
    let inst = installation("accueil");
    for c in ["compagnies/essai", "fictives/fictive-essai"] {
        crate::outils::ecrire(&inst.join(c), "config/entreprise.json", &serde_json::json!({"nom_affichage": c}));
    }
    let reelles = |inst: &Path| liste_de(inst).into_iter().filter(|c| c.reelle).count();
    assert_eq!(reelles(&inst), 0, "bride par defaut : pas de compagnie reelle");
    assert_eq!(liste_de(&inst).len(), 1, "la fictive");
    geste(&inst, "debloque");
    assert_eq!(reelles(&inst), 1, "debloque par le geste : la compagnie reelle");
}

