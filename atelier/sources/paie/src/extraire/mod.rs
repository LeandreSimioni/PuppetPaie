//! `paie extraire naf|usages|formats|urssaf <sources...> <sortie.json> [--remplacer]` : les releves de l'atelier, qui
//! fabriquent des referentiels de `logiciel/donnees/` a partir des textes deposes dans `atelier/reference/`.
//!
//! UN OUTIL DE L'EDITEUR. Il ne tourne qu'a la main, quand l'organisme republie son texte (une norme DSN, la NAF, la Table
//! des Urssaf) ; le moteur, lui, ne fait que lire le JSON produit (`naf.rs`, `usages.rs`, `formats.rs`, `dpae.rs`). Il est
//! dans le `paie` livre parce que c'est ce programme-la que le poste de l'editeur fait tourner, mais il n'apparait que dans
//! l'aide de bas niveau (`paie` sans argument), avec les autres outils d'atelier.
//!
//! Le portage des scripts `atelier/outils/extraire_naf.py`, `extraire_usages.py`, `extraire_formats.py` et
//! `extraire_urssaf.py` (et de leurs bibliotheques `lire_xlsx.py`, `lire_pdf.py`), retires le 15/10/2026 : chaque releve
//! redonne, a l'octet, le referentiel livre, hors le `_source` (qui nomme desormais la commande, le fichier depose et la
//! date du releve). La preuve : `tests/fichiers/extraction.rs`, qui rejoue chaque releve sur `atelier/reference/`.
//!
//! UN REFERENTIEL NE S'ECRASE PAS PAR ACCIDENT. La sortie se nomme toujours (pas de cible par defaut, contrairement a
//! l'ancien `extraire_urssaf.py`, qui ecrivait des qu'on le lancait) ; elle finit en `.json` ; un fichier deja la n'est
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
    /// L'indentation du JSON (`json.dump(..., indent=n)` du releve d'origine).
    pub indent: usize,
    /// Un saut de ligne apres le JSON (`extraire_urssaf.py` en mettait un, les autres non).
    pub saut_final: bool,
    /// Ce que la commande dit en finissant (les nombres, ce qui a ete ecarte).
    pub bilan: Vec<String>,
}

impl Releve {
    /// Les octets du fichier : `json.dump(donnees, ensure_ascii=False, indent=n)`, en LF.
    pub fn octets(&self) -> Vec<u8> {
        let mut t = socle::ecriture::json_py(&self.donnees, self.indent);
        if self.saut_final {
            t.push('\n');
        }
        t.into_bytes()
    }
}

const USAGE: &str = "usage : paie extraire naf <structure.xlsx> <correspondances.xlsx> <sortie.json> [--remplacer]\n\
\x20       paie extraire usages <dsn-cahier-technique-V.pdf | dsn-tableau-des-usages-V-AAAAMMJJ.xlsx> <sortie.json> [--remplacer]\n\
\x20       paie extraire formats <dsn-cahier-technique-V.pdf | dsn-tableaux-categories-controles-datatypes-usages-V-AAAAMMJJ.xlsx> <sortie.json> [--remplacer]\n\
\x20       paie extraire urssaf <urssaf-table-des-urssaf-AAAAMMJJ.csv> <urssaf-liste-des-sites-et-des-activites.csv> <insee-cog-2015-departements.zip> <sortie.json> [--remplacer]\n\
La sortie se nomme toujours ; un fichier deja la ne se remplace qu'avec --remplacer.";

/// `paie extraire ...` : rend le code de sortie (2 : la commande est mal formee, 1 : le releve refuse, 0 : ecrit).
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
            "paie extraire {} attend {} chemins, la sortie en dernier : rien n'est ecrit.\n{}",
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
                "{} absent : `paie sources` dit d'ou chaque texte se releve. Rien n'est ecrit.",
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

/// `" ".join(texte.split())` : les blancs (au sens de Python) ramenes a un, sans blanc au bord.
pub(crate) fn blancs_ramenes(texte: &str) -> String {
    texte.split(crate::usages::espace).filter(|m| !m.is_empty()).collect::<Vec<_>>().join(" ")
}

/// `unicodedata.normalize("NFC", texte)`.
pub(crate) fn nfc(texte: &str) -> String {
    icu_normalizer::ComposingNormalizerBorrowed::new_nfc().normalize(texte).into_owned()
}

/// Une case d'un classeur en texte comme le releve sur PDF le rend : vide pour une case vide, sinon `str()` de la valeur,
/// blancs ramenes a un, en forme composee (NFC).
pub(crate) fn propre(c: &Option<socle::lecture_xlsx::Cellule>) -> String {
    match c {
        None => String::new(),
        Some(c) => nfc(&blancs_ramenes(&c.py_str())),
    }
}

/// La date du releve, comme le `_source` l'ecrit (`JJ/MM/AAAA`).
pub(crate) fn date_du_releve(jour: &Jour) -> String {
    jour.jj_mm_aaaa()
}

/// `sum()` de Python sur des flottants (3.12 et suivants) : le premier terme, puis la somme compensee de Neumaier, la
/// compensation ajoutee a la fin si elle est non nulle et finie. Les frontieres des tableaux du cahier (`usages.rs`) sont des
/// moyennes : le meme calcul que le releve d'origine, au dernier bit.
pub(crate) fn somme_py(valeurs: &[f64]) -> f64 {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_blancs_se_ramenent_comme_en_python() {
        assert_eq!(blancs_ramenes("  a\u{a0}\n b\u{1c}c  "), "a b c");
        assert_eq!(blancs_ramenes(""), "");
    }

    #[test]
    fn la_forme_composee_recompose_un_accent_decompose() {
        assert_eq!(nfc("Re\u{301}mune\u{301}ration"), "Rémunération");
    }

    #[test]
    fn la_somme_compense_comme_python() {
        // sum([0.1] * 10) == 1.0 en Python 3.12 (0.9999999999999999 sans compensation).
        assert_eq!(somme_py(&[0.1; 10]), 1.0);
        assert_eq!(somme_py(&[1e100, 1.0, -1e100, 1.0]), 2.0);
    }
}
