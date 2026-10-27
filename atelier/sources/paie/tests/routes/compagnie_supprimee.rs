//! SUPPRIMER UNE COMPAGNIE, ET GARDER AU DEBLOCAGE CELLES QU'ON A AJOUTEES EN DEMO (ticket #236, points 3 et 4 ;
//! `interface::suppression`). Les contrats : la question du deblocage ne liste que les fictives ajoutees d'un logiciel
//! debloque ; une compagnie gardee devient reelle (sous `compagnies/`, sans marqueur, son coffre garde, ses mois faits en
//! demo montres non declares) ; une compagnie qu'on ne garde pas est effacee ; une fictive livree ne se supprime pas ; une
//! compagnie reelle ne se supprime que sur le mot de passe du logiciel (code du travail, article L3243-4, `atelier/
//! reference/sources/code-du-travail/articles-20261021.md` : le double des bulletins se conserve cinq ans) -- refusee,
//! rien n'est efface.

use std::cell::Cell;
use std::path::{Path, PathBuf};

use paie::administrateur::{self, Poste};
use paie::interface::compagnies::{ajouter_dans, est_ajoutee, est_copie, est_fictive, liste_de};
use paie::interface::suppression::{self, a_garder, effacer_les_restes, genre, repondre, supprimer, Genre, FICHIER_DEMO};
use serde_json::{json, Value};

use crate::outils::{compagnie, copier, debloquer, dossier_de_test, fictive, installation, lire, reussi};

const JOUR: &str = "2026-10-22";
const ADMIN: &str = "administrateur-essai-1";

fn inst(test: &str) -> PathBuf {
    installation(&dossier_de_test("compagnie_supprimee", test), "2026.10.22")
}

fn poste(inst: &Path) -> Poste {
    Poste { dossier_cle: inst.join("cle-du-poste"), maintenant: "2026-10-22T10:00".into() }
}

/// Une fictive ajoutee sur un logiciel bride, dans laquelle on a saisi : la configuration et les saisies de la petite
/// fictive 3090 -- rien de son journal des depots : en demo, rien n'y a ete ecrit.
fn ajoutee_avec_saisies(inst: &Path, nom: &str) -> PathBuf {
    let r = ajouter_dans(inst, nom).unwrap().racine;
    for d in ["config", "engagements", "gabarits"] {
        copier(&fictive("fictive-3090-petite").join(d), &r.join(d));
    }
    // Le nom qu'on lui a donne au « + ».
    let mut e = lire(&r, "config/entreprise.json");
    e["nom_affichage"] = json!(nom);
    crate::outils::ecrire(&r, "config/entreprise.json", &e);
    assert!(est_ajoutee(&r));
    r
}

/// Une fictive livree, posee dans `fictives/` de l'installation.
fn livree(inst: &Path) -> PathBuf {
    compagnie::entiere("fictive-3090-petite", &inst.join("fictives/fictive-3090-petite"))
}

fn sans_rien(_: &Path) {}

#[test]
fn la_question_du_deblocage_ne_liste_que_les_fictives_ajoutees_d_un_logiciel_debloque() {
    let i = inst("question");
    livree(&i);
    let a = ajouter_dans(&i, "Les Pupazzi").unwrap();
    let b = ajouter_dans(&i, "Le Castelet").unwrap();
    assert!(a_garder(&i).is_empty(), "bride, la question ne se pose pas");
    debloquer(&i);
    let mut ids: Vec<String> = a_garder(&i).into_iter().map(|c| c.id).collect();
    ids.sort();
    let mut attendus = vec![a.id.clone(), b.id.clone()];
    attendus.sort();
    assert_eq!(ids, attendus, "les deux ajoutees, pas la livree");
    // Une reponse qui nomme une compagnie hors de la question, ou la meme deux fois, ne change rien.
    let p = poste(&i);
    assert!(repondre(&i, &["fictive:fictive-3090-petite".into()], &[], &p, &sans_rien).is_err());
    assert!(repondre(&i, std::slice::from_ref(&a.id), std::slice::from_ref(&a.id), &p, &sans_rien).is_err());
    assert!(a.racine.is_dir() && b.racine.is_dir());
    // Une ajoutee nommee nulle part reste, et la question revient pour elle.
    repondre(&i, std::slice::from_ref(&a.id), &[], &p, &sans_rien).unwrap();
    assert_eq!(a_garder(&i).into_iter().map(|c| c.id).collect::<Vec<_>>(), vec![b.id.clone()]);
    // Rebride puis redebloque : elle est toujours a demander.
    paie::bridage::basculer_de(&i).unwrap();
    assert!(a_garder(&i).is_empty());
    paie::bridage::basculer_de(&i).unwrap();
    assert_eq!(a_garder(&i).len(), 1);
}

