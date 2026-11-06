//! Les minima conventionnels confrontes a une paie (la table et ses grilles : `droit::minima`) : des avertissements,
//! jamais un refus. `avertissements` lit la grille de la convention ; `sous_le_smic` lit la loi.
//!
//! Chaque minimum prend, quand la paie ne dit pas tout, l'hypothese la plus basse : un avertissement qui crie a tort ne
//! se lit plus.

use std::collections::BTreeSet;

use rust_decimal::RoundingStrategy;

use crate::calcul::{LigneBrut, Paie};
use droit::catalogue::rubrique;
use droit::contexte::Contexte;
use socle::date::{repr_texte, Jour};
use droit::emplois;
use crate::{Avis, Engagement};
use droit::minima::{ans_revolus, grille_au, iso, palier, plancher, plus_bas, signataire, table, case_lue, CaseLue, Conditions, Convention, Grille, Minimum, Par, Plancher, SpectacleDuContrat, Table};
use socle::texte::{euros, virgule};
use socle::num::{add, dec_f64, div, mul, sub, Decimal, Resultat};
/// Ce que l'IDCC d'un contrat lit du spectacle `sp` (`droit::minima::idcc_du_contrat`) ; un contrat paye au mois n'en a pas.
pub fn spectacle_du_contrat(sp: Option<&droit::fiche::Spectacle>) -> SpectacleDuContrat<'_> {
    SpectacleDuContrat {
        emission_tv_1734: sp.is_some_and(|s| s.emission_tv_1734.dite() == Some(&true)),
        // Le titre que la fiche du spectacle dit ; sans spectacle ni titre, aucun.
        titre: match sp.and_then(|s| s.titre.dite()) {
            Some(t) => t.as_str(),
            None => "",
        },
    }
}

/// La case `cle` du spectacle de la paie lue pour une condition (`droit::minima::case_lue`) : sans spectacle, rien.
fn case_du_spectacle(sp: Option<&droit::fiche::Spectacle>, cle: &str) -> CaseLue {
    match sp {
        Some(s) => case_lue(&s.champs, cle),
        None => CaseLue::default(),
    }
}

fn cachet(c: &str) -> bool {
    c == "cachet"
}
fn repetition(c: &str) -> bool {
    c == "repetition"
}
fn horaire(c: &str) -> bool {
    matches!(c, "heure" | "journee" | "semaine")
}
fn mensuel(c: &str) -> bool {
    c == "mois"
}


// ------------------------------------------------------------- les petits outils

/// Au centime, vers zéro.
fn bas(x: Decimal) -> Decimal {
    x.round_dp_with_strategy(2, RoundingStrategy::ToZero)
}

/// Au centime, au pair sur le demi.
fn pair(x: Decimal) -> Decimal {
    x.round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven)
}

/// Un flottant arrondi a deux decimales, par son texte.
fn arrondi2(x: f64) -> f64 {
    match format!("{:.2}", x).parse() {
        Ok(a) => a,
        Err(_) => x,
    }
}



/// La duree du contrat en jours calendaires, bornes comprises.
fn jours_du_contrat(eng: &Engagement) -> Option<i64> {
    Some(eng.fin?.moins(&eng.debut) + 1)
}

/// Le dernier jour de la periode payee, en AAAA-MM-JJ.
fn jour_de_paie(paie: &Paie) -> Option<String> {
    paie.engagement.fin.map(|j| j.iso())
}

fn premier_jour(paie: &Paie) -> Option<String> {
    paie.engagement.dates_travail.iter().map(|j| j.iso()).min()
}

fn permanent(paie: &Paie) -> Resultat<bool> {
    Ok(paie.engagement.nat()?.mensuelle())
}

// ------------------------------------------------------------------ les planchers

/// La premiere partie du texte d'une grille (`texte`, jusqu'a la premiere virgule).
fn premiere_partie(c: &Convention) -> Resultat<String> {
    let texte = c.texte.as_deref().ok_or_else(|| manque("texte"))?;
    Ok(match texte.split_once(',') {
        Some((tete, _)) => tete,
        None => texte,
    }
    .to_string())
}

/// Ce qui manque a la table livree, nomme par sa cle.
fn manque(cle: &str) -> String {
    socle::echec::livree("minima-conventionnels.json", cle)
}

/// Les donnees d'une ligne et de la paie qu'un plancher lit.
struct Cx<'a> {
    t: &'a Table,
    idcc: &'a str,
    /// La grille de la convention qui lie la compagnie au premier jour travaille (`droit::minima::grille_au`), et le
    /// jour depuis lequel elle la lie quand c'est celle d'un accord qui suit la grille de la convention.
    grille: &'a Convention,
    lie_depuis: Option<String>,
    m: &'a emplois::Metier,
    fiche: &'a droit::fiche::SalarieDuContrat,
    paie: &'a Paie,
    ent: &'a droit::fiche::ChampsEntreprise,
    annexe: Option<String>,
    organisation: Option<String>,
    nb_cachets: i64,
}

