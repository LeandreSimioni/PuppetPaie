//! Les six routes d'ENVOI de l'interface (`moteur/web.py`) : `envoyer-documents`, `dossier-envoyer`, `casc-envoyer`
//! ici ; `demander-signature`, `avenant-envoyer`, `avenants-envoyer` dans `envois_signature.rs`. Lot H3b.
//!
//! ECRIT, NON PROUVE, NON BRANCHE. Chaque route fait ce que fait le Python : preparer ce qui partira (les memes
//! messages que l'apercu a montres), ouvrir la boite (`_client_courriel` : la boite simulee de la compagnie fictive,
//! sinon le vrai client, sinon le refus « boite absente »), exiger la PREUVE (le mot de passe du logiciel, retape :
//! `depot_acte::verifier_preuve`), envoyer par `envoi::envoyer`, puis noter au journal ce qui s'est passe --
//! l'echec comme la reussite -- et ecrire ce que l'envoi laisse derriere lui (contrats partis, numeros reserves,
//! `.eml` gardes). La reponse a les memes cles que celle de `web.py`.
//!
//! Rien ne part sans la preuve. Dans une compagnie fictive il n'y a rien a prouver : rien ne quitte le poste, la boite
//! simulee ecrit les messages dans `courriels-simules/`.
//!
//! LE FILET EN FOND de `demander-signature` (`ops.preparer_filet_en_fond`) N'EST PAS ICI : au branchement, `web.py`
//! le lancera apres l'appel Rust, comme pour `produire`. Le moteur n'a pas de fil.
//!
//! Les tests (`tests/routes_envois.rs`) passent par `route_avec`, qui prend de quoi ouvrir la boite : un faux
//! client du trait `Courriel`, ou la boite simulee. `route` ouvre la vraie.

use std::collections::HashMap;

use serde_json::{json, Map, Value};

use crate::contexte::Contexte;
use crate::courriel::{self, BoiteSimulee, Client, Courriel};
use crate::depot_acte::{verifier_preuve, Preuve};
use crate::envoi::{self, Message};
use crate::etats::{erreur, py_str};
use crate::fichiers::{repr, vrai};
use crate::journal::{self, Note};
use crate::retours::Echec;

pub(crate) type R<T> = Result<T, Echec>;

/// De quoi ouvrir la boite : appele UNE fois par route, a l'endroit ou le Python ouvre `_client_courriel`.
pub type Ouvrir<'a> = &'a mut dyn FnMut() -> R<Box<dyn Courriel>>;

/// Les routes de ce module (les trois autres sont dans `envois_signature`).
pub const ROUTES: [&str; 6] = ["envoyer-documents", "dossier-envoyer", "casc-envoyer", "demander-signature", "avenant-envoyer", "avenants-envoyer"];

/// Une route d'envoi, avec la vraie boite (ou la simulee).
pub fn route(o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    route_avec(o, ctx, nom, charge, &mut || client_courriel(ctx))
}

/// Une route d'envoi, la boite venant de `ouvrir`.
pub fn route_avec(o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value, ouvrir: Ouvrir) -> R<Value> {
    match nom {
        "envoyer-documents" => envoyer_documents(o, ctx, charge, ouvrir),
        "dossier-envoyer" => dossier_envoyer(ctx, charge, ouvrir),
        "casc-envoyer" => casc_envoyer(ctx, charge, ouvrir),
        "demander-signature" => crate::envois_signature::demander_signature(o, ctx, charge, ouvrir),
        "avenant-envoyer" => crate::envois_signature::avenant_envoyer(o, ctx, charge, ouvrir),
        "avenants-envoyer" => crate::envois_signature::avenants_envoyer(o, ctx, charge, ouvrir),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

// ------------------------------------------------------------------ ce que toutes les routes partagent

/// Le jour et l'heure de la ligne du journal (`datetime.now().isoformat(timespec="seconds")`, a l'heure de Paris).
pub(crate) fn maintenant() -> String {
    crate::incidents::maintenant()
}

/// `ErreurOperation` : un refus ordinaire, avec sa phrase.
pub(crate) fn refus(message: impl Into<String>) -> Echec {
    erreur("ErreurOperation", message)
}

/// `d[cle]` : `KeyError` si elle manque.
pub(crate) fn requis<'a>(d: &'a Value, cle: &str) -> R<&'a Value> {
    match d {
        Value::Object(o) => o.get(cle).ok_or_else(|| erreur("KeyError", crate::fichiers::absent(cle))),
        autre => Err(erreur("TypeError", crate::fichiers::attendu("un objet", crate::etats::nom_type(autre)))),
    }
}

/// `d.get(cle) or defaut` en texte : la valeur quand elle est vraie.
pub(crate) fn vrai_ou(d: &Value, cle: &str) -> Option<String> {
    d.get(cle).filter(|v| vrai(v)).map(py_str)
}

/// `charge[cle]` en texte (`KeyError` si elle manque).
pub(crate) fn charge_texte(charge: &Value, cle: &str) -> R<String> {
    Ok(py_str(requis(charge, cle)?))
}

/// Le `qui` du journal : celui que la charge donne (la ligne de commande : `ligne de commande`), sinon « interface ».
pub(crate) fn qui_de(charge: &Value) -> String {
    vrai_ou(charge, "qui").unwrap_or_else(|| "interface".to_string())
}

