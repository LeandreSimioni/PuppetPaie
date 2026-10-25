//! Ce que la DSN lit de `moteur/operations_casc.py` : la base de la CASC-SVP,
//! les mois regularises hors DSN, et le fichier de la DSN deposee d'un mois.

use droit::contexte::Contexte;
use socle::fichiers::{dossier_du_code, lire_json, vrai};
use socle::num::Resultat;
use serde_json::Value;
use std::path::PathBuf;

/// `_base` : `casc-svp-base.json`, livre avec le code.
pub fn base() -> Resultat<Value> {
    lire_json(&dossier_du_code().join("casc-svp-base.json"))
}

/// `mois_regularises` : les mois de `config/casc-svp.json` regularises hors DSN
/// (un fichier absent ou illisible n'en regularise aucun).
pub fn mois_regularises(ctx: &Contexte) -> Resultat<Vec<String>> {
    let Ok(d) = lire_json(&ctx.racine.join("config").join("casc-svp.json")) else {
        return Ok(vec![]);
    };
    let mut mois: Vec<String> = vec![];
    for reg in d.get("regularisations").and_then(Value::as_array).into_iter().flatten() {
        for m in reg.get("mois").and_then(Value::as_array).into_iter().flatten() {
            let m = socle::date::en_texte(m);
            if !mois.contains(&m) {
                mois.push(m);
            }
        }
    }
    mois.sort();
    Ok(mois)
}

fn texte(v: Option<&Value>) -> String {
    match v {
        Some(x) if vrai(x) => socle::date::en_texte(x),
        _ => String::new(),
    }
}

/// `dsn_deposee` : le fichier de la DSN qui fait foi pour ce mois, tel qu'il a
/// ete depose -- la derniere DSN reelle acceptee au journal, sinon celle que
/// l'import CoolPaie a retenue. Un mois ni depose ni importe rend `None` ; un
/// mois depose dont le fichier est introuvable refuse. `nic` : un etablissement,
/// absent le principal.
pub fn dsn_deposee(ctx: &Contexte, mois: &str, nic: Option<&str>) -> Resultat<Option<PathBuf>> {
    let principal = texte(ctx.entreprise.get("nic"));
    let voulu = match nic {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => principal.clone(),
    };
    let journal = ctx.racine.join("journal").join("depots.json");
    let lignes = if journal.exists() { lire_json(&journal)? } else { Value::Array(vec![]) };
    let reels: Vec<&Value> = lignes
        .as_array()
        .into_iter()
        .flatten()
        .filter(|l| {
            l.get("quoi").and_then(Value::as_str) == Some("dsn")
                && l.get("mois").and_then(Value::as_str) == Some(mois)
                && l.get("mode").and_then(Value::as_str) == Some("reel")
        })
        .filter(|l| {
            let e = texte(l.get("detail").and_then(|d| d.get("etablissement")));
            (if e.is_empty() { principal.clone() } else { e }) == voulu
        })
        .filter(|l| l.get("accepte").map_or(false, vrai))
        .collect();
    if let Some(n) = nic {
        if !n.is_empty() && n != principal && reels.is_empty() {
            return Ok(None);
        }
    }
    if let Some(dernier) = reels.last() {
        let nom = texte(dernier.get("fichier"));
        if !nom.is_empty() {
            for p in [ctx.racine.join("sorties").join(mois).join(&nom), ctx.racine.join(&nom)] {
                if p.exists() {
                    return Ok(Some(p));
                }
            }
        }
        let quand: String = texte(dernier.get("quand")).chars().take(10).collect();
        return Err(format!(
            "La DSN de {} a ete deposee le {}, mais le journal ne dit pas quel fichier ({}) ou il n'est plus \
dans sorties/{}/. La regularisation se lit dans la DSN deposee, elle ne se recompose pas : retrouver le \
fichier d'abord.",
            mois,
            quand,
            if nom.is_empty() { "aucun nom" } else { &nom },
            mois
        ));
    }
    let chemin = ctx.racine.join("historique").join("paies.json");
    let decl = if chemin.exists() { lire_json(&chemin)? } else { Value::Object(Default::default()) };
    let noms: Vec<String> = decl
        .get("paies")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|x| x.get("mois").and_then(Value::as_str) == Some(mois))
        .map(|x| texte(x.get("fichier")))
        .filter(|f| !f.is_empty())
        .collect();
    let Some(nom) = noms.last() else { return Ok(None) };
    let source = match decl.get("source") {
        Some(v) if vrai(v) => PathBuf::from(socle::date::en_texte(v)),
        _ => ctx.racine.clone(),
    };
    let copie = ctx.racine.join("historique").join("coolpaie");
    for d in [
        copie.join("DSN"),
        ctx.racine.join("export-coolpaie").join("DSN"),
        ctx.racine.join("export-coolpaie"),
        source.join("DSN"),
    ] {
        if d.join(nom).exists() {
            return Ok(Some(d.join(nom)));
        }
    }
    Err(format!(
        "La DSN de {} ({}) a ete reprise de l'historique, mais le fichier n'est plus sous historique/coolpaie/DSN/ ni dans le dossier d'ou elle a ete reprise. \
La regularisation se lit dans la DSN deposee : la remettre la d'abord.",
        mois, nom
    ))
}
