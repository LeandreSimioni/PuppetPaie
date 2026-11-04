//! LE MAINTIEN DE SALAIRE D'UN PERMANENT EN ARRET, mois par mois : les jours que chaque texte indemnise, ce qu'il garantit
//! ce mois-la, les indemnites journalieres de la caisse qui s'en deduisent, et la subrogation. Le droit (la loi, la
//! convention, a l'anciennete du salarie) se lit dans `droit::maintien` ; la paie se fait dans `calcul`, et le seul calcul
//! de plus est celui d'un maintien garanti en NET (`au_net`), qui refait la paie du mois jusqu'au centime.
//!
//! LES JOURS. Un texte compte en jours de calendrier : la carence (D1226-3 ; celle de la convention), puis ses tranches,
//! chacune a son pourcentage (D1226-1), une tranche en mois durant de date a date. Ce que les arrets des douze mois
//! precedents ont deja indemnise s'impute sur les tranches (D1226-4 ; 1285, X.4.6, al. 4 ; 3090, 11.2) : le moteur le lit
//! dans les arrets du magasin, chacun compte par le meme texte, et `jours_deja_indemnises` (saisi sur l'arret) dit ceux
//! qu'aucun arret du magasin ne porte. INFERENCE : les douze mois courent jusqu'au premier jour de l'arret (les textes
//! disent « les douze mois anterieurs » a la periode de paie : un arret dans un seul mois de paie ne les distingue pas).
//!
//! LA GARANTIE. Pour chaque suite de jours au meme pourcentage, la part du salaire que l'absence retire (`imprevus`,
//! la retenue au reel) fois le pourcentage : c'est « la remuneration brute que le salarie aurait percue s'il avait
//! continue a travailler » (D1226-1). Le complement est la garantie moins les indemnites journalieres brutes des memes
//! jours (D1226-5 ; 1285, X.4.7 « sous deduction de ces indemnites »), jamais negatif. INFERENCE : l'indemnite deduite
//! est son montant brut, avant CSG et CRDS (le releve et le BPIJ le portent : `prix unitaire`).
//!
//! LA PLUS FAVORABLE. La loi et la convention s'appliquent toutes deux : le mois paie le plus haut des deux complements
//! (L1226-1 fixe un minimum que la convention peut depasser ; 3252, 8.2.1, et 3241, 52, sont etendues « sous reserve » de
//! L1226-1, D1226-1 et D1226-2).
//!
//! LA SUBROGATION. Subrogee, la compagnie percoit les indemnites a la place du salarie et les lui reverse : le bulletin
//! porte le salaire maintenu (le complement et les indemnites brutes), en deduit les indemnites brutes, et ajoute apres
//! le net les indemnites nettes de CSG et CRDS, hors cotisations (BOSS, assiette generale, § 850 : « les indemnites
//! journalieres de securite sociale non soumises a cotisations », `atelier/reference/sources/boss/`) et hors net
//! imposable (norme DSN 2026.1, § 2.3.2, p. 60 : « l'employeur ne doit declarer que les remunerations dont il a eu la
//! charge effective [...] a l'exclusion des indemnites journalieres »), mais dans l'assiette du prelevement a la source
//! (S21.G00.50.013 ; BOFiP BOI-IR-PAS-20-10-10, § 80 a 110) et dans le montant net social (BOSS, montant net social,
//! Q 13 bis : « pour leur montant net des prelevements sociaux », S21.G00.58 de type 10). Non subrogee, le bulletin ne
//! porte que le complement. Les dates de la subrogation sont celles de la duree maximale du maintien, pas celles de
//! l'arret (ameli, « La subrogation de salaire en cas d'arret de travail » ; net-entreprises, fiche 3028 :
//! `atelier/reference/sources/subrogation-indemnites-journalieres/`).

use serde_json::{json, Map, Value};

use crate::conges::{flot, info};
use crate::Avis;
use droit::contexte::Contexte;
use droit::maintien::{self as dm, Base, Droit, Duree, Origine, Regime};
use droit::natures::Nature;
use socle::date::{d, en_texte, Jour};
use socle::fichiers::vrai;
use socle::num::{dec_avec_echelle, div, eur2, mul, Decimal, Resultat};

const NUL: Value = Value::Null;

/// La rubrique des indemnites journalieres nettes que la compagnie subrogee reverse, et celle des brutes qu'elle deduit.
pub const IJ_NETTES: &str = "ijss_nettes";
pub const IJ_BRUTES: &str = "ijss_brutes";
/// Les cles de `Paie::bases` : ces indemnites nettes, et leur assiette du prelevement a la source.
pub const IJ_SUBROGEES: &str = "ij_subrogees_nettes";
pub const IJ_ASSIETTE_PAS: &str = "ij_subrogees_assiette_pas";
/// Ce que dit le refus d'un mois dont les indemnites journalieres manquent : une saisie a finir (`saisie_incomplete`).
pub const IJ_A_DIRE: &str = ", à dire sur l'arrêt (« Indemnités journalières »)";

fn g<'a>(v: &'a Value, k: &str) -> &'a Value {
    v.get(k).unwrap_or(&NUL)
}

/// Les deux textes qu'un arret met en jeu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Texte {
    Loi,
    Convention,
}

/// Ce que la loi et la convention donnent pour un arret.
#[derive(Debug, Clone)]
pub struct Droits {
    pub loi: Droit,
    pub convention: Option<Droit>,
    pub origine: Origine,
    pub anciennete_mois: Decimal,
    /// Ce que les suspensions retirent de l'anciennete (L1225-54), dit.
    pub sur_l_anciennete: Vec<Avis>,
}

impl Droits {
    /// Le regime d'un texte, quand le salarie y a droit.
    pub fn regime(&self, t: Texte) -> Option<&Regime> {
        match (t, &self.loi, &self.convention) {
            (Texte::Loi, Droit::Du(r), _) => Some(r),
            (Texte::Convention, _, Some(Droit::Du(r))) => Some(r),
            _ => None,
        }
    }

    /// Le salarie a-t-il droit au maintien, par l'un ou l'autre texte ?
    pub fn du(&self) -> bool {
        self.regime(Texte::Loi).is_some() || self.regime(Texte::Convention).is_some()
    }
}

/// L'origine d'un arret, lue sur son motif ; `None` pour un motif sans maintien (maternite, paternite...).
pub fn origine(imp: &Value) -> Option<Origine> {
    Origine::du_motif(&en_texte(g(imp, "motif")))
}

