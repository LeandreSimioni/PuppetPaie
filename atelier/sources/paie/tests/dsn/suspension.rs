//! La DSN d'un mois entierement suspendu sans remuneration (15/10/2026) : elle se produit et passe les deux controles
//! (usages, formats : `dsn::generer` les joue avant de rendre le texte). Textes :
//! `atelier/reference/sources/suspension-non-remuneree/` (`RAPPORT-20261015.md`, point 4) et le cahier technique
//! (`atelier/reference/norme-dsn/texte/bloc-S21.G00.50.txt`, `bloc-S21.G00.51.txt`, `bloc-S21.G00.65.txt`). Sur des copies
//! de `fictive-3090-grande` et `fictive-1285-grande` avec leurs DSN deposees (`compagnie::ses_dsn`).

use crate::outils::{self, compagnie, dec, ecrire, lire, valeur, versement};
use droit::contexte::Contexte;
use socle::date::Jour;
use paie::dsn::{generer, Options};
use socle::num::Decimal;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn copie(fictive: &str, test: &str) -> PathBuf {
    compagnie::ses_dsn(fictive, &[], &outils::dossier_de_test("dsn_suspension", test))
}

fn dsn_de_septembre(racine: &Path) -> Result<String, String> {
    let ctx = Contexte::nouveau(racine, Some(Jour::lire("2026-10-15").unwrap())).unwrap();
    let paies = engagement::charger(&ctx, "2026-09")?.paies;
    let o = Options { date_constitution: Some(Jour::lire("2026-10-15").unwrap()), envoi_test: true, ..Options::default() };
    generer(&ctx, &paies, "2026-09", &[], &[], &o)
}

/// Les (type, montant) des blocs 51 d'un versement.
fn remunerations(v: &[(String, String)]) -> Vec<(String, Decimal)> {
    let mut out = Vec::new();
    let mut type_courant = None;
    for (k, x) in v {
        match k.as_str() {
            "S21.G00.51.011" => type_courant = Some(x.clone()),
            "S21.G00.51.013" => out.push((type_courant.take().unwrap(), dec(x))),
            _ => {}
        }
    }
    out
}

/// Les (type, mesure) des blocs 53 d'un versement.
fn activites(v: &[(String, String)]) -> Vec<(String, Decimal)> {
    let mut out = Vec::new();
    let mut type_courant = None;
    for (k, x) in v {
        match k.as_str() {
            "S21.G00.53.001" => type_courant = Some(x.clone()),
            "S21.G00.53.002" => out.push((type_courant.take().unwrap(), dec(x))),
            _ => {}
        }
    }
    out
}

fn suspension(cle: &str, contrat_debut: &str, numero: &str, motif: &str) -> Value {
    json!({"imprevus": [{
        "genre": "suspension", "salarie": cle, "contrat_debut": contrat_debut, "contrat_numero": numero, "motif": motif,
        "debut": "2026-09-01", "fin": "2026-12-31"}]})
}

