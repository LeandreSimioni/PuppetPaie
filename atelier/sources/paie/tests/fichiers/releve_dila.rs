//! Les textes de la DILA tenus en local : `paie releve-codes` (LEGI) et `paie releve-conventions` (KALI), portes du Python
//! le 15/10/2026 (`releve_codes.rs`, `releve_conventions.rs`, `entrepot_dila.rs`).
//!
//! LE CONTRAT : les fichiers qu'ecrivait le Python, a l'octet -- `releve.json`, `xml/`, `articles-<date>.md`,
//! `changements-de-version-<date>.md`, `etat.json`, le contenu de `articles.jsonl.gz` --, sauf le nom de la commande dans
//! les documents. Les oracles de `tests/donnees/releve_dila/` sont ce que les scripts Python ont ecrit le 15/10/2026 sur
//! des extraits figes des entrepots du depot (`atelier/reference/sources/legi-entrepot/`, `kali-entrepot/`), avant d'etre
//! retires ; les livraisons `*-increment/` sont fabriquees, et ce que le Python en a fait est leur oracle. Le reseau est
//! remplace par des octets poses d'avance (`Faux`) : aucun test ne sort du conteneur.

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use socle::date::Jour;
use paie::entrepot_dila::Source;
use serde_json::Value;

use crate::outils;

const LEGI: &str = "https://echanges.dila.gouv.fr/OPENDATA/LEGI/";
const KALI: &str = "https://echanges.dila.gouv.fr/OPENDATA/KALI/";

fn donnees() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/donnees/releve_dila")
}

fn jour(iso: &str) -> Jour {
    Jour::lire(iso).unwrap()
}

/// Le site de la DILA, en octets poses d'avance ; une adresse en panne repond une erreur ; chaque appel est note.
struct Faux {
    pages: HashMap<String, Vec<u8>>,
    pannes: HashSet<String>,
    appels: Mutex<Vec<String>>,
}

impl Faux {
    fn nouveau() -> Faux {
        Faux { pages: HashMap::new(), pannes: HashSet::new(), appels: Mutex::new(Vec::new()) }
    }
    fn page(mut self, url: &str, octets: Vec<u8>) -> Faux {
        self.pages.insert(url.to_string(), octets);
        self
    }
    /// Un index de la DILA qui liste ces archives.
    fn index(self, url: &str, noms: &[String]) -> Faux {
        let html: String = noms.iter().map(|n| format!("<a href=\"{}\">{}</a>\n", n, n)).collect();
        self.page(url, html.into_bytes())
    }
}

impl Source for Faux {
    fn lire(&self, url: &str, _delai: u64) -> Result<Vec<u8>, String> {
        self.appels.lock().unwrap().push(url.to_string());
        if self.pannes.contains(url) {
            return Err(format!("{} : coupure", url));
        }
        self.pages.get(url).cloned().ok_or_else(|| format!("{} : 404", url))
    }
    fn flux(&self, url: &str) -> Result<Box<dyn Read + Send>, String> {
        Ok(Box::new(std::io::Cursor::new(self.lire(url, 0)?)))
    }
    fn pause(&self, _secondes: u64) {}
}

fn lire(p: &Path) -> Vec<u8> {
    std::fs::read(p).unwrap_or_else(|e| panic!("{} : {}", p.display(), e))
}

fn decompresse(p: &Path) -> Vec<u8> {
    let mut v = Vec::new();
    flate2::read::MultiGzDecoder::new(&lire(p)[..]).read_to_end(&mut v).unwrap();
    v
}

fn json(p: &Path) -> Value {
    serde_json::from_slice(&lire(p)).unwrap()
}

/// Deux dossiers aux memes fichiers, aux memes octets.
fn memes_dossiers(a: &Path, b: &Path) {
    let noms = |d: &Path| -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(d).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        v.sort();
        v
    };
    assert_eq!(noms(a), noms(b), "{} / {}", a.display(), b.display());
    for n in noms(a) {
        assert!(lire(&a.join(&n)) == lire(&b.join(&n)), "{} differe de {}", a.join(&n).display(), b.join(&n).display());
    }
}

const TEXTES: [(&str, &str); 2] = [("impots", "code-general-des-impots"), ("artistes-1975", "arrete-1975-artistes-du-spectacle")];

/// Les livraisons deja appliquees a l'extrait de `legi-entrepot/`, sous leur nom d'archive.
fn lues(etat: &Path, prefixe: &str) -> Vec<String> {
    json(etat)["lues"].as_array().unwrap().iter().map(|l| format!("{}_{}.tar.gz", prefixe, l.as_str().unwrap())).collect()
}

