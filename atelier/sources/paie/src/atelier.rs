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

/// Le faux net-entreprises : `BASE/authentifier/`, `BASE/lister-retours-declarant/1.0/`, `BASE/lister-retours-flux/1.0/`, `BASE/deposer-dsn/1.0/`.
pub const VARIABLE_DSN: &str = "PUPPETPAIE_ATELIER_DSN";
/// Le faux Urssaf : `BASE/authentifier_dpae`, et les mêmes services que la DSN.
pub const VARIABLE_DPAE: &str = "PUPPETPAIE_ATELIER_DPAE";
/// Le faux Google : `BASE/token`, `BASE/drive/v3/files`, `BASE/upload/drive/v3/files`.
pub const VARIABLE_GOOGLE: &str = "PUPPETPAIE_ATELIER_GOOGLE";

/// Ce que dit une redirection refusee : l'adresse qui n'est pas locale n'est jamais appelee.
pub const ADRESSE_REFUSEE: &str = "http://127.0.0.1:9/adresse-non-locale-refusee-par-l-atelier/";

/// La phrase du refus d'un acte reel.
pub const REFUS_ACTE_REEL: &str =
    "Ceci est le binaire de l'atelier (paie-atelier) : il refuse tout acte reel (depot reel, envoi, signature), mot de passe ou non. Rien n'est parti.";

/// Une adresse locale : le seul endroit ou l'atelier accepte d'aller.
pub fn est_locale(adresse: &str) -> bool {
    ["http://127.0.0.1:", "http://localhost:", "http://[::1]:"].iter().any(|p| adresse.starts_with(p))
}

/// La base d'une variable de redirection, sans barre finale ; `None` si elle n'est pas posee (ou vide). Une valeur qui
/// n'est pas locale rend `ADRESSE_REFUSEE`, qui n'aboutit nulle part.
pub fn base(variable: &str) -> Option<String> {
    let v = std::env::var(variable).ok()?;
    let v = v.trim().trim_end_matches('/').to_string();
    if v.is_empty() {
        return None;
    }
    Some(if est_locale(&v) { v } else { ADRESSE_REFUSEE.trim_end_matches('/').to_string() })
}

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