/// LES DROITS D'UN ARRET : a l'anciennete au premier jour de l'absence (D1226-8), comptee depuis la date d'anciennete de la
/// fiche ou le debut du contrat, moins les suspensions qui ne comptent pas (`imprevus::anciennete_depuis`). La convention
/// ne se lit que pour un contrat paye au mois : ses clauses de maintien sont celles des permanents (3252 : « ces clauses
/// sont celles des PERMANENTS », `clauses-conventionnelles-base.json`) ; la loi exclut les intermittents (L1226-1).
pub fn droits(ctx: &Contexte, imp: &Value, nat: &Nature) -> Resultat<Option<Droits>> {
    let Some(origine) = origine(imp) else { return Ok(None) };
    let premier = crate::imprevus::premier_jour(imp)?;
    let fiche = ctx.salarie(&en_texte(g(imp, "salarie")))?;
    let depuis = match fiche.date_anciennete.dite() {
        Some(j) => *j,
        None => d(g(imp, "contrat_debut"))?,
    };
    let (depuis, sur_l_anciennete) = crate::imprevus::anciennete_depuis(&ctx.racine, fiche.id.as_str(), &depuis, &premier)?;
    let anciennete_mois = droit::permanents::anciennete_mois(&depuis, &premier);
    let loi = dm::legal(nat, anciennete_mois, origine)?;
    let convention = if nat.mensuelle() { dm::conventionnel(&ctx.entreprise.idcc.en_valeur(), anciennete_mois, origine)? } else { None };
    Ok(Some(Droits { loi, convention, origine, anciennete_mois, sur_l_anciennete }))
}

// ------------------------------------------------------------------ les jours

/// Le jour `n` mois de calendrier apres `j`, au meme quantieme (le dernier du mois s'il n'existe pas).
fn plus_mois(j: &Jour, n: i64) -> Resultat<Jour> {
    let total = j.an as i64 * 12 + (j.mois as i64 - 1) + n;
    let (an, mois) = ((total / 12) as i32, (total % 12 + 1) as u32);
    let jour = j.jour.min(socle::date::jours_du_mois(an, mois));
    Jour::nouveau(an, mois, jour).ok_or_else(|| socle::date::date_impossible(an, mois, jour))
}

/// La capacite de chaque tranche en jours, la tranche commencant au jour `debut` (une tranche en mois dure de date a date ;
/// `None` : tant que la caisse indemnise).
fn capacites(r: &Regime, debut: &Jour) -> Resultat<Vec<Option<i64>>> {
    let mut out = Vec::new();
    let mut d0 = debut.clone();
    for t in &r.tranches {
        let c = match t.duree {
            Duree::Jours(j) => Some(j),
            Duree::Mois(m) => Some(plus_mois(&d0, m)?.moins(&d0)),
            Duree::PendantLesIndemnites => None,
        };
        if let Some(c) = c {
            d0 = d0.plus_jours(c)?;
        }
        out.push(c);
    }
    Ok(out)
}

/// LE POURCENTAGE DE CHAQUE JOUR D'UN ARRET, du premier au dernier jour d'absence, en fraction (zero : pas indemnise) :
/// la carence, puis les tranches, diminuees d'abord de `deja` jours deja indemnises dans les douze mois.
pub fn taux_des_jours(r: &Regime, premier: &Jour, dernier: &Jour, deja: i64) -> Resultat<Vec<Decimal>> {
    let n = (dernier.moins(premier) + 1).max(0);
    let carence = r.carence.max(0);
    let mut out = vec![Decimal::ZERO; n as usize];
    let mut caps = capacites(r, &premier.plus_jours(carence)?)?;
    let mut reste = deja.max(0);
    for c in caps.iter_mut() {
        if let Some(x) = c {
            let pris = (*x).min(reste);
            *x -= pris;
            reste -= pris;
        }
    }
    let (mut k, mut dans) = (0usize, 0i64);
    for i in carence..n {
        while k < caps.len() && caps[k].is_some_and(|c| dans >= c) {
            k += 1;
            dans = 0;
        }
        if k >= caps.len() {
            break;
        }
        out[i as usize] = dm::fraction(r.tranches[k].taux);
        dans += 1;
    }
    Ok(out)
}

/// Le dernier jour que le texte peut indemniser pour cet arret, d'un seul tenant depuis son premier jour : la duree
/// maximale du maintien, que la subrogation declare (ameli, fiche 3028) ; la fin de l'arret pour une tranche qui dure
/// tant que la caisse indemnise.
pub fn fin_possible(r: &Regime, premier: &Jour, fin_arret: &Jour) -> Resultat<Jour> {
    let debut = premier.plus_jours(r.carence.max(0))?;
    let mut total = 0i64;
    for c in capacites(r, &debut)? {
        match c {
            Some(x) => total += x,
            None => return Ok(fin_arret.clone()),
        }
    }
    debut.plus_jours(total - 1)
}

/// Les jours deja indemnises, pour le texte `t`, dans les douze mois qui precedent le premier jour de `imp` : ceux des
/// arrets anterieurs du meme salarie (comptes par le meme texte, a leur propre anciennete), plus ceux que l'arret dit
/// (`jours_deja_indemnises` : ce qu'aucun arret du magasin ne porte).
pub fn deja_indemnises(ctx: &Contexte, imp: &Value, nat: &Nature, t: Texte) -> Resultat<i64> {
    let premier = crate::imprevus::premier_jour(imp)?;
    let debut_fenetre = plus_mois(&premier, -12)?;
    let saisis = match g(imp, "jours_deja_indemnises") {
        x if vrai(x) => crate::conges::entier(x)?,
        _ => 0,
    };
    let mut total = saisis;
    let cle = en_texte(g(imp, "salarie"));
    for autre in crate::imprevus::du_salarie(&ctx.racine, &cle)? {
        if !crate::imprevus::est_un_arret(&autre) || en_texte(g(&autre, "salarie")) != cle {
            continue;
        }
        let (ap, ad) = (crate::imprevus::premier_jour(&autre)?, crate::imprevus::dernier_jour(&autre)?);
        if ap >= premier || ad < debut_fenetre {
            continue;
        }
        let Some(dr) = droits(ctx, &autre, nat)? else { continue };
        let Some(r) = dr.regime(t) else { continue };
        let taux = taux_des_jours(r, &ap, &ad, deja_indemnises(ctx, &autre, nat, t)?)?;
        for (i, x) in taux.iter().enumerate() {
            let j = ap.plus_jours(i as i64)?;
            if *x > Decimal::ZERO && j >= debut_fenetre && j < premier {
                total += 1;
            }
        }
    }
    Ok(total)
}

// ------------------------------------------------------------------ la subrogation

