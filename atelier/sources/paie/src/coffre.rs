//! Le coffre (`paie coffre`) : les mots de passe des organismes, scelles par
//! AES-256-GCM. Le fichier `coffre.bin` (`VERSION = 2`) et la cle du poste sont
//! ceux que `moteur/coffre.py` ecrivait ; son en-tete dit le pourquoi.
//!
//! Une cle maitresse de 32 octets scelle les secrets. Elle est scellee deux
//! fois : par la cle du poste (hors des donnees) et par le mot de passe du
//! logiciel (scrypt, parametres ecrits dans le fichier). Les fiches (ce qui est
//! pose, depuis quand) sont en clair et scellent les secrets avec elles.
//!
//! Les secrets ne passent que par l'entree et la sortie standard.
//!
//! LA CLE MAITRESSE SCELLE AUSSI LA CLE DE SIGNATURE DES MISES A JOUR (lot K1, 14/10/2026 ; Leandre : « si mon ordi
//! tombe, on pourra plus faire de mise a jour, ça me va pas »). `proteger_par_le_coffre` scelle par elle ce qui doit
//! s'ouvrir partout ou le coffre s'ouvre : la cle du poste, ou le mot de passe du logiciel sur un autre poste. Ce qui est
//! scelle ainsi depend du coffre : un coffre qui le porte ne s'efface pas en se vidant (`scelle_ailleurs`).

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use serde_json::{Map, Value};

use crate::retours::base64_ecrire;

pub const FICHIER: &str = "coffre.bin";
pub const FICHIER_CLE: &str = "cle-poste.bin";
pub const GARDEE: &str = "ouverture-gardee.bin";
pub const VERSION: i64 = 2;
pub const CONSULTATION: &str = "consultation";
pub const DEPOT: &str = "depot";
pub const DESCRIPTION: &str = "PuppetPaie - mots de passe des organismes";

const SCRYPT_N: u64 = 32768;
const SCRYPT_R: u64 = 8;
const SCRYPT_P: u64 = 1;
const SCRYPT_MAXMEM: u64 = 96 * 1024 * 1024;
pub const LONGUEUR_MINIMALE: usize = 6;
pub const LONGUEUR_MINIMALE_LOGICIEL: usize = 6;

const AAD_POSTE: &[u8] = b"puppetpaie/enveloppe/poste";
const AAD_MOTDEPASSE: &[u8] = b"puppetpaie/enveloppe/motdepasse";
const AAD_GARDEE: &[u8] = b"puppetpaie/ouverture-gardee";
/// Ce que la cle maitresse scelle hors du coffre : ce prefixe, puis la description de ce qui est scelle.
const AAD_MAITRESSE: &str = "puppetpaie/maitresse/";

pub struct Service {
    pub nom: &'static str,
    pub libelle: &'static str,
    pub variable: &'static str,
    pub sert_a: &'static str,
    pub avec_compte: bool,
}

pub const SERVICES: [Service; 3] = [
    Service {
        nom: "net-entreprises",
        libelle: "net-entreprises (DSN)",
        variable: "NET_ENTREPRISES_MOTDEPASSE",
        sert_a: "relever les retours, lister les dépôts, déposer une DSN",
        avec_compte: false,
    },
    Service {
        nom: "urssaf",
        libelle: "Urssaf (DPAE)",
        variable: "URSSAF_MOTDEPASSE",
        sert_a: "vérifier les identifiants et déposer une DPAE",
        avec_compte: false,
    },
    Service {
        nom: "courriel",
        libelle: "Boîte courriel (IMAP et SMTP)",
        variable: "COURRIEL_MOTDEPASSE",
        sert_a: "envoyer les documents et les contrats, relever les signatures",
        avec_compte: true,
    },
];

// ------------------------------------------------------------------ les refus

/// Un refus : la classe d'exception que le Python levait, et sa phrase.
#[derive(Debug, Clone)]
pub struct Echec {
    pub classe: &'static str,
    pub message: String,
    /// `errno`, `strerror` et `filename` d'une `OSError`.
    pub os: Option<(i32, String, String)>,
}

pub type R<T> = Result<T, Echec>;

fn echec(classe: &'static str, message: impl Into<String>) -> Echec {
    Echec { classe, message: message.into(), os: None }
}

/// `ErreurCoffre` : le coffre refuse, la phrase est ecrite pour etre lue.
fn refus(message: impl Into<String>) -> Echec {
    echec("ErreurCoffre", message)
}

fn oserr(e: &std::io::Error, p: &Path) -> Echec {
    let texte = e.to_string();
    let strerror = match texte.rfind(" (os error ") {
        Some(i) => texte[..i].to_string(),
        None => texte,
    };
    let errno = e.raw_os_error().unwrap_or(0);
    let nom = affiche(p);
    Echec {
        classe: "OSError",
        message: crate::fichiers::panne_fichier(e, p),
        os: Some((errno, strerror, nom)),
    }
}

// ------------------------------------------------- ce que Python fait en passant

fn affiche(p: &Path) -> String {
    p.display().to_string()
}

/// Le chemin tel que `pathlib` l'ecrit : sans `.` ni separateur double.
fn normal(p: PathBuf) -> PathBuf {
    let mut sortie = PathBuf::new();
    for c in p.components() {
        if c != Component::CurDir {
            sortie.push(c.as_os_str());
        }
    }
    if sortie.as_os_str().is_empty() {
        sortie.push(".");
    }
    sortie
}

/// `Path.resolve()` : absolu, liens suivis aussi loin que le chemin existe.
fn resoudre(p: &Path) -> PathBuf {
    let absolu = if p.is_absolute() { p.to_path_buf() } else { std::env::current_dir().unwrap_or_default().join(p) };
    let mut reste: Vec<std::ffi::OsString> = Vec::new();
    let mut tete = absolu;
    loop {
        if let Ok(c) = std::fs::canonicalize(&tete) {
            let mut sortie = c;
            for r in reste.iter().rev() {
                if r == ".." {
                    sortie.pop();
                } else if r != "." {
                    sortie.push(r);
                }
            }
            let t = sortie.to_string_lossy().to_string();
            return match t.strip_prefix(r"\\?\") {
                Some(s) if cfg!(windows) => PathBuf::from(s),
                _ => sortie,
            };
        }
        match (tete.file_name().map(|n| n.to_os_string()), tete.parent().map(Path::to_path_buf)) {
            (Some(n), Some(parent)) => {
                reste.push(n);
                tete = parent;
            }
            _ => return tete,
        }
    }
}

