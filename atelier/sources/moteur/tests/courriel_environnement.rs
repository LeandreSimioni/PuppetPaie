//! L'adresse et le secret de la boite courriel lus dans l'environnement (`COURRIEL_COMPTE`, `COURRIEL_MOTDEPASSE`) : le
//! seul test de `courriel` qui le touche, d'ou ce fichier seul. Aucune connexion.

use std::path::PathBuf;

use moteur::boite as courriel;
use serde_json::json;

fn racine(nom: &str) -> PathBuf {
    let r = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("courriel_environnement").join(nom);
    let _ = std::fs::remove_dir_all(&r);
    std::fs::create_dir_all(r.join("config")).unwrap();
    r
}

#[test]
fn adresse_disponible_et_verifier_sans_secret() {
    let r = racine("secret");
    std::env::remove_var("COURRIEL_COMPTE");
    std::env::remove_var("COURRIEL_MOTDEPASSE");
    assert_eq!(courriel::adresse(&r).unwrap(), "");
    assert!(!courriel::disponible(&r));
    // Sans adresse : un refus lisible, pas de connexion.
    let v = courriel::verifier(&r).unwrap();
    assert_eq!(v["ok"], json!(false));
    assert!(v["message"].as_str().unwrap().contains("Aucune adresse"));
    // Une adresse sans secret : disponible est faux, et verifier s'arrete AVANT toute connexion.
    std::env::set_var("COURRIEL_COMPTE", "  Compagnie@Gmail.COM ");
    assert_eq!(courriel::adresse(&r).unwrap(), "compagnie@gmail.com");
    assert!(!courriel::disponible(&r));
    let v = courriel::verifier(&r).unwrap();
    assert_eq!(v["ok"], json!(false));
    assert!(v["message"].as_str().unwrap().starts_with("Aucun mot de passe de boite courriel"), "{}", v);
    assert_eq!(v["conseil"], json!(""));
    // Adresse et secret : disponible (le secret n'est pas lu ici).
    std::env::set_var("COURRIEL_MOTDEPASSE", "abcd efgh ijkl mnop");
    assert!(courriel::disponible(&r));
    std::env::remove_var("COURRIEL_COMPTE");
    std::env::remove_var("COURRIEL_MOTDEPASSE");
}
