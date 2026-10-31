//! Les contrats qui partent : `demander-signature` (par courriel, avec le bouton « Je signe »), `contrats-imprimer`
//! (pour une remise en main propre), `avenant-envoyer` et `avenants-envoyer`.
//!
//! Un courriel par salarie, un PDF par courriel (le contrat, ou les avenants), le bouton « Je signe » (un lien `mailto:`
//! vers la boite de la compagnie, objet et corps deja ecrits) et un CODE UNIQUE (`SIG-XXXXXX`) par courriel, garde au
//! journal avec la demande : c'est par lui que la releve des signatures reconnait la reponse. Les messages sont ceux que
//! `signature-apercu` et `avenant-apercu` ont montres : les paies (`ecrire_documents::paies_aux_numeros_declares`), les
//! articles modifies (`avenant-apercu` lui-meme, appele ici), les etats (`signature-apercu`, pour la relance).
//!
//! La route lit la charge, ouvre la boite, exige la preuve (le mot de passe du logiciel) et fabrique les apercus
//! (`signature-apercu`, `avenant-apercu`) avant d'appeler ce module ; ici, ce qui reste se verifie avant que rien ne
//! parte, puis les PDF s'ecrivent : rien n'est ecrit sur un refus.
//!
//! Chaque contrat part sous le numero que son engagement porte depuis sa validation (`engagement::numeros`) ; le
//! journal note qu'il est parti.
//!
//! Le moteur fait les pieces (`documents::contrat`) et compose les messages ; `envois` les transporte
//! (`envoi::envoyer_suivi`) et note le depart (`ligne_d_envoi`).

use crate::routes::dossier_salarie::cle_de;
use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::{json, Map, Value};

use engagement::calcul::Paie;
use droit::contexte::Contexte;
use envois::courriel::Courriel;
use envois::envoi::{self, Message};
use socle::valeur::valeur_en_texte;
use socle::echec::{erreur, R};
use socle::fichiers::{repr, vrai};
use socle::echec::Echec;
use socle::fichiers::nom_du;
use socle::html::{html_echapper, mailto};
use socle::texte::capitaliser;
use envois::ligne_d_envoi::{echec_d_envoi, noter_echec_d_envoi, noter_envoi};

/// Un refus ordinaire, avec sa phrase.
fn refus(message: impl Into<String>) -> Echec {
    erreur("ErreurOperation", message)
}

/// `d[cle]` : `KeyError` si elle manque.
fn requis<'a>(d: &'a Value, cle: &str) -> R<&'a Value> {
    match d {
        Value::Object(o) => o.get(cle).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(cle))),
        autre => Err(erreur("TypeError", socle::fichiers::attendu("un objet", socle::valeur::nom_type(autre)))),
    }
}

/// `d.get(cle)` en texte, quand elle est vraie.
fn vrai_ou(d: &Value, cle: &str) -> Option<String> {
    d.get(cle).filter(|v| vrai(v)).map(valeur_en_texte)
}

/// Les clauses qu'un avenant peut modifier : ce que L1242-12 et les conventions mettent au contrat et qui bouge
/// d'une paie a l'autre -- l'emploi, les dates, le lieu, la remuneration.
const CLAUSES_MODIFIABLES: [&str; 8] = [
    "salarie.emploi",
    "contrat.debut",
    "contrat.fin",
    "contrat.dates_travail",
    "contrat.lieu",
    "remuneration.detail",
    "remuneration.brut",
    "remuneration.hors_brut",
];

// ------------------------------------------------------------------ les textes

const SUJET_SIGNATURE: &str = "Contrat d'engagement {spectacle} - {mois} {annee}";
const SUJET_SIGNATURE_PLURIEL: &str = "Contrats d'engagement - {mois} {annee}";
const PREFIXE_RELANCE: &str = "Rappel - ";
const RAPPEL_RELANCE: &str = "Petit rappel : ta signature n'est pas encore arrivée. C'est le même contrat que la dernière fois.\n\n";
const RAPPEL_RELANCE_AVENANT: &str = "Petit rappel : ta signature n'est pas encore arrivée. C'est le même avenant que la dernière fois.\n\n";
const CORPS_SIGNATURE: &str = "Bonjour {prenom},\n\n{contrats}\n\nPour {le_signer}, un clic suffit : appuie sur le bouton « Je signe » de ce message, puis envoie la réponse qui s'ouvre, sans la modifier. Si ta messagerie n'affiche pas le bouton, réponds simplement à ce message en écrivant « Je signe » suivi de ce code : {code}\n\nTu recevras aussitôt {exemplaire} signé des deux parties, à conserver.{avant_travail}\n\nSi quelque chose ne correspond pas à ce qui était convenu, dis-le avant de signer plutôt qu'après : c'est plus simple à corriger.\n\nBien à toi,\n{signataire}\n{raison_sociale}\n";
const HTML_SIGNATURE: &str = "<div style=\"font-family:Arial,Helvetica,sans-serif;font-size:15px;line-height:1.5;color:#2C3E50\">\n<p>Bonjour {prenom},</p>\n<p>{contrats}</p>\n<p>Pour {le_signer}, un clic suffit : appuie sur le bouton, puis envoie la réponse qui s'ouvre, sans la modifier.</p>\n<p style=\"margin:26px 0\">\n<a href=\"{mailto}\" style=\"display:inline-block;background:#B22222;color:#ffffff;padding:14px 26px;border-radius:6px;text-decoration:none;font-weight:bold;font-size:16px\">Je signe</a>\n</p>\n<p style=\"font-size:13px;color:#555\">Si le bouton ne fait rien, réponds simplement à ce message en écrivant « Je signe » suivi de ce code : <b>{code}</b></p>\n<p>Tu recevras aussitôt {exemplaire} signé des deux parties, à conserver.{avant_travail}</p>\n<p>Si quelque chose ne correspond pas à ce qui était convenu, dis-le avant de signer plutôt qu'après : c'est plus simple à corriger.</p>\n<p>Bien à toi,<br>{signataire}<br>{raison_sociale}</p>\n</div>\n";
pub const CORPS_REPONSE: &str = "Je soussigné(e) {nom} déclare avoir lu et accepter {quoi}.\n\nCode de signature : {code}\n";

// ------------------------------------------------------------------ les petits outils

/// Un contrat vise par la page : (salarie, debut, numero facultatif).
pub type Cible = (String, String, Option<String>);

/// LES CONTRATS QUE LA PAGE CHOISIT. La selection est par contrat (`contrats`) : deux contrats du meme salarie qui
/// commencent le meme jour se distinguent par leur numero ; sans numero, elle vaut pour tous ceux de ce salarie a ce
/// debut. `salaries` prend tous les contrats de ces salaries. Rien : tous les contrats du mois.
#[derive(Default)]
pub struct Choisis {
    pub salaries: Option<Vec<String>>,
    pub contrats: Option<Vec<Cible>>,
}

