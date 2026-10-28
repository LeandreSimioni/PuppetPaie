//! Le montage d'une compagnie d'essai (`paie essai monter`) : l'identite lue dans `PUPPETPAIE_COMPAGNIE`. Le contrat du
//! refus : une variable absente, une cle qui manque ou en trop, une forme fausse, un dossier dans un depot Git refusent,
//! nomment la CLE, et rien n'est ecrit ; aucune valeur de la variable n'apparait jamais a l'ecran.
//!
//! Chaque `paie` part de `env_clear`. Le dossier monte est sous le dossier temporaire du systeme, pas sous
//! `CARGO_TARGET_TMPDIR` : le montage refuse tout dossier d'un depot Git, et la cible de Cargo peut en etre un.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use crate::outils::{copier, dossier_de_test, dossier_du_processus};
use serde_json::{json, Value};

const AUJOURDHUI: &str = "2026-10-08";
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

fn lancer(binaire: &str, args: &[&str], env: &[(&str, &str)]) -> Sortie {
    let m = monde();
    let p = Command::new(binaire)
        .env_clear()
        .env(socle::garde_reseau::VARIABLE, "1") // le garde-fou reseau des essais : rien ne sort de la machine
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", m)
        .env("PUPPETPAIE_CLE", m.join("cle"))
        .env("PUPPETPAIE_CODE", m.join("logiciel/donnees"))
        .env("PUPPETPAIE_AUJOURDHUI", AUJOURDHUI)
        .envs(env.iter().copied())
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
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

/// `paie essai monter <d>` avec `variable` dans `PUPPETPAIE_COMPAGNIE`. La source (`--depuis`) est un dossier vide : le
/// montage refuse avant de la lire.
fn monter_avec(binaire: &str, d: &Path, variable: Option<&str>) -> Sortie {
    let source = dossier_de_test("essai", "source");
    let (src, dst) = (source.to_str().unwrap(), d.to_str().unwrap());
    let env: Vec<(&str, &str)> = variable.map(|v| vec![("PUPPETPAIE_COMPAGNIE", v)]).unwrap_or_default();
    lancer(binaire, &["essai", "monter", dst, "--depuis", src], &env)
}

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
