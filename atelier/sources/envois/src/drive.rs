//! Le client Drive (`google.Client`) : les dossiers, le listage, le televersement.
//!
//! Les appels passent par `transport::envoyer`. Le client recoit ce que l'appelant a lu
//! sur le poste (`Autorisation` : les identifiants de l'application, le jeton scelle,
//! l'adresse du compte ; `moteur::routes::coffre::client_drive`) : il n'ouvre jamais le coffre.
//!
//! LE TRAIT `Drive` : la sauvegarde et l'archivage ne connaissent que lui (comme
//! `Courriel` pour la messagerie). Le vrai `Client` l'implemente ; la preuve met a sa
//! place un faux, ou le vrai `Client` pointe sur un serveur local (`Adresses`).
//!
//! Ce module ne parle jamais a Google tout seul : `Client::nouveau` ne fait aucun appel
//! reseau ; le premier est `jeton_acces`, au premier
//! `appel`.
//!
//! `drive.file` ne laisse voir que ce que l'application a cree : la recherche d'un dossier
//! ne retrouvera jamais un dossier fait a la main, meme du meme nom. C'est voulu.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{json, Value};

use socle::fichiers::vrai;
use crate::google::{champ, en_texte, encoder, jeton_par_rafraichissement, phrase_de_panne, JETONS};
use crate::transport::{envoyer, Requete};

/// `google.DRIVE_FICHIERS`.
pub const DRIVE_FICHIERS: &str = "https://www.googleapis.com/drive/v3/files";
/// `google.DRIVE_TELEVERSEMENT`.
pub const DRIVE_TELEVERSEMENT: &str = "https://www.googleapis.com/upload/drive/v3/files";
/// Le type d'un dossier chez Drive.
pub const DOSSIER_DRIVE: &str = "application/vnd.google-apps.folder";

/// `urlopen(..., timeout=120)` des appels Drive.
const DELAI_APPEL: u64 = 120;

/// LES NOUVELLES TENTATIVES (un « HTTP 500 : Internal Error » passager ne doit pas remonter a la personne). Google le dit pour l'API Drive
/// (`atelier/reference/sources/google-drive-api/`, LISEZ-MOI.md) : sur 500, 502, 503, 504 et 429, « use exponential backoff to
/// retry the request » (`google-drive-api-gerer-les-erreurs-20261020.txt`, « 429 errors », « 500, 502, 503, 504 errors ») ;
/// sur un 403 de limite de debit, « retry the upload » (`google-drive-api-televerser-20261020.txt`, « Handle media upload
/// errors ») ; et l'algorithme : attendre 1 s, puis 2 s, puis 4 s, chaque fois plus un hasard d'au plus 1 000 ms, « up to some
/// maximum number of retries » (`google-drive-api-limites-et-quotas-20261020.txt`, « Example algorithm »). Trois nouvelles
/// tentatives au plus : un appel qui echoue encore apres sept secondes d'attente est rendu, et la sauvegarde le reporte au
/// passage suivant (`sauvegarde.rs`).
pub const NOUVELLES_TENTATIVES: u32 = 3;

/// L'attente avant la premiere nouvelle tentative, en millisecondes (1 s, puis le double) ; le hasard ajoute en vaut au plus
/// autant. La preuve la raccourcit (`avec_attente`).
const ATTENTE_DE_BASE_MS: u64 = 1000;

/// Une erreur que Google dit passagere : a retenter apres une attente (voir `NOUVELLES_TENTATIVES`).
pub fn passagere(e: &ErreurGoogle) -> bool {
    match e.statut {
        Some(500 | 502 | 503 | 504 | 429) => true,
        Some(403) => {
            let m = e.message.to_lowercase();
            m.contains("rate limit") || m.contains("ratelimitexceeded")
        }
        _ => false,
    }
}

