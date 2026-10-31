//! UN CONTRAT D'USAGE A CHEVAL SUR DEUX MOIS EST UNE SEULE EMBAUCHE : sa DPAE et son contrat se font a son premier jour
//! (code du travail, L. 1221-10 et R. 1221-1, 3°, `atelier/reference/sources/code-du-travail/xml/`), et la DSN de son
//! second mois le declare depuis ce jour-la (S21.G00.40.001, `norme-dsn/texte/bloc-S21.G00.40.txt`). Le banc, FULLER
//! engagee les 20 et 21/11 avec deux jours planifies les 02 et 03/12, vit jusqu'au 16/12 : decembre ne refait ni DPAE ni
//! contrat, et sa DSN dit le contrat du 20/11.
//!
//! L'AVENANT, LUI, SE VOIT A CHAQUE MOIS DU CONTRAT (un bug qui a echappe : le carre « Contrat » d'un mois qui n'est pas
//! celui de l'embauche restait vert, « Contrat signe », alors que l'avertissement de « Modifier l'engagement » venait de
//! dire qu'un avenant est a faire signer).

use std::path::PathBuf;

use serde_json::{json, Value};

use crate::outils::rejeu::{rejouer_dossier, scenario};
use crate::outils::{dossier_de_test, noms, refus, reussi, route, saisie};