/// `Preuve(motdepasse=charge.get("motdepasse"))`.
pub(crate) fn preuve_de(charge: &Value) -> Preuve {
    Preuve::de_la_charge(charge)
}

/// `operations_documents._client_courriel` : la boite qui envoie. La compagnie fictive ecrit les messages, elle ne
/// les envoie pas ; sans mot de passe d'application au coffre, il n'y en a pas (`BoiteAbsente`).
pub(crate) fn client_courriel(ctx: &Contexte) -> R<Box<dyn Courriel>> {
    // UNE FICTIVE AJOUTEE n'a pas de boite qui envoie (ticket #236, `bridage::refus_d_envoi`) : la route s'arrete ici,
    // avant la preuve et avant toute ligne au journal des depots (pas meme `noter_echec_d_envoi`).
    crate::depot_acte::refuser_l_envoi(ctx)?;
    if crate::routes_accueil::simulation_active(&ctx.racine) {
        return Ok(Box::new(BoiteSimulee::du_contexte(ctx)));
    }
    if !courriel::disponible(&ctx.racine) {
        return Err(erreur(
            "BoiteAbsente",
            "Aucune boite courriel : poser le mot de passe d'application de la boite (Configuration > Mots de passe, ou `paie coffre poser courriel --compte adresse@...`).",
        ));
    }
    let client = Client::du_contexte(ctx).map_err(|e| erreur("ErreurCourriel", e.0))?;
    Ok(Box::new(client))
}

/// L'erreur d'un envoi : celle du client (`ErreurCourriel`), ou, quand des messages etaient deja partis, la
/// `RuntimeError` qui le dit (`envoi.envoyer`).
pub(crate) fn echec_d_envoi(message: String) -> Echec {
    erreur(if message.contains("\nATTENTION, ") { "RuntimeError" } else { "ErreurCourriel" }, message)
}

/// Retours Windows, C : la ligne du journal d'un envoi qui a echoue en route. Le destinataire est celui des messages
/// REELLEMENT partis (vide si aucun) ; l'adresse ou l'envoi a echoue est a part, dans le detail (`echec_a`), avec les
/// partis (`partis`). Jamais l'expediteur comme destinataire. `detail` : ce que la route garde d'ordinaire.
pub(crate) fn noter_echec_d_envoi(ctx: &Contexte, quoi: &str, mois: Option<&str>, qui: &str, e: &envoi::ErreurEnvoi, detail: Option<Value>) -> R<Value> {
    let mut d = match detail {
        Some(Value::Object(m)) => m,
        _ => Map::new(),
    };
    d.insert("partis".into(), json!(e.partis));
    d.insert("echec_a".into(), json!(e.echoue));
    let reponse = if e.partis.is_empty() { format!("echec de l'envoi a {} : {}", e.echoue, e.message) } else { e.message.clone() };
    noter(ctx, quoi, mois, qui, false, &e.partis.join(", "), &reponse, Some(&Value::Object(d)))
}

/// `journal.noter(ctx, quoi, ...)` : une ligne a la fin de `journal/depots.json`, en mode `reel` (une
/// compagnie fictive aussi : c'est ce que fait le Python). `qui` : « interface ».
#[allow(clippy::too_many_arguments)]
pub(crate) fn noter(
    ctx: &Contexte,
    quoi: &str,
    mois: Option<&str>,
    qui: &str,
    accepte: bool,
    destinataire: &str,
    reponse: &str,
    detail: Option<&Value>,
) -> R<Value> {
    let note = Note {
        quoi,
        mois,
        fichier: None,
        mode: journal::REEL,
        qui,
        accepte,
        destinataire,
        flux: "",
        reponse,
        trace: None,
        detail,
        quand: "",
        origine: "",
    };
    journal::noter(&ctx.racine, &note, &maintenant()).map_err(|m| erreur("OSError", m))
}

/// `html.escape(s)` : `&`, `<`, `>`, `"` et `'`.
pub(crate) fn html_echapper(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#x27;"),
            c => o.push(c),
        }
    }
    o
}

/// `urllib.parse.quote(s, safe=surs)` : tout est encode en UTF-8 par `%XX`, sauf les lettres et chiffres ASCII,
/// `_.-~` et `surs`.
pub(crate) fn pourcent(s: &str, surs: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"_.-~".contains(&b) || surs.as_bytes().contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{:02X}", b));
        }
    }
    o
}

/// `_mailto` : un lien `mailto:` (RFC 6068), objet et corps encodes comme une URL.
pub(crate) fn mailto(adresse: &str, objet: &str, corps: &str) -> String {
    format!("mailto:{}?subject={}&body={}", pourcent(adresse, "@"), pourcent(objet, ""), pourcent(corps, ""))
}

/// `date.fromisoformat(texte)` : `AAAA-MM-JJ` ou `AAAAMMJJ`.
pub(crate) fn jour_iso(texte: &str) -> Option<crate::date::Jour> {
    let chiffres = |t: &str| t.chars().all(|c| c.is_ascii_digit());
    let (a, m, j) = match texte.len() {
        10 if texte.is_char_boundary(4) && texte.is_char_boundary(7) && &texte[4..5] == "-" && &texte[7..8] == "-" => (&texte[..4], &texte[5..7], &texte[8..10]),
        8 if texte.is_ascii() => (&texte[..4], &texte[4..6], &texte[6..8]),
        _ => return None,
    };
    if !(chiffres(a) && chiffres(m) && chiffres(j)) {
        return None;
    }
    crate::date::Jour::nouveau(a.parse().ok()?, m.parse().ok()?, j.parse().ok()?)
}

