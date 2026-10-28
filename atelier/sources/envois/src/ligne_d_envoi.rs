//! LA LIGNE D'UN ENVOI PAR COURRIEL au journal des envois (`journal`) : les documents du mois, les contrats a la
//! signature, le dossier d'un salarie, la regularisation CASC-SVP, une demande de fiche. Un envoi qui echoue en route
//! note ce qui est quand meme parti.

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use socle::echec::{erreur, Echec, R};

use crate::journal::{self, Note, REEL};


/// L'erreur d'un envoi : celle du client (`ErreurCourriel`), ou, quand des messages etaient deja partis, la
/// `RuntimeError` qui le dit.
pub fn echec_d_envoi(message: String) -> Echec {
    erreur(if message.contains("\nATTENTION, ") { "RuntimeError" } else { "ErreurCourriel" }, message)
}

/// La ligne d'un envoi par courriel, a la fin de `journal/depots.json`, en mode `reel` (une compagnie fictive aussi).
#[allow(clippy::too_many_arguments)]
pub fn noter(
    ctx: &Contexte,
    quoi: &str,
    mois: Option<&str>,
    qui: &str,
    accepte: bool,
    destinataire: &str,
    reponse: &str,
    detail: Option<&Value>,
) -> R<Value> {
    noter_envoi(ctx, quoi, mois, qui, accepte, destinataire, reponse, detail, None)
}

/// `noter`, pour un envoi parti : la ligne porte la version de chaque engagement qu'il emporte (`versions_du_mois`).
#[allow(clippy::too_many_arguments)]
pub fn noter_envoi(
    ctx: &Contexte,
    quoi: &str,
    mois: Option<&str>,
    qui: &str,
    accepte: bool,
    destinataire: &str,
    reponse: &str,
    detail: Option<&Value>,
    versions: Option<&Value>,
) -> R<Value> {
    let note = Note {
        quoi,
        mois,
        fichier: None,
        mode: REEL,
        qui,
        accepte,
        destinataire,
        flux: "",
        reponse,
        trace: None,
        detail,
        versions,
        quand: "",
        origine: "",
    };
    journal::noter(&ctx.racine, &note, &socle::paris::maintenant()).map_err(|m| erreur("OSError", m))
}

/// La ligne d'un envoi qui a echoue en route. Le destinataire est celui des messages REELLEMENT partis (`partis`, vide si
/// aucun) ; l'adresse ou l'envoi a echoue est a part, dans le detail (`echec_a`), avec les partis. Jamais l'expediteur
/// comme destinataire. `detail` : ce que l'envoi garde d'ordinaire. `versions` : celles des engagements des salaries
/// servis avant l'echec ; ce qui a atteint le salarie est parti (`departs_des_lignes`).
#[allow(clippy::too_many_arguments)]
pub fn noter_echec_d_envoi(
    ctx: &Contexte,
    quoi: &str,
    mois: Option<&str>,
    qui: &str,
    partis: &[String],
    echoue: &str,
    message: &str,
    detail: Option<Value>,
    versions: Option<&Value>,
) -> R<Value> {
    let mut d = match detail {
        Some(Value::Object(m)) => m,
        _ => Map::new(),
    };
    d.insert("partis".into(), json!(partis));
    d.insert("echec_a".into(), json!(echoue));
    let reponse = if partis.is_empty() { format!("echec de l'envoi a {} : {}", echoue, message) } else { message.to_string() };
    noter_envoi(ctx, quoi, mois, qui, false, &partis.join(", "), &reponse, Some(&Value::Object(d)), versions)
}

