//! `coffre-verifier` (`moteur/operations_ouverture.py` : `verifier_secret`, qui remonte `commandes_ouverture.
//! cmd_verifier_coffre`). Lot H4a.
//!
//! ECRIT, NON PROUVE, NON BRANCHE. Le secret pose ouvre-t-il une session chez l'organisme ? UNE SEULE TENTATIVE,
//! JAMAIS DE BOUCLE : au-dela de quelques echecs, net-entreprises bloque le compte (403). Trois services :
//! net-entreprises (la DSN), l'Urssaf (la DPAE : un autre compte, un autre mot de passe) et la boite courriel (une
//! connexion IMAP). Le verdict est rendu avec la premiere ligne de l'erreur, qui ne contient jamais le mot de passe.
//!
//! Le mot de passe est lu au coffre pour une CONSULTATION (l'environnement d'abord) et va directement au client : il
//! ne sort jamais d'ici, ni dans la reponse ni dans une erreur.
//!
//! `verifier_secret_avec` prend de quoi authentifier et de quoi ouvrir la boite : les tests y passent un faux, sans
//! reseau. `verifier_secret` ouvre les vraies connexions.

use serde_json::{json, Value};

use crate::coffre::{Coffre, CONSULTATION};
use droit::contexte::Contexte;
use socle::echec::erreur;
use crate::releve_retours::{mot_de_passe_de_consultation, Client, Panne};
use socle::echec::Echec;

type R<T> = Result<T, Echec>;

pub const ROUTE: &str = "coffre-verifier";

pub const NET_ENTREPRISES: &str = "net-entreprises";
pub const URSSAF: &str = "urssaf";
pub const COURRIEL: &str = "courriel";

/// De quoi ouvrir une session chez net-entreprises ou l'Urssaf : le service, et son mot de passe (`None` quand rien
/// n'est pose ; la variable d'environnement a deja ete consultee).
pub type Authentifier<'a> = &'a mut dyn FnMut(&Contexte, &str, Option<String>) -> Result<(), Panne>;

/// De quoi ouvrir la boite : `courriel.verifier` (une seule connexion IMAP).
pub type OuvrirBoite<'a> = &'a mut dyn FnMut(&Contexte) -> Result<Value, envois::courriel::ErreurCourriel>;

/// La route `coffre-verifier`.
pub fn route(ctx: &Contexte, charge: &Value) -> R<Value> {
    verifier_secret(ctx, charge.get("service"))
}

/// `operations_ouverture.verifier_secret`, avec les vraies connexions.
pub fn verifier_secret(ctx: &Contexte, service: Option<&Value>) -> R<Value> {
    verifier_secret_avec(ctx, service, &mut authentifier_pour_de_vrai, &mut |c| crate::boite::verifier(&c.racine))
}

/// L'authentification d'un service : le client de la DSN, ou celui de la DPAE (un autre compte).
fn authentifier_pour_de_vrai(ctx: &Contexte, service: &str, mdp: Option<String>) -> Result<(), Panne> {
    let mut client = if service == NET_ENTREPRISES { Client::dsn(ctx, "production", mdp)? } else { Client::dpae(ctx, false, mdp) };
    client.authentifier()
}

/// `operations_ouverture.verifier_secret`. Rend `{service, ok, message, conseil}` ; pour la boite, ce que
/// `courriel.verifier` dit, puis `service`.
pub fn verifier_secret_avec(ctx: &Contexte, service: Option<&Value>, authentifier: Authentifier, boite: OuvrirBoite) -> R<Value> {
    let nom = match service.and_then(Value::as_str) {
        Some(s @ (NET_ENTREPRISES | URSSAF | COURRIEL)) => s,
        _ => {
            let quoi = match service {
                Some(Value::String(s)) => format!("Service inconnu : « {} ».", s),
                _ => "La demande ne nomme pas de service.".to_string(),
            };
            return Err(erreur("ErreurOperation", format!("{} Connus : {}, {}, {}.", quoi, NET_ENTREPRISES, URSSAF, COURRIEL)));
        }
    };
    let coffre = Coffre::du_poste(&ctx.racine, "");
    if !coffre.disponible(nom).map_err(crate::routes_etats::vers_route)? {
        return Err(erreur("ErreurOperation", format!("Aucun mot de passe {} : rien a verifier.", nom)));
    }
    if nom == COURRIEL {
        // La boite : une connexion IMAP, une seule (`courriel.verifier`).
        let mut verdict = boite(ctx).map_err(|e| erreur("ErreurCourriel", e.0))?;
        if let Value::Object(o) = &mut verdict {
            o.insert("service".into(), json!(nom));
        }
        return Ok(verdict);
    }
    let mdp = mot_de_passe_de_consultation(ctx, nom)?;
    debug_assert!(CONSULTATION == "consultation");
    match authentifier(ctx, nom, mdp) {
        Ok(()) => Ok(json!({"service": nom, "ok": true, "message": "session ouverte"})),
        Err(p) if p.classe == "ErreurApi" => {
            let premiere = socle::texte::lignes_py(&p.message).first().copied().unwrap_or("").to_string();
            let conseil = if premiere.contains("422") {
                "422 = parametres invalides : le plus souvent un mot de passe tronque a la saisie, pas un compte fautif. Le reposer."
            } else if premiere.contains("403") {
                "403 = compte temporairement bloque (trop de tentatives). NE PAS REESSAYER, attendre."
            } else {
                ""
            };
            Ok(json!({"service": nom, "ok": false, "message": premiere, "conseil": conseil}))
        }
        Err(autre) => Err(autre.en_echec()),
    }
}
