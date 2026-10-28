//! Les routes d'ENVOI de l'interface : `envoyer-documents`, `dossier-envoyer`, `casc-envoyer`, et celles des contrats
//! (`demander-signature`, `contrats-imprimer`, `avenant-envoyer`, `avenants-envoyer`), qui appellent `envois::envois_signature`.
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

use droit::contexte::Contexte;
use envois::courriel::{BoiteSimulee, Courriel};
use envois::envois_signature::{Choisis, UnAvenant};
use crate::depot_acte::{verifier_preuve, Preuve};
use envois::envoi::{self, Message};
use socle::valeur::valeur_en_texte;
use socle::echec::erreur;
use socle::fichiers::{repr, vrai};
use envois::journal;
use envois::ligne_d_envoi::{echec_d_envoi, noter, noter_echec_d_envoi, noter_envoi};
use socle::echec::Echec;
use socle::date::jour_iso;
use socle::fichiers::nom_du;
use socle::html::courriel_en_html;

pub(crate) type R<T> = Result<T, Echec>;

/// De quoi ouvrir la boite : appele UNE fois par route, a l'endroit ou le Python ouvre `_client_courriel`.
pub type Ouvrir<'a> = &'a mut dyn FnMut() -> R<Box<dyn Courriel>>;

/// Les routes de ce module (les trois autres sont dans `envois_signature`).
pub const ROUTES: [&str; 8] = ["envoyer-documents", "documents-imprimer", "dossier-envoyer", "casc-envoyer", "demander-signature", "contrats-imprimer", "avenant-envoyer", "avenants-envoyer"];

/// Une route d'envoi, avec la vraie boite (ou la simulee).
pub fn route(o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    route_avec(o, ctx, nom, charge, &mut || client_courriel(ctx))
}

/// Une route d'envoi, la boite venant de `ouvrir`.
pub fn route_avec(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value, ouvrir: Ouvrir) -> R<Value> {
    match nom {
        "envoyer-documents" => envoyer_documents(ctx, charge, ouvrir),
        "documents-imprimer" => documents_imprimer(ctx, charge, &qui_de(charge)),
        "dossier-envoyer" => dossier_envoyer(ctx, charge, ouvrir),
        "casc-envoyer" => casc_envoyer(ctx, charge, ouvrir),
        "demander-signature" => demander_signature(ctx, charge, ouvrir),
        "contrats-imprimer" => contrats_imprimer_par(ctx, charge, "interface"),
        "avenant-envoyer" => avenant_envoyer(ctx, charge, ouvrir),
        "avenants-envoyer" => avenants_envoyer(ctx, charge, ouvrir),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

// ------------------------------------------------------------------ les contrats (`envois::envois_signature`)

/// Les elements d'une liste de la charge, ou rien : `charge.get(cle)` quand il est vrai.
fn liste_de<'a>(charge: &'a Value, cle: &str) -> R<Option<&'a Vec<Value>>> {
    match charge.get(cle).filter(|v| vrai(v)) {
        None => Ok(None),
        Some(Value::Array(a)) => Ok(Some(a)),
        Some(autre) => Err(erreur("TypeError", format!("`{}` : une liste est attendue, pas {}", cle, socle::valeur::nom_type(autre)))),
    }
}

/// Les contrats que la charge choisit : `salaries` (tous leurs contrats) et `contrats` ([{"salarie", "debut", "numero"}]).
fn choisis_de(charge: &Value) -> R<Choisis> {
    let salaries = liste_de(charge, "salaries")?.map(|l| l.iter().map(valeur_en_texte).collect());
    let contrats = match liste_de(charge, "contrats")? {
        None => None,
        Some(l) => {
            let mut v = Vec::new();
            for c in l {
                let numero = c.get("numero").filter(|n| vrai(n)).map(valeur_en_texte);
                v.push((valeur_en_texte(requis(c, "salarie")?), valeur_en_texte(requis(c, "debut")?), numero));
            }
            Some(v)
        }
    };
    Ok(Choisis { salaries, contrats })
}

/// La route `contrats-imprimer` (`envois_signature::contrats_imprimer`), `qui` donne (l'horloge d'une compagnie fictive
/// passe le sien).
pub(crate) fn contrats_imprimer_par(ctx: &Contexte, charge: &Value, qui: &str) -> R<Value> {
    envois::envois_signature::contrats_imprimer(ctx, &charge_texte(charge, "mois")?, &choisis_de(charge)?, qui)
}

