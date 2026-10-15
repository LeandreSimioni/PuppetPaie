//! Ce que les routes rendent de l'echeance de depot d'une DSN (`paie/src/echeance_dsn.rs`) : l'echeancier, les rappels,
//! les regularisations et le signalement d'arret subroge disent tous le meme jour, reporte, a midi. Sur `fictive-3090-petite`
//! (moins de cinquante salaries : le 15), la DSN d'octobre 2026.
//!
//! Les textes : CSS R. 243-6, II (`atelier/reference/sources/echeances-dsn/legi-securite-sociale-R243-6-LEGIARTI000034668597.xml`),
//! le 15 du mois suivant ; CSS R. 133-14, I (`atelier/reference/sources/code-de-la-securite-sociale/xml/R133-14.xml`), le
//! report au jour ouvrable suivant ; le calendrier de net-entreprises
//! (`atelier/reference/sources/echeances-dsn/net-entreprises-calendrier-des-declarations-20261015.txt`) : le dimanche
//! 15/11/2026 devient le lundi 16/11/2026 ; cahier technique DSN 2026.1, § 1.4.2
//! (`atelier/reference/norme-dsn/texte/chapitre-1-la-norme.txt`) : midi, et l'annule-et-remplace jusqu'a la veille a minuit.

use crate::outils::{compagnie, dossier_de_test, ecrire, reussi, route};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn compagnie(nom: &str) -> PathBuf {
    let racine = compagnie::saisies("fictive-3090-petite", &["2026-10"], &dossier_de_test("echeance_dsn", nom).join("compagnie"));
    std::fs::remove_file(racine.join("config/compteurs.json")).unwrap();
    std::fs::remove_file(racine.join("config/echeancier.json")).unwrap();
    racine
}

fn par_cle<'a>(liste: &'a Value, cle: &str) -> &'a Value {
    liste.as_array().unwrap().iter().find(|x| x["cle"] == json!(cle)).unwrap_or_else(|| panic!("pas de {} dans {}", cle, liste))
}

fn depot_dsn(mois: &str) -> Value {
    json!({
        "quoi": "dsn", "quand": "2026-11-05T10:00:00", "origine": "", "mois": mois, "fichier": format!("DSN_{}.dsn", mois),
        "empreinte": "abc", "mode": "reel", "qui": "test", "accepte": true, "destinataire": "net-entreprises", "flux": "F",
        "retours": [{"nature": "11", "verdict": "OK"}],
    })
}

/// Ce que porte `echeance_depot` : le lundi 16/11/2026 a midi, au lieu du dimanche 15.
fn reportee_au_16(e: &Value) {
    assert_eq!(
        (e["jour"].as_str(), e["heure"].as_str(), e["nominale"].as_str(), e["reportee"].as_bool()),
        (Some("2026-11-16"), Some("12:00"), Some("2026-11-15"), Some(true)),
        "{}",
        e
    );
    assert!(e["inference"].as_str().is_some_and(|i| i.starts_with("Inférence")), "{}", e);
}

#[test]
fn l_echeancier_dit_le_jour_reporte_a_midi() {
    let c = &compagnie("echeancier");
    let r = reussi(&route(c, "echeancier", json!({}), "2026-11-10")).clone();
    let dsn = par_cle(&r["ce_mois"], "dsn:2026-10");
    assert_eq!(dsn["date"], json!("2026-11-16"));
    let detail = dsn["detail"].as_str().unwrap();
    assert!(detail.starts_with("Au plus tard le 16 novembre 2026 à midi. Le 15/11/2026 (dimanche) n'est pas ouvré"), "{}", detail);
    reportee_au_16(&dsn["echeance_depot"]);
    // Le lundi 16 est encore dans les temps ; le mardi 17, en retard.
    let le = |jour: &str| reussi(&route(c, "echeancier", json!({}), jour))["ce_mois"].clone();
    assert_eq!(par_cle(&le("2026-11-16"), "dsn:2026-10")["etat"], json!("a_faire"));
    assert_eq!(par_cle(&le("2026-11-17"), "dsn:2026-10")["etat"], json!("en_retard"));
}

#[test]
fn le_rappel_n_est_en_retard_qu_apres_le_jour_reporte() {
    let c = &compagnie("rappels");
    let rappel = |jour: &str| par_cle(&reussi(&route(c, "rappels", json!({}), jour))["rappels"], "dsn:2026-10").clone();
    let r = rappel("2026-11-16");
    assert_eq!(r["en_retard"], json!(false), "{}", r);
    reportee_au_16(&r["echeance_depot"]);
    assert_eq!(rappel("2026-11-17")["en_retard"], json!(true));
}

/// L'annule-et-remplace s'arrete la veille de l'echeance a minuit : la veille du lundi 16, le dimanche 15 (inference :
/// la veille de l'echeance reportee).
#[test]
fn la_regularisation_suit_l_echeance_reportee() {
    let c = &compagnie("regularisations");
    ecrire(c, "journal/depots.json", &json!([depot_dsn("2026-10")]));
    let r = reussi(&route(c, "regularisations", json!({}), "2026-11-15")).clone();
    assert_eq!(r["mois"]["regularisables"], json!([]), "le dimanche 15, l'annule-et-remplace est encore admis : {}", r["mois"]);
    let r = reussi(&route(c, "regularisations", json!({}), "2026-11-16")).clone();
    let reg = &r["mois"]["regularisables"][0];
    assert_eq!((reg["mois"].as_str(), reg["echeance"].as_str()), (Some("2026-10"), Some("2026-11-16")), "{}", r["mois"]);
    assert!(reg["raison"].as_str().unwrap().contains("15/11/2026 a minuit"), "{}", reg["raison"]);
    reportee_au_16(&reg["echeance_depot"]);
}

/// R. 133-14, II, premier alinea : subroge, l'arret part avec la DSN du mois ou il commence ; il herite de son report.
#[test]
fn le_signalement_subroge_herite_du_report() {
    let c = &compagnie("subroge");
    ecrire_arret(c);
    let r = reussi(&route(c, "arrets", json!({}), "2026-10-13")).clone();
    let s = par_cle(&r["arrets"][0]["obligations"], "signalement_arret");
    assert_eq!((s["subrogation"].as_bool(), s["butoir"].as_str(), s["butoir_heure"].as_str()), (Some(true), Some("2026-11-16"), Some("12:00")), "{}", s);
    assert!(s["phrase"].as_str().unwrap().contains("le lundi 16/11/2026 à midi"), "{}", s["phrase"]);
    reportee_au_16(&s["echeance_dsn"]);
}

/// Un arret de BRIOCHE commence le samedi 10/10/2026, subroge, ecrit tel que `arret-enregistrer` l'ecrirait.
fn ecrire_arret(c: &Path) {
    let a = json!({
        "genre": "arret", "salarie": "BRIOCHE", "contrat_debut": "2026-01-02", "contrat_numero": "BR9.26.01", "motif": "01",
        "dernier_jour_travaille": "2026-10-09", "fin_previsionnelle": "2026-10-31", "subrogation": "01",
        "subrogation_debut": null, "subrogation_fin": null, "reprise": null, "reprise_motif": null,
        "date_accident": null, "saisi_le": "2026-10-12 10:00", "qui": "essai", "note": "", "envois": [],
    });
    ecrire(c, "imprevus/2026-10.json", &json!({"mois": "2026-10", "imprevus": [a]}));
}
