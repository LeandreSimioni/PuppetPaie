//! Deposer une DSN ou une DPAE (`moteur/operations_declarations.py` : `deposer_dsn`, `deposer_dpae`,
//! `_verifier_preuve`, `_exiger_repetition`, `_motdepasse_organisme`, `prochain_ordre_de_test`...) -- lot H2.
//!
//! DEPOSER EST L'UN DES RARES GESTES DU LOGICIEL QUI ENGAGENT L'EMPLOYEUR -- jusqu'a des ordres de
//! telepaiement reellement preleves. L'ordre est donc toujours le meme, et c'est le moteur qui le tient (jamais
//! la page) : lire le fichier ; verifier que le mode demande colle a ce qu'il code (un fichier `02` depose
//! << pour voir >> EST une declaration) ; exiger la preuve (le mot de passe du logiciel, retape au moment de
//! l'acte) ; exiger la REPETITION (ce contenu exact est deja passe en test, ou un motif ECRIT) ; seulement
//! alors chercher le secret de l'organisme ; deposer ; NOTER, quoi qu'il arrive -- succes comme echec.
//!
//! Le secret de l'organisme ne sort pas d'ici que pour le client (`releve_retours::Client`) : il n'est ni
//! journalise, ni affiche, ni renvoye. Un depot de test ne part jamais avec la nature `02` ni l'indicateur
//! `120` : la bascule se fait en memoire (`depot::basculer_*`), jamais sur le disque.
//!
//! La COMPAGNIE FICTIVE (`simulation.active`) joue le depot -- un flux `FICTIF-...` -- et rien ne quitte le poste ;
//! le reste (repetition, journal) se deroule comme en vrai.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use droit::contexte::Contexte;
use crate::depot::{self, Apercu};
use crate::etats;
use socle::valeur::valeur_en_texte;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::vrai;
use crate::releve_retours::{iso_de_naif, maintenant_naif, Client, Panne};
use socle::echec::Echec;

/// Une erreur qui porte une donnee de plus pour la page (`etat`, `manques`...) : sa phrase et la donnee
/// voyagent ensemble dans le message, que `routes.rs` redresse en `{"erreur", "classe", <cle>}`.
pub(crate) fn avec_donnee(classe: &'static str, message: impl Into<String>, cle: &str, donnee: Value) -> Echec {
    Echec { classe: Some(classe), message: json!({"message": message.into(), cle: donnee}).to_string() }
}

/// Le texte d'un refus qui porte une donnee (`avec_donnee`) : sa phrase seule.
pub(crate) fn phrase_de(e: &Echec) -> String {
    serde_json::from_str::<Value>(&e.message).ok().and_then(|d| d.get("message").and_then(Value::as_str).map(str::to_string)).unwrap_or_else(|| e.message.clone())
}

/// `Preuve` : ce qui prouve qu'un humain veut CET acte, MAINTENANT. Le mot de passe DU LOGICIEL, retape.
#[derive(Default, Clone)]
pub struct Preuve {
    pub motdepasse: Option<String>,
    pub mois_recopie: Option<String>,
}

impl Preuve {
    pub fn de_la_charge(charge: &Value) -> Preuve {
        Preuve { motdepasse: charge.get("motdepasse").filter(|m| vrai(m)).map(valeur_en_texte), mois_recopie: None }
    }
}

/// Ce qui vient de partir, et ce que l'organisme a repondu.
pub struct DepotFait {
    pub quoi: String,
    pub mois: String,
    pub reel: bool,
    pub reponse: String,
    pub flux: String,
    pub trace: Option<PathBuf>,
    pub ligne: Value,
    /// Le depot PRECEDENT du meme fichier, s'il y en a eu un (`Null` sinon).
    pub deja: Value,
    /// La repetition qui a ouvert ce depot reel (`Null` pour un test : il EST la repetition).
    pub repetition: Value,
}

impl DepotFait {
    /// La reponse de la route `deposer`.
    pub fn en_json(&self) -> Value {
        json!({
            "quoi": self.quoi, "mois": self.mois, "reel": self.reel, "reponse": self.reponse, "flux": self.flux,
            "trace": self.trace.as_ref().map(|t| t.to_string_lossy().into_owned()),
            "ligne": self.ligne, "deja": self.deja, "repetition": self.repetition,
        })
    }
}

// ------------------------------------------------------------------ l'horloge

fn maintenant_iso() -> String {
    iso_de_naif(maintenant_naif())
}

// -------------------------------------------------------------------- la preuve

/// `maj.plus_recente` : `_cle` lit une version en entiers separes par des points (illisible : `(0,)`).
fn cle_de_version(v: &str) -> Vec<i64> {
    let lu: Option<Vec<i64>> = v.trim().split('.').map(|x| x.trim().parse::<i64>().ok()).collect();
    lu.unwrap_or_else(|| vec![0])
}

/// `maj.exigee_en_attente` : la raison pour laquelle une version telechargee et pas encore appliquee est
/// indispensable avant de declarer -- ou `""`.
/// La version prete est celle de l'INSTALLATION du contexte (`<installation>/logiciel/donnees` = `ctx.code`, lot U1).
pub(crate) fn exigee_en_attente(ctx: &Contexte) -> R<String> {
    let code = ctx.code.parent().map(Path::to_path_buf).unwrap_or_default();
    let Some(p) = crate::routes_etats::prete(&code.join(".."))? else { return Ok(String::new()) };
    let exigee = p.get("exigee").filter(|e| vrai(e)).map(valeur_en_texte).unwrap_or_default();
    let version = p.get("version").map(valeur_en_texte).unwrap_or_default();
    if exigee.is_empty() || cle_de_version(&version) <= cle_de_version(&crate::routes_etats::courante(&code)?) {
        return Ok(String::new());
    }
    Ok(format!(
        "la version {} est indispensable avant de declarer ({}) : fermez et rouvrez PuppetPaie, elle s'appliquera au demarrage",
        version, exigee
    ))
}

/// `operations_socle._verifier_preuve` : la forme de preuve retenue, ou un refus. Aucun acte sans elle.
/// LA REGLE : LE MOT DE PASSE DU LOGICIEL, toujours -- le mois recopie seul n'a jamais prouve qu'un humain
/// etait la. Un booleen « confirme=True » venu du front ne prouve rien du tout.
///
/// DEUX FACONS DE LE PROUVER, et aucune autre (lot J1) : le mot de passe que la fenetre ou la page a fait taper et qui
/// arrive dans la charge (`Preuve`) ; sinon LE MOTEUR LE DEMANDE LUI-MEME (`demande::autoriser_acte` : fenetre, terminal,
/// ou refus). Ni la seance (`garder_ouverture`), ni `PUPPETPAIE_MOTDEPASSE`, ni le mot de passe retenu pour ouvrir le
/// coffre ne prouvent quoi que ce soit : ils ouvrent, ils n'autorisent pas. `quoi` et `a_qui` disent a l'humain ce qui
/// va partir ; `mois_attendu` est le mois que la preuve recopiee devrait porter.
pub(crate) fn verifier_preuve(ctx: &Contexte, preuve: Option<&Preuve>, mois_attendu: &str, quoi: &str, a_qui: &str) -> R<String> {
    // Le binaire de l'ATELIER refuse tout acte reel, avant toute autre question (`atelier.rs`). Un depot reel, un envoi,
    // une signature passent tous par ici.
    #[cfg(feature = "atelier")]
    return refus_atelier(ctx, preuve, mois_attendu, quoi, a_qui);
    #[cfg(not(feature = "atelier"))]
    verifier_preuve_du_logiciel(ctx, preuve, mois_attendu, quoi, a_qui)
}