/// `jj_mm_aaaa(x)` : `ValueError` si `x` ne se lit pas comme une date.
pub(crate) fn jj(iso: &str) -> R<String> {
    socle::date::Jour::lire(iso).map(|j| j.jj_mm_aaaa()).map_err(|m| erreur("ValueError", m))
}

/// « du 12/01/2026 au 30/06/2026 », ou « à partir du 12/01/2026 » pour un contrat sans terme.
fn periode_de(p: &Paie) -> String {
    match p.engagement.fin {
        Some(f) => format!("du {} au {}", p.engagement.debut.jj_mm_aaaa(), f.jj_mm_aaaa()),
        None => format!("à partir du {}", p.engagement.debut.jj_mm_aaaa()),
    }
}

/// CE QUI A ATTEINT LE SALARIE EST PARTI : d'un envoi coupe en route, les versions des seuls salaries servis (les
/// `servis` premiers messages, qui partent dans l'ordre), pour la ligne d'echec du journal.
fn versions_des_servis(versions: &Value, messages: &[Message], servis: usize) -> Value {
    let cles: Vec<&str> = messages[..servis.min(messages.len())].iter().map(|m| m.cle.as_str()).collect();
    json!(versions.as_array().into_iter().flatten().filter(|v| v["salarie"].as_str().is_some_and(|c| cles.contains(&c))).collect::<Vec<_>>())
}

/// `a, b et c`.
fn et_fr(morceaux: &[String]) -> String {
    if morceaux.len() < 2 {
        return morceaux.concat();
    }
    format!("{} et {}", morceaux[..morceaux.len() - 1].join(", "), morceaux[morceaux.len() - 1])
}

/// `code_signature` : `SIG-` puis six signes tires au hasard, sans 0/O ni 1/I/L, pour qu'il se relise sans
/// hesiter s'il faut le recopier a la main.
pub fn code_signature() -> String {
    documents::fiche_questionnaire::code_au_hasard("SIG-")
}

/// `signataire` et `raison` des courriels : le prenom d'affichage du signataire (sinon son prenom, capitalise), et
/// le nom d'affichage de la compagnie (sinon sa raison sociale).
pub fn signataire_et_raison(ctx: &Contexte) -> R<(String, String)> {
    let sg = requis(&ctx.entreprise, "signataire")?;
    let signataire = match vrai_ou(sg, "prenom_affichage") {
        Some(p) => p,
        None => capitaliser(&sg.get("prenom").map(valeur_en_texte).unwrap_or_default()),
    };
    let raison = match vrai_ou(&ctx.entreprise, "nom_affichage") {
        Some(r) => r,
        None => valeur_en_texte(requis(&ctx.entreprise, "raison_sociale")?),
    };
    Ok((signataire, raison))
}

/// Le prenom d'affichage, sinon le prenom.
fn prenom_de(sal: &Value) -> R<String> {
    match vrai_ou(sal, "prenom_affichage") {
        Some(p) => Ok(p),
        None => Ok(valeur_en_texte(requis(sal, "prenom")?)),
    }
}

/// « Prenom Nom » tel que la reponse « Je signe » le dit : `prenom_affichage or prenom`, `nom_affichage or nom`.
fn nom_complet_de(sal: &Value) -> R<String> {
    let nom = match vrai_ou(sal, "nom_affichage") {
        Some(n) => n,
        None => valeur_en_texte(requis(sal, "nom")?),
    };
    Ok(format!("{} {}", prenom_de(sal)?, nom))
}

/// Ce que le contrat dit, clause par clause (l'avenant qui le modifiera doit citer l'ancien
/// texte a cote du nouveau). `{}` si le contrat ne s'ecrit pas.
pub fn clauses_du_contrat(ctx: &Contexte, p: &Paie, ent: &Value) -> Value {
    match documents::contrat::valeurs_de_paie(ctx, ent, p) {
        Ok(v) => {
            let mut m = Map::new();
            for k in CLAUSES_MODIFIABLES {
                m.insert(k.to_string(), v.get(k).map_or(Value::Null, |x| json!(x)));
            }
            Value::Object(m)
        }
        Err(_) => json!({}),
    }
}

/// Le message d'une demande de signature : le texte, sa version HTML avec le bouton « Je signe », le PDF en piece
/// jointe. `liste` : ce que le courriel dit en premiere ligne ; `quoi` : ce que la reponse « Je signe » declare avoir
/// lu et accepte.
struct Demande<'a> {
    cle: &'a str,
    sal: &'a Value,
    adresse: &'a str,
    sujet: &'a str,
    liste: &'a str,
    quoi: &'a str,
    le_signer: &'a str,
    exemplaire: &'a str,
    avant_travail: &'a str,
    code: &'a str,
    expediteur: &'a str,
    signataire: &'a str,
    raison: &'a str,
}

fn message_de_signature(d: &Demande, pieces: Vec<PathBuf>) -> R<Message> {
    let prenom = prenom_de(d.sal)?;
    let reponse = envoi::formater(CORPS_REPONSE, &[("nom", nom_complet_de(d.sal)?), ("quoi", d.quoi.to_string()), ("code", d.code.to_string())]);
    let lien = mailto(d.expediteur, &format!("Re: {}", d.sujet), &reponse);
    let champs = |echappe: bool| -> Vec<(&'static str, String)> {
        let e = |s: &str| if echappe { html_echapper(s) } else { s.to_string() };
        vec![
            ("prenom", e(&prenom)),
            ("signataire", e(d.signataire)),
            ("raison_sociale", e(d.raison)),
            ("le_signer", e(d.le_signer)),
            ("expediteur", e(d.expediteur)),
            ("code", e(d.code)),
            ("exemplaire", e(d.exemplaire)),
            ("avant_travail", e(d.avant_travail)),
        ]
    };
    let mut texte = champs(false);
    texte.push(("contrats", d.liste.to_string()));
    let mut html = champs(true);
    html.push(("contrats", html_echapper(d.liste).replace('\n', "<br>")));
    html.push(("mailto", html_echapper(&lien)));
    Ok(Message {
        cle: d.cle.to_string(),
        nom: valeur_en_texte(requis(d.sal, "nom")?),
        destinataire: d.adresse.to_string(),
        sujet: d.sujet.to_string(),
        corps: envoi::formater(CORPS_SIGNATURE, &texte),
        html: envoi::formater(HTML_SIGNATURE, &html),
        pieces,
    })
}

// ------------------------------------------------------------------ les contrats choisis

