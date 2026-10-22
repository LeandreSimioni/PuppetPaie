//! `paie coffre [etat|poser|oublier|ouverture|verifier|seance]` : les mots de passe des organismes, scelles sur le disque
//! (`moteur/commandes_ouverture.py`, `cmd_coffre`). Jamais une valeur dans une sortie.
//!
//! LE COFFRE, ET COMMENT IL EST TENU ICI.
//!
//! - Un mot de passe ne se donne JAMAIS en option, en argument ni en variable : `poser` et `ouverture` le demandent par le moteur
//!   (`demande::saisir_secret` : la fenetre du moteur s'il y en a une, sinon le terminal SANS ECHO par `/dev/tty` -- ni l'entree ni la
//!   sortie standard --, sinon un refus). `--fenetre` est accepte et sans effet (la fenetre du moteur est au lot J3). Rien de ce qui est
//!   tape n'est ecrit, journalise, repete ou rendu : il va au coffre, qui le scelle, puis il est lache.
//! - Poser, oublier, changer le mot de passe du logiciel sont des ACTES : si un mot de passe du logiciel est pose, le moteur le
//!   demande LUI-MEME a un humain avant (`filet::exiger_mot_de_passe`, comme la page), sinon il refuse ; le Python ne demandait rien
//!   ici (un ecart voulu : la page le demandait deja). Dans `paie-atelier`, hors compagnie fictive, ces gestes sont refuses.
//! - `verifier` ne lit jamais un secret : il appelle la route `coffre-verifier`, qui ne rend qu'un verdict (UNE tentative par compte).
//! - `seance` : le mot de passe du logiciel tape une fois (par `demande::ouvrir_seance`), garde 12 h ; il ouvre le coffre, il
//!   n'autorise jamais un acte reel.
//!
//! `--json` est accepte et sans effet (le Python n'a pas de sortie JSON pour le coffre). Les phrases d'une demande de mot de passe vont
//! sur la sortie d'erreur ou sur le terminal, jamais sur la sortie standard d'un programme qui lirait du JSON.
//!
//! DEFAUTS DU PYTHON, QUI NE SE REPRODUISENT PAS : `oublier` d'un service inconnu, `poser` d'un secret que le coffre refuse (une
//! panne de disque) sortaient en trace d'appels ; ici une phrase. Le Python ne demandait pas le mot de passe du logiciel pour
//! poser ou oublier un secret ni pour changer celui du logiciel quand la cle du poste ouvrait le coffre.

use serde_json::{json, Value};

use super::actes::{est_refus, maintenant_minutes, refuser_dans_l_atelier, services_connus};
use super::analyse::{Args, Opt, Pos, Spec};
use super::rendu::{champ, texte};
use super::Cadre;
use crate::coffre::{Coffre, SERVICES};
use crate::retours::Echec;

pub static SPEC: Spec = Spec {
    commande: "coffre",
    positionnels: &[
        Pos::parmi("quoi", &["etat", "poser", "oublier", "ouverture", "verifier", "seance"], Some("etat")),
        Pos::facultatif("service", None),
    ],
    options: &[Opt::valeur("compte"), Opt::drapeau("fenetre"), Opt::drapeau("json")],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    let quoi = args.texte("quoi").unwrap_or("etat");
    let r = match quoi {
        "poser" => poser(cadre, args),
        "oublier" => oublier(cadre, args),
        "ouverture" => ouverture(cadre),
        "verifier" => verifier(cadre),
        "seance" => seance(cadre, args),
        _ => faire(cadre),
    };
    match r {
        Ok(c) => c,
        // Un refus (le mot de passe du logiciel non tape, l'atelier, une phrase du coffre) : une phrase, code 2.
        Err(e) if est_refus(e.classe) || e.classe == Some("ErreurCoffre") => {
            dire!("! {}", e.message);
            2
        }
        Err(e) => cadre.plantage(e),
    }
}

fn vers(e: crate::coffre::Echec) -> Echec {
    Echec { classe: None, message: e.message }
}

/// Le refus du coffre sous sa classe (`ErreurCoffre`, `OSError`...), pour que la commande sache s'il se rend en phrase.
fn vers_classe(e: crate::coffre::Echec) -> Echec {
    Echec { classe: Some(e.classe), message: e.message }
}

