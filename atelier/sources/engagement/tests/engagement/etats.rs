//! L'ENGAGEMENT, DE LA SAISIE A CE QUI PART (`engagement::stock`, `engagement::etats`) :
//! - un engagement est un objet, dans le fichier du mois ou il commence (`engagements/AAAA-MM.json`) ; un mois d'apres
//!   n'y ecrit que ce qu'il change, et le montre ; un mois paye ne sort pas du contrat ;
//! - « Valider l'engagement » ecrit une paie par mois qu'il couvre, avec son empreinte ; rien ne part d'un engagement en
//!   saisie, et ce qui part lit la paie ecrite ;
//! - a la lecture d'un mois, une paie validee dont l'empreinte ne tient plus se recalcule et dit pourquoi ; sans chiffre
//!   qui change, rien ne se dit ; des documents partis ne bougent plus ;
//! - « Modifier… » d'un engagement dont des documents sont partis pose la question du moteur ; la paie partie se garde.
//!
//! Sur la config et le journal de `fictive-3090-petite`, des engagements saisis par le test, au 05/11/2026.

use std::path::{Path, PathBuf};

use droit::contexte::Contexte;
use engagement::etats::{self, Etat};
use engagement::stock;
use serde_json::{json, Value};

use crate::outils::{config_et_journal, dossier_de_test, ecrire, lire};

const JOUR: &str = "2026-11-05";
const SPECTACLE: &str = "256Z00920011";
/// La cle de la fiche du spectacle `SPECTACLE`.
const FICHE_DU_SPECTACLE: &str = "ubu-roi";

/// La config et le journal de la petite fictive, sans engagement ; le taux de PAS de Brioche transmis en octobre 2026.
fn compagnie(nom: &str) -> PathBuf {
    let r = config_et_journal("fictive-3090-petite", &dossier_de_test("etats", nom).join("compagnie"));
    let mut pas = lire(&r, "config/pas.json");
    let b = &mut pas["salaries"]["BRIOCHE"];
    b["periode"] = json!("202610");
    b["mis_a_disposition"] = json!("20261008120000");
    b["releve_le"] = json!("2026-10-08");
    ecrire(&r, "config/pas.json", &pas);
    r
}

/// Le moteur tel qu'il lit la compagnie maintenant (une fiche ou un taux changes se relisent).
fn ctx(r: &Path) -> Contexte {
    Contexte::nouveau(r, Some(socle::date::Jour::lire(JOUR).unwrap())).unwrap()
}

/// Un contrat d'usage de `salarie`, du `debut` a la `fin`, un cachet par jour de `dates` ; `planning` : ses jours des
/// mois d'apres.
fn contrat(salarie: &str, debut: &str, fin: &str, dates: &[&str], planning: &[&str]) -> Value {
    json!({"salarie": salarie, "spectacle": SPECTACLE, "debut": debut, "fin": fin,
           "lignes": [{"rubrique": "cachet", "quantite": dates.len(), "taux": 155.0, "lieu": "Salle fictive Boris-Vian, Clermont",
                       "dates": dates, "planning": planning}]})
}

/// La saisie de `mois` que l'ecran montre, rendue a ses engagements.
fn enregistrer(r: &Path, mois: &str, engagements: Value) -> Result<Vec<PathBuf>, String> {
    let fin = if mois == "2026-11" { "2026-11-30" } else { "2026-12-31" };
    stock::ecrire_vue_du_mois(&ctx(r), mois, &json!({"mois": mois, "taux_cachet": 150.0, "date_paiement": fin, "engagements": engagements}))
}

/// Brioche, du 28/11 au 02/12 : trois cachets en novembre, deux en decembre.
fn a_cheval(r: &Path) {
    enregistrer(r, "2026-11", json!([contrat("Brioche", "2026-11-28", "2026-12-02", &["2026-11-28", "2026-11-29", "2026-11-30"], &["2026-12-01", "2026-12-02"])])).unwrap();
    enregistrer(r, "2026-12", json!([contrat("Brioche", "2026-11-28", "2026-12-02", &["2026-12-01", "2026-12-02"], &[])])).unwrap();
}

/// « Valider l'engagement » de Brioche, le seul de l'ecran de `mois`.
fn valider(r: &Path, mois: &str) -> String {
    etats::valider(&ctx(r), mois, 0).unwrap().le().to_string()
}

fn fichier(r: &Path, mois: &str) -> Value {
    lire(r, &format!("engagements/{}.json", mois))
}