#[cfg(feature = "atelier")]
fn refus_atelier(ctx: &Contexte, _preuve: Option<&Preuve>, _mois_attendu: &str, _quoi: &str, _a_qui: &str) -> R<String> {
    // UNE COMPAGNIE FICTIVE AUX ENVOIS SIMULES : son « acte reel » est joue (`FICTIF-…`), rien ne part -- comme dans le
    // binaire livre (`verifier_preuve_du_logiciel`). Le banc d'essai (#236) en vit : l'horloge y depose la DSN et la DPAE
    // « en reel ». Toute autre compagnie : refuse, mot de passe ou non (la commande `signatures` fait de meme).
    if crate::routes_accueil::simulation_active(&ctx.racine) {
        return Ok("fictive".into());
    }
    Err(erreur("ActeReelRefuse", crate::atelier::REFUS_ACTE_REEL.to_string()))
}

#[cfg_attr(feature = "atelier", allow(dead_code))]
fn verifier_preuve_du_logiciel(ctx: &Contexte, preuve: Option<&Preuve>, mois_attendu: &str, quoi: &str, a_qui: &str) -> R<String> {
    // LA COMPAGNIE FICTIVE : rien ne part, il n'y a rien a prouver.
    if crate::routes_accueil::simulation_active(&ctx.racine) {
        return Ok("fictive".into());
    }
    // Une version EXIGEE attend d'etre appliquee : l'ancienne ferait une declaration fausse.
    let presse = exigee_en_attente(ctx)?;
    if !presse.is_empty() {
        return Err(erreur("VersionExigee", format!("Rien n'est parti : {}.", presse)));
    }
    let coffre = crate::coffre::Coffre::du_poste(&ctx.racine, "");
    let vers = crate::routes_etats::vers_route;
    if !coffre.ouverture_exigee().map_err(vers)? {
        return Err(erreur(
            "PreuveInsuffisante",
            "Aucun mot de passe du logiciel n'est pose : rien ne peut partir en reel. Le poser d'abord -- la fenetre PuppetPaie le demande a sa premiere ouverture, ou `paie coffre ouverture`. Rien n'est parti.",
        ));
    }
    let Some(preuve) = preuve.filter(|p| p.motdepasse.as_deref().is_some_and(|m| !m.is_empty())) else {
        // Rien n'a ete tape pour cette charge : c'est le moteur qui demande, a un humain, ou il refuse.
        let acte = crate::demande::Acte::nouveau(quoi, mois_attendu, a_qui);
        return match crate::demande::autoriser_acte(&ctx.racine, &acte) {
            crate::demande::Verdict::Accepte => Ok("mot de passe du logiciel".into()),
            crate::demande::Verdict::Refuse(phrase) => Err(erreur("PreuveInsuffisante", phrase)),
        };
    };
    if let Some(m) = preuve.mois_recopie.as_deref().filter(|m| !m.is_empty()) {
        if m.trim() != mois_attendu {
            return Err(erreur("PreuveInsuffisante", format!("Mois confirme '{}' alors que le fichier declare '{}'. Rien n'est parti.", m.trim(), mois_attendu)));
        }
    }
    if !coffre.verifier_ouverture(preuve.motdepasse.as_deref().unwrap_or("")).map_err(vers)? {
        return Err(erreur("PreuveInsuffisante", "Mot de passe du logiciel refuse. Rien n'est parti."));
    }
    Ok("mot de passe du logiciel".into())
}

// ------------------------------------------------------------ le secret de l'organisme

/// `_motdepasse_organisme` : le secret de l'organisme -- l'environnement d'abord (quelqu'un l'a tape dans une
/// fenetre, il vaut confirmation), puis le coffre. Ce qui autorise a y puiser pour un depot REEL, c'est la
/// preuve verifiee juste avant par l'appelant. Ce qui SORT d'ici est un secret : il va directement au client.
pub(crate) fn motdepasse_organisme(ctx: &Contexte, service: &str, ouverture: Option<&str>) -> R<Option<String>> {
    let variable = if service == "net-entreprises" { crate::releve_retours::VARIABLE_MOTDEPASSE } else { crate::releve_retours::VARIABLE_MOTDEPASSE_DPAE };
    if let Ok(v) = std::env::var(variable) {
        if !v.is_empty() {
            return Ok(Some(v));
        }
    }
    let coffre = crate::coffre::Coffre::du_poste(&ctx.racine, "");
    let vers = crate::routes_etats::vers_route;
    if !coffre.pose(service).map_err(vers)? {
        return Err(erreur(
            "ErreurOperation",
            format!("Mot de passe {} absent : ni dans l'environnement, ni au coffre.\n  paie coffre poser {}", service, service),
        ));
    }
    coffre.lire(&json!(service), &json!(crate::coffre::DEPOT), true, ouverture).map_err(vers)
}

// -------------------------------------------------------- la repetition avant l'irreversible

/// `_pourquoi_rate` : ce qui a fait echouer un depot de test, en une phrase.
pub(crate) fn pourquoi_rate(ligne: &Value) -> String {
    let mut anomalies: Vec<String> = Vec::new();
    for r in ligne.get("retours").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]) {
        if r.get("verdict").and_then(Value::as_str) == Some("KO") {
            for a in r.get("anomalies").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]) {
                let t = valeur_en_texte(a);
                if !anomalies.contains(&t) {
                    anomalies.push(t);
                }
            }
        }
    }
    if !anomalies.is_empty() {
        return anomalies.join(" ; ");
    }
    let rep: String = ligne.get("reponse").filter(|r| vrai(r)).map(valeur_en_texte).unwrap_or_default().trim().chars().take(200).collect();
    if rep.is_empty() {
        "motif non lu".into()
    } else {
        rep
    }
}

fn quand_a(v: &Value) -> String {
    etats::quand_lisible(v).replace(" à ", " a ")
}