fn faire(cadre: &Cadre) -> Result<i32, Echec> {
    let c = Coffre::du_poste(&cadre.racine, "");
    dire!("Coffre : {}", c.chemin().display());
    dire!("  scelle (AES-256-GCM). Cle de ce poste : {}", c.chemin_cle().display());
    if !c.cle_posee().map_err(vers)? {
        // Pas de coffre non plus : il n'y a rien d'anormal a signaler, la cle sera tiree au premier secret pose.
        if c.chemin().exists() {
            dire!("  Elle est ABSENTE, et le coffre est la : il ne s'ouvrira");
            dire!("  qu'avec le mot de passe du logiciel, qui reposera une");
            dire!("  cle pour ce poste au passage.");
        } else {
            dire!("  Pas encore tiree -- elle le sera au premier secret pose.");
        }
    }
    dire!();
    for s in c.etat().map_err(vers)? {
        let mut marque = if s.get("pose").is_some_and(crate::fichiers::vrai) { "pose" } else { "-" };
        if s.get("dans_environnement").is_some_and(crate::fichiers::vrai) {
            marque = "environnement";
        }
        let depuis = match s.get("pose_le") {
            Some(d) if crate::fichiers::vrai(d) => format!("depuis le {}", texte(d)),
            _ => String::new(),
        };
        dire!("  {:<18} {:<14} {}", champ(&s, "service"), marque, depuis);
        dire!("     {}", champ(&s, "sert_a"));
        if let Some(Value::String(compte)) = s.get("compte").filter(|v| crate::fichiers::vrai(v)) {
            dire!("     boite : {}", compte);
        }
    }
    dire!();
    if !c.ouverture_exigee().map_err(vers)? {
        dire!("  Mot de passe du logiciel : AUCUN. Rien ne part en reel tant");
        dire!("  qu'il n'est pas pose.");
        dire!("  En poser un :  paie coffre ouverture");
    } else {
        dire!("  Mot de passe du logiciel : pose. Il est demande a l'ouverture");
        dire!("  de la fenetre et a chaque depot reel, envoi ou signature.");
    }
    Ok(0)
}

// ------------------------------------------------------------------ les gestes

/// Le moteur demande LUI-MEME le mot de passe du logiciel a un humain, s'il en est un de pose (`filet::exiger_mot_de_passe`) ;
/// sinon, il refuse. Le mot de passe tape est retenu pour la commande : il ouvre le coffre, il ne sort pas.
fn exiger(cadre: &Cadre, quoi: &str) -> Result<(), Echec> {
    crate::filet::exiger_mot_de_passe(&cadre.racine, &Value::Null, quoi)
}

/// Le service nomme a la ligne de commande : sa fiche, ou la phrase qui dit lesquels existent (code 2, comme le Python).
fn service_demande(args: &Args, inconnu: &str) -> Result<&'static crate::coffre::Service, i32> {
    let Some(nom) = args.texte("service").filter(|s| !s.is_empty()) else {
        dire!("Quel service ? {}", services_connus().join(", "));
        return Err(2);
    };
    match SERVICES.iter().find(|s| s.nom == nom) {
        Some(s) => Ok(s),
        None => {
            dire!("{} Connus : {}", inconnu, services_connus().join(", "));
            Err(2)
        }
    }
}

/// Deux saisies du meme secret, masquees. `Err(code)` : la phrase est dite, la commande s'arrete (rien n'est pose).
fn saisir_deux_fois(titre: &str, lignes: &[String], invite: &str, invite_bis: &str) -> Result<String, i32> {
    let mdp = match crate::demande::saisir_secret(titre, lignes, invite) {
        Ok(m) => m,
        Err(phrase) => {
            dire!("{}", phrase);
            return Err(2);
        }
    };
    let bis = match crate::demande::saisir_secret("PuppetPaie : le retaper", &[], invite_bis) {
        Ok(m) => m,
        Err(phrase) => {
            dire!("{}", phrase);
            return Err(2);
        }
    };
    if bis != mdp {
        dire!("Les deux saisies different. Rien n'a ete pose.");
        return Err(2);
    }
    Ok(mdp)
}

