//! L'entrepot local des textes de la DILA (LEGI, KALI) : lu une fois, tenu a jour ensuite. Porte de
//! `atelier/outils/entrepot_dila.py` (retire le 15/10/2026) ; ses deux utilisateurs sont `releve_codes` (`paie releve-codes`)
//! et `releve_conventions` (`paie releve-conventions`).
//!
//! POURQUOI (24/09/2026, journal §122). La moisson des codes relisait, a CHAQUE passage, l'archive globale de la base LEGI
//! (1,1 Go) puis toutes les livraisons quotidiennes parues depuis (438 le 24/09/2026), et ne gardait que les articles
//! qu'on lui demandait. Un article de plus -- le 231 du code general des impots pour la taxe sur les salaires -- coutait
//! donc une heure. Et les conventions collectives, dont la veille (`kali.rs`) ne garde que la liste des textes, se
//! relisaient a la main dans la base KALI entiere.
//!
//! L'entrepot tient les deux choses qui manquaient :
//!
//!   1. LE TEXTE COMPLET, garde en local et compresse : tous les articles des codes et des conventions suivis, pas
//!      seulement ceux qu'on cite. Ajouter un article, c'est le lire dans l'entrepot -- quelques secondes, rien a
//!      telecharger.
//!   2. LA LISTE DES LIVRAISONS DEJA APPLIQUEES : a chaque passage, seules les nouvelles se telechargent -- une par soir
//!      ouvre, quelques centaines de Ko --, plusieurs a la fois, et s'appliquent dans l'ordre.
//!
//! L'archive globale ne se relit que pour construire l'entrepot la premiere fois, ou si on le demande (`--reconstruire`).
//!
//! Ce module ne sait rien des codes ni des conventions : il range des articles sous une cle, et il sait faire defiler des
//! archives (`LecteurTar`, que `kali.rs` emploie aussi). Les deux commandes qui s'en servent disent quoi garder.
//!
//! LES FICHIERS sont ceux qu'ecrivait le Python, champ pour champ : `articles.jsonl.gz` (une ligne par article, cles
//! triees, la cle de l'article sous `cle`, lignes triees par cle ; gzip a `mtime` 0 et au nom `articles.jsonl.gz.tmp`
//! dans l'en-tete, comme `gzip.GzipFile(tmp, mtime=0)`) et `etat.json` (`json.dump(indent=1, sort_keys=True)`, sans saut
//! de ligne final). Deux sauvegardes du meme contenu donnent le meme fichier : Git ne voit changer que ce qui a change.
//! Le flux compresse, lui, n'est pas celui de la zlib du Python (meme contenu, autres octets compresses : le premier
//! enregistrement en Rust reecrit le fichier une fois, ce que faisait de toute facon chaque livraison appliquee).

use std::collections::{BTreeMap, HashSet};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::OnceLock;

use regex::Regex;
use serde_json::{json, Map, Value};

pub const ARTICLES: &str = "articles.jsonl.gz";
pub const ETAT: &str = "etat.json";

type R<T> = Result<T, String>;

// ------------------------------------------------------------------ l'entrepot

/// Le separateur des parties d'une cle d'article (`("impots", "1724")`, `("3090", "KALIARTI...")`) dans la cle de la table :
/// aucune des parties ne le porte, et une cle se trie comme le tuple du Python (une partie prefixe d'une autre passe avant).
const SEP: char = '\u{0}';

/// La cle de table d'un article, depuis ses parties.
pub fn cle(parties: &[&str]) -> String {
    parties.join(&SEP.to_string())
}

/// Les parties d'une cle de table.
pub fn parties(cle: &str) -> Vec<&str> {
    cle.split(SEP).collect()
}

/// Des articles sous une cle, et l'etat de lecture.
///
/// `articles` : cle -> `{"id", "debut", ...}` ; l'ordre est celui du dictionnaire du Python (l'ordre du fichier au
/// chargement, puis les cles neuves a la fin ; une cle remplacee garde sa place) -- c'est lui qui range les articles d'un
/// `releve.json`. `etat` : `{"global", "lues", "changes", "construit_le", "mis_a_jour_le"}`.
pub struct Entrepot {
    pub dossier: PathBuf,
    pub articles: Map<String, Value>,
    pub etat: Map<String, Value>,
}

