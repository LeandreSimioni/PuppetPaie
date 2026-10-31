//! `moteur casc-envoyer --vire-le ANNEE=AAAA-MM-JJ [--vire-le ...] [--fenetre]` : envoyer les deux courriels de regularisation
//! CASC-SVP (Audiens, et le CASC-SVP avec sa liste nominative), APRES les virements. Route `casc-envoyer`, celle que la fenetre appelle.
//!
//! UN ENVOI QUI NE SE RATTRAPE PAS. Les garde-fous sont ceux du moteur, jamais ceux de la commande :
//! - la route refait le dossier (la liste nominative est celle du jour), ouvre la boite, puis exige la PREUVE
//!   (`depot_acte::verifier_preuve`) : le moteur demande LUI-MEME le mot de passe du logiciel (fenetre, terminal sans echo, sinon
//!   refus). La commande ne le lit, ne le recoit et ne l'affiche jamais ; `--fenetre` est accepte et sans effet ;
//! - dans `moteur-atelier`, `verifier_preuve` refuse tout acte reel (`ActeReelRefuse`) ;
//! - les dates des virements sont verifiees par la route (lisibles, pas dans le futur, une au moins), avant toute demande ;
//! - la route journalise (`qui` = `ligne de commande`), garde les deux `.eml` et ne note les annees
//!   regularisees qu'APRES l'envoi.
//!
//! Une panne de la boite en cours d'envoi (`ErreurCourriel`, ou le `RuntimeError` qui dit quels messages etaient deja partis) :
//! `! phrase`, code 2.

use serde_json::{json, Map};

use super::actes::{est_refus_d_envoi, ouvrir_la_boite, options_requises, reponse_metier};
use super::analyse::{Args, Opt, Spec};
use super::rendu::champ;
use super::Cadre;
use socle::echec::Echec;

pub static SPEC: Spec = Spec { commande: "casc-envoyer", positionnels: &[], options: &[Opt::ajout("vire-le"), Opt::drapeau("fenetre")] };

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    if let Some(code) = options_requises(&SPEC, args, &["vire-le"]) {
        return code;
    }
    match faire(cadre, args) {
        Ok(c) => c,
        Err(e) if est_refus_d_envoi(e.classe) => reponse_metier("casc-envoyer", false, &e, &[]),
        Err(e) => cadre.plantage(e),
    }
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    // Chaque `annee=date` : la derniere date d'une annee l'emporte.
    let mut dates = Map::new();
    for v in args.liste("vire-le") {
        match v.split_once('=') {
            Some((annee, date)) => {
                dates.insert(annee.to_string(), json!(date));
            }
            None => {
                dire!("! --vire-le s'ecrit ANNEE=AAAA-MM-JJ (ex. 2025=2026-09-23).");
                return Ok(2);
            }
        }
    }
    ouvrir_la_boite(cadre);
    let r = cadre.route("casc-envoyer", json!({"vire_le": dates, "qui": "ligne de commande"}))?;
    let partis = super::actes::textes_de(&r, "partis");
    dire!(
        "CASC-SVP {} -- {} EUR : {} courriel(s) envoye(s) depuis {}, a {}.",
        champ(&r, "periode"),
        champ(&r, "total"),
        partis.len(),
        champ(&r, "expediteur"),
        partis.join(", ")
    );
    for a in r.get("annees").and_then(serde_json::Value::as_array).cloned().unwrap_or_default() {
        let nb = a.get("mois").and_then(serde_json::Value::as_array).map_or(0, Vec::len);
        dire!("  {} notee regularisee ({} mois, vire le {}).", champ(&a, "annee"), nb, champ(&a, "vire_le"));
    }
    dire!("Copies : {}", champ(&r, "dossier"));
    Ok(0)
}
