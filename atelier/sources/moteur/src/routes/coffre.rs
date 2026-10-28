//! Les routes qui ECRIVENT du lot G3 (`moteur/web.py`, `operations_ouverture`, `operations_exercice`,
//! `operations_importation`) : le coffre (`coffre-poser`, `coffre-compte-dpae`, `coffre-oublier`), le mot de
//! passe du logiciel (`mot-de-passe-changer`, `mot-de-passe-retirer`), `google-oublier`, `maj-malgre-calcul`,
//! `cloture` (et `rouvrir`), `export-compta` et `importer`.
//!
//! Memes fichiers ecrits, aux memes octets, memes reponses que le Python ; un refus garde sa phrase quand
//! elle est utile a la page, la classe de l'exception peut differer (`ErreurCoffre` devient
//! `ErreurOperation`, `ExerciceOuvert` et `ClotureRefusee` aussi).
//!
//! - Le coffre : ce que `moteur coffre` faisait deja (`coffre.rs`) ; ces routes y ajoutent la porte, le mot de
//!   passe du logiciel s'il est pose (`filet::exiger_mot_de_passe`). Aucune valeur de secret ne sort : la
//!   reponse est l'etat du coffre, des presences et des dates.
//! - `cloture`, `export-compta` : `exercice_ecrit.rs`. `importer` : `importation_ecrite.rs`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::coffre::{Coffre, Echec as EchecCoffre};
use crate::filet::exiger_mot_de_passe;
use droit::contexte::Contexte;
use super::{ou_texte, vers_route};
use envois::etats::{get, joindre, objet_vide, ou};
use envois::google::{champ, en_texte, jeton_par_rafraichissement, DELAI, PERMISSIONS};
use socle::echec::{erreur, Echec, R};
use socle::fichiers::{vrai, MARQUEUR_COPIE};
use socle::valeur::{nom_type, valeur_en_texte};



/// Les routes de ce module.
pub const ROUTES: &[&str] = &[
    "coffre-etat", "google-etat",
    "coffre-poser", "coffre-compte-dpae", "coffre-oublier", "mot-de-passe-changer", "mot-de-passe-retirer", "google-oublier",
];

/// Celles qui ecrivent.
pub const ECRITURES: &[&str] =
    &["coffre-poser", "coffre-compte-dpae", "coffre-oublier", "mot-de-passe-changer", "mot-de-passe-retirer", "google-oublier"];

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "coffre-etat" => coffre_etat(ctx),
        "google-etat" => google_etat(ctx),
        "coffre-poser" => coffre_poser(ctx, charge),
        "coffre-compte-dpae" => coffre_compte_dpae(ctx, charge),
        "coffre-oublier" => coffre_oublier(ctx, charge),
        "mot-de-passe-changer" => mot_de_passe_changer(ctx, charge),
        "mot-de-passe-retirer" => mot_de_passe_retirer(ctx, charge),
        "google-oublier" => google_oublier(ctx, charge),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

// ------------------------------------------------------------------ outils

#[track_caller]
fn refus(m: impl Into<String>) -> Echec {
    erreur("ErreurOperation", m)
}

/// `charge.get(cle)`.
fn optionnel<'a>(charge: &'a Value, cle: &str) -> &'a Value {
    charge.get(cle).unwrap_or(&Value::Null)
}

/// Le refus du coffre sous le nom que la route Python lui donnait : `ErreurCoffre` devient `ErreurOperation`
/// (`operations_ouverture` l'attrapait), tout le reste (`OSError`...) garde sa classe.
fn en_operation(x: EchecCoffre) -> Echec {
    Echec { classe: Some(if x.classe == "ErreurCoffre" { "ErreurOperation" } else { x.classe }), message: x.message }
}

/// Le refus du coffre tel quel (`coffre.oublier_ouverture` ne l'attrapait pas).
fn en_coffre(x: EchecCoffre) -> Echec {
    Echec { classe: Some(x.classe), message: x.message }
}

