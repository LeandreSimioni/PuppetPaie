//! `moteur route` : les routes de l'interface. La fenetre les sert dans son processus (`dans_la_fenetre`) ; `moteur
//! route`, une par processus, sert la ligne de commande et les tests.
//!
//! Demande sur l'entree standard : `{"route": "...", "charge": {...}}`, avec `--racine` et `--date`. Reponse :
//! `{"ok": true, "resultat": ...}` ; un refus rend `{"ok": false, "erreur": "...", "classe": "..."}` avec sa classe
//! (`ErreurOperation`, `FileNotFoundError`, `KeyError`...), que la page releve.
//!
//! UN FICHIER PAR GESTE DE L'ECRAN : `mois` (la saisie et l'apercu), `etapes` (le panneau et les carres de
//! l'engagement), `engagement` (valider, modifier, retirer), `contrats`, `documents`, `depots`, `imprevus`, `fiches`,
//! `coffre`, `filet`, `import`, `archives` (l'Historique, les exports, la cloture), `dossier_salarie`, `ouverture`,
//! `rappels`. Chacun dit ses routes et leurs fiches (`ROUTES`, `catalogue::FICHES`), comme les modules des obligations,
//! des conseils, de l'ouverture et de l'administrateur ; l'aiguillage ne fait rien d'autre que les servir (et l'horloge
//! du banc, hors catalogue). Ce que les routes partagent (lire un fichier, le salarie d'une charge, `qui_de`) est ici.

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
pub mod modeles_contrat;
pub mod fiches;
pub mod etablissements;
pub mod atmp;
pub mod import;
pub mod imprevus;
pub mod filet;
pub mod etapes;
pub mod rappels;
pub mod mois;
pub mod catalogue;

/// Supprimer la compagnie : ce que la boite demande.
pub const COMPAGNIE_SUPPRESSION: &str = "compagnie-suppression";

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
    crate::compagnie::coffre::oublier_mot_de_passe_tape();
    socle::num::ECHELLE_GARDEE.with(|f| f.set(false));
    socle::fichiers::fermer_tout();
}

