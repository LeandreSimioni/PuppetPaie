//! L'ENGAGEMENT, DE LA SAISIE AUX DOCUMENTS (`engagement::stock`, `engagement::etats`). Les contrats :
//! - un engagement est un objet, dans le fichier du mois ou il commence (`engagements/AAAA-MM.json`) ; un mois d'apres
//!   n'y ecrit que ce qu'il change, et le montre ;
//! - « Valider l'engagement » ecrit une paie par mois qu'il couvre, avec son empreinte ; rien ne part d'un engagement en
//!   saisie, et ce qui part lit la paie ecrite, sans la recalculer ;
//! - a l'ouverture d'un mois, une paie validee dont l'empreinte ne tient plus se recalcule et dit pourquoi ; sans chiffre
//!   qui change, rien ne se dit ; des documents partis ne bougent plus ;
//! - « Modifier… » d'un engagement dont des documents sont partis pose la question du moteur, et rien ne bouge sans la
//!   reponse.
//!
//! Sur la config et le journal de `fictive-3090-petite`, des engagements saisis par le test, au 05/11/2026.

use crate::outils::{compagnie, dossier_de_test, ecrire, lire, refus, reussi, route_en_processus};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const JOUR: &str = "2026-11-05";
const SPECTACLE: &str = "256Z00920011";

/// La config et le journal de la petite fictive, sans engagement ; le taux de PAS de Brioche transmis en octobre 2026.
fn compagnie(nom: &str) -> PathBuf {
    let r = compagnie::config_et_mois("fictive-3090-petite", &[], &dossier_de_test("engagements", nom).join("compagnie"));
    let mut pas = lire(&r, "config/pas.json");
    let b = &mut pas["salaries"]["BRIOCHE"];
    b["periode"] = json!("202610");
    b["mis_a_disposition"] = json!("20261008120000");
    b["releve_le"] = json!("2026-10-08");
    ecrire(&r, "config/pas.json", &pas);
    r
}

fn route(r: &Path, nom: &str, charge: Value) -> Value {
    route_en_processus(r, nom, charge, JOUR)
}

/// Un contrat d'usage de `salarie`, du `debut` a la `fin`, un cachet par jour de `dates` ; `planning` : ses jours des
/// mois d'apres.
fn contrat(salarie: &str, debut: &str, fin: &str, dates: &[&str], planning: &[&str]) -> Value {
    json!({"salarie": salarie, "spectacle": SPECTACLE, "debut": debut, "fin": fin,
           "lignes": [{"rubrique": "cachet", "quantite": dates.len(), "taux": 155.0, "lieu": "Salle fictive Boris-Vian, Clermont",
                       "dates": dates, "planning": planning}]})
}

/// La saisie de `mois` que l'ecran enregistre.
fn enregistrer(r: &Path, mois: &str, engagements: Value) -> Value {
    let fin = if mois == "2026-11" { "2026-11-30" } else { "2026-12-31" };
    route(r, "enregistrer-saisie", json!({"mois": mois, "saisie": {"mois": mois, "taux_cachet": 150.0, "date_paiement": fin, "engagements": engagements}}))
}

fn valider(r: &Path, mois: &str, salarie: &str, debut: &str, valide: bool) -> Value {
    route(r, "valider-engagement", json!({"mois": mois, "salarie": salarie, "debut": debut, "valide": valide}))
}

/// Brioche, du 28/11 au 02/12 : trois cachets en novembre, deux en decembre.
fn a_cheval(r: &Path) {
    reussi(&enregistrer(r, "2026-11", json!([contrat("Brioche", "2026-11-28", "2026-12-02", &["2026-11-28", "2026-11-29", "2026-11-30"], &["2026-12-01", "2026-12-02"])])));
    reussi(&enregistrer(r, "2026-12", json!([contrat("Brioche", "2026-11-28", "2026-12-02", &["2026-12-01", "2026-12-02"], &[])])));
}

fn fichier(r: &Path, mois: &str) -> Value {
    lire(r, &format!("engagements/{}.json", mois))
}

fn contexte(r: &Path) -> droit::contexte::Contexte {
    droit::contexte::Contexte::nouveau(r, Some(socle::date::Jour::lire(JOUR).unwrap())).unwrap()
}

/// La paie ecrite de Brioche pour `mois`.
fn paie_ecrite(r: &Path, mois: &str) -> Value {
    fichier(r, "2026-11")["engagements"][0]["valide"]["paies"][mois].clone()
}

