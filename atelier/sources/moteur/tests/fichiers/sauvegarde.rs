//! La sauvegarde du dossier prive dans le Drive (`sauvegarde.rs`) contre un FAUX `Drive` en memoire
//! (`tests/faux_drive`) : toutes les branches de `sauvegarder`, l'inventaire et les exclusions, `rien_n_a_bouge`, `derniere`,
//! l'index octet pour octet. Le client Drive a ses tests dans `tests/fichiers/drive.rs`, l'archivage d'un mois dans
//! `tests/fichiers/archivage.rs`.
//!
//! AUCUN RESEAU, AUCUN GOOGLE : le faux ne tient que des dossiers et des fichiers en memoire. Chaque test monte sa
//! compagnie dans un dossier temporaire a lui (jamais `prive/`, jamais un vrai coffre ni jeton : `coffre.bin` et
//! `google-jeton.bin` n'y sont que des textes inventés, pour montrer lequel part et lequel ne part pas).

use crate::faux_drive;

use std::path::{Path, PathBuf};

use faux_drive::{ecrire, racine as racine_de, FauxDrive};
use envois::drive::Drive;
use moteur::compagnie::sauvegarde::{sauvegarder, EtatFichier, Sauvegarde};

const RD: &str = "PuppetPaie";

fn racine(nom: &str) -> PathBuf {
    racine_de("sauvegarde", nom)
}

/// Une compagnie : 8 fichiers a sauvegarder, 7 qui ne partent pas (sans coffre : `le_coffre_monte_le_jeton_google_non`).
fn monde(nom: &str) -> PathBuf {
    let r = racine(nom);
    for (rel, contenu) in [
        ("notes.txt", "notes"),
        ("config/entreprise.json", "{\"a\": 1}"),
        ("config/avant/ancien.json", "{}"),
        ("journal/h.md", "# h"),
        ("paies/2026-06/dupont.json", "d"),
        ("paies/avant/vieux.json", "v"),
        ("sorties/2026-06/bulletin.pdf", "%PDF"),
        ("sorties/2026-06/x.tmp/garde.txt", "dans un dossier x.tmp"),
        // ce qui ne part pas
        ("config/google-jeton.bin", "JETON-INVENTE"),
        ("config/.jeton-interface", "j"),
        ("config/.compteurs.verrou", "v"),
        ("config/.ouverture.json", "{}"),
        ("paies/__pycache__/m.cpython-311.pyc", "p"),
        ("paies/2026-06/brouillon.tmp", "t"),
        (".git/config", "g"),
    ] {
        ecrire(&r, rel, contenu);
    }
    r
}

/// Ce que la premiere passe fait, dossier par dossier (dans l'ordre des noms de dossier).
const DOSSIERS: [(&str, &[&str]); 8] = [
    (".", &["notes.txt"]),
    ("config", &["entreprise.json"]),
    ("config/avant", &["ancien.json"]),
    ("journal", &["h.md"]),
    ("paies/2026-06", &["dupont.json"]),
    ("paies/avant", &["vieux.json"]),
    ("sorties/2026-06", &["bulletin.pdf"]),
    ("sorties/2026-06/x.tmp", &["garde.txt"]),
];

fn chemin_drive(dossier: &str) -> String {
    if dossier == "." {
        format!("{}/sauvegarde", RD)
    } else {
        format!("{}/sauvegarde/{}", RD, dossier)
    }
}

fn journal_de_la_premiere_passe() -> Vec<String> {
    let mut j = Vec::new();
    for (dossier, noms) in DOSSIERS {
        let c = chemin_drive(dossier);
        j.push(format!("chemin {}", c));
        j.push(format!("lister {}", c));
        for n in noms {
            j.push(format!("televerser {}/{} connu=-", c, n));
        }
    }
    j
}

fn passer(r: &Path, d: &mut FauxDrive) -> Sauvegarde {
    sauvegarder(r, RD, false, Some(d as &mut dyn Drive)).unwrap()
}

// ------------------------------------------------------------ l'empreinte

// ------------------------------------------------------------ l'inventaire

// Unix seulement : sous Windows, un lien symbolique demande des droits que le poste n'a pas.

// ------------------------------------------------------------ la premiere passe et l'index

