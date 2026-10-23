//! L'ouverture du logiciel et la veille (`moteur/operations_ouverture.py` : `ouverture`, `Ouverture`,
//! `derniere_ouverture`, `_noter_ouverture`, `_trier_hors_ligne`, `_google_pret`). Lot I2b.
//!
//! PROUVE contre l'ancien Python (harnais `prive/journal/i/i2b-harnais/`), TESTE (`tests/ouverture.rs`), BRANCHE (`web.py`,
//! `commandes_ouverture.py`). Ce que le logiciel fait tout seul quand on l'ouvre -- interface ou ligne de commande,
//! c'est la meme fonction -- et toutes les vingt minutes ensuite : l'horloge de la compagnie fictive, les retours DSN et DPAE,
//! les taux de PAS, les signatures et les fiches, le filet DPAE, les incidents et les reponses de l'editeur, la
//! sauvegarde en dernier, puis la trace `config/.ouverture.json`.
//!
//! REGLE : L'OUVERTURE N'ECHOUE JAMAIS. `ouverture` ne rend pas de `Result` : pas de reseau, pas de compte Google, pas de
//! secret pose, un coffre qui refuse de s'ouvrir -- elle le NOTE (un incident, un manque, `a_poser`) et rend la main.
//! La paie se calcule tres bien hors ligne.
//!
//! RIEN N'EST REECRIT : chaque acte est celui qui existe deja, appele tel quel --
//! `releve_retours::{relever_retours, relever_retours_dpae, rattacher_simulation}` (lot H2),
//! `releve_signatures::{relever_signatures, relever_signatures_fictive}` (lot I2a),
//! `routes_fiches_courriel::relever_fiches` (H4a), `horloge::{rattraper, dernier_rattrapage}` (I1),
//! `pas::avertissements`, `routes_actes_h2::filet_preparer` (le passage du filet DPAE, ouvert en `pub(crate)`),
//! `incidents::envoyer` et `problemes::relever_reponses` (le canal de l'editeur), `sauvegarde::{sauvegarder,
//! rien_n_a_bouge, hors_ligne}` (H4b), `routes_google::autorise` (ouvert ici), `coffre::Coffre`.
//!
//! CHAQUE CLIENT RESEAU PEUT ETRE REMPLACE (`Clients`) : le compte net-entreprises, le compte Urssaf (DPAE), la boite
//! courriel (signatures et fiches), le Drive. Un client donne est utilise a la place du vrai, SANS regarder le coffre.
//! Le serveur des incidents est celui de la configuration (`incidents::configuration`), qu'un serveur local peut tenir.
//! Le test automatique de la DPAE du filet a son client Urssaf remplacable (`Clients::depot_dpae`).
//!
//! NE FONT PAS PARTIE DE CE LOT (lot J, avec Tauri) : la recherche de mise a jour (`maj`, `MiseAJour`) et la verification
//! des sources (`_veiller_sources`, `VeilleSources`). Les options `sources` et `maj` de la route sont acceptees et
//! ignorees ; `maj` et `sources` valent `null` dans le resultat, et la trace GARDE les champs `maj*` et `sources*` de la
//! trace precedente (l'appelant y ecrit encore le resultat de la verification de version, en Python).
//!
//! Les phrases des incidents sont celles du Python ; les phrases des pannes (venues des modules appeles) peuvent differer.

use std::collections::HashMap;
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::contexte::Contexte;
use crate::courriel::Courriel;
use crate::drive::Drive;
use crate::etats::{self, py_str, R};
use crate::fichiers::vrai;
use crate::releve_retours::{self, Client, Releve};
use crate::releve_signatures::{self, ReleveSignatures};
use crate::retours::Echec;
use crate::routes_fiches_courriel::{self, ReleveFiches};
use crate::sauvegarde::{self, Sauvegarde};

/// Les routes de ce module.
pub const ROUTES: [&str; 2] = ["ouverture", "ouverture-derniere"];

/// `compagnies.MARQUEUR`.
const MARQUEUR_COPIE: &str = "COPIE-DE-DEVELOPPEMENT.json";

/// La trace de la derniere ouverture : `config/.ouverture.json`.
const TRACE: &str = ".ouverture.json";

/// Les champs de la trace qui sont au lot J (la mise a jour, les sources) : l'ouverture Rust les garde de la trace precedente.
/// Ordre de la trace du Python : `sources`, `maj`, `maj_note`, `maj_attend`, `maj_exigee`, `maj_etat`, `sources_etat`.
const LOT_J: &[(&str, fn() -> Value)] = &[
    ("sources", || json!("")),
    ("maj", || json!("")),
    ("maj_note", || json!("")),
    ("maj_attend", || json!([])),
    ("maj_exigee", || json!("")),
    ("maj_etat", || json!({})),
    ("sources_etat", || json!({})),
];

/// Secondes entre un depot joue et son compte rendu, dans une compagnie fictive (`simulation.DELAI`).
const DELAI_RETOURS_FICTIVE: i64 = 6;

// ------------------------------------------------------------------ les options et les clients

