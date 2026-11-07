//! La pile de bareme, de `bareme_du_statut` a `bareme_vm` : statut, nature du contrat, convention, sante, retraite,
//! categorie, prevoyance, regime local, versement mobilite.
//!
//! Un bareme est une `forme_bareme::Pile` : chaque etage prend la pile et en rend une nouvelle. Un refus est une `String`.
//!
//! Avec elle, parce que la pile les appelle : la lecture du catalogue de prevoyance de branche (module `prevoyance`
//! ci-dessous) et l'etablissement du salarie (`etablissement_de`, `surcharger`).

use socle::texte::sans_accent;
use crate::forme_bareme::{
    CotisationDsn, CotisationModifiee, CotisationSupplementaire, Part, Pile, ReductionGenerale, Rubrique, RubriqueModifiee, TauxAtmp, TauxAtmpEcrit, ValeurAtmp,
    VersementMobiliteEcrit,
};
use crate::natures::Nature;
use crate::fiche::{ChampsEntreprise, Lue, ChampsSalarie, Etablissement, SalarieDuContrat, Statut, StatutCategorielRc};
use socle::date::Jour;
use socle::echec::APart;
use socle::num::{add, dec_f64, div, mul, sub, Decimal, Resultat};
use rust_decimal::RoundingStrategy;
use std::collections::BTreeMap;

// ------------------------------------------------------------ les petits outils

/// Le décimal en flottant.
fn flt(x: Decimal) -> f64 {
    socle::num::flottant(x)
}

/// Un decimal tel que la pile le garde : ecrit en flottant, relu par sa forme la plus courte (`socle::num::dec_f64`).
fn ecrit(x: Decimal) -> Decimal {
    match dec_f64(flt(x)) {
        Ok(d) => d,
        Err(e) => unreachable!("un décimal fini s'écrit en flottant fini : {} ({})", x, e),
    }
}

/// `round(x, 4)` d'un flottant : arrondi correct, au pair sur le demi exact.
fn arrondi4(x: f64) -> f64 {
    match format!("{:.4}", x).parse::<f64>() {
        Ok(r) => r,
        Err(e) => unreachable!("un flottant s'écrit en nombre : {} ({})", x, e),
    }
}

/// `round(Decimal, n)` : au pair.
fn rond(x: Decimal, n: u32) -> Decimal {
    x.round_dp_with_strategy(n, RoundingStrategy::MidpointNearestEven)
}

/// `"%.2f" % x` avec la virgule.
fn deux_virgule(x: f64) -> String {
    format!("{:.2}", x).replace('.', ",")
}

/// Un flottant tel qu'un message l'ecrit (`6.01`, `1.0`).
fn ecrit_f(x: f64) -> String {
    socle::date::repr_flottant(x)
}

/// L'annee d'un bareme, pour un message.
fn annee(bar: &Pile) -> String {
    bar.annee.to_string()
}

/// Un refus en mots, son detail a part.
fn refus(phrase: impl Into<String>, detail: String) -> String {
    socle::echec::avec(phrase, APart { detail: Some(detail), ..APart::default() })
}

/// La date de fin de période (ses dix premiers caractères), vide si elle manque.
fn jour_texte(fin: Option<&Jour>) -> String {
    fin.map(Jour::iso).unwrap_or_default()
}

/// L'indice de la rubrique d'identifiant `id`.
fn rang(rubriques: &[Rubrique], id: &str) -> Option<usize> {
    rubriques.iter().position(|r| r.id == id)
}

/// `v` range apres l'element `i`, a la fin sans lui.
fn insere<T>(v: &mut Vec<T>, i: Option<usize>, x: T) {
    match i {
        None => v.push(x),
        Some(i) => v.insert(i + 1, x),
    }
}

/// Un texte dit : ni absent, ni vide.
fn dit(t: &Option<String>) -> Option<&str> {
    t.as_deref().filter(|s| !s.is_empty())
}

/// Le code 81 d'une cotisation, vide s'il n'est pas dit.
fn code81(c: &CotisationDsn) -> &str {
    c.code81.as_deref().unwrap_or("")
}

/// La nature, quand elle est un permanent (`nat and nat.mensuelle`).
fn mensuelle(nat: Option<&Nature>) -> bool {
    nat.is_some_and(|n| n.mensuelle())
}

fn nom_du_salarie(sal: &SalarieDuContrat, defaut: &str) -> String {
    sal.nomme()
        .or_else(|| Some(sal.id.to_string()))
        .unwrap_or_else(|| defaut.to_string())
}

/// Ce qu'une couche ecrit d'une rubrique remplace ce que la rubrique portait.
pub(crate) fn modifier(r: &mut Rubrique, m: &RubriqueModifiee) {
    macro_rules! si {
        ($($c:ident),*) => { $( if let Some(x) = &m.$c { r.$c = Some(x.clone()); } )* };
    }
    si!(famille, libelle, base, cet_si_depasse, rempli_par, a_confirmer, source, note);
    if let Some(t) = m.taux_pat {
        r.taux_pat = Some(t.0);
    }
    if let Some(t) = m.taux_sal {
        r.taux_sal = Some(t);
    }
    // Une part effacee : la rubrique n'a plus de part de ce cote (`Rubrique::taux_pat`, nul).
    for p in m.efface.iter().flatten() {
        match p {
            Part::TauxPat => r.taux_pat = None,
            Part::TauxSal => r.taux_sal = None,
        }
    }
    if let Some(c) = &m.composants {
        r.composants = Some(c.0.clone());
    }
    for (de, vers) in [(&m.composants_pat, &mut r.composants_pat), (&m.composants_sal, &mut r.composants_sal)] {
        if let Some(c) = de {
            *vers = Some(c.clone());
        }
    }
}

/// Ce qu'une couche ecrit d'une modification s'ajoute a ce qu'une autre en ecrit (une couche de statut sur celle de sa
/// categorie).
pub(crate) fn par_dessus(base: &mut RubriqueModifiee, m: &RubriqueModifiee) {
    macro_rules! si {
        ($($c:ident),*) => { $( if m.$c.is_some() { base.$c = m.$c.clone(); } )* };
    }
    si!(famille, libelle, base, taux_pat, taux_sal, composants, composants_pat, composants_sal, cet_si_depasse, rempli_par, a_confirmer, source, note);
    // La derniere couche dit chaque part : celle qu'elle pose n'est plus effacee, celle qu'elle efface n'est plus posee.
    let mut efface: Vec<Part> = base.efface.take().unwrap_or_default();
    efface.retain(|p| !matches!((p, m.taux_pat.is_some(), m.taux_sal.is_some()), (Part::TauxPat, true, _) | (Part::TauxSal, _, true)));
    for p in m.efface.iter().flatten() {
        match p {
            Part::TauxPat => base.taux_pat = None,
            Part::TauxSal => base.taux_sal = None,
        }
        if !efface.contains(p) {
            efface.push(*p);
        }
    }
    base.efface = Some(efface).filter(|e| !e.is_empty());
}

/// Ce qu'une couche ecrit d'une cotisation de la DSN remplace ce qu'elle portait.
pub(crate) fn modifier_cotisation(c: &mut CotisationDsn, m: &CotisationModifiee) {
    if let Some(t) = m.taux {
        c.taux = Some(crate::forme_bareme::TauxDate(t));
    }
    if m.efface.iter().flatten().any(|e| *e == crate::forme_bareme::CaseCotisation::Taux) {
        c.taux = None;
    }
    if let Some(x) = &m.composants {
        c.composants = Some(x.clone());
    }
    if let Some(x) = &m.base {
        c.base = Some(x.clone());
    }
    if let Some(x) = &m.base78 {
        c.base78 = x.clone();
    }
    if let Some(x) = &m.lignes {
        c.lignes = Some(x.clone());
    }
    if let Some(x) = &m.source {
        c.source = Some(x.clone());
    }
    if let Some(x) = &m.note {
        c.note = Some(x.clone());
    }
}

/// `par_dessus`, pour une cotisation.
fn cotisation_par_dessus(base: &mut CotisationModifiee, m: &CotisationModifiee) {
    macro_rules! si {
        ($($c:ident),*) => { $( if m.$c.is_some() { base.$c = m.$c.clone(); } )* };
    }
    si!(composants, base, base78, lignes, source, note);
    // La derniere couche dit le taux : celui qu'elle pose n'est plus efface, celui qu'elle efface n'est plus pose.
    if m.taux.is_some() {
        base.taux = m.taux;
        base.efface = None;
    }
    if m.efface.as_ref().is_some_and(|e| !e.is_empty()) {
        base.taux = None;
        base.efface = m.efface.clone();
    }
}

// ---------------------------------------------------------------- le statut

/// Le bareme applicable a ce statut, ou une erreur qui dit ce qui manque.
/// Trois issues : le statut du corps (on rend le bareme), une section
/// `par_statut` verifiee et sans trou (ses sections remplacent celles du
/// corps), sinon un refus qui nomme tout ce qui bloque.
pub fn bareme_du_statut(mut bar: Pile, statut: &str) -> Resultat<Pile> {
    let statut = statut.trim().to_lowercase();
    let statuts = bar.statuts.clone();
    if statut.is_empty() || statuts.contains(&statut) {
        return Ok(bar);
    }

    let Some(section) = bar.par_statut.get(&statut).cloned() else {
        return Err(refus(
            format!("Le barème {} ne sait pas payer ce statut : le signaler au développeur.", annee(&bar)),
            format!(
                "Statut '{}' : le bareme {} ne sait pas le calculer.\n  \
Il decrit les statuts {}. L'abattement de 30 % sur les taux \
(arrete du 24/01/1975) ne vaut que pour les artistes, et il est \
cuit dans ces taux : s'en servir pour un autre statut donnerait \
des cotisations d'artiste a quelqu'un qui n'en est pas, sans que \
rien ne le signale.\n  \
Ajouter une section par_statut['{}'] au bareme.",
                statut,
                annee(&bar),
                statuts.join(", "),
                statut
            ),
        ));
    };

    let mut trous: Vec<String> = Vec::new();
    for r in section.rubriques_bulletin.iter().flatten() {
        if dit(&r.rempli_par).is_some() {
            continue;
        }
        if r.taux_pat.is_none() && r.taux_sal.is_none() && r.composants.as_deref().is_none_or(<[f64]>::is_empty) {
            trous.push(format!("{} ({})", r.id, dit(&r.a_confirmer).unwrap_or("taux absent")));
        }
    }
    if !trous.is_empty() || section.verifie != Some(true) {
        let mut detail: Vec<String> = Vec::new();
        if !trous.is_empty() {
            detail.push(format!("{} rubrique(s) sans taux :\n    - {}", trous.len(), trous.join("\n    - ")));
        }
        let detail = if detail.is_empty() {
            "(rien de precis n'est signale)".to_string()
        } else {
            detail.join("\n  ")
        };
        return Err(refus(
            format!("Le barème {} ne paie pas encore ce statut : ses taux ne sont pas vérifiés.", annee(&bar)),
            format!(
                "Statut '{}', bareme {} : refus de payer.\n  {}\n  {}\n  \
Rien de tout cela ne se devine : un taux se releve (Urssaf, \
BOSS, fiche de parametrage Audiens, bulletin de reference), un \
mecanisme se decide. Tant que la section n'est pas marquee \
\"verifie\": true par un humain, le moteur refuse.",
                statut,
                annee(&bar),
                dit(&section.note).unwrap_or("Section non marquee verifiee."),
                detail
            ),
        ));
    }

    if let Some(r) = section.rubriques_bulletin {
        bar.rubriques_bulletin = r;
    }
    if let Some(c) = section.cotisations_dsn {
        bar.cotisations_dsn = c;
    }
    if section.reduction_generale.is_some() {
        bar.reduction_generale = section.reduction_generale;
    }
    Ok(bar)
}

// --------------------------------------------------- la nature du contrat

/// Le bareme du statut, avec la surcouche de la NATURE DU CONTRAT
/// (`par_nature`). Sans section verifiee et sans trou, on REFUSE.
pub fn bareme_de_la_nature(bar: Pile, nat: &Nature) -> Resultat<Pile> {
    let nom = match nat.bareme() {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => return Ok(bar),
    };
    let Some(sec) = bar.par_nature.get(&nom).cloned() else {
        return Err(refus(
            format!("Le barème {} ne sait pas payer cette nature de contrat : le signaler au développeur.", annee(&bar)),
            format!(
                "Contrat de nature '{}' : le bareme {} n'a pas de section \
par_nature['{}'].\n  \
Le corps de ce bareme decrit un INTERMITTENT : Conges \
Spectacles, chomage majore, formation a 2 %, assiettes majorees \
pour conges. Un permanent n'a rien de tout cela, et il a en plus \
une mutuelle sante obligatoire. Le payer avec ce bareme-la \
donnerait des cotisations fausses sans que rien ne le signale.",
                nat.cle,
                annee(&bar),
                nom
            ),
        ));
    };
    let mut trous: Vec<String> = sec
        .rubriques_ajoutees
        .iter()
        .flatten()
        .filter(|r| r.taux_pat.is_none() && r.taux_sal.is_none() && r.composants.as_deref().is_none_or(<[f64]>::is_empty) && dit(&r.rempli_par).is_none())
        .map(|r| format!("rubriques_ajoutees[{}]", r.id))
        .collect();
    // Ce que la section dit encore « a confirmer ».
    trous.extend(sec.rubriques.iter().flatten().filter(|(_, m)| dit(&m.a_confirmer).is_some()).map(|(id, _)| format!("rubriques.{}", id)));
    trous.extend(sec.rubriques_ajoutees.iter().flatten().filter(|r| dit(&r.a_confirmer).is_some()).map(|r| format!("rubriques_ajoutees[{}]", r.id)));
    let mut uniques: Vec<String> = Vec::new();
    for t in trous {
        if !t.is_empty() && !uniques.contains(&t) {
            uniques.push(t);
        }
    }
    if !uniques.is_empty() || sec.verifie != Some(true) {
        return Err(refus(
            format!("Le barème {} ne paie pas encore cette nature de contrat : ses taux ne sont pas vérifiés.", annee(&bar)),
            format!(
                "Contrat de nature '{}', bareme {} : refus de payer.\n  {}\n  A remplir : {}\n  \
Un taux se releve (BOSS, Agirc-Arrco, contrat de mutuelle, \
fiche de parametrage Audiens), il ne se devine pas. Tant que la \
section par_nature['{}'] n'est pas marquee \"verifie\": true \
par un humain, le moteur refuse.",
                nat.cle,
                annee(&bar),
                dit(&sec.note).unwrap_or("Section non marquee verifiee."),
                if uniques.is_empty() { "(rien de precis)".to_string() } else { uniques.join(", ") },
                nom
            ),
        ));
    }
    Ok(empiler(
        bar,
        Couche {
            retirees: sec.rubriques_retirees.as_deref(),
            rubriques: sec.rubriques.as_ref(),
            ajoutees: sec.rubriques_ajoutees.as_deref(),
            cotisations_dsn: sec.cotisations_dsn.as_ref(),
            prevoyance_retraite_supp: sec.cotisation_prevoyance_retraite_supp.as_ref(),
            sante_patronale: sec.cotisation_sante_patronale.as_ref(),
            majoration_conges_115: sec.majoration_conges_115.map(|n| n.0),
            majoration_conges_110: sec.majoration_conges_110.map(|n| n.0),
            reduction_generale: sec.reduction_generale.as_ref(),
        },
    ))
}

