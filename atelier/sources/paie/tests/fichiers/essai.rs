//! La compagnie d'essai (`paie essai monter`, lot E1) : l'identite d'une compagnie lue dans `PUPPETPAIE_COMPAGNIE`, posee sur
//! la copie d'une fictive, pour un vrai depot de TEST sans `prive/`. Ce qui se prouve :
//!
//! - le contrat du montage : une variable absente, une cle qui manque ou en trop refuse, nomme la CLE, et rien n'est ecrit ;
//!   aucune valeur de la variable n'apparait jamais a l'ecran (ni en refus, ni en succes, ni au depot) ;
//! - la copie : l'identite de la variable dans la fiche, celle de la fictive retiree (son `historique` compris), ses
//!   retours et ses depots de DSN absents, le marqueur d'une compagnie d'essai (et non d'une fictive) ;
//! - la DSN du mois s'ecrit sous cette identite (S10.G00.01, S10.G00.02, S20.G00.07, S21.G00.06 : cahier technique DSN,
//!   `norme-dsn/texte/bloc-S10.G00.02.txt`) ;
//! - ses depots de TEST vont au reseau (une fictive les joue) : `paie` livre, un faux mandataire local voit la demande
//!   `CONNECT` vers net-entreprises ou l'Urssaf et refuse, rien ne sort ; tout depot REEL est refuse avant le reseau ;
//! - dans `paie-atelier` (module `atelier`), le depot de test arrive au faux serveur sous l'identite de la variable, et
//!   `--ordre` le fait partir en annule-et-remplace a cet ordre.
//!
//! La copie se monte dans le dossier temporaire du systeme, pas sous `CARGO_TARGET_TMPDIR` : le montage refuse tout dossier
//! d'un depot Git, et la cible de Cargo peut en etre un (`target/` du depot). Chaque `paie` part de `env_clear`.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use crate::outils::{copier, dossier_du_processus, fictive, lire, mandataire};
use serde_json::{json, Value};

const AUJOURDHUI: &str = "2026-10-08";
const FICTIVE: &str = "fictive-3090-grande";
const FAUX_MOTDEPASSE: &str = "faux-motdepasse-essai-0001";

/// Une identite INVENTEE (SIREN et SIRET a cle de Luhn juste, personne de ce nom) : ce qu'un developpeur poserait pour sa
/// compagnie. Chaque valeur est assez singuliere pour etre cherchee a l'ecran.
fn identite() -> Value {
    json!({
        "siren": "987654324", "nic": "00019",
        "raison_sociale": "CIE ESSAI QUENOUILLE", "adresse": "7 IMPASSE DES ESSAIS", "code_postal": "75011", "commune": "PARIS",
        "naf": "9001Z", "medecine_travail": "MTESSAI7",
        "declarant": {"nom": "QUENOUILLE", "prenom": "ARIANE MARIE"},
        "declarant_dsn": {"civilite": "02", "nom_prenom": "QUENOUILLE ARIANE", "tel": "0199887766", "email": "essai@quenouille.invalid", "qualite": "04"},
        "api_dpae": {"nom": "QUENOUILLEDPAE", "prenom": "ARIANE", "siret": "", "service": ""}
    })
}

/// Ce qui ne doit jamais s'afficher : chaque valeur de l'identite (et le SIRET qui en derive), et le faux mot de passe.
fn valeurs(id: &Value) -> Vec<String> {
    fn feuilles(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Object(o) => o.values().for_each(|x| feuilles(x, out)),
            Value::String(s) if s.len() >= 4 => out.push(s.clone()),
            _ => {}
        }
    }
    let mut v = Vec::new();
    feuilles(id, &mut v);
    v.push(format!("{}{}", id["siren"].as_str().unwrap_or(""), id["nic"].as_str().unwrap_or("")));
    v.push(FAUX_MOTDEPASSE.into());
    v
}

