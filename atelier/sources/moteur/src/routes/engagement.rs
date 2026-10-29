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
    let (data, i) = engagement_designe(ctx, charge)?;
    let deja = data["engagements"][i].get("valide").and_then(|v| v.get("le")).cloned();
    let rendu = if valide {
        match deja {
            Some(le) => le,
            None => Value::String(engagement::etats::valider(ctx, mois, i).map_err(|m| crate::routes::mois::refus_du_calcul(ctx, &data, m))?.le().to_string()),
        }
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