/// `datetime.now().isoformat(timespec="minutes")`, a l'heure de Paris : ce que `coffre._maintenant` posait.
fn maintenant() -> String {
    let t = socle::temps::secondes();
    socle::paris::iso_minutes(t)
}

fn coffre(ctx: &Contexte) -> Coffre {
    Coffre::du_poste(&ctx.racine, &maintenant())
}

/// `coffre_etat(ctx)` : ce qui est pose, depuis quand. Jamais les valeurs.
fn etat_du_coffre(ctx: &Contexte) -> R<Value> {
    coffre_etat_de(&ctx.racine, &ctx.entreprise)
}

/// Une chaine de la charge, quand c'en est une (`texte` du moteur : le reste n'est rien).
fn chaine<'a>(charge: &'a Value, cle: &str) -> Option<&'a str> {
    charge.get(cle).and_then(Value::as_str)
}

// ------------------------------------------------------------------ le coffre

/// `operations_ouverture.poser_secret` : range le mot de passe d'un organisme, ou de la boite courriel avec
/// son adresse (`compte`). Rend l'etat du coffre, sans valeur.
fn coffre_poser(ctx: &Contexte, charge: &Value) -> R<Value> {
    exiger_mot_de_passe(&ctx.racine, optionnel(charge, "motdepasse"), "poser un mot de passe")?;
    let service = crate::coffre::service_connu(optionnel(charge, "service")).map_err(en_operation)?;
    let valeur = chaine(charge, "valeur").unwrap_or("");
    coffre(ctx).poser(service.nom, valeur, chaine(charge, "compte")).map_err(en_operation)?;
    etat_du_coffre(ctx)
}

/// `operations_ouverture.oublier_secret`.
fn coffre_oublier(ctx: &Contexte, charge: &Value) -> R<Value> {
    exiger_mot_de_passe(&ctx.racine, optionnel(charge, "motdepasse"), "retirer un mot de passe")?;
    let service = crate::coffre::service_connu(optionnel(charge, "service")).map_err(en_operation)?;
    coffre(ctx).oublier(service.nom).map_err(en_operation)?;
    etat_du_coffre(ctx)
}

/// `operations_ouverture.poser_compte_dpae` : le nom et le prenom du compte Urssaf qui depose les DPAE.
/// SANS ACCENTS, DANS LA CASSE TAPEE : la DUE MtoM refuse les accents. Les autres cles d'`api_dpae` sont gardees.
fn coffre_compte_dpae(ctx: &Contexte, charge: &Value) -> R<Value> {
    exiger_mot_de_passe(&ctx.racine, optionnel(charge, "motdepasse"), "changer le compte qui depose les DPAE")?;
    let mut propres: Vec<(&str, String)> = Vec::new();
    for cle in ["nom", "prenom"] {
        let v = optionnel(charge, cle);
        let brut = if vrai(v) { valeur_en_texte(v) } else { String::new() };
        let sans = declarations::champs::sans_accents(&brut);
        let net = sans.split(socle::texte::blanc).filter(|m| !m.is_empty()).collect::<Vec<_>>().join(" ");
        if net.is_empty() {
            return Err(refus(format!(
                "Le {} du compte Urssaf manque. Il est celui du compte ouvert sur urssaf.fr pour la DPAE -- pas forcement celui de la personne qui signe. Rien n'a ete modifie.",
                cle
            )));
        }
        propres.push((cle, net));
    }
    let chemin = ctx.racine.join("config").join("entreprise.json");
    if !chemin.exists() {
        // `json.loads(chemin.read_text())` : le fichier doit etre la.
        return Err(erreur("FileNotFoundError", format!("{} : fichier introuvable", chemin.display())));
    }
    let mut contenu = crate::routes::lire_json_ou_vide(&chemin)?;
    let Value::Object(entreprise) = &mut contenu else {
        return Err(erreur("AttributeError", socle::fichiers::attendu("un objet", socle::valeur::nom_type(&contenu))));
    };
    let mut cfg = match ctx.entreprise.get("api_dpae").filter(|v| vrai(v)) {
        Some(Value::Object(o)) => o.clone(),
        Some(autre) => return Err(erreur("TypeError", socle::fichiers::attendu("un objet", socle::valeur::nom_type(autre)))),
        None => serde_json::Map::new(),
    };
    for (cle, v) in propres {
        cfg.insert(cle.to_string(), Value::String(v));
    }
    // Les identifiants du compte valent a partir du jour ou on les pose (les premiers, de tout temps).
    droit::contexte::dater(entreprise, "api_dpae", Value::Object(cfg), Some(&ctx.aujourdhui), None);
    crate::routes::mois::sauver_avant(&chemin)?;
    socle::ecriture::ecrire_json(&chemin, &contenu, 2).map_err(|m| erreur("OSError", m))?;
    // L'etat se lit sur la fiche qu'on vient d'ecrire : `ctx` l'a lue avant.
    let relue = droit::contexte::fiche_au(&contenu, &ctx.aujourdhui, "config/entreprise.json").map_err(Echec::from)?;
    coffre_etat_de(&ctx.racine, &relue)
}

