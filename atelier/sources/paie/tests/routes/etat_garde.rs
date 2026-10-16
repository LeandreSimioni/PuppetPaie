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
