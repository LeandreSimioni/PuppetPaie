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
//!
//! L'ENVOI SE FAIT EN FOND (Leandre, 14/10/2026) : « Envoyer » enregistre le
//! message (`envoyer` : sa ligne `en-attente` dans la boite, son corps exact dans
//! `journal/signalements/<id>.json`) et rend la main ; `partir` le fait partir,
//! lance aussitot par l'interface et relance par la veille tant qu'il attend --
//! le modele des incidents (`en-attente`, `depose`).

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::ecriture::ecrire_json;
use crate::etats::{eq_py, erreur, get, items, nom_type, py_str, R};
use crate::fichiers::vrai;
use crate::incidents::{self, Canal, DEPOSE, EN_ATTENTE};
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

// ------------------------------------------------------------ la boite des messages

/// Au plus tant de messages par passage : chacun peut attendre le serveur 60 s, et le verrou d'envoi se tient pour
/// abandonne passe `verrou::DELAI_VERROU` (10 minutes).
const PAR_PASSAGE: usize = 8;
const DOSSIER_ATTENTE: &str = "signalements";

fn chemin_signalements(racine: &Path) -> PathBuf {
    incidents::chemin(racine).with_file_name(SIGNALEMENTS_FICHIER)
}

/// `journal/signalements/` : le corps exact de chaque message qui attend son depart, un fichier par message.
fn dossier_attente(racine: &Path) -> PathBuf {
    chemin_signalements(racine).with_file_name(DOSSIER_ATTENTE)
}

fn chemin_corps(racine: &Path, id: &str) -> PathBuf {
    dossier_attente(racine).join(format!("{}.json", id))
}

/// Ecrit en entier ou pas du tout : un fichier a cote, puis renomme. Un lecteur ne voit jamais un fichier a moitie.
fn ecrire_entier(p: &Path, v: &Value) -> R<()> {
    let mut nom = p.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    nom.push(".ecriture");
    let provisoire = p.with_file_name(nom);
    ecrire_json(&provisoire, v, 2).map_err(|m| erreur("OSError", m))?;
    std::fs::rename(&provisoire, p).map_err(|e| erreur("OSError", format!("{} : {}", p.display(), e)))
}

/// `signalements` : ceux faits depuis ce poste, du plus recent au plus ancien, chacun avec ce que la page en dit :
/// `etat` -- `arrive`, `en-cours` (il part, ou partira tout de suite) ou `pas-parti` -- et `raison`, la raison lisible
/// du dernier essai manque.
pub fn signalements(racine: &Path) -> Vec<Value> {
    let mut a: Vec<Value> = en_ordre(racine).into_iter().map(pour_la_page).collect();
    a.reverse();
    a
}

fn pour_la_page(mut l: Value) -> Value {
    let (etat, raison) = if en_attente(&l) {
        let e = l["envoi"]["erreur"].as_str().unwrap_or("").to_string();
        if e.is_empty() { ("en-cours", e) } else { ("pas-parti", e) }
    } else {
        ("arrive", String::new())
    };
    if let Value::Object(o) = &mut l {
        o.insert("etat".into(), json!(etat));
        o.insert("raison".into(), json!(raison));
    }
    l
}

/// Le fichier tel quel : du plus ancien au plus recent. Pour la lecture seulement : illisible, il ne dit rien.
fn en_ordre(racine: &Path) -> Vec<Value> {
    en_ordre_strict(racine).unwrap_or_default()
}

/// Le fichier tel quel, pour le reecrire : absent, il est vide ; illisible, on refuse -- reecrire par-dessus perdrait
/// tout ce qu'il garde.
fn en_ordre_strict(racine: &Path) -> R<Vec<Value>> {
    let p = chemin_signalements(racine);
    let octets = match std::fs::read(&p) {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(erreur("OSError", format!("{} : {}", p.display(), e))),
    };
    match String::from_utf8(octets).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
        Some(Value::Array(a)) => Ok(a),
        _ => Err(erreur("ErreurOperation", format!("{} est illisible : rien n'y est écrit, pour ne rien perdre de ce qu'il garde.", p.display()))),
    }
}

fn etat_d_envoi(l: &Value) -> Option<&str> {
    l.get("envoi").and_then(|e| e.get("etat")).and_then(Value::as_str)
}

/// Un message qui n'est pas encore arrive chez l'editeur. Une ligne sans `envoi` est arrivee : elle n'a jamais attendu.
fn en_attente(l: &Value) -> bool {
    etat_d_envoi(l) == Some(EN_ATTENTE)
}

