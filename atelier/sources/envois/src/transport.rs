//! Le transport vers net-entreprises et l'Urssaf : le client HTTP, l'authentification, les consultations, l'archivage
//! des retours et leur journal, et l'envoi du corps d'un depot (DSN, DPAE).
//!
//! CE MODULE NE CHERCHE JAMAIS UN SECRET : le mot de passe arrive de l'appelant, sert le temps de l'authentification
//! et n'est ecrit nulle part ; il n'ouvre pas le coffre et n'en parle pas. Le jeton, lui, vaut une session : c'est
//! l'appelant qui le garde en memoire et le repasse.
//!
//! Les pannes : `ErreurApi` (message et `code`) ; un delai de lecture, une reponse coupee, une connexion rompue, un gzip
//! illisible portent chacun leur `classe`, que l'appelant releve.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::{json, Map, Value};
use ureq::http;

use socle::base64::{ecrire as base64_ecrire, lire as base64_lire};

pub const USER_AGENT: &str = "Client-DSN (PuppetPaie/1.0; PuppetPaie)";

/// Les messages de l'API par statut HTTP (guide, section 8).
pub const MESSAGES: [(u16, &str); 7] = [
    (401, "Authentification refusee, ou declarant non inscrit au service DSN."),
    (
        403,
        "Compte temporairement bloque (trop de tentatives), ou tentative d'acces a des donnees deposees par quelqu'un d'autre.",
    ),
    (404, "Service ou identifiant inconnu. Peut aussi signaler une interruption de service."),
    (406, "Format de reponse demande non supporte."),
    (422, "Parametres invalides."),
    (
        429,
        "Contraintes de consultation non respectees : trop d'appels, ou plage de recherche trop large. Attendre et relancer.",
    ),
    (503, "Service indisponible."),
];

const DELAI_APPEL: u64 = 60;
const DELAI_DEPOT: u64 = 120;
const MAX_REDIRECTIONS: usize = 10;

pub fn message_du_statut(code: u16) -> &'static str {
    MESSAGES.iter().find(|(c, _)| *c == code).map(|(_, m)| *m).unwrap_or("")
}

// ----------------------------------------------------------------- les pannes

/// Ce qui peut mal tourner, en trois formes.
#[derive(Debug)]
pub enum Panne {
    /// Un statut HTTP hors 2xx ; le corps, lu en entier (ou la panne de sa lecture).
    Statut { code: u16, corps: Result<Vec<u8>, Box<Panne>> },
    /// Ce qu'urllib rangeait sous `URLError` : la raison.
    Reseau(String),
    /// Une exception brute d'urllib / http.client / gzip, sous son nom.
    Brute { classe: &'static str, message: String, plus: Map<String, Value> },
}

impl Panne {
    fn brute(classe: &'static str, message: impl Into<String>) -> Panne {
        Panne::Brute { classe, message: message.into(), plus: Map::new() }
    }
}

/// Une panne mise en reponse JSON : `ok: false`, `erreur`, `classe`, `code`.
fn echec(classe: &str, message: String, code: Option<i64>) -> Value {
    let mut m = Map::new();
    m.insert("ok".into(), Value::Bool(false));
    m.insert("erreur".into(), Value::String(message));
    m.insert("classe".into(), Value::String(classe.into()));
    if let Some(c) = code {
        m.insert("code".into(), json!(c));
    }
    Value::Object(m)
}

fn echec_brut(p: Panne) -> Value {
    match p {
        Panne::Brute { classe, message, plus } => {
            let mut v = echec(classe, message, None);
            v.as_object_mut().unwrap().extend(plus);
            v
        }
        // Les appelants traduisent les deux autres avant d'arriver ici.
        Panne::Reseau(r) => echec("URLError", r, None),
        Panne::Statut { code, .. } => echec("HTTPError", format!("HTTP {}", code), Some(code as i64)),
    }
}

// ------------------------------------------------------- les petits outils

use socle::texte::blanc as blanc;

fn py_strip(s: &str) -> &str {
    s.trim_matches(blanc)
}

fn py_split(s: &str) -> Vec<&str> {
    s.split(blanc).filter(|m| !m.is_empty()).collect()
}

/// `s[:n]` sur des points de code.
fn debut(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// `bytes.decode("utf-8", "replace")`.
fn utf8_remplace(o: &[u8]) -> String {
    String::from_utf8_lossy(o).into_owned()
}

/// `_sans_secret` : jamais un identifiant dans un message.
pub fn sans_secret(url: &str) -> &str {
    url.split('?').next().unwrap_or("")
}

/// `_echapper`.
pub fn echapper(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// `_horodatage` : une chaine passe telle quelle ; sinon `AAAAMMJJhhmmss`
/// depuis un instant ISO (`AAAA-MM-JJ[Thh:mm:ss...]`).
pub fn horodatage(x: &Value) -> Result<String, String> {
    if let Some(s) = x.as_str() {
        return Ok(s.to_string());
    }
    let iso = x.get("iso").and_then(Value::as_str).ok_or("horodatage : une chaine ou {\"iso\": ...}")?;
    let c: Vec<char> = iso.chars().collect();
    let chiffres = |a: usize, b: usize| -> Option<String> {
        let t: String = c.get(a..b)?.iter().collect();
        t.chars().all(|ch| ch.is_ascii_digit()).then_some(t)
    };
    let jour = (|| Some(format!("{}{}{}", chiffres(0, 4)?, chiffres(5, 7)?, chiffres(8, 10)?)))()
        .ok_or_else(|| format!("horodatage illisible : {}", iso))?;
    let heure = if c.len() >= 19 {
        (|| Some(format!("{}{}{}", chiffres(11, 13)?, chiffres(14, 16)?, chiffres(17, 19)?)))()
            .ok_or_else(|| format!("horodatage illisible : {}", iso))?
    } else {
        "000000".to_string()
    };
    Ok(jour + &heure)
}

/// `str(x)` d'une valeur JSON, comme `"%s" % x`.
fn en_texte(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        autre => autre.to_string(),
    }
}

/// Vrai pour une valeur presente et non vide (ni `null`, `false`, `0`, `""`, `[]`, `{}`).
fn vrai(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) | Some(Value::Bool(false)) => false,
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
        Some(Value::Number(n)) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Some(Value::Bool(true)) => true,
    }
}

