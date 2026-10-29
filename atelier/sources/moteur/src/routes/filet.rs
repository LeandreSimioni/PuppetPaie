//! Les routes du filet et du canal de l'editeur :
//!
//! - `incident-navigateur` : ce qui casse dans la page (le journal des incidents ecrit, et c'est tout ; ne leve jamais) ;
//! - `diagnostic-etat`, `diagnostic-envoyer`, `diagnostic-regler` : ce qui est parti chez l'editeur, et l'envoi de ce
//!   qui attend ;
//! - `signalements`, `signalement-apercu`, `signalement-envoyer` (enregistre, ne part pas), `signalements-partir` (le
//!   depart, en fond), `signalements-lus`, `signalements-reponses` : « Un probleme, une idee ? » ;
//! - `filet-etat`, `filet-verifier`, `filet-activer`, `filet-desactiver`, `filet-suivies` : le filet DPAE (activer et
//!   desactiver exigent le mot de passe du logiciel s'il est pose : le moteur le verifie, jamais la page) ;
//! - `incidents-lire`, `incidents-regrouper`, `incidents-oublier`, `incidents-envoyer` : les incidents gardes, pour la
//!   veille et la ligne de commande.
//!
//! Le canal de l'editeur est `editeur.rs` ; jamais d'autre serveur que celui de la configuration.

use std::collections::HashMap;
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::compagnie::incidents::{self, DEPOSE, EN_ATTENTE};
use crate::compagnie::{filet, problemes};
use droit::contexte::Contexte;
use super::{lire_json_tolerant, longueur, objet, valeur};
use envois::etats::{get, objet_vide};
use socle::echec::{erreur, Echec, R};
use socle::fichiers::vrai;
use socle::valeur::{valeur_en_texte, valeurs_egales};



/// Les routes que ce module sert.
pub const ROUTES: &[&str] = &[
    "filet-etat",
    "signalements",
    "incident-navigateur",
    "diagnostic-etat",
    "diagnostic-envoyer",
    "diagnostic-regler",
    "signalement-apercu",
    "signalement-envoyer",
    "signalements-lus",
    "signalements-partir",
    "signalements-reponses",
    "filet-verifier",
    "filet-activer",
    "filet-desactiver",
    "filet-suivies",
    "incidents-lire",
    "incidents-regrouper",
    "incidents-envoyer",
];

/// `incidents-oublier` est a part : elle efface, et ne se range pas avec les lectures.
pub const ROUTE_OUBLIER: &str = "incidents-oublier";

/// Une route de ce module.
pub fn route(o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    let mut v = route_seule(o, ctx, nom, charge)?;
    poser_les_pastilles(nom, &mut v);
    Ok(v)
}

/// LES PASTILLES DE CE QUE CES ROUTES RENDENT (`routes::pastille`). Le filet DPAE : desactive, une information ; actif,
/// fait ; une adresse a confirmer, a faire. Le diagnostic : coupe par la compagnie, une information ; rien de note ou
/// tout parti, fait ; des incidents en attente, une information (ils partent d'eux-memes). Un message a l'editeur : en
/// cours ou pas encore parti, une information.
fn poser_les_pastilles(nom: &str, v: &mut Value) {
    let Some(o) = v.as_object_mut() else { return };
    let nombre = |o: &serde_json::Map<String, Value>, k: &str| o.get(k).and_then(Value::as_u64).unwrap_or(0);
    match nom {
        "filet-etat" | "filet-verifier" | "filet-activer" | "filet-desactiver" => {
            let p = if !o.get("actif").is_some_and(vrai) {
                super::pastille("information", "désactivé")
            } else if o.get("confirme").is_some_and(vrai) {
                super::pastille("fait", "actif")
            } else {
                super::pastille("a_faire", "adresse à confirmer")
            };
            o.insert("pastille".into(), p);
        }
        "diagnostic-etat" | "diagnostic-envoyer" | "diagnostic-regler" => {
            let attente = nombre(o, "en_attente");
            let p = if o.get("envoi") == Some(&Value::Bool(false)) {
                super::pastille("information", "désactivé")
            } else if nombre(o, "total") == 0 {
                super::pastille("fait", "rien à signaler")
            } else if attente > 0 {
                super::pastille("information", &format!("{} en attente", attente))
            } else {
                super::pastille("fait", "tout est parti")
            };
            o.insert("pastille".into(), p);
            for g in o.get_mut("groupes").and_then(Value::as_array_mut).into_iter().flatten() {
                let Some(g) = g.as_object_mut() else { continue };
                let attente = nombre(g, "en_attente");
                let p = if attente > 0 {
                    super::pastille("information", &format!("{} en attente", attente))
                } else {
                    let le = g.get("parti_le").map(valeur_en_texte).unwrap_or_default();
                    super::pastille("fait", &format!("parti le {}", super::quand(&le)))
                };
                g.insert("pastille".into(), p);
            }
        }
        "signalements" | "signalement-envoyer" => {
            for s in o.get_mut("signalements").and_then(Value::as_array_mut).into_iter().flatten() {
                let Some(s) = s.as_object_mut() else { continue };
                let etat = s.get("etat").and_then(Value::as_str).map(String::from);
                let p = match etat.as_deref() {
                    Some("en-cours") => super::pastille("information", "en cours d'envoi"),
                    Some("pas-parti") => super::pastille("information", "pas encore parti"),
                    _ => Value::Null,
                };
                s.insert("pastille".into(), p);
                // Son depart, que la boite du message dit juste apres l'envoi : PuppetPaie renvoie de lui-meme ce qui
                // n'est pas parti, rien n'est a faire (une information).
                let depart = match etat.as_deref() {
                    Some("en-cours") => super::pastille("information", "Envoi en cours…"),
                    Some("pas-parti") => super::pastille("information", "Pas encore parti"),
                    _ => super::pastille("fait", "Message envoyé."),
                };
                s.insert("depart".into(), depart);
            }
        }
        _ => {}
    }
}

