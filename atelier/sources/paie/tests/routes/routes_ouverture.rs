//! `paie route` : les routes de l'ouverture, `rappels` (sans aucun calcul : les baremes se retirent pour le prouver) et
//! `apercu` (relu quand la saisie est enregistree, calcule en memoire sinon, rien d'ecrit, numeros reserves sans les
//! ecrire). Sur la configuration de `fictive-3090-petite` sans compteurs, et les mois de chaque test.

use crate::outils::{compagnie, dossier_de_test, ecrire, lire, reussi, route};
use paie::date::Jour;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn compagnie(nom: &str, mois: &[&str]) -> PathBuf {
    let racine = compagnie::saisies("fictive-3090-petite", mois, &dossier_de_test("routes_ouverture", nom).join("compagnie"));
    std::fs::remove_file(racine.join("config/compteurs.json")).unwrap();
    racine
}

fn rappels(c: &Path, aujourdhui: &str) -> Vec<Value> {
    reussi(&route(c, "rappels", json!({}), aujourdhui))["rappels"].as_array().unwrap().clone()
}

fn saisie(c: &Path, mois: &str) -> Value {
    lire(c, &format!("paies/{}.json", mois))
}

fn calcul(c: &Path, mois: &str) -> PathBuf {
    c.join("paies").join(format!("{}.calcul.json", mois))
}

fn journal(c: &Path, lignes: Value) {
    ecrire(c, "journal/depots.json", &lignes);
}

/// Plus aucun bareme : un calcul de paie refuserait, un rappel n'en fait pas.
fn sans_baremes(c: &Path) {
    compagnie::avec_le_code(c, true);
}

fn dsn_reelle(mois: &str, verdict: &str) -> Value {
    json!({
        "quoi": "dsn", "quand": "2026-09-10T10:00:00", "mois": mois, "fichier": format!("DSN_{}.dsn", mois),
        "mode": "reel", "qui": "essai", "accepte": true, "flux": "F1",
        "retours": [{"verdict": verdict, "nature": "11", "anomalies": ["controle test"]}],
    })
}

fn par_cle<'a>(r: &'a [Value], cle: &str) -> &'a Value {
    r.iter().find(|x| x["cle"] == json!(cle)).unwrap_or_else(|| panic!("pas de rappel {} dans {:?}", cle, r.iter().map(|x| &x["cle"]).collect::<Vec<_>>()))
}

// ============================================================ les rappels

#[test]
fn la_dsn_du_mois_d_avant_se_rappelle_sans_aucun_calcul() {
    let c = &compagnie("dsn", &["2026-08"]);
    sans_baremes(c);
    // Une paie validee sur les trois : c'est le seul chiffre que le rappel lit de la paie.
    let mut s = saisie(c, "2026-08");
    s["engagements"][0]["valide"] = json!("2026-09-01");
    ecrire(c, "paies/2026-08.json", &s);
    let r = rappels(c, "2026-09-10");
    let dsn = par_cle(&r, "dsn:2026-08");
    assert_eq!(dsn["quoi"], "dsn");
    assert_eq!(dsn["etat"], "a_produire");
    assert_eq!(dsn["en_retard"], json!(false));
    assert_eq!(dsn["paies"], json!(3));
    assert_eq!(dsn["validees"], json!(1));
    assert_eq!(dsn["titre"], "Il est temps d'envoyer la DSN de aout 2026");
    assert!(dsn["detail"].as_str().unwrap().contains("3 paies saisies, 1 validée. Le mois n'est pas encore écrit"));
    assert!(dsn["detail"].as_str().unwrap().contains("Il reste 2 paies non validées."));
    // Aucun calcul, donc aucun fichier de paie calculee.
    assert!(!calcul(c, "2026-08").exists());
    // Passee l'echeance du 15, c'est un retard.
    let r = rappels(c, "2026-09-16");
    assert_eq!(par_cle(&r, "dsn:2026-08")["en_retard"], json!(true));
}

