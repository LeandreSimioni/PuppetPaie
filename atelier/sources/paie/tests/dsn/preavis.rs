//! Le preavis et le delai de prevenance d'une fin de contrat, tels que la DSN les declare : le code du bloc 52 de
//! l'indemnite selon le motif (S21.G00.52.001), le type de preavis du signalement (S21.G00.63.001) confronte a ce que la
//! paie verse. Textes : `atelier/reference/norme-dsn/texte/bloc-S21.G00.52.txt` (cahier technique 2026.1, p. 250-254),
//! `texte-2027/bloc-S21.G00.63.txt` (les valeurs de 63.001 ; l'extrait 2026.1 s'arrete a la page 279) et
//! `atelier/reference/sources/code-du-travail/articles-20261013.md` (L1234-5, L1226-14, L1221-25, L1221-26).

use crate::outils::{self, compagnie};
use paie::contexte::Contexte;
use paie::date::Jour;
use paie::engagement::charger;
use paie::fctu::{self, Echec};
use serde_json::{json, Value};
use std::path::PathBuf;

const DEMO: &str = "fictive-3090-grande";
const JOUR: &str = "2026-10-20";

/// Une copie de la compagnie fictive ou le CDI de Montansier (MO.23.01, depuis le 04/09/2023, 3 120 EUR) finit le 30/06/2026
/// pour `motif`, avec les `champs` poses sur son engagement de juin.
fn fin_du_cdi(test: &str, motif: &str, champs: Value) -> PathBuf {
    let racine = compagnie::ses_dsn(DEMO, &[], &outils::dossier_de_test("preavis", test));
    let mut saisie = outils::lire(&racine, "paies/2026-06.json");
    let e = saisie["engagements"].as_array_mut().unwrap().iter_mut().find(|e| e["numero"] == "MO.23.01").unwrap();
    e["fin"] = "2026-06-30".into();
    e["motif_rupture"] = motif.into();
    for (k, v) in champs.as_object().unwrap() {
        e[k] = v.clone();
    }
    outils::ecrire(&racine, "paies/2026-06.json", &saisie);
    racine
}

/// Les codes S21.G00.52.001 du versement du contrat `numero` dans la mensuelle de juin 2026.
fn codes_52(racine: &PathBuf, numero: &str) -> Vec<String> {
    let dsn = outils::dsn_du_mois(racine, "2026-06", JOUR).unwrap_or_else(|m| panic!("{}", m));
    outils::versement(&dsn, numero).into_iter().filter(|(k, _)| k == "S21.G00.52.001").map(|(_, v)| v).collect()
}

/// Un licenciement dont le preavis (trois mois saisis) n'est pas effectue, a 1 500 EUR par mois (le brut du mois reste sous
/// les quatre plafonds que le CTP 772 de la compagnie fictive ne sait pas plafonner).
fn licenciement_sans_preavis_effectue(test: &str, motif: &str) -> PathBuf {
    fin_du_cdi(
        test,
        motif,
        json!({"notification_rupture": "2026-05-29", "engagement_licenciement": "2026-05-15",
               "preavis_non_effectue": true, "preavis_mois": 3, "salaire_mensuel": 1500}),
    )
}

#[test]
fn le_preavis_non_effectue_d_une_inaptitude_professionnelle_se_declare_en_032() {
    // 092 : L1226-14, une indemnite « d'un montant egal a celui de l'indemnite compensatrice de preavis » ; le bloc 52 a
    // son code, « 032 - Indemnite compensatrice de preavis pour inaptitude suite AT ou Maladie Professionnelle » (p. 253),
    // permis avec 092 (CCH-29, p. 252). Le moteur ecrivait 023.
    let codes = codes_52(&licenciement_sans_preavis_effectue("092", "092"), "MO.23.01");
    assert!(codes.iter().any(|c| c == "032"), "{:?}", codes);
    assert!(!codes.iter().any(|c| c == "023"), "{:?}", codes);
}

#[test]
fn le_preavis_non_effectue_d_un_licenciement_se_declare_en_023() {
    // Le temoin : 020, L1234-5, « 023 - Indemnite compensatrice de preavis paye non effectue ».
    let codes = codes_52(&licenciement_sans_preavis_effectue("020", "020"), "MO.23.01");
    assert!(codes.iter().any(|c| c == "023"), "{:?}", codes);
    assert!(!codes.iter().any(|c| c == "032"), "{:?}", codes);
}

