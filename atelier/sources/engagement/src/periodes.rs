//! La paie d'un mois decoupee en periodes elementaires (`droit::periodes`, qui cite les textes) : chaque periode est une
//! paie calculee par `calcul::calculer_periode`, sur un engagement borne (`Engagement::periode`) ; elle lit sa fiche sur
//! ses bornes, sa pile de bareme (rien n'est empile ailleurs que dans `bareme_applique`), son plafond
//! (`plafond::plafond_permanent` : le prorata calendaire de R. 242-2, sur les bornes de la periode), son salaire (la part
//! du mois de ses heures, `droit::periodes::part_du_mois`). La paie du mois est leur somme (`assembler`).

use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::calcul::{ligne_de_reduction, pile_de_la_paie, LigneCotisation, Paie, ReductionGenerale};
use droit::reduction::{reduction_au_coefficient, reduction_generale};
use droit::contexte::Contexte;
use socle::date::{en_texte, Jour};
use crate::{Avis, Engagement};
use socle::fichiers::vrai;
use socle::num::{dec, eur2, Decimal, Resultat};


/// Le jour ou l'avenant de la saisie du mois decoupe la paie (`droit::periodes::jour_de_l_avenant`).
fn jour_de_l_avenant(eng: &Engagement) -> Resultat<Option<Jour>> {
    droit::periodes::jour_de_l_avenant(eng.avant.as_ref(), eng.date_modification.as_deref(), eng.debut_periode()?, eng.fin_periode()?, &eng.qui())
}

/// Les bornes des periodes elementaires de la paie de `eng` : une seule (la periode payee) pour un mois d'un seul
/// tenant, un contrat d'usage, ou un calcul qui est deja une periode elementaire. Un contrat paye au mois se coupe aux
/// jours ou sa fiche change une valeur qui choisit la pile ou que le bloc 40 porte avec une rubrique en bloc 41
/// (`emplois::jours_de_changement`), et au jour de l'avenant que la saisie date en cours de mois avec ses termes
/// d'avant (`jour_de_l_avenant`).
pub(crate) fn bornes(ctx: &Contexte, eng: &Engagement) -> Resultat<Vec<(Jour, Jour)>> {
    let (debut, fin) = (eng.debut_periode()?, eng.fin_periode()?);
    if eng.periode.is_some() || !eng.nat()?.mensuelle() {
        return Ok(vec![(debut, fin)]);
    }
    let cle = crate::calcul::cle_du_salarie(ctx, eng)?;
    let mut jours = droit::emplois::jours_de_changement(ctx, &cle, &debut, &fin)?;
    jours.extend(jour_de_l_avenant(eng)?);
    droit::periodes::decouper(debut, fin, jours)
}

