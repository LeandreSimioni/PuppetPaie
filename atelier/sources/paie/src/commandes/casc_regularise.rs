//! `paie casc-regularise ANNEE --vire-le AAAA-MM-JJ --montant EUROS` : noter qu'une annee de CASC-SVP est regularisee, apres le
//! virement (`moteur/commandes_exports.py`, `cmd_casc_regularise` ; `operations_casc.marquer_regularise`). Route `casc-regularise`.
//! Rien ne part : c'est une ligne dans `config/casc-svp.json` ; l'envoi des courriels est `paie casc-envoyer`.
//!
//! `qui` : `ligne de commande`, comme le Python.
//!
//! DEFAUTS DU PYTHON, QUI NE SE REPRODUISENT PAS : la date du virement et le montant s'ecrivaient tels quels, lisibles ou non (une
//! date `hier`, un montant `abc` etaient notes, puis un `InvalidOperation` plantait le dossier suivant) ; une annee sans rien a
//! regulariser (deja notee, ou aucune DSN deposee) s'ecrivait quand meme, avec zero mois. Ici : une phrase (`! ...`), code 2,
//! et rien n'est ecrit.

use serde_json::json;

use super::actes::{est_refus, jour_option, reponse_metier};
use super::analyse::{Args, Opt, Pos, Spec};
use super::rendu::champ;
use super::Cadre;
use socle::echec::Echec;

pub static SPEC: Spec = Spec {
    commande: "casc-regularise",
    positionnels: &[Pos::requis("annee")],
    options: &[Opt::valeur("vire-le"), Opt::valeur("montant")],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    if let Some(code) = super::actes::options_requises(&SPEC, args, &["vire-le", "montant"]) {
        return code;
    }
    match faire(cadre, args) {
        Ok(c) => c,
        Err(e) if est_refus(e.classe) => reponse_metier("casc-regularise", false, &e, &[]),
        Err(e) => cadre.plantage(e),
    }
}

fn refus(m: String) -> Echec {
    Echec { classe: Some("ErreurOperation"), message: m }
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let annee = args.texte("annee").unwrap_or("");
    let vire_le = args.texte("vire-le").unwrap_or("");
    let montant = args.texte("montant").unwrap_or("");
    jour_option("vire-le", vire_le)?;
    match socle::num::dec(&json!(montant.trim())) {
        Ok(d) if d > socle::num::Decimal::ZERO => {}
        _ => return Err(refus(format!("--montant attend le montant vire, en euros (un nombre positif), recu {}.", socle::fichiers::repr(montant)))),
    }
    // Rien a regulariser : le Python notait quand meme une ligne sans mois.
    let r = crate::casc_regularisation::regularisation(
        &cadre.ctx,
        &crate::casc_regularisation::Demande { annee: Some(annee.to_string()), ..Default::default() },
    )?;
    if r.mois.is_empty() {
        return Err(refus(format!("Rien a regulariser pour {} (deja fait, ou aucune DSN deposee). Rien n'est note.", annee)));
    }
    let e = cadre.route("casc-regularise", json!({"annee": annee, "vire_le": vire_le, "montant": montant, "qui": "ligne de commande"}))?;
    let nb = e.get("mois").and_then(serde_json::Value::as_array).map_or(0, Vec::len);
    dire!(
        "CASC-SVP {} notee regularisee : {} mois, {} EUR vires le {} (calcule : {}).",
        champ(&e, "annee"),
        nb,
        champ(&e, "montant"),
        champ(&e, "vire_le"),
        champ(&e, "calcule")
    );
    Ok(0)
}
