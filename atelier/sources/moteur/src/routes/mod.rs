//! `moteur route` : les routes de l'interface. La fenetre les sert dans son processus (`dans_la_fenetre`, ticket #244) ;
//! `moteur route`, une par processus, reste pour la ligne de commande et les tests.
//!
//! Demande sur l'entree standard : `{"route": "...", "charge": {...}}`, avec `--racine` et `--date`. Reponse :
//! `{"ok": true, "resultat": ...}` ; un refus rend `{"ok": false, "erreur": "...", "classe": "..."}` avec sa classe
//! (`ErreurOperation`, `FileNotFoundError`, `KeyError`...), que la page releve.
//!
//! L'aiguillage ne fait rien d'autre : chaque lot de routes vit dans son
//! module et s'ajoute ici d'une ligne. Les routes de l'ouverture
//! (`routes_ouverture`, `rappels` et `apercu`) font leur propre Contexte, et
//! leur refus classe voyage dans le texte de l'erreur
//! (`crate::routes::rappels::decoder`).

use std::collections::HashMap;
use std::path::Path;

use serde_json::{json, Map, Value};

use socle::date::repr_texte;
use socle::echec::{erreur, Echec, R};
use socle::fichiers::vrai;
use droit::contexte::Contexte;
use socle::num::Resultat;
use socle::valeur::{nom_type, valeur_ecrite, valeur_en_texte};
use crate::service::contexte;


pub mod ouverture;
pub mod depots;
pub mod archives;
pub mod documents;
pub mod dossier_salarie;
pub mod coffre;
pub mod engagement;
pub mod contrats;
pub mod fiches;
pub mod import;
pub mod imprevus;
pub mod filet;
pub mod etapes;
pub mod rappels;
pub mod mois;

/// La pile d'un fil qui sert des routes dans la fenetre : 8 Mo, celle du fil principal d'un `moteur route` sous Linux (un
/// fil neuf n'en a que 2). Un calcul qui remonte les mois d'avant s'y empile ; un debordement de pile ne se rattrape pas
/// (`catch_unwind` n'y peut rien), il fermerait la fenetre.
pub const PILE_D_UNE_ROUTE: usize = 8 * 1024 * 1024;

/// Un fil qui sert des routes dans la fenetre (une connexion, la file des etats, la veille, un passage en fond), avec
/// la pile qu'il leur faut (`PILE_D_UNE_ROUTE`).
pub fn fil_de_routes<F: FnOnce() + Send + 'static>(f: F) {
    if let Err(e) = std::thread::Builder::new().stack_size(PILE_D_UNE_ROUTE).spawn(f) {
        eprintln!("Un fil du moteur ne demarre pas : {}.", e);
    }
}

/// La route d'essai qui tombe (`commande`, en essai seulement).
pub const ROUTE_QUI_TOMBE: &str = "essai-defaut-du-logiciel";

/// Le fil remis propre apres une route servie dans la fenetre (`dans_la_fenetre`, `etat_garde::FileDesEtats`) : plus
/// de mot de passe tape, les nombres a leur ecriture normale, aucun registre de lectures ouvert.
pub(crate) fn remettre_le_fil() {
    crate::coffre::oublier_mot_de_passe_tape();
    socle::num::ECHELLE_GARDEE.with(|f| f.set(false));
    let _ = socle::fichiers::rendre();
}