/// La paie ecrite de Brioche pour `mois`.
fn paie_ecrite(r: &Path, mois: &str) -> Value {
    fichier(r, "2026-11")["engagements"][0]["valide"]["paies"][mois].clone()
}

/// Ce qui lit l'engagement valide de `mois` (l'ecran, un export, un document) : `etats::du_mois`.
fn lire_le_mois(r: &Path, mois: &str) -> Vec<Etat> {
    etats::du_mois(&ctx(r), mois).unwrap()
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
    // Decembre dit ce que le planning de novembre annoncait : il n'ecrit rien.
    assert!(e.get("mois_suivants").is_none(), "{}", e);
    assert!(!r.join("engagements/2026-12.json").exists() || fichier(&r, "2026-12")["engagements"] == json!([]));
    // Decembre le montre, avec ses jours ; novembre n'a pas bouge.
    let dec = stock::vue_du_mois(&r, "2026-12").unwrap().unwrap();
    assert_eq!(dec["engagements"].as_array().unwrap().len(), 1);
    assert_eq!(dec["engagements"][0]["lignes"][0]["dates"], json!(["2026-12-01", "2026-12-02"]));
    assert_eq!(dec["engagements"][0]["debut"], json!("2026-11-28"));
    let vue_nov = stock::vue_du_mois(&r, "2026-11").unwrap().unwrap();
    assert_eq!(vue_nov["engagements"][0]["lignes"][0]["dates"], json!(["2026-11-28", "2026-11-29", "2026-11-30"]));
    // Le meme spectacle sous un autre nom (la cle de sa fiche, comme l'ecran la rend) n'est pas un changement.
    let mut decembre = contrat("Brioche", "2026-11-28", "2026-12-02", &["2026-12-01", "2026-12-02"], &[]);
    decembre["spectacle"] = json!(FICHE_DU_SPECTACLE);
    enregistrer(&r, "2026-12", json!([decembre])).unwrap();
    let e = &fichier(&r, "2026-11")["engagements"][0];
    assert!(e.get("mois_suivants").is_none(), "{}", e);
}

#[test]
fn des_bornes_qui_laissent_un_mois_paye_hors_du_contrat_se_refusent_sans_rien_ecrire() {
    let r = compagnie("bornes");
    a_cheval(&r);
    // Decembre a ses propres jours payes, ecrits dans decembre (le taux n'est pas celui du planning).
    let mut decembre = contrat("Brioche", "2026-11-28", "2026-12-02", &["2026-12-01", "2026-12-02"], &[]);
    decembre["lignes"][0]["taux"] = json!(160.0);
    enregistrer(&r, "2026-12", json!([decembre])).unwrap();
    let avant = std::fs::read(r.join("engagements/2026-11.json")).unwrap();
    let message = enregistrer(&r, "2026-11", json!([contrat("Brioche", "2026-11-28", "2026-11-30", &["2026-11-28", "2026-11-29", "2026-11-30"], &[])])).unwrap_err();
    assert!(message.contains("décembre 2026") || message.contains("decembre 2026"), "{}", message);
    assert_eq!(std::fs::read(r.join("engagements/2026-11.json")).unwrap(), avant);
}

#[test]
fn valider_ecrit_une_paie_par_mois_couvert_avec_son_empreinte() {
    let r = compagnie("valider");
    a_cheval(&r);
    let le = valider(&r, "2026-11");
    let valide = &fichier(&r, "2026-11")["engagements"][0]["valide"];
    let mois: Vec<&String> = valide["paies"].as_object().unwrap().keys().collect();
    assert_eq!(mois, ["2026-11", "2026-12"]);
    for m in ["2026-11", "2026-12"] {
        let p = &valide["paies"][m];
        assert!(p["paie"]["brut"].is_string() || p["paie"]["brut"].is_number(), "{} : {}", m, p);
        assert_eq!(p["empreinte"]["entrees"]["moteur"], json!(engagement::empreinte::MOTEUR));
        assert!(p["empreinte"]["entrees"][format!("saisie {}", m)].is_string(), "la saisie du mois est une entree : {}", p["empreinte"]);
    }
    // Decembre le lit valide, de la meme validation.
    let dec = lire_le_mois(&r, "2026-12");
    assert_eq!(dec[0].valide().map(|v| v.le().to_string()), Some(le));
}