impl<'a> Cx<'a> {
    /// Les lignes de minima du metier dans la 3090, annexe par annexe (hors catalogue : aucune).
    fn refs(&self) -> &'a std::collections::BTreeMap<String, String> {
        static AUCUNE: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
        match self.m.au_catalogue() {
            Some(f) => &f.minima.ccn_3090,
            None => &AUCUNE,
        }
    }

    /// La ligne de minima du metier dans une convention qui n'en a qu'une (1285, 2642, 3097), ou rien.
    fn ligne_de_minima(&self, idcc: &str) -> Option<&'a str> {
        self.m.au_catalogue().and_then(|f| f.minima.ligne(idcc))
    }

    /// Le statut du metier, `artiste` ou `technicien`.
    fn statut(&self) -> &'static str {
        self.m.statut().as_str()
    }

    /// La convention 3090 de la table.
    fn c3090(&self) -> Resultat<&'a Convention> {
        self.t.convention("3090").ok_or_else(|| manque("3090"))
    }

    /// L'annexe `a` de la 3090, que la table doit porter.
    fn annexe_3090(&self, a: &str) -> Resultat<&'a droit::minima::Annexe> {
        self.c3090()?.annexes.get(a).ok_or_else(|| manque(&format!("3090.annexes.{}", a)))
    }

    /// Les annexes de la 3090 ou ce metier a une ligne, restreintes a celle de
    /// l'entreprise quand elle est connue.
    fn annexes_candidates(&self) -> Resultat<Vec<String>> {
        let refs = self.refs();
        if let Some(a) = &self.annexe {
            if refs.contains_key(a) {
                return Ok(vec![a.clone()]);
            }
            if self.c3090()?.annexes.contains_key(a) {
                return Ok(vec![]);
            }
        }
        Ok(refs.keys().cloned().collect())
    }

    /// Les annexes candidates, ou a defaut celle de l'entreprise si la table la connait.
    fn annexes_ou_la_sienne(&self) -> Resultat<Vec<String>> {
        let c = self.annexes_candidates()?;
        if !c.is_empty() {
            return Ok(c);
        }
        let annexes = &self.c3090()?.annexes;
        Ok(match &self.annexe {
            Some(a) if annexes.contains_key(a) => vec![a.clone()],
            _ => vec![],
        })
    }

    fn plancher_cachet_3090(&self) -> Resultat<Option<Plancher>> {
        let refs = self.refs();
        let mut meilleurs: Vec<(Decimal, String, &droit::minima::GrilleDeCachets)> = Vec::new();
        for annexe in self.annexes_candidates()? {
            let ligne = refs.get(&annexe).ok_or_else(|| socle::fichiers::absent(&annexe))?;
            let grille = self.annexe_3090(&annexe)?.cachets.get(ligne).ok_or_else(|| manque(&format!("3090.annexes.{}.cachets.{}", annexe, ligne)))?;
            if let Some(montant) = palier(&grille.paliers, self.nb_cachets) {
                meilleurs.push((montant, annexe, grille));
            }
        }
        if meilleurs.is_empty() {
            return Ok(None);
        }
        let toutes = meilleurs.len() > 1;
        let mut noms: Vec<String> = meilleurs.iter().map(|x| x.1.clone()).collect();
        noms.sort();
        let (montant, annexe, grille) = plus_bas(meilleurs, |x| x.0).unwrap();
        let mut quoi = format!("CCN 3090, annexe {}, {}", annexe, grille.libelle);
        if grille.jauge.as_deref().is_some_and(|j| !j.is_empty()) {
            quoi += " (plus petite jauge retenue)";
        }
        if self.annexe.is_none() && toutes {
            quoi += &format!(" -- la plus basse des annexes {}, l'entreprise ne disant pas la sienne", noms.join(" et "));
        }
        Ok(plancher(montant, quoi))
    }

    /// Le texte d'ou vient la grille lue, et, pour un accord qui suit la grille de la convention, depuis quand il lie la
    /// compagnie : comme adherente d'une organisation signataire, ou comme toute compagnie de la convention.
    fn texte_de_la_grille(&self) -> Resultat<String> {
        let texte = premiere_partie(self.grille)?;
        let Some(depuis) = &self.lie_depuis else { return Ok(texte) };
        let le = Jour::lire(depuis)?.jj_mm_aaaa();
        Ok(match self.organisation.as_deref().filter(|o| signataire(self.grille, Some(o))) {
            Some(o) => format!("{}, que la compagnie doit depuis le {} comme adhérente de {}", texte, le, o),
            None => format!("{}, dû par toute compagnie depuis le {}", texte, le),
        })
    }

    fn plancher_cachet_1285(&self) -> Resultat<Option<Plancher>> {
        let Some(r) = self.ligne_de_minima("1285").filter(|r| !r.is_empty()) else {
            return Ok(None);
        };
        let grille = self.grille.cachets.get(r).ok_or_else(|| manque(&format!("1285.cachets.{}", r)))?;
        let Some(montant) = palier(&grille.paliers, self.nb_cachets) else { return Ok(None) };
        Ok(plancher(montant, format!("CCNEAC (1285), {}", grille.libelle)))
    }

    fn plancher_repetition(&self) -> Resultat<Option<Plancher>> {
        match self.idcc {
            "1285" => {
                let r = self.grille.repetition.as_ref().ok_or_else(|| manque("1285.repetition"))?;
                if let Some(propre) = self.ligne_de_minima("1285").and_then(|reference| r.par_categorie.get(reference)) {
                    return Ok(plancher(propre.service, format!("CCNEAC (1285), {}", propre.libelle)));
                }
                Ok(plancher(r.service, format!("CCNEAC (1285), {}", r.service_libelle)))
            }
            "3090" => {
                // Une heure n'est pas un cachet : seul un taux horaire de repetition se compare.
                let mut candidats = Vec::new();
                for annexe in self.annexes_ou_la_sienne()? {
                    if let Some(h) = self.annexe_3090(&annexe)?.repetition.as_ref().and_then(|r| r.heure) {
                        candidats.push((h, annexe));
                    }
                }
                Ok(plus_bas(candidats, |x| x.0).map(|(montant, annexe)| Plancher {
                    montant,
                    quoi: format!("CCN 3090, annexe {}, services de répétition (taux horaire)", annexe),
                    unite: false,
                }))
            }
            _ => Ok(None),
        }
    }

    /// L'echelon de la CCNEAC : celui de la fiche (1 a 12), ou le premier.
    fn echelon_ccneac(&self) -> (usize, String) {
        let v = self.fiche.echelon_ccneac.texte().to_string();
        let v = v.trim();
        if let Some(n) = socle::num::entier_de_chiffres(v).filter(|n| (1..=12).contains(n)) {
            return (n as usize, format!("échelon {} de la fiche", v));
        }
        (1, "échelon 1, la fiche ne disant pas le sien (`echelon_ccneac`)".into())
    }

    /// Le taux horaire minimum d'un emploi autre qu'artistique de la CCNEAC.
    fn plancher_horaire_1285(&self) -> Resultat<Option<Plancher>> {
        let Some(c1285) = self.m.au_catalogue().and_then(|f| f.classification_1285.as_ref()) else { return Ok(None) };
        let Some(groupe) = c1285.groupe.map(|g| g.to_string()).filter(|g| !g.is_empty()) else { return Ok(None) };
        let Some(grille) = self.grille.emplois_autres_qu_artistiques.as_ref() else { return Ok(None) };
        let Some(echelons) = grille.groupes.get(&groupe) else { return Ok(None) };
        let (echelon, d_ou) = self.echelon_ccneac();
        let mensuel = *echelons
            .get(echelon - 1)
            .ok_or_else(|| socle::fichiers::trop_courte(&format!("l'échelon {} du groupe {}", echelon, groupe)))?;
        let heures = grille.heures_mensuelles;
        let horaire = arrondi2(mensuel / heures);
        Ok(plancher(
            dec_f64(horaire)?,
            format!(
                "CCNEAC (1285), groupe {} ({}), {} : {} pour 151 h 40, ramenés à l'heure",
                groupe,
                c1285.emploi_xi3.as_str(),
                d_ou,
                euros(dec_f64(mensuel)?)
            ),
        ))
    }

    /// La fonction d'un metier dans la grille de la 2642 ou de la 3097 : celle de
    /// la fiche quand la grille la connait, sinon celle du catalogue.
    fn fonction(&self, idcc: &str, connue: impl Fn(&str) -> bool) -> Option<String> {
        let propre = self.fiche.classement_conventionnel.texte().to_string();
        let propre = propre.trim();
        if !propre.is_empty() && connue(propre) {
            return Some(propre.to_string());
        }
        let f = self.m.au_catalogue()?;
        let du_catalogue = match idcc {
            "2642" => f.classification_2642.as_ref().map(|c| c.fonction.as_str()),
            "3097" => f.classification_3097.as_ref().map(|c| c.fonction.as_str()),
            _ => None,
        };
        du_catalogue.filter(|c| !c.is_empty()).map(str::to_string)
    }

    fn plancher_2642(&self) -> Resultat<Option<Plancher>> {
        let c = self.t.convention("2642").ok_or_else(|| manque("2642"))?;
        let (cddu, permanents) = (&c.cddu_semaine_35h, &c.permanents_mensuel_35h);
        let Some(fonction) = self.fonction("2642", |f| cddu.contains_key(f) || permanents.contains_key(f)) else { return Ok(None) };
        let d = socle::num::dec_texte;
        if permanent(self.paie)? {
            let Some(mensuel) = permanents.get(&fonction) else { return Ok(None) };
            let m = mensuel.0;
            let h = bas(div(m, d("151.67")?));
            return Ok(plancher(
                h,
                format!(
                    "CCN 2642, {} en CDI ou CDD de droit commun : {} par mois pour 35 h, ramenés à l'heure sur 151,67 h",
                    fonction,
                    euros(m)
                ),
            ));
        }
        let Some(presents) = cddu.get(&fonction).map(|f| f.genres()).filter(|p| !p.is_empty()) else {
            return Ok(None);
        };
        let genre = case_du_spectacle(self.paie.spectacle.as_ref(), "genre_audiovisuel").texte;
        let (semaine, dit) = if let Some((_, s)) = presents.iter().find(|(g, _)| *g == genre) {
            (*s, format!("programme de {}", genre))
        } else {
            let (g, s) = plus_bas(presents, |x| x.1).unwrap();
            (s, format!("genre {}, le plus bas des genres de cette fonction -- la fiche du spectacle ne dit pas le sien (`genre_audiovisuel`)", g))
        };
        let jours = jours_du_contrat(&self.paie.engagement);
        let (h, regle) = match jours {
            Some(j) if j <= 4 => (div(semaine, d("31.5")?), "contrat de 4 jours ou moins : la semaine / 4,5 par journée de 7 h"),
            Some(j) if j >= 89 => {
                let mensuel = mul(semaine, d("3.8")?);
                let smic = mul(mul(dec_f64(self.paie.smic)?, d("151.67")?), d("1.3")?);
                let mensuel = if smic > mensuel { smic } else { mensuel };
                (div(mensuel, d("151.67")?), "contrat de 3 mois ou plus : la semaine x 3,8 par mois de 151,67 h, pas moins de 130 % du SMIC")
            }
            None => (div(mul(semaine, d("3.8")?), d("151.67")?), "durée du contrat inconnue : la base mensuelle, la plus basse"),
            Some(_) => (div(semaine, d("35")?), "la semaine de 35 h"),
        };
        Ok(plancher(
            bas(h),
            format!(
                "CCN 2642, {} en CDDU, {} : {} la semaine de 35 h ; {} (IV.2.1)",
                fonction,
                dit,
                euros(semaine),
                regle
            ),
        ))
    }

    fn plancher_3097(&self, ligne: &LigneBrut) -> Resultat<Option<Plancher>> {
        let c = self.t.convention("3097").ok_or_else(|| manque("3097"))?;
        let d = socle::num::dec_texte;
        let spec = self.paie.spectacle.as_ref();
        let statut = self.statut();
        let c3097 = self.m.au_catalogue().and_then(|f| f.classification_3097.as_ref());
        let a_fonction = c3097.is_some_and(|c| !c.fonction.is_empty());
        let heures = !ligne.heures_annexes.is_zero();
        if (statut == "technicien" || a_fonction) && (horaire(&ligne.code) || mensuel(&ligne.code)) && heures {
            if permanent(self.paie)? {
                let emploi = c3097.and_then(|c| c.titre_iv.as_deref()).filter(|e| !e.is_empty());
                let pm = c.permanents_mensuel.as_ref().ok_or_else(|| manque("3097.permanents_mensuel"))?;
                let Some((emploi, niveau)) = emploi.and_then(|e| pm.emplois_reperes.get(e).map(|n| (e, n))) else {
                    return Ok(None);
                };
                let mensuel = pm.niveaux.get(niveau).ok_or_else(|| manque(&format!("3097.permanents_mensuel.niveaux.{}", niveau)))?.0;
                return Ok(plancher(
                    bas(div(mensuel, d("151.67")?)),
                    format!(
                        "CCN 3097, titre IV, {} ({}) : {} par mois, salaire de base sans les compléments, ramené à l'heure sur 151,67 h",
                        emploi,
                        niveau.replace('_', " "),
                        euros(mensuel)
                    ),
                ));
            }
            let techniciens = &c.techniciens_semaine_39h;
            let Some(fonction) = self.fonction("3097", |f| techniciens.contains_key(f)) else { return Ok(None) };
            let Some(semaine) = techniciens.get(&fonction) else { return Ok(None) };
            let mut semaine = semaine.0;
            let debut = self.paie.engagement.debut.iso();
            if debut.as_str() < "2026-05-01" {
                if let Some(a) = c.avant_2026_05.as_ref().and_then(|a| a.montants.get(&fonction)) {
                    semaine = a.0;
                }
            }
            let mut dit = format!("{} la semaine de 39 h (annexe I du titre II)", euros(semaine));
            if case_du_spectacle(spec, "derogation_annexe_iii_3097").vraie {
                let a = c.annexe_iii.as_ref().ok_or_else(|| manque("3097.annexe_iii"))?;
                if semaine > a.socle {
                    semaine = add(a.socle, div(mul(sub(semaine, a.socle), a.part_au_dela), Decimal::ONE_HUNDRED));
                }
                dit = format!("film sous dérogation de l'annexe III : {} la semaine de 39 h", euros(bas(semaine)));
            }
            let h = bas(div(semaine, c.heures_payees_de_la_semaine.ok_or_else(|| manque("3097.heures_payees_de_la_semaine"))?));
            return Ok(plancher(h, format!("CCN 3097, {} : {}, soit 40 h au taux de base (35 h + 4 h à 125 %)", fonction, dit)));
        }
        if statut == "artiste" && cachet(&ligne.code) {
            let Some(reference) = self.ligne_de_minima("3097").filter(|r| !r.is_empty()) else {
                return Ok(None);
            };
            let fmt = case_du_spectacle(spec, "format_cinema").texte;
            let (grille, quoi) = if reference == "acteur" {
                let cl = if fmt == "long" { "long" } else { "court" };
                let sous = if fmt == "long" || fmt == "court" {
                    String::new()
                } else {
                    " (le plus bas : la fiche du spectacle ne dit pas le format, `format_cinema`)".into()
                };
                let artistes = c.artistes.as_ref().ok_or_else(|| manque("3097.artistes"))?;
                (if cl == "long" { &artistes.long } else { &artistes.court }, format!("artiste-interprète, {}-métrage{}", cl, sous))
            } else {
                let complement = c.complement.as_ref().ok_or_else(|| manque("3097.complement"))?;
                let Some(role) = complement.du_role(reference).filter(|_| fmt == "long") else {
                    return Ok(None);
                };
                (role, format!("acteur de complément ({}), long-métrage", reference.replace('_', " ")))
            };
            let (montant, regle) = if self.nb_cachets >= 5 {
                let s5 = div(grille.semaine_5j, Decimal::from(5));
                let s6 = div(grille.semaine_6j, Decimal::from(6));
                (bas(if s6 < s5 { s6 } else { s5 }), "engagement à la semaine, rapporté au cachet")
            } else {
                (grille.journee, "engagement à la journée")
            };
            return Ok(plancher(montant, format!("CCN 3097, titre III, {}, {}", quoi, regle)));
        }
        Ok(None)
    }

    fn a_grilles(&self) -> bool {
        !self.grille.grilles.is_empty()
    }

    fn conditions(&self, c: Conditions) -> Resultat<bool> {
        if let Some(cats) = c.categories {
            let ref_ = self.ligne_de_minima(self.idcc);
            if !cats.iter().any(|x| Some(x.as_str()) == ref_ || (x.is_empty() && ref_.is_none())) {
                return Ok(false);
            }
        }
        if let Some(codes) = c.codes {
            if !codes.iter().any(|x| *x == self.m.code) {
                return Ok(false);
            }
        }
        // Les conditions nomment les cases du spectacle, de la fiche entreprise et de celle du salarie.
        for (cl, attendu) in c.si_spectacle {
            if !attendu.vaut(&case_du_spectacle(self.paie.spectacle.as_ref(), cl)) {
                return Ok(false);
            }
        }
        for (cl, attendu) in c.si_entreprise {
            if !attendu.vaut(&case_lue(self.ent, cl)) {
                return Ok(false);
            }
        }
        for (cl, attendu) in c.si_salarie {
            if !attendu.vaut(&case_lue(&self.fiche.champs, cl)) {
                return Ok(false);
            }
        }
        if let Some(a) = c.anciennete {
            let depuis = case_lue(&self.fiche.champs, &a.champ).texte;
            match ans_revolus(&depuis, jour_de_paie(self.paie).as_deref()) {
                Some(n) if n >= a.ans => {}
                _ => return Ok(false),
            }
        }
        if let Some(max) = c.duree_contrat_max_jours {
            match jours_du_contrat(&self.paie.engagement) {
                Some(j) if j <= max => {}
                _ => return Ok(false),
            }
        }
        Ok(true)
    }

    /// Les grilles de cette convention qui visent cette ligne.
    fn grilles_de(&self, ligne: &LigneBrut, grilles: &'a [Grille]) -> Resultat<Vec<&'a Grille>> {
        let population = if permanent(self.paie)? { "permanent" } else { "intermittent" };
        let statut = self.statut();
        let est_cachet = cachet(&ligne.code);
        let heures = !ligne.heures_annexes.is_zero();
        let mut out: Vec<&'a Grille> = Vec::new();
        for gr in grilles {
            // Une grille qui ne dit pas sa population ou son statut les vise tous.
            if !gr.population.as_deref().is_none_or(|pop| pop == "tous" || pop == population) {
                continue;
            }
            if !gr.statut.as_deref().is_none_or(|st| st == "tous" || st == statut) {
                continue;
            }
            if est_cachet != (gr.unite == "cachet") {
                continue;
            }
            if !est_cachet {
                if a_l_heure(gr) {
                    if !(heures && (horaire(&ligne.code) || mensuel(&ligne.code))) {
                        continue;
                    }
                } else if ligne_de_l_unite(&gr.unite) != Some(ligne.code.as_str()) {
                    continue;
                }
            }
            if !self.conditions(gr.conditions())? {
                continue;
            }
            out.push(gr);
        }
        // Une grille en remplace une autre (`remplace`) quand elle s'applique.
        let remplacees: BTreeSet<&str> = out.iter().flat_map(|gr| gr.remplace.iter().map(String::as_str)).collect();
        Ok(out.into_iter().filter(|gr| !remplacees.contains(gr.nom.as_str())).collect())
    }

    /// Les lignes d'une grille, majorations comprises, et ce qui les a majorees.
    fn valeurs(&self, gr: &Grille) -> Resultat<(Vec<(String, Decimal)>, Vec<String>)> {
        let mut valeurs: Vec<(String, Decimal)> = gr.valeurs.iter().map(|(k, v)| (k.clone(), v.0)).collect();
        // Dans un meme groupe, seule la derniere majoration qui s'applique compte.
        let mut retenues: Vec<(String, &droit::minima::Majoration)> = Vec::new();
        for maj in &gr.majorations {
            if self.conditions(maj.conditions())? {
                // Une majoration sans groupe est seule dans le sien.
                let groupe = match maj.groupe.clone().filter(|g| !g.is_empty()) {
                    Some(g) => g,
                    None => maj.nom.clone(),
                };
                match retenues.iter_mut().find(|r| r.0 == groupe) {
                    Some(r) => r.1 = maj,
                    None => retenues.push((groupe, maj)),
                }
            }
        }
        for (_, maj) in &retenues {
            for (k, plus) in &maj.par_cle {
                if let Some(v) = valeurs.iter_mut().find(|v| &v.0 == k) {
                    v.1 = add(v.1, plus.0);
                }
            }
        }
        let mut noms = Vec::new();
        for (_, maj) in &retenues {
            noms.push(maj.nom.clone());
        }
        Ok((valeurs, noms))
    }

    /// Le plancher d'une ligne dans une convention a grilles : le montant du
    /// cachet, un taux horaire (la ligne de grille sur les heures qu'elle
    /// couvre, au centime inferieur), ou le montant d'une unite.
    fn plancher_grilles(&self, ligne: &LigneBrut, grilles: &'a [Grille]) -> Resultat<Option<Plancher>> {
        let mut grilles = self.grilles_de(ligne, grilles)?;
        if grilles.is_empty() {
            return Ok(None);
        }
        let idcc = self.idcc;
        let classement = {
            let c = self.fiche.classement_conventionnel.texte().to_string();
            let c = c.trim().to_string();
            // Le classement de la fiche, sinon celui que le catalogue donne au metier dans cette convention.
            if c.is_empty() { self.m.au_catalogue().and_then(|f| f.classement.get(idcc)).cloned().into_iter().collect() } else { c }
        };
        // Une grille a l'unite prime : elle prix la ligne telle qu'elle est payee.
        let unitaires: Vec<&Grille> = grilles.iter().copied().filter(|gr| gr.unite != "cachet" && !a_l_heure(gr)).collect();
        if !unitaires.is_empty() {
            grilles = unitaires;
        }
        // Un classement connu d'une grille ne se noie pas dans les autres.
        let connues: Vec<&Grille> =
            grilles.iter().copied().filter(|gr| !classement.is_empty() && gr.valeurs.iter().any(|(k, _)| *k == classement)).collect();
        let cherchees = if connues.is_empty() { grilles } else { connues };
        let mut trouves: Vec<Plancher> = Vec::new();
        for gr in cherchees {
            let (valeurs, majorees) = self.valeurs(gr)?;
            if valeurs.is_empty() {
                continue;
            }
            let (k, mut dit) = if valeurs.iter().any(|v| v.0 == classement) {
                (classement.clone(), classement.clone())
            } else {
                let k = plus_bas(valeurs.iter().collect(), |v| v.1).unwrap().0.clone();
                let dit = if classement.is_empty() {
                    format!("{}, le plus bas de la grille (la fiche ne dit pas le classement, `classement_conventionnel`)", k)
                } else {
                    format!("{}, le plus bas de la grille ({} n'y est pas)", k, repr_texte(&classement))
                };
                (k, dit)
            };
            if !majorees.is_empty() {
                dit += &format!(" ; {}", majorees.join(", "));
            }
            let mut montant = valeurs.iter().find(|v| v.0 == k).unwrap().1;
            if let Some(ab) = gr.abattement_max.filter(|a| !a.is_zero()) {
                montant = bas(div(mul(montant, sub(Decimal::ONE_HUNDRED, ab)), Decimal::ONE_HUNDRED));
                let ab_txt = ab.to_string().replace('.', ",");
                let pourquoi = match gr.abattement_motif.as_deref() {
                    Some(m) => m,
                    None => "ensemble",
                };
                dit += &format!(" ; abattu de {} % au plus ({})", ab_txt, pourquoi);
            }
            let nom = &gr.nom;
            let unite = gr.unite.as_str();
            if unite == "cachet" {
                trouves.push(Plancher {
                    montant,
                    quoi: format!("CCN {}, {}, {} : {} le cachet", idcc, nom, dit, euros(montant)),
                    unite: false,
                });
            } else if !a_l_heure(gr) {
                let par = match unite {
                    "mois" => "par mois",
                    "semaine" => "la semaine",
                    "jour" => "la journée",
                    autre => return Err(format!("unité inconnue : {}", repr_texte(autre))),
                };
                trouves.push(Plancher {
                    montant,
                    quoi: format!("CCN {}, {}, {} : {} {}", idcc, nom, dit, euros(montant), par),
                    unite: true,
                });
            } else {
                let heures = gr.heures.ok_or_else(|| manque("heures"))?;
                let mut h = bas(div(montant, heures));
                if let Some(facteur) = gr.facteur.filter(|f| !f.is_zero()) {
                    // Un taux horaire qui ne vaut pas pour toutes les heures : le plus bas taux effectif.
                    h = bas(mul(h, facteur));
                    let motif = match gr.facteur_motif.as_deref().filter(|m| !m.is_empty()) {
                        Some(m) => m,
                        None => "taux effectif le plus bas",
                    };
                    dit += &format!(" ; {}", motif);
                }
                let par = match unite {
                    "mois" => "par mois",
                    "semaine" => "la semaine",
                    "jour" => "la journée",
                    "heure" => "de l'heure",
                    autre => return Err(format!("unité inconnue : {}", repr_texte(autre))),
                };
                trouves.push(Plancher {
                    montant: h,
                    quoi: format!("CCN {}, {}, {} : {} {}, pour {} h, ramené à l'heure", idcc, nom, dit, euros(montant), par, virgule(heures.round_dp(2))),
                    unite: false,
                });
            }
        }
        Ok(plus_bas(trouves, |p| p.montant))
    }

    /// L'echelon de la grille administrative de la 3090 : 1 sous 5 ans dans la
    /// fonction, puis un de plus par tranche de 5 ans passee, jusqu'au 5e.
    fn echelon_3090(&self) -> Resultat<(usize, String)> {
        let (Some(depuis), Some(jour)) = (self.fiche.poste_depuis.dite().map(|j| j.iso()), jour_de_paie(self.paie)) else {
            return Ok((1, "échelon 1, la fiche ne disant pas depuis quand (`poste_depuis`)".into()));
        };
        let veille = iso(&jour).ok_or("date illisible")?.plus_jours(-1)?.iso();
        let Some(ans) = ans_revolus(&depuis, Some(&veille)) else {
            return Ok((1, "échelon 1, `poste_depuis` illisible".into()));
        };
        let e = (ans.div_euclid(5) + 1).clamp(1, 5) as usize;
        Ok((e, format!("échelon {}, dans la fonction depuis le {}", e, Jour::lire(&depuis)?.jj_mm_aaaa())))
    }

    /// Le taux horaire minimum d'un technicien, par son niveau 3090 ou son
    /// groupe CCNEAC. `mensuel` : la ligne est un salaire mensuel de permanent.
    fn plancher_horaire(&self, mensuel: bool) -> Resultat<Option<Plancher>> {
        if self.idcc == "1285" {
            return self.plancher_horaire_1285();
        }
        if self.idcc != "3090" {
            return Ok(None);
        }
        let Some(classement) = self.m.classification_3090().filter(|c| !c.niveau.is_empty()) else {
            return Ok(None);
        };
        let niveau = classement.niveau.clone();
        let t3090 = self.c3090()?;
        let communes_cl = t3090.clauses_communes.as_ref().ok_or_else(|| manque("3090.clauses_communes"))?;
        if classement.grille == "administratif" {
            // Grille mensuelle pour un temps plein, ramenee a l'heure sur 151,67 h.
            let Some(valeurs) = communes_cl.administratifs_mensuel_echelons.du_niveau(&niveau) else { return Ok(None) };
            let (echelon, mut d_ou) = self.echelon_3090()?;
            if valeurs.is_empty() {
                return Err(socle::fichiers::trop_courte(&format!("les échelons du niveau {}", niveau)));
            }
            let montant = *valeurs.get(echelon.min(valeurs.len()) - 1).ok_or_else(|| socle::fichiers::trop_courte(&format!("l'échelon {} du niveau {}", echelon, niveau)))?;
            if valeurs.len() == 1 {
                d_ou = "sans échelon".into();
            }
            let horaire = arrondi2(montant / 151.67);
            return Ok(plancher(
                dec_f64(horaire)?,
                format!(
                    "CCN 3090, grille administrative, {}, {} : {} par mois ramenés à l'heure sur 151,67 h",
                    niveau.replace('_', " "),
                    d_ou,
                    euros(dec_f64(montant)?)
                ),
            ));
        }
        let communes = &communes_cl.techniciens;
        let (mut grille, mut ou) = (communes, "clauses communes".to_string());
        if let Some(a) = self.annexe.as_deref().filter(|a| t3090.annexes.contains_key(*a)) {
            grille = self.annexe_3090(a)?.techniciens.as_ref().ok_or_else(|| manque(&format!("3090.annexes.{}.techniciens", a)))?;
            ou = format!("annexe {}", a);
            // Un salaire mensuel : le plus bas de la grille des clauses communes et de celle de l'annexe.
            if mensuel {
                if let Some(c) = communes.du_niveau(&niveau) {
                    let en_dessous = match grille.du_niveau(&niveau) {
                        None => true,
                        Some(x) => c.0 < x.0,
                    };
                    if en_dessous {
                        grille = communes;
                        ou = "clauses communes".into();
                    }
                }
            }
        }
        let Some(montant) = grille.du_niveau(&niveau) else { return Ok(None) };
        Ok(plancher(
            montant.0,
            format!(
                "CCN 3090, {}, taux horaire {}{}",
                ou,
                niveau.replace('_', " "),
                if mensuel { " (le mensuel pour 151,67 h, ramené à l'heure)" } else { "" }
            ),
        ))
    }

    /// Le plancher d'une ligne, ou rien.
    fn plancher_de(&self, ligne: &LigneBrut) -> Resultat<Option<Plancher>> {
        let statut = self.statut();
        let heures = !ligne.heures_annexes.is_zero();
        let code = ligne.code.as_str();
        if self.idcc == "2642" {
            if statut == "technicien" && (horaire(code) || mensuel(code)) && heures {
                if let Some(p) = self.plancher_2642()? {
                    return Ok(Some(p));
                }
            }
            return self.plancher_grilles(ligne, &self.grille.grilles_complementaires);
        }
        if self.idcc == "3097" {
            if let Some(p) = self.plancher_3097(ligne)? {
                return Ok(Some(p));
            }
            return self.plancher_grilles(ligne, &self.grille.grilles_complementaires);
        }
        if self.a_grilles() {
            return self.plancher_grilles(ligne, &self.grille.grilles);
        }
        if cachet(code) && statut == "artiste" {
            return if self.idcc == "3090" { self.plancher_cachet_3090() } else { self.plancher_cachet_1285() };
        }
        if repetition(code) && statut == "artiste" {
            return self.plancher_repetition();
        }
        if horaire(code) && statut == "technicien" {
            return self.plancher_horaire(false);
        }
        // Le salaire mensuel d'un permanent de la 3090 : sa grille est mensuelle, pour 151,67 h.
        if mensuel(code) && statut == "technicien" && self.idcc == "3090" && heures {
            return self.plancher_horaire(true);
        }
        // Les mensuels des artistes de la CCNEAC : par grille, ramenes a l'heure.
        if mensuel(code) && statut == "artiste" && self.idcc == "1285" && heures {
            return self.plancher_grilles(ligne, &self.grille.grilles_complementaires);
        }
        // Un salaire mensuel de la CCNEAC : le minimum de l'echelon pour 151 h 40, ramene a l'heure.
        if mensuel(code) && statut == "technicien" && self.idcc == "1285" && heures {
            return self.plancher_horaire_1285();
        }
        Ok(None)
    }

    /// Le minimum d'une ligne « Service de repetition » dans la 3090 :
    /// (montant, d'ou il vient, a l'heure), ou rien.
    fn plancher_service_3090(&self, par_jour: Option<Decimal>) -> Resultat<Option<(Decimal, String, bool)>> {
        let refs = self.refs();
        // (montant, annexe, a l'heure, libelle)
        let mut trouves: Vec<(Decimal, String, bool, &str)> = Vec::new();
        for annexe in self.annexes_ou_la_sienne()? {
            let r = self.annexe_3090(&annexe)?.repetition.as_ref();
            let (service_musicien, heure, service_isole) = (r.and_then(|r| r.service_musicien), r.and_then(|r| r.heure), r.and_then(|r| r.service_isole));
            let musicien = musicien_dans(refs, &annexe);
            if let (true, Some(m)) = (musicien, service_musicien) {
                trouves.push((m, annexe, false, "service de répétition d'un musicien"));
            } else if let (false, Some(h)) = (musicien, heure) {
                trouves.push((h, annexe, true, "services de répétition (taux horaire)"));
            } else if let Some(s) = service_isole.filter(|_| par_jour.is_some_and(|p| p <= Decimal::ONE)) {
                trouves.push((s, annexe, false, "service isolé de répétition"));
            } else {
                return Ok(None); // une annexe candidate sans minimum : on se tait
            }
        }
        if trouves.is_empty() || trouves.iter().any(|x| x.2 != trouves[0].2) {
            return Ok(None);
        }
        let (montant, annexe, heure, lib) = plus_bas(trouves, |x| x.0).unwrap();
        Ok(Some((montant, format!("CCN 3090, annexe {}, {}", annexe, lib), heure)))
    }
}

