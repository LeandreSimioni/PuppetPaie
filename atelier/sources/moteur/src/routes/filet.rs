//! Les routes du filet et du canal de l'editeur :
//!
//! - `incident-navigateur` : ce qui casse dans la page (le journal des incidents ecrit, et c'est tout ; ne leve jamais) ;
//! - `diagnostic-etat`, `diagnostic-envoyer`, `diagnostic-regler` : ce qui est parti chez l'editeur, et l'envoi de ce
//!   qui attend ;
//! - `signalements`, `signalement-apercu`, `signalement-envoyer` (enregistre, ne part pas), `signalements-partir` (le
//!   depart, en fond), `signalements-lus` : « Un probleme, une idee ? » ;
//! - `incidents-lire`, `incidents-regrouper`, `incidents-oublier`, `incidents-envoyer` : les incidents gardes, pour la
//!   veille et la ligne de commande.
//!
//! Le canal de l'editeur est `editeur.rs` ; jamais d'autre serveur que celui de la configuration.

use std::collections::HashMap;
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::compagnie::incidents::{self, DEPOSE, EN_ATTENTE};
use crate::compagnie::problemes;
use droit::contexte::Contexte;
use crate::routes::catalogue::{Acte, Champ, Fiche, FORME, OPERATION};
use envois::etats::get;
use socle::echec::{Echec, R};
use socle::fichiers::vrai;
use socle::valeur::{valeur_en_texte, valeurs_egales};
use crate::routes::catalogue::{Ecran, Place};



const TEXTE: Champ = Champ { nom: "texte", sens: "le message, en mots", avance: false };
const OU: Champ = Champ { nom: "ecran", sens: "où l'on était (l'écran, ou ce que le LLM a tenté : la route, la charge sans donnée personnelle, la réponse)", avance: false };
const CAPTURES: Champ = Champ { nom: "captures", sens: "[{type, donnees}] : des images en base64", avance: true };
const QUOI: Champ = Champ { nom: "quoi", sens: "une seule nature d'incident ; toutes sinon", avance: true };
const DIAGNOSTIC_REND: &str = "envoi (vrai : actif), total, en_attente, groupes (ce qui part, mot pour mot), pastille";

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Signaler);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        nom: "signalement-apercu",
        geste: "Montrer, mot pour mot, ce qu'un message au développeur emporterait.",
        charge: &[TEXTE, OU, CAPTURES],
        exemple: r#"{"texte":"La phrase de la DSN est coupée.","ecran":"Paie, novembre 2026"}"#,
        rend: "corps (ce qui part), caviarde, serveur, captures, incidents, peut_envoyer, pourquoi",
        refus: &[OPERATION, FORME],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        nom: "signalement-envoyer",
        geste: "Écrire au développeur (un problème, une idée) ; le message part seul.",
        charge: &[TEXTE, OU, CAPTURES],
        exemple: r#"{"texte":"La phrase de la DSN est coupée.","ecran":"Paie, novembre 2026"}"#,
        rend: "signalements (chacun son état et son départ)",
        refus: &[OPERATION, FORME],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        nom: "signalements",
        geste: "Dire les messages écrits au développeur, et ses réponses.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "signalements (chacun son état, sa pastille, ses réponses)",
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        nom: "signalements-lus",
        geste: "Marquer lues les réponses du développeur.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "lues (le nombre marqué)",
        refus: &[OPERATION],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        nom: "signalements-partir",
        geste: "Faire partir chez le développeur les messages qui attendent.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "le bilan du passage (partis, reste, erreur), phrase ; ne refuse jamais",
        refus: &[OPERATION],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: Ecran::Auto(Place::Signaler),
    },
    Fiche {
        nom: "incident-navigateur",
        geste: "Noter ce qui a cassé dans la page.",
        charge: &[Champ { nom: "message", sens: "ce que la page a vu (et genre, fichier, ligne, pile)", avance: false }],
        exemple: r#"{"message":"exemple","genre":"erreur"}"#,
        rend: "note (vrai : noté), empreinte",
        refus: &[OPERATION],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        nom: "diagnostic-etat",
        geste: "Dire ce qui est parti chez le développeur, et ce qui attend.",
        charge: &[],
        exemple: r#"{}"#,
        rend: DIAGNOSTIC_REND,
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        nom: "diagnostic-envoyer",
        geste: "Envoyer au développeur les incidents qui attendent.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "ce que diagnostic-etat rend, bilan et couleur",
        refus: &[OPERATION],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        nom: "diagnostic-regler",
        geste: "Couper l'envoi des incidents au développeur, ou le rétablir.",
        charge: &[Champ { nom: "actif", sens: "vrai : les envoyer", avance: false }],
        exemple: r#"{"actif":true}"#,
        rend: DIAGNOSTIC_REND,
        refus: &[OPERATION],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        nom: "incidents-lire",
        geste: "Lire les incidents gardés sur le poste.",
        charge: &[QUOI, Champ { nom: "en_attente", sens: "vrai : seulement ceux qui ne sont pas partis", avance: true }],
        exemple: r#"{}"#,
        rend: "phrase, phrases (une par incident), lignes (les plus récentes en dernier)",
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: true,
        phrase: 0,
        ecran: Ecran::Auto(Place::Signaler),
    },
    Fiche {
        nom: "incidents-regrouper",
        geste: "Regrouper les incidents gardés, le plus fréquent en tête.",
        charge: &[QUOI],
        exemple: r#"{}"#,
        rend: "phrase, phrases (une par paquet), paquets",
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: true,
        phrase: 0,
        ecran: Ecran::Auto(Place::Signaler),
    },
    Fiche {
        nom: "incidents-envoyer",
        geste: "Déposer chez le développeur les incidents qui attendent.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "envoyes, reste, erreur, sans_serveur, phrase ; ne refuse jamais",
        refus: &[OPERATION],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: Ecran::Auto(Place::Signaler),
    },
    Fiche {
        nom: "incidents-oublier",
        geste: "Effacer les incidents gardés, tous ou d'une nature.",
        charge: &[QUOI],
        exemple: r#"{"quoi":"logiciel"}"#,
        rend: "retires (le nombre), phrase",
        refus: &[OPERATION],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: Ecran::Auto(Place::Signaler),
    },
];
/// Une route de ce module, pour la compagnie `ctx`.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    route_de(&ctx.racine, nom, charge)
}