/// Les contrats choisis parmi `paies`, par salarie, dans l'ordre du mois : `(cle, ses paies, leur rang)`. Seuls partent
/// ceux qui se font signer ici (`engagement::contrat_a_signer`).
fn contrats_choisis(ctx: &Contexte, choisis: &Choisis, mois: &str, paies: &[Paie]) -> R<Vec<(String, Vec<Paie>, Vec<usize>)>> {
    let voulus = &choisis.salaries;
    let cibles = &choisis.contrats;
    let vise = |c: &Cible, cle: &str, p: &Paie| c.0 == cle && c.1 == p.engagement.debut.iso() && c.2.as_ref().map_or(true, |n| p.engagement.numero.as_deref() == Some(n.as_str()));
    let mut par_salarie: Vec<(String, Vec<Paie>, Vec<usize>)> = Vec::new();
    for (rang, p) in paies.iter().enumerate() {
        let cle = cle_de(p);
        if !engagement::contrat_a_signer(&ctx.racine, &p.engagement).map_err(Echec::from)? {
            continue;
        }
        if voulus.as_ref().is_some_and(|v| !v.is_empty() && !v.contains(&cle)) {
            continue;
        }
        if cibles.as_ref().is_some_and(|c| !c.iter().any(|x| vise(x, &cle, p))) {
            continue;
        }
        match par_salarie.iter_mut().find(|(k, _, _)| *k == cle) {
            Some((_, ps, rs)) => {
                ps.push(p.clone());
                rs.push(rang);
            }
            None => par_salarie.push((cle, vec![p.clone()], vec![rang])),
        }
    }
    if let Some(c) = &cibles {
        let mut manquants: Vec<&Cible> = c.iter().filter(|x| !par_salarie.iter().any(|(k, ps, _)| ps.iter().any(|p| vise(x, k, p)))).collect();
        manquants.sort();
        manquants.dedup();
        if !manquants.is_empty() {
            return Err(refus(format!(
                "Ces contrats ne sont pas dans la saisie de {} : {}. Rien n'est parti.",
                mois,
                manquants.iter().map(|x| format!("{} au {}{}", x.0, x.1, x.2.as_ref().map_or(String::new(), |n| format!(" (n° {})", n)))).collect::<Vec<_>>().join(", ")
            )));
        }
    }
    Ok(par_salarie)
}

/// Le contrat tel qu'il part, pour le journal : qui, quand, sous quel numero, dans quel fichier, et ce qu'il dit, clause
/// par clause. La version de l'engagement sur laquelle il part, la ligne du journal la porte (`journal::versions_des_contrats`).
#[allow(clippy::too_many_arguments)]
fn entree_du_contrat(ctx: &Contexte, p: &Paie, cle: &str, pdf: &std::path::Path, titre: &str, mois: &str, ent: &Value) -> Map<String, Value> {
    let mut c = Map::new();
    c.insert("salarie".into(), json!(cle));
    c.insert("debut".into(), json!(p.engagement.debut.iso()));
    c.insert("fin".into(), p.engagement.fin.map_or(Value::Null, |f| json!(f.iso())));
    c.insert("numero".into(), p.engagement.numero.clone().map_or(Value::Null, Value::String));
    c.insert("fichier".into(), json!(nom_du(pdf)));
    c.insert("spectacle".into(), json!(titre));
    c.insert("mois".into(), json!(mois));
    c.insert("clauses".into(), clauses_du_contrat(ctx, p, ent));
    c
}

/// Le nom du PDF des contrats d'un salarie : `Contrat_<cle>_<numero>_<mois>.pdf`, `Contrats_..._<n1>+<n2>_...` quand il en
/// porte plusieurs (une page par contrat).
fn nom_du_pdf(cle: &str, ps: &[Paie], etiq: &str, suffixe: &str) -> String {
    let numeros: Vec<String> = ps.iter().map(|p| p.engagement.numero.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| "sans-numero".to_string())).collect();
    format!("Contrat{}_{}_{}_{}{}.pdf", if ps.len() > 1 { "s" } else { "" }, cle, numeros.join("+"), etiq, suffixe)
}

// ------------------------------------------------------------------ contrats-imprimer
/// LE PDF A IMPRIMER : les pieces `pieces`, dans leur ordre, reunies en un seul fichier `<dossier>/Impression_<quoi>_<etiq>.pdf`
/// (une impression d'un mois remplace la precedente : les pieces parties gardent chacune leur fichier).
/// Le debut du nom du PDF a imprimer : ce fichier n'est la piece de personne, il ne se liste pas parmi les documents.
pub const PREFIXE_IMPRESSION: &str = "Impression_";

pub fn reunir_pour_imprimer(dossier: &std::path::Path, quoi: &str, etiq: &str, pieces: &[std::path::PathBuf]) -> R<std::path::PathBuf> {
    let octets = documents::fabrique::fusion_pieces(&pieces.iter().map(|p| (p.clone(), None)).collect::<Vec<_>>())?;
    let f = dossier.join(format!("{}{}_{}.pdf", PREFIXE_IMPRESSION, quoi, etiq));
    // Un mois dont seul un avenant s'imprime n'a pas encore son dossier d'envoi.
    std::fs::create_dir_all(dossier).map_err(|e| erreur("OSError", format!("{} : {}", dossier.display(), e)))?;
    std::fs::write(&f, octets).map_err(|e| erreur("OSError", format!("{} : {}", f.display(), e)))?;
    Ok(f)
}

