//! `paie route` : `contrats-en-cours`, `contrats-passes`, `emploi`, `rubriques-actives` et `metiers-catalogue`. Sur la
//! configuration de `fictive-3090-petite` et les mois que chaque test ecrit ; le code livre est copie sans ses baremes :
//! aucune de ces routes ne calcule de paie.

use crate::outils::{compagnie, dossier_de_test, ecrire, lire, reussi, route};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn compagnie(nom: &str) -> PathBuf {
    let racine = compagnie::saisies("fictive-3090-petite", &[], &dossier_de_test("routes_contrats", nom).join("compagnie"));
    compagnie::avec_le_code(&racine, true);
    racine
}

fn resultat(c: &Path, nom: &str, charge: Value) -> Value {
    reussi(&route(c, nom, charge, "2026-10-04")).clone()
}

/// La phrase d'un refus.
fn refus(c: &Path, nom: &str, charge: Value) -> String {
    crate::outils::refus(&route(c, nom, charge, "2026-10-04")).1
}

fn ecrire_mois(c: &Path, mois: &str, engagements: Value) {
    crate::outils::poser_saisie(c, &mois, &json!({"mois": mois, "taux_cachet": 130.0, "date_paiement": format!("{}-28", mois), "engagements": engagements}));
}

/// Un contrat de septembre qui court jusqu'au 10 octobre, trois jours de planning.
fn contrat_a_cheval() -> Value {
    json!({
        "salarie": "BRIOCHE", "spectacle": "ubu-roi", "fin": "2026-10-10", "numero": "BR1.26.01", "metier": "COM010",
        "lignes": [{"rubrique": "cachet", "quantite": 2, "taux": 130.0, "lieu": "Tours",
                    "dates": ["2026-09-28", "2026-09-29"], "planning": ["2026-10-01", "2026-10-05", "2026-10-10"]}],
    })
}

// ================================================ contrats-passes

#[test]
fn les_contrats_passes_lisent_les_fichiers_de_paie_puis_l_historique() {
    let c = &compagnie("passes");
    let mut autre = contrat_a_cheval();
    autre["salarie"] = json!("SERAPHIN");
    autre["lignes"][0]["dates"] = json!(["2026-09-28"]);
    autre["lignes"][0]["quantite"] = json!(1);
    ecrire_mois(c, "2026-09", json!([contrat_a_cheval(), autre]));
    ecrire(c, 
        "historique/engagements.json",
        &json!({"engagements": [
            // Declare avant ce logiciel, sur un mois que nos fichiers ne couvrent pas.
            {"salarie": "LEMERCIER", "debut": "2026-05-04", "fin": "2026-05-06", "numero_objet": "256Z00920022",
             "heures": 14, "cachets": 3, "jours": 3, "brut": "300"},
            // Sur un mois que nos fichiers couvrent : il se tait.
            {"salarie": "LEMERCIER", "debut": "2026-09-14", "fin": "2026-09-15", "numero_objet": "256Z00920022", "cachets": 2},
        ]}),
    );
    let r = resultat(c, "contrats-passes", json!({"mois": "2026-09"}));
    let l = r["contrats"].as_array().unwrap();
    assert_eq!(l.len(), 2, "{}", r);
    // Tries par salarie puis debut.
    assert_eq!((l[0]["salarie"].as_str(), l[1]["salarie"].as_str()), (Some("BRIOCHE"), Some("SERAPHIN")));
    assert_eq!(l[0]["source"], "paie");
    assert_eq!((l[0]["debut"].as_str(), l[0]["fin"].as_str()), (Some("2026-09-28"), Some("2026-10-10")));
    assert_eq!((l[0]["cachets"].as_i64(), l[0]["jours"].as_i64(), l[0]["heures"].as_i64()), (Some(2), Some(2), Some(0)));
    assert_eq!(l[0]["titre"], "Ubu roi");
    assert_eq!(l[0]["numero_objet"], "256Z00920011");
    // Seuls les jours JOUES du mois demande.
    assert_eq!(l[0]["dates"], json!(["2026-09-28", "2026-09-29"]));
    let mai = resultat(c, "contrats-passes", json!({"mois": "2026-05"}));
    assert_eq!(mai["contrats"].as_array().unwrap().len(), 1);
    let m = &mai["contrats"][0];
    assert_eq!((m["source"].as_str(), m["titre"].as_str(), m["cachets"].as_i64(), m["heures"].as_i64()), (Some("historique"), Some("Les Pupazzi"), Some(3), Some(14)));
    assert_eq!(m["dates"], json!([]));
    // Un salarie voulu, ou plusieurs.
    let filtre = resultat(c, "contrats-passes", json!({"mois": "2026-09", "salaries": ["SERAPHIN"]}));
    assert_eq!(filtre["contrats"].as_array().unwrap().len(), 1);
    assert_eq!(filtre["contrats"][0]["salarie"], "SERAPHIN");
    assert_eq!(resultat(c, "contrats-passes", json!({"mois": "2026-09", "salaries": ["PERSONNE"]}))["contrats"], json!([]));
}