/// `str.capitalize()`.
pub(crate) fn capitaliser(t: &str) -> String {
    let mut c = t.chars();
    match c.next() {
        Some(p) => p.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
        None => String::new(),
    }
}

/// Le nom d'un chemin, ou son texte entier s'il n'en a pas.
pub(crate) fn nom_du(chemin: &std::path::Path) -> String {
    chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| chemin.to_string_lossy().into_owned())
}

/// Les identifiants de la saisie : `expediteur`, la boite qui envoie, en minuscules.
fn expediteur_de(client: &dyn Courriel) -> String {
    client.compte().to_lowercase()
}

// ------------------------------------------------------------------ envoyer-documents

/// `operations_documents.envoyer_documents` : envoie a chaque salarie ses documents du mois. Irreversible.
fn envoyer_documents(o: &HashMap<String, String>, ctx: &Contexte, charge: &Value, ouvrir: Ouvrir) -> R<Value> {
    let mois = charge_texte(charge, "mois")?;
    envoyer_documents_de(o, ctx, &mois, &preuve_de(charge), &qui_de(charge), ouvrir)
}

/// `envoyer_documents(ctx, mois, preuve=..., qui=...)` : la meme operation, `qui` et la preuve donnes (l'horloge de
/// la compagnie fictive passe `horloge` et une boite muette).
pub(crate) fn envoyer_documents_de(o: &HashMap<String, String>, ctx: &Contexte, mois: &str, preuve: &Preuve, qui: &str, ouvrir: Ouvrir) -> R<Value> {
    let prepare = preparer_envoi(o, ctx, mois, ouvrir)?;
    expedier_envoi(ctx, prepare, mois, preuve, qui)
}

/// `operations_documents.EnvoiPrepare` : les messages, le compte verifie -- rien n'est parti.
pub(crate) struct EnvoiPrepare {
    pub(crate) mois_saisie: String,
    pub(crate) messages: Vec<Message>,
    pub(crate) expediteur: String,
    pub(crate) avertissements: Vec<String>,
    /// A qui les documents partent, et pourquoi (`choisir`) : le journal en garde l'empreinte et le resume.
    pub(crate) choix: Choix,
    client: Box<dyn Courriel>,
    /// La duree de la preparation (lecture du mois, messages, boite), en millisecondes : le journal la garde.
    preparation_ms: u128,
}

/// A QUI LES DOCUMENTS DU MOIS PARTENT (ticket #240, 21/10/2026 : « si on change une DSN, les documents ne sont plus
/// valables »). Chaque salarie du mois est dans l'un de trois cas, lus dans le journal (`etats::documents_partis`) et
/// dans la paie d'aujourd'hui :
///
/// * ses documents ne sont jamais partis : ils partent, avec le premier courriel ;
/// * ils sont partis, et sa paie a change depuis (l'empreinte notee a l'envoi n'est plus celle de sa paie,
///   `routes_mois::Moteur::empreintes_documents`) : ils repartent refaits, avec le courriel qui dit ce qui a change
///   (`envoi::CORPS_RENVOI`, `envoi::ce_qui_a_change` sur le resume note a l'envoi) ;
/// * ils sont partis et sa paie n'a pas bouge : ils ne repartent pas.
///
/// Quand TOUS sont a jour, l'envoi est un renvoi voulu, a l'identique : tous repartent, avec le premier courriel.
#[derive(Default)]
pub(crate) struct Choix {
    /// Ceux dont la paie a change depuis l'envoi de leurs documents : le jour de l'envoi, ce qui a change.
    pub(crate) renvois: HashMap<String, envoi::Renvoi>,
    /// Ceux dont les documents sont partis et dont la paie n'a pas change.
    pub(crate) a_jour: std::collections::HashSet<String>,
    /// L'empreinte et le resume de la paie de chaque salarie du mois, a noter au journal avec l'envoi.
    pub(crate) empreintes: HashMap<String, String>,
    pub(crate) resumes: HashMap<String, Value>,
}

impl Choix {
    /// Ce qui part, parmi les messages de tout le mois : ceux qui ne sont pas a jour ; s'ils le sont tous, tous.
    pub(crate) fn garder<T>(&self, messages: Vec<T>, cle: impl Fn(&T) -> &str) -> Vec<T> {
        if messages.iter().all(|m| self.a_jour.contains(cle(m))) {
            return messages;
        }
        messages.into_iter().filter(|m| !self.a_jour.contains(cle(m))).collect()
    }
}

