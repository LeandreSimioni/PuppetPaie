//! Les routes de l'engagement, celles de la carte : la valider, la modifier, la retirer. Chacune rend la phrase que
//! l'on lit ; un refus dit quoi corriger et n'ecrit rien.

use std::collections::HashMap;

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use socle::echec::{erreur, Echec, Refus, R};
use socle::fichiers::vrai;
use socle::valeur::valeur_en_texte;

use crate::routes::catalogue::{Acte, Champ, Fiche, Refus as RefusDit, FORME, MOIS};

const IDENTITE: Champ = Champ {
    nom: "engagement",
    sens: "l'identité de la carte, telle que le moteur l'a rendue : {salarie, debut, rang}",
    avance: false,
};
const SAISIE: Champ = Champ {
    nom: "saisie",
    sens: "ce que la carte porte : salarie (la clé de sa fiche), spectacle, lignes [{rubrique, quantite, taux, dates}], nature…",
    avance: false,
};
pub(crate) const SAISIE_INCOMPLETE: RefusDit = RefusDit { classe: "SaisieIncomplete", corriger: "le fait que la phrase nomme, sur la carte ou la fiche du salarié" };
const OPERATION: RefusDit = RefusDit {
    classe: "ErreurOperation",
    corriger: "ce que la phrase dit : un mois importé ou clos, une carte que l'écran ne montre pas, une saisie hors de ses bornes",
};

/// Les routes de ce module, et leurs fiches. Toutes ecrivent.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        nom: "valider-engagement",
        geste: "Valider l'engagement d'une carte : l'écrire et calculer ses paies.",
        charge: &[MOIS, Champ { nom: "engagement", sens: "l'identité de la carte ; absente pour un engagement neuf", avance: false }, SAISIE],
        exemple: r#"{"mois":"2026-11","saisie":{"salarie":"FULLER","spectacle":"le-banc-des-menteurs","lignes":[{"rubrique":"cachet","quantite":1,"taux":150,"dates":["2026-11-27"]}]}}"#,
        rend: "engagement (l'identité de la carte), phrase et couleur (ce qui se lit : validé, et ce qui ne pourra pas partir)",
        refus: &[SAISIE_INCOMPLETE, OPERATION, FORME],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 320,
    },
    Fiche {
        nom: "modifier-engagement",
        geste: "Modifier un engagement validé : écrire ce que sa carte ouverte montre.",
        charge: &[
            MOIS,
            IDENTITE,
            SAISIE,
            Champ { nom: "confirme", sens: "vrai : l'avertissement rendu a été lu et accepté", avance: false },
            Champ { nom: "a_partir_du", sens: "le mois choisi parmi ceux que l'avertissement propose (CDD, CDI)", avance: true },
            Champ { nom: "a_partir_du_jour", sens: "son jour, quand ce n'est pas le 1er", avance: true },
        ],
        exemple: r#"{"mois":"2026-11","engagement":{"salarie":"FULLER","debut":"2026-11-27","rang":0},"saisie":{"salarie":"FULLER","spectacle":"le-banc-des-menteurs","lignes":[{"rubrique":"cachet","quantite":1,"taux":160,"dates":["2026-11-27"]}]}}"#,
        rend: "rien n'a changé : change faux ; ce qui est parti le demande : avertissement (à montrer, puis renvoyer avec confirme), rien n'est écrit ; écrit : change, phrase et couleur",
        refus: &[
            SAISIE_INCOMPLETE,
            OPERATION,
            RefusDit { classe: "ModificationNonDeclarable", corriger: "rien : la norme ne permet plus de déclarer ce changement ; la phrase dit pourquoi" },
            FORME,
        ],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 320,
    },
    Fiche {
        nom: "retirer-engagement",
        geste: "Retirer l'engagement d'une carte, ou le rendre non actif si un envoi est parti.",
        charge: &[MOIS, IDENTITE, Champ { nom: "confirme", sens: "vrai : l'avertissement rendu a été lu, et l'engagement devient non actif", avance: false }],
        exemple: r#"{"mois":"2026-11","engagement":{"salarie":"FULLER","debut":"2026-11-27","rang":0},"confirme":true}"#,
        rend: "phrase : retiré, ou non actif et ce que cela rend à faire ; un envoi est parti : avertissement (à montrer, puis renvoyer avec confirme), rien n'est écrit",
        refus: &[OPERATION, FORME],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 200,
    },
];

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