// ------------------------------------------------------------------ LEGI

/// `rapport` ecrit le document lisible de chaque texte comme le moissonneur Python (le nom de la commande excepte).
#[test]
fn rapport_ecrit_le_document_de_chaque_texte() {
    let d = outils::dossier_de_test("releve_dila", "rapport");
    outils::copier(&donnees().join("legi"), &d);
    for (cle, dossier) in TEXTES {
        paie::releve_codes::rapport(&d, cle, &jour("2026-10-15")).unwrap();
        let ecrit = lire(&d.join(dossier).join("articles-20261015.md"));
        let attendu = lire(&donnees().join(format!("legi/attendus/articles-{}-20261015.md", cle)));
        assert!(ecrit == attendu, "{} : le rapport differe de celui du Python", cle);
    }
}

/// Sans livraison neuve, `moissonner` ne telecharge que l'index, et reecrit les releves et l'entrepot a l'identique.
#[test]
fn moissonner_sans_livraison_neuve_reecrit_tout_a_l_identique() {
    let d = outils::dossier_de_test("releve_dila", "moissonner_rien");
    outils::copier(&donnees().join("legi"), &d);
    let source = Faux::nouveau().index(LEGI, &lues(&d.join("legi-entrepot/etat.json"), "LEGI"));
    paie::releve_codes::moissonner(&d, false, &source, &jour("2026-10-13")).unwrap();
    assert_eq!(*source.appels.lock().unwrap(), [LEGI]);
    for (_, dossier) in TEXTES {
        assert!(lire(&d.join(dossier).join("releve.json")) == lire(&donnees().join("legi").join(dossier).join("releve.json")), "{}", dossier);
        memes_dossiers(&d.join(dossier).join("xml"), &donnees().join("legi").join(dossier).join("xml"));
    }
    let origine = donnees().join("legi/legi-entrepot");
    assert!(lire(&d.join("legi-entrepot/etat.json")) == lire(&origine.join("etat.json")));
    assert!(decompresse(&d.join("legi-entrepot/articles.jsonl.gz")) == decompresse(&origine.join("articles.jsonl.gz")));
}

/// Les livraisons neuves s'appliquent dans l'ordre : une version nouvelle remplace l'ancienne et se note (`changes`), une
/// version perimee ne remplace pas une version en vigueur, un article d'un texte non suivi est ignore, la liste de
/// suppression retire. Ce que le Python a fait des memes livraisons est l'oracle.
#[test]
fn moissonner_applique_les_livraisons_neuves_comme_le_python() {
    let d = outils::dossier_de_test("releve_dila", "moissonner_neuves");
    outils::copier(&donnees().join("legi"), &d);
    let inc = donnees().join("legi-increment");
    let mut noms = lues(&d.join("legi-entrepot/etat.json"), "LEGI");
    let mut source = Faux::nouveau();
    for n in ["LEGI_20261014-210000.tar.gz", "LEGI_20261015-210000.tar.gz"] {
        source = source.page(&format!("{}{}", LEGI, n), lire(&inc.join(n)));
        noms.push(n.to_string());
    }
    let source = source.index(LEGI, &noms);
    paie::releve_codes::moissonner(&d, false, &source, &jour("2026-10-15")).unwrap();
    for (_, dossier) in TEXTES {
        assert!(lire(&d.join(dossier).join("releve.json")) == lire(&inc.join("attendus").join(dossier).join("releve.json")), "{}", dossier);
        memes_dossiers(&d.join(dossier).join("xml"), &inc.join("attendus").join(dossier).join("xml"));
    }
    assert!(lire(&d.join("legi-entrepot/etat.json")) == lire(&inc.join("attendus/etat.json")));
    assert!(decompresse(&d.join("legi-entrepot/articles.jsonl.gz")) == decompresse(&inc.join("attendus/articles.jsonl.gz")));
}

