//! Le tri de la boite aux lettres `import/`. Une compagnie y met tout ce qu'elle a -- des
//! PDF (un bulletin par fichier, une annee par fichier), des DSN, des AEM, des zip, des doublons, des
//! fichiers qui n'ont rien a y faire -- et le moteur TRIE, PAR LE CONTENU, JAMAIS PAR LE NOM : le
//! « 2023.pdf » qui contient 2026 le prouve.
//!
//! Chaque fichier recoit une sorte, et une seule :
//!
//! | sorte | ce que le contenu dit |
//! |---|---|
//! | `dsn` | l'en-tete `S10.G00.00.001,` (comme `importation::fichiers_dsn`), et un mois declare (`importation::lire_une_dsn`) |
//! | `aem` | chaque enregistrement fait 1000 caracteres, commence par `AEMEDIC2` et porte ses dates aux positions que `importation::lire_aem` lit |
//! | `pdf_texte` | un PDF (`%PDF-`) que la fabrique ouvre, et dont au moins une page a des mots |
//! | `pdf_sans_texte` | un PDF que la fabrique ouvre, sans un seul mot (lettres en traces, ou page scannee : pas d'OCR) |
//! | `archive` | un zip, ouvert dans le dossier de travail ; son contenu est trie comme le reste |
//! | `doublon` | la meme empreinte SHA-256 qu'un fichier deja trie (`doublon_de` dit lequel) |
//! | `illisible` | un PDF que la fabrique n'ouvre pas, un zip qui ne s'ouvre pas, ce qui commence comme une DSN ou une AEM sans en avoir la forme |
//! | `autre` | tout le reste, avec son extension |
//!
//! RIEN NE SE DEVINE (regle 1) : un fichier qu'on ne reconnait pas est `autre`, pas « sans doute une DSN ».
//! Rien n'est ecrit dans les donnees de la compagnie : seuls les zip s'ouvrent, et dans `travail/`
//! seulement, un sous-dossier par archive, chemins d'archive assainis (`enclosed_name`).

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use socle::texte::{lignes_du_texte, texte_latin1};
use socle::echec::R;
use socle::echec::erreur;

/// Les sortes, dans l'ordre ou le compte rendu les compte.
pub const SORTES: [Sorte; 8] =
    [Sorte::Dsn, Sorte::Aem, Sorte::PdfTexte, Sorte::PdfSansTexte, Sorte::Archive, Sorte::Doublon, Sorte::Illisible, Sorte::Autre];

/// Une archive dans une archive dans une archive... s'ouvre jusqu'a cette profondeur, pas au-dela.
const PROFONDEUR_MAX: usize = 4;

/// Ce qu'une archive peut rendre une fois decompressee, en tout : au-dela, elle n'est pas ouverte (une
/// « bombe » de zip remplirait le disque).
const DECOMPRESSE_MAX: u64 = 2 * 1024 * 1024 * 1024;

/// Le gabarit AEM que l'import lit (`importation::lire_aem`) : 1000 caracteres par enregistrement.
const AEM_LONGUEUR: usize = 1000;
const AEM_MARQUE: &str = "AEMEDIC2";

/// Ce que l'import accepte, dit a l'ecran (le guide) et a chaque fichier refuse.
pub const ACCEPTE: &str = "L'import accepte les PDF de bulletins de paie, d'AEM, de certificats Congés Spectacles et d'AER Cinéma Spectacle, et les fichiers DSN et AEM déposés.";

/// La raison d'un PDF sans texte : une image ne se lit pas.
pub const IMAGE: &str = "ce document est une image ; déposez le PDF d'origine";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sorte {
    Dsn,
    Aem,
    PdfTexte,
    PdfSansTexte,
    Archive,
    Doublon,
    Illisible,
    Autre,
}

