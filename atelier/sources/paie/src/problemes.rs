//! Le bouton « Un probleme, une idee ? » (`moteur/operations_filet.py`,
//! `moteur/web.py::_captures_de`) : l'apercu de ce qui partira, l'envoi chez
//! l'editeur, la boite des signalements faits depuis ce poste, la releve des
//! reponses de l'editeur et leur lecture.
//!
//! Ce qui part : le texte (caviarde), la version, l'ecran, la machine, les dix
//! derniers incidents (chemin du dossier et adresses retires) et cinq captures
//! PNG ou JPEG au plus. `prive/journal/signalements.json` garde ce que ce poste
//! a signale, du plus ancien au plus recent ; la page le lit du plus recent au
//! plus ancien.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::ecriture::ecrire_json;
use crate::etats::{eq_py, erreur, get, items, nom_type, py_str, R};
use crate::fichiers::vrai;
use crate::incidents::{self, Canal};
use crate::verrou::Verrou;

const TEXTE_MAX: usize = 5000;
/// 1,5 Mo decodes.
const CAPTURE_MAX: usize = 1536 * 1024;
const CAPTURE_TYPES: [&str; 2] = ["image/png", "image/jpeg"];
const CAPTURES_MAX: usize = 5;
const INCIDENTS_JOINTS: usize = 10;
const SIGNALEMENTS_FICHIER: &str = "signalements.json";

