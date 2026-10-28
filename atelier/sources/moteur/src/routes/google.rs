//! `google-etat` : l'autorisation Google tient-elle ? (`google.py`,
//! `operations_ouverture.google_etat`).
//!
//! Un jeton present ne prouve rien : Google a pu le revoquer. Seul un
//! rafraichissement reel repond -- et s'il echoue, l'etat le DIT (`probleme`)
//! au lieu de lever : une page de configuration doit s'afficher sans reseau.
//!
//! Le jeton (`config/google-jeton.bin`) est scelle par la cle du poste
//! (`coffre::Coffre::deproteger`, description « moteur de paie - jeton
//! Google ») ; rien de ce qu'il porte ne sort d'ici, hormis l'adresse du compte
//! et les permissions accordees. Le jeton d'acces obtenu n'est ni rendu ni garde.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use socle::valeur::nom_type;
use socle::echec::erreur;
use socle::fichiers::vrai;
use socle::echec::Echec;
use envois::google::{champ, en_texte, jeton_par_rafraichissement, DELAI, PERMISSIONS};

type R<T> = Result<T, Echec>;

/// Les routes que ce module sert.
pub const ROUTES: [&str; 1] = ["google-etat"];

/// `google.APPLICATION` : l'application Google du logiciel, livree avec lui
/// dans `google-application.json` (a cote du code), `client_id` et
/// `client_secret`. Le secret ne s'ecrit pas dans le source Rust.
const FICHIER_APPLICATION: &str = "google-application.json";

/// `google.FICHIER_IDENTIFIANTS`, `google.FICHIER_JETON`.
const FICHIER_IDENTIFIANTS: &str = "google.json";
pub(crate) const FICHIER_JETON: &str = "google-jeton.bin";

/// La description scellee avec le jeton (`google._dpapi`).
pub(crate) const DESCRIPTION: &str = "moteur de paie - jeton Google";

/// `compagnies.MARQUEUR`.
use socle::fichiers::MARQUEUR_COPIE;

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, _charge: &Value) -> R<Value> {
    match nom {
        "google-etat" => google_etat(ctx),
        autre => Err(Echec { classe: None, message: format!("route inconnue : {}", autre) }),
    }
}

fn config(racine: &Path) -> PathBuf {
    racine.join("config")
}

/// `ErreurGoogle` : la phrase est ecrite pour etre lue.
#[track_caller]
fn erreur_google(message: impl Into<String>) -> Echec {
    erreur("ErreurGoogle", message)
}

/// `google.identifiants` : (client_id, client_secret, origine).
pub(crate) fn identifiants(racine: &Path) -> R<(Value, Value, String)> {
    let p = config(racine).join(FICHIER_IDENTIFIANTS);
    if p.exists() {
        // Un `google.json` illisible est un refus, pas un plantage (le Python laissait passer une trace : `JSONDecodeError`,
        // `UnicodeDecodeError`, `AttributeError`). Decision de Leandre : un defaut du Python ne se reproduit pas.
        let illisible = |raison: String| erreur_google(format!("{} est illisible : {}.\nLe corriger, ou le supprimer pour revenir a l'application du logiciel.", p.display(), raison));
        let octets = std::fs::read(&p).map_err(|e| illisible(e.to_string()))?;
        let texte = String::from_utf8(octets).map_err(|e| illisible(e.to_string()))?;
        let donnees: Value = serde_json::from_str(&texte).map_err(|e| illisible(e.to_string()))?;
        let Value::Object(d) = &donnees else { return Err(illisible("ce n'est pas un objet JSON".into())) };
        let manque: Vec<&str> = ["client_id", "client_secret"]
            .into_iter()
            .filter(|c| !d.get(*c).map(vrai).unwrap_or(false))
            .collect();
        if !manque.is_empty() {
            return Err(erreur_google(format!(
                "{} incomplet, il manque : {}.\nLe supprimer suffit a revenir a l'application du logiciel.",
                p.display(),
                manque.join(", ")
            )));
        }
        return Ok((d["client_id"].clone(), d["client_secret"].clone(), p.display().to_string()));
    }
    let livre = socle::fichiers::dossier_du_code().join(FICHIER_APPLICATION);
    let application = std::fs::read(&livre)
        .ok()
        .and_then(|o| serde_json::from_slice::<Value>(&o).ok())
        .filter(|a| a.get("client_id").map(vrai).unwrap_or(false));
    let Some(a) = application else {
        return Err(erreur_google(format!(
            "L'application Google du logiciel n'est pas renseignee.\n\n\
             Elle est livree avec chaque version publiee ({}, posee par\n\
             le flux « Publier » depuis le secret PUPPETPAIE_GOOGLE_APPLICATION) ;\n\
             une copie de travail du depot ne l'a pas.\n\n\
             Sinon, on peut utiliser sa propre application en creant\n\
             {} :\n\n\
             \x20 {{\n\
             \x20   \"client_id\": \"....apps.googleusercontent.com\",\n\
             \x20   \"client_secret\": \"....\"\n\
             \x20 }}\n",
            livre.display(),
            p.display()
        )));
    };
    Ok((a["client_id"].clone(), a.get("client_secret").cloned().unwrap_or(Value::Null), "le logiciel".into()))
}

