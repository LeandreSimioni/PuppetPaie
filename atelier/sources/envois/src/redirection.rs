//! LA REDIRECTION DE L'ATELIER : dans la construction `--features atelier` (le binaire `paie-atelier`, jamais livre),
//! trois variables d'environnement pointent le client de la DSN (net-entreprises), celui de la DPAE (Urssaf) et celui
//! de Google (jetons et Drive) vers un faux serveur LOCAL. Une adresse qui n'est pas locale (`http://127.0.0.1:`,
//! `http://localhost:` ou `http://[::1]:`) est refusee : jamais une redirection vers l'exterieur. La construction
//! ordinaire ne lit jamais ces variables : les adresses des organismes y sont en dur.

/// Le faux net-entreprises : `BASE/authentifier/`, `BASE/lister-retours-declarant/1.0/`, `BASE/lister-retours-flux/1.0/`, `BASE/deposer-dsn/1.0/`.
pub const VARIABLE_DSN: &str = "PUPPETPAIE_ATELIER_DSN";
/// Le faux Urssaf : `BASE/authentifier_dpae`, et les mêmes services que la DSN.
pub const VARIABLE_DPAE: &str = "PUPPETPAIE_ATELIER_DPAE";
/// Le faux Google : `BASE/token`, `BASE/drive/v3/files`, `BASE/upload/drive/v3/files`.
pub const VARIABLE_GOOGLE: &str = "PUPPETPAIE_ATELIER_GOOGLE";

/// Ce que dit une redirection refusee : l'adresse qui n'est pas locale n'est jamais appelee.
pub const ADRESSE_REFUSEE: &str = "http://127.0.0.1:9/adresse-non-locale-refusee-par-l-atelier/";

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
