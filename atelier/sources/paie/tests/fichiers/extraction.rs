//! Les releves de l'atelier (`paie extraire`, `src/extraire/`) : chaque referentiel livre de `logiciel/donnees/` est ce que
//! son texte depose dans `atelier/reference/` donne, a l'octet, hors le `_source` (la date du releve et la commande). Les
//! sources sont lues dans le depot meme, la ou elles sont deposees : un texte qui change sans que le referentiel soit
//! releve de nouveau, ou un releve qui ne redonne plus le referentiel, fait echouer le test.
//!
//! Les deux releves sur le cahier technique 2026.1 en PDF (382 pages) lisent tout le document par la fabrique : une a deux
//! minutes chacun.
//!
//! Et le contrat de la commande : la sortie se nomme toujours, un referentiel deja la ne se remplace qu'avec `--remplacer`.

use std::path::{Path, PathBuf};

use socle::date::Jour;
use paie::extraire::{self, Releve};
use serde_json::Value;

use crate::outils::{code_livre, dossier_de_test};

fn reference(chemin: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference").join(chemin)
}

fn jour() -> Jour {
    Jour::nouveau(2026, 10, 15).unwrap()
}

/// Le releve redonne le fichier livre `nom`, octet pour octet, une fois son `_source` remplace par celui du livre ; et son
/// `_source` nomme la commande et chaque fichier depose qu'il a lu.
fn redonne_le_livre(releve: Releve, nom: &str, commande: &str, deposes: &[&str]) {
    let source = releve.donnees["_source"].as_str().unwrap_or("").to_string();
    assert!(source.contains(commande), "le _source de {} ne nomme pas `{}` : {}", nom, commande, source);
    for d in deposes {
        assert!(source.contains(d), "le _source de {} ne nomme pas {} : {}", nom, d, source);
    }
    let livre = std::fs::read(code_livre().join(nom)).unwrap();
    let lu: Value = serde_json::from_slice(&livre).unwrap();
    let mut donnees = releve.donnees;
    donnees["_source"] = lu["_source"].clone();
    let refait = Releve { donnees, indent: releve.indent, saut_final: releve.saut_final, bilan: Vec::new() }.octets();
    if refait != livre {
        let (a, b) = (String::from_utf8_lossy(&refait), String::from_utf8_lossy(&livre));
        let ligne =
            a.lines().zip(b.lines()).position(|(x, y)| x != y).unwrap_or(a.lines().count().min(b.lines().count()));
        panic!(
            "{} : le releve differe du livre a la ligne {} :\n  releve : {:?}\n  livre  : {:?}",
            nom,
            ligne + 1,
            a.lines().nth(ligne),
            b.lines().nth(ligne)
        );
    }
}

#[test]
fn la_naf_livree_est_celle_des_classeurs_de_l_insee() {
    let (s, c) = (
        "insee-naf-2025-structure-20241004.xlsx",
        "insee-naf-2025-correspondances-naf-rev2-vers-naf-2025-20260107.xlsx",
    );
    let r = extraire::naf::relever(
        &reference(&format!("sources/insee-naf-2025/{}", s)),
        &reference(&format!("sources/insee-naf-2025/{}", c)),
        &jour(),
    );
    redonne_le_livre(r.unwrap(), "naf-base.json", "paie extraire naf", &[s, c]);
}

#[test]
fn les_usages_p27v01_livres_sont_ceux_du_classeur_2027_1_2() {
    let x = "dsn-tableau-des-usages-2027.1.2-20251224.xlsx";
    let r = extraire::usages::relever(&reference(&format!("norme-dsn/{}", x)), &jour());
    redonne_le_livre(r.unwrap(), "usages-dsn-p27v01-base.json", "paie extraire usages", &[x]);
}

#[test]
fn les_formats_p27v01_livres_sont_ceux_du_classeur_2027_1_2() {
    let x = "dsn-tableaux-categories-controles-datatypes-usages-2027.1.2-20251224.xlsx";
    let r = extraire::formats::relever(&reference(&format!("norme-dsn/{}", x)), &jour());
    redonne_le_livre(r.unwrap(), "formats-dsn-p27v01-base.json", "paie extraire formats", &[x]);
}

