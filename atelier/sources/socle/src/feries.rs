//! Les jours feries et ceux qui sont chomes (`moteur/feries.py`).
//!
//! Un ferie n'est pas forcement chome : la loi ne rend chome d'office que
//! le 1er mai. Le reste se lit dans la fiche entreprise
//! (`jours_feries_chomes`). Les dates mobiles se comptent depuis Paques.
//!
//! Un `dict {date: nom}` devient `Jours`, une liste dans l'ordre d'insertion.

use crate::date::Jour;
use crate::fichiers::{dossier_du_code, json_en_cache, repr};
use crate::num::Resultat;
use serde_json::Value;
use std::sync::Arc;

/// `{date: nom}`, dans l'ordre d'insertion du dict Python.
pub type Jours = Vec<(Jour, String)>;

/// `datetime.date(an, mois, jour)` avec les refus et messages de Python.
pub fn date_py(an: i64, mois: i64, jour: i64) -> Resultat<Jour> {
    if !(1..=9999).contains(&an) {
        return Err(crate::date::annee_impossible(an));
    }
    if !(1..=12).contains(&mois) {
        return Err(crate::date::mois_impossible(mois));
    }
    Jour::nouveau(an as i32, mois as u32, jour.max(0) as u32)
        .ok_or_else(|| crate::date::jour_impossible(an, mois, jour))
}

fn inserer(out: &mut Jours, jour: Jour, nom: &str) {
    match out.iter_mut().find(|(j, _)| *j == jour) {
        Some(e) => e.1 = nom.to_string(),
        None => out.push((jour, nom.to_string())),
    }
}

/// `feries-base.json`, lu une fois tant qu'il ne change pas.
pub fn nomenclature() -> Resultat<Arc<Value>> {
    json_en_cache(&dossier_du_code().join("feries-base.json"))
}

fn jours_du_fichier() -> Resultat<Vec<Value>> {
    let n = nomenclature()?;
    n.get("jours")
        .and_then(|j| j.as_array())
        .cloned()
        .ok_or_else(|| crate::fichiers::absent("jours"))
}

fn entier(v: &Value, cle: &str) -> Resultat<i64> {
    match v {
        Value::Number(n) => n
            .as_i64()
            .or_else(|| n.as_f64().map(|x| x.trunc() as i64))
            .ok_or_else(|| format!("{} : nombre hors de portée", repr(cle))),
        Value::String(s) => s
            .trim()
            .parse::<i64>()
            .map_err(|_| format!("nombre entier illisible : {}", repr(s))),
        autre => Err(format!("{} : {}", repr(cle), crate::fichiers::attendu("un nombre entier", crate::valeur::nom_type(autre)))),
    }
}

/// Le dimanche de Paques (Meeus, comput gregorien).
pub fn paques(annee: i32) -> Resultat<Jour> {
    let an = annee as i64;
    let a = an.rem_euclid(19);
    let (b, c) = (an.div_euclid(100), an.rem_euclid(100));
    let (d_, e) = (b.div_euclid(4), b.rem_euclid(4));
    let f = (b + 8).div_euclid(25);
    let g = (b - f + 1).div_euclid(3);
    let h = (19 * a + b - d_ - g + 15).rem_euclid(30);
    let (i, k) = (c.div_euclid(4), c.rem_euclid(4));
    let l = (32 + 2 * e + 2 * i - h - k).rem_euclid(7);
    let m = (a + 11 * h + 22 * l).div_euclid(451);
    let t = h + l - 7 * m + 114;
    date_py(an, t.div_euclid(31), t.rem_euclid(31) + 1)
}

/// Les onze fetes legales de l'annee.
pub fn feries(annee: i32) -> Resultat<Jours> {
    let mut out = Jours::new();
    let dimanche = paques(annee)?;
    for j in jours_du_fichier()? {
        let nom = j.get("nom").and_then(|n| n.as_str()).ok_or_else(|| crate::fichiers::absent("nom"))?;
        let jour = match j.get("paques") {
            Some(p) => dimanche.plus_jours(entier(p, "paques")?)?,
            None => {
                let mois = entier(j.get("mois").ok_or_else(|| crate::fichiers::absent("mois"))?, "mois")?;
                let jr = entier(j.get("jour").ok_or_else(|| crate::fichiers::absent("jour"))?, "jour")?;
                date_py(annee as i64, mois, jr)?
            }
        };
        inserer(&mut out, jour, nom);
    }
    Ok(out)
}

/// Ceux que la loi rend chomes : le 1er mai, et lui seul.
pub fn chomes_de_droit(annee: i32) -> Resultat<Jours> {
    let mut noms = Vec::new();
    for j in jours_du_fichier()? {
        if crate::fichiers::vrai(j.get("chome_de_droit").unwrap_or(&Value::Null)) {
            if let Some(n) = j.get("nom").and_then(|n| n.as_str()) {
                noms.push(n.to_string());
            }
        }
    }
    Ok(feries(annee)?.into_iter().filter(|(_, n)| noms.contains(n)).collect())
}

/// Les feries chomes dans cette entreprise, cette annee.
pub fn chomes(annee: i32, entreprise: Option<&Value>) -> Resultat<Jours> {
    let dits = entreprise.and_then(|e| e.get("jours_feries_chomes"));
    let dits = match dits {
        None | Some(Value::Null) => return chomes_de_droit(annee),
        Some(d) => d,
    };
    if dits.as_str() == Some("tous") {
        return feries(annee);
    }
    if dits.as_str() == Some("1er mai seulement") {
        return chomes_de_droit(annee);
    }
    // `set(dits)` : une chaine se decoupe en caracteres, comme en Python.
    let voulus: Vec<String> = match dits {
        Value::String(s) => s.chars().map(|c| c.to_string()).collect(),
        Value::Array(a) => a
            .iter()
            .map(|x| match x {
                Value::String(s) => s.clone(),
                autre => crate::date::en_texte(autre),
            })
            .collect(),
        Value::Object(o) => o.keys().cloned().collect(),
        autre => return Err(crate::fichiers::attendu("une liste", crate::valeur::nom_type(autre))),
    };
    let tous = jours_du_fichier()?;
    let connus: Vec<&str> = tous.iter().filter_map(|j| j.get("nom").and_then(|n| n.as_str())).collect();
    let mut inconnus: Vec<&String> = Vec::new();
    for v in &voulus {
        if !connus.contains(&v.as_str()) && !inconnus.contains(&v) {
            inconnus.push(v);
        }
    }
    if !inconnus.is_empty() {
        inconnus.sort();
        return Err(format!(
            "Jours fériés chômés (fiche Entreprise) : {} n'est pas un jour \
             férié légal. Les onze sont : {}.",
            inconnus.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "),
            connus.join(", ")
        ));
    }
    Ok(feries(annee)?.into_iter().filter(|(_, n)| voulus.contains(n)).collect())
}

/// Les feries chomes d'une periode, annees a cheval comprises.
pub fn chomes_entre(debut: &Jour, fin: &Jour, entreprise: Option<&Value>) -> Resultat<Jours> {
    let mut out = Jours::new();
    for annee in debut.an..=fin.an {
        for (jour, nom) in chomes(annee, entreprise)? {
            if *debut <= jour && jour <= *fin {
                inserer(&mut out, jour, &nom);
            }
        }
    }
    Ok(out)
}

/// L'entreprise n'a pas dit quels feries elle chome : le plancher legal
/// s'applique, et il faut le signaler.
pub fn rien_n_est_dit(entreprise: Option<&Value>) -> bool {
    matches!(entreprise.and_then(|e| e.get("jours_feries_chomes")), None | Some(Value::Null))
}
