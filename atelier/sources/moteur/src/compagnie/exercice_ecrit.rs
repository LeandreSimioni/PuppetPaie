//! Ce que la page ecrit pour l'exercice : `cloture` (et sa
//! reouverture, `rouvrir`) et `export-compta`.
//!
//! - L'export compta : les ecritures de l'annee (ou de `du` a `au`), mois par mois, et leur CSV
//!   (`documents::ecritures_comptables`).
//! - La cloture : fige l'annee (`config/exercices.json`) et reunit ce qu'il faut garder en un zip
//!   (`sorties/exercices/Exercice_AAAA.zip`) : saisies, sorties, retours des organismes, journal des depots,
//!   fiches, bareme, export compta, LISEZ-MOI. Le zip est ecrit par le zip de `routes::dossier_salarie` (deflate,
//!   CRC-32, entrees dans un ordre fixe) : la liste des fichiers et chacun d'eux sont fixes, octet pour octet ; les
//!   octets COMPRESSES (donc l'empreinte et la taille de l'archive) dependent du compresseur, et l'empreinte rendue
//!   est celle de l'archive que ce programme a reellement ecrite.
//!
//! Un refus de l'export pendant la cloture (une `ErreurOperation`, dont les refus du calcul) n'arrete pas la
//! cloture : il est rendu dans `manques`, et l'archive part sans export.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use droit::contexte::Contexte;
use socle::echec::erreur;
use socle::num::{zero2, Decimal};
use socle::echec::Echec;
use crate::routes::dossier_salarie::{entree_ecrite, entree_fichier, zip, Entree};
use documents::ecritures_comptables::{csv_compta, ecritures_du_mois, Ecriture};
use documents::plan_comptable::plan_comptable;
use crate::routes::rappels::mois_de_l_annee;

type R<T> = Result<T, Echec>;

fn refus(m: impl Into<String>) -> Echec {
    erreur("ErreurOperation", m)
}

fn en_os(e: &std::io::Error, p: &Path) -> Echec {
    erreur("OSError", socle::echec::du_disque(p, e))
}

/// Une erreur d'operation, que l'appelant rattrape : celles que le moteur nomme, et
/// les refus du calcul (`CalculRefuse`, `SaisieIncomplete`), qui en sont.
fn est_une_erreur_d_operation(e: &Echec) -> bool {
    matches!(e.classe, None | Some("ErreurOperation") | Some("SaisieIncomplete"))
}

/// L'heure de Paris, a la seconde.
fn maintenant() -> String {
    let t = socle::temps::secondes();
    socle::paris::iso_secondes(t)
}


// ------------------------------------------------------------------ le journal des depots

/// Une DSN est-elle partie pour ce mois, et pas refusee au bilan ? (`compagnie::etat_dsn`)
fn dsn_acceptee(ctx: &Contexte, mois: &str) -> R<bool> {
    Ok(crate::compagnie::etat_dsn::du_mois(ctx, socle::mois::Mois::lire(mois)?, None)?.partie())
}

// ------------------------------------------------------------------ l'export compta

/// L'export compta d'un exercice.
pub(crate) struct ExportCompta {
    pub(crate) annee: String,
    pub(crate) du: u32,
    pub(crate) au: u32,
    pub(crate) mois: Vec<String>,
    pub(crate) ecritures: Vec<Ecriture>,
    pub(crate) total_debit: Decimal,
    pub(crate) total_credit: Decimal,
    pub(crate) fichier: Option<PathBuf>,
    pub(crate) plan: String,
    pub(crate) avertissements: Vec<String>,
}

/// Une ecriture de l'export, telle que la route la rend : les montants en texte.
#[derive(Debug, Serialize)]
pub(crate) struct EcritureRendue {
    journal: String,
    date: String,
    compte: String,
    libelle: String,
    debit: String,
    credit: String,
    analytique: String,
    mois: String,
    auxiliaire: String,
    auxiliaire_libelle: String,
}

