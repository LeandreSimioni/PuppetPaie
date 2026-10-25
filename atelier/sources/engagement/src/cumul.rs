//! Ce que la compagnie a declare, pour les cumuls des annexes 8 et 10 (`droit::cumul`) : l'historique importe, puis les
//! paies de ses engagements, une ligne declaree (`Declare`) par paie d'intermittent.

use std::collections::BTreeSet;

use serde_json::Value;

use crate::calcul::Paie;
use crate::charger;
use droit::contexte::Contexte;
use droit::cumul::{heures_enseignement, Declare};
use socle::date::d;
use socle::fichiers::lire_json;
use socle::num::{dec_avec_echelle, Resultat};

// ------------------------------------------------------------------- lecture

/// Un entier d'une ligne de l'historique (`defaut` s'il est absent) ; `None` : une valeur qui n'en est pas un, et la
/// ligne abimee est laissee de cote.
fn entier_lu(v: Option<&Value>, defaut: i64) -> Option<i64> {
    match v {
        None => Some(defaut),
        Some(Value::Number(n)) => match n.as_i64() {
            Some(i) => Some(i),
            None => n.as_f64().filter(|f| f.is_finite() && f.abs() < 9.0e18).map(|f| f.trunc() as i64),
        },
        Some(Value::Bool(b)) => Some(i64::from(*b)),
        Some(Value::String(s)) => s.trim().parse().ok(),
        Some(_) => None,
    }
}

/// Une ligne de l'historique, ou `None` si elle est abimee (elle ne fait pas
/// tomber les autres).
fn ligne_historique(e: &Value) -> Resultat<Option<Declare>> {
    let Some(o) = e.as_object() else { return Ok(None) };
    let Some(Value::String(salarie)) = o.get("salarie") else { return Ok(None) };
    let (Some(debut), Some(fin)) = (o.get("debut"), o.get("fin")) else { return Ok(None) };
    let (Ok(debut), Ok(fin)) = (d(debut), d(fin)) else { return Ok(None) };
    let numero_objet = o.get("numero_objet").cloned().unwrap_or_else(|| Value::String(String::new()));
    let (Some(heures), Some(cachets), Some(jours)) =
        (entier_lu(o.get("heures"), 0), entier_lu(o.get("cachets"), 0), entier_lu(o.get("jours"), 0))
    else {
        return Ok(None);
    };
    let zero = Value::String("0".into());
    let brut = dec_avec_echelle(o.get("brut").unwrap_or(&zero))?;
    let Some(heures_enseignement) = entier_lu(o.get("heures_enseignement"), 0) else { return Ok(None) };
    Ok(Some(Declare { salarie: salarie.clone(), debut, fin, numero_objet, heures, cachets, jours, brut, heures_enseignement }))
}

/// Les contrats attestes avant ce logiciel (`historique/engagements.json`).
/// Vide tant qu'on n'a rien importe.
pub fn lire_historique(ctx: &Contexte) -> Resultat<Vec<Declare>> {
    let p = ctx.racine.join("historique").join("engagements.json");
    if !socle::fichiers::existe(&p) {
        return Ok(Vec::new());
    }
    let data = lire_json(&p)?;
    let mut out = Vec::new();
    if let Some(Value::Array(l)) = data.get("engagements") {
        for e in l {
            if let Some(x) = ligne_historique(e)? {
                out.push(x);
            }
        }
    }
    Ok(out)
}

/// Une ligne declaree par paie d'intermittent (les permanents n'ouvrent aucun droit).
fn engagement_de_paie(ctx: &Contexte, p: &Paie) -> Resultat<Option<Declare>> {
    let e = &p.engagement;
    if e.sans_paie() || e.nat()?.mensuelle() {
        return Ok(None);
    }
    let heures = e.heures.unwrap_or(0.0).round_ties_even() as i64;
    let cle = p.salarie.get("_cle").ok_or_else(|| socle::fichiers::absent("_cle"))?;
    Ok(Some(Declare {
        salarie: socle::date::en_texte(cle),
        debut: e.debut_periode()?,
        fin: e.fin_periode()?,
        numero_objet: p.spectacle.get("numero_objet").cloned().unwrap_or_else(|| Value::String(String::new())),
        heures,
        cachets: e.cachets,
        jours: e.jours,
        brut: p.brut,
        heures_enseignement: heures_enseignement(ctx, &e.lignes)?.min(heures),
    }))
}

/// Ce que nos fichiers de paie declarent : (engagements, mois couverts).
/// Un fichier qui ne se charge pas est laisse de cote, son mois n'est pas
/// couvert. `paies_courantes` : les paies du mois en cours, deja calculees ;
/// elles remplacent le fichier de leur mois.
pub fn engagements_des_paies(
    ctx: &Contexte,
    paies_courantes: Option<&[Paie]>,
) -> Resultat<(Vec<Declare>, BTreeSet<String>)> {
    // Tous les mois se chargent : le brut d'un mois anterieur une seule fois.
    let _derivation = crate::conges::Derivation::ouvrir();
    let mut groupes: Vec<(String, Vec<Declare>)> = Vec::new();
    for p in paies_courantes.unwrap_or(&[]) {
        let mois = p.engagement.mois.clone();
        let i = match groupes.iter().position(|(m, _)| *m == mois) {
            Some(i) => i,
            None => {
                groupes.push((mois, Vec::new()));
                groupes.len() - 1
            }
        };
        if let Some(e) = engagement_de_paie(ctx, p)? {
            groupes[i].1.push(e);
        }
    }
    let dossier = ctx.racine.join("paies");
    let mut fichiers: Vec<(String, std::path::PathBuf)> = Vec::new();
    if let Some(rd) = socle::fichiers::lister(&dossier) {
        for f in rd {
            let nom = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if let Some(stem) = nom.strip_suffix(".json") {
                fichiers.push((stem.to_string(), f.clone()));
            }
        }
    }
    fichiers.sort();
    for (stem, chemin) in fichiers {
        if stem.chars().count() != 7 || groupes.iter().any(|(m, _)| *m == stem) {
            continue;
        }
        let Ok(saisie) = charger(ctx, &chemin) else { continue };
        crate::conges::memoriser_mois(ctx, &stem, &saisie.paies);
        let mut es = Vec::new();
        for p in &saisie.paies {
            if let Some(e) = engagement_de_paie(ctx, p)? {
                es.push(e);
            }
        }
        groupes.push((stem, es));
    }
    let couverts = groupes.iter().map(|(m, _)| m.clone()).collect();
    Ok((groupes.into_iter().flat_map(|(_, es)| es).collect(), couverts))
}

/// TOUT ce que la compagnie a declare : l'historique importe, puis nos paies
/// (pour un mois qui a les deux, les paies seulement), trie par (debut, fin, salarie).
pub fn lire_engagements(ctx: &Contexte, paies_courantes: Option<&[Paie]>) -> Resultat<Vec<Declare>> {
    let (des_paies, couverts) = engagements_des_paies(ctx, paies_courantes)?;
    let mut tous: Vec<Declare> = lire_historique(ctx)?.into_iter().filter(|e| !couverts.contains(&e.mois())).collect();
    tous.extend(des_paies);
    tous.sort_by(|a, b| (a.debut, a.fin, &a.salarie).cmp(&(b.debut, b.fin, &b.salarie)));
    Ok(tous)
}