/// La ligne de minima `refs` de l'annexe `annexe` (la 3090) est-elle celle d'un musicien ou d'un chef d'orchestre (sa
/// sorte, avant le point) ?
fn musicien_dans(refs: &std::collections::BTreeMap<String, String>, annexe: &str) -> bool {
    refs.get(annexe).is_some_and(|s| matches!(s.split('.').next(), Some("musicien" | "chef_orchestre")))
}

/// Une grille qui se ramene a l'heure : elle dit les heures que sa valeur paie.
fn a_l_heure(gr: &Grille) -> bool {
    gr.heures.is_some_and(|h| !h.is_zero())
}

fn ligne_de_l_unite(unite: &str) -> Option<&'static str> {
    match unite {
        "jour" => Some("journee"),
        "semaine" => Some("semaine"),
        "mois" => Some("mois"),
        _ => None,
    }
}

// ------------------------------------------------------------------ les deux lectures

/// Une lecture, pas un verdict : la phrase renvoie au texte pour qu'on verifie.
fn a_verifier(t: &Table, idcc: &str, phrase: String) -> Avis {
    let lien = t.convention(idcc).and_then(|c| c.lien.as_deref()).filter(|l| !l.is_empty());
    let lien: String = lien.map(|l| format!(" : {}", l)).into_iter().collect();
    Avis {
        info: true,
        texte: format!(
            "{} Nous avons pu passer à côté d'un cas qui change ce minimum : vérifiez-le sur le texte de la convention{}.",
            phrase, lien
        ),
    }
}


