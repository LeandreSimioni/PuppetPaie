//! `paie route organismes-contrats` et `organismes-enregistrer` : les contrats de la DSN que l'ecran montre, et ce que
//! l'enregistrement compose. Ticket #214 : une compagnie qui n'emploie pas de permanent ne se voit pas proposer leurs
//! contrats. Sur `fictive-3090-petite` (que des intermittents) et `fictive-3090-grande` (des permanents).

use crate::outils::{compagnie, dossier_de_test, ecrire, lire, reussi, route};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const JOUR: &str = "2026-10-20";

fn compagnie(fictive: &str, nom: &str) -> PathBuf {
    compagnie::saisies(fictive, &[], &dossier_de_test("routes_organismes", nom).join("compagnie"))
}

fn cles(c: &Path) -> Vec<String> {
    reussi(&route(c, "organismes-contrats", json!({}), JOUR))["contrats"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["cle"].as_str().unwrap().to_string())
        .collect()
}

fn poser_permanents(c: &Path, oui: bool) {
    let mut e = lire(c, "config/entreprise.json");
    e["emploie_des_permanents"] = json!(oui);
    ecrire(c, "config/entreprise.json", &e);
}

/// Les contrats des permanents (prevoyance et sante, `role` de `donnees/organismes-base.json`) suivent le curseur
/// `emploie_des_permanents` de la fiche Entreprise, le meme qui ouvre les natures payees au mois.
#[test]
fn les_contrats_des_permanents_ne_se_montrent_qu_a_qui_en_emploie() {
    let c = &compagnie("fictive-3090-petite", "sans-permanent");
    poser_permanents(c, false);
    let sans = cles(c);
    assert!(sans.iter().any(|k| k == "interbranche"), "{:?}", sans);
    assert!(!sans.iter().any(|k| k == "prevoyance_permanents" || k == "sante_permanents"), "{:?}", sans);
    poser_permanents(c, true);
    let avec = cles(c);
    assert!(avec.iter().any(|k| k == "prevoyance_permanents") && avec.iter().any(|k| k == "sante_permanents"), "{:?}", avec);
    // Rien d'autre ne bouge : les lignes des intermittents sont les memes.
    assert_eq!(avec.iter().filter(|k| !k.ends_with("_permanents")).collect::<Vec<_>>(), sans.iter().collect::<Vec<_>>());
}

/// Sans permanent, l'enregistrement passe sans la reference de leur prevoyance, et ne perd rien de ce qui etait
/// declare : la sante des permanents de la 3090 (« declaree meme sans permanent ») et une prevoyance deja saisie restent
/// dans config/organismes.json, avec leurs identifiants.
#[test]
fn sans_permanent_l_enregistrement_garde_ce_qui_etait_declare() {
    let c = &compagnie("fictive-3090-petite", "enregistrer");
    poser_permanents(c, false);
    let avant = lire(c, "config/organismes.json");
    let reussite = route(c, "organismes-enregistrer", json!({"contrats": {}}), JOUR);
    reussi(&reussite);
    let apres = lire(c, "config/organismes.json");
    let refs = |v: &Value| -> Vec<String> {
        v["adhesions_prevoyance"].as_array().unwrap().iter().map(|a| a["reference"].as_str().unwrap().to_string()).collect()
    };
    for r in ["101", "106"] {
        assert!(refs(&avant).contains(&r.to_string()) && refs(&apres).contains(&r.to_string()), "{} : {:?}", r, refs(&apres));
    }
    assert_eq!(apres["affiliations_par_nature"]["permanent"]["references"], json!(["106", "101"]));
}

/// UNE COMPAGNIE QUI EMPLOIE DES PERMANENTS DIT LA REFERENCE DE LEUR PREVOYANCE (#236) : enregistrer sans elle passait
/// sans un mot, et le mois de son permanent se refusait ensuite (`dsn::affiliations` : « affiliations_par_nature.
/// permanent.references est null »). L'enregistrement se refuse, rien n'est ecrit ; avec elle, il passe.
#[test]
fn avec_permanents_la_reference_de_leur_prevoyance_est_exigee() {
    let c = &compagnie("fictive-3090-petite", "avec-permanents");
    std::fs::remove_file(c.join("config/organismes.json")).unwrap();
    poser_permanents(c, true);
    let r = route(c, "organismes-enregistrer", json!({"contrats": {}}), JOUR);
    assert_eq!(r["ok"], json!(false), "{}", r);
    assert!(!c.join("config/organismes.json").exists(), "rien n'est ecrit");
    let r = route(c, "organismes-enregistrer", json!({"contrats": {"prevoyance_permanents": {"libelle": "PREV PERM"}}}), JOUR);
    reussi(&r);
    let refs = lire(c, "config/organismes.json")["affiliations_par_nature"]["permanent"]["references"].clone();
    assert!(refs.as_array().is_some_and(|a| a.len() == 2), "{}", refs);
}