/// LA CONTRIBUTION CPF-CDD d'un contrat qui l'ouvre (`assujettissement::contrat_ouvre_cpf_cdd` :
/// un CDD de droit commun, ni saisonnier, ni CDDU, ni contrat aide) : la section `cpf_cdd` de la nature du contrat
/// (`par_nature.<nature>.cpf_cdd`) ajoute son taux comme un composant de la rubrique qu'elle nomme (« Autres
/// contributions dues par l'employeur », a cote de la formation professionnelle) et sa cotisation au nominatif (129). Le
/// taux vient du texte (C. trav. L. 6331-6, 1 %), cite par la section. Une nature sans section, pour un contrat qui
/// ouvre la contribution : refus (regle 1).
pub fn bareme_cpf_cdd(mut bar: Pile, nat: &Nature, cpf_cdd: bool) -> Resultat<Pile> {
    if !cpf_cdd {
        return Ok(bar);
    }
    let nom = nat.bareme().unwrap_or_default().to_string();
    let Some(sec) = bar.par_nature.get(&nom).and_then(|n| n.cpf_cdd.clone()) else {
        return Err(refus(
            format!("Le barème {} ne porte pas la contribution formation d'un CDD : le signaler au développeur.", annee(&bar)),
            format!(
                "Contrat de nature '{}' : il ouvre la contribution CPF-CDD (code du travail L. 6331-6), et le bareme {} n'a pas de \
section par_nature['{}'].cpf_cdd qui dise son taux et sa cotisation. Rien n'est calcule.",
                nat.cle,
                annee(&bar),
                nom
            ),
        ));
    };
    let incomplet = |cle: &str, lu: &str| socle::echec::a_la_cle("Le barème de l'année est incomplet : le signaler au développeur.", cle, lu);
    let r = bar
        .rubriques_bulletin
        .iter_mut()
        .find(|r| r.id == sec.rubrique)
        .ok_or_else(|| incomplet(&format!("par_nature.{}.cpf_cdd.rubrique", nom), &sec.rubrique))?;
    if r.composants.as_deref().is_none_or(<[f64]>::is_empty) {
        let premier = r.taux_pat.ok_or_else(|| incomplet(&format!("rubriques_bulletin[{}].taux_pat", sec.rubrique), ""))?;
        r.composants = Some(vec![premier]);
    }
    let composants = r.composants.get_or_insert_with(Vec::new);
    composants.push(sec.taux);
    let total: f64 = composants.iter().sum();
    // La somme affichee, au millieme comme les composants du bareme (0,30 + 0,016 + 0,55 + 1,0 = 1,866).
    r.taux_pat = Some((total * 1000.0).round() / 1000.0);
    bar.cotisations_dsn.push(sec.cotisation_dsn);
    Ok(bar)
}

/// LA MAJORATION DU CHOMAGE QUE LE CONTRAT NE DOIT PAS (`majoration_chomage::due` : un CDD d'usage de plus de trois mois).
/// Le bareme la porte sur le corps (`majoration_chomage_cddu` : la ligne du bulletin, son taux, sa cotisation 041) ; un
/// contrat qui ne la doit pas paie la ligne sans elle et ne declare pas la cotisation. Une pile qui ne la porte pas (un
/// permanent) n'a rien a retirer.
pub fn bareme_majoration_chomage(mut bar: Pile, due: bool) -> Resultat<Pile> {
    let Some(sec) = bar.majoration_chomage_cddu.clone() else { return Ok(bar) };
    let porte = bar.cotisations_dsn.iter().any(|c| code81(c) == sec.code81);
    if due || !porte {
        return Ok(bar);
    }
    let taux = dec_f64(sec.taux)?;
    let r = bar.rubriques_bulletin.iter_mut().find(|r| r.id == sec.rubrique).ok_or_else(|| {
        socle::echec::a_la_cle("Le barème de l'année est incomplet : le signaler au développeur.", "majoration_chomage_cddu.rubrique", &sec.rubrique)
    })?;
    let avant = dec_f64(r.taux_pat.ok_or_else(|| socle::fichiers::absent("taux_pat"))?)?;
    r.taux_pat = Some(flt(sub(avant, taux)));
    bar.cotisations_dsn.retain(|c| code81(c) != sec.code81);
    Ok(bar)
}

/// LA TAXE D'APPRENTISSAGE NON DUE. Quand la fiche entreprise dit un motif de non-assujettissement
/// (table MNATA, `taxe_apprentissage_motif_non_assujettissement`) qui vaut pour la periode d'emploi `fin`
/// (`apprentissage::taxe_apprentissage_non_due_au_mois`), la taxe n'est pas due : C. trav. L. 6241-1, I et III, dit
/// qui la doit et qui n'en est « pas redevable ». Une taxe que l'employeur ne doit pas ne se calcule pas : la ligne du
/// bulletin qui la porte (`"taxe_apprentissage": true` dans le bareme, part principale et solde) et sa cotisation
/// individuelle (81.001 = 130, « Part principale de la taxe d'apprentissage », norme DSN, bloc 81) sortent de la pile ;
/// le bordereau retire ses CTP 992 et 993 par la meme decision (`dsn::lignes_bordereau`) : le bulletin, le nominatif
/// et le bordereau disent la meme taxe. Un motif qui
/// vaut, et une pile sans ligne marquee : refus (regle 1), plutot qu'une taxe qu'on garderait sans le dire.
pub fn bareme_taxe_apprentissage(mut bar: Pile, ent: &ChampsEntreprise, fin: Option<&Jour>) -> Resultat<Pile> {
    let Some(fin) = fin else { return Ok(bar) };
    if !crate::apprentissage::taxe_apprentissage_non_due_au_mois(ent, fin.an, fin.mois)? {
        return Ok(bar);
    }
    let avant = bar.rubriques_bulletin.len();
    bar.rubriques_bulletin.retain(|r| r.taxe_apprentissage != Some(true));
    if bar.rubriques_bulletin.len() == avant {
        return Err(refus(
            format!("Le barème {} ne sait pas retirer la taxe d'apprentissage du bulletin : le signaler au développeur.", annee(&bar)),
            format!(
                "La fiche entreprise dit la compagnie non assujettie a la taxe d'apprentissage pour {:02}/{} (motif MNATA), et le bareme {} n'a aucune ligne marquee \"taxe_apprentissage\": true : le moteur ne sait pas quelle ligne retirer du bulletin. Marquer la ligne de la taxe dans le bareme. Rien n'est calcule.",
                fin.mois,
                fin.an,
                annee(&bar)
            ),
        ));
    }
    bar.cotisations_dsn.retain(|c| code81(c) != "130");
    Ok(bar)
}

/// Ce qu'une section de surcouche change aux rubriques du bulletin et au reste de la pile (`empiler`).
struct Couche<'a> {
    retirees: Option<&'a [String]>,
    rubriques: Option<&'a BTreeMap<String, RubriqueModifiee>>,
    ajoutees: Option<&'a [Rubrique]>,
    cotisations_dsn: Option<&'a Vec<CotisationDsn>>,
    prevoyance_retraite_supp: Option<&'a CotisationSupplementaire>,
    sante_patronale: Option<&'a CotisationSupplementaire>,
    majoration_conges_115: Option<Decimal>,
    majoration_conges_110: Option<Decimal>,
    reduction_generale: Option<&'a ReductionGenerale>,
}

/// Applique une section de surcouche sur les rubriques du bulletin :
/// remplacer par identifiant, retirer, ajouter apres une rubrique nommee.
fn empiler(mut bar: Pile, sec: Couche) -> Pile {
    if let Some(retirees) = sec.retirees {
        bar.rubriques_bulletin.retain(|r| !retirees.contains(&r.id));
    }
    for r in &mut bar.rubriques_bulletin {
        if let Some(m) = sec.rubriques.and_then(|mods| mods.get(&r.id)) {
            modifier(r, m);
        }
    }
    for a in sec.ajoutees.into_iter().flatten() {
        let mut a = a.clone();
        let i = a.apres.take().and_then(|apres| rang(&bar.rubriques_bulletin, &apres));
        insere(&mut bar.rubriques_bulletin, i, a);
    }
    if let Some(c) = sec.cotisations_dsn {
        bar.cotisations_dsn = c.clone();
    }
    if let Some(c) = sec.prevoyance_retraite_supp {
        bar.cotisation_prevoyance_retraite_supp = Some(c.clone());
    }
    if let Some(c) = sec.sante_patronale {
        bar.cotisation_sante_patronale = Some(c.clone());
    }
    if let Some(m) = sec.majoration_conges_115 {
        bar.majoration_conges_115 = m;
    }
    if let Some(m) = sec.majoration_conges_110 {
        bar.majoration_conges_110 = m;
    }
    // La reduction generale se complete, elle ne se remplace pas.
    if let Some(s) = sec.reduction_generale {
        let rg = bar.reduction_generale.get_or_insert_with(ReductionGenerale::default);
        macro_rules! si {
            ($($c:ident),*) => { $( if s.$c.is_some() { rg.$c = s.$c.clone(); } )* };
        }
        si!(verifie, formule, tmin, tdelta, puissance, t, plafond_smic, smic_horaire_reference, imputation, majoration_caisse_conges, source, note);
        if s.statuts.is_some() {
            rg.statuts = s.statuts.clone();
        }
        if let Some(d) = &s.dsn {
            let r = rg.dsn.get_or_insert_with(Default::default);
            macro_rules! si_dsn {
                ($($c:ident),*) => { $( if d.$c.is_some() { r.$c = d.$c.clone(); } )* };
            }
            si_dsn!(code81_urssaf, code81_chomage, code81_retraite, ctp, ctp_regularisation, qualifiant, cle_total_chomage, cle_total_urssaf, source, note);
        }
    }
    bar
}

// ------------------------------------------------------------ les cadres

/// S21.G00.40.003 = 01 : cadre au sens de la retraite complementaire.
pub fn est_cadre(sal: &ChampsSalarie) -> bool {
    sal.statut_categoriel_rc.dite() == Some(&StatutCategorielRc::Cadre)
}

/// Le salarie travaille-t-il dans le Bas-Rhin, le Haut-Rhin ou la Moselle ?
/// Le code postal de SON etablissement, sinon celui de l'entreprise.
pub fn alsace_moselle(ent: &ChampsEntreprise, sal: &SalarieDuContrat) -> bool {
    let mut cp = ent.code_postal.texte().trim();
    let nic = sal.etablissement.as_deref().unwrap_or("").trim();
    for e in ent.etablissements.dite().into_iter().flatten() {
        if !nic.is_empty() && e.nic.texte().trim() == nic {
            let c = e.code_postal.texte().trim();
            if !c.is_empty() {
                cp = c;
            }
        }
    }
    let debut: String = cp.chars().take(2).collect();
    matches!(debut.as_str(), "57" | "67" | "68")
}

/// La ligne mutuelle retiree du bulletin.
fn sans_mutuelle(mut bar: Pile) -> Pile {
    bar.rubriques_bulletin.retain(|r| r.id != "mutuelle");
    bar
}

/// Pose la cotisation de la mutuelle sur sa rubrique.
fn poser_mutuelle(mut bar: Pile, base: Option<&str>, total: Decimal, pat: Decimal, libelle: Option<&str>) -> Pile {
    for r in bar.rubriques_bulletin.iter_mut().filter(|r| r.id == "mutuelle") {
        if let Some(b) = base {
            r.base = Some(b.to_string());
        }
        r.taux_pat = Some(flt(rond(mul(total, pat), 4)));
        r.taux_sal = Some(flt(rond(sub(total, mul(total, pat)), 4)));
        if let Some(l) = libelle {
            r.libelle = Some(l.to_string());
        }
    }
    bar
}

/// Ce que la convention fait de la ligne mutuelle.
pub(crate) enum Mutuelle {
    Sans,
    Pose { base: String, total: Decimal, pat: Decimal },
}

/// Le champ de la fiche entreprise qui dit ce que le contrat de sante de la compagnie fait d'un mois entierement suspendu
/// sans indemnisation, hors de la 3090 (`logiciel/donnees/champs-base.json`, `atelier/docs/ARCHITECTURE.md`) : « suspendue » (les
/// garanties sont suspendues, rien sur la paie), « maintenue » (garanties et cotisations maintenues sur la paie).
pub const SANTE_SUSPENSION: &str = "sante_suspension_non_indemnisee";
pub const SANTE_SUSPENSION_SUSPENDUE: &str = "suspendue";
pub const SANTE_SUSPENSION_MAINTENUE: &str = "maintenue";

