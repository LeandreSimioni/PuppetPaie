//! LES CONSEILS « Le saviez-vous ? » (ticket 224) par les routes : `apercu` (le conseil de saisie, champ `conseil` d'une
//! paie), `conseil-du-jour`, `conseils-salarie`, `conseils-resume`. Sur la configuration de `fictive-3090-petite`
//! (convention 3090, annexe 4 ; BRIOCHE marionnettiste, artiste de l'annexe X ; LEMERCIER régisseur, technicien de
//! l'annexe VIII), et une saisie d'octobre 2026 propre à chaque test, calculée en mémoire.

use crate::outils::{compagnie, dossier_de_test, ecrire, lire, reussi, route};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const JOUR: &str = "2026-10-21";

fn compagnie(nom: &str, conseils: bool) -> PathBuf {
    let racine = compagnie::saisies("fictive-3090-petite", &[], &dossier_de_test("conseils", nom).join("compagnie"));
    let mut ent = lire(&racine, "config/entreprise.json");
    ent["conseils"] = json!(conseils);
    ecrire(&racine, "config/entreprise.json", &ent);
    racine
}

/// Un engagement d'octobre 2026 sur « Ubu roi ».
fn engagement(salarie: &str, debut: &str, fin: &str, lignes: Value) -> Value {
    json!({ "salarie": salarie, "spectacle": "256Z00920011", "debut": debut, "fin": fin, "lignes": lignes })
}

fn saisie(engagements: Vec<Value>) -> Value {
    json!({ "mois": "2026-10", "taux_cachet": 150.0, "date_paiement": "2026-10-31", "engagements": engagements })
}

fn dates(n: usize) -> Vec<String> {
    (1..=n).map(|j| format!("2026-10-{:02}", j)).collect()
}

fn cachets(n: usize) -> Value {
    json!({ "rubrique": "cachet", "quantite": n, "taux": 150.0, "dates": dates(n) })
}

fn apercu(c: &Path, s: &Value) -> Value {
    reussi(&route(c, "apercu", json!({ "saisie": s }), JOUR)).clone()
}

/// Le conseil de la `i`-ème paie de l'apercu : (son identifiant, son texte), ou rien.
fn conseil(r: &Value, i: usize) -> Option<(String, String)> {
    let c = r["paies"][i].get("conseil")?;
    Some((c["id"].as_str().unwrap().to_string(), c["texte"].as_str().unwrap().to_string()))
}

fn id(r: &Value, i: usize) -> Option<String> {
    conseil(r, i).map(|(id, _)| id)
}

/// Un conseil n'est qu'un champ de l'ecran : ni avertissement ni information de la paie (ceux-la vont au bulletin et a
/// `produire`), ni de l'apercu du mois.
fn nulle_part_ailleurs(r: &Value, texte: &str) {
    let mut avis: Vec<&Value> = Vec::new();
    for p in r["paies"].as_array().unwrap() {
        avis.extend(p["avertissements"].as_array().unwrap());
        avis.extend(p["informations"].as_array().unwrap());
    }
    avis.extend(r["avertissements"].as_array().unwrap());
    avis.extend(r["informations"].as_array().unwrap());
    assert!(avis.iter().all(|a| a.as_str().unwrap() != texte), "le conseil est passe dans les avis : {}", texte);
}

/// Des contrats declares avant le logiciel (`historique/engagements.json`), pour BRIOCHE : `n` cachets en un mois.
fn historique(c: &Path, lignes: &[(&str, &str, i64)]) {
    let e: Vec<Value> = lignes
        .iter()
        .map(|(debut, fin, n)| json!({ "salarie": "BRIOCHE", "debut": debut, "fin": fin, "heures": 0, "cachets": n, "jours": n, "brut": "1000" }))
        .collect();
    ecrire(c, "historique/engagements.json", &json!({ "engagements": e }));
}

// ------------------------------------------------------------- l'option

