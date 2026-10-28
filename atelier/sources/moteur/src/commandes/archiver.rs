//! `moteur archiver MOIS [--racine-drive NOM]` : deposer les sorties d'un mois dans le Drive (`moteur/cli.py`, `cmd_archiver`).
//! `archivage::archiver` fait le travail ; le jeton est lu au coffre par le client Drive, jamais passe par la commande.
//!
//! Dans `moteur-atelier`, hors compagnie fictive, la commande exige le faux Google (`PUPPETPAIE_ATELIER_GOOGLE`).
//!
//! DEFAUTS DU PYTHON, QUI NE SE REPRODUISENT PAS : un mois illisible (`..`) se lisait tel quel (corrige dans `archivage.rs` : refus
//! `ErreurOperation`) ; une erreur de disque hors `ErreurGoogle` sortait en trace d'appels.

use serde_json::{json, Value};

use super::actes::{refuser_si_pas_redirige, reponse_metier, Organisme};
use super::analyse::{Args, Opt, Pos, Spec};
use super::rendu::{enveloppe, erreur, rendre};
use super::Cadre;
use socle::echec::Echec;

pub static SPEC: Spec = Spec {
    commande: "archiver",
    positionnels: &[Pos::requis("mois")],
    options: &[Opt::avec_defaut("racine-drive", crate::sauvegarde::RACINE_DRIVE), Opt::drapeau("json")],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    let json = args.drapeau("json");
    let mois = args.texte("mois").unwrap_or("");
    let racine_drive = args.texte("racine-drive").unwrap_or(crate::sauvegarde::RACINE_DRIVE);
    if let Err(e) = refuser_si_pas_redirige(cadre, Organisme::Google, "l'archivage dans le Drive") {
        return reponse_metier("archiver", json, &e, &[]);
    }
    let r = match crate::archivage::archiver(&cadre.racine, mois, racine_drive, None) {
        Ok(r) => r,
        Err(e) if e.classe == "SortieAbsente" => {
            if json {
                let dossier = cadre.racine.join("sorties").join(mois);
                return rendre(&erreur("archiver", e.classe, &e.message, &[("dossier", json!(dossier.to_string_lossy()))]));
            }
            dire!("Rien a archiver : {}", e.message);
            return 1;
        }
        // `ErreurGoogle` et `ErreurOperation` (mois illisible) : une phrase, code 2. Le reste (disque) etait une trace d'appels.
        Err(e) => {
            let echec = Echec { classe: Some(e.classe), message: e.message.clone() };
            if matches!(e.classe, "ErreurGoogle" | "ErreurOperation" | "FileNotFoundError" | "PermissionError" | "OSError") {
                if json {
                    return rendre(&erreur("archiver", e.classe, &e.message, &[]));
                }
                dire!("{}", e.message);
                return 2;
            }
            return cadre.plantage(echec);
        }
    };
    if json {
        let fichiers: Vec<Value> =
            r.fichiers.iter().map(|f| json!({"nom": f.nom, "octets": f.octets, "remplace": f.remplace})).collect();
        return rendre(&enveloppe(
            "archiver",
            json!({"mois": r.mois, "racine_drive": r.racine_drive, "compte": r.compte, "fichiers": fichiers}),
        ));
    }
    let tirets = "-".repeat(62);
    dire!("Drive : {} / {}   (compte {})\n{}", r.racine_drive, r.mois, r.compte.as_deref().unwrap_or("None"), tirets);
    for f in &r.fichiers {
        dire!("  {:<40} {:>6} ko  {}", f.nom.as_deref().unwrap_or("None"), f.octets / 1024, if f.remplace { "remplace" } else { "nouveau" });
    }
    dire!("{}\n{} fichier(s) archive(s).", tirets, r.fichiers.len());
    0
}
