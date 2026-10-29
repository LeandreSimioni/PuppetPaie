//! Le jeton Google renouvele apres un 401, sans boucle (defaut 1), et le jeton revoque : rien ne part vers Drive.
//! Le jeton se scelle et se lit par la cle du poste, que le moteur ne prend que dans l'environnement (`PUPPETPAIE_CLE`),
//! d'ou ce fichier seul. Cle et jeton fictifs, dans `CARGO_TARGET_TMPDIR` ; un faux serveur sur `127.0.0.1`.

#[path = "outils/mod.rs"]
mod outils;

use std::path::PathBuf;
use std::sync::Once;

use moteur::compagnie::coffre::Coffre;
use envois::drive::{Adresses, Drive};

use outils::{http, Recu, Requete};
use serde_json::json;

fn reponse(code: u16, corps: &[u8]) -> Vec<u8> {
    http(code, "application/json", corps)
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

// ------------------------------------------------------------ le jeton renouvele

#[test]
fn jeton_revoque_au_rafraichissement_rien_ne_part_vers_drive() {
    let (base, recu) = serveur(|_| reponse(400, br#"{"error": "invalid_grant", "error_description": "Token has been expired or revoked."}"#));
    let r = compagnie_liee("revoque");
    let mut c = moteur::routes::coffre::client_drive(&r).unwrap().avec_adresses(adresses(&base));
    assert!(c.lister("P").is_err());
    assert_eq!(recu.lock().unwrap().len(), 1);
}
