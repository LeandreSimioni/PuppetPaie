//! LA FICTIVE AJOUTEE (ticket #236, Leandre 22/10/2026) : le « + » d'un logiciel bride ajoute une compagnie fictive
//! (`compagnies::ajouter_dans`) qui porte sa marque (`compagnies::est_ajoutee`). Tout s'y fait ; ce qui partirait vers un
//! organisme ou une personne est refuse par le moteur (`bridage::refus_d_envoi`), toujours avec la meme phrase, et rien
//! n'est ecrit au journal des depots. Les memes gestes depuis une fictive livree restent joues.
//!
//! Les gestes sont joues sur `fictive-3090-petite` (copiee) : une copie garde son marqueur de fictive livree, l'autre
//! recoit celui qu'ecrit `ajouter` -- une fictive ajoutee dans laquelle on aurait deja saisi, produit et ecrit.

use std::path::{Path, PathBuf};

use paie::bridage::{refus_d_envoi, CLASSE_REFUS_D_ENVOI, REFUS_D_ENVOI};
use paie::interface::compagnies::{ajouter_dans, est_ajoutee, est_fictive, est_fictive_livree, liste_de};
use serde_json::{json, Value};

use crate::outils::{compagnie, dossier_de_test, lire, refus, reussi};

const JOUR: &str = "2026-10-22";
const FICTIVE: &str = "fictive-3090-petite";

fn route(racine: &Path, nom: &str, charge: Value) -> Value {
    crate::outils::route(racine, nom, charge, JOUR)
}

/// Le marqueur qu'ecrit `ajouter` sur un logiciel bride, recopie d'une vraie fictive ajoutee.
fn rendre_ajoutee(racine: &Path) {
    // Une installation sans rien : bridee (ticket #246).
    let inst = dossier_de_test("fictive_ajoutee", &format!("marque-{}", racine.file_name().unwrap().to_string_lossy()));
    let c = ajouter_dans(&inst, "Marque").unwrap();
    std::fs::copy(c.racine.join("COPIE-DE-DEVELOPPEMENT.json"), racine.join("COPIE-DE-DEVELOPPEMENT.json")).unwrap();
    assert!(est_ajoutee(racine));
}

fn copie(test: &str, ajoutee: bool) -> PathBuf {
    let r = compagnie::entiere(FICTIVE, &dossier_de_test("fictive_ajoutee", test));
    if ajoutee {
        rendre_ajoutee(&r);
    }
    r
}

/// Le journal des depots, octet pour octet.
fn depots(racine: &Path) -> Vec<u8> {
    std::fs::read(racine.join("journal/depots.json")).unwrap_or_default()
}

fn refuse_l_envoi(r: &Value, geste: &str) {
    let (classe, phrase) = refus(r);
    assert_eq!((classe.as_str(), phrase.as_str()), (CLASSE_REFUS_D_ENVOI, REFUS_D_ENVOI), "{} : {}", geste, r);
}

/// Les gestes d'envoi de la carte (ticket #236), sur une compagnie : (geste, route, charge).
fn gestes(r: &Path) -> Vec<(&'static str, &'static str, Value)> {
    let f = |rel: &str| json!(r.join(rel).to_string_lossy());
    let contrat = json!([{"salarie": "BRIOCHE", "debut": "2026-10-13", "numero": "BR1.26.05"}]);
    vec![
        ("DSN de test", "deposer", json!({"quoi": "dsn", "fichier": f("sorties/2025-12/DSN_Decembre2025.dsn"), "reel": false})),
        ("DSN reelle", "deposer", json!({"quoi": "dsn", "fichier": f("sorties/2025-12/DSN_Decembre2025.dsn"), "reel": true, "motdepasse": "x"})),
        ("DPAE de test", "deposer", json!({"quoi": "dpae", "fichier": f("sorties/2025-12/DPAE_Decembre2025.txt"), "reel": false})),
        ("DPAE reelle", "deposer", json!({"quoi": "dpae", "fichier": f("sorties/2025-12/DPAE_Decembre2025.txt"), "reel": true, "motdepasse": "x"})),
        ("documents du mois aux salaries", "envoyer-documents", json!({"mois": "2025-12", "motdepasse": "x"})),
        ("dossier du salarie", "dossier-envoyer", json!({"salaries": ["BRIOCHE"], "mois": "2025-12", "categories": ["bulletins"], "motdepasse": "x"})),
        ("Audiens et CASC-SVP", "casc-envoyer", json!({"vire_le": {"2025": "2026-09-01", "2026": "2026-09-02"}, "motdepasse": "x"})),
        ("contrat a la signature", "demander-signature", json!({"mois": "2026-10", "contrats": contrat, "motdepasse": "x"})),
        ("demande de fiche", "fiche-demander", json!({"adresse": "ana@exemple.invalid", "prenom": "Ana", "motdepasse": "x"})),
        ("filet de securite DPAE", "filet-activer", json!({"adresse": "ana@exemple.invalid"})),
    ]
}

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

