//! La ligne de commande du produit, `paie COMMANDE ...` : ce qu'un humain (ou un agent) tape.
//!
//! Une commande = un module, qui declare ses options (`analyse::Spec`) et rend un code de sortie. Chacune reprend la
//! commande Python du meme nom (`moteur/cli.py`, `moteur/commandes_*.py`) : memes options, meme sortie -- texte et
//! `--json` (l'enveloppe de `moteur/sortie_json.py`) --, meme code de sortie. Elle s'appuie sur ce que le moteur sait
//! deja faire (les routes, `paie route NOM`) : rien ne se recopie ici, on lit et on met en forme.
//!
//! Les phrases proposent `paie ...`, jamais `python -m moteur ...`.
//!
//! LE MOT DE PASSE. Chaque commande installe au demarrage la demande reelle (`demande::installer`, lot J1). Une
//! consultation qui trouve le coffre ferme -- ni la cle du poste, ni un mot de passe tape, ni une seance -- le demande
//! par `demande::ouvrir_seance` (fenetre au lot J3 ; le terminal, sans echo ; sinon un refus avec sa phrase), jamais
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
pub mod aer;
pub mod casc_envoyer;
pub mod casc_regularise;
pub mod analyse;
pub mod arrivee;
pub mod archiver;
pub mod arret;
pub mod suspension;
pub mod cloture;
pub mod coffre;
pub mod compta_logiciel;
pub mod compagnie;
pub mod depot_dsn;
pub mod depots;
pub mod deposer_dpae;
pub mod dpae;
pub mod dossier;
pub mod envoi;
pub mod export_compta;
pub mod exporter;
pub mod droits;
pub mod emplois;
pub mod echeances;
pub mod fiche;
pub mod filet;
pub mod fin_contrat;
pub mod importer;
pub mod google;
pub mod incidents;
pub mod journal;
pub mod nouveau;
pub mod ouverture;
pub mod pas;
pub mod rendu;
pub mod restaurer;
pub mod retours;
pub mod sauvegarder;
pub mod signatures;
pub mod sources;
pub mod version;

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
    /// Une route du moteur (`paie route NOM`), sans lancer de processus : son `resultat`, ou son refus.
    pub fn route(&self, nom: &str, charge: Value) -> Result<Value, Echec> {
        let entree = json!({"route": nom, "charge": charge}).to_string();
        match crate::routes::commande(&self.o, &entree) {
            Ok(r) if r.get("ok").and_then(Value::as_bool) == Some(true) => Ok(r.get("resultat").cloned().unwrap_or(Value::Null)),
            Ok(r) => {
                let classe = r.get("classe").and_then(Value::as_str).map(classe_connue);
                Err(Echec { classe, message: r.get("erreur").map(rendu::texte).unwrap_or_default() })
            }
            Err(m) => {
                let (classe, message) = crate::routes_ouverture::decoder(&m);
                Err(Echec { classe: classe.map(classe_connue), message: message.to_string() })
            }
        }
    }

    /// Comme `route`, mais la reponse ENTIERE du moteur : `{"ok": true, "resultat"}` ou `{"ok": false, "erreur", "classe",
    /// <donnee du refus>}` (`manques` d'un mois pas pret, `etat` d'une DSN deja deposee). Pour qui doit rendre cette donnee.
    pub fn route_entiere(&self, nom: &str, charge: Value) -> Result<Value, Echec> {
        let entree = json!({"route": nom, "charge": charge}).to_string();
        match crate::routes::commande(&self.o, &entree) {
            Ok(r) => Ok(r),
            Err(m) => {
                let (classe, message) = crate::routes_ouverture::decoder(&m);
                Err(Echec { classe: classe.map(classe_connue), message: message.to_string() })
            }
        }
    }

    /// Un plantage : une ligne dans le journal des incidents (sa classe et sa nature), puis la phrase seule a l'ecran
    /// (sortie d'erreur), code 1.
    pub fn plantage(&self, e: Echec) -> i32 {
        let classe = e.classe.unwrap_or("RuntimeError");
        let nature = crate::incidents::nature_de(classe);
        let ou = if nature == crate::incidents::REFUS { self.commande } else { "" };
        let _ = crate::incidents::depuis_exception(&self.racine, classe, &e.message, Some(nature), ou, json!({"commande": self.commande}), None);
        eprintln!("{}", e.message);
        1
    }

    /// Ouvre le coffre si la cle du poste ne le fait pas (`operations`/`commandes_ouverture.ouverture_du_coffre`) :
    /// rien a demander quand la cle du poste l'ouvre, ou qu'une seance est gardee ; sinon la seance se demande par
    /// `demande::ouvrir_seance`. Un refus ne coupe pas la commande (comme le Python) : le secret vient peut-etre de
    /// l'environnement, et sinon la lecture du coffre dira elle-meme ce qui manque. La phrase est dite.
    pub fn ouvrir_le_coffre(&self) {
        let c = crate::coffre::Coffre::du_poste(&self.racine, "");
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
        eprintln!("fois, pour {} h.", crate::demande::HEURES_SEANCE);
        if let crate::demande::Verdict::Refuse(phrase) = crate::demande::ouvrir_seance(&self.racine) {
            eprintln!("! {}", phrase);
        }
    }
}

