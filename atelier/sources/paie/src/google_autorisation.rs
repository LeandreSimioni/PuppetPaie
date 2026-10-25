//! L'autorisation du compte Google, SANS navigateur (`google.autoriser`, `moteur/google.py`) : le flux « application de bureau » avec
//! PKCE. Ecrit au lot J2b (theme 3b), non prouve.
//!
//! Le moteur ecoute sur `127.0.0.1`, port tire au hasard, affiche l'URL de la fenetre de consentement et attend que Google y renvoie
//! le code. **Il n'ouvre aucun navigateur** : la fenetre de consentement est au lot J3b ; la personne ouvre l'URL elle-meme.
//!
//! - Le verificateur PKCE (64 octets aleatoires du systeme, base64 URL), le `state` (24 octets) ne sont ecrits nulle part et ne
//!   sortent pas : seuls le defi (SHA-256 du verificateur) et le `state` figurent dans l'URL, comme en Python.
//! - Le jeton de rafraichissement va du corps de la reponse de Google au fichier scelle `config/google-jeton.bin` (cle du poste,
//!   `Coffre::proteger`) sans passer par une sortie, un journal ni un message d'erreur ; le secret de l'application ne sert que dans
//!   le corps du POST. Une phrase d'erreur ne cite jamais un jeton ni un code.
//! - L'attente est un fil + `mpsc::recv_timeout` : jamais de `sleep`.
//!
//! DEFAUTS DU PYTHON, QUI NE SE REPRODUISENT PAS : le texte de l'erreur renvoyee par Google etait recopie sans echappement dans la page
//! de retour (HTML injecte, au prix d'un `state` valide) ; une reponse sans `access_token`, un corps illisible, une adresse de compte
//! vide, un code absent finissaient en trace d'appels ou en jeton sans compte : ici une phrase et rien d'ecrit. Les pannes de reseau
//! que le Python laissait sortir brutes (delai depasse) sont une phrase.
//! NON PORTE : l'abandon par Ctrl-C (« Abandon : aucune autorisation enregistree. ») -- le processus s'arrete, rien n'est ecrit.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use socle::echec::Echec;
use crate::routes_google::{en_texte, encoder, identifiants, phrase_de_panne, DESCRIPTION, FICHIER_JETON, JETONS, PERMISSIONS};
use crate::transport::{envoyer, Requete};

/// `google.AUTORISATION`, `google.USERINFO`, `google.DELAI_AUTORISATION`.
const AUTORISATION: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const USERINFO: &str = "https://www.googleapis.com/oauth2/v3/userinfo";
pub const DELAI_AUTORISATION: u64 = 300;

/// `google.PAGE_RETOUR`.
fn page_retour(titre: &str, couleur: &str, texte: &str) -> String {
    format!(
        "<!doctype html><html lang=\"fr\"><meta charset=\"utf-8\">\n<title>{t}</title>\n<style>body{{font:16px/1.6 system-ui,\"Segoe UI\",sans-serif;background:#14141a;\ncolor:#e8e8ec;display:grid;place-items:center;height:100vh;margin:0;text-align:center}}\nh1{{font-size:20px;color:{c};margin:0 0 8px}}p{{color:#9a9aa8;margin:0}}</style>\n<div><h1>{t}</h1><p>{x}</p></div></html>",
        t = titre,
        c = couleur,
        x = texte
    )
}

fn echapper(t: &str) -> String {
    t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#x27;")
}

fn refus(message: impl Into<String>) -> Echec {
    Echec { classe: Some("ErreurOperation"), message: message.into() }
}

/// Les adresses de Google : les vraies ; dans l'atelier seulement, le faux Google local (`atelier.rs`).
fn adresses() -> (String, String) {
    #[cfg(feature = "atelier")]
    if let Some(b) = crate::atelier::base(crate::atelier::VARIABLE_GOOGLE) {
        return (format!("{}/token", b), format!("{}/userinfo", b));
    }
    (JETONS.to_string(), USERINFO.to_string())
}

/// `secrets.token_urlsafe(n)` : `n` octets du systeme, base64 URL sans `=`.
fn aleatoire_url(n: usize) -> Result<String, Echec> {
    let mut o = vec![0u8; n];
    getrandom::fill(&mut o).map_err(|e| refus(format!("Le systeme ne donne pas d'aleas : {}.", e)))?;
    Ok(base64_url(&o))
}

fn base64_url(o: &[u8]) -> String {
    crate::retours::base64_ecrire(o).replace('+', "-").replace('/', "_").trim_end_matches('=').to_string()
}

fn egaux(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// `urllib.parse.parse_qs` : `%XX` et `+`, les valeurs vides ecartees.
fn decoder(t: &str) -> String {
    let b = t.as_bytes();
    let mut sortie = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => sortie.push(b' '),
            b'%' if b.get(i + 1).zip(b.get(i + 2)).is_some_and(|(h, l)| h.is_ascii_hexdigit() && l.is_ascii_hexdigit()) => {
                let h = (b[i + 1] as char).to_digit(16).unwrap_or(0) as u8;
                let l = (b[i + 2] as char).to_digit(16).unwrap_or(0) as u8;
                sortie.push(h * 16 + l);
                i += 2;
            }
            c => sortie.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&sortie).into_owned()
}

