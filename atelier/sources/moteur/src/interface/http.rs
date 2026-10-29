//! Un serveur HTTP/1.1 minuscule, sur 127.0.0.1 seulement : ce que la fenetre de PuppetPaie interroge (`web.py`).
//!
//! Un fil par connexion, la connexion gardee ouverte (HTTP/1.1, `Content-Length` toujours pose) : en HTTP/1.0, Windows
//! coupait parfois un gros envoi avant que la page l'ait lu (15/09/2026). Le serveur ne ferme jamais le premier.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

/// Au-dela, une requete est refusee : la page n'envoie que du JSON et des images de quelques Mo.
const CORPS_MAX: usize = 64 * 1024 * 1024;

pub struct Requete {
    pub methode: String,
    pub chemin: String,
    pub params: HashMap<String, String>,
    pub entetes: HashMap<String, String>,
    pub corps: Vec<u8>,
}

impl Requete {
    pub fn entete(&self, nom: &str) -> Option<&str> {
        self.entetes.get(&nom.to_ascii_lowercase()).map(String::as_str)
    }

    pub fn cookie(&self, nom: &str) -> Option<String> {
        self.entete("cookie")?.split(';').find_map(|c| {
            let (k, v) = c.trim().split_once('=')?;
            (k == nom).then(|| v.trim_matches('"').to_string())
        })
    }
}

pub struct Reponse {
    pub code: u16,
    pub type_mime: String,
    pub corps: Vec<u8>,
    pub entetes: Vec<(String, String)>,
}

impl Reponse {
    pub fn nouvelle(code: u16, type_mime: &str, corps: impl Into<Vec<u8>>) -> Reponse {
        Reponse { code, type_mime: type_mime.into(), corps: corps.into(), entetes: Vec::new() }
    }

    pub fn json(code: u16, v: &serde_json::Value) -> Reponse {
        Reponse::nouvelle(code, "application/json; charset=utf-8", serde_json::to_vec(v).unwrap_or_default())
    }

    pub fn avec(mut self, nom: &str, valeur: impl Into<String>) -> Reponse {
        self.entetes.push((nom.into(), valeur.into()));
        self
    }
}

fn phrase(code: u16) -> &'static str {
    match code {
        200 => "OK",
        303 => "See Other",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        413 => "Payload Too Large",
        _ => "Error",
    }
}

