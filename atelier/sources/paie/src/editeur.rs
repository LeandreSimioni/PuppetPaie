//! Le canal vers l'editeur (`moteur/editeur.py`, et ce que `filet.Client` en
//! utilisait) : le POST JSON vers le serveur de l'editeur, pour les incidents
//! (`rapport`), « Un probleme, une idee ? » (`retour`, `reponses`) et le filet
//! DPAE (`inscrire`, `etat`, `armer`, `desarmer`, `desinscrire`).
//!
//! Le paquet de DEMONSTRATION (`EDITION.json`, `{"edition": "demo"}`) ne garde
//! que les actions de l'editeur (et `poste`) : les autres levent `EditionDemo`,
//! comme le faisait `editeur.poster` (et la soudure de `filet.Client._post`).
//!
//! UNE CLE PAR POSTE, PLUS DE SECRET PARTAGE (serveur version 7, 13/10/2026 ;
//! le contrat : `prive/journal/filet/DEMANDE-SERVEUR-CLE-PAR-POSTE.md`). Le
//! serveur donne a chaque poste son numero et sa cle (`POST /poste`) ; les
//! actions de l'editeur (`inscrire`, `rapport`, `retour`, `reponses`) portent
//! le numero dans le corps et la cle dans `X-Filet-Cle`. `X-Filet-Secret` et le
//! `secret` de `config/filet-dpae.json` n'existent plus.
//!
//! Jamais le vrai serveur dans les essais : l'adresse vient de la
//! configuration, et les essais en donnent une locale. Quand la configuration
//! ne dit rien, c'est `SERVEUR_PAR_DEFAUT` (lot S1) : un essai qui veut le voir
//! passe par un faux mandataire local (`tests/outils`, `mandataire`), qui refuse.

use std::path::Path;

use serde_json::Value;

use crate::fichiers::dossier_du_code;
use crate::retours::Echec;
use crate::transport::{envoyer, Panne, Requete};

/// `editeur.ACTIONS_DEMO` : ce que le paquet de demonstration peut poster.
pub const ACTIONS_DEMO: [&str; 4] = ["poste", "rapport", "retour", "reponses"];

/// Les actions qui exigent l'identite du poste (`poste` dans le corps, `X-Filet-Cle`).
pub const ACTIONS_DU_POSTE: [&str; 4] = ["inscrire", "rapport", "retour", "reponses"];

/// Ce que dit le moteur a un 401 du serveur sur une action du poste : l'identite se redemande.
pub const REDEMANDER_IDENTITE: &str = "Le serveur ne reconnait plus ce poste : retirer \"cle\" de config/diagnostic.json pour lui redemander une identite.";

/// Le serveur de l'editeur QUAND LA CONFIGURATION NE DIT RIEN (lot S1, 13/10/2026) : le filet DPAE (ses rappels), les
/// incidents, « Un probleme, une idee ? » et leurs reponses. Il repond sur `filet.puppetpaie.fr` (enregistrement pose
/// par Leandre le 13/10/2026 ; le serveur vit dans le depot du site de l'editeur, `infra/filet/`). Ce qu'une compagnie a
/// pose le remplace toujours : `serveur` de `config/filet-dpae.json`, celui de son compte du filet, ou `EDITEUR.json` du
/// paquet. Jamais dans les essais : un test pose son serveur local, ou passe par un faux mandataire.
pub const SERVEUR_PAR_DEFAUT: &str = "https://filet.puppetpaie.fr";

/// `filet.DELAI` : le filet ne doit jamais faire attendre une paie.
pub const DELAI: u64 = 15;

/// `editeur.ErreurFilet`, ou une autre exception que le Python laissait passer
/// (`classe` : son nom) -- celle-ci ne se rattrape pas comme un `ErreurFilet`.
#[derive(Debug)]
pub struct ErreurFilet {
    pub message: String,
    pub statut: u16,
    pub classe: Option<&'static str>,
}

impl ErreurFilet {
    pub(crate) fn filet(message: String, statut: u16) -> ErreurFilet {
        ErreurFilet { message, statut, classe: None }
    }

    pub(crate) fn autre(classe: &'static str, message: String) -> ErreurFilet {
        ErreurFilet { message, statut: 0, classe: Some(classe) }
    }

    /// `ErreurFilet.reseau`.
    pub fn reseau(&self) -> bool {
        self.statut == 0 || self.statut >= 500
    }

