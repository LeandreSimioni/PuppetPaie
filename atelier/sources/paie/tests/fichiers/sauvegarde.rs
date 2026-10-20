//! La sauvegarde du dossier prive dans le Drive (`sauvegarde.rs`, lot H4b) contre un FAUX `Drive` en memoire
//! (`tests/faux_drive`) : toutes les branches de `sauvegarder`, l'inventaire et les exclusions, `rien_n_a_bouge`, `derniere`,
//! l'index octet pour octet, et UN TEST PAR DEFAUT CORRIGE du Python (les sept ; le 1 et le 2 aussi dans `tests/drive.rs`,
//! le 7 dans `tests/archivage.rs`).
//!
//! AUCUN RESEAU, AUCUN GOOGLE : le faux ne tient que des dossiers et des fichiers en memoire. Chaque test monte sa
//! compagnie dans un dossier temporaire a lui (jamais `prive/`, jamais un vrai coffre ni jeton : `coffre.bin` et
//! `google-jeton.bin` n'y sont que des textes inventés, pour montrer lequel part et lequel ne part pas).

use crate::faux_drive;

use std::path::{Path, PathBuf};

use faux_drive::{ecrire, fixer_date, md5_de, racine as racine_de, FauxDrive, Panne};
use md5::{Digest, Md5};
use paie::drive::{Drive, ErreurGoogle};
use paie::sauvegarde::{self, derniere, empreinte, inventaire, rien_n_a_bouge, sauvegarder, EtatFichier, Fichier, Sauvegarde};
use serde_json::{json, Value};

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

/// Ce que la premiere passe fait, dossier par dossier (dans l'ordre de `sorted(par_dossier.items())`).
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

fn lire_index(r: &Path) -> String {
    std::fs::read_to_string(r.join("config").join(".sauvegarde.json")).unwrap()
}

fn index_json(r: &Path) -> Value {
    serde_json::from_str(&lire_index(r)).unwrap()
}

/// Le texte de l'index avec `quand` masque (`AAAA-MM-JJ HH:MM`, a la minute) ; le format est verifie au passage.
fn index_masque(r: &Path) -> String {
    let t = lire_index(r);
    let mut sortie = Vec::new();
    for l in t.lines() {
        if let Some(v) = l.strip_prefix(" \"quand\": \"") {
            let v = v.strip_suffix("\",").expect(l);
            let b = v.as_bytes();
            assert!(v.len() == 16 && b[4] == b'-' && b[7] == b'-' && b[10] == b' ' && b[13] == b':', "quand : {}", v);
            sortie.push(" \"quand\": \"MASQUE\",".to_string());
        } else {
            sortie.push(l.to_string());
        }
    }
    sortie.join("\n") + "\n"
}

fn erreur(classe: &'static str, message: &str) -> ErreurGoogle {
    ErreurGoogle::nouvelle(classe, message)
}

// ------------------------------------------------------------ l'empreinte

#[test]
fn empreinte_md5_connue_et_aux_bornes_des_blocs() {
    let r = racine("empreinte");
    ecrire(&r, "abc", "abc");
    assert_eq!(empreinte(&r.join("abc")).unwrap(), "900150983cd24fb0d6963f7d28e17f72");
    // Lecture par blocs de 256 Ko : les tailles autour de la limite donnent l'empreinte du fichier entier.
    for n in [0usize, 1, 262_143, 262_144, 262_145, 524_288, 3 * 1024 * 1024] {
        let octets: Vec<u8> = (0..n).map(|i| (i * 31 % 251) as u8).collect();
        let p = r.join(format!("bloc-{}", n));
        std::fs::write(&p, &octets).unwrap();
        let attendu: String = Md5::digest(&octets).iter().map(|b| format!("{:02x}", b)).collect();
        assert_eq!(empreinte(&p).unwrap(), attendu, "{} octets", n);
    }
    assert_eq!(empreinte(&r.join("bloc-0")).unwrap(), "d41d8cd98f00b204e9800998ecf8427e");
    assert!(empreinte(&r.join("absent")).is_err());
}

// ------------------------------------------------------------ l'inventaire

#[test]
fn inventaire_ordre_exclusions_et_nombre_d_ecartes() {
    let r = monde("inventaire");
    let (presents, ecartes) = inventaire(&r);
    let noms: Vec<&str> = presents.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        noms,
        [
            "config/avant/ancien.json",
            "config/entreprise.json",
            "journal/h.md",
            "notes.txt",
            "paies/2026-06/dupont.json",
            "paies/avant/vieux.json",
            "sorties/2026-06/bulletin.pdf",
            "sorties/2026-06/x.tmp/garde.txt",
        ]
    );
    assert_eq!(ecartes, 7);
    let (_, e) = presents.iter().find(|(n, _)| n == "notes.txt").unwrap();
    assert_eq!((e.octets, e.mtime), (5, 1_700_000_000));
}

#[test]
fn inventaire_les_noms_exclus_ou_les_suffixes_n_importe_ou() {
    let r = racine("exclusions");
    for rel in [
        "google-jeton.bin", ".jeton-interface", ".compteurs.verrou", ".sauvegarde.json", ".ouverture.json",               // les noms
        "a.pyc", "a.tmp", "a.lock", "a.part",                                                                              // les suffixes
        "__pycache__/f.txt", "x/__pycache__/y/f.txt", ".git/HEAD", "d/.git/objets/o",                                      // les dossiers, au milieu d'un chemin
        "garde.txt", "coffre.bin", "coffre.bin.txt", "tmp", "x.tmp.txt", "dossier.tmp/dedans.txt", ".gitignore", "git/f.txt",            // ce qui reste
    ] {
        ecrire(&r, rel, "1");
    }
    let (presents, ecartes) = inventaire(&r);
    let noms: Vec<&str> = presents.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(noms, [".gitignore", "coffre.bin", "coffre.bin.txt", "dossier.tmp/dedans.txt", "garde.txt", "git/f.txt", "tmp", "x.tmp.txt"]);
    assert_eq!(ecartes, 13);
}

