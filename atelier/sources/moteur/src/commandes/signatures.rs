//! `moteur signatures [--mois AAAA-MM] [--json]` : relever maintenant les reponses « Je signe » dans la boite courriel.
//! Route `signatures-relever` : la releve, le journal, la preuve archivee, le
//! contrat contresigne et son renvoi. `qui` : `ligne de commande`.
//!
//! Code de sortie : 1 quand la releve a rencontre un incident, 0 sinon.
//!
//! GARDE-FOUS. La releve LIT la boite (IMAP) et, pour chaque signature trouvee, renvoie au salarie le contrat contresigne : c'est ce
//! que fait la veille a chaque ouverture, sans mot de passe (la signature est le geste du salarie, pas une
//! decision de l'employeur ; l'envoi est la reponse a un « Je signe » deja recu, pour un contrat deja envoye apres le mot de passe
//! du logiciel). La commande n'ajoute donc pas de preuve ; elle n'enleve rien non plus : la boite est ouverte par le moteur,
//! le coffre par la cle du poste ou une seance. Dans `moteur-atelier`, la boite courriel n'est pas redirigee vers un faux serveur :
//! la commande y est REFUSEE hors compagnie fictive (`ActeReelRefuse`), pour qu'un essai ne touche jamais une vraie boite.
//!
//! Un refus de la route (un fichier qui ne s'ecrit pas, une panne de la boite) : `! phrase`, code 2.

use super::actes::{est_refus_d_envoi, ouvrir_la_boite, reponse_metier};
use super::analyse::{Args, Opt, Spec};
use super::rendu::{champ, enveloppe, rendre};
use super::Cadre;
use socle::echec::Echec;
use serde_json::{json, Value};

pub static SPEC: Spec = Spec { commande: "signatures", positionnels: &[], options: &[Opt::valeur("mois"), Opt::drapeau("json")] };

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    let json = args.drapeau("json");
    match faire(cadre, args) {
        Ok(c) => c,
        Err(e) if est_refus_d_envoi(e.classe) => reponse_metier("signatures", json, &e, &[]),
        Err(e) => cadre.plantage(e),
    }
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let json = args.drapeau("json");
    #[cfg(feature = "atelier")]
    if !crate::compagnie::bridage::branchement(&cadre.racine).joue() {
        return Err(Echec { classe: Some("ActeReelRefuse"), message: crate::atelier::redirection::REFUS_ACTE_REEL.to_string() });
    }
    ouvrir_la_boite(cadre);
    let mut charge = json!({"qui": "ligne de commande"});
    if let Some(m) = args.texte("mois").filter(|m| !m.is_empty()) {
        charge["mois"] = json!(m);
    }
    let mut r = cadre.route("signatures-relever", charge)?;
    let recues: Vec<Value> = r.get("recues").and_then(Value::as_array).cloned().unwrap_or_default();
    let incidents: Vec<String> = super::actes::textes_de(&r, "incidents");
    let attendues = r.get("attendues").and_then(Value::as_i64).unwrap_or(0);
    let code = if incidents.is_empty() { 0 } else { 1 };
    if json {
        // `_signatures_json` : sans le `resume`, que la route ajoute pour l'ouverture.
        if let Value::Object(o) = &mut r {
            o.remove("resume");
        }
        rendre(&enveloppe("signatures", r));
        return Ok(code);
    }
    if attendues == 0 && recues.is_empty() {
        dire!("Aucune signature attendue.");
        return Ok(0);
    }
    dire!("{}", champ(&r, "resume"));
    for x in &recues {
        let quand: String = champ(x, "quand").replace('T', " ").chars().take(16).collect();
        let renvoye = if x.get("renvoye").is_some_and(socle::fichiers::vrai) { " -- contrat contresigne renvoye" } else { "" };
        dire!("      {}  signe le {} depuis {} ({}){}", champ(x, "nom"), quand, champ(x, "adresse"), champ(x, "code"), renvoye);
    }
    for i in &incidents {
        dire!("  ! {}", i);
    }
    Ok(code)
}
