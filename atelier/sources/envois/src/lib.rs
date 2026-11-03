//! CE QUI PART ET CE QUI REVIENT : le journal de ce qui est parti (`journal`, `ligne_d_envoi`), ce qu'il dit du mois
//! (`etats`), les retours des organismes et leur lecture (`retours`), la reprise au journal des depots que les retours
//! attestent (`reprise_journal`), le Drive et l'autorisation Google (`drive`, `google_autorisation`, `google`).
//!
//! Les envois recoivent des pieces deja fabriquees (par `documents` et `declarations`) et un mot de passe : ils ne
//! fabriquent aucun document, n'ouvrent jamais le coffre et ne decident pas de ce qui part. Ce qu'un envoi releve en
//! chemin (un incident, ce qu'il faut marquer) se rend a l'appelant, qui le note. L'engagement ne lit pas le journal :
//! `moteur` lui passe ce qui est parti (`journal::departs`).

pub mod aer_fictive;
pub mod branchement;
pub mod depot;
pub mod depot_acte;
pub mod etats;
pub mod google;
pub mod google_autorisation;
pub mod journal;
pub mod ligne_d_envoi;
pub mod ordre_signalements;
pub mod organismes_simules;
pub mod redirection;
pub mod releve_retours;
pub mod reprise_journal;
pub mod retours;
pub mod suites;
pub mod tables_journal;
pub mod tables_retours;
pub mod transport;
pub mod courriel;
pub mod courriel_lecture;
pub mod drive;
pub mod envoi;
pub mod releve_signatures;
pub mod signature_courriel;
pub mod simulation_courriels;