/// `str.isspace()` de Python.
fn espace(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

fn strip(s: &str) -> &str {
    s.trim_matches(espace)
}

fn vrai(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

fn type_python(v: &Value) -> &'static str {
    match v {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::Number(n) => {
            if n.is_f64() {
                "float"
            } else {
                "int"
            }
        }
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
    }
}

use crate::fichiers::imprimable;

/// `repr()` d'une chaine, comme Python l'ecrit.
fn repr_chaine(s: &str) -> String {
    let guillemet = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut t = String::new();
    t.push(guillemet);
    for c in s.chars() {
        match c {
            '\\' => t.push_str("\\\\"),
            '\n' => t.push_str("\\n"),
            '\r' => t.push_str("\\r"),
            '\t' => t.push_str("\\t"),
            c if c == guillemet => {
                t.push('\\');
                t.push(c);
            }
            c if !imprimable(c) => {
                let n = c as u32;
                if n < 0x100 {
                    t.push_str(&format!("\\x{:02x}", n));
                } else if n < 0x10000 {
                    t.push_str(&format!("\\u{:04x}", n));
                } else {
                    t.push_str(&format!("\\U{:08x}", n));
                }
            }
            c => t.push(c),
        }
    }
    t.push(guillemet);
    t
}

fn repr_valeur(v: &Value) -> String {
    match v {
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::String(s) => repr_chaine(s),
        Value::Number(n) => match n.as_f64().filter(|_| n.is_f64()) {
            Some(f) => repr_flottant(f),
            None => n.to_string(),
        },
        autre => autre.to_string(),
    }
}

/// `repr(float)` : le plus court qui se relit, notation scientifique hors de
/// 1e-4 .. 1e16.
fn repr_flottant(f: f64) -> String {
    if f.is_nan() {
        return "NaN".into();
    }
    if f.is_infinite() {
        return if f > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    let signe = if f.is_sign_negative() { "-" } else { "" };
    let sci = format!("{:e}", f.abs());
    let (mant, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let chiffres: String = mant.chars().filter(|c| *c != '.').collect();
    let n = chiffres.len() as i32;
    let decpt = exp + 1;
    let corps = if f == 0.0 {
        "0.0".to_string()
    } else if decpt > 16 || decpt < -3 {
        let mut m = chiffres[..1].to_string();
        if n > 1 {
            m.push('.');
            m.push_str(&chiffres[1..]);
        }
        format!("{}e{}{:02}", m, if exp < 0 { '-' } else { '+' }, exp.abs())
    } else if decpt <= 0 {
        format!("0.{}{}", "0".repeat((-decpt) as usize), chiffres)
    } else if decpt >= n {
        format!("{}{}.0", chiffres, "0".repeat((decpt - n) as usize))
    } else {
        format!("{}.{}", &chiffres[..decpt as usize], &chiffres[decpt as usize..])
    };
    format!("{}{}", signe, corps)
}

/// `json.dumps` de Python : `ensure_ascii`, cles triees si `trie`. Sans
/// `indent`, les separateurs sont `, ` et `: ` (ou `,` et `:` si `serre`).
pub fn dumps(v: &Value, indent: Option<usize>, serre: bool, trie: bool) -> String {
    let mut s = String::new();
    ecrire(v, indent, serre, trie, 0, &mut s);
    s
}

fn chaine_json(t: &str, s: &mut String) {
    s.push('"');
    for c in t.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            '\r' => s.push_str("\\r"),
            '\t' => s.push_str("\\t"),
            '\u{8}' => s.push_str("\\b"),
            '\u{c}' => s.push_str("\\f"),
            ' '..='~' => s.push(c),
            c => {
                let mut tampon = [0u16; 2];
                for u in c.encode_utf16(&mut tampon) {
                    s.push_str(&format!("\\u{:04x}", u));
                }
            }
        }
    }
    s.push('"');
}

fn ecrire(v: &Value, indent: Option<usize>, serre: bool, trie: bool, niveau: usize, s: &mut String) {
    let (sep_item, sep_cle) = match (indent, serre) {
        (Some(_), _) => (",", ": "),
        (None, true) => (",", ":"),
        (None, false) => (", ", ": "),
    };
    let retrait = |n: usize| match indent {
        Some(i) => format!("\n{}", " ".repeat(i * n)),
        None => String::new(),
    };
    match v {
        Value::Null => s.push_str("null"),
        Value::Bool(b) => s.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => match n.as_f64().filter(|_| n.is_f64()) {
            Some(f) => s.push_str(&repr_flottant(f)),
            None => s.push_str(&n.to_string()),
        },
        Value::String(t) => chaine_json(t, s),
        Value::Array(a) => {
            if a.is_empty() {
                s.push_str("[]");
                return;
            }
            s.push('[');
            s.push_str(&retrait(niveau + 1));
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    s.push_str(sep_item);
                    s.push_str(&retrait(niveau + 1));
                }
                ecrire(x, indent, serre, trie, niveau + 1, s);
            }
            s.push_str(&retrait(niveau));
            s.push(']');
        }
        Value::Object(o) => {
            if o.is_empty() {
                s.push_str("{}");
                return;
            }
            let mut entrees: Vec<(&String, &Value)> = o.iter().collect();
            if trie {
                entrees.sort_by(|a, b| a.0.cmp(b.0));
            }
            s.push('{');
            s.push_str(&retrait(niveau + 1));
            for (i, (k, x)) in entrees.into_iter().enumerate() {
                if i > 0 {
                    s.push_str(sep_item);
                    s.push_str(&retrait(niveau + 1));
                }
                chaine_json(k, s);
                s.push_str(sep_cle);
                ecrire(x, indent, serre, trie, niveau + 1, s);
            }
            s.push_str(&retrait(niveau));
            s.push('}');
        }
    }
}

/// `base64.b64decode(texte)` de Python : ce qui n'est pas de l'alphabet est
/// ignore, le remplissage s'arrete la ou il est complet.
pub fn d64(v: &Value) -> R<Vec<u8>> {
    let t = match v {
        Value::String(t) => t,
        autre => {
            return Err(echec(
                "TypeError",
                format!("argument should be a bytes-like object or ASCII string, not '{}'", type_python(autre)),
            ))
        }
    };
    if !t.is_ascii() {
        return Err(echec("ValueError", "string argument should contain only ASCII characters"));
    }
    let (mut sortie, mut quad, mut reste, mut bourrage) = (Vec::new(), 0u8, 0u8, 0u8);
    for c in t.bytes() {
        if c == b'=' {
            if quad >= 2 {
                bourrage += 1;
                if quad + bourrage >= 4 {
                    return Ok(sortie);
                }
            }
            continue;
        }
        let Some(x) = ALPHABET.iter().position(|a| *a == c) else { continue };
        let x = x as u8;
        bourrage = 0;
        match quad {
            0 => {
                quad = 1;
                reste = x;
            }
            1 => {
                quad = 2;
                sortie.push(reste << 2 | x >> 4);
                reste = x & 0x0f;
            }
            2 => {
                quad = 3;
                sortie.push(reste << 4 | x >> 2);
                reste = x & 0x03;
            }
            _ => {
                quad = 0;
                sortie.push(reste << 6 | x);
                reste = 0;
            }
        }
    }
    match quad {
        0 => Ok(sortie),
        1 => Err(echec(
            "Error",
            format!(
                "Invalid base64-encoded string: number of data characters ({}) cannot be 1 more than a multiple of 4",
                sortie.len() / 3 * 4 + 1
            ),
        )),
        _ => Err(echec("Error", "Incorrect padding")),
    }
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn b64(o: &[u8]) -> Value {
    Value::String(base64_ecrire(o))
}

/// `int(v)` de Python.
fn entier(v: &Value) -> R<i128> {
    match v {
        Value::Bool(b) => Ok(*b as i128),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(i as i128)
            } else if let Some(u) = n.as_u64() {
                Ok(u as i128)
            } else {
                let f = n.as_f64().unwrap_or(0.0);
                if f.is_finite() {
                    Ok(f.trunc() as i128)
                } else {
                    Err(echec("ValueError", "cannot convert float to integer"))
                }
            }
        }
        Value::String(t) => {
            let s = strip(t);
            let (neg, corps) = match s.strip_prefix('-') {
                Some(r) => (true, r),
                None => (false, s.strip_prefix('+').unwrap_or(s)),
            };
            let valide = !corps.is_empty()
                && corps.bytes().all(|b| b.is_ascii_digit() || b == b'_')
                && !corps.starts_with('_')
                && !corps.ends_with('_')
                && !corps.contains("__");
            let n: Option<i128> = if valide { corps.replace('_', "").parse().ok() } else { None };
            match n {
                Some(n) => Ok(if neg { -n } else { n }),
                None => Err(echec("ValueError", format!("nombre entier illisible : {}", repr_chaine(t)))),
            }
        }
        autre => Err(echec(
            "TypeError",
            crate::fichiers::attendu("un nombre entier", type_python(autre)),
        )),
    }
}

/// `x or {}` puis `.get(...)` : un objet, ou rien.
fn objet(v: Option<&Value>) -> R<Map<String, Value>> {
    match v {
        None | Some(Value::Null) => Ok(Map::new()),
        Some(Value::Object(o)) => Ok(o.clone()),
        Some(x) if !vrai(x) => Ok(Map::new()),
        Some(x) => Err(echec("AttributeError", crate::fichiers::attendu("un objet", type_python(x)))),
    }
}

