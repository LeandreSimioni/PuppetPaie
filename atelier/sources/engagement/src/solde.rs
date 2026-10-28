//! Le solde de tout compte (`operations_permanents.solde_de_tout_compte`) :
//! ce qui est du au salarie quand son contrat se termine -- conges non pris,
//! precarite, preavis, indemnite de rupture -- et les phrases qui le disent.

use serde_json::{json, Value};

use droit::catalogue::rubrique;
use droit::clauses;
use droit::conges::jours_ouvrables;
use crate::conges::{
    avertir, compteur_conges, dec_ou, indemnite_des_jours, info, jours_ouvrables_de, nature_de,
    numero, flot, remuneration_du_mois, sub,
};
use droit::contexte::{aujourdhui, Contexte};
use droit::natures::Nature;
use socle::date::{d, en_texte, premier_du_mois, repr_flottant};
use crate::Avis;
use socle::fichiers::vrai;
use socle::num::{add, dec, dec_avec_echelle, div, eur2, flottant, mul, zero2, Decimal, Resultat};
use droit::permanents::{self, anciennete_mois, arrondir_jours, jours_acquis, IndemniteDePreavis};

static NUL: Value = Value::Null;

/// Les lignes et les avertissements du solde.
#[derive(Default)]
struct Solde {
    lignes: Vec<Value>,
    avertissements: Vec<Avis>,
}

fn get<'a>(v: &'a Value, k: &str) -> &'a Value {
    v.get(k).unwrap_or(&NUL)
}

/// `v[k]` : une cle absente est un `KeyError`.
fn cle<'a>(v: &'a Value, k: &str) -> Resultat<&'a Value> {
    v.get(k).ok_or_else(|| socle::fichiers::absent(k))
}

fn t(v: &Value) -> String {
    en_texte(v)
}

fn decimal(v: &Value) -> Resultat<Decimal> {
    dec(v)
}

/// Le plus grand de `a` et `b` : `a` a egalite.
fn max_py(a: Decimal, b: Decimal) -> Decimal {
    if b > a {
        b
    } else {
        a
    }
}

/// Le plus petit de `a` et `b` : `a` a egalite.
fn min_py(a: Decimal, b: Decimal) -> Decimal {
    if b < a {
        b
    } else {
        a
    }
}

/// `x or default` d'un texte.
fn ou_texte(v: &Value, defaut: &str) -> String {
    if vrai(v) {
        t(v)
    } else {
        defaut.to_string()
    }
}

fn texte_ou_none(v: &Option<String>) -> String {
    v.clone().unwrap_or_else(|| "None".into())
}

fn idcc(ctx: &Contexte) -> &Value {
    ctx.entreprise.get("idcc").unwrap_or(&NUL)
}

fn motif_de(e: &Value) -> Option<&str> {
    e.get("motif_rupture").and_then(|m| m.as_str())
}

// ------------------------------------------------------------ le profil

fn profil(ctx: &Contexte, e: &Value) -> Resultat<Value> {
    let fiche = e
        .get("salarie")
        .and_then(|s| s.as_str())
        .and_then(|s| ctx.salarie(s).ok())
        .unwrap_or_else(|| json!({}));
    clauses::profil(idcc(ctx), &fiche, Some(e))
}

/// L'age au dernier jour du contrat (ou aujourd'hui), en annees revolues.
fn age(ctx: &Contexte, e: &Value) -> Resultat<Option<Decimal>> {
    let Some(naissance) = e
        .get("salarie")
        .and_then(|s| s.as_str())
        .and_then(|s| ctx.salarie(s).ok())
        .and_then(|f| d(f.get("date_naissance").unwrap_or(&NUL)).ok())
    else {
        return Ok(None);
    };
    let jour = match e.get("fin").filter(|x| vrai(x)) {
        Some(f) => d(f)?,
        None => aujourdhui()?,
    };
    let ans = jour.an as i64 - naissance.an as i64 - ((jour.mois, jour.jour) < (naissance.mois, naissance.jour)) as i64;
    Ok(Some(Decimal::from(ans)))
}

// ------------------------------------------------------------ le preavis

pub struct Preavis {
    pub mois: Option<Decimal>,
    pub legal: Option<Decimal>,
    pub convention: Option<Decimal>,
    pub qui: String,
    pub dit: String,
    pub pourquoi: Option<String>,
}

pub(crate) fn duree_du_preavis(ctx: &Contexte, e: &Value, anciennete: Decimal, motif: Option<&str>) -> Resultat<Preavis> {
    let idcc = idcc(ctx);
    let cdd = !nature_de(e.get("nature"))?.duree_indeterminee;
    let legal = permanents::preavis_mois(anciennete, motif, cdd)?;
    let prof = profil(ctx, e)?;
    let categorie = get(&prof, "categorie").as_str();
    let conv = clauses::preavis_mois(idcc, categorie, anciennete, motif, age(ctx, e)?)?;
    let conv_mois = match get(&conv, "mois") {
        Value::Null => None,
        m => Some(decimal(m)?),
    };
    let conv_pourquoi = get(&conv, "pourquoi");
    let mut out = Preavis { mois: None, legal, convention: conv_mois, qui: String::new(), dit: String::new(), pourquoi: None };
    let rel = clauses::releve(idcc)?;
    let releve = rel.is_some()
        && !rel
            .as_ref()
            .and_then(|r| r.get("preavis").filter(|p| vrai(p)))
            .and_then(|p| p.get("loi_seule"))
            .is_some_and(vrai);
    // La rupture anticipee d'un CDD par le salarie qui part pour un CDI (L1243-2) : un preavis en jours, que ni la loi du
    // licenciement ni la convention ne disent, et que le moteur ne compte pas.
    if let Some(regle) = permanents::regle_du_preavis(motif, cdd)? {
        out.convention = None;
        out.pourquoi = Some(format!("{} Dire la duree a la main (`preavis_mois`).", regle));
        return Ok(out);
    }
    if legal == Some(Decimal::ZERO) || conv_mois == Some(Decimal::ZERO) {
        out.mois = Some(Decimal::ZERO);
        out.qui = "le motif de rupture".into();
        out.pourquoi = if conv_mois == Some(Decimal::ZERO) && !conv_pourquoi.is_null() { Some(t(conv_pourquoi)) } else { None };
        return Ok(out);
    }
    if releve && conv_mois.is_none() && categorie.is_some_and(|c| !c.is_empty()) {
        out.pourquoi = Some(format!("{}. Dire la duree a la main (`preavis_mois`).", t(conv_pourquoi)));
        return Ok(out);
    }
    if releve && conv_mois.is_none() {
        let article = cle(cle(rel.as_ref().ok_or("None")?, "preavis")?, "article")?;
        out.pourquoi = Some(format!(
            "la duree du preavis depend de la categorie du salarie dans {} \
             (art. {}) -- et {}. La dire sur la fiche du salarie \
             (`categorie_conventionnelle` : artiste, employe, \
             agent_de_maitrise, cadre).",
            clauses::nom(idcc),
            t(article),
            t(get(&prof, "source"))
        ));
        return Ok(out);
    }
    let candidats: Vec<Decimal> = [legal, conv_mois].into_iter().flatten().collect();
    if candidats.is_empty() {
        out.pourquoi = Some(format!(
            "la loi ne dit pas sa duree en dessous de six mois d'anciennete \
             (L1234-1 1° : \"la convention ou l'accord collectif de travail \
             ou, a defaut, les usages\") et {}. La dire a la main \
             (`preavis_mois`).",
            t(conv_pourquoi)
        ));
        return Ok(out);
    }
    out.mois = Some(candidats.iter().copied().fold(candidats[0], max_py));
    if conv_mois.is_none() {
        out.qui = "la loi (L1234-1)".into();
        out.dit = format!("L1234-1 ; {} : seule la loi est calculee", t(conv_pourquoi));
    } else if legal.is_none() || conv_mois > legal {
        out.qui = t(get(&conv, "article"));
        out.dit = format!(
            "{} ({}) -- {}",
            t(get(&conv, "article")),
            t(get(&prof, "categorie")),
            match legal {
                None => "la loi n'en donne pas sous six mois".to_string(),
                Some(l) => format!("la loi en donne {}", l),
            }
        );
    } else {
        out.qui = "la loi (L1234-1)".into();
        out.dit = format!(
            "L1234-1 ; {} en donne {} ({})",
            t(get(&conv, "article")),
            t(get(&conv, "mois")),
            t(get(&prof, "categorie"))
        );
    }
    Ok(out)
}

