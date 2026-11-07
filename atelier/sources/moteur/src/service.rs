//! `moteur route` : la demande JSON sur l'entree standard, la reponse en une ligne de JSON sur la sortie standard (`--texte` : ses phrases) ; un
//! refus : `{"ok": false, "erreur": "..."}`, et le code de sortie 1. Et ce dont les routes ont besoin pour lire la compagnie (`contexte`).

use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use socle::date::Jour;
use socle::fichiers::dossier_du_code;
use socle::num::Resultat;

use crate::routes::catalogue::Route;

pub(crate) fn options(args: &[String]) -> Option<HashMap<String, String>> {
    let mut o = HashMap::new();
    let mut i = 0;
    while i < args.len() {
        let cle = args[i].strip_prefix("--")?;
        if !["racine", "date"].contains(&cle) {
            return None;
        }
        o.insert(cle.to_string(), args.get(i + 1)?.clone());
        i += 2;
    }
    Some(o)
}

/// La compagnie d'une commande : `--racine` (`o["racine"]`, toujours pose par la fenetre et la file des etats), sinon,
/// a la ligne de commande seulement, `PUPPETPAIE_DONNEES` (`moteur route` lance a la main ou par un test).
pub(crate) fn racine_de(o: &HashMap<String, String>) -> PathBuf {
    match (o.get("racine"), std::env::var("PUPPETPAIE_DONNEES")) {
        (Some(r), _) if !r.is_empty() => PathBuf::from(r),
        (_, Ok(e)) if !e.is_empty() => PathBuf::from(e),
        _ => dossier_du_code(),
    }
}

/// La route nomme-t-elle une compagnie (`--racine`, ou `PUPPETPAIE_DONNEES`) ?
pub(crate) fn compagnie_nommee(o: &HashMap<String, String>) -> bool {
    o.get("racine").is_some_and(|r| !r.is_empty()) || std::env::var("PUPPETPAIE_DONNEES").is_ok_and(|e| !e.is_empty())
}

pub(crate) fn contexte(o: &HashMap<String, String>) -> Resultat<Contexte> {
    let date = o.get("date").map(|d| Jour::lire(d)).transpose()?;
    Contexte::nouveau(&racine_de(o), date)
}

fn demande(entree: &str) -> Resultat<Value> {
    serde_json::from_str(entree).map_err(|e| format!("demande illisible : {}", e))
}