/// `contenu.get("fiches") or {}`, tel quel.
fn fiches_brutes(c: &Map<String, Value>) -> Value {
    match c.get("fiches") {
        Some(v) if vrai(v) => v.clone(),
        _ => Value::Object(Map::new()),
    }
}

/// `nom in fiches`, pour ce que Python en fait.
fn dans(fiches: &Value, nom: &str) -> bool {
    match fiches {
        Value::Object(o) => o.contains_key(nom),
        Value::String(t) => t.contains(nom),
        Value::Array(a) => a.iter().any(|x| x.as_str() == Some(nom)),
        _ => false,
    }
}

fn existe(p: &Path) -> R<bool> {
    match std::fs::metadata(p) {
        Ok(_) => Ok(true),
        Err(e) => match e.raw_os_error() {
            Some(2 | 20 | 9 | 40) => Ok(false),
            _ if e.kind() == std::io::ErrorKind::NotFound || cfg!(windows) => Ok(false),
            _ => Err(oserr(&e, p)),
        },
    }
}

fn lire_octets(p: &Path) -> R<Vec<u8>> {
    std::fs::read(p).map_err(|e| oserr(&e, p))
}

/// La cle du poste telle qu'elle est posee. Une cle plus courte que 32 octets peut etre celle qu'une autre commande
/// est en train d'ecrire (creee, pas encore remplie : reste 158) : on la relit un instant avant de la dire abimee.
fn lire_cle_posee(p: &Path) -> R<Vec<u8>> {
    let mut cle = lire_octets(p)?;
    for _ in 0..50 {
        if cle.len() >= 32 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        cle = lire_octets(p)?;
    }
    Ok(cle)
}

fn effacer(p: &Path) -> R<()> {
    match std::fs::remove_file(p) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(oserr(&e, p)),
    }
}

fn dossier_de(p: &Path) -> R<()> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| oserr(&e, d))?;
    }
    Ok(())
}

#[cfg(unix)]
fn permissions(p: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode));
}

#[cfg(not(unix))]
fn permissions(_: &Path, _: u32) {}

fn aleas(n: usize) -> R<Vec<u8>> {
    let mut o = vec![0u8; n];
    getrandom::fill(&mut o).map_err(|e| echec("OSError", format!("pas d'alea du systeme : {}", e)))?;
    Ok(o)
}

// ---------------------------------------------------------------- le scellage

fn boite(cle: &[u8]) -> R<Aes256Gcm> {
    Aes256Gcm::new_from_slice(cle).map_err(|_| echec("ValueError", "AESGCM key must be 128, 192, or 256 bits."))
}

/// nonce (12 octets) suivi du scelle.
pub fn sceller(cle: &[u8], clair: &[u8], aad: &[u8]) -> R<Vec<u8>> {
    let b = boite(cle)?;
    let nonce = aleas(12)?;
    let mut sortie = nonce.clone();
    let scelle = b
        .encrypt(Nonce::from_slice(&nonce), Payload { msg: clair, aad })
        .map_err(|_| echec("ValueError", "chiffrement impossible"))?;
    sortie.extend(scelle);
    Ok(sortie)
}

pub fn desceller(cle: &[u8], scelle: &[u8], aad: &[u8]) -> R<Vec<u8>> {
    let b = boite(cle)?;
    if scelle.len() < 13 {
        return Err(refus(format!("Scelle tronque : {} octets.", scelle.len())));
    }
    b.decrypt(Nonce::from_slice(&scelle[..12]), Payload { msg: &scelle[12..], aad }).map_err(|_| {
        refus(
            "Le scelle refuse de s'ouvrir : ce n'est pas la bonne cle, ou le \
             contenu a ete modifie.",
        )
    })
}

fn aad_fiches(fiches: &Value) -> Vec<u8> {
    dumps(fiches, None, true, true).into_bytes()
}

/// La cle scrypt de 32 octets. Les parametres sont ceux du fichier : ils sont
/// verifies comme `hashlib.scrypt` le fait (OpenSSL), memoire comprise.
pub fn deriver(motdepasse: &str, kdf: Option<&Value>) -> R<Vec<u8>> {
    let kdf = objet(kdf)?;
    let algo = kdf.get("algo").filter(|v| vrai(v));
    if algo.and_then(Value::as_str) != Some("scrypt") {
        let dit = match algo {
            Some(v) => repr_valeur(v),
            None => "'rien'".into(),
        };
        return Err(refus(format!("Ce coffre dit avoir ete ferme par {}, que cette version ne sait pas ouvrir.", dit)));
    }
    let cle = |nom: &str| kdf.get(nom).ok_or_else(|| echec("KeyError", crate::fichiers::absent(nom)));
    let sel = d64(cle("sel")?)?;
    let n = entier(cle("n")?)?;
    let r = entier(cle("r")?)?;
    let p = entier(cle("p")?)?;
    scrypt_32(motdepasse.as_bytes(), &sel, n, r, p)
}

fn scrypt_32(motdepasse: &[u8], sel: &[u8], n: i128, r: i128, p: i128) -> R<Vec<u8>> {
    let invalide = || echec("ValueError", "Invalid parameter combination for n, r, p, maxmem.");
    let memoire_max = || echec("ValueError", "[digital envelope routines] memory limit exceeded");
    for (nom, v) in [("n", n), ("r", r), ("p", p)] {
        if v < 0 {
            return Err(echec("TypeError", format!("{} is required and must be an unsigned int", nom)));
        }
    }
    if n < 2 || n & (n - 1) != 0 {
        return Err(echec("ValueError", "n must be a power of 2."));
    }
    if r == 0 || p == 0 {
        return Err(invalide());
    }
    if r > u32::MAX as i128 || p > u32::MAX as i128 {
        return Err(echec("ValueError", "[digital envelope routines] parameter too large"));
    }
    if p > ((1i128 << 30) - 1) / r {
        return Err(memoire_max());
    }
    let log_n = n.trailing_zeros() as i128;
    if 16 * r <= 63 && n >= 1i128 << (16 * r) {
        return Err(memoire_max());
    }
    let memoire = 128 * p * r + 128 * r * (n + 2);
    if memoire > SCRYPT_MAXMEM as i128 || 128 * p * r > i32::MAX as i128 {
        return Err(memoire_max());
    }
    let params = scrypt::Params::new(log_n as u8, r as u32, p as u32, 32).map_err(|_| invalide())?;
    let mut sortie = vec![0u8; 32];
    scrypt::scrypt(motdepasse, sel, &params, &mut sortie).map_err(|_| invalide())?;
    Ok(sortie)
}

// ------------------------------------------------------------- le poste et ses chemins

