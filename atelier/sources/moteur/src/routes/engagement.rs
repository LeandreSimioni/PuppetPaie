//! Les routes de l'engagement : le valider (`valider-engagement`), modifier un engagement valide d'un geste
//! (`modifier-engagement`), et le retirer (`retirer-engagement` : supprime s'il n'en est rien parti, sinon rendu non
//! actif). Un refus garde sa phrase quand elle est utile a la page.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use socle::echec::{erreur, Echec, Refus, R};
use socle::fichiers::vrai;
use socle::valeur::valeur_en_texte;



/// Les routes de ce module. `etat-par-engagement` se lit dans l'etat garde (`etat_garde`), que la fenetre refait apres
/// chaque ecriture.
pub const ROUTES: &[&str] = &["valider-engagement", "modifier-engagement", "retirer-engagement"];

/// Celles qui ecrivent.
pub const ECRITURES: &[&str] = ROUTES;

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "valider-engagement" => valider_engagement(ctx, charge),
        "modifier-engagement" => modifier_engagement(ctx, charge),
        "retirer-engagement" => retirer_engagement(ctx, charge),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

// ------------------------------------------------------------------ outils

#[track_caller]
pub(super) fn refus(m: impl Into<String>) -> Echec {
    erreur("ErreurOperation", m)
}

pub(super) fn en_echec(r: Refus) -> Echec {
    Echec { classe: r.classe, message: r.message }
}

/// `charge["cle"]` : un texte requis.
pub(super) fn texte_requis<'a>(charge: &'a Value, cle: &str) -> R<&'a str> {
    crate::routes::fiches::exiger(charge, cle)?.as_str().ok_or_else(|| erreur("TypeError", format!("`{}` : un texte est attendu", cle)))
}

/// `bool(charge.get(cle, defaut))`.
pub(super) fn booleen(charge: &Value, cle: &str, defaut: bool) -> bool {
    charge.get(cle).map_or(defaut, vrai)
}

/// `datetime.now().isoformat(timespec="seconds")`, a l'heure de Paris.
pub(super) fn maintenant() -> String {
    let t = socle::temps::secondes();
    socle::paris::iso_secondes(t)
}

// ------------------------------------------------------------------ valider-engagement

/// Ce que la page designe, lu dans l'ordre de l'ecran.
enum Designe {
    /// Un engagement auquel il manque un fait (`engagement::stock::A_COMPLETER`), hors de la saisie enregistree.
    ACompleter(engagement::stock::Objet),
    /// Un engagement de la saisie enregistree : la charge a donner a `engagement_designe`, son rang compte parmi les
    /// engagements de `vue_du_mois`.
    DansLaVue(Value),
}

/// L'ENGAGEMENT QUE LA PAGE DESIGNE, DANS L'ORDRE DE L'ECRAN (`engagement::stock::objets_a_l_ecran`) : `salarie`,
/// `debut`, et, si deux contrats du salarie commencent ce jour-la, `numero` ou `rang`. La page compte ce rang sur ses
/// cartes, celles des engagements a completer comprises (`etapes.rs`, `avec_les_engagements_a_completer`) ; la saisie
/// enregistree (`vue_du_mois`) n'a pas ceux qui n'ont jamais ete valides : leur rang se retire avant de la lire.
fn designe_a_l_ecran(ctx: &Contexte, charge: &Value) -> R<Designe> {
    let (mois, salarie, debut) = (texte_requis(charge, "mois")?, texte_requis(charge, "salarie")?, texte_requis(charge, "debut")?);
    let objets = engagement::stock::objets_a_l_ecran(&ctx.racine, mois).map_err(Echec::from)?;
    let du_jour: Vec<(engagement::stock::Objet, Option<String>)> = objets
        .into_iter()
        .filter_map(|o| {
            let e = Value::Object(o.brut.clone());
            let la = engagement::cle_de_l_engagement(ctx, &e).as_deref() == Some(salarie) && engagement::debut_brut(&e) == debut;
            la.then(|| (o, droit::jumeaux::numero_pose(&e)))
        })
        .collect();
    if !du_jour.iter().any(|(o, _)| o.a_completer().is_some()) {
        return Ok(Designe::DansLaVue(charge.clone()));
    }
    let numero = charge.get("numero").filter(|v| vrai(v)).map(valeur_en_texte);
    let rang = charge.get("rang").and_then(Value::as_u64).unwrap_or(0) as usize;
    let k = numero
        .as_ref()
        .and_then(|n| du_jour.iter().position(|(_, m)| m.as_ref() == Some(n)))
        .or_else(|| (du_jour.len() == 1).then_some(0))
        .or_else(|| (rang < du_jour.len()).then_some(rang));
    Ok(match k {
        Some(k) if du_jour[k].0.a_completer().is_some() => Designe::ACompleter(du_jour[k].0.clone()),
        Some(k) => {
            let hors_de_la_vue = du_jour[..k].iter().filter(|(o, _)| o.est_a_completer()).count();
            let mut c = charge.clone();
            c["rang"] = json!(k - hors_de_la_vue);
            Designe::DansLaVue(c)
        }
        None => Designe::DansLaVue(charge.clone()),
    })
}

