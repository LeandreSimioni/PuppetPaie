//! `moteur-atelier journal --dpae [--ecrire]` : remettre au journal des envois les DPAE que les certificats de l'Urssaf
//! (nature 41, `retours/dpae/`) attestent (`envois::reprise_journal::reprise_dpae`) -- des DPAE faites par un autre
//! logiciel ou un autre poste. Sans `--ecrire`, l'outil DIT ce qu'il ferait. Rien ne part, aucun reseau. La reprise des
//! DSN est la route `journal-reprise`.

use serde_json::Value;

use crate::commandes::analyse::{Args, Opt, Spec};
use crate::commandes::rendu::{champ, texte};
use crate::commandes::Cadre;
use socle::echec::Echec;

pub static SPEC: Spec = Spec { commande: "journal", positionnels: &[], options: &[Opt::drapeau("dpae"), Opt::drapeau("ecrire")] };

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    if !args.drapeau("dpae") {
        dire!("journal --dpae [--ecrire] : la reprise des DPAE attestees. Les DSN : la route journal-reprise.");
        return 2;
    }
    faire(cadre, args.drapeau("ecrire")).unwrap_or_else(|e| super::refus(&e))
}

fn liste(v: &Value, cle: &str) -> Vec<Value> {
    v.get(cle).and_then(Value::as_array).cloned().unwrap_or_default()
}

fn faire(cadre: &Cadre, ecrire: bool) -> Result<i32, Echec> {
    let maintenant = socle::paris::iso_de_naif(socle::paris::maintenant_naif());
    let d = envois::reprise_journal::reprise_dpae(&cadre.ctx, ecrire, "atelier", &maintenant)?;
    let (trouves, ajoutes) = (liste(&d, "trouves"), liste(&d, "ajoutes"));
    for x in liste(&d, "inconnus") {
        dire!("Non rattache : {}", texte(&x));
    }
    if trouves.is_empty() {
        dire!("Aucun certificat de DPAE (nature 41) dans retours/dpae/ : rien a reprendre.");
        return Ok(0);
    }
    dire!("DPAE attestees par les certificats de l'Urssaf : {}\n", trouves.len());
    for v in &trouves {
        let deja = v.get("deja").is_some_and(documents::valeur::vrai);
        dire!("  {}  {:<4}  {:<8}  {}{}", champ(v, "quand"), champ(v, "mode"), champ(v, "mois"), champ(v, "flux"), if deja { "   [deja au journal]" } else { "" });
        for e in liste(v, "embauches") {
            dire!("        {} a partir du {}", champ(&e, "salarie"), champ(&e, "debut"));
        }
    }
    if ajoutes.is_empty() {
        dire!("\nRien de nouveau : le journal connait deja tout cela.");
    } else if ecrire {
        dire!("\n{} ligne(s) ajoutee(s) au journal.", ajoutes.len());
    } else {
        dire!("\n{} ligne(s) SERAIENT ajoutees. Rien n'a ete ecrit : --ecrire les pose.", ajoutes.len());
    }
    Ok(0)
}
