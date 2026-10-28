//! L'ATELIER : ce que `paie-atelier` a de plus que `paie` (lot J2a, preuve, partie 1).
//!
//! Ce module n'existe que dans la construction `--features atelier` (le binaire `paie-atelier`, pour les essais, jamais
//! livre). La construction ordinaire (`cargo build --release`, le workflow « Moteur Rust ») ne le compile pas : `paie`
//! garde les adresses des organismes en dur et rien ne permet de les changer.
//!
//! Deux choses, et seulement elles :
//! - la REDIRECTION : trois variables d'environnement pointent le client de la DSN (net-entreprises), celui de la DPAE
//!   (Urssaf) et celui de Google (jetons et Drive) vers un faux serveur LOCAL. Une adresse qui n'est pas locale
//!   (`http://127.0.0.1:`, `http://localhost:` ou `http://[::1]:`) est refusee : jamais une redirection vers l'exterieur.
//! - le REFUS de tout acte reel : un depot reel, un envoi, une signature passent tous par `depot_acte::verifier_preuve` ;
//!   dans l'atelier elle refuse, mot de passe ou non (`refus_acte_reel`).
//!
//! Les adresses de la boite courriel (SMTP et IMAP, en TLS) et du serveur des incidents ne sont pas redirigees ici : celle
//! du serveur des incidents vient deja de la configuration de la compagnie.
//!
//! Et un outil (#236) : `paie-atelier fabriquer-bulletins`, les bulletins fictifs d'un logiciel precedent
//! (`fabriquer_bulletins`), dessines par la fabrique que les tests partagent (`bulletins_fictifs`).

pub mod bulletins_fictifs;
pub mod fabriquer_bulletins;
pub use envois::redirection::{base, est_locale, ADRESSE_REFUSEE, VARIABLE_DPAE, VARIABLE_DSN, VARIABLE_GOOGLE};

/// La phrase du refus d'un acte reel.
pub const REFUS_ACTE_REEL: &str =
    "Ceci est le binaire de l'atelier (paie-atelier) : il refuse tout acte reel (depot reel, envoi, signature), mot de passe ou non. Rien n'est parti.";

/// Au demarrage de `paie-atelier` : une variable posee sur autre chose qu'un serveur local arrete tout.
pub fn verifier_environnement() -> Result<(), String> {
    for variable in [VARIABLE_DSN, VARIABLE_DPAE, VARIABLE_GOOGLE] {
        if let Ok(v) = std::env::var(variable) {
            let v = v.trim();
            if !v.is_empty() && !est_locale(v) {
                return Err(format!("{} : l'atelier ne se redirige que vers un serveur local (http://127.0.0.1:PORT/...). Rien n'est parti.", variable));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seules_les_adresses_locales_passent() {
        assert!(est_locale("http://127.0.0.1:8080/dsn"));
        assert!(est_locale("http://localhost:1/"));
        assert!(!est_locale("https://services.net-entreprises.fr/"));
        assert!(!est_locale("http://127.0.0.1.exemple.fr:80/"));
        assert!(!est_locale("http://127.0.0.1/"));
    }
}