/// La garantie sante est-elle sans objet ce mois-la pour la fiche `sal` que la paie lit : une dispense
/// (`sante_dispense`), ou un mois entierement suspendu sans indemnisation (`SalarieDuContrat::sante_suspendue`) : le bareme pose alors la
/// paie sans mutuelle (`bareme_sante`), et la DSN ne rattache pas le salarie a l'adhesion sante ce mois-la
/// (`dsn::affiliations_du_permanent`), comme pour une dispense.
pub fn sans_sante(sal: &SalarieDuContrat) -> bool {
    !sal.sante_dispense.texte().is_empty() || sal.sante_suspendue.as_deref().is_some_and(|s| !s.is_empty())
}

/// La cotisation globale minimale des frais de sante d'un permanent (% du
/// PMSS, salarie seul) que la convention impose a cette date, et d'ou elle
/// vient -- ou `(None, "")` quand la convention n'en fixe pas.
pub fn minimum_sante(ent: &ChampsEntreprise, jour: &str) -> Resultat<(Option<Decimal>, String)> {
    prevoyance::minimum_sante(ent, jour)
}

/// Le niveau souscrit cotise-t-il au moins le minimum de la convention ?
fn controler_minimum_sante(ent: &ChampsEntreprise, sal: &SalarieDuContrat, niveau: &str, assure: Decimal, fin: Option<&Jour>) -> Resultat<()> {
    let (mini, origine) = minimum_sante(ent, &jour_texte(fin))?;
    let mini = match mini {
        Some(m) => m,
        None => return Ok(()),
    };
    if assure < mini {
        return Err(socle::echec::avec(
            format!("{} : la complémentaire santé de son niveau est sous le minimum de la convention : vérifier le niveau souscrit sur sa fiche.", nom_du_salarie(sal, "ce salarié")),
            APart {
                cle: Some("sante_svp.niveau".into()),
                detail: Some(format!(
                    "{} : complementaire sante au niveau {}, {} % du PMSS pour le \
salarie seul -- SOUS le minimum de la convention, {} % ({} ; \
accord du 19/07/2024, art. 13).\n  \
Soit le niveau souscrit est plus haut (le lire sur le bulletin \
d'adhesion, `sante_svp.niveau`), soit les taux des niveaux ont \
change et sont a relire sur la fiche de parametrage Audiens de \
l'annee (bareme, rubrique `mutuelle`, `releve_svp_sante`). Rien \
n'est calcule.",
                    nom_du_salarie(sal, "ce salarie"),
                    niveau,
                    deux_virgule(flt(assure)),
                    deux_virgule(flt(mini)),
                    origine
                )),
                ..APart::default()
            },
        ));
    }
    Ok(())
}

/// D'OU VIENT LA COMPLEMENTAIRE SANTE D'UN SALARIE AU MOIS sous la convention de la fiche entreprise `ent`, quand ce
/// n'est pas un niveau dit sur sa fiche, dans l'ordre ou `bareme_sante` les lit : le regime de la branche
/// (`sante_conventionnelle`) ou le contrat de la compagnie. `None` : la 3090, dont le niveau se lit sur le bulletin
/// d'adhesion du salarie (`sante_svp`), ou une compagnie qui n'a pas dit sa convention.
pub fn sante_hors_fiche(ent: &ChampsEntreprise) -> Resultat<Option<String>> {
    let idcc = ent.idcc.texte();
    if prevoyance::a_un_regime_de_sante(ent)? {
        return Ok(Some(format!(
            "Sa complémentaire santé est le régime de la convention {} : le taux est celui de la branche, et la part de l'employeur se dit dans Configuration › Entreprise, « Santé des permanents ». Une dispense se dit ci-dessous.",
            idcc
        )));
    }
    if idcc.is_empty() || idcc == "3090" {
        return Ok(None);
    }
    Ok(Some(format!(
        "Sa complémentaire santé est le contrat de la compagnie (la convention {} n'a pas de régime de santé à elle) : son taux et la part de l'employeur se disent dans Configuration › Entreprise, « Santé des permanents ». Une dispense se dit ci-dessous.",
        idcc
    )))
}

/// Le nombre entier d'un montant de la fiche (le nombre d'enfants).
fn entier_de(m: &crate::fiche::Montant) -> Resultat<i64> {
    use rust_decimal::prelude::ToPrimitive;
    m.decimal().trunc().to_i64().ok_or_else(|| socle::num::illisible(&m.decimal().to_string()))
}

/// La complementaire sante d'un permanent, selon SON contrat a lui.
pub fn bareme_sante(bar: Pile, ent: &ChampsEntreprise, sal: &SalarieDuContrat, nat: Option<&Nature>, fin: Option<&Jour>) -> Resultat<Pile> {
    if !mensuelle(nat) {
        return Ok(bar);
    }
    match prevoyance::sante_conventionnelle(ent, sal, fin)? {
        Some(Mutuelle::Sans) => return Ok(sans_mutuelle(bar)),
        Some(Mutuelle::Pose { base, total, pat }) => {
            return Ok(poser_mutuelle(bar, Some(&base), total, pat, Some("Complémentaire santé conventionnelle")))
        }
        None => {}
    }
    let idcc = ent.idcc.texte();
    if !idcc.is_empty() && idcc != "3090" {
        // Une convention sans regime de sante a elle : le contrat de l'entreprise.
        let choix = ent.sante_permanents.dite();
        let Some(taux_contrat) = choix.and_then(|c| c.taux_contrat.dite()) else {
            return Err(format!(
                "{} : sa complémentaire santé ne se calcule pas encore. Un salarié au mois y est affilié \
d'office (article L911-7 du code de la sécurité sociale), et la convention {} n'a pas de régime de santé à elle : \
c'est le contrat de la compagnie qui fixe la cotisation. À dire dans Configuration › Entreprise, « Santé des \
permanents » : le taux du contrat (en % du plafond mensuel, pour le salarié seul) et la part de l'employeur.",
                nom_du_salarie(sal, "ce salarie"),
                idcc
            ));
        };
        if sans_sante(sal) {
            return Ok(sans_mutuelle(bar));
        }
        let total = taux_contrat.decimal();
        let pp = choix.and_then(|c| c.part_patronale.dite());
        let minimum = bar.sante_obligatoire.as_ref().ok_or_else(|| socle::fichiers::absent("sante_obligatoire"))?.part_employeur_minimum;
        let part = pp.map_or(minimum, |m| m.decimal());
        if part < minimum || part > Decimal::ONE_HUNDRED {
            return Err(format!(
                "Fiche Entreprise, « Santé des permanents », part employeur {} % : \
l'employeur en paie au moins la moitie (art. L911-7 du code de \
la Securite sociale), et pas plus de 100.",
                pp.map_or(minimum.to_string(), |m| m.ecrit())
            ));
        }
        let pat = div(part, Decimal::ONE_HUNDRED);
        return Ok(poser_mutuelle(bar, Some("pss_periode"), total, pat, Some("Complémentaire santé (contrat de l'entreprise)")));
    }
    // La dispense d'un permanent 3090 : l'accord du 19/07/2024 (art. 9.1, 9.2,
    // `atelier/reference/sources/kali-3090-accord-sante-permanents-20240719-20261011.md`) rend l'adhesion obligatoire
    // « sous reserve des cas de dispenses facultatives visees a l'article 9.2.1 et des cas de dispenses de droit visees
    // a l'article 9.2.2 » ; le salarie dispense n'adhere pas au contrat collectif : pas de mutuelle, et la DSN ne le
    // rattache pas a la sante (`dsn::affiliations_du_permanent`). Ses bornes sont deja lues
    // (`calcul::dispense_de_la_periode`). Un mois entierement suspendu sans indemnisation suspend les garanties (art. 4.2 :
    // `calcul::sante_de_la_suspension`) : pas de mutuelle non plus (`calcul::sans_sante`).
    if sans_sante(sal) {
        return Ok(sans_mutuelle(bar));
    }
    let releve = nat
        .and_then(|n| n.bareme())
        .and_then(|b| bar.par_nature.get(b))
        .and_then(|s| s.rubriques_ajoutees.as_ref()?.iter().find(|r| r.id == "mutuelle"))
        .and_then(|r| r.releve_svp_sante.clone());
    let Some(releve) = releve.filter(|r| !r.niveaux.is_empty()) else {
        return Ok(bar);
    };
    let choix = sal.sante_svp.dite();
    let niveau = choix.map_or("", |c| c.niveau.texte()).to_string();
    let Some(taux) = releve.niveaux.get(&niveau) else {
        return Err(format!(
            "{} : sa complémentaire santé n'est pas encore dite. Un salarié au mois y est affilié \
d'office (article L911-7 du code de la sécurité sociale), et son niveau se lit sur son bulletin d'adhésion. \
À choisir sur sa fiche, Configuration › Salariés, « Complémentaire santé » : le niveau, ou sa dispense s'il en a une.",
            nom_du_salarie(sal, "ce salarie")
        ));
    };
    controler_minimum_sante(ent, sal, &niveau, taux.assure, fin)?;
    let mut total = taux.assure;
    if choix.is_some_and(|c| c.conjoint.oui()) {
        total = add(total, taux.conjoint);
    }
    let enfants = match choix.and_then(|c| c.enfants.dite()) {
        Some(e) => entier_de(e)?,
        None => 0,
    };
    total = add(total, mul(taux.enfant, Decimal::from(enfants)));
    // Au moins la moitie a la charge de l'employeur (L911-7) : le releve du bareme dit la part.
    let part_pat = div(releve.part_employeur.0, Decimal::ONE_HUNDRED);
    let bar = poser_mutuelle(bar, None, total, part_pat, None);
    Ok(composant_79(bar, "mutuelle", releve.composant_79.as_ref().map_or("", |c| c.genre.as_str())))
}

/// Le type de composant de base assujettie (S21.G00.79.001) que la base 31 de la ligne `id` declare, quand la source de
/// son taux le dit (`composant_79` sur la ligne ; `dsn::cotisations` le lit) : sans lui, la DSN garde la tranche A.
fn composant_79(mut bar: Pile, id: &str, type_79: &str) -> Pile {
    if !type_79.is_empty() {
        for r in bar.rubriques_bulletin.iter_mut().filter(|r| r.id == id) {
            r.composant_79 = Some(type_79.to_string());
        }
    }
    bar
}

/// LE TAUX AT/MP DU BAREME, RESOLU : celui que la paie applique (`taux_atmp_applique` : le dit, ou celui de l'arrete pour
/// un risque « TC ») s'ecrit dans `taux_atmp.valeur`, que la DSN declare (S21.G00.40.043) et que ses CTP portent ; `None`
/// quand il ne se resout pas (`bareme_atmp` dit alors pourquoi). La compagnie et chacun de ses etablissements
/// (`surcharger`) passent par ici.
pub fn taux_atmp_resolu(mut bar: Pile, ent: &ChampsEntreprise) -> (Pile, Option<Decimal>) {
    let Some(t) = &bar.taux_atmp else { return (bar, None) };
    let alsace = t.alsace_moselle == Some(true) || (bar.etablissement.is_none() && en_alsace_moselle(ent.code_postal.texte()));
    let applique = taux_atmp_applique(&risques_de(&bar), t, alsace);
    if let (Some(x), Some(t)) = (applique, bar.taux_atmp.as_mut()) {
        if t.valeur.is_none() {
            t.valeur = Some(ecrit(x));
        }
    }
    (bar, applique)
}

/// Le taux AT/MP : un seul endroit ou l'ecrire, les rubriques s'en deduisent. Le taux que la paie applique
/// (`taux_atmp_applique` : le dit, ou celui de l'arrete pour un risque « TC ») s'ecrit dans `taux_atmp.valeur`, que la
/// DSN declare (S21.G00.40.043).
pub fn bareme_atmp(bar: Pile, ent: &ChampsEntreprise) -> Resultat<Pile> {
    let (mut bar, applique) = taux_atmp_resolu(bar, ent);
    if !bar.rubriques_bulletin.iter().any(|r| r.facteur_atmp.is_some()) {
        return Ok(bar);
    }
    let Some(taux) = applique else {
        let a = annee(&bar);
        let t = bar.taux_atmp.clone().unwrap_or_default();
        // Le taux d'une annee que la compagnie n'a pas dit, et que celui d'avant ne donne pas (`taux_atmp_reporte`).
        if let Some(pourquoi) = t.manque {
            return Err(pourquoi);
        }
        return Err(if t.code_risque.as_deref() == Some(SANS_NOTIFICATION) {
            format!(
                "Le taux AT/MP {} manque : le code risque est 999ZZ (pas encore de notification de la CARSAT), que la DSN déclare sans taux, et aucun texte ne dit quel taux le bulletin cotise en attendant. Le code et le taux notifiés se saisissent dans Configuration › Entreprise, « Accidents du travail (AT/MP) », dès que la notification arrive (compte AT/MP sur net-entreprises).",
                a
            )
        } else {
            format!(
                "Le taux AT/MP {} manque. Il se lit sur la notification de la CARSAT (compte AT/MP sur net-entreprises) et se saisit avec le code risque dans Configuration › Entreprise, « Accidents du travail (AT/MP) ». Il est propre à l’établissement : le moteur ne le devine pas.",
                a
            )
        });
    };
    for r in &mut bar.rubriques_bulletin {
        if let Some(f) = r.facteur_atmp {
            r.taux_pat = Some(flt(rond(mul(taux, dec_f64(f)?), 3)));
        }
    }
    Ok(bar)
}