/// UNE LIVRAISON MANQUEE ARRETE LA SUITE : celle du lendemain ne s'applique pas par-dessus un trou, et l'entrepot reste
/// ce qu'il etait ; la prochaine moisson reprendra la.
#[test]
fn une_livraison_manquee_arrete_la_suite() {
    let d = outils::dossier_de_test("releve_dila", "moissonner_trou");
    outils::copier(&donnees().join("legi"), &d);
    let inc = donnees().join("legi-increment");
    let mut noms = lues(&d.join("legi-entrepot/etat.json"), "LEGI");
    noms.extend(["LEGI_20261014-210000.tar.gz".to_string(), "LEGI_20261015-210000.tar.gz".to_string()]);
    let mut source = Faux::nouveau().index(LEGI, &noms).page(&format!("{}LEGI_20261015-210000.tar.gz", LEGI), lire(&inc.join("LEGI_20261015-210000.tar.gz")));
    source.pannes.insert(format!("{}LEGI_20261014-210000.tar.gz", LEGI));
    paie::releve_codes::moissonner(&d, false, &source, &jour("2026-10-15")).unwrap();
    let origine = donnees().join("legi/legi-entrepot");
    assert_eq!(json(&d.join("legi-entrepot/etat.json"))["lues"], json(&origine.join("etat.json"))["lues"]);
    assert!(decompresse(&d.join("legi-entrepot/articles.jsonl.gz")) == decompresse(&origine.join("articles.jsonl.gz")));
    // Les quatre essais de la livraison en panne, et rien d'autre ne s'est applique.
    let appels = source.appels.lock().unwrap();
    assert_eq!(appels.iter().filter(|u| u.ends_with("LEGI_20261014-210000.tar.gz")).count(), 4);
}

/// L'entrepot est reproductible a l'octet : deux enregistrements du meme contenu donnent le meme fichier, a l'en-tete gzip
/// du Python (`GzipFile(tmp, mtime=0)` : nom du fichier temporaire, date zero, niveau 9).
#[test]
fn l_entrepot_s_ecrit_a_l_octet_pres() {
    let d = outils::dossier_de_test("releve_dila", "entrepot");
    outils::copier(&donnees().join("legi/legi-entrepot"), &d.join("a"));
    let mut e = paie::entrepot_dila::Entrepot::nouveau(&d.join("a")).charger().unwrap();
    e.sauver("2026-10-13").unwrap();
    let premier = lire(&d.join("a/articles.jsonl.gz"));
    e.dossier = d.join("b");
    e.sauver("2026-10-13").unwrap();
    assert!(premier == lire(&d.join("b/articles.jsonl.gz")));
    let python = lire(&donnees().join("legi/legi-entrepot/articles.jsonl.gz"));
    assert_eq!(premier[..32], python[..32], "l'en-tete gzip");
    assert!(decompresse(&d.join("a/articles.jsonl.gz")) == decompresse(&donnees().join("legi/legi-entrepot/articles.jsonl.gz")));
    assert!(lire(&d.join("a/etat.json")) == lire(&donnees().join("legi/legi-entrepot/etat.json")));
}

/// `comparer` relit chaque article change contre sa version d'avant, cherchee PAR SON IDENTIFIANT dans l'archive globale :
/// texte identique, alineas qui different (le diff du Python, lignes en `--` comprises), version introuvable.
#[test]
fn comparer_rend_les_differences_comme_le_python() {
    let d = outils::dossier_de_test("releve_dila", "comparer");
    let c = donnees().join("comparer");
    outils::copier(&c.join("entree"), &d);
    let source = Faux::nouveau().page(&format!("{}{}", LEGI, paie::releve_codes::GLOBAL), lire(&c.join("global.tar.gz")));
    paie::releve_codes::comparer(&d, &source, &jour("2026-10-15")).unwrap();
    assert!(lire(&d.join("changements-de-version-20261015.md")) == lire(&c.join("attendus/changements-de-version-20261015.md")));
    memes_dossiers(&d.join("code-general-des-impots/xml-anterieur"), &c.join("attendus/xml-anterieur"));
}

/// Le diff unifie est celui de `difflib` (Ratcliff et Obershelp, `autojunk` au-dela de 200 lignes), sur un echantillon
/// a graine fixe tire par le Python.
#[test]
fn le_diff_est_celui_de_difflib() {
    let cas = json(&donnees().join("difflib-unifie.json"));
    let lignes = |v: &Value| -> Vec<String> { v.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect() };
    for (i, c) in cas.as_array().unwrap().iter().enumerate() {
        assert_eq!(paie::releve_codes::diff_unifie(&lignes(&c["a"]), &lignes(&c["b"]), 1), lignes(&c["diff"]), "cas {}", i);
    }
}

// ------------------------------------------------------------------ KALI