/// `logiciel/donnees` copie une fois par processus (le code livre, jamais celui du depot) et le dossier de la cle du poste.
fn monde() -> &'static Path {
    static MONDE: OnceLock<PathBuf> = OnceLock::new();
    MONDE.get_or_init(|| {
        let m = dossier_du_processus("essai");
        copier(&crate::outils::code_livre(), &m.join("logiciel/donnees"));
        std::fs::create_dir_all(m.join("cle")).unwrap();
        m
    })
}

/// Le dossier de la compagnie d'essai d'un test : sous le dossier temporaire du systeme, neuf (il n'existe pas encore).
fn dossier(test: &str) -> PathBuf {
    let d = std::env::temp_dir().join("paie-essai-tests").join(format!("{}-{}", std::process::id(), test));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.parent().unwrap()).unwrap();
    d
}

struct Sortie {
    code: i32,
    texte: String,
}

fn lancer(binaire: &str, donnees: Option<&Path>, args: &[&str], env: &[(&str, &str)]) -> Sortie {
    let m = monde();
    let mut c = Command::new(binaire);
    c.env_clear()
        .env(paie::garde_reseau::VARIABLE, "1") // le garde-fou reseau des essais : rien ne sort de la machine
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", m)
        .env("PUPPETPAIE_CLE", m.join("cle"))
        .env("PUPPETPAIE_CODE", m.join("logiciel/donnees"))
        .env("PUPPETPAIE_AUJOURDHUI", AUJOURDHUI)
        .envs(env.iter().copied())
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(d) = donnees {
        c.env("PUPPETPAIE_DONNEES", d);
    }
    let p = c.output().unwrap();
    let texte = format!("{}{}", String::from_utf8_lossy(&p.stdout), String::from_utf8_lossy(&p.stderr));
    Sortie { code: p.status.code().unwrap_or(-1), texte }
}

/// Aucune valeur de l'identite, aucun mot de passe a l'ecran.
fn rien_d_affiche(s: &Sortie, id: &Value) {
    for v in valeurs(id) {
        assert!(!s.texte.contains(&v), "une valeur de la variable est apparue a l'ecran :\n{}", s.texte);
    }
}

const PAIE: &str = env!("CARGO_BIN_EXE_paie");

fn monter_avec(binaire: &str, d: &Path, variable: Option<&str>) -> Sortie {
    let source = fictive(FICTIVE);
    let (src, dst) = (source.to_str().unwrap(), d.to_str().unwrap());
    let env: Vec<(&str, &str)> = variable.map(|v| vec![("PUPPETPAIE_COMPAGNIE", v)]).unwrap_or_default();
    lancer(binaire, None, &["essai", "monter", dst, "--depuis", src], &env)
}

/// Une compagnie d'essai montee sous `identite()`.
fn montee(binaire: &str, test: &str) -> PathBuf {
    let d = dossier(test);
    let id = identite();
    let s = monter_avec(binaire, &d, Some(&id.to_string()));
    assert_eq!(s.code, 0, "{}", s.texte);
    rien_d_affiche(&s, &id);
    d
}

/// Les paires (rubrique, valeur) d'une DSN.
fn rubriques(dsn: &str) -> Vec<(String, String)> {
    crate::outils::paires(dsn)
}

fn valeur(v: &[(String, String)], r: &str) -> String {
    crate::outils::valeur(v, r).unwrap_or_default()
}

// ------------------------------------------------------------------ le contrat du montage

#[test]
fn sans_la_variable_le_montage_refuse_et_n_ecrit_rien() {
    let d = dossier("sans-variable");
    for variable in [None, Some(""), Some("   ")] {
        let s = monter_avec(PAIE, &d, variable);
        assert_eq!(s.code, 1, "{}", s.texte);
        assert!(s.texte.contains("PUPPETPAIE_COMPAGNIE absente") && s.texte.contains("Rien n'est ecrit"), "{}", s.texte);
        assert!(!d.exists(), "le dossier a ete cree");
    }
}