/// La subrogation d'un arret, `(debut, fin)`, ou `None`. Elle vient avec le maintien : la convention qui la donne a tout
/// salarie (1285, X.4.6 : « tout salarie beneficie de la subrogation et du maintien de son salaire brut »), sinon le choix
/// de la compagnie (fiche entreprise, `subrogation_ij`) ; ses dates vont du premier jour de l'arret a la fin de la duree
/// maximale du maintien (« Il ne faut surtout pas se limiter a la date de fin de l'arret », ameli).
pub fn subrogation(ctx: &Contexte, imp: &Value, nat: &Nature) -> Resultat<Option<(Jour, Jour)>> {
    let Some(dr) = droits(ctx, imp, nat)? else { return Ok(None) };
    let regimes: Vec<&Regime> = [Texte::Loi, Texte::Convention].iter().filter_map(|t| dr.regime(*t)).collect();
    if regimes.is_empty() {
        return Ok(None);
    }
    let choisie = ctx.entreprise.subrogation_ij.dite() == Some(&droit::fiche::Subrogation::Oui);
    if !choisie && !regimes.iter().any(|r| r.subrogation) {
        return Ok(None);
    }
    let premier = crate::imprevus::premier_jour(imp)?;
    let dernier = crate::imprevus::dernier_jour(imp)?;
    let mut fin = premier.clone();
    for r in regimes {
        fin = fin.max(fin_possible(r, &premier, &dernier)?);
    }
    Ok(Some((premier, fin)))
}

// ------------------------------------------------------------------ le mois

/// Ce que les arrets d'un permanent donnent a son mois, tous arrets du mois ensemble.
#[derive(Debug, Clone, Default)]
pub struct DuMois {
    /// Le complement de la loi, brut.
    pub loi: Decimal,
    /// Le complement de la convention en brut (son pourcentage est du brut, sans plafond net a verifier).
    pub convention_brut: Option<Decimal>,
    /// La convention garantit un pourcentage du net, ou plafonne au net : le complement se reconstitue (`au_net`).
    pub au_net: Option<AuNet>,
    /// Les indemnites journalieres de la periode subrogee : brutes, nettes, leurs jours, l'assiette du PAS.
    pub ij_subrogees_brutes: Decimal,
    pub ij_subrogees_nettes: Decimal,
    pub ij_subrogees_jours: i64,
    pub assiette_pas: Decimal,
    /// Les heures d'absence que le maintien couvre (la quantite de sa ligne).
    pub heures: Decimal,
    pub avis: Vec<Avis>,
}

/// Ce qu'il faut pour reconstituer une garantie nette : la part du salaire retiree et son pourcentage, suite par suite
/// de jours ; les indemnites nettes que le salarie percoit hors du bulletin (sans subrogation) et celles des jours que le
/// texte n'indemnise pas.
#[derive(Debug, Clone, Default)]
pub struct AuNet {
    pub texte: String,
    pub net: bool,
    pub plafond_net: bool,
    pub parts: Vec<(Decimal, Decimal)>,
    pub ij_nettes_hors_bulletin: Decimal,
    pub ij_nettes_hors_garantie: Decimal,
    pub complement_brut: Decimal,
}

/// Les indemnites par jour d'un arret, lues sur l'arret (`ij_brute`, `ij_nette`, et pour un accident du travail au-dela
/// du vingt-huitieme jour, `ij_brute_majoree`, `ij_nette_majoree`).
fn ij_du_jour(imp: &Value, majoree: bool) -> Resultat<Option<(Decimal, Decimal)>> {
    let (b, n) = if majoree { ("ij_brute_majoree", "ij_nette_majoree") } else { ("ij_brute", "ij_nette") };
    match (g(imp, b), g(imp, n)) {
        (x, y) if vrai(x) && vrai(y) => Ok(Some((dec_avec_echelle(x)?, dec_avec_echelle(y)?))),
        _ => Ok(None),
    }
}

/// Le premier jour indemnise par la caisse (R323-1 ; L433-1) et le jour ou l'indemnite d'un accident du travail se
/// majore (R433-3), en rang depuis le premier jour d'absence.
fn jours_de_la_caisse(origine: Origine) -> Resultat<(i64, Option<i64>)> {
    let ij = &droit::permanents::regles()?["indemnites_journalieres"];
    let n = |k: &str| ij.get(k).and_then(Value::as_i64).ok_or_else(|| socle::fichiers::absent(k));
    Ok(if origine.professionnelle() || origine == Origine::Trajet {
        (0, Some(n("majoree_des_le_jour")? - 1))
    } else {
        (n("debut_maladie")? - 1, None)
    })
}

/// Une suite de jours d'absence du mois au meme pourcentage : ses bornes et son pourcentage.
fn suites(taux: &[Decimal], premier: &Jour, debut: &Jour, fin: &Jour) -> Resultat<Vec<(Jour, Jour, Decimal)>> {
    let mut out: Vec<(Jour, Jour, Decimal)> = Vec::new();
    for (i, x) in taux.iter().enumerate() {
        let j = premier.plus_jours(i as i64)?;
        if j < *debut || j > *fin {
            continue;
        }
        match out.last_mut() {
            Some(s) if s.2 == *x && s.1.plus_jours(1)? == j => s.1 = j,
            _ => out.push((j.clone(), j, *x)),
        }
    }
    Ok(out)
}

/// La part du salaire du mois que les jours [debut, fin] retirent : la retenue au reel (`imprevus::lignes_du_mois`).
fn retenue(m: &crate::imprevus::MoisDuContrat, taux_horaire: Decimal, debut: &Jour, fin: &Jour) -> Resultat<(Decimal, Decimal)> {
    let jours = droit::periodes::part_du_mois(debut, fin, &m.fin_mois, droit::periodes::RegleDuMoisIncomplet::HoraireReel, m.horaire)?.jours;
    let heures = mul(m.heures_par_jour, Decimal::from(jours));
    Ok((mul(taux_horaire, heures), heures))
}

