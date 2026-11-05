//! UNE PAIE ECRITE SE RELIT, ELLE NE SE RECALCULE PAS. La validation ecrit chaque paie d'un engagement (`etats`) sous
//! sa forme canonique (`calcul::Paie::canon`, ses avertissements et les lignes de l'engagement) ; ce qui fabrique un
//! document la relit ici, telle qu'elle a ete calculee : l'engagement de la saisie du mois, la fiche et le spectacle
//! comme le calcul les lit (`calcul::fiche_de_l_engagement`), les montants ecrits, et les periodes elementaires d'un
//! mois decoupe.

use std::cell::Cell;

use serde_json::{json, Value};

use crate::calcul::{fiche_de_l_engagement, DeductionForfaitaire, LigneBrut, LigneCotisation, Paie, PlafondTempsPartiel, ReductionGenerale};
use crate::{Avis, Engagement, LignePaie};
use droit::contexte::Contexte;
use socle::date::{en_texte, Jour};
use socle::fichiers::vrai;
use socle::num::{dec, flottant, Decimal, Resultat};

/// La forme ecrite d'une paie : sa forme canonique, ses avertissements (`{"texte", "info"}`) et les lignes de son
/// engagement telles quelles (celles d'un permanent, deduites).
pub fn ecrire(ctx: &Contexte, p: &Paie) -> Resultat<Value> {
    let mut v = p.canon(ctx)?;
    v["avertissements"] = avis(&p.avertissements);
    v["engagement"]["lignes"] = socle::ecriture::en_valeur(&p.engagement.lignes);
    v[PLAFOND_TEMPS_PARTIEL] = json!(p.plafond_temps_partiel.code());
    if let Some(Value::Array(periodes)) = v.get_mut("periodes") {
        for (x, q) in periodes.iter_mut().zip(&p.periodes) {
            x["avertissements"] = avis(&q.avertissements);
            x["engagement"]["lignes"] = socle::ecriture::en_valeur(&q.engagement.lignes);
            x[PLAFOND_TEMPS_PARTIEL] = json!(q.plafond_temps_partiel.code());
        }
    }
    Ok(v)
}

/// Dans une paie ecrite : le plafond d'un salarie a temps partiel tel que le calcul l'a applique
/// (`calcul::PlafondTempsPartiel`), que la DSN declare en S21.G00.40.084.
pub const PLAFOND_TEMPS_PARTIEL: &str = "plafond_temps_partiel";

/// LES LIGNES DE PAIE QU'UNE PAIE ECRITE GARDE (`ecrire`), lues par leur forme (`LignePaie`).
pub fn lignes_ecrites(v: &Value) -> Resultat<Vec<LignePaie>> {
    socle::forme::lire(v, "Les lignes de paie d'une paie écrite ne se lisent pas : valider à nouveau l'engagement.")
}

fn avis(v: &[Avis]) -> Value {
    Value::Array(v.iter().map(|a| json!({ "texte": a.texte, "info": a.info })).collect())
}

fn entier_de(v: &Value) -> Option<i64> {
    match v {
        Value::String(s) => s.trim().parse().ok(),
        Value::Number(n) => n.as_i64(),
        _ => None,
    }
}

fn decimal_de(v: &Value, cle: &str) -> Resultat<Decimal> {
    dec(v.get(cle).ok_or_else(|| socle::echec::cle(socle::echec::PAIE_ILLISIBLE, cle))?)
}

fn decimal_opt(v: Option<&Value>) -> Resultat<Option<f64>> {
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(x) => flottant(x).map(Some),
    }
}

fn texte_opt(v: &Value, cle: &str) -> Option<String> {
    v.get(cle).filter(|x| !x.is_null()).map(en_texte)
}

fn texte_ou_vide(v: &Value, cle: &str) -> String {
    match v.get(cle) {
        Some(x) if vrai(x) => en_texte(x),
        _ => String::new(),
    }
}

fn drapeau(v: &Value, cle: &str) -> bool {
    v.get(cle).map_or(false, vrai)
}

