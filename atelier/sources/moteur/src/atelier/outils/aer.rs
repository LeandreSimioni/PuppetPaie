//! `moteur-atelier aer [--mois AAAA-MM] [--forcer]` : ranger de nouveau les attestations employeur de France Travail deja
//! recues (nature 46, `retours/`) -- apres qu'une fiche a ete corrigee, ou pour signer malgre un ecart (`--forcer` :
//! l'ecart est connu et assume). Le travail est celui de la releve (`routes::documents::ranger_aer_detail`), qui ne force
//! jamais ; ici l'editeur le force. On ne touche jamais aux chiffres du PDF : on remplit le cadre de l'employeur, et une
//! attestation qui ne dit pas ce que la paie dit ne se signe pas sans `--forcer` (code du travail, art. L. 5429-1).

use crate::commandes::analyse::{Args, Opt, Spec};
use crate::commandes::Cadre;
use envois::retours::NATURE_AER_SPECTACLE;
use socle::echec::Echec;

pub static SPEC: Spec = Spec { commande: "aer", positionnels: &[], options: &[Opt::valeur("mois"), Opt::drapeau("forcer")] };

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    faire(cadre, args).unwrap_or_else(|e| super::refus(&e))
}

fn nom(p: &std::path::Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let mois = args.texte("mois");
    let fait = crate::routes::documents::ranger_aer_detail(&cadre.ctx, mois, None, args.drapeau("forcer"))?;
    if fait.ecrits.is_empty() && fait.a_ranger.is_empty() && fait.refusees.is_empty() {
        dire!("Aucune attestation{} dans les retours de nature {}.", mois.map(|m| format!(" pour {}", m)).unwrap_or_default(), NATURE_AER_SPECTACLE);
        return Ok(2);
    }
    for (periode, cle, chemin) in &fait.ecrits {
        dire!("{}  {:<12} {}", periode, cle, chemin.display());
    }
    if !fait.a_ranger.is_empty() {
        dire!("\n{} attestation(s) non rattachee(s) a une fiche (le nom et le prenom ne se trouvent pas) :", fait.a_ranger.len());
        for (qui, chemin) in &fait.a_ranger {
            dire!("   {}  ({})", qui, nom(chemin));
        }
    }
    if !fait.refusees.is_empty() {
        dire!("\n{} attestation(s) NON SIGNEE(S) : elles ne disent pas ce que la paie dit.", fait.refusees.len());
        for (cle, periode, ecarts) in &fait.refusees {
            dire!("   {}, {}", cle, periode);
            for e in ecarts {
                dire!("      {}", e);
            }
        }
        dire!("\nLa correction se fait sur la DSN, et France Travail reedite l'attestation. --forcer signe quand meme.");
    }
    Ok(if fait.refusees.is_empty() { 0 } else { 2 })
}