/// CE QU'UN ARRET DONNE AU MOIS `mois` D'UN PERMANENT, ajoute a `dm` (les arrets du mois s'additionnent). Refuse quand le
/// maintien est du et que les indemnites journalieres du mois ne sont pas dites (regle 1 : elles se lisent sur le releve
/// de la caisse).
pub fn ajouter(ctx: &Contexte, e: &Value, imp: &Value, nat: &Nature, mois: &str, m: &crate::imprevus::MoisDuContrat, out: &mut DuMois) -> Resultat<()> {
    let qui = crate::conges::qui(ctx, e);
    let Some(dr) = droits(ctx, imp, nat)? else { return Ok(()) };
    out.avis.extend(dr.sur_l_anciennete.iter().cloned());
    if !dr.du() {
        let mut raisons = Vec::new();
        if let Droit::PasDu(r) = &dr.loi {
            raisons.push(r.clone());
        }
        if let Some(Droit::PasDu(r)) = &dr.convention {
            raisons.push(r.clone());
        }
        out.avis.push(info(format!("{} : pas de maintien de salaire pour cet arrêt : {}", qui, raisons.join(" "))));
        return Ok(());
    }
    let (premier, dernier) = (crate::imprevus::premier_jour(imp)?, crate::imprevus::dernier_jour(imp)?);
    let (debut, fin) = (premier.clone().max(m.debut.clone()), dernier.clone().min(m.fin.clone()));
    if debut > fin {
        return Ok(());
    }
    let heures_du_mois = mul(m.heures_par_jour, Decimal::from(m.jours_payes));
    if heures_du_mois <= Decimal::ZERO {
        return Ok(());
    }
    let taux_horaire = div(m.salaire_du_mois, heures_du_mois);
    let n = dernier.moins(&premier) + 1;
    let subroge = vrai(g(imp, "subrogation")) && en_texte(g(imp, "subrogation")) == "01";
    let (sub_debut, sub_fin) = (
        Some(g(imp, "subrogation_debut")).filter(|v| vrai(v)).map(d).transpose()?,
        Some(g(imp, "subrogation_fin")).filter(|v| vrai(v)).map(d).transpose()?,
    );
    let regles = droit::permanents::regles()?;
    let rij = &regles["indemnites_journalieres"];
    let csg_ded = dec_avec_echelle(&rij["csg_deductible"])?;
    let pas_jours = rij["pas_maladie_jours"].as_i64().unwrap_or(0);
    let pas_accident = dec_avec_echelle(&rij["pas_accident"])?;

    // Les indemnites de la caisse, jour par jour du mois (rang depuis le premier jour d'absence).
    let ij = match indemnites_des_jours(imp, dr.origine, &premier, &dernier, &debut, &fin)? {
        Ok(ij) => ij,
        Err(majoree) => return Err(refus_des_indemnites(&qui, mois, &premier, &dernier, majoree)),
    };
    let ij_de = |a: &Jour, b: &Jour| -> Resultat<(Decimal, Decimal)> {
        let (mut sb, mut sn) = (Decimal::ZERO, Decimal::ZERO);
        let mut j = a.clone();
        while j <= *b {
            if let Some((x, y)) = ij[j.moins(&premier) as usize] {
                sb += x;
                sn += y;
            }
            j = j.plus_jours(1)?;
        }
        Ok((sb, sn))
    };

    // La subrogation : les indemnites des jours qu'elle couvre passent par le bulletin.
    let (mut sb, mut sn) = (Decimal::ZERO, Decimal::ZERO);
    if subroge {
        if let (Some(a), Some(b)) = (&sub_debut, &sub_fin) {
            for i in 0..n {
                let j = premier.plus_jours(i)?;
                if j < *a || j > *b {
                    continue;
                }
                if let Some((x, y)) = ij[i as usize] {
                    sb += x;
                    sn += y;
                    out.ij_subrogees_jours += 1;
                    let imposable = mul(x, Decimal::ONE - div(csg_ded, Decimal::ONE_HUNDRED));
                    out.assiette_pas += match dr.origine {
                        Origine::Maladie if i < pas_jours => imposable,
                        Origine::Maladie => Decimal::ZERO,
                        _ => div(mul(imposable, pas_accident), Decimal::ONE_HUNDRED),
                    };
                }
            }
        }
    }
    out.ij_subrogees_brutes += sb;
    out.ij_subrogees_nettes += sn;

    // Chaque texte, suite par suite : la garantie, et le complement apres deduction des indemnites.
    let mut dit = Vec::new();
    let mut heures_max = Decimal::ZERO;
    for t in [Texte::Loi, Texte::Convention] {
        let Some(r) = dr.regime(t) else { continue };
        let deja = deja_indemnises(ctx, imp, nat, t)?;
        let taux = taux_des_jours(r, &premier, &dernier, deja)?;
        let (mut complement, mut garantie, mut deduit) = (Decimal::ZERO, Decimal::ZERO, Decimal::ZERO);
        let mut parts = Vec::new();
        let mut hors_garantie = Decimal::ZERO;
        let mut heures = Decimal::ZERO;
        for (a, b, f) in suites(&taux, &premier, &debut, &fin)? {
            let (x, h) = retenue(m, taux_horaire, &a, &b)?;
            let (ib, inet) = ij_de(&a, &b)?;
            parts.push((x, f));
            if f > Decimal::ZERO {
                let gs = mul(x, f);
                garantie += gs;
                deduit += ib;
                complement += (gs - ib).max(Decimal::ZERO);
                heures += h;
            } else {
                hors_garantie += inet;
            }
        }
        let complement = eur2(complement);
        heures_max = heures_max.max(heures);
        let deja_dit = if deja > 0 { format!(" ({} jours déjà indemnisés dans les douze mois)", deja) } else { String::new() };
        dit.push(if r.base == Base::Net {
            format!("{}{} — le net garanti se reconstitue à la paie", dm::phrase(r), deja_dit)
        } else {
            format!(
                "{}{} — garantie ce mois {}, moins {} d'indemnités journalières",
                dm::phrase(r),
                deja_dit,
                socle::texte::euros(eur2(garantie)),
                socle::texte::euros(eur2(deduit))
            )
        });
        match t {
            Texte::Loi => out.loi += complement,
            Texte::Convention if r.base == Base::Net || r.plafond_net => {
                let a = out.au_net.get_or_insert_with(AuNet::default);
                a.texte = r.texte.clone();
                a.net |= r.base == Base::Net;
                a.plafond_net |= r.plafond_net;
                a.parts.extend(parts);
                a.ij_nettes_hors_garantie += hors_garantie;
                a.complement_brut += if r.base == Base::Brut { complement } else { Decimal::ZERO };
            }
            Texte::Convention => *out.convention_brut.get_or_insert(Decimal::ZERO) += complement,
        }
    }
    out.heures += heures_max;
    if let Some(a) = out.au_net.as_mut() {
        let (_, toutes) = ij_de(&debut, &fin)?;
        a.ij_nettes_hors_bulletin += toutes - sn;
    }
    out.avis.push(info(format!(
        "{} : maintien de salaire de l'arrêt du {} au {}{} ; {}.",
        qui,
        premier.jj_mm_aaaa(),
        dernier.jj_mm_aaaa(),
        if subroge { ", la compagnie subrogée" } else { "" },
        dit.join(" ; ")
    )));
    Ok(())
}


/// Les indemnites de chaque jour d'un arret dans [debut, fin], en rang depuis son premier jour (`None` : un jour que la
/// caisse n'indemnise pas, ou hors du mois) ; `Err(majoree)` quand l'arret ne dit pas celles d'un jour indemnise
/// (`majoree` : seulement celles d'un accident du travail au-dela du vingt-huitieme jour).
fn indemnites_des_jours(imp: &Value, origine: Origine, premier: &Jour, dernier: &Jour, debut: &Jour, fin: &Jour) -> Resultat<Result<Vec<Option<(Decimal, Decimal)>>, bool>> {
    let (rang_caisse, rang_majore) = jours_de_la_caisse(origine)?;
    let (mut ij, mut manque, mut manque_majoree) = (Vec::new(), false, false);
    for i in 0..(dernier.moins(premier) + 1) {
        let j = premier.plus_jours(i)?;
        if j < *debut || j > *fin || i < rang_caisse {
            ij.push(None);
            continue;
        }
        let majoree = rang_majore.is_some_and(|r| i >= r);
        let x = ij_du_jour(imp, majoree)?;
        if x.is_none() {
            if majoree {
                manque_majoree = true;
            } else {
                manque = true;
            }
        }
        ij.push(x);
    }
    Ok(if manque || manque_majoree { Err(manque_majoree && !manque) } else { Ok(ij) })
}

