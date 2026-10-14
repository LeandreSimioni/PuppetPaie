//! Le binaire de l'atelier (lot J2a, preuve, partie 1) : `paie-atelier`, construit avec `--features atelier`, pour les essais
//! seulement. Trois choses a montrer, toutes SANS RESEAU (seulement des faux serveurs locaux, 127.0.0.1) :
//!
//! - dans `paie-atelier`, la redirection marche : `retours` et `retours --dpae` parlent au faux serveur que les variables
//!   `PUPPETPAIE_ATELIER_*` designent, et seulement a lui ; une adresse qui n'est pas locale arrete le programme ;
//! - dans `paie-atelier`, un acte reel est refuse, mot de passe ou non (la meme charge, dans `paie`, est refusee pour une
//!   autre raison : aucun mot de passe du logiciel) ;
//! - le binaire livre (`paie`, construit SANS l'option) ne contient pas la redirection : il ignore les variables et parle aux
//!   adresses en dur. Preuve sans reseau : un faux mandataire local (`HTTPS_PROXY`) recoit la demande `CONNECT` et dit a qui
//!   `paie` voulait parler ; le faux serveur, lui, ne recoit rien.
//!
//! Les tests de la premiere et de la deuxieme famille ne tournent qu'avec `--features atelier` (la suite de l'atelier,
//! `atelier/CLAUDE.md`) ; ceux de la troisieme, dans la suite livree. Chaque processus lance part de
//! `env_clear` : ni mot de passe du poste, ni mandataire du poste. Le seul « mot de passe » est un faux, jamais affiche.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use outils::{compagnie as cie, dossier_de_test, http, mandataire as faux_mandataire, serveur, Recu};
use serde_json::{json, Value};

#[path = "outils/mod.rs"]
mod outils;

const FAUX_MOTDEPASSE: &str = "faux-motdepasse-atelier-0001";
const AUJOURDHUI: &str = "2026-10-08";

/// Une copie d'une compagnie fictive devenue compagnie ordinaire (marqueur retire, declarant et compte Urssaf poses).
fn compagnie(nom: &str) -> PathBuf {
    let racine = cie::entiere("fictive-3090-petite", &dossier_de_test("atelier", nom).join("compagnie"));
    cie::ordinaire(&racine);
    racine
}

/// Un serveur local qui repond `statut` avec `corps` (sauf a l'authentification, qui reussit toujours).
fn faux_serveur(statut: u16, corps: &'static str) -> (String, Recu) {
    serveur(move |q| Some(if q.chemin.contains("authentifier") { http(200, "text/plain", b"JETON-FICTIF") } else { http(statut, "text/plain", corps.as_bytes()) }))
}

/// Ce que le serveur a recu : « METHODE chemin ».
fn lignes(recu: &Recu) -> Vec<String> {
    recu.lock().unwrap().iter().map(|q| format!("{} {}", q.methode, q.chemin)).collect()
}

struct Sortie {
    code: i32,
    sortie: String,
}

/// Lance un binaire de l'environnement VIDE, avec seulement ce qu'il faut. `extra` : les variables de l'essai.
fn lancer(binaire: &str, racine: &Path, args: &[&str], extra: &[(&str, &str)]) -> Sortie {
    let mut c = Command::new(binaire);
    c.env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", racine.parent().unwrap())
        .env("PUPPETPAIE_DONNEES", racine)
        .env("PUPPETPAIE_CLE", racine.parent().unwrap().join("cle"))
        .env("PUPPETPAIE_AUJOURDHUI", AUJOURDHUI)
        .env("NET_ENTREPRISES_MOTDEPASSE", FAUX_MOTDEPASSE)
        .env("URSSAF_MOTDEPASSE", FAUX_MOTDEPASSE)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in extra {
        c.env(k, v);
    }
    let p = c.output().unwrap();
    let sortie = format!("{}{}", String::from_utf8_lossy(&p.stdout), String::from_utf8_lossy(&p.stderr));
    assert!(!sortie.contains(FAUX_MOTDEPASSE), "un mot de passe est apparu a l'ecran");
    Sortie { code: p.status.code().unwrap_or(-1), sortie }
}

// =============================================================================== paie-atelier

#[cfg(feature = "atelier")]
mod atelier {
    use super::*;

    const ATELIER: &str = env!("CARGO_BIN_EXE_paie-atelier");

