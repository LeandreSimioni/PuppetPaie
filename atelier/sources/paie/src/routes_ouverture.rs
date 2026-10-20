//! Les deux routes de l'ouverture : `rappels` et `apercu` (`moteur/web.py`,
//! `moteur/operations_exercice.py`, `moteur/operations_paies.py`).
//!
//! `rappels` : ce que le logiciel propose en ouvrant une compagnie (la DSN des
//! mois passes, ce qui est du -- les fins de contrat qui se signalent et ce qu'un arret fait naitre, de `obligations.rs` --,
//! les DPAE, la cloture de l'exercice,
//! l'export compta, la revue des spectacles). AUCUN CALCUL DE PAIE : le Python
//! relisait la paie de chaque mois pour n'en tirer que le nombre de paies
//! validees (`operations_paies.paies_validees` passait par `charger`, donc par
//! `calculer`) ; ce qu'il lisait vraiment, c'est la saisie -- le couple
//! (salarie, debut) de chaque engagement et son drapeau `valide`. Ici, la saisie
//! se lit par `engagement::engagements` (les engagements construits, sans
//! chiffrer) et le journal par `etats` (les etats du journal, deja en Rust).
//!
//! `apercu` : le mois calcule pour l'ecran. Quand la saisie recue est celle du
//! fichier `paies/AAAA-MM.json` (meme contenu une fois lue, nombre pour nombre),
//! la paie se relit dans `paies/AAAA-MM.calcul.json` par `calcul_du_mois` : elle
//! ne se recalcule que si les entrees ont change (et le mois n'est pas parti).
//! Sinon (saisie en cours de frappe, pas encore enregistree) elle se calcule en
//! memoire, sans rien ecrire. Dans les deux cas, ce qui s'affiche se refait de
//! la reponse du calcul : les engagements se reconstruisent depuis la saisie
//! (sans calcul), les paies depuis le JSON, les numeros se reservent comme
//! `engagement.numeroter` (sans rien ecrire).
//!
//! Un refus porte sa `classe` (celle de l'exception Python d'avant) : voir
//! `encoder` et `decoder`, que `routes.rs` utilise.

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::calcul::{DeductionForfaitaire, LigneBrut, LigneCotisation, Paie, ReductionGenerale};
use crate::contexte::{archivee, telle_qu_au, Contexte};
use crate::date::{en_texte, Jour};
use crate::emplois::{emploi_de, EmploiEngagement};
use crate::engagement::{self, Avis, Engagement};
use crate::etats;
use crate::fichiers::{lire_json, vrai};
use crate::num::{dec, flottant, Decimal, Resultat};
use crate::retours::Echec;

// ------------------------------------------------------------------ les refus

/// Un refus et, quand Python l'aurait leve sous un nom, la classe de
/// l'exception (`SaisieIncomplete`, `KeyError`...).
#[derive(Debug)]
pub(crate) struct Refus {
    pub(crate) classe: Option<&'static str>,
    pub(crate) message: String,
}

pub(crate) type R<T> = Result<T, Refus>;

impl From<String> for Refus {
    fn from(message: String) -> Self {
        Refus { classe: None, message }
    }
}

impl From<&str> for Refus {
    fn from(m: &str) -> Self {
        Refus { classe: None, message: m.to_string() }
    }
}

impl From<Echec> for Refus {
    fn from(e: Echec) -> Self {
        Refus { classe: e.classe, message: e.message }
    }
}

impl From<Refus> for Echec {
    fn from(r: Refus) -> Self {
        Echec { classe: r.classe, message: r.message }
    }
}

fn avec_classe(classe: &'static str, message: impl Into<String>) -> Refus {
    Refus { classe: Some(classe), message: message.into() }
}

const SEPARATEUR: char = '\u{1}';

impl Refus {
    /// Le refus en texte : `\u{1}classe\u{1}message` quand il a une classe, le
    /// message seul sinon (le `Resultat<Value>` des routes ne porte qu'un texte).
    fn encoder(self) -> String {
        match self.classe {
            Some(c) => format!("{}{}{}{}", SEPARATEUR, c, SEPARATEUR, self.message),
            None => self.message,
        }
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
        "apercu" => apercu(o, charge),
        _ => return None,
    };
    Some(r.map_err(Refus::encoder))
}

// ------------------------------------------------------------------ outils

const MOIS_FR: [&str; 12] = [
    "janvier", "fevrier", "mars", "avril", "mai", "juin", "juillet", "aout", "septembre", "octobre", "novembre", "decembre",
];
pub(crate) const MOIS_TITRE: [&str; 12] = [
    "Janvier", "Fevrier", "Mars", "Avril", "Mai", "Juin", "Juillet", "Aout", "Septembre", "Octobre", "Novembre", "Decembre",
];

/// `str(x)` d'une valeur facultative (`None` si absente).
fn texte(v: Option<&Value>) -> String {
    v.map_or("None".to_string(), en_texte)
}

fn vrai_opt(v: Option<&Value>) -> bool {
    v.map_or(false, vrai)
}

/// `str(x.get(cle) or "")`.
fn texte_ou_vide(v: &Value, cle: &str) -> String {
    match v.get(cle) {
        Some(x) if vrai(x) => en_texte(x),
        _ => String::new(),
    }
}

/// `Path.stem`.
fn radical(nom: &str) -> String {
    match nom.rfind('.') {
        Some(i) if i > 0 && i < nom.len() - 1 => nom[..i].to_string(),
        _ => nom.to_string(),
    }
}

/// Les entrees d'un dossier dont le nom finit par `.json` (`glob("*.json")`,
/// qui prend aussi les `.calcul.json`).
fn entrees(dossier: &Path) -> Vec<(String, PathBuf)> {
    let mut v: Vec<(String, PathBuf)> = match std::fs::read_dir(dossier) {
        Ok(l) => l
            .filter_map(|e| e.ok())
            .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
            .collect(),
        Err(_) => Vec::new(),
    };
    v.sort();
    v
}

fn mtime(p: &Path) -> f64 {
    use std::time::UNIX_EPOCH;
    std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0.0, |d| d.as_secs() as f64 + f64::from(d.subsec_nanos()) * 1e-9)
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

fn libelle(mois: &str) -> String {
    let n: usize = mois[5..7].parse().unwrap_or(1);
    format!("{} {}", MOIS_FR[n.clamp(1, 12) - 1], &mois[..4])
}

fn mois_de(j: &Jour) -> String {
    format!("{:04}-{:02}", j.an, j.mois)
}

/// `str(ctx.entreprise.get("nic") or "")`.
pub(crate) fn nic_principal(ctx: &Contexte) -> String {
    texte_ou_vide(&ctx.entreprise, "nic")
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

/// La fenetre de la DPAE : au plus tot 8 jours avant l'embauche (R. 1221-5) ;
/// une embauche passee sans DPAE se rappelle une semaine.
const FENETRE_JOURS: i64 = 8;
const RETARD_JOURS: i64 = 7;

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
    paies: i64,
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
            paies: 0,
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
            "paies": self.paies, "validees": self.validees,
            "spectacles": self.spectacles.iter().map(brut_valeur).collect::<Vec<_>>(),
            "salarie": opt(&self.salarie), "jour": opt(&self.jour), "a_saisir": self.a_saisir,
            "echeance_depot": self.echeance_depot,
            "obligation": self.obligation, "objet": self.objet.as_ref().map(brut_valeur),
        })
    }
}

/// `sortie_json.brut` d'un JSON : les cles qui commencent par `_` s'en vont.
fn brut_valeur(v: &Value) -> Value {
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
    pub(crate) journal: Vec<Value>,
    pub(crate) principal: String,
}

