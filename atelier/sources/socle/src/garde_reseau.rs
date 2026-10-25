//! Le garde-fou reseau des essais : AUCUN ESSAI NE SORT DE LA MACHINE.
//!
//! Pourquoi : tout ce qui retombe sur une adresse reelle (le serveur de l'editeur, net-entreprises, l'Urssaf, Google,
//! la DILA, le depot des versions, une boite courriel) partirait des qu'un essai l'oublie, sur tout poste qui a
//! internet : un essai sans serveur pose joindrait le vrai serveur de l'editeur, qui inscrirait une compagnie et
//! enverrait un courriel. Reparer chaque essai ne suffit pas.
//!
//! La regle, en un seul endroit : quand la variable `PUPPETPAIE_ESSAIS_HORS_RESEAU` est posee -- Cargo la pose pour tout
//! processus qu'il lance, donc pour chaque essai et chaque `paie` qu'un essai lance (`atelier/sources/.cargo/config.toml`,
//! `[env]`, `force`) --, une sortie vers une autre machine que celle-ci est REFUSEE avant toute connexion. La seule
//! porte : un mandataire LOCAL que l'essai nomme dans cette variable meme (`outils::mandataire`, un faux qui note a qui
//! le moteur voulait parler et refuse) ; il remplace alors le mandataire de l'environnement (celui d'un conteneur, qui
//! laisse passer, n'est jamais pris). Vers la machine elle-meme (les faux serveurs des essais), rien ne change.
//!
//! Hors essais (l'executable livre, lance hors de Cargo), la variable n'existe pas : rien ne change.
//!
//! Chaque sortie du moteur passe par ici : `transport::envoyer` (HTTP : editeur, net-entreprises, Urssaf, Google),
//! `sources::telecharger`, `entrepot_dila`, `maj` (le depot des versions), `courriel` (IMAP, SMTP),
//! `interface::api::ouvrir_adresse` (le navigateur). L'essai `garde_reseau` (`tests/fichiers/`) le verifie dans les
//! sources : une nouvelle sortie qui n'y passerait pas le fait echouer.

use std::net::IpAddr;

/// La variable du garde-fou. Posee (n'importe quelle valeur) : aucune sortie hors de la machine. Sa valeur peut nommer
/// un mandataire local (`http://127.0.0.1:<port>`), seule porte alors permise.
pub const VARIABLE: &str = "PUPPETPAIE_ESSAIS_HORS_RESEAU";

/// Ce que le garde-fou dit d'une sortie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sortie {
    /// Hors essais : rien ne change (le mandataire de l'environnement fait foi).
    Libre,
    /// En essai, vers la machine elle-meme : directe, sans mandataire.
    Locale,
    /// En essai, vers dehors : par le seul mandataire local que l'essai a nomme.
    ParLaPorte(String),
}

/// Le nom d'hote d'une adresse (`https://hote:port/chemin` -> `hote` ; `[::1]` -> `::1`).
pub fn hote_de(url: &str) -> String {
    let reste = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let autorite = reste.split(['/', '?', '#']).next().unwrap_or("");
    let hote = autorite.rsplit_once('@').map(|(_, h)| h).unwrap_or(autorite);
    let nom = if let Some(h) = hote.strip_prefix('[') { h.split(']').next().unwrap_or(h) } else { hote.split(':').next().unwrap_or(hote) };
    nom.to_lowercase()
}

/// Cette machine : `localhost` (et ses sous-noms), 127.0.0.0/8, `::1`.
pub fn est_locale(hote: &str) -> bool {
    let h = hote.trim_start_matches('[').trim_end_matches(']').to_lowercase();
    h == "localhost" || h.ends_with(".localhost") || h.parse::<IpAddr>().map(|ip| ip.is_loopback()).unwrap_or(false)
}

/// En essai ? (la variable est posee, meme vide).
pub fn en_essai() -> bool {
    std::env::var_os(VARIABLE).is_some()
}

fn refus(hote: &str) -> String {
    format!(
        "Garde-fou des essais ({}) : aucune sortie vers {} ; un essai pose un serveur local, ou nomme un faux mandataire local dans cette variable.",
        VARIABLE, hote
    )
}

/// La decision pour une sortie vers `hote`.
pub fn sortie_vers(hote: &str) -> Result<Sortie, String> {
    // LE FIL DE FOND de la fenetre (`fil_de_fond`) : il tourne avant tout mot de passe, et ne sort jamais.
    if crate::fil_de_fond::ici() {
        return Err(crate::fil_de_fond::refus(&format!("une sortie vers {}", hote)));
    }
    let Some(valeur) = std::env::var_os(VARIABLE) else { return Ok(Sortie::Libre) };
    if est_locale(hote) {
        return Ok(Sortie::Locale);
    }
    let porte = valeur.to_string_lossy().trim().to_string();
    if porte.starts_with("http://") && est_locale(&hote_de(&porte)) {
        return Ok(Sortie::ParLaPorte(porte));
    }
    Err(refus(hote))
}

/// La decision pour une adresse (`https://...`).
pub fn sortie(url: &str) -> Result<Sortie, String> {
    sortie_vers(&hote_de(url))
}

/// Une connexion directe, sans mandataire possible (IMAP, SMTP, le navigateur) : en essai, vers cette machine seulement.
pub fn directe(hote: &str) -> Result<(), String> {
    match sortie_vers(hote)? {
        Sortie::Libre | Sortie::Locale => Ok(()),
        Sortie::ParLaPorte(_) => Err(refus(hote)),
    }
}

/// Un agent `ureq` pour `url`, regle par le garde-fou : hors essais, tel quel ; en essai, sans mandataire vers cette
/// machine, par la porte seule vers dehors, refuse sinon.
pub fn regler(c: ureq::config::ConfigBuilder<ureq::typestate::AgentScope>, url: &str) -> Result<ureq::config::ConfigBuilder<ureq::typestate::AgentScope>, String> {
    Ok(match sortie(url)? {
        Sortie::Libre => c,
        Sortie::Locale => c.proxy(None),
        Sortie::ParLaPorte(p) => c.proxy(Some(ureq::Proxy::new(&p).map_err(|e| format!("{} : mandataire illisible ({}) : {}", VARIABLE, p, e))?)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cargo pose la variable pour tout essai (`.cargo/config.toml`, `[env]`) : ce processus-ci est un essai, et rien ne
    /// sort vers le serveur de l'editeur, ni vers aucun autre ; la machine elle-meme reste joignable.
    #[test]
    fn dans_un_essai_rien_ne_sort_de_la_machine() {
        assert!(en_essai(), "Cargo doit poser {} pour les essais (.cargo/config.toml)", VARIABLE);
        for url in ["https://filet.puppetpaie.fr", "https://services.net-entreprises.fr/", "https://oauth2.googleapis.com/token", "https://github.com/LeandreSimioni/PuppetPaie/releases/latest/download"] {
            assert!(sortie(url).is_err(), "{}", url);
        }
        assert!(directe("smtp.gmail.com").is_err());
        assert_eq!(sortie("http://127.0.0.1:8080/x"), Ok(Sortie::Locale));
        assert!(directe("localhost").is_ok());
    }
}