#[test]
fn une_cle_qui_manque_ou_en_trop_est_nommee_sans_sa_valeur_et_rien_n_est_ecrit() {
    let d = dossier("cles");
    let mut id = identite();
    id["declarant_dsn"].as_object_mut().unwrap().remove("civilite");
    id.as_object_mut().unwrap().remove("medecine_travail");
    id["telephone_portable"] = json!("0611223344");
    let s = monter_avec(PAIE, &d, Some(&id.to_string()));
    assert_eq!(s.code, 1, "{}", s.texte);
    for nom in ["medecine_travail", "declarant_dsn.civilite", "telephone_portable"] {
        assert!(s.texte.contains(nom), "{} n'est pas nomme : {}", nom, s.texte);
    }
    rien_d_affiche(&s, &id);
    assert!(!s.texte.contains("0611223344"), "{}", s.texte);
    assert!(!d.exists());
}

/// Un SIREN a cle fausse (la valeur n'apparait pas dans le refus) et un JSON mal ecrit (le refus ne dit que sa position).
#[test]
fn une_forme_fausse_ou_un_json_illisible_refuse_sans_rien_citer() {
    let d = dossier("formes");
    let mut id = identite();
    id["siren"] = json!("987654321");
    let s = monter_avec(PAIE, &d, Some(&id.to_string()));
    assert_eq!(s.code, 1, "{}", s.texte);
    assert!(s.texte.contains("`siren`"), "{}", s.texte);
    rien_d_affiche(&s, &id);
    let tronque = identite().to_string();
    let tronque = &tronque[..tronque.len() / 2];
    let s = monter_avec(PAIE, &d, Some(tronque));
    assert_eq!(s.code, 1, "{}", s.texte);
    assert!(s.texte.contains("n'est pas un JSON lisible"), "{}", s.texte);
    rien_d_affiche(&s, &identite());
    assert!(!d.exists());
}

/// Une compagnie d'essai ne se monte que depuis une FICTIVE : jamais depuis une compagnie dont le marqueur ne le dit pas.
#[test]
fn le_montage_refuse_une_source_qui_n_est_pas_une_fictive() {
    let source = crate::outils::dossier_de_test("essai", "source-ordinaire");
    crate::outils::compagnie::config_et_mois(FICTIVE, &[], &source);
    let d = dossier("source-ordinaire");
    let id = identite();
    let s = lancer(PAIE, None, &["essai", "monter", d.to_str().unwrap(), "--depuis", source.to_str().unwrap()], &[("PUPPETPAIE_COMPAGNIE", &id.to_string())]);
    assert_eq!(s.code, 1, "{}", s.texte);
    assert!(s.texte.contains("n'est pas une compagnie fictive"), "{}", s.texte);
    assert!(!d.exists());
}

/// Un dossier dans un depot Git est refuse : la copie porterait l'identite d'une vraie compagnie la ou `git add` la prendrait.
#[test]
fn le_montage_refuse_un_dossier_dans_un_depot_git() {
    let racine = dossier("depot-git");
    std::fs::create_dir_all(racine.join(".git")).unwrap();
    let d = racine.join("sous").join("essai");
    let s = monter_avec(PAIE, &d, Some(&identite().to_string()));
    assert_eq!(s.code, 1, "{}", s.texte);
    assert!(s.texte.contains("hors de tout depot"), "{}", s.texte);
    assert!(!d.exists());
    let _ = std::fs::remove_dir_all(&racine);
}

// ------------------------------------------------------------------ la copie