/// La valeur de `cle` de la charge, vraie ou fausse ; `defaut` si elle manque.
pub(super) fn booleen(charge: &Value, cle: &str, defaut: bool) -> bool {
    charge.get(cle).map_or(defaut, vrai)
}

/// L'heure de Paris, a la seconde.
pub(super) fn maintenant() -> String {
    let t = socle::temps::secondes();
    socle::paris::iso_secondes(t)
}

/// L'ENGAGEMENT QU'UNE CARTE MONTRE : celui que designe `engagement` dans la charge, l'identite que le moteur a rendue a
/// la carte (`engagement::stock::identites`). Refus : une identite que l'ecran de `mois` ne montre pas.
fn designe(ctx: &Contexte, mois: &str, charge: &Value) -> R<(usize, engagement::stock::Objet)> {
    let id = charge.get("engagement").ok_or_else(|| erreur("TypeError", "`engagement` : l'identité de la carte est attendue"))?;
    engagement::stock::designe(&ctx.racine, mois, id).map_err(refus)
}


// ------------------------------------------------------------------ valider-engagement


/// « VALIDER L'ENGAGEMENT », UN GESTE : la carte s'ecrit (`saisie`, et `engagement`, son identite, absente
/// pour un engagement neuf : `crate::routes::mois::ecrire_une_carte`), puis l'engagement ecrit se valide : il ecrit ses
/// paies, une par mois qu'il couvre (`engagement::etats::valider`). Le tout se fait dans un brouillon
/// (`engagement::stock::Brouillon`) : il lui manque un fait, la validation refuse en disant lequel, et rien n'est ecrit.
/// Un engagement valide ne se valide pas (il se modifie par `modifier-engagement`).
fn valider_engagement(ctx: &Contexte, charge: &Value) -> R<Value> {
    let mois = texte_requis(charge, "mois")?;
    // Valider ne se defait pas : un engagement valide se modifie (`modifier-engagement`) ou se retire.
    if charge.get("valide").is_some() {
        return Err(erreur("TypeError", "`valide` : valider-engagement valide ; un engagement valide se modifie ou se retire. Rien n'est écrit."));
    }
    let _verrou = crate::routes::mois::avant_d_ecrire(ctx, mois)?;
    let brouillon = engagement::stock::Brouillon::ouvrir(&ctx.racine);
    let id = crate::routes::mois::ecrire_une_carte(ctx, mois, charge)?;
    let (i, _) = engagement::stock::designe(&ctx.racine, mois, &id).map_err(Echec::from)?;
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs).map_err(Echec::from)?;
    let data = engagement::stock::vue_du_mois(&ctx.racine, mois).map_err(Echec::from)?.unwrap_or(Value::Null);
    if let Some(manque) = crate::routes::mois::manques_de_la_saisie(ctx, &data).swap_remove(i) {
        return Err(crate::routes::mois::refus_du_calcul(ctx, &data, manque));
    }
    let v = engagement::etats::valider(ctx, mois, i, &departs).map_err(|m| crate::routes::mois::refus_du_calcul(ctx, &data, m))?;
    brouillon.ecrire().map_err(Echec::from)?;
    let e = &data["engagements"][i];
    if let Some(cle) = engagement::cle_de_l_engagement(ctx, e) {
        crate::routes::fiches::poser_anciennete_a_l_embauche(ctx, &cle, &engagement::debut_brut(e))?;
    }
    let (phrase, couleur) = phrase_de_validation("Engagement validé.", avis_de_validation(ctx, &v, &departs)?);
    Ok(json!({"engagement": id, "phrase": phrase, "couleur": couleur}))
}

/// La phrase que l'on lit apres une validation : `fait`, puis l'avis qui l'accompagne ; sa couleur, orange quand un avis
/// dit quelque chose a faire.
fn phrase_de_validation(fait: &str, avis: Option<String>) -> (String, Value) {
    match avis {
        Some(a) => (format!("{} {}", fait, a), json!("orange")),
        None => (fait.to_string(), Value::Null),
    }
}

// ------------------------------------------------------------------ modifier-engagement

/// Ce que la modification a donne.
enum Issue {
    /// Rien n'a change.
    Rien,
    /// Ce qui est parti le demande : l'avertissement, a confirmer.
    Avertir(Value),
    /// Ecrite : l'engagement revalide.
    Ecrite(engagement::etats::Valide),
}

