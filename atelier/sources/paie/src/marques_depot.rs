//! Ce qu'un depot REEL laisse derriere lui, hors du journal des depots (`moteur/operations_arrets.py`
//! `marquer_envoye_depuis` et `marquer_reportes`, `operations_amorcage.marquer_demande_depuis`,
//! `operations_regularisation.marquer_portees` et `marquer_preparees`, `pas.enregistrer_demande`) -- lot H2.
//!
//! Chaque fonction ecrit la trace d'un acte parti : la DSN qui a reporte un arret, le signalement d'un
//! arret, la demande d'un taux de PAS a la DGFiP, la correction portee. Rien d'ici ne fait partir quoi que ce
//! soit ; un depot de TEST n'y laisse rien (l'appelant ne les appelle qu'apres un reel).

use std::path::Path;

use serde_json::{json, Value};

use droit::contexte::Contexte;
use socle::date::Jour;
use socle::valeur::py_str;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::vrai;

/// `datetime.now().strftime("%Y-%m-%d %H:%M")`, a l'heure de Paris.
fn maintenant() -> String {
    crate::routes_ecritures_g2::maintenant()
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

/// `re.search(r"^<rubrique>,'(.*)'$", texte, re.M)` : la valeur de la premiere ligne qui la porte.
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
    crate::imprevus::du_mois(&ctx.racine, mois, Some(crate::imprevus::ARRET)).map_err(crate::routes_listes::refus_magasin)
}

/// `operations_arrets._mois_de` : le mois ou l'arret commence.
fn mois_de(arret: &Value) -> String {
    py_str(&arret["dernier_jour_travaille"]).chars().take(7).collect()
}

/// `_trouver`, avec `ArretIntrouvable` rendu en `None`.
fn trouver_ou_rien(ctx: &Contexte, salarie: &Value, dernier_jour: &Value) -> R<Option<(usize, Vec<Value>)>> {
    match crate::routes_ecritures_g2::trouver(ctx, salarie, dernier_jour) {
        Ok(t) => Ok(Some(t)),
        Err(e) if e.classe == Some("ErreurOperation") && e.message.starts_with("Aucun arret de ") => Ok(None),
        Err(e) => Err(e),
    }
}

/// `operations_arrets.marquer_envoye_depuis` : trace l'envoi d'un signalement a partir du FICHIER qui vient
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
    crate::routes_ecritures_g2::ecrire_arrets(ctx, &mois_de(&arret), liste)?;
    Ok(Some(arret))
}

/// Note, sur chaque imprevu (arret ou suspension) que la DSN mensuelle REELLE de `mois` a declare, qu'elle l'a reporte
/// (un envoi de nature 01) : un imprevu declare ne s'oublie plus.
pub(crate) fn marquer_reportes(ctx: &Contexte, mois: &str, fichier: &Path, quand: &str) -> R<()> {
    socle::date::premier_du_mois(mois).map_err(|m| erreur("ValueError", m))?;
    for a in crate::imprevus::du_mois(&ctx.racine, mois, None).map_err(crate::routes_listes::refus_magasin)? {
        let genre = crate::imprevus::genre(&a).to_string();
        let rangement = crate::imprevus::jour_de_rangement(&a).map_err(|m| erreur("ValueError", m))?;
        let Some((i, mut liste)) = crate::routes_imprevus::trouver(ctx, &genre, &py_str(&a["salarie"]), &rangement)? else {
            return Err(erreur("ErreurOperation", format!("Aucun imprevu ({}) de {} au {}.", genre, py_str(&a["salarie"]), rangement.jj_mm_aaaa())));
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
        crate::routes_ecritures_g2::ecrire_arrets(ctx, &rangement.mois_iso(), liste)?;
    }
    Ok(())
}

// --------------------------------------------------------------------- l'amorcage

/// `pas._jours_ouvres_apres` : le jour ouvre qui tombe `combien` jours ouvres apres `depart` -- ni samedi,
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
    let mut data = crate::pas::charger(ctx).map_err(|m| erreur("OSError", m))?;
    for cle in cles {
        data["demandes"][cle.as_str()] = json!({
            "quand": jour.iso(), "motif": motif, "reponse_attendue_avant": avant.iso(), "flux": flux,
        });
    }
    if !cles.is_empty() {
        let chemin = ctx.racine.join("config").join("pas.json");
        std::fs::write(&chemin, socle::ecriture::json_py(&data, 2)).map_err(|e| erreur("OSError", format!("{} : {}", chemin.display(), e)))?;
    }
    Ok(())
}

