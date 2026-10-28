//! Les regularisations par la route : `regularisation-ecrire`, `regularisation-annuler`, `regularisation-apercu`. La
//! fictive a ses DSN de 2025 parties pour de vrai, l'echeance passee : juin 2025 se regularise. Ce qui ne change
//! rien, un mois non depose, une preuve absente refusent sans rien ecrire. Sur `fictive-3090-petite` (config, journal,
//! juin 2025 et decembre 2026).

use crate::outils::{compagnie, dossier_de_test, lire, refus};
use paie::coffre::Coffre;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn compagnie(nom: &str) -> PathBuf {
    compagnie::config_et_mois("fictive-3090-petite", &["2025-06", "2026-12"], &dossier_de_test("routes_regularisations", nom).join("compagnie"))
}

fn route(racine: &Path, nom: &str, charge: Value) -> Value {
    crate::outils::route(racine, nom, charge, "2026-10-03")
}

fn refuse(r: &Value) -> String {
    refus(r).1
}

fn octets(racine: &Path, rel: &str) -> Option<Vec<u8>> {
    std::fs::read(racine.join(rel)).ok()
}

/// L'engagement declare de BRIOCHE en juin 2025, avec le cachet a un autre taux.
fn correction(racine: &Path, taux: f64) -> Value {
    let d = route(racine, "regularisation-mois", json!({"mois": "2025-06"}));
    let mut e = d["resultat"]["engagements"][0]["engagement"].clone();
    for l in e["lignes"].as_array_mut().unwrap() {
        if l["rubrique"] == json!("cachet") {
            l["taux"] = json!(taux);
        }
    }
    json!({"mois": "2025-06", "salarie": "BRIOCHE", "debut": "2025-06-03", "engagement": e, "motif": "  le taux etait faux  "})
}

#[test]
fn une_correction_s_ecrit_s_annule_et_se_refuse() {
    let racine = compagnie("correction");
    let r = route(&racine, "regularisation-ecrire", correction(&racine, 133.33));
    assert_eq!(r["ok"], json!(true), "{}", r);
    let c = &r["resultat"]["correction"];
    assert_eq!((c["numero"].clone(), c["motif"].clone(), c["qui"].clone()), (json!(1), json!("le taux etait faux"), json!("interface")));
    assert_eq!(r["resultat"]["etat"], json!("en attente"));
    let f = lire(&racine, "regularisations/2025-06.json");
    assert_eq!(f["mois"], json!("2025-06"));
    let mut champs: Vec<&String> = f["corrections"][0].as_object().unwrap().keys().collect();
    champs.sort();
    assert_eq!(champs, ["annulee_le", "debut", "decidee_le", "engagement", "mois", "motif", "numero", "portee_par", "preparee_par", "qui", "salarie"]);
    // Le contrat declare garde son numero et ses bornes.
    assert_eq!(f["corrections"][0]["engagement"]["numero"], json!("BR1.25.04"));
    assert_eq!(f["corrections"][0]["engagement"]["debut"], json!("2025-06-03"));
    assert_eq!(f["corrections"][0]["portee_par"], Value::Null);

    // Une seule en cours par engagement : la suivante la remplace (21/10/2026, rien n'est parti) ; ce qui ne change rien
    // ou ne se motive pas s'ecarte.
    let r = route(&racine, "regularisation-ecrire", correction(&racine, 140.0));
    assert_eq!(r["resultat"]["correction"]["numero"], json!(2), "{}", r);
    let f = lire(&racine, "regularisations/2025-06.json");
    assert!(f["corrections"][0]["annulee_le"].is_string());
    assert_eq!(f["corrections"][0]["remplacee_par"], json!(2));
    let apres_refus = octets(&racine, "regularisations/2025-06.json");
    let mut sans_motif = correction(&racine, 140.0);
    sans_motif["motif"] = json!("   ");
    assert!(refuse(&route(&racine, "regularisation-ecrire", sans_motif)).contains("se motive"));

    let r = route(&racine, "regularisation-annuler", json!({"mois": "2025-06", "numero": "2"}));
    assert_eq!(r["resultat"]["etat"], json!("annulee"), "{}", r);
    assert_eq!(r["resultat"]["correction"]["qui"], json!("interface"));
    assert_ne!(octets(&racine, "regularisations/2025-06.json"), apres_refus);
    assert!(refuse(&route(&racine, "regularisation-annuler", json!({"mois": "2025-06", "numero": 2}))).contains("deja annulee"));
    assert!(refuse(&route(&racine, "regularisation-annuler", json!({"mois": "2025-06", "numero": 7}))).contains("Pas de correction n° 7"));

    // Annulee, elle ne bloque plus : la suivante prend le numero 3.
    let r = route(&racine, "regularisation-ecrire", correction(&racine, 150.0));
    assert_eq!(r["resultat"]["correction"]["numero"], json!(3), "{}", r);
}

