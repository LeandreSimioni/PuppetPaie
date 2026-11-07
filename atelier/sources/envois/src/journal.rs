//! LE JOURNAL DE CE QUI EST PARTI : `journal/depots.jsonl`, qu'on relit filtre (`lire`), l'empreinte d'un fichier, et ce
//! qui est parti de chaque engagement, sur quelle version (`lu` : une lecture, d'ou `departs`, que `moteur` passe a
//! l'engagement). Les etats du mois, les resumes et le rattachement des retours sont dans `etats` ; la forme d'une ligne,
//! dans `ligne`.
//!
//! SA FORME : un enregistrement JSON par ligne (`Enregistrement`), et l'on n'ecrit qu'en ajoutant des lignes a la fin ;
//! ce qui est ecrit ne se reecrit jamais. Trois enregistrements :
//! - un envoi, `{"id", "ligne"}` (`noter`) : `ligne` est ce que le journal lu en montre (`ligne::Ligne`), `id`
//!   l'empreinte de son texte ;
//! - un retour, `{"a", "retour"}` (`rattacher`, `ranger`) : il s'ajoute aux `retours` de l'envoi `a`, dans la forme de
//!   sa sorte (`ligne::RetourOrganisme`, `RetourContrat`, `RetourFiche`) ;
//! - un complement, `{"a", "nature", "publication", "complete"}` (`ranger`) : le verdict (`ligne::Verdict`) s'ecrit dans
//!   le retour de l'envoi `a` qui porte cette nature et cette publication (le verdict d'un bilan arrive apres son annonce).
//! `lu` replie ces enregistrements dans l'ordre en `lignes` ; un processus ne relit que ce qui s'est ajoute depuis sa
//! derniere lecture. Un enregistrement qui ne se lit pas strictement (une cle inconnue, une valeur d'une autre forme)
//! arrete la lecture, sa ligne et sa cle nommees.
//!
//! DEUX JOURNAUX EN CONFLIT (deux postes, une sauvegarde) SE REUNISSENT EN METTANT LEURS LIGNES BOUT A BOUT, dans
//! n'importe quel ordre qui garde celui de chacun : un envoi deja lu (le meme `id`) et un retour deja range
//! (la meme nature et la meme publication, ou le meme retour, sur le meme envoi) ne comptent qu'une fois. Rien ne se
//! perd, rien ne compte double (regle 9 b).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use socle::mois::Mois;
use socle::num::Resultat;

pub use crate::ligne::*;

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
    std::fs::read(chemin).map(|b| empreinte(&b)).map_err(|e| socle::echec::du_disque(&chemin, &e))
}

/// UN ENREGISTREMENT DU FICHIER : un envoi (`id`, `ligne`), un retour (`a`, `retour`) ou un complement (`a`, `nature`,
/// `publication`, `complete`). Le retour se lit dans la forme de la sorte de son envoi (`Tenu::replier`).
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Enregistrement {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ligne: Option<Ligne>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    a: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    retour: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    nature: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    publication: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    complete: Option<Verdict>,
}