/// « MODIFIER L'ENGAGEMENT » : les valeurs que la carte ouverte d'un engagement valide montre, ecrites d'un geste.
/// La carte se designe par son identite (`engagement`, que le moteur lui a rendue) ; `saisie` est ce que
/// sa carte ecrit pour `mois`. Une carte porte un salarie : elle modifie son engagement, et lui seul.
///
/// - Rien n'a change : rien ne s'ecrit, la reponse le dit (`change` faux).
/// - Quelque chose a change, et rien de ce qui est parti ne le demande : la modification s'ecrit, l'engagement reste valide.
/// - Ce qui est parti le demande : la reponse porte l'avertissement, et rien n'est ecrit ; la page le montre et renvoie
///   la demande avec `confirme` (et `a_partir_du`, le mois choisi, `a_partir_du_jour`, son jour quand ce n'est pas le
///   1er). `avertissement` : `engagements`, l'engagement touche nomme (`qui`) avec une ligne par ce que le changement
///   entraine pour un envoi parti sur la version d'avant (`consequences`, `engagement::etats::a_refaire`) : un avenant a
///   faire signer, une DPAE a deposer pour une embauche avancee, un bulletin rectificatif, la DSN qui se corrige ; et,
///   pour un contrat au mois dont un mois est parti, `a_partir_du` : la question « a partir de quel mois ? » (`phrase`),
///   ce que chaque mois parti deviendrait (`si`) et ses choix.
///
/// Refus, et rien n'est ecrit : un engagement qui n'est pas valide (il se saisit sur sa carte), importe, d'un mois tenu
/// avant PuppetPaie, d'un exercice clos ; un changement que la norme ne permet plus de declarer
/// (`crate::regularisation::refus_non_declarable`) ; une saisie qui ne se calcule pas.
fn modifier_engagement(ctx: &Contexte, charge: &Value) -> R<Value> {
    let mois = texte_requis(charge, "mois")?;
    let Some(Value::Object(saisie)) = charge.get("saisie") else {
        return Err(erreur("TypeError", "`saisie` : un objet est attendu"));
    };
    let confirme = booleen(charge, "confirme", false);
    let a_partir_du = charge.get("a_partir_du").filter(|v| vrai(v)).map(valeur_en_texte);
    // Le jour du mois choisi ou le changement prend effet, quand on le dit (le 1er sinon).
    let le = match charge.get("a_partir_du_jour").filter(|v| vrai(v)) {
        None => None,
        Some(v) => Some(valeur_en_texte(v).trim().parse::<u32>().map_err(|_| {
            refus(format!("« le {} » : un jour du mois est attendu, de 1 à 31. Rien n'est écrit.", valeur_en_texte(v)))
        })?),
    };
    let _verrou = crate::routes::mois::avant_d_ecrire(ctx, mois)?;
    let (i, o) = designe(ctx, mois, charge)?;
    if !o.est_valide() {
        return Err(refus(format!(
            "{} : cet engagement n'est pas validé ; il se saisit sur sa carte, puis « Valider l'engagement ». Rien n'est écrit.",
            engagement::etats::qui_de(ctx, &o)
        )));
    }
    // LA MODIFICATION SE FAIT DANS UN BROUILLON (`engagement::stock::Brouillon`) : calculee en memoire, elle ne s'ecrit
    // sur le disque que quand elle est faite ; rien n'a change, un avertissement a confirmer, un refus : rien n'est ecrit.
    let brouillon = engagement::stock::Brouillon::ouvrir(&ctx.racine);
    let valide = match modifier_puis_revalider(ctx, mois, i, &o, saisie, confirme, a_partir_du.as_deref(), le)? {
        Issue::Ecrite(valide) => valide,
        Issue::Rien => return Ok(json!({"change": false})),
        Issue::Avertir(a) => return Ok(json!({"avertissement": a})),
    };
    brouillon.ecrire().map_err(Echec::from)?;
    let data = engagement::stock::vue_du_mois(&ctx.racine, mois).map_err(Echec::from)?.unwrap_or(Value::Null);
    let e = &data["engagements"][i];
    if let Some(cle) = engagement::cle_de_l_engagement(ctx, e) {
        crate::routes::fiches::poser_anciennete_a_l_embauche(ctx, &cle, &engagement::debut_brut(e))?;
    }
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs).map_err(Echec::from)?;
    let (phrase, couleur) = phrase_de_validation("Engagement modifié.", avis_de_validation(ctx, &valide, &departs)?);
    crate::interface::suppression::sortir_de_la_demo(&ctx.racine, mois).map_err(|m| erreur("OSError", m))?;
    Ok(json!({"change": true, "phrase": phrase, "couleur": couleur}))
}

