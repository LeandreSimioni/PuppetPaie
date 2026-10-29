//! `moteur verifier-avant-commit [--indexes]` : le garde avant commit. Il cherche, dans ce que Git emporterait du dépôt, les
//! données personnelles : trois motifs génériques (un NIR, un IBAN, un BIC), puis, si `prive/config/` est là, les VRAIES
//! valeurs de la compagnie du poste qu'il y lit (SIRET, SIREN, comptes Urssaf, Congés Spectacles et France Travail, IBAN,
//! BIC, et pour chaque salarié son NIR, son numéro Congés Spectacles, son courriel, son adresse, son nom et son prénom).
//! Porté de `atelier/outils/verifier_avant_commit.py` (lot B, 15/10/2026), qu'il remplace.
//!
//! Le verrou de la suite (`tests/fichiers/donnees_reelles.rs`) tient déjà, à chaque passage de tests, le nom et le SIREN
//! de la compagnie du poste, les chemins du poste et les NIR sans la marque d'invention. Ce garde-ci fait ce que le verrou
//! ne peut pas faire sans porter lui-même ce qu'il cherche : il LIT les vraies valeurs dans `prive/config/` (jamais
//! `prive/config/*.bin`) et les cherche dans ce qui part. **Il ne contient aucune donnée réelle** : un garde-fou qui
//! porterait la liste des NIR à protéger serait exactement ce qu'il prétend empêcher. Il ne tourne pas dans la suite (un
//! test ne lit jamais `prive/`) ; il tourne avant chaque commit, en crochet git (`atelier/.githooks/pre-commit`, posé par
//! `git config core.hooksPath atelier/.githooks`), et à la demande.
//!
//! Ce qu'il ne faut surtout pas en faire : nettoyer un test parce qu'il le signale. Une trouvaille se regarde : une valeur
//! réelle sort du dépôt public, une valeur inventée s'inscrit dans `FICTIVES` (ou porte la marque d'invention d'un NIR,
//! `INVENTE`, la convention du verrou).
//!
//! Il ne dit jamais la valeur trouvée : le fichier, la ligne, et ce que c'est (le NIR d'un salarié, l'IBAN de la
//! compagnie…), rien d'autre ; la sortie d'un crochet se lit partout, dans un terminal comme dans la séance d'un agent.
//!
//! Sortie : 0 si rien n'a été trouvé, 1 sinon, avec la liste fichier par fichier ; 2 s'il n'a pas pu vérifier (hors d'un
//! dépôt Git, ou un fichier de `prive/config/` illisible : un garde qui ne lit pas ce qu'il protège ne passe pas).

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use regex::Regex;
use serde_json::Value;

type R<T> = Result<T, String>;

/// Ce qui ne part jamais : les données de la compagnie du poste.
const PRIVE: &str = "prive/";

/// Les fichiers écrits en texte. Les formats de déclaration en sont (`.dsn`, les gabarits `.txt`, les retours `.xml`) :
/// ceux des compagnies fictives portent des NIR, inventés, qu'il faut voir passer.
const EXTENSIONS: [&str; 20] = [
    "py", "md", "html", "css", "js", "json", "txt", "cmd", "pyw", "modele", "yml", "yaml", "toml", "rs", "dsn", "xml", "csv",
    "sh", "command", "log",
];

/// La marque d'un NIR inventé, la même que le verrou de la suite (`tests/fichiers/donnees_reelles.rs`, qui la lit ici) :
/// une commune ou un rang que personne n'a, dans les treize premiers chiffres.
pub const INVENTE: [&str; 3] = ["999", "123456", "000"];

