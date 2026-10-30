//! Lire les fichiers JSON livres avec le code (`json_en_cache`) et dire
//! ou est le code.
//!
//! Le cache est cle par chemin ET par signature (date de modification,
//! taille) : un fichier reecrit se relit.

use crate::num::Resultat;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

// ------------------------------------------------ le registre des lectures
//
// Tout ce que le calcul lit sur le disque passe par `lire_texte`, `existe`,
// `est_dossier` et `lister`. Quand un registre est ouvert (`suivre`), chacun
// y note ce qu'il a vu : l'empreinte du contenu d'un fichier, `absent`, la
// liste des `.json` d'un dossier. C'est la liste des entrees du calcul
// (`engagement::empreinte`). Les caches qui ne notent rien quand ils servent (les baremes composes) se cle par
// `generation` : un calcul suivi a la sienne, a lui seul, et repart a froid ; hors registre, la generation est 0 et
// ces caches servent d'une route a l'autre (la fenetre garde le moteur charge).

struct Registre {
    racine: PathBuf,
    code: PathBuf,
    entrees: BTreeMap<String, String>,
    /// Le numero de ce registre, unique dans le processus.
    numero: u64,
}

static GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// La cle des caches qui ne notent pas : le numero du registre ouvert SUR CE FIL, 0 sans registre. Un calcul suivi ne
/// sert ainsi que ce qu'il a compose lui-meme (tout ce qu'il lit se note), meme si un autre fil compose en meme temps.
pub fn generation() -> u64 {
    REGISTRE.with(|r| r.borrow().last().map_or(0, |g| g.numero))
}

thread_local! {
    static REGISTRE: RefCell<Vec<Registre>> = const { RefCell::new(Vec::new()) };
}

/// Ouvre un registre : les chemins sous `racine` ou `code` s'y notent relatifs. Un calcul suivi peut en faire un autre
/// (la paie d'un mois lit celle du mois d'avant, qui se calcule alors) : le registre neuf se pose au-dessus de celui qui
/// est ouvert, et `rendre` redonne celui-ci a son calcul.
pub fn suivre(racine: &Path, code: &Path) {
    // Un registre neuf, c'est un calcul « a froid » : les caches de bareme (qui ne notent rien quand ils servent)
    // ne lui servent que ce qu'il compose, pour que la liste des entrees soit celle d'un processus neuf.
    let numero = GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    REGISTRE.with(|r| r.borrow_mut().push(Registre { racine: racine.to_path_buf(), code: code.to_path_buf(), entrees: BTreeMap::new(), numero }));
}

/// Un registre est-il ouvert sur ce fil ?
pub fn registre_ouvert() -> bool {
    suivi()
}

/// Au-dela, un cache du processus se vide (`ranger`).
pub const CACHE_MAX: usize = 512;

/// Range `v` sous `cle` dans un cache du processus. La fenetre garde le moteur charge : chaque calcul
/// suivi ajoute ses cles (`generation`), et un cache plein se vide plutot que de grossir tant que la fenetre tourne.
pub fn ranger<K: Eq + std::hash::Hash, V>(cache: &Mutex<HashMap<K, V>>, cle: K, v: V) {
    let mut c = cache.lock().unwrap_or_else(|p| p.into_inner());
    if c.len() >= CACHE_MAX {
        c.clear();
    }
    c.insert(cle, v);
}

/// Ferme le dernier registre ouvert et rend ce qu'il a note, trie par cle. Ce qu'un calcul fait au milieu d'un autre a
/// lu, celui qui l'a demande en depend aussi : ses entrees s'ajoutent au registre d'en dessous.
pub fn rendre() -> BTreeMap<String, String> {
    REGISTRE.with(|r| {
        let mut pile = r.borrow_mut();
        let entrees = pile.pop().map(|g| g.entrees).unwrap_or_default();
        if let Some(dessous) = pile.last_mut() {
            for (k, v) in &entrees {
                dessous.entrees.entry(k.clone()).or_insert_with(|| v.clone());
            }
        }
        entrees
    })
}