// LE MOT DE PASSE TAPE OUVRE TOUT (Leandre, 07/10/2026 : « le logiciel devra fonctionner entierement avec ce
// systeme »). La ou la cle du poste n'ouvre pas le coffre -- un autre compte, un AppData virtuel --, le mot de passe
// du logiciel tape par un humain pour CETTE commande l'ouvre, pour tout ce qu'elle lit : consultation, envois,
// verification des comptes, releves, depots. Il arrive par la demande (`motdepasse` d'une route) ou par
// `PUPPETPAIE_MOTDEPASSE`, la variable que les fenetres posent apres l'avoir fait taper. Il ne change rien au poste
// et ne s'ecrit nulle part. Par fil, et le temps d'une route : la fenetre sert ses routes dans son
// processus, une par fil a la fois, et `routes::commande` remet en sortant ce qu'il a trouve, quoi qu'il arrive (ticket
// #244) ; les tests, chacun sur son fil, ne se le transmettent pas.
thread_local! {
    static TAPE: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// Retient, pour cette commande, le mot de passe du logiciel qu'un humain vient de taper.
pub fn retenir_mot_de_passe_tape(motdepasse: &str) {
    if !motdepasse.is_empty() {
        TAPE.with(|t| *t.borrow_mut() = Some(motdepasse.to_string()));
    }
}

/// Oublie le mot de passe retenu pour ce fil (la fin d'une commande, ou d'un test).
pub fn oublier_mot_de_passe_tape() {
    TAPE.with(|t| *t.borrow_mut() = None);
}

/// Un mot de passe tape est-il retenu sur ce fil ? (Rien de plus : jamais sa valeur.) Les essais de la fenetre s'en
/// servent : un mot de passe ne survit pas a sa route (ticket #244).
pub fn mot_de_passe_retenu() -> bool {
    TAPE.with(|t| t.borrow().is_some())
}

/// Le mot de passe tape pour cette commande : celui de la demande, sinon `PUPPETPAIE_MOTDEPASSE`.
fn mot_de_passe_tape() -> Option<String> {
    TAPE.with(|t| t.borrow().clone()).or_else(|| env_non_vide("PUPPETPAIE_MOTDEPASSE"))
}

/// Le marqueur d'un dossier PORTABLE, a cote de `logiciel/` (Leandre, 14/10/2026 : « un admin ne depend pas d'un
/// ordi ») : le logiciel, ses donnees et la cle qui les scelle voyagent ensemble -- une cle USB -- et s'ouvrent sur
/// n'importe quel ordinateur, quel que soit le systeme. La cle du poste vit alors dans ce dossier, pas sur la machine.
pub const MARQUEUR_PORTABLE: &str = "PORTABLE.json";

/// Le dossier portable, s'il y en a un : ce qui est a cote de `logiciel/` quand il porte `PORTABLE.json`.
pub fn dossier_portable() -> Option<PathBuf> {
    let base = crate::interface::compagnies::base();
    base.join(MARQUEUR_PORTABLE).is_file().then_some(base)
}

/// Ou vit la cle du poste : `PUPPETPAIE_CLE` la deplace ; sinon le dossier portable (`dossier_portable`), s'il y en a
/// un ; sinon la machine, `%APPDATA%\PuppetPaie` (Windows) ou `$XDG_CONFIG_HOME/puppetpaie` (`~/.config/puppetpaie`).
pub fn dossier_cle_de(env: &dyn Fn(&str) -> Option<String>, portable: Option<&Path>) -> PathBuf {
    let maison = || -> PathBuf {
        env("HOME")
            .map(|h| if h.is_empty() { "/".to_string() } else { h })
            .or_else(|| if cfg!(windows) { env("USERPROFILE") } else { None })
            .map(PathBuf::from)
            .or_else(|| {
                #[allow(deprecated)]
                std::env::home_dir()
            })
            .unwrap_or_else(|| PathBuf::from("/"))
    };
    if let Some(force) = env("PUPPETPAIE_CLE") {
        let p = match force.strip_prefix('~') {
            Some(suite) if suite.is_empty() || suite.starts_with('/') || (cfg!(windows) && suite.starts_with('\\')) => {
                PathBuf::from(format!("{}{}", affiche(&maison()), suite))
            }
            _ => PathBuf::from(force),
        };
        return normal(p);
    }
    if let Some(dossier) = portable {
        return normal(dossier.to_path_buf());
    }
    if cfg!(windows) {
        let base = env("APPDATA").map(PathBuf::from).unwrap_or_else(|| maison().join("AppData").join("Roaming"));
        return normal(base.join("PuppetPaie"));
    }
    let base = env("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| maison().join(".config"));
    normal(base.join("puppetpaie"))
}

fn env_non_vide(nom: &str) -> Option<String> {
    std::env::var(nom).ok().filter(|v| !v.is_empty())
}

/// Le coffre d'une compagnie, vu depuis un poste.
pub struct Coffre {
    pub racine: PathBuf,
    pub dossier_cle: PathBuf,
    /// L'heure locale, `AAAA-MM-JJTHH:MM` : le moteur n'a pas de fuseau, le
    /// Python la donne.
    pub maintenant: String,
    /// Les variables d'environnement des services (elles passent devant le coffre).
    pub variables: BTreeMap<String, String>,
}

impl Coffre {
    /// Le coffre de `racine` avec l'environnement de ce processus.
    pub fn du_poste(racine: impl Into<PathBuf>, maintenant: &str) -> Coffre {
        Coffre {
            racine: racine.into(),
            dossier_cle: dossier_cle_de(&|n| if n == "HOME" { std::env::var(n).ok() } else { env_non_vide(n) }, dossier_portable().as_deref()),
            maintenant: maintenant.to_string(),
            variables: SERVICES.iter().filter_map(|s| env_non_vide(s.variable).map(|v| (s.variable.to_string(), v))).collect(),
        }
    }

    /// Le coffre de `racine` avec la cle du poste dans `dossier_cle`, sans variable de service : celui que l'administrateur
    /// rescelle (`administrateur.rs`), et que ses essais montent sans toucher a l'environnement du processus.
    pub fn sur(racine: impl Into<PathBuf>, dossier_cle: impl Into<PathBuf>, maintenant: &str) -> Coffre {
        Coffre { racine: racine.into(), dossier_cle: dossier_cle.into(), maintenant: maintenant.to_string(), variables: BTreeMap::new() }
    }

    /// Le dossier de la cle de CE poste, tel que `du_poste` le trouve.
    pub fn dossier_cle_du_poste() -> PathBuf {
        dossier_cle_de(&|n| if n == "HOME" { std::env::var(n).ok() } else { env_non_vide(n) }, dossier_portable().as_deref())
    }

    /// L'heure locale `AAAA-MM-JJTHH:MM` : celle qu'on a donnee, sinon l'horloge. Les lecteurs du moteur font leur
    /// coffre sans heure (`du_poste(racine, "")`) et la seance, elle, expire : une heure vide ne doit pas la rendre eternelle.
    fn heure(&self) -> String {
        if !self.maintenant.is_empty() {
            return self.maintenant.clone();
        }
        crate::releve_retours::iso_de_naif(crate::releve_retours::maintenant_naif())[..16].to_string()
    }

    pub fn chemin(&self) -> PathBuf {
        self.racine.join("config").join(FICHIER)
    }

    pub fn chemin_cle(&self) -> PathBuf {
        self.dossier_cle.join(FICHIER_CLE)
    }

    pub fn chemin_gardee(&self) -> PathBuf {
        self.dossier_cle.join(GARDEE)
    }

    pub fn cle_posee(&self) -> R<bool> {
        existe(&self.chemin_cle())
    }

    /// Les 32 octets de ce poste, ou None. `creer` en tire de neufs.
    fn cle_poste(&self, creer: bool) -> R<Option<Vec<u8>>> {
        let p = self.chemin_cle();
        if existe(&p)? {
            let cle = lire_cle_posee(&p)?;
            if cle.len() != 32 {
                return Err(refus(format!(
                    "La cle de ce poste ({}) fait {} octets au lieu de 32 : elle \
                     est abimee. La supprimer, puis rouvrir le coffre avec le mot \
                     de passe du logiciel -- une cle neuve sera posee.",
                    affiche(&p),
                    cle.len()
                )));
            }
            return Ok(Some(cle));
        }
        if !creer {
            return Ok(None);
        }
        dossier_de(&p)?;
        if let Some(d) = p.parent() {
            permissions(d, 0o700);
        }
        let cle = aleas(32)?;
        // 0600 des l'ouverture, et O_EXCL : pas d'instant ou la cle se lit.
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut f = match options.open(&p) {
            Ok(f) => f,
            // Une autre commande l'a posee entre `existe` et ici (reste 158) : c'est la sienne qui vaut, sinon le
            // coffre se scellerait avec deux cles. On la relit, elle et elle seule.
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return self.cle_poste(false),
            Err(e) => return Err(oserr(&e, &p)),
        };
        f.write_all(&cle).map_err(|e| oserr(&e, &p))?;
        Ok(Some(cle))
    }

    fn cle_ou_creer(&self) -> R<Vec<u8>> {
        self.cle_poste(true)?.ok_or_else(|| echec("OSError", "la cle de ce poste n'a pas pu etre posee"))
    }

    pub fn poser_cle_poste(&self) -> R<PathBuf> {
        self.cle_poste(true)?;
        Ok(self.chemin_cle())
    }

    // -------------------------------------------- proteger / deproteger

    pub fn proteger(&self, donnees: &[u8], description: &str) -> R<Vec<u8>> {
        sceller(&self.cle_ou_creer()?, donnees, description.as_bytes())
    }

    pub fn deproteger(&self, donnees: &[u8], description: &str) -> R<Vec<u8>> {
        let Some(cle) = self.cle_poste(false)? else {
            return Err(refus(format!(
                "La cle de ce poste est absente ({}) : ce fichier a ete protege \
                 sur une autre machine, ou la cle a ete perdue. Il faut reposer ce \
                 qu'il contenait.",
                affiche(&self.chemin_cle())
            )));
        };
        desceller(&cle, donnees, description.as_bytes())
    }

    /// Scelle `donnees` par la cle maitresse de ce coffre (lot K1) : ce qui est scelle ainsi s'ouvre partout ou le coffre
    /// s'ouvre, la cle de ce poste ou le mot de passe du logiciel ailleurs. N'ecrit rien. Refuse sans coffre, ou sans mot
    /// de passe du logiciel : seule la cle de ce poste l'ouvrirait, et ce qui est scelle partirait avec le poste.
    pub fn proteger_par_le_coffre(&self, donnees: &[u8], description: &str) -> R<Vec<u8>> {
        let contenu = self.lire_fichier()?;
        if contenu.is_empty() || !objet(contenu.get("enveloppes"))?.get("motdepasse").is_some_and(vrai) {
            return Err(refus(format!(
                "Le coffre de cette compagnie ({}) n'a pas de mot de passe du logiciel : scelle par lui, « {} » ne \
                 s'ouvrirait que sur ce poste et serait perdu avec lui. Rien n'a ete ecrit. Poser d'abord le mot de \
                 passe du logiciel :\n\
                 \x20 paie coffre ouverture",
                affiche(&self.chemin()),
                description
            )));
        }
        let Some(maitresse) = self.ouvrir(&contenu, None)? else { return Err(self.refus_ferme()?) };
        sceller(&maitresse, donnees, format!("{}{}", AAD_MAITRESSE, description).as_bytes())
    }

    /// Ouvre ce que `proteger_par_le_coffre` a scelle : par la cle de ce poste, sinon par le mot de passe du logiciel tape
    /// pour la commande, sinon par la seance (`ouvrir`).
    pub fn deproteger_par_le_coffre(&self, scelle: &[u8], description: &str) -> R<Vec<u8>> {
        let contenu = self.lire_fichier()?;
        if contenu.is_empty() {
            return Err(refus(format!(
                "« {} » est scelle par le coffre de la compagnie, et il n'y a pas de coffre ici ({}) : rien d'autre ne \
                 l'ouvre, et un coffre neuf ne l'ouvrira pas. Remettre le coffre qui l'a scelle (`config/coffre.bin` de \
                 l'ancien poste).",
                description,
                affiche(&self.chemin())
            )));
        }
        let Some(maitresse) = self.ouvrir(&contenu, None)? else { return Err(self.refus_ferme()?) };
        desceller(&maitresse, scelle, format!("{}{}", AAD_MAITRESSE, description).as_bytes())
    }

    /// Ce que la cle maitresse scelle hors du coffre, s'il y en a (la cle de signature des mises a jour) : tant qu'il est
    /// la, le coffre ne s'efface pas quand il ne porte plus rien. Sa cle maitresse est la seule qui l'ouvre.
    fn scelle_ailleurs(&self) -> bool {
        crate::maj::cle_scellee_par_le_coffre(&self.racine) == Some(true)
    }

    // ------------------------------------------------------------ le fichier

    /// Le coffre tel qu'il est sur le disque ; vide s'il n'y en a pas.
    fn lire_fichier(&self) -> R<Map<String, Value>> {
        let p = self.chemin();
        if !existe(&p)? {
            return Ok(Map::new());
        }
        let brut = lire_octets(&p)?;
        let contenu = std::str::from_utf8(&brut).ok().and_then(|t| serde_json::from_str::<Value>(t).ok());
        if let Some(Value::Object(o)) = contenu {
            let version = o.get("version").and_then(|v| match v {
                Value::Bool(b) => Some(*b as i64 as f64),
                Value::Number(n) => n.as_f64(),
                _ => None,
            });
            if version == Some(VERSION as f64) {
                return Ok(o);
            }
        }
        Err(refus(format!(
            "Le coffre ({}) n'est pas au format de cette version -- il est plus \
             ancien, ou abime. Il ne se convertit pas : le supprimer, avec \
             `google-jeton.bin` et `maj-cle-privee.bin` qui sont fermes de la \
             meme facon, puis reposer ce qu'ils contenaient :\n\
             \x20 paie coffre ouverture\n\
             \x20 paie coffre poser net-entreprises\n\
             \x20 paie coffre poser urssaf\n\
             \x20 paie coffre poser courriel --compte <adresse>\n\
             Puis relier le Drive, et refaire une cle de signature si ce poste \
             publie les mises a jour.",
            affiche(&p)
        )))
    }

    /// Un coffre neuf : (cle maitresse, le contenu a ecrire). L'enveloppe du
    /// poste est posee tout de suite ; celle du mot de passe vient avec lui.
    fn creer(&self) -> R<(Vec<u8>, Map<String, Value>)> {
        let maitresse = aleas(32)?;
        let mut kdf = Map::new();
        kdf.insert("algo".into(), "scrypt".into());
        kdf.insert("n".into(), SCRYPT_N.into());
        kdf.insert("r".into(), SCRYPT_R.into());
        kdf.insert("p".into(), SCRYPT_P.into());
        kdf.insert("sel".into(), b64(&aleas(16)?));
        let mut enveloppes = Map::new();
        enveloppes.insert("poste".into(), b64(&sceller(&self.cle_ou_creer()?, &maitresse, AAD_POSTE)?));
        let mut c = Map::new();
        c.insert("version".into(), VERSION.into());
        c.insert("kdf".into(), Value::Object(kdf));
        c.insert("enveloppes".into(), Value::Object(enveloppes));
        c.insert("fiches".into(), Value::Object(Map::new()));
        c.insert("secrets".into(), "".into());
        Ok((maitresse, c))
    }

    /// La cle maitresse, ou None si rien ne l'ouvre ici : la cle du poste
    /// d'abord, le mot de passe du logiciel ensuite -- celui qu'on donne, sinon
    /// celui qu'un humain a tape pour cette commande (`mot_de_passe_tape`).
    ///
    /// LA SEANCE EN TROISIEME (lot J1) : si ni la cle du poste ni un mot de passe donne ou tape n'ouvrent, le mot de passe
    /// GARDE par `garder_ouverture` (12 h) est essaye. Il ouvre le coffre ; il n'AUTORISE aucun acte reel (cela, seule
    /// `demande::autoriser_acte` le fait).
    fn ouvrir(&self, contenu: &Map<String, Value>, motdepasse: Option<&str>) -> R<Option<Vec<u8>>> {
        let tape = mot_de_passe_tape();
        let donne = motdepasse.filter(|m| !m.is_empty()).or(tape.as_deref());
        if let Some(m) = self.ouvrir_avec(contenu, donne)? {
            return Ok(Some(m));
        }
        if donne.is_some() {
            // Un mot de passe donne qui n'ouvre pas : la seance ne le repare pas, et ne le contredit pas.
            return Ok(None);
        }
        match self.ouverture_gardee()? {
            Some(garde) => self.ouvrir_avec(contenu, Some(&garde)),
            None => Ok(None),
        }
    }

    fn ouvrir_avec(&self, contenu: &Map<String, Value>, motdepasse: Option<&str>) -> R<Option<Vec<u8>>> {
        if contenu.is_empty() {
            return Ok(None);
        }
        let enveloppes = objet(contenu.get("enveloppes"))?;
        let cle = self.cle_poste(false)?;
        let enveloppe = |nom: &str| enveloppes.get(nom).filter(|v| vrai(v));
        if let (Some(cle), Some(e)) = (&cle, enveloppe("poste")) {
            match desceller(cle, &d64(e)?, AAD_POSTE) {
                Ok(m) => return Ok(Some(m)),
                Err(x) if x.classe == "ErreurCoffre" => {}
                Err(x) => return Err(x),
            }
        }
        if let (Some(mdp), Some(e)) = (motdepasse.filter(|m| !m.is_empty()), enveloppe("motdepasse")) {
            let r = deriver(mdp, contenu.get("kdf")).and_then(|k| desceller(&k, &d64(e)?, AAD_MOTDEPASSE));
            return match r {
                Ok(m) => Ok(Some(m)),
                Err(x) if x.classe == "ErreurCoffre" => Ok(None),
                Err(x) => Err(x),
            };
        }
        Ok(None)
    }

    fn ouvrir_ou_lever(&self, motdepasse: Option<&str>) -> R<(Vec<u8>, Map<String, Value>)> {
        let contenu = self.lire_fichier()?;
        if contenu.is_empty() {
            return self.creer();
        }
        match self.ouvrir(&contenu, motdepasse)? {
            Some(m) => Ok((m, contenu)),
            None => Err(self.refus_ferme()?),
        }
    }

    /// Le refus d'un coffre que rien n'ouvre ici.
    fn refus_ferme(&self) -> R<Echec> {
        Ok(refus(format!(
            "Ce coffre ne s'ouvre pas ici : la cle de ce poste ({}) n'est pas \
             celle qui l'a ferme{}. Le mot de passe du logiciel l'ouvre -- et \
             reposera une cle pour ce poste :\n\
             \x20 paie coffre ouverture",
            affiche(&self.chemin_cle()),
            if self.cle_posee()? { "" } else { ", et elle est absente" }
        )))
    }

    /// La cle de CE poste ouvre-t-elle le coffre qui est la ? Vrai aussi sans
    /// coffre : il n'y a rien qui resiste.
    pub fn ouvre_a_ce_poste(&self) -> R<bool> {
        let contenu = self.lire_fichier()?;
        if contenu.is_empty() {
            return Ok(true);
        }
        Ok(self.ouvrir_avec(&contenu, None)?.is_some())
    }

    fn secrets(&self, contenu: &Map<String, Value>, maitresse: &[u8]) -> R<Map<String, Value>> {
        if !contenu.get("secrets").is_some_and(vrai) {
            return Ok(Map::new());
        }
        let clair = desceller(maitresse, &d64(&contenu["secrets"])?, &aad_fiches(&fiches_brutes(contenu)))?;
        let t = String::from_utf8(clair).map_err(|_| echec("ValueError", "secrets illisibles"))?;
        match serde_json::from_str::<Value>(&t) {
            Ok(Value::Object(o)) => Ok(o),
            _ => Err(echec("ValueError", "secrets illisibles")),
        }
    }

    fn poste_ouvre(&self, enveloppes: &Map<String, Value>, maitresse: &[u8]) -> R<bool> {
        let cle = self.cle_poste(false)?;
        let (Some(cle), Some(e)) = (cle, enveloppes.get("poste").filter(|v| vrai(v))) else { return Ok(false) };
        match desceller(&cle, &d64(e)?, AAD_POSTE) {
            Ok(m) => Ok(m == maitresse),
            Err(x) if x.classe == "ErreurCoffre" => Ok(false),
            Err(x) => Err(x),
        }
    }

    /// Reecrit le coffre : les fiches en clair, les secrets scelles avec elles.
    fn enregistrer(
        &self,
        contenu: &Map<String, Value>,
        maitresse: &[u8],
        fiches: Map<String, Value>,
        secrets: &Map<String, Value>,
    ) -> R<PathBuf> {
        let octets = self.octets_a_ecrire(contenu, maitresse, fiches, secrets)?;
        self.ecrire_octets(&octets)
    }

    /// Ecrit `octets` a la place du coffre (0600).
    fn ecrire_octets(&self, octets: &[u8]) -> R<PathBuf> {
        let p = self.chemin();
        dossier_de(&p)?;
        std::fs::write(&p, octets).map_err(|e| oserr(&e, &p))?;
        permissions(&p, 0o600);
        Ok(p)
    }

    /// Ce que `enregistrer` ecrirait, sans l'ecrire.
    fn octets_a_ecrire(
        &self,
        contenu: &Map<String, Value>,
        maitresse: &[u8],
        fiches: Map<String, Value>,
        secrets: &Map<String, Value>,
    ) -> R<Vec<u8>> {
        let mut c = contenu.clone();
        c.insert("version".into(), VERSION.into());
        let clair = dumps(&Value::Object(secrets.clone()), None, false, true);
        c.insert("secrets".into(), b64(&sceller(maitresse, clair.as_bytes(), &aad_fiches(&Value::Object(fiches.clone())))?));
        c.insert("fiches".into(), Value::Object(fiches));
        // L'enveloppe du poste est refaite si elle manque ou ne vaut plus ici :
        // une sauvegarde restauree, ouverte par le mot de passe, rend son
        // autonomie a la machine.
        let mut enveloppes = objet(c.get("enveloppes"))?;
        if !self.poste_ouvre(&enveloppes, maitresse)? {
            enveloppes.insert("poste".into(), b64(&sceller(&self.cle_ou_creer()?, maitresse, AAD_POSTE)?));
        }
        c.insert("enveloppes".into(), Value::Object(enveloppes));
        Ok(dumps(&Value::Object(c), Some(2), false, true).into_bytes())
    }

    // ------------------------------------------------------------- l'usage

    pub fn poser(&self, service: &str, motdepasse: &str, compte: Option<&str>) -> R<PathBuf> {
        let fiche = service_connu(&Value::String(service.into()))?;
        if strip(motdepasse).is_empty() {
            return Err(refus("Mot de passe vide : rien n'a ete pose."));
        }
        let longueur = motdepasse.chars().count();
        if longueur < LONGUEUR_MINIMALE {
            return Err(refus(format!(
                "Mot de passe de {} caractere(s) : c'est trop court pour un compte \
                 d'organisme, et c'est presque toujours le signe que la saisie \
                 masquee n'a pas tout capture. Rien n'a ete pose.\n\
                 Reessayer dans une fenetre, qui ne depend pas du terminal :\n\
                 \x20 paie coffre poser {} --fenetre",
                longueur, service
            )));
        }
        let compte = strip(compte.unwrap_or("")).to_lowercase();
        if fiche.avec_compte && (!compte.contains('@') || compte.contains(' ')) {
            return Err(refus(format!(
                "Ce service se pose avec l'adresse de la boite (compte@domaine) : \
                 {}. Rien n'a ete pose.",
                if compte.is_empty() { "elle manque".to_string() } else { format!("{} n'en est pas une", repr_chaine(&compte)) }
            )));
        }
        let (maitresse, contenu) = self.ouvrir_ou_lever(None)?;
        let mut secrets = self.secrets(&contenu, &maitresse)?;
        let mut fiches = objet(contenu.get("fiches"))?;
        secrets.insert(service.into(), motdepasse.into());
        let mut f = Map::new();
        f.insert("pose_le".into(), self.pose_le()?.into());
        f.insert("libelle".into(), fiche.libelle.into());
        if !compte.is_empty() {
            f.insert("compte".into(), compte.into());
        }
        fiches.insert(service.into(), Value::Object(f));
        self.enregistrer(&contenu, &maitresse, fiches, &secrets)
    }

    /// `AAAA-MM-JJ HH:MM`.
    fn pose_le(&self) -> R<String> {
        if self.maintenant.len() != 16 {
            return Err(echec("ValueError", "demande illisible : il faut `maintenant` (AAAA-MM-JJTHH:MM)"));
        }
        Ok(self.maintenant.replacen('T', " ", 1))
    }

    pub fn oublier(&self, service: &str) -> R<bool> {
        service_connu(&Value::String(service.into()))?;
        let contenu = self.lire_fichier()?;
        if !dans(&fiches_brutes(&contenu), service) {
            return Ok(false);
        }
        let (maitresse, contenu) = self.ouvrir_ou_lever(None)?;
        let mut secrets = self.secrets(&contenu, &maitresse)?;
        let mut fiches = objet(contenu.get("fiches"))?;
        fiches.shift_remove(service);
        secrets.shift_remove(service);
        if fiches.is_empty() && !objet(contenu.get("enveloppes"))?.get("motdepasse").is_some_and(vrai) && !self.scelle_ailleurs() {
            effacer(&self.chemin())?;
            return Ok(true);
        }
        self.enregistrer(&contenu, &maitresse, fiches, &secrets)?;
        Ok(true)
    }

    /// Les fiches EN CLAIR. N'ouvre rien, ne derive rien, ne refuse pas.
    fn fiches(&self) -> R<Map<String, Value>> {
        match self.lire_fichier() {
            Ok(c) => objet(c.get("fiches")),
            Err(x) if x.classe == "ErreurCoffre" => Ok(Map::new()),
            Err(x) => Err(x),
        }
    }

    pub fn pose(&self, service: &str) -> R<bool> {
        match self.lire_fichier() {
            Ok(c) => Ok(dans(&fiches_brutes(&c), service)),
            Err(x) if x.classe == "ErreurCoffre" => Ok(false),
            Err(x) => Err(x),
        }
    }

    /// Ce qui est pose, depuis quand : jamais une valeur.
    pub fn etat(&self) -> R<Vec<Value>> {
        let fiches = self.fiches()?;
        let mut liste = Vec::new();
        for s in SERVICES.iter() {
            let f = objet(fiches.get(s.nom))?;
            let texte = |k: &str| f.get(k).filter(|v| vrai(v)).cloned().unwrap_or_else(|| "".into());
            let mut m = Map::new();
            m.insert("service".into(), s.nom.into());
            m.insert("libelle".into(), s.libelle.into());
            m.insert("sert_a".into(), s.sert_a.into());
            m.insert("pose".into(), (!f.is_empty()).into());
            m.insert("pose_le".into(), texte("pose_le"));
            m.insert("variable".into(), s.variable.into());
            m.insert("dans_environnement".into(), self.variables.contains_key(s.variable).into());
            m.insert("compte".into(), texte("compte"));
            m.insert("avec_compte".into(), s.avec_compte.into());
            liste.push(Value::Object(m));
        }
        Ok(liste)
    }

    /// L'adresse posee avec le secret, ou "".
    pub fn compte(&self, service: &str) -> R<String> {
        service_connu(&Value::String(service.into()))?;
        let f = objet(self.fiches()?.get(service))?;
        Ok(f.get("compte").and_then(Value::as_str).unwrap_or("").to_string())
    }

    /// Le mot de passe du service, ou None. L'environnement l'emporte sur le
    /// coffre. `ouverture` : le mot de passe du logiciel, quand l'appelant l'a
    /// fait taper ; ce module ne le demande jamais.
    pub fn lire(&self, service: &Value, usage: &Value, confirme: bool, ouverture: Option<&str>) -> R<Option<String>> {
        let fiche = service_connu(service)?;
        let nom = fiche.nom;
        let usage_ok = matches!(usage.as_str(), Some(CONSULTATION | DEPOT));
        if !usage_ok {
            return Err(refus(format!(
                "usage doit valoir {} ou {} (recu {}). Lire un secret sans dire \
                 pourquoi n'est pas permis.",
                repr_chaine(CONSULTATION),
                repr_chaine(DEPOT),
                repr_valeur(usage)
            )));
        }
        if usage.as_str() == Some(DEPOT) && !confirme {
            return Err(refus(
                "Un depot ne puise dans le coffre qu'apres confirmation d'un \
                 humain. `confirme` doit porter ce qui a ete confirme -- le mois \
                 recopie, ou la reponse de la fenetre. Voir le docstring de \
                 moteur/coffre.py.",
            ));
        }
        if let Some(v) = self.variables.get(fiche.variable) {
            return Ok(Some(v.clone()));
        }
        let contenu = self.lire_fichier()?;
        if !dans(&fiches_brutes(&contenu), nom) {
            return Ok(None);
        }
        let Some(maitresse) = self.ouvrir(&contenu, ouverture)? else {
            return Err(refus(format!(
                "Le secret {} est dans le coffre, mais rien ici ne l'ouvre : la \
                 cle de ce poste ({}) n'est pas celle qui l'a ferme{}, et le mot \
                 de passe du logiciel {}.\n\
                 Le donner ouvre le coffre sans rien changer au poste ; le \
                 reposer rend a cette machine son ouverture sans mot de passe :\n\
                 \x20 paie coffre ouverture",
                nom,
                affiche(&self.chemin_cle()),
                if self.cle_posee()? { "" } else { ", et elle est absente" },
                if ouverture.is_some_and(|o| !o.is_empty()) || mot_de_passe_tape().is_some() { "ne l'ouvre pas non plus" } else { "n'a pas ete donne" }
            )));
        };
        let secrets = self.secrets(&contenu, &maitresse)?;
        Ok(secrets.get(nom).and_then(Value::as_str).filter(|s| !s.is_empty()).map(String::from))
    }

    pub fn disponible(&self, service: &str) -> R<bool> {
        let fiche = service_connu(&Value::String(service.into()))?;
        Ok(self.variables.contains_key(fiche.variable) || self.pose(service)?)
    }

    // ------------------------------------------- le mot de passe du logiciel

    /// Pose (ou change) le mot de passe du logiciel. `ancien` sert quand la cle
    /// du poste n'ouvre pas le coffre.
    pub fn poser_ouverture(&self, motdepasse: &str, ancien: Option<&str>) -> R<PathBuf> {
        let octets = self.preparer_ouverture(motdepasse, ancien)?;
        self.ecrire_octets(&octets)
    }

    /// Ce que `poser_ouverture` ecrirait, SANS L'ECRIRE : les octets du coffre rescelle par `motdepasse`. Sert a qui doit
    /// resceller plusieurs coffres d'un coup (l'administrateur, `administrateur.rs`) : tout se prepare, puis tout se remplace.
    pub fn preparer_ouverture(&self, motdepasse: &str, ancien: Option<&str>) -> R<Vec<u8>> {
        if strip(motdepasse).chars().count() < LONGUEUR_MINIMALE_LOGICIEL {
            return Err(refus(format!(
                "Mot de passe du logiciel trop court : {} caracteres au moins. \
                 Il n'est plus seulement un oui ou non depuis le 20/09/2026 -- il \
                 ouvre le coffre sur une machine qui n'a pas sa cle, donc qui \
                 vole le fichier peut l'essayer hors ligne. Rien n'a ete pose.",
                LONGUEUR_MINIMALE_LOGICIEL
            )));
        }
        let (maitresse, contenu) = self.ouvrir_ou_lever(ancien)?;
        let mut enveloppes = objet(contenu.get("enveloppes"))?;
        let kdf = contenu.get("kdf").ok_or_else(|| echec("KeyError", crate::fichiers::absent("kdf")))?;
        enveloppes.insert("motdepasse".into(), b64(&sceller(&deriver(motdepasse, Some(kdf))?, &maitresse, AAD_MOTDEPASSE)?));
        let mut c = contenu;
        c.insert("enveloppes".into(), Value::Object(enveloppes));
        c.insert("logiciel_pose_le".into(), self.pose_le()?.into());
        let secrets = self.secrets(&c, &maitresse)?;
        let fiches = objet(c.get("fiches"))?;
        self.octets_a_ecrire(&c, &maitresse, fiches, &secrets)
    }

    /// Un mot de passe du logiciel a-t-il ete pose ?
    pub fn ouverture_exigee(&self) -> R<bool> {
        match self.lire_fichier() {
            Ok(c) => Ok(objet(c.get("enveloppes"))?.get("motdepasse").is_some_and(vrai)),
            Err(x) if x.classe == "ErreurCoffre" => Ok(false),
            Err(x) => Err(x),
        }
    }

    /// Retire l'enveloppe du mot de passe du logiciel.
    pub fn oublier_ouverture(&self, ancien: Option<&str>) -> R<bool> {
        let contenu = self.lire_fichier()?;
        if !objet(contenu.get("enveloppes"))?.get("motdepasse").is_some_and(vrai) {
            return Ok(false);
        }
        let (maitresse, contenu) = self.ouvrir_ou_lever(ancien)?;
        let mut enveloppes = objet(contenu.get("enveloppes"))?;
        enveloppes.shift_remove("motdepasse");
        let fiches = objet(contenu.get("fiches"))?;
        if fiches.is_empty() && !self.scelle_ailleurs() {
            effacer(&self.chemin())?;
            return Ok(true);
        }
        let secrets = self.secrets(&contenu, &maitresse)?;
        let mut c = contenu;
        c.insert("enveloppes".into(), Value::Object(enveloppes));
        c.shift_remove("logiciel_pose_le");
        self.enregistrer(&c, &maitresse, fiches, &secrets)?;
        Ok(true)
    }

    /// Le mot de passe du logiciel est-il le bon ? L'enveloppe le dit en
    /// s'ouvrant : GCM ne connait pas le « presque ».
    pub fn verifier_ouverture(&self, motdepasse: &str) -> R<bool> {
        let contenu = match self.lire_fichier() {
            Ok(c) => c,
            Err(x) if x.classe == "ErreurCoffre" => return Ok(false),
            Err(x) => return Err(x),
        };
        let enveloppes = objet(contenu.get("enveloppes"))?;
        let Some(e) = enveloppes.get("motdepasse").filter(|v| vrai(v)) else { return Ok(false) };
        let r = deriver(motdepasse, contenu.get("kdf")).and_then(|k| desceller(&k, &d64(e)?, AAD_MOTDEPASSE));
        match r {
            Ok(_) => Ok(true),
            Err(x) if x.classe == "ErreurCoffre" => Ok(false),
            Err(x) => Err(x),
        }
    }

    // -------------------------------------------- l'ouverture gardee

    /// Garde le mot de passe du logiciel jusqu'a `fin` (`AAAA-MM-JJTHH:MM`).
    /// Refuse un mot de passe qui n'ouvre pas le coffre.
    pub fn garder_ouverture(&self, motdepasse: &str, fin: &str) -> R<String> {
        if !self.verifier_ouverture(motdepasse)? {
            return Err(refus("Mot de passe du logiciel refuse : rien n'est garde."));
        }
        let mut d = Map::new();
        d.insert("motdepasse".into(), motdepasse.into());
        d.insert("fin".into(), fin.into());
        d.insert("coffre".into(), affiche(&resoudre(&self.chemin())).into());
        let clair = dumps(&Value::Object(d), None, false, false);
        let p = self.chemin_gardee();
        dossier_de(&p)?;
        std::fs::write(&p, sceller(&self.cle_ou_creer()?, clair.as_bytes(), AAD_GARDEE)?).map_err(|e| oserr(&e, &p))?;
        Ok(fin.to_string())
    }

    /// La gardee dechiffree, si elle se lit.
    fn gardee(&self) -> R<Option<Map<String, Value>>> {
        let p = self.chemin_gardee();
        let cle = self.cle_poste(false)?;
        let Some(cle) = cle.filter(|_| existe(&p).unwrap_or(false)) else { return Ok(None) };
        let lu = lire_octets(&p).and_then(|o| desceller(&cle, &o, AAD_GARDEE)).map(|c| {
            String::from_utf8(c).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok())
        });
        match lu {
            Ok(Some(Value::Object(d))) => Ok(Some(d)),
            Ok(_) => Err(echec("Illisible", "")),
            Err(x) if x.classe == "ErreurCoffre" => Err(echec("Illisible", "")),
            Err(x) => Err(x),
        }
    }

    /// Le mot de passe garde, s'il y en a un, pas expire, pour CE coffre, et
    /// qu'il l'ouvre encore. Une gardee expiree ou perimee s'efface.
    pub fn ouverture_gardee(&self) -> R<Option<String>> {
        let d = match self.gardee() {
            Ok(Some(d)) => d,
            Ok(None) => return Ok(None),
            Err(x) if x.classe == "Illisible" => {
                self.oublier_ouverture_gardee()?;
                return Ok(None);
            }
            Err(x) => return Err(x),
        };
        let texte = |k: &str| d.get(k).and_then(Value::as_str).map(String::from);
        if texte("fin").unwrap_or_default() < self.heure() {
            self.oublier_ouverture_gardee()?;
            return Ok(None);
        }
        if texte("coffre").as_deref() != Some(&affiche(&resoudre(&self.chemin()))) {
            return Ok(None);
        }
        let mdp = texte("motdepasse").unwrap_or_default();
        if !self.verifier_ouverture(&mdp)? {
            self.oublier_ouverture_gardee()?;
            return Ok(None);
        }
        Ok(Some(mdp))
    }

    /// L'heure de fin de la gardee en cours, sans rien verifier d'autre.
    pub fn fin_ouverture_gardee(&self) -> R<Option<String>> {
        let d = match self.gardee() {
            Ok(Some(d)) => d,
            Ok(None) => return Ok(None),
            Err(x) if x.classe == "Illisible" => return Ok(None),
            Err(x) => return Err(x),
        };
        Ok(d.get("fin").and_then(Value::as_str).filter(|f| *f >= self.heure().as_str()).map(String::from))
    }

    pub fn oublier_ouverture_gardee(&self) -> R<bool> {
        let p = self.chemin_gardee();
        if existe(&p)? {
            std::fs::remove_file(&p).map_err(|e| oserr(&e, &p))?;
            return Ok(true);
        }
        Ok(false)
    }
}

pub(crate) fn service_connu(service: &Value) -> R<&'static Service> {
    if let Some(s) = service.as_str().and_then(|n| SERVICES.iter().find(|s| s.nom == n)) {
        return Ok(s);
    }
    let mut noms: Vec<&str> = SERVICES.iter().map(|s| s.nom).collect();
    noms.sort_unstable();
    Err(refus(format!("Service inconnu : {}. Connus : {}.", repr_valeur(service), noms.join(", "))))
}

