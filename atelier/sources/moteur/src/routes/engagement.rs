//! Les routes de l'engagement, celles de la carte : la valider, la modifier, la retirer. Chacune rend la phrase que
//! l'on lit ; un refus dit quoi corriger et n'ecrit rien.

use std::collections::HashMap;

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use socle::echec::{erreur, Echec, Refus, R};
use socle::fichiers::vrai;
use socle::valeur::valeur_en_texte;

use crate::routes::catalogue::{Acte, Champ, Fiche, Refus as RefusDit, FORME, MOIS};
use crate::routes::catalogue::{Ecran, Place};

const IDENTITE: Champ = Champ {
    nom: "engagement",
    sens: "l'identité de la carte, telle que le moteur l'a rendue : {salarie, debut, rang}",
    avance: false,
};
/// La saisie d'une carte, ses cases et ou se lit ce que chacune demande.
pub(crate) const SAISIE: Champ = Champ {
    nom: "saisie",
    sens: "ce que la carte porte : salarie (la clé de sa fiche), spectacle, nature (`natures` de la route etat ; muette, c'est un CDDU : un CDI la dit), debut, fin, motif_recours (un CDD), lignes [{rubrique, quantite, taux, dates, et les cases que sa rubrique demande, `rubriques[].saisie` de la route etat : heures (par unité : 4 pour des services de 4 h), jours, montant…}] ; un contrat au mois : les cases de la carte (`cases` de cette fiche, les avancées seulement quand `faits_avances` de la route apercu les nomme) ; sa fin : `cases_de_la_fin` de cette fiche ; les cases que son contrat écrit appelle, et numero (le numéro déjà déclaré en DSN d'un contrat commencé avant PuppetPaie) : `contrat.ecrits` de la route apercu (leur sens : `aides` de la route etat)",
    avance: false,
};
pub(crate) const SAISIE_INCOMPLETE: RefusDit = RefusDit { classe: "SaisieIncomplete", corriger: "le fait que la phrase nomme, sur la carte ou la fiche du salarié" };
const OPERATION: RefusDit = RefusDit {
    classe: "ErreurOperation",
    corriger: "ce que la phrase dit : un mois importé ou clos, une carte que l'écran ne montre pas, un contrat au mois déjà engagé (sa carte le modifie), une saisie hors de ses bornes",
};

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Paie);

/// Les routes de ce module, et leurs fiches. Toutes ecrivent.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        nom: "valider-engagement",
        geste: "Valider l'engagement d'une carte : l'écrire et calculer ses paies.",
        charge: &[MOIS, Champ { nom: "engagement", sens: "l'identité de la carte ; absente pour un engagement neuf", avance: false }, SAISIE],
        exemple: r#"{"mois":"2026-11","saisie":{"salarie":"FULLER","spectacle":"le-banc-des-menteurs","lignes":[{"rubrique":"cachet","quantite":1,"taux":150,"dates":["2026-11-27"]}]}}"#,
        rend: "engagement (l'identité de la carte), phrase",
        refus: &[SAISIE_INCOMPLETE, OPERATION, FORME],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 320,
        ecran: ECRAN,
    },
    Fiche {
        nom: "modifier-engagement",
        geste: "Modifier un engagement validé : écrire ce que sa carte ouverte montre.",
        charge: &[
            MOIS,
            IDENTITE,
            SAISIE,
            Champ { nom: "confirme", sens: "vrai : l'avertissement rendu a été lu et accepté", avance: false },
            Champ { nom: "jour", sens: "le jour du mois (un nombre, de 1 à 31) où un terme changé prend effet, quand ce n'est pas le 1er", avance: true },
        ],
        exemple: r#"{"mois":"2026-11","engagement":{"salarie":"FULLER","debut":"2026-11-27","rang":0},"saisie":{"salarie":"FULLER","spectacle":"le-banc-des-menteurs","lignes":[{"rubrique":"cachet","quantite":1,"taux":160,"dates":["2026-11-27"]}]}}"#,
        rend: "rien n'a changé : change faux ; ce qui est parti le demande : avertissement (à montrer, puis renvoyer avec confirme), rien n'est écrit ; écrit : change, phrase (un terme vaut à partir du mois de la carte)",
        refus: &[
            SAISIE_INCOMPLETE,
            OPERATION,
            RefusDit { classe: "ModificationNonDeclarable", corriger: "rien : la norme ne permet plus de déclarer ce changement ; la phrase dit pourquoi" },
            FORME,
        ],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 320,
        ecran: ECRAN,
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
        ecran: ECRAN,
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
    engagement::etats::valider(ctx, mois, i, &departs).map_err(|m| crate::routes::mois::refus_du_calcul(ctx, &data, m))?;
    brouillon.ecrire().map_err(Echec::from)?;
    let e = &data["engagements"][i];
    if let Some(cle) = engagement::cle_de_l_engagement(ctx, e) {
        crate::routes::fiches::poser_anciennete_a_l_embauche(ctx, &cle, &engagement::debut_brut(e))?;
    }
    Ok(json!({"engagement": id, "phrase": "Engagement validé."}))
}

