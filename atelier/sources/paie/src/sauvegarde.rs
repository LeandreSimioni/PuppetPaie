//! La sauvegarde du dossier prive dans le Drive, par differences (`sauvegarde.py`).
//!
//! Ecrit au lot H4b, non prouve. Ce sont des FONCTIONS, pas des routes : leurs appelants sont
//! l'ouverture et la veille (lot I), la ligne de commande (lot J), et `web.py` qui lit
//! `derniere` et `rien_n_a_bouge`. Le client Drive se passe en parametre (`drive::Drive`).
//!
//! CE QU'ELLE SAUVE : tout le dossier des donnees (`config/`, `paies/`, `sorties/`, `retours/`,
//! `gabarits/`, `journal/`, `reference/`), coffre compris (`coffre.bin`, scelle par le mot de passe du logiciel, qui
//! l'ouvre sur un poste neuf apres `paie restaurer` : Leandre, 13/10/2026, redit le 14/10), sauf le jeton Google
//! (`google-jeton.bin`, le compte se relie), l'etat de la machine (`.compteurs.verrou`, l'index lui-meme) et le rebut.
//!
//! ELLE NE SUPPRIME JAMAIS RIEN dans le Drive : le trait `Drive` n'a d'ailleurs aucune
//! suppression. Un fichier efface ici reste la-bas.
//!
//! COMMENT ELLE SAIT CE QUI A CHANGE : par l'empreinte MD5 que Google publie
//! (`md5Checksum`). L'index local (`config/.sauvegarde.json`) ne sert qu'a repondre SANS RESEAU
//! (`rien_n_a_bouge`) et a garder les identifiants des dossiers Drive ; il n'est jamais cru pour
//! DECIDER d'un televersement.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use md5::{Digest, Md5};
use serde_json::{json, Map, Value};

use crate::drive::{Client, Drive, ErreurGoogle};

pub const RACINE_DRIVE: &str = "PuppetPaie";
pub const DOSSIER_DRIVE: &str = "sauvegarde";
pub const INDEX: &str = ".sauvegarde.json";

/// Ce qui ne monte pas (`NOMS_EXCLUS`).
const NOMS_EXCLUS: [&str; 6] = ["google-jeton.bin", ".jeton-interface", ".compteurs.verrou", ".sauvegarde.json", ".ouverture.json", ".ouverture-etape.json"];
const SUFFIXES_EXCLUS: [&str; 4] = [".pyc", ".tmp", ".lock", ".part"];
const DOSSIERS_EXCLUS: [&str; 2] = ["__pycache__", ".git"];

/// `_SIGNES_HORS_LIGNE`, SANS `timed out` (DEFAUT 2 DU PYTHON, CORRIGE) : le Python rangeait « timed out » avec l'absence de
/// reseau, si bien qu'un delai de LECTURE depasse (un serveur qui repond trop lentement, un dossier de Drive qui ne
/// repond pas) arretait TOUTE la sauvegarde (« Hors connexion : sauvegarde au prochain passage ») alors que le reseau est la.
/// Un delai depasse est maintenant un incident du dossier ou du fichier ; l'absence de reseau reste « injoignable »,
/// `getaddrinfo`, `Network is unreachable`, `Name or service not known`... (un delai de CONNEXION dit « Reseau injoignable :
/// timed out » : il garde son « injoignable »).
const SIGNES_HORS_LIGNE: [&str; 10] = [
    "injoignable",
    "getaddrinfo",
    "errno 11001",
    "errno 11004",
    "winerror 10051",
    "winerror 10060",
    "winerror 10065",
    "network is unreachable",
    "name or service not known",
    "temporary failure in name resolution",
];

/// `hors_ligne` : ce message dit-il que le reseau manque, plutot qu'un vrai refus ?
pub fn hors_ligne(erreur: &str) -> bool {
    let texte = erreur.to_lowercase();
    SIGNES_HORS_LIGNE.iter().any(|s| texte.contains(s))
}

/// L'etat d'un fichier a l'envoi (`Fichier.etat`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtatFichier {
    Neuf,
    Modifie,
}

