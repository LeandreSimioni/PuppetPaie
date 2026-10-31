//! La route `rappels` : ce que le logiciel propose en ouvrant une compagnie (la DSN des mois passes, ce qui est du -- les
//! fins de contrat qui se signalent et ce qu'un arret fait naitre, de `obligations.rs` --, les DPAE, la cloture de
//! l'exercice, l'export compta, la revue des spectacles). Aucun calcul de paie : les engagements se lisent sans
//! chiffrer, et leur etat (`engagement::etats`), le journal par `etats`. Ses lectures servent aussi le panneau et le
//! filet (`mois_a_declarer`, `exercice_clos`) ; les DPAE a faire se lisent au panneau (`etapes::Moteur::embauches_a_declarer`).
//!
//! Un refus porte sa `classe` : voir `encoder` et `decoder`, que `mod.rs` utilise.

use socle::date::jj_mm_aaaa;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use droit::contexte::{archivee, Contexte};
use socle::date::{en_texte, Jour};
use engagement;
use envois::etats;
use crate::compagnie::etat_dsn;
use socle::fichiers::{lire_json, vrai};
use socle::num::Resultat;

// ------------------------------------------------------------------ les refus

pub(crate) use socle::echec::Refus;
pub(crate) type R<T> = Result<T, Refus>;

fn avec_classe(classe: &'static str, message: impl Into<String>) -> Refus {
    Refus { classe: Some(classe), message: message.into() }
}

const SEPARATEUR: char = '\u{1}';

/// Le refus en texte : `\u{1}classe\u{1}message` quand il a une classe, le
/// message seul sinon (le `Resultat<Value>` des routes ne porte qu'un texte).
fn encoder(r: Refus) -> String {
    match r.classe {
        Some(c) => format!("{}{}{}{}", SEPARATEUR, c, SEPARATEUR, r.message),
        None => r.message,
    }
}

/// L'inverse de `encoder` : `(classe, message)`.
pub fn decoder(erreur: &str) -> (Option<&str>, &str) {
    if let Some(reste) = erreur.strip_prefix(SEPARATEUR) {
        if let Some((classe, message)) = reste.split_once(SEPARATEUR) {
            return (Some(classe), message);
        }
    }
    (None, erreur)
}

/// Une route de l'ouverture : `None` si ce n'est pas la sienne.
pub fn traiter(o: &HashMap<String, String>, route: &str, charge: &Value) -> Option<Resultat<Value>> {
    let r = match route {
        "rappels" => rappels(o, charge),
        _ => return None,
    };
    Some(r.map_err(encoder))
}

// ------------------------------------------------------------------ outils

/// Une valeur facultative en texte (`None` si absente).
pub(super) fn texte(v: Option<&Value>) -> String {
    v.map_or("None".to_string(), en_texte)
}

fn vrai_opt(v: Option<&Value>) -> bool {
    v.map_or(false, vrai)
}

/// La valeur de `cle` en texte, vide si elle manque ou est vide.
fn texte_ou_vide(v: &Value, cle: &str) -> String {
    match v.get(cle) {
        Some(x) if vrai(x) => en_texte(x),
        _ => String::new(),
    }
}

fn mois_precedent(mois: &str) -> String {
    let (an, mo): (i32, u32) = (mois[..4].parse().unwrap_or(0), mois[5..7].parse().unwrap_or(1));
    if mo == 1 {
        format!("{:04}-12", an - 1)
    } else {
        format!("{:04}-{:02}", an, mo - 1)
    }
}

fn mois_suivant(mois: &str) -> String {
    let (an, mo): (i32, u32) = (mois[..4].parse().unwrap_or(0), mois[5..7].parse().unwrap_or(1));
    if mo == 12 {
        format!("{:04}-01", an + 1)
    } else {
        format!("{:04}-{:02}", an, mo + 1)
    }
}

fn mois_de(j: &Jour) -> String {
    format!("{:04}-{:02}", j.an, j.mois)
}

/// Ce que `journal.lire(ctx, quoi, reel_seulement=...)` rend, sur des lignes deja lues.
fn lire_journal<'a>(lignes: &'a [Value], quoi: &str, reel: bool) -> R<Vec<&'a Value>> {
    Ok(etats::filtrer(lignes, Some(quoi), None, reel)?)
}

// ============================================================ les rappels

const DSN: &str = "dsn";
const CLOTURE: &str = "cloture";
const EXPORT_COMPTA: &str = "export-compta";
const REVUE_SPECTACLES: &str = "revue-spectacles";
/// Une obligation due (`obligations.rs`) : la fin d'un contrat qui se signale, ce qu'un arret fait naitre.
const OBLIGATION: &str = "obligation";
const DPAE: &str = "dpae";

/// Ou en est le mois dont on rappelle la DSN : un mois sans saisie se cree, un
/// mois saisi se produit, un mois produit se depose, une DSN refusee se corrige.
const A_SAISIR: &str = "a_saisir";
const A_PRODUIRE: &str = "a_produire";
const A_DEPOSER: &str = "a_deposer";
const A_CORRIGER: &str = "a_corriger";

