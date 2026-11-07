//! Lire les fichiers livres avec le code (`forme_en_cache`) et dire ou est le code.
//!
//! Le cache est cle par chemin ET par signature (date de modification,
//! taille) : un fichier reecrit se relit.

use crate::num::Resultat;
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
// (`engagement::empreinte`). Un cache qui ne note rien quand il sert (le compteur des conges) se cle par
// `generation` : un calcul suivi a la sienne, a lui seul, et repart a froid ; hors registre, la generation est 0 et
// ce cache sert d'une route a l'autre (la fenetre garde le moteur charge). Les baremes composes gardent ce qu'ils ont lu
// et le notent quand ils servent (`garde_notee`).

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
    REGISTRE.with(|r| r.borrow().last().into_iter().map(|g| g.numero).sum())
}

thread_local! {
    static REGISTRE: RefCell<Vec<Registre>> = const { RefCell::new(Vec::new()) };
}

/// Ouvre un registre : les chemins sous `racine` ou `code` s'y notent relatifs. Un calcul suivi peut en faire un autre
/// (la paie d'un mois lit celle du mois d'avant, qui se calcule alors) : le registre neuf se pose au-dessus de celui qui
/// est ouvert, et `rendre` redonne celui-ci a son calcul.
pub fn suivre(racine: &Path, code: &Path) {
    // Un registre neuf, c'est un calcul « a froid » : un cache qui ne note rien quand il sert (`generation`)
    // ne lui sert que ce qu'il compose, pour que la liste des entrees soit celle d'un processus neuf.
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
    let mut c = crate::memoire::tenu(cache);
    if c.len() >= CACHE_MAX {
        c.clear();
    }
    c.insert(cle, v);
}

/// Ce qu'un cache qui note garde (`garde_notee`) : la valeur, et ce que sa composition a lu, sous la base (racine, code)
/// du registre ou elle s'est faite ; rien quand elle s'est faite hors registre.
pub struct Notee<V> {
    valeur: V,
    lu: Option<(PathBuf, PathBuf, BTreeMap<String, String>)>,
}

/// UN CACHE QUI NOTE : la valeur que `composer` fait (un bareme fondu) se garde sous `cle` avec ce que sa composition a
/// lu. Servie dans un registre ouvert, elle y note ces entrees, comme si elle venait de se composer : la liste des
/// entrees d'un calcul suivi reste celle d'un processus neuf, sans recomposer. Une valeur faite hors registre, ou sous
/// une autre base, se recompose la premiere fois qu'un registre la demande. `cle` dit ce dont la valeur depend.
pub fn garde_notee<V: Clone, E>(cache: &Mutex<HashMap<String, Notee<V>>>, cle: String, composer: impl FnOnce() -> Result<V, E>) -> Result<V, E> {
    let base = REGISTRE.with(|r| r.borrow().last().map(|g| (g.racine.clone(), g.code.clone())));
    let servie = {
        let c = crate::memoire::tenu(cache);
        c.get(&cle).and_then(|n| match (&base, &n.lu) {
            (None, _) => Some((n.valeur.clone(), None)),
            (Some((r, k)), Some((r2, k2, entrees))) if r == r2 && k == k2 => Some((n.valeur.clone(), Some(entrees.clone()))),
            _ => None,
        })
    };
    if let Some((v, entrees)) = servie {
        if let Some(entrees) = entrees {
            REGISTRE.with(|r| {
                if let Some(g) = r.borrow_mut().last_mut() {
                    for (k, x) in entrees {
                        g.entrees.entry(k).or_insert(x);
                    }
                }
            });
        }
        return Ok(v);
    }
    let (v, lu) = match base {
        None => (composer()?, None),
        Some((r, k)) => {
            suivre(&r, &k);
            let v = composer();
            let entrees = rendre();
            (v?, Some((r, k, entrees)))
        }
    };
    ranger(cache, cle, Notee { valeur: v.clone(), lu });
    Ok(v)
}