#[test]
fn la_copie_porte_l_identite_de_la_variable_et_plus_rien_de_celle_de_la_fictive() {
    let d = montee(PAIE, "copie");
    let id = identite();
    let e = lire(&d, "config/entreprise.json");
    for k in ["siren", "nic", "raison_sociale", "adresse", "code_postal", "commune", "naf", "medecine_travail"] {
        assert_eq!(e[k], id[k], "{}", k);
    }
    assert_eq!(e["siret"], json!("98765432400019"));
    // Le declarant tel qu'inscrit (un prenom compose reste entier) ; le compte DPAE prend le SIRET de la compagnie.
    assert_eq!(e["declarant"], json!({"siret": "98765432400019", "nom": "QUENOUILLE", "prenom": "ARIANE MARIE"}));
    assert_eq!(e["declarant_dsn"], id["declarant_dsn"]);
    assert_eq!(e["api_dpae"], json!({"nom": "QUENOUILLEDPAE", "prenom": "ARIANE", "siret": "98765432400019"}));
    // Rien de la fictive ne dit plus son identite : ni sa raison sociale datee (`historique`), ni son affichage.
    let source = lire(&fictive(FICTIVE), "config/entreprise.json");
    let texte = e.to_string();
    for k in ["raison_sociale", "siret", "adresse", "commune", "adresse_affichage", "commune_affichage"] {
        if let Some(v) = source[k].as_str() {
            assert!(!texte.contains(v), "{} de la fictive est reste dans la fiche", k);
        }
    }
    for h in e["historique"].as_array().unwrap() {
        assert_ne!(h["champ"], json!("raison_sociale"), "{}", h);
    }
    // Le marqueur d'une compagnie d'essai, qui n'est pas celui d'une fictive.
    let m = lire(&d, "COPIE-DE-DEVELOPPEMENT.json");
    assert!(m["copie_de"].as_str().unwrap().starts_with("compagnie d'essai"), "{}", m);
    assert!(paie::essai::est_essai(&d));
    assert!(!paie::interface::compagnies::est_fictive(&d));
    // Ni les retours de la fictive (ceux de SON SIREN), ni ses depots de DSN ; ses DPAE et ses contrats signes restent.
    assert!(!d.join("retours").exists());
    let journal = lire(&d, "journal/depots.json");
    let quoi: Vec<&str> = journal.as_array().unwrap().iter().filter_map(|l| l["quoi"].as_str()).collect();
    assert!(!quoi.contains(&"dsn"), "un depot de DSN de la fictive est reste au journal");
    assert!(quoi.contains(&"dpae") && quoi.contains(&"contrat-recu"));
    assert!(std::fs::read_to_string(d.join(".gitignore")).unwrap().lines().any(|l| l.trim() == "*"));
    let _ = std::fs::remove_dir_all(&d);
}

/// Monter de nouveau une compagnie d'essai garde ce qu'elle a fait (journal, sorties) et ne reecrit que son identite.
#[test]
fn remonter_une_compagnie_d_essai_ne_reecrit_que_son_identite() {
    let d = montee(PAIE, "remonter");
    std::fs::write(d.join("sorties/trace-de-l-essai.txt"), "garde").unwrap();
    let mut id = identite();
    id["declarant_dsn"]["qualite"] = json!("05");
    let s = monter_avec(PAIE, &d, Some(&id.to_string()));
    assert_eq!(s.code, 0, "{}", s.texte);
    rien_d_affiche(&s, &id);
    assert!(s.texte.contains("identite reecrite"), "{}", s.texte);
    assert_eq!(lire(&d, "config/entreprise.json")["declarant_dsn"]["qualite"], json!("05"));
    assert_eq!(std::fs::read_to_string(d.join("sorties/trace-de-l-essai.txt")).unwrap(), "garde");
    let _ = std::fs::remove_dir_all(&d);
}

// ------------------------------------------------------------------ la DSN et la DPAE, sous cette identite

/// La DSN de septembre s'ecrit sous l'identite de la variable, et la sortie de `paie mois` n'en montre rien.
fn dsn_de_septembre(binaire: &str, d: &Path) -> String {
    let s = lancer(binaire, Some(d), &["mois", "paies/2026-09.json"], &[]);
    assert_eq!(s.code, 0, "{}", s.texte);
    rien_d_affiche(&s, &identite());
    std::fs::read_to_string(d.join("sorties/2026-09/DSN_Septembre2026.dsn")).unwrap()
}

