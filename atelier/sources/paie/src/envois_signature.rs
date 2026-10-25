//! `demander-signature`, `avenant-envoyer` et `avenants-envoyer` (`moteur/operations_documents.py` :
//! `demander_signature`, `envoyer_avenant`, `envoyer_avenants`, `_verifier_relance`, `_clauses_du_contrat`,
//! `contrat_parti`, `code_signature`, `_mailto` ; `operations_paies.reserver_numeros_envoyes`). Lot H3b.
//!
//! ECRIT, NON PROUVE, NON BRANCHE. Un courriel par salarie, un PDF par courriel (le contrat, ou les avenants), le
//! bouton « Je signe » (un lien `mailto:` vers la boite de la compagnie, objet et corps deja ecrits) et un CODE
//! UNIQUE (`SIG-XXXXXX`) par courriel, garde au journal avec la demande : c'est par lui que la releve des signatures
//! (lots H4 et I) reconnaitra la reponse. Les messages sont ceux que `signature-apercu` et `avenant-apercu` ont
//! montres : les paies (`apercu_dsn::paies_aux_numeros_declares`), les articles modifies (`avenant-apercu` lui-meme,
//! appele ici), les etats (`signature-apercu`, pour la relance).
//!
//! Tout se verifie avant que rien ne parte, puis la preuve (le mot de passe du logiciel) ; les PDF s'ecrivent APRES
//! elle (le Python creait `sorties/<mois>/envoi/` avant, un dossier vide : ecart, rien n'est ecrit sur un refus).
//!
//! LE FILET EN FOND (`ops.preparer_filet_en_fond`) N'EST PAS ICI : `web.py` le lancera apres l'appel Rust.

use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::{json, Map, Value};

use engagement::calcul::Paie;
use droit::contexte::Contexte;
use crate::depot_acte::verifier_preuve;
use crate::envoi::{self, Message};
use socle::valeur::valeur_en_texte;
use socle::echec::erreur;
use socle::fichiers::{repr, vrai};
use socle::echec::Echec;
use crate::routes_envois::{
    capitaliser, charge_texte, echec_d_envoi, html_echapper, jour_iso, mailto, noter, noter_echec_d_envoi, nom_du, preuve_de, refus, requis, vrai_ou, Ouvrir, R,
};
use crate::routes_mois::{Moteur, Paire, SaisieMois};

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

// ------------------------------------------------------------------ les textes (ceux de `operations_documents.py`)

const SUJET_SIGNATURE: &str = "Contrat d'engagement {spectacle} - {mois} {annee}";
const SUJET_SIGNATURE_PLURIEL: &str = "Contrats d'engagement - {mois} {annee}";
const PREFIXE_RELANCE: &str = "Rappel - ";
const RAPPEL_RELANCE: &str = "Petit rappel : ta signature n'est pas encore arrivée. C'est le même contrat que la dernière fois.\n\n";
const ALPHABET_CODE: &str = "ABCDEFGHJKMNPQRSTUVWXYZ23456789";
const CORPS_SIGNATURE: &str = "Bonjour {prenom},\n\n{contrats}\n\nPour {le_signer}, un clic suffit : appuie sur le bouton « Je signe » de ce message, puis envoie la réponse qui s'ouvre, sans la modifier. Si ta messagerie n'affiche pas le bouton, réponds simplement à ce message en écrivant « Je signe » suivi de ce code : {code}\n\nTu recevras aussitôt {exemplaire} signé des deux parties, à conserver.{avant_travail}\n\nSi quelque chose ne correspond pas à ce qui était convenu, dis-le avant de signer plutôt qu'après : c'est plus simple à corriger.\n\nBien à toi,\n{signataire}\n{raison_sociale}\n";
const HTML_SIGNATURE: &str = "<div style=\"font-family:Arial,Helvetica,sans-serif;font-size:15px;line-height:1.5;color:#2C3E50\">\n<p>Bonjour {prenom},</p>\n<p>{contrats}</p>\n<p>Pour {le_signer}, un clic suffit : appuie sur le bouton, puis envoie la réponse qui s'ouvre, sans la modifier.</p>\n<p style=\"margin:26px 0\">\n<a href=\"{mailto}\" style=\"display:inline-block;background:#B22222;color:#ffffff;padding:14px 26px;border-radius:6px;text-decoration:none;font-weight:bold;font-size:16px\">Je signe</a>\n</p>\n<p style=\"font-size:13px;color:#555\">Si le bouton ne fait rien, réponds simplement à ce message en écrivant « Je signe » suivi de ce code : <b>{code}</b></p>\n<p>Tu recevras aussitôt {exemplaire} signé des deux parties, à conserver.{avant_travail}</p>\n<p>Si quelque chose ne correspond pas à ce qui était convenu, dis-le avant de signer plutôt qu'après : c'est plus simple à corriger.</p>\n<p>Bien à toi,<br>{signataire}<br>{raison_sociale}</p>\n</div>\n";
pub(crate) const CORPS_REPONSE: &str = "Je soussigné(e) {nom} déclare avoir lu et accepter {quoi}.\n\nCode de signature : {code}\n";