/// Le contexte d'un plancher pour cette paie (la convention du contrat, le metier de la fiche), passe a `f` ; rien
/// quand la table ne connait pas la convention ou que la fiche ne dit pas de metier connu.
fn avec_cx<T>(ctx: &Contexte, paie: &Paie, f: impl FnOnce(&Cx) -> Resultat<Option<T>>) -> Resultat<Option<T>> {
    let ent = &ctx.entreprise;
    let idcc = droit::minima::idcc_du_contrat(ent.idcc.texte(), &spectacle_du_contrat(paie.spectacle.as_ref()), &paie.salarie)?;
    let tab = table()?;
    let t: &Table = &tab;
    if t.convention(&idcc).is_none() {
        return Ok(None);
    }
    let Some(m) = emplois::metier(paie.salarie.code_emploi_ft.texte())? else { return Ok(None) };
    let nb_cachets = paie
        .remuneration
        .iter()
        .filter(|l| cachet(&l.code))
        .fold(Decimal::ZERO, |a, l| add(a, l.quantite))
        .trunc()
        .to_string()
        .parse::<i64>()
        .map_err(|e| e.to_string())?;
    let organisation = ent.organisation_patronale.dite().filter(|o| !o.is_empty()).cloned();
    let Some((grille, lie_depuis)) = grille_au(t, &idcc, organisation.as_deref(), premier_jour(paie).as_deref()) else { return Ok(None) };
    let cx = Cx {
        t,
        idcc: &idcc,
        grille,
        lie_depuis,
        m: &m,
        fiche: &paie.salarie,
        paie,
        ent,
        annexe: ent.annexe_convention.dite().map(|a| a.as_str().to_string()),
        organisation,
        nb_cachets,
    };
    f(&cx)
}