/// Le choix, pour le mois `mois`, sur sa saisie `data` et ses paies calculees (`charger_le_mois_avec`).
pub(crate) fn choisir(ctx: &Contexte, mois: &str, data: &Value, paies: &[crate::calcul::Paie]) -> R<Choix> {
    let lignes = crate::etats::charger(&ctx.racine)?;
    let partis = crate::etats::documents_partis(&lignes, mois)?;
    let cles: Vec<String> = paies.iter().map(|p| p.salarie.get("_cle").map(py_str).unwrap_or_default()).collect();
    let engagements = data.get("engagements").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    let empreintes = crate::routes_mois::Moteur::empreintes_documents(engagements, &cles);
    let mut resumes: HashMap<String, Value> = HashMap::new();
    for cle in &cles {
        if resumes.contains_key(cle) {
            continue;
        }
        let siennes: Vec<&crate::calcul::Paie> = paies.iter().zip(&cles).filter(|(_, c)| *c == cle).map(|(p, _)| p).collect();
        resumes.insert(cle.clone(), envoi::resume_de(&siennes).map_err(Echec::from)?);
    }
    let mut choix = Choix { empreintes, resumes, ..Choix::default() };
    let declaration = declaration_de_la_correction(ctx, mois)?;
    for (cle, parti) in &partis {
        let notee = parti.get("empreinte").filter(|e| vrai(e)).map(py_str);
        match (notee, choix.empreintes.get(cle)) {
            (Some(n), Some(a)) if &n != a => {
                let quand = parti.get("quand").map(py_str).unwrap_or_default();
                let envoye_le = jour_iso(quand.get(..10).unwrap_or("")).map(|j| j.jj_mm_aaaa()).unwrap_or(quand);
                let ancien = parti.get("resume").cloned().unwrap_or(Value::Null);
                let changements = envoi::ce_qui_a_change(&ancien, choix.resumes.get(cle).unwrap_or(&Value::Null));
                let declaration = declaration.as_ref().map(|d| d(cle)).unwrap_or_default();
                choix.renvois.insert(cle.clone(), envoi::Renvoi { envoye_le, changements, declaration });
            }
            _ => {
                choix.a_jour.insert(cle.clone());
            }
        }
    }
    Ok(choix)
}

/// LA CORRECTION D'UN MOIS DONT L'ANNULE-ET-REMPLACE EST PASSE (Leandre, 21/10/2026 : « le courriel de renvoi, dans ce
/// cas, ajoute une phrase : la correction sera declaree avec la paie du mois suivant »). La DSN deposee ne se refait plus ;
/// la correction part dans la DSN d'un mois suivant, celle qui embarque les corrections ECRITES (« Corriger ce mois »,
/// `produire_regularisations::corrections_a_porter`) : celle du mois courant pour une correction en attente, celle qui
/// l'a preparee ou portee sinon. Le paragraphe nomme ce mois quand la correction du salarie est ecrite ; sans elle, il ne
/// dit que « une paie suivante » (l'etape DSN du mois reste orange tant qu'elle ne l'est pas). `None` : le mois n'en est
/// pas la (pas de DSN deposee, ou son annule-et-remplace encore admis).
#[allow(clippy::type_complexity)]
fn declaration_de_la_correction(ctx: &Contexte, mois: &str) -> R<Option<Box<dyn Fn(&str) -> String>>> {
    let o = HashMap::new();
    let mut m = crate::routes_mois::Moteur::nouveau(&o, ctx);
    let etat = crate::routes_regularisation::etat_mois(&mut m, ctx, &json!(mois))?;
    if !vrai(&etat["regularisable"]) {
        return Ok(None);
    }
    let courant = py_str(&etat["mois_courant"]);
    let mut porteurs: HashMap<String, String> = HashMap::new();
    for c in crate::routes_ecrites::lire_corrections(ctx, mois)? {
        if c.get("annulee_le").is_some_and(vrai) {
            continue;
        }
        let par = |k: &str| c.get(k).filter(|p| vrai(p)).and_then(|p| p.get("mois")).map(py_str);
        let porteur = par("portee_par").or_else(|| par("preparee_par")).unwrap_or_else(|| courant.clone());
        porteurs.insert(c.get("salarie").map(py_str).unwrap_or_default(), porteur);
    }
    let de = crate::routes_regularisation::de_mois_en_clair(mois);
    Ok(Some(Box::new(move |cle: &str| match porteurs.get(cle) {
        Some(p) => format!(
            "Ta paie {} était déjà déclarée : la correction sera déclarée avec la paie {}.\n\n",
            de,
            crate::routes_regularisation::de_mois_en_clair(p)
        ),
        None => format!("Ta paie {} était déjà déclarée : la correction sera déclarée avec une paie suivante.\n\n", de),
    })))
}