// ------------------------------------------------------------------ les petits outils

/// Un contrat vise par la page : (salarie, debut, numero facultatif).
type Cible = (String, String, Option<String>);

/// `jj_mm_aaaa(x)` : `ValueError` si `x` ne se lit pas comme une date.
fn jj(iso: &str) -> R<String> {
    socle::date::Jour::lire(iso).map(|j| j.jj_mm_aaaa()).map_err(|m| erreur("ValueError", m))
}

/// `jj_mm_aaaa(p.engagement.fin)`.
fn fin_de(p: &Paie) -> R<String> {
    p.engagement.fin.map(|f| f.jj_mm_aaaa()).ok_or_else(|| erreur("ValueError", "contrat sans fin"))
}

fn cle_de(p: &Paie) -> String {
    p.salarie.get("_cle").map(valeur_en_texte).unwrap_or_default()
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
pub(crate) fn code_signature() -> String {
    code_au_hasard("SIG-")
}

/// Un code a six signes derriere son prefixe (`SIG-` d'une signature, `FIC-` d'une fiche demandee a la personne).
pub(crate) fn code_au_hasard(prefixe: &str) -> String {
    let alphabet = ALPHABET_CODE.as_bytes();
    let limite = 256 - 256 % alphabet.len();
    let mut code = String::from(prefixe);
    while code.len() < prefixe.len() + 6 {
        let mut b = [0u8; 1];
        if socle::temps::aleas(&mut b).is_err() {
            let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(7);
            b[0] = (t >> (code.len() % 3)) as u8;
        }
        if (b[0] as usize) < limite {
            code.push(alphabet[b[0] as usize % alphabet.len()] as char);
        }
    }
    code
}

/// `signataire` et `raison` des courriels : le prenom d'affichage du signataire (sinon son prenom, capitalise), et
/// le nom d'affichage de la compagnie (sinon sa raison sociale).
pub(crate) fn signataire_et_raison(ctx: &Contexte) -> R<(String, String)> {
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

/// `sal.get("prenom_affichage") or sal["prenom"]`.
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

/// `_clauses_du_contrat` : ce que le contrat dit, clause par clause (l'avenant qui le modifiera doit citer l'ancien
/// texte a cote du nouveau). `{}` si le contrat ne s'ecrit pas.
fn clauses_du_contrat(ctx: &Contexte, p: &Paie, ent: &Value) -> Value {
    match crate::contrat::valeurs_de_paie(ctx, ent, p) {
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

/// Les elements d'une liste de la charge, ou rien : `charge.get(cle)` quand il est vrai.
fn liste_de<'a>(charge: &'a Value, cle: &str) -> R<Option<&'a Vec<Value>>> {
    match charge.get(cle).filter(|v| vrai(v)) {
        None => Ok(None),
        Some(Value::Array(a)) => Ok(Some(a)),
        Some(autre) => Err(erreur("TypeError", format!("`{}` : une liste est attendue, pas {}", cle, socle::valeur::nom_type(autre)))),
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

// ------------------------------------------------------------------ demander-signature

/// `operations_documents.demander_signature` : envoie a chaque salarie son contrat, avec le bouton « Je signe ».
///
/// `remplace` : le contrat refait EN ENTIER, qui annule et remplace celui qui est deja parti -- une phrase en tete
/// le dit. Refuse pour un contrat jamais parti, ou dont l'execution a commence : celui-la se modifie par un
/// avenant. `relance` : redemander un contrat deja demande et pas revenu, TEL QUEL -- objet « Rappel », une ligne
/// de rappel, et le journal le dit. Refuse pour un contrat jamais demande, deja signe ou caduc.
///
/// LA SELECTION EST PAR CONTRAT, pas par salarie (`contrats`). `salaries` reste accepte (tous les contrats de ces
/// salaries). UN COURRIEL PAR SALARIE, UN PDF PAR COURRIEL, tous ses contrats dedans (une page par contrat), ecrit
/// dans `sorties/<mois>/envoi/` avec les MEMES numeros que le fichier groupe.
pub(crate) fn demander_signature(o: &HashMap<String, String>, ctx: &Contexte, charge: &Value, ouvrir: Ouvrir) -> R<Value> {
    let mois = charge_texte(charge, "mois")?;
    let relance = charge.get("relance").is_some_and(vrai);
    let remplace = charge.get("remplace").is_some_and(vrai);
    // Les numeros que le compteur PROPOSE, en memoire seulement : ils servent aux contrats qui partent. Rien n'en
    // est ecrit (le Python ecrivait tout, bulletin compris : defaut non reproduit, decision de Leandre du 07/10/2026) ;
    // `reserver_numeros_envoyes` n'avance que les compteurs de contrat, jusqu'au numero reellement envoye.
    let mut reserves = crate::produire_mois::Reserves::default();
    let (data, _rep, paies) = crate::apercu_dsn::paies_aux_numeros_declares_avec(o, ctx, &mois, Some(&mut reserves), false)?;
    let s_mois = data.get("mois").map(socle::date::en_texte).ok_or_else(|| erreur("KeyError", socle::fichiers::absent("mois")))?;
    let dossier = ctx.racine.join("sorties").join(&s_mois).join("envoi");
    let etiq = envoi::etiquette(&s_mois).map_err(|m| erreur("ValueError", m))?;

    let voulus: Option<Vec<String>> = liste_de(charge, "salaries")?.map(|l| l.iter().map(valeur_en_texte).collect());
    let mut client = ouvrir()?;
    let a_qui = match &voulus {
        Some(l) if !l.is_empty() => format!("{} salarie(s) : {}", l.len(), l.join(", ")),
        _ => "les salaries du mois qui ont un contrat a signer".to_string(),
    };
    let quoi = if relance { "relance de demande de signature de contrat" } else if remplace { "contrat refait, a signer (il annule et remplace le precedent)" } else { "demande de signature de contrat" };
    verifier_preuve(ctx, Some(&preuve_de(charge)), &mois, quoi, &a_qui)?;

    let (an, nom_mois) = an_et_nom_du_mois(&s_mois)?;
    let ent = crate::avenant::ent_du_mois(ctx, &s_mois)?;
    // Retours Windows, B : deux contrats du meme salarie qui commencent le meme jour se distinguent par leur NUMERO
    // (`numero`, que `signature-apercu` rend et que la page renvoie) : on peut n'en envoyer qu'un. Sans `numero`, la
    // selection vaut pour tous les contrats de ce salarie a ce debut (l'ancien comportement).
    let cibles: Option<Vec<Cible>> = match liste_de(charge, "contrats")? {
        None => None,
        Some(l) => {
            let mut v = Vec::new();
            for c in l {
                let numero = c.get("numero").filter(|n| vrai(n)).map(valeur_en_texte);
                v.push((valeur_en_texte(requis(c, "salarie")?), valeur_en_texte(requis(c, "debut")?), numero));
            }
            Some(v)
        }
    };
    let vise = |c: &Cible, cle: &str, p: &Paie| c.0 == cle && c.1 == p.engagement.debut.iso() && c.2.as_ref().map_or(true, |n| p.engagement.numero.as_deref() == Some(n.as_str()));
    // Par salarie, dans l'ordre du mois ; le rang de chaque contrat dans la saisie suit sa paie.
    let mut par_salarie: Vec<(String, Vec<Paie>, Vec<usize>)> = Vec::new();
    for (rang, p) in paies.iter().enumerate() {
        let cle = cle_de(p);
        if !vrai(&p.spectacle) {
            continue; // un permanent : voir `signature-apercu`
        }
        if let Some(v) = voulus.as_ref().filter(|v| !v.is_empty()) {
            if !v.contains(&cle) {
                continue;
            }
        }
        if let Some(c) = &cibles {
            if !c.iter().any(|x| vise(x, &cle, p)) {
                continue;
            }
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

    // Le journal, lu une fois : chaque paie y retrouve SON contrat (salarie, debut, numero), pas celui de son jumeau du meme jour.
    let lignes_j = crate::etats::charger(&ctx.racine)?;
    let tous: Vec<(String, String, String)> = paies.iter().map(|p| (cle_de(p), p.engagement.debut.iso(), p.engagement.numero.clone().unwrap_or_default())).collect();
    let cles_j = crate::etats::cles_des_paies(&lignes_j, &tous)?;
    if relance {
        let cles: Vec<(String, String, String)> = par_salarie.iter().flat_map(|(_, ps, rs)| rs.iter().zip(ps).map(|(r, _)| tous[*r].clone())).collect();
        verifier_relance(o, ctx, &mois, &cles)?;
    }
    let aujourdhui = ctx.aujourdhui.iso();
    let mut remplacements: Vec<((String, String, String), (String, String))> = Vec::new();
    if remplace {
        if relance {
            return Err(refus("Une relance renvoie le même contrat : elle n'en remplace aucun."));
        }
        let lignes = crate::journal::lire(&ctx.racine, None, None, false).map_err(Echec::from)?;
        for (k, ps, rs) in &par_salarie {
            for (p, r) in ps.iter().zip(rs) {
                let debut = p.engagement.debut.iso();
                // Le contrat qui part est remplace, lui : le jumeau du meme jour qui n'est pas parti n'a rien a remplacer.
                let Some(parti) = crate::avenant::contrat_parti(&lignes, k, &debut, Some(cles_j[*r][0].2.as_str()))? else {
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

    // L'engagement tel qu'il part : si la paie change ensuite, l'ecran dira que ce contrat n'est plus le bon.
    // Une empreinte PAR CONTRAT : deux contrats du meme salarie au meme debut ne partagent pas la leur (le rang les separe).
    let rangs: Vec<Paire> = paies.iter().enumerate().map(|(i, p)| (cle_de(p), format!("{}#{}", p.engagement.debut.iso(), i))).collect();
    let empreintes = Moteur::empreintes_engagements(&SaisieMois { data: data.clone(), paies: Vec::new() }, &rangs);
    let (signataire, raison) = signataire_et_raison(ctx)?;
    let expediteur = client.compte().to_lowercase();
    let mut messages: Vec<Message> = Vec::new();
    let mut contrats: Vec<Value> = Vec::new();
    let mut rangs_envoyes: Vec<usize> = Vec::new();
    // Retours Windows, B : TOUTES les adresses se verifient avant d'ecrire le moindre PDF (un refus ne laisse rien sur le disque).
    for (cle, ps, _) in &par_salarie {
        if vrai_ou(&ps[0].salarie, "email").unwrap_or_default().trim().is_empty() {
            return Err(refus(format!("{} n'a pas de courriel dans sa fiche : son contrat ne peut pas partir.", cle)));
        }
    }
    for (cle, ps, rs) in &par_salarie {
        let sal = &ps[0].salarie;
        let adresse = vrai_ou(sal, "email").unwrap_or_default().trim().to_lowercase();
        let code = code_signature();
        let numeros: Vec<String> = ps.iter().map(|p| p.engagement.numero.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| "sans-numero".to_string())).collect();
        // Le contrat qui en remplace un autre porte son propre nom : le PDF parti avant lui reste sur le disque.
        let pdf = dossier.join(format!(
            "Contrat{}_{}_{}_{}{}.pdf",
            if ps.len() > 1 { "s" } else { "" },
            cle,
            numeros.join("+"),
            etiq,
            if remplace { "_remplace" } else { "" }
        ));
        crate::contrat::ecrire_et_garder(ctx, ps, &ent, &pdf, &remplacements)?;
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
        let du_au = |p: &Paie, titre: &str| -> R<String> { Ok(format!("{}, du {} au {}", titre, p.engagement.debut.jj_mm_aaaa(), fin_de(p)?)) };
        let numero_de = |p: &Paie| p.engagement.numero.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| "(sans numéro)".to_string());
        let (mut sujet, mut liste, le_signer, quoi, exemplaire);
        if ps.len() == 1 {
            let p = &ps[0];
            sujet = sujet_de(&titres[0]);
            liste = format!("Tu trouveras ci-joint ton contrat d'engagement pour {}, du {} au {}.", titres[0], p.engagement.debut.jj_mm_aaaa(), fin_de(p)?);
            le_signer = "le signer";
            quoi = format!("le contrat d'engagement n° {} pour {}, du {} au {}", numero_de(p), titres[0], p.engagement.debut.jj_mm_aaaa(), fin_de(p)?);
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
                morceaux.push(format!("n° {} pour {}, du {} au {}", numero_de(p), t, p.engagement.debut.jj_mm_aaaa(), fin_de(p)?));
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
        for ((p, t), rang) in ps.iter().zip(&titres).zip(rs) {
            let debut = p.engagement.debut.iso();
            rangs_envoyes.push(*rang);
            let mut c = Map::new();
            c.insert("salarie".into(), json!(cle));
            c.insert("debut".into(), json!(debut));
            c.insert("fin".into(), p.engagement.fin.map_or(Value::Null, |f| json!(f.iso())));
            c.insert("numero".into(), p.engagement.numero.clone().map_or(Value::Null, Value::String));
            c.insert("fichier".into(), json!(nom_du(&pdf)));
            c.insert("spectacle".into(), json!(t));
            c.insert("mois".into(), json!(s_mois));
            c.insert("empreinte".into(), empreintes.get(&(cle.clone(), format!("{}#{}", debut, rang))).map_or(Value::Null, |e| json!(e)));
            // CE QUE LE CONTRAT DIT, clause par clause : l'avenant qui le modifiera doit citer l'ancien texte.
            c.insert("clauses".into(), clauses_du_contrat(ctx, p, &ent));
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
    let mut identifiants: Vec<String> = Vec::new();
    let partis = match envoi::envoyer_suivi(&messages, &expediteur, client.as_mut(), Some(&mut identifiants), None) {
        Ok(p) => p,
        Err(e) => {
            noter_echec_d_envoi(ctx, "contrat-signature", Some(&mois), "interface", &e, Some(json!({"contrats": contrats})))?;
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
    // Le numero parti est celui que la DSN declarera : il s'ecrit dans la saisie et le compteur passe au-dessus.
    reserver_numeros_envoyes(o, ctx, &s_mois, &contrats, &rangs_envoyes, "interface")?;
    let detail = json!({"contrats": contrats, "expediteur": expediteur, "relance": relance});
    let ligne = noter(
        ctx,
        "contrat-signature",
        Some(&mois),
        "interface",
        true,
        &partis.join(", "),
        &format!("{} contrat(s) {} a la signature", contrats.len(), if relance { "relance(s)" } else { "envoye(s)" }),
        Some(&detail),
    )?;
    Ok(json!({"partis": partis, "contrats": contrats, "expediteur": expediteur, "ligne": ligne}))
}

/// L'annee et le nom du mois d'un « AAAA-MM », pour les sujets des courriels (« octobre 2026 ») ; un mois illisible ou
/// hors 01-12 se refuse (`date::nom_du_mois`).
pub(crate) fn an_et_nom_du_mois(mois: &str) -> R<(i32, &'static str)> {
    let lire = |t: Option<&str>| -> R<i64> {
        let t = t.unwrap_or("");
        t.trim().parse::<i64>().map_err(|_| erreur("ValueError", format!("nombre entier illisible : {}", repr(t))))
    };
    let an = lire(mois.get(..4))?;
    let nom = socle::date::nom_du_mois(lire(mois.get(5..7))?, socle::date::Graphie::Accentuee).map_err(|e| erreur("IndexError", e))?;
    Ok((an as i32, nom))
}

/// `_verifier_relance` : une relance ne vaut que pour un contrat demande, pas revenu, a jour.
fn verifier_relance(o: &HashMap<String, String>, ctx: &Contexte, mois: &str, cles: &[(String, String, String)]) -> R<()> {
    let apercu = crate::routes_documents::signature_apercu_avec(o, ctx, &json!({"mois": mois}), false)?;
    let etats: Vec<&Value> = apercu.get("contrats").and_then(Value::as_array).map(|l| l.iter().collect()).unwrap_or_default();
    for (salarie, debut, numero) in cles {
        // Le contrat, pas son jumeau du meme jour : (salarie, debut, numero), comme l'ecran.
        let c = etats.iter().rev().find(|c| {
            c["salarie"].as_str() == Some(salarie)
                && c["debut"].as_str() == Some(debut)
                && valeur_en_texte(&crate::etats::numero_de_cle(&c["numero"])) == *numero
        });
        let pb = match c {
            None => "n'a jamais ete demande : il s'envoie",
            Some(c) if !vrai(&c["demande_le"]) => "n'a jamais ete demande : il s'envoie",
            Some(c) if vrai(&c["caduc"]) => "a change depuis son envoi : il se renvoie a jour",
            Some(c) if vrai(&c["recu_le"]) => "est deja revenu signe",
            Some(_) => continue,
        };
        return Err(refus(format!("Le contrat de {} du {} {}, il ne se relance pas. Rien n'est parti.", salarie, debut, pb)));
    }
    Ok(())
}

// ------------------------------------------------------------------ reserver_numeros_envoyes

/// `operations_paies.reserver_numeros_envoyes` : UN CONTRAT PARTI A LA SIGNATURE GARDE SON NUMERO.
///
/// Avant l'ecriture du mois, un contrat part sous le numero que le compteur PROPOSE, sans le reserver. Le numero
/// s'ecrit dans la saisie des l'envoi (`numeroter` garde un numero deja porte) et le compteur passe au-dessus.
/// `contrats` : [{"salarie", "debut", "numero"}]. Sous le verrou des compteurs.
pub(crate) fn reserver_numeros_envoyes(
    o: &HashMap<String, String>,
    ctx: &Contexte,
    mois: &str,
    contrats: &[Value],
    rangs_envoyes: &[usize],
    qui: &str,
) -> R<()> {
    use crate::produire_mois::{calculer, charger};
    let chemin = ctx.racine.join("paies").join(format!("{}.json", mois));
    if !chemin.exists() {
        return Ok(());
    }
    let _verrou = crate::ecrire_documents::prendre_le_verrou(ctx, qui)?;
    // `_paies_par_rang` : les donnees brutes, et (cle du salarie, premier jour) de chaque paie, dans l'ordre du fichier.
    // Les compteurs ne bougent que par ce qui est ecrit sur un contrat parti : `reserves` ne recoit que
    // `enregistrer` (le Python reservait aussi ce que `charger` propose, bulletin compris : defaut non reproduit).
    let mut reserves = crate::produire_mois::Reserves::default();
    let mut data = engagement::lire_saisie(&chemin).map_err(|m| erreur("ValueError", m))?;
    let rep = calculer(o, ctx, &data)?;
    let paies = charger(ctx, &data, &rep, None)?;
    let rangs: Vec<Paire> = paies.iter().map(|p| (cle_de(p), p.engagement.debut.iso())).collect();
    // Par RANG dans la saisie, pas par (salarie, debut) : deux contrats du meme jour ne se confondent pas, et celui qui n'est
    // pas parti ne recoit pas le numero de l'autre.
    let mut voulus: HashMap<usize, Value> = HashMap::new();
    for (c, rang) in contrats.iter().zip(rangs_envoyes) {
        voulus.insert(*rang, c.get("numero").cloned().unwrap_or(Value::Null));
    }
    let mut change = false;
    if let Some(Value::Array(eng)) = data.get_mut("engagements") {
        for (i, cle) in rangs.iter().enumerate() {
            let Some(n) = voulus.get(&i).filter(|n| vrai(n)) else { continue };
            if i >= eng.len() || eng[i].get("numero").is_some_and(vrai) {
                continue;
            }
            if let Value::Object(e) = &mut eng[i] {
                e.insert("numero".into(), n.clone());
            }
            change = true;
            if let Some((annee, seq)) = annee_et_rang(&valeur_en_texte(n)) {
                let compteur = format!("contrat_{}", cle.0);
                if reserves.prochain(ctx, &compteur, annee)? <= seq {
                    reserves.enregistrer(annee, &compteur, seq);
                }
            }
        }
    }
    if change {
        reserves.valider(ctx)?;
        crate::routes_contrats::sauver_avant(&chemin).map_err(Echec::from)?;
        socle::ecriture::ecrire_json(&chemin, &data, 2).map_err(|m| erreur("OSError", m))?;
    }
    Ok(())
}

/// `re.search(r"\.(\d{2})\.(\d+)$", n)` : l'annee (2000 + les deux chiffres) et le rang d'un numero `FA1.26.16`.
fn annee_et_rang(n: &str) -> Option<(i64, i64)> {
    let (tete, rang) = n.rsplit_once('.')?;
    if rang.is_empty() || !rang.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let t: Vec<char> = tete.chars().collect();
    if t.len() < 3 || t[t.len() - 3] != '.' || !t[t.len() - 2].is_ascii_digit() || !t[t.len() - 1].is_ascii_digit() {
        return None;
    }
    let yy: i64 = t[t.len() - 2..].iter().collect::<String>().parse().ok()?;
    Some((2000 + yy, rang.parse().ok()?))
}

// ------------------------------------------------------------------ les avenants

/// Ce que `avenant-apercu` dit d'un avenant a envoyer, et la paie du contrat.
struct UnAvenant {
    apercu: Value,
    effet: String,
}

impl UnAvenant {
    fn texte(&self, cle: &str) -> String {
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
}

/// `operations_documents.envoyer_avenant` : un seul avenant. La reponse de la route : `partis`, `fichier`,
/// `numero`, `ligne`.
pub(crate) fn avenant_envoyer(o: &HashMap<String, String>, ctx: &Contexte, charge: &Value, ouvrir: Ouvrir) -> R<Value> {
    let voulu = json!({
        "salarie": requis(charge, "salarie")?, "debut": requis(charge, "debut")?, "effet": charge.get("effet").cloned().unwrap_or(Value::Null),
        "numero": charge.get("numero").cloned().unwrap_or(Value::Null), "mois": charge.get("mois").cloned().unwrap_or(Value::Null),
    });
    let r = envoyer_avenants(o, ctx, &[voulu], &preuve_de(charge), ouvrir)?;
    let a = r.avenants.first().ok_or_else(|| erreur("ValueError", "aucun avenant n'est parti"))?;
    Ok(json!({"partis": r.partis, "fichier": a["fichier"], "numero": a["numero"], "ligne": r.lignes.first().cloned().unwrap_or(Value::Null)}))
}

/// La route `avenants-envoyer` : les avenants coches, UN COURRIEL PAR SALARIE. La reponse : `partis`, `avenants`.
pub(crate) fn avenants_envoyer(o: &HashMap<String, String>, ctx: &Contexte, charge: &Value, ouvrir: Ouvrir) -> R<Value> {
    let voulus: Vec<Value> = match requis(charge, "avenants")? {
        Value::Array(a) => a.clone(),
        Value::Null => Vec::new(),
        autre => return Err(erreur("TypeError", format!("`avenants` : une liste est attendue, pas {}", socle::valeur::nom_type(autre)))),
    };
    let r = envoyer_avenants(o, ctx, &voulus, &preuve_de(charge), ouvrir)?;
    Ok(json!({"partis": r.partis, "avenants": r.avenants}))
}

/// Ce que `envoyer_avenants` rend.
struct Avenants {
    partis: Vec<String>,
    avenants: Vec<Value>,
    lignes: Vec<Value>,
}

/// `operations_documents.envoyer_avenants` : ecrit les avenants et les envoie a la signature, avec le bouton
/// « Je signe » -- le meme circuit que le contrat. UN COURRIEL PAR SALARIE : ses avenants en pieces jointes -- un
/// PDF chacun, qui garde son numero -- et un seul code, dont la reponse vaut pour tous. Tout se verifie avant que
/// rien ne parte : un avenant sans objet ou un salarie sans courriel refuse l'envoi entier.
///
/// Le journal `contrat-signature` porte une ligne par courriel, `acte: "avenant"` et le numero de chacun ;
/// l'empreinte de la paie d'aujourd'hui y est notee, et c'est elle qui dit ensuite que le contrat est a jour.
fn envoyer_avenants(o: &HashMap<String, String>, ctx: &Contexte, voulus: &[Value], preuve: &crate::depot_acte::Preuve, ouvrir: Ouvrir) -> R<Avenants> {
    if voulus.is_empty() {
        return Err(refus("Aucun avenant coché. Rien n'est parti."));
    }
    let mut par_salarie: Vec<(String, Vec<UnAvenant>)> = Vec::new();
    for v in voulus {
        let salarie_v = requis(v, "salarie")?;
        let debut_v = requis(v, "debut")?;
        // `numero` (celui que `signature-apercu` rend) et `mois` (celui de l'ecran) disent QUEL contrat du jour : facultatifs.
        let apercu = crate::routes_documents::avenant_apercu_avec(
            o,
            ctx,
            &json!({"salarie": salarie_v, "debut": debut_v, "numero": v.get("numero").cloned().unwrap_or(Value::Null), "mois": v.get("mois").cloned().unwrap_or(Value::Null)}),
            false,
        )?;
        let modifications = apercu["modifications"].as_array().map_or(0, Vec::len);
        let nom = apercu.get("nom").map(valeur_en_texte).unwrap_or_default();
        if modifications == 0 {
            return Err(refus(format!(
                "{}, contrat du {} : le contrat parti dit déjà ce que dit la paie, pas d'avenant à faire. Rien n'est parti.",
                nom,
                jj(&valeur_en_texte(debut_v))?
            )));
        }
        let effet = vrai_ou(v, "effet").unwrap_or_else(|| apercu.get("effet").map(valeur_en_texte).unwrap_or_default());
        if jour_iso(&effet).is_none() {
            return Err(refus(format!("Date d'effet illisible : {}.", repr(&effet))));
        }
        if apercu.get("destinataire").map(valeur_en_texte).unwrap_or_default().trim().is_empty() {
            return Err(refus(format!("{} n'a pas de courriel dans sa fiche : l'avenant ne peut pas partir.", valeur_en_texte(salarie_v))));
        }
        let cle = valeur_en_texte(salarie_v);
        let un = UnAvenant { apercu, effet };
        match par_salarie.iter_mut().find(|(k, _)| *k == cle) {
            Some((_, l)) => l.push(un),
            None => par_salarie.push((cle, vec![un])),
        }
    }
    let mut client = ouvrir()?;
    let mut mois_vus: Vec<String> = par_salarie.iter().flat_map(|(_, aa)| aa.iter().map(|a| a.texte("mois"))).collect();
    mois_vus.sort();
    let a_qui = format!("{} salarie(s) : {}", par_salarie.len(), par_salarie.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>().join(", "));
    verifier_preuve(ctx, Some(preuve), mois_vus.first().map_or("", String::as_str), "envoi d'avenant(s) a signer", &a_qui)?;

    let (signataire, raison) = signataire_et_raison(ctx)?;
    let expediteur = client.compte().to_lowercase();
    let aujourdhui_jj = ctx.aujourdhui.jj_mm_aaaa();
    // La saisie de chaque mois, lue une fois : rien ne l'ecrit pendant l'envoi.
    let mut saisies: HashMap<String, (Value, Vec<Paie>)> = HashMap::new();
    let lignes_j = crate::etats::charger(&ctx.racine)?;
    let (mut partis, mut lignes, mut faits): (Vec<String>, Vec<Value>, Vec<Value>) = (Vec::new(), Vec::new(), Vec::new());
    for (salarie, aa) in &par_salarie {
        let mois = aa[0].texte("mois");
        let (an, nom_mois) = an_et_nom_du_mois(&mois)?;
        let mut pdfs: Vec<PathBuf> = Vec::new();
        let mut lignes_contrats: Vec<Value> = Vec::new();
        let mut sal: Option<Value> = None;
        for a in aa {
            let mois_a = a.texte("mois");
            if !saisies.contains_key(&mois_a) {
                let (d, _rep, p) = crate::apercu_dsn::paies_aux_numeros_declares_avec(o, ctx, &mois_a, None, false)?;
                saisies.insert(mois_a.clone(), (d, p));
            }
            let (data, paies) = &saisies[&mois_a];
            // `_paie_du_contrat`.
            let debut = a.texte("debut");
            let numero_j = valeur_en_texte(&crate::etats::numero_de_cle(&a.apercu["contrat"]));
            let Some(rang_p) = crate::routes_documents::paie_du_contrat(paies, &lignes_j, salarie, &debut, &numero_j, false)? else {
                return Err(refus(format!("Aucun contrat de {} commençant le {} dans la paie de {}.", salarie, jj(&debut)?, mois_a)));
            };
            let p = &paies[rang_p];
            sal = Some(p.salarie.clone());
            let ent = crate::avenant::ent_du_mois(ctx, &mois_a)?;
            let pdf = ctx.racine.join("sorties").join(&mois_a).join("envoi").join(format!(
                "Avenant{}_{}_{}_{}.pdf",
                a.numero(),
                salarie,
                if vrai(&a.apercu["contrat"]) { valeur_en_texte(&a.apercu["contrat"]) } else { "sans-numero".to_string() },
                envoi::etiquette(&mois_a).map_err(|m| erreur("ValueError", m))?
            ));
            crate::contrat::ecrire_avenant(
                ctx,
                p,
                &ent,
                &pdf,
                &a.modifications(),
                a.numero(),
                &jj(&a.texte("date_contrat"))?,
                &jj(&a.effet)?,
                &aujourdhui_jj,
            )?;
            // Une empreinte par rang de la saisie : deux contrats du meme jour ne partagent pas la leur.
            let rangs: Vec<Paire> = paies.iter().enumerate().map(|(i, x)| (cle_de(x), format!("{}#{}", x.engagement.debut.iso(), i))).collect();
            let empreintes = Moteur::empreintes_engagements(&SaisieMois { data: data.clone(), paies: Vec::new() }, &rangs);
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
            c.insert("empreinte".into(), empreintes.get(&(salarie.clone(), format!("{}#{}", debut, rang_p))).map_or(Value::Null, |e| json!(e)));
            c.insert("clauses".into(), clauses_du_contrat(ctx, p, &ent));
            c.insert("adresse".into(), json!(a.texte("destinataire").trim().to_lowercase()));
            c.insert("relance".into(), json!(false));
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
                "l'avenant n° {} à ton contrat d'engagement n° {} pour {}, du {} au {}, qui modifie {}",
                a.numero(),
                a.contrat(),
                a.texte("spectacle"),
                jj(&a.texte("debut"))?,
                jj(&a.texte("fin"))?,
                articles
            ))
        };
        let sujet_de = |spectacle: &str| envoi::formater(SUJET_SIGNATURE, &[("spectacle", spectacle.to_string()), ("mois", nom_mois.to_string()), ("annee", an.to_string())]);
        let (sujet, liste, quoi);
        if aa.len() == 1 {
            let a = &aa[0];
            sujet = format!("Avenant n° {} - {}", a.numero(), sujet_de(&a.texte("spectacle")));
            liste = format!("Tu trouveras ci-joint {} ; le reste du contrat ne change pas.", un(a)?);
            quoi = format!("l'avenant n° {} au contrat d'engagement n° {} pour {}", a.numero(), a.contrat(), a.texte("spectacle"));
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
                aa.iter().map(|a| format!("n° {} au contrat d'engagement n° {} pour {}", a.numero(), a.contrat(), a.texte("spectacle"))).collect::<Vec<_>>().join(" ; ")
            );
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
        let mut identifiants: Vec<String> = Vec::new();
        let partis_ici = match envoi::envoyer_suivi(std::slice::from_ref(&message), &expediteur, client.as_mut(), Some(&mut identifiants), None) {
            Ok(p) => p,
            Err(e) => {
                noter_echec_d_envoi(ctx, "contrat-signature", Some(&mois), "interface", &e, Some(json!({"contrats": lignes_contrats})))?;
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
        let detail = json!({"contrats": lignes_contrats, "expediteur": expediteur, "relance": false, "acte": "avenant"});
        lignes.push(noter(ctx, "contrat-signature", Some(&mois), "interface", true, &partis_ici.join(", "), &reponse, Some(&detail))?);
        partis.extend(partis_ici);
        for (a, pdf) in aa.iter().zip(&pdfs) {
            faits.push(json!({"salarie": salarie, "debut": a.texte("debut"), "numero": a.apercu["numero"], "fichier": nom_du(pdf)}));
        }
    }
    Ok(Avenants { partis, avenants: faits, lignes })
}
