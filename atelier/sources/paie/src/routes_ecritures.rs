//! Les routes qui ECRIVENT (`moteur/web.py`, `operations_fiches`,
//! `operations_catalogues`, `operations_questionnaire`, `operations_paies`) :
//! `activer-fiche`, `enregistrer-fiche`, `fiche-oublier-demande`,
//! `poser-identite`, `retirer-identite`, `enregistrer-saisie`, `ajuster`,
//! `metier-retenu`, `metiers-ajoutes`.
//!
//! Memes fichiers ecrits, aux memes octets (sauvegardes `avant/` comprises),
//! memes reponses que le Python. Un refus garde sa phrase quand elle est utile
//! a la page ; la classe de l'exception peut differer.
//!
//! - Une fiche refusee (`enregistrer-fiche`) porte ses anomalies : la page les
//!   montre et propose d'enregistrer quand meme (`FicheRefusee`).
//! - `ajuster` n'ecrit rien : il cherche le taux qui donne le total voulu.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::champs;
use crate::contexte::Contexte;
use crate::date::Jour;
use crate::ecriture::ecrire_json;
use crate::emplois;
use crate::engagement;
use crate::etats::{eq_py, erreur, py_str, R};
use crate::fichiers::vrai;
use crate::journal;
use crate::jumeaux::{apparier, numero_pose, Ident};
use crate::num::{dec, Decimal};
use crate::retours::Echec;
use crate::routes_accueil;
use crate::routes_contrats::sauver_avant;
use crate::routes_fiches::{self, entier, exiger};
use crate::routes_saisie::lire_saisie;

/// Les routes que ce module sert.
pub const ROUTES: [&str; 9] = [
    "activer-fiche",
    "enregistrer-fiche",
    "fiche-oublier-demande",
    "poser-identite",
    "retirer-identite",
    "enregistrer-saisie",
    "ajuster",
    "metier-retenu",
    "metiers-ajoutes",
];

/// Une route de ce module.
pub fn route(o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "activer-fiche" => activer_fiche_route(o, ctx, charge),
        "enregistrer-fiche" => enregistrer_fiche(o, ctx, charge),
        "fiche-oublier-demande" => oublier_demande(o, ctx, charge),
        "poser-identite" => poser_identite(ctx, charge),
        "retirer-identite" => retirer_identite(ctx, charge),
        "enregistrer-saisie" => enregistrer_saisie(o, ctx, charge),
        "ajuster" => ajuster(ctx, charge),
        "metier-retenu" => metier_retenu(ctx, charge),
        "metiers-ajoutes" => metiers_ajoutes(ctx, charge),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

// ------------------------------------------------------------------ outils

fn plain(m: impl Into<String>) -> Echec {
    Echec { classe: None, message: m.into() }
}

fn refus(m: impl Into<String>) -> Echec {
    erreur("ErreurOperation", m)
}

fn en_os(m: String) -> Echec {
    erreur("OSError", m)
}

fn texte_requis<'a>(charge: &'a Value, cle: &str) -> R<&'a str> {
    exiger(charge, cle)?.as_str().ok_or_else(|| erreur("TypeError", format!("`{}` : un texte est attendu", cle)))
}

fn creer_dossier(d: &Path) -> R<()> {
    std::fs::create_dir_all(d).map_err(|e| erreur("OSError", format!("{} : {}", d.display(), e)))
}

/// Un fichier de configuration : son objet, vide s'il n'existe pas.
fn objet_du_fichier(chemin: &Path) -> R<Map<String, Value>> {
    if !chemin.exists() {
        return Ok(Map::new());
    }
    match lire_saisie(chemin)? {
        Value::Object(m) => Ok(m),
        _ => Err(erreur("AttributeError", format!("{} : un objet JSON est attendu", chemin.display()))),
    }
}

fn ecrire(chemin: &Path, v: Value) -> R<()> {
    ecrire_json(chemin, &v, 2).map_err(en_os)
}

/// La configuration, relue apres l'ecriture (`web._configuration(Contexte(racine))`).
pub(crate) fn configuration(o: &HashMap<String, String>, ctx: &Contexte) -> R<Value> {
    let neuf = Contexte::nouveau(&ctx.racine, None).map_err(plain)?;
    routes_accueil::route(o, &neuf, "configuration", &json!({}))
}

/// L'heure de Paris, a la seconde (`datetime.now().isoformat(timespec="seconds")`).
fn maintenant() -> String {
    let t = crate::temps::secondes();
    crate::paris::iso_secondes(t)
}

/// Aujourd'hui, a Paris (`date.today()`).
fn aujourd_hui() -> Jour {
    Jour::lire(&maintenant()[..10]).expect("date de Paris")
}

// --------------------------------------------------------------- activer-fiche

fn fichier_de(nom: &str) -> Option<&'static str> {
    match nom {
        "entreprise" => Some("entreprise.json"),
        "salarie" => Some("salaries.json"),
        "spectacle" => Some("spectacles.json"),
        _ => None,
    }
}

/// `operations_fiches.activer_fiche` : seul `actif` change ; la copie horodatee
/// du fichier est faite avant.
pub(crate) fn activer_fiche(ctx: &Contexte, nom: &str, cle: &str, actif: bool) -> R<()> {
    let fichier = match nom {
        "salarie" | "spectacle" => fichier_de(nom).unwrap(),
        _ => return Err(refus(format!("Seuls un salarie ou un spectacle s'activent : {}.", crate::fichiers::repr(nom)))),
    };
    let chemin = ctx.racine.join("config").join(fichier);
    let mut contenu = objet_du_fichier(&chemin)?;
    match contenu.get_mut(cle) {
        Some(Value::Object(f)) => {
            f.insert("actif".into(), Value::Bool(actif));
        }
        Some(_) => return Err(erreur("TypeError", format!("la fiche {} n'est pas un objet", cle))),
        None => return Err(refus(format!("Fiche introuvable : {}.", cle))),
    }
    sauver_avant(&chemin)?;
    ecrire(&chemin, Value::Object(contenu))
}

fn activer_fiche_route(o: &HashMap<String, String>, ctx: &Contexte, charge: &Value) -> R<Value> {
    let (nom, cle) = (texte_requis(charge, "nom")?, texte_requis(charge, "cle")?);
    activer_fiche(ctx, nom, cle, charge.get("actif").is_some_and(vrai))?;
    Ok(json!({"configuration": configuration(o, ctx)?}))
}

// ----------------------------------------------------- fiche-oublier-demande

fn oublier_demande(_o: &HashMap<String, String>, ctx: &Contexte, charge: &Value) -> R<Value> {
    let code = charge.get("code").filter(|c| vrai(c)).map(py_str).unwrap_or_default();
    let lignes = crate::etats::charger(&ctx.racine)?;
    let attendues = routes_accueil::attendues(&lignes)?;
    if !attendues.iter().any(|a| a.get("code").and_then(Value::as_str) == Some(code.as_str())) {
        return Err(refus(format!("Aucune demande de fiche en attente sous le code {}.", code)));
    }
    let detail = json!({"code": code, "abandon": true});
    let note = journal::Note {
        quoi: "fiche-recue",
        mois: None,
        fichier: None,
        mode: journal::REEL,
        qui: "interface",
        accepte: true,
        destinataire: "",
        flux: "",
        reponse: "demande abandonnee, sans reponse",
        trace: None,
        detail: Some(&detail),
        quand: "",
        origine: "",
    };
    journal::noter(&ctx.racine, &note, &maintenant()).map_err(plain)?;
    // Ni NIR ni IBAN dans la reponse : la page relit `configuration` (comme apres `fiche-demander`).
    Ok(json!({"code": code}))
}