/// LE DOSSIER DE L'INSTALLATION QUI GARDE CE QU'ON ECRIT AU DEVELOPPEUR SANS COMPAGNIE (depuis l'accueil, ou en ligne de
/// commande sans `--racine`) : ses messages, leur depart et l'identite du poste, comme une compagnie les garde dans son
/// dossier ; aucune compagnie n'y est lue, le message part sans ses renseignements.
pub fn sans_compagnie(o: &HashMap<String, String>) -> std::path::PathBuf {
    super::accueil::installation(o).join(".accueil")
}

/// Une route de ce module, sans compagnie (`sans_compagnie`).
pub fn route_sans_compagnie(o: &HashMap<String, String>, nom: &str, charge: &Value) -> R<Value> {
    let racine = sans_compagnie(o);
    std::fs::create_dir_all(&racine).map_err(|e| Echec::from(format!("{} : {}", racine.display(), e)))?;
    route_de(&racine, nom, charge)
}

fn route_de(racine: &Path, nom: &str, charge: &Value) -> R<Value> {
    let mut v = route_seule(racine, nom, charge)?;
    poser_les_pastilles(nom, &mut v);
    let n = |k: &str| v.get(k).and_then(Value::as_u64).unwrap_or(0);
    let message = |l: Option<&Value>| l.map(|s| format!("{} : {}", valeur_en_texte(&s["debut"]), valeur_en_texte(&s["pastille"]["mots"]))).unwrap_or_default();
    let phrase = match nom {
        "diagnostic-etat" | "diagnostic-envoyer" | "diagnostic-regler" => format!(
            "Signalement des dysfonctionnements : {} ; incidents gardés : {}, partis : {}, en attente : {}.",
            if v["envoi"] == json!(false) { "désactivé" } else { "actif" },
            n("total"),
            n("partis"),
            n("en_attente")
        ),
        "incident-navigateur" => if v["note"] == json!(true) { "Incident noté." } else { "Rien n'est noté." }.to_string(),
        "signalement-apercu" if v["peut_envoyer"] == json!(true) => format!("Ce message partirait chez {}, avec {} incidents et {} captures.", valeur_en_texte(&v["serveur"]), n("incidents"), n("captures")),
        "signalement-apercu" => valeur_en_texte(&v["pourquoi"]),
        "signalement-envoyer" => {
            let s = v["signalements"].as_array().and_then(|l| l.iter().find(|s| s["id"] == v["id"]));
            format!("{} {}", if v["deja"] == json!(true) { "Ce message est déjà enregistré." } else { "Message enregistré pour le développeur." }, s.map(|s| valeur_en_texte(&s["depart"]["mots"])).unwrap_or_default()).trim().to_string()
        }
        "signalements" => format!("Messages au développeur : {} ; réponses non lues : {}.", v["signalements"].as_array().map_or(0, Vec::len), n("non_lues")),
        "signalements-lus" => format!("Réponses marquées lues : {}.", n("lues")),
        _ => return Ok(v),
    };
    if nom == "signalements" {
        v["phrases"] = json!(v["signalements"].as_array().into_iter().flatten().map(|s| message(Some(s))).collect::<Vec<_>>());
    }
    Ok(crate::routes::phrase_en_tete(v, phrase))
}

