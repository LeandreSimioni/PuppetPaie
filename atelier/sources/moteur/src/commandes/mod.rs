//! La ligne de commande de ce qui n'est pas un geste de l'ecran, `moteur COMMANDE ...` : le coffre (la seance, et les
//! secrets tapes sans echo, que l'entree standard ne porte jamais), la restauration d'un poste neuf depuis le Drive, et
//! les outils de l'editeur (`sources`, `depots`). Un geste de l'ecran se fait par sa route : `moteur routes` les dit,
//! `moteur route` en sert une (`--texte` : ses phrases).
//!
//! Une commande = un module, qui declare ses options (`analyse::Spec`) et rend un code de sortie ; sa sortie est un
//! texte, ou l'enveloppe `--json` (`rendu::enveloppe`).
//!
//! LE MOT DE PASSE. Chaque commande installe au demarrage la demande reelle (`demande::installer`). Une
//! consultation qui trouve le coffre ferme -- ni la cle du poste, ni un mot de passe tape, ni une seance -- le demande
//! par `demande::ouvrir_seance` (la fenetre du moteur s'il y en a une ; le terminal, sans echo ; sinon un refus avec sa phrase), jamais
//! par autre chose.

/// Ecrit une ligne sur la sortie standard. Un tuyau ferme (`| head`) n'est pas une erreur du programme.
macro_rules! dire {
    () => {{
        use std::io::Write;
        let _ = writeln!(std::io::stdout());
    }};
    ($($arg:tt)*) => {{
        use std::io::Write;
        let _ = writeln!(std::io::stdout(), $($arg)*);
    }};
}

/// Ecrit sans saut de ligne (l'equivalent de `print(..., end="")`).
#[allow(unused_macros)]
macro_rules! dire_sans_ligne {
    ($($arg:tt)*) => {{
        use std::io::Write;
        let _ = write!(std::io::stdout(), $($arg)*);
    }};
}

pub mod actes;
pub mod analyse;
pub mod coffre;
pub mod compagnie;
pub mod depots;
pub mod rendu;
pub mod restaurer;
pub mod sources;

use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use serde_json::{json, Value};

use droit::contexte::Contexte;
use socle::date::Jour;
use socle::echec::Echec;
use analyse::{Args, Spec};

/// Ce qu'une commande trouve en arrivant : la compagnie, le jour, et ses options.
pub struct Cadre {
    pub racine: PathBuf,
    /// `--racine` et `--date` des routes du moteur (`service::options`).
    pub o: HashMap<String, String>,
    pub ctx: Contexte,
    pub aujourdhui: Jour,
    /// Le nom de la commande, pour le journal des incidents.
    pub commande: &'static str,
}

impl Cadre {
    /// Une route du moteur (`moteur route NOM`), sans lancer de processus : son `resultat`, ou son refus.
    pub fn route(&self, nom: &str, charge: Value) -> Result<Value, Echec> {
        let entree = json!({"route": nom, "charge": charge}).to_string();
        match crate::routes::commande(&self.o, &entree) {
            Ok(r) if r.get("ok").and_then(Value::as_bool) == Some(true) => Ok(r.get("resultat").cloned().unwrap_or(Value::Null)),
            Ok(r) => {
                let classe = r.get("classe").and_then(Value::as_str).map(classe_connue);
                Err(Echec { classe, message: r.get("erreur").map(rendu::texte).unwrap_or_default() })
            }
            Err(m) => {
                Err(Echec { classe: None, message: m })
            }
        }
    }

    /// Un plantage : une ligne dans le journal des incidents (sa classe et sa nature), puis la phrase seule a l'ecran
    /// (sortie d'erreur), code 1.
    pub fn plantage(&self, e: Echec) -> i32 {
        let classe = e.classe.unwrap_or("RuntimeError");
        let nature = crate::compagnie::incidents::nature_de(classe);
        let ou = if nature == crate::compagnie::incidents::REFUS { self.commande } else { "" };
        let _ = crate::compagnie::incidents::depuis_exception(&self.racine, classe, &e.message, Some(nature), ou, json!({"commande": self.commande}), None);
        eprintln!("{}", e.message);
        1
    }

