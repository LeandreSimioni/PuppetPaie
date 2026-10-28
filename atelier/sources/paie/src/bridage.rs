//! LE BRIDAGE (lot U1, 14/10/2026). Il n'y a qu'un logiciel, BRIDE ou DEBLOQUE ; Leandre : « Il n'y a pas de version
//! demonstration. C'est toujours le meme logiciel qui est bride ou pas bride. » Bride, l'accueil ne propose que les
//! compagnies fictives, et son « + » ajoute une FICTIVE AJOUTEE (ticket #236, `compagnies::ajouter`) dont les envois sont
//! refuses (`refus_d_envoi`) ; debloque, il propose aussi les compagnies reelles. Rien d'autre ne change :
//! le meme moteur, la meme mise a jour (le bridage n'y joue aucun role, `maj.rs`).
//!
//! BRIDE TANT QUE RIEN NE DIT QU'IL EST DEBLOQUE (ticket #246, Leandre 22/10/2026 : « Le logiciel ne doit jamais etre
//! debloque par defaut »). Une installation neuve -- l'installeur, une archive de la Release decompressee dans un dossier
//! neuf, un `logiciel/` dont un fichier aurait disparu -- part bridee ; seul un fichier qui dit « debloque » la debloque.
//!
//! Deux fichiers le disent, lus ici et nulle part ailleurs :
//! - `.edition.json`, a cote de `logiciel/` : ce que le geste de l'accueil a retenu sur ce poste, `"edition": "bride"` ou
//!   `"debloque"`. C'est le seul chemin du deblocage (demain aussi celui du ticket #236).
//! - `logiciel/EDITION.json`, `"edition": "bride"` : le PAQUET SOUDE, bride pour toujours, dont les envois sont joues et
//!   qui ne poste a l'editeur que ses incidents ; le geste n'y fait rien. Propre a l'installation : la mise a jour ne le
//!   touche pas (`maj.rs`, `PROPRES_A_L_INSTALLATION`), aucune archive n'en porte.
//!
//! Avant le ticket #246, `logiciel/EDITION.json` pouvait aussi dire `"defaut": "bride"` (l'installeur Windows le posait) et,
//! sans rien, le logiciel partait debloque. Le defaut est desormais bride : la cle `defaut` ne se lit plus, et l'installeur
//! ne pose plus de fichier. Une installation debloquee par le geste garde son `.edition.json` ; une installation qui
//! n'etait debloquee que faute de fichier devient bridee.
//!
//! Les mots d'avant le lot U1 se lisent encore : `"demo"` s'y lit « bride », `"complete"` « debloque ». Le geste ecrit les
//! mots neufs.
//!
//! L'INSTALLATION dont on parle : celle du logiciel qui tourne (`compagnies::base()`), ou celle qu'on nomme (`_de`) --
//! l'administrateur et la liste des compagnies lisent l'installation qu'on leur donne.

use std::path::Path;

use serde_json::{json, Value};

const FICHIER_GESTE: &str = ".edition.json";
const FICHIER_INSTALLATION: &str = "EDITION.json";
pub const BRIDE: &str = "bride";
pub const DEBLOQUE: &str = "debloque";

/// Un mot du bridage : `Some(true)` bride, `Some(false)` debloque ; les mots d'avant le lot U1 se lisent encore.
fn bride_selon(mot: &str) -> Option<bool> {
    match mot {
        BRIDE | "demo" => Some(true),
        DEBLOQUE | "complete" => Some(false),
        _ => None,
    }
}

fn lire(p: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()
}

fn mot(v: &Option<Value>, cle: &str) -> Option<bool> {
    v.as_ref().and_then(|d| d.get(cle)).and_then(Value::as_str).and_then(bride_selon)
}

/// Le PAQUET SOUDE : `logiciel/EDITION.json` dit `"edition": "bride"` (ou `"demo"`). Bride pour toujours ; ses envois
/// sont joues.
pub fn paquet_soude() -> bool {
    paquet_soude_de(&crate::interface::compagnies::base())
}

/// Le paquet soude de l'installation `inst` (le dossier qui porte `logiciel/`).
pub fn paquet_soude_de(inst: &Path) -> bool {
    mot(&lire(&inst.join("logiciel").join(FICHIER_INSTALLATION)), "edition") == Some(true)
}

/// Bride ou debloque, pour le logiciel qui tourne (`est_bride_de`).
pub fn est_bride() -> bool {
    est_bride_de(&crate::interface::compagnies::base())
}

/// Bride ou debloque, pour l'installation `inst` : le paquet soude reste bride ; sinon ce que le geste a retenu sur le
/// poste (`.edition.json`) ; SINON BRIDE (ticket #246) -- rien d'autre ne debloque.
pub fn est_bride_de(inst: &Path) -> bool {
    if paquet_soude_de(inst) {
        return true;
    }
    mot(&lire(&inst.join(FICHIER_GESTE)), "edition").unwrap_or(true)
}

