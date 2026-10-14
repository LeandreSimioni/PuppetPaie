//! `paie route` : `arret-contrats`, `dossier-formes` et `exports-periodes`, qui ne font que lire. Sur
//! `fictive-3090-petite` (config, janvier a mars 2025, journal vide), que chaque test mute ; un fichier abime refuse.

use crate::outils::{compagnie, fictive, dossier_de_test, ecrire, lire, reussi, route};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn compagnie(nom: &str) -> PathBuf {
    compagnie::saisies("fictive-3090-petite", &["2025-01", "2025-02", "2025-03"], &dossier_de_test("routes_dossiers", nom).join("compagnie"))
}

fn resultat(c: &Path, nom: &str, charge: Value) -> Value {
    reussi(&route(c, nom, charge, "2026-10-03")).clone()
}

fn refuse(c: &Path, nom: &str, charge: Value) -> bool {
    route(c, nom, charge, "2026-10-03")["ok"] == json!(false)
}

fn ecrire_texte(c: &Path, rel: &str, t: &str) {
    std::fs::write(c.join(rel), t).unwrap();
}

// ---------------------------------------------------------------- arret-contrats

fn contrats_de(c: &Path, salarie: &str, jour: &str) -> Vec<Value> {
    resultat(c, "arret-contrats", json!({"salarie": salarie, "jour": jour}))["contrats"].as_array().unwrap().clone()
}

#[test]
fn un_jour_dans_un_contrat_rend_ce_contrat_avec_son_mois() {
    let c = &compagnie("ac-un");
    let r = contrats_de(c, "BRIOCHE", "2025-01-22");
    assert_eq!(r.len(), 1);
    assert_eq!(r[0]["salarie"], json!("BRIOCHE"));
    assert_eq!(r[0]["numero"], json!("BR1.25.01"));
    assert_eq!(r[0]["debut"], json!("2025-01-21"));
    assert_eq!(r[0]["fin"], json!("2025-01-24"));
    assert_eq!(r[0]["mois"], json!("2025-01"));
    assert_eq!(r[0]["spectacle"], json!("256Z00920011"));
    let nom = lire(c, "config/salaries.json")["BRIOCHE"]["nom"].as_str().unwrap().to_string();
    assert!(r[0]["nom"].as_str().unwrap().starts_with(&nom));
}

#[test]
fn un_jour_hors_contrat_ou_un_autre_format_de_date() {
    let c = &compagnie("ac-hors");
    assert!(contrats_de(c, "BRIOCHE", "2025-01-25").is_empty());
    assert_eq!(contrats_de(c, "brioche", "22/01/2025").len(), 1, "le nom approchant et la date a la francaise se lisent");
}

#[test]
fn un_contrat_declare_deux_fois_ne_sort_qu_une_fois_et_sans_numero_il_sort_vide() {
    let c = &compagnie("ac-doublon");
    let mut d = lire(c, "paies/2025-01.json");
    let e0 = d["engagements"][0].clone();
    d["engagements"].as_array_mut().unwrap().push(e0);
    let n = d["engagements"].as_array().unwrap().len();
    d["engagements"][0].as_object_mut().unwrap().remove("numero");
    d["engagements"][n - 1].as_object_mut().unwrap().remove("numero");
    ecrire(c, "paies/2025-01.json", &d);
    let r = contrats_de(c, "BRIOCHE", "2025-01-22");
    assert_eq!(r.len(), 1);
    assert_eq!(r[0]["numero"], json!(""));
}

#[test]
fn une_charge_fausse_ou_un_mois_abime_refuse() {
    let c = &compagnie("ac-refus");
    for charge in [json!({"jour": "2025-01-22"}), json!({"salarie": "BRIOCHE"}), json!({"salarie": "Zzzzzzzzzz", "jour": "2025-01-22"}), json!({"salarie": "BRIOCHE", "jour": "pas une date"})] {
        assert!(refuse(c, "arret-contrats", charge.clone()), "{}", charge);
    }
    let r = route(c, "arret-contrats", json!({"salarie": "Zzzzzzzzzz", "jour": "2025-01-22"}), "2026-10-03");
    assert!(r["erreur"].as_str().unwrap().contains("Salarie inconnu"), "{}", r);
    ecrire_texte(c, "paies/2025-02.json", "[1, 2");
    assert!(refuse(c, "arret-contrats", json!({"salarie": "BRIOCHE", "jour": "2025-01-22"})));
}

// ---------------------------------------------------------------- dossier-formes

fn formes(c: &Path) -> Value {
    resultat(c, "dossier-formes", json!({}))
}

