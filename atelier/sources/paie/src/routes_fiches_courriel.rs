//! `fiche-demander` et `fiches-relever` (`moteur/operations_questionnaire.py` : `demander_fiche`, `fiches_attendues`,
//! `relever_fiches`, `ReleveFiches` ; `moteur/simulation.py` : `BoiteDesFiches`). Lot H4a.
//!
//! ECRIT, NON PROUVE, NON BRANCHE. La fiche remplie par le salarie lui-meme, par courriel, comme le « Je signe » d'un
//! contrat et sans aucun service tiers : la demande part avec un bouton « Remplir ma fiche » (un lien `mailto:` vers la
//! boite de la compagnie, les questions deja ecrites) et un CODE (`FIC-XXXXXX`) garde au journal ; la releve cherche ce
//! code dans la boite, lit ce que la personne a ECRIT (la citation exclue) et CREE la fiche salarie avec ce qu'elle a
//! repondu, par le meme code que `enregistrer-fiche` (`routes_ecritures::ecrire_fiche`, `forcer`).
//!
//! - `demander_fiche` part chez quelqu'un : la PREUVE (le mot de passe du logiciel, retape), comme les contrats. Dans
//!   une compagnie fictive, la boite simulee ecrit le message dans `courriels-simules/`. Le journal garde l'echec comme la
//!   reussite.
//! - `relever_fiches` ne fait rien partir et ne demande rien. Trois filtres : notre propre envoi est ecarte, le code
//!   doit etre dans ce que la personne a ECRIT, et la reponse doit venir de l'adresse a laquelle la demande est
//!   partie. Elle ne leve pas pour une boite absente : elle le dit dans `incidents`. C'est UNE FONCTION, que la veille
//!   (lot I) appellera avec son `qui`.
//! - Dans une compagnie fictive, la personne fictive repond (`BoiteDesFiches`) et c'est la vraie releve qui la lit.
//!
//! Les phrases des refus et des incidents sont celles du Python ; la classe d'un refus peut differer.

use std::collections::HashMap;

use serde_json::{json, Map, Value};

use socle::texte::strip;
use droit::contexte::Contexte;
use crate::courriel::{self, Client, Courriel};
use crate::simulation_courriels::BoiteQuiRepond;
use crate::courriel_lecture as lecture;
use crate::depot_acte::{verifier_preuve, Preuve};
use crate::envoi::{self, Message};
use socle::valeur::valeur_en_texte;
use socle::echec::erreur;
use socle::fichiers::vrai;
use documents::fiche_questionnaire as fq;
use envois::journal::{self, Note};
use socle::echec::Echec;
use crate::routes_envois::{client_courriel, preuve_de, refus, vrai_ou, R};
use envois::ligne_d_envoi::{echec_d_envoi, noter};
use socle::html::{html_echapper, mailto};
use socle::texte::capitaliser;
use socle::paris::maintenant;

/// Les routes de ce module.
pub const ROUTES: [&str; 2] = ["fiche-demander", "fiches-relever"];

/// Secondes entre une demande de fiche et la reponse de la personne fictive de la compagnie fictive (`DELAI_SIGNATURE`).
pub const DELAI_FICTIVE: i64 = 20;

/// Une route de ce module, avec la vraie boite (ou la simulee).
pub fn route(o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    route_avec(o, ctx, nom, charge, None)
}

