//! `moteur/instance.py` : une seule fenetre de PuppetPaie par poste.
//!
//! Le verrou est une prise d'ecoute sur la boucle locale, a un port fixe : la
//! tenir, c'est etre la fenetre du poste ; le systeme la rend quand le
//! processus meurt. Le second lancement s'y connecte, dit « devant » et
//! s'arrete. Meme port et memes mots que le Python (il s'entendent pendant la
//! transition). Si le port est tenu par un AUTRE programme, on ouvre quand
//! meme : un verrou ne doit jamais empecher une paie.

use std::io::{ErrorKind, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub const PORT_INSTANCE: u16 = 7330;
pub const SALUT: &[u8] = b"PuppetPaie devant\n";
pub const REPONSE: &[u8] = b"PuppetPaie ok\n";

const DELAI: Duration = Duration::from_secs(3);

/// Ce que rend `prendre`.
pub enum Prise {
    /// Cette fenetre est la premiere : a garder tant qu'elle vit.
    Tenue(Verrou),
    /// Une fenetre de PuppetPaie est deja ouverte, et vient d'etre ramenee devant.
    Devant,
    /// Le port est a un autre programme : pas de verrou.
    Sans,
}

pub struct Verrou {
    ferme: Arc<AtomicBool>,
}

impl Drop for Verrou {
    fn drop(&mut self) {
        self.ferme.store(true, Ordering::SeqCst);
    }
}

/// Demande a la fenetre deja ouverte de revenir devant. Vrai si c'est bien
/// PuppetPaie qui a repondu.
pub fn appeler(port: u16) -> bool {
    let adresse = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let Ok(mut c) = TcpStream::connect_timeout(&adresse, DELAI) else { return false };
    let _ = c.set_read_timeout(Some(DELAI));
    if c.write_all(SALUT).is_err() {
        return false;
    }
    let mut tampon = [0u8; 64];
    matches!(c.read(&mut tampon), Ok(n) if &tampon[..n] == REPONSE)
}

/// Prend le verrou du poste ; `devant` est appelee a chaque lancement suivant.
pub fn prendre<F: Fn() + Send + 'static>(devant: F, port: u16) -> Prise {
    let Ok(ecoute) = TcpListener::bind((Ipv4Addr::LOCALHOST, port)) else {
        return if appeler(port) { Prise::Devant } else { Prise::Sans };
    };
    if ecoute.set_nonblocking(true).is_err() {
        return Prise::Sans;
    }
    let ferme = Arc::new(AtomicBool::new(false));
    let vu = ferme.clone();
    std::thread::spawn(move || {
        while !vu.load(Ordering::SeqCst) {
            match ecoute.accept() {
                Ok((mut c, _)) => {
                    let _ = c.set_nonblocking(false);
                    let _ = c.set_read_timeout(Some(DELAI));
                    let mut tampon = [0u8; 64];
                    match c.read(&mut tampon) {
                        Ok(n) if &tampon[..n] == SALUT && c.write_all(REPONSE).is_ok() => devant(),
                        _ => {}
                    }
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(20)),
                Err(_) => std::thread::sleep(Duration::from_millis(20)),
            }
        }
    });
    Prise::Tenue(Verrou { ferme })
}