/// Une erreur de l'API ou de la configuration, avec le nom de sa classe
/// (`ErreurGoogle` pour ce qui est deja traduit en francais ; `CopieDeDeveloppement`,
/// `FileNotFoundError`, `SortieAbsente`... pour le reste). La phrase est ecrite pour etre lue.
#[derive(Debug, Clone, PartialEq)]
pub struct ErreurGoogle {
    pub classe: &'static str,
    pub message: String,
    /// Le statut HTTP d'une reponse d'erreur de Google (rien d'autre n'en porte) : de quoi decider d'une nouvelle tentative.
    pub statut: Option<u16>,
}

impl ErreurGoogle {
    pub fn nouvelle(classe: &'static str, message: impl Into<String>) -> ErreurGoogle {
        ErreurGoogle { classe, message: message.into(), statut: None }
    }

    /// `google.ErreurGoogle`.
    pub fn google(message: impl Into<String>) -> ErreurGoogle {
        ErreurGoogle::nouvelle("ErreurGoogle", message)
    }

    /// Un fichier local qui manque, ou ne se lit pas (`FileNotFoundError`, `OSError`).
    pub fn fichier(e: &std::io::Error, chemin: &Path) -> ErreurGoogle {
        let classe = match e.kind() {
            std::io::ErrorKind::NotFound => "FileNotFoundError",
            std::io::ErrorKind::PermissionDenied => "PermissionError",
            _ => "OSError",
        };
        ErreurGoogle::nouvelle(classe, socle::fichiers::panne_fichier(e, chemin))
    }
}

impl std::fmt::Display for ErreurGoogle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ErreurGoogle {}

impl From<socle::echec::Echec> for ErreurGoogle {
    fn from(e: socle::echec::Echec) -> Self {
        ErreurGoogle::nouvelle(e.classe.unwrap_or("Exception"), e.message)
    }
}

/// Un fichier deja depose dans un dossier Drive (`Client.lister`).
#[derive(Debug, Clone, PartialEq)]
pub struct Distant {
    pub id: String,
    /// `md5Checksum`, calcule par Google ; absent pour un fichier Google Docs.
    pub md5: Option<String>,
    pub octets: u64,
}

/// Ce que Drive rend d'un depot (`{id, name, webViewLink}`), plus `remplace`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Depose {
    pub id: Option<String>,
    pub nom: Option<String>,
    pub lien: Option<String>,
    /// `True` quand le contenu d'un fichier de meme nom a ete remplace au lieu d'un second depot.
    pub remplace: bool,
}

/// Ce que la sauvegarde et l'archivage demandent a Drive.
pub trait Drive {
    /// `Client.compte` : l'adresse du compte autorise, lue sans reseau.
    fn compte(&self) -> Option<String>;
    /// `Client.chemin` : cree au besoin toute une suite de dossiers et rend le dernier.
    fn chemin(&mut self, noms: &[&str]) -> Result<String, ErreurGoogle>;
    /// `Client.lister` : les FICHIERS deja deposes dans un dossier, par nom ; les sous-dossiers
    /// sont ecartes.
    fn lister(&mut self, parent: &str) -> Result<BTreeMap<String, Distant>, ErreurGoogle>;
    /// `Client.televerser` : depose un fichier. `connu` : le fichier de meme nom que `lister` a trouve dans ce dossier ;
    /// s'il y en a un, son contenu est REMPLACE (`Depose::remplace`, meme identifiant, meme lien), sinon le fichier est
    /// CREE. Le client ne cherche pas lui-meme (une recherche par nom seul confondrait un dossier et un fichier, et
    /// ne tiendrait pas compte de ce que `lister` vient de dire) : la decision est celle de `lister`, la meme que
    /// celle de la comparaison des empreintes.
    /// Un fichier disparu entre-temps est une `FileNotFoundError`.
    fn televerser(&mut self, fichier: &Path, parent: &str, nom: Option<&str>, connu: Option<&Distant>) -> Result<Depose, ErreurGoogle>;
    /// Les SOUS-DOSSIERS d'un dossier, `(nom, identifiant)`, dans l'ordre ou Drive les rend, homonymes compris (la
    /// restauration les refuse plutot que d'en choisir un). Ne cree rien : c'est ce qui distingue la recherche de la
    /// restauration de `chemin`, qui cree ce qui manque.
    fn sous_dossiers(&mut self, parent: &str) -> Result<Vec<(String, String)>, ErreurGoogle>;
    /// Le contenu d'un fichier, tel que Drive le sert (`alt=media`). La restauration le compare a `md5Checksum` avant
    /// d'ecrire quoi que ce soit.
    fn telecharger(&mut self, id: &str) -> Result<Vec<u8>, ErreurGoogle>;
}