/// UNE ROUTE SERVIE DANS LA FENETRE, ET CE QU'ELLE LAISSE SUR SON FIL (ticket #244). La fenetre sert ses routes dans son
/// processus (`interface::api::route`, la file des etats), et le fil d'une connexion en sert d'autres ensuite. Cette
/// fonction fait la route (`commande`), gardee d'une panique (une route qui tombe rend un refus `DefautDuLogiciel` et le
/// fil continue), puis remet le fil propre, quoi qu'il soit arrive : le mot de passe tape oublie (`coffre::TAPE` : il ne
/// survit jamais a sa route), les nombres a leur ecriture normale (`num::ECHELLE_GARDEE`), le registre des lectures
/// qu'un calcul aurait laisse ouvert en tombant ferme (`fichiers::suivre`).
///
/// `moteur route` et la ligne de commande (`commandes::Cadre`) appellent `commande` directement : le processus meurt avec
/// sa commande, et le mot de passe qu'elle a fait taper (`demande::autoriser_acte`) sert a toute la commande.
pub fn dans_la_fenetre(o: &HashMap<String, String>, route: &str, charge: &Value) -> Value {
    let entree = json!({"route": route, "charge": charge}).to_string();
    let rendu = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| commande(o, &entree)));
    remettre_le_fil();
    match rendu {
        Ok(Ok(rep)) => rep,
        Ok(Err(m)) => json!({"ok": false, "erreur": m}),
        Err(panique) => {
            let pourquoi = panique
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| panique.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "sans phrase".into());
            json!({"ok": false, "classe": "DefautDuLogiciel", "erreur": format!("Defaut du logiciel : la route {} s'est arretee ({}).", route, pourquoi)})
        }
    }
}