/// `operations_documents.preparer_envoi` : la saisie du mois, les messages, puis la boite -- une boite absente se voit
/// avant la preuve. Rien n'est parti.
pub(crate) fn preparer_envoi(o: &HashMap<String, String>, ctx: &Contexte, mois: &str, ouvrir: Ouvrir) -> R<EnvoiPrepare> {
    let debut = std::time::Instant::now();
    let (data, paies) = crate::routes_documents::charger_le_mois_avec(o, ctx, mois, false)?;
    let mois_saisie = data.get("mois").map(crate::date::en_texte).ok_or_else(|| erreur("KeyError", crate::fichiers::absent("mois")))?;
    let dossier = ctx.racine.join("sorties").join(&mois_saisie).join("envoi");
    if !dossier.exists() {
        return Err(erreur("SortieAbsente", format!("{} n'existe pas.", dossier.display())));
    }
    let fiche = |cle: &str| ctx.salarie(cle).unwrap_or_else(|_| json!({}));
    let choix = choisir(ctx, mois, &data, &paies)?;
    // APRES LA LIMITE DE L'ANNULE-ET-REMPLACE, rien ne refait le mois (`produire` refuse, la DSN deposee ne se regenere
    // pas, regle 4) : les documents de ceux dont la paie a change se refont ICI, depuis la paie d'aujourd'hui, aux numeros
    // du fichier archive (`ecrire_documents`, qui ne redeclare rien), avant de repartir.
    if !choix.renvois.is_empty() && choix.renvois.values().any(|r| !r.declaration.is_empty()) {
        crate::ecrire_documents::ecrire_documents(ctx, &json!({"mois": mois_saisie}))?;
    }
    let messages = envoi::preparer_avec(&paies, &ctx.entreprise, &mois_saisie, &dossier, Some(&fiche), &choix.renvois).map_err(|m| refus_de_preparation(&m))?;
    let messages = choix.garder(messages, |m| m.cle.as_str());
    let client = ouvrir()?;
    let expediteur = expediteur_de(client.as_ref());
    let mut avertissements: Vec<String> = Vec::new();
    let signataire = requis(&ctx.entreprise, "signataire")?.get("email").filter(|v| vrai(v)).map(py_str).unwrap_or_default().to_lowercase();
    if !signataire.is_empty() && signataire != expediteur {
        avertissements.push(format!(
            "Les messages partiront de {} (la boite posee), pas de {} (le courriel du signataire, fiche Entreprise).",
            expediteur, signataire
        ));
    }
    Ok(EnvoiPrepare { mois_saisie, messages, expediteur, avertissements, choix, client, preparation_ms: debut.elapsed().as_millis() })
}

/// Ce qui suit la preparation : la PREUVE (le mot de passe du logiciel), l'envoi, le journal. `qui` : la ligne du journal.
pub(crate) fn expedier_envoi(ctx: &Contexte, prepare: EnvoiPrepare, mois: &str, preuve: &Preuve, qui: &str) -> R<Value> {
    let EnvoiPrepare { messages, expediteur, avertissements, choix, mut client, preparation_ms, .. } = prepare;
    let a_qui = format!("{} salarie(s) : {}", messages.len(), messages.iter().map(|m| m.nom.as_str()).collect::<Vec<_>>().join(", "));
    // LA DUREE DE CHAQUE ETAPE (08/10/2026 : un envoi de deux courriels a pris quatre minutes, sans qu'on sache ou).
    let t_preuve = std::time::Instant::now();
    verifier_preuve(ctx, Some(preuve), mois, "envoi des documents du mois aux salaries", &a_qui)?;
    let preuve_ms = t_preuve.elapsed().as_millis();
    let t_envoi = std::time::Instant::now();
    let partis = match envoi::envoyer_suivi(&messages, &expediteur, client.as_mut(), None, None) {
        Ok(p) => p,
        Err(e) => {
            noter_echec_d_envoi(ctx, "documents", Some(mois), qui, &e, None)?;
            return Err(echec_d_envoi(e.message));
        }
    };
    let envoi_ms = t_envoi.elapsed().as_millis();
    let detail = json!({
        // Ce que chaque salarie a recu : ses pieces, l'empreinte de sa paie et son resume (ticket #240 : ce qui dira, si sa
        // paie change, que ses documents sont a renvoyer, et ce qui a change), et si c'etait un renvoi.
        "salaries": messages.iter().map(|m| json!({
            "cle": m.cle, "nom": m.nom, "pieces": m.pieces.iter().map(|p| nom_du(p)).collect::<Vec<_>>(),
            "empreinte": choix.empreintes.get(&m.cle), "resume": choix.resumes.get(&m.cle),
            "renvoi": choix.renvois.contains_key(&m.cle),
        })).collect::<Vec<_>>(),
        "expediteur": expediteur,
        "durees_ms": {"preparation": preparation_ms as u64, "mot_de_passe": preuve_ms as u64, "envoi": envoi_ms as u64},
    });
    let ligne = noter(ctx, "documents", Some(mois), qui, true, &partis.join(", "), &format!("{} message(s) envoye(s)", partis.len()), Some(&detail))?;
    Ok(json!({
        "partis": partis,
        "messages": messages.len(),
        "expediteur": expediteur,
        "avertissements": avertissements,
        "ligne": ligne,
    }))
}

/// Le refus de `envoi.preparer` : une fiche sans adresse est un `ValueError`, une cle absente un `KeyError`.
fn refus_de_preparation(m: &str) -> Echec {
    if m.starts_with("Pas d'adresse e-mail") {
        erreur("ValueError", m)
    } else if m.starts_with("champ ") && m.ends_with(" absent") {
        erreur("KeyError", m)
    } else {
        Echec::from(m.to_string())
    }
}

// ------------------------------------------------------------------ dossier-envoyer

const CORPS_DOSSIER: &str = "Bonjour {prenom},\n\nTu trouveras ci-joint ton dossier de salarié{periode} : {liste}.\n\n{reedition}Bien à toi,\n{signataire}\n{raison_sociale}\n";

/// Ce que le courriel dit des contrats, quand le dossier en porte : eux seuls se reeditent (les bulletins et les
/// certificats sont les pages memes de l'archive, lot V1p ; le reste se prend tel quel).
const REEDITION_CONTRATS: &str = "Les contrats ont été réédités par notre logiciel de paie depuis les données\nd'origine : les numéros et les montants sont ceux des contrats qui t'ont été\nremis ; seule la mise en page peut avoir changé depuis.\n\n";

