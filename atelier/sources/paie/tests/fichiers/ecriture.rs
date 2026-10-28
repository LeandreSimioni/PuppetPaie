//! L'ecriture des fichiers (`ecriture`) et le verrou des compteurs (`verrou`). La
//! forme des JSON de la compagnie (indentation 2, accents tels quels, flottants ecrits au plus court) est figee : `prive/`
//! est commite, et la changer ferait un diff sur tous ses fichiers deja ecrits.

use socle::ecriture::{ecrire_json, json_indente, repr_flottant};
use socle::verrou;
use serde_json::{json, Value};
use std::path::PathBuf;

fn dossier(nom: &str) -> PathBuf {
    crate::outils::dossier_de_test("ecriture", nom)
}

// ------------------------------------------------------------------ JSON

#[test]
fn les_flottants_gardent_l_ecriture_des_fichiers_deja_ecrits() {
    for (x, attendu) in [
        (1e16, "1e+16"),
        (1e-5, "1e-05"),
        (100.0, "100.0"),
        (0.0001, "0.0001"),
        (1.5e300, "1.5e+300"),
        (5e-324, "5e-324"),
        (-0.0, "-0.0"),
        (0.0, "0.0"),
        (1234567890123456.0, "1234567890123456.0"),
        (2.5e-7, "2.5e-07"),
        (0.1, "0.1"),
        (9999999999999998.0, "9999999999999998.0"),
        (123456789012345678.0, "1.2345678901234568e+17"),
        (-1.5, "-1.5"),
        // Valeur exactement entre deux ecritures : la paire.
        (1872106132238403.25, "1872106132238403.2"),
        (3663627176240.15625, "3663627176240.1562"),
        (1.7976931348623157e308, "1.7976931348623157e+308"),
    ] {
        assert_eq!(repr_flottant(x), attendu);
    }
}

#[test]
fn la_forme_du_fichier() {
    let v: Value = serde_json::from_str(
        r#"{"a": [1, 2.5, {"b": null}], "é": "x\u0001\n\\\"/\u007f ", "e": [], "f": {},
            "g": [1e16, 1e-5, 100.0, 9223372036854775808, -9223372036854775808, 18446744073709551615]}"#,
    )
    .unwrap();
    let attendu = "{\n  \"a\": [\n    1,\n    2.5,\n    {\n      \"b\": null\n    }\n  ],\n  \"é\": \"x\\u0001\\n\\\\\\\"/\u{7f}\u{2028}\",\n  \"e\": [],\n  \"f\": {},\n  \"g\": [\n    1e+16,\n    1e-05,\n    100.0,\n    9223372036854775808,\n    -9223372036854775808,\n    18446744073709551615\n  ]\n}";
    assert_eq!(json_indente(&v, 2), attendu);
    assert_eq!(json_indente(&json!([[], {}]), 1), "[\n [],\n {}\n]");
    assert_eq!(json_indente(&json!({"a": [1, {"b": 2}]}), 0), "{\n\"a\": [\n1,\n{\n\"b\": 2\n}\n]\n}");
}

#[test]
fn ecrire_json_en_lf_avec_saut_final_et_dossiers() {
    let d = dossier("json");
    let f = d.join("a/b/c.json");
    ecrire_json(&f, &json!({"é": [1, 2.0]}), 2).unwrap();
    assert_eq!(std::fs::read(&f).unwrap(), "{\n  \"é\": [\n    1,\n    2.0\n  ]\n}\n".as_bytes());
}

// ---------------------------------------------------------------- verrou

#[test]
fn le_verrou_se_prend_se_refuse_se_rend() {
    let d = dossier("verrou");
    let p = d.join(".compteurs.verrou");
    assert_eq!(verrou::prendre(&p, "moi pid 1 x", 600).unwrap(), Ok(()));
    assert_eq!(std::fs::read_to_string(&p).unwrap(), "moi pid 1 x");
    let refus = verrou::prendre(&p, "autre pid 2 y", 600).unwrap().unwrap_err();
    assert_eq!(refus.detenteur, "moi pid 1 x");
    assert!(refus.message().starts_with("Une autre production tient les compteurs depuis "));
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

#[test]
fn le_verrou_se_rend_en_sortant() {
    let d = dossier("portee");
    let p = d.join(".compteurs.verrou");
    {
        let _v = verrou::Verrou::prendre(&p, "moi pid 1 x", 600).unwrap().unwrap();
        assert!(p.exists());
    }
    assert!(!p.exists());
}

#[test]
fn sans_dossier_une_erreur_pas_un_verrou_tenu() {
    let d = dossier("sansdossier");
    assert!(verrou::prendre(&d.join("config/.compteurs.verrou"), "m", 600).is_err());
}
