//! CE QUI PART ET CE QUI REVIENT : le journal de ce qui est parti (`journal`, `ligne_d_envoi`), ce qu'il dit du mois
//! (`etats`), les retours des organismes et leur lecture (`retours`), la reprise au journal des depots que les retours
//! attestent (`reprise_journal`).
//!
//! Les envois recoivent des pieces deja fabriquees (par `documents` et `declarations`) et un mot de passe : ils ne
//! fabriquent aucun document, n'ouvrent jamais le coffre et ne decident pas de ce qui part. Ce qu'un envoi releve en
//! chemin (un incident, ce qu'il faut marquer) se rend a l'appelant, qui le note. L'engagement ne lit pas le journal :
//! `paie` lui passe ce qui est parti (`journal::departs`).

pub mod aer_fictive;
pub mod branchement;
pub mod depot;
pub mod etats;
pub mod journal;
pub mod ligne_d_envoi;
pub mod ordre_signalements;
pub mod redirection;
pub mod releve_retours;
pub mod reprise_journal;
pub mod retours;
pub mod suites;
pub mod tables_journal;
pub mod tables_retours;
pub mod transport;
