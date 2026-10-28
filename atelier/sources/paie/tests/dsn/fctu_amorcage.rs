//! Les controles propres au signalement de fin de contrat (nature 07) : CCH-14 et CCH-15 de `fctu::verifier_controles_07`.

use declarations::fctu::{self, Echec};

#[test]
fn cch_14_la_base_31_est_interdite() {
    let faux = "S21.G00.78.001,'23'\r\nS21.G00.78.001,'31'\r\n";
    match fctu::verifier_controles_07(faux) {
        Err(Echec::Impossible(m)) => assert!(m.starts_with("CCH-14 : la valeur « 31 »"), "{}", m),
        r => panic!("une base 31 est passee : {:?}", r),
    }
}

#[test]
fn cch_15_il_faut_les_trois_blocs_51() {
    let faux = "S21.G00.51.011,'001'\nS21.G00.51.011,'002'\n";
    match fctu::verifier_controles_07(faux) {
        Err(Echec::Impossible(m)) => assert!(m.starts_with("CCH-15") && m.contains("Manque : 010."), "{}", m),
        r => panic!("un FCTU sans salaire de base est passe : {:?}", r),
    }
    let moins = "S21.G00.51.011,'001'\n";
    match fctu::verifier_controles_07(moins) {
        Err(Echec::Impossible(m)) => assert!(m.contains("Manquent : 002, 010."), "{}", m),
        r => panic!("{:?}", r),
    }
    fctu::verifier_controles_07("S21.G00.51.011,'001'\nS21.G00.51.011,'002'\nS21.G00.51.011,'010'\n").unwrap();
}