/// Le banc rejoue jusqu'au 16/12, FULLER engagee les 20 et 21/11 et planifiee les 02 et 03/12.
fn fuller_a_cheval(cas: &str) -> PathBuf {
    let d = dossier_de_test("contrat_a_cheval", cas);
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    let gestes = std::fs::read_to_string(scenario("treteaux-du-banc").join("gestes.jsonl")).unwrap();
    let novembre = r#""dates":["2026-11-20","2026-11-21"]}"#;
    assert!(gestes.contains(novembre));
    let gestes = gestes.replace(novembre, r#""dates":["2026-11-20","2026-11-21"],"planning":["2026-12-02","2026-12-03"]}"#);
    std::fs::write(sc.join("gestes.jsonl"), gestes).unwrap();
    rejouer_dossier(&sc, &d).1
}

#[test]
fn le_second_mois_d_un_contrat_d_usage_n_est_pas_une_embauche() {
    let c = fuller_a_cheval("second_mois");
    let dpae: Vec<String> = noms(&c.join("retours/depots")).into_iter().filter(|n| n.contains("depot-dpae")).collect();
    assert!(dpae.iter().all(|n| n.contains("_202611")), "une DPAE est repartie en decembre : {:?}", dpae);
    assert!(!c.join("sorties/2026-12/envoi").exists(), "un contrat est refait en decembre : {:?}", noms(&c.join("sorties/2026-12/envoi")));
    let jour = "2026-12-20";
    let ctx = droit::contexte::Contexte::nouveau(&c, Some(socle::date::Jour::lire(jour).unwrap())).unwrap();
    assert_eq!(moteur::routes::etapes::Moteur::nouveau(&ctx).embauches_a_declarer(None).unwrap(), json!([]));

    let ecrite = reussi(&route(&c, "dsn-a-deposer", json!({"mois": "2026-12"}), jour)).clone();
    let decembre = String::from_utf8_lossy(&std::fs::read(ecrite["chemin"].as_str().unwrap()).unwrap()).to_string();
    for attendu in ["S21.G00.40.001,'20112026'", "S21.G00.40.009,'FU1.26.01'", "S21.G00.40.010,'03122026'", "S21.G00.40.013,'4.00'"] {
        assert!(decembre.contains(attendu), "{} manque :\n{}", attendu, decembre);
    }
}

/// LA CARTE DU SECOND MOIS RENVOYEE TELLE QUE LE MOTEUR LA REND NE CHANGE RIEN (un bug qui a echappe : le debut qu'un
/// contrat d'usage deduit de son premier jour, rendu au second mois, revenait comme une saisie et faisait « un avenant à
/// faire signer » sans que rien n'ait change).
#[test]
fn la_carte_du_second_mois_renvoyee_telle_quelle_ne_change_rien() {
    let c = fuller_a_cheval("aller_retour");
    let jour = "2026-12-16";
    let fuller: Value = saisie(&c, "2026-12")["engagements"].as_array().unwrap().iter().find(|e| e["salarie"] == "FULLER").cloned().unwrap();
    let r = route(&c, "modifier-engagement", json!({"mois": "2026-12", "engagement": crate::outils::identite(&c, "2026-12", "FULLER", None), "saisie": fuller}), jour);
    assert_eq!(reussi(&r)["change"], json!(false), "{}", r);
}

/// LA DATE DE DEBUT D'UN CONTRAT DEJA DECLARE, CHANGEE AU SECOND MOIS : la DSN de decembre declare la nouvelle et, dans
/// un bloc Changements contrat, l'ancienne (41.021, § 2.1.2.3.2 de `norme-dsn/texte/chapitre-2-principes-de-
/// constitution.txt`, p. 21). Ce bloc se date de tout jour pour un identifiant (meme paragraphe), au plus tard le dernier
/// du mois declare (CCH-15 de 41.001, `bloc-S21.G00.41.txt`, p. 217) : sans avenant date, ce dernier jour.
#[test]
fn la_date_de_debut_changee_se_declare_avec_l_ancienne() {
    let c = fuller_a_cheval("date_de_debut");
    let jour = "2026-12-16";
    let mut fuller: Value = saisie(&c, "2026-12")["engagements"].as_array().unwrap().iter().find(|e| e["salarie"] == "FULLER").cloned().unwrap();
    fuller["debut"] = json!("2026-11-19");
    let r = route(&c, "modifier-engagement", json!({"mois": "2026-12", "confirme": true,
        "engagement": crate::outils::identite(&c, "2026-12", "FULLER", None), "saisie": fuller}), jour);
    assert_eq!(reussi(&r)["change"], json!(true), "{}", r);

    let ecrite = reussi(&route(&c, "dsn-a-deposer", json!({"mois": "2026-12"}), "2026-12-20")).clone();
    let decembre = String::from_utf8_lossy(&std::fs::read(ecrite["chemin"].as_str().unwrap()).unwrap()).to_string();
    let bloc = "S21.G00.41.001,'31122026'\r\nS21.G00.41.021,'20112026'";
    assert!(decembre.contains("S21.G00.40.001,'19112026'") && decembre.contains(bloc), "{}", decembre);
}

/// UNE DATE DE DEBUT CHANGEE APRES LE DEPART DU CONTRAT SE FAIT SIGNER EN AVENANT de ce contrat, pas en contrat neuf (un
/// bug qui a echappe : le contrat se lisait au journal sous sa date de debut ; changee, il passait pour jamais parti,
/// « Contrat à envoyer », et l'horloge en faisait partir un neuf, alors que l'avertissement annoncait un avenant). Le
/// numero ne change plus des qu'un envoi l'a porte (`engagement::numeros`) : c'est lui qui retrouve le contrat parti, et
/// l'avenant comme sa signature se rangent sous le jour ou le contrat est parti.
#[test]
fn la_date_de_debut_changee_se_fait_signer_en_avenant() {
    let c = fuller_a_cheval("debut_en_avenant");
    let jour = "2026-12-16";
    let mut fuller: Value = saisie(&c, "2026-12")["engagements"].as_array().unwrap().iter().find(|e| e["salarie"] == "FULLER").cloned().unwrap();
    let numero = fuller["numero"].clone();
    fuller["debut"] = json!("2026-11-19");
    let r = route(&c, "modifier-engagement", json!({"mois": "2026-12", "confirme": true,
        "engagement": crate::outils::identite(&c, "2026-12", "FULLER", None), "saisie": fuller}), jour);
    assert_eq!(reussi(&r)["change"], json!(true), "{}", r);

    let contrat = |c: &PathBuf| -> Value {
        let a = reussi(&route(c, "signature-apercu", json!({"mois": "2026-11"}), jour)).clone();
        a["contrats"].as_array().and_then(|l| l.iter().find(|x| x["salarie"] == "FULLER")).cloned().unwrap_or(a)
    };
    let x = contrat(&c);
    assert!(x["recu_le"].is_string() && x["en_avenant"] == json!(true), "le contrat signe passe pour jamais parti : {}", x);

    // L'avenant dit l'ancienne date et la nouvelle ; imprime puis note signe, le contrat est a jour.
    let charge = json!({"salarie": "FULLER", "debut": "2026-11-19", "numero": numero, "mois": "2026-11"});
    let a = reussi(&route(&c, "avenant-apercu", charge.clone(), jour)).clone();
    let m = a["modifications"].to_string();
    assert!(m.contains("20/11/2026") && m.contains("19/11/2026"), "{}", a);
    reussi(&route(&c, "contrats-imprimer", json!({"mois": "2026-11", "avenants": [charge]}), jour));
    let x = contrat(&c);
    assert_eq!(x["avenant_attendu"], json!(true), "{}", x);
    let mut recu = x["recu"]["charge"].clone();
    recu["recu"] = json!(true);
    reussi(&route(&c, "contrat-recu", recu, jour));
    let x = contrat(&c);
    assert!(x["caduc"] != json!(true) && x["en_attente"] != json!(true) && x["a_envoyer"] != json!(true), "{}", x);
}

/// LA RUPTURE SE NOTIFIE PENDANT LE CONTRAT (CCH-11 de S21.G00.62.003, `atelier/reference/norme-dsn/texte/bloc-S21.G00.62.txt` :
/// la date de notification est « supérieure ou égale à la date de début de contrat de travail et inférieure ou égale à
/// la date de fin ») : une date hors du contrat se refuse a la validation, en disant les bornes, et rien ne s'ecrit.
#[test]
fn une_notification_hors_du_contrat_se_refuse_a_la_validation() {
    let c = fuller_a_cheval("notification");
    let jour = "2026-12-16";
    let avant = saisie(&c, "2026-12");
    let mut fuller: Value = avant["engagements"].as_array().unwrap().iter().find(|e| e["salarie"] == "FULLER").cloned().unwrap();
    fuller["notification_rupture"] = json!("2026-11-14");
    let r = route(&c, "modifier-engagement", json!({"mois": "2026-12", "confirme": true,
        "engagement": crate::outils::identite(&c, "2026-12", "FULLER", None), "saisie": fuller}), jour);
    let (_, phrase) = refus(&r);
    assert!(phrase.contains("14/11/2026") && phrase.contains("20/11/2026") && phrase.contains("03/12/2026"), "{}", r);
    assert_eq!(saisie(&c, "2026-12"), avant, "une notification refusee s'est ecrite");
}
