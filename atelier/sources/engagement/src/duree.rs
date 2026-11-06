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

use crate::{Chiffre, LignePaie};

use droit::catalogue::rubrique;
use droit::contexte::Contexte;
use socle::date::Jour;
use socle::num::{dec_f64, Resultat};
use socle::texte::virgule;

/// La duree quotidienne maximale de travail effectif (L. 3121-18).
pub const MAXIMUM_QUOTIDIEN: f64 = 10.0;

/// Une journee.
const JOURNEE: f64 = 24.0;

/// Les heures de travail de chaque jour des lignes `lignes` d'un engagement, et si l'une d'elles est une moyenne (une
/// ligne qui repartit ses heures sur plusieurs jours).
fn heures_par_jour(ctx: &Contexte, lignes: &[LignePaie]) -> Resultat<BTreeMap<Jour, (f64, bool)>> {
    let mut jours: BTreeMap<Jour, (f64, bool)> = BTreeMap::new();
    for l in lignes {
        let Some(code) = &l.rubrique else { continue };
        let r = rubrique(&ctx.racine, code, &ctx.lue_au)?;
        let travail = r.nature == "salaire"
            && matches!(r.unite.as_str(), "heure" | "jour" | "service")
            && r.heures_forfait.is_none()
            && !(r.arret_de_travail || r.conges_pris || r.suspension);
        if !travail {
            continue;
        }
        let dates: Vec<Jour> = l.jours()?;
        if dates.is_empty() {
            continue;
        }
        // Une case vide ne dit rien ; une virgule decimale se lit comme un point ; un texte qui n'est pas un nombre se
        // refuse.
        let nombre = |c: &Option<Chiffre>| -> Resultat<Option<f64>> {
            match crate::ligne::dite(c) {
                None => Ok(None),
                Some(Chiffre::Nombre(n)) => n.as_f64().map(Some).ok_or_else(|| socle::num::illisible(&n.to_string())),
                Some(Chiffre::Texte(s)) => s.trim().replace(',', ".").parse().map(Some).map_err(|_| socle::num::illisible(s)),
            }
        };
        // La quantite d'une ligne de travail est dite (`lignes_dites`, avant ce compte) : sans elle, rien ne se compte.
        let quantite = nombre(&l.quantite)?.ok_or_else(|| socle::fichiers::absent("quantite"))?;
        let total = r.heures(quantite, nombre(&l.heures)?);
        let par_jour = total / dates.len() as f64;
        for j in &dates {
            let x = jours.entry(*j).or_insert((0.0, false));
            x.0 += par_jour;
            x.1 |= dates.len() > 1;
        }
    }
    Ok(jours)
}

/// LE REFUS : un jour de plus de 24 heures dans l'engagement `e` de `qui`. `None` quand il n'y en a pas.
pub fn plus_d_une_journee(ctx: &Contexte, lignes: &[LignePaie], qui: &str) -> Resultat<Option<String>> {
    let trop: Vec<(Jour, (f64, bool))> = heures_par_jour(ctx, lignes)?.into_iter().filter(|(_, (h, _))| *h > JOURNEE + 1e-9).collect();
    let Some((j, (h, moyenne))) = trop.first() else { return Ok(None) };
    Ok(Some(format!(
        "{} : {} h de travail {}le {}{} : saisissez les heures de ce jour-là, ou répartissez-les sur plus de jours.",
        qui,
        virgule(dec_f64(*h)?.round_dp(2)),
        if *moyenne { "en moyenne " } else { "" },
        j.jj_mm_aaaa(),
        crate::JOURNEE_DE_PLUS_DE_24
    )))
}

/// CE QUI SE DIT SANS SE REFUSER : les jours de l'engagement `e` de `qui` au-dela de la duree quotidienne maximale
/// (L. 3121-18). `None` quand il n'y en a pas.
pub fn au_dela_du_maximum(ctx: &Contexte, lignes: &[LignePaie], qui: &str) -> Resultat<Option<String>> {
    let longs: Vec<(Jour, (f64, bool))> =
        heures_par_jour(ctx, lignes)?.into_iter().filter(|(_, (h, _))| *h > MAXIMUM_QUOTIDIEN + 1e-9 && *h <= JOURNEE + 1e-9).collect();
    if longs.is_empty() {
        return Ok(None);
    }
    let moyenne = longs.iter().any(|(_, (_, m))| *m);
    let dits: Vec<String> = longs.iter().map(|(j, (h, _))| format!("{} h le {}", dec_f64(*h).iter().map(|h| virgule(h.round_dp(2))).collect::<String>(), j.jj_mm())).collect();
    Ok(Some(format!(
        "{} : {}{}. La durée quotidienne de travail effectif ne peut dépasser 10 heures (code du travail, L. 3121-18), sauf \
         une convention ou un accord qui la porte jusqu'à 12 heures (L. 3121-19), une dérogation de l'inspecteur du travail \
         ou l'urgence (L. 3121-18, 1° et 2°).",
        qui,
        dits.join(", "),
        if moyenne { " (en moyenne sur les jours de sa ligne)" } else { "" }
    )))
}
