//! La restauration du dossier prive depuis le Drive (`restauration.rs`, lot R1) contre le FAUX `Drive` en memoire
//! (`tests/faux_drive`) : l'index reconstruit
//! laisse la sauvegarde suivante sans rien a faire ; le dossier qui n'est pas vide (refus, ou accepte s'il ne porte que le
//! jeton ou des copies identiques, d'ou la reprise apres une coupure) ; l'empreinte qui ne colle pas ; ce qui ne redescend
//! pas ; et ce qu'il reste a faire, coffre compris (un coffre invente, sur deux postes inventes).
//!
//! AUCUN RESEAU, AUCUN GOOGLE, jamais `prive/` ni `fictives/` vivante.

use crate::faux_drive;
use crate::outils::{chemin_unique, dossier_de_test};

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use faux_drive::{ecrire, FauxDrive, Panne};
use paie::coffre::{oublier_mot_de_passe_tape, retenir_mot_de_passe_tape, Coffre, CONSULTATION};
use paie::drive::{Drive, ErreurGoogle};
use paie::maj::{generer_cle_au_poste, signature_valide, signer_par_la_cle};
use paie::restauration::{a_faire, restaurer};
use paie::sauvegarde::{inventaire, rien_n_a_bouge, sauvegarder};
use serde_json::json;

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

