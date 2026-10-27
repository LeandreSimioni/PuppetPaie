//! Les suites d'un arret en cours (21/10/2026) : l'avis de prolongation (`arret-prolonger`) et la reprise plus tot que
//! prevu (`arret-reprise`). Sur une copie de `fictive-3090-grande` avec ses DSN deposees : NICOLET, permanent (CDI
//! NI.24.01), arrete le 15/07/2026 ; le moteur au 20/07/2026 (un lundi).
//!
//! Les textes :
//! - la prolongation : « En cas de prolongation d'arret, il ne faut pas envoyer de nouveau signalement de nature " arret de
//!   travail ". Il suffit de declarer la nouvelle date de fin de l'arret en DSN mensuelle, au niveau de la rubrique " Date
//!   de fin previsionnelle - S21.G00.60.003 " » ; une subrogation « insuffisante » se corrige par « un nouveau signalement
//!   de nature " arret de travail " de type annule et remplace, dans lequel une nouvelle date de fin de subrogation sera
//!   renseignee » (fiche net-entreprises 1983, `atelier/reference/sources/net-entreprises-base-connaissances/
//!   net-entreprises-fiche1983-prolongation-arret-dates-de-subrogation-20250322.txt`, l. 13 et 37) ; ameli : « modifier la
//!   date de fin previsionnelle de l'arret de travail dans votre logiciel de paie » (`atelier/reference/sources/
//!   ameli-arret-de-travail/`) ;
//! - le bloc S21.G00.60 dans chaque DSN mensuelle que l'arret traverse (cahier technique DSN 2026.1, p. 265,
//!   `atelier/reference/norme-dsn/texte/bloc-S21.G00.60.txt`) ;
//! - la reprise anticipee : signalement 05 « qu'en cas de reprise anticipee par rapport a la date previsionnelle »
//!   (cahier technique § 5.4, `chapitre-5-modeles.txt`), dans les cinq jours ouvres sans subrogation (CSS R. 133-14, II,
//!   `atelier/reference/sources/code-de-la-securite-sociale/xml/R133-14.xml` ; arrete du 2 juillet 2013, art. 1er,
//!   `atelier/reference/sources/arrete-2013-delai-evenements-dsn/xml/1.xml`) ; une reprise a la date prevue n'appelle que
//!   la DSN mensuelle (ameli).

use crate::outils::{self, compagnie, dossier_de_test, ecrire, lire, refus, reussi};
use droit::contexte::Contexte;
use socle::date::Jour;
use paie::dsn::{generer, Arret, Options};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const LE: &str = "2026-07-20";

fn copie(test: &str) -> PathBuf {
    compagnie::ses_dsn("fictive-3090-grande", &[], &dossier_de_test("arret_prolonge", test))
}

fn route(racine: &Path, nom: &str, charge: Value) -> Value {
    outils::route(racine, nom, charge, LE)
}

/// L'arret de NICOLET : dernier jour travaille le 14/07, prescrit jusqu'au 31/07.
fn arreter(racine: &Path, subrogation: Value) -> Value {
    let mut charge = json!({"salarie": "NICOLET", "dernier_jour": "2026-07-14", "fin": "2026-07-31", "motif": "01"});
    for (k, v) in subrogation.as_object().unwrap() {
        charge[k] = v.clone();
    }
    reussi(&route(racine, "arret-enregistrer", charge))["arret"].clone()
}

fn prolonger(racine: &Path, charge: Value) -> Value {
    let mut c = json!({"salarie": "NICOLET", "dernier_jour": "2026-07-14"});
    for (k, v) in charge.as_object().unwrap() {
        c[k] = v.clone();
    }
    route(racine, "arret-prolonger", c)
}

/// L'arret tel que la route `imprevus` le rend, avec ses obligations.
fn l_arret(racine: &Path, jour: &str) -> Value {
    let r = outils::route(racine, "imprevus", json!({"salarie": "NICOLET"}), jour);
    reussi(&r)["imprevus"].as_array().unwrap().iter().find(|i| i["genre"] == json!("arret")).unwrap().clone()
}

fn cles(a: &Value) -> Vec<String> {
    a["obligations"].as_array().unwrap().iter().map(|o| o["cle"].as_str().unwrap().to_string()).collect()
}