/// `%xx` -> l'octet, `+` -> espace (comme `unquote_plus`). Un `%` que deux chiffres hexadecimaux ne suivent pas reste
/// tel quel : `u8::from_str_radix` accepterait un signe (`%+1`), on ne lui donne que des chiffres (reste 143).
pub fn decoder_pourcent(s: &str) -> String {
    let o = s.as_bytes();
    let mut v = Vec::with_capacity(o.len());
    let mut i = 0;
    while i < o.len() {
        match o[i] {
            b'+' => v.push(b' '),
            b'%' if i + 2 < o.len() && o[i + 1].is_ascii_hexdigit() && o[i + 2].is_ascii_hexdigit() => {
                let chiffre = |c: u8| (c as char).to_digit(16).unwrap_or(0) as u8;
                v.push(chiffre(o[i + 1]) * 16 + chiffre(o[i + 2]));
                i += 2;
            }
            b => v.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&v).into_owned()
}

pub fn encoder_pourcent(s: &str) -> String {
    s.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{:02X}", b) })
        .collect()
}

fn lire_requete(flux: &mut BufReader<TcpStream>) -> Option<Result<Requete, u16>> {
    let mut ligne = String::new();
    if flux.read_line(&mut ligne).ok()? == 0 {
        return None;
    }
    let mut morceaux = ligne.split_whitespace();
    let methode = morceaux.next()?.to_string();
    let cible = morceaux.next()?.to_string();
    let mut entetes = HashMap::new();
    loop {
        let mut l = String::new();
        if flux.read_line(&mut l).ok()? == 0 {
            return None;
        }
        let l = l.trim_end();
        if l.is_empty() {
            break;
        }
        if let Some((k, v)) = l.split_once(':') {
            entetes.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }
    let longueur: usize = entetes.get("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
    if longueur > CORPS_MAX {
        return Some(Err(413));
    }
    let mut corps = vec![0; longueur];
    flux.read_exact(&mut corps).ok()?;
    let (chemin, requete) = cible.split_once('?').unwrap_or((&cible, ""));
    let params = requete
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (decoder_pourcent(k), decoder_pourcent(v))
        })
        .collect();
    Some(Ok(Requete { methode, chemin: decoder_pourcent(chemin), params, entetes, corps }))
}

/// Une requete refusee avant d'etre lue : un corps JSON que la page sait lire (`d.erreur.message`), et non un
/// corps vide sur lequel `r.json()` levait une SyntaxError.
fn refus_de_lecture(code: u16) -> Reponse {
    let message = match code {
        413 => format!(
            "La demande est trop grosse pour PuppetPaie (plus de {} Mo) : rien n'a été lu ni enregistré.",
            CORPS_MAX / (1024 * 1024)
        ),
        _ => format!("La demande n'a pas pu être lue (code {}).", code),
    };
    Reponse::json(code, &serde_json::json!({"ok": false, "erreur": {"type": "RequeteRefusee", "message": message}}))
}

fn ecrire_reponse(flux: &mut TcpStream, r: &Reponse) -> std::io::Result<()> {
    let mut tete = format!("HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n", r.code, phrase(r.code), r.type_mime, r.corps.len());
    // Rien ne se garde, sauf ce qui le dit (une feuille ou un script nomme par son empreinte, `interface/mod.rs`).
    if !r.entetes.iter().any(|(k, _)| k == "Cache-Control") {
        tete.push_str("Cache-Control: no-cache\r\n");
    }
    if !r.entetes.iter().any(|(k, _)| k == "Content-Security-Policy") {
        tete.push_str("Content-Security-Policy: default-src 'self' 'unsafe-inline'; img-src 'self' data:; frame-ancestors 'none'\r\n");
    }
    for (k, v) in &r.entetes {
        tete.push_str(&format!("{}: {}\r\n", k, v));
    }
    tete.push_str("\r\n");
    // UNE SEULE ECRITURE, SANS NAGLE : la tete puis le corps en deux `write`, puis la lecture de la requete
    // suivante, c'est le motif ou le reste du corps attend l'accuse de reception de la tete, que l'autre bout differe
    // (jusqu'a 200 ms sous Windows ; sous Linux, en boucle locale, l'accuse part aussitot et rien ne se mesure). Tout part
    // d'un coup, sans attente (`servir_connexion` : `set_nodelay`).
    let mut tout = Vec::with_capacity(tete.len() + r.corps.len());
    tout.extend_from_slice(tete.as_bytes());
    tout.extend_from_slice(&r.corps);
    flux.write_all(&tout)?;
    flux.flush()
}

fn servir_connexion(flux: TcpStream, traiter: Arc<dyn Fn(Requete) -> Reponse + Send + Sync>) {
    let _ = flux.set_read_timeout(Some(Duration::from_secs(30)));
    let _ = flux.set_nodelay(true);
    let Ok(mut sortie) = flux.try_clone() else { return };
    let mut entree = BufReader::new(flux);
    while let Some(r) = lire_requete(&mut entree) {
        let (reponse, refusee) = match r {
            Ok(req) => (traiter(req), false),
            Err(code) => (refus_de_lecture(code), true),
        };
        // Le client parti (la fenetre fermee pendant un releve) n'est pas un defaut : il n'y a plus personne a qui le dire.
        if ecrire_reponse(&mut sortie, &reponse).is_err() {
            return;
        }
        // Le corps d'une requete refusee n'a pas ete lu : la suite du flux n'est plus une requete. On ferme.
        if refusee {
            return;
        }
    }
}

/// Ecoute sur 127.0.0.1:`port` (le port fixe s'il est libre, sinon un port tire par le systeme). Rend l'ecouteur et son port.
pub fn ecouter(port: u16) -> std::io::Result<(TcpListener, u16)> {
    let l = TcpListener::bind(("127.0.0.1", port)).or_else(|_| TcpListener::bind(("127.0.0.1", 0)))?;
    sans_heritage(&l);
    let p = l.local_addr()?.port();
    Ok((l, p))
}

/// Une prise d'ecoute ne passe jamais a un processus enfant (une route, le relais de la mise a jour). La bibliotheque
/// standard la cree deja non heritable, mais un fournisseur reseau intercale (un antivirus) peut perdre ce drapeau : un
/// enfant la gardait alors ouverte apres la mort de la fenetre (mesure le 09/10/2026, port tenu par un PID mort).
pub fn sans_heritage(l: &TcpListener) {
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawSocket;
        extern "system" {
            fn SetHandleInformation(objet: isize, masque: u32, drapeaux: u32) -> i32;
        }
        const HANDLE_FLAG_INHERIT: u32 = 1;
        // SAFETY : la prise est vivante (empruntee) ; on ne change que son drapeau d'heritage.
        unsafe {
            SetHandleInformation(l.as_raw_socket() as isize, HANDLE_FLAG_INHERIT, 0);
        }
    }
    #[cfg(not(windows))]
    let _ = l;
}

/// Sert pour toujours ; chaque connexion dans son propre fil, chaque requete traitee par `traiter`.
pub fn servir(ecouteur: TcpListener, traiter: Arc<dyn Fn(Requete) -> Reponse + Send + Sync>) {
    for flux in ecouteur.incoming().flatten() {
        let t = traiter.clone();
        crate::routes::fil_de_routes(move || servir_connexion(flux, t));
    }
}
