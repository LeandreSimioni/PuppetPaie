//! Les routes de l'engagement, celles de la carte : la valider, la modifier, la retirer. Chacune rend la phrase que
//! l'on lit ; un refus dit quoi corriger et n'ecrit rien.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use droit::contexte::Contexte;
use engagement::stock::Identite;
use socle::echec::{erreur, Echec, Refus, R};
use socle::fichiers::vrai;
use socle::mois::Mois;

use crate::routes::catalogue::{Acte, Affiche, Champ, Forme, Fiche, Refus as RefusDit, FORME, MOIS};
use crate::routes::catalogue::Route;
use crate::routes::catalogue::{Ecran, Place};

const IDENTITE: Champ = Champ {
    nom: "engagement", libelle: Affiche::new("Carte"), forme: Forme::Objet,
    sens: "l'identité de la carte, telle que le moteur l'a rendue : {salarie, debut, rang}",
    avance: false,
};
/// La saisie d'une carte, ses cases et ou se lit ce que chacune demande.
pub(crate) const SAISIE: Champ = Champ {
    nom: "saisie", libelle: Affiche::new("Saisie"), forme: Forme::Objet,
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
        route: Route::ValiderEngagement,
        geste: "Valider l'engagement d'une carte : l'écrire et calculer ses paies.",
        charge: &[MOIS, Champ { nom: "engagement", libelle: Affiche::new("Carte"), forme: Forme::Objet, sens: "l'identité de la carte ; absente pour un engagement neuf", avance: false }, SAISIE],
        exemple: r#"{"mois":"2026-11","saisie":{"salarie":"FULLER","spectacle":"le-banc-des-menteurs","lignes":[{"rubrique":"cachet","quantite":1,"taux":150,"dates":["2026-11-27"]}]}}"#,
        rend: "engagement (l'identité de la carte), phrase",
        refus: &[SAISIE_INCOMPLETE, OPERATION, FORME],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 320,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::ModifierEngagement,
        geste: "Modifier un engagement validé : écrire ce que sa carte ouverte montre.",
        charge: &[
            MOIS,
            IDENTITE,
            SAISIE,
            Champ { nom: "confirme", libelle: Affiche::new("Avertissement lu"), forme: Forme::OuiNon, sens: "vrai : l'avertissement rendu a été lu et accepté", avance: false },
            Champ { nom: "jour", libelle: Affiche::new("Jour d'effet"), forme: Forme::Nombre, sens: "le jour du mois (un nombre, de 1 à 31) où un terme changé prend effet, quand ce n'est pas le 1er", avance: true },
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
        route: Route::RetirerEngagement,
        geste: "Retirer l'engagement d'une carte, ou le rendre non actif si un envoi est parti.",
        charge: &[MOIS, IDENTITE, Champ { nom: "confirme", libelle: Affiche::new("Avertissement lu"), forme: Forme::OuiNon, sens: "vrai : l'avertissement rendu a été lu, et l'engagement devient non actif", avance: false }],
        exemple: r#"{"mois":"2026-11","engagement":{"salarie":"FULLER","debut":"2026-11-27","rang":0},"confirme":true}"#,
        rend: "phrase : retiré, ou non actif et ce que cela rend à faire ; un envoi est parti : avertissement (à montrer, puis renvoyer avec confirme), rien n'est écrit",
        refus: &[OPERATION, FORME],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 200,
        ecran: ECRAN,
    },
];

/// Une route de ce module : sa charge lue en sa forme, sa reponse une structure.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: Route, charge: &Value) -> R<Value> {
    use crate::routes::{en_json, lire_charge};
    match nom {
        Route::ValiderEngagement => en_json(valider_engagement(ctx, lire_charge(charge)?)?),
        Route::ModifierEngagement => en_json(modifier_engagement(ctx, lire_charge(charge)?)?),
        Route::RetirerEngagement => en_json(retirer_engagement(ctx, lire_charge(charge)?)?),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

/// La charge de « Modifier l'engagement » : le mois de la carte, son identite, ce qu'elle montre ; l'avertissement lu
/// (`confirme`) et le jour du mois d'ou vaut un terme change (`jour`, le 1er sinon).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeModifier {
    mois: Mois,
    engagement: Identite,
    /// TRAVAIL A FAIRE (chaine A) : la saisie d'un engagement, telle que son fichier l'ecrit.
    saisie: Map<String, Value>,
    #[serde(default)]
    confirme: bool,
    #[serde(default)]
    jour: Option<u32>,
}

/// La charge de « Retirer cet engagement » : le mois de la carte, son identite, l'avertissement lu (`confirme`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeRetirer {
    mois: Mois,
    engagement: Identite,
    #[serde(default)]
    confirme: bool,
}