    /// Une vraie `ErreurFilet`, que les appelants rattrapent ; sinon une autre
    /// exception, qui remonte.
    pub fn est_filet(&self) -> bool {
        self.classe.is_none()
    }

    /// L'exception qui remonte de la route.
    pub fn en_echec(self) -> Echec {
        Echec { classe: self.classe.or(Some("ErreurFilet")), message: self.message }
    }
}

/// `compagnies.paquet_soude` : `EDITION.json`, a cote du code, dit `demo`.
pub fn paquet_soude() -> bool {
    let logiciel = dossier_du_code().parent().map(Path::to_path_buf).unwrap_or_default();
    std::fs::read(logiciel.join("EDITION.json"))
        .ok()
        .and_then(|o| String::from_utf8(o).ok())
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .is_some_and(|d| d.get("edition").and_then(Value::as_str) == Some("demo"))
}

/// `compagnies.est_demonstration` : la compagnie porte le marqueur d'une copie
/// dont l'origine est une compagnie de DEMONSTRATION (ses envois sont joues).
pub fn est_demonstration(racine: &Path) -> bool {
    std::fs::read(racine.join("COPIE-DE-DEVELOPPEMENT.json"))
        .ok()
        .and_then(|o| String::from_utf8(o).ok())
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|i| i.get("copie_de").filter(|v| crate::fichiers::vrai(v)).map(crate::etats::py_str))
        .is_some_and(|c| c.starts_with("compagnie de demonstration"))
}

/// Le message d'`EditionDemo(quoi)`.
fn edition_demo(quoi: &str) -> String {
    format!(
        "Version demo : cet envoi n'existe pas dans ce paquet ({}). Dans la \
         version complete, c'est ici que le fichier part vers l'organisme ou \
         que le courriel s'envoie ; ici, rien ne quitte le poste.",
        quoi
    )
}

/// `http.client.responses[code]` (le texte de `str(HTTPError)`).
fn phrase(code: u16) -> &'static str {
    match code {
        100 => "Continue",
        101 => "Switching Protocols",
        102 => "Processing",
        103 => "Early Hints",
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        203 => "Non-Authoritative Information",
        204 => "No Content",
        205 => "Reset Content",
        206 => "Partial Content",
        207 => "Multi-Status",
        208 => "Already Reported",
        226 => "IM Used",
        300 => "Multiple Choices",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        304 => "Not Modified",
        305 => "Use Proxy",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        402 => "Payment Required",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        406 => "Not Acceptable",
        407 => "Proxy Authentication Required",
        408 => "Request Timeout",
        409 => "Conflict",
        410 => "Gone",
        411 => "Length Required",
        412 => "Precondition Failed",
        413 => "Request Entity Too Large",
        414 => "Request-URI Too Long",
        415 => "Unsupported Media Type",
        416 => "Requested Range Not Satisfiable",
        417 => "Expectation Failed",
        418 => "I'm a Teapot",
        421 => "Misdirected Request",
        422 => "Unprocessable Entity",
        423 => "Locked",
        424 => "Failed Dependency",
        425 => "Too Early",
        426 => "Upgrade Required",
        428 => "Precondition Required",
        429 => "Too Many Requests",
        431 => "Request Header Fields Too Large",
        451 => "Unavailable For Legal Reasons",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        505 => "HTTP Version Not Supported",
        506 => "Variant Also Negotiates",
        507 => "Insufficient Storage",
        508 => "Loop Detected",
        510 => "Not Extended",
        511 => "Network Authentication Required",
        _ => "",
    }
}

/// `json.dumps(v)` : ASCII seulement (`\uXXXX`, paires de substitution au-dela
/// du plan de base), separateurs `, ` et `: `.
pub fn json_ascii(v: &Value) -> String {
    let mut s = String::new();
    ecrire(v, &mut s);
    s
}

fn ecrire(v: &Value, s: &mut String) {
    match v {
        Value::Null => s.push_str("null"),
        Value::Bool(b) => s.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => match (n.as_i64(), n.as_u64()) {
            (Some(i), _) => s.push_str(&i.to_string()),
            (_, Some(u)) => s.push_str(&u.to_string()),
            _ => s.push_str(&crate::ecriture::repr_flottant(n.as_f64().unwrap_or(0.0))),
        },
        Value::String(t) => chaine(t, s),
        Value::Array(a) => {
            s.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    s.push_str(", ");
                }
                ecrire(x, s);
            }
            s.push(']');
        }
        Value::Object(o) => {
            s.push('{');
            for (i, (k, x)) in o.iter().enumerate() {
                if i > 0 {
                    s.push_str(", ");
                }
                chaine(k, s);
                s.push_str(": ");
                ecrire(x, s);
            }
            s.push('}');
        }
    }
}

