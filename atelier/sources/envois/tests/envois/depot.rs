//! La preparation d'un depot : un depot de test ne part jamais en reel (S10.G00.00.005 = 01 pour l'essai, 02 pour le reel :
//! `atelier/reference/norme-dsn/texte/chapitre-1-la-norme.txt`, l. 278-279 ; la DPAE en C_VERSION `TST`), un reel exige
//! sa preuve et un fichier reel, l'annule-et-remplace (`atelier/reference/norme-dsn/texte/bloc-S20.G00.05.txt`, l. 84-88 :
//! 01 normale, 02 normale sans individu, 03 et 05 leurs annule-et-remplace). Fichiers fabriques, NIR et SIREN inventes.

use envois::depot::{basculer_dpae, basculer_dsn, est_reel, lire, renumeroter, verifier};

fn dsn(nature_envoi: &str) -> Vec<u8> {
    format!(
        "S10.G00.00.001,'PUPPETPAIE'\r\nS10.G00.00.005,'{}'\r\nS10.G00.01.001,'123456789'\r\nS20.G00.05.001,'01'\r\n\
         S20.G00.05.002,'01'\r\nS20.G00.05.003,'11'\r\nS20.G00.05.004,'1'\r\nS20.G00.05.005,'01082026'\r\n\
         S20.G00.05.006,'030920261'\r\nS20.G00.05.007,'10092026'\r\nS21.G00.11.001,'12345678900011'\r\n\
         S21.G00.30.001,'1850175123456'\r\nS21.G00.30.002,'DUPONT'\r\nS90.G00.90.001,'14'\r\n",
        nature_envoi
    )
    .into_bytes()
}

/// Des enregistrements DPAE a plat de 990 caracteres, l'indicateur en 72-75.
fn flux(indicateur: &str, n: usize) -> Vec<u8> {
    let mut un = vec![b'A'; 990];
    un[72..75].copy_from_slice(indicateur.as_bytes());
    un.repeat(n)
}

#[test]
fn le_test_ne_part_jamais_en_02() {
    for nature in ["02", "01", "03", "99"] {
        let s = basculer_dsn(&dsn(nature)).unwrap();
        assert_eq!(lire(&s).champ("nature_envoi"), "01", "{}", nature);
    }
}

#[test]
fn la_bascule_dpae_repasse_chaque_enregistrement_en_tst() {
    let f = flux("120", 3);
    let s = basculer_dpae(&f, 990).unwrap();
    for i in 0..3 {
        assert_eq!(&s[i * 990 + 72..i * 990 + 75], b"TST");
    }
    assert!(!est_reel(&s));
    // Le fichier d'origine ne bouge pas : la bascule est en memoire.
    assert!(est_reel(&f));
}

#[test]
fn la_renumerotation_fait_un_annule_et_remplace() {
    let a = lire(&renumeroter(&dsn("01"), "7").unwrap());
    assert_eq!((a.champ("type_declaration"), a.champ("ordre")), ("03", "7"));
    let t = String::from_utf8(dsn("01")).unwrap().replace("S20.G00.05.002,'01'", "S20.G00.05.002,'02'");
    assert_eq!(lire(&renumeroter(t.as_bytes(), "2").unwrap()).champ("type_declaration"), "05");
    // Sans numero d'ordre, rien ne se renumerote.
    let sans = String::from_utf8(dsn("01")).unwrap().replace("S20.G00.05.004,'1'\r\n", "");
    assert!(renumeroter(sans.as_bytes(), "2").is_err());
}

#[test]
fn un_reel_demande_sur_un_fichier_de_test_est_refuse() {
    assert!(verifier(&lire(&dsn("01")), true, Some("2026-08"), true).is_err());
}

#[test]
fn un_reel_exige_une_preuve_et_jamais_rien() {
    let a = lire(&dsn("02"));
    assert!(verifier(&a, true, None, false).is_err());
    assert!(verifier(&a, true, Some(""), false).is_err());
    assert!(verifier(&a, true, None, true).is_ok());
    assert!(verifier(&a, true, Some("2026-08"), false).is_ok());
    assert!(verifier(&a, true, Some("  2026-08 \n"), false).is_ok());
    // Un mois recopie faux refuse, meme avec la presence prouvee : on ne confirme pas a moitie.
    for mauvais in ["2026-8", "oui", "2026-07", "  "] {
        assert!(verifier(&a, true, Some(mauvais), true).is_err(), "{}", mauvais);
    }
}
