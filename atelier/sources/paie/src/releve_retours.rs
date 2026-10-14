//! La releve des retours (`moteur/operations_retours.py` : `relever_retours`, `relever_retours_dpae`),
//! et le client qui parle aux organismes (`moteur/netentreprises.py` : `Client`, `moteur/dpae_api.py` :
//! `ClientDpae`) -- ce que le Python gardait derriere `paie transport` (lot H2).
//!
//! RELEVER EST UNE LECTURE : on archive sans rien demander a personne, aucune seance n'est exigee, rien
//! n'est applique. L'archive brute est la piece justificative : elle est ecrite avant toute analyse. Un 404
//! sur une tranche veut dire << rien sur cette tranche >>, tout autre refus arrete (CLAUDE.md, regle 5).
//!
//! CE MODULE NE CHERCHE JAMAIS UN SECRET POUR LE TRANSPORT : `Client` en recoit un, ou lit la variable
//! d'environnement du service, comme `netentreprises.Client` ; c'est la releve (couche qui a vu l'humain)
//! qui puise au coffre pour une consultation. Le mot de passe sert le temps de l'authentification
//! (`transport::operation`), n'est ni journalise, ni affiche, ni renvoye.
//!
//! Le reseau passe par `transport::operation` (le meme JSON que `paie transport`) : rien n'est reecrit.
//! La demonstration (`simulation.rattacher`) pose les retours des depots joues, sans reseau.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::contexte::Contexte;
use crate::etats::{erreur, py_str, R};
use crate::fichiers::vrai;
use crate::retours::{base64_ecrire, base64_lire, Echec};

// ------------------------------------------------------------ l'horloge locale

/// Les secondes unix de l'horloge.
fn secondes_unix() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// `datetime.now()` a l'heure de Paris, en secondes d'une epoque « naive » (l'heure locale lue comme si
/// c'etait de l'UTC) : de quoi additionner des jours et comparer deux heures locales.
pub fn maintenant_naif() -> i64 {
    let t = secondes_unix();
    t + crate::paris::decalage(t)
}

/// `AAAA-MM-JJTHH:MM:SS` d'une heure locale naive.
pub fn iso_de_naif(local: i64) -> String {
    let jours = local.div_euclid(86_400);
    let reste = local.rem_euclid(86_400);
    let j = crate::date::Jour::depuis_jours_unix(jours).unwrap_or(crate::date::Jour { an: 1970, mois: 1, jour: 1 });
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}", j.an, j.mois, j.jour, reste / 3_600, (reste % 3_600) / 60, reste % 60)
}

/// L'heure locale naive d'un `AAAA-MM-JJ[THH:MM[:SS]]` (`datetime.fromisoformat`), ou `None`.
pub fn naif_de_iso(iso: &str) -> Option<i64> {
    let c: Vec<char> = iso.chars().collect();
    let n = |a: usize, b: usize| -> Option<i64> {
        let t: String = c.get(a..b)?.iter().collect();
        t.chars().all(|x| x.is_ascii_digit()).then(|| t.parse().ok()).flatten()
    };
    let jour = crate::date::Jour::nouveau(n(0, 4)? as i32, n(5, 7)? as u32, n(8, 10)? as u32)?;
    let jours = jour.ordinal() - 719_163;
    let (h, m, s) = if c.len() >= 16 { (n(11, 13)?, n(14, 16)?, if c.len() >= 19 { n(17, 19)? } else { 0 }) } else { (0, 0, 0) };
    Some(jours * 86_400 + h * 3_600 + m * 60 + s)
}

/// `strftime("%Y%m%d-%H%M%S")` d'une heure locale naive.
fn horodatage_archive(local: i64) -> String {
    let iso = iso_de_naif(local);
    format!("{}{}{}-{}{}{}", &iso[0..4], &iso[5..7], &iso[8..10], &iso[11..13], &iso[14..16], &iso[17..19])
}

// -------------------------------------------------------------------- les pannes

/// Une panne du transport : `ErreurApi` (son message et son `code`, 0 quand aucun statut), ou une autre
/// exception que la releve ne rattrape pas.
#[derive(Debug, Clone)]
pub struct Panne {
    pub classe: String,
    pub message: String,
    pub code: i64,
}

impl Panne {
    pub fn est_api(&self) -> bool {
        self.classe == "ErreurApi" || self.classe == "DepotRefuse"
    }

    /// Une panne du RESEAU (injoignable, coupe, delai depasse) : le Python la laissait passer en plantage avec une trace
    /// (`URLError`, `RemoteDisconnected`, `TimeoutError`...). Les commandes la disent en une phrase.
    pub fn est_reseau(&self) -> bool {
        matches!(self.classe.as_str(), "URLError" | "TimeoutError" | "RemoteDisconnected" | "IncompleteRead" | "HTTPException")
    }

    /// La panne comme un refus de route : sa classe et sa phrase.
    pub fn en_echec(self) -> Echec {
        Echec { classe: Some(classe_connue(&self.classe)), message: self.message }
    }
}

