//! LES ROUTES DE L'ENGAGEMENT REFUSENT, ET RIEN N'EST ECRIT : `valider-engagement` vise un seul contrat, et ne remet pas
//! en saisie un engagement dont les documents sont partis sans la reponse a la question du moteur ; une charge
//! incomplete se refuse. Les regles de l'engagement lui-meme se prouvent dans sa crate (`engagement/tests/`).
//!
//! Sur la config et le journal de `fictive-3090-petite`, des engagements saisis par le test, au 05/11/2026.

use crate::outils::{compagnie, dossier_de_test, ecrire, lire, refus, reussi, route_en_processus};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const JOUR: &str = "2026-11-05";
const SPECTACLE: &str = "256Z00920011";

/// La config et le journal de la petite fictive, sans engagement.
fn compagnie(nom: &str) -> PathBuf {
    compagnie::config_et_mois("fictive-3090-petite", &[], &dossier_de_test("engagements", nom).join("compagnie"))
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
fn une_charge_incomplete_ou_sans_reponse_a_la_question_refuse_sans_rien_ecrire() {
    let r = compagnie("incomplet");
    a_cheval(&r);
    let avant = std::fs::read(r.join("engagements/2026-11.json")).unwrap();
    for nom in ["valider-engagement", "contrat-recu", "casc-regularise"] {
        refus(&route(&r, nom, json!({})));
    }
    refus(&route(&r, "valider-engagement", json!({"mois": "2099-01", "salarie": "personne", "debut": "2099-01-01"})));
    assert_eq!(std::fs::read(r.join("engagements/2026-11.json")).unwrap(), avant);
    assert!(!r.join("config/casc-svp.json").exists());
    // Les documents de novembre sont partis : « Modifier… » sans la reponse a la question du moteur se refuse, depuis
    // chacun des mois de l'engagement, et rien ne bouge.
    reussi(&valider(&r, "2026-11", "BRIOCHE", "2026-11-28", true));
    let numero = lire(&r, "engagements/2026-11.json")["engagements"][0]["numero"].clone();
    let mut j = lire(&r, "journal/depots.json");
    j.as_array_mut().unwrap().push(json!({"quoi": "documents", "quand": "2026-12-03T10:00:00", "mois": "2026-11", "mode": "reel",
        "accepte": true, "detail": {"salaries": [{"cle": "BRIOCHE", "nom": "Fanchon BRIOCHE", "pieces": []}]},
        "versions": [{"salarie": "BRIOCHE", "numero": numero, "version": 1}]}));
    ecrire(&r, "journal/depots.json", &j);
    let avant = std::fs::read(r.join("engagements/2026-11.json")).unwrap();
    for mois in ["2026-11", "2026-12"] {
        let rendu = valider(&r, mois, "BRIOCHE", "2026-11-28", false);
        assert_eq!(rendu["classe"], json!(paie::routes_ecritures_g4::CLASSE_DOCUMENTS_PARTIS), "{}", rendu);
        assert_eq!(rendu["erreur"], json!(engagement::etats::QUESTION_DOCUMENTS_PARTIS));
    }
    assert_eq!(std::fs::read(r.join("engagements/2026-11.json")).unwrap(), avant, "rien ne bouge sans la reponse");
}