/// LE JOURNAL LU : ses lignes (ce qui est parti et ce qui en est revenu : bilans, contrats signes, anomalies), et ce qui
/// est parti de chaque engagement, sur quelle version (`departs`), la part que le moteur passe a l'engagement. Le
/// processus le tient replie (`Tenu`) : une lecture ne replie que les enregistrements ajoutes depuis la precedente, et
/// ce que le processus ajoute lui-meme se replie sans relire le fichier.
#[derive(Debug, Clone, Default)]
pub struct Journal {
    pub lignes: Arc<Vec<Ligne>>,
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
fn depend_de_ses_retours(l: &Ligne) -> Option<bool> {
    matches!(l.acte(), Acte::Dsn | Acte::Dpae).then(|| crate::etats::refuse_par_un_retour(l))
}

/// La phrase d'un journal qui ne se lit pas, sa ligne nommee.
fn abime(p: &Path, n: usize) -> String {
    format!("Le journal des envois de la compagnie ne se lit pas : sa ligne {} est abîmée ({}).", n, p.display())
}

/// Le retour `r` lu dans la forme `T` de la sorte de son envoi.
fn retour_de<T: serde::de::DeserializeOwned>(r: &serde_json::Value, p: &Path, n: usize) -> Resultat<T> {
    socle::forme::lire(r, &abime(p, n))
}

impl Tenu {
    /// Replie un enregistrement (`n` : sa ligne dans le fichier, pour le dire s'il ne se lit pas).
    fn replier(&mut self, e: Enregistrement, n: usize, p: &Path) -> Resultat<()> {
        let illisible = |quoi: &str| format!("{} : {}.", abime(p, n), quoi);
        match e {
            Enregistrement { id: Some(id), ligne: Some(ligne), a: None, retour: None, nature: None, publication: None, complete: None } => {
                if self.index.contains_key(&id) {
                    // Le meme envoi (son `id` est l'empreinte du texte qu'il a note), lu dans deux journaux reunis, ne compte
                    // qu'une fois ; ce qui lui est revenu sur chaque poste s'est range a la suite.
                    return Ok(());
                }
                noter_ligne(&mut self.journal.departs, &ligne);
                self.index.insert(id.clone(), self.journal.lignes.len());
                Arc::make_mut(&mut self.journal.ids).push(id);
                Arc::make_mut(&mut self.journal.lignes).push(ligne);
                Ok(())
            }
            Enregistrement { id: None, ligne: None, a: Some(a), retour, nature, publication, complete } => {
                let i = *self.index.get(&a).ok_or_else(|| illisible("ce qui revient d'un envoi que le journal ne porte pas"))?;
                let lignes = Arc::make_mut(&mut self.journal.lignes);
                let avant = depend_de_ses_retours(&lignes[i]);
                match (retour, complete) {
                    (Some(r), None) if nature.is_none() && publication.is_none() => match &mut lignes[i] {
                        // LE MEME RETOUR, RANGE SUR DEUX POSTES, NE COMPTE QU'UNE FOIS. Un compte rendu d'organisme se
                        // reconnait a sa nature et a sa publication (`etats::rattacher_retours` n'en range qu'un par envoi) :
                        // ce que l'autre poste en savait de plus (un verdict) le complete ; ce qui revient d'un envoi au
                        // salarie, a lui-meme.
                        Ligne::Dsn(l) => ranger_organisme(&mut l.retours, retour_de(&r, p, n)?),
                        Ligne::Dpae(l) => ranger_organisme(&mut l.retours, retour_de(&r, p, n)?),
                        Ligne::ContratSignature(l) => ranger_une_fois(&mut l.retours, retour_de(&r, p, n)?),
                        Ligne::FicheDemande(l) => ranger_une_fois(&mut l.retours, retour_de(&r, p, n)?),
                        _ => return Err(illisible("un retour sur un envoi dont rien ne revient")),
                    },
                    (None, Some(v)) => {
                        let retours = match &mut lignes[i] {
                            Ligne::Dsn(l) => &mut l.retours,
                            Ligne::Dpae(l) => &mut l.retours,
                            _ => return Err(illisible("le complement d'un retour sur un envoi dont rien ne revient")),
                        };
                        let nature = nature.unwrap_or_default();
                        let r = retours
                            .iter_mut()
                            .find(|r| r.nature == nature && r.publication == publication)
                            .ok_or_else(|| illisible("un complement d'un retour que l'envoi ne porte pas"))?;
                        r.poser(v);
                    }
                    _ => return Err(illisible("ce qui revient d'un envoi, sans `retour` ni `complete`")),
                }
                if depend_de_ses_retours(&lignes[i]) != avant {
                    self.journal.departs = departs_des_lignes(lignes);
                }
                Ok(())
            }
            _ => Err(illisible("ni un envoi (`id`, `ligne`), ni ce qui lui revient (`a`)")),
        }
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
            let e = lire_enregistrement(texte, p, n)?;
            self.replier(e, n, p)?;
        }
        self.octets.extend_from_slice(&octets[..=fin]);
        Ok(())
    }
}

