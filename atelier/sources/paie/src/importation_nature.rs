//! LA NATURE DES CONTRATS QUE LES BULLETINS N'ONT PAS DITE (#236). L'import d'un bulletin de paie atteste un contrat
//! (`historique/engagements.json`, `importation_bulletins::engagement`) ; aucun gabarit n'imprime sa nature. Un bulletin
//! qui porte une marque du spectacle (numero d'objet, cotisation Congés Spectacles) laisse le contrat muet, et un
//! engagement muet est un CDDU (regle 6). Un bulletin qui n'en porte aucune est celui d'un permanent : CDI ou CDD de
//! droit commun, que rien ne dit. Ses contrats sont marques `nature_a_dire` ; ce module :
//!
//! - `a_dire` : les salaries dont des contrats repris attendent leur nature, avec les natures qu'on peut leur donner
//!   (celles d'un permanent : `contrats-base.json`, paie « mensuelle ») -- `importation-etat` les rend, la page les
//!   demande apres l'import (Configuration, « Reprendre l'historique ») ;
//! - la route `importation-nature` `{"salarie": <cle>, "nature": <cle de nature>}` : la nature dite s'ecrit sur chacun
//!   de ses contrats a dire, et la marque s'efface. Une nature qui n'est pas d'un permanent, un salarie sans contrat a
//!   dire : refus, rien n'est ecrit.
//!
//! Un salarie, une reponse : les bulletins d'un permanent attestent un contrat par mois, tous du meme contrat.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::contexte::Contexte;
use crate::etats::{erreur, py_str, R};
use crate::fichiers::vrai;

/// La marque d'un contrat repris dont la nature reste a dire.
pub const A_DIRE: &str = "nature_a_dire";

/// La route de ce module.
pub const ROUTE: &str = "importation-nature";

fn chemin(racine: &Path) -> std::path::PathBuf {
    racine.join("historique").join("engagements.json")
}

/// Les natures qu'un permanent peut recevoir : (cle, libelle).
fn natures_permanentes() -> R<Vec<(String, String)>> {
    let toutes = crate::contrats::natures().map_err(|m| erreur("ErreurOperation", m))?;
    Ok(toutes.iter().filter(|n| n.paie == "mensuelle").map(|n| (n.cle.clone(), n.libelle.clone())).collect())
}

/// Le nom d'un salarie, tel que la fiche le montre ; sa cle a defaut.
fn nom_du_salarie(racine: &Path, cle: &str) -> String {
    let fiches = crate::routes_etats::lire_json_ou_vide(&racine.join("config").join("salaries.json")).unwrap_or(Value::Null);
    let Some(f) = fiches.get(cle).filter(|f| f.is_object()) else { return cle.to_string() };
    let t = |k: &str| f.get(k).filter(|v| vrai(v)).map(py_str).unwrap_or_default();
    let prenom = if t("prenom_affichage").is_empty() { t("prenom") } else { t("prenom_affichage") };
    let nom = if t("nom_affichage").is_empty() { t("nom") } else { t("nom_affichage") };
    let n = format!("{} {}", prenom, nom).trim().to_string();
    if n.is_empty() {
        cle.to_string()
    } else {
        n
    }
}

/// Les salaries dont des contrats repris attendent leur nature : `[{salarie, nom, contrats, du, au, natures}]`, dans
/// l'ordre des cles. Ne leve jamais : un historique illisible n'a rien a dire.
pub(crate) fn a_dire(racine: &Path) -> Vec<Value> {
    let Ok(h) = crate::routes_etats::lire_json_ou_vide(&chemin(racine)) else { return Vec::new() };
    let mut par: BTreeMap<String, (usize, String, String)> = BTreeMap::new();
    for e in h.get("engagements").and_then(Value::as_array).into_iter().flatten() {
        if !e.get(A_DIRE).is_some_and(vrai) {
            continue;
        }
        let Some(s) = e.get("salarie").and_then(Value::as_str) else { continue };
        let (debut, fin) = (e.get("debut").map(py_str).unwrap_or_default(), e.get("fin").map(py_str).unwrap_or_default());
        let x = par.entry(s.to_string()).or_insert((0, debut.clone(), fin.clone()));
        x.0 += 1;
        if debut < x.1 {
            x.1 = debut;
        }
        if fin > x.2 {
            x.2 = fin;
        }
    }
    if par.is_empty() {
        return Vec::new();
    }
    let natures: Vec<Value> = natures_permanentes().unwrap_or_default().into_iter().map(|(c, l)| json!({"cle": c, "libelle": l})).collect();
    par.into_iter()
        .map(|(s, (n, du, au))| {
            json!({"salarie": s, "nom": nom_du_salarie(racine, &s), "contrats": n, "du": du, "au": au, "natures": natures})
        })
        .collect()
}

/// `importation-nature` : la nature dite pour les contrats a dire d'un salarie. Rend `{"ecrits": n, "a_dire": [...]}`.
pub(crate) fn dire(ctx: &Contexte, charge: &Value) -> R<Value> {
    let texte = |k: &str| charge.get(k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let salarie = texte("salarie").ok_or_else(|| erreur("ErreurOperation", "Le salarié manque : rien n'a été écrit."))?;
    let nature = texte("nature").ok_or_else(|| erreur("ErreurOperation", "La nature du contrat manque : rien n'a été écrit."))?;
    let permises = natures_permanentes()?;
    if !permises.iter().any(|(c, _)| *c == nature) {
        let noms: Vec<&str> = permises.iter().map(|(_, l)| l.as_str()).collect();
        return Err(erreur(
            "ErreurOperation",
            format!("« {} » n'est pas la nature d'un contrat de permanent ({}) : rien n'a été écrit.", nature, noms.join(", ")),
        ));
    }
    let p = chemin(&ctx.racine);
    let mut h = crate::routes_etats::lire_json_ou_vide(&p)?;
    let mut ecrits = 0usize;
    if let Some(Value::Array(liste)) = h.get_mut("engagements") {
        for e in liste.iter_mut() {
            let Some(o) = e.as_object_mut() else { continue };
            if !o.get(A_DIRE).is_some_and(vrai) || o.get("salarie").and_then(Value::as_str) != Some(salarie.as_str()) {
                continue;
            }
            o.remove(A_DIRE);
            o.insert("nature".into(), json!(nature));
            ecrits += 1;
        }
    }
    if ecrits == 0 {
        return Err(erreur(
            "ErreurOperation",
            format!("Aucun contrat repris de {} n'attend sa nature : rien n'a été écrit.", nom_du_salarie(&ctx.racine, &salarie)),
        ));
    }
    crate::routes_contrats::sauver_avant(&p)?;
    crate::ecriture::ecrire_json(&p, &h, 1).map_err(|m| erreur("OSError", m))?;
    Ok(json!({"ecrits": ecrits, "a_dire": a_dire(&ctx.racine)}))
}

/// La nature d'un contrat repris, pour qui la montre (le livre de paie d'un mois d'avant) : la sienne, « a dire » s'il
/// l'attend, `None` s'il est muet (un CDDU, regle 6).
pub(crate) fn libelle_d_un_contrat(e: &Map<String, Value>) -> R<Option<String>> {
    if e.get(A_DIRE).is_some_and(vrai) {
        return Ok(Some("nature à dire".into()));
    }
    match e.get("nature").and_then(Value::as_str).filter(|s| !s.is_empty()) {
        Some(n) => Ok(Some(crate::contrats::nature(Some(n)).map_err(|m| erreur("ErreurOperation", m))?.libelle.clone())),
        None => Ok(None),
    }
}
