//! LES ROUTES SERVIES DANS LE PROCESSUS DE LA FENETRE (ticket #244, « ouvrir une compagnie prend 4 s ») : la fenetre ne
//! lance plus un processus par route (`interface::api::route` appelle `routes::dans_la_fenetre`). Trois contrats :
//!
//! 1. une route servie dans la fenetre rend ce que rend `paie route` dans son processus ;
//! 2. une route qui tombe (une panique) rend un refus `DefautDuLogiciel`, ne laisse rien sur son fil (ni mot de passe
//!    tape, ni nombres « fideles ») et la suivante est servie ; un mot de passe tape ne survit jamais a sa route ;
//! 3. deux compagnies servies EN MEME TEMPS, par deux fils du meme processus, lisent chacune les siennes (la compagnie
//!    vient de la route, jamais d'une variable du processus : la fenetre en gardera plusieurs).
//!
//! Seul dans `tests/` : il pose des variables du processus (le jour, une cle de poste absente), une fois.

#[path = "outils/mod.rs"]
mod outils;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Once;

use outils::{compagnie as cie, dossier_de_test, lire};
use paie::interface::api;
use serde_json::{json, Value};

const JOUR: &str = "2026-10-22";

/// Le processus du test comme celui d'un `paie` de test (`outils::bac_a_sable`) : le jour, une cle de poste qui n'existe
/// pas, aucun secret du poste.
fn preparer() {
    static UNE_FOIS: Once = Once::new();
    UNE_FOIS.call_once(|| {
        for v in outils::RETIREES {
            std::env::remove_var(v);
        }
        std::env::remove_var("PUPPETPAIE_DONNEES");
        std::env::set_var("PUPPETPAIE_AUJOURDHUI", JOUR);
        std::env::set_var("PUPPETPAIE_CLE", Path::new(env!("CARGO_TARGET_TMPDIR")).join("routes-dans-la-fenetre-pas-de-cle"));
    });
}

fn compagnie(test: &str, fictive: &str) -> PathBuf {
    preparer();
    cie::entiere(fictive, &dossier_de_test("routes_dans_la_fenetre", test).join(fictive))
}

/// `paie route --racine R` dans son processus, comme la fenetre la lancait avant (sans `--date` : le jour est celui de
/// `PUPPETPAIE_AUJOURDHUI`, comme ici).
fn en_processus(racine: &Path, nom: &str, charge: &Value) -> Result<Value, (String, String)> {
    let mut c = Command::new(env!("CARGO_BIN_EXE_paie"));
    c.args(["route", "--racine", racine.to_str().unwrap()]).stdin(Stdio::piped()).stdout(Stdio::piped());
    outils::bac_a_sable(&mut c, racine, JOUR);
    c.env_remove("PUPPETPAIE_DONNEES").env("PUPPETPAIE_CLE", std::env::var_os("PUPPETPAIE_CLE").unwrap());
    let mut p = c.spawn().unwrap();
    p.stdin.take().unwrap().write_all(json!({"route": nom, "charge": charge}).to_string().as_bytes()).unwrap();
    let sortie = p.wait_with_output().unwrap().stdout;
    let rep: Value = serde_json::from_slice(&sortie).unwrap_or_else(|e| panic!("{} : reponse illisible ({}) : {}", nom, e, String::from_utf8_lossy(&sortie)));
    api::depuis_reponse(rep).map_err(|r| (r.classe, r.message))
}

fn dans_la_fenetre(racine: &Path, nom: &str, charge: &Value) -> Result<Value, (String, String)> {
    api::route(racine, nom, charge).map_err(|r| (r.classe, r.message))
}

/// Les lectures de l'ouverture d'une compagnie (ce que la page demande en s'ouvrant), et quelques autres.
fn lectures() -> Vec<(&'static str, Value)> {
    vec![
        ("etat", json!({})),
        ("mois-a-ouvrir", json!({})),
        ("echeancier", json!({})),
        ("rappels", json!({})),
        ("signalements", json!({})),
        ("conseil-du-jour", json!({"decalage": 0})),
        ("pour-demarrer", json!({})),
        ("conseils-resume", json!({})),
        ("identite", json!({})),
        ("saisie", json!({"mois": "2026-09"})),
        ("bilan-du-mois", json!({"mois": "2026-09"})),
        ("contrats-en-cours", json!({})),
        // Un refus se rend pareil : sa classe et sa phrase.
        ("saisie", json!({})),
    ]
}

#[test]
fn une_route_servie_dans_la_fenetre_rend_ce_que_rend_paie_route() {
    let racine = compagnie("pareil", "fictive-1285-grande");
    for (nom, charge) in lectures() {
        let avant = en_processus(&racine, nom, &charge);
        // Deux fois dans la fenetre : la premiere a froid, la seconde avec ce que le processus garde en memoire.
        for tour in ["a froid", "a chaud"] {
            assert_eq!(dans_la_fenetre(&racine, nom, &charge), avant, "{} ({}) : la fenetre ne rend pas ce que rend paie route", nom, tour);
        }
    }
}