/// La prevoyance d'un permanent : la CONVENTION la fixe, pas le bareme.
pub fn bareme_prevoyance(mut bar: Pile, ent: &ChampsEntreprise, sal: &SalarieDuContrat, nat: Option<&Nature>, fin: Option<&Jour>) -> Resultat<Pile> {
    if !mensuelle(nat) {
        return Ok(bar);
    }
    let cot = prevoyance::cotisation(sal, est_cadre(sal), fin, ent)?;
    let mut rubriques: Vec<Rubrique> = Vec::new();
    let mut posee = false;
    for r in std::mem::take(&mut bar.rubriques_bulletin) {
        if r.id != "prevoyance" {
            rubriques.push(r);
            continue;
        }
        let famille = dit(&r.famille).unwrap_or("SANTÉ").to_string();
        let mut rr = r;
        rr.taux_pat = Some(cot.t1.taux_pat);
        rr.taux_sal = Some(cot.t1.taux_sal);
        if cot.t1.base.is_some() {
            rr.base = cot.t1.base.clone();
        }
        rubriques.push(rr);
        posee = true;
        if let Some(t2) = &cot.t2 {
            rubriques.push(Rubrique {
                id: "prevoyance_t2".into(),
                famille: Some(famille),
                libelle: Some(if t2.obligatoire { "Prévoyance Tranche 2" } else { "Prévoyance Tranche 2 (extension conventionnelle)" }.into()),
                base: Some(t2.base.clone().ok_or_else(|| socle::fichiers::absent("t2_base"))?),
                taux_pat: Some(t2.taux_pat),
                taux_sal: Some(t2.taux_sal),
                ..Rubrique::default()
            });
        }
    }
    if cot.sans_regime {
        // Une convention sans regime pour ses non cadres : pas de ligne.
        rubriques.retain(|r| r.id != "prevoyance");
    }
    if !posee {
        return Err(refus(
            format!("Le barème {} ne porte pas la prévoyance d'un permanent : le signaler au développeur.", annee(&bar)),
            format!(
                "Le bareme {} n'a pas de rubrique `prevoyance` pour un permanent. \
Elle doit exister, meme sans taux : c'est elle que la convention \
remplit (voir par_nature.permanent.rubriques.prevoyance).",
                annee(&bar)
            ),
        ));
    }
    bar.rubriques_bulletin = rubriques;
    Ok(bar)
}

/// L'entreprise est-elle hors du dispositif CASC-SVP ? (CCN 3090, 4.22 B)
pub fn casc_exemptee(ent: &ChampsEntreprise) -> Resultat<bool> {
    if !ent.casc_svp_exemptee.oui() {
        return Ok(false);
    }
    let rep = ent.representation_personnel.code();
    if rep != "cse" && rep != "cse_adherent_fnas" {
        return Err(socle::echec::avec(
            "La fiche entreprise dit la compagnie exemptée de la CASC-SVP, et l'exemption demande un comité social et économique : le déclarer, ou décocher l'exemption.",
            APart {
                cle: Some("casc_svp_exemptee".into()),
                detail: Some(format!(
                    "config/entreprise.json : `casc_svp_exemptee` est coche, mais \
l'exemption de la CASC-SVP ne vaut que pour une entreprise d'au \
moins 50 salaries dotee d'un CSE (CCN 3090, art. 4.22 B), et \
`representation_personnel` dit '{}'. Declarer le CSE (<< cse >>) \
ou decocher l'exemption. Rien n'est calcule.",
                    if rep.is_empty() { "rien" } else { rep }
                )),
                ..APart::default()
            },
        ));
    }
    Ok(true)
}

/// Retire la CASC-SVP (rubrique `ccn` de la 3090) d'une entreprise exemptee.
pub fn bareme_casc(mut bar: Pile, ent: &ChampsEntreprise) -> Resultat<Pile> {
    if ent.idcc.texte() != "3090" || !casc_exemptee(ent)? {
        return Ok(bar);
    }
    bar.rubriques_bulletin.retain(|r| r.id != "ccn");
    Ok(bar)
}

/// La representation du personnel (CCNEAC), lue sur la fiche entreprise et
/// confrontee a l'effectif : elle decide du FNAS (article III.3.1).
pub fn representation_du_personnel(ent: &EntrepriseLue) -> Resultat<String> {
    let v = ent.fiche.representation_personnel.code();
    if !v.is_empty() {
        return Ok(v.to_string());
    }
    match ent.effectif {
        // III.3.1.b : moins de onze salaries, 1,45 %.
        Some(e) if e < 11.0 => Ok("aucune".to_string()),
        Some(_) => Err(
            "Comité social et économique (CSE) : la compagnie compte au moins onze salariés, et la CCNEAC fait dépendre de son CSE \
la contribution au FNAS (article III.3.1). Le dire dans Configuration › Entreprise : pas de CSE (carence constatée), un CSE \
conventionnel (11 à 49 salariés), ou un CSE de 50 salariés et plus, adhérent au FNAS ou non. Il ne se devine pas : le CSE \
conventionnel est la règle « sauf carence »."
                .to_string(),
        ),
        // Sans effectif connu, le CSE ne se demande pas : c'est l'effectif qui manque (`contexte::Effectif::a_dire`).
        None => Err("L'effectif de la compagnie n'est pas connu : sous onze salariés, la CCNEAC fixe la contribution au FNAS \
sans CSE (article III.3.1). Le dire dans « Pour démarrer »."
            .to_string()),
    }
}

/// Ce qu'etait la pile avant la nature du contrat : ce que la caisse des Conges Spectacles en reprend
/// (`bareme_caisse_conges`).
pub struct AvantLaNature {
    rubriques: Vec<Rubrique>,
    cotisations: Vec<CotisationDsn>,
}

/// LA CAISSE DES CONGES SPECTACLES (`par_caisse_conges`), quand le contrat n'est pas ce que sa nature decrit : un
/// CDDU de douze mois ou plus n'y est pas declare (`non_affilie` : ni la ligne de la caisse, ni le 066, ni les
/// majorations d'assiette) ; un CDD de droit commun de moins de douze mois d'un artiste l'est
/// (`affilie` : la ligne et le 066 reprises du statut tel qu'il etait avant la nature -- `avant_la_nature` --, et les
/// majorations). Qui est declare : `caisse_conges::affilie` ; les textes : la section du bareme et
/// `atelier/reference/sources/caisse-conges-affiliation/LISEZ-MOI.md`. Sans section verifiee : refus.
pub fn bareme_caisse_conges(bar: Pile, avant_la_nature: Option<&AvantLaNature>, nat: &Nature, affilie: bool) -> Resultat<Pile> {
    if affilie == crate::caisse_conges::par_defaut(nat) {
        return Ok(bar);
    }
    let cle = if affilie { "affilie" } else { "non_affilie" };
    let sec = bar.par_caisse_conges.as_ref().map(|p| if affilie { p.affilie.clone() } else { p.non_affilie.clone() });
    let Some(sec) = sec.filter(|s| s.verifie == Some(true)) else {
        return Err(refus(
            format!("Le barème {} ne sait pas payer ce contrat avec les Congés Spectacles : le signaler au développeur.", annee(&bar)),
            format!(
                "Contrat de nature '{}' {} la caisse des Conges Spectacles, et le bareme {} n'a pas de section \
par_caisse_conges['{}'] marquee \"verifie\": true. Le payer comme sa nature le decrit donnerait {}. Refus.",
                nat.cle,
                if affilie { "declare a" } else { "non declare a" },
                annee(&bar),
                cle,
                if affilie {
                    "un bulletin sans cotisation a la caisse ni majoration d'assiette"
                } else {
                    "une cotisation a la caisse et des assiettes majorees que l'employeur ne doit pas"
                }
            ),
        ));
    };
    let mut out = empiler(
        bar,
        Couche {
            retirees: sec.rubriques_retirees.as_deref(),
            rubriques: None,
            ajoutees: None,
            cotisations_dsn: None,
            prevoyance_retraite_supp: None,
            sante_patronale: None,
            majoration_conges_115: sec.majoration_conges_115.map(|n| n.0),
            majoration_conges_110: sec.majoration_conges_110.map(|n| n.0),
            reduction_generale: None,
        },
    );
    let corps = sec.du_corps.clone().unwrap_or_default();
    let (d_avant, c_avant): (&[Rubrique], &[CotisationDsn]) = avant_la_nature.map_or((&[], &[]), |a| (&a.rubriques, &a.cotisations));
    for id in &corps.rubriques_bulletin {
        if rang(&out.rubriques_bulletin, id).is_some() {
            continue;
        }
        let i = rang(d_avant, id).ok_or_else(|| {
            format!("par_caisse_conges['{}'] reprend la rubrique {} du corps du bareme {}, qui ne la porte pas.", cle, id, annee(&out))
        })?;
        // A sa place : apres la rubrique qui la precede dans le corps, si elle est la ; sinon a la fin.
        let apres = d_avant[..i].iter().rev().find_map(|r| rang(&out.rubriques_bulletin, &r.id));
        insere(&mut out.rubriques_bulletin, apres, d_avant[i].clone());
    }
    out.cotisations_dsn.retain(|c| !sec.cotisations_dsn_retirees.iter().flatten().any(|r| r == code81(c)));
    for code in &corps.cotisations_dsn {
        if out.cotisations_dsn.iter().any(|c| code81(c) == code) {
            continue;
        }
        let reprises: Vec<&CotisationDsn> = c_avant.iter().filter(|c| code81(c) == code).collect();
        if reprises.is_empty() {
            return Err(refus(
                "Le barème de l'année est incomplet : le signaler au développeur.",
                format!(
                    "par_caisse_conges['{}'] reprend la cotisation DSN {} du corps du bareme {}, qui ne la porte pas.",
                    cle,
                    code,
                    annee(&out)
                ),
            ));
        }
        for c in reprises {
            // Dans l'ordre des blocs 78 : avant le premier bloc de type superieur.
            match out.cotisations_dsn.iter().position(|x| x.bloc78 > c.bloc78) {
                Some(i) => out.cotisations_dsn.insert(i, c.clone()),
                None => out.cotisations_dsn.push(c.clone()),
            }
        }
    }
    Ok(out)
}

/// Le libellé d'une convention collective au catalogue : `par_convention.<idcc>.libelle` du socle livré de l'année
/// (`donnees/baremes/<an>.json`), celui que la fiche entreprise propose au choix de l'IDCC. Le bulletin et le contrat
/// l'impriment d'ici, à la date de leur période ; une convention que le socle de l'année ne porte pas n'a pas de
/// libellé, et le refus le dit (règle 1).
pub fn libelle_convention(idcc: &str, an: i32) -> Resultat<String> {
    let Some(pile) = crate::contexte::socle_livre_lu(an)? else {
        return Err(format!("Aucun barème livré pour {} : le libellé de la convention {} ne se lit pas.", an, idcc));
    };
    match pile.par_convention.get(idcc).map(|c| c.libelle.clone()).filter(|l| !l.trim().is_empty()) {
        Some(l) => Ok(l),
        None => Err(socle::echec::a_la_cle(&format!("La convention collective « {} » n'est pas au catalogue du barème {} : le signaler au développeur.", idcc, an), "par_convention", "")),
    }
}

/// Les contributions de la convention collective, posees sur les rubriques
/// que le statut et la nature ont laissees.
pub fn bareme_convention(mut bar: Pile, ent: &EntrepriseLue, nat: Option<&Nature>) -> Resultat<Pile> {
    use crate::forme_bareme::{BaseParPopulation, TauxParPopulation};
    let Some(contributions) = bar.contributions_conventionnelles.take().filter(|l| !l.is_empty()) else {
        return Ok(bar);
    };
    let population = if mensuelle(nat) { "permanent" } else { "intermittent" };
    let permanent = population == "permanent";
    let mut representation: Option<String> = None;
    let mut rubriques = std::mem::take(&mut bar.rubriques_bulletin);
    for c in &contributions {
        // Les taux d'une paie importee se lisent sur ses pieces (`EntrepriseLue::taux_lus`) : la contribution garde sa
        // rubrique et sa base, sans taux, et la representation du personnel ne se demande pas.
        let par_representation = c.par_representation.as_ref().is_some_and(|p| !p.is_empty());
        let sans_taux = ent.taux_lus && par_representation;
        let efface = |p: Part| c.efface.iter().flatten().any(|e| *e == p);
        // La part patronale : (le taux pose, la part effacee).
        let (taux_pat, efface_pat): (Option<f64>, bool) = if sans_taux {
            (None, true)
        } else if par_representation {
            if representation.is_none() {
                representation = Some(representation_du_personnel(ent)?);
            }
            let rep = representation.clone().unwrap_or_default();
            let t = c.par_representation.as_ref().and_then(|p| p.get(&rep)).ok_or_else(|| socle::fichiers::absent(&rep))?;
            let t = match t {
                TauxParPopulation::Un(x) => *x,
                TauxParPopulation::Par { intermittent, permanent: p } => if permanent { *p } else { *intermittent },
            };
            (Some(t), false)
        } else {
            (c.taux_pat, efface(Part::TauxPat))
        };
        let base = c.base.as_ref().map(|b| match b {
            BaseParPopulation::Une(x) => x.clone(),
            BaseParPopulation::Par { intermittent, permanent: p } => if permanent { p.clone() } else { intermittent.clone() },
        });
        let i = rang(&rubriques, &c.id);
        let vaut = |t: Option<f64>| t.is_some_and(|x| x != 0.0);
        if !sans_taux && !vaut(taux_pat) && !vaut(c.taux_sal) {
            if let Some(i) = i {
                rubriques.remove(i);
            }
            continue;
        }
        if let Some(i) = i {
            let r = &mut rubriques[i];
            macro_rules! si {
                ($($ch:ident),*) => { $( if c.$ch.is_some() { r.$ch = c.$ch.clone(); } )* };
            }
            si!(famille, libelle, source, note);
            if base.is_some() {
                r.base = base;
            }
            if taux_pat.is_some() || efface_pat {
                r.taux_pat = taux_pat;
            }
            if c.taux_sal.is_some() || efface(Part::TauxSal) {
                r.taux_sal = c.taux_sal;
            }
            if c.masquer_base_et_taux.is_some() {
                r.masquer_base_et_taux = c.masquer_base_et_taux;
            }
            continue;
        }
        let j = c.apres.as_deref().and_then(|a| rang(&rubriques, a));
        let nouvelle = Rubrique {
            id: c.id.clone(),
            famille: c.famille.clone(),
            libelle: c.libelle.clone(),
            base,
            taux_pat,
            taux_sal: c.taux_sal,
            masquer_base_et_taux: c.masquer_base_et_taux,
            source: c.source.clone(),
            note: c.note.clone(),
            ..Rubrique::default()
        };
        insere(&mut rubriques, j, nouvelle);
    }
    bar.rubriques_bulletin = rubriques;
    bar.contributions_conventionnelles = Some(contributions);
    Ok(bar)
}