#[test]
fn une_dsn_reelle_acceptee_fait_taire_le_rappel_une_refusee_le_corrige() {
    let c = &compagnie("journal", &["2026-08"]);
    journal(c, json!([dsn_reelle("2026-08", "OK")]));
    assert!(rappels(c, "2026-09-10").iter().all(|x| x["quoi"] != "dsn"), "la DSN d'aout est passee");
    journal(c, json!([dsn_reelle("2026-08", "KO")]));
    let r = rappels(c, "2026-09-10");
    let dsn = par_cle(&r, "dsn:2026-08");
    assert_eq!(dsn["etat"], "a_corriger");
    assert!(dsn["detail"].as_str().unwrap().contains("REFUSÉE : controle test. À corriger et à renvoyer."));
}

#[test]
fn une_compagnie_sans_saisie_ni_dsn_n_est_en_retard_de_rien() {
    let c = &compagnie("neuve", &[]);
    // Une compagnie qui a des salaries (ou des paies, ou des DSN) a deja une histoire ;
    // celle-ci n'a rien : le premier mois du est le mois courant.
    ecrire(c, "config/salaries.json", &json!({}));
    assert!(rappels(c, "2026-09-30").is_empty());
}

#[test]
fn la_fin_d_annee_cloture_exporte_et_revoit_les_spectacles() {
    let c = &compagnie("annee", &["2026-12"]);
    journal(c, json!([dsn_reelle("2026-12", "OK")]));
    let r = rappels(c, "2027-01-20");
    assert_eq!(par_cle(&r, "cloture:2026")["quoi"], "cloture");
    assert_eq!(par_cle(&r, "export-compta:2026")["quoi"], "export-compta");
    let revue = par_cle(&r, "revue-spectacles:2027");
    assert!(!revue["spectacles"].as_array().unwrap().is_empty());
    // Exercice clos : plus de cloture. Export fait apres la derniere saisie : plus d'export.
    ecrire(c, "config/exercices.json", &json!({ "2026": { "le": "2027-01-15", "qui": "essai" } }));
    std::fs::create_dir_all(c.join("sorties/exercices")).unwrap();
    std::fs::write(c.join("sorties/exercices/Compta_2026.csv"), "journal;date\n").unwrap();
    let r = rappels(c, "2027-01-20");
    assert!(r.iter().all(|x| x["quoi"] != "cloture" && x["quoi"] != "export-compta"));
    // La revue de l'annee cochee : elle se tait. Passe mars, elle ne se pose plus.
    ecrire(c, "config/echeancier.json", &json!({ "fait": { "revue-spectacles:2027": "x" } }));
    assert!(rappels(c, "2027-01-20").iter().all(|x| x["quoi"] != "revue-spectacles"));
    std::fs::remove_file(c.join("config/echeancier.json")).unwrap();
    assert!(rappels(c, "2027-04-02").iter().all(|x| x["quoi"] != "revue-spectacles"));
}

#[test]
fn le_rappel_d_une_fin_de_cdd_dit_le_preavis_de_son_motif_de_rupture() {
    // Le meme preavis que l'ecran des fins de contrat (`routes_listes`) : L1243-1 (aucun pour une faute grave), L1243-2
    // (la regle du salarie qui part pour un CDI, sans duree) ; textes dans `code-du-travail/articles-20261013.md`.
    let c = &compagnie("rappel-fin-cdd", &["2026-08"]);
    let mut s = saisie(c, "2026-08");
    for (salarie, motif) in [("LEMERCIER", "095"), ("BRIOCHE", "037")] {
        s["engagements"].as_array_mut().unwrap().push(json!({"salarie": salarie, "nature": "cdd", "debut": "2019-03-04",
            "fin": "2026-08-20", "numero": format!("{}.26.01", &salarie[..2]), "motif_rupture": motif}));
    }
    ecrire(c, "paies/2026-08.json", &s);
    let r = rappels(c, "2026-08-24");
    let a_saisir = |salarie: &str| par_cle(&r, &format!("fin-contrat:{}:2026-08-20", salarie))["a_saisir"].to_string();
    let faute = a_saisir("LEMERCIER");
    assert!(faute.contains("La loi ne donne aucun preavis pour ce motif") && faute.contains("L1243-1"), "{}", faute);
    assert!(!faute.contains("La loi en donne"), "{}", faute);
    let cdi = a_saisir("BRIOCHE");
    assert!(cdi.contains("La loi en donne un que le moteur ne compte pas") && cdi.contains("L1243-2"), "{}", cdi);
    assert!(!cdi.contains("(L1234-1)") && !cdi.contains("c'est elle qui s'applique"), "{}", cdi);
}