/// Les trois adresses que le client appelle. Les vraies par defaut ; la preuve pointe sur un
/// serveur local (`http://127.0.0.1:port/...`).
#[derive(Debug, Clone)]
pub struct Adresses {
    pub jetons: String,
    pub fichiers: String,
    pub televersement: String,
}

impl Default for Adresses {
    fn default() -> Self {
        Adresses { jetons: JETONS.into(), fichiers: DRIVE_FICHIERS.into(), televersement: DRIVE_TELEVERSEMENT.into() }
    }
}

impl Adresses {
    /// Les adresses d'un Google qui repond a `base` (un serveur local).
    pub fn de_la_base(base: &str) -> Adresses {
        Adresses { jetons: format!("{}/token", base), fichiers: format!("{}/drive/v3/files", base), televersement: format!("{}/upload/drive/v3/files", base) }
    }
}

/// CE QUE LE CLIENT RECOIT de l'appelant, qui l'a lu sur le poste (le coffre, la cle du poste) : les identifiants de
/// l'application, le jeton de rafraichissement scelle (ou la phrase qui dit pourquoi il ne se lit pas), l'adresse du
/// compte. Ce module n'ouvre jamais le coffre.
pub struct Autorisation {
    pub identifiants: (Value, Value),
    /// Le jeton enregistre (`Null` s'il n'y en a pas), ou la phrase du refus de sa lecture.
    pub jeton: Result<Value, String>,
    pub compte: Option<String>,
}

/// Le jeton d'acces ne vit qu'en memoire.
pub struct Client {
    identifiants: Option<(Value, Value)>,
    jeton: Result<Value, String>,
    compte: Option<String>,
    acces: Option<String>,
    adresses: Adresses,
    /// Le delai d'un appel, en secondes (120, comme `urlopen(timeout=120)`) ; la preuve le raccourcit.
    delai: u64,
    /// L'attente avant la premiere nouvelle tentative, en millisecondes (`ATTENTE_DE_BASE_MS`) ; la preuve la raccourcit.
    attente_ms: u64,
}

impl Client {
    /// Le client du Drive, avec ce que l'appelant a lu (`Autorisation`). Aucun appel reseau.
    pub fn nouveau(autorisation: Autorisation, adresses: Adresses) -> Client {
        Client {
            identifiants: Some(autorisation.identifiants),
            jeton: autorisation.jeton,
            compte: autorisation.compte,
            acces: None,
            adresses,
            delai: DELAI_APPEL,
            attente_ms: ATTENTE_DE_BASE_MS,
        }
    }

    /// Pour la preuve : un client sans coffre ni jeton scelle, qui parle a `adresses` avec un jeton
    /// d'acces deja donne. Ne refuse rien : n'a de sens que sur des adresses locales.
    pub fn pour_essai(adresses: Adresses, acces: &str, compte: Option<&str>) -> Client {
        Client {
            identifiants: None,
            jeton: Ok(Value::Null),
            compte: compte.map(str::to_string),
            acces: Some(acces.to_string()),
            adresses,
            delai: DELAI_APPEL,
            attente_ms: ATTENTE_DE_BASE_MS,
        }
    }

    /// Les memes identifiants et le meme jeton scelle, mais d'autres adresses (un serveur local).
    pub fn avec_adresses(mut self, adresses: Adresses) -> Client {
        self.adresses = adresses;
        self
    }

    /// Pour la preuve : une attente avant la premiere nouvelle tentative plus courte que la seconde de Google.
    pub fn avec_attente(mut self, millisecondes: u64) -> Client {
        self.attente_ms = millisecondes;
        self
    }

    // ------------------------------------------------------------- transport