/// LA PILE ENTIERE, ET LE SEUL ENDROIT OU ELLE S'ECRIT : l'etablissement du
/// salarie, le statut, le taux AT/MP, la nature, la convention, la CASC, la
/// sante, la retraite en tranche 1, la categorie, la
/// prevoyance, le regime local, le versement mobilite -- dans cet ordre.
///
/// Le contrat dit s'il est declare a la caisse des Conges Spectacles (`caisse_conges::affilie`) : la caisse est un
/// etage, pose juste apres la nature (`bareme_caisse_conges`).
///
/// `ent` : la compagnie telle que la pile la lit, au jour du bareme `partage` (`Contexte::entreprise_lue`).
pub fn bareme_applique(
    partage: &Pile, ent: &EntrepriseLue, sal: &SalarieDuContrat, nat: &Nature, fin_periode: Option<&Jour>, caisse_conges: bool,
    cpf_cdd: bool, majoration_chomage: bool,
) -> Resultat<Pile> {
    let fiche = &ent.fiche;
    let mut bar = if fiche.secondaires().is_empty() {
        partage.clone()
    } else {
        surcharger(partage.clone(), fiche, &etablissement_de(fiche, sal)?)?
    };
    bar = bareme_du_statut(bar, sal.statut.exigee("statut")?.as_str())?;
    bar = bareme_atmp(bar, fiche)?;
    // Ce que la caisse reprend du statut tel qu'avant la nature : ses rubriques et ses cotisations (`bareme_caisse_conges`).
    let avant_la_nature = (caisse_conges != crate::caisse_conges::par_defaut(nat))
        .then(|| AvantLaNature { rubriques: bar.rubriques_bulletin.clone(), cotisations: bar.cotisations_dsn.clone() });
    bar = bareme_de_la_nature(bar, nat)?;
    bar = bareme_caisse_conges(bar, avant_la_nature.as_ref(), nat, caisse_conges)?;
    bar = bareme_cpf_cdd(bar, nat, cpf_cdd)?;
    bar = bareme_majoration_chomage(bar, majoration_chomage)?;
    bar = bareme_taxe_apprentissage(bar, fiche, fin_periode)?;
    bar = bareme_convention(bar, ent, Some(nat))?;
    bar = bareme_casc(bar, fiche)?;
    bar = bareme_sante(bar, fiche, sal, Some(nat), fin_periode)?;
    bar = bareme_retraite_t1(bar, fiche, sal, Some(nat))?;
    bar = bareme_cadre(bar, sal, Some(nat))?;
    bar = bareme_prevoyance(bar, fiche, sal, Some(nat), fin_periode)?;
    bar = bareme_regime_local(bar, fiche, sal)?;
    bareme_vm(bar, sal)
}

/// LA COMPAGNIE QUE LA PILE LIT : sa fiche au jour du bareme, et son effectif de l'annee (`contexte::effectif_de`), que
/// la representation du personnel confronte (CCNEAC, III.3.1 : `representation_du_personnel`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EntrepriseLue {
    pub fiche: ChampsEntreprise,
    pub effectif: Option<f64>,
    /// La pile d'une paie IMPORTEE : ses taux se lisent sur ses pieces, la pile ne fait que ranger ses bases
    /// (`engagement::importe`). Ce qui ne decide qu'un taux ne se demande pas : la representation du personnel, et
    /// l'effectif qui la remplace (`bareme_convention`). Une annee que l'ancien logiciel a payee n'a pas a dire
    /// l'effectif de l'annee d'avant.
    pub taux_lus: bool,
}

// ---------------------------------------------------- les etablissements

/// Le NIC de l'etablissement du salarie, de la fiche entreprise.
pub fn etablissement_de(fiche: &ChampsEntreprise, sal: &SalarieDuContrat) -> Resultat<String> {
    rattachement(&fiche.nics(), sal)
}

/// `nics` : le siege, puis les autres etablissements.
fn rattachement(nics: &[String], sal: &SalarieDuContrat) -> Resultat<String> {
    let nic = sal.etablissement.as_deref().unwrap_or("").trim().to_string();
    if nic.is_empty() || nic == nics[0] {
        return Ok(nics[0].clone());
    }
    if !nics[1..].contains(&nic) {
        return Err(format!(
            "{} : l'engagement le rattache a l'etablissement {}, que la \
fiche entreprise ne porte pas a cette date (Configuration › Entreprise › Avancé, « Autres établissements » : son ouverture ; connus : {}).",
            nom_ou(sal, "Ce salarie"),
            nic,
            nics.join(", ")
        ));
    }
    Ok(nic)
}

/// Le nom du salarie, ou `defaut` s'il n'en a pas.
fn nom_ou(sal: &ChampsSalarie, defaut: &str) -> String {
    match sal.nom.texte() {
        "" => defaut.to_string(),
        n => n.to_string(),
    }
}

/// L'entreprise vue depuis l'etablissement `nic`.
/// Un autre etablissement que le siege : son NIC, son SIRET (celui du SIREN et de ce NIC), et son adresse quand il la dit.
pub fn fiche_de_l_etablissement(fiche: &ChampsEntreprise, nic: &str) -> Resultat<ChampsEntreprise> {
    if nic.is_empty() || nic == fiche.nic_principal() {
        return Ok(fiche.clone());
    }
    let e = fiche.etablissement(nic).ok_or_else(|| format!("Etablissement inconnu : {}.", nic))?;
    let mut vue = fiche.clone();
    vue.nic = Some(e.nic.ecrite("nic")?.to_string());
    vue.siret = Some(format!("{}{}", fiche.siren.texte(), e.nic.texte()));
    // L'adresse de l'etablissement vient avec son complement, vide ou non : celui du siege n'est pas le sien.
    if e.adresse.remplie().is_some() {
        vue.adresse = e.adresse.clone();
        vue.complement_adresse = e.complement_adresse.clone();
        vue.complement_adresse_affichage = e.complement_adresse_affichage.clone();
    }
    if e.code_postal.remplie().is_some() {
        vue.code_postal = e.code_postal.clone();
    }
    if e.commune.remplie().is_some() {
        vue.commune = e.commune.clone();
    }
    for (case, de) in [
        (&mut vue.adresse_affichage, &e.adresse_affichage),
        (&mut vue.commune_affichage, &e.commune_affichage),
    ] {
        if de.remplie().is_some() {
            *case = de.clone();
        }
    }
    Ok(vue)
}

/// L'etablissement sous lequel un salarie se declare : la fiche entreprise vue depuis l'etablissement de rattachement
/// que dit SA fiche (`etablissement`, sinon le principal ; un NIC que la fiche entreprise ne porte pas refuse).
///
/// « Dans le cadre d'un contrat de travail, il s'agit de l'etablissement de rattachement administratif du salarie »
/// (S21.G00.11.001, cahier technique DSN 2026.1, p. 144, `atelier/reference/norme-dsn/texte/bloc-S21.G00.11.txt`). Le
/// NIC est une donnee identifiante : un signalement porte « les memes valeurs que celles portees dans la derniere DSN
/// mensuelle deposee » (§ 2.1.1, p. 18 ; § 2.5.1, p. 67, `chapitre-2-principes-de-constitution.txt`). La mensuelle
/// (une DSN par etablissement, qui refuse un salarie d'un autre, `generer`) et les signalements (arret, reprise, FCTU,
/// amorcage) le lisent donc au meme endroit, comme le bulletin (« la designation de l'etablissement dont depend le salarie »,
/// code du travail, R. 3243-1, 1°, `atelier/reference/sources/code-du-travail/xml/R3243-1.xml`) : `etablissement_de`,
/// puis `fiche_de_l_etablissement`.
pub fn entreprise_du_salarie(ent: &ChampsEntreprise, salarie: &SalarieDuContrat) -> Resultat<ChampsEntreprise> {
    fiche_de_l_etablissement(ent, &etablissement_de(ent, salarie)?)
}

/// Le taux AT/MP et le versement mobilite de
/// l'etablissement remplacent ceux du siege.
pub fn surcharger(mut bar: Pile, fiche: &ChampsEntreprise, nic: &str) -> Resultat<Pile> {
    let e = if !nic.is_empty() && nic != fiche.nic_principal() { fiche.etablissement(nic) } else { None };
    let e = match e {
        Some(e) => e,
        None => return Ok(bar),
    };
    let an = annee(&bar);
    // Le taux de l'etablissement, que `bareme_atmp` lit comme celui de la compagnie ; en Alsace-Moselle d'apres SON code postal.
    if e.taux_atmp.is_none() {
        return Err(socle::fichiers::absent("taux_atmp"));
    }
    let mut atmp = taux_atmp_d_un_etablissement(e, &an);
    let alsace = en_alsace_moselle(e.code_postal.texte());
    if let Some(t) = atmp.as_mut() {
        if alsace {
            t.alsace_moselle = Some(true);
        }
        // Le taux saisi d'une autre annee ne vaut pas pour celle-ci, et l'arrete ne le donne pas (un risque sans « TC ») :
        // `bareme_atmp` le refuse en le disant.
        if t.valeur.is_none() && taux_atmp_applique(&risques_de(&bar), t, alsace).is_none() {
            t.manque = Some(format!(
                "Le taux AT/MP {} de l'établissement « {} » manque : la CARSAT le notifie chaque année à cet établissement \
(compte AT/MP sur net-entreprises), et son risque {} n'a pas de taux collectif dans l'arrêté de l'année. Il se saisit dans \
Configuration › Entreprise › Avancé, « Autres établissements », à partir du 1er janvier {}.",
                an,
                e.libelle.remplie().filter(|l| !l.trim().is_empty()).unwrap_or(e.nic.texte()),
                t.code_risque.as_deref().unwrap_or("").trim(),
                an
            ));
        }
    }
    bar.taux_atmp = atmp;
    // « aucun », ou un taux de zero : la commune de l'etablissement n'en a pas, ou il y emploie moins de onze salaries
    // (CGCT, L. 2333-64, I, l'effectif « employe dans chacune des zones »).
    let vm = e.versement_mobilite.dite();
    let aucun = vm.is_some_and(|v| v.taux.dite().is_some_and(|t| t.decimal().is_zero()));
    if aucun || (e.versement_mobilite.is_none() && bar.versement_mobilite.as_ref().is_none_or(VersementMobiliteEcrit::est_vide)) {
        bar.versement_mobilite = None;
    } else if let Some(v) = vm {
        bar.versement_mobilite = Some(VersementMobiliteEcrit::Ecrit(crate::forme_bareme::VersementMobiliteTaux {
            taux: v.taux.dite().map(|t| socle::forme::Nombre(t.decimal())),
            code_insee: v.code_insee.dite().cloned(),
            ..Default::default()
        }));
    }
    bar.etablissement = Some(e.nic.ecrite("nic")?.to_string());
    Ok(bar)
}

// ------------------------------------------- le versement mobilite, le local

/// Le versement mobilite que la compagnie a ecrit dans sa surcouche.
#[derive(Debug, Clone, PartialEq)]
pub struct VersementMobilite {
    pub taux: Decimal,
    pub code_insee: String,
    pub autorite: String,
}

/// Le versement mobilite ecrit dans le bareme, ou `None` (rien d'ecrit, ou
/// « aucun »). Un taux sans code INSEE de cinq caracteres : refus.
pub fn versement_mobilite(bar: &Pile) -> Resultat<Option<VersementMobilite>> {
    let Some(VersementMobiliteEcrit::Ecrit(crate::forme_bareme::VersementMobiliteTaux { taux, code_insee, autorite, .. })) = &bar.versement_mobilite else {
        return Ok(None);
    };
    let insee = code_insee.as_deref().unwrap_or("").trim().to_string();
    match taux {
        Some(taux) if insee.chars().count() == 5 => {
            Ok(Some(VersementMobilite { taux: taux.0, code_insee: insee, autorite: autorite.clone().unwrap_or_default() }))
        }
        _ => Err(refus(
            "Le versement mobilité est écrit sans son taux ou sans le code de la commune de l'établissement : les lire sur le bordereau Urssaf.",
            format!(
                "Bareme {} : le versement mobilite est ecrit sans son taux ou sans \
le code INSEE (cinq caracteres) de la commune de l'etablissement. \
Les deux se lisent sur la deliberation de l'autorite \
organisatrice ou sur le bordereau Urssaf :\n    \
\"versement_mobilite\": {{\"taux\": 1.75, \"code_insee\": \"37261\", \
\"autorite\": \"...\", \"source\": \"...\"}}",
                annee(bar)
            ),
        )),
    }
}

/// L'organisme du premier 075 de la DSN : l'Urssaf, que le bareme dit.
fn org_urssaf(bar: &Pile) -> Resultat<String> {
    bar.cotisations_dsn
        .iter()
        .find(|c| code81(c) == "075" && dit(&c.org).is_some())
        .and_then(|c| c.org.clone())
        .ok_or_else(|| socle::fichiers::absent("cotisations_dsn[075].org"))
}

/// La ligne du versement mobilite, au bulletin et dans la DSN.
pub fn bareme_vm(mut bar: Pile, sal: &SalarieDuContrat) -> Resultat<Pile> {
    let vm = match versement_mobilite(&bar)? {
        Some(v) => v,
        None => return Ok(bar),
    };
    let artiste = *sal.statut.exigee("statut")? == Statut::Artiste;
    let facteur = if artiste { Decimal::new(70, 2) } else { Decimal::ONE };
    let taux = flt(rond(mul(vm.taux, facteur), 3));
    let org = org_urssaf(&bar)?;
    // L'ASSIETTE MAJOREE (CSS L. 243-1-3, 1° ; D. 243-0-2, 1° : 11,5 %) : l'employeur affilie a une caisse de conges
    // acquitte le versement mobilite du sur les indemnites de conges par une majoration de son assiette (lettre
    // circulaire Acoss 2013-019, § 1.3) -- celle du FNAL, `majoration_conges_115`, qui vaut 1 pour un permanent.
    // Le 78.004 reste le brut. Textes : atelier/reference/sources/majoration-caisses-conges/LISEZ-MOI.md.
    bar.rubriques_bulletin.push(Rubrique {
        id: "versement_mobilite".into(),
        famille: Some("AUTRES CONTRIBUTIONS DUES PAR L'EMPLOYEUR".into()),
        libelle: Some("Versement mobilité".into()),
        base: Some("brut_x115".into()),
        taux_pat: Some(taux),
        taux_sal: None,
        ..Rubrique::default()
    });
    bar.cotisations_dsn.push(CotisationDsn {
        bloc78: "03".into(),
        code81: Some("081".into()),
        org: Some(org),
        base: Some("brut_x115".into()),
        taux: Some(crate::forme_bareme::TauxDate(taux)),
        base78: "brut".into(),
        code_insee: Some(vm.code_insee),
        ..CotisationDsn::default()
    });
    Ok(bar)
}