fn la<'a>(a: &'a Value, cle: &str) -> &'a Value {
    a["obligations"].as_array().unwrap().iter().find(|o| o["cle"] == json!(cle)).unwrap_or_else(|| panic!("pas de {} : {:?}", cle, cles(a)))
}

/// La DSN mensuelle de `mois` telle que la production la fait : les paies du mois et les arrets du magasin qui le croisent.
fn dsn(racine: &Path, mois: &str) -> (String, Vec<engagement::calcul::Paie>) {
    let ctx = Contexte::nouveau(racine, Some(Jour::lire(LE).unwrap())).unwrap();
    let paies = engagement::charger(&ctx, &mois).unwrap().paies;
    let arrets: Vec<Arret> = engagement::imprevus::du_mois(racine, mois, Some(engagement::imprevus::ARRET)).unwrap().into_iter().map(|v| Arret { v }).collect();
    let o = Options { date_constitution: Some(Jour::lire("2026-10-15").unwrap()), envoi_test: true, ..Options::default() };
    (generer(&ctx, &paies, mois, &[], &arrets, &o).unwrap_or_else(|e| panic!("{} : {}", mois, e)), paies)
}

fn valeurs(texte: &str, rubrique: &str) -> Vec<String> {
    outils::paires(texte).into_iter().filter(|(r, _)| r == rubrique).map(|(_, v)| v).collect()
}

/// Le signalement que `arret-signaler` ecrit (et que le moteur a passe aux controles des usages et des formats).
fn signaler(racine: &Path, charge: Value) -> (Value, String) {
    let mut c = json!({"salarie": "NICOLET", "dernier_jour": "2026-07-14"});
    for (k, v) in charge.as_object().unwrap() {
        c[k] = v.clone();
    }
    let r = reussi(&route(racine, "arret-signaler", c)).clone();
    let texte = String::from_utf8_lossy(&std::fs::read(r["fichier"].as_str().unwrap()).unwrap()).into_owned();
    (r, texte)
}

/// Note sur l'arret un depot reel, comme `marques_depot` le fait apres un depot accepte.
fn depose(racine: &Path, envoi: Value) {
    let mut f = lire(racine, "imprevus/2026-07.json");
    f["imprevus"][0]["envois"].as_array_mut().unwrap().push(envoi);
    ecrire(racine, "imprevus/2026-07.json", &f);
}

/// Fiche 1983, l. 13 : la prolongation change la fin previsionnelle, et rien d'autre ne part. La DSN d'aout, que l'arret
/// ne croisait pas, porte desormais le bloc 60 avec la nouvelle fin (60.003) ; la paie d'aout retire les jours ajoutes
/// (le mecanisme de toujours, `imprevus::lignes_du_mois`) ; aucune obligation nouvelle ne nait ; un signalement d'arret pas
/// encore parti part avec la nouvelle fin, et passe les controles des usages et des formats.
#[test]
fn la_prolongation_change_la_fin_et_ne_fait_partir_aucun_signalement() {
    let c = &copie("prolongation");
    let a = arreter(c, json!({"subrogation": "02"}));
    assert_eq!((a["en_cours"].clone(), a["peut_prolonger"].clone(), a["peut_reprendre_plus_tot"].clone()), (json!(true), json!(true), json!(true)), "{}", a);
    let (aout, paies) = dsn(c, "2026-08");
    assert!(valeurs(&aout, "S21.G00.60.001").is_empty(), "avant la prolongation, l'arret ne croise pas aout");
    let nicolet = paies.iter().find(|p| p.salarie["_cle"] == json!("NICOLET")).unwrap();
    assert!(!nicolet.engagement.lignes.iter().any(|l| l["rubrique"] == json!("absence_maladie")));
    let avant = cles(&l_arret(c, LE));

    let r = reussi(&prolonger(c, json!({"fin": "2026-08-20"}))).clone();
    assert_eq!(r["arret"]["fin_previsionnelle"], json!("2026-08-20"), "{}", r);
    assert_eq!(r["subrogation_a_revoir"], json!(false));
    assert!(r["avertissements"][0].as_str().unwrap().contains("Rien ne part"), "{}", r);
    let p = &lire(c, "imprevus/2026-07.json")["imprevus"][0];
    assert_eq!(p["fin_previsionnelle"], json!("2026-08-20"));
    let trace = &p["prolongations"][0];
    assert_eq!((trace["le"].as_str(), trace["fin_avant"].as_str(), trace["fin"].as_str()), (Some(LE), Some("2026-07-31"), Some("2026-08-20")), "{}", p);
    assert_eq!(cles(&l_arret(c, LE)), avant, "une prolongation ne fait naitre aucune obligation (fiche 1983)");

    let (aout, paies) = dsn(c, "2026-08");
    assert_eq!(valeurs(&aout, "S21.G00.60.001"), vec!["01"]);
    assert_eq!(valeurs(&aout, "S21.G00.60.002"), vec!["14072026"]);
    assert_eq!(valeurs(&aout, "S21.G00.60.003"), vec!["20082026"], "la nouvelle fin previsionnelle part dans la mensuelle");
    let nicolet = paies.iter().find(|p| p.salarie["_cle"] == json!("NICOLET")).unwrap();
    assert!(nicolet.engagement.lignes.iter().any(|l| l["rubrique"] == json!("absence_maladie")), "la paie d'aout retire les jours ajoutes");
    let (juillet, _) = dsn(c, "2026-07");
    assert_eq!(valeurs(&juillet, "S21.G00.60.003"), vec!["20082026"], "juillet, pas encore depose, porte la fin connue");

    // Le signalement d'arret, pas encore parti : un signalement normal, avec la nouvelle fin (generer passe les controles).
    let (s, texte) = signaler(c, json!({}));
    assert_eq!((s["nature"].as_str(), s["type"].as_str()), (Some("04"), Some("01")));
    assert_eq!(valeurs(&texte, "S21.G00.60.003"), vec!["20082026"]);
}