/// Ce que rend `export-compta` : sa phrase, l'annee et ses mois (de `du` a `au`), les ecritures et leurs totaux, le
/// fichier ecrit (son nom ; `dossier`, celui ou il est), le plan comptable pris, et ce qu'il faut savoir.
#[derive(Debug, Serialize)]
pub(crate) struct ExportComptaRendu {
    phrase: String,
    annee: String,
    du: u32,
    au: u32,
    mois: Vec<String>,
    ecritures: Vec<EcritureRendue>,
    total_debit: String,
    total_credit: String,
    dossier: Option<String>,
    fichier: Option<String>,
    plan: String,
    avertissements: Vec<String>,
}

impl ExportCompta {
    /// Ce que la route rend : un chemin ne rend que son nom ; `dossier`, celui ou il est ecrit.
    pub(crate) fn rendu(self) -> ExportComptaRendu {
        let dossier = self.fichier.as_ref().and_then(|f| f.parent()).map(|d| d.to_string_lossy().into_owned());
        let fichier = self.fichier.as_ref().map(|f| f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
        let phrase = match (&fichier, &dossier) {
            (Some(f), d) => format!("Export comptable de {} : {} écritures, dans {} ({}).", self.annee, self.ecritures.len(), f, d.as_deref().unwrap_or("")),
            (None, _) => format!("Export comptable de {} : {} écritures, rien n'est écrit.", self.annee, self.ecritures.len()),
        };
        ExportComptaRendu {
            phrase,
            annee: self.annee,
            du: self.du,
            au: self.au,
            mois: self.mois,
            ecritures: self
                .ecritures
                .into_iter()
                .map(|e| EcritureRendue {
                    journal: e.journal,
                    date: e.date,
                    compte: e.compte,
                    libelle: e.libelle,
                    debit: e.debit.to_string(),
                    credit: e.credit.to_string(),
                    analytique: e.analytique,
                    mois: e.mois,
                    auxiliaire: e.auxiliaire,
                    auxiliaire_libelle: e.auxiliaire_libelle,
                })
                .collect(),
            total_debit: self.total_debit.to_string(),
            total_credit: self.total_credit.to_string(),
            dossier,
            fichier,
            plan: self.plan,
            avertissements: self.avertissements,
        }
    }
}

fn dossier_exercices(ctx: &Contexte) -> PathBuf {
    ctx.racine.join("sorties").join("exercices")
}

/// Le journal de paie de l'annee (ou de `du` a `au`), et son fichier CSV.
/// Les mois pris sont ceux qui ont une saisie.
pub(crate) fn export_compta(ctx: &Contexte, annee: &str, du: u32, au: u32, ecrire: bool) -> R<ExportCompta> {
    let plan = plan_comptable(ctx)?;
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let propre = ctx.racine.join(documents::plan_comptable::PLAN_COMPTABLE);
    let origine = if propre.exists() { documents::plan_comptable::PLAN_COMPTABLE } else { "plan livré avec le logiciel (plan-comptable-base.json)" };
    let mois: Vec<String> = mois_de_l_annee(ctx, annee)
        .into_iter()
        .filter(|m| {
            let n: u32 = m.chars().skip(5).take(2).collect::<String>().parse().unwrap_or(0);
            du <= n && n <= au && engagement::stock::a_une_saisie(&ctx.racine, m).unwrap_or(false)
        })
        .collect();
    if mois.is_empty() {
        return Err(refus(format!(
            "Aucune paie saisie en {}{}.",
            annee,
            if (du, au) == (1, 12) { String::new() } else { format!(" entre les mois {} et {}", du, au) }
        )));
    }
    let mut avert: Vec<String> = Vec::new();
    let mut ecritures: Vec<Ecriture> = Vec::new();
    for m in &mois {
        if !ctx.racine.join("sorties").join(m).is_dir() {
            avert.push(format!("{} : saisi mais jamais écrit (pas de bulletin) -- pris quand même, depuis la saisie.", m));
        } else if !dsn_acceptee(ctx, m)? {
            avert.push(format!("{} : pas de DSN réelle acceptée au journal.", m));
        }
        ecritures.extend(ecritures_du_mois(ctx, m, &plan, &departs)?);
    }
    if !propre.exists() {
        avert.push("Plan comptable : celui livré avec le logiciel, à faire valider par le comptable (config/plan-comptable.json pour le remplacer).".to_string());
    }
    let zero = zero2();
    let total_debit = ecritures.iter().fold(zero, |a, e| a + e.debit);
    let total_credit = ecritures.iter().fold(zero, |a, e| a + e.credit);
    let mut fichier = None;
    if ecrire {
        let dossier = dossier_exercices(ctx);
        std::fs::create_dir_all(&dossier).map_err(|e| en_os(&e, &dossier))?;
        let nom = if (du, au) == (1, 12) { format!("Compta_{}.csv", annee) } else { format!("Compta_{}_{:02}-{:02}.csv", annee, du, au) };
        let f = dossier.join(nom);
        std::fs::write(&f, csv_compta(&ecritures)).map_err(|e| en_os(&e, &f))?;
        // L'export de l'annee entiere note les saisies qu'il a lues (`export_compta_a_jour`).
        let lues = ((du, au) == (1, 12)).then(|| empreintes_des_saisies(&ctx.racine, &annee.to_string()));
        crate::compagnie::depart::noter_piece_ecrite(&ctx.racine, &crate::compagnie::depart::cle_de_piece(&f), lues, None)?;
        fichier = Some(f);
    }
    Ok(ExportCompta { annee: annee.to_string(), du, au, mois, ecritures, total_debit, total_credit, fichier, plan: origine.to_string(), avertissements: avert })
}

/// LES SAISIES DE L'ANNEE, PAR LEUR EMPREINTE : `{"AAAA-MM": empreinte}` pour chaque mois saisi de l'annee
/// (`crate::routes::etapes::empreinte_de` de la saisie du mois, la meme que celle d'une DSN ecrite) ; une saisie illisible vaut
/// `null`. L'export compta de l'annee la note en s'ecrivant (`noter_piece_ecrite`) : c'est ce qu'il a lu.
pub(crate) fn empreintes_des_saisies(racine: &Path, annee: &str) -> Value {
    let mut m = Map::new();
    for mois in engagement::stock::mois_saisis(racine).unwrap_or_default().into_iter().map(String::from).filter(|x| x.starts_with(annee)) {
        let e = engagement::stock::vue_du_mois(racine, &mois).ok().flatten().map_or(Value::Null, |v| json!(crate::routes::etapes::empreinte_de(&socle::ecriture::en_valeur(&v))));
        m.insert(mois, e);
    }
    Value::Object(m)
}

/// L'EXPORT COMPTA DE L'ANNEE EST-IL A JOUR ? Un seul critere, celui des rappels (`routes::rappels`) et de
/// l'horloge d'une fictive (`horloge::l_annee`) : `Compta_<annee>.csv` existe, et les saisies de l'annee sont celles
/// qu'il a lues -- les empreintes notees au journal des ecritures en l'ecrivant (`empreintes_des_saisies`). Jamais la
/// date des fichiers : une fictive decompressee chez un visiteur a des dates qui ne disent rien. Un export dont le
/// journal ne dit pas ce qu'il a lu (ecrit avant cette note, ou livre avec une fictive) : le moteur ne sait pas que les
/// saisies ont change depuis, il ne le redemande pas.
pub(crate) fn export_compta_a_jour(racine: &Path, annee: &str) -> bool {
    let nom = format!("Compta_{}.csv", annee);
    if !racine.join("sorties").join("exercices").join(&nom).is_file() {
        return false;
    }
    let journal = crate::compagnie::depart::journal_des_ecritures(racine);
    match journal.get(&format!("exercices/{}", nom)).and_then(|n| n.get("empreinte_saisie")) {
        Some(lues) if lues.is_object() => *lues == empreintes_des_saisies(racine, annee),
        _ => true,
    }
}

/// La charge de `export-compta` : l'annee, du mois au mois (toute l'annee sans eux), et l'apercu (rien n'est ecrit).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeExportCompta {
    annee: crate::routes::dossier_salarie::AnneeDite,
    #[serde(default)]
    du: Option<u32>,
    #[serde(default)]
    au: Option<u32>,
    #[serde(default)]
    apercu: bool,
}