/// Ce que la sauvegarde porte d'un dossier : chemin relatif -> octets.
fn contenu(r: &Path) -> BTreeMap<String, Vec<u8>> {
    inventaire(r).0.into_iter().map(|(rel, _)| (rel.clone(), std::fs::read(r.join(&rel)).unwrap())).collect()
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


#[test]
fn pas_de_sauvegarde_dans_le_drive_refuse_sans_rien_creer_ni_ecrire() {
    let cible = dossier("absente").join("prive");
    let mut d = FauxDrive::nouveau();
    let e = restaurer(&cible, RD, &mut d).unwrap_err();
    assert_eq!(e.classe, "SauvegardeAbsente");
    assert!(e.message.contains("PuppetPaie/sauvegarde est introuvable") && e.message.contains("compte@exemple.invalid"), "{}", e.message);
    assert_eq!(d.nombre_de_dossiers(), 0);
    assert!(!cible.exists());
    // La racine sans `sauvegarde/` (un Drive qui n'a que des archives de mois) : de meme.
    d.deposer("PuppetPaie/2026-06", "bulletins.pdf", b"%PDF", true);
    assert_eq!(restaurer(&cible, RD, &mut d).unwrap_err().classe, "SauvegardeAbsente");
    assert!(!cible.exists());
}

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

#[test]
fn un_dossier_qui_ne_porte_que_le_jeton_et_des_copies_identiques_est_accepte_sans_rien_remplacer() {
    let base = dossier("jeton");
    let source = base.join("source");
    petite(&source);
    let mut d = FauxDrive::nouveau();
    sauvegarder_dans(&source, &mut d);
    // Le poste neuf a relie son Drive (le jeton est la), et le `google.json` d'avant a ete recopie a la main.
    let cible = base.join("cible");
    ecrire(&cible, "config/google-jeton.bin", "AUTRE-JETON-INVENTE");
    ecrire(&cible, "config/.compteurs.verrou", "x");
    ecrire(&cible, "config/entreprise.json", "{\"a\": 1}");
    let r = restaurer(&cible, RD, &mut d).unwrap();
    assert!(r.incidents.is_empty(), "{:?}", r.incidents);
    assert_eq!((r.redescendus.len(), r.deja_la), (5, 1));
    assert!(r.redescendus.iter().all(|f| f.relatif != "config/entreprise.json"));
    assert_eq!(contenu(&cible), contenu(&source));
    // Le jeton de ce poste est le sien : ni remplace, ni dans l'index.
    assert_eq!(std::fs::read_to_string(cible.join("config/google-jeton.bin")).unwrap(), "AUTRE-JETON-INVENTE");
    assert!(rien_n_a_bouge(&cible, RD));
    assert!(!std::fs::read_to_string(cible.join("config/.sauvegarde.json")).unwrap().contains("google-jeton"));
}

#[test]
fn une_restauration_coupee_par_le_reseau_se_relance_et_reprend() {
    let base = dossier("reprise");
    let source = base.join("source");
    petite(&source);
    let mut d = FauxDrive::nouveau();
    sauvegarder_dans(&source, &mut d);
    let cible = base.join("cible");
    // L'ordre est celui des chemins : `config/`, `journal/`, `notes.txt`, `paies/`... le reseau tombe sur `notes.txt`.
    d.pannes.push(Panne { operation: "telecharger", motif: "notes.txt".into(), erreur: ErreurGoogle::google("Reseau injoignable : getaddrinfo failed") });
    let r = restaurer(&cible, RD, &mut d).unwrap();
    assert!(r.hors_ligne);
    assert_eq!(r.redescendus.iter().map(|f| f.relatif.as_str()).collect::<Vec<_>>(), ["config/entreprise.json", "journal/depots.json"]);
    assert!(r.resume().contains("relancer `paie restaurer` reprend"), "{}", r.resume());
    assert_eq!(d.appels("telecharger"), 3, "arretee au premier fichier hors connexion : {:?}", d.journal);
    // L'index ne porte que ce qui est la et verifie.
    let index: serde_json::Value = serde_json::from_slice(&std::fs::read(cible.join("config/.sauvegarde.json")).unwrap()).unwrap();
    assert_eq!(index["fichiers"].as_object().unwrap().keys().cloned().collect::<Vec<_>>(), ["config/entreprise.json", "journal/depots.json"]);
    // Le reseau revient : la meme commande reprend, sans retelecharger ce qui est deja la.
    d.pannes.clear();
    d.journal.clear();
    let r = restaurer(&cible, RD, &mut d).unwrap();
    assert!(r.incidents.is_empty() && !r.hors_ligne, "{:?}", r.incidents);
    assert_eq!((r.redescendus.len(), r.deja_la), (4, 2));
    assert_eq!(d.appels("telecharger"), 4);
    assert_eq!(contenu(&cible), contenu(&source));
    assert!(rien_n_a_bouge(&cible, RD));
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

#[test]
fn ce_que_la_sauvegarde_n_envoie_pas_un_nom_qui_sortirait_et_un_document_sans_empreinte_ne_redescendent_pas() {
    let base = dossier("ecartes");
    let source = base.join("source");
    petite(&source);
    let mut d = FauxDrive::nouveau();
    sauvegarder_dans(&source, &mut d);
    d.deposer("PuppetPaie/sauvegarde/config", "google-jeton.bin", b"JETON-INVENTE", true);
    d.deposer("PuppetPaie/sauvegarde/paies", "brouillon.tmp", b"t", true);
    d.deposer("PuppetPaie/sauvegarde/config", "..", b"evasion", true);
    d.deposer("PuppetPaie/sauvegarde", "a\\b.txt", b"evasion", true);
    d.deposer("PuppetPaie/sauvegarde/journal", "note Google", b"", false);
    let cible = base.join("poste").join("prive");
    let r = restaurer(&cible, RD, &mut d).unwrap();
    assert_eq!(r.ecartes, 2);
    assert_eq!(r.redescendus.len(), 6);
    // Un incident par nom refuse, qui le nomme.
    let mut incidents = r.incidents.clone();
    incidents.sort();
    assert_eq!(incidents.len(), 3, "{:?}", incidents);
    for (i, nom) in ["a\\b.txt :", "config/.. :", "journal/note Google :"].iter().enumerate() {
        assert!(incidents[i].starts_with(nom), "{:?}", incidents);
    }
    assert_eq!(contenu(&cible), contenu(&source));
    assert!(!cible.join("config/google-jeton.bin").exists() && !base.join("poste").join("evasion").exists());
    assert!(tout(&base.join("poste")).iter().all(|f| f.starts_with("prive/")), "{:?}", tout(&base.join("poste")));
}

#[test]
fn des_dossiers_homonymes_ne_se_choisissent_pas() {
    let base = dossier("homonymes");
    let source = base.join("source");
    petite(&source);
    let mut d = FauxDrive::nouveau();
    sauvegarder_dans(&source, &mut d);
    // Plus bas : un incident, et le reste revient.
    d.dossier_homonyme("PuppetPaie/sauvegarde/paies");
    let cible = base.join("cible");
    let r = restaurer(&cible, RD, &mut d).unwrap();
    assert_eq!(r.incidents.len(), 1);
    assert!(r.incidents[0].starts_with("paies/ :"), "l'incident nomme le dossier : {}", r.incidents[0]);
    assert_eq!(r.redescendus.len(), 4);
    assert!(!cible.join("paies").exists());
    // A la racine : un refus, rien d'ecrit.
    d.dossier_homonyme("PuppetPaie");
    let ailleurs = base.join("ailleurs");
    let e = restaurer(&ailleurs, RD, &mut d).unwrap_err();
    assert_eq!(e.classe, "SauvegardeAmbigue");
    assert!(e.message.starts_with("2 dossiers « PuppetPaie » dans le Drive"), "{}", e.message);
    assert!(!ailleurs.exists());
}

// ------------------------------------------------------------ ce qu'il reste a faire : le coffre

/// Le coffre de `racine` vu d'un poste dont la cle est dans `poste`.
fn coffre(racine: &Path, poste: &Path) -> Coffre {
    std::fs::create_dir_all(poste).unwrap();
    Coffre { racine: racine.to_path_buf(), dossier_cle: poste.to_path_buf(), maintenant: "2026-10-03T14:05".into(), variables: BTreeMap::new() }
}

/// Une petite compagnie au coffre fait sur le poste A (`mot_de_passe` : avec l'enveloppe du mot de passe du logiciel),
/// sauvegardee puis restauree sur le poste B. `coffre.bin` monte (Leandre, 13/10/2026, redit le 14/10) et redescend
/// octet pour octet : la suite prouve ce qu'il reste a faire, et que le mot de passe du logiciel l'ouvre au poste B.
fn coffre_restaure(test: &str, mot_de_passe: bool) -> Coffre {
    let base = dossier(test);
    let source = base.join("source");
    petite(&source);
    let a = coffre(&source, &chemin_unique("poste-a"));
    if mot_de_passe {
        a.poser_ouverture("sesame-123", None).unwrap();
    }
    a.poser("net-entreprises", "unvraimotdepasse", None).unwrap();
    let mut d = FauxDrive::nouveau();
    sauvegarder_dans(&source, &mut d);
    let cible = base.join("poste-b");
    let r = restaurer(&cible, RD, &mut d).unwrap();
    assert!(r.incidents.is_empty(), "{:?}", r.incidents);
    assert!(d.arbre().keys().any(|k| k.ends_with("config/coffre.bin")), "le coffre monte");
    let b = coffre(&cible, &chemin_unique("poste-b"));
    assert_eq!(std::fs::read(b.chemin()).unwrap(), std::fs::read(a.chemin()).unwrap(), "le coffre redescend tel quel");
    b
}

#[test]
fn le_coffre_redescendu_s_ouvre_ailleurs_par_le_mot_de_passe_du_logiciel_et_le_dit() {
    let b = coffre_restaure("coffre", true);
    // Ce qu'il reste a faire : ce que le coffre porte, comment il s'ouvre ici, ce qui manque. Jamais une valeur.
    let lignes = a_faire(&b);
    assert!(lignes[0].contains("net-entreprises (pose le 2026-10-03 14:05)"), "ce que le coffre porte : {}", lignes[0]);
    assert!(lignes[1].contains("`paie coffre ouverture`"), "la commande qui l'ouvre ici : {}", lignes[1]);
    assert_eq!(&lignes[3..], ["  paie coffre poser urssaf", "  paie coffre poser courriel --compte <adresse>"], "ce qui manque");
    assert_eq!(lignes.len(), 5);
    assert!(!lignes.join("\n").contains("unvraimotdepasse"));
    // Et c'est vrai : la cle du poste B n'ouvre pas, le mot de passe du logiciel ouvre, et le reposer rend une cle a B.
    assert!(!b.ouvre_a_ce_poste().unwrap());
    let lire = |c: &Coffre, mdp: Option<&str>| c.lire(&json!("net-entreprises"), &json!(CONSULTATION), false, mdp).map_err(|e| e.message);
    assert!(lire(&b, None).is_err());
    assert_eq!(lire(&b, Some("sesame-123")).unwrap().as_deref(), Some("unvraimotdepasse"));
    b.poser_ouverture("sesame-123", Some("sesame-123")).unwrap();
    assert!(b.ouvre_a_ce_poste().unwrap());
    assert_eq!(lire(&b, None).unwrap().as_deref(), Some("unvraimotdepasse"));
    assert!(!a_faire(&b).join("\n").contains("paie coffre ouverture"), "la cle de ce poste l'ouvre : plus rien a ouvrir");
}

#[test]
fn un_coffre_sans_mot_de_passe_du_logiciel_ne_s_ouvrira_pas_ailleurs_et_le_dit() {
    let lignes = a_faire(&coffre_restaure("coffre-sans", false));
    assert!(lignes[0].contains("net-entreprises (pose le 2026-10-03 14:05)") && lignes[0].contains("ne s'ouvrira pas"), "{}", lignes[0]);
    assert_eq!(&lignes[1..], ["  paie coffre poser net-entreprises", "  paie coffre poser urssaf", "  paie coffre poser courriel --compte <adresse>"]);
}

#[test]
fn sans_coffre_dans_la_sauvegarde_tout_est_a_reposer_et_la_cle_de_signature_se_signale() {
    let base = dossier("sans-coffre");
    let source = base.join("source");
    petite(&source);
    ecrire(&source, "config/maj-cle-privee.bin", "CLE-INVENTEE");
    let mut d = FauxDrive::nouveau();
    sauvegarder_dans(&source, &mut d);
    let cible = base.join("cible");
    restaurer(&cible, RD, &mut d).unwrap();
    let lignes = a_faire(&coffre(&cible, &chemin_unique("poste-b")));
    assert!(lignes[0].contains("`paie coffre ouverture`"), "{}", lignes[0]);
    assert_eq!(
        lignes[1..4],
        [
            "  paie coffre poser net-entreprises",
            "  paie coffre poser urssaf",
            "  paie coffre poser courriel --compte <adresse>",
        ]
    );
    assert!(lignes[4].starts_with("config/maj-cle-privee.bin") && lignes[4].contains("ne s'ouvre pas"), "scellee par la seule cle du poste A : {}", lignes[4]);
    assert_eq!(lignes.len(), 5);
}

// ------------------------------------------------------------ ce qu'il reste a faire : la cle de signature

/// Le poste A de l'editeur : une petite compagnie, `config/maj.json` editeur, le mot de passe du logiciel, une cle de
/// signature scellee par le coffre (lot K1). Sauvegardee, puis restauree sur le poste B. Rend (A, B, la cle publique).
fn cle_restauree(test: &str) -> (Coffre, Coffre, String) {
    let base = dossier(test);
    let source = base.join("source");
    petite(&source);
    ecrire(&source, "config/maj.json", "{\"editeur\": true}");
    let a = coffre(&source, &chemin_unique("poste-a"));
    a.poser_ouverture("sesame-123", None).unwrap();
    let publique = generer_cle_au_poste(&a, false).unwrap()["cle_publique"].as_str().unwrap().to_string();
    let mut d = FauxDrive::nouveau();
    sauvegarder_dans(&source, &mut d);
    assert!(d.arbre().keys().any(|k| k.ends_with("config/maj-cle-privee.bin")), "la cle de signature monte");
    let cible = base.join("poste-b");
    let r = restaurer(&cible, RD, &mut d).unwrap();
    assert!(r.incidents.is_empty(), "{:?}", r.incidents);
    assert!(cible.join("config/maj-cle-privee.bin").exists());
    (a, coffre(&cible, &chemin_unique("poste-b")), publique)
}

#[test]
fn la_cle_de_signature_redescend_et_s_ouvre_ailleurs_par_le_mot_de_passe_du_logiciel_avec_son_coffre() {
    let (a, b, publique) = cle_restauree("cle-et-coffre");
    // Le coffre est redescendu avec elle, tel quel.
    assert_eq!(std::fs::read(b.chemin()).unwrap(), std::fs::read(a.chemin()).unwrap());
    let lignes = a_faire(&b);
    // La cle est dite, sans commande de remplacement : elle s'ouvre avec le coffre.
    let derniere = lignes.last().unwrap();
    assert!(derniere.starts_with("config/maj-cle-privee.bin") && !derniere.contains("ne s'ouvre pas") && !derniere.contains("--remplacer"), "{}", derniere);
    // Et c'est vrai : le mot de passe du logiciel, tape pour la commande, ouvre la cle au poste B, et elle signe.
    retenir_mot_de_passe_tape("sesame-123");
    let signature = signer_par_la_cle(&b, b"une archive inventee");
    oublier_mot_de_passe_tape();
    assert!(signature_valide(b"une archive inventee", &signature.unwrap(), &publique));
}

#[test]
fn la_cle_de_signature_revenue_sans_son_coffre_ne_s_ouvre_pas_et_le_dit() {
    let (_, b, _) = cle_restauree("cle-sans-coffre");
    // Le coffre redescend avec elle ; le cas est celui ou il manque quand meme (perdu, ou ecarte a la main).
    std::fs::remove_file(b.chemin()).unwrap();
    let lignes = a_faire(&b);
    assert!(lignes[0].contains("`paie coffre ouverture`"), "sans coffre, le mot de passe du logiciel est a poser : {}", lignes[0]);
    let derniere = lignes.last().unwrap();
    assert!(derniere.starts_with("config/maj-cle-privee.bin") && derniere.contains("`paie maj --cle --remplacer`"), "la cle, et la commande qui la remplace : {}", derniere);
    retenir_mot_de_passe_tape("sesame-123");
    let refus = signer_par_la_cle(&b, b"une archive inventee");
    oublier_mot_de_passe_tape();
    assert!(refus.is_err(), "sans coffre, la cle ne signe pas");
}
