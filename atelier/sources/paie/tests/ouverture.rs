//! L'ouverture et la veille (`ouverture.rs`, lot I2b) : chaque branche d'`ouverture`, les routes `ouverture` et
//! `ouverture-derniere`, la trace `config/.ouverture.json`, les decisions du 07/10/2026 et UN TEST PAR DEFAUT CORRIGE de
//! l'ancien Python (la regle : un defaut du Python ne se reproduit jamais dans le Rust).
//!
//! AUCUN RESEAU VERS L'EXTERIEUR : chaque service est un client donne (`Clients`) qui parle a un faux serveur local
//! (127.0.0.1, un fil du test), un faux courriel, un faux Drive en memoire (`tests/faux_drive`). `tour()` retire de
//! l'environnement toute variable de mot de passe, pose `PUPPETPAIE_CODE` sur un dossier de code sans `EDITEUR.json`
//! (le serveur des incidents de l'editeur n'est alors que celui qu'un test pose, jamais celui du poste), et un test ne
//! laisse `incidents_editeur` vrai que pour parler a son faux serveur. Chaque test monte sa compagnie dans un dossier a
//! lui, copie de `fictives/` (jamais `prive/`, jamais un vrai coffre ni jeton).

mod faux_drive;
#[path = "outils/mod.rs"]
mod outils;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::io::Write;
use std::sync::{Arc, Mutex, MutexGuard, Once, OnceLock};

use faux_drive::{FauxDrive, Panne};
use paie::contexte::Contexte;
use outils::{compagnie as cie, dossier_de_test, dossier_du_processus, ecrire, lire, port_ferme, serveur, http, http_json, FauxCourriel, Requete};
use paie::courriel::Courriel;
use paie::date::Jour;
use paie::drive::{Depose, Distant, Drive, ErreurGoogle};
use paie::ouverture::{derniere_ouverture, ouverture, Clients, Options, Ouverture};
use paie::releve_retours::{maintenant_naif, Adresses, Client};
use serde_json::{json, Value};

const AUJOURDHUI: &str = "2026-10-08";

static VERROU: Mutex<()> = Mutex::new(());
static UNE_FOIS: Once = Once::new();

/// Le poste du processus : une cle, et un dossier de code sans `EDITEUR.json` ni `prive/` voisin (le serveur des incidents
/// n'est jamais celui du poste).
fn poste() -> &'static Path {
    static P: OnceLock<PathBuf> = OnceLock::new();
    P.get_or_init(|| {
        let base = dossier_du_processus("ouverture-poste");
        std::fs::create_dir_all(base.join("cle")).unwrap();
        outils::copier(&outils::code_livre(), &base.join("logiciel/donnees"));
        base
    })
}

/// Un test a la fois (l'environnement est celui du processus), l'environnement nettoye.
fn tour() -> MutexGuard<'static, ()> {
    let g = VERROU.lock().unwrap_or_else(|e| e.into_inner());
    UNE_FOIS.call_once(|| {
        std::env::set_var("PUPPETPAIE_CODE", poste().join("logiciel/donnees"));
        std::env::set_var("PUPPETPAIE_AUJOURDHUI", AUJOURDHUI);
        std::env::set_var("PUPPETPAIE_CLE", poste().join("cle"));
    });
    for v in [
        "COURRIEL_COMPTE",
        "COURRIEL_MOTDEPASSE",
        "NET_ENTREPRISES_MOTDEPASSE",
        "URSSAF_MOTDEPASSE",
        "PUPPETPAIE_INCIDENTS",
        "PUPPETPAIE_MOTDEPASSE",
        "HTTPS_PROXY",
        "https_proxy",
        "HTTP_PROXY",
        "http_proxy",
    ] {
        std::env::remove_var(v);
    }
    g
}

// ------------------------------------------------------------------ les compagnies

/// Une copie d'une compagnie fictive. `ordinaire` : le marqueur est retire (une compagnie comme une autre, coffre vide) et
/// l'identite du declarant net-entreprises et du compte Urssaf est posee (ce que fait une compagnie qui releve).
fn compagnie(nom: &str, fictive: &str, ordinaire: bool) -> PathBuf {
    let racine = cie::entiere(fictive, &dossier_de_test("ouverture", nom).join("compagnie"));
    if ordinaire {
        cie::ordinaire(&racine);
    }
    racine
}

fn trace(racine: &Path) -> Value {
    lire(racine, "config/.ouverture.json")
}

fn trace_octets(racine: &Path) -> Vec<u8> {
    std::fs::read(racine.join("config/.ouverture.json")).unwrap()
}

/// Les cles d'un objet, triees.
fn cles(v: &Value) -> Vec<&str> {
    let mut c: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    c.sort();
    c
}

fn tri<const N: usize>(mut c: [&str; N]) -> [&str; N] {
    c.sort();
    c
}

fn contexte(racine: &Path) -> Contexte {
    Contexte::nouveau(racine, Some(Jour::lire(AUJOURDHUI).unwrap())).unwrap()
}

fn options_de(racine: &Path) -> HashMap<String, String> {
    let mut o = HashMap::new();
    o.insert("racine".to_string(), racine.to_string_lossy().into_owned());
    o.insert("date".to_string(), AUJOURDHUI.to_string());
    o
}

/// Les options d'un passage de test : tout allume, SAUF ce que le test demande, et jamais le fil de l'editeur.
fn opt() -> Options {
    Options { incidents_editeur: false, ..Options::default() }
}

fn seule_la_trace() -> Options {
    Options { sauvegarder: false, retours: false, signatures: false, filet: false, ..opt() }
}

fn jouer(racine: &Path, o: &Options, clients: Clients<'_>) -> Ouverture {
    ouverture(&options_de(racine), &contexte(racine), o, clients, None)
}

fn sans_clients(racine: &Path, o: &Options) -> Ouverture {
    jouer(racine, o, Clients::default())
}

/// L'horloge de la compagnie fictive remise a une date (ce qui etait du depuis se rejoue d'un coup).
fn reculer_l_horloge(racine: &Path, le: &str) {
    let mut h = lire(racine, "config/horloge.json");
    h["le"] = json!(le);
    let garde = le[..7].to_string();
    h["mois_faits"] = Value::Array(h["mois_faits"].as_array().unwrap().iter().filter(|m| m.as_str().unwrap() < garde.as_str()).cloned().collect());
    ecrire(racine, "config/horloge.json", &h);
}

#[derive(Clone)]
struct RetourFaux {
    service: &'static str,
    flux: &'static str,
    nature: &'static str,
    contenu: &'static str,
}

/// Les pannes qu'un faux net-entreprises (ou Urssaf) peut jouer.
#[derive(Clone, Default)]
struct Pannes {
    auth: Option<u16>,
    consultation: Option<u16>,
    telechargement: Option<u16>,
    depot: Option<u16>,
    coupe: bool,
}

fn dsn(flux: &'static str, nature: &'static str, contenu: &'static str) -> RetourFaux {
    RetourFaux { service: "dsn", flux, nature, contenu }
}

fn dpae(flux: &'static str, nature: &'static str, contenu: &'static str) -> RetourFaux {
    RetourFaux { service: "dpae", flux, nature, contenu }
}

const BILAN: &str = "<?xml version='1.0' encoding='UTF-8'?><bilan><statut>OK</statut></bilan>";
const ATMP: &str = "<atmp><taux>1.20</taux></atmp>";

/// La publication d un retour : il y a douze heures, a la seconde (14 chiffres), au milieu d une tranche de la releve.
fn hier() -> String {
    let iso = paie::releve_retours::iso_de_naif(maintenant_naif() - 43_200);
    format!("{}{}{}{}{}{}", &iso[0..4], &iso[5..7], &iso[8..10], &iso[11..13], &iso[14..16], &iso[17..19])
}

/// Un faux net-entreprises (`/dsn`) et Urssaf (`/dpae`) : l'authentification, la consultation par plage, le telechargement et,
/// pour la DPAE, le depot. Rend l'adresse, ce qu'il a recu, et le nombre de depots.
fn organismes(retours: Vec<RetourFaux>, pannes: Pannes) -> (String, outils::Recu, Arc<Mutex<usize>>) {
    let depots = Arc::new(Mutex::new(0usize));
    let d2 = depots.clone();
    let publication = hier();
    let (url, recu) = serveur(move |q: &Requete| {
        let (base, chemin) = (q.base.as_str(), q.chemin.as_str());
        if pannes.coupe {
            return None;
        }
        let reste = chemin.trim_start_matches('/').split_once('/').map(|(_, r)| r.to_string()).unwrap_or_default();
        let service = chemin.trim_start_matches('/').split('/').next().unwrap_or("");
        if reste.starts_with("authentifier") {
            return Some(match pannes.auth {
                Some(s) => http(s, "text/plain", b"Identifiants refuses (faux)"),
                None => http(200, "text/plain", b"JETON-FICTIF"),
            });
        }
        if reste.starts_with("lister-retours-declarant") {
            if let Some(s) = pannes.consultation {
                return Some(http(s, "text/plain", b"Erreur interne (faux)"));
            }
            // La plage demandee (`.../AAAAMMJJhhmmss/AAAAMMJJhhmmss`) : seulement les retours publies dedans.
            let bornes: Vec<&str> = reste.trim_end_matches('/').split('/').rev().take(2).collect();
            let (fin, debut) = (bornes[0].to_string(), bornes.get(1).copied().unwrap_or("0").to_string());
            let dedans = publication.as_str() >= debut.as_str() && publication.as_str() <= fin.as_str();
            let ici: Vec<&RetourFaux> = retours.iter().filter(|r| r.service == service).collect();
            if ici.is_empty() || !dedans {
                return Some(http(404, "text/plain", b"Aucun retour sur cette plage"));
            }
            let mut xml = String::from("<retours>");
            for (i, r) in ici.iter().enumerate() {
                xml += &format!(
                    "<flux><id>{}</id><retour><publication>{}</publication><production>{}</production><nature>{}</nature><statut>OK</statut><id>R{}</id><url>{}/{}/telecharger/{}</url></retour></flux>",
                    r.flux, publication, publication, r.nature, i, base, service, i
                );
            }
            xml += "</retours>";
            return Some(http(200, "application/xml", xml.as_bytes()));
        }
        if let Some(i) = reste.strip_prefix("telecharger/") {
            if let Some(s) = pannes.telechargement {
                return Some(http(s, "text/plain", b"Telechargement impossible (faux)"));
            }
            let ici: Vec<&RetourFaux> = retours.iter().filter(|r| r.service == service).collect();
            let r = ici[i.parse::<usize>().unwrap_or(0).min(ici.len().saturating_sub(1))];
            return Some(http(200, "application/xml", r.contenu.as_bytes()));
        }
        if reste.starts_with("deposer-dsn") {
            *d2.lock().unwrap() += 1;
            return Some(match pannes.depot {
                Some(s) => http(s, "text/plain", b"Refus du depot (faux)"),
                None => http(200, "text/plain", b"<idflux>FLUX-FAUX</idflux>\nDPAE recue (faux Urssaf)."),
            });
        }
        Some(http(404, "text/plain", b"inconnu"))
    });
    (url, recu, depots)
}