/// Les valeurs FABRIQUÉES qui remplacent les vraies dans les tests et les exemples. Elles ne désignent personne, mais elles
/// ont la forme du vrai -- c'est même leur intérêt : leurs clés se vérifient, donc un test ne peut pas passer sur un numéro
/// impossible. Sans cette liste, le motif générique les signalerait à chaque fois et le garde-fou finirait ignoré.
///
/// Y ajouter une valeur est un acte visible, relu comme le reste du code. N'y mettre QUE des valeurs inventées : cette
/// liste est publiée avec le dépôt.
pub const FICTIVES: [(&str, &str); 8] = [
    ("2930899999999", "NIR féminin fabriqué (clé 47)"),
    ("1850375999999", "NIR masculin fabriqué (clé 87)"),
    // Les deux ci-dessus finissent par six « 9 », ce que la norme n'admet pas (CCH-16) : ils servent d'exemples dans des
    // comptes rendus, jamais dans une fiche. Celui-ci est fabriqué ET déclarable -- c'est celui qu'il faut quand un test
    // construit une fiche de salarié.
    ("1850375999001", "NIR masculin fabriqué et déclarable (clé 18)"),
    ("1234567890123", "NIR de forme invalide, pour les tests de rejet"),
    ("12345678900017", "SIRET fabriqué (clé de Luhn juste)"),
    // Les compagnies fictives ont besoin d'un compte : la DSN porte un ordre de télépaiement, et un IBAN de forme fausse
    // ferait refuser le fichier par la plate-forme avant qu'on ait rien éprouvé. Celui-ci est fabriqué, sa clé de contrôle
    // (85) est juste, et il ne désigne aucun compte.
    ("FR7630001007941234567890185", "IBAN fabriqué (clé 85 juste)"),
    // (14/10/2026) Les deux suivants se disent inventés dans leur fichier : la vie de démonstration
    // `atelier/outils/demos/les-faisceaux/gestes.jsonl`, que reprend le scenario du banc `tests/donnees/scenarios/treteaux-du-banc/` (« Aucune valeur ne désigne une entreprise ou une personne réelle »), et
    // `tests/fichiers/caviardage.rs` (« les IBAN sont faits de zéros »).
    ("FR7630001007943252044000039", "IBAN de la démonstration Les Faisceaux"),
    ("FR7630001000000000000000185", "IBAN fait de zéros, test du caviardage"),
];

/// Les textes de référence (`atelier/reference/`, 15/10/2026) sont des copies, octet pour octet, de ce que d'autres ont
/// publié (leur empreinte se vérifie, `moteur sources`) : un numéro qui y a la forme d'un NIR, d'un IBAN ou d'un BIC est
/// celui de l'émetteur -- l'exemple d'une documentation officielle, ou un nombre qui n'en est pas un. Chacun est nommé
/// ici, fichier par fichier (relatif à la racine du dépôt), avec ce qu'il est ; seuls les motifs GÉNÉRIQUES s'y taisent,
/// jamais les vraies valeurs lues dans `prive/config/`.
///
/// Le verrou de la suite tient sa propre liste (`donnees_reelles.rs`, `EXEMPLES_DES_TEXTES`) : elle excuse ce que SON
/// détecteur de NIR trouve, qui n'est pas ce que trouvent les trois motifs d'ici (il ne lit pas un NIR dans une adresse
/// web, mais il en lit un à mois 20-99) ; une liste commune excuserait chez l'un ce que seul l'autre trouve.
pub const EXEMPLES_DES_TEXTES: [(&str, &str); 9] = [
    (
        "atelier/reference/sources/gip-mds-api-dsn/ctip-dsn-crm-oc-phases-2-3-20160601.txt",
        "les salariés et les comptes d'exemple des comptes rendus métier OC imprimés par le CTIP (présentation de juin 2016)",
    ),
    (
        "atelier/reference/sources/agirc-arrco/agirc-arrco-aide-codification-dsn-ct2025-v1.3-20250301.txt",
        "l'IBAN d'exemple (FR99 1234...) de l'aide à la codification Agirc-Arrco",
    ),
    (
        "atelier/reference/sources/agirc-arrco/agirc-arrco-aide-codification-dsn-ct2026-v1.0-20251201.txt",
        "l'IBAN d'exemple (FR99 1234...) de l'aide à la codification Agirc-Arrco",
    ),
    (
        "atelier/reference/sources/bofip/BOI-RSA-BASE-30-10-20-20260810.html",
        "un identifiant de requête EUR-Lex (qid) dans une adresse de la page du BOFiP",
    ),
    (
        "atelier/reference/sources/gip-mds-api-dsn/gip-mds-guide-implementation-api-dsn-v6.8-20260706.txt",
        "les exemples de flux et de réponses du guide de l'API DSN (GIP-MDS)",
    ),
    (
        "atelier/reference/sources/net-entreprises-base-connaissances/net-entreprises-fiche2956-bloc-41-changement-modalites-contrat-20250429.html",
        "le salarié d'exemple de la fiche net-entreprises 2956",
    ),
    (
        "atelier/reference/sources/net-entreprises-base-connaissances/net-entreprises-fiche2956-bloc-41-changement-modalites-contrat-20250429.txt",
        "le salarié d'exemple de la fiche net-entreprises 2956",
    ),
    (
        "atelier/reference/sources/insee-nir-siret/insee-schema-nir-20261028.html",
        "les NIR d'exemple du schéma des identifiants d'individus de l'INSEE",
    ),
    (
        "atelier/reference/sources/insee-nir-siret/insee-schema-nir-20261028.txt",
        "les NIR d'exemple du schéma des identifiants d'individus de l'INSEE",
    ),
];