/// La classe d'une panne, rendue en `&'static str` pour `Echec` ; une classe inconnue devient `OSError`.
fn classe_connue(c: &str) -> &'static str {
    match c {
        "ErreurApi" => "ErreurApi",
        "DepotRefuse" => "DepotRefuse",
        "ValueError" => "ValueError",
        "FileNotFoundError" => "FileNotFoundError",
        "PermissionError" => "PermissionError",
        "TimeoutError" => "TimeoutError",
        "URLError" => "URLError",
        "RemoteDisconnected" => "RemoteDisconnected",
        "IncompleteRead" => "IncompleteRead",
        "HTTPException" => "HTTPException",
        "UnicodeDecodeError" => "UnicodeDecodeError",
        "JSONDecodeError" => "JSONDecodeError",
        "AttributeError" => "AttributeError",
        "TypeError" => "TypeError",
        _ => "OSError",
    }
}

fn panne_de(v: &Value) -> Panne {
    Panne {
        classe: v.get("classe").and_then(Value::as_str).unwrap_or("OSError").to_string(),
        message: v.get("erreur").and_then(Value::as_str).unwrap_or("").to_string(),
        code: v.get("code").and_then(Value::as_i64).unwrap_or(0),
    }
}

/// `paie transport` : l'operation, ou la panne.
fn transport(demande: Value) -> Result<Value, Panne> {
    crate::transport::operation(&demande).map_err(|v| panne_de(&v))
}

// ----------------------------------------------------------------------- le client

pub const SERVICE_DSN_REGIME_GENERAL: &str = "25";
pub const SERVICE_DSN_REGIME_GENERAL_TEST: &str = "97";
pub const VARIABLE_MOTDEPASSE: &str = "NET_ENTREPRISES_MOTDEPASSE";
pub const VARIABLE_MOTDEPASSE_DPAE: &str = "URSSAF_MOTDEPASSE";

/// Les adresses d'un service (`netentreprises.ADRESSES`, `dpae_api.ADRESSES_DPAE`).
#[derive(Clone)]
pub struct Adresses {
    pub authentification: String,
    pub retours: String,
    pub retours_flux: String,
    pub depot: String,
    pub service: Option<String>,
}

fn adresses_dsn(environnement: &str) -> Result<Adresses, Panne> {
    // L'atelier seulement : un faux net-entreprises local (`atelier.rs`). Le service reste celui de l'environnement.
    #[cfg(feature = "atelier")]
    if let Some(b) = crate::atelier::base(crate::atelier::VARIABLE_DSN) {
        let service = if environnement == "editeur" { SERVICE_DSN_REGIME_GENERAL_TEST } else { SERVICE_DSN_REGIME_GENERAL };
        return Ok(Adresses {
            authentification: format!("{}/authentifier/", b),
            retours: format!("{}/lister-retours-declarant/1.0/", b),
            retours_flux: format!("{}/lister-retours-flux/1.0/", b),
            depot: format!("{}/deposer-dsn/1.0/", b),
            service: Some(service.into()),
        });
    }
    let (auth, hote_consultation, depot, service) = match environnement {
        "production" => (
            "https://services.net-entreprises.fr/authentifier/1.0/",
            "https://consultation.dsnrg.net-entreprises.fr",
            "https://depot.dsnrg.net-entreprises.fr/deposer-dsn/1.0/",
            SERVICE_DSN_REGIME_GENERAL,
        ),
        "editeur" => (
            "https://test-services.net-entreprises.fr/authentifier/1.0/",
            "https://consultation-edit.dsnrg.net-entreprises.fr",
            "https://depot-edit.dsnrg.net-entreprises.fr/deposer-dsn/1.0/",
            SERVICE_DSN_REGIME_GENERAL_TEST,
        ),
        autre => return Err(Panne { classe: "ValueError".into(), message: format!("environnement inconnu : '{}'", autre), code: 0 }),
    };
    Ok(Adresses {
        authentification: auth.into(),
        retours: format!("{}/lister-retours-declarant/1.0/", hote_consultation),
        retours_flux: format!("{}/lister-retours-flux/1.0/", hote_consultation),
        depot: depot.into(),
        service: Some(service.into()),
    })
}

fn adresses_dpae(ent: &Value) -> Adresses {
    let service = ent
        .get("api_dpae")
        .and_then(|c| c.get("service"))
        .filter(|s| vrai(s))
        .map(py_str);
    // L'atelier seulement : un faux Urssaf local (`atelier.rs`).
    #[cfg(feature = "atelier")]
    if let Some(b) = crate::atelier::base(crate::atelier::VARIABLE_DPAE) {
        return Adresses {
            authentification: format!("{}/authentifier_dpae", b),
            retours: format!("{}/lister-retours-declarant/1.0/", b),
            retours_flux: format!("{}/lister-retours-flux/1.0/", b),
            depot: format!("{}/deposer-dsn/1.0/", b),
            service,
        };
    }
    Adresses {
        authentification: "https://mon.urssaf.fr/authentifier_dpae".into(),
        retours: "https://consultation.dpae-edi.urssaf.fr/lister-retours-declarant/1.0/".into(),
        retours_flux: "https://consultation.dpae-edi.urssaf.fr/lister-retours-flux/1.0/".into(),
        depot: "https://depot.dpae-edi.urssaf.fr/deposer-dsn/1.0/".into(),
        service,
    }
}