#[test]
fn l_option_decochee_ne_montre_aucun_conseil_nulle_part() {
    // Rien de remplissable ni d'affiche « au cas ou » (atelier/CLAUDE.md) : sans l'option de la fiche Entreprise, la
    // saisie qui appellerait trois conseils n'en montre aucun, ni l'accueil ni le dossier du salarie.
    let c = &compagnie("decochee", false);
    historique(c, &[("2026-05-01", "2026-05-31", 31), ("2026-06-01", "2026-06-30", 20)]);
    let s = saisie(vec![engagement("Brioche", "2026-10-01", "2026-10-30", json!([cachets(30)]))]);
    let r = apercu(c, &s);
    assert!(r["paies"][0].get("conseil").is_none(), "{}", r["paies"][0]);
    assert_eq!(reussi(&route(c, "conseil-du-jour", json!({}), JOUR))["conseil"], Value::Null);
    assert_eq!(reussi(&route(c, "conseils-salarie", json!({ "salarie": "BRIOCHE" }), JOUR))["conseils"], json!([]));
    assert_eq!(reussi(&route(c, "conseils-resume", json!({}), JOUR))["conseils"], json!([]));
}

// ---------------------------------------------------------- la saisie

#[test]
fn au_dela_de_28_cachets_dans_le_mois_les_suivants_ne_comptent_pas() {
    // Annexe X, art. 3 §1 : « Le nombre maximum de cachets pris en compte pour la recherche de la durée d'affiliation
    // requise est de 28 par mois. » ; art. 12 §2 : ni dans le salaire de reference. Une information, pas un report.
    let c = &compagnie("vingt-huit", true);
    let r = apercu(c, &saisie(vec![engagement("Brioche", "2026-10-01", "2026-10-30", json!([cachets(30)]))]));
    let (id, texte) = conseil(&r, 0).expect("30 cachets appellent le conseil");
    assert_eq!(id, "saisie-plus-de-28-cachets");
    assert!(texte.starts_with("BRIOCHE ") && texte.contains("30 cachets en octobre 2026") && texte.contains("2 cachets ne compteront pas"), "{}", texte);
    nulle_part_ailleurs(&r, &texte);
    // 28 cachets : aucun ne se perd (le mois est sature, c'est l'autre conseil qui parle).
    let r = apercu(c, &saisie(vec![engagement("Brioche", "2026-10-01", "2026-10-28", json!([cachets(28)]))]));
    assert_ne!(id_de(&r), Some("saisie-plus-de-28-cachets".into()));
}

fn id_de(r: &Value) -> Option<String> {
    id(r, 0)
}

#[test]
fn un_mois_sature_chez_vous_ne_retire_rien_de_plus() {
    // Annexe X, art. 30 : « En cas d'exercice d'une activité au moins égale à 27 jours de travail par mois calendaire,
    // déterminés en fonction du nombre d'heures de travail effectuées à raison de dix heures par jour, aucune
    // indemnisation n'est servie. » 23 cachets font 276 heures (guide France Travail : un cachet compte 12 heures) : au
    // moins 270. 22 cachets, 264 heures : non.
    let c = &compagnie("sature", true);
    let r = apercu(c, &saisie(vec![engagement("Brioche", "2026-10-01", "2026-10-23", json!([cachets(23)]))]));
    let (id, texte) = conseil(&r, 0).unwrap();
    assert_eq!(id, "saisie-mois-sature");
    assert!(texte.contains("276 heures en octobre 2026") && texte.contains("270 heures"), "{}", texte);
    let r = apercu(c, &saisie(vec![engagement("Brioche", "2026-10-01", "2026-10-22", json!([cachets(22)]))]));
    assert_eq!(id_de(&r), None, "{}", r["paies"][0]);
}

#[test]
fn un_technicien_n_a_que_208_heures_retenues_par_mois() {
    // Guide France Travail : « De 208 heures pour les ouvriers et techniciens : cette limite est majorée à 250 heures
    // si vous avez travaillé pour différents employeurs sur le mois » ; annexe VIII, art. 3 §1.
    let c = &compagnie("technicien", true);
    let heures = |q: i64| json!([{ "rubrique": "heure", "quantite": q, "taux": 18.0, "dates": dates(26) }]);
    let r = apercu(c, &saisie(vec![engagement("Lemercier", "2026-10-01", "2026-10-26", heures(210))]));
    let (id, texte) = conseil(&r, 0).unwrap();
    assert_eq!(id, "saisie-technicien-plus-de-208-heures");
    assert!(texte.starts_with("LEMERCIER ") && texte.contains("210 heures") && texte.contains("250"), "{}", texte);
    let r = apercu(c, &saisie(vec![engagement("Lemercier", "2026-10-01", "2026-10-26", heures(200))]));
    assert_ne!(id_de(&r), Some("saisie-technicien-plus-de-208-heures".into()));
}

