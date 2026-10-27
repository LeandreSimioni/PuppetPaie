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

/// Le conseil de la `i`-ème paie de l'apercu : (son identifiant, sa phrase courte suivie de son detail), ou rien. Un
/// conseil de saisie nomme le salarie par son seul prenom, jamais par son nom (Leandre, 21/10/2026) : la phrase courte
/// porte le prenom de la fiche, et ni elle ni le detail ne portent le nom.
fn conseil(r: &Value, i: usize) -> Option<(String, String)> {
    let c = r["paies"][i].get("conseil")?;
    let court = c["texte"].as_str().unwrap().to_string();
    let detail = c.get("detail").and_then(Value::as_str).unwrap_or("");
    for nom in ["BRIOCHE", "Brioche", "SERAPHIN", "Seraphin", "LEMERCIER", "Lemercier"] {
        assert!(!court.contains(nom) && !detail.contains(nom), "le nom dans un conseil : {} / {}", court, detail);
    }
    assert!(["Fanchon", "Valentin", "Nicole"].iter().any(|p| court.contains(p)), "le prenom manque : {}", court);
    Some((c["id"].as_str().unwrap().to_string(), format!("{} {}", court, detail)))
}

/// Le detail d'un conseil de saisie (derriere « pourquoi ? »).
fn detail(r: &Value, i: usize) -> String {
    r["paies"][i]["conseil"]["detail"].as_str().unwrap_or_default().to_string()
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
    assert!(texte.contains("30 cachets") && texte.contains("octobre 2026") && texte.contains("2 cachets ne compteront pas"), "{}", texte);
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
    assert!(texte.contains("276 heures") && texte.contains("octobre 2026") && texte.contains("270 heures"), "{}", texte);
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
    assert!(texte.contains("210 heures") && texte.contains("250"), "{}", texte);
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
    // le cachet de repetition de l'annexe 4, 94,34 EUR) compterait 8 heures de plus. La phrase courte dit le gain ; le
    // detail, les deux faces et le cout : art. 30, 8 h / 10 x 1,3 = 1,04 jour, « environ 1 jour » au demi-jour le plus
    // proche (Leandre, 21/10/2026).
    let c = &compagnie("repetition", true);
    let r = apercu(c, &saisie(vec![engagement("Brioche", "2026-10-05", "2026-10-05", repetition(100.0))]));
    let (id, texte) = conseil(&r, 0).unwrap();
    assert_eq!(id, "saisie-repetition-en-cachet");
    let court = r["paies"][0]["conseil"]["texte"].as_str().unwrap();
    assert!(court.contains("8 heures de plus"), "{}", court);
    let d = detail(&r, 0);
    assert!(d.contains("94,34") && d.contains("mieux vaut s'abstenir") && d.contains("environ 1 jour d'allocation"), "{}", d);
    assert!(!texte.contains("allocation journalière"), "{}", texte);
    nulle_part_ailleurs(&r, court);
    // Sous le minimum : rien (un conseil ne propose jamais moins que la convention).
    let r = apercu(c, &saisie(vec![engagement("Brioche", "2026-10-05", "2026-10-05", repetition(90.0))]));
    assert_eq!(id_de(&r), None, "{}", r["paies"][0]);
    // Une repetition de 11 heures : 1 heure de plus, 0,13 jour, qui s'arrondit a zero : on ne parle pas de perte.
    let onze = json!([{ "rubrique": "repetition", "quantite": 1, "taux": 100.0, "heures": 11, "dates": ["2026-10-05"] }]);
    let r = apercu(c, &saisie(vec![engagement("Brioche", "2026-10-05", "2026-10-05", onze)]));
    assert_eq!(id_de(&r).as_deref(), Some("saisie-repetition-en-cachet-minime"), "{}", r["paies"][0]);
    let d = detail(&r, 0);
    assert!(d.contains("moins d'un demi-jour") && !d.contains("perdrait"), "{}", d);
}