/// Ferme tous les registres de ce fil : un calcul tombe a pu en laisser d'ouverts.
pub fn fermer_tout() {
    REGISTRE.with(|r| r.borrow_mut().clear());
}

fn suivi() -> bool {
    REGISTRE.with(|r| !r.borrow().is_empty())
}

/// `genre racine:chemin` / `genre code:chemin` / `genre abs:chemin`.
fn cle_de(genre: &str, chemin: &Path) -> String {
    REGISTRE.with(|r| {
        let r = r.borrow();
        let r = r.last().expect("registre ouvert");
        for (nom, base) in [("racine", &r.racine), ("code", &r.code)] {
            if let Ok(rel) = chemin.strip_prefix(base) {
                return format!("{} {}:{}", genre, nom, chemin_en_barres(rel));
            }
        }
        format!("{} abs:{}", genre, chemin.to_string_lossy())
    })
}

/// UN CHEMIN RELATIF NOTE S'ECRIT AVEC DES BARRES DROITES, sur tous les systemes : la saisie du mois
/// se reconnait dans les entrees sous `racine:engagements/AAAA-MM.json` (`engagement::empreinte`) : une cle notee avec la barre de
/// Windows ne s'y retrouverait pas. `chemin_de` la relit telle quelle : Windows accepte la barre droite.
fn chemin_en_barres(rel: &Path) -> String {
    rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")
}

/// Le genre et le chemin absolu d'une entree notee, pour la relire.
pub fn chemin_de(cle: &str, racine: &Path, code: &Path) -> Option<(String, PathBuf)> {
    let (genre, reste) = cle.split_once(' ')?;
    let (base, rel) = reste.split_once(':')?;
    let chemin = match base {
        "racine" => racine.join(rel),
        "code" => code.join(rel),
        "abs" => PathBuf::from(rel),
        _ => return None,
    };
    Some((genre.to_string(), chemin))
}

fn noter(genre: &str, chemin: &Path, valeur: String) {
    if suivi() {
        let k = cle_de(genre, chemin);
        REGISTRE.with(|r| {
            if let Some(g) = r.borrow_mut().last_mut() {
                g.entrees.insert(k, valeur);
            }
        });
    }
}

/// Note une entree qui n'est pas un fichier (`fiches`, `moteur`, `aujourdhui`).
pub fn noter_valeur(cle: &str, valeur: String) {
    REGISTRE.with(|r| {
        if let Some(g) = r.borrow_mut().last_mut() {
            g.entrees.insert(cle.to_string(), valeur);
        }
    });
}

pub fn sha256_hex(octets: &[u8]) -> String {
    Sha256::digest(octets).iter().map(|o| format!("{:02x}", o)).collect()
}

/// Ce que vaut un fichier pour l'empreinte : le hachage de son contenu, ou `absent`.
pub fn empreinte_fichier(chemin: &Path) -> String {
    match std::fs::read(chemin) {
        Ok(o) => empreinte_des_octets(chemin, o),
        Err(_) => "absent".into(),
    }
}

/// LE HACHAGE DES OCTETS `octets`, lus de `chemin`, fait une fois par contenu : les referentiels du code pesent plus
/// d'un megaoctet, et chaque calcul suivi, chaque empreinte relue d'une paie les nomme tous. Le hachage garde ne sert
/// que si les octets sont exactement ceux qu'il a haches : il ne se perime jamais.
fn empreinte_des_octets(chemin: &Path, octets: Vec<u8>) -> String {
    type Empreintes = Mutex<HashMap<PathBuf, (Vec<u8>, String)>>;
    static EMPREINTES: OnceLock<Empreintes> = OnceLock::new();
    let cache = EMPREINTES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some((vus, e)) = cache.lock().unwrap_or_else(|p| p.into_inner()).get(chemin) {
        if *vus == octets {
            return e.clone();
        }
    }
    let e = sha256_hex(&octets);
    ranger(cache, chemin.to_path_buf(), (octets, e.clone()));
    e
}

