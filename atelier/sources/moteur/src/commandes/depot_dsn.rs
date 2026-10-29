//! `moteur depot-dsn (FICHIER | AAAA-MM) (--test | --reel | --apercu) [--confirmer-mois AAAA-MM] [--force] [--sans-repetition MOTIF]
//! [--environnement production|editeur] [--ordre N] [--fenetre]` : deposer une DSN sur net-entreprises (`moteur/commandes_dsn.py`,
//! `cmd_depot_dsn`). La seule commande qui declare quelque chose avec `deposer-dpae`.
//! Un mois `AAAA-MM` a la place du fichier : la DSN du mois, fabriquee a son depot depuis ses engagements valides
//! (`depart::fabriquer_dsn`).
//!
//! LE DEPOT LUI-MEME EST `depot_acte::deposer_dsn_dans` : la commande lit, affiche, puis appelle -- et affiche ce qui est revenu.
//! Les garde-fous sont ceux du moteur, jamais ceux de la commande :
//!
//! - `--apercu` ne va pas au reseau et ne demande rien ;
//! - un depot REEL passe par `verifier_preuve` : le moteur demande LUI-MEME le mot de passe du logiciel (fenetre, terminal sans
//!   echo, sinon refus). Aucun mot de passe en option, en variable lue en clair, ni dans une sortie ; `--fenetre` est accepte et
//!   sans effet (la demande est celle du moteur) ;
//! - `--confirmer-mois` est verifie ICI, avant toute demande de mot de passe (le moteur ne le lit que lorsqu'un mot de passe est
//!   donne a l'avance, ce que la commande ne fait jamais) ;
//! - dans `moteur-atelier`, tout depot reel est refuse par `verifier_preuve` (`ActeReelRefuse`) ;
//! - un depot REEL n'est possible que sur `production` : la plate-forme `editeur` est un service d'essai, et le journal y
//!   noterait comme declaree une DSN qui ne l'est pas (voir `depot_acte::deposer_dsn_dans`).
//!
//! - `--ordre N` (lot E1) : seulement avec `--test`, sur une mensuelle -- l'essai part en annule-et-remplace a l'ordre N au
//!   moins, quand la base d'essai a deja retenu des essais de ce SIRET pour ce mois que le journal de ce dossier ignore (un
//!   autre poste, une compagnie d'essai). C'est le moteur qui refuse ailleurs (`depot_acte::deposer_dsn_dans`).
//! - dans une COMPAGNIE D'ESSAI (`essai.rs`), l'aperçu ne montre pas le SIREN : c'est celui d'une vraie compagnie, venu de
//!   `PUPPETPAIE_COMPAGNIE`, qui ne s'affiche pas.
//!
//! `qui` = `ligne de commande`, comme le Python. Pas de `--json` (le Python n'en a pas ici).
//!
//! DEFAUT DU PYTHON, NE SE REPRODUIT PAS : `--reel --environnement editeur` y partait et etait note au journal comme un depot
//! reel ; un fichier illisible autre qu'`OSError` ou une panne hors `ErreurApi` y donnaient une trace. Ici : une phrase.

use serde_json::{json, Value};

use super::analyse::{Args, Opt, Pos, Spec};
use super::Cadre;
use envois::depot::{self, Apercu};
use crate::compagnie::depot_acte::{self, DepotFait, Preuve};
use socle::echec::Echec;

pub static SPEC: Spec = Spec {
    commande: "depot-dsn",
    positionnels: &[Pos::requis("fichier")],
    options: &[
        Opt::drapeau("apercu"),
        Opt::drapeau("test"),
        Opt::drapeau("reel"),
        Opt::valeur("confirmer-mois"),
        Opt::drapeau("force"),
        Opt::valeur("sans-repetition"),
        Opt::choix("environnement", &["production", "editeur"], Some("production")),
        Opt::valeur("ordre"),
        Opt::drapeau("fenetre"),
    ],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    match faire(cadre, args) {
        Ok(c) => c,
        Err(e) => cadre.plantage(e),
    }
}

/// Un horodatage ISO a la francaise, avec ` a ` (`util.quand_lisible(x, " a ")`).
pub(crate) fn quand_a(v: &Value) -> String {
    envois::etats::quand_lisible(v).replace(" à ", " a ")
}