    #[test]
    fn la_dsn_est_redirigee_vers_le_faux_serveur() {
        let racine = compagnie("dsn");
        let (base, recu) = faux_serveur(404, "Aucun retour sur cette plage");
        let s = lancer(ATELIER, &racine, &["retours", "--depuis", "01/10/2026", "--jusqu-a", "02/10/2026"], &[("PUPPETPAIE_ATELIER_DSN", &format!("{}/dsn", base))]);
        let vus = lignes(&recu);
        assert!(vus.iter().any(|l| l.contains("/dsn/authentifier/")), "pas d'authentification sur le faux serveur : {:?}\n{}", vus, s.sortie);
        assert!(vus.iter().any(|l| l.contains("/dsn/lister-retours-declarant/1.0/")), "pas de consultation : {:?}\n{}", vus, s.sortie);
        assert!(vus.iter().all(|l| l.contains("/dsn/")), "une requete est sortie du prefixe : {:?}", vus);
        assert!(s.sortie.to_lowercase().contains("aucun retour"), "{}", s.sortie);
    }

    #[test]
    fn la_dpae_est_redirigee_vers_le_faux_serveur() {
        let racine = compagnie("dpae");
        let (base, recu) = faux_serveur(404, "Aucun retour");
        let s = lancer(ATELIER, &racine, &["retours", "--dpae", "--flux", "FLUX-FAUX"], &[("PUPPETPAIE_ATELIER_DPAE", &format!("{}/dpae", base))]);
        let vus = lignes(&recu);
        assert!(vus.iter().any(|l| l.contains("/dpae/authentifier_dpae")), "{:?}\n{}", vus, s.sortie);
        assert!(vus.iter().any(|l| l.contains("/dpae/lister-retours-flux/1.0/FLUX-FAUX")), "{:?}\n{}", vus, s.sortie);
        assert!(vus.iter().all(|l| l.contains("/dpae/")), "{:?}", vus);
    }

    #[test]
    fn sans_variable_latelier_parle_aux_adresses_en_dur_donc_a_personne_ici() {
        // Rien n'est redirige : le faux serveur ne recoit rien. Le mandataire local capte la demande vers l'adresse en dur
        // (ce serait la seule sortie possible) et refuse : aucun octet ne quitte la machine.
        let racine = compagnie("sans-variable");
        let (base, recu) = faux_serveur(404, "x");
        let (proxy, vus_proxy) = faux_mandataire();
        let _ = base;
        lancer(ATELIER, &racine, &["retours", "--depuis", "01/10/2026", "--jusqu-a", "02/10/2026"], &[("HTTPS_PROXY", &proxy)]);
        assert!(recu.lock().unwrap().is_empty());
        let v = vus_proxy.lock().unwrap().clone();
        assert!(v.iter().any(|l| l.starts_with("CONNECT services.net-entreprises.fr:443")), "{:?}", v);
    }

    #[test]
    fn une_adresse_qui_nest_pas_locale_arrete_le_programme() {
        let racine = compagnie("non-locale");
        let (proxy, vus_proxy) = faux_mandataire();
        for variable in ["PUPPETPAIE_ATELIER_DSN", "PUPPETPAIE_ATELIER_DPAE", "PUPPETPAIE_ATELIER_GOOGLE"] {
            let s = lancer(ATELIER, &racine, &["version"], &[(variable, "https://services.net-entreprises.fr/"), ("HTTPS_PROXY", &proxy)]);
            assert_eq!(s.code, 2, "{}", s.sortie);
            assert!(s.sortie.contains("serveur local") && s.sortie.contains(variable), "{}", s.sortie);
        }
        assert!(vus_proxy.lock().unwrap().is_empty(), "une demande est partie");
    }

    /// Google : les trois adresses du client Drive suivent `PUPPETPAIE_ATELIER_GOOGLE` (seul test du processus qui la touche).
    #[test]
    fn google_est_redirige_et_une_adresse_non_locale_ne_mene_nulle_part() {
        std::env::set_var("PUPPETPAIE_ATELIER_GOOGLE", "http://127.0.0.1:4321/g/");
        let a = paie::drive::Adresses::default();
        assert_eq!(a.jetons, "http://127.0.0.1:4321/g/token");
        assert_eq!(a.fichiers, "http://127.0.0.1:4321/g/drive/v3/files");
        assert_eq!(a.televersement, "http://127.0.0.1:4321/g/upload/drive/v3/files");
        std::env::set_var("PUPPETPAIE_ATELIER_GOOGLE", "https://www.googleapis.com");
        let a = paie::drive::Adresses::default();
        assert!(a.jetons.starts_with("http://127.0.0.1:9/") && a.fichiers.starts_with("http://127.0.0.1:9/") && a.televersement.starts_with("http://127.0.0.1:9/"), "{:?}", a);
        std::env::remove_var("PUPPETPAIE_ATELIER_GOOGLE");
        assert_eq!(paie::drive::Adresses::default().jetons, "https://oauth2.googleapis.com/token");
    }

