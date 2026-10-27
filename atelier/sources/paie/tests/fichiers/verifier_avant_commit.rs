//! Le garde avant commit (`paie verifier-avant-commit`, `src/verifier_avant_commit.rs`), sur un faux dépôt Git monté dans
//! le dossier de test, avec un faux `prive/config/` : des valeurs inventées (le NIR porte la marque d'invention, l'IBAN et
//! le SIRET se composent à la volée pour que ce fichier n'en porte aucun). Jamais le vrai `prive/`.
//!
//! Le contrat : une valeur lue dans `prive/config/` (ou un NIR, un IBAN, un BIC sans la marque d'invention) dans ce que
//! Git emporterait arrête le commit (code 1), et la sortie dit le fichier et la ligne, jamais la valeur ; sans elle, il
//! passe (code 0) ; `FICTIVES` et `EXEMPLES_DES_TEXTES` sont honorés ; `--indexes` juge ce que l'index porte.
//!
//! Le faux dépôt n'a pas de `.gitignore` : `prive/` y est un dossier non suivi que Git listerait, et que le garde écarte
//! de lui-même.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use paie::verifier_avant_commit::{EXEMPLES_DES_TEXTES, FICTIVES};
use serde_json::json;

use crate::outils::dossier_de_test;

/// Inventé : la marque `999` dans ses treize chiffres.
const NIR_INVENTE: &str = "285129991234567";
const COURRIEL: &str = "camille.fictive@exemple.invalid";
const NOM: &str = "FICTIVENOM";
const PRENOM: &str = "Camille";

/// Un SIRET et un IBAN à la forme juste, composés à la volée.
fn siret() -> String {
    format!("{}{}", "4242", "4242424242")
}

fn iban() -> String {
    format!("FR{}{}", "76", "11112222333344445555666")
}

struct Sortie {
    code: i32,
    out: String,
}

/// Le dossier `depot/` (le faux dépôt Git) et `home/` (un HOME vide : ni la configuration de Git de l'utilisateur, ni
/// ses exclusions).
fn monter(test: &str, avec_prive: bool) -> PathBuf {
    let racine = dossier_de_test("verifier_avant_commit", test);
    let depot = racine.join("depot");
    std::fs::create_dir_all(&depot).unwrap();
    std::fs::create_dir_all(racine.join("home")).unwrap();
    git(&depot, &["init", "-q"]);
    if avec_prive {
        let cfg = depot.join("prive/config");
        std::fs::create_dir_all(&cfg).unwrap();
        let entreprise = json!({"siret": siret(), "siren": "42", "banque": {"iban": iban(), "bic": ""}});
        let salaries = json!({
            "_commentaire": "inventés",
            "FICTIVENOM": {"nom": NOM, "prenom": PRENOM, "nir": NIR_INVENTE, "email": COURRIEL, "adresse": "1 rue"},
        });
        std::fs::write(cfg.join("entreprise.json"), entreprise.to_string()).unwrap();
        std::fs::write(cfg.join("salaries.json"), salaries.to_string()).unwrap();
    }
    depot
}

fn environnement(c: &mut Command, depot: &Path) {
    c.env_clear()
        .env(socle::garde_reseau::VARIABLE, "1") // le garde-fou reseau des essais : rien ne sort de la machine
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", depot.parent().unwrap().join("home"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .current_dir(depot)
        .stdin(Stdio::null());
}

fn git(depot: &Path, args: &[&str]) {
    let mut c = Command::new("git");
    environnement(&mut c, depot);
    let s = c.args(args).output().expect("git introuvable");
    assert!(s.status.success(), "git {:?} : {}", args, String::from_utf8_lossy(&s.stderr));
}

fn ecrire(depot: &Path, rel: &str, contenu: &str) {
    let p = depot.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, contenu).unwrap();
}

fn garde(depot: &Path, args: &[&str]) -> Sortie {
    let mut c = Command::new(env!("CARGO_BIN_EXE_paie"));
    environnement(&mut c, depot);
    let s = c.arg("verifier-avant-commit").args(args).output().unwrap();
    Sortie { code: s.status.code().unwrap_or(-1), out: String::from_utf8_lossy(&s.stdout).into_owned() }
}

/// Les valeurs de `prive/config/` ne sont jamais dans ce que la commande affiche.
fn sans_les_valeurs(s: &Sortie) {
    for v in [COURRIEL.to_string(), NOM.to_string(), NOM.to_lowercase(), siret(), iban(), NIR_INVENTE.to_string()] {
        assert!(!s.out.contains(&v), "la sortie du garde porte une valeur :\n{}", s.out);
    }
}

#[test]
fn une_valeur_de_prive_dans_ce_qui_part_arrete_le_commit() {
    let depot = monter("refus", true);
    let texte = format!("rien\nécrire à {}\n{} {} est dans un test\nle siret {} ici\n", COURRIEL, NOM.to_lowercase(), PRENOM.to_lowercase(), siret());
    ecrire(&depot, "atelier/notes.md", &texte);
    git(&depot, &["add", "atelier/notes.md"]);
    for args in [&[][..], &["--indexes"][..]] {
        let s = garde(&depot, args);
        assert_eq!(s.code, 1, "{:?} :\n{}", args, s.out);
        assert!(s.out.contains("atelier/notes.md"), "{}", s.out);
        for (ligne, quoi) in [(2, "salarie.email"), (3, "salarie (identite)"), (4, "siret")] {
            assert!(s.out.lines().any(|l| l.contains(&format!("ligne {} ", ligne)) && l.contains(quoi)), "ligne {} ({}) :\n{}", ligne, quoi, s.out);
        }
        sans_les_valeurs(&s);
    }
}

