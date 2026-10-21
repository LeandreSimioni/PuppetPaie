//! MODIFIER LA PAIE D'UN MOIS DONT LA DSN EST DEPOSEE (Leandre, 21/10/2026). Tant que l'annule-et-remplace est admis
//! (jusqu'a la veille de l'echeance, minuit : cahier technique DSN §1.4.2, `echeance_dsn::Echeance::veille`), la paie se
//! modifie sans rien confirmer, et le moteur dit qu'un annule-et-remplace partira. Apres, la DSN deposee ne se remplace
//! plus : une ecriture qui change la paie se refuse sans confirmation explicite, et rien n'est ecrit ; avec, elle s'ecrit,
//! la saisie telle que declaree est gardee, et « Corriger ce mois » en part pour la DSN suivante. Sur
//! `fictive-3090-petite` (aout 2026, DSN reelle deposee et acceptee).

use crate::outils::{compagnie, dossier_de_test, lire, refus, reussi};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const MOIS: &str = "2026-08";
/// La limite de l'annule-et-remplace d'aout 2026 est passee (la DSN d'octobre embarque la correction).
const APRES: &str = "2026-10-03";

fn compagnie(nom: &str) -> PathBuf {
    compagnie::config_et_mois("fictive-3090-petite", &[MOIS], &dossier_de_test("modifier_apres_echeance", nom).join("compagnie"))
}

fn route(racine: &Path, nom: &str, charge: Value, jour: &str) -> Value {
    crate::outils::route(racine, nom, charge, jour)
}

fn octets(racine: &Path, rel: &str) -> Option<Vec<u8>> {
    std::fs::read(racine.join(rel)).ok()
}

/// La saisie d'aout, les heures de Lemercier portees a `heures`.
fn saisie_modifiee(racine: &Path, heures: i64) -> Value {
    let mut s = lire(racine, &format!("paies/{}.json", MOIS));
    let rang = s["engagements"].as_array().unwrap().iter().position(|e| e["salarie"] == "Lemercier").unwrap();
    s["engagements"][rang]["lignes"][0]["quantite"] = json!(heures);
    s
}

/// L'engagement de LEMERCIER dans `regularisation-mois`.
fn lemercier(racine: &Path) -> Value {
    let r = route(racine, "regularisation-mois", json!({"mois": MOIS}), APRES);
    reussi(&r)["engagements"].as_array().unwrap().iter().find(|e| e["salarie"] == "LEMERCIER").unwrap().clone()
}

#[test]
fn apres_la_limite_une_modification_se_refuse_sans_confirmation_et_rien_n_est_ecrit() {
    let racine = compagnie("apres-la-limite");
    let saisie = format!("paies/{}.json", MOIS);
    let copie = format!("regularisations/{}.saisie-declaree.json", MOIS);

    // Le mois n'est pas ferme : il se modifie, en le confirmant. Le moteur le dit avant toute frappe.
    let s = route(&racine, "saisie", json!({"mois": MOIS}), APRES);
    let d = reussi(&s)["dsn_deposee"].clone();
    assert_eq!((d["confirmer"].as_bool(), &reussi(&s)["hors_delai"]), (Some(true), &Value::Null), "{}", s);
    assert!(d["phrase"].as_str().unwrap().contains("La DSN d'août 2026 est déposée et ne peut plus être remplacée"), "{}", d);

    // Sans confirmation : refuse, avec l'avertissement, et rien n'est ecrit.
    let avant = octets(&racine, &saisie).unwrap();
    let r = route(&racine, "enregistrer-saisie", json!({"mois": MOIS, "saisie": saisie_modifiee(&racine, 40)}), APRES);
    let (classe, texte) = refus(&r);
    assert_eq!(classe, "ModificationApresEcheance", "{}", r);
    assert!(texte.starts_with("La limite de la DSN d'août 2026 est passée : l'annule-et-remplace n'est plus admis depuis le "), "{}", texte);
    assert!(texte.contains("La DSN déposée ne changera pas : la correction se déclarera dans la DSN d'octobre 2026, une fois écrite par « Corriger ce mois »"), "{}", texte);
    assert!(texte.ends_with("Êtes-vous sûr de vouloir modifier cette paie ?"), "{}", texte);
    assert_eq!(octets(&racine, &saisie).unwrap(), avant);
    assert!(!racine.join(&copie).exists());

    // Ce qui ne change pas la paie (la meme saisie) n'a rien a confirmer.
    let meme = lire(&racine, &saisie);
    reussi(&route(&racine, "enregistrer-saisie", json!({"mois": MOIS, "saisie": meme}), APRES));
    assert!(!racine.join(&copie).exists());

    // Avec la confirmation : ecrite, et la saisie telle que declaree est gardee.
    let r = route(&racine, "enregistrer-saisie", json!({"mois": MOIS, "saisie": saisie_modifiee(&racine, 40), "modifier_apres_echeance": true}), APRES);
    assert_eq!(reussi(&r)["dsn_deposee"]["confirmer"], json!(true), "{}", r);
    assert_ne!(octets(&racine, &saisie).unwrap(), avant);
    assert_eq!(octets(&racine, &copie).unwrap(), avant);
    // Une seconde modification ne la remplace pas : elle reste la paie declaree.
    reussi(&route(&racine, "enregistrer-saisie", json!({"mois": MOIS, "saisie": saisie_modifiee(&racine, 41), "modifier_apres_echeance": true}), APRES));
    assert_eq!(octets(&racine, &copie).unwrap(), avant);
    reussi(&route(&racine, "enregistrer-saisie", json!({"mois": MOIS, "saisie": saisie_modifiee(&racine, 40), "modifier_apres_echeance": true}), APRES));

    // L'etape DSN le dit, en orange, tant que la correction n'est pas ecrite.
    let b = route(&racine, "bilan-du-mois", json!({"mois": MOIS}), APRES);
    let dsn = &reussi(&b)["dsn"];
    assert_eq!(dsn["couleur"], json!("alerte"), "{}", dsn);
    assert!(dsn["phrase"].as_str().unwrap().contains("1 contrat à corriger dans la DSN suivante (Corriger ce mois)"), "{}", dsn);
    assert_eq!(dsn["remplacement"]["correction_ecrite"], json!(false));

    // « Corriger ce mois » part de ce qui est declare, et propose ce qui est saisi.
    let e = lemercier(&racine);
    assert_eq!(e["engagement"]["lignes"][0]["quantite"], json!(32), "{}", e);
    assert_eq!(e["engagement_saisi"]["lignes"][0]["quantite"], json!(40), "{}", e);
    let r = route(
        &racine,
        "regularisation-ecrire",
        json!({"mois": MOIS, "salarie": "LEMERCIER", "debut": e["debut"], "engagement": {"lignes": e["engagement_saisi"]["lignes"]}, "motif": "heures de Lemercier"}),
        APRES,
    );
    assert_eq!(reussi(&r)["etat"], json!("en attente"), "{}", r);

    // La correction ecrite, l'etape n'a plus rien a faire : la DSN suivante l'embarque.
    let b = route(&racine, "bilan-du-mois", json!({"mois": MOIS}), APRES);
    let dsn = &reussi(&b)["dsn"];
    assert_eq!((dsn["couleur"].as_str(), dsn["remplacement"]["correction_ecrite"].as_bool()), (Some("bien"), Some(true)), "{}", dsn);
    assert!(dsn["phrase"].as_str().unwrap().contains("la correction est écrite (Corriger ce mois), elle part dans la DSN suivante"), "{}", dsn);
}