/// La route `dossier-envoyer` : le dossier se refait ici (`dossier_salarie`, un fichier par salarie, ecrit dans
/// `exports/dossiers/`), puis part a chaque salarie par la boite de la compagnie. La reponse est celle du dossier
/// (`dossier_json`) et, sous `envoi`, ce que l'envoi a fait.
fn dossier_envoyer(ctx: &Contexte, charge: &Value, ouvrir: Ouvrir) -> R<Value> {
    let qui = qui_de(charge);
    // `ops.dossier_salarie(salaries=..., mois=..., annee=..., toutes_les_annees=..., categories=..., ensemble=False, ecrire=True)`.
    let mut demande = Map::new();
    for cle in ["salaries", "mois", "annee", "toutes_les_annees", "categories"] {
        if let Some(v) = charge.get(cle) {
            demande.insert(cle.to_string(), v.clone());
        }
    }
    let d = crate::routes_dossier_salarie::construire(ctx, &Value::Object(demande))?;
    let mut r = crate::routes_dossier_salarie::dossier_json(&d);
    let envoi = envoyer_dossier(ctx, &d, &preuve_de(charge), &qui, ouvrir)?;
    if let Value::Object(o) = &mut r {
        o.insert("envoi".into(), envoi);
    }
    Ok(r)
}

/// `operations_dossier.envoyer_dossier` : envoie a chaque salarie SON dossier, par la boite de la compagnie.
/// Irreversible : la meme preuve que les pieces du mois, le meme journal. Un dossier `ensemble` ne part a personne.
pub(crate) fn envoyer_dossier(ctx: &Contexte, d: &crate::routes_dossier_salarie::Dossier, preuve: &Preuve, qui: &str, ouvrir: Ouvrir) -> R<Value> {
    if d.fichiers.iter().any(|f| nom_du(f).starts_with("Dossiers_")) {
        return Err(refus(
            "Un dossier qui réunit tous les salariés ne s'envoie à personne : il porte la paie de chacun. Refaire un fichier par salarié.",
        ));
    }
    if d.fichiers.len() != d.salaries.len() {
        return Err(refus("Le dossier n'a pas été écrit : rien à envoyer."));
    }
    let mut messages: Vec<Message> = Vec::new();
    for (cle, f) in d.salaries.iter().zip(&d.fichiers) {
        let s = ctx.salaries.get(cle).ok_or_else(|| erreur("KeyError", crate::fichiers::absent(cle)))?;
        let Some(email) = s.get("email").filter(|v| vrai(v)).map(py_str) else {
            return Err(refus(format!(
                "Pas d'adresse e-mail pour {} {} : compléter sa fiche avant d'envoyer. Rien n'est parti.",
                py_str(requis(s, "nom")?),
                py_str(requis(s, "prenom")?)
            )));
        };
        let prenom = match vrai_ou(s, "prenom_affichage") {
            Some(p) => p,
            None => py_str(requis(s, "prenom")?),
        };
        let signataire = ctx.entreprise.get("signataire").filter(|v| vrai(v)).cloned().unwrap_or_else(|| json!({}));
        let ses_pages = d.pages.get(cle).and_then(Value::as_object);
        let liste = d
            .categories
            .iter()
            .filter(|c| ses_pages.is_some_and(|p| p.contains_key(**c)))
            .map(|c| crate::routes_dossier_salarie::libelle_de(c).to_lowercase())
            .collect::<Vec<_>>()
            .join(", ");
        let corps = envoi::formater(
            CORPS_DOSSIER,
            &[
                ("prenom", prenom.clone()),
                ("periode", if d.periode == "toute la période" { String::new() } else { format!(" ({})", d.periode) }),
                ("liste", liste),
                ("reedition", if ses_pages.is_some_and(|p| p.contains_key("contrats")) { REEDITION_CONTRATS.to_string() } else { String::new() }),
                ("signataire", vrai_ou(&signataire, "prenom_affichage").or_else(|| signataire.get("prenom").map(py_str)).unwrap_or_default()),
                ("raison_sociale", vrai_ou(&ctx.entreprise, "nom_affichage").or_else(|| ctx.entreprise.get("raison_sociale").map(py_str)).unwrap_or_default()),
            ],
        );
        messages.push(Message {
            cle: cle.clone(),
            nom: format!("{} {}", prenom, py_str(requis(s, "nom")?)),
            destinataire: email.to_lowercase(),
            sujet: format!("Ton dossier de salarié — {}", d.periode),
            corps,
            pieces: vec![f.clone()],
            html: String::new(),
        });
    }
    let mut client = ouvrir()?;
    let aujourdhui = crate::routes_dossier_salarie::aujourdhui_reel();
    let a_qui = format!("{} salarie(s) : {}", messages.len(), messages.iter().map(|m| m.nom.as_str()).collect::<Vec<_>>().join(", "));
    verifier_preuve(ctx, Some(preuve), &format!("{:04}-{:02}", aujourdhui.an, aujourdhui.mois), "envoi du dossier aux salaries", &a_qui)?;
    let expediteur = expediteur_de(client.as_ref());
    let partis = match envoi::envoyer_suivi(&messages, &expediteur, client.as_mut(), None, None) {
        Ok(p) => p,
        Err(e) => {
            noter_echec_d_envoi(ctx, "dossier-salarie", None, qui, &e, None)?;
            return Err(echec_d_envoi(e.message));
        }
    };
    let detail = json!({
        "periode": d.periode,
        "categories": d.categories,
        "salaries": messages.iter().map(|m| json!({"cle": m.cle, "piece": m.pieces.first().map(|p| nom_du(p))})).collect::<Vec<_>>(),
        "expediteur": expediteur,
    });
    let ligne = noter(ctx, "dossier-salarie", None, qui, true, &partis.join(", "), "", Some(&detail))?;
    Ok(json!({"partis": partis, "messages": messages.len(), "expediteur": expediteur, "ligne": ligne}))
}