/// L'enregistrement de la ligne `n` du fichier `p`, lu strictement.
fn lire_enregistrement(texte: &[u8], p: &Path, n: usize) -> Resultat<Enregistrement> {
    let texte = std::str::from_utf8(texte).map_err(|e| format!("{} : {}.", abime(p, n), e))?;
    socle::forme::lire_texte(texte, &abime(p, n))
}

/// Un compte rendu d'organisme de plus, ou ce qu'il apporte au meme (sa nature, sa publication) deja range.
fn ranger_organisme(retours: &mut Vec<RetourOrganisme>, r: RetourOrganisme) {
    match retours.iter_mut().find(|x| x.meme(&r)) {
        Some(x) => x.completer(&r),
        None => retours.push(r),
    }
}

/// Un retour de plus, s'il n'est pas deja range.
fn ranger_une_fois<T: PartialEq>(retours: &mut Vec<T>, r: T) {
    if !retours.contains(&r) {
        retours.push(r);
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
        Err(e) => return Err(socle::echec::du_disque(&p, &e)),
    };
    let vu = (meta.len(), meta.modified().ok());
    if tenus.get(p).is_some_and(|t| t.vu == vu) {
        return Ok(tenus.get_mut(p));
    }
    let octets = std::fs::read(p).map_err(|e| socle::echec::du_disque(&p, &e))?;
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
fn ajouter(racine: &Path, faire: impl FnOnce(&Journal, &HashMap<String, usize>) -> Resultat<Vec<Enregistrement>>) -> Resultat<()> {
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
        std::fs::create_dir_all(dossier).map_err(|e| socle::echec::du_disque(&dossier, &e))?;
    }
    {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&p).map_err(|e| socle::echec::du_disque(&p, &e))?;
        f.write_all(texte.as_bytes()).map_err(|e| socle::echec::du_disque(&p, &e))?;
    }
    let meta = std::fs::metadata(&p).map_err(|e| socle::echec::du_disque(&p, &e))?;
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
pub fn ne_garder_que(racine: &Path, garder: impl Fn(&Ligne) -> bool) -> Resultat<()> {
    let p = chemin(racine);
    let Ok(octets) = std::fs::read(&p) else { return Ok(()) };
    let mut gardes: std::collections::HashSet<String> = Default::default();
    let mut texte = String::new();
    for (n, l) in octets.split(|&o| o == b'\n').enumerate() {
        if l.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let e = lire_enregistrement(l, &p, n + 1)?;
        let garde = match (&e.ligne, &e.id, &e.a) {
            (Some(ligne), Some(id), _) => garder(ligne) && gardes.insert(id.clone()),
            (_, _, Some(a)) => gardes.contains(a),
            _ => false,
        };
        if garde {
            texte.push_str(std::str::from_utf8(l).map_err(|x| x.to_string())?);
            texte.push('\n');
        }
    }
    let provisoire = socle::ecriture::provisoire(&p);
    std::fs::write(&provisoire, texte).map_err(|e| socle::echec::du_disque(&provisoire, &e))?;
    std::fs::rename(&provisoire, &p).map_err(|e| socle::echec::du_disque(&p, &e))?;
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

/// Les lignes de `lignes` d'une sorte (`quoi`), d'un mois, reelles seulement ; un filtre absent ne filtre pas.
pub fn filtrer<'a>(lignes: &'a [Ligne], quoi: Option<Acte>, mois: Option<Mois>, reel_seulement: bool) -> impl DoubleEndedIterator<Item = &'a Ligne> {
    lignes
        .iter()
        .filter(move |l| quoi.is_none_or(|q| l.acte() == q) && mois.is_none_or(|m| l.mois() == Some(m)) && (!reel_seulement || l.reel()))
}

/// Les lignes du journal de `racine`, les plus recentes en dernier (`filtrer`).
pub fn lire(racine: &Path, quoi: Option<Acte>, mois: Option<Mois>, reel_seulement: bool) -> Resultat<Vec<Ligne>> {
    Ok(filtrer(&lu(racine)?.lignes, quoi, mois, reel_seulement).cloned().collect())
}

