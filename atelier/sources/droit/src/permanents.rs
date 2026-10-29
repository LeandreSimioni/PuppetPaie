//! Ce qu'un contrat permanent a de plus : conges, absences, fin de contrat.
//!
//! Le module calcule, il ne decide pas : les nombres vivent dans
//! `permanents-base.json`, avec l'article qui les porte.
//!
//! Les resultats sont des `serde_json::Value` ; un `Decimal` y est
//! ecrit en chaine (`"149.66"`), un entier en nombre.

use crate::natures::Nature;
use socle::date::{jours_du_mois, Jour};
use socle::feries::date_ou_refus;
use socle::fichiers::{dossier_du_code, json_en_cache, repr, vrai};
use socle::num::{add, sub, dec, dec_f64, dec_avec_echelle, div, eur2, mul, zero2, Decimal, Resultat};
use rust_decimal::prelude::*;
use serde_json::{json, Map, Value};
use std::sync::Arc;

/// Une tranche d'indemnisation du maintien : sa duree et son taux (en %).
#[derive(Debug, Clone, PartialEq)]
pub struct Tranche {
    pub jours: i64,
    pub taux: Decimal,
}

/// Les arguments nommes de `maintien_de_salaire`.
#[derive(Debug, Clone, Default)]
pub struct OptionsMaintien {
    pub deja_indemnises: i64,
    pub accident_du_travail: bool,
    pub accident_de_trajet: bool,
}

// ------------------------------------------------------------ les aides
fn cle<'a>(v: &'a Value, k: &str) -> Resultat<&'a Value> {
    v.get(k).ok_or_else(|| socle::fichiers::absent(k))
}

/// Le decimal de la valeur d'une cle du fichier.
fn nombre(v: &Value, k: &str) -> Resultat<Decimal> {
    dec_avec_echelle(cle(v, k)?)
}

fn entier(v: &Value) -> Resultat<i64> {
    match v {
        Value::Number(n) => n
            .as_i64()
            .or_else(|| n.as_f64().map(|x| x.trunc() as i64))
            .ok_or_else(|| socle::fichiers::entier_hors_limites(&n.to_string())),
        Value::String(s) => s
            .trim()
            .parse::<i64>()
            .map_err(|_| format!("nombre entier illisible : {}", repr(s))),
        autre => Err(socle::fichiers::attendu("un nombre entier", socle::valeur::nom_type(autre))),
    }
}

/// Un `Decimal` dans un resultat : sa chaine, a son echelle.
fn dv(x: Decimal) -> Value {
    Value::String(x.to_string())
}

fn douze() -> Decimal {
    Decimal::from(12)
}

fn motif_texte(m: Option<&str>) -> &str {
    m.unwrap_or("")
}

/// Le fichier de regles, tel qu'ecrit.
pub fn regles() -> Resultat<Arc<Value>> {
    json_en_cache(&dossier_du_code().join("permanents-base.json"))
}

pub fn source() -> Resultat<Value> {
    Ok(match regles()?.get("_source") {
        Some(s) if vrai(s) => s.clone(),
        _ => Value::Object(Map::new()),
    })
}

// --------------------------------------------------------------- les conges
/// Les bornes de la periode de reference qui contient `jour` : du 1er juin
/// au 31 mai, sauf accord (`conges_payes.debut_periode_mois`).
pub fn periode_de_reference(jour: &Jour, entreprise: Option<&Value>) -> Resultat<(Jour, Jour)> {
    let rg = regles()?;
    let r = cle(cle(&rg, "conges_payes")?, "periode_de_reference")?;
    let dit = entreprise
        .and_then(|e| e.get("conges_payes"))
        .filter(|c| vrai(c))
        .and_then(|c| c.get("debut_periode_mois"))
        .filter(|m| vrai(m));
    let mois = match dit {
        Some(m) => entier(m)?,
        None => entier(cle(r, "debut_mois")?)?,
    };
    let dj = entier(cle(r, "debut_jour")?)?;
    let an = if (jour.mois as i64, jour.jour as i64) >= (mois, dj) { jour.an } else { jour.an - 1 } as i64;
    let debut = date_ou_refus(an, mois, dj)?;
    let fin = date_ou_refus(an + 1, mois, dj)?.plus_jours(-1)?;
    Ok((debut, fin))
}

