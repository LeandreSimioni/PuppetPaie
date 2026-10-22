//! `paie route` : `contrats-en-cours`, `contrats-passes`, `emploi`, `rubriques-actives` et `metiers-catalogue`. Sur la
//! configuration de `fictive-3090-petite` et les mois que chaque test ecrit ; le code livre est copie sans ses baremes :
//! aucune de ces routes ne calcule de paie.

use crate::outils::{compagnie, dossier_de_test, ecrire, lire, reussi, route};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn compagnie(nom: &str) -> PathBuf {
    let racine = compagnie::saisies("fictive-3090-petite", &[], &dossier_de_test("routes_contrats", nom).join("compagnie"));
    compagnie::avec_le_code(&racine, true);
    racine
}

fn resultat(c: &Path, nom: &str, charge: Value) -> Value {
    reussi(&route(c, nom, charge, "2026-10-04")).clone()
}

/// La phrase d'un refus.
fn refus(c: &Path, nom: &str, charge: Value) -> String {
    crate::outils::refus(&route(c, nom, charge, "2026-10-04")).1
}

fn ecrire_mois(c: &Path, mois: &str, engagements: Value) {
    ecrire(c, &format!("paies/{}.json", mois), &json!({"mois": mois, "taux_cachet": 130.0, "date_paiement": format!("{}-28", mois), "engagements": engagements}));
}

/// Un contrat de septembre qui court jusqu'au 10 octobre, trois jours de planning.
fn contrat_a_cheval() -> Value {
    json!({
        "salarie": "BRIOCHE", "spectacle": "ubu-roi", "fin": "2026-10-10", "numero": "BR1.26.01", "metier": "COM010",
        "lignes": [{"rubrique": "cachet", "quantite": 2, "taux": 130.0, "lieu": "Tours",
                    "dates": ["2026-09-28", "2026-09-29"], "planning": ["2026-10-01", "2026-10-05", "2026-10-10"]}],
    })
}

// ================================================ contrats-en-cours

#[test]
fn un_contrat_a_cheval_se_reprend_au_mois_suivant_sans_aucun_calcul() {
    let c = &compagnie("en-cours");
    ecrire_mois(c, "2026-09", json!([contrat_a_cheval()]));
    let r = resultat(c, "contrats-en-cours", json!({"mois": "2026-10"}));
    assert_eq!(r["mois"], "2026-10");
    let l = r["contrats"].as_array().unwrap();
    assert_eq!(l.len(), 1);
    let k = &l[0];
    assert_eq!(k["salarie"], "BRIOCHE");
    assert_eq!(k["nom"], "BRIOCHE Fanchon");
    assert_eq!((k["debut"].as_str(), k["fin"].as_str(), k["depuis"].as_str()), (Some("2026-09-28"), Some("2026-10-10"), Some("2026-09")));
    assert_eq!(k["deja_repris"], json!(false));
    assert_eq!(k["taux_cachet"], json!(130.0));
    // Les jours d'octobre deviennent les dates de la ligne, au meme taux et au meme lieu.
    let e = &k["engagement"];
    assert_eq!(e["numero"], "BR1.26.01");
    assert_eq!(e["metier"], "COM010");
    assert!(e.get("cadre").is_none() && e.get("lieu").is_none());
    assert_eq!(
        e["lignes"],
        json!([{"rubrique": "cachet", "taux": 130.0, "lieu": "Tours", "quantite": 3.0,
                "dates": ["2026-10-01", "2026-10-05", "2026-10-10"]}])
    );
    // Le mois d'apres ne les reprend plus.
    let r = resultat(c, "contrats-en-cours", json!({"mois": "2026-11"}));
    assert_eq!(r["contrats"], json!([]));
}