/// Ce que rend « Valider l'engagement » : l'identite de la carte ecrite, et la phrase.
#[derive(Debug, Serialize)]
pub(crate) struct Validation {
    pub(crate) engagement: Identite,
    pub(crate) phrase: &'static str,
}

/// Ce que rend « Modifier l'engagement » : fait (`change` dit si quelque chose s'est ecrit), ou l'avertissement a
/// confirmer, rien d'ecrit.
#[derive(Debug, Serialize)]
#[serde(untagged)]
enum Modification {
    Faite { change: bool, phrase: String },
    Avertie { avertissement: Avertissement, phrase: &'static str, phrases: Vec<String> },
}

/// L'avertissement d'une modification : chaque engagement touche, ce que le changement entraine pour lui, et d'ou vaut
/// un terme change (`jour`, `null` quand il vaut du debut).
#[derive(Debug, Serialize)]
struct Avertissement {
    engagements: Vec<Touche>,
    jour: Option<String>,
}

/// Un engagement que la modification touche : nomme (`qui`), une ligne par consequence.
#[derive(Debug, Serialize)]
struct Touche {
    qui: String,
    consequences: Vec<String>,
}

/// Ce que rend « Retirer cet engagement » : la phrase ; un envoi parti, l'avertissement a confirmer.
#[derive(Debug, Serialize)]
struct Retrait {
    phrase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    avertissement: Option<String>,
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
    crate::routes::fiches::exiger(charge, cle)?.as_str().ok_or_else(|| erreur("TypeError", socle::echec::charge(cle, "un texte")))
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

/// L'ENGAGEMENT QU'UNE CARTE MONTRE : celui que designe son identite, que le moteur lui a rendue
/// (`engagement::stock::identites`). Refus : une identite que l'ecran de `mois` ne montre pas.
fn designe(ctx: &Contexte, mois: &str, id: &Identite) -> R<(usize, engagement::stock::Objet)> {
    engagement::stock::designe(&ctx.racine, mois, id).map_err(refus)
}


// ------------------------------------------------------------------ valider-engagement


/// « VALIDER L'ENGAGEMENT », UN GESTE : la carte s'ecrit (`saisie`, et `engagement`, son identite, absente
/// pour un engagement neuf : `crate::routes::mois::ecrire_une_carte`), puis l'engagement ecrit se valide : il ecrit ses
/// paies, une par mois qu'il couvre (`engagement::etats::valider`). Le tout se fait dans un brouillon
/// (`engagement::stock::Brouillon`) : il lui manque un fait, la validation refuse en disant lequel, et rien n'est ecrit.
/// Un engagement valide ne se valide pas (il se modifie par `modifier-engagement`).
/// Sa charge est celle d'une carte (`mois::ChargeCarte`) : valider ne se defait pas, un engagement valide se modifie
/// (`modifier-engagement`) ou se retire. L'horloge de la compagnie fictive valide par ce geste aussi.
pub(crate) fn valider_engagement(ctx: &Contexte, carte: crate::routes::mois::ChargeCarte) -> R<Validation> {
    let mois = &carte.mois.to_string();
    let _verrou = crate::routes::mois::avant_d_ecrire(ctx, mois)?;
    let brouillon = engagement::stock::Brouillon::ouvrir(&ctx.racine);
    let id = crate::routes::mois::ecrire_une_carte(ctx, mois, carte.engagement.as_ref(), &carte.saisie)?;
    let (i, _) = engagement::stock::designe(&ctx.racine, mois, &id).map_err(Echec::from)?;
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs).map_err(Echec::from)?;
    let data = engagement::stock::vue_du_mois(&ctx.racine, mois).map_err(Echec::from)?.unwrap_or_else(|| engagement::saisie::VueDuMois::vide(carte.mois));
    if let Some(manque) = crate::routes::mois::manque_de(ctx, &data, i) {
        return Err(crate::routes::mois::refus_du_calcul(ctx, &data, manque));
    }
    engagement::etats::valider(ctx, mois, i, &departs).map_err(|m| crate::routes::mois::refus_du_calcul(ctx, &data, m))?;
    brouillon.ecrire().map_err(Echec::from)?;
    let e = data.engagements.get(i).cloned().unwrap_or_default();
    if let Some(cle) = e.cle(ctx) {
        crate::routes::fiches::poser_anciennete_a_l_embauche(ctx, &cle, &e.debut_lu(&ctx.racine))?;
    }
    Ok(Validation { engagement: id, phrase: "Engagement validé." })
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
fn modifier_engagement(ctx: &Contexte, charge: ChargeModifier) -> R<Modification> {
    let ChargeModifier { mois, engagement: id, saisie, confirme, jour } = charge;
    let mois = &mois.to_string();
    // Le jour du mois de la carte ou un terme change, quand on le dit (le 1er sinon).
    if let Some(j) = jour.filter(|j| !(1..=31).contains(j)) {
        return Err(refus(format!("« le {} » : un jour du mois est attendu, de 1 à 31. Rien n'est écrit.", j)));
    }
    let _verrou = crate::routes::mois::avant_d_ecrire(ctx, mois)?;
    let (i, o) = designe(ctx, mois, &id)?;
    if !o.est_valide() {
        return Err(refus(format!(
            "{} : cet engagement n'est pas validé ; il se saisit sur sa carte, puis « Valider l'engagement ». Rien n'est écrit.",
            engagement::etats::qui_de(ctx, &o)
        )));
    }
    // LA MODIFICATION SE FAIT DANS UN BROUILLON (`engagement::stock::Brouillon`) : calculee en memoire, elle ne s'ecrit
    // sur le disque que quand elle est faite ; rien n'a change, un avertissement a confirmer, un refus : rien n'est ecrit.
    let brouillon = engagement::stock::Brouillon::ouvrir(&ctx.racine);
    let depuis = match modifier_puis_revalider(ctx, mois, i, &o, &saisie, confirme, jour)? {
        Issue::Ecrite(depuis) => depuis,
        Issue::Rien => return Ok(Modification::Faite { change: false, phrase: "Rien n'a changé : rien n'est écrit.".into() }),
        Issue::Avertir(a) => {
            let phrases = avertissement_en_phrases(&a);
            return Ok(Modification::Avertie { avertissement: a, phrase: "Ce qui est parti le demande : rien n'est écrit sans confirme.", phrases });
        }
    };
    brouillon.ecrire().map_err(Echec::from)?;
    let data = engagement::stock::vue_du_mois(&ctx.racine, mois).map_err(Echec::from)?.map(|d| d.engagements).unwrap_or_default();
    let e = data.get(i).cloned().unwrap_or_default();
    if let Some(cle) = e.cle(ctx) {
        crate::routes::fiches::poser_anciennete_a_l_embauche(ctx, &cle, &e.debut_lu(&ctx.racine))?;
    }
    crate::interface::suppression::sortir_de_la_demo(&ctx.racine, mois).map_err(|m| erreur("OSError", m))?;
    let phrase = match depuis {
        Some(j) => format!("Engagement modifié. {}", vaut_a_partir_du(&j)),
        None => "Engagement modifié.".to_string(),
    };
    Ok(Modification::Faite { change: true, phrase })
}

/// Ce que la modification a donne.
enum Issue {
    /// Rien n'a change.
    Rien,
    /// Ce qui est parti le demande : l'avertissement, a confirmer.
    Avertir(Avertissement),
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
        x.changements().into_iter().filter(|(_, j, _)| j.mois_civil().to_string() == mois).collect()
    };
    let apres = du_mois(devenu);
    (apres != du_mois(o)).then(|| apres.iter().map(|(_, j, _)| *j).max()).flatten()
}

/// La modification elle-meme, dans le brouillon ouvert : l'engagement `o`, de rang `i`, se rouvre, prend sa saisie et
/// se revalide. Ce qu'elle rend dit si elle s'ecrit (`Issue::Ecrite`).
fn modifier_puis_revalider(ctx: &Contexte, mois: &str, i: usize, o: &engagement::stock::Objet, saisie: &Map<String, Value>, confirme: bool, jour: Option<u32>) -> R<Issue> {
    engagement::etats::modifier(ctx, mois, i).map_err(Echec::from)?;
    engagement::stock::poser_la_saisie(ctx, mois, i, &engagement::saisie::Carte::lire(&Value::Object(saisie.clone())).map_err(refus)?, jour).map_err(refus)?;
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
        match engagement::stock::vue_du_mois(&ctx.racine, mois).ok().flatten() {
            Some(data) => crate::routes::mois::refus_du_calcul(ctx, &data, m),
            None => Echec::from(m),
        }
    })?;
    // L'AVERTISSEMENT : l'engagement touche, nomme (`qui`), une ligne par ce que le changement entraine pour lui, et le
    // jour d'ou vaut un terme change, qui se dit quand ce n'est pas le 1er.
    let lignes = ce_que_le_changement_entraine(ctx, &v, &departs)?;
    if !confirme && !lignes.is_empty() {
        let touche = Touche { qui: engagement::etats::qui_de(ctx, v.objet()), consequences: lignes };
        return Ok(Issue::Avertir(Avertissement { engagements: vec![touche], jour: depuis.map(|j| vaut_a_partir_du(&j)) }));
    }
    Ok(Issue::Ecrite(depuis))
}