fn afficher_apercu(chemin: &str, a: &Apercu, essai: bool) {
    dire!("Fichier   : {}", chemin);
    // Un SIGNALEMENT n'a pas de mois principal declare : ce qui le date, c'est son evenement.
    if a.est_signalement() {
        dire!("Nature    : {} - {}", a.champ("nature"), a.nature_lisible());
        dire!("Evenement : {}", a.evenement_lisible());
    } else {
        dire!("Mois      : {}", a.mois_lisible());
    }
    let type_declaration = a.champ("type_declaration");
    let lisible = depot::TYPES.iter().find(|(c, _)| *c == type_declaration).map_or("?", |(_, l)| *l);
    dire!("Type      : {} - {}", type_declaration, lisible);
    dire!("Ordre     : {}   fraction {}   constitue le {}", a.champ("ordre"), a.champ("fraction"), a.champ("constitution"));
    if !a.champ("remplace").is_empty() {
        dire!("Remplace  : declaration {}", a.champ("remplace"));
    }
    if essai {
        dire!("Logiciel  : {}   SIREN de la compagnie d'essai (ne s'affiche pas)", a.champ("logiciel"));
    } else {
        dire!("Logiciel  : {}   SIREN {}", a.champ("logiciel"), a.champ("siren"));
    }
    dire!("Contenu   : {} salarie(s), {} lignes", a.salaries, a.lignes);
    if !a.versements.is_empty() {
        let v: Vec<String> = a.versements.iter().map(|x| format!("{} EUR", x)).collect();
        dire!("Ordres de telepaiement : {}", v.join(", "));
    }
    // NON BLOQUANT, ET DIT FORT : la grille par defaut preleve le plus souvent PLUS que le taux reel.
    if a.pas_bareme > 0 {
        dire!("\nPRELEVEMENT A LA SOURCE : {} versement(s) sur {} partent sur la", a.pas_bareme, a.pas_versements);
        dire!("GRILLE PAR DEFAUT (art. 204 H du CGI), faute de taux de la DGFiP.");
        dire!("  Qui, et pourquoi :  paie pas");
        dire!("  Ce qui les remplit :  moteur retours --appliquer");
    }
}

/// Ce qui a ouvert le depot reel : le test qui en repond, ou le motif (`dire_repetition`). Un contournement se LIT au moment ou
/// il sert.
pub(crate) fn dire_repetition(r: &Value) {
    if !socle::fichiers::vrai(r) {
        return;
    }
    if r.get("contournement").is_some_and(socle::fichiers::vrai) {
        dire!("\nDEPOSE SANS REPETITION -- motif inscrit au journal :\n  {}", super::rendu::champ(r, "contournement"));
    } else {
        let test = r.get("test").filter(|x| socle::fichiers::vrai(x)).map(quand_a).filter(|t| !t.is_empty()).unwrap_or_else(|| "?".into());
        dire!("\nRepetition : depot de test du {}, compte rendu {}.", test, super::rendu::champ_ou(r, "compte_rendu", "?"));
    }
}