#[test]
fn deux_contrats_a_cheval_du_meme_jour_se_reprennent_chacun() {
    // Les jumeaux (meme salarie, meme debut, numeros distincts) : le second n'etait jamais repris.
    let c = &compagnie("en-cours-jumeaux");
    let premier = contrat_a_cheval();
    let mut jumeau = contrat_a_cheval();
    jumeau["numero"] = json!("BR1.26.02");
    jumeau["lignes"][0]["quantite"] = json!(1);
    jumeau["lignes"][0]["dates"] = json!(["2026-09-28"]);
    jumeau["lignes"][0]["planning"] = json!(["2026-10-02"]);
    ecrire_mois(c, "2026-09", json!([premier, jumeau.clone()]));
    let r = resultat(c, "contrats-en-cours", json!({"mois": "2026-10"}));
    let l = r["contrats"].as_array().unwrap();
    let numeros: Vec<&str> = l.iter().map(|k| k["numero"].as_str().unwrap()).collect();
    assert_eq!(numeros, vec!["BR1.26.01", "BR1.26.02"], "{}", r);
    assert_eq!(l[1]["engagement"]["lignes"][0]["dates"], json!(["2026-10-02"]));
    // Octobre ne reprend que le jumeau : lui seul est « deja repris ».
    let mut repris = jumeau;
    repris["debut"] = json!("2026-09-28");
    repris["lignes"][0]["dates"] = json!(["2026-10-02"]);
    repris["lignes"][0].as_object_mut().unwrap().remove("planning");
    ecrire_mois(c, "2026-10", json!([repris]));
    let r = resultat(c, "contrats-en-cours", json!({"mois": "2026-10"}));
    let l = r["contrats"].as_array().unwrap();
    assert_eq!(l.len(), 2, "{}", r);
    assert_eq!((l[0]["numero"].as_str(), l[0]["deja_repris"].as_bool()), (Some("BR1.26.01"), Some(false)));
    assert_eq!((l[1]["numero"].as_str(), l[1]["deja_repris"].as_bool()), (Some("BR1.26.02"), Some(true)));
}

#[test]
fn le_planning_des_mois_suivants_reste_en_planning_et_un_mois_repris_se_dit() {
    let c = &compagnie("planning");
    let mut e = contrat_a_cheval();
    e["fin"] = json!("2026-11-20");
    e["lignes"][0]["planning"] = json!(["2026-10-01", "2026-10-05", "2026-11-03", "2026-11-20"]);
    ecrire_mois(c, "2026-09", json!([e.clone()]));
    let r = resultat(c, "contrats-en-cours", json!({"mois": "2026-10"}));
    let ligne = &r["contrats"][0]["engagement"]["lignes"][0];
    assert_eq!(ligne["dates"], json!(["2026-10-01", "2026-10-05"]));
    assert_eq!(ligne["planning"], json!(["2026-11-03", "2026-11-20"]));
    assert_eq!(ligne["quantite"], json!(2.0));
    // Le fichier d'octobre porte deja ce contrat (meme salarie, meme debut) : il est « repris ».
    ecrire_mois(c, "2026-10", json!([e]));
    let r = resultat(c, "contrats-en-cours", json!({"mois": "2026-10"}));
    assert_eq!(r["contrats"][0]["deja_repris"], json!(true));
    // Un contrat sans planning dans le mois : une ligne de cachets a completer.
    let mut sans = contrat_a_cheval();
    sans["lignes"][0].as_object_mut().unwrap().remove("planning");
    sans["fin"] = json!("2026-10-10");
    ecrire_mois(c, "2026-09", json!([sans]));
    std::fs::remove_file(c.join("paies/2026-10.json")).unwrap();
    let r = resultat(c, "contrats-en-cours", json!({"mois": "2026-10"}));
    assert_eq!(r["contrats"][0]["engagement"]["lignes"], json!([{"rubrique": "cachet", "quantite": 0, "taux": 130.0, "dates": []}]));
}

