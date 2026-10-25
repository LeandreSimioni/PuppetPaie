//! Le cumul des droits intermittents : le plafond d'enseignement de la fiche livree (`logiciel/donnees/rubriques-base.json`,
//! France Travail cite), les positions sur chaque fenetre (engagements fabriques) contre `donnees/cumul-positions.json`,
//! et le signe d'une ligne d'enseignement.

use crate::outils;
use droit::bareme::canon;
use droit::contexte::Contexte;
use engagement::cumul::{
    fenetre_annee_civile, fenetre_anniversaire, fenetre_douze_mois, plafond_enseignement, position, Constantes,
    Engagement, Fenetre,
};
use socle::date::Jour;
use socle::fichiers::lire_json;
use socle::num::dec;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn donnees() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/donnees")
}

fn oracle(quoi: &str) -> PathBuf {
    donnees().join(format!("cumul-{}.json", quoi))
}

fn attendus(quoi: &str) -> Value {
    lire_json(&oracle(quoi)).unwrap()
}

fn fictive(nom: &str) -> PathBuf {
    outils::fictive(nom)
}

fn jour(v: &Value) -> Option<Jour> {
    v.as_str().map(|s| Jour::lire(s).unwrap())
}

/// « Retenues pour les 507 heures dans la limite de 70 h (moins de 50 ans) ou 120 h (50 ans et plus) ; l'age retenu est
/// celui a la fin du contrat » (la fiche `enseignement`, France Travail, francetravail.fr/spectacle, 10/09/2026).
#[test]
fn le_plafond_d_enseignement_suit_l_age_a_la_date() {
    let fiche = outils::lire(&outils::code_livre(), "rubriques-base.json")["rubriques"]["enseignement"]["plafond_annuel_heures"].clone();
    assert_eq!(fiche, json!({"moins_de_50_ans": 70, "50_ans_et_plus": 120}));
    let racine = fictive("fictive-1285-grande");
    let j = |s: &str| Jour::lire(s).unwrap();
    let plafond = |naissance: Option<&str>, date: &str| plafond_enseignement(&racine, naissance.map(j), j(date)).unwrap();
    assert_eq!(plafond(None, "2026-09-27"), 70, "sans date de naissance, le plafond bas");
    assert_eq!(plafond(Some("1976-09-27"), "2026-09-27"), 120, "50 ans le jour meme");
    assert_eq!(plafond(Some("1976-09-28"), "2026-09-27"), 70, "50 ans le lendemain");
    assert_eq!(plafond(Some("1976-02-29"), "2026-02-28"), 70, "ne un 29 fevrier : pas encore 50 ans le 28");
    assert_eq!(plafond(Some("1976-02-29"), "2026-03-01"), 120);
    assert_eq!(plafond(Some("1975-12-31"), "2025-12-31"), 120);
    assert_eq!(plafond(Some("2010-05-05"), "2026-09-27"), 70);
}

fn engagement(r: &Value) -> Engagement {
    let entier = |c: &str| r[c].as_i64().unwrap();
    Engagement {
        salarie: r["salarie"].as_str().unwrap().to_string(),
        debut: jour(&r["debut"]).unwrap(),
        fin: jour(&r["fin"]).unwrap(),
        numero_objet: r["numero_objet"].clone(),
        heures: entier("heures"),
        cachets: entier("cachets"),
        jours: entier("jours"),
        brut: dec(&r["brut"]).unwrap(),
        heures_enseignement: entier("heures_enseignement"),
    }
}

fn fenetre(c: &Constantes, spec: &[Value]) -> Fenetre {
    match spec[0].as_str().unwrap() {
        "douze_mois" => fenetre_douze_mois(c, jour(&spec[1]).unwrap()).unwrap(),
        "annee" => fenetre_annee_civile(spec[1].as_i64().unwrap() as i32, jour(&spec[2])),
        "anniversaire" => fenetre_anniversaire(c, jour(&spec[1]).unwrap()).unwrap(),
        _ => (jour(&spec[1]).unwrap(), jour(&spec[2]).unwrap(), "periode".to_string()),
    }
}

#[test]
fn les_positions_sur_chaque_fenetre() {
    let mut a = attendus("positions");
    let aujourdhui = jour(&a["aujourdhui"]).unwrap();
    let ctx = Contexte::nouveau(&fictive("fictive-1285-grande"), Some(aujourdhui)).unwrap();
    let c = Constantes::lire(&ctx.racine).unwrap();
    let engs: Vec<Engagement> = a["lignes"].as_array().unwrap().iter().map(engagement).collect();
    let cas = a["cas"].as_array_mut().unwrap();
    assert!(cas.len() > 40);
    for cas in cas.iter_mut() {
        let f = fenetre(&c, cas["fenetre"].as_array().unwrap());
        let p = position(&ctx, &c, &engs, cas["salarie"].as_str().unwrap(), &f, jour(&cas["naissance"]));
        cas["attendu"] = canon(&p.canon(aujourdhui).unwrap());
    }
    outils::oracle("cumul-positions.json", "", &outils::trie(&a));
}

/// Reste R1 #162 : une ligne « - » (une rubrique de sens `-`) retire ses heures d'enseignement au lieu d'en ajouter,
/// comme au calcul (`calcul::lignes_brut`) et a l'engagement (`engagement.rs`, « le signe porte aussi sur les heures »).
/// Aucune rubrique livree n'est a la fois d'enseignement et de sens `-` : la compagnie fictive en ajoute une dans son
/// catalogue propre (`config/rubriques-compagnie.json`), ce que toute compagnie peut faire.
#[test]
fn une_ligne_moins_d_enseignement_retire_ses_heures() {
    let racine = outils::dossier_de_test("cumul", "enseignement-moins");
    outils::copier(&fictive("fictive-3090-petite").join("config"), &racine.join("config"));
    let base = outils::lire(&outils::code_livre(), "rubriques-base.json");
    let mut retrait = base["rubriques"]["enseignement"].clone();
    retrait["sens"] = "-".into();
    retrait["libelle"] = "Masterclass annulee".into();
    outils::ecrire(&racine, "config/rubriques-compagnie.json", &json!({"rubriques": {"enseignement_annule": retrait}}));
    let ctx = Contexte::nouveau(&racine, Some(Jour::lire("2026-10-11").unwrap())).unwrap();
    let lignes = vec![json!({"rubrique": "enseignement", "quantite": 10, "taux": 50.0}), json!({"rubrique": "enseignement_annule", "quantite": 4, "taux": 50.0})];
    assert_eq!(engagement::cumul::heures_enseignement(&ctx, &lignes).unwrap(), 6);
}