/// La route `contrats-imprimer` : les contrats choisis (comme `demander-signature` ; `None` : aucun) et les avenants
/// choisis (`avenants`, deja verifies par la route), chacun dans son PDF dans `sorties/<mois>/envoi/` (la piece partie de
/// chacun : un PDF de contrats par salarie, un PDF par avenant), et tous reunis en un seul PDF a imprimer (`impression`),
/// a remettre en main propre. L'IMPRESSION EST UN DEPART : chaque contrat y part sous son numero, chaque avenant sous le
/// sien, et le journal le note (`contrat-signature`, `main_propre` ; une ligne pour les contrats, une pour les avenants) ;
/// ce qui revient signe se note ensuite d'un clic (`contrat-recu`). Rien ne quitte le poste : aucun mot de passe. `qui` :
/// la ligne du journal.
pub fn contrats_imprimer(ctx: &Contexte, mois: &str, choisis: Option<&Choisis>, avenants: &[(String, Vec<UnAvenant>)], qui: &str) -> R<Value> {
    let mois = mois.to_string();
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs).map_err(Echec::from)?;
    let (data, paies) = documents::ecrire_documents::paies_aux_numeros_declares(ctx, &mois, None, &departs)?;
    let s_mois = data.get("mois").map(socle::date::en_texte).ok_or_else(|| erreur("KeyError", socle::fichiers::absent("mois")))?;
    let par_salarie = match choisis {
        Some(c) => contrats_choisis(ctx, c, &mois, &paies)?,
        None => Vec::new(),
    };
    if par_salarie.is_empty() && avenants.is_empty() {
        return Err(refus(format!("Aucun contrat a imprimer pour {}.", mois)));
    }
    let dossier = ctx.racine.join("sorties").join(&s_mois).join("envoi");
    let etiq = socle::date::etiquette_du_mois(&s_mois).map_err(|m| erreur("ValueError", m))?;
    let ent = documents::avenant::ent_du_mois(ctx, &s_mois)?;
    let (mut fichiers, mut contrats) = (Vec::new(), Vec::new());
    for (cle, ps, _) in &par_salarie {
        let pdf = dossier.join(nom_du_pdf(cle, ps, &etiq, ""));
        documents::contrat::ecrire_contrat(ctx, ps, &ent, &pdf, &[], None)?;
        fichiers.push(json!(pdf.to_string_lossy()));
        for p in ps.iter() {
            contrats.push(Value::Object(entree_du_contrat(ctx, p, cle, &pdf, &p.titre()?, &s_mois, &ent)));
        }
    }
    let mut saisies: HashMap<String, (Value, Vec<Paie>)> = HashMap::new();
    let aujourdhui_jj = ctx.aujourdhui.jj_mm_aaaa();
    let mut lignes_avenants: Vec<Value> = Vec::new();
    for (salarie, aa) in avenants {
        for a in aa {
            let (pdf, c, _) = avenant_ecrit(ctx, salarie, a, &mut saisies, &departs, &aujourdhui_jj)?;
            fichiers.push(json!(pdf.to_string_lossy()));
            lignes_avenants.push(Value::Object(c));
        }
    }
    let pdfs: Vec<std::path::PathBuf> = fichiers.iter().filter_map(Value::as_str).map(std::path::PathBuf::from).collect();
    let impression = reunir_pour_imprimer(&dossier, "contrats", &etiq, &pdfs)?;
    let mut lignes = Vec::new();
    if !contrats.is_empty() {
        let detail = json!({"contrats": contrats, "main_propre": true});
        let versions = envois::journal::versions_des_contrats(ctx, &contrats, &departs).map_err(Echec::from)?;
        let reponse = format!("{} contrat(s) imprime(s) pour une remise en main propre", contrats.len());
        lignes.push(noter_envoi(ctx, "contrat-signature", Some(&mois), qui, true, "", &reponse, Some(&detail), Some(&versions))?);
    }
    if !lignes_avenants.is_empty() {
        let detail = json!({"contrats": lignes_avenants, "main_propre": true, "acte": "avenant"});
        let versions = envois::journal::versions_des_contrats(ctx, &lignes_avenants, &departs).map_err(Echec::from)?;
        let reponse = format!(
            "{} imprime(s) pour une remise en main propre",
            lignes_avenants.iter().map(|c| format!("avenant n° {} au contrat {}", valeur_en_texte(&c["avenant"]), valeur_en_texte(&c["numero"]))).collect::<Vec<_>>().join("; ")
        );
        lignes.push(noter_envoi(ctx, "contrat-signature", Some(&mois), qui, true, "", &reponse, Some(&detail), Some(&versions))?);
    }
    Ok(json!({"fichiers": fichiers, "impression": nom_du(&impression), "contrats": contrats, "avenants": lignes_avenants, "lignes": lignes}))
}

// ------------------------------------------------------------------ demander-signature

