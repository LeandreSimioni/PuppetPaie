//! Le coffre par la route : `coffre-poser`, `coffre-compte-dpae`, `coffre-oublier`, le mot de passe du logiciel
//! (`mot-de-passe-changer`, `mot-de-passe-retirer`) et `google-oublier`. Aucun secret ne sort d'une reponse ; ce qui est
//! trop court ou inconnu refuse. Sur la configuration de `demo-3090-petite`, mots de passe inventes, cle de poste a part.

use crate::outils::{compagnie, dossier_de_test, ecrire, lire, refus, reussi};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn compagnie(nom: &str) -> PathBuf {
    compagnie::saisies("demo-3090-petite", &[], &dossier_de_test("routes_coffre", nom).join("compagnie"))
}

fn route(racine: &Path, nom: &str, charge: Value) -> Value {
    crate::outils::route(racine, nom, charge, "2026-10-03")
}

fn service<'a>(etat: &'a Value, nom: &str) -> &'a Value {
    etat["services"].as_array().unwrap().iter().find(|s| s["service"] == json!(nom)).unwrap()
}

#[test]
fn un_secret_se_pose_et_s_oublie_sans_jamais_sortir() {
    let racine = compagnie("coffre");
    let secret = "motdepasse-inventé-4711";
    let r = route(&racine, "coffre-poser", json!({"service": "urssaf", "valeur": secret}));
    assert!(!r.to_string().contains(secret), "une valeur de secret est sortie");
    assert_eq!(service(reussi(&r), "urssaf")["pose"], json!(true), "{}", r);
    assert_eq!(service(reussi(&r), "net-entreprises")["pose"], json!(false));
    assert!(racine.join("config").join("coffre.bin").exists());
    // Trop court, service inconnu : refuses, rien n'est pose.
    refus(&route(&racine, "coffre-poser", json!({"service": "net-entreprises", "valeur": "abc"})));
    refus(&route(&racine, "coffre-poser", json!({"service": "banque", "valeur": "unmotdepasse1"})));
    assert_eq!(service(reussi(&route(&racine, "coffre-etat", json!({}))), "net-entreprises")["pose"], json!(false), "rien n'est pose");
    let r = route(&racine, "coffre-oublier", json!({"service": "urssaf"}));
    assert_eq!(service(reussi(&r), "urssaf")["pose"], json!(false), "{}", r);
    // Oublier ce qui n'est pas pose n'est pas une erreur.
    let r = route(&racine, "coffre-oublier", json!({"service": "urssaf"}));
    assert_eq!(service(reussi(&r), "urssaf")["pose"], json!(false), "{}", r);
}

#[test]
fn la_boite_courriel_garde_son_adresse() {
    let racine = compagnie("courriel");
    let r = route(&racine, "coffre-poser", json!({"service": "courriel", "valeur": "courriel-secret-1", "compte": "boite@demo.invalid"}));
    let s = service(reussi(&r), "courriel");
    assert_eq!(s["pose"], json!(true), "{}", r);
    assert!(r.to_string().contains("boite@demo.invalid"), "{}", r);
    assert!(!r.to_string().contains("courriel-secret-1"));
}

#[test]
fn le_compte_dpae_perd_ses_accents_et_garde_les_autres_cles() {
    let racine = compagnie("dpae");
    let mut ent = lire(&racine, "config/entreprise.json");
    ent["api_dpae"] = json!({"service": "urssaf", "note": "a garder", "nom": "ANCIEN"});
    ecrire(&racine, "config/entreprise.json", &ent);
    let r = route(&racine, "coffre-compte-dpae", json!({"nom": "  Éloï   de  la  Rôche ", "prenom": "Zoë-Hélène"}));
    reussi(&r);
    let api = lire(&racine, "config/entreprise.json")["api_dpae"].clone();
    assert_eq!(api, json!({"service": "urssaf", "note": "a garder", "nom": "Eloi de la Roche", "prenom": "Zoe-Helene"}));
    // Un nom vide refuse et ne modifie rien.
    let avant = std::fs::read(racine.join("config").join("entreprise.json")).unwrap();
    let (_, m) = refus(&route(&racine, "coffre-compte-dpae", json!({"nom": "   ", "prenom": "Jeanne"})));
    assert!(m.contains("nom du compte Urssaf manque"), "{}", m);
    assert_eq!(std::fs::read(racine.join("config").join("entreprise.json")).unwrap(), avant);
}

#[test]
fn le_mot_de_passe_du_logiciel_devient_exige_puis_se_retire() {
    let racine = compagnie("mdp");
    let (un, deux) = ("logiciel-un-123", "logiciel-deux-456");
    // Deux saisies qui different, ou trop courtes : refus.
    let (_, m) = refus(&route(&racine, "mot-de-passe-changer", json!({"nouveau": un, "confirmation": "autre"})));
    assert!(m.contains("different"), "{}", m);
    refus(&route(&racine, "mot-de-passe-changer", json!({"nouveau": "abc", "confirmation": "abc"})));
    reussi(&route(&racine, "mot-de-passe-changer", json!({"nouveau": un, "confirmation": un})));
    // Desormais, poser un secret exige le mot de passe.
    let poser = |mdp: Option<&str>| {
        let mut c = json!({"service": "urssaf", "valeur": "urssaf-secret-1"});
        if let Some(m) = mdp {
            c["motdepasse"] = json!(m);
        }
        route(&racine, "coffre-poser", c)
    };
    assert_eq!(refus(&poser(None)).0, "PreuveInsuffisante");
    assert_eq!(refus(&poser(Some("faux-faux-faux"))).0, "PreuveInsuffisante");
    assert_eq!(service(reussi(&poser(Some(un))), "urssaf")["pose"], json!(true));
    // Changer exige l'actuel.
    assert_eq!(refus(&route(&racine, "mot-de-passe-changer", json!({"nouveau": deux, "confirmation": deux}))).0, "PreuveInsuffisante");
    reussi(&route(&racine, "mot-de-passe-changer", json!({"nouveau": deux, "confirmation": deux, "motdepasse": un})));
    assert_eq!(refus(&poser(Some(un))).0, "PreuveInsuffisante");
    // Le secret pose avant le changement est toujours la.
    assert_eq!(service(reussi(&poser(Some(deux))), "urssaf")["pose"], json!(true));
    // Retirer : l'actuel est exige, puis plus rien ne l'est.
    assert_eq!(refus(&route(&racine, "mot-de-passe-retirer", json!({"motdepasse": "faux-faux-faux"}))).0, "PreuveInsuffisante");
    reussi(&route(&racine, "mot-de-passe-retirer", json!({"motdepasse": deux})));
    assert_eq!(service(reussi(&poser(None)), "urssaf")["pose"], json!(true));
}

#[test]
fn google_oublier_efface_le_jeton_local() {
    let racine = compagnie("google");
    assert_eq!(reussi(&route(&racine, "google-oublier", json!({}))), &json!({"efface": false}));
    std::fs::write(racine.join("config").join("google-jeton.bin"), b"jeton factice").unwrap();
    assert_eq!(reussi(&route(&racine, "google-oublier", json!({}))), &json!({"efface": true}));
    assert!(!racine.join("config").join("google-jeton.bin").exists());
}
