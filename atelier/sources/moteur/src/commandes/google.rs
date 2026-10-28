//! `moteur google [etat|autoriser|oublier]` : le compte Google, qui ne porte que le Drive (`moteur/cli.py`, `cmd_google`).
//! `autoriser` et `oublier` sont des actes (J2b) ; `autoriser` AFFICHE l'URL de consentement sans ouvrir de navigateur (la fenetre
//! de consentement est au lot J3b).
//!
//! ATTENTION : l'etat rafraichit reellement le jeton aupres de Google quand un compte est lie (la route `google-etat`) :
//! c'est la seule facon de savoir si l'autorisation tient. Sans jeton, il n'y a aucune connexion.

use super::analyse::{Args, Opt, Pos, Spec};
use super::rendu::{champ, enveloppe, erreur, rendre, texte};
use super::Cadre;
use serde_json::{json, Value};

pub static SPEC: Spec = Spec {
    commande: "google",
    positionnels: &[Pos::parmi("quoi", &["etat", "autoriser", "oublier"], Some("etat"))],
    options: &[Opt::drapeau("sans-navigateur"), Opt::drapeau("fenetre"), Opt::drapeau("json")],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    let quoi = args.texte("quoi").unwrap_or("etat");
    let json = args.drapeau("json");
    if quoi == "oublier" {
        return oublier(cadre, json);
    }
    if quoi == "autoriser" {
        return autoriser(cadre, json);
    }
    if quoi != "etat" {
        eprintln!("`moteur google {}` n'existe pas.", quoi);
        return 2;
    }
    let e = match cadre.route("google-etat", json!({})) {
        Ok(e) => e,
        Err(f) if matches!(f.classe, Some("ErreurGoogle" | "ErreurOperation")) => {
            if json {
                return rendre(&erreur("google", f.classe.unwrap_or("ErreurGoogle"), &f.message, &[]));
            }
            dire!("{}", f.message);
            return 2;
        }
        Err(f) => return cadre.plantage(f),
    };
    let vrai = |k: &str| e.get(k).is_some_and(socle::fichiers::vrai);
    if json {
        // L'ordre des cles de `sortie_json.etat_google` (la route les range autrement).
        let ordre: serde_json::Map<String, Value> =
            ["application", "permissions", "accordees", "compte", "autorise", "verifie", "probleme"].iter().map(|k| ((*k).to_string(), e.get(*k).cloned().unwrap_or(Value::Null))).collect();
        rendre(&enveloppe("google", Value::Object(ordre)));
        return if vrai("verifie") { 0 } else { 1 };
    }
    let liste = |k: &str| -> String { e.get(k).and_then(Value::as_array).map(|a| a.iter().map(texte).collect::<Vec<_>>().join(", ")).unwrap_or_default() };
    dire!("Application : {}", champ(&e, "application"));
    dire!("Permissions : {} (demandees)", liste("permissions"));
    if !vrai("compte") {
        dire!("Compte      : aucun\n\nPas encore lie :  moteur google autoriser\n  (ou Configuration > Mots de passe > Lier le Drive)");
        return 1;
    }
    dire!("Compte      : {}", champ(&e, "compte"));
    if vrai("accordees") {
        dire!("Accordees   : {}", liste("accordees"));
    }
    if vrai("probleme") {
        dire!("Verification: {}", champ(&e, "probleme"));
        return 1;
    }
    dire!("Verification: le compte repond, l'autorisation tient.");
    0
}

/// `moteur google oublier` : efface le jeton local (rien ne part au reseau). La route exige le mot de passe du logiciel s'il en est un
/// de pose : le MOTEUR le demande lui-meme a un humain (fenetre, terminal sans echo), sinon il refuse. Dans `moteur-atelier`, hors
/// compagnie fictive, le geste est refuse. Les erreurs sont rendues comme le Python : le message nu (code 2), ou `--json`.
fn oublier(cadre: &Cadre, json: bool) -> i32 {
    let r = super::actes::refuser_dans_l_atelier(cadre, "d'oublier le compte Google").and_then(|()| cadre.route("google-oublier", json!({})));
    match r {
        Ok(e) => {
            if e.get("efface").is_some_and(socle::fichiers::vrai) {
                dire!("Jeton local efface. L'acces reste accorde cote Google :\n  le retirer sur myaccount.google.com > Securite >\n  Vos connexions a des applications tierces.");
            } else {
                dire!("Il n'y avait pas de jeton local.");
            }
            0
        }
        Err(f) if super::actes::est_refus(f.classe) || f.classe == Some("ErreurGoogle") => {
            let phrase = super::actes::phrase(&f);
            if json {
                return rendre(&erreur("google", f.classe.unwrap_or("ErreurOperation"), &phrase, &[]));
            }
            dire!("{}", phrase);
            2
        }
        Err(f) => cadre.plantage(f),
    }
}

/// `moteur google autoriser [--sans-navigateur]` : lier le compte Google. Le moteur affiche l'URL de la fenetre de consentement et
/// attend, sur `127.0.0.1`, la reponse de Google (`google_autorisation`) ; il N'OUVRE PAS de navigateur (lot J3b) : la personne ouvre
/// l'URL, `--sans-navigateur` est donc sans effet. Aucun secret ne passe par la commande : le jeton va de la reponse de Google au
/// fichier scelle par la cle du poste.
///
/// GARDE-FOUS. Le mot de passe du logiciel s'il en est un de pose : le moteur le demande lui-meme a un humain (`exiger_mot_de_passe`),
/// sinon il refuse. Dans `moteur-atelier`, hors compagnie fictive : refuse, sauf si Google est redirige vers le faux Google local.
/// L'URL va sur la sortie standard (comme le Python) ; avec `--json`, sur la sortie d'erreur, pour que la sortie standard ne porte que
/// le JSON d'une erreur.
fn autoriser(cadre: &Cadre, json: bool) -> i32 {
    let r = super::actes::refuser_si_pas_redirige(cadre, super::actes::Organisme::Google, "lier le compte Google")
        .and_then(|()| crate::filet::exiger_mot_de_passe(&cadre.racine, &Value::Null, "lier le compte Google"))
        .and_then(|()| {
            let url = |u: &str| {
                if json {
                    eprintln!("Fenetre d'autorisation Google : {}\n", u);
                } else {
                    dire!("Fenetre d'autorisation Google : {}\n", u);
                }
            };
            let attention = |t: &str| {
                if json {
                    eprintln!("{}", t);
                } else {
                    dire!("{}", t);
                }
            };
            crate::routes_google::autoriser(&cadre.racine, envois::google_autorisation::DELAI_AUTORISATION, &url, &attention)
        });
    match r {
        Ok(compte) => {
            dire!("C'est bon : {} est lie.\nRien d'autre a faire, l'autorisation tient tant qu'elle n'est pas retiree.", compte);
            0
        }
        Err(f) => {
            let phrase = super::actes::phrase(&f);
            if json {
                return rendre(&erreur("google", f.classe.unwrap_or("ErreurOperation"), &phrase, &[]));
            }
            dire!("{}", phrase);
            2
        }
    }
}