/// Les jours ouvrables acquis sur une periode de presence : 2,5 par mois,
/// au prorata des jours calendaires de chaque mois traverse.
pub fn jours_acquis(debut: &Jour, fin: &Jour) -> Resultat<Decimal> {
    let rg = regles()?;
    let par_mois = nombre(cle(cle(&rg, "conges_payes")?, "acquisition")?, "jours_par_mois")?;
    if fin < debut {
        return Ok(Decimal::ZERO);
    }
    let (mut total, mut jour) = (Decimal::ZERO, *debut);
    while jour <= *fin {
        let dans_le_mois = jours_du_mois(jour.an, jour.mois);
        let derniere = Jour { an: jour.an, mois: jour.mois, jour: dans_le_mois };
        let borne = if *fin < derniere { *fin } else { derniere };
        let jours = Decimal::from(borne.moins(&jour) + 1);
        total = add(total, div(mul(par_mois, jours), Decimal::from(dans_le_mois)));
        jour = borne.plus_jours(1)?;
    }
    Ok(total)
}

/// Trente jours ouvrables au plus.
pub fn plafonner(acquis: Decimal) -> Resultat<Decimal> {
    let rg = regles()?;
    let maxi = nombre(cle(cle(&rg, "conges_payes")?, "acquisition")?, "maximum_annuel")?;
    Ok(acquis.min(maxi))
}

/// Le solde affiche : au demi-jour superieur.
pub fn arrondir_jours(jours: Decimal) -> Decimal {
    let demi = (jours * Decimal::from(2)).ceil();
    div(demi, Decimal::from(2)).normalize()
}

/// L'indemnite pour des jours de conges pris : le dixieme, le maintien, et
/// celui qui gagne.
pub fn indemnite_conges(
    jours_pris: Decimal,
    remuneration_periode: Decimal,
    jours_acquis_periode: Decimal,
    maintien: Decimal,
) -> Resultat<Value> {
    let rg = regles()?;
    let taux = nombre(cle(cle(&rg, "conges_payes")?, "indemnisation")?, "dixieme")?;
    let dixieme = if jours_acquis_periode > Decimal::ZERO {
        eur2(div(mul(mul(remuneration_periode, taux), jours_pris), jours_acquis_periode))
    } else {
        zero2()
    };
    let maintien = eur2(maintien);
    Ok(json!({
        "dixieme": dv(dixieme),
        "maintien": dv(maintien),
        "retenu": dv(dixieme.max(maintien)),
        "regle": if dixieme > maintien { "dixieme" } else { "maintien" },
    }))
}


/// Des jours de conges acquis sur une periode de reference, et la remuneration de cette periode.
#[derive(Debug, Clone, Copy)]
pub struct Acquisition {
    pub jours: Decimal,
    pub remuneration: Decimal,
}

/// L'indemnite de `jours` de conges (L3141-24), chaque jour au dixieme DE LA PERIODE OU IL A ETE ACQUIS : les
/// `report_restant` jours du report d'abord (au dixieme de `report`), puis la periode `en_cours`. Le maintien se
/// compare au TOTAL.
pub fn indemnite_des_jours(
    jours: Decimal,
    report_restant: Decimal,
    report: Acquisition,
    en_cours: Acquisition,
    maintien: Decimal,
) -> Resultat<crate::conges::IndemniteDesJours> {
    let sur_report = jours.min(report_restant);
    let reste = sub(jours, sur_report);
    let mut dixieme = zero2();
    let dixieme_de = |jours: Decimal, a: Acquisition| -> Resultat<Decimal> {
        let v = indemnite_conges(jours, a.remuneration, if a.jours != Decimal::ZERO { a.jours } else { jours }, Decimal::ZERO)?;
        dec(v.get("dixieme").ok_or_else(|| socle::fichiers::absent("dixieme"))?)
    };
    if sur_report != Decimal::ZERO {
        dixieme += dixieme_de(sur_report, report)?;
    }
    if reste != Decimal::ZERO {
        dixieme += dixieme_de(reste, en_cours)?;
    }
    Ok(crate::conges::la_plus_forte(dixieme, maintien))
}
// ------------------------------------------------------- la fin du contrat
/// `None` si l'indemnite de fin de CDD est due, sinon pourquoi elle ne l'est pas.
pub fn precarite_due(nature: &Nature, motif_rupture: Option<&str>) -> Resultat<Option<String>> {
    let rg = regles()?;
    let r = cle(&rg, "indemnite_precarite")?;
    if nature.duree_indeterminee {
        return Ok(Some(
            "un contrat a duree indeterminee n'a pas d'indemnite de fin \
             de contrat : elle compense la precarite d'un terme."
                .to_string(),
        ));
    }
    let exclues = r.get("natures_exclues").and_then(|e| e.as_array());
    if exclues.map_or(false, |e| e.iter().any(|x| x.as_str() == Some(nature.cle.as_str()))) {
        return Ok(Some(cle(r, "_natures_exclues")?.as_str().unwrap_or("").to_string()));
    }
    Ok(r.get("motifs_rupture_exclus")
        .and_then(|m| m.get(motif_texte(motif_rupture)))
        .and_then(|m| m.as_str())
        .map(|s| s.to_string()))
}

