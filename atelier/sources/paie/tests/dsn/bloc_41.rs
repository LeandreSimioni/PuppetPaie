//! Corrections DSN, vague V3c : la paie et la DSN d'un permanent lisent sa fiche a la date du mois paye, et le bloc
//! « Changements contrat - S21.G00.41 » s'ecrit quand une modalite du contrat change.
//!
//! - Un contrat paye au mois (CDI, CDD de droit commun) lit l'emploi, la PCS, le statut et la categorie cadre de la
//!   fiche TELS QU'AU MOIS PAYE, plus tels qu'au premier jour du contrat : un passage cadre date sur la fiche atteint le
//!   bareme et le bloc 40 du mois ou il prend effet. Un changement en cours de mois decoupe la paie en periodes
//!   elementaires (lot R10, `tests/periodes.rs`).
//! - Cahier technique DSN 2026.1 (`atelier/reference/norme-dsn/texte/`) : le bloc 41 porte l'ANCIENNE valeur
//!   (`bloc-S21.G00.41.txt`, p. 215), datee en 41.001, une modalite par bloc (CCH-13, § 4.4.14.2 a, p. 103), jamais un
//!   identifiant et une modalite ensemble (CCH-16, principe b), 41.028 sur le bloc le plus ancien d'une modalite
//!   (principe c) et jamais pour un identifiant (principe d) ; « Le changement d'une des modalites visees doit
//!   imperativement etre declare via un bloc « Changements Contrat - S21.G00.41 » » (§ 2.1.2.5.2, p. 25).
//!
//! Sur la copie figee de `fictive-1285-grande` (`tests/donnees/fictives/`, compagnie inventee : deux CDI, NADAR depuis le
//! 03/01/2022, cadre, et DORVAL depuis le 01/04/2025, non cadre ; toutes les DSN deposees jusqu'a aout 2026), copiee
//! dans le dossier temporaire des tests avant d'y retoucher les fiches et les saisies.

use crate::outils::permanents::{bloc, blocs_41, contexte, deposer, dorval_cadre_au, dsn, retoucher_saisie, saisie};
use crate::outils::{self, compagnie, individu};
use paie::dsn::{generer, Options};
use engagement::{charger, Saisie};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Une copie de la 1285-grande, sans ses pieces, propre a un test.
fn copie(test: &str) -> PathBuf {
    compagnie::ses_dsn("fictive-1285-grande", &[], &outils::dossier_de_test("bloc_41", test))
}

fn retoucher_fiche(racine: &Path, cle: &str, champs: Value) {
    outils::retoucher_fiches(racine, &[(cle, champs)]);
}

/// Les identifiants des lignes de cotisation de la paie de `cle` (cle de la fiche).
fn lignes_de(s: &Saisie, cle: &str) -> Vec<String> {
    let p = s.paies.iter().find(|p| p.salarie["_cle"] == cle).unwrap_or_else(|| panic!("pas de paie pour {}", cle));
    p.lignes.iter().map(|l| l.id.clone()).collect()
}

fn valeur(lignes: &[(String, String)], r: &str) -> String {
    outils::valeur(lignes, r).unwrap_or_default()
}

// ------------------------------------------------------------------ point 1 : la fiche a la date du mois

#[test]
fn un_cdi_qui_passe_cadre_au_1er_du_mois_cotise_cadre_ce_mois_la_et_pas_avant() {
    let racine = copie("cadre-bareme");
    dorval_cadre_au(&racine, "2026-08-31");
    let sept = saisie(&racine, "2026-09").unwrap();
    assert!(lignes_de(&sept, "DORVAL").iter().any(|l| l == "apec_t1"), "septembre : la pile du cadre permanent (Apec) : {:?}", lignes_de(&sept, "DORVAL"));
    let aout = saisie(&racine, "2026-08").unwrap();
    assert!(!lignes_de(&aout, "DORVAL").iter().any(|l| l == "apec_t1"), "aout : encore non cadre");
    let d = individu(&dsn(&racine, "2026-09").unwrap(), "DORVAL");
    assert_eq!((valeur(&d, "S21.G00.40.002"), valeur(&d, "S21.G00.40.003")), ("04".into(), "01".into()));
}