/// Le minimum d'une ligne de remuneration (`saisie` : la ligne telle que saisie, pour ses dates), ou rien.
fn minimum_dans(cx: &Cx, l: &LigneBrut, saisie: Option<&crate::LignePaie>) -> Resultat<Option<Minimum>> {
    let statut = cx.statut();
    // Le service de repetition de la 3090 : un minimum par service ou par heure selon
    // l'annexe, jamais le cachet de la journee.
    if repetition(&l.code) && cx.idcc == "3090" && statut == "artiste" {
        let dates: BTreeSet<String> = saisie.iter().flat_map(|s| s.dates.iter()).map(|d| d.jour().to_string()).collect();
        let par_jour = (!dates.is_empty()).then(|| div(l.quantite, Decimal::from(dates.len())));
        return Ok(cx.plancher_service_3090(par_jour)?.map(|(montant, quoi, heure)| Minimum {
            montant,
            quoi,
            par: if heure { Par::Heure } else { Par::Service },
            service_3090: true,
        }));
    }
    let Some(p) = cx.plancher_de(l)? else { return Ok(None) };
    let par = if p.unite {
        Par::Unite
    } else if horaire(&l.code) || mensuel(&l.code) {
        Par::Heure
    } else if cachet(&l.code) {
        Par::Cachet
    } else {
        Par::Service
    };
    Ok(Some(Minimum { montant: p.montant, quoi: p.quoi, par, service_3090: false }))
}