// ------------------------------------------------------------------- gzip

/// Decompresse un corps gzip ; les refus (tronque, en-tete invalide, somme de controle) sont dits en francais.
pub fn gunzip(donnees: &[u8]) -> Result<Vec<u8>, Panne> {
    let eof = || Panne::brute("EOFError", "réponse gzip tronquée");
    let mut sortie = Vec::new();
    let mut reste = donnees;
    loop {
        if reste.is_empty() {
            return Ok(sortie);
        }
        if reste.len() < 2 || reste[0] != 0x1f || reste[1] != 0x8b {
            let debut: Vec<String> = reste.iter().take(2).map(|o| format!("{:02x}", o)).collect();
            return Err(Panne::brute("BadGzipFile", format!("réponse attendue en gzip, elle commence par {}", debut.join(" "))));
        }
        let mut pos = 2;
        let lire = |pos: &mut usize, n: usize| -> Result<(), Panne> {
            if reste.len() < *pos + n {
                return Err(eof());
            }
            *pos += n;
            Ok(())
        };
        if reste.len() < 10 {
            return Err(eof());
        }
        let (methode, drapeaux) = (reste[2], reste[3]);
        pos += 8;
        if methode != 8 {
            return Err(Panne::brute("BadGzipFile", "réponse gzip : méthode de compression inconnue"));
        }
        if drapeaux & 4 != 0 {
            if reste.len() < pos + 2 {
                return Err(eof());
            }
            let n = u16::from_le_bytes([reste[pos], reste[pos + 1]]) as usize;
            pos += 2;
            lire(&mut pos, n)?;
        }
        for drapeau in [8u8, 16u8] {
            if drapeaux & drapeau != 0 {
                loop {
                    match reste.get(pos) {
                        None => break,
                        Some(0) => {
                            pos += 1;
                            break;
                        }
                        Some(_) => pos += 1,
                    }
                }
            }
        }
        if drapeaux & 2 != 0 {
            lire(&mut pos, 2)?;
        }
        let mut d = flate2::Decompress::new(false);
        let mut membre = Vec::new();
        let en_entree = &reste[pos..];
        loop {
            let avant_in = d.total_in() as usize;
            let avant_out = d.total_out();
            membre.reserve(32 * 1024);
            let st = d
                .decompress_vec(&en_entree[avant_in..], &mut membre, flate2::FlushDecompress::None)
                .map_err(|e| Panne::brute("error", format!("réponse gzip illisible : {}", e)))?;
            if st == flate2::Status::StreamEnd {
                break;
            }
            if d.total_in() as usize == avant_in && d.total_out() == avant_out {
                return Err(eof());
            }
        }
        let utilises = d.total_in() as usize;
        let apres = &en_entree[utilises..];
        if apres.len() < 8 {
            return Err(eof());
        }
        let crc = u32::from_le_bytes([apres[0], apres[1], apres[2], apres[3]]);
        let longueur = u32::from_le_bytes([apres[4], apres[5], apres[6], apres[7]]);
        let mut h = flate2::Crc::new();
        h.update(&membre);
        if crc != h.sum() {
            return Err(Panne::brute("BadGzipFile", "réponse gzip : somme de contrôle fausse"));
        }
        if longueur != (membre.len() as u64 & 0xffff_ffff) as u32 {
            return Err(Panne::brute("BadGzipFile", "réponse gzip : longueur fausse"));
        }
        sortie.extend_from_slice(&membre);
        let mut suite = &apres[8..];
        while let Some((0, r)) = suite.split_first().map(|(a, r)| (*a, r)) {
            suite = r;
        }
        reste = suite;
    }
}

// ------------------------------------------------------------------- HTTP

type Entetes = Vec<(String, String)>;