fn adresses(ctx: &Contexte, base: &str, dpae: bool) -> Adresses {
    let service = if dpae { Client::dpae(ctx, false, Some("x".into())).adresses.service } else { Client::dsn(ctx, "production", Some("x".into())).unwrap().adresses.service };
    Adresses {
        authentification: format!("{}/{}", base, if dpae { "dpae/authentifier_dpae" } else { "dsn/authentifier/" }),
        retours: format!("{}/{}/lister-retours-declarant/1.0/", base, if dpae { "dpae" } else { "dsn" }),
        retours_flux: format!("{}/{}/lister-retours-flux/1.0/", base, if dpae { "dpae" } else { "dsn" }),
        depot: format!("{}/dpae/deposer-dsn/1.0/", base),
        service,
    }
}

/// Le client net-entreprises, pointe sur un faux serveur local.
fn client_dsn(racine: &Path, base: &str) -> Client {
    assert!(base.starts_with("http://127.0.0.1:"), "le faux serveur est local");
    let ctx = contexte(racine);
    let mut c = Client::dsn(&ctx, "production", Some("faux-motdepasse-dsn".into())).unwrap();
    c.adresses = adresses(&ctx, base, false);
    c
}

/// Le client Urssaf, pointe sur un faux serveur local.
fn client_dpae(racine: &Path, base: &str) -> Client {
    assert!(base.starts_with("http://127.0.0.1:"), "le faux serveur est local");
    let ctx = contexte(racine);
    let mut c = Client::dpae(&ctx, false, Some("faux-motdepasse-dpae".into()));
    c.adresses = adresses(&ctx, base, true);
    c
}

/// Les deux clients de la releve, sur un meme faux serveur.
fn les_deux<'a>(racine: &Path, base: &str) -> Clients<'a> {
    Clients { dsn: Some(client_dsn(racine, base)), dpae: Some(client_dpae(racine, base)), ..Clients::default() }
}

// ------------------------------------------------------------------ un faux courriel

const ZOE: &str = "zoe@exemple.invalid";
const CODE_FICHE: &str = "FIC-ABCDEF";

/// Une demande de fiche attendue, au journal.
fn noter_demande_de_fiche(racine: &Path) {
    let mut lignes = lire(racine, "journal/depots.json").as_array().unwrap().clone();
    lignes.push(json!({
        "quoi": "fiche-demande", "quand": "2026-10-02T09:00:00", "origine": "", "mois": null, "fichier": null, "empreinte": null,
        "mode": "reel", "qui": "interface", "accepte": true, "destinataire": ZOE, "flux": "", "retours": [],
        "reponse": format!("fiche demandee a {}", ZOE), "trace": null,
        "detail": {"code": CODE_FICHE, "adresse": ZOE, "prenom": "Zoé", "message": "<demande-1@exemple.invalid>", "expediteur": "boite@exemple.invalid"}
    }));
    ecrire(racine, "journal/depots.json", &Value::Array(lignes));
}

fn reponse_de_fiche() -> Vec<u8> {
    let mut lignes: Vec<String> = paie::fiche_questionnaire::QUESTIONS
        .iter()
        .map(|(cle, lib, fac)| {
            let v = match *cle {
                "prenom" => "Zoé",
                "nom" => "Dubois",
                "naissance" => "14/03/1991",
                "lieu" => "Lyon",
                "pays" => "France",
                "nationalite" => "française",
                "nir" => "291036912345653",
                "adresse" => "5 rue des Lilas",
                "code_postal" => "69003",
                "commune" => "Lyon",
                _ => "",
            };
            format!("{}{} : {}", lib, if *fac { " (facultatif)" } else { "" }, v)
        })
        .collect();
    lignes.push(String::new());
    lignes.push(format!("Code (ne pas modifier) : {}", CODE_FICHE));
    format!(
        "From: {}\r\nTo: boite@exemple.invalid\r\nSubject: Re: Ta fiche\r\nDate: Sat, 03 Oct 2026 10:15:00 +0200\r\nMessage-ID: <rep-1@exemple.invalid>\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\n{}\r\n",
        ZOE,
        lignes.join("\r\n")
    )
    .into_bytes()
}

// ------------------------------------------------------------------ un Drive qui compte ses appels

/// Le faux Drive en memoire, dont un compteur partage dit combien d'appels il a recus (lisible pendant le passage).
struct DriveCompte {
    drive: FauxDrive,
    appels: Rc<Cell<usize>>,
}

impl Drive for DriveCompte {
    fn compte(&self) -> Option<String> {
        self.drive.compte()
    }
    fn chemin(&mut self, noms: &[&str]) -> Result<String, ErreurGoogle> {
        self.appels.set(self.appels.get() + 1);
        self.drive.chemin(noms)
    }
    fn lister(&mut self, parent: &str) -> Result<std::collections::BTreeMap<String, Distant>, ErreurGoogle> {
        self.appels.set(self.appels.get() + 1);
        self.drive.lister(parent)
    }
    fn televerser(&mut self, fichier: &Path, parent: &str, nom: Option<&str>, connu: Option<&Distant>) -> Result<Depose, ErreurGoogle> {
        self.appels.set(self.appels.get() + 1);
        self.drive.televerser(fichier, parent, nom, connu)
    }
    fn sous_dossiers(&mut self, parent: &str) -> Result<Vec<(String, String)>, ErreurGoogle> {
        self.appels.set(self.appels.get() + 1);
        self.drive.sous_dossiers(parent)
    }
    fn telecharger(&mut self, id: &str) -> Result<Vec<u8>, ErreurGoogle> {
        self.appels.set(self.appels.get() + 1);
        self.drive.telecharger(id)
    }
}

fn drive_compte() -> (DriveCompte, Rc<Cell<usize>>) {
    let n = Rc::new(Cell::new(0));
    (DriveCompte { drive: FauxDrive::nouveau(), appels: n.clone() }, n)
}

fn avec_drive<'a>(d: &'a mut DriveCompte) -> Clients<'a> {
    Clients { drive: Some(d as &mut dyn Drive), ..Clients::default() }
}

fn seule_la_sauvegarde() -> Options {
    Options { retours: false, signatures: false, filet: false, ..opt() }
}

// ============================================================================ la compagnie fictive

#[test]
fn la_fictive_joue_l_horloge_les_retours_et_ne_fait_rien_d_autre() {
    let _t = tour();
    let c = compagnie("fictive-horloge", "fictive-3090-petite", false);
    reculer_l_horloge(&c, "2026-08-15");
    let r = sans_clients(&c, &Options { filet: false, ..opt() });
    assert!(!r.horloge.is_empty(), "l'horloge a rejoue ce qui etait du avant aujourd'hui");
    let releve = r.releve.as_ref().expect("les retours des depots joues sont rattaches");
    assert!(releve.trouves.is_empty() && releve.archives.is_empty(), "un releve qui ne porte que `rattaches`");
    // Rien d'autre : ni coffre, ni reseau, ni sauvegarde, ni signatures relevees (rien ne manque au coffre).
    assert!(r.sauvegarde.is_none() && r.signatures.is_none() && r.releve_dpae.is_none());
    assert!(r.a_poser.is_empty() && r.manquants.is_empty(), "{:?} {:?}", r.a_poser, r.manquants);
    assert!(r.fiches.is_some(), "les fiches jouees");
    // La trace dit l'horloge.
    assert_eq!(trace(&c)["horloge"], json!(r.horloge));
    // A jour : l'horloge redit le dernier rattrapage, et ne rejoue rien.
    let r2 = sans_clients(&c, &Options { filet: false, ..opt() });
    assert_eq!(trace(&c)["horloge"], json!(r2.horloge));
    assert!(r2.a_poser.is_empty());
}

#[test]
fn la_fictive_sans_retours_n_en_rattache_pas() {
    let _t = tour();
    let c = compagnie("fictive-sans-retours", "fictive-3090-petite", false);
    let r = sans_clients(&c, &Options { retours: false, filet: false, ..opt() });
    assert!(r.releve.is_none());
    assert!(r.a_poser.is_empty() && r.sauvegarde.is_none());
}

