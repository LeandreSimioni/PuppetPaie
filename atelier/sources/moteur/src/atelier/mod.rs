//! L'ATELIER : les outils de l'editeur et des seances, a cote du produit. Le releve des textes (LEGI, KALI, la veille
//! des sources), les referentiels extraits des textes deposes, les tables Unicode generees, le garde avant commit, le
//! guet du contexte, les scenarios du banc, et la publication des versions (la cle de l'editeur).
//!
//! Seuls la redirection vers de faux serveurs locaux, les bulletins fictifs et les outils sur une compagnie (`outils` : les
//! taux de PAS, les AER, la reprise des DPAE, l'enquete des retours, le depot de test a un ordre) n'existent que dans le binaire de
//! l'atelier (`--features atelier`, jamais livre).

pub mod extraire;
pub mod generer_tables;
pub mod guet;
pub mod publier;
pub mod releve;
pub mod scenario;
pub mod sources;
pub mod verifier_avant_commit;

#[cfg(feature = "atelier")]
pub mod bulletins_fictifs;
#[cfg(feature = "atelier")]
pub mod fabriquer_bulletins;
#[cfg(feature = "atelier")]
pub mod redirection;
#[cfg(feature = "atelier")]
pub mod outils;
