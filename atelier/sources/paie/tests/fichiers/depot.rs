//! La preparation des depots (`envois::depot`) : lecture d'une DSN, bascule en test, annule-et-remplace, preuve d'un depot
//! reel, indicateur DPAE, identifiants du declarant ; sur des fichiers fabriques (NIR et SIREN inventes), sans reseau.

use envois::depot::{basculer_dpae, basculer_dsn, est_reel, identifiants, indicateur, lire, renumeroter, verifier, INDICATEUR_REEL, INDICATEUR_TEST};
use serde_json::json;

fn dsn(nature_envoi: &str) -> Vec<u8> {
    format!(
        "S10.G00.00.001,'PUPPETPAIE'\r\nS10.G00.00.005,'{}'\r\nS10.G00.01.001,'123456789'\r\nS20.G00.05.001,'01'\r\n\
         S20.G00.05.002,'01'\r\nS20.G00.05.003,'11'\r\nS20.G00.05.004,'1'\r\nS20.G00.05.005,'01082026'\r\n\
         S20.G00.05.006,'030920261'\r\nS20.G00.05.007,'10092026'\r\nS21.G00.11.001,'12345678900011'\r\n\
         S21.G00.20.005,'275.90'\r\nS21.G00.20.005,'697.00'\r\nS21.G00.30.001,'1850175123456'\r\n\
         S21.G00.30.002,'DUPONT'\r\nS21.G00.50.007,'01'\r\nS21.G00.50.007,'23'\r\nS90.G00.90.001,'14'\r\n",
        nature_envoi
    )
    .into_bytes()
}

// ------------------------------------------------------------------- lire

#[test]
fn lire_rend_ce_que_le_fichier_declare() {
    let a = lire(&dsn("02"));
    assert_eq!(a.champ("nature_envoi"), "02");
    assert_eq!(a.champ("siren"), "123456789");
    assert_eq!(a.champ("mois"), "01082026");
    assert_eq!(a.champ("etablissement"), "12345678900011");
    assert_eq!(a.salaries, 1);
    assert_eq!(a.versements, ["275.90", "697.00"]);
    assert_eq!(a.lignes, 18);
    assert_eq!((a.pas_versements, a.pas_bareme), (2, 1));
    assert!(a.reel());
    assert_eq!(a.mois_lisible(), "2026-08");
    assert_eq!(a.confirmation_attendue(), "2026-08");
}

#[test]
fn un_retour_chariot_seul_est_une_fin_de_ligne() {
    let cr = String::from_utf8(dsn("02")).unwrap().replace("\r\n", "\r");
    let a = lire(cr.as_bytes());
    assert_eq!(a.versements.len(), 2);
    assert_eq!(a.champ("mois"), "01082026");
}

#[test]
fn la_premiere_valeur_de_chaque_rubrique_l_emporte() {
    let a = lire(b"S10.G00.00.005,'01'\nS10.G00.00.005,'02'\n");
    assert_eq!(a.champ("nature_envoi"), "01");
    // Une ligne sans guillemet final n'est pas une rubrique.
    assert_eq!(lire(b"S10.G00.00.005,'02\n").champ("nature_envoi"), "");
}

#[test]
fn un_signalement_se_date_par_son_evenement() {
    let mut f = dsn("02");
    f.extend_from_slice(b"S21.G00.60.002,'05102026'\r\n");
    let t = String::from_utf8(f).unwrap().replace("S20.G00.05.001,'01'", "S20.G00.05.001,'04'");
    let a = lire(t.as_bytes());
    assert!(a.est_signalement());
    assert_eq!(a.confirmation_attendue(), "2026-10-05");
    // Sans la date de l'evenement, il n'y a rien a recopier.
    let t = t.replace("S21.G00.60.002,'05102026'", "S21.G00.60.002,''");
    assert_eq!(lire(t.as_bytes()).confirmation_attendue(), "?");
}

// ---------------------------------------------------------------- la bascule

#[test]
fn la_bascule_en_test_mord_vraiment() {
    let f = dsn("02");
    let s = basculer_dsn(&f).unwrap();
    assert!(String::from_utf8_lossy(&s).contains("S10.G00.00.005,'01'"));
    assert!(!String::from_utf8_lossy(&s).contains("S10.G00.00.005,'02'"));
    assert_eq!(s.len(), f.len());
    assert!(String::from_utf8_lossy(&s).contains("S20.G00.05.006,'030920261'"));
    assert_eq!(lire(&s).champ("nature_envoi"), "01");
}