/// Les lignes saisies de cette paie, une par ligne de remuneration ; `Null` quand elles ne se correspondent pas.
pub(crate) fn saisies_de(paie: &Paie) -> Vec<Option<&crate::LignePaie>> {
    let lignes = &paie.remuneration;
    if paie.engagement.lignes.len() == lignes.len() {
        paie.engagement.lignes.iter().map(Some).collect()
    } else {
        vec![None; lignes.len()]
    }
}

/// Le minimum conventionnel de la ligne `i` de la remuneration de cette paie, tel que `avertissements` le compare
/// (les conseils de saisie s'en servent pour ne jamais proposer moins que lui ; `conseils::de_saisie`).
#[cfg(feature = "ecran")]
pub(crate) fn minimum_de_la_ligne(ctx: &Contexte, paie: &Paie, i: usize) -> Resultat<Option<Minimum>> {
    let Some(l) = paie.remuneration.get(i) else { return Ok(None) };
    let saisie = saisies_de(paie)[i];
    avec_cx(ctx, paie, |cx| minimum_dans(cx, l, saisie))
}

/// LE CACHET DE REPETITION de la convention, quand elle en a un pour ce metier : la 3090 fixe un « cachet de
/// repetition pour 1 ou 2 services de repetition dans la meme journee » (annexe 1, 84,00 EUR), un « cachet de base des
/// journees de repetition » (annexe 2, 106,20 EUR), un cachet de repetition « pour un ou deux services de repetitions de
/// quatre heures dans la meme journee » (annexe 4, 94,34 EUR), un « cachet de base par jour » de repetition (annexe 5,
/// 108,03 EUR) -- avenant du 21/07/2025, `atelier/reference/sources/legifrance-3090-avenant-salaires-20250721.md`, lus
/// dans `minima-conventionnels.json` (`annexes.<a>.repetition.cachet`). Rien pour un musicien de l'annexe 1 (« Sauf
/// dispositions particulieres prevues pour les artistes musiciens » : son cachet est un service de 3 h 30,
/// `chomage-intermittents/kali-3090-repetitions-cachets-representations-20261020.md`, KALIARTI000028157549) ; rien
/// pour une autre convention (la CCNEAC paie la repetition par service : `kali-1285-x3-remuneration-artistes-20260929.md`).
/// L'annexe de l'entreprise inconnue : le plus HAUT des cachets possibles, pour qu'un conseil ne propose jamais moins
/// que le minimum de la bonne annexe.
#[cfg(feature = "ecran")]
pub(crate) fn cachet_de_repetition(ctx: &Contexte, paie: &Paie) -> Resultat<Option<Minimum>> {
    avec_cx(ctx, paie, |cx| {
        if cx.idcc != "3090" || cx.statut() != "artiste" {
            return Ok(None);
        }
        let refs = cx.refs();
        let mut trouves: Vec<(Decimal, String, String)> = Vec::new();
        for annexe in cx.annexes_ou_la_sienne()? {
            if annexe == "1" && musicien_dans(refs, &annexe) {
                return Ok(None);
            }
            let r = cx.annexe_3090(&annexe)?.repetition.as_ref();
            let Some(c) = r.and_then(|r| r.cachet) else { return Ok(None) };
            trouves.push((c, annexe, r.and_then(|r| r.cachet_libelle.clone()).into_iter().collect()));
        }
        Ok(trouves.into_iter().max_by(|a, b| a.0.cmp(&b.0)).map(|(montant, annexe, libelle)| Minimum {
            montant,
            quoi: format!("CCN 3090, annexe {}, {}", annexe, libelle),
            par: Par::Cachet,
            service_3090: false,
        }))
    })
}