/// Le refus d'un mois dont les indemnites d'un arret ne sont pas dites : une saisie a finir (`IJ_A_DIRE`).
fn refus_des_indemnites(qui: &str, mois: &str, premier: &Jour, dernier: &Jour, majoree: bool) -> String {
    format!(
        "{} : les indemnités journalières de {} (l'arrêt du {} au {}){} : le montant par jour{}, brut puis net de CSG et \
CRDS : celui du relevé de la CPAM ou le montant de principe proposé. Le maintien de salaire s'en déduit ; rien n'est \
calculé.",
        qui,
        socle::mois::Mois::en_clair(mois),
        premier.jj_mm_aaaa(),
        dernier.jj_mm_aaaa(),
        IJ_A_DIRE,
        if majoree { " à partir du 29e jour (80 %)" } else { "" }
    )
}

/// CE QUI MANQUE AU MOIS `mois` D'UN ENGAGEMENT PAYE AU MOIS POUR SE CALCULER, lu sans calculer : les indemnites
/// journalieres d'un arret qui ouvre le maintien (le refus que `ajouter` donnerait). Un engagement valide en manque
/// quand un arret vient apres sa validation : l'ecran le montre a completer.
pub fn indemnites_a_dire(ctx: &Contexte, e: &Value, mois: &str) -> Resultat<Option<String>> {
    let nat = crate::conges::nature_de(e.get("nature"))?;
    if !nat.mensuelle() || !vrai(g(e, "debut")) {
        return Ok(None);
    }
    let (pm, fm) = (socle::mois::Mois::lire(mois)?.premier_jour(), socle::mois::Mois::lire(mois)?.dernier_jour());
    let debut = d(g(e, "debut"))?.max(pm);
    let fin = match Some(g(e, "fin")).filter(|v| vrai(v)) {
        Some(f) => d(f)?.min(fm),
        None => fm,
    };
    for imp in crate::imprevus::du_contrat_dans_le_mois(ctx, e, mois)? {
        if !crate::imprevus::est_un_arret(&imp) || !droits(ctx, &imp, &nat)?.is_some_and(|dr| dr.du()) {
            continue;
        }
        let (premier, dernier) = (crate::imprevus::premier_jour(&imp)?, crate::imprevus::dernier_jour(&imp)?);
        let Some(o) = origine(&imp) else { continue };
        if let Err(majoree) = indemnites_des_jours(&imp, o, &premier, &dernier, &debut.clone().max(premier.clone()), &fin.clone().min(dernier.clone()))? {
            return Ok(Some(refus_des_indemnites(&crate::conges::qui(ctx, e), mois, &premier, &dernier, majoree)));
        }
    }
    Ok(None)
}
/// LE MONTANT DE PRINCIPE DES INDEMNITES D'UN ARRET, propose avant le releve de la caisse : par jour, brut et net de CSG et
/// CRDS (et, pour un accident du travail, de trajet ou une maladie professionnelle, le taux majore des le 29e jour), et la phrase qui le dit.
/// Textes : `logiciel/donnees/permanents-base.json`, `indemnites_journalieres._principe` et `._principe_accident`.
pub struct IjDePrincipe {
    pub brute: Decimal,
    pub nette: Decimal,
    pub majorees: Option<(Decimal, Decimal)>,
    pub phrase: String,
}

const IJ_AU_RELEVE: &str = "Le montant d'un jour se lit sur le relevé de la CPAM.";
const IJ_A_VERIFIER: &str = "quand le relevé CPAM arrive, vérifiez le montant et corrigez-le s'il diffère.";

/// `Ok(Err(phrase))` quand aucun montant ne se propose : un accident survenu avant le plafond depose (R433-2), un mois sans
/// paie validee du salarie, un bareme absent.
/// Une maladie : la moitie (CSS R323-5) de 1/91,25 des trois dernieres paies des mois civils anterieurs a l'interruption
/// (R323-4, 1°), chacune dans la limite de 1,4 SMIC du mois, au SMIC en vigueur le dernier jour du mois civil precedant
/// l'interruption, sur la duree legale (R323-4, al. 5) ; le brut des paies validees, sans abattement (meme alinea).
/// Un accident du travail, de trajet ou une maladie professionnelle (la date de sa premiere constatation medicale, ou une
/// date posterieure, vaut date de l'accident, L461-1) : 60 %, puis 80 % (R433-1, R433-3), du salaire journalier, 1/30,42 de la paie du
/// mois civil anterieur (R433-4, 1°), pris dans la limite de 0,226 % du plafond annuel (L433-2, R433-2) ; l'indemnite au
/// plus le gain journalier net, ce salaire diminue du taux forfaitaire de 21 % (R433-4, dernier alinea ; R331-5, al. 2 ;
/// arrete du 28 mars 2013). Net : CSG (L136-8, II, 1°, b) et CRDS (ordonnance n° 96-50, art. 14, I, et 19).
pub fn ij_de_principe(ctx: &Contexte, imp: &Value) -> Resultat<Result<IjDePrincipe, String>> {
    let regles = &droit::permanents::regles()?["indemnites_journalieres"];
    let premier = crate::imprevus::premier_jour(imp)?;
    let veille = premier.mois_civil().premier_jour().plus_jours(-1)?;
    let salarie = en_texte(g(imp, "salarie"));
    // La paie validee d'un mois du salarie ; sans elle, rien ne se propose.
    let paie = |mois: &str| -> Resultat<Result<Decimal, String>> {
        let brut = crate::conges::remuneration_du_mois(ctx, mois, &salarie)?;
        Ok(if brut > Decimal::ZERO {
            Ok(brut)
        } else {
            Err(format!("Le montant d'un jour se lit sur le relevé de la CPAM : aucune paie validée en {}.", socle::mois::Mois::en_clair(mois)))
        })
    };
    match origine(imp) {
        Some(Origine::Maladie) => {
            let p = &regles["principe"];
            // Le SMIC du plafond, au bareme de son annee : sans lui, rien ne se propose.
            let Some(smic) = ctx.smic(&veille).ok().and_then(|s| Decimal::try_from(s).ok()) else {
                return Ok(Err(format!("Le montant d'un jour se lit sur le relevé de la CPAM : le barème {} n'est pas livré.", veille.an)));
            };
            let combien = nombre(p, "mois")?.to_string().parse::<usize>().map_err(|e| e.to_string())?;
            let (mut bruts, mut jour) = (Vec::new(), veille.clone());
            for _ in 0..combien {
                let mois = jour.mois_civil().to_string();
                match paie(&mois)? {
                    Ok(b) => bruts.push(b),
                    Err(phrase) => return Ok(Err(phrase)),
                }
                jour = socle::mois::Mois::lire(&mois)?.premier_jour().plus_jours(-1)?;
            }
            let brute = montant_de_principe(&bruts, smic, p)?;
            Ok(Ok(IjDePrincipe {
                brute,
                nette: net_de_csg_crds(brute, p)?,
                majorees: None,
                phrase: format!("Montant de principe (la moitié du salaire journalier des trois derniers mois) : {}", IJ_A_VERIFIER),
            }))
        }
        Some(Origine::AccidentDuTravail | Origine::Trajet | Origine::MaladieProfessionnelle) => {
            let p = &regles["principe_accident"];
            let accident = match imp.get("date_accident").filter(|x| vrai(x)) {
                Some(x) => d(x)?,
                None => return Ok(Err(IJ_AU_RELEVE.to_string())),
            };
            if accident < d(p.get("plafond_depuis").ok_or_else(|| socle::fichiers::absent("plafond_depuis"))?)? {
                return Ok(Err(IJ_AU_RELEVE.to_string()));
            }
            let Ok((mensuel, _)) = ctx.plafond_securite_sociale(premier.an) else {
                return Ok(Err(format!("Le montant d'un jour se lit sur le relevé de la CPAM : le barème {} n'est pas livré.", premier.an)));
            };
            let brut = match paie(&veille.mois_civil().to_string())? {
                Ok(b) => b,
                Err(phrase) => return Ok(Err(phrase)),
            };
            let (brute, majoree) = montants_d_accident(brut, Decimal::from(mensuel * 12), p)?;
            Ok(Ok(IjDePrincipe {
                brute,
                nette: net_de_csg_crds(brute, p)?,
                majorees: Some((majoree, net_de_csg_crds(majoree, p)?)),
                phrase: format!("Montant de principe (60 % du salaire journalier du mois précédent, 80 % dès le 29e jour) : {}", IJ_A_VERIFIER),
            }))
        }
        _ => Ok(Err(IJ_AU_RELEVE.to_string())),
    }
}