#[test]
fn un_cdi_de_2022_qui_change_de_metier_declare_le_metier_du_mois() {
    // NADAR, administratrice de production depuis 2022, devient directrice de production au 01/09/2026 (meme PCS
    // 353B, cadre). Le libelle de l'emploi (40.006) n'a pas de rubrique en bloc 41 : il se change en reecrivant le
    // bloc 40 (§ 2.1.2.4, p. 21-22).
    let racine = copie("nadar-metier");
    retoucher_fiche(
        &racine,
        "NADAR",
        json!({
            "emploi": "DIRECTEUR DE PRODUCTION",
            "emploi_affichage": "Directrice de production",
            "code_emploi_ft": "DIR120",
            "historique": [
                {"champ": "code_emploi_ft", "valeur": "ADM020", "jusqu_au": "2026-08-31", "note": ""},
                {"champ": "emploi", "valeur": "ADMINISTRATEUR DE PRODUCTION", "jusqu_au": "2026-08-31", "note": ""},
                {"champ": "emploi_affichage", "valeur": "Administratrice de production", "jusqu_au": "2026-08-31", "note": ""}
            ]
        }),
    );
    let n = individu(&dsn(&racine, "2026-09").unwrap(), "NADAR");
    assert_eq!(valeur(&n, "S21.G00.40.006"), "DIRECTEUR DE PRODUCTION");
    assert_eq!(valeur(&n, "S21.G00.40.004"), "353B");
    // Un seul emploi : le contrat relu sans etre paye (`amorcage::embauche`, que les emplois multiples comptent) lit la
    // fiche comme la paie, au mois paye (avant, au premier jour du contrat : deux metiers, et 02).
    assert_eq!(valeur(&n, "S21.G00.40.036"), "01");
    let a = individu(&dsn(&racine, "2026-08").unwrap(), "NADAR");
    assert_eq!(valeur(&a, "S21.G00.40.006"), "ADMINISTRATEUR DE PRODUCTION", "aout garde l'ancien metier");
}

// ------------------------------------------------------------------ point 2 : le bloc 41

#[test]
fn un_passage_cadre_declare_l_ancien_statut_en_bloc_41_et_le_mois_suivant_n_en_porte_plus() {
    let racine = copie("cadre-bloc-41");
    dorval_cadre_au(&racine, "2026-08-31");
    let sept = dsn(&racine, "2026-09").unwrap();
    let d = individu(&sept, "DORVAL");
    assert_eq!(
        blocs_41(&d),
        vec![
            bloc(&[("S21.G00.41.001", "01092026"), ("S21.G00.41.002", "05"), ("S21.G00.41.028", "01092026")]),
            bloc(&[("S21.G00.41.001", "01092026"), ("S21.G00.41.003", "04"), ("S21.G00.41.028", "01092026")]),
        ],
        "une modalite par bloc (CCH-13), l'ancienne valeur, la date, la profondeur : {:?}",
        d
    );
    // Le bloc 41 suit le bloc 40 du contrat, avant ses affiliations.
    let rang = |r: &str| d.iter().position(|(k, _)| k.starts_with(r)).unwrap();
    assert!(rang("S21.G00.40.") < rang("S21.G00.41.") && rang("S21.G00.41.") < rang("S21.G00.70."));
    assert!(blocs_41(&individu(&sept, "NADAR")).is_empty(), "NADAR ne change rien");
    paie::usages::verifier_fichier(&sept).unwrap();
    paie::formats::verifier_fichier(&sept).unwrap();
    // Septembre depose : octobre declare 04 / 01 sans bloc 41.
    deposer(&racine, "2026-09", "DSN_Septembre2026.dsn", &sept);
    // Octobre n'est pas produit dans la compagnie fictive : ses CDDU n'ont pas de numero ; seuls les deux CDI restent.
    let mut s = outils::lire(&racine, "paies/2026-10.json");
    s["engagements"].as_array_mut().unwrap().retain(|e| e["nature"] == "cdi");
    outils::ecrire(&racine, "paies/2026-10.json", &s);
    let oct = dsn(&racine, "2026-10").unwrap();
    assert!(!oct.contains("S21.G00.41."), "octobre ne redeclare pas le changement");
    let d = individu(&oct, "DORVAL");
    assert_eq!((valeur(&d, "S21.G00.40.002"), valeur(&d, "S21.G00.40.003")), ("04".into(), "01".into()));
}