/// Les options de `ouverture(...)`. `sources` et `maj` du Python n'y sont pas : lot J.
#[derive(Debug, Clone)]
pub struct Options {
    /// Sauvegarder le dossier prive dans le Drive (en dernier).
    pub sauvegarder: bool,
    /// Relever les retours DSN et DPAE.
    pub retours: bool,
    /// Relever les signatures et les fiches.
    pub signatures: bool,
    /// Le filet DPAE (le passage), et ce qui part chez l'editeur (incidents, reponses aux signalements).
    pub filet: bool,
    /// Combien de jours en arriere on cherche les retours.
    pub jours: i64,
    /// Le dossier racine dans le Drive.
    pub racine_drive: String,
    /// Ne rien refaire si la derniere ouverture a moins de N minutes (0 : toujours).
    pub pas_avant: i64,
    /// Le passage periodique : saute la sauvegarde quand rien n'a bouge sur le disque.
    pub veille: bool,
    /// Envoyer chez l'editeur ce que le journal des incidents a note, et relever ses reponses. Le Python le lance dans un
    /// fil ; un processus `paie route` ne peut pas attendre un fil. Mettre `false` quand l'appelant le lance lui-meme
    /// (routes `incidents-envoyer` et `signalements-reponses`, dans son propre fil).
    pub incidents_editeur: bool,
    /// La SECONDE moitie de la veille (decision du 07/10/2026) : ne fait que la sauvegarde, et met la trace du premier appel a
    /// jour (sauvegarde, hors ligne, incidents, manque `google`) sans rien effacer de ce qu'il a ecrit. Le premier appel
    /// (`sauvegarder: false`) a releve et ecrit la trace ; l'appelant a dit « releve fait » ; celui-ci sauvegarde, en dernier.
    pub sauvegarde_seule: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            sauvegarder: true,
            retours: true,
            signatures: true,
            filet: true,
            jours: 30,
            racine_drive: sauvegarde::RACINE_DRIVE.to_string(),
            pas_avant: 0,
            veille: false,
            incidents_editeur: true,
            sauvegarde_seule: false,
        }
    }
}

impl Options {
    /// Les options d'une charge de route : `{"sauvegarder", "retours", "signatures", "filet", "jours", "racine_drive",
    /// "pas_avant", "veille", "incidents_editeur", "sauvegarde_seule"}`, chacune facultative. `sources` et `maj` sont acceptees, ignorees.
    pub fn de_la_charge(charge: &Value) -> Options {
        let mut o = Options::default();
        let booleen = |cle: &str, defaut: bool| charge.get(cle).filter(|v| !v.is_null()).map_or(defaut, vrai);
        o.sauvegarder = booleen("sauvegarder", o.sauvegarder);
        o.retours = booleen("retours", o.retours);
        o.signatures = booleen("signatures", o.signatures);
        o.filet = booleen("filet", o.filet);
        o.veille = booleen("veille", o.veille);
        o.incidents_editeur = booleen("incidents_editeur", o.incidents_editeur);
        o.sauvegarde_seule = booleen("sauvegarde_seule", o.sauvegarde_seule);
        let entier = |cle: &str, defaut: i64| charge.get(cle).and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64))).unwrap_or(defaut);
        o.jours = entier("jours", o.jours);
        o.pas_avant = entier("pas_avant", o.pas_avant);
        if let Some(r) = charge.get("racine_drive").filter(|v| vrai(v)).map(py_str) {
            o.racine_drive = r;
        }
        o
    }
}

/// Les clients reseau que l'appelant peut donner a la place des vrais (la preuve : un faux, ou un serveur local). `None` :
/// le vrai, construit ici. Un client donne n'est jamais confronte au coffre.
#[derive(Default)]
pub struct Clients<'a> {
    /// Le compte net-entreprises (retours DSN).
    pub dsn: Option<Client>,
    /// Le compte Urssaf (retours DPAE).
    pub dpae: Option<Client>,
    /// La boite courriel, pour les signatures ET les fiches.
    pub courriel: Option<&'a mut (dyn Courriel + 'a)>,
    /// Le Drive.
    pub drive: Option<&'a mut (dyn Drive + 'a)>,
    /// Le compte Urssaf du TEST AUTOMATIQUE des DPAE du filet (il depose en test) : distinct de `dpae`, qui sert la releve.
    pub depot_dpae: Option<Client>,
}

// ------------------------------------------------------------------ ce que l'ouverture rend

/// `Ouverture` : le compte rendu d'un passage. `maj` et `sources` (lot J) n'y sont pas.
#[derive(Default)]
pub struct Ouverture {
    /// `JJ/MM/AAAA HH:MM`, l'heure de Paris.
    pub quand: String,
    pub sauvegarde: Option<Sauvegarde>,
    /// Les retours DSN (dans une compagnie fictive : un releve vide qui ne porte que `rattaches`).
    pub releve: Option<Releve>,
    pub releve_dpae: Option<Releve>,
    pub signatures: Option<ReleveSignatures>,
    pub fiches: Option<ReleveFiches>,
    pub incidents: Vec<String>,
    pub manquants: Vec<String>,
    /// Ce qui manque au coffre, nomme : `google`, `net-entreprises`, `urssaf`.
    pub a_poser: Vec<String>,
    /// Les phrases de ces manques, a ne pas redire dans la trace (`a_poser` les dit).
    pub phrases_coffre: Vec<String>,
    /// Une phrase par salarie non archive dont le taux de PAS n'est pas (ou plus) celui de la DGFiP.
    pub taux_pas: Vec<String>,
    /// Le passage du filet DPAE : `{testees, armes, desarmes, incidents, occupe}`.
    pub filet: Option<Value>,
    /// Ce que l'horloge de la compagnie fictive a joue aujourd'hui, en phrases.
    pub horloge: Vec<String>,
    pub secondes: f64,
    /// Le reseau manquait : dit une fois, pas par erreur.
    pub hors_ligne: bool,
    /// Ce qui n'a pas repondu, une phrase par service (l'erreur telle qu'elle est venue) : le reseau manquait pour CELA,
    /// pas forcement pour tout -- la releve a pu passer quand le Drive ne repondait pas (21/10/2026).
    pub injoignables: Vec<String>,
    /// `pas_avant` a retenu ce passage (la derniere ouverture est trop recente) : rien n'a ete fait, la trace n'est pas
    /// reecrite. L'appelant qui lance encore la verification de version en Python (lot J) ne la lance pas non plus.
    pub retenue: bool,
}

impl Ouverture {
    /// `Ouverture.tout_va_bien`.
    pub fn tout_va_bien(&self) -> bool {
        self.incidents.is_empty() && self.manquants.is_empty()
    }

