//! Les aides des tests, écrites une fois : les dossiers, les oracles, les compagnies, la route en sous-processus et son
//! bac à sable, les lecteurs de DSN, l'AEM, le faux serveur HTTP, le faux mandataire, le faux `Courriel`.
//!
//! Déclaré par chaque `main.rs` de groupe (`#[path = "../outils/mod.rs"] mod outils;`) et par les fichiers seuls
//! (`#[path = "outils/mod.rs"] mod outils;`).

#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use paie::courriel::{Courriel, ErreurCourriel};
use rust_decimal::Decimal;
use serde_json::{json, Value};

// ------------------------------------------------------------------ les dossiers

/// La copie figée d'une compagnie fictive (`tests/donnees/fictives/<nom>`), qu'aucun test n'écrit.
pub fn fictive(nom: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/donnees/fictives").join(nom)
}

/// Les données livrées (`logiciel/donnees`).
pub fn code_livre() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../logiciel/donnees")
}

pub fn copier(src: &Path, dst: &Path) {
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

/// Une INSTALLATION (lot U1) : `<dossier>/logiciel/` avec les données livrées et `VERSION`. La mise à jour est la sienne
/// (`<dossier>/maj/`) ; un `paie` la trouve par `PUPPETPAIE_CODE` = `code_de(<dossier>)`. Rend `<dossier>`.
pub fn installation(dossier: &Path, version: &str) -> PathBuf {
    copier(&code_livre(), &dossier.join("logiciel").join("donnees"));
    std::fs::write(dossier.join("logiciel").join("VERSION"), format!("{}\n", version)).unwrap();
    dossier.to_path_buf()
}

/// Le geste de l'accueil sur une installation de test : `.edition.json` « debloque » a cote de `logiciel/`. Sans lui,
/// une installation est BRIDEE (ticket #246, `paie/src/bridage.rs`) : elle ne propose ni ne rattache de compagnie reelle.
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

/// Les noms des fichiers d'un dossier, triés (vide s'il n'existe pas).
pub fn noms(dossier: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dossier).map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
    v.sort();
    v
}

// ------------------------------------------------------------------ les compagnies

pub mod compagnie {
    use super::*;

    /// La compagnie fictive entière, copiée dans `racine`.
    pub fn entiere(fictive: &str, racine: &Path) -> PathBuf {
        copier(&super::fictive(fictive), racine);
        racine.to_path_buf()
    }

    /// La compagnie fictive entière sans ses PDF (les bulletins, les contrats : le gros de `sorties/`), pour un cas qui relit
    /// le journal et les DSN déposées.
    pub fn sans_les_pdf(fictive: &str, racine: &Path) -> PathBuf {
        fn copier_sans_pdf(src: &Path, dst: &Path) {
            std::fs::create_dir_all(dst).unwrap();
            for e in std::fs::read_dir(src).unwrap().flatten() {
                let (s, d) = (e.path(), dst.join(e.file_name()));
                if s.is_dir() {
                    copier_sans_pdf(&s, &d);
                } else if s.extension().is_none_or(|x| x != "pdf") {
                    std::fs::copy(&s, &d).unwrap();
                }
            }
        }
        copier_sans_pdf(&super::fictive(fictive), racine);
        racine.to_path_buf()
    }

    /// Les engagements que les `mois` montrent : le fichier de chaque mois ou l'un d'eux commence
    /// (`engagements/AAAA-MM.json`), copie tel quel.
    fn copier_les_engagements(d: &Path, mois: &[&str], racine: &Path) {
        std::fs::create_dir_all(racine.join(engagement::stock::DOSSIER)).unwrap();
        let mut fichiers: Vec<String> = Vec::new();
        for m in mois {
            for o in engagement::stock::objets_du_mois(d, m).unwrap() {
                if !fichiers.contains(&o.fichier) {
                    fichiers.push(o.fichier.clone());
                }
            }
            if engagement::stock::chemin(d, m).is_file() && !fichiers.contains(&m.to_string()) {
                fichiers.push(m.to_string());
            }
        }
        for f in fichiers {
            std::fs::copy(engagement::stock::chemin(d, &f), engagement::stock::chemin(racine, &f)).unwrap();
        }
    }

    /// `config/`, `journal/`, les saisies des `mois` et leurs `sorties/<mois>/` quand il y en a.
    pub fn config_et_mois(fictive: &str, mois: &[&str], racine: &Path) -> PathBuf {
        let d = super::fictive(fictive);
        for sous in ["config", "journal"] {
            if d.join(sous).is_dir() {
                copier(&d.join(sous), &racine.join(sous));
            }
        }
        copier_les_engagements(&d, mois, racine);
        for m in mois {
            let sorties = format!("sorties/{}", m);
            if d.join(&sorties).is_dir() {
                copier(&d.join(&sorties), &racine.join(&sorties));
            }
        }
        racine.to_path_buf()
    }