#[test]
fn un_changement_de_metier_sans_rubrique_en_bloc_41_reecrit_le_bloc_40_seulement() {
    let racine = copie("nadar-sans-41");
    retoucher_fiche(
        &racine,
        "NADAR",
        json!({
            "emploi": "DIRECTEUR DE PRODUCTION",
            "code_emploi_ft": "DIR120",
            "historique": [
                {"champ": "code_emploi_ft", "valeur": "ADM020", "jusqu_au": "2026-08-31", "note": ""},
                {"champ": "emploi", "valeur": "ADMINISTRATEUR DE PRODUCTION", "jusqu_au": "2026-08-31", "note": ""}
            ]
        }),
    );
    let n = individu(&dsn(&racine, "2026-09").unwrap(), "NADAR");
    assert_eq!(valeur(&n, "S21.G00.40.006"), "DIRECTEUR DE PRODUCTION");
    assert!(blocs_41(&n).is_empty(), "le libelle de l'emploi n'a pas de rubrique en bloc 41 (p. 22) : {:?}", blocs_41(&n));
}

#[test]
fn un_avenant_de_temps_de_travail_ecrit_41_007_et_41_008_en_deux_blocs() {
    let racine = copie("quotite");
    // Aout : DORVAL passe a temps partiel (121,34 h) par avenant au 01/08 ; juillet a ete declare a temps plein.
    retoucher_saisie(&racine, "2026-08", "Dorval", json!({"quotite_contrat": 121.34, "date_modification": "2026-08-01"}));
    let aout = dsn(&racine, "2026-08").unwrap();
    let d = individu(&aout, "DORVAL");
    assert_eq!((valeur(&d, "S21.G00.40.013"), valeur(&d, "S21.G00.40.014")), ("121.34".into(), "20".into()));
    assert_eq!(
        blocs_41(&d),
        vec![
            bloc(&[("S21.G00.41.001", "01082026"), ("S21.G00.41.007", "151.67"), ("S21.G00.41.028", "01082026")]),
            bloc(&[("S21.G00.41.001", "01082026"), ("S21.G00.41.008", "10"), ("S21.G00.41.028", "01082026")]),
        ]
    );
    deposer(&racine, "2026-08", "DSN_Aout2026.dsn", &aout);
    // Septembre : retour a temps plein (151,67 h) par avenant au 01/09.
    retoucher_saisie(&racine, "2026-09", "Dorval", json!({"date_modification": "2026-09-01"}));
    let d = individu(&dsn(&racine, "2026-09").unwrap(), "DORVAL");
    assert_eq!((valeur(&d, "S21.G00.40.013"), valeur(&d, "S21.G00.40.014")), ("151.67".into(), "10".into()));
    assert_eq!(
        blocs_41(&d),
        vec![
            bloc(&[("S21.G00.41.001", "01092026"), ("S21.G00.41.007", "121.34"), ("S21.G00.41.028", "01092026")]),
            bloc(&[("S21.G00.41.001", "01092026"), ("S21.G00.41.008", "20"), ("S21.G00.41.028", "01092026")]),
        ]
    );
}