/// 10 % de la remuneration totale brute versee pendant le CDD ; `taux` est
/// le taux reduit, et seulement quand un accord le prevoit.
pub fn indemnite_precarite(remuneration_totale: Decimal, taux: Option<f64>) -> Resultat<Decimal> {
    let rg = regles()?;
    let r = cle(&rg, "indemnite_precarite")?;
    let t = match taux {
        Some(x) => dec_f64(x)?,
        None => nombre(r, "taux")?,
    };
    Ok(eur2(mul(remuneration_totale, t)))
}

/// L'anciennete en mois : les mois complets et la fraction du mois entame.
pub fn anciennete_mois(debut: &Jour, fin: &Jour) -> Decimal {
    if fin < debut {
        return Decimal::ZERO;
    }
    let mut mois = (fin.an as i64 - debut.an as i64) * 12 + (fin.mois as i64 - debut.mois as i64);
    let anniv = debut.jour as i64;
    let reste = if fin.jour as i64 + 1 >= anniv {
        fin.jour as i64 + 1 - anniv
    } else {
        mois -= 1;
        let (an, precedent) = if fin.mois > 1 { (fin.an, fin.mois - 1) } else { (fin.an - 1, 12) };
        jours_du_mois(an, precedent) as i64 + fin.jour as i64 + 1 - anniv
    };
    let dans_le_mois = Decimal::from(jours_du_mois(fin.an, fin.mois));
    add(Decimal::from(mois), div(Decimal::from(reste), dans_le_mois))
}

/// Ce que `preavis.rupture_anticipee_cdd` dit du motif d'un contrat a duree determinee : `Some(true)` aucun preavis,
/// `Some(false)` un preavis que le moteur ne compte pas (L1243-2), avec le texte ; `None` hors de la section.
fn regle_du_cdd(cdd: bool, motif_rupture: Option<&str>) -> Resultat<Option<(bool, String)>> {
    if !cdd {
        return Ok(None);
    }
    let rg = regles()?;
    let sec = cle(&rg, "preavis")?.get("rupture_anticipee_cdd");
    let m = motif_texte(motif_rupture);
    for (cle_liste, sans) in [("motifs_sans_preavis", true), ("motifs_a_dire", false)] {
        if let Some(t) = sec.and_then(|s| s.get(cle_liste)).and_then(|l| l.get(m)).and_then(Value::as_str) {
            return Ok(Some((sans, t.to_string())));
        }
    }
    Ok(None)
}

/// La duree du preavis en mois, ou `None` quand la loi ne la donne pas (ou que le moteur ne sait pas la compter :
/// `regle_du_preavis`). `cdd` : le contrat est a duree determinee, et ses motifs de rupture anticipee ont leur regle.
pub fn preavis_mois(anciennete: Decimal, motif_rupture: Option<&str>, cdd: bool) -> Resultat<Option<Decimal>> {
    if let Some((sans, _)) = regle_du_cdd(cdd, motif_rupture)? {
        return Ok(if sans { Some(Decimal::ZERO) } else { None });
    }
    let rg = regles()?;
    let r = cle(&rg, "preavis")?;
    let sans = r.get("motifs_sans_preavis").and_then(|m| m.get(motif_texte(motif_rupture)));
    if sans.map_or(false, vrai) {
        return Ok(Some(Decimal::ZERO));
    }
    for t in cle(r, "tranches")?.as_array().ok_or_else(|| socle::fichiers::absent("tranches"))? {
        let haut = match t.get("anciennete_mois_max") {
            None | Some(Value::Null) => None,
            Some(h) => Some(dec(h)?),
        };
        if anciennete >= nombre(t, "anciennete_mois_min")? && haut.map_or(true, |h| anciennete < h) {
            return match cle(t, "mois")? {
                Value::Null => Ok(None),
                m => Ok(Some(dec(m)?)),
            };
        }
    }
    Ok(None)
}

