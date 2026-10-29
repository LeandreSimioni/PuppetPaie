//! UNE COMPAGNIE NEUVE QUI REPREND LES BULLETINS DE SON LOGICIEL PRECEDENT (#236). Ce que le moteur en decide :
//!
//! - l'apercu de l'import annonce, pour une fiche a creer, les anomalies que l'ecriture trouvera, et elles seules
//!   (`crate::routes::fiches::verifier_comme_a_l_ecriture`) : un nom ou une adresse que le bulletin imprime en minuscules
//!   (« Compagnie des Essais ») va dans la case qu'on lit, la forme declaree s'en derive ; il n'est pas « a mettre en
//!   majuscules » ;
//! - l'emploi imprime qui nomme un metier du catalogue, et un seul, donne a la fiche ce que le choix de ce metier
//!   donne (le code PCS-ESE : `emplois::par_intitule`) ;
//! - la nature du contrat qu'aucun bulletin n'imprime : un bulletin qui porte une marque du spectacle (numero
//!   d'objet, cotisation Congés Spectacles, due pour l'artiste ou le technicien du spectacle, D. 7121-41 du code du
//!   travail, `atelier/reference/sources/caisse-conges-affiliation/LISEZ-MOI.md`) laisse le contrat muet, un CDDU
//!   (regle 6) ; sans aucune, le contrat est celui d'un permanent, sa nature se demande apres l'import
//!   (`importation-etat`, `a_dire`) et se dit par `importation-nature`, qui refuse une nature qui n'est pas d'un
//!   permanent sans rien ecrire.
//!
//! Chaque test cree sa compagnie par `compagnies::ajouter_dans`, comme le « + ».

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use moteur::interface::compagnies::ajouter_dans;
use serde_json::{json, Value};

use crate::bulletins_fictifs::{a_heures, a_repetitions, page_a, pdf, A};
use crate::outils::{dossier_de_test, fiche_au, lire, reussi};

const JOUR: &str = "2026-10-03";

fn route(racine: &Path, nom: &str, charge: Value) -> Value {
    crate::outils::route(racine, nom, charge, JOUR)
}

/// La fiche entreprise ecrite, lue au jour du test.
fn entreprise_lue(racine: &Path) -> Value {
    fiche_au(&lire(racine, "config/entreprise.json"), JOUR)
}

/// Les fiches des salaries ecrites, lues au jour du test.
fn salaries_lus(racine: &Path) -> Value {
    let brutes = lire(racine, "config/salaries.json");
    Value::Object(brutes.as_object().unwrap().iter().filter(|(k, _)| !k.starts_with('_')).map(|(k, v)| (k.clone(), fiche_au(v, JOUR))).collect())
}

/// Les contrats attestes des engagements importes, chacun avec son salarie.
fn contrats_importes(racine: &Path) -> Vec<Value> {
    engagement::importe::contrats_lus(racine).unwrap()
}

/// Les fichiers des engagements, tels qu'ils sont sur le disque.
fn engagements_ecrits(racine: &Path) -> Vec<(String, Vec<u8>)> {
    let mut v: Vec<(String, Vec<u8>)> = std::fs::read_dir(racine.join("engagements"))
        .unwrap()
        .flatten()
        .filter(|e| e.path().is_file())
        .map(|e| (e.file_name().to_string_lossy().into_owned(), std::fs::read(e.path()).unwrap()))
        .collect();
    v.sort();
    v
}

fn neuve(test: &str) -> PathBuf {
    ajouter_dans(&dossier_de_test("import_neuve", test), "Compagnie des Essais").unwrap().racine
}

/// L'artiste, au cachet, sur un spectacle (numero d'objet, Congés Spectacles) ; son adresse en minuscules.
fn artiste() -> A {
    let mut a = a_repetitions();
    a.id.adresse = ["3 impasse des Essais", "99999 VILLE FICTIVE"];
    a
}

/// Un permanent (NIR et nom inventes) : ses heures, ni numero d'objet ni Congés Spectacles.
fn permanent() -> A {
    let mut a = a_heures();
    a.id.nom = "TARTEMPION";
    a.id.prenom = "ZOE";
    a.id.nir = "185079999912388";
    a.id.code = "8";
    a.id.naissance = "14/07/1985";
    a.id.conges_spectacles = None;
    a.cotisations.retain(|c| c.2 != "conges_spectacles");
    a.total = ["178,03", "112,18"];
    a.cumuls[3] = ["818,03", "2 622,50"];
    a.total_verse = "818,03";
    a
}

fn deposer(racine: &Path) {
    let import = racine.join("import");
    std::fs::create_dir_all(&import).unwrap();
    pdf(vec![page_a(&artiste())], &import.join("artiste.pdf"));
    pdf(vec![page_a(&permanent())], &import.join("permanent.pdf"));
}