// ------------------------------------------------------------------ modifier-engagement

/// « MODIFIER L'ENGAGEMENT » : les valeurs que la carte ouverte d'un engagement valide montre, ecrites d'un geste.
/// La carte se designe par son identite (`engagement`, que le moteur lui a rendue) ; `saisie` est ce que
/// sa carte ecrit pour `mois`. Une carte porte un salarie : elle modifie son engagement, et lui seul. LE MOIS DE LA CARTE
/// EST LE « A PARTIR DE » : un terme du contrat qu'elle change (le salaire, la duree du travail, l'emploi...) vaut de ce
/// mois jusqu'au changement suivant (`engagement::stock::poser_la_saisie`), du 1er, ou du `jour` dit.
///
/// - Rien n'a change : rien ne s'ecrit, la reponse le dit (`change` faux).
/// - Quelque chose a change, et rien de ce qui est parti ne le demande : la modification s'ecrit, l'engagement reste valide.
/// - Ce qui est parti le demande : la reponse porte l'avertissement, et rien n'est ecrit ; la page le montre et renvoie
///   la demande avec `confirme` (et `jour`, quand le changement ne vaut pas du 1er). `avertissement` : `engagements`,
///   l'engagement touche nomme (`qui`) avec une ligne par ce que le changement entraine pour un envoi parti sur la
///   version d'avant (`consequences`, `engagement::etats::a_refaire`) : un avenant a faire signer, une DPAE a deposer
///   pour une embauche avancee, un bulletin rectificatif, la DSN qui se corrige ; et, pour un contrat au mois dont un
///   terme change dans un mois d'apres son premier, `jour` : la phrase qui dit d'ou il vaut (`a_partir_de`).
///
/// Refus, et rien n'est ecrit : un engagement qui n'est pas valide (il se saisit sur sa carte), importe, d'un mois tenu
/// avant PuppetPaie, d'un exercice clos ; un jour qui n'est pas un jour du contrat dans ce mois ; un changement que la
/// norme ne permet plus de declarer (`crate::regularisation::refus_non_declarable`) ; une saisie qui ne se calcule pas.
fn modifier_engagement(ctx: &Contexte, charge: &Value) -> R<Value> {
    let mois = texte_requis(charge, "mois")?;
    let Some(Value::Object(saisie)) = charge.get("saisie") else {
        return Err(erreur("TypeError", "`saisie` : un objet est attendu"));
    };
    let confirme = booleen(charge, "confirme", false);
    // Le jour du mois de la carte ou un terme change, quand on le dit (le 1er sinon).
    let jour = match charge.get("jour").filter(|v| vrai(v)) {
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
    let depuis = match modifier_puis_revalider(ctx, mois, i, &o, saisie, confirme, jour)? {
        Issue::Ecrite(depuis) => depuis,
        Issue::Rien => return Ok(json!({"change": false, "phrase": "Rien n'a changé : rien n'est écrit."})),
        Issue::Avertir(a) => {
            let phrases = avertissement_en_phrases(&a);
            return Ok(json!({"avertissement": a, "phrase": "Ce qui est parti le demande : rien n'est écrit sans confirme.", "phrases": phrases}));
        }
    };
    brouillon.ecrire().map_err(Echec::from)?;
    let data = engagement::stock::vue_du_mois(&ctx.racine, mois).map_err(Echec::from)?.unwrap_or(Value::Null);
    let e = &data["engagements"][i];
    if let Some(cle) = engagement::cle_de_l_engagement(ctx, e) {
        crate::routes::fiches::poser_anciennete_a_l_embauche(ctx, &cle, &engagement::debut_brut(e))?;
    }
    crate::interface::suppression::sortir_de_la_demo(&ctx.racine, mois).map_err(|m| erreur("OSError", m))?;
    let phrase = match depuis {
        Some(j) => format!("Engagement modifié. {}", vaut_a_partir_du(&j)),
        None => "Engagement modifié.".to_string(),
    };
    Ok(json!({"change": true, "phrase": phrase}))
}

/// Ce que la modification a donne.
enum Issue {
    /// Rien n'a change.
    Rien,
    /// Ce qui est parti le demande : l'avertissement, a confirmer.
    Avertir(Value),
    /// Ecrite : l'engagement revalide ; le jour d'ou vaut un terme qu'elle change dans un mois d'apres le premier.
    Ecrite(Option<socle::date::Jour>),
}

/// « Ce qui change vaut à partir du 01/09/2026… » : un terme change du jour `j` au changement suivant ; un montant d'un
/// seul mois est un fait du mois.
fn vaut_a_partir_du(j: &socle::date::Jour) -> String {
    format!(
        "Ce qui change vaut à partir du {}, jusqu'au prochain changement ; un montant d'un seul mois se dit en prime.",
        j.jj_mm_aaaa()
    )
}

/// LE JOUR D'OU VAUT UN TERME que la carte de `mois` change dans un contrat paye au mois, `o` avant et `devenu` apres : le
/// dernier changement date de ce mois, s'il n'etait pas deja ecrit. `None` : la carte ne change aucun terme a ce mois
/// (le premier mois du contrat dit ses termes de son debut).
fn a_partir_de(o: &engagement::stock::Objet, devenu: &engagement::stock::Objet, mois: &str) -> Option<socle::date::Jour> {
    if !devenu.mensuel() {
        return None;
    }
    let du_mois = |x: &engagement::stock::Objet| -> Vec<(String, socle::date::Jour, Value)> {
        x.changements().into_iter().filter(|(_, j, _)| j.mois_iso() == mois).collect()
    };
    let apres = du_mois(devenu);
    (apres != du_mois(o)).then(|| apres.iter().map(|(_, j, _)| *j).max()).flatten()
}

/// La modification elle-meme, dans le brouillon ouvert : l'engagement `o`, de rang `i`, se rouvre, prend sa saisie et
/// se revalide. Ce qu'elle rend dit si elle s'ecrit (`Issue::Ecrite`).
fn modifier_puis_revalider(ctx: &Contexte, mois: &str, i: usize, o: &engagement::stock::Objet, saisie: &Map<String, Value>, confirme: bool, jour: Option<u32>) -> R<Issue> {
    engagement::etats::modifier(ctx, mois, i).map_err(Echec::from)?;
    engagement::stock::poser_la_saisie(ctx, mois, i, saisie, jour).map_err(refus)?;
    if !engagement::etats::a_revalider_rouvert(ctx, mois, i).map_err(Echec::from)? {
        return Ok(Issue::Rien);
    }
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs).map_err(Echec::from)?;
    let devenu = engagement::stock::objets(&ctx.racine).map_err(Echec::from)?.into_iter().find(|x| x.fichier == o.fichier && x.rang == o.rang);
    let depuis = devenu.as_ref().and_then(|d| a_partir_de(o, d, mois));
    if let Some(devenu) = devenu {
        let mut m = crate::routes::etapes::Moteur::nouveau(ctx);
        if let Some(texte) = crate::regularisation::refus_non_declarable(&mut m, ctx, o, &devenu, &departs)? {
            return Err(erreur(crate::regularisation::CLASSE_NON_DECLARABLE, texte));
        }
    }
    let v = engagement::etats::valider(ctx, mois, i, &departs).map_err(|m| {
        let data = engagement::stock::vue_du_mois(&ctx.racine, mois).ok().flatten().unwrap_or(Value::Null);
        crate::routes::mois::refus_du_calcul(ctx, &data, m)
    })?;
    // L'AVERTISSEMENT : l'engagement touche, nomme (`qui`), une ligne par ce que le changement entraine pour lui, et le
    // jour d'ou vaut un terme change, qui se dit quand ce n'est pas le 1er.
    let lignes = ce_que_le_changement_entraine(ctx, &v, &departs)?;
    if !confirme && !lignes.is_empty() {
        let jour = depuis.map_or(Value::Null, |j| json!(vaut_a_partir_du(&j)));
        let en_valeur = json!([{"qui": engagement::etats::qui_de(ctx, v.objet()), "consequences": lignes}]);
        return Ok(Issue::Avertir(json!({"engagements": en_valeur, "jour": jour})));
    }
    Ok(Issue::Ecrite(depuis))
}