/// Envoie a chaque salarie son contrat, avec le bouton « Je signe ».
///
/// `remplace` : le contrat refait EN ENTIER, qui annule et remplace celui qui est deja parti -- une phrase en tete
/// le dit. Refuse pour un contrat jamais parti, ou dont l'execution a commence : celui-la se modifie par un
/// avenant. `relance` : redemander un contrat deja demande et pas revenu, TEL QUEL -- objet « Rappel », une ligne
/// de rappel, et le journal le dit ; ce qu'elle refuse, `verifier_demande` le dit avant le mot de passe.
///
/// LA SELECTION EST PAR CONTRAT, pas par salarie (`contrats`). `salaries` reste accepte (tous les contrats de ces
/// salaries). UN COURRIEL PAR SALARIE, UN PDF PAR COURRIEL, tous ses contrats dedans (une page par contrat), ecrit
/// dans `sorties/<mois>/envoi/` avec les MEMES numeros que le fichier groupe.
pub fn demander_signature(ctx: &Contexte, d: &DemandeDeSignature, client: &mut dyn Courriel) -> R<Value> {
    let (mois, relance, remplace) = (d.mois.to_string(), d.relance, d.remplace);
    // Chaque contrat sous le numero qu'il porte, ou celui qui lui serait donne : c'est celui-la qui part.
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs).map_err(Echec::from)?;
    let (data, paies) = documents::ecrire_documents::paies_aux_numeros_declares(ctx, &mois, None, &departs)?;
    let s_mois = data.get("mois").map(socle::date::en_texte).ok_or_else(|| erreur("KeyError", socle::fichiers::absent("mois")))?;
    let dossier = ctx.racine.join("sorties").join(&s_mois).join("envoi");
    let etiq = socle::date::etiquette_du_mois(&s_mois).map_err(|m| erreur("ValueError", m))?;

    let (an, nom_mois) = an_et_nom_du_mois(&s_mois)?;
    let ent = documents::avenant::ent_du_mois(ctx, &s_mois)?;
    // UN SALARIE SANS COURRIEL n'empeche pas les autres : son contrat ne part pas par courriel, il s'imprime ; les
    // autres partent. Les adresses se lisent avant d'ecrire le moindre PDF : un refus ne laisse rien sur le disque.
    let (par_salarie, noms_sans) = avec_et_sans_courriel(contrats_choisis(ctx, &d.choisis, &mois, &paies)?)?;

    let aujourdhui = ctx.aujourdhui.iso();
    let mut remplacements: Vec<((String, String, String), (String, String))> = Vec::new();
    if remplace {
        let lignes = envois::journal::lire(&ctx.racine, None, None, false).map_err(Echec::from)?;
        for (k, ps, _) in &par_salarie {
            for p in ps {
                let debut = p.engagement.debut.iso();
                // Le contrat qui part est remplace, lui (son numero) : un autre contrat du salarie n'a rien a remplacer.
                let n = p.engagement.numero.clone().unwrap_or_default();
                let Some(parti) = envois::etats::contrat_parti(&lignes, k, &n)? else {
                    return Err(refus(format!("Le contrat de {} du {} n'est jamais parti : il n'a rien à remplacer, il s'envoie. Rien n'est parti.", k, jj(&debut)?)));
                };
                if debut <= aujourdhui {
                    return Err(refus(format!(
                        "Le contrat de {} du {} a commencé : un contrat en cours d'exécution ne s'annule pas, il se modifie par un avenant. Rien n'est parti.",
                        k,
                        jj(&debut)?
                    )));
                }
                let numero = if vrai(&parti.numero) { valeur_en_texte(&parti.numero) } else { "(sans numéro)".to_string() };
                remplacements.push(((k.clone(), debut, p.engagement.numero.clone().unwrap_or_default()), (numero, jj(&parti.date())?)));
            }
        }
    }

    let (signataire, raison) = signataire_et_raison(ctx)?;
    let expediteur = client.compte().to_lowercase();
    let mut messages: Vec<Message> = Vec::new();
    let mut contrats: Vec<Value> = Vec::new();
    for (cle, ps, _) in &par_salarie {
        let sal = &ps[0].salarie;
        let adresse = vrai_ou(sal, "email").unwrap_or_default().trim().to_lowercase();
        let code = code_signature();
        // Le contrat qui en remplace un autre porte son propre nom : le PDF parti avant lui reste sur le disque.
        let pdf = dossier.join(nom_du_pdf(cle, ps, &etiq, if remplace { "_remplace" } else { "" }));
        documents::contrat::ecrire_contrat(ctx, ps, &ent, &pdf, &remplacements, None)?;
        let mut spectacles: Vec<String> = Vec::new();
        let mut titres: Vec<String> = Vec::new();
        for p in ps {
            let t = p.titre()?;
            if !spectacles.contains(&t) {
                spectacles.push(t.clone());
            }
            titres.push(t);
        }
        let sujet_de = |spectacle: &str| envoi::formater(SUJET_SIGNATURE, &[("spectacle", spectacle.to_string()), ("mois", nom_mois.to_string()), ("annee", an.to_string())]);
        let du_au = |p: &Paie, titre: &str| -> R<String> { Ok(format!("{}, {}", titre, periode_de(p))) };
        let numero_de = |p: &Paie| p.engagement.numero.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| "(sans numéro)".to_string());
        let (mut sujet, mut liste, le_signer, quoi, exemplaire);
        if ps.len() == 1 {
            let p = &ps[0];
            // Le contrat d'un spectacle se nomme par lui ; celui d'un permanent par sa nature (`contrat_ecrit.nom`).
            let nom_c = p.engagement.nat().map_err(Echec::from)?.nom_du_contrat();
            let pour = if vrai(&p.spectacle) { format!(" pour {}", titres[0]) } else { String::new() };
            sujet = if vrai(&p.spectacle) { sujet_de(&titres[0]) } else { format!("{} - {} {}", capitaliser(&nom_c), nom_mois, an) };
            liste = format!("Tu trouveras ci-joint ton {}{}, {}.", nom_c, pour, periode_de(p));
            le_signer = "le signer";
            quoi = format!("le {} n° {}{}, {}", nom_c, numero_de(p), pour, periode_de(p));
            exemplaire = "ton contrat";
        } else {
            sujet = if spectacles.len() == 1 {
                sujet_de(&spectacles[0])
            } else {
                envoi::formater(SUJET_SIGNATURE_PLURIEL, &[("mois", nom_mois.to_string()), ("annee", an.to_string())])
            };
            let mut lignes = Vec::new();
            for (p, t) in ps.iter().zip(&titres) {
                lignes.push(format!("  - {}", du_au(p, t)?));
            }
            liste = format!("Tu trouveras ci-joint tes {} contrats d'engagement, dans un seul document (un par page) :\n{}", ps.len(), lignes.join("\n"));
            le_signer = "les signer";
            let mut morceaux = Vec::new();
            for (p, t) in ps.iter().zip(&titres) {
                morceaux.push(format!("n° {} pour {}, {}", numero_de(p), t, periode_de(p)));
            }
            quoi = format!("les contrats d'engagement suivants : {}", morceaux.join(" ; "));
            exemplaire = "tes contrats";
        }
        if relance {
            sujet = format!("{}{}", PREFIXE_RELANCE, sujet);
            liste = format!("{}{}", RAPPEL_RELANCE, liste);
        }
        if remplace {
            liste.push_str(&format!(
                "\n\n{} celui que tu as reçu avant, qui est réputé n'avoir jamais produit d'effet.",
                if ps.len() == 1 { "Il annule et remplace" } else { "Chacun annule et remplace" }
            ));
        }
        let message = message_de_signature(
            &Demande {
                cle,
                sal,
                adresse: &adresse,
                sujet: &sujet,
                liste: &liste,
                quoi: &quoi,
                le_signer,
                exemplaire,
                avant_travail: " Le contrat doit être signé avant le début du travail.",
                code: &code,
                expediteur: &expediteur,
                signataire: &signataire,
                raison: &raison,
            },
            vec![pdf.clone()],
        )?;
        messages.push(message);
        for (p, t) in ps.iter().zip(&titres) {
            let debut = p.engagement.debut.iso();
            let mut c = entree_du_contrat(ctx, p, cle, &pdf, t, &s_mois, &ent);
            // Ce qu'il faut pour reconnaitre la reponse : le code, et l'adresse dont elle doit venir.
            c.insert("code".into(), json!(code));
            c.insert("adresse".into(), json!(adresse));
            c.insert("relance".into(), json!(relance));
            if remplace {
                let propre = p.engagement.numero.clone().unwrap_or_default();
                let numero = remplacements.iter().find(|(k, _)| *k == (cle.clone(), debut.clone(), propre.clone())).map(|(_, (n, _))| n.clone()).unwrap_or_default();
                c.insert("acte".into(), json!("remplace"));
                c.insert("remplace".into(), json!(numero));
            }
            contrats.push(Value::Object(c));
        }
    }

    if messages.is_empty() {
        return Err(refus(format!("Aucun contrat a envoyer pour {}.", mois)));
    }
    // La version de l'engagement sur laquelle chaque contrat part, lue avant l'envoi.
    let versions = envois::journal::versions_des_contrats(ctx, &contrats, &departs).map_err(Echec::from)?;
    let mut identifiants: Vec<String> = Vec::new();
    let partis = match envoi::envoyer_suivi(&messages, &expediteur, client, Some(&mut identifiants), None) {
        Ok(p) => p,
        Err(e) => {
            let servis = versions_des_servis(&versions, &messages, e.partis.len());
            noter_echec_d_envoi(ctx, "contrat-signature", Some(&mois), "interface", &e.partis, &e.echoue, &e.message, Some(json!({"contrats": contrats})), Some(&servis))?;
            return Err(echec_d_envoi(e.message));
        }
    };
    // L'identifiant de chaque message parti, sur chacun de ses contrats : la releve l'ecarte de ses candidats.
    for c in contrats.iter_mut() {
        let cle = c["salarie"].as_str().unwrap_or("");
        let ident = messages.iter().zip(&identifiants).find(|(m, _)| m.cle == cle).map(|(_, i)| i.clone()).unwrap_or_default();
        if let Value::Object(o) = c {
            o.insert("message".into(), json!(ident));
        }
    }
    let detail = json!({"contrats": contrats, "expediteur": expediteur, "relance": relance});
    let ligne = noter_envoi(
        ctx,
        "contrat-signature",
        Some(&mois),
        "interface",
        true,
        &partis.join(", "),
        &format!("{} contrat(s) {} a la signature", contrats.len(), if relance { "relance(s)" } else { "envoye(s)" }),
        Some(&detail),
        Some(&versions),
    )?;
    let avertissements: Vec<String> = if noms_sans.is_empty() { Vec::new() } else { vec![sans_courriel_dit(&noms_sans)] };
    Ok(json!({"partis": partis, "contrats": contrats, "expediteur": expediteur, "ligne": ligne, "avertissements": avertissements}))
}