fn id_de(l: &Value) -> Option<&str> {
    l.get("id").and_then(Value::as_str).filter(|s| !s.is_empty())
}

/// Le verrou de la boite : chaque ecriture le tient le temps de relire et de reecrire le fichier, jamais le temps d'un
/// appel au serveur. Tenu par un autre, on attend un peu (dix secondes) ; `Err(())` passe ce delai.
fn verrouiller(racine: &Path) -> R<Result<Verrou, ()>> {
    let p = chemin_signalements(racine).with_file_name(".signalements.verrou");
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| erreur("OSError", format!("{} : {}", d.display(), e)))?;
    }
    let marque = crate::verrou::marque("signalements", std::process::id(), &incidents::maintenant());
    let fin = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        match Verrou::prendre(&p, &marque, crate::verrou::DELAI_VERROU).map_err(|m| erreur("OSError", m))? {
            Ok(v) => return Ok(Ok(v)),
            Err(_) if std::time::Instant::now() >= fin => return Ok(Err(())),
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(50)),
        }
    }
}

/// Relit la boite sous son verrou, la passe a `f`, et la reecrit si `f` dit qu'elle a change. `None` : la boite est
/// restee occupee.
fn modifier<T>(racine: &Path, f: impl FnOnce(&mut Vec<Value>) -> R<(T, bool)>) -> R<Option<T>> {
    let Ok(_verrou) = verrouiller(racine)? else { return Ok(None) };
    let mut lignes = en_ordre_strict(racine)?;
    let (t, change) = f(&mut lignes)?;
    if change {
        ecrire_entier(&chemin_signalements(racine), &Value::Array(lignes))?;
    }
    Ok(Some(t))
}

/// Un identifiant de message, propre au poste : l'heure et six octets tires au hasard. Il nomme son fichier d'attente.
fn nouvel_id() -> R<String> {
    let mut alea = [0u8; 6];
    getrandom::fill(&mut alea).map_err(|e| erreur("OSError", e.to_string()))?;
    let heure: String = incidents::maintenant().chars().filter(char::is_ascii_alphanumeric).collect();
    Ok(format!("{}-{}", heure, alea.iter().map(|o| format!("{:02x}", o)).collect::<String>()))
}

/// `brut_ou_vide` : `r.get(cle) or ""`, la valeur telle que le serveur l'a donnee.
fn brut_ou_vide(v: &Value, cle: &str) -> R<Value> {
    let x = get(v, cle)?;
    Ok(if vrai(x) { x.clone() } else { json!("") })
}

fn texte_ou_vide(v: &Value, cle: &str) -> R<String> {
    let x = get(v, cle)?;
    Ok(if vrai(x) { py_str(x) } else { String::new() })
}

/// `signalement-envoyer` : ENREGISTRE le message sur le poste, et rend la main tout de suite (Leandre, 14/10/2026 : le
/// bouton ne reste plus sur « Envoi… » le temps du serveur). Rien ne part d'ici : le depart est `partir`, que l'interface
/// lance aussitot en fond et que la veille relance tant que le message attend. Le corps exact qui partira (celui de
/// l'apercu, captures comprises) est fige maintenant, dans `journal/signalements/<id>.json` ; la boite
/// (`journal/signalements.json`) en garde la ligne, `en-attente`.
pub fn envoyer(racine: &Path, texte: &Value, ecran: &Value, captures: &[Value]) -> R<Value> {
    let (corps, _trouve, caps) = corps_signalement(racine, texte, ecran, captures)?;
    let id = nouvel_id()?;
    let fichier = chemin_corps(racine, &id);
    ecrire_entier(&fichier, &corps)?;
    let ligne = json!({
        "id": id,
        "quand": incidents::maintenant()[..16].to_string(),
        "debut": tete(corps["texte"].as_str().unwrap_or(""), 80),
        "captures": caps.len(),
        "incidents": corps["incidents"].as_array().map_or(0, Vec::len),
        "envoi": {"etat": EN_ATTENTE, "quand": Value::Null, "essai": Value::Null, "erreur": ""},
    });
    let ecrit = modifier(racine, |lignes| {
        lignes.push(ligne);
        Ok(((), true))
    });
    match ecrit {
        Ok(Some(())) => {}
        Ok(None) => {
            let _ = std::fs::remove_file(&fichier);
            return Err(erreur("ErreurOperation", "La boîte des messages est occupée : réessayez dans un instant. Rien n'est enregistré."));
        }
        Err(e) => {
            let _ = std::fs::remove_file(&fichier);
            return Err(e);
        }
    }
    Ok(json!({"id": id, "signalements": signalements(racine)}))
}

