//! La restauration du dossier prive depuis le Drive (`restauration.rs`) contre le FAUX `Drive` en memoire
//! (`tests/faux_drive`) : l'index reconstruit
//! laisse la sauvegarde suivante sans rien a faire ; le dossier qui n'est pas vide (refus, ou accepte s'il ne porte que le
//! jeton ou des copies identiques, d'ou la reprise apres une coupure) ; l'empreinte qui ne colle pas ; ce qui ne redescend
//! pas ; et ce qu'il reste a faire, coffre compris (un coffre invente, sur deux postes inventes).
//!
//! AUCUN RESEAU, AUCUN GOOGLE, jamais `prive/` ni `fictives/` vivante.

use crate::faux_drive;
use crate::outils::dossier_de_test;

use std::path::{Path, PathBuf};

use faux_drive::{ecrire, FauxDrive};
use envois::drive::Drive;
use moteur::compagnie::restauration::restaurer;
use moteur::compagnie::sauvegarde::sauvegarder;

const RD: &str = "PuppetPaie";

fn dossier(test: &str) -> PathBuf {
    dossier_de_test("restauration", test)
}

/// Une petite compagnie : six fichiers qui partent, deux qui ne partent pas.
fn petite(r: &Path) {
    for (rel, contenu) in [
        ("notes.txt", "notes"),
        ("config/entreprise.json", "{\"a\": 1}"),
        ("journal/depots.json", "[]"),
        ("paies/2026-06/dupont.json", "{\"d\": 1}"),
        ("paies/avant/vieux.json", "v"),
        ("sorties/2026-06/bulletin.pdf", "%PDF-1.4 bulletin"),
        ("config/google-jeton.bin", "JETON-INVENTE"),
        ("config/.compteurs.verrou", "v"),
    ] {
        ecrire(r, rel, contenu);
    }
}

fn sauvegarder_dans(r: &Path, d: &mut FauxDrive) {
    let s = sauvegarder(r, RD, false, Some(d as &mut dyn Drive)).unwrap();
    assert!(s.incidents.is_empty(), "{:?}", s.incidents);
}

/// Tous les fichiers d'un dossier (ecartes compris), relatifs.
fn tout(r: &Path) -> Vec<String> {
    fn parcourir(base: &Path, d: &Path, v: &mut Vec<String>) {
        for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
            if e.path().is_dir() {
                parcourir(base, &e.path(), v);
            } else {
                v.push(e.path().strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let mut v = Vec::new();
    parcourir(r, r, &mut v);
    v.sort();
    v
}

// ------------------------------------------------------------ l'aller et le retour

// ------------------------------------------------------------ un dossier qui n'est pas vide

#[test]
fn un_dossier_qui_porte_autre_chose_refuse_et_rien_n_est_ecrit_ni_efface() {
    let base = dossier("non-vide");
    let source = base.join("source");
    petite(&source);
    let mut d = FauxDrive::nouveau();
    sauvegarder_dans(&source, &mut d);
    let cible = base.join("cible");
    ecrire(&cible, "config/entreprise.json", "{\"a\": 2}"); // une autre version
    ecrire(&cible, "paies/2026-07/martin.json", "{}"); // absent de la sauvegarde
    ecrire(&cible, "notes.txt", "notes"); // identique : pas un ecart
    let avant = tout(&cible);
    d.journal.clear();
    let e = restaurer(&cible, RD, &mut d).unwrap_err();
    assert_eq!(e.classe, "DossierNonVide");
    assert!(e.message.contains("2 fichier(s)"), "{}", e.message);
    assert!(e.message.contains("config/entreprise.json (differe de la copie du Drive)"), "{}", e.message);
    assert!(e.message.contains("paies/2026-07/martin.json (absent de la sauvegarde)"), "{}", e.message);
    assert!(e.message.contains("Rien n'a ete redescendu, rien n'a ete efface"), "{}", e.message);
    assert!(!e.message.contains("notes.txt"), "{}", e.message);
    // Rien de touche : les memes fichiers, le meme contenu, pas d'index, aucun telechargement.
    assert_eq!(tout(&cible), avant);
    assert_eq!(std::fs::read_to_string(cible.join("config/entreprise.json")).unwrap(), "{\"a\": 2}");
    assert_eq!(d.appels("telecharger"), 0);
}

// ------------------------------------------------------------ l'empreinte, et ce qui ne redescend pas

#[test]
fn une_empreinte_qui_ne_colle_pas_n_ecrit_rien_de_ce_fichier() {
    let base = dossier("empreinte");
    let source = base.join("source");
    petite(&source);
    let mut d = FauxDrive::nouveau();
    sauvegarder_dans(&source, &mut d);
    // Le Drive annonce une empreinte que le contenu ne donne pas (un transfert abime, un fichier change en route).
    d.fichier_mut("PuppetPaie/sauvegarde/paies/2026-06/dupont.json").md5 = Some("0".repeat(32));
    let cible = base.join("cible");
    let r = restaurer(&cible, RD, &mut d).unwrap();
    assert_eq!(r.incidents.len(), 1, "{:?}", r.incidents);
    assert!(r.incidents[0].starts_with("paies/2026-06/dupont.json") && r.incidents[0].contains(&"0".repeat(32)), "le fichier et l'empreinte annoncee : {}", r.incidents[0]);
    assert_eq!(r.redescendus.len(), 5);
    // Ni le fichier, ni un `.part`, ni une ligne d'index.
    assert!(!cible.join("paies/2026-06/dupont.json").exists());
    assert!(tout(&cible).iter().all(|f| !f.ends_with(".part") && !f.contains("dupont")), "{:?}", tout(&cible));
    assert!(!std::fs::read_to_string(cible.join("config/.sauvegarde.json")).unwrap().contains("dupont"));
}

// ------------------------------------------------------------ ce qu'il reste a faire : le coffre

// ------------------------------------------------------------ ce qu'il reste a faire : la cle de signature

