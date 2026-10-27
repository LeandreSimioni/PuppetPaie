//! Le modele de bulletin : l'apercu d'une paie FICTIVE sous chaque modele n'ecrit rien dans `logiciel/donnees/apercu-bulletin/`,
//! le choix de la compagnie vaut pour tous et la case d'un salarie l'emporte. Routes appelees dans le processus, sur
//! `fictive-3090-petite` (config, octobre 2026).

use crate::outils::{compagnie, dossier_de_test, ecrire, lire, route_en_processus};
use paie::bulletin::{jeux_et_description, Modele};
use droit::contexte::Contexte;
use socle::date::Jour;
use engagement::charger;
use paie::routes_bulletin_modele::{apercu_description, apercu_pdf, dossier_apercu};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

fn compagnie(nom: &str) -> PathBuf {
    compagnie::saisies("fictive-3090-petite", &["2026-10"], &dossier_de_test("routes_bulletin_modele", nom).join("compagnie"))
}

fn route(racine: &Path, nom: &str, charge: Value) -> Value {
    route_en_processus(racine, nom, charge, "2026-10-03")
}

fn texte(c: &Value) -> String {
    match c {
        Value::String(s) => s.clone(),
        Value::Object(o) => o.get("texte").and_then(Value::as_str).unwrap_or_default().to_string(),
        _ => String::new(),
    }
}

/// Les rangs du tableau des cotisations (l'en-tete d'abord).
fn rangs(desc: &Value) -> Vec<Vec<String>> {
    let blocs = desc["documents"][0]["blocs"].as_array().unwrap();
    let t = blocs.iter().find(|b| b["type"] == json!("tableau") && b.to_string().contains("MONTANT NET SOCIAL")).unwrap();
    t["entete"].as_array().unwrap().iter().chain(t["lignes"].as_array().unwrap()).map(|r| r["cellules"].as_array().unwrap().iter().map(texte).collect()).collect()
}

fn ligne<'a>(r: &'a [Vec<String>], libelle: &str) -> &'a Vec<String> {
    r.iter().find(|x| x[0] == libelle).unwrap_or_else(|| panic!("pas de {:?}", libelle))
}

/// Chaque fichier d'un dossier, avec sa date de modification et sa taille.
fn etat(d: &Path) -> Vec<(PathBuf, SystemTime, u64)> {
    let mut v = Vec::new();
    for e in std::fs::read_dir(d).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            v.extend(etat(&p));
        } else {
            let m = std::fs::metadata(&p).unwrap();
            v.push((p, m.modified().unwrap(), m.len()));
        }
    }
    v.sort();
    v
}

#[test]
fn l_apercu_est_une_paie_fictive_sous_chaque_modele_et_n_ecrit_rien() {
    let avant = etat(&dossier_apercu());
    let simple = apercu_description(Modele::Simplifie).unwrap();
    let complet = apercu_description(Modele::Complet).unwrap();
    let (s, c) = (rangs(&simple), rangs(&complet));
    assert_eq!(s[0].len(), 5, "le simplifie : les quatre colonnes du modele");
    assert_eq!(simple["documents"][0], complet["documents"][0], "le bulletin du complet est le simplifie");
    assert_eq!(simple["documents"].as_array().unwrap().len(), 1);
    assert_eq!(complet["documents"].as_array().unwrap().len(), 2, "le complet : le bulletin, puis son annexe");
    assert!(complet["documents"][1].to_string().contains("Taux employeur"), "l'annexe donne le taux employeur");
    for libelle in ["TOTAL VERSÉ PAR L'EMPLOYEUR", "MONTANT NET A PAYER (en Euros)", "MONTANT NET SOCIAL"] {
        assert_eq!(ligne(&s, libelle), ligne(&c, libelle), "{} : le meme sous les deux modeles", libelle);
    }
    // Fictive : la compagnie de l'apercu, un salarie sans NIR, ni le logo ni la signature du poste.
    let tout = simple.to_string();
    assert!(tout.contains("Compagnie fictive (aperçu du bulletin)"), "la compagnie fictive");
    assert!(tout.contains("en cours d'immatriculation"), "aucun NIR");
    assert!(simple.get("logo").is_none(), "pas le logo du poste");
    // Le PDF s'ecrit, et la route le rend en base64.
    assert!(apercu_pdf(Modele::Complet).unwrap().starts_with(b"%PDF"));
    let r = route(&compagnie("apercu"), "bulletin-modele-apercu", json!({"modele": "complet"}));
    assert_eq!(r["ok"], json!(true), "{}", r);
    let octets = paie::retours::base64_lire(r["resultat"]["pdf"].as_str().unwrap()).unwrap();
    assert!(octets.starts_with(b"%PDF"));
    // Rien n'a ete ecrit dans le dossier livre.
    assert_eq!(etat(&dossier_apercu()), avant, "l'apercu n'ecrit rien dans logiciel/donnees/apercu-bulletin");
}

