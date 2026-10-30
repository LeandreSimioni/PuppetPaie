//! UNE COMPAGNIE NEUVE, DU « + » A SA PREMIERE DSN, PAR LA SEULE PAGE. Ce que le moteur decide pour elle :
//!
//! - « Pour demarrer » (`demarrage::manques`) ne demande que ce qui se remplit dans un champ que l'ecran montre : une
//!   valeur deduite se demande par celle dont elle se deduit (le SIREN et le NIC par le SIRET, la forme declaree par sa
//!   forme lisible : `demarrage::a_l_ecran`) ; la qualite du contact DSN ne se demande plus (le moteur l'ecrit :
//!   `dsn::type_du_contact`, prouve dans `tests/dsn/contact_declare.rs`) ; le code du service de sante au travail se
//!   demande par sa case des que la DPAE l'exigerait (`dpae::employeur_manques`), meme quand le SIRET manque aussi.
//!   La reponse sur la TVA (la taxe sur les salaires de la DSN de decembre) se demande d'avance.
//! - Le SIREN et le NIC se deduisent du SIRET a l'enregistrement de la fiche (`crate::routes::fiches::deriver`) : un SIRET est un
//!   SIREN de 9 chiffres suivi d'un NIC de 5.
//! - La rubrique d'une ligne de paie neuve (`etat.rubrique_par_defaut`, `catalogue::par_defaut`) : une rubrique de
//!   salaire, jamais la premiere du catalogue trie (« Absence » dans une compagnie ou rien n'est coche).
//!
//! Chaque test cree sa compagnie par `compagnies::ajouter_dans`, comme le « + » d'un logiciel bride.

use std::path::PathBuf;

use moteur::interface::compagnies::ajouter_dans;
use serde_json::{json, Value};

use crate::outils::{datee, dossier_de_test, ecrire, fiche_au, lire, reussi, route};

const JOUR: &str = "2026-11-02";

fn neuve(test: &str) -> PathBuf {
    ajouter_dans(&dossier_de_test("demarrage", test), "Les Tréteaux Neufs").unwrap().racine
}

fn manques(racine: &std::path::Path) -> Vec<Value> {
    reussi(&route(racine, "pour-demarrer", json!({}), JOUR))["manques"].as_array().unwrap().clone()
}

fn par_champ<'a>(m: &'a [Value], champ: &str) -> Vec<&'a Value> {
    m.iter().filter(|x| x["champ"] == json!(champ)).collect()
}

/// AUCUN MANQUE NE POINTE VERS UN CHAMP MASQUE, POUR AUCUNE CONVENTION (un bug qui a echappe : « Licence d'entrepreneur de spectacles —
/// bloque la DSN » en 1790, alors que la case y etait repliee). Pour chaque convention du bareme 2026 (et la compagnie qui
/// n'a pas encore dit la sienne), avec et sans permanents, avec et sans spectacle : tout ce que « Pour demarrer » demande
/// dans la fiche Entreprise est une case que l'ecran montre (`Champ::montre`, la regle que la page applique, `fiche.js`),
/// et tout champ EXIGE (`exige_si`) est montre.
///
/// La licence (`demarrage::licence_exigee`) : « est entrepreneur de spectacles vivants toute personne qui exerce une
/// activite d'exploitation de lieux de spectacles, de production ou de diffusion de spectacles » (L7122-2), et il declare
/// son activite, le recepisse valant licence (L7122-3 ; `atelier/reference/sources/code-du-travail/articles-20261023.md`).
/// Exigee de la 3090 et de la 1285 (et de la compagnie sans convention) ; en 1790 et en 3252, montree, et exigee des qu'un
/// spectacle est configure ; ni montree ni demandee dans l'audiovisuel. La DSN le prouve dans `tests/dsn/licence.rs`.
#[test]
fn aucun_manque_ne_pointe_vers_un_champ_masque_pour_aucune_convention() {
    let bareme: Value = serde_json::from_slice(&std::fs::read(socle::fichiers::dossier_du_code().join("baremes/2026.json")).unwrap()).unwrap();
    let mut conventions: Vec<String> =
        bareme["par_convention"].as_object().unwrap().keys().filter(|k| !k.starts_with('_')).cloned().collect();
    assert_eq!(conventions.len(), 10, "les dix conventions de 2026 : {:?}", conventions);
    conventions.push(String::new());
    let fiche = declarations::champs::fiche("entreprise").unwrap();
    let licence = fiche.iter().find(|c| c.cle == "licence_spectacle").unwrap();
    for idcc in &conventions {
        for (permanents, spectacle) in [(false, false), (false, true), (true, false), (true, true)] {
            let nom = format!("grille-{}-{}-{}", if idcc.is_empty() { "sans" } else { idcc }, permanents, spectacle);
            let racine = neuve(&nom);
            let mut ent = fiche_au(&lire(&racine, "config/entreprise.json"), JOUR);
            ent["idcc"] = json!(idcc);
            ent["emploie_des_permanents"] = json!(permanents);
            ecrire(&racine, "config/entreprise.json", &datee(&ent));
            if spectacle {
                ecrire(&racine, "config/spectacles.json", &json!({"266Z00910022": datee(&json!({"titre": "Le Banc", "actif": true}))}));
            }
            let m = manques(&racine);
            for x in &m {
                let Some(champ) = x["champ"].as_str() else { continue };
                if x["ou"] != json!("Configuration › Entreprise") || champ == "taux_atmp" {
                    continue; // le taux AT/MP vit dans le bareme de la compagnie (`ATMP_HTML`)
                }
                let c = fiche.iter().find(|c| c.cle == champ.split('.').next().unwrap()).unwrap_or_else(|| panic!("{} : pas un champ", champ));
                assert!(c.montre(&ent, permanents, false), "{} : « {} » demande, case masquee", nom, champ);
            }
            for c in &fiche {
                if c.exige(&ent) {
                    assert!(c.montre(&ent, permanents, false), "{} : « {} » exige et masque", nom, c.cle);
                }
            }
            let demandee = par_champ(&m, "licence_spectacle").len() == 1;
            let (montree, exigee) = (licence.montre(&ent, permanents, false), declarations::demarrage::licence_exigee(&ent, spectacle));
            assert_eq!(demandee, exigee, "{} : la licence demandee si et seulement si exigee", nom);
            assert!(!exigee || montree, "{} : la licence exigee, case masquee", nom);
            match idcc.as_str() {
                "3090" | "1285" | "" => assert!(exigee, "{}", nom),
                "1790" | "3252" => assert!(montree && exigee == spectacle, "{} : un lieu sans spectacle n'a pas a la dire", nom),
                _ => assert!(!montree && !exigee, "{} : l'audiovisuel n'a pas de licence de spectacles vivants", nom),
            }
        }
    }
}