/// Le preavis que la loi donne pour ce motif d'un CDD sans que le moteur sache le compter (`motifs_a_dire` : L1243-2).
pub fn regle_du_preavis(motif_rupture: Option<&str>, cdd: bool) -> Resultat<Option<String>> {
    Ok(regle_du_cdd(cdd, motif_rupture)?.filter(|(sans, _)| !sans).map(|(_, t)| t))
}

/// Pourquoi la loi ne donne aucun preavis pour ce motif (`preavis.motifs_sans_preavis`, et pour un CDD
/// `preavis.rupture_anticipee_cdd.motifs_sans_preavis`), quand il est de cette liste.
pub fn raison_sans_preavis(motif_rupture: Option<&str>, cdd: bool) -> Resultat<Option<String>> {
    if let Some((sans, t)) = regle_du_cdd(cdd, motif_rupture)? {
        return Ok(if sans { Some(t) } else { None });
    }
    let rg = regles()?;
    let r = cle(&rg, "preavis")?;
    Ok(r.get("motifs_sans_preavis")
        .and_then(|m| m.get(motif_texte(motif_rupture)))
        .and_then(Value::as_str)
        .filter(|t| !t.is_empty())
        .map(str::to_string))
}

/// Ce que le texte fait du preavis non effectue, pour ce motif (`preavis.indemnite_compensatrice`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndemniteDePreavis {
    /// L1234-5, l'indemnite du salarie licencie : le motif n'est dans aucune des listes.
    Licenciement,
    /// Aucune indemnite (`motifs_sans_indemnite`), avec le texte qui le dit.
    Aucune(String),
    /// Une indemnite d'un montant egal, par un autre article (`motifs_par_un_autre_article`).
    AutreArticle(String),
    /// Aucun texte depose ne tranche (`motifs_a_trancher`) : le moteur paie l'indemnite (L1234-5), et le dit.
    ATrancher(String),
}

/// L'indemnite compensatrice d'un preavis non effectue, selon le motif de rupture. L1234-5 est au chapitre du
/// licenciement d'un CDI ; les autres fins de contrat ont leurs articles (`code-du-travail/articles-20261013.md`,
/// « Le preavis non effectue : ce que chaque fin de contrat en fait »). Un motif sans preavis ne passe pas par ici.
pub fn indemnite_de_preavis(motif_rupture: Option<&str>) -> Resultat<IndemniteDePreavis> {
    let rg = regles()?;
    let ic = cle(cle(&rg, "preavis")?, "indemnite_compensatrice")?;
    let m = motif_texte(motif_rupture);
    let dit = |liste: &str| ic.get(liste).and_then(|l| l.get(m)).and_then(Value::as_str).map(str::to_string);
    Ok(if let Some(t) = dit("motifs_sans_indemnite") {
        IndemniteDePreavis::Aucune(t)
    } else if let Some(t) = dit("motifs_par_un_autre_article") {
        IndemniteDePreavis::AutreArticle(t)
    } else if let Some(t) = dit("motifs_a_trancher") {
        IndemniteDePreavis::ATrancher(t)
    } else {
        IndemniteDePreavis::Licenciement
    })
}

/// La rubrique de la ligne qui paie le preavis non effectue, selon le motif : `preavis.indemnite_compensatrice.rubrique`
/// (`indemnite_preavis`, DSN 52 « 023 »), sauf le motif qui a la sienne (`rubrique_par_motif` : le 092, L1226-14,
/// `indemnite_preavis_inaptitude`, DSN 52 « 032 », cahier technique 2026.1 p. 253, `norme-dsn/texte/bloc-S21.G00.52.txt`).
/// Le code du bloc 52 est celui de la rubrique : il suit le motif comme l'indemnite.
pub fn rubrique_de_l_indemnite_de_preavis(motif_rupture: Option<&str>) -> Resultat<String> {
    let rg = regles()?;
    let ic = cle(cle(&rg, "preavis")?, "indemnite_compensatrice")?;
    if let Some(r) = ic.get("rubrique_par_motif").and_then(|p| p.get(motif_texte(motif_rupture))).and_then(Value::as_str) {
        return Ok(r.to_string());
    }
    Ok(cle(ic, "rubrique")?.as_str().ok_or_else(|| socle::fichiers::absent("rubrique"))?.to_string())
}

// ------------------------------------------------ le delai de prevenance d'une fin d'essai (L1221-25)