/// `bilan-du-mois` (et `mois-a-ouvrir`, qui le fait pour chaque mois passe) lit l'etat garde du mois quand il est juste,
/// au lieu de recharger la saisie : il rend la meme chose avec ou sans lui.
#[test]
fn le_bilan_d_un_mois_est_le_meme_avec_ou_sans_son_etat_garde() {
    let racine = compagnie("bilan-garde", "fictive-1285-grande");
    for mois in ["2025-03", "2026-02", "2026-09"] {
        let _ = std::fs::remove_file(racine.join("paies").join(format!("{}.etat.json", mois)));
        let charge = json!({ "mois": mois });
        let sans = dans_la_fenetre(&racine, "bilan-du-mois", &charge);
        assert!(sans.is_ok(), "{} : {:?}", mois, sans);
        // L'etat garde se fait (la route `etat-par-paie` le refait et l'ecrit), puis le bilan le lit.
        dans_la_fenetre(&racine, "etat-par-paie", &charge).unwrap();
        assert_eq!(paie::etat_garde::servir(&racine, mois, JOUR).map(|(_, juste)| juste), Some(true), "{} : pas d'etat garde juste", mois);
        assert_eq!(dans_la_fenetre(&racine, "bilan-du-mois", &charge), sans, "{} : le bilan change avec l'etat garde", mois);
    }
}

#[test]
fn une_route_qui_tombe_rend_un_refus_et_la_suivante_est_servie() {
    let racine = compagnie("tombe", "fictive-1285-petite");
    let (classe, message) = dans_la_fenetre(&racine, paie::routes::ROUTE_QUI_TOMBE, &json!({"motdepasse": "tape-par-un-humain"})).unwrap_err();
    assert_eq!(classe, "DefautDuLogiciel", "{}", message);
    // Le fil est propre : la route tombee avait retenu un mot de passe et rendu les nombres « fideles ».
    assert!(!paie::coffre::mot_de_passe_retenu(), "le mot de passe tape a survecu a la route tombee");
    assert!(!socle::num::FIDELE.with(|f| f.get()), "les nombres sont restes fideles apres la route tombee");
    // La suivante, sur le meme fil, est servie, et rend ce que rend paie route.
    assert_eq!(dans_la_fenetre(&racine, "etat", &json!({})), en_processus(&racine, "etat", &json!({})));
}

#[test]
fn un_mot_de_passe_tape_ne_survit_pas_a_sa_route() {
    let racine = compagnie("mot-de-passe", "fictive-1285-petite");
    // Une route qui reussit, une qui refuse : aucune ne laisse le mot de passe sur son fil.
    for (nom, charge) in [("coffre-etat", json!({"motdepasse": "tape-par-un-humain"})), ("saisie", json!({"motdepasse": "tape-par-un-humain"}))] {
        let _ = dans_la_fenetre(&racine, nom, &charge);
        assert!(!paie::coffre::mot_de_passe_retenu(), "{} : le mot de passe tape a survecu a sa route", nom);
    }
}

/// La raison sociale telle que `etat` la rend, le dossier de son identite, le mois a ouvrir.
fn ce_que_lit(racine: &Path) -> (Value, String, Value) {
    let etat = dans_la_fenetre(racine, "etat", &json!({})).unwrap();
    let identite = dans_la_fenetre(racine, "identite", &json!({})).unwrap();
    let signature = &identite["identite"]["signature"];
    let dossier = signature["fichier"].as_str().or(signature["dossier"].as_str()).unwrap_or_default().to_string();
    let mois = dans_la_fenetre(racine, "mois-a-ouvrir", &json!({})).unwrap();
    (etat.pointer("/configuration/entreprise/donnees/raison_sociale").cloned().unwrap_or(Value::Null), dossier, mois["mois"].clone())
}

#[test]
fn deux_compagnies_servies_en_meme_temps_lisent_chacune_les_siennes() {
    let a = compagnie("deux-compagnies-a", "fictive-1285-grande");
    let b = compagnie("deux-compagnies-b", "fictive-3090-grande");
    let nom = |r: &Path| lire(r, "config/entreprise.json")["raison_sociale"].clone();
    assert_ne!(nom(&a), nom(&b), "deux compagnies de noms differents sont attendues");
    // Ce que chacune lit, seule.
    let attendu_a = ce_que_lit(&a);
    let attendu_b = ce_que_lit(&b);
    assert_eq!(attendu_a.0, nom(&a));
    assert_eq!(attendu_b.0, nom(&b));
    assert!(Path::new(&attendu_a.1).starts_with(&a), "l'identite de A est lue hors de A : {}", attendu_a.1);
    assert!(Path::new(&attendu_b.1).starts_with(&b), "l'identite de B est lue hors de B : {}", attendu_b.1);
    // Puis toutes deux a la fois, plusieurs fils chacune, plusieurs tours : chacune lit toujours les siennes.
    std::thread::scope(|s| {
        let mut fils = Vec::new();
        for _ in 0..3 {
            for (racine, attendu) in [(&a, &attendu_a), (&b, &attendu_b)] {
                fils.push(s.spawn(move || {
                    for _ in 0..4 {
                        assert_eq!(&ce_que_lit(racine), attendu, "{} n'a pas lu les siennes", racine.display());
                    }
                }));
            }
        }
        for f in fils {
            f.join().unwrap();
        }
    });
}