/// La route `export-compta`.
pub(crate) fn export_compta_route(ctx: &Contexte, c: ChargeExportCompta) -> R<ExportComptaRendu> {
    Ok(export_compta(ctx, &c.annee.texte(), c.du.unwrap_or(1), c.au.unwrap_or(12), !c.apercu)?.rendu())
}

// ------------------------------------------------------------------ la cloture

/// Les exercices clos de la compagnie, sous sa racine.
pub const EXERCICES: &str = "config/exercices.json";

/// `config/exercices.json` : chaque annee close (sa cloture, sous son annee) et, sous `_rouverts`, les clotures retirees,
/// dans l'ordre ou elles l'ont ete. Lu par sa forme (`lire_exercices` : une cle qui n'est ni une annee ni `_rouverts`
/// est refusee, comme une cle inconnue d'une cloture), ecrit par `ecrire_exercices`.
#[derive(Debug, Default, Clone)]
pub(crate) struct Exercices {
    clos: BTreeMap<String, ExerciceNote>,
    rouverts: Vec<ExerciceRouvertNote>,
}

/// La cloture d'une annee : quand, par qui, l'archive (son chemin sous la racine, sa taille, son empreinte, le nombre de
/// fichiers qu'elle reunit), si elle a ete forcee, et les mois clos sans DSN acceptee.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExerciceNote {
    pub(crate) le: String,
    qui: String,
    archive: String,
    octets: u64,
    empreinte: String,
    fichiers: u64,
    forcee: bool,
    sans_dsn: Vec<String>,
}