/// `Content-Type` -> `Content-Type`, comme un serveur les ecrit.
fn nom_canonique(n: &str) -> String {
    n.split('-')
        .map(|m| {
            let mut c = m.chars();
            match c.next() {
                Some(p) => p.to_ascii_uppercase().to_string() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join("-")
}

fn errno_texte(e: &std::io::Error) -> String {
    match e.raw_os_error() {
        Some(n) => {
            let t = e.to_string();
            let t = t.strip_suffix(&format!(" (os error {})", n)).unwrap_or(&t).to_string();
            format!("[Errno {}] {}", n, t)
        }
        None => e.to_string(),
    }
}

fn delai_expire(https: bool) -> &'static str {
    if https {
        "The read operation timed out"
    } else {
        "timed out"
    }
}

/// Une panne de `ureq` dans les mots d'urllib : `URLError` avant la reponse,
/// exception brute apres l'envoi.
fn panne_de(e: ureq::Error, https: bool) -> Panne {
    use std::io::ErrorKind as K;
    use ureq::{Error, Timeout};
    match e {
        Error::Timeout(Timeout::Connect) | Error::Timeout(Timeout::Resolve) => Panne::Reseau("timed out".into()),
        Error::Timeout(_) => Panne::brute("TimeoutError", delai_expire(https)),
        Error::HostNotFound => Panne::Reseau("[Errno -2] Name or service not known".into()),
        Error::Io(io) => match io.kind() {
            K::UnexpectedEof => Panne::brute("RemoteDisconnected", "Remote end closed connection without response"),
            // Observe sur le service reel : la connexion est rompue pendant
            // l'etablissement (mandataire, TLS), ce qu'urllib rangeait sous
            // `URLError`. ureq ne dit pas a quel moment : on garde cette lecture.
            K::ConnectionReset | K::ConnectionAborted => Panne::Reseau(errno_texte(&io)),
            K::TimedOut if io.raw_os_error().is_none() => Panne::Reseau("timed out".into()),
            _ => Panne::Reseau(errno_texte(&io)),
        },
        Error::Protocol(p) => Panne::brute("HTTPException", p.to_string()),
        Error::Rustls(r) => Panne::Reseau(format!("[SSL: CERTIFICATE_VERIFY_FAILED] certificate verify failed: {}", r)),
        Error::Tls(t) => Panne::Reseau(format!("[SSL] {}", t)),
        Error::BadUri(u) => Panne::brute("ValueError", format!("unknown url type: {:?}", u)),
        autre => Panne::Reseau(autre.to_string()),
    }
}

/// Le mandataire de l'environnement, comme urllib le lit : `https_proxy` pour
/// une adresse https, `http_proxy` pour une adresse http, et `no_proxy`
/// (noms, suffixes de domaine, `*`) qui l'ecarte. La machine elle-meme (les faux
/// serveurs de l'atelier et des essais) ne passe jamais par un mandataire.
fn mandataire(url: &str) -> Option<ureq::Proxy> {
    let (schema, reste) = url.split_once("://")?;
    let var = |n: &str| std::env::var(n.to_lowercase()).ok().or_else(|| std::env::var(n.to_uppercase()).ok()).filter(|v| !v.is_empty());
    let valeur = var(&format!("{}_proxy", schema.to_lowercase()))?;
    let autorite = reste.split(['/', '?', '#']).next().unwrap_or("");
    let hote = autorite.rsplit_once('@').map(|(_, h)| h).unwrap_or(autorite);
    let nom = if hote.starts_with('[') { hote.split(']').next().unwrap_or(hote).trim_start_matches('[') } else { hote.split(':').next().unwrap_or(hote) };
    if matches!(nom.to_lowercase().as_str(), "127.0.0.1" | "localhost" | "::1") {
        return None;
    }
    if let Some(np) = var("no_proxy") {
        for e in np.split(',') {
            let e = py_strip(e).trim_start_matches('.').to_lowercase();
            if e == "*" {
                return None;
            }
            if e.is_empty() {
                continue;
            }
            let (h, a) = (nom.to_lowercase(), hote.to_lowercase());
            let correspond = |x: &str| x == e || x.ends_with(&format!(".{}", e));
            if correspond(&h) || correspond(&a) {
                return None;
            }
        }
    }
    ureq::Proxy::new(&valeur).ok()
}

/// L'agent d'une demande. En essai, le garde-fou reseau (`garde_reseau`) le regle : rien ne sort de la machine.
fn agent(delai: u64, url: &str) -> Result<ureq::Agent, Panne> {
    let tls = url.starts_with("https:");
    let d = Some(Duration::from_secs(delai));
    let mut c = ureq::Agent::config_builder()
        .proxy(mandataire(url))
        .http_status_as_error(false)
        .max_redirects(0)
        .max_redirects_will_error(false)
        .user_agent(USER_AGENT)
        .accept("")
        .timeout_resolve(d)
        .timeout_connect(d)
        .timeout_send_request(d)
        .timeout_send_body(d)
        .timeout_recv_response(d);
    if tls {
        c = c.tls_config(ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build());
    }
    let c = socle::garde_reseau::regler(c, url).map_err(Panne::Reseau)?;
    Ok(c.build().into())
}

/// Lit le corps par morceaux, avec le delai d'urllib : un silence de `delai`
/// secondes entre deux morceaux, pas une duree totale.
fn lire_corps(corps: ureq::Body, delai: u64, attendu: Option<u64>, https: bool) -> Result<Vec<u8>, Panne> {
    let (tx, rx) = mpsc::channel::<Result<Vec<u8>, std::io::Error>>();
    std::thread::spawn(move || {
        let mut r = corps.into_reader();
        let mut tampon = vec![0u8; 64 * 1024];
        loop {
            match r.read(&mut tampon) {
                Ok(0) => {
                    let _ = tx.send(Ok(Vec::new()));
                    return;
                }
                Ok(n) => {
                    if tx.send(Ok(tampon[..n].to_vec())).is_err() {
                        return;
                    }
                }
                Err(e) => {
                    let _ = tx.send(Err(e));
                    return;
                }
            }
        }
    });
    let mut tout = Vec::new();
    loop {
        match rx.recv_timeout(Duration::from_secs(delai)) {
            Ok(Ok(m)) if m.is_empty() => return Ok(tout),
            Ok(Ok(m)) => tout.extend_from_slice(&m),
            Ok(Err(e)) => {
                if matches!(e.kind(), std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted) {
                    return Err(Panne::brute("ConnectionResetError", errno_texte(&e)));
                }
                return Err(coupee(tout.len(), attendu));
            }
            Err(_) => return Err(Panne::brute("TimeoutError", delai_expire(https))),
        }
    }
}

/// `http.client.IncompleteRead` : N octets lus, M attendus de plus.
fn coupee(lus: usize, attendu: Option<u64>) -> Panne {
    let mut plus = Map::new();
    plus.insert("lus".into(), json!(lus));
    let message = match attendu {
        Some(a) if a as usize > lus => {
            plus.insert("manque".into(), json!(a as usize - lus));
            format!("IncompleteRead({} bytes read, {} more expected)", lus, a as usize - lus)
        }
        _ => format!("IncompleteRead({} bytes read)", lus),
    };
    Panne::Brute { classe: "IncompleteRead", message, plus }
}

/// `urllib.parse.urljoin` pour un `Location` de redirection.
fn joindre(base: &str, loc: &str) -> String {
    let loc = loc.replace(' ', "%20");
    if let Some(i) = loc.find("://") {
        if loc[..i].chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c)) {
            return loc;
        }
    }
    let (schema, reste) = match base.split_once("://") {
        Some((s, r)) => (s, r),
        None => return loc,
    };
    if let Some(l) = loc.strip_prefix("//") {
        return format!("{}://{}", schema, l);
    }
    let (autorite, chemin) = match reste.find(['/', '?']) {
        Some(i) => (&reste[..i], &reste[i..]),
        None => (reste, ""),
    };
    if loc.starts_with('/') {
        return format!("{}://{}{}", schema, autorite, loc);
    }
    let chemin = chemin.split(['?', '#']).next().unwrap_or("");
    let dossier = match chemin.rfind('/') {
        Some(i) => &chemin[..=i],
        None => "/",
    };
    format!("{}://{}{}{}", schema, autorite, dossier, loc)
}