/// Ce que la demande de signature et son apercu disent des contrats coches dont le salarie n'a pas de courriel : ils ne
/// partent pas par courriel, ils s'impriment pour se signer en main propre, ou le courriel se saisit dans la fiche.
pub fn sans_courriel_dit(noms: &[String]) -> String {
    if noms.len() == 1 {
        format!(
            "Pas de courriel dans la fiche de {} : son contrat ne part pas à la signature par courriel. Imprimez-le pour le faire signer en main propre, ou saisissez son courriel dans sa fiche.",
            noms[0]
        )
    } else {
        format!(
            "Pas de courriel dans les fiches de {} : leurs contrats ne partent pas à la signature par courriel. Imprimez-les pour les faire signer en main propre, ou saisissez leur courriel dans leur fiche.",
            noms.join(", ")
        )
    }
}

/// Ce que l'envoi des avenants et l'apercu de la signature disent des avenants coches dont le salarie n'a pas de
/// courriel : ils ne partent pas par courriel, ils s'impriment pour se signer en main propre, ou le courriel se saisit.
pub fn avenants_sans_courriel_dit(noms: &[String]) -> String {
    if noms.len() == 1 {
        format!(
            "Pas de courriel dans la fiche de {} : son avenant ne part pas à la signature par courriel. Imprimez-le pour le faire signer en main propre, ou saisissez son courriel dans sa fiche.",
            noms[0]
        )
    } else {
        format!(
            "Pas de courriel dans les fiches de {} : leurs avenants ne partent pas à la signature par courriel. Imprimez-les pour les faire signer en main propre, ou saisissez leur courriel dans leur fiche.",
            noms.join(", ")
        )
    }
}

/// L'annee et le nom du mois d'un « AAAA-MM », pour les sujets des courriels (« octobre 2026 ») ; un mois illisible ou
/// hors 01-12 se refuse (`date::nom_du_mois`).
pub fn an_et_nom_du_mois(mois: &str) -> R<(i32, &'static str)> {
    let lire = |t: Option<&str>| -> R<i64> {
        let t = t.unwrap_or("");
        t.trim().parse::<i64>().map_err(|_| erreur("ValueError", format!("nombre entier illisible : {}", repr(t))))
    };
    let an = lire(mois.get(..4))?;
    let nom = socle::date::nom_du_mois(lire(mois.get(5..7))?, socle::date::Graphie::Accentuee).map_err(|e| erreur("IndexError", e))?;
    Ok((an as i32, nom))
}

/// LA DEMANDE DE SIGNATURE : le mois, ce qu'elle est (une relance, un contrat refait qui remplace celui qui est parti),
/// les contrats choisis. `etats_des_contrats` : pour une relance, l'etat de chaque contrat du mois tel que
/// `signature-apercu` le montre (demande, revenu, caduc).
pub struct DemandeDeSignature<'a> {
    pub mois: &'a str,
    pub relance: bool,
    pub remplace: bool,
    pub choisis: Choisis,
    pub etats_des_contrats: Vec<Value>,
}

/// CE QUE LA DEMANDE REFUSE D'ELLE-MEME, avant la boite et avant le mot de passe : la preuve ne se demande que pour ce
/// qui part. Une relance renvoie le meme contrat (elle n'en remplace aucun), et seulement un contrat demande par courriel,
/// pas revenu, a jour. Rien n'est ecrit.
pub fn verifier_demande(ctx: &Contexte, d: &DemandeDeSignature) -> R<()> {
    if d.relance && d.remplace {
        return Err(refus("Une relance renvoie le même contrat : elle n'en remplace aucun."));
    }
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs).map_err(Echec::from)?;
    let (_, paies) = documents::ecrire_documents::paies_aux_numeros_declares(ctx, d.mois, None, &departs)?;
    let (par_salarie, _) = avec_et_sans_courriel(contrats_choisis(ctx, &d.choisis, d.mois, &paies)?)?;
    if !d.relance {
        return Ok(());
    }
    let cles: Vec<(String, String)> = par_salarie.iter().flat_map(|(k, ps, _)| ps.iter().map(|p| (k.clone(), p.engagement.numero.clone().unwrap_or_default()))).collect();
    verifier_relance(&d.etats_des_contrats, &cles)
}

/// LES CONTRATS QUI PARTENT PAR COURRIEL : ceux dont la fiche du salarie a un courriel ; les autres s'impriment (leurs
/// noms, pour le dire). Refus si aucun contrat choisi n'a de courriel : rien ne partirait.
type ParSalarie = Vec<(String, Vec<Paie>, Vec<usize>)>;
fn avec_et_sans_courriel(choisis: ParSalarie) -> R<(ParSalarie, Vec<String>)> {
    let (avec, sans): (ParSalarie, ParSalarie) = choisis.into_iter().partition(|(_, ps, _)| !vrai_ou(&ps[0].salarie, "email").unwrap_or_default().trim().is_empty());
    let noms: Vec<String> = sans.iter().map(|(k, ps, _)| socle::texte::salarie_nomme_ou(Some(&ps[0].salarie), k)).collect();
    if avec.is_empty() && !noms.is_empty() {
        return Err(refus(format!("{} Rien n'est parti.", sans_courriel_dit(&noms))));
    }
    Ok((avec, noms))
}