impl<D, R> Envoi<D, R> {
    /// L'envoi que `qui` fait maintenant, dans `mode` : `noter` le date, et y lit le nom et l'empreinte du fichier.
    pub fn nouveau(mode: Mode, qui: &str, accepte: bool, destinataire: &str, reponse: &str, detail: D) -> Envoi<D, R> {
        Envoi {
            quand: String::new(),
            origine: String::new(),
            mois: None,
            fichier: None,
            empreinte: None,
            mode,
            qui: qui.to_string(),
            accepte,
            destinataire: destinataire.to_string(),
            flux: String::new(),
            retours: Vec::new(),
            reponse: reponse.to_string(),
            trace: None,
            detail,
            versions: None,
        }
    }
}

/// AJOUTE LA LIGNE `ligne` A LA FIN DU FICHIER ET LA REND, telle qu'elle se relira. Son heure est celle qu'elle porte
/// (une reprise : `origine` doit alors la justifier), celle de `dater_pendant`, sinon `maintenant` (l'heure locale que
/// l'appelant donne : le moteur ne connait pas le fuseau). `fichier` : le fichier parti, dont la ligne garde le nom et
/// l'empreinte (s'il existe).
pub fn noter(racine: &Path, mut ligne: Ligne, fichier: Option<&Path>, maintenant: &str) -> Resultat<Ligne> {
    let (quand, origine) = commun_mut(&mut ligne);
    if !quand.is_empty() && origine.is_empty() {
        return Err(socle::echec::cle("Une ligne datée du journal doit dire d'où vient sa date : sans cela, rien ne distingue une reprise d'une ligne fabriquée.", "origine"));
    }
    let (q, o) = DATE.with(|d| d.borrow().clone()).unwrap_or_else(|| ((if quand.is_empty() { maintenant.to_string() } else { quand.clone() }), origine.clone()));
    *quand = q;
    *origine = o;
    if let Some(f) = fichier.filter(|f| !f.as_os_str().is_empty()) {
        let nom = f.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let e = if f.exists() { Some(empreinte_fichier(f)?) } else { None };
        poser_fichier(&mut ligne, nom, e);
    }
    let ecrite = ligne.clone();
    ajouter(racine, |_, index| {
        // L'empreinte du texte de la ligne ; deux lignes au meme texte se distinguent par leur rang.
        let base = self::empreinte(serde_json::to_string(&ligne).map_err(|x| x.to_string())?.as_bytes());
        let id = std::iter::once(base.clone()).chain((2..).map(|k| format!("{}-{}", base, k))).find(|id| !index.contains_key(id)).unwrap_or(base);
        Ok(vec![Enregistrement { id: Some(id), ligne: Some(ligne), ..Default::default() }])
    })?;
    Ok(ecrite)
}

/// L'heure et l'origine de la ligne, a poser.
fn commun_mut(l: &mut Ligne) -> (&mut String, &mut String) {
    match l {
        Ligne::Dsn(e) => (&mut e.quand, &mut e.origine),
        Ligne::Dpae(e) => (&mut e.quand, &mut e.origine),
        Ligne::Documents(e) => (&mut e.quand, &mut e.origine),
        Ligne::ContratSignature(e) => (&mut e.quand, &mut e.origine),
        Ligne::ContratContresigne(e) => (&mut e.quand, &mut e.origine),
        Ligne::RegularisationCasc(e) => (&mut e.quand, &mut e.origine),
        Ligne::DossierSalarie(e) => (&mut e.quand, &mut e.origine),
        Ligne::FicheDemande(e) => (&mut e.quand, &mut e.origine),
    }
}

fn poser_fichier(l: &mut Ligne, nom: String, e: Option<String>) {
    macro_rules! poser {
        ($x:ident) => {{
            $x.fichier = Some(nom);
            $x.empreinte = e;
        }};
    }
    match l {
        Ligne::Dsn(x) => poser!(x),
        Ligne::Dpae(x) => poser!(x),
        Ligne::Documents(x) => poser!(x),
        Ligne::ContratSignature(x) => poser!(x),
        Ligne::ContratContresigne(x) => poser!(x),
        Ligne::RegularisationCasc(x) => poser!(x),
        Ligne::DossierSalarie(x) => poser!(x),
        Ligne::FicheDemande(x) => poser!(x),
    }
}