fn poser(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let fiche = match service_demande(args, "Service inconnu.") {
        Ok(f) => f,
        Err(code) => return Ok(code),
    };
    refuser_dans_l_atelier(cadre, "de poser un mot de passe")?;
    dire!("Mot de passe {}.", fiche.libelle);
    dire!("Il sert a : {}.", fiche.sert_a);
    dire!("Il sera chiffre par le compte Windows et jamais affiche.");
    let compte = args.texte("compte").filter(|c| !c.is_empty());
    if fiche.avec_compte {
        let Some(compte) = compte else {
            dire!("\nCe service se pose avec l'adresse de la boite :");
            dire!("  paie coffre poser {} --compte adresse@...", fiche.nom);
            dire!("Pour Gmail, le mot de passe est un mot de passe D'APPLICATION");
            dire!("(validation en deux etapes, puis myaccount.google.com > Securite");
            dire!("> Mots de passe des applications), pas celui du compte.");
            return Ok(2);
        };
        dire!("Boite : {}", compte);
    }
    // Si un mot de passe du logiciel est pose, un humain le tape d'abord (le moteur le demande lui-meme) ; sinon on s'arrete la.
    exiger(cadre, &format!("poser le mot de passe {}", fiche.libelle))?;
    let lignes = vec![
        format!("Service : {}", fiche.libelle),
        "La frappe ne s'affiche pas : elle est scellee dans le coffre, nulle part ailleurs.".to_string(),
    ];
    let mdp = match saisir_deux_fois("PuppetPaie : poser un mot de passe", &lignes, "mot de passe (rien ne s'affiche) : ", "le retaper (rien ne s'affiche) : ") {
        Ok(m) => m,
        Err(code) => return Ok(code),
    };
    let coffre = Coffre::du_poste(&cadre.racine, &maintenant_minutes());
    match coffre.poser(fiche.nom, &mdp, compte) {
        Ok(_) => {}
        Err(e) if e.classe == "ErreurCoffre" => {
            dire!("\n{}", e.message);
            return Ok(2);
        }
        Err(e) => return Err(vers_classe(e)),
    }
    drop(mdp);
    dire!("Pose. `paie coffre` dit desormais qu'il est la, jamais ce qu'il vaut.");
    Ok(0)
}

fn oublier(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let fiche = match service_demande(args, "Service inconnu.") {
        Ok(f) => f,
        Err(code) => return Ok(code),
    };
    refuser_dans_l_atelier(cadre, "d'oublier un mot de passe")?;
    let coffre = Coffre::du_poste(&cadre.racine, &maintenant_minutes());
    // Rien a oublier : inutile de demander quoi que ce soit a quiconque.
    if !coffre.pose(fiche.nom).map_err(vers_classe)? {
        dire!("Il n'y avait rien pour ce service.");
        return Ok(0);
    }
    exiger(cadre, &format!("retirer le mot de passe {}", fiche.libelle))?;
    let efface = coffre.oublier(fiche.nom).map_err(vers_classe)?;
    dire!("{}", if efface { "Oublie." } else { "Il n'y avait rien pour ce service." });
    Ok(0)
}

fn ouverture(cadre: &Cadre) -> Result<i32, Echec> {
    refuser_dans_l_atelier(cadre, "de changer le mot de passe du logiciel")?;
    dire!("Mot de passe DU LOGICIEL -- pas celui d'un organisme.");
    dire!("Il est demande a l'ouverture de la fenetre PuppetPaie, puis a");
    dire!("chaque acte qui engage : depot reel, envoi, signature.");
    dire!("Il n'est pas ecrit : il scelle une enveloppe de la cle du coffre,");
    dire!("et une enveloppe ne rend rien a qui n'a pas le mot de passe.");
    let coffre = Coffre::du_poste(&cadre.racine, &maintenant_minutes());
    // UNE COMPAGNIE DE L'ADMINISTRATEUR (ticket #244) : son mot de passe est aussi celui des autres ; il se change pour
    // toutes a la fois, dans la fenetre (Configuration > Mots de passe), jamais pour une seule ici.
    if crate::administrateur::est_rattachee(&crate::administrateur::installation_de(&cadre.racine), &cadre.racine) {
        dire!();
        dire!("Cette compagnie est geree par l'administrateur : son mot de passe ouvre aussi ses autres");
        dire!("compagnies, et il se change pour toutes a la fois, dans la fenetre PuppetPaie");
        dire!("(Configuration > Mots de passe). Rien n'a ete modifie.");
        return Ok(2);
    }
    // LE MOT DE PASSE ACTUEL, quand la cle de ce poste n'ouvre plus le coffre : le moteur le demande avec la preuve de l'acte
    // (ci-dessous), et ce meme mot de passe ouvre le coffre pour la suite de la commande.
    if !coffre.ouvre_a_ce_poste().map_err(vers_classe)? {
        dire!();
        dire!("La cle de ce poste n'ouvre pas ce coffre -- il a ete ferme");
        dire!("ailleurs (sauvegarde restauree, poste neuf, autre session).");
        dire!("Son mot de passe ACTUEL l'ouvre, et une cle sera reposee ici.");
        dire!();
    }
    // S'il y a deja un mot de passe du logiciel, un humain tape l'actuel (le moteur le demande lui-meme) ; le Python ne le
    // demandait que si la cle du poste n'ouvrait pas le coffre.
    exiger(cadre, "changer le mot de passe du logiciel")?;
    let lignes = vec!["Le NOUVEAU mot de passe du logiciel (pas celui d'un organisme).".to_string(), "La frappe ne s'affiche pas.".to_string()];
    let mdp = match saisir_deux_fois(
        "PuppetPaie : mot de passe du logiciel",
        &lignes,
        "mot de passe (rien ne s'affiche) : ",
        "le retaper (rien ne s'affiche) : ",
    ) {
        Ok(m) => m,
        Err(code) => return Ok(code),
    };
    match coffre.poser_ouverture(&mdp, None) {
        Ok(_) => {}
        Err(e) if e.classe == "ErreurCoffre" => {
            dire!("\n{}", e.message);
            return Ok(2);
        }
        Err(e) => return Err(vers_classe(e)),
    }
    drop(mdp);
    dire!("Pose. Il sera demande a la prochaine ouverture de la fenetre, et a");
    dire!("chaque depot.");
    Ok(0)
}