/// Une regle chiffree de `permanents-base.json`.
fn nombre(p: &Value, k: &str) -> Resultat<Decimal> {
    dec_avec_echelle(p.get(k).ok_or_else(|| socle::fichiers::absent(k))?)
}

/// L'indemnite brute d'un jour de maladie, des paies des trois mois (`bruts`) au SMIC horaire du plafond (`smic`), selon
/// les regles `principe` de `permanents-base.json` (voir `ij_de_principe`).
fn montant_de_principe(bruts: &[Decimal], smic: Decimal, p: &Value) -> Resultat<Decimal> {
    let plafond = eur2(mul(mul(nombre(p, "plafond_smic")?, smic), nombre(p, "heures_du_mois")?));
    let total = bruts.iter().fold(Decimal::ZERO, |t, b| t + (*b).min(plafond));
    Ok(eur2(mul(div(total, nombre(p, "diviseur")?), nombre(p, "fraction")?)))
}

/// Les indemnites brutes d'un jour d'accident, normale et majoree, de la paie du mois anterieur (`brut`) au plafond
/// annuel de la Securite sociale (`pass`), selon les regles `principe_accident` (voir `ij_de_principe`).
fn montants_d_accident(brut: Decimal, pass: Decimal, p: &Value) -> Resultat<(Decimal, Decimal)> {
    let journalier = div(brut, nombre(p, "diviseur")?);
    let base = journalier.min(eur2(div(mul(pass, nombre(p, "plafond_pass")?), Decimal::from(100))));
    let gain_net = journalier - div(mul(journalier, nombre(p, "taux_forfaitaire")?), Decimal::from(100));
    let au_taux = |k: &str| -> Resultat<Decimal> { Ok(eur2(mul(base, nombre(p, k)?).min(gain_net))) };
    Ok((au_taux("fraction")?, au_taux("fraction_majoree")?))
}

/// Une indemnite brute nette de la CSG et de la CRDS des revenus de remplacement.
fn net_de_csg_crds(brute: Decimal, p: &Value) -> Resultat<Decimal> {
    Ok(eur2(brute - mul(brute, div(nombre(p, "csg")? + nombre(p, "crds")?, Decimal::from(100)))))
}

/// Les lignes du mois : le maintien (le plus favorable des complements, plus les indemnites subrogees qu'il avance), et,
/// subroge, la deduction des indemnites brutes et le reversement des nettes. Quand la convention garantit du net (ou
/// plafonne au net), la ligne du maintien porte ce qu'il faut pour le reconstituer (`au_net`, sous la cle `maintien`).
pub fn lignes(dmois: &DuMois, mois: &str) -> Vec<Value> {
    let mut out = Vec::new();
    let plancher = eur2(dmois.ij_subrogees_brutes);
    let direct = eur2(dmois.loi.max(dmois.convention_brut.unwrap_or(Decimal::ZERO)));
    let montant = direct + plancher;
    if montant > Decimal::ZERO || dmois.au_net.is_some() {
        let q = if dmois.heures > Decimal::ZERO { eur2(dmois.heures) } else { Decimal::ONE };
        let mut l = Map::new();
        l.insert("rubrique".into(), json!("maintien_salaire"));
        l.insert("quantite".into(), json!(flot(q)));
        l.insert("taux".into(), Value::String(div(montant, q).normalize().to_string()));
        // Une somme, pas des heures de travail : les heures de l'absence restent celles de sa retenue (S21.G00.53).
        l.insert("heures".into(), json!(0));
        if let Some(a) = &dmois.au_net {
            l.insert(
                "maintien".into(),
                json!({
                    "mois": mois,
                    "texte": a.texte,
                    "net": a.net,
                    "plafond_net": a.plafond_net,
                    "parts": a.parts.iter().map(|(x, f)| json!([x.to_string(), f.to_string()])).collect::<Vec<_>>(),
                    "ij_nettes_hors_bulletin": eur2(a.ij_nettes_hors_bulletin).to_string(),
                    "ij_nettes_hors_garantie": eur2(a.ij_nettes_hors_garantie).to_string(),
                    "complement_brut": eur2(a.complement_brut).to_string(),
                    "complement_direct": direct.to_string(),
                    "plancher": plancher.to_string(),
                }),
            );
        }
        out.push(Value::Object(l));
    }
    if dmois.ij_subrogees_jours > 0 {
        let j = Decimal::from(dmois.ij_subrogees_jours);
        out.push(json!({"rubrique": IJ_BRUTES, "quantite": dmois.ij_subrogees_jours, "taux": div(plancher, j).normalize().to_string(), "heures": 0, "ij": true}));
        out.push(json!({
            "rubrique": IJ_NETTES, "quantite": dmois.ij_subrogees_jours,
            "taux": div(eur2(dmois.ij_subrogees_nettes), j).normalize().to_string(),
            "heures": 0, "ij": true, "assiette_pas": eur2(dmois.assiette_pas).to_string(),
        }));
    }
    out
}