    /// `Client.jeton_acces` : le jeton d'acces, demande a Google au premier appel.
    pub fn jeton_acces(&mut self) -> Result<String, ErreurGoogle> {
        if let Some(a) = self.acces.as_ref().filter(|a| !a.is_empty()) {
            return Ok(a.clone());
        }
        let enregistre = self.jeton.clone().map_err(ErreurGoogle::google)?;
        let rafraichissement = champ(&enregistre, "rafraichissement")?;
        if !vrai(&rafraichissement) {
            return Err(ErreurGoogle::google(
                "Compte Google pas encore lie : Configuration > Mots de passe > Lier le Drive, ou `moteur google \
                 autoriser`.",
            ));
        }
        let Some((id, secret)) = &self.identifiants else {
            return Err(ErreurGoogle::google("Client sans identifiants de l'application Google."));
        };
        let acces = jeton_par_rafraichissement(&self.adresses.jetons, id, secret, &rafraichissement, self.delai.min(60))?.map_err(ErreurGoogle::google)?;
        self.acces = Some(acces.clone());
        Ok(acces)
    }

    /// `Client.appel` : GET, ou POST si des donnees sont fournies, ou `methode`. Rend le JSON de la
    /// reponse (`{}` si elle est vide).
    ///
    /// UN JETON D'ACCES NE VIT QU'UNE HEURE (3600 s) : garde pour la vie du client, une sauvegarde de plus
    /// d'une heure recevrait des 401 pour chaque fichier restant. Un 401 fait donc oublier le jeton, en demande
    /// un autre (rafraichissement) et recommence UNE fois ; un second 401 est rendu comme une erreur ordinaire. Un
    /// client sans identifiants (`pour_essai`) ne peut pas rafraichir : le 401 est rendu tel quel.
    pub fn appel(&mut self, url: &str, donnees: Option<&[u8]>, content_type: Option<&str>, methode: Option<&str>) -> Result<Value, ErreurGoogle> {
        let brut = self.appel_brut(url, donnees, content_type, methode)?;
        if brut.is_empty() {
            return Ok(json!({}));
        }
        let texte = String::from_utf8(brut).map_err(|e| ErreurGoogle::nouvelle("UnicodeDecodeError", e.to_string()))?;
        serde_json::from_str(&texte).map_err(|e| ErreurGoogle::nouvelle("JSONDecodeError", e.to_string()))
    }

    /// `appel` sans lecture du JSON : les octets de la reponse, tels quels (le contenu d'un fichier, `telecharger`). Le
    /// meme 401 rattrape une fois ; une erreur passagere (5xx, 429, limite de debit) se retente jusqu'a
    /// `NOUVELLES_TENTATIVES` fois, apres 1 s, 2 s puis 4 s, plus un hasard (`passagere`, la page de Google citee la).
    pub fn appel_brut(&mut self, url: &str, donnees: Option<&[u8]>, content_type: Option<&str>, methode: Option<&str>) -> Result<Vec<u8>, ErreurGoogle> {
        let mut tentative = 0;
        loop {
            let r = match self.appel_une_fois(url, donnees, content_type, methode) {
                Err(e) if e.statut == Some(401) && self.identifiants.is_some() => {
                    self.acces = None;
                    self.appel_une_fois(url, donnees, content_type, methode)
                }
                autre => autre,
            };
            match r {
                Err(e) if passagere(&e) && tentative < NOUVELLES_TENTATIVES => {
                    std::thread::sleep(std::time::Duration::from_millis(attente(self.attente_ms, tentative)));
                    tentative += 1;
                }
                autre => return autre,
            }
        }
    }

    fn appel_une_fois(&mut self, url: &str, donnees: Option<&[u8]>, content_type: Option<&str>, methode: Option<&str>) -> Result<Vec<u8>, ErreurGoogle> {
        let autorisation = format!("Bearer {}", self.jeton_acces()?);
        let entetes = [("Authorization", autorisation.as_str())];
        let requete = Requete {
            url,
            jeton: None,
            donnees,
            content_type,
            delai: self.delai,
            entetes: &entetes,
            methode,
            ..Default::default()
        };
        match envoyer(&requete) {
            Ok((brut, _)) => Ok(brut),
            Err(panne) => {
                let statut = match &panne {
                    crate::transport::Panne::Statut { code, .. } => Some(*code),
                    _ => None,
                };
                let mut e = match phrase_de_panne(panne) {
                    Ok(phrase) => ErreurGoogle::google(phrase),
                    Err((classe, message)) => ErreurGoogle::nouvelle(classe, message),
                };
                e.statut = statut;
                Err(e)
            }
        }
    }

