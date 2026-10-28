//! La nationalité d'un salarié : le pays se choisit dans une liste, la codification UE (S21.G00.30.013) s'en déduit, un
//! pays hors de la liste laisse l'administrateur choisir la zone, le pays de naissance (30.015) se lit dans la table « Pays »
//! de la DSN ; à la fiche, au questionnaire, dans la DSN et au registre du personnel.
//!
//! Les textes : cahier technique DSN 2026.1, `atelier/reference/norme-dsn/texte/bloc-S21.G00.30.txt` (30.013, p. 173,
//! l. 267-277 : « 01 - France / 02 - UE / 03 - EEE / 04 - Reste du Monde », « le code le plus précis applicable » ; 30.015,
//! p. 174, l. 297-303, CRE-11) ; les Etats : `atelier/reference/sources/union-europeenne/LISEZ-MOI.md` ; la liste :
//! `logiciel/donnees/pays-base.json` (COG de l'INSEE 2026, `atelier/reference/sources/insee-cog-pays/`) ; la table « Pays » :
//! `logiciel/donnees/pays-dsn-base.json` (`atelier/reference/sources/dsn-nomenclatures/tables-PAYS-et-PAY-20260924.txt`) ;
//! le pays hors liste : décision de Léandre du 10/10/2026 ; le registre : C. trav., D1221-23
//! (`atelier/reference/sources/code-du-travail/xml/D1221-23.xml`).

use serde_json::{json, Value};
use std::collections::HashMap;

#[test]
fn la_fiche_dit_l_ancien_03_avant_la_dsn() {
    let erreurs = |v: Value| -> Vec<Value> {
        paie::champs::verifier_fiche("salarie", v.as_object().unwrap())
            .unwrap()
            .into_iter()
            .filter(|a| a["champ"] == "code_nationalite_dsn" && a["gravite"] == "erreur")
            .collect()
    };
    let avec = |k: &[(&str, &str)]| {
        let mut b = json!({"nom": "ANDERSEN", "prenom": "INGRID", "sexe": "2"});
        for (c, v) in k {
            b[*c] = json!(v);
        }
        b
    };
    assert_eq!(erreurs(avec(&[("code_nationalite_dsn", "03")])).len(), 1, "un 03 sans pays se dit dans la fiche");
    assert!(erreurs(avec(&[("code_nationalite_dsn", "03"), ("pays_nationalite", "NO")])).is_empty());
    assert!(erreurs(avec(&[("code_nationalite_dsn", "04")])).is_empty(), "04 est un choix de la fiche");
}

// ------------------------------------------------------------------ la liste des pays





#[test]
fn le_questionnaire_lit_le_pays_dans_la_liste_et_en_deduit_la_codification() {
    use documents::fiche_questionnaire::{fiche_depuis_reponses, pays_lu};
    let lire = |cle: &'static str, reponse: &str| fiche_depuis_reponses(&[(cle, reponse.to_string())].into_iter().collect());
    for (reponse, pays, code) in [("norvégienne", "NO", "03"), ("Marocain", "MA", "04"), ("italienne", "IT", "02"), ("liechtensteinoise", "LI", "03"), ("britannique", "GB", "04"), ("Suisse", "CH", "04"), ("Islande", "IS", "03"), ("française", "FR", "01")] {
        let (d, illisible) = lire("nationalite", reponse);
        assert!(illisible.is_empty(), "{} : {:?}", reponse, illisible);
        assert_eq!((&d["pays_nationalite"], &d["code_nationalite_dsn"]), (&json!(pays), &json!(code)), "{}", reponse);
    }
    // Des noms que l'ancienne table courte ne connaissait pas.
    assert_eq!(pays_lu("Sénégal").map(|p| p.0), Some("SN".into()));
    assert_eq!(pays_lu("Géorgie").map(|p| p.0), Some("GE".into()));
    assert_eq!(pays_lu("Islande"), Some(("IS".into(), Some("03"))));
    assert_eq!(pays_lu("Côte d’Ivoire").map(|p| p.0), Some("CI".into()));
    assert_eq!(pays_lu("République tchèque").map(|p| p.0), Some("CZ".into()));
    // Un code que la liste ne connait pas ne se garde plus.
    assert_eq!(pays_lu("zz"), None);
    let rep: HashMap<&'static str, String> = [("pays", "Atlantide".to_string()), ("nationalite", "Vietnam".to_string())].into_iter().collect();
    let (d, illisible) = fiche_depuis_reponses(&rep);
    assert!(d.get("pays_naissance").is_none(), "{:?}", d);
    assert_eq!(illisible, vec!["pays de naissance : « Atlantide »".to_string()]);
    assert_eq!((d["pays_nationalite"].as_str(), d["code_nationalite_dsn"].as_str()), (Some("VN"), Some("04")));
    // Un pays de naissance que le COG n'a pas, mais la table « Pays » de la DSN si, se lit par son libelle.
    let (d, illisible) = lire("pays", "Kosovo");
    assert_eq!(d.get("pays_naissance"), Some(&json!("XK")), "{:?} {:?}", d, illisible);
    assert!(illisible.is_empty(), "{:?}", illisible);
}

// ------------------------------------------------------------------ un pays hors de la liste