    /// Ouvre le coffre si la cle du poste ne le fait pas :
    /// rien a demander quand la cle du poste l'ouvre, ou qu'une seance est gardee ; sinon la seance se demande par
    /// `demande::ouvrir_seance`. Un refus ne coupe pas la commande : le secret vient peut-etre de
    /// l'environnement, et sinon la lecture du coffre dira elle-meme ce qui manque. La phrase est dite.
    pub fn ouvrir_le_coffre(&self) {
        let c = crate::compagnie::coffre::Coffre::du_poste(&self.racine, "");
        match c.ouvre_a_ce_poste() {
            Ok(true) => return,
            Ok(false) => {}
            // Un coffre illisible : la lecture du secret le dira.
            Err(_) => return,
        }
        if matches!(c.ouverture_gardee(), Ok(Some(_))) {
            // La seance gardee ouvre le coffre (`Coffre::ouvrir`) : rien a retenir.
            return;
        }
        // Sur la sortie d'ERREUR : les phrases d'une seance ne doivent pas polluer la sortie standard d'un `--json`.
        eprintln!();
        eprintln!("Le coffre de ce poste ne s'ouvre pas avec sa cle. Ouverture de la seance : le mot de passe du logiciel, une");
        eprintln!("fois, pour {} h.", crate::compagnie::demande::HEURES_SEANCE);
        if let crate::compagnie::demande::Verdict::Refuse(phrase) = crate::compagnie::demande::ouvrir_seance(&self.racine) {
            eprintln!("! {}", phrase);
        }
    }
}

/// La classe d'un refus rendue en `&'static str` pour `Echec` (une classe inconnue devient `RuntimeError`).
fn classe_connue(c: &str) -> &'static str {
    const CONNUES: [&str; 32] = [
        "ErreurOperation", "SaisieIncomplete", "PreuveInsuffisante", "FicheRefusee", "KeyError", "ValueError",
        "FileNotFoundError", "PermissionError", "OSError", "UnicodeDecodeError", "JSONDecodeError", "TypeError", "AttributeError",
        "ErreurApi", "ErreurCoffre", "ErreurGoogle", "DepotRefuse", "VersionExigee", "EditionBridee", "CompteursVerrouilles", "CalculRefuse",
        // Les classes des actes : un depot, une correction, un arret.
        "DsnDejaDeposee", "RepetitionManquante", "OrdreTropPetit", "AnnuleRemplaceHorsDelai", "MoisNonRegularisable",
        "ExerciceClos", "ActeReelRefuse",
        // Les envois (theme 2) : une sortie absente, une boite absente ou en panne.
        "SortieAbsente", "BoiteAbsente", "ErreurCourriel",
        // Le refus d'un envoi depuis une fictive ajoutee (`bridage::branchement`).
        "EnvoiBride",
    ];
    CONNUES.iter().find(|x| **x == c).copied().unwrap_or("RuntimeError")
}

type Executer = fn(&Cadre, &Args) -> i32;

struct Commande {
    nom: &'static str,
    aide: &'static str,
    cachee: bool,
    spec: &'static Spec,
    executer: Executer,
}