/// « VALIDER L'ENGAGEMENT » : l'engagement se designe dans la saisie enregistree de `mois` : `salarie`, `debut`, et, si
/// deux contrats du salarie commencent ce jour-la, `numero` ou `rang` (son rang parmi eux). Il ecrit ses paies, une par
/// mois qu'il couvre (`engagement::etats::valider`) ; un engagement deja valide le reste, tel quel (il se modifie par
/// `modifier-engagement`). Refus : un engagement absent de la saisie enregistree (valider ce qui n'est pas enregistre
/// poserait une pastille verte sur un formulaire qu'un rechargement effacerait), ou auquel il manque un fait.
fn valider_engagement(ctx: &Contexte, charge: &Value) -> R<Value> {
    let (mois, salarie, debut) = (texte_requis(charge, "mois")?, texte_requis(charge, "salarie")?, texte_requis(charge, "debut")?);
    // Il lui manque un fait : la validation dit lequel, et rien n'est ecrit. Ce qui manquait se relit d'abord : le fait a
    // pu se dire depuis, sur sa fiche.
    crate::routes::mois::relire_les_manques(ctx)?;
    let charge = match designe_a_l_ecran(ctx, charge)? {
        Designe::ACompleter(o) => {
            let manque = o.a_completer().unwrap_or_default();
            let data = engagement::stock::vue_a_l_ecran(&ctx.racine, mois).map_err(Echec::from)?.unwrap_or(Value::Null);
            return Err(crate::routes::mois::refus_du_calcul(ctx, &data, manque));
        }
        Designe::DansLaVue(c) => c,
    };
    let (data, i) = engagement_designe(ctx, &charge)?;
    let le = match data["engagements"][i].get("valide").and_then(|v| v.get("le")).cloned() {
        Some(le) => le,
        None => {
            let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs).map_err(Echec::from)?;
            let le = engagement::etats::valider(ctx, mois, i, &departs).map_err(|m| crate::routes::mois::refus_du_calcul(ctx, &data, m))?.le().to_string();
            let cle = engagement::cle_de_l_engagement(ctx, &data["engagements"][i]).unwrap_or_else(|| salarie.to_string());
            crate::routes::fiches::poser_anciennete_a_l_embauche(ctx, &cle, debut)?;
            Value::String(le)
        }
    };
    let avis = avis_de_validation(ctx, mois, i)?;
    Ok(json!({"salarie": salarie, "debut": debut, "valide": le, "avis": avis}))
}

// ------------------------------------------------------------------ modifier-engagement

/// Les fichiers des engagements tels qu'ils sont avant une modification : une modification qui n'aboutit pas (rien n'a
/// change, l'avertissement n'est pas encore confirme, un refus) les remet octet pour octet.
struct Photo {
    dossier: PathBuf,
    fichiers: Vec<(PathBuf, Vec<u8>)>,
}

impl Photo {
    fn prendre(racine: &Path) -> R<Photo> {
        let dossier = racine.join(engagement::stock::DOSSIER);
        let mut fichiers = Vec::new();
        for p in Photo::presents(&dossier) {
            let octets = std::fs::read(&p).map_err(|e| erreur("OSError", format!("{} : {}", p.display(), e)))?;
            fichiers.push((p, octets));
        }
        Ok(Photo { dossier, fichiers })
    }

