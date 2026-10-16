//! La validation d'une paie : `valider-paie` valide CE contrat et pas son jumeau du meme jour (par numero ou par rang),
//! `etat-par-paie` ne prete pas la validation de l'un a l'autre, `contrat-recu` ne touche pas au calcul du mois, et une
//! charge incomplete refuse. Sur `fictive-3090-petite` (config, journal, novembre 2026).

use crate::outils::{compagnie, dossier_de_test, lire, refus, reussi};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn compagnie(nom: &str) -> PathBuf {
    compagnie::config_et_mois("fictive-3090-petite", &["2026-11"], &dossier_de_test("routes_validation", nom).join("compagnie"))
}

fn route(racine: &Path, nom: &str, charge: Value) -> Value {
    crate::outils::route(racine, nom, charge, "2026-10-03")
}

#[test]
fn les_routes_a_charge_incomplete_refusent_sans_ecrire() {
    let racine = compagnie("incomplet");
    let avant = std::fs::read(racine.join("paies/2026-11.json")).unwrap();
    for nom in ["valider-paie", "contrat-recu", "casc-regularise"] {
        refus(&route(&racine, nom, json!({})));
    }
    refus(&route(&racine, "valider-paie", json!({"mois": "2099-01", "salarie": "personne", "debut": "2099-01-01"})));
    assert_eq!(std::fs::read(racine.join("paies/2026-11.json")).unwrap(), avant);
    assert!(!racine.join("config/casc-svp.json").exists());
}

/// Novembre de la petite avec un JUMEAU : un second contrat de Brioche qui commence le meme jour (un cachet, le
/// 24), rang 1 du fichier. `numeros` : ceux des deux contrats, ou rien.
fn novembre_avec_un_jumeau(racine: &Path, numeros: Option<(&str, &str)>) {
    let mut fichier = lire(racine, "paies/2026-11.json");
    let mut jumeau = fichier["engagements"][0].clone();
    jumeau["fin"] = json!("2026-11-24");
    jumeau["date_paiement"] = json!("2026-11-24");
    jumeau["lignes"][0]["quantite"] = json!(1);
    jumeau["lignes"][0]["dates"] = json!(["2026-11-24"]);
    if let Some((a, b)) = numeros {
        fichier["engagements"][0]["numero"] = json!(a);
        jumeau["numero"] = json!(b);
    }
    fichier["engagements"].as_array_mut().unwrap().insert(1, jumeau);
    std::fs::write(racine.join("paies/2026-11.json"), serde_json::to_vec_pretty(&fichier).unwrap()).unwrap();
}

#[test]
fn valider_une_paie_valide_ce_contrat_et_pas_son_jumeau_du_meme_jour() {
    let racine = compagnie("valider-jumeaux");
    novembre_avec_un_jumeau(&racine, Some(("BR.26.01", "BR.26.02")));
    let charge = json!({"mois": "2026-11", "salarie": "BRIOCHE", "debut": "2026-11-24"});
    // Sans dire lequel, deux contrats du meme jour : refus, rien n'est ecrit.
    let avant = std::fs::read(racine.join("paies/2026-11.json")).unwrap();
    let (_, message) = refus(&route(&racine, "valider-paie", charge.clone()));
    assert!(message.contains("BR.26.01") && message.contains("BR.26.02"), "{}", message);
    assert_eq!(std::fs::read(racine.join("paies/2026-11.json")).unwrap(), avant);
    // Par son numero : le jumeau, et lui seul.
    let mut c = charge.clone();
    c["numero"] = json!("BR.26.02");
    reussi(&route(&racine, "valider-paie", c));
    let e = lire(&racine, "paies/2026-11.json")["engagements"].clone();
    assert!(e[0].get("valide").is_none(), "{}", e);
    assert!(e[1]["valide"]["le"].is_string() && e[1]["valide"]["carte"].is_object(), "{}", e);
    assert_eq!((e[0]["numero"].as_str(), e[1]["numero"].as_str()), (Some("BR.26.01"), Some("BR.26.02")));
    // Un numero que le fichier ne porte pas : refus.
    let mut c = charge.clone();
    c["numero"] = json!("BR.26.09");
    refus(&route(&racine, "valider-paie", c));

    // Sans numeros (avant la production), par le rang parmi les contrats du meme jour.
    let racine = compagnie("valider-jumeaux-rang");
    novembre_avec_un_jumeau(&racine, None);
    refus(&route(&racine, "valider-paie", charge.clone()));
    let mut c = charge;
    c["rang"] = json!(1);
    reussi(&route(&racine, "valider-paie", c));
    let e = lire(&racine, "paies/2026-11.json")["engagements"].clone();
    assert!(e[0].get("valide").is_none(), "{}", e);
    assert!(e[1]["valide"]["le"].is_string() && e[1]["valide"]["carte"].is_object(), "{}", e);
}