#[test]
fn des_heures_payees_au_dessus_du_minimum_couvriraient_plus_d_heures_pour_la_meme_somme() {
    // Annexe VIII, art. 30 (le cout : heures / 8 x 1,4) et art. 3 §1 (les 507 heures) : 4 heures a 100 EUR, 400 EUR, bien
    // au-dessus du minimum horaire de la grille de la regisseuse (17,28 EUR en 2026) : la meme somme au minimum couvre
    // 23 heures, 19 de plus.
    let c = &compagnie("heures", true);
    let ligne = |q: i64, taux: f64| json!([{ "rubrique": "heure", "quantite": q, "taux": taux, "dates": ["2026-10-05"] }]);
    let r = apercu(c, &saisie(vec![engagement("Lemercier", "2026-10-05", "2026-10-05", ligne(4, 100.0))]));
    let (id, _) = conseil(&r, 0).unwrap();
    assert_eq!(id, "saisie-heures-pour-la-meme-somme");
    let d = detail(&r, 0);
    assert!(d.contains("4 heures payées 400,00 €") && d.contains("mieux vaut s'abstenir"), "{}", d);
    nulle_part_ailleurs(&r, r["paies"][0]["conseil"]["texte"].as_str().unwrap());
    // Sous le minimum horaire : rien.
    let r = apercu(c, &saisie(vec![engagement("Lemercier", "2026-10-05", "2026-10-05", ligne(4, 5.0))]));
    assert_eq!(id_de(&r), None, "{}", r["paies"][0]);
    // Un petit ecart ne s'affiche pas : il ne sert que si le salarie est a moins de ce gain des 507 heures (le seuil :
    // le poids d'un cachet, 12 heures ; `conseils::gain_heures`). 4 heures a 50 EUR, 200 EUR : 11 heures au minimum,
    // 7 de plus.
    let r = apercu(c, &saisie(vec![engagement("Lemercier", "2026-10-05", "2026-10-05", ligne(4, 50.0))]));
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
    // Le salarie par son seul prenom (Leandre, 21/10/2026), ici comme dans son dossier.
    let t = resume["conseils"][0]["texte"].as_str().unwrap();
    assert!(t.contains("Fanchon") && !t.contains("BRIOCHE"), "{}", resume);
    assert!(textes.iter().all(|t| !t.contains("BRIOCHE")) && ouverts.contains("Fanchon"), "{:?}", textes);
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
fn un_conseil_cite_l_annexe_du_metier_du_salarie() {
    // Annexe X pour un artiste, annexe VIII pour un technicien (le metier de la fiche, `emplois`) : le dossier de Nicole,
    // regisseuse, ne cite que l'annexe VIII (art. 3 §1 : ses 507 heures ; art. 30 : huit heures par jour, coefficient
    // 1,4) ; celui de Fanchon, marionnettiste, que l'annexe X.
    let c = &compagnie("annexe-du-metier", true);
    ecrire(
        c,
        "historique/engagements.json",
        &json!({ "engagements": [
            { "salarie": "LEMERCIER", "debut": "2026-05-01", "fin": "2026-05-31", "heures": 300, "cachets": 0, "jours": 25, "brut": "5000" },
            { "salarie": "LEMERCIER", "debut": "2026-06-01", "fin": "2026-06-30", "heures": 0, "cachets": 31, "jours": 30, "brut": "5000" },
            { "salarie": "BRIOCHE", "debut": "2026-05-01", "fin": "2026-05-10", "heures": 0, "cachets": 10, "jours": 10, "brut": "1500" },
        ] }),
    );
    let conseils = |cle: &str| -> Value { reussi(&route(c, "conseils-salarie", json!({ "salarie": cle }), JOUR))["conseils"].clone() };
    let references = |cle: &str| -> Vec<String> {
        let r = conseils(cle);
        let l: Vec<String> = r.as_array().unwrap().iter().map(|x| x["reference"].as_str().unwrap().to_string()).collect();
        assert!(!l.is_empty(), "{}", r);
        l
    };
    for r in references("LEMERCIER") {
        assert!(r.contains("annexe VIII") && !r.contains("annexe X"), "{}", r);
    }
    for r in references("BRIOCHE") {
        assert!(r.contains("annexe X") && !r.contains("annexe VIII"), "{}", r);
    }
    // A la saisie, de meme : 4 heures a 100 EUR pour Nicole, dont nos contrats assurent les 507 heures ; le cout d'une
    // heure de plus se dit en annexe VIII (art. 30 : huit heures par jour, coefficient 1,4).
    let ligne = json!([{ "rubrique": "heure", "quantite": 4, "taux": 100.0, "dates": ["2026-10-05"] }]);
    let r = apercu(c, &saisie(vec![engagement("Lemercier", "2026-10-05", "2026-10-05", ligne)]));
    assert_eq!(id(&r, 0).as_deref(), Some("saisie-507-assurees"), "{}", r["paies"][0]);
    assert!(detail(&r, 0).contains("tranche de 8 heures déclarées retire 1,4 jour"), "{}", detail(&r, 0));
    let reference = r["paies"][0]["conseil"]["reference"].as_str().unwrap();
    assert!(reference.contains("annexe VIII") && !reference.contains("annexe X"), "{}", reference);
    // Le plafond de 28 cachets par mois est de l'annexe X (art. 3 §1 ; l'annexe VIII ne connait pas le cachet) : ses 31
    // cachets de juin ne valent pas a Nicole le conseil des cachets perdus, que Fanchon recevrait.
    let ids: Vec<Value> = conseils("LEMERCIER").as_array().unwrap().iter().map(|x| x["id"].clone()).collect();
    assert!(ids.contains(&json!("historique-intense")) && !ids.contains(&json!("historique-cachets-perdus")), "{:?}", ids);
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

#[test]
fn le_resume_de_l_accueil_compte_chacun_sur_la_fenetre_de_ses_droits() {
    // Leandre, 21/10/2026 (capture de l'accueil) : « il ne devrait pas dire 12 derniers mois mais compter
    // l'anniversaire ». Le resume prend, salarie par salarie, la fenetre de sa fiche (`conseils::position`) : les 12 mois
    // avant la date anniversaire notifiee (annexe X, art. 9 §1 c), sinon les 12 derniers mois (art. 3 §1).
    let c = &compagnie("resume-anniversaire", true);
    let e = |salarie: &str, debut: &str, fin: &str, heures: i64, cachets: i64| {
        json!({ "salarie": salarie, "debut": debut, "fin": fin, "heures": heures, "cachets": cachets, "jours": cachets.max(1), "brut": "1000" })
    };
    ecrire(
        c,
        "historique/engagements.json",
        &json!({ "engagements": [
            // Fanchon : 48 cachets (576 heures) en septembre et jusqu'au 20 octobre 2025, avant les 12 derniers mois
            // (depuis le 21/10/2025).
            e("BRIOCHE", "2025-09-02", "2025-09-26", 0, 25),
            e("BRIOCHE", "2025-09-27", "2025-09-30", 0, 3),
            e("BRIOCHE", "2025-10-01", "2025-10-20", 0, 20),
            // Nicole, technicienne : 520 heures en septembre 2025.
            e("LEMERCIER", "2025-09-01", "2025-09-30", 520, 0),
            // Valentin : 45 cachets en mai et juin 2026, dans les 12 derniers mois.
            e("SERAPHIN", "2026-05-01", "2026-05-25", 0, 25),
            e("SERAPHIN", "2026-06-01", "2026-06-20", 0, 20),
        ] }),
    );
    let resume = |c: &Path| -> Vec<String> {
        let r = reussi(&route(c, "conseils-resume", json!({}), JOUR)).clone();
        r["conseils"].as_array().unwrap().iter().map(|x| x["texte"].as_str().unwrap().to_string()).collect()
    };
    let reference = |c: &Path| -> String {
        reussi(&route(c, "conseils-resume", json!({}), JOUR))["conseils"][0]["reference"].as_str().unwrap().to_string()
    };
    // Sans date anniversaire, Fanchon et Nicole n'ont rien sur les 12 derniers mois : seul Valentin.
    assert_eq!(
        resume(c),
        ["Vos contrats ont fait compter au moins 507 heures à Valentin sur les 12 derniers mois : son droit au chômage est ouvert grâce à vous."]
    );
    // Leur fiche porte la date anniversaire du 01/09/2026 : les 12 mois qui la precedent (du 01/09/2025 au
    // 01/09/2026) comptent ses 48 cachets pour Fanchon, 520 heures pour Nicole. La liste mele les deux fenetres, et la phrase les dit l'une et l'autre.
    crate::outils::retoucher_fiches(
        c,
        &[("BRIOCHE", json!({ "anniversaire_droits": "2026-09-01" })), ("LEMERCIER", json!({ "anniversaire_droits": "2026-09-01" }))],
    );
    assert_eq!(
        resume(c),
        ["Vos contrats ont fait compter au moins 507 heures à Fanchon et Nicole sur les 12 mois avant leur date anniversaire, et à Valentin sur les 12 derniers mois : leurs droits au chômage sont ouverts grâce à vous."]
    );
    // Fanchon et Valentin sont artistes (annexe X), Nicole technicienne (annexe VIII) : la reference cite les deux annexes,
    // chacune pour les 507 heures (art. 3 §1) et la date anniversaire (art. 9 §1 c).
    let r = reference(c);
    assert!(r.contains("annexe X") && r.contains("annexe VIII"), "{}", r);
    assert!(r.matches("art. 3 §1").count() == 2 && r.matches("art. 9 §1 c)").count() == 2, "{}", r);
    // La fiche de Fanchon dit la meme chose que l'accueil.
    let fiche = reussi(&route(c, "conseils-salarie", json!({ "salarie": "BRIOCHE" }), JOUR)).clone();
    assert!(fiche["conseils"].as_array().unwrap().iter().any(|x| x["id"] == "historique-droits-ouverts"), "{}", fiche);
    // L'inverse : la date anniversaire de Valentin, le 01/04/2026, laisse ses contrats de mai et juin hors de sa
    // fenetre, bien qu'ils passent 507 heures sur les 12 derniers mois. Et Nicole n'a plus la sienne : ses heures de
    // septembre 2025 sortent des 12 derniers mois.
    crate::outils::retoucher_fiches(
        c,
        &[("SERAPHIN", json!({ "anniversaire_droits": "2026-04-01" })), ("LEMERCIER", json!({ "anniversaire_droits": null }))],
    );
    assert_eq!(
        resume(c),
        ["Vos contrats ont fait compter au moins 507 heures à Fanchon sur les 12 mois avant sa date anniversaire : son droit au chômage est ouvert grâce à vous."]
    );
    // Fanchon seule, artiste : l'annexe X seule.
    let r = reference(c);
    assert!(r.contains("annexe X") && !r.contains("annexe VIII"), "{}", r);
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
    let mut jour = socle::date::Jour::lire("2026-01-01").unwrap();
    for _ in 0..n {
        let r = reussi(&route(c, "conseil-du-jour", json!({}), &jour.iso())).clone();
        assert!(vus.insert(r["conseil"]["id"].as_str().unwrap().to_string()), "repetition avant le tour : {}", r);
        jour = jour.plus_jours(1).unwrap();
    }
    let autre = reussi(&route(c, "conseil-du-jour", json!({ "decalage": 1 }), "2026-01-01")).clone();
    assert_ne!(autre["conseil"]["id"], premier["conseil"]["id"]);
    // Les deux signalements que Leandre a tranches le 21/10/2026 (« 5 jours ouvres », CSS R. 133-14 II) sont du
    // nombre ; le conseil retire (le droit Conges Spectacles « des le 1er jour ») n.y est plus.
    assert!(vus.contains("signalement-arret-cinq-jours") && vus.contains("signalement-fin-contrat-cinq-jours"));
    assert!(vus.contains("conges-spectacles-ce-que-finance-la-cotisation") && !vus.contains("conges-spectacles-droit-ouvre-1er-jour"));
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
    // plus ; le cout, un mois indemnise, en jours (art. 30 : 16 h / 10 x 1,3 = 2,08), au demi-jour le plus proche : 2.
    let c = &compagnie("gain", true);
    let s = saisie(vec![engagement(
        "Seraphin",
        "2026-09-07",
        "2026-09-08",
        json!([{ "rubrique": "repetition", "quantite": 2, "taux": 100.0, "heures": 4, "dates": ["2026-09-07", "2026-09-08"] }]),
    )]);
    let mut s = s;
    s["mois"] = json!("2026-09");
    crate::outils::poser_saisie(c, "2026-09", &s);
    let r = reussi(&route(c, "conseils-salarie", json!({ "salarie": "SERAPHIN" }), JOUR)).clone();
    let gain = r["conseils"].as_array().unwrap().iter().find(|x| x["id"] == "historique-gain-des-conseils").unwrap_or_else(|| panic!("{}", r));
    let t = gain["texte"].as_str().unwrap();
    assert!(t.contains("16 heures de plus") && t.contains("3,2 %") && t.contains("environ 2 jours") && t.contains("Valentin"), "{}", t);
}