#[test]
fn premiere_passe_tout_part_dans_l_ordre_et_l_index_est_ecrit() {
    let r = monde("premiere");
    let mut d = FauxDrive::nouveau();
    let s = passer(&r, &mut d);
    assert_eq!(d.journal, journal_de_la_premiere_passe());
    assert_eq!(s.deposes.len(), 8);
    assert!(s.deposes.iter().all(|f| f.etat == EtatFichier::Neuf));
    assert_eq!((s.identiques, s.exclus, s.disparus, s.hors_ligne), (0, 7, 0, false));
    assert!(s.incidents.is_empty());
    assert_eq!(s.compte.as_deref(), Some("compte@exemple.invalid"));
    assert_eq!(s.racine_drive, RD);
    assert_eq!(s.octets_deposes, [5u64, 8, 2, 3, 1, 1, 4, 21].iter().sum::<u64>());
    assert!(s.a_change());
    // L'arbre du Drive : le contenu de chaque fichier, et rien des secrets.
    let arbre = d.arbre();
    assert_eq!(arbre.len(), 8);
    assert_eq!(arbre["PuppetPaie/sauvegarde/config/entreprise.json"], ["{\"a\": 1}"]);
    assert_eq!(arbre["PuppetPaie/sauvegarde/sorties/2026-06/x.tmp/garde.txt"], ["dans un dossier x.tmp"]);
    assert!(arbre.keys().all(|k| !k.contains("coffre.bin") && !k.contains("google-jeton") && !k.contains(".git") && !k.contains("pycache")));
    assert!(d.fichiers.iter().all(|f| !String::from_utf8_lossy(&f.contenu).contains("INVENTE")));
}

/// Le coffre monte, le jeton Google non (une decision d'editeur). Le coffre est scelle par le mot de passe du logiciel : c'est ce qui l'ouvre sur un
/// poste neuf apres `moteur restaurer`. Le jeton Google, lui, se refait en reliant le compte.
#[test]
fn le_coffre_monte_le_jeton_google_non() {
    let r = monde("coffre-monte");
    ecrire(&r, "config/coffre.bin", "COFFRE-INVENTE");
    let mut d = FauxDrive::nouveau();
    let s = passer(&r, &mut d);
    let arbre = d.arbre();
    assert_eq!(arbre["PuppetPaie/sauvegarde/config/coffre.bin"], ["COFFRE-INVENTE"]);
    assert!(arbre.keys().all(|k| !k.contains("google-jeton")));
    assert_eq!((s.deposes.len(), s.exclus), (9, 7));
}

// ------------------------------------------------------------ rien de change, modifie, ajoute, supprime

#[test]
fn rien_de_change_rien_ne_part_et_les_dossiers_connus_ne_sont_pas_recherches() {
    let r = monde("rien");
    let mut d = FauxDrive::nouveau();
    passer(&r, &mut d);
    d.journal.clear();
    let s = passer(&r, &mut d);
    assert!(s.deposes.is_empty() && !s.a_change());
    assert_eq!((s.identiques, s.exclus), (8, 8)); // l'index ecrit s'ajoute aux ecartes
    assert_eq!(d.appels("televerser"), 0);
    assert_eq!(d.appels("chemin"), 0, "les identifiants de dossiers viennent de l'index");
    assert_eq!(d.appels("lister"), 8, "un listage par dossier");
    assert_eq!(s.resume(), "Sauvegarde a jour (8 fichiers).");
    assert_eq!(d.arbre().len(), 8);
}

// ------------------------------------------------------------ hors ligne, incidents, disparus

// ------------------------------------------------------------ ce qui est passager, ce qui persiste

#[test]
fn copie_de_developpement_refus_avant_tout() {
    let r = monde("copie");
    ecrire(&r, "COPIE-DE-DEVELOPPEMENT.json", "{}");
    // Sans client donne : le vrai se construit, et refuse une copie de developpement avant tout (pas de reseau, pas d'index).
    let e = sauvegarder(&r, RD, false, None).unwrap_err();
    assert_eq!(e.classe, "CopieDeDeveloppement");
    assert!(!r.join("config/.sauvegarde.json").exists());
}

// ------------------------------------------------------------ rien_n_a_bouge et derniere

// ------------------------------------------------------------ l'index qui n'est pas un objet JSON

// ------------------------------------------------------------ le compte rendu

