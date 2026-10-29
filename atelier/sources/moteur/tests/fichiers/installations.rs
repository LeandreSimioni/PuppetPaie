//! UNE FENETRE PAR INSTALLATION (`interface::instance`) : deux installations s'ouvrent cote a cote sur un
//! poste, chacune a son creneau (son verrou et le port de sa page) ; la meme, relancee, ramene sa fenetre devant ; le
//! port d'une installation reste le meme d'un lancement a l'autre (retenu dans `.fenetre.json`), et un creneau pris par
//! un autre programme ou par une copie de l'installation se saute. Sur une plage de ports a ces essais (jamais celle du
//! poste, 7330-7429), dans des installations de test.
//!
//! Le fil de fond de la fenetre (`fil_de_fond`) ne sort pas et ne touche a aucune cle du coffre.

use std::path::Path;

use crate::outils::dossier_de_test;

#[test]
fn le_fil_de_fond_ne_sort_pas_et_ne_touche_a_aucune_cle() {
    let racine = dossier_de_test("installations", "fil-de-fond");
    std::fs::create_dir_all(racine.join("config")).unwrap();
    let coffre = move |r: &Path| moteur::compagnie::coffre::Coffre::sur(r, r.join("cle"), "2026-10-22T10:00");
    let r = racine.clone();
    let (reseau, cle) = std::thread::spawn(move || {
        socle::fil_de_fond::marquer();
        (
            socle::garde_reseau::sortie_vers("127.0.0.1").is_err() && socle::garde_reseau::sortie_vers("net-entreprises.fr").is_err(),
            coffre(&r).poser_cle_poste().is_err(),
        )
    })
    .join()
    .unwrap();
    assert!(reseau, "aucune sortie depuis le fil de fond");
    assert!(cle && !racine.join("cle").join(moteur::compagnie::coffre::FICHIER_CLE).exists(), "la cle du poste ne s'y sert ni ne s'y pose");
    // Hors du fil de fond, rien ne change.
    assert!(socle::garde_reseau::sortie_vers("127.0.0.1").is_ok());
    assert!(coffre(&racine).poser_cle_poste().is_ok());
}

