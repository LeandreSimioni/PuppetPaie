//! LE TAUX AT/MP QUE LA CARSAT NOTIFIE SE POSE DE LUI-MEME. La notification arrive en retour DSN de nature 34 (« CR métier
//! taux AT », guide de l'API DSN v6.8, §5.26.1), au format des bilans : la declaration a laquelle elle est rattachee
//! (`SIREN`, `nic_affectation`) et un element `taux_AT` (`code_risque`, `taux`, `date_effet`, `date_calcul`…), specification
//! du schema des bilans VT2.2 (`atelier/reference/sources/gip-mds-api-dsn/`, LISEZ-MOI, « La notification du taux AT/MP »).
//! Le code et le taux se lisent (regle 1) et se posent dans la surcouche de l'annee de la date d'effet ; un etablissement
//! qui n'est pas celui de la compagnie ne pose rien.

use std::path::{Path, PathBuf};

use moteur::interface::compagnies::ajouter_dans;
use serde_json::{json, Value};

use crate::outils::{datee, dossier_de_test, ecrire, fiche_au, lire};

fn notification(racine: &Path, nom: &str, siren: &str, nic: &str, taux_at: &[(&str, &str, &str, &str)]) -> (Value, PathBuf) {
    let taux: String = taux_at
        .iter()
        .map(|(code, taux, effet, calcul)| {
            format!("<taux_AT><code_ctn>I</code_ctn><section>01</section><code_risque>{}</code_risque><taux>{}</taux><date_effet>{}</date_effet><date_calcul>{}</date_calcul></taux_AT>", code, taux, effet, calcul)
        })
        .collect();
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ns2:rapport xmlns:ns2=\"http://www.gip-mds.fr/\" profil=\"DSN\" version=\"v02r02\"><declaration>\
         <declaration_identification><rang>1</rang><nature>01</nature><SIREN>{}</SIREN><nic_affectation>{}</nic_affectation></declaration_identification>\
         <declaration_bilan><etat>OK</etat>{}</declaration_bilan></declaration></ns2:rapport>",
        siren, nic, taux
    );
    let chemin = racine.join("retours").join(nom);
    std::fs::create_dir_all(chemin.parent().unwrap()).unwrap();
    std::fs::write(&chemin, xml).unwrap();
    (json!({"nature": "34", "publication": "20251215100000", "statut": "OK"}), chemin)
}

fn suites(racine: &Path, notifications: Vec<(Value, PathBuf)>) -> Vec<String> {
    let ctx = droit::contexte::Contexte::nouveau(racine, None).unwrap();
    let s = envois::suites::Suites { taux_atmp: notifications, ..Default::default() };
    let faites = moteur::compagnie::suites::faire(&ctx, s).unwrap();
    assert!(faites.incidents.is_empty(), "{:?}", faites.incidents);
    faites.a_faire
}

#[test]
fn la_notification_lue_pose_le_taux_de_l_annee_a_sa_date_d_effet() {
    let racine = ajouter_dans(&dossier_de_test("notification_atmp", "pose"), "Les Tréteaux Notifiés").unwrap().racine;
    let mut ent = fiche_au(&lire(&racine, "config/entreprise.json"), "2025-12-15");
    ent["siren"] = json!("123456873");
    ent["nic"] = json!("00017");
    ecrire(&racine, "config/entreprise.json", &datee(&ent));

    // Effet au 1er janvier : le taux de l'annee, avec la notification pour source.
    let a_faire = suites(&racine, vec![notification(&racine, "n1.xml", "123456873", "00017", &[("923AD", "1.70", "20260101", "20251210")])]);
    assert!(a_faire.is_empty(), "{:?}", a_faire);
    let t = &lire(&racine, "config/baremes/2026.json")["taux_atmp"];
    assert_eq!((&t["code_risque"], &t["valeur"]), (&json!("923AD"), &json!(1.7)), "{}", t);
    assert_eq!(t["_notification"]["effet"], json!("2026-01-01"), "{}", t);

    // Effet en cours d'annee : le taux d'avant jusqu'a la date d'effet, le notifie ensuite.
    let a_faire = suites(&racine, vec![notification(&racine, "n2.xml", "123456873", "00017", &[("923AD", "1,90", "20260701", "20260615")])]);
    assert!(a_faire.is_empty(), "{:?}", a_faire);
    let t = &lire(&racine, "config/baremes/2026.json")["taux_atmp"];
    let au = |j: &str| droit::bareme::taux_atmp_au(&t["valeur"], &socle::date::Jour::lire(j).unwrap()).unwrap().to_string();
    assert_eq!((au("2026-06-30"), au("2026-07-01")), ("1.7".to_string(), "1.9".to_string()), "{}", t);

    // Un autre etablissement : rien ne se pose, et cela se dit.
    let a_faire = suites(&racine, vec![notification(&racine, "n3.xml", "987654321", "00011", &[("923AD", "2.10", "20270101", "20261210")])]);
    assert_eq!(a_faire.len(), 1, "{:?}", a_faire);
    assert!(!racine.join("config/baremes/2027.json").exists());
}