/// La raison d'un essai manque, dite pour la boite : le canal de l'editeur parle du « filet DPAE », qui n'a rien a voir ici.
fn raison_lisible(message: &str) -> String {
    if let Some(r) = message.strip_prefix("Filet DPAE injoignable : ") {
        format!("le serveur de l'éditeur est injoignable ({}).", r.trim_end_matches('.'))
    } else if let Some(r) = message.strip_prefix("Filet DPAE : ") {
        format!("le serveur de l'éditeur a répondu : {}", r)
    } else {
        message.to_string()
    }
}

/// Le bilan d'un passage : `{"partis", "reste", "erreur", "occupe"}`.
fn bilan(partis: usize, reste: usize, erreur: &str, occupe: bool) -> Value {
    json!({"partis": partis, "reste": reste, "erreur": erreur, "occupe": occupe})
}

/// `signalements-partir` : fait partir chez l'editeur ce qui attend, du plus ancien au plus recent, et ne leve jamais.
/// UN SEUL PASSAGE A LA FOIS (`journal/.signalements-envoi.verrou`) : un autre en cours, celui-ci rend `occupe` -- et
/// l'autre prend aussi ce qui s'est enregistre pendant qu'il tournait. Arrive, un message passe a `depose` avec le numero
/// que le serveur lui a donne, et son fichier d'attente s'efface (les captures ne restent pas sur le poste). Une panne
/// arrete le passage : tout ce qui attend garde sa ligne, et la raison ; la veille suivante reessaie.
pub fn partir(racine: &Path) -> Value {
    match passage(racine) {
        Ok(b) => b,
        Err(e) => bilan(0, en_ordre(racine).iter().filter(|l| en_attente(l)).count(), &e.message, false),
    }
}

fn passage(racine: &Path) -> R<Value> {
    let p = chemin_signalements(racine).with_file_name(".signalements-envoi.verrou");
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| erreur("OSError", format!("{} : {}", d.display(), e)))?;
    }
    let marque = crate::verrou::marque("signalements-envoi", std::process::id(), &incidents::maintenant());
    let _envoi = match Verrou::prendre(&p, &marque, crate::verrou::DELAI_VERROU).map_err(|m| erreur("OSError", m))? {
        Ok(v) => v,
        Err(_) => return Ok(bilan(0, en_ordre(racine).iter().filter(|l| en_attente(l)).count(), "", true)),
    };
    effacer_les_partis(racine)?;
    // Ce qui attend part maintenant : la boite le dit « en cours d'envoi ».
    let essai = incidents::maintenant();
    let attendent = modifier(racine, |lignes| {
        let mut n = 0;
        for l in lignes.iter_mut().filter(|l| en_attente(l)) {
            l["envoi"]["essai"] = json!(essai);
            l["envoi"]["erreur"] = json!("");
            n += 1;
        }
        Ok((n, n > 0))
    })?;
    let Some(attendent) = attendent else { return Ok(bilan(0, 0, "La boîte des messages est occupée.", false)) };
    if attendent == 0 {
        return Ok(bilan(0, 0, "", false));
    }
    let mut client = None;
    let (mut partis, mut tentes, mut panne) = (0, Vec::<String>::new(), String::new());
    while tentes.len() < PAR_PASSAGE {
        // Le plus ancien qui attend et n'a pas ete tente : un message enregistre pendant le passage part avec lui.
        let lignes = en_ordre_strict(racine)?;
        let Some(id) = lignes.iter().filter(|l| en_attente(l)).filter_map(id_de).find(|i| !tentes.iter().any(|t| t == i)).map(str::to_string) else {
            break;
        };
        tentes.push(id.clone());
        let corps = std::fs::read(chemin_corps(racine, &id)).ok().and_then(|o| serde_json::from_slice::<Value>(&o).ok());
        let Some(corps) = corps else {
            noter_raison(racine, &[id], "le contenu de ce message n'est plus sur cet ordinateur : il ne peut pas partir.")?;
            continue;
        };
        if client.is_none() {
            match incidents::canal(racine)? {
                Canal::Aucun => {
                    panne = "aucun serveur de l'éditeur n'est configuré.".into();
                    break;
                }
                Canal::Panne(e) => {
                    panne = raison_lisible(&e.message);
                    break;
                }
                Canal::Pret(c) => client = Some(c),
            }
        }
        let c = client.as_ref().expect("pose juste au-dessus");
        match c.signaler(&corps) {
            Ok(r) => {
                marquer_arrive(racine, &id, &r)?;
                let _ = std::fs::remove_file(chemin_corps(racine, &id));
                partis += 1;
            }
            Err(e) => {
                panne = raison_lisible(&e.message);
                break;
            }
        }
    }
    // Ce qui attend encore, et que ce passage n'a pas fait partir, le dit : la panne si elle a eu lieu (elle vaut pour
    // tous), sinon qu'il partira au passage suivant.
    let reste = modifier(racine, |lignes| {
        let mut n = 0;
        let mut change = false;
        for l in lignes.iter_mut().filter(|l| en_attente(l)) {
            n += 1;
            let vide = l["envoi"]["erreur"].as_str().unwrap_or("").is_empty();
            if !panne.is_empty() || vide {
                l["envoi"]["erreur"] = json!(if panne.is_empty() { "il partira au prochain passage de la veille.".to_string() } else { format!("{} PuppetPaie le renverra de lui-même.", panne) });
                change = true;
            }
        }
        Ok((n, change))
    })?
    .unwrap_or(0);
    Ok(bilan(partis, reste, &panne, false))
}