impl EtatFichier {
    pub fn texte(self) -> &'static str {
        match self {
            EtatFichier::Neuf => "neuf",
            EtatFichier::Modifie => "modifie",
        }
    }
}

/// Un fichier depose (`Fichier`).
#[derive(Debug, Clone, PartialEq)]
pub struct Fichier {
    pub relatif: String,
    pub octets: u64,
    pub etat: EtatFichier,
}

/// Le compte rendu d'un passage (`Sauvegarde`).
#[derive(Debug, Clone, PartialEq)]
pub struct Sauvegarde {
    pub racine_drive: String,
    pub compte: Option<String>,
    pub deposes: Vec<Fichier>,
    pub identiques: usize,
    pub octets_deposes: u64,
    pub exclus: usize,
    /// Purges entre l'inventaire et l'envoi : pas un incident.
    pub disparus: usize,
    pub secondes: f64,
    pub incidents: Vec<String>,
    /// Pas de reseau : arretee au premier dossier.
    pub hors_ligne: bool,
}

impl Sauvegarde {
    pub fn a_change(&self) -> bool {
        !self.deposes.is_empty()
    }

    pub fn resume(&self) -> String {
        if self.hors_ligne {
            return "Hors connexion : sauvegarde au prochain passage.".into();
        }
        if self.deposes.is_empty() {
            return format!("Sauvegarde a jour ({} fichiers).", self.identiques);
        }
        format!("Sauvegarde : {} fichier(s) envoye(s), {}, {} inchange(s).", self.deposes.len(), lisible(self.octets_deposes), self.identiques)
    }

    /// Ce que `commandes_ouverture._en_json` en lit : `resume`, `deposes` (les chemins relatifs),
    /// `identiques`, `octets`.
    pub fn en_json(&self) -> Value {
        json!({
            "resume": self.resume(),
            "deposes": self.deposes.iter().map(|f| f.relatif.clone()).collect::<Vec<_>>(),
            "identiques": self.identiques,
            "octets": self.octets_deposes,
        })
    }
}

fn lisible(octets: u64) -> String {
    if octets >= 1024 * 1024 {
        format!("{:.1} Mo", octets as f64 / (1024.0 * 1024.0))
    } else if octets >= 1024 {
        format!("{} Ko", octets / 1024)
    } else {
        format!("{} o", octets)
    }
}

// ---------------------------------------------------------------- l'inventaire

/// La taille et la date de modification (secondes entieres) d'un fichier (`inventaire`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Etat {
    pub octets: u64,
    pub mtime: i64,
}

/// `_ecarte` : un nom ou un suffixe exclu, ou un dossier exclu n'importe ou dans le chemin relatif
/// (le nom du fichier compris). La restauration s'en sert aussi : elle ne redescend que ce que la sauvegarde envoie.
pub(crate) fn ecarte(relatif: &Path) -> bool {
    let nom = relatif.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if NOMS_EXCLUS.contains(&nom.as_str()) || SUFFIXES_EXCLUS.iter().any(|s| nom.ends_with(s)) {
        return true;
    }
    relatif.components().any(|c| DOSSIERS_EXCLUS.contains(&c.as_os_str().to_string_lossy().as_ref()))
}