impl Sorte {
    pub fn nom(self) -> &'static str {
        match self {
            Sorte::Dsn => "dsn",
            Sorte::Aem => "aem",
            Sorte::PdfTexte => "pdf_texte",
            Sorte::PdfSansTexte => "pdf_sans_texte",
            Sorte::Archive => "archive",
            Sorte::Doublon => "doublon",
            Sorte::Illisible => "illisible",
            Sorte::Autre => "autre",
        }
    }
}

/// Une page d'un PDF, telle que la fabrique l'a relevee (`fabrique_pdf::inspection`).
#[derive(Clone, Debug)]
pub struct PageLue {
    pub mots: usize,
    pub polices: usize,
    pub images: usize,
    pub traits: usize,
}

/// Un fichier trie.
#[derive(Clone, Debug)]
pub struct Fichier {
    /// Le chemin relatif au dossier trie, en `/` ; dans une archive : `lot.zip/sous/fichier.dsn`.
    pub chemin: String,
    /// Ou il se lit sur le disque (sous `travail/` pour ce qui sort d'une archive).
    pub disque: PathBuf,
    pub sorte: Sorte,
    pub taille: u64,
    /// SHA-256 en hexadecimal ; vide pour une entree d'archive qui n'a pas ete extraite.
    pub empreinte: String,
    /// L'extension, en minuscules, sans le point (`""` s'il n'y en a pas). Elle ne decide de rien.
    pub extension: String,
    /// Les pages d'un PDF ouvert ; vide pour le reste.
    pub pages: Vec<PageLue>,
    /// Pourquoi, en une phrase, quand il y a quelque chose a dire.
    pub raison: Option<String>,
    /// Le fichier dont celui-ci est la copie exacte.
    pub doublon_de: Option<String>,
    /// L'archive d'ou il sort.
    pub archive: Option<String>,
    /// Le mois qu'une DSN declare (`AAAA-MM`).
    pub mois: Option<String>,
    /// Le nombre d'enregistrements d'une AEM.
    pub enregistrements: Option<usize>,
}

/// Le tri d'un dossier : chaque fichier, dans l'ordre du parcours (noms tries, une archive suivie de
/// son contenu).
#[derive(Clone, Debug)]
pub struct Tri {
    pub dossier: PathBuf,
    pub fichiers: Vec<Fichier>,
}

impl Tri {
    /// Combien de fichiers ont cette sorte.
    pub fn compte(&self, sorte: Sorte) -> usize {
        self.fichiers.iter().filter(|f| f.sorte == sorte).count()
    }

    /// Ce que l'ecran montre : la liste des fichiers et les comptes par sorte (toutes les sortes, meme a 0).
    pub fn json(&self) -> Value {
        let fichiers: Vec<Value> = self.fichiers.iter().map(fichier_json).collect();
        let mut comptes = Map::new();
        for s in SORTES {
            comptes.insert(s.nom().to_string(), json!(self.compte(s)));
        }
        json!({
            "dossier": self.dossier.display().to_string(),
            "fichiers": fichiers,
            "comptes": comptes,
            "total": self.fichiers.len(),
        })
    }
}

fn fichier_json(f: &Fichier) -> Value {
    let mut m = Map::new();
    m.insert("chemin".into(), json!(f.chemin));
    m.insert("sorte".into(), json!(f.sorte.nom()));
    m.insert("taille".into(), json!(f.taille));
    m.insert("empreinte".into(), json!(f.empreinte));
    m.insert("extension".into(), json!(f.extension));
    if matches!(f.sorte, Sorte::PdfTexte | Sorte::PdfSansTexte) {
        let pages: Vec<Value> = f
            .pages
            .iter()
            .enumerate()
            .map(|(i, p)| {
                json!({"page": i + 1, "texte": p.mots > 0, "mots": p.mots, "polices": p.polices, "images": p.images, "traits": p.traits})
            })
            .collect();
        m.insert("pages".into(), Value::Array(pages));
    }
    let mut option = |cle: &str, v: Option<Value>| {
        if let Some(v) = v {
            m.insert(cle.into(), v);
        }
    };
    option("raison", f.raison.as_ref().map(|r| json!(r)));
    option("doublon_de", f.doublon_de.as_ref().map(|d| json!(d)));
    option("archive", f.archive.as_ref().map(|a| json!(a)));
    option("mois", f.mois.as_ref().map(|x| json!(x)));
    option("enregistrements", f.enregistrements.map(|n| json!(n)));
    Value::Object(m)
}

