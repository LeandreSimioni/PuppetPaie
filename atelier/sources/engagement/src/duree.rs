//! LA DUREE D'UNE JOURNEE DE TRAVAIL SAISIE. Les heures qu'un engagement fait travailler chaque jour, ligne par ligne :
//! une ligne a l'heure, ou dont les heures se saisissent par unite (une journee, un service), repartit les siennes
//! egalement sur ses jours ; un cachet (son forfait de 12 h n'est pas une duree de travail), une absence, un conge, un
//! maintien de salaire ne comptent pas. La journee la plus chargee en compte au moins autant que cette moyenne.
//!
//! - Au-dela de 24 heures en un jour, la saisie se refuse : une journee n'en a que 24 (une faute de frappe, 25 pour 2,5).
//! - Au-dela de 10 heures, le moteur le dit sans refuser : « La duree quotidienne de travail effectif par salarie ne
//!   peut exceder dix heures, sauf » une derogation de l'inspecteur du travail, l'urgence, ou une convention ou un
//!   accord qui la porte jusqu'a douze heures (code du travail, L. 3121-18 et L. 3121-19,
//!   `atelier/reference/sources/code-du-travail/articles-20261030.md`, « La duree maximale du travail et le repos
//!   quotidien »). C'est l'employeur qui en repond : une tournee a des journees longues, et la convention du spectacle
//!   vivant prive (3090) porte la journee a 12 heures pour certains metiers et certaines annexes.

use std::collections::BTreeMap;

use serde_json::Value;

use droit::catalogue::rubrique;
use droit::contexte::Contexte;
use socle::date::{d, en_texte, Jour};
use socle::num::Resultat;

/// La duree quotidienne maximale de travail effectif (L. 3121-18).
pub const MAXIMUM_QUOTIDIEN: f64 = 10.0;

/// Une journee.
const JOURNEE: f64 = 24.0;

/// Les heures de travail de chaque jour de l'engagement `e` (une saisie), et si l'une d'elles est une moyenne (une
/// ligne qui repartit ses heures sur plusieurs jours).
fn heures_par_jour(ctx: &Contexte, e: &Value) -> Resultat<BTreeMap<Jour, (f64, bool)>> {
    let mut jours: BTreeMap<Jour, (f64, bool)> = BTreeMap::new();
    for l in e.get("lignes").and_then(Value::as_array).into_iter().flatten() {
        let Some(code) = l.get("rubrique") else { continue };
        let r = rubrique(&ctx.racine, &code.as_str().map_or_else(|| en_texte(code), String::from), &ctx.lue_au)?;
        let travail = r.nature == "salaire"
            && matches!(r.unite.as_str(), "heure" | "jour" | "service")
            && r.heures_forfait.is_none()
            && !(r.arret_de_travail || r.conges_pris || r.suspension);
        if !travail {
            continue;
        }
        let dates: Vec<Jour> = l.get("dates").and_then(Value::as_array).into_iter().flatten().filter_map(|x| d(x.get("jour").unwrap_or(x)).ok()).collect();
        if dates.is_empty() {
            continue;
        }
        let nombre = |k: &str| l.get(k).and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim().replace(',', ".").parse().ok())));
        let total = r.heures(nombre("quantite").unwrap_or(0.0), nombre("heures"));
        let par_jour = total / dates.len() as f64;
        for j in &dates {
            let x = jours.entry(*j).or_insert((0.0, false));
            x.0 += par_jour;
            x.1 |= dates.len() > 1;
        }
    }
    Ok(jours)
}

fn heures(h: f64) -> String {
    let t = format!("{:.2}", h);
    t.trim_end_matches('0').trim_end_matches('.').replace('.', ",")
}

/// LE REFUS : un jour de plus de 24 heures dans l'engagement `e` de `qui`. `None` quand il n'y en a pas.
pub fn plus_d_une_journee(ctx: &Contexte, e: &Value, qui: &str) -> Resultat<Option<String>> {
    let trop: Vec<(Jour, (f64, bool))> = heures_par_jour(ctx, e)?.into_iter().filter(|(_, (h, _))| *h > JOURNEE + 1e-9).collect();
    let Some((j, (h, moyenne))) = trop.first() else { return Ok(None) };
    Ok(Some(format!(
        "{} : {} h de travail {}le {}{} : saisissez les heures de ce jour-là, ou répartissez-les sur plus de jours.",
        qui,
        heures(*h),
        if *moyenne { "en moyenne " } else { "" },
        j.jj_mm_aaaa(),
        crate::JOURNEE_DE_PLUS_DE_24
    )))
}

/// CE QUI SE DIT SANS SE REFUSER : les jours de l'engagement `e` de `qui` au-dela de la duree quotidienne maximale
/// (L. 3121-18). `None` quand il n'y en a pas.
pub fn au_dela_du_maximum(ctx: &Contexte, e: &Value, qui: &str) -> Resultat<Option<String>> {
    let longs: Vec<(Jour, (f64, bool))> =
        heures_par_jour(ctx, e)?.into_iter().filter(|(_, (h, _))| *h > MAXIMUM_QUOTIDIEN + 1e-9 && *h <= JOURNEE + 1e-9).collect();
    if longs.is_empty() {
        return Ok(None);
    }
    let moyenne = longs.iter().any(|(_, (_, m))| *m);
    let dits: Vec<String> = longs.iter().map(|(j, (h, _))| format!("{} h le {}", heures(*h), j.jj_mm())).collect();
    Ok(Some(format!(
        "{} : {}{}. La durée quotidienne de travail effectif ne peut dépasser 10 heures (code du travail, L. 3121-18), sauf \
         une convention ou un accord qui la porte jusqu'à 12 heures (L. 3121-19), une dérogation de l'inspecteur du travail \
         ou l'urgence (L. 3121-18, 1° et 2°).",
        qui,
        dits.join(", "),
        if moyenne { " (en moyenne sur les jours de sa ligne)" } else { "" }
    )))
}
