//! LE BRANCHEMENT : ou part ce qu'un envoi fait partir. Trois branches, et un seul point d'entree que `paie` choisit
//! pour la compagnie et passe aux envois :
//! - **le monde** : les organismes et les personnes, pour de vrai (net-entreprises, l'Urssaf, la boite courriel) ;
//! - **le simulateur** : la compagnie fictive, dont chaque envoi est joue sans quitter le poste (le depot rend un flux
//!   `FICTIF-…`, la boite est simulee, France Travail rend une AER fictive) ;
//! - **le message** : rien ne part ; chaque envoi rend le refus que porte la branche, avant que rien ne s'ecrive.
//!
//! Les envois ne savent pas pourquoi une compagnie est sur une branche : ils obeissent a celle qu'on leur passe.

use socle::echec::{erreur, R};

/// Le refus que rend la branche « message » : sa classe (que la page reconnait) et sa phrase, mot pour mot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refus {
    pub classe: &'static str,
    pub phrase: &'static str,
}

/// Ou part ce qu'un envoi fait partir.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Branchement {
    Monde,
    Simulateur,
    Message(Refus),
}

impl Branchement {
    /// Les envois sont-ils joues par le simulateur ?
    pub fn joue(self) -> bool {
        self == Branchement::Simulateur
    }

    /// Le refus de la branche « message », s'il y a lieu.
    pub fn refus(self) -> Option<Refus> {
        match self {
            Branchement::Message(r) => Some(r),
            _ => None,
        }
    }

    /// La porte d'un envoi : la branche « message » refuse, les deux autres laissent passer. Un envoi l'ouvre avant de
    /// rien lire ni ecrire.
    pub fn ouvrir(self) -> R<()> {
        match self.refus() {
            Some(r) => Err(erreur(r.classe, r.phrase)),
            None => Ok(()),
        }
    }
}