/// `trier(dossier, travail)` : parcourt `dossier` et ses sous-dossiers, ouvre chaque zip dans `travail/`
/// (cree s'il manque ; s'il est sous `dossier`, il n'est pas parcouru), et donne a chaque fichier sa sorte.
pub fn trier(dossier: &Path, travail: &Path) -> R<Tri> {
    if !dossier.is_dir() {
        return Err(erreur("ErreurOperation", format!("Le dossier a trier n'existe pas : {}", dossier.display())));
    }
    std::fs::create_dir_all(travail)
        .map_err(|e| erreur("ErreurOperation", format!("Le dossier de travail ne se cree pas : {} ({})", travail.display(), e)))?;
    let mut t = Trieur { travail: travail.to_path_buf(), travail_canon: std::fs::canonicalize(travail).ok(), vus: HashMap::new(), fichiers: Vec::new() };
    t.parcourir(dossier, "", None, 0);
    Ok(Tri { dossier: dossier.to_path_buf(), fichiers: t.fichiers })
}

struct Trieur {
    travail: PathBuf,
    travail_canon: Option<PathBuf>,
    /// Empreinte -> chemin du premier fichier qui l'a portee.
    vus: HashMap<String, String>,
    fichiers: Vec<Fichier>,
}

/// L'extension d'un nom, en minuscules, sans le point.
fn extension(nom: &str) -> String {
    match nom.rfind('.') {
        Some(i) if i > 0 && i < nom.len() - 1 => nom[i + 1..].to_lowercase(),
        _ => String::new(),
    }
}

fn hexa(octets: &[u8]) -> String {
    Sha256::digest(octets).iter().map(|b| format!("{:02x}", b)).collect()
}

impl Trieur {
    fn nouveau(&self, chemin: String, disque: PathBuf, archive: Option<&str>) -> Fichier {
        let nom = chemin.rsplit('/').next().unwrap_or("").to_string();
        Fichier {
            extension: extension(&nom),
            chemin,
            disque,
            sorte: Sorte::Autre,
            taille: 0,
            empreinte: String::new(),
            pages: Vec::new(),
            raison: None,
            doublon_de: None,
            archive: archive.map(String::from),
            mois: None,
            enregistrements: None,
        }
    }

    /// Les entrees d'un dossier, par nom, sous-dossiers compris.
    fn parcourir(&mut self, dossier: &Path, prefixe: &str, archive: Option<&str>, profondeur: usize) {
        let mut entrees: Vec<(String, PathBuf, std::fs::FileType)> = match std::fs::read_dir(dossier) {
            Ok(l) => l.flatten().filter_map(|e| Some((e.file_name().to_string_lossy().into_owned(), e.path(), e.file_type().ok()?))).collect(),
            Err(e) => {
                let mut f = self.nouveau(prefixe.trim_end_matches('/').to_string(), dossier.to_path_buf(), archive);
                f.sorte = Sorte::Illisible;
                f.raison = Some(format!("dossier qui ne se lit pas ({})", e));
                self.fichiers.push(f);
                return;
            }
        };
        entrees.sort_by(|a, b| a.0.cmp(&b.0));
        for (nom, chemin, genre) in entrees {
            let rel = format!("{}{}", prefixe, nom);
            if genre.is_dir() {
                if self.travail_canon.is_some() && std::fs::canonicalize(&chemin).ok() == self.travail_canon {
                    continue;
                }
                self.parcourir(&chemin, &format!("{}/", rel), archive, profondeur);
            } else if genre.is_symlink() {
                let mut f = self.nouveau(rel, chemin, archive);
                f.raison = Some("lien symbolique : il n'est pas suivi".into());
                self.fichiers.push(f);
            } else {
                self.classer(rel, chemin, archive, profondeur);
            }
        }
    }

