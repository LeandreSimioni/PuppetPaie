//! La sauvegarde du dossier prive dans le Drive, par differences (`sauvegarde.py`).
//!
//! Ecrit au lot H4b, non prouve. Ce sont des FONCTIONS, pas des routes : leurs appelants sont
//! l'ouverture et la veille (lot I), la ligne de commande (lot J), et `web.py` qui lit
//! `derniere` et `rien_n_a_bouge`. Le client Drive se passe en parametre (`drive::Drive`).
//!
//! CE QU'ELLE SAUVE : tout le dossier des donnees (`config/`, `engagements/`, `sorties/`, `retours/`,
//! `gabarits/`, `journal/`, `reference/`), coffre compris (`coffre.bin`, scelle par le mot de passe du logiciel, qui
//! l'ouvre sur un poste neuf apres `moteur restaurer` : Leandre, 13/10/2026, redit le 14/10), sauf le jeton Google
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

use envois::drive::{Client, Drive, ErreurGoogle};

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
    /// Le detail brut de ce qui n'est pas passe (`dossier : erreur`), pour la ligne de commande. La fenetre ne le montre
    /// pas : un dossier reporte au passage suivant n'est pas un incident (tickets #231 et #232).
    pub incidents: Vec<String>,
    /// Ce qui n'a pas pu monter a ce passage, reporte au suivant.
    pub reportes: Vec<Reporte>,
    /// La phrase de ce qui PERSISTE (`souci`), apres ce passage : `None` tant que ce n'est que passager.
    pub persistant: Option<String>,
    /// Pas de reseau : arretee au premier dossier.
    pub hors_ligne: bool,
}

/// Un dossier (tout ce qu'il porte) ou un fichier qui n'a pas pu monter a un passage : il reste hors de l'index, et le
/// passage suivant le reprend.
#[derive(Debug, Clone, PartialEq)]
pub struct Reporte {
    /// Le chemin relatif du dossier (`.` pour la racine) ou du fichier.
    pub quoi: String,
    pub dossier: bool,
    /// La cause, en clair (`cause_de`).
    pub cause: String,
}

impl Reporte {
    /// La cle de l'index (`echecs`) : un dossier finit par `/`.
    fn cle(&self) -> String {
        if self.dossier {
            format!("{}/", self.quoi)
        } else {
            self.quoi.clone()
        }
    }
}

/// Les causes, en clair. Le detail brut (« HTTP 500 : Internal Error ») reste dans `incidents`, pour la ligne de commande.
pub const GOOGLE_INDISPONIBLE: &str = "Google indisponible";
const GOOGLE_MUET: &str = "Google ne répond pas";
const GOOGLE_REFUSE: &str = "Google refuse l'envoi";
const FICHIER_ILLISIBLE: &str = "fichier illisible sur ce poste";

/// La cause d'une erreur, en clair : passagere chez Google (`drive::passagere`, deja retentee par le client), un delai
/// depasse, un refus de Google, ou un fichier du poste qui ne se lit pas.
pub fn cause_de(e: &ErreurGoogle) -> &'static str {
    if envois::drive::passagere(e) {
        GOOGLE_INDISPONIBLE
    } else if e.classe == "TimeoutError" || e.message.to_lowercase().contains("timed out") {
        GOOGLE_MUET
    } else if matches!(e.classe, "PermissionError" | "OSError") {
        FICHIER_ILLISIBLE
    } else {
        GOOGLE_REFUSE
    }
}

impl Sauvegarde {
    pub fn a_change(&self) -> bool {
        !self.deposes.is_empty()
    }

