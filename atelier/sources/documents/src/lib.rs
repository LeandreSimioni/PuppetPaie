//! Les documents : ce qu'un engagement validé fait imprimer, signer ou envoyer. Chacun se fabrique au moment où il part,
//! depuis les paies écrites de l'engagement validé (`engagement::etats`) ; aucun ne recalcule une paie.

pub mod aer;
pub mod avenant;
pub mod bordereau;
pub mod bulletin;
pub mod conges_spectacles;
pub mod contrat;
pub mod ecrire_documents;
pub mod etats_paie;
pub mod etats_personnel;
pub mod fabrique;
pub mod fiche_questionnaire;
pub mod fiche_recapitulative;
pub mod prelevement;
pub mod tableaux_export;