/// Un rappel est une DONNEE : quoi, pour quel mois, ou en est-on, et ce qu'il y
/// a a faire. Il ne fait rien : << verifier et envoyer >> passe par les
/// operations existantes, avec leurs garde-fous et leur mot de passe.
struct Rappel {
    quoi: &'static str,
    cle: String,
    titre: String,
    question: String,
    mois: Option<String>,
    annee: Option<String>,
    etat: String,
    detail: String,
    fichier: Option<String>,
    en_retard: bool,
    engagements: i64,
    validees: i64,
    spectacles: Vec<Value>,
    salarie: Option<String>,
    jour: Option<String>,
    a_saisir: Vec<String>,
    /// Le rappel d'une DSN : son echeance de depot (`echeance_dsn::Echeance::en_json`) ; `null` pour les autres.
    echeance_depot: Option<Value>,
    /// Le rappel d'une obligation due : l'obligation telle que `obligations.rs` l'a calculee ; `null` pour les autres.
    obligation: Option<Value>,
    /// Ce sur quoi elle porte (la fin de contrat, l'arret resume), que la page repasse au geste qui la fait.
    objet: Option<Value>,
}

impl Rappel {
    fn nouveau(quoi: &'static str, cle: String, titre: String, question: String) -> Rappel {
        Rappel {
            quoi,
            cle,
            titre,
            question,
            mois: None,
            annee: None,
            etat: String::new(),
            detail: String::new(),
            fichier: None,
            en_retard: false,
            engagements: 0,
            validees: 0,
            spectacles: Vec::new(),
            salarie: None,
            jour: None,
            a_saisir: Vec::new(),
            echeance_depot: None,
            obligation: None,
            objet: None,
        }
    }

    /// `sortie_json.brut(Rappel)`.
    fn json(&self) -> Value {
        let opt = |o: &Option<String>| o.clone().map_or(Value::Null, Value::String);
        json!({
            "quoi": self.quoi, "cle": self.cle, "titre": self.titre, "question": self.question,
            "mois": opt(&self.mois), "annee": opt(&self.annee), "etat": self.etat,
            "detail": self.detail, "fichier": opt(&self.fichier), "en_retard": self.en_retard,
            // En retard : a faire, la compagnie le peut.
            "pastille": if self.en_retard { super::pastille("en_retard", "en retard") } else { Value::Null },
            "engagements": self.engagements, "validees": self.validees,
            "spectacles": self.spectacles.iter().map(brut_valeur).collect::<Vec<_>>(),
            "salarie": opt(&self.salarie), "jour": opt(&self.jour), "a_saisir": self.a_saisir,
            "echeance_depot": self.echeance_depot,
            "obligation": self.obligation, "objet": self.objet.as_ref().map(brut_valeur),
        })
    }
}

/// `sortie_json.brut` d'un JSON : les cles qui commencent par `_` s'en vont.
pub(super) fn brut_valeur(v: &Value) -> Value {
    match v {
        Value::Object(o) => Value::Object(o.iter().filter(|(k, _)| !k.starts_with('_')).map(|(k, x)| (k.clone(), brut_valeur(x))).collect()),
        Value::Array(a) => Value::Array(a.iter().map(brut_valeur).collect()),
        autre => autre.clone(),
    }
}

/// Ce qu'un rappel lit : la compagnie, le jour, et le journal des depots (lu une fois).
pub(crate) struct Etat<'a> {
    pub(crate) ctx: &'a Contexte,
    pub(crate) aujourdhui: Jour,
    pub(crate) journal: std::sync::Arc<Vec<Value>>,
}

impl Etat<'_> {
    fn racine(&self) -> &Path {
        &self.ctx.racine
    }
}

/// `rappels` : `{"rappels": [...]}`. `charge.aujourdhui` (AAAA-MM-JJ) remplace
/// le jour du contexte (la ligne de commande `rappels --le`).
fn rappels(o: &HashMap<String, String>, charge: &Value) -> R<Value> {
    let ctx = crate::service::contexte(o)?;
    let aujourdhui = match charge.get("aujourdhui").filter(|v| vrai(v)) {
        Some(v) => Jour::lire(&en_texte(v))?,
        None => ctx.aujourdhui,
    };
    let st = Etat { ctx: &ctx, aujourdhui, journal: envois::journal::lu(&ctx.racine).map(|j| j.lignes)? };
    let out = lister(&st)?;
    Ok(json!({ "rappels": out.iter().map(Rappel::json).collect::<Vec<_>>() }))
}