/// `_phrase_repetition` : le refus, ecrit pour qu'on sache quoi faire dans la minute. Un test qui a RATE se
/// corrige, un test PERIME se refait, une absence de test se fait.
fn phrase_repetition(ctx: &Contexte, quoi: &str, chemin: &Path, etat: &Value) -> String {
    let acte = if quoi == "dsn" { "DSN" } else { "DPAE" };
    // LE CHEMIN DOIT SE RECOPIER TEL QUEL : « sorties/2026-08/... » marche dans les deux cas.
    let ou = chemin.strip_prefix(&ctx.racine).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| chemin.to_string_lossy().into_owned());
    let verbe = if quoi == "dsn" { "depot-dsn" } else { "deposer-dpae" };
    let commande = format!("paie {} {} --test", verbe, ou);
    let dernier = |cle: &str| etat.get(cle).and_then(Value::as_array).and_then(|a| a.last()).cloned();
    let situation = if vrai(&etat["conforme"]) {
        format!(
            "Ce contenu est parti en test le {}, et son compte rendu de conformite n'est pas encore arrive :\nl'accuse d'enregistrement dit que l'envoi est recu, pas qu'il est conforme. Attendre le bilan\n(en general dans les dix minutes), relever les retours, puis envoyer le reel.",
            quand_a(&etat["conforme"]["quand"])
        )
    } else if let Some(l) = dernier("ratees") {
        format!(
            "Ce contenu a ete essaye en test le {}, et l'essai a ECHOUE : {}.\nC'est cela qu'il faut corriger : le meme refus attend en reel.",
            quand_a(&l["quand"]),
            pourquoi_rate(&l)
        )
    } else if let Some(l) = dernier("perimees") {
        format!(
            "Un fichier du meme nom a bien ete essaye en test le {}, mais son\ncontenu a CHANGE depuis ({} teste, {} aujourd'hui) : un octet qui\nbouge rend le test caduc.",
            quand_a(&l["quand"]),
            l.get("empreinte").filter(|e| vrai(e)).map(valeur_en_texte).unwrap_or_else(|| "?".into()),
            valeur_en_texte(&etat["empreinte"])
        )
    } else {
        "Ce contenu n'a jamais ete depose en test.".to_string()
    };
    format!(
        "Depot {} REEL refuse : aucune repetition reussie de ce contenu-la. Rien n'est parti.\n{}\n\nFaire la repetition avec CE fichier-ci -- la bascule en envoi de test se fait\nen memoire, le fichier sur le disque n'est pas modifie :\n  {}\n\nSi le service de test est indisponible et que l'echeance ne peut pas attendre,\nle dire en toutes lettres -- le motif est ecrit au journal des depots :\n  ... --reel --sans-repetition \"ce qui empechait le test\"",
        acte, situation, commande
    )
}

/// `_exiger_repetition` : leve tant que ce contenu n'a pas ete repete ; rend ce qu'il faut noter. LA SORTIE
/// DE SECOURS EST VOULUE, et elle est tracee : un motif ECRIT (dix caracteres au moins) passe outre ; ce
/// qu'on refuse, c'est le contournement SILENCIEUX.
fn exiger_repetition(ctx: &Contexte, lignes: &[Value], quoi: &str, chemin: &Path, contournement: &str) -> R<Value> {
    let octets = std::fs::read(chemin).map_err(|e| erreur("OSError", format!("{} : {}", chemin.display(), e)))?;
    let nom = chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let etat = etats::repetition(lignes, quoi, &crate::journal::empreinte(&octets), &nom)?;
    // UN REEL NE PART QUE SUR LE BILAN LU DE SON TEST (tickets #221, #222, 19/10/2026) : l'accuse d'enregistrement dit
    // « envoi recu », pas « conforme » (regle 5). Pour une DSN, c'est le bilan de nature 11 du test, qui arrive « en
    // general dans les dix minutes » (la reponse du point de depot). Pour une DPAE, le bilan 11 ou le certificat 41 :
    // celui d'un test revient bel et bien (journal du depot prive, archives 2026-10-02, « a la lumiere du certificat 41
    // du test (09:13) : les deux DPAE [...] sont revenues enregistrees (code 99, deux dossiers) » ; archives
    // 2026-09-09_au_09-13, §20.2 : « Passee = bilan OK ; le certificat 41 confirme »).
    if vrai(&etat["conforme"]) && etats::est(&etat["compte_rendu"], "ok") {
        let l = &etat["conforme"];
        return Ok(json!({
            "test": l.get("quand").cloned().unwrap_or(Value::Null),
            "flux": if vrai(&l["flux"]) { l["flux"].clone() } else { json!("") },
            "empreinte": etat["empreinte"], "compte_rendu": etat["compte_rendu"],
        }));
    }
    let raison = contournement.trim();
    if !raison.is_empty() {
        if raison.chars().count() < 10 {
            return Err(erreur(
                "ErreurOperation",
                "Deposer sans repetition demande un motif ECRIT, pas un mot : ce qui empechait le test, en une phrase. C'est ce qu'on relira le jour ou ce depot posera question. Rien n'est parti.",
            ));
        }
        return Ok(json!({"contournement": raison, "empreinte": etat["empreinte"]}));
    }
    Err(avec_donnee("RepetitionManquante", phrase_repetition(ctx, quoi, chemin, &etat), "etat", etat))
}

// -------------------------------------------------------------- les incidents et la trace

/// `_noter_incident_depot` : un depot refuse s'ecrit au journal des incidents, test comme reel. LE CAS URGENT
/// SE DISTINGUE ICI : un refus en REEL alors que la repetition etait conforme.
fn noter_incident_depot(ctx: &Contexte, quoi: &str, reel: bool, classe: &str, message: &str, repetition: &Value) {
    let apres_test = reel && repetition.get("test").is_some_and(vrai);
    let _ = crate::incidents::noter(
        &ctx.racine,
        &crate::incidents::Note {
            nature: if quoi == "dsn" { crate::incidents::DSN } else { crate::incidents::DPAE },
            ou: &format!("depot-{}", quoi),
            quoi: classe,
            resume: message,
            detail: json!({"reel": reel, "apres_test_conforme": apres_test, "repetition": if vrai(repetition) { repetition.clone() } else { json!({}) }}),
            ..Default::default()
        },
    );
}

/// `_archiver_reponse` : garde la reponse de l'organisme a cote du journal, en clair, datee. Le nom doit
/// tenir sur tous les systemes (un `?` est impossible sous Windows).
fn archiver_reponse(ctx: &Contexte, quoi: &str, mois: &str, reponse: &str) -> R<PathBuf> {
    let dossier = ctx.racine.join("retours").join("depots");
    std::fs::create_dir_all(&dossier).map_err(|e| erreur("OSError", format!("{} : {}", dossier.display(), e)))?;
    let etiquette: String = mois.replace('-', "").chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')).collect();
    let etiquette = if etiquette.is_empty() { "sans-date".to_string() } else { etiquette };
    let iso = maintenant_iso();
    let horo = format!("{}{}{}-{}{}{}", &iso[0..4], &iso[5..7], &iso[8..10], &iso[11..13], &iso[14..16], &iso[17..19]);
    // JAMAIS PAR-DESSUS UN AUTRE (lot I1 : un depot de test puis le reel, joues a la suite par l'horloge de la compagnie fictive dans
    // la meme seconde, portaient le meme nom, et la reponse du test disparaissait ; l'ancien Python aussi). Le nom ordinaire ne
    // change pas ; en cas de collision le fichier prend `-2`, `-3`... avant `.txt`.
    let mut rang = 1;
    loop {
        let nom = if rang == 1 { format!("{}_{}_{}.txt", horo, quoi, etiquette) } else { format!("{}_{}_{}-{}.txt", horo, quoi, etiquette, rang) };
        let p = dossier.join(nom);
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&p) {
            Ok(mut f) => {
                use std::io::Write;
                f.write_all(reponse.as_bytes()).map_err(|e| erreur("OSError", format!("{} : {}", p.display(), e)))?;
                return Ok(p);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => rang += 1,
            Err(e) => return Err(erreur("OSError", format!("{} : {}", p.display(), e))),
        }
    }
}

