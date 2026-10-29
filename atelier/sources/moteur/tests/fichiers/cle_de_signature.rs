//! La cle qui signe les mises a jour (`config/maj-cle-privee.bin`, `moteur::compagnie::maj`) : scellee par la cle maitresse du coffre,
//! elle s'ouvre partout ou le coffre s'ouvre — la cle du poste qui l'a faite, ou le mot de passe du logiciel sur un autre
//! (lot K1, 14/10/2026 ; Leandre : « si mon ordi tombe, on pourra plus faire de mise a jour, ça me va pas »). Sur des
//! compagnies, des postes et des mots de passe inventes, chacun dans son dossier (jamais la vraie cle du poste).

use crate::outils::chemin_unique;
use moteur::compagnie::coffre::{oublier_mot_de_passe_tape, retenir_mot_de_passe_tape, Coffre};
use moteur::compagnie::maj::{generer_cle_au_poste, signature_valide, signer_par_la_cle};
use std::collections::BTreeMap;
use std::path::Path;

const LOGICIEL: &str = "sesame-123";
/// Ce que le moteur d'avant le lot K1 ecrivait : la graine scellee par la cle du poste, avec cette description (`maj.rs`).
const DESCRIPTION_CLE: &str = "PuppetPaie : cle de signature des mises a jour";
const ARCHIVE: &[u8] = b"une archive inventee";

/// Un poste (sa cle, dans un dossier a lui) qui voit la compagnie `racine`.
fn poste(racine: &Path, nom: &str) -> Coffre {
    Coffre { racine: racine.to_path_buf(), dossier_cle: chemin_unique(nom), maintenant: "2026-10-14T10:05".into(), variables: BTreeMap::new() }
}

/// La compagnie de l'editeur sur le poste A : le coffre a le mot de passe du logiciel.
fn editeur(nom: &str) -> Coffre {
    let racine = chemin_unique(nom).join("compagnie");
    std::fs::create_dir_all(racine.join("config")).unwrap();
    let a = poste(&racine, "poste-a");
    a.poser_ouverture(LOGICIEL, None).unwrap();
    a
}

/// Le dossier de la compagnie apporte sur un poste neuf, tel qu'une restauration le redescendrait : `config/` entier, coffre
/// compris. Le poste B a sa propre cle, qui n'ouvre rien de ce qui vient de A.
fn apporte(a: &Coffre, nom: &str) -> Coffre {
    let racine = chemin_unique(nom).join("compagnie");
    std::fs::create_dir_all(racine.join("config")).unwrap();
    for f in std::fs::read_dir(a.racine.join("config")).unwrap() {
        let f = f.unwrap();
        std::fs::copy(f.path(), racine.join("config").join(f.file_name())).unwrap();
    }
    poste(&racine, "poste-b")
}

fn publique(r: &serde_json::Value) -> String {
    r["cle_publique"].as_str().unwrap().to_string()
}

#[test]
fn une_cle_faite_sur_un_poste_apportee_sur_un_autre_s_ouvre_par_le_mot_de_passe_du_logiciel_et_signe() {
    let a = editeur("cle-apportee");
    let publique = publique(&generer_cle_au_poste(&a, false).unwrap());
    // Sur A, la cle du poste ouvre le coffre, donc la cle de signature : rien a taper.
    assert!(signature_valide(ARCHIVE, &signer_par_la_cle(&a, ARCHIVE).unwrap(), &publique));

    let b = apporte(&a, "cle-apportee-b");
    // Sur B, sans le mot de passe du logiciel, rien ne l'ouvre ; la commande dit par quoi elle s'ouvrirait.
    let refus = signer_par_la_cle(&b, ARCHIVE).unwrap_err();
    assert!(refus.contains("mot de passe du logiciel"), "{}", refus);
    // Un mauvais mot de passe non plus.
    retenir_mot_de_passe_tape("pas-le-bon");
    assert!(signer_par_la_cle(&b, ARCHIVE).is_err());
    // Le mot de passe du logiciel, tape pour la commande, l'ouvre ; la signature est celle de la cle publique de A.
    retenir_mot_de_passe_tape(LOGICIEL);
    let signature = signer_par_la_cle(&b, ARCHIVE);
    oublier_mot_de_passe_tape();
    assert!(signature_valide(ARCHIVE, &signature.unwrap(), &publique));
    assert!(!b.chemin_cle().exists(), "ouvrir par le mot de passe ne pose rien au poste");

    // Reposer le mot de passe du logiciel rend a B son ouverture sans mot de passe : la cle de signature suit le coffre.
    b.poser_ouverture(LOGICIEL, Some(LOGICIEL)).unwrap();
    assert!(signature_valide(ARCHIVE, &signer_par_la_cle(&b, ARCHIVE).unwrap(), &publique));
}