fn noms(chemin: &Path) -> Option<Vec<String>> {
    let mut n: Vec<String> = std::fs::read_dir(chemin).ok()?.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".json")).collect();
    n.sort();
    Some(n)
}

/// L'empreinte d'un dossier : les noms de ses `.json`, tries ; `absent` s'il ne s'ouvre pas.
pub fn empreinte_dossier(chemin: &Path) -> String {
    match noms(chemin) {
        Some(n) => sha256_hex(n.join("\n").as_bytes()),
        None => "absent".into(),
    }
}

/// Un fichier servi par un cache : le hacher une fois si le registre ne l'a pas encore.
pub fn noter_si_besoin(chemin: &Path) {
    if suivi() {
        let k = cle_de("fichier", chemin);
        let connu = REGISTRE.with(|r| r.borrow().last().is_some_and(|g| g.entrees.contains_key(&k)));
        if !connu {
            noter("fichier", chemin, empreinte_fichier(chemin));
        }
    }
}

/// `std::fs::read_to_string`, note au registre.
pub fn lire_texte(chemin: &Path) -> std::io::Result<String> {
    match std::fs::read(chemin) {
        Ok(o) => {
            if suivi() {
                noter("fichier", chemin, empreinte_des_octets(chemin, o.clone()));
            }
            String::from_utf8(o).map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "stream did not contain valid UTF-8"))
        }
        Err(e) => {
            noter("fichier", chemin, "absent".into());
            Err(e)
        }
    }
}

/// `Path::exists`, note.
pub fn existe(chemin: &Path) -> bool {
    let v = chemin.exists();
    noter("existe", chemin, if v { "oui" } else { "non" }.into());
    v
}

/// `Path::is_dir`, note.
pub fn est_dossier(chemin: &Path) -> bool {
    let v = chemin.is_dir();
    noter("dossier?", chemin, if v { "oui" } else { "non" }.into());
    v
}

/// Les `.json` d'un dossier, tries ; `None` s'il ne s'ouvre pas. Note : ce
/// qui apparait ou disparait dans le dossier change l'empreinte.
pub fn lister(dossier: &Path) -> Option<Vec<PathBuf>> {
    let n = noms(dossier);
    noter("dossier", dossier, match &n {
        Some(n) => sha256_hex(n.join("\n").as_bytes()),
        None => "absent".into(),
    });
    Some(n?.into_iter().map(|x| dossier.join(x)).collect())
}

/// Le dossier des referentiels livres (`donnees/`) : `PUPPETPAIE_CODE` si posee, sinon
/// `<crate>/../donnees`.
pub fn dossier_du_code() -> PathBuf {
    match std::env::var_os("PUPPETPAIE_CODE") {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../logiciel/donnees"),
    }
}

/// La signature d'un fichier : (date de modification en ns, taille), ou `None`.
///
/// LE CODE LIVRE NE BOUGE PAS PENDANT UNE ROUTE : le temps d'une route (`code_fige`), la signature d'un
/// fichier du code (`dossier_du_code`) se lit une fois. Les caches du moteur la demandaient a chaque service -- mille
/// fois `norme-dsn-base.json` pour un seul `etat` --, et sous Windows chaque demande ouvre le fichier, que l'antivirus
/// inspecte. Les fichiers de la compagnie, eux, se relisent toujours : une route les ecrit.
pub fn signature(chemin: &Path) -> Option<(u128, u64)> {
    let memo = CODE_FIGE.with(|c| c.borrow().as_ref().filter(|(code, _)| chemin.starts_with(code)).map(|(_, m)| m.get(chemin).copied()));
    match memo {
        Some(Some(s)) => s,
        Some(None) => {
            let s = signature_lue(chemin);
            CODE_FIGE.with(|c| {
                if let Some((_, m)) = c.borrow_mut().as_mut() {
                    m.insert(chemin.to_path_buf(), s);
                }
            });
            s
        }
        None => signature_lue(chemin),
    }
}