impl Entrepot {
    pub fn nouveau(dossier: &Path) -> Entrepot {
        let mut etat = Map::new();
        etat.insert("global".into(), json!(""));
        etat.insert("lues".into(), json!([]));
        etat.insert("changes".into(), json!([]));
        Entrepot { dossier: dossier.to_path_buf(), articles: Map::new(), etat }
    }

    pub fn existe(&self) -> bool {
        self.dossier.join(ARTICLES).exists() && self.dossier.join(ETAT).exists()
    }

    pub fn charger(mut self) -> R<Entrepot> {
        let p = self.dossier.join(ARTICLES);
        let f = std::fs::File::open(&p).map_err(|e| format!("{} : {}", p.display(), e))?;
        let lecteur = BufReader::new(flate2::read::MultiGzDecoder::new(BufReader::new(f)));
        for (n, ligne) in lecteur.split(b'\n').enumerate() {
            let ligne = ligne.map_err(|e| format!("{} : {}", p.display(), e))?;
            if ligne.is_empty() {
                continue;
            }
            let mut a: Map<String, Value> = match serde_json::from_slice(&ligne) {
                Ok(Value::Object(m)) => m,
                Ok(_) => return Err(format!("{}, ligne {} : pas un objet", p.display(), n + 1)),
                Err(e) => return Err(format!("{}, ligne {} illisible : {}", p.display(), n + 1, e)),
            };
            // `a.pop("cle")` : l'article garde l'ordre de ses autres champs.
            let parts: Vec<String> = match a.shift_remove("cle") {
                Some(Value::Array(v)) => v.iter().map(|x| x.as_str().unwrap_or_default().to_string()).collect(),
                _ => return Err(format!("{}, ligne {} : pas de « cle »", p.display(), n + 1)),
            };
            let refs: Vec<&str> = parts.iter().map(String::as_str).collect();
            self.articles.insert(cle(&refs), Value::Object(a));
        }
        let pe = self.dossier.join(ETAT);
        let t = std::fs::read_to_string(&pe).map_err(|e| format!("{} : {}", pe.display(), e))?;
        self.etat = match serde_json::from_str(&t) {
            Ok(Value::Object(m)) => m,
            Ok(_) => return Err(format!("{} n'est pas un objet JSON", pe.display())),
            Err(e) => return Err(format!("{} illisible : {}", pe.display(), e)),
        };
        Ok(self)
    }

    /// Ecrit les deux fichiers ; `aujourdhui` (AAAA-MM-JJ) devient `mis_a_jour_le`.
    pub fn sauver(&mut self, aujourdhui: &str) -> R<()> {
        std::fs::create_dir_all(&self.dossier).map_err(|e| format!("{} : {}", self.dossier.display(), e))?;
        let tmp = self.dossier.join(format!("{}.tmp", ARTICLES));
        {
            let f = std::fs::File::create(&tmp).map_err(|e| format!("{} : {}", tmp.display(), e))?;
            // L'en-tete de `gzip.GzipFile(tmp, "wb", mtime=0)` : le nom du fichier temporaire, la date zero, le niveau 9.
            let mut g = flate2::GzBuilder::new()
                .filename(format!("{}.tmp", ARTICLES))
                .mtime(0)
                .operating_system(255)
                .write(BufWriter::new(f), flate2::Compression::best());
            // Trie : deux sauvegardes du meme contenu donnent le meme fichier, et Git ne voit changer que ce qui a change.
            let mut cles: Vec<&String> = self.articles.keys().collect();
            cles.sort_by(|a, b| parties(a).cmp(&parties(b)));
            for k in cles {
                let ligne = ligne_triee(&self.articles[k.as_str()], &parties(k));
                g.write_all(ligne.as_bytes()).and_then(|_| g.write_all(b"\n")).map_err(|e| format!("{} : {}", tmp.display(), e))?;
            }
            let mut w = g.finish().map_err(|e| format!("{} : {}", tmp.display(), e))?;
            w.flush().map_err(|e| format!("{} : {}", tmp.display(), e))?;
        }
        let fin = self.dossier.join(ARTICLES);
        std::fs::rename(&tmp, &fin).map_err(|e| format!("{} : {}", fin.display(), e))?;
        self.etat.insert("mis_a_jour_le".into(), json!(aujourdhui));
        let p = self.dossier.join(ETAT);
        let texte = crate::ecriture::json_py(&trie(&Value::Object(self.etat.clone())), 1);
        std::fs::write(&p, texte).map_err(|e| format!("{} : {}", p.display(), e))
    }