/// La modification elle-meme, dans le brouillon ouvert : l'engagement `o`, de rang `i`, se rouvre, prend sa saisie et
/// se revalide. Ce qu'elle rend dit si elle s'ecrit (`Issue::Ecrite`).
#[allow(clippy::too_many_arguments)]
fn modifier_puis_revalider(
    ctx: &Contexte,
    mois: &str,
    i: usize,
    o: &engagement::stock::Objet,
    saisie: &Map<String, Value>,
    confirme: bool,
    a_partir_du: Option<&str>,
    le: Option<u32>,
) -> R<Issue> {
    engagement::etats::modifier(ctx, mois, i).map_err(Echec::from)?;
    engagement::stock::poser_la_saisie(ctx, mois, i, saisie).map_err(refus)?;
    if !engagement::etats::a_revalider_rouvert(ctx, mois, i).map_err(Echec::from)? {
        return Ok(Issue::Rien);
    }
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs).map_err(Echec::from)?;
    let devenu = engagement::stock::objets(&ctx.racine).map_err(Echec::from)?.into_iter().find(|x| x.fichier == o.fichier && x.rang == o.rang);
    if let Some(devenu) = devenu {
        let mut m = crate::routes::etapes::Moteur::nouveau(ctx);
        if let Some(texte) = crate::regularisation::refus_non_declarable(&mut m, ctx, o, &devenu, &departs)? {
            return Err(erreur(crate::regularisation::CLASSE_NON_DECLARABLE, texte));
        }
    }
    let en_valeur = |qui: String, c: Vec<String>| json!([{"qui": qui, "consequences": c}]);
    // « A PARTIR DE QUEL MOIS ? » : un contrat au mois dont un mois est parti et dont les termes changent. Sans reponse
    // parmi ses choix, la question vient dans l'avertissement ; un seul choix vaut reponse.
    if let Some(q) = engagement::etats::question_a_partir_du(ctx, mois, i, &departs).map_err(Echec::from)? {
        let depuis = a_partir_du.filter(|m| q.choix.iter().any(|c| c == m)).map(String::from).or_else(|| (q.choix.len() == 1).then(|| q.choix[0].clone()));
        match depuis {
            Some(d) => engagement::etats::poser_a_partir_du(ctx, mois, i, &d, le, &departs).map_err(refus)?,
            None => {
                let o = engagement::stock::objet_du_mois(&ctx.racine, mois, i).map_err(Echec::from)?;
                let avenant = o.as_ref().is_some_and(|o| contrat_parti(ctx, o, &departs));
                let choix: Vec<Value> =
                    q.choix.iter().map(|m| json!({"mois": m, "libelle": engagement::libelle_mois(m), "partis": q.partis.contains(m)})).collect();
                let question = json!({"phrase": q.phrase(), "si": ce_qui_arrive_aux_mois_partis(ctx, &q)?, "choix": choix});
                let lignes = if avenant { vec![UN_AVENANT.to_string()] } else { Vec::new() };
                return Ok(Issue::Avertir(json!({"engagements": en_valeur(q.qui.clone(), lignes), "a_partir_du": question})));
            }
        }
    }
    let v = engagement::etats::valider(ctx, mois, i, &departs).map_err(|m| {
        let data = engagement::stock::vue_du_mois(&ctx.racine, mois).ok().flatten().unwrap_or(Value::Null);
        crate::routes::mois::refus_du_calcul(ctx, &data, m)
    })?;
    // L'AVERTISSEMENT : l'engagement touche, nomme (`qui`), et une ligne par ce que le changement entraine pour lui.
    let lignes = ce_que_le_changement_entraine(ctx, &v, &departs)?;
    if !confirme && !lignes.is_empty() {
        return Ok(Issue::Avertir(json!({"engagements": en_valeur(engagement::etats::qui_de(ctx, v.objet()), lignes), "a_partir_du": Value::Null})));
    }
    Ok(Issue::Ecrite(v))
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
/// (`crate::regularisation::modification_de_l_engagement`) : un annule-et-remplace jusqu'a la limite, ensuite la correction
/// dans la DSN qui porte (`crate::regularisation::dsn_qui_porte`).
fn la_dsn_qui_change(m: &mut crate::routes::etapes::Moteur, ctx: &Contexte, mois: &str) -> R<String> {
    let de = crate::regularisation::de_mois_en_clair(mois);
    let d = crate::regularisation::modification_de_l_engagement(m, ctx, mois)?;
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
        let d = crate::regularisation::modification_de_l_engagement(&mut m, ctx, mois)?;
        let dsn = if d.is_null() { String::new() } else { format!(" ; {}", la_dsn_qui_change(&mut m, ctx, mois)?) };
        out.push(format!("dès {} : {}{}", engagement::libelle_mois(mois), un_bulletin_rectificatif(mois), dsn));
    }
    Ok(out)
}