    // ----------------------------------------------------------------- Drive

    /// `Client.dossier` : l'identifiant du dossier `nom` sous `parent`, cree s'il n'existe pas.
    pub fn dossier(&mut self, nom: &str, parent: &str) -> Result<String, ErreurGoogle> {
        let requete = format!(
            "name = '{}' and mimeType = '{}' and '{}' in parents and trashed = false",
            echapper(nom),
            DOSSIER_DRIVE,
            parent
        );
        let url = format!(
            "{}?{}",
            self.adresses.fichiers,
            encoder(&[("q", requete), ("fields", "files(id,name)".into()), ("pageSize", "10".into())])
        );
        let trouves = self.appel(&url, None, None, None)?;
        if let Some(id) = premier_de(&trouves, true)? {
            return Ok(id);
        }
        let corps = socle::json_ascii::dumps(&json!({"name": nom, "mimeType": DOSSIER_DRIVE, "parents": [parent]}), None, false, false);
        let url = self.adresses.fichiers.clone();
        let cree = self.appel(&url, Some(corps.as_bytes()), Some("application/json"), None)?;
        texte_requis(&cree, "id")
    }
}

/// L'apostrophe se protege dans la requete `q`.
///
/// La barre oblique inverse se protege AUSSI (`\\`), comme la syntaxe de `q` l'exige : sans cela, un nom
/// qui en porte une donnerait une requete que Drive refuse.
fn echapper(nom: &str) -> String {
    nom.replace('\\', "\\\\").replace('\'', "\\'")
}

/// `x` doit etre un objet JSON (`AttributeError` sinon).
fn en_dict(v: &Value) -> Result<&serde_json::Map<String, Value>, ErreurGoogle> {
    match v {
        Value::Object(o) => Ok(o),
        autre => Err(ErreurGoogle::nouvelle("AttributeError", socle::fichiers::attendu("un objet", socle::valeur::nom_type(autre)))),
    }
}

/// `x[cle]` : le texte d'une cle qui doit etre la (`KeyError` sinon ; `TypeError` si `x` n'est pas un objet).
fn texte_requis(v: &Value, cle: &str) -> Result<String, ErreurGoogle> {
    match v {
        Value::Object(o) => match o.get(cle) {
            Some(x) => Ok(en_texte(x)),
            None => Err(ErreurGoogle::nouvelle("KeyError", socle::fichiers::absent(cle))),
        },
        autre => Err(ErreurGoogle::nouvelle("TypeError", socle::fichiers::attendu("un objet", socle::valeur::nom_type(autre)))),
    }
}

/// Les elements du listage (`files`, une liste ; absent : aucun).
fn elements_de(r: &Value) -> Result<Vec<&Value>, ErreurGoogle> {
    let o = en_dict(r)?;
    match o.get("files") {
        None => Ok(Vec::new()),
        Some(Value::Array(a)) => Ok(a.iter().collect()),
        Some(autre) => Err(ErreurGoogle::nouvelle("TypeError", socle::fichiers::attendu("une liste", socle::valeur::nom_type(autre)))),
    }
}

/// La taille d'un fichier (`size`, que Drive ecrit en texte).
fn octets_de(t: &Value) -> Result<u64, ErreurGoogle> {
    match t {
        Value::Number(n) => Ok(n.as_u64().or_else(|| n.as_f64().map(|f| f.trunc().max(0.0) as u64)).unwrap_or(0)),
        Value::String(x) => x
            .trim()
            .parse::<u64>()
            .map_err(|_| ErreurGoogle::nouvelle("ValueError", format!("nombre entier illisible : '{}'", x))),
        autre => Err(ErreurGoogle::nouvelle(
            "TypeError",
            socle::fichiers::attendu("un nombre entier", socle::valeur::nom_type(autre)),
        )),
    }
}