fn signature_lue(chemin: &Path) -> Option<(u128, u64)> {
    let m = std::fs::metadata(chemin).ok()?;
    let ns = m
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    Some((ns, m.len()))
}

type SignaturesDuCode = Option<(PathBuf, HashMap<PathBuf, Option<(u128, u64)>>)>;

thread_local! {
    static CODE_FIGE: RefCell<SignaturesDuCode> = const { RefCell::new(None) };
}

/// Le temps d'une route, le code livre est fige (`signature`). Rendu par `code_fige`, il le libere en tombant ; une
/// route lue depuis une autre ne fige rien de plus.
pub struct CodeFige(bool);

#[must_use = "le code n'est fige que tant que son garde vit"]
pub fn code_fige() -> CodeFige {
    CodeFige(CODE_FIGE.with(|c| {
        let mut c = c.borrow_mut();
        if c.is_some() {
            return false;
        }
        *c = Some((dossier_du_code(), HashMap::new()));
        true
    }))
}

impl Drop for CodeFige {
    fn drop(&mut self) {
        if self.0 {
            CODE_FIGE.with(|c| *c.borrow_mut() = None);
        }
    }
}

type Cache = Mutex<HashMap<PathBuf, ((u128, u64), Arc<Value>)>>;

/// Un fichier JSON lu une fois tant qu'il ne change pas (objet partage : on
/// ne le modifie pas).
pub fn json_en_cache(chemin: &Path) -> Resultat<Arc<Value>> {
    json_en_cache_ou(chemin, lire_json)
}

/// `json_en_cache`, lu par `lire` quand il faut le lire : le refus d'un fichier illisible reste celui de l'appelant (sa
/// classe, son message), et ne se garde jamais -- seul un JSON lu se garde, sous la signature du fichier (date, taille).
/// `lire` doit noter sa lecture (`lire_texte`) ; servi du cache, le fichier se note ici (`noter_si_besoin`).
pub fn json_en_cache_ou<E>(chemin: &Path, lire: impl FnOnce(&Path) -> Result<Value, E>) -> Result<Arc<Value>, E> {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let sig = signature(chemin);
    if let Some(s) = sig {
        if let Some((vue, v)) = cache.lock().unwrap().get(chemin) {
            if *vue == s {
                noter_si_besoin(chemin);
                return Ok(v.clone());
            }
        }
    }
    let v = Arc::new(lire(chemin)?);
    if let Some(s) = sig {
        cache.lock().unwrap().insert(chemin.to_path_buf(), (s, v.clone()));
    }
    Ok(v)
}

type CacheDesContenus = Mutex<HashMap<PathBuf, (Arc<Vec<u8>>, Arc<Value>)>>;

/// UN JSON LU, ANALYSE UNE FOIS PAR CONTENU : le fichier se relit a chaque fois (ses octets), et ce qu'on en avait tire
/// ne sert que si ses octets sont exactement ceux d'alors. Rien ne se lit perime, quel que soit qui l'a ecrit et quand
/// (une autre fenetre, une restauration, une cle USB aux dates a deux secondes pres) : seule l'analyse s'epargne, et
/// c'est elle qui coute. `None` s'il n'existe pas. Rien ne se note au registre : l'appelant dit ce qu'il lit (les
/// engagements se comptent par leur saisie, `engagement::stock`). Partage : on ne le modifie pas ; deux lectures du meme
/// contenu rendent la meme valeur (`Arc::ptr_eq`).
pub fn json_partage(chemin: &Path) -> Resultat<Option<Arc<Value>>> {
    static CACHE: OnceLock<CacheDesContenus> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let octets = match std::fs::read(chemin) {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(panne_fichier(&e, chemin)),
    };
    if let Some((vus, v)) = cache.lock().unwrap_or_else(|p| p.into_inner()).get(chemin) {
        if **vus == octets {
            return Ok(Some(v.clone()));
        }
    }
    let v = Arc::new(serde_json::from_slice::<Value>(&octets).map_err(|e| format!("{} : {}", chemin.display(), e))?);
    ranger(cache, chemin.to_path_buf(), (Arc::new(octets), v.clone()));
    Ok(Some(v))
}