fn en_tetes_de(parts: &http::response::Parts) -> Entetes {
    let mut v: Entetes = Vec::new();
    for (n, val) in parts.headers.iter() {
        let nom = nom_canonique(n.as_str());
        let val = String::from_utf8_lossy(val.as_bytes()).into_owned();
        // `dict(r.headers)` : le premier l'emporte (`Message.get`).
        if !v.iter().any(|(k, _)| *k == nom) {
            v.push((nom, val));
        }
    }
    v
}

fn entete<'a>(e: &'a Entetes, nom: &str) -> Option<&'a str> {
    e.iter().find(|(k, _)| k.eq_ignore_ascii_case(nom)).map(|(_, v)| v.as_str())
}

/// Ce qu'on envoie : une requete du guide.
#[derive(Default)]
pub struct Requete<'a> {
    pub url: &'a str,
    pub jeton: Option<&'a str>,
    pub donnees: Option<&'a [u8]>,
    pub content_type: Option<&'a str>,
    /// L'en-tete `Content-Encoding: gzip` d'un depot.
    pub corps_gzip: bool,
    /// Un depot part toujours en POST, meme vide.
    pub post: bool,
    pub delai: u64,
    /// Des en-tetes en plus (le canal de l'editeur : `User-Agent`, `X-Filet-Cle`).
    pub entetes: &'a [(&'a str, &'a str)],
    /// Ne pas demander `gzip` (`urllib` ne le demande pas, le canal de l'editeur non plus).
    pub sans_gzip: bool,
    /// Une autre methode que GET ou POST (`PATCH` du Drive) ; elle part avec son corps, meme vide.
    pub methode: Option<&'a str>,
}

/// Le corps de la reponse (decompresse si `Content-Encoding: gzip`) et ses
/// en-tetes ; une panne sinon (`Panne::Statut` pour un statut hors 2xx).
pub fn envoyer(r: &Requete) -> Result<(Vec<u8>, Entetes), Panne> {
    let https = r.url.starts_with("https:");
    let mut url = r.url.to_string();
    let mut donnees = r.donnees;
    let mut methode = r.methode;
    let mut post = r.post || methode.is_some() || donnees.map(|d| !d.is_empty()).unwrap_or(false);
    let mut avec_entetes_du_corps = true;
    let mut sauts = 0;
    let (parts, corps) = loop {
        let mut b = http::Request::builder().method(methode.unwrap_or(if post { "POST" } else { "GET" })).uri(&url);
        if !r.sans_gzip {
            b = b.header("Accept-Encoding", "gzip");
        }
        for (nom, valeur) in r.entetes {
            b = b.header(*nom, *valeur);
        }
        if avec_entetes_du_corps {
            // urllib pose ce type a un POST qui n'en donne pas.
            match r.content_type {
                Some(ct) => b = b.header("Content-Type", ct),
                None if post => b = b.header("Content-Type", "application/x-www-form-urlencoded"),
                None => {}
            }
            if r.corps_gzip {
                b = b.header("Content-Encoding", "gzip");
            }
        }
        // Le jeton ne suit une redirection que vers le meme hote.
        if let Some(j) = r.jeton {
            if sauts == 0 || meme_hote(r.url, &url) {
                b = b.header("Authorization", format!("DSNLogin jeton={}", j));
            }
        }
        let ag = agent(r.delai, &url)?;
        let rep = if post {
            let corps = donnees.unwrap_or(&[]).to_vec();
            ag.run(b.body(corps).map_err(|e| Panne::brute("ValueError", e.to_string()))?)
        } else {
            ag.run(b.body(()).map_err(|e| Panne::brute("ValueError", e.to_string()))?)
        }
        .map_err(|e| panne_de(e, https))?;
        let (parts, corps) = rep.into_parts();
        let code = parts.status.as_u16();
        let suite = matches!(code, 301 | 302 | 303 | 307 | 308) && (!post || code <= 303) && sauts < MAX_REDIRECTIONS;
        let loc = parts.headers.get("location").and_then(|v| v.to_str().ok()).map(str::to_string);
        match (suite, loc) {
            (true, Some(l)) => {
                url = joindre(&url, &l);
                if post {
                    post = false;
                    methode = None;
                    donnees = None;
                    avec_entetes_du_corps = false;
                }
                sauts += 1;
                continue;
            }
            _ => break (parts, corps),
        }
    };
    let code = parts.status.as_u16();
    let entetes = en_tetes_de(&parts);
    let attendu = entete(&entetes, "Content-Length").and_then(|v| v.trim().parse::<u64>().ok());
    let brut = lire_corps(corps, r.delai, attendu, https);
    if !(200..300).contains(&code) {
        return Err(Panne::Statut { code, corps: brut.map_err(Box::new) });
    }
    let mut brut = brut?;
    if entete(&entetes, "Content-Encoding") == Some("gzip") {
        brut = gunzip(&brut)?;
    }
    Ok((brut, entetes))
}