/// La route `demander-signature` : la boite, la preuve, puis `envois_signature::demander_signature`. `relance` : l'etat
/// des contrats du mois (`signature-apercu`) dit lesquels peuvent se relancer.
pub(crate) fn demander_signature(ctx: &Contexte, charge: &Value, ouvrir: Ouvrir) -> R<Value> {
    let mois = charge_texte(charge, "mois")?;
    let relance = charge.get("relance").is_some_and(vrai);
    let remplace = charge.get("remplace").is_some_and(vrai);
    let choisis = choisis_de(charge)?;
    let a_qui = match &choisis.salaries {
        Some(l) if !l.is_empty() => format!("{} salarie(s) : {}", l.len(), l.join(", ")),
        _ => "les salaries du mois qui ont un contrat a signer".to_string(),
    };
    let etats_des_contrats = if relance {
        let apercu = crate::routes_documents::signature_apercu(ctx, &json!({"mois": mois}))?;
        apercu.get("contrats").and_then(Value::as_array).cloned().unwrap_or_default()
    } else {
        Vec::new()
    };
    let demande = envois::envois_signature::DemandeDeSignature { mois: &mois, relance, remplace, choisis, etats_des_contrats };
    // Un refus se dit d'abord : la boite et la preuve ne se demandent que pour ce qui part.
    envois::envois_signature::verifier_demande(ctx, &demande)?;
    let mut client = ouvrir()?;
    let quoi = if relance { "relance de demande de signature de contrat" } else if remplace { "contrat refait, a signer (il annule et remplace le precedent)" } else { "demande de signature de contrat" };
    verifier_preuve(ctx, Some(&preuve_de(charge)), &mois, quoi, &a_qui)?;
    envois::envois_signature::demander_signature(ctx, &demande, client.as_mut())
}

/// La route `avenant-envoyer` : un seul avenant. La reponse : `partis`, `fichier`, `numero`, `ligne`.
fn avenant_envoyer(ctx: &Contexte, charge: &Value, ouvrir: Ouvrir) -> R<Value> {
    let voulu = json!({
        "salarie": requis(charge, "salarie")?, "debut": requis(charge, "debut")?, "effet": charge.get("effet").cloned().unwrap_or(Value::Null),
        "numero": charge.get("numero").cloned().unwrap_or(Value::Null), "mois": charge.get("mois").cloned().unwrap_or(Value::Null),
    });
    let r = envoyer_avenants(ctx, &[voulu], &preuve_de(charge), ouvrir)?;
    let a = r.avenants.first().ok_or_else(|| erreur("ValueError", "aucun avenant n'est parti"))?;
    Ok(json!({"partis": r.partis, "fichier": a["fichier"], "numero": a["numero"], "ligne": r.lignes.first().cloned().unwrap_or(Value::Null)}))
}

/// La route `avenants-envoyer` : les avenants coches, UN COURRIEL PAR SALARIE. La reponse : `partis`, `avenants`.
fn avenants_envoyer(ctx: &Contexte, charge: &Value, ouvrir: Ouvrir) -> R<Value> {
    let voulus: Vec<Value> = match requis(charge, "avenants")? {
        Value::Array(a) => a.clone(),
        Value::Null => Vec::new(),
        autre => return Err(erreur("TypeError", format!("`avenants` : une liste est attendue, pas {}", socle::valeur::nom_type(autre)))),
    };
    let r = envoyer_avenants(ctx, &voulus, &preuve_de(charge), ouvrir)?;
    Ok(json!({"partis": r.partis, "avenants": r.avenants}))
}

/// `jj_mm_aaaa(x)` : `ValueError` si `x` ne se lit pas comme une date.
fn jj(iso: &str) -> R<String> {
    socle::date::Jour::lire(iso).map(|j| j.jj_mm_aaaa()).map_err(|m| erreur("ValueError", m))
}