/// L'avertissement d'une modification en phrases : ce que le changement entraine pour chaque engagement touche, puis
/// d'ou vaut un terme change (`jour`).
fn avertissement_en_phrases(a: &Avertissement) -> Vec<String> {
    let mut lignes: Vec<String> = a.engagements.iter().flat_map(|e| e.consequences.iter().map(move |c| format!("{} : {}", e.qui, c))).collect();
    if let Some(j) = &a.jour {
        lignes.push(format!("{} (jour : un autre jour du mois)", j));
    }
    lignes
}

/// Ce qu'un contrat parti que le changement modifie entraine.
const UN_AVENANT: &str = "un avenant à faire signer";

/// CE QUE LE CHANGEMENT ENTRAINE, une ligne par consequence, pour l'engagement revalide `v` : chaque envoi parti sur une
/// version qui n'est plus la sienne (`engagement::etats::a_refaire`). Une embauche reportee ne refait pas la DPAE
/// (`crate::regularisation::dpae_d_une_date_changee`).
pub(crate) fn ce_que_le_changement_entraine(ctx: &Contexte, v: &engagement::etats::Valide, departs: &engagement::departs::Departs) -> R<Vec<String>> {
    use engagement::etats::ARefaire;
    let saisie = |x: Option<&Value>| x.and_then(|x| x.get("saisie")).cloned().unwrap_or(Value::Null);
    let presente = saisie(v.objet().brut.get("valide"));
    let mut m = crate::routes::etapes::Moteur::nouveau(ctx);
    let mut out = Vec::new();
    for a in engagement::etats::a_refaire(ctx, v, departs) {
        match a {
            ARefaire::Avenant { .. } => out.push(UN_AVENANT.to_string()),
            ARefaire::Dpae { debut_declare, .. } => {
                let (rien_a_refaire, phrase) = crate::regularisation::dpae_d_une_date_changee(&debut_declare, &engagement::debut_brut(&ctx.racine, &presente));
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
    use crate::regularisation::{de_mois_en_clair, DsnDeposee};
    let de = de_mois_en_clair(mois);
    Ok(match crate::regularisation::modification_de_l_engagement(m, ctx, Mois::lire(mois)?)? {
        Some(DsnDeposee::Corrigee { porteur }) => {
            format!("la correction dans la DSN {} (celle {}, déposée, ne se remplace plus)", de_mois_en_clair(&porteur), de)
        }
        Some(DsnDeposee::Remplacee { limite }) => {
            format!("un annule-et-remplace de la DSN {}, admis jusqu'au {} à minuit", de, limite.jj_mm_aaaa())
        }
        None => format!("la DSN {} à corriger", de),
    })
}

// ------------------------------------------------------------------ retirer-engagement

/// « Retirer cet engagement » : la carte se designe par son identite (`engagement`, que le moteur lui a rendue).
/// Tant que rien n'est parti de lui, l'engagement se supprime, meme valide, et rend son numero
/// (`engagement::etats::supprimer`). Des qu'un envoi est parti, la route avertit sans rien ecrire (`avertissement` : ce
/// qui est parti) ; renvoyee avec `confirme`, l'engagement devient non actif (`engagement::etats::desactiver`) et la
/// phrase dit ce que cela rend a faire (la DSN d'un mois a corriger). Rien ne part de lui-meme.
fn retirer_engagement(ctx: &Contexte, charge: ChargeRetirer) -> R<Retrait> {
    let mois = &charge.mois.to_string();
    let _verrou = crate::routes::mois::avant_d_ecrire(ctx, mois)?;
    let (i, o) = designe(ctx, mois, &charge.engagement)?;
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs).map_err(Echec::from)?;
    if !charge.confirme {
        let parti = !engagement::etats::ce_qui_est_parti(ctx, &o, &departs).is_empty();
        return match engagement::etats::supprimer(ctx, &o, &departs) {
            Ok(()) => Ok(Retrait { phrase: "Engagement retiré.".into(), avertissement: None }),
            Err(m) if parti => Ok(Retrait { phrase: m.clone(), avertissement: Some(m) }),
            Err(m) => Err(Echec::from(m)),
        };
    }
    let n = engagement::etats::desactiver(ctx, mois, i, &departs).map_err(Echec::from)?;
    let a_faire = a_faire_en_phrases(&n.a_refaire(ctx, &departs));
    let phrase = if a_faire.is_empty() { "Engagement non actif.".to_string() } else { format!("Engagement non actif. À faire : {}.", a_faire.join(", ")) };
    Ok(Retrait { phrase, avertissement: None })
}

/// Ce qu'un engagement non actif rend a faire, en phrases pour l'ecran.
pub(crate) fn a_faire_en_phrases(a: &[engagement::etats::ARefaire]) -> Vec<String> {
    use engagement::etats::ARefaire;
    a.iter()
        .filter_map(|x| match x {
            ARefaire::Dsn { mois, .. } => Some(format!("DSN de {} à corriger", socle::mois::Mois::en_clair(mois))),
            _ => None,
        })
        .collect()
}
