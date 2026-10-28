//! LA FICTIVE AJOUTEE (ticket #236, Leandre 22/10/2026) : le « + » d'un logiciel bride ajoute une compagnie fictive
//! (`compagnies::ajouter_dans`) qui porte sa marque (`compagnies::est_ajoutee`). Tout s'y fait ; ce qui partirait vers un
//! organisme ou une personne est refuse par le moteur (`bridage::refus_d_envoi`), toujours avec la meme phrase, et rien
//! n'est ecrit au journal des depots. Les memes gestes depuis une fictive livree restent joues.

use paie::bridage::{refus_d_envoi, REFUS_D_ENVOI};
use paie::interface::compagnies::{ajouter_dans, est_ajoutee, est_fictive, est_fictive_livree, liste_de};
use serde_json::{json, Value};

use crate::outils::{dossier_de_test, lire};








#[test]
fn le_plus_d_un_logiciel_bride_ajoute_une_fictive_ajoutee() {
    let inst = dossier_de_test("fictive_ajoutee", "ajouter");
    assert!(paie::bridage::est_bride_de(&inst));
    let c = ajouter_dans(&inst, "  Les   Pupazzi ").unwrap();
    assert_eq!(c.racine, inst.join("fictives/ajoutee-les-pupazzi"));
    assert_eq!((c.id.as_str(), c.reelle, c.nom.as_str()), ("fictive:ajoutee-les-pupazzi", false, "Les Pupazzi"));
    assert!(est_fictive(&c.racine) && est_ajoutee(&c.racine) && !est_fictive_livree(&c.racine));
    assert!(!paie::routes_accueil::simulation_active(&c.racine), "ses envois ne sont pas joues");
    assert_eq!(refus_d_envoi(&c.racine), Some(REFUS_D_ENVOI));
    // L'accueil bride la propose, parmi les fictives ; rien n'est ecrit sous `compagnies/`.
    assert!(liste_de(&inst).iter().any(|x| x.id == c.id && !x.reelle));
    assert!(!inst.join("compagnies").exists());
    // Le nom d'une fictive livree ne se prend pas.
    let homonyme = ajouter_dans(&inst, "Fictive 3090 petite").unwrap();
    assert_eq!(homonyme.racine, inst.join("fictives/ajoutee-fictive-3090-petite"));
    // Son marqueur retire, un dossier de `fictives/` reste refuse : rien n'en part, rien n'en est joue.
    std::fs::remove_file(homonyme.racine.join("COPIE-DE-DEVELOPPEMENT.json")).unwrap();
    assert!(!est_fictive(&homonyme.racine) && refus_d_envoi(&homonyme.racine).is_some());
    assert!(!paie::routes_accueil::simulation_active(&homonyme.racine));
    assert_eq!(ajouter_dans(&inst, "Les Pupazzi").unwrap().racine, inst.join("fictives/ajoutee-les-pupazzi-2"));
    // Pas de photo d'origine (donc pas de remise a zero).
    paie::interface::compagnies::figer_origine(&c.racine).unwrap();
    assert!(!inst.join("fictives/.origine").exists());
    // Debloque, le meme geste ajoute une compagnie reelle, qui envoie.
    crate::outils::debloquer(&inst);
    let reelle = ajouter_dans(&inst, "Les Pupazzi").unwrap();
    assert_eq!((reelle.racine.clone(), reelle.reelle), (inst.join("compagnies/les-pupazzi"), true));
    assert!(!est_fictive(&reelle.racine) && refus_d_envoi(&reelle.racine).is_none());
}

/// LA FICHE NEE DU « + » EST DANS LA FORME QUE LE MOTEUR RELIT (#236) : la raison sociale (S10.G00.01.003) en
/// majuscules sans accents, deduite du nom comme a chaque enregistrement. `to_uppercase` gardait les accents : une
/// compagnie neuve naissait avec « 1 erreur de configuration » qu'aucune case ne montrait. Il ne lui manque que ce qui
/// est a saisir (« obligatoire »), aucune valeur n'y est fausse.
#[test]
fn le_plus_ecrit_la_raison_sociale_dans_sa_forme_declaree() {
    let inst = dossier_de_test("fictive_ajoutee", "raison-sociale");
    let c = ajouter_dans(&inst, "Les Tréteaux de l'Été").unwrap();
    let ent = lire(&c.racine, "config/entreprise.json");
    assert_eq!(ent["nom_affichage"], json!("Les Tréteaux de l'Été"));
    assert_eq!(ent["raison_sociale"], json!("LES TRETEAUX DE L'ETE"));
    let fautes: Vec<Value> = paie::champs::verifier_fiche("entreprise", ent.as_object().unwrap())
        .unwrap()
        .into_iter()
        .filter(|a| a["gravite"] == json!("erreur") && a["message"] != json!("obligatoire"))
        .collect();
    assert!(fautes.is_empty(), "{:?}", fautes);
}





