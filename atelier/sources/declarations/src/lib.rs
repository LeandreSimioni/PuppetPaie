//! Les déclarations : la DSN mensuelle et ses signalements, la DPAE, les régularisations, les contrôles de la norme
//! (usages, formats, natures), et les états qui disent ce qu'une DSN a porté. Chacune se fabrique depuis les paies écrites
//! de l'engagement validé ; ce qui a été déclaré se lit dans la version sur laquelle la déclaration est partie.

pub mod amorcage;
pub mod assujettissement;
pub mod casc;
pub mod casc_regularisation;
pub mod champs;
pub mod changements_contrat;
pub mod changements_individu;
pub mod csa;
pub mod demarrage;
pub mod dpae;
pub mod dsn;
pub mod echeance_dsn;
pub mod etats_organismes;
pub mod fctu;
pub mod formats;
pub mod naf;
pub mod norme;
pub mod ntt;
pub mod oeth;
pub mod ordres_paiement;
pub mod produire_regularisations;
pub mod recap_dsn;
pub mod signalement;
pub mod usages;
pub mod versement_declare;
