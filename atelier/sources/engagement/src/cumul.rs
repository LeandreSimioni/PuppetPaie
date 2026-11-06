//! Ce que la compagnie a declare, pour les cumuls des annexes 8 et 10 (`droit::cumul`) : les engagements importes, puis les
//! paies de ses engagements, une ligne declaree (`Declare`) par paie d'intermittent.


use crate::calcul::Paie;
use droit::contexte::Contexte;
use droit::cumul::Declare;
use socle::date::Jour;
use socle::num::Resultat;

/// La part d'enseignement de lignes de paie, en heures entieres : les rubriques dont le catalogue dit
/// `compte_pour = ouverture_des_droits`.
fn heures_enseignement(ctx: &Contexte, lignes: &[crate::LignePaie]) -> Resultat<i64> {
    let mut heures = 0.0;
    for l in lignes {
        let r = droit::catalogue::rubrique(&ctx.racine, l.rubrique.as_deref().ok_or_else(|| socle::fichiers::absent("rubrique"))?, &ctx.lue_au)?;
        if r.compte_pour != "ouverture_des_droits" {
            continue;
        }
        let q = l.quantite.as_ref().ok_or_else(|| socle::fichiers::absent("quantite"))?.flottant()?;
        // Le signe porte aussi sur les heures, comme au calcul (`calcul::lignes_brut`) : une ligne « - » en retire.
        let signe = if r.sens == "-" { -1.0 } else { 1.0 };
        heures += signe * r.heures(q, l.heures_lues()?);
    }
    Ok(heures.round_ties_even() as i64)
}

// ------------------------------------------------------------------- lecture

/// Les contrats que l'import atteste (`importe::contrats_lus` : une AEM, un bulletin), sauf ceux des permanents, qui
/// n'ouvrent aucun droit. Vide tant qu'on n'a rien importe. Un contrat dont l'engagement n'a pas de bornes ne se compte
/// pas : rien ne le situe.
pub(crate) fn lire_importes(ctx: &Contexte) -> Resultat<Vec<Declare>> {
    let mut out = Vec::new();
    for i in crate::importe::tous(&ctx.racine)?.into_iter().filter(|i| !i.permanent()) {
        let Some(c) = i.contrat_lu() else { continue };
        // Un contrat qui continue ici : ce que les pieces attestent s'arrete a son dernier mois verse, la suite est
        // dans les paies de ce logiciel.
        let fin = match i.continue_apres() {
            Some(dernier) => Ok(socle::mois::Mois::lire(&dernier)?.dernier_jour()),
            None => Jour::lire(&c.fin),
        };
        let (Ok(debut), Ok(fin)) = (Jour::lire(&c.debut), fin) else { continue };
        // Une piece d'intermittent atteste ses compteurs et son brut (l'AEM, le bulletin, le certificat : `import`) ; les
        // heures d'enseignement, seulement celles qu'elle distingue.
        let a = &c.contrat;
        let compte = |x: Option<i64>, cle: &str| x.ok_or_else(|| socle::fichiers::absent(cle));
        out.push(Declare {
            salarie: c.salarie.clone(),
            debut,
            fin,
            heures: compte(a.heures, "heures")?,
            cachets: compte(a.cachets, "cachets")?,
            jours: compte(a.jours, "jours")?,
            brut: a.brut.ok_or_else(|| socle::fichiers::absent("brut"))?,
            heures_enseignement: a.heures_enseignement.into_iter().sum(),
        });
    }
    Ok(out)
}

/// Une ligne declaree par paie d'intermittent (les permanents n'ouvrent aucun droit).
fn engagement_de_paie(ctx: &Contexte, p: &Paie) -> Resultat<Option<Declare>> {
    let e = &p.engagement;
    if e.sans_paie() || e.nat()?.mensuelle() {
        return Ok(None);
    }
    let heures = e.heures.into_iter().sum::<f64>().round_ties_even() as i64;
    Ok(Some(Declare {
        salarie: p.salarie.id.to_string(),
        debut: e.debut_periode()?,
        fin: e.fin_periode()?,
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
        for p in paies_courantes.into_iter().flatten() {
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
        for stem in crate::stock::mois_saisis(&ctx.racine)?.into_iter().map(String::from) {
            if groupes.iter().any(|(m, _)| *m == stem) || self.jusqu_au.as_deref().is_some_and(|j| stem.as_str() > j) {
                continue;
            }
            if let Some(es) = self.lus.borrow().get(&stem) {
                groupes.push((stem, es.clone()));
                continue;
            }
            let sien = |v: &crate::etats::Valide| self.salaries.as_ref().is_none_or(|s| crate::cle_de_l_engagement(ctx, &v.objet().brut).is_some_and(|c| s.contains(c.as_str())));
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
        let mut tous: Vec<Declare> = self.importes.borrow().iter().flatten().cloned().collect();
        tous.extend(groupes.into_iter().flat_map(|(_, es)| es));
        tous.sort_by(|a, b| (a.debut, a.fin, &a.salarie).cmp(&(b.debut, b.fin, &b.salarie)));
        Ok(tous)
    }
}

/// TOUT ce que la compagnie a declare (`Passe::engagements`), pour un seul cumul.
pub fn lire_engagements(ctx: &Contexte, paies_courantes: Option<&[Paie]>, departs: &crate::departs::Departs) -> Resultat<Vec<Declare>> {
    Passe::lire(ctx, None, None, departs).engagements(paies_courantes)
}
