//! LE NUMERO D'UN CONTRAT (regle 4 : il ne se recalcule pas, il se lit). Il identifie le contrat en DSN
//! (S21.G00.40.009), unique par individu et par etablissement dans une declaration (CCH-12,
//! `atelier/reference/norme-dsn/texte-2027/bloc-S21.G00.40.txt`) ; aucune continuite n'est exigee.
//!
//! Avant tout depart, un engagement n'a pas de numero : l'ecran et les documents en fabrication montrent celui qui lui
//! serait donne (`derniers_pris` + 1, dans l'ordre de l'ecran). Le premier depart qui le porte -- l'envoi du contrat a la
//! signature, son impression pour une remise en main propre, le depot reel de la DSN -- l'ecrit dans l'engagement par
//! `numero_au_depart`, la seule fonction qui l'ecrit. Ensuite il se relit.
//!
//! Le rang d'un numero se lit dans ce qui est deja pris : les numeros que portent les engagements, et ceux que le journal
//! des envois (`journal/depots.json`) dit partis, meme quand leur engagement n'est plus la.

use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;

use droit::contexte::Contexte;
use socle::date::en_texte;
use socle::fichiers::vrai;
use socle::num::Resultat;

use crate::stock;

/// L'annee (2000 + les deux chiffres) et le rang d'un numero `FA1.26.16` ; `None` s'il n'a pas cette forme.
pub fn annee_et_rang(n: &str) -> Option<(i32, i64)> {
    let (tete, rang) = n.rsplit_once('.')?;
    if rang.is_empty() || !rang.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let (_, yy) = tete.rsplit_once('.')?;
    if yy.len() != 2 || !yy.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((2000 + yy.parse::<i32>().ok()?, rang.parse().ok()?))
}

/// Le numero de rang `seq` de l'annee `annee`, pour le salarie de cette fiche : `<initiales><code>.<AA>.<rang>`.
pub fn former(fiche: &Value, annee: i32, seq: i64) -> Resultat<String> {
    let champ = |k: &str| fiche.get(k).map(en_texte).ok_or_else(|| socle::fichiers::absent(k));
    let an = annee.to_string();
    Ok(format!("{}{}.{}.{:02}", champ("initiales_contrat")?, champ("code_salarie")?, &an[2.min(an.len())..], seq))
}

/// Le dernier rang pris en `annee`, par salarie (la cle de sa fiche) : le plus grand des numeros de cette annee que
/// portent ses engagements ou que le journal des envois dit partis pour lui.
pub fn derniers_pris(ctx: &Contexte, annee: i32) -> Resultat<HashMap<String, i64>> {
    let mut pris: HashMap<String, i64> = HashMap::new();
    let mut noter = |cle: &str, numero: &str| {
        if let Some((a, seq)) = annee_et_rang(numero) {
            if a == annee {
                let v = pris.entry(cle.to_string()).or_insert(0);
                *v = (*v).max(seq);
            }
        }
    };
    for o in stock::objets(&ctx.racine)? {
        let e = Value::Object(o.brut.clone());
        let (Some(cle), Some(n)) = (crate::cle_de_l_engagement(ctx, &e), o.brut.get("numero").filter(|n| vrai(n))) else { continue };
        noter(&cle, &en_texte(n));
    }
    for l in lignes_du_journal(&ctx.racine)? {
        let Some(d) = l.get("detail") else { continue };
        let contrats = d.get("contrats").and_then(Value::as_array).cloned().unwrap_or_else(|| vec![d.clone()]);
        for c in &contrats {
            if let (Some(cle), Some(n)) = (c.get("salarie").and_then(Value::as_str), c.get("numero").and_then(Value::as_str)) {
                noter(cle, n);
            }
        }
    }
    Ok(pris)
}

fn lignes_du_journal(racine: &Path) -> Resultat<Vec<Value>> {
    let p = racine.join("journal").join("depots.json");
    match std::fs::read(&p) {
        Ok(o) => match serde_json::from_slice::<Value>(&o).map_err(|e| format!("{} : {}", p.display(), e))? {
            Value::Array(a) => Ok(a),
            _ => Ok(Vec::new()),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(socle::fichiers::panne_fichier(&e, &p)),
    }
}

/// Les rangs, a l'ecran de `mois`, des engagements valides, dans l'ordre : le `i`-ieme engagement qui peut partir
/// (`etats::validees_du_mois`) est a l'ecran au rang `rangs_des_valides(..)[i]`.
pub fn rangs_des_valides(racine: &Path, mois: &str) -> Resultat<Vec<usize>> {
    Ok(stock::objets_du_mois(racine, mois)?
        .iter()
        .enumerate()
        .filter(|(_, o)| o.brut.get("valide").is_some_and(|v| v.get("le").is_some()))
        .map(|(i, _)| i)
        .collect())
}

/// LE NUMERO AU DEPART. `departs` : pour chaque contrat qui part, son rang a l'ecran de `mois` et le numero que le
/// document qui part porte (`None` : il se donne ici, a la suite de `derniers_pris`). Un engagement qui porte deja son
/// numero le garde ; les autres recoivent le leur, ecrit dans l'engagement. Rend le numero de chaque contrat, dans
/// l'ordre de `departs`.
pub fn numero_au_depart(ctx: &Contexte, mois: &str, departs: &[(usize, Option<String>)]) -> Resultat<Vec<String>> {
    let mut objets = stock::objets_du_mois(&ctx.racine, mois)?;
    let mut pris: HashMap<i32, HashMap<String, i64>> = HashMap::new();
    let mut out = Vec::new();
    for (rang, porte) in departs {
        let o = objets.get_mut(*rang).ok_or_else(|| format!("{} : aucun engagement au rang {}", crate::libelle_mois(mois), rang + 1))?;
        if let Some(n) = o.brut.get("numero").filter(|n| vrai(n)) {
            out.push(en_texte(n));
            continue;
        }
        let e = Value::Object(o.brut.clone());
        let numero = match porte.as_deref().filter(|n| !n.is_empty()) {
            Some(n) => n.to_string(),
            None => {
                let nom = e.get("salarie").map(en_texte).unwrap_or_default();
                let fiche = ctx.salarie(&nom)?;
                let cle = fiche.get("_cle").map(en_texte).unwrap_or(nom);
                let debut = crate::debut_brut(&e);
                let annee: i32 = debut.get(..4).and_then(|a| a.parse().ok()).ok_or_else(|| format!("date illisible : {}", debut))?;
                if !pris.contains_key(&annee) {
                    pris.insert(annee, derniers_pris(ctx, annee)?);
                }
                let dernier = pris.get_mut(&annee).unwrap().entry(cle).or_insert(0);
                *dernier += 1;
                former(&fiche, annee, *dernier)?
            }
        };
        o.brut.insert("numero".into(), Value::String(numero.clone()));
        stock::reecrire_en_gardant(&ctx.racine, o)?;
        out.push(numero);
    }
    Ok(out)
}