    /// `_manque_coffre` : un secret absent -- la phrase du terminal, gardee pour ne pas la redire dans la fenetre.
    fn manque_coffre(&mut self, phrase: &str) {
        self.manquants.push(phrase.to_string());
        self.phrases_coffre.push(phrase.to_string());
    }

    /// `ReleveRetours.resume`.
    fn resume_retours(releve: &Releve) -> String {
        if releve.trouves.is_empty() {
            return "Aucun retour sur la periode.".to_string();
        }
        format!("{} retour(s), dont {} nouveau(x), {} archive(s).", releve.trouves.len(), releve.nouveaux.len(), releve.archives.len())
    }

    /// Ce que la page et la ligne de commande lisent du passage (`commandes_ouverture._en_json`, et ce que `_dire_ouverture`
    /// lit en plus : le detail des fichiers deposes et des retours archives). Les cles de `_en_json` sont celles du Python,
    /// dans le meme ordre ; les autres sont ajoutees. `maj` et `sources` valent `null` (lot J).
    pub fn en_json(&self) -> Value {
        let sauvegarde = self.sauvegarde.as_ref().map(|s| {
            json!({
                "resume": s.resume(),
                "deposes": s.deposes.iter().map(|f| f.relatif.clone()).collect::<Vec<_>>(),
                "identiques": s.identiques,
                "octets": s.octets_deposes,
                "deposes_detail": s.deposes.iter().map(|f| json!({"relatif": f.relatif, "etat": f.etat.texte(), "octets": f.octets})).collect::<Vec<_>>(),
                "exclus": s.exclus,
                "hors_ligne": s.hors_ligne,
                "souci": s.persistant,
            })
        });
        let retours = self.releve.as_ref().map(|r| {
            json!({
                "resume": Ouverture::resume_retours(r),
                "trouves": r.trouves.len(),
                "nouveaux": r.nouveaux.len(),
                "archives": r.archives.iter().map(|a| json!(a.chemin.to_string_lossy())).collect::<Vec<_>>(),
                "archives_detail": r.archives.iter().map(|a| json!({
                    "chemin": a.chemin.to_string_lossy(),
                    "fichier": a.chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                    "nature": a.retour.get("nature").cloned().unwrap_or(Value::Null),
                    "libelle": crate::retours::libelle_nature(&a.retour.get("nature").map(py_str).unwrap_or_else(|| "None".to_string())),
                })).collect::<Vec<_>>(),
                "rattaches": r.rattaches,
            })
        });
        let retours_dpae = self.releve_dpae.as_ref().map(|r| {
            json!({
                "resume": Ouverture::resume_retours(r),
                "trouves": r.trouves.len(),
                "nouveaux": r.nouveaux.len(),
                "archives": r.archives.iter().map(|a| json!(a.chemin.to_string_lossy())).collect::<Vec<_>>(),
                "rattaches": r.rattaches,
            })
        });
        let fiches = self.fiches.as_ref().map(|f| json!({"resume": f.resume(), "attendues": f.attendues, "recues": f.recues, "incidents": f.incidents}));
        json!({
            "quand": self.quand,
            "secondes": self.secondes,
            "sauvegarde": sauvegarde,
            "retours": retours,
            "signatures": self.signatures.as_ref().map(ReleveSignatures::en_json),
            "maj": Value::Null,
            "sources": Value::Null,
            "incidents": self.incidents,
            "manquants": self.manquants,
            "taux_pas": self.taux_pas,
            "retours_dpae": retours_dpae,
            "fiches": fiches,
            "a_poser": self.a_poser,
            "filet": self.filet,
            "horloge": self.horloge,
            "hors_ligne": self.hors_ligne,
            "injoignables": self.injoignables,
            "tout_va_bien": self.tout_va_bien(),
            "retenue": self.retenue,
        })
    }
}

// ------------------------------------------------------------------ la route

/// Une route de ce module.
///
/// - `ouverture` : `{"sauvegarder"?, "retours"?, "signatures"?, "filet"?, "jours"?, "racine_drive"?, "pas_avant"?, "veille"?,
///   "incidents_editeur"?, "sources"?, "maj"?}` -> `Ouverture::en_json`. **Hors d'une compagnie fictive, elle va au
///   reseau.** Elle ne rend jamais d'erreur : une panne est une phrase de `incidents`.
/// - `ouverture-derniere` : la trace de la derniere ouverture (`config/.ouverture.json`), `{}` si elle manque ou ne se lit
///   pas. Sans reseau.
pub fn route(o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "ouverture" => Ok(ouverture(o, ctx, &Options::de_la_charge(charge), Clients::default(), None).en_json()),
        "ouverture-derniere" => Ok(derniere_ouverture(ctx)),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

// ------------------------------------------------------------------ l'etape en cours

/// L'ETAPE EN COURS D'UN PASSAGE (ticket #220, Leandre, 20/10/2026 : « qui on appelle, puis qui d'autre, la sauvegarde, juste
/// un message dans la ligne qui explique ce que fait la machine »). L'ouverture tourne dans un processus a part (la veille
/// appelle la route `ouverture`) : elle ecrit son etape dans `config/.ouverture-etape.json` en y entrant, l'efface en
/// finissant ; la veille de la fenetre la lit (`Veille::etape`) tant qu'un passage tourne. Le fichier ne monte pas au Drive
/// (`sauvegarde::NOMS_EXCLUS`). Une ecriture qui echoue est passee sous silence : ce n'est qu'un message.
pub const ETAPE: &str = ".ouverture-etape.json";

fn chemin_etape(racine: &Path) -> std::path::PathBuf {
    racine.join("config").join(ETAPE)
}

fn dire_l_etape(ctx: &Contexte, etape: &str) {
    let _ = crate::ecriture::ecrire_json(&chemin_etape(&ctx.racine), &json!({"etape": etape, "depuis": quand_maintenant()}), 1);
}

fn finir_les_etapes(ctx: &Contexte) {
    let _ = std::fs::remove_file(chemin_etape(&ctx.racine));
}

/// L'etape qu'un passage en cours a ecrite, ou `None`.
pub fn etape_en_cours(racine: &Path) -> Option<String> {
    let o = std::fs::read(chemin_etape(racine)).ok()?;
    let v: Value = serde_json::from_slice(&o).ok()?;
    v.get("etape").and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string)
}