/// LES AVENANTS VOULUS : chacun a son apercu (`avenant-apercu`), qui dit ce qui change ; tout se verifie avant que rien ne
/// parte (un avenant sans objet, une date d'effet illisible ou un salarie sans courriel refuse l'envoi entier), puis la
/// boite, la preuve, et `envois_signature::envoyer_avenants`.
fn envoyer_avenants(ctx: &Contexte, voulus: &[Value], preuve: &Preuve, ouvrir: Ouvrir) -> R<envois::envois_signature::Avenants> {
    if voulus.is_empty() {
        return Err(refus("Aucun avenant coché. Rien n'est parti."));
    }
    // La porte du branchement, avant de rien lire.
    crate::bridage::branchement(&ctx.racine).ouvrir()?;
    let mut par_salarie: Vec<(String, Vec<UnAvenant>)> = Vec::new();
    for v in voulus {
        let salarie_v = requis(v, "salarie")?;
        let debut_v = requis(v, "debut")?;
        // `numero` (celui que `signature-apercu` rend) et `mois` (celui de l'ecran) disent QUEL contrat du jour : facultatifs.
        let apercu = crate::routes_documents::avenant_apercu(
            ctx,
            &json!({"salarie": salarie_v, "debut": debut_v, "numero": v.get("numero").cloned().unwrap_or(Value::Null), "mois": v.get("mois").cloned().unwrap_or(Value::Null)}),
        )?;
        let modifications = apercu["modifications"].as_array().map_or(0, Vec::len);
        let nom = apercu.get("nom").map(valeur_en_texte).unwrap_or_default();
        if modifications == 0 {
            return Err(refus(format!(
                "{}, contrat du {} : le contrat parti dit déjà ce que dit la paie, pas d'avenant à faire. Rien n'est parti.",
                nom,
                jj(&valeur_en_texte(debut_v))?
            )));
        }
        let effet = vrai_ou(v, "effet").unwrap_or_else(|| apercu.get("effet").map(valeur_en_texte).unwrap_or_default());
        if jour_iso(&effet).is_none() {
            return Err(refus(format!("Date d'effet illisible : {}.", repr(&effet))));
        }
        if apercu.get("destinataire").map(valeur_en_texte).unwrap_or_default().trim().is_empty() {
            return Err(refus(format!("{} n'a pas de courriel dans sa fiche : l'avenant ne peut pas partir.", valeur_en_texte(salarie_v))));
        }
        let cle = valeur_en_texte(salarie_v);
        let un = UnAvenant { apercu, effet };
        match par_salarie.iter_mut().find(|(k, _)| *k == cle) {
            Some((_, l)) => l.push(un),
            None => par_salarie.push((cle, vec![un])),
        }
    }
    let mut client = ouvrir()?;
    let mut mois_vus: Vec<String> = par_salarie.iter().flat_map(|(_, aa)| aa.iter().map(|a| a.texte("mois"))).collect();
    mois_vus.sort();
    let a_qui = format!("{} salarie(s) : {}", par_salarie.len(), par_salarie.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>().join(", "));
    verifier_preuve(ctx, Some(preuve), mois_vus.first().map_or("", String::as_str), "envoi d'avenant(s) a signer", &a_qui)?;
    envois::envois_signature::envoyer_avenants(ctx, &par_salarie, client.as_mut())
}

// ------------------------------------------------------------------ ce que toutes les routes partagent

/// `ErreurOperation` : un refus ordinaire, avec sa phrase.
pub(crate) fn refus(message: impl Into<String>) -> Echec {
    erreur("ErreurOperation", message)
}

/// `d[cle]` : `KeyError` si elle manque.
pub(crate) fn requis<'a>(d: &'a Value, cle: &str) -> R<&'a Value> {
    match d {
        Value::Object(o) => o.get(cle).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(cle))),
        autre => Err(erreur("TypeError", socle::fichiers::attendu("un objet", socle::valeur::nom_type(autre)))),
    }
}

/// `d.get(cle) or defaut` en texte : la valeur quand elle est vraie.
pub(crate) fn vrai_ou(d: &Value, cle: &str) -> Option<String> {
    d.get(cle).filter(|v| vrai(v)).map(valeur_en_texte)
}

/// `charge[cle]` en texte (`KeyError` si elle manque).
pub(crate) fn charge_texte(charge: &Value, cle: &str) -> R<String> {
    Ok(valeur_en_texte(requis(charge, cle)?))
}

/// Le `qui` du journal : celui que la charge donne (la ligne de commande : `ligne de commande`), sinon « interface ».
pub(crate) fn qui_de(charge: &Value) -> String {
    vrai_ou(charge, "qui").unwrap_or_else(|| "interface".to_string())
}

/// `Preuve(motdepasse=charge.get("motdepasse"))`.
pub(crate) fn preuve_de(charge: &Value) -> Preuve {
    Preuve::de_la_charge(charge)
}

/// La boite qui envoie, selon le branchement de la compagnie (`bridage::branchement`) : le simulateur ecrit les messages
/// sans les envoyer ; le message refuse, avant la preuve et avant toute ligne au journal des depots ; le monde prend la
/// vraie boite, et sans mot de passe d'application au coffre, il n'y en a pas (`BoiteAbsente`).
pub(crate) fn client_courriel(ctx: &Contexte) -> R<Box<dyn Courriel>> {
    let branchement = crate::bridage::branchement(&ctx.racine);
    branchement.ouvrir()?;
    if branchement.joue() {
        return Ok(Box::new(BoiteSimulee::du_contexte(ctx)));
    }
    if !crate::boite::disponible(&ctx.racine) {
        return Err(erreur(
            "BoiteAbsente",
            "Aucune boite courriel : poser le mot de passe d'application de la boite (Configuration > Mots de passe, ou `moteur coffre poser courriel --compte adresse@...`).",
        ));
    }
    let client = crate::boite::client_du_contexte(ctx).map_err(|e| erreur("ErreurCourriel", e.0))?;
    Ok(Box::new(client))
}

