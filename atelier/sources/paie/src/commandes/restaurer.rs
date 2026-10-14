//! `paie restaurer [DOSSIER] [--racine-drive NOM] [--json]` : redescendre le dossier prive du Drive (lot R1). La decision est
//! dans `restauration.rs` ; ici, l'affichage.
//!
//! Le Drive est celui que la compagnie de ce poste a relie (`paie google autoriser`) : le client lit son jeton dans le dossier
//! des donnees du poste. `DOSSIER` est ou ecrire ; par defaut, ce meme dossier des donnees (le cas d'un poste neuf : relier le
//! Drive, puis restaurer). Dans `paie-atelier`, hors compagnie fictive, la commande exige le faux Google, comme `sauvegarder`.
//!
//! Codes : 0 tout est revenu ; 1 des incidents (un fichier, une empreinte) ; 2 un refus, rien n'a ete ecrit.

use std::path::PathBuf;

use super::actes::refuser_si_pas_redirige;
use super::actes::Organisme;
use super::analyse::{Args, Opt, Pos, Spec};
use super::Cadre;
use crate::drive::Client;
use crate::{restauration, sauvegarde};

pub static SPEC: Spec = Spec {
    commande: "restaurer",
    positionnels: &[Pos::facultatif("dossier", None)],
    options: &[Opt::avec_defaut("racine-drive", sauvegarde::RACINE_DRIVE), Opt::drapeau("json")],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    let json = args.drapeau("json");
    let refus = |classe: &str, message: &str| -> i32 {
        if json {
            dire!("{}", serde_json::json!({"ok": false, "erreur": {"type": classe, "message": message}}));
        } else {
            dire!("! {}", message);
        }
        2
    };
    if let Err(e) = refuser_si_pas_redirige(cadre, Organisme::Google, "la restauration depuis le Drive") {
        return refus("ErreurOperation", &e.message);
    }
    if !crate::routes_google::autorise(&cadre.racine) {
        return refus("ErreurGoogle", "Compte Google non autorise : relier d'abord le Drive de la compagnie.\n  paie google autoriser");
    }
    let racine_drive = args.texte("racine-drive").unwrap_or(sauvegarde::RACINE_DRIVE);
    let cible = args.texte("dossier").filter(|d| !d.is_empty()).map(PathBuf::from).unwrap_or_else(|| cadre.racine.clone());
    let mut client = match Client::nouveau(&cadre.racine) {
        Ok(c) => c,
        Err(e) => return refus(e.classe, &e.message),
    };
    if !json {
        dire!("Restauration de Drive/{}/{} vers {}", racine_drive, sauvegarde::DOSSIER_DRIVE, cible.display());
    }
    let r = match restauration::restaurer(&cible, racine_drive, &mut client) {
        Ok(r) => r,
        Err(e) => return refus(e.classe, &e.message),
    };
    let reste = restauration::a_faire(&crate::coffre::Coffre::du_poste(&cible, ""));
    let code = if r.incidents.is_empty() { 0 } else { 1 };
    if json {
        let mut v = r.en_json();
        v["a_faire"] = serde_json::json!(reste);
        dire!("{}", serde_json::json!({"ok": code == 0, "resultat": v}));
        return code;
    }
    for f in &r.redescendus {
        dire!("  redescendu {}", f.relatif);
    }
    dire!("\n{}", r.resume());
    if r.ecartes > 0 {
        dire!("{} fichier(s) du Drive laisse(s) la-bas : ce que la sauvegarde n'envoie pas.", r.ecartes);
    }
    for i in &r.incidents {
        dire!("  ! {}", i);
    }
    if !reste.is_empty() {
        dire!("\nCe qu'il reste a faire a la main :");
        for l in &reste {
            dire!("  {}", l);
        }
    }
    code
}