    /// Les livraisons deja appliquees.
    pub fn lues(&self) -> HashSet<String> {
        liste(&self.etat, "lues").into_iter().collect()
    }

    /// `etat["lues"].append(nom)`.
    pub fn noter_lue(&mut self, nom: &str) {
        let v = self.etat.entry("lues").or_insert_with(|| json!([]));
        if !v.is_array() {
            *v = json!([]);
        }
        if let Some(a) = v.as_array_mut() {
            a.push(json!(nom));
        }
    }
}

/// Les chaines d'une liste de l'etat.
pub fn liste(etat: &Map<String, Value>, cle: &str) -> Vec<String> {
    etat.get(cle).and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default()
}

/// `json.dumps(dict(article, cle=list(cle)), ensure_ascii=False, sort_keys=True)`.
fn ligne_triee(article: &Value, cle: &[&str]) -> String {
    let mut entrees: Vec<(&str, Value)> = Vec::new();
    if let Some(o) = article.as_object() {
        for (k, v) in o {
            if k != "cle" {
                entrees.push((k.as_str(), v.clone()));
            }
        }
    }
    entrees.push(("cle", json!(cle)));
    entrees.sort_by(|a, b| a.0.cmp(b.0));
    let mut s = String::from("{");
    for (i, (k, v)) in entrees.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        s.push_str(&crate::ecriture::json_py_trie(&json!(k)));
        s.push_str(": ");
        s.push_str(&crate::ecriture::json_py_trie(v));
    }
    s.push('}');
    s
}

/// Les cles de chaque objet triees (`sort_keys=True`), recursivement.
pub fn trie(v: &Value) -> Value {
    match v {
        Value::Object(o) => {
            let mut cles: Vec<&String> = o.keys().collect();
            cles.sort();
            Value::Object(cles.into_iter().map(|k| (k.clone(), trie(&o[k.as_str()]))).collect())
        }
        Value::Array(a) => Value::Array(a.iter().map(trie).collect()),
        autre => autre.clone(),
    }
}

// ------------------------------------------------------------------ le reseau

/// D'ou viennent les pages et les archives : le reseau (`Reseau`), ou, dans les essais, des octets poses d'avance.
pub trait Source: Sync {
    /// Une page ou une archive entiere en memoire, un seul essai.
    fn lire(&self, url: &str, delai: u64) -> R<Vec<u8>>;
    /// Une archive lue en flux, sans jamais l'ecrire sur le disque (l'archive globale de LEGI fait 1,1 Go).
    fn flux(&self, url: &str) -> R<Box<dyn Read + Send>>;
    /// L'attente entre deux essais.
    fn pause(&self, secondes: u64) {
        std::thread::sleep(std::time::Duration::from_secs(secondes));
    }
}

/// Le site de la DILA, par HTTPS (racines du systeme, mandataire de l'environnement, comme `sources::telecharger`).
pub struct Reseau;

fn agent(delai_global: Option<u64>, url: &str) -> R<ureq::Agent> {
    let mut c = ureq::Agent::config_builder()
        .user_agent(crate::sources::AGENT)
        .timeout_connect(Some(std::time::Duration::from_secs(120)))
        .timeout_recv_response(Some(std::time::Duration::from_secs(600)));
    if let Some(s) = delai_global {
        c = c.timeout_global(Some(std::time::Duration::from_secs(s)));
    }
    c = c.tls_config(ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build());
    // En essai, le garde-fou reseau : rien ne sort de la machine.
    Ok(crate::garde_reseau::regler(c, url)?.build().into())
}