/// `netentreprises.Client` et `dpae_api.ClientDpae`.
pub struct Client {
    pub adresses: Adresses,
    pub jeton: Option<String>,
    pub expire_le: Option<String>,
    pub declarant: Value,
    motdepasse: Option<String>,
    variable: &'static str,
    majuscules: bool,
}

impl Client {
    /// `Client(ctx, environnement, motdepasse=...)` : le compte net-entreprises de la DSN.
    pub fn dsn(ctx: &Contexte, environnement: &str, motdepasse: Option<String>) -> Result<Client, Panne> {
        let declarant = crate::transport::declarant(&ctx.entreprise).map_err(|v| panne_de(&v))?;
        Ok(Client {
            adresses: adresses_dsn(environnement)?,
            jeton: None,
            expire_le: None,
            declarant,
            motdepasse,
            variable: VARIABLE_MOTDEPASSE,
            majuscules: false,
        })
    }

    /// `ClientDpae(ctx, majuscules, motdepasse)` : le compte Urssaf, qui n'est pas celui de la DSN.
    pub fn dpae(ctx: &Contexte, majuscules: bool, motdepasse: Option<String>) -> Client {
        let cfg = ctx.entreprise.get("api_dpae").cloned().unwrap_or(Value::Null);
        let champ = |k: &str| cfg.get(k).cloned().unwrap_or(Value::Null);
        let siret = if vrai(&champ("siret")) { champ("siret") } else { ctx.entreprise.get("siret").cloned().unwrap_or(Value::Null) };
        Client {
            adresses: adresses_dpae(&ctx.entreprise),
            jeton: None,
            expire_le: None,
            declarant: json!({"siret": siret, "nom": champ("nom"), "prenom": champ("prenom")}),
            motdepasse,
            variable: VARIABLE_MOTDEPASSE_DPAE,
            majuscules,
        }
    }

    /// Le mot de passe a verifier avant l'envoi : celui qu'on a donne, sinon la variable d'environnement.
    fn secret(&self) -> Option<String> {
        self.motdepasse.clone().filter(|m| !m.is_empty()).or_else(|| std::env::var(self.variable).ok().filter(|m| !m.is_empty()))
    }

    /// `ClientDpae.verifier_identifiants` : ce qui ferait echouer la connexion MtoM. Le mot de passe ne
    /// quitte pas le client : on ne sait que s'il porte un `@`.
    pub fn verifier_identifiants(&self) -> Result<Vec<String>, Echec> {
        let arobase = self.secret().is_some_and(|m| m.contains('@'));
        crate::depot::identifiants(&self.declarant, arobase)
    }

    /// `Client.authentifier` : ouvre une session. Le mot de passe ne survit pas a cet appel.
    pub fn authentifier(&mut self) -> Result<(), Panne> {
        let mdp = self.secret().map(|m| if self.majuscules { m.to_uppercase() } else { m });
        let rep = transport(json!({
            "operation": "authentifier",
            "url": self.adresses.authentification,
            "service": self.adresses.service,
            "declarant": self.declarant,
            "motdepasse": mdp,
            "variable": self.variable,
            "jeton": self.jeton,
        }))?;
        self.jeton = rep.get("jeton").and_then(Value::as_str).map(str::to_string);
        self.expire_le = rep.get("expire_le").and_then(Value::as_str).map(str::to_string);
        Ok(())
    }

    pub fn assurer_session(&mut self) -> Result<(), Panne> {
        // `if not self.jeton` : un jeton vide n'ouvre pas de session.
        if self.jeton.as_deref().map_or(true, str::is_empty) {
            self.authentifier()?;
        }
        Ok(())
    }

    fn appel(&self, url: &str) -> Result<(Vec<u8>, Value), Panne> {
        let rep = transport(json!({"operation": "appel", "url": url, "jeton": self.jeton, "donnees": Value::Null, "content_type": Value::Null}))?;
        let corps = base64_lire(rep.get("corps").and_then(Value::as_str).unwrap_or("")).map_err(|m| Panne { classe: "ValueError".into(), message: m, code: 0 })?;
        Ok((corps, rep.get("entetes").cloned().unwrap_or(Value::Null)))
    }

    /// `Client._consulter` : l'adresse de base et la plage (heures locales naives).
    fn consulter(&mut self, base: &str, debut: i64, fin: Option<i64>) -> Result<Vec<u8>, Panne> {
        self.assurer_session()?;
        let rep = transport(json!({
            "operation": "consulter", "base": base, "jeton": self.jeton,
            "debut": {"iso": iso_de_naif(debut)},
            "fin": fin.map(|f| json!({"iso": iso_de_naif(f)})),
        }))?;
        base64_lire(rep.get("corps").and_then(Value::as_str).unwrap_or("")).map_err(|m| Panne { classe: "ValueError".into(), message: m, code: 0 })
    }