fn tete(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn valeur_erreur(m: &str) -> crate::retours::Echec {
    erreur("ValueError", m)
}

/// `base64.b64decode(s, validate=True)` : seulement des caracteres de
/// l'alphabet, le remplissage qu'il faut et rien apres.
pub fn base64_strict(t: &str) -> Option<Vec<u8>> {
    let o = t.as_bytes();
    let utile = o.iter().position(|&c| c == b'=').unwrap_or(o.len());
    let (donnees, reste) = o.split_at(utile);
    let valeur = |c: u8| -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a') as u32 + 26),
            b'0'..=b'9' => Some((c - b'0') as u32 + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    let n = donnees.len();
    // `binascii.a2b_base64` (non strict) apres la regex de `validate=True` : au plus
    // deux `=`, ignores quand les donnees finissent sur un bloc complet ; sinon ils
    // doivent completer le bloc.
    if reste.len() > 2 || !reste.iter().all(|&c| c == b'=') {
        return None;
    }
    match n % 4 {
        0 => {}
        1 => return None,
        2 if reste.len() < 2 => return None,
        3 if reste.len() != 1 => return None,
        _ => {}
    }
    let mut sortie = Vec::with_capacity(n / 4 * 3 + 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    for &c in donnees {
        acc = (acc << 6) | valeur(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            sortie.push((acc >> bits & 0xff) as u8);
        }
    }
    Some(sortie)
}

/// `web._captures_de` : les captures de la page, `donnees` avec ou sans le
/// prefixe `data:image/...;base64,`.
pub fn captures_de(charge: &Value) -> R<Vec<Value>> {
    let vide = Value::Array(Vec::new());
    let liste = match get(charge, "captures") {
        Ok(v) if vrai(v) => v,
        Ok(_) => &vide,
        Err(_) => return Ok(Vec::new()),
    };
    let mut rendu = Vec::new();
    for c in items(liste)? {
        let Value::Object(o) = c.as_ref() else { continue };
        let donnees = match o.get("donnees") {
            Some(d) if vrai(d) => py_str(d),
            _ => String::new(),
        };
        let donnees = if donnees.starts_with("data:") { donnees.split_once(',').map(|(_, r)| r.to_string()).unwrap_or_default() } else { donnees };
        rendu.push(json!({"type": o.get("type").cloned().unwrap_or(Value::Null), "donnees": donnees}));
    }
    Ok(rendu)
}

struct Capture {
    mime: String,
    donnees: String,
    octets: usize,
}

fn capture_valide(capture: &Value) -> R<Capture> {
    let mime = capture.get("type").cloned().unwrap_or(Value::Null);
    let mime = match mime.as_str() {
        Some(m) if CAPTURE_TYPES.contains(&m) => m.to_string(),
        _ => return Err(valeur_erreur("Une capture doit être une image PNG ou JPEG.")),
    };
    let donnees = match capture.get("donnees") {
        Some(d) if vrai(d) => py_str(d),
        _ => String::new(),
    };
    let Some(brut) = base64_strict(&donnees) else { return Err(valeur_erreur("Une capture est illisible.")) };
    if brut.is_empty() {
        return Err(valeur_erreur("Une capture est vide."));
    }
    if brut.len() > CAPTURE_MAX {
        return Err(valeur_erreur("Une capture dépasse 1,5 Mo."));
    }
    Ok(Capture { mime, donnees, octets: brut.len() })
}

fn captures_valides(captures: &[Value]) -> R<Vec<Capture>> {
    let gardees: Vec<&Value> = captures.iter().filter(|c| vrai(c)).collect();
    if gardees.len() > CAPTURES_MAX {
        return Err(valeur_erreur(&format!("{} captures au plus.", CAPTURES_MAX)));
    }
    gardees.into_iter().map(capture_valide).collect()
}

/// Le corps exact qui part, et ce que le caviardage a retire.
fn corps_signalement(racine: &Path, texte: &Value, ecran: &Value, captures: &[Value]) -> R<(Value, bool, Vec<Capture>)> {
    let texte = match texte {
        Value::String(s) => s.trim_matches(|c: char| c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)).to_string(),
        t if !vrai(t) => String::new(),
        t => return Err(erreur("AttributeError", crate::fichiers::attendu("un texte", nom_type(t)))),
    };
    if texte.is_empty() {
        return Err(valeur_erreur("Écrivez votre message avant d'envoyer."));
    }
    if texte.chars().count() > TEXTE_MAX {
        return Err(valeur_erreur(&format!("Le texte dépasse {} caractères.", TEXTE_MAX)));
    }
    let caps = captures_valides(captures)?;
    let (texte, trouve) = incidents::caviarder(&texte);
    let fictive = crate::interface::compagnies::est_fictive(racine);
    let nom_du_dossier = racine.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let poste = incidents::poste(racine)?;
    let ecran_txt = if vrai(ecran) { tete(&py_str(ecran), 200) } else { String::new() };
    let lues = incidents::lire(racine, None, false)?;
    let debut = lues.len().saturating_sub(INCIDENTS_JOINTS);
    let joints: Vec<Value> = lues[debut..].iter().map(incidents::pour_l_editeur).collect::<R<_>>()?;
    let corps = json!({
        "poste": poste,
        "texte": texte,
        "version": incidents::version(),
        "contexte": {
            // `demonstration` : le nom du champ que le serveur de l'editeur attend (son contrat, `infra/filet/`) ; il dit
            // si le retour vient d'une compagnie fictive.
            "ecran": ecran_txt, "demonstration": fictive,
            "compagnie": if fictive { nom_du_dossier } else { String::new() },
            "systeme": incidents::machine(),
        },
        "incidents": joints,
        "captures": caps.iter().map(|c| json!({"type": c.mime, "donnees": c.donnees})).collect::<Vec<_>>(),
    });
    Ok((corps, !trouve.is_empty(), caps))
}

/// `signalement_apercu` : ce qui partira, au mot pres (les octets des captures
/// mis a part). Facultatif : on peut envoyer sans l'avoir regarde.
pub fn apercu(racine: &Path, texte: &Value, ecran: &Value, captures: &[Value]) -> R<Value> {
    let (corps, caviarde, caps) = corps_signalement(racine, texte, ecran, captures)?;
    let mut montre = corps.clone();
    montre["captures"] = Value::Array(
        caps.iter().map(|c| json!({"type": c.mime, "donnees": format!("({} Ko, non affichés)", c.octets.div_ceil(1024))})).collect(),
    );
    let serveur = incidents::configuration(racine)?.map(|(s, _)| s).unwrap_or_default();
    let nb_incidents = corps["incidents"].as_array().map_or(0, Vec::len);
    Ok(json!({
        "corps": montre, "caviarde": caviarde, "serveur": serveur,
        "captures": caps.len(), "incidents": nb_incidents,
        "peut_envoyer": !serveur.is_empty(),
        "pourquoi": if serveur.is_empty() { format!("Aucun serveur n'est configuré (config/{}) : rien ne peut partir.", crate::filet::FICHIER) } else { String::new() },
    }))
}

fn chemin_signalements(racine: &Path) -> PathBuf {
    incidents::chemin(racine).with_file_name(SIGNALEMENTS_FICHIER)
}

/// `signalements` : ceux faits depuis ce poste, du plus recent au plus ancien.
pub fn signalements(racine: &Path) -> Vec<Value> {
    let mut a = en_ordre(racine);
    a.reverse();
    a
}

/// Le fichier tel quel : du plus ancien au plus recent.
fn en_ordre(racine: &Path) -> Vec<Value> {
    match std::fs::read(chemin_signalements(racine)).ok().and_then(|o| String::from_utf8(o).ok()).and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
        Some(Value::Array(a)) => a,
        _ => Vec::new(),
    }
}

fn verrouiller(racine: &Path) -> R<Result<Verrou, ()>> {
    let p = chemin_signalements(racine).with_file_name(".signalements.verrou");
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| erreur("OSError", format!("{} : {}", d.display(), e)))?;
    }
    let marque = crate::verrou::marque("signalements", std::process::id(), &incidents::maintenant());
    Ok(Verrou::prendre(&p, &marque, crate::verrou::DELAI_VERROU).map_err(|m| erreur("OSError", m))?.map_err(|_| ()))
}