#[test]
fn une_charge_fausse_ou_un_mois_abime_refuse() {
    let c = &compagnie("refus");
    ecrire_mois(c, "2026-09", json!([contrat_a_cheval()]));
    for (nom, charge) in [("contrats-en-cours", json!({})), ("contrats-passes", json!({})), ("contrats-en-cours", json!({"mois": 5}))] {
        refus(c, nom, charge);
    }
    std::fs::write(c.join("engagements/2026-08.json"), "{pas du json").unwrap();
    refus(c, "contrats-en-cours", json!({"mois": "2026-10"}));
    // Des lignes qui ne sont pas des objets.
    std::fs::remove_file(c.join("engagements/2026-08.json")).unwrap();
    let mut mal = contrat_a_cheval();
    mal["lignes"] = json!(["texte"]);
    ecrire_mois(c, "2026-07", json!([mal]));
    refus(c, "contrats-passes", json!({"mois": "2026-10"}));
}

// ================================================ emploi

#[test]
fn un_metier_se_traduit_en_champs_de_fiche_et_un_code_inconnu_est_un_refus() {
    let c = &compagnie("emploi");
    let r = resultat(c, "emploi", json!({"metier": " com010 "}));
    assert_eq!(r["code_emploi_ft"], "COM010");
    let cadre = resultat(c, "emploi", json!({"metier": "COM010", "cadre": true, "feminin": true}));
    assert_eq!((cadre["statut_categoriel_rc"].as_str(), cadre["statut_conventionnel"].as_str()), (Some("01"), Some("04")));
    let non = resultat(c, "emploi", json!({"metier": "COM010", "cadre": false}));
    assert_eq!(non["statut_categoriel_rc"], "04");
    let message = refus(c, "emploi", json!({"metier": "zzz999"}));
    assert!(message.starts_with("Metier 'zzz999' inconnu"), "{}", message);
    for charge in [json!({"metier": ""}), json!({"metier": 5}), json!({})] {
        refus(c, "emploi", charge);
    }
}

// ================================================ metiers-catalogue

#[test]
fn le_catalogue_dit_ce_que_la_compagnie_retient() {
    let c = &compagnie("metiers");
    let r = resultat(c, "metiers-catalogue", json!({}));
    let metiers = r["metiers"].as_array().unwrap();
    assert!(metiers.len() > 50);
    assert_eq!(r["familles"][0], "Artistes");
    // Chacun dit s'il vient de la convention de la compagnie, et s'il est retenu.
    assert!(metiers.iter().all(|m| m["retenu"].is_boolean() && m["de_la_convention"].is_boolean() && m["de"].is_string()));
    let hors = metiers.iter().find(|m| m["de_la_convention"] == json!(false)).expect("un metier d'une autre convention");
    assert_eq!(hors["retenu"], json!(false));
    assert_ne!(hors["de"], "");
    // La compagnie le coche, et en decoche un des siens.
    let propre = metiers.iter().find(|m| m["de_la_convention"] == json!(true)).unwrap()["code"].clone();
    ecrire(c, "config/metiers-ajoutes.json", &json!({"codes": [hors["code"]], "retires": [propre]}));
    let r = resultat(c, "metiers-catalogue", json!({}));
    let par = |code: &Value| r["metiers"].as_array().unwrap().iter().find(|m| &m["code"] == code).unwrap().clone();
    assert_eq!(par(&hors["code"])["retenu"], json!(true));
    assert_eq!(par(&propre)["retenu"], json!(false));
    assert_eq!(par(&propre)["de_la_convention"], json!(true));
    // Sans IDCC (une compagnie qui s'installe), rien ne se cache.
    let mut entreprise = lire(c, "config/entreprise.json");
    entreprise["idcc"] = json!("");
    ecrire(c, "config/entreprise.json", &entreprise);
    let r = resultat(c, "metiers-catalogue", json!({}));
    assert!(r["metiers"].as_array().unwrap().iter().all(|m| m["de_la_convention"] == json!(true)));
}

