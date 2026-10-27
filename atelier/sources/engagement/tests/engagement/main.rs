//! Les tests de l'engagement : les preuves de ce qu'il garantit (rien ne part sans validation, ce qui part est la paie
//! validee, le recalcul de lui-meme et sa raison, ce qui est parti ne bouge plus, la question avant « Modifier… »). Ils
//! appellent la crate directement. Un fichier seul : `cargo nextest run --cargo-profile essais -p engagement -E
//! 'test(/^etats::/)'`.

#[path = "../outils/mod.rs"]
mod outils;

mod etats;
