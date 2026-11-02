//! LE JOURNAL DE CE QUI EST PARTI : `journal/depots.jsonl`, qu'on relit filtre (`lire`), l'empreinte d'un fichier, et ce
//! qui est parti de chaque engagement, sur quelle version (`lu` : une lecture, d'ou `departs`, que `moteur` passe a
//! l'engagement). Les etats du mois, les resumes et le rattachement des retours sont dans `etats`.
//!
//! SA FORME : un enregistrement JSON par ligne, et l'on n'ecrit qu'en ajoutant des lignes a la fin ; ce qui est ecrit
//! ne se reecrit jamais. Trois enregistrements :
//! - un envoi, `{"id", "ligne"}` (`noter`) : `ligne` est ce que le journal lu en montre, `id` l'empreinte de son texte ;
//! - un retour, `{"a", "retour"}` (`rattacher`, `ranger`) : il s'ajoute aux `retours` de l'envoi `a` ;
//! - un complement, `{"a", "nature", "publication", "complete"}` (`ranger`) : ses champs s'ecrivent dans le retour de
//!   l'envoi `a` qui porte cette nature et cette publication (le verdict d'un bilan arrive apres son annonce).
//! `lu` replie ces enregistrements dans l'ordre en `lignes` ; un processus ne relit que ce qui s'est ajoute depuis sa
//! derniere lecture.
//!
//! DEUX JOURNAUX EN CONFLIT (deux postes, une sauvegarde) SE REUNISSENT EN METTANT LEURS LIGNES BOUT A BOUT, dans
//! n'importe quel ordre qui garde celui de chacun : un envoi deja lu (le meme `id`) et un retour deja range
//! (la meme nature et la meme publication, ou le meme texte, sur le meme envoi) ne comptent qu'une fois. Rien ne se
//! perd, rien ne compte double (regle 9 b).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use socle::fichiers::{repr, vrai};
use socle::num::Resultat;

/// Les actes que l'on note ; un autre est refuse.
pub const ACTES: [&str; 8] = [
    "dsn",
    "dpae",
    "documents",
    "contrat-signature",
    "contrat-contresigne",
    "regularisation-casc",
    "dossier-salarie",
    "fiche-demande",
];

pub const REEL: &str = "reel";

/// Le dossier du journal d'une compagnie, sous sa racine : ce qui est parti, les incidents, les messages.
pub const JOURNAL: &str = "journal";

/// Le journal de ce qui est parti, sous la racine de la compagnie.
pub const DEPOTS: &str = "journal/depots.jsonl";

/// `<racine>/journal/depots.jsonl`.
pub fn chemin(racine: &Path) -> PathBuf {
    racine.join(DEPOTS)
}

/// Les 16 premiers caracteres du SHA-256.
pub fn empreinte(contenu: &[u8]) -> String {
    let h = Sha256::digest(contenu);
    h.iter().take(8).map(|o| format!("{:02x}", o)).collect()
}

/// L'empreinte d'un fichier, comme le journal l'écrit.
pub fn empreinte_fichier(chemin: &Path) -> Resultat<String> {
    std::fs::read(chemin).map(|b| empreinte(&b)).map_err(|e| format!("{} : {}", chemin.display(), e))
}

/// LE JOURNAL LU : ses lignes (ce qui est parti et ce qui en est revenu : bilans, contrats signes, anomalies), et ce qui
/// est parti de chaque engagement, sur quelle version (`departs`), la part que le moteur passe a l'engagement. Le
/// processus le tient replie (`Tenu`) : une lecture ne replie que les enregistrements ajoutes depuis la precedente, et
/// ce que le processus ajoute lui-meme se replie sans relire le fichier.
#[derive(Debug, Clone, Default)]
pub struct Journal {
    pub lignes: Arc<Vec<Value>>,
    pub departs: engagement::departs::Departs,
    /// L'`id` de chaque ligne, dans le meme ordre.
    ids: Arc<Vec<String>>,
}

/// Le journal d'un dossier tel que le processus l'a replie : les octets lus jusqu'a la derniere fin de ligne, ce que
/// le systeme de fichiers en disait (taille, date), le journal, et l'indice de chaque `id`.
#[derive(Default)]
struct Tenu {
    octets: Vec<u8>,
    vu: (u64, Option<std::time::SystemTime>),
    journal: Journal,
    index: HashMap<String, usize>,
}

