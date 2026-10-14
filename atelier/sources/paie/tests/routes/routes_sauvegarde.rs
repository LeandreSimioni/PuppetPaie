//! `sauvegarde-derniere` et `sauvegarde-rien-n-a-bouge` : ce que la page lit de la sauvegarde, sans reseau et sans rien
//! ecrire (le faux Drive n'est consulte que par la sauvegarde elle-meme). Les routes passent par l'aiguillage, dans le
//! processus. Sur `fictive-3090-petite` (config, aout 2026).

use crate::faux_drive::FauxDrive;
use crate::outils::{compagnie, dossier_de_test, reussi, route_en_processus};
use paie::drive::Drive;
use paie::sauvegarde::{derniere, rien_n_a_bouge, sauvegarder};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn compagnie(nom: &str) -> PathBuf {
    compagnie::saisies("fictive-3090-petite", &["2026-08"], &dossier_de_test("routes_sauvegarde", nom).join("compagnie"))
}

fn reponse(racine: &Path, route: &str, charge: Value) -> Value {
    route_en_processus(racine, route, charge, "2026-10-03")
}

fn appeler(racine: &Path, route: &str, charge: Value) -> Value {
    reussi(&reponse(racine, route, charge)).clone()
}

#[test]
fn derniere_sans_index_dit_jamais() {
    let r = compagnie("sans-index");
    let _ = std::fs::remove_file(r.join("config/.sauvegarde.json"));
    assert_eq!(appeler(&r, "sauvegarde-derniere", json!({})), json!({"quand": "", "fichiers": 0, "racine_drive": ""}));
    assert_eq!(appeler(&r, "sauvegarde-rien-n-a-bouge", json!({})), json!({"rien_n_a_bouge": false}));
}

#[test]
fn derniere_est_ce_que_l_index_porte_les_memes_valeurs_que_la_fonction() {
    let r = compagnie("index");
    std::fs::create_dir_all(r.join("config")).unwrap();
    std::fs::write(r.join("config/.sauvegarde.json"), r#"{"quand": "2026-10-02 18:30", "racine_drive": "PuppetPaie", "dossiers": {}, "fichiers": {"a": {}, "b": {}}}"#).unwrap();
    let route = appeler(&r, "sauvegarde-derniere", json!({}));
    assert_eq!(route, json!({"quand": "2026-10-02 18:30", "fichiers": 2, "racine_drive": "PuppetPaie"}));
    assert_eq!(route, derniere(&r).en_json());
    // Un index qui n'est pas un objet JSON se lit comme absent (la veille ne plante plus).
    std::fs::write(r.join("config/.sauvegarde.json"), "[]").unwrap();
    assert_eq!(appeler(&r, "sauvegarde-derniere", json!({})), json!({"quand": "", "fichiers": 0, "racine_drive": ""}));
}

#[test]
fn rien_n_a_bouge_apres_une_sauvegarde_puis_un_fichier_change() {
    let r = compagnie("rien");
    let mut d = FauxDrive::nouveau();
    sauvegarder(&r, "PuppetPaie", false, Some(&mut d as &mut dyn Drive)).unwrap();
    assert_eq!(appeler(&r, "sauvegarde-rien-n-a-bouge", json!({})), json!({"rien_n_a_bouge": true}));
    assert!(rien_n_a_bouge(&r, "PuppetPaie"));
    let derniere = appeler(&r, "sauvegarde-derniere", json!({}));
    assert_eq!(derniere["racine_drive"], json!("PuppetPaie"));
    assert!(derniere["fichiers"].as_u64().unwrap() > 10, "{}", derniere);
    assert_eq!(derniere["quand"].as_str().unwrap().len(), 16);
    // Un fichier ecrit apres : non. La route ne depend pas du reseau : le faux Drive n'est plus consulte.
    let appels = d.journal.len();
    std::fs::write(r.join("config/note-de-test.txt"), "nouveau").unwrap();
    assert_eq!(appeler(&r, "sauvegarde-rien-n-a-bouge", json!({})), json!({"rien_n_a_bouge": false}));
    assert_eq!(d.journal.len(), appels);
}

#[test]
fn rien_n_a_bouge_regarde_la_racine_du_drive_de_la_charge() {
    let r = compagnie("racine");
    let mut d = FauxDrive::nouveau();
    sauvegarder(&r, "PuppetPaie", false, Some(&mut d as &mut dyn Drive)).unwrap();
    assert_eq!(appeler(&r, "sauvegarde-rien-n-a-bouge", json!({"racine_drive": "PuppetPaie"})), json!({"rien_n_a_bouge": true}));
    // Une autre racine du Drive n'est pas « a jour ».
    assert_eq!(appeler(&r, "sauvegarde-rien-n-a-bouge", json!({"racine_drive": "Autre"})), json!({"rien_n_a_bouge": false}));
    assert_eq!(appeler(&r, "sauvegarde-rien-n-a-bouge", json!({"racine_drive": null})), json!({"rien_n_a_bouge": true}));
    // Une racine qui n'est pas un texte est refusee.
    assert_eq!(reponse(&r, "sauvegarde-rien-n-a-bouge", json!({"racine_drive": 5}))["ok"], json!(false));
    let e = reponse(&r, "sauvegarde-rien-n-a-bouge", json!({"racine_drive": ""}));
    assert_eq!(e["ok"], json!(false));
}

#[test]
fn les_routes_ne_touchent_pas_au_disque_en_ecriture() {
    let r = compagnie("lecture-seule");
    let avant = std::fs::read_dir(r.join("config")).unwrap().count();
    appeler(&r, "sauvegarde-derniere", json!({}));
    appeler(&r, "sauvegarde-rien-n-a-bouge", json!({}));
    assert_eq!(std::fs::read_dir(r.join("config")).unwrap().count(), avant);
    assert!(!r.join("config/.sauvegarde.json").exists(), "une route de lecture ne cree pas l'index");
}
