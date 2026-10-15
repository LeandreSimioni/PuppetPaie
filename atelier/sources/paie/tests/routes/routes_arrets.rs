//! Les arrets de travail saisis par la route : `arret-enregistrer`, `arret-reprise`, `arret-oublier`, dans le magasin des
//! imprevus (`imprevus/AAAA-MM.json`). Ce que la norme refuse n'ecrit rien, un arret deja declare ne se retire pas, deux
//! imprevus du meme contrat ne se chevauchent pas. Sur `fictive-3090-petite` (config, janvier et mars 2025).

use crate::outils::{compagnie, dossier_de_test, lire, refus};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn compagnie(nom: &str) -> PathBuf {
    compagnie::saisies("fictive-3090-petite", &["2025-01", "2025-03"], &dossier_de_test("routes_arrets", nom).join("compagnie"))
}

fn route(racine: &Path, nom: &str, charge: Value) -> Value {
    crate::outils::route(racine, nom, charge, "2026-10-03")
}

fn refuse(r: &Value) -> String {
    refus(r).1
}

fn arret(dernier_jour: &str) -> Value {
    json!({"salarie": "Brioche", "dernier_jour": dernier_jour, "fin": "2025-02-10", "motif": "01", "subrogation": "02"})
}

#[test]
fn un_arret_se_saisit_se_reprend_et_s_oublie() {
    let racine = compagnie("arret");
    let r = route(&racine, "arret-enregistrer", arret("2025-01-22"));
    assert_eq!(r["ok"], json!(true), "{}", r);
    let a = &r["resultat"]["arret"];
    assert_eq!(a["salarie"], json!("BRIOCHE"));
    assert_eq!(a["genre"], json!("arret"));
    assert_eq!(a["contrat"], json!("BR1.25.01"));
    assert_eq!(a["contrat_debut"], json!("2025-01-21"));
    assert_eq!(a["motif_libelle"], json!("maladie"));
    assert_eq!(a["etat"], json!("a signaler"));
    assert_eq!((a["premier_jour"].clone(), a["dernier_jour"].clone()), (json!("2025-01-23"), json!("2025-02-10")));
    // Un CDDU : pas de maintien de salaire a saisir (L1226-1), et ses cachets des 23 et 24 tombent dans l'arret.
    assert_eq!(a["maintien_ouvert"], json!(false));
    assert_eq!(a["jours_deja_indemnises_attendus"], json!(false));
    let dates: Vec<Value> = a["cachets"].as_array().unwrap().iter().flat_map(|c| c["dates"].as_array().unwrap().clone()).collect();
    assert!(dates.contains(&json!("2025-01-23")) && dates.contains(&json!("2025-01-24")), "{}", a);
    // Le fichier : les champs de l'arret, que le moteur relit, dans le magasin des imprevus.
    let f = lire(&racine, "imprevus/2025-01.json");
    assert_eq!(f["mois"], json!("2025-01"));
    let mut champs: Vec<&String> = f["imprevus"][0].as_object().unwrap().keys().collect();
    champs.sort();
    assert_eq!(
        champs,
        [
            "contrat_debut", "contrat_numero", "date_accident", "dernier_jour_travaille", "envois", "fin_previsionnelle", "genre",
            "jours_deja_indemnises", "maintien", "motif", "note", "qui", "reprise", "reprise_motif", "saisi_le", "salarie",
            "subrogation", "subrogation_debut", "subrogation_fin"
        ]
    );
    assert_eq!(f["imprevus"][0]["qui"], json!("interface"));
    assert_eq!(f["imprevus"][0]["envois"], json!([]));
    let saisi = f["imprevus"][0]["saisi_le"].as_str().unwrap();
    assert_eq!((saisi.len(), &saisi[10..11]), (16, " "));

    // La reprise : anticipee, avec son motif ; puis effacee.
    let r = route(&racine, "arret-reprise", json!({"salarie": "BRIOCHE", "dernier_jour": "2025-01-22", "reprise": "2025-01-30", "motif_reprise": "01"}));
    assert_eq!(r["resultat"]["arret"]["reprise"], json!("2025-01-30"), "{}", r);
    assert_eq!(lire(&racine, "imprevus/2025-01.json")["imprevus"][0]["reprise_motif"], json!("01"));
    let r = route(&racine, "arret-reprise", json!({"salarie": "BRIOCHE", "dernier_jour": "2025-01-22", "reprise": ""}));
    assert_eq!(r["resultat"]["arret"]["reprise"], Value::Null, "{}", r);

    // Deux arrets au meme dernier jour travaille se contredisent ; un second qui chevauche le premier aussi.
    assert!(refuse(&route(&racine, "arret-enregistrer", arret("2025-01-22"))).contains("deja enregistre"));
    assert!(refuse(&route(&racine, "arret-enregistrer", arret("2025-01-23"))).contains("chevauche"));
    // Repris le 23, le premier laisse la place : un second, le 23, s'ajoute au fichier du mois.
    assert_eq!(route(&racine, "arret-reprise", json!({"salarie": "BRIOCHE", "dernier_jour": "2025-01-22", "reprise": "2025-01-23", "motif_reprise": "01"}))["ok"], json!(true));
    assert_eq!(route(&racine, "arret-enregistrer", arret("2025-01-23"))["ok"], json!(true));
    assert_eq!(lire(&racine, "imprevus/2025-01.json")["imprevus"].as_array().unwrap().len(), 2);

    let r = route(&racine, "arret-oublier", json!({"salarie": "BRIOCHE", "dernier_jour": "2025-01-22"}));
    assert_eq!(r["resultat"]["arret"]["dernier_jour_travaille"], json!("2025-01-22"), "{}", r);
    let restants = lire(&racine, "imprevus/2025-01.json")["imprevus"].clone();
    assert_eq!(restants.as_array().unwrap().len(), 1);
    assert_eq!(restants[0]["dernier_jour_travaille"], json!("2025-01-23"));
}

