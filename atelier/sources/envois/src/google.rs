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
    let mut detail: String = corps.to_string();
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
                \x20 `moteur google autoriser`)."
            .into();
    }
    if code == 403 && corps.to_lowercase().contains("insufficient") {
        return "Permission refusee : l'autorisation en cours ne couvre pas\n\
                \x20 cette action. Relier le Drive (Configuration > Mots de\n\
                \x20 passe, ou `moteur google autoriser`) pour\n\
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
