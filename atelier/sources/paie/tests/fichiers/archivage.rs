//! L'archivage d'un mois dans le Drive (`archivage.rs`, lot H4b) contre le FAUX `Drive` en memoire (`tests/faux_drive`).
//! Le mois est controle avant tout appel (defaut 7 : `../config` archivait `config`, coffre et jeton compris).
//!
//! AUCUN RESEAU, AUCUN GOOGLE. Chaque test monte sa compagnie dans un dossier temporaire a lui.

use crate::faux_drive;

use std::path::{Path, PathBuf};

use faux_drive::{ecrire, racine as racine_de, FauxDrive, Panne};
use paie::archivage::{archiver, mois_valide, Archivage, FichierArchive};
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
fn premiere_fois_tout_part_a_plat_dans_l_ordre_sans_envoi() {
    let r = monde("premiere");
    let mut d = FauxDrive::nouveau();
    let a = archiver_avec(&r, "2026-06", &mut d).unwrap();
    assert_eq!(
        a,
        Archivage {
            mois: "2026-06".into(),
            racine_drive: RD.into(),
            compte: Some("compte@exemple.invalid".into()),
            fichiers: vec![
                FichierArchive { nom: Some("bulletins.pdf".into()), octets: 11, remplace: false },
                FichierArchive { nom: Some("dsn.dsn".into()), octets: 10, remplace: false },
                FichierArchive { nom: Some("vide.txt".into()), octets: 0, remplace: false },
            ],
        }
    );
    assert_eq!(
        d.journal,
        [
            "chemin PuppetPaie/2026-06",
            "lister PuppetPaie/2026-06",
            "televerser PuppetPaie/2026-06/bulletins.pdf connu=-",
            "televerser PuppetPaie/2026-06/dsn.dsn connu=-",
            "televerser PuppetPaie/2026-06/vide.txt connu=-",
        ]
    );
    let arbre = d.arbre();
    assert_eq!(arbre.keys().cloned().collect::<Vec<_>>(), ["PuppetPaie/2026-06/bulletins.pdf", "PuppetPaie/2026-06/dsn.dsn", "PuppetPaie/2026-06/vide.txt"]);
    assert_eq!(arbre["PuppetPaie/2026-06/vide.txt"], [""], "un fichier de zero octet part aussi");
    // `envoi/` (les fiches par salarie, deja parties) n'est pas archive ; ni config, ni paies.
    assert!(d.fichiers.iter().all(|f| !String::from_utf8_lossy(&f.contenu).contains("INVENTE") && f.nom != "dupont.pdf"));
}

#[test]
fn deuxieme_fois_tout_se_remplace_sans_copie() {
    let r = monde("deuxieme");
    let mut d = FauxDrive::nouveau();
    archiver_avec(&r, "2026-06", &mut d).unwrap();
    let ids: Vec<String> = d.fichiers.iter().map(|f| f.id.clone()).collect();
    // Le mois est reproduit (un montant est arrive) : meme nom, autre contenu.
    ecrire(&r, "sorties/2026-06/dsn.dsn", "S10.G00.00-v2");
    d.journal.clear();
    let a = archiver_avec(&r, "2026-06", &mut d).unwrap();
    assert!(a.fichiers.iter().all(|f| f.remplace));
    assert_eq!(a.fichiers.len(), 3);
    assert_eq!(d.fichiers.len(), 3, "remplace, pas empile");
    assert_eq!(d.contenus("PuppetPaie/2026-06/dsn.dsn"), ["S10.G00.00-v2"]);
    for (i, f) in ["bulletins.pdf", "dsn.dsn", "vide.txt"].iter().enumerate() {
        assert!(d.journal.contains(&format!("televerser PuppetPaie/2026-06/{} connu={}", f, ids[i])), "{:?}", d.journal);
    }
    assert_eq!(d.nombre_de_dossiers(), 2);
}