fn chaine(t: &str, s: &mut String) {
    s.push('"');
    for c in t.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            '\r' => s.push_str("\\r"),
            '\t' => s.push_str("\\t"),
            '\u{8}' => s.push_str("\\b"),
            '\u{c}' => s.push_str("\\f"),
            c if (c as u32) < 0x20 || (c as u32) > 0x7e => {
                let mut u = [0u16; 2];
                for unite in c.encode_utf16(&mut u) {
                    s.push_str(&format!("\\u{:04x}", unite));
                }
            }
            c => s.push(c),
        }
    }
    s.push('"');
}

/// `editeur.poster(serveur, action, corps, entetes, delai)` : la reponse JSON du
/// serveur, ou l'`ErreurFilet` qu'elle a causee.
pub fn poster(serveur: &str, action: &str, corps: &Value, entetes: &[(&str, &str)], delai: u64) -> Result<Value, ErreurFilet> {
    if paquet_soude() && !ACTIONS_DEMO.contains(&action) {
        return Err(ErreurFilet::autre("EditionDemo", edition_demo(action)));
    }
    let donnees = json_ascii(corps);
    let url = format!("{}/{}", serveur.trim_end_matches('/'), action);
    // Les en-tetes d'urllib : il ne demande aucun codage.
    let mut en_tetes: Vec<(&str, &str)> = vec![("User-Agent", "PuppetPaie-filet"), ("Accept-Encoding", "identity")];
    en_tetes.extend_from_slice(entetes);
    let r = Requete {
        url: &url,
        donnees: Some(donnees.as_bytes()),
        content_type: Some("application/json"),
        post: true,
        delai,
        entetes: &en_tetes,
        sans_gzip: true,
        ..Default::default()
    };
    match envoyer(&r) {
        Ok((brut, _)) => {
            let octets = if brut.is_empty() { b"{}".to_vec() } else { brut };
            // `json.loads(bytes)` : le texte se lit en UTF-8 ; un echec est une
            // `ValueError`, que le Python rangeait avec les pannes de reseau.
            let texte = match std::str::from_utf8(&octets) {
                Ok(t) => t.trim_start_matches('\u{feff}'),
                Err(e) => return Err(ErreurFilet::filet(format!("Filet DPAE injoignable : {}", e), 0)),
            };
            match serde_json::from_str::<Value>(texte) {
                Ok(v) => Ok(v),
                Err(e) => Err(ErreurFilet::filet(format!("Filet DPAE injoignable : reponse illisible ({})", e), 0)),
            }
        }
        Err(Panne::Statut { code, corps }) => {
            let propre = format!("HTTP Error {}: {}", code, phrase(code));
            let lu = corps.ok().filter(|c| !c.is_empty()).unwrap_or_else(|| b"{}".to_vec());
            let msg = match std::str::from_utf8(&lu).ok().and_then(|t| serde_json::from_str::<Value>(t.trim_start_matches('\u{feff}')).ok()) {
                Some(Value::Object(o)) => match o.get("erreur") {
                    Some(e) if crate::fichiers::vrai(e) => crate::etats::py_str(e),
                    _ => propre,
                },
                Some(autre) => {
                    return Err(ErreurFilet::autre(
                        "AttributeError",
                        crate::fichiers::attendu("un objet", crate::etats::nom_type(&autre)),
                    ))
                }
                None => propre,
            };
            Err(ErreurFilet::filet(format!("Filet DPAE : {}", msg), code))
        }
        Err(Panne::Reseau(raison)) => Err(ErreurFilet::filet(format!("Filet DPAE injoignable : {}", raison), 0)),
        Err(Panne::Brute { classe, message, .. }) => match classe {
            // Ce qu'urllib laissait sortir sous `OSError`.
            "TimeoutError" | "ConnectionResetError" | "RemoteDisconnected" | "BadGzipFile" => {
                Err(ErreurFilet::filet(format!("Filet DPAE injoignable : {}", message), 0))
            }
            autre => Err(ErreurFilet::autre(autre, message)),
        },
    }
}