/// UN MATRICULE, UN SALARIE (un bug qui a echappe : deux fiches au matricule « 1 » s'enregistraient, et la DSN, qui
/// rapproche chaque salarie de ses declarations par son matricule, S21.G00.30.019, prenait l'un pour l'autre). Un
/// matricule deja porte se refuse a l'enregistrement, en nommant son porteur, et rien n'est ecrit ; une fiche sans
/// matricule prend le suivant.
#[test]
fn un_matricule_deja_porte_se_refuse() {
    let racine = neuve("matricule");
    let fiche = |nom: &str, nir: &str, cle: &str, matricule: &str| {
        json!({"nom": "salarie", "cle": null, "donnees": {
            "prenom_affichage": "Loïe", "nom_affichage": nom, "nir": nir, "nir_cle": cle, "date_naissance": "1982-01-15",
            "lieu_naissance_affichage": "Nantes", "departement_naissance": "44", "pays_naissance": "FR",
            "code_nationalite_dsn": "01", "adresse_affichage": "7 rue de la Paix", "code_postal": "44000",
            "commune_affichage": "Nantes", "emploi_affichage": "Comédien", "statut": "artiste", "statut_categoriel_rc": "04",
            "pcs_ese": "354c", "code_emploi_ft": "COM010", "statut_conventionnel": "05", "code_salarie": matricule, "actif": true}})
    };
    reussi(&route(&racine, "enregistrer-fiche", fiche("Fuller", "2820144999002", "46", "1"), JOUR));
    let avant = lire(&racine, "config/salaries.json");
    let (_, phrase) = crate::outils::refus(&route(&racine, "enregistrer-fiche", fiche("Duncan", "2820144999013", "35", "1"), JOUR));
    assert!(phrase.contains("matricule 1") && phrase.contains("FULLER"), "{}", phrase);
    assert_eq!(lire(&racine, "config/salaries.json"), avant, "un refus n'ecrit rien");
    reussi(&route(&racine, "enregistrer-fiche", fiche("Duncan", "2820144999013", "35", ""), JOUR));
    let salaries = lire(&racine, "config/salaries.json");
    let matricules: Vec<String> = salaries
        .as_object()
        .unwrap()
        .iter()
        .filter(|(k, _)| !k.starts_with('_'))
        .map(|(_, s)| fiche_au(s, JOUR)["code_salarie"].as_str().unwrap_or_default().to_string())
        .collect();
    assert_eq!(matricules.len(), 2);
    assert!(matricules.contains(&"1".to_string()) && matricules.contains(&"2".to_string()), "{:?}", matricules);
}