#[test]
fn defaut_3_homonymes_le_dernier_est_remplace_un_dossier_de_meme_nom_jamais() {
    let r = monde("homonymes");
    let mut d = FauxDrive::nouveau();
    let premier = d.deposer("PuppetPaie/2026-06", "dsn.dsn", b"ancien-1", true);
    let dernier = d.deposer("PuppetPaie/2026-06", "dsn.dsn", b"ancien-2", true);
    d.chemin(&["PuppetPaie", "2026-06", "bulletins.pdf"]).unwrap(); // un DOSSIER nomme comme un fichier
    d.journal.clear();
    archiver_avec(&r, "2026-06", &mut d).unwrap();
    assert!(d.journal.contains(&format!("televerser PuppetPaie/2026-06/dsn.dsn connu={}", dernier)));
    assert_ne!(premier, dernier);
    assert!(d.journal.contains(&"televerser PuppetPaie/2026-06/bulletins.pdf connu=-".to_string()));
    assert_eq!(d.contenus("PuppetPaie/2026-06/dsn.dsn"), ["ancien-1", "S10.G00.00"]);
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

#[test]
fn mois_valide_aaaa_mm_ascii_et_rien_d_autre() {
    for bon in ["2026-06", "2026-01", "2026-12", "1999-09", "0001-10", "9999-12"] {
        assert!(mois_valide(bon), "{}", bon);
    }
    for mauvais in ["2026-0", "2026-1", "2026-00", "2026-13", "2026-99", "2026-0a", "x026-06", "2026–06", "2026-06-01"] {
        assert!(!mois_valide(mauvais), "{}", mauvais);
    }
}

#[test]
fn mois_absent_vide_ou_sans_fichier_est_une_sortie_absente_avant_le_client() {
    let r = racine("absent");
    ecrire(&r, "COPIE-DE-DEVELOPPEMENT.json", "{}"); // le vrai client refuserait : preuve que `SortieAbsente` vient AVANT lui
    let mut d = FauxDrive::nouveau();
    let e = archiver_avec(&r, "2026-06", &mut d).unwrap_err();
    assert_eq!(e.classe, "SortieAbsente");
    assert!(e.message.contains(&r.join("sorties").join("2026-06").display().to_string()), "le refus nomme le dossier : {}", e.message);
    assert_eq!(archiver(&r, "2026-06", RD, None).unwrap_err().classe, "SortieAbsente");
    // Un dossier vide.
    std::fs::create_dir_all(r.join("sorties/2026-06")).unwrap();
    assert_eq!(archiver(&r, "2026-06", RD, None).unwrap_err().classe, "SortieAbsente");
    // Un dossier qui ne porte que `envoi/` : rien a archiver non plus.
    ecrire(&r, "sorties/2026-06/envoi/a.pdf", "x");
    assert_eq!(archiver(&r, "2026-06", RD, None).unwrap_err().classe, "SortieAbsente");
    assert!(d.journal.is_empty());
    // Quand il y a des fichiers, c'est le vrai client qui refuse la copie de developpement.
    ecrire(&r, "sorties/2026-06/a.pdf", "x");
    assert_eq!(archiver(&r, "2026-06", RD, None).unwrap_err().classe, "CopieDeDeveloppement");
}

#[test]
fn erreur_au_milieu_leve_et_laisse_un_archivage_partiel() {
    let r = monde("milieu");
    let mut d = FauxDrive::nouveau();
    d.pannes.push(Panne { operation: "televerser", motif: "dsn.dsn".into(), erreur: ErreurGoogle::google("HTTP 500 : Backend Error") });
    let e = archiver_avec(&r, "2026-06", &mut d).unwrap_err();
    assert_eq!((e.classe, e.message.as_str()), ("ErreurGoogle", "HTTP 500 : Backend Error"));
    // Le premier est parti, les suivants non : le premier echec arrete l'archivage.
    assert_eq!(d.arbre().keys().cloned().collect::<Vec<_>>(), ["PuppetPaie/2026-06/bulletins.pdf"]);
}

#[test]
fn reseau_ou_listage_en_panne_leve_avant_tout_envoi() {
    let r = monde("panne");
    let mut d = FauxDrive::nouveau();
    d.pannes.push(Panne { operation: "lister", motif: "2026-06".into(), erreur: ErreurGoogle::google("HTTP 404 : File not found") });
    assert_eq!(archiver_avec(&r, "2026-06", &mut d).unwrap_err().message, "HTTP 404 : File not found");
    assert_eq!(d.appels("televerser"), 0);
    d.pannes.clear();
    d.pannes.push(Panne { operation: "chemin", motif: "".into(), erreur: ErreurGoogle::google("Reseau injoignable : connexion refusee") });
    assert!(archiver_avec(&r, "2026-06", &mut d).unwrap_err().message.starts_with("Reseau injoignable"));
}

#[test]
fn une_autre_racine_du_drive_et_un_compte_inconnu() {
    let r = monde("racine-drive");
    let mut d = FauxDrive::nouveau();
    d.compte = None;
    let a = archiver(&r, "2026-06", "Archives", Some(&mut d as &mut dyn Drive)).unwrap();
    assert_eq!((a.racine_drive.as_str(), a.compte), ("Archives", None));
    assert!(d.journal.contains(&"chemin Archives/2026-06".to_string()));
}

#[test]
fn un_nom_de_fichier_a_accent_et_espace_part_tel_quel() {
    let r = racine("accents");
    ecrire(&r, "sorties/2026-06/Bulletin é à ç (1).pdf", "%PDF");
    let mut d = FauxDrive::nouveau();
    let a = archiver_avec(&r, "2026-06", &mut d).unwrap();
    assert_eq!(a.fichiers[0].nom.as_deref(), Some("Bulletin é à ç (1).pdf"));
    assert_eq!(d.contenus("PuppetPaie/2026-06/Bulletin é à ç (1).pdf"), ["%PDF"]);
}
