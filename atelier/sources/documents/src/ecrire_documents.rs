//! Les compteurs des documents : les numeros de contrat et de bulletin que portent les paies d'un mois
//! (`numeroter`, `paies_aux_numeros_declares`), ceux qu'un envoi reserve avant de les ecrire (`Reserves`), la sequence
//! continue des N CE (`prochain_continu`) et le verrou sous lequel un numero se reserve puis s'ecrit (`prendre_le_verrou`).

use std::collections::HashMap;

use serde_json::{json, Value};

use droit::contexte::Contexte;
use socle::echec::erreur;
use socle::echec::Echec;
use socle::echec::R;
use socle::fichiers::lire_json;
use socle::verrou::Verrou;
use socle::echec::Refus;
use socle::ecriture::json_indente;
use socle::fichiers::repr;
use socle::num::entier_lu;
use engagement::calcul::Paie;

fn os(chemin: &std::path::Path, e: std::io::Error) -> Echec {
    erreur("OSError", format!("{} : {}", chemin.display(), e))
}

fn valeur(m: String) -> Echec {
    erreur("ValueError", m)
}

/// `Contexte.prochain_continu` : une sequence qui ne repart pas a 1 chaque an. La LECTURE balaie toutes les
/// annees de `compteurs.json` et retient le maximum.
pub fn prochain_continu(ctx: &Contexte, cle: &str, defaut: i64) -> R<i64> {
    let p = ctx.racine.join("config").join("compteurs.json");
    let mut vu = defaut - 1;
    if p.exists() {
        let c = lire_json(&p).map_err(|m| Echec::from(m))?;
        if let Value::Object(annees) = &c {
            for valeurs in annees.values() {
                if let Some(v) = valeurs.as_object().and_then(|o| o.get(cle)) {
                    let n = match v {
                        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
                        Value::String(s) => s.trim().parse().ok(),
                        Value::Bool(b) => Some(*b as i64),
                        _ => None,
                    }
                    .ok_or_else(|| erreur("ValueError", format!("nombre entier illisible : {}", socle::fichiers::repr_valeur(v))))?;
                    vu = vu.max(n);
                }
            }
        }
    }
    Ok(vu + 1)
}

/// `verrou_compteurs(ctx, qui=...)` : refuse si un autre tient les compteurs.
pub fn prendre_le_verrou(ctx: &Contexte, qui: &str) -> R<Verrou> {
    let chemin = socle::verrou::chemin_compteurs(&ctx.racine);
    let maintenant = socle::paris::iso_secondes(socle::temps::secondes()).replace('T', " ");
    let marque = socle::verrou::marque(qui, std::process::id(), &maintenant);
    match Verrou::prendre(&chemin, &marque, socle::verrou::DELAI_VERROU).map_err(|m| erreur("OSError", m))? {
        Ok(v) => Ok(v),
        Err(v) => Err(erreur("CompteursVerrouilles", v.message())),
    }
}


// ------------------------------------------------------------------ les numeros des contrats et des bulletins

/// Les numeros de contrat (FA1.26.13) et de bulletin (17/2026) que les paies porteraient (`engagement::numeros::numeroter`), en
/// memoire : l'apercu n'ecrit rien. Un engagement qui PORTE DEJA son numero de contrat le garde (`engagement::numeros`),
/// une paie partie son numero de bulletin (`engagement::etats::Valide::bulletin`).
pub fn numeroter(ctx: &Contexte, paies: &mut [Paie], annee: i32) -> Result<(), Refus> {
    numeroter_en_attente(ctx, paies, annee, &mut HashMap::new())
}

/// La meme, avec les numeros de bulletin deja RESERVES par un appel precedent du meme processus : `(annee, "bulletin")
/// -> dernier numero`, sinon a la suite du compteur de l'annee (`config/compteurs.json`) et des bulletins importes de
/// l'annee (`engagement::numeros::dernier_bulletin_importe`).
pub fn numeroter_en_attente(ctx: &Contexte, paies: &mut [Paie], annee: i32, attente: &mut HashMap<(String, String), i64>) -> Result<(), Refus> {
    let p = ctx.racine.join("config").join("compteurs.json");
    let compteurs = if p.exists() { lire_json(&p)? } else { Value::Null };
    let an = annee.to_string();
    let n_bull = match attente.get(&(an.clone(), "bulletin".to_string())) {
        Some(v) => v + 1,
        None => compteurs.get(&an).and_then(|c| c.get("bulletin")).and_then(|v| match v {
            Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
            Value::String(s) => s.trim().parse().ok(),
            _ => None,
        }).unwrap_or(0).max(engagement::numeros::dernier_bulletin_importe(&ctx.racine, annee.into())?) + 1,
    };
    let n_bull = engagement::numeros::numeroter(ctx, paies, annee, n_bull)?;
    attente.insert((an, "bulletin".to_string()), n_bull - 1);
    Ok(())
}

// ------------------------------------------------------------- les numeros reserves

