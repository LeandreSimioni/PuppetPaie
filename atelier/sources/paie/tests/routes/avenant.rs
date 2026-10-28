//! `avenant` : le texte qu'on compare (`nfkc` contre 174 paires de `unicodedata.normalize`, `donnees/nfkc.json` ;
//! `normaliser`).

use paie::avenant::normaliser;
use socle::nfkc::nfkc;
use serde_json::Value;
use std::path::Path;

// ------------------------------------------------------------ le texte qu'on compare

fn fixture() -> Vec<(String, String)> {
    let f = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/donnees/nfkc.json");
    let v: Value = serde_json::from_str(&std::fs::read_to_string(f).unwrap()).unwrap();
    v.as_array().unwrap().iter().map(|p| (p[0].as_str().unwrap().to_string(), p[1].as_str().unwrap().to_string())).collect()
}

#[test]
fn nfkc_rend_la_forme_de_chaque_paire_relevee() {
    let cas = fixture();
    assert!(cas.len() > 150);
    for (entree, attendu) in cas {
        assert_eq!(nfkc(&entree), attendu, "{:?}", entree);
    }
}

#[test]
fn normaliser_defait_les_ligatures_recolle_les_coupures_et_ramene_les_blancs() {
    assert_eq!(normaliser("  Le  contrat\u{a0}n°\u{202f}1\n\tﬁn  "), "Le contrat n° 1 fin");
    // Une coupure apres un trait d'union est recollee ; avant un blanc, elle reste un blanc.
    assert_eq!(normaliser("porte-\nparole"), "porte-parole");
    assert_eq!(normaliser("tiret -\n suivi"), "tiret - suivi");
    assert_eq!(normaliser("a-\n"), "a-");
}