/// Une cloture retiree : la cloture telle qu'elle etait, son annee, quand et par qui elle l'a ete.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ExerciceRouvertNote {
    le: String,
    qui: String,
    archive: String,
    octets: u64,
    empreinte: String,
    fichiers: u64,
    forcee: bool,
    sans_dsn: Vec<String>,
    annee: String,
    rouvert_le: String,
    rouvert_par: String,
}

/// La cle des clotures retirees.
const ROUVERTS: &str = "_rouverts";

impl<'de> Deserialize<'de> for Exercices {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Exercices, D::Error> {
        struct Lecteur;
        impl<'de> serde::de::Visitor<'de> for Lecteur {
            type Value = Exercices;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("les années closes et la liste des clôtures retirées")
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut m: A) -> Result<Exercices, A::Error> {
                let mut e = Exercices::default();
                while let Some(cle) = m.next_key::<String>()? {
                    if cle == ROUVERTS {
                        e.rouverts = m.next_value()?;
                    } else if cle.len() == 4 && cle.chars().all(|c| c.is_ascii_digit()) {
                        let clos = m.next_value()?;
                        e.clos.insert(cle, clos);
                    } else {
                        return Err(serde::de::Error::custom(format!("la clé « {} » n'est ni une année close ni la liste des clôtures retirées", cle)));
                    }
                }
                Ok(e)
            }
        }
        d.deserialize_map(Lecteur)
    }
}

impl Serialize for Exercices {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut m = s.serialize_map(None)?;
        for (annee, clos) in &self.clos {
            m.serialize_entry(annee, clos)?;
        }
        if !self.rouverts.is_empty() {
            m.serialize_entry(ROUVERTS, &self.rouverts)?;
        }
        m.end()
    }
}

impl Exercices {
    /// La cloture de l'annee de `mois_ou_annee` (`AAAA` ou `AAAA-MM`), ou `None`.
    pub(crate) fn de(&self, mois_ou_annee: &str) -> Option<&ExerciceNote> {
        self.clos.get(mois_ou_annee.get(..4).unwrap_or(mois_ou_annee))
    }

    /// Les annees closes, et leur cloture.
    pub(crate) fn clos(&self) -> &BTreeMap<String, ExerciceNote> {
        &self.clos
    }
}

