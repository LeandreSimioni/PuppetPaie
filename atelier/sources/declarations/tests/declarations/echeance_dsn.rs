//! L'echeance de depot d'une DSN mensuelle (`declarations/src/echeance_dsn.rs`), contre ses textes deposes :
//!
//! - le jour : CSS R. 243-6, II (`atelier/reference/sources/echeances-dsn/legi-securite-sociale-R243-6-LEGIARTI000034668597.xml`),
//!   le 5 ou le 15 du mois suivant ;
//! - le report : CSS R. 133-14, I (`atelier/reference/sources/code-de-la-securite-sociale/xml/R133-14.xml`), « Si le delai
//!   [...] expire un jour ferie ou non ouvre, il est proroge jusqu'au jour ouvrable suivant » ;
//! - la pratique du point de depot : `atelier/reference/sources/echeances-dsn/net-entreprises-calendrier-des-declarations-20261015.txt` ;
//! - midi : cahier technique DSN 2026.1, § 1.4.2 (`atelier/reference/norme-dsn/texte/chapitre-1-la-norme.txt`) ;
//! - les fetes legales : L. 3133-1 (`logiciel/donnees/feries-base.json`).

use socle::date::Jour;
use declarations::echeance_dsn::{du_mois, reporter, HEURE_LIMITE};

fn j(s: &str) -> Jour {
    Jour::lire(s).unwrap()
}

/// Le calendrier de net-entreprises (releve du 15/10/2026) : DSN d'octobre 2026 au 15, le dimanche 15/11/2026 devient le
/// lundi 16/11/2026 ; R. 133-14, I.
#[test]
fn un_dimanche_reporte_au_lundi() {
    let e = du_mois("2026-10", 15).unwrap();
    assert_eq!((e.nominale, e.jour, e.reportee()), (j("2026-11-15"), j("2026-11-16"), true));
    assert!(e.pourquoi().unwrap().contains("dimanche"), "{:?}", e.pourquoi());
}

/// Le meme calendrier : DSN de novembre 2026 au 5, le samedi 05/12/2026 devient le lundi 07/12/2026 -- le samedi est non
/// ouvre pour le point de depot.
#[test]
fn un_samedi_reporte_au_lundi() {
    let e = du_mois("2026-11", 5).unwrap();
    assert_eq!((e.nominale, e.jour), (j("2026-12-05"), j("2026-12-07")));
    assert_eq!(e.sautes.len(), 2, "le samedi et le dimanche");
}

/// R. 133-14, I, « un jour ferie » ; L. 3133-1, 8° (`feries-base.json`) : le mercredi 15/08/2029, Assomption, reporte la
/// DSN de juillet 2029 au jeudi 16/08/2029.
#[test]
fn un_ferie_reporte_au_jour_ouvre_suivant() {
    let e = du_mois("2029-07", 15).unwrap();
    assert_eq!((e.nominale, e.jour), (j("2029-08-15"), j("2029-08-16")));
    assert!(e.pourquoi().unwrap().contains("Assomption"), "{:?}", e.pourquoi());
    // Le dimanche 05/04/2026, puis le lundi de Paques 06/04/2026 (L. 3133-1, 2°) : le mardi 07/04/2026.
    assert_eq!(du_mois("2026-03", 5).unwrap().jour, j("2026-04-07"));
}

/// INFERENCE (aucune piece officielle ne montre ce cas, `echeance_dsn.rs`, inference 2) : le vendredi 15/08/2025, ferie,
/// reporte au lundi 18/08/2025, pas au samedi.
#[test]
fn un_vendredi_ferie_reporte_au_lundi_par_inference() {
    assert_eq!(du_mois("2025-07", 15).unwrap().jour, j("2025-08-18"));
}

/// Le calendrier de net-entreprises : jeudi 05/11/2026, jeudi 15/10/2026 et mardi 15/12/2026 ne bougent pas (R. 243-6, II,
/// seul) ; le passage de decembre a janvier.
#[test]
fn une_date_ouvree_ne_bouge_pas() {
    for (mois, jour, attendu) in [("2026-10", 5, "2026-11-05"), ("2026-09", 15, "2026-10-15"), ("2026-11", 15, "2026-12-15"), ("2026-08", 15, "2026-09-15")] {
        let e = du_mois(mois, jour).unwrap();
        assert_eq!((e.jour, e.reportee(), e.pourquoi()), (j(attendu), false, None), "{} au {}", mois, jour);
    }
    // Le 05/01/2027 est un mardi : la DSN de decembre 2026 au 5.
    assert_eq!(du_mois("2026-12", 5).unwrap().jour, j("2027-01-05"));
}

/// Cahier technique § 1.4.2 : l'heure limite est midi ; l'annule-et-remplace s'arrete la veille a minuit -- la veille de
/// l'echeance reportee (inference 3).
#[test]
fn l_heure_limite_est_midi_et_la_veille_suit_le_report() {
    let e = reporter(j("2026-11-15")).unwrap();
    assert_eq!(HEURE_LIMITE, "12:00");
    let v = e.en_json();
    assert_eq!((v["jour"].as_str(), v["heure"].as_str(), v["nominale"].as_str()), (Some("2026-11-16"), Some("12:00"), Some("2026-11-15")));
    assert_eq!(v["veille_annule_et_remplace"].as_str(), Some("2026-11-15"));
    assert!(!v["inference"].as_str().unwrap().is_empty(), "l'inference se dit comme telle");
    assert!(v["textes"].to_string().contains("R133-14.xml"));
    assert!(e.phrase().contains("lundi 16/11/2026"), "{}", e.phrase());
}