/// La section `preavis.delai_de_prevenance` si ce motif est le sien (034 : la fin d'essai a l'initiative de l'employeur),
/// sinon `None` : le 035 (L1221-26) n'ouvre rien au salarie.
pub fn regle_du_delai_de_prevenance(motif_rupture: Option<&str>) -> Resultat<Option<Value>> {
    let rg = regles()?;
    let s = cle(cle(&rg, "preavis")?, "delai_de_prevenance")?;
    Ok(if s.get("motif_rupture").and_then(Value::as_str) == Some(motif_texte(motif_rupture)) { Some(s.clone()) } else { None })
}

/// Le delai de prevenance que L1221-25 donne a cette presence, et le jour ou il expire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelaiDePrevenance {
    /// Ce que la tranche dit (« quarante-huit heures (L1221-25, 2° : ...) »).
    pub texte: String,
    /// Les jours de presence, du premier jour du contrat a la notification, l'un et l'autre compris.
    pub presence_jours: i64,
    /// Le dernier jour du delai : le salarie l'a recu si le contrat finit ce jour-la ou apres.
    pub expiration: Jour,
}

/// Le meme quantieme `n` mois plus tard, ou le dernier jour du mois s'il ne l'a pas.
fn plus_mois(j: &Jour, n: u32) -> Resultat<Jour> {
    let rang = j.mois - 1 + n;
    let (an, mois) = (j.an + (rang / 12) as i32, rang % 12 + 1);
    Jour::nouveau(an, mois, j.jour.min(jours_du_mois(an, mois))).ok_or_else(|| socle::date::date_impossible(an, mois, j.jour))
}

/// L1221-25 : le delai de prevenance d'une fin d'essai notifiee le `notification` dans un contrat commence le `debut`
/// (`preavis.delai_de_prevenance.tranches`). Le decompte est une lecture, ecrite dans `_lecture` : la presence va du
/// debut a la notification compris ; le delai court du lendemain de la notification, en jours entiers, ou jusqu'au meme
/// quantieme du mois suivant.
pub fn delai_de_prevenance(debut: &Jour, notification: &Jour) -> Resultat<DelaiDePrevenance> {
    let rg = regles()?;
    let s = cle(cle(&rg, "preavis")?, "delai_de_prevenance")?;
    let tranches = cle(s, "tranches")?.as_array().ok_or_else(|| socle::fichiers::absent("tranches"))?;
    let presence_jours = notification.moins(debut) + 1;
    let presence_mois = anciennete_mois(debut, notification);
    let tranche = tranches
        .iter()
        .find(|t| match (t.get("presence_moins_de_jours"), t.get("presence_jusqu_a_mois")) {
            (Some(j), _) => j.as_i64().is_some_and(|j| presence_jours < j),
            (None, Some(m)) => m.as_i64().is_some_and(|m| presence_mois <= Decimal::from(m)),
            (None, None) => true,
        })
        .ok_or_else(|| "permanents-base.json, preavis.delai_de_prevenance : aucune tranche pour cette presence.".to_string())?;
    let expiration = match (tranche.get("delai_jours"), tranche.get("delai_mois")) {
        (Some(j), _) => notification.plus_jours(entier(j)?)?,
        (None, Some(m)) => plus_mois(notification, entier(m)? as u32)?,
        (None, None) => return Err(socle::fichiers::absent("delai_jours")),
    };
    Ok(DelaiDePrevenance { texte: socle::date::en_texte(cle(tranche, "texte")?), presence_jours, expiration })
}

/// Le salaire de reference de l'indemnite de licenciement : la moyenne des
/// douze derniers mois ou le tiers des trois derniers, le plus favorable.
pub fn salaire_de_reference(douze_derniers: &[Decimal], trois_derniers: &[Decimal]) -> Value {
    let moyenne = if douze_derniers.is_empty() {
        zero2()
    } else {
        let s = douze_derniers.iter().fold(Decimal::ZERO, |a, b| add(a, *b));
        eur2(div(s, Decimal::from(douze_derniers.len() as i64)))
    };
    let tiers = if trois_derniers.is_empty() {
        zero2()
    } else {
        let s = trois_derniers.iter().fold(Decimal::ZERO, |a, b| add(a, *b));
        eur2(div(s, Decimal::from(3)))
    };
    json!({
        "moyenne_12_mois": dv(moyenne),
        "tiers_3_mois": dv(tiers),
        "retenu": dv(moyenne.max(tiers)),
        "regle": if moyenne >= tiers { "moyenne_12_mois" } else { "tiers_3_mois" },
    })
}