/// L'avertissement d'une modification en phrases : ce que le changement entraine pour chaque engagement touche, puis
/// d'ou vaut un terme change (`jour`).
fn avertissement_en_phrases(a: &Value) -> Vec<Value> {
    let mut lignes = Vec::new();
    for e in a["engagements"].as_array().into_iter().flatten() {
        for c in e["consequences"].as_array().into_iter().flatten() {
            lignes.push(json!(format!("{} : {}", valeur_en_texte(&e["qui"]), valeur_en_texte(c))));
        }
    }
    if let Some(j) = a["jour"].as_str() {
        lignes.push(json!(format!("{} (jour : un autre jour du mois)", j)));
    }
    lignes
}

/// Ce qu'un contrat parti que le changement modifie entraine.
const UN_AVENANT: &str = "un avenant à faire signer";

/// CE QUE LE CHANGEMENT ENTRAINE, une ligne par consequence, pour l'engagement revalide `v` : chaque envoi parti sur une
/// version qui n'est plus la sienne (`engagement::etats::a_refaire`). Une embauche reportee ne refait pas la DPAE
/// (`crate::regularisation::dpae_d_une_date_changee`).
fn ce_que_le_changement_entraine(ctx: &Contexte, v: &engagement::etats::Valide, departs: &engagement::departs::Departs) -> R<Vec<String>> {
    use engagement::etats::ARefaire;
    let saisie = |x: Option<&Value>| x.and_then(|x| x.get("saisie")).cloned().unwrap_or(Value::Null);
    let presente = saisie(v.objet().brut.get("valide"));
    let mut m = crate::routes::etapes::Moteur::nouveau(ctx);
    let mut out = Vec::new();
    for a in engagement::etats::a_refaire(ctx, v, departs) {
        match a {
            ARefaire::Avenant { .. } => out.push(UN_AVENANT.to_string()),
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
            Err(m) if parti => Ok(json!({"phrase": m.clone(), "avertissement": m})),
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