/// Ouvrir le mois, comme l'ecran (la route `saisie`).
fn ouvrir(r: &Path, mois: &str) {
    reussi(&route(r, "saisie", json!({"mois": mois})));
}

/// Le taux de PAS de Brioche change : un compte rendu de la DGFiP est arrive.
fn taux_de_pas(r: &Path, taux: &str) {
    let mut pas = lire(r, "config/pas.json");
    pas["salaries"]["BRIOCHE"]["taux"] = json!(taux);
    ecrire(r, "config/pas.json", &pas);
}

/// Les documents de Brioche pour `mois` sont partis (le journal le dit, comme l'envoi l'ecrit).
fn documents_partis(r: &Path, mois: &str) {
    let mut j = lire(r, "journal/depots.json");
    j.as_array_mut().unwrap().push(json!({"quoi": "documents", "quand": "2026-12-03T10:00:00", "mois": mois, "mode": "reel",
        "accepte": true, "detail": {"salaries": [{"cle": "BRIOCHE", "nom": "Fanchon BRIOCHE", "pieces": []}]}}));
    ecrire(r, "journal/depots.json", &j);
}

#[test]
fn un_engagement_sur_deux_mois_est_un_seul_objet_dans_le_fichier_de_son_premier_mois() {
    let r = compagnie("a-cheval");
    a_cheval(&r);
    let nov = fichier(&r, "2026-11");
    assert_eq!(nov["engagements"].as_array().unwrap().len(), 1);
    let e = &nov["engagements"][0];
    assert_eq!(e["lignes"][0]["dates"], json!(["2026-11-28", "2026-11-29", "2026-11-30"]));
    assert_eq!(e["mois_suivants"]["2026-12"]["lignes"][0]["dates"], json!(["2026-12-01", "2026-12-02"]));
    assert!(!r.join("engagements/2026-12.json").exists() || fichier(&r, "2026-12")["engagements"] == json!([]));
    // Decembre le montre, avec ses jours ; novembre n'a pas bouge.
    let dec = reussi(&route(&r, "saisie", json!({"mois": "2026-12"})))["saisie"].clone();
    assert_eq!(dec["engagements"].as_array().unwrap().len(), 1);
    assert_eq!(dec["engagements"][0]["lignes"][0]["dates"], json!(["2026-12-01", "2026-12-02"]));
    assert_eq!(dec["engagements"][0]["debut"], json!("2026-11-28"));
    let vue_nov = crate::outils::saisie(&r, "2026-11");
    assert_eq!(vue_nov["engagements"][0]["lignes"][0]["dates"], json!(["2026-11-28", "2026-11-29", "2026-11-30"]));
}

#[test]
fn des_bornes_qui_laissent_un_mois_paye_hors_du_contrat_se_refusent_sans_rien_ecrire() {
    let r = compagnie("bornes");
    a_cheval(&r);
    let avant = std::fs::read(r.join("engagements/2026-11.json")).unwrap();
    let r2 = enregistrer(&r, "2026-11", json!([contrat("Brioche", "2026-11-28", "2026-11-30", &["2026-11-28", "2026-11-29", "2026-11-30"], &[])]));
    let (_, message) = refus(&r2);
    assert!(message.contains("décembre 2026") || message.contains("decembre 2026"), "{}", message);
    assert_eq!(std::fs::read(r.join("engagements/2026-11.json")).unwrap(), avant);
}

#[test]
fn valider_ecrit_une_paie_par_mois_couvert_avec_son_empreinte() {
    let r = compagnie("valider");
    a_cheval(&r);
    let v = reussi(&valider(&r, "2026-11", "BRIOCHE", "2026-11-28", true)).clone();
    assert!(v["valide"].is_string(), "{}", v);
    let valide = &fichier(&r, "2026-11")["engagements"][0]["valide"];
    let mois: Vec<&String> = valide["paies"].as_object().unwrap().keys().collect();
    assert_eq!(mois, ["2026-11", "2026-12"]);
    for m in ["2026-11", "2026-12"] {
        let p = &valide["paies"][m];
        assert!(p["paie"]["brut"].is_string() || p["paie"]["brut"].is_number(), "{} : {}", m, p);
        assert_eq!(p["empreinte"]["entrees"]["moteur"], json!(engagement::empreinte::MOTEUR));
        assert!(p["empreinte"]["entrees"][format!("saisie {}", m)].is_string(), "la saisie du mois est une entree : {}", p["empreinte"]);
    }
    // Decembre le dit valide, et ce que l'ecran montre de lui est la paie ecrite.
    let etat = reussi(&route(&r, "etat-par-engagement", json!({"mois": "2026-12"}))).clone();
    assert_eq!(etat["engagements"][0]["valide"], v["valide"]);
}