/// `empreinte` : MD5 du contenu, le meme que Drive publie. Lu par morceaux.
pub fn empreinte(chemin: &Path) -> std::io::Result<String> {
    let mut f = std::fs::File::open(chemin)?;
    let mut h = Md5::new();
    let mut bloc = vec![0u8; 1024 * 256];
    loop {
        let n = f.read(&mut bloc)?;
        if n == 0 {
            break;
        }
        h.update(&bloc[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{:02x}", b)).collect())
}

/// Les entrees (fichiers et liens, pas les dossiers) sous `dossier`, relatives a `racine`. Un
/// dossier lien n'est pas suivi ; un dossier illisible est passe sans bruit (comme `rglob`).
fn parcourir(racine: &Path, dossier: &Path, sortie: &mut Vec<PathBuf>) {
    let Ok(entrees) = std::fs::read_dir(dossier) else { return };
    for e in entrees.flatten() {
        let chemin = e.path();
        match e.file_type() {
            Ok(t) if t.is_dir() => parcourir(racine, &chemin, sortie),
            Ok(_) => {
                if let Ok(rel) = chemin.strip_prefix(racine) {
                    sortie.push(rel.to_path_buf());
                }
            }
            Err(_) => {}
        }
    }
}

/// `inventaire` : les fichiers dans l'ordre de `sorted(racine.rglob("*"))` (composante par
/// composante, pas en texte : `a/x` passe avant `a b/x`), et le nombre d'ecartes. Ne lit aucun
/// contenu.
pub fn inventaire(racine: &Path) -> (Vec<(String, Etat)>, usize) {
    let mut chemins = Vec::new();
    parcourir(racine, racine, &mut chemins);
    chemins.sort();
    let (mut trouves, mut ecartes) = (Vec::new(), 0);
    for rel in chemins {
        let complet = racine.join(&rel);
        if !complet.is_file() {
            continue;
        }
        if ecarte(&rel) {
            ecartes += 1;
            continue;
        }
        let Ok(m) = std::fs::metadata(&complet) else { continue };
        let relatif = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
        trouves.push((relatif, Etat { octets: m.len(), mtime: crate::paris::mtime(&m) as i64 }));
    }
    (trouves, ecartes)
}

// --------------------------------------------------------------------- l'index

/// `chemin_index` : `config/`, et pas `journal/` : de l'etat machine, pas un document. Il est
/// ecarte de la sauvegarde (`NOMS_EXCLUS`).
pub fn chemin_index(racine: &Path) -> PathBuf {
    racine.join("config").join(INDEX)
}

/// `_lire_index` : `{}` si le fichier manque ou ne se lit pas comme du JSON.
fn lire_index(racine: &Path) -> Map<String, Value> {
    let Ok(octets) = std::fs::read(chemin_index(racine)) else { return Map::new() };
    match serde_json::from_slice::<Value>(&octets) {
        Ok(Value::Object(o)) => o,
        _ => Map::new(),
    }
}

pub(crate) fn ecrire_index(racine: &Path, index: &Value) -> Result<(), ErreurGoogle> {
    crate::ecriture::ecrire_json(&chemin_index(racine), index, 1).map_err(|e| ErreurGoogle::nouvelle("OSError", e))
}

/// `rien_n_a_bouge` : le disque est-il identique au dernier passage ? SANS RESEAU. Taille et date
/// suffisent : on ne decide pas d'un televersement, seulement s'il vaut la peine d'aller voir. Au
/// moindre doute (index absent, fichier apparu ou disparu), la reponse est non.
///
/// DEFAUT 6 DU PYTHON, CORRIGE : `rien_n_a_bouge` ne regardait pas `racine_drive` ; un index ecrit pour une autre racine
/// du Drive (`--racine-drive`) laissait la veille croire que tout etait sauvegarde la ou on l'attend. Un index d'une autre
/// racine (ou qui n'en dit aucune) donne non.
pub fn rien_n_a_bouge(racine: &Path, racine_drive: &str) -> bool {
    let index = lire_index(racine);
    if index.get("racine_drive").and_then(Value::as_str) != Some(racine_drive) {
        return false;
    }
    let Some(Value::Object(connus)) = index.get("fichiers") else { return false };
    if connus.is_empty() {
        return false;
    }
    let (presents, _) = inventaire(racine);
    if presents.len() != connus.len() || presents.iter().any(|(r, _)| !connus.contains_key(r)) {
        return false;
    }
    presents.iter().all(|(relatif, etat)| {
        let ancien = &connus[relatif];
        egal(ancien.get("octets"), etat.octets as f64) && egal(ancien.get("mtime"), etat.mtime as f64)
    })
}

/// `etat["x"] != ancien.get("x")` : un nombre JSON egal, entier ou flottant (`5 == 5.0`).
fn egal(v: Option<&Value>, x: f64) -> bool {
    v.and_then(Value::as_f64).is_some_and(|a| a == x)
}

/// Ce que dit l'index du dernier passage (`derniere`) : quand, combien. Pour l'affichage.
#[derive(Debug, Clone, PartialEq)]
pub struct Derniere {
    /// `index.get("quand", "")` : tel que l'index le porte (un texte, en pratique).
    pub quand: Value,
    pub fichiers: usize,
    /// `index.get("racine_drive", "")`.
    pub racine_drive: Value,
}

impl Derniere {
    /// `{"quand", "fichiers", "racine_drive"}`, comme le dict du Python.
    pub fn en_json(&self) -> Value {
        json!({"quand": self.quand, "fichiers": self.fichiers, "racine_drive": self.racine_drive})
    }
}

pub fn derniere(racine: &Path) -> Derniere {
    let index = lire_index(racine);
    let brut = |c: &str| index.get(c).cloned().unwrap_or_else(|| json!(""));
    // `len(index.get("fichiers") or {})`
    let fichiers = match index.get("fichiers") {
        Some(Value::Object(o)) => o.len(),
        Some(Value::Array(a)) => a.len(),
        Some(Value::String(t)) => t.chars().count(),
        _ => 0,
    };
    Derniere { quand: brut("quand"), fichiers, racine_drive: brut("racine_drive") }
}

// ---------------------------------------------------------------- le passage

/// `sauvegarder` : envoie ce qui a change. Rend le compte rendu, n'imprime rien.
///
/// `client` : le client Drive a utiliser. `None` : le vrai (`Client::nouveau`), construit AVANT
/// l'inventaire comme le Python ; il refuse une copie de developpement.
pub fn sauvegarder(racine: &Path, racine_drive: &str, tout: bool, client: Option<&mut dyn Drive>) -> Result<Sauvegarde, ErreurGoogle> {
    let debut = std::time::Instant::now();
    let mut propre: Client;
    let client: &mut dyn Drive = match client {
        Some(c) => c,
        None => {
            propre = Client::nouveau(racine)?;
            &mut propre
        }
    };
    let (presents, ecartes) = inventaire(racine);
    let index = lire_index(racine);
    let mut dossiers: Map<String, Value> = match index.get("dossiers") {
        Some(Value::Object(o)) if !tout => o.clone(),
        _ => Map::new(),
    };

    let mut resultat = Sauvegarde {
        racine_drive: racine_drive.to_string(),
        compte: client.compte(),
        deposes: Vec::new(),
        identiques: 0,
        octets_deposes: 0,
        exclus: ecartes,
        disparus: 0,
        secondes: 0.0,
        incidents: Vec::new(),
        hors_ligne: false,
    };
    // Ce qui n'a PAS pu etre depose. Ces fichiers n'entrent pas dans l'index : sans cela, le
    // passage suivant les croirait a jour.
    let mut en_echec: std::collections::HashSet<String> = std::collections::HashSet::new();
    let etat_de: BTreeMap<&str, &Etat> = presents.iter().map(|(r, e)| (r.as_str(), e)).collect();

    // Par dossier : un seul listage Drive sert a tous les fichiers qu'il porte.
    let mut par_dossier: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (relatif, _) in &presents {
        let parent = relatif.rfind('/').map(|i| &relatif[..i]).unwrap_or(".");
        par_dossier.entry(parent.to_string()).or_default().push(relatif.clone());
    }

    for (dossier, fichiers) in &par_dossier {
        let mut segments: Vec<&str> = vec![racine_drive, DOSSIER_DRIVE];
        if dossier != "." && !dossier.is_empty() {
            segments.extend(dossier.split('/'));
        }
        let cle = segments.join("/");
        let essai: Result<(String, BTreeMap<String, crate::drive::Distant>), ErreurGoogle> = (|| {
            let connu = dossiers.get(&cle).and_then(Value::as_str).filter(|p| !p.is_empty()).map(str::to_string);
            let parent = match connu {
                Some(p) => p,
                None => client.chemin(&segments)?,
            };
            dossiers.insert(cle.clone(), json!(parent));
            // Meme avec `tout`, on liste : c'est `lister` qui dit ce qui existe, donc ce qui se REMPLACE (defaut 4 du Python :
            // plus de recherche par fichier). `tout` ne sert qu'a ne pas comparer les empreintes.
            let deja = client.lister(&parent)?;
            Ok((parent, deja))
        })();
        let (parent, deja) = match essai {
            Ok(x) => x,
            Err(e) => {
                if hors_ligne(&e.message) {
                    // Pas de reseau : les dossiers suivants tomberaient pareil. On s'arrete,
                    // rien de ce qui reste n'entre a l'index, et le prochain passage reprendra tout.
                    resultat.hors_ligne = true;
                    en_echec.extend(par_dossier.values().flatten().cloned());
                    dossiers.shift_remove(&cle);
                    break;
                }
                // Un dossier qui resiste ne doit pas emporter les autres : la sauvegarde est
                // partielle, elle le dit, et le prochain passage reprendra celui-la.
                resultat.incidents.push(format!("{} : {}", dossier, e));
                dossiers.shift_remove(&cle);
                en_echec.extend(fichiers.iter().cloned());
                continue;
            }
        };

        let mut tries: Vec<&String> = fichiers.iter().collect();
        tries.sort();
        for relatif in tries {
            let chemin = racine.join(relatif);
            let nom = relatif.rsplit('/').next().unwrap_or(relatif);
            let mut faire = || -> Result<Option<EtatFichier>, ErreurGoogle> {
                let locale = empreinte(&chemin).map_err(|e| ErreurGoogle::fichier(&e, &chemin))?;
                // Le fichier de ce nom que `lister` connait (defaut 3 : le meme pour decider ET pour remplacer).
                let connu = deja.get(nom);
                let distante = if tout { None } else { connu.and_then(|d| d.md5.clone()).filter(|m| !m.is_empty()) };
                if distante.as_deref() == Some(locale.as_str()) {
                    return Ok(None);
                }
                client.televerser(&chemin, &parent, None, connu)?;
                // Defaut 5 : « modifie » est ce qui est REMPLACE (un fichier que Drive connait, avec ou sans empreinte,
                // ou renvoye par `tout`) ; « neuf » est ce qui est CREE.
                Ok(Some(if connu.is_some() { EtatFichier::Modifie } else { EtatFichier::Neuf }))
            };
            match faire() {
                Ok(None) => resultat.identiques += 1,
                Ok(Some(etat)) => {
                    let octets = etat_de[relatif.as_str()].octets;
                    resultat.deposes.push(Fichier { relatif: relatif.clone(), octets, etat });
                    resultat.octets_deposes += octets;
                }
                // LE FICHIER A DISPARU ENTRE L'INVENTAIRE ET L'ENVOI, et ce n'est pas un incident :
                // `paies/avant/` et `config/avant/` ne gardent que les dernieres copies, et
                // l'interface enregistre a chaque frappe. On ne l'inscrit pas a l'index non plus.
                Err(e) if e.classe == "FileNotFoundError" => {
                    resultat.disparus += 1;
                    en_echec.insert(relatif.clone());
                }
                Err(e) => {
                    resultat.incidents.push(format!("{} : {}", relatif, e));
                    en_echec.insert(relatif.clone());
                }
            }
        }
    }

    // L'index n'est ecrit qu'a la fin, et seulement pour ce qui est passe.
    let mut fichiers = Map::new();
    for (r, e) in &presents {
        if !en_echec.contains(r) {
            fichiers.insert(r.clone(), json!({"octets": e.octets, "mtime": e.mtime}));
        }
    }
    let maintenant = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
    let index = json!({
        "quand": crate::paris::iso_minutes(maintenant).replacen('T', " ", 1),
        "racine_drive": racine_drive,
        "dossiers": Value::Object(dossiers),
        "fichiers": Value::Object(fichiers),
    });
    ecrire_index(racine, &index)?;
    resultat.secondes = (debut.elapsed().as_secs_f64() * 10.0).round() / 10.0;
    Ok(resultat)
}
