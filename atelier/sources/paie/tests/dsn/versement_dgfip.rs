//! Corrections DSN, vague 1d (point 21 de la relecture de la norme, fiche N3 du bloc 20) : le prelevement a la source
//! retenu se reverse a la DGFiP par un bloc 20 a part.
//!
//! Norme DSN 2026.1, `prive/reference/norme-dsn/texte/bloc-S21.G00.20.txt` : 20.001 = « DGFIP » (p. 154), 20.002 =
//! « DGFIP_PAS » (p. 155-156, CCH-11), 20.005 « arrondi a l'entier le plus proche » et, sur un annule et remplace sans
//! PAS d'une declaration qui en versait, un bloc « renseigne a zero » (p. 157), 20.006 et 20.007 le premier et le
//! dernier jour du mois principal declare (p. 158-159), 20.010 = 05 ou 06 (CCH-14, p. 161), 20.011 non concerne
//! (p. 161), 20.012 interdite en prelevement SEPA (CCH-13, p. 162).
//!
//! Sur la copie figee des demonstrations (`tests/donnees/demo/`, compagnie inventee).

use crate::outils::{self, compagnie, dec, demo, paires as lignes};
use paie::contexte::Contexte;
use paie::date::Jour;
use paie::dsn::{generer, Options};
use paie::engagement::charger;
use paie::num::Decimal;
use rust_decimal::RoundingStrategy;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn contexte() -> Contexte {
    Contexte::nouveau(&demo("demo-3090-grande"), Some(Jour::lire("2026-07-28").unwrap())).unwrap()
}

/// Les blocs 20 de la DGFiP : chacun, ses rubriques jusqu'au bloc suivant (20.001 suivant ou autre bloc que 20).
fn blocs_dgfip(dsn: &str) -> Vec<Vec<(String, String)>> {
    let l = lignes(dsn);
    let mut out = Vec::new();
    for (i, (k, v)) in l.iter().enumerate() {
        if k == "S21.G00.20.001" && v == "DGFIP" {
            let fin = l[i + 1..]
                .iter()
                .position(|(k, _)| k == "S21.G00.20.001" || !k.starts_with("S21.G00.20."))
                .map_or(l.len(), |p| i + 1 + p);
            out.push(l[i..fin].to_vec());
        }
    }
    out
}

fn val<'a>(b: &'a [(String, String)], r: &str) -> Option<&'a str> {
    b.iter().find(|(k, _)| k == r).map(|(_, v)| v.as_str())
}

/// Le PAS retenu, relu dans le nominatif : 50.009 et 56.007.
fn pas_du_nominatif(dsn: &str) -> Decimal {
    lignes(dsn).iter().filter(|(k, _)| k == "S21.G00.50.009" || k == "S21.G00.56.007").fold(Decimal::ZERO, |s, (_, v)| s + dec(v))
}

fn a_l_euro(x: Decimal) -> Decimal {
    let mut m = x.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero);
    m.rescale(2);
    m
}

/// Tous les salaries du mois a un taux DGFiP nul.
fn pas_nul(paies: &[paie::calcul::Paie]) -> Options {
    let mut o = Options::default();
    for p in paies {
        o.pas_par_salarie.insert(p.salarie["_cle"].as_str().unwrap().to_string(), Some((json!("0.00"), json!("01"), json!("891000000"))));
    }
    o
}

