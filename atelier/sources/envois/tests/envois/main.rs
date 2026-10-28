//! Les tests des envois sans compagnie : ce que le transport envoie a net-entreprises et ce qu'il garde pour lui, la
//! bascule d'un depot en test et la preuve d'un depot reel, la lecture des retours reels transcrits, la boite simulee,
//! un envoi coupe en route. Rien ne sort de la machine (un faux serveur local). Un fichier seul :
//! `cargo nextest run --cargo-profile essais -p envois -E 'test(/^<fichier>::/)'`.

#[path = "../outils/mod.rs"]
mod outils;

mod courriel;
mod courriel_lecture;
mod depot;
mod envoi;
mod retours;
mod transport;
