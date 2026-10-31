//! Les aides des tests, écrites une fois : les dossiers, les oracles, les compagnies, la route en sous-processus et son
//! bac à sable, les lecteurs de DSN, l'AEM, le faux serveur HTTP, le faux mandataire.
//!
//! Déclaré par chaque `main.rs` de groupe (`#[path = "../outils/mod.rs"] mod outils;`) et par les fichiers seuls
//! (`#[path = "outils/mod.rs"] mod outils;`).

#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use rust_decimal::Decimal;
use serde_json::{json, Value};

// ------------------------------------------------------------------ les dossiers

/// Les données livrées (`logiciel/donnees`).
pub fn code_livre() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../logiciel/donnees")
}

pub fn copier(src: &Path, dst: &Path) {
    moteur::compagnie::incidents::brancher_l_engagement();
    std::fs::create_dir_all(dst).unwrap();
    for e in std::fs::read_dir(src).unwrap().flatten() {
        let (s, d) = (e.path(), dst.join(e.file_name()));
        if s.is_dir() {
            copier(&s, &d);
        } else {
            std::fs::copy(&s, &d).unwrap();
        }
    }
}

/// Une INSTALLATION : `<dossier>/logiciel/` avec les données livrées et `VERSION`. La mise à jour est la sienne
/// (`<dossier>/maj/`) ; un `moteur` la trouve par `PUPPETPAIE_CODE` = `code_de(<dossier>)`. Rend `<dossier>`.
pub fn installation(dossier: &Path, version: &str) -> PathBuf {
    copier(&code_livre(), &dossier.join("logiciel").join("donnees"));
    std::fs::write(dossier.join("logiciel").join("VERSION"), format!("{}\n", version)).unwrap();
    dossier.to_path_buf()
}

/// Le geste de l'accueil sur une installation de test : `.edition.json` « debloque » a cote de `logiciel/`. Sans lui,
/// une installation est BRIDEE (`moteur/src/compagnie/bridage.rs`) : elle ne propose ni ne rattache de compagnie reelle.
/// Un test qui fait vivre des compagnies reelles dans son installation la debloque ainsi, et le dit.
pub fn debloquer(installation: &Path) {
    std::fs::create_dir_all(installation).unwrap();
    std::fs::write(installation.join(".edition.json"), "{\"edition\": \"debloque\"}\n").unwrap();
}

/// Le `PUPPETPAIE_CODE` d'une installation.
pub fn code_de(installation: &Path) -> String {
    installation.join("logiciel").join("donnees").display().to_string()
}