// Unix seulement : sous Windows, un lien symbolique demande des droits que le poste n'a pas.
#[cfg(unix)]
#[test]
fn inventaire_liens_et_dossiers_illisibles() {
    use std::os::unix::fs::symlink;
    let r = racine("liens");
    ecrire(&r, "reel/f.txt", "1");
    ecrire(&r, "cible.txt", "22");
    symlink(r.join("cible.txt"), r.join("lien.txt")).unwrap();
    symlink(r.join("reel"), r.join("lien-dossier")).unwrap();
    symlink(r.join("n-existe-pas"), r.join("lien-casse")).unwrap();
    let (presents, _) = inventaire(&r);
    let noms: Vec<&str> = presents.iter().map(|(n, _)| n.as_str()).collect();
    // Un lien vers un fichier compte (taille de la cible) ; un lien vers un dossier n'est pas suivi ; un lien casse n'est pas un fichier.
    assert_eq!(noms, ["cible.txt", "lien.txt", "reel/f.txt"]);
    assert_eq!(presents[1].1.octets, 2);
}

#[test]
fn inventaire_la_date_garde_l_arrondi_des_index_deja_ecrits() {
    // La date se compte comme dans les index deja ecrits (`int(secondes + ns * 1e-9)` en flottant) : a 999 999 999 ns, la
    // seconde suivante. Sinon ces fichiers repartiraient tous une fois.
    let r = racine("mtime");
    for (nom, ns) in [("a", 0u32), ("b", 500_000_000), ("c", 999_999_000), ("d", 999_999_999)] {
        ecrire(&r, nom, "1");
        fixer_date(&r.join(nom), 1_700_000_000, ns);
    }
    let (presents, _) = inventaire(&r);
    let m: Vec<i64> = presents.iter().map(|(_, e)| e.mtime).collect();
    assert_eq!(m, [1_700_000_000, 1_700_000_000, 1_700_000_000, 1_700_000_001]);
}

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

/// Le coffre monte, le jeton Google non (Leandre, 13/10/2026, redit le 14/10 : « j'avais choisi de l'envoyer sur le
/// drive » ; `prive/journal/ETAT.md`). Le coffre est scelle par le mot de passe du logiciel : c'est ce qui l'ouvre sur un
/// poste neuf apres `paie restaurer`. Le jeton Google, lui, se refait en reliant le compte.
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

#[test]
fn l_index_octet_pour_octet() {
    // Reference : `json.dumps(index, ensure_ascii=False, indent=1) + "\n"` de `util.ecrire_json` (LF), `quand` a la minute,
    // `dossiers` dans l'ordre de passage, `fichiers` dans l'ordre de `sorted(rglob)`.
    let r = racine("index-octets");
    ecrire(&r, "config/é à.json", "{}");
    ecrire(&r, "notes.txt", "12345");
    fixer_date(&r.join("notes.txt"), 1_700_000_042, 0);
    let mut d = FauxDrive::nouveau();
    passer(&r, &mut d);
    let (racine_d, sauv, config) = (d.id_du_dossier("PuppetPaie"), d.id_du_dossier("PuppetPaie/sauvegarde"), d.id_du_dossier("PuppetPaie/sauvegarde/config"));
    let _ = racine_d;
    let attendu = format!(
        "{{\n \"quand\": \"MASQUE\",\n \"racine_drive\": \"PuppetPaie\",\n \"dossiers\": {{\n  \"PuppetPaie/sauvegarde\": \"{}\",\n  \"PuppetPaie/sauvegarde/config\": \"{}\"\n }},\n \"fichiers\": {{\n  \"config/é à.json\": {{\n   \"octets\": 2,\n   \"mtime\": 1700000000\n  }},\n  \"notes.txt\": {{\n   \"octets\": 5,\n   \"mtime\": 1700000042\n  }}\n }}\n}}\n",
        sauv, config
    );
    assert_eq!(index_masque(&r), attendu);
    // Les octets bruts : LF seulement, accents en UTF-8 et pas en \uXXXX, un saut de ligne final.
    let brut = std::fs::read(r.join("config").join(".sauvegarde.json")).unwrap();
    assert!(!brut.contains(&b'\r') && brut.ends_with(b"}\n") && String::from_utf8(brut).unwrap().contains("é à.json"));
}