    pub fn lister_retours(&mut self, debut: i64, fin: i64) -> Result<Vec<Value>, Panne> {
        let base = self.adresses.retours.clone();
        let brut = self.consulter(&base, debut, Some(fin))?;
        crate::retours::liste_retours_d_octets(&brut).map_err(|e| Panne { classe: e.classe.unwrap_or("ValueError").into(), message: e.message, code: 0 })
    }

    /// `Client.lister_depots` : les depots que net-entreprises a vus sur une plage. L'adresse est celle de la consultation
    /// des retours, dont seul le service change (`lister-depots/1.0/` au lieu de `lister-retours-declarant/1.0/`).
    pub fn lister_depots(&mut self, debut: i64, fin: i64) -> Result<Vec<Value>, Panne> {
        let base = self.adresses.retours.replace("lister-retours-declarant", "lister-depots");
        let brut = self.consulter(&base, debut, Some(fin))?;
        crate::retours::liste_depots_d_octets(&brut).map_err(|e| Panne { classe: e.classe.unwrap_or("ValueError").into(), message: e.message, code: 0 })
    }

    pub fn lister_retours_du_flux(&mut self, idflux: &str) -> Result<Vec<Value>, Panne> {
        self.assurer_session()?;
        let (brut, _) = self.appel(&format!("{}{}", self.adresses.retours_flux, idflux))?;
        crate::retours::liste_retours_d_octets(&brut).map_err(|e| Panne { classe: e.classe.unwrap_or("ValueError").into(), message: e.message, code: 0 })
    }

    /// Recupere un retour : (contenu brut, type de contenu).
    pub fn telecharger(&mut self, url: &str) -> Result<(Vec<u8>, String), Panne> {
        self.assurer_session()?;
        let (brut, entetes) = self.appel(url)?;
        let ctype = entetes.get("Content-Type").and_then(Value::as_str).unwrap_or("").to_string();
        Ok((brut, ctype))
    }

    /// L'envoi du guide (text/plain, gzip, jeton DSNLogin) : le moteur `paie transport` envoie les octets
    /// deja prepares. `forme` : `dsn` (`DepotRefuse`) ou `dpae` (`ErreurApi`).
    pub fn deposer_octets(&mut self, url: &str, contenu: &[u8], forme: &str) -> Result<(String, String), Panne> {
        self.assurer_session()?;
        let corps = gzip(contenu)?;
        let rep = transport(json!({"operation": "depot", "url": url, "jeton": self.jeton, "forme": forme, "corps": base64_ecrire(&corps)}))?;
        let brut = base64_lire(rep.get("corps").and_then(Value::as_str).unwrap_or("")).map_err(|m| Panne { classe: "ValueError".into(), message: m, code: 0 })?;
        let texte = String::from_utf8_lossy(&brut).trim_matches(crate::retours::blanc_py).to_string();
        let ctype = rep.get("entetes").and_then(|e| e.get("Content-Type")).and_then(Value::as_str).unwrap_or("").to_string();
        Ok((texte, ctype))
    }
}

/// `gzip.compress`.
fn gzip(donnees: &[u8]) -> Result<Vec<u8>, Panne> {
    use std::io::Write;
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(donnees).map_err(|x| Panne { classe: "OSError".into(), message: x.to_string(), code: 0 })?;
    e.finish().map_err(|x| Panne { classe: "OSError".into(), message: x.to_string(), code: 0 })
}

// -------------------------------------------------------------------- le coffre

/// Le mot de passe d'un service, pour une CONSULTATION : l'environnement d'abord, puis le coffre ; `None`
/// quand rien n'est pose. Un coffre qui ne s'ouvre pas est un refus, pas une absence.
pub fn mot_de_passe_de_consultation(ctx: &Contexte, service: &str) -> R<Option<String>> {
    let coffre = crate::coffre::Coffre::du_poste(&ctx.racine, "");
    coffre.lire(&json!(service), &json!(crate::coffre::CONSULTATION), false, None).map_err(crate::routes_etats::vers_route)
}

// ------------------------------------------------------------------ la releve

/// Un retour telecharge et archive.
pub struct RetourArchive {
    pub retour: Value,
    pub chemin: PathBuf,
    pub contenu: Vec<u8>,
}

/// `ReleveRetours`.
#[derive(Default)]
pub struct Releve {
    pub trouves: Vec<Value>,
    pub nouveaux: Vec<Value>,
    pub archives: Vec<RetourArchive>,
    pub incidents: Vec<String>,
    pub session_jusqu_a: String,
    /// Combien de ces retours ont trouve le depot auquel ils repondent.
    pub rattaches: i64,
    /// Ce qu'un retour recu demande de faire (un taux AT/MP a reporter).
    pub a_faire: Vec<String>,
}