/// Ce qui revient d'un envoi au salarie : un contrat signe (ou la mention retiree), une fiche recue (ou la demande
/// abandonnee).
pub enum Revenu {
    Contrat(RetourContrat),
    Fiche(RetourFiche),
}

/// CE QUI REVIENT D'UN ENVOI (un contrat signe, la mention retiree, une fiche) : `retour` se range dans les `retours` de
/// la derniere ligne que `envoi` reconnait, et la rend. `None` : aucune ligne ne la porte, rien n'est ecrit.
pub fn rattacher(racine: &Path, envoi: impl Fn(&Ligne) -> bool, retour: Revenu) -> Resultat<Option<Ligne>> {
    let mut trouvee = None;
    ajouter(racine, |j, _| {
        let Some(i) = j.lignes.iter().rposition(|l| envoi(l)) else { return Ok(Vec::new()) };
        let sien = matches!((&j.lignes[i], &retour), (Ligne::ContratSignature(_), Revenu::Contrat(_)) | (Ligne::FicheDemande(_), Revenu::Fiche(_)));
        if !sien {
            return Err(format!("journal : ce retour ne revient pas d'un envoi {}", j.lignes[i].acte().nom()));
        }
        trouvee = Some(i);
        let r = match &retour {
            Revenu::Contrat(r) => serde_json::to_value(r),
            Revenu::Fiche(r) => serde_json::to_value(r),
        }
        .map_err(|x| x.to_string())?;
        Ok(vec![Enregistrement { a: Some(j.ids[i].clone()), retour: Some(r), ..Default::default() }])
    })?;
    Ok(match trouvee {
        Some(i) => lu(racine)?.lignes.get(i).cloned(),
        None => None,
    })
}

/// Ce qui se range sur la ligne d'une DSN ou d'une DPAE (`ranger`).
pub enum Rangement {
    /// Un compte rendu de plus.
    Retour(RetourOrganisme),
    /// Le verdict du compte rendu de cette nature et de cette publication.
    Complement { nature: String, publication: Option<String>, verdict: Verdict },
}

/// RANGE CE QUI REVIENT des envois : chaque `(i, rangement)` sur la `i`-ieme ligne du journal lu (`lu`), dans l'ordre.
pub fn ranger(racine: &Path, rangements: Vec<(usize, Rangement)>) -> Resultat<()> {
    ajouter(racine, |j, _| {
        rangements
            .into_iter()
            .map(|(i, r)| {
                let a = Some(j.ids.get(i).ok_or_else(|| format!("journal : aucune ligne {}", i))?.clone());
                Ok(match r {
                    Rangement::Retour(retour) => Enregistrement { a, retour: Some(serde_json::to_value(retour).map_err(|x| x.to_string())?), ..Default::default() },
                    Rangement::Complement { nature, publication, verdict } => Enregistrement { a, nature: Some(nature), publication, complete: Some(verdict), ..Default::default() },
                })
            })
            .collect()
    })
}

/// Ce que les lignes `lignes` du journal disent de ce qui est parti (`departs`) : les lignes qui precedent un envoi
/// disent ce qui etait parti avant lui.
pub fn departs_des_lignes(lignes: &[Ligne]) -> engagement::departs::Departs {
    let mut d = engagement::departs::Departs::default();
    for l in lignes {
        noter_ligne(&mut d, l);
    }
    d
}