/// Les engagements des periodes elementaires `bornes` de `eng` : chacun borne a sa periode, sur les termes du contrat
/// de la periode (ceux d'`avant` jusqu'a la veille de l'avenant, ceux de la saisie ensuite), avec sa ligne de salaire :
/// le salaire mensuel de la periode a la part du mois de ses heures (`droit::periodes::part_du_mois`, la regle de la
/// ligne « mois » de `conges::lignes_du_mois`), pour la duree du travail de la periode ; le plafond, lui, se coupe aux
/// jours calendaires (FAQ Agirc-Arrco, exemples 1 et 2 ; CSS R. 242-2, I).
///
/// Refus : une paie qui porte autre chose que son salaire mensuel. Une prime, un conge, une absence, une prime
/// d'anciennete ou un solde de tout compte se rattachent a une periode, ou a l'autre, ou aux deux, et rien ne le dit
/// encore ; le logiciel ne choisit pas.
pub(crate) fn engagements_des_periodes(ctx: &Contexte, eng: &Engagement, bornes: &[(Jour, Jour)]) -> Resultat<Vec<Engagement>> {
    let ligne = match eng.lignes.as_slice() {
        [l] if l.get("rubrique").and_then(Value::as_str) == Some("mois") => l,
        _ => {
            let autres: Vec<String> = eng
                .lignes
                .iter()
                .filter(|l| l.get("rubrique").and_then(Value::as_str) != Some("mois"))
                .map(|l| l.get("rubrique").map(en_texte).unwrap_or_else(|| "?".into()))
                .collect();
            return Err(format!(
                "{} : la paie de ce mois se decoupe en periodes elementaires (un changement de statut, de categorie ou de \
duree du travail le {}), et elle porte {} en plus du salaire mensuel. A quelle periode une telle ligne se rattache (avant \
le changement, apres, ou les deux) ne se lit nulle part dans la saisie, et le logiciel ne decoupe encore que le salaire \
mensuel. Refus : la reporter sur le mois suivant, ou dater le changement au premier jour d'un mois.",
                eng.qui(),
                bornes.get(1).map_or_else(|| "?".to_string(), |b| b.0.jj_mm_aaaa()),
                if autres.is_empty() { "des lignes saisies a la main".to_string() } else { format!("les lignes {}", autres.join(", ")) }
            ));
        }
    };
    let salaire_apres = dec(ligne.get("taux").ok_or_else(|| socle::fichiers::absent("taux"))?)?;
    let quotite_apres = eng.quotite()?;
    let avenant = jour_de_l_avenant(eng)?;
    let (salaire_avant, quotite_avant) = match &eng.avant {
        Some(a) => (dec(&a["salaire_mensuel"])?, dec(&a["quotite_contrat"])?),
        None => (salaire_apres, quotite_apres),
    };
    let fin_mois = eng.fin_mois()?;
    let debut_mois = Jour::nouveau(fin_mois.an, fin_mois.mois, 1).ok_or("engagements_des_periodes : premier jour du mois")?;
    // Le contrat borne au mois : la regle d'un premier ou d'un dernier mois incomplet le regarde, pas la coupure.
    let (debut, fin) = (eng.debut_periode()?, eng.fin_periode()?);
    let regle = crate::conges::regle_du_mois_incomplet(ctx, &eng.salarie, &debut, &fin, &debut_mois, &fin_mois);
    let mut out = Vec::new();
    for (d, f) in bornes {
        let (salaire, quotite) = if avenant.map_or(false, |a| *d < a) { (salaire_avant, quotite_avant) } else { (salaire_apres, quotite_apres) };
        // Chaque periode, au salaire de ses termes, par la regle d'un mois incomplet (`droit::periodes::part_du_mois`).
        let part = droit::periodes::part_du_mois(d, f, &fin_mois, regle)?;
        let mut e = eng.clone();
        e.periode = Some((*d, *f));
        e.quotite_contrat = Some(crate::conges::flot(quotite));
        let (q, h) = (crate::conges::flot(part.fraction), crate::conges::flot(eur2(quotite)));
        let mut ligne = json!({"rubrique": "mois", "quantite": q, "taux": crate::conges::flot(salaire), "heures": h});
        if let Some(p) = part.sur_le_bulletin(droit::conges::heures_par_jour_oeuvre(quotite)) {
            ligne["precision"] = json!(p);
        }
        e.lignes = vec![ligne];
        // Les heures que la DSN mesure (S21.G00.53, « heures » d'un contrat paye au mois) : celles de la ligne de la
        // periode, comme `Engagement::nouveau` les tire des lignes du mois.
        let heures = droit::catalogue::rubrique(&ctx.racine, "mois", &ctx.lue_au)?.heures(q, Some(h));
        e.heures = if heures != 0.0 { Some(heures) } else { None };
        out.push(e);
    }
    Ok(out)
}

/// Ajoute `x` a `cle` de `m`.
fn cumuler(m: &mut BTreeMap<String, Decimal>, cle: &str, x: Decimal) {
    *m.entry(cle.to_string()).or_insert_with(|| Decimal::new(0, 2)) += x;
}

/// Remplace la reduction generale de la periode `p` par `rg` : sa ligne, le total patronal et le cout employeur.
fn poser_reduction(p: &mut Paie, rg: Option<ReductionGenerale>) {
    let p = p.ecrire();
    if let Some(i) = p.lignes.iter().position(|l| l.id == "reduction_generale") {
        let l = p.lignes.remove(i);
        p.total_pat -= l.montant_pat;
    }
    if let Some(r) = rg.as_ref().filter(|r| !r.montant.is_zero()) {
        p.lignes.push(ligne_de_reduction(r, p.brut));
        p.total_pat -= r.montant;
    }
    p.cout_employeur = p.brut + p.total_pat;
    p.reduction_generale = rg;
}

/// Le statut (artiste, technicien) d'une periode, lu comme le bareme le lit : vide, un artiste.
fn statut(p: &Paie) -> String {
    p.salarie.get("statut").filter(|s| vrai(s)).map(en_texte).unwrap_or_else(|| "artiste".into())
}