#[test]
fn une_correction_qui_ne_change_rien_ou_sur_un_mois_non_depose_n_ecrit_rien() {
    let racine = compagnie("correction-refus");
    // Aucune DSN reelle pour decembre 2026 (saisi, pas encore parti) : il se saisit et se depose normalement.
    let mut c = correction(&racine, 133.33);
    c["mois"] = json!("2026-12");
    assert!(refuse(&route(&racine, "regularisation-ecrire", c)).contains("Aucune DSN reelle"));
    // Le meme taux que celui declare.
    let d = route(&racine, "regularisation-mois", json!({"mois": "2025-06"}));
    let e = d["resultat"]["engagements"][0]["engagement"].clone();
    let r = route(&racine, "regularisation-ecrire", json!({"mois": "2025-06", "salarie": "BRIOCHE", "debut": "2025-06-03", "engagement": e, "motif": "rien"}));
    assert!(refuse(&r).contains("ne change rien"), "{}", r);
    let r = route(&racine, "regularisation-ecrire", json!({"mois": "2025-06", "salarie": "NULLE", "debut": "2025-06-03", "engagement": e, "motif": "x"}));
    assert!(refuse(&r).contains("Aucun engagement de NULLE"), "{}", r);
    assert!(!racine.join("regularisations").exists());
}

#[test]
fn une_correction_portee_ne_s_annule_plus() {
    let racine = compagnie("correction-portee");
    assert_eq!(route(&racine, "regularisation-ecrire", correction(&racine, 133.33))["ok"], json!(true));
    let mut f = lire(&racine, "regularisations/2025-06.json");
    f["corrections"][0]["portee_par"] = json!({"mois": "2026-09", "fichier": "DSN_x.dsn", "quand": "2026-09-30 10:00"});
    std::fs::write(racine.join("regularisations/2025-06.json"), f.to_string()).unwrap();
    let avant = octets(&racine, "regularisations/2025-06.json");
    let message = refuse(&route(&racine, "regularisation-annuler", json!({"mois": "2025-06", "numero": 1})));
    assert!(message.contains("PORTEE par la DSN de 2026-09 (deposee le 2026-09-30 10:00)"), "{}", message);
    assert_eq!(octets(&racine, "regularisations/2025-06.json"), avant);
    // Portee, elle ne bloque plus une correction nouvelle.
    let r = route(&racine, "regularisation-ecrire", correction(&racine, 150.0));
    assert_eq!(r["resultat"]["correction"]["numero"], json!(2), "{}", r);
}

#[test]
fn un_mot_de_passe_pose_est_exige_pour_ecrire_et_pour_annuler_une_correction() {
    let racine = compagnie("correction-mdp");
    let coffre = Coffre { racine: racine.clone(), dossier_cle: racine.join("pas-de-cle"), maintenant: "2026-10-03T14:05".into(), variables: BTreeMap::new() };
    // Sans mot de passe pose, il n'en faut pas : le geste reste possible.
    assert_eq!(route(&racine, "regularisation-ecrire", correction(&racine, 133.33))["ok"], json!(true));
    coffre.poser_ouverture("sesame-123", None).unwrap();
    let avant = octets(&racine, "regularisations/2025-06.json");
    let mut c = correction(&racine, 140.0);
    for (essai, attendu) in [(None, "Un mot de passe du logiciel est pose"), (Some(""), "Un mot de passe du logiciel est pose"), (Some("faux"), "refuse")] {
        if let Some(m) = essai {
            c["motdepasse"] = json!(m);
        }
        let r = route(&racine, "regularisation-annuler", json!({"mois": "2025-06", "numero": 1, "motdepasse": essai}));
        assert_eq!(r["classe"], json!("PreuveInsuffisante"), "{}", r);
        assert!(refuse(&r).contains("Rien n'a ete modifie"), "{}", r);
        let r = route(&racine, "regularisation-ecrire", c.clone());
        assert_eq!(r["classe"], json!("PreuveInsuffisante"), "{}", r);
        assert!(refuse(&r).contains(attendu), "{}", r);
    }
    assert_eq!(octets(&racine, "regularisations/2025-06.json"), avant);
    let r = route(&racine, "regularisation-annuler", json!({"mois": "2025-06", "numero": 1, "motdepasse": "sesame-123"}));
    assert_eq!(r["resultat"]["etat"], json!("annulee"), "{}", r);
}

#[test]
fn un_apercu_d_un_mois_illisible_refuse() {
    let racine = compagnie("apercu-mois");
    let r = route(&racine, "regularisation-apercu", json!({"mois": "abc", "salarie": "B", "debut": "d", "engagement": {}}));
    assert_eq!(r["ok"], json!(false), "{}", r);
}