/// `json.loads(p.read_text(encoding="utf-8"))`, sans cache.
pub fn lire_json(chemin: &Path) -> Resultat<Value> {
    let texte = lire_texte(chemin)
        .map_err(|e| format!("{} : {}", chemin.display(), e))?;
    serde_json::from_str(&texte).map_err(|e| format!("{} : {}", chemin.display(), e))
}

/// Une valeur est-elle vraie ? Faux : `null`, `false`, zero, un texte, une liste ou un objet vides.
pub fn vrai(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|x| x != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// Un texte cite dans un message : entre guillemets simples (doubles s'il porte une apostrophe et aucun guillemet),
/// echappe.
pub fn repr(s: &str) -> String {
    let guillemet = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut out = String::from(guillemet);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == guillemet => {
                out.push('\\');
                out.push(c);
            }
            c if !imprimable(c) => {
                let n = c as u32;
                if n < 0x100 {
                    out.push_str(&format!("\\x{:02x}", n));
                } else if n < 0x10000 {
                    out.push_str(&format!("\\u{:04x}", n));
                } else {
                    out.push_str(&format!("\\U{:08x}", n));
                }
            }
            c => out.push(c),
        }
    }
    out.push(guillemet);
    out
}

/// Ni controle, ni separateur (hors espace), ni format,
/// ni usage prive. Les points de code non attribues ne sont pas connus ici.
pub fn imprimable(c: char) -> bool {
    let n = c as u32;
    if c == ' ' {
        return true;
    }
    !(c.is_control()
        || c.is_whitespace()
        || matches!(n, 0xad | 0x600..=0x605 | 0x61c | 0x6dd | 0x70f | 0x8e2 | 0x180e | 0x200b..=0x200f | 0x202a..=0x202e
            | 0x2060..=0x2064 | 0x2066..=0x206f | 0xfeff | 0xfff9..=0xfffb | 0x110bd | 0x1d173..=0x1d17a | 0xe0001 | 0xe0020..=0xe007f
            | 0xe000..=0xf8ff | 0xf0000..=0x10ffff | 0xfdd0..=0xfdef)
        || n & 0xfffe == 0xfffe)
}

/// Une valeur JSON citee dans un message (`None`, `True`, `12`, `'x'`).
pub fn repr_valeur(v: &Value) -> String {
    match v {
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::String(s) => repr(s),
        Value::Array(a) => format!("[{}]", a.iter().map(repr_valeur).collect::<Vec<_>>().join(", ")),
        Value::Object(o) => format!("{{{}}}", o.iter().map(|(k, v)| format!("{}: {}", repr(k), repr_valeur(v))).collect::<Vec<_>>().join(", ")),
        autre => autre.to_string(),
    }
}

// ------------------------------------------------ les refus d'une donnee lue
//
// Un fichier ou une demande dont un champ manque ou n'a pas la forme attendue :
// la phrase nomme le champ, ce qui etait attendu et ce qui a ete lu.

/// « champ 'prenom' absent ».
pub fn absent(cle: &str) -> String {
    format!("champ {} absent", repr(cle))
}

/// Une valeur JSON dite en francais, depuis son nom de type (`valeur::nom_type`).
pub fn nature(nom_type: &str) -> &'static str {
    match nom_type {
        "dict" => "un objet",
        "list" | "tuple" => "une liste",
        "str" => "un texte",
        "int" => "un nombre entier",
        "float" => "un nombre",
        "bool" => "un booléen",
        "NoneType" => "null",
        _ => "une valeur d'une autre forme",
    }
}