/// L'indemnite legale de licenciement : `{montant, du, pourquoi}` (+ `annees`
/// quand elle est due).
pub fn indemnite_licenciement(
    anciennete: Decimal,
    reference: Decimal,
    motif_rupture: Option<&str>,
) -> Resultat<Value> {
    let rg = regles()?;
    let r = cle(&rg, "indemnite_licenciement")?;
    let exclu = r
        .get("motifs_sans_indemnite")
        .and_then(|m| m.get(motif_texte(motif_rupture)))
        .filter(|e| vrai(e));
    if let Some(e) = exclu {
        return Ok(json!({"montant": dv(zero2()), "du": false, "pourquoi": e}));
    }
    let mini = nombre(r, "anciennete_minimale_mois")?;
    if anciennete < mini {
        return Ok(json!({"montant": dv(zero2()), "du": false, "pourquoi": format!(
            "{} mois d'anciennete, et l'indemnite legale de licenciement en demande {} (L1234-9).",
            arrondir_jours(anciennete), mini)}));
    }
    let (montant, annees) = bareme_licenciement(anciennete, reference)?;
    Ok(json!({"montant": dv(montant), "du": true, "pourquoi": null, "annees": dv(annees)}))
}

/// Le montant de R1234-2, sans condition ni exclusion : un quart de mois
/// par annee jusqu'a dix ans, un tiers au-dela. Sert au licenciement, a la
/// mise a la retraite et a la rupture conventionnelle.
fn bareme_licenciement(anciennete: Decimal, reference: Decimal) -> Resultat<(Decimal, Decimal)> {
    let rg = regles()?;
    let r = cle(&rg, "indemnite_licenciement")?;
    let annees = div(anciennete, douze());
    let (mut total, mut bas) = (Decimal::ZERO, Decimal::ZERO);
    for t in cle(r, "taux")?.as_array().ok_or_else(|| socle::fichiers::absent("taux"))? {
        let haut = match cle(t, "jusqu_a_annees")? {
            Value::Null => annees,
            h => dec(h)?,
        };
        let part = add(annees.min(haut), -bas).max(Decimal::ZERO);
        total = add(total, mul(part, nombre(t, "fraction_de_mois")?));
        bas = haut;
        if annees <= haut {
            break;
        }
    }
    Ok((eur2(mul(total, reference)), annees))
}

// ------------------------------------- les trois autres fins d'un CDI
/// La section de regles que ce motif designe (043 rupture conventionnelle,
/// 038 mise a la retraite, 039 depart a la retraite), avec `_cle`.
pub fn rupture(motif_rupture: Option<&str>) -> Resultat<Option<Value>> {
    let rg = regles()?;
    let m = motif_texte(motif_rupture);
    for c in ["rupture_conventionnelle", "mise_a_la_retraite", "depart_a_la_retraite"] {
        let sec = match rg.get(c) {
            Some(s) if vrai(s) => s.clone(),
            _ => Value::Object(Map::new()),
        };
        if sec.get("motif_rupture").and_then(|x| x.as_str()) == Some(m) {
            let mut o = sec.as_object().cloned().unwrap_or_default();
            o.insert("_cle".to_string(), Value::String(c.to_string()));
            return Ok(Some(Value::Object(o)));
        }
    }
    Ok(None)
}

/// `None` si ce code d'indemnite peut accompagner ce motif (CCH-21 a
/// CCH-24), sinon pourquoi non.
pub fn code_52_permis(code: Option<&str>, motif_rupture: Option<&str>) -> Resultat<Option<String>> {
    let code = match code {
        Some(c) if !c.is_empty() => c,
        _ => return Ok(None),
    };
    let rg = regles()?;
    let r = cle(&rg, "indemnites_de_rupture")?;
    let dit = match r.get("codes").and_then(|c| c.get(code)) {
        Some(d) => d,
        None => return Ok(None),
    };
    let motif = motif_texte(motif_rupture);
    let motifs = cle(dit, "motifs")?.as_array().ok_or_else(|| socle::fichiers::absent("motifs"))?;
    if motif.is_empty() || motifs.iter().any(|m| m.as_str() == Some(motif)) {
        return Ok(None);
    }
    let liste: Vec<&str> = motifs.iter().filter_map(|m| m.as_str()).collect();
    Ok(Some(format!(
        "{} ({}) n'est permis qu'avec les motifs de rupture {}, et celui-ci est {} -- {} du cahier technique.",
        code,
        cle(dit, "libelle")?.as_str().unwrap_or(""),
        liste.join(", "),
        motif,
        cle(dit, "controle")?.as_str().unwrap_or("")
    )))
}