#[test]
fn une_cle_scellee_a_l_ancienne_s_ouvre_sur_son_poste_et_s_y_rescelle_par_le_coffre() {
    let a = editeur("cle-ancienne");
    // La cle d'avant : la graine scellee par la cle du poste A seule.
    let graine = [7u8; 32];
    let publique = socle::base64::ecrire(ed25519_dalek::SigningKey::from_bytes(&graine).verifying_key().as_bytes());
    let chemin = a.racine.join("config/maj-cle-privee.bin");
    std::fs::write(&chemin, a.proteger(&graine, DESCRIPTION_CLE).unwrap()).unwrap();
    let avant = std::fs::read(&chemin).unwrap();

    // Apportee telle quelle sur B, elle ne s'ouvre pas, meme par le mot de passe du logiciel : c'est le defaut corrige.
    let b = apporte(&a, "cle-ancienne-b");
    retenir_mot_de_passe_tape(LOGICIEL);
    assert!(signer_par_la_cle(&b, ARCHIVE).is_err());
    oublier_mot_de_passe_tape();

    // Sur A, elle s'ouvre encore et signe ; a cette ouverture, elle se rescelle par la cle maitresse du coffre. La paire
    // ne change pas : les compagnies gardent la cle publique qu'elles ont.
    assert!(signature_valide(ARCHIVE, &signer_par_la_cle(&a, ARCHIVE).unwrap(), &publique));
    assert_ne!(std::fs::read(&chemin).unwrap(), avant);
    assert!(signature_valide(ARCHIVE, &signer_par_la_cle(&a, ARCHIVE).unwrap(), &publique));

    // Desormais, apportee sur un autre poste, le mot de passe du logiciel l'ouvre.
    let c = apporte(&a, "cle-ancienne-c");
    retenir_mot_de_passe_tape(LOGICIEL);
    let signature = signer_par_la_cle(&c, ARCHIVE);
    oublier_mot_de_passe_tape();
    assert!(signature_valide(ARCHIVE, &signature.unwrap(), &publique));
}

#[test]
fn sans_mot_de_passe_du_logiciel_la_cle_ne_se_fabrique_pas_elle_serait_perdue_avec_le_poste() {
    let racine = chemin_unique("cle-sans-logiciel").join("compagnie");
    std::fs::create_dir_all(racine.join("config")).unwrap();
    let a = poste(&racine, "poste-a");
    // Un coffre qui ne porte qu'un mot de passe d'organisme : seule la cle du poste l'ouvre.
    a.poser("net-entreprises", "unvraimotdepasse", None).unwrap();
    let refus = generer_cle_au_poste(&a, false).unwrap_err();
    assert!(refus.contains("moteur coffre ouverture"), "{}", refus);
    assert!(!racine.join("config/maj-cle-privee.bin").exists());
    // Sans coffre du tout, pareil.
    std::fs::remove_file(a.chemin()).unwrap();
    assert!(generer_cle_au_poste(&a, false).unwrap_err().contains("moteur coffre ouverture"));
    assert!(!racine.join("config/maj-cle-privee.bin").exists() && !a.chemin().exists());
}

#[test]
fn le_coffre_qui_scelle_la_cle_ne_s_efface_pas_quand_il_ne_porte_plus_rien() {
    let a = editeur("cle-coffre-vide");
    let publique = publique(&generer_cle_au_poste(&a, false).unwrap());
    // Un coffre sans mot de passe d'organisme, dont on retire le mot de passe du logiciel : avant, il s'effacait. Il porte
    // la cle maitresse qui ouvre la cle de signature : il reste.
    assert!(a.oublier_ouverture(None).unwrap());
    assert!(a.chemin().exists());
    assert!(signature_valide(ARCHIVE, &signer_par_la_cle(&a, ARCHIVE).unwrap(), &publique));
}

#[test]
fn paie_maj_cle_sur_une_cle_a_l_ancienne_la_rescelle_sans_changer_la_paire_ou_dit_ce_qui_manque() {
    // Sans mot de passe du logiciel : la cle s'ouvre, reste comme elle est, et la reponse dit pourquoi et quoi faire.
    let racine = chemin_unique("cle-ancienne-cle").join("compagnie");
    std::fs::create_dir_all(racine.join("config")).unwrap();
    let a = poste(&racine, "poste-a");
    a.poser("net-entreprises", "unvraimotdepasse", None).unwrap();
    let graine = [9u8; 32];
    let publique = socle::base64::ecrire(ed25519_dalek::SigningKey::from_bytes(&graine).verifying_key().as_bytes());
    let chemin = racine.join("config/maj-cle-privee.bin");
    std::fs::write(&chemin, a.proteger(&graine, DESCRIPTION_CLE).unwrap()).unwrap();
    let avant = std::fs::read(&chemin).unwrap();
    let r = generer_cle_au_poste(&a, false).unwrap();
    assert_eq!((r["existante"].as_bool(), r["cle_publique"].as_str()), (Some(true), Some(publique.as_str())));
    assert!(r["a_l_ancienne"].as_str().unwrap().contains("moteur coffre ouverture"), "{}", r);
    assert_eq!(std::fs::read(&chemin).unwrap(), avant);

    // Le mot de passe du logiciel pose : `moteur maj --cle` la rescelle, la meme paire, et rien ne reste a dire.
    a.poser_ouverture(LOGICIEL, None).unwrap();
    let r = generer_cle_au_poste(&a, false).unwrap();
    assert_eq!((r["existante"].as_bool(), r["cle_publique"].as_str()), (Some(true), Some(publique.as_str())));
    assert!(r.get("a_l_ancienne").is_none(), "{}", r);
    assert_ne!(std::fs::read(&chemin).unwrap(), avant);
    // Une cle deja scellee par le coffre ne se refait pas sans --remplacer.
    assert!(generer_cle_au_poste(&a, false).unwrap_err().contains("--remplacer"));
    let b = apporte(&a, "cle-ancienne-cle-b");
    retenir_mot_de_passe_tape(LOGICIEL);
    let signature = signer_par_la_cle(&b, ARCHIVE);
    oublier_mot_de_passe_tape();
    assert!(signature_valide(ARCHIVE, &signature.unwrap(), &publique));
}