#[test]
fn la_fictive_ne_sauvegarde_pas_meme_quand_on_le_demande() {
    let _t = tour();
    let c = compagnie("fictive-sauvegarde", "fictive-3090-petite", false);
    let (mut d, appels) = drive_compte();
    let r = jouer(&c, &Options { filet: false, ..opt() }, avec_drive(&mut d));
    assert!(r.sauvegarde.is_none());
    assert_eq!(appels.get(), 0, "aucun appel au Drive depuis une compagnie fictive");
}

// ============================================================================ la copie de developpement

#[test]
fn la_copie_de_developpement_ne_sauvegarde_pas_et_ne_releve_pas_les_signatures_mais_releve_les_retours() {
    let _t = tour();
    let c = compagnie("copie-dev", "fictive-3090-petite", true);
    ecrire(&c, "COPIE-DE-DEVELOPPEMENT.json", &json!({"copie_de": "compagnie ordinaire du poste (fictive)", "quand": "2026-10-01T10:00:00"}));
    let (base, _recu, _) = organismes(vec![dsn("F1", "11", BILAN)], Pannes::default());
    let (mut d, appels) = drive_compte();
    let mut clients = les_deux(&c, &base);
    clients.drive = Some(&mut d as &mut dyn Drive);
    let r = jouer(&c, &Options { filet: false, ..opt() }, clients);
    assert!(r.sauvegarde.is_none() && r.signatures.is_none(), "ni sauvegarde ni signatures");
    assert_eq!(appels.get(), 0);
    assert!(!r.a_poser.contains(&"google".to_string()), "pas de manque google : elle ne sauvegarde pas");
    assert_eq!(r.releve.as_ref().unwrap().trouves.len(), 1, "les retours se relevent");
}

// ============================================================================ le coffre : vide, partiel, client donne

#[test]
fn coffre_vide_trois_manques_et_une_trace_sans_les_phrases_du_coffre() {
    let _t = tour();
    let c = compagnie("coffre-vide", "fictive-3090-petite", true);
    let r = sans_clients(&c, &Options { filet: false, ..opt() });
    assert_eq!(r.a_poser, ["net-entreprises", "urssaf", "google"]);
    assert_eq!(r.manquants.len(), 3);
    assert!(r.manquants[0].starts_with("Mot de passe net-entreprises absent du coffre"));
    assert!(r.manquants[1].starts_with("Mot de passe Urssaf (DPAE) absent du coffre"));
    assert!(r.manquants[2].starts_with("Compte Google non lie"));
    assert!(r.releve.is_none() && r.releve_dpae.is_none() && r.sauvegarde.is_none());
    assert!(!r.tout_va_bien());
    // La trace ne repete pas les phrases du coffre : `a_poser` les dit.
    let t = trace(&c);
    assert_eq!(t["manquants"], json!([]));
    assert_eq!(t["a_poser"], json!(["net-entreprises", "urssaf", "google"]));
    // Les options retirent leur manque.
    assert_eq!(sans_clients(&c, &Options { filet: false, retours: false, ..opt() }).a_poser, ["google"]);
    assert_eq!(sans_clients(&c, &Options { filet: false, sauvegarder: false, ..opt() }).a_poser, ["net-entreprises", "urssaf"]);
    // La veille: le manque google passe avant « rien n'a bouge ».
    assert_eq!(sans_clients(&c, &Options { filet: false, veille: true, ..opt() }).a_poser, ["net-entreprises", "urssaf", "google"]);
}

#[test]
fn un_client_donne_n_est_jamais_confronte_au_coffre() {
    let _t = tour();
    let c = compagnie("client-donne", "fictive-3090-petite", true);
    let (base, recu, _) = organismes(
        vec![dsn("F-REEL", "11", BILAN), dsn("F-ATMP", "34", ATMP)],
        Pannes::default(),
    );
    let o = Options { filet: false, sauvegarder: false, signatures: false, ..opt() };
    let r = jouer(&c, &o, les_deux(&c, &base));
    assert!(r.a_poser.is_empty(), "un client donne : ni manque net-entreprises ni manque urssaf ({:?})", r.a_poser);
    let rel = r.releve.as_ref().unwrap();
    assert_eq!((rel.trouves.len(), rel.nouveaux.len(), rel.archives.len()), (2, 2, 2));
    assert!(r.incidents.is_empty(), "{:?}", r.incidents);
    // Le taux AT/MP recu demande un geste : il devient un manquant (le geste, pas une phrase du coffre).
    assert!(r.manquants.iter().any(|m| m.contains("taux AT/MP")), "{:?}", r.manquants);
    assert!(trace(&c)["manquants"].as_array().unwrap().iter().any(|m| m.as_str().unwrap().contains("taux AT/MP")));
    assert_eq!(trace(&c)["retours"], json!("2 retour(s), dont 2 nouveau(x)."));
    assert_eq!(trace(&c)["nouveaux"], json!(2));
    // Les pieces sont archivees.
    assert!(std::fs::read_dir(c.join("retours")).unwrap().flatten().count() >= 2);
    // Le canal DPAE a ete consulte aussi (un 404 : rien sur la plage).
    assert!(recu.lock().unwrap().iter().any(|q| q.chemin.starts_with("/dpae/lister-retours-declarant")));
    assert_eq!(r.releve_dpae.as_ref().unwrap().trouves.len(), 0);
    // Un second tour ne trouve rien de nouveau.
    let r2 = jouer(&c, &o, les_deux(&c, &base));
    assert_eq!(r2.releve.as_ref().unwrap().nouveaux.len(), 0);
    assert_eq!(trace(&c)["retours"], json!("2 retour(s), dont 0 nouveau(x)."));
}

#[test]
fn le_canal_dpae_avec_un_client_donne_releve_les_retours_de_l_urssaf() {
    let _t = tour();
    let c = compagnie("canal-dpae", "fictive-3090-petite", true);
    let (base, recu, _) = organismes(vec![dpae("F-DPAE", "41", BILAN)], Pannes::default());
    let o = Options { filet: false, sauvegarder: false, signatures: false, ..opt() };
    let r = jouer(&c, &o, Clients { dpae: Some(client_dpae(&c, &base)), ..Clients::default() });
    // Net-entreprises n'a pas de client : son manque reste ; l'Urssaf, elle, est relevee.
    assert_eq!(r.a_poser, ["net-entreprises"]);
    let rel = r.releve_dpae.as_ref().unwrap();
    assert_eq!((rel.trouves.len(), rel.nouveaux.len(), rel.archives.len()), (1, 1, 1));
    assert!(recu.lock().unwrap().iter().all(|q| q.chemin.starts_with("/dpae/")), "rien n'a ete demande a net-entreprises");
    assert!(c.join("retours/dpae").is_dir(), "archives dans retours/dpae/");
}

#[test]
fn les_pannes_d_un_organisme_sont_des_incidents_pas_un_arret() {
    let _t = tour();
    let c = compagnie("pannes", "fictive-3090-petite", true);
    let o = Options { filet: false, sauvegarder: false, signatures: false, ..opt() };
    let un = dsn("F1", "11", BILAN);
    for (pannes, morceau) in [
        (Pannes { auth: Some(401), ..Pannes::default() }, "Authentification refusee"),
        (Pannes { consultation: Some(500), ..Pannes::default() }, "Consultation interrompue"),
        (Pannes { telechargement: Some(500), ..Pannes::default() }, "Telechargement du retour nature"),
    ] {
        let (base, _, _) = organismes(vec![un.clone()], pannes);
        let r = jouer(&c, &o, les_deux(&c, &base));
        assert!(r.incidents.iter().any(|i| i.contains(morceau)), "{} : {:?}", morceau, r.incidents);
        assert!(!r.hors_ligne);
        assert!(trace(&c)["incidents"].as_array().unwrap().iter().any(|i| i.as_str().unwrap().contains(morceau)));
    }
    // Une connexion coupee au milieu: un incident (ou hors ligne), jamais un arret.
    let (base, _, _) = organismes(vec![un], Pannes { coupe: true, ..Pannes::default() });
    let r = jouer(&c, &o, les_deux(&c, &base));
    assert!(!r.incidents.is_empty() || r.hors_ligne);
    // Une plage sans retour (404) n'est pas une panne.
    let (base, _, _) = organismes(vec![], Pannes::default());
    let r = jouer(&c, &o, les_deux(&c, &base));
    assert!(r.incidents.is_empty(), "{:?}", r.incidents);
    assert_eq!(r.releve.as_ref().unwrap().trouves.len(), 0);
    assert_eq!(trace(&c)["retours"], json!("Aucun retour sur la periode."));
}

#[test]
fn hors_ligne_une_seule_phrase_et_pas_de_ligne_de_retours() {
    let _t = tour();
    let c = compagnie("hors-ligne", "fictive-3090-petite", true);
    let ferme = port_ferme();
    let (mut d, _) = drive_compte();
    d.drive.pannes.push(Panne { operation: "chemin", motif: "".into(), erreur: ErreurGoogle::google("Reseau injoignable : [Errno 111] Connection refused") });
    let mut clients = les_deux(&c, &ferme);
    clients.drive = Some(&mut d as &mut dyn Drive);
    let r = jouer(&c, &Options { filet: false, signatures: false, ..opt() }, clients);
    assert!(r.hors_ligne);
    assert!(r.incidents.is_empty(), "les erreurs de reseau sortent des incidents : {:?}", r.incidents);
    let t = trace(&c);
    assert_eq!(t["hors_ligne"], json!(true));
    assert_eq!(t["retours"], json!(""), "hors ligne, la ligne des retours se tait");
    assert_eq!(t["incidents"], json!([]));
    // Elles ne se perdent pas : la trace garde qui n'a pas repondu.
    assert!(t["injoignables"].as_array().unwrap().iter().any(|i| i.as_str().unwrap().contains("Drive")), "{}", t);
}