/// LE REFUS D'UN ENVOI DEPUIS UNE FICTIVE AJOUTEE (ticket #236, Leandre 22/10/2026), mot pour mot : le meme pour tout geste
/// qui ferait partir quelque chose vers un organisme ou une personne. Sa classe : `CLASSE_REFUS_D_ENVOI` (la page la
/// reconnait et montre la phrase dans sa boite, `js/tuyau.js`).
pub const REFUS_D_ENVOI: &str = "L'envoi n'est pas possible depuis la version démo. Pour 2027, et afin d'offrir un service de qualité, le nombre d'utilisateurs est restreint. N'hésitez pas à contacter Léandre (puppetpaie@gmail.com) avec une courte présentation et votre motivation pour débloquer le logiciel et faire partie des premiers utilisateurs.";
pub const CLASSE_REFUS_D_ENVOI: &str = "EnvoiBride";

/// UNE FICTIVE AJOUTEE N'ENVOIE RIEN (ticket #236) : `Some(REFUS_D_ENVOI)` pour la compagnie `racine` si elle en est une
/// (`compagnies::est_ajoutee`), `None` sinon. Tout le refus tient ici ; deux portes l'appellent, avant que rien ne
/// s'ecrive au journal des depots : `depot_acte::refuser_l_envoi` (les depots DSN et DPAE, test et reel, signalements
/// compris, quel qu'en soit l'appelant : page, ligne de commande, filet, administrateur) et `routes_envois::client_courriel`
/// (la boite de tout courriel envoye a quelqu'un) ; s'y ajoutent les deux envois qui ouvrent leur propre chemin : le
/// contrat contresigne (`releve_signatures`) et l'inscription au filet (`filet::activer`, dont le serveur ecrit a
/// l'adresse). Ce qui va a l'editeur (incidents, « Un probleme, une idee ? ») et la verification d'un mot de passe ne
/// sont pas des envois : ils restent permis.
pub fn refus_d_envoi(racine: &Path) -> Option<&'static str> {
    branchement(racine).refus().map(|r| r.phrase)
}

/// LE BRANCHEMENT DE LA COMPAGNIE `racine` (`envois::branchement`), choisi ici et nulle part ailleurs :
/// - une FICTIVE AJOUTEE (`compagnies::est_ajoutee`) n'envoie rien : le message, `REFUS_D_ENVOI` ;
/// - une compagnie FICTIVE (son marqueur, `compagnies::est_fictive`), ou toute compagnie du PAQUET SOUDE
///   (`paquet_soude`) : le simulateur ;
/// - toute autre : le monde. Le logiciel simplement bride ne joue rien.
pub fn branchement(racine: &Path) -> envois::branchement::Branchement {
    use crate::interface::compagnies::{est_ajoutee, est_fictive};
    use envois::branchement::{Branchement, Refus};
    if est_ajoutee(racine) {
        Branchement::Message(Refus { classe: CLASSE_REFUS_D_ENVOI, phrase: REFUS_D_ENVOI })
    } else if paquet_soude() || est_fictive(racine) {
        Branchement::Simulateur
    } else {
        Branchement::Monde
    }
}

/// Le mot qui le dit (le journal des incidents, la description du poste) : `bride` ou `debloque`.
pub fn etat() -> &'static str {
    if est_bride() {
        BRIDE
    } else {
        DEBLOQUE
    }
}

/// Le geste secret de l'accueil : bride <-> debloque, retenu sur le poste. Sans effet sur un paquet soude.
pub fn basculer() -> Result<(), String> {
    basculer_de(&crate::interface::compagnies::base())
}

/// Le geste, sur l'installation `inst`.
pub fn basculer_de(inst: &Path) -> Result<(), String> {
    if paquet_soude_de(inst) {
        return Ok(());
    }
    let nouveau = if est_bride_de(inst) { DEBLOQUE } else { BRIDE };
    socle::ecriture::ecrire_json(
        &inst.join(FICHIER_GESTE),
        &json!({"edition": nouveau, "_quoi": "Le bridage de PuppetPaie sur ce poste : bride (l'accueil ne propose que les compagnies fictives) ou debloque. Change par le geste de l'accueil."}),
        2,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_mots_d_avant_se_lisent_encore() {
        assert_eq!(bride_selon("bride"), Some(true));
        assert_eq!(bride_selon("demo"), Some(true), "une installation posee avant le lot U1");
        assert_eq!(bride_selon("debloque"), Some(false));
        assert_eq!(bride_selon("complete"), Some(false), "un geste retenu avant le lot U1");
        assert_eq!(bride_selon("autre"), None);
    }
}
