//! Les minima conventionnels : le jour ou un accord de salaires lie la compagnie.

use droit::minima::{accord, grille_au, lie_depuis};
use serde_json::json;

use crate::outils::{code_livre, lire};

/// UN ACCORD LIE L'ADHERENTE D'UN SIGNATAIRE DES SA DATE, TOUTE AUTRE COMPAGNIE DES SON EXTENSION. L'accord de salaires
/// de la 1285 du 1er juillet 2026 s'applique « a compter du 1er aout 2026 pour les montants des salaires minima », « a
/// compter du 1er septembre 2026 pour les montants des indemnites et primes » (art. 5) ; il oblige « les signataires ou
/// membres des organisations ou groupements signataires » (C. trav. L2262-1) ; son extension « prend effet a compter de
/// la date de publication » de l'arrete (arrete du 7 septembre 2026, art. 2, JORF du 19/09/2026 ; L2261-15). Fichiers :
/// `atelier/reference/sources/kali-1285-accord-salaires-2026/` (art5, arrete art2, transcription) et
/// `legi-entrepot/articles.jsonl.gz`. Un accord que rien n'etend ne lie que les adherents.
#[test]
fn un_accord_lie_l_adherente_des_sa_date_et_toute_compagnie_des_son_extension() {
    let t: droit::minima::Table = socle::forme::lire(&lire(&code_livre(), "minima-conventionnels.json"), "").unwrap();
    let a = accord(&t, "kali-1285-accord-salaires-20260701").expect("l'accord de 2026 de la 1285 est dans la table");
    let lie = |org: Option<&str>, depuis: &str| lie_depuis(a, org, depuis);

    // Les minima (01/08) et les indemnites (01/09), pour une adherente d'un signataire (sous un nom ou sous un autre).
    for org in ["SYNDEAC", "syndeac", "Les Forces musicales", "Scène Ensemble"] {
        assert_eq!(lie(Some(org), "2026-08-01").as_deref(), Some("2026-08-01"), "{}", org);
        assert_eq!(lie(Some(org), "2026-09-01").as_deref(), Some("2026-09-01"), "{}", org);
    }
    // Sans adhesion, ou adherente d'une organisation qui n'a pas signe : le jour de la publication de l'extension.
    for org in [None, Some(""), Some("UNE AUTRE ORGANISATION")] {
        assert_eq!(lie(org, "2026-08-01").as_deref(), Some("2026-09-19"), "{:?}", org);
        assert_eq!(lie(org, "2026-09-01").as_deref(), Some("2026-09-19"), "{:?}", org);
    }
    // Un accord qui n'est pas etendu ne lie que les adherentes.
    let non_etendu: droit::minima::Convention = socle::forme::lire(&json!({"signataires": ["SYNDEAC"]}), "").unwrap();
    assert_eq!(lie_depuis(&non_etendu, Some("SYNDEAC"), "2026-08-01").as_deref(), Some("2026-08-01"));
    assert_eq!(lie_depuis(&non_etendu, None, "2026-08-01"), None);

    // La grille du jour suit la meme date : celle de l'accord des qu'il lie la compagnie, celle d'avant jusque-la.
    let depuis = |org: Option<&str>, jour: &str| grille_au(&t, "1285", org, Some(jour)).unwrap().1;
    assert_eq!(depuis(Some("SYNDEAC"), "2026-07-31"), None);
    assert_eq!(depuis(Some("SYNDEAC"), "2026-08-01").as_deref(), Some("2026-08-01"));
    assert_eq!(depuis(None, "2026-09-18"), None);
    assert_eq!(depuis(None, "2026-09-19").as_deref(), Some("2026-09-19"));
}