/// « un objet est attendu, lu : une liste ».
pub fn attendu(quoi: &str, nom_type: &str) -> String {
    let accord = if quoi.starts_with("une ") { "e" } else { "" };
    format!("{} est attendu{}, lu : {}", quoi, accord, nature(nom_type))
}

/// « liste trop courte : <ce qui y etait cherche> ».
pub fn trop_courte(quoi: &str) -> String {
    format!("liste trop courte : {}", quoi)
}

/// « une liste ne peut pas servir de clé » : une valeur cherchée dans une table, ou rangée parmi des clés.
pub fn pas_une_cle(nom_type: &str) -> String {
    format!("{} ne peut pas servir de clé", nature(nom_type))
}

/// « comparaison impossible : un texte et un nombre entier » : deux valeurs à ranger qui ne se comparent pas.
pub fn incomparables(type_a: &str, type_b: &str) -> String {
    format!("comparaison impossible : {} et {}", nature(type_a), nature(type_b))
}

/// « élément 3 de la liste : <ce qui ne va pas> » (`rang` compte depuis zéro, l'élément depuis un).
pub fn element(rang: usize, quoi: &str) -> String {
    format!("élément {} de la liste : {}", rang + 1, quoi)
}

/// « élément 3 de la liste : une paire (clé, valeur) est attendue, lu : 3 valeurs » : une liste qui se lit en objet.
pub fn paire_attendue(rang: usize, longueur: usize) -> String {
    element(rang, &format!("une paire (clé, valeur) est attendue, lu : {} valeur{}", longueur, if longueur > 1 { "s" } else { "" }))
}

/// « élément 3 de la liste : une paire (clé, valeur) est attendue, lu : un nombre ».
pub fn pas_une_paire(rang: usize, nom_type: &str) -> String {
    element(rang, &format!("une paire (clé, valeur) est attendue, lu : {}", nature(nom_type)))
}

/// « un texte ne se modifie pas par clé » : une valeur qui n'est ni un objet ni une liste, là où l'on écrit dedans.
pub fn non_modifiable(nom_type: &str) -> String {
    format!("{} ne se modifie pas par clé", nature(nom_type))
}

/// Un nombre que l'on voudrait entier et qui ne tient pas : infini, ou hors de ce qu'un entier porte.
pub fn entier_hors_limites(n: &str) -> String {
    format!("nombre entier hors limites : {}", n)
}

/// Le refus du disque : « fichier introuvable : '<chemin>' », « accès refusé : '<chemin>' »,
/// sinon « fichier illisible : '<chemin>' : <cause du systeme> ».
pub fn panne_fichier(e: &std::io::Error, chemin: &Path) -> String {
    let c = repr(&chemin.to_string_lossy());
    match e.kind() {
        std::io::ErrorKind::NotFound => format!("fichier introuvable : {}", c),
        std::io::ErrorKind::PermissionDenied => format!("accès refusé : {}", c),
        _ if chemin.is_dir() => format!("un dossier, pas un fichier : {}", c),
        _ => format!("fichier illisible : {} : {}", c, e),
    }
}

/// « fichier introuvable : '<chemin>' ».
pub fn introuvable(chemin: &Path) -> String {
    format!("fichier introuvable : {}", repr(&chemin.to_string_lossy()))
}

/// « écriture impossible : '<chemin>' : <cause du systeme> ».
pub fn ecriture_impossible(e: &std::io::Error, chemin: &Path) -> String {
    format!("écriture impossible : {} : {}", repr(&chemin.to_string_lossy()), e)
}

/// Le marqueur d'une compagnie qui n'est pas la vraie (une compagnie fictive, une copie de developpement).
pub const MARQUEUR_COPIE: &str = "COPIE-DE-DEVELOPPEMENT.json";