/// `filet.Client` : le serveur, la cle du compte (les actions du filet), et
/// l'identite du poste -- `(numero, cle)` donnee par le serveur -- pour les
/// actions de l'editeur.
pub struct Client {
    pub serveur: String,
    pub cle: String,
    pub poste: Option<(String, String)>,
}

type Rep = Result<Value, ErreurFilet>;

impl Client {
    /// Le client d'un compte du filet (`etat`, `armer`, `desarmer`, `desinscrire`).
    pub fn nouveau(serveur: &str, cle: &str) -> Client {
        Client { serveur: serveur.trim_end_matches('/').to_string(), cle: cle.to_string(), poste: None }
    }

    /// Le client des actions de l'editeur, avec l'identite de ce poste.
    pub fn du_poste(serveur: &str, numero: &str, cle_du_poste: &str) -> Client {
        Client { serveur: serveur.trim_end_matches('/').to_string(), cle: String::new(), poste: Some((numero.to_string(), cle_du_poste.to_string())) }
    }

    /// `POST /poste` : l'identite que le serveur donne a ce poste, `{"poste", "cle"}`,
    /// rendue une seule fois (il n'en garde que l'empreinte).
    pub fn demander_poste(serveur: &str) -> Rep {
        poster(serveur, "poste", &serde_json::json!({}), &[], DELAI)
    }

    fn post(&self, action: &str, corps: Value, entetes: &[(&str, &str)], delai: u64) -> Rep {
        poster(&self.serveur, action, &corps, entetes, delai)
    }

    /// Une action du poste : le numero dans le corps, la cle dans l'en-tete. Un 401
    /// du serveur (poste inconnu, revoque, ou cle fausse : il ne dit pas lequel) se
    /// complete de ce qu'il y a a faire.
    fn post_du_poste(&self, action: &str, mut corps: Value, delai: u64) -> Rep {
        let Some((numero, cle)) = &self.poste else {
            return Err(ErreurFilet::autre("ValueError", format!("{} : ce client n'a pas l'identite du poste.", action)));
        };
        if let Value::Object(o) = &mut corps {
            o.insert("poste".into(), Value::String(numero.clone()));
        }
        match self.post(action, corps, &[("X-Filet-Cle", cle.as_str())], delai) {
            Err(e) if e.est_filet() && e.statut == 401 => Err(ErreurFilet::filet(format!("{} {}", e.message, REDEMANDER_IDENTITE), 401)),
            r => r,
        }
    }

    pub fn inscrire(&self, adresse: &str) -> Rep {
        self.post_du_poste("inscrire", serde_json::json!({"adresse": adresse}), DELAI)
    }

    pub fn etat(&self) -> Rep {
        self.post("etat", serde_json::json!({"cle": self.cle}), &[], DELAI)
    }

    /// `embauche` : le jour que le courriel nomme ; `objet` ne part que s'il
    /// n'est pas la DPAE, pour qu'un serveur d'avant la DSN comprenne encore.
    pub fn armer(&self, code: &str, echeance: &str, embauche: &str, objet: &str) -> Rep {
        let mut corps = serde_json::json!({"cle": self.cle, "code": code, "echeance": echeance, "embauche": embauche});
        if objet != "dpae" {
            corps["objet"] = Value::String(objet.to_string());
        }
        self.post("armer", corps, &[], DELAI)
    }

    pub fn desarmer(&self, code: &str) -> Rep {
        self.post("desarmer", serde_json::json!({"cle": self.cle, "code": code}), &[], DELAI)
    }

    pub fn desinscrire(&self) -> Rep {
        self.post("desinscrire", serde_json::json!({"cle": self.cle}), &[], DELAI)
    }

    pub fn rapporter(&self, incidents: &[Value]) -> Rep {
        self.post_du_poste("rapport", serde_json::json!({"incidents": incidents}), DELAI)
    }

    /// Un texte, des incidents et des captures : le delai est plus long.
    pub fn signaler(&self, corps: &Value) -> Rep {
        self.post_du_poste("retour", corps.clone(), 60)
    }

    pub fn reponses(&self, numeros: &[Value]) -> Rep {
        self.post_du_poste("reponses", serde_json::json!({"numeros": numeros}), DELAI)
    }
}