    /// Un fichier : son empreinte, puis ce que son contenu dit.
    fn classer(&mut self, rel: String, disque: PathBuf, archive: Option<&str>, profondeur: usize) {
        let mut f = self.nouveau(rel, disque, archive);
        let octets = match std::fs::read(&f.disque) {
            Ok(o) => o,
            Err(e) => {
                f.sorte = Sorte::Illisible;
                f.raison = Some(format!("le fichier ne se lit pas ({})", e));
                self.fichiers.push(f);
                return;
            }
        };
        f.taille = octets.len() as u64;
        f.empreinte = hexa(&octets);
        if let Some(premier) = self.vus.get(&f.empreinte) {
            f.sorte = Sorte::Doublon;
            f.doublon_de = Some(premier.clone());
            f.raison = Some(format!("copie exacte de {}", premier));
            self.fichiers.push(f);
            return;
        }
        self.vus.insert(f.empreinte.clone(), f.chemin.clone());
        if octets.is_empty() {
            f.raison = Some("fichier vide".into());
            self.fichiers.push(f);
            return;
        }
        if est_un_zip(&octets) {
            self.ouvrir_archive(f, &octets, profondeur);
            return;
        }
        if est_un_pdf(&octets) {
            lire_pdf(&mut f);
        } else if apres_bom(&octets).starts_with(b"S10.G00.00.001,") {
            match crate::import::dsn_aem::lire_une_dsn(&f.disque) {
                Some(d) => {
                    f.sorte = Sorte::Dsn;
                    f.mois = d.get("mois").and_then(Value::as_str).map(String::from);
                }
                None => {
                    f.sorte = Sorte::Illisible;
                    f.raison = Some("commence comme une DSN, mais ne dit pas son mois (S20.G00.05.005)".into());
                }
            }
        } else if apres_bom(&octets).starts_with(b"AEMEDI") {
            lire_aem(&mut f, &octets);
        } else {
            f.raison = Some(format!("ce fichier n'est ni un PDF, ni une DSN, ni une AEM, ni une archive zip. {}", ACCEPTE));
        }
        self.fichiers.push(f);
    }