#[test]
fn un_engagement_valide_ne_change_pas_par_la_saisie() {
    let r = compagnie("valide-fige");
    a_cheval(&r);
    valider(&r, "2026-11");
    let avant = fichier(&r, "2026-11")["engagements"][0].clone();
    let mut autre = contrat("Brioche", "2026-11-28", "2026-12-02", &["2026-11-28"], &["2026-12-01", "2026-12-02"]);
    autre["lignes"][0]["taux"] = json!(300.0);
    enregistrer(&r, "2026-11", json!([autre])).unwrap();
    assert_eq!(fichier(&r, "2026-11")["engagements"][0], avant);
}

#[test]
fn rien_ne_part_d_un_engagement_en_saisie() {
    let r = compagnie("en-saisie");
    a_cheval(&r);
    let ctx = ctx(&r);
    assert!(etats::paies_du_mois(&ctx, "2026-11").is_err());
    // Ce qui peut partir : rien, tant qu'il n'est pas valide.
    let (_, paies) = etats::validees_du_mois(&ctx, "2026-11").unwrap().unwrap();
    assert!(paies.is_empty());
}

#[test]
fn ce_qui_part_lit_la_paie_validee_telle_qu_elle_est_ecrite() {
    let r = compagnie("lue");
    a_cheval(&r);
    valider(&r, "2026-11");
    // La paie ecrite fait foi : un net retouche dans le fichier est celui que lit ce qui fabrique un document.
    let mut f = fichier(&r, "2026-11");
    f["engagements"][0]["valide"]["paies"]["2026-11"]["paie"]["net_a_payer"] = json!("123.45");
    ecrire(&r, "engagements/2026-11.json", &f);
    let (_, paies) = etats::validees_du_mois(&ctx(&r), "2026-11").unwrap().unwrap();
    assert_eq!(paies[0].net_a_payer.to_string(), "123.45");
    // Son prelevement a la source aussi : celui qui a ete ecrit.
    assert_eq!(paies[0].pas.as_ref().map(|v| v.taux.clone()), Some(json!("4.30")));
}

#[test]
fn un_taux_de_pas_recu_recalcule_la_paie_a_sa_lecture_et_le_dit() {
    let r = compagnie("pas-recu");
    a_cheval(&r);
    valider(&r, "2026-11");
    let avant = paie_ecrite(&r, "2026-11");
    taux_de_pas(&r, "7.50");
    // Ce qui part lit la paie recalculee, sans que le mois ait ete ouvert.
    let (_, paies) = etats::validees_du_mois(&ctx(&r), "2026-11").unwrap().unwrap();
    assert_eq!(paies[0].pas.as_ref().map(|v| v.taux.clone()), Some(json!("7.50")));
    let apres = paie_ecrite(&r, "2026-11");
    assert_eq!(apres["pas"]["taux"], json!("7.50"));
    assert_eq!(apres["paie"]["brut"], avant["paie"]["brut"], "le brut ne change pas");
    assert_eq!(apres["recalcul"]["raison"], json!(etats::TAUX_DE_PAS));
    let nov = lire_le_mois(&r, "2026-11");
    let phrase = nov[0].valide().and_then(|v| v.recalcul("2026-11")).and_then(etats::phrase_du_recalcul).unwrap();
    assert!(phrase.starts_with("Recalculé le ") && phrase.ends_with(": taux de PAS reçu."), "{}", phrase);
    // Decembre n'est pas lu : il ne s'est pas recalcule.
    assert!(paie_ecrite(&r, "2026-12").get("recalcul").is_none());
}

/// La raison d'un recalcul dit ce qui a change : la configuration de la compagnie, un autre engagement que le calcul
/// lit, ce que la saisie du mois pose pour tous, le code qui calcule. Les trois derniers se posent sur la paie ecrite :
/// une empreinte d'une autre saisie du mois, ou d'un autre moteur, et un brut qui n'est plus celui du calcul.
#[test]
fn la_raison_d_un_recalcul_dit_ce_qui_a_change() {
    let r = compagnie("raisons");
    a_cheval(&r);
    valider(&r, "2026-11");
    let avant = paie_ecrite(&r, "2026-11");
    let mut b = lire(&r, "config/baremes/2026.json");
    b["taux_atmp"]["valeur"] = json!(3.5);
    ecrire(&r, "config/baremes/2026.json", &b);
    lire_le_mois(&r, "2026-11");
    let apres = paie_ecrite(&r, "2026-11");
    assert_ne!(apres["paie"]["cout_employeur"], avant["paie"]["cout_employeur"]);
    assert_eq!(apres["recalcul"]["raison"], json!(etats::CONFIGURATION));
    // La saisie d'un mois s'ecrit `<ce que le mois pose>:<ses engagements>` (`stock::empreinte_de_saisie`).
    let saisie = apres["empreinte"]["entrees"]["saisie 2026-11"].as_str().unwrap().to_string();
    let (_, engagements) = saisie.split_once(':').unwrap();
    for (entree, valeur, raison) in [
        ("saisie 2026-11", "autre:autres".to_string(), etats::AUTRE_ENGAGEMENT),
        ("saisie 2026-11", format!("autre:{}", engagements), etats::SAISIE_DU_MOIS),
        ("moteur", "autre".to_string(), etats::CALCUL_CORRIGE),
    ] {
        let mut f = fichier(&r, "2026-11");
        let p = &mut f["engagements"][0]["valide"]["paies"]["2026-11"];
        p["empreinte"]["entrees"][entree] = json!(valeur);
        p["paie"]["brut"] = json!("1.00");
        ecrire(&r, "engagements/2026-11.json", &f);
        lire_le_mois(&r, "2026-11");
        assert_eq!(paie_ecrite(&r, "2026-11")["recalcul"]["raison"], json!(raison), "{} = {}", entree, valeur);
    }
}