#[test]
fn une_dsn_sans_nature_d_envoi_ne_se_bascule_pas() {
    let e = basculer_dsn(b"bonjour\r\n").unwrap_err();
    assert_eq!(e.classe, Some("DepotRefuse"));
    assert!(e.message.contains("S10.G00.00.005 introuvable"));
    // Une nature sans deux chiffres n'est pas une nature : rien ne change, et on refuse.
    assert!(basculer_dsn(b"S10.G00.00.005,'2'\n").is_err());
}

#[test]
fn la_bascule_ne_touche_que_la_premiere_nature() {
    let s = basculer_dsn(b"S10.G00.00.005,'02'\nS10.G00.00.005,'02'\n").unwrap();
    assert_eq!(s, b"S10.G00.00.005,'01'\nS10.G00.00.005,'02'\n");
}

#[test]
fn le_test_ne_part_jamais_en_02() {
    for nature in ["02", "01", "03", "99"] {
        let s = basculer_dsn(&dsn(nature)).unwrap();
        assert_eq!(lire(&s).champ("nature_envoi"), "01", "{}", nature);
    }
}

#[test]
fn la_renumerotation_fait_un_annule_et_remplace() {
    let s = renumeroter(&dsn("01"), "7").unwrap();
    let a = lire(&s);
    assert_eq!(a.champ("type_declaration"), "03");
    assert_eq!(a.champ("ordre"), "7");
    // 02 devient 05 ; un autre type reste, l'ordre change quand meme.
    let t = String::from_utf8(dsn("01")).unwrap().replace("S20.G00.05.002,'01'", "S20.G00.05.002,'02'");
    assert_eq!(lire(&renumeroter(t.as_bytes(), "2").unwrap()).champ("type_declaration"), "05");
    let t = t.replace("S20.G00.05.002,'02'", "S20.G00.05.002,'04'");
    assert_eq!(lire(&renumeroter(t.as_bytes(), "2").unwrap()).champ("type_declaration"), "04");
    // Sans ordre, on refuse.
    let sans = String::from_utf8(dsn("01")).unwrap().replace("S20.G00.05.004,'1'\r\n", "");
    let e = renumeroter(sans.as_bytes(), "2").unwrap_err();
    assert!(e.message.contains("S20.G00.05.004 introuvable"));
    assert!(e.message.contains("DSN mensuelle"));
}

// ------------------------------------------------------------------ verifier

#[test]
fn un_test_demande_sur_un_fichier_reel_passe_sans_preuve() {
    // Le test est la regle de la bascule : `verifier` ne refuse rien, c'est elle qui force `01`.
    assert!(verifier(&lire(&dsn("02")), false, None, false).is_ok());
    assert!(verifier(&lire(&dsn("01")), false, None, false).is_ok());
}

#[test]
fn un_reel_demande_sur_un_fichier_de_test_est_refuse() {
    let e = verifier(&lire(&dsn("01")), true, Some("2026-08"), true).unwrap_err();
    assert_eq!(e.classe, Some("DepotRefuse"));
    assert!(e.message.starts_with("Le fichier porte S10.G00.00.005='01' (envoi de test)"));
    assert!(e.message.contains("REEL"));
}

#[test]
fn un_reel_exige_une_preuve_et_jamais_rien() {
    let a = lire(&dsn("02"));
    let e = verifier(&a, true, None, false).unwrap_err();
    assert_eq!(
        e.message,
        "Un depot REEL demande une preuve : reecrire le mois declare (2026-08), ou retaper le mot de passe du logiciel."
    );
    assert!(verifier(&a, true, Some(""), false).is_err());
    assert!(verifier(&a, true, None, true).is_ok());
    assert!(verifier(&a, true, Some("2026-08"), false).is_ok());
    assert!(verifier(&a, true, Some("  2026-08 \n"), false).is_ok());
    for mauvais in ["2026-8", "oui", "y", "2026-07", "  "] {
        let e = verifier(&a, true, Some(mauvais), true).unwrap_err();
        assert!(e.message.starts_with(&format!("Mois confirme '{}' alors que le fichier declare '2026-08'", mauvais.trim())));
        assert!(e.message.ends_with("Rien n'est parti."));
    }
}