type Tenus = Mutex<HashMap<PathBuf, Tenu>>;

fn tenus() -> std::sync::MutexGuard<'static, HashMap<PathBuf, Tenu>> {
    static TENUS: OnceLock<Tenus> = OnceLock::new();
    TENUS.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

/// Une ligne de journal qui compte pour ce qui est parti malgre ses retours : un envoi au salarie (documents,
/// contrat) ne depend pas de ses retours ; une declaration refusee au bilan n'est pas partie.
fn depend_de_ses_retours(l: &Value) -> Option<bool> {
    matches!(l.get("quoi").and_then(Value::as_str), Some("dsn") | Some("dpae")).then(|| crate::etats::refuse_par_un_retour(l).unwrap_or(false))
}

impl Tenu {
    /// Replie un enregistrement (`n` : sa ligne dans le fichier, pour le dire s'il ne se lit pas).
    fn replier(&mut self, e: &Value, n: usize, p: &Path) -> Resultat<()> {
        let illisible = |quoi: &str| format!("{}, ligne {} : {}", p.display(), n, quoi);
        if let Some(ligne) = e.get("ligne") {
            let id = e.get("id").and_then(Value::as_str).ok_or_else(|| illisible("un envoi sans `id`"))?;
            if self.index.contains_key(id) {
                // Le meme envoi (son `id` est l'empreinte du texte qu'il a note), lu dans deux journaux reunis, ne compte
                // qu'une fois ; ce qui lui est revenu sur chaque poste s'est range a la suite.
                return Ok(());
            }
            noter_ligne(&mut self.journal.departs, ligne);
            self.index.insert(id.to_string(), self.journal.lignes.len());
            Arc::make_mut(&mut self.journal.ids).push(id.to_string());
            Arc::make_mut(&mut self.journal.lignes).push(ligne.clone());
            return Ok(());
        }
        let a = e.get("a").and_then(Value::as_str).ok_or_else(|| illisible("ni un envoi, ni ce qui lui revient"))?;
        let i = *self.index.get(a).ok_or_else(|| illisible("ce qui revient d'un envoi que le journal ne porte pas"))?;
        let lignes = Arc::make_mut(&mut self.journal.lignes);
        let avant = depend_de_ses_retours(&lignes[i]);
        let Value::Object(l) = &mut lignes[i] else { return Err(illisible("un envoi qui n'est pas un objet")) };
        let retours = l.entry("retours").or_insert_with(|| json!([]));
        if retours.is_null() {
            *retours = json!([]);
        }
        let Value::Array(retours) = retours else { return Err(illisible("un envoi dont les retours ne sont pas une liste")) };
        if let Some(r) = e.get("retour") {
            // LE MEME RETOUR, RANGE SUR DEUX POSTES, NE COMPTE QU'UNE FOIS. Un compte rendu d'organisme se reconnait a sa
            // nature et a sa publication (`etats::rattacher_retours` n'en range qu'un par envoi) : ce que l'autre poste en
            // savait de plus (un verdict) le complete ; ce qui revient d'un envoi au salarie, a son texte.
            let meme = |x: &Value| match r.get("nature") {
                Some(n) => x.get("nature").is_some_and(|m| socle::valeur::valeurs_egales(m, n)) && socle::valeur::valeurs_egales(x.get("publication").unwrap_or(&Value::Null), r.get("publication").unwrap_or(&Value::Null)),
                None => x == r,
            };
            match (retours.iter_mut().find(|x| meme(x)), r) {
                (Some(Value::Object(x)), Value::Object(r)) => {
                    for (k, v) in r {
                        x.entry(k.clone()).or_insert_with(|| v.clone());
                    }
                }
                (Some(_), _) => {}
                (None, _) => retours.push(r.clone()),
            }
        } else if let Some(Value::Object(champs)) = e.get("complete") {
            let (nature, publication) = (e.get("nature").unwrap_or(&Value::Null), e.get("publication").unwrap_or(&Value::Null));
            let r = retours
                .iter_mut()
                .find(|r| r.get("nature").unwrap_or(&Value::Null) == nature && r.get("publication").unwrap_or(&Value::Null) == publication)
                .and_then(Value::as_object_mut)
                .ok_or_else(|| illisible("un complement d'un retour que l'envoi ne porte pas"))?;
            for (k, x) in champs {
                r.insert(k.clone(), x.clone());
            }
        } else {
            return Err(illisible("ce qui revient d'un envoi, sans `retour` ni `complete`"));
        }
        if depend_de_ses_retours(&lignes[i]) != avant {
            self.journal.departs = departs_des_lignes(lignes);
        }
        Ok(())
    }

