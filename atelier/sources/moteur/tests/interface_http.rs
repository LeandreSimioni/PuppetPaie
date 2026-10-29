//! `moteur interface` : le lanceur refuse en phrase et un corps trop gros se dit en JSON. Le journal de la fenetre
//! va dans un dossier utilisateur sous `CARGO_TARGET_TMPDIR`, jamais celui du poste ; le serveur n'ecoute que sur 127.0.0.1.

use moteur::interface::demarrer_sur;
use std::path::Path;

/// Le dossier utilisateur des fenetres de ce fichier, pose dans l'environnement du processus, sous un verrou : un
/// autre test qui le changerait ecrirait son journal dans le dossier que celui-ci efface.
fn utilisateur() -> (std::sync::MutexGuard<'static, ()>, std::path::PathBuf) {
    static VERROU: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let g = VERROU.lock().unwrap_or_else(|e| e.into_inner());
    let dossier = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("interface-http-utilisateur-{}", std::process::id()));
    std::fs::create_dir_all(&dossier).unwrap();
    for v in ["LOCALAPPDATA", "HOME", "USERPROFILE", "XDG_DATA_HOME"] {
        std::env::set_var(v, &dossier);
    }
    (g, dossier)
}

/// Une requete brute au serveur de la fenetre : le code et le corps de la reponse.
fn requete_brute(port: u16, brut: &str) -> (u16, String) {
    use std::io::{Read, Write};
    let mut c = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    c.set_read_timeout(Some(std::time::Duration::from_secs(10))).unwrap();
    c.write_all(brut.as_bytes()).unwrap();
    let mut tout = Vec::new();
    let mut tampon = [0u8; 4096];
    // La reponse a un `Content-Length` : on lit jusqu'a l'avoir entiere.
    loop {
        let n = c.read(&mut tampon).unwrap_or(0);
        if n == 0 {
            break;
        }
        tout.extend_from_slice(&tampon[..n]);
        let texte = String::from_utf8_lossy(&tout).into_owned();
        if let Some((tete, corps)) = texte.split_once("\r\n\r\n") {
            let longueur: usize = tete
                .lines()
                .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap()))
                .unwrap_or(0);
            if corps.len() >= longueur {
                break;
            }
        }
    }
    let texte = String::from_utf8_lossy(&tout).into_owned();
    let (tete, corps) = texte.split_once("\r\n\r\n").unwrap();
    (tete.split_whitespace().nth(1).unwrap().parse().unwrap(), corps.to_string())
}

/// Un geste de l'accueil refuse (jeton absent) dit une PHRASE, pas un objet (« [object Object] ») ; un corps
/// trop gros recoit une reponse JSON qui le dit, pas un corps vide que la page ne sait pas lire.
#[test]
fn le_lanceur_refuse_en_phrase_et_un_corps_trop_gros_se_dit_en_json() {
    let (_verrou, utilisateur) = utilisateur();
    let (_f, port) = demarrer_sur(0, 0).unwrap();
    let (code, corps) = requete_brute(port, "POST /lanceur/choisir HTTP/1.1\r\nHost: x\r\nContent-Length: 2\r\n\r\n[]");
    assert_eq!(code, 403);
    let v: serde_json::Value = serde_json::from_str(&corps).unwrap();
    assert_eq!(v["ok"], serde_json::json!(false));
    assert!(v["erreur"].is_string(), "{}", corps);
    let (code, corps) = requete_brute(port, "POST /api/etat HTTP/1.1\r\nHost: x\r\nContent-Length: 999999999\r\n\r\n");
    assert_eq!(code, 413);
    let v: serde_json::Value = serde_json::from_str(&corps).unwrap_or_else(|_| panic!("corps illisible : {:?}", corps));
    assert_eq!(v["ok"], serde_json::json!(false));
    assert!(v["erreur"]["message"].as_str().unwrap().contains("trop"), "{}", corps);
    let _ = std::fs::remove_dir_all(&utilisateur);
}