/// « Hors connexion, c'est quand il n'y a pas internet » (Leandre, 21/10/2026) : le Drive qui ne repond pas ne tait pas la
/// releve qui, elle, est passee ; la trace dit les deux.
#[test]
fn le_drive_injoignable_ne_tait_pas_la_releve_passee() {
    let _t = tour();
    let c = compagnie("drive-injoignable", "fictive-3090-petite", true);
    let (base, _, _) = organismes(vec![], Pannes::default());
    let (mut d, _) = drive_compte();
    d.drive.pannes.push(Panne { operation: "chemin", motif: "".into(), erreur: ErreurGoogle::google("Reseau injoignable : [Errno 111] Connection refused") });
    let mut clients = les_deux(&c, &base);
    clients.drive = Some(&mut d as &mut dyn Drive);
    let r = jouer(&c, &Options { filet: false, signatures: false, ..opt() }, clients);
    assert!(r.hors_ligne);
    assert!(r.releve.is_some());
    let t = trace(&c);
    assert_eq!(t["retours"], json!("Aucun retour sur la periode."), "{}", t);
    assert_eq!(t["injoignables"].as_array().unwrap().len(), 1, "{}", t);
}

// ============================================================================ les gardes (l'ouverture n'echoue jamais)

#[test]
fn defaut_un_coffre_qui_refuse_de_s_ouvrir_est_un_incident_pas_une_chute() {
    // L'ancien Python laissait lever l'erreur du coffre (`coffre.disponible` etait hors de tout `try`) : l'ouverture tombait.
    let _t = tour();
    let c = compagnie("coffre-refuse", "fictive-3090-petite", true);
    std::fs::create_dir(c.join("config/coffre.bin")).unwrap(); // illisible : OSError, pas « non pose »
    let r = sans_clients(&c, &Options { filet: false, sauvegarder: false, signatures: false, ..opt() });
    assert!(r.incidents.iter().any(|i| i.starts_with("Retours : ")), "{:?}", r.incidents);
    assert!(r.incidents.iter().any(|i| i.starts_with("Retours DPAE : ")), "{:?}", r.incidents);
    assert!(r.a_poser.is_empty() && r.manquants.is_empty(), "ni manque ni a_poser : on ne sait pas si c'est pose");
    assert!(c.join("config/.ouverture.json").is_file(), "la trace s'ecrit quand meme");
}

#[test]
fn defaut_un_journal_illisible_ne_fait_pas_tomber_l_ouverture() {
    // Le Python levait : dans une compagnie fictive (`simulation.rattacher`), et dans la trace (`journal.flux_de_test`).
    let _t = tour();
    let c = compagnie("journal-illisible", "fictive-3090-petite", true);
    std::fs::write(c.join("journal/depots.json"), "{ ceci n'est pas du json").unwrap();
    let r = sans_clients(&c, &seule_la_trace());
    assert!(c.join("config/.ouverture.json").is_file(), "la trace s'ecrit : les retours de test sont supposes vides");
    assert_eq!(trace(&c)["nouveaux"], json!(0));
    assert!(r.incidents.is_empty());
    // Une compagnie fictive dont le journal est illisible : des incidents, pas une chute.
    let d = compagnie("journal-illisible-fictive", "fictive-3090-petite", false);
    std::fs::write(d.join("journal/depots.json"), "{ ceci n'est pas du json").unwrap();
    let r = sans_clients(&d, &opt());
    assert!(r.incidents.iter().any(|i| i.starts_with("Retours : ")), "{:?}", r.incidents);
    assert!(r.incidents.iter().any(|i| i.starts_with("Horloge de la compagnie fictive : ")), "{:?}", r.incidents);
}

#[test]
fn defaut_un_delai_depasse_n_est_pas_un_reseau_absent() {
    // Le Python rangeait « timed out » parmi les signes de reseau absent (`_SIGNES_HORS_LIGNE`) : un refus lent devenait
    // « hors ligne » et sortait des incidents. Le serveur repond : le dossier est reporte (tickets #231 et #232 : ce n'est
    // pas un incident du releve), avec sa cause en clair ; s'il dure, la ligne de la sauvegarde le dira.
    let _t = tour();
    let c = compagnie("timed-out", "fictive-3090-petite", true);
    let (mut d, _) = drive_compte();
    d.drive.pannes.push(Panne { operation: "lister", motif: "sauvegarde/config".into(), erreur: ErreurGoogle::nouvelle("TimeoutError", "The read operation timed out") });
    let r = jouer(&c, &seule_la_sauvegarde(), avec_drive(&mut d));
    assert!(!r.hors_ligne);
    let s = r.sauvegarde.as_ref().unwrap();
    assert!(!s.reportes.is_empty() && s.reportes.iter().all(|x| x.cause == "Google ne répond pas"), "{:?}", s.reportes);
    assert!(s.incidents.iter().any(|i| i.contains("timed out")), "le detail reste pour la ligne de commande : {:?}", s.incidents);
}

// ============================================================================ le PAS, les signatures, les fiches

#[test]
fn les_taux_de_pas_sont_ceux_du_module_pas() {
    let _t = tour();
    let c = compagnie("pas", "fictive-3090-petite", true);
    let r = sans_clients(&c, &seule_la_trace());
    let attendu = paie::pas::avertissements(&contexte(&c), None, None).unwrap();
    assert_eq!(r.taux_pas, attendu);
    assert_eq!(trace(&c)["taux_pas"], json!(attendu));
}

#[test]
fn une_fiche_attendue_et_sa_reponse_dans_la_boite() {
    let _t = tour();
    let c = compagnie("fiches", "fictive-3090-petite", true);
    noter_demande_de_fiche(&c);
    let mut boite = FauxCourriel::avec(vec![("m1", reponse_de_fiche())]);
    let o = Options { retours: false, filet: false, sauvegarder: false, ..opt() };
    let r = jouer(&c, &o, Clients { courriel: Some(&mut boite as &mut dyn Courriel), ..Clients::default() });
    let f = r.fiches.as_ref().unwrap();
    assert_eq!((f.attendues, f.recues.len()), (1, 1), "{:?}", f.incidents);
    assert!(r.signatures.is_some());
    assert!(r.incidents.is_empty(), "{:?}", r.incidents);
    assert!(!trace(&c)["fiches"].as_str().unwrap().is_empty());
    // La demande est close : plus rien a attendre.
    let r2 = jouer(&c, &o, Clients { courriel: Some(&mut boite as &mut dyn Courriel), ..Clients::default() });
    assert_eq!(r2.fiches.as_ref().unwrap().attendues, 0);
    // Les signatures et les fiches se relevent a la demande seulement.
    let r3 = jouer(&c, &Options { signatures: false, ..o.clone() }, Clients::default());
    assert!(r3.signatures.is_none() && r3.fiches.is_none());
}

#[test]
fn sans_boite_courriel_une_fiche_attendue_est_un_incident() {
    let _t = tour();
    let c = compagnie("fiches-sans-boite", "fictive-3090-petite", true);
    noter_demande_de_fiche(&c);
    let r = sans_clients(&c, &Options { retours: false, filet: false, sauvegarder: false, ..opt() });
    assert!(r.incidents.iter().any(|i| i.contains("Aucune boite courriel")), "{:?}", r.incidents);
    assert!(!r.hors_ligne);
}

// ============================================================================ le filet DPAE

#[test]
fn le_test_automatique_de_la_dpae_du_filet_depose_chez_le_faux_urssaf_une_seule_fois() {
    let _t = tour();
    let c = compagnie("filet", "fictive-1285-petite", false);
    // La compagnie fictive joue d'abord son passe (jusqu'a aujourd'hui) ; puis c'est une compagnie ordinaire.
    sans_clients(&c, &Options { filet: false, ..opt() });
    cie::ordinaire(&c);
    let o = Options { retours: false, signatures: false, sauvegarder: false, ..opt() };
    // Sans mot de passe ni client : rien ne part.
    let r = sans_clients(&c, &o);
    assert_eq!(r.filet.as_ref().unwrap()["testees"].as_array().unwrap().len(), 0);
    // Avec le client Urssaf de test, remplace : le test part chez le faux serveur.
    let (base, _, depots) = organismes(vec![], Pannes::default());
    let r = jouer(&c, &o, Clients { depot_dpae: Some(client_dpae(&c, &base)), ..Clients::default() });
    let testees = r.filet.as_ref().unwrap()["testees"].as_array().unwrap().len();
    assert!(testees >= 1, "{:?}", r.filet);
    assert!(*depots.lock().unwrap() >= 1, "un depot chez le faux Urssaf");
    // Deja teste : plus rien ne part tant que rien ne change.
    let avant = *depots.lock().unwrap();
    let r = jouer(&c, &o, Clients { depot_dpae: Some(client_dpae(&c, &base)), ..Clients::default() });
    assert_eq!(r.filet.as_ref().unwrap()["testees"].as_array().unwrap().len(), 0);
    assert_eq!(*depots.lock().unwrap(), avant);
    // Sans filet : le passage n'a pas lieu.
    let r = sans_clients(&c, &Options { filet: false, ..o });
    assert!(r.filet.is_none());
}