#[test]
fn un_mois_avec_du_pas_retenu_le_reverse_a_la_dgfip() {
    let ctx = contexte();
    let s = charger(&ctx, &demo("demo-3090-grande").join("paies/2026-07.json")).unwrap();
    let dsn = generer(&ctx, &s.paies, "2026-07", &[], &[], &Options::default()).unwrap();
    let retenu = pas_du_nominatif(&dsn);
    assert!(retenu > Decimal::ZERO, "la demonstration retient du PAS en juillet 2026");
    let blocs = blocs_dgfip(&dsn);
    assert_eq!(blocs.len(), 1, "un versement DGFiP, un seul");
    let b = &blocs[0];
    assert_eq!(val(b, "S21.G00.20.002"), Some("DGFIP_PAS"), "CCH-11, p. 156");
    assert_eq!(dec(val(b, "S21.G00.20.005").unwrap()), a_l_euro(retenu), "le PAS retenu, arrondi a l'euro (p. 157)");
    assert!(val(b, "S21.G00.20.005").unwrap().ends_with(".00"), "les deux decimales a zero (p. 157)");
    assert_eq!(val(b, "S21.G00.20.006"), Some("01072026"), "premier jour du mois principal declare (p. 158)");
    assert_eq!(val(b, "S21.G00.20.007"), Some("31072026"), "dernier jour du mois principal declare (p. 159)");
    assert_eq!(val(b, "S21.G00.20.010"), Some("05"), "prelevement SEPA (CCH-14, p. 161)");
    let ent = &ctx.entreprise["banque"];
    assert_eq!(val(b, "S21.G00.20.003"), ent["bic"].as_str());
    assert_eq!(val(b, "S21.G00.20.004"), ent["iban"].as_str());
    assert_eq!(val(b, "S21.G00.20.011"), None, "DGFiP : non concerne (p. 161)");
    assert_eq!(val(b, "S21.G00.20.012"), None, "interdite en prelevement SEPA (CCH-13, p. 162)");
    // Aucun bloc 55 ne suit : il est reserve aux organismes complementaires.
    let l = lignes(&dsn);
    let i = l.iter().position(|(k, v)| k == "S21.G00.20.001" && v == "DGFIP").unwrap();
    assert!(!l[i + 1..i + b.len() + 1].iter().any(|(k, _)| k.starts_with("S21.G00.55.")));
}

#[test]
fn un_mois_a_pas_nul_n_ecrit_pas_de_versement_dgfip() {
    let ctx = contexte();
    let s = charger(&ctx, &demo("demo-3090-grande").join("paies/2026-07.json")).unwrap();
    let dsn = generer(&ctx, &s.paies, "2026-07", &[], &[], &pas_nul(&s.paies)).unwrap();
    assert_eq!(pas_du_nominatif(&dsn), Decimal::ZERO);
    assert!(blocs_dgfip(&dsn).is_empty(), "rien a reverser : aucun bloc DGFiP sur une DSN normale");
}

#[test]
fn un_annule_et_remplace_sans_pas_ecrit_un_versement_a_zero_si_la_declaration_remplacee_en_portait_un() {
    let ctx = contexte();
    let s = charger(&ctx, &demo("demo-3090-grande").join("paies/2026-07.json")).unwrap();
    let ar = |versait: Option<bool>| {
        let mut o = pas_nul(&s.paies);
        o.type_declaration = "03".into();
        o.ordre = 2;
        o.remplacee_versait_pas = versait;
        generer(&ctx, &s.paies, "2026-07", &[], &[], &o)
    };
    let dsn = ar(Some(true)).unwrap();
    let blocs = blocs_dgfip(&dsn);
    assert_eq!(blocs.len(), 1);
    assert_eq!(val(&blocs[0], "S21.G00.20.005"), Some("0.00"), "« renseignee a zero » (p. 157)");
    assert!(blocs_dgfip(&ar(Some(false)).unwrap()).is_empty(), "la declaration remplacee ne versait rien : rien a annuler");
    let refus = ar(None).unwrap_err();
    assert!(refus.contains("DSN remplacee n'a pas pu etre relue"), "{}", refus);
}

#[test]
fn une_dsn_sans_individu_ne_verse_rien_a_la_dgfip_sauf_a_remplacer_un_versement() {
    let ctx = contexte();
    let dsn = generer(&ctx, &[], "2026-07", &[], &[], &Options::default()).unwrap();
    assert!(blocs_dgfip(&dsn).is_empty());
    let o = Options { type_declaration: "03".into(), ordre: 2, remplacee_versait_pas: Some(true), ..Options::default() };
    let dsn = generer(&ctx, &[], "2026-07", &[], &[], &o).unwrap();
    assert!(dsn.contains("S20.G00.05.002,'05'"), "sans individu, l'annule et remplace est de type 05");
    let blocs = blocs_dgfip(&dsn);
    assert_eq!(blocs.len(), 1);
    assert_eq!(val(&blocs[0], "S21.G00.20.005"), Some("0.00"));
}

