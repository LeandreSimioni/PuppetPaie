//! VISER UN MONTANT : le taux qu'il faudrait pour que des engagements de la saisie coutent, versent ou paient brut un
//! montant donne. La recherche refait le calcul de la saisie pour l'ecran (`pour_l_ecran`) a chaque essai, les salaires
//! des engagements vises multiplies par un meme facteur ; rien ne s'ecrit.

use socle::forme::NombreLu;

use crate::{Chiffre, LignePaie};

use crate::calcul::Paie;
use crate::saisie::{Carte, VueDuMois};
use droit::contexte::Contexte;
use socle::num::Decimal;

/// Ce qui se vise : le brut, le cout employeur, le net a payer ; la charge du montant vise le dit `brut`, `cout`, `net`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Cible {
    Brut,
    Cout,
    Net,
}

/// Pourquoi la recherche n'aboutit pas : un refus de la recherche elle-meme, ou le calcul de la saisie qui refuse.
#[derive(Debug, Clone)]
pub enum RefusVisee {
    Refus(String),
    Calcul(String),
}

/// Le meilleur essai : la saisie aux taux trouves, ses paies, le montant atteint, celui du depart, le facteur applique
/// aux salaires et le nombre d'essais.
pub struct Visee {
    pub saisie: VueDuMois,
    pub paies: Vec<Paie>,
    pub atteint: Decimal,
    pub depart: Decimal,
    pub facteur: Decimal,
    pub tours: usize,
}

type R<T> = Result<T, RefusVisee>;

fn refus(m: impl Into<String>) -> RefusVisee {
    RefusVisee::Refus(m.into())
}

const TOURS: usize = 12;

/// Au centime.
fn centimes(x: Decimal) -> Decimal {
    let mut r = x.round_dp(2);
    r.rescale(2);
    r
}

/// Un taux ou un total en nombre.
fn flottant(x: Decimal) -> Option<Chiffre> {
    NombreLu::de_f64(socle::num::flottant(x)).map(Chiffre::Nombre)
}

/// La rubrique de la ligne ; une rubrique que le catalogue ne connait pas se refuse.
fn rubrique_de(ctx: &Contexte, ligne: &LignePaie) -> R<droit::catalogue::Rubrique> {
    droit::catalogue::rubrique(&ctx.racine, ligne.rubrique(), &ctx.lue_au).map_err(refus)
}

/// Une ligne dont le taux est une case : en unites (un salaire, une absence, une indemnite), ou de frais.
fn taux_saisi(l: &LignePaie) -> bool {
    matches!(l, LignePaie::Unites(_) | LignePaie::UnitesEtHeures(_) | LignePaie::Frais(_))
}

/// Un taux, une quantite absents ou nuls.
fn absent_ou_zero(v: Option<Decimal>) -> bool {
    v.is_none_or(|x| x.is_zero())
}

/// Un SALAIRE se hausse ; un defraiement, une prime, un acompte, non.
fn ligne_ajustable(ctx: &Contexte, ligne: &LignePaie) -> R<bool> {
    Ok(taux_saisi(ligne) && rubrique_de(ctx, ligne)?.nature == "salaire" && !absent_ou_zero(ligne.taux()))
}

/// Une ligne dont le taux est une case vide.
fn taux_vide(l: &LignePaie) -> bool {
    taux_saisi(l) && l.taux().is_none()
}

/// L'engagement de rang `i` de la saisie.
fn engagement_de(data: &mut VueDuMois, i: usize) -> R<&mut Carte> {
    data.engagements.get_mut(i).ok_or_else(|| refus(format!("La saisie n'a pas d'engagement de rang {}.", i + 1)))
}

/// Une ligne de salaire SANS TAUX recoit un taux provisoire de 1 EUR.
fn poser_taux_provisoires(ctx: &Contexte, data: &mut VueDuMois, rangs: &[usize]) -> R<()> {
    for &i in rangs {
        let e = engagement_de(data, i)?;
        let lignes = e.lignes().to_vec();
        let mut ajustable = false;
        for l in &lignes {
            ajustable |= ligne_ajustable(ctx, l)?;
        }
        if e.salaire_mensuel.is_some() || ajustable {
            continue;
        }
        let mut sans_taux: Vec<(usize, String)> = Vec::new();
        for (k, ligne) in lignes.iter().enumerate() {
            if !taux_saisi(ligne) {
                continue;
            }
            let r = rubrique_de(ctx, ligne)?;
            if r.nature == "salaire" && absent_ou_zero(ligne.taux()) && !absent_ou_zero(ligne.quantite()) {
                sans_taux.push((k, r.libelle));
            }
        }
        if sans_taux.len() > 1 {
            let qui = match e.salarie_dit() {
                Some(s) => s.to_string(),
                None => "cet engagement".to_string(),
            };
            let noms: Vec<&str> = sans_taux.iter().map(|(_, l)| l.as_str()).collect();
            return Err(refus(format!(
                "{} : {} lignes de salaire sans taux ({}). Le montant vise ne dit pas comment le partager entre elles : saisir un taux \
                 sur chacune, meme approximatif, et l'ajustement gardera leurs proportions.",
                qui,
                sans_taux.len(),
                noms.join(", ")
            )));
        }
        if let Some(l) = sans_taux.first().and_then(|(k, _)| e.lignes.as_mut().and_then(|ls| ls.get_mut(*k))) {
            l.poser_taux(Decimal::ONE).map_err(refus)?;
        }
    }
    // Les AUTRES blocs du mois repartent comme la page les enverrait d'ordinaire : sans leurs lignes incompletes.
    for (i, e) in data.engagements.iter_mut().enumerate() {
        let Some(ls) = e.lignes.as_mut() else { continue };
        if rangs.contains(&i) {
            for l in ls.iter_mut().filter(|l| taux_vide(l)) {
                l.poser_taux(Decimal::ZERO).map_err(refus)?;
            }
        } else {
            ls.retain(|l| !taux_vide(l));
        }
    }
    Ok(())
}