    /// Replie les lignes completes de `octets` (le texte qui suit `self.octets`) ; une ligne que rien ne termine encore
    /// (un ajout en train de s'ecrire) attend la lecture suivante.
    fn replier_le_texte(&mut self, octets: &[u8], p: &Path) -> Resultat<()> {
        let Some(fin) = octets.iter().rposition(|&o| o == b'\n') else { return Ok(()) };
        let mut n = self.octets.iter().filter(|&&o| o == b'\n').count();
        for texte in octets[..fin].split(|&o| o == b'\n') {
            n += 1;
            if texte.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            let e: Value = serde_json::from_slice(texte).map_err(|x| format!("{}, ligne {} : {}", p.display(), n, x))?;
            self.replier(&e, n, p)?;
        }
        self.octets.extend_from_slice(&octets[..=fin]);
        Ok(())
    }
}

/// Le journal de `p` replie a jour de ce que porte le fichier ; `None` s'il n'existe pas.
fn a_jour<'t>(tenus: &'t mut HashMap<PathBuf, Tenu>, p: &Path) -> Resultat<Option<&'t mut Tenu>> {
    let meta = match std::fs::metadata(p) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tenus.remove(p);
            return Ok(None);
        }
        Err(e) => return Err(format!("{} : {}", p.display(), e)),
    };
    let vu = (meta.len(), meta.modified().ok());
    if tenus.get(p).is_some_and(|t| t.vu == vu) {
        return Ok(tenus.get_mut(p));
    }
    let octets = std::fs::read(p).map_err(|e| format!("{} : {}", p.display(), e))?;
    // Ce qui a ete lu reste en tete du fichier, puisqu'on n'y fait qu'ajouter : seul le reste se replie. Un fichier
    // qui ne commence plus par ce qui a ete lu (un journal remplace) se replie en entier.
    let mut t = match tenus.remove(p) {
        Some(t) if octets.starts_with(&t.octets) => t,
        _ => Tenu::default(),
    };
    let debut = t.octets.len();
    t.replier_le_texte(&octets[debut..], p)?;
    t.vu = vu;
    Ok(Some(tenus.entry(p.to_path_buf()).or_insert(t)))
}

/// `journal/depots.jsonl` lu (`Journal`) ; vide s'il n'existe pas.
pub fn lu(racine: &Path) -> Resultat<Journal> {
    let p = chemin(racine);
    let j = match a_jour(&mut tenus(), &p)? {
        Some(t) => t.journal.clone(),
        None => return Ok(Journal::default()),
    };
    socle::fichiers::noter_si_besoin(&p);
    Ok(j)
}