// ------------------------------------------------------------------ l'identite

const IDENTITES: [&str; 3] = ["signature", "cachet", "logo"];
const OCTETS_MAX: usize = 2 * 1024 * 1024;

fn identite_inconnue(quoi: &str) -> Echec {
    refus(format!("Identite inconnue : {}. Connues : cachet, logo, signature.", crate::fichiers::repr(quoi)))
}

fn dossier_identite(ctx: &Contexte) -> PathBuf {
    ctx.gabarits().join("identite")
}

/// Le fichier pose pour `quoi`, quelle que soit son extension.
fn chemin_identite(dossier: &Path, quoi: &str) -> Option<PathBuf> {
    [".png", ".jpg", ".svg"].iter().map(|e| dossier.join(format!("{}{}", quoi, e))).find(|f| f.exists())
}

/// Le format de ces octets, reconnu sur l'en-tete et non sur l'extension.
fn format_de(o: &[u8]) -> Option<&'static str> {
    if o.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        return Some("png");
    }
    if o.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some("jpeg");
    }
    let tete = &o[..o.len().min(2048)];
    let debut = tete.iter().position(|c| !c.is_ascii_whitespace() && *c != 0x0b).unwrap_or(tete.len());
    let tete = &tete[debut..];
    if tete.starts_with(b"<?xml") || tete.starts_with(b"<svg") || tete.starts_with(b"<!--") {
        let long = o[..o.len().min(8192)].to_ascii_lowercase();
        if long.windows(4).any(|w| w == b"<svg") {
            return Some("svg");
        }
    }
    None
}

fn avant(f: &Path) -> R<()> {
    let mut nom = f.as_os_str().to_owned();
    nom.push(".avant");
    std::fs::rename(f, PathBuf::from(nom)).map_err(|e| en_os(format!("{} : {}", f.display(), e)))
}

fn poser_identite(ctx: &Contexte, charge: &Value) -> R<Value> {
    let quoi = texte_requis(charge, "quoi")?;
    let contenu = match exiger(charge, "contenu")? {
        Value::Null => "",
        Value::String(s) => s.as_str(),
        _ => return Err(erreur("TypeError", "`contenu` : un texte est attendu")),
    };
    if !IDENTITES.contains(&quoi) {
        return Err(identite_inconnue(quoi));
    }
    let utile = contenu.rsplit(',').next().unwrap_or("");
    let octets = crate::problemes::base64_strict(utile).ok_or_else(|| refus("Le fichier n'est pas lisible : base64 invalide."))?;
    if octets.is_empty() {
        return Err(refus("Le fichier est vide."));
    }
    if octets.len() > OCTETS_MAX {
        return Err(refus(format!("Le fichier fait {:.1} Mo ; la limite est de 2 Mo.", octets.len() as f64 / 1024.0 / 1024.0)));
    }
    let format = format_de(&octets).ok_or_else(|| {
        refus("Ce fichier n'est ni un PNG, ni un JPEG, ni un SVG (ses premiers octets ne sont ceux d'aucun des trois). Rien n'a ete ecrit.")
    })?;
    let dossier = dossier_identite(ctx);
    creer_dossier(&dossier)?;
    // Les autres extensions s'en vont : une identite, un fichier.
    if let Some(ancien) = chemin_identite(&dossier, quoi) {
        avant(&ancien)?;
    }
    let extension = match format {
        "png" => ".png",
        "jpeg" => ".jpg",
        _ => ".svg",
    };
    let cible = dossier.join(format!("{}{}", quoi, extension));
    std::fs::write(&cible, &octets).map_err(|e| en_os(format!("{} : {}", cible.display(), e)))?;
    if quoi == "signature" {
        // Pour qui elle est posee : la personne qui signe CE JOUR-LA.
        let registre = dossier.join("identite.json");
        let mut tenu = objet_du_fichier(&registre)?;
        let signataire = routes_accueil::signataire_de(&ctx.entreprise)?;
        tenu.insert("signature".into(), json!({"de": signataire["nom"], "le": aujourd_hui().iso()}));
        ecrire(&registre, Value::Object(tenu))?;
    }
    identite(ctx)
}

fn retirer_identite(ctx: &Contexte, charge: &Value) -> R<Value> {
    let quoi = texte_requis(charge, "quoi")?;
    if !IDENTITES.contains(&quoi) {
        return Err(identite_inconnue(quoi));
    }
    if let Some(f) = chemin_identite(&dossier_identite(ctx), quoi) {
        avant(&f)?;
    }
    identite(ctx)
}

fn identite(ctx: &Contexte) -> R<Value> {
    Ok(json!({"identite": routes_accueil::identite(ctx, &ctx.gabarits())?}))
}

// ---------------------------------------------------------------- les metiers

const QUOI_METIERS: &str = "Les metiers que cette compagnie a coches ou decoches dans Configuration > Metiers : `codes`, pris a une autre convention collective ; `retires`, decoches dans la sienne. Preference d'affichage : aucun calcul ne change. Le catalogue est livre avec le logiciel (donnees/emplois-base.json).";

fn ecrire_preferences(ctx: &Contexte, ajoutes: &[String], retires: &[String]) -> R<()> {
    let p = ctx.racine.join("config").join("metiers-ajoutes.json");
    if let Some(parent) = p.parent() {
        creer_dossier(parent)?;
    }
    if p.exists() {
        sauver_avant(&p)?;
    }
    ecrire(&p, json!({"_quoi": QUOI_METIERS, "codes": ajoutes, "retires": retires}))
}

fn emplois_rendus(ctx: &Contexte) -> R<Value> {
    Ok(json!({"emplois": routes_accueil::metiers(ctx)?, "emplois_autres": routes_accueil::metiers_autres_conventions(ctx)?}))
}

fn metier_retenu(ctx: &Contexte, charge: &Value) -> R<Value> {
    let code = texte_requis(charge, "code")?;
    let retenu = charge.get("retenu").is_some_and(vrai);
    let catalogue = emplois::catalogue().map_err(plain)?;
    let Some(m) = catalogue.iter().find(|m| m.get("code").and_then(Value::as_str) == Some(code)) else {
        return Err(refus(format!("Metier hors catalogue : {}.", code)));
    };
    // Ceux de sa convention le sont d'office ; sans IDCC, rien ne se cache.
    let idcc = ctx.entreprise.get("idcc").filter(|v| vrai(v)).map(py_str).unwrap_or_default();
    let conventions = m.get("conventions").and_then(Value::as_array).cloned().unwrap_or_default();
    let de_la_convention = conventions.is_empty() || idcc.is_empty() || conventions.iter().any(|c| c.as_str() == Some(idcc.as_str()));
    let (mut ajoutes, mut retires) = routes_accueil::preferences_metiers(&ctx.racine)?;
    ajoutes.retain(|c| c != code);
    retires.retain(|c| c != code);
    if retenu && !de_la_convention {
        ajoutes.push(code.to_string());
    }
    if !retenu && de_la_convention {
        retires.push(code.to_string());
    }
    ecrire_preferences(ctx, &ajoutes, &retires)?;
    emplois_rendus(ctx)
}

