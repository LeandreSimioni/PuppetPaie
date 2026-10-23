//! L'etat garde d'un mois et sa file (`etat_garde::FileDesEtats`, ticket #212 : « quand je change de mois c'est pas
//! instantane »). Les contrats : un etat perime se sert tout de suite, marque comme tel, et se refait ; ce que l'ecran
//! d'un mois ne lit pas (`HORS_DE_L_ECRAN`) ne le perime plus ; une ecriture de saisie le perime toujours ; la file,
//! apres une ecriture, rend justes les etats de tous les mois suivis. Sur `fictive-3090-petite` (config, aout et
//! septembre 2026), calculs au 03/10/2026, dans ce processus (la file est celle de la fenetre).

use crate::outils::{compagnie, dossier_de_test, reussi, route_en_processus};
use paie::etat_garde::{servir, FileDesEtats};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

const JOUR: &str = "2026-10-03";
const MOIS: [&str; 2] = ["2026-08", "2026-09"];

fn compagnie(nom: &str) -> PathBuf {
    compagnie::saisies("fictive-3090-petite", &MOIS, &dossier_de_test("etat_garde", nom).join("compagnie"))
}

/// Un fichier dont la date de modification avance (ce que fait toute ecriture), contenu garde ou cree.
fn toucher(racine: &Path, rel: &str) {
    let p = racine.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    if !p.exists() {
        std::fs::write(&p, b"{}").unwrap();
    }
    let f = std::fs::OpenOptions::new().append(true).open(&p).unwrap();
    f.set_modified(SystemTime::now() + Duration::from_secs(3)).unwrap();
}

/// L'etat garde de `mois` est-il juste ? Au jour d'un `Contexte` (`aujourdhui`, que la date des calculs ne regle pas).
fn juste(racine: &Path, mois: &str) -> bool {
    let jour = paie::contexte::aujourdhui_sans_note().unwrap().iso();
    servir(racine, mois, &jour).map(|(_, j)| j) == Some(true)
}

/// Les etats de tous les mois suivis, frais : la file refait ce qui manque.
fn tout_frais(f: &FileDesEtats, racine: &Path) {
    for m in MOIS.iter().chain(["2026-10"].iter()) {
        f.attendre(m).unwrap_or_else(|e| panic!("{} : {:?}", m, e.message));
        assert!(juste(racine, m), "{} : l'etat refait est juste", m);
    }
}

#[test]
fn un_etat_perime_se_sert_marque_et_se_refait() {
    let racine = compagnie("perime");
    let f: Arc<FileDesEtats> = FileDesEtats::nouvelle(&racine, Some(JOUR));
    // Aucun etat garde : l'ecran attend l'etat frais.
    let premier = f.ecran("2026-09", false).unwrap_or_else(|e| panic!("{}", e.message));
    assert_eq!(premier["perime"], json!(false), "un mois jamais calcule se calcule");
    assert!(juste(&racine, "2026-09"));
    // La saisie du mois change : l'ecran rend l'etat d'avant, TOUT DE SUITE, marque perime, et le fait refaire.
    toucher(&racine, "paies/2026-09.json");
    assert!(!juste(&racine, "2026-09"), "une ecriture de saisie perime son mois");
    let perime = f.ecran("2026-09", false).unwrap_or_else(|e| panic!("{}", e.message));
    assert_eq!(perime["perime"], json!(true), "{}", perime["perime"]);
    assert_eq!(perime["lectures"], premier["lectures"], "c'est le dernier etat connu qui se sert");
    // L'etat frais : refait par la file, juste, et non marque.
    let frais = f.ecran("2026-09", true).unwrap_or_else(|e| panic!("{}", e.message));
    assert_eq!(frais["perime"], json!(false));
    assert!(juste(&racine, "2026-09"), "la file a refait l'etat");
    // Le meme ecran que la route `mois` de `paie route`.
    let route = route_en_processus(&racine, "mois", json!({"mois": "2026-09"}), JOUR);
    assert_eq!(reussi(&route)["lectures"], frais["lectures"]);
    f.arreter();
}