/// Ce que le logiciel doit proposer a
/// l'ouverture, dans l'ordre ou ca se fait.
///
/// D'abord la DSN des mois passes qui n'en ont pas. Puis, si on est en janvier
/// ou apres et que l'exercice d'avant n'est pas clos : sa cloture -- apres sa
/// derniere DSN, jamais avant --, l'export compta, et les numeros d'objet de
/// l'annee qui commence. Chaque rappel se tait des que la chose est faite ;
/// aucun ne se tait parce qu'on a clique << plus tard >>.
fn lister(st: &Etat) -> R<Vec<Rappel>> {
    let aujourdhui = st.aujourdhui;
    let mut dsn = Vec::new();
    for m in mois_a_declarer(st)? {
        dsn.push(rappel_dsn(st, &m)?);
    }
    // CE QUI EST DU AVANT LE RESTE (les obligations de `obligations.rs`) : quarante-huit heures pour un accident, cinq
    // jours ouvres pour un signalement -- les delais les plus courts du logiciel, et ils courent depuis un evenement, pas
    // depuis une fin de mois qu'on voit venir.
    let mut out = rappels_obligations(st)?;
    out.extend(dsn);
    // ET LES DPAE AVANT TOUT : elles se comptent en heures, pas en jours ouvres
    // -- avant la premiere heure de travail. Un rappel qui ne se calcule pas
    // n'empeche pas les autres.
    if let Ok(mut dpae) = rappels_dpae(st) {
        dpae.extend(out);
        out = dpae;
    }

    let precedent = aujourdhui.an - 1;
    let decembre = format!("{}-12", precedent);
    if !vrai_opt(exercice_clos(st.ctx, &precedent.to_string())?.as_ref()) && !mois_de_l_annee(st.ctx, &precedent.to_string()).is_empty() {
        // La cloture vient APRES la DSN de decembre : tant qu'elle manque, c'est
        // son rappel qui parle, pas celui-ci.
        let e = etat_dsn::du_mois(st.ctx, &decembre, &Value::Null)?;
        if e.partie() || e.refusee() {
            let mut r = Rappel::nouveau(
                CLOTURE,
                format!("cloture:{}", precedent),
                format!("Clôturer l'exercice {}", precedent),
                format!("La DSN de décembre est passée. Figer l'année {} ?", precedent),
            );
            r.annee = Some(precedent.to_string());
            r.detail = format!(
                "Les saisies et les sorties de {} ne se modifieront plus, et tout ce qu'il faut garder (bulletins, \
                 contrats signés, DPAE, DSN, retours, export compta) est réuni dans une archive.",
                precedent
            );
            out.push(r);
        }
        if !crate::compagnie::exercice_ecrit::export_compta_a_jour(&st.ctx.racine, &precedent.to_string()) {
            let mut r = Rappel::nouveau(
                EXPORT_COMPTA,
                format!("export-compta:{}", precedent),
                format!("Exporter les chiffres de {} pour la comptabilité", precedent),
                "Le journal de paie de l'année, pour le logiciel de compta.".into(),
            );
            r.annee = Some(precedent.to_string());
            r.detail = "Une écriture par mois, équilibrée : salaires bruts et charges au débit, organismes et net à payer au crédit.".into();
            out.push(r);
        }
    }

    // La revue des spectacles : au premier trimestre, une fois par annee. Un
    // spectacle qui ne se joue plus s'archive ; les autres continuent, avec leur
    // numero d'objet.
    let a_revoir = if aujourdhui.mois <= 3 { spectacles_a_revoir(st.ctx, aujourdhui.an)? } else { Vec::new() };
    if !a_revoir.is_empty() {
        let mut r = Rappel::nouveau(
            REVUE_SPECTACLES,
            format!("{}:{}", REVUE_SPECTACLES, aujourdhui.an),
            format!("Les spectacles de {}", aujourdhui.an),
            format!("Lesquels de vos spectacles se jouent encore en {} ?", aujourdhui.an),
        );
        r.annee = Some(aujourdhui.an.to_string());
        r.detail = "Les autres s'archivent : ils restent lisibles et se réactivent d'un clic (Configuration › Spectacles).".into();
        r.spectacles = a_revoir;
        out.push(r);
    }
    Ok(out)
}

// -------------------------------------------------------------- l'exercice

/// La cloture de l'annee (ou `None`).
pub(crate) fn exercice_clos(ctx: &Contexte, annee: &str) -> R<Option<Value>> {
    let p = ctx.racine.join("config").join("exercices.json");
    if !socle::fichiers::existe(&p) {
        return Ok(None);
    }
    // Lu une fois tant qu'il ne change pas, le refus d'un fichier illisible inchange (`lire_json`).
    match &*socle::fichiers::json_en_cache_ou(&p, lire_json)? {
        Value::Object(o) => Ok(o.iter().find(|(k, _)| !k.starts_with('_') && k.as_str() == &annee[..4.min(annee.len())]).map(|(_, v)| v.clone())),
        autre => Err(avec_classe("AttributeError", format!("config/exercices.json : {}", socle::fichiers::attendu("un objet", socle::valeur::nom_type(&autre))))),
    }
}