// ------------------------------------------------------------------ la trace

fn chemin_trace(ctx: &Contexte) -> std::path::PathBuf {
    ctx.racine.join("config").join(TRACE)
}

/// `derniere_ouverture` : la trace, ou `{}` si elle est absente ou ne se lit pas (JSON invalide, octets invalides, fichier
/// illisible).
pub fn derniere_ouverture(ctx: &Contexte) -> Value {
    let p = chemin_trace(ctx);
    if !p.exists() {
        return json!({});
    }
    std::fs::read(&p)
        .ok()
        .and_then(|o| String::from_utf8(o).ok())
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .unwrap_or_else(|| json!({}))
}

/// `_noter_ouverture` : la trace de la derniere ouverture. Aucun secret n'y figure, par construction. Une ecriture qui
/// echoue est passee sous silence (`except OSError: pass`).
fn noter_ouverture(ctx: &Contexte, r: &Ouverture, precedente: &Value) {
    let p = chemin_trace(ctx);
    if let Some(dossier) = p.parent() {
        let _ = std::fs::create_dir_all(dossier);
    }
    // LES RETOURS DE LA COMPAGNIE, PAS CEUX DU BAC A SABLE (25/09/2026) : les retours des depots de test ne se comptent que
    // dans la copie de developpement.
    let tests: Vec<Value> = if est_copie(&ctx.racine) {
        Vec::new()
    } else {
        // Un journal illisible ne doit pas empecher la trace (l'ouverture n'echoue jamais).
        etats::charger(&ctx.racine)
            .and_then(|l| etats::flux_de_test(&l))
            .ok()
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
    };
    let hors_tests = |liste: &[Value]| -> usize {
        liste.iter().filter(|x| !tests.iter().any(|t| x.get("flux").is_some_and(|f| etats::eq_py(f, t)))).count()
    };
    let (trouves, nouveaux) = match &r.releve {
        Some(rel) => (hors_tests(&rel.trouves), hors_tests(&rel.nouveaux)),
        None => (0, 0),
    };
    // La ligne des retours dit ce que la releve a fait, si ELLE a joint net-entreprises -- le Drive qui ne repond pas ne
    // la tait plus (21/10/2026) ; une releve sans reseau, elle, n'a rien vu, et « aucun retour » serait faux.
    let releve_passee = r.releve.as_ref().is_some_and(|rel| !rel.incidents.iter().any(|i| sauvegarde::hors_ligne(i)));
    let retours = if releve_passee {
        if trouves > 0 {
            format!("{} retour(s), dont {} nouveau(x).", trouves, nouveaux)
        } else {
            "Aucun retour sur la periode.".to_string()
        }
    } else {
        String::new()
    };
    let mut trace = Map::new();
    trace.insert("quand".into(), json!(r.quand));
    trace.insert("secondes".into(), json!(r.secondes));
    trace.insert("sauvegarde".into(), json!(r.sauvegarde.as_ref().map(Sauvegarde::resume).unwrap_or_default()));
    trace.insert("sauvegarde_souci".into(), json!(sauvegarde::souci(&ctx.racine).unwrap_or_default()));
    trace.insert("retours".into(), json!(retours));
    trace.insert("hors_ligne".into(), json!(r.hors_ligne));
    trace.insert("injoignables".into(), json!(r.injoignables));
    trace.insert("nouveaux".into(), json!(nouveaux));
    trace.insert("signatures".into(), json!(r.signatures.as_ref().map(ReleveSignatures::resume).unwrap_or_default()));
    trace.insert("fiches".into(), json!(r.fiches.as_ref().map(ReleveFiches::resume).unwrap_or_default()));
    trace.insert(
        "signatures_recues".into(),
        json!(r.signatures.as_ref().map(|s| s.recues.iter().map(|x| json!({"nom": x.nom, "quand": x.quand, "renvoye": x.renvoye})).collect::<Vec<_>>()).unwrap_or_default()),
    );
    // LOT J (decision du 07/10/2026) : ni la verification des sources ni la recherche de mise a jour ne tournent ici. La trace
    // GARDE ce que la trace precedente en disait (le bandeau « mise a jour prete » de la page, `sources_etat` qui limite la
    // verification des sources a une fois par semaine) : l'appelant, qui lance encore la verification de version en Python,
    // y ecrit son resultat. Rien d'ecrit avant : les valeurs vides d'une trace neuve.
    for (cle, vide) in LOT_J {
        let valeur = precedente.get(*cle).cloned().unwrap_or_else(|| vide());
        trace.insert((*cle).into(), valeur);
    }
    trace.insert("incidents".into(), json!(r.incidents));
    // Les phrases du coffre ne repartent pas : `a_poser` les dit.
    let manquants: Vec<&String> = r.manquants.iter().filter(|m| !r.phrases_coffre.contains(m)).collect();
    trace.insert("manquants".into(), json!(manquants));
    trace.insert("a_poser".into(), json!(r.a_poser));
    trace.insert("taux_pas".into(), json!(r.taux_pas));
    trace.insert("horloge".into(), json!(r.horloge));
    let _ = crate::ecriture::ecrire_json(&p, &Value::Object(trace), 1);
}

// ------------------------------------------------------------------ outils