// ------------------------------------------------------------------ le mot de passe du logiciel

/// `operations_ouverture.changer_mot_de_passe_logiciel` : pose ou change le mot de passe du logiciel. L'actuel
/// est exige s'il existe, et il ouvre aussi le coffre quand la cle de ce poste ne l'ouvre plus.
fn mot_de_passe_changer(ctx: &Contexte, charge: &Value) -> R<Value> {
    let actuel = optionnel(charge, "motdepasse");
    exiger_mot_de_passe(&ctx.racine, actuel, "changer le mot de passe du logiciel")?;
    let texte = |cle: &str| {
        let v = optionnel(charge, cle);
        if vrai(v) {
            valeur_en_texte(v)
        } else {
            String::new()
        }
    };
    let (nouveau, confirmation) = (texte("nouveau"), texte("confirmation"));
    if nouveau != confirmation {
        return Err(refus("Les deux saisies du nouveau mot de passe different. Rien n'a ete modifie."));
    }
    // Une saisie qui n'est pas un texte n'est pas un mot de passe : le coffre la lit comme vide.
    let nouveau = if matches!(optionnel(charge, "nouveau"), Value::String(_) | Value::Null) { nouveau } else { String::new() };
    // UNE COMPAGNIE DE L'ADMINISTRATEUR (ticket #244) : son mot de passe est celui de toutes ses compagnies, et il se change
    // pour toutes a la fois (`administrateur::changer`) ; l'actuel les ouvre chacune, tout se prepare puis tout se remplace.
    let inst = crate::administrateur::installation_de(&ctx.racine);
    if crate::administrateur::est_rattachee(&inst, &ctx.racine) {
        let Some(actuel) = chaine(charge, "motdepasse").filter(|m| !m.is_empty()) else {
            return Err(refus("Tapez le mot de passe actuel de l'administrateur : le nouveau le remplace dans toutes ses compagnies. Rien n'a ete modifie."));
        };
        crate::administrateur::changer(&inst, actuel, &nouveau, &confirmation, &crate::administrateur::Poste::de_ce_poste()).map_err(refus)?;
        return etat_du_coffre(ctx);
    }
    coffre(ctx).poser_ouverture(&nouveau, chaine(charge, "motdepasse")).map_err(en_operation)?;
    etat_du_coffre(ctx)
}

/// `operations_ouverture.retirer_mot_de_passe_logiciel` : plus de mot de passe du logiciel. L'actuel est exige.
fn mot_de_passe_retirer(ctx: &Contexte, charge: &Value) -> R<Value> {
    if crate::administrateur::est_rattachee(&crate::administrateur::installation_de(&ctx.racine), &ctx.racine) {
        return Err(refus("Cette compagnie est geree par l'administrateur : son mot de passe ouvre aussi les autres. La detacher d'abord (elle recoit alors son propre mot de passe), puis le retirer. Rien n'a ete modifie."));
    }
    exiger_mot_de_passe(&ctx.racine, optionnel(charge, "motdepasse"), "retirer le mot de passe du logiciel")?;
    coffre(ctx).oublier_ouverture(chaine(charge, "motdepasse")).map_err(en_coffre)?;
    etat_du_coffre(ctx)
}