/// UNE ROUTE SERVIE DANS LA FENETRE, ET CE QU'ELLE LAISSE SUR SON FIL. La fenetre sert ses routes dans son
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
    crate::compagnie::incidents::brancher_l_engagement();
    let d: Value = serde_json::from_str(entree).map_err(|e| format!("demande illisible : {}", e))?;
    let route = d.get("route").and_then(Value::as_str).ok_or("demande illisible : il faut `route`")?;
    let charge = d.get("charge").cloned().unwrap_or_else(|| json!({}));
    // Le mot de passe du logiciel tape pour cette route ouvre aussi le coffre (`coffre::retenir_mot_de_passe_tape`) ;
    // dans la fenetre, le temps de cette route seulement (`dans_la_fenetre`).
    if let Some(m) = charge.get("motdepasse").and_then(Value::as_str) {
        crate::compagnie::coffre::retenir_mot_de_passe_tape(m);
    }
    // UNE ROUTE QUI TOMBE, pour les essais seulement (`garde_reseau::en_essai`, que Cargo pose) : la fenetre doit rendre
    // un refus et servir la suivante (`dans_la_fenetre`, `tests/routes/`). Le fil est laisse sale
    // expres, comme un calcul qui tombe le laisserait.
    if route == ROUTE_QUI_TOMBE && socle::garde_reseau::en_essai() {
        socle::num::ECHELLE_GARDEE.with(|f| f.set(true));
        panic!("la route d'essai tombe expres");
    }
    // UNE ROUTE QUI PORTE UN MOIS LIT LA CONFIGURATION DE CE MOIS (`Contexte::du_mois`) : sa paie, sa DSN et ses documents
    // se font sous les valeurs qui valaient alors, jamais sous celles d'aujourd'hui.
    let ctx = match charge.get("mois").and_then(Value::as_str).filter(|m| socle::date::premier_du_mois(m).is_ok()) {
        Some(m) => contexte(o)?.du_mois(m)?,
        None => contexte(o)?,
    };
    let dans = |l: &[catalogue::Fiche]| catalogue::dans(l, route).is_some();
    let rendu: Result<Value, Echec> = match route {
        // L'etat d'un mois : l'etat garde, ecrit apres chaque ecriture, lu tel quel (`etat_garde`).
        _ if dans(crate::etat_garde::ROUTES) => crate::etat_garde::route(o, &ctx, route, &charge),
        // Un geste de l'ecran, une route : un module par geste.
        _ if dans(mois::ROUTES) => mois::route(o, &ctx, route, &charge),
        _ if dans(etapes::ROUTES) => etapes::route(o, &ctx, route, &charge),
        _ if dans(engagement::ROUTES) => engagement::route(o, &ctx, route, &charge),
        _ if dans(contrats::ROUTES) => contrats::route(o, &ctx, route, &charge),
        _ if dans(modeles_contrat::ROUTES) => modeles_contrat::route(o, &ctx, route, &charge),
        _ if dans(documents::ROUTES) => documents::route(o, &ctx, route, &charge),
        // Un refus du depot qui porte une donnee pour la page (`etat`) la rend a cote de `erreur` et `classe`.
        _ if dans(depots::ROUTES) => match depots::route(o, &ctx, route, &charge) {
            Err(Echec { classe: Some(c), message }) if matches!(c, "RepetitionManquante" | "DsnDejaDeposee") => {
                let d: Value = serde_json::from_str(&message).unwrap_or_else(|_| json!({"message": message}));
                return Ok(json!({"ok": false, "erreur": d["message"], "classe": c, "etat": d["etat"]}));
            }
            autre => autre,
        },
        _ if dans(imprevus::ROUTES) => imprevus::route(o, &ctx, route, &charge),
        // Les obligations d'un arret : le jour ou la compagnie l'a su, la declaration d'accident faite.
        _ if dans(crate::obligations::ROUTES) => crate::obligations::route(&ctx, route, &charge),
        _ if dans(fiches::ROUTES) => fiches::route(o, &ctx, route, &charge),
        // Les organismes et les contrats de la DSN : lus, et composes quand la compagnie les enregistre (`organismes`).
        _ if dans(crate::compagnie::organismes::ROUTES) => crate::compagnie::organismes::route(&ctx, route, &charge),
        _ if dans(coffre::ROUTES) => coffre::route(o, &ctx, route, &charge),
        _ if dans(filet::ROUTES) => filet::route(o, &ctx, route, &charge),
        _ if dans(import::ROUTES) => import::route(o, &ctx, route, &charge),
        _ if dans(archives::ROUTES) => archives::route(o, &ctx, route, &charge),
        // L'Historique d'un mois : ses documents, ses envois et leurs retours, lus (lecture seule).
        _ if dans(crate::historique_mois::ROUTES) => crate::historique_mois::route(&ctx, &charge),
        _ if dans(dossier_salarie::ROUTES) => dossier_salarie::route(o, &ctx, route, &charge),
        _ if dans(ouverture::ROUTES) => ouverture::route(o, &ctx, route, &charge),
        _ if dans(rappels::ROUTES) => rappels::rappels(&ctx),
        // Les conseils « Le saviez-vous ? » : le conseil du jour, ceux du dossier d'un salarie, le resume de l'accueil.
        _ if dans(crate::compagnie::conseils::ROUTES) => crate::compagnie::conseils::route(&ctx, route, &charge),
        // La releve des reponses « Je signe » : cherche les codes attendus dans la boite (la vraie, ou celle d'une
        // compagnie fictive), note les signatures, archive la preuve, contresigne et renvoie le contrat.
        _ if dans(crate::compagnie::releve_signatures::ROUTES) => crate::compagnie::releve_signatures::route(&ctx, route, &charge),
        // L'ouverture du logiciel et la veille : sauvegarder, relever les retours, les signatures et les fiches, sans
        // jamais echouer. ATTENTION : hors d'une compagnie fictive, cette route va au reseau (net-entreprises, Urssaf,
        // boite courriel, Drive, serveur des incidents).
        _ if dans(crate::compagnie::ouverture::ROUTES) => crate::compagnie::ouverture::route(o, &ctx, route, &charge),
        // La fenetre de l'administrateur : le tableau de ses compagnies, le depot groupe.
        _ if dans(crate::compagnie::administrateur::ROUTES) => crate::compagnie::administrateur::route(o, &ctx, route, &charge),
        // Le banc : l'horloge de la compagnie fictive (rattraper ce qui etait du, ce que le bandeau « pendant votre
        // absence » lit ; `horloge::HorlogeFictive`).
        r if crate::horloge::ROUTES.contains(&r) => crate::horloge::route(o, &ctx, route, &charge),
        // Le catalogue lui-meme, ce que `moteur routes --json` imprime.
        _ if dans(catalogue::ROUTES) => Ok(catalogue::en_json(charge.get("avance").is_some_and(socle::fichiers::vrai))),
        autre => return Err(format!("route inconnue : {}", autre)),
    };
    // UN ACTE REFUSE A UNE COMPAGNIE QUI N'A PAS FINI DE DEMARRER dit tout ce qui lui manque, pas un manque a la fois.
    // Les mots de passe et les comptes (`routes::coffre`), le filet et les messages au developpeur (`routes::filet`) ne
    // dependent d'aucun de ces manques : leur refus se dit seul.
    let rendu = match rendu {
        Err(Echec { classe, message }) if est_une_ecriture(route) && !dans(coffre::ROUTES) && !dans(filet::ROUTES) && classe != Some(crate::compagnie::bridage::CLASSE_REFUS_D_ENVOI) && !message.trim_start().starts_with('{') => {
            let suite = declarations::demarrage::a_la_suite(&ctx, &message).unwrap_or_default();
            Err(Echec { classe, message: message + &suite })
        }
        autre => autre,
    };
    // Les etats des mois se refont dans la fenetre, dans son processus, apres chaque ecriture
    // (`etat_garde::FileDesEtats`) ; hors de la fenetre, un etat perime se refait a sa lecture.
    match rendu {
        Ok(v) => {
            phrase_a_sa_place(&ctx.racine, route, &v);
            Ok(json!({"ok": true, "resultat": v}))
        }
        // Une fiche refusee porte ses anomalies : la page les montre.
        Err(Echec { classe: Some("FicheRefusee"), message }) => {
            let d: Value = serde_json::from_str(&message).unwrap_or_else(|_| json!({"message": message}));
            Ok(json!({"ok": false, "erreur": d["message"], "classe": "FicheRefusee", "anomalies": d["anomalies"]}))
        }
        Err(Echec { classe: None, message }) => Err(message),
        Err(Echec { classe: Some(c), message }) => Ok(json!({"ok": false, "erreur": message, "classe": c})),
    }
}