/// Ce que la prolongation refuse n'ecrit rien : une fin plus tot (une reprise anticipee), la meme fin, une fin de
/// subrogation sur un arret qui n'est pas subroge (CCH-11 de S21.G00.60.006), un arret deja repris, un arret inconnu.
#[test]
fn ce_que_la_prolongation_refuse_n_ecrit_rien() {
    let c = &copie("refus");
    arreter(c, json!({"subrogation": "02"}));
    let avant = lire(c, "imprevus/2026-07.json");
    for (nom, charge, dit) in [
        ("plus tot", json!({"fin": "2026-07-25"}), "reprise anticipée"),
        ("la meme", json!({"fin": "2026-07-31"}), "déjà prescrit"),
        ("rien", json!({}), "rien n'est changé"),
        ("subrogation sans subrogation", json!({"fin": "2026-08-20", "subrogation_fin": "2026-09-30"}), "CCH-11"),
        ("date illisible", json!({"fin": "pas une date"}), ""),
    ] {
        let (_, m) = refus(&prolonger(c, charge));
        assert!(m.contains(dit), "{} : {}", nom, m);
    }
    assert!(refus(&route(c, "arret-prolonger", json!({"salarie": "NICOLET", "dernier_jour": "2026-07-13", "fin": "2026-08-20"}))).1.contains("Aucun arret"));
    reussi(&route(c, "arret-reprise", json!({"salarie": "NICOLET", "dernier_jour": "2026-07-14", "reprise": "2026-07-20", "motif_reprise": "01"})));
    let repris = lire(c, "imprevus/2026-07.json");
    assert!(refus(&prolonger(c, json!({"fin": "2026-08-20"}))).1.contains("a pris fin"));
    assert_eq!(lire(c, "imprevus/2026-07.json"), repris);
    assert_eq!(avant["imprevus"][0]["fin_previsionnelle"], repris["imprevus"][0]["fin_previsionnelle"]);
}