fn cle_de_retour(r: &Value) -> [Value; 3] {
    ["flux", "nature", "publication"].map(|c| r.get(c).cloned().unwrap_or(Value::Null))
}

fn date_publication(r: &Value) -> String {
    let p = r.get("publication").filter(|v| !v.is_null()).map(py_str).unwrap_or_default();
    let c: Vec<char> = p.chars().collect();
    if c.len() >= 8 {
        format!("{}{}/{}{}/{}", c[6], c[7], c[4], c[5], c[..4].iter().collect::<String>())
    } else {
        "?".into()
    }
}

/// `archiver` : la piece telle quelle, nommee `AAAAMMJJ-hhmmss_natureN_flux.ext`.
fn archiver(ctx: &Contexte, retour: &Value, contenu: &[u8], ctype: &str, canal: &str) -> Result<PathBuf, Panne> {
    let rep = transport(json!({
        "operation": "archiver", "racine": ctx.racine.to_string_lossy(), "retour": retour, "canal": canal,
        "contenu": base64_ecrire(contenu), "content_type": ctype, "maintenant": horodatage_archive(maintenant_naif()),
    }))?;
    Ok(PathBuf::from(rep.get("chemin").and_then(Value::as_str).unwrap_or("")))
}

/// `netentreprises.noter` : la liste de ce qui a ete vu.
fn noter_vus(ctx: &Contexte, entrees: &[Value], canal: &str) -> Result<(), Panne> {
    transport(json!({"operation": "noter", "racine": ctx.racine.to_string_lossy(), "entrees": entrees, "canal": canal})).map(|_| ())
}

fn deja_vus(ctx: &Contexte, canal: &str) -> Result<Vec<[Value; 3]>, Panne> {
    let rep = transport(json!({"operation": "deja_vus", "racine": ctx.racine.to_string_lossy(), "canal": canal}))?;
    Ok(rep
        .get("vus")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|k| {
                    let c = k.as_array().cloned().unwrap_or_default();
                    [c.first().cloned().unwrap_or(Value::Null), c.get(1).cloned().unwrap_or(Value::Null), c.get(2).cloned().unwrap_or(Value::Null)]
                })
                .collect()
        })
        .unwrap_or_default())
}

fn egal_cle(a: &[Value; 3], b: &[Value; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| crate::etats::eq_py(x, y))
}

/// La liste est-elle deja vue ? Les trois champs, comme `(flux, nature, publication) in connus`.
fn est_connu(connus: &[[Value; 3]], r: &Value) -> bool {
    let k = cle_de_retour(r);
    connus.iter().any(|c| egal_cle(c, &k))
}

fn en_echec(p: Panne) -> Echec {
    p.en_echec()
}

/// `_tranches` : le SI DSN limite l'etendue d'une recherche, on decoupe par jour.
fn tranches(debut: i64, fin: i64) -> Vec<(i64, i64)> {
    let mut v = Vec::new();
    let mut t = debut;
    while t < fin {
        v.push((t, (t + 86_400).min(fin)));
        t += 86_400;
    }
    v
}

/// `simulation.rattacher` : pose les retours des depots joues qui les attendent depuis `delai` secondes.
/// Rend le nombre de depots completes. Ne touche a aucune autre ligne : un vrai depot (flux sans le
/// prefixe) garde ses vrais retours.
pub fn rattacher_simulation(ctx: &Contexte, delai: i64) -> R<i64> {
    let p = crate::journal::chemin(&ctx.racine);
    if !p.exists() {
        return Ok(0);
    }
    let mut lignes = crate::etats::charger(&ctx.racine)?;
    let maintenant = maintenant_naif();
    let pub_ = horodatage_archive(maintenant).replace('-', "");
    let mut faits = 0;
    for l in lignes.iter_mut() {
        let quoi = l.get("quoi").and_then(Value::as_str).unwrap_or("");
        let flux = l.get("flux").map(|f| if vrai(f) { py_str(f) } else { String::new() }).unwrap_or_default();
        let age = l.get("quand").and_then(Value::as_str).and_then(naif_de_iso).map_or(0, |t| maintenant - t);
        if (quoi == "dsn" || quoi == "dpae")
            && l.get("accepte").is_some_and(vrai)
            && flux.starts_with("DEMO-")
            && !l.get("retours").is_some_and(vrai)
            && age >= delai
        {
            let base = |nature: &str, verdict: Option<&str>, libelle: &str| {
                let mut m = Map::new();
                m.insert("statut".into(), json!("demonstration"));
                m.insert("publication".into(), json!(pub_));
                m.insert("anomalies".into(), json!([]));
                m.insert("nature".into(), json!(nature));
                if let Some(v) = verdict {
                    m.insert("verdict".into(), json!(v));
                }
                m.insert("libelle".into(), json!(libelle));
                m
            };
            let mut retours: Vec<Value> = Vec::new();
            if quoi == "dpae" {
                retours.push(Value::Object(base("11", Some("OK"), "Bilan de conformite (demonstration)")));
                let mut m = base("41", Some("OK"), "Certificat de conformite des DPAE (demonstration)");
                m.insert("dpae".into(), json!([]));
                retours.push(Value::Object(m));
            } else {
                retours.push(Value::Object(base("10", Some("OK"), "Accuse de reception (demonstration)")));
                retours.push(Value::Object(base("11", Some("OK"), "Bilan de conformite (demonstration)")));
                if l.get("mode").and_then(Value::as_str) == Some("reel") {
                    retours.push(Value::Object(base("46", None, "Attestation employeur rematerialisee (demonstration)")));
                }
            }
            l["retours"] = Value::Array(retours);
            faits += 1;
        }
    }
    if faits > 0 {
        crate::ecriture::ecrire_json(&p, &Value::Array(lignes), 2).map_err(|m| erreur("OSError", m))?;
    }
    Ok(faits)
}

