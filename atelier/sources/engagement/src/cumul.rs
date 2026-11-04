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

/// Les contrats que l'import atteste (`importe::contrats_lus` : une AEM, un bulletin), sauf ceux des permanents, qui
/// n'ouvrent aucun droit. Vide tant qu'on n'a rien importe.
pub(crate) fn lire_importes(ctx: &Contexte) -> Resultat<Vec<Declare>> {
    let mut out = Vec::new();
    for i in crate::importe::tous(&ctx.racine)?.into_iter().filter(|i| !i.permanent()) {
        let Some(mut e) = i.ligne_du_contrat() else { continue };
        // Un contrat qui continue ici : ce que les pieces attestent s'arrete a son dernier mois verse, la suite est
        // dans les paies de ce logiciel.
        if let Some(dernier) = i.continue_apres() {
            e["fin"] = Value::String(socle::mois::Mois::lire(&dernier)?.dernier_jour().iso());
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

/// CE QUE LA COMPAGNIE A DECLARE, LU UNE FOIS POUR UN GESTE QUI EN TIRE PLUSIEURS CUMULS (les bulletins d'un envoi) : les
/// engagements importes, et les paies ecrites des engagements valides des salaries `salaries` (leurs cles ; `None` :
/// tous) dans chaque mois saisi jusqu'a `jusqu_au` (les mois d'apres ne se lisent pas : un mois passe ne depend pas des
/// suivants). Chaque mois se lit dans les paies ecrites de ses engagements valides (`etats::validees_du_mois`) : ce qui
/// est en saisie n'est pas paye, il n'entre dans aucun cumul. Un mois se lit la premiere fois qu'un cumul le demande, et
/// une seule fois.
pub struct Passe<'a> {
    ctx: &'a Contexte,
    jusqu_au: Option<String>,
    salaries: Option<std::collections::BTreeSet<String>>,
    departs: &'a crate::departs::Departs,
    importes: std::cell::RefCell<Option<Vec<Declare>>>,
    lus: std::cell::RefCell<std::collections::BTreeMap<String, Vec<Declare>>>,
}

impl<'a> Passe<'a> {
    pub fn lire(
        ctx: &'a Contexte,
        jusqu_au: Option<&str>,
        salaries: Option<std::collections::BTreeSet<String>>,
        departs: &'a crate::departs::Departs,
    ) -> Passe<'a> {
        Passe { ctx, jusqu_au: jusqu_au.map(str::to_string), salaries, departs, importes: Default::default(), lus: Default::default() }
    }

    /// TOUT ce que la compagnie a declare, trie par (debut, fin, salarie) : les engagements importes, puis les paies.
    /// `paies_courantes` : les paies d'un mois deja lues par l'appelant ; elles remplacent celles de leur mois. Un mois a
    /// la fois importe et saisi compte les deux : un engagement importe ne touche aucun mois de la saisie
    /// (`stock::Objet::couvre`), aucune paie d'ici ne le declare une seconde fois.
    pub fn engagements(&self, paies_courantes: Option<&[Paie]>) -> Resultat<Vec<Declare>> {
        let ctx = self.ctx;
        if self.importes.borrow().is_none() {
            *self.importes.borrow_mut() = Some(lire_importes(ctx)?);
        }
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
            if groupes.iter().any(|(m, _)| *m == stem) || self.jusqu_au.as_deref().is_some_and(|j| stem.as_str() > j) {
                continue;
            }
            if let Some(es) = self.lus.borrow().get(&stem) {
                groupes.push((stem, es.clone()));
                continue;
            }
            let sien = |v: &crate::etats::Valide| self.salaries.as_ref().is_none_or(|s| crate::cle_de_l_engagement(ctx, &v.objet().brut).is_some_and(|c| s.contains(&c)));
            let mut es = Vec::new();
            if let Some((_, paies)) = crate::etats::validees_du_mois_parmi(ctx, &stem, self.departs, &sien)? {
                for p in &paies {
                    if let Some(e) = engagement_de_paie(ctx, p)? {
                        es.push(e);
                    }
                }
            }
            self.lus.borrow_mut().insert(stem.clone(), es.clone());
            groupes.push((stem, es));
        }
        let mut tous = self.importes.borrow().clone().unwrap_or_default();
        tous.extend(groupes.into_iter().flat_map(|(_, es)| es));
        tous.sort_by(|a, b| (a.debut, a.fin, &a.salarie).cmp(&(b.debut, b.fin, &b.salarie)));
        Ok(tous)
    }
}

/// TOUT ce que la compagnie a declare (`Passe::engagements`), pour un seul cumul.
pub fn lire_engagements(ctx: &Contexte, paies_courantes: Option<&[Paie]>, departs: &crate::departs::Departs) -> Resultat<Vec<Declare>> {
    Passe::lire(ctx, None, None, departs).engagements(paies_courantes)
}