// ------------------------------------------------------ le solde lui-meme

/// Les lignes du dernier bulletin : conges non pris, precarite, preavis,
/// indemnite de rupture. `deja` : les lignes de CE mois deja construites.
pub(crate) fn solde_de_tout_compte(ctx: &Contexte, e: &Value, mois: &str, deja: &[Value]) -> Resultat<(Vec<Value>, Vec<Avis>)> {
    let nat = nature_de(e.get("nature"))?;
    let mut stc = Solde::default();
    let Some(fin) = e.get("fin").filter(|x| vrai(x)) else {
        return Ok((stc.lignes, stc.avertissements));
    };
    let fin = d(fin)?;
    let salaire = dec_ou(e.get("salaire_mensuel"), Decimal::ZERO)?;
    let numero = numero(e);
    let motif = motif_de(e);
    let ent = &ctx.entreprise;
    let premier = premier_du_mois(mois)?;

    // --- l'indemnite compensatrice de conges payes (L3141-28) : toujours due
    let salarie = t(cle(e, "salarie")?);
    let cpt = compteur_conges(ctx, &salarie, &fin, Some(mois))?;
    let mut pris_ce_mois = Decimal::ZERO;
    for c in e.get("conges_pris").and_then(|c| c.as_array()).map_or(&[][..], |a| &a[..]) {
        pris_ce_mois = add(pris_ce_mois, Decimal::from(jours_ouvrables_de(c, ent)?));
    }
    let acquis_ce_mois = jours_acquis(&d(cle(e, "debut")?)?.max(premier), &fin)?;
    let solde = sub(add(cpt.solde(), acquis_ce_mois), pris_ce_mois);
    if solde > Decimal::ZERO {
        let ouvrables = Decimal::from(jours_ouvrables(&premier, &fin, None)?);
        let ouvrables = if ouvrables.is_zero() { Decimal::ONE } else { ouvrables };
        let maintien = eur2(mul(div(salaire, ouvrables), solde));
        // Les jours pris ce mois-ci ont deja consomme le report d'abord :
        // le reste du report est ce qui se paie au dixieme d'avant.
        let entame = min_py(pris_ce_mois, cpt.report_restant());
        let mut cpt2 = cpt.clone();
        cpt2.reports = sub(cpt.reports, entame);
        let ind = indemnite_des_jours(&cpt2, solde, maintien)?;
        stc.lignes.push(json!({
            "rubrique": "indemnite_compensatrice_conges",
            "quantite": flot(solde),
            "taux": flot(eur2(div(ind.retenu, solde))),
        }));
        stc.avertissements.push(info(format!(
            "{} : {} jour(s) de conges acquis et non pris, payes {} ({}) -- \
             art. L3141-28, due quelle que soit la cause de la rupture.",
            numero,
            arrondir_jours(solde),
            ind.retenu,
            ind.regle
        )));
    } else if solde < Decimal::ZERO {
        stc.avertissements.push(avertir(format!(
            "{} : le compteur de conges est NEGATIF ({} jours) -- le salarie \
             a pris plus que ce qu'il a acquis. La retenue sur le solde de \
             tout compte n'est pas calculee : elle se decide, et elle est \
             encadree. A regler a la main.",
            numero,
            arrondir_jours(solde)
        )));
    }

    // --- l'indemnite de fin de CDD (L1243-8)
    match permanents::precarite_due(&nat, motif)? {
        None => {
            if e.get("precarite_due") == Some(&Value::Bool(false)) {
                stc.avertissements.push(info(format!(
                    "{} : indemnite de precarite ecartee a la main ({}).",
                    numero,
                    ou_texte(get(e, "precarite_pourquoi"), "sans raison dite")
                )));
            } else {
                let total = remuneration_du_contrat(ctx, e, mois, brut_des_lignes(ctx, deja)?)?;
                let taux = ent.get("indemnite_precarite_taux").filter(|x| vrai(x));
                let montant = permanents::indemnite_precarite(total, taux.map(flottant).transpose()?)?;
                if montant > Decimal::ZERO {
                    stc.lignes.push(json!({"rubrique": "indemnite_precarite", "quantite": 1, "taux": flot(montant)}));
                    let pour_cent = match taux {
                        Some(Value::Number(n)) if n.is_i64() => (n.as_i64().unwrap_or(0) * 100).to_string(),
                        Some(x) => repr_flottant(flottant(x)? * 100.0),
                        None => repr_flottant(0.10 * 100.0),
                    };
                    stc.avertissements.push(info(format!(
                        "{} : indemnite de fin de contrat {} % sur {} EUR de \
                         remuneration totale = {} (L1243-8).",
                        numero, pour_cent, total, montant
                    )));
                } else {
                    // Une indemnite nulle ne s'ecrit pas : la norme refuse `0.00`.
                    stc.avertissements.push(avertir(format!(
                        "{} : indemnite de fin de contrat NULLE -- la \
                         remuneration totale du contrat est de {} EUR. Si le \
                         contrat a paye quelque chose, ce sont ses mois \
                         precedents dont la saisie manque : l'assiette de \
                         L1243-8 est la remuneration de TOUT le contrat.",
                        numero, total
                    )));
                }
            }
        }
        Some(pourquoi) => {
            if !nat.duree_indeterminee {
                stc.avertissements.push(info(format!(
                    "{} : pas d'indemnite de fin de contrat -- {}",
                    numero, pourquoi
                )));
            }
        }
    }

    // --- le preavis non effectue : L1234-5 pour un licenciement ; les autres fins de contrat ont leurs articles
    // (`preavis.indemnite_compensatrice`), et certaines n'ouvrent aucune indemnite (037, 059, 039, 091).
    let anciennete = anciennete_mois(&d(cle(e, "debut")?)?, &fin);
    let indemnite = if e.get("preavis_non_effectue").is_some_and(vrai) {
        Some(permanents::indemnite_de_preavis(motif)?)
    } else {
        None
    };
    if let Some(IndemniteDePreavis::Aucune(texte)) = &indemnite {
        let saisi = if e.get("preavis_mois").is_some_and(|x| !x.is_null()) {
            " La duree saisie (`preavis_mois`) n'y change rien."
        } else {
            ""
        };
        stc.avertissements.push(avertir(format!(
            "{} : preavis declare non effectue, et AUCUNE indemnite compensatrice de preavis n'est due pour ce motif : \
             le moteur n'ecrit pas de ligne.{} -- {}",
            numero, saisi, texte
        )));
    } else if let Some(indemnite) = &indemnite {
        let duree = duree_du_preavis(ctx, e, anciennete, motif)?;
        let mut mois_preavis = duree.mois;
        if let Some(saisi) = e.get("preavis_mois").filter(|x| !x.is_null()) {
            let saisi = dec_avec_echelle(saisi)?;
            if let Some(m) = mois_preavis {
                if saisi < m {
                    return Err(format!(
                        "{} : preavis de {} mois saisi, et {} en donne {}. Un \
                         contrat peut accorder plus que la loi et la convention, \
                         jamais moins.",
                        numero, saisi, duree.qui, m
                    ));
                }
            }
            mois_preavis = Some(saisi);
        } else if mois_preavis.is_none() {
            return Err(format!("{} : preavis non effectue, et {}", numero, texte_ou_none(&duree.pourquoi)));
        }
        match mois_preavis.filter(|m| !m.is_zero()) {
            Some(m) => {
                // La rubrique, et avec elle le code du bloc 52, suit le motif : 023, ou 032 pour le 092 (L1226-14).
                let rubrique = permanents::rubrique_de_l_indemnite_de_preavis(motif)?;
                stc.lignes.push(json!({"rubrique": rubrique, "quantite": flot(m), "taux": flot(salaire)}));
                let article = match indemnite {
                    IndemniteDePreavis::AutreArticle(t) => format!(" Pour ce motif, l'indemnite vient d'un autre article : {}", t),
                    IndemniteDePreavis::ATrancher(t) => format!(
                        " Pour ce motif, aucun texte depose ne dit si L1234-5 s'applique, ni qu'il ne s'applique pas : {} \
                         A trancher.",
                        t
                    ),
                    _ => String::new(),
                };
                stc.avertissements.push(avertir(format!(
                    "{} : {} mois de preavis non effectue, payes {} EUR \
                     (L1234-5) -- {}. Ce preavis OUVRE DES CONGES PAYES -- \
                     l'article dit \"indemnite de conges payes comprise\" -- et le \
                     moteur ne les a PAS ajoutes au compteur : a verifier.{}",
                    numero,
                    m,
                    eur2(mul(m, salaire)),
                    duree.dit,
                    article
                )));
            }
            None => {
                // Un preavis a zero n'est pas un preavis de zero mois : il n'y en a pas.
                let sans = permanents::raison_sans_preavis(motif, !nat.duree_indeterminee)?;
                let raison = sans
                    .or(duree.pourquoi.filter(|p| !p.is_empty()))
                    .unwrap_or_else(|| "aucun preavis pour ce motif.".to_string());
                stc.avertissements.push(info(format!(
                    "{} : preavis declare non effectue, et il n'y a PAS de \
                     preavis a payer -- {}",
                    numero, raison
                )));
            }
        }
    }

    // --- le delai de prevenance d'une fin d'essai a l'initiative de l'employeur (L1221-25)
    delai_de_prevenance(e, &fin, salaire, motif, &nat, &numero, &mut stc)?;

    // --- l'indemnite de rupture, et laquelle depend du motif
    indemnite_de_rupture(ctx, e, mois, anciennete, motif, &nat, &mut stc)?;

    // --- le type de preavis que le signalement declarera (S21.G00.63.001) et ce que ce solde verse : le
    // signalement refuse un desaccord (`fctu::generer`) ; le solde le dit des la saisie.
    if let Some(type_preavis) = e.get("preavis").and_then(|p| p.get("type")).filter(|t| vrai(t)).map(t) {
        let codes = droit::preavis::codes_52_de_l_indemnite_de_preavis()?;
        let mut versees = Vec::new();
        for l in &stc.lignes {
            let r = rubrique(&ctx.racine, &t(cle(l, "rubrique")?), &ctx.lue_au)?;
            if let Some(c) = r.dsn_bloc52.filter(|c| codes.contains(c)) {
                versees.push((c, r.libelle));
            }
        }
        if let Some(pourquoi) = droit::preavis::desaccord_du_preavis(&type_preavis, &versees)? {
            stc.avertissements.push(avertir(format!(
                "{} : {} Le signalement de fin de contrat le refusera tel quel.",
                numero, pourquoi
            )));
        }
    }

    stc.avertissements.push(info(format!(
        "{} : le CERTIFICAT DE TRAVAIL et le RECU POUR SOLDE DE TOUT COMPTE \
         sont dus au salarie ; le logiciel les ecrit (Paie, « Fin de \
         contrat », « Les deux papiers… »). L'attestation France \
         Travail, elle, part d'elle-meme avec le signalement de fin de \
         contrat.",
        numero
    )));
    Ok((stc.lignes, stc.avertissements))
}

