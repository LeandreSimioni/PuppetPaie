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
        assert!(!["siren", "nic"].contains(&champ), "{} se deduit d'un autre champ", champ);
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

/// UN SEUL NUMERO CONGES SPECTACLES (#236) : la compagnie ne saisit que son numero d'adherent, « 10 chiffres + 1 lettre »,
/// dont la DSN tire S21.G00.40.022 (« 97 puis le numero d'adherent », Audiens, fiche de specificites, `atelier/reference/
/// sources/audiens-dsn/audiens-fiche-specificites-conges-spectacles-20261023.txt`, l. 80-81 ; la deduction est prouvee
/// dans `tests/calcul/caisse_conges_affiliation.rs`). Un numero mal forme refuse et rien n'est ecrit ; « Pour demarrer »
/// d'une compagnie qui a des spectacles ne demande que ce numero.
#[test]
fn le_seul_numero_conges_spectacles_saisi_est_le_numero_d_adherent() {
    let racine = neuve("conges-spectacles");
    ecrire(&racine, "config/spectacles.json", &json!({"266Z00910022": {"titre": "Le Banc", "actif": true}}));
    let m = manques(&racine);
    assert_eq!(par_champ(&m, "congesspectacles_adherent").len(), 1, "{:?}", m);
    assert!(m.iter().all(|x| !x.to_string().contains("congesspectacles_affiliation")), "un seul numero demande : {:?}", m);

    let poser = |adherent: &str| {
        let fiche = json!({"nom_affichage": "Les Tréteaux Neufs", "congesspectacles_adherent": adherent});
        route(&racine, "enregistrer-fiche", json!({"nom": "entreprise", "cle": null, "donnees": fiche, "forcer": false}), JOUR)
    };
    let fautif = |r: &Value| r["anomalies"].as_array().unwrap().iter().any(|a| a["champ"] == json!("congesspectacles_adherent"));
    for mal_forme in ["970123450001F", "0123450001", "01234500011F", "A123450001F", "0123450001É"] {
        let r = poser(mal_forme);
        assert_eq!(r["ok"], json!(false));
        assert!(fautif(&r), "{} : pas dix chiffres et une lettre : {}", mal_forme, r);
        assert!(lire(&racine, "config/entreprise.json").get("congesspectacles_adherent").is_none(), "rien n'est ecrit");
    }
    assert!(!fautif(&poser("0123450001F")), "dix chiffres et une lettre : rien a redire");
    // La meme valeur ecrite avec des blancs, un tiret ou la lettre en minuscule : le moteur l'ecrit dans sa forme
    // (`conges_spectacles::forme_adherent`), sans rien reprocher.
    assert!(!fautif(&poser("0123 450001-f")), "la meme valeur : rien a redire");
    let fiche = json!({"nom_affichage": "Les Tréteaux Neufs", "congesspectacles_adherent": "0123 450001-f"});
    reussi(&route(&racine, "enregistrer-fiche", json!({"nom": "entreprise", "cle": null, "donnees": fiche, "forcer": true}), JOUR));
    let ent = lire(&racine, "config/entreprise.json");
    assert_eq!(ent["congesspectacles_adherent"], json!("0123450001F"));
    assert!(ent.get("congesspectacles_affiliation").is_none(), "rien d'autre ne s'ecrit : {}", ent);
    assert!(par_champ(&manques(&racine), "congesspectacles_adherent").is_empty());
}

/// AUCUN MANQUE NE POINTE VERS UN CHAMP MASQUE, POUR AUCUNE CONVENTION (#236 : « Licence d'entrepreneur de spectacles —
/// bloque la DSN » en 1790, alors que la case y etait repliee). Pour chaque convention du bareme 2026 (et la compagnie qui
/// n'a pas encore dit la sienne), avec et sans permanents, avec un spectacle : tout ce que « Pour demarrer » demande dans
/// la fiche Entreprise est une case que l'ecran montre (`Champ::montre`, la regle que la page applique, `fiche.js`), et
/// tout champ EXIGE (`exige_si`) est montre.
///
/// La licence : exigee de la 3090 et de la 1285 (et de la compagnie sans convention), montree sans etre exigee en 1790 et
/// en 3252, ni montree ni demandee dans l'audiovisuel. Ce que la norme DSN en dit (S21.G00.40.049, cahier technique
/// 2027.1.1, `atelier/reference/norme-dsn/texte-2027/bloc-S21.G00.40.txt` l. 1418-1426) : obligatoire pour la structure
/// qui produit, diffuse ou exploite un lieu de spectacle ; la DSN le prouve dans `tests/dsn/licence.rs`.
#[test]
fn aucun_manque_ne_pointe_vers_un_champ_masque_pour_aucune_convention() {
    let bareme: Value = serde_json::from_slice(&std::fs::read(paie::fichiers::dossier_du_code().join("baremes/2026.json")).unwrap()).unwrap();
    let mut conventions: Vec<String> =
        bareme["par_convention"].as_object().unwrap().keys().filter(|k| !k.starts_with('_')).cloned().collect();
    assert_eq!(conventions.len(), 10, "les dix conventions de 2026 : {:?}", conventions);
    conventions.push(String::new());
    let fiche = paie::champs::fiche("entreprise").unwrap();
    let licence = fiche.iter().find(|c| c.cle == "licence_spectacle").unwrap();
    for idcc in &conventions {
        for permanents in [false, true] {
            let racine = neuve(&format!("grille-{}-{}", if idcc.is_empty() { "sans" } else { idcc }, permanents));
            let mut ent = lire(&racine, "config/entreprise.json");
            ent["idcc"] = json!(idcc);
            ent["emploie_des_permanents"] = json!(permanents);
            ecrire(&racine, "config/entreprise.json", &ent);
            ecrire(&racine, "config/spectacles.json", &json!({"266Z00910022": {"titre": "Le Banc", "actif": true}}));
            for x in manques(&racine) {
                let Some(champ) = x["champ"].as_str() else { continue };
                if x["ou"] != json!("Configuration › Entreprise") || champ == "taux_atmp" {
                    continue; // le taux AT/MP vit dans le bareme de la compagnie (`ATMP_HTML`)
                }
                let c = fiche.iter().find(|c| c.cle == champ.split('.').next().unwrap()).unwrap_or_else(|| panic!("{} : pas un champ", champ));
                assert!(c.montre(&ent, permanents, false), "convention « {} », permanents {} : « {} » demande, case masquee", idcc, permanents, champ);
            }
            for c in &fiche {
                if c.exige(&ent) {
                    assert!(c.montre(&ent, permanents, false), "« {} » exige en « {} » et masque", c.cle, idcc);
                }
            }
            let demandee = par_champ(&manques(&racine), "licence_spectacle").len() == 1;
            let (montree, exigee) = (licence.montre(&ent, permanents, false), licence.exige(&ent));
            assert_eq!(demandee, exigee, "« {} » : la licence demandee si et seulement si exigee", idcc);
            match idcc.as_str() {
                "3090" | "1285" | "" => assert!(exigee && montree, "{}", idcc),
                "1790" | "3252" => assert!(montree && !exigee, "{}", idcc),
                _ => assert!(!montree && !exigee, "{} : l'audiovisuel n'a pas de licence de spectacles vivants", idcc),
            }
        }
    }
}