/// La paie du mois de `eng`, somme de ses periodes elementaires `parts` (dans l'ordre des jours), qu'elle garde
/// (`Paie::periodes`) pour la DSN.
///
/// LA REDUCTION GENERALE : une par mois, comme pour toute paie du moteur -- son coefficient se determine sur le brut et
/// les heures du mois entier (`reduction_generale`) ; chaque periode en recoit la part de son brut, a ce coefficient
/// (`reduction_au_coefficient`), et sa base 03 porte le SMIC de reference de ses heures (S21.G00.79.004). INFERENCE :
/// aucun texte depose ne dit comment repartir la reduction d'un mois decoupe entre ses periodes elementaires ; le
/// coefficient du mois garde la regle mensuelle du moteur, et la somme des parts est la reduction du mois au centime pres
/// de l'arrondi par periode.
///
/// Refus : un statut artiste / technicien qui change en cours de mois (deux regimes d'assiette, L. 242-10 ; pas un cas
/// que le lot porte).
pub(crate) fn assembler(ctx: &Contexte, eng: &Engagement, mut parts: Vec<Paie>) -> Resultat<Paie> {
    let derniere = parts.last().ok_or("assembler : aucune periode")?;
    let statut_du_mois = statut(derniere);
    if let Some(p) = parts.iter().find(|p| statut(p) != statut_du_mois) {
        return Err(format!(
            "{} : le statut du salarie passe de << {} >> a << {} >> en cours de mois. Artiste et technicien n'ont pas les \
memes assiettes (taux reduits, plafond, CSS L. 242-10) : la paie d'un tel mois n'est pas decoupee par le logiciel. \
Dater le changement au premier jour d'un mois sur la fiche.",
            eng.qui(),
            statut(p),
            statut_du_mois
        ));
    }
    // Sous deduction forfaitaire specifique, la reduction se calcule sur le brut abattu et se borne a 130 % de
    // celle du brut entier, et le plafond de 7 600 EUR se suit contrat par contrat : le partage d'un tel mois entre
    // periodes n'est porte par aucun texte depose. `calcul::dfs_applicable` refuse deja la DFS d'un contrat paye au
    // mois ; ce refus-ci la garde si cela changeait.
    if let Some(p) = parts.iter().find(|p| p.deduction_forfaitaire.is_some()) {
        return Err(format!(
            "{} : la paie de ce mois se decoupe en periodes elementaires, et la periode du {} porte une deduction \
forfaitaire specifique. Ni la reduction generale sous deduction (bornee a 130 %), ni le plafond annuel de 7 600 EUR ne se \
partagent entre periodes d'apres un texte depose. Rien n'est calcule.",
            eng.qui(),
            p.engagement.debut_periode().map(|d| d.jj_mm_aaaa()).unwrap_or_default()
        ));
    }
    // La reduction generale du mois, puis sa part dans chaque periode.
    let zero = Decimal::new(0, 2);
    let brut_mois = parts.iter().fold(zero, |a, p| a + p.brut);
    let heures_mois = parts.iter().fold(Decimal::ZERO, |a, p| a + p.heures_travaillees);
    let caisse = parts.iter().any(|p| p.bases.get("brut_cs").map_or(false, |b| *b > Decimal::ZERO));
    let pile = pile_de_la_paie(ctx, derniere)?;
    let du_mois = reduction_generale(&pile, Some(statut_du_mois.as_str()), brut_mois, heures_mois, caisse)?;
    for p in parts.iter_mut() {
        let rg = match &du_mois {
            Some(m) if p.brut > Decimal::ZERO && p.heures_travaillees > Decimal::ZERO => {
                Some(reduction_au_coefficient(&*pile_de_la_paie(ctx, p)?, m.coefficient, p.brut, p.heures_travaillees)?)
            }
            _ => None,
        };
        poser_reduction(p, rg);
    }

    let derniere = parts.last().ok_or("assembler : aucune periode")?;
    let mut e = eng.clone();
    e.lignes = parts.iter().flat_map(|p| p.engagement.lignes.iter().cloned()).collect();
    let heures: f64 = parts.iter().filter_map(|p| p.engagement.heures).sum();
    e.heures = if heures != 0.0 { Some(heures) } else { None };
    let somme = |f: &dyn Fn(&Paie) -> Decimal| parts.iter().fold(zero, |a, p| a + f(p));
    let mut paie = Paie::nouveau(
        e,
        derniere.salarie.clone(),
        derniere.spectacle.clone(),
        somme(&|p| p.brut),
        somme(&|p| p.base_csg),
        somme(&|p| p.plafond),
        derniere.smic,
        parts.iter().fold(Decimal::ZERO, |a, p| a + p.heures),
        derniere.plafond_temps_partiel,
    );
    let m = paie.ecrire();
    // Les lignes de cotisation : une par identifiant et par taux, dans l'ordre ou elles paraissent ; deux periodes au
    // meme taux font une ligne (base et montants ajoutes), un taux qui change en fait deux (le bulletin dit les deux).
    // Une ligne qu'une periode est seule a porter se range apres celle de meme identifiant, sinon apres celle qui la
    // precede dans cette periode ; la reduction
    // generale reste la derniere, comme dans toute paie.
    let mut lignes: Vec<LigneCotisation> = Vec::new();
    for p in &parts {
        let mut precedente: Option<usize> = None;
        for l in &p.lignes {
            let meme = |x: &LigneCotisation| {
                x.id == l.id && x.taux_pat == l.taux_pat && x.taux_sal == l.taux_sal && x.masquer_base_et_taux == l.masquer_base_et_taux
            };
            match lignes.iter().position(meme) {
                Some(i) => {
                    let x = &mut lignes[i];
                    x.base += l.base;
                    x.montant_pat += l.montant_pat;
                    x.montant_sal += l.montant_sal;
                    precedente = Some(i);
                }
                None => {
                    // Apres la ligne de meme identifiant a un autre taux s'il y en a une, sinon apres la precedente.
                    let i = lignes.iter().rposition(|x| x.id == l.id).or(precedente).map_or(0, |i| i + 1);
                    lignes.insert(i, l.clone());
                    precedente = Some(i);
                }
            }
        }
    }
    if let Some(i) = lignes.iter().position(|l| l.id == "reduction_generale") {
        let l = lignes.remove(i);
        lignes.push(l);
    }
    m.lignes = lignes;
    m.total_pat = somme(&|p| p.total_pat);
    m.total_sal = somme(&|p| p.total_sal);
    m.net_a_payer = somme(&|p| p.net_a_payer);
    m.net_social = somme(&|p| p.net_social);
    m.net_imposable = somme(&|p| p.net_imposable);
    m.cout_employeur = somme(&|p| p.cout_employeur);
    let mut bases = BTreeMap::new();
    for p in &parts {
        for (k, v) in &p.bases {
            cumuler(&mut bases, k, *v);
        }
    }
    m.bases = bases;
    m.remuneration = parts.iter().flat_map(|p| p.remuneration.iter().cloned()).collect();
    m.reduction_generale = du_mois.map(|d| {
        let rgs: Vec<&ReductionGenerale> = parts.iter().filter_map(|p| p.reduction_generale.as_ref()).collect();
        let s = |f: &dyn Fn(&ReductionGenerale) -> Decimal| rgs.iter().fold(zero, |a, r| a + f(r));
        ReductionGenerale {
            coefficient: d.coefficient,
            montant: s(&|r| r.montant),
            smic_reference: s(&|r| r.smic_reference),
            heures: rgs.iter().fold(Decimal::ZERO, |a, r| a + r.heures),
            urssaf: s(&|r| r.urssaf),
            chomage: s(&|r| r.chomage),
            agirc_arrco: s(&|r| r.agirc_arrco),
        }
    });
    m.deduction_forfaitaire = derniere.deduction_forfaitaire.clone();
    m.salaire_de_base = somme(&|p| p.salaire_de_base);
    m.salaire_retabli = somme(&|p| p.salaire_retabli);
    m.assiette_chomage = somme(&|p| p.assiette_chomage);
    m.heures_travaillees = parts.iter().fold(Decimal::ZERO, |a, p| a + p.heures_travaillees);
    m.heures_absence = parts.iter().fold(Decimal::ZERO, |a, p| a + p.heures_absence);
    let decrites: Vec<String> = parts
        .iter()
        .filter_map(|p| p.engagement.periode.map(|b| (p, b)))
        .map(|(p, (d, f))| {
            let cadre = if p.salarie.get("statut_categoriel_rc").and_then(Value::as_str) == Some("01") { "cadre" } else { "non cadre" };
            format!(
                "du {} au {} ({}, {} h, brut {})",
                d.jj_mm_aaaa(),
                f.jj_mm_aaaa(),
                cadre,
                p.engagement.quotite().map(socle::num::nombre).unwrap_or_default(),
                p.brut
            )
        })
        .collect();
    m.avertissements.push(Avis {
        info: true,
        texte: format!(
            "{} : le contrat (statut, categorie, duree du travail, salaire) change en cours de mois ; la paie se decoupe en {} periodes \
elementaires, chacune payee pour ses heures (Cass. soc., 20/01/1999, n° 96-45.042), sur ses cotisations et sa part du plafond : {}. La DSN declare les remunerations et les bases de \
chacune (norme DSN, § 2.1.3.2).",
            eng.qui(),
            parts.len(),
            decrites.join(" ; ")
        ),
    });
    m.periodes = parts;
    Ok(paie)
}