    fn presents(dossier: &Path) -> Vec<PathBuf> {
        std::fs::read_dir(dossier).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_file()).collect()
    }

    fn remettre(&self) -> R<()> {
        let io = |p: &Path, e: std::io::Error| erreur("OSError", format!("{} : {}", p.display(), e));
        for p in Photo::presents(&self.dossier) {
            if !self.fichiers.iter().any(|(q, _)| *q == p) {
                std::fs::remove_file(&p).map_err(|e| io(&p, e))?;
            }
        }
        for (p, octets) in &self.fichiers {
            if std::fs::read(p).ok().as_ref() != Some(octets) {
                std::fs::write(p, octets).map_err(|e| io(p, e))?;
            }
        }
        Ok(())
    }
}

/// Ce que la modification a donne.
enum Issue {
    /// Rien n'a change.
    Rien,
    /// Ce qui est parti le demande : l'avertissement, a confirmer.
    Avertir(Value),
    /// Ecrite : pour chaque engagement, son rang et la date de sa validation.
    Ecrite(Vec<(usize, String)>),
}

/// « MODIFIER L'ENGAGEMENT » : les valeurs que la carte ouverte d'un engagement valide montre, ecrites d'un geste.
/// `engagements` : chaque engagement modifie ensemble (ceux de plusieurs salaries valides ensemble), designe comme pour
/// `valider-engagement` (`salarie`, `debut`, `numero`, `rang`), avec `saisie`, ce que sa carte ecrit pour `mois`. Un
/// salarie coche en plus sur la carte ouverte n'a pas d'engagement a designer : son entree ne porte que `saisie`, et il
/// s'ajoute au mois, valide avec les autres (`engagement::stock::ajouter_au_mois`) ; il faut au moins un engagement
/// designe.
///
/// - Rien n'a change : rien ne s'ecrit, la reponse le dit (`change` faux).
/// - Quelque chose a change, et rien de ce qui est parti ne le demande : la modification s'ecrit, l'engagement reste valide.
/// - Ce qui est parti le demande : la reponse porte l'avertissement, et rien n'est ecrit ; la page le montre et renvoie
///   la demande avec `confirme` (et `a_partir_du`, le mois choisi, `a_partir_du_jour`, son jour quand ce n'est pas le
///   1er). `avertissement` : `engagements`, chaque engagement
///   touche nomme une fois (`qui`) avec une ligne par ce que le changement entraine pour un envoi parti sur la version
///   d'avant (`consequences`, `engagement::etats::a_refaire`) : un avenant a faire signer, une DPAE a deposer pour une
///   embauche avancee, un bulletin rectificatif, la DSN qui se corrige ; et, pour un contrat au mois dont un mois est
///   parti, `a_partir_du` : la question « a partir de quel mois ? » (`phrase`), ce que chaque mois parti deviendrait
///   (`si`) et ses choix.
///
/// Refus, et rien n'est ecrit : un engagement qui n'est pas valide (il se saisit sur sa carte), importe, d'un mois tenu
/// avant PuppetPaie, d'un exercice clos ; un changement que la norme ne permet plus de declarer
/// (`crate::regularisation::refus_non_declarable`) ; une saisie qui ne se calcule pas.
fn modifier_engagement(ctx: &Contexte, charge: &Value) -> R<Value> {
    let mois = texte_requis(charge, "mois")?;
    let liste = charge.get("engagements").and_then(Value::as_array).cloned().unwrap_or_default();
    if liste.is_empty() {
        return Err(erreur("TypeError", "`engagements` : une liste d'engagements est attendue"));
    }
    let confirme = booleen(charge, "confirme", false);
    let a_partir_du = charge.get("a_partir_du").filter(|v| vrai(v)).map(valeur_en_texte);
    // Le jour du mois choisi ou le changement prend effet, quand on le dit (le 1er sinon).
    let le = match charge.get("a_partir_du_jour").filter(|v| vrai(v)) {
        None => None,
        Some(v) => Some(valeur_en_texte(v).trim().parse::<u32>().map_err(|_| {
            refus(format!("« le {} » : un jour du mois est attendu, de 1 à 31. Rien n'est écrit.", valeur_en_texte(v)))
        })?),
    };
    crate::routes::mois::refuser_si_importe(ctx, mois)?;
    crate::routes::mois::refuser_si_clos(ctx, mois)?;
    crate::routes::mois::refuser_si_avant_la_premiere_embauche(ctx, mois)?;
    let _une_a_la_fois = crate::routes::mois::une_ecriture_a_la_fois();
    let mut rangs = Vec::new();
    let mut saisies = Vec::new();
    let mut nouvelles = Vec::new();
    for e in &liste {
        if e.get("salarie").is_none() {
            let Some(Value::Object(s)) = e.get("saisie") else {
                return Err(erreur("TypeError", "`saisie` : un objet est attendu pour chaque engagement"));
            };
            nouvelles.push(s.clone());
            continue;
        }
        let mut c = e.clone();
        c["mois"] = json!(mois);
        let (data, i) = engagement_designe(ctx, &c)?;
        if !data["engagements"][i].get("valide").is_some_and(Value::is_object) {
            return Err(refus(format!(
                "{} : cet engagement n'est pas validé ; il se saisit sur sa carte, puis « Valider l'engagement ». Rien n'est écrit.",
                valeur_en_texte(&data["engagements"][i]["salarie"])
            )));
        }
        let Some(Value::Object(s)) = e.get("saisie") else {
            return Err(erreur("TypeError", "`saisie` : un objet est attendu pour chaque engagement"));
        };
        rangs.push(i);
        saisies.push(s.clone());
    }
    if rangs.is_empty() {
        return Err(erreur("TypeError", "`engagements` : au moins un engagement valide a designer (`salarie`, `debut`) est attendu"));
    }
    // Ce qu'une DSN deposee dont la limite est passee a declare : la base de ce que la norme permet encore.
    let mut m = crate::routes::etapes::Moteur::nouveau(ctx);
    let declaree = crate::regularisation::modification_de_la_paie(&mut m, ctx, mois)?;
    let base = if declaree.get("confirmer").is_some_and(vrai) {
        crate::regularisation::saisie_declaree(ctx, mois)?.or(engagement::stock::vue_du_mois(&ctx.racine, mois).map_err(Echec::from)?)
    } else {
        None
    };
    let photo = Photo::prendre(&ctx.racine)?;
    let issue = modifier_puis_revalider(ctx, mois, &rangs, &saisies, &nouvelles, base.as_ref(), confirme, a_partir_du.as_deref(), le);
    let faits = match issue {
        Ok(Issue::Ecrite(faits)) => faits,
        Ok(Issue::Rien) => {
            photo.remettre()?;
            return Ok(json!({"change": false}));
        }
        Ok(Issue::Avertir(a)) => {
            photo.remettre()?;
            return Ok(json!({"change": true, "avertissement": a}));
        }
        Err(e) => {
            photo.remettre()?;
            return Err(e);
        }
    };
    let mut avis = Vec::new();
    for (i, _) in &faits {
        let data = engagement::stock::vue_du_mois(&ctx.racine, mois).map_err(Echec::from)?.unwrap_or(Value::Null);
        let e = &data["engagements"][*i];
        if let Some(cle) = engagement::cle_de_l_engagement(ctx, e) {
            crate::routes::fiches::poser_anciennete_a_l_embauche(ctx, &cle, &engagement::debut_brut(e))?;
        }
        avis.extend(avis_de_validation(ctx, mois, *i)?);
    }
    crate::interface::suppression::sortir_de_la_demo(&ctx.racine, mois).map_err(|m| erreur("OSError", m))?;
    Ok(json!({"change": true, "valide": faits.first().map(|(_, le)| le.clone()), "avis": avis}))
}

