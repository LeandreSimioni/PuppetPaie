//! Le faux serveur HTTP des essais : local (127.0.0.1), un fil par connexion. Ecrit une fois ; les aides de `paie`
//! l'incluent (`paie/tests/outils/mod.rs`).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use serde_json::Value;

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