impl Source for Reseau {
    fn lire(&self, url: &str, delai: u64) -> R<Vec<u8>> {
        let mut r = agent(Some(delai), url)?.get(url).call().map_err(|e| format!("{} : {}", url, e))?;
        r.body_mut().with_config().limit(2 * 1024 * 1024 * 1024).read_to_vec().map_err(|e| format!("{} : {}", url, e))
    }

    fn flux(&self, url: &str) -> R<Box<dyn Read + Send>> {
        let r = agent(None, url)?.get(url).call().map_err(|e| format!("{} : {}", url, e))?;
        Ok(Box::new(r.into_body().into_reader()))
    }
}

/// Une archive entiere en memoire, avec trois nouvelles tentatives : une coupure de transport n'est pas une reponse.
pub fn telecharger(source: &dyn Source, url: &str) -> R<Vec<u8>> {
    let mut derniere = String::new();
    for n in 0..4u64 {
        match source.lire(url, 300) {
            Ok(o) => return Ok(o),
            Err(e) => {
                derniere = e;
                source.pause(2 * (n + 1));
            }
        }
    }
    Err(derniere)
}

/// Une page d'index, un seul essai (`urlopen(BASE, timeout=120)` du Python).
pub fn page(source: &dyn Source, url: &str) -> R<String> {
    Ok(String::from_utf8_lossy(&source.lire(url, 120)?).into_owned())
}

/// Telecharge plusieurs archives a la fois (`fils`), et les rend DANS L'ORDRE de la liste : une livraison s'applique apres
/// celle de la veille, jamais avant. `rendre(i, octets ou panne)` dit s'il faut continuer : `false` arrete (les
/// telechargements en cours finissent, aucun autre ne part). Les fils ne prennent pas plus de quatre tours d'avance sur
/// ce qui a ete rendu : rejouer 459 livraisons ne les garde pas toutes en memoire a la fois.
pub fn en_ordre(source: &dyn Source, urls: &[String], fils: usize, mut rendre: impl FnMut(usize, R<Vec<u8>>) -> bool) {
    if urls.is_empty() {
        return;
    }
    let suivant = AtomicUsize::new(0);
    let rendus = AtomicUsize::new(0);
    let arret = AtomicBool::new(false);
    let avance = 4 * fils.max(1);
    let (tx, rx) = std::sync::mpsc::channel::<(usize, R<Vec<u8>>)>();
    std::thread::scope(|s| {
        for _ in 0..fils.max(1).min(urls.len()) {
            let tx = tx.clone();
            let (suivant, rendus, arret) = (&suivant, &rendus, &arret);
            s.spawn(move || loop {
                if arret.load(Ordering::SeqCst) {
                    break;
                }
                let i = suivant.fetch_add(1, Ordering::SeqCst);
                if i >= urls.len() {
                    break;
                }
                while i >= rendus.load(Ordering::SeqCst) + avance && !arret.load(Ordering::SeqCst) {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                if arret.load(Ordering::SeqCst) {
                    break;
                }
                let r = telecharger(source, &urls[i]);
                if tx.send((i, r)).is_err() {
                    break;
                }
            });
        }
        drop(tx);
        let mut attente: BTreeMap<usize, R<Vec<u8>>> = BTreeMap::new();
        let mut prochain = 0;
        'recevoir: for (i, r) in rx.iter() {
            attente.insert(i, r);
            while let Some(r) = attente.remove(&prochain) {
                let continuer = rendre(prochain, r);
                prochain += 1;
                rendus.store(prochain, Ordering::SeqCst);
                if !continuer {
                    arret.store(true, Ordering::SeqCst);
                    break 'recevoir;
                }
            }
        }
        // Les fils encore en route finissent leur telechargement ; `rx` lache, leurs envois echouent et ils s'arretent.
        drop(rx);
    });
}

/// Les estampilles `AAAAMMJJ-HHMMSS` des livraisons `<PREFIXE>_...tar.gz` d'un index, sans doublon, dans l'ordre.
pub fn livraisons(index_html: &str, prefixe: &str) -> Vec<String> {
    let m = Regex::new(&format!(r"{}_(\d{{8}}-\d{{6}})\.tar\.gz", regex::escape(prefixe))).expect("motif des livraisons");
    let mut v: Vec<String> = m.captures_iter(index_html).map(|c| c[1].to_string()).collect();
    v.sort();
    v.dedup();
    v
}