// ================================================ rubriques-actives

#[test]
fn cocher_des_rubriques_ecrit_la_preference_garde_une_copie_et_rend_le_catalogue() {
    let c = &compagnie("actives");
    let avant = std::fs::read_to_string(c.join("config/rubriques-actives.json")).unwrap();
    let r = resultat(c, "rubriques-actives", json!({"actives": ["journee", "cachet", "journee"]}));
    let rubriques = r["rubriques"].as_array().unwrap();
    // Les actives d'abord, dans l'ordre coche.
    assert_eq!(rubriques.iter().filter(|x| x["active"] == json!(true)).count(), 2);
    assert!(rubriques[0]["active"] == json!(true) && rubriques[2]["active"] == json!(false));
    let ecrit = lire(c, "config/rubriques-actives.json");
    assert_eq!(ecrit["actives"], json!(["journee", "cachet"]));
    // La copie horodatee de l'ancien fichier, avec l'heure de Paris.
    let copies: Vec<_> = std::fs::read_dir(c.join("config/avant")).unwrap().flatten().collect();
    assert_eq!(copies.len(), 1);
    let nom = copies[0].file_name().to_string_lossy().into_owned();
    let horodatage = nom.strip_prefix("rubriques-actives.json.").expect(&nom);
    assert!(horodatage.len() == 15 && horodatage.as_bytes()[8] == b'-' && horodatage.bytes().filter(u8::is_ascii_digit).count() == 14, "{}", nom);
    assert_eq!(std::fs::read_to_string(copies[0].path()).unwrap(), avant);
}

#[test]
fn une_rubrique_inconnue_est_refusee_sans_rien_ecrire() {
    let c = &compagnie("actives-refus");
    let avant = std::fs::read_to_string(c.join("config/rubriques-actives.json")).unwrap();
    let message = refus(c, "rubriques-actives", json!({"actives": ["cachet", "nope"]}));
    assert!(message.starts_with("Rubrique(s) inconnue(s) : nope. Connues : "), "{}", message);
    assert_eq!(std::fs::read_to_string(c.join("config/rubriques-actives.json")).unwrap(), avant);
    assert!(!c.join("config/avant").exists());
    refus(c, "rubriques-actives", json!({}));
    refus(c, "rubriques-actives", json!({"actives": 5}));
    assert!(!c.join("config/avant").exists());
}

#[test]
fn seules_les_vingt_dernieres_copies_restent() {
    let c = &compagnie("actives-copies");
    for k in 0..25 {
        ecrire(c, &format!("config/avant/rubriques-actives.json.20260101-0000{:02}", k), &json!(k));
    }
    ecrire(c, "config/avant/autre.json.20260101-000000", &json!("autre"));
    resultat(c, "rubriques-actives", json!({"actives": ["cachet"]}));
    let mut noms: Vec<String> =
        std::fs::read_dir(c.join("config/avant")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    noms.sort();
    let siennes: Vec<_> = noms.iter().filter(|n| n.starts_with("rubriques-actives.json.")).collect();
    assert_eq!(siennes.len(), 20);
    // Les plus anciennes sont parties, celle d'un autre fichier reste.
    assert!(!noms.contains(&"rubriques-actives.json.20260101-000000".to_string()));
    assert!(noms.contains(&"autre.json.20260101-000000".to_string()));
}