#[test]
fn un_changement_sans_chiffre_ne_laisse_pas_de_trace() {
    let r = compagnie("sans-chiffre");
    a_cheval(&r);
    valider(&r, "2026-11");
    let avant = paie_ecrite(&r, "2026-11");
    let mut s = lire(&r, "config/salaries.json");
    s["BRIOCHE"]["email"] = json!("fanchon.nouvelle@exemple.invalid");
    ecrire(&r, "config/salaries.json", &s);
    lire_le_mois(&r, "2026-11");
    let apres = paie_ecrite(&r, "2026-11");
    assert_eq!(apres["paie"], avant["paie"]);
    assert!(apres.get("recalcul").is_none(), "{}", apres);
    assert_ne!(apres["empreinte"]["entrees"]["fiches"], avant["empreinte"]["entrees"]["fiches"], "l'empreinte est celle d'aujourd'hui");
}

#[test]
fn des_documents_partis_ne_bougent_plus() {
    let r = compagnie("partis");
    a_cheval(&r);
    valider(&r, "2026-11");
    documents_partis(&r, "2026-11");
    let avant = paie_ecrite(&r, "2026-11");
    taux_de_pas(&r, "7.50");
    assert!(matches!(lire_le_mois(&r, "2026-11")[0], Etat::Partis(_)));
    assert_eq!(paie_ecrite(&r, "2026-11"), avant);
    // Decembre, dont rien n'est parti, se recalcule.
    lire_le_mois(&r, "2026-12");
    assert_eq!(paie_ecrite(&r, "2026-12")["pas"]["taux"], json!("7.50"));
}

#[test]
fn modifier_un_engagement_dont_les_documents_sont_partis_pose_la_question_et_garde_la_paie_partie() {
    let r = compagnie("question");
    a_cheval(&r);
    valider(&r, "2026-11");
    documents_partis(&r, "2026-11");
    // La question porte sur tous les mois de l'engagement : on la pose depuis novembre comme depuis decembre.
    for mois in ["2026-11", "2026-12"] {
        let q = etats::question_avant_de_modifier(&ctx(&r), mois, 0).unwrap();
        assert_eq!(q.as_deref(), Some(etats::QUESTION_DOCUMENTS_PARTIS), "{}", mois);
    }
    // Modifie : en saisie, et la paie partie se garde.
    let partie = paie_ecrite(&r, "2026-11");
    etats::modifier(&ctx(&r), "2026-11", 0).unwrap();
    let e = fichier(&r, "2026-11")["engagements"][0].clone();
    assert!(e.get("valide").is_none());
    assert_eq!(e["figees"]["2026-11"], partie);
    // Revalide sans rien changer a novembre : la paie partie se reprend telle quelle.
    valider(&r, "2026-11");
    assert_eq!(paie_ecrite(&r, "2026-11"), partie);
}

#[test]
fn modifier_un_engagement_dont_rien_n_est_parti_le_remet_en_saisie() {
    let r = compagnie("modifier");
    a_cheval(&r);
    valider(&r, "2026-11");
    assert_eq!(etats::question_avant_de_modifier(&ctx(&r), "2026-12", 0).unwrap(), None);
    etats::modifier(&ctx(&r), "2026-12", 0).unwrap();
    let e = fichier(&r, "2026-11")["engagements"][0].clone();
    assert!(e.get("valide").is_none() && e.get("figees").is_none(), "{}", e);
    assert!(lire_le_mois(&r, "2026-11")[0].valide().is_none());
}