fn route_seule(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    let racine = ctx.racine.as_path();
    match nom {
        "filet-etat" => etat_filet(racine),
        "signalements" => signalements_route(racine),
        "incident-navigateur" => {
            let vide = json!({});
            let rapport = if charge.is_object() { charge } else { &vide };
            let ligne = incidents::depuis_navigateur(racine, rapport);
            Ok(json!({
                "note": ligne.is_some(),
                "empreinte": ligne.as_ref().and_then(|l| l.get("empreinte")).cloned().unwrap_or(Value::Null),
            }))
        }
        "diagnostic-etat" => diagnostic_etat(racine),
        "diagnostic-envoyer" => {
            let bilan = incidents::envoyer(racine);
            let mut etat = diagnostic_etat(racine)?;
            etat.as_object_mut().expect("un objet").insert("bilan".into(), bilan);
            Ok(etat)
        }
        "incidents-envoyer" => Ok(incidents::envoyer(racine)),
        // `{"actif": bool}` : couper ou rendre l'envoi des incidents a l'editeur.
        "diagnostic-regler" => {
            incidents::regler_envoi(racine, vrai(&charge["actif"]))?;
            diagnostic_etat(racine)
        }
        "incidents-lire" => {
            let (nature, en_attente) = (nature_voulue(charge), vrai(&charge["en_attente"]));
            Ok(json!({"lignes": incidents::lire(racine, nature.as_deref(), en_attente)?}))
        }
        "incidents-regrouper" => Ok(json!({"paquets": incidents::regrouper(racine, nature_voulue(charge).as_deref())?})),
        ROUTE_OUBLIER => Ok(json!({"retires": incidents::oublier(racine, nature_voulue(charge).as_deref())})),
        "signalement-apercu" => problemes::apercu(racine, &champ(charge, "texte"), &ou_vide(charge, "ecran"), &problemes::captures_de(charge)?),
        "signalement-envoyer" => problemes::envoyer(racine, &champ(charge, "texte"), &ou_vide(charge, "ecran"), &problemes::captures_de(charge)?),
        "signalements-lus" => Ok(json!({"lues": problemes::signalements_lus(racine)?})),
        "signalements-partir" => Ok(problemes::partir(racine)),
        "signalements-reponses" => Ok(json!({"nouvelles": problemes::relever_reponses(racine)?})),
        "filet-verifier" => filet::verifier(racine),
        "filet-activer" => filet::activer(racine, &ou_vide(charge, "adresse"), &champ(charge, "motdepasse")),
        "filet-desactiver" => filet::desactiver(racine, &champ(charge, "motdepasse")),
        "filet-suivies" => Ok(json!({"suivies": filet::suivies(ctx)?})),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

/// `charge.get(cle)`.
fn champ(charge: &Value, cle: &str) -> Value {
    charge.get(cle).cloned().unwrap_or(Value::Null)
}

/// `charge.get(cle) or ""`.
fn ou_vide(charge: &Value, cle: &str) -> Value {
    match charge.get(cle) {
        Some(v) if vrai(v) => v.clone(),
        _ => json!(""),
    }
}

fn nature_voulue(charge: &Value) -> Option<String> {
    charge.get("quoi").and_then(Value::as_str).filter(|q| !q.is_empty()).map(str::to_string)
}

/// `diagnostic_etat` : l'ecran « ce qui est parti chez l'editeur ». Le journal
/// des incidents, regroupe, avec pour chaque groupe ce qui est parti et ce qui
/// attend -- et, pour le dernier de chaque groupe, EXACTEMENT ce qui part
/// (`pour_l_editeur`), pas un resume : l'utilisateur lit ce qui quitte son poste.
fn diagnostic_etat(racine: &std::path::Path) -> R<Value> {
    let serveur = incidents::configuration(racine)?.map(|(s, _)| s).unwrap_or_default();
    let lignes = incidents::lire(racine, None, false)?;
    let mut groupes: Vec<(Value, Map<String, Value>)> = Vec::new();
    for l in &lignes {
        let empreinte = get(l, "empreinte")?;
        let quand = get(l, "quand")?;
        let regroupe = {
            let q = get(l, "quoi")?;
            q.as_str().is_some_and(|q| q == incidents::LOGICIEL || q == incidents::REFUS)
        };
        let cle = if regroupe { empreinte.clone() } else { json!(format!("{}|{}", valeur_en_texte(empreinte), valeur_en_texte(quand))) };
        let i = match groupes.iter().position(|(k, _)| valeurs_egales(k, &cle)) {
            Some(i) => i,
            None => {
                let mut g = Map::new();
                g.insert("quoi".into(), get(l, "quoi")?.clone());
                g.insert("ou".into(), get(l, "ou")?.clone());
                g.insert("genre".into(), get(l, "genre")?.clone());
                g.insert("resume".into(), get(l, "resume")?.clone());
                g.insert("combien".into(), json!(0));
                g.insert("partis".into(), json!(0));
                g.insert("en_attente".into(), json!(0));
                g.insert("premiere".into(), quand.clone());
                g.insert("derniere".into(), Value::Null);
                g.insert("parti_le".into(), Value::Null);
                g.insert("contenu".into(), Value::Null);
                groupes.push((cle, g));
                groupes.len() - 1
            }
        };
        let envoi = get(l, "envoi")?;
        let envoi = if vrai(envoi) { envoi.clone() } else { json!({}) };
        let etat = get(&envoi, "etat")?;
        let g = &mut groupes[i].1;
        // Un incident revu (ses doublons, `fois`) compte autant de fois qu'il a eu lieu.
        let fois = incidents::fois_de(l);
        let incr = |g: &mut Map<String, Value>, c: &str| {
            let n = g[c].as_i64().unwrap_or(0) + fois;
            g.insert(c.into(), json!(n));
        };
        incr(g, "combien");
        g.insert("derniere".into(), l.get("derniere").filter(|d| d.is_string()).unwrap_or(quand).clone());
        let resume = get(l, "resume")?;
        if vrai(resume) {
            g.insert("resume".into(), resume.clone());
        }
        g.insert("contenu".into(), incidents::pour_l_editeur(racine, l)?);
        if etat.as_str() == Some(DEPOSE) {
            incr(g, "partis");
            let q = get(&envoi, "quand")?;
            if vrai(q) {
                g.insert("parti_le".into(), q.clone());
            }
        } else if etat.as_str() == Some(EN_ATTENTE) {
            incr(g, "en_attente");
        }
    }
    // `sorted(..., key=derniere or "", reverse=True)` : stable.
    let cle_de = |g: &Map<String, Value>| if vrai(&g["derniere"]) { valeur_en_texte(&g["derniere"]) } else { String::new() };
    let mut liste: Vec<Map<String, Value>> = groupes.into_iter().map(|(_, g)| g).collect();
    liste.sort_by(|a, b| cle_de(b).cmp(&cle_de(a)));
    let partis: Vec<String> = liste.iter().filter(|g| vrai(&g["parti_le"])).map(|g| valeur_en_texte(&g["parti_le"])).collect();
    let somme = |c: &str| liste.iter().map(|g| g[c].as_i64().unwrap_or(0)).sum::<i64>();
    let (partis_total, en_attente_total) = (somme("partis"), somme("en_attente"));
    // LE NOMBRE DE PAIES DE L'ANNEE se voit dans la meme liste : une entree par annee partie, avec exactement
    // ce qui a quitte le poste. Ce n'est pas un incident : il ne compte ni dans `total`, ni dans `partis`.
    let annees = crate::compagnie::nombre_de_paies::pour_l_ecran(racine);
    for a in &annees {
        let mut g = Map::new();
        g.insert("quoi".into(), json!("paies"));
        g.insert("ou".into(), json!("une fois par an"));
        g.insert("genre".into(), json!(""));
        g.insert("resume".into(), json!(format!("{} : {} paie(s) faite(s)", a["annee"], a["paies"])));
        g.insert("combien".into(), json!(1));
        g.insert("partis".into(), json!(1));
        g.insert("en_attente".into(), json!(0));
        g.insert("premiere".into(), a["parti_le"].clone());
        g.insert("derniere".into(), a["parti_le"].clone());
        g.insert("parti_le".into(), a["parti_le"].clone());
        g.insert("contenu".into(), a["contenu"].clone());
        liste.push(g);
    }
    liste.sort_by(|a, b| cle_de(b).cmp(&cle_de(a)));
    Ok(json!({
        "serveur": serveur,
        "envoi": incidents::envoi_actif(racine),
        "total": lignes.len(),
        "partis": partis_total,
        "en_attente": en_attente_total,
        "dernier_envoi": partis.iter().max().map_or(Value::Null, |m| json!(m)),
        "paies_annuelles": annees,
        "groupes": liste.into_iter().map(Value::Object).collect::<Vec<_>>(),
    }))
}

/// `etat_filet` : le compte (sans sa cle), et le nombre de reveils. Lit
/// `config/filet-dpae.json` : rien ne part vers le serveur.
pub(crate) fn etat_filet(racine: &Path) -> R<Value> {
    let mut e = lire_json_tolerant(&racine.join("config").join("filet-dpae.json"))?.unwrap_or_else(|| Value::Object(Map::new()));
    let Value::Object(m) = &mut e else {
        // Un fichier qui n'est pas un objet JSON est un refus avec sa phrase, jamais un plantage.
        // `commandes/filet.rs` le dit en « ! ... », code 1.
        return Err(erreur("ErreurOperation", format!("{} est illisible : ce n'est pas un objet JSON.\nLe deplacer suffit a repartir de zero (le filet sera a reactiver).", racine.join("config").join("filet-dpae.json").display())));
    };
    for (cle, defaut) in [("compte", Value::Null), ("tests", json!({})), ("reveils", json!({}))] {
        m.entry(cle).or_insert(defaut);
    }
    let compte = valeur(m, "compte");
    let c = if vrai(compte) { compte } else { objet_vide() };
    let c = objet(c)?;
    let actif = !c.is_empty();
    let adresse = c.get("adresse").cloned().unwrap_or_else(|| json!(""));
    let confirme = vrai(valeur(c, "confirme"));
    // Le serveur auquel `filet-activer` s'inscrirait : celui du compte, de la racine, sinon celui du moteur.
    let serveur = json!(crate::compagnie::filet::serveur(&Value::Object(m.clone())));
    let inscrit_le = c.get("inscrit_le").cloned().unwrap_or_else(|| json!(""));
    let erreur_c = c.get("erreur").cloned().unwrap_or_else(|| json!(""));
    let reveils = longueur(valeur(m, "reveils"))?;
    Ok(json!({
        "actif": actif, "adresse": adresse, "confirme": confirme, "serveur": serveur,
        "inscrit_le": inscrit_le, "erreur": erreur_c, "reveils": reveils,
    }))
}

/// La route `signalements` : `{"signalements", "non_lues"}`.
pub(crate) fn signalements_route(racine: &Path) -> R<Value> {
    let liste = crate::compagnie::problemes::signalements(racine);
    // `reponses_non_lues` : les reponses que la boite n'a pas encore montrees.
    let mut non_lues = 0;
    for l in &liste {
        if vrai(get(l, "reponse")?) && !vrai(get(l, "lu")?) {
            non_lues += 1;
        }
    }
    Ok(json!({"signalements": liste, "non_lues": non_lues}))
}