#[test]
fn une_embauche_sans_trace_de_dpae_se_rappelle_dans_sa_fenetre() {
    let c = &compagnie("dpae", &["2026-10"]);
    let debut = saisie(c, "2026-10")["engagements"][0]["lignes"][0]["dates"][0].as_str().unwrap().to_string();
    let jour = Jour::lire(&debut).unwrap();
    // La veille de l'embauche : « il reste 1 jour ».
    let r = rappels(c, &jour.plus_jours(-1).unwrap().iso());
    let dpae = par_cle(&r, &format!("dpae:{}", debut));
    assert_eq!(dpae["quoi"], "dpae");
    assert_eq!(dpae["en_retard"], json!(false));
    assert!(dpae["detail"].as_str().unwrap().contains("il reste 1 jour."), "{}", dpae["detail"]);
    // Le jour meme, puis en retard.
    assert!(par_cle(&rappels(c, &debut), &format!("dpae:{}", debut))["detail"].as_str().unwrap().contains("L'embauche est aujourd'hui"));
    let apres = jour.plus_jours(1).unwrap().iso();
    let r = rappels(c, &apres);
    assert_eq!(par_cle(&r, &format!("dpae:{}", debut))["en_retard"], json!(true));
}

// =========================================================== l'apercu

const MOIS: &str = "2026-08";

fn apercu(c: &Path, saisie: &Value, lignes: bool) -> Value {
    route(c, "apercu", json!({ "saisie": saisie, "lignes": lignes }), "2026-09-10")
}

#[test]
fn l_apercu_d_une_saisie_enregistree_se_relit_il_ne_se_recalcule_pas() {
    let c = &compagnie("relu", &[MOIS]);
    let saisie = saisie(c, MOIS);
    assert!(!calcul(c, MOIS).exists());
    let premier = apercu(c, &saisie, false);
    assert_eq!(premier["ok"], json!(true), "{}", premier);
    assert!(calcul(c, MOIS).exists(), "la saisie enregistree s'est calculee et son calcul est ecrit");
    let r = &premier["resultat"];
    assert_eq!(r["mois"], MOIS);
    assert_eq!(r["libelle"], "Aout 2026");
    assert_eq!(r["paies"].as_array().unwrap().len(), saisie["engagements"].as_array().unwrap().len());
    // Le numero de bulletin se reserve sans rien ecrire : compteurs absents -> 1/2026.
    assert_eq!(r["paies"][0]["numero_bulletin"], "1/2026");
    assert!(!c.join("config/compteurs.json").exists());

    // On retouche le resultat stocke (les entrees, donc l'empreinte, ne bougent pas) :
    // un second apercu le relit tel quel, il ne calcule pas.
    let mut f = lire(c, &format!("paies/{}.calcul.json", MOIS));
    f["resultat"]["paies"][0]["brut"] = json!("999999.99");
    ecrire(c, &format!("paies/{}.calcul.json", MOIS), &f);
    let second = apercu(c, &saisie, true);
    assert_eq!(second["resultat"]["paies"][0]["brut"], "999999.99", "relu, pas recalcule");
    assert!(second["resultat"]["paies"][0]["lignes"].as_array().is_some_and(|l| !l.is_empty()), "`lignes` demande les cotisations");
    assert!(premier["resultat"]["paies"][0].get("lignes").is_none());
    // Le total ne se retouche pas : il se refait des paies relues.
    let somme: f64 = second["resultat"]["paies"].as_array().unwrap().iter().map(|p| p["brut"].as_str().unwrap().parse::<f64>().unwrap()).sum();
    assert!((second["resultat"]["total_brut"].as_str().unwrap().parse::<f64>().unwrap() - somme).abs() < 0.01);
}

