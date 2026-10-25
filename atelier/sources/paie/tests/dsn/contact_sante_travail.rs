//! LE SERVICE DE SANTE AU TRAVAIL ET SON CONTACT (#236). Norme 2027 (cahier technique 2027.1.1,
//! `atelier/reference/norme-dsn/texte-2027/`) :
//! - S21.G00.11.025 (`bloc-S21.G00.11.txt` l. 232-237, la meme en 2026) : « Cette rubrique doit etre renseignee par l'identifiant du
//!   service de prevention et de sante au travail (SPST) » ; les SPST autonomes y valent « MT01 - MEDECINE INTERNE
//!   ETABLISSEMENT » (note differentielle 2026.1.2 -> 2027.1.2, § 13, l. 3497-3503) ;
//! - CCH-11 de S20.G00.07.004 (`bloc-S20.G00.07.txt` l. 46-65 et 81) : « Si et seulement si » l'etablissement declare
//!   MT01, un contact de type « 17 - Contact au sein du service de medecine du travail interne » doit obligatoirement etre
//!   present ; « dans le cas contraire, il est interdit ».
//!
//! La norme 2026 n'a ni MT01 ni le type 17 (`texte/bloc-S20.G00.07.txt` : les types s'arretent a 16). Sur une copie de
//! `fictive-3090-grande` (compagnie inventee) dont le bareme couvre 2027, comme `rubriques_2027.rs`.

use crate::outils::{self, compagnie, ecrire, lire};
use droit::contexte::Contexte;
use socle::date::Jour;
use paie::dsn::{generer, Options};
use serde_json::{json, Value};
use std::path::PathBuf;

fn copie(test: &str, entreprise: Value) -> PathBuf {
    let racine = compagnie::ses_dsn("fictive-3090-grande", &[], &outils::dossier_de_test("contact_sante_travail", test));
    let mut b = lire(&racine, "config/baremes/2026.json");
    b["annee"] = json!(2027);
    b["socle"] = json!(2026);
    ecrire(&racine, "config/baremes/2027.json", &b);
    let mut ent = lire(&racine, "config/entreprise.json");
    // Un fichier P27V01 ne part qu'avec un code NAF 2025 (`naf::verifier_pour`).
    ent["naf"] = json!("9020Y");
    for (k, v) in entreprise.as_object().unwrap() {
        ent[k] = v.clone();
    }
    ecrire(&racine, "config/entreprise.json", &ent);
    racine
}

/// La DSN de `mois` (decembre 2026, en P26V01 ; janvier 2027, en P27V01), un artiste deux cachets les 20 et 21.
fn dsn(test: &str, mois: &str, entreprise: Value) -> Result<String, String> {
    let racine = copie(test, entreprise);
    let jour = if mois == "2027-01" { "2027-02-03" } else { "2027-01-05" };
    let ctx = Contexte::nouveau(&racine, Some(Jour::lire(jour).unwrap())).unwrap();
    let (j1, j2) = (format!("{}-20", mois), format!("{}-21", mois));
    let s = json!({"mois": mois, "date_paiement": format!("{}-28", mois), "engagements": [
        {"salarie": "Deburau", "spectacle": "266Z00910022", "debut": j1, "fin": j2, "numero": format!("DE.{}", mois),
         "date_paiement": j2, "lignes": [{"rubrique": "cachet", "quantite": 2, "taux": 300.0, "lieu": "Theatre fictif", "dates": [j1, j2]}]}]});
    let chemin = outils::chemin_unique("contact-sante-travail.json");
    std::fs::write(&chemin, s.to_string()).unwrap();
    let paies = paie::engagement::charger(&ctx, &chemin)?.paies;
    generer(&ctx, &paies, mois, &[], &[], &Options { date_constitution: Some(Jour::lire(jour).unwrap()), ..Options::default() })
}

/// Les types des contacts (S20.G00.07.004), dans l'ordre.
fn types(d: &str) -> Vec<String> {
    outils::paires(d).into_iter().filter(|(k, _)| k == "S20.G00.07.004").map(|(_, v)| v).collect()
}

const CONTACT: &str = r#"{"nom_prenom": "VIGNE ROSE", "tel": "0102030405", "email": "medecin@exemple.invalid"}"#;

#[test]
fn en_2027_la_medecine_interne_declare_son_contact_de_type_17() {
    let contact: Value = serde_json::from_str(CONTACT).unwrap();
    let d = dsn("mt01-avec", "2027-01", json!({"medecine_travail": "MT01", "contact_sante_travail": contact})).unwrap();
    assert_eq!(outils::valeur(&outils::paires(&d), "S21.G00.11.025").as_deref(), Some("MT01"));
    assert_eq!(types(&d), vec!["04", "17"]);
    assert!(outils::paires(&d).iter().any(|(k, v)| k == "S20.G00.07.001" && v == "VIGNE ROSE"));
    // Obligatoire : sans lui, rien n'est ecrit.
    let e = dsn("mt01-sans", "2027-01", json!({"medecine_travail": "MT01", "contact_sante_travail": {}})).unwrap_err();
    assert!(e.contains("S20.G00.07"), "{}", e);
}

#[test]
fn hors_de_la_medecine_interne_le_type_17_est_interdit() {
    let contact: Value = serde_json::from_str(CONTACT).unwrap();
    // Un contact saisi puis le service change : « dans le cas contraire, il est interdit ».
    let d = dsn("externe", "2027-01", json!({"medecine_travail": "MT42", "contact_sante_travail": contact})).unwrap();
    assert_ne!(outils::valeur(&outils::paires(&d), "S21.G00.11.025").as_deref(), Some("MT01"));
    assert_eq!(types(&d), vec!["04"]);
}

#[test]
fn en_2026_ni_l_identifiant_ni_le_type_17() {
    let contact: Value = serde_json::from_str(CONTACT).unwrap();
    let d = dsn("2026", "2026-12", json!({"medecine_travail": "MT01", "contact_sante_travail": contact})).unwrap();
    assert!(!outils::porte(&d, "S21.G00.11.025"));
    assert_eq!(types(&d), vec!["04"]);
}