/// #140 : un contrat repris sans ligne a dater recoit le DERNIER TAUX saisi (`routes_saisie::dernier_taux`), pas
/// 130 € pose par la reprise.
#[test]
fn une_reprise_sans_cachet_propose_le_dernier_taux_saisi() {
    let c = &compagnie("dernier-taux");
    let ecrire = |mois: &str, v: Value| ecrire(c, &format!("paies/{}.json", mois), &v);
    ecrire("2026-08", json!({"mois": "2026-08", "date_paiement": "2026-08-31", "engagements": [{
        "salarie": "BRIOCHE", "spectacle": "ubu-roi", "metier": "COM010",
        "lignes": [{"rubrique": "cachet", "quantite": 1, "taux": 155.0, "dates": ["2026-08-20"]}]}]}));
    // Septembre : un contrat de journees qui court en octobre, sans planning ni cachet.
    ecrire("2026-09", json!({"mois": "2026-09", "date_paiement": "2026-09-30", "engagements": [{
        "salarie": "BRIOCHE", "spectacle": "ubu-roi", "fin": "2026-10-10", "numero": "BR1.26.01", "metier": "COM010",
        "lignes": [{"rubrique": "journee", "quantite": 1, "taux": 200.0, "dates": ["2026-09-28"]}]}]}));
    let r = resultat(c, "contrats-en-cours", json!({"mois": "2026-10"}));
    assert_eq!(r["contrats"][0]["engagement"]["lignes"], json!([{"rubrique": "cachet", "quantite": 0, "taux": 155.0, "dates": []}]), "{}", r);
}

/// R12 : sans aucun cachet saisi dans le dossier, la reprise ne propose aucun taux (regle 1 : les 130 EUR de CoolPaie ne
/// venaient d'aucun texte) ; la ligne reste a completer.
#[test]
fn une_reprise_sans_aucun_cachet_dans_le_dossier_ne_propose_aucun_taux() {
    let c = &compagnie("aucun-taux");
    ecrire(c, "paies/2026-09.json", &json!({"mois": "2026-09", "date_paiement": "2026-09-30", "engagements": [{
        "salarie": "BRIOCHE", "spectacle": "ubu-roi", "fin": "2026-10-10", "numero": "BR1.26.01", "metier": "COM010",
        "lignes": [{"rubrique": "journee", "quantite": 1, "taux": 200.0, "dates": ["2026-09-28"]}]}]}));
    let r = resultat(c, "contrats-en-cours", json!({"mois": "2026-10"}));
    assert_eq!(r["contrats"][0]["engagement"]["lignes"], json!([{"rubrique": "cachet", "quantite": 0, "taux": null, "dates": []}]), "{}", r);
}

#[test]
fn un_salarie_inconnu_ou_un_contrat_fini_ne_donne_rien() {
    let c = &compagnie("inconnu");
    let mut inconnu = contrat_a_cheval();
    inconnu["salarie"] = json!("Personne Zzz");
    let mut court = contrat_a_cheval();
    court["fin"] = json!("2026-09-30");
    court["lignes"][0].as_object_mut().unwrap().remove("planning");
    ecrire_mois(c, "2026-09", json!([inconnu, court]));
    assert_eq!(resultat(c, "contrats-en-cours", json!({"mois": "2026-10"}))["contrats"], json!([]));
}

#[test]
fn un_cdd_paye_au_mois_se_reprend_avec_sa_nature_sans_spectacle_ni_cachet() {
    // Un CDD de droit commun de septembre a decembre : il court en octobre, et c'est un permanent.
    // Il ne se rattache a aucun spectacle (S21.G00.40.050, CCH-11) et ses lignes, le moteur les
    // compose chaque mois depuis son salaire : la reprise recopie ce qui le definit, rien d'autre.
    let c = &compagnie("cdd-au-mois");
    ecrire_mois(c, 
        "2026-09",
        json!([{
            "salarie": "SERAPHIN", "nature": "cdd", "debut": "2026-09-01", "fin": "2026-12-31", "numero": "SE.26.01",
            "metier": "COM010", "motif_recours": "02", "salaire_mensuel": 2100.0, "quotite_contrat": 151.67,
            "modalite_temps_travail": "10", "primes": [{"libelle": "Prime de septembre", "montant": 100.0}],
            "date_paiement": "2026-09-30",
        }]),
    );
    let r = resultat(c, "contrats-en-cours", json!({"mois": "2026-10"}));
    let l = r["contrats"].as_array().unwrap();
    assert_eq!(l.len(), 1, "{}", r);
    let k = &l[0];
    assert_eq!(k["spectacle"], Value::Null, "un permanent ne joue aucun spectacle : {}", k);
    assert_eq!(
        k["engagement"],
        json!({
            "salarie": "SERAPHIN", "debut": "2026-09-01", "fin": "2026-12-31", "numero": "SE.26.01", "metier": "COM010",
            "nature": "cdd", "motif_recours": "02", "salaire_mensuel": 2100.0, "quotite_contrat": 151.67,
            "modalite_temps_travail": "10",
        })
    );
}