fn metiers_ajoutes(ctx: &Contexte, charge: &Value) -> R<Value> {
    let codes: Vec<Value> = match exiger(charge, "codes")? {
        Value::Array(a) => a.clone(),
        _ => return Err(erreur("TypeError", "`codes` : une liste est attendue")),
    };
    let retirer = charge.get("retirer").is_some_and(vrai);
    let connus: HashSet<String> = emplois::catalogue()
        .map_err(plain)?
        .iter()
        .filter_map(|m| m.get("code").and_then(Value::as_str).map(str::to_string))
        .collect();
    let inconnus: Vec<String> = codes.iter().filter(|c| !c.as_str().is_some_and(|s| connus.contains(s))).map(py_str).collect();
    if !inconnus.is_empty() {
        return Err(refus(format!("Metier(s) hors catalogue : {}.", inconnus.join(", "))));
    }
    let codes: Vec<String> = codes.iter().filter_map(|c| c.as_str().map(str::to_string)).collect();
    let (avant, retires) = routes_accueil::preferences_metiers(&ctx.racine)?;
    let apres: Vec<String> = if retirer {
        avant.into_iter().filter(|c| !codes.contains(c)).collect()
    } else {
        let mut v = avant;
        for c in codes {
            if !v.contains(&c) {
                v.push(c);
            }
        }
        v
    };
    ecrire_preferences(ctx, &apres, &retires)?;
    emplois_rendus(ctx)
}

// ------------------------------------------------------------- enregistrer-fiche

/// Le prochain matricule : le plus grand code salarie du fichier, plus un.
fn matricule_suivant(salaries: &Map<String, Value>) -> i64 {
    let codes: Vec<i64> = salaries
        .iter()
        .filter(|(k, _)| !k.starts_with('_'))
        .filter_map(|(_, s)| {
            let s = s.as_object()?;
            let texte = s.get("code_salarie").map_or_else(String::new, py_str);
            entier(&Value::String(texte.trim().to_string())).ok()
        })
        .collect();
    codes.into_iter().max().map_or(1, |m| m + 1)
}

/// La cle d'une fiche neuve, lisible et unique dans son fichier.
fn cle_neuve(nom: &str, donnees: &Map<String, Value>, existants: &Map<String, Value>) -> R<String> {
    let propre = |texte: &str, bas: bool| -> String {
        let t = champs::sans_accents(texte).replace(['\'', '\u{2019}'], "");
        let t: String = t.chars().map(|c| if crate::etats::alnum_py(c) { c } else { '-' }).collect();
        let t = t.split('-').filter(|m| !m.is_empty()).collect::<Vec<_>>().join("-");
        if bas {
            t.to_lowercase()
        } else {
            t.to_uppercase()
        }
    };
    let texte = |k: &str| donnees.get(k).filter(|v| vrai(v)).map(py_str).unwrap_or_default();
    let ou = |a: &str, b: &str| {
        let x = texte(a);
        if x.is_empty() {
            texte(b)
        } else {
            x
        }
    };
    let candidats: Vec<String> = if nom == "salarie" {
        let base = propre(&ou("nom", "nom_affichage"), false);
        let prenom = propre(&ou("prenom", "prenom_affichage"), false);
        vec![base.clone(), format!("{}-{}", base, prenom)]
    } else {
        let titre = texte("titre");
        let titre = titre.trim();
        let base = if (titre.is_empty() || titre.to_uppercase() == "A COMPLETER") && donnees.get("numero_objet").is_some_and(vrai) {
            format!("objet-{}", py_str(&donnees["numero_objet"]).trim())
        } else {
            propre(titre, true)
        };
        vec![base]
    };
    let candidats: Vec<String> = candidats.into_iter().filter(|c| !c.is_empty() && !c.starts_with('-')).collect();
    if candidats.is_empty() {
        return Err(refus(format!(
            "Impossible de nommer cette fiche {} : il lui faut au moins un {}.",
            nom,
            if nom == "salarie" { "nom" } else { "titre" }
        )));
    }
    if let Some(c) = candidats.iter().find(|c| !existants.contains_key(*c)) {
        return Ok(c.clone());
    }
    let dernier = candidats.last().unwrap();
    let mut n = 2;
    while existants.contains_key(&format!("{}-{}", dernier, n)) {
        n += 1;
    }
    Ok(format!("{}-{}", dernier, n))
}

/// `champs.coercer_booleens` : les cases a cocher arrivees en texte redeviennent booleennes.
fn coercer_booleens(nom: &str, donnees: &mut Map<String, Value>) -> R<()> {
    for c in champs::fiche(nom)? {
        if c.forme != "booleen" {
            continue;
        }
        let Some(v) = donnees.get(&c.cle) else { continue };
        let neuf = match v {
            Value::String(s) => match s.trim().to_lowercase().as_str() {
                "true" | "vrai" | "oui" | "1" | "on" | "coche" => Some(Value::Bool(true)),
                "false" | "faux" | "non" | "0" | "off" | "" => Some(Value::Bool(false)),
                _ => None,
            },
            Value::Null => None,
            autre => Some(Value::Bool(vrai(autre))),
        };
        if let Some(n) = neuf {
            donnees.insert(c.cle.clone(), n);
        }
    }
    Ok(())
}

/// `_dater` : fait descendre dans `historique` les valeurs declarees qui sortent,
/// avec la veille du jour d'effet.
fn dater(historique: Option<&Value>, changements: &[Value], depuis: &Value, note: &Value) -> R<Vec<Value>> {
    let mut hist: Vec<Value> = match historique {
        Some(Value::Array(a)) => a.clone(),
        _ => Vec::new(),
    };
    if changements.is_empty() {
        return Ok(hist);
    }
    let jour = if vrai(depuis) { crate::date::d(depuis).map_err(|m| erreur("ValueError", m))? } else { aujourd_hui() };
    let veille = jour.plus_jours(-1).map_err(|m| erreur("ValueError", m))?.iso();
    let note = if vrai(note) { py_str(note) } else { String::new() };
    for c in changements {
        hist.push(json!({"champ": c["champ"], "valeur": c["avant"], "jusqu_au": veille, "note": note}));
    }
    let cle = |e: &Value, k: &str| e.get(k).map(py_str).unwrap_or_default();
    hist.sort_by(|a, b| (cle(a, "champ"), cle(a, "jusqu_au")).cmp(&(cle(b, "champ"), cle(b, "jusqu_au"))));
    Ok(hist)
}

/// Ce que `echoue` dit : la fiche porte au moins une erreur, elle n'a pas ete ecrite.
fn fiche_refusee(anomalies: &[Value]) -> Echec {
    let erreurs: Vec<&Value> = anomalies.iter().filter(|a| a.get("gravite").and_then(Value::as_str) == Some("erreur")).collect();
    let phrases: Vec<String> = erreurs
        .iter()
        .map(|a| format!("{} : {}", a.get("champ").map_or_else(String::new, py_str), a.get("message").map_or_else(String::new, py_str)))
        .collect();
    let message = format!("{} erreur(s) : {}", erreurs.len(), phrases.join(" ; "));
    Echec { classe: Some("FicheRefusee"), message: json!({"message": message, "anomalies": anomalies}).to_string() }
}

/// Ce que l'ecriture d'une fiche a fait : sa cle (celle d'une fiche neuve est fabriquee ici), ce
/// qui a ete ecrit apres derivation, les anomalies, et les valeurs derivees.
pub(crate) struct FicheEcrite {
    pub(crate) cle: Option<String>,
    pub(crate) donnees: Map<String, Value>,
    pub(crate) anomalies: Vec<Value>,
    pub(crate) derives: Map<String, Value>,
}

fn enregistrer_fiche(o: &HashMap<String, String>, ctx: &Contexte, charge: &Value) -> R<Value> {
    let nom = texte_requis(charge, "nom")?;
    let cle: Option<String> = match charge.get("cle") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.clone()),
        Some(_) => return Err(erreur("TypeError", "`cle` : un texte est attendu")),
    };
    let donnees = exiger(charge, "donnees")?;
    let forcer = charge.get("forcer").is_some_and(vrai);
    let depuis = charge.get("depuis").unwrap_or(&Value::Null);
    let note = charge.get("note").unwrap_or(&Value::Null);
    let e = ecrire_fiche(ctx, nom, cle, donnees, forcer, depuis, note)?;
    let fiche = routes_accueil::vue_fiche(nom, e.cle.as_deref(), &e.donnees, e.anomalies, &e.derives)?;
    // « enregistré à HH:MM » : l'heure du moteur (`temps.rs`), pas celle du navigateur (#236 : au banc, le temps est simule).
    Ok(json!({"fiche": fiche, "configuration": configuration(o, ctx)?, "enregistre_a": maintenant()}))
}