/// LE SEUL ENDROIT QUI ECRIT LE JOURNAL : les enregistrements que `faire` tire du journal a jour s'ajoutent a la fin du
/// fichier, d'une seule ecriture, et se replient tels qu'ils se reliront.
fn ajouter(racine: &Path, faire: impl FnOnce(&Journal, &HashMap<String, usize>) -> Resultat<Vec<Value>>) -> Resultat<()> {
    let p = chemin(racine);
    let mut tenus = tenus();
    let vide = Tenu::default();
    let (j, index) = match a_jour(&mut tenus, &p)? {
        Some(t) => (&t.journal, &t.index),
        None => (&vide.journal, &vide.index),
    };
    let enregistrements = faire(j, index)?;
    if enregistrements.is_empty() {
        return Ok(());
    }
    let mut texte = String::new();
    for e in &enregistrements {
        texte.push_str(&serde_json::to_string(e).map_err(|x| x.to_string())?);
        texte.push('\n');
    }
    if let Some(dossier) = p.parent() {
        std::fs::create_dir_all(dossier).map_err(|e| format!("{} : {}", dossier.display(), e))?;
    }
    {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&p).map_err(|e| format!("{} : {}", p.display(), e))?;
        f.write_all(texte.as_bytes()).map_err(|e| format!("{} : {}", p.display(), e))?;
    }
    let meta = std::fs::metadata(&p).map_err(|e| format!("{} : {}", p.display(), e))?;
    let mut t = tenus.remove(&p).unwrap_or_default();
    // Un autre processus a pu ajouter entre la lecture et l'ecriture : le journal se relira alors du fichier.
    if t.octets.len() + texte.len() == meta.len() as usize {
        t.replier_le_texte(texte.as_bytes(), &p)?;
        t.vu = (meta.len(), meta.modified().ok());
        tenus.insert(p, t);
    }
    Ok(())
}

/// Ne garde du journal de `racine` que les envois que `garder` retient, et ce qui leur est revenu : le journal d'une
/// compagnie montee depuis une autre (`moteur::compagnie::essai`). Le fichier se remplace d'un coup.
pub fn ne_garder_que(racine: &Path, garder: impl Fn(&Value) -> bool) -> Resultat<()> {
    let p = chemin(racine);
    let Ok(octets) = std::fs::read(&p) else { return Ok(()) };
    let mut gardes: std::collections::HashSet<String> = Default::default();
    let mut texte = String::new();
    for (n, l) in octets.split(|&o| o == b'\n').enumerate() {
        if l.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let e: Value = serde_json::from_slice(l).map_err(|x| format!("{}, ligne {} : {}", p.display(), n + 1, x))?;
        let garde = match (e.get("ligne"), e.get("id").and_then(Value::as_str), e.get("a").and_then(Value::as_str)) {
            (Some(ligne), Some(id), _) => garder(ligne) && gardes.insert(id.to_string()),
            (_, _, Some(a)) => gardes.contains(a),
            _ => false,
        };
        if garde {
            texte.push_str(std::str::from_utf8(l).map_err(|x| x.to_string())?);
            texte.push('\n');
        }
    }
    let provisoire = socle::ecriture::provisoire(&p);
    std::fs::write(&provisoire, texte).map_err(|e| format!("{} : {}", provisoire.display(), e))?;
    std::fs::rename(&provisoire, &p).map_err(|e| format!("{} : {}", p.display(), e))?;
    tenus().remove(&p);
    Ok(())
}

thread_local! {
    /// L'heure et l'origine que portent les lignes notees pendant `dater_pendant`.
    static DATE: std::cell::RefCell<Option<(String, String)>> = const { std::cell::RefCell::new(None) };
}

/// `f`, chaque ligne qu'il note portant `quand` et `origine` : un pas que l'horloge d'une compagnie fictive joue a sa
/// date (`moteur::horloge`).
pub fn dater_pendant<T>(quand: &str, origine: &str, f: impl FnOnce() -> T) -> T {
    let avant = DATE.with(|d| d.replace(Some((quand.to_string(), origine.to_string()))));
    let r = f();
    DATE.with(|d| *d.borrow_mut() = avant);
    r
}

/// Les lignes, les plus recentes en dernier. Un filtre vide
/// ne filtre pas.
pub fn lire(racine: &Path, quoi: Option<&str>, mois: Option<&str>, reel_seulement: bool) -> Resultat<Vec<Value>> {
    let mut lignes = (*lu(racine)?.lignes).clone();
    let egal = |l: &Value, c: &str, v: &str| l.get(c).and_then(Value::as_str) == Some(v);
    if let Some(q) = quoi.filter(|q| !q.is_empty()) {
        lignes.retain(|l| egal(l, "quoi", q));
    }
    if let Some(m) = mois.filter(|m| !m.is_empty()) {
        lignes.retain(|l| egal(l, "mois", m));
    }
    if reel_seulement {
        lignes.retain(|l| egal(l, "mode", REEL));
    }
    Ok(lignes)
}