fn fiches_a_creer(apercu: &Value) -> Vec<(String, &Value)> {
    let mut out = vec![("entreprise".to_string(), &apercu["entreprise"])];
    for s in apercu["salaries"].as_array().unwrap() {
        out.push(("salarie".to_string(), s));
    }
    out
}

/// L'apercu annonce les anomalies que l'ecriture trouvera, et aucune sur une case que le bulletin remplit.
#[test]
fn l_apercu_annonce_ce_que_l_ecriture_trouvera() {
    let racine = neuve("apercu");
    deposer(&racine);
    let apercu = reussi(&route(&racine, "importation-apercu", json!({"dossier": ""}))).clone();
    let a_creer = fiches_a_creer(&apercu);
    assert_eq!(a_creer.len(), 3, "{}", apercu);
    for (nom, f) in &a_creer {
        assert_eq!(f["etat"], json!("a_creer"), "{}", f);
        for a in f["anomalies"].as_array().unwrap() {
            let champ = a["champ"].as_str().unwrap();
            let lu = &f["donnees"][champ];
            assert!(lu.is_null() || lu == &json!("") || lu == &json!([]), "{} : {} est rempli ({}) et pourtant signale : {}", nom, champ, lu, a);
        }
    }
    let r = reussi(&route(&racine, "importer", json!({"dossier": ""}))).clone();
    let entreprise = entreprise_lue(&racine);
    let salaries = salaries_lus(&racine);
    for (nom, f) in &a_creer {
        let ecrite = if nom == "entreprise" {
            entreprise.clone()
        } else {
            let nir = f["donnees"]["nir"].clone();
            salaries.as_object().unwrap().values().find(|s| s["nir"] == nir).cloned().unwrap_or_else(|| panic!("{} non ecrit : {}", nir, r))
        };
        let v = reussi(&route(&racine, "verifier-fiche", json!({"nom": nom, "donnees": ecrite}))).clone();
        let erreurs = |x: &Value| -> BTreeSet<String> {
            x.as_array().unwrap().iter().filter(|a| a["gravite"] == json!("erreur")).map(|a| a["champ"].as_str().unwrap().to_string()).collect()
        };
        assert_eq!(erreurs(&f["anomalies"]), erreurs(&v["anomalies"]), "{} : l'apercu et la fiche ecrite ne disent pas la meme chose", nom);
    }
    // L'emploi imprime « ARTISTE DRAMATIQUE » nomme un metier du catalogue : la fiche a son code PCS-ESE.
    let brioche = salaries.as_object().unwrap().values().find(|s| s["nir"] == json!("2881175999001")).unwrap();
    assert_eq!(brioche["pcs_ese"], json!("354c"), "{}", brioche);
    assert_eq!(brioche["statut"], json!("artiste"));
}

/// La nature : muette (un CDDU) pour l'artiste ; a dire pour le permanent, et seulement une nature de permanent.
#[test]
fn la_nature_d_un_contrat_de_permanent_se_demande_apres_l_import() {
    let racine = neuve("nature");
    deposer(&racine);
    reussi(&route(&racine, "importer", json!({"dossier": ""})));
    let salaries = salaries_lus(&racine);
    let cle = |nir: &str| salaries.as_object().unwrap().iter().find(|(_, s)| s["nir"] == json!(nir)).map(|(k, _)| k.clone()).unwrap();
    let (artiste, permanent) = (cle("2881175999001"), cle("1850799999123"));
    let engagements = || contrats_importes(&racine);
    for e in engagements() {
        if e["salarie"] == json!(artiste) {
            assert!(e.get("nature").is_none() && e.get("nature_a_dire").is_none(), "{}", e);
        }
    }
    let etat = reussi(&route(&racine, "importation-etat", json!({}))).clone();
    let a_dire = etat["a_dire"].as_array().unwrap();
    assert_eq!(a_dire.len(), 1, "{}", etat["a_dire"]);
    assert_eq!(a_dire[0]["salarie"], json!(permanent));
    let natures: BTreeSet<String> = a_dire[0]["natures"].as_array().unwrap().iter().map(|n| n["cle"].as_str().unwrap().to_string()).collect();
    assert!(natures.contains("cdi") && natures.contains("cdd") && !natures.contains("cddu_spectacle"), "{:?}", natures);

    // Une nature qui n'est pas d'un permanent, un salarie sans contrat a dire : refus, rien n'est ecrit.
    let avant = engagements_ecrits(&racine);
    assert_eq!(route(&racine, "importation-nature", json!({"salarie": permanent, "nature": "cddu_spectacle"}))["ok"], json!(false));
    assert_eq!(route(&racine, "importation-nature", json!({"salarie": artiste, "nature": "cdi"}))["ok"], json!(false));
    assert_eq!(engagements_ecrits(&racine), avant);

    // Dire la nature d'un permanent, c'est dire que la compagnie emploie des salaries au mois : la case s'en coche.
    assert_ne!(entreprise_lue(&racine)["emploie_des_permanents"], json!(true));
    let r = reussi(&route(&racine, "importation-nature", json!({"salarie": permanent, "nature": "cdi"}))).clone();
    assert!(r["ecrits"].as_u64().unwrap() >= 1, "{}", r);
    assert_eq!(entreprise_lue(&racine)["emploie_des_permanents"], json!(true));
    assert_eq!(r["a_dire"], json!([]));
    for e in engagements() {
        if e["salarie"] == json!(permanent) {
            assert_eq!((e["nature"].clone(), e.get("nature_a_dire").cloned()), (json!("cdi"), None), "{}", e);
        }
    }
    assert_eq!(reussi(&route(&racine, "importation-etat", json!({})))["a_dire"], json!([]));
}

