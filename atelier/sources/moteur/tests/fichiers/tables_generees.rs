//! Les tables Unicode du moteur (`socle/src/tables_nfkc.rs`, `socle/src/nfkd_ascii.rs`, `envois/src/tables_retours.rs`,
//! `envois/src/tables_journal.rs`) sont celles que `moteur generer-tables` (`src/generer_tables.rs`) ecrit depuis les textes
//! deposes : `atelier/reference/sources/unicode/` (base de caracteres Unicode, pages de code cp437 et cp1252),
//! `python/` (les proprietes que `str.isdigit`, `str.isalnum` et `\d` traduisent ; le releve des encodages de `pyexpat`),
//! `w3c/` (XML 1.0 quatrieme edition, annexe B). Chaque `LISEZ-MOI.md` dit ce qu'on en lit.
//!
//! Une table editee a la main, ou un texte depose change sans que les tables suivent, fait echouer ce test. Pour
//! regenerer : `moteur generer-tables atelier/reference/sources atelier/sources` (ou `PUPPETPAIE_REECRIRE_ORACLES=1`
//! sur ce test), puis lire le `git diff`.

use std::path::{Path, PathBuf};

use moteur::generer_tables::{generer, FICHIERS};

use crate::outils::{dossier_de_test, reecrire_oracles};

fn sources() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference/sources")
}

#[test]
fn les_tables_commitees_sont_celles_des_textes_deposes() {
    let fichiers = generer(&sources()).unwrap();
    assert_eq!(fichiers.iter().map(|(n, _)| *n).collect::<Vec<_>>(), FICHIERS);
    let mut ecarts = Vec::new();
    for (nom, contenu) in fichiers {
        let chemin = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(nom);
        if std::fs::read_to_string(&chemin).unwrap() != contenu {
            if reecrire_oracles() {
                std::fs::write(&chemin, contenu).unwrap();
            } else {
                ecarts.push(nom);
            }
        }
    }
    assert!(ecarts.is_empty(), "{:?} ne sont pas ce que les textes deposes donnent : moteur generer-tables atelier/reference/sources atelier/sources", ecarts);
}

#[test]
fn un_texte_manquant_refuse_et_rien_ne_s_ecrit() {
    let d = dossier_de_test("tables_generees", "texte_manquant");
    let sortie = d.join("src");
    std::fs::create_dir_all(&sortie).unwrap();
    // Les sources sans le releve des encodages de pyexpat : la generation refuse avant d'ecrire quoi que ce soit.
    let copie = d.join("sources");
    for dossier in ["unicode", "w3c", "python"] {
        std::fs::create_dir_all(copie.join(dossier)).unwrap();
        for e in std::fs::read_dir(sources().join(dossier)).unwrap().flatten() {
            if !e.file_name().to_string_lossy().starts_with("python-releve-codecs") {
                std::fs::copy(e.path(), copie.join(dossier).join(e.file_name())).unwrap();
            }
        }
    }
    let args = [copie.to_string_lossy().into_owned(), sortie.to_string_lossy().into_owned()];
    assert_ne!(moteur::generer_tables::commande(&args), 0);
    assert_eq!(std::fs::read_dir(&sortie).unwrap().count(), 0, "rien ne s'ecrit");
}
