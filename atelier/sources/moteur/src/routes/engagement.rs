//! Les routes de l'engagement : le valider, ou le remettre en saisie (« Modifier l'engagement… ») (`valider-engagement`),
//! et le retirer (`retirer-engagement` : supprime s'il n'en est rien parti, sinon rendu non actif). Un refus garde sa
//! phrase quand elle est utile a la page.


use std::collections::HashMap;

use serde_json::{json, Value};

use droit::contexte::Contexte;
use socle::echec::{erreur, Echec, Refus, R};
use socle::fichiers::vrai;
use socle::valeur::valeur_en_texte;



/// Les routes de ce module. `etat-par-engagement` se lit dans l'etat garde (`etat_garde`), que la fenetre refait apres
/// chaque ecriture.
pub const ROUTES: &[&str] = &["valider-engagement", "retirer-engagement"];

/// Celles qui ecrivent.
pub const ECRITURES: &[&str] = ROUTES;

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "valider-engagement" => valider_engagement(ctx, charge),
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
    /// Un engagement auquel il manque un fait (`engagement::stock::A_COMPLETER`), tel que l'ecran le montre (sa saisie,
    /// `Objet::a_l_ecran_tel_quel`).
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
            let e = Value::Object(o.a_l_ecran_tel_quel().brut);
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

/// La classe du refus qui pose la question avant de modifier un engagement dont des documents sont partis
/// (`engagement::etats::QUESTION_DOCUMENTS_PARTIS`) : la page la montre, et renvoie la demande avec `confirme`.
pub const CLASSE_DOCUMENTS_PARTIS: &str = "DocumentsPartis";

/// « Valider l'engagement » (`valide` vrai, par defaut) ou « Modifier… » (`valide` faux). L'engagement se designe
/// dans la saisie enregistree de `mois` : `salarie`, `debut`, et, si deux contrats du salarie commencent ce jour-la,
/// `numero` ou `rang` (son rang parmi eux). Refus : un engagement absent de la saisie enregistree (valider ce qui
/// n'est pas enregistre poserait une pastille verte sur un formulaire qu'un rechargement effacerait).
///
/// VALIDER ecrit les paies de l'engagement, une par mois qu'il couvre (`engagement::etats::valider`) ; un engagement
/// deja valide le reste, tel quel. MODIFIER le remet en saisie (`engagement::etats::modifier`) ; si les documents d'un
/// de ses mois sont partis, le moteur pose d'abord sa question (`CLASSE_DOCUMENTS_PARTIS`) et ne modifie qu'avec
/// `confirme`.
fn valider_engagement(ctx: &Contexte, charge: &Value) -> R<Value> {
    let (mois, salarie, debut) = (texte_requis(charge, "mois")?, texte_requis(charge, "salarie")?, texte_requis(charge, "debut")?);
    let valide = booleen(charge, "valide", true);
    let confirme = booleen(charge, "confirme", false);
    // Il lui manque un fait : la validation dit lequel, et rien n'est ecrit. Il n'a jamais ete valide : rien a modifier.
    // Ce qui manquait se relit d'abord : le fait a pu se dire depuis, sur sa fiche.
    crate::routes::mois::relire_les_manques(ctx)?;
    let charge = match designe_a_l_ecran(ctx, charge)? {
        Designe::ACompleter(o) => {
            if !valide {
                return Ok(json!({"salarie": salarie, "debut": debut, "valide": Value::Null}));
            }
            let manque = o.a_completer().unwrap_or_default();
            let data = engagement::stock::vue_a_l_ecran(&ctx.racine, mois).map_err(Echec::from)?.unwrap_or(Value::Null);
            return Err(crate::routes::mois::refus_du_calcul(ctx, &data, manque));
        }
        Designe::DansLaVue(c) => c,
    };
    let (data, i) = engagement_designe(ctx, &charge)?;
    let deja = data["engagements"][i].get("valide").and_then(|v| v.get("le")).cloned();
    let rendu = if valide {
        let le = match deja {
            Some(le) => le,
            None => {
                let le = engagement::etats::valider(ctx, mois, i).map_err(|m| crate::routes::mois::refus_du_calcul(ctx, &data, m))?.le().to_string();
                let cle = engagement::cle_de_l_engagement(ctx, &data["engagements"][i]).unwrap_or_else(|| salarie.to_string());
                crate::routes::fiches::poser_anciennete_a_l_embauche(ctx, &cle, debut)?;
                Value::String(le)
            }
        };
        let avis = avis_de_validation(ctx, mois, i)?;
        return Ok(json!({"salarie": salarie, "debut": debut, "valide": le, "avis": avis}));
    } else {
        if !confirme {
            if let Some(q) = engagement::etats::question_avant_de_modifier(ctx, mois, i).map_err(Echec::from)? {
                return Err(erreur(CLASSE_DOCUMENTS_PARTIS, q));
            }
        }
        engagement::etats::modifier(ctx, mois, i).map_err(Echec::from)?;
        Value::Null
    };
    Ok(json!({"salarie": salarie, "debut": debut, "valide": rendu}))
}