    /// `config/`, les saisies des `mois` et un journal vide : ni sorties, ni dépôts.
    pub fn saisies(fictive: &str, mois: &[&str], racine: &Path) -> PathBuf {
        let d = super::fictive(fictive);
        copier(&d.join("config"), &racine.join("config"));
        copier_les_engagements(&d, mois, racine);
        ecrire(racine, "journal/depots.json", &json!([]));
        racine.to_path_buf()
    }

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

    /// La compagnie fictive sans les pièces de `sorties/` (bulletins, contrats, DPAE, certificats) : ses DSN déposées, que
    /// relit la DSN d'un mois suivant ; les mois `entiers` gardent tout.
    pub fn ses_dsn(fictive: &str, entiers: &[&str], racine: &Path) -> PathBuf {
        let d = super::fictive(fictive);
        std::fs::create_dir_all(racine).unwrap();
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let (s, nom) = (e.path(), e.file_name());
            if nom == "sorties" {
                continue;
            }
            if s.is_dir() {
                copier(&s, &racine.join(&nom));
            } else {
                std::fs::copy(&s, racine.join(&nom)).unwrap();
            }
        }
        for e in std::fs::read_dir(d.join("sorties")).unwrap().flatten() {
            let (s, dst) = (e.path(), racine.join("sorties").join(e.file_name()));
            if entiers.iter().any(|m| e.file_name() == *m) {
                copier(&s, &dst);
                continue;
            }
            std::fs::create_dir_all(&dst).unwrap();
            for f in std::fs::read_dir(&s).unwrap().flatten().filter(|f| f.path().extension().is_some_and(|x| x == "dsn")) {
                std::fs::copy(f.path(), dst.join(f.file_name())).unwrap();
            }
        }
        racine.to_path_buf()
    }

    /// Les données livrées copiées à côté de la compagnie (`<racine>/../donnees`), pour un cas qui les modifie ; `route`
    /// les prend alors pour `PUPPETPAIE_CODE`. Rend le dossier du code.
    pub fn avec_le_code(racine: &Path, sans_baremes: bool) -> PathBuf {
        let code = racine.parent().unwrap().join("donnees");
        copier(&code_livre(), &code);
        if sans_baremes {
            std::fs::remove_dir_all(code.join("baremes")).unwrap();
        }
        code
    }

    /// Une compagnie fictive devenue compagnie ordinaire : marqueur retiré, déclarant net-entreprises et compte Urssaf posés.
    pub fn ordinaire(racine: &Path) {
        let _ = std::fs::remove_file(racine.join("COPIE-DE-DEVELOPPEMENT.json"));
        let mut e = lire(racine, "config/entreprise.json");
        let siret = e["siret"].clone();
        for cle in ["declarant", "api_dpae"] {
            if e.get(cle).is_none() {
                e[cle] = json!({"siret": siret, "nom": "FICTIF", "prenom": "Declarant"});
            }
        }
        ecrire(racine, "config/entreprise.json", &e);
    }
}

/// Pose des champs sur des fiches salariés (`config/salaries.json`) : (clé, champs) ; un champ `null` s'efface.
pub fn retoucher_fiches(racine: &Path, retouches: &[(&str, Value)]) {
    let mut sal = lire(racine, "config/salaries.json");
    for (cle, champs) in retouches {
        for (k, v) in champs.as_object().unwrap() {
            if v.is_null() {
                sal[*cle].as_object_mut().unwrap().remove(k);
            } else {
                sal[*cle][k] = v.clone();
            }
        }
    }
    ecrire(racine, "config/salaries.json", &sal);
}

/// La saisie de `mois` telle que ses engagements la montrent (`engagement::saisie_du_mois`).
pub fn saisie(racine: &Path, mois: &str) -> Value {
    let mut v = engagement::saisie_du_mois(racine, mois).unwrap();
    v["mois"] = Value::String(mois.to_string());
    v
}

/// « Modifier… » pour les engagements valides de `mois` (`numero` : celui de ce contrat seulement) : ils repassent en
/// saisie, et ce que l'ecran enregistre ensuite s'ecrit.
pub fn remettre_en_saisie(racine: &Path, mois: &str, numero: Option<&str>) {
    let ctx = droit::contexte::Contexte::nouveau(racine, None).unwrap();
    let objets = engagement::stock::objets_du_mois(racine, mois).unwrap();
    for (i, o) in objets.iter().enumerate() {
        let son_numero = o.brut.get("numero").and_then(Value::as_str);
        if o.est_valide() && numero.map_or(true, |n| son_numero == Some(n)) {
            engagement::etats::modifier(&ctx, mois, i).unwrap();
        }
    }
}