// ------------------------------------------------------------------ Google

/// `operations_ouverture.google_oublier` : efface le jeton local, sans reseau. L'acces reste accorde cote
/// Google, et se retire sur myaccount.google.com.
fn google_oublier(ctx: &Contexte, charge: &Value) -> R<Value> {
    exiger_mot_de_passe(&ctx.racine, optionnel(charge, "motdepasse"), "oublier le compte Google")?;
    let p = ctx.racine.join("config").join("google-jeton.bin");
    let efface = p.exists();
    if efface {
        std::fs::remove_file(&p).map_err(|e| erreur("OSError", format!("{} : {}", p.display(), e)))?;
    }
    Ok(json!({ "efface": efface }))
}

// ------------------------------------------------------------------ la mise a jour


// ------------------------------------------------------------------ le compte Google

/// `google.APPLICATION` : l'application Google du logiciel, livree avec lui
/// dans `google-application.json` (a cote du code), `client_id` et
/// `client_secret`. Le secret ne s'ecrit pas dans le source Rust.
const FICHIER_APPLICATION: &str = "google-application.json";

/// `google.FICHIER_IDENTIFIANTS`, `google.FICHIER_JETON`.
const FICHIER_IDENTIFIANTS: &str = "google.json";
pub(crate) const FICHIER_JETON: &str = "google-jeton.bin";

/// La description scellee avec le jeton (`google._dpapi`).
pub(crate) const DESCRIPTION: &str = "moteur de paie - jeton Google";

/// `compagnies.MARQUEUR`.

fn config(racine: &Path) -> PathBuf {
    racine.join("config")
}

/// `ErreurGoogle` : la phrase est ecrite pour etre lue.
#[track_caller]
fn erreur_google(message: impl Into<String>) -> Echec {
    erreur("ErreurGoogle", message)
}

/// `google.identifiants` : (client_id, client_secret, origine).
pub(crate) fn identifiants(racine: &Path) -> R<(Value, Value, String)> {
    let p = config(racine).join(FICHIER_IDENTIFIANTS);
    if p.exists() {
        // Un `google.json` illisible est un refus, pas un plantage (le Python laissait passer une trace : `JSONDecodeError`,
        // `UnicodeDecodeError`, `AttributeError`). Decision de Leandre : un defaut du Python ne se reproduit pas.
        let illisible = |raison: String| erreur_google(format!("{} est illisible : {}.\nLe corriger, ou le supprimer pour revenir a l'application du logiciel.", p.display(), raison));
        let octets = std::fs::read(&p).map_err(|e| illisible(e.to_string()))?;
        let texte = String::from_utf8(octets).map_err(|e| illisible(e.to_string()))?;
        let donnees: Value = serde_json::from_str(&texte).map_err(|e| illisible(e.to_string()))?;
        let Value::Object(d) = &donnees else { return Err(illisible("ce n'est pas un objet JSON".into())) };
        let manque: Vec<&str> = ["client_id", "client_secret"]
            .into_iter()
            .filter(|c| !d.get(*c).map(vrai).unwrap_or(false))
            .collect();
        if !manque.is_empty() {
            return Err(erreur_google(format!(
                "{} incomplet, il manque : {}.\nLe supprimer suffit a revenir a l'application du logiciel.",
                p.display(),
                manque.join(", ")
            )));
        }
        return Ok((d["client_id"].clone(), d["client_secret"].clone(), p.display().to_string()));
    }
    let livre = socle::fichiers::dossier_du_code().join(FICHIER_APPLICATION);
    let application = std::fs::read(&livre)
        .ok()
        .and_then(|o| serde_json::from_slice::<Value>(&o).ok())
        .filter(|a| a.get("client_id").map(vrai).unwrap_or(false));
    let Some(a) = application else {
        return Err(erreur_google(format!(
            "L'application Google du logiciel n'est pas renseignee.\n\n\
             Elle est livree avec chaque version publiee ({}, posee par\n\
             le flux « Publier » depuis le secret PUPPETPAIE_GOOGLE_APPLICATION) ;\n\
             une copie de travail du depot ne l'a pas.\n\n\
             Sinon, on peut utiliser sa propre application en creant\n\
             {} :\n\n\
             \x20 {{\n\
             \x20   \"client_id\": \"....apps.googleusercontent.com\",\n\
             \x20   \"client_secret\": \"....\"\n\
             \x20 }}\n",
            livre.display(),
            p.display()
        )));
    };
    Ok((a["client_id"].clone(), a.get("client_secret").cloned().unwrap_or(Value::Null), "le logiciel".into()))
}