/// `chercher` et `article` rendent ce que rendait le Python.
#[test]
fn chercher_et_article_rendent_ce_que_rendait_le_python() {
    let ent = paie::releve_conventions::charger(&donnees().join("kali")).unwrap();
    let att = donnees().join("kali/attendus");
    let texte = |n: &str| String::from_utf8(lire(&att.join(n))).unwrap();
    assert_eq!(paie::releve_conventions::rendre_recherche(&ent, "3090", "licenciement"), texte("chercher-3090-licenciement.txt"));
    assert_eq!(paie::releve_conventions::rendre_recherche(&ent, "2642", "Préavis"), texte("chercher-2642-preavis.txt"));
    let nums = |l: &[&str]| -> Vec<String> { l.iter().map(|s| s.to_string()).collect() };
    assert_eq!(paie::releve_conventions::rendre_articles(&ent, "2642", &nums(&["3.1", "IX", "9"])), texte("article-2642-3.1-IX-9.txt"));
    assert_eq!(paie::releve_conventions::rendre_articles(&ent, "3090", &nums(&["7.6"])), texte("article-3090-7.6.txt"));
}

/// `mettre-a-jour` n'applique que les livraisons nouvelles : un article change d'etat ou de texte (note dans `changes`),
/// un article neuf, un article d'une convention non suivie ignore, la liste de suppression. Le Python est l'oracle.
#[test]
fn mettre_a_jour_applique_les_livraisons_neuves_comme_le_python() {
    let d = outils::dossier_de_test("releve_dila", "kali_neuves");
    outils::copier(&donnees().join("kali"), &d);
    let inc = donnees().join("kali-increment");
    let mut noms = lues(&d.join("kali-entrepot/etat.json"), "KALI");
    let mut source = Faux::nouveau();
    for n in ["KALI_20260925-210000.tar.gz", "KALI_20260926-210000.tar.gz"] {
        source = source.page(&format!("{}{}", KALI, n), lire(&inc.join(n)));
        noms.push(n.to_string());
    }
    let source = source.index(KALI, &noms);
    let manifeste = json(&inc.join("sources.json"));
    paie::releve_conventions::mettre_a_jour(&d, false, &manifeste, &source, &jour("2026-10-15")).unwrap();
    assert!(lire(&d.join("kali-entrepot/etat.json")) == lire(&inc.join("attendus/etat.json")));
    assert!(decompresse(&d.join("kali-entrepot/articles.jsonl.gz")) == decompresse(&inc.join("attendus/articles.jsonl.gz")));
}

/// Une convention que le manifeste cite et que l'entrepot ne tient pas fait relire la base entiere (la plus recente de
/// l'index), puis les seules livraisons depuis sa date.
#[test]
fn une_convention_nouvelle_fait_relire_la_base_entiere() {
    let d = outils::dossier_de_test("releve_dila", "kali_nouvelle");
    outils::copier(&donnees().join("kali"), &d);
    let inc = donnees().join("kali-increment");
    let global = "Freemium_kali_global_20260925-000000.tar.gz";
    let noms = [
        "Freemium_kali_global_20250713-140000.tar.gz".to_string(),
        global.to_string(),
        "KALI_20260924-214040.tar.gz".to_string(),
        "KALI_20260926-210000.tar.gz".to_string(),
    ];
    let source = Faux::nouveau()
        .index(KALI, &noms)
        .page(&format!("{}{}", KALI, global), lire(&inc.join("KALI_20260925-210000.tar.gz")))
        .page(&format!("{}KALI_20260926-210000.tar.gz", KALI), lire(&inc.join("KALI_20260926-210000.tar.gz")));
    let mut manifeste = json(&inc.join("sources.json"));
    // 9999 : une convention que l'entrepot ne tient pas ; 1285, tenue, dont la base entiere porte un article.
    manifeste["conventions_a_relever"]["9999"] = serde_json::json!({"conteneur": "KALICONT000099999999"});
    manifeste["conventions_a_relever"]["1285"] = serde_json::json!({"conteneur": "KALICONT000005635964"});
    let ent = paie::releve_conventions::mettre_a_jour(&d, false, &manifeste, &source, &jour("2026-10-15")).unwrap();
    assert_eq!(ent.etat["global"], global);
    assert_eq!(ent.etat["lues"], serde_json::json!(["20260926-210000"]));
    assert_eq!(ent.etat["conteneurs"]["KALICONT000099999999"], "9999");
    // La base entiere ne garde que ce qu'elle porte : la convention 1285 y est, l'ancien entrepot est oublie.
    let idcc: HashSet<String> = ent.articles.keys().map(|k| paie::entrepot_dila::parties(k)[0].to_string()).collect();
    assert!(idcc.contains("1285") && ent.articles.len() < 10, "{:?}", idcc);
}