/// `r.get(cle) or ""` : la valeur telle que le serveur l'a donnee.
fn brut_ou_vide(v: &Value, cle: &str) -> R<Value> {
    let x = get(v, cle)?;
    Ok(if vrai(x) { x.clone() } else { json!("") })
}

fn texte_ou_vide(v: &Value, cle: &str) -> R<String> {
    let x = get(v, cle)?;
    Ok(if vrai(x) { py_str(x) } else { String::new() })
}

/// `signalement_envoyer` : envoie chez l'editeur, et garde ce qui est parti.
pub fn envoyer(racine: &Path, texte: &Value, ecran: &Value, captures: &[Value]) -> R<Value> {
    let (corps, _trouve, caps) = corps_signalement(racine, texte, ecran, captures)?;
    let client = match incidents::canal(racine)? {
        Canal::Aucun => return Err(erreur("ErreurOperation", "Aucun serveur n'est configuré : rien n'est parti.")),
        Canal::Panne(e) if e.est_filet() => return Err(erreur("ErreurOperation", format!("Le signalement n'est pas parti : {}", e.message))),
        Canal::Panne(e) => return Err(e.en_echec()),
        Canal::Pret(c) => c,
    };
    // Un seul envoi a la fois : la boite des signalements s'ecrit en entier.
    let Ok(_verrou) = verrouiller(racine)? else {
        return Err(erreur("ErreurOperation", "Un autre signalement est en cours d'envoi : reessayer dans un instant. Rien n'est parti."));
    };
    let r = match client.signaler(&corps) {
        Ok(r) => r,
        Err(e) if e.est_filet() => return Err(erreur("ErreurOperation", format!("Le signalement n'est pas parti : {}", e.message))),
        Err(e) => return Err(e.en_echec()),
    };
    let mut lignes = en_ordre(racine);
    let numero = brut_ou_vide(&r, "numero")?;
    let ticket = brut_ou_vide(&r, "ticket")?;
    lignes.push(json!({
        "quand": incidents::maintenant()[..16].to_string(),
        "numero": numero, "ticket": ticket,
        "ticket_numero": get(&r, "ticket_numero")?.clone(),
        "debut": tete(corps["texte"].as_str().unwrap_or(""), 80),
        "captures": caps.len(),
        "incidents": corps["incidents"].as_array().map_or(0, Vec::len),
    }));
    ecrire_json(&chemin_signalements(racine), &Value::Array(lignes), 2).map_err(|m| erreur("OSError", m))?;
    Ok(json!({
        "numero": numero, "ticket": ticket, "ticket_numero": get(&r, "ticket_numero")?.clone(),
        "signalements": signalements(racine),
    }))
}