/// Les lignes de cette paie payees sous un minimum conventionnel : une phrase
/// par ligne fautive, qui dit le paye, le minimum, et la grille lue.
pub(crate) fn avertissements(ctx: &Contexte, paie: &Paie) -> Resultat<Vec<Avis>> {
    let avis = avec_cx(ctx, paie, |cx| {
        let t = cx.t;
        let idcc = cx.idcc;
        let qui = crate::qui_de(cx.fiche)?;
        let texte_source = cx.texte_de_la_grille()?;
        let mut avis = Vec::new();
        for (l, saisie) in paie.remuneration.iter().zip(saisies_de(paie)) {
            let Some(p) = minimum_dans(cx, l, saisie)? else { continue };
            if p.service_3090 {
                let (paye, unite) = if p.par == Par::Heure {
                    if l.heures_annexes.is_zero() {
                        continue;
                    }
                    (pair(div(l.montant, l.heures_annexes)), "l'heure")
                } else {
                    (l.taux, "le service")
                };
                if paye < p.montant {
                    avis.push(a_verifier(
                        t,
                        idcc,
                        format!(
                            "{}, {} : {} {}. Notre lecture de la convention donne un minimum conventionnel de {} {} ({} ; {}).",
                            qui,
                            l.designation(),
                            euros(paye),
                            unite,
                            euros(p.montant),
                            unite,
                            p.quoi,
                            texte_source
                        ),
                    ));
                }
                continue;
            }
            let (paye, unite) = match p.par {
                // Une grille a l'unite : le taux de la ligne contre le montant de la grille.
                Par::Unite => (
                    l.taux,
                    match l.code.as_str() {
                        "journee" => "la journée",
                        "semaine" => "la semaine",
                        "mois" => "le mois",
                        _ => "l'unite",
                    },
                ),
                Par::Heure => {
                    if l.heures_annexes.is_zero() {
                        continue;
                    }
                    (pair(div(l.montant, l.heures_annexes)), "l'heure")
                }
                Par::Cachet => (l.taux, "le cachet"),
                Par::Service => (l.taux, "le service"),
            };
            if paye < p.montant {
                avis.push(a_verifier(
                    t,
                    idcc,
                    format!(
                        "{}, {} : {} {}. Notre lecture de la convention donne un minimum conventionnel de {} ({} ; {}).",
                        qui,
                        l.designation(),
                        euros(paye),
                        unite,
                        euros(p.montant),
                        p.quoi,
                        texte_source
                    ),
                ));
            }
        }
        Ok(Some(avis))
    })?;
    // Sans convention ni metier au catalogue, rien a dire.
    Ok(avis.into_iter().flatten().collect())
}