/// Les identifiants de la saisie : `expediteur`, la boite qui envoie, en minuscules.
fn expediteur_de(client: &dyn Courriel) -> String {
    client.compte().to_lowercase()
}

// ------------------------------------------------------------------ envoyer-documents

/// `operations_documents.envoyer_documents` : envoie a chaque salarie ses documents du mois. Irreversible.
fn envoyer_documents(ctx: &Contexte, charge: &Value, ouvrir: Ouvrir) -> R<Value> {
    let mois = charge_texte(charge, "mois")?;
    let prepare = preparer_envoi(ctx, &mois, salaries_choisis(charge)?.as_deref(), ouvrir)?;
    expedier_envoi(ctx, prepare, &mois, &preuve_de(charge), &qui_de(charge))
}

/// `envoyer_documents(ctx, mois, preuve=..., qui=...)` : la meme operation, `qui` et la preuve donnes (l'horloge de
/// la compagnie fictive passe `horloge` et une boite muette).
pub(crate) fn envoyer_documents_de(ctx: &Contexte, mois: &str, preuve: &Preuve, qui: &str, ouvrir: Ouvrir) -> R<Value> {
    let prepare = preparer_envoi(ctx, mois, None, ouvrir)?;
    expedier_envoi(ctx, prepare, mois, preuve, qui)
}

/// Les salaries que la charge choisit (`salaries`), ou tous quand elle n'en dit rien.
fn salaries_choisis(charge: &Value) -> R<Option<Vec<String>>> {
    Ok(liste_de(charge, "salaries")?.map(|l| l.iter().map(valeur_en_texte).collect()))
}

/// Les pieces d'un salarie, dans l'ordre ou elles s'impriment : bulletin, bulletin rectificatif, certificat Conges
/// Spectacles, attestation France Travail (celles qui sont ecrites).
const PIECES_A_IMPRIMER: [&str; 4] = ["Bulletin", "BulletinRectificatif", "CertificatCS", "AER"];

/// La route `documents-imprimer` : les documents du mois des salaries choisis (`salaries`, tous sinon), fabriques comme
/// pour l'envoi par courriel (`depart::fabriquer_documents`), reunis en un seul PDF a imprimer, salarie par salarie, et
/// remis en main propre. L'IMPRESSION EST UN DEPART : ce qui est remis au salarie est parti. Les numeros des bulletins et
/// des certificats s'ecrivent dans les paies, le journal note `documents` (`main_propre`) avec la version de chaque
/// engagement, et la paie est figee comme par le courriel. Rien ne quitte le poste : aucun mot de passe.
pub(crate) fn documents_imprimer(ctx: &Contexte, charge: &Value, qui: &str) -> R<Value> {
    let mois = charge_texte(charge, "mois")?;
    let seulement = salaries_choisis(charge)?;
    let (data, paies) = crate::routes_documents::charger_le_mois(ctx, &mois)?;
    if paies.is_empty() {
        return Err(erreur(
            "AucunDocument",
            format!("{} : aucun engagement valide, aucun document a imprimer. Rien ne part d'un engagement en saisie.", engagement::libelle_mois(&mois)),
        ));
    }
    let mois_saisie = data.get("mois").map(socle::date::en_texte).ok_or_else(|| erreur("KeyError", socle::fichiers::absent("mois")))?;
    let mut cles: Vec<String> = Vec::new();
    let mut noms: HashMap<String, String> = HashMap::new();
    for p in &paies {
        let cle = p.salarie.get("_cle").map(valeur_en_texte).unwrap_or_default();
        if cles.contains(&cle) || seulement.as_ref().is_some_and(|s| !s.contains(&cle)) {
            continue;
        }
        let prenom = vrai_ou(&p.salarie, "prenom_affichage").or_else(|| vrai_ou(&p.salarie, "prenom")).unwrap_or_default();
        noms.insert(cle.clone(), format!("{} {}", prenom, vrai_ou(&p.salarie, "nom").unwrap_or_default()).trim().to_string());
        cles.push(cle);
    }
    if cles.is_empty() {
        return Err(refus("Aucun salarié coché. Rien n'est imprimé."));
    }
    let choix = choisir(ctx, &mois, &paies)?;
    let documents = crate::depart::fabriquer_documents(ctx, &mois_saisie, &cles, qui)?;
    let dossier = ctx.racine.join("sorties").join(&mois_saisie).join("envoi");
    let etiq = socle::date::etiquette_du_mois(&mois_saisie).map_err(|m| erreur("ValueError", m))?;
    let mut pieces: Vec<std::path::PathBuf> = Vec::new();
    let mut salaries: Vec<Value> = Vec::new();
    for cle in &cles {
        let siennes: Vec<std::path::PathBuf> =
            PIECES_A_IMPRIMER.iter().map(|prefixe| dossier.join(format!("{}_{}_{}.pdf", prefixe, cle, etiq))).filter(|f| f.is_file()).collect();
        salaries.push(json!({
            "cle": cle, "nom": noms.get(cle), "pieces": siennes.iter().map(|p| nom_du(p)).collect::<Vec<_>>(),
            "resume": choix.resumes.get(cle), "renvoi": choix.renvois.contains_key(cle),
        }));
        pieces.extend(siennes);
    }
    if pieces.is_empty() {
        return Err(refus(format!("Aucun document écrit pour {} : rien à imprimer.", engagement::libelle_mois(&mois))));
    }
    let versions = journal::versions_du_mois(ctx, &mois, |cle, _, _| cles.iter().any(|c| c == cle)).map_err(Echec::from)?;
    let impression = envois::envois_signature::reunir_pour_imprimer(&dossier, "documents", &etiq, &pieces)?;
    let avertissements = documents.avertissements.clone();
    documents.engager(ctx, &mois_saisie)?;
    let detail = json!({"salaries": salaries, "main_propre": true});
    let reponse = format!("documents de {} salarie(s) imprimes pour une remise en main propre", cles.len());
    let ligne = noter_envoi(ctx, "documents", Some(&mois), qui, true, "", &reponse, Some(&detail), Some(&versions))?;
    Ok(json!({"impression": nom_du(&impression), "salaries": cles, "avertissements": avertissements, "ligne": ligne}))
}