/// UNE PHRASE PLUS LONGUE QUE SA PLACE (la `phrase` de la fiche) se lit entiere, et note un incident silencieux au
/// developpeur : la route, la place, la longueur ; jamais la phrase, qui peut nommer un salarie. On la corrige a la cause,
/// en l'ecrivant plus breve.
fn phrase_a_sa_place(racine: &Path, route: &str, rendu: &Value) {
    let Some(place) = catalogue::catalogue().iter().find(|f| f.nom == route).map(|f| f.phrase).filter(|p| *p > 0) else { return };
    let Some(longueur) = rendu.get("phrase").and_then(Value::as_str).map(|p| p.chars().count()).filter(|n| *n > place) else { return };
    let resume = format!("La phrase de la route {} passe sa place : {} caractères pour {}.", route, longueur, place);
    let _ = crate::compagnie::incidents::noter(racine, &crate::compagnie::incidents::Note {
        nature: crate::compagnie::incidents::LOGICIEL,
        resume: &resume,
        ou: route,
        quoi: "PhraseTropLongue",
        detail: json!({"route": route, "place": place, "longueur": longueur}),
        sans_contexte: true,
        ..Default::default()
    });
}

/// Les routes qui ecrivent ou engagent, d'apres l'acte de leur fiche : apres elles, la fenetre verifie les etats des mois
/// (`interface/api.rs`).
pub fn est_une_ecriture(route: &str) -> bool {
    catalogue::FICHES.iter().any(|l| catalogue::dans(l, route).is_some_and(|f| f.acte != catalogue::Acte::Lit))
}

/// `socle::paris::maintenant` a la minute, `AAAA-MM-JJ HH:MM` : l'heure d'une saisie.
pub(crate) fn maintenant() -> String {
    socle::paris::maintenant()[..16].replacen('T', " ", 1)
}