#[test]
fn les_formes_listent_les_salaries_les_categories_et_les_mois() {
    let c = &compagnie("df-base");
    let r = formes(c);
    let cles: Vec<&str> = r["salaries"].as_array().unwrap().iter().map(|s| s["cle"].as_str().unwrap()).collect();
    assert_eq!(cles, ["BRIOCHE", "LEMERCIER", "SERAPHIN"], "dans l'ordre des cles");
    assert_eq!(r["salaries"][0]["actif"], json!(true));
    assert_eq!(r["salaries"][0]["email"], json!(true));
    assert!(r["salaries"][0]["nom"].as_str().unwrap().starts_with("BRIOCHE "));
    assert_eq!(r["categories"].as_array().unwrap().len(), 7);
    assert_eq!(r["categories"][2], json!({"categorie": "contrats-signes", "quoi": "Contrats signés"}));
    assert_eq!(r["mois"], json!(["2025-01", "2025-02", "2025-03"]));
    assert_eq!(r["annees"], json!(["2025"]));
    assert_eq!(r["boite_liee"], json!(false), "ni compagnie fictive, ni mot de passe de boite au coffre");
    assert!(r["dossier"].as_str().unwrap().ends_with("/exports/dossiers"));
}

#[test]
fn les_mois_declares_avant_le_logiciel_s_ajoutent_et_les_annees_se_rangent_du_plus_recent() {
    let c = &compagnie("df-historique");
    ecrire(c, "historique/paies.json", &json!({"paies": [{"mois": "2023-11"}, {"mois": "2023-12"}, {"mois": "2025-01"}, {"mois": ""}, {}]}));
    let r = formes(c);
    assert_eq!(r["mois"], json!(["2023-11", "2023-12", "2025-01", "2025-02", "2025-03"]));
    assert_eq!(r["annees"], json!(["2025", "2023"]));
}

#[test]
fn une_fiche_archivee_reste_proposee_sans_courriel() {
    let c = &compagnie("df-fiche");
    let mut s = lire(c, "config/salaries.json");
    s["BRIOCHE"]["actif"] = json!(false);
    s["BRIOCHE"].as_object_mut().unwrap().remove("email");
    s["BRIOCHE"]["prenom_affichage"] = json!("Zoe");
    ecrire(c, "config/salaries.json", &s);
    let r = formes(c);
    assert_eq!(r["salaries"][0]["actif"], json!(false));
    assert_eq!(r["salaries"][0]["email"], json!(false));
    assert!(r["salaries"][0]["nom"].as_str().unwrap().ends_with(" Zoe"));
}

#[test]
fn la_boite_est_liee_pour_une_compagnie_de_fictive_seulement() {
    let c = &compagnie("df-boite");
    assert_eq!(formes(c)["boite_liee"], json!(false));
    std::fs::copy(fictive("fictive-3090-petite").join("COPIE-DE-DEVELOPPEMENT.json"), c.join("COPIE-DE-DEVELOPPEMENT.json")).unwrap();
    assert_eq!(formes(c)["boite_liee"], json!(true), "une compagnie fictive joue ses envois");
}

#[test]
fn les_formes_refusent_quand_la_compta_ou_l_historique_est_abime() {
    let c = &compagnie("df-compta");
    // Un logiciel connu et un plan dont le modele de compte ne se remplit pas : la page l'apprend par
    // `etat_logiciel`, pas par un refus.
    ecrire(c, "config/compta.json", &json!({"logiciel": "quadra"}));
    ecrire(c, "config/plan-comptable.json", &json!({"net": {"compte": "421000", "par_salarie": "421{x}"}}));
    assert_eq!(formes(c)["mois"].as_array().unwrap().len(), 3);
    for plan in [json!([1]), json!({"pas": {}})] {
        ecrire(c, "config/plan-comptable.json", &plan);
        assert!(refuse(c, "dossier-formes", json!({})), "{}", plan);
    }
    // Un logiciel sans limite de zone (le FEC) ne regarde pas le plan.
    ecrire(c, "config/compta.json", &json!({"logiciel": "autre"}));
    assert_eq!(formes(c)["mois"].as_array().unwrap().len(), 3);
    for compta in [json!([1]), json!({"logiciel": ["x"]})] {
        ecrire(c, "config/compta.json", &compta);
        assert!(refuse(c, "dossier-formes", json!({})), "{}", compta);
    }
    ecrire_texte(c, "config/compta.json", "{abime");
    assert!(refuse(c, "dossier-formes", json!({})));
    std::fs::remove_file(c.join("config/compta.json")).unwrap();
    // L'historique importe abime.
    for h in [json!([1]), json!({"paies": [5]}), json!({"paies": 5}), json!({"paies": [{"mois": 5}, {"mois": "2020-01"}]}), json!({"paies": [{"mois": ["a"]}]})] {
        ecrire(c, "historique/paies.json", &h);
        assert!(refuse(c, "dossier-formes", json!({})), "{}", h);
    }
}

// -------------------------------------------------------------- exports-periodes

fn periodes(c: &Path, charge: Value) -> Value {
    resultat(c, "exports-periodes", charge)
}

