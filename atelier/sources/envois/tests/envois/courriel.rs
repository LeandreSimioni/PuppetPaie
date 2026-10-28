//! La boite simulee des compagnies fictives : elle garde ce qu'on lui envoie, sans rien envoyer.

use envois::courriel::{BoiteSimulee, Courriel};
use serde_json::json;

use crate::outils::dossier_de_test;

#[test]
fn boite_simulee_garde_sans_envoyer() {
    let r = dossier_de_test("courriel", "boite-envoi");
    let mut b = BoiteSimulee::nouvelle(&r, &json!({"signataire": {"email": "direction@demo.invalid"}}));
    let m1 = b"From: a@b.fr\nMessage-ID: <un.deux@demo.invalid>\nSubject: un\n\ncorps\n".to_vec();
    let m2 = b"From: a@b.fr\nSubject: sans identifiant\n\ncorps\n".to_vec();
    assert_eq!(b.envoyer(&m1).unwrap(), "<un.deux@demo.invalid>");
    // Sans identifiant, la boite en donne un, comme un serveur.
    assert!(b.envoyer(&m2).unwrap().contains('@'));
    // Chaque message est ecrit tel quel dans `courriels-simules/`, et nulle part ailleurs.
    let ecrits: Vec<Vec<u8>> =
        std::fs::read_dir(r.join("courriels-simules")).unwrap().map(|e| std::fs::read(e.unwrap().path()).unwrap()).collect();
    assert_eq!(ecrits.len(), 2);
    assert!(ecrits.contains(&m1) && ecrits.contains(&m2));
    b.fermer();
}