fn neuf(d: PathBuf) -> PathBuf {
    // Ce que pose toute commande du moteur avant de toucher a un engagement : le lecteur de ce qui est parti.
    moteur::compagnie::incidents::brancher_l_engagement();
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// `CARGO_TARGET_TMPDIR/<fichier>/<test>`, vidé puis créé : le dossier d'un test, à lui seul.
pub fn dossier_de_test(fichier: &str, test: &str) -> PathBuf {
    neuf(Path::new(env!("CARGO_TARGET_TMPDIR")).join(fichier).join(test))
}

/// `CARGO_TARGET_TMPDIR/<nom>-<pid>`, vidé puis créé : un modèle monté une fois par processus (sous un `OnceLock`), puis
/// seulement lu ou recopié.
pub fn dossier_du_processus(nom: &str) -> PathBuf {
    neuf(Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{}-{}", nom, std::process::id())))
}

/// `CARGO_TARGET_TMPDIR/uniques/<pid>-<n>-<nom>` : un fichier que nul autre appel, ni autre processus, ne prend.
pub fn chemin_unique(nom: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("uniques");
    std::fs::create_dir_all(&d).unwrap();
    d.join(format!("{}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst), nom))
}

pub fn lire(racine: &Path, rel: &str) -> Value {
    serde_json::from_slice(&std::fs::read(racine.join(rel)).unwrap()).unwrap_or_else(|e| panic!("{} illisible : {}", rel, e))
}

pub fn ecrire(racine: &Path, rel: &str, v: &Value) {
    let p = racine.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, serde_json::to_vec_pretty(v).unwrap()).unwrap();
}

/// Une fiche de la configuration datee (`contexte::fiche_au`), lue a `jour` (AAAA-MM-JJ).
pub fn fiche_au(datee: &Value, jour: &str) -> Value {
    droit::contexte::fiche_au(datee, &socle::date::Jour::lire(jour).unwrap(), "test").unwrap()
}

/// La fiche datee d'une compagnie de test : chaque valeur de `plate` de tout temps (`contexte::fiche_datee`).
pub fn datee(plate: &Value) -> Value {
    Value::Object(droit::contexte::fiche_datee(plate.as_object().unwrap()))
}

/// Les noms des fichiers d'un dossier, triés (vide s'il n'existe pas).
pub fn noms(dossier: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dossier).map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
    v.sort();
    v
}

// ------------------------------------------------------------------ les compagnies

pub mod compagnie {
    use super::*;

    /// La compagnie réduite aux engagements de `salaries` (leur nom dans la saisie), dans chaque mois : un cas qui ne
    /// regarde qu'eux ne calcule pas les autres.
    pub fn seulement(racine: &Path, salaries: &[&str]) {
        for f in std::fs::read_dir(racine.join("engagements")).unwrap().flatten() {
            let rel = format!("engagements/{}", f.file_name().to_string_lossy());
            let mut mois = super::lire(racine, &rel);
            if let Some(l) = mois["engagements"].as_array_mut() {
                l.retain(|e| salaries.iter().any(|s| e["salarie"] == *s));
            }
            super::ecrire(racine, &rel, &mois);
        }
    }


}


/// La saisie de `mois` telle que ses engagements la montrent (`engagement::vue_requise`).
pub fn saisie(racine: &Path, mois: &str) -> Value {
    let mut v = engagement::vue_requise(racine, mois).unwrap();
    v["mois"] = Value::String(mois.to_string());
    v
}

/// « Valider l'engagement » sur la carte du premier engagement en saisie de `salarie` que l'ecran de `mois` montre,
/// telle qu'elle est enregistree (son identite, `saisie`).
pub fn valider_la_carte(racine: &Path, mois: &str, salarie: &str, jour: &str) -> Value {
    let vue = engagement::stock::vue_du_mois(racine, mois).unwrap().unwrap();
    let es = vue["engagements"].as_array().unwrap();
    let k = es.iter().position(|e| e["salarie"] == salarie && e.get("valide").is_none()).expect("une carte en saisie");
    route(racine, "valider-engagement", json!({"mois": mois, "engagement": identite_a(racine, mois, k), "saisie": es[k]}), jour)
}

/// L'identite (`engagement::stock::identites`) de la carte du premier engagement de `salarie` dans l'ecran de `mois`
/// (de numero `numero` s'il est dit).
pub fn identite(racine: &Path, mois: &str, salarie: &str, numero: Option<&str>) -> Value {
    let vue = engagement::stock::vue_du_mois(racine, mois).unwrap().unwrap();
    let k = vue["engagements"].as_array().unwrap().iter().position(|e| e["salarie"] == salarie && numero.is_none_or(|n| e["numero"] == n)).expect("la carte");
    identite_a(racine, mois, k)
}

/// L'identite de la `k`-ieme carte de l'ecran de `mois`.
pub fn identite_a(racine: &Path, mois: &str, k: usize) -> Value {
    engagement::stock::identites(&engagement::stock::objets_du_mois(racine, mois).unwrap()).swap_remove(k)
}







/// Le rejeu d'un scenario du banc d'essai et le releve de ses sorties.
pub mod rejeu;

// ------------------------------------------------------------------ la route

/// Les variables qu'aucun sous-processus de test n'hérite du poste : les secrets, le bac à sable des incidents, le code.
pub const RETIREES: [&str; 7] =
    ["COURRIEL_COMPTE", "COURRIEL_MOTDEPASSE", "NET_ENTREPRISES_MOTDEPASSE", "URSSAF_MOTDEPASSE", "PUPPETPAIE_INCIDENTS", "PUPPETPAIE_MOTDEPASSE", "PUPPETPAIE_CODE"];

/// Le bac à sable d'un `moteur` lancé par un test : la compagnie, une clé de poste qui n'existe pas, le jour ; aucun secret du
/// poste ; le code copié à côté (`compagnie::avec_le_code`) s'il y en a un.
pub fn bac_a_sable(c: &mut Command, racine: &Path, jour: &str) {
    for v in RETIREES {
        c.env_remove(v);
    }
    c.env("PUPPETPAIE_AUJOURDHUI", jour).env("PUPPETPAIE_DONNEES", racine).env("PUPPETPAIE_CLE", racine.join("pas-de-cle"));
    let code = racine.parent().map(|p| p.join("donnees"));
    if let Some(code) = code.filter(|c| c.is_dir()) {
        c.env("PUPPETPAIE_CODE", code);
    }
}

/// `moteur route` dans un sous-processus : la réponse entière (`{"ok", "resultat"}` ou `{"ok": false, "erreur", "classe"}`).
pub fn route(racine: &Path, nom: &str, charge: Value, jour: &str) -> Value {
    route_avec(racine, nom, charge, jour, &[])
}

/// La même, avec des variables posées par le cas après le bac à sable (une boîte factice, un autre `PUPPETPAIE_AUJOURDHUI`…).
pub fn route_avec(racine: &Path, nom: &str, charge: Value, jour: &str, env: &[(&str, &str)]) -> Value {
    let mut c = Command::new(env!("CARGO_BIN_EXE_moteur"));
    c.args(["route", "--racine", racine.to_str().unwrap(), "--date", jour]).stdin(Stdio::piped()).stdout(Stdio::piped());
    bac_a_sable(&mut c, racine, jour);
    c.envs(env.iter().copied());
    let mut p = c.spawn().unwrap();
    p.stdin.take().unwrap().write_all(json!({"route": nom, "charge": charge}).to_string().as_bytes()).unwrap();
    let sortie = p.wait_with_output().unwrap().stdout;
    serde_json::from_slice(&sortie).unwrap_or_else(|e| panic!("reponse illisible ({}) : {}", e, String::from_utf8_lossy(&sortie)))
}


pub fn reussi(r: &Value) -> &Value {
    assert_eq!(r["ok"], json!(true), "une reussite etait attendue : {}", r);
    &r["resultat"]
}

/// Un refus : (classe, phrase).
pub fn refus(r: &Value) -> (String, String) {
    assert_eq!(r["ok"], json!(false), "un refus etait attendu : {}", r);
    (r["classe"].as_str().unwrap_or_default().to_string(), r["erreur"].as_str().unwrap_or_default().to_string())
}

// ------------------------------------------------------------------ lire une DSN

pub fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap_or_else(|_| panic!("montant illisible : {:?}", s))
}

/// Les rubriques d'une DSN, dans l'ordre : (code, valeur sans apostrophes).
pub fn paires(dsn: &str) -> Vec<(String, String)> {
    dsn.lines().filter_map(|l| l.trim().split_once(',')).map(|(k, v)| (k.to_string(), v.trim().trim_matches('\'').to_string())).collect()
}

/// La première valeur de la rubrique.
pub fn valeur(v: &[(String, String)], rubrique: &str) -> Option<String> {
    v.iter().find(|(k, _)| k == rubrique).map(|(_, x)| x.clone())
}

pub fn porte(dsn: &str, rubrique: &str) -> bool {
    dsn.lines().any(|l| l.trim().starts_with(&format!("{},", rubrique)))
}

/// Le bloc 30 de l'individu `nom` (30.002) et tout ce qui le suit jusqu'à l'individu suivant.
pub fn individu(dsn: &str, nom: &str) -> Vec<(String, String)> {
    let (mut dedans, mut out) = (false, Vec::new());
    for (k, v) in paires(dsn) {
        if k == "S21.G00.30.001" {
            dedans = false;
        }
        if k == "S21.G00.30.002" {
            dedans = v == nom;
        }
        if dedans {
            out.push((k, v));
        }
    }
    out
}

/// Le bloc 40 du contrat `numero` et ses versements, jusqu'au contrat ou à l'individu suivant.
pub fn contrat(dsn: &str, numero: &str) -> Vec<(String, String)> {
    let lignes = paires(dsn);
    let n = lignes.iter().position(|(k, v)| k == "S21.G00.40.009" && v == numero).unwrap_or_else(|| panic!("pas de contrat {}", numero));
    let i = lignes[..n].iter().rposition(|(k, _)| k == "S21.G00.40.001").expect("le 40.001 du contrat");
    let fin = lignes[n + 1..].iter().position(|(k, _)| k == "S21.G00.40.001" || k == "S21.G00.30.001").map_or(lignes.len(), |p| n + 1 + p);
    lignes[i..fin].to_vec()
}

/// Le versement du contrat `numero` : du 50.001 qui précède son 51.010 au versement suivant.
pub fn versement(dsn: &str, numero: &str) -> Vec<(String, String)> {
    let lignes = paires(dsn);
    let mut debut = None;
    for (i, (k, v)) in lignes.iter().enumerate() {
        if k == "S21.G00.50.001" {
            debut = Some(i);
        }
        if k == "S21.G00.51.010" && v == numero {
            let d = debut.expect("un versement avant ses remunerations");
            let fin = lignes[d + 1..].iter().position(|(k, _)| k == "S21.G00.50.001").map_or(lignes.len(), |p| d + 1 + p);
            return lignes[d..fin].to_vec();
        }
    }
    panic!("aucun versement du contrat {} dans la DSN", numero)
}






/// Un enregistrement AEM de 1000 caractères aux offsets que lit `import/dsn_aem.rs` ; `zones` : ce que le cas ajoute
/// (`(0, "AEMEDIC2")`, `(36, "000123")`, le Siret en 50…).
pub fn aem(zones: &[(usize, &str)], nir: &str, debut: &str, fin: &str, objet: &str) -> String {
    let mut l = vec![' '; 1000];
    let nir = format!("{}46", nir);
    let communes = [
        (300, "BRIOCHE"),
        (325, "FANCHON"),
        (375, nir.as_str()),
        (390, "19881104"),
        (617, "N"),
        (618, debut),
        (626, "N"),
        (627, fin),
        (638, objet),
        (650, "024"),
        (653, "05"),
        (655, "00"),
        (657, "05"),
        (659, "000010000"),
        (858, fin),
    ];
    for (a, v) in zones.iter().chain(communes.iter()) {
        for (i, c) in v.chars().enumerate() {
            l[a + i] = c;
        }
    }
    l.into_iter().collect()
}

// ------------------------------------------------------------------ le faux serveur HTTP

#[path = "../../../envois/tests/outils/serveur.rs"]
mod serveur;
#[allow(unused_imports)]
pub use serveur::*;

/// Un faux mandataire, que l'essai nomme comme seule porte du garde-fou reseau (`socle::garde_reseau::VARIABLE`) : il
/// note la première ligne de chaque demande (`CONNECT hote:port HTTP/1.1`) et refuse ; rien ne sort de la machine.
pub fn mandataire() -> (String, Arc<Mutex<Vec<String>>>) {
    let ecoute = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://127.0.0.1:{}", ecoute.local_addr().unwrap().port());
    let vus: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let v2 = vus.clone();
    std::thread::spawn(move || {
        for flux in ecoute.incoming() {
            let Ok(mut s) = flux else { continue };
            let mut tete = Vec::new();
            let mut octet = [0u8; 1];
            while !tete.ends_with(b"\r\n\r\n") {
                match s.read(&mut octet) {
                    Ok(1) => tete.push(octet[0]),
                    _ => break,
                }
            }
            v2.lock().unwrap().push(String::from_utf8_lossy(&tete).lines().next().unwrap_or("").to_string());
            let _ = s.write_all(b"HTTP/1.1 403 Refuse\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        }
    });
    (base, vus)
}

// ------------------------------------------------------------------ les oracles (communs aux trois crates)

#[path = "../../../socle/tests/outils/oracles.rs"]
mod oracles;
pub use oracles::*;
