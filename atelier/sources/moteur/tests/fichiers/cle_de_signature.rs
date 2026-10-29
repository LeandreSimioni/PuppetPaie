//! La cle qui signe les mises a jour (`config/maj-cle-privee.bin`, `moteur::compagnie::maj`) : scellee par la cle maitresse du coffre,
//! elle s'ouvre partout ou le coffre s'ouvre — la cle du poste qui l'a faite, ou le mot de passe du logiciel sur un autre
//! (un poste perdu n'empeche pas de signer la mise a jour suivante). Sur des
//! compagnies, des postes et des mots de passe inventes, chacun dans son dossier (jamais la vraie cle du poste).

use crate::outils::chemin_unique;
use moteur::compagnie::coffre::{oublier_mot_de_passe_tape, retenir_mot_de_passe_tape, Coffre};
use moteur::atelier::publier::{generer_cle_au_poste, signer_par_la_cle};
use moteur::compagnie::maj::signature_valide;
use std::collections::BTreeMap;
use std::path::Path;

const LOGICIEL: &str = "sesame-123";
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