    /// La ligne de la sauvegarde. Ce qui est reporte au prochain passage s'y dit en passant, sans alarme : c'est
    /// `persistant` (la trace, `sauvegarde_souci`) qui dit ce qui dure.
    pub fn resume(&self) -> String {
        if self.hors_ligne {
            return "Hors connexion : sauvegarde au prochain passage.".into();
        }
        let reporte = if self.reportes.is_empty() { String::new() } else { format!(" ; {} au prochain passage", combien(&self.reportes, "reporte", "reportes")) };
        if self.deposes.is_empty() {
            if reporte.is_empty() {
                return format!("Sauvegarde a jour ({} fichiers).", self.identiques);
            }
            return format!("Sauvegarde : {} fichier(s) inchange(s){}.", self.identiques, reporte);
        }
        format!("Sauvegarde : {} fichier(s) envoye(s), {}, {} inchange(s){}.", self.deposes.len(), lisible(self.octets_deposes), self.identiques, reporte)
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

/// « 1 dossier reporte », « 2 fichiers reportes », « 1 dossier et 2 fichiers reportes ».
fn combien(reportes: &[Reporte], un: &str, plusieurs: &str) -> String {
    let (nd, nf) = (reportes.iter().filter(|r| r.dossier).count(), reportes.iter().filter(|r| !r.dossier).count());
    let nom = |n: usize, quoi: &str| format!("{} {}{}", n, quoi, if n > 1 { "s" } else { "" });
    let (texte, pluriel) = match (nd, nf) {
        (0, f) => (nom(f, "fichier"), f > 1),
        (d, 0) => (nom(d, "dossier"), d > 1),
        (d, f) => (format!("{} et {}", nom(d, "dossier"), nom(f, "fichier")), true),
    };
    format!("{} {}", texte, if pluriel { plusieurs } else { un })
}

// ---------------------------------------------------------------- ce qui persiste

/// CE QUI PERSISTE, ET SEULEMENT CELA (tickets #231 et #232, 20/10/2026). Un dossier reporte au prochain passage n'est pas un
/// incident a montrer : Google rend parfois un 500 que le passage suivant ne voit plus. Ce qui se dit, c'est ce qui dure :
/// le meme dossier (ou fichier) en echec sur `PASSAGES_QUI_DURENT` passages de suite (une heure, a vingt minutes d'une veille a
/// l'autre), ou depuis `UNE_JOURNEE`, ou une journee entiere sans sauvegarde complete.
pub const PASSAGES_QUI_DURENT: i64 = 3;
pub const UNE_JOURNEE: i64 = 86_400;

/// Echecs passagers de suite (chacun deja retente par le client) au-dela desquels le reste du passage est reporte sans appel.
const INDISPONIBLE_D_AFFILEE: usize = 3;

/// « un dossier n'a pas pu être envoyé », « 2 fichiers n'ont pas pu être envoyés ».
fn ce_qui_manque(nd: usize, nf: usize) -> String {
    let nom = |n: usize, quoi: &str| if n == 1 { format!("un {}", quoi) } else { format!("{} {}s", n, quoi) };
    let (texte, pluriel) = match (nd, nf) {
        (0, f) => (nom(f, "fichier"), f > 1),
        (d, 0) => (nom(d, "dossier"), d > 1),
        (d, f) => (format!("{} et {}", nom(d, "dossier"), nom(f, "fichier")), true),
    };
    if pluriel {
        format!("{} n'ont pas pu être envoyés", texte)
    } else {
        format!("{} n'a pas pu être envoyé", texte)
    }
}

/// « depuis 09:51 » le jour meme, « depuis hier », « depuis le 18/10 ».
fn depuis_quand(depuis: i64, maintenant: i64) -> String {
    let iso = socle::paris::iso_de_naif(depuis);
    match maintenant.div_euclid(UNE_JOURNEE) - depuis.div_euclid(UNE_JOURNEE) {
        0 => format!("depuis {}", &iso[11..16]),
        1 => "depuis hier".to_string(),
        _ => format!("depuis le {}/{}", &iso[8..10], &iso[5..7]),
    }
}

/// `AAAA-MM-JJ HH:MM` (l'heure de Paris, comme `quand`) en heure naive.
fn naif(quand: &str) -> Option<i64> {
    socle::paris::naif_de_iso(&quand.replacen(' ', "T", 1))
}

/// La phrase de ce qui persiste dans l'index (`echecs`, `derniere_complete`), a l'heure naive `maintenant`, ou `None`.
fn phrase_persistante(index: &Map<String, Value>, maintenant: i64) -> Option<String> {
    let echecs = index.get("echecs").and_then(Value::as_object).filter(|e| !e.is_empty())?;
    // (cle, depuis, cause) de chaque echec, et ceux qui durent.
    let tous: Vec<(&String, i64, &str, bool)> = echecs
        .iter()
        .map(|(cle, e)| {
            let depuis = e.get("depuis").and_then(Value::as_str).and_then(naif).unwrap_or(maintenant);
            let passages = e.get("passages").and_then(Value::as_i64).unwrap_or(1);
            let cause = e.get("cause").and_then(Value::as_str).unwrap_or(GOOGLE_REFUSE);
            (cle, depuis, cause, passages >= PASSAGES_QUI_DURENT || maintenant - depuis >= UNE_JOURNEE)
        })
        .collect();
    let complete = index.get("derniere_complete").and_then(Value::as_str).and_then(naif);
    let durent: Vec<_> = tous.iter().filter(|t| t.3).collect();
    let (retenus, depuis, par_l_item): (Vec<_>, i64, bool) = if !durent.is_empty() {
        let depuis = durent.iter().map(|t| t.1).min().unwrap_or(maintenant);
        (durent, depuis, true)
    } else {
        match complete.filter(|c| maintenant - c >= UNE_JOURNEE) {
            Some(c) => (tous.iter().collect(), c, false),
            None => return None,
        }
    };
    let nd = retenus.iter().filter(|t| t.0.ends_with('/')).count();
    let nf = retenus.len() - nd;
    // La cause du plus ancien.
    let cause = retenus.iter().min_by_key(|t| t.1).map(|t| t.2).unwrap_or(GOOGLE_REFUSE);
    let quand = depuis_quand(depuis, maintenant);
    Some(if par_l_item {
        format!("Sauvegarde : {} {} ({}) ; le logiciel réessaie.", ce_qui_manque(nd, nf), quand, cause)
    } else {
        format!("Sauvegarde : {} ({}), pas de sauvegarde complète {} ; le logiciel réessaie.", ce_qui_manque(nd, nf), cause, quand)
    })
}

/// `souci` : ce qui persiste de la sauvegarde, dit en clair (« Sauvegarde : un dossier n'a pas pu être envoyé depuis hier
/// (Google indisponible) ; le logiciel réessaie. »), ou `None` si tout est passe, ou si ce n'est que passager. SANS RESEAU :
/// lu dans l'index. La trace de l'ouverture le porte (`sauvegarde_souci`), la fenetre le montre a la place de la sauvegarde.
pub fn souci(racine: &Path) -> Option<String> {
    phrase_persistante(&lire_index(racine), socle::paris::maintenant_naif())
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
        trouves.push((relatif, Etat { octets: m.len(), mtime: socle::paris::mtime(&m) as i64 }));
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
    socle::ecriture::ecrire_json(&chemin_index(racine), index, 1).map_err(|e| ErreurGoogle::nouvelle("OSError", e))
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
            propre = crate::routes::google::client_drive(racine)?;
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
        reportes: Vec::new(),
        persistant: None,
        hors_ligne: false,
    };
    // GOOGLE ABSENT POUR DE BON : le client a deja retente chaque appel (`drive::NOUVELLES_TENTATIVES`) ; apres
    // `INDISPONIBLE_D_AFFILEE` echecs passagers de suite, le reste du passage est reporte sans appeler, plutot que de
    // payer sept secondes d'attente par fichier.
    let mut indisponibles_de_suite = 0usize;
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
        if indisponibles_de_suite >= INDISPONIBLE_D_AFFILEE {
            resultat.reportes.push(Reporte { quoi: dossier.clone(), dossier: true, cause: GOOGLE_INDISPONIBLE.into() });
            en_echec.extend(fichiers.iter().cloned());
            continue;
        }
        let essai: Result<(String, BTreeMap<String, envois::drive::Distant>), ErreurGoogle> = (|| {
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
                let cause = cause_de(&e);
                indisponibles_de_suite = if cause == GOOGLE_INDISPONIBLE { indisponibles_de_suite + 1 } else { 0 };
                resultat.reportes.push(Reporte { quoi: dossier.clone(), dossier: true, cause: cause.into() });
                dossiers.shift_remove(&cle);
                en_echec.extend(fichiers.iter().cloned());
                continue;
            }
        };

        let mut tries: Vec<&String> = fichiers.iter().collect();
        tries.sort();
        for relatif in tries {
            if indisponibles_de_suite >= INDISPONIBLE_D_AFFILEE {
                resultat.reportes.push(Reporte { quoi: relatif.clone(), dossier: false, cause: GOOGLE_INDISPONIBLE.into() });
                en_echec.insert(relatif.clone());
                continue;
            }
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
            let fait = faire();
            if fait.is_ok() {
                indisponibles_de_suite = 0;
            }
            match fait {
                Ok(None) => resultat.identiques += 1,
                Ok(Some(etat)) => {
                    let octets = etat_de[relatif.as_str()].octets;
                    resultat.deposes.push(Fichier { relatif: relatif.clone(), octets, etat });
                    resultat.octets_deposes += octets;
                }
                // LE FICHIER A DISPARU ENTRE L'INVENTAIRE ET L'ENVOI, et ce n'est pas un incident :
                // `engagements/avant/` et `config/avant/` ne gardent que les dernieres copies, et
                // l'interface enregistre a chaque frappe. On ne l'inscrit pas a l'index non plus.
                Err(e) if e.classe == "FileNotFoundError" => {
                    resultat.disparus += 1;
                    en_echec.insert(relatif.clone());
                }
                Err(e) => {
                    resultat.incidents.push(format!("{} : {}", relatif, e));
                    let cause = cause_de(&e);
                    indisponibles_de_suite = if cause == GOOGLE_INDISPONIBLE { indisponibles_de_suite + 1 } else { 0 };
                    resultat.reportes.push(Reporte { quoi: relatif.clone(), dossier: false, cause: cause.into() });
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
    let quand = socle::paris::iso_minutes(maintenant).replacen('T', " ", 1);
    let mut nouvel_index = Map::new();
    nouvel_index.insert("quand".into(), json!(quand));
    nouvel_index.insert("racine_drive".into(), json!(racine_drive));
    nouvel_index.insert("dossiers".into(), Value::Object(dossiers));
    nouvel_index.insert("fichiers".into(), Value::Object(fichiers));
    // CE QUI RESTE EN ECHEC, D'UN PASSAGE A L'AUTRE : de quoi dire ce qui persiste (`souci`). Un passage hors ligne ne compte
    // pas (il n'a rien essaye) : il garde les echecs tels quels. Un passage sans rien de reporte les efface. Les deux cles ne
    // s'ecrivent que s'il y a un echec : l'index d'une sauvegarde complete reste celui d'avant.
    let anciens = index.get("echecs").and_then(Value::as_object).cloned().unwrap_or_default();
    let echecs = if resultat.hors_ligne {
        anciens.clone()
    } else {
        let mut e = Map::new();
        for r in &resultat.reportes {
            let cle = r.cle();
            let avant = anciens.get(&cle);
            let passages = avant.and_then(|a| a.get("passages")).and_then(Value::as_i64).unwrap_or(0) + 1;
            let depuis = avant.and_then(|a| a.get("depuis")).and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| quand.clone());
            e.insert(cle, json!({"depuis": depuis, "passages": passages, "cause": r.cause}));
        }
        e
    };
    if !echecs.is_empty() {
        // La derniere sauvegarde complete : celle que l'index d'avant disait deja, sinon le passage d'avant s'il etait complet.
        let complete = match index.get("derniere_complete") {
            Some(c) => Some(c.clone()),
            None if anciens.is_empty() => index.get("quand").filter(|q| q.as_str().is_some_and(|s| !s.is_empty())).cloned(),
            None => None,
        };
        if let Some(c) = complete {
            nouvel_index.insert("derniere_complete".into(), c);
        }
        nouvel_index.insert("echecs".into(), Value::Object(echecs));
    }
    resultat.persistant = phrase_persistante(&nouvel_index, socle::paris::maintenant_naif());
    ecrire_index(racine, &Value::Object(nouvel_index))?;
    resultat.secondes = (socle::temps::duree(debut).as_secs_f64() * 10.0).round() / 10.0;
    Ok(resultat)
}
