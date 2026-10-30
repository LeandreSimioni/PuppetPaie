//! `moteur route` : la demande JSON sur l'entree standard, la reponse en une ligne de JSON sur la sortie standard ; un
//! refus : `{"ok": false, "erreur": "..."}`. Et ce dont les routes ont besoin pour lire la compagnie (`contexte`), la
//! position des droits (`droits`) et la provision de conges (`provision_conges_de`).

use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;

use serde_json::{Map, Value};

use droit::contexte::Contexte;
use socle::date::Jour;
use socle::fichiers::dossier_du_code;
use socle::num::Resultat;

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

pub(crate) fn contexte(o: &HashMap<String, String>) -> Resultat<Contexte> {
    let date = o.get("date").map(|d| Jour::lire(d)).transpose()?;
    Contexte::nouveau(&racine_de(o), date)
}

fn demande(entree: &str) -> Resultat<Value> {
    serde_json::from_str(entree).map_err(|e| format!("demande illisible : {}", e))
}

fn texte<'a>(d: &'a Value, cle: &str) -> Resultat<&'a str> {
    d.get(cle).and_then(Value::as_str).ok_or_else(|| format!("demande illisible : il faut `{}`", cle))
}

fn jour(j: Option<Jour>) -> Value {
    j.map_or(Value::Null, |j| Value::String(j.iso()))
}

/// `droits` : la position de chaque salarie au regard des annexes 8 et 10,
/// `{"mois", "aujourdhui"}` -> `{"constantes", "positions": [{"cle",
/// "anniversaire", "position", "colonnes"}]}`, les salaries dans l'ordre.
pub(crate) fn droits(o: &HashMap<String, String>, entree: &str) -> Resultat<Value> {
    use droit::cumul::{anniversaire_configure, position_droits, tableau_cumul, Constantes};
    use engagement::cumul::lire_engagements;
    let d = demande(entree)?;
    let ctx = contexte(o)?;
    let (mois, aujourdhui) = (texte(&d, "mois")?, Jour::lire(texte(&d, "aujourdhui")?)?);
    let c = Constantes::lire(&ctx.racine, &ctx.lue_au)?;
    let engs = lire_engagements(&ctx, None, &envois::journal::departs(&ctx.racine)?)?;
    let cles: std::collections::BTreeSet<&str> = engs.iter().map(|e| e.salarie.as_str()).collect();
    let mut positions = Vec::new();
    for cle in cles {
        let anniv = anniversaire_configure(&ctx, cle)?;
        let pos = position_droits(&ctx, cle, &engs, anniv, None)?;
        let colonnes = tableau_cumul(&ctx, cle, mois, &engs)?;
        let mut m = Map::new();
        m.insert("cle".into(), Value::String(cle.to_string()));
        m.insert("anniversaire".into(), jour(anniv));
        m.insert("position".into(), pos.canon(aujourdhui)?);
        m.insert("colonnes".into(), Value::Array(colonnes.iter().map(|x| x.canon()).collect()));
        positions.push(Value::Object(m));
    }
    let mut k = Map::new();
    k.insert("heures_par_cachet".into(), c.heures_par_cachet.into());
    k.insert("cachets_par_mois".into(), c.cachets_par_mois.into());
    k.insert("seuil_heures".into(), c.seuil_heures.into());
    let mut m = Map::new();
    m.insert("ok".into(), Value::Bool(true));
    m.insert("constantes".into(), Value::Object(k));
    m.insert("positions".into(), Value::Array(positions));
    Ok(Value::Object(m))
}

/// `moteur route [--racine R] [--date D]` : la demande sur l'entree standard. `None` : usage.
pub fn commande(quoi: &str, args: &[String]) -> Option<i32> {
    if quoi != "route" {
        return None;
    }
    let o = options(args)?;
    // `--date D` est l'aujourd'hui de la route : le jour du contexte, et celui de l'horloge (`socle::temps`, le jour pose)
    // qui date ce qu'elle ecrit. Une seule horloge : chaque ligne du journal porte le jour de la route.
    if let Some(d) = o.get("date").filter(|d| Jour::lire(d).is_ok()) {
        std::env::set_var("PUPPETPAIE_AUJOURDHUI", d);
    }
    let mut entree = String::new();
    let corps = match std::io::stdin().read_to_string(&mut entree) {
        Ok(_) => crate::routes::commande(&o, entree.trim_start_matches('\u{feff}')),
        Err(e) => Err(format!("lecture de l'entree standard impossible : {}", e)),
    };
    let corps = corps.unwrap_or_else(|e| {
        let mut m = Map::new();
        m.insert("ok".into(), Value::Bool(false));
        m.insert("erreur".into(), Value::String(e));
        Value::Object(m)
    });
    println!("{}", corps);
    Some(0)
}
