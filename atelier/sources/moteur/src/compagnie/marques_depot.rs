//! Ce qu'un depot REEL laisse derriere lui, hors du journal des depots.
//!
//! Chaque fonction ecrit la trace d'un acte parti : la DSN qui a reporte un arret, le signalement d'un
//! arret, la demande d'un taux de PAS a la DGFiP. Rien d'ici ne fait partir quoi que ce
//! soit ; un depot de TEST n'y laisse rien (l'appelant ne les appelle qu'apres un reel).

use std::path::Path;

use serde_json::{json, Value};

use droit::contexte::Contexte;
use socle::date::Jour;
use socle::valeur::valeur_en_texte;
use socle::echec::R;
use socle::echec::erreur;

/// `datetime.now().strftime("%Y-%m-%d %H:%M")`, a l'heure de Paris.
fn maintenant() -> String {
    crate::routes::maintenant()
}

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

/// La cle de la fiche qui porte ce NIR (`next(c for c, f in ctx.salaries.items() if f.get("nir") == nir)`).
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

/// Le mois ou l'arret commence.
fn mois_de(arret: &Value) -> String {
    valeur_en_texte(&arret["dernier_jour_travaille"]).chars().take(7).collect()
}

/// `_trouver`, avec `ArretIntrouvable` rendu en `None`.
fn trouver_ou_rien(ctx: &Contexte, salarie: &Value, dernier_jour: &Value) -> R<Option<(usize, Vec<Value>)>> {
    match crate::routes::imprevus::trouver_l_arret(ctx, salarie, dernier_jour) {
        Ok(t) => Ok(Some(t)),
        Err(e) if e.classe == Some("ErreurOperation") && e.message.starts_with("Aucun arret de ") => Ok(None),
        Err(e) => Err(e),
    }
}

/// Trace l'envoi d'un signalement a partir du FICHIER qui vient
/// de partir. Le fichier ne porte pas la cle de nos fiches : il porte un NIR (S21.G00.30.001) et le jour de
/// l'evenement. `None` si le fichier ne vient pas du magasin -- ce n'est pas une erreur.
pub(crate) fn marquer_envoye_depuis(
    ctx: &Contexte,
    chemin: &Path,
    nature: &str,
    ordre: &str,
    type_declaration: &str,
    evenement: &str,
    quand: &str,
) -> R<Option<Value>> {
    let texte = lire_latin1(chemin)?;
    let Some(nir) = valeur_de_rubrique(&texte, "S21.G00.30.001") else { return Ok(None) };
    let Some(cle) = cle_du_nir(ctx, &nir) else { return Ok(None) };
    let Some((i, mut liste)) = trouver_ou_rien(ctx, &json!(cle), &json!(evenement))? else { return Ok(None) };
    let rang = ordre.trim().parse::<i64>().ok().filter(|n| *n != 0).unwrap_or(1);
    let quand = if quand.is_empty() { maintenant() } else { quand.to_string() };
    liste[i]["envois"]
        .as_array_mut()
        .ok_or_else(|| erreur("AttributeError", "'envois' n'est pas une liste"))?
        .push(json!({
            "nature": nature, "type": type_declaration, "ordre": rang,
            "fichier": chemin.to_string_lossy(), "quand": quand,
            // L'identifiant rendu par le point de depot : il se lit sur le certificat de conformite.
            "identifiant": "",
        }));
    let arret = liste[i].clone();
    crate::routes::imprevus::ecrire_arrets(ctx, &mois_de(&arret), liste)?;
    Ok(Some(arret))
}

/// Note, sur chaque imprevu (arret ou suspension) que la DSN mensuelle REELLE de `mois` a declare, qu'elle l'a reporte
/// (un envoi de nature 01) : un imprevu declare ne s'oublie plus.
pub(crate) fn marquer_reportes(ctx: &Contexte, mois: &str, fichier: &Path, quand: &str) -> R<()> {
    socle::date::premier_du_mois(mois).map_err(|m| erreur("ValueError", m))?;
    for a in engagement::imprevus::du_mois(&ctx.racine, mois, None).map_err(crate::routes::imprevus::refus_magasin)? {
        let genre = engagement::imprevus::genre(&a).to_string();
        let rangement = engagement::imprevus::jour_de_rangement(&a).map_err(|m| erreur("ValueError", m))?;
        let Some((i, mut liste)) = crate::routes::imprevus::trouver(ctx, &genre, &valeur_en_texte(&a["salarie"]), &rangement)? else {
            return Err(erreur("ErreurOperation", format!("Aucun imprevu ({}) de {} au {}.", genre, valeur_en_texte(&a["salarie"]), rangement.jj_mm_aaaa())));
        };
        let deja = liste[i]["envois"].as_array().is_some_and(|e| {
            e.iter().any(|r| r.get("nature").and_then(Value::as_str) == Some("01") && r.get("mois").and_then(Value::as_str) == Some(mois))
        });
        if deja {
            continue;
        }
        let quand = if quand.is_empty() { maintenant() } else { quand.to_string() };
        liste[i]["envois"].as_array_mut().ok_or_else(|| erreur("AttributeError", "'envois' n'est pas une liste"))?.push(json!({
            "nature": "01", "type": "01", "ordre": 0, "mois": mois,
            "fichier": fichier.to_string_lossy(), "quand": quand, "identifiant": "",
        }));
        crate::routes::imprevus::ecrire_arrets(ctx, &rangement.mois_iso(), liste)?;
    }
    Ok(())
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