#[test]
fn l_index_ne_porte_pas_de_secret_et_n_est_pas_sauvegarde() {
    let r = monde("index-propre");
    let mut d = FauxDrive::nouveau();
    passer(&r, &mut d);
    let t = lire_index(&r);
    assert!(!t.contains("coffre.bin") && !t.contains("google-jeton") && !t.contains(".sauvegarde.json"));
    // Le deuxieme passage voit l'index ecrit, et l'ecarte : rien a envoyer.
    let s = passer(&r, &mut d);
    assert!(s.deposes.is_empty());
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

#[test]
fn modifie_ajoute_supprime() {
    let r = monde("changements");
    let mut d = FauxDrive::nouveau();
    passer(&r, &mut d);
    let ids: std::collections::BTreeMap<String, String> = d.fichiers.iter().map(|f| (f.nom.clone(), f.id.clone())).collect();
    d.journal.clear();
    // Modifie : meme taille, autre contenu. Ajoute : un fichier neuf. Supprime : plus la.
    ecrire(&r, "config/entreprise.json", "{\"a\": 2}");
    ecrire(&r, "config/nouveau.json", "[]");
    std::fs::remove_file(r.join("journal/h.md")).unwrap();
    let s = passer(&r, &mut d);
    let envoyes: Vec<(&str, EtatFichier)> = s.deposes.iter().map(|f| (f.relatif.as_str(), f.etat)).collect();
    assert_eq!(envoyes, [("config/entreprise.json", EtatFichier::Modifie), ("config/nouveau.json", EtatFichier::Neuf)]);
    assert_eq!(s.identiques, 6);
    assert_eq!(d.appels("televerser"), 2);
    assert!(d.journal.contains(&format!("televerser PuppetPaie/sauvegarde/config/entreprise.json connu={}", ids["entreprise.json"])));
    assert!(d.journal.contains(&"televerser PuppetPaie/sauvegarde/config/nouveau.json connu=-".to_string()));
    // Remplace, pas empile ; le supprime reste au Drive (rien n'est jamais supprime) ; l'index l'oublie.
    assert_eq!(d.contenus("PuppetPaie/sauvegarde/config/entreprise.json"), ["{\"a\": 2}"]);
    assert_eq!(d.contenus("PuppetPaie/sauvegarde/journal/h.md"), ["# h"]);
    assert_eq!(d.arbre().len(), 9);
    assert!(index_json(&r)["fichiers"].get("journal/h.md").is_none());
    assert_eq!(s.resume(), "Sauvegarde : 2 fichier(s) envoye(s), 10 o, 6 inchange(s).");
}

#[test]
fn tout_renvoie_tout_en_remplacant_sans_comparer_et_sans_doublon() {
    let r = monde("tout");
    let mut d = FauxDrive::nouveau();
    passer(&r, &mut d);
    d.journal.clear();
    let s = sauvegarder(&r, RD, true, Some(&mut d as &mut dyn Drive)).unwrap();
    assert_eq!(s.deposes.len(), 8);
    assert!(s.deposes.iter().all(|f| f.etat == EtatFichier::Modifie), "ce que Drive connait se remplace");
    assert_eq!(s.identiques, 0);
    assert_eq!(d.arbre().len(), 8, "aucun doublon");
    assert_eq!(d.fichiers.len(), 8);
    // `tout` ignore les identifiants de l'index (les dossiers se retrouvent) mais LISTE quand meme : c'est `lister` qui dit ce qui se remplace.
    assert_eq!(d.appels("chemin"), 8);
    assert_eq!(d.appels("lister"), 8);
    assert!(d.journal.iter().filter(|l| l.starts_with("televerser")).all(|l| !l.ends_with("connu=-")));
}

// ------------------------------------------------------------ les defauts du Python, un test chacun

#[test]
fn defaut_3_on_remplace_le_fichier_que_lister_retient_jamais_un_dossier_ni_un_autre_doublon() {
    let r = racine("defaut-3");
    ecrire(&r, "notes.txt", "local");
    ecrire(&r, "config/p.json", "local-config");
    let mut d = FauxDrive::nouveau();
    // Deux fichiers de meme nom (Drive l'autorise), contenus differents du local.
    let premier = d.deposer("PuppetPaie/sauvegarde", "notes.txt", b"ancien-1", true);
    let dernier = d.deposer("PuppetPaie/sauvegarde", "notes.txt", b"ancien-2", true);
    // Un DOSSIER de meme nom qu'un fichier a envoyer.
    d.chemin(&["PuppetPaie", "sauvegarde", "config", "p.json"]).unwrap();
    d.journal.clear();
    let s = passer(&r, &mut d);
    // On remplace le DERNIER des doublons (celui que `lister` retient) ; le premier n'est ni touche ni supprime.
    assert!(d.journal.contains(&format!("televerser PuppetPaie/sauvegarde/notes.txt connu={}", dernier)), "{:?}", d.journal);
    assert_ne!(premier, dernier);
    assert_eq!(d.contenus("PuppetPaie/sauvegarde/notes.txt"), ["ancien-1", "local"]);
    // Le dossier `p.json` n'est pas un fichier : un fichier neuf est cree a cote, sans PATCH de contenu sur un dossier.
    assert!(d.journal.contains(&"televerser PuppetPaie/sauvegarde/config/p.json connu=-".to_string()));
    assert_eq!(s.deposes.iter().map(|f| f.etat).collect::<Vec<_>>(), [EtatFichier::Modifie, EtatFichier::Neuf]);
}

#[test]
fn defaut_4_aucune_recherche_par_fichier_un_seul_listage_par_dossier() {
    let r = monde("defaut-4");
    let mut d = FauxDrive::nouveau();
    passer(&r, &mut d);
    // Huit fichiers neufs, huit dossiers : 8 listages, 8 envois, et le journal n'a aucune autre sorte d'appel (le Python
    // faisait une recherche par fichier, 8 de plus, toutes vides).
    assert_eq!(d.appels("lister"), 8);
    assert_eq!(d.appels("televerser"), 8);
    assert_eq!(d.appels("chemin"), 8);
    assert_eq!(d.journal.len(), 24);
}

#[test]
fn defaut_5_un_fichier_sans_empreinte_chez_drive_est_remplace_donc_modifie_pas_neuf() {
    let r = racine("defaut-5");
    ecrire(&r, "doc.txt", "local");
    let mut d = FauxDrive::nouveau();
    let id = d.deposer("PuppetPaie/sauvegarde", "doc.txt", b"version-drive", false);
    d.journal.clear();
    let s = passer(&r, &mut d);
    assert_eq!(s.deposes, [Fichier { relatif: "doc.txt".into(), octets: 5, etat: EtatFichier::Modifie }]);
    assert!(d.journal.contains(&format!("televerser PuppetPaie/sauvegarde/doc.txt connu={}", id)));
    assert_eq!(d.fichiers.len(), 1, "remplace, pas empile");
    // Le fichier est maintenant identique chez Drive (il a une empreinte) : rien ne part.
    assert!(passer(&r, &mut d).deposes.is_empty());
    // Un Drive qui perd l'empreinte (un fichier Google Docs) : rien a comparer, le fichier est renvoye, en remplacement.
    d.fichiers[0].md5 = None;
    let s = passer(&r, &mut d);
    assert_eq!(s.deposes.len(), 1);
    assert_eq!(s.deposes[0].etat, EtatFichier::Modifie);
    assert_eq!(d.fichiers.len(), 1);
}

#[test]
fn defaut_6_rien_n_a_bouge_regarde_la_racine_du_drive() {
    let r = monde("defaut-6");
    let mut d = FauxDrive::nouveau();
    passer(&r, &mut d);
    assert!(rien_n_a_bouge(&r, RD));
    assert!(!rien_n_a_bouge(&r, "AutreRacine"), "un index ecrit pour une autre racine ne vaut pas pour celle-ci");
    assert!(!rien_n_a_bouge(&r, "puppetpaie"));
    assert!(!rien_n_a_bouge(&r, ""));
    // Un index qui ne dit aucune racine : non.
    let mut i = index_json(&r);
    i.as_object_mut().unwrap().remove("racine_drive");
    std::fs::write(r.join("config/.sauvegarde.json"), i.to_string()).unwrap();
    assert!(!rien_n_a_bouge(&r, RD));
    // Une racine qui n'est pas un texte : non.
    i["racine_drive"] = json!(5);
    std::fs::write(r.join("config/.sauvegarde.json"), i.to_string()).unwrap();
    assert!(!rien_n_a_bouge(&r, RD));
}

#[test]
fn defaut_6_une_sauvegarde_vers_une_autre_racine_reecrit_l_index_pour_celle_la() {
    let r = monde("defaut-6b");
    let mut d = FauxDrive::nouveau();
    passer(&r, &mut d);
    let s = sauvegarder(&r, "Essai", false, Some(&mut d as &mut dyn Drive)).unwrap();
    assert_eq!(s.deposes.len(), 8, "une autre racine du Drive : tout y est neuf");
    assert!(rien_n_a_bouge(&r, "Essai"));
    assert!(!rien_n_a_bouge(&r, RD));
}

// (defaut 1 : `tests/drive.rs` ; defaut 2 : plus bas pour la sauvegarde, `tests/drive.rs` pour le client ; defaut 7 : `tests/archivage.rs`)

#[test]
fn defaut_2_un_delai_depasse_est_un_incident_du_dossier_pas_la_fin_de_la_sauvegarde() {
    let r = monde("defaut-2");
    let mut d = FauxDrive::nouveau();
    d.pannes.push(Panne { operation: "lister", motif: "sauvegarde/config".into(), erreur: erreur("TimeoutError", "The read operation timed out") });
    let s = passer(&r, &mut d);
    assert!(!s.hors_ligne);
    // `config` et `config/avant` portent le motif : deux incidents, tout le reste a ete envoye.
    assert_eq!(s.incidents, ["config : The read operation timed out", "config/avant : The read operation timed out"]);
    assert_eq!(s.deposes.len(), 6);
    let i = index_json(&r);
    assert!(i["fichiers"].get("config/entreprise.json").is_none());
    assert!(i["dossiers"].get("PuppetPaie/sauvegarde/config").is_none(), "un dossier en echec n'est pas retenu");
    assert!(i["fichiers"].get("notes.txt").is_some());
    // Le passage suivant reprend ce qui manque, et seulement cela.
    d.pannes.clear();
    d.journal.clear();
    let s = passer(&r, &mut d);
    assert_eq!(s.deposes.iter().map(|f| f.relatif.as_str()).collect::<Vec<_>>(), ["config/entreprise.json", "config/avant/ancien.json"]);
    assert_eq!(s.identiques, 6);
    assert!(s.incidents.is_empty());
}

// ------------------------------------------------------------ hors ligne, incidents, disparus

#[test]
fn reseau_absent_arret_au_premier_dossier_et_rien_n_entre_a_l_index() {
    let r = monde("hors-ligne");
    let mut d = FauxDrive::nouveau();
    d.pannes.push(Panne { operation: "chemin", motif: "".into(), erreur: ErreurGoogle::google("Reseau injoignable : [Errno 111] Connection refused") });
    let s = passer(&r, &mut d);
    assert!(s.hors_ligne);
    assert!(s.deposes.is_empty() && s.incidents.is_empty());
    assert_eq!(d.journal, ["chemin PuppetPaie/sauvegarde"], "arret au premier dossier");
    assert_eq!(s.resume(), "Hors connexion : sauvegarde au prochain passage.");
    assert_eq!(index_masque(&r), "{\n \"quand\": \"MASQUE\",\n \"racine_drive\": \"PuppetPaie\",\n \"dossiers\": {},\n \"fichiers\": {}\n}\n");
    assert!(!rien_n_a_bouge(&r, RD));
    // Le reseau revient : tout part.
    d.pannes.clear();
    let s = passer(&r, &mut d);
    assert_eq!(s.deposes.len(), 8);
}

#[test]
fn reseau_perdu_au_milieu_les_dossiers_deja_faits_restent_a_l_index() {
    let r = monde("hors-ligne-milieu");
    let mut d = FauxDrive::nouveau();
    d.pannes.push(Panne { operation: "lister", motif: "sauvegarde/paies".into(), erreur: ErreurGoogle::google("Reseau injoignable : timed out") });
    let s = passer(&r, &mut d);
    assert!(s.hors_ligne);
    assert_eq!(s.deposes.iter().map(|f| f.relatif.as_str()).collect::<Vec<_>>(), ["notes.txt", "config/entreprise.json", "config/avant/ancien.json", "journal/h.md"]);
    let i = index_json(&r);
    assert_eq!(i["fichiers"].as_object().unwrap().len(), 0, "rien de ce qui reste, ni de ce qui est passe, n'entre a l'index d'une passe hors ligne");
}

#[test]
fn erreur_de_dossier_incident_et_les_autres_continuent() {
    let r = monde("incident-dossier");
    let mut d = FauxDrive::nouveau();
    d.pannes.push(Panne { operation: "lister", motif: "sauvegarde/journal".into(), erreur: ErreurGoogle::google("HTTP 500 : Backend Error") });
    let s = passer(&r, &mut d);
    assert_eq!(s.incidents, ["journal : HTTP 500 : Backend Error"]);
    assert_eq!(s.deposes.len(), 7);
    assert!(!s.hors_ligne);
    let i = index_json(&r);
    assert_eq!(i["fichiers"].as_object().unwrap().len(), 7);
    assert!(i["fichiers"].get("journal/h.md").is_none());
}

#[test]
fn erreur_de_fichier_incident_pas_a_l_index_les_autres_partent() {
    let r = monde("incident-fichier");
    let mut d = FauxDrive::nouveau();
    d.pannes.push(Panne { operation: "televerser", motif: "entreprise.json".into(), erreur: ErreurGoogle::google("HTTP 403 : quota") });
    let s = passer(&r, &mut d);
    assert_eq!(s.incidents, ["config/entreprise.json : HTTP 403 : quota"]);
    assert_eq!(s.deposes.len(), 7);
    assert!(index_json(&r)["fichiers"].get("config/entreprise.json").is_none());
    // Pas « rien n'a bouge » : un fichier manque a l'index.
    assert!(!rien_n_a_bouge(&r, RD));
}

// ------------------------------------------------------------ tickets #231 et #232 : ce qui est passager, ce qui persiste

/// Une erreur que Google dit passagere (un 500 que le client a deja retente).
fn erreur_500() -> ErreurGoogle {
    let mut e = ErreurGoogle::google("HTTP 500 : Internal Error");
    e.statut = Some(500);
    e
}

/// `AAAA-MM-JJ HH:MM` de l'heure de Paris, il y a `secondes`.
fn il_y_a(secondes: i64) -> String {
    paie::releve_retours::iso_de_naif(paie::releve_retours::maintenant_naif() - secondes)[..16].replacen('T', " ", 1)
}

/// `JJ/MM` de l'heure de Paris, il y a `secondes`.
fn jour_il_y_a(secondes: i64) -> String {
    let iso = il_y_a(secondes);
    format!("{}/{}", &iso[8..10], &iso[5..7])
}

#[test]
fn un_dossier_reporte_une_fois_se_tait_et_repart_au_passage_suivant() {
    let r = monde("passager");
    let mut d = FauxDrive::nouveau();
    d.pannes.push(Panne { operation: "lister", motif: "sauvegarde/journal".into(), erreur: erreur_500() });
    let s = passer(&r, &mut d);
    assert_eq!(s.reportes.len(), 1);
    assert_eq!((s.reportes[0].quoi.as_str(), s.reportes[0].dossier, s.reportes[0].cause.as_str()), ("journal", true, "Google indisponible"));
    assert_eq!(s.persistant, None, "un dossier reporte une fois n'est pas un incident a montrer");
    assert_eq!(sauvegarde::souci(&r), None);
    // La ligne de la sauvegarde dit ce qui est vrai, positivement.
    assert_eq!(s.resume(), format!("Sauvegarde : 7 fichier(s) envoye(s), {} o, 0 inchange(s) ; 1 dossier reporte au prochain passage.", s.octets_deposes));
    let i = index_json(&r);
    assert_eq!(i["echecs"]["journal/"]["passages"], json!(1));
    assert_eq!(i["echecs"]["journal/"]["cause"], json!("Google indisponible"));
    // Le passage suivant passe : plus rien n'est reporte, l'index redevient celui d'une sauvegarde complete.
    d.pannes.clear();
    let s = passer(&r, &mut d);
    assert!(s.reportes.is_empty() && s.persistant.is_none());
    assert_eq!(s.deposes.iter().map(|f| f.relatif.as_str()).collect::<Vec<_>>(), ["journal/h.md"]);
    let i = index_json(&r);
    assert!(i.get("echecs").is_none() && i.get("derniere_complete").is_none(), "{}", i);
}

#[test]
fn le_meme_dossier_en_echec_trois_passages_de_suite_se_dit() {
    let r = monde("persiste");
    let mut d = FauxDrive::nouveau();
    d.pannes.push(Panne { operation: "lister", motif: "sauvegarde/journal".into(), erreur: erreur_500() });
    assert_eq!(passer(&r, &mut d).persistant, None);
    let premier = index_json(&r)["echecs"]["journal/"]["depuis"].clone();
    assert_eq!(passer(&r, &mut d).persistant, None);
    let s = passer(&r, &mut d);
    assert_eq!(index_json(&r)["echecs"]["journal/"]["depuis"], premier, "depuis le premier echec");
    let heure = &premier.as_str().unwrap()[11..16];
    assert_eq!(
        s.persistant.as_deref(),
        Some(format!("Sauvegarde : un dossier n'a pas pu être envoyé depuis {} (Google indisponible) ; le logiciel réessaie.", heure).as_str())
    );
    assert_eq!(sauvegarde::souci(&r), s.persistant, "la meme phrase, relue sans reseau");
    assert_eq!(index_json(&r)["echecs"]["journal/"]["passages"], json!(3));
    // Google revient : la phrase s'en va d'elle-meme.
    d.pannes.clear();
    assert_eq!(passer(&r, &mut d).persistant, None);
    assert_eq!(sauvegarde::souci(&r), None);
}

#[test]
fn un_echec_depuis_hier_se_dit_des_le_passage_suivant() {
    let r = monde("depuis-hier");
    let mut d = FauxDrive::nouveau();
    passer(&r, &mut d);
    // L'index dit : le dossier `journal` est en echec depuis hier, a la meme heure (un seul passage : la veille ne tournait pas).
    let mut i = index_json(&r);
    i["echecs"] = json!({"journal/": {"depuis": il_y_a(86_400), "passages": 1, "cause": "Google indisponible"}});
    std::fs::write(r.join("config/.sauvegarde.json"), i.to_string()).unwrap();
    d.pannes.push(Panne { operation: "lister", motif: "sauvegarde/journal".into(), erreur: erreur_500() });
    let s = passer(&r, &mut d);
    assert_eq!(
        s.persistant.as_deref(),
        Some("Sauvegarde : un dossier n'a pas pu être envoyé depuis hier (Google indisponible) ; le logiciel réessaie.")
    );
}

#[test]
fn une_journee_sans_sauvegarde_complete_se_dit() {
    let r = monde("journee");
    let mut d = FauxDrive::nouveau();
    passer(&r, &mut d);
    // La derniere sauvegarde complete date d'avant-hier ; depuis, rien n'est passe (le poste etait eteint).
    let mut i = index_json(&r);
    i["quand"] = json!(il_y_a(2 * 86_400));
    std::fs::write(r.join("config/.sauvegarde.json"), i.to_string()).unwrap();
    d.pannes.push(Panne { operation: "televerser", motif: "entreprise.json".into(), erreur: erreur_500() });
    ecrire(&r, "config/entreprise.json", "{\"a\": 2}");
    let s = passer(&r, &mut d);
    // Le fichier n'echoue que depuis ce passage : c'est la journee sans sauvegarde complete qui se dit.
    assert_eq!(
        s.persistant.as_deref(),
        Some(format!("Sauvegarde : un fichier n'a pas pu être envoyé (Google indisponible), pas de sauvegarde complète depuis le {} ; le logiciel réessaie.", jour_il_y_a(2 * 86_400)).as_str())
    );
    // Les deux cles ne restent que tant qu'un echec reste.
    assert_eq!(index_json(&r)["derniere_complete"], json!(il_y_a(2 * 86_400)));
    d.pannes.clear();
    let s = passer(&r, &mut d);
    assert_eq!(s.persistant, None);
    assert!(index_json(&r).get("derniere_complete").is_none());
}

#[test]
fn un_passage_hors_ligne_ne_compte_pas_et_garde_les_echecs() {
    let r = monde("hors-ligne-echecs");
    let mut d = FauxDrive::nouveau();
    d.pannes.push(Panne { operation: "lister", motif: "sauvegarde/journal".into(), erreur: erreur_500() });
    passer(&r, &mut d);
    d.pannes.clear();
    d.pannes.push(Panne { operation: "chemin", motif: "".into(), erreur: ErreurGoogle::google("Reseau injoignable : [Errno 111] Connection refused") });
    let s = passer(&r, &mut d);
    assert!(s.hors_ligne);
    assert_eq!(index_json(&r)["echecs"]["journal/"]["passages"], json!(1), "le passage hors ligne n'a rien essaye");
}

#[test]
fn google_absent_le_reste_du_passage_est_reporte_sans_appel() {
    let r = monde("google-absent");
    let mut d = FauxDrive::nouveau();
    // Tout depot rend un 500 (deja retente par le client) : apres trois de suite, on n'appelle plus.
    d.pannes.push(Panne { operation: "televerser", motif: "".into(), erreur: erreur_500() });
    let s = passer(&r, &mut d);
    assert_eq!(d.appels("televerser"), 3, "{:?}", d.journal);
    assert!(d.appels("chemin PuppetPaie/sauvegarde/journal") == 0, "les dossiers suivants ne sont pas appeles : {:?}", d.journal);
    // Trois fichiers essayes, puis cinq dossiers (d'un fichier chacun) reportes d'office.
    assert_eq!((s.reportes.iter().filter(|x| !x.dossier).count(), s.reportes.iter().filter(|x| x.dossier).count()), (3, 5));
    assert!(s.deposes.is_empty());
    assert!(s.reportes.iter().all(|x| x.cause == "Google indisponible"));
    assert_eq!(s.persistant, None);
    // Le prochain passage reprend tout : rien n'est entre a l'index.
    assert_eq!(index_json(&r)["fichiers"].as_object().unwrap().len(), 0);
}

#[test]
fn les_causes_en_clair() {
    use paie::sauvegarde::cause_de;
    assert_eq!(cause_de(&erreur_500()), "Google indisponible");
    let mut e = ErreurGoogle::google("HTTP 429 : Rate Limit Exceeded");
    e.statut = Some(429);
    assert_eq!(cause_de(&e), "Google indisponible");
    assert_eq!(cause_de(&erreur("TimeoutError", "The read operation timed out")), "Google ne répond pas");
    let mut e = ErreurGoogle::google("HTTP 403 : The user's Drive storage quota has been exceeded.");
    e.statut = Some(403);
    assert_eq!(cause_de(&e), "Google refuse l'envoi");
    assert_eq!(cause_de(&erreur("PermissionError", "Permission refusee : x")), "fichier illisible sur ce poste");
}

#[test]
fn fichier_disparu_entre_l_empreinte_et_l_envoi_ni_incident_ni_index() {
    let r = monde("disparu-envoi");
    let mut d = FauxDrive::nouveau();
    // L'empreinte est lue, puis le fichier disparait : le faux l'efface a `televerser`, avant de le lire (`FileNotFoundError`).
    d.effacer_au_televerser.push(("vieux.json".into(), r.join("paies/avant/vieux.json")));
    let s = passer(&r, &mut d);
    assert_eq!(s.disparus, 1);
    assert!(s.incidents.is_empty(), "{:?}", s.incidents);
    assert_eq!(s.deposes.len(), 7);
    assert!(index_json(&r)["fichiers"].get("paies/avant/vieux.json").is_none());
    // Le passage suivant ne le voit plus du tout : tout le reste est identique.
    let s = passer(&r, &mut d);
    assert_eq!((s.deposes.len(), s.identiques, s.disparus), (0, 7, 0));
}

#[test]
fn fichier_disparu_apres_l_inventaire_avant_l_empreinte() {
    let r = monde("disparu-empreinte");
    let mut d = FauxDrive::nouveau();
    // Le fichier est dans l'inventaire ; le faux l'efface pendant le listage de son dossier : l'empreinte ne peut plus le lire.
    d.effacer_au_lister.push(("sauvegarde/journal".into(), r.join("journal/h.md")));
    let s = passer(&r, &mut d);
    assert_eq!((s.disparus, s.deposes.len()), (1, 7));
    assert!(s.incidents.is_empty());
    assert_eq!(d.appels("televerser PuppetPaie/sauvegarde/journal"), 0);
    assert!(index_json(&r)["fichiers"].get("journal/h.md").is_none());
}

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

#[test]
fn rien_n_a_bouge_au_moindre_doute_non() {
    let r = monde("rien-n-a-bouge");
    // Pas d'index : non.
    assert!(!rien_n_a_bouge(&r, RD));
    let mut d = FauxDrive::nouveau();
    passer(&r, &mut d);
    assert!(rien_n_a_bouge(&r, RD));
    // Un fichier change de date : non.
    fixer_date(&r.join("notes.txt"), 1_700_000_001, 0);
    assert!(!rien_n_a_bouge(&r, RD));
    fixer_date(&r.join("notes.txt"), 1_700_000_000, 0);
    assert!(rien_n_a_bouge(&r, RD));
    // De taille (meme date) : non.
    ecrire(&r, "notes.txt", "notes!");
    assert!(!rien_n_a_bouge(&r, RD));
    ecrire(&r, "notes.txt", "notes");
    assert!(rien_n_a_bouge(&r, RD));
    // Un fichier apparu : non. Un fichier ecarte apparu : oui (il ne compte pas).
    ecrire(&r, "nouveau.txt", "1");
    assert!(!rien_n_a_bouge(&r, RD));
    std::fs::remove_file(r.join("nouveau.txt")).unwrap();
    ecrire(&r, "config/x.tmp", "1");
    assert!(rien_n_a_bouge(&r, RD));
    // Un fichier disparu : non.
    std::fs::remove_file(r.join("journal/h.md")).unwrap();
    assert!(!rien_n_a_bouge(&r, RD));
}

#[test]
fn rien_n_a_bouge_ne_croit_pas_un_index_aux_formes_etranges() {
    let r = racine("rien-formes");
    ecrire(&r, "a.txt", "12");
    std::fs::create_dir_all(r.join("config")).unwrap();
    let ecrire_index = |i: Value| std::fs::write(r.join("config/.sauvegarde.json"), i.to_string()).unwrap();
    ecrire_index(json!({"racine_drive": RD, "fichiers": {"a.txt": {"octets": "2", "mtime": 1700000000}}}));
    assert!(!rien_n_a_bouge(&r, RD));
    ecrire_index(json!({"racine_drive": RD, "fichiers": {"a.txt": {}}}));
    assert!(!rien_n_a_bouge(&r, RD));
    ecrire_index(json!({"racine_drive": RD, "fichiers": {}}));
    assert!(!rien_n_a_bouge(&r, RD));
    ecrire_index(json!({"racine_drive": RD, "fichiers": []}));
    assert!(!rien_n_a_bouge(&r, RD));
    ecrire_index(json!({"racine_drive": RD}));
    assert!(!rien_n_a_bouge(&r, RD));
}

#[test]
fn derniere_rend_l_index_tel_qu_il_est() {
    let r = racine("derniere");
    assert_eq!(derniere(&r).en_json(), json!({"quand": "", "fichiers": 0, "racine_drive": ""}));
    std::fs::create_dir_all(r.join("config")).unwrap();
    let ecrire_index = |t: &str| std::fs::write(r.join("config/.sauvegarde.json"), t).unwrap();
    ecrire_index(r#"{"quand": "2026-10-06 21:04", "racine_drive": "PuppetPaie", "fichiers": {"a": {}, "b": {}, "c": {}}}"#);
    assert_eq!(derniere(&r).en_json(), json!({"quand": "2026-10-06 21:04", "fichiers": 3, "racine_drive": "PuppetPaie"}));
    // Les valeurs d'un index forge a la main sont rendues telles quelles (pas forcees en texte).
    ecrire_index(r#"{"quand": 5, "racine_drive": null, "fichiers": "abc"}"#);
    assert_eq!(derniere(&r).en_json(), json!({"quand": 5, "fichiers": 3, "racine_drive": null}));
    ecrire_index(r#"{"fichiers": null}"#);
    assert_eq!(derniere(&r).en_json(), json!({"quand": "", "fichiers": 0, "racine_drive": ""}));
    ecrire_index(r#"{"fichiers": [1, 2]}"#);
    assert_eq!(derniere(&r).fichiers, 2);
}

#[test]
fn derniere_apres_une_passe() {
    let r = monde("derniere-passe");
    let mut d = FauxDrive::nouveau();
    passer(&r, &mut d);
    let dn = derniere(&r);
    assert_eq!(dn.fichiers, 8);
    assert_eq!(dn.racine_drive, json!("PuppetPaie"));
    assert_eq!(dn.quand.as_str().unwrap().len(), 16);
}

// ------------------------------------------------------------ l'index qui n'est pas un objet JSON

#[test]
fn un_index_corrompu_ou_qui_n_est_pas_un_objet_json_se_lit_comme_absent() {
    // Le Python plantait (AttributeError) sur `[]`, `5`, `null`, `"x"` et la veille entiere avec lui ; le Rust le lit comme absent.
    for (i, contenu) in ["[]", "5", "null", "\"x\"", "true", "[{\"fichiers\": {}}]", "{ pas du json", "", "{\"fichiers\": {\"a\"", "\u{feff}{}"].iter().enumerate() {
        let r = monde(&format!("corrompu-{}", i));
        std::fs::create_dir_all(r.join("config")).unwrap();
        std::fs::write(r.join("config/.sauvegarde.json"), contenu).unwrap();
        assert_eq!(derniere(&r).en_json(), json!({"quand": "", "fichiers": 0, "racine_drive": ""}), "index {:?}", contenu);
        assert!(!rien_n_a_bouge(&r, RD), "index {:?}", contenu);
        // Et la sauvegarde repart de zero, sans erreur, et reecrit un index valide.
        let mut d = FauxDrive::nouveau();
        let s = passer(&r, &mut d);
        assert_eq!(s.deposes.len(), 8, "index {:?}", contenu);
        assert!(index_json(&r).is_object());
        assert!(rien_n_a_bouge(&r, RD));
    }
}

#[test]
fn un_index_dont_dossiers_n_est_pas_un_objet_est_ignore() {
    let r = monde("dossiers-bizarres");
    std::fs::create_dir_all(r.join("config")).unwrap();
    std::fs::write(r.join("config/.sauvegarde.json"), r#"{"dossiers": [1, 2], "fichiers": {}}"#).unwrap();
    let mut d = FauxDrive::nouveau();
    let s = passer(&r, &mut d);
    assert_eq!(s.deposes.len(), 8);
    assert_eq!(d.appels("chemin"), 8);
}

#[test]
fn les_identifiants_de_dossiers_de_l_index_servent_et_un_identifiant_vide_non() {
    let r = racine("ids-index");
    ecrire(&r, "notes.txt", "1");
    let mut d = FauxDrive::nouveau();
    let id = d.chemin(&["PuppetPaie", "sauvegarde"]).unwrap();
    d.journal.clear();
    std::fs::create_dir_all(r.join("config")).unwrap();
    std::fs::write(r.join("config/.sauvegarde.json"), json!({"dossiers": {"PuppetPaie/sauvegarde": id}}).to_string()).unwrap();
    passer(&r, &mut d);
    assert_eq!(d.appels("chemin"), 0);
    std::fs::write(r.join("config/.sauvegarde.json"), json!({"dossiers": {"PuppetPaie/sauvegarde": ""}}).to_string()).unwrap();
    passer(&r, &mut d);
    assert_eq!(d.appels("chemin"), 1);
}

// ------------------------------------------------------------ le compte rendu

#[test]
fn le_compte_rendu_resume_et_json() {
    let mut s = Sauvegarde {
        racine_drive: RD.into(),
        compte: None,
        deposes: vec![
            Fichier { relatif: "a/b.json".into(), octets: 2048, etat: EtatFichier::Neuf },
            Fichier { relatif: "c.md".into(), octets: 1_572_864, etat: EtatFichier::Modifie },
        ],
        identiques: 3,
        octets_deposes: 2048 + 1_572_864,
        exclus: 0,
        disparus: 0,
        secondes: 0.0,
        incidents: vec![],
        reportes: vec![],
        persistant: None,
        hors_ligne: false,
    };
    assert_eq!(s.resume(), "Sauvegarde : 2 fichier(s) envoye(s), 1.5 Mo, 3 inchange(s).");
    assert_eq!(s.en_json(), json!({"resume": s.resume(), "deposes": ["a/b.json", "c.md"], "identiques": 3, "octets": 1_574_912}));
    s.deposes.truncate(0);
    assert_eq!(s.resume(), "Sauvegarde a jour (3 fichiers).");
    s.hors_ligne = true;
    assert_eq!(s.resume(), "Hors connexion : sauvegarde au prochain passage.");
    assert_eq!((EtatFichier::Neuf.texte(), EtatFichier::Modifie.texte()), ("neuf", "modifie"));
    let t = Sauvegarde { deposes: vec![Fichier { relatif: "x".into(), octets: 5, etat: EtatFichier::Neuf }], octets_deposes: 5, ..s.clone() };
    let mut t = Sauvegarde { hors_ligne: false, ..t };
    assert_eq!(t.resume(), "Sauvegarde : 1 fichier(s) envoye(s), 5 o, 3 inchange(s).");
    t.octets_deposes = 1024;
    assert!(t.resume().contains("1 Ko"));
}

#[test]
fn hors_ligne_reconnait_le_reseau_absent_et_pas_le_delai() {
    // Un delai de lecture n'arrete pas toute la sauvegarde (defaut 2) ; un delai de connexion est « injoignable ».
    for (message, hors) in [
        ("The read operation timed out", false),
        ("timed out", false),
        ("HTTP 404 : File not found", false),
        ("", false),
        ("Reseau injoignable : timed out", true),
        ("Reseau injoignable : Connection refused", true),
        ("failed to lookup address information: Name or service not known", true),
        ("Network is unreachable (os error 101)", true),
    ] {
        assert_eq!(sauvegarde::hors_ligne(message), hors, "{:?}", message);
    }
}

#[test]
fn md5_de_reference_du_faux_est_celui_de_drive() {
    assert_eq!(md5_de(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
}