/// `moteur route [--racine R] [--date D] [--texte]` : la demande sur l'entree standard. `None` : usage.
///
/// UN ACTE REEL PAR `moteur route` : la demande du mot de passe du logiciel est celle du moteur (`demande::Reel` : sa
/// fenetre, sinon le terminal de l'humain, sans echo, sinon un refus). L'entree standard est celle d'un programme (un LLM,
/// un script) : aucun secret ne s'y lit, ils sont retires de la charge (`sans_mot_de_passe`) et la route les demande a un humain.
pub fn commande(quoi: &str, args: &[String]) -> Option<i32> {
    if quoi != "route" {
        return None;
    }
    let texte_seul = args.iter().any(|a| a == "--texte");
    let args: Vec<String> = args.iter().filter(|a| *a != "--texte").cloned().collect();
    let o = options(&args)?;
    // Les referentiels : ceux de l'installation du programme, comme pour toute autre entree.
    crate::commandes::compagnie::fixer_le_code();
    // `--date D` est l'aujourd'hui de la route : le jour du contexte, et celui de l'horloge (`socle::temps`, le jour pose)
    // qui date ce qu'elle ecrit. Une seule horloge : chaque ligne du journal porte le jour de la route.
    if let Some(d) = o.get("date").filter(|d| Jour::lire(d).is_ok()) {
        std::env::set_var("PUPPETPAIE_AUJOURDHUI", d);
    }
    let _canaux = crate::compagnie::demande::installer(crate::compagnie::demande::canal_de_la_route(&racine_de(&o)));
    let mut entree = String::new();
    let lu = std::io::stdin().read_to_string(&mut entree);
    let nom = demande(entree.trim_start_matches('\u{feff}')).ok().and_then(|d| d.get("route").and_then(Value::as_str).map(String::from)).unwrap_or_default();
    let corps = match lu {
        Ok(_) => sans_mot_de_passe(entree.trim_start_matches('\u{feff}')).and_then(|e| crate::routes::commande(&o, &e)),
        Err(e) => Err(format!("lecture de l'entree standard impossible : {}", e)),
    };
    let corps = corps.unwrap_or_else(|e| {
        let mut m = Map::new();
        m.insert("ok".into(), Value::Bool(false));
        m.insert("erreur".into(), Value::String(e));
        Value::Object(m)
    });
    // LE MESSAGE AU DEVELOPPEUR PART AUSSITOT, comme la fenetre le fait partir en fond (`interface::api`) : le processus
    // attend son depart, sans changer ce que la route a rendu.
    if Route::lire(&nom) == Some(Route::SignalementEnvoyer) && corps["ok"] == Value::Bool(true) {
        let _ = crate::routes::servir_la_route(&o, Route::SignalementsPartir, &json!({}));
    }
    if texte_seul {
        dire_sans_ligne!("{}", en_texte(&nom, &corps));
    } else {
        dire!("{}", corps);
    }
    // UN REFUS SORT EN 1 (la reponse reste la meme) : qui se fie au code de sortie ne croit pas le geste fait. 2 : la
    // commande elle-meme est mal dite (`usage`).
    Some(if corps["ok"] == Value::Bool(true) { 0 } else { 1 })
}

/// La demande sans les secrets de sa charge : le `motdepasse` de toute route, et ceux d'une route du coffre
/// (`routes::coffre::SECRETS`). La route les demande alors elle-meme a un humain.
fn sans_mot_de_passe(entree: &str) -> Resultat<String> {
    let mut d = demande(entree)?;
    let nom = d.get("route").and_then(Value::as_str).unwrap_or("").to_string();
    let du_coffre = Route::lire(&nom).and_then(|r| crate::routes::catalogue::dans(crate::routes::coffre::ROUTES, r)).is_some();
    if let Some(c) = d.get_mut("charge").and_then(Value::as_object_mut) {
        c.remove("motdepasse");
        if du_coffre {
            for s in crate::routes::coffre::SECRETS {
                c.remove(s);
            }
        }
    }
    Ok(d.to_string())
}

/// `--texte` : ce que la page lit de la reponse, sans la composer, et jamais du JSON. Une reussite : la phrase de la route
/// (`phrase`, que chaque route rend : `routes::phrase_en_tete`), puis les lignes d'une liste `phrases` ; un refus : sa
/// phrase, et ce que la fiche de cette route dit de corriger pour sa classe.
pub fn en_texte(route: &str, rep: &Value) -> String {
    if rep.get("ok").and_then(Value::as_bool) == Some(true) {
        let r = &rep["resultat"];
        let mut lignes: Vec<String> = r.get("phrase").and_then(Value::as_str).filter(|p| !p.is_empty()).map(String::from).into_iter().collect();
        lignes.extend(r.get("phrases").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(String::from));
        if lignes.is_empty() {
            lignes.push(format!("La route {} n'a pas rendu de phrase.", route));
        }
        return lignes.iter().map(|p| format!("{}\n", p)).collect();
    }
    let phrase = rep.get("erreur").map(documents::valeur::valeur_en_texte).unwrap_or_default();
    let classe = rep.get("classe").and_then(Value::as_str).unwrap_or("");
    let corriger = crate::routes::catalogue::catalogue()
        .into_iter()
        .filter(|f| f.nom() == route)
        .flat_map(|f| f.refus.iter().copied())
        .find(|r| r.classe == classe)
        .map(|r| format!("À corriger : {}\n", r.corriger))
        .unwrap_or_default();
    format!("Refus : {}\n{}", phrase, corriger)
}