/// Les mois de l'annee connus du dossier.
pub(crate) fn mois_de_l_annee(ctx: &Contexte, annee: &str) -> Vec<String> {
    let mut v: Vec<String> = super::ouverture::mois_connus(&ctx.racine).into_iter().map(|m| m.0).filter(|m| m.starts_with(annee)).collect();
    v.sort();
    v
}

/// `spectacles_a_revoir` : les spectacles actifs, tant que la revue de l'annee
/// n'est pas faite.
fn spectacles_a_revoir(ctx: &Contexte, annee: i32) -> R<Vec<Value>> {
    if super::mois::cochees(ctx)?.contains_key(&format!("{}:{}", REVUE_SPECTACLES, annee)) {
        return Ok(Vec::new());
    }
    Ok(ctx
        .spectacles
        .iter()
        .filter(|(_, s)| !archivee(s))
        .map(|(cle, s)| {
            let titre = s.get("titre").filter(|t| vrai(t)).cloned().unwrap_or_else(|| Value::String(cle.clone()));
            let objet = s.get("numero_objet").filter(|t| vrai(t)).cloned().unwrap_or(Value::Null);
            json!({ "cle": cle, "titre": titre, "numero_objet": objet })
        })
        .collect())
}

// ------------------------------------------------------- la DSN des mois

/// Le mois le plus recent dont la DSN reelle est partie et n'a pas ete
/// refusee -- passee, ou en attente de son bilan.
fn derniere_dsn_passee(st: &Etat) -> R<Option<String>> {
    let mut mois = BTreeSet::new();
    for l in lire_journal(&st.journal, DSN, true)? {
        if vrai_opt(l.get("accepte")) && vrai_opt(l.get("mois")) {
            mois.insert(en_texte(&l["mois"]));
        }
    }
    let mut passes = Vec::new();
    for m in mois {
        let e = etat_dsn::du_mois(st.ctx, &m, &Value::Null)?;
        if !e.a_faire() {
            passes.push(m);
        }
    }
    Ok(passes.into_iter().max())
}

/// Le premier mois dont la compagnie doit une DSN, quand on le sait.
///
/// UNE COMPAGNIE NEUVE N'EST EN RETARD DE RIEN. Creee depuis
/// l'accueil un 30 septembre, elle s'ouvrait sur << Deposer la DSN d'aout 2026
/// -- en retard >>. Le premier mois du, c'est celui de sa premiere embauche ;
/// sans cette date, une compagnie qui n'a ni salarie, ni paie saisie, ni DSN au
/// journal n'a encore rien a declarer (le mois courant). `None` : rien ne borne
/// -- une compagnie qui a deja des salaries, des paies ou des declarations.
pub(crate) fn premier_mois_declarable(st: &Etat) -> Option<String> {
    let embauche: String = texte_ou_vide(&st.ctx.entreprise, "premiere_embauche").chars().take(7).collect();
    if !embauche.is_empty() {
        return Some(embauche);
    }
    // Une compagnie qui a deja des engagements a deja declare.
    if !engagement::stock::mois_saisis(st.racine()).unwrap_or_default().is_empty() {
        return None;
    }
    // DES FICHES NE SONT PAS UN PASSE : une compagnie neuve remplit ses salaries avant sa premiere paie ;
    // sans paie ni DSN, elle n'a aucun mois d'avant a declarer (elle reclamait la DSN du mois precedent sa naissance).
    if !st.journal.is_empty() && st.journal.iter().any(|l| l.get("quoi").and_then(Value::as_str) == Some(DSN)) {
        return None;
    }
    Some(mois_de(&st.aujourdhui))
}

/// Les mois dont la DSN manque -- ou a ete refusee --, du plus ancien au plus
/// recent : depuis le mois qui suit la derniere DSN passee jusqu'au mois d'avant
/// aujourd'hui. Sans aucune DSN au journal : le seul mois d'avant -- on ne va pas
/// reclamer cinq ans d'histoire a une compagnie qui commence.
pub(crate) fn mois_a_declarer(st: &Etat) -> R<Vec<String>> {
    let courant = mois_de(&st.aujourdhui);
    let dernier = mois_precedent(&courant);
    let depuis = derniere_dsn_passee(st)?;
    let mut premier = depuis.as_ref().map_or_else(|| dernier.clone(), |d| mois_suivant(d));
    if let (Some(borne), None) = (premier_mois_declarable(st), &depuis) {
        premier = premier.max(borne);
    }
    if premier > dernier {
        return Ok(Vec::new());
    }
    let (mut out, mut m) = (Vec::new(), premier);
    while m <= dernier {
        let e = etat_dsn::du_mois(st.ctx, &m, &Value::Null)?;
        if e.a_faire() {
            out.push(m.clone());
        }
        m = mois_suivant(&m);
    }
    Ok(out)
}