#[test]
fn un_engagement_valide_ne_change_pas_par_la_saisie() {
    let r = compagnie("valide-fige");
    a_cheval(&r);
    reussi(&valider(&r, "2026-11", "BRIOCHE", "2026-11-28", true));
    let avant = fichier(&r, "2026-11")["engagements"][0].clone();
    let mut autre = contrat("Brioche", "2026-11-28", "2026-12-02", &["2026-11-28"], &["2026-12-01", "2026-12-02"]);
    autre["lignes"][0]["taux"] = json!(300.0);
    reussi(&enregistrer(&r, "2026-11", json!([autre])));
    assert_eq!(fichier(&r, "2026-11")["engagements"][0], avant);
}

#[test]
fn rien_ne_part_d_un_engagement_en_saisie() {
    let r = compagnie("en-saisie");
    a_cheval(&r);
    let saisie = crate::outils::saisie(&r, "2026-11");
    let rendu = route(&r, "produire", json!({"mois": "2026-11", "saisie": saisie}));
    assert_eq!(rendu["classe"], json!("EngagementEnSaisie"), "{}", rendu);
    assert!(!r.join("sorties/2026-11").exists(), "rien n'est ecrit");
    let ctx = contexte(&r);
    assert!(engagement::etats::paies_du_mois(&ctx, "2026-11").is_err());
    // Ce qui peut partir : rien, tant qu'il n'est pas valide.
    let (_, paies) = engagement::etats::validees_du_mois(&ctx, "2026-11").unwrap().unwrap();
    assert!(paies.is_empty());
}

#[test]
fn ce_qui_part_lit_la_paie_validee_telle_qu_elle_est_ecrite() {
    let r = compagnie("lue");
    a_cheval(&r);
    reussi(&valider(&r, "2026-11", "BRIOCHE", "2026-11-28", true));
    // La paie ecrite fait foi : un net retouche dans le fichier est celui que lit ce qui fabrique un document.
    let mut f = fichier(&r, "2026-11");
    f["engagements"][0]["valide"]["paies"]["2026-11"]["paie"]["net_a_payer"] = json!("123.45");
    ecrire(&r, "engagements/2026-11.json", &f);
    let (_, paies) = engagement::etats::validees_du_mois(&contexte(&r), "2026-11").unwrap().unwrap();
    assert_eq!(paies[0].net_a_payer.to_string(), "123.45");
    // Son prelevement a la source aussi : celui qui a ete ecrit.
    assert_eq!(paies[0].pas.as_ref().map(|v| v.taux.clone()), Some(json!("4.30")));
}

#[test]
fn un_taux_de_pas_recu_recalcule_la_paie_a_l_ouverture_et_le_dit() {
    let r = compagnie("pas-recu");
    a_cheval(&r);
    reussi(&valider(&r, "2026-11", "BRIOCHE", "2026-11-28", true));
    let avant = paie_ecrite(&r, "2026-11");
    taux_de_pas(&r, "7.50");
    ouvrir(&r, "2026-11");
    let apres = paie_ecrite(&r, "2026-11");
    assert_eq!(apres["pas"]["taux"], json!("7.50"));
    assert_eq!(apres["paie"]["brut"], avant["paie"]["brut"], "le brut ne change pas");
    assert_eq!(apres["recalcul"]["raison"], json!(engagement::etats::TAUX_DE_PAS));
    let etat = reussi(&route(&r, "etat-par-engagement", json!({"mois": "2026-11"}))).clone();
    let phrase = etat["engagements"][0]["recalcul"].as_str().unwrap().to_string();
    assert!(phrase.starts_with("Recalculé le ") && phrase.ends_with(": taux de PAS reçu."), "{}", phrase);
    // Decembre n'est pas ouvert : il ne s'est pas recalcule.
    assert!(paie_ecrite(&r, "2026-12").get("recalcul").is_none());
}