fn premiere_valeur(requete: &str, cle: &str) -> String {
    requete
        .split('&')
        .filter_map(|p| p.split_once('='))
        .find(|(k, v)| decoder(k) == cle && !v.is_empty())
        .map(|(_, v)| decoder(v))
        .unwrap_or_default()
}

/// Ce que Google a renvoye a l'adresse locale.
struct Retour {
    code: String,
    erreur: String,
}

/// Sert une connexion : `Some(retour)` quand c'est la reponse de Google (le `state` est le bon), `None` sinon (on continue d'attendre).
/// Un `state` faux : 403, comme le Python. Une autre methode que GET : 501, comme `BaseHTTPRequestHandler`.
fn servir(flux: &mut TcpStream, etat: &str) -> Option<Retour> {
    let _ = flux.set_read_timeout(Some(Duration::from_secs(5)));
    let mut tampon = [0u8; 8192];
    let mut lu = 0;
    while lu < tampon.len() {
        match flux.read(&mut tampon[lu..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                lu += n;
                if tampon[..lu].windows(2).any(|w| w == b"\r\n") {
                    break;
                }
            }
        }
    }
    let texte = String::from_utf8_lossy(&tampon[..lu]).into_owned();
    let ligne = texte.lines().next().unwrap_or("");
    let mut mots = ligne.split(' ');
    let (methode, cible) = (mots.next().unwrap_or(""), mots.next().unwrap_or(""));
    let repondre_vide = |flux: &mut TcpStream, statut: &str| {
        let _ = flux.write_all(format!("HTTP/1.1 {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", statut).as_bytes());
    };
    if methode != "GET" {
        repondre_vide(flux, "501 Not Implemented");
        return None;
    }
    let requete = cible.split_once('?').map_or("", |(_, q)| q.split('#').next().unwrap_or(""));
    if !egaux(&premiere_valeur(requete, "state"), etat) {
        repondre_vide(flux, "403 Forbidden");
        return None;
    }
    let retour = Retour { code: premiere_valeur(requete, "code"), erreur: premiere_valeur(requete, "error") };
    let page = if retour.erreur.is_empty() {
        page_retour("C'est bon.", "#4f9f6f", "Tu peux fermer cette fenetre et revenir a PuppetPaie.")
    } else {
        page_retour("Autorisation refusee", "#d05353", &echapper(&retour.erreur))
    };
    let corps = page.into_bytes();
    let _ = flux.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", corps.len()).as_bytes());
    let _ = flux.write_all(&corps);
    Some(retour)
}

/// Le corps JSON d'une reponse de Google, ou la phrase de la panne.
fn appeler(requete: &Requete) -> Result<Value, Echec> {
    match envoyer(requete) {
        Ok((brut, _)) => String::from_utf8(brut)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .filter(Value::is_object)
            .ok_or_else(|| refus("Google a rendu une reponse illisible. Aucune autorisation enregistree. Recommencer plus tard.")),
        Err(panne) => match phrase_de_panne(panne) {
            Ok(phrase) => Err(refus(phrase)),
            Err((_, message)) => Err(refus(format!("Google ne repond pas : {}. Aucune autorisation enregistree.", message))),
        },
    }
}