/// LES PASTILLES DE CE QUE CES ROUTES RENDENT (`routes::pastille`). Le diagnostic : coupe par la compagnie, une
/// information ; rien de note ou tout parti, fait ; des incidents en attente, une information (ils partent d'eux-memes).
/// Un message a l'editeur : en cours ou pas encore parti, une information.
fn poser_les_pastilles(nom: &str, v: &mut Value) {
    let Some(o) = v.as_object_mut() else { return };
    let nombre = |o: &serde_json::Map<String, Value>, k: &str| o.get(k).and_then(Value::as_u64).unwrap_or(0);
    match nom {
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

fn route_seule(racine: &Path, nom: &str, charge: &Value) -> R<Value> {
    match nom {
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
            // La couleur du message qui le dit : un envoi impossible est refuse, sinon parti.
            let couleur = crate::routes::couleur_de_l_etat(if vrai(&bilan["erreur"]) { "refuse" } else { "fait" });
            etat.as_object_mut().expect("un objet").insert("bilan".into(), bilan);
            etat["couleur"] = json!(couleur);
            Ok(etat)
        }
        "incidents-envoyer" => Ok(avec_phrase(incidents::envoyer(racine), phrase_d_envoi)),
        // `{"actif": bool}` : couper ou rendre l'envoi des incidents a l'editeur.
        "diagnostic-regler" => {
            incidents::regler_envoi(racine, vrai(&charge["actif"]))?;
            diagnostic_etat(racine)
        }
        "incidents-lire" => {
            let (nature, en_attente) = (nature_voulue(charge), vrai(&charge["en_attente"]));
            let lignes = incidents::lire(racine, nature.as_deref(), en_attente)?;
            Ok(json!({"phrase": format!("{} incident(s) gardé(s) sur ce poste.", lignes.len()), "phrases": lignes.iter().map(phrase_d_incident).collect::<Vec<_>>(), "lignes": lignes}))
        }
        "incidents-regrouper" => {
            let paquets = incidents::regrouper(racine, nature_voulue(charge).as_deref())?;
            Ok(json!({"phrase": format!("{} sorte(s) d'incident gardée(s) sur ce poste.", paquets.len()), "phrases": paquets.iter().map(phrase_de_paquet).collect::<Vec<_>>(), "paquets": paquets}))
        }
        "incidents-oublier" => {
            let n = incidents::oublier(racine, nature_voulue(charge).as_deref());
            Ok(json!({"retires": n, "phrase": format!("{} incident(s) effacé(s) de ce poste.", n)}))
        }
        "signalement-apercu" => problemes::apercu(racine, &champ(charge, "texte"), &ou_vide(charge, "ecran"), &problemes::captures_de(charge)?),
        "signalement-envoyer" => problemes::envoyer(racine, &champ(charge, "texte"), &ou_vide(charge, "ecran"), &problemes::captures_de(charge)?),
        "signalements-lus" => Ok(json!({"lues": problemes::signalements_lus(racine)?})),
        "signalements-partir" => Ok(avec_phrase(problemes::partir(racine), phrase_d_envoi)),
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
    // Les plus récents d'abord ; tri stable.
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

/// LES PHRASES QUE L'ECRAN LIT de ces routes, sous « Un problème, une idée ? » › Avancé : ce que ce poste a envoyé au
/// développeur, et ce qui attend.
fn avec_phrase(mut v: Value, phrase: fn(&Value) -> String) -> Value {
    let p = phrase(&v);
    v["phrase"] = json!(p);
    v
}

/// Un passage d'envoi (incidents ou messages) : ce qui est parti, ce qui attend, et pourquoi.
fn phrase_d_envoi(v: &Value) -> String {
    let n = |k: &str| v.get(k).and_then(Value::as_u64).unwrap_or(0);
    let erreur = v.get("erreur").map(valeur_en_texte).unwrap_or_default();
    if v.get("sans_serveur").is_some_and(vrai) {
        return "Aucun serveur de l'éditeur n'est configuré : rien ne part.".to_string();
    }
    if v.get("occupe").is_some_and(vrai) {
        return "Un autre envoi est en cours : il prend aussi ceux-ci.".to_string();
    }
    let partis = n("envoyes").max(n("partis"));
    let mut p = format!("{} parti(s) chez le développeur ; {} en attente.", partis, n("reste"));
    if !erreur.is_empty() {
        p.push_str(&format!(" {}", erreur));
    }
    p
}

/// Un incident garde : quand, sa nature, ce qu'il dit, et s'il est parti.
fn phrase_d_incident(l: &Value) -> String {
    let t = |k: &str| l.get(k).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
    let parti = l.get("envoi").and_then(|e| e.get("etat")).and_then(Value::as_str) == Some(DEPOSE);
    format!("{} · {} · {} — {}", super::quand(&t("quand")), t("quoi"), t("resume"), if parti { "parti" } else { "pas encore parti" })
}

/// Un paquet d'incidents semblables : combien, sa nature, ce qu'il dit, le dernier.
fn phrase_de_paquet(p: &Value) -> String {
    let t = |k: &str| p.get(k).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
    format!("{} fois · {} · {} — le dernier le {}", t("combien"), t("quoi"), t("resume"), super::quand(&t("derniere")))
}