/// L'identifiant du premier fichier trouve, `None` s'il n'y en a pas ; `strict` : le premier doit porter un `id`. Les
/// formes que Drive ne rend jamais sont refusees.
fn premier_de(trouves: &Value, strict: bool) -> Result<Option<String>, ErreurGoogle> {
    let o = en_dict(trouves)?;
    let files = match o.get("files") {
        Some(f) if vrai(f) => f,
        _ => return Ok(None),
    };
    let premier: &Value = match files {
        Value::Array(a) => &a[0],
        autre => return Err(ErreurGoogle::nouvelle("TypeError", socle::fichiers::attendu("une liste", socle::valeur::nom_type(autre)))),
    };
    if strict {
        return Ok(Some(texte_requis(premier, "id")?));
    }
    Ok(en_dict(premier)?.get("id").filter(|i| vrai(i)).map(en_texte))
}

/// L'attente avant la nouvelle tentative `n` (0 pour la premiere) : `base * 2^n` plus un hasard d'au plus `base`
/// (« wait 1 + random_number_milliseconds », puis 2, puis 4 : `google-drive-api-limites-et-quotas-20261020.txt`).
fn attente(base: u64, n: u32) -> u64 {
    let mut o = [0u8; 2];
    let hasard = if getrandom::fill(&mut o).is_ok() { u64::from(u16::from_le_bytes(o)) * base / u64::from(u16::MAX) } else { base / 2 };
    base.saturating_mul(1 << n.min(6)) + hasard
}