/// L1221-25 : l'employeur qui met fin a l'essai (034) previent le salarie dans un delai qui depend de sa presence ; s'il
/// ne l'a pas respecte, le salarie a droit, sauf faute grave, a une indemnite egale aux salaires qu'il aurait percus
/// jusqu'a l'expiration du delai (`preavis.delai_de_prevenance`, ses lectures dans `_lecture`). La ligne : la part du
/// delai apres la fin du contrat, chaque jour au prorata de son mois, au salaire mensuel ; DSN 52 « 017 ».
fn delai_de_prevenance(
    e: &Value, fin: &socle::date::Jour, salaire: Decimal, motif: Option<&str>, nat: &Nature, numero: &str, stc: &mut Solde,
) -> Resultat<()> {
    let Some(regle) = permanents::regle_du_delai_de_prevenance(motif)? else {
        return Ok(());
    };
    // Un CDD : seulement s'il stipule une periode d'essai d'au moins une semaine (L1221-25, renvoi a L1242-10).
    if !nat.duree_indeterminee {
        let minimum = dec_avec_echelle(cle(&regle, "cdd_essai_minimum_jours")?)?;
        let Some(essai) = e.get("periode_essai_jours").filter(|x| !x.is_null()) else {
            return Err(format!(
                "{} : fin de periode d'essai a l'initiative de l'employeur (034) d'un contrat a duree determinee : L1221-25 \
                 ne donne un delai de prevenance que si le contrat stipule une periode d'essai d'au moins une semaine. \
                 La saisie ne dit pas celle du contrat (`periode_essai_jours`, en jours) ; le moteur ne la devine pas.",
                numero
            ));
        };
        if dec_avec_echelle(essai)? < minimum {
            stc.avertissements.push(info(format!(
                "{} : periode d'essai stipulee de {} jour(s), moins d'une semaine : pas de delai de prevenance (L1221-25), \
                 rien a payer.",
                numero,
                t(essai)
            )));
            return Ok(());
        }
    }
    if e.get("faute_grave").is_some_and(vrai) {
        stc.avertissements.push(info(format!(
            "{} : fin de periode d'essai pour faute grave : pas d'indemnite du delai de prevenance (L1221-25, « sauf s'il a \
             commis une faute grave »).",
            numero
        )));
        return Ok(());
    }
    let Some(notification) = e.get("notification_rupture").filter(|x| vrai(x)) else {
        return Err(format!(
            "{} : fin de periode d'essai a l'initiative de l'employeur (034) : la date de notification de la rupture n'est \
             pas dite (`notification_rupture`). Le delai de prevenance de L1221-25 se compte depuis elle, selon la presence \
             du salarie ; le moteur ne la devine pas.",
            numero
        ));
    };
    let notification = d(notification)?;
    let debut = d(cle(e, "debut")?)?;
    if notification < debut || notification > *fin {
        return Err(format!(
            "{} : la notification de la rupture ({}) doit tomber entre le debut ({}) et la fin ({}) du contrat.",
            numero,
            notification.iso(),
            debut.iso(),
            fin.iso()
        ));
    }
    let delai = permanents::delai_de_prevenance(&debut, &notification)?;
    let dit = format!(
        "fin de periode d'essai notifiee le {} apres {} jour(s) de presence : delai de prevenance de {}, jusqu'au {}",
        notification.jj_mm_aaaa(),
        delai.presence_jours,
        delai.texte,
        delai.expiration.jj_mm_aaaa()
    );
    if delai.expiration <= *fin {
        stc.avertissements.push(info(format!(
            "{} : {} ; le contrat finit le {} : delai respecte, rien a payer.",
            numero,
            dit,
            fin.jj_mm_aaaa()
        )));
        return Ok(());
    }
    // La part non effectuee : du lendemain de la fin au dernier jour du delai, chaque jour au prorata de son mois.
    let (mut fraction, mut jours, mut j) = (Decimal::ZERO, 0i64, fin.plus_jours(1)?);
    while j <= delai.expiration {
        fraction = add(fraction, div(Decimal::ONE, Decimal::from(j.jours_du_mois())));
        jours += 1;
        j = j.plus_jours(1)?;
    }
    let rubrique = t(cle(&regle, "rubrique")?);
    stc.lignes.push(json!({"rubrique": rubrique, "quantite": flot(fraction), "taux": flot(salaire)}));
    stc.avertissements.push(avertir(format!(
        "{} : {} ; le contrat finit le {} : {} jour(s) du delai non effectue(s), payes {} EUR au salaire mensuel, chaque \
         jour au prorata de son mois (L1221-25, indemnite compensatrice ; DSN 52 : 017). Elle comprend les avantages et \
         l'indemnite compensatrice de conges payes de ces jours : le moteur ne les a PAS ajoutes, a verifier. Le decompte \
         de la presence et du delai est une lecture (permanents-base.json, `preavis.delai_de_prevenance._lecture`).",
        numero,
        dit,
        fin.jj_mm_aaaa(),
        jours,
        eur2(mul(fraction, salaire))
    )));
    Ok(())
}

