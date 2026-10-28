//! Ce que la page ecrit pour l'exercice (`moteur/operations_exercice.py`) : `cloture` (et sa
//! reouverture, `rouvrir`) et `export-compta`.
//!
//! - L'export compta : les ecritures de l'annee (ou de `du` a `au`), mois par mois, et leur CSV
//!   (`documents::ecritures_comptables`).
//! - La cloture : fige l'annee (`config/exercices.json`) et reunit ce qu'il faut garder en un zip
//!   (`sorties/exercices/Exercice_AAAA.zip`) : saisies, sorties, retours des organismes, journal des depots,
//!   fiches, bareme, export compta, LISEZ-MOI. Le zip est ecrit par le zip de `routes::dossier_salarie` (deflate,
//!   CRC-32, entrees dans l'ordre du Python) : la liste des fichiers et chacun d'eux, octet pour octet, sont ceux
//!   de l'ancien Python ; les octets COMPRESSES (donc l'empreinte et la taille de l'archive) dependent du
//!   compresseur, et l'empreinte rendue est celle de l'archive que ce programme a reellement ecrite.
//!
//! Un refus de l'export pendant la cloture (une `ErreurOperation`, dont les refus du calcul) n'arrete pas la
//! cloture : il est rendu dans `manques`, et l'archive part sans export.

use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use droit::contexte::Contexte;
use envois::etats;
use socle::valeur::valeur_en_texte;
use socle::echec::erreur;
use socle::fichiers::vrai;
use socle::num::{zero2, Decimal};
use socle::echec::Echec;
use crate::routes::dossier_salarie::{entree_ecrite, entree_fichier, zip, Entree};
use documents::ecritures_comptables::{csv_compta, ecritures_du_mois, Ecriture};
use documents::plan_comptable::plan_comptable;
use crate::routes::rappels::{mois_de_l_annee, nic_principal};

type R<T> = Result<T, Echec>;

fn refus(m: impl Into<String>) -> Echec {
    erreur("ErreurOperation", m)
}

fn en_os(e: &std::io::Error, p: &Path) -> Echec {
    erreur("OSError", format!("{} : {}", p.display(), e))
}

/// Une `ErreurOperation` pour un `except ErreurOperation` de Python : celles que le moteur nomme, et
/// les refus du calcul (`CalculRefuse`, `SaisieIncomplete`), qui en sont.
fn est_une_erreur_d_operation(e: &Echec) -> bool {
    matches!(e.classe, None | Some("ErreurOperation") | Some("SaisieIncomplete"))
}

/// L'heure de Paris, a la seconde (`datetime.now().isoformat(timespec="seconds")`).
fn maintenant() -> String {
    let t = socle::temps::secondes();
    socle::paris::iso_secondes(t)
}

/// `str(annee)` d'une valeur de la charge.
fn texte_de(v: &Value) -> String {
    valeur_en_texte(v)
}

fn exiger<'a>(charge: &'a Value, cle: &str) -> R<&'a Value> {
    charge.get(cle).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(cle)))
}

/// `int(x)` d'une valeur JSON.
fn entier(v: &Value) -> R<i64> {
    match v {
        Value::Number(n) => Ok(n.as_i64().unwrap_or_else(|| n.as_f64().unwrap_or(0.0) as i64)),
        Value::Bool(b) => Ok(i64::from(*b)),
        Value::String(s) => socle::num::int_py(s)
            .and_then(|n| i64::try_from(n).ok())
            .ok_or_else(|| erreur("ValueError", format!("nombre entier illisible : {}", socle::fichiers::repr(s)))),
        autre => Err(erreur(
            "TypeError",
            socle::fichiers::attendu("un nombre entier", socle::valeur::nom_type(autre)),
        )),
    }
}

// ------------------------------------------------------------------ le journal des depots

/// Le journal des depots, relu une fois, quand il faut dire si la DSN d'un mois est partie.
struct Depots<'a> {
    ctx: &'a Contexte,
    lignes: Option<Vec<Value>>,
}

impl<'a> Depots<'a> {
    fn nouveau(ctx: &'a Contexte) -> Self {
        Depots { ctx, lignes: None }
    }