/// La raison d'un recalcul dit ce qui a change : la configuration de la compagnie, la saisie qu'il lit (un autre
/// engagement), le code qui calcule. Les deux derniers se posent sur la paie ecrite : une empreinte d'un autre moteur, ou
/// d'une autre saisie du mois, et un brut qui n'est plus celui du calcul.
#[test]
fn la_raison_d_un_recalcul_dit_ce_qui_a_change() {
    let r = compagnie("raisons");
    a_cheval(&r);
    reussi(&valider(&r, "2026-11", "BRIOCHE", "2026-11-28", true));
    let avant = paie_ecrite(&r, "2026-11");
    let mut b = lire(&r, "config/baremes/2026.json");
    b["taux_atmp"]["valeur"] = json!(3.5);
    ecrire(&r, "config/baremes/2026.json", &b);
    ouvrir(&r, "2026-11");
    let apres = paie_ecrite(&r, "2026-11");
    assert_ne!(apres["paie"]["cout_employeur"], avant["paie"]["cout_employeur"]);
    assert_eq!(apres["recalcul"]["raison"], json!(engagement::etats::CONFIGURATION));
    for (entree, raison) in [("saisie 2026-11", engagement::etats::AUTRE_ENGAGEMENT), ("moteur", engagement::etats::CALCUL_CORRIGE)] {
        let mut f = fichier(&r, "2026-11");
        let p = &mut f["engagements"][0]["valide"]["paies"]["2026-11"];
        p["empreinte"]["entrees"][entree] = json!("autre");
        p["paie"]["brut"] = json!("1.00");
        ecrire(&r, "engagements/2026-11.json", &f);
        ouvrir(&r, "2026-11");
        assert_eq!(paie_ecrite(&r, "2026-11")["recalcul"]["raison"], json!(raison), "{}", entree);
    }
}

#[test]
fn un_changement_sans_chiffre_ne_laisse_pas_de_trace() {
    let r = compagnie("sans-chiffre");
    a_cheval(&r);
    reussi(&valider(&r, "2026-11", "BRIOCHE", "2026-11-28", true));
    let avant = paie_ecrite(&r, "2026-11");
    let mut s = lire(&r, "config/salaries.json");
    s["BRIOCHE"]["email"] = json!("fanchon.nouvelle@exemple.invalid");
    ecrire(&r, "config/salaries.json", &s);
    ouvrir(&r, "2026-11");
    let apres = paie_ecrite(&r, "2026-11");
    assert_eq!(apres["paie"], avant["paie"]);
    assert!(apres.get("recalcul").is_none(), "{}", apres);
    assert_ne!(apres["empreinte"]["entrees"]["fiches"], avant["empreinte"]["entrees"]["fiches"], "l'empreinte est celle d'aujourd'hui");
}

#[test]
fn des_documents_partis_ne_bougent_plus() {
    let r = compagnie("partis");
    a_cheval(&r);
    reussi(&valider(&r, "2026-11", "BRIOCHE", "2026-11-28", true));
    documents_partis(&r, "2026-11");
    let avant = paie_ecrite(&r, "2026-11");
    taux_de_pas(&r, "7.50");
    ouvrir(&r, "2026-11");
    assert_eq!(paie_ecrite(&r, "2026-11"), avant);
    // Decembre, dont rien n'est parti, se recalcule.
    ouvrir(&r, "2026-12");
    assert_eq!(paie_ecrite(&r, "2026-12")["pas"]["taux"], json!("7.50"));
}

#[test]
fn modifier_un_engagement_dont_les_documents_sont_partis_pose_la_question_du_moteur() {
    let r = compagnie("question");
    a_cheval(&r);
    reussi(&valider(&r, "2026-11", "BRIOCHE", "2026-11-28", true));
    documents_partis(&r, "2026-11");
    let avant = std::fs::read(r.join("engagements/2026-11.json")).unwrap();
    let rendu = valider(&r, "2026-11", "BRIOCHE", "2026-11-28", false);
    assert_eq!(rendu["classe"], json!(paie::routes_ecritures_g4::CLASSE_DOCUMENTS_PARTIS));
    assert_eq!(rendu["erreur"], json!(engagement::etats::QUESTION_DOCUMENTS_PARTIS));
    assert_eq!(std::fs::read(r.join("engagements/2026-11.json")).unwrap(), avant, "rien ne bouge sans la reponse");
    // Depuis decembre aussi : la question porte sur tous les mois de l'engagement.
    let depuis_decembre = valider(&r, "2026-12", "BRIOCHE", "2026-11-28", false);
    assert_eq!(depuis_decembre["classe"], json!(paie::routes_ecritures_g4::CLASSE_DOCUMENTS_PARTIS), "{}", depuis_decembre);
    assert_eq!(std::fs::read(r.join("engagements/2026-11.json")).unwrap(), avant);
    // Avec la reponse : en saisie, et la paie partie se garde.
    let partie = paie_ecrite(&r, "2026-11");
    reussi(&route(&r, "valider-engagement", json!({"mois": "2026-11", "salarie": "BRIOCHE", "debut": "2026-11-28", "valide": false, "confirme": true})));
    let e = fichier(&r, "2026-11")["engagements"][0].clone();
    assert!(e.get("valide").is_none());
    assert_eq!(e["figees"]["2026-11"], partie);
    // Revalide sans rien changer a novembre : la paie partie se reprend telle quelle.
    reussi(&valider(&r, "2026-11", "BRIOCHE", "2026-11-28", true));
    assert_eq!(paie_ecrite(&r, "2026-11"), partie);
}

