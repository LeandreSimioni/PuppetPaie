//! `moteur envoi FICHIER [--sans-navigateur] [--json]` : envoyer a chaque salarie son bulletin, son certificat Conges Spectacles et
//! son AER (`moteur/cli.py`, `cmd_envoi` ; `operations_documents.preparer_envoi` puis l'envoi de la route `envoyer-documents`).
//!
//! SANS LA PAGE DU NAVIGATEUR (decision de Leandre, TRI-J0) : le Python servait une page de relecture locale et n'envoyait qu'au
//! clic. Ici la commande prepare, MONTRE ce qui partirait (les memes lignes que le Python), puis l'envoi est celui du moteur :
//! `crate::routes::documents::expedier_envoi`, donc `depot_acte::verifier_preuve` -- le moteur demande LUI-MEME le mot de passe du logiciel a
//! un humain (fenetre, terminal sans echo ; sinon REFUS), c'est la confirmation. Aucun mot de passe en option, en variable lue en
//! clair ni dans une sortie. Dans `moteur-atelier`, tout envoi reel est refuse (`ActeReelRefuse`). `--sans-navigateur` est accepte
//! (il n'y a plus de navigateur) et sans effet. Dans une compagnie fictive rien ne quitte le poste : la boite simulee ecrit les messages.
//!
//! `--json` s'arrete a la preparation, comme le Python (`envoye: false`) : un programme ne confirme pas un envoi.
//!
//! `qui` : `ligne de commande` -- le Python n'ecrivait pas dans le journal ici (`envoi.envoyer` seul) ; la route le fait, avec ce
//! `qui` (point douteux de la passation).
//!
//! La commande lit le mois (`AAAA-MM`, ou `engagements/AAAA-MM.json`) des donnees. Rien ne s'enchaine a `mois`.
//!
//! DEFAUTS DU PYTHON, QUI NE SE REPRODUISENT PAS : une saisie incomplete, une fiche sans adresse, un fichier introuvable donnaient
//! une trace d'appels. Ici : `! phrase`, code 2.

use serde_json::{json, Value};

use super::actes::{est_refus_d_envoi, mois_du_fichier, ouvrir_la_boite, reponse_metier};
use super::analyse::{Args, Opt, Pos, Spec};
use super::rendu::{enveloppe, erreur, rendre};
use super::Cadre;
use crate::depot_acte::Preuve;
use socle::echec::Echec;

pub static SPEC: Spec = Spec { commande: "envoi", positionnels: &[Pos::requis("fichier")], options: &[Opt::drapeau("sans-navigateur"), Opt::drapeau("json")] };

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    match faire(cadre, args) {
        Ok(c) => c,
        Err(e) if est_refus_d_envoi(e.classe) => reponse_metier("envoi", args.drapeau("json"), &e, &[]),
        Err(e) => cadre.plantage(e),
    }
}

/// Ce que le Python attrapait pour la preparation (`SortieAbsente`, `BoiteAbsente`, `ErreurGoogle`/`ErreurCourriel`) : sa phrase
/// ou son erreur JSON, code 1. `None` pour tout autre refus.
fn preparation_refusee(json: bool, fichier: &str, e: &Echec) -> Option<i32> {
    let classe = e.classe?;
    let message = super::actes::phrase(e);
    match classe {
        "SortieAbsente" => {
            if json {
                let dossier = message.strip_suffix(" n'existe pas.").unwrap_or(&message).to_string();
                return Some(rendre(&erreur("envoi", classe, &message, &[("dossier", json!(dossier))])));
            }
            dire!("Rien a envoyer : {} (mois {}).", message, fichier);
            Some(1)
        }
        "BoiteAbsente" | "ErreurCourriel" | "ErreurGoogle" => {
            if json {
                return Some(rendre(&erreur("envoi", classe, &message, &[])));
            }
            dire!("{}", message);
            Some(1)
        }
        _ => None,
    }
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let json = args.drapeau("json");
    let fichier = args.texte("fichier").unwrap_or("");
    let mois = mois_du_fichier(cadre, fichier, "L'envoi se prepare")?;
    ouvrir_la_boite(cadre);
    let ctx = &cadre.ctx;
    let prep = match crate::routes::documents::preparer_envoi(ctx, &mois, None, &mut || crate::routes::documents::client_courriel(ctx)) {
        Ok(p) => p,
        Err(e) => match preparation_refusee(json, fichier, &e) {
            Some(code) => return Ok(code),
            None => return Err(e),
        },
    };

    if json {
        // `envoi_prepare` : ce qui PARTIRAIT. Rien n'est envoye par une sortie JSON.
        let messages: Vec<Value> = prep
            .messages
            .iter()
            .map(|m| {
                json!({"cle": m.cle, "nom": m.nom, "destinataire": m.destinataire, "sujet": m.sujet,
                       "pieces": m.pieces.iter().map(|p| socle::fichiers::nom_du(p)).collect::<Vec<_>>(), "octets": m.taille()})
            })
            .collect();
        rendre(&enveloppe(
            "envoi",
            json!({
                "mois": prep.mois_saisie, "expediteur": prep.expediteur, "avertissements": prep.avertissements, "messages": messages,
                "envoye": false,
                "note": "Preparation seulement : l'expedition demande le mot de passe du logiciel, tape par un humain au moment de l'envoi. Relancer sans --json pour envoyer.",
            }),
        ));
        return Ok(0);
    }

    for a in &prep.avertissements {
        dire!("Note : {}\n", a);
    }
    let libelle = socle::texte::capitaliser(&engagement::libelle_mois(&prep.mois_saisie));
    dire!("\n{} - {} message(s)\n{}", libelle, prep.messages.len(), "-".repeat(62));
    for m in &prep.messages {
        dire!("  {:<22} {:<30} {} piece(s), {} ko", m.nom, m.destinataire, m.pieces.len(), m.taille() / 1024);
    }
    dire!("{}\n  Expediteur : {}\n", "-".repeat(62), prep.expediteur);

    // L'envoi : la preuve est demandee par le moteur, a un humain. Rien ne part sans elle.
    match crate::routes::documents::expedier_envoi(ctx, prep, &mois, &Preuve::default(), "ligne de commande") {
        Ok(r) => {
            let partis: Vec<String> = super::actes::textes_de(&r, "partis");
            dire!("{} message(s) ENVOYES :", partis.len());
            for a in &partis {
                dire!("  {}", a);
            }
            Ok(0)
        }
        // Une panne en cours d'envoi : « Echec : ... » (le Python), code 2.
        Err(e) if matches!(e.classe, Some("ErreurCourriel") | Some("RuntimeError")) => {
            dire!("Echec : {}", super::actes::phrase(&e));
            Ok(2)
        }
        Err(e) => Err(e),
    }
}