/// `fichiers_deposables(ctx, mois)["dsn"]` : `(nom,
/// chemin)` de la DSN mensuelle du dossier du mois, dans l'ordre de `crate::routes::etapes::ordre_de_depot`.
fn dsn_deposable(st: &Etat, mois: &str) -> Option<(String, String)> {
    // Le mois n'est ecrit que si le moteur a note sa DSN mensuelle (`depart::mois_ecrit`, le critere de l'horloge
    // et de l'accueil) : un fichier DSN qu'il n'a pas note ne fait pas « le mois est ecrit ».
    if !crate::compagnie::depart::mois_ecrit(st.racine(), mois) {
        return None;
    }
    let dossier = st.racine().join("sorties").join(mois);
    let mut fichiers: Vec<(String, PathBuf)> = entrees_dans_l_ordre(&dossier)
        .into_iter()
        .filter(|(nom, c)| c.is_file() && Path::new(nom).extension().map_or(false, |e| e.to_string_lossy().to_lowercase() == "dsn"))
        // LA MENSUELLE, PAS UN SIGNALEMENT : un signalement d'arret ou de fin ecrit apres la DSN du mois, dans
        // le meme dossier, devenait « la DSN a deposer » du rappel.
        .filter(|(_, c)| std::fs::read(c).map(|o| !envois::depot::lire(&o).est_signalement()).unwrap_or(false))
        .collect();
    fichiers.sort_by_cached_key(|(nom, c)| crate::routes::etapes::ordre_de_depot(nom, c));
    fichiers.into_iter().next().map(|(nom, c)| (nom, c.to_string_lossy().into_owned()))
}

/// Les entrees d'un dossier dans l'ordre du systeme (`iterdir`), pour que deux
/// fichiers de meme date se departagent toujours de la meme facon.
fn entrees_dans_l_ordre(dossier: &Path) -> Vec<(String, PathBuf)> {
    match std::fs::read_dir(dossier) {
        Ok(l) => l.filter_map(|e| e.ok()).map(|e| (e.file_name().to_string_lossy().into_owned(), e.path())).collect(),
        Err(_) => Vec::new(),
    }
}

/// Combien d'engagements le mois montre, et combien sont valides. Muette en cas de pepin : un mois illisible rend
/// `(0, 0)`.
fn engagements_valides(ctx: &Contexte, mois: &str) -> (i64, i64) {
    let Ok(valides) = engagement::etats::valides_du_mois(ctx, mois) else { return (0, 0) };
    (valides.len() as i64, valides.iter().filter(|v| **v).count() as i64)
}

fn rappel_dsn(st: &Etat, mois: &str) -> R<Rappel> {
    let lib = socle::date::libelle_du_mois(mois, socle::date::Graphie::Minuscule).map_err(Refus::from)?;
    let mut r = Rappel::nouveau(
        DSN,
        format!("dsn:{}", mois),
        format!("Il est temps d'envoyer la DSN de {}", lib),
        format!("Avez-vous bien fini les paies de {} ?", lib),
    );
    r.mois = Some(mois.to_string());
    // L'echeance : `echeance_dsn`, le 5 ou le 15 du mois suivant selon l'effectif, reporte au jour ouvre suivant un
    // samedi, un dimanche ou un jour ferie (R. 133-14, I), a midi. Passee, ce n'est plus un rappel, c'est un retard --
    // et on le dit.
    let echeance = declarations::echeance_dsn::de_la_compagnie(st.ctx, mois)?;
    r.en_retard = echeance.passee_au(&st.aujourdhui);
    r.echeance_depot = Some(echeance.en_json());

    let (engagements, validees) = engagements_valides(st.ctx, mois);
    if !engagement::stock::a_une_saisie(&st.ctx.racine, mois).unwrap_or(false) {
        r.etat = A_SAISIR.into();
        r.detail = format!(
            "Aucun engagement saisi pour {}. S'il n'y a eu aucun cachet, une DSN « néant » se dépose quand même : le mois s'écrit vide, puis se dépose.",
            lib
        );
        return Ok(r);
    }
    r.engagements = engagements;
    r.validees = validees;

    let etat = etat_dsn::du_mois(st.ctx, mois, &Value::Null)?;
    if etat.refusee() {
        r.etat = A_CORRIGER.into();
        let anomalies: Vec<String> = etat.0.get("anomalies").and_then(Value::as_array).map(|a| a.iter().map(en_texte).collect()).unwrap_or_default();
        r.detail = format!(
            "La DSN de {} a été REFUSÉE : {}. À corriger et à renvoyer.",
            lib,
            if anomalies.is_empty() { "motif non lu".to_string() } else { anomalies.join(" ; ") }
        );
        return Ok(r);
    }

    let s = |n: i64| if n == 1 { "" } else { "s" };
    if let Some((nom, chemin)) = dsn_deposable(st, mois) {
        r.etat = A_DEPOSER.into();
        r.detail = format!(
            "{} engagement{}, {} validé{}. Le mois est écrit ({}) ; la DSN n'est pas partie.",
            r.engagements,
            s(r.engagements),
            r.validees,
            s(r.validees),
            nom
        );
        r.fichier = Some(chemin);
    } else {
        r.etat = A_PRODUIRE.into();
        r.detail = format!(
            "{} engagement{} saisi{}, {} validé{}. Le mois n'est pas encore écrit : bulletins, DSN et le reste s'écrivent d'abord, puis la DSN se dépose.",
            r.engagements,
            s(r.engagements),
            s(r.engagements),
            r.validees,
            s(r.validees)
        );
    }
    if r.engagements != 0 && r.validees < r.engagements {
        let reste = r.engagements - r.validees;
        r.detail.push_str(&format!(" Il reste {} engagement{} non validé{}.", reste, s(reste), s(reste)));
    }
    ajouter_les_mois_de_l_ancien_logiciel(st, mois, &mut r)?;
    Ok(r)
}