#[test]
fn l_apercu_d_une_saisie_en_cours_se_calcule_en_memoire_sans_rien_ecrire() {
    let c = &compagnie("memoire", &[MOIS]);
    let saisie = saisie(c, MOIS);
    let attendu = apercu(c, &saisie, false); // le fichier : calcule, stocke
    std::fs::remove_file(calcul(c, MOIS)).unwrap();
    // Une saisie qui n'est pas celle du fichier (une cle de plus) : meme paie, calculee, rien d'ecrit.
    let mut encours = saisie.clone();
    encours["_frappe"] = json!("en cours");
    let r = apercu(c, &encours, false);
    assert_eq!(r, attendu, "le calcul en memoire rend ce que la relecture rend");
    assert!(!calcul(c, MOIS).exists(), "rien n'est ecrit pour une saisie qui n'est pas enregistree");
    // Un nombre de plus ou de moins dans le fichier n'est pas la meme saisie.
    let mut autre = saisie.clone();
    autre["engagements"][0]["lignes"][0]["taux"] = json!(131);
    let r = apercu(c, &autre, false);
    assert_ne!(r["resultat"]["paies"][0]["brut"], attendu["resultat"]["paies"][0]["brut"]);
    assert!(!calcul(c, MOIS).exists());
}

#[test]
fn les_numeros_se_reservent_a_la_suite_des_compteurs_sans_les_ecrire() {
    let c = &compagnie("numeros", &[MOIS]);
    let compteurs = json!({ "2026": { "bulletin": 41 } });
    ecrire(c, "config/compteurs.json", &compteurs);
    let r = apercu(c, &saisie(c, MOIS), false);
    let paies = r["resultat"]["paies"].as_array().unwrap();
    for (i, p) in paies.iter().enumerate() {
        assert_eq!(p["numero_bulletin"], json!(format!("{}/2026", 42 + i)));
        assert!(p["numero_contrat"].as_str().unwrap().contains(".26."), "{}", p["numero_contrat"]);
    }
    assert_eq!(lire(c, "config/compteurs.json"), compteurs, "l'apercu n'ecrit aucun compteur");
}

#[test]
fn une_saisie_incomplete_est_un_refus_qui_garde_son_nom() {
    let c = &compagnie("incomplete", &[MOIS]);
    let mut s = saisie(c, MOIS);
    s["engagements"][0]["lignes"][0]["dates"] = json!([]);
    let r = apercu(c, &s, false);
    assert_eq!(r["ok"], json!(false));
    assert_eq!(r["classe"], "SaisieIncomplete");
    assert!(r["erreur"].as_str().unwrap().contains("ne dit aucun jour joue"));
    // Un bloc dont le spectacle n'est pas choisi : meme nom.
    let mut s = saisie(c, MOIS);
    s["engagements"][0].as_object_mut().unwrap().remove("spectacle");
    s.as_object_mut().unwrap().remove("spectacle");
    let r = apercu(c, &s, false);
    assert_eq!(r["classe"], "SaisieIncomplete");
    // Un salarie inconnu : un refus du moteur, sans le nom que la page reconnait.
    let mut s = saisie(c, MOIS);
    s["engagements"][0]["salarie"] = json!("Personne Inventee");
    let r = apercu(c, &s, false);
    assert_eq!(r["ok"], json!(false));
    assert!(r.get("classe").is_none());
}

#[test]
fn sans_les_organismes_le_bordereau_attend_la_paie_non() {
    let c = &compagnie("organismes", &[MOIS]);
    std::fs::remove_file(c.join("config/organismes.json")).unwrap();
    let r = apercu(c, &saisie(c, MOIS), false);
    assert_eq!(r["resultat"]["urssaf"], Value::Null);
    assert!(r["resultat"]["avertissements"][0].as_str().unwrap().starts_with("Bordereau Urssaf pas encore calculable"));
    assert!(!r["resultat"]["paies"].as_array().unwrap().is_empty());
}