/// `operations_retours.relever_retours` : telecharge et archive ce que les organismes ont publie. `debut`
/// et `fin` : heures locales naives (`maintenant_naif`). `client` : un client deja construit (le canal DPAE
/// passe le sien, avec `canal = "dpae"`).
pub fn relever_retours(
    ctx: &Contexte,
    o: &HashMap<String, String>,
    debut: i64,
    fin: i64,
    tout: bool,
    environnement: &str,
    client: Option<Client>,
    canal: &str,
) -> R<Releve> {
    let mut releve = Releve::default();
    // LA DEMONSTRATION : les retours des depots joues arrivent d'ici, sans reseau ni mot de passe.
    if client.is_none() && crate::routes_accueil::simulation_active(&ctx.racine) {
        releve.rattaches = rattacher_simulation(ctx, 6)?;
        return Ok(releve);
    }
    let mut client = match client {
        Some(c) => c,
        None => {
            let Some(mdp) = mot_de_passe_de_consultation(ctx, "net-entreprises")? else {
                releve.incidents.push(
                    "Mot de passe net-entreprises absent : ni dans l'environnement, ni dans le coffre. `paie coffre poser net-entreprises`.".into(),
                );
                return Ok(releve);
            };
            Client::dsn(ctx, environnement, Some(mdp)).map_err(en_echec)?
        }
    };
    match client.authentifier() {
        Ok(()) => releve.session_jusqu_a = client.expire_le.clone().unwrap_or_default(),
        Err(p) if p.est_api() => {
            releve.incidents.push(format!("Authentification refusee : {}", p.message));
            return Ok(releve);
        }
        Err(p) => return Err(en_echec(p)),
    }
    let connus = deja_vus(ctx, canal).map_err(en_echec)?;
    // UN 404 SUR UNE TRANCHE VEUT DIRE << RIEN SUR CETTE TRANCHE >> ; tout autre code arrete : un 401 ne
    // s'ameliorera pas au jour suivant.
    for (t0, t1) in tranches(debut, fin) {
        match client.lister_retours(t0, t1) {
            Ok(trouves) => {
                for r in trouves {
                    if !est_connu(&connus, &r) {
                        releve.nouveaux.push(r.clone());
                    }
                    releve.trouves.push(r);
                }
            }
            Err(p) if p.est_api() => {
                if p.code == 404 {
                    continue;
                }
                releve.incidents.push(format!("Consultation interrompue : {}", p.message));
                break;
            }
            Err(p) => return Err(en_echec(p)),
        }
    }
    if !releve.incidents.is_empty() && releve.trouves.is_empty() {
        return Ok(releve);
    }

    // UN RETOUR RATE N'EST PAS UN RETOUR VU (23/09/2026) : un echec reste NEUF, et se retente au releve suivant.
    let mut rates: Vec<Value> = Vec::new();
    let a_telecharger: Vec<Value> = if tout { releve.trouves.clone() } else { releve.nouveaux.clone() };
    for r in a_telecharger {
        let Some(url) = r.get("url").filter(|u| vrai(u)).map(py_str) else { continue };
        match client.telecharger(&url) {
            Ok((contenu, ctype)) => {
                let chemin = archiver(ctx, &r, &contenu, &ctype, canal).map_err(en_echec)?;
                if canal.is_empty() && r.get("nature").map(py_str).as_deref() == Some(crate::retours::NATURE_TAUX_ATMP) {
                    releve.a_faire.push(format!(
                        "Notification du taux AT/MP recue le {} (retour de nature 34, archive {}) : le taux se reporte dans config/baremes/<annee>.json (`taux_atmp`), avec sa source.",
                        date_publication(&r),
                        chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
                    ));
                }
                // UN SIGNALEMENT REFUSE B1-208-10 (lot O1) : l'ordre etait deja pris ; la conduite a tenir se dit.
                if canal.is_empty() && r.get("nature").map(py_str).as_deref() == Some("11") {
                    let flux = r.get("flux").filter(|f| vrai(f)).map(py_str).unwrap_or_default();
                    for d in crate::retours::declarations_du_bilan(&contenu) {
                        releve.a_faire.extend(crate::ordre_signalements::conduite_b1_208_10(&d, &flux));
                    }
                }
                releve.archives.push(RetourArchive { retour: r, chemin, contenu });
            }
            Err(p) if p.est_api() => {
                releve.incidents.push(format!("Telechargement du retour nature {} : {}", r.get("nature").map(py_str).unwrap_or_else(|| "None".into()), p.message));
                rates.push(r);
            }
            Err(p) => return Err(en_echec(p)),
        }
    }

    if canal.is_empty() {
        // L'EFFECTIF S'ECRIT SEUL (23/09/2026) : `config/effectifs.json` se refait depuis les archives.
        reconstruire_effectifs(&ctx.racine)?;
    }

    if !releve.trouves.is_empty() {
        let vus: Vec<Value> = releve.trouves.iter().filter(|r| !rates.contains(r)).cloned().collect();
        noter_vus(ctx, &vus, canal).map_err(en_echec)?;
        // Un depot n'est fini que quand sa reponse est la : chaque retour se range en face du depot qui l'a
        // provoque -- par le flux, et par rien d'autre.
        let contenus: Vec<([Value; 3], Vec<u8>)> = releve.archives.iter().map(|a| (cle_de_retour(&a.retour), a.contenu.clone())).collect();
        releve.rattaches = crate::etats::rattacher_retours_avec_contenus(&ctx.racine, &Value::Array(releve.trouves.clone()), &contenus)?;
    }

    // L'AER SE RANGE DES QU'ELLE ARRIVE (01/10/2026) : la boite d'envoi la voit.
    let aer: Vec<PathBuf> = releve
        .archives
        .iter()
        .filter(|a| a.retour.get("nature").map(py_str).as_deref() == Some(crate::retours::NATURE_AER_SPECTACLE))
        .map(|a| a.chemin.clone())
        .collect();
    if !aer.is_empty() && canal.is_empty() {
        match crate::routes_documents::ranger_aer(o, ctx, None, Some(&aer), true) {
            Ok(a_faire) => releve.a_faire.extend(a_faire),
            Err(e) => releve.incidents.push(format!("Attestations France Travail recues mais non rangees : {}", e.message)),
        }
    }
    Ok(releve)
}