impl Etat<'_> {
    fn racine(&self) -> &Path {
        &self.ctx.racine
    }

    /// `journal.etat_dsn_du_mois(ctx, mois)` : `Null` ou l'etat.
    /// UN MOIS TENU AVANT CE LOGICIEL (ticket #215) NE SE RECLAME JAMAIS : sans ligne au journal, la DSN que l'import
    /// a lue pour lui (`mois_importe::dsn_importee`) le dit declare ; sans aucune trace, il n'est pas reclame non plus
    /// (`sans_trace`) -- sa DSN, faite ou a faire, releve du logiciel qui a calcule ses paies, et PuppetPaie ne la
    /// refait pas (regle 4). C'est ICI, et seulement ici, que rappels et echeancier l'apprennent : ni rappel, ni
    /// « en retard ». La page Paie et l'Historique, eux, disent « aucune trace » (`mois_importe::dsn_avant`).
    pub(crate) fn etat_dsn(&self, mois: &str) -> R<Value> {
        let e = etats::etat_dsn_du_mois(&self.journal, mois, &Value::Null, &self.principal)?;
        if !e.is_null() {
            return Ok(e);
        }
        if let Some(d) = crate::mois_importe::dsn_importee(self.racine(), mois)? {
            return Ok(d);
        }
        if crate::mois_importe::importe(self.racine(), mois)? {
            return Ok(json!({"importee": true, "sans_trace": true, "passee": false, "refusee": false}));
        }
        Ok(Value::Null)
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
    let st = Etat { ctx: &ctx, aujourdhui, journal: etats::charger(&ctx.racine)?, principal: nic_principal(&ctx) };
    let out = lister(&st)?;
    Ok(json!({ "rappels": out.iter().map(Rappel::json).collect::<Vec<_>>() }))
}

/// `operations_exercice.rappels` : ce que le logiciel doit proposer a
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
        let e = st.etat_dsn(&decembre)?;
        if vrai(&e) && !vrai_opt(e.get("sans_trace")) {
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
        if !export_compta_a_jour(st.ctx, &precedent.to_string()) {
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

/// `operations_socle.exercice_clos` : la cloture de l'annee (ou `None`).
pub(crate) fn exercice_clos(ctx: &Contexte, annee: &str) -> R<Option<Value>> {
    let p = ctx.racine.join("config").join("exercices.json");
    if !p.exists() {
        return Ok(None);
    }
    match lire_json(&p)? {
        Value::Object(o) => Ok(o.iter().find(|(k, _)| !k.starts_with('_') && k.as_str() == &annee[..4.min(annee.len())]).map(|(_, v)| v.clone())),
        autre => Err(avec_classe("AttributeError", format!("config/exercices.json : {}", crate::fichiers::attendu("un objet", crate::etats::nom_type(&autre))))),
    }
}

/// `operations_paies.mois_connus` : `(mois, saisie, dossier)` du plus recent au plus ancien.
fn mois_connus(ctx: &Contexte) -> Vec<(String, Option<PathBuf>, Option<PathBuf>)> {
    let mut vus: BTreeMap<String, (Option<PathBuf>, Option<PathBuf>)> = BTreeMap::new();
    let est_un_mois = |n: &str| {
        let b = n.as_bytes();
        b.len() == 7 && b[4] == b'-' && b.iter().enumerate().all(|(i, c)| i == 4 || c.is_ascii_digit())
    };
    // Les saisies `paies/AAAA-MM.json` seulement : le Python prenait le radical
    // de tout `*.json`, et `2026-09.calcul.json` (§192) y devenait un mois
    // « 2026-09.calcul ». Meme regle que `routes_mois::mois_connus`.
    for (nom, chemin) in entrees(&ctx.racine.join("paies")) {
        if nom.ends_with(".json") && est_un_mois(&radical(&nom)) {
            vus.entry(radical(&nom)).or_default().0 = Some(chemin);
        }
    }
    for (nom, chemin) in entrees(&ctx.racine.join("sorties")) {
        // Les dossiers AAAA-MM seulement : `sorties/exercices/` n'est pas un mois.
        if chemin.is_dir() && est_un_mois(&nom) {
            vus.entry(nom).or_default().1 = Some(chemin);
        }
    }
    vus.into_iter().rev().map(|(m, (s, d))| (m, s, d)).collect()
}

/// `operations_exercice.mois_de_l_annee` : les mois de l'annee connus du dossier.
pub(crate) fn mois_de_l_annee(ctx: &Contexte, annee: &str) -> Vec<String> {
    let mut v: Vec<String> = mois_connus(ctx).into_iter().map(|m| m.0).filter(|m| m.starts_with(annee)).collect();
    v.sort();
    v
}

/// `_export_compta_a_jour` : l'export existe et il est plus recent que la
/// derniere saisie de l'annee. Un export fait en septembre ne dit rien
/// d'octobre : en janvier, on le redemande.
fn export_compta_a_jour(ctx: &Contexte, annee: &str) -> bool {
    let p = ctx.racine.join("sorties").join("exercices").join(format!("Compta_{}.csv", annee));
    if !p.exists() {
        return false;
    }
    let prefixe = format!("{}-", annee);
    let derniere = entrees(&ctx.racine.join("paies"))
        .into_iter()
        // Les saisies seulement : la paie calculee (`.calcul.json`) se reecrit a
        // l'ouverture, et l'export paraitrait perime a chaque fois.
        .filter(|(nom, _)| nom.starts_with(&prefixe) && nom.ends_with(".json") && !nom.ends_with(".calcul.json"))
        .map(|(_, c)| mtime(&c))
        .fold(0.0_f64, f64::max);
    mtime(&p) >= derniere
}

/// `spectacles_a_revoir` : les spectacles actifs, tant que la revue de l'annee
/// n'est pas faite.
fn spectacles_a_revoir(ctx: &Contexte, annee: i32) -> R<Vec<Value>> {
    let p = ctx.racine.join("config").join("echeancier.json");
    if p.exists() {
        let v = lire_json(&p)?;
        let cle = format!("{}:{}", REVUE_SPECTACLES, annee);
        if let Some(Value::Object(fait)) = v.get("fait") {
            if fait.contains_key(&cle) {
                return Ok(Vec::new());
            }
        }
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
        let e = st.etat_dsn(&m)?;
        if vrai(&e) && !vrai_opt(e.get("refusee")) {
            passes.push(m);
        }
    }
    Ok(passes.into_iter().max())
}

/// Le premier mois dont la compagnie doit une DSN, quand on le sait.
///
/// UNE COMPAGNIE NEUVE N'EST EN RETARD DE RIEN (30/09/2026). Creee depuis
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
    let paies = st.racine().join("paies");
    if paies.is_dir() && entrees(&paies).iter().any(|(n, _)| n.ends_with(".json")) {
        return None;
    }
    // DES FICHES NE SONT PAS UN PASSE (09/10/2026) : une compagnie neuve remplit ses salaries avant sa premiere paie ;
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
        let e = st.etat_dsn(&m)?;
        if !vrai(&e) || vrai_opt(e.get("refusee")) {
            out.push(m.clone());
        }
        m = mois_suivant(&m);
    }
    Ok(out)
}

/// `operations_declarations.fichiers_deposables(ctx, mois)["dsn"][0]` : `(nom,
/// chemin)` de la DSN la plus recemment ecrite du dossier du mois.
fn dsn_deposable(st: &Etat, mois: &str) -> Option<(String, String)> {
    let dossier = st.racine().join("sorties").join(mois);
    if !dossier.is_dir() {
        return None;
    }
    let mut fichiers: Vec<(String, PathBuf)> = entrees_dans_l_ordre(&dossier)
        .into_iter()
        .filter(|(nom, c)| c.is_file() && Path::new(nom).extension().map_or(false, |e| e.to_string_lossy().to_lowercase() == "dsn"))
        .collect();
    fichiers.sort_by(|a, b| mtime(&b.1).partial_cmp(&mtime(&a.1)).unwrap_or(std::cmp::Ordering::Equal));
    fichiers.into_iter().next().map(|(nom, c)| (nom, c.to_string_lossy().into_owned()))
}

/// Les entrees d'un dossier dans l'ordre du systeme (`iterdir`), pour que deux
/// fichiers de meme date se departagent comme en Python.
fn entrees_dans_l_ordre(dossier: &Path) -> Vec<(String, PathBuf)> {
    match std::fs::read_dir(dossier) {
        Ok(l) => l.filter_map(|e| e.ok()).map(|e| (e.file_name().to_string_lossy().into_owned(), e.path())).collect(),
        Err(_) => Vec::new(),
    }
}