/// `operations_documents.EnvoiPrepare` : les messages, le compte verifie -- rien n'est parti.
pub(crate) struct EnvoiPrepare {
    pub(crate) mois_saisie: String,
    pub(crate) messages: Vec<Message>,
    pub(crate) expediteur: String,
    pub(crate) avertissements: Vec<String>,
    /// A qui les documents partent, et pourquoi (`choisir`) : le journal en garde l'empreinte et le resume.
    pub(crate) choix: Choix,
    /// Les documents fabriques pour ceux a qui ils partent ; leurs numeros s.ecrivent quand ils sont partis.
    documents: crate::depart::DocumentsFabriques,
    client: Box<dyn Courriel>,
    /// La duree de la preparation (lecture du mois, messages, boite), en millisecondes : le journal la garde.
    preparation_ms: u128,
}
/// A QUI LES DOCUMENTS DU MOIS PARTENT. Chaque salarie du mois est dans l'un de trois cas, lus dans le journal
/// (`routes_mois::documents_partis` : ce que l'engagement sait de ce qui est parti) et dans ses engagements :
///
/// * ses documents ne sont jamais partis : ils partent, avec le premier courriel ;
/// * ils sont partis, et la paie d'un de ses engagements a change depuis la version sur laquelle ils sont partis
///   (`engagement::etats::a_refaire`, un bulletin rectificatif) : ils repartent refaits, avec le courriel qui dit ce qui
///   a change (`envoi::CORPS_RENVOI`, `envoi::ce_qui_a_change` sur le resume note a l'envoi) ;
/// * ils sont partis et sa paie n'a pas bouge : ils ne repartent pas.
///
/// Quand TOUS sont a jour, l'envoi est un renvoi voulu, a l'identique : tous repartent, avec le premier courriel.
#[derive(Default)]
pub(crate) struct Choix {
    /// Ceux dont la paie a change depuis l'envoi de leurs documents : le jour de l'envoi, ce qui a change.
    pub(crate) renvois: HashMap<String, envoi::Renvoi>,
    /// Ceux dont les documents sont partis et dont la paie n'a pas change.
    pub(crate) a_jour: std::collections::HashSet<String>,
    /// Le resume de la paie de chaque salarie du mois, a noter au journal avec l'envoi.
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

/// Les salaries de `mois` dont un engagement a un bulletin a refaire : la paie de ce mois a change depuis la version sur
/// laquelle ses documents sont partis (`engagement::etats::a_refaire`).
pub(crate) fn bulletins_a_refaire(ctx: &Contexte, mois: &str) -> R<std::collections::HashSet<String>> {
    let departs = envois::journal::departs(&ctx.racine)?;
    let mut out = std::collections::HashSet::new();
    for e in engagement::etats::du_mois(ctx, mois)? {
        let Some(v) = e.valide() else { continue };
        let refaire = engagement::etats::a_refaire(ctx, v, &departs)
            .into_iter()
            .any(|a| matches!(a, engagement::etats::ARefaire::Bulletin { mois: ref m, .. } if m == mois));
        if refaire {
            if let Some(c) = engagement::cle_de_l_engagement(ctx, &Value::Object(v.objet().brut.clone())) {
                out.insert(c);
            }
        }
    }
    Ok(out)
}

/// Le choix, pour le mois `mois`, sur ses paies validees (`routes_documents::charger_le_mois`).
pub(crate) fn choisir(ctx: &Contexte, mois: &str, paies: &[engagement::calcul::Paie]) -> R<Choix> {
    let lignes = envois::etats::charger(&ctx.racine)?;
    let partis = crate::routes_mois::documents_partis(ctx, mois, &engagement::etats::du_mois(ctx, mois).map_err(Echec::from)?, &lignes)?;
    let cles: Vec<String> = paies.iter().map(|p| p.salarie.get("_cle").map(valeur_en_texte).unwrap_or_default()).collect();
    let mut resumes: HashMap<String, Value> = HashMap::new();
    for cle in &cles {
        if resumes.contains_key(cle) {
            continue;
        }
        let siennes: Vec<&engagement::calcul::Paie> = paies.iter().zip(&cles).filter(|(_, c)| *c == cle).map(|(p, _)| p).collect();
        resumes.insert(cle.clone(), envoi::resume_de(&siennes).map_err(Echec::from)?);
    }
    let changes = bulletins_a_refaire(ctx, mois)?;
    let mut choix = Choix { resumes, ..Choix::default() };
    let declaration = declaration_de_la_correction(ctx, mois)?;
    for (cle, parti) in &partis {
        if !changes.contains(cle) {
            choix.a_jour.insert(cle.clone());
            continue;
        }
        let quand = parti.get("quand").map(valeur_en_texte).unwrap_or_default();
        let envoye_le = jour_iso(quand.get(..10).unwrap_or("")).map(|j| j.jj_mm_aaaa()).unwrap_or(quand);
        let ancien = parti.get("resume").cloned().unwrap_or(Value::Null);
        let changements = envoi::ce_qui_a_change(&ancien, choix.resumes.get(cle).unwrap_or(&Value::Null));
        let declaration = declaration.as_ref().map(|d| d(cle)).unwrap_or_default();
        choix.renvois.insert(cle.clone(), envoi::Renvoi { envoye_le, changements, declaration });
    }
    Ok(choix)
}


/// LA CORRECTION D'UN MOIS DONT L'ANNULE-ET-REMPLACE EST PASSE : le courriel de renvoi ajoute une phrase, la correction
/// sera declaree avec la paie d'un mois suivant. La DSN deposee ne se refait plus ; la correction part dans la DSN qui la
/// porte (`routes_regularisation::corrections_du_mois` : celle d'un mois suivant deja partie, ou la DSN du mois
/// courant). `None` : le mois n'en est pas la (pas de DSN deposee, ou son annule-et-remplace encore admis).
#[allow(clippy::type_complexity)]
fn declaration_de_la_correction(ctx: &Contexte, mois: &str) -> R<Option<Box<dyn Fn(&str) -> String>>> {
    let mut m = crate::routes_mois::Moteur::nouveau(ctx);
    let etat = crate::routes_regularisation::etat_mois(&mut m, ctx, &json!(mois))?;
    if !vrai(&etat["regularisable"]) {
        return Ok(None);
    }
    let courant = valeur_en_texte(&etat["mois_courant"]);
    let (a_porter, portees) = crate::routes_regularisation::corrections_du_mois(ctx, mois)?;
    let mut porteurs: HashMap<String, String> = HashMap::new();
    for c in &portees {
        porteurs.insert(valeur_en_texte(&c["salarie"]), valeur_en_texte(&c["portee_par"]));
    }
    for c in &a_porter {
        porteurs.insert(valeur_en_texte(&c["salarie"]), courant.clone());
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
pub(crate) fn preparer_envoi(ctx: &Contexte, mois: &str, seulement: Option<&[String]>, ouvrir: Ouvrir) -> R<EnvoiPrepare> {
    let debut = std::time::Instant::now();
    let (data, paies) = crate::routes_documents::charger_le_mois(ctx, mois)?;
    if paies.is_empty() {
        return Err(erreur(
            "AucunDocument",
            format!("{} : aucun engagement valide, aucun document a envoyer. Rien ne part d'un engagement en saisie.", engagement::libelle_mois(mois)),
        ));
    }
    let mois_saisie = data.get("mois").map(socle::date::en_texte).ok_or_else(|| erreur("KeyError", socle::fichiers::absent("mois")))?;
    let dossier = ctx.racine.join("sorties").join(&mois_saisie).join("envoi");
    let fiche = |cle: &str| ctx.salarie(cle).unwrap_or_else(|_| json!({}));
    let choix = choisir(ctx, mois, &paies)?;
    // LES DOCUMENTS SE FABRIQUENT ICI, pour ceux a qui ils partent : depuis les paies ecrites, a leurs numeros.
    let mut cles: Vec<String> = Vec::new();
    for p in &paies {
        let cle = p.salarie.get("_cle").map(valeur_en_texte).unwrap_or_default();
        if !cles.contains(&cle) {
            cles.push(cle);
        }
    }
    let cles: Vec<String> = choix.garder(cles, |c| c.as_str()).into_iter().filter(|c| seulement.is_none_or(|s| s.contains(c))).collect();
    if cles.is_empty() {
        return Err(refus("Aucun salarié coché. Rien n'est parti."));
    }
    let documents = crate::depart::fabriquer_documents(ctx, &mois_saisie, &cles, "interface")?;
    let messages = envoi::preparer_avec(&paies, &ctx.entreprise, &mois_saisie, &dossier, Some(&fiche), &choix.renvois).map_err(|m| refus_de_preparation(&m))?;
    let messages: Vec<Message> = choix.garder(messages, |m| m.cle.as_str()).into_iter().filter(|m| cles.contains(&m.cle)).collect();
    let client = ouvrir()?;
    let expediteur = expediteur_de(client.as_ref());
    let mut avertissements: Vec<String> = documents.avertissements.clone();
    let signataire = requis(&ctx.entreprise, "signataire")?.get("email").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default().to_lowercase();
    if !signataire.is_empty() && signataire != expediteur {
        avertissements.push(format!(
            "Les messages partiront de {} (la boite posee), pas de {} (le courriel du signataire, fiche Entreprise).",
            expediteur, signataire
        ));
    }
    Ok(EnvoiPrepare { mois_saisie, messages, expediteur, avertissements, choix, documents, client, preparation_ms: socle::temps::duree(debut).as_millis() })
}

/// Ce qui suit la preparation : la PREUVE (le mot de passe du logiciel), l'envoi, le journal. `qui` : la ligne du journal.
pub(crate) fn expedier_envoi(ctx: &Contexte, prepare: EnvoiPrepare, mois: &str, preuve: &Preuve, qui: &str) -> R<Value> {
    let EnvoiPrepare { mois_saisie, messages, expediteur, avertissements, choix, documents, mut client, preparation_ms } = prepare;
    let a_qui = format!("{} salarie(s) : {}", messages.len(), messages.iter().map(|m| m.nom.as_str()).collect::<Vec<_>>().join(", "));
    // LA VERSION SUR LAQUELLE ILS PARTENT, engagement par engagement des salaries servis.
    let servis: Vec<&str> = messages.iter().map(|m| m.cle.as_str()).collect();
    let versions = journal::versions_du_mois(ctx, mois, |cle, _, _| servis.contains(&cle)).map_err(Echec::from)?;
    // LA DUREE DE CHAQUE ETAPE (08/10/2026 : un envoi de deux courriels a pris quatre minutes, sans qu'on sache ou).
    let t_preuve = std::time::Instant::now();
    verifier_preuve(ctx, Some(preuve), mois, "envoi des documents du mois aux salaries", &a_qui)?;
    let preuve_ms = socle::temps::duree(t_preuve).as_millis();
    let t_envoi = std::time::Instant::now();
    let envoye = envoi::envoyer_suivi(&messages, &expediteur, client.as_mut(), None, None);
    let envoi_ms = socle::temps::duree(t_envoi).as_millis();
    // Ce que chaque salarie servi a recu : ses pieces, le resume de sa paie (ce qui dira, si elle change, ce qui a change),
    // et si c'etait un renvoi.
    let salaries = |servis: &[Message]| -> Value {
        json!(servis.iter().map(|m| json!({
            "cle": m.cle, "nom": m.nom, "pieces": m.pieces.iter().map(|p| nom_du(p)).collect::<Vec<_>>(),
            "resume": choix.resumes.get(&m.cle),
            "renvoi": choix.renvois.contains_key(&m.cle),
        })).collect::<Vec<_>>())
    };
    let durees = json!({"preparation": preparation_ms as u64, "mot_de_passe": preuve_ms as u64, "envoi": envoi_ms as u64});
    let partis = match envoye {
        Ok(p) => p,
        Err(e) => {
            // CE QUI A ATTEINT LE SALARIE EST PARTI : les messages partent dans l'ordre, les premiers sont servis. Leurs
            // numeros, et seulement les leurs, s'ecrivent d'abord ; la ligne porte ensuite la version de leurs
            // engagements, et seulement les leurs.
            let servis = &messages[..e.partis.len().min(messages.len())];
            let mut versions_servies = Value::Array(Vec::new());
            if !servis.is_empty() {
                let cles: Vec<&str> = servis.iter().map(|m| m.cle.as_str()).collect();
                documents.engager_servis(ctx, &mois_saisie, &cles)?;
                versions_servies = json!(versions.as_array().into_iter().flatten().filter(|v| v["salarie"].as_str().is_some_and(|c| cles.contains(&c))).collect::<Vec<_>>());
            }
            let detail = json!({"salaries": salaries(servis), "expediteur": expediteur, "durees_ms": durees});
            noter_echec_d_envoi(ctx, "documents", Some(mois), qui, &e.partis, &e.echoue, &e.message, Some(detail), Some(&versions_servies))?;
            return Err(echec_d_envoi(e.message));
        }
    };
    // LES DOCUMENTS SONT PARTIS : leurs numeros s'ecrivent dans les paies des engagements, puis la ligne du journal dit
    // qu'ils sont partis, et sur quelle version.
    documents.engager(ctx, &mois_saisie)?;
    let detail = json!({"salaries": salaries(&messages), "expediteur": expediteur, "durees_ms": durees});
    let reponse = format!("{} message(s) envoye(s)", partis.len());
    let ligne = noter_envoi(ctx, "documents", Some(mois), qui, true, &partis.join(", "), &reponse, Some(&detail), Some(&versions))?;
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
        let s = ctx.salaries.get(cle).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(cle)))?;
        let Some(email) = s.get("email").filter(|v| vrai(v)).map(valeur_en_texte) else {
            return Err(refus(format!(
                "Pas d'adresse e-mail pour {} {} : compléter sa fiche avant d'envoyer. Rien n'est parti.",
                valeur_en_texte(requis(s, "nom")?),
                valeur_en_texte(requis(s, "prenom")?)
            )));
        };
        let prenom = match vrai_ou(s, "prenom_affichage") {
            Some(p) => p,
            None => valeur_en_texte(requis(s, "prenom")?),
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
                ("signataire", vrai_ou(&signataire, "prenom_affichage").or_else(|| signataire.get("prenom").map(valeur_en_texte)).unwrap_or_default()),
                ("raison_sociale", vrai_ou(&ctx.entreprise, "nom_affichage").or_else(|| ctx.entreprise.get("raison_sociale").map(valeur_en_texte)).unwrap_or_default()),
            ],
        );
        messages.push(Message {
            cle: cle.clone(),
            nom: format!("{} {}", prenom, valeur_en_texte(requis(s, "nom")?)),
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
            noter_echec_d_envoi(ctx, "dossier-salarie", None, qui, &e.partis, &e.echoue, &e.message, None, None)?;
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
    use declarations::casc_regularisation::{ecrire, marquer_regularise, regularisation, Demande};
    // `charge.get("vire_le") or {}`.
    let vire_le: Vec<(String, Value)> = match charge.get("vire_le").filter(|v| vrai(v)) {
        None => Vec::new(),
        Some(Value::Object(o)) => o.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        Some(autre) => return Err(erreur("AttributeError", socle::fichiers::attendu("un objet", socle::valeur::nom_type(autre)))),
    };
    let aujourdhui = crate::routes_dossier_salarie::aujourdhui_reel();
    let mut dates: Vec<(String, String)> = Vec::new();
    for (a, d) in &vire_le {
        let texte = valeur_en_texte(d);
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
    let dernier_mois = r.mois.last().map(|m| m.mois.clone()).ok_or_else(|| erreur("IndexError", socle::fichiers::trop_courte("les mois de la régularisation")))?;
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
            noter_echec_d_envoi(ctx, "regularisation-casc", None, &qui, &e.partis, &e.echoue, &e.message, Some(json!({"annees": annees_voulues})), None)?;
            return Err(echec_d_envoi(e.message));
        }
    };

    // Ce que l'envoi garde : les deux messages tels qu'ils sont partis.
    let garde = std::path::Path::new(&r.dossier).join("envoyes");
    std::fs::create_dir_all(&garde).map_err(|e| erreur("OSError", format!("{} : {}", garde.display(), e)))?;
    let iso = socle::paris::maintenant();
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
    let auj = socle::paris::maintenant();
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
    valeur_en_texte(d)
}