#[cfg(test)]
mod tests_dossier_cle {
    use super::dossier_cle_de;
    use std::path::{Path, PathBuf};

    fn env_de<'a>(valeurs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |n| valeurs.iter().find(|(k, _)| *k == n).map(|(_, v)| v.to_string())
    }

    /// Sur une machine : `%APPDATA%\PuppetPaie` ou `$XDG_CONFIG_HOME/puppetpaie` (un profil inventé : le verrou
    /// des données réelles refuse un chemin sous le dossier des utilisateurs de Windows).
    fn de_la_machine() -> PathBuf {
        if cfg!(windows) {
            Path::new("D:/Profils/x/AppData/Roaming").join("PuppetPaie")
        } else {
            Path::new("/home/x/.config").join("puppetpaie")
        }
    }

    const MACHINE: [(&str, &str); 3] =
        [("HOME", "/home/x"), ("XDG_CONFIG_HOME", "/home/x/.config"), ("APPDATA", "D:/Profils/x/AppData/Roaming")];

    #[test]
    fn sans_dossier_portable_la_cle_vit_sur_la_machine() {
        assert_eq!(dossier_cle_de(&env_de(&MACHINE), None), de_la_machine());
    }

    #[test]
    fn en_portable_la_cle_voyage_avec_le_dossier() {
        let cle_usb = Path::new("/media/cle-usb/PuppetPaie");
        assert_eq!(dossier_cle_de(&env_de(&MACHINE), Some(cle_usb)), cle_usb);
    }

    #[test]
    fn puppetpaie_cle_passe_devant_le_dossier_portable() {
        let force = [("PUPPETPAIE_CLE", "/tmp/cle-forcee"), ("HOME", "/home/x")];
        assert_eq!(dossier_cle_de(&env_de(&force), Some(Path::new("/media/cle-usb/PuppetPaie"))), Path::new("/tmp/cle-forcee"));
    }
}
