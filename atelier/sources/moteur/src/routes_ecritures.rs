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

use declarations::champs;
use droit::contexte::Contexte;
use socle::date::Jour;
use socle::ecriture::ecrire_json;
use droit::emplois;
use socle::valeur::valeurs_egales;
use socle::valeur::valeur_en_texte;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::vrai;
use envois::journal;
use droit::jumeaux::{numero_pose, Ident};
use socle::num::{dec, Decimal};
use socle::echec::Echec;
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
        "enregistrer-saisie" => enregistrer_saisie(ctx, charge),
        "ajuster" => ajuster(ctx, charge),
        "metier-retenu" => metier_retenu(ctx, charge),
        "metiers-ajoutes" => metiers_ajoutes(ctx, charge),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

// ------------------------------------------------------------------ outils

#[track_caller]
fn plain(m: impl Into<String>) -> Echec {
    socle::echec::noter_et(None, m)
}

#[track_caller]
fn refus(m: impl Into<String>) -> Echec {
    erreur("ErreurOperation", m)
}

#[track_caller]
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
    let t = socle::temps::secondes();
    socle::paris::iso_secondes(t)
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

/// `operations_fiches.activer_fiche` : seul `actif` change, a partir d.aujourd.hui (archiver, c.est ne plus proposer a
/// partir de ce jour ; ce qui precede reste ce qu.il etait) ; la copie horodatee du fichier est faite avant.
pub(crate) fn activer_fiche(ctx: &Contexte, nom: &str, cle: &str, actif: bool) -> R<()> {
    let fichier = match nom {
        "salarie" | "spectacle" => fichier_de(nom).unwrap(),
        _ => return Err(refus(format!("Seuls un salarie ou un spectacle s'activent : {}.", socle::fichiers::repr(nom)))),
    };
    let chemin = ctx.racine.join("config").join(fichier);
    let mut contenu = objet_du_fichier(&chemin)?;
    match contenu.get_mut(cle) {
        Some(Value::Object(f)) => {
            // Une fiche sans `actif` etait active de tout temps.
            if !f.contains_key("actif") {
                droit::contexte::dater(f, "actif", Value::Bool(true), None, None);
            }
            droit::contexte::dater(f, "actif", Value::Bool(actif), Some(&ctx.aujourdhui), None);
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
    let code = charge.get("code").filter(|c| vrai(c)).map(valeur_en_texte).unwrap_or_default();
    let lignes = envois::etats::charger(&ctx.racine)?;
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
        versions: None,
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

#[track_caller]
fn identite_inconnue(quoi: &str) -> Echec {
    refus(format!("Identite inconnue : {}. Connues : cachet, logo, signature.", socle::fichiers::repr(quoi)))
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
    let idcc = ctx.entreprise.get("idcc").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
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
    let inconnus: Vec<String> = codes.iter().filter(|c| !c.as_str().is_some_and(|s| connus.contains(s))).map(valeur_en_texte).collect();
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
            let texte = s.get("code_salarie").map_or_else(String::new, valeur_en_texte);
            entier(&Value::String(texte.trim().to_string())).ok()
        })
        .collect();
    codes.into_iter().max().map_or(1, |m| m + 1)
}

/// La cle d'une fiche neuve, lisible et unique dans son fichier.
fn cle_neuve(nom: &str, donnees: &Map<String, Value>, existants: &Map<String, Value>) -> R<String> {
    let propre = |texte: &str, bas: bool| -> String {
        let t = champs::sans_accents(texte).replace(['\'', '\u{2019}'], "");
        let t: String = t.chars().map(|c| if envois::etats::lettre_ou_chiffre(c) { c } else { '-' }).collect();
        let t = t.split('-').filter(|m| !m.is_empty()).collect::<Vec<_>>().join("-");
        if bas {
            t.to_lowercase()
        } else {
            t.to_uppercase()
        }
    };
    let texte = |k: &str| donnees.get(k).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
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
            format!("objet-{}", valeur_en_texte(&donnees["numero_objet"]).trim())
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

// ------------------------------------------------ ecrire une valeur datee

fn vide(v: Option<&Value>) -> bool {
    v.map_or(true, |v| v.is_null() || v.as_str() == Some(""))
}

/// La meme valeur, a la forme pres : un nombre saisi en texte (100 et « 100 »), une personne et sa forme lisible (les
/// cles `_affichage` que l'ecran comble, `champs::combler_affichage`, ne se declarent pas).
fn meme(a: &Value, b: &Value) -> bool {
    let sans_affichage = |v: &Value| match v {
        Value::Object(o) => Value::Object(o.iter().filter(|(k, _)| !k.ends_with("_affichage")).map(|(k, v)| (k.clone(), v.clone())).collect()),
        autre => autre.clone(),
    };
    valeurs_egales(a, b)
        || (a.is_object() && valeurs_egales(&sans_affichage(a), &sans_affichage(b)))
        || (!a.is_object() && !a.is_array() && valeur_en_texte(a) == valeur_en_texte(b))
}

/// Les champs de `donnees` qui CHANGENT une valeur existante de `ancienne` (la fiche telle que l'ecran l'a montree) :
/// ceux-la demandent leur date d'effet. Remplir un champ vide n'en est pas un : c'est sa premiere valeur. Sauf le NIR qui
/// arrive sur une fiche qui existait : la premiere DSN qui le porte porte aussi le NTT sous lequel le salarie etait
/// declare (`ntt::depuis_quand_le_nir`), et le jour ou il arrive ne se devine pas.
pub(crate) fn changements_a_dater(ancienne: &Value, donnees: &Map<String, Value>) -> Vec<String> {
    let existait = ancienne.as_object().is_some_and(|o| o.keys().any(|k| !k.starts_with('_')));
    let nir_qui_arrive = |k: &str, v: &Value| existait && k == "nir" && vide(ancienne.get(k)) && !vide(Some(v));
    donnees
        .iter()
        .filter(|(k, v)| {
            !k.starts_with('_')
                && (nir_qui_arrive(k, v) || (!vide(ancienne.get(k.as_str())) && !ancienne.get(k.as_str()).is_some_and(|a| meme(a, v))))
        })
        .map(|(k, _)| k.clone())
        .collect()
}

/// La date d'effet que la charge d'une route d'ecriture donne (`depuis`, AAAA-MM-JJ), ou `None`.
pub(crate) fn depuis_de(depuis: &Value) -> R<Option<Jour>> {
    if vrai(depuis) {
        socle::date::d(depuis).map(Some).map_err(|m| erreur("ValueError", m))
    } else {
        Ok(None)
    }
}

/// POSER UN CHAMP DE LA FICHE ENTREPRISE (une route qui ne touche qu'une case : le modele de bulletin, la case des
/// permanents) : comme une fiche de l'ecran (`ecrire_datee`), l'ancien fichier sauvegarde.
pub(crate) fn poser_entreprise(ctx: &Contexte, champ: &str, valeur: Value, depuis: Option<Jour>) -> R<()> {
    let chemin = ctx.racine.join("config").join("entreprise.json");
    let mut contenu = objet_du_fichier(&chemin)?;
    let mut donnees = Map::new();
    donnees.insert(champ.to_string(), valeur);
    ecrire_datee(&mut contenu, "entreprise", &ctx.entreprise, &donnees, depuis, None, &ctx.lue_au)?;
    sauver_avant(&chemin)?;
    ecrire(&chemin, Value::Object(contenu))
}

/// ECRIRE UNE FICHE DATEE : chaque valeur de `donnees` qui differe de `ancienne` (la fiche lue a `lue_au`, celle que
/// l'ecran a montree) s'ecrit dans la fiche datee `brute` (`contexte::dater`). Ce qui change une valeur existante vaut a
/// partir de `depuis`, et sans lui se refuse : ce qui precede garde l'ancienne valeur. La premiere valeur d'un champ
/// (absent de la fiche) vaut de tout temps ; celle qui remplit un champ vide vaut depuis `depuis`, a defaut depuis la
/// valeur vide qu'elle remplit. Le NIR qui arrive : la fiche disait « sans NIR » de tout temps, le NIR vaut depuis
/// `depuis`. Les notes `_...` s'ecrivent telles quelles.
pub(crate) fn ecrire_datee(
    brute: &mut Map<String, Value>,
    nom: &str,
    ancienne: &Value,
    donnees: &Map<String, Value>,
    depuis: Option<Jour>,
    note: Option<&str>,
    lue_au: &Jour,
) -> R<()> {
    let a_dater = changements_a_dater(ancienne, donnees);
    if depuis.is_none() && !a_dater.is_empty() {
        let libelles: HashMap<String, String> = champs::fiche(nom)?.into_iter().map(|c| (c.cle, c.libelle)).collect();
        let noms: Vec<String> = a_dater.iter().map(|k| format!("« {} »", libelles.get(k).unwrap_or(k))).collect();
        return Err(refus(format!(
            "Changer {} demande sa date d'effet (« a partir du ») : ce qui precede garde l'ancienne valeur, et la date ne \
             se devine pas. Rien n'a ete enregistre.",
            noms.join(", ")
        )));
    }
    for (k, v) in donnees {
        if k.starts_with('_') {
            brute.insert(k.clone(), v.clone());
            continue;
        }
        let avant = ancienne.get(k.as_str());
        if (vide(avant) && vide(Some(v))) || avant.is_some_and(|a| meme(a, v)) {
            continue;
        }
        let nir_qui_arrive = k == "nir" && !brute.contains_key("nir") && depuis.is_some();
        let jour = if depuis.is_some() {
            depuis
        } else {
            droit::contexte::valeurs_datees(&Value::Object(brute.clone()), k)
                .map_err(plain)?
                .into_iter()
                .filter(|(j, _)| j.map_or(true, |j| j <= *lue_au))
                .last()
                .and_then(|(j, _)| j)
        };
        if nir_qui_arrive {
            droit::contexte::dater(brute, k, Value::String(String::new()), None, None);
        }
        droit::contexte::dater(brute, k, v.clone(), jour.as_ref(), note);
    }
    Ok(())
}

/// CE QUE L'ENREGISTREMENT D'UNE FICHE PASSE OUTRE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Forcer {
    /// Rien : une erreur refuse la fiche.
    Rien,
    /// Ce que l'ecran peut forcer (« Enregistrer quand meme ») : une erreur de forme (`forcable`), jamais une valeur
    /// obligatoire absente d'un salarie ou d'un spectacle, sans laquelle la DPAE et la DSN ne se font pas (#236,
    /// `champs::verifier_fiche`).
    Forme,
    /// Tout : l'import et le questionnaire du salarie posent une fiche a completer, qui le dit (« A COMPLETER »).
    Tout,
}

impl Forcer {
    /// La charge `forcer` de la route `enregistrer-fiche` : l'ecran ne force que la forme.
    pub(crate) fn depuis_l_ecran(forcer: bool) -> Forcer {
        if forcer {
            Forcer::Forme
        } else {
            Forcer::Rien
        }
    }

    /// Ces anomalies refusent-elles la fiche ?
    fn refuse(self, anomalies: &[Value]) -> bool {
        let erreurs = || anomalies.iter().filter(|a| a.get("gravite").and_then(Value::as_str) == Some("erreur"));
        match self {
            Forcer::Rien => erreurs().next().is_some(),
            Forcer::Forme => erreurs().any(|a| a.get("forcable") == Some(&Value::Bool(false))),
            Forcer::Tout => false,
        }
    }
}

/// Ce que `echoue` dit : la fiche porte au moins une erreur, elle n'a pas ete ecrite.
fn fiche_refusee(anomalies: &[Value]) -> Echec {
    let erreurs: Vec<&Value> = anomalies.iter().filter(|a| a.get("gravite").and_then(Value::as_str) == Some("erreur")).collect();
    let phrases: Vec<String> = erreurs
        .iter()
        .map(|a| format!("{} : {}", a.get("champ").map_or_else(String::new, valeur_en_texte), a.get("message").map_or_else(String::new, valeur_en_texte)))
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
    let e = ecrire_fiche(ctx, nom, cle, donnees, Forcer::depuis_l_ecran(forcer), depuis, note)?;
    let fiche = routes_accueil::vue_fiche(nom, e.cle.as_deref(), &e.donnees, e.anomalies, &e.derives)?;
    // « enregistré à HH:MM » : l'heure du moteur (`temps.rs`), pas celle du navigateur (#236 : au banc, le temps est simule).
    Ok(json!({"fiche": fiche, "configuration": configuration(o, ctx)?, "enregistre_a": maintenant()}))
}

/// `operations_fiches.enregistrer_fiche` : verifie, date ce qui change, sauvegarde l'ancien fichier et
/// ecrit. Rend ce qui a ete ecrit, sans la page : l'import s'en sert fiche par fiche.
pub(crate) fn ecrire_fiche(ctx: &Contexte, nom: &str, cle: Option<String>, donnees: &Value, forcer: Forcer, depuis: &Value, note: &Value) -> R<FicheEcrite> {
    let mut cle = cle;
    let Some(fichier) = fichier_de(nom) else {
        return Err(refus(format!("Fiche inconnue : {}", socle::fichiers::repr(nom))));
    };
    if !donnees.is_object() {
        return Err(erreur("TypeError", "`donnees` : un objet est attendu"));
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
    coercer_booleens(nom, &mut donnees)?;

    // LE MATRICULE D'UN SALARIE SANS NIR NE SE CHANGE PAS : il finit son numero technique temporaire.
    if nom == "salarie" && cle.as_deref().is_some_and(|k| !k.is_empty()) {
        let (avant, apres) = (ancienne.get("code_salarie"), donnees.get("code_salarie"));
        if let (Some(a), Some(b)) = (avant.filter(|v| vrai(v)), apres.filter(|v| vrai(v))) {
            let nir = donnees.get("nir").is_some_and(vrai) || ancienne.get("nir").is_some_and(vrai);
            if valeur_en_texte(a) != valeur_en_texte(b) && !nir {
                return Err(refus(format!(
                    "Le matricule de cette fiche passerait de {} a {}, et le salarie n'a pas de NIR : il est declare sous un numero \
                     technique temporaire qui FINIT par son matricule. Le changer ferait apparaitre un deuxieme individu dans la DSN, \
                     que rien ne rattacherait au premier. Saisir d'abord le NIR notifie par la caisse.",
                    valeur_en_texte(a),
                    valeur_en_texte(b)
                )));
            }
        }
    }
    // UN NIR, UNE PERSONNE : `forcer` ne leve pas ce refus.
    if nom == "salarie" {
        let nir = donnees.get("nir").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
        let nir = nir.trim();
        if !nir.is_empty() {
            let autres: Vec<&str> = ctx
                .salaries
                .iter()
                .filter(|(k, s)| Some(k.as_str()) != cle.as_deref() && s.get("nir").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default().trim() == nir)
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
        if !donnees.get("code_salarie").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default().trim().is_empty() {
        } else {
            let existants = objet_du_fichier(&chemin)?;
            donnees.insert("code_salarie".into(), Value::String(matricule_suivant(&existants).to_string()));
        }
        // L'ANCIENNETE PART DU JOUR DE LA FICHE ; une date saisie est gardee telle quelle.
        if donnees.get("date_anciennete").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default().trim().is_empty() {
            donnees.insert("date_anciennete".into(), Value::String(ctx.aujourdhui.iso()));
        }
    }
    let derives = routes_fiches::deriver(nom, &mut donnees, true)?;

    let anomalies = routes_fiches::verifier(nom, &donnees)?;
    if forcer.refuse(&anomalies) {
        return Err(fiche_refusee(&anomalies));
    }

    let (depuis, note) = (depuis_de(depuis)?, Some(valeur_en_texte(note)).filter(|n| vrai(note) && !n.trim().is_empty()));
    let mut contenu = objet_du_fichier(&chemin)?;
    if nom == "entreprise" {
        // La fiche entreprise est le fichier entier.
        ecrire_datee(&mut contenu, nom, ancienne, &donnees, depuis, note.as_deref(), &ctx.lue_au)?;
        return finir_fiche(None, &chemin, contenu, donnees, anomalies, derives);
    }
    if cle.as_deref().map_or(true, str::is_empty) {
        // UNE FICHE NEUVE : la cle se fabrique ici et nulle part ailleurs.
        cle = Some(cle_neuve(nom, &donnees, &contenu)?);
    }
    let k = cle.clone().unwrap();
    let mut brute = match contenu.get(&k) {
        Some(Value::Object(m)) => m.clone(),
        _ => Map::new(),
    };
    ecrire_datee(&mut brute, nom, ancienne, &donnees, depuis, note.as_deref(), &ctx.lue_au)?;
    contenu.insert(k, Value::Object(brute));
    finir_fiche(cle, &chemin, contenu, donnees, anomalies, derives)
}

/// L'IMPORT COMPLETE UNE FICHE QUI EXISTE (lot I2, `importation_completer`) : chaque champ de `champs` va dans la fiche
/// telle qu'elle est sur le disque, s'il y est vide ou « A COMPLETER » -- et seulement alors : un champ rempli depuis
/// l'apercu reste ce qu'il est. Une case vide n'avait rien declare : la valeur qui la remplit vaut depuis elle (un champ
/// absent : de tout temps) ; rien d'autre ne bouge ; l'ancien fichier est sauvegarde comme pour une fiche
/// de l'ecran. Rend les champs ecrits.
pub(crate) fn completer_fiche(ctx: &Contexte, nom: &str, cle: Option<&str>, champs: &Map<String, Value>) -> R<Map<String, Value>> {
    let Some(fichier) = fichier_de(nom) else {
        return Err(refus(format!("Fiche inconnue : {}", socle::fichiers::repr(nom))));
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
        let en_vigueur = droit::contexte::valeurs_datees(&Value::Object(cible.clone()), k)
            .map_err(plain)?
            .into_iter()
            .filter(|(j, _)| j.map_or(true, |j| j <= ctx.lue_au))
            .last()
            .map(|(j, v)| (j, v.clone()));
        if crate::importation_completer::vide(en_vigueur.as_ref().map(|(_, v)| v)) {
            droit::contexte::dater(cible, k, v.clone(), en_vigueur.and_then(|(j, _)| j).as_ref(), None);
            ecrits.insert(k.clone(), v.clone());
        }
    }
    if !ecrits.is_empty() {
        sauver_avant(&chemin)?;
        ecrire(&chemin, Value::Object(contenu))?;
    }
    Ok(ecrits)
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
        let le: String = clos.get("le").filter(|v| vrai(v)).map_or_else(|| "?".to_string(), valeur_en_texte).chars().take(10).collect();
        return Err(refus(format!(
            "L'exercice {} est cloture depuis le {} : {} ne se modifie plus. Rouvrir l'exercice (Historique > L'exercice, ou \
             `moteur cloture {} --rouvrir`) avant de toucher a ce mois.",
            annee, le, mois, annee
        )));
    }
    Ok(())
}

/// UN MOIS TENU AVANT CE LOGICIEL NE SE SAISIT PAS (ticket #215) : ses paies ont ete calculees et declarees ailleurs
/// (l'historique importe les porte, `mois_importe`). Une saisie le ferait recalculer ici et proposerait d'en ecrire la
/// DSN -- une declaration deja faite ne se regenere pas (regle 4). Rien ne passe outre.
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

/// UN MOIS D'AVANT LA PREMIERE EMBAUCHE NE SE SAISIT PAS (`routes_saisie::avant_la_premiere_embauche`) : aucune DSN n'y
/// est due.
pub(crate) fn refuser_si_avant_la_premiere_embauche(ctx: &Contexte, mois: &str) -> R<()> {
    if let Some(embauche) = crate::routes_saisie::avant_la_premiere_embauche(ctx, mois)? {
        return Err(refus(format!(
            "{} precede la premiere embauche de la compagnie ({}) : aucune DSN n'y est due, rien ne s'y saisit. Si la \
             compagnie employait deja, corriger la date de premiere embauche (Configuration › Entreprise). Rien n'est ecrit.",
            mois,
            Jour::lire(&embauche).map(|j| j.jj_mm_aaaa()).unwrap_or(embauche)
        )));
    }
    Ok(())
}

/// Le contrat de chaque engagement : (cle du salarie, premier jour, numero POSE dans la saisie) -- l'identite de
/// `jumeaux` : deux contrats du meme salarie qui commencent le meme jour ne se confondent pas.
pub(crate) fn identifiants(ctx: &Contexte, data: &Value) -> Result<Vec<Ident>, String> {
    let engs = engagement::engagements_de_la_saisie(ctx, data)?;
    let bruts = data.get("engagements").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    let mut out = Vec::new();
    for (i, e) in engs.iter().enumerate() {
        let fiche = ctx.salarie(&e.salarie)?;
        out.push((fiche.get("_cle").map_or_else(String::new, valeur_en_texte), e.debut.iso(), bruts.get(i).and_then(numero_pose)));
    }
    Ok(out)
}

fn enregistrer_saisie(ctx: &Contexte, charge: &Value) -> R<Value> {
    let mois = texte_requis(charge, "mois")?;
    let confirme = charge.get(crate::routes_regularisation::CONFIRMATION).is_some_and(vrai);
    let Value::Object(brute) = exiger(charge, "saisie")? else {
        return Err(erreur("TypeError", "`saisie` : un objet est attendu"));
    };
    let p = ecrire_saisie_confirmee(ctx, mois, brute, confirme)?;
    // Ce que la page dit du mois deja declare, a jour apres l'ecriture (`routes_regularisation::modification_de_la_paie`).
    let mut m = crate::routes_mois::Moteur::nouveau(ctx);
    let dsn = crate::routes_regularisation::modification_de_la_paie(&mut m, ctx, mois)?;
    // « Enregistré à HH:MM » : l'heure du moteur (`temps.rs`), pas celle du navigateur (#236 : au banc, le temps est simule).
    Ok(json!({"chemin": p.display().to_string(), "dsn_deposee": dsn, "enregistre_a": maintenant()}))
}

/// LA SAISIE D'UN MOIS DONT LA DSN EST DEPOSEE ET L'ANNULE-ET-REMPLACE PASSE : une ecriture qui en change la paie
/// (`routes_mois::empreinte_de` : ni la validation ni ce que le moteur tient seul) se refuse sans `confirme`
/// (`routes_regularisation::CONFIRMATION` dans la charge), sous `CLASSE_A_CONFIRMER`, avec l'avertissement que la page
/// montre ; rien n'est ecrit. Avec, elle s'ecrit ; revalide, l'engagement fait partir sa correction dans la DSN d'un
/// mois suivant (`produire_regularisations::corrections_dues`).
///
/// UNE SAISIE S'ECRIT A LA FOIS : la fenetre sert ses routes en parallele, et la page enregistre une seconde apres la
/// derniere frappe, meme si l'enregistrement d'avant n'a pas fini. Deux ecritures du meme mois passent l'une apres
/// l'autre, dans ce processus (`SAISIE_EN_COURS`).
pub(crate) fn ecrire_saisie_confirmee(ctx: &Contexte, mois: &str, brute: &Map<String, Value>, confirme: bool) -> R<PathBuf> {
    static SAISIE_EN_COURS: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _une_a_la_fois = SAISIE_EN_COURS.lock().unwrap_or_else(|e| e.into_inner());
    ecrire_saisie_seule(ctx, mois, brute, confirme)
}

/// `ecrire_saisie_confirmee`, la seule ecriture en cours.
fn ecrire_saisie_seule(ctx: &Contexte, mois: &str, brute: &Map<String, Value>, confirme: bool) -> R<PathBuf> {
    let mut donnees = brute.clone();
    donnees.insert("mois".into(), Value::String(mois.to_string()));
    let donnees = Value::Object(donnees);
    refuser_si_importe(ctx, mois)?;
    refuser_si_clos(ctx, mois)?;
    let mut m = crate::routes_mois::Moteur::nouveau(ctx);
    let declaree = crate::routes_regularisation::modification_de_la_paie(&mut m, ctx, mois)?;
    let a_confirmer = declaree.get("confirmer").is_some_and(vrai);
    refuser_si_avant_la_premiere_embauche(ctx, mois)?;
    // Le calcul valide la saisie : un nom de salarie ou un spectacle inconnu refuse, et rien n'est ecrit.
    engagement::pour_l_ecran(ctx, &donnees).map_err(|m| Echec::from(crate::routes_ouverture::refus_du_calcul(ctx, &donnees, m)))?;
    if a_confirmer {
        // Ce qui change LA PAIE (`sans_bruit`) ; un mois sans saisie n'a rien de declare a changer.
        let actuelle = engagement::stock::vue_du_mois(&ctx.racine, mois).map_err(plain)?;
        let change = actuelle.as_ref().is_some_and(|a| crate::routes_mois::empreinte_de(a) != crate::routes_mois::empreinte_de(&donnees));
        // Ce que la norme ne permet pas de declarer apres la limite (un contrat declare retire, ses dates changees) se
        // refuse, confirme ou non : accepte, il ne serait jamais declare (`routes_regularisation::refus_non_declarable`).
        if change {
            let base = crate::routes_regularisation::saisie_declaree(ctx, mois)?.or_else(|| actuelle.clone());
            if let Some(base) = base {
                if let Some(texte) = crate::routes_regularisation::refus_non_declarable(ctx, mois, &base, &donnees)? {
                    return Err(erreur(crate::routes_regularisation::CLASSE_NON_DECLARABLE, texte));
                }
            }
        }
        if change && !confirme {
            let texte = declaree.get("avertissement").map(valeur_en_texte).unwrap_or_default();
            return Err(erreur(crate::routes_regularisation::CLASSE_A_CONFIRMER, texte));
        }
    }
    engagement::stock::ecrire_vue_du_mois(ctx, mois, &donnees).map_err(|m| erreur("ErreurOperation", m))?;
    // UN MOIS FAIT EN DEMO, retravaille dans la compagnie gardee : il cesse d'etre « de demo » (`suppression::sortir_de_la_demo`).
    crate::interface::suppression::sortir_de_la_demo(&ctx.racine, mois).map_err(|m| erreur("OSError", m))?;
    Ok(engagement::stock::chemin(&ctx.racine, mois))
}

// ----------------------------------------------------------------------- ajuster

fn centimes(x: Decimal) -> Decimal {
    let mut r = x.round_dp(2);
    r.rescale(2);
    r
}

/// Une valeur saisie : ni absente, ni `null`, ni vide.
fn rempli(v: Option<&Value>) -> bool {
    !matches!(v, None | Some(Value::Null)) && v.and_then(Value::as_str) != Some("")
}

fn lignes_de(e: &Value) -> Vec<Value> {
    match e.get("lignes") {
        Some(Value::Array(a)) => a.clone(),
        _ => Vec::new(),
    }
}

/// Ce qui a change, engagement par engagement.
fn ce_qui_a_change(avant: &Value, apres: &Value, index: usize) -> Value {
    let nul = Value::Null;
    let lire = |v: &'_ Value, k: &str| v.get(k).cloned().unwrap_or(Value::Null);
    let mut d = Map::new();
    d.insert("index".into(), json!(index));
    if !valeurs_egales(&lire(apres, "salaire_mensuel"), &lire(avant, "salaire_mensuel")) {
        d.insert("salaire_mensuel".into(), lire(apres, "salaire_mensuel"));
    }
    let mut lignes = Vec::new();
    for (rang, (a, b)) in lignes_de(avant).iter().zip(lignes_de(apres).iter()).enumerate() {
        let (ta, tb) = (a.get("taux").unwrap_or(&nul), b.get("taux").unwrap_or(&nul));
        if !rempli(a.get("taux")) && !vrai(tb) {
            continue; // restee sans taux : rien a ecrire
        }
        if !valeurs_egales(ta, tb) {
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

/// Le taux qu'il faudrait, pour que ces engagements coutent CE montant. N'ecrit rien.
fn ajuster(ctx: &Contexte, charge: &Value) -> R<Value> {
    let source = exiger(charge, "saisie")?;
    let engagements = exiger(charge, "engagements")?;
    let cible = texte_requis(charge, "cible")?;
    let montant = exiger(charge, "montant")?;
    let cible_visee = match cible {
        "net" => engagement::viser::Cible::Net,
        "brut" => engagement::viser::Cible::Brut,
        "cout" => engagement::viser::Cible::Cout,
        _ => return Err(refus(format!("Cible inconnue : {}. Les trois qui se visent sont brut, cout, net.", socle::fichiers::repr(cible)))),
    };
    let vise = centimes(dec(montant).map_err(|m| erreur("InvalidOperation", m))?);
    if vise <= Decimal::ZERO {
        return Err(refus(format!("Un montant vise se dit en euros et il est positif (recu : {}).", vise)));
    }
    if !source.is_object() {
        return Err(erreur("TypeError", "`saisie` : un objet est attendu"));
    }
    let data = source.clone();
    let mut rangs: Vec<i64> = Vec::new();
    for i in envois::etats::items(engagements)? {
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
    let v = engagement::viser::viser(ctx, &data, &rangs, cible_visee, vise).map_err(|r| match r {
        engagement::viser::RefusVisee::Refus(m) => refus(m),
        engagement::viser::RefusVisee::Calcul(m) => Echec::from(crate::routes_ouverture::refus_du_calcul(ctx, &data, m)),
    })?;
    // Les paies de ces engagements, telles que la page les montre.
    let mut vues = Vec::new();
    for &i in &rangs {
        vues.push(crate::routes_ouverture::vue_paie(ctx, &v.paies[i], &v.saisie["engagements"][i], &v.saisie, false).map_err(Echec::from)?);
    }
    let atteint = centimes(v.atteint);
    let mut f = v.facteur.round_dp(6);
    f.rescale(6);
    Ok(json!({
        "cible": cible,
        "vise": vise.to_string(), "atteint": atteint.to_string(), "depart": centimes(v.depart).to_string(),
        "ecart": (atteint - vise).to_string(),
        "facteur": f.to_string(), "tours": v.tours,
        "engagements": rangs.iter().map(|&i| ce_qui_a_change(&data["engagements"][i], &v.saisie["engagements"][i], i)).collect::<Vec<_>>(),
        "paies": vues,
    }))
}