/// LA GRILLE : chaque geste qui ferait partir quelque chose, test et reel, organismes et courriels, refuse depuis une
/// fictive ajoutee avec la phrase, sans une ligne au journal des depots ; depuis une fictive livree, le meme geste n'est
/// jamais ce refus, et les depots de test, les documents, la signature et la CASC-SVP y sont joues et notes.
#[test]
fn chaque_envoi_d_une_fictive_ajoutee_refuse_et_n_ecrit_rien_quand_la_livree_le_joue() {
    let ajoutee = copie("grille-ajoutee", true);
    let avant = depots(&ajoutee);
    for (geste, nom, charge) in gestes(&ajoutee) {
        refuse_l_envoi(&route(&ajoutee, nom, charge), geste);
        assert_eq!(depots(&ajoutee), avant, "{} : rien au journal des depots", geste);
    }
    assert!(!ajoutee.join("courriels-simules").exists(), "rien n'est joue non plus");
    assert!(!ajoutee.join("config/filet-dpae.json").exists() || lire(&ajoutee, "config/filet-dpae.json").get("compte").is_none());

    let livree = copie("grille-livree", false);
    let joues = ["DSN de test", "DPAE de test", "documents du mois aux salaries", "contrat a la signature", "Audiens et CASC-SVP"];
    for (geste, nom, charge) in gestes(&livree) {
        let avant = depots(&livree);
        let r = route(&livree, nom, charge);
        assert_ne!(r["classe"], json!(CLASSE_REFUS_D_ENVOI), "{} : {}", geste, r);
        if joues.contains(&geste) {
            reussi(&r);
            assert_ne!(depots(&livree), avant, "{} : joue, et note", geste);
        }
    }
    assert!(livree.join("courriels-simules").is_dir());
}

/// Un contrat parti a la signature avant que la compagnie soit ajoutee (une fictive copiee) : l'avenant, la relance, et
/// le contrat contresigne que la releve renverrait refusent comme le reste.
#[test]
fn l_avenant_la_relance_et_le_contresigne_d_une_fictive_ajoutee_ne_partent_pas() {
    let r = copie("contresigne", false);
    reussi(&route(&r, "demander-signature", json!({"mois": "2026-10", "contrats": [{"salarie": "BRIOCHE", "debut": "2026-10-13", "numero": "BR1.26.05"}], "motdepasse": ""})));
    // Le lieu change : un avenant est du.
    let mut s = lire(&r, "paies/2026-10.json");
    s["engagements"][0]["lignes"][0]["lieu"] = json!("Theatre de la Ville");
    crate::outils::ecrire(&r, "paies/2026-10.json", &s);
    rendre_ajoutee(&r);
    let avant = depots(&r);
    refuse_l_envoi(&route(&r, "avenant-envoyer", json!({"salarie": "BRIOCHE", "debut": "2026-10-13", "motdepasse": "x"})), "avenant");
    refuse_l_envoi(&route(&r, "avenants-envoyer", json!({"avenants": [{"salarie": "BRIOCHE", "debut": "2026-10-13"}], "motdepasse": "x"})), "avenants");
    refuse_l_envoi(
        &route(&r, "demander-signature", json!({"mois": "2026-10", "contrats": [{"salarie": "BRIOCHE", "debut": "2026-10-13", "numero": "BR1.26.05"}], "motdepasse": "x", "relance": true})),
        "relance",
    );
    let releve = reussi(&route(&r, "signatures-relever", json!({}))).clone();
    assert!(releve.to_string().contains(REFUS_D_ENVOI), "la releve le dit : {}", releve);
    assert_eq!(depots(&r), avant);
}