/// La cotisation salariale maladie du regime local d'Alsace-Moselle (CSS
/// L242-13), au bulletin et dans la DSN (bloc 81, code 115).
pub fn bareme_regime_local(mut bar: Pile, ent: &ChampsEntreprise, sal: &SalarieDuContrat) -> Resultat<Pile> {
    if !alsace_moselle(ent, sal) {
        return Ok(bar);
    }
    let Some(rl) = bar.regime_local_alsace_moselle.clone() else {
        return Err(refus(
            "Le barème de l'année ne porte pas le taux du régime local d'Alsace-Moselle : le signaler au développeur.",
            format!(
                "Bareme {} : le salarie travaille en Alsace-Moselle, et le bareme \
ne porte pas le taux du regime local (`regime_local_alsace_moselle`).",
                annee(&bar)
            ),
        ));
    };
    let org = org_urssaf(&bar)?;
    bar.rubriques_bulletin.push(Rubrique {
        id: "regime_local".into(),
        famille: Some("SANTÉ".into()),
        libelle: Some("Régime local Alsace-Moselle — maladie".into()),
        base: Some("brut".into()),
        taux_pat: None,
        taux_sal: Some(rl.taux_sal),
        ..Rubrique::default()
    });
    bar.cotisations_dsn.push(CotisationDsn {
        bloc78: "03".into(),
        code81: Some(if rl.code81.is_empty() { "115".into() } else { rl.code81.clone() }),
        org: Some(org),
        base: Some("brut".into()),
        taux: Some(crate::forme_bareme::TauxDate(rl.taux_sal)),
        base78: "brut".into(),
        ..CotisationDsn::default()
    });
    Ok(bar)
}

// ------------------------------------------- la retraite en tranche 1

/// « Scène Ensemble » -> « SCENE ENSEMBLE ».
fn sigle(v: &str) -> String {
    let s: String = sans_accent(v).chars().filter(char::is_ascii).collect();
    s.to_uppercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// La couche de l'organisation patronale `nom` (sigle ou alias), ou `None`.
pub fn couche_organisation<'a>(bar: &'a Pile, nom: &str) -> Option<&'a crate::forme_bareme::Organisation> {
    let cle = sigle(nom);
    if cle.is_empty() {
        return None;
    }
    bar.par_organisation_patronale.iter().find(|(k, c)| cle == sigle(k) || c.alias.iter().flatten().any(|a| cle == sigle(a))).map(|(_, c)| c)
}

/// Une fraction `num/den` irreductible.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fraction {
    pub num: i128,
    pub den: i128,
}

impl Fraction {
    fn nouvelle(num: i128, den: i128) -> Fraction {
        fn pgcd(a: i128, b: i128) -> i128 {
            if b == 0 {
                a.abs()
            } else {
                pgcd(b, a % b)
            }
        }
        let g = pgcd(num, den).max(1);
        Fraction { num: num / g, den: den / g }
    }

    /// `Fraction("2/3")` ou `Fraction("1")`.
    fn lire(s: &str) -> Resultat<Fraction> {
        let s = s.trim();
        let faux = || format!("fraction illisible : {}", socle::fichiers::repr(s));
        let (n, dn) = match s.split_once('/') {
            Some((a, b)) => (a.trim().parse::<i128>().map_err(|_| faux())?, b.trim().parse::<i128>().map_err(|_| faux())?),
            None => (s.parse::<i128>().map_err(|_| faux())?, 1),
        };
        if dn == 0 {
            return Err(format!("fraction de denominateur nul : {}", socle::fichiers::repr(s)));
        }
        Ok(Fraction::nouvelle(n, dn))
    }
}

/// (part patronale, part salariale) de la tranche 1, en fractions, pour la
/// convention de l'entreprise et cette population.
pub fn repartition_t1(bar: &Pile, ent: &ChampsEntreprise, population: &str) -> Resultat<(Fraction, Fraction)> {
    let rep = bar.retraite_t1_repartition.as_ref();
    let premiere = ent.premiere_embauche.dite().map(Jour::iso).unwrap_or_default();
    let avant_1999 = rep.map(|r| r.avant_1999.as_str()).filter(|a| !a.is_empty());
    let texte_rep: String = match avant_1999 {
        Some(a) if !premiere.is_empty() && premiere.as_str() < "1999-01-01" => a.to_string(),
        _ => {
            let idcc = ent.idcc.texte().to_string();
            let branche = rep.and_then(|r| r.par_idcc.get(&idcc));
            let t = branche.and_then(|b| match population {
                "permanent" => b.permanent.clone(),
                "technicien" => b.technicien.clone(),
                "artiste" => b.artiste.clone(),
                _ => None,
            });
            match t.filter(|t| !t.is_empty()) {
                Some(t) => t,
                None => {
                    return Err(refus(
                        "Le barème de l'année ne porte pas la répartition de la retraite complémentaire de cette convention : le signaler au développeur.",
                        format!(
                            "Bareme {} : la repartition de la retraite complementaire en \
tranche 1 n'est pas relevee pour la convention {} ({}). Elle \
se lit dans le tableau n 3 de la fiche Audiens << Taux et \
repartitions >>, puis s'ecrit dans `retraite_t1_repartition` \
du socle.",
                            annee(bar),
                            if idcc.is_empty() { "(aucune)" } else { &idcc },
                            population
                        ),
                    ))
                }
            }
        }
    };
    // « 60/40 » en pourcentages, « 2/3-1/3 » en fractions.
    if texte_rep.contains('-') {
        let mut p = texte_rep.split('-');
        match (p.next(), p.next(), p.next()) {
            (Some(a), Some(b), None) => Ok((Fraction::lire(a)?, Fraction::lire(b)?)),
            _ => Err(format!("repartition de la tranche 1 illisible : {}", socle::fichiers::repr(&texte_rep))),
        }
    } else {
        let (a, b) = texte_rep
            .split_once('/')
            .ok_or_else(|| format!("repartition de la tranche 1 illisible : {}", socle::fichiers::repr(&texte_rep)))?;
        let ent = |s: &str| {
            s.trim().parse::<i128>().map_err(|_| format!("nombre entier illisible : {}", socle::fichiers::repr(s)))
        };
        Ok((Fraction::nouvelle(ent(a)?, 100), Fraction::nouvelle(ent(b)?, 100)))
    }
}

/// Un taux global partage : la part salariale arrondie au centieme, la demie
/// vers le bas, la patronale en est le reste.
pub fn parts_t1(total: Decimal, sal: Fraction) -> (Decimal, Decimal) {
    let part_sal = div(mul(total, Decimal::from_i128_with_scale(sal.num, 0)), Decimal::from_i128_with_scale(sal.den, 0))
        .round_dp_with_strategy(2, RoundingStrategy::MidpointTowardZero);
    (total - part_sal, part_sal)
}

/// Remplace la retraite de la ligne `retraite_t1` (CEG inchangee) et, dans la
/// DSN, le 131 (regime unifie Agirc-Arrco, cahier technique 2026.1 p. 302) qui
/// porte un taux sur la meme assiette.
fn poser_t1(mut bar: Pile, retraite_pat: Decimal, retraite_sal: Decimal) -> Resultat<Pile> {
    let ceg = bar.retraite_t1_repartition.as_ref().map(|r| r.ceg.clone()).ok_or_else(|| socle::fichiers::absent("retraite_t1_repartition"))?;
    let ceg_pat = dec_f64(ceg.pat)?;
    let ceg_sal = dec_f64(ceg.sal)?;
    let pat = retraite_pat + ceg_pat;
    let sal = retraite_sal + ceg_sal;
    let mut base = None;
    for r in bar.rubriques_bulletin.iter_mut().filter(|r| r.id == "retraite_t1") {
        r.taux_pat = Some(flt(pat));
        r.taux_sal = Some(flt(sal));
        if r.composants_pat.as_deref().is_some_and(|l| !l.is_empty()) {
            r.composants_pat = Some(vec![flt(retraite_pat), flt(ceg_pat)]);
            r.composants_sal = Some(vec![flt(retraite_sal), flt(ceg_sal)]);
        }
        base = r.base.clone();
    }
    for c in &mut bar.cotisations_dsn {
        if code81(c) == "131" && c.base == base && c.taux.is_some() && c.lignes.as_deref().is_none_or(<[String]>::is_empty) {
            c.taux = Some(crate::forme_bareme::TauxDate(flt(pat + sal)));
            if c.composants.as_deref().is_some_and(|l| !l.is_empty()) {
                c.composants = Some(vec![flt(retraite_pat), flt(ceg_pat), flt(retraite_sal), flt(ceg_sal)]);
            }
        }
    }
    Ok(bar)
}

/// La retraite complementaire en tranche 1, quand elle n'est pas celle du
/// corps du bareme : la repartition de la branche pour un permanent, le taux
/// d'une organisation patronale pour un intermittent non cadre.
pub fn bareme_retraite_t1(bar: Pile, ent: &ChampsEntreprise, sal: &SalarieDuContrat, nat: Option<&Nature>) -> Resultat<Pile> {
    let Some(rep) = bar.retraite_t1_repartition.clone() else { return Ok(bar) };
    if nat.is_some_and(|n| n.mensuelle()) {
        match bar.rubrique("retraite_t1") {
            Some(l) if l.base.as_deref().unwrap_or("").starts_with("t1_mensuelle") => {}
            _ => return Ok(bar),
        }
        if ent.idcc.texte().is_empty() && !ent.premiere_embauche.est_dite() {
            return Ok(bar);
        }
        let (_, part_sal) = repartition_t1(&bar, ent, "permanent")?;
        let (p, s) = parts_t1(dec_f64(rep.taux_droit_commun)?, part_sal);
        return poser_t1(bar, p, s);
    }
    let couche = match couche_organisation(&bar, ent.organisation_patronale.texte()) {
        Some(c) if c.applicable => c.clone(),
        _ => return Ok(bar),
    };
    let statut = sal.statut.exigee("statut")?.as_str().trim().to_lowercase();
    let taux = couche.retraite_t1.as_ref().and_then(|t| match statut.as_str() {
        "artiste" => t.artiste,
        "technicien" => t.technicien,
        _ => None,
    });
    let Some(taux) = taux else { return Ok(bar) };
    if let Some(avant) = dit(&couche.adhesion_avant) {
        let depuis = ent.organisation_patronale_depuis.dite().map(Jour::iso).unwrap_or_default();
        if depuis.is_empty() {
            return Err(format!(
                "Fiche Entreprise : l'adhesion a {} ne donne son taux de \
retraite ({} %) qu'aux adherents d'avant le {}. Renseigner \
« Adhérente depuis le » -- le logiciel ne le devine pas.",
                couche.libelle,
                ecrit_f(taux),
                Jour::lire(avant)?.jj_mm_aaaa()
            ));
        }
        if depuis.as_str() >= avant {
            return Ok(bar);
        }
    }
    if est_cadre(sal) {
        return Err(refus(
            "La retraite complémentaire d'un cadre intermittent ne se lit pas pour cette adhésion : la demander à Audiens.",
            format!(
                "{}, cadre : l'adhesion de la compagnie a {} porte la retraite \
complementaire des {}s intermittents a {} % en tranche 1, et la \
source (Audiens, tableau n 2) ne dit pas si elle vaut pour un \
cadre, dont le taux n'est deja pas celui d'un non cadre. A \
demander a Audiens, puis a ecrire dans la couche de \
`par_organisation_patronale` du socle.",
                nom_ou(sal, "Ce salarie"),
                couche.libelle,
                statut,
                ecrit_f(taux)
            ),
        ));
    }
    let (_, part_sal) = repartition_t1(&bar, ent, &statut)?;
    let (p, s) = parts_t1(dec_f64(taux)?, part_sal);
    poser_t1(bar, p, s)
}

/// Les cotisations DSN qu'une section de categorie AJOUTE (`cotisations_ajoutees`) : l'Apec d'un cadre, qui se declare
/// a part depuis la norme 2026.1 (« 132 - Cotisation Apec », a cote de « 131 - Cotisation regime unifie Agirc-Arrco »,
/// cahier technique 2026.1 p. 302 et 305). Chacune se place juste apres la derniere cotisation de code `apres` sur la
/// meme base 78 : elle partage son bloc 78 (`dsn::bases_et_cotisations` ouvre un bloc par base). Sans elle, refus : une
/// cotisation posee ailleurs que sous la base qu'elle suit changerait les blocs 78 sans que rien ne le dise.
fn ajouter_cotisations(cots: &mut Vec<CotisationDsn>, ajouts: &[CotisationDsn], nom: &str, an: &str) -> Resultat<()> {
    for a in ajouts {
        let mut x = a.clone();
        let apres = x.apres.take().ok_or_else(|| {
            socle::echec::a_la_cle(
                "Le barème de l'année est incomplet : le signaler au développeur.",
                &format!("par_categorie.{}.cotisations_ajoutees.apres", nom),
                a.code81.as_deref().unwrap_or(""),
            )
        })?;
        let i = cots.iter().rposition(|c| code81(c) == apres && c.base78 == a.base78).ok_or_else(|| {
            format!(
                "Bareme {} : par_categorie['{}'].cotisations_ajoutees ajoute la cotisation {} apres une cotisation {} sur \
la base {}, que la pile de bareme de ce contrat ne declare pas. Elle ne se place pas au hasard.",
                an,
                nom,
                a.code81.as_deref().unwrap_or(""),
                apres,
                a.base78
            )
        })?;
        cots.insert(i + 1, x);
    }
    Ok(())
}