#[test]
fn retirer_un_engagement_apres_la_limite_se_confirme_aussi() {
    let racine = compagnie("ajout-retrait");
    let saisie = format!("paies/{}.json", MOIS);
    let avant = octets(&racine, &saisie).unwrap();
    let mut s = lire(&racine, &saisie);
    s["engagements"].as_array_mut().unwrap().pop();
    let r = route(&racine, "enregistrer-saisie", json!({"mois": MOIS, "saisie": s}), APRES);
    assert_eq!(refus(&r).0, "ModificationApresEcheance", "{}", r);
    assert_eq!(octets(&racine, &saisie).unwrap(), avant);
    reussi(&route(&racine, "enregistrer-saisie", json!({"mois": MOIS, "saisie": s, "modifier_apres_echeance": true}), APRES));
    // Un contrat retire n'est pas une correction : l'etape le dit, et ne se dit pas couverte.
    let b = route(&racine, "bilan-du-mois", json!({"mois": MOIS}), APRES);
    let dsn = &reussi(&b)["dsn"];
    assert_eq!(dsn["couleur"], json!("alerte"), "{}", dsn);
    assert!(dsn["phrase"].as_str().unwrap().contains("1 contrat ajouté ou retiré depuis la DSN"), "{}", dsn);
}

#[test]
fn avant_la_limite_la_modification_ne_demande_rien_et_l_annule_et_remplace_se_dit() {
    let racine = compagnie("avant-la-limite");
    let s = route(&racine, "saisie", json!({"mois": MOIS}), "2026-09-10");
    let d = reussi(&s)["dsn_deposee"].clone();
    assert_eq!(d["confirmer"], json!(false), "{}", s);
    let limite = paie::date::Jour::lire(d["limite"].as_str().unwrap()).unwrap();
    assert!(limite.iso().as_str() >= "2026-09-10", "{}", d);
    assert_eq!(
        d["phrase"],
        json!(format!("La DSN d'août 2026 est déposée : la modifier fera partir un annule-et-remplace, jusqu'au {} à minuit.", limite.jj_mm_aaaa()))
    );
    let r = route(&racine, "enregistrer-saisie", json!({"mois": MOIS, "saisie": saisie_modifiee(&racine, 40)}), "2026-09-10");
    reussi(&r);
    let ecrite = lire(&racine, &format!("paies/{}.json", MOIS));
    let rang = ecrite["engagements"].as_array().unwrap().iter().position(|e| e["salarie"] == "Lemercier").unwrap();
    assert_eq!(ecrite["engagements"][rang]["lignes"][0]["quantite"], json!(40));
    // Rien n'est garde : la DSN refaite remplacera la deposee.
    assert!(!racine.join(format!("regularisations/{}.saisie-declaree.json", MOIS)).exists());
}