/// `google.autorise` : y a-t-il un jeton utilisable, sans rien demander au reseau. Un jeton illisible, un coffre qui
/// refuse, un jeton sans `rafraichissement` : non. Ne leve jamais (l'ouverture, lot I2b, l'appelle).
pub(crate) fn autorise(racine: &Path) -> bool {
    match lire_jeton(racine) {
        Ok(j) => champ(&j, "rafraichissement").map(|v| vrai(&v)).unwrap_or(false),
        Err(()) => false,
    }
}

/// `google._lire_jeton` : le contenu du jeton, `Null` s'il n'y en a pas.
/// `Err` : ce que `autorise` et ses voisins rattrapent (`ErreurGoogle`,
/// `ValueError`) -- le coffre qui refuse, un JSON illisible.
fn lire_jeton(racine: &Path) -> Result<Value, ()> {
    lire_jeton_dit(racine).map_err(|_| ())
}

/// `google._lire_jeton`, avec la phrase du refus : le coffre qui refuse dit
/// quoi faire (`google._dpapi`), un JSON illisible dit seulement qu'il l'est.
/// Le client Drive (`drive.rs`) la rend telle quelle.
pub(crate) fn lire_jeton_dit(racine: &Path) -> Result<Value, String> {
    let p = config(racine).join(FICHIER_JETON);
    if !p.exists() {
        return Ok(Value::Null);
    }
    // Une lecture impossible (droits) levait une OSError que rien ne
    // rattrapait ; elle se range ici avec les refus du coffre : l'etat se lit.
    let brut = std::fs::read(&p).map_err(|e| format!("{} : {}", p.display(), e))?;
    let clair = crate::coffre::Coffre::du_poste(racine, "").deproteger(&brut, DESCRIPTION).map_err(|e| {
        format!("{} Pour le jeton Google, le remede est : relancer `moteur google autoriser`.", e.message)
    })?;
    let texte = String::from_utf8(clair).map_err(|e| e.to_string())?;
    serde_json::from_str(&texte).map_err(|e| e.to_string())
}

/// `google.compte_autorise` : l'adresse du compte, sans reseau ; `None` si pas autorise.
pub fn compte_autorise(racine: &Path) -> Option<String> {
    let j = lire_jeton_dit(racine).ok()?;
    match champ(&j, "compte").ok()? {
        Value::Null => None,
        Value::String(s) => Some(s),
        autre => Some(en_texte(&autre)),
    }
}

/// Les adresses de Google ; dans l'atelier, un faux Google local (`atelier.rs`).
pub fn adresses_du_poste() -> envois::drive::Adresses {
    #[cfg(feature = "atelier")]
    if let Some(b) = crate::atelier::base(crate::atelier::VARIABLE_GOOGLE) {
        return envois::drive::Adresses::de_la_base(&b);
    }
    envois::drive::Adresses::default()
}

/// LE CLIENT DU DRIVE DE LA COMPAGNIE : refuse une copie de developpement (qui ecrirait dans le Drive de la VRAIE
/// compagnie), lit l'application (`google.json` ou celle du logiciel), le jeton scelle par la cle du poste et l'adresse
/// du compte, et les passe au client (`drive::Client::nouveau`). Aucun appel reseau.
pub fn client_drive(racine: &Path) -> Result<envois::drive::Client, envois::drive::ErreurGoogle> {
    if let Some(refus) = refus_de_copie(racine) {
        return Err(refus.into());
    }
    let (id, secret, _origine) = identifiants(racine)?;
    let autorisation = envois::drive::Autorisation { identifiants: (id, secret), jeton: lire_jeton_dit(racine), compte: compte_autorise(racine) };
    Ok(envois::drive::Client::nouveau(autorisation, adresses_du_poste()))
}

