//! LE BRIDAGE. Il n'y a qu'un logiciel, BRIDE ou DEBLOQUE, et pas de version de demonstration a part : c'est toujours
//! le meme logiciel, bride ou pas. Bride, l'accueil ne propose que les
//! compagnies fictives, et son « + » ajoute une FICTIVE AJOUTEE (`compagnies::ajouter`) dont les envois sont
//! refuses (`branchement` : le message) ; debloque, il propose aussi les compagnies reelles. Rien d'autre ne change :
//! le meme moteur, la meme mise a jour (le bridage n'y joue aucun role, `compagnie::maj`).
//!
//! BRIDE TANT QUE RIEN NE DIT QU'IL EST DEBLOQUE : le logiciel n'est jamais debloque par defaut. Une installation neuve -- l'installeur, une archive de la Release decompressee dans un dossier
//! neuf, un `logiciel/` dont un fichier aurait disparu -- part bridee ; seul un fichier qui dit « debloque » la debloque.
//!
//! Deux fichiers le disent, lus ici et nulle part ailleurs :
//! - `.edition.json`, a cote de `logiciel/` : ce que le geste de l'accueil a retenu sur ce poste, `"edition": "bride"` ou
//!   `"debloque"`. C'est le seul chemin du deblocage.
//! - `logiciel/EDITION.json`, `"edition": "bride"` : le PAQUET SOUDE, bride pour toujours, dont les envois sont joues et
//!   qui ne poste a l'editeur que ses incidents ; le geste n'y fait rien. Propre a l'installation : la mise a jour ne le
//!   touche pas (`compagnie::maj`, `PROPRES_A_L_INSTALLATION`), aucune archive n'en porte.
//!
//! Une cle `defaut` dans `logiciel/EDITION.json` ne se lit pas, et l'installeur ne pose aucun fichier.
//!
//! Le geste ecrit `"bride"` ou `"debloque"`. `"demo"` se lit aussi « bride », et `"complete"` « debloque » : des
//! installations posees et debloquees portent ces mots ; les lire autrement les briderait et bloquerait leurs envois.
//! C'est le contrat avec les installations posees, comme la forme de la mise a jour.
//!
//! L'INSTALLATION dont on parle : celle du logiciel qui tourne (`compagnies::base()`), ou celle qu'on nomme --
//! l'administrateur et la liste des compagnies lisent l'installation qu'on leur donne.

use std::path::Path;

use serde_json::{json, Value};

const FICHIER_GESTE: &str = ".edition.json";
const FICHIER_INSTALLATION: &str = "EDITION.json";
pub const BRIDE: &str = "bride";
pub const DEBLOQUE: &str = "debloque";

/// Un mot du bridage : `Some(true)` bride, `Some(false)` debloque ; `"demo"` et `"complete"`, que portent des
/// installations posees, se lisent aussi (en tete du module).
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
/// poste (`.edition.json`) ; SINON BRIDE -- rien d'autre ne debloque.
pub fn est_bride_de(inst: &Path) -> bool {
    if paquet_soude_de(inst) {
        return true;
    }
    mot(&lire(&inst.join(FICHIER_GESTE)), "edition").unwrap_or(true)
}

/// LE REFUS D'UN ENVOI DEPUIS UNE FICTIVE AJOUTEE, mot pour mot : le meme pour tout geste
/// qui ferait partir quelque chose vers un organisme ou une personne. Sa classe : `CLASSE_REFUS_D_ENVOI` (la page la
/// reconnait et montre la phrase dans sa boite, `js/tuyau.js`).
pub const REFUS_D_ENVOI: &str = "L'envoi n'est pas possible depuis la version démo. Pour 2027, et afin d'offrir un service de qualité, le nombre d'utilisateurs est restreint. N'hésitez pas à contacter Léandre (puppetpaie@gmail.com) avec une courte présentation et votre motivation pour débloquer le logiciel et faire partie des premiers utilisateurs.";
pub const CLASSE_REFUS_D_ENVOI: &str = "EnvoiBride";

/// LE BRANCHEMENT DE LA COMPAGNIE `racine` (`envois::branchement`), choisi ici et nulle part ailleurs :
/// - une FICTIVE AJOUTEE (`compagnies::est_ajoutee`) n'envoie rien : le message, `REFUS_D_ENVOI` ;
/// - une compagnie FICTIVE (son marqueur, `compagnies::est_fictive`), ou toute compagnie du PAQUET SOUDE
///   (`paquet_soude`) : le simulateur ;
/// - toute autre : le monde. Le logiciel simplement bride ne joue rien.
///
/// Chaque envoi vers un organisme ou une personne l'ouvre avant que rien ne s'ecrive (`Branchement::ouvrir`). Ce qui va
/// a l'editeur (incidents, « Un probleme, une idee ? ») et la verification d'un mot de passe ne sont pas des envois.
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