/// Une route de ce module, la boite venant de `client` quand il est donne (un faux client dans les tests).
pub fn route_avec(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value, client: Option<&mut dyn Courriel>) -> R<Value> {
    let texte = |cle: &str| charge.get(cle).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
    match nom {
        "fiche-demander" => {
            let d = demander_fiche(ctx, &texte("adresse"), &texte("prenom"), &preuve_de(charge), "interface", client)?;
            // Retours Windows, A : plus de configuration dans la reponse (NIR et IBAN compris) ; la page la relit.
            Ok(json!({"code": d.code, "adresse": d.adresse}))
        }
        "fiches-relever" => {
            let r = relever_fiches(ctx, client, "interface")?;
            Ok(json!({"attendues": r.attendues, "recues": r.recues, "incidents": r.incidents}))
        }
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

// ------------------------------------------------------------------ l'envoi

/// `re.fullmatch(r"[^@\s]+@[^@\s]+\.[^@\s]+", adresse)`.
fn adresse_valide(adresse: &str) -> bool {
    let blanc = socle::texte::blanc;
    if adresse.chars().any(blanc) || adresse.matches('@').count() != 1 {
        return false;
    }
    let (local, domaine) = adresse.split_once('@').unwrap_or(("", ""));
    let d: Vec<char> = domaine.chars().collect();
    !local.is_empty() && d.len() >= 3 && (1..d.len() - 1).any(|i| d[i] == '.')
}

/// Ce que `demander_fiche` rend : le code, l'adresse (en minuscules), la ligne du journal.
pub struct FicheDemandee {
    pub code: String,
    pub adresse: String,
    pub ligne: Value,
}

/// `operations_questionnaire.demander_fiche` : envoie a `adresse` le courriel « Remplir ma fiche ». Le journal garde
/// le code : c'est par lui que la releve reconnaitra la reponse. `client` : la boite, ouverte ici (boite simulee dans une
/// compagnie fictive, sinon refus « boite absente ») quand l'appelant n'en donne pas.
pub fn demander_fiche(ctx: &Contexte, adresse: &str, prenom: &str, preuve: &Preuve, qui: &str, client: Option<&mut dyn Courriel>) -> R<FicheDemandee> {
    let adresse = strip(adresse).to_lowercase();
    if !adresse_valide(&adresse) {
        return Err(refus(format!("« {} » n'est pas une adresse de courriel.", adresse)));
    }
    let mut ouverte: Option<Box<dyn Courriel>> = None;
    let client: &mut dyn Courriel = match client {
        Some(c) => c,
        None => ouverte.insert(client_courriel(ctx)?).as_mut(),
    };
    verifier_preuve(ctx, Some(preuve), &maintenant()[..7], "demande de fiche (« Remplir ma fiche »)", &adresse)?;

    let ent = &ctx.entreprise;
    let compagnie = match vrai_ou(ent, "nom_affichage") {
        Some(c) => c,
        None => ent.get("raison_sociale").map(valeur_en_texte).unwrap_or_default(),
    };
    let sg = ent.get("signataire").filter(|v| vrai(v)).cloned().unwrap_or_else(|| json!({}));
    let signataire = match vrai_ou(&sg, "prenom_affichage") {
        Some(p) => p,
        None => capitaliser(&sg.get("prenom").map(valeur_en_texte).unwrap_or_default()),
    };
    let expediteur = client.compte().to_lowercase();
    let code = fq::code_fiche();
    let q = fq::questionnaire(&code);
    let sujet = envoi::formater(fq::SUJET, &[("compagnie", compagnie.clone())]);
    let prenom = strip(prenom).to_string();
    let virgule_prenom = if prenom.is_empty() { String::new() } else { format!(" {}", prenom) };
    let champs = |echappe: bool| -> Vec<(&'static str, String)> {
        let e = |s: &str| if echappe { html_echapper(s) } else { s.to_string() };
        vec![("virgule_prenom", e(&virgule_prenom)), ("compagnie", e(&compagnie)), ("signataire", e(&signataire))]
    };
    let lien = mailto(&expediteur, &format!("Re: {}", sujet), &q);
    let mut texte = champs(false);
    texte.push(("questionnaire", q.clone()));
    let mut html = champs(true);
    html.push(("questionnaire", html_echapper(&q)));
    html.push(("mailto", html_echapper(&lien)));
    let message = Message {
        cle: format!("fiche-{}", code),
        nom: if prenom.is_empty() { adresse.clone() } else { prenom.clone() },
        destinataire: adresse.clone(),
        sujet,
        corps: envoi::formater(fq::CORPS, &texte),
        html: envoi::formater(fq::HTML, &html),
        pieces: Vec::new(),
    };
    let mut identifiants: Vec<String> = Vec::new();
    if let Err(e) = envoi::envoyer(&[message], &expediteur, client, Some(&mut identifiants), None) {
        let detail = json!({"code": code, "adresse": adresse, "prenom": prenom});
        noter(ctx, "fiche-demande", None, qui, false, &adresse, &e, Some(&detail))?;
        return Err(echec_d_envoi(e));
    }
    let detail = json!({
        "code": code,
        "adresse": adresse,
        "prenom": prenom,
        "message": identifiants.first().cloned().unwrap_or_default(),
        "expediteur": expediteur,
    });
    let ligne = noter(ctx, "fiche-demande", None, qui, true, &adresse, &format!("fiche demandee a {}", adresse), Some(&detail))?;
    Ok(FicheDemandee { code, adresse, ligne })
}

// ------------------------------------------------------------------ les demandes attendues

/// Une demande partie et pas encore revenue.
#[derive(Debug, Clone)]
pub struct Attendue {
    pub code: String,
    pub adresse: String,
    pub prenom: String,
    /// L'heure de la demande (ISO), celle de la ligne du journal.
    pub quand: String,
    /// Les identifiants de NOS envois (le `Message-ID` gardé avec la demande) : jamais pris pour une reponse.
    pub envoyes: Vec<String>,
}

/// `fiches_attendues` : les demandes parties et pas encore revenues, les plus anciennes d'abord. Une demande a
/// laquelle on renonce se retire (`fiche-oublier-demande`, `routes_ecritures`).
pub fn fiches_attendues(ctx: &Contexte) -> R<Vec<Attendue>> {
    let lignes = envois::etats::charger(&ctx.racine)?;
    let detail = |l: &Value| -> Value { l.get("detail").filter(|d| vrai(d)).cloned().unwrap_or_else(|| json!({})) };
    let closes: Vec<Value> = envois::etats::filtrer(&lignes, Some("fiche-recue"), None, false)?.iter().map(|l| detail(l).get("code").cloned().unwrap_or(Value::Null)).collect();
    let mut sortie = Vec::new();
    for l in envois::etats::filtrer(&lignes, Some("fiche-demande"), None, false)? {
        let d = detail(l);
        let code = d.get("code").cloned().unwrap_or(Value::Null);
        if !l.get("accepte").is_some_and(vrai) || !vrai(&code) || closes.contains(&code) {
            continue;
        }
        let texte = |cle: &str| d.get(cle).map(valeur_en_texte).unwrap_or_default();
        sortie.push(Attendue {
            code: valeur_en_texte(&code),
            adresse: texte("adresse"),
            prenom: texte("prenom"),
            quand: l.get("quand").and_then(Value::as_str).unwrap_or("").to_string(),
            envoyes: d.get("message").and_then(Value::as_str).filter(|m| !m.is_empty()).map(|m| vec![m.to_string()]).unwrap_or_default(),
        });
    }
    Ok(sortie)
}

// ------------------------------------------------------------------ la releve

/// Ce que la releve a fait : combien de demandes attendaient, les fiches creees, ce qui l'a empechee.
#[derive(Debug, Default)]
pub struct ReleveFiches {
    pub attendues: usize,
    /// `{code, cle, nom, illisible, erreurs}` par fiche creee ou completee.
    pub recues: Vec<Value>,
    pub incidents: Vec<String>,
}

impl ReleveFiches {
    /// `ReleveFiches.resume` : la phrase de la veille, vide quand rien n'est arrive.
    pub fn resume(&self) -> String {
        if self.recues.is_empty() {
            return String::new();
        }
        let noms: Vec<String> = self.recues.iter().map(|r| r.get("nom").map(valeur_en_texte).unwrap_or_default()).collect();
        format!("{} fiche(s) salarié reçue(s) : {}", self.recues.len(), noms.join(", "))
    }
}

fn os(m: String) -> Echec {
    erreur("OSError", m)
}

/// `operations_questionnaire.relever_fiches` : cherche les reponses aux demandes de fiche et cree chaque fiche.
///
/// `client` : la boite, ouverte ici quand l'appelant n'en donne pas -- la personne fictive de la compagnie fictive, ou la
/// vraie boite ; sans boite, un incident et rien d'autre. `qui` : `interface`, ou `veille`.
pub fn relever_fiches(ctx: &Contexte, client: Option<&mut dyn Courriel>, qui: &str) -> R<ReleveFiches> {
    let mut r = ReleveFiches::default();
    let attendues = fiches_attendues(ctx)?;
    r.attendues = attendues.len();
    if attendues.is_empty() {
        return Ok(r);
    }
    let mut ouverte: Option<Box<dyn Courriel>> = None;
    let client: &mut dyn Courriel = match client {
        Some(c) => c,
        None if crate::routes_accueil::simulation_active(&ctx.racine) => {
            // LA COMPAGNIE FICTIVE : la personne fictive repond, et c'est la vraie lecture qui suit.
            ouverte.insert(Box::new(boite_des_fiches(ctx, &attendues, DELAI_FICTIVE)?)).as_mut()
        }
        None => {
            if !courriel::disponible(&ctx.racine) {
                r.incidents.push("Aucune boite courriel : les fiches demandees ne peuvent pas etre relevees.".to_string());
                return Ok(r);
            }
            let vrai_client = Client::du_contexte(ctx).map_err(|e| erreur("ErreurCourriel", e.0))?;
            ouverte.insert(Box::new(vrai_client)).as_mut()
        }
    };
    let expediteur = client.compte().to_lowercase();

    for a in &attendues {
        let code = &a.code;
        let identifiants = match client.rechercher(&format!("\"{}\"", code), 10) {
            Ok(i) => i,
            Err(e) => {
                r.incidents.push(format!("Recherche de {} : {}", code, e));
                break;
            }
        };
        let mut retenu: Option<(String, Vec<u8>, String, String, String)> = None;
        for ident in identifiants {
            if a.envoyes.contains(&ident) {
                continue;
            }
            let brut = match client.message_brut(&ident) {
                Ok(b) => b,
                Err(e) => {
                    r.incidents.push(format!("Lecture du message {} : {}", ident, e));
                    continue;
                }
            };
            let Ok(m) = mailparse::parse_mail(&brut) else {
                r.incidents.push(format!("Lecture du message {} : message illisible.", ident));
                continue;
            };
            if a.envoyes.contains(&lecture::identifiant_de(&m)) {
                continue;
            }
            let texte = lecture::texte_propre(&m);
            if !texte.to_uppercase().contains(code.as_str()) || texte.contains("Remplir ma fiche") {
                continue;
            }
            let de = lecture::adresse_de(&m);
            if de == expediteur && de != a.adresse {
                continue;
            }
            if de != a.adresse {
                r.incidents.push(format!("Fiche {} : une reponse vient de {} et non de {} ; elle n'est pas retenue.", code, de, a.adresse));
                continue;
            }
            let quand = lecture::date_de(&m, &maintenant());
            retenu = Some((ident, brut.clone(), texte, de, quand));
            break;
        }
        let Some((ident, brut, texte, de, quand)) = retenu else { continue };
        let (mut donnees, illisible) = fq::fiche_depuis_reponses(&fq::lire_reponses(&texte));
        donnees.insert("email".into(), json!(de));
        if !(donnees.get("nom_affichage").is_some_and(vrai) || donnees.get("prenom_affichage").is_some_and(vrai)) {
            r.incidents.push(format!("Fiche {} : la reponse de {} ne porte ni nom ni prenom lisible ; rien n'est cree.", code, de));
            continue;
        }
        let dossier = ctx.racine.join("sorties").join("fiches-salaries");
        std::fs::create_dir_all(&dossier).map_err(|e| os(format!("{} : {}", dossier.display(), e)))?;
        let eml = dossier.join(format!("Fiche_{}.eml", code));
        std::fs::write(&eml, &brut).map_err(|e| os(format!("{} : {}", eml.display(), e)))?;
        let jour: Vec<&str> = quand.get(..10).unwrap_or("").split('-').rev().collect();
        let mut note = format!("Fiche remplie par la personne, par courriel, le {}. Reste à choisir l'emploi.", jour.join("/"));
        if !illisible.is_empty() {
            note.push_str(&format!(" Non lu, à compléter : {}.", illisible.join(" ; ")));
        }
        let existante: Option<String> = ctx
            .salaries
            .iter()
            .find(|(k, s)| s.is_object() && !k.starts_with('_') && strip(&s.get("email").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default()).to_lowercase() == de)
            .map(|(k, _)| k.clone());
        let donnees: Map<String, Value> = match &existante {
            Some(k) => {
                // Une fiche qui existe deja : on ne complete que ses cases VIDES. Ce qu'elle porte a peut-etre deja
                // ete declare.
                let ancienne = ctx.salaries.get(k).and_then(Value::as_object).cloned().unwrap_or_default();
                let mut fusion = ancienne.clone();
                for (cle, valeur) in &donnees {
                    if strip(&ancienne.get(cle).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default()).is_empty() {
                        fusion.insert(cle.clone(), valeur.clone());
                    }
                }
                fusion.insert("fiche_recue".into(), json!(note));
                fusion
            }
            None => {
                let mut d = donnees;
                d.insert("fiche_recue".into(), json!(note));
                d.insert("actif".into(), json!(true));
                d
            }
        };
        let f = crate::routes_ecritures::ecrire_fiche(ctx, "salarie", existante.clone(), &Value::Object(donnees.clone()), crate::routes_ecritures::Forcer::Tout, &Value::Null, &Value::Null)?;
        let texte_de = |cle: &str| donnees.get(cle).map(valeur_en_texte).unwrap_or_default();
        let nom = format!("{} {}", texte_de("prenom_affichage"), texte_de("nom_affichage"));
        let erreurs = f.anomalies.iter().filter(|a| a.get("gravite").and_then(Value::as_str) == Some("erreur")).count();
        let detail = json!({"code": code, "salarie": f.cle, "illisible": illisible, "erreurs": erreurs});
        let reponse = format!("fiche {} par courriel", if existante.is_some() { "completee" } else { "creee" });
        let origine = format!("courriel {}", ident);
        let note_du_journal = Note {
            quoi: "fiche-recue",
            mois: None,
            fichier: None,
            mode: journal::REEL,
            qui,
            accepte: true,
            destinataire: &de,
            flux: "",
            reponse: &reponse,
            trace: None,
            detail: Some(&detail),
            versions: None,
            quand: &quand,
            origine: &origine,
        };
        journal::noter(&ctx.racine, &note_du_journal, &maintenant()).map_err(os)?;
        r.recues.push(json!({"code": code, "cle": f.cle, "nom": strip(&nom), "illisible": illisible, "erreurs": erreurs}));
    }
    client.fermer();
    Ok(r)
}

// ------------------------------------------------------------------ la compagnie fictive

/// Des personnes inventees, comme celles des compagnies fictives : (prenom, nom, sexe INSEE). Le NIR se
/// fabrique a la demande, cle comprise.
const PERSONNES: [(&str, &str, &str); 4] = [("Marie", "Dorval", "2"), ("Pierre", "Bocage", "1"), ("Rose", "Cheri", "2"), ("Frederick", "Lemaitre", "1")];

/// `_reponse_de_fiche` : le questionnaire que la personne fictive renvoie, rempli ligne a ligne dans les termes memes
/// des questions, le code en dernier. `bouton` : (objet, corps) du lien du courriel de la demande, quand on l'a.
fn reponse_de_fiche(demande: &Attendue, compte: &str, bouton: Option<&(String, String)>) -> Result<Vec<u8>, String> {
    let code = &demande.code;
    let somme: u32 = code.chars().map(|c| c as u32).sum();
    let (prenom, nom, sexe) = PERSONNES[somme as usize % PERSONNES.len()];
    let prenom = if demande.prenom.is_empty() { prenom.to_string() } else { demande.prenom.clone() };
    let an = 80 + code.chars().count() % 15;
    let mois = 1 + somme % 12;
    let treize = format!("{}{:02}{:02}75056{:03}", sexe, an, mois, 1 + somme % 900);
    let cle = 97 - treize.parse::<u64>().unwrap_or(0) % 97;
    let reponses: HashMap<&str, String> = HashMap::from([
        ("prenom", prenom),
        ("nom", nom.to_string()),
        ("naissance", format!("12/{:02}/19{:02}", mois, an)),
        ("lieu", "Paris".to_string()),
        ("pays", "France".to_string()),
        ("nationalite", "française".to_string()),
        ("nir", format!("{}{:02}", treize, cle)),
        ("adresse", "12 rue du Temple".to_string()),
        ("code_postal", "75003".to_string()),
        ("commune", "Paris".to_string()),
    ]);
    let mut lignes: Vec<String> =
        fq::QUESTIONS.iter().map(|(cle, lib, fac)| format!("{}{} : {}", lib, if *fac { " (facultatif)" } else { "" }, reponses.get(cle).map(String::as_str).unwrap_or(""))).collect();
    lignes.push(String::new());
    lignes.push(format!("Code (ne pas modifier) : {}", code));
    let sujet = bouton.map(|b| b.0.clone()).unwrap_or_else(|| "Re: fiche".to_string());
    crate::simulation_courriels::reponse_jouee(&demande.adresse, compte, &sujet, &lignes.join("\n"), &format!("fiche-{}", code))
}

/// `simulation.BoiteDesFiches` : la boite de la compagnie fictive, vue par la releve. Chaque demande de fiche partie
/// depuis `delai` secondes y trouve sa reponse remplie ; la vraie releve la lit et cree la fiche. Ce qu'on y envoie
/// s'ecrit comme tout courriel joue (`BoiteQuiRepond`, partagee avec `BoiteDesSignatures`).
pub fn boite_des_fiches(ctx: &Contexte, attendues: &[Attendue], delai: i64) -> R<BoiteQuiRepond> {
    let mut boite = BoiteQuiRepond::nouvelle(ctx);
    let maintenant_naif = socle::paris::maintenant_naif();
    // Les courriels joues, par leur `Message-ID` : la demande retrouve ainsi son bouton.
    let envoyes = crate::simulation_courriels::courriels_par_identifiant(&ctx.racine);
    for a in attendues {
        let age = socle::paris::naif_de_iso(&a.quand).map_or(0, |t| maintenant_naif - t);
        if age < delai {
            continue;
        }
        let bouton = crate::simulation_courriels::bouton_de_la_demande(&envoyes, &a.envoyes);
        let brut = reponse_de_fiche(a, Courriel::compte(&boite), bouton.as_ref()).map_err(|m| erreur("ValueError", m))?;
        boite.repondre(&a.code, brut);
    }
    Ok(boite)
}