/// Ce que la DSN de `mois` declare en plus pour les mois dont l'ancien logiciel a depose la DSN, dit comme la page Paie
/// le dit (`import::mois::dsn_de_l_ancien_logiciel`).
fn ajouter_les_mois_de_l_ancien_logiciel(st: &Etat, mois: &str, r: &mut Rappel) -> R<()> {
    for m in engagement::stock::mois_saisis(st.racine()).unwrap_or_default().into_iter().filter(|m| m.as_str() < mois) {
        let Some(d) = crate::import::mois::dsn_de_l_ancien_logiciel(st.ctx, &m)? else { continue };
        if d["porteur"] == json!(mois) && d["engagements"].as_u64().is_some_and(|n| n > 0) {
            let lib = socle::date::libelle_du_mois(&m, socle::date::Graphie::Accentuee).map_err(Refus::from)?;
            r.detail.push_str(&format!(" {} : {}", socle::texte::capitaliser(&lib), en_texte(&d["phrase"])));
        }
    }
    Ok(())
}

// ------------------------------------------------------ les obligations dues

/// CE QUI EST DU, A L'OUVERTURE : toute obligation que `obligations.rs` dit a faire ou en retard -- la
/// fin d'un contrat qui se signale, et ce qu'un arret fait naitre (le signalement d'arret ou de reprise anticipee, la
/// declaration d'accident du travail ou de trajet sous quarante-huit heures, l'attestation de salaire du deuil d'enfant).
/// UN SEUL MECANISME : chaque obligation devient un rappel par `rappel_d_obligation`, qui porte l'obligation telle que le
/// moteur l'a calculee (sa date butoir, son etat, sa phrase, ses textes) et ce sur quoi elle porte (`objet` : la fin de
/// contrat de la route `fins-de-contrat`, ou l'arret resume comme la route `imprevus` le resume) -- la page en tire le meme
/// geste que dans la colonne du mois. Le plus presse d'abord : en retard, puis par date butoir.
///
/// Ils se taisent des que la chose est faite -- le depot REEL du signalement note au journal, la declaration ou
/// l'attestation dite faite (`accident-declare`, `attestation-declare`) --, jamais parce qu'on a clique « plus tard ».
/// Ce qui n'est pas encore du (`a_venir`), ce qui est a la charge du salarie (la maladie professionnelle) et ce dont le
/// delai ne se compte pas (`indetermine` : il n'a pas de date butoir ; la colonne du mois le montre) ne s'y rappellent pas.
fn rappels_obligations(st: &Etat) -> R<Vec<Rappel>> {
    let mut out = Vec::new();

    // LES FINS DE CONTRAT : celles du mois passe et du mois en cours, deja finies (`crate::routes::imprevus::fins_a_signaler`, la
    // meme liste que la colonne du mois), et celles d'avant dont le signalement est en retard (`fins_en_retard_avant`). Un CDD
    // d'usage du spectacle arrive a son terme (031) n'en fait pas partie.
    let mois = mois_de(&st.aujourdhui);
    let mut fins = crate::routes::imprevus::fins_en_retard_avant(st.ctx, &mois_precedent(&mois), st.aujourdhui)?;
    for m in [mois_precedent(&mois), mois.clone()] {
        fins.extend(crate::routes::imprevus::fins_a_signaler(st.ctx, &m, st.aujourdhui)?);
    }
    for f in fins {
        let m = en_texte(&f["mois"]);
        let fin = Jour::lire(&en_texte(&f["fin"]))?;
        if fin > st.aujourdhui {
            continue; // il n'a pas encore pris fin
        }
        let o = &f["obligation"];
        if !du(o) {
            continue;
        }
        let salarie = en_texte(&f["salarie"]);
        let a_saisir: Vec<String> = f["a_saisir"].as_array().map(|l| l.iter().map(crate::routes::imprevus::phrase_du_manque).collect()).unwrap_or_default();
        let mut r = rappel_d_obligation(
            o,
            format!("fin-contrat:{}:{}", salarie, en_texte(&f["fin"])),
            format!("Signaler la fin du contrat de {}", salarie),
            format!("Le contrat {} s'est terminé le {}.", texte(f.get("numero")), fin.jj_mm_aaaa()),
            f.clone(),
        );
        r.mois = Some(m.clone());
        r.salarie = Some(salarie.clone());
        r.etat = if a_saisir.is_empty() { A_PRODUIRE } else { A_SAISIR }.into();
        if !a_saisir.is_empty() {
            r.detail.push_str(&format!(" À saisir d'abord, dans la carte du contrat : {}", a_saisir.join(" ")));
        }
        r.a_saisir = a_saisir;
        out.push(r);
    }

    // CE QU'UN ARRET FAIT NAITRE : tous les arrets du magasin des imprevus (`crate::routes::imprevus::tous_les_arrets`), leurs
    // obligations comptees par `obligations::de_l_arret` au jour du rappel.
    let arrets = crate::routes::imprevus::tous_les_arrets(st.ctx)?;
    if !arrets.is_empty() {
        let n = crate::routes::imprevus::nomenclatures(st.ctx)?;
        let suivi = crate::obligations::Suivi::lire(st.ctx)?;
        for a in &arrets {
            let obligations = crate::obligations::de_l_arret(st.ctx, a, &suivi, st.aujourdhui)?;
            if !obligations.iter().any(du) {
                continue;
            }
            let mut resume = crate::routes::imprevus::resume(st.ctx, &n, a)?;
            resume["obligations"] = Value::Array(obligations.clone());
            let salarie = st.ctx.nomme(&en_texte(&a["salarie"]));
            for o in obligations.iter().filter(|o| du(o)) {
                let mut r = rappel_d_obligation(
                    o,
                    format!("{}:{}:{}", en_texte(&o["cle"]), en_texte(&a["salarie"]), en_texte(&a["dernier_jour_travaille"])),
                    format!("{} — {}", en_texte(&o["libelle"]), salarie),
                    format!("{}, {}.", arret_en_clair(&resume), periode_de_l_arret(&resume)),
                    resume.clone(),
                );
                r.mois = Some(en_texte(&a["dernier_jour_travaille"]).chars().take(7).collect());
                r.salarie = Some(en_texte(&a["salarie"]));
                out.push(r);
            }
        }
    }

    // Le plus presse d'abord : ce qui est en retard, puis par date butoir (le tri est stable : a butoir egale, l'ordre
    // de lecture reste).
    out.sort_by_key(|r| (!r.en_retard, r.jour.clone().unwrap_or_default()));
    Ok(out)
}