/// Ce que la ligne `l` dit de ce qui est parti, a la suite de `d`.
fn noter_ligne(d: &mut engagement::departs::Departs, l: &Ligne) {
    use engagement::departs::{Depart, Envoi};
    let envoi = match l.acte() {
        Acte::ContratSignature => Envoi::Contrat,
        Acte::Dpae => Envoi::Dpae,
        Acte::Documents => Envoi::Documents,
        Acte::Dsn if l.signalement() => Envoi::Signalement,
        Acte::Dsn => Envoi::Dsn,
        _ => return,
    };
    if !l.reel() {
        return;
    }
    // Une declaration refusee au depot ou au bilan n'a rien depose (`etats::refuse_par_un_retour`). Un courriel coupe en
    // route a servi ceux qui l'ont recu : la ligne ne porte que leurs versions (`ligne_d_envoi::noter_echec_d_envoi`),
    // et ce qui a atteint le salarie est parti.
    let au_salarie = matches!(envoi, Envoi::Documents | Envoi::Contrat);
    if !au_salarie && (!l.accepte() || crate::etats::refuse_par_un_retour(l)) {
        return;
    }
    let mois = matches!(envoi, Envoi::Documents | Envoi::Dsn).then(|| l.mois().map(|m| m.to_string())).flatten();
    let le = l.quand().to_string();
    for v in l.versions().unwrap_or_default() {
        // La correction d'un mois passe que la DSN porte : sa version est celle de l'engagement du mois corrige.
        let corrige = (envoi == Envoi::Dsn).then(|| v.mois.map(|m| m.to_string())).flatten().filter(|m| Some(m) != mois.as_ref());
        let (m, par) = match corrige {
            Some(c) => (Some(c), mois.clone()),
            None => (mois.clone(), None),
        };
        // Un avenant : la ligne du journal le dit (`acte`), et son contrat porte son numero.
        let avenant = l
            .signature()
            .filter(|s| s.detail.acte.as_deref() == Some("avenant"))
            .and_then(|s| s.detail.contrats.iter().find(|c| c.salarie == v.salarie && c.numero.as_ref() == Some(&v.numero)))
            .and_then(|c| c.avenant)
            .map(|a| a.to_string());
        d.noter(&v.salarie, &v.numero, Depart { envoi, mois: m, par, avenant, version: v.version, le: le.clone() });
    }
}

/// LES VERSIONS QU'UN ENVOI EMPORTE : pour chaque engagement valide que `mois` montre et que `garder` retient (la cle du
/// salarie, le numero du contrat, l'engagement), sa version. Ce que la ligne du journal de l'envoi porte : elles se lisent
/// AVANT l'envoi, qui ne part pas d'un engagement a revalider (`Valide::a_revalider`).
pub fn versions_du_mois(ctx: &droit::contexte::Contexte, mois: &str, departs: &engagement::departs::Departs, garder: impl Fn(&str, &str, &engagement::stock::Ecrit) -> bool) -> Resultat<Vec<Version>> {
    let mut out = Vec::new();
    for e in engagement::etats::du_mois(ctx, mois, departs)? {
        let Some(v) = e.valide() else { continue };
        let brut = &*v.objet().brut;
        let (Some(cle), Some(numero)) = (&brut.saisie.carte.salarie, brut.saisie.carte.numero.as_deref()) else { continue };
        if garder(cle, numero, brut) {
            if let Some(phrase) = v.a_revalider(ctx) {
                return Err(phrase);
            }
            let version = v.version().ok_or_else(|| socle::echec::cle(socle::echec::PAIE_ILLISIBLE, "version"))?;
            out.push(Version { salarie: cle.clone(), numero: engagement::numeros::NumeroContrat::lire(numero)?, version, mois: None });
        }
    }
    Ok(out)
}

/// `versions_du_mois` des contrats qui partent, chacun lu dans le mois qu'il nomme.
pub fn versions_des_contrats(ctx: &droit::contexte::Contexte, contrats: &[ContratParti], departs: &engagement::departs::Departs) -> Resultat<Vec<Version>> {
    let mut mois: Vec<Mois> = contrats.iter().map(|c| c.mois).collect();
    mois.sort();
    mois.dedup();
    let mut out: Vec<Version> = Vec::new();
    for m in mois {
        let siens = |cle: &str, n: &str, _: &engagement::stock::Ecrit| contrats.iter().any(|c| c.mois == m && c.salarie.as_str() == cle && c.numero_dit() == n);
        for x in versions_du_mois(ctx, &m.to_string(), departs, siens)? {
            if !out.contains(&x) {
                out.push(x);
            }
        }
    }
    Ok(out)
}