/// La modification elle-meme, sur les fichiers : chaque engagement se rouvre, prend sa saisie et se revalide ; chaque
/// saisie `nouvelles` s'ajoute au mois et se valide avec eux. Ce qu'elle rend dit si elle reste ecrite
/// (`Issue::Ecrite`) ; sinon la route remet les fichiers.
#[allow(clippy::too_many_arguments)]
fn modifier_puis_revalider(
    ctx: &Contexte,
    mois: &str,
    rangs: &[usize],
    saisies: &[Map<String, Value>],
    nouvelles: &[Map<String, Value>],
    base: Option<&Value>,
    confirme: bool,
    a_partir_du: Option<&str>,
    le: Option<u32>,
) -> R<Issue> {
    for (i, s) in rangs.iter().zip(saisies) {
        engagement::etats::modifier(ctx, mois, *i).map_err(Echec::from)?;
        engagement::stock::poser_la_saisie(ctx, mois, *i, s).map_err(refus)?;
    }
    let mut changes = !nouvelles.is_empty();
    for i in rangs {
        changes |= engagement::etats::a_revalider_rouvert(ctx, mois, *i).map_err(Echec::from)?;
    }
    // Les nouveaux s'ajoutent apres les autres, au bout du fichier du mois, le dernier de son ecran : les rangs des
    // engagements designes ne bougent pas.
    let mut rangs = rangs.to_vec();
    for s in nouvelles {
        rangs.push(engagement::stock::ajouter_au_mois(ctx, mois, s).map_err(refus)?);
    }
    let rangs = &rangs[..];
    if !changes {
        return Ok(Issue::Rien);
    }
    if let Some(base) = base {
        let nouvelle = engagement::stock::vue_du_mois(&ctx.racine, mois).map_err(Echec::from)?.unwrap_or(Value::Null);
        if let Some(texte) = crate::regularisation::refus_non_declarable(ctx, mois, base, &nouvelle)? {
            return Err(erreur(crate::regularisation::CLASSE_NON_DECLARABLE, texte));
        }
    }
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs).map_err(Echec::from)?;
    // L'AVERTISSEMENT : chaque engagement que le changement touche, nomme une fois (`qui`), et une ligne par ce que le
    // changement entraine pour lui (`consequences`).
    let mut touches: Vec<(String, Vec<String>)> = Vec::new();
    let mut dire = |qui: String, lignes: Vec<String>| match touches.iter_mut().find(|(q, _)| *q == qui) {
        Some((_, l)) => l.extend(lignes),
        None => touches.push((qui, lignes)),
    };
    // « A PARTIR DE QUEL MOIS ? » : un contrat au mois dont un mois est parti et dont les termes changent. Sans reponse
    // parmi ses choix, la question vient dans l'avertissement ; un seul choix vaut reponse.
    let mut question = Value::Null;
    for i in rangs {
        let Some(q) = engagement::etats::question_a_partir_du(ctx, mois, *i, &departs).map_err(Echec::from)? else { continue };
        let depuis = a_partir_du.filter(|m| q.choix.iter().any(|c| c == m)).map(String::from).or_else(|| (q.choix.len() == 1).then(|| q.choix[0].clone()));
        match depuis {
            Some(d) => engagement::etats::poser_a_partir_du(ctx, mois, *i, &d, le, &departs).map_err(refus)?,
            None => {
                let o = engagement::stock::objet_du_mois(&ctx.racine, mois, *i).map_err(Echec::from)?;
                let avenant = o.as_ref().is_some_and(|o| contrat_parti(ctx, o, &departs));
                dire(q.qui.clone(), if avenant { vec![UN_AVENANT.to_string()] } else { Vec::new() });
                let choix: Vec<Value> =
                    q.choix.iter().map(|m| json!({"mois": m, "libelle": engagement::libelle_mois(m), "partis": q.partis.contains(m)})).collect();
                question = json!({"phrase": q.phrase(), "si": ce_qui_arrive_aux_mois_partis(ctx, &q)?, "choix": choix});
            }
        }
    }
    let en_valeur = |t: Vec<(String, Vec<String>)>| -> Value {
        Value::Array(t.into_iter().map(|(qui, c)| json!({"qui": qui, "consequences": c})).collect())
    };
    if !question.is_null() {
        return Ok(Issue::Avertir(json!({"engagements": en_valeur(touches), "a_partir_du": question})));
    }
    let mut faits = Vec::new();
    for i in rangs {
        let v = engagement::etats::valider(ctx, mois, *i, &departs).map_err(|m| {
            let data = engagement::stock::vue_a_l_ecran(&ctx.racine, mois).ok().flatten().unwrap_or(Value::Null);
            crate::routes::mois::refus_du_calcul(ctx, &data, m)
        })?;
        let lignes = ce_que_le_changement_entraine(ctx, &v, &departs)?;
        if !lignes.is_empty() {
            dire(engagement::etats::qui_de(ctx, v.objet()), lignes);
        }
        faits.push((*i, v.le().to_string()));
    }
    if !confirme && !touches.is_empty() {
        return Ok(Issue::Avertir(json!({"engagements": en_valeur(touches), "a_partir_du": Value::Null})));
    }
    Ok(Issue::Ecrite(faits))
}

