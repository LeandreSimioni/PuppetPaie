//! Le harnais de la preuve du lot H3b (les six routes d'envoi) : appelle `routes_envois::route` ou `route_avec` depuis
//! un script de comparaison avec l'ancien Python. Ce n'est PAS un test et rien d'ici n'est dans le binaire.
//!
//!     h3b_preuve RACINE JOUR ROUTE CHARGE.json SORTIE/ [normal | faux [ECHEC_AU_N]]
//!
//! `normal` : la boite que `_client_courriel` choisit (simulee dans une compagnie fictive, sinon aucune). `faux` : un faux client
//! qui ecrit chaque message recu dans SORTIE/NN.eml (octets exacts), et refuse le message numero ECHEC_AU_N (depuis 0).
//! Ecrit sur la sortie standard `{"ok":true,"resultat":...}` ou `{"ok":false,"classe":...,"erreur":...}`.
//! Aucun reseau : le vrai `Client` n'est jamais ouvert sans mot de passe d'application au coffre.

use std::collections::HashMap;
use std::path::PathBuf;

use droit::contexte::Contexte;
use paie::courriel::{Courriel, ErreurCourriel};
use socle::date::Jour;
use paie::routes_envois;
use serde_json::{json, Value};

struct Faux {
    sortie: PathBuf,
    n: usize,
    echec: Option<usize>,
}

impl Courriel for Faux {
    fn compte(&self) -> &str {
        "boite@exemple.invalid"
    }
    fn envoyer(&mut self, m: &[u8]) -> Result<String, ErreurCourriel> {
        if self.echec == Some(self.n) {
            return Err(ErreurCourriel("le serveur refuse ce message".into()));
        }
        std::fs::write(self.sortie.join(format!("{:02}.eml", self.n)), m).unwrap();
        self.n += 1;
        Ok(format!("<faux-{}@exemple.invalid>", self.n - 1))
    }
    fn rechercher(&mut self, _: &str, _: usize) -> Result<Vec<String>, ErreurCourriel> {
        Ok(Vec::new())
    }
    fn message_brut(&mut self, _: &str) -> Result<Vec<u8>, ErreurCourriel> {
        Ok(Vec::new())
    }
    fn fermer(&mut self) {}
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (racine, jour, nom, charge, sortie) = (&a[1], &a[2], &a[3], &a[4], PathBuf::from(&a[5]));
    let mode = a.get(6).map(String::as_str).unwrap_or("normal");
    let echec = a.get(7).and_then(|s| s.parse::<usize>().ok());
    std::fs::create_dir_all(&sortie).unwrap();
    let charge: Value = serde_json::from_slice(&std::fs::read(charge).unwrap()).unwrap();
    let mut o = HashMap::new();
    o.insert("racine".to_string(), racine.clone());
    o.insert("date".to_string(), jour.clone());
    let ctx = Contexte::nouveau(std::path::Path::new(racine), Some(Jour::lire(jour).unwrap())).unwrap();
    let r = if mode == "faux" {
        let faux = std::cell::RefCell::new(Some(Faux { sortie: sortie.clone(), n: 0, echec }));
        routes_envois::route_avec(&o, &ctx, nom, &charge, &mut || Ok(Box::new(faux.borrow_mut().take().unwrap())))
    } else {
        routes_envois::route(&o, &ctx, nom, &charge)
    };
    let sortie = match r {
        Ok(v) => json!({"ok": true, "resultat": v}),
        Err(e) => json!({"ok": false, "classe": e.classe.unwrap_or(""), "erreur": e.message}),
    };
    println!("{}", serde_json::to_string(&sortie).unwrap());
}