/// Les arguments de `journal.noter`.
pub struct Note<'a> {
    pub quoi: &'a str,
    pub mois: Option<&'a str>,
    pub fichier: Option<&'a str>,
    pub mode: &'a str,
    pub qui: &'a str,
    pub accepte: bool,
    pub destinataire: &'a str,
    pub flux: &'a str,
    pub reponse: &'a str,
    pub trace: Option<&'a str>,
    pub detail: Option<&'a Value>,
    /// Pour une ligne d'envoi : la version de chaque engagement qu'elle couvre (`versions_du_mois`).
    pub versions: Option<&'a Value>,
    /// L'heure de la ligne (ISO, a la seconde) ; `origine` doit alors la justifier.
    pub quand: &'a str,
    pub origine: &'a str,
}

/// Ajoute une ligne a la fin du fichier et la rend. `quand`
/// est l'heure locale que l'appelant donne (le moteur ne connait pas le
/// fuseau) ; vide, elle est lue dans `maintenant`.
pub fn noter(racine: &Path, n: &Note, maintenant: &str) -> Resultat<Value> {
    if !ACTES.contains(&n.quoi) {
        return Err(format!(
            "acte inconnu : {} (attendu {})",
            repr(n.quoi),
            ACTES.join(", ")
        ));
    }
    if !n.quand.is_empty() && n.origine.is_empty() {
        return Err("Une ligne datee doit dire d'ou vient sa date : passer `origine`. Sans ca, rien ne distingue une reprise d'une ligne fabriquee.".to_string());
    }
    let fichier = n.fichier.filter(|f| !f.is_empty());
    let nom = fichier.map(|f| Path::new(f).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
    let empreinte = match fichier {
        Some(f) if Path::new(f).exists() => Some(empreinte_fichier(Path::new(f))?),
        _ => None,
    };
    let mut l = Map::new();
    l.insert("quoi".into(), json!(n.quoi));
    let (quand, origine) = DATE.with(|d| d.borrow().clone()).unwrap_or_else(|| ((if n.quand.is_empty() { maintenant } else { n.quand }).to_string(), n.origine.to_string()));
    l.insert("quand".into(), json!(quand));
    l.insert("origine".into(), json!(origine));
    l.insert("mois".into(), json!(n.mois));
    l.insert("fichier".into(), json!(nom));
    l.insert("empreinte".into(), json!(empreinte));
    l.insert("mode".into(), json!(n.mode));
    l.insert("qui".into(), json!(n.qui));
    l.insert("accepte".into(), json!(n.accepte));
    l.insert("destinataire".into(), json!(n.destinataire));
    l.insert("flux".into(), json!(n.flux));
    l.insert("retours".into(), json!([]));
    l.insert("reponse".into(), json!(n.reponse));
    l.insert("trace".into(), json!(n.trace.filter(|t| !t.is_empty())));
    l.insert("detail".into(), match n.detail {
        Some(d) if vrai(d) => d.clone(),
        _ => json!({}),
    });
    if let Some(v) = n.versions {
        l.insert("versions".into(), v.clone());
    }
    let ligne = Value::Object(l);
    ajouter(racine, |_, index| {
        // L'empreinte du texte de la ligne ; deux lignes au meme texte se distinguent par leur rang.
        let base = self::empreinte(serde_json::to_string(&ligne).map_err(|x| x.to_string())?.as_bytes());
        let id = std::iter::once(base.clone()).chain((2..).map(|k| format!("{}-{}", base, k))).find(|id| !index.contains_key(id)).unwrap_or(base);
        Ok(vec![json!({"id": id, "ligne": ligne})])
    })?;
    Ok(ligne)
}

/// CE QUI REVIENT D'UN ENVOI (un contrat signe, la mention retiree) : `retour` se range dans les `retours` de la derniere
/// ligne que `envoi` reconnait, et la rend. `None` : aucune ligne ne la porte, rien n'est ecrit.
pub fn rattacher(racine: &Path, envoi: impl Fn(&Value) -> bool, retour: Value) -> Resultat<Option<Value>> {
    let mut trouvee = None;
    ajouter(racine, |j, _| {
        let Some(i) = j.lignes.iter().rposition(|l| envoi(l)) else { return Ok(Vec::new()) };
        trouvee = Some(i);
        Ok(vec![json!({"a": j.ids[i], "retour": retour})])
    })?;
    Ok(match trouvee {
        Some(i) => lu(racine)?.lignes.get(i).cloned(),
        None => None,
    })
}

/// Ce qui se range sur une ligne du journal (`ranger`).
pub enum Rangement {
    /// Un retour de plus.
    Retour(Value),
    /// Les champs qui completent le retour de cette nature et de cette publication.
    Complement { nature: Value, publication: Value, champs: Map<String, Value> },
}

/// RANGE CE QUI REVIENT des envois : chaque `(i, rangement)` sur la `i`-ieme ligne du journal lu (`lu`), dans l'ordre.
pub fn ranger(racine: &Path, rangements: Vec<(usize, Rangement)>) -> Resultat<()> {
    ajouter(racine, |j, _| {
        rangements
            .into_iter()
            .map(|(i, r)| {
                let a = j.ids.get(i).ok_or_else(|| format!("journal : aucune ligne {}", i))?;
                Ok(match r {
                    Rangement::Retour(retour) => json!({"a": a, "retour": retour}),
                    Rangement::Complement { nature, publication, champs } => json!({"a": a, "nature": nature, "publication": publication, "complete": champs}),
                })
            })
            .collect()
    })
}

/// Ce que les lignes `lignes` du journal disent de ce qui est parti (`departs`) : les lignes qui precedent un envoi
/// disent ce qui etait parti avant lui.
pub fn departs_des_lignes(lignes: &[Value]) -> engagement::departs::Departs {
    let mut d = engagement::departs::Departs::default();
    for l in lignes {
        noter_ligne(&mut d, l);
    }
    d
}

/// Ce que la ligne `l` dit de ce qui est parti, a la suite de `d`.
fn noter_ligne(d: &mut engagement::departs::Departs, l: &Value) {
    use engagement::departs::{Depart, Envoi};
    {
        let envoi = match l.get("quoi").and_then(Value::as_str) {
            Some("contrat-signature") => Envoi::Contrat,
            Some("dpae") => Envoi::Dpae,
            Some("documents") => Envoi::Documents,
            Some("dsn") if l.get("detail").and_then(|d| d.get("nature")).and_then(Value::as_str).is_some_and(|n| declarations::norme::SIGNALEMENTS.contains(&n)) => {
                Envoi::Signalement
            }
            Some("dsn") => Envoi::Dsn,
            _ => return,
        };
        if l.get("mode").and_then(Value::as_str) != Some(REEL) {
            return;
        }
        // Une declaration refusee au depot ou au bilan n'a rien depose (`etats::refuse_par_un_retour`). Un courriel coupe en
        // route a servi ceux qui l'ont recu : la ligne ne porte que leurs versions (`ligne_d_envoi::noter_echec_d_envoi`),
        // et ce qui a atteint le salarie est parti.
        let au_salarie = matches!(envoi, Envoi::Documents | Envoi::Contrat);
        if !au_salarie && (l.get("accepte") == Some(&Value::Bool(false)) || crate::etats::refuse_par_un_retour(l).unwrap_or(false)) {
            return;
        }
        let mois = matches!(envoi, Envoi::Documents | Envoi::Dsn).then(|| l.get("mois").and_then(Value::as_str).map(String::from)).flatten();
        let le = l.get("quand").and_then(Value::as_str).unwrap_or_default().to_string();
        // Un contrat parti sans que sa ligne dise sur quelle version (une ligne qui ne porte pas `versions`) est parti sur
        // la premiere : une validation qui ne dit pas sa version s'ecrit en version 1 (`engagement::etats`). Il se range par
        // le numero que la ligne nomme, comme les autres.
        let sans_versions: Vec<Value> = match (envoi, l.get("versions")) {
            (Envoi::Contrat, None) => l
                .pointer("/detail/contrats")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|c| json!({"salarie": c.get("salarie"), "numero": c.get("numero").map(socle::valeur::valeur_en_texte), "version": 1}))
                .collect(),
            _ => Vec::new(),
        };
        for v in l.get("versions").and_then(Value::as_array).into_iter().flatten().chain(&sans_versions) {
            let (Some(cle), Some(numero), Some(version)) =
                (v.get("salarie").and_then(Value::as_str), v.get("numero").and_then(Value::as_str), v.get("version").and_then(Value::as_u64))
            else {
                continue;
            };
            // La correction d'un mois passe que la DSN porte : sa version est celle de l'engagement du mois corrige.
            let corrige = (envoi == Envoi::Dsn).then(|| v.get("mois").and_then(Value::as_str).map(String::from)).flatten().filter(|m| Some(m) != mois.as_ref());
            let (m, par) = match corrige {
                Some(c) => (Some(c), mois.clone()),
                None => (mois.clone(), None),
            };
            // Un avenant : la ligne du journal le dit (`acte`), et son contrat porte son numero.
            let avenant = (envoi == Envoi::Contrat && l.pointer("/detail/acte").and_then(Value::as_str) == Some("avenant"))
                .then(|| {
                    l.pointer("/detail/contrats")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .find(|c| c.get("salarie").and_then(Value::as_str) == Some(cle) && c.get("numero").and_then(Value::as_str) == Some(numero))
                        .and_then(|c| c.get("avenant"))
                        .map(socle::valeur::valeur_en_texte)
                })
                .flatten();
            d.noter(cle, numero, Depart { envoi, mois: m, par, avenant, version, le: le.clone() });
        }
    }
}