#[test]
fn la_dsn_du_mois_s_ecrit_sous_l_identite_de_la_variable() {
    let d = montee(PAIE, "dsn");
    let v = rubriques(&dsn_de_septembre(PAIE, &d));
    let id = identite();
    for (r, k) in [("S10.G00.01.001", "siren"), ("S10.G00.01.002", "nic"), ("S10.G00.01.003", "raison_sociale"), ("S21.G00.06.001", "siren"), ("S21.G00.06.002", "nic")] {
        assert_eq!(valeur(&v, r), id[k].as_str().unwrap(), "{}", r);
    }
    for (r, k) in [("S10.G00.02.001", "civilite"), ("S10.G00.02.002", "nom_prenom"), ("S10.G00.02.004", "email"), ("S20.G00.07.004", "qualite")] {
        assert_eq!(valeur(&v, r), id["declarant_dsn"][k].as_str().unwrap(), "{}", r);
    }
    // Des individus : la fictive a du monde chaque mois.
    assert!(v.iter().any(|(r, _)| r == "S21.G00.30.001"));
    let _ = std::fs::remove_dir_all(&d);
}

/// Le depot de TEST va au reseau (pas joue comme dans une fictive) : le faux mandataire voit la demande vers net-entreprises
/// et la refuse, rien ne sort. Le SIREN ne s'affiche pas. Le depot REEL est refuse avant toute demande au reseau.
#[test]
fn le_depot_de_test_de_la_dsn_va_au_reseau_et_le_reel_est_refuse() {
    let d = montee(PAIE, "depot-dsn");
    dsn_de_septembre(PAIE, &d);
    let fichier = "sorties/2026-09/DSN_Septembre2026.dsn";
    let (proxy, vus) = mandataire();
    let env = [("NET_ENTREPRISES_MOTDEPASSE", FAUX_MOTDEPASSE), (paie::garde_reseau::VARIABLE, proxy.as_str())];
    let s = lancer(PAIE, Some(&d), &["depot-dsn", fichier, "--test"], &env);
    rien_d_affiche(&s, &identite());
    assert!(s.texte.contains("SIREN de la compagnie d'essai (ne s'affiche pas)"), "{}", s.texte);
    let v = vus.lock().unwrap().clone();
    assert!(v.iter().any(|l| l.starts_with("CONNECT services.net-entreprises.fr:443")), "le test n'est pas alle au reseau : {:?}\n{}", v, s.texte);
    assert!(!s.texte.contains("FICTIF-"), "le depot a ete joue : {}", s.texte);

    vus.lock().unwrap().clear();
    let s = lancer(PAIE, Some(&d), &["depot-dsn", fichier, "--reel", "--confirmer-mois", "2026-09"], &env);
    assert_eq!(s.code, 2, "{}", s.texte);
    assert!(s.texte.contains("Refus") && s.texte.contains("Rien n'est parti"), "{}", s.texte);
    rien_d_affiche(&s, &identite());
    assert!(vus.lock().unwrap().is_empty(), "un depot reel est alle au reseau : {:?}", vus.lock().unwrap());
    let _ = std::fs::remove_dir_all(&d);
}

/// Une DSN copiee de la fictive (son SIREN) ne part pas d'une compagnie d'essai, et le refus ne cite aucun numero.
#[test]
fn une_dsn_ecrite_sous_l_identite_de_la_fictive_ne_part_pas() {
    let d = montee(PAIE, "dsn-de-la-fictive");
    let (proxy, vus) = mandataire();
    let s = lancer(PAIE, Some(&d), &["depot-dsn", "sorties/2026-08/DSN_Aout2026.dsn", "--test"], &[("NET_ENTREPRISES_MOTDEPASSE", FAUX_MOTDEPASSE), (paie::garde_reseau::VARIABLE, proxy.as_str())]);
    assert_eq!(s.code, 2, "{}", s.texte);
    assert!(s.texte.contains("ne porte pas le SIREN de la compagnie"), "{}", s.texte);
    let siren_fictive = lire(&fictive(FICTIVE), "config/entreprise.json")["siren"].as_str().unwrap().to_string();
    assert_eq!(s.texte.matches(&siren_fictive).count(), 0, "{}", s.texte);
    assert!(vus.lock().unwrap().is_empty(), "{:?}", vus.lock().unwrap());
    let _ = std::fs::remove_dir_all(&d);
}