/// Le contrat (ou un avenant) de l'engagement `o` est-il parti ?
fn contrat_parti(ctx: &Contexte, o: &engagement::stock::Objet, departs: &engagement::departs::Departs) -> bool {
    let b = Value::Object(o.brut.clone());
    let (Some(cle), Some(numero)) = (engagement::cle_de_l_engagement(ctx, &b), b.get("numero").filter(|n| vrai(n)).map(valeur_en_texte)) else {
        return false;
    };
    departs.de(&cle, &numero).iter().any(|d| d.envoi == engagement::departs::Envoi::Contrat)
}

/// Ce qu'un contrat parti que le changement modifie entraine.
const UN_AVENANT: &str = "un avenant à faire signer";

/// CE QUE LE CHANGEMENT ENTRAINE, une ligne par consequence, pour l'engagement revalide `v` : chaque envoi parti sur une
/// version qui n'est plus la sienne (`engagement::etats::a_refaire`, sans l'avenant d'un contrat rompu :
/// `crate::routes::contrats::seule_la_rupture`). Une embauche reportee ne refait pas la DPAE
/// (`crate::regularisation::dpae_d_une_date_changee`).
fn ce_que_le_changement_entraine(ctx: &Contexte, v: &engagement::etats::Valide, departs: &engagement::departs::Departs) -> R<Vec<String>> {
    use engagement::etats::ARefaire;
    let saisie = |x: Option<&Value>| x.and_then(|x| x.get("saisie")).cloned().unwrap_or(Value::Null);
    let presente = saisie(v.objet().brut.get("valide"));
    let mut m = crate::routes::etapes::Moteur::nouveau(ctx);
    let mut out = Vec::new();
    for a in engagement::etats::a_refaire(ctx, v, departs) {
        match a {
            ARefaire::Avenant { depuis } => {
                if !crate::routes::contrats::seule_la_rupture(&saisie(v.version_numero(depuis)), &presente) {
                    out.push(UN_AVENANT.to_string());
                }
            }
            ARefaire::Dpae { debut_declare, .. } => {
                let (rien_a_refaire, phrase) = crate::regularisation::dpae_d_une_date_changee(&debut_declare, &engagement::debut_brut(&presente));
                if !rien_a_refaire {
                    out.push(phrase);
                }
            }
            ARefaire::Bulletin { mois, .. } => out.push(un_bulletin_rectificatif(&mois)),
            ARefaire::Dsn { mois, .. } => out.push(la_dsn_qui_change(&mut m, ctx, &mois)?),
        }
    }
    Ok(out)
}

