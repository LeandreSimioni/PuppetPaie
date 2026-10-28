//! `paie deposer-dpae FICHIER [--test | --reel] [--confirmer AAAA-MM] [--majuscules] [--sans-repetition MOTIF] [--fenetre]` :
//! deposer une DPAE deja produite (`moteur/commandes_dpae.py`, `cmd_deposer_dpae`). ESSAI A BLANC par defaut : sans `--test` ni
//! `--reel`, la commande relit le fichier, affiche chaque personne declaree, verifie les delais, et s'arrete.
//!
//! Les garde-fous sont ceux du moteur (`depot_acte::deposer_dpae`), jamais ceux de la commande :
//!
//! - un depot REEL passe par `verifier_preuve` : le moteur demande LUI-MEME le mot de passe du logiciel (fenetre, terminal sans
//!   echo, sinon refus). Aucun mot de passe en option, en variable lue en clair, ni dans une sortie ; `--fenetre` est accepte et
//!   sans effet ;
//! - `--confirmer` est verifie ICI contre les embauches du fichier, AVANT toute demande de mot de passe ;
//! - dans `paie-atelier`, tout depot reel est refuse par `verifier_preuve` (`ActeReelRefuse`) ;
//! - un fichier dont l'indicateur n'est pas celui d'une declaration (C_VERSION != 120) ne part pas, et un depot reel exige une
//!   repetition reussie de ce contenu (`--sans-repetition MOTIF` : le motif ecrit passe, tracé au journal).
//!
//! `qui` = `ligne de commande`, comme le Python.
//!
//! DEFAUTS DU PYTHON, NE SE REPRODUISENT PAS : le jour des delais etait `date.today()` (ici, le jour du contexte, comme les autres
//! commandes) ; une date d'embauche illisible donnait une trace d'appels (ici une phrase) ; la phrase finale citait `dpae
//! --restantes`, option qui n'existe pas (ici : `paie journal --quoi dpae`).

use serde_json::json;

use super::analyse::{Args, Opt, Pos, Spec};
use super::Cadre;
use super::depot_dsn::dire_repetition;
use crate::depot_acte::{self, Preuve};
use socle::echec::Echec;

pub static SPEC: Spec = Spec {
    commande: "deposer-dpae",
    positionnels: &[Pos::requis("fichier")],
    options: &[
        Opt::drapeau("test"),
        Opt::drapeau("reel"),
        Opt::valeur("confirmer"),
        Opt::drapeau("majuscules"),
        Opt::valeur("sans-repetition"),
        Opt::drapeau("fenetre"),
    ],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    match faire(cadre, args) {
        Ok(c) => c,
        Err(e) => cadre.plantage(e),
    }
}

/// `contrats.TYPES_CONTRAT_DPAE`.
fn type_de_contrat(t: &str) -> &'static str {
    match t {
        "1" => "CDD",
        "2" => "CDI",
        "3" => "CTT",
        _ => "?  ",
    }
}