#[test]
fn sans_coordonnees_bancaires_la_dsn_refuse_et_dit_ou_les_saisir() {
    let mut ctx = contexte();
    let s = charger(&ctx, &demo("demo-3090-grande").join("paies/2026-07.json")).unwrap();
    ctx.entreprise["banque"] = json!({"bic": "", "iban": "FR7630001007941234567890185"});
    let refus = generer(&ctx, &s.paies, "2026-07", &[], &[], &Options::default()).unwrap_err();
    assert!(refus.contains("le BIC (S21.G00.20.003)") && !refus.contains("l'IBAN"), "{}", refus);
    assert!(refus.contains("Configuration › Paiement des cotisations"), "{}", refus);
    ctx.entreprise.as_object_mut().unwrap().remove("banque");
    let refus = generer(&ctx, &s.paies, "2026-07", &[], &[], &Options::default()).unwrap_err();
    assert!(refus.contains("le BIC (S21.G00.20.003) et l'IBAN (S21.G00.20.004)"), "{}", refus);
}

#[test]
fn chaque_etablissement_reverse_le_pas_de_ses_salaries() {
    let mut ctx = contexte();
    ctx.entreprise["etablissements"] = json!([{
        "nic": "00099", "naf": ctx.entreprise["naf"].clone(), "adresse": "1 rue de l'Essai",
        "code_postal": "69001", "commune": "LYON",
        "taux_atmp": {"valeur": 1.25, "code_risque": "923AD"}
    }]);
    let s = charger(&ctx, &demo("demo-3090-grande").join("paies/2026-07.json")).unwrap();
    // Le premier salarie qui retient du PAS part a l'etablissement secondaire.
    let seul = generer(&ctx, &s.paies, "2026-07", &[], &[], &Options::default()).unwrap();
    let cle = s
        .paies
        .iter()
        .map(|p| p.salarie["_cle"].as_str().unwrap().to_string())
        .find(|c| {
            let un: Vec<_> = s.paies.iter().filter(|p| p.salarie["_cle"].as_str() == Some(c)).cloned().collect();
            pas_du_nominatif(&generer(&ctx, &un, "2026-07", &[], &[], &Options::default()).unwrap()) > Decimal::ZERO
        })
        .expect("un salarie retient du PAS");
    let mut secondaire = Vec::new();
    let mut principal = Vec::new();
    for p in &s.paies {
        let mut p = p.clone();
        if p.salarie["_cle"].as_str() == Some(&cle) {
            p.salarie["etablissement"] = Value::String("00099".into());
            // Un contrat deja declare par le principal (un CDI) y passe au 01/07/2026 : la mutation se date sur la fiche,
            // sans quoi la mensuelle refuse d'ecrire le SIRET de l'ancien etablissement sans date (41.012, vague V3c).
            let mut historique = p.salarie["historique"].as_array().cloned().unwrap_or_default();
            historique.push(json!({"champ": "etablissement", "valeur": "", "jusqu_au": "2026-06-30", "note": ""}));
            p.salarie["historique"] = Value::Array(historique);
            secondaire.push(p);
        } else {
            principal.push(p);
        }
    }
    let d1 = generer(&ctx, &principal, "2026-07", &[], &[], &Options::default()).unwrap();
    let o2 = Options { etablissement: Some("00099".into()), ..Options::default() };
    let d2 = generer(&ctx, &secondaire, "2026-07", &[], &[], &o2).unwrap();
    for d in [&d1, &d2] {
        let r = pas_du_nominatif(d);
        let b = blocs_dgfip(d);
        if a_l_euro(r).is_zero() {
            assert!(b.is_empty());
        } else {
            assert_eq!(dec(val(&b[0], "S21.G00.20.005").unwrap()), a_l_euro(r), "le PAS de SES salaries");
        }
    }
    assert_eq!(pas_du_nominatif(&d1) + pas_du_nominatif(&d2), pas_du_nominatif(&seul));
}

