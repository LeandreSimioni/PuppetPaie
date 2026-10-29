//! L'archivage d'un mois dans le Drive (`archivage.rs`, lot H4b) contre le FAUX `Drive` en memoire (`tests/faux_drive`).
//! Le mois est controle avant tout appel (defaut 7 : `../config` archivait `config`, coffre et jeton compris).
//!
//! AUCUN RESEAU, AUCUN GOOGLE. Chaque test monte sa compagnie dans un dossier temporaire a lui.

use crate::faux_drive;

use std::path::{Path, PathBuf};

use faux_drive::{ecrire, racine as racine_de, FauxDrive};
use moteur::compagnie::archivage::{archiver, mois_valide, Archivage};
use envois::drive::{Drive, ErreurGoogle};

const RD: &str = "PuppetPaie";

fn racine(nom: &str) -> PathBuf {
    racine_de("archivage", nom)
}

fn monde(nom: &str) -> PathBuf {
    let r = racine(nom);
    ecrire(&r, "sorties/2026-06/bulletins.pdf", "%PDF-groupe");
    ecrire(&r, "sorties/2026-06/dsn.dsn", "S10.G00.00");
    ecrire(&r, "sorties/2026-06/vide.txt", "");
    ecrire(&r, "sorties/2026-06/envoi/dupont.pdf", "fiche de Dupont");
    ecrire(&r, "config/coffre.bin", "SECRET-INVENTE");
    ecrire(&r, "config/google-jeton.bin", "JETON-INVENTE");
    ecrire(&r, "paies/2026-06/dupont.json", "{}");
    r
}

fn archiver_avec(r: &Path, mois: &str, d: &mut FauxDrive) -> Result<Archivage, ErreurGoogle> {
    archiver(r, mois, RD, Some(d as &mut dyn Drive))
}

#[test]
fn defaut_7_le_mois_est_controle_avant_tout() {
    let r = monde("defaut-7");
    for mois in [
        "..", "../config", "../../etc", "2026-06/envoi", "2026-06/", "/2026-06", ".", "", "2026-6", "2026-13", "2026-00", "26-06", "2026-06 ", " 2026-06",
        "2026/06", "2026_06", "202606", "2026-066", "abcd-ef", "+026-06", "2026-0\u{FF16}", "\u{FF12}026-06", "2026-06\n", "2026-1é", "2026-06\0",
    ] {
        let mut d = FauxDrive::nouveau();
        // Sans client donne : si le controle venait apres, le vrai client serait construit (aucun reseau n'est possible ici,
        // mais la phrase serait autre). Avec le faux : rien ne doit lui etre demande.
        let e = archiver_avec(&r, mois, &mut d).unwrap_err();
        assert_eq!(e.classe, "ErreurOperation", "mois {:?}", mois);
        assert!(e.message.starts_with("Mois illisible"), "le refus est celui du mois {:?} : {}", mois, e.message);
        assert!(d.journal.is_empty(), "mois {:?} : le Drive n'a rien a savoir", mois);
        assert!(archiver(&r, mois, RD, None).unwrap_err().message.starts_with("Mois illisible"));
    }
    // Le dossier `config` (coffre et jeton) n'a donc jamais pu partir ; le dossier `envoi` non plus.
    assert!(!mois_valide("../config"));
    assert!(!mois_valide("2026-06/envoi"));
}

