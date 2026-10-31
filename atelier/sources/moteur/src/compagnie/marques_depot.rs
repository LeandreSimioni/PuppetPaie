//! Ce qu'un depot REEL laisse derriere lui, hors du journal des depots : la demande d'un taux de PAS a la DGFiP
//! qu'un signalement d'amorcage fait courir. Et les arrets que la DSN d'un mois reporte (`arrets_pour`). Rien d'ici ne
//! fait partir quoi que ce soit ; un depot de TEST n'y laisse rien (l'appelant ne l'appelle qu'apres un reel).

use std::path::Path;

use serde_json::{json, Value};

use droit::contexte::Contexte;
use socle::date::Jour;
use socle::echec::R;
use socle::echec::erreur;

/// Un fichier lu comme `Path.read_text(encoding="latin-1")` : un octet, un caractere, et les fins de ligne
/// `\r\n` et `\r` lues comme `\n`.
fn lire_latin1(chemin: &Path) -> R<String> {
    let octets = std::fs::read(chemin).map_err(|e| erreur("OSError", format!("{} : {}", chemin.display(), e)))?;
    let mut s = String::with_capacity(octets.len());
    let mut i = 0;
    while i < octets.len() {
        if octets[i] == b'\r' {
            s.push('\n');
            if octets.get(i + 1) == Some(&b'\n') {
                i += 1;
            }
        } else {
            s.push(octets[i] as char);
        }
        i += 1;
    }
    Ok(s)
}

/// La valeur de la premiere ligne qui la porte.
fn valeur_de_rubrique(texte: &str, rubrique: &str) -> Option<String> {
    let debut = format!("{},'", rubrique);
    texte.split('\n').find_map(|l| l.strip_prefix(debut.as_str())?.strip_suffix('\'').map(str::to_string))
}

/// La cle de la fiche qui porte ce NIR.
fn cle_du_nir(ctx: &Contexte, nir: &str) -> Option<String> {
    ctx.salaries.iter().find(|(_, f)| f.is_object() && f.get("nir").and_then(Value::as_str) == Some(nir)).map(|(c, _)| c.clone())
}

// ---------------------------------------------------------------------- les arrets

/// Les arrets que la DSN de `mois` doit reporter -- ceux du magasin des imprevus dont la periode croise le mois principal
/// declare (`imprevus::du_mois`).
pub(crate) fn arrets_pour(ctx: &Contexte, mois: &str) -> R<Vec<Value>> {
    socle::date::premier_du_mois(mois).map_err(|m| erreur("ValueError", m))?;
    engagement::imprevus::du_mois(&ctx.racine, mois, Some(engagement::imprevus::ARRET)).map_err(crate::routes::imprevus::refus_magasin)
}

// --------------------------------------------------------------------- l'amorcage

/// Le jour ouvre qui tombe `combien` jours ouvres apres `depart` -- ni samedi,
/// ni dimanche, ni fete legale.
fn jours_ouvres_apres(depart: Jour, combien: i64) -> R<Jour> {
    let mut jour = depart;
    let mut restants = combien;
    let mut cache: std::collections::HashMap<i32, Vec<Jour>> = std::collections::HashMap::new();
    while restants > 0 {
        jour = jour.plus_jours(1).map_err(|m| erreur("OverflowError", m))?;
        if jour.jour_de_la_semaine() >= 5 {
            continue;
        }
        if !cache.contains_key(&jour.an) {
            let f = socle::feries::feries(jour.an).map_err(|m| erreur("ValueError", m))?;
            cache.insert(jour.an, f.into_iter().map(|(j, _)| j).collect());
        }
        if cache[&jour.an].contains(&jour) {
            continue;
        }
        restants -= 1;
    }
    Ok(jour)
}

/// `pas.enregistrer_demande` (reel) : note qu'un signalement d'amorcage est parti pour ces salaries.
/// `quand` : l'instant du depot (`AAAA-MM-JJ...`), sinon le jour du contexte.
pub(crate) fn enregistrer_demande(ctx: &Contexte, cles: &[String], motif: &str, quand: Option<&str>, flux: &str) -> R<()> {
    let jour = match quand.filter(|q| !q.is_empty()) {
        Some(q) => Jour::lire(&q.chars().take(10).collect::<String>()).map_err(|m| erreur("ValueError", m))?,
        None => ctx.date,
    };
    let avant = jours_ouvres_apres(jour, 5)?;
    let mut data = droit::pas::charger(ctx).map_err(|m| erreur("OSError", m))?;
    for cle in cles {
        data["demandes"][cle.as_str()] = json!({
            "quand": jour.iso(), "motif": motif, "reponse_attendue_avant": avant.iso(), "flux": flux,
        });
    }
    if !cles.is_empty() {
        let chemin = ctx.racine.join("config").join("pas.json");
        std::fs::write(&chemin, socle::ecriture::json_indente(&data, 2)).map_err(|e| erreur("OSError", format!("{} : {}", chemin.display(), e)))?;
    }
    Ok(())
}

/// Note la demande a partir du FICHIER qui vient de partir.
/// C'est ici que la demande commence a courir -- ce qui fait courir les cinq jours ouvres, c'est le depot.
pub(crate) fn marquer_demande_depuis(ctx: &Contexte, chemin: &Path, quand: &str) -> R<()> {
    let texte = lire_latin1(chemin)?;
    let (Some(nir), Some(motif)) = (valeur_de_rubrique(&texte, "S21.G00.30.001"), valeur_de_rubrique(&texte, "S20.G00.05.011")) else {
        return Ok(());
    };
    let Some(cle) = cle_du_nir(ctx, &nir) else { return Ok(()) };
    enregistrer_demande(ctx, &[cle], &motif, Some(quand), "")
}