/// `moteur route [--racine R] [--date D]`.
pub fn commande(o: &HashMap<String, String>, entree: &str) -> Resultat<Value> {
    // Le code livre ne bouge pas le temps de la route : ses signatures se lisent une fois (`fichiers::code_fige`).
    let _code = socle::fichiers::code_fige();
    crate::incidents::brancher_l_engagement();
    let d: Value = serde_json::from_str(entree).map_err(|e| format!("demande illisible : {}", e))?;
    let route = d.get("route").and_then(Value::as_str).ok_or("demande illisible : il faut `route`")?;
    let charge = d.get("charge").cloned().unwrap_or_else(|| json!({}));
    // Le mot de passe du logiciel tape pour cette route ouvre aussi le coffre (`coffre::retenir_mot_de_passe_tape`) ;
    // dans la fenetre, le temps de cette route seulement (`dans_la_fenetre`).
    if let Some(m) = charge.get("motdepasse").and_then(Value::as_str) {
        crate::coffre::retenir_mot_de_passe_tape(m);
    }
    // UNE ROUTE QUI TOMBE, pour les essais seulement (`garde_reseau::en_essai`, que Cargo pose) : la fenetre doit rendre
    // un refus et servir la suivante (`dans_la_fenetre`, `tests/routes_dans_la_fenetre.rs`). Le fil est laisse sale
    // expres, comme un calcul qui tombe le laisserait.
    if route == ROUTE_QUI_TOMBE && socle::garde_reseau::en_essai() {
        socle::num::ECHELLE_GARDEE.with(|f| f.set(true));
        panic!("la route d'essai tombe expres");
    }
    if let Some(r) = crate::routes::rappels::traiter(o, route, &charge) {
        return match r {
            Ok(v) => Ok(json!({"ok": true, "resultat": v})),
            Err(e) => match crate::routes::rappels::decoder(&e) {
                (Some(c), message) => Ok(json!({"ok": false, "erreur": message, "classe": c})),
                (None, message) => Err(message.to_string()),
            },
        };
    }
    // UNE ROUTE QUI PORTE UN MOIS LIT LA CONFIGURATION DE CE MOIS (`Contexte::du_mois`) : sa paie, sa DSN et ses documents
    // se font sous les valeurs qui valaient alors, jamais sous celles d'aujourd'hui.
    let ctx = match charge.get("mois").and_then(Value::as_str).filter(|m| socle::date::premier_du_mois(m).is_ok()) {
        Some(m) => contexte(o)?.du_mois(m)?,
        None => contexte(o)?,
    };
    let rendu: Result<Value, Echec> = match route {
        // L'etat d'un mois : l'etat garde, ecrit apres chaque ecriture, lu tel quel (`etat_garde`).
        r if crate::etat_garde::ROUTES.contains(&r) => crate::etat_garde::route(o, &ctx, route, &charge),
        // Un geste de l'ecran, une route : un module par geste.
        r if mois::ROUTES.contains(&r) => mois::route(o, &ctx, route, &charge),
        r if etapes::ROUTES.contains(&r) => etapes::route(o, &ctx, route, &charge),
        r if engagement::ROUTES.contains(&r) => engagement::route(o, &ctx, route, &charge),
        r if contrats::ROUTES.contains(&r) => contrats::route(o, &ctx, route, &charge),
        r if documents::ROUTES.contains(&r) => documents::route(o, &ctx, route, &charge),
        // Un refus du depot qui porte une donnee pour la page (`etat`) la rend a cote de `erreur` et `classe`.
        r if depots::ROUTES.contains(&r) => match depots::route(o, &ctx, route, &charge) {
            Err(Echec { classe: Some(c), message }) if matches!(c, "RepetitionManquante" | "DsnDejaDeposee") => {
                let d: Value = serde_json::from_str(&message).unwrap_or_else(|_| json!({"message": message}));
                return Ok(json!({"ok": false, "erreur": d["message"], "classe": c, "etat": d["etat"]}));
            }
            autre => autre,
        },
        r if imprevus::ROUTES.contains(&r) => imprevus::route(o, &ctx, route, &charge),
        // Les obligations d'un arret : le jour ou la compagnie l'a su, la declaration d'accident faite.
        r if crate::obligations::ROUTES.contains(&r) || crate::obligations::ROUTES_LECTURE.contains(&r) => crate::obligations::route(&ctx, route, &charge),
        r if fiches::ROUTES.contains(&r) => fiches::route(o, &ctx, route, &charge),
        // Les organismes et les contrats de la DSN : lus, et composes quand la compagnie les enregistre (`organismes`).
        crate::organismes::ROUTE_LIRE => crate::organismes::lire(&ctx, &charge),
        crate::organismes::ROUTE_ECRIRE => crate::organismes::enregistrer(&ctx, &charge),
        r if coffre::ROUTES.contains(&r) => coffre::route(o, &ctx, route, &charge),
        crate::coffre_verifier::ROUTE => crate::coffre_verifier::route(&ctx, &charge),
        r if filet::ROUTES.contains(&r) || r == filet::ROUTE_OUBLIER => filet::route(o, &ctx, route, &charge),
        r if import::ROUTES.contains(&r) => import::route(o, &ctx, route, &charge),
        // Les fichiers que l'import n'a pas repris : le message caviarde qui partirait chez l'editeur (rien n'est ecrit),
        // et son envoi, sur le seul clic de la personne (`importation_incident`).
        r if crate::importation_incident::ROUTES.contains(&r) => crate::importation_incident::route(&ctx, route, &charge),
        r if archives::ROUTES.contains(&r) => archives::route(o, &ctx, route, &charge),
        // L'Historique d'un mois : ses documents, ses envois et leurs retours, lus (lecture seule).
        crate::historique_mois::ROUTE => crate::historique_mois::route(&ctx, &charge),
        r if dossier_salarie::ROUTES.contains(&r) => dossier_salarie::route(o, &ctx, route, &charge),
        r if ouverture::ROUTES.contains(&r) => ouverture::route(o, &ctx, route, &charge),
        // Ce qu'il manque a la compagnie pour ses premiers actes, d'un coup (`demarrage`).
        declarations::demarrage::ROUTE => declarations::demarrage::route(&ctx),
        // Les conseils « Le saviez-vous ? » : le conseil du jour, ceux du dossier d'un salarie, le resume de l'accueil.
        r if crate::conseils::ROUTES.contains(&r) => crate::conseils::route(&ctx, route, &charge),
        // Supprimer la compagnie : ce que la boite demande (`interface::suppression::question`) ; la suppression elle-meme
        // est un geste de la fenetre, qui lache d'abord ce qu'elle tient de la compagnie.
        "compagnie-suppression" => crate::interface::suppression::question(&ctx.racine).map_err(|m| Echec { classe: Some("ErreurOperation"), message: m }),
        // La releve des reponses « Je signe » : cherche les codes attendus dans la boite (la vraie, ou celle d'une
        // compagnie fictive), note les signatures, archive la preuve, contresigne et renvoie le contrat.
        r if crate::releve_signatures::ROUTES.contains(&r) => crate::releve_signatures::route(&ctx, route, &charge),
        // L'ouverture du logiciel et la veille : sauvegarder, relever les retours, les signatures et les fiches, sans
        // jamais echouer. ATTENTION : hors d'une compagnie fictive, cette route va au reseau (net-entreprises, Urssaf,
        // boite courriel, Drive, serveur des incidents).
        r if crate::ouverture::ROUTES.contains(&r) => crate::ouverture::route(o, &ctx, route, &charge),
        // Le banc : l'horloge de la compagnie fictive (rattraper ce qui etait du, ce que le bandeau « pendant votre
        // absence » lit ; `horloge::HorlogeFictive`) et la fenetre de l'administrateur (le tableau de ses compagnies, le
        // depot groupe).
        r if crate::horloge::ROUTES.contains(&r) => crate::horloge::route(o, &ctx, route, &charge),
        r if crate::administrateur::ROUTES.contains(&r) => crate::administrateur::route(o, &ctx, r, &charge),
        autre => return Err(format!("route inconnue : {}", autre)),
    };
    // UN ACTE REFUSE A UNE COMPAGNIE QUI N'A PAS FINI DE DEMARRER dit tout ce qui lui manque, pas un manque a la fois.
    let rendu = match rendu {
        Err(Echec { classe, message }) if est_une_ecriture(route) && classe != Some(crate::bridage::CLASSE_REFUS_D_ENVOI) && classe != Some(crate::routes::engagement::CLASSE_DOCUMENTS_PARTIS) && classe != Some(crate::routes::engagement::CLASSE_ENGAGEMENT_PARTI) && !message.trim_start().starts_with('{') => {
            let suite = declarations::demarrage::a_la_suite(&ctx).unwrap_or_default();
            Err(Echec { classe, message: message + &suite })
        }
        autre => autre,
    };
    // Les etats des mois ne se refont plus ici : c'est la fenetre qui les refait, dans son processus, apres chaque
    // ecriture (`etat_garde::FileDesEtats`, ticket #212). Hors de la fenetre, un etat perime se refait a sa lecture.
    match rendu {
        Ok(v) => Ok(json!({"ok": true, "resultat": v})),
        // Une fiche refusee porte ses anomalies : la page les montre.
        Err(Echec { classe: Some("FicheRefusee"), message }) => {
            let d: Value = serde_json::from_str(&message).unwrap_or_else(|_| json!({"message": message}));
            Ok(json!({"ok": false, "erreur": d["message"], "classe": "FicheRefusee", "anomalies": d["anomalies"]}))
        }
        Err(Echec { classe: None, message }) => Err(message),
        Err(Echec { classe: Some(c), message }) => Ok(json!({"ok": false, "erreur": message, "classe": c})),
    }
}