/// `secrets.token_hex(16)`.
fn hasard_hex() -> String {
    let mut o = [0u8; 16];
    if getrandom::fill(&mut o).is_err() {
        // Pas d'alea du systeme : l'heure suffit a faire une frontiere improbable dans un corps.
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        o.copy_from_slice(&t.to_le_bytes());
    }
    o.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Le corps `multipart/related` d'un premier depot (`Client.televerser`) : les metadonnees JSON,
/// puis le contenu. Rend (corps, frontiere).
pub fn corps_multipart(nom: &str, parent: &str, type_mime: &str, contenu: &[u8], frontiere: &str) -> Vec<u8> {
    let meta = socle::json_ascii::dumps(&json!({"name": nom, "parents": [parent]}), None, false, false);
    let mut corps = Vec::with_capacity(contenu.len() + 512);
    corps.extend_from_slice(b"--");
    corps.extend_from_slice(frontiere.as_bytes());
    corps.extend_from_slice(b"\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n");
    corps.extend_from_slice(meta.as_bytes());
    corps.extend_from_slice(b"\r\n--");
    corps.extend_from_slice(frontiere.as_bytes());
    corps.extend_from_slice(format!("\r\nContent-Type: {}\r\n\r\n", type_mime).as_bytes());
    corps.extend_from_slice(contenu);
    corps.extend_from_slice(b"\r\n--");
    corps.extend_from_slice(frontiere.as_bytes());
    corps.extend_from_slice(b"--\r\n");
    corps
}

impl Drive for Client {
    fn compte(&self) -> Option<String> {
        self.compte.clone()
    }

    fn chemin(&mut self, noms: &[&str]) -> Result<String, ErreurGoogle> {
        let mut parent = "root".to_string();
        for nom in noms {
            parent = self.dossier(nom, &parent)?;
        }
        Ok(parent)
    }

    fn lister(&mut self, parent: &str) -> Result<BTreeMap<String, Distant>, ErreurGoogle> {
        let mut fichiers = BTreeMap::new();
        let mut page: Option<String> = None;
        loop {
            let mut champs = vec![
                ("q", format!("'{}' in parents and trashed = false", parent)),
                ("fields", "nextPageToken,files(id,name,md5Checksum,size,mimeType)".to_string()),
                ("pageSize", "200".to_string()),
            ];
            if let Some(p) = &page {
                champs.push(("pageToken", p.clone()));
            }
            let url = format!("{}?{}", self.adresses.fichiers, encoder(&champs));
            let r = self.appel(&url, None, None, None)?;
            for f in elements_de(&r)? {
                if en_dict(f)?.get("mimeType").and_then(Value::as_str) == Some(DOSSIER_DRIVE) {
                    continue;
                }
                let nom = texte_requis(f, "name")?;
                let id = texte_requis(f, "id")?;
                let md5 = f.get("md5Checksum").filter(|m| !m.is_null()).map(en_texte);
                let octets = match f.get("size") {
                    Some(t) if vrai(t) => octets_de(t)?,
                    _ => 0,
                };
                fichiers.insert(nom, Distant { id, md5, octets });
            }
            page = r.get("nextPageToken").filter(|p| vrai(p)).map(en_texte);
            if page.is_none() {
                return Ok(fichiers);
            }
        }
    }

    fn televerser(&mut self, fichier: &Path, parent: &str, nom: Option<&str>, connu: Option<&Distant>) -> Result<Depose, ErreurGoogle> {
        let nom: String = match nom.filter(|n| !n.is_empty()) {
            Some(n) => n.to_string(),
            None => fichier.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        };
        let type_mime = crate::envoi::type_devine(&nom).unwrap_or("application/octet-stream");

        if let Some(distant) = connu {
            let url = format!(
                "{}/{}?{}",
                self.adresses.televersement,
                distant.id,
                encoder(&[("uploadType", "media".into()), ("fields", "id,name,webViewLink".into())])
            );
            let contenu = std::fs::read(fichier).map_err(|e| ErreurGoogle::fichier(&e, fichier))?;
            let r = self.appel(&url, Some(&contenu), Some(type_mime), Some("PATCH"))?;
            let mut depose = depose_de(&r);
            depose.remplace = true;
            return Ok(depose);
        }

        let frontiere = format!("----{}", hasard_hex());
        let contenu = std::fs::read(fichier).map_err(|e| ErreurGoogle::fichier(&e, fichier))?;
        let corps = corps_multipart(&nom, parent, type_mime, &contenu, &frontiere);
        let url = format!(
            "{}?{}",
            self.adresses.televersement,
            encoder(&[("uploadType", "multipart".into()), ("fields", "id,name,webViewLink".into())])
        );
        let r = self.appel(&url, Some(&corps), Some(&format!("multipart/related; boundary={}", frontiere)), None)?;
        Ok(depose_de(&r))
    }

    fn sous_dossiers(&mut self, parent: &str) -> Result<Vec<(String, String)>, ErreurGoogle> {
        let mut dossiers = Vec::new();
        let mut page: Option<String> = None;
        loop {
            let mut champs = vec![
                ("q", format!("'{}' in parents and mimeType = '{}' and trashed = false", parent, DOSSIER_DRIVE)),
                ("fields", "nextPageToken,files(id,name)".to_string()),
                ("pageSize", "200".to_string()),
            ];
            if let Some(p) = &page {
                champs.push(("pageToken", p.clone()));
            }
            let url = format!("{}?{}", self.adresses.fichiers, encoder(&champs));
            let r = self.appel(&url, None, None, None)?;
            for f in elements_de(&r)? {
                dossiers.push((texte_requis(f, "name")?, texte_requis(f, "id")?));
            }
            page = r.get("nextPageToken").filter(|p| vrai(p)).map(en_texte);
            if page.is_none() {
                return Ok(dossiers);
            }
        }
    }

    fn telecharger(&mut self, id: &str) -> Result<Vec<u8>, ErreurGoogle> {
        // Un identifiant Drive ne porte que des lettres, des chiffres, `-` et `_` ; il s'encode quand meme, on ne
        // fabrique pas un chemin d'URL avec ce qu'un serveur a rendu.
        let url = format!("{}/{}?{}", self.adresses.fichiers, socle::html::pourcent(id, ""), encoder(&[("alt", "media".into())]));
        self.appel_brut(&url, None, None, None)
    }
}

fn depose_de(r: &Value) -> Depose {
    let texte = |c: &str| r.get(c).filter(|x| !x.is_null()).map(en_texte);
    Depose { id: texte("id"), nom: texte("name"), lien: texte("webViewLink"), remplace: false }
}
