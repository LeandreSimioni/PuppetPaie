//! Ce que le Drive et l'autorisation Google partagent : les adresses et les permissions de Google, l'encodage d'un
//! formulaire, la phrase d'une reponse d'erreur et le jeton d'acces obtenu pour un jeton de rafraichissement. Rien ici
//! ne lit le poste : le jeton, les identifiants et les adresses viennent de l'appelant.

use serde_json::Value;

use socle::echec::{erreur, Echec};
use socle::fichiers::vrai;
use socle::valeur::nom_type;

use crate::transport::{envoyer, Panne, Requete};

type R<T> = Result<T, Echec>;

/// L'adresse des jetons de Google.
pub const JETONS: &str = "https://oauth2.googleapis.com/token";

/// Les permissions que le logiciel demande : les fichiers qu'il cree dans le Drive, et l'adresse du compte.
pub const PERMISSIONS: [&str; 2] =
    ["https://www.googleapis.com/auth/drive.file", "https://www.googleapis.com/auth/userinfo.email"];

/// Le delai d'un appel a Google, en secondes.
pub const DELAI: u64 = 60;

/// `(j or {}).get(cle)` : `Null` pour un jeton vide.
pub fn champ(j: &Value, cle: &str) -> R<Value> {
    if !vrai(j) {
        return Ok(Value::Null);
    }
    match j {
        Value::Object(o) => Ok(o.get(cle).cloned().unwrap_or(Value::Null)),
        autre => Err(erreur("AttributeError", socle::fichiers::attendu("un objet", nom_type(autre)))),
    }
}

/// Le texte d'une valeur qu'un formulaire recoit.
pub fn en_texte(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        autre => socle::valeur::valeur_en_texte(autre),
    }
}

/// Un formulaire `application/x-www-form-urlencoded` : cles et valeurs encodees, l'espace en `+`.
pub fn encoder(champs: &[(&str, String)]) -> String {
    let q = |s: &str| -> String {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-' | b'~' => (b as char).to_string(),
                b' ' => "+".into(),
                _ => format!("%{:02X}", b),
            })
            .collect()
    };
    champs.iter().map(|(k, v)| format!("{}={}", q(k), q(v))).collect::<Vec<_>>().join("&")
}

/// Le message lisible d'une reponse d'erreur de Google.
pub fn traduire(code: u16, corps: &str) -> String {
    let mut detail: String = corps.chars().take(500).collect();
    let mut code_g = String::new();
    if let Ok(Value::Object(o)) = serde_json::from_str::<Value>(corps) {
        match o.get("error") {
            Some(Value::String(s)) => {
                code_g = s.clone();
                match o.get("error_description") {
                    Some(d) if vrai(d) => detail = en_texte(d),
                    _ => {}
                }
            }
            Some(Value::Object(e)) => {
                code_g = e.get("status").map(en_texte).unwrap_or_default();
                match e.get("message") {
                    Some(m) if vrai(m) => detail = en_texte(m),
                    _ => {}
                }
            }
            _ => {}
        }
    }
    if code_g == "invalid_grant" || corps.contains("invalid_grant") {
        return "Autorisation Google perdue ou expiree.\n\
                \x20 Cause la plus frequente : l'ecran de consentement est reste\n\
                \x20 en mode « Test », ou Google revoque le jeton au bout de 7\n\
                \x20 jours. Le passer « En production » dans la console, puis\n\
                \x20 relier le Drive (Configuration > Mots de passe, ou\n\
                \x20 `paie google autoriser`)."
            .into();
    }
    if code == 403 && corps.to_lowercase().contains("insufficient") {
        return "Permission refusee : l'autorisation en cours ne couvre pas\n\
                \x20 cette action. Relier le Drive (Configuration > Mots de\n\
                \x20 passe, ou `paie google autoriser`) pour\n\
                \x20 redemander les permissions."
            .into();
    }
    format!("HTTP {} : {}", code, detail)
}

/// Une panne du transport : `Ok(phrase)` pour un statut de Google (traduit) ou un reseau injoignable,
/// `Err((classe, message))` pour une panne brute (delai depasse, connexion fermee...).
pub fn phrase_de_panne(p: Panne) -> Result<String, (&'static str, String)> {
    match p {
        Panne::Statut { code, corps } => {
            // Le transport demande `gzip` : une reponse d'erreur peut arriver compressee.
            let corps = corps
                .ok()
                .map(|o| if o.starts_with(&[0x1f, 0x8b]) { crate::transport::gunzip(&o).unwrap_or(o) } else { o })
                .map(|o| String::from_utf8_lossy(&o).into_owned())
                .unwrap_or_default();
            Ok(traduire(code, &corps))
        }
        Panne::Reseau(raison) => Ok(format!("Reseau injoignable : {}", raison)),
        Panne::Brute { classe, message, .. } => Err((classe, message)),
    }
}