/// `journal.noter` : ajoute la ligne et la rend.
#[allow(clippy::too_many_arguments)]
fn noter(
    ctx: &Contexte,
    quoi: &str,
    mois: &str,
    fichier: &Path,
    reel: bool,
    qui: &str,
    accepte: bool,
    destinataire: &str,
    flux: &str,
    reponse: &str,
    trace: Option<&Path>,
    detail: &Value,
    versions: Option<&Value>,
) -> R<Value> {
    let fichier = fichier.to_string_lossy().into_owned();
    // LA TRACE, RELATIVE A LA COMPAGNIE (#236 : une compagnie change de dossier -- gardee de `fictives/` a `compagnies/`
    // --, et un chemin absolu ne mene plus nulle part) ; `journal::trace_de` la relit, et lit tel quel un absolu d'avant.
    let trace = trace.map(|t| t.strip_prefix(&ctx.racine).unwrap_or(t).to_string_lossy().replace('\\', "/"));
    let n = crate::journal::Note {
        quoi,
        mois: Some(mois),
        fichier: Some(&fichier),
        mode: if reel { "reel" } else { "test" },
        qui,
        accepte,
        destinataire,
        flux,
        reponse,
        trace: trace.as_deref(),
        detail: Some(detail),
        versions,
        quand: "",
        origine: "",
    };
    crate::journal::noter(&ctx.racine, &n, &maintenant_iso()).map_err(|m| erreur("OSError", m))
}

// ----------------------------------------------------------------- la compagnie fictive

/// Le flux d'un depot joue par une compagnie fictive : `FICTIF-` et douze chiffres hexadecimaux. Les depots joues avant le
/// lot U1 portent `DEMO-` (`ANCIEN_PREFIXE_FLUX_FICTIF`) : leurs retours se jouent encore (`releve_retours`).
pub(crate) const PREFIXE_FLUX_FICTIF: &str = "FICTIF-";
pub(crate) const ANCIEN_PREFIXE_FLUX_FICTIF: &str = "DEMO-";

/// `simulation.reponse_depot` : ce que le point de depot aurait repondu -- un flux, et la mention.
fn reponse_fictive(quoi: &str, reel: bool) -> R<String> {
    let mut o = [0u8; 6];
    socle::temps::aleas(&mut o).map_err(|e| erreur("OSError", format!("pas d'alea du systeme : {}", e)))?;
    let flux = format!("{}{}", PREFIXE_FLUX_FICTIF, o.iter().map(|b| format!("{:02X}", b)).collect::<String>());
    Ok(format!(
        "<idflux>{}</idflux>\nCompagnie fictive : {} {} jouee, rien n'a quitte le poste.",
        flux,
        if quoi == "dsn" { "DSN" } else { "DPAE" },
        if reel { "reelle" } else { "de test" }
    ))
}

/// LA PORTE DES ORGANISMES POUR UNE FICTIVE AJOUTEE (ticket #236, `bridage::refus_d_envoi`) : la premiere chose que font
/// `deposer_dsn_dans` et `deposer_dpae_avec`, seuls chemins vers `Client::deposer_octets` -- donc tout depot, test et
/// reel, signalements compris, d'ou qu'il vienne. Avant elle rien n'est lu ni ecrit : pas de ligne au journal des depots,
/// pas d'incident, pas de preuve demandee.
pub(crate) fn refuser_l_envoi(ctx: &Contexte) -> R<()> {
    match crate::bridage::refus_d_envoi(&ctx.racine) {
        Some(m) => Err(erreur(crate::bridage::CLASSE_REFUS_D_ENVOI, m)),
        None => Ok(()),
    }
}

/// `compagnies.refuser_si_copie` : la derniere porte -- une copie de developpement ne declare rien en reel.
fn refuser_si_copie(ctx: &Contexte, quoi: &str) -> R<()> {
    if ctx.racine.join("COPIE-DE-DEVELOPPEMENT.json").is_file() {
        return Err(erreur(
            "ErreurOperation",
            format!("Copie de developpement : {} ne part jamais d'ici. Les essais se font en test ; pour le faire en vrai, ouvrir la vraie compagnie.", quoi),
        ));
    }
    Ok(())
}

// ------------------------------------------------------------------------ la DSN

/// `libelle_dsn_deposee` : la phrase en face d'une DSN deja partie -- la meme pour les deux faces.
fn libelle_dsn_deposee(mois: &str, e: &Value) -> String {
    let quand = quand_a(&e["quand"]);
    let anomalies = || -> String {
        let mut v: Vec<String> = Vec::new();
        for a in e.get("anomalies").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]) {
            let t = valeur_en_texte(a);
            if !v.contains(&t) {
                v.push(t);
            }
        }
        v.join(" ; ")
    };
    if vrai(&e["refusee"]) {
        let a = anomalies();
        return format!("DSN de {} deposee le {} -- REFUSEE : {} -- a corriger et a renvoyer", mois, quand, if a.is_empty() { "motif non lu".to_string() } else { a });
    }
    if vrai(&e["passee"]) {
        return format!("DSN de {} deposee le {} -- bilan de conformite recu : elle est passee, rien a faire de notre cote", mois, quand);
    }
    format!("DSN de {} deposee le {} -- retour pas encore arrive : on attend, on ne renvoie pas", mois, quand)
}

/// `ordre_d_essai_du_fichier` : l'ordre auquel un essai part : le calcule, jamais plus bas que celui que le fichier porte
/// deja (journal §115). `None` reste `None`.
fn ordre_d_essai_du_fichier(calcule: Option<i64>, chemin: &Path) -> R<Option<i64>> {
    let Some(c) = calcule else { return Ok(None) };
    let octets = std::fs::read(chemin).map_err(|e| erreur("OSError", format!("{} : {}", chemin.display(), e)))?;
    Ok(Some(rang_dans_le_fichier(&octets).map_or(c, |m| c.max(m))))
}

/// Le mois de constitution (`AAAA-MM`) d'une date S20.G00.05.007 (`JJMMAAAA`) ; illisible, celui d'aujourd'hui (la
/// date de constitution est le jour ou le fichier s'ecrit, `dsn::generer`).
fn mois_de_constitution(jjmmaaaa: &str, aujourdhui: &socle::date::Jour) -> String {
    let c: Vec<char> = jjmmaaaa.trim().chars().collect();
    if c.len() == 8 && c.iter().all(char::is_ascii_digit) {
        return format!("{}-{}", c[4..8].iter().collect::<String>(), c[2..4].iter().collect::<String>());
    }
    aujourdhui.iso().chars().take(7).collect()
}

