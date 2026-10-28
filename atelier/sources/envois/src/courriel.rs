//! La boite courriel par IMAP et SMTP, avec un mot de passe d'application : la boite s'ouvre par un MOT DE PASSE
//! D'APPLICATION que chaque compagnie fabrique dans son compte Google (aucune permission Google, aucun audit). Le compte
//! Google autorise ne sert qu'au Drive.
//!
//! CE QUE CE MODULE OFFRE : le trait `Courriel` (`compte`, `envoyer(mime) -> identifiant`, `rechercher`,
//! `message_brut`, `fermer`), avec DEUX realisations : `Client`, le vrai (IMAP ecrit ici sur un flux rustls, SMTP par
//! `lettre`), et `BoiteSimulee`, la boite de la compagnie fictive, qui GARDE les messages et ne les envoie pas. Un
//! identifiant de message ENVOYE est son en-tete `Message-ID` ; celui d'un message RECU est son UID IMAP. La
//! recherche est celle du serveur (`UID SEARCH TEXT`), dans la boite de reception seule.
//!
//! LE SECRET : `Client::nouveau` recoit l'adresse et le mot de passe de la boite ; ce module n'ouvre jamais le coffre
//! (regle 3). Les espaces du mot de passe d'application (quatre groupes de quatre lettres) sont retires.
//!
//! LES SERVEURS : Gmail par defaut (`imap.gmail.com:993`, `smtp.gmail.com:465`, TLS implicite) ; une autre messagerie
//! se dit dans `config/courriel.json` (`{"imap": ..., "smtp": ..., "port_smtp": 465}`). Le port SMTP 587 declenche
//! STARTTLS ; tout autre port, TLS implicite.
//!
//! LE CERTIFICAT, ET L'ANTIVIRUS : verification normale contre les racines du SYSTEME (le verificateur de la
//! plate-forme, comme `transport.rs`), de sorte que la racine d'un antivirus (Avast « Web/Mail Shield », qui
//! s'intercale sur les ports 993 et 465) installee dans le magasin Windows passe, comme chez Thunderbird ou Outlook.
//! Delai de 60 s par connexion.
//!
//! L'IMAP : le Python ne fait que `LOGIN`, `EXAMINE` (lecture seule), `UID SEARCH TEXT`, `UID FETCH BODY.PEEK[]`,
//! `LOGOUT`. Ce dialogue est ecrit ici sur un flux quelconque (`Read + Write` : un faux serveur en memoire dans les
//! tests), et les reponses sont lues par `imap-proto` ; seuls les litteraux `{n}` se lisent a la main, par leur
//! longueur.
//!
//! Ecarts admis, notes : les phrases d'erreur different du Python ; un mot de passe d'un caractere de controle est
//! refuse ; le message envoye part avec des fins de ligne CRLF sur le fil (le Python envoyait les octets tels quels,
//! LF seul) -- le message archive reste celui d'`envoi::en_mime`. La ligne vide que `smtplib` ajoutait avant le point
//! final (le message finit par LF, pas CRLF) est reproduite : au CRLF pres, les octets du fil sont ceux du Python.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use imap_proto::{AttributeValue, MailboxDatum, Response, Status};
use serde_json::{json, Value};

use droit::contexte::Contexte;

pub const FICHIER_SERVEURS: &str = "courriel.json";
/// Secondes, par connexion.
pub const DELAI: u64 = 60;
pub const DOSSIER_RECEPTION: &str = "INBOX";
pub const DOSSIER_COURRIELS: &str = "courriels-simules";

/// La boite refuse, ou le reseau. Jamais un mot de passe dedans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErreurCourriel(pub String);

impl std::fmt::Display for ErreurCourriel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ErreurCourriel {}

fn erreur(m: impl Into<String>) -> ErreurCourriel {
    ErreurCourriel(m.into())
}

