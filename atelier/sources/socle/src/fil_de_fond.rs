//! LE FIL DE FOND : le fil de la fenetre qui tient les etats des compagnies a jour des le lancement, AVANT
//! tout mot de passe (`etat_garde::Fil`). Il ne fait que du calcul local : lire les fichiers de la compagnie (en clair sur
//! le disque) et ecrire ses etats gardes. Ce module le marque, et deux gardes le tiennent, quoi qu'une lecture demande :
//! - `garde_reseau::sortie_vers` refuse toute sortie hors de la machine depuis ce fil ;
//! - le coffre (`coffre.rs`) refuse d'y servir la cle du poste ou de deriver un mot de passe : rien ne s'y dechiffre.
//! Ce qui est en clair dans un coffre (ce qui est pose, depuis quand, s'il a un mot de passe) se lit comme ailleurs.

use std::cell::Cell;

thread_local! {
    static DE_FOND: Cell<bool> = const { Cell::new(false) };
}

/// Ce fil est le fil de fond (pour toute sa vie).
pub fn marquer() {
    DE_FOND.with(|d| d.set(true));
}

/// Sommes-nous sur le fil de fond ?
pub fn ici() -> bool {
    DE_FOND.with(|d| d.get())
}

/// Le refus, en une phrase.
pub fn refus(quoi: &str) -> String {
    format!("Le calcul en fond de la fenetre ne fait que lire et calculer : {} lui est refuse.", quoi)
}