/// LES VERSIONS QU'UN ENVOI EMPORTE : pour chaque engagement valide que `mois` montre et que `garder` retient (la cle du
/// salarie, le numero du contrat, l'engagement), `{"salarie", "numero", "version"}`. Ce que la ligne du journal de l'envoi
/// porte : elles se lisent AVANT l'envoi, qui ne part pas d'un engagement a revalider (`Valide::a_revalider`).
pub fn versions_du_mois(ctx: &droit::contexte::Contexte, mois: &str, departs: &engagement::departs::Departs, garder: impl Fn(&str, &str, &Value) -> bool) -> Resultat<Value> {
    let mut out = Vec::new();
    for e in engagement::etats::du_mois(ctx, mois, departs)? {
        let Some(v) = e.valide() else { continue };
        let brut = &*v.objet().brut;
        let (Some(cle), Some(numero)) = (engagement::cle_de_l_engagement(ctx, &brut), brut.get("numero").and_then(Value::as_str)) else { continue };
        if garder(&cle, numero, &brut) {
            if let Some(phrase) = v.a_revalider(ctx) {
                return Err(phrase);
            }
            let version = v.version().ok_or("validation illisible : `version`")?;
            out.push(json!({"salarie": cle, "numero": numero, "version": version}));
        }
    }
    Ok(Value::Array(out))
}