/// Les routes qui ecrivent (les lots d'ecritures, d'actes, d'envois et de releves) : apres elles, la fenetre verifie les
/// etats des mois (`interface/api.rs`).
pub fn est_une_ecriture(route: &str) -> bool {
    [mois::ECRITURES, engagement::ECRITURES, contrats::ECRITURES, documents::ECRITURES, depots::ECRITURES, imprevus::ECRITURES]
        .iter()
        .chain(&[fiches::ECRITURES, coffre::ECRITURES, import::ECRITURES, archives::ECRITURES, ouverture::ECRITURES])
        .any(|l| l.contains(&route))
        || crate::obligations::ROUTES.contains(&route)
        || crate::releve_signatures::ROUTES.contains(&route)
        || crate::administrateur::ROUTES.contains(&route)
}

/// `socle::paris::maintenant` a la minute, `AAAA-MM-JJ HH:MM` : l'heure d'une saisie.
pub(crate) fn maintenant() -> String {
    socle::paris::maintenant()[..16].replacen('T', " ", 1)
}

/// QUI ECRIT. Les routes inscrivent `interface` : c'est la fenetre. La ligne de commande (`moteur ...`) ajoute `qui` a la charge
/// (`cli`, ou `ligne de commande` selon la commande, comme le Python les ecrit) : une valeur ecrite reste identique d'une face a
/// l'autre. Absent, vide ou pas un texte : `interface` -- la fenetre ne change pas.
pub(crate) fn qui_de(charge: &Value) -> String {
    match charge.get("qui") {
        Some(Value::String(s)) if !s.trim().is_empty() => s.clone(),
        _ => "interface".to_string(),
    }
}

