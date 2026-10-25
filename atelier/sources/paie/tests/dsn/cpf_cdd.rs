//! La contribution CPF-CDD (code du travail L. 6331-6) d'un salarie en CDD de droit commun, et la contribution a la
//! formation professionnelle (L. 6331-1) qui partage sa ligne du bulletin.
//!
//! Textes deposes : L. 6331-6 (`atelier/reference/sources/code-du-travail/articles-20261010.md`, l. 545-549) : « une
//! contribution dediee au financement du compte personnel de formation egale a 1 % du revenu d'activite retenu pour le
//! calcul des cotisations sociales des titulaires d'un contrat a duree determinee », sauf les CDD de formation ou
//! d'alternance et « ceux visant les salaries occupant un emploi a caractere saisonnier » ; L. 6331-55 : l'intermittent
//! du spectacle en est exclu (« par derogation [...] a l'article L. 6331-6 »). Norme DSN 2026.1 : « 129 - Contribution
//! dediee au financement du Compte Personnel de Formation pour les titulaires de CDD (CPF-CDD) »
//! (`atelier/reference/norme-dsn/texte/bloc-S21.G00.81.txt`, l. 285). Tableau DIDA de l'Urssaf : CTP 987 -> 129, base 03,
//! qualifiant 920 (`atelier/reference/sources/chomage-permanents/urssaf-equivalence-dida-20260611.csv`, l. 1001) ; table
//! des CTP : 987 « CONTRIBUTION CPF CDD », 1,0 deplafonne depuis 2022 (`logiciel/donnees/urssaf-codes-types-personnel.csv`).
//!
//! La majoration d'un CDD declare a la caisse des Conges Spectacles : CSS L. 243-1-3, 1° (nomme L. 6131-2 et L. 6331-6)
//! et D. 243-0-2, 1° (11,5 %) ; sa declaration, l'assiette majoree au 81.003 et au CTP (« le montant d'assiette doit
//! integrer la majoration de 11,5% », « les salaires a declarer sont ceux [...] effectivement payes, majores d'un
//! coefficient forfaitaire de 1,1150 ») : `atelier/reference/sources/majoration-caisses-conges/` (LISEZ-MOI, « La CPF-CDD
//! d'un CDD declare a la caisse » ; guide Urssaf des contributions de formation du 12/03/2026) et
//! `atelier/reference/sources/net-entreprises-base-connaissances/` (fiches 2502 et 2503).
//!
//! Sur la copie figee des compagnies fictives (`tests/donnees/fictives/`, compagnies inventees) : fictive-3090-grande, mars 2026,
//! AUDINOT en CDD (motif 02, accroissement temporaire d'activite, technicienne : non declaree a la caisse) et MONTANSIER
//! en CDI ; en octobre 2026, Deburau, comedien, en CDD de droit commun de trois mois (declare a la caisse).

use paie::calcul::Paie;
use droit::contexte::Contexte;
use socle::date::Jour;
use paie::dsn::{generer, Options};
use paie::engagement::charger;
use socle::num::{pct, Decimal};
use rust_decimal::RoundingStrategy;
use crate::outils::{base81, chemin_unique, ctp, dec, fictive, ecrire, lire, versement, Copie};
use serde_json::{json, Value};

/// Les rubriques 81.002 a 81.007 de la cotisation `code81` du versement, s'il la porte.
fn cotisation(v: &[(String, String)], code81: &str) -> Option<Vec<(String, String)>> {
    let i = v.iter().position(|(k, x)| k == "S21.G00.81.001" && x == code81)?;
    Some(v[i + 1..].iter().take_while(|(k, _)| k.starts_with("S21.G00.81.") && k != "S21.G00.81.001").cloned().collect())
}

fn paie<'a>(ps: &'a [Paie], numero: &str) -> &'a Paie {
    ps.iter().find(|p| p.engagement.numero.as_deref() == Some(numero)).unwrap()
}