fn commandes() -> Vec<Commande> {
    #[allow(unused_mut)]
    let mut v = vec![
        Commande { nom: "coffre", aide: "les mots de passe des organismes, scelles sur le disque (ce qui est pose)", cachee: false, spec: &coffre::SPEC, executer: coffre::executer },
        // L'enquete de l'editeur sur le poste d'une compagnie : elle marche, elle ne figure pas dans l'aide.
        Commande { nom: "depots", aide: "lister les depots vus par net-entreprises (enquete)", cachee: true, spec: &depots::SPEC, executer: depots::executer },
        // L'outil de l'editeur, avant chaque publication : refuse ailleurs que sur son poste (le reglage de l'installation, `maj::reglage`).
        Commande { nom: "sources", aide: "les sources des donnees livrees, et leur veille (poste de l'editeur)", cachee: true, spec: &sources::SPEC, executer: sources::executer },
        Commande { nom: "restaurer", aide: "redescendre le dossier prive du Drive (poste neuf, ordinateur perdu)", cachee: false, spec: &restaurer::SPEC, executer: restaurer::executer },
    ];
    // LES OUTILS DE L'ATELIER sur une compagnie (`atelier::outils`) : le binaire de l'atelier seulement.
    #[cfg(feature = "atelier")]
    {
        use crate::atelier::outils::{aer, depot_dsn, journal, pas, retours};
        v.extend([
            Commande { nom: "pas", aide: "les taux de PAS connus ; --amorcer, --demander (atelier)", cachee: false, spec: &pas::SPEC, executer: pas::executer },
            Commande { nom: "aer", aide: "ranger de nouveau les AER recues ; --forcer (atelier)", cachee: false, spec: &aer::SPEC, executer: aer::executer },
            Commande { nom: "journal", aide: "--dpae : reprendre au journal les DPAE attestees (atelier)", cachee: false, spec: &journal::SPEC, executer: journal::executer },
            Commande { nom: "retours", aide: "l'enquete des retours : plage, flux, plate-forme (atelier)", cachee: false, spec: &retours::SPEC, executer: retours::executer },
            Commande { nom: "depot-dsn", aide: "un depot de test a un ordre, ou chez l'editeur (atelier)", cachee: false, spec: &depot_dsn::SPEC, executer: depot_dsn::executer },
        ]);
    }
    v
}

fn aide() -> String {
    let mut t = String::from(
        "usage: moteur [--dev] COMMANDE [options]\n\nPuppetPaie - paie du spectacle vivant\n\ncommandes :\n",
    );
    for c in commandes().iter().filter(|c| !c.cachee) {
        t.push_str(&format!("  {:<11} {}\n", c.nom, c.aide));
    }
    t.push_str("\nTout ce que fait la fenetre : `moteur routes` (le catalogue), `moteur route` (une route, `--texte` : ses phrases). `moteur pdf` est la fabrique de PDF.\n");
    t
}

/// `moteur [--dev] COMMANDE ...`. Rend le code de sortie.
pub fn executer(args: &[String]) -> i32 {
    let mut args = args;
    let mut dev = false;
    if args.first().map(String::as_str) == Some("--dev") {
        dev = true;
        args = &args[1..];
    }
    let Some(nom) = args.first().map(String::as_str) else {
        eprintln!("{}", aide());
        return 2;
    };
    if matches!(nom, "-h" | "--help" | "--aide" | "aide") {
        dire!("{}", aide());
        return 0;
    }
    let table = commandes();
    let Some(c) = table.iter().find(|c| c.nom == nom) else {
        eprintln!("moteur: error: commande inconnue : {}\n\n{}", nom, aide());
        return 2;
    };
    let reste = &args[1..];
    if reste.iter().any(|a| a == "-h" || a == "--help" || a == "--aide") {
        dire!("{}", analyse::usage(c.spec));
        return 0;
    }
    let a = match analyse::analyser_ou_sortir(c.spec, reste) {
        Ok(a) => a,
        Err(code) => return code,
    };
    // La commande est lancee a la main : sans PUPPETPAIE_CODE, le code est celui qu'on trouve a cote du programme.
    compagnie::fixer_le_code();
    let racine = match compagnie::racine(dev) {
        Ok(r) => r,
        Err(phrase) => {
            eprintln!("{}", phrase);
            return 1;
        }
    };
    let aujourdhui = compagnie::poser_le_jour();
    let mut o = HashMap::new();
    o.insert("racine".to_string(), racine.to_string_lossy().into_owned());
    o.insert("date".to_string(), aujourdhui.iso());
    let ctx = match Contexte::nouveau(&racine, Some(aujourdhui)) {
        Ok(c) => c,
        Err(m) => {
            eprintln!("{}", m);
            return 1;
        }
    };
    let cadre = Cadre { racine, o, ctx, aujourdhui, commande: c.nom };
    // La demande du mot de passe du logiciel est celle du moteur : fenetre s'il y en a une, sinon le terminal.
    let _canaux = crate::compagnie::demande::installer(Rc::new(crate::compagnie::demande::Reel::du_poste()));
    (c.executer)(&cadre, &a)
}