/// `--ordre` ne vaut que pour le test d'une mensuelle : avec `--reel`, la commande refuse avant tout.
#[test]
fn l_ordre_d_essai_n_est_admis_qu_en_test() {
    let d = montee(PAIE, "ordre-reel");
    dsn_de_septembre(PAIE, &d);
    let (proxy, vus) = mandataire();
    for args in [&["--reel", "--ordre", "5"][..], &["--test", "--ordre", "0"][..], &["--test", "--ordre", "cinq"][..]] {
        let mut a = vec!["depot-dsn", "sorties/2026-09/DSN_Septembre2026.dsn"];
        a.extend_from_slice(args);
        let s = lancer(PAIE, Some(&d), &a, &[(paie::garde_reseau::VARIABLE, proxy.as_str())]);
        assert_eq!(s.code, 2, "{:?} : {}", args, s.texte);
        assert!(s.texte.contains("--ordre N"), "{}", s.texte);
    }
    assert!(vus.lock().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&d);
}

/// La DPAE s'ecrit sous l'identite de la variable (SIRET, medecine du travail) et son test va a l'Urssaf.
#[test]
fn la_dpae_s_ecrit_sous_l_identite_et_son_test_va_a_l_urssaf() {
    let d = montee(PAIE, "dpae");
    let s = lancer(PAIE, Some(&d), &["dpae", "paies/2026-11.json"], &[]);
    assert_eq!(s.code, 0, "{}", s.texte);
    rien_d_affiche(&s, &identite());
    let dpae = std::fs::read_to_string(d.join("sorties/2026-11/DPAE_Novembre2026.txt")).unwrap();
    assert!(dpae.contains("98765432400019") && dpae.contains("MTESSAI7"), "la DPAE ne porte pas l'identite de la variable");
    let (proxy, vus) = mandataire();
    let s = lancer(PAIE, Some(&d), &["deposer-dpae", "sorties/2026-11/DPAE_Novembre2026.txt", "--test"], &[("URSSAF_MOTDEPASSE", FAUX_MOTDEPASSE), (paie::garde_reseau::VARIABLE, proxy.as_str())]);
    rien_d_affiche(&s, &identite());
    let v = vus.lock().unwrap().clone();
    assert!(v.iter().any(|l| l.starts_with("CONNECT mon.urssaf.fr:443")), "le test n'est pas alle a l'Urssaf : {:?}\n{}", v, s.texte);
    let _ = std::fs::remove_dir_all(&d);
}

// ================================================================== paie-atelier, contre un faux serveur local
#[cfg(feature = "atelier")]
mod atelier {
    use super::*;
    use crate::outils::{http, serveur, Recu};
    use std::io::Read;

    const ATELIER: &str = env!("CARGO_BIN_EXE_paie-atelier");

    /// Un faux net-entreprises et un faux Urssaf : l'authentification reussit, le depot rend un flux.
    fn faux() -> (String, Recu) {
        serveur(|q| {
            Some(if q.chemin.contains("authentifier") {
                http(200, "text/plain", b"JETON-FICTIF")
            } else if q.chemin.contains("deposer-dsn") {
                http(200, "text/plain", b"<idflux>FLUX-ESSAI</idflux>")
            } else {
                http(404, "text/plain", b"Aucun retour")
            })
        })
    }

    fn degonfle(octets: &[u8]) -> String {
        let mut t = String::new();
        flate2::read::GzDecoder::new(octets).read_to_string(&mut t).unwrap();
        t
    }