fn repetition(taux: f64) -> Value {
    json!([{ "rubrique": "repetition", "quantite": 1, "taux": taux, "heures": 4, "dates": ["2026-10-05"] }])
}

#[test]
fn une_repetition_payee_au_moins_le_cachet_de_repetition_compterait_douze_heures() {
    // CCN 3090, avenant salaires du 21/07/2025 : un « Cachet de répétition (pour 1 ou 2 services de répétition dans la
    // même journée) » ; annexe X, art. 3 §1 : un cachet vaut 12 heures. Un service de 4 heures paye 100 EUR (au moins
    // le cachet de repetition de l'annexe 4, 94,34 EUR) compterait 8 heures de plus ; le conseil dit ses deux faces et
    // le cout (art. 30 : 8 h / 10 x 1,3 = 1,04 jour, environ 1,5 au demi-jour superieur).
    let c = &compagnie("repetition", true);
    let r = apercu(c, &saisie(vec![engagement("Brioche", "2026-10-05", "2026-10-05", repetition(100.0))]));
    let (id, texte) = conseil(&r, 0).unwrap();
    assert_eq!(id, "saisie-repetition-en-cachet");
    assert!(texte.contains("8 heures de plus") && texte.contains("94,34") && texte.contains("s'il lui manque des heures") && texte.contains("mieux vaut s'abstenir"), "{}", texte);
    assert!(!texte.contains('€') || !texte.contains("allocation journalière"), "{}", texte);
    nulle_part_ailleurs(&r, &texte);
    // Sous le minimum : rien (un conseil ne propose jamais moins que la convention).
    let r = apercu(c, &saisie(vec![engagement("Brioche", "2026-10-05", "2026-10-05", repetition(90.0))]));
    assert_eq!(id_de(&r), None, "{}", r["paies"][0]);
}

#[test]
fn des_heures_payees_au_dessus_du_minimum_couvriraient_plus_d_heures_pour_la_meme_somme() {
    // Annexe VIII, art. 30 (le cout : heures / 8 x 1,4) et art. 3 §1 (les 507 heures) : 4 heures a 50 EUR, 200 EUR, bien
    // au-dessus du minimum horaire de la grille du regisseur ; la meme somme au minimum couvre davantage d'heures.
    let c = &compagnie("heures", true);
    let ligne = |taux: f64| json!([{ "rubrique": "heure", "quantite": 4, "taux": taux, "dates": ["2026-10-05"] }]);
    let r = apercu(c, &saisie(vec![engagement("Lemercier", "2026-10-05", "2026-10-05", ligne(50.0))]));
    let (id, texte) = conseil(&r, 0).unwrap();
    assert_eq!(id, "saisie-heures-pour-la-meme-somme");
    assert!(texte.contains("4 heures payées 200,00 €") && texte.contains("mieux vaut s'abstenir"), "{}", texte);
    nulle_part_ailleurs(&r, &texte);
    // Sous le minimum horaire : rien.
    let r = apercu(c, &saisie(vec![engagement("Lemercier", "2026-10-05", "2026-10-05", ligne(5.0))]));
    assert_eq!(id_de(&r), None, "{}", r["paies"][0]);
}

#[test]
fn quand_vos_contrats_assurent_les_507_heures_le_conseil_s_inverse() {
    // Annexe X, art. 3 §1 : 507 heures sur les douze mois. 45 cachets declares chez nous (540 heures) dans la fenetre :
    // ses droits sont ouverts grace a nos seuls contrats. Le conseil « cachet de repetition » cede la place a
    // « intense plutot que dilue » (art. 30 : chaque tranche de 10 heures retire 1,3 jour d'allocation).
    let c = &compagnie("assurees", true);
    historique(c, &[("2026-05-01", "2026-05-25", 25), ("2026-06-01", "2026-06-20", 20)]);
    let r = apercu(c, &saisie(vec![engagement("Brioche", "2026-10-05", "2026-10-05", repetition(100.0))]));
    let (id, texte) = conseil(&r, 0).unwrap();
    assert_eq!(id, "saisie-507-assurees");
    assert!(texte.contains("544 heures") && texte.contains("intense que dilué") && texte.contains("1,3 jour"), "{}", texte);
}