// ------------------------------------------------------------------ l'archive

/// Un membre d'une archive tar : son nom, sa taille, et s'il est un fichier ordinaire (`TarInfo.isfile()`).
pub struct Membre {
    pub nom: String,
    pub taille: u64,
    pub fichier: bool,
}

/// Une archive tar lue EN FLUX (POSIX ustar, avec le prefixe de nom, les noms longs GNU `L` et l'en-tete pax `path=`) :
/// 512 octets d'en-tete, le contenu, le bourrage. Le contenu d'un membre se lit (`lire`) ou se saute (au `suivant`), sans
/// jamais tenir l'archive entiere : l'archive globale de LEGI fait 1,1 Go.
pub struct LecteurTar<L: Read> {
    r: L,
    /// Ce qui reste du membre courant : son contenu non lu, puis son bourrage.
    contenu: u64,
    bourrage: u64,
    fini: bool,
}

/// Une archive `.tar.gz` en flux (`tarfile.open(mode="r|gz")`).
pub fn tar_gz<L: Read>(r: L) -> LecteurTar<BufReader<flate2::read::MultiGzDecoder<BufReader<L>>>> {
    LecteurTar::nouveau(BufReader::with_capacity(1 << 16, flate2::read::MultiGzDecoder::new(BufReader::new(r))))
}

/// Une archive `.tar.gz` en memoire (`tarfile.open(fileobj=BytesIO(octets), mode="r:gz")`).
pub fn archive(octets: &[u8]) -> LecteurTar<BufReader<flate2::read::MultiGzDecoder<BufReader<&[u8]>>>> {
    tar_gz(octets)
}

fn champ(b: &[u8]) -> String {
    let fin = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..fin]).into_owned()
}

fn taille_tar(b: &[u8]) -> R<u64> {
    // Le codage base 256 de GNU (bit de poids fort du premier octet) pour les tailles qui ne tiennent pas en octal.
    if b[0] & 0x80 != 0 {
        return Ok(b[1..].iter().fold(0u64, |t, &c| (t << 8) | c as u64));
    }
    let t = champ(b);
    let t = t.trim_matches(|c: char| c == ' ' || c == '\0');
    if t.is_empty() {
        return Ok(0);
    }
    u64::from_str_radix(t, 8).map_err(|_| format!("archive illisible (tar) : taille « {} »", t))
}

impl<L: Read> LecteurTar<L> {
    pub fn nouveau(r: L) -> LecteurTar<L> {
        LecteurTar { r, contenu: 0, bourrage: 0, fini: false }
    }

    fn sauter(&mut self, n: u64) -> R<()> {
        let copie = std::io::copy(&mut (&mut self.r).take(n), &mut std::io::sink()).map_err(|e| format!("archive illisible : {}", e))?;
        if copie < n {
            return Err("archive illisible (tar) : fichier tronque".into());
        }
        Ok(())
    }

    /// Le bloc d'en-tete suivant ; `None` a la fin propre du flux.
    fn bloc(&mut self) -> R<Option<[u8; 512]>> {
        let mut h = [0u8; 512];
        let mut lu = 0;
        while lu < 512 {
            match self.r.read(&mut h[lu..]) {
                Ok(0) => break,
                Ok(n) => lu += n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => return Err(format!("archive illisible : {}", e)),
            }
        }
        // Une archive sans ses blocs de fin, ou coupee entre deux membres : on s'arrete la, comme `kali.rs` avant.
        Ok(if lu < 512 { None } else { Some(h) })
    }

    fn lire_n(&mut self, n: u64) -> R<Vec<u8>> {
        let mut v = Vec::with_capacity(n.min(1 << 24) as usize);
        (&mut self.r).take(n).read_to_end(&mut v).map_err(|e| format!("archive illisible : {}", e))?;
        if (v.len() as u64) < n {
            return Err("archive illisible (tar) : fichier tronque".into());
        }
        Ok(v)
    }