    /// Un zip : ouvert dans son propre sous-dossier de `travail/`, puis son contenu trie a la suite.
    fn ouvrir_archive(&mut self, mut f: Fichier, octets: &[u8], profondeur: usize) {
        if profondeur >= PROFONDEUR_MAX {
            f.raison = Some(format!("zip rangé à plus de {} niveaux de zips l'un dans l'autre : pas ouvert", PROFONDEUR_MAX));
            self.fichiers.push(f);
            return;
        }
        let mut z = match zip::ZipArchive::new(std::io::Cursor::new(octets)) {
            Ok(z) => z,
            Err(e) => {
                f.sorte = Sorte::Illisible;
                f.raison = Some(format!("archive zip qui ne s'ouvre pas ({})", e));
                self.fichiers.push(f);
                return;
            }
        };
        let nom = f.chemin.rsplit('/').next().unwrap_or("archive");
        let assaini: String = nom.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' }).collect();
        let cible = self.travail.join(format!("{}-{}", assaini, &f.empreinte[..12]));
        let _ = std::fs::remove_dir_all(&cible);
        if let Err(e) = std::fs::create_dir_all(&cible) {
            f.sorte = Sorte::Illisible;
            f.raison = Some(format!("le dossier de travail de l'archive ne se cree pas ({})", e));
            self.fichiers.push(f);
            return;
        }
        // Les entrees qui ne s'extraient pas : elles ont leur sort elles aussi, apres l'archive.
        let mut refusees: Vec<Fichier> = Vec::new();
        let mut extraites = 0usize;
        let mut total: u64 = 0;
        for i in 0..z.len() {
            let mut e = match z.by_index(i) {
                Ok(e) => e,
                Err(err) => {
                    let mut r = self.nouveau(format!("{}/#{}", f.chemin, i + 1), cible.clone(), Some(&f.chemin));
                    r.sorte = Sorte::Illisible;
                    r.raison = Some(format!("entree d'archive qui ne se lit pas ({})", err));
                    refusees.push(r);
                    continue;
                }
            };
            let brut = e.name().to_string();
            let Some(rel) = e.enclosed_name() else {
                let mut r = self.nouveau(format!("{}/{}", f.chemin, brut), cible.clone(), Some(&f.chemin));
                r.sorte = Sorte::Illisible;
                r.taille = e.size();
                r.raison = Some("chemin d'archive refuse : il sortirait du dossier de l'archive".into());
                refusees.push(r);
                continue;
            };
            let chemin = cible.join(rel);
            if e.is_dir() {
                let _ = std::fs::create_dir_all(&chemin);
                continue;
            }
            let mut contenu = Vec::new();
            let lu = (&mut e).take(DECOMPRESSE_MAX - total + 1).read_to_end(&mut contenu);
            let ecrit = lu.map_err(|x| x.to_string()).and_then(|n| {
                total += n as u64;
                if total > DECOMPRESSE_MAX {
                    return Err(format!("l'archive rend plus de {} Mo decompresses", DECOMPRESSE_MAX / (1024 * 1024)));
                }
                if let Some(p) = chemin.parent() {
                    std::fs::create_dir_all(p).map_err(|x| x.to_string())?;
                }
                std::fs::write(&chemin, &contenu).map_err(|x| x.to_string())
            });
            match ecrit {
                Ok(()) => extraites += 1,
                Err(m) => {
                    let mut r = self.nouveau(format!("{}/{}", f.chemin, brut.trim_end_matches('/')), chemin.clone(), Some(&f.chemin));
                    r.sorte = Sorte::Illisible;
                    r.taille = e.size();
                    r.raison = Some(format!("entree d'archive qui ne s'extrait pas ({})", m));
                    let _ = std::fs::remove_file(&chemin);
                    refusees.push(r);
                    if total > DECOMPRESSE_MAX {
                        break;
                    }
                }
            }
        }
        f.sorte = Sorte::Archive;
        f.raison = Some(format!("zip ouvert : {} fichier(s) extrait(s)", extraites));
        let chemin_archive = f.chemin.clone();
        self.fichiers.push(f);
        self.fichiers.extend(refusees);
        self.parcourir(&cible, &format!("{}/", chemin_archive), Some(&chemin_archive), profondeur + 1);
    }
}

/// Les marques d'ordre d'octets UTF-8 en tete (comme `importation::fichiers_dsn`).
fn apres_bom(mut o: &[u8]) -> &[u8] {
    while o.starts_with(b"\xef\xbb\xbf") {
        o = &o[3..];
    }
    o
}

/// Un zip commence par un en-tete local (`PK\3\4`), ou, vide, par la fin du repertoire central (`PK\5\6`).
fn est_un_zip(o: &[u8]) -> bool {
    o.starts_with(b"PK\x03\x04") || o.starts_with(b"PK\x05\x06")
}

/// L'en-tete `%PDF-` se trouve dans les 1024 premiers octets (ISO 32000-1, annexe H : les lecteurs le
/// cherchent la, pas seulement au tout debut).
fn est_un_pdf(o: &[u8]) -> bool {
    o[..o.len().min(1024)].windows(5).any(|w| w == b"%PDF-")
}

