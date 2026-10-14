//! Le harnais de la preuve du lot H4b (le client Drive, la sauvegarde, l'archivage) : appelle les fonctions depuis un
//! script de comparaison avec l'ancien Python. Ce n'est PAS un test et rien d'ici n'est dans le binaire.
//!
//!     h4b_preuve RACINE OP ARGS.json BASE_URL [DELAI_SECONDES]
//!
//! OP : `sauvegarder` {racine_drive, tout}, `archiver` {mois, racine_drive}, `rien` {racine_drive}, `derniere`,
//! `hors_ligne` {textes}, `defaut` et `archiver-defaut` (`client = None` : le vrai client, sans adresse de remplacement : lances QUE sur une
//! copie de developpement SANS jeton, qui refuse avant tout reseau).
//! BASE_URL : le faux Drive local (`http://127.0.0.1:port`) ; le client est `Client::nouveau` (le vrai jeton scelle de la
//! copie, fabrique par le harnais) pointe dessus par `avec_adresses`. AUCUN APPEL A GOOGLE.
//! Ecrit sur la sortie standard `{"ok":true,"resultat":...}` ou `{"ok":false,"classe":...,"erreur":...}`.

use std::path::Path;

use paie::archivage::archiver;
use paie::drive::{Adresses, Client, ErreurGoogle};
use paie::sauvegarde::{derniere, hors_ligne, rien_n_a_bouge, sauvegarder, Sauvegarde, RACINE_DRIVE};
use serde_json::{json, Value};

fn client(racine: &Path, base: &str, delai: Option<u64>) -> Result<Client, ErreurGoogle> {
    assert!(base.starts_with("http://127.0.0.1:"), "le faux Drive est local");
    let c = Client::nouveau(racine)?.avec_adresses(Adresses {
        jetons: format!("{}/token", base),
        fichiers: format!("{}/drive/v3/files", base),
        televersement: format!("{}/upload/drive/v3/files", base),
    });
    Ok(match delai {
        Some(d) => c.avec_delai(d),
        None => c,
    })
}

fn sauvegarde_json(r: &Sauvegarde) -> Value {
    json!({
        "racine_drive": r.racine_drive,
        "compte": r.compte,
        "deposes": r.deposes.iter().map(|f| json!({"relatif": f.relatif, "octets": f.octets, "etat": f.etat.texte()})).collect::<Vec<_>>(),
        "identiques": r.identiques,
        "octets_deposes": r.octets_deposes,
        "exclus": r.exclus,
        "disparus": r.disparus,
        "incidents": r.incidents,
        "hors_ligne": r.hors_ligne,
        "resume": r.resume(),
        "a_change": r.a_change(),
        "en_json": r.en_json(),
    })
}

fn echec(e: ErreurGoogle) -> Value {
    json!({"ok": false, "classe": e.classe, "erreur": e.message})
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (racine, op, args, base) = (Path::new(&a[1]), a[2].as_str(), &a[3], &a[4]);
    let delai = a.get(5).and_then(|d| d.parse::<f64>().ok()).map(|d| d.ceil() as u64);
    let args: Value = serde_json::from_slice(&std::fs::read(args).unwrap()).unwrap();
    let racine_drive = args.get("racine_drive").and_then(Value::as_str).unwrap_or(RACINE_DRIVE).to_string();
    let tout = args.get("tout").and_then(Value::as_bool).unwrap_or(false);
    let sortie = match op {
        "sauvegarder" => match client(racine, base, delai).and_then(|mut c| sauvegarder(racine, &racine_drive, tout, Some(&mut c))) {
            Ok(r) => json!({"ok": true, "resultat": sauvegarde_json(&r)}),
            Err(e) => echec(e),
        },
        "defaut" => match sauvegarder(racine, &racine_drive, tout, None) {
            Ok(r) => json!({"ok": true, "resultat": sauvegarde_json(&r)}),
            Err(e) => echec(e),
        },
        "archiver" | "archiver-defaut" => {
            let mois = args["mois"].as_str().unwrap_or("");
            let rendu = if op == "archiver-defaut" {
                archiver(racine, mois, &racine_drive, None)
            } else {
                client(racine, base, delai).and_then(|mut c| archiver(racine, mois, &racine_drive, Some(&mut c)))
            };
            match rendu {
                Ok(r) => json!({"ok": true, "resultat": {
                    "mois": r.mois, "racine_drive": r.racine_drive, "compte": r.compte,
                    "fichiers": r.fichiers.iter().map(|f| json!({"nom": f.nom, "octets": f.octets, "remplace": f.remplace})).collect::<Vec<_>>(),
                }}),
                Err(e) => echec(e),
            }
        }
        "rien" => json!({"ok": true, "resultat": {"rien_n_a_bouge": rien_n_a_bouge(racine, &racine_drive)}}),
        "derniere" => json!({"ok": true, "resultat": derniere(racine).en_json()}),
        "hors_ligne" => {
            let textes: Vec<Value> = args["textes"].as_array().unwrap().iter().map(|t| json!(hors_ligne(t.as_str().unwrap()))).collect();
            json!({"ok": true, "resultat": textes})
        }
        _ => panic!("op ?"),
    };
    println!("{}", serde_json::to_string(&sortie).unwrap());
}