/// Le modele `nom` (gabarit de contrat, de DPAE, de certificat) : celui de la compagnie (`_gabarits` de sa fiche `ent`),
/// a defaut celui du poste (`poste`), a defaut celui livre avec le logiciel (`donnees/modeles/`). Une compagnie qui
/// ecrit ses propres clauses passe devant ; une copie de la configuration du poste sans son dossier de gabarits reste la
/// meme compagnie et garde ceux du poste (son gabarit DPAE porte son centre de medecine du travail). Une compagnie qui
/// porte `MARQUEUR_COPIE` n'emprunte rien au poste : ses documents ne prennent jamais les clauses de la compagnie reelle.
pub fn modele_de(nom: &str, ent: &Value, poste: &Path) -> PathBuf {
    let propre = match ent.get("_gabarits") {
        Some(g) if vrai(g) => PathBuf::from(crate::date::en_texte(g)),
        _ => poste.to_path_buf(),
    };
    let copie = propre.parent().is_some_and(|p| p.join(MARQUEUR_COPIE).is_file());
    let dossiers: Vec<&Path> = if copie { vec![&propre] } else { vec![&propre, poste] };
    for dossier in dossiers {
        if dossier.join(nom).exists() {
            return dossier.join(nom);
        }
    }
    dossier_du_code().join("modeles").join(nom)
}

/// Un fichier texte UTF-8 ; le refus porte la classe de l'erreur de lecture (`FileNotFoundError`, `PermissionError`,
/// `IsADirectoryError`, `OSError`, `UnicodeDecodeError`).
pub fn lire_texte_echec(p: &Path) -> Result<String, crate::echec::Echec> {
    use crate::echec::erreur;
    let octets = std::fs::read(p).map_err(|e| {
        let classe = if p.is_dir() {
            "IsADirectoryError"
        } else {
            match e.kind() {
                std::io::ErrorKind::NotFound => "FileNotFoundError",
                std::io::ErrorKind::PermissionDenied => "PermissionError",
                _ => "OSError",
            }
        };
        erreur(classe, panne_fichier(&e, p))
    })?;
    String::from_utf8(octets).map_err(|e| erreur("UnicodeDecodeError", format!("{} : {}", p.display(), e)))
}

/// Un fichier JSON (`lire_texte_echec`, puis `JSONDecodeError` s'il ne se lit pas).
pub fn lire_json_echec(p: &Path) -> Result<Value, crate::echec::Echec> {
    serde_json::from_str(&lire_texte_echec(p)?).map_err(|e| crate::echec::erreur("JSONDecodeError", e.to_string()))
}

/// Le nom d'un chemin, ou son texte entier s'il n'en a pas.
pub fn nom_du(chemin: &std::path::Path) -> String {
    chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| chemin.to_string_lossy().into_owned())
}

#[cfg(test)]
mod essais {
    use super::*;

    /// Un calcul suivi qui en fait un autre (la paie de juin d'un contrat sans terme lit celle de mai, qui se calcule) :
    /// le calcul de dessous garde son registre, et ce que celui du dessus a lu s'y ajoute ; sinon la paie s'ecrirait avec
    /// la seule marque du moteur, et ne se recalculerait plus quand ce qu'elle lit change.
    #[test]
    fn un_calcul_dans_un_autre_rend_son_registre_a_celui_qui_l_a_demande() {
        let (racine, code) = (Path::new("/compagnie"), Path::new("/code"));
        suivre(racine, code);
        noter_valeur("saisie 2026-06", "a".into());
        suivre(racine, code);
        noter_valeur("saisie 2026-05", "b".into());
        let dessus = rendre();
        noter_valeur("fiches", "c".into());
        let dessous = rendre();
        assert_eq!(dessus.keys().collect::<Vec<_>>(), ["saisie 2026-05"]);
        assert_eq!(dessous.keys().collect::<Vec<_>>(), ["fiches", "saisie 2026-05", "saisie 2026-06"]);
        assert!(!registre_ouvert());
    }
}