/// `operations_fiches.enregistrer_fiche` : verifie, date ce qui change, sauvegarde l'ancien fichier et
/// ecrit. Rend ce qui a ete ecrit, sans la page : l'import s'en sert fiche par fiche.
pub(crate) fn ecrire_fiche(ctx: &Contexte, nom: &str, cle: Option<String>, donnees: &Value, forcer: bool, depuis: &Value, note: &Value) -> R<FicheEcrite> {
    let mut cle = cle;
    let Some(fichier) = fichier_de(nom) else {
        return Err(refus(format!("Fiche inconnue : {}", crate::fichiers::repr(nom))));
    };
    if !donnees.is_object() {
        return Err(erreur("TypeError", "`donnees` : un objet est attendu"));
    }
    // UN HISTORIQUE QUI N'EST PAS UNE LISTE SE REFUSE (Leandre, 05/10/2026) : il remplacerait celui de
    // la fiche, et l'ignorer effacerait en silence les valeurs datees. Le Python l'ecrivait corrompu.
    if donnees.get("historique").is_some_and(|h| !h.is_array()) {
        return Err(refus("L'historique de la fiche doit etre une liste : rien n'a ete enregistre."));
    }
    let vide = Value::Object(Map::new());
    let ancienne: &Value = if nom == "entreprise" {
        &ctx.entreprise
    } else {
        let magasin = if nom == "salarie" { &ctx.salaries } else { &ctx.spectacles };
        cle.as_deref().and_then(|k| magasin.get(k)).unwrap_or(&vide)
    };
    // Les notes internes que l'ecran n'a pas vues sont rendues a la fiche AVANT tout le reste.
    let Value::Object(mut donnees) = routes_fiches::garder_notes_internes(ancienne, donnees) else { unreachable!() };
    let changements = routes_fiches::changements(ctx, nom, ancienne, donnees.clone())?;
    coercer_booleens(nom, &mut donnees)?;

    // LE MATRICULE D'UN SALARIE SANS NIR NE SE CHANGE PAS : il finit son numero technique temporaire.
    if nom == "salarie" && cle.as_deref().is_some_and(|k| !k.is_empty()) {
        let (avant, apres) = (ancienne.get("code_salarie"), donnees.get("code_salarie"));
        if let (Some(a), Some(b)) = (avant.filter(|v| vrai(v)), apres.filter(|v| vrai(v))) {
            let nir = donnees.get("nir").is_some_and(vrai) || ancienne.get("nir").is_some_and(vrai);
            if py_str(a) != py_str(b) && !nir {
                return Err(refus(format!(
                    "Le matricule de cette fiche passerait de {} a {}, et le salarie n'a pas de NIR : il est declare sous un numero \
                     technique temporaire qui FINIT par son matricule. Le changer ferait apparaitre un deuxieme individu dans la DSN, \
                     que rien ne rattacherait au premier. Saisir d'abord le NIR notifie par la caisse.",
                    py_str(a),
                    py_str(b)
                )));
            }
        }
    }
    // UN NIR, UNE PERSONNE : `forcer` ne leve pas ce refus.
    if nom == "salarie" {
        let nir = donnees.get("nir").filter(|v| vrai(v)).map(py_str).unwrap_or_default();
        let nir = nir.trim();
        if !nir.is_empty() {
            let autres: Vec<&str> = ctx
                .salaries
                .iter()
                .filter(|(k, s)| Some(k.as_str()) != cle.as_deref() && s.get("nir").filter(|v| vrai(v)).map(py_str).unwrap_or_default().trim() == nir)
                .map(|(k, _)| k.as_str())
                .collect();
            if !autres.is_empty() {
                return Err(refus(format!(
                    "Le NIR {} est deja celui de la fiche {}. Un NIR designe une personne et une seule : completer ou reactiver cette \
                     fiche-la plutot que d'en creer une seconde.",
                    nir,
                    autres.join(", ")
                )));
            }
        }
    }
    let chemin = ctx.racine.join("config").join(fichier);
    if cle.as_deref().map_or(true, str::is_empty) && nom == "salarie" {
        // Le matricule d'une fiche neuve : le suivant. Un matricule saisi est garde tel quel.
        if !donnees.get("code_salarie").filter(|v| vrai(v)).map(py_str).unwrap_or_default().trim().is_empty() {
        } else {
            let existants = objet_du_fichier(&chemin)?;
            donnees.insert("code_salarie".into(), Value::String(matricule_suivant(&existants).to_string()));
        }
        // L'ANCIENNETE PART DU JOUR DE LA FICHE ; une date saisie est gardee telle quelle.
        if donnees.get("date_anciennete").filter(|v| vrai(v)).map(py_str).unwrap_or_default().trim().is_empty() {
            donnees.insert("date_anciennete".into(), Value::String(ctx.aujourdhui.iso()));
        }
    }
    let derives = routes_fiches::deriver(nom, &mut donnees, true)?;

    let anomalies = routes_fiches::verifier(nom, &donnees)?;
    if !forcer && anomalies.iter().any(|a| a.get("gravite").and_then(Value::as_str) == Some("erreur")) {
        return Err(fiche_refusee(&anomalies));
    }

    let mut contenu = objet_du_fichier(&chemin)?;
    let ecrite: &mut Value = if nom == "entreprise" {
        contenu.extend(donnees.clone());
        // La fiche entreprise est le fichier entier.
        let hist = dater(contenu.get("historique"), &changements, depuis, note)?;
        poser_historique(&mut contenu, hist);
        return finir_fiche(None, &chemin, contenu, donnees, anomalies, derives);
    } else {
        if cle.as_deref().map_or(true, str::is_empty) {
            // UNE FICHE NEUVE : la cle se fabrique ici et nulle part ailleurs.
            cle = Some(cle_neuve(nom, &donnees, &contenu)?);
        }
        let k = cle.clone().unwrap();
        let mut fusion = match contenu.get(&k) {
            Some(Value::Object(m)) => m.clone(),
            _ => Map::new(),
        };
        fusion.extend(donnees.clone());
        contenu.insert(k.clone(), Value::Object(fusion));
        contenu.get_mut(&k).unwrap()
    };
    let Value::Object(fiche) = ecrite else { unreachable!() };
    let hist = dater(fiche.get("historique"), &changements, depuis, note)?;
    poser_historique(fiche, hist);
    finir_fiche(cle, &chemin, contenu, donnees, anomalies, derives)
}