/// LA DSN D'UN MOIS SANS REMUNERATION EST ATTENDUE, ET ELLE PASSE. Pour Nicolet en conge parental tout septembre :
/// - le bloc 65 (motif 632, ses deux dates) : « dans toutes les DSN mensuelles dont le Mois Principal declare presente une
///   periode commune avec la suspension » (`bloc-S21.G00.65.txt`) ;
/// - les quatre remunerations requises (CCH-11 de S21.G00.51) : 001 et 002 a 0.00 ; 003 « Salaire retabli - reconstitue »,
///   le salaire reconstitue sans l'absence (2 310.00), 010 « la remuneration brute habituelle du salarie » (2 310.00)
///   (`bloc-S21.G00.51.txt`, Type) ; une base assujettie 03 nulle (CCH-16 et SIG-11 ne l'exigent que pour un 001 non nul) ;
/// - l'activite : type 01 a « 0.00 » « pour un individu entierement absent sur le mois » (fiche 2640), type 02 aux heures
///   reelles, 154.00 (fiche 2693, exemple 10) ;
/// - 50.002 et 50.013 a 0.00, et aucune affiliation sante ce mois-ci (3090, art. 4.2 ; inference pour la DSN).
#[test]
fn la_dsn_d_un_mois_entierement_suspendu_passe_les_controles() {
    let racine = copie("fictive-3090-grande", "mois-suspendu");
    ecrire(&racine, "imprevus/2026-09.json", &suspension("NICOLET", "2024-03-01", "NI.24.01", "632"));
    let t = dsn_de_septembre(&racine).unwrap_or_else(|e| panic!("{}", e));
    let c = outils::contrat(&t, "NI.24.01");
    assert_eq!(valeur(&c, "S21.G00.65.001").as_deref(), Some("632"));
    assert_eq!(valeur(&c, "S21.G00.65.002").as_deref(), Some("01092026"));
    let v = versement(&t, "NI.24.01");
    assert_eq!(valeur(&v, "S21.G00.50.002").as_deref(), Some("0.00"));
    assert_eq!(valeur(&v, "S21.G00.50.013").as_deref(), Some("0.00"));
    let r = remunerations(&v);
    let de = |ty: &str| r.iter().find(|(t, _)| t == ty).map(|(_, m)| *m);
    assert_eq!(de("001"), Some(dec("0.00")), "{:?}", r);
    assert_eq!(de("002"), Some(dec("0.00")), "{:?}", r);
    assert_eq!(de("003"), Some(dec("2310.00")), "{:?}", r);
    assert_eq!(de("010"), Some(dec("2310.00")), "{:?}", r);
    assert_eq!(activites(&v), vec![("01".to_string(), dec("0.00")), ("02".to_string(), dec("154.00"))]);
    // CCH-16 et SIG-11 n'exigent une base 03 que pour un 001 non nul ; ils ne l'interdisent pas : la base du mois, s'il y en a, est nulle.
    assert!(outils::base78(&v, "03").iter().all(|b| b.is_zero()), "{:?}", outils::base78(&v, "03"));
    // L'affiliation sante (place 1 : 70.012 = 203, la base 31 qui s'y affecte) ne part pas ce mois-ci ; la prevoyance
    // (place 0 : 202) reste.
    let affiliations: Vec<String> = c.iter().filter(|(k, _)| k == "S21.G00.70.012").map(|(_, x)| x.clone()).collect();
    assert!(affiliations.contains(&"202".to_string()) && !affiliations.contains(&"203".to_string()), "{:?}", affiliations);
    assert!(!v.iter().any(|(k, x)| k == "S21.G00.78.005" && x == "203"), "aucune base affectee a la sante");
}

/// UNE REMUNERATION NETTE FISCALE NEGATIVE : la fiche DSN 1810 (`net-entreprises-fiche1810-rnf-negative-pas-20250322.txt`)
/// la declare telle quelle en 50.002, et « La rubrique « Montant soumis au PAS - S21.G00.50.013 » ne peut pas etre
/// renseignee avec une valeur negative » : 0.00, et le PAS (50.009) 0.00. Le net social negatif (58.004) est licite (BOSS,
/// FAQ Q37). Le cas : Nadar (1285, taux DGFiP) en conge sabbatique tout septembre, et une compagnie dont le contrat de
/// sante maintient les garanties et les cotisations (`sante_suspension_non_indemnisee` = « maintenue ») -- la part
/// patronale se reintegre (CGI 83, 1° quater), la part salariale et la CSG se deduisent.
#[test]
fn un_net_fiscal_negatif_declare_un_montant_soumis_au_pas_nul() {
    let racine = copie("fictive-1285-grande", "rnf-negative");
    ecrire(&racine, "imprevus/2026-09.json", &suspension("NADAR", "2022-01-03", "NA.22.01", "639"));
    let mut ent = lire(&racine, "config/entreprise.json");
    ent["sante_suspension_non_indemnisee"] = json!("maintenue");
    ecrire(&racine, "config/entreprise.json", &ent);
    let t = dsn_de_septembre(&racine).unwrap_or_else(|e| panic!("{}", e));
    let v = versement(&t, "NA.22.01");
    let rnf = dec(&valeur(&v, "S21.G00.50.002").unwrap());
    assert!(rnf < Decimal::ZERO, "50.002 = {}", rnf);
    assert_eq!(valeur(&v, "S21.G00.50.013").as_deref(), Some("0.00"), "50.007 = {:?}", valeur(&v, "S21.G00.50.007"));
    assert_eq!(valeur(&v, "S21.G00.50.009").as_deref(), Some("0.00"));
    let net_social = v.iter().skip_while(|(k, x)| !(k == "S21.G00.58.003" && x == "03")).find(|(k, _)| k == "S21.G00.58.004").map(|(_, x)| dec(x));
    assert!(net_social.is_some_and(|m| m < Decimal::ZERO), "58.004 = {:?}", net_social);
}