/// Fiche 1983, l. 25, 37, 66 et 110. L'arret subroge jusqu'au 31/07 est prolonge au 20/08 : la subrogation s'arrete
/// avant lui, le moteur le dit et demande (il ne connait pas le maximum de la convention, regle 1) ; repoussee au 14/10
/// APRES le depot du signalement d'arret, elle fait naitre le signalement rectificatif (04, annule et remplace), dont le
/// delai retenu est celui d'un signalement d'arret, cinq jours ouvres (fiche 673 ; inference dite) : saisie le lundi
/// 20/07, a faire au plus tard le lundi 27/07. Le rectificatif s'ecrit (type 03, la nouvelle fin de subrogation) et passe
/// les controles ; depose, l'obligation est faite.
#[test]
fn une_subrogation_prolongee_apres_le_signalement_appelle_un_annule_et_remplace() {
    let c = &copie("subrogation");
    arreter(c, json!({"subrogation": "01", "subrogation_debut": "2026-07-15", "subrogation_fin": "2026-07-31"}));
    depose(c, json!({"nature": "04", "type": "01", "ordre": 1, "fichier": "x", "quand": "2026-07-16 10:00", "identifiant": ""}));

    let r = reussi(&prolonger(c, json!({"fin": "2026-08-20"}))).clone();
    assert_eq!(r["subrogation_a_revoir"], json!(true), "{}", r);
    assert!(r["phrase"].as_str().unwrap().contains("31/07/2026"), "{}", r);
    assert!(!cles(&l_arret(c, LE)).contains(&"signalement_arret_rectificatif".to_string()), "la fin de subrogation n'a pas change");

    let r = reussi(&prolonger(c, json!({"subrogation_fin": "2026-10-14"}))).clone();
    assert_eq!(r["subrogation_a_revoir"], json!(false), "{}", r);
    assert_eq!(r["arret"]["subrogation_fin"], json!("2026-10-14"));
    assert!(r["avertissements"].to_string().contains("annule et"), "{}", r);
    let a = l_arret(c, LE);
    let o = la(&a, "signalement_arret_rectificatif");
    assert_eq!((o["depart"]["jour"].as_str(), o["butoir"].as_str(), o["etat"].as_str()), (Some(LE), Some("2026-07-27"), Some("a_faire")), "{}", o);
    assert!(o["textes"].to_string().contains("net-entreprises-fiche1983-prolongation-arret-dates-de-subrogation-20250322.txt"));
    assert_eq!(la(&a, "signalement_arret")["etat"], json!("fait"), "le signalement d'arret, lui, est parti");
    // Le rappel de l'accueil le porte (il lit les memes obligations).
    let rappels = outils::route(c, "rappels", json!({}), LE);
    assert!(rappels.to_string().contains("signalement_arret_rectificatif"), "{}", rappels);

    // Le rectificatif : un annule et remplace du signalement parti, avec la nouvelle fin de subrogation.
    let (s, texte) = signaler(c, json!({"remplace": "16072026123"}));
    assert_eq!((s["nature"].as_str(), s["type"].as_str()), (Some("04"), Some("03")), "{}", s);
    assert_eq!(valeurs(&texte, "S20.G00.05.002"), vec!["03"]);
    assert_eq!(valeurs(&texte, "S21.G00.60.003"), vec!["20082026"]);
    assert_eq!(valeurs(&texte, "S21.G00.60.006"), vec!["14102026"]);

    // Depose apres la saisie de la prolongation : fait.
    let saisi = lire(c, "imprevus/2026-07.json")["imprevus"][0]["prolongations"][1]["saisi_le"].as_str().unwrap().to_string();
    depose(c, json!({"nature": "04", "type": "03", "ordre": 2, "fichier": "y", "quand": saisi, "identifiant": ""}));
    assert_eq!(la(&l_arret(c, LE), "signalement_arret_rectificatif")["etat"], json!("fait"));
}