/// Les motifs génériques (une forme), par opposition aux valeurs réelles lues.
const GENERIQUES: [&str; 3] = ["NIR", "IBAN", "BIC"];

/// Ce que la compagnie déclare d'elle-même, dans `prive/config/entreprise.json`.
const CLES_ENTREPRISE: [&str; 5] = ["siret", "siren", "urssaf_compte", "congesspectacles_adherent", "affiliation_france_travail"];
/// Ce qui désigne un salarié, dans chaque fiche de `prive/config/salaries.json`.
const CLES_SALARIE: [&str; 4] = ["nir", "num_conges_spectacles", "email", "adresse"];

/// Une valeur trouvée : où, et ce que c'est. Jamais la valeur elle-même.
#[derive(Debug, Clone, PartialEq)]
pub struct Trouvaille {
    /// Relatif à la racine du dépôt, en `/`.
    pub fichier: String,
    /// À partir de 1, les lignes comptées comme `str.splitlines` de Python les comptait.
    pub ligne: usize,
    pub quoi: String,
}

/// Un motif et ce qu'il désigne : une forme (une expression, et ce qu'une ligne doit au moins porter pour qu'elle y
/// trouve quelque chose -- la `regex` du moteur est compilée sans ses optimisations, qui pèsent sur le binaire, et ce
/// filtre lui épargne les lignes où elle ne peut rien trouver), ou une valeur lue, cherchée sans tenir compte de la casse.
enum Motif {
    Forme { re: Regex, quoi: &'static str, filtre: fn(&str) -> bool },
    Valeur { minuscules: String, quoi: String },
}

impl Motif {
    fn quoi(&self) -> &str {
        match self {
            Motif::Forme { quoi, .. } => quoi,
            Motif::Valeur { quoi, .. } => quoi,
        }
    }

    /// Une valeur qui n'est pas inventée, dans `ligne` (`minuscules` : la même, en minuscules).
    fn trouve(&self, ligne: &str, minuscules: &str) -> bool {
        match self {
            Motif::Forme { re, quoi, filtre } => filtre(ligne) && re.find_iter(ligne).any(|t| !invente(t.as_str(), quoi)),
            Motif::Valeur { minuscules: v, .. } => minuscules.contains(v.as_str()),
        }
    }
}

/// Treize chiffres ASCII de suite : ce que tout NIR du motif générique porte.
fn treize_chiffres(ligne: &str) -> bool {
    let mut suite = 0;
    for b in ligne.bytes() {
        suite = if b.is_ascii_digit() { suite + 1 } else { 0 };
        if suite == 13 {
            return true;
        }
    }
    false
}

/// « FR » : ce que tout IBAN et tout BIC du motif générique porte.
fn porte_fr(ligne: &str) -> bool {
    ligne.contains("FR")
}

/// Les trois motifs génériques. La casse compte : un BIC et un IBAN s'écrivent en capitales, et chercher « CHIFFRE » sans
/// tenir compte de la casse trouve le mot « chiffres » dans une phrase française (première version du script Python, 56
/// fausses alertes).
fn generiques() -> Vec<Motif> {
    [
        // Un NIR : sexe, année, MOIS 01-12, puis 8 chiffres ; treize chiffres, ou quinze avec la clé. Le mois est restreint
        // aux valeurs courantes : sans cela, un horodatage « 20260906080000 » se lit comme un NIR. On y perd les
        // immatriculations à mois 20-42, rares : ce motif n'est qu'un filet, les vraies valeurs à protéger sont lues dans la
        // configuration (et le verrou de la suite, lui, les voit).
        (r"\b[12][0-9]{2}(?:0[1-9]|1[0-2])[0-9]{8}(?:[0-9]{2})?\b", "NIR", treize_chiffres as fn(&str) -> bool),
        (r"\bFR\d{2} ?(?:\d{4} ?){5}\d{3}\b", "IBAN", porte_fr),
        // Un BIC est en capitales et porte au moins un chiffre chez nous.
        (r"\b[A-Z]{4}FR[A-Z0-9]{2}[0-9][A-Z0-9]{0,2}\b", "BIC", porte_fr),
    ]
    .into_iter()
    .map(|(m, quoi, filtre)| Motif::Forme { re: Regex::new(m).unwrap(), quoi, filtre })
    .collect()
}

/// Une valeur JSON comme Python l'écrirait par `str()` (un nombre en chiffres, une chaîne telle quelle), si elle est
/// « vraie » au sens de Python (ni nulle, ni vide, ni zéro, ni `false`).
fn valeur_py(v: &Value) -> Option<String> {
    match v {
        Value::Null | Value::Bool(false) => None,
        Value::Bool(true) => Some("True".into()),
        Value::String(s) if s.is_empty() => None,
        Value::String(s) => Some(s.clone()),
        Value::Number(n) if n.as_f64() == Some(0.0) => None,
        Value::Number(n) => Some(n.to_string()),
        Value::Array(a) if a.is_empty() => None,
        Value::Object(o) if o.is_empty() => None,
        autre => Some(autre.to_string()),
    }
}

/// Un fichier de `prive/config/` : `None` s'il n'y est pas ; une erreur s'il y est sans se lire.
fn lire_config(cfg: &Path, nom: &str) -> R<Option<Value>> {
    let p = cfg.join(nom);
    if !p.exists() {
        return Ok(None);
    }
    let t = std::fs::read_to_string(&p).map_err(|e| format!("prive/config/{} illisible ({})", nom, e))?;
    serde_json::from_str(&t).map(Some).map_err(|e| format!("prive/config/{} illisible ({})", nom, e))
}

fn objet<'a>(v: &'a Value, quoi: &str) -> R<&'a serde_json::Map<String, Value>> {
    v.as_object().ok_or_else(|| format!("{} n'est pas un objet JSON", quoi))
}