/// Ce que la fin d'un contrat ouvre au salarie, selon sa cause : licenciement,
/// rupture conventionnelle (043), mise a la retraite (038), depart (039).
fn indemnite_de_rupture(
    ctx: &Contexte,
    e: &Value,
    mois: &str,
    anciennete: Decimal,
    motif: Option<&str>,
    nat: &Nature,
    stc: &mut Solde,
) -> Resultat<()> {
    let numero = numero(e);
    let Some(sec) = permanents::rupture(motif)? else {
        return indemnite_de_licenciement(ctx, e, mois, anciennete, motif, nat, stc);
    };
    let cle_sec = t(cle(&sec, "_cle")?);
    if !nat.duree_indeterminee {
        return Err(format!(
            "{} : motif de rupture {} ({}) sur un {}. Ces ruptures-la sont \
             celles d'un contrat a duree INDETERMINEE -- leurs articles sont \
             au titre III du livre II, qui ne traite que de lui.",
            numero,
            motif.unwrap_or("None"),
            cle_sec.replace('_', " "),
            nat.libelle
        ));
    }
    let reference = get(&indemnite_legale(ctx, e, mois, anciennete, motif)?, "reference").clone();
    let retenu = decimal(get(&reference, "retenu"))?;
    let mut rubrique = t(cle(&sec, "rubrique")?);
    let saisi = e.get(&t(cle(&sec, "rubrique")?)).filter(|x| !x.is_null());
    let idcc = idcc(ctx);
    let prof = profil(ctx, e)?;
    let (mut conv_bas, mut conv_haut): (Option<Decimal>, Option<Decimal>) = (None, None);
    let mut conv_article = Value::Null;
    let mut conv_releve = false;
    let mut ouvert = false;
    let mut supplement_rub = format!("{}_conventionnelle", t(cle(&sec, "rubrique")?));
    let (mut legale, article, quoi, c): (Value, &str, &str, Value);

    if cle_sec == "depart_a_la_retraite" {
        legale = permanents::indemnite_depart_retraite(anciennete, retenu)?;
        article = "L1237-9 et D1237-1";
        quoi = "depart a la retraite";
        c = clauses::indemnite_depart_retraite(idcc, anciennete, retenu)?;
    } else if cle_sec == "mise_a_la_retraite" {
        legale = permanents::indemnite_mise_a_la_retraite(anciennete, retenu)?;
        article = "L1237-7";
        quoi = "mise a la retraite";
        stc.avertissements.push(avertir(format!(
            "{} : MISE A LA RETRAITE -- la condition d'age n'est pas \
             verifiee par le moteur (L1237-5 renvoie a L351-8 du code de la \
             securite sociale). Si elle n'est pas remplie, << la rupture du \
             contrat de travail par l'employeur constitue un licenciement >> \
             (L1237-8), et ce qui est declare ici serait faux.",
            numero
        )));
        c = clauses::indemnite_mise_a_la_retraite(idcc, anciennete, retenu)?;
        let cdd = get(&c, "cadre_de_direction");
        if vrai(cdd)
            && get(&prof, "categorie").as_str() == Some("cadre_de_direction")
            && ou_texte(ctx.entreprise.get("annexe_convention").unwrap_or(&NUL), "") == "1"
            && anciennete > Decimal::from(120)
        {
            if saisi.is_none() {
                return Err(format!(
                    "{} : mise a la retraite d'un cadre de direction de plus \
                     de dix ans d'anciennete -- {} ({}) fixe l'indemnite a << 6 \
                     mois de salaires minimum >>, et le texte ne dit pas quel \
                     salaire. Saisir le montant retenu (`{}`) : le moteur \
                     l'accepte a partir du minimum legal.",
                    numero,
                    clauses::nom(idcc),
                    t(cle(cdd, "article")?),
                    rubrique
                ));
            }
            ouvert = true;
        }
    } else {
        let rc = permanents::indemnite_rupture_conventionnelle(anciennete, retenu)?;
        let plancher = get(&rc, "plancher");
        legale = json!({
            "montant": if vrai(plancher) { plancher.clone() } else { Value::String(zero2().to_string()) },
            "du": !plancher.is_null(),
            "pourquoi": get(&rc, "pourquoi"),
        });
        article = "L1237-13";
        quoi = "rupture conventionnelle";
        supplement_rub = "indemnite_rupture_conventionnelle_supplement".into();
        // 3090, art. 7.6 : le plancher est l'indemnite CONVENTIONNELLE de
        // licenciement, et non celle du code du travail.
        let lic = indemnite_conventionnelle(ctx, e, anciennete, retenu, motif)?;
        let mut cc = json!({"releve": get(&lic, "releve"), "pourquoi": get(&lic, "pourquoi"), "article": get(&lic, "article")});
        let rc_sec = clauses::releve(idcc)?.and_then(|r| r.get("rupture_conventionnelle").cloned()).unwrap_or(Value::Null);
        if vrai(get(&lic, "du")) && vrai(&rc_sec) {
            cc["montant"] = cle(&lic, "bas")?.clone();
            cc["haut"] = cle(&lic, "haut")?.clone();
            cc["article"] = Value::String(format!(
                "{} (art. {}, qui renvoie a {})",
                clauses::nom(idcc),
                t(cle(&rc_sec, "article")?),
                t(cle(&lic, "article")?)
            ));
        } else {
            cc["montant"] = Value::Null;
        }
        c = cc;
    }
    if vrai(get(&c, "releve")) {
        conv_releve = true;
        conv_article = get(&c, "article").clone();
        conv_bas = match get(&c, "montant") {
            Value::Null => None,
            m => Some(decimal(m)?),
        };
        conv_haut = match c.get("haut") {
            Some(Value::Null) => None,
            Some(h) => Some(decimal(h)?),
            None => conv_bas,
        };
    }
    let conv_pourquoi = get(&c, "pourquoi").clone();
    legale["reference"] = reference.clone();

    let du_legal = vrai(get(&legale, "du"));
    let m_legal = if du_legal { decimal(get(&legale, "montant"))? } else { zero2() };
    let bas = max_py(m_legal, conv_bas.filter(|b| !b.is_zero()).unwrap_or_else(zero2));
    let haut = max_py(m_legal, conv_haut.filter(|b| !b.is_zero()).unwrap_or_else(zero2));
    let du = du_legal || bas > Decimal::ZERO;
    let phrase = phrase_de_la_convention(conv_releve, &conv_article, conv_bas, conv_haut, m_legal, &conv_pourquoi, idcc);

    let montant;
    if let Some(saisi) = saisi {
        montant = dec_avec_echelle(saisi)?;
        if !du {
            return Err(format!(
                "{} : indemnite de {} de {} EUR saisie, et ni la loi ni la \
                 convention ne donnent de montant a quoi la comparer -- {}. \
                 Le moteur ne declare pas une indemnite dont il ne sait pas \
                 dire ce qu'elle supporte.",
                numero,
                quoi,
                montant,
                t(get(&legale, "pourquoi"))
            ));
        }
        if montant < bas {
            return Err(format!(
                "{} : indemnite de {} de {} EUR, INFERIEURE au minimum de {} \
                 EUR (loi, {} : {} EUR ; {}). Ce n'est pas au moteur de la \
                 corriger.",
                numero, quoi, montant, bas, article, m_legal, phrase
            ));
        }
        if montant > haut && !ouvert {
            return Err(format!(
                "{} : indemnite de {} de {} EUR, alors que le montant LEGAL \
                 est de {} EUR ({}) -- {}. La part au-dela a un autre regime \
                 social -- CSG et CRDS dues des le premier euro qui depasse \
                 le montant legal ou conventionnel -- et le moteur ne sait pas \
                 l'assujettir sur une seule ligne : il refuse plutot que de la \
                 declarer de travers.",
                numero, quoi, montant, m_legal, article, phrase
            ));
        }
    } else if let (Some(cb), Some(ch)) = (conv_bas, conv_haut) {
        if cb != ch && ch > m_legal && cle_sec != "rupture_conventionnelle" {
            return Err(format!(
                "{} : {} se lit de deux facons, de {} a {} EUR ; la loi donne {} \
                 EUR. Saisir le montant retenu (`{}`).",
                numero,
                t(&conv_article),
                cb,
                ch,
                m_legal,
                rubrique
            ));
        }
        montant = bas;
    } else {
        montant = bas;
    }

    if !du {
        stc.avertissements.push(info(format!(
            "{} : pas d'indemnite legale de {} -- {} Et {}.",
            numero,
            quoi,
            t(get(&legale, "pourquoi")),
            phrase
        )));
        return Ok(());
    }
    if cle_sec == "rupture_conventionnelle" && saisi.is_none() {
        // Le montant est celui de la convention signee, que le moteur n'a pas.
        stc.avertissements.push(avertir(format!(
            "{} : RUPTURE CONVENTIONNELLE -- l'indemnite specifique est due \
             et n'a PAS ete ecrite : son montant est celui de la convention \
             signee, que le moteur n'a pas. Le plancher legal est de {} EUR \
             ({}) ; {}. Ajouter a la paie une ligne « {} » avec le montant \
             convenu.",
            numero,
            m_legal,
            article,
            phrase,
            libelle_rubrique(ctx, &rubrique)
        )));
        return Ok(());
    }
    if cle_sec == "rupture_conventionnelle" {
        // Deux rubriques pour une seule indemnite : l'impot depend d'un fait du salarie.
        let droit = e.get("en_droit_a_la_retraite").filter(|x| !x.is_null());
        let Some(droit) = droit else {
            return Err(format!(
                "{} : rupture conventionnelle -- il manque un fait, et il \
                 change l'impot du salarie : est-il EN DROIT DE BENEFICIER \
                 D'UNE PENSION de retraite d'un regime legalement \
                 obligatoire ? S'il ne l'est pas, son indemnite n'est pas \
                 imposable (CGI 80 duodecies 6°) ; s'il l'est, elle l'est. \
                 Le dire sur l'engagement (`en_droit_a_la_retraite`, vrai ou \
                 faux).",
                numero
            ));
        };
        if vrai(droit) {
            rubrique = "indemnite_rupture_conventionnelle_imposable".into();
            supplement_rub = "indemnite_rupture_conventionnelle_supplement_imposable".into();
        }
    }
    if montant <= Decimal::ZERO {
        stc.avertissements.push(info(format!(
            "{} : pas d'indemnite de {} -- {} Et {}.",
            numero,
            quoi,
            ou_texte(get(&legale, "pourquoi"), ""),
            phrase
        )));
        return Ok(());
    }
    verifier_les_plafonds(ctx, mois, montant, quoi, &numero)?;
    let part_legale = min_py(montant, m_legal);
    let supplement = sub(montant, part_legale);
    if part_legale > Decimal::ZERO {
        stc.lignes.push(json!({"rubrique": rubrique, "quantite": 1, "taux": flot(part_legale)}));
    }
    if supplement > Decimal::ZERO {
        stc.lignes.push(json!({"rubrique": supplement_rub, "quantite": 1, "taux": flot(supplement)}));
    }
    // L137-12 : une contribution patronale sur la part exoneree, due et non calculee.
    let regles = permanents::regles()?;
    let contrib = cle(&regles, "regime_des_indemnites_de_rupture")?.get("contribution_patronale").filter(|x| vrai(x));
    if let (Some(contrib), Some(m)) = (contrib, motif) {
        if get(contrib, "motifs").as_array().is_some_and(|l| l.iter().any(|x| x.as_str() == Some(m))) {
            let taux = dec_avec_echelle(cle(contrib, "taux")?)?;
            stc.avertissements.push(avertir(format!(
                "{} : l'employeur doit en plus la CONTRIBUTION DE {} % sur la \
                 part exoneree de cette indemnite (L137-12 du code de la \
                 securite sociale, au profit de la CNAV). Le moteur ne la \
                 calcule PAS : elle n'est dans aucune section du bareme. Sur {} \
                 EUR exoneres, elle vaut {} EUR.",
                numero,
                taux,
                montant,
                eur2(div(mul(montant, taux), Decimal::ONE_HUNDRED))
            )));
        }
    }
    let mut detail = String::new();
    if cle_sec == "depart_a_la_retraite" && vrai(get(&legale, "palier")) {
        detail = format!(
            " -- palier de {} ans de la loi, {} mois de salaire",
            t(get(&legale, "palier")),
            t(cle(&legale, "mois")?)
        );
    }
    stc.avertissements.push(info(format!(
        "{} : indemnite de {} {} EUR ({} mois d'anciennete, salaire de \
         reference {} par {}){} -- loi ({}) : {} EUR ; {}.{}",
        numero,
        quoi,
        montant,
        arrondir_jours(anciennete),
        t(get(&reference, "retenu")),
        t(get(&reference, "regle")),
        detail,
        article,
        m_legal,
        phrase,
        if supplement > Decimal::ZERO {
            format!(" Le supplement de {} EUR se declare a part, sous son code conventionnel.", supplement)
        } else {
            String::new()
        }
    )));
    let prevenance = get(&c, "prevenance_mois");
    if vrai(prevenance) && cle_sec != "rupture_conventionnelle" {
        stc.avertissements.push(avertir(format!(
            "{} : {} impose un DELAI DE PREVENANCE de {} mois, par lettre \
             recommandee, a qui prend l'initiative du depart a la retraite. \
             Le moteur ne le verifie pas.",
            numero,
            t(&conv_article),
            t(prevenance)
        )));
    }
    Ok(())
}