/// QUI ECRIT. Les routes inscrivent `interface` : c'est la fenetre. La ligne de commande (`moteur ...`) ajoute `qui` a la charge
/// (`cli`, ou `ligne de commande` selon la commande) : une valeur ecrite reste identique d'une face a
/// l'autre. Absent, vide ou pas un texte : `interface` -- la fenetre ne change pas.
pub(crate) fn qui_de(charge: &Value) -> String {
    match charge.get("qui") {
        Some(Value::String(s)) if !s.trim().is_empty() => s.clone(),
        _ => "interface".to_string(),
    }
}

/// Le salarié que `nom` désigne (`Contexte::salarie`) ; refus (`KeyError`) avec la phrase de `Contexte::salarie`.
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

/// LA COULEUR D'UN ETAT, la meme sur tout l'ecran : `vert` ce qui est fait, `orange` ce qui reste a faire et que la
/// compagnie peut faire (une signature attendue se relance), `rouge` ce qui est refuse, `gris` ce qui n'est qu'une
/// information (rien a faire). La page dessine la couleur qu'on lui donne ; elle n'en choisit aucune.
pub(crate) fn couleur_de_l_etat(etat: &str) -> &'static str {
    match etat {
        "fait" => "vert",
        "a_faire" | "en_attente" | "en_retard" => "orange",
        "refuse" => "rouge",
        _ => "gris",
    }
}

/// LA PASTILLE D'UN ETAT, telle que la page la dessine : sa couleur (`couleur_de_l_etat`) et ses mots.
pub(crate) fn pastille(etat: &str, mots: &str) -> Value {
    json!({"couleur": couleur_de_l_etat(etat), "mots": mots})
}

/// « 12/09/2026 à 18:18 » : un horodatage ISO tel que l'ecran le dit ; ce qui n'en est pas un, tel quel.
pub(crate) fn quand(iso: &str) -> String {
    let b = iso.as_bytes();
    if b.len() < 10 || b[4] != b'-' || b[7] != b'-' {
        return iso.to_string();
    }
    let jour = format!("{}/{}/{}", &iso[8..10], &iso[5..7], &iso[..4]);
    match iso.get(11..16) {
        Some(h) => format!("{} à {}", jour, h),
        None => jour,
    }
}

/// LE NOM D'UN SALARIE TEL QU'UN ECRAN LE MONTRE : le nom de famille en capitales, sous sa forme lisible (`nom_affichage`,
/// accents compris : « SÉRAPHIN »), puis le prenom lisible. La forme declaree (`nom`, sans accents) reste celle de la DSN
/// et de la DPAE ; une fiche qui n'a que celle-la la montre.
pub(crate) fn nom_a_l_ecran(sal: &Value) -> String {
    let lu = |cles: [&str; 2]| {
        cles.iter().filter_map(|c| sal.get(*c).filter(|v| vrai(v))).map(valeur_en_texte).find(|t| !t.trim().is_empty()).unwrap_or_default()
    };
    format!("{} {}", lu(["nom_affichage", "nom"]).to_uppercase(), lu(["prenom_affichage", "prenom"])).trim().to_string()
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

/// Un fichier lu en UTF-8.
fn lire_texte(p: &Path) -> R<String> {
    let octets = std::fs::read(p).map_err(|e| erreur_io(&e, p))?;
    String::from_utf8(octets).map_err(|e| erreur("UnicodeDecodeError", format!("{} : {}", p.display(), e)))
}

/// Le JSON d'un fichier UTF-8.
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

/// Le JSON d'un fichier : `None` quand il est absent, illisible comme texte ou comme JSON ; un autre refus du
/// disque se leve.
fn lire_json_tolerant(p: &Path) -> R<Option<Value>> {
    if !p.exists() {
        return Ok(None);
    }
    let octets = std::fs::read(p).map_err(|e| erreur_io(&e, p))?;
    Ok(String::from_utf8(octets).ok().and_then(|t| serde_json::from_str(&t).ok()))
}

/// La longueur d'un texte, d'une liste ou d'un objet.
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

pub(crate) fn vers_route(x: crate::compagnie::coffre::Echec) -> Echec {
    Echec { classe: Some(x.classe), message: x.message }
}
