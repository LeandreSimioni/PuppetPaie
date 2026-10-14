//! Le harnais de la preuve du lot H4a (`coffre-verifier`, `fiche-demander`, `fiches-relever`) : appelle les routes
//! depuis un script de comparaison avec l'ancien Python. Ce n'est PAS un test et rien d'ici n'est dans le binaire.
//!
//!     h4a_preuve RACINE JOUR ROUTE CHARGE.json SORTIE/ [normal | faux [CONFIG.json]]
//!
//! `normal` : la vraie route (boite simulee dans une compagnie fictive, sinon aucune ; `coffre-verifier` : rien n'est jamais
//! ouvert vers l'exterieur par le harnais, les cas ne posent ni mot de passe, ou un declarant incomplet). `faux` : un
//! faux client (courriel) ou un faux `authentifier` / faux `courriel.verifier` (coffre-verifier), selon CONFIG.json :
//!   courriel : {"compte", "echec_envoi": n, "messages": {id: chemin.eml}, "ordre": [id..], "recherche": "tous"|"erreur",
//!               "lecture_erreur": [id..]}
//!   coffre   : {"auth": {"ok": true} | {"classe", "message"}, "boite": {verdict} | {"erreur": "texte"}}
//! Aucun reseau. Ecrit sur la sortie standard `{"ok":true,"resultat":...}` ou `{"ok":false,"classe":...,"erreur":...}`.

use std::collections::HashMap;
use std::path::PathBuf;

use paie::coffre_verifier;
use paie::contexte::Contexte;
use paie::courriel::{Courriel, ErreurCourriel};
use paie::date::Jour;
use paie::releve_retours::Panne;
use paie::routes_fiches_courriel;
use serde_json::{json, Value};

struct Faux {
    sortie: PathBuf,
    n: usize,
    cfg: Value,
}

impl Courriel for Faux {
    fn compte(&self) -> &str {
        self.cfg.get("compte").and_then(Value::as_str).unwrap_or("boite@exemple.invalid")
    }
    fn envoyer(&mut self, m: &[u8]) -> Result<String, ErreurCourriel> {
        if self.cfg.get("echec_envoi").and_then(Value::as_u64) == Some(self.n as u64) {
            return Err(ErreurCourriel("le serveur refuse ce message".into()));
        }
        std::fs::write(self.sortie.join(format!("{:02}.eml", self.n)), m).unwrap();
        self.n += 1;
        Ok(format!("<faux-{}@exemple.invalid>", self.n - 1))
    }
    fn rechercher(&mut self, _: &str, _: usize) -> Result<Vec<String>, ErreurCourriel> {
        if self.cfg.get("recherche").and_then(Value::as_str) == Some("erreur") {
            return Err(ErreurCourriel("la recherche echoue".into()));
        }
        Ok(self.cfg.get("ordre").and_then(Value::as_array).map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default())
    }
    fn message_brut(&mut self, id: &str) -> Result<Vec<u8>, ErreurCourriel> {
        if self.cfg.get("lecture_erreur").and_then(Value::as_array).is_some_and(|a| a.iter().any(|v| v.as_str() == Some(id))) {
            return Err(ErreurCourriel("la lecture echoue".into()));
        }
        let chemin = self.cfg["messages"][id].as_str().unwrap();
        Ok(std::fs::read(chemin).unwrap())
    }
    fn fermer(&mut self) {}
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (racine, jour, nom, charge, sortie) = (&a[1], &a[2], &a[3], &a[4], PathBuf::from(&a[5]));
    let mode = a.get(6).map(String::as_str).unwrap_or("normal");
    let cfg: Value = a.get(7).map(|p| serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap()).unwrap_or_else(|| json!({}));
    std::fs::create_dir_all(&sortie).unwrap();
    let charge: Value = serde_json::from_slice(&std::fs::read(charge).unwrap()).unwrap();
    let mut o = HashMap::new();
    o.insert("racine".to_string(), racine.clone());
    o.insert("date".to_string(), jour.clone());
    let ctx = Contexte::nouveau(std::path::Path::new(racine), Some(Jour::lire(jour).unwrap())).unwrap();
    if !["coffre-verifier", "fiche-demander", "fiches-relever"].contains(&nom.as_str()) {
        // Une autre route (`fiche-oublier-demande`...) : l'aiguillage du binaire, tel quel.
        let entree = json!({"route": nom, "charge": charge}).to_string();
        let sortie = match paie::routes::commande(&o, &entree) {
            Ok(v) => v,
            Err(m) => json!({"ok": false, "classe": "", "erreur": m}),
        };
        println!("{}", serde_json::to_string(&sortie).unwrap());
        return;
    }
    let r = if nom == "coffre-verifier" {
        if mode == "faux" {
            let auth = cfg.get("auth").cloned().unwrap_or(json!({"ok": true}));
            let boite = cfg.get("boite").cloned().unwrap_or(json!({"ok": true, "message": "boite ouverte", "conseil": ""}));
            coffre_verifier::verifier_secret_avec(
                &ctx,
                charge.get("service"),
                &mut |_, _, _| {
                    if auth.get("ok") == Some(&json!(true)) {
                        Ok(())
                    } else {
                        Err(Panne { classe: auth["classe"].as_str().unwrap().to_string(), message: auth["message"].as_str().unwrap().to_string(), code: 0 })
                    }
                },
                &mut |_| match boite.get("erreur").and_then(Value::as_str) {
                    Some(e) => Err(ErreurCourriel(e.to_string())),
                    None => Ok(boite.clone()),
                },
            )
        } else {
            coffre_verifier::route(&ctx, &charge)
        }
    } else if mode == "faux" {
        let mut faux = Faux { sortie: sortie.clone(), n: 0, cfg };
        routes_fiches_courriel::route_avec(&o, &ctx, nom, &charge, Some(&mut faux))
    } else {
        routes_fiches_courriel::route(&o, &ctx, nom, &charge)
    };
    let sortie = match r {
        Ok(v) => json!({"ok": true, "resultat": v}),
        Err(e) => json!({"ok": false, "classe": e.classe.unwrap_or(""), "erreur": e.message}),
    };
    println!("{}", serde_json::to_string(&sortie).unwrap());
}