// ------------------------------------------------------------------ casc-envoyer

/// `operations_casc._html` : le courriel en HTML, le meme texte, mais le tableau des masses en vrai tableau -- en
/// texte brut, ses barres ne s'alignent que dans une police a chasse fixe, ce qu'aucune messagerie n'emploie.
pub(crate) fn courriel_en_html(corps: &str) -> String {
    let blanc = |c: char| c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c);
    let cellule = |c: &str, balise: &str| -> String {
        let c = c.trim_matches(blanc);
        let gras = c.starts_with("**") && c.ends_with("**");
        let c = html_echapper(c.trim_matches('*'));
        let mut style = "padding:3px 10px;border-bottom:1px solid #ddd".to_string();
        if c.chars().any(|ch| ch.is_ascii_digit()) && c.contains('€') {
            style.push_str(";text-align:right");
        }
        format!("<{b} style=\"{s}\">{c}</{b}>", b = balise, s = style, c = if gras { format!("<b>{}</b>", c) } else { c })
    };
    let mut blocs: Vec<String> = Vec::new();
    for para in corps.split("\n\n") {
        let lignes: Vec<&str> = para.split('\n').collect();
        if lignes.iter().filter(|l| !l.is_empty()).all(|l| l.starts_with('|')) {
            let tableau: Vec<Vec<&str>> = lignes.iter().filter(|l| !l.is_empty()).map(|l| l.trim_matches('|').split('|').collect()).collect();
            if tableau.is_empty() {
                continue;
            }
            let entete: String = tableau[0].iter().map(|c| cellule(c, "th")).collect();
            let rangees: String = tableau.iter().skip(2).map(|l| format!("<tr>{}</tr>", l.iter().map(|c| cellule(c, "td")).collect::<String>())).collect();
            blocs.push(format!("<table style=\"border-collapse:collapse\"><tr>{}</tr>{}</table>", entete, rangees));
        } else {
            blocs.push(format!("<p>{}</p>", lignes.iter().map(|l| html_echapper(l)).collect::<Vec<_>>().join("<br>")));
        }
    }
    format!("<div style=\"font-family:sans-serif;font-size:14px\">{}</div>", blocs.concat())
}

/// `operations_casc._jjmmaaaa`.
fn jjmmaaaa(iso: &str) -> String {
    if iso.is_empty() {
        return String::new();
    }
    let c: Vec<char> = iso.chars().collect();
    let t = |a: usize, b: usize| -> String { c.iter().skip(a).take(b - a).collect() };
    format!("{}/{}/{}", t(8, 10), t(5, 7), t(0, 4))
}

