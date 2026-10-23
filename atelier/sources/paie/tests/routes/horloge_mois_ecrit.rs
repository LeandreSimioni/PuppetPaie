//! L'HORLOGE D'UNE COMPAGNIE FICTIVE ECRIT LE MOIS (#236). Un mois est « ecrit » quand le moteur l'a consigne en le
//! produisant -- sa DSN mensuelle notee au journal des ecritures (`produire_mois::mois_ecrit`) --, jamais parce qu'une
//! DPAE ou un contrat est dans `sorties/<mois>/`. Au banc, une compagnie ajoutee dont les DPAE etaient parties ne voyait
//! jamais son mois ecrit : le testeur devait le fermer lui-meme. Sur `fictive-3090-petite`, dont novembre est saisi et
//! pas ecrit.

use crate::outils::{compagnie, dossier_de_test, lire, reussi, route};
use serde_json::json;

#[test]
fn une_dpae_dans_les_sorties_n_ecrit_pas_le_mois_l_horloge_l_ecrit() {
    let c = compagnie::ses_dsn("fictive-3090-petite", &[], &dossier_de_test("horloge_mois_ecrit", "dpae").join("compagnie"));
    // La DPAE de novembre est partie avant le 1er decembre : son fichier est dans les sorties du mois.
    let sorties = c.join("sorties").join("2026-11");
    std::fs::create_dir_all(&sorties).unwrap();
    std::fs::write(sorties.join("DPAE_Novembre2026.txt"), "x").unwrap();
    let ecrites = |c: &std::path::Path| {
        lire(c, "journal/dsn-ecrites.json").as_object().unwrap().keys().filter(|k| k.starts_with("2026-11/DSN_")).count()
    };
    assert_eq!(ecrites(&c), 0, "novembre n'est pas encore ecrit");
    let phrases = reussi(&route(&c, "horloge-rattraper", json!({"aujourdhui": "2026-12-01"}), "2026-12-01")).clone();
    assert!(
        phrases.as_array().unwrap().iter().any(|p| p.as_str().unwrap().contains("Novembre 2026 écrit")),
        "le 1er decembre, l'horloge ecrit novembre : {}",
        phrases
    );
    assert_eq!(ecrites(&c), 1, "la DSN de novembre est consignee au journal des ecritures");
    // Le lendemain, la saisie inchangee : rien a reecrire.
    let phrases = reussi(&route(&c, "horloge-rattraper", json!({"aujourdhui": "2026-12-02"}), "2026-12-02")).clone();
    assert!(!phrases.to_string().contains("Novembre 2026 écrit"), "{}", phrases);
}
