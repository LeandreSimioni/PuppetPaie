//! Les routes du lot « Incidents et canal editeur » (`moteur/web.py`,
//! `moteur/operations_filet.py`, `moteur/commandes_incidents.py`) :
//!
//! - `incident-navigateur` : ce qui casse dans la page (la page poste, le journal
//!   des incidents ecrit, et c'est tout ; ne leve jamais) ;
//! - `diagnostic-etat`, `diagnostic-envoyer` : ce qui est parti chez l'editeur, et
//!   l'envoi de ce qui attend ;
//! - `signalement-apercu`, `signalement-envoyer` (enregistre, ne part pas),
//!   `signalements-partir` (le depart, en fond), `signalements-lus` : « Un
//!   probleme, une idee ? » ;
//! - `filet-verifier`, `filet-activer`, `filet-desactiver` : le filet DPAE (activer
//!   et desactiver exigent le mot de passe du logiciel s'il est pose : le moteur
//!   le verifie, jamais la page).
//!
//! Et celles que le Python encore appele (la veille, la ligne de commande, les
//! depots) utilise pour ne plus rien faire lui-meme : `incidents-lire`,
//! `incidents-regrouper`, `incidents-oublier`, `incidents-envoyer`,
//! `signalements-reponses`, `filet-suivies`, `filet-passage`, `filet-desarmer`.
//!
//! Le canal de l'editeur est `editeur.rs` ; jamais d'autre serveur que celui de la
//! configuration.

use std::collections::HashMap;

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use envois::etats::{get};
use socle::valeur::valeurs_egales;
use socle::valeur::valeur_en_texte;
use socle::echec::R;
use socle::fichiers::vrai;
use crate::incidents::{self, DEPOSE, EN_ATTENTE};
use socle::echec::Echec;
use crate::{filet, problemes};

/// Les routes que ce module sert.
pub const ROUTES: [&str; 18] = [
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
    "filet-passage",
    "filet-desarmer",
    "incidents-lire",
    "incidents-regrouper",
    "incidents-envoyer",
];

/// `incidents-oublier` est a part : elle efface, et ne se range pas avec les lectures.
pub const ROUTE_OUBLIER: &str = "incidents-oublier";

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    let racine = ctx.racine.as_path();
    match nom {
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
        "filet-passage" => filet::passage(ctx, &champ(charge, "tests"), &champ(charge, "vivantes"), &champ(charge, "maintenant")),
        "filet-desarmer" => Ok(json!({"desarme": filet::desarmer(racine, &filet::cle_du_reveil(charge))?})),
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
    // LE NOMBRE DE PAIES DE L'ANNEE (ticket #246) se voit dans la meme liste : une entree par annee partie, avec exactement
    // ce qui a quitte le poste. Ce n'est pas un incident : il ne compte ni dans `total`, ni dans `partis`.
    let annees = crate::nombre_de_paies::pour_l_ecran(racine);
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