/// La surface de `courriel.Client` : celle que `envoi` et la releve des signatures utilisent.
pub trait Courriel {
    /// L'adresse de la boite.
    fn compte(&self) -> &str;
    /// Envoie un message deja assemble ; rend son `Message-ID`.
    fn envoyer(&mut self, message_mime: &[u8]) -> Result<String, ErreurCourriel>;
    /// Les UID des messages de la boite de reception qui contiennent le texte, les plus recents d'abord.
    fn rechercher(&mut self, requete: &str, maximum: usize) -> Result<Vec<String>, ErreurCourriel>;
    /// Un message entier (RFC 822), par son UID.
    fn message_brut(&mut self, identifiant: &str) -> Result<Vec<u8>, ErreurCourriel>;
    fn fermer(&mut self);
    /// La compagnie de la boite : c'est sa fiche qui porte la signature des courriels et le nom affiche
    /// (`signature_courriel`). Rien : les courriels partent tels que les modeles les ecrivent.
    fn racine(&self) -> Option<&Path> {
        None
    }
}

// ---------------------------------------------------------------------------------------------------- le disque

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Serveurs {
    pub imap: String,
    pub port_imap: u16,
    pub smtp: String,
    pub port_smtp: u16,
}

impl Serveurs {
    pub fn gmail() -> Serveurs {
        Serveurs { imap: "imap.gmail.com".into(), port_imap: 993, smtp: "smtp.gmail.com".into(), port_smtp: 465 }
    }
}

fn port_de(v: &Value) -> Result<u16, ErreurCourriel> {
    let n = match v {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        Value::String(s) => s.trim().parse::<i64>().ok(),
        _ => None,
    };
    n.and_then(|n| u16::try_from(n).ok()).ok_or_else(|| erreur(format!("port illisible dans config/{} : {}", FICHIER_SERVEURS, v)))
}

fn hote_de(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        autre => autre.to_string(),
    }
}

/// Gmail par defaut ; `config/courriel.json` change les cles `imap`, `port_imap`, `smtp`, `port_smtp` (les autres
/// cles sont ignorees).
pub fn serveurs(racine: &Path) -> Result<Serveurs, ErreurCourriel> {
    let p = racine.join("config").join(FICHIER_SERVEURS);
    let mut s = Serveurs::gmail();
    if !p.exists() {
        return Ok(s);
    }
    let texte = std::fs::read_to_string(&p).map_err(|e| erreur(format!("{} illisible : {}", p.display(), e)))?;
    let propres: Value = serde_json::from_str(&texte).map_err(|e| erreur(format!("{} illisible : {}", p.display(), e)))?;
    let Value::Object(o) = propres else { return Err(erreur(format!("{} illisible : ce n'est pas un objet", p.display()))) };
    if let Some(v) = o.get("imap") {
        s.imap = hote_de(v);
    }
    if let Some(v) = o.get("smtp") {
        s.smtp = hote_de(v);
    }
    if let Some(v) = o.get("port_imap") {
        s.port_imap = port_de(v)?;
    }
    if let Some(v) = o.get("port_smtp") {
        s.port_smtp = port_de(v)?;
    }
    Ok(s)
}

/// Le message du serveur, en francais quand on le reconnait.
pub fn traduire(texte: &str) -> String {
    if texte.contains("AUTHENTICATIONFAILED") || texte.contains("Username and Password not accepted") || texte.contains("(535)") {
        return "La boite refuse le mot de passe. Pour Gmail il faut un mot de passe D'APPLICATION (validation en deux \
                etapes activee, puis myaccount.google.com > Securite > Mots de passe des applications), pas celui du \
                compte."
            .to_string();
    }
    texte.to_string()
}

fn injoignable(e: impl std::fmt::Display) -> ErreurCourriel {
    erreur(format!("Serveur injoignable : {}", e))
}

// ------------------------------------------------------------------------------------------------------- le TLS

