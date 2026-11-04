//! LA LIGNE D'UN ENVOI PAR COURRIEL au journal des envois (`journal`) : les documents du mois, les contrats a la
//! signature, le dossier d'un salarie, la regularisation CASC-SVP, une demande de fiche. Un envoi qui echoue en route
//! note ce qui est quand meme parti.

use droit::contexte::Contexte;
use socle::echec::{erreur, Echec, R};
use socle::mois::Mois;

use crate::journal::{self, EnRoute, Envoi, Ligne, Mode, Version};


/// L'erreur d'un envoi : celle du client (`ErreurCourriel`), ou, quand des messages etaient deja partis, la
/// `RuntimeError` qui le dit.
pub fn echec_d_envoi(message: String) -> Echec {
    erreur(if message.contains("\nATTENTION, ") { "RuntimeError" } else { "ErreurCourriel" }, message)
}

/// L'envoi par courriel que `qui` fait du mois `mois`, en mode `reel` (une compagnie fictive aussi) : `versions`, celle de
/// chaque engagement qu'il emporte (`journal::versions_du_mois`).
pub fn par_courriel<D, Rt>(mois: Option<Mois>, qui: &str, accepte: bool, destinataire: &str, reponse: &str, detail: D, versions: Option<Vec<Version>>) -> Envoi<D, Rt> {
    let mut e = Envoi::nouveau(Mode::Reel, qui, accepte, destinataire, reponse, detail);
    e.mois = mois;
    e.versions = versions;
    e
}

/// La ligne d'un envoi par courriel, a la fin de `journal/depots.jsonl`.
pub fn noter(ctx: &Contexte, ligne: Ligne) -> R<Ligne> {
    journal::noter(&ctx.racine, ligne, None, &socle::paris::maintenant()).map_err(|m| erreur("OSError", m))
}

/// La ligne d'un envoi qui a echoue en route. Le destinataire est celui des messages REELLEMENT partis (`partis`, vide si
/// aucun) ; l'adresse ou l'envoi a echoue est a part, dans le detail (`echec_a`), avec les partis. Jamais l'expediteur
/// comme destinataire. `detail` : ce que l'envoi garde d'ordinaire ; `sorte`, la ligne de sa sorte. `versions` : celles
/// des engagements des salaries servis avant l'echec ; ce qui a atteint le salarie est parti (`departs_des_lignes`).
#[allow(clippy::too_many_arguments)]
pub fn noter_echec_d_envoi<D: EnRoute, Rt>(
    ctx: &Contexte,
    sorte: impl FnOnce(Envoi<D, Rt>) -> Ligne,
    mois: Option<Mois>,
    qui: &str,
    partis: &[String],
    echoue: &str,
    message: &str,
    mut detail: D,
    versions: Option<Vec<Version>>,
) -> R<Ligne> {
    detail.coupe(partis.to_vec(), echoue.to_string());
    let reponse = if partis.is_empty() { format!("echec de l'envoi a {} : {}", echoue, message) } else { message.to_string() };
    noter(ctx, sorte(par_courriel(mois, qui, false, &partis.join(", "), &reponse, detail, versions)))
}
