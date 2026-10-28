//! `moteur sauvegarder [--tout] [--point] [--racine-drive NOM]` : le dossier prive dans le Drive, par differences
//! (`moteur/commandes_ouverture.py`, `cmd_sauvegarder`). Ne supprime jamais rien la-bas.
//!
//! `--json` est accepte et sans effet (le Python n'a pas de sortie JSON ici). Ni mot de passe ni jeton ne passe par la commande : le
//! client Drive lit le jeton au coffre (`sauvegarde::sauvegarder`, `Client::nouveau`). Dans `moteur-atelier`, hors compagnie fictive, la
//! commande exige le faux Google (`PUPPETPAIE_ATELIER_GOOGLE`) : sinon elle irait au vrai Drive.
//!
//! DEFAUT DU PYTHON, QUI NE SE REPRODUIT PAS : une `ErreurGoogle` (ou une erreur de disque) hors des cas attrapes sortait en trace
//! d'appels ; ici `! phrase` et le code 2.

use super::actes::refuser_si_pas_redirige;
use super::actes::Organisme;
use super::analyse::{Args, Opt, Spec};
use super::rendu::texte;
use super::Cadre;
use crate::sauvegarde;

pub static SPEC: Spec = Spec {
    commande: "sauvegarder",
    positionnels: &[],
    options: &[Opt::drapeau("tout"), Opt::drapeau("point"), Opt::avec_defaut("racine-drive", sauvegarde::RACINE_DRIVE), Opt::drapeau("json")],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    if let Err(e) = refuser_si_pas_redirige(cadre, Organisme::Google, "la sauvegarde dans le Drive") {
        dire!("! {}", e.message);
        return 2;
    }
    if !crate::routes::coffre::autorise(&cadre.racine) {
        dire!("Compte Google non autorise.\n  moteur google autoriser");
        return 2;
    }
    let racine_drive = args.texte("racine-drive").unwrap_or(sauvegarde::RACINE_DRIVE);
    if args.drapeau("point") {
        let d = sauvegarde::derniere(&cadre.racine);
        let quand = if socle::fichiers::vrai(&d.quand) { texte(&d.quand) } else { "jamais".to_string() };
        dire!("Dernier passage : {} ({} fichier(s) suivis)", quand, d.fichiers);
        dire!("Rien n'a bouge depuis : {}", if sauvegarde::rien_n_a_bouge(&cadre.racine, racine_drive) { "oui" } else { "non" });
        return 0;
    }
    dire!("Sauvegarde de {} vers Drive/{}/{}", cadre.racine.display(), racine_drive, sauvegarde::DOSSIER_DRIVE);
    let r = match sauvegarde::sauvegarder(&cadre.racine, racine_drive, args.drapeau("tout"), None) {
        Ok(r) => r,
        Err(e) => {
            dire!("! {}", e.message);
            return 2;
        }
    };
    for f in &r.deposes {
        dire!("  {:<8} {}", f.etat.texte(), f.relatif);
    }
    dire!("\n{}", r.resume());
    dire!("{} fichier(s) ecarte(s) : secrets, verrous, rebut.", r.exclus);
    for i in &r.incidents {
        dire!("  ! {}", i);
    }
    if let Some(p) = &r.persistant {
        dire!("{}", p);
    }
    dire!("En {} s.", socle::ecriture::repr_flottant(r.secondes));
    if r.incidents.is_empty() {
        0
    } else {
        1
    }
}