/// `compagnies.est_copie`.
fn est_copie(racine: &Path) -> bool {
    racine.join(MARQUEUR_COPIE).is_file()
}

/// `datetime.now().strftime("%d/%m/%Y %H:%M")`, a l'heure de Paris.
fn quand_maintenant() -> String {
    let iso = releve_retours::iso_de_naif(releve_retours::maintenant_naif());
    format!("{}/{}/{} {}:{}", &iso[8..10], &iso[5..7], &iso[0..4], &iso[11..13], &iso[14..16])
}

/// L'heure locale naive d'une trace `quand` : `JJ/MM/AAAA HH:MM` (ce que l'ouverture ecrit) ou `AAAA-MM-JJ HH:MM` (ce que
/// le Python attendait en le relisant, voir `age_de_la_derniere`).
fn naif_de_quand(quand: &str) -> Option<i64> {
    let c: Vec<char> = quand.chars().collect();
    let tranche = |a: usize, b: usize| -> String { c.get(a..b).map(|s| s.iter().collect()).unwrap_or_default() };
    let iso = if c.len() == 16 && c[2] == '/' && c[5] == '/' {
        format!("{}-{}-{}T{}", tranche(6, 10), tranche(3, 5), tranche(0, 2), tranche(11, 16))
    } else if c.len() == 16 && c[4] == '-' && c[7] == '-' {
        format!("{}T{}", tranche(0, 10), tranche(11, 16))
    } else {
        return None;
    };
    releve_retours::naif_de_iso(&iso)
}

/// `pas_avant` : l'age, en secondes, de la derniere ouverture, ou `None` si la trace n'en dit rien de lisible.
///
/// DEFAUT DU PYTHON, NON REPRODUIT : le Python relisait `precedente["quand"]` avec `strptime("%Y-%m-%d %H:%M")`, alors que
/// `Ouverture.quand` s'ecrit `"%d/%m/%Y %H:%M"` : la relecture levait toujours `ValueError`, rattrapee par un `pass`, et
/// `pas_avant` ne retenait donc jamais rien. Ici les deux formes se lisent, et l'option fait ce que son nom et son aide
/// disent. A VALIDER (voir la passation).
fn age_de_la_derniere(precedente: &Value) -> Option<i64> {
    let quand = precedente.get("quand").filter(|v| vrai(v)).map(py_str)?;
    let avant = naif_de_quand(&quand)?;
    Some(releve_retours::maintenant_naif() - avant)
}

/// Le coffre dit-il que `service` est pose (ou dans l'environnement) ? `Err` : le coffre refuse de s'ouvrir.
fn coffre_dispo(ctx: &Contexte, service: &str) -> Result<bool, String> {
    crate::coffre::Coffre::du_poste(&ctx.racine, "").disponible(service).map_err(|e| e.message)
}

/// `_trier_hors_ligne` : une seule phrase pour l'absence de reseau, pas une erreur par dossier et par organisme. Les erreurs
/// qui ne disent que « pas de reseau » sortent des incidents ; les autres (un mot de passe refuse, un coffre ferme)
/// restent, elles demandent un geste.
/// Elles ne se perdent pas pour autant : `injoignables` garde de quoi dire QUI n'a pas repondu.
fn trier_hors_ligne(r: &mut Ouverture) {
    let reseau: Vec<String> = r.incidents.iter().filter(|i| sauvegarde::hors_ligne(i)).cloned().collect();
    let drive = r.sauvegarde.as_ref().is_some_and(|s| s.hors_ligne);
    if !reseau.is_empty() || drive {
        r.hors_ligne = true;
        r.incidents.retain(|i| !sauvegarde::hors_ligne(i));
        for i in reseau.into_iter().chain(drive.then(|| "Sauvegarde dans le Drive : réseau injoignable.".to_string())) {
            if !r.injoignables.contains(&i) {
                r.injoignables.push(i);
            }
        }
    }
}

/// Termine le compte rendu : le tri, la duree, la trace.
fn clore(ctx: &Contexte, r: &mut Ouverture, debut: std::time::Instant, precedente: &Value) {
    finir_les_etapes(ctx);
    trier_hors_ligne(r);
    r.secondes = (debut.elapsed().as_secs_f64() * 10.0).round() / 10.0;
    noter_ouverture(ctx, r, precedente);
}

/// La seconde moitie de la veille : la trace du premier appel, mise a jour par la sauvegarde (qui est faite en dernier) sans
/// rien effacer de ce que le premier a ecrit. Le resultat est celui d'une seule ouverture : la sauvegarde, la duree totale,
/// « hors ligne » si l'un des deux l'a vu (et alors plus de ligne « retours », comme la trace d'une seule ouverture), les
/// incidents et les manques a la suite des premiers. Une trace qui manque ou ne se lit pas : la trace ordinaire de ce passage.
fn completer_la_trace(ctx: &Contexte, r: &mut Ouverture, debut: std::time::Instant, precedente: &Value) {
    finir_les_etapes(ctx);
    let Some(avant) = precedente.as_object().filter(|m| !m.is_empty()) else {
        return clore(ctx, r, debut, precedente);
    };
    let etait_hors_ligne = avant.get("hors_ligne").is_some_and(vrai);
    trier_hors_ligne(r);
    let ma_duree = (debut.elapsed().as_secs_f64() * 10.0).round() / 10.0;
    r.secondes = ma_duree;
    let mut trace = avant.clone();
    let liste = |cle: &str| -> Vec<Value> { avant.get(cle).and_then(Value::as_array).cloned().unwrap_or_default() };
    trace.insert("sauvegarde".into(), json!(r.sauvegarde.as_ref().map(Sauvegarde::resume).unwrap_or_default()));
    trace.insert("sauvegarde_souci".into(), json!(sauvegarde::souci(&ctx.racine).unwrap_or_default()));
    let deja = avant.get("secondes").and_then(Value::as_f64).unwrap_or(0.0);
    trace.insert("secondes".into(), json!(((deja + ma_duree) * 10.0).round() / 10.0));
    trace.insert("hors_ligne".into(), json!(etait_hors_ligne || r.hors_ligne));
    // La sauvegarde qui ne passe pas ne tait plus la ligne des retours : la releve, elle, est passee (21/10/2026).
    let mut injoignables = liste("injoignables");
    injoignables.extend(r.injoignables.iter().map(|i| json!(i)));
    trace.insert("injoignables".into(), Value::Array(injoignables));
    let mut incidents = liste("incidents");
    incidents.extend(r.incidents.iter().map(|i| json!(i)));
    trace.insert("incidents".into(), Value::Array(incidents));
    let mut a_poser = liste("a_poser");
    a_poser.extend(r.a_poser.iter().map(|i| json!(i)));
    trace.insert("a_poser".into(), Value::Array(a_poser));
    let mut manquants = liste("manquants");
    manquants.extend(r.manquants.iter().filter(|m| !r.phrases_coffre.contains(m)).map(|m| json!(m)));
    trace.insert("manquants".into(), Value::Array(manquants));
    let _ = crate::ecriture::ecrire_json(&chemin_trace(ctx), &Value::Object(trace), 1);
}