#[test]
fn sans_la_valeur_le_commit_passe() {
    let depot = monter("passe", true);
    ecrire(&depot, "atelier/notes.md", "rien de personnel\n");
    // Un fichier qui n'est pas du texte ne se lit pas ; `prive/` (non suivi, sans `.gitignore`) non plus.
    ecrire(&depot, "logiciel/image.png", COURRIEL);
    git(&depot, &["add", "atelier/notes.md", "logiciel/image.png"]);
    for args in [&[][..], &["--indexes"][..]] {
        let s = garde(&depot, args);
        assert_eq!(s.code, 0, "{:?} :\n{}", args, s.out);
    }
}

#[test]
fn le_nir_d_un_salarie_s_arrete_meme_avec_la_marque_d_invention() {
    // La marque excuse un NIR trouvé par le motif générique, jamais un NIR lu dans `prive/config/`.
    let depot = monter("nir_lu", true);
    ecrire(&depot, "fictives/x/config/salaries.json", &format!("{{\"nir\": \"{}\"}}\n", NIR_INVENTE));
    let s = garde(&depot, &[]);
    assert_eq!(s.code, 1, "{}", s.out);
    assert!(s.out.contains("salarie.nir"), "{}", s.out);
    sans_les_valeurs(&s);
}

#[test]
fn les_indexes_jugent_ce_que_l_index_porte() {
    let depot = monter("index", true);
    // Indexé avec la valeur, puis nettoyé dans la copie de travail sans réindexer : le commit l'emporterait.
    ecrire(&depot, "atelier/a.md", &format!("{}\n", COURRIEL));
    git(&depot, &["add", "atelier/a.md"]);
    ecrire(&depot, "atelier/a.md", "propre\n");
    assert_eq!(garde(&depot, &["--indexes"]).code, 1);
    assert_eq!(garde(&depot, &[]).code, 0);
    // L'inverse : indexé propre, sali ensuite sans réindexer : le commit ne l'emporterait pas.
    git(&depot, &["add", "atelier/a.md"]);
    ecrire(&depot, "atelier/a.md", &format!("{}\n", COURRIEL));
    assert_eq!(garde(&depot, &["--indexes"]).code, 0);
    assert_eq!(garde(&depot, &[]).code, 1);
}

#[test]
fn sans_prive_les_motifs_generiques_suffisent() {
    let depot = monter("generiques", false);
    let sans_marque = format!("{}{}", "18503754", "12387");
    ecrire(&depot, "atelier/a.md", &format!("le NIR {}\n", sans_marque));
    ecrire(&depot, "atelier/b.md", &format!("l'IBAN {}\n", iban()));
    let s = garde(&depot, &[]);
    assert_eq!(s.code, 1, "{}", s.out);
    assert!(s.out.lines().any(|l| l.contains("ligne 1 ") && l.contains("NIR")), "{}", s.out);
    assert!(s.out.lines().any(|l| l.contains("ligne 1 ") && l.contains("IBAN")), "{}", s.out);
    assert!(!s.out.contains(&sans_marque) && !s.out.contains(&iban()), "{}", s.out);
}

#[test]
fn les_fictives_et_les_exemples_des_textes_passent() {
    let depot = monter("exceptions", true);
    // Les valeurs fabriquées de `FICTIVES`, et un NIR qui porte la marque d'invention (hors de `prive/config/`).
    let fictives: Vec<&str> = FICTIVES.iter().map(|f| f.0).collect();
    ecrire(&depot, "fictives/x/config/entreprise.json", &format!("{}\n1850675123456\n", fictives.join("\n")));
    // Un IBAN qui n'est pas dans `FICTIVES`, dans un texte de référence nommé dans `EXEMPLES_DES_TEXTES`.
    let exemple = EXEMPLES_DES_TEXTES[0].0;
    ecrire(&depot, exemple, &format!("exemple : {}\n", format!("FR{}{}", "99", "12341234123412341234123")));
    let s = garde(&depot, &[]);
    assert_eq!(s.code, 0, "{}", s.out);
    // Le même IBAN ailleurs s'arrête ; dans l'exemple, une valeur lue dans `prive/config/` s'arrête aussi.
    ecrire(&depot, "atelier/copie.md", &format!("{}\n", format!("FR{}{}", "99", "12341234123412341234123")));
    ecrire(&depot, exemple, &format!("{}\n", COURRIEL));
    let s = garde(&depot, &[]);
    assert_eq!(s.code, 1, "{}", s.out);
    assert!(s.out.contains("atelier/copie.md") && s.out.contains(exemple), "{}", s.out);
}

#[test]
fn une_configuration_illisible_ne_laisse_pas_passer() {
    let depot = monter("illisible", true);
    std::fs::write(depot.join("prive/config/salaries.json"), "{ pas du JSON").unwrap();
    ecrire(&depot, "atelier/a.md", "propre\n");
    assert_eq!(garde(&depot, &[]).code, 2);
}