/// Ferme le dernier registre ouvert et rend ce qu'il a note, trie par cle. Ce qu'un calcul fait au milieu d'un autre a
/// lu, celui qui l'a demande en depend aussi : ses entrees s'ajoutent au registre d'en dessous.
pub fn rendre() -> BTreeMap<String, String> {
    REGISTRE.with(|r| {
        let mut pile = r.borrow_mut();
        let entrees: BTreeMap<String, String> = pile.pop().into_iter().flat_map(|g| g.entrees).collect();
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

/// Ce que vaut un fichier pour l'empreinte : le hachage de son contenu, `absent` s'il n'existe pas, `illisible` (et la
/// raison) s'il ne se lit pas.
pub fn empreinte_fichier(chemin: &Path) -> String {
    match std::fs::read(chemin) {
        Ok(o) => empreinte_des_octets(chemin, o),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => "absent".into(),
        Err(e) => format!("illisible : {}", e),
    }
}

/// LE HACHAGE DES OCTETS `octets`, lus de `chemin`, fait une fois par contenu : les referentiels du code pesent plus
/// d'un megaoctet, et chaque calcul suivi, chaque empreinte relue d'une paie les nomme tous. Le hachage garde ne sert
/// que si les octets sont exactement ceux qu'il a haches : il ne se perime jamais.
fn empreinte_des_octets(chemin: &Path, octets: Vec<u8>) -> String {
    type Empreintes = Mutex<HashMap<PathBuf, (Vec<u8>, String)>>;
    static EMPREINTES: OnceLock<Empreintes> = OnceLock::new();
    let cache = EMPREINTES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some((vus, e)) = crate::memoire::tenu(cache).get(chemin) {
        if *vus == octets {
            return e.clone();
        }
    }
    let e = sha256_hex(&octets);
    ranger(cache, chemin.to_path_buf(), (octets, e.clone()));
    e
}

fn noms(chemin: &Path) -> std::io::Result<Vec<String>> {
    let mut n = Vec::new();
    for e in std::fs::read_dir(chemin)? {
        let nom = e?.file_name().to_string_lossy().into_owned();
        if nom.ends_with(".json") {
            n.push(nom);
        }
    }
    n.sort();
    Ok(n)
}

/// L'empreinte d'un dossier : les noms de ses `.json`, tries ; `absent` s'il n'existe pas, `illisible` (et la raison)
/// s'il ne se lit pas : ni l'un ni l'autre n'est l'empreinte d'un dossier lu.
pub fn empreinte_dossier(chemin: &Path) -> String {
    empreinte_des_noms(&noms(chemin))
}

fn empreinte_des_noms(noms: &std::io::Result<Vec<String>>) -> String {
    match noms {
        Ok(n) => sha256_hex(n.join("\n").as_bytes()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => "absent".into(),
        Err(e) => format!("illisible : {}", e),
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

/// Les `.json` d'un dossier, tries ; `None` s'il n'existe pas ; le disque qui refuse se dit (`echec::du_disque`). Note :
/// ce qui apparait ou disparait dans le dossier change l'empreinte.
pub fn lister(dossier: &Path) -> Resultat<Option<Vec<PathBuf>>> {
    let n = noms(dossier);
    noter("dossier", dossier, empreinte_des_noms(&n));
    match n {
        Ok(n) => Ok(Some(n.into_iter().map(|x| dossier.join(x)).collect())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(crate::echec::du_disque(dossier, &e)),
    }
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
pub fn signature(chemin: &Path) -> Resultat<Option<(u128, u64)>> {
    let memo = CODE_FIGE.with(|c| c.borrow().as_ref().filter(|(code, _)| chemin.starts_with(code)).map(|(_, m)| m.get(chemin).copied()));
    match memo {
        Some(Some(s)) => Ok(s),
        Some(None) => {
            let s = signature_lue(chemin)?;
            CODE_FIGE.with(|c| {
                if let Some((_, m)) = c.borrow_mut().as_mut() {
                    m.insert(chemin.to_path_buf(), s);
                }
            });
            Ok(s)
        }
        None => signature_lue(chemin),
    }
}

/// `None` pour un fichier absent : rien ne se garde de lui ; un fichier qui ne dit pas sa date (illisible, d'avant 1970)
/// se refuse.
fn signature_lue(chemin: &Path) -> Resultat<Option<(u128, u64)>> {
    // Un fichier absent n'a pas de signature : rien ne se garde de lui.
    if !chemin.is_file() {
        return Ok(None);
    }
    let illisible = |e: std::io::Error| format!("{} ne dit pas sa date : {}", repr(&chemin.display().to_string()), e);
    let m = std::fs::metadata(chemin).map_err(illisible)?;
    let d = m.modified().map_err(illisible)?.duration_since(std::time::UNIX_EPOCH).map_err(|e| {
        format!("{} est daté d'avant 1970 ({} s) : régler la date du fichier.", repr(&chemin.display().to_string()), e.duration().as_secs())
    })?;
    Ok(Some((d.as_nanos(), m.len())))
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

/// Le cache d'une table livree lue par sa forme (`forme_en_cache`) : le chemin, sa signature, la table construite.
pub type FormeGardee<T> = Mutex<Option<(PathBuf, Option<(u128, u64)>, Arc<T>)>>;

/// UNE TABLE LIVREE (`donnees/<nom>`), lue par sa forme `B` (`forme::lire_fichier`, `phrase` pour son refus) et
/// construite par `construire` une fois tant que son fichier ne change pas (chemin, date, taille) ; servie du cache, elle
/// se note quand meme au registre d'un calcul suivi. Un calcul la consulte a chaque champ : elle ne se relit pas a chaque
/// fois.
pub fn forme_en_cache<B: serde::de::DeserializeOwned, T>(cache: &'static OnceLock<FormeGardee<T>>, nom: &str, phrase: &str, construire: impl Fn(B) -> Resultat<T>) -> Resultat<Arc<T>> {
    let chemin = dossier_du_code().join(nom);
    let sig = signature(&chemin)?;
    let cache = cache.get_or_init(|| Mutex::new(None));
    if let Some((p, s, v)) = crate::memoire::tenu(cache).as_ref() {
        if *p == chemin && sig.is_some() && *s == sig {
            noter_si_besoin(&chemin);
            return Ok(v.clone());
        }
    }
    let v = Arc::new(construire(crate::forme::lire_fichier(&chemin, phrase)?)?);
    *crate::memoire::tenu(cache) = Some((chemin, sig, v.clone()));
    Ok(v)
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

// ------------------------------------------------ les refus d'une donnee lue
//
// Un fichier ou une demande dont un champ manque ou n'a pas la forme attendue :
// la phrase nomme le champ, ce qui etait attendu et ce qui a ete lu.

/// « champ 'prenom' absent ».
pub fn absent(cle: &str) -> String {
    crate::echec::cle("Il manque une donnée attendue.", cle)
}

/// Une valeur dite en francais, depuis son nom de type (`NoneType`, `bool`, `int`, `float`, `str`, `list`, `dict`).
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

/// Le dossier de la configuration d'une compagnie, sous sa racine.
pub const CONFIG: &str = "config";

/// Le marqueur d'une compagnie qui n'est pas la vraie (une compagnie fictive, une copie de developpement).
pub const MARQUEUR_COPIE: &str = "COPIE-DE-DEVELOPPEMENT.json";

/// Le modele `nom` (gabarit de contrat, de DPAE, de certificat) : celui de la compagnie (son dossier `gabarits`),
/// a defaut celui du poste (`poste`), a defaut celui livre avec le logiciel (`donnees/modeles/`). Une compagnie qui
/// ecrit ses propres clauses passe devant ; une copie de la configuration du poste sans son dossier de gabarits reste la
/// meme compagnie et garde ceux du poste (son gabarit DPAE porte son centre de medecine du travail). Une compagnie qui
/// porte `MARQUEUR_COPIE` n'emprunte rien au poste : ses documents ne prennent jamais les clauses de la compagnie reelle.
pub fn modele_de(nom: &str, gabarits: Option<&Path>, poste: &Path) -> PathBuf {
    let propre = match gabarits {
        Some(g) => g.to_path_buf(),
        None => poste.to_path_buf(),
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
    String::from_utf8(octets).map_err(|e| erreur("UnicodeDecodeError", crate::echec::du_disque(&p, &e)))
}

/// Le nom d'un chemin, ou son texte entier s'il n'en a pas.
pub fn nom_du(chemin: &std::path::Path) -> String {
    match chemin.file_name() {
        Some(n) => n.to_string_lossy().into_owned(),
        None => chemin.to_string_lossy().into_owned(),
    }
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