#[test]
fn la_preuve_n_est_pas_celle_d_un_mois_qui_n_est_pas_le_bon() {
    // Une confirmation fausse refuse meme avec la presence prouvee : on ne confirme pas a moitie.
    let a = lire(&dsn("02"));
    assert!(verifier(&a, true, Some("2026-07"), true).is_err());
}

#[test]
fn un_signalement_se_confirme_par_le_jour_de_l_evenement() {
    let t = String::from_utf8(dsn("02")).unwrap().replace("S20.G00.05.001,'01'", "S20.G00.05.001,'07'")
        + "S21.G00.62.001,'30092026'\r\n";
    let a = lire(t.as_bytes());
    let e = verifier(&a, true, None, false).unwrap_err();
    assert!(e.message.contains("reecrire le jour de l'evenement signale (2026-09-30)"));
    let e = verifier(&a, true, Some("2026-09"), false).unwrap_err();
    assert!(e.message.starts_with("Jour de l'evenement confirme '2026-09'"));
    assert!(verifier(&a, true, Some("2026-09-30"), false).is_ok());
}

// --------------------------------------------------------------------- DPAE

fn flux(indicateur: &str, n: usize) -> Vec<u8> {
    let mut un = vec![b'A'; 990];
    un[72..75].copy_from_slice(indicateur.as_bytes());
    un.repeat(n)
}

#[test]
fn l_indicateur_se_lit_en_72_75() {
    assert_eq!(indicateur(&flux("120", 1)), "120");
    assert!(est_reel(&flux(INDICATEUR_REEL, 2)));
    assert!(!est_reel(&flux(INDICATEUR_TEST, 1)));
    assert_eq!(indicateur(b"court"), "");
    assert_eq!(indicateur(&[b'x'; 73]), "x");
    assert!(!est_reel(b""));
}

#[test]
fn la_bascule_dpae_repasse_chaque_enregistrement_en_tst() {
    let f = flux("120", 3);
    let s = basculer_dpae(&f, 990).unwrap();
    assert_eq!(s.len(), f.len());
    for i in 0..3 {
        assert_eq!(&s[i * 990 + 72..i * 990 + 75], b"TST");
        assert_eq!(s[i * 990 + 71], b'A');
        assert_eq!(s[i * 990 + 75], b'A');
    }
    assert!(!est_reel(&s));
    // Le fichier d'origine ne bouge pas : la bascule est en memoire.
    assert!(est_reel(&f));
}

#[test]
fn un_flux_de_longueur_inattendue_ne_se_bascule_pas() {
    let mut f = flux("120", 2);
    f.push(b'x');
    let e = basculer_dpae(&f, 990).unwrap_err();
    assert_eq!(e.message, "flux DPAE de longueur inattendue");
    assert_eq!(basculer_dpae(&[], 990).unwrap(), Vec::<u8>::new());
    assert!(basculer_dpae(&f, 0).is_err());
}

#[test]
fn les_identifiants_du_declarant() {
    let ok = json!({ "siret": "12345678900011", "nom": "DURAND", "prenom": "Jean-Pierre" });
    assert!(identifiants(&ok, false).unwrap().is_empty());
    let s = identifiants(&json!({ "siret": "1", "nom": "Zoë", "prenom": "Élodie" }), true).unwrap();
    assert_eq!(s.len(), 3);
    assert!(s[0].starts_with("Le nom du declarant ('Zoë') porte un accent."));
    assert!(s[1].starts_with("Le prenom du declarant ('Élodie') porte un accent."));
    assert!(s[2].starts_with("Le mot de passe contient un caractere @"));
    let s = identifiants(&json!({ "siret": "", "nom": "X" }), false).unwrap();
    assert!(s[0].contains("manque : siret, prenom"));
    // L'insecable est un accent pour la DUE.
    let s = identifiants(&json!({ "siret": "1", "nom": "a\u{a0}b", "prenom": "x" }), false).unwrap();
    assert!(s[0].contains("porte un accent"), "{}", s[0]);
    assert!(identifiants(&json!({ "siret": "1", "nom": 7, "prenom": "x" }), false).is_err());
}