/// CE QUE LA VALIDATION DIT SANS REFUSER, pour l'engagement valide `v` : ce qu'il fait signer (son contrat tant qu'il
/// n'est pas parti, ensuite l'avenant que le changement demande, `ce_que_le_changement_entraine`, ou rien) part a la
/// signature par courriel, a l'adresse de la fiche ; sans elle, il ne partira pas (`compagnie::signature`, qui le refuse
/// au depart). Il s'imprime, ou le courriel se saisit.
fn avis_de_validation(ctx: &Contexte, v: &engagement::etats::Valide, departs: &engagement::departs::Departs) -> R<Option<String>> {
    let e = &Value::Object(v.objet().brut.clone());
    let Some(cle) = engagement::cle_de_l_engagement(ctx, e) else { return Ok(None) };
    let nature = droit::natures::nature(e.get("nature").and_then(Value::as_str)).map_err(Echec::from)?;
    let numero = e.get("numero").filter(|n| vrai(n)).map(valeur_en_texte).unwrap_or_default();
    if nature.modele_contrat().is_none() || engagement::importe::vient_d_un_ancien_logiciel(&ctx.racine, &cle, &engagement::debut_brut(e), &numero) {
        return Ok(None);
    }
    let fiche = ctx.salarie(&cle).map_err(Echec::from)?;
    if fiche.get("email").map(valeur_en_texte).is_some_and(|m| !m.trim().is_empty()) {
        return Ok(None);
    }
    let document = if !contrat_parti(ctx, v.objet(), departs) {
        "contrat"
    } else if ce_que_le_changement_entraine(ctx, v, departs)?.iter().any(|l| l == UN_AVENANT) {
        "avenant"
    } else {
        return Ok(None);
    };
    let texte = |c: &str| fiche.get(c).filter(|x| vrai(x)).map(valeur_en_texte);
    let qui = [texte("nom"), texte("prenom_affichage").or_else(|| texte("prenom"))].into_iter().flatten().collect::<Vec<_>>().join(" ");
    Ok(Some(format!(
        "{} n'a pas de courriel dans sa fiche : son {} ne pourra pas partir à la signature par courriel. Imprimez-le \
         pour le faire signer, ou saisissez son courriel (étape « Contrats » de la paie, ou sa fiche dans Configuration › \
         Salariés).",
        if qui.is_empty() { cle } else { qui },
        document
    )))
}

// ------------------------------------------------------------------ retirer-engagement

/// « Retirer cet engagement » : la carte se designe par son identite (`engagement`, que le moteur lui a rendue).
/// Tant que rien n'est parti de lui, l'engagement se supprime, meme valide, et rend son numero
/// (`engagement::etats::supprimer`). Des qu'un envoi est parti, la route avertit sans rien ecrire (`avertissement` : ce
/// qui est parti) ; renvoyee avec `confirme`, l'engagement devient non actif (`engagement::etats::desactiver`) et la
/// phrase dit ce que cela rend a faire (la DSN d'un mois a corriger). Rien ne part de lui-meme.
fn retirer_engagement(ctx: &Contexte, charge: &Value) -> R<Value> {
    let mois = texte_requis(charge, "mois")?;
    let _verrou = crate::routes::mois::avant_d_ecrire(ctx, mois)?;
    let (i, o) = designe(ctx, mois, charge)?;
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs).map_err(Echec::from)?;
    if !booleen(charge, "confirme", false) {
        let parti = !engagement::etats::ce_qui_est_parti(ctx, &o, &departs).is_empty();
        return match engagement::etats::supprimer(ctx, &o, &departs) {
            Ok(()) => Ok(json!({"phrase": "Engagement retiré."})),
            Err(m) if parti => Ok(json!({"avertissement": m})),
            Err(m) => Err(Echec::from(m)),
        };
    }
    let n = engagement::etats::desactiver(ctx, mois, i, &departs).map_err(Echec::from)?;
    let a_faire = a_faire_en_phrases(&n.a_refaire(ctx, &departs));
    let phrase = if a_faire.is_empty() { "Engagement non actif.".to_string() } else { format!("Engagement non actif. À faire : {}.", a_faire.join(", ")) };
    Ok(json!({"phrase": phrase}))
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