/// Un flux lisible et inscriptible : une connexion TLS, ou un faux serveur en memoire.
pub trait Flux: Read + Write {}
impl<T: Read + Write + ?Sized> Flux for T {}

fn connecter(hote: &str, port: u16) -> Result<TcpStream, ErreurCourriel> {
    // En essai, le garde-fou reseau : aucune boite reelle.
    socle::garde_reseau::directe(hote).map_err(injoignable)?;
    let delai = Duration::from_secs(DELAI);
    let adresses: Vec<_> = (hote, port).to_socket_addrs().map_err(injoignable)?.collect();
    let mut derniere = None;
    for a in adresses {
        match TcpStream::connect_timeout(&a, delai) {
            Ok(s) => {
                s.set_read_timeout(Some(delai)).map_err(injoignable)?;
                s.set_write_timeout(Some(delai)).map_err(injoignable)?;
                return Ok(s);
            }
            Err(e) => derniere = Some(e),
        }
    }
    Err(derniere.map(injoignable).unwrap_or_else(|| injoignable("aucune adresse")))
}

/// La configuration TLS des connexions IMAP : racines du systeme (`rustls-platform-verifier`).
fn configuration_tls() -> Result<Arc<rustls::ClientConfig>, ErreurCourriel> {
    use rustls_platform_verifier::BuilderVerifierExt;
    let fournisseur = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ClientConfig::builder_with_provider(fournisseur)
        .with_safe_default_protocol_versions()
        .map_err(|e| erreur(format!("TLS : {}", e)))?
        .with_platform_verifier()
        .map_err(|e| erreur(format!("TLS : {}", e)))?
        .with_no_client_auth();
    Ok(Arc::new(config))
}

fn ouvrir_tls(hote: &str, port: u16) -> Result<Box<dyn Flux>, ErreurCourriel> {
    let tcp = connecter(hote, port)?;
    let nom = rustls::pki_types::ServerName::try_from(hote.to_string()).map_err(|e| erreur(format!("nom de serveur illisible : {}", e)))?;
    let connexion = rustls::ClientConnection::new(configuration_tls()?, nom).map_err(|e| erreur(format!("TLS : {}", e)))?;
    Ok(Box::new(rustls::StreamOwned::new(connexion, tcp)))
}

// ------------------------------------------------------------------------------------------------------- l'IMAP

/// Ce que le serveur a rendu d'utile a une commande.
#[derive(Debug, Default)]
pub struct Reponse {
    /// Les numeros d'une reponse `SEARCH`.
    pub recherche: Vec<u32>,
    /// Les corps `BODY[]` d'une reponse `FETCH`.
    pub corps: Vec<Vec<u8>>,
}

/// Le dialogue IMAP sur un flux. Pas de connexion ici : l'appelant donne le flux.
pub struct Imap<F: Read + Write> {
    flux: F,
    tampon: Vec<u8>,
    numero: u32,
}

/// Un litteral plus gros que cela n'est pas un courriel que ce logiciel attend.
const LITTERAL_MAXIMUM: usize = 256 * 1024 * 1024;

fn quoter(texte: &str) -> Result<String, ErreurCourriel> {
    if texte.chars().any(|c| c.is_control()) {
        return Err(erreur("Un caractere de controle ne passe pas dans une commande IMAP."));
    }
    Ok(format!("\"{}\"", texte.replace('\\', "\\\\").replace('"', "\\\"")))
}

/// La longueur d'un litteral annonce en fin de ligne (`... {123}\r\n`).
fn longueur_litteral(ligne: &[u8]) -> Option<usize> {
    let sans = ligne.strip_suffix(b"\r\n").or_else(|| ligne.strip_suffix(b"\n"))?;
    let sans = sans.strip_suffix(b"}")?;
    let ouvrante = sans.iter().rposition(|b| *b == b'{')?;
    let chiffres = &sans[ouvrante + 1..];
    if chiffres.is_empty() || !chiffres.iter().all(|c| c.is_ascii_digit()) {
        return None;
    }
    std::str::from_utf8(chiffres).ok()?.parse().ok()
}