#[test]
fn un_refus_de_l_urssaf_au_test_du_filet_devient_un_incident() {
    let _t = tour();
    let c = compagnie("filet-refus", "fictive-1285-petite", false);
    sans_clients(&c, &Options { filet: false, ..opt() });
    cie::ordinaire(&c);
    let (base, _, _) = organismes(vec![], Pannes { depot: Some(500), ..Pannes::default() });
    let o = Options { retours: false, signatures: false, sauvegarder: false, ..opt() };
    let r = jouer(&c, &o, Clients { depot_dpae: Some(client_dpae(&c, &base)), ..Clients::default() });
    assert!(r.incidents.iter().any(|i| i.starts_with("Test automatique de la DPAE")), "{:?}", r.incidents);
}

// ============================================================================ la sauvegarde

#[test]
fn la_sauvegarde_part_en_dernier_et_la_veille_ne_rappelle_pas_le_drive_quand_rien_n_a_bouge() {
    let _t = tour();
    let c = compagnie("sauvegarde", "fictive-3090-petite", true);
    let (mut d, appels) = drive_compte();
    let r = jouer(&c, &seule_la_sauvegarde(), avec_drive(&mut d));
    let s = r.sauvegarde.as_ref().expect("sauvegarde");
    assert!(!s.deposes.is_empty() && r.a_poser.is_empty());
    assert!(trace(&c)["sauvegarde"].as_str().unwrap().starts_with("Sauvegarde : "));
    // Veille, rien de neuf sur le disque : AUCUN appel.
    let avant = appels.get();
    let r = jouer(&c, &Options { veille: true, ..seule_la_sauvegarde() }, avec_drive(&mut d));
    assert!(r.sauvegarde.is_none());
    assert_eq!(appels.get(), avant, "la veille n'appelle pas le Drive quand rien n'a bouge");
    // Un fichier change : la veille sauvegarde.
    let mut pas = std::fs::read(c.join("config/pas.json")).unwrap();
    pas.push(b' ');
    std::fs::write(c.join("config/pas.json"), pas).unwrap();
    let r = jouer(&c, &Options { veille: true, ..seule_la_sauvegarde() }, avec_drive(&mut d));
    assert_eq!(r.sauvegarde.as_ref().unwrap().deposes.len(), 1);
    // Hors veille, un tour complet : tout est identique.
    let r = jouer(&c, &seule_la_sauvegarde(), avec_drive(&mut d));
    assert_eq!(r.sauvegarde.as_ref().unwrap().deposes.len(), 0);
}

#[test]
fn un_compte_google_non_lie_est_un_manque_meme_en_veille() {
    let _t = tour();
    let c = compagnie("google-non-lie", "fictive-3090-petite", true);
    let r = sans_clients(&c, &seule_la_sauvegarde());
    assert_eq!(r.a_poser, ["google"]);
    assert!(r.manquants[0].starts_with("Compte Google non lie"));
    assert_eq!(trace(&c)["manquants"], json!([]));
    let r = sans_clients(&c, &Options { veille: true, ..seule_la_sauvegarde() });
    assert_eq!(r.a_poser, ["google"]);
}

#[test]
fn un_dossier_reporte_n_est_pas_un_incident_et_ce_qui_dure_se_dit_dans_la_ligne_de_la_sauvegarde() {
    // Tickets #231 et #232 : « sorties/2026-08/envoi/... : HTTP 500 : Internal Error » montait dans le bandeau des retours,
    // et le passage suivant passait. Un dossier reporte une fois se tait ; ce qui dure se dit, a sa place.
    let _t = tour();
    let c = compagnie("drive-incidents", "fictive-3090-petite", true);
    let (mut d, _) = drive_compte();
    let mut e = ErreurGoogle::google("HTTP 500 : Internal Error");
    e.statut = Some(500);
    d.drive.pannes.push(Panne { operation: "lister", motif: "sauvegarde/config".into(), erreur: e });
    let r = jouer(&c, &seule_la_sauvegarde(), avec_drive(&mut d));
    assert!(r.incidents.is_empty(), "{:?}", r.incidents);
    assert!(!r.hors_ligne && r.tout_va_bien());
    assert!(r.sauvegarde.as_ref().unwrap().resume().contains(" reportes au prochain passage."), "{}", r.sauvegarde.as_ref().unwrap().resume());
    assert_eq!(trace(&c)["incidents"], json!([]));
    assert_eq!(trace(&c)["sauvegarde_souci"], json!(""));
    // Trois passages de suite : la phrase arrive dans la trace (`sauvegarde_souci`), jamais dans les incidents.
    jouer(&c, &seule_la_sauvegarde(), avec_drive(&mut d));
    let r = jouer(&c, &seule_la_sauvegarde(), avec_drive(&mut d));
    assert!(r.incidents.is_empty(), "{:?}", r.incidents);
    let souci = trace(&c)["sauvegarde_souci"].as_str().unwrap().to_string();
    assert!(
        souci.starts_with("Sauvegarde : ") && souci.contains("pas pu être envoy") && souci.contains(" depuis ") && souci.ends_with("(Google indisponible) ; le logiciel réessaie."),
        "{}",
        souci
    );
    assert_eq!(r.en_json()["sauvegarde"]["souci"], json!(souci));
    assert_eq!(trace(&c)["incidents"], json!([]));
}

#[test]
fn defaut_en_veille_la_racine_du_drive_compte() {
    // Le Python ignorait la racine dans `rien_n_a_bouge` : apres une sauvegarde dans « PuppetPaie », une veille vers une
    // autre racine ne sauvegardait rien alors que ce Drive-la est vide. Le Rust compare avec la racine demandee.
    let _t = tour();
    let c = compagnie("racine-veille", "fictive-3090-petite", true);
    let (mut d, _) = drive_compte();
    let r = jouer(&c, &seule_la_sauvegarde(), avec_drive(&mut d));
    assert!(r.sauvegarde.is_some());
    let r = jouer(&c, &Options { veille: true, racine_drive: "AutreRacine".into(), ..seule_la_sauvegarde() }, avec_drive(&mut d));
    assert!(r.sauvegarde.is_some(), "une autre racine : tout est a envoyer");
    assert_eq!(r.sauvegarde.as_ref().unwrap().racine_drive, "AutreRacine");
}

// ============================================================================ pas_avant

#[test]
fn defaut_pas_avant_retient_vraiment_et_ne_reecrit_pas_la_trace() {
    // Le Python relisait `quand` (ecrit « JJ/MM/AAAA HH:MM ») avec « %Y-%m-%d %H:%M » : ValueError rattrapee par un `pass`,
    // donc `--pas-avant N` ne retenait jamais rien. Decision du 07/10/2026 : il retient.
    let _t = tour();
    let c = compagnie("pas-avant", "fictive-3090-petite", true);
    let o = Options { pas_avant: 60, ..seule_la_trace() };
    let r = sans_clients(&c, &o);
    assert!(!r.retenue, "la premiere ouverture se fait");
    let avant = trace_octets(&c);
    let r = sans_clients(&c, &Options { sauvegarder: true, ..o.clone() });
    assert!(r.retenue);
    assert_eq!(r.incidents, ["Ouverture deja faite il y a 0 minute(s) : rien a refaire."]);
    assert!(r.a_poser.is_empty() && r.manquants.is_empty(), "rien n'a ete refait");
    assert_eq!(trace_octets(&c), avant, "la trace n'est pas reecrite");
    assert_eq!(r.en_json()["retenue"], json!(true));
}

#[test]
fn pas_avant_ne_retient_pas_une_trace_plus_vieille_illisible_ou_absente() {
    let _t = tour();
    let c = compagnie("pas-avant-ne-retient-pas", "fictive-3090-petite", true);
    let o = Options { pas_avant: 60, sauvegarder: true, ..seule_la_trace() };
    let ecrire_quand = |quand: Value| {
        ecrire(&c, "config/.ouverture.json", &json!({"quand": quand, "secondes": 0.1}));
    };
    let jj = |secondes: i64| -> String {
        let iso = paie::releve_retours::iso_de_naif(maintenant_naif() - secondes);
        format!("{}/{}/{} {}:{}", &iso[8..10], &iso[5..7], &iso[0..4], &iso[11..13], &iso[14..16])
    };
    // Deux heures : on refait.
    ecrire_quand(json!(jj(7_200)));
    assert!(!sans_clients(&c, &o).retenue);
    // Dix minutes, a l'ancienne mode (AAAA-MM-JJ HH:MM) : retenu.
    let iso = paie::releve_retours::iso_de_naif(maintenant_naif() - 600);
    ecrire_quand(json!(format!("{} {}", &iso[0..10], &iso[11..16])));
    let r = sans_clients(&c, &o);
    assert!(r.retenue && r.incidents[0].starts_with("Ouverture deja faite il y a "), "{:?}", r.incidents);
    // Illisible, absente, d'un autre type : on refait.
    ecrire_quand(json!("hier soir"));
    assert!(!sans_clients(&c, &o).retenue);
    ecrire(&c, "config/.ouverture.json", &json!({"secondes": 0.1}));
    assert!(!sans_clients(&c, &o).retenue);
    ecrire_quand(json!(42));
    assert!(!sans_clients(&c, &o).retenue);
    std::fs::write(c.join("config/.ouverture.json"), "{ pas du json").unwrap();
    assert!(!sans_clients(&c, &o).retenue);
    // pas_avant a zero : jamais retenu, meme tout frais.
    sans_clients(&c, &seule_la_trace());
    assert!(!sans_clients(&c, &Options { pas_avant: 0, ..o }).retenue);
}

// ============================================================================ la trace