#[test]
fn le_code_032_n_accompagne_qu_une_inaptitude() {
    // CCH-29 : avec 032, le motif de rupture est 033, 091, 092 ou 099 ; un 032 avec un licenciement ordinaire refuse.
    assert_eq!(paie::permanents::code_52_permis(Some("032"), Some("092")).unwrap(), None);
    assert_eq!(paie::permanents::code_52_permis(Some("032"), Some("033")).unwrap(), None);
    let refus = paie::permanents::code_52_permis(Some("032"), Some("020")).unwrap().expect("032 avec 020");
    assert!(refus.contains("CCH-29") && refus.contains("092"), "{}", refus);
}

// ------------------------------------------------ le type de preavis du signalement et ce que la paie verse (lot P2)
// S21.G00.63.001 (cahier technique 2027.1.1, p. 283-284, `texte-2027/bloc-S21.G00.63.txt` l. 98-106) : « 02 - preavis
// non effectue et paye », « 03 - preavis non effectue et non paye » ; `fctu-base.json`, `types_preavis._indemnite_de_preavis`.

/// Le contrat `numero` de la copie, sa paie de juin 2026 (le solde de tout compte), et le signalement de sa fin pour
/// `motif` avec le type de preavis `type_preavis` (du 01/06 au 30/06, notifie le 29/05).
fn paie_et_fctu(racine: &PathBuf, numero: &str, motif: &str, type_preavis: &str) -> (paie::calcul::Paie, Result<String, Echec>) {
    let ctx = Contexte::nouveau(racine, Some(Jour::lire(JOUR).unwrap())).unwrap();
    let s = charger(&ctx, &racine.join("paies/2026-06.json")).unwrap_or_else(|m| panic!("{}", m));
    let paie = s.paies.into_iter().find(|p| p.engagement.numero.as_deref() == Some(numero)).unwrap();
    let fin = fctu::FinDeContrat::lire(&json!({
        "fin": "2026-06-30", "motif": motif, "mois_rattachement": "2026-06", "type_preavis": type_preavis,
        "preavis_debut": "2026-06-01", "preavis_fin": "2026-06-30", "notification": "2026-05-29",
    }))
    .unwrap();
    let op = fctu::Options { envoi_test: true, date_constitution: Some(Jour::lire("2026-10-02").unwrap()), ..Default::default() };
    let texte = fctu::generer(&ctx, &paie, &fin, &op, &[], &[]);
    (paie, texte)
}

/// Le CDI de Montansier fini pour `motif`, preavis declare non effectue (`preavis_non_effectue`, trois mois), du type
/// `type_preavis` dans la saisie, le dernier jour paye au salaire habituel le 31/05.
fn cdi_preavis_non_effectue(test: &str, motif: &str, type_preavis: &str) -> PathBuf {
    fin_du_cdi(
        test,
        motif,
        json!({"notification_rupture": "2026-05-29", "engagement_licenciement": "2026-05-15", "dernier_jour_paye": "2026-05-31",
               "preavis_non_effectue": true, "preavis_mois": 3, "salaire_mensuel": 1500,
               "preavis": {"type": type_preavis, "debut": "2026-06-01", "fin": "2026-06-30"}}),
    )
}

fn avis(paie: &paie::calcul::Paie) -> String {
    paie.avertissements.iter().map(|a| a.texte.clone()).collect::<Vec<_>>().join("\n")
}

#[test]
fn un_cdd_rompu_par_le_salarie_ne_se_declare_pas_preavis_paye() {
    // 037 : aucune indemnite (L1243-2, L1243-3 ; lot P1) ; « 02 - non effectue et paye » dirait le contraire de la paie.
    let racine = compagnie::ses_dsn(DEMO, &[], &outils::dossier_de_test("preavis", "037-02"));
    let mut saisie = outils::lire(&racine, "paies/2026-06.json");
    let e = saisie["engagements"].as_array_mut().unwrap().iter_mut().find(|e| e["numero"] == "AU.25.01").unwrap();
    for (k, v) in [
        ("motif_rupture", json!("037")),
        ("notification_rupture", json!("2026-05-29")),
        ("dernier_jour_paye", json!("2026-05-31")),
        ("preavis_non_effectue", json!(true)),
        ("preavis", json!({"type": "02", "debut": "2026-06-01", "fin": "2026-06-30"})),
    ] {
        e[k] = v;
    }
    outils::ecrire(&racine, "paies/2026-06.json", &saisie);
    let (paie, fctu) = paie_et_fctu(&racine, "AU.25.01", "037", "02");
    match fctu {
        Err(Echec::Impossible(m)) => assert!(m.contains("« 02 - ") && m.contains("aucune indemnite") && m.contains("« 03 - "), "{}", m),
        r => panic!("un 037 « paye » sans indemnite est passe : {:?}", r.map(|t| outils::valeur(&outils::paires(&t), "S21.G00.63.001"))),
    }
    assert!(avis(&paie).contains("le preavis se declare « 02 - "), "{}", avis(&paie));
}