fn es(e: std::io::Error) -> ErreurCourriel {
    use std::io::ErrorKind::*;
    match e.kind() {
        TimedOut | WouldBlock => injoignable(format!("delai depasse ({})", e)),
        _ => erreur(e.to_string()),
    }
}

impl<F: Read + Write> Imap<F> {
    /// Lit l'accueil du serveur, puis `LOGIN` et `EXAMINE` (lecture seule) du dossier.
    pub fn ouvrir(flux: F, compte: &str, mot_de_passe: &str, dossier: &str) -> Result<Imap<F>, ErreurCourriel> {
        let mut imap = Imap { flux, tampon: Vec::new(), numero: 0 };
        let accueil = imap.lire_unite()?;
        if let Ok((_, Response::Data { status: Status::Bye, .. })) = Response::parse(&accueil) {
            return Err(erreur(traduire(String::from_utf8_lossy(&accueil).trim())));
        }
        imap.commande(&format!("LOGIN {} {}", quoter(compte)?, quoter(mot_de_passe)?))?;
        imap.commande(&format!("EXAMINE {}", quoter(dossier)?))?;
        Ok(imap)
    }

    fn lire_ligne(&mut self) -> Result<Vec<u8>, ErreurCourriel> {
        loop {
            if let Some(i) = self.tampon.iter().position(|b| *b == b'\n') {
                return Ok(self.tampon.drain(..=i).collect());
            }
            let mut bloc = [0u8; 8192];
            let n = self.flux.read(&mut bloc).map_err(es)?;
            if n == 0 {
                return Err(erreur("Connexion fermee par le serveur."));
            }
            self.tampon.extend_from_slice(&bloc[..n]);
        }
    }

    fn lire_octets(&mut self, n: usize) -> Result<Vec<u8>, ErreurCourriel> {
        while self.tampon.len() < n {
            let mut bloc = [0u8; 8192];
            let lus = self.flux.read(&mut bloc).map_err(es)?;
            if lus == 0 {
                return Err(erreur("Connexion fermee par le serveur."));
            }
            self.tampon.extend_from_slice(&bloc[..lus]);
        }
        Ok(self.tampon.drain(..n).collect())
    }

    /// Une reponse entiere : une ligne, et tout ce que ses litteraux `{n}` en font suivre, par leur longueur.
    fn lire_unite(&mut self) -> Result<Vec<u8>, ErreurCourriel> {
        let mut unite = Vec::new();
        loop {
            let ligne = self.lire_ligne()?;
            unite.extend_from_slice(&ligne);
            match longueur_litteral(&ligne) {
                Some(n) if n > LITTERAL_MAXIMUM => return Err(erreur("Reponse du serveur trop grosse.")),
                Some(n) => {
                    let corps = self.lire_octets(n)?;
                    unite.extend_from_slice(&corps);
                }
                None => return Ok(unite),
            }
        }
    }