#[test]
fn ce_que_l_ecran_ne_lit_pas_ne_perime_pas() {
    let racine = compagnie("hors-ecran");
    let f = FileDesEtats::nouvelle(&racine, Some(JOUR));
    tout_frais(&f, &racine);
    // La trace de la veille, l'index de la sauvegarde, l'identite du poste, l'horloge d'une fictive, le filet, les
    // signalements, les incidents : chacun reecrit sans que l'ecran d'un mois le lise.
    for rel in [
        "config/.ouverture.json",
        "config/.sauvegarde.json",
        "config/diagnostic.json",
        "config/horloge.json",
        "config/filet-dpae.json",
        "config/.filet.verrou",
        "journal/signalements.json",
        "journal/incidents.json",
    ] {
        toucher(&racine, rel);
        for m in MOIS {
            assert!(juste(&racine, m), "{} ne perime pas {}", rel, m);
        }
    }
    // Ce que l'ecran lit le perime toujours : une fiche, le journal des depots, la saisie d'un AUTRE mois.
    for rel in ["config/salaries.json", "journal/depots.json", "paies/2026-08.json"] {
        tout_frais(&f, &racine);
        toucher(&racine, rel);
        assert!(!juste(&racine, "2026-09"), "{} perime 2026-09", rel);
    }
    f.arreter();
}

#[test]
fn apres_une_ecriture_la_file_rend_tous_les_etats_justes() {
    let racine = compagnie("ecriture");
    let f = FileDesEtats::nouvelle(&racine, Some(JOUR));
    tout_frais(&f, &racine);
    // Une ecriture du moteur : la saisie d'aout, enregistree telle qu'elle est (le mois est passe : `forcer`).
    let saisie: Value = serde_json::from_slice(&std::fs::read(racine.join("paies/2026-08.json")).unwrap()).unwrap();
    let r = route_en_processus(&racine, "enregistrer-saisie", json!({"mois": "2026-08", "saisie": saisie, "forcer": true}), JOUR);
    reussi(&r);
    toucher(&racine, "paies/2026-08.json");
    assert!(!juste(&racine, "2026-08") && !juste(&racine, "2026-09"), "l'ecriture perime les mois");
    // La fenetre, apres l'ecriture : tous les mois suivis se verifient, en fond, et finissent justes.
    f.tout_verifier();
    let debut = Instant::now();
    while !MOIS.iter().all(|m| juste(&racine, m)) {
        assert!(debut.elapsed() < Duration::from_secs(120), "la file n'a pas refait les etats");
        std::thread::sleep(Duration::from_millis(50));
    }
    f.arreter();
}

// ------------------------------------------------------------------ l'ouverture (ticket #244)

/// L'etat garde de l'ouverture est-il juste ?
fn ouverture_juste(racine: &Path) -> bool {
    juste(racine, paie::etat_garde::OUVERTURE)
}

/// L'ouverture, d'un coup, telle que la fenetre la sert (`FileDesEtats::ouverture`).
fn ouvrir(f: &FileDesEtats, patienter: u64, frais: bool) -> Value {
    f.ouverture(Duration::from_millis(patienter), frais).unwrap_or_else(|e| panic!("{}", e.message))
}

#[test]
fn l_ouverture_sans_etat_connu_se_calcule_et_dit_ce_qu_elle_fait() {
    let racine = compagnie("ouverture-neuve");
    let f = FileDesEtats::nouvelle(&racine, Some(JOUR));
    // Rien de garde : la fenetre ne fait pas attendre sans rien dire -- au bout du delai, la phrase du moteur.
    let attente = ouvrir(&f, 0, false);
    assert_eq!(attente["pret"], json!(false), "{}", attente);
    assert!(attente["phrase"].as_str().is_some_and(|p| !p.is_empty()), "la phrase dit ce que fait le moteur : {}", attente);
    // Le temps qu'il faut : tout est la, juste, et ecrit.
    let o = ouvrir(&f, 120_000, false);
    assert_eq!(o["pret"], json!(true), "{}", o);
    assert_eq!(o["perime"], json!(false));
    assert_eq!(o["ecran"]["perime"], json!(false));
    assert!(ouverture_juste(&racine), "l'etat de l'ouverture est ecrit, et juste");
    let mois = o["mois"].as_str().unwrap().to_string();
    assert!(juste(&racine, &mois), "l'ecran du mois a ouvrir est ecrit, et juste");
    // Les memes lectures que les routes de `paie route`.
    for route in ["etat", "mois-a-ouvrir", "echeancier", "rappels", "pour-demarrer"] {
        let r = route_en_processus(&racine, route, json!({}), JOUR);
        assert_eq!(&o["lectures"][route]["resultat"], reussi(&r), "{}", route);
    }
    assert_eq!(o["mois"], reussi(&route_en_processus(&racine, "mois-a-ouvrir", json!({}), JOUR))["mois"]);
    let ecran = route_en_processus(&racine, "mois", json!({"mois": mois}), JOUR);
    assert_eq!(o["ecran"]["lectures"], reussi(&ecran)["lectures"]);
    f.arreter();
}