/// `re.search(rb"S20\.G00\.05\.004,'(\d+)'", octets)`.
fn rang_dans_le_fichier(octets: &[u8]) -> Option<i64> {
    let motif = b"S20.G00.05.004,'";
    let mut i = 0;
    while i + motif.len() <= octets.len() {
        if &octets[i..i + motif.len()] == motif {
            let chiffres: Vec<u8> = octets[i + motif.len()..].iter().take_while(|b| b.is_ascii_digit()).copied().collect();
            if !chiffres.is_empty() && octets.get(i + motif.len() + chiffres.len()) == Some(&b'\'') {
                return String::from_utf8_lossy(&chiffres).parse().ok();
            }
        }
        i += 1;
    }
    None
}

/// `prochain_ordre_de_test` : l'ordre a donner au prochain essai de ce mois, ou `None` pour le premier. Les
/// suivants partent en annule-et-remplace, a un ordre plus grand que tout ce que la base de test a pu retenir
/// : le plus grand des ordres deja poses par nous, de ceux des fichiers essayes encore sur le disque, du nombre
/// d'essais, et de ce que les bilans archives disent de la base d'essai pour le mois de constitution du fichier
/// (`ordre_signalements::dernier_ordre_des_mensuelles` : les essais faits AILLEURS sous le meme SIREN, l'atelier ou un
/// autre poste, comptent pour l'ordre sans s'afficher ; Leandre, 20/10/2026), plus un. Une mensuelle du mois deja
/// acceptee dans la base d'essai, meme venue d'ailleurs, fait partir l'essai en annule-et-remplace. Trop grand ne coute
/// rien ; trop petit, c'est B1-208-11. `constitution` : le mois de constitution du fichier (`AAAA-MM`, S20.G00.05.007).
fn prochain_ordre_de_test(ctx: &Contexte, mois: &str, nic: &Value, constitution: &str) -> R<Option<i64>> {
    let du_mois = crate::journal::lire(&ctx.racine, Some("dsn"), Some(mois), false).map_err(|m| erreur("OSError", m))?;
    let essais: Vec<Value> = etats::de_l_etablissement(&du_mois, nic, &etats::nic_principal(&ctx.entreprise))?
        .into_iter()
        .filter(|l| l.get("mode").and_then(Value::as_str) == Some("test"))
        .collect();
    let vues = crate::ordre_signalements::dernier_ordre_des_mensuelles(&ctx.racine, &crate::ordre_signalements::siren_de(ctx), true, constitution, mois)?;
    if essais.is_empty() && !vues.du_mois_acceptee {
        return Ok(None);
    }
    let mut connus: Vec<i64> = vec![essais.len() as i64, vues.ordre];
    for l in &essais {
        let d = l.get("detail").cloned().unwrap_or(Value::Null);
        let o = d.get("ordre_test").filter(|o| vrai(o)).map(valeur_en_texte).unwrap_or_default();
        if !o.is_empty() && o.chars().all(|c| c.is_ascii_digit()) {
            if let Ok(n) = o.parse() {
                connus.push(n);
            }
        }
        if let Some(nom) = l.get("fichier").filter(|f| vrai(f)).map(valeur_en_texte) {
            let p = ctx.racine.join("sorties").join(mois).join(nom);
            if p.exists() {
                if let Some(n) = std::fs::read(&p).ok().and_then(|o| rang_dans_le_fichier(&o)) {
                    connus.push(n);
                }
            }
        }
    }
    Ok(connus.into_iter().max().map(|m| m + 1))
}

/// `operations_remplacement.verifier_depot` : le garde-fou du depot REEL d'une mensuelle, ecrit depuis la
/// norme. `Ok(true)` : un annule-et-remplace en regle (rien a forcer) ; `Ok(false)` : une premiere DSN ou une
/// normale (`DsnDejaDeposee` tranche) ; un refus sinon.
fn verifier_depot_remplacement(o: &HashMap<String, String>, ctx: &Contexte, mois: &str, a: &Apercu, nic: &Value) -> R<bool> {
    let mut m = crate::routes_mois::Moteur::nouveau(o, ctx);
    let faits = m.depots_reels(mois, nic)?;
    let mut dernier: Option<i64> = None;
    for l in &faits {
        let n = m.ordre_de_la_ligne(mois, l)?;
        dernier = Some(dernier.map_or(n, |d| d.max(n)));
    }
    if !["03", "05"].contains(&a.champ("type_declaration")) {
        return Ok(false);
    }
    // CE QUE LA PLATE-FORME A VU EN REEL au mois de constitution du fichier, lu en silence dans les bilans archives
    // (`ordre_signalements::dernier_ordre_des_mensuelles`) : une mensuelle reelle envoyee d'un autre poste compte, comme
    // a la production (`depart::forme_a_produire`).
    let constitution = mois_de_constitution(a.champ("constitution"), &ctx.aujourdhui);
    let vues = crate::ordre_signalements::dernier_ordre_des_mensuelles(&ctx.racine, &crate::ordre_signalements::siren_de(ctx), false, &constitution, mois)?;
    let dernier = match dernier {
        Some(d) => d.max(vues.ordre),
        None if vues.du_mois_acceptee || vues.ordre > 0 => vues.ordre,
        None => return Ok(false),
    };
    let limite = m.limite(mois)?;
    if ctx.aujourdhui > limite {
        return Err(erreur(
            "AnnuleRemplaceHorsDelai",
            format!(
                "La DSN de {} est deja partie, et l'annule-et-remplace n'est plus admis depuis le {} a minuit (veille de l'echeance, cahier technique DSN §1.4.2). Ce qui a change dans ce mois part dans la DSN d'un mois suivant, en correction. Rien n'est parti.",
                mois,
                limite.jj_mm_aaaa()
            ),
        ));
    }
    let ordre: i64 = a.champ("ordre").trim().parse().unwrap_or(0);
    if ordre <= dernier {
        return Err(erreur(
            "OrdreTropPetit",
            format!(
                "Cet annule-et-remplace porte l'ordre {}, et la derniere DSN reelle connue pour {} (journal et bilans de la plate-forme) l'ordre {} : la plate-forme n'integre pas un rang inferieur ou egal (cahier technique DSN, p. 15 ; B1-208-11). Refaire la DSN du mois : elle partira a l'ordre {}. Rien n'est parti.",
                ordre,
                mois,
                dernier,
                dernier + 1
            ),
        ));
    }
    Ok(true)
}

/// `operations_declarations.deposer_dsn` : depose une DSN. La seule sortie du moteur qui declare quelque chose.
///
/// L'ORDRE COMPTE : 1. lire le fichier ; 2. verifier que le mode demande colle a la nature codee dedans ;
/// 3. exiger la preuve (reel) ; 4. exiger la REPETITION (reel, `contournement` pour la sortie de secours) ;
/// 5. seulement alors chercher le secret de l'organisme ; 6. deposer ; 7. NOTER, quoi qu'il arrive.
pub fn deposer_dsn(
    o: &HashMap<String, String>,
    ctx: &Contexte,
    fichier: &Value,
    reel: bool,
    preuve: Option<&Preuve>,
    qui: &str,
    forcer: bool,
    contournement: &str,
) -> R<DepotFait> {
    deposer_dsn_dans(o, ctx, fichier, reel, preuve, qui, "production", forcer, contournement, None)
}