#[test]
fn un_arret_que_la_norme_refuse_n_ecrit_rien() {
    let racine = compagnie("arret-refus");
    let sans_subrogation = json!({"salarie": "Brioche", "dernier_jour": "2025-01-22", "fin": "2025-02-10", "motif": "01"});
    let mut accident = arret("2025-01-22");
    accident["motif"] = json!("06");
    let mut avant_le_contrat = arret("2025-01-22");
    avant_le_contrat["motif"] = json!("05");
    avant_le_contrat["date_accident"] = json!("2025-01-02");
    let mut fin_avant = arret("2025-01-22");
    fin_avant["fin"] = json!("2025-01-05");
    let mut contrat_inconnu = arret("2025-01-22");
    contrat_inconnu["contrat"] = json!("ZZZ.99.99");
    let hors_contrat = arret("2025-03-22");
    let mut annulation = arret("2025-01-22");
    annulation["motif"] = json!("99");
    // Les jours deja indemnises (D1226-4) ne servent qu'au maintien d'un permanent : un CDDU ne les appelle pas.
    let mut deja = arret("2025-01-22");
    deja["jours_deja_indemnises"] = json!(3);
    for (nom, charge) in [
        ("subrogation absente", sans_subrogation),
        ("accident sans date", accident),
        ("accident avant le contrat", avant_le_contrat),
        ("fin avant le dernier jour", fin_avant),
        ("contrat inconnu", contrat_inconnu),
        ("aucun contrat ce jour-la", hors_contrat),
        ("motif d'annulation", annulation),
        ("jours deja indemnises d'un CDDU", deja),
        ("date illisible", arret("pas une date")),
        ("salarie inconnu", json!({"salarie": "Nulle part", "dernier_jour": "2025-01-22", "fin": "2025-02-10", "motif": "01", "subrogation": "02"})),
    ] {
        assert!(!refuse(&route(&racine, "arret-enregistrer", charge)).is_empty(), "{}", nom);
    }
    assert!(!racine.join("imprevus").exists(), "un refus ne laisse aucun fichier");
    assert!(refuse(&route(&racine, "arret-reprise", json!({"salarie": "Brioche", "dernier_jour": "2025-01-22", "reprise": "2025-01-30"}))).contains("Aucun arret"));
}

#[test]
fn un_arret_deja_declare_ne_se_retire_pas_et_un_fichier_sans_ses_facultatifs_se_reecrit_complet() {
    let racine = compagnie("arret-declare");
    // Un fichier ecrit sans ses champs facultatifs : `qui`, `note`, `envois`, `maintien`... absents.
    std::fs::create_dir_all(racine.join("imprevus")).unwrap();
    let ancien = json!({"_origine": "a la main", "mois": "2025-01", "imprevus": [
        {"genre": "arret", "salarie": "BRIOCHE", "contrat_debut": "2025-01-21", "contrat_numero": "BR1.25.01", "motif": "01",
         "dernier_jour_travaille": "2025-01-21", "fin_previsionnelle": "2025-01-21", "subrogation": "02"}]});
    std::fs::write(racine.join("imprevus/2025-01.json"), ancien.to_string()).unwrap();
    assert_eq!(route(&racine, "arret-enregistrer", arret("2025-01-22"))["ok"], json!(true));
    let f = lire(&racine, "imprevus/2025-01.json");
    assert_eq!(f["imprevus"][0]["qui"], json!("?"));
    assert_eq!(f["imprevus"][0]["note"], json!(""));
    assert_eq!(f["imprevus"][0]["envois"], json!([]));
    assert_eq!(f["imprevus"][0]["maintien"], json!({}));
    assert!(f["_origine"].as_str().unwrap().starts_with("Les imprevus ranges en 2025-01"));
    // Declare : un envoi est trace, l'arret ne s'oublie plus.
    let mut f = f;
    f["imprevus"][0]["envois"] = json!([{"nature": "04", "type": "01", "ordre": 1, "fichier": "x", "quand": "2025-01-23 10:00", "identifiant": ""}]);
    std::fs::write(racine.join("imprevus/2025-01.json"), f.to_string()).unwrap();
    let r = route(&racine, "arret-oublier", json!({"salarie": "BRIOCHE", "dernier_jour": "2025-01-21"}));
    assert!(refuse(&r).contains("1 envoi(s)"), "{}", r);
    assert_eq!(lire(&racine, "imprevus/2025-01.json")["imprevus"].as_array().unwrap().len(), 2);
}

/// Le dossier `arrets/` d'avant le 15/10/2026 n'est plus lu, et sa presence refuse : un arret qu'il garderait ne serait
/// ni reporte ni retire de la paie.
#[test]
fn le_dossier_arrets_d_avant_refuse_plutot_que_d_etre_ignore() {
    let racine = compagnie("arret-ancien");
    std::fs::create_dir_all(racine.join("arrets")).unwrap();
    std::fs::write(racine.join("arrets/2025-01.json"), json!({"mois": "2025-01", "arrets": []}).to_string()).unwrap();
    let m = refuse(&route(&racine, "arrets", json!({})));
    assert!(m.contains("n'est plus lu") && m.contains("imprevus"), "{}", m);
    assert!(!refuse(&route(&racine, "arret-enregistrer", arret("2025-01-22"))).is_empty());
}