/// L'IMPORT COMPLETE UNE FICHE QUI EXISTE (lot I2, `importation_completer`) : chaque champ de `champs` va dans la fiche
/// telle qu'elle est sur le disque, s'il y est vide ou « A COMPLETER » -- et seulement alors : un champ rempli depuis
/// l'apercu reste ce qu'il est. Rien ne se date (une case vide n'avait rien declare), rien d'autre ne bouge ; l'ancien
/// fichier est sauvegarde comme pour une fiche de l'ecran. Rend les champs ecrits.
pub(crate) fn completer_fiche(ctx: &Contexte, nom: &str, cle: Option<&str>, champs: &Map<String, Value>) -> R<Map<String, Value>> {
    let Some(fichier) = fichier_de(nom) else {
        return Err(refus(format!("Fiche inconnue : {}", crate::fichiers::repr(nom))));
    };
    let chemin = ctx.racine.join("config").join(fichier);
    let mut contenu = objet_du_fichier(&chemin)?;
    let cible: &mut Map<String, Value> = if nom == "entreprise" {
        &mut contenu
    } else {
        match cle.and_then(|k| contenu.get_mut(k)) {
            Some(Value::Object(m)) => m,
            _ => return Err(refus(format!("La fiche {} {} n'existe plus : rien n'est complete.", nom, cle.unwrap_or("?")))),
        }
    };
    let mut ecrits = Map::new();
    for (k, v) in champs {
        if crate::importation_completer::vide(cible.get(k)) {
            cible.insert(k.clone(), v.clone());
            ecrits.insert(k.clone(), v.clone());
        }
    }
    if !ecrits.is_empty() {
        sauver_avant(&chemin)?;
        ecrire(&chemin, Value::Object(contenu))?;
    }
    Ok(ecrits)
}

/// `fiche_ecrite["historique"] = hist`, ou la cle retiree quand il n'y a rien.
fn poser_historique(fiche: &mut Map<String, Value>, hist: Vec<Value>) {
    if hist.is_empty() {
        fiche.shift_remove("historique");
    } else {
        fiche.insert("historique".into(), Value::Array(hist));
    }
}

fn finir_fiche(
    cle: Option<String>,
    chemin: &Path,
    contenu: Map<String, Value>,
    donnees: Map<String, Value>,
    anomalies: Vec<Value>,
    derives: Map<String, Value>,
) -> R<FicheEcrite> {
    sauver_avant(chemin)?;
    ecrire(chemin, Value::Object(contenu))?;
    Ok(FicheEcrite { cle, donnees, anomalies, derives })
}

// ------------------------------------------------------------ enregistrer-saisie

/// `refuser_si_clos` : un exercice cloture ne bouge plus.
pub(crate) fn refuser_si_clos(ctx: &Contexte, mois: &str) -> R<()> {
    let annee: String = mois.chars().take(4).collect();
    if let Some(clos) = crate::routes_ouverture::exercice_clos(ctx, &annee)?.filter(vrai) {
        let le: String = clos.get("le").filter(|v| vrai(v)).map_or_else(|| "?".to_string(), py_str).chars().take(10).collect();
        return Err(refus(format!(
            "L'exercice {} est cloture depuis le {} : {} ne se modifie plus. Rouvrir l'exercice (Historique > L'exercice, ou \
             `paie cloture {} --rouvrir`) avant de toucher a ce mois.",
            annee, le, mois, annee
        )));
    }
    Ok(())
}

/// UN MOIS TENU AVANT CE LOGICIEL NE SE SAISIT PAS (ticket #215) : ses paies ont ete calculees et declarees ailleurs
/// (l'historique importe les porte, `mois_importe`). Une saisie le ferait recalculer ici et proposerait d'en ecrire la
/// DSN -- une declaration deja faite ne se regenere pas (regle 4). Rien ne passe outre, pas meme `forcer`.
pub(crate) fn refuser_si_importe(ctx: &Contexte, mois: &str) -> R<()> {
    if crate::mois_importe::importe(&ctx.racine, mois)? {
        return Err(refus(format!(
            "{} a ete tenu avant PuppetPaie : ses paies ont ete faites par l'ancien logiciel et sont dans l'historique \
             importe. Il se lit dans la page Paie ; il ne se saisit pas ici.",
            mois
        )));
    }
    Ok(())
}

/// `refuser_si_hors_delai` : la DSN du mois etait due ; seul `forcer` passe outre.
pub(crate) fn refuser_si_hors_delai(ctx: &Contexte, mois: &str, forcer: bool) -> R<()> {
    // Le calcul de la limite se fait meme sous `forcer` : un mois impossible (2026-13) se refuse.
    if let Some(limite) = crate::routes_saisie::mois_hors_delai(ctx, mois)?.filter(|_| !forcer) {
        return Err(refus(format!(
            "Le mois {} ne se modifie plus : sa DSN etait due le {}, nous sommes le {}. Un mois passe ne se saisit plus et ne se \
             produit plus. Pour une DSN en retard, assumee : `paie mois paies/{}.json --forcer`.",
            mois,
            limite.jj_mm_aaaa(),
            ctx.aujourdhui.jj_mm_aaaa(),
            mois
        )));
    }
    Ok(())
}

/// Le contrat de chaque engagement : (cle du salarie, premier jour, numero POSE dans la saisie) -- l'identite de
/// `jumeaux` : deux contrats du meme salarie qui commencent le meme jour ne se confondent pas.
pub(crate) fn identifiants(ctx: &Contexte, data: &Value) -> Result<Vec<Ident>, String> {
    let lignes: engagement::LignesPermanent = &crate::conges::lignes_du_mois;
    let (engs, _) = engagement::engagements(ctx, data, lignes)?;
    let bruts = data.get("engagements").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    let mut out = Vec::new();
    for (i, e) in engs.iter().enumerate() {
        let fiche = ctx.salarie(&e.salarie)?;
        out.push((fiche.get("_cle").map_or_else(String::new, py_str), e.debut.iso(), bruts.get(i).and_then(numero_pose)));
    }
    Ok(out)
}

fn enregistrer_saisie(o: &HashMap<String, String>, ctx: &Contexte, charge: &Value) -> R<Value> {
    let mois = texte_requis(charge, "mois")?;
    let forcer = charge.get("forcer").is_some_and(vrai);
    let confirme = charge.get(crate::routes_regularisation::CONFIRMATION).is_some_and(vrai);
    let Value::Object(brute) = exiger(charge, "saisie")? else {
        return Err(erreur("TypeError", "`saisie` : un objet est attendu"));
    };
    let p = ecrire_saisie_confirmee(o, ctx, mois, brute, forcer, confirme, true)?;
    // Ce que la page dit du mois deja declare, a jour apres l'ecriture (`routes_regularisation::modification_de_la_paie`).
    let mut m = crate::routes_mois::Moteur::nouveau(o, ctx);
    let dsn = crate::routes_regularisation::modification_de_la_paie(&mut m, ctx, mois)?;
    // « Enregistré à HH:MM » : l'heure du moteur (`temps.rs`), pas celle du navigateur (#236 : au banc, le temps est simule).
    Ok(json!({"chemin": p.display().to_string(), "dsn_deposee": dsn, "enregistre_a": maintenant()}))
}

/// `operations_paies.enregistrer_saisie` : verifie la saisie en la calculant, puis l'ecrit. `fusionner` a faux
/// pour qui ecrit le fichier ENTIER en connaissance de cause (`valider_paie`). Sans confirmation : voir
/// `ecrire_saisie_confirmee`.
pub(crate) fn ecrire_saisie(o: &HashMap<String, String>, ctx: &Contexte, mois: &str, brute: &Map<String, Value>, forcer: bool, fusionner_ancien: bool) -> R<PathBuf> {
    ecrire_saisie_confirmee(o, ctx, mois, brute, forcer, false, fusionner_ancien)
}