#[test]
fn un_contrat_d_usage_sans_spectacle_prend_celui_du_mois_et_rien_ne_se_devine() {
    let c = &compagnie("sans-spectacle");
    let mut e = contrat_a_cheval();
    e.as_object_mut().unwrap().remove("spectacle");
    // Le fichier de septembre nomme son spectacle : c'est celui que l'engagement jouait.
    let mois = |spectacle: Value| {
        json!({"mois": "2026-09", "taux_cachet": 130.0, "date_paiement": "2026-09-28", "spectacle": spectacle, "engagements": [e.clone()]})
    };
    ecrire(c, "paies/2026-09.json", &mois(json!("les-pupazzi")));
    let r = resultat(c, "contrats-en-cours", json!({"mois": "2026-10"}));
    assert_eq!(r["contrats"][0]["spectacle"], "les-pupazzi", "{}", r);
    assert_eq!(r["contrats"][0]["engagement"]["spectacle"], "les-pupazzi");
    // Ni l'un ni l'autre : le contrat se reprend (il court), sans spectacle -- pas avec le premier venu.
    ecrire(c, "paies/2026-09.json", &mois(json!("")));
    let r = resultat(c, "contrats-en-cours", json!({"mois": "2026-10"}));
    let l = r["contrats"].as_array().unwrap();
    assert_eq!(l.len(), 1, "{}", r);
    assert_eq!(l[0]["spectacle"], Value::Null, "{}", r);
    assert!(l[0]["engagement"].get("spectacle").is_none(), "{}", r);
}

// ================================================ contrats-passes

#[test]
fn les_contrats_passes_lisent_les_fichiers_de_paie_puis_l_historique() {
    let c = &compagnie("passes");
    let mut autre = contrat_a_cheval();
    autre["salarie"] = json!("SERAPHIN");
    autre["lignes"][0]["dates"] = json!(["2026-09-28"]);
    autre["lignes"][0]["quantite"] = json!(1);
    ecrire_mois(c, "2026-09", json!([contrat_a_cheval(), autre]));
    ecrire(c, 
        "historique/engagements.json",
        &json!({"engagements": [
            // Declare avant ce logiciel, sur un mois que nos fichiers ne couvrent pas.
            {"salarie": "LEMERCIER", "debut": "2026-05-04", "fin": "2026-05-06", "numero_objet": "256Z00920022",
             "heures": 14, "cachets": 3, "jours": 3, "brut": "300"},
            // Sur un mois que nos fichiers couvrent : il se tait.
            {"salarie": "LEMERCIER", "debut": "2026-09-14", "fin": "2026-09-15", "numero_objet": "256Z00920022", "cachets": 2},
        ]}),
    );
    let r = resultat(c, "contrats-passes", json!({"mois": "2026-09"}));
    let l = r["contrats"].as_array().unwrap();
    assert_eq!(l.len(), 2, "{}", r);
    // Tries par salarie puis debut.
    assert_eq!((l[0]["salarie"].as_str(), l[1]["salarie"].as_str()), (Some("BRIOCHE"), Some("SERAPHIN")));
    assert_eq!(l[0]["source"], "paie");
    assert_eq!((l[0]["debut"].as_str(), l[0]["fin"].as_str()), (Some("2026-09-28"), Some("2026-10-10")));
    assert_eq!((l[0]["cachets"].as_i64(), l[0]["jours"].as_i64(), l[0]["heures"].as_i64()), (Some(2), Some(2), Some(0)));
    assert_eq!(l[0]["titre"], "Ubu roi");
    assert_eq!(l[0]["numero_objet"], "256Z00920011");
    // Seuls les jours JOUES du mois demande.
    assert_eq!(l[0]["dates"], json!(["2026-09-28", "2026-09-29"]));
    let mai = resultat(c, "contrats-passes", json!({"mois": "2026-05"}));
    assert_eq!(mai["contrats"].as_array().unwrap().len(), 1);
    let m = &mai["contrats"][0];
    assert_eq!((m["source"].as_str(), m["titre"].as_str(), m["cachets"].as_i64(), m["heures"].as_i64()), (Some("historique"), Some("Les Pupazzi"), Some(3), Some(14)));
    assert_eq!(m["dates"], json!([]));
    // Un salarie voulu, ou plusieurs.
    let filtre = resultat(c, "contrats-passes", json!({"mois": "2026-09", "salaries": ["SERAPHIN"]}));
    assert_eq!(filtre["contrats"].as_array().unwrap().len(), 1);
    assert_eq!(filtre["contrats"][0]["salarie"], "SERAPHIN");
    assert_eq!(resultat(c, "contrats-passes", json!({"mois": "2026-09", "salaries": ["PERSONNE"]}))["contrats"], json!([]));
}

