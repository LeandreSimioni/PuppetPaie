//! La releve des retours des organismes et le client qui leur parle (net-entreprises pour la DSN, l'Urssaf pour la DPAE).
//!
//! RELEVER EST UNE LECTURE : on archive sans rien demander a personne, aucune seance n'est exigee, rien
//! n'est applique. L'archive brute est la piece justificative : elle est ecrite avant toute analyse. Un 404
//! sur une tranche veut dire << rien sur cette tranche >>, tout autre refus arrete (CLAUDE.md, regle 5).
//!
//! CE MODULE NE CHERCHE JAMAIS UN SECRET (regle 3) : `Client` recoit le mot de passe, que `moteur` prend a
//! la variable d'environnement du service ou au coffre. Le mot de passe sert le temps de l'authentification
//! (`transport::authentifier`), n'est ni journalise, ni affiche, ni renvoye.
//!
//! La releve obeit au branchement : le simulateur pose les retours des depots joues, sans reseau (`rattacher_simulation`) ;
//! le message ne releve rien. Ce qu'elle trouve en chemin et qui touche a la compagnie (les incidents des retours, les
//! attestations France Travail a ranger) va dans les suites (`suites::Suites`).

use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use socle::valeur::valeur_en_texte;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::vrai;
use socle::echec::Echec;
use socle::paris::{iso_de_naif, maintenant_naif, naif_de_iso};

use crate::branchement::{Branchement, ANCIEN_PREFIXE_FLUX_FICTIF, PREFIXE_FLUX_FICTIF};
use crate::suites::Suites;

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

    /// Une panne du RESEAU (injoignable, coupe, delai depasse : `URLError`, `RemoteDisconnected`, `TimeoutError`...).
    /// Les commandes la disent en une phrase.
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