/// « un bulletin rectificatif de janvier 2026 » : la paie partie de `mois` se refait.
fn un_bulletin_rectificatif(mois: &str) -> String {
    format!("un bulletin rectificatif {}", crate::regularisation::de_mois_en_clair(mois))
}

/// Ce qui arrive a la DSN deposee de `mois` quand sa paie change, comme toute modification d'un mois declare
/// (`crate::regularisation::modification_de_la_paie`) : un annule-et-remplace jusqu'a la limite, ensuite la correction
/// dans la DSN qui porte (`crate::regularisation::dsn_qui_porte`).
fn la_dsn_qui_change(m: &mut crate::routes::etapes::Moteur, ctx: &Contexte, mois: &str) -> R<String> {
    let de = crate::regularisation::de_mois_en_clair(mois);
    let d = crate::regularisation::modification_de_la_paie(m, ctx, mois)?;
    Ok(if d.get("confirmer").is_some_and(vrai) {
        format!(
            "la correction dans la DSN {} (celle {}, déposée, ne se remplace plus)",
            crate::regularisation::de_mois_en_clair(&valeur_en_texte(&d["porteur"])),
            de
        )
    } else if d.is_null() {
        format!("la DSN {} à corriger", de)
    } else {
        let limite = socle::date::Jour::lire(&valeur_en_texte(&d["limite"])).map(|j| j.jj_mm_aaaa()).unwrap_or_default();
        format!("un annule-et-remplace de la DSN {}, admis jusqu'au {} à minuit", de, limite)
    })
}

/// CE QUI ARRIVE A UN MOIS PARTI que le changement toucherait, pour chaque mois parti parmi les choix de la question
/// « a partir de quel mois ? » : sa paie se refait (bulletin rectificatif), et sa DSN, si elle est deposee, se corrige
/// (`la_dsn_qui_change`).
fn ce_qui_arrive_aux_mois_partis(ctx: &Contexte, q: &engagement::etats::APartirDu) -> R<Vec<String>> {
    let mut m = crate::routes::etapes::Moteur::nouveau(ctx);
    let mut out = Vec::new();
    for mois in q.choix.iter().filter(|c| q.partis.contains(c)) {
        let d = crate::regularisation::modification_de_la_paie(&mut m, ctx, mois)?;
        let dsn = if d.is_null() { String::new() } else { format!(" ; {}", la_dsn_qui_change(&mut m, ctx, mois)?) };
        out.push(format!("dès {} : {}{}", engagement::libelle_mois(mois), un_bulletin_rectificatif(mois), dsn));
    }
    Ok(out)
}