/// Les valeurs à ne jamais publier, lues hors du dépôt (`<racine>/prive/config/`), dans l'ordre où on les lit, une fois
/// chacune (une valeur vue deux fois garde sa place et prend le dernier nom, comme le dictionnaire du Python). Sous six
/// caractères, trop de fausses alertes : elles ne se cherchent pas. Sans `prive/config/` -- un clone frais, le nuage --,
/// rien : les motifs génériques suffisent à attraper un NIR ou un IBAN quelconque.
///
/// Écart voulu avec le Python : il retombait en silence sur les seuls motifs génériques quand `entreprise.json` ou
/// `salaries.json` manquait ou ne se lisait pas. Ici, un fichier absent ne se lit pas et l'autre se lit quand même ; un
/// fichier présent mais illisible arrête tout (code 2) : un garde qui ne lit pas ce qu'il protège ne laisse pas passer.
/// Et le nom d'un salarié n'est jamais dans ce que la commande affiche (`salarie.nir`, pas la clé de sa fiche).
fn valeurs_reelles(racine: &Path) -> R<Vec<(String, String)>> {
    let cfg = racine.join("prive").join("config");
    let mut valeurs: Vec<(String, String)> = Vec::new();
    let mut poser = |v: String, quoi: String| match valeurs.iter_mut().find(|(x, _)| *x == v) {
        Some(e) => e.1 = quoi,
        None => valeurs.push((v, quoi)),
    };
    if !cfg.is_dir() {
        return Ok(Vec::new());
    }
    if let Some(ent) = lire_config(&cfg, "entreprise.json")? {
        let ent = objet(&ent, "prive/config/entreprise.json")?;
        // Toute valeur qu'une fiche datee a portee, a quelque date que ce soit (`contexte::toutes_les_valeurs`).
        for cle in CLES_ENTREPRISE {
            for v in droit::contexte::toutes_les_valeurs(ent.get(cle)).into_iter().filter_map(valeur_py) {
                poser(v, cle.to_string());
            }
        }
        for banque in droit::contexte::toutes_les_valeurs(ent.get("banque")).into_iter().filter(|b| valeur_py(b).is_some()) {
            let banque = objet(banque, "prive/config/entreprise.json : banque")?;
            for cle in ["iban", "bic"] {
                if let Some(v) = banque.get(cle).and_then(valeur_py) {
                    poser(v, format!("banque.{}", cle));
                }
            }
        }
    }
    if let Some(sal) = lire_config(&cfg, "salaries.json")? {
        for (k, s) in objet(&sal, "prive/config/salaries.json")? {
            if k.starts_with('_') {
                continue;
            }
            let s = objet(s, "une fiche de prive/config/salaries.json")?;
            for cle in CLES_SALARIE {
                for v in droit::contexte::toutes_les_valeurs(s.get(cle)).into_iter().filter_map(valeur_py) {
                    poser(v, format!("salarie.{}", cle));
                }
            }
            // Le nom seul est trop courant pour être un motif ; le nom ET le prénom collés le sont beaucoup moins.
            for nom in droit::contexte::toutes_les_valeurs(s.get("nom")).into_iter().filter_map(valeur_py) {
                for prenom in droit::contexte::toutes_les_valeurs(s.get("prenom")).into_iter().filter_map(valeur_py) {
                    poser(format!("{} {}", nom, prenom), "salarie (identite)".to_string());
                }
            }
        }
    }
    Ok(valeurs.into_iter().filter(|(v, _)| v.chars().count() >= 6).collect())
}