/// LIER LE COMPTE GOOGLE : lit l'application et les adresses du poste, mene l'autorisation
/// (`google_autorisation::autoriser`), puis scelle le jeton obtenu. Rend l'adresse du compte lie.
pub(crate) fn autoriser(racine: &Path, delai: u64, dire_url: &dyn Fn(&str), dire: &dyn Fn(&str)) -> R<String> {
    let (id, secret, _) = identifiants(racine).map_err(|e| Echec { classe: Some("ErreurOperation"), message: e.message })?;
    let adresses = {
        #[cfg(feature = "atelier")]
        if let Some(b) = crate::atelier::base(crate::atelier::VARIABLE_GOOGLE) {
            envois::google_autorisation::Adresses::de_la_base(&b)
        } else {
            envois::google_autorisation::Adresses::default()
        }
        #[cfg(not(feature = "atelier"))]
        envois::google_autorisation::Adresses::default()
    };
    let jeton = envois::google_autorisation::autoriser((id, secret), &adresses, delai, dire_url, dire)?;
    sceller_le_jeton(racine, &jeton)
}

/// Le jeton que l'autorisation a obtenu (`google_autorisation::autoriser`) et l'adresse du compte, dans un seul fichier
/// scelle par la cle du poste. Rend l'adresse du compte lie.
fn sceller_le_jeton(racine: &Path, jeton: &envois::google_autorisation::JetonGoogle) -> R<String> {
    let brut = socle::json_ascii::dumps(
        &serde_json::json!({"rafraichissement": jeton.rafraichissement, "compte": jeton.compte, "permissions": jeton.permissions}),
        None,
        false,
        false,
    );
    let scelle = crate::coffre::Coffre::du_poste(racine, "")
        .proteger(brut.as_bytes(), DESCRIPTION)
        .map_err(|e| erreur_google(format!("Le jeton n'a pas pu etre scelle, rien n'est enregistre : {}", e.message)))?;
    let dossier = config(racine);
    std::fs::create_dir_all(&dossier).map_err(|e| erreur_google(format!("{} : {}", dossier.display(), e)))?;
    let chemin = dossier.join(FICHIER_JETON);
    std::fs::write(&chemin, scelle).map_err(|e| erreur_google(format!("{} : {}", chemin.display(), e)))?;
    Ok(jeton.compte.clone())
}

/// `p.rsplit("/", 1)[-1]` sur chaque permission.
fn courtes<'a>(ps: impl IntoIterator<Item = &'a Value>) -> R<Vec<Value>> {
    ps.into_iter()
        .map(|p| match p {
            Value::String(s) => Ok(json!(s.rsplit('/').next().unwrap_or(s))),
            autre => Err(erreur("AttributeError", socle::fichiers::attendu("un texte", nom_type(autre)))),
        })
        .collect()
}

/// `list(x)` de ce que le jeton enregistre comme permissions.
fn liste(v: &Value) -> R<Vec<Value>> {
    match v {
        Value::Array(a) => Ok(a.clone()),
        Value::String(s) => Ok(s.chars().map(|c| json!(c.to_string())).collect()),
        Value::Object(o) => Ok(o.keys().map(|k| json!(k)).collect()),
        autre => Err(erreur("TypeError", socle::fichiers::attendu("une liste", nom_type(autre)))),
    }
}

/// `compagnies.refuser_si_copie` : une copie de developpement n'ecrit jamais
/// dans le Drive de la vraie compagnie. Le refus, sous la classe du Python.
pub(crate) fn refus_de_copie(racine: &Path) -> Option<Echec> {
    Path::new(racine).join(MARQUEUR_COPIE).is_file().then(|| {
        erreur(
            "CopieDeDeveloppement",
            "Copie de developpement : la sauvegarde ou l'archivage Drive ne part jamais d'ici. Les essais se font en \
             test ; pour le faire en vrai, ouvrir la vraie compagnie.",
        )
    })
}