fn meme_hote(a: &str, b: &str) -> bool {
    let hote = |u: &str| u.split_once("://").map(|(_, r)| r.split(['/', '?', '#']).next().unwrap_or("").to_string());
    hote(a) == hote(b)
}

// ------------------------------------------------- les operations, une a une

type Resultat = Result<Value, Value>;

fn texte<'a>(d: &'a Value, cle: &str) -> Result<&'a str, Value> {
    d.get(cle)
        .and_then(Value::as_str)
        .ok_or_else(|| echec("ValueError", format!("demande illisible : il faut `{}`", cle), None))
}

fn octets(d: &Value, cle: &str) -> Result<Option<Vec<u8>>, Value> {
    match d.get(cle) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => base64_lire(s).map(Some).map_err(|e| echec("ValueError", e, None)),
        Some(_) => Err(echec("ValueError", format!("demande illisible : `{}` n'est pas du base64", cle), None)),
    }
}

fn entetes_json(e: &Entetes) -> Value {
    Value::Object(e.iter().map(|(k, v)| (k.clone(), Value::String(v.clone()))).collect())
}

fn reponse(corps: &[u8], e: &Entetes) -> Value {
    json!({"ok": true, "corps": base64_ecrire(corps), "entetes": entetes_json(e)})
}

/// `Client._appel` : la panne en `ErreurApi`.
fn en_erreur_api(url: &str, p: Panne) -> Value {
    match p {
        Panne::Statut { code, corps } => {
            let corps = corps.map(|c| debut(&utf8_remplace(&c), 400)).unwrap_or_default();
            let mut m = format!("HTTP {} sur {}\n  {}", code, sans_secret(url), message_du_statut(code));
            if !py_strip(&corps).is_empty() {
                m.push_str("\n  ");
                m.push_str(&corps);
            }
            echec("ErreurApi", m, Some(code as i64))
        }
        Panne::Reseau(r) => echec("ErreurApi", format!("Reseau injoignable pour {} : {}", sans_secret(url), r), Some(0)),
        brute => echec_brut(brute),
    }
}

/// `Client._appel` : GET, ou POST si des donnees sont fournies.
fn appel(d: &Value) -> Resultat {
    let url = texte(d, "url")?;
    let donnees = octets(d, "donnees")?;
    let r = Requete {
        url,
        jeton: d.get("jeton").and_then(Value::as_str),
        donnees: donnees.as_deref(),
        content_type: d.get("content_type").and_then(Value::as_str),
        corps_gzip: false,
        post: false,
        delai: d.get("delai").and_then(Value::as_u64).unwrap_or(DELAI_APPEL),
        ..Default::default()
    };
    envoyer(&r).map(|(c, e)| reponse(&c, &e)).map_err(|p| en_erreur_api(url, p))
}

/// `Client._consulter` : l'adresse de base et la plage, puis `_appel`.
fn consulter(d: &Value) -> Resultat {
    let mut url = texte(d, "base")?.to_string();
    url.push_str(&horodatage(d.get("debut").ok_or_else(|| echec("ValueError", "il faut `debut`".into(), None))?).map_err(|e| echec("ValueError", e, None))?);
    if let Some(fin) = d.get("fin").filter(|f| !f.is_null()) {
        url.push('/');
        url.push_str(&horodatage(fin).map_err(|e| echec("ValueError", e, None))?);
    }
    let mut d2 = d.clone();
    d2["url"] = Value::String(url);
    d2["donnees"] = Value::Null;
    appel(&d2)
}