/// `Contexte.salarie(nom)` : le refus est un `KeyError`. Rust rend le texte
/// du refus tel quel pour une phrase (`Salarie ...`), et deja entre guillemets
/// pour une cle manquante d'une fiche ; `str(KeyError(phrase))` est la phrase
/// entre guillemets. Un `nom` qui n'est pas un texte se lit `str(nom)` et se
/// montre `repr(nom)` dans la phrase.
pub(crate) fn salarie(ctx: &Contexte, nom: &Value) -> R<Value> {
    let texte = valeur_en_texte(nom);
    ctx.salarie(&texte).map_err(|m| {
        let m = if m.starts_with("Salarie ") {
            let m = match nom {
                Value::String(_) => m,
                autre => m.replacen(&repr_texte(&texte), &valeur_ecrite(autre), 1),
            };
            m
        } else {
            m
        };
        erreur("KeyError", m)
    })
}

/// `sortie_json.brut` d'une valeur lue d'un fichier : un dictionnaire perd ses
/// cles qui commencent par `_`.
pub(crate) fn brut(v: &Value) -> Value {
    match v {
        Value::Object(o) => Value::Object(o.iter().filter(|(k, _)| !k.starts_with('_')).map(|(k, x)| (k.clone(), brut(x))).collect()),
        Value::Array(a) => Value::Array(a.iter().map(brut).collect()),
        autre => autre.clone(),
    }
}

fn erreur_io(e: &std::io::Error, p: &Path) -> Echec {
    let classe = match e.kind() {
        std::io::ErrorKind::NotFound => "FileNotFoundError",
        std::io::ErrorKind::PermissionDenied => "PermissionError",
        _ => "OSError",
    };
    erreur(classe, format!("{} : {}", p.display(), e))
}

/// `p.read_text(encoding="utf-8")`.
fn lire_texte(p: &Path) -> R<String> {
    let octets = std::fs::read(p).map_err(|e| erreur_io(&e, p))?;
    String::from_utf8(octets).map_err(|e| erreur("UnicodeDecodeError", format!("{} : {}", p.display(), e)))
}

/// `json.loads(p.read_text(encoding="utf-8"))`.
fn lire_json(p: &Path) -> R<Value> {
    serde_json::from_str(&lire_texte(p)?).map_err(|e| erreur("JSONDecodeError", e.to_string()))
}

/// `_lire_json(p)` : `{}` si le fichier n'existe pas.
pub(crate) fn lire_json_ou_vide(p: &Path) -> R<Value> {
    if p.exists() {
        lire_json(p)
    } else {
        Ok(Value::Object(Map::new()))
    }
}

/// `try: json.loads(p.read_text(...)) except ValueError: ...` : `None` quand le
/// fichier est absent, illisible comme texte ou comme JSON ; un autre refus du
/// disque (`OSError`) se leve.
fn lire_json_tolerant(p: &Path) -> R<Option<Value>> {
    if !p.exists() {
        return Ok(None);
    }
    let octets = std::fs::read(p).map_err(|e| erreur_io(&e, p))?;
    Ok(String::from_utf8(octets).ok().and_then(|t| serde_json::from_str(&t).ok()))
}

/// `len(v)`.
fn longueur(v: &Value) -> R<usize> {
    match v {
        Value::Array(a) => Ok(a.len()),
        Value::Object(o) => Ok(o.len()),
        Value::String(s) => Ok(s.chars().count()),
        autre => Err(erreur("TypeError", socle::fichiers::attendu("un texte ou une liste", nom_type(autre)))),
    }
}

/// `v.get(...)` sur un dictionnaire ; sinon `AttributeError`.
fn objet<'a>(v: &'a Value) -> R<&'a Map<String, Value>> {
    v.as_object().ok_or_else(|| erreur("AttributeError", socle::fichiers::attendu("un objet", nom_type(v))))
}

fn valeur<'a>(m: &'a Map<String, Value>, cle: &str) -> &'a Value {
    static NUL: Value = Value::Null;
    m.get(cle).unwrap_or(&NUL)
}

/// `x or ""`.
fn ou_texte(v: &Value) -> Value {
    if vrai(v) {
        v.clone()
    } else {
        json!("")
    }
}

pub(crate) fn vers_route(x: crate::coffre::Echec) -> Echec {
    Echec { classe: Some(x.classe), message: x.message }
}