/// CE QUE LA VALIDATION DIT SANS REFUSER, pour l'engagement de rang `i` de la saisie de `mois` : un contrat que le
/// logiciel fait signer (`engagement::contrat_a_signer`) part a la signature par courriel, a l'adresse de la fiche ; sans
/// elle, il ne partira pas (`envois::envois_signature`, qui le refuse au depart). Il s'imprime, ou le courriel se saisit.
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
    // Un engagement auquel il manque un fait et qui n'a jamais ete valide : rien n'est parti de lui, il s'efface. Deja
    // valide une fois, il se retire comme les autres, tel qu'il est ecrit.
    let designe = match designe_a_l_ecran(ctx, charge)? {
        Designe::ACompleter(o) if o.jamais_valide() => {
            engagement::stock::effacer(&ctx.racine, &o).map_err(Echec::from)?;
            return Ok(json!({"retire": "supprime"}));
        }
        Designe::ACompleter(o) => tel_qu_il_est_ecrit(ctx, charge, &o)?,
        Designe::DansLaVue(c) => c,
    };
    let charge = &designe;
    let (_, i) = engagement_designe(ctx, charge)?;
    if !booleen(charge, "non_actif", false) {
        return match engagement::etats::supprimer(ctx, mois, i) {
            Ok(()) => Ok(json!({"retire": "supprime"})),
            Err(m) => Err(erreur(CLASSE_ENGAGEMENT_PARTI, m)),
        };
    }
    let n = engagement::etats::desactiver(ctx, mois, i).map_err(Echec::from)?;
    let departs = envois::journal::departs(&ctx.racine).map_err(Echec::from)?;
    Ok(json!({"retire": "non_actif", "le": n.le(), "a_faire": a_faire_en_phrases(&n.a_refaire(ctx, &departs))}))
}

/// La charge qui designe dans `vue_du_mois` l'engagement deja valide une fois et remis en saisie `o` : son debut et son
/// rang tels qu'il est ecrit, pas tels que sa saisie gardee a cote les montre.
fn tel_qu_il_est_ecrit(ctx: &Contexte, charge: &Value, o: &engagement::stock::Objet) -> R<Value> {
    let mois = texte_requis(charge, "mois")?;
    let ident = |x: &engagement::stock::Objet| {
        let e = Value::Object(x.brut.clone());
        (engagement::cle_de_l_engagement(ctx, &e), engagement::debut_brut(&e))
    };
    let (cle, debut) = ident(o);
    let objets = engagement::stock::objets_a_l_ecran(&ctx.racine, mois).map_err(Echec::from)?;
    let rang = objets
        .iter()
        .filter(|x| !x.est_a_completer() && ident(x) == (cle.clone(), debut.clone()))
        .position(|x| x.fichier == o.fichier && x.rang == o.rang)
        .unwrap_or(0);
    let mut c = charge.clone();
    c["debut"] = json!(debut);
    c["rang"] = json!(rang);
    Ok(c)
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