#[test]
fn un_changement_de_quotite_sans_date_refuse() {
    let racine = copie("quotite-sans-date");
    retoucher_saisie(&racine, "2026-09", "Dorval", json!({"quotite_contrat": 121.34}));
    let e = dsn(&racine, "2026-09").unwrap_err();
    assert!(e.contains("S21.G00.40.013") && e.contains("date_modification") && e.contains("DORVAL"), "{}", e);
}

#[test]
fn un_changement_de_date_de_debut_se_declare_en_41_021_s_il_est_date_et_refuse_sinon() {
    let racine = copie("debut");
    retoucher_saisie(&racine, "2026-09", "Dorval", json!({"debut": "2025-04-02"}));
    let e = dsn(&racine, "2026-09").unwrap_err();
    assert!(e.contains("S21.G00.40.001") && e.contains("S21.G00.41.021") && e.contains("date_modification"), "{}", e);
    retoucher_saisie(&racine, "2026-09", "Dorval", json!({"date_modification": "2026-09-10"}));
    let d = individu(&dsn(&racine, "2026-09").unwrap(), "DORVAL");
    assert_eq!(valeur(&d, "S21.G00.40.001"), "02042025");
    assert_eq!(
        blocs_41(&d),
        vec![bloc(&[("S21.G00.41.001", "10092026"), ("S21.G00.41.021", "01042025")])],
        "un identifiant : seul dans son bloc (CCH-16), sans profondeur de recalcul (principe d)"
    );
}

#[test]
fn un_passage_cadre_date_avant_le_mois_deja_declare_refuse() {
    // La fiche dit cadre depuis le 01/08, mais aout est depose non cadre : une correction de mois deja declares.
    let racine = copie("cadre-tardif");
    dorval_cadre_au(&racine, "2026-07-31");
    let e = dsn(&racine, "2026-09").unwrap_err();
    assert!(e.contains("01/08/2026") && e.contains("correction"), "{}", e);
}

#[test]
fn une_mutation_datee_declare_l_ancien_siret_seul_puis_le_lieu_et_le_code_risque_en_modalites() {
    // DORVAL passe au 01/09/2026 a l'etablissement 00021 (Lyon, code risque 923AC) ; aout a ete declare par le
    // principal (00013, 923AD). Le SIRET est un identifiant : seul dans son bloc, sans profondeur (CCH-16, principe d),
    // date du jour de la mutation (p. 21). Le lieu de travail (40.019) et le code risque (40.040) sont des modalites :
    // un bloc chacune, avec 41.028.
    let racine = copie("mutation");
    let mut ent = outils::lire(&racine, "config/entreprise.json");
    ent["etablissements"] = json!([{
        "nic": "00021", "naf": ent["naf"].clone(), "adresse": "1 RUE DE L'ESSAI", "code_postal": "69002", "commune": "LYON",
        "taux_atmp": {"valeur": 1.1, "code_risque": "923AC"}
    }]);
    outils::ecrire(&racine, "config/entreprise.json", &ent);
    retoucher_fiche(
        &racine,
        "DORVAL",
        json!({"etablissement": "00021", "historique": [{"champ": "etablissement", "valeur": "", "jusqu_au": "2026-08-31", "note": ""}]}),
    );
    let ctx = contexte(&racine, "2026-09");
    let s = charger(&ctx, &racine.join("paies/2026-09.json")).unwrap();
    let siens: Vec<_> = s.paies.iter().filter(|p| p.salarie["_cle"] == "DORVAL").cloned().collect();
    let o = Options { etablissement: Some("00021".into()), ..Options::default() };
    let texte = generer(&ctx, &siens, "2026-09", &[], &[], &o).unwrap();
    let d = individu(&texte, "DORVAL");
    assert_eq!(valeur(&d, "S21.G00.40.019"), "12345693100021");
    assert_eq!(
        blocs_41(&d),
        vec![
            bloc(&[("S21.G00.41.001", "01092026"), ("S21.G00.41.012", "12345693100013")]),
            bloc(&[("S21.G00.41.001", "01092026"), ("S21.G00.41.013", "12345693100013"), ("S21.G00.41.028", "01092026")]),
            bloc(&[("S21.G00.41.001", "01092026"), ("S21.G00.41.024", "923AD"), ("S21.G00.41.028", "01092026")]),
        ],
        "{:?}",
        d
    );
    paie::usages::verifier_fichier(&texte).unwrap();
    paie::formats::verifier_fichier(&texte).unwrap();
    // La meme mutation, non datee : refus.
    retoucher_fiche(&racine, "DORVAL", json!({"historique": null}));
    let ctx = contexte(&racine, "2026-09");
    let s = charger(&ctx, &racine.join("paies/2026-09.json")).unwrap();
    let siens: Vec<_> = s.paies.iter().filter(|p| p.salarie["_cle"] == "DORVAL").cloned().collect();
    let e = generer(&ctx, &siens, "2026-09", &[], &[], &o).unwrap_err();
    assert!(e.contains("S21.G00.41.012") && e.contains("12345693100013"), "{}", e);
}