#[test]
fn un_cdd_de_droit_commun_paie_la_contribution_cpf_cdd_au_bulletin_au_nominatif_et_au_bordereau() {
    let racine = fictive("fictive-3090-grande");
    let ctx = Contexte::nouveau(&racine, Some(Jour::lire("2026-03-28").unwrap())).unwrap();
    let s = charger(&ctx, &racine.join("paies/2026-03.json")).unwrap();
    let dsn = generer(&ctx, &s.paies, "2026-03", &[], &[], &Options::default()).unwrap();
    let cdd = paie(&s.paies, "AU.25.01");
    let brut = cdd.brut;
    assert!(brut > Decimal::ZERO);
    // Au bulletin : la ligne de la formation (« Autres contributions dues par l'employeur ») porte le 1 % en plus de la
    // formation professionnelle, chacun arrondi a part, sur le brut : le contrat n'est pas declare a la caisse.
    let l = cdd.lignes.iter().find(|l| l.id == "autres_4").expect("la ligne de la formation");
    assert_eq!(l.base, brut);
    assert_eq!(l.montant_pat, pct(brut, dec("0.55")) + pct(brut, dec("1.0")), "autres_4 du CDD : CFP + CPF-CDD");
    // Au nominatif : la cotisation 129 sur la base 03, au taux de 1 %.
    let c = cotisation(&versement(&dsn, "AU.25.01"), "129").expect("le CDD declare la cotisation 129");
    let v = |k: &str| c.iter().find(|(x, _)| x == k).map(|(_, y)| y.clone());
    assert_eq!(v("S21.G00.81.003").map(|x| dec(&x)), Some(brut));
    assert_eq!(v("S21.G00.81.004").map(|x| dec(&x)), Some(pct(brut, dec("1.0"))));
    // Le CDI ne la paie pas.
    let cdi = paie(&s.paies, "MO.23.01");
    assert!(cotisation(&versement(&dsn, "MO.23.01"), "129").is_none());
    let l = cdi.lignes.iter().find(|l| l.id == "autres_4").unwrap();
    assert_eq!(l.montant_pat, pct(cdi.brut, dec("0.55")));
    // Au bordereau : le CTP 987 sur le brut des CDD.
    let a_l_euro = |x: Decimal| x.round_dp_with_strategy(0, RoundingStrategy::MidpointNearestEven).round_dp(2);
    assert_eq!(ctp(&dsn, "987", "920"), Some(a_l_euro(brut)));
}

#[test]
fn un_cdd_saisonnier_ou_un_cddu_ne_la_paient_pas() {
    // L. 6331-6 : « ceux visant les salaries occupant un emploi a caractere saisonnier » n'y donnent pas lieu (motif 03) ;
    // L. 6331-55 : l'intermittent en CDDU non plus.
    let racine = fictive("fictive-3090-grande");
    let ctx = Contexte::nouveau(&racine, Some(Jour::lire("2026-03-28").unwrap())).unwrap();
    let mut saisie: Value = serde_json::from_str(&std::fs::read_to_string(racine.join("paies/2026-03.json")).unwrap()).unwrap();
    for e in saisie["engagements"].as_array_mut().unwrap() {
        if e["numero"] == "AU.25.01" {
            // Un avenant au 01/03/2026 : la DSN de mars declare l'ancien motif dans un bloc 41.
            e["motif_recours"] = Value::from("03");
            e["date_modification"] = Value::from("2026-03-01");
        }
    }
    let chemin = crate::outils::chemin_unique("cpf-cdd-saisonnier.json");
    std::fs::write(&chemin, saisie.to_string()).unwrap();
    let s = charger(&ctx, &chemin).unwrap();
    let dsn = generer(&ctx, &s.paies, "2026-03", &[], &[], &Options::default()).unwrap();
    assert!(cotisation(&versement(&dsn, "AU.25.01"), "129").is_none());
    assert_eq!(ctp(&dsn, "987", "920"), None);
    for p in s.paies.iter().filter(|p| !p.engagement.nat().unwrap().mensuelle()) {
        assert!(cotisation(&versement(&dsn, p.engagement.numero.as_deref().unwrap()), "129").is_none(), "un CDDU");
    }
}