/// `JJMMAAAA` -> le jour, ou le refus d'une phrase (le Python levait une trace d'appels).
fn jour_de(t: &str, quoi: &str) -> Result<socle::date::Jour, Echec> {
    let b = t.as_bytes();
    let bon = b.len() == 8 && b.iter().all(u8::is_ascii_digit);
    let iso = if bon { format!("{}-{}-{}", &t[4..], &t[2..4], &t[..2]) } else { String::new() };
    super::actes::jour_iso(&iso).ok_or_else(|| Echec {
        classe: Some("ErreurOperation"),
        message: format!("{} illisible dans le fichier ({}) : ce n'est pas un flux DPAE que le moteur ait produit. Rien n'est envoye.", quoi, socle::fichiers::repr(t)),
    })
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let (test, reel) = (args.drapeau("test"), args.drapeau("reel"));
    let fichier = args.texte("fichier").unwrap_or("");
    let chemin = match crate::routes_apercus::resoudre(&cadre.racine, &json!(fichier)) {
        Ok(c) => c,
        Err(e) => {
            dire!("Fichier introuvable : {}", e.message);
            return Ok(1);
        }
    };
    let p = std::path::Path::new(&chemin);
    if !p.exists() {
        dire!("Fichier introuvable : {}", chemin);
        dire!("Le produire d'abord :  paie dpae AAAA-MM");
        return Ok(1);
    }
    if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")) {
        dire!("Le SI DPAE refuse les archives : deposer le .txt, pas le .zip.");
        return Ok(1);
    }
    let contenu = match crate::routes_apercus::lire_octets(&chemin) {
        Ok(c) => c,
        Err(e) => {
            dire!("Fichier illisible : {}", e.message);
            return Ok(1);
        }
    };
    if contenu.is_empty() || contenu.len() % declarations::dpae::LONGUEUR != 0 {
        dire!("Ce fichier ne fait pas un multiple de {} octets : ce n'est pas un\nflux DPAE a plat. Rien n'est envoye.", declarations::dpae::LONGUEUR);
        return Ok(1);
    }
    let texte: String = contenu.iter().map(|&b| b as char).collect();
    let enregistrements = declarations::dpae::relire(&texte);
    let champ = |e: &Vec<(&'static str, String)>, k: &str| e.iter().find(|(n, _)| *n == k).map(|(_, v)| v.clone()).unwrap_or_default();

    // ------------------------------------------------------------- relecture
    let nom_fichier = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    dire!("\n{} -- {} declaration(s), {} octets\n{}", nom_fichier, enregistrements.len(), contenu.len(), "-".repeat(66));
    let aujourdhui = cadre.aujourdhui;
    let mut hors_delai = 0usize;
    for e in &enregistrements {
        let debut = match jour_de(&champ(e, "date_embauche"), "Date d'embauche") {
            Ok(j) => j,
            Err(r) => return Ok(super::actes::reponse_metier("deposer-dpae", false, &r, &[])),
        };
        let naissance = match jour_de(&champ(e, "date_naissance"), "Date de naissance") {
            Ok(j) => j,
            Err(r) => return Ok(super::actes::reponse_metier("deposer-dpae", false, &r, &[])),
        };
        let reste = debut.ordinal() - aujourdhui.ordinal();
        let etat = if reste < 0 {
            hors_delai += 1;
            "EMBAUCHE PASSEE - hors delai".to_string()
        } else if reste > 0 {
            format!("dans {} jour(s)", reste)
        } else {
            "aujourd'hui".to_string()
        };
        let heure = champ(e, "heure_embauche");
        let (h, m) = (heure.chars().take(2).collect::<String>(), heure.chars().skip(2).collect::<String>());
        // Le type de contrat se LIT dans le fichier : c'est ce qui separe la DPAE d'un permanent de celle d'un intermittent.
        dire!(
            "  {:<22} {:<12} ne(e) le {}   {} embauche le {} a {}h{}   {}",
            champ(e, "nom").trim(),
            champ(e, "prenom").trim(),
            naissance.jj_mm_aaaa(),
            type_de_contrat(champ(e, "type_contrat").trim()),
            debut.jj_mm_aaaa(),
            h,
            m,
            etat
        );
    }
    dire!("{}", "-".repeat(66));

    // --------------------------------------------------------------- controles
    let indicateur = envois::depot::indicateur(&contenu);
    if !envois::depot::est_reel(&contenu) {
        dire!(
            "\nARRET - champ C_VERSION {} en position {}-{}, attendu {}.\n\nLe fichier produit par le moteur porte toujours {}. Un flux dont on ignore\ns'il sera traite comme un test ou comme une declaration ne part pas d'ici.\nPour un depot de test, ne pas bricoler le fichier : utiliser --test, qui\nbascule C_VERSION en {} en memoire, au moment de l'envoi.\n",
            socle::fichiers::repr(&indicateur),
            envois::depot::INDICATEUR.0,
            envois::depot::INDICATEUR.1,
            socle::fichiers::repr(envois::depot::INDICATEUR_REEL),
            socle::fichiers::repr(envois::depot::INDICATEUR_REEL),
            socle::fichiers::repr(envois::depot::INDICATEUR_TEST)
        );
        return Ok(2);
    }
    if test && reel {
        dire!("\nARRET - --test et --reel s'excluent. Choisir l'un des deux.");
        return Ok(2);
    }
    // Les mois des embauches, AAAA-MM.
    let mut mois: Vec<String> = enregistrements
        .iter()
        .filter_map(|e| {
            let j = champ(e, "date_embauche");
            (j.len() == 8 && j.bytes().all(|b| b.is_ascii_digit())).then(|| format!("{}-{}", &j[4..], &j[2..4]))
        })
        .collect();
    mois.sort();
    mois.dedup();
    if hors_delai > 0 {
        dire!("\n>>> {} embauche(s) deja commencee(s) : la declaration est en retard.", hors_delai);
        dire!(">>> La deposer quand meme, telle quelle, sans antidater.");
    }

    // ------------------------------------------------------------ essai a blanc
    if !reel && !test {
        let adresses = envois::releve_retours::Client::dpae(&cadre.ctx, false, None).adresses;
        dire!(
            "\nESSAI A BLANC - rien n'a ete envoye.\n\nCe qui partirait :\n  {} declaration(s), {} octets, gzip\n  POST {}\n  authentification {}\n\nLe depot de TEST d'abord -- il est EXIGE avant le reel, et rien n'y est\ndeclare (C_VERSION bascule en {}, le fichier n'est pas modifie) :\n  paie deposer-dpae {} --test\n\nPuis, le meme fichier, pour de vrai :\n  paie deposer-dpae {} --reel\n",
            enregistrements.len(),
            contenu.len(),
            adresses.depot,
            adresses.authentification,
            envois::depot::INDICATEUR_TEST,
            fichier,
            fichier
        );
        return Ok(0);
    }

    // ---------------------------------------------------------- confirmation
    // Le mois ne se recopie plus : il s'affiche. Donne quand meme, il est verifie -- un script qui se trompe de mois est arrete ici,
    // avant le reseau et AVANT TOUTE DEMANDE DE MOT DE PASSE.
    let confirmer = args.texte("confirmer").filter(|c| !c.is_empty());
    if let Some(c) = confirmer {
        if reel && !mois.iter().any(|m| m == c) {
            dire!("\nARRET - mois confirme ({}) different des embauches du fichier ({}).", c, mois.join(", "));
            return Ok(2);
        }
    }

    if test {
        dire!(
            "\nDEPOT DE TEST - C_VERSION bascule de {} en {} sur les {} enregistrement(s).\n\nLe service de production accepte les depots de test : la FAQ de l'Urssaf le\nprevoit explicitement. Rien ne sera declare, et les retours porteront la\nmention de test. Le fichier sur le disque n'est pas modifie.\n",
            envois::depot::INDICATEUR_REEL,
            envois::depot::INDICATEUR_TEST,
            enregistrements.len()
        );
    }
    let quoi = if test { "TEST".to_string() } else { confirmer.map(str::to_string).unwrap_or_else(|| mois.join(", ")) };
    dire!("\nDepot de {} declaration(s) -- {}...", enregistrements.len(), quoi);
    if reel {
        // CE QUI PART, EN GRAND, JUSTE AVANT LA DEMANDE.
        let embauches = if mois.is_empty() { "?".to_string() } else { mois.join(", ") };
        dire!("\n    Mois des embauches declarees par ce fichier");
        dire!("    {}", "=".repeat(44));
        dire!("        >>>  {}  <<<   {} declaration(s)\n", embauches, enregistrements.len());
    } else {
        // Un depot de test n'engage rien : il lui faut seulement le secret de l'organisme, donc le coffre ouvert.
        cadre.ouvrir_le_coffre();
    }

    let contournement = args.texte("sans-repetition").unwrap_or("");
    let preuve = Preuve::default();
    let fait = match depot_acte::deposer_dpae(
        &cadre.ctx,
        &json!(chemin),
        reel,
        Some(&preuve),
        "ligne de commande",
        args.drapeau("majuscules"),
        contournement,
    ) {
        Ok(f) => f,
        Err(e) => return Ok(refus_du_depot(&e, test)),
    };

    if socle::fichiers::vrai(&fait.deja) {
        dire!("\nA SAVOIR : ce fichier exact avait deja ete depose le {}.", super::rendu::champ_ou(&fait.deja, "quand", "?"));
    }
    dire_repetition(&fait.repetition);
    // RECU N'EST PAS ACCEPTE : la reponse du depot est l'accuse de transport ; c'est le bilan (nature 11) qui tranche.
    dire!("\nDepot {} recu par le SI DPAE (accuse de transport).", if test { "DE TEST" } else { "REEL" });
    dire!("Reponse archivee : {}", fait.trace.as_ref().map(|t| t.display().to_string()).unwrap_or_else(|| "None".into()));
    dire!("\n{}", fait.reponse.chars().take(2000).collect::<String>());
    let bilan = if fait.flux.is_empty() { "paie retours --dpae".to_string() } else { format!("paie retours --dpae --flux {}", fait.flux) };
    if test {
        dire!(
            "\nC'etait un TEST : rien n'a ete declare, aucune embauche n'est enregistree.\nLe transport est valide -- authentification, depot, accuse. La conformite du\nfichier, elle, se lit dans le bilan (nature 11), qui arrive ensuite :\n\n      {}\n",
            bilan
        );
    } else {
        dire!(
            "\nEnsuite : les retours (accuse, certificat de conformite, bilan\nd'identification des salaries) sont publies par l'Urssaf et se consultent sur\nle tableau de bord DPAE. Ils ont une retention de 3 mois.\n\nLes embauches deposees sont notees au journal : `paie journal --quoi dpae`\nles montre ; `{}` en releve les retours.\n",
            bilan
        );
    }
    Ok(0)
}

/// Les erreurs d'un depot, comme le Python les rend : `Refus :` (2) pour la preuve, la phrase (2) pour une erreur metier, `Le depot a
/// echoue` (1) pour le service ou le reseau, `Echec du depot` (1) pour le reste.
fn refus_du_depot(e: &Echec, test: bool) -> i32 {
    let phrase = super::actes::phrase(e);
    match e.classe {
        Some("PreuveInsuffisante") => {
            dire!("\nRefus : {}", phrase);
            2
        }
        c if super::actes::est_operation(c) => {
            dire!("\n{}", phrase);
            2
        }
        Some("ErreurApi") | Some("DepotRefuse") | Some("URLError") | Some("RemoteDisconnected") | Some("TimeoutError") => {
            dire!("\nLe depot a echoue.\n{}", phrase);
            if !test {
                dire!("\nRien ne garantit qu'il n'a pas ete pris en compte : verifier le");
                dire!("tableau de bord DPAE avant de recommencer, pour ne pas declarer");
                dire!("deux fois les memes embauches.");
            }
            1
        }
        _ => {
            dire!("\nEchec du depot : {}", phrase);
            1
        }
    }
}
