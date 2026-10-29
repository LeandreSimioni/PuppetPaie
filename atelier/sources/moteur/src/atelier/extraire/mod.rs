//! `moteur extraire naf|usages|formats|urssaf <sources...> <sortie.json> [--remplacer]` (et `transcrire`, la lecture d'un
//! cahier technique en PDF, une fois : `Transcription`) : les releves de l'atelier, qui
//! fabriquent des referentiels de `logiciel/donnees/` a partir des textes deposes dans `atelier/reference/`.
//!
//! UN OUTIL DE L'EDITEUR. Il ne tourne qu'a la main, quand l'organisme republie son texte (une norme DSN, la NAF, la Table
//! des Urssaf) ; le moteur, lui, ne fait que lire le JSON produit (`naf.rs`, `usages.rs`, `formats.rs`, `dpae.rs`). Il est
//! dans le `moteur` livre parce que c'est ce programme-la que le poste de l'editeur fait tourner, mais il n'apparait que dans
//! l'aide de bas niveau (`moteur` sans argument), avec les autres outils d'atelier.
//!
//! Chaque releve redonne, a l'octet, le referentiel livre, hors le `_source` (qui nomme la commande, le fichier depose et
//! la date du releve). La preuve : `tests/fichiers/extraction.rs`, qui rejoue chaque releve sur `atelier/reference/`.
//!
//! UN REFERENTIEL NE S'ECRASE PAS PAR ACCIDENT. La sortie se nomme toujours (pas de cible par defaut) ; elle finit en `.json` ; un fichier deja la n'est
//! remplace qu'avec `--remplacer` ; et rien ne s'ecrit avant que le releve soit entierement fait (fichier temporaire, puis
//! renommage).

pub mod formats;
pub mod naf;
pub mod urssaf;
pub mod usages;

use std::path::{Path, PathBuf};

use serde_json::Value;

use socle::date::Jour;
use socle::num::Resultat;

/// Un releve fait, pret a s'ecrire.
pub struct Releve {
    pub donnees: Value,
    /// L'indentation du JSON ecrit (n espaces par niveau).
    pub indent: usize,
    /// Un saut de ligne apres le JSON.
    pub saut_final: bool,
    /// Ce que la commande dit en finissant (les nombres, ce qui a ete ecarte).
    pub bilan: Vec<String>,
}

impl Releve {
    /// Les octets du fichier : le JSON indente, accents non echappes, en LF.
    pub fn octets(&self) -> Vec<u8> {
        let mut t = socle::ecriture::json_indente(&self.donnees, self.indent);
        if self.saut_final {
            t.push('\n');
        }
        t.into_bytes()
    }
}

const USAGE: &str = "usage : moteur extraire naf <structure.xlsx> <correspondances.xlsx> <sortie.json> [--remplacer]\n\
\x20       moteur extraire usages <dsn-cahier-technique-V.pdf | dsn-cahier-technique-V-usages.txt | dsn-tableau-des-usages-V-AAAAMMJJ.xlsx> <sortie.json> [--remplacer]\n\
\x20       moteur extraire formats <dsn-cahier-technique-V.pdf | dsn-cahier-technique-V-formats.txt | dsn-tableaux-categories-controles-datatypes-usages-V-AAAAMMJJ.xlsx> <sortie.json> [--remplacer]\n\
\x20       moteur extraire urssaf <urssaf-table-des-urssaf-AAAAMMJJ.csv> <urssaf-liste-des-sites-et-des-activites.csv> <insee-cog-2015-departements.zip> <sortie.json> [--remplacer]\n\
\x20       moteur extraire transcrire <dsn-cahier-technique-V.pdf> <usages.txt> <formats.txt> [--remplacer]\n\
La sortie se nomme toujours ; un fichier deja la ne se remplace qu'avec --remplacer.";