/// `config/exercices.json`, lu par sa forme : aucune annee close s'il n'existe pas. Le texte est lu une fois tant qu'il
/// ne change pas (l'etat d'un mois le demande pour chaque mois) ; le refus d'un fichier illisible ne se garde pas.
pub(crate) fn lire_exercices(racine: &Path) -> R<Exercices> {
    let p = racine.join(EXERCICES);
    if !socle::fichiers::existe(&p) {
        return Ok(Exercices::default());
    }
    let v = socle::fichiers::json_en_cache_ou(&p, socle::fichiers::lire_json_echec)?;
    socle::forme::lire(&v, "Le fichier des exercices clos (config/exercices.json) ne se lit pas.").map_err(|m| erreur("ValueError", m))
}

fn ecrire_exercices(ctx: &Contexte, tous: &Exercices) -> R<()> {
    let chemin = ctx.racine.join(EXERCICES);
    socle::ecriture::ecrire_json(&chemin, &crate::routes::en_json(tous)?, 1).map_err(|m| erreur("OSError", m))
}

/// Les fichiers sous `d`, triés par chemin (les chemins se comparent composant par composant).
fn fichiers_sous(d: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(d) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            fichiers_sous(&p, out);
        } else if p.is_file() {
            out.push(p);
        }
    }
}