    #[test]
    fn version_dit_que_cest_latelier() {
        let racine = compagnie("version");
        let s = lancer(ATELIER, &racine, &["version"], &[]);
        assert_eq!(s.code, 0, "{}", s.sortie);
        assert!(s.sortie.contains("BINAIRE DE L'ATELIER") && s.sortie.contains("refuse tout acte reel"), "{}", s.sortie);
        let j = lancer(ATELIER, &racine, &["version", "--json"], &[]);
        let v: Value = serde_json::from_str(&j.sortie).unwrap();
        assert_eq!(v["resultat"]["atelier"], json!(true), "{}", j.sortie);
    }

    /// La meme charge (un depot REEL d'une DSN deja produite, avec un mot de passe quelconque) : `paie-atelier` la refuse avec
    /// SA classe, avant toute autre question ; rien n'est parti.
    #[test]
    fn un_acte_reel_est_refuse_mot_de_passe_ou_non() {
        let racine = compagnie("acte-reel");
        let dsn = racine.join("sorties/2026-07/DSN_Juillet2026.dsn");
        assert!(dsn.is_file());
        let (base, recu) = faux_serveur(200, "x");
        for motdepasse in [Some("un-mot-de-passe-quelconque"), None] {
            let mut charge = json!({"quoi": "dsn", "fichier": dsn.to_string_lossy(), "reel": true});
            if let Some(m) = motdepasse {
                charge["motdepasse"] = json!(m);
            }
            let mut c = Command::new(ATELIER);
            c.env_clear()
                .env("PATH", std::env::var("PATH").unwrap_or_default())
                .env("HOME", racine.parent().unwrap())
                .env("PUPPETPAIE_DONNEES", &racine)
                .env("PUPPETPAIE_CLE", racine.parent().unwrap().join("cle"))
                .env("PUPPETPAIE_AUJOURDHUI", AUJOURDHUI)
                .env("PUPPETPAIE_ATELIER_DSN", format!("{}/dsn", base))
                .args(["route", "--racine", racine.to_str().unwrap(), "--date", AUJOURDHUI])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            let mut p = c.spawn().unwrap();
            p.stdin.take().unwrap().write_all(json!({"route": "deposer", "charge": charge}).to_string().as_bytes()).unwrap();
            let sortie = p.wait_with_output().unwrap().stdout;
            let r: Value = serde_json::from_slice(&sortie).unwrap_or_else(|e| panic!("reponse illisible ({}) : {}", e, String::from_utf8_lossy(&sortie)));
            assert_eq!(r["ok"], json!(false), "{}", r);
            assert_eq!(r["classe"], json!("ActeReelRefuse"), "{}", r);
            assert!(r["erreur"].as_str().unwrap().contains("binaire de l'atelier"), "{}", r);
        }
        assert!(recu.lock().unwrap().is_empty(), "le faux serveur a recu quelque chose : un depot est parti");
        assert!(!racine.join("config/depots.json").exists() || std::fs::read_to_string(racine.join("config/depots.json")).unwrap().trim_matches(|c| c == '[' || c == ']' || c == ' ' || c == '\n').is_empty());
    }
}

// ============================================================================= paie (livre)

#[cfg(not(feature = "atelier"))]
mod livre {
    use super::*;

    const LIVRE: &str = env!("CARGO_BIN_EXE_paie");

    fn contient(octets: &[u8], aiguille: &[u8]) -> bool {
        octets.windows(aiguille.len()).any(|f| f == aiguille)
    }

    /// Le binaire livre ne porte ni les noms des variables de redirection, ni la phrase du refus, ni le mot « atelier » de
    /// `version` : le code de l'atelier n'y est pas compile.
    #[test]
    fn le_binaire_livre_ne_contient_pas_la_redirection() {
        let octets = std::fs::read(LIVRE).unwrap();
        for absent in ["PUPPETPAIE_ATELIER", "paie-atelier", "BINAIRE DE L'ATELIER", "adresse-non-locale-refusee"] {
            assert!(!contient(&octets, absent.as_bytes()), "le binaire livre contient « {} »", absent);
        }
    }