/// `moteur extraire ...` : rend le code de sortie (2 : la commande est mal formee, 1 : le releve refuse, 0 : ecrit).
pub fn commande(args: &[String]) -> i32 {
    if args.iter().any(|a| matches!(a.as_str(), "-h" | "--help" | "--aide")) {
        println!("{}", USAGE);
        return 0;
    }
    let remplacer = args.iter().any(|a| a == "--remplacer");
    if let Some(o) = args.iter().find(|a| a.starts_with("--") && a.as_str() != "--remplacer") {
        eprintln!("option inconnue : {}\n{}", o, USAGE);
        return 2;
    }
    let pos: Vec<&str> = args.iter().filter(|a| !a.starts_with("--")).map(String::as_str).collect();
    if pos.first() == Some(&"transcrire") {
        return transcrire(&pos[1..], remplacer);
    }
    let attendus = match pos.first().copied() {
        Some("naf") => 3,
        Some("usages") | Some("formats") => 2,
        Some("urssaf") => 4,
        _ => {
            eprintln!("{}", USAGE);
            return 2;
        }
    };
    if pos.len() != attendus + 1 {
        eprintln!(
            "moteur extraire {} attend {} chemins, la sortie en dernier : rien n'est ecrit.\n{}",
            pos[0], attendus, USAGE
        );
        return 2;
    }
    let sources: Vec<PathBuf> = pos[1..attendus].iter().map(PathBuf::from).collect();
    let sortie = PathBuf::from(pos[attendus]);
    if let Err(m) = verifier_la_sortie(&sources, &sortie, remplacer) {
        eprintln!("{}", m);
        return 1;
    }
    let jour = crate::commandes::compagnie::jour_de_paris();
    let releve = match pos[0] {
        "naf" => naf::relever(&sources[0], &sources[1], &jour),
        "usages" => usages::relever(&sources[0], &jour),
        "formats" => formats::relever(&sources[0], &jour),
        _ => urssaf::relever(&sources[0], &sources[1], &sources[2], &jour),
    };
    let releve = match releve {
        Ok(r) => r,
        Err(m) => {
            eprintln!("{}", m);
            return 1;
        }
    };
    if let Err(m) = ecrire(&releve, &sortie, remplacer) {
        eprintln!("{}", m);
        return 1;
    }
    for l in &releve.bilan {
        println!("{}", l);
    }
    println!("-> {}", sortie.display());
    0
}

/// La sortie avant tout releve : un `.json`, qui n'est aucune des sources, et qui n'existe pas encore (sauf `--remplacer`).
fn verifier_la_sortie(sources: &[PathBuf], sortie: &Path, remplacer: bool) -> Resultat<()> {
    if sortie.extension().and_then(|e| e.to_str()) != Some("json") {
        return Err(format!("La sortie doit etre un fichier .json (recu : {}) : rien n'est ecrit.", sortie.display()));
    }
    for s in sources {
        if !s.is_file() {
            return Err(format!(
                "{} absent : `moteur sources` dit d'ou chaque texte se releve. Rien n'est ecrit.",
                s.display()
            ));
        }
        if s == sortie || (sortie.exists() && std::fs::canonicalize(s).ok() == std::fs::canonicalize(sortie).ok()) {
            return Err(format!("La sortie {} est une des sources : rien n'est ecrit.", sortie.display()));
        }
    }
    if sortie.exists() && !remplacer {
        return Err(format!(
            "{} existe deja : un referentiel ne se remplace pas par accident (`--remplacer` pour l'ecraser). Rien n'est ecrit.",
            sortie.display()
        ));
    }
    Ok(())
}

/// Ecrit le releve : un fichier temporaire a cote, puis le renommage (la sortie est entiere ou intacte).
fn ecrire(releve: &Releve, sortie: &Path, remplacer: bool) -> Resultat<()> {
    if sortie.exists() && !remplacer {
        return Err(format!("{} existe deja (`--remplacer` pour l'ecraser) : rien n'est ecrit.", sortie.display()));
    }
    let mut nom = sortie.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    nom.push(".extraire-tmp");
    let tmp = sortie.with_file_name(nom);
    std::fs::write(&tmp, releve.octets()).map_err(|e| format!("{} : {}", tmp.display(), e))?;
    std::fs::rename(&tmp, sortie).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("{} : {}", sortie.display(), e)
    })
}