    /// `journal.etat_dsn_du_mois(ctx, mois)` : une DSN reelle acceptee est-elle partie pour ce mois ?
    fn dsn_acceptee(&mut self, mois: &str) -> R<bool> {
        if self.lignes.is_none() {
            self.lignes = Some(etats::charger(&self.ctx.racine)?);
        }
        let l = self.lignes.as_ref().unwrap();
        Ok(vrai(&etats::etat_dsn_du_mois(l, mois, &Value::Null, &nic_principal(self.ctx))?))
    }
}

// ------------------------------------------------------------------ l'export compta

/// `operations_exercice.ExportCompta`.
pub(crate) struct ExportCompta {
    pub(crate) annee: String,
    pub(crate) du: i64,
    pub(crate) au: i64,
    pub(crate) mois: Vec<String>,
    pub(crate) ecritures: Vec<Ecriture>,
    pub(crate) total_debit: Decimal,
    pub(crate) total_credit: Decimal,
    pub(crate) fichier: Option<PathBuf>,
    pub(crate) plan: String,
    pub(crate) avertissements: Vec<String>,
}

impl ExportCompta {
    /// `sortie_json.brut(export)` : un chemin ne rend que son nom.
    pub(crate) fn json(&self) -> Value {
        json!({
            "annee": self.annee, "du": self.du, "au": self.au, "mois": self.mois,
            "ecritures": self.ecritures.iter().map(Ecriture::json).collect::<Vec<_>>(),
            "total_debit": self.total_debit.to_string(), "total_credit": self.total_credit.to_string(),
            "fichier": self.fichier.as_ref().map(|f| f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
            "plan": self.plan, "avertissements": self.avertissements,
        })
    }
}

fn dossier_exercices(ctx: &Contexte) -> PathBuf {
    ctx.racine.join("sorties").join("exercices")
}

/// `operations_exercice.export_compta` : le journal de paie de l'annee (ou de `du` a `au`), et son fichier CSV.
/// Les mois pris sont ceux qui ont une saisie.
fn export_compta(ctx: &Contexte, annee: &str, du: i64, au: i64, ecrire: bool) -> R<ExportCompta> {
    let plan = plan_comptable(ctx)?;
    let propre = ctx.racine.join("config").join("plan-comptable.json");
    let origine = if propre.exists() { "config/plan-comptable.json" } else { "plan livré avec le logiciel (plan-comptable-base.json)" };
    let mois: Vec<String> = mois_de_l_annee(ctx, annee)
        .into_iter()
        .filter(|m| {
            let n: i64 = m.chars().skip(5).take(2).collect::<String>().parse().unwrap_or(0);
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
    let mut depots = Depots::nouveau(ctx);
    let mut avert: Vec<String> = Vec::new();
    let mut ecritures: Vec<Ecriture> = Vec::new();
    for m in &mois {
        if !ctx.racine.join("sorties").join(m).is_dir() {
            avert.push(format!("{} : saisi mais jamais écrit (pas de bulletin) -- pris quand même, depuis la saisie.", m));
        } else if !depots.dsn_acceptee(m)? {
            avert.push(format!("{} : pas de DSN réelle acceptée au journal.", m));
        }
        ecritures.extend(ecritures_du_mois(ctx, m, &plan)?);
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
        crate::depart::noter_piece_ecrite(&ctx.racine, &crate::depart::cle_de_piece(&f), lues)?;
        fichier = Some(f);
    }
    Ok(ExportCompta { annee: annee.to_string(), du, au, mois, ecritures, total_debit, total_credit, fichier, plan: origine.to_string(), avertissements: avert })
}

/// LES SAISIES DE L'ANNEE, PAR LEUR EMPREINTE (#236) : `{"AAAA-MM": empreinte}` pour chaque mois saisi de l'annee
/// (`crate::routes::etapes::empreinte_de` de la saisie du mois, la meme que celle d'une DSN ecrite) ; une saisie illisible vaut
/// `null`. L'export compta de l'annee la note en s'ecrivant (`noter_piece_ecrite`) : c'est ce qu'il a lu.
pub(crate) fn empreintes_des_saisies(racine: &Path, annee: &str) -> Value {
    let mut m = Map::new();
    for mois in engagement::stock::mois_saisis(racine).unwrap_or_default().into_iter().filter(|x| x.starts_with(annee)) {
        let e = engagement::stock::vue_du_mois(racine, &mois).ok().flatten().map_or(Value::Null, |v| json!(crate::routes::etapes::empreinte_de(&v)));
        m.insert(mois, e);
    }
    Value::Object(m)
}

/// L'EXPORT COMPTA DE L'ANNEE EST-IL A JOUR ? (#236) Un seul critere, celui des rappels (`routes::rappels`) et de
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
    let journal = crate::depart::journal_des_ecritures(racine);
    match journal.get(&format!("exercices/{}", nom)).and_then(|n| n.get("empreinte_saisie")) {
        Some(lues) if lues.is_object() => *lues == empreintes_des_saisies(racine, annee),
        _ => true,
    }
}

/// La route `export-compta` : `{"annee", "du"?, "au"?, "apercu"?}`.
pub(crate) fn export_compta_route(ctx: &Contexte, charge: &Value) -> R<Value> {
    Ok(export_compta_charge(ctx, charge)?.json())
}

/// L'export compta de la charge de la route, avant sa mise en JSON (la ligne de commande en rend le chemin entier).
pub(crate) fn export_compta_charge(ctx: &Contexte, charge: &Value) -> R<ExportCompta> {
    let annee = texte_de(exiger(charge, "annee")?);
    let borne = |cle: &str, defaut: i64| -> R<i64> {
        match charge.get(cle).filter(|v| vrai(v)) {
            Some(v) => entier(v),
            None => Ok(defaut),
        }
    };
    let du = borne("du", 1)?;
    let au = borne("au", 12)?;
    let ecrire = !charge.get("apercu").is_some_and(vrai);
    export_compta(ctx, &annee, du, au, ecrire)
}

// ------------------------------------------------------------------ la cloture

/// `operations_socle.exercices` : `config/exercices.json`, sans les cles qui commencent par `_`.
fn lire_exercices(ctx: &Contexte) -> R<Map<String, Value>> {
    match crate::routes::lire_json_ou_vide(&ctx.racine.join("config").join("exercices.json"))? {
        Value::Object(o) => Ok(o),
        autre => Err(erreur("AttributeError", socle::fichiers::attendu("un objet", socle::valeur::nom_type(&autre)))),
    }
}

/// `exercice_clos(ctx, annee)` : la cloture de l'annee (ou `None`).
fn exercice_clos(ctx: &Contexte, annee: &str) -> R<Option<Value>> {
    let a: String = annee.chars().take(4).collect();
    Ok(lire_exercices(ctx)?.into_iter().find(|(k, _)| !k.starts_with('_') && *k == a).map(|(_, v)| v))
}

fn ecrire_exercices(ctx: &Contexte, tous: &Map<String, Value>) -> R<()> {
    let chemin = ctx.racine.join("config").join("exercices.json");
    socle::ecriture::ecrire_json(&chemin, &Value::Object(tous.clone()), 1).map_err(|m| erreur("OSError", m))
}

/// Les fichiers sous `d`, dans l'ordre de `sorted(d.rglob("*"))` (les chemins se comparent composant par composant).
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

/// `operations_exercice.cloturer` : fige l'annee et reunit ce qu'il faut garder en un zip. Refuse si un mois
/// de l'annee qui a des paies n'a pas de DSN reelle acceptee (`forcer` passe outre, et c'est ecrit dans
/// `exercices.json`), une annee deja close, et l'annee en cours.
pub(crate) fn cloturer(ctx: &Contexte, annee: &str, forcer: bool, qui: &str) -> R<Value> {
    if let Some(clos) = exercice_clos(ctx, annee)? {
        let le: String = match clos.get("le") {
            Some(v) => valeur_en_texte(v),
            None => "?".to_string(),
        };
        return Err(refus(format!("L'exercice {} est déjà clos (le {}).", annee, le.chars().take(10).collect::<String>())));
    }
    let an = entier(&Value::String(annee.to_string()))?;
    if an >= i64::from(ctx.aujourdhui.an) {
        return Err(refus(format!("L'exercice {} n'est pas fini : on clôture une année passée.", annee)));
    }
    let mois = mois_de_l_annee(ctx, annee);
    let mut depots = Depots::nouveau(ctx);
    let mut sans_dsn: Vec<String> = Vec::new();
    for m in &mois {
        if engagement::stock::a_une_saisie(&ctx.racine, m).unwrap_or(false) && !depots.dsn_acceptee(m)? {
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
        let d = if sous.is_empty() { ctx.racine.join("retours") } else { ctx.racine.join("retours").join(sous) };
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
    let j = ctx.racine.join("journal").join("depots.json");
    if j.exists() {
        entrees.push(entree_fichier(&j, "journal/depots.json".to_string())?);
    }
    let cfg = ctx.racine.join("config");
    for nom in ["entreprise.json", "salaries.json", "spectacles.json", "organismes.json", "rubriques.json", "compteurs.json", "pas.json"] {
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
    let mut tous = match crate::routes::lire_json_ou_vide(&ctx.racine.join("config").join("exercices.json"))? {
        Value::Object(o) => o,
        autre => return Err(erreur("TypeError", socle::fichiers::non_modifiable(socle::valeur::nom_type(&autre)))),
    };
    let relatif = archive.strip_prefix(&ctx.racine).unwrap_or(&archive).to_string_lossy().replace('\\', "/");
    tous.insert(
        annee.to_string(),
        json!({
            "le": le, "qui": qui, "archive": relatif, "octets": octets.len(), "empreinte": empreinte,
            "fichiers": n, "forcee": forcer && !sans_dsn.is_empty(),
            "sans_dsn": if forcer { json!(sans_dsn) } else { json!([]) },
        }),
    );
    ecrire_exercices(ctx, &tous)?;
    Ok(json!({
        "annee": annee, "le": le, "archive": archive.to_string_lossy(), "octets": octets.len(),
        "empreinte": empreinte, "fichiers": n, "manques": manques,
    }))
}

/// `operations_exercice.rouvrir_exercice` : retire la cloture. L'archive reste sur le disque : elle dit ce
/// qu'etait l'annee au moment ou on l'a fermee.
fn rouvrir_exercice(ctx: &Contexte, annee: &str, qui: &str) -> R<Value> {
    let mut tous = match crate::routes::lire_json_ou_vide(&ctx.racine.join("config").join("exercices.json"))? {
        Value::Object(o) => o,
        autre => return Err(erreur("TypeError", format!("config/exercices.json : {}", socle::fichiers::attendu("un objet", socle::valeur::nom_type(&autre))))),
    };
    let Some(clos) = tous.shift_remove(annee) else {
        return Err(refus(format!("L'exercice {} n'est pas clos.", annee)));
    };
    let mut rouvert = match &clos {
        Value::Object(o) => o.clone(),
        autre => return Err(erreur("TypeError", socle::fichiers::attendu("un objet", socle::valeur::nom_type(autre)))),
    };
    rouvert.insert("annee".into(), json!(annee));
    rouvert.insert("rouvert_le".into(), json!(maintenant()));
    rouvert.insert("rouvert_par".into(), json!(qui));
    let liste = tous.entry("_rouverts").or_insert_with(|| json!([]));
    match liste {
        Value::Array(a) => a.push(Value::Object(rouvert)),
        autre => return Err(erreur("AttributeError", socle::fichiers::attendu("une liste", socle::valeur::nom_type(autre)))),
    }
    ecrire_exercices(ctx, &tous)?;
    Ok(json!({
        "annee": annee,
        "etait_clos_depuis": clos.get("le").cloned().unwrap_or(Value::Null),
        "archive": clos.get("archive").cloned().unwrap_or(Value::Null),
    }))
}

/// La route `cloture` : `{"annee", "forcer"?, "rouvrir"?, "qui"?}`.
pub(crate) fn cloture_route(ctx: &Contexte, charge: &Value) -> R<Value> {
    let annee = texte_de(exiger(charge, "annee")?);
    // `qui` : la ligne de commande ecrit `cli`, la page `interface`.
    let qui = charge.get("qui").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_else(|| "interface".to_string());
    if charge.get("rouvrir").is_some_and(vrai) {
        return rouvrir_exercice(ctx, &annee, &qui);
    }
    cloturer(ctx, &annee, charge.get("forcer").is_some_and(vrai), &qui)
}
