//! Le socle du moteur : ce que toute partie du dessus lit sans rien savoir de la paie. Les dates et l'heure de Paris, le
//! temps (celui de la machine, ou le temps simule de l'atelier), les nombres et leur arrondi, le texte, les fichiers lus
//! et ecrits, l'echec et sa memoire, le garde-fou reseau. Aucun droit, aucun metier.

pub mod base64;
pub mod codification_ue;
pub mod csv;
pub mod d28;
pub mod date;
pub mod echec;
pub mod ecriture;
pub mod feries;
pub mod fichiers;
pub mod fil_de_fond;
pub mod garde_reseau;
pub mod html;
pub mod json_ascii;
pub mod lecture_xlsx;
pub mod nfkc;
pub mod num;
pub mod paris;
pub mod pays;
pub mod nfkd_ascii;
pub mod tables_nfkc;
pub mod temps;
pub mod texte;
pub mod valeur;
pub mod verrou;