/// LA PAIE D'UN MOIS DONT LA DSN EST DEPOSEE ET L'ANNULE-ET-REMPLACE PASSE (Leandre, 21/10/2026 : « modifier la paie
/// d'un mois dont la DSN est deposee et dont la limite de l'annule-et-remplace est passee demande une confirmation
/// explicite, avec un gros avertissement »). Une ecriture qui en change la paie (`routes_mois::empreinte_de` : ni la
/// validation ni la reouverture) se refuse sans `confirme` (`routes_regularisation::CONFIRMATION` dans la charge), sous
/// `CLASSE_A_CONFIRMER`, avec l'avertissement que la page montre ; rien n'est ecrit. Avec, elle s'ecrit -- la limite de
/// depot (`refuser_si_hors_delai`) ne la retient pas : le mois n'est pas en retard, il est declare -- et la saisie telle
/// que declaree est gardee d'abord (`garder_la_saisie_declaree`) : « Corriger ce mois » en part.
#[allow(clippy::too_many_arguments)]
pub(crate) fn ecrire_saisie_confirmee(
    o: &HashMap<String, String>, ctx: &Contexte, mois: &str, brute: &Map<String, Value>, forcer: bool, confirme: bool, fusionner_ancien: bool,
) -> R<PathBuf> {
    let mut donnees = brute.clone();
    donnees.insert("mois".into(), Value::String(mois.to_string()));
    refuser_si_importe(ctx, mois)?;
    refuser_si_clos(ctx, mois)?;
    let mut m = crate::routes_mois::Moteur::nouveau(o, ctx);
    let declaree = crate::routes_regularisation::modification_de_la_paie(&mut m, ctx, mois)?;
    let a_confirmer = declaree.get("confirmer").is_some_and(vrai);
    if !a_confirmer {
        refuser_si_hors_delai(ctx, mois, forcer)?;
    }
    let calculee = Value::Object(donnees.clone());
    // Le calcul valide la saisie : un nom de salarie ou un spectacle inconnu refuse, et rien n'est ecrit.
    let (rep, entrees) = crate::calcul_du_mois::calculer_suivi(o, &json!({"saisie": calculee}))
        .map_err(|m| Echec::from(crate::routes_ouverture::refus_du_calcul(ctx, &calculee, m)))?;
    let nouveaux = identifiants(ctx, &calculee).map_err(plain)?;

    let p = ctx.racine.join("paies").join(format!("{}.json", mois));
    if let Some(parent) = p.parent() {
        creer_dossier(parent)?;
    }
    if fusionner_ancien && p.exists() {
        // ELLE GARDE CE QUE L'ECRAN NE MONTRE PAS : `_origine`, la validation, le numero fige, la reouverture.
        // Un fichier qu'on ne sait plus lire ne doit pas empecher d'enregistrer : la copie de `avant/` le garde tel quel.
        let lu = lire_saisie(&p).ok().filter(Value::is_object);
        // L'ancien fichier se CHARGE en entier, calcul compris (`charger(ctx, p)`) : un fichier qui ne se calcule plus
        // (spectacle devenu inconnu...) n'est pas fusionne.
        let anciens = lu.as_ref().filter(|_| crate::calcul_du_mois::calculer_suivi(o, &json!({"fichier": p.display().to_string()})).is_ok());
        let anciens = anciens.and_then(|a| identifiants(ctx, a).ok());
        if let (Some(ancien), Some(anciens)) = (lu, anciens) {
            fusionner(&mut donnees, &ancien, &anciens, &nouveaux);
        }
    }
    let donnees = Value::Object(donnees);
    if a_confirmer {
        // Ce qui change LA PAIE, pas sa validation ni sa reouverture (`sans_bruit`) ; un mois sans fichier n'a rien de
        // declare a changer.
        let avant = lire_saisie(&p).ok().map(|v| crate::routes_mois::empreinte_de(&v));
        let change = avant.is_some_and(|a| a != crate::routes_mois::empreinte_de(&donnees));
        // Ce que la norme ne permet pas de declarer apres la limite (un contrat declare retire, ses dates changees) se
        // refuse, confirme ou non : accepte, il ne serait jamais declare (`routes_regularisation::refus_non_declarable`).
        if change {
            let copie = crate::routes_regularisation::chemin_saisie_declaree(ctx, mois);
            let base = lire_saisie(if copie.exists() { &copie } else { &p }).ok();
            if let Some(base) = base {
                if let Some(texte) = crate::routes_regularisation::refus_non_declarable(ctx, mois, &base, &donnees)? {
                    return Err(erreur(crate::routes_regularisation::CLASSE_NON_DECLARABLE, texte));
                }
            }
        }
        if change && !confirme {
            let texte = declaree.get("avertissement").map(py_str).unwrap_or_default();
            return Err(erreur(crate::routes_regularisation::CLASSE_A_CONFIRMER, texte));
        }
        if change {
            crate::routes_regularisation::garder_la_saisie_declaree(&mut m, ctx, mois)?;
        }
    }
    // LES DATES D'UN CONTRAT SUR PLUSIEURS MOIS (21/10/2026) : ses autres mois recoivent les memes dates
    // (`routes_regularisation::propager_les_bornes`). Chacun se verifie comme cette saisie -- calcul, mois declare a
    // confirmer -- avant que rien ne s'ecrive.
    let ancienne = if p.exists() { lire_saisie(&p).ok() } else { None };
    let autres = match &ancienne {
        Some(a) => crate::routes_regularisation::propager_les_bornes(ctx, mois, a, &donnees)?,
        None => Vec::new(),
    };
    let mut a_garder = Vec::new();
    for (autre, saisie) in &autres {
        refuser_si_importe(ctx, autre)?;
        refuser_si_clos(ctx, autre)?;
        let d = crate::routes_regularisation::modification_de_la_paie(&mut m, ctx, autre)?;
        if d.get("confirmer").is_some_and(vrai) {
            if !confirme {
                let texte = d.get("avertissement").map(py_str).unwrap_or_default();
                return Err(erreur(crate::routes_regularisation::CLASSE_A_CONFIRMER, texte));
            }
            a_garder.push(autre.clone());
        }
        crate::calcul_du_mois::calculer_suivi(o, &json!({"saisie": saisie}))
            .map_err(|msg| Echec::from(crate::routes_ouverture::refus_du_calcul(ctx, saisie, msg)))?;
    }
    for autre in &a_garder {
        crate::routes_regularisation::garder_la_saisie_declaree(&mut m, ctx, autre)?;
    }
    for (autre, saisie) in autres {
        let chemin = ctx.racine.join("paies").join(format!("{}.json", autre));
        sauver_avant(&chemin)?;
        ecrire(&chemin, saisie)?;
        crate::interface::suppression::sortir_de_la_demo(&ctx.racine, &autre).map_err(|m| erreur("OSError", m))?;
    }
    if p.exists() {
        sauver_avant(&p)?;
    }
    ecrire(&p, donnees.clone())?;
    // UN MOIS FAIT EN DEMO, retravaille dans la compagnie gardee : il cesse d'etre « de demo » (`suppression::sortir_de_la_demo`).
    crate::interface::suppression::sortir_de_la_demo(&ctx.racine, mois).map_err(|m| erreur("OSError", m))?;
    // Le calcul qui vient de valider la saisie est celui du fichier quand la fusion n'y a rien change.
    if eq_py(&donnees, &calculee) {
        crate::calcul_du_mois::ecrire_calcul(o, mois, &rep, entrees);
    }
    Ok(p)
}