// ------------------------------------------------------------------ la transcription du cahier en PDF

/// LA TRANSCRIPTION D'UN CAHIER TECHNIQUE EN PDF. Le PDF ne se lit qu'une fois, a son depot (`moteur extraire transcrire`,
/// une a deux minutes pour 382 pages) : ce qu'un releve en tire -- les cellules du tableau des usages, le texte des
/// definitions -- se transcrit dans un fichier texte depose a cote de lui, qui porte l'empreinte du PDF dont il vient.
/// Les releves lisent ensuite la transcription, en un instant ; une transcription dont l'empreinte n'est plus celle du
/// PDF depose se refait.
pub struct Transcription {
    /// Le nom du PDF transcrit.
    pub pdf: String,
    /// Son empreinte SHA-256.
    pub sha256: String,
    /// Ce que l'en-tete dit d'autre (`pages`...).
    pub champs: Vec<(String, String)>,
    /// Ce qui est transcrit.
    pub corps: String,
}

const FIN_DE_L_ENTETE: &str = "# ---";

impl Transcription {
    /// La transcription de `corps`, tire du PDF `pdf`.
    pub(crate) fn de(pdf: &Path, champs: Vec<(String, String)>, corps: String) -> Resultat<Transcription> {
        Ok(Transcription { pdf: nom_de(pdf), sha256: sha256(pdf)?, champs, corps })
    }

    /// Le fichier : `# titre`, `# pdf : ...`, `# sha256 : ...`, les autres champs, `# ---`, puis le corps.
    pub fn texte(&self, titre: &str) -> String {
        let mut t = format!("# {}\n# pdf : {}\n# sha256 : {}\n", titre, self.pdf, self.sha256);
        for (k, v) in &self.champs {
            t.push_str(&format!("# {} : {}\n", k, v));
        }
        t.push_str(FIN_DE_L_ENTETE);
        t.push('\n');
        t.push_str(&self.corps);
        t
    }

    /// Lit une transcription deposee.
    pub fn lire(chemin: &Path) -> Resultat<Transcription> {
        let t = std::fs::read_to_string(chemin).map_err(|e| format!("{} illisible ({})", chemin.display(), e))?;
        let (entete, corps) = t
            .split_once(&format!("\n{}\n", FIN_DE_L_ENTETE))
            .ok_or_else(|| format!("{} : ce n'est pas une transcription (pas de ligne « {} »)", chemin.display(), FIN_DE_L_ENTETE))?;
        let (mut pdf, mut sha, mut champs) = (None, None, Vec::new());
        for l in entete.lines().skip(1) {
            let Some((k, v)) = l.strip_prefix("# ").and_then(|l| l.split_once(" : ")) else { continue };
            match k {
                "pdf" => pdf = Some(v.to_string()),
                "sha256" => sha = Some(v.to_string()),
                _ => champs.push((k.to_string(), v.to_string())),
            }
        }
        let manque = |k: &str| format!("{} : l'en-tete ne dit pas « {} »", chemin.display(), k);
        Ok(Transcription { pdf: pdf.ok_or_else(|| manque("pdf"))?, sha256: sha.ok_or_else(|| manque("sha256"))?, champs, corps: corps.to_string() })
    }

    pub(crate) fn champ(&self, k: &str) -> Option<&str> {
        self.champs.iter().find(|(c, _)| c == k).map(|(_, v)| v.as_str())
    }
}

/// Une transcription se reconnait a son extension.
pub(crate) fn en_transcription(source: &Path) -> bool {
    source.extension().and_then(|e| e.to_str()) == Some("txt")
}