/// `relever_reponses` : rend le nombre de reponses nouvelles. Rien a demander,
/// rien ne part que les numeros des signalements qui n'en ont pas encore.
pub fn relever_reponses(racine: &Path) -> R<usize> {
    let Ok(_verrou) = verrouiller(racine)? else { return Ok(0) };
    let mut lignes = en_ordre(racine);
    let mut attendent = Vec::new();
    for l in &lignes {
        let numero = get(l, "numero")?;
        if vrai(numero) && !vrai(get(l, "reponse")?) {
            attendent.push(numero.clone());
        }
    }
    if attendent.is_empty() {
        return Ok(0);
    }
    let client = match incidents::canal(racine)? {
        Canal::Aucun => return Ok(0),
        Canal::Panne(e) if e.est_filet() => return Ok(0), // la veille suivante reessaie
        Canal::Panne(e) => return Err(e.en_echec()),
        Canal::Pret(c) => c,
    };
    let debut = attendent.len().saturating_sub(200);
    let r = match client.reponses(&attendent[debut..]) {
        Ok(r) => r,
        Err(e) if e.est_filet() => return Ok(0), // la veille suivante reessaie
        Err(e) => return Err(e.en_echec()),
    };
    let retours = get(&r, "retours")?;
    let mut recues: Vec<(Value, Value)> = Vec::new();
    if vrai(retours) {
        for x in items(retours)? {
            if let Value::Object(o) = x.as_ref() {
                if o.get("reponse").is_some_and(vrai) {
                    let k = o.get("numero").cloned().unwrap_or(Value::Null);
                    match recues.iter().position(|(c, _)| eq_py(c, &k)) {
                        Some(i) => recues[i].1 = x.clone().into_owned(),
                        None => recues.push((k, x.into_owned())),
                    }
                }
            }
        }
    }
    let mut n = 0;
    for l in lignes.iter_mut() {
        let numero = get(l, "numero")?.clone();
        let Some((_, x)) = recues.iter().find(|(c, _)| eq_py(c, &numero)) else { continue };
        if vrai(get(l, "reponse")?) {
            continue;
        }
        let o = l.as_object_mut().expect("get a verifie");
        o.insert("reponse".into(), json!(tete(&py_str(&x["reponse"]), 2000)));
        o.insert("repondu_le".into(), json!(texte_ou_vide(x, "repondu_le")?));
        o.insert("lu".into(), json!(false));
        n += 1;
    }
    if n > 0 {
        ecrire_json(&chemin_signalements(racine), &Value::Array(lignes), 2).map_err(|m| erreur("OSError", m))?;
    }
    Ok(n)
}

/// `signalements_lus` : la boite est ouverte, ce qu'elle montre est lu. Rend le
/// nombre marque.
pub fn signalements_lus(racine: &Path) -> R<usize> {
    let Ok(_verrou) = verrouiller(racine)? else { return Ok(0) };
    let mut lignes = en_ordre(racine);
    let mut n = 0;
    for l in &lignes {
        if vrai(get(l, "reponse")?) && !vrai(get(l, "lu")?) {
            n += 1;
        }
    }
    for l in lignes.iter_mut() {
        if vrai(get(l, "reponse")?) {
            l.as_object_mut().expect("get a verifie").insert("lu".into(), json!(true));
        }
    }
    if n > 0 {
        ecrire_json(&chemin_signalements(racine), &Value::Array(lignes), 2).map_err(|m| erreur("OSError", m))?;
    }
    Ok(n)
}