/// Ce que la convention a dit d'une indemnite de rupture, en une phrase.
fn phrase_de_la_convention(
    releve: bool,
    article: &Value,
    bas: Option<Decimal>,
    haut: Option<Decimal>,
    m_legal: Decimal,
    pourquoi: &Value,
    idcc: &Value,
) -> String {
    if !releve {
        return format!("les clauses de {} ne sont pas relevees : seule la loi est calculee", clauses::nom(idcc));
    }
    let Some(bas) = bas else {
        return format!(
            "{} n'ouvre rien ici{}",
            if vrai(article) { t(article) } else { clauses::nom(idcc) },
            if vrai(pourquoi) { format!(" ({})", t(pourquoi)) } else { String::new() }
        );
    };
    let article = t(article);
    if haut.is_some_and(|h| h != bas) {
        format!("{} donne de {} a {} EUR selon la lecture", article, bas, haut.unwrap_or(bas))
    } else if bas == m_legal {
        format!("{} donne la meme chose", article)
    } else if bas > m_legal {
        format!("{} donne {} EUR, et c'est elle qui s'applique", article, bas)
    } else {
        format!("{} donne moins ({} EUR) : la loi s'applique", article, bas)
    }
}

/// Les deux plafonds de L242-1 II 7° : au-dela, le moteur refuse.
fn verifier_les_plafonds(ctx: &Contexte, mois: &str, montant: Decimal, quoi: &str, numero: &str) -> Resultat<()> {
    let regles = permanents::regles()?;
    let regles_plafonds = cle(cle(&regles, "regime_des_indemnites_de_rupture")?, "plafonds")?;
    let an: i32 = mois.chars().take(4).collect::<String>().parse().map_err(|_| format!("nombre entier illisible : {}", mois))?;
    let Ok((mensuel, _)) = ctx.plafond_securite_sociale(an) else {
        return Ok(());
    };
    let annuel = mul(Decimal::from(mensuel), Decimal::from(12));
    for (k, quel) in [
        ("cotisations_exoneration_pass", "exoneree"),
        ("cotisations_assujettissement_total_pass", "hors de l'assiette"),
    ] {
        let combien = cle(regles_plafonds, k)?;
        let seuil = mul(annuel, dec_avec_echelle(combien)?);
        if montant > seuil {
            return Err(format!(
                "{} : indemnite de {} de {} EUR, au-dela de {} plafonds \
                 annuels de Securite sociale ({} EUR). Elle n'est plus {} \
                 en entier (L242-1 II 7° du code de la securite sociale) et \
                 le moteur ne sait pas partager une ligne entre une part \
                 exoneree et une part soumise : il refuse.",
                numero,
                quoi,
                montant,
                t(combien),
                eur2(seuil),
                quel
            ));
        }
    }
    Ok(())
}