/// CE QUE LA VALIDATION DIT SANS REFUSER, pour l'engagement de rang `i` de la saisie de `mois` : un contrat que le
/// logiciel fait signer (`engagement::contrat_a_signer`) part a la signature par courriel, a l'adresse de la fiche ; sans
/// elle, il ne partira pas (`compagnie::signature`, qui le refuse au depart). Il s'imprime, ou le courriel se saisit.
fn avis_de_validation(ctx: &Contexte, mois: &str, i: usize) -> R<Vec<Value>> {
    let Some(data) = engagement::stock::vue_du_mois(&ctx.racine, mois).map_err(Echec::from)? else { return Ok(Vec::new()) };
    let e = &data["engagements"][i];
    let Some(cle) = engagement::cle_de_l_engagement(ctx, e) else { return Ok(Vec::new()) };
    let nature = droit::natures::nature(e.get("nature").and_then(Value::as_str)).map_err(Echec::from)?;
    let numero = e.get("numero").filter(|n| vrai(n)).map(valeur_en_texte).unwrap_or_default();
    if nature.modele_contrat().is_none() || engagement::importe::vient_d_un_ancien_logiciel(&ctx.racine, &cle, &engagement::debut_brut(e), &numero) {
        return Ok(Vec::new());
    }
    let fiche = ctx.salarie(&cle).map_err(Echec::from)?;
    if fiche.get("email").map(valeur_en_texte).is_some_and(|m| !m.trim().is_empty()) {
        return Ok(Vec::new());
    }
    let texte = |c: &str| fiche.get(c).filter(|x| vrai(x)).map(valeur_en_texte);
    let qui = [texte("nom"), texte("prenom_affichage").or_else(|| texte("prenom"))].into_iter().flatten().collect::<Vec<_>>().join(" ");
    // Orange : a faire, et la compagnie peut le faire.
    Ok(vec![json!({"couleur": "orange", "phrase": format!(
        "{} n'a pas de courriel dans sa fiche : son contrat ne pourra pas partir à la signature par courriel. Imprimez-le \
         pour le faire signer, ou saisissez son courriel (étape « Contrats » de la paie, ou sa fiche dans Configuration › \
         Salariés).",
        if qui.is_empty() { cle } else { qui }
    )})])
}

/// L'ENGAGEMENT QUE LA PAGE DESIGNE dans la saisie enregistree de `mois` : `salarie`, `debut`, et, si deux contrats du
/// salarie commencent ce jour-la, `numero` ou `rang` (son rang parmi eux). Rend la saisie du mois et son rang dans
/// l'ecran. Refus : un engagement absent de la saisie enregistree (agir sur ce qui n'est pas enregistre poserait un etat
/// sur un formulaire qu'un rechargement effacerait).
fn engagement_designe(ctx: &Contexte, charge: &Value) -> R<(Value, usize)> {
    let (mois, salarie, debut) = (texte_requis(charge, "mois")?, texte_requis(charge, "salarie")?, texte_requis(charge, "debut")?);
    // UN ENGAGEMENT IMPORTE se lit : aucun geste ne le valide, ne le modifie ni ne le retire.
    if engagement::importe::celui_de(&ctx.racine, mois, salarie, debut).map_err(Echec::from)?.is_some() {
        return Err(refus(engagement::importe::refus(salarie, debut)));
    }
    let Some(data) = engagement::stock::vue_du_mois(&ctx.racine, mois).map_err(Echec::from)? else {
        return Err(refus(format!("Le mois {} n'est pas enregistre : il n'y a pas d'engagement a designer. Enregistrer la saisie d'abord.", mois)));
    };
    let rangs = crate::routes::fiches::identifiants(ctx, &data).map_err(Echec::from)?;
    // LE CONTRAT, PAS LE SALARIE DU JOUR (`jumeaux`) : deux contrats du meme salarie peuvent commencer le meme jour.
    let numero = charge.get("numero").filter(|v| vrai(v)).map(valeur_en_texte);
    let rang = charge.get("rang").and_then(Value::as_u64);
    let du_jour: Vec<usize> = (0..rangs.len()).filter(|&j| rangs[j].0 == salarie && rangs[j].1 == debut).collect();
    let par_numero = numero.as_ref().and_then(|n| du_jour.iter().copied().find(|&j| rangs[j].2.as_ref() == Some(n)));
    let i = match (par_numero, du_jour.as_slice()) {
        (_, []) => {
            return Err(refus(format!(
                "Aucun engagement de {} commencant le {} dans la saisie enregistree de {}. Enregistrer la saisie avant de valider.",
                salarie, debut, mois
            )))
        }
        (Some(j), _) => j,
        // Un seul contrat ce jour-la : c'est lui, quel que soit le numero que l'ecran lui prete.
        (None, [j]) => *j,
        (None, plusieurs) => match rang.and_then(|r| plusieurs.get(r as usize)) {
            Some(j) if numero.is_none() || rangs[*j].2.is_none() => *j,
            _ => {
                let lesquels: Vec<String> = plusieurs.iter().map(|&j| rangs[j].2.clone().unwrap_or_else(|| "sans numero".into())).collect();
                return Err(refus(format!(
                    "{} a {} contrats qui commencent le {} dans la saisie de {} ({}){} : dire lequel valider (son numero, ou son rang parmi eux). Rien n'est ecrit.",
                    salarie,
                    plusieurs.len(),
                    debut,
                    mois,
                    lesquels.join(", "),
                    numero.as_ref().map_or(String::new(), |n| format!(", aucun sous le n° {}", n))
                )));
            }
        },
    };
    Ok((data, i))
}

