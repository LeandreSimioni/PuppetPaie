//! L'ATELIER : les outils de l'editeur et des seances, a cote du produit. Le releve des textes (LEGI, KALI, la veille
//! des sources), les referentiels extraits des textes deposes, les tables Unicode generees, le garde avant commit, le
//! guet du contexte, les scenarios du banc, et la publication des versions (la cle de l'editeur).
//!
//! Seuls la redirection vers de faux serveurs locaux et les bulletins fictifs n'existent que dans le binaire de
//! l'atelier (`--features atelier`, jamais livre).

pub mod extraire;
pub mod generer_tables;
pub mod guet;
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