// ------------------------------------------------------------------ le net reconstitue

/// Le nombre de calculs de la paie qu'une reconstitution s'autorise.
const ESSAIS: usize = 40;

/// UN MAINTIEN GARANTI EN NET, reconstitue par le calcul de la paie : le montant de la ligne du maintien pour lequel ce
/// que le salarie recoit (le net a payer, plus les indemnites qu'il percoit hors du bulletin) egale la garantie, au
/// centime. La garantie : le net du mois sans l'absence (`net0`), moins, pour chaque suite de jours, la part du net que
/// l'absence retire au prorata de sa retenue, fois ce que le texte n'en garantit pas ; les indemnites nettes des jours
/// non garantis s'y ajoutent (3090, 11.2 : « 90 % de la remuneration nette que le salarie aurait percue s'il avait
/// continue de travailler », « deduction faite des indemnites que l'interesse percoit de la securite sociale »). Un texte
/// en brut qui plafonne au net (« un montant total superieur a la remuneration nette qu'il aurait effectivement percue »)
/// baisse son complement jusqu'a `net0`. La loi, en brut, reste le plancher (la plus favorable). `paie(lignes)` est le
/// calcul de la paie du mois, le seul : rien ne se calcule ici a cote de lui.
pub fn au_net(
    lignes: &[Value],
    paie: &mut dyn FnMut(Vec<Value>) -> Resultat<crate::calcul::Paie>,
) -> Resultat<Option<(Decimal, Avis)>> {
    let Some(i) = lignes.iter().position(|l| l.get("maintien").is_some_and(Value::is_object)) else { return Ok(None) };
    let spec = lignes[i]["maintien"].clone();
    let n = |k: &str| dec_avec_echelle(&spec[k]);
    let (plancher, direct, complement_brut) = (n("plancher")?, n("complement_direct")?, n("complement_brut")?);
    let (hors_bulletin, hors_garantie) = (n("ij_nettes_hors_bulletin")?, n("ij_nettes_hors_garantie")?);
    let parts: Vec<(Decimal, Decimal)> = spec["parts"]
        .as_array()
        .map_or(&[][..], |a| &a[..])
        .iter()
        .map(|p| Ok((dec_avec_echelle(&p[0])?, dec_avec_echelle(&p[1])?)))
        .collect::<Resultat<_>>()?;
    let avec = |montant: Option<Decimal>| -> Vec<Value> {
        let mut out = Vec::new();
        for (k, l) in lignes.iter().enumerate() {
            if k == i {
                if let Some(m) = montant {
                    out.extend(avec_le_montant(std::slice::from_ref(l), m));
                }
                continue;
            }
            if montant.is_none() && g(l, "ij").as_bool() == Some(true) {
                continue;
            }
            out.push(l.clone());
        }
        out
    };
    let sans_absence: Vec<Value> = avec(None).into_iter().filter(|l| g(l, "arret").as_bool() != Some(true)).collect();
    let net0 = paie(sans_absence)?.net_a_payer;
    let net_abs = paie(avec(None))?.net_a_payer;
    let retire: Decimal = parts.iter().map(|p| p.0).sum();
    let perdu = net0 - net_abs;
    let cible = if spec["net"].as_bool() == Some(true) {
        let mut c = net_abs + hors_garantie;
        if retire > Decimal::ZERO {
            for (x, f) in &parts {
                c += mul(div(mul(perdu, *x), retire), *f);
            }
        }
        Some(eur2(c))
    } else {
        None
    };
    let mut calculs = 2usize;
    let mut recu = |a: Decimal, calculs: &mut usize| -> Resultat<Decimal> {
        *calculs += 1;
        Ok(paie(avec(Some(a)))?.net_a_payer + hors_bulletin)
    };
    let resoudre = |cible: Decimal, recu: &mut dyn FnMut(Decimal, &mut usize) -> Resultat<Decimal>, calculs: &mut usize| -> Resultat<Option<Decimal>> {
        let (mut lo, mut flo) = (plancher, recu(plancher, calculs)? - cible);
        if flo >= Decimal::new(-1, 2) {
            return Ok(Some(plancher));
        }
        let mut hi = plancher + (cible - (flo + cible)).max(Decimal::ONE) * Decimal::from(2);
        let mut fhi = recu(hi, calculs)? - cible;
        while fhi < Decimal::ZERO && *calculs < ESSAIS {
            lo = hi;
            flo = fhi;
            hi = hi * Decimal::from(2);
            fhi = recu(hi, calculs)? - cible;
        }
        while *calculs < ESSAIS {
            let a = if fhi != flo { eur2(lo - div(mul(flo, hi - lo), fhi - flo)) } else { eur2(div(lo + hi, Decimal::TWO)) };
            let a = if a <= lo || a >= hi { eur2(div(lo + hi, Decimal::TWO)) } else { a };
            let fa = recu(a, calculs)? - cible;
            if fa.abs() <= Decimal::new(1, 2) {
                return Ok(Some(a));
            }
            if fa < Decimal::ZERO {
                lo = a;
                flo = fa;
            } else {
                hi = a;
                fhi = fa;
            }
            if hi - lo <= Decimal::new(1, 2) {
                return Ok(Some(if fhi.abs() <= flo.abs() { hi } else { lo }));
            }
        }
        Ok(None)
    };
    let texte = en_texte(&spec["texte"]);
    let mois = en_texte(&spec["mois"]);
    let echec = || {
        format!(
            "Le maintien de {} ({}) ne se reconstitue pas au centime en {} calculs de la paie : la garantie en net n'est pas \
atteinte. Rien n'est calculé ; signaler ce mois au développeur.",
            socle::mois::Mois::en_clair(&mois),
            texte,
            ESSAIS
        )
    };
    let (convention, phrase) = match cible {
        Some(c) => {
            let a = resoudre(c, &mut recu, &mut calculs)?.ok_or_else(echec)?;
            (a, format!("garantit {} net ce mois", socle::texte::euros(c)))
        }
        None => {
            let a = complement_brut + plancher;
            let r = recu(a, &mut calculs)?;
            if r > net0 {
                let a = resoudre(net0, &mut recu, &mut calculs)?.ok_or_else(echec)?;
                (a, format!("ne dépasse pas le net qu'il aurait perçu, {}", socle::texte::euros(net0)))
            } else {
                (a, format!("garantit {} brut, sous le net qu'il aurait perçu", socle::texte::euros(complement_brut)))
            }
        }
    };
    let loi = direct + plancher;
    let retenu = convention.max(loi);
    let avis = info(format!(
        "Maintien de {} : {} {} ; reconstitué en {} calculs de la paie, au centime{}.",
        socle::mois::Mois::en_clair(&mois),
        texte,
        phrase,
        calculs,
        if retenu > convention { " ; la loi donne plus, c'est elle qui est payée" } else { "" }
    ));
    Ok(Some((retenu, avis)))
}