/// `operations_amorcage.marquer_demande_depuis` : note la demande a partir du FICHIER qui vient de partir.
/// C'est ici que la demande commence a courir -- ce qui fait courir les cinq jours ouvres, c'est le depot.
pub(crate) fn marquer_demande_depuis(ctx: &Contexte, chemin: &Path, quand: &str) -> R<()> {
    let texte = lire_latin1(chemin)?;
    let (Some(nir), Some(motif)) = (valeur_de_rubrique(&texte, "S21.G00.30.001"), valeur_de_rubrique(&texte, "S20.G00.05.011")) else {
        return Ok(());
    };
    let Some(cle) = cle_du_nir(ctx, &nir) else { return Ok(()) };
    enregistrer_demande(ctx, &[cle], &motif, Some(quand), "")
}

// ------------------------------------------------------------------ la regularisation

/// `operations_regularisation.marquer_portees` : la DSN de `mois_courant` (ce fichier) vient d'etre DEPOSEE
/// en reel -- les corrections qu'elle avait preparees sont portees.
pub(crate) fn marquer_portees(ctx: &Contexte, mois_courant: &str, fichier: &Path, quand: &str) -> R<()> {
    let nom = fichier.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut mois: Vec<String> = crate::routes_ecrites::toutes_les_corrections(ctx)?.iter().map(|c| py_str(&c["mois"])).collect();
    mois.sort();
    mois.dedup();
    for m in mois {
        let mut corrections = crate::routes_ecrites::lire_corrections(ctx, &m)?;
        let mut touche = false;
        for c in corrections.iter_mut() {
            let prep = c.get("preparee_par").cloned().unwrap_or(Value::Null);
            let preparee_ici = vrai(&prep)
                && prep.get("mois").and_then(Value::as_str) == Some(mois_courant)
                && prep.get("fichier").and_then(Value::as_str) == Some(nom.as_str());
            if preparee_ici && !c.get("annulee_le").is_some_and(vrai) {
                let q = if quand.is_empty() { maintenant() } else { quand.to_string() };
                c.insert("portee_par".into(), json!({"mois": mois_courant, "fichier": nom, "quand": q}));
                touche = true;
            }
        }
        if touche {
            crate::routes_ecritures_g2::ecrire_corrections(ctx, &m, corrections)?;
        }
    }
    Ok(())
}

/// `operations_regularisation.marquer_preparees` : la DSN de `mois_courant` vient d'etre ECRITE avec ces
/// corrections ((mois d'origine, numero) de chacune).
pub(crate) fn marquer_preparees(ctx: &Contexte, corrections: &[(String, i64)], mois_courant: &str, fichier: &Path) -> R<()> {
    let q = maintenant();
    let nom = fichier.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut par_mois: Vec<(String, Vec<i64>)> = Vec::new();
    for (m, n) in corrections {
        match par_mois.iter_mut().find(|(x, _)| x == m) {
            Some((_, v)) => v.push(*n),
            None => par_mois.push((m.clone(), vec![*n])),
        }
    }
    for (m, numeros) in par_mois {
        let mut lues = crate::routes_ecrites::lire_corrections(ctx, &m)?;
        for c in lues.iter_mut() {
            if c.get("numero").and_then(Value::as_i64).is_some_and(|n| numeros.contains(&n)) {
                c.insert("preparee_par".into(), json!({"mois": mois_courant, "fichier": nom, "quand": q}));
            }
        }
        crate::routes_ecritures_g2::ecrire_corrections(ctx, &m, lues)?;
    }
    Ok(())
}