/// `operations_casc.envoyer_regularisation` : envoie les deux courriels de la regularisation, APRES les virements.
/// Irreversible. `vire_le` : {annee: AAAA-MM-JJ}, une date par virement fait ; le dossier envoye est celui de ces
/// annees-la, exactement.
///
/// Dans cet ordre : le dossier (ecrit, pour que la liste nominative jointe soit celle du jour), la boite, la
/// preuve, l'envoi, puis ce qui s'en garde -- les deux `.eml` dans `exports/casc-svp/envoyes/`, la ligne de
/// journal, et les annees notees regularisees. Rien de tout cela ne s'ecrit si l'envoi casse.
fn casc_envoyer(ctx: &Contexte, charge: &Value, ouvrir: Ouvrir) -> R<Value> {
    let qui = qui_de(charge);
    use crate::casc_regularisation::{ecrire, marquer_regularise, regularisation, Demande};
    // `charge.get("vire_le") or {}`.
    let vire_le: Vec<(String, Value)> = match charge.get("vire_le").filter(|v| vrai(v)) {
        None => Vec::new(),
        Some(Value::Object(o)) => o.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        Some(autre) => return Err(erreur("AttributeError", crate::fichiers::attendu("un objet", crate::etats::nom_type(autre)))),
    };
    let aujourdhui = crate::routes_dossier_salarie::aujourdhui_reel();
    let mut dates: Vec<(String, String)> = Vec::new();
    for (a, d) in &vire_le {
        let texte = py_str(d);
        let Some(jour) = jour_iso(&texte) else {
            return Err(refus(format!("Date du virement {} illisible : {} (attendu AAAA-MM-JJ). Rien n'est parti.", a, repr(&d_en_repr(d)))));
        };
        if jour > aujourdhui {
            return Err(refus(format!(
                "Le virement {} est date du {}, qui n'est pas encore arrive : le courriel annonce un virement FAIT. Rien n'est parti.",
                a,
                jjmmaaaa(&jour.iso())
            )));
        }
        match dates.iter_mut().find(|(k, _)| k == a) {
            Some(e) => e.1 = jour.iso(),
            None => dates.push((a.clone(), jour.iso())),
        }
    }
    if dates.is_empty() {
        return Err(refus("Aucune date de virement : les courriels annoncent des virements faits, ils partent apres. Rien n'est parti."));
    }
    let mut annees_voulues: Vec<String> = dates.iter().map(|(a, _)| a.clone()).collect();
    annees_voulues.sort();

    // Le dossier, ecrit : la liste nominative jointe est celle du jour.
    let mut r = regularisation(ctx, &Demande { toutes_les_annees: true, annees: annees_voulues.clone(), vire_le: dates.clone(), ..Default::default() })?;
    ecrire(ctx, &mut r)?;
    let mut manquent: Vec<String> = annees_voulues.iter().filter(|a| !r.annees.iter().any(|x| &x.annee == *a)).cloned().collect();
    manquent.sort();
    if !manquent.is_empty() {
        return Err(refus(format!("Rien a regulariser pour {} (deja fait, ou aucune DSN deposee). Rien n'est parti.", manquent.join(", "))));
    }
    let classeur = std::path::Path::new(&r.dossier).join("CASC-SVP_liste-nominative.xlsx");
    if !classeur.exists() {
        return Err(refus(format!(
            "La liste nominative n'a pas pu etre remplie ({}) : le courriel au CASC-SVP la promet en piece jointe. Rien n'est parti.",
            r.avertissements.last().cloned().unwrap_or_default()
        )));
    }

    let mut client = ouvrir()?;
    let dernier_mois = r.mois.last().map(|m| m.mois.clone()).ok_or_else(|| erreur("IndexError", crate::fichiers::trop_courte("les mois de la régularisation")))?;
    verifier_preuve(ctx, Some(&preuve_de(charge)), &dernier_mois, "envoi a Audiens et au CASC-SVP (liste nominative)", "Audiens et CASC-SVP")?;

    let messages = vec![
        Message {
            cle: "audiens".into(),
            nom: "Audiens".into(),
            destinataire: r.courriel_audiens.a.clone(),
            sujet: r.courriel_audiens.objet.clone(),
            corps: r.courriel_audiens.corps.clone(),
            pieces: Vec::new(),
            html: courriel_en_html(&r.courriel_audiens.corps),
        },
        Message {
            cle: "casc".into(),
            nom: "CASC-SVP".into(),
            destinataire: r.courriel_casc.a.clone(),
            sujet: r.courriel_casc.objet.clone(),
            corps: r.courriel_casc.corps.clone(),
            pieces: vec![classeur],
            html: String::new(),
        },
    ];
    let compte = client.compte().to_string();
    let mut copies: Vec<Vec<u8>> = Vec::new();
    let partis = match envoi::envoyer_suivi(&messages, &compte, client.as_mut(), None, Some(&mut copies)) {
        Ok(p) => p,
        Err(e) => {
            noter_echec_d_envoi(ctx, "regularisation-casc", None, &qui, &e, Some(json!({"annees": annees_voulues})))?;
            return Err(echec_d_envoi(e.message));
        }
    };

    // Ce que l'envoi garde : les deux messages tels qu'ils sont partis.
    let garde = std::path::Path::new(&r.dossier).join("envoyes");
    std::fs::create_dir_all(&garde).map_err(|e| erreur("OSError", format!("{} : {}", garde.display(), e)))?;
    let iso = maintenant();
    let horodate = format!("{}_{}{}{}", &iso[..10], &iso[11..13], &iso[14..16], &iso[17..19]);
    let mut emls: Vec<String> = Vec::new();
    for (m, brut) in messages.iter().zip(&copies) {
        let p = garde.join(format!("{}_CASC-SVP_{}.eml", horodate, m.cle));
        std::fs::write(&p, brut).map_err(|e| erreur("OSError", format!("{} : {}", p.display(), e)))?;
        emls.push(nom_du(&p));
    }
    let date_de = |a: &str| dates.iter().find(|(k, _)| k == a).map(|(_, d)| d.clone()).unwrap_or_default();
    let detail = json!({
        "annees": r.annees.iter().map(|a| json!({"annee": a.annee, "vire_le": date_de(&a.annee), "montant": a.cotisation.to_string()})).collect::<Vec<_>>(),
        "mois": r.mois.iter().map(|m| m.mois.clone()).collect::<Vec<_>>(),
        "total": r.total.to_string(),
        "expediteur": compte,
        "eml": emls,
    });
    let ligne = noter(ctx, "regularisation-casc", None, &qui, true, &partis.join(", "), &format!("{} courriel(s) envoye(s)", partis.len()), Some(&detail))?;
    // Les annees regularisees : leurs mois sortent des dossiers suivants. S'ecrit APRES le virement, jamais avant.
    let auj = maintenant();
    let mut notees: Vec<Value> = Vec::new();
    for a in &r.annees {
        notees.push(marquer_regularise(ctx, &json!(a.annee), &json!(date_de(&a.annee)), &json!(a.cotisation.to_string()), &qui, &auj[..10])?);
    }
    Ok(json!({
        "partis": partis,
        "expediteur": compte,
        "periode": r.periode,
        "total": r.total.to_string(),
        "annees": notees,
        "eml": emls,
        "dossier": garde.to_string_lossy(),
        "ligne": ligne,
    }))
}

/// Le texte d'une valeur de `vire_le` comme `%r` l'ecrit : ce qui n'est pas un texte s'ecrit sans guillemets.
fn d_en_repr(d: &Value) -> String {
    py_str(d)
}