// ------------------------------------------------------------------ l'ouverture

/// `ouverture` : ce qui se fait a l'ouverture du logiciel, et toutes les N minutes ensuite. Ne rend pas de `Result` : elle
/// n'echoue jamais, elle rend compte.
///
/// - `pas_avant` : ne rien refaire si la derniere ouverture a moins de N minutes. Trois fenetres ouvertes coup sur coup ne
///   relevent pas trois fois.
/// - `veille` : le passage periodique. Il saute la sauvegarde quand rien n'a bouge sur le disque -- une comparaison locale,
///   sans un octet de reseau.
/// - `apres_releve` : appele une fois les retours releves, AVANT la sauvegarde (qui passe en dernier depuis le 25/09/2026 :
///   elle prenait six minutes a 37 Mo, et les retours attendaient derriere elle). La trace de l'ouverture est ecrite a ce
///   moment-la, puis refaite a la fin. La route n'en a pas : un processus ne rappelle pas son appelant (voir la passation).
/// - `clients` : les clients reseau a utiliser a la place des vrais (la preuve).
///
/// LA COPIE DE DEVELOPPEMENT ne sauvegarde pas (elle ecrirait dans le Drive de la vraie compagnie), ne releve pas les
/// signatures d'elle-meme (la copie et l'original liraient la meme boite et renverraient chacun le contrat contresigne). Les
/// retours, eux, se relevent : ce sont ceux des depots de test qu'on y fait.
///
/// LA COMPAGNIE FICTIVE : l'horloge d'abord, les retours des depots joues, les signatures et les fiches qui repondent, le filet,
/// et rien d'autre (ni coffre, ni reseau, ni sauvegarde).
pub fn ouverture(o: &HashMap<String, String>, ctx: &Contexte, opt: &Options, mut clients: Clients<'_>, mut apres_releve: Option<&mut dyn FnMut()>) -> Ouverture {
    let debut = std::time::Instant::now();
    let mut r = Ouverture { quand: quand_maintenant(), ..Ouverture::default() };
    let mut sauvegarder = opt.sauvegarder;
    let mut retours = opt.retours;
    let mut signatures = opt.signatures;
    // La trace d'avant ce passage : `pas_avant` la lit, la trace garde d'elle ce qui est au lot J, la seconde moitie de la
    // veille la complete.
    let precedente = derniere_ouverture(ctx);

    if opt.sauvegarde_seule {
        return sauvegarder_seulement(ctx, opt, clients, r, debut, &precedente);
    }

    if opt.pas_avant > 0 {
        if let Some(age) = age_de_la_derniere(&precedente) {
            if age < opt.pas_avant * 60 {
                r.incidents.push(format!("Ouverture deja faite il y a {} minute(s) : rien a refaire.", age.div_euclid(60)));
                r.retenue = true;
                return r;
            }
        }
    }

    let copie = est_copie(&ctx.racine);
    if copie {
        sauvegarder = false;
        signatures = false;
    }

    // LA COMPAGNIE FICTIVE : les retours des depots joues et les signatures demandees arrivent d'ici, sans coffre ni reseau --
    // et rien ne manque au coffre, puisqu'il n'y a rien a y poser.
    if crate::routes_accueil::simulation_active(&ctx.racine) {
        // L'HORLOGE D'ABORD : ce qui etait du avant aujourd'hui se joue, a sa date.
        dire_l_etape(ctx, "Compagnie fictive : l'administrateur fictif fait ce qui tombait…");
        let jouee = crate::horloge::rattraper(&ctx.racine, None, true).and_then(|fait| if fait.is_empty() { crate::horloge::dernier_rattrapage(&ctx.racine, None) } else { Ok(fait) });
        match jouee {
            Ok(fait) => r.horloge = fait,
            Err(e) => r.incidents.push(format!("Horloge de la compagnie fictive : {}", e.message)),
        }
        if retours {
            dire_l_etape(ctx, "Compagnie fictive : retours des dépôts joués…");
            let mut releve = Releve::default();
            match releve_retours::rattacher_simulation_detail(ctx, DELAI_RETOURS_FICTIVE) {
                Ok((n, a_faire)) => (releve.rattaches, releve.a_faire) = (n, a_faire),
                // Le Python ne rattrapait rien ici : l'ouverture ne doit pas tomber (lot I2b).
                Err(e) => r.incidents.push(format!("Retours : {}", e.message)),
            }
            r.releve = Some(releve);
        }
        dire_l_etape(ctx, "Compagnie fictive : signatures et fiches des salariés…");
        // Le resultat de la relève jouee n'est pas gardé : le Python le jetait aussi (`simulation.relever_signatures(ctx)`).
        if let Err(e) = releve_signatures::relever_signatures_fictive(o, ctx, releve_signatures::DELAI_FICTIVE) {
            r.incidents.push(format!("Signatures : {}", e.message));
        }
        match routes_fiches_courriel::relever_fiches(ctx, reborrow_courriel(&mut clients.courriel), crate::horloge::QUI_HORLOGE) {
            // Les incidents de la relève des fiches jouée ne sont pas repris : le Python non plus.
            Ok(f) => r.fiches = Some(f),
            Err(e) => r.incidents.push(format!("Fiches salariés : {}", e.message)),
        }
        retours = false;
        signatures = false;
        sauvegarder = false;
    }

    if retours {
        relever_les_retours(o, ctx, opt, &mut clients, &mut r);
    }

    // LE PAS, AU MEME PASSAGE QUE LES RETOURS, et juste apres eux : c'est un retour qui pose les taux, donc c'est ici qu'ils
    // sont les plus frais. Cela se fait hors ligne aussi (une lecture de config/pas.json), parce qu'un taux perime le reste
    // que le reseau soit la ou non.
    match crate::pas::avertissements(ctx, None, None) {
        Ok(l) => r.taux_pas = l,
        Err(m) => r.incidents.push(format!("Taux de PAS : {}", m)),
    }

    if signatures {
        dire_l_etape(ctx, "Boîte courriel : contrats revenus signés…");
        // Les reponses « Je signe ». La relève ne leve pas pour un compte absent ou une permission manquante : elle le dit.
        match releve_signatures::relever_signatures(o, ctx, reborrow_courriel(&mut clients.courriel), None, "veille") {
            Ok(s) => {
                r.incidents.extend(s.incidents.iter().cloned());
                r.signatures = Some(s);
            }
            Err(e) => r.incidents.push(format!("Signatures : {}", e.message)),
        }
        // Les fiches demandees aux salaries (23/09/2026) : la meme boite, le meme passage, la meme regle.
        dire_l_etape(ctx, "Boîte courriel : fiches des salariés…");
        match routes_fiches_courriel::relever_fiches(ctx, reborrow_courriel(&mut clients.courriel), "veille") {
            Ok(f) => {
                r.incidents.extend(f.incidents.iter().cloned());
                r.fiches = Some(f);
            }
            Err(e) => r.incidents.push(format!("Fiches salariés : {}", e.message)),
        }
    }

    // LA DPAE PREPAREE D'AVANCE (27/09/2026). Apres les retours DPAE : un test automatique dont le bilan vient d'arriver se
    // lit a jour. Le test ne declare rien -- la veille ne DEPOSE toujours rien de reel.
    if opt.filet {
        dire_l_etape(ctx, "DPAE préparée d'avance : dépôt de test…");
        match crate::routes_actes_h2::filet_preparer_avec(o, ctx, &json!({"qui": "veille"}), clients.depot_dpae.as_mut()) {
            Ok(f) => {
                r.incidents.extend(f.get("incidents").and_then(Value::as_array).into_iter().flatten().map(py_str));
                r.filet = Some(f);
            }
            Err(e) => r.incidents.push(format!("DPAE préparée d'avance : {}", e.message)),
        }
    }

    // LE DIAGNOSTIC A DISTANCE (29/09/2026) : ce que le journal des incidents a note part chez l'editeur -- pas depuis la
    // copie de developpement (ses refus sont ceux qu'on provoque pour essayer), mais depuis une compagnie fictive. Et les
    // reponses de l'editeur aux signalements de ce poste. Pas pendant les tests (`PUPPETPAIE_INCIDENTS` pose : le journal est
    // celui d'un bac a sable). Le Python lancait cela dans un fil ; ici c'est fait avant de rendre la main.
    let bac_a_sable = std::env::var("PUPPETPAIE_INCIDENTS").is_ok_and(|v| !v.is_empty());
    if opt.filet && opt.incidents_editeur && !bac_a_sable && (!copie || crate::interface::compagnies::est_fictive(&ctx.racine)) {
        dire_l_etape(ctx, "Serveur de l'éditeur : incidents, messages et réponses…");
        let _ = crate::incidents::envoyer(&ctx.racine);
        // LE NOMBRE DE PAIES DE L'ANNEE PASSEE (ticket #246), a partir du 16 janvier, une fois : `nombre_de_paies` decide
        // seul (ni fictive, ni copie de developpement, ni logiciel bride, ni envoi coupe) et retient ce qui est parti.
        let _ = crate::nombre_de_paies::envoyer_si_du(o, ctx);
        // Les messages « Un probleme, une idee ? » que l'envoi en fond n'a pas fait partir (serveur injoignable...).
        let _ = crate::problemes::partir(&ctx.racine);
        let _ = crate::problemes::relever_reponses(&ctx.racine);
    }

    // (Lot J : ici le Python verifiait les sources, et cherchait une mise a jour.)

    if let Some(rappel) = apres_releve.as_mut() {
        clore(ctx, &mut r, debut, &precedente);
        rappel();
    }

    if sauvegarder {
        sauvegarder_le_dossier(ctx, opt, &mut clients, &mut r);
    }

    clore(ctx, &mut r, debut, &precedente);
    r
}