/// Fige l'annee et reunit ce qu'il faut garder en un zip. Refuse si un mois
/// de l'annee qui a des paies n'a pas de DSN reelle acceptee (`forcer` passe outre, et c'est ecrit dans
/// `exercices.json`), une annee deja close, et l'annee en cours.
pub(crate) fn cloturer(ctx: &Contexte, annee: &str, forcer: bool, qui: &str) -> R<ExerciceClos> {
    let mut tous = lire_exercices(&ctx.racine)?;
    if let Some(clos) = tous.de(annee) {
        return Err(refus(format!("L'exercice {} est déjà clos (le {}).", annee, clos.le.chars().take(10).collect::<String>())));
    }
    let an: i64 = socle::num::entier_lu(annee).and_then(|n| i64::try_from(n).ok()).ok_or_else(|| erreur("ValueError", format!("nombre entier illisible : {}", socle::fichiers::repr(annee))))?;
    if an >= i64::from(ctx.aujourdhui.an) {
        return Err(refus(format!("L'exercice {} n'est pas fini : on clôture une année passée.", annee)));
    }
    let mois = mois_de_l_annee(ctx, annee);
    let mut sans_dsn: Vec<String> = Vec::new();
    for m in &mois {
        if engagement::stock::a_une_saisie(&ctx.racine, m).unwrap_or(false) && !dsn_acceptee(ctx, m)? {
            sans_dsn.push(m.clone());
        }
    }
    if !sans_dsn.is_empty() && !forcer {
        return Err(refus(format!(
            "Pas de DSN réelle acceptée pour {}. La clôture vient APRÈS la dernière DSN ; déposer d'abord, ou forcer en le sachant.",
            sans_dsn.join(", ")
        )));
    }

    // L'export compta fait partie de ce qu'on garde : on l'ecrit si absent.
    let (fichier_export, manques) = match export_compta(ctx, annee, 1, 12, true) {
        Ok(e) => (e.fichier, Vec::new()),
        Err(e) if est_une_erreur_d_operation(&e) => (None, vec![format!("export compta : {}", e.message)]),
        Err(e) => return Err(e),
    };

    let dossier = dossier_exercices(ctx);
    std::fs::create_dir_all(&dossier).map_err(|e| en_os(&e, &dossier))?;
    let archive = dossier.join(format!("Exercice_{}.zip", annee));
    let mut entrees: Vec<Entree> = Vec::new();
    // Les engagements qui touchent l'annee : le fichier de chacun de ses mois, et celui du mois ou commence un engagement
    // qui court encore dans l'annee (un contrat commence l'annee d'avant).
    let mut fichiers_engagements: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for m in &mois {
        fichiers_engagements.insert(m.clone());
        for o in engagement::stock::objets_du_mois(&ctx.racine, m).map_err(refus)? {
            fichiers_engagements.insert(o.fichier);
        }
    }
    for f in &fichiers_engagements {
        let s = engagement::stock::chemin(&ctx.racine, f);
        if s.exists() {
            entrees.push(entree_fichier(&s, format!("{}/{}.json", engagement::stock::DOSSIER, f))?);
        }
    }
    // Les paies de chaque mois de l'annee.
    for m in &mois {
        let s = engagement::paies::chemin(&ctx.racine, m);
        if s.exists() {
            entrees.push(entree_fichier(&s, format!("{}/{}.json", engagement::paies::DOSSIER, m))?);
        }
    }
    for m in &mois {
        let d = ctx.racine.join("sorties").join(m);
        if d.is_dir() {
            let mut fichiers = Vec::new();
            fichiers_sous(&d, &mut fichiers);
            fichiers.sort();
            for p in fichiers {
                let rel: Vec<String> = p.strip_prefix(&d).unwrap_or(&p).components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
                entrees.push(entree_fichier(&p, format!("sorties/{}/{}", m, rel.join("/")))?);
            }
        }
    }
    // Les retours des organismes : les fichiers dont le nom commence par l'annee (AAAAMMJJ-HHMMSS_nature..). Ceux de
    // janvier suivant qui repondent a la DSN de decembre sont pris aussi : ils portent sur l'annee close.
    let suivante = format!("{}01", an + 1);
    for sous in ["", "dpae"] {
        let d = if sous.is_empty() { ctx.racine.join(envois::retours::RETOURS) } else { ctx.racine.join(envois::retours::RETOURS).join(sous) };
        if d.is_dir() {
            let mut noms: Vec<PathBuf> = std::fs::read_dir(&d).map_err(|e| en_os(&e, &d))?.flatten().map(|e| e.path()).collect();
            noms.sort();
            for p in noms {
                let nom = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                if p.is_file() && (nom.starts_with(annee) || nom.starts_with(&suivante)) {
                    let prefixe = if sous.is_empty() { String::new() } else { format!("{}/", sous) };
                    entrees.push(entree_fichier(&p, format!("retours/{}{}", prefixe, nom))?);
                }
            }
        }
    }
    let j = envois::journal::chemin(&ctx.racine);
    if j.exists() {
        entrees.push(entree_fichier(&j, envois::journal::DEPOTS.to_string())?);
    }
    let cfg = ctx.racine.join(socle::fichiers::CONFIG);
    for nom in ["entreprise.json", "salaries.json", "spectacles.json", "organismes.json", "rubriques.json", "compteurs.json"] {
        if cfg.join(nom).exists() {
            entrees.push(entree_fichier(&cfg.join(nom), format!("config/{}", nom))?);
        }
    }
    let bar = cfg.join("baremes").join(format!("{}.json", annee));
    if bar.exists() {
        entrees.push(entree_fichier(&bar, format!("config/baremes/{}.json", annee))?);
    }
    if let Some(f) = &fichier_export {
        entrees.push(entree_fichier(f, format!("compta/{}", f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()))?);
    }
    let aujourd_hui = ctx.aujourdhui;
    let lisez_moi = format!(
        "Exercice {} -- archive de cloture, PuppetPaie, le {:02}/{:02}/{:04}.\n\n\
         engagements/ les engagements de chaque mois ou ils commencent, et leurs paies validees\n\
         sorties/    bulletins, contrats (signes dans envoi/), DPAE, DSN, certificats, courriels\n\
         retours/    les reponses des organismes (net-entreprises, Urssaf DPAE)\n\
         journal/    le journal des depots\n\
         config/     les fiches telles qu'elles etaient a la cloture\n\
         compta/     le journal des ecritures de paie\n",
        annee, aujourd_hui.jour, aujourd_hui.mois, aujourd_hui.an
    );
    entrees.push(entree_ecrite("LISEZ-MOI.txt".to_string(), lisez_moi.into_bytes()));
    let n = entrees.len();
    let octets = zip(&entrees)?;
    std::fs::write(&archive, &octets).map_err(|e| en_os(&e, &archive))?;

    let empreinte: String = Sha256::digest(&octets).iter().map(|b| format!("{:02x}", b)).collect();
    let le = maintenant();
    let relatif = archive.strip_prefix(&ctx.racine).unwrap_or(&archive).to_string_lossy().replace('\\', "/");
    tous.clos.insert(
        annee.to_string(),
        ExerciceNote {
            le: le.clone(),
            qui: qui.to_string(),
            archive: relatif,
            octets: octets.len() as u64,
            empreinte: empreinte.clone(),
            fichiers: n as u64,
            forcee: forcer && !sans_dsn.is_empty(),
            sans_dsn: if forcer { sans_dsn } else { Vec::new() },
        },
    );
    ecrire_exercices(ctx, &tous)?;
    Ok(ExerciceClos { annee: annee.to_string(), le, archive: archive.to_string_lossy().into_owned(), octets: octets.len(), empreinte, fichiers: n, manques })
}