/// Le jeton d'acces que Google rend pour un jeton de rafraichissement : `Ok(Ok(jeton))`, `Ok(Err(phrase))` pour un refus
/// de Google, `Err` pour une panne brute. `adresse` : `JETONS`, ou un serveur local.
pub fn jeton_par_rafraichissement(
    adresse: &str,
    client_id: &Value,
    client_secret: &Value,
    rafraichissement: &Value,
    delai: u64,
) -> R<Result<String, String>> {
    let corps = encoder(&[
        ("client_id", en_texte(client_id)),
        ("client_secret", en_texte(client_secret)),
        ("refresh_token", en_texte(rafraichissement)),
        ("grant_type", "refresh_token".into()),
    ]);
    let requete = Requete {
        url: adresse,
        jeton: None,
        donnees: Some(corps.as_bytes()),
        content_type: Some("application/x-www-form-urlencoded"),
        corps_gzip: false,
        post: true,
        delai,
        ..Default::default()
    };
    match envoyer(&requete) {
        Ok((brut, _)) => {
            // Une reponse 200 sans jeton d'acces, ou illisible, est un refus de Google comme un autre : une phrase.
            let Some(v) = String::from_utf8(brut).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) else {
                return Ok(Err("Google a rendu une reponse illisible a la demande de jeton. Reessayer plus tard.".into()));
            };
            match &v {
                Value::Object(o) if o.get("access_token").is_some_and(|t| t.is_string() && vrai(t)) => Ok(Ok(en_texte(&o["access_token"]))),
                _ => Ok(Err("Google n'a pas rendu de jeton d'acces (reponse sans `access_token`). Reessayer plus tard ; si cela dure, lier de nouveau le compte.".into())),
            }
        }
        Err(panne) => match phrase_de_panne(panne) {
            Ok(phrase) => Ok(Err(phrase)),
            Err((classe, message)) => Err(erreur(classe, message)),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn urlencode_comme_python() {
        assert_eq!(encoder(&[("a b", "x/y+z=é".into()), ("k", "~-._".into())]), "a+b=x%2Fy%2Bz%3D%C3%A9&k=~-._");
    }

    /// Un faux Google local (127.0.0.1) : repond 200 avec `corps` a la premiere demande, puis s'arrete.
    fn faux_google(corps: &'static str) -> String {
        use std::io::{Read, Write};
        let ecoute = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = ecoute.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = ecoute.accept() {
                let mut tete = Vec::new();
                let mut octet = [0u8; 1];
                while !tete.ends_with(b"\r\n\r\n") {
                    match s.read(&mut octet) {
                        Ok(1) => tete.push(octet[0]),
                        _ => break,
                    }
                }
                // Le corps de la demande (le jeton de rafraichissement) n'est ni lu ni garde.
                let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", corps.len(), corps).as_bytes());
            }
        });
        format!("http://127.0.0.1:{}/token", port)
    }

    #[test]
    fn google_200_sans_jeton_d_acces_est_une_phrase_pas_un_plantage() {
        for corps in [r#"{"expires_in": 3599}"#, r#"{"access_token": ""}"#, "[]", "ceci n'est pas du json", ""] {
            let adresse = faux_google(corps);
            let r = jeton_par_rafraichissement(&adresse, &json!("id"), &json!("secret"), &json!("rafraichissement"), 5).expect("pas de plantage");
            let phrase = r.expect_err("pas de jeton");
            assert!(phrase.contains("Google") && phrase.contains("Reessayer"), "{}", phrase);
        }
        let ok = faux_google(r#"{"access_token": "JETON-FICTIF", "expires_in": 3599}"#);
        assert_eq!(jeton_par_rafraichissement(&ok, &json!("id"), &json!("secret"), &json!("rf"), 5).unwrap().unwrap(), "JETON-FICTIF");
    }

    #[test]
    fn traduire_les_deux_formes() {
        assert!(traduire(400, r#"{"error": "invalid_grant", "error_description": "Bad Request"}"#).starts_with("Autorisation Google perdue"));
        assert!(traduire(403, r#"{"error": {"status": "PERMISSION_DENIED", "message": "Insufficient scope"}}"#).starts_with("Permission refusee"));
        assert_eq!(traduire(401, r#"{"error": "invalid_client", "error_description": "The OAuth client was not found."}"#), "HTTP 401 : The OAuth client was not found.");
        assert_eq!(traduire(500, "pas du json"), "HTTP 500 : pas du json");
    }
}