/// `google.autorise` : y a-t-il un jeton utilisable, sans rien demander au reseau. Un jeton illisible, un coffre qui
/// refuse, un jeton sans `rafraichissement` : non. Ne leve jamais (l'ouverture, lot I2b, l'appelle).
pub(crate) fn autorise(racine: &Path) -> bool {
    match lire_jeton(racine) {
        Ok(j) => champ(&j, "rafraichissement").map(|v| vrai(&v)).unwrap_or(false),
        Err(()) => false,
    }
}

/// `google._lire_jeton` : le contenu du jeton, `Null` s'il n'y en a pas.
/// `Err` : ce que `autorise` et ses voisins rattrapent (`ErreurGoogle`,
/// `ValueError`) -- le coffre qui refuse, un JSON illisible.
fn lire_jeton(racine: &Path) -> Result<Value, ()> {
    lire_jeton_dit(racine).map_err(|_| ())
}

/// `google._lire_jeton`, avec la phrase du refus : le coffre qui refuse dit
/// quoi faire (`google._dpapi`), un JSON illisible dit seulement qu'il l'est.
/// Le client Drive (`drive.rs`) la rend telle quelle.
pub(crate) fn lire_jeton_dit(racine: &Path) -> Result<Value, String> {
    let p = config(racine).join(FICHIER_JETON);
    if !p.exists() {
        return Ok(Value::Null);
    }
    // Une lecture impossible (droits) levait une OSError que rien ne
    // rattrapait ; elle se range ici avec les refus du coffre : l'etat se lit.
    let brut = std::fs::read(&p).map_err(|e| format!("{} : {}", p.display(), e))?;
    let clair = crate::coffre::Coffre::du_poste(racine, "").deproteger(&brut, DESCRIPTION).map_err(|e| {
        format!("{} Pour le jeton Google, le remede est : relancer `moteur google autoriser`.", e.message)
    })?;
    let texte = String::from_utf8(clair).map_err(|e| e.to_string())?;
    serde_json::from_str(&texte).map_err(|e| e.to_string())
}

/// `google.compte_autorise` : l'adresse du compte, sans reseau ; `None` si pas autorise.
pub fn compte_autorise(racine: &Path) -> Option<String> {
    let j = lire_jeton_dit(racine).ok()?;
    match champ(&j, "compte").ok()? {
        Value::Null => None,
        Value::String(s) => Some(s),
        autre => Some(en_texte(&autre)),
    }
}

/// Les adresses de Google ; dans l'atelier, un faux Google local (`atelier.rs`).
pub fn adresses_du_poste() -> envois::drive::Adresses {
    #[cfg(feature = "atelier")]
    if let Some(b) = crate::atelier::base(crate::atelier::VARIABLE_GOOGLE) {
        return envois::drive::Adresses::de_la_base(&b);
    }
    envois::drive::Adresses::default()
}