/// Un montant multiplie par `facteur`, au centime, en flottant.
fn mis_a_l_echelle(x: &Chiffre, facteur: Decimal) -> R<Option<Chiffre>> {
    let a = x.dec().map_err(refus)?;
    let p = a.checked_mul(facteur).ok_or_else(|| refus("Montant trop grand pour l'ajustement."))?;
    Ok(flottant(centimes(p)))
}

/// L'engagement recopie, ses salaires multiplies -- et rien d'autre. Rend aussi s'il y avait quelque chose a multiplier.
fn a_l_echelle(ctx: &Contexte, e: &mut Carte, facteur: Decimal) -> R<bool> {
    let mut touche = false;
    if let Some(s) = &e.salaire_mensuel {
        e.salaire_mensuel = match mis_a_l_echelle(&Chiffre::Nombre(s.clone()), facteur)? {
            Some(Chiffre::Nombre(n)) => Some(n),
            _ => return Err(refus("Montant trop grand pour l'ajustement.")),
        };
        touche = true;
    }
    for l in e.lignes.iter_mut().flatten() {
        if !ligne_ajustable(ctx, l)? {
            continue;
        }
        if let Some(t) = l.taux() {
            // Le taux au centime, sous sa forme flottante la plus courte (`mis_a_l_echelle`).
            let x = mis_a_l_echelle(&Chiffre::de_decimal(t), facteur)?.ok_or_else(|| refus("Montant trop grand pour l'ajustement."))?;
            l.poser_taux(x.dec().map_err(refus)?).map_err(refus)?;
        }
        touche = true;
    }
    Ok(touche)
}

fn somme(paies: &[Paie], rangs: &[usize], cible: Cible) -> R<Decimal> {
    let mut total = Decimal::new(0, 2);
    for &i in rangs {
        let p = paies.get(i).ok_or_else(|| refus("le calcul du mois n'a pas cette paie"))?;
        total += match cible {
            Cible::Net => p.net_a_payer,
            Cible::Brut => p.brut,
            Cible::Cout => p.cout_employeur,
        };
    }
    Ok(total)
}

/// Les salaires des engagements de rangs `rangs` (tries, sans doublon, presents dans la saisie `data`) qui font
/// atteindre `vise` a leur `cible` additionnee. Une ligne de salaire sans taux en recoit un provisoire ; le facteur se
/// cherche par approches successives (au plus `TOURS` essais) et le MEILLEUR essai gagne, jamais le dernier : a egale
/// distance, celui qui ne depasse pas.
pub fn viser(ctx: &Contexte, data: &VueDuMois, rangs: &[usize], cible: Cible, vise: Decimal) -> R<Visee> {
    let mut data = data.clone();
    // Une ligne au tarif de la convention a son taux : elle n'est pas une ligne sans taux.
    crate::saisie_au_tarif(ctx, &mut data).map_err(RefusVisee::Calcul)?;
    poser_taux_provisoires(ctx, &mut data, rangs)?;
    let essai = |facteur: Decimal| -> R<(VueDuMois, Vec<Paie>, Decimal)> {
        let mut d = data.clone();
        let mut touche = false;
        for &i in rangs {
            touche |= a_l_echelle(ctx, engagement_de(&mut d, i)?, facteur)?;
        }
        if !touche {
            return Err(refus(
                "Rien a ajuster : cet engagement ne porte aucune ligne de salaire avec une quantite. Un defraiement, une prime ou un \
                 acompte ne se recalcule pas depuis une enveloppe -- c'est un montant, pas un taux. Poser d'abord une ligne de paie \
                 (cachet, service, heure, journee...) avec sa quantite ; le taux, lui, peut rester vide.",
            ));
        }
        let paies = crate::charger_donnees(ctx, &d).map_err(RefusVisee::Calcul)?.paies;
        let atteint = somme(&paies, rangs, cible)?;
        Ok((d, paies, atteint))
    };

    let (_, _, depart) = essai(Decimal::ONE)?;
    if depart <= Decimal::ZERO {
        return Err(refus(format!(
            "Cet engagement paie {} : il n'y a pas de taux à faire varier. Poser d'abord une quantité et un taux, puis viser le total.",
            socle::texte::euros(depart)
        )));
    }
    let rang_essai = |atteint: Decimal| ((atteint - vise).abs(), atteint > vise);
    let mut facteur = (vise / depart).round_dp(20);
    let mut vus = vec![facteur];
    let mut meilleur: Option<Visee> = None;
    for tour in 1..=TOURS {
        let (d, paies, atteint) = essai(facteur)?;
        if meilleur.as_ref().is_none_or(|m| rang_essai(atteint) < rang_essai(m.atteint)) {
            meilleur = Some(Visee { saisie: d, paies, atteint, depart, facteur, tours: tour });
        }
        if atteint == vise || atteint <= Decimal::ZERO {
            break;
        }
        let suivant = facteur.checked_mul(vise).and_then(|x| x.checked_div(atteint)).ok_or_else(|| refus("Ajustement impossible : montants trop grands."))?.round_dp(20);
        if vus.contains(&suivant) {
            break;
        }
        vus.push(suivant);
        facteur = suivant;
    }
    meilleur.ok_or_else(|| refus("aucun essai"))
}