#[test]
fn les_periodes_disent_les_mois_leur_source_les_etats_et_les_faits_du_poste() {
    let c = &compagnie("ep-base");
    ecrire(c, "historique/paies.json", &json!({"paies": [{"mois": "2024-12"}, {"mois": "2025-01"}]}));
    let r = periodes(c, json!({"xlsx": true, "pdf": false}));
    assert_eq!(r["mois"], json!(["2024-12", "2025-01", "2025-02", "2025-03"]));
    // Reste R1 #146 : un mois de l'historique n'est pas une DSN deposee PAR PuppetPaie (§216) : il est repris.
    assert_eq!(r["sources"], json!({"2024-12": "repris de l'historique", "2025-01": "saisie", "2025-02": "saisie", "2025-03": "saisie"}));
    assert_eq!(r["annees"], json!(["2025", "2024"]));
    assert_eq!(r["xlsx"], json!(true));
    assert_eq!(r["pdf"], json!(false));
    assert!(r["dossier"].as_str().unwrap().ends_with("/exports"));
    assert_eq!(r["etats"][0]["etat"], json!("livre-de-paie"));
    assert_eq!(r["etats"][0]["famille"], json!("États de paie"));
    assert_eq!(r["etats"][0]["formats"], json!(["csv", "xlsx", "pdf"]));
    assert_eq!(r["formats"][0]["format"], json!("csv"));
    assert_eq!(r["regroupements"].as_array().unwrap().len(), 4);
    assert_eq!(r["compta"]["logiciel"], json!(""));
    assert_eq!(r["compta"]["controle"], json!([]));
    assert_eq!(r["compta"]["logiciels"].as_array().unwrap().len(), 11);
    // Sans rien dans la charge, les deux faits sont faux.
    let r = periodes(c, json!({}));
    assert_eq!((r["xlsx"].clone(), r["pdf"].clone()), (json!(false), json!(false)));
}

fn noms_des_etats(r: &Value) -> Vec<String> {
    r["etats"].as_array().unwrap().iter().map(|e| e["etat"].as_str().unwrap().to_string()).collect()
}

#[test]
fn la_page_ne_propose_que_les_etats_qui_concernent_la_compagnie() {
    let c = &compagnie("ep-etats");
    let mut e = lire(c, "config/entreprise.json");
    e["idcc"] = json!("3090");
    e.as_object_mut().unwrap().remove("emploie_des_permanents");
    ecrire(c, "config/entreprise.json", &e);
    let r = noms_des_etats(&periodes(c, json!({})));
    assert!(r.contains(&"fcap-svp".to_string()));
    assert!(!r.contains(&"afdas-2642".to_string()), "une 3090 n'a pas l'Afdas 2642");
    assert!(!r.contains(&"provision-conges".to_string()), "sans permanents, pas de dette de conges");
    e["idcc"] = json!("1285");
    e["emploie_des_permanents"] = json!(true);
    ecrire(c, "config/entreprise.json", &e);
    let r = noms_des_etats(&periodes(c, json!({})));
    assert!(!r.contains(&"fcap-svp".to_string()));
    assert!(r.contains(&"provision-conges".to_string()));
}

#[test]
fn le_controle_du_plan_comptable_se_lit_dans_la_page() {
    let c = &compagnie("ep-plan");
    ecrire(c, "config/compta.json", &json!({"logiciel": "quadra", "analytique": 1}));
    ecrire(c, "config/plan-comptable.json", &json!({"journal": "ODXXXX"}));
    let r = periodes(c, json!({}));
    assert_eq!(r["compta"]["logiciel"], json!("quadra"));
    assert_eq!(r["compta"]["analytique"], json!(true));
    assert!(r["compta"]["controle"].as_array().unwrap().iter().any(|m| m.as_str().unwrap() == "Quadra : le journal « ODXXXX » a 6 caractères, la zone en prend 3."));
}

#[test]
fn un_modele_de_compte_illisible_est_refuse_en_le_disant() {
    let c = &compagnie("ep-format");
    ecrire(c, "config/compta.json", &json!({"logiciel": "sage"}));
    let modele = |m: &str| ecrire(c, "config/plan-comptable.json", &json!({"net": {"compte": "421000", "par_salarie": m}}));
    modele("{code:e}");
    let r = route(c, "exports-periodes", json!({}), "2026-10-03");
    assert!(r["erreur"].as_str().unwrap().contains("ne prend pas en charge"), "{}", r);
    for m in ["{code:010,}", "{code[0]}", "{cle[x]}", "{cle.foo}"] {
        modele(m);
        assert!(refuse(c, "exports-periodes", json!({})), "{}", m);
    }
    // Ce que `compte_du_salarie` rattrape devient une phrase du controle, pas un refus.
    modele("421{x}");
    assert!(periodes(c, json!({}))["compta"]["controle"][0].as_str().unwrap().starts_with("Le modèle du compte individuel « 421{x} »"));
    modele("421{code:03}{cle:>9}");
    assert!(periodes(c, json!({}))["compta"]["controle"].as_array().unwrap().iter().all(|m| !m.as_str().unwrap().contains("ne se remplit pas")));
}