    /// Le depot de test arrive au faux net-entreprises : la DSN porte l'identite de la variable, l'indicateur d'essai, et, avec
    /// `--ordre 7`, part en annule-et-remplace a l'ordre 7 ; le journal note l'essai et son ordre.
    #[test]
    fn le_depot_de_test_arrive_au_faux_serveur_sous_l_identite_et_a_l_ordre_donne() {
        let d = montee(ATELIER, "atelier-dsn");
        dsn_de_septembre(ATELIER, &d);
        let (base, recu) = faux();
        let dsn = format!("{}/dsn", base);
        let s = lancer(
            ATELIER,
            Some(&d),
            &["depot-dsn", "sorties/2026-09/DSN_Septembre2026.dsn", "--test", "--ordre", "7"],
            &[("NET_ENTREPRISES_MOTDEPASSE", FAUX_MOTDEPASSE), ("PUPPETPAIE_ATELIER_DSN", &dsn)],
        );
        assert_eq!(s.code, 0, "{}", s.texte);
        rien_d_affiche(&s, &identite());
        let recus = recu.lock().unwrap().clone();
        assert!(recus.iter().any(|q| q.chemin.ends_with("/dsn/authentifier/")), "{:?}", recus.iter().map(|q| &q.chemin).collect::<Vec<_>>());
        let depot = recus.iter().find(|q| q.chemin.ends_with("/dsn/deposer-dsn/1.0/")).expect("aucun depot");
        let v = rubriques(&degonfle(&depot.corps));
        assert_eq!(valeur(&v, "S10.G00.01.001"), "987654324");
        assert_eq!(valeur(&v, "S10.G00.00.005"), "01", "le depot n'est pas un essai");
        assert_eq!(valeur(&v, "S20.G00.05.002"), "03", "un essai a un ordre donne part en annule-et-remplace");
        assert_eq!(valeur(&v, "S20.G00.05.004"), "7");
        let j = lire(&d, "journal/depots.json");
        let l = j.as_array().unwrap().iter().rev().find(|l| l["quoi"] == json!("dsn")).unwrap().clone();
        assert_eq!(l["mode"], json!("test"), "{}", l);
        assert_eq!(l["detail"]["ordre_test"], json!(7), "{}", l);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Le depot de test de la DPAE arrive au faux Urssaf, avec le compte DPAE de la variable.
    #[test]
    fn le_depot_de_test_de_la_dpae_arrive_au_faux_urssaf() {
        let d = montee(ATELIER, "atelier-dpae");
        let s = lancer(ATELIER, Some(&d), &["dpae", "paies/2026-11.json"], &[]);
        assert_eq!(s.code, 0, "{}", s.texte);
        let (base, recu) = faux();
        let dpae = format!("{}/dpae", base);
        let s = lancer(
            ATELIER,
            Some(&d),
            &["deposer-dpae", "sorties/2026-11/DPAE_Novembre2026.txt", "--test"],
            &[("URSSAF_MOTDEPASSE", FAUX_MOTDEPASSE), ("PUPPETPAIE_ATELIER_DPAE", &dpae)],
        );
        assert_eq!(s.code, 0, "{}", s.texte);
        rien_d_affiche(&s, &identite());
        let chemins: Vec<String> = recu.lock().unwrap().iter().map(|q| q.chemin.clone()).collect();
        assert!(chemins.iter().any(|c| c.ends_with("/dpae/authentifier_dpae")), "{:?}", chemins);
        assert!(chemins.iter().any(|c| c.ends_with("/dpae/deposer-dsn/1.0/")), "{:?}", chemins);
        let j = lire(&d, "journal/depots.json");
        assert!(j.as_array().unwrap().iter().any(|l| l["quoi"] == json!("dpae") && l["mode"] == json!("test") && l["mois"] == json!("2026-11")), "{}", j);
        let _ = std::fs::remove_dir_all(&d);
    }
}