/// Le nombre de paies validees d'un mois, lu de la saisie enregistree
/// (`operations_paies.paies_validees`).
///
/// Le Python passait par `charger`, donc par le calcul de toutes les paies du
/// mois, pour obtenir la cle du salarie et le premier jour de chaque
/// engagement : c'est ce que les engagements construits donnent sans rien
/// chiffrer. Muette en cas de pepin, comme avant : un mois absent, un fichier
/// illisible ou une saisie refusee rendent 0 -- l'ecran affichera des paies non
/// validees, ce qui est le defaut sur. (Un mois dont la saisie se lit mais dont
/// le CALCUL refuserait n'est plus vu comme vide : c'est la seule difference.)
fn paies_validees(ctx: &Contexte, mois: &str) -> i64 {
    let chemin = ctx.racine.join("paies").join(format!("{}.json", mois));
    if !chemin.exists() {
        return 0;
    }
    let Ok(data) = lire_json(&chemin) else { return 0 };
    let _derivation = crate::conges::Derivation::ouvrir();
    let Ok((engs, _)) = engagement::engagements(ctx, &data, &crate::conges::lignes_du_mois) else { return 0 };
    let tous = data.get("engagements").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut cles = BTreeSet::new();
    for (i, e) in engs.iter().enumerate() {
        let Ok(fiche) = ctx.salarie(&e.salarie) else { return 0 };
        if tous.get(i).map_or(false, |x| vrai_opt(x.get("valide"))) {
            cles.insert((texte_ou_vide(&fiche, "_cle"), e.debut.iso()));
        }
    }
    cles.len() as i64
}

fn rappel_dsn(st: &Etat, mois: &str) -> R<Rappel> {
    let lib = libelle(mois);
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
    let echeance = crate::echeance_dsn::de_la_compagnie(st.ctx, mois)?;
    r.en_retard = st.aujourdhui > echeance.jour;
    r.echeance_depot = Some(echeance.en_json());

    let saisie = st.racine().join("paies").join(format!("{}.json", mois));
    if !saisie.exists() {
        r.etat = A_SAISIR.into();
        r.detail = format!(
            "Aucune paie saisie pour {}. S'il n'y a eu aucun cachet, une DSN « néant » se dépose quand même : le mois s'écrit vide, puis se dépose.",
            lib
        );
        return Ok(r);
    }
    r.paies = match lire_json(&saisie) {
        Ok(d) => match d.get("engagements") {
            Some(Value::Array(a)) => a.len() as i64,
            _ => 0,
        },
        Err(_) => 0,
    };
    r.validees = paies_validees(st.ctx, mois);

    let etat = st.etat_dsn(mois)?;
    if vrai(&etat) && vrai_opt(etat.get("refusee")) {
        r.etat = A_CORRIGER.into();
        let anomalies: Vec<String> = etat.get("anomalies").and_then(Value::as_array).map(|a| a.iter().map(en_texte).collect()).unwrap_or_default();
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
            "{} paie{}, {} validée{}. Le mois est écrit ({}) ; la DSN n'est pas partie.",
            r.paies,
            s(r.paies),
            r.validees,
            s(r.validees),
            nom
        );
        r.fichier = Some(chemin);
    } else {
        r.etat = A_PRODUIRE.into();
        r.detail = format!(
            "{} paie{} saisie{}, {} validée{}. Le mois n'est pas encore écrit : bulletins, DSN et le reste s'écrivent d'abord, puis la DSN se dépose.",
            r.paies,
            s(r.paies),
            s(r.paies),
            r.validees,
            s(r.validees)
        );
    }
    if r.paies != 0 && r.validees < r.paies {
        let reste = r.paies - r.validees;
        r.detail.push_str(&format!(" Il reste {} paie{} non validée{}.", reste, s(reste), s(reste)));
    }
    Ok(r)
}

// ------------------------------------------------------ les obligations dues