    /// Une commande, et les reponses qui lui sont propres jusqu'a la reponse etiquetee. Un refus (`NO`, `BAD`) est
    /// une erreur dont le texte est celui du serveur.
    pub fn commande(&mut self, texte: &str) -> Result<Reponse, ErreurCourriel> {
        self.numero += 1;
        let etiquette = format!("A{:04}", self.numero);
        let ligne = format!("{} {}\r\n", etiquette, texte);
        self.flux.write_all(ligne.as_bytes()).map_err(es)?;
        self.flux.flush().map_err(es)?;
        let mut reponse = Reponse::default();
        loop {
            let unite = self.lire_unite()?;
            if unite.starts_with(format!("{} ", etiquette).as_bytes()) {
                return match Response::parse(&unite) {
                    Ok((_, Response::Done { status: Status::Ok, .. })) => Ok(reponse),
                    Ok((_, Response::Done { .. })) => {
                        let brut = String::from_utf8_lossy(&unite[etiquette.len() + 1..]).trim().to_string();
                        Err(erreur(traduire(&brut)))
                    }
                    _ => Err(erreur("Reponse du serveur illisible.")),
                };
            }
            if unite.starts_with(b"* ") {
                match Response::parse(&unite) {
                    Ok((_, Response::MailboxData(MailboxDatum::Search(v)))) => reponse.recherche.extend(v),
                    Ok((_, Response::Fetch(_, attributs))) => {
                        for a in attributs {
                            match a {
                                AttributeValue::BodySection { data: Some(d), .. } | AttributeValue::Rfc822(Some(d)) => {
                                    reponse.corps.push(d.into_owned())
                                }
                                _ => {}
                            }
                        }
                    }
                    Ok((_, Response::Data { status: Status::Bye, .. })) => {
                        return Err(erreur(traduire(String::from_utf8_lossy(&unite).trim())));
                    }
                    // Le reste (FLAGS, EXISTS, CAPABILITY...) n'interesse pas ce dialogue ; une ligne que le
                    // parseur ne lit pas non plus.
                    _ => {}
                }
                continue;
            }
            if unite.starts_with(b"+") {
                return Err(erreur("Le serveur attend une suite que ce dialogue ne donne jamais."));
            }
        }
    }

    /// `UID SEARCH TEXT "..."` : les UID, les plus recents d'abord, `maximum` au plus. Les guillemets de la syntaxe
    /// Gmail sont toleres et retires.
    pub fn rechercher(&mut self, requete: &str, maximum: usize) -> Result<Vec<String>, ErreurCourriel> {
        let texte = requete.trim().trim_matches('"');
        let r = self.commande(&format!("UID SEARCH TEXT {}", quoter(texte)?))?;
        Ok(r.recherche.iter().rev().take(maximum).map(|u| u.to_string()).collect())
    }

    /// `UID FETCH <uid> (BODY.PEEK[])` : un message entier, sans le marquer lu.
    pub fn message_brut(&mut self, uid: &str) -> Result<Vec<u8>, ErreurCourriel> {
        if uid.is_empty() || !uid.bytes().all(|c| c.is_ascii_digit()) {
            return Err(erreur(format!("Message {} introuvable.", uid)));
        }
        let r = self.commande(&format!("UID FETCH {} (BODY.PEEK[])", uid))?;
        r.corps.into_iter().next().ok_or_else(|| erreur(format!("Message {} introuvable.", uid)))
    }

    /// `LOGOUT`, sans se soucier de la reponse.
    pub fn deconnecter(&mut self) {
        let _ = self.commande("LOGOUT");
    }
}

// --------------------------------------------------------------------------------------------------- le client

/// IMAP pour lire, SMTP pour envoyer. Le mot de passe ne vit qu'en memoire, le temps d'une connexion ; rien n'est
/// journalise.
pub struct Client {
    racine: PathBuf,
    compte: String,
    mot_de_passe: String,
    pub serveurs: Serveurs,
    imap: Option<Imap<Box<dyn Flux>>>,
}

impl Client {
    /// `compte` : l'adresse de la boite ; `mot_de_passe` : celui que l'appelant a lu au coffre (les espaces d'un mot de
    /// passe d'application se retirent ici). Les serveurs se lisent dans `config/courriel.json` de `racine`.
    pub fn nouveau(racine: &Path, compte: &str, mot_de_passe: &str) -> Result<Client, ErreurCourriel> {
        Ok(Client {
            racine: racine.to_path_buf(),
            compte: compte.trim().to_lowercase(),
            mot_de_passe: mot_de_passe.replace(' ', ""),
            serveurs: serveurs(racine)?,
            imap: None,
        })
    }