#[test]
fn garder_rend_reelle_et_ne_pas_garder_efface() {
    let i = inst("garder");
    let p = poste(&i);
    let gardee = ajoutee_avec_saisies(&i, "Les Pupazzi");
    let ecartee = ajouter_dans(&i, "Essai jetable").unwrap().racine;
    // Un mot de passe d'organisme pose en demo : le coffre de la fictive ajoutee n'est scelle que par la cle du poste.
    p.coffre(&gardee).poser("urssaf", "motdepasse-invente-4711", None).unwrap();
    assert!(!p.coffre(&gardee).ouverture_exigee().unwrap());
    debloquer(&i);
    let ids = |r: &Path| format!("fictive:{}", r.file_name().unwrap().to_string_lossy());
    let lache = Cell::new(0);
    let v = repondre(&i, &[ids(&gardee)], &[ids(&ecartee)], &p, &|_| lache.set(lache.get() + 1)).unwrap();
    assert_eq!(v["erreurs"], json!([]), "{}", v);
    assert_eq!(lache.get(), 2, "la fenetre lache chacune avant qu'elle bouge");

    // Gardee : sous `compagnies/`, jamais sous `fictives/`, sans marqueur ; reelle a l'accueil, ses envois ne sont plus refuses.
    let r = i.join("compagnies/les-pupazzi");
    assert!(r.join("config/entreprise.json").is_file() && !gardee.exists());
    assert!(!est_fictive(&r) && !est_ajoutee(&r) && !est_copie(&r), "une compagnie reelle : le nombre de paies en part");
    assert!(paie::bridage::refus_d_envoi(&r).is_none());
    assert!(liste_de(&i).iter().any(|c| c.racine == r && c.reelle && c.id == "cie:les-pupazzi"));
    assert_eq!(genre(&i, &r), Ok(Genre::Reelle));
    // Elle s'ouvre comme une reelle neuve : le mot de passe du logiciel se choisit, et rescelle le coffre pose en demo.
    assert!(!p.coffre(&r).ouverture_exigee().unwrap(), "premiere ouverture : le choix du mot de passe");
    p.coffre(&r).poser_ouverture(ADMIN, None).unwrap();
    assert!(p.coffre(&r).verifier_ouverture(ADMIN).unwrap() && p.coffre(&r).pose("urssaf").unwrap(), "le coffre de la demo est garde");
    // Sa mention : les mois saisis en demo.
    let mention = lire(&r, FICHIER_DEMO);
    assert!(mention["mois"].as_array().unwrap().contains(&json!("2026-10")), "{}", mention);

    // Ecartee : effacee, sans reste.
    assert!(!ecartee.exists());
    let restes: Vec<_> = std::fs::read_dir(i.join("fictives")).unwrap().flatten().map(|e| e.file_name()).collect();
    assert!(restes.is_empty(), "{:?}", restes);
    assert!(a_garder(&i).is_empty(), "une fois repondu, il n'en reste plus");
}

/// Le cas serieux : une embauche passee, faite en demo -- sa DPAE n'est jamais partie. Gardee, la page la montre due (rien
/// n'est parti), et la mention le dit sur le mois.
#[test]
fn un_mois_fait_en_demo_puis_garde_montre_ses_dpae_dues() {
    let i = inst("dpae-due");
    let a = ajoutee_avec_saisies(&i, "Les Pupazzi");
    // En demo, la DPAE est refusee : rien n'est ecrit au journal des depots.
    let dpae = reussi(&crate::outils::route(&a, "dpae", json!({"mois": "2026-10", "paies": [{"salarie": "BRIOCHE", "debut": "2026-10-13"}]}), JOUR)).clone();
    let fichier = dpae["fichiers"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|f| f["chemin"].as_str())
        .find(|c| c.ends_with(".txt") && !c.contains("recapitulatif"))
        .unwrap()
        .to_string();
    for reel in [false, true] {
        let r = crate::outils::route(&a, "deposer", json!({"quoi": "dpae", "fichier": fichier, "reel": reel, "motdepasse": "x"}), JOUR);
        assert_eq!(r["classe"], json!(paie::bridage::CLASSE_REFUS_D_ENVOI), "{}", r);
    }
    debloquer(&i);
    repondre(&i, &[format!("fictive:{}", a.file_name().unwrap().to_string_lossy())], &[], &poste(&i), &sans_rien).unwrap();
    let r = i.join("compagnies/les-pupazzi");
    let paies = reussi(&crate::outils::route(&r, "etat-par-engagement", json!({"mois": "2026-10"}), JOUR)).clone();
    let paies = paies["engagements"].as_array().cloned().unwrap_or_else(|| paies.as_array().cloned().unwrap_or_default());
    assert!(!paies.is_empty());
    for p in &paies {
        assert_eq!(p["etats"]["dpae"]["fait"], Value::Null, "DPAE due, pas faite : {}", p);
        assert_eq!(p["etats"]["dsn"]["fait"], Value::Null, "DSN jamais deposee : {}", p);
    }
    let etat = reussi(&crate::outils::route(&r, "etat", json!({}), JOUR)).clone();
    assert!(etat["demo"]["mois"].as_array().unwrap().contains(&json!("2026-10")), "{}", etat["demo"]);
    assert_eq!(etat["supprimable"], json!(true));
    assert_eq!(etat["envois_refuses"], json!(false));
}