/// L'ETAT DE CHAQUE JUMEAU : un seul des deux contrats du meme jour est valide ; `etat-par-paie` le dit de lui, et
/// pas de l'autre -- avec ou sans numeros. Chaque ligne porte son rang, celui que l'ecran rend a `valider-paie`.
#[test]
fn l_etat_par_paie_ne_prete_pas_la_validation_d_un_jumeau_a_l_autre() {
    for (nom, numeros) in [("etat-jumeaux-rang", None), ("etat-jumeaux-numero", Some(("BR.26.01", "BR.26.02")))] {
        let racine = compagnie(nom);
        novembre_avec_un_jumeau(&racine, numeros);
        let mut c = json!({"mois": "2026-11", "salarie": "BRIOCHE", "debut": "2026-11-24", "rang": 1});
        if let Some((_, b)) = numeros {
            c["numero"] = json!(b);
        }
        reussi(&route(&racine, "valider-paie", c));
        let r = route(&racine, "etat-par-paie", json!({"mois": "2026-11"}));
        let jumeaux: Vec<Value> = reussi(&r)["paies"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|l| l["salarie"] == "BRIOCHE" && l["debut"] == "2026-11-24")
            .cloned()
            .collect();
        assert_eq!(jumeaux.len(), 2, "{}", r);
        assert_eq!(jumeaux[0]["valide"], Value::Null, "{} : le premier n'est pas valide : {}", nom, jumeaux[0]);
        assert!(jumeaux[1]["valide"].is_string(), "{} : le second l'est : {}", nom, jumeaux[1]);
        assert_eq!((jumeaux[0]["rang"].clone(), jumeaux[1]["rang"].clone()), (json!(0), json!(1)), "{}", r);
    }
}

/// Noter qu'un contrat signe est revenu ne touche pas au calcul du mois (`paies/AAAA-MM.calcul.json`) : ni cree s'il
/// n'y est pas, ni change s'il y est. La route est appelee DANS ce processus : l'etat du mois se refait ensuite dans la
/// fenetre (`etat_garde::FileDesEtats`), et lui ecrit le calcul, a dessein.
#[test]
fn le_contrat_recu_ne_touche_pas_au_calcul_du_mois() {
    let racine = compagnie("contrat-recu-calcul");
    let o: std::collections::HashMap<String, String> =
        [("racine".to_string(), racine.to_string_lossy().into_owned()), ("date".to_string(), "2026-10-03".to_string())].into();
    let ctx = paie::contexte::Contexte::nouveau(&racine, Some(paie::date::Jour::lire("2026-10-03").unwrap())).unwrap();
    let contrat_recu = |recu: bool| {
        paie::routes_ecritures_g4::route(&o, &ctx, "contrat-recu", &json!({"mois": "2026-11", "salarie": "BRIOCHE", "debut": "2026-11-24", "recu": recu}))
            .unwrap_or_else(|e| panic!("{:?}", e))
    };
    let calcul = racine.join("paies/2026-11.calcul.json");
    let _ = std::fs::remove_file(&calcul);
    contrat_recu(true);
    assert!(!calcul.exists(), "le calcul du mois ne se cree pas");
    std::fs::write(&calcul, b"{\"perime\": true}").unwrap();
    contrat_recu(false);
    assert_eq!(std::fs::read(&calcul).unwrap(), b"{\"perime\": true}");
}