    fn boite(&mut self) -> Result<&mut Imap<Box<dyn Flux>>, ErreurCourriel> {
        if self.imap.is_none() {
            let flux = ouvrir_tls(&self.serveurs.imap, self.serveurs.port_imap)?;
            self.imap = Some(Imap::ouvrir(flux, &self.compte, &self.mot_de_passe, DOSSIER_RECEPTION)?);
        }
        Ok(self.imap.as_mut().expect("boite ouverte"))
    }
}

/// `\r\n|\n|\r(?!\n)` -> CRLF.
fn crlf(o: &[u8]) -> Vec<u8> {
    let mut s = Vec::with_capacity(o.len() + o.len() / 40);
    let mut i = 0;
    while i < o.len() {
        match o[i] {
            b'\r' if o.get(i + 1) == Some(&b'\n') => {
                s.extend_from_slice(b"\r\n");
                i += 2;
            }
            b'\r' | b'\n' => {
                s.extend_from_slice(b"\r\n");
                i += 1;
            }
            c => {
                s.push(c);
                i += 1;
            }
        }
    }
    s
}

/// Ce que `lettre::send_raw` recoit : le message en CRLF. `lettre` ajoute lui-meme la fin de ligne avant le point final.
/// `smtplib.data` en ajoutait une quand le message ne finissait pas par CRLF (et les octets d'`en_mime` finissent par un
/// LF seul : le Python envoyait donc une ligne vide de plus avant le point). On fait de meme : on ne retire la fin de
/// ligne que si le message en avait deja une, CRLF.
pub fn donnees_du_fil(brut: &[u8]) -> Vec<u8> {
    let mut sur_le_fil = crlf(brut);
    if brut.ends_with(b"\r\n") && sur_le_fil.ends_with(b"\r\n") {
        sur_le_fil.truncate(sur_le_fil.len() - 2);
    }
    sur_le_fil
}

/// Le premier en-tete de ce nom, sa valeur dans le texte du message.
fn premier_entete(entetes: &[mailparse::MailHeader], nom: &str) -> Option<String> {
    entetes.iter().find(|h| h.get_key_ref().eq_ignore_ascii_case(nom)).map(|h| String::from_utf8_lossy(h.get_value_raw()).into_owned())
}

/// Les adresses de `To`, `Cc` et `Bcc` (le premier en-tete de chaque nom, comme `m.get`).
pub fn destinataires(entetes: &[mailparse::MailHeader]) -> Vec<String> {
    let mut sortie = Vec::new();
    for champ in ["To", "Cc", "Bcc"] {
        let Some(valeur) = premier_entete(entetes, champ) else { continue };
        let Ok(liste) = mailparse::addrparse(&valeur) else { continue };
        for a in liste.iter() {
            match a {
                mailparse::MailAddr::Single(s) => sortie.push(s.addr.clone()),
                mailparse::MailAddr::Group(g) => sortie.extend(g.addrs.iter().map(|s| s.addr.clone())),
            }
        }
    }
    sortie
}

impl Courriel for Client {
    fn compte(&self) -> &str {
        &self.compte
    }

    fn racine(&self) -> Option<&Path> {
        Some(&self.racine)
    }