/// La reprise plus tot que prevu, apres une prolongation : prescrit jusqu'au 20/08, le salarie revient le lundi 10/08.
/// C'est une reprise anticipee par rapport a la fin previsionnelle (cahier technique § 5.4) : le signalement de reprise
/// (05) nait, sans subrogation dans les cinq jours ouvres (R. 133-14, II ; arrete du 2 juillet 2013), soit au plus tard le
/// lundi 17/08 ; il s'ecrit et passe les controles. L'arret n'est plus en cours ; la reprise se corrige tant que son
/// signalement n'est pas parti. Une reprise a la date prevue n'appelle aucun signalement ; une reprise apres la fin
/// previsionnelle se refuse et dit de prolonger.
#[test]
fn la_reprise_anticipee_fait_naitre_le_signalement_de_reprise_a_cinq_jours_ouvres() {
    let c = &copie("reprise");
    arreter(c, json!({"subrogation": "02"}));
    reussi(&prolonger(c, json!({"fin": "2026-08-20"})));
    let (_, m) = refus(&route(c, "arret-reprise", json!({"salarie": "NICOLET", "dernier_jour": "2026-07-14", "reprise": "2026-08-25", "motif_reprise": "01"})));
    assert!(m.contains("prolonger"), "{}", m);

    let r = reussi(&outils::route(c, "arret-reprise", json!({"salarie": "NICOLET", "dernier_jour": "2026-07-14", "reprise": "2026-08-10", "motif_reprise": "01"}), "2026-08-10")).clone();
    let a = &r["arret"];
    assert_eq!((a["en_cours"].clone(), a["peut_prolonger"].clone(), a["reprise_corrigeable"].clone()), (json!(false), json!(false), json!(true)), "{}", a);
    let a = l_arret(c, "2026-08-10");
    let o = la(&a, "signalement_reprise");
    assert_eq!((o["depart"]["jour"].as_str(), o["butoir"].as_str(), o["etat"].as_str()), (Some("2026-08-10"), Some("2026-08-17"), Some("a_faire")), "{}", o);
    let (s, texte) = signaler(c, json!({"nature": "05"}));
    assert_eq!(s["nature"], json!("05"));
    assert_eq!(valeurs(&texte, "S21.G00.60.010"), vec!["10082026"]);
    assert_eq!(valeurs(&texte, "S21.G00.60.003"), vec!["20082026"]);
    // La DSN d'aout porte la reprise (60.010) sous le bloc 60.
    let (aout, _) = dsn(c, "2026-08");
    assert_eq!(valeurs(&aout, "S21.G00.60.010"), vec!["10082026"]);
    // Le 05 depose : la reprise ne se corrige plus depuis la page.
    let mut f = lire(c, "imprevus/2026-07.json");
    f["imprevus"][0]["envois"] = json!([{"nature": "05", "type": "01", "ordre": 1, "fichier": "z", "quand": "2026-08-11 09:00", "identifiant": ""}]);
    ecrire(c, "imprevus/2026-07.json", &f);
    assert_eq!(l_arret(c, "2026-08-11")["reprise_corrigeable"], json!(false));

    // Reprise a la date prevue (le 20/08, la fin previsionnelle, que 60.010 dit « date de reprise previsionnelle ») :
    // aucun signalement de reprise ; la DSN mensuelle seule.
    let c = &copie("reprise-prevue");
    arreter(c, json!({"subrogation": "02"}));
    reussi(&route(c, "arret-reprise", json!({"salarie": "NICOLET", "dernier_jour": "2026-07-14", "reprise": "2026-07-31", "motif_reprise": "01"})));
    assert!(!cles(&l_arret(c, "2026-08-03")).contains(&"signalement_reprise".to_string()));
}

/// UN ARRET EN COURS : sans reprise saisie, tant que sa fin previsionnelle n'est pas passee, ou que la DSN du mois ou
/// elle tombe n'est pas partie (l'avis de prolongation peut arriver apres la fin prescrite) ; une fois cette DSN partie,
/// l'arret y est declare fini a sa date prevue (ameli : la reprise a la date prevue « doit etre signalee dans la DSN
/// mensuelle ») et n'est plus en cours.
#[test]
fn un_arret_est_en_cours_jusqu_a_la_dsn_du_mois_de_sa_fin() {
    let c = &copie("en-cours");
    arreter(c, json!({"subrogation": "02"}));
    assert_eq!(l_arret(c, "2026-07-31")["en_cours"], json!(true));
    assert_eq!(l_arret(c, "2026-08-04")["en_cours"], json!(true), "fin passee, DSN de juillet pas partie");
    depose(c, json!({"nature": "01", "type": "01", "ordre": 0, "mois": "2026-07", "fichier": "d", "quand": "2026-08-05 09:00", "identifiant": ""}));
    let a = l_arret(c, "2026-08-06");
    assert_eq!((a["en_cours"].clone(), a["peut_prolonger"].clone(), a["reprise_corrigeable"].clone()), (json!(false), json!(false), json!(false)), "{}", a);
    // Le moteur admet encore l'avis de prolongation (rien ne le lui interdit) ; il dit que juillet est deja parti si
    // des jours ajoutes y tombaient -- ici, ils sont tous en aout.
    let r = reussi(&outils::route(c, "arret-prolonger", json!({"salarie": "NICOLET", "dernier_jour": "2026-07-14", "fin": "2026-08-20"}), "2026-08-06")).clone();
    assert_eq!(r["avertissements"].as_array().unwrap().len(), 1, "{}", r);
}