/// Les engagements de la saisie `data`, construits sans rien chiffrer, puis ce que le calcul a pose dessus : le numero
/// (quand l'engagement n'en porte pas),
/// les jours du contrat, les suspensions, et, pour un permanent, les lignes et les jours que le moteur a deduits. `paies` : la forme
/// ecrite de chaque paie, dans l'ordre de la saisie.
pub fn engagements_ecrits(ctx: &Contexte, data: &crate::saisie::VueDuMois, paies: &[&Value]) -> Resultat<Vec<Engagement>> {
    let mensuels: Vec<usize> = data.engagements.iter().enumerate().filter(|(_, e)| e.mensuelle()).map(|(i, _)| i).collect();
    let appel = Cell::new(0usize);
    let lignes = |_: &Contexte, _: &crate::saisie::Carte, _: &str| -> Resultat<(Vec<LignePaie>, Vec<Avis>, Option<i64>)> {
        let rang = *mensuels.get(appel.get()).ok_or("rang d'engagement inconnu")?;
        appel.set(appel.get() + 1);
        let ecrit = paies.get(rang).and_then(|p| p.get("engagement"));
        let l = ecrit.and_then(|e| e.get("lignes")).ok_or_else(|| socle::echec::cle(socle::echec::PAIE_ILLISIBLE, "lignes"))?;
        let jours = ecrit.and_then(|e| e.get("jours")).and_then(|j| socle::date::en_texte(j).trim().parse::<i64>().ok());
        Ok((lignes_ecrites(l)?, Vec::new(), jours))
    };
    let (mut engs, _) = crate::engagements(ctx, data, &lignes)?;
    if engs.len() != paies.len() {
        return Err(socle::echec::cle(socle::echec::PAIE_ILLISIBLE, "paies"));
    }
    for (e, p) in engs.iter_mut().zip(paies) {
        let v = p.get("engagement").ok_or_else(|| socle::echec::cle(socle::echec::PAIE_ILLISIBLE, "engagement"))?;
        // Le numero est celui de l'engagement, pose quand le contrat part ; une paie validee avant ne le porte pas.
        if e.numero.is_none() {
            e.numero = crate::numeros::NumeroContrat::de_la_valeur(v.get("numero"))?;
        }
        e.jours_contrat = v.get("jours_contrat").and_then(entier_de).ok_or_else(|| socle::echec::cle(socle::echec::PAIE_ILLISIBLE, "jours_contrat"))?;
        e.suspensions = v.get("suspensions").and_then(Value::as_array).cloned().unwrap_or_default();
    }
    Ok(engs)
}

/// Une `Paie` du moteur, sur l'engagement `eng`, depuis sa forme ecrite `v`.
pub fn paie(ctx: &Contexte, eng: Engagement, v: &Value) -> Resultat<Paie> {
    paie_lue(ctx, eng, v, None, &[])
}

/// LES FICHES DU SALARIE TELLES QUE LE CALCUL D'UNE PAIE LES A LUES : celle de la paie, et celle de chaque periode
/// elementaire d'un mois decoupe, sans les taux recus (`droit::pas::sans_taux_recus` : le taux de la paie est son `pas`).
/// Elles s'ecrivent a cote de la paie, et ce qui relit ce qui a ete declare les relit (`lire`).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FichesDuCalcul {
    pub paie: droit::fiche::SalarieDuContrat,
    pub periodes: Vec<droit::fiche::SalarieDuContrat>,
}

impl FichesDuCalcul {
    /// Celles que le calcul de `p` a lues.
    pub fn de(p: &Paie) -> FichesDuCalcul {
        let sans = |s: &droit::fiche::SalarieDuContrat| {
            let mut s = s.clone();
            s.fiche.champs = droit::pas::sans_taux_recus(&s.fiche.champs);
            s
        };
        FichesDuCalcul { paie: sans(&p.salarie), periodes: p.periodes.iter().map(|q| sans(&q.salarie)).collect() }
    }

    /// Relues d'a cote de la paie, la seule lecture.
    pub fn lire(v: &Value) -> Resultat<FichesDuCalcul> {
        socle::forme::lire(v, "Les fiches lues par la paie ne se lisent pas : valider à nouveau l'engagement.")
    }
}