/// L'identite du declarant inscrit au service DSN. Une fiche
/// dont `declarant_dsn` n'est pas un objet, ou dont `nom_prenom` n'est pas un
/// texte, leve `AttributeError`.
pub fn declarant(ent: &Value) -> Result<Value, Value> {
    if vrai(ent.get("declarant")) {
        return Ok(ent["declarant"].clone());
    }
    let mut brut = "";
    if let Some(dsn) = ent.get("declarant_dsn").filter(|v| vrai(Some(v))) {
        let o = dsn.as_object().ok_or_else(|| echec("AttributeError", "le declarant_dsn n'est pas un objet".into(), None))?;
        brut = match o.get("nom_prenom") {
            None => "",
            Some(Value::String(t)) => t,
            Some(_) => return Err(echec("AttributeError", "`nom_prenom` n'est pas un texte".into(), None)),
        };
    }
    let morceaux = py_split(py_strip(brut));
    let siret = ent.get("siret").cloned().unwrap_or(Value::Null);
    let mut m = Map::new();
    m.insert("siret".into(), siret);
    if morceaux.len() >= 2 {
        m.insert("nom".into(), Value::String(morceaux[0].into()));
        m.insert("prenom".into(), Value::String(morceaux[1..].join(" ")));
    }
    Ok(Value::Object(m))
}

/// Ce qu'une authentification envoie : le declarant inscrit au service, le mot de passe deja prepare par l'appelant, le
/// service, et le jeton d'une session deja ouverte.
pub struct Authentification<'a> {
    pub url: &'a str,
    pub declarant: &'a Value,
    pub motdepasse: Option<&'a str>,
    /// La variable d'environnement que le refus nomme quand le mot de passe manque.
    pub variable: &'a str,
    pub service: Option<&'a str>,
    pub jeton: Option<&'a str>,
}

fn authentifier_op(d: &Value) -> Resultat {
    authentifier(&Authentification {
        url: texte(d, "url")?,
        declarant: d.get("declarant").unwrap_or(&Value::Null),
        motdepasse: d.get("motdepasse").and_then(Value::as_str),
        variable: d.get("variable").and_then(Value::as_str).unwrap_or("NET_ENTREPRISES_MOTDEPASSE"),
        service: d.get("service").and_then(Value::as_str),
        jeton: d.get("jeton").and_then(Value::as_str),
    })
}

/// L'authentification du guide de l'API DSN : `{"ok", "jeton", "expire_le"}`. Sans mot de passe ou sans declarant
/// complet, rien ne part. Le mot de passe ne survit pas a cet appel et n'entre dans aucun message.
pub fn authentifier(a: &Authentification) -> Resultat {
    let variable = a.variable;
    let mdp = match a.motdepasse {
        Some(m) if !m.is_empty() => m,
        _ => {
            return Err(echec(
                "ErreurApi",
                format!(
                    "Mot de passe absent.\n\
Trois facons de le fournir, dans cet ordre de priorite :\n\n\
\x20 1. le passer a Client(motdepasse=...) -- ce que font les\n\
\x20    fenetres et les commandes apres avoir vu l'humain ;\n\
\x20 2. la variable d'environnement {v} ;\n\
\x20    PowerShell :  $env:{v} = (Read-Host \"mot de passe\")\n\
\x20 3. le coffre, pour les lectures automatiques :\n\
\x20    paie coffre poser net-entreprises\n",
                    v = variable
                ),
                Some(0),
            ))
        }
    };
    let decl = a.declarant;
    let manquants: Vec<&str> = ["siret", "nom", "prenom"].into_iter().filter(|c| !vrai(decl.get(*c))).collect();
    if !manquants.is_empty() {
        return Err(echec(
            "ErreurApi",
            format!(
                "Identite du declarant incomplete dans config/entreprise.json :\n\
\x20 \"declarant\": {{\"siret\": ..., \"nom\": ..., \"prenom\": ...}}\n\
Manque : {}.\n\
Ce sont le SIRET, le nom et le prenom inscrits au service DSN\n\
sur net-entreprises.fr (sans accent).",
                manquants.join(", ")
            ),
            Some(0),
        ));
    }
    let service = a.service.filter(|s| !s.is_empty());
    let corps = format!(
        "<identifiants><siret>{}</siret><nom>{}</nom><prenom>{}</prenom><motdepasse>{}</motdepasse>{}</identifiants>",
        en_texte(&decl["siret"]),
        en_texte(&decl["nom"]),
        en_texte(&decl["prenom"]),
        echapper(mdp),
        service.map(|s| format!("<service>{}</service>", s)).unwrap_or_default()
    );
    let url = a.url;
    let r = Requete {
        url,
        jeton: a.jeton,
        donnees: Some(corps.as_bytes()),
        content_type: Some("application/xml"),
        corps_gzip: false,
        post: false,
        delai: DELAI_APPEL,
        ..Default::default()
    };
    let (brut, entetes) = envoyer(&r).map_err(|p| en_erreur_api(url, p))?;
    let jeton: String = brut.iter().map(|&b| if b < 0x80 { b as char } else { '\u{fffd}' }).collect();
    Ok(json!({"ok": true, "jeton": py_strip(&jeton), "expire_le": entete(&entetes, "Expires")}))
}