/// `versions_du_mois` des contrats qui partent (`[{"salarie", "numero", "mois"}]`, chacun lu dans le mois qu'il nomme).
pub fn versions_des_contrats(ctx: &droit::contexte::Contexte, contrats: &[Value], departs: &engagement::departs::Departs) -> Resultat<Value> {
    let mut mois: Vec<String> = contrats.iter().filter_map(|c| c.get("mois").and_then(Value::as_str).map(String::from)).collect();
    mois.sort();
    mois.dedup();
    let mut out = Vec::new();
    for m in mois {
        let siens = |cle: &str, n: &str, _: &Value| {
            contrats.iter().any(|c| c["mois"].as_str() == Some(m.as_str()) && c["salarie"].as_str() == Some(cle) && c["numero"].as_str() == Some(n))
        };
        if let Value::Array(v) = versions_du_mois(ctx, &m, departs, siens)? {
            for x in v {
                if !out.contains(&x) {
                    out.push(x);
                }
            }
        }
    }
    Ok(Value::Array(out))
}

/// Le plus grand numero d'ordre (`S20.G00.05.004`) des signalements REELS deja envoyes par la compagnie, tous
/// evenements confondus (arret, reprise, fin de contrat, amorcage) ; 0 s'il n'y en a aucun.
///
/// Cahier technique DSN 2026.1, p. 136 : « Pour un signalement, ce numero ne doit pas etre remis a zero afin
/// qu'il soit unique quel que soit le mois d'emission » ; p. 14 : un second signalement normal au meme SIRET et
/// au meme numero d'ordre n'est pas accepte. Le numero se LIT de ce qui est parti (`detail.ordre` des lignes
/// `dsn` reelles du journal), il ne se devine pas : une ligne reelle acceptee qui ne dit pas son ordre refuse.
/// Un signalement se reconnait a `detail.nature` (04, 05, 07, 08) ; une ligne plus ancienne qui ne la porte pas,
/// a son `mois` : une mensuelle se date `AAAA-MM`, un signalement par son evenement (`AAAA-MM-JJ`). Une ligne
/// non acceptee (le point de depot n'a rien recu) ne consomme pas de numero.
pub fn dernier_ordre_des_signalements(racine: &Path) -> Resultat<i64> {
    let mut dernier = 0;
    for l in lire(racine, Some("dsn"), None, true)? {
        let detail = l.get("detail").cloned().unwrap_or(Value::Null);
        let signalement = match detail.get("nature").and_then(Value::as_str).filter(|n| !n.is_empty()) {
            Some(n) => declarations::norme::SIGNALEMENTS.contains(&n),
            None => {
                let m: Vec<char> = l.get("mois").and_then(Value::as_str).unwrap_or("").chars().collect();
                !(m.len() == 7 && m[4] == '-' && m.iter().enumerate().all(|(i, c)| i == 4 || c.is_ascii_digit()))
            }
        };
        if !signalement {
            continue;
        }
        let ordre = match detail.get("ordre") {
            Some(Value::Number(n)) => n.as_i64(),
            Some(Value::String(s)) => s.trim().parse::<i64>().ok(),
            _ => None,
        };
        match ordre {
            Some(o) => dernier = dernier.max(o),
            None if l.get("accepte").map(vrai).unwrap_or(false) => {
                return Err(format!(
                    "Le journal des envois ({}) note le signalement reel du {} ({}) sans son numero d'ordre : l'ecrire dans le `detail` de cette ligne (`\"ordre\": N`, N lu en S20.G00.05.004 du fichier envoye). Le numero d'ordre d'un signalement ne se remet jamais a zero (cahier technique DSN, p. 136) : sans lui, le suivant ne se numerote pas. Rien n'est ecrit.",
                    chemin(racine).display(),
                    l.get("quand").and_then(Value::as_str).unwrap_or("?"),
                    l.get("fichier").and_then(Value::as_str).unwrap_or("?"),
                ))
            }
            None => {}
        }
    }
    Ok(dernier)
}