/// Le contrat d'un permanent paye le dernier mois repris continue : sa nature se dit avec la date de debut et le numero
/// que la DSN de l'ancien logiciel lui a donnes (S21.G00.40.001 et .009, `norme-dsn/texte/bloc-S21.G00.40.txt` ; un
/// numero ne se recalcule pas, regle 4). Sans eux, ou avec un debut que les bulletins dementent : refus, rien n'est
/// ecrit. Dits, le contrat se reprend au mois suivant, avec sa nature, son debut et son numero.
#[test]
fn le_contrat_d_un_permanent_qui_continue_se_reprend_au_mois_suivant() {
    let racine = neuve("continue");
    let import = racine.join("import");
    std::fs::create_dir_all(&import).unwrap();
    pdf(vec![page_a(&permanent())], &import.join("permanent.pdf"));
    reussi(&route(&racine, "importer", json!({"dossier": ""})));
    let etat = reussi(&route(&racine, "importation-etat", json!({}))).clone();
    let x = &etat["a_dire"][0];
    let cle = x["salarie"].as_str().unwrap().to_string();
    assert_eq!((x["en_cours"].clone(), x["suite"].clone()), (json!(true), json!("2025-06")), "{}", x);
    let demande = |n: &str| x["natures"].as_array().unwrap().iter().find(|v| v["cle"] == json!(n)).unwrap()["demande"].clone();
    assert_eq!(demande("cdi"), json!(["debut", "numero", "salaire", "quotite"]));
    assert_eq!(demande("cdd"), json!(["debut", "fin", "numero", "salaire", "quotite"]));

    let avant = engagements_ecrits(&racine);
    for charge in [
        json!({"salarie": cle, "nature": "cdi"}),
        json!({"salarie": cle, "nature": "cdi", "debut": "2024-01-15"}),
        json!({"salarie": cle, "nature": "cdi", "debut": "2025-05-20", "numero": "00042", "salaire": "2400", "quotite": "151,67"}),
        json!({"salarie": cle, "nature": "cdd", "debut": "2024-01-15", "numero": "00042", "salaire": "2400", "quotite": "151,67"}),
        json!({"salarie": cle, "nature": "cdd", "debut": "2024-01-15", "fin": "2025-05-01", "numero": "00042", "salaire": "2400", "quotite": "151,67"}),
        json!({"salarie": cle, "nature": "cdi", "debut": "2024-01-15", "numero": "00042"}),
        json!({"salarie": cle, "nature": "cdi", "debut": "2024-01-15", "numero": "00042", "salaire": "0", "quotite": "151.67"}),
        json!({"salarie": cle, "nature": "cdi", "debut": "2024-01-15", "numero": "00042", "salaire": "2400"}),
    ] {
        assert_eq!(route(&racine, "importation-nature", charge.clone())["ok"], json!(false), "{}", charge);
    }
    assert_eq!(engagements_ecrits(&racine), avant);
    assert!(!racine.join("config/entreprise.json").exists() || entreprise_lue(&racine)["emploie_des_permanents"] != json!(true));

    reussi(&route(&racine, "importation-nature", json!({"salarie": cle, "nature": "cdi", "debut": "15/01/2024", "numero": "00042", "salaire": "2 400,50", "quotite": "151,67"})));
    let r = reussi(&route(&racine, "contrats-en-cours", json!({"mois": "2025-06"}))).clone();
    let c = r["contrats"].as_array().unwrap().iter().find(|c| c["salarie"] == json!(cle)).unwrap_or_else(|| panic!("{}", r)).clone();
    assert_eq!((c["debut"].clone(), c["numero"].clone(), c["engagement"]["nature"].clone()), (json!("2024-01-15"), json!("00042"), json!("cdi")), "{}", c);
    assert_eq!((c["engagement"]["salaire_mensuel"].clone(), c["engagement"]["quotite_contrat"].clone()), (json!(2400.5), json!(151.67)), "{}", c);
    // Le mois repris lui-meme ne le reprend pas : il est dans l'historique.
    let mai = reussi(&route(&racine, "contrats-en-cours", json!({"mois": "2025-05"}))).clone();
    assert!(mai["contrats"].as_array().unwrap().iter().all(|c| c["salarie"] != json!(cle)), "{}", mai);
}