// ------------------------------------------------------------------ retirer-engagement

/// La classe du refus de supprimer un engagement dont quelque chose est parti : la page montre la phrase, et renvoie la
/// demande avec `non_actif` si l'utilisateur veut qu'il ne soit plus actif.
pub const CLASSE_ENGAGEMENT_PARTI: &str = "EngagementParti";

/// « Retirer cet engagement » : l'engagement se designe comme pour `valider-engagement` (`engagement_designe`). Tant
/// que rien n'est parti de lui, il se supprime, meme valide, et rend son numero (`engagement::etats::supprimer`). Des
/// qu'un envoi est parti, le moteur refuse (`CLASSE_ENGAGEMENT_PARTI`, la phrase dit ce qui est parti) ; avec
/// `non_actif`, il devient non actif (`engagement::etats::desactiver`) et la reponse dit ce que cela rend a faire
/// (`a_faire`, en phrases : la DPAE a annuler, la DSN d'un mois a corriger). Rien ne part de lui-meme.
fn retirer_engagement(ctx: &Contexte, charge: &Value) -> R<Value> {
    let mois = texte_requis(charge, "mois")?;
    // Un engagement auquel il manque un fait n'a jamais ete valide : rien n'est parti de lui, il s'efface.
    let designe = match designe_a_l_ecran(ctx, charge)? {
        Designe::ACompleter(o) => {
            engagement::stock::effacer(&ctx.racine, &o).map_err(Echec::from)?;
            return Ok(json!({"retire": "supprime"}));
        }
        Designe::DansLaVue(c) => c,
    };
    let charge = &designe;
    let (_, i) = engagement_designe(ctx, charge)?;
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs).map_err(Echec::from)?;
    if !booleen(charge, "non_actif", false) {
        return match engagement::etats::supprimer(ctx, mois, i, &departs) {
            Ok(()) => Ok(json!({"retire": "supprime"})),
            Err(m) => Err(erreur(CLASSE_ENGAGEMENT_PARTI, m)),
        };
    }
    let n = engagement::etats::desactiver(ctx, mois, i, &departs).map_err(Echec::from)?;
    Ok(json!({"retire": "non_actif", "le": n.le(), "a_faire": a_faire_en_phrases(&n.a_refaire(ctx, &departs))}))
}

/// Ce qu'un engagement non actif rend a faire, en phrases pour l'ecran.
pub(crate) fn a_faire_en_phrases(a: &[engagement::etats::ARefaire]) -> Vec<String> {
    use engagement::etats::ARefaire;
    a.iter()
        .filter_map(|x| match x {
            ARefaire::Dsn { mois, .. } => Some(format!("DSN de {} à corriger", engagement::libelle_mois(mois))),
            _ => None,
        })
        .collect()
}

// ------------------------------------------------------------------ contrat-recu




// ------------------------------------------------------------------ retenir-logiciel


// ------------------------------------------------------------------ casc-regularise


// ------------------------------------------------------------------ exporter