/// Le bareme du statut, avec la surcouche cadre si la fiche le dit : la
/// section `par_categorie.cadre` (un intermittent) ou `cadre_permanent`.
pub fn bareme_cadre(mut bar: Pile, sal: &SalarieDuContrat, nat: Option<&Nature>) -> Resultat<Pile> {
    if !est_cadre(sal) {
        return Ok(bar);
    }
    let mensuelle = mensuelle(nat);
    let nom = if mensuelle { "cadre_permanent" } else { "cadre" };
    let Some(mut sec) = bar.par_categorie.get(nom).filter(|s| s.verifie == Some(true)).cloned() else {
        return Err(refus(
            "Le barème de l'année ne sait pas payer un cadre : le signaler au développeur.",
            format!(
                "Cadre (statut categoriel 01){} : le bareme {} n'a pas de section \
par_categorie['{}'] verifiee. Prevoyance, retraite et Apec d'un \
cadre ne sont pas celles d'un non-cadre{}.",
                if mensuelle { " en contrat permanent" } else { "" },
                annee(&bar),
                nom,
                if mensuelle {
                    ", et celles d'un cadre permanent ne sont pas celles d'un cadre \
intermittent : ses tranches sont mensuelles"
                } else {
                    ""
                }
            ),
        ));
    };
    // Un cadre d'un statut donne, quand la convention le distingue.
    let st = sal.statut.exigee("statut")?.as_str().to_string();
    if let Some(sous) = sec.par_statut.as_ref().and_then(|p| p.get(&st)).cloned() {
        for (rid, champs) in sous.rubriques.iter().flatten() {
            par_dessus(sec.rubriques.get_or_insert_with(BTreeMap::new).entry(rid.clone()).or_default(), champs);
        }
        if let Some(l) = sous.rubriques_ajoutees {
            sec.rubriques_ajoutees.get_or_insert_with(Vec::new).extend(l);
        }
        if let Some(l) = sous.cotisations_ajoutees {
            sec.cotisations_ajoutees.get_or_insert_with(Vec::new).extend(l);
        }
        for (cle, champs) in sous.cotisations_dsn.iter().flatten() {
            cotisation_par_dessus(sec.cotisations_dsn.get_or_insert_with(BTreeMap::new).entry(cle.clone()).or_default(), champs);
        }
    }
    for r in &mut bar.rubriques_bulletin {
        if let Some(m) = sec.rubriques.as_ref().and_then(|mods| mods.get(&r.id)) {
            modifier(r, m);
        }
    }
    // Les ajouts se placent « apres » une rubrique -- existante ou ajoutee
    // juste avant --, dans l'ordre du bareme.
    for a in sec.rubriques_ajoutees.iter().flatten() {
        let mut x = a.clone();
        let apres = x.apres.take();
        let i = apres.and_then(|p| rang(&bar.rubriques_bulletin, &p)).unwrap_or(bar.rubriques_bulletin.len().wrapping_sub(1));
        bar.rubriques_bulletin.insert(i.wrapping_add(1), x);
    }
    if sec.cotisation_prevoyance_retraite_supp.is_some() {
        bar.cotisation_prevoyance_retraite_supp = sec.cotisation_prevoyance_retraite_supp.clone();
    }
    for c in &mut bar.cotisations_dsn {
        let Some(code) = c.code81.clone() else { continue };
        let cle = match dit(&c.affectation) {
            Some(a) => format!("{}:{}", code, a),
            None => code,
        };
        if let Some(m) = sec.cotisations_dsn.as_ref().and_then(|mods| mods.get(&cle)) {
            modifier_cotisation(c, m);
        }
    }
    let an = annee(&bar);
    ajouter_cotisations(&mut bar.cotisations_dsn, sec.cotisations_ajoutees.as_deref().unwrap_or_default(), nom, &an)?;
    Ok(bar)
}

// ------------------------------------------------ la prevoyance de branche

pub mod prevoyance;

/// L'affiliation d'un permanent a la prevoyance de sa convention collective, un mois donne (`affiliation_prevoyance`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AffiliationPrevoyance {
    /// Pas affilie : la convention n'a pas de regime pour lui (non cadre sans regime de branche), ou l'anciennete qui
    /// l'ouvre n'est pas atteinte -- sa cotisation est nulle (`prevoyance::cotisation`, `due` faux).
    NonAffilie,
    /// Affilie des l'embauche.
    DesLEmbauche,
    /// Affilie depuis ce jour : l'anciennete dans la branche que la fiche date (`prevoyance_branche_depuis`) l'a ouverte.
    Le(Jour),
    /// Affilie, au plus tard depuis ce jour : la fiche ne date pas l'entree dans la branche, seulement l'anciennete dans
    /// l'entreprise -- assez pour la cotisation, pas pour dire quand l'affiliation a commence.
    AuPlusTardLe(Jour),
}

/// L'affiliation d'un permanent a la prevoyance de sa convention au `fin` de la periode, selon la regle de sa
/// cotisation : le meme regime, la meme anciennete (`bareme_prevoyance`).
pub fn affiliation_prevoyance(ent: &ChampsEntreprise, sal: &SalarieDuContrat, fin: &Jour) -> Resultat<AffiliationPrevoyance> {
    prevoyance::affiliation(sal, est_cadre(sal), fin, ent)
}

/// La somme des parts d'une base `a` a chacun des taux `taux` (en pourcent), chacune arrondie au centime : une ligne
/// dont la part se compose de plusieurs taux.
pub fn somme_pct(a: Decimal, taux: &[f64]) -> Resultat<Decimal> {
    let mut m = Decimal::new(0, 2);
    for t in taux {
        m += socle::num::pct(a, socle::num::dec_f64(*t)?);
    }
    Ok(m)
}

// ------------------------------------------------------------ les risques AT/MP de l'arrete de l'annee



/// UN RISQUE AT/MP QUE LE SOCLE DE L'ANNEE CONNAIT (`risques_atmp.codes`), tel que l'annexe 1 de l'arrete de tarification
/// de l'annee l'ecrit (`atelier/reference/sources/tarification-atmp/`, transcription des annexes l. 497-561).
#[derive(Debug, Clone, PartialEq)]
pub struct RisqueAtmp {
    /// La forme de la DSN (S21.G00.40.040), sans point : `923AD`.
    pub code: String,
    /// La forme de l'arrete, `92.3AD` ; rien pour 999ZZ.
    pub code_arrete: Option<String>,
    pub libelle: String,
    pub precisions: Option<String>,
    /// Le taux net collectif de l'annexe 1, en pourcentage ; rien pour 999ZZ.
    pub taux: Option<Decimal>,
    /// « TC » : le taux reste collectif quel que soit l'effectif (D. 242-6-14, I ; art. 5 de l'arrete).
    pub tc: bool,
    /// Le groupe et le taux de l'annexe 3, en Alsace-Moselle (art. 4 de l'arrete ; D. 242-35, non releve).
    pub alsace_moselle: Option<(i64, Decimal)>,
}

/// Le code sans notification de la CARSAT (S21.G00.40.040, `norme-dsn/texte/bloc-S21.G00.40.txt` l. 1236-1252) : la DSN
/// interdit alors le taux (SIG-11, l. 1311-1313).
pub const SANS_NOTIFICATION: &str = "999ZZ";

/// Le risque que le socle pose a la creation d'une compagnie (`risques_atmp.par_defaut`), s'il le connait.
pub fn risque_par_defaut(socle: &Pile) -> Option<RisqueAtmp> {
    let code = socle.risques_atmp.as_ref()?.par_defaut.as_deref()?;
    risques_de(socle).into_iter().find(|r| r.code == code)
}

/// LE TAUX QUE L'ARRETE FIXE pour ce risque : le taux net collectif de l'annexe 1 quand il s'applique, c'est-a-dire pour
/// un risque « TC » quel que soit l'effectif (D. 242-6-14, I), et pour un autre sous 20 salaries (D. 242-6-2, 1°). `moins_de_20`
/// : l'effectif dit-il moins de 20 salaries (`None` : il ne le dit pas) ? Rien quand le taux est celui que la CARSAT
/// notifie, ou qu'il n'y en a pas (999ZZ). Rien en Alsace-Moselle (`alsace_moselle`) : l'annexe 3 y fixe d'autres taux
/// collectifs, sans lettres TC, en application de D. 242-35, qui n'est pas releve : ce qu'il dit de l'effectif n'est pas
/// etabli, le taux se saisit.
pub fn taux_de_l_arrete(r: &RisqueAtmp, moins_de_20: Option<bool>, alsace_moselle: bool) -> Option<Decimal> {
    if alsace_moselle {
        None
    } else if r.tc || moins_de_20 == Some(true) {
        r.taux
    } else {
        None
    }
}

/// LE TAUX AT/MP QUE LA PAIE APPLIQUE, celui de la compagnie ou celui d'un de ses etablissements (`taux_atmp` :
/// `valeur`, `code_risque`), pour l'annee du bareme `bar` : la valeur dite, sinon, pour un risque « TC » de l'annexe 1 de
/// l'arrete de l'annee (`risques_atmp` du socle), le taux collectif qu'il fixe, quel que soit l'effectif (D. 242-6-14, I ;
/// art. 5 de l'arrete, `atelier/reference/sources/tarification-atmp/`). Le taux est determine par etablissement, le
/// classement suivant l'activite qui y est exercee (D. 242-6-1, `code-de-la-securite-sociale/xml/D242-6-1.xml` ; arrete du
/// 17/10/1995, art. 1, I, 1°). Rien : le taux est celui que la CARSAT notifie (un risque sans TC, 999ZZ, l'Alsace-Moselle,
/// `taux_de_l_arrete`).
pub fn taux_atmp_applique(risques: &[RisqueAtmp], taux_atmp: &TauxAtmp, alsace_moselle: bool) -> Option<Decimal> {
    if let Some(v) = taux_atmp.valeur {
        return Some(v);
    }
    if taux_atmp.manque.is_some() {
        return None;
    }
    let code = taux_atmp.code_risque.as_deref().unwrap_or("").trim();
    let r = risques.iter().find(|r| r.code == code)?;
    // Un etablissement nouvellement cree a le taux collectif, quel que soit l'effectif (`taux_atmp_d_un_etablissement`).
    let collectif = if taux_atmp.etablissement_neuf == Some(true) { Some(true) } else { None };
    taux_de_l_arrete(r, collectif, alsace_moselle)
}

/// Les risques que la pile `bar` connait (`risques_atmp` de son socle).
pub fn risques_de(bar: &Pile) -> Vec<RisqueAtmp> {
    let deux = |t: f64| Decimal::try_from(t).ok().map(|t| t.round_dp(2));
    let texte = |t: &Option<String>| dit(t).map(|s| s.trim()).filter(|s| !s.is_empty()).map(String::from);
    bar.risques_atmp
        .iter()
        .flat_map(|r| &r.codes)
        .filter_map(|r| {
            Some(RisqueAtmp {
                code: Some(r.code.trim()).filter(|c| !c.is_empty())?.to_string(),
                code_arrete: texte(&r.code_arrete),
                libelle: texte(&r.libelle).unwrap_or_default(),
                precisions: texte(&r.precisions),
                taux: r.taux.and_then(deux),
                tc: r.tc,
                alsace_moselle: r.alsace_moselle.as_ref().and_then(|a| Some((a.groupe, deux(a.taux)?))),
            })
        })
        .collect()
}

/// La derniere annee de cotisations a laquelle s'applique D. 242-6-17 dans sa version LEGIARTI000022446511 : la version
/// issue du decret n° 2026-951 s'applique aux cotisations dues a compter de 2027 (sa note,
/// `code-de-la-securite-sociale/xml/D242-6-17.xml`).
const DERNIERE_ANNEE_D242_6_17_DE_2012: i32 = 2026;

/// LE CODE ET LE TAUX D'UN AUTRE ETABLISSEMENT (`e`, une ligne de `etablissements`) POUR L'ANNEE `an`.
///
/// Le taux saisi porte l'annee pour laquelle la CARSAT l'a notifie (`annee`) et ne vaut que pour elle, comme celui de la
/// compagnie se dit dans la surcouche de chaque annee : le taux est determine par etablissement (D. 242-6-1) et « le taux
/// net notifie » l'est d'une annee sur l'autre (D. 242-6-15, `code-de-la-securite-sociale/xml-anterieur/D242-6-15.xml`, la
/// version applicable en 2026). Une autre annee garde le code, sans taux.
///
/// Sans taux saisi, « les taux nets collectifs sont applicables aux etablissements nouvellement crees durant l'annee de
/// leur creation et les deux annees civiles suivantes, quel que soit leur effectif ou celui de l'entreprise dont ils
/// relevent » (D. 242-6-17, version applicable aux cotisations jusqu'a 2026,
/// `code-de-la-securite-sociale/xml-anterieur/D242-6-17.xml`) : l'annee de son ouverture et les deux suivantes, le taux
/// de l'arrete de l'annee pour son risque, TC ou non (`etablissement_neuf`, lu par `taux_atmp_applique`). Ce que cet
/// article excepte (un etablissement issu d'un precedent, le taux unique d'une entreprise qui l'a choisi) se dit par le taux
/// notifie, qui se saisit. A compter de 2027, la version du decret n° 2026-951 ne se lit pas ici : un risque sans TC
/// demande son taux.
///
/// Le taux s'ecrit dans la pile (`forme_bareme::Pile::taux_atmp`) ; `None` quand la fiche ne dit pas le taux de
/// l'etablissement.
pub fn taux_atmp_d_un_etablissement(e: &Etablissement, an: &str) -> Option<TauxAtmp> {
    let dit = e.taux_atmp.dite()?;
    let annee: Option<i32> = an.trim().parse().ok();
    let ouvert: Option<i32> = e.ouverture.dite().map(|j| j.an);
    let d_une_autre_annee = dit.annee.dite().is_some_and(|a| *a != 0 && a.to_string() != an.trim());
    let mut t = TauxAtmp {
        code_risque: dit.code_risque.clone(),
        valeur: if d_une_autre_annee { None } else { dit.valeur.dite().map(|m| m.decimal()) },
        ..TauxAtmp::default()
    };
    let neuf = matches!((annee, ouvert), (Some(a), Some(y)) if a <= DERNIERE_ANNEE_D242_6_17_DE_2012 && y <= a && a <= y + 2);
    if neuf && t.valeur.is_none() {
        t.etablissement_neuf = Some(true);
    }
    Some(t)
}