/// Une obligation due : a faire, ou en retard.
fn du(o: &Value) -> bool {
    matches!(o.get("etat").and_then(Value::as_str), Some(crate::obligations::A_FAIRE) | Some(crate::obligations::EN_RETARD))
}

/// Le rappel d'une obligation : son titre, sa question, et ce que le moteur en a calcule -- la date butoir (`jour`), le
/// retard, la phrase -- ; `objet` : ce sur quoi elle porte, que la page repasse au geste qui la fait.
fn rappel_d_obligation(o: &Value, cle: String, titre: String, question: String, objet: Value) -> Rappel {
    let mut r = Rappel::nouveau(OBLIGATION, cle, titre, question);
    r.en_retard = o.get("etat").and_then(Value::as_str) == Some(crate::obligations::EN_RETARD);
    r.etat = en_texte(&o["etat"]);
    r.jour = o.get("butoir").and_then(Value::as_str).map(str::to_string);
    let delai = o.get("delai").and_then(Value::as_str).unwrap_or("");
    let phrase = o.get("phrase").and_then(Value::as_str).unwrap_or("");
    r.detail = if delai.is_empty() { phrase.to_string() } else { format!("Délai : {}. {}", delai, phrase) };
    r.obligation = Some(o.clone());
    r.objet = Some(objet);
    r
}

/// « Arrêt de travail (accident du travail) » : ce qu'est l'arret, avec le libelle de son motif.
fn arret_en_clair(resume: &Value) -> String {
    format!("Arrêt de travail ({})", en_texte(&resume["motif_libelle"]).to_lowercase())
}

/// Sa periode : depuis son premier jour ; jusqu'a la reprise si elle est dite, sinon la fin prescrite.
fn periode_de_l_arret(resume: &Value) -> String {
    let jj = |k: &str| Jour::lire(&en_texte(&resume[k])).map(|j| j.jj_mm_aaaa()).unwrap_or_else(|_| en_texte(&resume[k]));
    if vrai(&resume["reprise"]) {
        format!("du {} au {}", jj("premier_jour"), jj("dernier_jour"))
    } else {
        format!("depuis le {}, prescrit jusqu'au {}", jj("premier_jour"), jj("dernier_jour"))
    }
}

// ------------------------------------------------------------- les DPAE
/// Ce qui a fait echouer un depot de test, en une phrase.
fn pourquoi_rate(ligne: &Value) -> String {
    let mut anomalies: Vec<String> = Vec::new();
    if let Some(Value::Array(retours)) = ligne.get("retours") {
        for r in retours {
            if texte_ou_vide(r, "verdict") == "KO" {
                if let Some(Value::Array(a)) = r.get("anomalies") {
                    for x in a {
                        let t = en_texte(x);
                        if !anomalies.contains(&t) {
                            anomalies.push(t);
                        }
                    }
                }
            }
        }
    }
    if !anomalies.is_empty() {
        return anomalies.join(" ; ");
    }
    let reponse: String = texte_ou_vide(ligne, "reponse").trim().chars().take(200).collect();
    if reponse.is_empty() { "motif non lu".to_string() } else { reponse }
}