/// `deposer_dsn` avec l'ENVIRONNEMENT du point de depot (`production` ou `editeur`, comme `Client(ctx, environnement)` du Python). La
/// plate-forme de l'editeur est une plate-forme d'ESSAI : un depot REEL n'y a pas de sens et reste refuse ici, car le journal le
/// noterait comme une declaration partie.
///
/// `ordre_d_essai` (`paie depot-dsn --test --ordre N`, lot E1) : l'ordre au-dessous duquel un ESSAI de mensuelle ne part pas,
/// donne par qui sait ce que la base d'essai a deja retenu pour ce SIRET et ce mois (un autre poste, une compagnie d'essai, le
/// « dernier ordre accepte » d'un B1-208-11). L'essai part alors en annule-et-remplace a cet ordre, ou plus haut si le journal
/// en sait davantage. Refuse pour un depot reel et pour un signalement (leur ordre est celui du fichier, lu sur la
/// plate-forme a son ecriture : `ordre_signalements`).
#[allow(clippy::too_many_arguments)]
pub fn deposer_dsn_dans(
    o: &HashMap<String, String>,
    ctx: &Contexte,
    fichier: &Value,
    reel: bool,
    preuve: Option<&Preuve>,
    qui: &str,
    environnement: &str,
    forcer: bool,
    contournement: &str,
    ordre_d_essai: Option<i64>,
) -> R<DepotFait> {
    refuser_l_envoi(ctx)?;
    if reel && environnement != "production" {
        return Err(erreur(
            "ErreurOperation",
            format!("L'environnement '{}' est la plate-forme d'essai de l'editeur : rien n'y est declare. Un depot REEL ne part que sur 'production' ; pour essayer, utiliser --test. Rien n'est parti.", environnement),
        ));
    }
    let chemin = PathBuf::from(crate::routes_apercus::resoudre(&ctx.racine, fichier)?);
    let octets = crate::routes_apercus::lire_octets(&chemin.to_string_lossy())?;
    let apercu = depot::lire(&octets);
    // 2. le mode contre le fichier ; `presence_prouvee` : c'est ICI qu'on la prouve, jamais le transport.
    depot::verifier(&apercu, reel, None, true)?;
    let mois = apercu.mois_lisible();
    if let Some(n) = ordre_d_essai {
        if reel || apercu.est_signalement() || n < 1 {
            return Err(erreur(
                "ErreurOperation",
                "Un ordre d'essai (--ordre) ne vaut que pour le depot de TEST d'une DSN mensuelle, et c'est un entier positif : un depot reel part a l'ordre du fichier, un signalement a celui que la plate-forme a donne a son ecriture. Rien n'est parti.",
            ));
        }
    }
    // UNE COMPAGNIE D'ESSAI ne depose que ce qu'elle a ecrit sous SON identite : les DSN copiees de la fictive portent le
    // SIREN de la fictive, que le compte net-entreprises ne declare pas. Le refus ne cite aucun des deux numeros.
    if crate::essai::est_essai(&ctx.racine) && apercu.champ("siren") != valeur_en_texte(ctx.entreprise.get("siren").unwrap_or(&Value::Null)) {
        return Err(erreur(
            "ErreurOperation",
            "Compagnie d'essai : ce fichier ne porte pas le SIREN de la compagnie (il a ete ecrit sous une autre identite : celle de la compagnie fictive, avant le montage, ou une identite d'avant). Deposer ici la DSN du mois (`paie depot-dsn AAAA-MM`), qui s'y fabrique. Rien n'est parti.",
        ));
    }
    let nic: Value = if apercu.champ("etablissement").is_empty() { Value::Null } else { json!(apercu.champ("etablissement")) };
    let principal = etats::nic_principal(&ctx.entreprise);
    let lignes = etats::charger(&ctx.racine)?;

    let (mut forme, mut repetition) = (String::new(), Value::Null);
    // UN SIGNALEMENT REEL PART A UN ORDRE QUE PERSONNE N'A ENCORE PRIS : la sequence ne se remet jamais a zero
    // (cahier technique DSN, p. 136), et la plate-forme n'accepte pas un second signalement normal au meme ordre
    // (p. 14). Deux signalements ecrits avant que le premier parte portent le meme : le second se refait. Refuse
    // avant la preuve : aucun mot de passe n'y change rien.
    if reel && apercu.est_signalement() {
        let dernier = crate::journal::dernier_ordre_des_signalements(&ctx.racine).map_err(|m| erreur("ErreurOperation", m))?;
        let ordre: i64 = apercu.champ("ordre").trim().parse().unwrap_or(0);
        if ordre <= dernier {
            return Err(erreur(
                "ErreurOperation",
                format!(
                    "Ce signalement porte l'ordre {}, et un signalement reel est deja parti a l'ordre {} : le numero d'ordre d'un signalement ne se remet jamais a zero et ne se prend pas deux fois (cahier technique DSN, p. 136 et 14). Refaire le signalement : il partira a l'ordre {}. Rien n'est parti.",
                    ordre,
                    dernier,
                    dernier + 1
                ),
            ));
        }
    }
    if reel {
        forme = verifier_preuve(ctx, preuve, &apercu.confirmation_attendue(), "depot REEL d'une DSN", "net-entreprises (la declaration part a l'Urssaf et aux organismes)")?;
        // PAS DEUX FOIS SANS LE VOULOIR. Une DSN reelle deja partie pour ce mois, et pas refusee, ne repart pas.
        // UN SIGNALEMENT N'EST PAS CONCERNE : plusieurs peuvent partir le meme mois.
        if !apercu.est_signalement() {
            let mut etat_mois = etats::etat_dsn_du_mois(&lignes, &mois, &nic, &principal)?;
            // L'ANNULE-ET-REMPLACE EN REGLE PART SANS QU'ON FORCE RIEN (30/09/2026).
            if verifier_depot_remplacement(o, ctx, &mois, &apercu, &nic)? {
                etat_mois = Value::Null;
            }
            if vrai(&etat_mois) && !vrai(&etat_mois["refusee"]) && !forcer {
                return Err(avec_donnee(
                    "DsnDejaDeposee",
                    format!(
                        "{}. Rien n'est parti. Ce fichier est une declaration NORMALE, et la plate-forme n'en accepte pas une seconde pour le meme mois (cahier technique DSN §1.4.3.2) : pour corriger, refaire la DSN du mois -- elle sortira en annule-et-remplace.",
                        libelle_dsn_deposee(&mois, &etat_mois)
                    ),
                    "etat",
                    etat_mois,
                ));
            }
        }
        // LA PAIE A CHANGE DEPUIS L'ECRITURE (30/09/2026) : le fichier est peut-etre conforme a son test, il ne
        // declare plus la paie.
        let change = if apercu.est_signalement() {
            None
        } else {
            crate::routes_mois::Moteur::nouveau(o, ctx).paie_changee_depuis_l_ecriture(&mois, &chemin)?
        };
        if let Some(c) = change {
            return Err(erreur(
                "ErreurOperation",
                format!(
                    "{} : {}. Rien n'est parti. Reecrire le mois, deposer la nouvelle DSN en test, puis l'envoyer.",
                    chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                    c
                ),
            ));
        }
        // Et la repetition, en DERNIER des garde-fous.
        repetition = exiger_repetition(ctx, &lignes, "dsn", &chemin, contournement)?;
    }

    let empreinte = crate::journal::empreinte(&octets);
    let deja = etats::deja_depose(&lignes, "dsn", &empreinte)?;
    let joue = crate::routes_accueil::simulation_active(&ctx.racine);
    let mut mdp: Option<String> = None;
    if !joue {
        mdp = motdepasse_organisme(ctx, "net-entreprises", preuve.and_then(|p| p.motdepasse.as_deref()))?;
    }

    // UN ESSAI DE PLUS SUR UN MOIS DEJA ESSAYE part en annule-et-remplace, a l'ordre suivant : la base de test
    // garde les essais. JAMAIS PLUS BAS QUE L'ORDRE DU FICHIER (journal §115).
    let ordre_test = if reel || apercu.est_signalement() { None } else { prochain_ordre_de_test(ctx, &mois, &nic, &mois_de_constitution(apercu.champ("constitution"), &ctx.aujourdhui))? };
    let ordre_test = ordre_test.max(ordre_d_essai);
    let ordre_test = ordre_d_essai_du_fichier(ordre_test, &chemin)?;

    let envoi = || -> R<String> {
        if joue {
            return reponse_fictive("dsn", reel);
        }
        let mut client = Client::dsn(ctx, environnement, mdp.clone()).map_err(Panne::en_echec)?;
        client.authentifier().map_err(Panne::en_echec)?;
        // La verification est refaite ici : la derniere porte avant le reseau.
        let a = depot::lire(&octets);
        depot::verifier(&a, reel, None, true)?;
        let mut contenu = octets.clone();
        if reel {
            refuser_si_copie(ctx, "une DSN reelle")?;
        } else {
            contenu = depot::basculer_dsn(&contenu)?;
            if let Some(n) = ordre_test.filter(|n| *n != 0) {
                contenu = depot::renumeroter(&contenu, &n.to_string())?;
            }
        }
        let url = client.adresses.depot.clone();
        let (reponse, _type) = client.deposer_octets(&url, &contenu, "dsn").map_err(Panne::en_echec)?;
        Ok(reponse)
    };
    let reponse = match envoi() {
        Ok(r) => r,
        Err(e) => {
            let classe = e.classe.unwrap_or("Exception");
            let phrase = phrase_de(&e);
            let _ = noter(
                ctx, "dsn", &mois, &chemin, reel, qui, false, "net-entreprises", "", &phrase.chars().take(600).collect::<String>(), None,
                &json!({"preuve": forme, "erreur": classe, "repetition": repetition, "nature": apercu.champ("nature"), "ordre": apercu.champ("ordre")}), None,
            );
            noter_incident_depot(ctx, "dsn", reel, classe, &phrase, &repetition);
            return Err(e);
        }
    };

    let flux = etats::flux_de(&reponse);
    let trace = archiver_reponse(ctx, "depot-dsn", &mois, &reponse)?;
    let detail = json!({
        "preuve": forme, "type": apercu.champ("type_declaration"),
        // LA NATURE : un signalement se numerote a la suite de tous ceux deja partis (`journal::dernier_ordre_des_signalements`).
        "nature": apercu.champ("nature"),
        // LE RANG DECLARE : le prochain annule-et-remplace se numerote au-dessus de cet ordre.
        "ordre": apercu.champ("ordre"),
        "versements": apercu.versements, "salaries": apercu.salaries,
        // CE QUI A ETE PRELEVE SUR LA GRILLE, et non sur le taux du salarie : la seule trace qui reste.
        "pas_bareme": apercu.pas_bareme,
        "repetition": repetition, "ordre_test": ordre_test,
        "etablissement": apercu.champ("etablissement"),
    });
    // LA PAIE DECLAREE : la version de chaque engagement du mois sur laquelle la mensuelle part ; une paie qui change
    // ensuite appelle un annule-et-remplace (`engagement::etats::a_refaire`).
    let versions = if apercu.est_signalement() { None } else { Some(crate::journal::versions_du_mois(ctx, &mois, |_, _, _| true).map_err(Echec::from)?) };
    let ligne = noter(ctx, "dsn", &mois, &chemin, reel, qui, true, "net-entreprises", &flux, &reponse, Some(&trace), &detail, versions.as_ref())?;
    let quand = ligne.get("quand").and_then(Value::as_str).unwrap_or("").to_string();
    if reel && apercu.est_signalement() {
        // LA TRACE SUR L'ARRET LUI-MEME : ce qui est parti, quand, avec quel numero d'ordre.
        crate::marques_depot::marquer_envoye_depuis(
            ctx,
            &chemin,
            apercu.champ("nature"),
            apercu.champ("ordre"),
            apercu.champ("type_declaration"),
            &apercu.evenement_lisible(),
            &quand,
        )?;
        // ET LA TRACE D'UNE DEMANDE DE TAUX, quand c'est un signalement d'amorcage : sans elle, le moteur
        // redirait << aucun taux connu, il faut demander >> a chaque ouverture.
        if apercu.champ("nature") == crate::amorcage::NATURE {
            crate::marques_depot::marquer_demande_depuis(ctx, &chemin, &quand)?;
        }
    }
    if reel && !apercu.est_signalement() && !joue {
        // LE REVEIL DE LA DSN SE TAIT (30/09/2026). Ne leve jamais.
        let _ = crate::filet::desarmer(&ctx.racine, &format!("dsn|{}", mois));
    }
    if reel && !apercu.est_signalement() {
        // Les corrections de mois anterieurs que ce fichier portait sont PORTEES ; et les arrets que la DSN a
        // reportes le savent.
        crate::marques_depot::marquer_portees(ctx, &mois, &chemin, &quand)?;
        crate::marques_depot::marquer_reportes(ctx, &mois, &chemin, &quand)?;
    }
    Ok(DepotFait { quoi: "dsn".into(), mois, reel, reponse, flux, trace: Some(trace), ligne, deja, repetition })
}