/// La classe d'un refus rendue en `&'static str` pour `Echec` (une classe inconnue devient `RuntimeError`).
fn classe_connue(c: &str) -> &'static str {
    const CONNUES: [&str; 37] = [
        "ErreurOperation", "SaisieIncomplete", "PreuveInsuffisante", "FicheRefusee", "MoisPasPret", "KeyError", "ValueError",
        "FileNotFoundError", "PermissionError", "OSError", "UnicodeDecodeError", "JSONDecodeError", "TypeError", "AttributeError",
        "ErreurApi", "ErreurCoffre", "ErreurGoogle", "DepotRefuse", "VersionExigee", "EditionBridee", "CompteursVerrouilles", "CalculRefuse",
        // Les classes des actes (lot J2b) : un depot, une correction, un arret.
        "DsnDejaDeposee", "DpaeDejaDeclaree", "RepetitionManquante", "OrdreTropPetit", "AnnuleRemplaceHorsDelai", "MoisNonRegularisable",
        "ExerciceClos", "ClotureRefusee", "MoisHorsDelai", "ActeReelRefuse", "ArretIntrouvable",
        // Les envois (theme 2) : une sortie absente, une boite absente ou en panne.
        "SortieAbsente", "BoiteAbsente", "ErreurCourriel",
        // Le refus d'un envoi depuis une fictive ajoutee (ticket #236, `bridage::refus_d_envoi`).
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
    vec![
        Commande { nom: "droits", aide: "heures et cachets cumules (annexes 8 et 10)", cachee: false, spec: &droits::SPEC, executer: droits::executer },
        Commande { nom: "echeancier", aide: "la date, ce qui est a faire ce mois-ci, l'annee qui vient", cachee: false, spec: &echeances::SPEC_ECHEANCIER, executer: echeances::echeancier },
        Commande { nom: "rappels", aide: "ce que l'ouverture propose : DSN du mois d'avant, cloture", cachee: false, spec: &echeances::SPEC_RAPPELS, executer: echeances::rappels },
        Commande { nom: "incidents", aide: "ce que le logiciel n'a pas su faire : plantages, refus, rejets de declaration", cachee: false, spec: &incidents::SPEC, executer: incidents::executer },
        Commande { nom: "filet", aide: "la DPAE preparee d'avance et son filet de securite : l'etat, les embauches suivies", cachee: false, spec: &filet::SPEC, executer: filet::executer },
        Commande { nom: "version", aide: "la version du logiciel, et ce qui est pret", cachee: false, spec: &version::SPEC, executer: version::executer },
        Commande { nom: "coffre", aide: "les mots de passe des organismes, scelles sur le disque (ce qui est pose)", cachee: false, spec: &coffre::SPEC, executer: coffre::executer },
        Commande { nom: "journal", aide: "le journal des depots -- le lire", cachee: false, spec: &journal::SPEC, executer: journal::executer },
        Commande { nom: "pas", aide: "taux de prelevement a la source connus", cachee: false, spec: &pas::SPEC, executer: pas::executer },
        Commande { nom: "google", aide: "le compte Google (Drive : sauvegarde, archivage) : son etat", cachee: false, spec: &google::SPEC, executer: google::executer },
        Commande { nom: "arrivee", aide: "l'arrivee d'une compagnie : ce qui est pret, ce qui manque, quoi lancer (rien n'est ecrit)", cachee: false, spec: &arrivee::SPEC, executer: arrivee::executer },
        Commande { nom: "retours", aide: "recuperer les retours DSN (lecture seule)", cachee: false, spec: &retours::SPEC, executer: retours::executer },
        // L'enquete de l'editeur sur le poste d'une compagnie : elle marche, elle ne figure pas dans l'aide.
        Commande { nom: "depots", aide: "lister les depots vus par net-entreprises (enquete)", cachee: true, spec: &depots::SPEC, executer: depots::executer },
        // L'outil de l'editeur, avant chaque publication : refuse ailleurs que sur son poste (le reglage de l'installation, `maj::reglage`).
        Commande { nom: "sources", aide: "les sources des donnees livrees, et leur veille (poste de l'editeur)", cachee: true, spec: &sources::SPEC, executer: sources::executer },
        Commande { nom: "fiche", aide: "rappeler les donnees de reference", cachee: false, spec: &fiche::SPEC, executer: fiche::executer },
        Commande { nom: "nouveau", aide: "creer un fichier de saisie pour un mois", cachee: false, spec: &nouveau::SPEC, executer: nouveau::executer },
        Commande { nom: "dpae", aide: "produire la seule DPAE, avant l embauche", cachee: false, spec: &dpae::SPEC, executer: dpae::executer },
        Commande { nom: "fin-contrat", aide: "le signalement de fin de contrat de travail unique (nature 07)", cachee: false, spec: &fin_contrat::SPEC, executer: fin_contrat::executer },
        Commande { nom: "arret", aide: "les arrets de travail : saisir, signaler, reporter", cachee: false, spec: &arret::SPEC, executer: arret::executer },
        Commande { nom: "suspension", aide: "les suspensions du contrat (conge parental, sabbatique, sans solde...) : saisies une fois", cachee: false, spec: &suspension::SPEC, executer: suspension::executer },
        Commande { nom: "emplois", aide: "le catalogue des metiers (codes emploi, PCS, statut)", cachee: false, spec: &emplois::SPEC, executer: emplois::executer },
        Commande { nom: "importer", aide: "reprendre l historique depuis le dossier import/ : l apercu en JSON, --ecrire pour ecrire", cachee: false, spec: &importer::SPEC, executer: importer::executer },
        Commande { nom: "aer", aide: "ranger les attestations France Travail deja recues", cachee: false, spec: &aer::SPEC, executer: aer::executer },
        Commande { nom: "depot-dsn", aide: "deposer une DSN sur net-entreprises (test ou reel)", cachee: false, spec: &depot_dsn::SPEC, executer: depot_dsn::executer },
        Commande { nom: "deposer-dpae", aide: "deposer une DPAE deja produite (essai a blanc par defaut)", cachee: false, spec: &deposer_dpae::SPEC, executer: deposer_dpae::executer },
        Commande { nom: "dossier", aide: "le dossier d un salarie : bulletins, contrats, certificats, attestations - un PDF par categorie", cachee: false, spec: &dossier::SPEC, executer: dossier::executer },
        Commande { nom: "signatures", aide: "relever maintenant les contrats signes d un clic (Gmail)", cachee: false, spec: &signatures::SPEC, executer: signatures::executer },
        Commande { nom: "envoi", aide: "envoyer bulletin, certificat CS et AER aux salaries", cachee: false, spec: &envoi::SPEC, executer: envoi::executer },
        Commande { nom: "casc-envoyer", aide: "envoyer les deux courriels de regularisation CASC-SVP (apres les virements)", cachee: false, spec: &casc_envoyer::SPEC, executer: casc_envoyer::executer },
        Commande { nom: "casc-regularise", aide: "noter qu une annee de CASC-SVP est regularisee (apres le virement)", cachee: false, spec: &casc_regularise::SPEC, executer: casc_regularise::executer },
        Commande { nom: "sauvegarder", aide: "envoyer le dossier prive dans le Drive, par differences", cachee: false, spec: &sauvegarder::SPEC, executer: sauvegarder::executer },
        Commande { nom: "restaurer", aide: "redescendre le dossier prive du Drive (poste neuf, ordinateur perdu)", cachee: false, spec: &restaurer::SPEC, executer: restaurer::executer },
        Commande { nom: "archiver", aide: "deposer les sorties d un mois dans le Drive", cachee: false, spec: &archiver::SPEC, executer: archiver::executer },
        Commande { nom: "ouverture", aide: "ce que le logiciel fait a l ouverture : sauvegarde + retours", cachee: false, spec: &ouverture::SPEC, executer: ouverture::executer },
        Commande { nom: "exporter", aide: "les etats pour le comptable, les organismes, les financeurs", cachee: false, spec: &exporter::SPEC, executer: exporter::executer },
        Commande { nom: "export-compta", aide: "le journal de paie de l annee, pour la comptabilite", cachee: false, spec: &export_compta::SPEC, executer: export_compta::executer },
        Commande { nom: "compta-logiciel", aide: "le logiciel du comptable, retenu pour les ecritures comptables", cachee: false, spec: &compta_logiciel::SPEC, executer: compta_logiciel::executer },
        Commande { nom: "cloture", aide: "cloturer un exercice (zip de l annee), ou l etat des exercices", cachee: false, spec: &cloture::SPEC, executer: cloture::executer },
    ]
}

fn aide() -> String {
    let mut t = String::from(
        "usage: paie [--dev] COMMANDE [options]\n\nPuppetPaie - paie du spectacle vivant\n\nChaque commande accepte --json : meme travail, sortie lisible par un programme.\n\ncommandes :\n",
    );
    for c in commandes().iter().filter(|c| !c.cachee) {
        t.push_str(&format!("  {:<11} {}\n", c.nom, c.aide));
    }
    t.push_str("\n`paie route NOM` donne acces a tout ce que fait la fenetre ; `paie pdf` est la fabrique de PDF.\n");
    t
}

/// `paie [--dev] COMMANDE ...`. Rend le code de sortie.
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
        eprintln!("paie: error: commande inconnue : {}\n\n{}", nom, aide());
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
    // La demande du mot de passe du logiciel est celle du moteur (lot J1) : fenetre s'il y en a une, sinon le terminal.
    let _canaux = crate::demande::installer(Rc::new(crate::demande::Reel::du_poste()));
    (c.executer)(&cadre, &a)
}