/// Les lignes d'un engagement, la ligne du maintien portee a `montant` (le resultat de `au_net`).
pub fn avec_le_montant(lignes: &[Value], montant: Decimal) -> Vec<Value> {
    lignes
        .iter()
        .map(|l| {
            if l.get("maintien").is_some_and(Value::is_object) {
                let q = dec_avec_echelle(g(l, "quantite")).unwrap_or(Decimal::ONE);
                let q = if q > Decimal::ZERO { q } else { Decimal::ONE };
                json!({"rubrique": l["rubrique"], "quantite": flot(q), "taux": div(montant, q).normalize().to_string(), "heures": 0})
            } else {
                l.clone()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CSS R323-5 : « la moitié du revenu d'activité antérieur » ; R323-4, 1° : « 1/91,25 du montant des trois dernières
    /// paies », chacune « dans la limite d'un plafond égal à 1,4 fois le salaire minimum de croissance [...] calculé pour un
    /// mois » ; net de la CSG à 6,2 % (L136-8, II, 1°, b) et de la CRDS à 0,5 % (ordonnance n° 96-50, art. 14, I, et 19)
    /// (`atelier/reference/sources/code-de-la-securite-sociale/articles-20261103.md`, `ordonnance-96-50-crds/`).
    /// L'accident du travail : R433-1 et R433-3, 60 % puis 80 % du salaire journalier ; R433-4, 1°, « 1/30,42 du montant
    /// de la paye du mois civil antérieur », l'indemnité au plus le gain journalier net du taux forfaitaire de R331-5, fixé
    /// « à 21 % » (`arrete-2013-taux-forfaitaire-r331-5/`) ; R433-2, le salaire journalier pris au plus à 0,226 % du
    /// plafond annuel, soit 65,17 et 86,90 € au plus en 2026 (`accident-du-travail/ameli-reglementation-atmp-1er-novembre-2026-20261030.md`).
    #[test]
    fn l_indemnite_de_principe_est_la_fraction_du_salaire_journalier_plafonne() {
        let regles = droit::permanents::regles().unwrap();
        let p = &regles["indemnites_journalieres"]["principe"];
        let smic = Decimal::new(1202, 2);
        // 2 487,39 par mois : sous le plafond (1,4 × 12,02 × 151,67 = 2 552,30) ; 7 462,17 / 91,25 / 2.
        let sous = montant_de_principe(&[Decimal::new(248739, 2); 3], smic, p).unwrap();
        assert_eq!((sous, net_de_csg_crds(sous, p).unwrap()), (Decimal::new(4089, 2), Decimal::new(3815, 2)));
        // 4 000 par mois : chaque paie retenue au plafond.
        assert_eq!(montant_de_principe(&[Decimal::from(4000); 3], smic, p).unwrap(), Decimal::new(4196, 2));
        let a = &regles["indemnites_journalieres"]["principe_accident"];
        let pass = Decimal::from(48060);
        // 2 487,39 : 81,77 par jour ; 60 % = 49,06 ; 80 % = 65,41, ramené au gain net de 79 % = 64,60.
        assert_eq!(montants_d_accident(Decimal::new(248739, 2), pass, a).unwrap(), (Decimal::new(4906, 2), Decimal::new(6460, 2)));
        // 5 000 : le salaire journalier pris au plafond, 108,62.
        assert_eq!(montants_d_accident(Decimal::from(5000), pass, a).unwrap(), (Decimal::new(6517, 2), Decimal::new(8690, 2)));
    }

    /// « le plus favorable » : la loi (L1226-1, D1226-1) et la convention s'appliquent ensemble, le mois paie le plus haut
    /// des deux complements ; subrogee, la compagnie y ajoute les indemnites brutes qu'elle avance et les retire aussitot.
    #[test]
    fn le_mois_paie_le_plus_favorable_de_la_loi_et_de_la_convention() {
        let montant = |loi: i64, conv: i64, ij: i64| {
            let m = DuMois {
                loi: Decimal::from(loi),
                convention_brut: Some(Decimal::from(conv)),
                ij_subrogees_brutes: Decimal::from(ij),
                ij_subrogees_jours: if ij > 0 { 10 } else { 0 },
                ..DuMois::default()
            };
            let l = lignes(&m, "2026-03");
            dec_avec_echelle(&l[0]["taux"]).unwrap()
        };
        assert_eq!(montant(500, 300, 0), Decimal::from(500));
        assert_eq!(montant(300, 500, 0), Decimal::from(500));
        assert_eq!(montant(300, 500, 200), Decimal::from(700));
    }

    /// CSS R323-1, 1° : « le point de depart de l'indemnite journaliere [...] est le quatrieme jour de l'incapacite de
    /// travail » (`atelier/reference/sources/code-de-la-securite-sociale/articles-20261102.md`), et l'incapacite commence au
    /// jour que l'avis prescrit : travaille jusqu'au vendredi 06/03, arrete a partir du lundi 09/03, indemnise des le
    /// 12/03. Un accident du travail l'est des le premier jour qui suit l'arret du travail (L433-1).
    #[test]
    fn la_caisse_indemnise_des_le_quatrieme_jour_de_l_incapacite_prescrite() {
        let imp = json!({"genre": "arret", "dernier_jour_travaille": "2026-03-06", "debut": "2026-03-09",
            "fin_previsionnelle": "2026-03-20", "reprise": null, "ij_brute": "40.89", "ij_nette": "38.15"});
        let (premier, dernier) = (crate::imprevus::premier_jour(&imp).unwrap(), crate::imprevus::dernier_jour(&imp).unwrap());
        let premier_indemnise = |o: Origine| {
            let ij = indemnites_des_jours(&imp, o, &premier, &dernier, &premier, &dernier).unwrap().unwrap();
            premier.plus_jours(ij.iter().position(Option::is_some).unwrap() as i64).unwrap().iso()
        };
        assert_eq!(premier_indemnise(Origine::Maladie), "2026-03-12");
        assert_eq!(premier_indemnise(Origine::AccidentDuTravail), "2026-03-09");
    }
}