/// Un signalement (ici de fin de contrat) s'ecrit dans une fictive ajoutee -- rien ne part -- et son depot, test ou reel,
/// refuse comme une mensuelle.
#[test]
fn le_signalement_s_ecrit_et_son_depot_refuse() {
    let r = compagnie::ses_dsn("fictive-3090-grande", &[], &dossier_de_test("fictive_ajoutee", "signalement"));
    let mut saisie = lire(&r, "paies/2026-05.json");
    let e = saisie["engagements"].as_array_mut().unwrap().iter_mut().find(|e| e["numero"] == "BI4.26.05").unwrap();
    e["motif_rupture"] = json!("084");
    e["preavis"] = json!({"type": "90"});
    crate::outils::ecrire(&r, "paies/2026-05.json", &saisie);
    rendre_ajoutee(&r);
    let avant = depots(&r);
    let ecrit = reussi(&route(&r, "fin-contrat-signaler", json!({"mois": "2026-05", "salarie": "Biancolelli", "test": true}))).clone();
    let fichier = r.join("sorties/2026-05").join(ecrit["nom"].as_str().unwrap());
    assert!(fichier.is_file(), "{}", ecrit);
    for reel in [false, true] {
        refuse_l_envoi(&route(&r, "deposer", json!({"quoi": "dsn", "fichier": fichier.to_string_lossy(), "reel": reel, "motdepasse": "x"})), "signalement");
    }
    assert_eq!(depots(&r), avant);
}

/// Les automatismes : l'ouverture (et la veille, qui la rejoue) ne depose ni n'envoie rien depuis une fictive ajoutee --
/// ni horloge de fictive, ni test automatique de DPAE -- ; ses mots de passe se posent au coffre.
#[test]
fn l_ouverture_n_envoie_rien_et_le_coffre_se_pose() {
    let r = copie("ouverture", true);
    let avant = depots(&r);
    reussi(&route(&r, "ouverture", json!({"qui": "veille"})));
    assert_eq!(depots(&r), avant, "rien au journal des depots");
    let etat = reussi(&route(&r, "coffre-poser", json!({"service": "urssaf", "valeur": "motdepasse-invente-4711"}))).clone();
    let urssaf = etat["services"].as_array().unwrap().iter().find(|s| s["service"] == json!("urssaf")).unwrap();
    assert_eq!(urssaf["pose"], json!(true), "{}", etat);
    // Et la page sait qu'un envoi n'y demande pas de mot de passe.
    assert_eq!(reussi(&route(&r, "etat", json!({})))["envois_refuses"], json!(true));
    assert_eq!(reussi(&route(&copie("configuration-livree", false), "etat", json!({})))["envois_refuses"], json!(false));
}

/// Le nombre de paies annuel ne part d'aucune fictive : la fictive ajoutee en est une.
#[test]
fn le_nombre_de_paies_ne_part_pas_d_une_fictive_ajoutee() {
    let r = copie("nombre-de-paies", true);
    let o = std::collections::HashMap::from([("racine".to_string(), r.display().to_string())]);
    let ctx = droit::contexte::Contexte::nouveau(&r, Some(socle::date::Jour::lire("2027-01-20").unwrap())).unwrap();
    let bilan = paie::nombre_de_paies::envoyer_si_du(&o, &ctx);
    assert_eq!((bilan["envoye"].clone(), bilan["raison"].clone()), (json!(false), json!("compagnie fictive")), "{}", bilan);
}