#[test]
fn derniere_ouverture_est_vide_quand_la_trace_manque_ou_ne_se_lit_pas() {
    let _t = tour();
    let c = compagnie("derniere", "fictive-3090-petite", true);
    let ctx = contexte(&c);
    let p = c.join("config/.ouverture.json");
    assert_eq!(derniere_ouverture(&ctx), json!({}), "absente");
    for (octets, attendu) in [
        (&b"{ pas du json"[..], json!({})),
        (&b"\xff\xfe\x00\xd8 rien"[..], json!({})),
        (&b""[..], json!({})),
        (&b"{}"[..], json!({})),
        (&b"{\"quand\": \"01/10/2026 10:00\"}"[..], json!({"quand": "01/10/2026 10:00"})),
    ] {
        std::fs::write(&p, octets).unwrap();
        assert_eq!(derniere_ouverture(&ctx), attendu);
    }
    // DEFAUT DU PYTHON : un dossier a la place du fichier faisait lever `IsADirectoryError`.
    std::fs::remove_file(&p).unwrap();
    std::fs::create_dir(&p).unwrap();
    assert_eq!(derniere_ouverture(&ctx), json!({}));
    let r = sans_clients(&c, &seule_la_trace());
    assert!(r.incidents.is_empty(), "l'ouverture ne tombe pas, la trace ne s'ecrit pas");
    assert!(p.is_dir());
}

#[test]
fn la_trace_et_le_resultat_portent_toutes_leurs_cles() {
    let _t = tour();
    let c = compagnie("trace-cles", "fictive-3090-petite", true);
    sans_clients(&c, &Options { filet: false, ..opt() });
    let t = trace(&c);
    assert_eq!(
        cles(&t),
        tri([
            "quand", "secondes", "sauvegarde", "sauvegarde_souci", "retours", "hors_ligne", "injoignables", "nouveaux", "signatures", "fiches", "signatures_recues", "sources", "maj", "maj_note",
            "maj_attend", "maj_exigee", "maj_etat", "sources_etat", "incidents", "manquants", "a_poser", "taux_pas", "horloge"
        ])
    );
    // `quand` : JJ/MM/AAAA HH:MM, a la minute.
    let q = t["quand"].as_str().unwrap();
    assert!(q.len() == 16 && &q[2..3] == "/" && &q[5..6] == "/" && &q[13..14] == ":", "{}", q);
    let r = sans_clients(&c, &seule_la_trace());
    assert_eq!(
        cles(&r.en_json()),
        tri([
            "quand", "secondes", "sauvegarde", "retours", "signatures", "maj", "sources", "incidents", "manquants", "taux_pas", "retours_dpae", "fiches", "a_poser", "filet",
            "horloge", "hors_ligne", "injoignables", "tout_va_bien", "retenue"
        ])
    );
    assert_eq!(r.en_json()["maj"], Value::Null);
    assert_eq!(r.en_json()["sources"], Value::Null);
}

#[test]
fn les_retours_des_depots_de_test_se_relevent_mais_ne_se_comptent_pas_dans_la_trace() {
    let _t = tour();
    let c = compagnie("retours-de-test", "fictive-3090-petite", true);
    let mut j = lire(&c, "journal/depots.json").as_array().unwrap().clone();
    j.push(json!({"quoi": "dsn", "quand": "2026-10-01T10:00:00", "mois": "2026-09", "fichier": "DSN_test.dsn", "mode": "test", "qui": "essai", "accepte": true, "flux": "F-TEST", "retours": []}));
    ecrire(&c, "journal/depots.json", &Value::Array(j));
    let (base, _, _) = organismes(
        vec![dsn("F-TEST", "11", BILAN), dsn("F-REEL", "11", BILAN)],
        Pannes::default(),
    );
    let o = Options { filet: false, sauvegarder: false, signatures: false, ..opt() };
    let r = jouer(&c, &o, les_deux(&c, &base));
    let rel = r.releve.as_ref().unwrap();
    assert_eq!((rel.trouves.len(), rel.nouveaux.len()), (2, 2), "le resultat les compte");
    let t = trace(&c);
    assert_eq!(t["nouveaux"], json!(1));
    assert_eq!(t["retours"], json!("1 retour(s), dont 1 nouveau(x)."), "la trace ecarte le retour du depot de test");
}

#[test]
fn defaut_une_trace_qui_ne_s_ecrit_pas_ne_fait_pas_tomber_l_ouverture() {
    // Un dossier a la place de la trace : l'ecriture echoue (`except OSError: pass` du Python, mais celui-ci levait avant, a la lecture).
    let _t = tour();
    let c = compagnie("trace-dossier", "fictive-3090-petite", true);
    std::fs::create_dir(c.join("config/.ouverture.json")).unwrap();
    let r = sans_clients(&c, &Options { filet: false, ..opt() });
    assert_eq!(r.a_poser.len(), 3);
}

// ============================================================================ les decisions du 07/10/2026

#[test]
fn decision_2_la_trace_garde_les_champs_maj_et_sources_de_la_trace_precedente() {
    let _t = tour();
    let c = compagnie("garde-maj", "fictive-3090-petite", true);
    let avant = json!({
        "quand": "01/10/2026 10:00", "secondes": 0.5, "sources": "3 sources a relire", "maj": "Version 2026.10.09 prete", "maj_note": "un calcul change",
        "maj_attend": ["2026-09"], "maj_exigee": "norme DSN", "maj_etat": {"courante": "2026.10.02", "derniere": "2026.10.09", "prete": true, "detail": "d"},
        "sources_etat": {"le": "2026-10-05", "total": 12, "a_relire": ["x"], "injoignables": [], "details": {"x": "y"}}
    });
    ecrire(&c, "config/.ouverture.json", &avant);
    sans_clients(&c, &seule_la_trace());
    let t = trace(&c);
    for k in ["sources", "maj", "maj_note", "maj_attend", "maj_exigee", "maj_etat", "sources_etat"] {
        assert_eq!(t[k], avant[k], "{} gardé", k);
    }
    // Les champs de ce passage, eux, sont neufs.
    assert_ne!(t["quand"], avant["quand"]);
    // Sans trace precedente: les valeurs vides d'une trace neuve.
    std::fs::remove_file(c.join("config/.ouverture.json")).unwrap();
    sans_clients(&c, &seule_la_trace());
    let t = trace(&c);
    assert_eq!((t["sources"].clone(), t["maj"].clone(), t["maj_attend"].clone(), t["maj_etat"].clone()), (json!(""), json!(""), json!([]), json!({})));
}

#[test]
fn decision_3_incidents_et_reponses_de_l_editeur_partent_seulement_quand_l_appelant_ne_s_en_charge_pas() {
    let _t = tour();
    const ATTEND: &str = "20261005T100000-000000000001";
    // Le faux serveur de l'editeur : `/rapport` recoit les incidents, `/reponses` rend la reponse a un signalement.
    let (url, recu) = serveur(|q: &Requete| {
        let v = q.json();
        let rep = match q.chemin.trim_matches('/') {
            "poste" => json!({"poste": "a1b2c3d4e5f60718a1b2c3d4e5f60718", "cle": "0f1e2d3c4b5a6978".repeat(4)}),
            "rapport" => json!({"recus": v["incidents"].as_array().into_iter().flatten().map(|x| json!([x["empreinte"], x["quand"]])).collect::<Vec<_>>()}),
            "reponses" => json!({"retours": [{"numero": "R-000001", "reponse": "Merci, c'est corrige.", "repondu_le": "2026-10-06"}]}),
            "retour" => json!({"numero": "R-000002"}),
            _ => json!({}),
        };
        Some(http_json(200, &rep))
    });
    let fabriquer = |nom: &str| -> PathBuf {
        let c = compagnie(nom, "fictive-3090-petite", true);
        let lignes: Vec<Value> = (0..5)
            .map(|i| {
                json!({
                    "quoi": "logiciel", "quand": format!("2026-10-05T10:00:{:02}", i), "version": "2026.10.02", "empreinte": format!("e{:02}", i % 2),
                    "ou": format!("calcul.py:{}", i), "genre": "ValueError", "resume": "ValueError: boom", "detail": {"i": i}, "trace": "Traceback",
                    "caviarde": [], "envoi": {"etat": "en-attente", "quand": null},
                })
            })
            .collect();
        ecrire(&c, "journal/incidents.json", &Value::Array(lignes));
        // Et un message « Un probleme, une idee ? » que l'envoi en fond n'a pas fait partir : la veille le reprend.
        ecrire(&c, "journal/signalements.json", &json!([
            {"numero": "R-000001", "quand": "2026-10-01T10:00:00", "texte": "un probleme", "reponse": "", "lu": false},
            {"id": ATTEND, "quand": "2026-10-05T10:00", "debut": "en attente", "envoi": {"etat": "en-attente", "quand": null, "essai": null, "erreur": "injoignable"}},
        ]));
        ecrire(&c, &format!("journal/signalements/{}.json", ATTEND), &json!({"texte": "en attente"}));
        ecrire(&c, "config/filet-dpae.json", &json!({"serveur": url}));
        c
    };
    let actions = |avant: usize| -> Vec<String> { recu.lock().unwrap()[avant..].iter().map(|q| q.chemin.trim_matches('/').to_string()).collect() };
    let o = Options { incidents_editeur: true, ..seule_la_trace() };
    // Part : un rapport, puis les reponses aux signalements.
    let c = fabriquer("editeur-part");
    let n = recu.lock().unwrap().len();
    let o_filet = Options { filet: true, ..o.clone() };
    sans_clients(&c, &o_filet);
    assert_eq!(actions(n), ["poste", "rapport", "paies", "retour", "reponses"], "l'identite du poste d'abord (serveur version 7), et le nombre de paies de 2025 (ticket #246 : apres le 16 janvier)");
    let attendait = lire(&c, "journal/signalements.json")[1].clone();
    assert_eq!((attendait["envoi"]["etat"].clone(), attendait["numero"].clone()), (json!("depose"), json!("R-000002")));
    assert!(!c.join(format!("journal/signalements/{}.json", ATTEND)).exists(), "parti, son corps s'efface");
    assert!(lire(&c, "journal/incidents.json").as_array().unwrap().iter().take(5).all(|l| l["envoi"]["etat"] == json!("depose")));
    assert!(lire(&c, "journal/signalements.json")[0]["reponse"].as_str().unwrap().starts_with("Merci"));
    // Rien de plus a envoyer.
    let n = recu.lock().unwrap().len();
    sans_clients(&c, &o_filet);
    assert!(!actions(n).contains(&"rapport".to_string()));
    // `incidents_editeur: false` : l'appelant s'en charge (la veille de la fenetre), rien ne part.
    let c = fabriquer("editeur-appelant");
    let n = recu.lock().unwrap().len();
    sans_clients(&c, &Options { incidents_editeur: false, ..o_filet.clone() });
    assert_eq!(actions(n), Vec::<String>::new());
    // Sans filet : rien ne part.
    sans_clients(&c, &Options { filet: false, ..o.clone() });
    assert_eq!(actions(n), Vec::<String>::new());
    // `PUPPETPAIE_INCIDENTS` pose (un bac a sable) : rien ne part.
    std::env::set_var("PUPPETPAIE_INCIDENTS", c.join("journal/incidents-bac.json"));
    sans_clients(&c, &o_filet);
    std::env::remove_var("PUPPETPAIE_INCIDENTS");
    assert_eq!(actions(n), Vec::<String>::new());
    // Une copie de developpement : ses refus sont ceux qu'on provoque, rien ne part.
    let d = fabriquer("editeur-copie");
    ecrire(&d, "COPIE-DE-DEVELOPPEMENT.json", &json!({"copie_de": "compagnie ordinaire du poste (fictive)", "quand": "2026-10-01T10:00:00"}));
    sans_clients(&d, &o_filet);
    assert_eq!(actions(n), Vec::<String>::new());
}