#[test]
fn une_fictive_livree_ne_se_supprime_pas() {
    let i = inst("livree");
    let l = livree(&i);
    let p = poste(&i);
    let appele = Cell::new(false);
    assert!(supprimer(&i, &l, None, &p, &|_| appele.set(true)).is_err());
    assert!(!appele.get() && l.join("config/entreprise.json").is_file(), "rien n'est efface");
    assert!(suppression::question(&l).is_err());
    assert_eq!(reussi(&crate::outils::route(&l, "etat", json!({}), JOUR))["supprimable"], json!(false));
    // `logiciel/` n'est pas une compagnie.
    assert!(supprimer(&i, &i.join("logiciel"), None, &p, &sans_rien).is_err());
    assert!(i.join("logiciel/VERSION").is_file());
}

#[test]
fn une_fictive_ajoutee_se_supprime_sans_mot_de_passe() {
    let i = inst("ajoutee");
    let a = ajouter_dans(&i, "Essai").unwrap().racine;
    assert_eq!(suppression::question(&a).unwrap()["mot_de_passe"], json!(false));
    assert_eq!(supprimer(&i, &a, None, &poste(&i), &sans_rien).unwrap(), "Essai");
    assert!(!a.exists() && liste_de(&i).iter().all(|c| c.racine != a));
}

#[test]
fn une_compagnie_reelle_ne_se_supprime_que_sur_le_mot_de_passe_du_logiciel() {
    let i = inst("reelle");
    debloquer(&i);
    let p = poste(&i);
    let a = ajouter_dans(&i, "Compagnie A").unwrap().racine;
    let b = ajouter_dans(&i, "Compagnie B").unwrap().racine;
    p.coffre(&a).poser_ouverture(ADMIN, None).unwrap();
    administrateur::rattacher(&i, &b, ADMIN, None, &p).unwrap();
    assert!(administrateur::est_rattachee(&i, &b));
    let q = suppression::question(&b).unwrap();
    assert_eq!((q["genre"].clone(), q["mot_de_passe"].clone()), (json!("reelle"), json!(true)), "{}", q);
    assert!(q["conservation"].as_str().unwrap().contains("L3243-4"));
    // La sauvegarde ne se propose que si un Drive est lie (son jeton est la).
    assert_eq!(q["sauvegarde"]["liee"], json!(false));
    std::fs::write(b.join("config/google-jeton.bin"), b"jeton-invente").unwrap();
    assert_eq!(suppression::question(&b).unwrap()["sauvegarde"]["liee"], json!(true));

    let appele = Cell::new(false);
    // Sans mot de passe : personne pour le taper ici, refuse ; un mauvais, refuse. Rien n'est efface.
    for m in [None, Some(""), Some("pas-le-bon-1")] {
        assert!(supprimer(&i, &b, m, &p, &|_| appele.set(true)).is_err(), "{:?}", m);
        assert!(!appele.get() && b.join("config/entreprise.json").is_file(), "{:?} : rien n'est efface", m);
        assert!(administrateur::est_rattachee(&i, &b));
    }
    // Une compagnie reelle sans mot de passe du logiciel pose : rien ne prouve qui la supprime.
    let c = ajouter_dans(&i, "Compagnie C").unwrap().racine;
    assert!(supprimer(&i, &c, Some(ADMIN), &p, &sans_rien).is_err() && c.is_dir());

    assert_eq!(supprimer(&i, &b, Some(ADMIN), &p, &|_| appele.set(true)).unwrap(), "Compagnie B");
    assert!(appele.get() && !b.exists());
    assert!(liste_de(&i).iter().all(|x| x.racine != b), "la tuile a disparu");
    assert_eq!(administrateur::rattachees(&i), vec![a.clone()], "le registre l'oublie");
    // Une compagnie neuve sous le meme dossier n'en herite rien.
    let neuve = ajouter_dans(&i, "Compagnie B").unwrap().racine;
    assert_eq!(neuve, b);
    assert!(!administrateur::est_rattachee(&i, &neuve));
}

#[test]
fn le_reste_d_une_suppression_ne_fait_pas_de_tuile_et_s_efface_au_lancement() {
    let i = inst("restes");
    debloquer(&i);
    let reste = i.join("compagnies/.supprimee-les-pupazzi-1");
    std::fs::create_dir_all(reste.join("config")).unwrap();
    std::fs::write(reste.join("config/entreprise.json"), "{}").unwrap();
    assert!(liste_de(&i).is_empty(), "un dossier cache n'est pas une tuile");
    assert_eq!(effacer_les_restes(&i), vec![reste.clone()]);
    assert!(!reste.exists());
}