/// CE QUE LE MOTEUR TIENT SEUL, repris du fichier d'avant : `_origine`, les numeros figes, et, engagement par engagement,
/// la validation (sa carte figee comprise, `calcul_du_mois::sceller`), la reouverture et le numero du contrat. Ce que
/// l'ecran envoie de `valide` et de `rouverte` ne compte pas : une paie se valide et se rouvre par `valider-paie`, jamais
/// par une saisie (une page qui renverrait une validation lue rouvrirait ou revaliderait en silence).
///
/// LE CONTRAT, PAS LE SALARIE DU JOUR : chaque engagement de l'ecran retrouve le sien dans le fichier par
/// `jumeaux::apparier` (le numero, sinon le rang parmi ceux du meme salarie et du meme jour) ; le jumeau d'un contrat
/// valide ne devient pas sa copie, et chacun garde son numero.
fn fusionner(donnees: &mut Map<String, Value>, ancien: &Value, anciens: &[Ident], nouveaux: &[Ident]) {
    if let Some(origine) = ancien.get("_origine") {
        if !donnees.contains_key("_origine") {
            donnees.insert("_origine".into(), origine.clone());
        }
    }
    if ancien.get("numeros_figes").is_some_and(vrai) && !donnees.contains_key("numeros_figes") {
        donnees.insert("numeros_figes".into(), Value::Bool(true));
    }
    let Some(Value::Array(engagements)) = donnees.get_mut("engagements") else { return };
    for e in engagements.iter_mut() {
        if let Value::Object(eng) = e {
            eng.remove("valide");
            eng.remove("rouverte");
        }
    }
    let engagements_anciens = ancien.get("engagements").and_then(Value::as_array).cloned().unwrap_or_default();
    let en_option = |l: &[Ident]| l.iter().cloned().map(Some).collect::<Vec<_>>();
    let paires = apparier(&en_option(nouveaux), &en_option(anciens));
    for (i, paire) in paires.iter().enumerate() {
        let Some(e) = (*paire).and_then(|j| engagements_anciens.get(j)).filter(|e| e.is_object()) else { continue };
        let Some(Value::Object(eng)) = engagements.get_mut(i) else { continue };
        // UNE PAIE VALIDEE NE CHANGE PAS PAR CE CHEMIN : on garde celle du fichier, telle quelle.
        if e.get("valide").is_some_and(vrai) {
            engagements[i] = e.clone();
            continue;
        }
        if !eng.get("numero").is_some_and(vrai) {
            if let Some(n) = e.get("numero").filter(|v| vrai(v)) {
                eng.insert("numero".into(), n.clone());
            }
        }
        if let Some(r) = e.get("rouverte").filter(|v| vrai(v)) {
            eng.insert("rouverte".into(), r.clone());
        }
    }
}

// ----------------------------------------------------------------------- ajuster

const TOURS: usize = 12;

fn centimes(x: Decimal) -> Decimal {
    let mut r = x.round_dp(2);
    r.rescale(2);
    r
}

/// Un taux ou un total, tel que Python l'ecrit : `float(Decimal)`.
fn flottant(x: Decimal) -> Value {
    x.to_string().parse::<f64>().ok().and_then(serde_json::Number::from_f64).map_or(Value::Null, Value::Number)
}

/// `valeur not in (None, "")`.
fn rempli(v: Option<&Value>) -> bool {
    !matches!(v, None | Some(Value::Null)) && v.and_then(Value::as_str) != Some("")
}

/// `valeur in (None, "", 0)`.
fn absent_ou_zero(v: Option<&Value>) -> bool {
    !rempli(v) || v.is_some_and(|x| crate::etats::nombre(x) == Some(0.0))
}

fn rubrique_de(ctx: &Contexte, ligne: &Value) -> Option<crate::catalogue::Rubrique> {
    let code = match ligne.get("rubrique") {
        None => "cachet",
        Some(Value::String(s)) => s.as_str(),
        Some(_) => return None,
    };
    crate::catalogue::rubrique(&ctx.racine, code).ok()
}

/// Un SALAIRE se hausse ; un defraiement, une prime, un acompte, non.
fn ligne_ajustable(ctx: &Contexte, ligne: &Value) -> bool {
    rubrique_de(ctx, ligne).is_some_and(|r| r.nature == "salaire" && r.saisie.iter().any(|s| s == "taux") && !absent_ou_zero(ligne.get("taux")))
}

fn lignes_de(e: &Value) -> Vec<Value> {
    match e.get("lignes") {
        Some(Value::Array(a)) => a.clone(),
        _ => Vec::new(),
    }
}

fn taux_vide(l: &Value) -> bool {
    matches!(l.get("taux"), Some(Value::Null)) || l.get("taux").and_then(Value::as_str) == Some("")
}

/// Une ligne de salaire SANS TAUX recoit un taux provisoire de 1 EUR.
fn poser_taux_provisoires(ctx: &Contexte, data: &mut Value, rangs: &[usize]) -> R<()> {
    for &i in rangs {
        let e = &mut data["engagements"][i];
        let lignes = lignes_de(e);
        if rempli(e.get("salaire_mensuel")) || lignes.iter().any(|l| ligne_ajustable(ctx, l)) {
            continue;
        }
        let mut sans_taux: Vec<(usize, String)> = Vec::new();
        for (k, ligne) in lignes.iter().enumerate() {
            let Some(r) = rubrique_de(ctx, ligne) else { continue };
            if r.nature == "salaire" && r.saisie.iter().any(|s| s == "taux") && absent_ou_zero(ligne.get("taux")) && rempli(ligne.get("quantite")) && !absent_ou_zero(ligne.get("quantite")) {
                sans_taux.push((k, r.libelle));
            }
        }
        if sans_taux.len() > 1 {
            let qui = e.get("salarie").filter(|v| vrai(v)).map_or_else(|| "cet engagement".to_string(), py_str);
            let noms: Vec<&str> = sans_taux.iter().map(|(_, l)| l.as_str()).collect();
            return Err(refus(format!(
                "{} : {} lignes de salaire sans taux ({}). Le montant vise ne dit pas comment le partager entre elles : saisir un taux \
                 sur chacune, meme approximatif, et l'ajustement gardera leurs proportions.",
                qui,
                sans_taux.len(),
                noms.join(", ")
            )));
        }
        if let Some((k, _)) = sans_taux.first() {
            e["lignes"][*k]["taux"] = json!(1.0);
        }
    }
    // Les AUTRES blocs du mois repartent comme la page les enverrait d'ordinaire : sans leurs lignes incompletes.
    let Some(Value::Array(tous)) = data.get_mut("engagements") else { return Ok(()) };
    for (i, e) in tous.iter_mut().enumerate() {
        let lignes = lignes_de(e);
        if rangs.contains(&i) {
            if let Some(Value::Array(ls)) = e.get_mut("lignes") {
                for l in ls.iter_mut().filter(|l| taux_vide(l)) {
                    l["taux"] = json!(0);
                }
            }
        } else if lignes.iter().any(taux_vide) {
            e["lignes"] = Value::Array(lignes.into_iter().filter(|l| !taux_vide(l)).collect());
        }
    }
    Ok(())
}

/// `Decimal(str(x)) * facteur`, au centime, en flottant.
fn mis_a_l_echelle(x: &Value, facteur: Decimal) -> R<Value> {
    let a = dec(x).map_err(|m| erreur("InvalidOperation", m))?;
    let p = a.checked_mul(facteur).ok_or_else(|| refus("Montant trop grand pour l'ajustement."))?;
    Ok(flottant(centimes(p)))
}

/// L'engagement recopie, ses salaires multiplies -- et rien d'autre. Rend aussi s'il y avait quelque chose a multiplier.
fn a_l_echelle(ctx: &Contexte, e: &Value, facteur: Decimal) -> R<(Value, bool)> {
    let mut out = e.clone();
    let mut touche = false;
    if rempli(out.get("salaire_mensuel")) {
        out["salaire_mensuel"] = mis_a_l_echelle(&out["salaire_mensuel"], facteur)?;
        touche = true;
    }
    if let Some(Value::Array(lignes)) = out.get_mut("lignes") {
        for l in lignes.iter_mut() {
            if !ligne_ajustable(ctx, l) {
                continue;
            }
            l["taux"] = mis_a_l_echelle(&l["taux"], facteur)?;
            touche = true;
        }
    }
    Ok((out, touche))
}