#[test]
fn une_charge_fausse_ou_un_mois_abime_refuse() {
    let c = &compagnie("refus");
    ecrire_mois(c, "2026-09", json!([contrat_a_cheval()]));
    for (nom, charge) in [("contrats-en-cours", json!({})), ("contrats-passes", json!({})), ("contrats-en-cours", json!({"mois": 5}))] {
        refus(c, nom, charge);
    }
    std::fs::write(c.join("paies/2026-08.json"), "{pas du json").unwrap();
    refus(c, "contrats-en-cours", json!({"mois": "2026-10"}));
    // Des lignes qui ne sont pas des objets.
    std::fs::remove_file(c.join("paies/2026-08.json")).unwrap();
    let mut mal = contrat_a_cheval();
    mal["lignes"] = json!(["texte"]);
    ecrire_mois(c, "2026-07", json!([mal]));
    refus(c, "contrats-passes", json!({"mois": "2026-10"}));
}

// ================================================ emploi

#[test]
fn un_metier_se_traduit_en_champs_de_fiche_et_un_code_inconnu_est_un_refus() {
    let c = &compagnie("emploi");
    let r = resultat(c, "emploi", json!({"metier": " com010 "}));
    assert_eq!(r["code_emploi_ft"], "COM010");
    let cadre = resultat(c, "emploi", json!({"metier": "COM010", "cadre": true, "feminin": true}));
    assert_eq!((cadre["statut_categoriel_rc"].as_str(), cadre["statut_conventionnel"].as_str()), (Some("01"), Some("04")));
    let non = resultat(c, "emploi", json!({"metier": "COM010", "cadre": false}));
    assert_eq!(non["statut_categoriel_rc"], "04");
    let message = refus(c, "emploi", json!({"metier": "zzz999"}));
    assert!(message.starts_with("Metier 'zzz999' inconnu"), "{}", message);
    for charge in [json!({"metier": ""}), json!({"metier": 5}), json!({})] {
        refus(c, "emploi", charge);
    }
}

// ================================================ metiers-catalogue

#[test]
fn le_catalogue_dit_ce_que_la_compagnie_retient() {
    let c = &compagnie("metiers");
    let r = resultat(c, "metiers-catalogue", json!({}));
    let metiers = r["metiers"].as_array().unwrap();
    assert!(metiers.len() > 50);
    assert_eq!(r["familles"][0], "Artistes");
    // Chacun dit s'il vient de la convention de la compagnie, et s'il est retenu.
    assert!(metiers.iter().all(|m| m["retenu"].is_boolean() && m["de_la_convention"].is_boolean() && m["de"].is_string()));
    let hors = metiers.iter().find(|m| m["de_la_convention"] == json!(false)).expect("un metier d'une autre convention");
    assert_eq!(hors["retenu"], json!(false));
    assert_ne!(hors["de"], "");
    // La compagnie le coche, et en decoche un des siens.
    let propre = metiers.iter().find(|m| m["de_la_convention"] == json!(true)).unwrap()["code"].clone();
    ecrire(c, "config/metiers-ajoutes.json", &json!({"codes": [hors["code"]], "retires": [propre]}));
    let r = resultat(c, "metiers-catalogue", json!({}));
    let par = |code: &Value| r["metiers"].as_array().unwrap().iter().find(|m| &m["code"] == code).unwrap().clone();
    assert_eq!(par(&hors["code"])["retenu"], json!(true));
    assert_eq!(par(&propre)["retenu"], json!(false));
    assert_eq!(par(&propre)["de_la_convention"], json!(true));
    // Sans IDCC (une compagnie qui s'installe), rien ne se cache.
    let mut entreprise = lire(c, "config/entreprise.json");
    entreprise["idcc"] = json!("");
    ecrire(c, "config/entreprise.json", &entreprise);
    let r = resultat(c, "metiers-catalogue", json!({}));
    assert!(r["metiers"].as_array().unwrap().iter().all(|m| m["de_la_convention"] == json!(true)));
}