#[test]
fn un_licenciement_dont_le_preavis_est_paye_se_declare_02_avec_son_023() {
    let racine = cdi_preavis_non_effectue("020-02", "020", "02");
    let (paie, fctu) = paie_et_fctu(&racine, "MO.23.01", "020", "02");
    let texte = fctu.unwrap_or_else(|e| panic!("{}", e.message()));
    let v = outils::paires(&texte);
    assert_eq!(outils::valeur(&v, "S21.G00.63.001").as_deref(), Some("02"));
    assert!(v.iter().any(|(k, x)| k == "S21.G00.52.001" && x == "023"), "{}", texte);
    assert!(!avis(&paie).contains("le preavis se declare"), "{}", avis(&paie));
}

#[test]
fn un_preavis_paye_ne_se_declare_ni_non_paye_ni_effectue() {
    // La paie verse l'indemnite (020, L1234-5) : « 03 - non effectue et non paye » la contredit ; « 01 - effectue et
    // paye » aussi (lecture : 023 est l'indemnite d'un preavis « paye non effectue », et le moteur n'ecrit qu'un bloc 63).
    for type_preavis in ["03", "01"] {
        let racine = cdi_preavis_non_effectue(&format!("020-{}", type_preavis), "020", type_preavis);
        let (paie, fctu) = paie_et_fctu(&racine, "MO.23.01", "020", type_preavis);
        match fctu {
            Err(Echec::Impossible(m)) => {
                assert!(m.contains(&format!("« {} - ", type_preavis)) && m.contains("(023)") && m.contains("« 02 - "), "{}", m)
            }
            r => panic!("{} avec une indemnite versee est passe : {:?}", type_preavis, r.is_ok()),
        }
        assert!(avis(&paie).contains(&format!("le preavis se declare « {} - ", type_preavis)), "{}", avis(&paie));
    }
}

#[test]
fn une_demission_dont_le_preavis_n_est_pas_effectue_se_declare_03_sans_indemnite() {
    // 059 : aucune indemnite (L1237-1, lot P1) ; « 03 - non effectue et non paye » dit ce que la paie fait.
    let racine = cdi_preavis_non_effectue("059-03", "059", "03");
    let (paie, fctu) = paie_et_fctu(&racine, "MO.23.01", "059", "03");
    let texte = fctu.unwrap_or_else(|e| panic!("{}", e.message()));
    let v = outils::paires(&texte);
    assert_eq!(outils::valeur(&v, "S21.G00.63.001").as_deref(), Some("03"));
    assert!(!v.iter().any(|(k, x)| k == "S21.G00.52.001" && (x == "023" || x == "032")), "{}", texte);
    assert!(!avis(&paie).contains("le preavis se declare"), "{}", avis(&paie));
}

// ------------------------------------------------ le delai de prevenance d'une fin d'essai (L1221-25, lot P2)

#[test]
fn l_indemnite_du_delai_de_prevenance_se_declare_en_017() {
    // Le CDD AU.25.01 (depuis le 01/09/2025, 1 980 EUR, un essai stipule de trente jours) : fin d'essai par l'employeur
    // (034) notifiee le 20/06/2026, plus de trois mois de presence, un mois de delai jusqu'au 20/07 ; le contrat finit le
    // 30/06 : vingt jours de juillet. Bloc 52 : « 017 - Dommages et interets dus [...] a une rupture de periode d'essai
    // (delai de prevenance) » (cahier technique 2026.1, p. 253).
    let racine = compagnie::ses_dsn(DEMO, &[], &outils::dossier_de_test("preavis", "034"));
    let mut saisie = outils::lire(&racine, "paies/2026-06.json");
    let e = saisie["engagements"].as_array_mut().unwrap().iter_mut().find(|e| e["numero"] == "AU.25.01").unwrap();
    e["motif_rupture"] = "034".into();
    e["notification_rupture"] = "2026-06-20".into();
    e["periode_essai_jours"] = 30.into();
    outils::ecrire(&racine, "paies/2026-06.json", &saisie);
    let dsn = outils::dsn_du_mois(&racine, "2026-06", JOUR).unwrap_or_else(|m| panic!("{}", m));
    let v = outils::versement(&dsn, "AU.25.01");
    let i = v.iter().position(|(k, x)| k == "S21.G00.52.001" && x == "017").unwrap_or_else(|| panic!("{:?}", v));
    assert_eq!(v[i + 1], ("S21.G00.52.002".to_string(), "1277.42".to_string()), "20/31 de 1 980 EUR");
}