/// La panne que le transport rend (`classe`, `erreur`, `code`).
fn panne_de(v: Value) -> Panne {
    Panne {
        classe: v.get("classe").and_then(Value::as_str).unwrap_or("OSError").to_string(),
        message: v.get("erreur").and_then(Value::as_str).unwrap_or("").to_string(),
        code: v.get("code").and_then(Value::as_i64).unwrap_or(0),
    }
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
    // L'atelier seulement : un faux net-entreprises local (`redirection`). Le service reste celui de l'environnement.
    #[cfg(feature = "atelier")]
    if let Some(b) = crate::redirection::base(crate::redirection::VARIABLE_DSN) {
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
        .map(valeur_en_texte);
    // L'atelier seulement : un faux Urssaf local (`redirection`).
    #[cfg(feature = "atelier")]
    if let Some(b) = crate::redirection::base(crate::redirection::VARIABLE_DPAE) {
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
        let declarant = crate::transport::declarant(&ctx.entreprise).map_err(panne_de)?;
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

    /// Le mot de passe que l'appelant a donne (le moteur le prend a l'environnement ou au coffre) : le client n'en
    /// cherche aucun.
    fn secret(&self) -> Option<String> {
        self.motdepasse.clone().filter(|m| !m.is_empty())
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
        let rep = crate::transport::authentifier(&crate::transport::Authentification {
            url: &self.adresses.authentification,
            declarant: &self.declarant,
            motdepasse: mdp.as_deref(),
            variable: self.variable,
            service: self.adresses.service.as_deref(),
            jeton: self.jeton.as_deref(),
        })
        .map_err(panne_de)?;
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

    fn appel(&self, url: &str) -> Result<(Vec<u8>, crate::transport::Entetes), Panne> {
        crate::transport::appeler(url, self.jeton.as_deref()).map_err(panne_de)
    }

    /// La consultation d'une plage (heures locales naives).
    fn consulter(&mut self, base: &str, debut: i64, fin: Option<i64>) -> Result<Vec<u8>, Panne> {
        self.assurer_session()?;
        let fin = fin.map(iso_de_naif);
        crate::transport::consulter(base, self.jeton.as_deref(), &iso_de_naif(debut), fin.as_deref()).map_err(panne_de)
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
        let ctype = crate::transport::entete(&entetes, "Content-Type").unwrap_or("").to_string();
        Ok((brut, ctype))
    }

    /// L'envoi du guide (text/plain, gzip, jeton DSNLogin) des octets deja prepares. `forme` : `dsn` (`DepotRefuse`)
    /// ou `dpae` (`ErreurApi`).
    pub fn deposer_octets(&mut self, url: &str, contenu: &[u8], forme: &str) -> Result<(String, String), Panne> {
        self.assurer_session()?;
        let corps = gzip(contenu)?;
        let jeton = self.jeton.clone().unwrap_or_default();
        let (brut, entetes) = crate::transport::depot(url, &jeton, &corps, forme, crate::transport::DELAI_DEPOT).map_err(panne_de)?;
        let texte = String::from_utf8_lossy(&brut).trim_matches(socle::texte::blanc).to_string();
        let ctype = crate::transport::entete(&entetes, "Content-Type").unwrap_or("").to_string();
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

/// La date de publication d'un retour (`AAAAMMJJ…`), en JJ/MM/AAAA ; « ? » quand il ne la dit pas.
pub fn date_publication(r: &Value) -> String {
    let p = r.get("publication").filter(|v| !v.is_null()).map(valeur_en_texte).unwrap_or_default();
    let c: Vec<char> = p.chars().collect();
    if c.len() >= 8 {
        format!("{}{}/{}{}/{}", c[6], c[7], c[4], c[5], c[..4].iter().collect::<String>())
    } else {
        "?".into()
    }
}

/// La piece telle quelle, nommee `AAAAMMJJ-hhmmss_natureN_flux.ext`.
fn archiver(ctx: &Contexte, retour: &Value, contenu: &[u8], ctype: &str, canal: &str) -> Result<PathBuf, Panne> {
    crate::transport::archiver(&ctx.racine, canal, retour, contenu, ctype, &horodatage_archive(maintenant_naif())).map_err(panne_de)
}

/// Ce qui a ete vu, au journal du canal.
fn noter_vus(ctx: &Contexte, entrees: &[Value], canal: &str) -> Result<(), Panne> {
    crate::transport::noter_vus(&ctx.racine, canal, entrees).map_err(panne_de)
}

fn deja_vus(ctx: &Contexte, canal: &str) -> Result<Vec<[Value; 3]>, Panne> {
    crate::transport::deja_vus(&ctx.racine, canal).map_err(panne_de)
}

fn egal_cle(a: &[Value; 3], b: &[Value; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| socle::valeur::valeurs_egales(x, y))
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

/// LE SIMULATEUR : pose les retours des depots joues qui les attendent depuis `delai` secondes, et rend le nombre de
/// depots completes. Ne touche a aucune autre ligne : un vrai depot (flux sans le prefixe) garde ses vrais retours. Les
/// attestations France Travail jouees s'archivent dans `retours/` comme les vraies, et vont dans les suites, a ranger.
pub fn rattacher_simulation(ctx: &Contexte, delai: i64, suites: &mut Suites) -> R<i64> {
    let p = crate::journal::chemin(&ctx.racine);
    if !p.exists() {
        return Ok(0);
    }
    let mut a_archiver: Vec<(Value, Vec<u8>)> = Vec::new();
    // Ce que les organismes rendent de plus (refus, taux du PAS, effectif) : archive puis range par le meme chemin
    // qu'un retour reel (`rattacher_retours_avec_contenus`), qui en lit le verdict.
    let mut pieces: Vec<crate::organismes_simules::Piece> = Vec::new();
    let mut lignes = crate::etats::charger(&ctx.racine)?;
    let avant = crate::organismes_simules::memoire(&ctx.racine)?;
    let mut memoire = avant.clone();
    let maintenant = maintenant_naif();
    let pub_ = horodatage_archive(maintenant).replace('-', "");
    let mut faits = 0;
    for i in 0..lignes.len() {
        let l = &lignes[i];
        let quoi = l.get("quoi").and_then(Value::as_str).unwrap_or("");
        let flux = l.get("flux").map(|f| if vrai(f) { valeur_en_texte(f) } else { String::new() }).unwrap_or_default();
        let age = l.get("quand").and_then(Value::as_str).and_then(naif_de_iso).map_or(0, |t| maintenant - t);
        if (quoi == "dsn" || quoi == "dpae")
            && l.get("accepte").is_some_and(vrai)
            && (flux.starts_with(PREFIXE_FLUX_FICTIF) || flux.starts_with(ANCIEN_PREFIXE_FLUX_FICTIF))
            && !l.get("retours").is_some_and(vrai)
            && age >= delai
        {
            let base = |nature: &str, verdict: Option<&str>, libelle: &str| {
                let mut m = Map::new();
                m.insert("statut".into(), json!("fictive"));
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
                retours.push(Value::Object(base("11", Some("OK"), "Bilan de conformite (compagnie fictive)")));
                let mut m = base("41", Some("OK"), "Certificat de conformite des DPAE (compagnie fictive)");
                m.insert("dpae".into(), json!([]));
                retours.push(Value::Object(m));
            } else {
                match crate::organismes_simules::reponses_dsn(ctx, &mut memoire, &lignes, l, &flux, &pub_)? {
                    // Refusee : ses deux bilans, rien d'autre ; le verdict se lit dans la piece.
                    Err(bilans) => {
                        pieces.extend(bilans);
                        faits += 1;
                        continue;
                    }
                    Ok(autres) => pieces.extend(autres),
                }
                retours.push(Value::Object(base("10", Some("OK"), "Accuse de reception (compagnie fictive)")));
                retours.push(Value::Object(base("11", Some("OK"), "Bilan de conformite (compagnie fictive)")));
                // L'AER : seulement quand elle est due, et avec sa piece, comme France Travail la publie.
                if let Some(archive) = aer_jouee(ctx, l)? {
                    let r46 = base("46", None, "Attestation employeur rematerialisee (compagnie fictive)");
                    let mut a = r46.clone();
                    a.insert("flux".into(), json!(flux));
                    a_archiver.push((Value::Object(a), archive));
                    retours.push(Value::Object(r46));
                }
            }
            lignes[i]["retours"] = Value::Array(retours);
            faits += 1;
        }
    }
    if faits > 0 {
        socle::ecriture::ecrire_json(&p, &Value::Array(lignes), 2).map_err(|m| erreur("OSError", m))?;
    }
    crate::organismes_simules::garder(&ctx.racine, &avant, &memoire)?;
    // LE MEME CHEMIN QU'UNE VRAIE : l'archive dans `retours/`, puis le rangement (par `moteur`), qui signe le cadre 5 et la
    // pose dans `sorties/<mois>/envoi/`, ou l'envoi va la chercher.
    for (retour, archive) in &a_archiver {
        suites.aer.push(archiver(ctx, retour, archive, "application/zip", "").map_err(en_echec)?);
    }
    if !pieces.is_empty() {
        let mut contenus: Vec<([Value; 3], Vec<u8>)> = Vec::new();
        for x in &pieces {
            let chemin = archiver(ctx, &x.retour, &x.contenu, "application/xml", "").map_err(en_echec)?;
            if x.retour.get("nature").and_then(Value::as_str) == Some(crate::retours::NATURE_TAUX_PAS) {
                suites.taux_pas.push((x.retour.clone(), chemin));
            } else if x.retour.get("nature").and_then(Value::as_str) == Some(crate::retours::NATURE_TAUX_ATMP) {
                suites.taux_atmp.push((x.retour.clone(), chemin));
            }
            contenus.push((cle_de_retour(&x.retour), x.contenu.clone()));
        }
        let decrits: Vec<Value> = pieces.into_iter().map(|x| x.retour).collect();
        suites.garder(crate::etats::rattacher_retours_avec_contenus(&ctx.racine, &Value::Array(decrits), &contenus))?;
        // L'EFFECTIF S'ECRIT SEUL, comme a la releve reelle.
        reconstruire_effectifs(&ctx.racine)?;
    }
    Ok(faits)
}

/// L'AER Cinema Spectacle qu'une DSN mensuelle REELLE jouee appelle (`aer_fictive::archive`) : l'archive de nature 46,
/// ou `None` quand aucune n'est due (France Travail ne publie rien) ; un signalement (nature 04, 05, 07) n'en appelle pas.
/// Le fichier depose est relu la ou le depot l'a pris (`sorties/<mois>/<fichier>`).
fn aer_jouee(ctx: &Contexte, l: &Value) -> R<Option<Vec<u8>>> {
    if l.get("mode").and_then(Value::as_str) != Some("reel") {
        return Ok(None);
    }
    let nature = l.get("detail").and_then(|d| d.get("nature")).filter(|n| vrai(n)).map(valeur_en_texte).unwrap_or_default();
    if !nature.is_empty() && nature != "01" {
        return Ok(None);
    }
    let mois = l.get("mois").map(valeur_en_texte).unwrap_or_default();
    let fichier = l.get("fichier").filter(|f| vrai(f)).map(valeur_en_texte).unwrap_or_default();
    if mois.is_empty() || fichier.is_empty() {
        return Ok(None);
    }
    let chemin = if Path::new(&fichier).is_absolute() { PathBuf::from(&fichier) } else { ctx.racine.join("sorties").join(&mois).join(&fichier) };
    let Ok(octets) = std::fs::read(&chemin) else { return Ok(None) };
    crate::aer_fictive::archive(ctx, &octets, &mois, ctx.aujourdhui).map_err(Echec::from)
}

/// Le delai, en secondes, au bout duquel le simulateur rend les retours d'un depot joue a la releve.
pub const DELAI_RETOURS_JOUES: i64 = 6;

/// Telecharge et archive ce que les organismes ont publie. `debut` et `fin` : heures locales naives
/// (`maintenant_naif`). `client` : le client du service, avec son mot de passe (le canal DPAE passe le sien, avec
/// `canal = "dpae"`) ; `None` dans le monde : aucun mot de passe n'est pose, et cela se dit.
#[allow(clippy::too_many_arguments)]
pub fn relever_retours(
    ctx: &Contexte,
    branchement: Branchement,
    debut: i64,
    fin: i64,
    tout: bool,
    client: Option<Client>,
    canal: &str,
    suites: &mut Suites,
) -> R<Releve> {
    let mut releve = Releve::default();
    match branchement {
        // Rien n'est parti, rien ne revient.
        Branchement::Message(_) => return Ok(releve),
        // Les retours des depots joues arrivent d'ici, sans reseau ni mot de passe.
        Branchement::Simulateur if client.is_none() => {
            releve.rattaches = rattacher_simulation(ctx, DELAI_RETOURS_JOUES, suites)?;
            return Ok(releve);
        }
        _ => {}
    }
    let Some(mut client) = client else {
        releve.incidents.push(
            "Mot de passe net-entreprises absent : ni dans l'environnement, ni dans le coffre. `moteur coffre poser net-entreprises`.".into(),
        );
        return Ok(releve);
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

    // UN RETOUR RATE N'EST PAS UN RETOUR VU : un echec reste NEUF, et se retente au releve suivant.
    let mut rates: Vec<Value> = Vec::new();
    let a_telecharger: Vec<Value> = if tout { releve.trouves.clone() } else { releve.nouveaux.clone() };
    for r in a_telecharger {
        let Some(url) = r.get("url").filter(|u| vrai(u)).map(valeur_en_texte) else { continue };
        match client.telecharger(&url) {
            Ok((contenu, ctype)) => {
                let chemin = archiver(ctx, &r, &contenu, &ctype, canal).map_err(en_echec)?;
                // LE TAUX AT/MP NOTIFIE SE LIT DE LA NOTIFICATION (regle 1) : il se pose dans la surcouche, par les suites.
                if canal.is_empty() && r.get("nature").map(valeur_en_texte).as_deref() == Some(crate::retours::NATURE_TAUX_ATMP) {
                    suites.taux_atmp.push((r.clone(), chemin.clone()));
                }
                // UN SIGNALEMENT REFUSE B1-208-10 : l'ordre etait deja pris ; la conduite a tenir se dit.
                if canal.is_empty() && r.get("nature").map(valeur_en_texte).as_deref() == Some("11") {
                    let flux = r.get("flux").filter(|f| vrai(f)).map(valeur_en_texte).unwrap_or_default();
                    for d in crate::retours::declarations_du_bilan(&contenu) {
                        releve.a_faire.extend(crate::ordre_signalements::conduite_b1_208_10(&d, &flux));
                    }
                }
                // LE TAUX DU PAS SE LIT DU COMPTE RENDU (regle 1) : il va au magasin du PAS, par les suites.
                if canal.is_empty() && r.get("nature").map(valeur_en_texte).as_deref() == Some(crate::retours::NATURE_TAUX_PAS) {
                    suites.taux_pas.push((r.clone(), chemin.clone()));
                }
                releve.archives.push(RetourArchive { retour: r, chemin, contenu });
            }
            Err(p) if p.est_api() => {
                releve.incidents.push(format!("Telechargement du retour nature {} : {}", r.get("nature").map(valeur_en_texte).unwrap_or_else(|| "None".into()), p.message));
                rates.push(r);
            }
            Err(p) => return Err(en_echec(p)),
        }
    }

    if canal.is_empty() {
        // L'EFFECTIF S'ECRIT SEUL : `config/effectifs.json` se refait depuis les archives.
        reconstruire_effectifs(&ctx.racine)?;
    }

    if !releve.trouves.is_empty() {
        let vus: Vec<Value> = releve.trouves.iter().filter(|r| !rates.contains(r)).cloned().collect();
        noter_vus(ctx, &vus, canal).map_err(en_echec)?;
        // Un depot n'est fini que quand sa reponse est la : chaque retour se range en face du depot qui l'a
        // provoque -- par le flux, et par rien d'autre.
        let contenus: Vec<([Value; 3], Vec<u8>)> = releve.archives.iter().map(|a| (cle_de_retour(&a.retour), a.contenu.clone())).collect();
        releve.rattaches = suites.garder(crate::etats::rattacher_retours_avec_contenus(&ctx.racine, &Value::Array(releve.trouves.clone()), &contenus))?;
    }

    // UN DEPOT QUE LES RETOURS ATTESTENT EST UN DEPOT : ce qu'un autre logiciel a depose sous le meme compte
    // entre au journal de lui-meme, et le mois qu'il declare n'est plus « en retard ».
    if canal.is_empty() {
        let maintenant = socle::paris::maintenant()[..16].replacen('T', " ", 1);
        suites.garder(crate::reprise_journal::reprendre_les_depots(ctx, "releve", &maintenant))?;
    }

    // L'AER SE RANGE DES QU'ELLE ARRIVE : la boite d'envoi la voit (les suites).
    if canal.is_empty() {
        suites.aer.extend(
            releve
                .archives
                .iter()
                .filter(|a| a.retour.get("nature").map(valeur_en_texte).as_deref() == Some(crate::retours::NATURE_AER_SPECTACLE))
                .map(|a| a.chemin.clone()),
        );
    }
    Ok(releve)
}

/// Les retours de l'API DPAE -- UN AUTRE SERVICE que celui de la DSN, ses archives sont dans `retours/dpae/`. `flux` : le
/// seul chemin documente (la FAQ) quand on connait l'idflux ; sinon la consultation par plage, sur le canal `dpae`.
/// `client` : le client Urssaf, avec son mot de passe ; `None` dans le monde : aucun mot de passe n'est pose.
#[allow(clippy::too_many_arguments)]
pub fn relever_retours_dpae(
    ctx: &Contexte,
    branchement: Branchement,
    debut: i64,
    fin: i64,
    tout: bool,
    flux: &str,
    client: Option<Client>,
    suites: &mut Suites,
) -> R<Releve> {
    match branchement {
        Branchement::Message(_) => return Ok(Releve::default()),
        Branchement::Simulateur => {
            let mut r = Releve::default();
            r.rattaches = rattacher_simulation(ctx, DELAI_RETOURS_JOUES, suites)?;
            return Ok(r);
        }
        Branchement::Monde => {}
    }
    let Some(mut client) = client else {
        let mut r = Releve::default();
        r.incidents.push("Mot de passe DPAE absent : ni dans l'environnement, ni dans le coffre. `moteur coffre poser urssaf`.".into());
        return Ok(r);
    };
    if flux.is_empty() {
        return relever_retours(ctx, branchement, debut, fin, tout, Some(client), "dpae", suites);
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
        let Some(url) = x.get("url").filter(|u| vrai(u)).map(valeur_en_texte) else { continue };
        match client.telecharger(&url) {
            Ok((contenu, ctype)) => {
                let chemin = archiver(ctx, &x, &contenu, &ctype, "dpae").map_err(en_echec)?;
                r.archives.push(RetourArchive { retour: x, chemin, contenu });
            }
            Err(p) if p.est_api() => r.incidents.push(format!("Telechargement nature {} : {}", x.get("nature").map(valeur_en_texte).unwrap_or_else(|| "None".into()), p.message)),
            Err(p) => return Err(en_echec(p)),
        }
    }
    if !r.trouves.is_empty() {
        noter_vus(ctx, &r.trouves, "dpae").map_err(en_echec)?;
        // Le chemin par flux accroche aussi ce qu'il trouve au journal.
        let contenus: Vec<([Value; 3], Vec<u8>)> = r.archives.iter().map(|a| (cle_de_retour(&a.retour), a.contenu.clone())).collect();
        r.rattaches = suites.garder(crate::etats::rattacher_retours_avec_contenus(&ctx.racine, &Value::Array(r.trouves.clone()), &contenus))?;
    }
    Ok(r)
}

/// La derniere journee, comme `suivre_le_depot` et `bilan_de_la_repetition` la relevent : de hier a maintenant.
pub fn derniere_journee() -> (i64, i64) {
    let fin = maintenant_naif();
    (fin - 86_400, fin)
}

// ------------------------------------------------------------------- l'effectif

/// `effectif.lire_crm118` : les effectifs d'un retour de nature 118 (code, valeur, periode, calcul), les parametres
/// (`RO.B008`) que lit le lecteur des comptes rendus NEOReS (`retours::lire_neores`, d'apres le cahier technique NEOReS
/// 2026.1.2, `atelier/reference/sources/neores/`) ; un XML qui ne se lit pas ne dit rien.
fn lire_crm118(contenu: &[u8]) -> R<Option<Vec<(String, f64, String, String, String)>>> {
    let Some(crm) = crate::retours::lire_neores(contenu) else { return Ok(None) };
    let mut out = Vec::new();
    for p in crm["parametres"].as_array().into_iter().flatten() {
        let t = |k: &str| p[k].as_str().unwrap_or("").to_string();
        let (code, valeur) = (t("type"), t("valeur"));
        if code.is_empty() || valeur.is_empty() {
            continue;
        }
        let v: f64 = valeur.parse().map_err(|_| erreur("ValueError", format!("nombre illisible : '{}'", valeur)))?;
        out.push((code, v, t("effet"), t("fin"), t("calcule_le")));
    }
    Ok(Some(out))
}

/// Relit tous les 118 archives et reecrit `config/effectifs.json`. Pour chaque annee
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
        socle::ecriture::ecrire_json(&chemin, &donnees, 2).map_err(|m| erreur("OSError", m))?;
    }
    Ok(())
}

/// Le resume d'une relève, pour une phrase d'incident : `"; ".join(rel.incidents)`.
pub fn incidents_en_phrase(r: &Releve) -> String {
    r.incidents.join("; ")
}