// ----------------------------------------------------------------------- la DPAE

/// `operations_declarations.deposer_dpae` : depose une DPAE. Meme ordre, memes regles que la DSN. La DPAE
/// declare des EMBAUCHES, et le journal retient lesquelles (salarie, premier jour).
pub fn deposer_dpae(
    o: &HashMap<String, String>,
    ctx: &Contexte,
    fichier: &Value,
    reel: bool,
    preuve: Option<&Preuve>,
    qui: &str,
    majuscules: bool,
    contournement: &str,
) -> R<DepotFait> {
    deposer_dpae_avec(o, ctx, fichier, reel, preuve, qui, majuscules, contournement, None)
}

/// `deposer_dpae` avec, au besoin, le client Urssaf que l'appelant donne a la place du vrai (la preuve : un client pointe sur
/// un serveur local, lot I2b). Un client donne n'est jamais confronte au coffre, et ses identifiants sont verifies comme ceux
/// d'un client construit ici. Sans client, rien ne change.
#[allow(clippy::too_many_arguments)]
pub fn deposer_dpae_avec(
    o: &HashMap<String, String>,
    ctx: &Contexte,
    fichier: &Value,
    reel: bool,
    preuve: Option<&Preuve>,
    qui: &str,
    majuscules: bool,
    contournement: &str,
    client_donne: Option<&mut Client>,
) -> R<DepotFait> {
    refuser_l_envoi(ctx)?;
    let chemin = PathBuf::from(crate::routes_apercus::resoudre(&ctx.racine, fichier)?);
    let apercu = crate::routes_apercus::apercu_depot_dpae(o, ctx, &json!(chemin.to_string_lossy()))?;
    let octets = crate::routes_apercus::lire_octets(&chemin.to_string_lossy())?;
    let mois = valeur_en_texte(&apercu["mois"]);

    if reel && !vrai(&apercu["reel"]) {
        return Err(erreur(
            "ErreurOperation",
            format!(
                "Le fichier porte l'indicateur {} (test) et vous demandez un depot REEL : rien ne serait declare. Reproduire le fichier sans --test.",
                valeur_en_texte(&apercu["indicateur"])
            ),
        ));
    }

    let (mut forme, mut repetition) = (String::new(), Value::Null);
    if reel {
        forme = verifier_preuve(ctx, preuve, &mois, "depot REEL d'une DPAE", "l'Urssaf (declaration d'embauche)")?;
        // LA REPETITION, apres la preuve : le fichier sur le disque porte toujours l'indicateur reel, et c'est
        // NOUS qui le basculons en test juste en dessous.
        let lignes = etats::charger(&ctx.racine)?;
        repetition = exiger_repetition(ctx, &lignes, "dpae", &chemin, contournement)?;
    }

    let mut contenu = octets.clone();
    if !reel {
        contenu = depot::basculer_dpae(&contenu, depot::LONGUEUR_DPAE)?;
    }

    let joue = crate::routes_accueil::simulation_active(&ctx.racine);
    let mut client: Option<Client> = None;
    let mut donne = client_donne;
    if !joue {
        let soucis = match donne.as_deref() {
            Some(c) => c.verifier_identifiants()?,
            None => {
                let mdp = motdepasse_organisme(ctx, "urssaf", preuve.and_then(|p| p.motdepasse.as_deref()))?;
                let c = Client::dpae(ctx, majuscules, mdp);
                let soucis = c.verifier_identifiants()?;
                client = Some(c);
                soucis
            }
        };
        if !soucis.is_empty() {
            return Err(erreur("ErreurOperation", format!("Ces points feraient echouer la connexion MtoM. Rien n'est envoye :\n  {}", soucis.join("\n  "))));
        }
    }

    let mut envoi = || -> R<String> {
        if joue {
            return reponse_fictive("dpae", reel);
        }
        let c: &mut Client = match donne.as_deref_mut() {
            Some(c) => c,
            None => client.as_mut().ok_or_else(|| erreur("ErreurOperation", "client absent"))?,
        };
        c.authentifier().map_err(Panne::en_echec)?;
        if depot::est_reel(&contenu) {
            refuser_si_copie(ctx, "une DPAE reelle")?;
        }
        let url = c.adresses.depot.clone();
        let (reponse, _type) = c.deposer_octets(&url, &contenu, "dpae").map_err(Panne::en_echec)?;
        Ok(reponse)
    };
    let embauches: Vec<Value> = apercu["embauches"]
        .as_array()
        .map(|a| a.iter().map(|l| json!({"salarie": l["salarie"], "debut": l["debut"], "fin": l["fin"]})).collect())
        .unwrap_or_default();
    let reponse = match envoi() {
        Ok(r) => r,
        Err(e) => {
            let classe = e.classe.unwrap_or("Exception");
            let phrase = phrase_de(&e);
            let _ = noter(
                ctx, "dpae", &mois, &chemin, reel, qui, false, "urssaf", "", &phrase.chars().take(600).collect::<String>(), None,
                &json!({"preuve": forme, "erreur": classe, "repetition": repetition}), None,
            );
            noter_incident_depot(ctx, "dpae", reel, classe, &phrase, &repetition);
            return Err(e);
        }
    };

    let trace = archiver_reponse(ctx, "depot-dpae", &mois, &reponse)?;
    let flux = etats::flux_de(&reponse);
    // Ce que ce depot a REELLEMENT declare, a la maille de l'embauche : c'est ce qui repondra ensuite a
    // « lesquelles restent a faire ».
    let detail = json!({"preuve": forme, "repetition": repetition, "embauches": embauches});
    // La version de chaque engagement dont l'embauche part : une date d'embauche qui change ensuite le dit.
    let declare = |cle: &str, e: &Value| embauches.iter().any(|x| x["salarie"].as_str() == Some(cle) && x["debut"].as_str() == Some(engagement::debut_brut(e).as_str()));
    let versions = crate::journal::versions_du_mois(ctx, &mois, |cle, _, e| declare(cle, e)).map_err(Echec::from)?;
    let ligne = noter(ctx, "dpae", &mois, &chemin, reel, qui, true, "urssaf", &flux, &reponse, Some(&trace), &detail, Some(&versions))?;
    // LE REVEIL SE TAIT (27/09/2026) : la DPAE est partie, le filet n'a plus a sonner pour ces embauches.
    if reel && !joue {
        for e in &embauches {
            let _ = crate::filet::desarmer(&ctx.racine, &crate::filet::cle_du_reveil(&json!({"salarie": e["salarie"], "debut": e["debut"]})));
        }
    }
    Ok(DepotFait { quoi: "dpae".into(), mois, reel, reponse, flux, trace: Some(trace), ligne, deja: apercu["deja_depose"].clone(), repetition })
}