/// Un CDD de droit commun de Deburau, comedien, du 01/09 au 30/11/2026, paye 2 400,00 en octobre : declare a la caisse
/// des Conges Spectacles (`caisse_conges::affilie`). Sa fiche recoit ce qu'un permanent demande (sante, anciennete dans
/// la branche) et le fichier des organismes le CTP 332 des permanents tel que le recompose `donnees/organismes-base.json`
/// (le 959 y reste sur `brut_permanent`, l'assiette d'un fichier compose avant : `dsn::ctp_urssaf` le lit comme le
/// fichier livre).
fn cdd_d_artiste_declare_a_la_caisse() -> (Copie, Contexte, Vec<Paie>) {
    let c = Copie::config_de("fictive-3090-grande");
    let r = &c.0;
    let (mut sal, mut orgs) = (lire(r, "config/salaries.json"), lire(r, "config/organismes.json"));
    sal["DEBURAU"]["sante_svp"] = json!({"niveau": "1", "conjoint": false, "enfants": 0});
    sal["DEBURAU"]["prevoyance_branche_depuis"] = json!("2024-01-08");
    for c in orgs["ctp_urssaf"].as_array_mut().unwrap() {
        if c["code"] == json!("332") && c["nature"] == json!("permanent") {
            c["assiette"] = json!("t1_x115_permanent");
        }
    }
    ecrire(r, "config/salaries.json", &sal);
    ecrire(r, "config/organismes.json", &orgs);
    let ctx = c.contexte("2026-10-28");
    let e = json!({"salarie": "Deburau", "nature": "cdd", "motif_recours": "02", "debut": "2026-09-01", "fin": "2026-11-30",
                   "numero": "DE5.26.09", "quotite_contrat": 151.67,
                   "lignes": [{"rubrique": "mois", "quantite": 1, "taux": 2400.0, "heures": 151.67}]});
    let chemin = chemin_unique("cpf-cdd-caisse.json");
    std::fs::write(&chemin, json!({"mois": "2026-10", "date_paiement": "2026-10-31", "engagements": [e]}).to_string()).unwrap();
    let s = charger(&ctx, &chemin).unwrap();
    let _ = std::fs::remove_file(&chemin);
    (c, ctx, s.paies)
}

#[test]
fn un_cdd_declare_a_la_caisse_majore_de_11_5_pour_cent_l_assiette_de_sa_formation_et_de_sa_cpf_cdd() {
    // CSS L. 243-1-3, 1° et D. 243-0-2, 1° : la CFP (par L. 6131-2) et la CPF-CDD (L. 6331-6) d'un employeur affilie se
    // paient sur l'assiette majoree de 11,5 % ; le guide Urssaf et la fiche 2503 : le 81.003 et le CTP la portent, le
    // 78.004 reste le brut. La CSA et le dialogue social (L. 243-1-3, 2° : la caisse les verse) restent sur le brut.
    let (_copie, ctx, paies) = cdd_d_artiste_declare_a_la_caisse();
    let p = &paies[0];
    assert_eq!(p.brut, dec("2400.00"));
    let majore = dec("2676.00");
    let l = p.lignes.iter().find(|l| l.id == "autres_4").expect("la ligne de la formation");
    assert_eq!(l.base, majore);
    assert_eq!(l.montant_pat, pct(majore, dec("0.55")) + pct(majore, dec("1.0")), "14,72 + 26,76");
    assert_eq!(l.montant_pat, dec("41.48"));
    let l = p.lignes.iter().find(|l| l.id == "autres_1").unwrap();
    assert_eq!((l.base, l.montant_pat), (dec("2400.00"), pct(dec("2400.00"), dec("0.30")) + pct(dec("2400.00"), dec("0.016"))));

    let dsn = generer(&ctx, &paies, "2026-10", &[], &[], &Options::default()).unwrap();
    let v = versement(&dsn, "DE5.26.09");
    for (code, taux, montant) in [("128", "0.55", "14.72"), ("129", "1.0", "26.76")] {
        let c = cotisation(&v, code).unwrap_or_else(|| panic!("la cotisation {}", code));
        let x = |k: &str| c.iter().find(|(y, _)| y == k).map(|(_, z)| dec(z));
        assert_eq!(x("S21.G00.81.003"), Some(majore), "{} : l'assiette majoree", code);
        assert_eq!(x("S21.G00.81.004"), Some(dec(montant)), "{}", code);
        assert_eq!(x("S21.G00.81.007"), Some(dec(taux)), "{}", code);
    }
    let i = v.iter().position(|(k, x)| k == "S21.G00.78.001" && x == "03").unwrap();
    let b78 = v[i..].iter().find(|(k, _)| k == "S21.G00.78.004").map(|(_, x)| dec(x));
    assert_eq!(b78, Some(dec("2400.00")), "le 78.004 de la base 03 reste le brut");
    assert_eq!(base81(&v, "049"), Some(majore), "le FNAL, deja majore");
    assert_eq!(ctp(&dsn, "987", "920"), Some(dec("2676")), "CPF-CDD : la somme des 81.003 de la 129");
    assert_eq!(ctp(&dsn, "959", "920"), Some(dec("2676")), "CFP des permanents : la somme des 81.003 de la 128");
}