/// LE CLIENT DU DRIVE DE LA COMPAGNIE : refuse une copie de developpement (qui ecrirait dans le Drive de la VRAIE
/// compagnie), lit l'application (`google.json` ou celle du logiciel), le jeton scelle par la cle du poste et l'adresse
/// du compte, et les passe au client (`drive::Client::nouveau`). Aucun appel reseau.
pub fn client_drive(racine: &Path) -> Result<envois::drive::Client, envois::drive::ErreurGoogle> {
    if let Some(refus) = refus_de_copie(racine) {
        return Err(refus.into());
    }
    let (id, secret, _origine) = identifiants(racine)?;
    let autorisation = envois::drive::Autorisation { identifiants: (id, secret), jeton: lire_jeton_dit(racine), compte: compte_autorise(racine) };
    Ok(envois::drive::Client::nouveau(autorisation, adresses_du_poste()))
}

/// LIER LE COMPTE GOOGLE : lit l'application et les adresses du poste, mene l'autorisation
/// (`google_autorisation::autoriser`), puis scelle le jeton obtenu. Rend l'adresse du compte lie.
pub(crate) fn autoriser(racine: &Path, delai: u64, dire_url: &dyn Fn(&str), dire: &dyn Fn(&str)) -> R<String> {
    let (id, secret, _) = identifiants(racine).map_err(|e| Echec { classe: Some("ErreurOperation"), message: e.message })?;
    let adresses = {
        #[cfg(feature = "atelier")]
        if let Some(b) = crate::atelier::base(crate::atelier::VARIABLE_GOOGLE) {
            envois::google_autorisation::Adresses::de_la_base(&b)
        } else {
            envois::google_autorisation::Adresses::default()
        }
        #[cfg(not(feature = "atelier"))]
        envois::google_autorisation::Adresses::default()
    };
    let jeton = envois::google_autorisation::autoriser((id, secret), &adresses, delai, dire_url, dire)?;
    sceller_le_jeton(racine, &jeton)
}

/// Le jeton que l'autorisation a obtenu (`google_autorisation::autoriser`) et l'adresse du compte, dans un seul fichier
/// scelle par la cle du poste. Rend l'adresse du compte lie.
fn sceller_le_jeton(racine: &Path, jeton: &envois::google_autorisation::JetonGoogle) -> R<String> {
    let brut = socle::json_ascii::dumps(
        &serde_json::json!({"rafraichissement": jeton.rafraichissement, "compte": jeton.compte, "permissions": jeton.permissions}),
        None,
        false,
        false,
    );
    let scelle = crate::coffre::Coffre::du_poste(racine, "")
        .proteger(brut.as_bytes(), DESCRIPTION)
        .map_err(|e| erreur_google(format!("Le jeton n'a pas pu etre scelle, rien n'est enregistre : {}", e.message)))?;
    let dossier = config(racine);
    std::fs::create_dir_all(&dossier).map_err(|e| erreur_google(format!("{} : {}", dossier.display(), e)))?;
    let chemin = dossier.join(FICHIER_JETON);
    std::fs::write(&chemin, scelle).map_err(|e| erreur_google(format!("{} : {}", chemin.display(), e)))?;
    Ok(jeton.compte.clone())
}

/// `p.rsplit("/", 1)[-1]` sur chaque permission.
fn courtes<'a>(ps: impl IntoIterator<Item = &'a Value>) -> R<Vec<Value>> {
    ps.into_iter()
        .map(|p| match p {
            Value::String(s) => Ok(json!(s.rsplit('/').next().unwrap_or(s))),
            autre => Err(erreur("AttributeError", socle::fichiers::attendu("un texte", nom_type(autre)))),
        })
        .collect()
}

/// `list(x)` de ce que le jeton enregistre comme permissions.
fn liste(v: &Value) -> R<Vec<Value>> {
    match v {
        Value::Array(a) => Ok(a.clone()),
        Value::String(s) => Ok(s.chars().map(|c| json!(c.to_string())).collect()),
        Value::Object(o) => Ok(o.keys().map(|k| json!(k)).collect()),
        autre => Err(erreur("TypeError", socle::fichiers::attendu("une liste", nom_type(autre)))),
    }
}