#[cfg(test)]
mod tests_archives {
    use super::*;

    #[test]
    fn la_reponse_d_un_depot_n_en_ecrase_jamais_une_autre() {
        // Defaut de l'ancien Python (lot I1) : un depot de test puis le reel, dans la meme seconde, portaient le meme nom
        // `<horodatage>_depot-dsn_<mois>.txt` et la reponse du test disparaissait.
        let r = crate::horloge::racine_d_essai(false);
        let ctx = Contexte::nouveau(&r, None).unwrap();
        let mut chemins = Vec::new();
        for i in 0..6 {
            chemins.push(archiver_reponse(&ctx, "depot-dsn", "2026-09", &format!("reponse {}", i)).unwrap());
        }
        let mut tous = chemins.clone();
        tous.sort();
        tous.dedup();
        assert_eq!(tous.len(), 6, "{:?}", chemins);
        for (i, c) in chemins.iter().enumerate() {
            assert_eq!(std::fs::read_to_string(c).unwrap(), format!("reponse {}", i), "{}", c.display());
            let nom = c.file_name().unwrap().to_string_lossy().into_owned();
            assert!(nom.ends_with("_depot-dsn_202609.txt") || nom.contains("_depot-dsn_202609-"), "{}", nom);
        }
        // Le nom ordinaire (premier de la seconde) ne change pas : `AAAAMMJJ-HHMMSS_depot-dsn_AAAAMM.txt`.
        let ordinaire = chemins.iter().filter(|c| c.to_string_lossy().ends_with("_depot-dsn_202609.txt")).count();
        assert!(ordinaire >= 1);
    }
}