/// Ecrit la saisie de `mois` comme l'ecran l'enregistre (`engagement::stock::ecrire_vue_du_mois`) : chaque engagement
/// valide que `v` change repasse d'abord en saisie (« Modifier… »), puis tout engagement du mois en saisie se valide
/// comme l'ecran (« Valider l'engagement ») : ce qu'un test retouche s'ecrit, et part. Un engagement dont le calcul
/// refuse reste en saisie.
pub fn poser_saisie(racine: &Path, mois: &str, v: &Value) {
    let ctx = droit::contexte::Contexte::nouveau(racine, None).unwrap();
    let sans_validation = |e: &Value| {
        let mut e = e.clone();
        if let Some(m) = e.as_object_mut() {
            m.remove("valide");
        }
        e
    };
    let avant = engagement::stock::vue_du_mois(racine, mois).ok().flatten().unwrap_or(Value::Null);
    let objets = engagement::stock::objets_du_mois(racine, mois).unwrap();
    for (i, o) in objets.iter().enumerate() {
        let change = v["engagements"].get(i).map(sans_validation) != avant["engagements"].get(i).map(sans_validation);
        if o.est_valide() && change {
            engagement::etats::modifier(&ctx, mois, i).unwrap();
        }
    }
    engagement::stock::ecrire_vue_du_mois(&ctx, mois, v).unwrap();
    for (i, o) in engagement::stock::objets_du_mois(racine, mois).unwrap().iter().enumerate() {
        if !o.est_valide() {
            let _ = engagement::etats::valider(&ctx, mois, i);
        }
    }
}

/// La DSN mensuelle de `mois`, produite au `jour` depuis ses engagements, sans rien écrire.
pub fn dsn_du_mois(racine: &Path, mois: &str, jour: &str) -> Result<String, String> {
    let ctx = droit::contexte::Contexte::nouveau(racine, Some(socle::date::Jour::lire(jour).unwrap())).unwrap();
    let s = engagement::charger(&ctx, mois)?;
    paie::dsn::generer(&ctx, &s.paies, mois, &[], &[], &paie::dsn::Options::default())
}

/// Les changements datés d'un permanent sur une copie de `fictive-1285-grande` (deux CDI, NADAR cadre depuis 2022,
/// DORVAL non cadre depuis le 01/04/2025 ; toutes les DSN déposées jusqu'à août 2026) : `tests/dsn/bloc_41.rs` et
/// `tests/calcul/periodes.rs`.
pub mod permanents {
    use super::*;
    use droit::contexte::Contexte;
    use socle::date::Jour;
    use engagement::{charger, Saisie};

    /// Pose des champs sur l'engagement de `salarie` dans la saisie du mois.
    pub fn retoucher_saisie(racine: &Path, mois: &str, salarie: &str, champs: Value) {
        let mut s = super::saisie(racine, mois);
        for e in s["engagements"].as_array_mut().unwrap() {
            if e["salarie"] == salarie {
                for (k, v) in champs.as_object().unwrap() {
                    e[k] = v.clone();
                }
            }
        }
        poser_saisie(racine, mois, &s);
    }

    /// Le contexte du jour où l'on produit `mois` : le 5 du mois suivant.
    pub fn contexte(racine: &Path, mois: &str) -> Contexte {
        let (an, mo): (i32, u32) = (mois[..4].parse().unwrap(), mois[5..7].parse().unwrap());
        let (an, mo) = if mo == 12 { (an + 1, 1) } else { (an, mo + 1) };
        Contexte::nouveau(racine, Some(Jour::lire(&format!("{:04}-{:02}-05", an, mo)).unwrap())).unwrap()
    }

    /// La saisie de `mois`, lue au 5 du mois suivant.
    pub fn saisie(racine: &Path, mois: &str) -> Result<Saisie, String> {
        charger(&contexte(racine, mois), mois)
    }

    /// La DSN de `mois`, produite au 5 du mois suivant, sans rien écrire.
    pub fn dsn(racine: &Path, mois: &str) -> Result<String, String> {
        let ctx = contexte(racine, mois);
        let s = charger(&ctx, mois)?;
        paie::dsn::generer(&ctx, &s.paies, mois, &[], &[], &paie::dsn::Options::default())
    }

    /// DORVAL (CDI depuis le 01/04/2025, chargée de production, 05 / 04) passe cadre le lendemain de `veille`, changement
    /// saisi et daté sur sa fiche comme l'écran le fait (`historique` : l'ancienne valeur jusqu'à la veille).
    pub fn dorval_cadre_au(racine: &Path, veille: &str) {
        retoucher_fiches(
            racine,
            &[(
                "DORVAL",
                json!({
                    "statut_conventionnel": "04",
                    "statut_categoriel_rc": "01",
                    "historique": [
                        {"champ": "statut_categoriel_rc", "valeur": "04", "jusqu_au": veille, "note": ""},
                        {"champ": "statut_conventionnel", "valeur": "05", "jusqu_au": veille, "note": ""}
                    ]
                }),
            )],
        );
    }