/// Les motifs : les génériques, puis les vraies valeurs, sans tenir compte de la casse (les deux côtés en minuscules,
/// comme `re.IGNORECASE` du Python les comparait). Une valeur lue qui est elle-même dans `FICTIVES` ne se cherche pas :
/// c'est une compagnie fictive copiée dans `prive/`, pas une donnée réelle.
fn motifs(racine: &Path) -> R<Vec<Motif>> {
    let mut m = generiques();
    for (v, quoi) in valeurs_reelles(racine)? {
        let sans_espaces = v.replace(' ', "");
        if !FICTIVES.iter().any(|(f, _)| f.eq_ignore_ascii_case(&sans_espaces)) {
            m.push(Motif::Valeur { minuscules: v.to_lowercase(), quoi });
        }
    }
    Ok(m)
}

fn git(racine: &Path, args: &[&str]) -> R<Vec<u8>> {
    let sortie = Command::new("git")
        .arg("-C")
        .arg(racine)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("git introuvable ({}) : ce garde lit ce que Git emporterait", e))?;
    if !sortie.status.success() {
        return Err(format!("git {} : {}", args.join(" "), String::from_utf8_lossy(&sortie.stderr).trim()));
    }
    Ok(sortie.stdout)
}

/// La racine du dépôt Git où l'on est (`git rev-parse --show-toplevel`) : c'est là que se trouve `prive/`.
pub fn racine_du_depot(depuis: &Path) -> R<PathBuf> {
    let o = git(depuis, &["rev-parse", "--show-toplevel"])?;
    let t = String::from_utf8(o).map_err(|_| "git rev-parse : chemin illisible".to_string())?;
    Ok(PathBuf::from(t.trim_end_matches(['\n', '\r'])))
}

fn extension_texte(rel: &str) -> bool {
    Path::new(rel).extension().and_then(|x| x.to_str()).is_some_and(|x| EXTENSIONS.contains(&x.to_lowercase().as_str()))
}

/// Ce qui part, relatif à la racine : ce que Git commiterait (`indexes`), ou ce qu'il emporterait du dépôt entier (suivis
/// et non suivis, les `.gitignore` appliqués par Git lui-même) -- hors de `prive/` dans les deux cas, en texte seulement.
fn fichiers(racine: &Path, indexes: bool) -> R<Vec<String>> {
    let args: &[&str] = if indexes {
        &["diff", "--cached", "--name-only", "-z", "--diff-filter=ACM"]
    } else {
        &["ls-files", "-z", "--cached", "--others", "--exclude-standard"]
    };
    let mut noms: Vec<String> = git(racine, args)?
        .split(|&b| b == 0)
        .filter(|n| !n.is_empty())
        .map(|n| String::from_utf8_lossy(n).into_owned())
        .filter(|n| !n.starts_with(PRIVE) && extension_texte(n))
        .collect();
    noms.sort();
    noms.dedup();
    if !indexes {
        noms.retain(|n| racine.join(n).is_file());
    }
    Ok(noms)
}