// ============================================================================ la veille en deux appels (decision 4)

fn les_retours_et_le_rappel() -> Options {
    Options { sauvegarder: false, signatures: false, filet: false, veille: true, ..opt() }
}

#[test]
fn decision_4_apres_releve_est_appele_une_fois_avant_la_sauvegarde_la_trace_deja_ecrite() {
    let _t = tour();
    let c = compagnie("apres-releve", "fictive-3090-petite", true);
    let (mut d, appels) = drive_compte();
    let vus = RefCell::new(Vec::new());
    let mut rappel = || {
        let t = std::fs::read(c.join("config/.ouverture.json")).ok().and_then(|o| serde_json::from_slice::<Value>(&o).ok());
        vus.borrow_mut().push((appels.get(), t));
    };
    let r = ouverture(&options_de(&c), &contexte(&c), &seule_la_sauvegarde(), avec_drive(&mut d), Some(&mut rappel));
    assert!(r.sauvegarde.is_some());
    let vus = vus.into_inner();
    assert_eq!(vus.len(), 1, "appele une seule fois");
    assert_eq!(vus[0].0, 0, "avant le premier appel au Drive");
    let t = vus[0].1.as_ref().expect("la trace est deja ecrite");
    assert_eq!(t["sauvegarde"], json!(""), "sans la sauvegarde, qui vient apres");
    assert!(trace(&c)["sauvegarde"].as_str().unwrap().starts_with("Sauvegarde : "), "puis la trace est refaite a la fin");
    // Un passage retenu n'appelle rien.
    let appele = Cell::new(0);
    let mut rappel2 = || appele.set(appele.get() + 1);
    ouverture(&options_de(&c), &contexte(&c), &Options { pas_avant: 60, ..seule_la_trace() }, Clients::default(), Some(&mut rappel2));
    assert_eq!(appele.get(), 0);
}

#[test]
fn decision_4_la_seconde_moitie_complete_la_trace_sans_rien_effacer() {
    let _t = tour();
    let c = compagnie("deux-appels", "fictive-3090-petite", true);
    let (base, _, _) = organismes(vec![dsn("F-ATMP", "34", ATMP)], Pannes::default());
    // Appel 1 : la releve, sans sauvegarde.
    let r1 = jouer(&c, &les_retours_et_le_rappel(), les_deux(&c, &base));
    assert!(r1.sauvegarde.is_none());
    let t1 = trace(&c);
    assert_eq!(t1["retours"], json!("1 retour(s), dont 1 nouveau(x)."));
    assert_eq!(t1["a_poser"], json!([]));
    // Appel 2 : la sauvegarde seule, qui complete.
    let (mut d, _) = drive_compte();
    let r2 = jouer(&c, &Options { sauvegarde_seule: true, veille: true, sauvegarder: true, ..opt() }, avec_drive(&mut d));
    assert!(r2.sauvegarde.is_some());
    let t2 = trace(&c);
    assert!(t2["sauvegarde"].as_str().unwrap().starts_with("Sauvegarde : "));
    for k in ["retours", "nouveaux", "manquants", "incidents", "signatures", "fiches", "horloge", "taux_pas"] {
        assert_eq!(t2[k], t1[k], "{} : le premier appel n'est pas efface", k);
    }
    assert_eq!(t2["quand"], t1["quand"]);
    assert!(t2["secondes"].as_f64().unwrap() >= t1["secondes"].as_f64().unwrap(), "les secondes se cumulent");
    assert_eq!(cles(&t2), cles(&t1));
}

#[test]
fn decision_4_le_hors_ligne_ne_vient_que_de_la_seconde_moitie() {
    let _t = tour();
    let c = compagnie("deux-appels-hors-ligne", "fictive-3090-petite", true);
    let (base, _, _) = organismes(vec![dsn("F-ATMP", "34", ATMP)], Pannes::default());
    jouer(&c, &les_retours_et_le_rappel(), les_deux(&c, &base));
    let t1 = trace(&c);
    assert!(!t1["retours"].as_str().unwrap().is_empty());
    let (mut d, _) = drive_compte();
    d.drive.pannes.push(Panne { operation: "chemin", motif: "".into(), erreur: ErreurGoogle::google("Reseau injoignable : [Errno 111] Connection refused") });
    let r2 = jouer(&c, &Options { sauvegarde_seule: true, sauvegarder: true, ..opt() }, avec_drive(&mut d));
    assert!(r2.hors_ligne);
    let t2 = trace(&c);
    assert_eq!(t2["hors_ligne"], json!(true));
    // Le Drive qui ne repond pas ne tait plus la releve passee (Leandre, 21/10/2026) ; il se nomme.
    assert_eq!(t2["retours"], t1["retours"], "la releve du premier appel reste dite");
    assert!(t2["injoignables"].as_array().unwrap().iter().any(|i| i.as_str().unwrap().contains("Drive")), "{}", t2);
    assert_eq!(t2["incidents"], t1["incidents"], "l'erreur de reseau n'est pas un incident");
    assert_eq!(t2["manquants"], t1["manquants"]);
}

#[test]
fn decision_4_le_manque_google_de_la_seconde_moitie_s_ajoute_a_a_poser() {
    let _t = tour();
    let c = compagnie("deux-appels-google", "fictive-3090-petite", true);
    let (base, _, _) = organismes(vec![], Pannes::default());
    jouer(&c, &les_retours_et_le_rappel(), les_deux(&c, &base));
    assert_eq!(trace(&c)["a_poser"], json!([]));
    let r2 = sans_clients(&c, &Options { sauvegarde_seule: true, sauvegarder: true, ..opt() });
    assert_eq!(r2.a_poser, ["google"]);
    let t = trace(&c);
    assert_eq!(t["a_poser"], json!(["google"]));
    assert_eq!(t["manquants"], json!([]), "la phrase du coffre n'est pas dans la trace");
}

#[test]
fn decision_4_la_seconde_moitie_ne_fait_rien_dans_une_fictive_en_copie_ou_sans_sauvegarde() {
    let _t = tour();
    // Une compagnie fictive.
    let d = compagnie("seule-fictive", "fictive-3090-petite", false);
    sans_clients(&d, &Options { filet: false, ..opt() });
    let avant = trace_octets(&d);
    let r = sans_clients(&d, &Options { sauvegarde_seule: true, ..opt() });
    assert!(r.sauvegarde.is_none() && r.a_poser.is_empty());
    assert_eq!(trace_octets(&d), avant);
    // Copie de developpement.
    let c = compagnie("seule-copie", "fictive-3090-petite", true);
    ecrire(&c, "COPIE-DE-DEVELOPPEMENT.json", &json!({"copie_de": "compagnie ordinaire du poste (fictive)", "quand": "2026-10-01T10:00:00"}));
    sans_clients(&c, &seule_la_trace());
    let avant = trace_octets(&c);
    let (mut dr, appels) = drive_compte();
    jouer(&c, &Options { sauvegarde_seule: true, ..opt() }, avec_drive(&mut dr));
    assert_eq!(appels.get(), 0);
    assert_eq!(trace_octets(&c), avant);
    // `sauvegarder: false`.
    let o = compagnie("seule-sans", "fictive-3090-petite", true);
    sans_clients(&o, &seule_la_trace());
    let avant = trace_octets(&o);
    jouer(&o, &Options { sauvegarde_seule: true, sauvegarder: false, ..opt() }, avec_drive(&mut dr));
    assert_eq!(trace_octets(&o), avant);
    // Une trace qui manque : la trace ordinaire de ce passage.
    let n = compagnie("seule-sans-trace", "fictive-3090-petite", true);
    let r = jouer(&n, &Options { sauvegarde_seule: true, ..opt() }, avec_drive(&mut dr));
    assert!(r.sauvegarde.is_some());
    assert!(trace(&n)["sauvegarde"].as_str().unwrap().starts_with("Sauvegarde : "));
    // La seconde moitie garde aussi maj et sources.
    let g = compagnie("seule-garde", "fictive-3090-petite", true);
    ecrire(&g, "config/.ouverture.json", &json!({"quand": "01/10/2026 10:00", "secondes": 1.0, "retours": "x", "incidents": [], "manquants": [], "a_poser": [], "maj": "prete", "sources": "s"}));
    jouer(&g, &Options { sauvegarde_seule: true, ..opt() }, avec_drive(&mut dr));
    let t = trace(&g);
    assert_eq!((t["maj"].clone(), t["sources"].clone()), (json!("prete"), json!("s")));
}