#[test]
fn un_second_emploi_dans_le_mois_declare_l_ancien_code_emplois_multiples_et_le_mois_suivant_aussi() {
    // DORVAL (CDI, chargee de production) tient aussi un CDDU de regisseuse lumiere les 29 et 30/09/2026 : son CDI
    // passe de « 01 - emploi unique » (aout) a « 02 - emplois multiples » (p. 199), une modalite (41.061, § 4.4.14.1,
    // p. 103), datee du premier jour de la periode payee ; en octobre, elle revient a 01.
    let racine = copie("emplois-multiples");
    let mut s = outils::lire(&racine, "paies/2026-09.json");
    let mut cddu = s["engagements"].as_array().unwrap().iter().find(|e| e["salarie"] == "Torelli").unwrap().clone();
    cddu["salarie"] = json!("Dorval");
    cddu["numero"] = json!("DO7.26.09");
    cddu["metier"] = json!("REG130");
    s["engagements"].as_array_mut().unwrap().push(cddu);
    outils::ecrire(&racine, "paies/2026-09.json", &s);
    let sept = dsn(&racine, "2026-09").unwrap();
    let d = individu(&sept, "DORVAL");
    let cdi: Vec<(String, String)> = {
        // Le bloc du CDI : de son 40.001 au contrat suivant.
        let i = d.iter().position(|(k, v)| k == "S21.G00.40.009" && v == "DO.25.01").unwrap();
        let debut = d[..i].iter().rposition(|(k, _)| k == "S21.G00.40.001").unwrap();
        let fin = d[i..].iter().position(|(k, _)| k == "S21.G00.40.001").map_or(d.len(), |j| i + j);
        d[debut..fin].to_vec()
    };
    assert_eq!(valeur(&cdi, "S21.G00.40.036"), "02");
    assert_eq!(blocs_41(&cdi), vec![bloc(&[("S21.G00.41.001", "01092026"), ("S21.G00.41.061", "01"), ("S21.G00.41.028", "01092026")])]);
    assert_eq!(blocs_41(&d).len(), 1, "le CDDU commence ce mois-ci : pas de bloc 41 : {:?}", d);
    paie::usages::verifier_fichier(&sept).unwrap();
    paie::formats::verifier_fichier(&sept).unwrap();
    deposer(&racine, "2026-09", "DSN_Septembre2026.dsn", &sept);
    let mut s = outils::lire(&racine, "paies/2026-10.json");
    s["engagements"].as_array_mut().unwrap().retain(|e| e["nature"] == "cdi");
    outils::ecrire(&racine, "paies/2026-10.json", &s);
    let d = individu(&dsn(&racine, "2026-10").unwrap(), "DORVAL");
    assert_eq!(valeur(&d, "S21.G00.40.036"), "01");
    assert_eq!(blocs_41(&d), vec![bloc(&[("S21.G00.41.001", "01102026"), ("S21.G00.41.061", "02"), ("S21.G00.41.028", "01102026")])]);
}