/// L'indemnite de mise a la retraite : le bareme du licenciement, sans ses
/// exclusions de motif.
pub fn indemnite_mise_a_la_retraite(anciennete: Decimal, reference: Decimal) -> Resultat<Value> {
    let rg = regles()?;
    let mini = nombre(cle(&rg, "indemnite_licenciement")?, "anciennete_minimale_mois")?;
    if anciennete < mini {
        return Ok(json!({"montant": dv(zero2()), "du": false, "pourquoi": format!(
            "{} mois d'anciennete : L1237-7 renvoie a l'indemnite de L1234-9, qui en demande {}. \
             La convention collective peut en prevoir une.",
            arrondir_jours(anciennete), mini)}));
    }
    let (montant, annees) = bareme_licenciement(anciennete, reference)?;
    Ok(json!({"montant": dv(montant), "du": true, "pourquoi": null, "annees": dv(annees)}))
}

/// Le plancher de l'indemnite de rupture conventionnelle ; `None` sous
/// huit mois d'anciennete.
pub fn indemnite_rupture_conventionnelle(anciennete: Decimal, reference: Decimal) -> Resultat<Value> {
    let rg = regles()?;
    let mini = nombre(cle(&rg, "indemnite_licenciement")?, "anciennete_minimale_mois")?;
    if anciennete < mini {
        return Ok(json!({"plancher": null, "annees": dv(div(anciennete, Decimal::from(12))), "pourquoi": format!(
            "{} mois d'anciennete : L1237-13 renvoie a l'indemnite de L1234-9, qui en demande {}, \
             donc la loi ne donne pas de plancher. Le montant de la convention se saisit.",
            arrondir_jours(anciennete), mini)}));
    }
    let (plancher, annees) = bareme_licenciement(anciennete, reference)?;
    Ok(json!({"plancher": dv(plancher), "annees": dv(annees), "pourquoi": null}))
}

/// L'indemnite de depart volontaire a la retraite : un PALIER, pas un cumul,
/// et rien sous le premier.
pub fn indemnite_depart_retraite(anciennete: Decimal, reference: Decimal) -> Resultat<Value> {
    let rg = regles()?;
    let sec = cle(cle(&rg, "depart_a_la_retraite")?, "indemnite")?;
    let annees = div(anciennete, douze());
    let taux = cle(sec, "taux")?.as_array().ok_or_else(|| socle::fichiers::absent("taux"))?;
    let mut tries: Vec<(Decimal, &Value)> = Vec::new();
    for t in taux {
        tries.push((nombre(t, "anciennete_annees")?, t));
    }
    tries.sort_by(|a, b| a.0.cmp(&b.0));
    let mut palier: Option<&Value> = None;
    for (seuil, t) in &tries {
        if annees >= *seuil {
            palier = Some(t);
        }
    }
    let palier = match palier {
        Some(p) => p,
        None => {
            let premier = tries.iter().map(|(_, t)| *t).min_by(|a, b| {
                let (x, y) = (dec(&a["anciennete_annees"]), dec(&b["anciennete_annees"]));
                x.unwrap_or_default().cmp(&y.unwrap_or_default())
            });
            let premier = match premier {
                Some(t) => socle::fichiers::repr_valeur(cle(t, "anciennete_annees")?),
                None => return Err("min() arg is an empty sequence".to_string()),
            };
            return Ok(json!({"montant": dv(zero2()), "du": false, "mois": dv(Decimal::ZERO),
                "palier": null, "pourquoi": format!(
                "{} an(s) d'anciennete, et l'indemnite legale de depart a la retraite commence a {} ans \
                 (D1237-1) -- il n'y a pas de prorata en dessous. La convention collective, elle, \
                 en prevoit souvent une des la premiere annee.",
                annees_lisibles(annees), premier)}));
        }
    };
    let mois = nombre(palier, "mois_de_salaire")?;
    Ok(json!({"montant": dv(eur2(mul(mois, reference))), "du": true, "mois": dv(mois),
        "palier": entier(cle(palier, "anciennete_annees")?)?, "pourquoi": null}))
}

/// `str(annees.quantize(Decimal("0.1")))` : un chiffre, au pair sur le demi.
fn annees_lisibles(annees: Decimal) -> String {
    let mut a = annees.round_dp_with_strategy(1, RoundingStrategy::MidpointNearestEven);
    a.rescale(1);
    a.to_string()
}