/// Le tableau des usages par rubrique du cahier 2026.1 occupe les pages 363 a 371 ; le tableau des usages des controles
/// qui le suit (p. 372-382) n'a aucune ligne de rubrique.
#[test]
fn les_usages_p26v01_livres_sont_ceux_du_cahier_2026_1() {
    let pdf = "dsn-cahier-technique-2026.1.pdf";
    let r = extraire::usages::relever(&reference(&format!("norme-dsn/{}", pdf)), &jour()).unwrap();
    assert!(r.donnees["_source"].as_str().unwrap().contains("(p. 363-371)"), "{}", r.donnees["_source"]);
    redonne_le_livre(r, "usages-dsn-base.json", "paie extraire usages", &[pdf]);
}

#[test]
fn les_formats_p26v01_livres_sont_ceux_du_cahier_2026_1() {
    let pdf = "dsn-cahier-technique-2026.1.pdf";
    let r = extraire::formats::relever(&reference(&format!("norme-dsn/{}", pdf)), &jour());
    redonne_le_livre(r.unwrap(), "formats-dsn-base.json", "paie extraire formats", &[pdf]);
}

const TABLE: &str = "sources/urssaf-table-des-urssaf-20260618.csv";
const SITES: &str = "sources/urssaf-liste-des-sites-et-des-activites.csv";
const COG: &str = "sources/insee-cog-2015-departements.zip";

#[test]
fn les_organismes_urssaf_livres_sont_ceux_des_trois_tables() {
    let r = extraire::urssaf::relever(&reference(TABLE), &reference(SITES), &reference(COG), &jour());
    redonne_le_livre(
        r.unwrap(),
        "urssaf-dpae-base.json",
        "paie extraire urssaf",
        &[
            "urssaf-table-des-urssaf-20260618.csv",
            "urssaf-liste-des-sites-et-des-activites.csv",
            "insee-cog-2015-departements.zip",
        ],
    );
}

fn args(liste: &[&str]) -> Vec<String> {
    liste.iter().map(|s| s.to_string()).collect()
}

/// Sans sortie nommee, rien ne s'ecrit ; une sortie deja la ne se remplace qu'avec `--remplacer` ; une sortie qui n'est
/// pas un `.json` est refusee.
#[test]
fn un_referentiel_ne_s_ecrase_pas_par_accident() {
    let d = dossier_de_test("extraction", "un_referentiel_ne_s_ecrase_pas_par_accident");
    let (table, sites, cog) = (reference(TABLE), reference(SITES), reference(COG));
    let (table, sites, cog) = (table.to_str().unwrap(), sites.to_str().unwrap(), cog.to_str().unwrap());
    let sortie = d.join("urssaf-dpae-base.json");
    let s = sortie.to_str().unwrap();

    // La sortie oubliee : la commande ne prend pas la derniere source pour une cible.
    assert_eq!(extraire::commande(&args(&["urssaf", table, sites, cog])), 2);
    assert_eq!(extraire::commande(&args(&["urssaf", table, sites, s])), 2);
    assert!(!sortie.exists());
    // Une sortie qui n'est pas un .json : refus, rien d'ecrit.
    let texte = d.join("sortie.txt");
    assert_eq!(extraire::commande(&args(&["urssaf", table, sites, cog, texte.to_str().unwrap()])), 1);
    assert!(!texte.exists());

    // Un referentiel deja la : refus sans --remplacer, le fichier intact.
    std::fs::write(&sortie, "garde").unwrap();
    assert_eq!(extraire::commande(&args(&["urssaf", table, sites, cog, s])), 1);
    assert_eq!(std::fs::read_to_string(&sortie).unwrap(), "garde");
    // Avec --remplacer, il est remplace par le releve entier, et rien d'autre ne reste dans le dossier.
    assert_eq!(extraire::commande(&args(&["urssaf", table, sites, cog, s, "--remplacer"])), 0);
    let ecrit: Value = serde_json::from_str(&std::fs::read_to_string(&sortie).unwrap()).unwrap();
    assert_eq!(ecrit["par_departement"]["75"], "117");
    let restes: Vec<_> = std::fs::read_dir(&d).unwrap().flatten().map(|e| e.file_name()).collect();
    assert_eq!(restes.len(), 1, "{:?}", restes);
}