/// Ce qui a change, engagement par engagement.
fn ce_qui_a_change(avant: &Value, apres: &Value, index: usize) -> Value {
    let nul = Value::Null;
    let lire = |v: &'_ Value, k: &str| v.get(k).cloned().unwrap_or(Value::Null);
    let mut d = Map::new();
    d.insert("index".into(), json!(index));
    if !eq_py(&lire(apres, "salaire_mensuel"), &lire(avant, "salaire_mensuel")) {
        d.insert("salaire_mensuel".into(), lire(apres, "salaire_mensuel"));
    }
    let mut lignes = Vec::new();
    for (rang, (a, b)) in lignes_de(avant).iter().zip(lignes_de(apres).iter()).enumerate() {
        let (ta, tb) = (a.get("taux").unwrap_or(&nul), b.get("taux").unwrap_or(&nul));
        if !rempli(a.get("taux")) && !vrai(tb) {
            continue; // restee sans taux : rien a ecrire
        }
        if !eq_py(ta, tb) {
            lignes.push(json!({
                "rang": rang,
                "rubrique": b.get("rubrique").cloned().unwrap_or_else(|| json!("cachet")),
                "taux_avant": ta, "taux": tb,
            }));
        }
    }
    if !lignes.is_empty() {
        d.insert("lignes".into(), Value::Array(lignes));
    }
    Value::Object(d)
}

fn somme(rep: &Value, rangs: &[usize], champ: &str) -> R<Decimal> {
    let paies = rep.get("paies").and_then(Value::as_array).ok_or_else(|| plain("reponse du calcul illisible : `paies`"))?;
    let mut total = Decimal::new(0, 2);
    for &i in rangs {
        let v = paies.get(i).and_then(|p| p.get(champ)).ok_or_else(|| plain(format!("reponse du calcul illisible : `{}`", champ)))?;
        total += dec(v).map_err(plain)?;
    }
    Ok(total)
}

/// Le taux qu'il faudrait, pour que ces engagements coutent CE montant. N'ecrit rien.
fn ajuster(ctx: &Contexte, charge: &Value) -> R<Value> {
    let source = exiger(charge, "saisie")?;
    let engagements = exiger(charge, "engagements")?;
    let cible = texte_requis(charge, "cible")?;
    let montant = exiger(charge, "montant")?;
    let champ = match cible {
        "net" => "net_a_payer",
        "brut" => "brut",
        "cout" => "cout_employeur",
        _ => return Err(refus(format!("Cible inconnue : {}. Les trois qui se visent sont brut, cout, net.", crate::fichiers::repr(cible)))),
    };
    let vise = centimes(dec(montant).map_err(|m| erreur("InvalidOperation", m))?);
    if vise <= Decimal::ZERO {
        return Err(refus(format!("Un montant vise se dit en euros et il est positif (recu : {}).", vise)));
    }
    if !source.is_object() {
        return Err(erreur("TypeError", "`saisie` : un objet est attendu"));
    }
    let mut data = source.clone();
    let mut rangs: Vec<i64> = Vec::new();
    for i in crate::etats::items(engagements)? {
        rangs.push(entier(&i)?);
    }
    rangs.sort_unstable();
    rangs.dedup();
    let total = data.get("engagements").and_then(Value::as_array).map_or(0, Vec::len);
    if rangs.is_empty() || rangs[0] < 0 || rangs[rangs.len() - 1] >= total as i64 {
        let noms: Vec<String> = rangs.iter().map(i64::to_string).collect();
        return Err(refus(format!(
            "Engagement(s) {} : la saisie en porte {}.",
            if noms.is_empty() { "aucun".to_string() } else { noms.join(", ") },
            total
        )));
    }
    let rangs: Vec<usize> = rangs.into_iter().map(|i| i as usize).collect();
    let origine = data.clone();
    poser_taux_provisoires(ctx, &mut data, &rangs)?;

    let essai = |facteur: Decimal| -> R<(Value, Value, Decimal)> {
        let mut d = data.clone();
        let mut touche = false;
        for &i in &rangs {
            let (e, t) = a_l_echelle(ctx, &d["engagements"][i], facteur)?;
            d["engagements"][i] = e;
            touche |= t;
        }
        if !touche {
            return Err(refus(
                "Rien a ajuster : cet engagement ne porte aucune ligne de salaire avec une quantite. Un defraiement, une prime ou un \
                 acompte ne se recalcule pas depuis une enveloppe -- c'est un montant, pas un taux. Poser d'abord une ligne de paie \
                 (cachet, service, heure, journee...) avec sa quantite ; le taux, lui, peut rester vide.",
            ));
        }
        let rep = crate::service::calculer_avec(ctx, &json!({"saisie": d}))
            .map_err(|m| Echec::from(crate::routes_ouverture::refus_du_calcul(ctx, &d, m)))?;
        let atteint = somme(&rep, &rangs, champ)?;
        Ok((d, rep, atteint))
    };

    let (_, _, depart) = essai(Decimal::ONE)?;
    if depart <= Decimal::ZERO {
        return Err(refus(format!(
            "Cet engagement paie {} EUR : il n'y a pas de taux a faire varier. Poser d'abord une quantite et un taux, puis viser le total.",
            centimes(depart)
        )));
    }
    // On ne divise pas, on approche : on garde le MEILLEUR essai, jamais le dernier ; a egale distance,
    // celui qui NE DEPASSE PAS gagne.
    let rang_essai = |atteint: Decimal| ((atteint - vise).abs(), atteint > vise);
    let mut facteur = (vise / depart).round_dp(20);
    let mut vus = vec![facteur];
    let mut meilleur: Option<(Value, Value, Decimal, Decimal, usize)> = None;
    for tour in 1..=TOURS {
        let (d, rep, atteint) = essai(facteur)?;
        if meilleur.as_ref().map_or(true, |m| rang_essai(atteint) < rang_essai(m.2)) {
            meilleur = Some((d, rep, atteint, facteur, tour));
        }
        if atteint == vise || atteint <= Decimal::ZERO {
            break;
        }
        let suivant = facteur.checked_mul(vise).and_then(|x| x.checked_div(atteint)).ok_or_else(|| refus("Ajustement impossible : montants trop grands."))?.round_dp(20);
        if vus.contains(&suivant) {
            break;
        }
        vus.push(suivant);
        facteur = suivant;
    }
    let (d, rep, atteint, facteur, tours) = meilleur.expect("au moins un essai");

    // Les paies de ces engagements, telles que la page les montre.
    let engs = crate::routes_ouverture::engagements_calcules(ctx, &d, &rep).map_err(Echec::from)?;
    let paies_json = rep.get("paies").and_then(Value::as_array).ok_or_else(|| plain("reponse du calcul illisible : `paies`"))?;
    let mut vues = Vec::new();
    for (i, e) in engs.into_iter().enumerate() {
        if !rangs.contains(&i) {
            continue;
        }
        let p = crate::routes_ouverture::paie_de_json(ctx, e, &paies_json[i]).map_err(Echec::from)?;
        vues.push(crate::routes_ouverture::vue_paie(ctx, &p, &d["engagements"][i], &d, false).map_err(Echec::from)?);
    }
    let atteint = centimes(atteint);
    let mut f = facteur.round_dp(6);
    f.rescale(6);
    Ok(json!({
        "cible": cible,
        "vise": vise.to_string(), "atteint": atteint.to_string(), "depart": centimes(depart).to_string(),
        "ecart": (atteint - vise).to_string(),
        "facteur": f.to_string(), "tours": tours,
        "engagements": rangs.iter().map(|&i| ce_qui_a_change(&origine["engagements"][i], &d["engagements"][i], i)).collect::<Vec<_>>(),
        "paies": vues,
    }))
}