/// Le plus grand numero d'ordre (`S20.G00.05.004`) des signalements REELS deja envoyes par la compagnie, tous
/// evenements confondus (arret, reprise, fin de contrat, amorcage) ; 0 s'il n'y en a aucun.
///
/// Cahier technique DSN 2026.1, p. 136 : « Pour un signalement, ce numero ne doit pas etre remis a zero afin
/// qu'il soit unique quel que soit le mois d'emission » ; p. 14 : un second signalement normal au meme SIRET et
/// au meme numero d'ordre n'est pas accepte. Le numero se LIT de ce qui est parti (`detail.ordre` des lignes
/// `dsn` reelles du journal), il ne se devine pas : une ligne reelle acceptee qui ne dit pas son ordre refuse.
/// Un signalement se reconnait a `detail.nature` (04, 05, 07, 08) ; une ligne reprise des retours, qui ne la porte pas,
/// a son mois : le bilan d'une mensuelle le dit, celui d'un signalement non. Une ligne non acceptee (le point de depot
/// n'a rien recu) ne consomme pas de numero.
pub fn dernier_ordre_des_signalements(racine: &Path) -> Resultat<i64> {
    let mut dernier = 0;
    for l in lire(racine, Some(Acte::Dsn), None, true)? {
        let Some(e) = l.dsn() else { continue };
        let signalement = match e.detail.nature.as_deref().filter(|n| !n.is_empty()) {
            Some(n) => declarations::norme::SIGNALEMENTS.contains(&n),
            None => e.mois.is_none(),
        };
        if !signalement {
            continue;
        }
        match e.detail.ordre.as_deref().and_then(|o| o.trim().parse::<i64>().ok()) {
            Some(o) => dernier = dernier.max(o),
            None if e.accepte => {
                return Err(socle::echec::avec(
                    format!(
                        "Le journal des envois note le signalement reel du {} sans son numero d'ordre, lu en S20.G00.05.004 du fichier envoye. Le numero d'ordre d'un signalement ne se remet jamais a zero (cahier technique DSN, p. 136) : sans lui, le suivant ne se numerote pas. Le signaler au developpeur. Rien n'est ecrit.",
                        e.quand,
                    ),
                    serde_json::json!({"chemin": chemin(racine).to_string_lossy(), "fichier": e.fichier.as_deref().unwrap_or("?"), "cle": "detail.ordre"}),
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
        let retour = |nature: &str, verdict: &str| RetourOrganisme { nature: nature.into(), verdict: Some(verdict.into()), anomalies: None, dpae: None, libelle: String::new(), statut: None, publication: None };
        fn envoi<D>(salarie: &str, retours: Vec<RetourOrganisme>, detail: D) -> Envoi<D, RetourOrganisme> {
            let mut e = Envoi::nouveau(Mode::Reel, "t", true, "", "", detail);
            e.mois = Some(Mois::lire("2026-06").unwrap());
            e.quand = "2026-07-05T10:00:00".into();
            e.retours = retours;
            e.versions = Some(vec![Version { salarie: droit::contexte::IdSalarie::lire(salarie).unwrap(), numero: engagement::numeros::NumeroContrat::lire("DE1.26.01").unwrap(), version: 1, mois: None }]);
            e
        }
        let ko = vec![retour("10", "OK"), retour("11", "KO")];
        let lignes = vec![
            Ligne::Dsn(envoi("ALPHA", ko.clone(), DetailDsn::default())),
            Ligne::Dsn(envoi("BETA", vec![], DetailDsn { reconstruit: true, etat_declaration: Some("KO".into()), ..Default::default() })),
            Ligne::Dpae(envoi("GAMMA", ko, DetailDpae::default())),
            Ligne::Dsn(envoi("DELTA", vec![retour("11", "OK")], DetailDsn::default())),
        ];
        let d = departs_des_lignes(&lignes);
        for refusee in ["ALPHA", "BETA", "GAMMA"] {
            assert!(d.de(refusee, "DE1.26.01").is_empty(), "{refusee}");
        }
        assert_eq!(d.de("DELTA", "DE1.26.01").len(), 1);
    }
}