/// Un PDF : la fabrique l'ouvre (dans le meme processus) et releve chaque page.
fn lire_pdf(f: &mut Fichier) {
    let (texte, avertissements) = match fabrique_pdf::inspection::inspecter(&f.disque.to_string_lossy()) {
        Ok(r) => r,
        Err(r) => {
            f.sorte = Sorte::Illisible;
            // Le refus de la fabrique commence par le chemin du disque : seule la cause compte ici.
            let cause = r.0.strip_prefix(&*f.disque.to_string_lossy()).unwrap_or(&r.0).trim_start_matches([' ', ':']).to_string();
            f.raison = Some(format!("PDF que la fabrique n'ouvre pas : {}", cause));
            return;
        }
    };
    let releve: Value = match serde_json::from_str(&texte) {
        Ok(v) => v,
        Err(e) => {
            f.sorte = Sorte::Illisible;
            f.raison = Some(format!("PDF dont le releve ne se lit pas ({})", e));
            return;
        }
    };
    let n = |v: &Value, k: &str| v.get(k).and_then(Value::as_array).map_or(0, Vec::len);
    f.pages = releve
        .get("pages")
        .and_then(Value::as_array)
        .map(|ps| ps.iter().map(|p| PageLue { mots: n(p, "mots"), polices: n(p, "polices"), images: n(p, "images"), traits: n(p, "traits") }).collect())
        .unwrap_or_default();
    let avec = f.pages.iter().filter(|p| p.mots > 0).count();
    let total = f.pages.len();
    let mut raisons: Vec<String> = Vec::new();
    if total == 0 {
        f.sorte = Sorte::PdfSansTexte;
        raisons.push("PDF sans aucune page".into());
    } else if avec == 0 {
        f.sorte = Sorte::PdfSansTexte;
        raisons.push(IMAGE.into());
    } else {
        f.sorte = Sorte::PdfTexte;
        if avec < total {
            raisons.push(format!("{} page(s) sur {} sans aucun mot", total - avec, total));
        }
    }
    if !avertissements.is_empty() {
        raisons.push(format!("la fabrique a signale : {}", avertissements.join(" ; ")));
    }
    if !raisons.is_empty() {
        f.raison = Some(raisons.join(" ; "));
    }
}

/// Une AEM : chaque enregistrement (ligne non vide) a la forme que `importation::lire_aem` lit -- 1000
/// caracteres, la marque `AEMEDIC2`, des dates de debut et de fin AAAAMMJJ aux positions 618 et 627.
fn lire_aem(f: &mut Fichier, octets: &[u8]) {
    let lignes: Vec<Vec<char>> =
        lignes_du_texte(&texte_latin1(apres_bom(octets))).into_iter().filter(|l| !l.is_empty()).map(|l| l.chars().collect()).collect();
    let date = |l: &[char], a: usize| socle::date::lire_aaaammjj(&l[a..a + 8].iter().collect::<String>()).is_some();
    for (i, l) in lignes.iter().enumerate() {
        let marque: String = l.iter().take(AEM_MARQUE.len()).collect();
        let defaut = if marque != AEM_MARQUE {
            Some(format!("ne porte pas la marque {} (« {} »)", AEM_MARQUE, marque))
        } else if l.len() != AEM_LONGUEUR {
            Some(format!("fait {} caracteres au lieu de {}", l.len(), AEM_LONGUEUR))
        } else if !date(l, 618) || !date(l, 627) {
            Some("n'a pas ses dates de debut et de fin (positions 618 et 627, AAAAMMJJ)".to_string())
        } else {
            None
        };
        if let Some(d) = defaut {
            f.sorte = Sorte::Illisible;
            f.raison = Some(format!("commence comme une AEM, mais l'enregistrement {} {}", i + 1, d));
            return;
        }
    }
    f.sorte = Sorte::Aem;
    f.enregistrements = Some(lignes.len());
}
