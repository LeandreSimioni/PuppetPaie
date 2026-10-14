//! `paie incidents [--detail] [--quoi NATURE] [--json]` : le journal des incidents, en lecture
//! (`moteur/commandes_incidents.py`). Les routes `incidents-lire` et `incidents-regrouper` font le travail.
//!
//! `--envoyer` (deposer chez l'editeur, route `incidents-envoyer`) et `--oublier` (vider, route `incidents-oublier`) : lot J2b, theme 2.
//! Ni l'un ni l'autre ne demande de mot de passe (le Python non plus) : le journal est deja caviarde par le moteur, `paie route
//! diagnostic-etat` montre exactement ce qui part, et l'ouverture le depose d'elle-meme. Pas de `--json` sur ces deux gestes (le Python n'en a pas).

use serde_json::{json, Value};

use super::analyse::{Args, Opt, Spec};
use super::rendu::{brut, champ, enveloppe, rendre, texte};
use super::Cadre;
use crate::retours::Echec;

pub static SPEC: Spec = Spec {
    commande: "incidents",
    positionnels: &[],
    options: &[
        Opt::drapeau("detail"),
        Opt::choix("quoi", &["dpae", "dsn", "logiciel", "refus"], None),
        Opt::drapeau("envoyer"),
        Opt::drapeau("oublier"),
        Opt::drapeau("json"),
    ],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    match faire(cadre, args) {
        Ok(c) => c,
        Err(e) => cadre.plantage(e),
    }
}

/// `quand_lisible` : `2026-09-12T18:18:08` -> `12/09/2026 à 18:18`.
fn quand_lisible(v: Option<&Value>) -> String {
    let s = v.filter(|x| crate::fichiers::vrai(x)).map(texte).unwrap_or_default();
    let c: Vec<char> = s.chars().collect();
    if c.len() < 10 || c[4] != '-' || c[7] != '-' {
        return s;
    }
    let jour = format!("{}{}/{}{}/{}", c[8], c[9], c[5], c[6], c[..4].iter().collect::<String>());
    if c.len() >= 16 {
        format!("{} à {}", jour, c[11..16].iter().collect::<String>())
    } else {
        jour
    }
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    if args.drapeau("envoyer") {
        return envoyer(cadre);
    }
    if args.drapeau("oublier") {
        return oublier(cadre, args);
    }
    let charge = match args.texte("quoi") {
        Some(q) => json!({"quoi": q}),
        None => json!({}),
    };
    if args.drapeau("detail") {
        let mut lignes: Vec<Value> = cadre.route("incidents-lire", charge)?.get("lignes").and_then(Value::as_array).cloned().unwrap_or_default();
        if args.drapeau("json") {
            rendre(&enveloppe("incidents", brut(&Value::Array(lignes))));
            return Ok(0);
        }
        if lignes.is_empty() {
            dire!("Aucun incident.");
            return Ok(0);
        }
        // Les lignes caviardees d'abord : chacune designe un message d'erreur qui cite une valeur au lieu de nommer un
        // champ, et c'est le message qu'il faut corriger.
        lignes.sort_by_key(|l| !l.get("caviarde").is_some_and(crate::fichiers::vrai));
        for l in &lignes {
            dire!("{}  {:<9} {}", quand_lisible(l.get("quand")), champ(l, "quoi"), champ(l, "ou"));
            if l.get("resume").is_some_and(crate::fichiers::vrai) {
                dire!("    {}", champ(l, "resume"));
            }
            if let Some(c) = l.get("caviarde").filter(|c| crate::fichiers::vrai(c)) {
                let noms: Vec<String> = c.as_array().map(|a| a.iter().map(texte).collect()).unwrap_or_default();
                dire!("    /!\\ caviarde : {} -- le message cite une valeur, il devrait nommer le champ", noms.join(", "));
            }
            let etat = l.get("envoi").and_then(|e| e.get("etat")).filter(|e| crate::fichiers::vrai(e)).map(texte);
            if let Some(e) = etat.filter(|e| e != "sans-objet") {
                dire!("    envoi : {}", e);
            }
        }
        dire!("\n{} incident(s).", lignes.len());
        return Ok(0);
    }

    let paquets: Vec<Value> = cadre.route("incidents-regrouper", charge)?.get("paquets").and_then(Value::as_array).cloned().unwrap_or_default();
    if args.drapeau("json") {
        rendre(&enveloppe("incidents", brut(&Value::Array(paquets))));
        return Ok(0);
    }
    if paquets.is_empty() {
        dire!("Aucun incident. (C'est l'etat normal ; ce journal se remplit quand le logiciel casse ou refuse.)");
        return Ok(0);
    }
    let mut total = 0;
    for p in &paquets {
        let combien = p.get("combien").and_then(Value::as_i64).unwrap_or(0);
        total += combien;
        let ou = match p.get("ou") {
            Some(o) if crate::fichiers::vrai(o) => texte(o),
            _ => "?".to_string(),
        };
        let genre = p.get("genre").filter(|g| crate::fichiers::vrai(g)).map(texte).unwrap_or_default();
        dire!("{:<9} {:<28} {}{}", champ(p, "quoi"), ou, genre, if combien > 1 { format!(" x{}", combien) } else { String::new() });
        if p.get("resume").is_some_and(crate::fichiers::vrai) {
            dire!("    {}", champ(p, "resume"));
        }
        let versions: Vec<String> = p.get("versions").and_then(Value::as_array).map(|a| a.iter().map(texte).collect()).unwrap_or_default();
        dire!("    du {} au {}, version(s) {}", quand_lisible(p.get("premiere")), quand_lisible(p.get("derniere")), versions.join(", "));
        if let Some(c) = p.get("caviarde").filter(|c| crate::fichiers::vrai(c)) {
            let noms: Vec<String> = c.as_array().map(|a| a.iter().map(texte).collect()).unwrap_or_default();
            dire!("    /!\\ caviarde : {}", noms.join(", "));
        }
    }
    dire!("\n{} sorte(s) d'incident, {} au total.", paquets.len(), total);
    Ok(0)
}

/// `--envoyer` : deposer chez l'editeur ce qui attend (`incidents-envoyer`). Le moteur ne leve jamais : un bilan.
fn envoyer(cadre: &Cadre) -> Result<i32, Echec> {
    let b = cadre.route("incidents-envoyer", json!({}))?;
    let reste = b.get("reste").and_then(Value::as_i64).unwrap_or(0);
    if b.get("sans_serveur").is_some_and(crate::fichiers::vrai) {
        dire!("Aucun serveur configure (\"serveur\" dans config/filet-dpae.json) : {} incident(s) restent en attente.", reste);
        return Ok(1);
    }
    dire!("{} incident(s) depose(s) chez l'editeur, {} en attente.", b.get("envoyes").and_then(Value::as_i64).unwrap_or(0), reste);
    if b.get("erreur").is_some_and(crate::fichiers::vrai) {
        dire!("Interrompu : {}", champ(&b, "erreur"));
        return Ok(1);
    }
    Ok(0)
}

/// `--oublier [--quoi NATURE]` : vider le journal, en entier ou d'une nature (`incidents-oublier`).
fn oublier(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let charge = match args.texte("quoi") {
        Some(q) => json!({"quoi": q}),
        None => json!({}),
    };
    let n = cadre.route("incidents-oublier", charge)?.get("retires").and_then(Value::as_i64).unwrap_or(0);
    dire!("{} incident(s) retire(s){}.", n, args.texte("quoi").map(|q| format!(" ({})", q)).unwrap_or_default());
    Ok(0)
}
