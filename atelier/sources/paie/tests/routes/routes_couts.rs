//! `couts` : le cout de l'annee, paye et prevu, par spectacle et par salarie ; un mois declare avant PuppetPaie compte
//! sans cout employeur. Sur `demo-3090-petite` (config et toutes ses saisies).

use crate::outils::{compagnie, copier, demo, dossier_de_test, refus, route};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn compagnie(nom: &str) -> PathBuf {
    let racine = compagnie::saisies("demo-3090-petite", &[], &dossier_de_test("routes_couts", nom).join("compagnie"));
    copier(&demo("demo-3090-petite").join("paies"), &racine.join("paies"));
    racine
}

fn couts(racine: &Path, charge: Value) -> Value {
    route(racine, "couts", charge, "2026-10-03")
}

fn somme(v: &Value) -> f64 {
    v.as_object().unwrap().values().map(|d| d["cout"].as_str().unwrap().parse::<f64>().unwrap() + d["prevu"].as_str().unwrap().parse::<f64>().unwrap()).sum()
}

#[test]
fn le_cout_de_l_annee_se_partage_entre_ce_qui_est_paye_et_ce_qui_est_prevu() {
    let racine = compagnie("couts");
    let r = couts(&racine, json!({"annee": "2026"}));
    assert_eq!(r["ok"], json!(true), "{}", r);
    let c = &r["resultat"];
    assert_eq!((c["annee"].clone(), c["jusqu_a"].clone()), (json!("2026"), json!("2026-10")));
    assert!(!c["spectacles"].as_object().unwrap().is_empty());
    // Les montants sont des textes a deux decimales ; ce qui est apres octobre est `prevu`.
    let un = c["salaries"].as_object().unwrap().values().next().unwrap();
    for k in ["brut", "charges", "cout", "prevu"] {
        let t = un[k].as_str().unwrap();
        assert_eq!(t.len() - t.find('.').unwrap(), 3, "{} : {}", k, t);
    }
    assert!(c["salaries"].as_object().unwrap().values().any(|d| d["prevu"] != json!("0.00")), "novembre et decembre sont saisis pour plus tard");
    assert_eq!(un["incomplet"], json!(false));
    // Meme livre de paie, lu par spectacle et par salarie : les totaux tombent (compagnie sans permanent).
    assert!((somme(&c["spectacles"]) - somme(&c["salaries"])).abs() < 0.005);
    // Un nombre vaut son texte ; une annee sans paie ou sans nom ne rend rien.
    assert_eq!(couts(&racine, json!({"annee": 2026}))["resultat"]["annee"], json!("2026"));
    for annee in ["2019", ""] {
        let r = couts(&racine, json!({"annee": annee}));
        assert_eq!((r["resultat"]["spectacles"].clone(), r["resultat"]["salaries"].clone()), (json!({}), json!({})), "{}", annee);
    }
    assert!(refus(&couts(&racine, json!({}))).1.contains("annee"));
}

#[test]
fn un_mois_declare_avant_puppetpaie_compte_sans_cout_employeur_et_se_dit_incomplet() {
    let racine = compagnie("couts-dsn");
    std::fs::create_dir_all(racine.join("historique")).unwrap();
    std::fs::write(
        racine.join("historique/paies.json"),
        json!({"paies": [{"mois": "2024-01", "salarie": "BRIOCHE", "contrat": "OLD.1", "brut": "1000.50", "net_a_payer": "790.10"}]}).to_string(),
    )
    .unwrap();
    let r = couts(&racine, json!({"annee": "2024"}));
    let d = &r["resultat"]["salaries"]["BRIOCHE"];
    assert_eq!(
        d,
        &json!({"bulletins": 1, "brut": "1000.50", "charges": "0.00", "cout": "0.00", "prevu": "0.00", "incomplet": true}),
        "{}",
        r
    );
}