#[test]
fn des_cachets_et_des_heures_se_declarent_chacun_dans_leur_unite() {
    // Guide France Travail, « Vos obligations » : « Déclaration des artistes : ne reportez pas l'équivalence de vos
    // cachets en heures sur vos déclarations. »
    let c = &compagnie("cachets-heures", true);
    let lignes = json!([
        { "rubrique": "cachet", "quantite": 2, "taux": 150.0, "dates": ["2026-10-05", "2026-10-06"] },
        { "rubrique": "heure", "quantite": 3, "taux": 15.0, "dates": ["2026-10-04"] }
    ]);
    let r = apercu(c, &saisie(vec![engagement("Brioche", "2026-10-04", "2026-10-06", lignes)]));
    assert_eq!(id_de(&r).as_deref(), Some("saisie-cachets-et-heures"), "{}", r["paies"][0]);
}

// -------------------------------------------------------- l'historique

#[test]
fn nos_seuls_contrats_au_dela_de_507_heures_ouvrent_ses_droits() {
    // Annexe X, art. 3 §1. Le seul cas de certitude : nos contrats font a eux seuls 540 heures sur les 12 mois glissants.
    let c = &compagnie("droits-ouverts", true);
    historique(c, &[("2026-05-01", "2026-05-31", 31), ("2026-06-01", "2026-06-17", 17)]);
    let r = reussi(&route(c, "conseils-salarie", json!({ "salarie": "BRIOCHE" }), JOUR)).clone();
    let textes: Vec<String> = r["conseils"].as_array().unwrap().iter().map(|x| x["texte"].as_str().unwrap().to_string()).collect();
    let ids: Vec<&str> = r["conseils"].as_array().unwrap().iter().map(|x| x["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&"historique-droits-ouverts"), "{:?}", ids);
    let ouverts = &textes[ids.iter().position(|i| *i == "historique-droits-ouverts").unwrap()];
    // 28 cachets retenus en mai (31 declares) et 17 en juin : 45 cachets, 540 heures.
    assert!(ouverts.contains("540 heures") && ouverts.contains("grâce à vous") && ouverts.contains("sur les 12 mois du 21/10/2025 au 21/10/2026"), "{}", ouverts);
    // En mai, 31 cachets : les 3 derniers ne comptent pas.
    assert!(ids.contains(&"historique-cachets-perdus"), "{:?}", ids);
    assert!(textes.iter().any(|t| t.contains("En mai 2026") && t.contains("3 cachets ne comptent pas")), "{:?}", textes);
    // L'accueil le resume.
    let resume = reussi(&route(c, "conseils-resume", json!({}), JOUR)).clone();
    assert!(resume["conseils"][0]["texte"].as_str().unwrap().contains("BRIOCHE"), "{}", resume);
}

#[test]
fn sous_507_heures_chez_nous_on_dit_la_part_pas_les_droits() {
    let c = &compagnie("part", true);
    historique(c, &[("2026-05-01", "2026-05-10", 10)]);
    let r = reussi(&route(c, "conseils-salarie", json!({ "salarie": "BRIOCHE" }), JOUR)).clone();
    let ids: Vec<&str> = r["conseils"].as_array().unwrap().iter().map(|x| x["id"].as_str().unwrap()).collect();
    assert!(!ids.contains(&"historique-droits-ouverts"), "{:?}", ids);
    let t = r["conseils"][0]["texte"].as_str().unwrap();
    assert_eq!(ids[0], "historique-heures-comptees");
    // 10 cachets, 120 heures : 24 % des 507.
    assert!(t.contains("120 heures") && t.contains("24 %"), "{}", t);
    assert_eq!(reussi(&route(c, "conseils-resume", json!({}), JOUR))["conseils"], json!([]));
}

#[test]
fn la_fenetre_part_de_la_date_anniversaire_quand_la_fiche_la_porte() {
    // `cumul::fenetre_anniversaire` : les 12 mois qui precedent la date anniversaire notifiee ; la phrase le dit.
    let c = &compagnie("anniversaire", true);
    historique(c, &[("2026-05-01", "2026-05-10", 10)]);
    crate::outils::retoucher_fiches(c, &[("BRIOCHE", json!({ "anniversaire_droits": "2027-02-01" }))]);
    let r = reussi(&route(c, "conseils-salarie", json!({ "salarie": "BRIOCHE" }), JOUR)).clone();
    let t = r["conseils"][0]["texte"].as_str().unwrap();
    assert!(t.contains("les 12 mois avant sa date anniversaire") && t.contains("01/02/2027"), "{}", t);
}

// ---------------------------------------------------------- le demarrage

#[test]
fn le_conseil_du_jour_fait_le_tour_de_la_banque_sans_se_repeter() {
    // Sans rien ecrire : N jours de suite montrent N conseils differents, et « un autre » en montre un autre.
    let c = &compagnie("jour", true);
    let premier = reussi(&route(c, "conseil-du-jour", json!({}), "2026-01-01")).clone();
    let n = premier["total"].as_u64().unwrap() as usize;
    assert!(n > 100, "{}", n);
    let mut vus = std::collections::BTreeSet::new();
    // Une annee civile : le bareme de l'annee fournit les chiffres dates (un conseil dont le gabarit ne se resout pas se tait).
    let mut jour = paie::date::Jour::lire("2026-01-01").unwrap();
    for _ in 0..n {
        let r = reussi(&route(c, "conseil-du-jour", json!({}), &jour.iso())).clone();
        assert!(vus.insert(r["conseil"]["id"].as_str().unwrap().to_string()), "repetition avant le tour : {}", r);
        jour = jour.plus_jours(1).unwrap();
    }
    let autre = reussi(&route(c, "conseil-du-jour", json!({ "decalage": 1 }), "2026-01-01")).clone();
    assert_ne!(autre["conseil"]["id"], premier["conseil"]["id"]);
    // Un conseil suspendu (point non tranche) n'est jamais du nombre.
    assert!(!vus.contains("conges-spectacles-droit-ouvre-1er-jour") && !vus.contains("signalement-arret-cinq-jours"));
    assert!(premier["conseil"]["reference"].as_str().is_some_and(|r| !r.is_empty()));
}


#[test]
fn l_enseignement_compte_dans_sa_limite() {
    // Annexe X, art. 3 §1 : « La limite de 70 heures est portée à 120 heures pour les artistes âgés de 50 ans et plus » ;
    // les limites sont celles du catalogue (rubrique « enseignement », `plafond_annuel_heures`).
    let c = &compagnie("enseignement", true);
    let lignes = json!([{ "rubrique": "enseignement", "quantite": 6, "taux": 30.0, "dates": ["2026-10-05"] }]);
    let r = apercu(c, &saisie(vec![engagement("Seraphin", "2026-10-05", "2026-10-05", lignes)]));
    let (id, texte) = conseil(&r, 0).unwrap();
    assert_eq!(id, "saisie-enseignement");
    assert!(texte.contains("70 heures") && texte.contains("120"), "{}", texte);
}

#[test]
fn l_historique_chiffre_ce_que_les_conseils_auraient_change() {
    // Un mois enregistre de la fenetre : deux journees de repetition d'un service de 4 heures payees 100 EUR, au-dessus du
    // cachet de repetition de l'annexe 4 (CCN 3090) : payees en cachets (annexe X, art. 3 §1, 12 heures), 16 heures de
    // plus ; le cout, s'il etait indemnise, en jours (art. 30 : 8 h / 10 x 1,3 = 1,04, au demi-jour superieur, par paie).
    let c = &compagnie("gain", true);
    let s = saisie(vec![engagement(
        "Seraphin",
        "2026-09-07",
        "2026-09-08",
        json!([{ "rubrique": "repetition", "quantite": 2, "taux": 100.0, "heures": 4, "dates": ["2026-09-07", "2026-09-08"] }]),
    )]);
    let mut s = s;
    s["mois"] = json!("2026-09");
    ecrire(c, "paies/2026-09.json", &s);
    let r = reussi(&route(c, "conseils-salarie", json!({ "salarie": "SERAPHIN" }), JOUR)).clone();
    let gain = r["conseils"].as_array().unwrap().iter().find(|x| x["id"] == "historique-gain-des-conseils").unwrap_or_else(|| panic!("{}", r));
    let t = gain["texte"].as_str().unwrap();
    assert!(t.contains("16 heures de plus") && t.contains("3,2 %") && t.contains("2,5 jours"), "{}", t);
}
