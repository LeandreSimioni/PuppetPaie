//! Les routes qui ECRIVENT du lot G3 (`moteur/web.py`, `operations_ouverture`, `operations_exercice`,
//! `operations_importation`) : le coffre (`coffre-poser`, `coffre-compte-dpae`, `coffre-oublier`), le mot de
//! passe du logiciel (`mot-de-passe-changer`, `mot-de-passe-retirer`), `google-oublier`, `maj-malgre-calcul`,
//! `cloture` (et `rouvrir`), `export-compta` et `importer`.
//!
//! Memes fichiers ecrits, aux memes octets, memes reponses que le Python ; un refus garde sa phrase quand
//! elle est utile a la page, la classe de l'exception peut differer (`ErreurCoffre` devient
//! `ErreurOperation`, `ExerciceOuvert` et `ClotureRefusee` aussi).
//!
//! - Le coffre : ce que `paie coffre` faisait deja (`coffre.rs`) ; ces routes y ajoutent la porte, le mot de
//!   passe du logiciel s'il est pose (`filet::exiger_mot_de_passe`). Aucune valeur de secret ne sort : la
//!   reponse est l'etat du coffre, des presences et des dates.
//! - `cloture`, `export-compta` : `exercice_ecrit.rs`. `importer` : `importation_ecrite.rs`.

use std::collections::HashMap;
use std::path::Path;

use serde_json::{json, Value};

use crate::coffre::{Coffre, Echec as EchecCoffre};
use crate::contexte::Contexte;
use crate::etats::{erreur, py_str, R};
use crate::fichiers::vrai;
use crate::filet::exiger_mot_de_passe;
use crate::retours::Echec;
use crate::routes_etats::{coffre_etat_de, version};

/// Les routes que ce module sert.
pub const ROUTES: [&str; 12] = [
    "coffre-poser",
    "coffre-compte-dpae",
    "coffre-oublier",
    "mot-de-passe-changer",
    "mot-de-passe-retirer",
    "google-oublier",
    "maj-malgre-calcul",
    "cloture",
    "export-compta",
    "importer",
    // L'administrateur (ticket #244) : le tableau de ses compagnies et le depot groupe (`administrateur.rs`).
    crate::administrateur::ROUTES[0],
    crate::administrateur::ROUTES[1],
];

/// Une route de ce module.
pub fn route(o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "coffre-poser" => coffre_poser(ctx, charge),
        "coffre-compte-dpae" => coffre_compte_dpae(ctx, charge),
        "coffre-oublier" => coffre_oublier(ctx, charge),
        "mot-de-passe-changer" => mot_de_passe_changer(ctx, charge),
        "mot-de-passe-retirer" => mot_de_passe_retirer(ctx, charge),
        "google-oublier" => google_oublier(ctx, charge),
        "maj-malgre-calcul" => maj_malgre_calcul(o, ctx),
        "cloture" => crate::exercice_ecrit::cloture_route(ctx, charge),
        "export-compta" => crate::exercice_ecrit::export_compta_route(ctx, charge),
        "importer" => crate::importation_ecrite::importer(ctx, charge),
        r if crate::administrateur::ROUTES.contains(&r) => crate::administrateur::route(o, ctx, r, charge),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

// ------------------------------------------------------------------ outils

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
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64());
    crate::paris::iso_minutes(t)
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
        let brut = if vrai(v) { py_str(v) } else { String::new() };
        let sans = crate::champs::sans_accents(&brut);
        let net = sans.split(crate::retours::blanc_py).filter(|m| !m.is_empty()).collect::<Vec<_>>().join(" ");
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
    let mut contenu = crate::routes_etats::lire_json_ou_vide(&chemin)?;
    let Value::Object(entreprise) = &mut contenu else {
        return Err(erreur("AttributeError", crate::fichiers::attendu("un objet", crate::etats::nom_type(&contenu))));
    };
    let mut cfg = match entreprise.get("api_dpae").filter(|v| vrai(v)) {
        Some(Value::Object(o)) => o.clone(),
        Some(autre) => return Err(erreur("TypeError", crate::fichiers::attendu("un objet", crate::etats::nom_type(autre)))),
        None => serde_json::Map::new(),
    };
    for (cle, v) in propres {
        cfg.insert(cle.to_string(), Value::String(v));
    }
    entreprise.insert("api_dpae".into(), Value::Object(cfg));
    crate::routes_contrats::sauver_avant(&chemin)?;
    crate::ecriture::ecrire_json(&chemin, &contenu, 2).map_err(|m| erreur("OSError", m))?;
    // L'etat se lit sur la fiche qu'on vient d'ecrire : `ctx` l'a lue avant.
    let relue = crate::contexte::telle_qu_au(&contenu, &ctx.date).map_err(Echec::from)?;
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
            py_str(v)
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

/// `operations_ouverture.appliquer_maj_malgre_calcul` : decider, depuis l'interface, qu'une version qui change
/// un calcul s'appliquera au prochain demarrage meme si un mois saisi n'a pas sa DSN. La decision s'ecrit dans
/// `maj/prete.json` de l'installation ; la reponse est celle de `maj-etat`.
fn maj_malgre_calcul(o: &HashMap<String, String>, ctx: &Contexte) -> R<Value> {
    let inst = crate::fichiers::dossier_du_code().join("..").join("..");
    let Some(mut p) = crate::routes_etats::prete(&inst)? else {
        return Err(refus("Aucune version prete."));
    };
    let Value::Object(m) = &mut p else {
        return Err(erreur("TypeError", "prete.json : un objet est attendu"));
    };
    m.insert("malgre_calcul".into(), Value::Bool(true));
    let chemin = crate::maj::dossier_maj(&inst).join("prete.json");
    // `write_text(json.dumps(p, ensure_ascii=False, indent=1))` : sans saut de ligne final.
    std::fs::write(&chemin, crate::ecriture::json_py(&p, 1)).map_err(|e| erreur("OSError", format!("{} : {}", Path::new(&chemin).display(), e)))?;
    version(o, ctx)
}