/// L'indemnite de licenciement (L1234-9, R1234-2) : la loi sous 007, ce que la
/// convention donne de plus sous 021.
fn indemnite_de_licenciement(
    ctx: &Contexte,
    e: &Value,
    mois: &str,
    anciennete: Decimal,
    motif: Option<&str>,
    nat: &Nature,
    stc: &mut Solde,
) -> Resultat<()> {
    let numero = numero(e);
    let saisi = e.get("indemnite_licenciement").filter(|x| !x.is_null());
    if saisi.is_none() && !(nat.duree_indeterminee && motif.is_some_and(|m| !m.is_empty())) {
        return Ok(());
    }
    let legale = indemnite_legale(ctx, e, mois, anciennete, motif)?;
    let reference = get(&legale, "reference").clone();
    let conv = indemnite_conventionnelle(ctx, e, anciennete, decimal(get(&reference, "retenu"))?, motif)?;
    let m_legal = if vrai(get(&legale, "du")) { decimal(get(&legale, "montant"))? } else { zero2() };
    let conv_du = vrai(get(&conv, "du"));
    let bas = max_py(m_legal, if conv_du { decimal(cle(&conv, "bas")?)? } else { zero2() });
    let haut = max_py(m_legal, if conv_du { decimal(cle(&conv, "haut")?)? } else { zero2() });

    let montant;
    if let Some(saisi) = saisi {
        montant = dec_avec_echelle(saisi)?;
        if montant > haut {
            return Err(format!(
                "{} : indemnite de licenciement de {} EUR, alors que \
                 l'indemnite LEGALE est de {} EUR{}. La part supra-legale et \
                 supra-conventionnelle a un autre regime social (CSG/CRDS \
                 dues au-dela, cotisations au premier euro au-dela de dix \
                 plafonds annuels) et le moteur ne sait pas l'assujettir : il \
                 refuse plutot que de la declarer de travers.",
                numero,
                montant,
                m_legal,
                dit_conventionnel(&conv)?
            ));
        }
        if montant < bas {
            return Err(format!(
                "{} : indemnite de licenciement de {} EUR, INFERIEURE a ce \
                 qui est du ({} EUR : loi {} EUR{}).",
                numero,
                montant,
                bas,
                m_legal,
                dit_conventionnel(&conv)?
            ));
        }
    } else {
        if conv_du && decimal(cle(&conv, "bas")?)? != decimal(cle(&conv, "haut")?)? && decimal(cle(&conv, "haut")?)? > m_legal {
            return Err(format!(
                "{} : {} se lit de deux facons, qui ne donnent pas le meme \
                 montant : {}. La loi donne {} EUR. Ajouter a la paie une \
                 ligne « Indemnite de licenciement » avec le montant retenu, \
                 entre {} et {} EUR.",
                numero,
                t(cle(&conv, "article")?),
                lectures(&conv)?,
                m_legal,
                bas,
                haut
            ));
        }
        montant = bas;
    }
    if montant <= Decimal::ZERO {
        let pourquoi = if vrai(get(&legale, "pourquoi")) { get(&legale, "pourquoi") } else { get(&conv, "pourquoi") };
        if vrai(pourquoi) {
            stc.avertissements.push(info(format!("{} : pas d'indemnite de licenciement -- {}", numero, t(pourquoi))));
        }
        return Ok(());
    }
    verifier_les_plafonds(ctx, mois, montant, "licenciement", &numero)?;
    let part_legale = min_py(montant, m_legal);
    if part_legale > Decimal::ZERO {
        let mut ligne = json!({"rubrique": "indemnite_licenciement", "quantite": 1, "taux": flot(part_legale)});
        if saisi.is_some() {
            ligne["montant"] = json!(flot(part_legale));
        }
        stc.lignes.push(ligne);
    }
    let supplement = sub(montant, part_legale);
    if supplement > Decimal::ZERO {
        stc.lignes.push(json!({"rubrique": "indemnite_licenciement_conventionnelle", "quantite": 1, "taux": flot(supplement)}));
    }
    stc.avertissements.push(info(format!(
        "{} : indemnite de licenciement {} EUR ({} mois d'anciennete, salaire \
         de reference {} par {}) -- loi (L1234-9 et R1234-2) : {} EUR ; {}.{}",
        numero,
        montant,
        arrondir_jours(anciennete),
        t(get(&reference, "retenu")),
        t(get(&reference, "regle")),
        m_legal,
        phrase_conventionnelle(&conv, m_legal)?,
        if supplement > Decimal::ZERO {
            format!(" Le supplement de {} EUR se declare a part, en 021.", supplement)
        } else {
            String::new()
        }
    )));
    if vrai(get(&conv, "hors_primes")) && supplement > Decimal::ZERO {
        stc.avertissements.push(avertir(format!(
            "{} : l'assiette de {} est le salaire de BASE, hors primes ; le \
             logiciel a pris les bruts des derniers mois. S'ils comptent des \
             primes, ajouter a la paie une ligne « Indemnite de \
             licenciement » avec le montant recalcule.",
            numero,
            t(cle(&conv, "article")?)
        )));
    }
    Ok(())
}