/// Une relance ne vaut que pour un contrat demande par courriel, pas revenu, a jour.
fn verifier_relance(etats: &[Value], cles: &[(String, String)]) -> R<()> {
    for (salarie, numero) in cles {
        let c = etats.iter().rev().find(|c| c["salarie"].as_str() == Some(salarie) && valeur_en_texte(&envois::etats::numero_de_cle(&c["numero"])) == *numero);
        let pb = match c {
            None => "n'a jamais ete demande : il s'envoie",
            Some(c) if !vrai(&c["demande_le"]) => "n'a jamais ete demande : il s'envoie",
            Some(c) if vrai(&c["caduc"]) => "a change depuis son envoi : il se modifie par un avenant",
            Some(c) if vrai(&c["recu_le"]) => "est deja revenu signe",
            // Un contrat imprime n'a pas de courriel a renvoyer : il se remet en main propre.
            Some(c) if c["par"].as_str() == Some("imprime") => "est parti imprime, a remettre en main propre",
            Some(_) => continue,
        };
        return Err(refus(format!("Le contrat n° {} de {} {}, il ne se relance pas. Rien n'est parti.", numero, salarie, pb)));
    }
    Ok(())
}

// ------------------------------------------------------------------ les avenants

/// Ce que `avenant-apercu` dit d'un avenant a envoyer, et la paie du contrat.
pub struct UnAvenant {
    pub apercu: Value,
    pub effet: String,
}

impl UnAvenant {
    pub fn texte(&self, cle: &str) -> String {
        self.apercu.get(cle).map(valeur_en_texte).unwrap_or_default()
    }
    fn numero(&self) -> i64 {
        self.apercu["numero"].as_i64().unwrap_or(0)
    }
    /// `a["contrat"] or "(sans numéro)"`.
    fn contrat(&self) -> String {
        if vrai(&self.apercu["contrat"]) {
            valeur_en_texte(&self.apercu["contrat"])
        } else {
            "(sans numéro)".to_string()
        }
    }
    fn modifications(&self) -> Vec<Value> {
        self.apercu["modifications"].as_array().cloned().unwrap_or_default()
    }
    /// L'avenant deja parti et pas revenu, qui repart tel quel (`apercu.fichier` : sa piece partie).
    pub fn relance(&self) -> bool {
        vrai(&self.apercu["relance"])
    }
}

/// L'AVENANT `a` DU CONTRAT DE `salarie`, ECRIT : son PDF dans `sorties/<mois>/envoi/` (la piece partie, a cote de celle
/// du contrat), et ce que le journal en dira. Une relance reprend le PDF parti, sans le reecrire. Rend le PDF, l'entree
/// du journal et la fiche du salarie.
fn avenant_ecrit(
    ctx: &Contexte,
    salarie: &str,
    a: &UnAvenant,
    saisies: &mut HashMap<String, (Value, Vec<Paie>)>,
    departs: &engagement::departs::Departs,
    aujourdhui_jj: &str,
) -> R<(PathBuf, Map<String, Value>, Value)> {
    let mois_a = a.texte("mois");
    if !saisies.contains_key(&mois_a) {
        let (d, p) = documents::ecrire_documents::paies_aux_numeros_declares(ctx, &mois_a, None, departs)?;
        saisies.insert(mois_a.clone(), (d, p));
    }
    let (_, paies) = &saisies[&mois_a];
    let debut = a.texte("debut");
    let numero_j = valeur_en_texte(&envois::etats::numero_de_cle(&a.apercu["contrat"]));
    let Some(rang_p) = envois::etats::paie_du_contrat(paies, salarie, &numero_j)? else {
        return Err(refus(format!("Aucun contrat de {} commençant le {} dans les engagements de {}.", salarie, jj(&debut)?, mois_a)));
    };
    let p = &paies[rang_p];
    let ent = documents::avenant::ent_du_mois(ctx, &mois_a)?;
    let dossier = ctx.racine.join("sorties").join(&mois_a).join("envoi");
    let pdf = if a.relance() {
        let f = dossier.join(a.texte("fichier"));
        if a.texte("fichier").is_empty() || !f.is_file() {
            return Err(refus(format!("L'avenant n° {} de {} n'est plus dans {} : il ne se relance pas. Rien n'est parti.", a.numero(), salarie, dossier.display())));
        }
        f
    } else {
        let f = dossier.join(format!(
            "Avenant{}_{}_{}_{}.pdf",
            a.numero(),
            salarie,
            if vrai(&a.apercu["contrat"]) { valeur_en_texte(&a.apercu["contrat"]) } else { "sans-numero".to_string() },
            socle::date::etiquette_du_mois(&mois_a).map_err(|m| erreur("ValueError", m))?
        ));
        documents::contrat::ecrire_avenant(ctx, p, &ent, &f, &a.modifications(), a.numero(), &jj(&a.texte("date_contrat"))?, &jj(&a.effet)?, aujourdhui_jj, None)?;
        f
    };
    let modifie: Vec<Value> = a.modifications().iter().map(|m| m["article"].clone()).collect();
    let mut c = Map::new();
    c.insert("salarie".into(), json!(salarie));
    c.insert("debut".into(), json!(debut));
    c.insert("fin".into(), a.apercu["fin"].clone());
    c.insert("numero".into(), a.apercu["contrat"].clone());
    c.insert("fichier".into(), json!(nom_du(&pdf)));
    c.insert("spectacle".into(), a.apercu["spectacle"].clone());
    c.insert("mois".into(), json!(mois_a));
    c.insert("acte".into(), json!("avenant"));
    c.insert("avenant".into(), a.apercu["numero"].clone());
    c.insert("effet".into(), json!(a.effet));
    c.insert("modifie".into(), Value::Array(modifie));
    c.insert("modifications".into(), Value::Array(a.modifications()));
    c.insert("clauses".into(), clauses_du_contrat(ctx, p, &ent));
    Ok((pdf, c, p.salarie.clone()))
}

/// Ce que `envoyer_avenants` rend.
pub struct Avenants {
    pub partis: Vec<String>,
    pub avenants: Vec<Value>,
    pub lignes: Vec<Value>,
}