/// L'envoi du corps d'un depot (deja prepare : octets gzip) : POST `text/plain`,
/// `Content-Encoding: gzip`, jeton DSNLogin. `forme` dit comment une panne se
/// raconte, `dsn` (`DepotRefuse`) ou `dpae` (`ErreurApi`).
pub fn depot(url: &str, jeton: &str, corps_gzip: &[u8], forme: &str, delai: u64) -> Result<(Vec<u8>, Entetes), Value> {
    let r = Requete {
        url,
        jeton: Some(jeton),
        donnees: Some(corps_gzip),
        content_type: Some("text/plain"),
        corps_gzip: true,
        post: true,
        delai,
        ..Default::default()
    };
    envoyer(&r).map_err(|p| match (forme, p) {
        ("dsn", Panne::Statut { code, corps }) => match corps {
            // `MESSAGES.get(str(code))` : les cles sont des entiers, le detail
            // n'a donc jamais ete affiche ; le corps passe par `_sans_secret`.
            Ok(c) => echec(
                "DepotRefuse",
                format!("Le point de depot a repondu {}.\n{}", code, sans_secret(&debut(&utf8_remplace(&c), 800))),
                None,
            ),
            Err(p) => echec_brut(*p),
        },
        ("dsn", Panne::Reseau(r)) => echec("DepotRefuse", format!("Pas de reponse du point de depot : {}", r), None),
        (_, Panne::Statut { code, corps }) => {
            let c = corps.map(|c| debut(&utf8_remplace(&c), 400)).unwrap_or_default();
            let mut m = format!("HTTP {} au depot sur {}\n  {}", code, sans_secret(url), message_du_statut(code));
            if !py_strip(&c).is_empty() {
                m.push_str("\n  ");
                m.push_str(&c);
            }
            echec("ErreurApi", m, None)
        }
        (_, Panne::Reseau(r)) => echec("ErreurApi", format!("Reseau injoignable pour {} : {}", sans_secret(url), r), None),
        (_, brute) => echec_brut(brute),
    })
}

fn depot_op(d: &Value) -> Resultat {
    let url = texte(d, "url")?;
    let jeton = d.get("jeton").map(en_texte).unwrap_or_else(|| "None".into());
    let corps = octets(d, "corps")?.unwrap_or_default();
    let forme = d.get("forme").and_then(Value::as_str).unwrap_or("dpae");
    depot(url, &jeton, &corps, forme, d.get("delai").and_then(Value::as_u64).unwrap_or(DELAI_DEPOT)).map(|(c, e)| reponse(&c, &e))
}

// -------------------------------------------------------------- l'archivage

fn racine(d: &Value) -> Result<PathBuf, Value> {
    Ok(PathBuf::from(texte(d, "racine")?))
}

fn canal(d: &Value) -> &str {
    d.get("canal").and_then(Value::as_str).unwrap_or("")
}

fn erreur_io(e: std::io::Error, chemin: &Path) -> Value {
    let classe = match e.kind() {
        std::io::ErrorKind::NotFound => "FileNotFoundError",
        std::io::ErrorKind::PermissionDenied => "PermissionError",
        _ => "OSError",
    };
    echec(classe, format!("{} : {}", errno_texte(&e), chemin.display()), None)
}

/// `dossier_archive` : `retours/` (la DSN) ou `retours/<canal>/`, cree.
pub fn dossier_archive(racine: &Path, canal: &str) -> Result<PathBuf, Value> {
    let mut p = racine.join("retours");
    if !canal.is_empty() {
        p = p.join(canal);
    }
    std::fs::create_dir_all(&p).map_err(|e| erreur_io(e, &p))?;
    Ok(p)
}

fn chemin_texte(p: &Path) -> Value {
    Value::String(p.to_string_lossy().into_owned())
}

/// `archiver` : la piece telle quelle, nommee `AAAAMMJJ-hhmmss_natureN_flux.ext`.
fn archiver(d: &Value) -> Resultat {
    let retour = d.get("retour").cloned().unwrap_or(Value::Null);
    let contenu = octets(d, "contenu")?.unwrap_or_default();
    let ctype = d.get("content_type").and_then(Value::as_str).unwrap_or("");
    let ext = if ctype.contains("pdf") {
        ".pdf"
    } else {
        let tete = &contenu[..contenu.len().min(200)];
        let debut_utile = tete.iter().position(|b| !matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)).unwrap_or(tete.len());
        if ctype.contains("xml") || tete[debut_utile..].starts_with(b"<") {
            ".xml"
        } else {
            ".txt"
        }
    };
    let chaine = |cle: &str| -> Result<String, Value> {
        match retour.get(cle) {
            None | Some(Value::Null) => Ok(String::new()),
            Some(Value::String(s)) => Ok(s.clone()),
            Some(_) => Err(echec("TypeError", format!("`{}` : une chaine est attendue", cle), None)),
        }
    };
    let publication = debut(&chaine("publication")?, 14);
    let horo = if publication.chars().count() == 14 {
        let c: Vec<char> = publication.chars().collect();
        format!("{}-{}", c[..8].iter().collect::<String>(), c[8..].iter().collect::<String>())
    } else {
        texte(d, "maintenant")?.to_string()
    };
    let nature = match retour.get("nature") {
        None => "??".to_string(),
        Some(v) => en_texte(v),
    };
    let flux = chaine("flux")?;
    let flux = if flux.is_empty() { "sansflux".to_string() } else { debut(&flux, 24) };
    let nom = format!("{}_nature{}_{}{}", horo, nature, flux, ext);
    let dossier = dossier_archive(&racine(d)?, canal(d))?;
    let dest = dossier.join(nom);
    if !dest.exists() {
        std::fs::write(&dest, &contenu).map_err(|e| erreur_io(e, &dest))?;
    }
    Ok(json!({"ok": true, "chemin": chemin_texte(&dest)}))
}