    #[test]
    fn le_binaire_livre_ignore_la_variable_de_la_dsn() {
        let racine = compagnie("livre-dsn");
        let (base, recu) = faux_serveur(404, "x");
        let (proxy, vus_proxy) = faux_mandataire();
        lancer(LIVRE, &racine, &["retours", "--depuis", "01/10/2026", "--jusqu-a", "02/10/2026"], &[("PUPPETPAIE_ATELIER_DSN", &format!("{}/dsn", base)), ("HTTPS_PROXY", &proxy)]);
        assert!(lignes(&recu).is_empty(), "le faux serveur a ete appele : {:?}", lignes(&recu));
        let v = vus_proxy.lock().unwrap().clone();
        assert!(v.iter().any(|l| l.starts_with("CONNECT services.net-entreprises.fr:443")), "l'adresse en dur n'a pas ete visee : {:?}", v);
    }

    #[test]
    fn le_binaire_livre_ignore_la_variable_de_la_dpae() {
        let racine = compagnie("livre-dpae");
        let (base, recu) = faux_serveur(404, "x");
        let (proxy, vus_proxy) = faux_mandataire();
        lancer(LIVRE, &racine, &["retours", "--dpae", "--flux", "FLUX-FAUX"], &[("PUPPETPAIE_ATELIER_DPAE", &format!("{}/dpae", base)), ("HTTPS_PROXY", &proxy)]);
        assert!(lignes(&recu).is_empty(), "le faux serveur a ete appele : {:?}", lignes(&recu));
        let v = vus_proxy.lock().unwrap().clone();
        assert!(v.iter().any(|l| l.starts_with("CONNECT mon.urssaf.fr:443")), "l'adresse en dur n'a pas ete visee : {:?}", v);
    }

    #[test]
    fn le_binaire_livre_ne_se_dit_pas_atelier() {
        let racine = compagnie("livre-version");
        let s = lancer(LIVRE, &racine, &["version"], &[]);
        assert_eq!(s.code, 0, "{}", s.sortie);
        assert!(!s.sortie.contains("ATELIER"), "{}", s.sortie);
        let j = lancer(LIVRE, &racine, &["version", "--json"], &[]);
        let v: Value = serde_json::from_str(&j.sortie).unwrap();
        assert!(v["resultat"].get("atelier").is_none(), "{}", j.sortie);
    }

    /// La meme charge qu'a l'atelier : ici elle va jusqu'a la preuve et s'arrete faute de mot de passe du logiciel (aucun n'est
    /// pose dans une compagnie fictive). Elle ne rend surtout pas `ActeReelRefuse`, qui n'existe que dans l'atelier.
    #[test]
    fn la_meme_charge_dans_le_binaire_livre_est_refusee_pour_une_autre_raison() {
        let racine = compagnie("livre-acte");
        let dsn = racine.join("sorties/2026-07/DSN_Juillet2026.dsn");
        let (proxy, vus_proxy) = faux_mandataire();
        let mut c = Command::new(LIVRE);
        c.env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("HOME", racine.parent().unwrap())
            .env("PUPPETPAIE_DONNEES", &racine)
            .env("PUPPETPAIE_CLE", racine.parent().unwrap().join("cle"))
            .env("PUPPETPAIE_AUJOURDHUI", AUJOURDHUI)
            .env("HTTPS_PROXY", &proxy)
            .args(["route", "--racine", racine.to_str().unwrap(), "--date", AUJOURDHUI])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut p = c.spawn().unwrap();
        let charge = json!({"quoi": "dsn", "fichier": dsn.to_string_lossy(), "reel": true, "motdepasse": "un-mot-de-passe-quelconque"});
        p.stdin.take().unwrap().write_all(json!({"route": "deposer", "charge": charge}).to_string().as_bytes()).unwrap();
        let sortie = p.wait_with_output().unwrap().stdout;
        let r: Value = serde_json::from_slice(&sortie).unwrap();
        assert_eq!(r["ok"], json!(false), "{}", r);
        assert_eq!(r["classe"], json!("PreuveInsuffisante"), "{}", r);
        assert!(vus_proxy.lock().unwrap().is_empty(), "une demande est partie : {:?}", vus_proxy.lock().unwrap());
    }
}
