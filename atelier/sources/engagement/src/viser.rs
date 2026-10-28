//! VISER UN MONTANT : le taux qu'il faudrait pour que des engagements de la saisie coutent, versent ou paient brut un
//! montant donne. La recherche refait le calcul de la saisie pour l'ecran (`pour_l_ecran`) a chaque essai, les salaires
//! des engagements vises multiplies par un meme facteur ; rien ne s'ecrit.

use serde_json::{json, Value};

use crate::calcul::Paie;
use droit::contexte::Contexte;
use socle::fichiers::vrai;
use socle::num::{dec, Decimal};
use socle::valeur::valeur_en_texte;

/// Ce qui se vise : le brut, le cout employeur, le net a payer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub saisie: Value,
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

/// Un taux ou un total en nombre JSON.
fn flottant(x: Decimal) -> Value {
    x.to_string().parse::<f64>().ok().and_then(serde_json::Number::from_f64).map_or(Value::Null, Value::Number)
}

/// Une valeur saisie : ni absente, ni `null`, ni vide.
fn rempli(v: Option<&Value>) -> bool {
    !matches!(v, None | Some(Value::Null)) && v.and_then(Value::as_str) != Some("")
}

/// Une valeur absente, vide ou nulle.
fn absent_ou_zero(v: Option<&Value>) -> bool {
    !rempli(v) || v.is_some_and(|x| socle::valeur::nombre(x) == Some(0.0))
}

fn rubrique_de(ctx: &Contexte, ligne: &Value) -> Option<droit::catalogue::Rubrique> {
    let code = match ligne.get("rubrique") {
        None => "cachet",
        Some(Value::String(s)) => s.as_str(),
        Some(_) => return None,
    };
    droit::catalogue::rubrique(&ctx.racine, code, &ctx.lue_au).ok()
}

/// Un SALAIRE se hausse ; un defraiement, une prime, un acompte, non.
fn ligne_ajustable(ctx: &Contexte, ligne: &Value) -> bool {
    rubrique_de(ctx, ligne).is_some_and(|r| r.nature == "salaire" && r.saisie.iter().any(|s| s == "taux") && !absent_ou_zero(ligne.get("taux")))
}

fn lignes_de(e: &Value) -> Vec<Value> {
    match e.get("lignes") {
        Some(Value::Array(a)) => a.clone(),
        _ => Vec::new(),
    }
}

fn taux_vide(l: &Value) -> bool {
    matches!(l.get("taux"), Some(Value::Null)) || l.get("taux").and_then(Value::as_str) == Some("")
}

/// Une ligne de salaire SANS TAUX recoit un taux provisoire de 1 EUR.
fn poser_taux_provisoires(ctx: &Contexte, data: &mut Value, rangs: &[usize]) -> R<()> {
    for &i in rangs {
        let e = &mut data["engagements"][i];
        let lignes = lignes_de(e);
        if rempli(e.get("salaire_mensuel")) || lignes.iter().any(|l| ligne_ajustable(ctx, l)) {
            continue;
        }
        let mut sans_taux: Vec<(usize, String)> = Vec::new();
        for (k, ligne) in lignes.iter().enumerate() {
            let Some(r) = rubrique_de(ctx, ligne) else { continue };
            if r.nature == "salaire" && r.saisie.iter().any(|s| s == "taux") && absent_ou_zero(ligne.get("taux")) && rempli(ligne.get("quantite")) && !absent_ou_zero(ligne.get("quantite")) {
                sans_taux.push((k, r.libelle));
            }
        }
        if sans_taux.len() > 1 {
            let qui = e.get("salarie").filter(|v| vrai(v)).map_or_else(|| "cet engagement".to_string(), valeur_en_texte);
            let noms: Vec<&str> = sans_taux.iter().map(|(_, l)| l.as_str()).collect();
            return Err(refus(format!(
                "{} : {} lignes de salaire sans taux ({}). Le montant vise ne dit pas comment le partager entre elles : saisir un taux \
                 sur chacune, meme approximatif, et l'ajustement gardera leurs proportions.",
                qui,
                sans_taux.len(),
                noms.join(", ")
            )));
        }
        if let Some((k, _)) = sans_taux.first() {
            e["lignes"][*k]["taux"] = json!(1.0);
        }
    }
    // Les AUTRES blocs du mois repartent comme la page les enverrait d'ordinaire : sans leurs lignes incompletes.
    let Some(Value::Array(tous)) = data.get_mut("engagements") else { return Ok(()) };
    for (i, e) in tous.iter_mut().enumerate() {
        let lignes = lignes_de(e);
        if rangs.contains(&i) {
            if let Some(Value::Array(ls)) = e.get_mut("lignes") {
                for l in ls.iter_mut().filter(|l| taux_vide(l)) {
                    l["taux"] = json!(0);
                }
            }
        } else if lignes.iter().any(taux_vide) {
            e["lignes"] = Value::Array(lignes.into_iter().filter(|l| !taux_vide(l)).collect());
        }
    }
    Ok(())
}

/// Un montant multiplie par `facteur`, au centime, en flottant.
fn mis_a_l_echelle(x: &Value, facteur: Decimal) -> R<Value> {
    let a = dec(x).map_err(refus)?;
    let p = a.checked_mul(facteur).ok_or_else(|| refus("Montant trop grand pour l'ajustement."))?;
    Ok(flottant(centimes(p)))
}

/// L'engagement recopie, ses salaires multiplies -- et rien d'autre. Rend aussi s'il y avait quelque chose a multiplier.
fn a_l_echelle(ctx: &Contexte, e: &Value, facteur: Decimal) -> R<(Value, bool)> {
    let mut out = e.clone();
    let mut touche = false;
    if rempli(out.get("salaire_mensuel")) {
        out["salaire_mensuel"] = mis_a_l_echelle(&out["salaire_mensuel"], facteur)?;
        touche = true;
    }
    if let Some(Value::Array(lignes)) = out.get_mut("lignes") {
        for l in lignes.iter_mut() {
            if !ligne_ajustable(ctx, l) {
                continue;
            }
            l["taux"] = mis_a_l_echelle(&l["taux"], facteur)?;
            touche = true;
        }
    }
    Ok((out, touche))
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
pub fn viser(ctx: &Contexte, data: &Value, rangs: &[usize], cible: Cible, vise: Decimal) -> R<Visee> {
    let mut data = data.clone();
    poser_taux_provisoires(ctx, &mut data, rangs)?;
    let essai = |facteur: Decimal| -> R<(Value, Vec<Paie>, Decimal)> {
        let mut d = data.clone();
        let mut touche = false;
        for &i in rangs {
            let (e, t) = a_l_echelle(ctx, &d["engagements"][i], facteur)?;
            d["engagements"][i] = e;
            touche |= t;
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
            "Cet engagement paie {} EUR : il n'y a pas de taux a faire varier. Poser d'abord une quantite et un taux, puis viser le total.",
            centimes(depart)
        )));
    }
    let rang_essai = |atteint: Decimal| ((atteint - vise).abs(), atteint > vise);
    let mut facteur = (vise / depart).round_dp(20);
    let mut vus = vec![facteur];
    let mut meilleur: Option<Visee> = None;
    for tour in 1..=TOURS {
        let (d, paies, atteint) = essai(facteur)?;
        if meilleur.as_ref().map_or(true, |m| rang_essai(atteint) < rang_essai(m.atteint)) {
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