/// Le mois declare, ecrit assez gros pour qu'on ne le lise pas de travers, juste avant la demande du mot de passe.
fn annoncer_ce_qui_part(a: &Apercu) {
    let quoi = if a.est_signalement() { "Jour de l'evenement signale" } else { "Mois declare par ce fichier" };
    let valeur = a.confirmation_attendue();
    dire!("\n    {}", quoi);
    dire!("    {}", "=".repeat((valeur.chars().count() + 8).max(quoi.chars().count())));
    dire!("        >>>  {}  <<<\n", valeur);
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let (apercu_seul, test, reel) = (args.drapeau("apercu"), args.drapeau("test"), args.drapeau("reel"));
    if !apercu_seul && test == reel {
        dire!("Choisir --test OU --reel. Les deux s'excluent, et il n'y a pas de defaut :");
        dire!("  --test    controles executes, bilan rendu, RIEN de conserve");
        dire!("  --reel    la declaration part, avec ses ordres de telepaiement");
        dire!("  --apercu  lit le fichier et l'affiche, sans reseau ni depot");
        return Ok(2);
    }
    let environnement = args.texte("environnement").unwrap_or("production");
    let fichier = args.texte("fichier").unwrap_or("");
    // UN MOIS (`AAAA-MM`) A LA PLACE D.UN FICHIER : la DSN du mois, fabriquee depuis ses engagements valides.
    let fabriquee;
    let fichier = if est_un_mois(fichier) {
        let dsn = crate::compagnie::depart::fabriquer_dsn(&cadre.ctx, fichier, "cli")?;
        fabriquee = dsn.fichiers.first().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
        fabriquee.as_str()
    } else {
        fichier
    };
    let chemin = crate::routes::depots::resoudre(&cadre.racine, &json!(fichier))?;
    let octets = match crate::routes::depots::lire_octets(&chemin) {
        Ok(o) => o,
        Err(e) => {
            dire!("Fichier illisible : {}", e.message);
            return Ok(1);
        }
    };
    let apercu = depot::lire(&octets);
    let essai = crate::compagnie::essai::est_essai(&cadre.racine);
    let ordre = match args.texte("ordre").filter(|t| !t.is_empty()) {
        None => None,
        Some(t) => match t.trim().parse::<i64>() {
            Ok(n) if n >= 1 && test => Some(n),
            _ => {
                dire!("--ordre N : un entier positif, et seulement avec --test (le depot de test d'une DSN mensuelle).");
                return Ok(2);
            }
        },
    };

    if apercu_seul {
        // Ni reseau ni mot de passe : la relecture avant decision.
        dire!("{}\nCE QUI PARTIRAIT - rien n'est envoye\n{}", "-".repeat(62), "-".repeat(62));
        afficher_apercu(&chemin, &apercu, essai);
        dire!("{}", "-".repeat(62));
        dire!("Le fichier est code {} : sans --test, un depot {}.", apercu.champ("nature_envoi"), if apercu.reel() { "DECLARERAIT" } else { "ne declarerait rien" });
        return Ok(0);
    }

    // UN DEPOT REEL NE PART QUE SUR LA PRODUCTION : refuse avant tout affichage, avant toute demande.
    if reel && environnement != "production" {
        dire!("\nRefus : l'environnement '{}' est la plate-forme d'essai de l'editeur : rien n'y est declare. Un depot REEL ne part que sur 'production' ; pour essayer, utiliser --test. Rien n'est parti.", environnement);
        return Ok(2);
    }

    dire!("{}\n{}\n{}", "-".repeat(62), if reel { "DEPOT REEL - LA DECLARATION PART" } else { "DEPOT DE TEST - rien ne sera conserve" }, "-".repeat(62));
    afficher_apercu(&chemin, &apercu, essai);
    dire!("{}", "-".repeat(62));

    if reel {
        // `--confirmer-mois` : verifie contre le fichier AVANT que le moteur ne demande le mot de passe. Une commande scriptee qui
        // croit deposer aout sur un fichier de septembre doit se faire arreter sans avoir rien demande a personne.
        if let Some(m) = args.texte("confirmer-mois").filter(|m| !m.is_empty()) {
            if let Err(e) = depot::verifier(&apercu, true, Some(m), true) {
                dire!("\nRefus : {}", depot_acte::phrase_de(&e));
                return Ok(2);
            }
        }
        // CE QUI PART, EN GRAND, JUSTE AVANT LA DEMANDE : la derniere chose lue avant le mot de passe.
        annoncer_ce_qui_part(&apercu);
    } else {
        // Un depot de test n'engage rien : il lui faut seulement le secret de l'organisme, donc le coffre ouvert.
        cadre.ouvrir_le_coffre();
    }

    let contournement = args.texte("sans-repetition").unwrap_or("");
    let preuve = Preuve::default();
    let resultat = depot_acte::deposer_dsn_dans(
        &cadre.ctx,
        &json!(chemin),
        reel,
        Some(&preuve),
        "ligne de commande",
        environnement,
        args.drapeau("force"),
        contournement,
        ordre,
    );
    let fait = match resultat {
        Ok(f) => f,
        Err(e) => return Ok(refus_du_depot(&e)),
    };
    dire_la_suite(&fait, reel, &apercu);
    Ok(0)
}

/// Les erreurs d'un depot, comme le Python les rend : une preuve ou un refus du point de depot -> `Refus :` (2) ; une erreur metier ->
/// sa phrase (2) ; le reste -> `Echec du depot` (1).
fn refus_du_depot(e: &Echec) -> i32 {
    let phrase = super::actes::phrase(e);
    match e.classe {
        Some("PreuveInsuffisante") | Some("DepotRefuse") => {
            dire!("\nRefus : {}", phrase);
            2
        }
        c if super::actes::est_operation(c) => {
            dire!("\n{}", phrase);
            2
        }
        _ => {
            dire!("\nEchec du depot : {}", phrase);
            1
        }
    }
}

fn dire_la_suite(fait: &DepotFait, reel: bool, apercu: &Apercu) {
    if socle::fichiers::vrai(&fait.deja) {
        dire!("\nA SAVOIR : ce fichier exact avait deja ete depose le {}.", super::rendu::champ_ou(&fait.deja, "quand", "?"));
    }
    dire_repetition(&fait.repetition);
    dire!("\nReponse du point de depot :\n{}", if fait.reponse.is_empty() { "(vide)" } else { fait.reponse.as_str() });
    dire!("Reponse archivee : {}", fait.trace.as_ref().map(|t| t.display().to_string()).unwrap_or_else(|| "None".into()));
    if !fait.flux.is_empty() {
        dire!("Flux : {} -- c'est par lui que les retours se rattacheront.", fait.flux);
    }
    dire!("\nLe depot n'est pas la fin : les retours arrivent ensuite, et c'est eux qui\ndisent si la declaration est conforme.\n\n  moteur retours");
    if reel && !apercu.est_signalement() {
        dire!("\nPuis, quelques jours plus tard, les attestations France Travail :\n\n  moteur aer --mois {}", apercu.mois_lisible());
    }
}

/// `AAAA-MM` : un mois, pas un fichier.
fn est_un_mois(t: &str) -> bool {
    let b = t.as_bytes();
    b.len() == 7 && b[4] == b'-' && b.iter().enumerate().all(|(i, c)| i == 4 || c.is_ascii_digit())
}