/// `compagnies.refuser_si_copie` : une copie de developpement n'ecrit jamais
/// dans le Drive de la vraie compagnie. Le refus, sous la classe du Python.
pub(crate) fn refus_de_copie(racine: &Path) -> Option<Echec> {
    Path::new(racine).join(MARQUEUR_COPIE).is_file().then(|| {
        erreur(
            "CopieDeDeveloppement",
            "Copie de developpement : la sauvegarde ou l'archivage Drive ne part jamais d'ici. Les essais se font en \
             test ; pour le faire en vrai, ouvrir la vraie compagnie.",
        )
    })
}

/// `Client(ctx).jeton_acces()` : `Ok(None)` si Google rend un jeton d'acces,
/// `Ok(Some(probleme))` pour une `ErreurGoogle`, `Err` pour ce que Python
/// laissait passer.
fn rafraichir(ctx: &Contexte, client_id: &Value, client_secret: &Value) -> R<Option<String>> {
    // `compagnies.refuser_si_copie`, a la construction du client.
    if let Some(refus) = refus_de_copie(&ctx.racine) {
        return Err(refus);
    }
    let j = match lire_jeton(&ctx.racine) {
        Ok(j) => j,
        Err(()) => return Ok(Some("le jeton Google ne s'ouvre plus".into())),
    };
    let rafraichissement = champ(&j, "rafraichissement")?;
    if !vrai(&rafraichissement) {
        return Ok(Some(
            "Compte Google pas encore lie : Configuration > Mots de passe > Lier le Drive, ou `moteur google \
             autoriser`."
                .into(),
        ));
    }
    Ok(jeton_par_rafraichissement(&adresses_du_poste().jetons, client_id, client_secret, &rafraichissement, DELAI)?.err())
}

/// `operations_ouverture.google_etat`, rendu par `sortie_json.etat_google`.
fn google_etat(ctx: &Contexte) -> R<Value> {
    // L'application ne sert qu'a rafraichir un jeton : sans Drive lie, son absence (une copie de travail du depot) n'est
    // pas un refus, et l'etat dit seulement « pas lie » (#235 du prive). Un `config/google.json` pose mais illisible ou
    // incomplet en reste un : c'est un fichier de l'utilisateur, a corriger.
    let application = match identifiants(&ctx.racine) {
        Err(e) if config(&ctx.racine).join(FICHIER_IDENTIFIANTS).exists() => return Err(e),
        a => a,
    };
    let origine = application.as_ref().map(|a| json!(a.2)).unwrap_or(Value::Null);
    let permissions = courtes(&PERMISSIONS.map(|p| json!(p)))?;
    let etat = |compte: Value, verifie: bool, probleme: Option<String>, accordees: Vec<Value>| -> Value {
        let mut o = Map::new();
        o.insert("application".into(), origine.clone());
        o.insert("permissions".into(), json!(permissions));
        o.insert("accordees".into(), json!(accordees));
        o.insert("autorise".into(), json!(vrai(&compte)));
        o.insert("compte".into(), compte);
        o.insert("verifie".into(), json!(verifie));
        o.insert("probleme".into(), json!(probleme));
        Value::Object(o)
    };
    // `autorise` : un jeton illisible ou sans rafraichissement, c'est « pas autorise ».
    let jeton = lire_jeton(&ctx.racine);
    let autorise = match &jeton {
        Ok(j) => vrai(&champ(j, "rafraichissement")?),
        Err(()) => false,
    };
    if !autorise {
        return Ok(etat(Value::Null, false, None, Vec::new()));
    }
    let j = jeton.unwrap_or(Value::Null);
    let compte = champ(&j, "compte")?;
    let accordees = match champ(&j, "permissions")? {
        p if vrai(&p) => courtes(&liste(&p)?)?,
        _ => permissions.clone(),
    };
    let (client_id, client_secret, _) = application?;
    match rafraichir(ctx, &client_id, &client_secret)? {
        None => Ok(etat(compte, true, None, accordees)),
        Some(probleme) => Ok(etat(compte, false, Some(probleme), accordees)),
    }
}


