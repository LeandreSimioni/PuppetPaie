//! La fabrique de PDF de PuppetPaie : le contrat d'interface est `FORMAT.md`.

pub mod assemblage;
pub mod chiffrement;
pub mod description;
pub mod formulaire;
pub mod glyphes;
pub mod inspection;
pub mod lecture;
pub mod ligne_de_commande;
pub mod marquage;
pub mod mise_en_page;
pub mod modele;
pub mod polices;
pub mod style_contrat;
pub mod texte;

pub use assemblage::assembler;
pub use description::{Refus, FORMAT};
pub use mise_en_page::fabriquer;