/// Le nom qu'un administrateur voit dans la liste des rubriques.
fn libelle_rubrique(ctx: &Contexte, code: &str) -> String {
    rubrique(&ctx.racine, code, &ctx.lue_au).map(|r| r.libelle).unwrap_or_else(|_| code.to_string())
}

/// L'indemnite de licenciement de la convention de la compagnie.
fn indemnite_conventionnelle(
    ctx: &Contexte,
    e: &Value,
    anciennete: Decimal,
    reference: Decimal,
    motif: Option<&str>,
) -> Resultat<Value> {
    clauses::indemnite_licenciement(
        idcc(ctx),
        ctx.entreprise.get("annexe_convention").unwrap_or(&NUL),
        &profil(ctx, e)?,
        anciennete,
        reference,
        motif,
        age(ctx, e)?,
    )
}

fn lectures(conv: &Value) -> Resultat<String> {
    let l = cle(conv, "lectures")?.as_object().ok_or("lectures attendues")?;
    Ok(l.iter().map(|(n, v)| format!("{} {} EUR", n.replace('_', " "), t(v))).collect::<Vec<_>>().join(" ; "))
}

fn dit_conventionnel(conv: &Value) -> Resultat<String> {
    if !vrai(get(conv, "releve")) {
        return Ok(String::new());
    }
    let article = t(cle(conv, "article")?);
    if !vrai(get(conv, "du")) {
        return Ok(format!(" et rien par {}", article));
    }
    let (bas, haut) = (decimal(cle(conv, "bas")?)?, decimal(cle(conv, "haut")?)?);
    Ok(if bas == haut {
        format!(" et {} EUR par {}", bas, article)
    } else {
        format!(" et de {} a {} EUR par {}", bas, haut, article)
    })
}