    /// Envoie un message deja assemble. Rend son `Message-ID` (ajoute s'il manque : c'est par lui que la releve
    /// reconnait nos propres envois).
    fn envoyer(&mut self, message_mime: &[u8]) -> Result<String, ErreurCourriel> {
        use lettre::address::{Address, Envelope};
        use lettre::transport::smtp::authentication::Credentials;
        use lettre::transport::smtp::client::{Tls, TlsParameters};
        use lettre::{SmtpTransport, Transport};

        let (entetes, _) = mailparse::parse_headers(message_mime).map_err(|e| erreur(format!("message illisible : {}", e)))?;
        let mut brut = message_mime.to_vec();
        let ident = match premier_entete(&entetes, "Message-ID").map(|v| v.trim().to_string()).filter(|v| !v.is_empty()) {
            Some(i) => i,
            None => {
                let domaine = self.compte.rsplit('@').next().unwrap_or("localhost");
                let i = crate::envoi::nouveau_message_id(domaine);
                let mut avec = format!("Message-ID: {}\n", i).into_bytes();
                avec.extend_from_slice(&brut);
                brut = avec;
                i
            }
        };
        let destinataires = destinataires(&entetes);
        let adresses: Result<Vec<Address>, _> = destinataires.iter().map(|a| a.parse::<Address>()).collect();
        let adresses = adresses.map_err(|e| erreur(format!("adresse de destinataire illisible : {}", e)))?;
        let expediteur: Address = self.compte.parse().map_err(|e| erreur(format!("adresse de la boite illisible : {}", e)))?;
        let enveloppe = Envelope::new(Some(expediteur), adresses).map_err(|e| erreur(e.to_string()))?;
        let mdp = self.mot_de_passe.clone();
        let (hote, port) = (self.serveurs.smtp.clone(), self.serveurs.port_smtp);
        // En essai, le garde-fou reseau : aucun serveur d'envoi reel.
        socle::garde_reseau::directe(&hote).map_err(injoignable)?;
        let tls = TlsParameters::builder(hote.clone()).build_rustls().map_err(|e| erreur(format!("TLS : {}", e)))?;
        let transport = SmtpTransport::builder_dangerous(hote.as_str())
            .port(port)
            .tls(if port == 587 { Tls::Required(tls) } else { Tls::Wrapper(tls) })
            .credentials(Credentials::new(self.compte.clone(), mdp))
            .timeout(Some(Duration::from_secs(DELAI)))
            .build();
        let sur_le_fil = donnees_du_fil(&brut);
        transport.send_raw(&enveloppe, &sur_le_fil).map_err(|e| erreur(traduire(&e.to_string())))?;
        Ok(ident)
    }

    fn rechercher(&mut self, requete: &str, maximum: usize) -> Result<Vec<String>, ErreurCourriel> {
        self.boite()?.rechercher(requete, maximum)
    }

    fn message_brut(&mut self, identifiant: &str) -> Result<Vec<u8>, ErreurCourriel> {
        self.boite()?.message_brut(identifiant)
    }

    fn fermer(&mut self) {
        if let Some(mut i) = self.imap.take() {
            i.deconnecter();
        }
    }
}

// ----------------------------------------------------------------------------------------------- la verification

/// La boite s'ouvre-t-elle avec ce client ? UNE tentative IMAP. Rend `{"ok", "message", "conseil"}` -- jamais le mot
/// de passe.
pub fn verifier(mut c: Client) -> Value {
    let ouverture = c.boite().map(|_| ());
    c.fermer();
    match ouverture {
        Err(e) => json!({"ok": false, "message": e.0.lines().next().unwrap_or(""), "conseil": ""}),
        Ok(()) => json!({
            "ok": true,
            "message": format!("boite {} ouverte ({}).", c.compte, c.serveurs.imap),
            "conseil": ""
        }),
    }
}

// ---------------------------------------------------------------------------------------------- la boite simulee

/// La boite courriel de la compagnie fictive : elle garde, elle n'envoie pas. Meme surface que `Client`. Chaque message
/// est ecrit tel quel dans `courriels-simules/`, pour qu'on puisse l'ouvrir et voir ce qui serait parti.
pub struct BoiteSimulee {
    racine: PathBuf,
    compte: String,
}

impl BoiteSimulee {
    /// `entreprise` : la fiche de la compagnie (le signataire donne l'adresse de la boite).
    pub fn nouvelle(racine: &Path, entreprise: &Value) -> BoiteSimulee {
        let adresse = entreprise
            .get("signataire")
            .and_then(|s| s.get("email"))
            .and_then(Value::as_str)
            .filter(|e| !e.is_empty())
            .unwrap_or("fictive@puppetpaie.invalid");
        BoiteSimulee { racine: racine.to_path_buf(), compte: adresse.to_lowercase() }
    }