#[test]
fn modifier_un_engagement_dont_rien_n_est_parti_le_remet_en_saisie() {
    let r = compagnie("modifier");
    a_cheval(&r);
    reussi(&valider(&r, "2026-11", "BRIOCHE", "2026-11-28", true));
    reussi(&valider(&r, "2026-12", "BRIOCHE", "2026-11-28", false));
    let e = fichier(&r, "2026-11")["engagements"][0].clone();
    assert!(e.get("valide").is_none() && e.get("figees").is_none(), "{}", e);
    let etat = reussi(&route(&r, "etat-par-engagement", json!({"mois": "2026-11"}))).clone();
    assert_eq!(etat["engagements"][0]["valide"], Value::Null);
}

/// Deux contrats de Brioche le 24/11 (les JUMEAUX) : `valider-engagement` vise l'un par son numero, ou par son rang parmi
/// ceux du jour, jamais les deux ; sans dire lequel, il refuse sans rien ecrire.
#[test]
fn valider_vise_ce_contrat_et_pas_son_jumeau_du_meme_jour() {
    let r = compagnie("jumeaux");
    let mut a = contrat("Brioche", "2026-11-24", "2026-11-25", &["2026-11-24", "2026-11-25"], &[]);
    let mut b = contrat("Brioche", "2026-11-24", "2026-11-24", &["2026-11-24"], &[]);
    a["numero"] = json!("BR1.26.11");
    b["numero"] = json!("BR1.26.12");
    reussi(&enregistrer(&r, "2026-11", json!([a, b])));
    let avant = std::fs::read(r.join("engagements/2026-11.json")).unwrap();
    let (_, message) = refus(&valider(&r, "2026-11", "BRIOCHE", "2026-11-24", true));
    assert!(message.contains("BR1.26.11") && message.contains("BR1.26.12"), "{}", message);
    assert_eq!(std::fs::read(r.join("engagements/2026-11.json")).unwrap(), avant);
    reussi(&route(&r, "valider-engagement", json!({"mois": "2026-11", "salarie": "BRIOCHE", "debut": "2026-11-24", "numero": "BR1.26.12"})));
    let e = fichier(&r, "2026-11")["engagements"].clone();
    assert!(e[0].get("valide").is_none() && e[1]["valide"]["le"].is_string(), "{}", e);
    let etat = reussi(&route(&r, "etat-par-engagement", json!({"mois": "2026-11"}))).clone();
    assert_eq!(etat["engagements"][0]["valide"], Value::Null);
    assert!(etat["engagements"][1]["valide"].is_string());
    // Sans numeros, par le rang.
    let r = compagnie("jumeaux-rang");
    let a = contrat("Brioche", "2026-11-24", "2026-11-25", &["2026-11-24", "2026-11-25"], &[]);
    let b = contrat("Brioche", "2026-11-24", "2026-11-24", &["2026-11-24"], &[]);
    reussi(&enregistrer(&r, "2026-11", json!([a, b])));
    refus(&valider(&r, "2026-11", "BRIOCHE", "2026-11-24", true));
    reussi(&route(&r, "valider-engagement", json!({"mois": "2026-11", "salarie": "BRIOCHE", "debut": "2026-11-24", "rang": 1})));
    let e = fichier(&r, "2026-11")["engagements"].clone();
    assert!(e[0].get("valide").is_none() && e[1]["valide"]["le"].is_string(), "{}", e);
}

#[test]
fn une_charge_incomplete_refuse_sans_rien_ecrire() {
    let r = compagnie("incomplet");
    a_cheval(&r);
    let avant = std::fs::read(r.join("engagements/2026-11.json")).unwrap();
    for nom in ["valider-engagement", "contrat-recu", "casc-regularise"] {
        refus(&route(&r, nom, json!({})));
    }
    refus(&route(&r, "valider-engagement", json!({"mois": "2099-01", "salarie": "personne", "debut": "2099-01-01"})));
    assert_eq!(std::fs::read(r.join("engagements/2026-11.json")).unwrap(), avant);
    assert!(!r.join("config/casc-svp.json").exists());
}