/// Les lignes de cette paie payees sous le SMIC horaire. Le SMIC est la loi :
/// aucune convention, aucun statut, aucun catalogue de metiers n'entre ici.
/// Les lignes visees portent des heures reellement faites (pas un forfait) ;
/// la comparaison porte sur le total, pas sur un taux reconstitue.
pub(crate) fn sous_le_smic(ctx: &Contexte, paie: &Paie) -> Resultat<Vec<Avis>> {
    let qui = crate::qui_de(&paie.salarie)?;
    let jour = paie.engagement.fin_periode()?;
    let smic = dec_f64(ctx.smic(&jour)?)?;
    let mut avis = Vec::new();
    for l in &paie.remuneration {
        let heures = l.heures_annexes;
        if heures <= Decimal::ZERO {
            continue;
        }
        if rubrique(&ctx.racine, &l.code, &ctx.lue_au)?.heures_forfait.is_some() {
            continue;
        }
        let du = pair(mul(smic, heures));
        let paye = l.montant;
        if paye >= du {
            continue;
        }
        avis.push(Avis {
            info: false,
            texte: format!(
                "{}, {} : {} pour {} h. Le SMIC est de {} de l'heure au {} : il fallait {}, il manque {}. À corriger avant de déclarer (le SMIC vaut pour tous, art. L. 3231-2).",
                qui,
                l.designation(),
                euros(paye),
                virgule(heures.round_dp(2)),
                euros(smic),
                jour.jj_mm_aaaa(),
                euros(du),
                euros(sub(du, paye))
            ),
        });
    }
    Ok(avis)
}