#[test]
fn la_page_lit_les_modeles_et_la_compagnie_choisit_pour_tous() {
    let racine = compagnie("choisir");
    // Un salarie a demande le complet (sa case, dans Avance).
    let mut sal = lire(&racine, "config/salaries.json");
    sal["SERAPHIN"]["modele_bulletin"] = json!("complet");
    ecrire(&racine, "config/salaries.json", &sal);

    let r = route(&racine, "bulletin-modeles", json!({}));
    assert_eq!(r["ok"], json!(true), "{}", r);
    let m = &r["resultat"];
    assert_eq!(m["choisi"], json!("simplifié"));
    assert_eq!(m["par_defaut"], json!(true), "rien n'est choisi : le simplifie");
    let cles: Vec<&str> = m["modeles"].as_array().unwrap().iter().map(|x| x["cle"].as_str().unwrap()).collect();
    assert_eq!(cles, ["simplifié", "complet"]);
    assert_eq!(m["modeles"][1]["apercu"], json!("/apercu/bulletin?modele=complet"));
    assert_eq!(m["salaries_autrement"].as_array().unwrap().len(), 1);
    assert_eq!(m["salaries_autrement"][0]["cle"], json!("SERAPHIN"));
    assert_eq!(m["salaries_autrement"][0]["modele"], json!("complet"));

    // Un modele inconnu refuse, et rien n'est ecrit.
    let r = route(&racine, "bulletin-modele-choisir", json!({"modele": "détaillé"}));
    assert_eq!(r["ok"], json!(false), "{}", r);
    assert!(lire(&racine, "config/entreprise.json").get("modele_bulletin").is_none());

    // La compagnie choisit le complet : la fiche le dit (copie avant/ faite), la route le relit.
    let r = route(&racine, "bulletin-modele-choisir", json!({"modele": "complet"}));
    assert_eq!(r["ok"], json!(true), "{}", r);
    assert_eq!(r["resultat"]["choisi"], json!("complet"));
    assert_eq!(r["resultat"]["par_defaut"], json!(false));
    assert_eq!(r["resultat"]["salaries_autrement"], json!([]), "le salarie qui demandait le complet ne s'ecarte plus");
    assert_eq!(lire(&racine, "config/entreprise.json")["modele_bulletin"], json!("complet"));
    assert!(std::fs::read_dir(racine.join("config/avant")).unwrap().flatten().any(|e| e.file_name().to_string_lossy().starts_with("entreprise.json.")));

    // Le bulletin suit le choix enregistre ; la case d'un salarie qui demande le simplifie l'emporte.
    let ctx = Contexte::nouveau(&racine, Some(Jour::lire("2026-10-28").unwrap())).unwrap();
    let s = charger(&ctx, "2026-10").unwrap();
    let ent = ctx.entreprise_du_mois("2026-10").unwrap();
    let mut p = s.paies.iter().find(|p| !p.engagement.sans_paie()).unwrap().clone();
    let documents = |p: &engagement::calcul::Paie| jeux_et_description(&ctx, std::slice::from_ref(p), &ent, "2026-10", None, false, &HashMap::new()).unwrap()["documents"].as_array().unwrap().len();
    assert_eq!(documents(&p), 2, "le choix de la compagnie vaut pour tous : le bulletin et son annexe");
    p.salarie["modele_bulletin"] = json!("simplifié");
    assert_eq!(documents(&p), 1, "la case du salarie l'emporte");

    // Retour au simplifie.
    let r = route(&racine, "bulletin-modele-choisir", json!({"modele": "simplifié"}));
    assert_eq!(r["resultat"]["choisi"], json!("simplifié"));
    assert_eq!(r["resultat"]["salaries_autrement"][0]["cle"], json!("SERAPHIN"));
}
