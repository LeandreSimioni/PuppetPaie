//! Le jeton Google renouvele apres un 401, sans boucle (defaut 1), et le jeton revoque : rien ne part vers Drive.
//! Le jeton se scelle et se lit par la cle du poste, que le moteur ne prend que dans l'environnement (`PUPPETPAIE_CLE`),
//! d'ou ce fichier seul. Cle et jeton fictifs, dans `CARGO_TARGET_TMPDIR` ; un faux serveur sur `127.0.0.1`.

#[path = "outils/mod.rs"]
mod outils;

use std::path::PathBuf;
use std::sync::{Arc, Mutex, Once};

use moteur::coffre::Coffre;
use envois::drive::{Adresses, Drive};

use outils::{http, Recu, Requete};
use serde_json::{json, Value};

fn reponse(code: u16, corps: &[u8]) -> Vec<u8> {
    http(code, "application/json", corps)
}

fn json_ok(v: Value) -> Vec<u8> {
    reponse(200, v.to_string().as_bytes())
}

/// Le faux serveur d'`outils`, qui repond toujours.
fn serveur(f: impl Fn(&Requete) -> Vec<u8> + Send + Sync + 'static) -> (String, Recu) {
    outils::serveur(move |q| Some(f(q)))
}

fn adresses(base: &str) -> Adresses {
    Adresses {
        jetons: format!("{}/token", base),
        fichiers: format!("{}/drive/v3/files", base),
        televersement: format!("{}/upload/drive/v3/files", base),
    }
}

/// La cle du poste des tests, posee une fois, avant tout jeton.
fn racine(nom: &str) -> PathBuf {
    static UNE_FOIS: Once = Once::new();
    UNE_FOIS.call_once(|| std::env::set_var("PUPPETPAIE_CLE", outils::dossier_du_processus("drive-jeton-poste")));
    let r = outils::dossier_de_test("drive_jeton", nom);
    std::fs::create_dir_all(r.join("config")).unwrap();
    r
}

/// Une compagnie avec un compte Google FICTIF : `google.json` inventé, jeton scelle par la cle du poste des tests.
fn compagnie_liee(nom: &str) -> PathBuf {
    let r = racine(nom);
    std::fs::write(r.join("config").join("google.json"), r#"{"client_id": "id-fictif", "client_secret": "secret-fictif"}"#).unwrap();
    let jeton = json!({"rafraichissement": "rafraichissement-fictif", "compte": "compte@exemple.invalid", "permissions": []});
    let scelle = Coffre::du_poste(&r, "").proteger(jeton.to_string().as_bytes(), "moteur de paie - jeton Google").unwrap();
    std::fs::write(r.join("config").join("google-jeton.bin"), scelle).unwrap();
    r
}

// ------------------------------------------------------------ defaut 1 : le jeton renouvele

/// Le serveur de jetons rend `t1`, puis `t2`, `t3`... ; Drive refuse en 401 tout jeton qui n'est pas dans `valides`.
fn serveur_a_jetons(valides: &'static [&'static str]) -> (String, Recu) {
    let compte = Arc::new(Mutex::new(0usize));
    serveur(move |v| {
        if v.chemin == "/token" {
            let mut n = compte.lock().unwrap();
            *n += 1;
            return json_ok(json!({"access_token": format!("t{}", *n), "expires_in": 3600}));
        }
        let porteur = v.entete("authorization").strip_prefix("Bearer ").unwrap_or("").to_string();
        if valides.contains(&porteur.as_str()) {
            json_ok(json!({"files": []}))
        } else {
            reponse(401, br#"{"error": {"status": "UNAUTHENTICATED", "message": "Invalid Credentials"}}"#)
        }
    })
}

#[test]
fn defaut_1_un_401_renouvelle_le_jeton_et_recommence_une_fois() {
    let (base, recu) = serveur_a_jetons(&["t2"]);
    let r = compagnie_liee("jeton-401");
    let mut c = moteur::routes::google::client_drive(&r).unwrap().avec_adresses(adresses(&base));
    assert_eq!(c.compte().as_deref(), Some("compte@exemple.invalid"));
    // Le premier jeton (t1) n'est plus bon : 401, un second rafraichissement (t2), et la requete passe.
    assert!(c.lister("P").unwrap().is_empty());
    {
        let v = recu.lock().unwrap();
        let suite: Vec<(String, Option<String>)> = v.iter().map(|v| (v.chemin.clone(), Some(v.entete("authorization").to_string()).filter(|a| !a.is_empty()))).collect();
        assert_eq!(
            suite,
            [
                ("/token".to_string(), None),
                ("/drive/v3/files".to_string(), Some("Bearer t1".to_string())),
                ("/token".to_string(), None),
                ("/drive/v3/files".to_string(), Some("Bearer t2".to_string())),
            ]
        );
        // Le rafraichissement part avec les identifiants et le jeton de rafraichissement, en formulaire.
        assert_eq!(v[0].methode, "POST");
        assert_eq!(v[0].entete("content-type"), "application/x-www-form-urlencoded");
        assert_eq!(String::from_utf8(v[0].corps.clone()).unwrap(), "client_id=id-fictif&client_secret=secret-fictif&refresh_token=rafraichissement-fictif&grant_type=refresh_token");
    }
    // Le jeton renouvele est gardé : l'appel suivant n'en redemande pas.
    assert!(c.lister("P").unwrap().is_empty());
    let v = recu.lock().unwrap();
    assert_eq!(v.iter().filter(|v| v.chemin == "/token").count(), 2);
    assert_eq!(v.len(), 5);
}

#[test]
fn jeton_revoque_au_rafraichissement_rien_ne_part_vers_drive() {
    let (base, recu) = serveur(|_| reponse(400, br#"{"error": "invalid_grant", "error_description": "Token has been expired or revoked."}"#));
    let r = compagnie_liee("revoque");
    let mut c = moteur::routes::google::client_drive(&r).unwrap().avec_adresses(adresses(&base));
    assert!(c.lister("P").is_err());
    assert_eq!(recu.lock().unwrap().len(), 1);
}
