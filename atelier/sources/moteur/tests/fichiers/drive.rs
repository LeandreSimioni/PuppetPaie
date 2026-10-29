//! Le client Drive de la sauvegarde contre un faux serveur sur `127.0.0.1` : un delai de lecture n'est pas une absence
//! de reseau, rien ne part sans compte lie ni d'une copie de developpement (regle 11), et le trait `Drive` ne sait rien
//! supprimer. Le jeton renouvele apres un 401 (cle du poste dans l'environnement) : `tests/drive_jeton.rs`. Aucune
//! connexion vers l'exterieur ; `google.json` est fictif.

use std::path::{Path, PathBuf};
use std::time::Duration;

use envois::drive::{Adresses, Client, Drive};
use moteur::compagnie::sauvegarde::hors_ligne;

use crate::outils::{http, Recu, Requete};
use serde_json::json;

fn json_ok(v: serde_json::Value) -> Vec<u8> {
    http(200, "application/json", v.to_string().as_bytes())
}

/// Le faux serveur d'`outils`, qui repond toujours.
fn serveur(f: impl Fn(&Requete) -> Vec<u8> + Send + Sync + 'static) -> (String, Recu) {
    crate::outils::serveur(move |q| Some(f(q)))
}

fn adresses(base: &str) -> Adresses {
    Adresses {
        jetons: format!("{}/token", base),
        fichiers: format!("{}/drive/v3/files", base),
        televersement: format!("{}/upload/drive/v3/files", base),
    }
}

fn racine(nom: &str) -> PathBuf {
    let r = crate::outils::dossier_de_test("drive", nom);
    std::fs::create_dir_all(r.join("config")).unwrap();
    r
}

// ------------------------------------------------------------ un delai n'est pas « hors ligne »

#[test]
fn defaut_2_un_delai_de_lecture_est_un_incident_pas_une_absence_de_reseau() {
    // Le serveur accepte, lit la requete, puis ne repond qu'apres le delai du client.
    let (base, _) = serveur(|_| {
        std::thread::sleep(Duration::from_millis(2500));
        json_ok(json!({}))
    });
    let mut c = Client::pour_essai(adresses(&base), "jeton", None).avec_delai(1);
    let debut = std::time::Instant::now();
    let e = c.lister("P").unwrap_err();
    assert!(debut.elapsed() < Duration::from_millis(2400), "le delai de 1 s doit couper l'appel");
    assert!(!hors_ligne(&e.message), "un delai de lecture ne doit pas arreter toute la sauvegarde : {}", e.message);
}

// ------------------------------------------------------------ compte et copie de developpement

#[test]
fn compte_pas_lie_est_refuse_avant_tout_reseau() {
    let (base, recu) = serveur(|_| json_ok(json!({})));
    let r = racine("pas-lie");
    std::fs::write(r.join("config").join("google.json"), r#"{"client_id": "i", "client_secret": "s"}"#).unwrap();
    let mut c = moteur::routes::coffre::client_drive(&r).unwrap().avec_adresses(adresses(&base));
    assert_eq!(c.compte(), None);
    assert!(c.lister("P").is_err());
    assert!(recu.lock().unwrap().is_empty());
}

#[test]
fn une_copie_de_developpement_est_refusee_a_la_construction() {
    let r = racine("copie");
    std::fs::write(r.join("config").join("google.json"), r#"{"client_id": "i", "client_secret": "s"}"#).unwrap();
    std::fs::write(r.join("COPIE-DE-DEVELOPPEMENT.json"), "{}").unwrap();
    assert!(moteur::routes::coffre::client_drive(&r).is_err());
}

// ------------------------------------------------------------ « ne supprime jamais »

/// Le trait `Drive` ne porte que six methodes (deux de lecture pour la restauration), et aucune ne supprime : si
/// quelqu'un en ajoute une (`supprimer`, `effacer`, `corbeille`...), ce test echoue et force a le dire. La garantie « ne
/// supprime jamais rien dans le Drive » est dans le TYPE : la sauvegarde, l'archivage et la restauration ne recoivent qu'un
/// `&mut dyn Drive`.
#[test]
fn le_trait_drive_n_a_pas_de_suppression() {
    let source = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../envois/src/drive.rs")).unwrap();
    let debut = source.find("pub trait Drive {").expect("le trait Drive");
    let corps = &source[debut..];
    let fin = corps.find("\n}\n").expect("fin du trait");
    let trait_ = &corps[..fin];
    let methodes: Vec<&str> = trait_
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with("//"))
        .filter_map(|l| l.strip_prefix("fn "))
        .map(|l| l.split(|c: char| c == '(' || c == '<').next().unwrap())
        .collect();
    assert_eq!(methodes, ["compte", "chemin", "lister", "televerser", "sous_dossiers", "telecharger"], "le trait Drive a change : {:?}", methodes);
    for m in &methodes {
        for mot in ["suppr", "supprim", "effac", "delete", "remove", "corbeille", "trash", "detruire"] {
            assert!(!m.to_lowercase().contains(mot), "une methode de suppression dans Drive : {}", m);
        }
    }
    // Et le client n'appelle jamais DELETE ni ne met a la corbeille.
    let client = &source[..debut + corps.len()];
    assert!(!client.contains("\"DELETE\"") && !client.contains("trashed\": true") && !client.contains("\"trashed\":true"));
}
