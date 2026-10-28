//! La deduction forfaitaire specifique (DFS) en paie.
//!
//! Textes (`atelier/reference/sources/dfs/LISEZ-MOI.md`) :
//! - arrete du 4 septembre 2025 relatif aux frais professionnels, art. 9 (base LEGI,
//!   `dfs/arrete-2025-frais-professionnels/`) : les professions de l'article 5 de l'annexe IV du CGI au 31/12/2000 ;
//!   l'option de l'employeur (accord collectif, CE, CSE ou DP, a defaut chaque salarie) ; « dans la limite de 7 600 euros
//!   par annee civile » ; les taux du spectacle vivant d'annee en annee (II) ; l'assiette garde les remboursements de
//!   frais, sauf la liste de l'annexe 2 ; « sans prejudice du sixieme alinea de l'article R. 242-1 » ;
//! - l'annexe 2, I, 2° et 5° : les defraiements de tournee des artistes dramatiques, lyriques, choregraphiques et
//!   regisseurs de theatre, les frais de deplacement des musiciens, choristes et chefs d'orchestre restent hors de
//!   l'assiette ;
//! - BOSS « Frais professionnels », § 2320 (`atelier/reference/sources/boss/boss-frais-professionnels-20261013.txt`) : pour
//!   le spectacle vivant, par tolerance, tout remboursement de frais au sens de l'arrete se cumule avec la DFS, hors de
//!   l'assiette abattue ;
//! - CSS R. 242-1, al. 6, et BOSS « Assiette generale » § 410 : l'assiette abattue ne descend pas sous le SMIC ;
//! - annexes VIII et X du 15/11/2024, art. 49 : le chomage « avant application de l'abattement » ; norme DSN, bloc 78 :
//!   les Conges Spectacles « avant toute deduction forfaitaire specifique ».
//! La reduction generale sous DFS : arrete du 4 decembre 2019, art. 1er (`dfs/arrete-2019-reduction-generale/`).

use crate::outils::lire;
use serde_json::{json, Value};

#[test]
fn la_fiche_salarie_dit_une_profession_et_son_accord_la_fiche_entreprise_l_option() {
    // Lot R9 : la fiche salarie dit la PROFESSION de l'article 5 de l'annexe IV du CGI (plus une categorie, « 20 » ou
    // « 25 »), et l'accord du salarie quand l'option repose sur lui ; la fiche entreprise dit l'option de l'employeur
    // (arrete du 4 septembre 2025, art. 9, I, al. 2 et 3). Le moteur verifie les valeurs (`champs::verifier_fiche`).
    let erreur = |nom: &str, cle: &str, v: &str| -> bool {
        let mut f = serde_json::Map::new();
        f.insert(cle.into(), json!(v));
        declarations::champs::verifier_fiche(nom, &f).unwrap().iter().any(|a| a["champ"] == cle && a["gravite"] == "erreur")
    };
    assert!(erreur("salarie", "deduction_forfaitaire", "25"), "une categorie n'est plus une profession");
    for profession in ["musicien", "choriste", "chef_orchestre", "regisseur_theatre", "artiste_dramatique", "artiste_lyrique",
                       "artiste_choregraphique", "artiste_cinematographique", ""] {
        assert!(!erreur("salarie", "deduction_forfaitaire", profession), "{}", profession);
    }
    for accord in ["contrat", "consentement_annuel", "refus", ""] {
        assert!(!erreur("salarie", "deduction_forfaitaire_accord", accord), "{}", accord);
    }
    assert!(erreur("salarie", "deduction_forfaitaire_accord", "oui"));
    for option in ["accord_collectif", "accord_cse", "accord_individuel", ""] {
        assert!(!erreur("entreprise", "deduction_forfaitaire_option", option), "{}", option);
    }
    assert!(erreur("entreprise", "deduction_forfaitaire_option", "oui"));
    // Les professions de la fiche sont celles du bareme de l'annee, et aucune autre.
    let bar: Value = lire(&crate::outils::code_livre(), "baremes/2026.json");
    let mut du_bareme: Vec<String> = bar["deduction_forfaitaire"]["professions"].as_object().unwrap().keys().cloned().collect();
    du_bareme.sort();
    let champs = declarations::champs::fiche("salarie").unwrap();
    let c = champs.iter().find(|c| c.cle == "deduction_forfaitaire").unwrap();
    let mut de_la_fiche: Vec<String> = c.choix.iter().filter_map(|x| x.as_str()).filter(|x| !x.is_empty()).map(String::from).collect();
    de_la_fiche.sort();
    assert_eq!(de_la_fiche, du_bareme);
}