// ============================================================================ les options et les routes

#[test]
fn les_options_de_la_charge() {
    let d = Options::de_la_charge(&json!({}));
    assert!(d.sauvegarder && d.retours && d.signatures && d.filet && d.incidents_editeur);
    assert!(!d.veille && !d.sauvegarde_seule);
    assert_eq!((d.jours, d.pas_avant, d.racine_drive.as_str()), (30, 0, "PuppetPaie"));
    let o = Options::de_la_charge(&json!({
        "sauvegarder": false, "retours": false, "signatures": false, "filet": false, "jours": 7, "racine_drive": "Autre", "pas_avant": 15, "veille": true,
        "incidents_editeur": false, "sauvegarde_seule": true, "sources": false, "maj": false
    }));
    assert!(!o.sauvegarder && !o.retours && !o.signatures && !o.filet && o.veille && !o.incidents_editeur && o.sauvegarde_seule);
    assert_eq!((o.jours, o.pas_avant, o.racine_drive.as_str()), (7, 15, "Autre"));
    // Des valeurs de travers : les defauts, jamais une erreur.
    let o = Options::de_la_charge(&json!({"jours": "abc", "pas_avant": null, "racine_drive": "", "sauvegarder": null, "veille": 0, "filet": "oui"}));
    assert_eq!((o.jours, o.pas_avant, o.racine_drive.as_str()), (30, 0, "PuppetPaie"));
    assert!(o.sauvegarder && !o.veille && o.filet);
    assert_eq!(Options::de_la_charge(&json!({"jours": 7.9})).jours, 7);
    for charge in [json!(null), json!("x"), json!([1]), json!(3)] {
        assert_eq!(Options::de_la_charge(&charge).jours, 30);
    }
}

/// `paie route` : la reponse entiere, sous un environnement nettoye (jamais un mot de passe, jamais le poste de l'utilisateur).
fn route(racine: &Path, nom: &str, charge: Value) -> Value {
    let base = poste();
    let mut p = Command::new(env!("CARGO_BIN_EXE_paie"))
        .args(["route", "--racine", racine.to_str().unwrap(), "--date", AUJOURDHUI])
        .env_clear()
        .env(paie::garde_reseau::VARIABLE, "1") // le garde-fou reseau des essais : rien ne sort de la machine
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", base.join("home"))
        .env("TZ", "Europe/Paris")
        .env("PUPPETPAIE_CODE", base.join("logiciel/donnees"))
        .env("PUPPETPAIE_AUJOURDHUI", AUJOURDHUI)
        .env("PUPPETPAIE_CLE", base.join("cle"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    p.stdin.take().unwrap().write_all(json!({"route": nom, "charge": charge}).to_string().as_bytes()).unwrap();
    let sortie = p.wait_with_output().unwrap().stdout;
    serde_json::from_slice(&sortie).unwrap_or_else(|_| panic!("reponse illisible : {}", String::from_utf8_lossy(&sortie)))
}

#[test]
fn la_route_ouverture_ne_rend_jamais_d_erreur_et_la_derniere_trace_se_relit() {
    let _t = tour();
    let c = compagnie("route", "fictive-3090-petite", false);
    // La trace d'avant : absente.
    let r = route(&c, "ouverture-derniere", json!({}));
    assert_eq!((r["ok"].clone(), r["resultat"].clone()), (json!(true), json!({})));
    // Une compagnie fictive, avec n'importe quelle charge, y compris de travers : jamais une erreur.
    for charge in [json!({}), json!(null), json!("x"), json!([1]), json!({"jours": "abc", "racine_drive": 5, "pas_avant": "x", "sources": true, "maj": true, "incidents_editeur": false})] {
        let r = route(&c, "ouverture", charge.clone());
        assert_eq!(r["ok"], json!(true), "{} : {}", charge, r);
        assert!(r["resultat"]["incidents"].is_array());
    }
    // Les options sont prises : `retenue` et la trace relue.
    let r = route(&c, "ouverture", json!({"pas_avant": 60, "incidents_editeur": false}));
    assert_eq!(r["resultat"]["retenue"], json!(true));
    let d = route(&c, "ouverture-derniere", json!({}));
    assert_eq!(d["resultat"], trace(&c));
    assert_eq!(d["resultat"]["maj"], json!(""));
    // Une compagnie ordinaire, coffre vide et environnement nettoye : les manques, rien ne part.
    let o = compagnie("route-ordinaire", "fictive-3090-petite", true);
    let r = route(&o, "ouverture", json!({"filet": false, "incidents_editeur": false}));
    assert_eq!(r["ok"], json!(true), "{}", r);
    assert_eq!(r["resultat"]["a_poser"], json!(["net-entreprises", "urssaf", "google"]));
    assert_eq!(r["resultat"]["tout_va_bien"], json!(false));
    // Un coffre illisible, un journal casse : toujours `ok`.
    std::fs::create_dir(o.join("config/coffre.bin")).unwrap();
    std::fs::write(o.join("journal/depots.json"), "{ casse").unwrap();
    let r = route(&o, "ouverture", json!({"filet": false, "incidents_editeur": false}));
    assert_eq!(r["ok"], json!(true), "{}", r);
}

// ============================================================================ l'etape en cours (ticket #220)

/// Un faux Drive qui note, a chaque appel, l'etape que le passage a ecrite (`ouverture::etape_en_cours`).
struct DriveEtape {
    drive: FauxDrive,
    racine: PathBuf,
    vues: Rc<RefCell<Vec<String>>>,
}

impl DriveEtape {
    fn noter(&self) {
        if let Some(e) = paie::ouverture::etape_en_cours(&self.racine) {
            if self.vues.borrow().last() != Some(&e) {
                self.vues.borrow_mut().push(e);
            }
        }
    }
}

impl Drive for DriveEtape {
    fn compte(&self) -> Option<String> {
        self.drive.compte()
    }
    fn chemin(&mut self, noms: &[&str]) -> Result<String, ErreurGoogle> {
        self.noter();
        self.drive.chemin(noms)
    }
    fn lister(&mut self, parent: &str) -> Result<std::collections::BTreeMap<String, Distant>, ErreurGoogle> {
        self.noter();
        self.drive.lister(parent)
    }
    fn televerser(&mut self, fichier: &Path, parent: &str, nom: Option<&str>, connu: Option<&Distant>) -> Result<Depose, ErreurGoogle> {
        self.noter();
        assert!(!fichier.ends_with(paie::ouverture::ETAPE), "l'etape ne monte pas au Drive");
        self.drive.televerser(fichier, parent, nom, connu)
    }
    fn sous_dossiers(&mut self, parent: &str) -> Result<Vec<(String, String)>, ErreurGoogle> {
        self.noter();
        self.drive.sous_dossiers(parent)
    }
    fn telecharger(&mut self, id: &str) -> Result<Vec<u8>, ErreurGoogle> {
        self.noter();
        self.drive.telecharger(id)
    }
}

/// Ticket #220 (Leandre, 20/10/2026 : « qui on appelle, puis qui d'autre, la sauvegarde ») : chaque etape d'un passage
/// s'ecrit en y entrant -- net-entreprises, puis l'Urssaf, puis le Drive --, ce que la veille de la fenetre lit pendant
/// qu'il tourne ; le passage fini, il n'en reste rien.
#[test]
fn chaque_etape_d_un_passage_se_dit_en_y_entrant_et_s_efface_a_la_fin() {
    let _t = tour();
    let c = compagnie("etapes", "fictive-3090-petite", true);
    let vues: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let (racine, au_serveur) = (c.clone(), Arc::new(Mutex::new(Vec::<String>::new())));
    let a2 = au_serveur.clone();
    let (base, _) = serveur(move |q: &Requete| {
        if let Some(e) = paie::ouverture::etape_en_cours(&racine) {
            let mut v = a2.lock().unwrap();
            if v.last() != Some(&e) {
                v.push(e);
            }
        }
        Some(if q.chemin.contains("authentifier") { http(200, "text/plain", b"JETON-FICTIF") } else { http(404, "text/plain", b"Aucun retour sur cette plage") })
    });
    let mut d = DriveEtape { drive: FauxDrive::nouveau(), racine: c.clone(), vues: vues.clone() };
    let o = Options { filet: false, signatures: false, ..opt() };
    let mut clients = les_deux(&c, &base);
    clients.drive = Some(&mut d as &mut dyn Drive);
    let r = jouer(&c, &o, clients);
    assert!(r.sauvegarde.is_some(), "{:?}", r.incidents);
    assert_eq!(*au_serveur.lock().unwrap(), ["net-entreprises : retours des DSN…", "Urssaf : retours des DPAE…"]);
    assert_eq!(*vues.borrow(), ["Google Drive : sauvegarde du dossier…"]);
    assert_eq!(paie::ouverture::etape_en_cours(&c), None, "le passage fini, plus d'etape");
    assert!(!c.join("config").join(paie::ouverture::ETAPE).exists());
}
