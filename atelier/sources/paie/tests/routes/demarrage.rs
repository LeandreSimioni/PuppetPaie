//! UNE COMPAGNIE NEUVE, DU « + » A SA PREMIERE DSN, PAR LA SEULE PAGE (#236). Ce que le moteur decide pour elle :
//!
//! - « Pour demarrer » (`demarrage::manques`) ne demande que ce qui se remplit dans un champ que l'ecran montre : une
//!   valeur deduite se demande par celle dont elle se deduit (le SIREN et le NIC par le SIRET, la forme declaree par sa
//!   forme lisible : `demarrage::a_l_ecran`) ; la qualite du contact DSN ne se demande plus (le moteur l'ecrit :
//!   `dsn::type_du_contact`, prouve dans `tests/dsn/contact_declare.rs`) ; le code du service de sante au travail se
//!   demande par sa case des que la DPAE l'exigerait (`dpae::employeur_manques`), meme quand le SIRET manque aussi.
//!   La reponse sur la TVA (la taxe sur les salaires de la DSN de decembre) se demande d'avance.
//! - Le SIREN et le NIC se deduisent du SIRET a l'enregistrement de la fiche (`routes_fiches::deriver`) : un SIRET est un
//!   SIREN de 9 chiffres suivi d'un NIC de 5.
//! - La rubrique d'une ligne de paie neuve (`etat.rubrique_par_defaut`, `catalogue::par_defaut`) : une rubrique de
//!   salaire, jamais la premiere du catalogue trie (« Absence » dans une compagnie ou rien n'est coche).
//!
//! Chaque test cree sa compagnie par `compagnies::ajouter_dans`, comme le « + » d'un logiciel bride.

use std::path::PathBuf;

use paie::interface::compagnies::ajouter_dans;
use serde_json::{json, Value};

use crate::outils::{dossier_de_test, ecrire, lire, reussi, route};

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

#[test]
fn chaque_chose_que_demande_une_compagnie_neuve_se_remplit_dans_un_champ_montre() {
    let racine = neuve("champs-montres");
    let m = manques(&racine);
    assert!(!m.is_empty());
    let fiche = paie::champs::fiche("entreprise").unwrap();
    for x in &m {
        let Some(champ) = x["champ"].as_str() else {
            // Ce qui n'est pas une case mene a son onglet.
            assert!(x["ou"] != json!("Configuration › Entreprise"), "un manque de la fiche sans sa case : {}", x);
            continue;
        };
        if champ == "taux_atmp" {
            continue; // il vit dans le bareme de la compagnie, sa case est en tete des numeros (`ATMP_HTML`)
        }
        let c = fiche.iter().find(|c| c.cle == champ).unwrap_or_else(|| panic!("{} : pas un champ de la fiche", champ));
        assert!(c.ecran && !c.calcule(), "{} : un champ que l'ecran ne montre pas", champ);
        assert!(!["siren", "nic", "congesspectacles_affiliation"].contains(&champ), "{} se deduit d'un autre champ", champ);
    }
    assert!(m.iter().all(|x| !x["cle"].as_str().unwrap_or("").ends_with("qualite")), "la qualite du contact ne se demande plus");
    assert_eq!(par_champ(&m, "siret").len(), 1, "SIRET, SIREN et NIC font une ligne, celle du SIRET");
    let mt = par_champ(&m, "medecine_travail");
    assert_eq!(mt.len(), 1, "le code du service de sante au travail, par sa case, meme sans SIRET : {:?}", m);
    assert_eq!(mt[0]["bloque"], json!("la DPAE"));
    // La DSN de decembre declare la taxe sur les salaires (bloc S21.G00.44) et ne se fait pas sans reponse : demandee
    // d'avance, par sa case (`assujettissement::regime_taxe_sur_les_salaires`).
    assert_eq!(par_champ(&m, "taxe_sur_les_salaires").len(), 1, "{:?}", m);
}

#[test]
fn le_siret_enregistre_donne_le_siren_et_le_nic_et_la_dpae_demande_encore_sa_medecine() {
    let racine = neuve("siret");
    let fiche = json!({"nom_affichage": "Les Tréteaux Neufs", "siret": "32520440200011", "taxe_sur_les_salaires": {"tva": "exoneree", "sous_franchise": true}});
    reussi(&route(&racine, "enregistrer-fiche", json!({"nom": "entreprise", "cle": null, "donnees": fiche, "forcer": true}), JOUR));
    let ent = lire(&racine, "config/entreprise.json");
    assert_eq!((ent["siren"].as_str(), ent["nic"].as_str()), (Some("325204402"), Some("00011")));
    let m = manques(&racine);
    assert!(par_champ(&m, "siret").is_empty(), "{:?}", m);
    assert_eq!(par_champ(&m, "medecine_travail").len(), 1);
    assert!(par_champ(&m, "taxe_sur_les_salaires").is_empty(), "exoneree de TVA et sous les plafonds : la reponse est donnee");
    // Le SIRET change : le SIREN et le NIC le suivent.
    let fiche = json!({"nom_affichage": "Les Tréteaux Neufs", "siret": "32520440200029"});
    reussi(&route(&racine, "enregistrer-fiche", json!({"nom": "entreprise", "cle": null, "donnees": fiche, "forcer": true}), JOUR));
    assert_eq!(lire(&racine, "config/entreprise.json")["nic"], json!("00029"));
}

#[test]
fn une_ligne_de_paie_neuve_part_d_une_rubrique_de_salaire() {
    let racine = neuve("rubrique");
    let etat = |r: &std::path::Path| reussi(&route(r, "etat", json!({}), JOUR))["rubrique_par_defaut"].clone();
    // Rien de coche : le catalogue est trie par libelle, « Absence » en tete ; la ligne neuve n'est pas une absence.
    let premiere = reussi(&route(&racine, "etat", json!({}), JOUR))["rubriques"][0]["code"].clone();
    assert_eq!(premiere, json!("absence"), "le cas qui trompait la page");
    assert_eq!(etat(&racine), json!({"activite": "cachet", "mensuelle": "mois"}));
    // La compagnie paie surtout a l'heure : sa ligne neuve aussi ; au mois, le salaire de base reste devant.
    ecrire(&racine, "config/rubriques-actives.json", &json!({"actives": ["cachet", "heure"], "usages": {"heure": 7, "cachet": 2}}));
    assert_eq!(etat(&racine), json!({"activite": "heure", "mensuelle": "mois"}));
}