/// `operations_retours.relever_retours_dpae` : les retours de l'API DPAE -- UN AUTRE SERVICE que celui de
/// la DSN, ses archives sont dans `retours/dpae/`. `flux` : le seul chemin documente (la FAQ) quand on
/// connait l'idflux ; sinon la consultation par plage, sur le canal `dpae`.
pub fn relever_retours_dpae(ctx: &Contexte, o: &HashMap<String, String>, debut: i64, fin: i64, tout: bool, flux: &str) -> R<Releve> {
    if crate::routes_accueil::simulation_active(&ctx.racine) {
        let mut r = Releve::default();
        r.rattaches = rattacher_simulation(ctx, 6)?;
        return Ok(r);
    }
    let Some(mdp) = mot_de_passe_de_consultation(ctx, "urssaf")? else {
        let mut r = Releve::default();
        r.incidents.push("Mot de passe DPAE absent : ni dans l'environnement, ni dans le coffre. `paie coffre poser urssaf`.".into());
        return Ok(r);
    };
    let mut client = Client::dpae(ctx, false, Some(mdp));
    if flux.is_empty() {
        return relever_retours(ctx, o, debut, fin, tout, "production", Some(client), "dpae");
    }
    let mut r = Releve::default();
    let trouves = client.authentifier().and_then(|_| client.lister_retours_du_flux(flux));
    match trouves {
        Ok(t) => r.trouves = t,
        Err(p) if p.est_api() => {
            r.incidents.push(format!("Consultation du flux {} : {}", flux, p.message));
            return Ok(r);
        }
        Err(p) => return Err(en_echec(p)),
    }
    for x in r.trouves.clone() {
        let Some(url) = x.get("url").filter(|u| vrai(u)).map(py_str) else { continue };
        match client.telecharger(&url) {
            Ok((contenu, ctype)) => {
                let chemin = archiver(ctx, &x, &contenu, &ctype, "dpae").map_err(en_echec)?;
                r.archives.push(RetourArchive { retour: x, chemin, contenu });
            }
            Err(p) if p.est_api() => r.incidents.push(format!("Telechargement nature {} : {}", x.get("nature").map(py_str).unwrap_or_else(|| "None".into()), p.message)),
            Err(p) => return Err(en_echec(p)),
        }
    }
    if !r.trouves.is_empty() {
        noter_vus(ctx, &r.trouves, "dpae").map_err(en_echec)?;
        // Le chemin par flux n'accrochait pas ce qu'il trouvait au journal (vu le 12/09/2026).
        let contenus: Vec<([Value; 3], Vec<u8>)> = r.archives.iter().map(|a| (cle_de_retour(&a.retour), a.contenu.clone())).collect();
        r.rattaches = crate::etats::rattacher_retours_avec_contenus(&ctx.racine, &Value::Array(r.trouves.clone()), &contenus)?;
    }
    Ok(r)
}