/// Les numeros reserves par ce processus, dans leur ordre d'arrivee. Rien n'est ecrit avant `valider`.
#[derive(Default)]
pub struct Reserves {
    valeurs: HashMap<(String, String), i64>,
    ordre: Vec<(String, String)>,
}

impl Reserves {
    /// Le suivant de ce qui est reserve, sinon de ce que `compteurs.json` porte ; un bulletin, apres les bulletins importes
    /// de l'annee (`engagement::numeros::dernier_bulletin_importe`).
    pub fn prochain(&self, ctx: &Contexte, cle: &str, annee: i64) -> R<i64> {
        let an = annee.to_string();
        if let Some(v) = self.valeurs.get(&(an.clone(), cle.to_string())) {
            return Ok(v + 1);
        }
        let p = ctx.racine.join("config").join("compteurs.json");
        let c = if p.exists() { lire_json(&p).map_err(valeur)? } else { Value::Null };
        let lu = match c.get(&an).and_then(|a| a.get(cle)) {
            None => 0,
            Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)).unwrap_or(0),
            Some(Value::String(s)) => s.trim().parse().map_err(|_| valeur(format!("nombre entier illisible : {}", socle::fichiers::repr(s))))?,
            Some(Value::Bool(b)) => *b as i64,
            Some(_) => 0,
        };
        let importe = if cle == "bulletin" { engagement::numeros::dernier_bulletin_importe(&ctx.racine, annee).map_err(Echec::from)? } else { 0 };
        Ok(lu.max(importe) + 1)
    }

    /// Reserve, sans ecrire.
    pub fn enregistrer(&mut self, annee: i64, cle: &str, v: i64) {
        let k = (annee.to_string(), cle.to_string());
        if !self.valeurs.contains_key(&k) {
            self.ordre.push(k.clone());
        }
        self.valeurs.insert(k, v);
    }

    /// Les numeros de bulletin des paies, reserves ; leurs numeros de contrat, ceux qu'elles porteraient.
    pub fn numeroter(&mut self, ctx: &Contexte, paies: &mut [Paie], annee: i32) -> R<()> {
        numeroter_en_attente(ctx, paies, annee, &mut self.valeurs)?;
        let k = (annee.to_string(), "bulletin".to_string());
        if !self.ordre.contains(&k) {
            self.ordre.push(k);
        }
        Ok(())
    }

    /// Ecrit les numeros reserves dans `config/compteurs.json`.
    pub fn valider(&self, ctx: &Contexte) -> R<()> {
        if self.ordre.is_empty() {
            return Ok(());
        }
        let p = ctx.racine.join("config").join("compteurs.json");
        let mut c = if p.exists() { lire_json(&p).map_err(valeur)? } else { json!({}) };
        let lu = socle::valeur::nom_type(&c);
        let racine = c.as_object_mut().ok_or_else(|| erreur("AttributeError", format!("config/compteurs.json : {}", socle::fichiers::attendu("un objet", lu))))?;
        for (an, cle) in &self.ordre {
            let annee = racine.entry(an.clone()).or_insert_with(|| json!({}));
            let annee = annee.as_object_mut().ok_or_else(|| erreur("AttributeError", "l'annee des compteurs n'est pas un dictionnaire"))?;
            annee.insert(cle.clone(), json!(self.valeurs[&(an.clone(), cle.clone())]));
        }
        std::fs::write(&p, json_indente(&c, 2)).map_err(|e| os(&p, e))
    }
}

// ------------------------------------------------------------------ les paies d'un mois aux numeros declares

/// LA SAISIE DU MOIS AUX NUMEROS DECLARES : ce qui peut partir de `mois` (`etats::validees_du_mois` : la saisie reduite
/// aux engagements valides, et leurs paies ecrites), chaque contrat sous le numero que son engagement porte depuis sa
/// validation (`engagement::numeros`), chaque bulletin sous le sien (ecrit dans la paie a son depart, ou celui que le
/// compteur propose). `reserves` : les numeros de bulletin qu'un envoi propose, que la route d'envoi ecrit plus loin. Un
/// mois sans engagement valide n'a rien.
pub fn paies_aux_numeros_declares(ctx: &Contexte, mois: &str, reserves: Option<&mut Reserves>) -> Result<(Value, Vec<Paie>), Refus> {
    let Some((data, mut paies)) = engagement::etats::validees_du_mois(ctx, mois)? else {
        return Ok((json!({ "mois": mois, "engagements": [] }), Vec::new()));
    };
    let mois_saisie = data.get("mois").map(socle::date::en_texte).ok_or_else(|| socle::fichiers::absent("mois"))?;
    let annee = entier_lu(&mois_saisie.chars().take(4).collect::<String>()).ok_or_else(|| format!("mois illisible : {}", repr(&mois_saisie)))? as i32;
    match reserves {
        Some(r) => r.numeroter(ctx, &mut paies, annee)?,
        None => numeroter(ctx, &mut paies, annee)?,
    }
    Ok((data, paies))
}