fn journal(racine: &Path, canal: &str) -> Result<PathBuf, Value> {
    Ok(dossier_archive(racine, canal)?.join("journal.json"))
}

/// Le journal tel qu'il est sur le disque (absent : une liste vide).
fn lire_journal(p: &Path) -> Result<Value, Value> {
    if !p.exists() {
        return Ok(Value::Array(Vec::new()));
    }
    let t = std::fs::read(p).map_err(|e| erreur_io(e, p))?;
    let t = String::from_utf8(t).map_err(|e| echec("UnicodeDecodeError", e.to_string(), None))?;
    serde_json::from_str::<Value>(&t).map_err(|e| echec("JSONDecodeError", e.to_string(), None))
}

fn cle_de_retour(e: &Value) -> Result<[Value; 3], Value> {
    if !e.is_object() {
        return Err(echec("AttributeError", "une ligne du journal n'est pas un objet".into(), None));
    }
    Ok(["flux", "nature", "publication"].map(|c| e.get(c).cloned().unwrap_or(Value::Null)))
}

/// L'egalite entre deux valeurs d'une cle : `1 == 1.0 == true`.
fn egal_py(a: &Value, b: &Value) -> bool {
    let nombre = |v: &Value| match v {
        Value::Number(n) => n.as_f64(),
        Value::Bool(x) => Some(if *x { 1.0 } else { 0.0 }),
        _ => None,
    };
    match (nombre(a), nombre(b)) {
        (Some(x), Some(y)) => x == y,
        _ => a == b,
    }
}

fn meme_cle(a: &[Value; 3], b: &[Value; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| egal_py(x, y))
}

/// `{(flux, nature, publication) for e in journal}` : ce que l'on tolere
/// d'un journal qui n'est pas une liste (`{}` ou `""` : rien a parcourir).
fn cles_vues(journal: &Value) -> Result<Vec<[Value; 3]>, Value> {
    match journal {
        Value::Array(a) => a.iter().map(cle_de_retour).collect(),
        Value::Object(o) if o.is_empty() => Ok(Vec::new()),
        Value::String(t) if t.is_empty() => Ok(Vec::new()),
        Value::Object(_) | Value::String(_) => Err(echec("AttributeError", "le journal n'est pas une liste de lignes".into(), None)),
        _ => Err(echec("TypeError", "le journal n'est pas iterable".into(), None)),
    }
}

/// `noter` : ajoute ce qui n'a pas ete vu ; LF, deux espaces, l'accent tel quel.
fn noter(d: &Value) -> Resultat {
    let p = journal(&racine(d)?, canal(d))?;
    let mut deja = lire_journal(&p)?;
    let connus = cles_vues(&deja)?;
    for e in d.get("entrees").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]) {
        let cle = cle_de_retour(e)?;
        if !connus.iter().any(|c| meme_cle(c, &cle)) {
            match &mut deja {
                Value::Array(a) => a.push(e.clone()),
                _ => return Err(echec("AttributeError", "le journal n'est pas une liste".into(), None)),
            }
        }
    }
    let mut t = serde_json::to_string_pretty(&deja).map_err(|e| echec("TypeError", e.to_string(), None))?;
    t.push('\n');
    std::fs::write(&p, t).map_err(|e| erreur_io(e, &p))?;
    Ok(json!({"ok": true}))
}

fn deja_vus(d: &Value) -> Resultat {
    let p = journal(&racine(d)?, canal(d))?;
    let vus: Vec<Value> = cles_vues(&lire_journal(&p)?)?.into_iter().map(|k| Value::Array(k.to_vec())).collect();
    Ok(json!({"ok": true, "vus": vus}))
}

fn constantes() -> Value {
    let messages: Map<String, Value> = MESSAGES.iter().map(|(c, m)| (c.to_string(), Value::String((*m).into()))).collect();
    json!({"ok": true, "user_agent": USER_AGENT, "messages": messages, "delai": DELAI_APPEL, "delai_depot": DELAI_DEPOT})
}

/// Une operation du transport (`operation` dans la demande).
pub fn operation(d: &Value) -> Resultat {
    let op = d.get("operation").and_then(Value::as_str).ok_or_else(|| echec("ValueError", "demande illisible : il faut `operation`".into(), None))?;
    match op {
        "constantes" => Ok(constantes()),
        "appel" => appel(d),
        "consulter" => consulter(d),
        "authentifier" => authentifier_op(d),
        "depot" => depot_op(d),
        "declarant" => Ok(json!({"ok": true, "declarant": declarant(d.get("entreprise").unwrap_or(&Value::Null))?})),
        "dossier_archive" => Ok(json!({"ok": true, "chemin": chemin_texte(&dossier_archive(&racine(d)?, canal(d))?)})),
        "journal" => Ok(json!({"ok": true, "chemin": chemin_texte(&journal(&racine(d)?, canal(d))?)})),
        "archiver" => archiver(d),
        "noter" => noter(d),
        "deja_vus" => deja_vus(d),
        "horodatage" => Ok(json!({"ok": true, "texte": horodatage(d.get("valeur").unwrap_or(&Value::Null)).map_err(|e| echec("ValueError", e, None))?})),
        "echapper" => Ok(json!({"ok": true, "texte": echapper(texte(d, "texte")?)})),
        "sans_secret" => Ok(json!({"ok": true, "texte": sans_secret(texte(d, "texte")?)})),
        autre => Err(echec("ValueError", format!("operation inconnue : {}", autre), None)),
    }
}