/// `Client(ctx).jeton_acces()` : `Ok(None)` si Google rend un jeton d'acces,
/// `Ok(Some(probleme))` pour une `ErreurGoogle`, `Err` pour ce que Python
/// laissait passer.
fn rafraichir(ctx: &Contexte, client_id: &Value, client_secret: &Value) -> R<Option<String>> {
    // `compagnies.refuser_si_copie`, a la construction du client.
    if let Some(refus) = refus_de_copie(&ctx.racine) {
        return Err(refus);
    }
    let j = match lire_jeton(&ctx.racine) {
        Ok(j) => j,
        Err(()) => return Ok(Some("le jeton Google ne s'ouvre plus".into())),
    };
    let rafraichissement = champ(&j, "rafraichissement")?;
    if !vrai(&rafraichissement) {
        return Ok(Some(
            "Compte Google pas encore lie : Configuration > Mots de passe > Lier le Drive, ou `moteur google \
             autoriser`."
                .into(),
        ));
    }
    Ok(jeton_par_rafraichissement(&adresses_du_poste().jetons, client_id, client_secret, &rafraichissement, DELAI)?.err())
}

/// `operations_ouverture.google_etat`, rendu par `sortie_json.etat_google`.
fn google_etat(ctx: &Contexte) -> R<Value> {
    // L'application ne sert qu'a rafraichir un jeton : sans Drive lie, son absence (une copie de travail du depot) n'est
    // pas un refus, et l'etat dit seulement « pas lie » (#235 du prive). Un `config/google.json` pose mais illisible ou
    // incomplet en reste un : c'est un fichier de l'utilisateur, a corriger.
    let application = match identifiants(&ctx.racine) {
        Err(e) if config(&ctx.racine).join(FICHIER_IDENTIFIANTS).exists() => return Err(e),
        a => a,
    };
    let origine = application.as_ref().map(|a| json!(a.2)).unwrap_or(Value::Null);
    let permissions = courtes(&PERMISSIONS.map(|p| json!(p)))?;
    let etat = |compte: Value, verifie: bool, probleme: Option<String>, accordees: Vec<Value>| -> Value {
        let mut o = Map::new();
        o.insert("application".into(), origine.clone());
        o.insert("permissions".into(), json!(permissions));
        o.insert("accordees".into(), json!(accordees));
        o.insert("autorise".into(), json!(vrai(&compte)));
        o.insert("compte".into(), compte);
        o.insert("verifie".into(), json!(verifie));
        o.insert("probleme".into(), json!(probleme));
        Value::Object(o)
    };
    // `autorise` : un jeton illisible ou sans rafraichissement, c'est « pas autorise ».
    let jeton = lire_jeton(&ctx.racine);
    let autorise = match &jeton {
        Ok(j) => vrai(&champ(j, "rafraichissement")?),
        Err(()) => false,
    };
    if !autorise {
        return Ok(etat(Value::Null, false, None, Vec::new()));
    }
    let j = jeton.unwrap_or(Value::Null);
    let compte = champ(&j, "compte")?;
    let accordees = match champ(&j, "permissions")? {
        p if vrai(&p) => courtes(&liste(&p)?)?,
        _ => permissions.clone(),
    };
    let (client_id, client_secret, _) = application?;
    match rafraichir(ctx, &client_id, &client_secret)? {
        None => Ok(etat(compte, true, None, accordees)),
        Some(probleme) => Ok(etat(compte, false, Some(probleme), accordees)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_google_json_illisible_est_un_refus_de_google() {
        // Le Python plantait : `JSONDecodeError`, `UnicodeDecodeError`, `AttributeError`.
        let dossier = std::env::temp_dir().join(format!("google-json-illisible-{}", std::process::id()));
        let config = dossier.join("config");
        std::fs::create_dir_all(&config).unwrap();
        for contenu in [&b"pas du json"[..], b"[1, 2]", b"\xff\xfe", b""] {
            std::fs::write(config.join("google.json"), contenu).unwrap();
            let e = identifiants(&dossier).expect_err("illisible");
            assert_eq!(e.classe, Some("ErreurGoogle"));
            assert!(e.message.contains("est illisible"), "{}", e.message);
        }
        let _ = std::fs::remove_dir_all(&dossier);
    }
}