/// Une `Paie` du moteur, sur l'engagement `eng`, depuis sa forme ecrite `v`, sur les fiches du salarie telles que son
/// calcul les a lues plutot que sur la fiche d'aujourd'hui.
pub fn paie_sur_ses_fiches(ctx: &Contexte, eng: Engagement, v: &Value, fiches: &FichesDuCalcul) -> Resultat<Paie> {
    paie_lue(ctx, eng, v, Some(&fiches.paie), &fiches.periodes)
}

fn paie_lue(ctx: &Contexte, eng: Engagement, v: &Value, fiche: Option<&droit::fiche::SalarieDuContrat>, periodes: &[droit::fiche::SalarieDuContrat]) -> Resultat<Paie> {
    let sal = match fiche {
        Some(f) => f.clone(),
        None => fiche_de_l_engagement(ctx, &eng)?,
    };
    let nat = eng.nat()?;
    let spectacle = if nat.mensuelle() {
        Value::Null
    } else {
        crate::calcul::spectacle_de_la_paie(&ctx.spectacle(eng.spectacle.as_deref().ok_or_else(|| socle::fichiers::absent("spectacle"))?)?)
    };
    let smic = flottant(v.get("smic").ok_or_else(|| socle::echec::cle(socle::echec::PAIE_ILLISIBLE, "smic"))?)?;
    let tp = PlafondTempsPartiel::lire(v.get(PLAFOND_TEMPS_PARTIEL).and_then(Value::as_str).ok_or_else(|| socle::echec::cle(socle::echec::PAIE_ILLISIBLE, PLAFOND_TEMPS_PARTIEL))?)?;
    let mut paie = Paie::nouveau(eng.clone(), sal, spectacle, decimal_de(v, "brut")?, decimal_de(v, "base_csg")?, decimal_de(v, "plafond")?, smic, decimal_de(v, "heures")?, tp);
    let p = paie.ecrire();
    p.total_pat = decimal_de(v, "total_pat")?;
    p.total_sal = decimal_de(v, "total_sal")?;
    p.net_a_payer = decimal_de(v, "net_a_payer")?;
    p.net_social = decimal_de(v, "net_social")?;
    p.net_imposable = decimal_de(v, "net_imposable")?;
    p.cout_employeur = decimal_de(v, "cout_employeur")?;
    p.salaire_de_base = decimal_de(v, "salaire_de_base")?;
    p.salaire_retabli = decimal_de(v, "salaire_retabli")?;
    p.assiette_chomage = decimal_de(v, "assiette_chomage")?;
    p.heures_travaillees = decimal_de(v, "heures_travaillees")?;
    p.heures_absence = decimal_de(v, "heures_absence")?;
    p.numero_bulletin = texte_opt(v, "numero_bulletin");
    for l in v.get("lignes").and_then(Value::as_array).map_or(&[][..], |a| &a[..]) {
        p.lignes.push(LigneCotisation {
            id: texte_ou_vide(l, "id"),
            famille: texte_ou_vide(l, "famille"),
            libelle: texte_ou_vide(l, "libelle"),
            base: decimal_de(l, "base")?,
            taux_pat: decimal_opt(l.get("taux_pat"))?,
            montant_pat: decimal_de(l, "montant_pat")?,
            taux_sal: decimal_opt(l.get("taux_sal"))?,
            montant_sal: decimal_de(l, "montant_sal")?,
            masquer_base_et_taux: drapeau(l, "masquer_base_et_taux"),
        });
    }
    for l in v.get("remuneration").and_then(Value::as_array).map_or(&[][..], |a| &a[..]) {
        p.remuneration.push(LigneBrut {
            code: texte_ou_vide(l, "code"),
            libelle: texte_ou_vide(l, "libelle"),
            unite: texte_ou_vide(l, "unite"),
            quantite: decimal_de(l, "quantite")?,
            taux: decimal_de(l, "taux")?,
            montant: decimal_de(l, "montant")?,
            heures_annexes: decimal_de(l, "heures_annexes")?,
            soumis: drapeau(l, "soumis"),
            imposable: drapeau(l, "imposable"),
            conges_spectacles: drapeau(l, "conges_spectacles"),
            salaire_de_base: drapeau(l, "salaire_de_base"),
            retabli: drapeau(l, "retabli"),
            conges_pris: drapeau(l, "conges_pris"),
            dsn_bloc52: texte_opt(l, "dsn_bloc52"),
            arret_de_travail: drapeau(l, "arret_de_travail"),
            dsn_autres_bruts: texte_opt(l, "dsn_autres_bruts"),
            precision: texte_opt(l, "precision"),
        });
    }
    if let Some(Value::Object(b)) = v.get("bases") {
        for (k, x) in b {
            p.bases.insert(k.clone(), dec(x)?);
        }
    }
    if let Some(r) = v.get("reduction_generale").filter(|r| r.is_object()) {
        p.reduction_generale = Some(ReductionGenerale {
            coefficient: decimal_de(r, "coefficient")?,
            montant: decimal_de(r, "montant")?,
            smic_reference: decimal_de(r, "smic_reference")?,
            heures: decimal_de(r, "heures")?,
            urssaf: decimal_de(r, "urssaf")?,
            chomage: decimal_de(r, "chomage")?,
            agirc_arrco: decimal_de(r, "agirc_arrco")?,
        });
    }
    if let Some(d) = v.get("deduction_forfaitaire").filter(|d| d.is_object()) {
        p.deduction_forfaitaire = Some(DeductionForfaitaire {
            categorie: texte_ou_vide(d, "categorie"),
            profession: texte_ou_vide(d, "profession"),
            taux: decimal_de(d, "taux")?,
            rubriques: d.get("rubriques").and_then(Value::as_array).map_or_else(Vec::new, |a| a.iter().map(en_texte).collect()),
            montant: decimal_de(d, "montant")?,
            deja: decimal_de(d, "deja")?,
        });
    }
    p.avertissements = v
        .get("avertissements")
        .and_then(Value::as_array)
        .map_or_else(Vec::new, |a| a.iter().map(|x| Avis { info: drapeau(x, "info"), texte: texte_ou_vide(x, "texte") }).collect());
    // Les periodes elementaires d'un mois decoupe : chacune sur l'engagement borne a sa periode, aux termes qu'elle porte.
    for (n, x) in v.get("periodes").and_then(Value::as_array).map_or(&[][..], |a| &a[..]).iter().enumerate() {
        let e = x.get("engagement").ok_or_else(|| socle::echec::cle(socle::echec::PAIE_ILLISIBLE, "periodes.engagement"))?;
        let lire = |k: &str| e.get(k).filter(|j| !j.is_null()).map(|j| Jour::lire(&en_texte(j))).transpose();
        let (Some(debut), Some(fin)) = (lire("debut_periode")?, lire("fin_periode")?) else {
            return Err(socle::echec::cle(socle::echec::PAIE_ILLISIBLE, "periodes"));
        };
        let mut sub = eng.clone();
        sub.periode = Some((debut, fin));
        sub.lignes = e.get("lignes").map(lignes_ecrites).transpose()?.unwrap_or_default();
        sub.quotite_contrat = decimal_opt(e.get("quotite"))?;
        sub.heures = decimal_opt(e.get("heures"))?;
        p.periodes.push(paie_lue(ctx, sub, x, periodes.get(n), &[])?);
    }
    Ok(paie)
}

/// Les paies ecrites d'une saisie de mois, dans son ordre : `ecrites[i]` est la forme ecrite de l'engagement `i`.
pub fn paies(ctx: &Contexte, data: &crate::saisie::VueDuMois, ecrites: &[&Value]) -> Resultat<Vec<Paie>> {
    let engs = engagements_ecrits(ctx, data, ecrites)?;
    engs.into_iter().zip(ecrites).map(|(e, v)| paie(ctx, e, v)).collect()
}

/// La saisie `data` reduite a son engagement `i` : ce que la relecture d'une seule paie lit.
pub fn saisie_d_un_engagement(data: &crate::saisie::VueDuMois, i: usize) -> Resultat<crate::saisie::VueDuMois> {
    let e = data.engagements.get(i).cloned().ok_or_else(|| socle::echec::cle(socle::echec::PAIE_ILLISIBLE, "engagements"))?;
    Ok(crate::saisie::VueDuMois { mois: data.mois, taux_cachet: data.taux_cachet.clone(), engagements: vec![e] })
}