/// CE QUI EST DU, A L'OUVERTURE (20/10/2026, #205) : toute obligation que `obligations.rs` dit a faire ou en retard -- la
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

    // LES FINS DE CONTRAT : celles du mois passe et du mois en cours, deja finies (`routes_listes::fins_a_signaler`, la
    // meme liste que la colonne du mois). Un CDD d'usage du spectacle arrive a son terme (031) n'en fait pas partie.
    let mois = mois_de(&st.aujourdhui);
    for m in [mois_precedent(&mois), mois.clone()] {
        for f in crate::routes_listes::fins_a_signaler(st.ctx, &m, st.aujourdhui)? {
            let fin = Jour::lire(&en_texte(&f["fin"]))?;
            if fin > st.aujourdhui {
                continue; // il n'a pas encore pris fin
            }
            let o = &f["obligation"];
            if !du(o) {
                continue;
            }
            let salarie = en_texte(&f["salarie"]);
            let a_saisir: Vec<String> = f["a_saisir"].as_array().map(|l| l.iter().map(en_texte).collect()).unwrap_or_default();
            let mut r = rappel_d_obligation(
                o,
                format!("fin-contrat:{}:{}", salarie, en_texte(&f["fin"])),
                format!("Signaler la fin du contrat de {}", salarie),
                format!("Le contrat {} s'est terminé le {}.", texte(f.get("numero")), en_texte(&f["fin"])),
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
    }

    // CE QU'UN ARRET FAIT NAITRE : tous les arrets du magasin des imprevus (`routes_listes::tous_les_arrets`), leurs
    // obligations comptees par `obligations::de_l_arret` au jour du rappel.
    let arrets = crate::routes_listes::tous_les_arrets(st.ctx)?;
    if !arrets.is_empty() {
        let n = crate::routes_listes::nomenclatures(st.ctx)?;
        let suivi = crate::obligations::Suivi::lire(st.ctx)?;
        for a in &arrets {
            let obligations = crate::obligations::de_l_arret(st.ctx, a, &suivi, st.aujourdhui)?;
            if !obligations.iter().any(du) {
                continue;
            }
            let mut resume = crate::routes_imprevus::resume(st.ctx, &n, a)?;
            resume["obligations"] = Value::Array(obligations.clone());
            let salarie = nom_affiche(st.ctx, &en_texte(&a["salarie"]));
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

/// Le nom d'un salarie tel que les listes le montrent (`routes_accueil` : « NOM Prenom ») ; sa cle a defaut.
fn nom_affiche(ctx: &Contexte, cle: &str) -> String {
    let Ok(f) = ctx.salarie(cle) else { return cle.to_string() };
    let prenom = f.get("prenom_affichage").filter(|p| vrai(p)).or(f.get("prenom")).map(en_texte).unwrap_or_default();
    match f.get("nom").map(en_texte) {
        Some(nom) if !nom.is_empty() => format!("{} {}", nom, prenom).trim().to_string(),
        _ => cle.to_string(),
    }
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

/// Une embauche sans trace de DPAE : ce qu'un rappel en dit.
pub(crate) struct Embauche {
    pub(crate) mois: String,
    pub(crate) salarie: String,
    pub(crate) debut: String,
    pub(crate) fin: String,
    pub(crate) nom: String,
    pub(crate) spectacle: String,
    /// `operations_filet._empreinte` : ce que la DPAE de l'embauche declare.
    pub(crate) empreinte: String,
}

/// `operations_filet._empreinte` : ce que la DPAE d'une embauche declare -- si
/// cela change, le test fait ne vaut plus. L'identite du salarie, les bornes du
/// contrat, le spectacle ; pas les taux ni les cachets. `sha256(repr(tuple))[:16]`.
fn empreinte_de_l_embauche(e: &Engagement, sal: &Value) -> String {
    use sha2::{Digest, Sha256};
    let ident: Vec<String> = ["nir", "nom", "prenom", "date_naissance", "lieu_naissance", "departement_naissance", "sexe", "nationalite"]
        .iter()
        .map(|k| crate::fichiers::repr(&texte_ou_vide(sal, k)))
        .collect();
    let fin = e.fin.map_or("None".to_string(), |f| f.iso());
    let spectacle = e.spectacle.clone().unwrap_or_else(|| "None".to_string());
    let brut = format!(
        "({}, {}, {}, ({}), {})",
        crate::fichiers::repr(&e.debut.iso()),
        crate::fichiers::repr(&fin),
        crate::fichiers::repr(&spectacle),
        ident.join(", "),
        crate::fichiers::repr(e.nature.as_deref().unwrap_or(""))
    );
    let h = Sha256::digest(brut.as_bytes());
    h.iter().map(|o| format!("{:02x}", o)).collect::<String>()[..16].to_string()
}

/// La fiche salarie telle qu'elle vaut POUR CET ENGAGEMENT (`engagement.emploi_de`).
fn fiche_pour(ctx: &Contexte, e: &Engagement) -> Resultat<Value> {
    let fiche = ctx.salarie(&e.salarie)?;
    let tqa = |f: &Value, j: &str| telle_qu_au(f, &Jour::lire(j)?);
    let emploi = EmploiEngagement {
        emploi: e.emploi.clone(),
        emploi_affichage: e.emploi_affichage.clone(),
        pcs_ese: e.pcs_ese.clone(),
        code_emploi_ft: e.code_emploi_ft.clone(),
        statut: e.statut.clone(),
        statut_conventionnel: e.statut_conventionnel.clone(),
        statut_categoriel_rc: e.statut_categoriel_rc.clone(),
    };
    emploi_de(&fiche, Some(&e.debut.iso()), &emploi, &tqa)
}

/// `operations_filet.embauches_suivies` : les embauches sans trace de DPAE,
/// d'une semaine en arriere a l'infini. Chacune dit si son CONTRAT EST PARTI --
/// envoye a la signature, revenu signe, ou ecrit avec le mois (le contrat qu'on
/// imprime). << Sans trace >> et non << non declaree >> : le journal ne connait
/// que ce que CE logiciel a depose.
pub(crate) fn embauches_suivies(st: &Etat) -> R<Vec<(Embauche, bool)>> {
    let depuis = st.aujourdhui.plus_jours(-RETARD_JOURS)?;
    let mois_min = mois_de(&depuis);
    let faites = etats::embauches_par_depot(&st.journal)?;
    let mut demandes = BTreeSet::new();
    for l in lire_journal(&st.journal, "contrat-signature", false)? {
        if vrai_opt(l.get("accepte")) {
            let detail = l.get("detail").filter(|d| vrai(d)).cloned().unwrap_or_else(|| json!({}));
            if let Some(Value::Array(contrats)) = detail.get("contrats") {
                for c in contrats {
                    demandes.insert(format!("{}\u{1f}{}", texte(c.get("salarie")), texte(c.get("debut"))));
                }
            }
        }
    }
    let recus = etats::contrats_recus(&st.journal, etats::Maille::Paire)?;

    let mut out = Vec::new();
    for (mois, saisie, dossier) in mois_connus(st.ctx) {
        let Some(saisie) = saisie else { continue };
        if mois < mois_min {
            continue;
        }
        let ecrits = dossier.as_ref().map_or(false, |d| {
            entrees_dans_l_ordre(d).iter().any(|(n, _)| n.starts_with("Contrats_") && n.ends_with(".pdf"))
        });
        // Une saisie en cours d'ecriture : le passage suivant.
        let Ok(data) = lire_json(&saisie) else { continue };
        let _derivation = crate::conges::Derivation::ouvrir();
        let Ok((engs, _)) = engagement::engagements(st.ctx, &data, &crate::conges::lignes_du_mois) else { continue };
        let mut du_mois = Vec::new();
        let mut illisible = false;
        for e in &engs {
            let (Ok(sal), Ok(nat)) = (fiche_pour(st.ctx, e), e.nat()) else {
                illisible = true;
                break;
            };
            let spectacle = if nat.mensuelle() {
                Value::Null
            } else {
                match e.spectacle.as_deref().map(|s| st.ctx.spectacle(s)) {
                    Some(Ok(s)) => s,
                    _ => {
                        illisible = true;
                        break;
                    }
                }
            };
            du_mois.push((e, sal, spectacle));
        }
        if illisible {
            continue;
        }
        let mois_dit = data.get("mois").map(en_texte).unwrap_or_default();
        for (e, sal, spectacle) in du_mois {
            if !e.premier_mois()? {
                continue; // un contrat en cours : sa DPAE est partie avec son 1er mois
            }
            let cle = texte_ou_vide(&sal, "_cle");
            let debut = e.debut.iso();
            if e.debut < depuis {
                continue;
            }
            let k = [Value::String(cle.clone()), Value::String(debut.clone())];
            if faites.contient(&k)? {
                continue;
            }
            let prenom = match sal.get("prenom_affichage") {
                Some(p) if vrai(p) => en_texte(p),
                _ => sal.get("prenom").map(en_texte).unwrap_or_default(),
            };
            let titre = match spectacle.get("titre") {
                Some(t) if vrai(t) => en_texte(t),
                _ => "salarié au mois".to_string(),
            };
            let parti = ecrits
                || demandes.contains(&format!("{}\u{1f}{}", texte(Some(&k[0])), texte(Some(&k[1]))))
                || recus.contient(&k)?;
            out.push((
                Embauche {
                    mois: mois_dit.clone(),
                    salarie: cle,
                    debut,
                    fin: e.fin.map_or("None".to_string(), |f| f.iso()),
                    nom: format!("{} {}", sal.get("nom").map(en_texte).unwrap_or_default(), prenom),
                    spectacle: titre,
                    empreinte: empreinte_de_l_embauche(e, &sal),
                },
                parti,
            ));
        }
    }
    out.sort_by(|a, b| (&a.0.debut, &a.0.nom).cmp(&(&b.0.debut, &b.0.nom)));
    Ok(out)
}

/// `journal._pourquoi_rate` : ce qui a fait echouer un depot de test, en une phrase.
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

fn jj_mm_aaaa(iso: &str) -> String {
    Jour::lire(iso).map(|j| j.jj_mm_aaaa()).unwrap_or_else(|_| iso.to_string())
}

/// `operations_filet.rappels_dpae` : les DPAE a envoyer maintenant -- dans la
/// fenetre des 8 jours, ou en retard. Une par jour d'embauche, avec le fichier
/// teste s'il y en a un -- le bouton ouvre alors directement son depot. Se tait
/// des que la DPAE reelle est au journal.
fn rappels_dpae(st: &Etat) -> R<Vec<Rappel>> {
    let aujourdhui = st.aujourdhui;
    // L'etat du filet : les tests faits par embauche (`config/filet-dpae.json`).
    let tests: Value = match std::fs::read_to_string(st.racine().join("config").join("filet-dpae.json")) {
        Ok(t) => serde_json::from_str::<Value>(&t).ok().and_then(|v| v.get("tests").cloned()).filter(|t| t.is_object()).unwrap_or_else(|| json!({})),
        Err(_) => json!({}),
    };
    let limite = aujourdhui.plus_jours(FENETRE_JOURS)?.iso();
    let mut jours: BTreeMap<(String, String), Vec<Embauche>> = BTreeMap::new();
    for (e, _) in embauches_suivies(st)? {
        if e.debut <= limite {
            jours.entry((e.mois.clone(), e.debut.clone())).or_default().push(e);
        }
    }

    let mut out = Vec::new();
    for ((mois, debut), groupe) in jours {
        let jour = Jour::lire(&debut)?;
        let essais: Vec<Value> = groupe
            .iter()
            .map(|e| {
                let t = tests.get(format!("{}|{}", e.salarie, e.debut)).cloned().unwrap_or(Value::Null);
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
                    let empreinte = crate::journal::empreinte_fichier(Path::new(&f))?;
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
        let noms = groupe.iter().map(|e| format!("{} ({})", e.nom, e.spectacle)).collect::<Vec<_>>().join(", ");
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
        r.paies = groupe.len() as i64;
        out.push(r);
    }
    Ok(out)
}

// ============================================================== l'apercu

/// Le refus du calcul d'une saisie : le moteur refuse avec sa phrase, mais
/// quand la saisie elle-meme est encore incomplete (un spectacle pas encore
/// choisi, des jours ou des heures pas encore saisis :
/// `engagement::saisie_incomplete`) c'est cette phrase-la, sous son nom
/// (`SaisieIncomplete`), qui monte (`operations_paies.charger`). La page les
/// dit sobrement ; l'ecran enregistre a chaque frappe, donc ce ne sont pas des
/// defauts du logiciel.
pub(crate) fn refus_du_calcul(ctx: &Contexte, data: &Value, message: String) -> Refus {
    if engagement::saisie_incomplete(&message) {
        return avec_classe("SaisieIncomplete", message);
    }
    let vide: crate::engagement::LignesPermanent = &|_, _, _| Ok((Vec::new(), Vec::new()));
    if let Err(m) = engagement::engagements(ctx, data, vide) {
        if engagement::saisie_incomplete(&m) {
            return avec_classe("SaisieIncomplete", m);
        }
    }
    Refus::from(message)
}

/// Une saisie recue : un dictionnaire (l'ecran), ou le chemin d'un fichier de mois.
fn saisie_recue(ctx: &Contexte, charge: &Value) -> R<Value> {
    match charge.get("saisie") {
        None => Err(avec_classe("KeyError", crate::fichiers::absent("saisie"))),
        Some(d) if d.is_object() => Ok(d.clone()),
        Some(Value::String(s)) => {
            let direct = PathBuf::from(s);
            let chemin = if direct.exists() {
                direct
            } else if s.len() == 7 && ctx.racine.join("paies").join(format!("{}.json", s)).exists() {
                ctx.racine.join("paies").join(format!("{}.json", s))
            } else {
                ctx.racine.join(s)
            };
            Ok(engagement::lire_saisie(&chemin).map_err(|m| avec_classe("FileNotFoundError", m))?)
        }
        Some(autre) => Err(avec_classe("TypeError", format!("saisie illisible : {}", autre))),
    }
}

fn mois_valide(m: &str) -> bool {
    let b = m.as_bytes();
    b.len() == 7 && b[4] == b'-' && b.iter().enumerate().all(|(i, c)| i == 4 || c.is_ascii_digit())
}

/// La reponse de `paie calculer` pour cette saisie : relue dans
/// `paies/AAAA-MM.calcul.json` quand la saisie est celle du fichier enregistre
/// (meme contenu, nombre pour nombre), calculee en memoire sinon -- sans rien
/// ecrire. Un mois dont la DSN reelle est partie garde ses chiffres.
pub(crate) fn paie_du_mois(o: &HashMap<String, String>, ctx: &Contexte, data: &Value) -> R<Value> {
    let mois = data.get("mois").and_then(Value::as_str).filter(|m| mois_valide(m));
    if let Some(mois) = mois {
        let fichier = ctx.racine.join("paies").join(format!("{}.json", mois));
        if fichier.is_file() && lire_json(&fichier).map_or(false, |f| &f == data) {
            let journal = etats::charger(&ctx.racine)?;
            let parti = vrai(&etats::etat_dsn_du_mois(&journal, mois, &Value::Null, &nic_principal(ctx))?);
            let demande = json!({ "mois": mois, "parti": parti }).to_string();
            return crate::calcul_du_mois::commande(o, &demande).map_err(|m| refus_du_calcul(ctx, data, m));
        }
    }
    crate::service::calculer_avec(ctx, &json!({ "saisie": data })).map_err(|m| refus_du_calcul(ctx, data, m))
}

fn entier_de(v: &Value) -> Option<i64> {
    match v {
        Value::String(s) => s.trim().parse().ok(),
        Value::Number(n) => n.as_i64(),
        _ => None,
    }
}

/// Les engagements de la saisie, construits sans rien chiffrer, puis ce que le
/// calcul a pose dessus (`paie_rust.completer_engagement` : numero repris du
/// premier mois, jours du contrat, suspensions).
pub(crate) fn engagements_calcules(ctx: &Contexte, data: &Value, rep: &Value) -> R<Vec<Engagement>> {
    let paies = rep.get("paies").and_then(Value::as_array).ok_or("reponse du calcul illisible : `paies`")?;
    let tous = data.get("engagements").and_then(Value::as_array).ok_or_else(|| crate::fichiers::absent("engagements"))?;
    // Les lignes d'un permanent sont celles que le moteur a deduites.
    let mensuels: Vec<usize> = tous
        .iter()
        .enumerate()
        .filter(|(_, e)| crate::contrats::nature(e.get("nature").and_then(Value::as_str)).map_or(false, |n| n.mensuelle()))
        .map(|(i, _)| i)
        .collect();
    let appel = Cell::new(0usize);
    let lignes = |_: &Contexte, _: &Value, _: &str| -> Resultat<(Vec<Value>, Vec<Avis>)> {
        let rang = *mensuels.get(appel.get()).ok_or("rang d'engagement inconnu")?;
        appel.set(appel.get() + 1);
        let l = paies.get(rang).and_then(|p| p.get("engagement")).and_then(|e| e.get("lignes")).and_then(Value::as_array);
        Ok((l.cloned().ok_or("reponse du calcul illisible : `lignes`")?, Vec::new()))
    };
    let (mut engs, _) = engagement::engagements(ctx, data, &lignes)?;
    if engs.len() != paies.len() {
        return Err(Refus::from("reponse du calcul illisible : le nombre de paies"));
    }
    for (e, p) in engs.iter_mut().zip(paies) {
        let v = p.get("engagement").ok_or("reponse du calcul illisible : `engagement`")?;
        e.numero = v.get("numero").filter(|n| !n.is_null()).map(en_texte);
        e.jours_contrat = v.get("jours_contrat").and_then(entier_de).ok_or("reponse du calcul illisible : `jours_contrat`")?;
        e.suspensions = v.get("suspensions").and_then(Value::as_array).cloned().unwrap_or_default();
    }
    Ok(engs)
}

fn decimal_de(v: &Value, cle: &str) -> Resultat<Decimal> {
    dec(v.get(cle).ok_or_else(|| format!("reponse du calcul illisible : `{}`", cle))?)
}

fn decimal_opt(v: Option<&Value>) -> Resultat<Option<f64>> {
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(x) => flottant(x).map(Some),
    }
}

fn texte_opt(v: &Value, cle: &str) -> Option<String> {
    v.get(cle).filter(|x| !x.is_null()).map(en_texte)
}

fn drapeau(v: &Value, cle: &str) -> bool {
    v.get(cle).map_or(false, vrai)
}

/// `paie_rust.paie` : une `Paie` du moteur, sur l'engagement et les fiches, depuis la reponse du calcul.
pub(crate) fn paie_de_json(ctx: &Contexte, eng: Engagement, v: &Value) -> R<Paie> {
    let sal = fiche_pour(ctx, &eng)?;
    let nat = eng.nat()?;
    let spectacle = if nat.mensuelle() {
        Value::Null
    } else {
        ctx.spectacle(eng.spectacle.as_deref().ok_or_else(|| crate::fichiers::absent("spectacle"))?)?
    };
    let smic = flottant(v.get("smic").ok_or("reponse du calcul illisible : `smic`")?)?;
    let mut p = Paie::nouveau(eng, sal, spectacle, decimal_de(v, "brut")?, decimal_de(v, "base_csg")?, decimal_de(v, "plafond")?, smic, decimal_de(v, "heures")?);
    p.total_pat = decimal_de(v, "total_pat")?;
    p.total_sal = decimal_de(v, "total_sal")?;
    p.net_a_payer = decimal_de(v, "net_a_payer")?;
    // Un calcul stocke avant la vague 1b (celui d'un mois parti, que `calcul_du_mois` garde tel quel) n'a pas
    // `net_social` : le moteur qui l'a fait declarait le net a payer en 58.004 et l'imprimait en net social. C'est ce
    // chiffre-la qui est parti ; on le relit, on ne le recalcule pas.
    p.net_social = match v.get("net_social") {
        Some(_) => decimal_de(v, "net_social")?,
        None => p.net_a_payer,
    };
    p.net_imposable = decimal_de(v, "net_imposable")?;
    p.cout_employeur = decimal_de(v, "cout_employeur")?;
    p.salaire_de_base = decimal_de(v, "salaire_de_base")?;
    p.salaire_retabli = decimal_de(v, "salaire_retabli")?;
    p.assiette_chomage = decimal_de(v, "assiette_chomage")?;
    p.heures_travaillees = decimal_de(v, "heures_travaillees")?;
    p.heures_absence = decimal_de(v, "heures_absence")?;
    p.numero_bulletin = texte_opt(v, "numero_bulletin");
    for l in v.get("lignes").and_then(Value::as_array).map_or(&[][..], |a| &a[..]) {
        p.lignes.push(LigneCotisation {
            id: texte_ou_vide(l, "id"),
            famille: texte_ou_vide(l, "famille"),
            libelle: texte_ou_vide(l, "libelle"),
            base: decimal_de(l, "base")?,
            taux_pat: decimal_opt(l.get("taux_pat"))?,
            montant_pat: decimal_de(l, "montant_pat")?,
            taux_sal: decimal_opt(l.get("taux_sal"))?,
            montant_sal: decimal_de(l, "montant_sal")?,
            masquer_base_et_taux: drapeau(l, "masquer_base_et_taux"),
        });
    }
    for l in v.get("remuneration").and_then(Value::as_array).map_or(&[][..], |a| &a[..]) {
        p.remuneration.push(LigneBrut {
            code: texte_ou_vide(l, "code"),
            libelle: texte_ou_vide(l, "libelle"),
            unite: texte_ou_vide(l, "unite"),
            quantite: decimal_de(l, "quantite")?,
            taux: decimal_de(l, "taux")?,
            montant: decimal_de(l, "montant")?,
            heures_annexes: decimal_de(l, "heures_annexes")?,
            soumis: drapeau(l, "soumis"),
            imposable: drapeau(l, "imposable"),
            conges_spectacles: drapeau(l, "conges_spectacles"),
            salaire_de_base: drapeau(l, "salaire_de_base"),
            retabli: drapeau(l, "retabli"),
            conges_pris: drapeau(l, "conges_pris"),
            dsn_bloc52: texte_opt(l, "dsn_bloc52"),
            arret_de_travail: drapeau(l, "arret_de_travail"),
            dsn_autres_bruts: texte_opt(l, "dsn_autres_bruts"),
        });
    }
    if let Some(Value::Object(b)) = v.get("bases") {
        for (k, x) in b {
            p.bases.insert(k.clone(), dec(x)?);
        }
    }
    if let Some(r) = v.get("reduction_generale").filter(|r| r.is_object()) {
        p.reduction_generale = Some(ReductionGenerale {
            coefficient: decimal_de(r, "coefficient")?,
            montant: decimal_de(r, "montant")?,
            smic_reference: decimal_de(r, "smic_reference")?,
            heures: decimal_de(r, "heures")?,
            urssaf: decimal_de(r, "urssaf")?,
            chomage: decimal_de(r, "chomage")?,
            agirc_arrco: decimal_de(r, "agirc_arrco")?,
        });
    }
    if let Some(d) = v.get("deduction_forfaitaire").filter(|d| d.is_object()) {
        // Un calcul stocke avant le lot R9 n'a ni la profession, ni le montant retenu : la deduction est alors le brut
        // moins le brut abattu de ses bases (ce que la DSN declarait en bloc 54 de type 10).
        let montant = match d.get("montant") {
            Some(m) if !m.is_null() => decimal_de(d, "montant")?,
            _ => p.bases.get("brut_dfs").map_or(Decimal::ZERO, |a| p.brut - *a),
        };
        p.deduction_forfaitaire = Some(DeductionForfaitaire {
            categorie: texte_ou_vide(d, "categorie"),
            profession: texte_ou_vide(d, "profession"),
            taux: decimal_de(d, "taux")?,
            rubriques: d.get("rubriques").and_then(Value::as_array).map_or_else(Vec::new, |a| a.iter().map(en_texte).collect()),
            montant,
            deja: match d.get("deja") {
                Some(m) if !m.is_null() => decimal_de(d, "deja")?,
                _ => Decimal::ZERO,
            },
        });
    }
    p.avertissements = v
        .get("avertissements")
        .and_then(Value::as_array)
        .map_or_else(Vec::new, |a| a.iter().map(|x| Avis { info: drapeau(x, "info"), texte: texte_ou_vide(x, "texte") }).collect());
    Ok(p)
}

/// `engagement.numeroter` : les numeros de contrat (FA1.26.13) et de bulletin
/// (17/2026), RESERVES et pas ecrits : l'apercu ne touche a aucun compteur.
/// Un engagement qui PORTE DEJA son numero de contrat le garde ; les numeros de
/// bulletin se reservent toujours : un bulletin est mensuel.
pub(crate) fn numeroter(ctx: &Contexte, paies: &mut [Paie], annee: i32) -> R<()> {
    numeroter_en_attente(ctx, paies, annee, &mut HashMap::new())
}

/// La meme, avec les numeros deja RESERVES par un appel precedent du meme processus
/// (`Contexte._compteurs_en_attente`, `enregistrer_compteur`) : `(annee, compteur) -> dernier numero`.
pub(crate) fn numeroter_en_attente(ctx: &Contexte, paies: &mut [Paie], annee: i32, attente: &mut HashMap<(String, String), i64>) -> R<()> {
    let p = ctx.racine.join("config").join("compteurs.json");
    let compteurs = if p.exists() { lire_json(&p)? } else { Value::Null };
    let an = annee.to_string();
    let prochain = |attente: &HashMap<(String, String), i64>, cle: &str| -> i64 {
        if let Some(v) = attente.get(&(an.clone(), cle.to_string())) {
            return v + 1;
        }
        compteurs.get(&an).and_then(|c| c.get(cle)).and_then(|v| match v {
            Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
            Value::String(s) => s.trim().parse().ok(),
            _ => None,
        }).unwrap_or(0) + 1
    };
    let mut n_bull = prochain(attente, "bulletin");
    let mut seqs: Vec<(String, i64)> = Vec::new();
    let manque = |s: &Value, k: &str| s.get(k).map(en_texte).ok_or_else(|| avec_classe("KeyError", crate::fichiers::absent(k)));
    for p in paies.iter_mut() {
        let sal = p.salarie.clone();
        let sans_paie = p.engagement.sans_paie();
        let sans_numero = p.engagement.numero.as_deref().map_or(true, str::is_empty);
        if sans_numero {
            let cle = format!("contrat_{}", manque(&sal, "_cle")?);
            let seq = match seqs.iter().find(|(k, _)| *k == cle) {
                Some((_, s)) => s + 1,
                None => prochain(attente, &cle),
            };
            match seqs.iter_mut().find(|(k, _)| *k == cle) {
                Some(e) => e.1 = seq,
                None => seqs.push((cle, seq)),
            }
            p.engagement.numero = Some(format!(
                "{}{}.{}.{:02}",
                manque(&sal, "initiales_contrat")?,
                manque(&sal, "code_salarie")?,
                &an[2.min(an.len())..],
                seq
            ));
        }
        // Un mois sans cachet n'a pas de bulletin : rien n'est paye, aucun
        // numero de bulletin ne se reserve.
        if sans_paie {
            continue;
        }
        p.numero_bulletin = Some(format!("{}/{}", n_bull, annee));
        n_bull += 1;
    }
    attente.insert((an.clone(), "bulletin".to_string()), n_bull - 1);
    for (cle, v) in seqs {
        attente.insert((an.clone(), cle), v);
    }
    Ok(())
}

/// `Decimal.quantize(Decimal("0.01"))` : deux decimales, la moitie au pair.
pub(crate) fn q2(x: Decimal) -> Decimal {
    let mut r = x.round_dp(2);
    r.rescale(2);
    r
}

fn nombre_f(x: f64) -> Value {
    serde_json::Number::from_f64(x).map_or(Value::Null, Value::Number)
}

/// `sortie_json._ligne_saisie` : une ligne de la saisie, avec ce que la rubrique en dit.
fn ligne_saisie(ctx: &Contexte, l: &Value) -> Value {
    let rubrique = l.get("rubrique");
    let infos = || -> Option<(String, String, f64)> {
        let r = crate::catalogue::rubrique(&ctx.racine, l.get("rubrique")?.as_str()?).ok()?;
        let quantite = match l.get("quantite") {
            None | Some(Value::Null) => 0.0,
            Some(q) if !vrai(q) => 0.0,
            Some(q) => flottant(q).ok()?,
        };
        let saisies = match l.get("heures") {
            None | Some(Value::Null) => None,
            Some(Value::Number(n)) => n.as_f64(),
            Some(_) => return None,
        };
        let total = r.heures(quantite, saisies);
        Some((r.libelle, r.unite, total))
    };
    let (libelle, unite, total) = match infos() {
        Some((l, u, t)) => (Value::String(l), Value::String(u), Some(t)),
        None => (rubrique.cloned().unwrap_or(Value::Null), Value::Null, None),
    };
    let jours = |cle: &str| -> Value {
        Value::Array(l.get(cle).and_then(Value::as_array).map_or_else(Vec::new, |a| a.iter().map(brut_valeur).collect()))
    };
    json!({
        "rubrique": rubrique.cloned().unwrap_or(Value::Null),
        "libelle": libelle, "unite": unite,
        "quantite": l.get("quantite").cloned().unwrap_or(Value::Null),
        "taux": l.get("taux").cloned().unwrap_or(Value::Null),
        "heures_par_unite": l.get("heures").cloned().unwrap_or(Value::Null),
        "heures": total.filter(|t| *t != 0.0).map_or(Value::Null, nombre_f),
        "lieu": match l.get("lieu") { Some(x) if vrai(x) => x.clone(), _ => Value::String(String::new()) },
        "dates": jours("dates"),
        "planning": jours("planning"),
    })
}

/// `sortie_json.paie` : une paie pour l'ecran. `brut` : l'engagement tel qu'il
/// est ecrit dans la saisie ; `mois` : le mois du fichier (pour `lieu` et
/// `taux_cachet`, que Python garde tels qu'ecrits).
pub(crate) fn vue_paie(ctx: &Contexte, p: &Paie, brut: &Value, data: &Value, avec_lignes: bool) -> R<Value> {
    let e = &p.engagement;
    let nat = e.nat()?;
    let iso = |j: &Jour| Value::String(j.iso());
    let s = &p.salarie;
    let requis = |v: &Value, k: &str| v.get(k).cloned().ok_or_else(|| avec_classe("KeyError", crate::fichiers::absent(k)));
    let sal_vue = json!({
        "cle": s.get("_cle").cloned().unwrap_or(Value::Null),
        "nom": requis(s, "nom")?,
        "prenom": match s.get("prenom_affichage") { Some(x) if vrai(x) => x.clone(), _ => requis(s, "prenom")? },
    });
    let spectacle = if vrai(&p.spectacle) {
        json!({ "titre": requis(&p.spectacle, "titre")?, "numero_objet": p.spectacle.get("numero_objet").cloned().unwrap_or(Value::Null) })
    } else {
        Value::Null
    };
    let mois_du_contrat = e.mois_du_contrat()?;
    // Le taux du cachet, tel que le contrat l'imprime : celui de la premiere
    // ligne a l'unite << cachet >> ; sans cachet, la valeur ecrite, telle quelle,
    // et rien quand rien n'est ecrit (regle 1, lot F1 : plus de 130 EUR).
    let premier_cachet = e
        .lignes
        .iter()
        .find(|l| l.get("rubrique").and_then(Value::as_str).and_then(|c| crate::catalogue::rubrique(&ctx.racine, c).ok()).map_or(false, |r| r.unite == "cachet"));
    let taux_cachet = match premier_cachet {
        Some(l) => nombre_f(flottant(l.get("taux").ok_or_else(|| avec_classe("KeyError", crate::fichiers::absent("taux")))?)?),
        None => brut.get("taux_cachet").or_else(|| data.get("taux_cachet")).cloned().unwrap_or(Value::Null),
    };
    let jours = match brut.get("jours") {
        Some(j) if !j.is_null() => j.clone(),
        _ => Value::from(e.jours),
    };
    let lieu = brut.get("lieu").or_else(|| data.get("lieu")).cloned().unwrap_or_else(|| Value::String(String::new()));
    let avis: Vec<&Avis> = p.avertissements.iter().collect();
    let mut d = json!({
        "numero_bulletin": p.numero_bulletin.clone().map_or(Value::Null, Value::String),
        "numero_contrat": e.numero.clone().map_or(Value::Null, Value::String),
        "salarie": sal_vue,
        // UN PERMANENT N'A PAS DE SPECTACLE, et `None` est la reponse juste.
        "spectacle": spectacle,
        "nature": { "cle": nat.cle, "libelle": nat.libelle, "mensuelle": nat.mensuelle() },
        "debut": iso(&e.debut), "fin": e.fin.as_ref().map_or(Value::Null, iso),
        // Le contrat par rapport au mois : un contrat qui deborde n'a pas de
        // DPAE ni de contrat imprime hors de son premier mois.
        "contrat": {
            "premier_mois": e.premier_mois()?, "dernier_mois": e.dernier_mois()?, "a_cheval": e.a_cheval()?,
            "sans_paie": e.sans_paie(),
            "depuis": mois_du_contrat.first().cloned().ok_or_else(|| crate::fichiers::trop_courte("les mois du contrat"))?,
            "jusqu_a": mois_du_contrat.last().cloned().ok_or_else(|| crate::fichiers::trop_courte("les mois du contrat"))?,
            "periode": { "debut": iso(&e.debut_periode()?), "fin": iso(&e.fin_periode()?) },
        },
        "dates_travail": e.dates_travail.iter().map(iso).collect::<Vec<_>>(),
        "cachets": e.cachets, "taux_cachet": taux_cachet,
        "heures": e.heures.map_or(Value::Null, nombre_f),
        "remuneration": p.remuneration.iter().map(|l| json!({
            "rubrique": l.code, "designation": l.designation(),
            "quantite": l.quantite.to_string(), "taux": l.taux.to_string(), "montant": q2(l.montant).to_string(),
        })).collect::<Vec<_>>(),
        "lignes_saisies": e.lignes.iter().map(|l| ligne_saisie(ctx, l)).collect::<Vec<_>>(),
        "jours": jours, "lieu": lieu,
        "brut": q2(p.brut).to_string(), "base_csg": q2(p.base_csg).to_string(),
        "plafond": q2(p.plafond).to_string(), "smic": nombre_f(p.smic),
        "total_patronal": q2(p.total_pat).to_string(), "total_salarial": q2(p.total_sal).to_string(),
        "net_social": q2(p.net_social).to_string(), "net_a_payer": q2(p.net_a_payer).to_string(),
        "net_imposable": q2(p.net_imposable).to_string(), "cout_employeur": q2(p.cout_employeur).to_string(),
        "avertissements": avis.iter().filter(|a| !a.info).map(|a| a.texte.clone()).collect::<Vec<_>>(),
        "informations": avis.iter().filter(|a| a.info).map(|a| a.texte.clone()).collect::<Vec<_>>(),
        "emploi": {
            "libelle": s.get("emploi").cloned().unwrap_or(Value::Null),
            "code_emploi_ft": s.get("code_emploi_ft").cloned().unwrap_or(Value::Null),
            "statut": s.get("statut").cloned().unwrap_or(Value::Null),
            "cadre": s.get("statut_categoriel_rc").and_then(Value::as_str) == Some("01"),
        },
    });
    if avec_lignes {
        let taux = |t: Option<f64>| t.map_or(Value::Null, nombre_f);
        d["lignes"] = Value::Array(
            p.lignes
                .iter()
                .map(|l| {
                    json!({
                        "id": l.id, "famille": l.famille, "libelle": l.libelle, "base": q2(l.base).to_string(),
                        "taux_pat": taux(l.taux_pat), "montant_pat": q2(l.montant_pat).to_string(),
                        "taux_sal": taux(l.taux_sal), "montant_sal": q2(l.montant_sal).to_string(),
                        "masquer_base_et_taux": l.masquer_base_et_taux,
                    })
                })
                .collect(),
        );
    }
    Ok(d)
}

/// `operations_paies._archives_employees` : ce qui, dans ce mois, porte sur une
/// fiche archivee. On n'INTERDIT pas : rejouer une paie passee, reimprimer un
/// bulletin perdu, cela doit marcher. Mais une paie NEUVE sur une fiche mise de
/// cote est presque toujours une erreur, et elle ne se verrait nulle part ailleurs.
pub(crate) fn archives_employees(paies: &[Paie]) -> R<Vec<String>> {
    let mut out = Vec::new();
    for p in paies {
        if archivee(&p.salarie) {
            let nom = p.salarie.get("nom").ok_or_else(|| avec_classe("KeyError", crate::fichiers::absent("nom")))?;
            let prenom = match p.salarie.get("prenom_affichage") {
                Some(x) if vrai(x) => x.clone(),
                _ => p.salarie.get("prenom").cloned().ok_or_else(|| avec_classe("KeyError", crate::fichiers::absent("prenom")))?,
            };
            out.push(format!(
                "{} {} : sa fiche est archivee. Pour refaire une paie passee, rien a faire ; pour une nouvelle embauche, la \
                 reactiver (Configuration › Salaries, « Reactiver »).",
                en_texte(nom),
                en_texte(&prenom)
            ));
        }
        // Un permanent n'a pas de spectacle -- le numero d'objet lui est interdit
        // (CCH-11) -- donc rien a verifier de ce cote.
        if vrai(&p.spectacle) && archivee(&p.spectacle) {
            let titre = p.spectacle.get("titre").ok_or_else(|| avec_classe("KeyError", crate::fichiers::absent("titre")))?;
            out.push(format!(
                "Le spectacle « {} » (objet {}) est archive : pour le jouer a nouveau, le reactiver (Configuration › Spectacles).",
                en_texte(titre),
                texte(p.spectacle.get("numero_objet"))
            ));
        }
    }
    Ok(out)
}

/// Le bordereau Urssaf que l'apercu montre (celui du PRINCIPAL quand la compagnie a plusieurs etablissements ;
/// chacun des autres a le sien, dans sa DSN), et l'avertissement quand il ne se calcule pas. Partage avec `produire`.
pub(crate) fn urssaf_de_l_apercu(ctx: &Contexte, paies: &[Paie], mois: &str) -> R<(Value, Vec<String>)> {
    // Le bordereau montre est celui du PRINCIPAL quand la compagnie a plusieurs
    // etablissements ; chacun des autres a le sien, dans sa DSN.
    let principal = crate::bareme::nic_principal(&ctx.entreprise);
    let plusieurs = !crate::bareme::secondaires(&ctx.entreprise).is_empty();
    let mut paies_p: Vec<Paie> = Vec::new();
    for p in paies {
        if !plusieurs || crate::bareme::etablissement_du_salarie(&ctx.entreprise, &p.salarie).map_err(|m| avec_classe("ValueError", m))? == principal {
            paies_p.push(p.clone());
        }
    }
    let mut avert: Vec<String> = Vec::new();
    // L'apercu n'a pas a refuser : c'est la production qui le fera.
    let urssaf = if !ctx.racine.join("config").join("organismes.json").is_file() {
        // Une compagnie neuve n'a pas encore ses organismes : le bordereau
        // attend, la paie non.
        avert.push(
            "Bordereau Urssaf pas encore calculable : les organismes de la compagnie (France Travail, Audiens, Urssaf) ne sont pas encore renseignés."
                .to_string(),
        );
        Value::Null
    } else {
        match crate::dsn::montant_urssaf(ctx, &paies_p, mois, None) {
            Ok(m) => Value::String(m.to_string()),
            Err(e) => {
                avert.push(format!("Bordereau Urssaf non calculable : {}", e));
                Value::Null
            }
        }
    };

    Ok((urssaf, avert))
}

/// Les paies d'une saisie depuis la reponse de son calcul, numerotees comme l'apercu les montre, et la vue de chacune
/// (`vue_paie`). Ce que `apercu` affiche, et ce que `valider-paie` fige (`calcul_du_mois::sceller`).
pub(crate) fn vues_du_calcul(ctx: &Contexte, data: &Value, rep: &Value, avec_lignes: bool) -> R<(Vec<Paie>, Vec<Value>)> {
    let engs = engagements_calcules(ctx, data, rep)?;
    let paies_json = rep.get("paies").and_then(Value::as_array).ok_or("reponse du calcul illisible : `paies`")?;
    let mut paies = Vec::new();
    for (e, v) in engs.into_iter().zip(paies_json) {
        paies.push(paie_de_json(ctx, e, v)?);
    }
    let mois = rep.get("mois").map(en_texte).ok_or_else(|| crate::fichiers::absent("mois"))?;
    numeroter(ctx, &mut paies, annee_de(&mois)?)?;
    let tous = data.get("engagements").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut vues = Vec::new();
    for (i, p) in paies.iter().enumerate() {
        vues.push(vue_paie(ctx, p, tous.get(i).unwrap_or(&Value::Null), data, avec_lignes)?);
    }
    Ok((paies, vues))
}

fn annee_de(mois: &str) -> R<i32> {
    Ok(mois.get(..4).and_then(|a| a.parse().ok()).ok_or_else(|| format!("mois illisible : {}", crate::fichiers::repr(mois)))?)
}

/// Les paies d'une saisie SANS CALCUL : le salarie, l'engagement et son contrat suivi, numerotes comme l'apercu les
/// numerote (`numeroter`). Ce que l'apercu d'un mois tout valide lit a cote de ses cartes figees.
fn paies_sans_calcul(ctx: &Contexte, data: &Value, mois: &str) -> R<Vec<Paie>> {
    let _derivation = crate::conges::Derivation::ouvrir();
    let (engs, _) = engagement::engagements(ctx, data, &crate::conges::lignes_du_mois)?;
    let mut paies = Vec::new();
    for e in &engs {
        let mut p = crate::amorcage::embauche(ctx, e)?;
        engagement::suivre_contrat(ctx, &mut p.engagement)?;
        paies.push(p);
    }
    numeroter(ctx, &mut paies, annee_de(mois)?)?;
    Ok(paies)
}

/// `apercu` : le mois tel que l'ecran le montre, rien d'ecrit -- ni fichier, ni compteur. Charge : `saisie` (le mois tel
/// que l'ecran le tient), `lignes` (vrai : les lignes de cotisation de chaque paie, pour le bulletin affiche), `urssaf`
/// (vrai : le bordereau Urssaf du mois, que la boite « Fermer le mois » montre ; il ne se calcule que la).
///
/// UNE PAIE VALIDEE SE LIT, ELLE NE SE CALCULE PAS (Leandre, 16/10/2026) : sa carte figee (`calcul_du_mois::cartes`) est
/// sa vue, avec ses numeros du moment. Un mois dont toutes les paies tiennent leur carte ne calcule rien. Sinon le mois se
/// calcule comme avant ; une paie validee y garde sa carte, et celle dont l'empreinte a bouge (un taux, une fiche...)
/// est comparee au calcul d'aujourd'hui : s'il ne donne plus la meme paie, la carte le dit (`revoir`).
fn apercu(o: &HashMap<String, String>, charge: &Value) -> R<Value> {
    let ctx = crate::service::contexte(o)?;
    let data = saisie_recue(&ctx, charge)?;
    let avec_lignes = vrai_opt(charge.get("lignes"));
    let avec_urssaf = vrai_opt(charge.get("urssaf"));
    let cartes = crate::calcul_du_mois::cartes(o, &ctx, &data);
    let toutes_tiennent = !cartes.is_empty() && cartes.iter().all(|c| matches!(c, Some((_, true))));
    let (mois, paies, vues) = if toutes_tiennent && !avec_lignes && !avec_urssaf {
        let mois = data.get("mois").map(en_texte).ok_or_else(|| crate::fichiers::absent("mois"))?;
        let paies = paies_sans_calcul(&ctx, &data, &mois)?;
        let vues = paies
            .iter()
            .zip(&cartes)
            .map(|(p, c)| {
                let mut v = c.as_ref().map(|(carte, _)| carte.clone()).unwrap_or(Value::Null);
                v["numero_bulletin"] = p.numero_bulletin.clone().map_or(Value::Null, Value::String);
                v["numero_contrat"] = p.engagement.numero.clone().map_or(Value::Null, Value::String);
                v
            })
            .collect::<Vec<_>>();
        (mois, paies, vues)
    } else {
        let rep = paie_du_mois(o, &ctx, &data)?;
        let mois = rep.get("mois").map(en_texte).ok_or_else(|| crate::fichiers::absent("mois"))?;
        let (paies, mut vues) = vues_du_calcul(&ctx, &data, &rep, avec_lignes)?;
        for (v, c) in vues.iter_mut().zip(&cartes) {
            let Some((carte, tient)) = c else { continue };
            let mut figee = if *tient { carte.clone() } else { crate::calcul_du_mois::devant_le_calcul(carte, v) };
            for k in ["numero_bulletin", "numero_contrat", "lignes"] {
                if let Some(x) = v.get(k) {
                    figee[k] = x.clone();
                }
            }
            *v = figee;
        }
        (mois, paies, vues)
    };
    let annee = annee_de(&mois)?;
    let (urssaf, avert) = if avec_urssaf { urssaf_de_l_apercu(&ctx, &paies, &mois)? } else { (Value::Null, Vec::new()) };
    let somme = |cle: &str| -> R<Decimal> {
        let mut t = Decimal::new(0, 2);
        for v in &vues {
            t += dec(v.get(cle).ok_or_else(|| crate::fichiers::absent(cle))?)?;
        }
        Ok(q2(t))
    };
    let (total_brut, total_cout) = (somme("brut")?, somme("cout_employeur")?);
    let mut a_regarder = avert;
    a_regarder.extend(archives_employees(&paies)?);
    let mut informations = Vec::new();
    for v in &vues {
        let textes = |k: &str| v.get(k).and_then(Value::as_array).cloned().unwrap_or_default();
        a_regarder.extend(textes("avertissements").iter().map(en_texte));
        informations.extend(textes("informations").iter().map(en_texte));
    }
    let n: usize = mois[5..7].parse().map_err(|_| format!("mois illisible : {}", crate::fichiers::repr(&mois)))?;
    Ok(json!({
        "mois": mois,
        "libelle": format!("{} {}", MOIS_TITRE[n.clamp(1, 12) - 1], annee),
        "paies": vues,
        "total_brut": total_brut.to_string(), "total_cout": total_cout.to_string(),
        "urssaf": urssaf,
        "avertissements": a_regarder,
        "informations": informations,
    }))
}