/// `sauvegarde_seule` : la seconde moitie de la veille. La copie de developpement et la compagnie fictive ne sauvegardent pas
/// (rien a faire, la trace n'est pas touchee) ; sinon la sauvegarde, en dernier, et la trace du premier appel completee.
fn sauvegarder_seulement(ctx: &Contexte, opt: &Options, mut clients: Clients<'_>, mut r: Ouverture, debut: std::time::Instant, precedente: &Value) -> Ouverture {
    let ne_sauvegarde_pas = !opt.sauvegarder || est_copie(&ctx.racine) || crate::routes_accueil::simulation_active(&ctx.racine);
    if ne_sauvegarde_pas {
        return r;
    }
    sauvegarder_le_dossier(ctx, opt, &mut clients, &mut r);
    completer_la_trace(ctx, &mut r, debut, precedente);
    r
}

/// Reprend la boite donnee sans la perdre : les signatures puis les fiches la relisent l'une apres l'autre.
fn reborrow_courriel<'b>(c: &'b mut Option<&mut dyn Courriel>) -> Option<&'b mut dyn Courriel> {
    match c {
        Some(x) => Some(&mut **x),
        None => None,
    }
}

/// Les retours DSN (net-entreprises) puis DPAE (Urssaf) : un secret absent est un manque, une panne est un incident.
fn relever_les_retours(o: &HashMap<String, String>, ctx: &Contexte, opt: &Options, clients: &mut Clients<'_>, r: &mut Ouverture) {
    // Le canal DSN.
    let client_dsn = clients.dsn.take();
    let dispo = match &client_dsn {
        Some(_) => Some(true),
        None => match coffre_dispo(ctx, "net-entreprises") {
            Ok(b) => Some(b),
            Err(m) => {
                // Le Python laissait lever l'erreur du coffre : l'ouverture ne tombe pas.
                r.incidents.push(format!("Retours : {}", m));
                None
            }
        },
    };
    match dispo {
        Some(false) => {
            r.a_poser.push("net-entreprises".to_string());
            r.manque_coffre(
                "Mot de passe net-entreprises absent du coffre : les retours ne peuvent pas etre releves seuls. `paie coffre poser net-entreprises`.",
            );
        }
        Some(true) => {
            dire_l_etape(ctx, "net-entreprises : retours des DSN…");
            let fin = releve_retours::maintenant_naif();
            match releve_retours::relever_retours(ctx, o, fin - opt.jours * 86_400, fin, false, "production", client_dsn, "") {
                Ok(rel) => {
                    r.incidents.extend(rel.incidents.iter().cloned());
                    r.manquants.extend(rel.a_faire.iter().cloned());
                    r.releve = Some(rel);
                }
                Err(e) => r.incidents.push(format!("Retours : {}", e.message)),
            }
        }
        None => {}
    }

    // LE CANAL DPAE AUSSI. Jusqu'au 12/09/2026 la veille ne relevait que net-entreprises : une DPAE refusee en douze secondes
    // restait << pas encore de retour >> tant qu'on ne tapait pas la commande a la main.
    let client_dpae = clients.dpae.take();
    let dispo = match &client_dpae {
        Some(_) => Some(true),
        None => match coffre_dispo(ctx, "urssaf") {
            Ok(b) => Some(b),
            Err(m) => {
                r.incidents.push(format!("Retours DPAE : {}", m));
                None
            }
        },
    };
    match dispo {
        Some(false) => {
            r.a_poser.push("urssaf".to_string());
            r.manque_coffre("Mot de passe Urssaf (DPAE) absent du coffre : les retours de DPAE ne peuvent pas etre releves seuls. `paie coffre poser urssaf`.");
        }
        Some(true) => {
            dire_l_etape(ctx, "Urssaf : retours des DPAE…");
            let fin = releve_retours::maintenant_naif();
            let debut = fin - opt.jours * 86_400;
            // Un client donne passe par la meme releve que le vrai (canal `dpae`) ; sans client, `relever_retours_dpae`
            // cherche le mot de passe de consultation et construit le sien.
            let rel = match client_dpae {
                Some(c) => releve_retours::relever_retours(ctx, o, debut, fin, false, "production", Some(c), "dpae"),
                None => releve_retours::relever_retours_dpae(ctx, o, debut, fin, false, ""),
            };
            match rel {
                Ok(rel) => {
                    r.incidents.extend(rel.incidents.iter().cloned());
                    r.releve_dpae = Some(rel);
                }
                Err(e) => r.incidents.push(format!("Retours DPAE : {}", e.message)),
            }
        }
        None => {}
    }
}

