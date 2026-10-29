//! `moteur compta-logiciel [LOGICIEL|aucun] [--analytique]` : le logiciel du comptable, retenu pour les ecritures comptables, et ce
//! que le plan comptable ne tiendrait pas dans son format. Sans argument :
//! ce qui est retenu (lecture seule) ; avec : route `retenir-logiciel` (ecrit `config/compta.json` -- un reglage, pas un depot : pas de
//! mot de passe). `aucun` retire le choix. La sortie est toujours le texte : `--json` est accepte et sans effet.
//!
//! Code 0 ; un refus (`ErreurOperation`) : `! phrase`, code 2.
//!
//! Le champ `_quoi` de `config/compta.json` dit `moteur compta-logiciel` : c'est un commentaire que rien ne lit.

use serde_json::{json, Value};

use super::actes::{est_refus, reponse_metier};
use super::analyse::{Args, Opt, Pos, Spec};
use super::exporter::LOGICIELS;
use super::Cadre;
use socle::echec::Echec;

const CHOIX: &[&str] = &["oxygene", "sage", "quadra", "ciel", "ebp", "pennylane", "paheko", "acd", "loop", "isacompta", "autre", "aucun"];

pub static SPEC: Spec = Spec {
    commande: "compta-logiciel",
    positionnels: &[Pos::parmi("logiciel", CHOIX, None)],
    options: &[Opt::drapeau("analytique"), Opt::drapeau("json")],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    debug_assert!(LOGICIELS.iter().all(|l| CHOIX.contains(l)));
    match faire(cadre, args) {
        Ok(c) => c,
        Err(e) if est_refus(e.classe) || e.classe == Some("CalculRefuse") => reponse_metier("compta-logiciel", false, &e, &[]),
        Err(e) => cadre.plantage(e),
    }
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let e: Value = match args.texte("logiciel").filter(|t| !t.is_empty()) {
        Some(l) => {
            let logiciel = if l == "aucun" { Value::Null } else { json!(l) };
            cadre.route("retenir-logiciel", json!({"logiciel": logiciel, "analytique": args.drapeau("analytique")}))?
        }
        None => crate::routes::archives::etat_logiciel(&cadre.ctx)?,
    };
    let retenu = e.get("logiciel").and_then(Value::as_str).unwrap_or("");
    let nom = e.get("logiciels").and_then(Value::as_array).and_then(|l| l.iter().find(|x| x.get("logiciel").and_then(Value::as_str) == Some(retenu)));
    match nom {
        Some(n) if !retenu.is_empty() => dire!("Logiciel du comptable : {} (format {})", super::rendu::champ(n, "nom"), super::rendu::champ(n, "format")),
        _ => dire!("Logiciel du comptable : aucun -- le format se choisit"),
    }
    dire!("Analytique par spectacle : {}", if e.get("analytique").is_some_and(socle::fichiers::vrai) { "oui" } else { "non" });
    for m in super::actes::textes_de(&e, "controle") {
        dire!("  ! {}", m);
    }
    Ok(0)
}