    /// Le membre suivant (le reste du courant est saute), ou `None` a la fin de l'archive.
    pub fn suivant(&mut self) -> R<Option<Membre>> {
        if self.fini {
            return Ok(None);
        }
        let reste = self.contenu + self.bourrage;
        self.contenu = 0;
        self.bourrage = 0;
        self.sauter(reste)?;
        let mut nom_suivant: Option<String> = None;
        loop {
            let Some(h) = self.bloc()? else {
                self.fini = true;
                return Ok(None);
            };
            if h.iter().all(|&c| c == 0) {
                self.fini = true;
                return Ok(None);
            }
            let taille = taille_tar(&h[124..136])?;
            let type_ = h[156];
            let bourrage = taille.div_ceil(512) * 512 - taille;
            match type_ {
                // Le nom long du membre qui suit (GNU) ; le lien long, sans interet.
                b'L' | b'K' => {
                    let d = self.lire_n(taille)?;
                    self.sauter(bourrage)?;
                    if type_ == b'L' {
                        nom_suivant = Some(champ(&d));
                    }
                }
                // L'en-tete pax : `longueur cle=valeur\n`, dont `path`.
                b'x' | b'g' => {
                    let d = self.lire_n(taille)?;
                    self.sauter(bourrage)?;
                    if type_ == b'x' {
                        for ligne in String::from_utf8_lossy(&d).lines() {
                            if let Some((_, kv)) = ligne.split_once(' ') {
                                if let Some(p) = kv.strip_prefix("path=") {
                                    nom_suivant = Some(p.to_string());
                                }
                            }
                        }
                    }
                }
                _ => {
                    let nom = match nom_suivant.take() {
                        Some(n) => n,
                        None => {
                            let prefixe = if &h[257..262] == b"ustar" { champ(&h[345..500]) } else { String::new() };
                            let n = champ(&h[0..100]);
                            if prefixe.is_empty() {
                                n
                            } else {
                                format!("{}/{}", prefixe, n)
                            }
                        }
                    };
                    // Un fichier ordinaire (`0`, l'ancien NUL, `7`) ; les dossiers et les liens n'ont pas de contenu a lire.
                    let fichier = matches!(type_, b'0' | 0 | b'7');
                    // Un lien ou un dossier n'a pas de contenu dans l'archive, quelle que soit sa taille declaree.
                    let (contenu, bourrage) = if matches!(type_, b'1' | b'2' | b'5') { (0, 0) } else { (taille, bourrage) };
                    self.contenu = contenu;
                    self.bourrage = bourrage;
                    return Ok(Some(Membre { nom, taille: contenu, fichier }));
                }
            }
        }
    }

    /// Le contenu du membre courant (une fois).
    pub fn lire(&mut self) -> R<Vec<u8>> {
        let n = self.contenu;
        self.contenu = 0;
        self.lire_n(n)
    }
}