/// La sauvegarde, en dernier : un compte Google non lie est un manque ; rien de neuf sur le disque, en veille, est un
/// silence (aucun appel) ; une panne est un incident.
fn sauvegarder_le_dossier(ctx: &Contexte, opt: &Options, clients: &mut Clients<'_>, r: &mut Ouverture) {
    let pret = clients.drive.is_some() || crate::routes_google::autorise(&ctx.racine);
    if !pret {
        r.a_poser.push("google".to_string());
        r.manque_coffre(
            "Compte Google non lie : la sauvegarde ne peut pas partir. Lier le Drive : Configuration > Mots de passe, ou `paie google autoriser`.",
        );
    } else if opt.veille && sauvegarde::rien_n_a_bouge(&ctx.racine, &opt.racine_drive) {
        // rien de neuf sur le disque : on n'appelle pas
    } else {
        dire_l_etape(ctx, "Google Drive : sauvegarde du dossier…");
        let drive: Option<&mut dyn Drive> = match clients.drive.as_mut() {
            Some(d) => Some(&mut **d),
            None => None,
        };
        match sauvegarde::sauvegarder(&ctx.racine, &opt.racine_drive, false, drive) {
            // CE QUI EST REPORTE N'EST PAS UN INCIDENT (tickets #231 et #232) : un dossier que Google a refuse une fois (un 500,
            // deja retente par le client) repart au passage suivant, en silence. Ce qui persiste se dit dans la ligne de la
            // sauvegarde (`sauvegarde::souci`, la trace `sauvegarde_souci`), pas dans les incidents du releve.
            Ok(s) => r.sauvegarde = Some(s),
            Err(e) => r.incidents.push(format!("Sauvegarde : {}", e)),
        }
    }
}
