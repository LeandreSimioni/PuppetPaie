//! Le transport contre un faux serveur local : ce que l'authentification et le depot envoient, selon le guide
//! d'implementation de l'API DSN (`atelier/reference/sources/gip-mds-api-dsn/gip-mds-guide-implementation-api-dsn-v6.8-20260706.txt`,
//! l'exemple de requete d'authentification, l. 1091-1105, et le depot, l. 915-919), et ce qui ne part pas. Mot de passe
//! invente.

use std::io::Write;

use envois::transport::{authentifier, depot, Authentification};
use serde_json::{json, Value};

use crate::outils::{serveur, Recu, Requete};

fn reponse(code: u16, entetes: &[(&str, &str)], corps: &[u8]) -> Vec<u8> {
    let mut r = format!("HTTP/1.1 {} X\r\n", code).into_bytes();
    for (k, v) in entetes {
        r.extend(format!("{}: {}\r\n", k, v).bytes());
    }
    r.extend(format!("Content-Length: {}\r\n\r\n", corps.len()).bytes());
    r.extend(corps);
    r
}

fn gzip(donnees: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(donnees).unwrap();
    e.finish().unwrap()
}

fn en_tete<'a>(v: &'a Requete, nom: &str) -> Option<&'a str> {
    v.entetes.iter().find(|(k, _)| k == nom).map(|(_, v)| v.as_str())
}

fn faux() -> (String, Recu) {
    serveur(|q| match q.chemin.as_str() {
        "/auth" => Some(reponse(200, &[("Expires", "Sat, 03 Oct 2026 14:00:00 GMT")], b" JETON-123 \n")),
        "/depot" => Some(reponse(200, &[("Content-Type", "application/xml")], b"<retours/>")),
        "/ferme" => None,
        c if c.starts_with("/code/") => Some(reponse(c[6..].parse().unwrap(), &[("Content-Type", "text/plain")], b"refus")),
        _ => Some(reponse(404, &[], b"inconnu")),
    })
}

fn declarant() -> Value {
    json!({"siret": "12345678901234", "nom": "DUPONT", "prenom": "Jean Pierre"})
}

fn auth<'a>(url: &'a str, declarant: &'a Value, motdepasse: Option<&'a str>) -> Authentification<'a> {
    Authentification { url, declarant, motdepasse, variable: "NET_ENTREPRISES_MOTDEPASSE", service: Some("97"), jeton: None }
}

#[test]
fn l_authentification_envoie_les_identifiants_du_guide_et_rend_le_jeton() {
    let (base, recu) = faux();
    let (url, d) = (format!("{}/auth", base), declarant());
    let r = authentifier(&auth(&url, &d, Some("a&b<c>d"))).unwrap();
    assert_eq!(r["jeton"], json!("JETON-123"));
    let v = &recu.lock().unwrap()[0];
    assert_eq!((v.methode.as_str(), en_tete(v, "content-type")), ("POST", Some("application/xml")));
    // Le mot de passe s'echappe : un `<` ou un `&` ne casse pas le XML du guide.
    assert_eq!(
        String::from_utf8_lossy(&v.corps),
        "<identifiants><siret>12345678901234</siret><nom>DUPONT</nom><prenom>Jean Pierre</prenom><motdepasse>a&amp;b&lt;c&gt;d</motdepasse><service>97</service></identifiants>"
    );
}

#[test]
fn un_mot_de_passe_absent_ou_un_declarant_incomplet_ne_partent_pas() {
    let (base, recu) = faux();
    let url = format!("{}/auth", base);
    let d = declarant();
    for mdp in [None, Some("")] {
        assert!(authentifier(&auth(&url, &d, mdp)).is_err());
    }
    let incomplet = json!({"siret": "1", "nom": "", "prenom": null});
    assert!(authentifier(&auth(&url, &incomplet, Some("secret"))).is_err());
    assert!(recu.lock().unwrap().is_empty(), "rien n'est parti");
}

#[test]
fn le_mot_de_passe_ne_fuit_dans_aucun_message() {
    let (base, _) = faux();
    let d = declarant();
    for chemin in ["/code/401", "/code/500", "/ferme", "/introuvable"] {
        let url = format!("{}{}", base, chemin);
        let r = authentifier(&auth(&url, &d, Some("Tres-Secret-42")));
        let tout = match r {
            Ok(v) | Err(v) => v.to_string(),
        };
        assert!(!tout.contains("Tres-Secret-42"), "{} : {}", chemin, tout);
    }
}

#[test]
fn un_depot_part_en_post_text_plain_gzip_avec_le_jeton() {
    let (base, recu) = faux();
    let corps = gzip(b"faux depot");
    let (rendu, _) = depot(&format!("{}/depot", base), "J-9", &corps, "dsn", 10).unwrap();
    assert_eq!(rendu, b"<retours/>");
    let v = &recu.lock().unwrap()[0];
    assert_eq!(v.methode, "POST");
    assert_eq!(en_tete(v, "content-type"), Some("text/plain"));
    assert_eq!(en_tete(v, "content-encoding"), Some("gzip"));
    assert_eq!(en_tete(v, "authorization"), Some("DSNLogin jeton=J-9"));
    assert_eq!(v.corps, corps);
}