/// La ligne `id` est arrivee chez l'editeur : `depose`, avec ce que le serveur a rendu.
fn marquer_arrive(racine: &Path, id: &str, r: &Value) -> R<()> {
    let (numero, ticket, ticket_numero) = (brut_ou_vide(r, "numero")?, brut_ou_vide(r, "ticket")?, get(r, "ticket_numero")?.clone());
    let quand = incidents::maintenant();
    let fait = modifier(racine, |lignes| {
        let mut change = false;
        for l in lignes.iter_mut().filter(|l| id_de(l) == Some(id)) {
            let essai = l["envoi"]["essai"].clone();
            l["envoi"] = json!({"etat": DEPOSE, "quand": quand, "essai": essai, "erreur": ""});
            l["numero"] = numero.clone();
            l["ticket"] = ticket.clone();
            l["ticket_numero"] = ticket_numero.clone();
            change = true;
        }
        Ok(((), change))
    })?;
    // La boite est restee occupee dix secondes : le message est parti, mais sa ligne le dirait en attente et la veille le
    // renverrait. On le dit, sans rien effacer.
    fait.ok_or_else(|| erreur("ErreurOperation", format!("Le message {} est parti, mais la boîte des messages est restée occupée : il n'a pas pu être noté.", id)))
}

fn noter_raison(racine: &Path, ids: &[String], raison: &str) -> R<()> {
    modifier(racine, |lignes| {
        let mut change = false;
        for l in lignes.iter_mut().filter(|l| en_attente(l) && id_de(l).is_some_and(|i| ids.iter().any(|x| x == i))) {
            l["envoi"]["erreur"] = json!(raison);
            change = true;
        }
        Ok(((), change))
    })?;
    Ok(())
}

/// Les fichiers d'attente dont le message n'attend plus (parti, ou que la boite ne connait pas : son enregistrement n'a
/// pas abouti, et la page l'a dit) s'effacent.
fn effacer_les_partis(racine: &Path) -> R<()> {
    let Ok(entrees) = std::fs::read_dir(dossier_attente(racine)) else { return Ok(()) };
    let lignes = en_ordre_strict(racine)?;
    for e in entrees.flatten() {
        let nom = e.file_name().to_string_lossy().into_owned();
        let Some(id) = nom.strip_suffix(".json") else { continue };
        if !lignes.iter().any(|l| en_attente(l) && id_de(l) == Some(id)) {
            let _ = std::fs::remove_file(e.path());
        }
    }
    Ok(())
}

/// `relever_reponses` : rend le nombre de reponses nouvelles. Rien a demander,
/// rien ne part que les numeros des signalements qui n'en ont pas encore.
pub fn relever_reponses(racine: &Path) -> R<usize> {
    let mut attendent = Vec::new();
    for l in &en_ordre(racine) {
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
    // La boite se relit sous son verrou apres l'appel : un message enregistre ou parti pendant ce temps reste.
    let n = modifier(racine, |lignes| {
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
        Ok((n, n > 0))
    })?;
    Ok(n.unwrap_or(0))
}

/// `signalements_lus` : la boite est ouverte, ce qu'elle montre est lu. Rend le
/// nombre marque.
pub fn signalements_lus(racine: &Path) -> R<usize> {
    let n = modifier(racine, |lignes| {
        let mut n = 0;
        for l in lignes.iter_mut() {
            if vrai(get(l, "reponse")?) {
                if !vrai(get(l, "lu")?) {
                    n += 1;
                }
                l.as_object_mut().expect("get a verifie").insert("lu".into(), json!(true));
            }
        }
        Ok((n, n > 0))
    })?;
    Ok(n.unwrap_or(0))
}