/// `google.autoriser` sans navigateur. `dire_url` recoit l'URL de la fenetre de consentement (la personne l'ouvre), `dire` les
/// avertissements. Rend l'adresse du compte lie. `delai` : l'attente de la reponse de Google, en secondes.
pub fn autoriser(racine: &Path, delai: u64, dire_url: &dyn Fn(&str), dire: &dyn Fn(&str)) -> Result<String, Echec> {
    let (jetons, userinfo) = adresses();
    let (client_id, client_secret, _) = identifiants(racine).map_err(|e| refus(e.message))?;
    let verificateur = aleatoire_url(64)?;
    let defi = base64_url(&Sha256::digest(verificateur.as_bytes()));
    let etat = aleatoire_url(24)?;

    let ecouteur = TcpListener::bind("127.0.0.1:0").map_err(|e| refus(format!("Impossible d'ecouter sur 127.0.0.1 : {}.", e)))?;
    let adresse = ecouteur.local_addr().map_err(|e| refus(format!("Impossible d'ecouter sur 127.0.0.1 : {}.", e)))?;
    let redirection = format!("http://127.0.0.1:{}/", adresse.port());
    let url = format!(
        "{}?{}",
        AUTORISATION,
        encoder(&[
            ("client_id", en_texte(&client_id)),
            ("redirect_uri", redirection.clone()),
            ("response_type", "code".into()),
            ("scope", PERMISSIONS.join(" ")),
            ("access_type", "offline".into()),
            // Sans « consent », Google ne renvoie le jeton de rafraichissement qu'a la toute premiere autorisation.
            ("prompt", "consent".into()),
            ("code_challenge", defi),
            ("code_challenge_method", "S256".into()),
            ("state", etat.clone()),
        ])
    );

    let (tx, rx) = mpsc::channel::<Retour>();
    let fini = Arc::new(AtomicBool::new(false));
    let fil_fini = Arc::clone(&fini);
    let fil_etat = etat.clone();
    std::thread::spawn(move || {
        for flux in ecouteur.incoming() {
            if fil_fini.load(Ordering::SeqCst) {
                break;
            }
            let Ok(mut flux) = flux else { continue };
            if let Some(r) = servir(&mut flux, &fil_etat) {
                let _ = tx.send(r);
                break;
            }
        }
    });
    dire_url(&url);

    let recu = match rx.recv_timeout(Duration::from_secs(delai)) {
        Ok(r) => r,
        Err(_) => {
            // Reveille le fil, qui attend une connexion, pour qu'il rende le port.
            fini.store(true, Ordering::SeqCst);
            let _ = TcpStream::connect(adresse);
            return Err(refus(format!(
                "Aucune reponse de Google en {} minutes : aucune autorisation enregistree. Recommencer, et accepter dans la fenetre du navigateur.",
                std::cmp::max(1, delai / 60)
            )));
        }
    };
    if !recu.erreur.is_empty() {
        return Err(refus(format!("Google a refuse : {}", recu.erreur)));
    }
    if recu.code.is_empty() {
        return Err(refus("Google n'a renvoye aucun code d'autorisation : aucune autorisation enregistree. Recommencer."));
    }

    let corps = encoder(&[
        ("client_id", en_texte(&client_id)),
        ("client_secret", en_texte(&client_secret)),
        ("code", recu.code),
        ("code_verifier", verificateur),
        ("grant_type", "authorization_code".into()),
        ("redirect_uri", redirection),
    ]);
    let reponse = appeler(&Requete {
        url: &jetons,
        donnees: Some(corps.as_bytes()),
        content_type: Some("application/x-www-form-urlencoded"),
        post: true,
        delai: crate::routes_google::DELAI,
        ..Default::default()
    })?;
    let texte_de = |cle: &str| reponse.get(cle).filter(|v| v.is_string()).map(en_texte).unwrap_or_default();
    let rafraichissement = texte_de("refresh_token");
    if rafraichissement.is_empty() {
        return Err(refus(
            "Google n'a pas renvoye de jeton de rafraichissement. Retirer\nl'acces de l'application sur myaccount.google.com > Securite >\nVos connexions a des applications tierces, puis recommencer.",
        ));
    }
    let acces = texte_de("access_token");
    if acces.is_empty() {
        return Err(refus("Google n'a pas renvoye de jeton d'acces : aucune autorisation enregistree. Recommencer."));
    }
    let mut accordees: Vec<String> = texte_de("scope").split_whitespace().map(str::to_string).collect();
    accordees.sort();
    accordees.dedup();
    let oubliees: Vec<&str> = PERMISSIONS.iter().filter(|p| !accordees.iter().any(|a| a == *p)).map(|p| p.rsplit('/').next().unwrap_or(p)).collect();
    if !oubliees.is_empty() {
        dire(&format!(
            "Attention : permission(s) non accordee(s) : {}.\n  Les cases correspondantes n'ont pas ete cochees dans la fenetre.",
            oubliees.join(", ")
        ));
    }

    let autorisation = format!("Bearer {}", acces);
    let entetes = [("Authorization", autorisation.as_str())];
    let infos = appeler(&Requete { url: &userinfo, entetes: &entetes, delai: crate::routes_google::DELAI, ..Default::default() })?;
    let compte = infos.get("email").filter(|v| v.is_string()).map(en_texte).unwrap_or_default();
    if compte.is_empty() {
        return Err(refus("Google n'a pas donne l'adresse du compte : aucune autorisation enregistree. Recommencer."));
    }

    ecrire_le_jeton(racine, &rafraichissement, &compte, &accordees)?;
    Ok(compte)
}

/// `google._ecrire_jeton` : le jeton et l'adresse du compte, dans un seul fichier scelle par la cle du poste.
fn ecrire_le_jeton(racine: &Path, rafraichissement: &str, compte: &str, permissions: &[String]) -> Result<(), Echec> {
    let brut = crate::coffre::dumps(&json!({"rafraichissement": rafraichissement, "compte": compte, "permissions": permissions}), None, false, false);
    let scelle = crate::coffre::Coffre::du_poste(racine, "")
        .proteger(brut.as_bytes(), DESCRIPTION)
        .map_err(|e| refus(format!("Le jeton n'a pas pu etre scelle, rien n'est enregistre : {}", e.message)))?;
    let dossier = racine.join("config");
    std::fs::create_dir_all(&dossier).map_err(|e| refus(format!("{} : {}", dossier.display(), e)))?;
    let chemin = dossier.join(FICHIER_JETON);
    std::fs::write(&chemin, scelle).map_err(|e| refus(format!("{} : {}", chemin.display(), e)))
}