    pub fn du_contexte(ctx: &Contexte) -> BoiteSimulee {
        BoiteSimulee::nouvelle(&ctx.racine, &ctx.entreprise)
    }
}

/// `re.search(rb"^Message-ID:\s*(<[^>\r\n]+>)", brut, re.I | re.M)`.
fn message_id_de(brut: &[u8]) -> Option<String> {
    let blanc = |c: u8| matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c);
    let mut debut = 0;
    loop {
        let reste = &brut[debut..];
        if reste.len() >= 11 && reste[..11].eq_ignore_ascii_case(b"message-id:") {
            let mut i = 11;
            while i < reste.len() && blanc(reste[i]) {
                i += 1;
            }
            if reste.get(i) == Some(&b'<') {
                let fin = reste[i + 1..].iter().position(|c| matches!(c, b'>' | b'\r' | b'\n'));
                if let Some(f) = fin.filter(|f| *f > 0 && reste[i + 1 + f] == b'>') {
                    return Some(String::from_utf8_lossy(&reste[i..i + f + 2]).into_owned());
                }
            }
        }
        match brut[debut..].iter().position(|c| *c == b'\n') {
            Some(p) => debut += p + 1,
            None => return None,
        }
    }
}

pub fn hexa_au_hasard(octets: usize) -> String {
    let mut o = vec![0u8; octets];
    if socle::temps::aleas(&mut o).is_err() {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(1);
        for (i, c) in o.iter_mut().enumerate() {
            *c = (t >> (i * 8 % 64)) as u8;
        }
    }
    o.iter().map(|c| format!("{:02x}", c)).collect()
}

/// `%Y%m%d-%H%M%S-%f` de l'heure de Paris : l'ordre des noms est celui des envois.
fn horodatage_de(t: std::time::Duration) -> String {
    let secondes = t.as_secs() as i64;
    let local = secondes + socle::paris::decalage(secondes);
    let j = socle::date::Jour::depuis_jours_unix(local.div_euclid(86_400)).unwrap_or(socle::date::Jour { an: 1970, mois: 1, jour: 1 });
    let s = local.rem_euclid(86_400);
    format!("{:04}{:02}{:02}-{:02}{:02}{:02}-{:06}", j.an, j.mois, j.jour, s / 3600, s % 3600 / 60, s % 60, t.subsec_micros())
}

impl Courriel for BoiteSimulee {
    fn compte(&self) -> &str {
        &self.compte
    }

    fn racine(&self) -> Option<&Path> {
        Some(&self.racine)
    }

    /// Ecrit le message dans `courriels-simules/` ; l'identifiant est son `Message-ID`, comme chez le vrai client :
    /// c'est par lui que la compagnie fictive retrouve ce qui est parti.
    fn envoyer(&mut self, message_mime: &[u8]) -> Result<String, ErreurCourriel> {
        let dossier = self.racine.join(DOSSIER_COURRIELS);
        std::fs::create_dir_all(&dossier).map_err(|e| erreur(format!("{} : {}", dossier.display(), e)))?;
        let ident = message_id_de(message_mime).unwrap_or_else(|| format!("<{}@fictive.puppetpaie>", hexa_au_hasard(8)));
        let maintenant = std::time::Duration::from_secs_f64(socle::temps::secondes());
        let nom = format!("{}_{}.eml", horodatage_de(maintenant), hexa_au_hasard(4));
        let chemin = dossier.join(&nom);
        std::fs::write(&chemin, message_mime).map_err(|e| erreur(format!("{} : {}", chemin.display(), e)))?;
        Ok(ident)
    }

    fn rechercher(&mut self, _requete: &str, _maximum: usize) -> Result<Vec<String>, ErreurCourriel> {
        Ok(Vec::new())
    }

    fn message_brut(&mut self, _identifiant: &str) -> Result<Vec<u8>, ErreurCourriel> {
        Ok(Vec::new())
    }

    fn fermer(&mut self) {}
}