/// Ecrit les avenants et les envoie a la signature, avec le bouton « Je signe » -- le meme circuit que le contrat. UN
/// COURRIEL PAR SALARIE : ses avenants en pieces jointes -- un PDF chacun, qui garde son numero -- et un seul code, dont la
/// reponse vaut pour tous. `par_salarie` : les avenants deja verifies par la route (chacun a un objet, une date d'effet
/// lisible, un destinataire).
///
/// Le journal `contrat-signature` porte une ligne par courriel, `acte: "avenant"` et le numero de chacun ;
/// la version de chaque engagement y est notee, et c'est elle qui dit ensuite que le contrat est a jour.
pub fn envoyer_avenants(ctx: &Contexte, par_salarie: &[(String, Vec<UnAvenant>)], client: &mut dyn Courriel) -> R<Avenants> {
    let (signataire, raison) = signataire_et_raison(ctx)?;
    let expediteur = client.compte().to_lowercase();
    let aujourdhui_jj = ctx.aujourdhui.jj_mm_aaaa();
    // La saisie de chaque mois, lue une fois : rien ne l'ecrit pendant l'envoi.
    let mut saisies: HashMap<String, (Value, Vec<Paie>)> = HashMap::new();
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let (mut partis, mut lignes, mut faits): (Vec<String>, Vec<Value>, Vec<Value>) = (Vec::new(), Vec::new(), Vec::new());
    for (salarie, aa) in par_salarie {
        let mois = aa[0].texte("mois");
        let (an, nom_mois) = an_et_nom_du_mois(&mois)?;
        let mut pdfs: Vec<PathBuf> = Vec::new();
        let mut lignes_contrats: Vec<Value> = Vec::new();
        let mut sal: Option<Value> = None;
        for a in aa {
            let (pdf, mut c, s) = avenant_ecrit(ctx, salarie, a, &mut saisies, &departs, &aujourdhui_jj)?;
            sal = Some(s);
            c.insert("adresse".into(), json!(a.texte("destinataire").trim().to_lowercase()));
            c.insert("relance".into(), json!(a.relance()));
            lignes_contrats.push(Value::Object(c));
            pdfs.push(pdf);
        }
        let sal = sal.unwrap_or(Value::Null);
        let adresse = lignes_contrats[0]["adresse"].as_str().unwrap_or("").to_string();
        let code = code_signature();
        let mut spectacles: Vec<String> = Vec::new();
        for a in aa {
            let t = a.texte("spectacle");
            if !spectacles.contains(&t) {
                spectacles.push(t);
            }
        }
        let un = |a: &UnAvenant| -> R<String> {
            let articles = et_fr(
                &a.modifications().iter().map(|m| format!("l'article {} « {} »", valeur_en_texte(&m["numero"]), valeur_en_texte(&m["article"]))).collect::<Vec<_>>(),
            );
            Ok(format!(
                "l'avenant n° {} à ton {} n° {}{}, {}, qui modifie {}",
                a.numero(),
                a.texte("nom_du_contrat"),
                a.contrat(),
                a.texte("pour"),
                a.texte("periode"),
                articles
            ))
        };
        let sujet_de = |spectacle: &str| envoi::formater(SUJET_SIGNATURE, &[("spectacle", spectacle.to_string()), ("mois", nom_mois.to_string()), ("annee", an.to_string())]);
        let (mut sujet, mut liste, quoi);
        if aa.len() == 1 {
            let a = &aa[0];
            sujet = if a.texte("pour").is_empty() {
                format!("Avenant n° {} - {} - {} {}", a.numero(), capitaliser(&a.texte("nom_du_contrat")), nom_mois, an)
            } else {
                format!("Avenant n° {} - {}", a.numero(), sujet_de(&a.texte("spectacle")))
            };
            liste = format!("Tu trouveras ci-joint {} ; le reste du contrat ne change pas.", un(a)?);
            quoi = format!("l'avenant n° {} au {} n° {}{}", a.numero(), a.texte("nom_du_contrat"), a.contrat(), a.texte("pour"));
        } else {
            sujet = format!(
                "Avenants - {}",
                if spectacles.len() == 1 {
                    sujet_de(&spectacles[0])
                } else {
                    envoi::formater(SUJET_SIGNATURE_PLURIEL, &[("mois", nom_mois.to_string()), ("annee", an.to_string())])
                }
            );
            let mut morceaux = Vec::new();
            for a in aa {
                morceaux.push(format!("  - {} ;", un(a)?));
            }
            liste = format!("Tu trouveras ci-joint {} avenants à tes contrats, un document chacun :\n{}\nLe reste de chaque contrat ne change pas.", aa.len(), morceaux.join("\n"));
            quoi = format!(
                "les avenants suivants : {}",
                aa.iter().map(|a| format!("n° {} au {} n° {}{}", a.numero(), a.texte("nom_du_contrat"), a.contrat(), a.texte("pour"))).collect::<Vec<_>>().join(" ; ")
            );
        }
        if aa.iter().any(UnAvenant::relance) {
            sujet = format!("{}{}", PREFIXE_RELANCE, sujet);
            liste = format!("{}{}", RAPPEL_RELANCE_AVENANT, liste);
        }
        let message = message_de_signature(
            &Demande {
                cle: salarie,
                sal: &sal,
                adresse: &adresse,
                sujet: &sujet,
                liste: &liste,
                quoi: &quoi,
                le_signer: if aa.len() == 1 { "le signer" } else { "les signer" },
                exemplaire: if aa.len() == 1 { "ton avenant" } else { "tes avenants" },
                avant_travail: "",
                code: &code,
                expediteur: &expediteur,
                signataire: &signataire,
                raison: &raison,
            },
            pdfs.clone(),
        )?;
        for c in lignes_contrats.iter_mut() {
            if let Value::Object(m) = c {
                m.insert("code".into(), json!(code));
            }
        }
        let versions = envois::journal::versions_des_contrats(ctx, &lignes_contrats, &departs).map_err(Echec::from)?;
        let mut identifiants: Vec<String> = Vec::new();
        let partis_ici = match envoi::envoyer_suivi(std::slice::from_ref(&message), &expediteur, client, Some(&mut identifiants), None) {
            Ok(p) => p,
            Err(e) => {
                let servis = versions_des_servis(&versions, std::slice::from_ref(&message), e.partis.len());
                noter_echec_d_envoi(ctx, "contrat-signature", Some(&mois), "interface", &e.partis, &e.echoue, &e.message, Some(json!({"contrats": lignes_contrats})), Some(&servis))?;
                return Err(echec_d_envoi(e.message));
            }
        };
        for c in lignes_contrats.iter_mut() {
            if let Value::Object(m) = c {
                m.insert("message".into(), json!(identifiants.first().cloned().unwrap_or_default()));
            }
        }
        let reponse = format!(
            "{} envoye(s) a la signature",
            aa.iter().map(|a| format!("avenant n° {} au contrat {}", a.numero(), valeur_en_texte(&a.apercu["contrat"]))).collect::<Vec<_>>().join("; ")
        );
        let detail = json!({"contrats": lignes_contrats, "expediteur": expediteur, "relance": aa.iter().any(UnAvenant::relance), "acte": "avenant"});
        lignes.push(noter_envoi(ctx, "contrat-signature", Some(&mois), "interface", true, &partis_ici.join(", "), &reponse, Some(&detail), Some(&versions))?);
        partis.extend(partis_ici);
        for (a, pdf) in aa.iter().zip(&pdfs) {
            faits.push(json!({"salarie": salarie, "debut": a.texte("debut"), "numero": a.apercu["numero"], "fichier": nom_du(pdf)}));
        }
    }
    Ok(Avenants { partis, avenants: faits, lignes })
}
