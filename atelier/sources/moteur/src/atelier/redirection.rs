//! LA REDIRECTION DU BINAIRE DE L'ATELIER (`moteur-atelier`, `--features atelier`, jamais livre). La construction
//! ordinaire ne la compile pas : `moteur` garde les adresses des organismes en dur et rien ne permet de les changer.
//!
//! - Trois variables d'environnement pointent le client de la DSN (net-entreprises), celui de la DPAE (Urssaf) et celui
//!   de Google (jetons et Drive) vers un faux serveur LOCAL. Une adresse qui n'est pas locale (`http://127.0.0.1:`,
//!   `http://localhost:` ou `http://[::1]:`) est refusee : jamais une redirection vers l'exterieur.
//! - Tout acte reel est refuse : un depot reel, un envoi, une signature passent tous par `depot_acte::verifier_preuve` ;
//!   dans l'atelier elle refuse, mot de passe ou non (`REFUS_ACTE_REEL`).
//!
//! Les adresses de la boite courriel (SMTP et IMAP, en TLS) et du serveur des incidents ne sont pas redirigees ici : celle
//! du serveur des incidents vient deja de la configuration de la compagnie.

pub use envois::redirection::{base, est_locale, ADRESSE_REFUSEE, VARIABLE_DPAE, VARIABLE_DSN, VARIABLE_GOOGLE};

/// La phrase du refus d'un acte reel.
pub const REFUS_ACTE_REEL: &str =
    "Ceci est le binaire de l'atelier (moteur-atelier) : il refuse tout acte reel (depot reel, envoi, signature), mot de passe ou non. Rien n'est parti.";

/// Au demarrage de `moteur-atelier` : une variable posee sur autre chose qu'un serveur local arrete tout.
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
