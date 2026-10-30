//! Ce que la compagnie a declare, pour les cumuls des annexes 8 et 10 (`droit::cumul`) : les engagements importes, puis les
//! paies de ses engagements, une ligne declaree (`Declare`) par paie d'intermittent.

use serde_json::Value;

use crate::calcul::Paie;
use droit::contexte::Contexte;
use droit::cumul::{heures_enseignement, Declare};
use socle::date::d;
use socle::num::{dec_avec_echelle, Resultat};

// ------------------------------------------------------------------- lecture

/// Un entier d'un contrat importe (`defaut` s'il est absent) ; `None` : une valeur qui n'en est pas un, et la
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

/// Un contrat importe, ou `None` s'il est abime (il ne fait pas
/// tomber les autres).
fn ligne_importee(e: &Value) -> Resultat<Option<Declare>> {
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

/// Les contrats que l'import atteste (`importe::contrats_lus` : une AEM, un bulletin). Vide tant qu'on n'a rien importe.
pub(crate) fn lire_importes(ctx: &Contexte) -> Resultat<Vec<Declare>> {
    let mut out = Vec::new();
    for i in crate::importe::tous(&ctx.racine)? {
        let Some(mut e) = i.ligne_du_contrat() else { continue };
        // Un contrat qui continue ici : ce que les pieces attestent s'arrete a son dernier mois verse, la suite est
        // dans les paies de ce logiciel.
        if let Some(dernier) = i.continue_apres() {
            e["fin"] = Value::String(socle::date::dernier_du_mois(&dernier)?.iso());
        }
        if let Some(x) = ligne_importee(&e)? {
            out.push(x);
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

/// Ce que les engagements valides declarent. Chaque mois se lit dans les paies ecrites de ses engagements valides
/// (`etats::validees_du_mois`) : ce qui est en saisie n'est pas paye, il n'entre dans aucun cumul. `paies_courantes` : les
/// paies d'un mois deja lues par l'appelant ; elles remplacent celles de leur mois. `jusqu_au` : les mois d'apres ne se lisent pas.
pub(crate) fn engagements_des_paies(ctx: &Contexte, paies_courantes: Option<&[Paie]>, jusqu_au: Option<&str>) -> Resultat<Vec<Declare>> {
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
    for stem in crate::stock::mois_saisis(&ctx.racine)? {
        if groupes.iter().any(|(m, _)| *m == stem) || jusqu_au.is_some_and(|j| stem.as_str() > j) {
            continue;
        }
        let Some((_, paies)) = crate::etats::validees_du_mois(ctx, &stem)? else { continue };
        let mut es = Vec::new();
        for p in &paies {
            if let Some(e) = engagement_de_paie(ctx, p)? {
                es.push(e);
            }
        }
        groupes.push((stem, es));
    }
    Ok(groupes.into_iter().flat_map(|(_, es)| es).collect())
}

/// TOUT ce que la compagnie a declare : les engagements importes, puis les paies ecrites de ses engagements valides,
/// trie par (debut, fin, salarie). Un mois a la fois importe et saisi compte les deux : un engagement importe ne touche
/// aucun mois de la saisie (`stock::Objet::couvre`), aucune paie d'ici ne le declare une seconde fois.
pub fn lire_engagements(ctx: &Contexte, paies_courantes: Option<&[Paie]>) -> Resultat<Vec<Declare>> {
    lire_engagements_jusqu_au(ctx, paies_courantes, None)
}

/// `lire_engagements`, sans les mois d'apres `jusqu_au` : ce qu'un document de ce mois-la cumule (ses fenetres se
/// ferment a la fin du mois, `droit::cumul::tableau_cumul`). Un mois d'apres ne lui manque pas : un mois passe ne
/// depend pas des suivants.
pub fn lire_engagements_jusqu_au(ctx: &Contexte, paies_courantes: Option<&[Paie]>, jusqu_au: Option<&str>) -> Resultat<Vec<Declare>> {
    let mut tous = lire_importes(ctx)?;
    tous.extend(engagements_des_paies(ctx, paies_courantes, jusqu_au)?);
    tous.sort_by(|a, b| (a.debut, a.fin, &a.salarie).cmp(&(b.debut, b.fin, &b.salarie)));
    Ok(tous)
}