// ================================================ rubriques-actives

#[test]
fn cocher_des_rubriques_ecrit_la_preference_garde_une_copie_et_rend_le_catalogue() {
    let c = &compagnie("actives");
    let avant = std::fs::read_to_string(c.join("config/rubriques-actives.json")).unwrap();
    let r = resultat(c, "rubriques-actives", json!({"actives": ["journee", "cachet", "journee"]}));
    let rubriques = r["rubriques"].as_array().unwrap();
    // Les actives d'abord, dans l'ordre coche.
    assert_eq!(rubriques.iter().filter(|x| x["active"] == json!(true)).count(), 2);
    assert!(rubriques[0]["active"] == json!(true) && rubriques[2]["active"] == json!(false));
    let ecrit = lire(c, "config/rubriques-actives.json");
    assert_eq!(ecrit["actives"], json!(["journee", "cachet"]));
    // La copie horodatee de l'ancien fichier, avec l'heure de Paris.
    let copies: Vec<_> = std::fs::read_dir(c.join("config/avant")).unwrap().flatten().collect();
    assert_eq!(copies.len(), 1);
    let nom = copies[0].file_name().to_string_lossy().into_owned();
    let horodatage = nom.strip_prefix("rubriques-actives.json.").expect(&nom);
    assert!(horodatage.len() == 15 && horodatage.as_bytes()[8] == b'-' && horodatage.bytes().filter(u8::is_ascii_digit).count() == 14, "{}", nom);
    assert_eq!(std::fs::read_to_string(copies[0].path()).unwrap(), avant);
}

#[test]
fn une_rubrique_inconnue_est_refusee_sans_rien_ecrire() {
    let c = &compagnie("actives-refus");
    let avant = std::fs::read_to_string(c.join("config/rubriques-actives.json")).unwrap();
    let message = refus(c, "rubriques-actives", json!({"actives": ["cachet", "nope"]}));
    assert!(message.starts_with("Rubrique(s) inconnue(s) : nope. Connues : "), "{}", message);
    assert_eq!(std::fs::read_to_string(c.join("config/rubriques-actives.json")).unwrap(), avant);
    assert!(!c.join("config/avant").exists());
    refus(c, "rubriques-actives", json!({}));
    refus(c, "rubriques-actives", json!({"actives": 5}));
    assert!(!c.join("config/avant").exists());
}

#[test]
fn seules_les_vingt_dernieres_copies_restent() {
    let c = &compagnie("actives-copies");
    for k in 0..25 {
        ecrire(c, &format!("config/avant/rubriques-actives.json.20260101-0000{:02}", k), &json!(k));
    }
    ecrire(c, "config/avant/autre.json.20260101-000000", &json!("autre"));
    resultat(c, "rubriques-actives", json!({"actives": ["cachet"]}));
    let mut noms: Vec<String> =
        std::fs::read_dir(c.join("config/avant")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    noms.sort();
    let siennes: Vec<_> = noms.iter().filter(|n| n.starts_with("rubriques-actives.json.")).collect();
    assert_eq!(siennes.len(), 20);
    // Les plus anciennes sont parties, celle d'un autre fichier reste.
    assert!(!noms.contains(&"rubriques-actives.json.20260101-000000".to_string()));
    assert!(noms.contains(&"autre.json.20260101-000000".to_string()));
}