#[cfg(test)]
mod tests {
    use super::*;

    /// Bug reel : une DSN ou une DPAE refusee au bilan comptait pour partie ; le mois se figeait et sa correction allait a
    /// la DSN suivante. Rien n'a ete recu (cahier technique DSN, § 4.2, `norme-dsn/texte/chapitre-4-controles.txt`).
    #[test]
    fn une_declaration_refusee_au_bilan_n_est_pas_partie() {
        let ligne = |quoi: &str, salarie: &str, retours: Value, detail: Value| {
            json!({"quoi": quoi, "mode": REEL, "accepte": true, "mois": "2026-06", "quand": "2026-07-05T10:00:00", "detail": detail,
                   "retours": retours, "versions": [{"salarie": salarie, "numero": "N1", "version": 1}]})
        };
        let ko = json!([{"nature": "10", "verdict": "OK"}, {"nature": "11", "verdict": "KO"}]);
        let lignes = vec![
            ligne("dsn", "ALPHA", ko.clone(), json!({})),
            ligne("dsn", "BETA", json!([]), json!({"reconstruit": true, "etat_declaration": "KO"})),
            ligne("dpae", "GAMMA", ko, json!({})),
            ligne("dsn", "DELTA", json!([{"nature": "11", "verdict": "OK"}]), json!({})),
        ];
        let d = departs_des_lignes(&lignes);
        for refusee in ["ALPHA", "BETA", "GAMMA"] {
            assert!(d.de(refusee, "N1").is_empty(), "{refusee}");
        }
        assert_eq!(d.de("DELTA", "N1").len(), 1);
    }
}