/// Ce que l'index porte pour chaque fichier (`git cat-file --batch`, `:0:<chemin>`) : c'est cela, et non la copie de
/// travail, que le commit emportera. Écart voulu avec le Python, qui relisait la copie de travail : un fichier indexé
/// puis retouché passait sur sa version retouchée.
fn contenus_indexes(racine: &Path, noms: &[String]) -> R<Vec<Option<Vec<u8>>>> {
    if noms.is_empty() {
        return Ok(Vec::new());
    }
    let mut p = Command::new("git")
        .arg("-C")
        .arg(racine)
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("git introuvable ({})", e))?;
    let mut entree = p.stdin.take().unwrap();
    let demande: String = noms.iter().map(|n| format!(":0:{}\n", n)).collect();
    let ecrivain = std::thread::spawn(move || entree.write_all(demande.as_bytes()));
    let mut o = Vec::new();
    p.stdout.take().unwrap().read_to_end(&mut o).map_err(|e| e.to_string())?;
    let fin = p.wait_with_output().map_err(|e| e.to_string())?;
    ecrivain.join().map_err(|_| "git cat-file : écriture interrompue".to_string())?.map_err(|e| e.to_string())?;
    if !fin.status.success() {
        return Err(format!("git cat-file : {}", String::from_utf8_lossy(&fin.stderr).trim()));
    }
    let mut contenus = Vec::new();
    let mut i = 0;
    for _ in noms {
        let nl = o[i..].iter().position(|&b| b == b'\n').ok_or("git cat-file : réponse tronquée")? + i;
        let entete = String::from_utf8_lossy(&o[i..nl]).into_owned();
        i = nl + 1;
        let champs: Vec<&str> = entete.split(' ').collect();
        if champs.len() == 3 && champs[1] == "blob" {
            let n: usize = champs[2].parse().map_err(|_| format!("git cat-file : en-tête illisible ({})", entete))?;
            contenus.push(Some(o.get(i..i + n).ok_or("git cat-file : réponse tronquée")?.to_vec()));
            i += n + 1;
        } else {
            // `missing` : un sous-module, ou un chemin que l'index ne porte plus.
            contenus.push(None);
        }
    }
    Ok(contenus)
}

/// Les octets lus en UTF-8, ce qui n'en est pas sauté (le `errors="ignore"` du Python).
fn utf8_sans_erreurs(mut o: &[u8]) -> String {
    let mut s = String::with_capacity(o.len());
    loop {
        match std::str::from_utf8(o) {
            Ok(t) => {
                s.push_str(t);
                return s;
            }
            Err(e) => {
                let (bon, reste) = o.split_at(e.valid_up_to());
                s.push_str(std::str::from_utf8(bon).unwrap());
                match e.error_len() {
                    Some(n) => o = &reste[n..],
                    None => return s,
                }
            }
        }
    }
}

/// Une valeur fabriquée : listée dans `FICTIVES`, ou un NIR (générique) qui porte la marque d'invention dans ses treize
/// premiers chiffres. Un IBAN se compare sans ses espaces (il s'écrit par groupes de quatre).
fn invente(trouve: &str, quoi: &str) -> bool {
    let sans_espaces = trouve.replace(' ', "");
    if FICTIVES.iter().any(|(f, _)| *f == sans_espaces) {
        return true;
    }
    quoi == "NIR" && {
        let treize: String = trouve.chars().take(13).collect();
        INVENTE.iter().any(|m| treize.contains(m))
    }
}

/// Ce que `motifs` trouvent dans un texte : au plus une trouvaille par ligne, celle du premier motif qui y trouve une
/// valeur qui n'est pas inventée.
fn chercher(fichier: &str, o: &[u8], motifs: &[Motif], trouvailles: &mut Vec<Trouvaille>) {
    let exemple = EXEMPLES_DES_TEXTES.iter().any(|(f, _)| *f == fichier);
    let siens: Vec<&Motif> = motifs.iter().filter(|m| !(exemple && GENERIQUES.contains(&m.quoi()))).collect();
    let texte = utf8_sans_erreurs(o);
    // Les séparateurs de lignes n'ont pas de casse : les deux textes ont les mêmes lignes. Sans valeur lue (un clone
    // frais, le nuage), les minuscules ne servent à rien.
    let valeurs = siens.iter().any(|m| matches!(m, Motif::Valeur { .. }));
    let minuscules = if valeurs { texte.to_lowercase() } else { String::new() };
    let lignes_min = if valeurs { socle::texte::lignes_py(&minuscules) } else { Vec::new() };
    let lignes_min = lignes_min.into_iter().chain(std::iter::repeat(""));
    for (n, (ligne, min)) in socle::texte::lignes_py(&texte).into_iter().zip(lignes_min).enumerate() {
        if let Some(m) = siens.iter().find(|m| m.trouve(ligne, min)) {
            trouvailles.push(Trouvaille { fichier: fichier.to_string(), ligne: n + 1, quoi: m.quoi().to_string() });
        }
    }
}