/// `identifiant_du_compte` : le compte que ce mot de passe ouvre, en clair ; ce
/// qui en manque ; et s'il se saisit sur cette ligne (`ici`).
fn identifiant_du_compte(ent: &Value, service: &str) -> R<Value> {
    match service {
        "net-entreprises" => {
            let d = envois::transport::declarant(ent).map_err(|v| Echec {
                classe: match v.get("classe").and_then(Value::as_str) {
                    Some("AttributeError") => Some("AttributeError"),
                    _ => None,
                },
                message: v.get("erreur").and_then(Value::as_str).unwrap_or("").to_string(),
            })?;
            let mut manque: Vec<Value> = Vec::new();
            for c in ["nom", "prenom"] {
                if !vrai(get(&d, c)?) {
                    manque.push(json!(c));
                }
            }
            if !vrai(get(&d, "siret")?) {
                manque.insert(0, json!("siret"));
            }
            let noms: Vec<Value> = [get(&d, "nom")?, get(&d, "prenom")?].into_iter().filter(|x| vrai(x)).cloned().collect();
            let texte = joindre(" ", &noms)?;
            let phrase = if manque.is_empty() { format!("compte de {}, SIRET {}", texte, valeur_en_texte(get(&d, "siret")?)) } else { String::new() };
            Ok(json!({"texte": phrase, "manque": manque, "ici": false, "ou": "fiche Entreprise › Qui les organismes contactent"}))
        }
        "urssaf" => {
            let cfg = ou(get(ent, "api_dpae")?, objet_vide());
            let siret = ou(get(cfg, "siret")?, get(ent, "siret")?);
            let mut manque: Vec<Value> = Vec::new();
            for c in ["nom", "prenom"] {
                if !vrai(get(cfg, c)?) {
                    manque.push(json!(c));
                }
            }
            let noms: Vec<Value> = [get(cfg, "prenom")?, get(cfg, "nom")?].into_iter().filter(|x| vrai(x)).cloned().collect();
            let texte = joindre(" ", &noms)?;
            let phrase = if manque.is_empty() { format!("compte de {}, SIRET {}", texte, valeur_en_texte(siret)) } else { String::new() };
            Ok(json!({
                "texte": phrase, "manque": manque, "ici": true,
                "nom": ou_texte(get(cfg, "nom")?), "prenom": ou_texte(get(cfg, "prenom")?),
            }))
        }
        _ => Ok(json!({"texte": "", "manque": [], "ici": false})),
    }
}

/// `coffre_etat` : ce qui est pose, pour quel usage, depuis quand. Jamais les
/// valeurs.
fn coffre_etat(ctx: &Contexte) -> R<Value> {
    coffre_etat_de(&ctx.racine, &ctx.entreprise)
}

/// `coffre_etat` sur une fiche entreprise donnee : `poser_compte_dpae` le relit sur la fiche qu'il vient d'ecrire.
pub(crate) fn coffre_etat_de(racine: &Path, entreprise: &Value) -> R<Value> {
    let coffre = crate::coffre::Coffre::du_poste(racine, "");
    let secrets = coffre.etat().map_err(vers_route)?;
    let mut services = Vec::new();
    for s in &secrets {
        let nom = s["service"].as_str().unwrap_or("");
        services.push(json!({
            "service": s["service"], "libelle": s["libelle"], "sert_a": s["sert_a"],
            "pose": s["pose"], "pose_le": s["pose_le"], "environnement": s["dans_environnement"],
            "compte": s["compte"], "avec_compte": s["avec_compte"],
            "identifiant": identifiant_du_compte(entreprise, nom)?,
        }));
    }
    let mot_de_passe_pose = coffre.ouverture_exigee().map_err(vers_route)?;
    let cle_poste = coffre.cle_posee().map_err(vers_route)?;
    Ok(json!({
        "services": services,
        "mot_de_passe_pose": mot_de_passe_pose,
        "cle_poste": cle_poste,
        "chemin_cle": coffre.chemin_cle().display().to_string(),
    }))
}