/// Les DPAE a envoyer : celles des engagements valides (`etapes::Moteur::embauches_a_declarer`), a toute distance de
/// l'embauche, celles en retard comprises. Une par jour d'embauche, avec le fichier
/// teste s'il y en a un -- le bouton ouvre alors directement son depot. Se tait
/// des que la DPAE reelle est au journal.
fn rappels_dpae(st: &Etat) -> R<Vec<Rappel>> {
    let aujourdhui = st.aujourdhui;
    // L'etat du filet : les tests faits par embauche (`config/filet-dpae.json`).
    let tests: Value = match std::fs::read_to_string(st.racine().join("config").join("filet-dpae.json")) {
        Ok(t) => serde_json::from_str::<Value>(&t).ok().and_then(|v| v.get("tests").cloned()).filter(|t| t.is_object()).unwrap_or_else(|| json!({})),
        Err(_) => json!({}),
    };
    let mut jours: BTreeMap<(String, String), Vec<Value>> = BTreeMap::new();
    let a_faire = crate::routes::etapes::Moteur::nouveau(st.ctx).embauches_a_declarer(None)?;
    for e in a_faire.as_array().into_iter().flatten() {
        jours.entry((texte_ou_vide(e, "mois"), texte_ou_vide(e, "debut"))).or_default().push(e.clone());
    }

    let mut out = Vec::new();
    for ((mois, debut), groupe) in jours {
        let jour = Jour::lire(&debut)?;
        let essais: Vec<Value> = groupe
            .iter()
            .map(|e| {
                let t = tests.get(format!("{}|{}", texte_ou_vide(e, "salarie"), debut)).cloned().unwrap_or(Value::Null);
                if vrai(&t) { t } else { json!({}) }
            })
            .collect();
        let fichiers: BTreeSet<String> = essais.iter().map(|t| texte(t.get("fichier"))).collect();
        let (mut fichier, mut pret) = (None, String::new());
        if fichiers.len() == 1 {
            let f = essais[0].get("fichier").cloned().unwrap_or(Value::Null);
            if vrai(&f) {
                let f = en_texte(&f);
                if Path::new(&f).exists() {
                    let empreinte = envois::journal::empreinte_fichier(Path::new(&f))?;
                    let nom = Path::new(&f).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    let rep = etats::repetition(&st.journal, DPAE, &empreinte, &nom)?;
                    if vrai_opt(rep.get("conforme")) {
                        fichier = Some(f);
                        pret = "La DPAE est prête et son test est passé : il reste à l'envoyer.".to_string();
                    } else if let Some(ratee) = rep.get("ratees").and_then(Value::as_array).and_then(|r| r.last()) {
                        pret = format!("Le test automatique a été refusé : {}. À corriger avant l'envoi.", pourquoi_rate(ratee));
                    }
                }
            }
        }
        if pret.is_empty() {
            let erreurs: Vec<String> = essais.iter().filter(|t| vrai_opt(t.get("erreur"))).map(|t| en_texte(&t["erreur"])).collect();
            pret = match erreurs.first() {
                Some(e) => format!("Le test automatique n'a pas pu se faire : {}.", e),
                None => "Elle n'est pas encore préparée : le bouton ouvre les embauches du mois.".to_string(),
            };
        }
        let retard = jour < aujourdhui;
        let noms = groupe.iter().map(|e| format!("{} ({})", texte_ou_vide(e, "nom"), texte_ou_vide(e, "spectacle"))).collect::<Vec<_>>().join(", ");
        let quand = if retard {
            format!("L'embauche a commencé le {} : la DPAE est EN RETARD, à envoyer tout de suite.", jj_mm_aaaa(&debut))
        } else if jour == aujourdhui {
            "L'embauche est aujourd'hui : la DPAE doit partir avant la première heure de travail.".to_string()
        } else {
            let reste = jour.moins(&aujourdhui);
            format!(
                "Elle doit partir avant la première heure de travail — il reste {} jour{}.",
                reste,
                if reste == 1 { "" } else { "s" }
            )
        };
        let mut r = Rappel::nouveau(
            DPAE,
            format!("dpae:{}", debut),
            format!("DPAE à envoyer — embauche du {}", jj_mm_aaaa(&debut)),
            format!("Pas de trace d'envoi pour : {}.", noms),
        );
        r.mois = Some(mois);
        r.jour = Some(debut);
        r.detail = format!("{} {}", pret, quand);
        r.fichier = fichier;
        r.en_retard = retard;
        r.engagements = groupe.len() as i64;
        out.push(r);
    }
    Ok(out)
}