/// L'etablissement est-il dans le Haut-Rhin, le Bas-Rhin ou la Moselle (art. 4 de l'arrete) ? Lu dans les deux premiers
/// chiffres de son code postal (68, 67, 57) : une inference, le code postal suivant le numero du departement dans ces trois
/// departements.
pub fn en_alsace_moselle(code_postal: &str) -> bool {
    matches!(code_postal.trim().get(..2), Some("57" | "67" | "68"))
}

#[cfg(test)]
mod tests_risques_atmp {
    use super::*;

    /// 92.3AD est « TC » a 1,50 % en 2026 (`tarification-atmp/legifrance-arrete-tarification-atmp-2026-complet-20261029.txt`,
    /// annexe 1, l. 496-498) : son taux est celui de l'arrete quel que soit l'effectif
    /// (D. 242-6-14, I) ; 92.1CC, sans TC (l. 489-493), ne l'a que sous 20 salaries (D. 242-6-2, 1°) ; en Alsace-Moselle,
    /// l'annexe 3 vaut et le taux se saisit (art. 4).
    #[test]
    fn le_taux_d_un_risque_tc_est_celui_de_l_arrete_quel_que_soit_l_effectif() {
        let socle = crate::contexte::socle_livre_lu(2026).unwrap().unwrap();
        let r = risque_par_defaut(&socle).unwrap();
        assert_eq!((r.code.as_str(), r.code_arrete.as_deref(), r.tc), ("923AD", Some("92.3AD"), true));
        assert_eq!(taux_de_l_arrete(&r, Some(false), false), Some(Decimal::new(150, 2)));
        assert_eq!(taux_de_l_arrete(&r, None, false), Some(Decimal::new(150, 2)));
        assert_eq!(taux_de_l_arrete(&r, Some(true), true), None);
        assert_eq!(r.alsace_moselle, Some((3, Decimal::new(334, 2))));
        let gestion = risques_de(&socle).into_iter().find(|r| r.code == "921CC").unwrap();
        assert_eq!(taux_de_l_arrete(&gestion, Some(true), false), Some(Decimal::new(107, 2)));
        assert_eq!(taux_de_l_arrete(&gestion, Some(false), false), None);
        assert_eq!(taux_de_l_arrete(&gestion, None, false), None);
        let sans = risques_de(&socle).into_iter().find(|r| r.code == SANS_NOTIFICATION).unwrap();
        assert_eq!(taux_de_l_arrete(&sans, Some(true), false), None);
    }

    /// Le taux est determine par etablissement (D. 242-6-1, `code-de-la-securite-sociale/xml/D242-6-1.xml`) : un autre
    /// etablissement classe en 92.3AD, « TC », a le taux collectif de l'arrete de l'annee sans le saisir (1,50 % en 2026,
    /// D. 242-6-14), lu par `bareme_atmp` comme celui de la compagnie et ecrit pour la DSN ; un taux notifie saisi vaut ;
    /// en Alsace-Moselle, le taux se saisit (art. 4 de l'arrete).
    #[test]
    fn le_taux_d_un_etablissement_en_risque_tc_est_celui_de_l_arrete() {
        let socle = crate::contexte::socle_livre_lu(2026).unwrap().unwrap();
        let bar = Pile { annee: 2026, risques_atmp: socle.risques_atmp.clone(), ..Pile::default() };
        let fiche: ChampsEntreprise = crate::fiche::lire_champs(&serde_json::json!({"nic": "00017", "etablissements": [
            {"nic": "00025", "code_postal": "44000", "taux_atmp": {"code_risque": "923AD"}},
            {"nic": "00033", "code_postal": "67000", "taux_atmp": {"code_risque": "923AD"}},
            {"nic": "00041", "code_postal": "44000", "taux_atmp": {"code_risque": "921CC", "valeur": 2.4}}]}), "fiche").unwrap();
        let taux = |nic: &str| {
            let b = taux_atmp_resolu(surcharger(bar.clone(), &fiche, nic).unwrap(), &fiche).0;
            b.taux_atmp.and_then(|t| t.valeur).map(flt)
        };
        assert_eq!(taux("00025"), Some(1.5));
        assert_eq!(taux("00033"), None);
        assert_eq!(taux("00041"), Some(2.4));
    }

    /// « Les taux nets collectifs sont applicables aux etablissements nouvellement crees durant l'annee de leur creation et
    /// les deux annees civiles suivantes, quel que soit leur effectif » (D. 242-6-17, version applicable aux cotisations de
    /// 2026, `code-de-la-securite-sociale/xml-anterieur/D242-6-17.xml`) : un etablissement ouvert en 2024 ou apres, classe
    /// en 92.1CC (sans TC), a en 2026 le taux collectif de l'arrete (1,07 %, `legifrance-arrete-tarification-atmp-2026-
    /// complet-20261029.txt`, annexe 1) ; ouvert en 2023, il demande son taux notifie. Le taux saisi pour une annee
    /// (D. 242-6-15) ne vaut pas pour la suivante.
    #[test]
    fn un_etablissement_neuf_a_le_taux_collectif_trois_annees() {
        let socle = crate::contexte::socle_livre_lu(2026).unwrap().unwrap();
        let bar = Pile { annee: 2026, risques_atmp: socle.risques_atmp.clone(), ..Pile::default() };
        let fiche: ChampsEntreprise = crate::fiche::lire_champs(&serde_json::json!({"nic": "00017", "etablissements": [
            {"nic": "00025", "code_postal": "44000", "ouverture": "2024-06-01", "taux_atmp": {"code_risque": "921CC"}},
            {"nic": "00033", "code_postal": "38000", "ouverture": "2023-12-01", "taux_atmp": {"code_risque": "921CC"}},
            {"nic": "00041", "code_postal": "38000", "ouverture": "2023-12-01", "taux_atmp": {"code_risque": "921CC", "valeur": 2.4, "annee": 2025}}]}), "fiche").unwrap();
        let taux = |nic: &str| {
            let b = taux_atmp_resolu(surcharger(bar.clone(), &fiche, nic).unwrap(), &fiche).0;
            b.taux_atmp.and_then(|t| t.valeur).map(flt)
        };
        assert_eq!(taux("00025"), Some(1.07));
        assert_eq!(taux("00033"), None);
        assert_eq!(taux("00041"), None);
    }

    /// L'annee que la compagnie n'a pas dite : 92.3AD, « TC », prend le taux de l'arrete de l'annee (1,65 % en 2025, annexe 1
    /// de `legifrance-arrete-tarification-atmp-2025-complet-20261029.txt` ; 1,50 % en 2026), quel que soit l'effectif
    /// (D. 242-6-14, I). Un taux notifie propre a la compagnie, un risque sans TC (92.1CC, D. 242-6-2) ou une annee dont
    /// l'arrete n'est pas depose demandent le taux notifie de l'annee.
    #[test]
    fn le_taux_collectif_d_un_risque_tc_se_lit_dans_l_arrete_de_chaque_annee() {
        let lire = |an: i32| crate::contexte::socle_livre_lu(an).unwrap().unwrap();
        let (s25, s26) = (lire(2025), lire(2026));
        let pose = |code: &str, t: f64| TauxAtmpEcrit { code_risque: Some(code.into()), valeur: Some(ValeurAtmp::Seule(socle::forme::Nombre(dec_f64(t).unwrap()))), ..TauxAtmpEcrit::default() };
        let t = taux_atmp_reporte(2026, Some(&s26), (2025, &pose("923AD", 1.65)), Some(&s25), false).unwrap().unwrap();
        assert_eq!((t.code_risque.as_deref(), t.valeur.map(|v| v.valeurs()[0].1)), (Some("923AD"), Some(Decimal::new(150, 2))));
        assert!(taux_atmp_reporte(2026, Some(&s26), (2025, &pose("923AD", 2.10)), Some(&s25), false).unwrap().is_err());
        assert!(taux_atmp_reporte(2026, Some(&s26), (2025, &pose("921CC", 1.04)), Some(&s25), false).unwrap().is_err());
        assert!(taux_atmp_reporte(2026, Some(&s26), (2025, &pose("923AD", 1.65)), Some(&s25), true).unwrap().is_err());
        assert!(taux_atmp_reporte(2027, None, (2026, &pose("923AD", 1.50)), Some(&s26), false).unwrap().is_err());
        assert!(taux_atmp_reporte(2026, Some(&s26), (2025, &TauxAtmpEcrit { valeur: Some(ValeurAtmp::Seule(socle::forme::Nombre(Decimal::new(165, 2)))), ..TauxAtmpEcrit::default() }), Some(&s25), false).is_none());
    }
}

/// Le taux AT/MP `t` ecrit pour l'ecran : « 1,50 % ».
fn pourcent(t: Decimal) -> String {
    let mut t = t.round_dp(2);
    t.rescale(2);
    format!("{} %", t.to_string().replace('.', ","))
}

/// Ou le code et le taux notifies se saisissent.
const OU_SAISIR_ATMP: &str = "Ils se lisent sur la notification de la CARSAT (compte AT/MP sur net-entreprises) et se saisissent dans Configuration › Entreprise, « Accidents du travail (AT/MP) ».";

/// LE TAUX AT/MP D'UNE ANNEE QUE LA COMPAGNIE N'A PAS DIT, d'apres celui qu'elle a dit pour une annee d'avant (`avant` :
/// `(annee, taux_atmp)` de sa surcouche). Le code risque est celui que la CARSAT a notifie a l'etablissement, et il le
/// reste. Pour un risque « TC » de l'annexe 1 de l'arrete de l'annee (`socle_an`, `risques_atmp`), le taux est collectif
/// quel que soit l'effectif (D. 242-6-14, I ; art. 5 de l'arrete, `atelier/reference/sources/tarification-atmp/`) : c'est
/// celui de l'arrete de l'annee, et la compagnie n'a rien a redire au 1er janvier. Il ne se reprend que si celui d'avant
/// etait deja ce taux collectif (`socle_avant`) : un taux notifie propre a la compagnie ne dit rien de l'annee suivante.
/// Tout autre cas (un risque sans TC, 999ZZ, l'Alsace-Moselle, une annee dont l'arrete n'est pas depose) demande le taux
/// notifie de l'annee : `Err` dit lequel et ou le saisir. `None` : aucun code n'a ete dit avant. Un taux d'avant que
/// sa notification datait en cours d'annee (une liste) n'etait pas le taux collectif de l'annee : il ne se reprend pas.
pub fn taux_atmp_reporte(an: i32, socle_an: Option<&Pile>, avant: (i32, &TauxAtmpEcrit), socle_avant: Option<&Pile>, alsace_moselle: bool) -> Option<Result<TauxAtmpEcrit, String>> {
    let (pa, pose) = avant;
    let code = pose.code_risque.as_deref().map(str::trim).filter(|c| !c.is_empty())?.to_string();
    if code == SANS_NOTIFICATION {
        return Some(Err(format!(
            "Le code risque et le taux AT/MP de {} manquent : en {}, la compagnie n'avait pas encore de notification (999ZZ). {}",
            an, pa, OU_SAISIR_ATMP
        )));
    }
    let r_avant = socle_avant.and_then(|s| risques_de(s).into_iter().find(|r| r.code == code));
    let nom = r_avant.as_ref().and_then(|r| r.code_arrete.clone()).unwrap_or_else(|| code.clone());
    let valeur_avant = match &pose.valeur {
        Some(ValeurAtmp::Seule(n)) => Some(n.0.round_dp(2)),
        _ => None,
    };
    let collectif_avant = r_avant.as_ref().and_then(|r| taux_de_l_arrete(r, None, alsace_moselle));
    let notifie = |pourquoi: String| {
        Err(format!("Le taux AT/MP de {} manque (code risque {}). {} Celui de {} se lit sur la notification de la CARSAT (compte AT/MP sur net-entreprises) et se saisit dans Configuration › Entreprise, « Accidents du travail (AT/MP) ».", an, nom, pourquoi, an))
    };
    if alsace_moselle {
        return Some(notifie("L'établissement est en Alsace-Moselle : son taux est celui que la CARSAT lui notifie chaque année.".into()));
    }
    if valeur_avant.is_none() || valeur_avant != collectif_avant {
        return Some(notifie(format!(
            "Celui de {}{} était le taux notifié à la compagnie, qui change chaque année.",
            pa,
            valeur_avant.map(|t| format!(" ({})", pourcent(t))).unwrap_or_default()
        )));
    }
    let Some(r) = socle_an.and_then(|s| risques_de(s).into_iter().find(|r| r.code == code)) else {
        return Some(notifie(format!("Son taux est celui de l'arrêté de tarification de {}, que le logiciel n'a pas.", an)));
    };
    if !r.tc {
        return Some(notifie("Ce risque n'a un taux collectif que sous 20 salariés (code de la sécurité sociale, art. D. 242-6-2) : au-delà, c'est celui que la CARSAT notifie.".into()));
    }
    let Some(t) = taux_de_l_arrete(&r, None, false) else {
        return Some(notifie(format!("L'arrêté de tarification de {} ne donne pas son taux.", an)));
    };
    Some(Ok(TauxAtmpEcrit {
        valeur: Some(ValeurAtmp::Seule(socle::forme::Nombre(ecrit(t)))),
        code_risque: Some(code),
        reporte_de: Some(pa),
        provenance: Some(format!(
            "Code risque notifié, dit pour {} ; taux collectif de {} ({}, « TC » : code de la sécurité sociale, art. D. 242-6-14), atelier/reference/sources/tarification-atmp/.",
            pa,
            socle_an.and_then(arrete_atmp).unwrap_or_else(|| format!("l'arrêté de tarification de {}", an)),
            r.code_arrete.as_deref().unwrap_or(&r.code)
        )),
        ..TauxAtmpEcrit::default()
    }))
}

/// Le nom de l'arrete qui fixe les taux du socle (`risques_atmp.arrete`), pour le dire a l'ecran.
pub fn arrete_atmp(socle: &Pile) -> Option<String> {
    socle.risques_atmp.as_ref().map(|r| r.arrete.clone())
}