/// L'etat garde de l'ouverture, marque a la main (ce qu'aucun calcul n'ecrirait) : ce qui se sert ensuite est-il le garde ?
fn marquer_l_ouverture(racine: &Path) {
    let p = paie::etat_garde::chemin(racine, paie::etat_garde::OUVERTURE);
    let mut v: Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
    v["lectures"]["marque-du-test"] = json!({"ok": true, "resultat": "garde"});
    std::fs::write(&p, v.to_string()).unwrap();
}

#[test]
fn l_ouverture_juste_se_sert_sans_recalcul_perimee_se_sert_marquee_et_se_refait() {
    let racine = compagnie("ouverture-gardee");
    let f = FileDesEtats::nouvelle(&racine, Some(JOUR));
    ouvrir(&f, 120_000, false);
    assert!(ouverture_juste(&racine));
    // JUSTE : servie telle qu'elle est gardee, sans rien refaire (la marque du test est la).
    marquer_l_ouverture(&racine);
    assert!(ouverture_juste(&racine), "le fichier garde n'est pas dans son empreinte");
    let o = ouvrir(&f, 0, false);
    assert_eq!(o["pret"], json!(true));
    assert_eq!(o["perime"], json!(false));
    assert_eq!(o["lectures"]["marque-du-test"]["resultat"], json!("garde"), "servie sans recalcul");
    // UNE FICHE CHANGE : le dernier etat connu, TOUT DE SUITE, marque perime ; puis l'etat frais, refait par la file.
    toucher(&racine, "config/salaries.json");
    assert!(!ouverture_juste(&racine), "une fiche perime l'ouverture");
    let debut = Instant::now();
    let perime = ouvrir(&f, 0, false);
    assert!(debut.elapsed() < Duration::from_secs(2), "le perime se sert sans attendre le calcul");
    assert_eq!(perime["pret"], json!(true));
    assert_eq!(perime["perime"], json!(true), "{}", perime["perime"]);
    assert_eq!(perime["lectures"]["marque-du-test"]["resultat"], json!("garde"), "c'est le dernier etat connu qui se sert");
    let frais = ouvrir(&f, 0, true);
    assert_eq!(frais["perime"], json!(false));
    assert!(frais["lectures"].get("marque-du-test").is_none(), "l'etat frais est refait");
    assert!(ouverture_juste(&racine), "la file a refait l'etat de l'ouverture");
    // Ce que l'ouverture ne garde pas (les signalements : lus a cote) ne la perime pas.
    toucher(&racine, "journal/signalements.json");
    assert!(ouverture_juste(&racine));
    f.arreter();
}

#[test]
fn une_ecriture_perime_l_ouverture_et_la_file_la_refait() {
    let racine = compagnie("ouverture-ecriture");
    let f = FileDesEtats::nouvelle(&racine, Some(JOUR));
    ouvrir(&f, 120_000, false);
    assert!(ouverture_juste(&racine));
    let saisie: Value = serde_json::from_slice(&std::fs::read(racine.join("paies/2026-08.json")).unwrap()).unwrap();
    reussi(&route_en_processus(&racine, "enregistrer-saisie", json!({"mois": "2026-08", "saisie": saisie, "forcer": true}), JOUR));
    toucher(&racine, "paies/2026-08.json");
    assert!(!ouverture_juste(&racine), "une ecriture perime l'ouverture");
    // La fenetre, apres une ecriture (`api::repondre`) : tout se verifie en fond, l'ouverture comprise.
    f.tout_verifier();
    let debut = Instant::now();
    while !ouverture_juste(&racine) {
        assert!(debut.elapsed() < Duration::from_secs(120), "la file n'a pas refait l'ouverture");
        std::thread::sleep(Duration::from_millis(50));
    }
    f.arreter();
}

/// LE JOUR DU MOTEUR (#236) : l'ouverture dit quel jour on est, et c'est celui du moteur -- son temps simule au banc
/// d'essai (`temps`), jamais l'horloge du navigateur. La page le prend pour tout ce qu'elle date ou presente au jour.
#[test]
fn l_ouverture_dit_le_jour_du_moteur() {
    let racine = compagnie("ouverture-jour");
    let jour = paie::date::Jour::lire("2026-10-05").unwrap();
    paie::temps::figer(paie::temps::instant_de(jour, 9));
    let f = FileDesEtats::nouvelle(&racine, Some(JOUR));
    let o = ouvrir(&f, 120_000, false);
    assert_eq!(o["pret"], json!(true));
    assert_eq!(o["aujourdhui"], json!("2026-10-05"));
    assert_eq!(ouvrir(&f, 0, true)["aujourdhui"], json!("2026-10-05"), "l'ouverture fraiche aussi");
    f.arreter();
    paie::temps::liberer();
}