    /// Les blocs S21.G00.41 d'un individu, chacun ouvert par son 41.001.
    pub fn blocs_41(lignes: &[(String, String)]) -> Vec<Vec<(String, String)>> {
        let mut out: Vec<Vec<(String, String)>> = Vec::new();
        for (k, v) in lignes {
            if k == "S21.G00.41.001" {
                out.push(Vec::new());
            }
            if k.starts_with("S21.G00.41.") {
                out.last_mut().unwrap().push((k.clone(), v.clone()));
            }
        }
        out
    }

    /// Un bloc attendu, en paires (rubrique, valeur).
    pub fn bloc(paires: &[(&str, &str)]) -> Vec<(String, String)> {
        paires.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    /// Le mois est déposé : la DSN produite remplace le fichier du mois dans la copie et le journal la dit acceptée en
    /// réel (ce que `casc::dsn_deposee` relit).
    pub fn deposer(racine: &Path, mois: &str, nom: &str, texte: &str) {
        paie::dsn::ecrire(texte, &racine.join("sorties").join(mois).join(nom)).unwrap();
        dsn_deposee(racine, mois, nom, &format!("{}-28T12:00:00", mois));
    }
}

/// Le mois est prêt à produire : les DPAE des engagements qui y commencent sont parties et leurs contrats sont revenus
/// signés (des lignes de journal comme celles de la compagnie fictive).
pub fn mois_pret(racine: &Path, mois: &str) {
    let saisie = saisie(racine, mois);
    let mut j = lire(racine, "journal/depots.json");
    let l = j.as_array_mut().unwrap();
    let modele = l.iter().find(|x| x["quoi"] == json!("dpae") && x["mode"] == json!("reel")).unwrap().clone();
    let contrat = l.iter().find(|x| x["quoi"] == json!("contrat-recu")).unwrap().clone();
    let gens: Vec<(String, String)> = saisie["engagements"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e.get("spectacle").is_some() && e["debut"].as_str().unwrap().starts_with(mois))
        .map(|e| (e["salarie"].as_str().unwrap().to_uppercase(), e["debut"].as_str().unwrap().to_string()))
        .collect();
    let mut d = modele;
    d["mois"] = json!(mois);
    d["detail"]["embauches"] = json!(gens.iter().map(|(g, deb)| json!({"salarie": g, "debut": deb, "fin": deb})).collect::<Vec<_>>());
    l.push(d);
    for (g, deb) in &gens {
        let mut c = contrat.clone();
        c["mois"] = json!(mois);
        c["detail"]["salarie"] = json!(g);
        c["detail"]["debut"] = json!(deb);
        l.push(c);
    }
    ecrire(racine, "journal/depots.json", &j);
}

/// La DSN `fichier` de `mois` est déposée en réel et acceptée le `quand` : une ligne de journal comme la dernière DSN
/// réelle du journal.
pub fn dsn_deposee(racine: &Path, mois: &str, fichier: &str, quand: &str) {
    let mut j = lire(racine, "journal/depots.json");
    let l = j.as_array_mut().unwrap();
    let mut d = l.iter().rev().find(|x| x["quoi"] == json!("dsn") && x["mode"] == json!("reel")).expect("une DSN reelle au journal").clone();
    d["mois"] = json!(mois);
    d["fichier"] = json!(fichier);
    d["quand"] = json!(quand);
    l.push(d);
    ecrire(racine, "journal/depots.json", &j);
}

/// Une copie d'une compagnie fictive, retouchée par le test, qui s'efface quand elle sort de portée.
pub struct Copie(pub PathBuf);

impl Copie {
    fn dossier(fictive: &str) -> PathBuf {
        static N: AtomicUsize = AtomicUsize::new(0);
        let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("copies").join(format!("{}-{}-{}", fictive, std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    pub fn de(fictive: &str) -> Copie {
        let d = Copie::dossier(fictive);
        copier(&self::fictive(fictive), &d);
        Copie(d)
    }

    /// La seule config de la compagnie fictive : ni journal, ni engagements, ni sorties.
    pub fn config_de(fictive: &str) -> Copie {
        let d = Copie::dossier(fictive);
        copier(&self::fictive(fictive).join("config"), &d.join("config"));
        Copie(d)
    }

    pub fn contexte(&self, jour: &str) -> droit::contexte::Contexte {
        droit::contexte::Contexte::nouveau(&self.0, Some(socle::date::Jour::lire(jour).unwrap())).unwrap()
    }
}

impl Drop for Copie {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Le rejeu d'un scenario du banc d'essai et le releve de ses sorties (#236, phase 2).
pub mod rejeu;

// ------------------------------------------------------------------ la route

/// Les variables qu'aucun sous-processus de test n'hérite du poste : les secrets, le bac à sable des incidents, le code.
pub const RETIREES: [&str; 7] =
    ["COURRIEL_COMPTE", "COURRIEL_MOTDEPASSE", "NET_ENTREPRISES_MOTDEPASSE", "URSSAF_MOTDEPASSE", "PUPPETPAIE_INCIDENTS", "PUPPETPAIE_MOTDEPASSE", "PUPPETPAIE_CODE"];

/// Le bac à sable d'un `paie` lancé par un test : la compagnie, une clé de poste qui n'existe pas, le jour ; aucun secret du
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

/// `paie route` dans un sous-processus : la réponse entière (`{"ok", "resultat"}` ou `{"ok": false, "erreur", "classe"}`).
pub fn route(racine: &Path, nom: &str, charge: Value, jour: &str) -> Value {
    route_avec(racine, nom, charge, jour, &[])
}

/// La même, avec des variables posées par le cas après le bac à sable (une boîte factice, un autre `PUPPETPAIE_AUJOURDHUI`…).
pub fn route_avec(racine: &Path, nom: &str, charge: Value, jour: &str, env: &[(&str, &str)]) -> Value {
    let mut c = Command::new(env!("CARGO_BIN_EXE_paie"));
    c.args(["route", "--racine", racine.to_str().unwrap(), "--date", jour]).stdin(Stdio::piped()).stdout(Stdio::piped());
    bac_a_sable(&mut c, racine, jour);
    c.envs(env.iter().copied());
    let mut p = c.spawn().unwrap();
    p.stdin.take().unwrap().write_all(json!({"route": nom, "charge": charge}).to_string().as_bytes()).unwrap();
    let sortie = p.wait_with_output().unwrap().stdout;
    serde_json::from_slice(&sortie).unwrap_or_else(|e| panic!("reponse illisible ({}) : {}", e, String::from_utf8_lossy(&sortie)))
}

/// La même route, dans le processus du test (`paie::routes::commande`) : même forme de réponse.
pub fn route_en_processus(racine: &Path, nom: &str, charge: Value, jour: &str) -> Value {
    let mut o = HashMap::new();
    o.insert("racine".to_string(), racine.to_string_lossy().into_owned());
    o.insert("date".to_string(), jour.to_string());
    match paie::routes::commande(&o, &json!({"route": nom, "charge": charge}).to_string()) {
        Ok(r) => r,
        Err(m) => {
            let (classe, message) = paie::routes_ouverture::decoder(&m);
            json!({"ok": false, "classe": classe, "erreur": message})
        }
    }
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

/// Les 78.004 des blocs 78 de code `code`.
pub fn base78(v: &[(String, String)], code: &str) -> Vec<Decimal> {
    let mut courant = String::new();
    let mut out = Vec::new();
    for (k, x) in v {
        if k == "S21.G00.78.001" {
            courant = x.clone();
        } else if k == "S21.G00.78.004" && courant == code {
            out.push(dec(x));
        }
    }
    out
}

/// Le 81.003 de la première cotisation `code81`.
pub fn base81(v: &[(String, String)], code81: &str) -> Option<Decimal> {
    let i = v.iter().position(|(k, x)| k == "S21.G00.81.001" && x == code81)?;
    let suite = v[i + 1..].iter().take_while(|(k, _)| k != "S21.G00.81.001" && !k.starts_with("S21.G00.78."));
    suite.into_iter().find(|(k, _)| k == "S21.G00.81.003").map(|(_, x)| dec(x))
}

/// L'assiette (23.004) du CTP `code` sous le qualifiant `qualifiant`.
pub fn ctp(dsn: &str, code: &str, qualifiant: &str) -> Option<Decimal> {
    let (mut c, mut q) = (String::new(), String::new());
    for (k, v) in paires(dsn) {
        match k.as_str() {
            "S21.G00.23.001" => c = v,
            "S21.G00.23.002" => q = v,
            "S21.G00.23.004" if c == code && q == qualifiant => return Some(dec(&v)),
            _ => {}
        }
    }
    None
}

/// Les cotisations (81.004) d'une DSN par contrat de prévoyance, relues sans le moteur : une base qui cite une affiliation
/// (78.005) que son contrat de travail déclare (70.012) va à la référence d'adhésion (70.013) dont le bloc 15 donne le
/// contrat chez l'organisme (15.005 -> 15.001), le code d'affectation du bloc 55 (norme 2026.1 p. 260). Le contrat d'une
/// base : 78.006, ou le 40.009 de l'individu qui n'en a qu'un (cahier technique 2026.1 p. 298).
pub fn par_contrat_de_prevoyance(lignes: &[(String, String)]) -> std::collections::BTreeMap<String, Decimal> {
    let adhesions: HashMap<&str, &str> = lignes
        .iter()
        .scan("", |libelle, (k, v)| {
            if k == "S21.G00.15.001" {
                *libelle = v.as_str();
            }
            Some((k == "S21.G00.15.005").then(|| (v.as_str(), *libelle)))
        })
        .flatten()
        .collect();
    let mut out = std::collections::BTreeMap::new();
    let mut individu: Vec<&[(String, String)]> = Vec::new();
    let mut debut = 0;
    for (i, (k, _)) in lignes.iter().enumerate() {
        if k == "S21.G00.30.001" {
            individu.push(&lignes[debut..i]);
            debut = i;
        }
    }
    individu.push(&lignes[debut..]);
    for bloc in individu {
        let (mut contrat40, mut id70) = ("", "");
        let mut affiliations: HashMap<(&str, &str), &str> = HashMap::new();
        let (mut contrat78, mut aff) = ("", None::<&str>);
        let mut cotisations = Vec::new();
        for (k, v) in bloc {
            match k.as_str() {
                "S21.G00.40.009" => contrat40 = v.as_str(),
                "S21.G00.70.012" => id70 = v.as_str(),
                "S21.G00.70.013" => {
                    affiliations.insert((contrat40, id70), v.as_str());
                }
                "S21.G00.78.001" => (contrat78, aff) = (contrat40, None),
                "S21.G00.78.005" => aff = Some(v.as_str()),
                "S21.G00.78.006" => contrat78 = v.as_str(),
                "S21.G00.81.004" => {
                    if let Some(a) = aff {
                        cotisations.push((contrat78, a, dec(v)));
                    }
                }
                _ => {}
            }
        }
        for (contrat, a, montant) in cotisations {
            let reference = affiliations.get(&(contrat, a)).unwrap_or_else(|| panic!("78.005 {} sans affiliation du contrat {}", a, contrat));
            let libelle = adhesions.get(reference).unwrap_or_else(|| panic!("l'adhesion {} sans bloc 15", reference));
            *out.entry(libelle.to_string()).or_insert(Decimal::ZERO) += montant;
        }
    }
    out
}

/// Les blocs 58 : (001, 002, 003, le reste).
pub fn blocs_58(v: &[(String, String)]) -> Vec<(String, String, String, String)> {
    let mut out: Vec<(String, String, String, String)> = Vec::new();
    for (k, x) in v.iter().filter(|(k, _)| k.starts_with("S21.G00.58.")) {
        let nouveau = match out.last() {
            None => true,
            Some(c) => k == "S21.G00.58.001" || (k == "S21.G00.58.003" && !c.2.is_empty()),
        };
        if nouveau {
            out.push(Default::default());
        }
        let c = out.last_mut().unwrap();
        match &k[11..] {
            "001" => c.0 = x.clone(),
            "002" => c.1 = x.clone(),
            "003" => c.2 = x.clone(),
            _ => c.3 = x.clone(),
        }
    }
    out
}

/// Un enregistrement AEM de 1000 caractères aux offsets que lit `importation.rs` ; `zones` : ce que le cas ajoute
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

/// Une requête reçue. Le corps d'une authentification (un chemin qui contient `authentifier`) n'est jamais gardé : il porte
/// un mot de passe.
#[derive(Clone, Debug)]
pub struct Requete {
    /// L'adresse du serveur (`http://127.0.0.1:port`), pour écrire des liens absolus.
    pub base: String,
    pub methode: String,
    pub chemin: String,
    pub requete: String,
    /// Les en-têtes, noms en minuscules.
    pub entetes: Vec<(String, String)>,
    pub corps: Vec<u8>,
}

impl Requete {
    pub fn entete(&self, nom: &str) -> &str {
        self.entetes.iter().find(|(k, _)| k.eq_ignore_ascii_case(nom)).map(|(_, v)| v.as_str()).unwrap_or("")
    }
    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.corps).unwrap_or(Value::Null)
    }
}

pub type Recu = Arc<Mutex<Vec<Requete>>>;

fn lire_requete(s: &mut TcpStream, base: &str) -> Option<Requete> {
    let mut tampon = Vec::new();
    let mut un = [0u8; 8192];
    let fin = loop {
        if let Some(i) = tampon.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
        match s.read(&mut un) {
            Ok(0) | Err(_) => return None,
            Ok(n) => tampon.extend_from_slice(&un[..n]),
        }
    };
    let tete = String::from_utf8_lossy(&tampon[..fin]).into_owned();
    let mut lignes = tete.split("\r\n");
    let mut mots = lignes.next()?.split(' ');
    let (methode, cible) = (mots.next()?.to_string(), mots.next()?.to_string());
    let entetes: Vec<(String, String)> = lignes.filter_map(|l| l.split_once(": ")).map(|(k, v)| (k.to_lowercase(), v.to_string())).collect();
    let longueur = entetes.iter().find(|(k, _)| k == "content-length").and_then(|(_, v)| v.parse::<usize>().ok()).unwrap_or(0);
    let mut corps = tampon[fin + 4..].to_vec();
    while corps.len() < longueur {
        match s.read(&mut un) {
            Ok(0) | Err(_) => break,
            Ok(n) => corps.extend_from_slice(&un[..n]),
        }
    }
    let (chemin, requete) = cible.split_once('?').map(|(c, q)| (c.to_string(), q.to_string())).unwrap_or((cible.clone(), String::new()));
    if chemin.contains("authentifier") {
        corps.clear();
    }
    Some(Requete { base: base.to_string(), methode, chemin, requete, entetes, corps })
}

/// Une réponse HTTP entière.
pub fn http(statut: u16, type_: &str, corps: &[u8]) -> Vec<u8> {
    let mut r = format!("HTTP/1.1 {} X\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", statut, type_, corps.len()).into_bytes();
    r.extend_from_slice(corps);
    r
}

pub fn http_json(statut: u16, v: &Value) -> Vec<u8> {
    http(statut, "application/json", v.to_string().as_bytes())
}

/// Un serveur local (127.0.0.1, un fil par connexion) : `f` rend les octets de la réponse, ou `None` pour couper la
/// connexion sans répondre. Rend son adresse et ce qu'il a reçu, dans l'ordre.
pub fn serveur(f: impl Fn(&Requete) -> Option<Vec<u8>> + Send + Sync + 'static) -> (String, Recu) {
    let ecoute = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://127.0.0.1:{}", ecoute.local_addr().unwrap().port());
    let recu: Recu = Arc::new(Mutex::new(Vec::new()));
    let (f, r2, b2) = (Arc::new(f), recu.clone(), base.clone());
    std::thread::spawn(move || {
        for flux in ecoute.incoming() {
            let Ok(mut s) = flux else { continue };
            let (f, r2, b2) = (f.clone(), r2.clone(), b2.clone());
            std::thread::spawn(move || {
                let Some(q) = lire_requete(&mut s, &b2) else { return };
                r2.lock().unwrap().push(q.clone());
                match f(&q) {
                    None => {
                        let _ = s.shutdown(std::net::Shutdown::Both);
                    }
                    Some(octets) => {
                        let _ = s.write_all(&octets);
                        let _ = s.flush();
                    }
                }
            });
        }
    });
    (base, recu)
}

/// Un port où personne n'écoute (« réseau injoignable »).
pub fn port_ferme() -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let u = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
    drop(l);
    u
}

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

// ------------------------------------------------------------------ le faux courriel

/// Une boîte en mémoire : elle garde ce qu'on lui envoie, peut refuser un envoi, rend à la recherche les messages qu'on lui
/// a préparés (ceux posés avec un code ne répondent qu'à ce code), et peut échouer à chercher ou à lire.
#[derive(Clone)]
pub struct FauxCourriel {
    pub compte: String,
    pub recus: Rc<RefCell<Vec<Vec<u8>>>>,
    /// L'envoi de ce rang seulement est refusé.
    pub refuse_le: Option<usize>,
    /// Tout envoi à partir de ce rang est refusé.
    pub refuse_a_partir_de: Option<usize>,
    /// (code, identifiant, message).
    pub boite: Rc<RefCell<Vec<(Option<String>, String, Vec<u8>)>>>,
    pub recherche_en_echec: bool,
    /// Les codes dont la recherche échoue.
    pub recherche_en_echec_pour: Vec<String>,
    pub lecture_en_echec: Vec<String>,
    pub requetes: Rc<RefCell<Vec<String>>>,
    pub fermetures: Rc<RefCell<usize>>,
}

impl Default for FauxCourriel {
    fn default() -> FauxCourriel {
        FauxCourriel {
            compte: "boite@exemple.invalid".into(),
            recus: Rc::default(),
            refuse_le: None,
            refuse_a_partir_de: None,
            boite: Rc::default(),
            recherche_en_echec: false,
            recherche_en_echec_pour: Vec::new(),
            lecture_en_echec: Vec::new(),
            requetes: Rc::default(),
            fermetures: Rc::default(),
        }
    }
}

impl FauxCourriel {
    pub fn nouveau() -> FauxCourriel {
        FauxCourriel::default()
    }
    /// Une boîte qui rend ces messages à toute recherche.
    pub fn avec(messages: Vec<(&str, Vec<u8>)>) -> FauxCourriel {
        let f = FauxCourriel::default();
        *f.boite.borrow_mut() = messages.into_iter().map(|(i, m)| (None, i.to_string(), m)).collect();
        f
    }
    /// Un message que seule la recherche du `code` rend, sous l'identifiant `id`.
    pub fn pose(&self, code: &str, id: &str, octets: Vec<u8>) {
        self.boite.borrow_mut().push((Some(code.to_string()), id.to_string(), octets));
    }
    pub fn partis(&self) -> usize {
        self.recus.borrow().len()
    }
    pub fn recherches(&self) -> usize {
        self.requetes.borrow().len()
    }
}

impl Courriel for FauxCourriel {
    fn compte(&self) -> &str {
        &self.compte
    }
    fn envoyer(&mut self, m: &[u8]) -> Result<String, ErreurCourriel> {
        let n = self.recus.borrow().len();
        if self.refuse_le == Some(n) || self.refuse_a_partir_de.is_some_and(|a| n >= a) {
            return Err(ErreurCourriel("le serveur refuse ce message".into()));
        }
        self.recus.borrow_mut().push(m.to_vec());
        Ok(format!("<faux-{}@exemple.invalid>", n))
    }
    fn rechercher(&mut self, requete: &str, _: usize) -> Result<Vec<String>, ErreurCourriel> {
        self.requetes.borrow_mut().push(requete.to_string());
        let code = requete.trim_matches('"');
        if self.recherche_en_echec || self.recherche_en_echec_pour.iter().any(|c| c == code) {
            return Err(ErreurCourriel("la recherche echoue".into()));
        }
        Ok(self.boite.borrow().iter().filter(|(c, _, _)| c.as_deref().is_none_or(|c| c == code)).map(|(_, i, _)| i.clone()).collect())
    }
    fn message_brut(&mut self, id: &str) -> Result<Vec<u8>, ErreurCourriel> {
        if self.lecture_en_echec.iter().any(|i| i == id) {
            return Err(ErreurCourriel("la lecture echoue".into()));
        }
        Ok(self.boite.borrow().iter().find(|(_, i, _)| i == id).map(|(_, _, m)| m.clone()).unwrap_or_default())
    }
    fn fermer(&mut self) {
        *self.fermetures.borrow_mut() += 1;
    }
}

// ------------------------------------------------------------------ les oracles (communs aux trois crates)

#[path = "../../../socle/tests/outils/oracles.rs"]
mod oracles;
pub use oracles::*;

/// Un message parti, lu comme une messagerie le lit.
pub struct Mail(pub Vec<u8>);

impl Mail {
    fn lu(&self) -> mailparse::ParsedMail<'_> {
        mailparse::parse_mail(&self.0).unwrap()
    }
    pub fn entete(&self, nom: &str) -> String {
        mailparse::MailHeaderMap::get_first_value(self.lu().headers.as_slice(), nom).unwrap_or_default()
    }
    fn jointes(&self) -> Vec<(String, Vec<u8>)> {
        fn parcourir(m: &mailparse::ParsedMail, out: &mut Vec<(String, Vec<u8>)>) {
            let d = m.get_content_disposition();
            if d.disposition == mailparse::DispositionType::Attachment {
                out.push((d.params.get("filename").cloned().unwrap_or_default(), m.get_body_raw().unwrap()));
            }
            m.subparts.iter().for_each(|s| parcourir(s, out));
        }
        let mut out = Vec::new();
        parcourir(&self.lu(), &mut out);
        out
    }
    /// Les noms des pièces jointes.
    pub fn pieces(&self) -> Vec<String> {
        self.jointes().into_iter().map(|(n, _)| n).collect()
    }
    /// Les octets des pièces jointes, décodés.
    pub fn pieces_octets(&self) -> Vec<Vec<u8>> {
        self.jointes().into_iter().map(|(_, o)| o).collect()
    }
    /// Le texte de la première partie `text/plain` et celui de la première `text/html`, en fins de ligne `\n`.
    pub fn textes(&self) -> (String, String) {
        fn parcourir(m: &mailparse::ParsedMail, plain: &mut Option<String>, html: &mut Option<String>) {
            match m.ctype.mimetype.as_str() {
                "text/plain" if plain.is_none() => *plain = m.get_body().ok(),
                "text/html" if html.is_none() => *html = m.get_body().ok(),
                _ => {}
            }
            m.subparts.iter().for_each(|s| parcourir(s, plain, html));
        }
        let (mut p, mut h) = (None, None);
        parcourir(&self.lu(), &mut p, &mut h);
        (p.unwrap_or_default().replace("\r\n", "\n"), h.unwrap_or_default().replace("\r\n", "\n"))
    }
}

impl FauxCourriel {
    /// Le message parti de rang `n`.
    pub fn message(&self, n: usize) -> Mail {
        Mail(self.recus.borrow()[n].clone())
    }
}

/// Les paies d'une saisie ecrite dans un fichier de test (`engagement::charger_donnees`).
pub fn charger_fichier(ctx: &droit::contexte::Contexte, chemin: &Path) -> Result<engagement::Saisie, String> {
    let v: Value = serde_json::from_slice(&std::fs::read(chemin).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    engagement::charger_donnees(ctx, &v)
}

/// « Valider l'engagement » pour chaque engagement de `mois` encore en saisie, comme l'ecran le fait au `jour`.
pub fn tout_valider(racine: &Path, mois: &str, jour: &str) {
    let etat = route_en_processus(racine, "etat-par-engagement", json!({"mois": mois}), jour);
    for e in reussi(&etat)["engagements"].as_array().unwrap().iter().filter(|e| e["valide"].is_null()) {
        let charge = json!({"mois": mois, "salarie": e["salarie"], "debut": e["debut"], "numero": e["numero"], "rang": e["rang"], "valide": true});
        reussi(&route_en_processus(racine, "valider-engagement", charge, jour));
    }
}
