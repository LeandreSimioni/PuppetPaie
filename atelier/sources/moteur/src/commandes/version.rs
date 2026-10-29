//! `moteur version [--json]` : la version du logiciel, et ce qui est pret (`moteur/commandes_maj.py`, `cmd_version`). La route
//! `maj-etat` fait le travail. `--controle` (il verifiait que le code Python s'importe en entier) n'a plus d'objet : le
//! moteur est un seul programme.

use serde_json::json;

use super::analyse::{Args, Opt, Spec};
use super::rendu::{champ, enveloppe, rendre};
use super::Cadre;
use socle::echec::Echec;

pub static SPEC: Spec = Spec { commande: "version", positionnels: &[], options: &[Opt::drapeau("json")] };

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    match faire(cadre, args) {
        Ok(c) => c,
        Err(e) => cadre.plantage(e),
    }
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    #[cfg_attr(not(feature = "atelier"), allow(unused_mut))]
    let mut v = cadre.route("maj-etat", json!({}))?;
    // Le binaire de l'atelier le dit, en toutes lettres et en JSON (`atelier.rs`) ; `moteur` ne porte rien de tel.
    #[cfg(feature = "atelier")]
    if let serde_json::Value::Object(o) = &mut v {
        o.insert("atelier".into(), json!(true));
    }
    if args.drapeau("json") {
        rendre(&enveloppe("version", v));
        return Ok(0);
    }
    let vrai = |k: &str| v.get(k).is_some_and(socle::fichiers::vrai);
    let lue = |k: &str| crate::compagnie::maj::version_lue(&champ(&v, k));
    dire!("PuppetPaie {}{}", lue("courante"), if vrai("editeur") { " (poste de l'editeur)" } else { "" });
    #[cfg(feature = "atelier")]
    dire!("  BINAIRE DE L'ATELIER (moteur-atelier) : pour les essais seulement, jamais livre ; il refuse tout acte reel (depot reel, envoi, signature)");
    dire!("  depot : {}", if vrai("depot") { champ(&v, "depot") } else { "aucun (maj/reglage.json)".to_string() });
    if vrai("prete") && vrai("exigee") {
        dire!(
            "  prete : {} -- INDISPENSABLE avant de declarer ({}) : aucun depot ne part avant qu'elle soit appliquee (redemarrer le logiciel)",
            lue("prete"),
            champ(&v, "exigee")
        );
    } else if vrai("prete") {
        dire!("  prete : {} -- s'applique au prochain demarrage", lue("prete"));
    }
    if vrai("prete") && vrai("calcul") {
        dire!("  elle change un calcul de paie : {}", champ(&v, "calcul"));
    }
    dire!("  archives : {}", if vrai("signee") { "signees par l'editeur" } else { "NON signees (maj.CLE_PUBLIQUE vide)" });
    if vrai("avant") {
        dire!("  avant : {} dans .avant/ -- la mise a jour peut y revenir", lue("avant"));
    }
    Ok(0)
}