/// Ce que la convention a dit, pour l'avertissement.
fn phrase_conventionnelle(conv: &Value, m_legal: Decimal) -> Resultat<String> {
    if !vrai(get(conv, "releve")) {
        let pourquoi = match conv.get("pourquoi") {
            Some(p) => t(p),
            None => "les clauses de la convention ne sont pas relevees".to_string(),
        };
        return Ok(format!("{} : seule la loi est calculee", pourquoi));
    }
    let article = t(cle(conv, "article")?);
    if !vrai(get(conv, "du")) {
        return Ok(format!("{} n'ouvre rien ici ({})", article, t(get(conv, "pourquoi"))));
    }
    let bas = decimal(cle(conv, "bas")?)?;
    Ok(if bas == m_legal {
        format!("{} donne la meme chose", article)
    } else if bas > m_legal {
        format!("{} donne {} EUR, et c'est elle qui s'applique", article, bas)
    } else {
        format!("{} donne moins ({} EUR) : la loi s'applique", article, bas)
    })
}

/// L'indemnite legale de licenciement et son salaire de reference.
fn indemnite_legale(ctx: &Contexte, e: &Value, mois: &str, anciennete: Decimal, motif: Option<&str>) -> Resultat<Value> {
    let (douze, trois) = bruts_precedents(ctx, e, mois)?;
    let reference = permanents::salaire_de_reference(&douze, &trois);
    let mut out = permanents::indemnite_licenciement(anciennete, decimal(get(&reference, "retenu"))?, motif)?;
    out["reference"] = reference;
    Ok(out)
}

/// Les bruts mensuels des douze et des trois derniers mois (R1234-4) ; faute
/// de mois produits, le salaire mensuel du contrat.
fn bruts_precedents(ctx: &Contexte, e: &Value, mois: &str) -> Resultat<(Vec<Decimal>, Vec<Decimal>)> {
    let salarie = t(cle(e, "salarie")?);
    let (an, mo): (i32, i32) = (
        mois.get(..4).and_then(|s| s.parse().ok()).ok_or("mois illisible")?,
        mois.get(5..7).and_then(|s| s.parse().ok()).ok_or("mois illisible")?,
    );
    let mut bruts = Vec::new();
    for i in 1..=12 {
        let (mut m, mut a) = (mo - i, an);
        while m <= 0 {
            m += 12;
            a -= 1;
        }
        let nom = format!("{:04}-{:02}", a, m);
        if !crate::stock::a_une_saisie(&ctx.racine, &nom)? {
            continue;
        }
        bruts.push(remuneration_du_mois(ctx, &nom, &salarie)?);
    }
    if bruts.is_empty() {
        let salaire = dec_ou(e.get("salaire_mensuel"), Decimal::ZERO)?;
        bruts = vec![salaire; 12];
    }
    let trois = bruts.iter().take(3).copied().collect();
    Ok((bruts, trois))
}

/// Le brut que portent des lignes deja construites, sans les charger : la
/// meme somme que `calcul.calculer`.
fn brut_des_lignes(ctx: &Contexte, lignes: &[Value]) -> Resultat<Decimal> {
    let mut total = zero2();
    for l in lignes {
        let r = rubrique(&ctx.racine, &t(cle(l, "rubrique")?), &ctx.lue_au)?;
        if !r.soumis {
            continue;
        }
        let signe = if r.sens == "-" { Decimal::NEGATIVE_ONE } else { Decimal::ONE };
        total = add(total, mul(signe, eur2(mul(dec_avec_echelle(cle(l, "taux")?)?, dec_avec_echelle(cle(l, "quantite")?)?))));
    }
    Ok(total)
}

/// La remuneration totale brute versee pendant le contrat (L1243-8) : les mois
/// produits, plus le mois courant, dont le brut arrive tout calcule.
fn remuneration_du_contrat(ctx: &Contexte, e: &Value, mois: &str, ce_mois: Decimal) -> Resultat<Decimal> {
    let salarie = t(cle(e, "salarie")?);
    let mut total = ce_mois;
    let debut = d(cle(e, "debut")?)?;
    let (mut an, mut mo) = (debut.an, debut.mois);
    loop {
        let m = format!("{:04}-{:02}", an, mo);
        if m.as_str() > mois {
            break;
        }
        if m != mois && crate::stock::a_une_saisie(&ctx.racine, &m)? {
            total = add(total, remuneration_du_mois(ctx, &m, &salarie)?);
        }
        (an, mo) = if mo == 12 { (an + 1, 1) } else { (an, mo + 1) };
    }
    Ok(total)
}