/// La derniere journee, comme `suivre_le_depot` et `bilan_de_la_repetition` la relevent : de hier a maintenant.
pub fn derniere_journee() -> (i64, i64) {
    let fin = maintenant_naif();
    (fin - 86_400, fin)
}

// ------------------------------------------------------------------- l'effectif

/// `effectif.lire_crm118` : les effectifs d'un retour de nature 118 (code, valeur, periode, calcul) ; un XML
/// qui ne se lit pas ne dit rien.
fn lire_crm118(contenu: &[u8]) -> R<Option<Vec<(String, f64, String, String, String)>>> {
    let Ok(texte) = std::str::from_utf8(contenu) else { return Ok(None) };
    let Ok(doc) = roxmltree::Document::parse(texte) else { return Ok(None) };
    let mut out = Vec::new();
    for el in doc.descendants().filter(|n| n.is_element() && n.tag_name().name() == "RO_B008") {
        let enfant = |nom: &str| -> String {
            el.children().find(|c| c.is_element() && c.tag_name().name() == nom).map(|c| c.text().unwrap_or("").trim().to_string()).unwrap_or_default()
        };
        let (code, valeur) = (enfant("RO_B008_R001"), enfant("RO_B008_R002"));
        if code.is_empty() || valeur.is_empty() {
            continue;
        }
        let v: f64 = valeur.parse().map_err(|_| erreur("ValueError", format!("nombre illisible : '{}'", valeur)))?;
        out.push((code, v, enfant("RO_B008_R003"), enfant("RO_B008_R004"), enfant("RO_B008_R006")));
    }
    Ok(Some(out))
}

/// `effectif.reconstruire` : relit tous les 118 archives et reecrit `config/effectifs.json`. Pour chaque annee
/// et chaque code, le calcul le plus RECENT l'emporte. Ne touche pas au fichier s'il n'y a aucun 118.
pub fn reconstruire_effectifs(racine: &Path) -> R<()> {
    let dossier = racine.join("retours");
    let mut noms: Vec<String> = match std::fs::read_dir(&dossier) {
        Ok(l) => l
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains("_nature118_") && n.ends_with(".xml"))
            .collect(),
        Err(_) => Vec::new(),
    };
    noms.sort();
    let mut annees: std::collections::BTreeMap<String, Map<String, Value>> = std::collections::BTreeMap::new();
    for nom in &noms {
        let p = dossier.join(nom);
        let octets = std::fs::read(&p).map_err(|e| erreur("OSError", format!("{} : {}", p.display(), e)))?;
        let Some(mesures) = lire_crm118(&octets)? else { continue };
        for (code, valeur, debut, fin, calcule_le) in mesures {
            let ref_an = if debut.is_empty() { &fin } else { &debut };
            let an: String = ref_an.chars().take(4).collect();
            if an.is_empty() || !an.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let codes = annees.entry(an).or_default();
            if let Some(deja) = codes.get(&code) {
                if deja.get("calcule_le").and_then(Value::as_str).unwrap_or("") > calcule_le.as_str() {
                    continue;
                }
            }
            codes.insert(code, json!({"valeur": valeur, "calcule_le": calcule_le, "source": format!("retours/{}", nom)}));
        }
    }
    let chemin = racine.join("config").join("effectifs.json");
    let existant: Value = if chemin.exists() {
        std::fs::read_to_string(&chemin).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| json!({}))
    } else {
        json!({})
    };
    if annees.is_empty() {
        return Ok(());
    }
    let mut donnees = Map::new();
    donnees.insert(
        "_quoi".into(),
        json!("Effectifs calcules par l'Urssaf et renvoyes en CRM de nature 118. `annees` est ECRIT PAR LE MOTEUR a chaque releve des retours, depuis les archives de retours/ : ne pas le modifier a la main. Seule `jamais_au_dessus_de` s'ecrit a la main -- la declaration de l'employeur pour les annees que les retours ne servent plus -- et le releve la garde. A01 = effectif de securite sociale (art. L. 130-1 CSS), celui des seuils de cotisation."),
    );
    donnees.insert("annees".into(), Value::Object(annees.into_iter().map(|(an, codes)| (an, Value::Object(codes))).collect()));
    // La declaration de l'employeur pour le passe n'est pas un retour : le releve ne l'efface pas.
    if let Some(passe) = existant.get("jamais_au_dessus_de").filter(|p| vrai(p)) {
        donnees.insert("jamais_au_dessus_de".into(), passe.clone());
    }
    let donnees = Value::Object(donnees);
    if existant != donnees {
        crate::ecriture::ecrire_json(&chemin, &donnees, 2).map_err(|m| erreur("OSError", m))?;
    }
    Ok(())
}

/// Le resume d'une relève, pour une phrase d'incident : `"; ".join(rel.incidents)`.
pub fn incidents_en_phrase(r: &Releve) -> String {
    r.incidents.join("; ")
}
