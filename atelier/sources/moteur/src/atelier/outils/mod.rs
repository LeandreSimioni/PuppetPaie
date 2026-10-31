//! LES OUTILS DE L'ATELIER SUR UNE COMPAGNIE : des enquetes et des gestes d'editeur qui ne sont pas des gestes de l'ecran,
//! dans le binaire de l'atelier seulement (`--features atelier`, `moteur-atelier`). Ils prennent le cadre des commandes
//! (`commandes::Cadre` : la compagnie, le jour, la demande du mot de passe) et appellent les fonctions du moteur ; aucun
//! ne recopie une regle.
//!
//! - `pas` : les taux de prelevement a la source connus, `--amorcer` depuis les DSN deposees, `--demander` les
//!   signalements d'amorcage (nature 08), ecrits sans etre deposes ;
//! - `aer` : ranger de nouveau les attestations France Travail recues, `--forcer` pour signer malgre un ecart ;
//! - `journal --dpae` : reprendre au journal les DPAE que les certificats de l'Urssaf attestent ;
//! - `retours` : relever les retours d'une plage, d'un flux, sur la plate-forme de l'editeur (l'enquete) ;
//! - `depot-dsn` : un depot de TEST a un ordre donne (`--ordre`) ou sur la plate-forme de l'editeur.

pub mod aer;
pub mod depot_dsn;
pub mod journal;
pub mod pas;
pub mod retours;

/// Une erreur d'outil : sa phrase, code 2.
pub(crate) fn refus(e: &socle::echec::Echec) -> i32 {
    dire!("! {}", e.message);
    2
}
