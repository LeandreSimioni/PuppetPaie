//! L'HORLOGE D'UNE COMPAGNIE FICTIVE ECRIT LE MOIS (#236). Un mois est « ecrit » quand le moteur l'a consigne en le
//! produisant -- sa DSN mensuelle notee au journal des ecritures (`produire_mois::mois_ecrit`) --, jamais parce qu'une
//! DPAE ou un contrat est dans `sorties/<mois>/`. Au banc, une compagnie ajoutee dont les DPAE etaient parties ne voyait
//! jamais son mois ecrit : le testeur devait le fermer lui-meme. Sur `fictive-3090-petite`, dont novembre est saisi et
//! pas ecrit.

use crate::outils::{compagnie, dossier_de_test, ecrire, lire, reussi, route};
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

/// UN SEUL CRITERE POUR « MOIS ECRIT » (#236) : l'accueil (« a deposer » ou « a ecrire »), le rappel de la DSN et
/// l'etape DSN de la page Paie lisent celui de l'horloge (`produire_mois::mois_ecrit`) : la DSN mensuelle notee au journal
/// des ecritures, et toujours dans `sorties/<mois>/`. Un fichier DSN pose sans que le moteur l'ait note n'ecrit pas le
/// mois ; fichier et note qui concordent l'ecrivent ; une note dont le fichier a disparu ne l'ecrit plus. Sur
/// `fictive-3090-petite`, dont octobre 2026 est saisi et pas ecrit.
#[test]
fn l_accueil_le_rappel_et_la_page_paie_lisent_le_critere_de_l_horloge() {
    let c = compagnie::ses_dsn("fictive-3090-petite", &[], &dossier_de_test("horloge_mois_ecrit", "un-critere").join("compagnie"));
    const LE: &str = "2026-11-03";
    let accueil = |c: &std::path::Path| {
        let e = reussi(&route(c, "etat", json!({}), LE)).clone();
        e["historique"].as_array().unwrap().iter().find(|m| m["mois"] == json!("2026-10")).unwrap()["dsn"].clone()
    };
    let rappel = |c: &std::path::Path| {
        let r = reussi(&route(c, "rappels", json!({}), LE)).clone();
        r["rappels"].as_array().unwrap().iter().find(|x| x["cle"] == json!("dsn:2026-10")).cloned().unwrap_or_else(|| panic!("le rappel de la DSN d'octobre : {}", r))
    };
    let etape = |c: &std::path::Path| reussi(&route(c, "bilan-du-mois", json!({"mois": "2026-10"}), LE))["dsn"]["etape"].clone();
    assert_eq!(accueil(&c), json!("a_ecrire"));

    // Un fichier DSN pose a la main, que le moteur n'a pas note : le mois n'est pas ecrit, nulle part.
    let mut octets = std::fs::read(c.join("sorties/2026-09/DSN_Septembre2026.dsn")).unwrap();
    octets.extend_from_slice(b"\r\n");
    std::fs::create_dir_all(c.join("sorties/2026-10")).unwrap();
    std::fs::write(c.join("sorties/2026-10/DSN_Octobre2026.dsn"), &octets).unwrap();
    assert_eq!(accueil(&c), json!("a_ecrire"));
    assert_ne!(rappel(&c)["etat"], json!("a_deposer"));
    assert!(etape(&c).is_null(), "{}", etape(&c));

    // Le fichier et sa note concordent : ecrit, partout.
    let mut journal = lire(&c, "journal/dsn-ecrites.json");
    journal["2026-10/DSN_Octobre2026.dsn"] = json!({"quand": "2026-11-02T10:00:00"});
    ecrire(&c, "journal/dsn-ecrites.json", &journal);
    assert_eq!(accueil(&c), json!("a_deposer"));
    let r = rappel(&c);
    assert_eq!(r["etat"], json!("a_deposer"), "{}", r);
    assert!(r["fichier"].as_str().unwrap().ends_with("DSN_Octobre2026.dsn"), "{}", r);
    assert!(!etape(&c).is_null(), "l'etape DSN dit ou en est la DSN ecrite");

    // La note reste, le fichier est parti : plus rien a deposer.
    std::fs::remove_file(c.join("sorties/2026-10/DSN_Octobre2026.dsn")).unwrap();
    assert_eq!(accueil(&c), json!("a_ecrire"));
    assert_ne!(rappel(&c)["etat"], json!("a_deposer"));
    assert!(etape(&c).is_null());
}