/// Un blanc au sens de Python (`str.isspace`, `\s` d'un motif `str`) : celui d'Unicode, plus `\x1c` a `\x1f`.
pub fn blanc_py(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// `str.strip()`.
pub fn strip_py(t: &str) -> &str {
    t.trim_matches(blanc_py)
}

/// `html.unescape` pour ce que portent les XML de la DILA : les references numeriques (avec les regles de Python : `&#13;`
/// rend `\r`, `&#128;`-`&#159;` la page 1252, `&#0;` et les substituts U+FFFD, les points de code interdits rien) et les
/// entites nommees `amp`, `lt`, `gt`, `quot`, `apos`, `nbsp`. INFERENCE : un XML bien forme ne connait que ces cinq-la
/// (`nbsp` en plus, par prudence) ; le releve du 15/10/2026 de `legi-entrepot/` (55 035 articles) ne porte que `&#13;`,
/// `&#10;`, `&gt;`, `&lt;` et `&amp;`. Une autre entite nommee resterait telle quelle, la ou le Python la decoderait.
pub fn unescape_html(t: &str) -> String {
    if !t.contains('&') {
        return t.to_string();
    }
    static M: OnceLock<Regex> = OnceLock::new();
    let m = M.get_or_init(|| Regex::new(r"&(#[0-9]+;?|#[xX][0-9a-fA-F]+;?|[^\t\n\x0C <&#;]{1,32};?)").expect("motif des entites"));
    m.replace_all(t, |c: &regex::Captures| {
        let s = &c[1];
        if let Some(num) = s.strip_prefix('#') {
            let num = num.trim_end_matches(';');
            let n = if let Some(h) = num.strip_prefix(['x', 'X']) { u64::from_str_radix(h, 16) } else { num.parse::<u64>() };
            return reference_numerique(n.unwrap_or(u64::MAX));
        }
        // Le nom entier, puis (comme le Python) le plus long debut qui en est un : `&ampx` rend `&x`.
        if let Some(r) = entite(s) {
            return r.to_string();
        }
        let bornes: Vec<usize> = s.char_indices().map(|(i, _)| i).skip(2).collect();
        for &x in bornes.iter().rev() {
            if let Some(r) = entite(&s[..x]) {
                return format!("{}{}", r, &s[x..]);
            }
        }
        c[0].to_string()
    })
    .into_owned()
}

/// Les entites nommees connues, avec le point-virgule ou sans (les formes historiques de HTML5).
fn entite(nom: &str) -> Option<&'static str> {
    Some(match nom {
        "amp;" | "amp" => "&",
        "lt;" | "lt" => "<",
        "gt;" | "gt" => ">",
        "quot;" | "quot" => "\"",
        "nbsp;" | "nbsp" => "\u{a0}",
        "apos;" => "'",
        _ => return None,
    })
}

/// `html._replace_charref` pour une reference numerique.
fn reference_numerique(n: u64) -> String {
    const CP1252: [char; 32] = [
        '\u{20ac}', '\u{81}', '\u{201a}', '\u{192}', '\u{201e}', '\u{2026}', '\u{2020}', '\u{2021}', '\u{2c6}', '\u{2030}', '\u{160}',
        '\u{2039}', '\u{152}', '\u{8d}', '\u{17d}', '\u{8f}', '\u{90}', '\u{2018}', '\u{2019}', '\u{201c}', '\u{201d}', '\u{2022}',
        '\u{2013}', '\u{2014}', '\u{2dc}', '\u{2122}', '\u{161}', '\u{203a}', '\u{153}', '\u{9d}', '\u{17e}', '\u{178}',
    ];
    match n {
        0 => "\u{fffd}".into(),
        0x0d => "\r".into(),
        0x80..=0x9f => CP1252[(n - 0x80) as usize].to_string(),
        0xd800..=0xdfff => "\u{fffd}".into(),
        n if n > 0x10ffff => "\u{fffd}".into(),
        0x1..=0x8 | 0xb | 0xe..=0x1f | 0x7f..=0x9f | 0xfdd0..=0xfdef => String::new(),
        n if (n & 0xfffe) == 0xfffe => String::new(),
        n => char::from_u32(n as u32).map(String::from).unwrap_or_default(),
    }
}

/// `s[:n]` : les `n` premiers caracteres.
pub fn debut_py(s: &str, n: usize) -> &str {
    match s.char_indices().nth(n) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

/// `str(v)` d'une valeur lue dans un JSON.
pub fn texte(v: &Value) -> String {
    crate::etats::py_str(v)
}

/// La verite d'une valeur au sens du Python (`if v:`).
pub fn vrai(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64() != Some(0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// Ecrit une ligne sur la sortie standard ; un tuyau ferme n'est pas une erreur.
pub fn dire(t: &str) {
    let mut o = std::io::stdout().lock();
    let _ = writeln!(o, "{}", t);
    let _ = o.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_blancs_et_les_entites_sont_ceux_de_python() {
        assert_eq!(strip_py("\u{1f} x \u{a0}"), "x");
        // Ce que rend `html.unescape` du Python sur la meme chaine.
        assert_eq!(unescape_html("a &amp;lt; &#13;&#x41;&#128;&#1;&inconnue; &ampx &ltfoo; &apos &apos;"), "a &lt; \rA\u{20ac}&inconnue; &x <foo; &apos '");
        assert_eq!(debut_py("éèà", 2), "éè");
    }
}
