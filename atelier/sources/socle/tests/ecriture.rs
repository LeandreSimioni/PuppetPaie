//! L'ecriture des fichiers (`ecriture`) et le verrou des compteurs (`verrou`). Un JSON de la compagnie s'ecrit en LF,
//! avec un saut final, ses dossiers crees ; le verrou des compteurs n'est rendu que par celui qui le tient, et un
//! verrou abandonne se reprend.

use socle::ecriture::ecrire_json;
use socle::verrou;
use serde_json::json;
use std::path::PathBuf;

fn dossier(nom: &str) -> PathBuf {
    let d = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("ecriture").join(nom);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn ecrire_json_en_lf_avec_saut_final_et_dossiers() {
    let d = dossier("json");
    let f = d.join("a/b/c.json");
    ecrire_json(&f, &json!({"é": [1, 2.0]}), 2).unwrap();
    assert_eq!(std::fs::read(&f).unwrap(), "{\n  \"é\": [\n    1,\n    2.0\n  ]\n}\n".as_bytes());
}

#[test]
fn le_verrou_se_prend_se_refuse_se_rend() {
    let d = dossier("verrou");
    let p = d.join(".compteurs.verrou");
    assert_eq!(verrou::prendre(&p, "moi pid 1 x", 600).unwrap(), Ok(()));
    assert_eq!(std::fs::read_to_string(&p).unwrap(), "moi pid 1 x");
    let refus = verrou::prendre(&p, "autre pid 2 y", 600).unwrap().unwrap_err();
    assert_eq!(refus.detenteur, "moi pid 1 x");
    // Le sortant ne retire que le sien.
    verrou::rendre(&p, "autre pid 2 y");
    assert!(p.exists());
    verrou::rendre(&p, "moi pid 1 x");
    assert!(!p.exists());
}

#[test]
fn un_verrou_perime_est_repris() {
    let d = dossier("perime");
    let p = d.join(".compteurs.verrou");
    std::fs::write(&p, "mort pid 9 z").unwrap();
    // Delai nul : tout verrou existant est deja abandonne.
    assert_eq!(verrou::prendre(&p, "moi pid 1 x", 0).unwrap(), Ok(()));
    assert_eq!(std::fs::read_to_string(&p).unwrap(), "moi pid 1 x");
}