// -------------------------------------------------- le maintien de salaire
/// Pourquoi le maintien de salaire n'est PAS du, `None` s'il l'est
/// (`nature` absente : seule l'anciennete compte).
pub fn droit_au_maintien(nature: Option<&Nature>, anciennete_mois: Decimal) -> Resultat<Option<String>> {
    let rg = regles()?;
    let r = cle(&rg, "maintien_de_salaire")?;
    if let Some(n) = nature {
        if !n.mensuelle() {
            return Ok(Some(format!(
                "L1226-1 ne s'applique pas a un {} : l'indemnite complementaire est due aux \
                 salaries permanents, pas aux intermittents.",
                n.libelle
            )));
        }
    }
    let minimum = entier(cle(r, "anciennete_mois_minimum")?)?;
    if anciennete_mois < Decimal::from(minimum) {
        return Ok(Some(format!(
            "L1226-1 demande {} mois d'anciennete au premier jour de l'absence (D1226-8) ; il y en a {}.",
            minimum,
            arrondir_jours(anciennete_mois)
        )));
    }
    Ok(None)
}

/// Sept jours de carence, sauf accident du travail (l'accident de trajet
/// garde la sienne).
pub fn jours_de_carence(accident_du_travail: bool, accident_de_trajet: bool) -> Resultat<i64> {
    let rg = regles()?;
    let r = cle(&rg, "maintien_de_salaire")?;
    if accident_du_travail && !accident_de_trajet {
        return Ok(0);
    }
    entier(cle(r, "carence_jours")?)
}

/// Les tranches d'indemnisation, allongees par l'anciennete.
pub fn duree_du_maintien(anciennete_mois: Decimal) -> Resultat<Vec<Tranche>> {
    let rg = regles()?;
    let r = cle(&rg, "maintien_de_salaire")?;
    let maj = cle(r, "majoration")?;
    let reste = anciennete_mois % douze();
    let annees = div(sub(anciennete_mois, reste), douze()).trunc().to_i64().unwrap_or(0);
    let au_dela = (annees - 1).max(0);
    let bonus = (au_dela / entier(cle(maj, "par_annees")?)?) * entier(cle(maj, "jours")?)?;
    let plafond = entier(cle(maj, "plafond_jours")?)?;
    let mut out = Vec::new();
    for t in cle(r, "tranches")?.as_array().ok_or_else(|| socle::fichiers::absent("tranches"))? {
        let jours = (entier(cle(t, "jours")?)? + bonus).min(plafond);
        out.push(Tranche { jours, taux: nombre(t, "taux")? });
    }
    Ok(out)
}

/// Le complement BRUT du a un salarie pendant un arret, avant deduction des
/// indemnites de la Securite sociale et de la prevoyance (D1226-5).
pub fn maintien_de_salaire(
    anciennete_mois: Decimal,
    jours_d_absence: i64,
    remuneration_journaliere: Decimal,
    opt: &OptionsMaintien,
) -> Resultat<Value> {
    let carence = jours_de_carence(opt.accident_du_travail, opt.accident_de_trajet)?;
    let indemnisables = (jours_d_absence - carence).max(0);
    let (mut restant, mut reste_a_imputer) = (indemnisables, opt.deja_indemnises);
    let mut brut = zero2();
    let mut tranches = Vec::new();
    for t in duree_du_maintien(anciennete_mois)? {
        let capacite = t.jours;
        let consommes = capacite.min(reste_a_imputer);
        reste_a_imputer -= consommes;
        let disponible = capacite - consommes;
        let pris = restant.min(disponible);
        restant -= pris;
        if pris != 0 {
            // Les deux tiers se calculent en deux tiers, pas en 66,66 %.
            let taux = if t.taux < Decimal::from(67) {
                div(Decimal::from(2), Decimal::from(3))
            } else {
                div(t.taux, Decimal::ONE_HUNDRED)
            };
            let montant = eur2(mul(mul(remuneration_journaliere, Decimal::from(pris)), taux));
            brut += montant;
            tranches.push(json!({"jours": pris, "taux": dv(t.taux), "montant": dv(montant)}));
        }
    }
    brut.rescale(2);
    Ok(json!({
        "carence": carence,
        "jours_indemnisables": indemnisables,
        "jours_indemnises": indemnisables - restant,
        "jours_au_dela_du_droit": restant,
        "tranches": tranches,
        "brut_avant_deduction": dv(brut),
        "_a_deduire": "Les indemnites journalieres de la Securite sociale et la part patronale de \
                       prevoyance se deduisent de ce montant (D1226-5) : elles se saisissent, le \
                       moteur ne les devine pas.",
    }))
}