/// Le garde, sur le dépôt de `racine` : ce qui partirait (`indexes` : ce que l'index porte), et les valeurs de
/// `<racine>/prive/config/`.
pub fn verifier(racine: &Path, indexes: bool) -> R<Vec<Trouvaille>> {
    let motifs = motifs(racine)?;
    let noms = fichiers(racine, indexes)?;
    let contenus: Vec<Option<Vec<u8>>> =
        if indexes { contenus_indexes(racine, &noms)? } else { noms.iter().map(|n| std::fs::read(racine.join(n)).ok()).collect() };
    let mut trouvailles = Vec::new();
    for (nom, o) in noms.iter().zip(contenus) {
        if let Some(o) = o {
            chercher(nom, &o, &motifs, &mut trouvailles);
        }
    }
    Ok(trouvailles)
}

/// Ce que la commande affiche : la phrase de passage, ou chaque fichier et ses lignes, et ce qu'il faut en faire.
pub fn rapport(trouvailles: &[Trouvaille]) -> String {
    if trouvailles.is_empty() {
        return "Rien de personnel dans le code. C'est publiable.\n".to_string();
    }
    let mut s = format!("ARRET - {} occurrence(s) de donnees personnelles :\n\n", trouvailles.len());
    let mut par_fichier: Vec<&Trouvaille> = trouvailles.iter().collect();
    par_fichier.sort_by(|a, b| a.fichier.cmp(&b.fichier));
    let mut courant: Option<&str> = None;
    for t in par_fichier {
        if courant != Some(t.fichier.as_str()) {
            if courant.is_some() {
                s.push('\n');
            }
            s.push_str(&format!("  {}\n", t.fichier));
            courant = Some(&t.fichier);
        }
        s.push_str(&format!("    ligne {:<5} {}\n", t.ligne, t.quoi));
    }
    s.push('\n');
    s.push_str(
        "Ces valeurs identifient des personnes reelles. L'historique d'un\n\
         depot est definitif : une fois pousse, efface ne veut plus rien dire.\n\
         Retirer les identites, garder les chiffres -- ce sont eux qui prouvent\n\
         que le moteur est cale. Voir AVANT-LE-PREMIER-COMMIT.md.\n",
    );
    s
}

const USAGE: &str = "usage : moteur verifier-avant-commit [--indexes]\n\
\x20 cherche les donnees personnelles dans ce que git emporterait (hors de prive/) :\n\
\x20 NIR, IBAN, BIC, et les vraies valeurs de prive/config/ s'il est la.\n\
\x20 --indexes   ce que l'index porte (le crochet atelier/.githooks/pre-commit)\n\
\x20 sortie : 0 rien trouve, 1 trouve, 2 pas pu verifier\n";

/// `moteur verifier-avant-commit [--indexes]`, depuis n'importe où dans le dépôt.
pub fn commande(args: &[String]) -> i32 {
    let mut indexes = false;
    for a in args {
        match a.as_str() {
            "--indexes" => indexes = true,
            "--aide" | "--help" | "-h" => {
                print!("{}", USAGE);
                return 0;
            }
            _ => {
                eprint!("{}", USAGE);
                return 2;
            }
        }
    }
    let r = std::env::current_dir().map_err(|e| e.to_string()).and_then(|d| racine_du_depot(&d)).and_then(|r| verifier(&r, indexes));
    match r {
        Ok(t) => {
            print!("{}", rapport(&t));
            i32::from(!t.is_empty())
        }
        Err(m) => {
            eprintln!("verifier-avant-commit : {}", m);
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_octet_hors_utf8_se_saute() {
        assert_eq!(utf8_sans_erreurs(b"a\xffb\xe9"), "ab");
    }

    #[test]
    fn une_valeur_inventee_passe() {
        assert!(invente("FR76 3000 1007 9412 3456 7890 185", "IBAN"));
        assert!(invente("1850675999456", "NIR"));
        assert!(!invente("1850675999456", "salarie.nir"));
    }
}