// ------------------------------------------------- l'annule-et-remplace produit par le moteur, de bout en bout

/// Une copie de `demo-3090-grande` (juillet 2026 y est depose en reel), ou tous les salaries de juillet ont un taux
/// DGFiP nul ; `dgfip` : le depot de juillet portait un versement DGFiP.
fn compagnie_pas_nul(test: &str, dgfip: bool) -> PathBuf {
    let racine = compagnie::ses_dsn("demo-3090-grande", &["2026-07"], &outils::dossier_de_test("versement_dgfip", test));
    let ctx = Contexte::nouveau(&racine, Some(Jour::lire("2026-08-03").unwrap())).unwrap();
    let s = charger(&ctx, &racine.join("paies/2026-07.json")).unwrap();
    let chemin = racine.join("config/pas.json");
    let mut pas: Value = serde_json::from_slice(&std::fs::read(&chemin).unwrap()).unwrap();
    for p in &s.paies {
        pas["salaries"][p.salarie["_cle"].as_str().unwrap()] = json!({
            "taux": "0.00", "type": "01", "identifiant": "891000000", "mis_a_disposition": "20260601120000",
            "periode": "202606", "source": "essai", "releve_le": "2026-06-01"
        });
    }
    std::fs::write(&chemin, serde_json::to_vec_pretty(&pas).unwrap()).unwrap();
    if dgfip {
        let f = racine.join("sorties/2026-07/DSN_Juillet2026.dsn");
        let t = String::from_utf8_lossy(&std::fs::read(&f).unwrap()).into_owned();
        let i = t.find("S21.G00.20.001,").unwrap();
        let bloc = "S21.G00.20.001,'DGFIP'\r\nS21.G00.20.002,'DGFIP_PAS'\r\nS21.G00.20.003,'BANQFRPPXXX'\r\n\
S21.G00.20.004,'FR7630001007941234567890185'\r\nS21.G00.20.005,'738.00'\r\nS21.G00.20.006,'01072026'\r\n\
S21.G00.20.007,'31072026'\r\nS21.G00.20.010,'05'\r\n";
        std::fs::write(&f, format!("{}{}{}", &t[..i], bloc, &t[i..])).unwrap();
    }
    racine
}

fn produire_juillet(racine: &Path) -> Value {
    outils::route(racine, "produire", json!({"mois": "2026-07", "saisie": outils::lire(racine, "paies/2026-07.json")}), "2026-08-03")
}

/// La DSN d'annule-et-remplace que la production a ecrite.
fn l_annule_et_remplace(racine: &Path) -> String {
    let f = std::fs::read_dir(racine.join("sorties/2026-07"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .find(|p| p.file_name().unwrap().to_string_lossy().starts_with("DSN_Juillet2026_AR"))
        .expect("un annule-et-remplace ecrit");
    String::from_utf8_lossy(&std::fs::read(f).unwrap()).into_owned()
}

#[test]
fn l_annule_et_remplace_d_un_mois_qui_versait_du_pas_porte_un_versement_dgfip_a_zero() {
    let r = compagnie_pas_nul("versait", true);
    outils::reussi(&produire_juillet(&r));
    let dsn = l_annule_et_remplace(&r);
    assert!(dsn.contains("S20.G00.05.002,'03'"));
    assert_eq!(pas_du_nominatif(&dsn), Decimal::ZERO);
    let blocs = blocs_dgfip(&dsn);
    assert_eq!(blocs.len(), 1, "la declaration remplacee versait du PAS : un bloc a zero (p. 157)");
    assert_eq!(val(&blocs[0], "S21.G00.20.005"), Some("0.00"));
}

#[test]
fn l_annule_et_remplace_d_un_mois_qui_ne_versait_rien_n_ecrit_pas_de_versement_dgfip() {
    let r = compagnie_pas_nul("ne-versait-pas", false);
    outils::reussi(&produire_juillet(&r));
    let dsn = l_annule_et_remplace(&r);
    assert!(blocs_dgfip(&dsn).is_empty());
}