/// `coffre verifier` : le secret pose ouvre-t-il vraiment une session ? UNE tentative par compte, jamais de boucle. Rien du secret
/// ne passe ici : la route rend un verdict (`ok`, `message`, `conseil`).
fn verifier(cadre: &Cadre) -> Result<i32, Echec> {
    use super::actes::{boite_interdite, pas_redirige, Organisme};
    dire!("Verification des secrets poses -- UNE tentative par compte.");
    dire!();
    // La cle du poste ouvre le coffre, ou une seance (le moteur la demande lui-meme).
    cadre.ouvrir_le_coffre();
    let coffre = Coffre::du_poste(&cadre.racine, "");
    let mut essais: Vec<&str> = Vec::new();
    for service in ["net-entreprises", "urssaf", "courriel"] {
        if !coffre.disponible(service).map_err(vers_classe)? {
            dire!("  {:<16} pas pose -- rien a verifier", service);
            continue;
        }
        // `paie-atelier` ne parle qu'aux faux serveurs : ni a la boite, ni a un organisme non redirige.
        let hors = match service {
            "net-entreprises" => pas_redirige(cadre, Organisme::Dsn),
            "urssaf" => pas_redirige(cadre, Organisme::Dpae),
            _ => boite_interdite(cadre),
        };
        if hors {
            dire!("  {:<16} non verifie : le binaire de l'atelier ne tient pas ce serveur", service);
            continue;
        }
        match cadre.route("coffre-verifier", json!({"service": service})) {
            Ok(v) if v.get("ok").is_some_and(crate::fichiers::vrai) => dire!("  {:<16} OK -- session ouverte", service),
            Ok(v) => {
                essais.push(service);
                dire!("  {:<16} REFUSE : {}", service, champ(&v, "message"));
                if v.get("conseil").is_some_and(crate::fichiers::vrai) {
                    dire!("       {}", champ(&v, "conseil"));
                }
            }
            // Une panne (reseau, boite, coffre qui ne s'ouvre pas) : le Python sortait en trace d'appels. Ici, le verdict est un refus
            // d'une ligne -- la premiere de la phrase, qui ne porte jamais un secret.
            Err(e) => {
                essais.push(service);
                dire!("  {:<16} REFUSE : {}", service, e.message.lines().next().unwrap_or(""));
            }
        }
    }
    if !essais.is_empty() {
        dire!("\nNe pas relancer cette commande en boucle : chaque essai compte.");
    }
    Ok(if essais.is_empty() { 0 } else { 1 })
}

/// `coffre seance` : taper le mot de passe du logiciel UNE fois ; les commandes suivantes ouvrent le coffre sans le redemander, 12 h
/// au plus. `coffre seance fermer` la clot. Jamais une preuve d'acte reel.
fn seance(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let coffre = Coffre::du_poste(&cadre.racine, &maintenant_minutes());
    if args.texte("service") == Some("fermer") {
        let fermee = coffre.oublier_ouverture_gardee().map_err(vers_classe)?;
        dire!("{}", if fermee { "Seance fermee." } else { "Aucune seance ouverte." });
        return Ok(0);
    }
    if let Some(fin) = coffre.fin_ouverture_gardee().map_err(vers_classe)? {
        if coffre.ouverture_gardee().map_err(vers_classe)?.is_some() {
            dire!("Seance deja ouverte, jusqu'au {}.", fin.replace('T', " a "));
            return Ok(0);
        }
    }
    match crate::demande::ouvrir_seance(&cadre.racine) {
        crate::demande::Verdict::Accepte => {
            let fin = coffre.fin_ouverture_gardee().map_err(vers_classe)?.unwrap_or_default();
            dire!(
                "Seance ouverte jusqu'au {} : le coffre s'ouvre sans redemander le mot de passe du logiciel. Un depot REEL le redemande toujours.",
                fin.replace('T', " a ")
            );
            Ok(0)
        }
        crate::demande::Verdict::Refuse(phrase) => {
            dire!("! {}", phrase);
            Ok(2)
        }
    }
}