/// `moteur extraire transcrire <cahier.pdf> <usages.txt> <formats.txt>` : les deux transcriptions du cahier.
fn transcrire(args: &[&str], remplacer: bool) -> i32 {
    let [pdf, usages_txt, formats_txt] = args else {
        eprintln!("moteur extraire transcrire attend le cahier en PDF puis les deux sorties : rien n'est ecrit.\n{}", USAGE);
        return 2;
    };
    let (pdf, sorties) = (PathBuf::from(pdf), [PathBuf::from(usages_txt), PathBuf::from(formats_txt)]);
    for s in &sorties {
        if !en_transcription(s) {
            eprintln!("Une transcription est un fichier .txt (recu : {}) : rien n'est ecrit.", s.display());
            return 1;
        }
        if s.exists() && !remplacer {
            eprintln!("{} existe deja (`--remplacer` pour l'ecraser) : rien n'est ecrit.", s.display());
            return 1;
        }
    }
    let faites = usages::transcrire(&pdf).and_then(|u| Ok((u, formats::transcrire(&pdf)?)));
    let (u, f) = match faites {
        Ok(x) => x,
        Err(m) => {
            eprintln!("{}", m);
            return 1;
        }
    };
    for (texte, sortie) in [(u, &sorties[0]), (f, &sorties[1])] {
        let mut nom = sortie.file_name().map(|n| n.to_os_string()).unwrap_or_default();
        nom.push(".extraire-tmp");
        let tmp = sortie.with_file_name(nom);
        if let Err(e) = std::fs::write(&tmp, texte).and_then(|_| std::fs::rename(&tmp, sortie)) {
            let _ = std::fs::remove_file(&tmp);
            eprintln!("{} : {}", sortie.display(), e);
            return 1;
        }
        println!("-> {}", sortie.display());
    }
    0
}

// ------------------------------------------------------------------ les aides communes

/// Le nom du fichier (`Path(x).name`).
pub(crate) fn nom_de(chemin: &Path) -> String {
    chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// L'empreinte SHA-256 du fichier, en hexadecimal.
pub(crate) fn sha256(chemin: &Path) -> Resultat<String> {
    let octets = std::fs::read(chemin).map_err(|e| format!("{} illisible ({})", chemin.display(), e))?;
    Ok(socle::fichiers::sha256_hex(&octets))
}

/// Les blancs (`socle::texte::blanc`) ramenes a un, sans blanc au bord.
pub(crate) fn blancs_ramenes(texte: &str) -> String {
    texte.split(socle::texte::blanc).filter(|m| !m.is_empty()).collect::<Vec<_>>().join(" ")
}

/// Le texte en forme composee (NFC).
pub(crate) fn nfc(texte: &str) -> String {
    icu_normalizer::ComposingNormalizerBorrowed::new_nfc().normalize(texte).into_owned()
}

/// Une case d'un classeur en texte comme le releve sur PDF le rend : vide pour une case vide, sinon la valeur en texte,
/// blancs ramenes a un, en forme composee (NFC).
pub(crate) fn propre(c: &Option<socle::lecture_xlsx::Cellule>) -> String {
    match c {
        None => String::new(),
        Some(c) => nfc(&blancs_ramenes(&c.en_texte())),
    }
}

/// La date du releve, comme le `_source` l'ecrit (`JJ/MM/AAAA`).
pub(crate) fn date_du_releve(jour: &Jour) -> String {
    jour.jj_mm_aaaa()
}

/// La somme de flottants : le premier terme, puis la somme compensee de Neumaier, la
/// compensation ajoutee a la fin si elle est non nulle et finie. Les frontieres des tableaux du cahier (`usages.rs`) sont des
/// moyennes : ce calcul redonne, au dernier bit, celles du referentiel livre.
pub(crate) fn somme_compensee(valeurs: &[f64]) -> f64 {
    let Some((premier, reste)) = valeurs.split_first() else { return 0.0 };
    let mut f = 0.0 + premier;
    let mut c = 0.0;
    for &x in reste {
        let t = f + x;
        if f.abs() >= x.abs() {
            c += (f - t) + x;
        } else {
            c += (x - t) + f;
        }
        f = t;
    }
    if c != 0.0 && c.is_finite() {
        f += c;
    }
    f
}