/// Un exercice clos : l'annee, quand, l'archive (son chemin, sa taille, son empreinte, le nombre de fichiers qu'elle
/// reunit), et ce qui n'a pu s'y archiver.
#[derive(Debug, serde::Serialize)]
pub(crate) struct ExerciceClos {
    pub(crate) annee: String,
    le: String,
    archive: String,
    octets: usize,
    empreinte: String,
    pub(crate) fichiers: usize,
    pub(crate) manques: Vec<String>,
}

/// Un exercice rouvert : l'annee, depuis quand il etait clos, et son archive, qui reste.
#[derive(Debug, serde::Serialize)]
pub(crate) struct ExerciceRouvert {
    pub(crate) annee: String,
    etait_clos_depuis: String,
    archive: String,
}

/// Ce que fait la route `cloture` : clore l'exercice, ou le rouvrir.
#[derive(Debug, serde::Serialize)]
#[serde(untagged)]
pub(crate) enum Cloture {
    Close(ExerciceClos),
    Rouverte(ExerciceRouvert),
}

/// La charge de `cloture` : l'annee (comme `etapes::ChargeAnnee` l'ecrit, ou en nombre), clore malgre un mois sans DSN
/// acceptee (`forcer`), ou rouvrir ; `qui` : la ligne de commande ecrit `cli`, la page rien (`interface`).
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeCloture {
    annee: crate::routes::dossier_salarie::AnneeDite,
    #[serde(default)]
    forcer: bool,
    #[serde(default)]
    rouvrir: bool,
    #[serde(default)]
    qui: Option<String>,
}

/// Retire la cloture. L'archive reste sur le disque : elle dit ce
/// qu'etait l'annee au moment ou on l'a fermee.
fn rouvrir_exercice(ctx: &Contexte, annee: &str, qui: &str) -> R<ExerciceRouvert> {
    let mut tous = lire_exercices(&ctx.racine)?;
    let Some(clos) = tous.clos.remove(annee) else {
        return Err(refus(format!("L'exercice {} n'est pas clos.", annee)));
    };
    let (etait_clos_depuis, archive) = (clos.le.clone(), clos.archive.clone());
    let ExerciceNote { le, qui: clos_par, archive: a, octets, empreinte, fichiers, forcee, sans_dsn } = clos;
    tous.rouverts.push(ExerciceRouvertNote {
        le,
        qui: clos_par,
        archive: a,
        octets,
        empreinte,
        fichiers,
        forcee,
        sans_dsn,
        annee: annee.to_string(),
        rouvert_le: maintenant(),
        rouvert_par: qui.to_string(),
    });
    ecrire_exercices(ctx, &tous)?;
    Ok(ExerciceRouvert { annee: annee.to_string(), etait_clos_depuis, archive })
}

/// La route `cloture` : clore l'annee, ou la rouvrir.
pub(crate) fn cloture_route(ctx: &Contexte, c: ChargeCloture) -> R<Cloture> {
    let annee = c.annee.texte();
    let qui = c.qui.filter(|q| !q.is_empty()).unwrap_or_else(|| crate::routes::INTERFACE.to_string());
    if c.rouvrir {
        return rouvrir_exercice(ctx, &annee, &qui).map(Cloture::Rouverte);
    }
    cloturer(ctx, &annee, c.forcer, &qui).map(Cloture::Close)
}
