//! `moteur coffre [etat|seance]` : ce qui est pose dans le coffre (jamais une valeur), et la seance du mot de passe du
//! logiciel.
//!
//! - `etat` dit ce qui est pose, pour quel usage, depuis quand, et ou est la cle du poste.
//! - `seance` : le mot de passe du logiciel tape une fois (par `demande::ouvrir_seance`), garde 12 h ; il ouvre le coffre,
//!   il n'autorise jamais un acte reel. `seance fermer` la clot.
//!
//! Poser, oublier, verifier un secret, poser ou changer le mot de passe du logiciel sont des gestes de l'ecran : leurs
//! routes (`coffre-poser`, `coffre-oublier`, `coffre-verifier`, `mot-de-passe-changer`, `mot-de-passe-retirer`) font
//! taper les secrets par le moteur lui-meme, jamais par l'entree standard (`routes::coffre`).
//!
//! `--json` et `--fenetre` sont acceptes et sans effet. Les phrases d'une demande de mot de passe vont sur la sortie
//! d'erreur ou sur le terminal, jamais sur la sortie standard d'un programme qui lirait du JSON.

use super::actes::{est_refus, maintenant_minutes};
use super::analyse::{Args, Opt, Pos, Spec};
use super::rendu::{champ, texte};
use super::Cadre;
use crate::compagnie::coffre::Coffre;
use serde_json::Value;
use socle::echec::Echec;

pub static SPEC: Spec = Spec {
    commande: "coffre",
    positionnels: &[Pos::parmi("quoi", &["etat", "seance"], Some("etat")), Pos::facultatif("service", None)],
    options: &[Opt::drapeau("fenetre"), Opt::drapeau("json")],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    let r = match args.texte("quoi").unwrap_or("etat") {
        "seance" => seance(cadre, args),
        _ => faire(cadre),
    };
    match r {
        Ok(c) => c,
        // Un refus (le mot de passe du logiciel non tape, l'atelier, une phrase du coffre) : une phrase, code 2.
        Err(e) if est_refus(e.classe) || e.classe == Some("ErreurCoffre") => {
            dire!("! {}", e.message);
            2
        }
        Err(e) => cadre.plantage(e),
    }
}

fn vers(e: crate::compagnie::coffre::Echec) -> Echec {
    Echec { classe: None, message: e.message }
}

/// Le refus du coffre sous sa classe (`ErreurCoffre`, `OSError`...), pour que la commande sache s'il se rend en phrase.
fn vers_classe(e: crate::compagnie::coffre::Echec) -> Echec {
    Echec { classe: Some(e.classe), message: e.message }
}

fn faire(cadre: &Cadre) -> Result<i32, Echec> {
    let c = Coffre::du_poste(&cadre.racine, "");
    dire!("Coffre : {}", c.chemin().display());
    dire!("  scelle (AES-256-GCM). Cle de ce poste : {}", c.chemin_cle().display());
    if !c.cle_posee().map_err(vers)? {
        // Pas de coffre non plus : il n'y a rien d'anormal a signaler, la cle sera tiree au premier secret pose.
        if c.chemin().exists() {
            dire!("  Elle est ABSENTE, et le coffre est la : il ne s'ouvrira");
            dire!("  qu'avec le mot de passe du logiciel, qui reposera une");
            dire!("  cle pour ce poste au passage.");
        } else {
            dire!("  Pas encore tiree -- elle le sera au premier secret pose.");
        }
    }
    dire!();
    for s in c.etat().map_err(vers)? {
        let mut marque = if s.get("pose").is_some_and(socle::fichiers::vrai) { "pose" } else { "-" };
        if s.get("dans_environnement").is_some_and(socle::fichiers::vrai) {
            marque = "environnement";
        }
        let depuis = match s.get("pose_le") {
            Some(d) if socle::fichiers::vrai(d) => format!("depuis le {}", texte(d)),
            _ => String::new(),
        };
        dire!("  {:<18} {:<14} {}", champ(&s, "service"), marque, depuis);
        dire!("     {}", champ(&s, "sert_a"));
        if let Some(Value::String(compte)) = s.get("compte").filter(|v| socle::fichiers::vrai(v)) {
            dire!("     boite : {}", compte);
        }
    }
    dire!();
    if !c.ouverture_exigee().map_err(vers)? {
        dire!("  Mot de passe du logiciel : AUCUN. Rien ne part en reel tant");
        dire!("  qu'il n'est pas pose.");
        dire!("  En poser un :  la route mot-de-passe-changer (Configuration > Mots de passe).");
    } else {
        dire!("  Mot de passe du logiciel : pose. Il est demande a l'ouverture");
        dire!("  de la fenetre et a chaque depot reel, envoi ou signature.");
    }
    Ok(0)
}

/// `coffre seance` : taper le mot de passe du logiciel UNE fois ; les commandes suivantes ouvrent le coffre sans le redemander, 12 h
/// au plus. `coffre seance fermer` la clot. Jamais une preuve d'acte reel.
fn seance(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let coffre = Coffre::du_poste(&cadre.racine, &maintenant_minutes());
    if args.texte("service") == Some("fermer") {
        let fermee = coffre.oublier_ouverture_gardee().map_err(vers_classe)?;
        dire!("{}", if fermee { "Seance fermee." } else { "Aucune seance ouverte." });
        return Ok(0);
    }
    if let Some(fin) = coffre.fin_ouverture_gardee().map_err(vers_classe)? {
        if coffre.ouverture_gardee().map_err(vers_classe)?.is_some() {
            dire!("Seance deja ouverte, jusqu'au {}.", fin.replace('T', " a "));
            return Ok(0);
        }
    }
    match crate::compagnie::demande::ouvrir_seance(&cadre.racine) {
        crate::compagnie::demande::Verdict::Accepte => {
            let fin = coffre.fin_ouverture_gardee().map_err(vers_classe)?.unwrap_or_default();
            dire!(
                "Seance ouverte jusqu'au {} : le coffre s'ouvre sans redemander le mot de passe du logiciel. Un depot REEL le redemande toujours.",
                fin.replace('T', " a ")
            );
            Ok(0)
        }
        crate::compagnie::demande::Verdict::Refuse(phrase) => {
            dire!("! {}", phrase);
            Ok(2)
        }
    }
}
