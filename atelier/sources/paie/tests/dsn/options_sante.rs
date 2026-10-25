//! L'affiliation prevoyance d'un salarie (bloc S21.G00.70) dit l'option de sante que
//! l'organisme demande, ses ayants droit, la date ou elle commence quand ce n'est pas celle du contrat -- et un salarie
//! qui n'est pas affilie n'est pas declare affilie.
//!
//! Textes :
//! - cahier technique DSN 2026.1, bloc S21.G00.70 (`atelier/reference/norme-dsn/texte/bloc-S21.G00.70.txt`, p. 285-286) :
//!   70.004 « Les valeurs de code sont fournies par l'organisme de prévoyance et sont à renseigner seulement sur sa
//!   demande » ; 70.008 et 70.011 « doit être renseignée si elle intervient dans le calcul de la cotisation » ; 70.009
//!   la somme des trois compteurs « si ces rubriques sont toutes les trois valorisées » ; 70.014 et 70.015
//!   (`bloc-S21.G00.71.txt`, p. 287) : « lorsque la date de début d'affiliation au contrat OC ne correspond pas à la
//!   date de début du contrat de travail » ; le bloc 15 (`bloc-S21.G00.15.txt`, CCH-13 et CCH-14 de 15.004) ;
//! - bloc S21.G00.79 (`bloc-S21.G00.79.txt`, p. 298-300) : « Pour la Prévoyance, ce bloc est à alimenter par les
//!   éléments constituant les paramètres qui entrent dans le calcul de la cotisation », « 18 - Base forfaitaire
//!   Prévoyance » ;
//! - fiche de parametrage Audiens version 139 (`atelier/reference/fiches-parametrage/audiens-2025-12-13-fiche139-expurgee.xml`),
//!   groupes 00003 a 00008 : `ReferenceContrat="FM SPECT VIVANT PRIV"`, `CodeOption` NIV1/I/GE ... NIV3F/IACE/GE,
//!   `BaseMontantSpecifique` `ValeurCodeNature="18"`, `Assiette` PMSS, par assure, conjoint et enfant.
//!
//! Sur des copies des compagnies fictives figees (compagnies inventees), dans le dossier temporaire des tests ; rien n'est
//! ecrit dans `tests/donnees/fictives/`.

use droit::contexte::Contexte;
use socle::date::Jour;
use paie::dsn::{bloc_contrat, generer, Fichier, Options};
use engagement::{charger, Saisie};
use crate::outils::{chemin_unique, contrat, dec, ecrire, lire, paires, valeur, versement, Copie};
use serde_json::{json, Value};

/// La config d'une compagnie fictive, copiee ; `changer` recoit ses salaries et la fiche de l'entreprise.
fn copie_de(fictive: &str, changer: impl FnOnce(&mut Value, &mut Value)) -> Copie {
    let c = Copie::config_de(fictive);
    let (mut sal, mut ent) = (lire(&c.0, "config/salaries.json"), lire(&c.0, "config/entreprise.json"));
    changer(&mut sal, &mut ent);
    ecrire(&c.0, "config/salaries.json", &sal);
    ecrire(&c.0, "config/entreprise.json", &ent);
    c
}

fn saisie(ctx: &Contexte, mois: &str, engagements: Value) -> Result<Saisie, String> {
    let chemin = chemin_unique("options-sante.json");
    let s = json!({"mois": mois, "date_paiement": format!("{}-28", mois), "engagements": engagements});
    std::fs::write(&chemin, s.to_string()).unwrap();
    let r = charger(ctx, &chemin);
    let _ = std::fs::remove_file(&chemin);
    r
}

/// Un CDI paye au mois (2 400,00), commence le `debut`.
fn cdi(salarie: &str, numero: &str, debut: &str) -> Value {
    json!({"salarie": salarie, "nature": "cdi", "debut": debut, "numero": numero, "quotite_contrat": 151.67,
           "lignes": [{"rubrique": "mois", "quantite": 1, "taux": 2400.0, "heures": 151.67}]})
}

/// Un CDDU d'un cachet de 100,00 le `le`.
fn cddu(salarie: &str, numero: &str, debut: &str, fin: &str, le: &str) -> Value {
    json!({"salarie": salarie, "spectacle": "266Z00910022", "debut": debut, "fin": fin, "numero": numero,
           "date_paiement": le,
           "lignes": [{"rubrique": "cachet", "quantite": 1, "taux": 100.0, "lieu": "Theatre fictif", "dates": [le]}]})
}

/// Les blocs 70 du contrat, un par affiliation : une affiliation commence quand l'ordre des rubriques repart.
fn affiliations(dsn: &str, numero: &str) -> Vec<Vec<(String, String)>> {
    let mut out: Vec<Vec<(String, String)>> = Vec::new();
    let mut derniere = String::new();
    for (k, v) in contrat(dsn, numero) {
        if !k.starts_with("S21.G00.70.") {
            continue;
        }
        if out.is_empty() || k <= derniere {
            out.push(Vec::new());
        }
        derniere = k.clone();
        out.last_mut().unwrap().push((k, v));
    }
    out
}

fn affiliation<'a>(a: &'a [Vec<(String, String)>], adhesion: &str) -> Option<&'a Vec<(String, String)>> {
    a.iter().find(|x| valeur(x, "S21.G00.70.013").as_deref() == Some(adhesion))
}

/// La base 31 du versement rattachee a l'affiliation `id` (78.005), du 78.001 au 78.001 suivant.
fn base_31(v: &[(String, String)], id: &str) -> Option<Vec<(String, String)>> {
    let mut i = 0;
    while i < v.len() {
        if v[i].0 == "S21.G00.78.001" {
            let fin = v[i + 1..].iter().position(|(k, _)| k == "S21.G00.78.001").map_or(v.len(), |p| i + 1 + p);
            let bloc = &v[i..fin];
            if v[i].1 == "31" && valeur(bloc, "S21.G00.78.005").as_deref() == Some(id) {
                return Some(bloc.to_vec());
            }
            i = fin;
        } else {
            i += 1;
        }
    }
    None
}

/// S21.G00.15.004 de l'adhesion `reference` (15.005).
fn personnel_couvert(dsn: &str, reference: &str) -> Option<String> {
    let mut couvert = None;
    for (k, v) in paires(dsn) {
        match k.as_str() {
            "S21.G00.15.004" => couvert = Some(v),
            "S21.G00.15.005" if v == reference => return couvert,
            _ => {}
        }
    }
    None
}

/// Chaque base 31 du versement cite une affiliation (70.012) que le contrat declare.
fn bases_31_rattachees(dsn: &str, numero: &str) {
    let ids: Vec<String> = affiliations(dsn, numero).iter().filter_map(|a| valeur(a, "S21.G00.70.012")).collect();
    let v = versement(dsn, numero);
    for (i, (k, x)) in v.iter().enumerate() {
        if k == "S21.G00.78.001" && x == "31" {
            let id = valeur(&v[i..], "S21.G00.78.005").expect("78.005");
            assert!(ids.contains(&id), "la base 31 cite l'affiliation {} que le contrat {} ne declare pas ({:?})", id, numero, ids);
        }
    }
}

const RUBRIQUES_DE_L_OPTION: [&str; 6] =
    ["S21.G00.70.004", "S21.G00.70.008", "S21.G00.70.009", "S21.G00.70.010", "S21.G00.70.011", "S21.G00.70.014"];

#[test]
fn un_permanent_au_niveau_2_famille_declare_son_option_son_conjoint_et_ses_enfants() {
    // Deburau en CDI depuis le 01/09/2026, sante SVP au niveau 2 avec son conjoint et deux enfants : groupe 00006 de la
    // fiche 139, NIV2F/IACE/GE, cotisation par ASSURE PSS, CONJOINT PSS et ENFANT PSS -- les deux compteurs entrent dans
    // le calcul, 70.008 = 1 et 70.011 = 2. 70.010 n'est pas ecrit (aucun ascendant dans la fiche), donc 70.009 non plus.
    let copie = copie_de("fictive-3090-grande", |sal, _| {
        sal["DEBURAU"]["sante_svp"] = json!({"niveau": "2", "conjoint": true, "enfants": 2});
        sal["DEBURAU"]["prevoyance_branche_depuis"] = json!("2024-01-08");
    });
    let ctx = copie.contexte("2026-10-28");
    let s = saisie(&ctx, "2026-10", json!([cdi("Deburau", "DE5.26.09", "2026-09-01")])).unwrap();
    let dsn = generer(&ctx, &s.paies, "2026-10", &[], &[], &Options::default()).unwrap();
    let a = affiliations(&dsn, "DE5.26.09");
    let sante = affiliation(&a, "101").expect("l'affiliation a la sante des permanents (FM SPECT VIVANT PRIV)");
    assert_eq!(valeur(sante, "S21.G00.70.004").as_deref(), Some("NIV2F/IACE/GE"));
    assert_eq!(valeur(sante, "S21.G00.70.008").as_deref(), Some("1"));
    assert_eq!(valeur(sante, "S21.G00.70.011").as_deref(), Some("2"));
    assert_eq!(valeur(sante, "S21.G00.70.009"), None, "70.009 : la fiche ne calcule rien sur le total des ayants droit");
    assert_eq!(valeur(sante, "S21.G00.70.010"), None);
    assert_eq!(valeur(sante, "S21.G00.70.014"), None, "affilie des le debut du contrat");
    let ordre: Vec<&str> = sante.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(ordre, ["S21.G00.70.004", "S21.G00.70.008", "S21.G00.70.011", "S21.G00.70.012", "S21.G00.70.013"]);
    // La prevoyance et la CASC-SVP : aucune option demandee.
    for adhesion in ["106", "100"] {
        let x = affiliation(&a, adhesion).unwrap_or_else(|| panic!("l'affiliation {}", adhesion));
        assert!(RUBRIQUES_DE_L_OPTION.iter().all(|r| valeur(x, r).is_none()), "{} : {:?}", adhesion, x);
    }
    bases_31_rattachees(&dsn, "DE5.26.09");
}

#[test]
fn la_base_31_de_la_sante_d_un_permanent_3090_declare_la_base_forfaitaire_du_plafond() {
    // Fiche 139 : chaque option de FM SPECT VIVANT PRIV attend `BaseMontantSpecifique ValeurCodeNature="18"` sur le
    // PMSS. Le composant de la base 31 de la sante est donc la base forfaitaire (18), le plafond du mois sur lequel la
    // mutuelle est calculee -- pas la tranche A (11) sur le brut, que garde la prevoyance (groupes TAPrev).
    let copie = copie_de("fictive-3090-grande", |sal, _| {
        sal["DEBURAU"]["sante_svp"] = json!({"niveau": "2", "conjoint": true, "enfants": 2});
        sal["DEBURAU"]["prevoyance_branche_depuis"] = json!("2024-01-08");
    });
    let ctx = copie.contexte("2026-10-28");
    let s = saisie(&ctx, "2026-10", json!([cdi("Deburau", "DE5.26.09", "2026-09-01")])).unwrap();
    let dsn = generer(&ctx, &s.paies, "2026-10", &[], &[], &Options::default()).unwrap();
    let a = affiliations(&dsn, "DE5.26.09");
    let v = versement(&dsn, "DE5.26.09");
    let mutuelle = s.paies[0].lignes.iter().find(|l| l.id == "mutuelle").expect("la ligne de la mutuelle");
    let b = base_31(&v, &valeur(affiliation(&a, "101").unwrap(), "S21.G00.70.012").unwrap()).expect("la base 31 de la sante");
    assert_eq!(valeur(&b, "S21.G00.79.001").as_deref(), Some("18"));
    assert_eq!(valeur(&b, "S21.G00.79.004").map(|x| dec(&x)), Some(mutuelle.base));
    assert_ne!(mutuelle.base, s.paies[0].brut);
    assert_eq!(b.iter().filter(|(k, _)| k == "S21.G00.79.001").count(), 1, "une seule base 18, meme pour une famille");
    let bp = base_31(&v, &valeur(affiliation(&a, "106").unwrap(), "S21.G00.70.012").unwrap()).expect("la base 31 de la prevoyance");
    assert_eq!(valeur(&bp, "S21.G00.79.001").as_deref(), Some("11"));
    let bc = base_31(&v, &valeur(affiliation(&a, "100").unwrap(), "S21.G00.70.012").unwrap()).expect("la base 31 de la CASC-SVP");
    assert_eq!(valeur(&bc, "S21.G00.79.001").as_deref(), Some("11"));
}

#[test]
fn un_permanent_isole_au_niveau_1_declare_son_option_sans_ayants_droit() {
    let copie = copie_de("fictive-3090-grande", |sal, _| {
        sal["DEBURAU"]["sante_svp"] = json!({"niveau": "1", "conjoint": false, "enfants": 0});
        sal["DEBURAU"]["prevoyance_branche_depuis"] = json!("2024-01-08");
    });
    let ctx = copie.contexte("2026-10-28");
    let s = saisie(&ctx, "2026-10", json!([cdi("Deburau", "DE5.26.09", "2026-09-01")])).unwrap();
    let dsn = generer(&ctx, &s.paies, "2026-10", &[], &[], &Options::default()).unwrap();
    let a = affiliations(&dsn, "DE5.26.09");
    let sante = affiliation(&a, "101").expect("l'affiliation a la sante des permanents");
    assert_eq!(valeur(sante, "S21.G00.70.004").as_deref(), Some("NIV1/I/GE"));
    for r in ["S21.G00.70.008", "S21.G00.70.009", "S21.G00.70.010", "S21.G00.70.011", "S21.G00.70.014"] {
        assert_eq!(valeur(sante, r), None, "{} : l'option isole ne calcule rien sur des ayants droit", r);
    }
}

#[test]
fn un_intermittent_ne_declare_ni_option_ni_ayant_droit() {
    // Thalie Sante (groupe 00011), l'interbranche (00001/00002) et la CASC-SVP : la fiche 139 n'y donne ni option ni
    // population, et le CDDU commence apres le 01/01/2026 : rien d'autre que 70.012 et 70.013.
    let copie = copie_de("fictive-3090-grande", |_, _| {});
    let ctx = copie.contexte("2026-10-28");
    let s = saisie(&ctx, "2026-10", json!([cddu("Clairon", "CL3.26.10", "2026-10-20", "2026-10-20", "2026-10-20")])).unwrap();
    let dsn = generer(&ctx, &s.paies, "2026-10", &[], &[], &Options::default()).unwrap();
    let a = affiliations(&dsn, "CL3.26.10");
    assert!(affiliation(&a, "103").is_some() && affiliation(&a, "102").is_some());
    for x in &a {
        let ordre: Vec<&str> = x.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(ordre, ["S21.G00.70.012", "S21.G00.70.013"], "{:?}", x);
    }
}

#[test]
fn un_permanent_affilie_a_la_sante_svp_sans_niveau_refuse() {
    // Le calcul refuse deja (`bareme_sante`) ; la DSN refuse aussi, d'elle-meme, de declarer l'affiliation sans son
    // option : elle nomme la fiche du salarie et la fiche de parametrage Audiens.
    let copie = copie_de("fictive-3090-grande", |sal, _| {
        sal["DEBURAU"]["sante_svp"] = json!({"niveau": "2", "conjoint": false, "enfants": 0});
        sal["DEBURAU"]["prevoyance_branche_depuis"] = json!("2024-01-08");
        sal["CLAIRON"]["prevoyance_branche_depuis"] = json!("2024-01-08");
    });
    let ctx = copie.contexte("2026-10-28");
    let e = match saisie(&ctx, "2026-10", json!([cdi("Clairon", "CL5.26.09", "2026-09-01")])) {
        Ok(_) => panic!("un permanent sans `sante_svp` se paie"),
        Err(e) => e,
    };
    assert!(e.contains("Complémentaire santé"), "le refus dit la case de la fiche : {}", e);

    let s = saisie(&ctx, "2026-10", json!([cdi("Deburau", "DE5.26.09", "2026-09-01")])).unwrap();
    let mut p = s.paies[0].clone();
    p.salarie.as_object_mut().unwrap().remove("sante_svp");
    let orgs = lire(&copie.0, "config/organismes.json");
    let bar = ctx.bareme(&Jour::lire("2026-10-31").unwrap()).unwrap();
    let mut f = Fichier::en_norme(paie::norme::de_la_mensuelle("2026-10").unwrap());
    let e = bloc_contrat(&mut f, &p, &ctx.entreprise, &orgs, 1, &bar, false, &[], "01", "01").unwrap_err();
    assert!(e.contains("sante_svp") && e.contains("S21.G00.70.004") && e.contains("fiche de parametrage Audiens"), "{}", e);
    // Un niveau que la fiche 139 ne connait pas refuse de meme.
    p.salarie["sante_svp"] = json!({"niveau": "4", "conjoint": false, "enfants": 0});
    let e = bloc_contrat(&mut Fichier::en_norme(paie::norme::de_la_mensuelle("2026-10").unwrap()), &p, &ctx.entreprise, &orgs, 1, &bar, false, &[], "01", "01").unwrap_err();
    assert!(e.contains("S21.G00.70.004"), "{}", e);
}

#[test]
fn un_permanent_du_regime_local_refuse_faute_d_option_a_lui() {
    // Les six options de la fiche 139 sont du « Régime Général » (GE) : celle d'un salarie d'Alsace-Moselle n'y est pas.
    let copie = copie_de("fictive-3090-grande", |sal, ent| {
        sal["DEBURAU"]["sante_svp"] = json!({"niveau": "1", "conjoint": false, "enfants": 0});
        sal["DEBURAU"]["prevoyance_branche_depuis"] = json!("2024-01-08");
        ent["code_postal"] = json!("67000");
    });
    let ctx = copie.contexte("2026-10-28");
    let s = saisie(&ctx, "2026-10", json!([cdi("Deburau", "DE5.26.09", "2026-09-01")])).unwrap();
    let e = generer(&ctx, &s.paies, "2026-10", &[], &[], &Options::default()).unwrap_err();
    assert!(e.contains("S21.G00.70.004") && e.contains("regime local"), "{}", e);
}

#[test]
fn thalie_sante_dit_sa_date_d_affiliation_a_un_cddu_commence_en_2025() {
    // TSINTERMITTENTS existe depuis le 01/01/2026 (fiche 139, groupe 00011 ; `organismes-base.json`, `depuis`) : un
    // CDDU commence le 20/12/2025 n'y est affilie que depuis le 01/01/2026 -- 70.014 ; l'interbranche, des le contrat.
    let copie = copie_de("fictive-3090-grande", |_, _| {});
    let ctx = copie.contexte("2026-01-28");
    let s = saisie(&ctx, "2026-01", json!([cddu("Clairon", "CL9.25.12", "2025-12-20", "2026-01-10", "2026-01-08")])).unwrap();
    let dsn = generer(&ctx, &s.paies, "2026-01", &[], &[], &Options::default()).unwrap();
    let a = affiliations(&dsn, "CL9.25.12");
    let ts = affiliation(&a, "103").expect("Thalie Sante");
    assert_eq!(valeur(ts, "S21.G00.70.014").as_deref(), Some("01012026"));
    assert_eq!(valeur(ts, "S21.G00.70.015"), None);
    assert_eq!(valeur(affiliation(&a, "102").expect("l'interbranche"), "S21.G00.70.014"), None);
    let ordre: Vec<&str> = ts.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(ordre, ["S21.G00.70.012", "S21.G00.70.013", "S21.G00.70.014"]);
}

#[test]
fn un_non_cadre_de_moins_d_un_an_dans_la_branche_n_est_pas_affilie_a_la_prevoyance() {
    // CCN 3090, art. 12 (`prevoyance-conventionnelle-base.json`, `ouverture_non_cadre_mois` 12) : arrive dans la
    // branche le 15/01/2026, Deburau n'a droit a la prevoyance qu'au 15/01/2027. En octobre 2026, la DSN ne le rattache
    // pas a PREVOYANCE SVP (ni bloc 70, ni base 31 a zero) et l'adhesion n'a plus personne de couvert (15.004 = 02).
    let copie = copie_de("fictive-3090-grande", |sal, _| {
        sal["DEBURAU"]["sante_svp"] = json!({"niveau": "1", "conjoint": false, "enfants": 0});
        sal["DEBURAU"]["prevoyance_branche_depuis"] = json!("2026-01-15");
    });
    let ctx = copie.contexte("2026-10-28");
    let s = saisie(&ctx, "2026-10", json!([cdi("Deburau", "DE5.26.09", "2026-09-01")])).unwrap();
    let dsn = generer(&ctx, &s.paies, "2026-10", &[], &[], &Options::default()).unwrap();
    let a = affiliations(&dsn, "DE5.26.09");
    assert!(affiliation(&a, "106").is_none(), "pas encore affilie a la prevoyance : {:?}", a);
    assert!(affiliation(&a, "101").is_some() && affiliation(&a, "100").is_some());
    bases_31_rattachees(&dsn, "DE5.26.09");
    assert_eq!(personnel_couvert(&dsn, "106").as_deref(), Some("02"));
    assert_eq!(personnel_couvert(&dsn, "101").as_deref(), Some("01"));
}

#[test]
fn la_prevoyance_d_un_non_cadre_dit_le_jour_ou_elle_s_ouvre() {
    // Arrive dans la branche le 15/10/2025, en CDI depuis le 01/09/2026 : la prevoyance s'ouvre le 15/10/2026, apres le
    // debut du contrat -- 70.014 = 15102026 sous PREVOYANCE SVP, et rien sous la sante, ouverte avec le contrat.
    let copie = copie_de("fictive-3090-grande", |sal, _| {
        sal["DEBURAU"]["sante_svp"] = json!({"niveau": "1", "conjoint": false, "enfants": 0});
        sal["DEBURAU"]["prevoyance_branche_depuis"] = json!("2025-10-15");
    });
    let ctx = copie.contexte("2026-10-28");
    let s = saisie(&ctx, "2026-10", json!([cdi("Deburau", "DE5.26.09", "2026-09-01")])).unwrap();
    let dsn = generer(&ctx, &s.paies, "2026-10", &[], &[], &Options::default()).unwrap();
    let a = affiliations(&dsn, "DE5.26.09");
    assert_eq!(valeur(affiliation(&a, "106").expect("affilie a la prevoyance"), "S21.G00.70.014").as_deref(), Some("15102026"));
    assert_eq!(valeur(affiliation(&a, "101").unwrap(), "S21.G00.70.014"), None);
    bases_31_rattachees(&dsn, "DE5.26.09");

    // Entre dans la branche avec son contrat il y a plus d'un an : affilie depuis le 01/09/2025, 70.014 aussi.
    let copie = copie_de("fictive-3090-grande", |sal, _| {
        sal["DEBURAU"]["sante_svp"] = json!({"niveau": "1", "conjoint": false, "enfants": 0});
        sal["DEBURAU"]["prevoyance_branche_depuis"] = json!("2025-09-01");
    });
    let ctx = copie.contexte("2026-10-28");
    let s = saisie(&ctx, "2026-10", json!([cdi("Deburau", "DE5.25.09", "2025-09-01")])).unwrap();
    let dsn = generer(&ctx, &s.paies, "2026-10", &[], &[], &Options::default()).unwrap();
    let a = affiliations(&dsn, "DE5.25.09");
    assert_eq!(valeur(affiliation(&a, "106").unwrap(), "S21.G00.70.014").as_deref(), Some("01092026"));
}

#[test]
fn une_prevoyance_ouverte_sans_date_d_entree_dans_la_branche_refuse_quand_elle_a_pu_s_ouvrir_pendant_le_contrat() {
    // Pas de `prevoyance_branche_depuis` : le calcul se contente de l'anciennete dans l'entreprise (plus d'un an, donc
    // au moins autant dans la branche) ; mais la DSN ne sait pas si l'affiliation a commence avec le contrat (le
    // 01/09/2025) ou un an plus tard. Elle refuse et demande la date.
    let copie = copie_de("fictive-3090-grande", |sal, _| {
        sal["DEBURAU"]["sante_svp"] = json!({"niveau": "1", "conjoint": false, "enfants": 0});
        sal["DEBURAU"]["date_anciennete"] = json!("2025-09-01");
        sal["DEBURAU"].as_object_mut().unwrap().remove("prevoyance_branche_depuis");
    });
    let ctx = copie.contexte("2026-10-28");
    let s = saisie(&ctx, "2026-10", json!([cdi("Deburau", "DE5.25.09", "2025-09-01")])).unwrap();
    let e = generer(&ctx, &s.paies, "2026-10", &[], &[], &Options::default()).unwrap_err();
    assert!(e.contains("Dans la branche depuis") && e.contains("S21.G00.70.014"), "{}", e);

    // Dans l'entreprise depuis plus d'un an AVANT ce contrat : l'affiliation commence avec lui, rien a dater.
    let copie = copie_de("fictive-3090-grande", |sal, _| {
        sal["DEBURAU"]["sante_svp"] = json!({"niveau": "1", "conjoint": false, "enfants": 0});
        sal["DEBURAU"]["date_anciennete"] = json!("2024-01-08");
        sal["DEBURAU"].as_object_mut().unwrap().remove("prevoyance_branche_depuis");
    });
    let ctx = copie.contexte("2026-10-28");
    let s = saisie(&ctx, "2026-10", json!([cdi("Deburau", "DE5.26.09", "2026-09-01")])).unwrap();
    let dsn = generer(&ctx, &s.paies, "2026-10", &[], &[], &Options::default()).unwrap();
    assert_eq!(valeur(affiliation(&affiliations(&dsn, "DE5.26.09"), "106").unwrap(), "S21.G00.70.014"), None);
}

#[test]
fn un_permanent_dispense_de_sante_n_est_pas_affilie_a_la_sante() {
    // CCNEAC, XII.2.1.8 : la dispense, a la demande ecrite du salarie (`sante_dispense`) ; le bulletin n'a plus de
    // mutuelle. La DSN ne le rattache pas a SANTE CCNEAC (ni bloc 70, ni base 31 a zero) : 70.014/.015 decrivent la
    // dispense comme ce qui ouvre ou ferme une affiliation (p. 287). La prevoyance reste.
    let copie = copie_de("fictive-1285-grande", |sal, _| {
        sal["DUMESNIL"]["sante_dispense"] = json!("couverture collective obligatoire chez son conjoint");
    });
    let ctx = copie.contexte("2026-10-28");
    let s = saisie(&ctx, "2026-10", json!([cdi("Dumesnil", "DU5.26.09", "2026-09-01")])).unwrap();
    assert!(s.paies[0].lignes.iter().all(|l| l.id != "mutuelle"), "dispense : pas de mutuelle au bulletin");
    let dsn = generer(&ctx, &s.paies, "2026-10", &[], &[], &Options::default()).unwrap();
    let a = affiliations(&dsn, "DU5.26.09");
    assert!(affiliation(&a, "107").is_none(), "dispense : pas d'affiliation a la sante ({:?})", a);
    assert!(affiliation(&a, "106").is_some(), "la prevoyance reste");
    bases_31_rattachees(&dsn, "DU5.26.09");
    assert_eq!(personnel_couvert(&dsn, "107").as_deref(), Some("02"));
}

/// R12 : la 3090 a ses dispenses (accord du 19/07/2024, art. 9.1 et 9.2,
/// `atelier/reference/sources/kali-3090-accord-sante-permanents-20240719-20261011.md` : facultatives 9.2.1 a-e, de droit
/// 9.2.2 a-e). Un permanent 3090 dispense n'a pas de mutuelle au bulletin, et sa DSN ne le rattache pas a FM SPECT
/// VIVANT PRIV (101) : ni bloc 70, ni base 31 forfaitaire (18) de la sante. La prevoyance (106) et la CASC-SVP (100)
/// restent. Le motif n'est pas lu : la dispense suffit, son niveau `sante_svp` aussi devient sans objet.
#[test]
fn un_permanent_3090_dispense_de_sante_n_est_pas_affilie_a_la_sante() {
    let copie = copie_de("fictive-3090-grande", |sal, _| {
        sal["DEBURAU"]["prevoyance_branche_depuis"] = json!("2024-01-08");
        sal["DEBURAU"]["sante_dispense"] = json!("9.2.2 c : couvert au titre de l'emploi de son conjoint");
        sal["DEBURAU"].as_object_mut().unwrap().remove("sante_svp");
    });
    let ctx = copie.contexte("2026-10-28");
    let s = saisie(&ctx, "2026-10", json!([cdi("Deburau", "DE5.26.09", "2026-09-01")])).unwrap();
    assert!(s.paies[0].lignes.iter().all(|l| l.id != "mutuelle"), "dispense : pas de mutuelle au bulletin");
    let dsn = generer(&ctx, &s.paies, "2026-10", &[], &[], &Options::default()).unwrap();
    let a = affiliations(&dsn, "DE5.26.09");
    assert!(affiliation(&a, "101").is_none(), "dispense : pas d'affiliation a la sante ({:?})", a);
    assert!(affiliation(&a, "106").is_some() && affiliation(&a, "100").is_some(), "la prevoyance et la CASC-SVP restent");
    bases_31_rattachees(&dsn, "DE5.26.09");
    let v = versement(&dsn, "DE5.26.09");
    assert!(v.iter().all(|(k, x)| !(k == "S21.G00.79.001" && x == "18")), "pas de base forfaitaire de la sante : {:?}", v);
    assert_eq!(personnel_couvert(&dsn, "101").as_deref(), Some("02"));

    // Ses bornes valent pour la 3090 comme pour la CCNEAC (Q13) : une dispense jusqu'au 30/09 (9.2.2 b, l'echeance de
    // son assurance individuelle) le laisse affilie en octobre, l'affiliation datee du 01/10.
    let copie = copie_de("fictive-3090-grande", |sal, _| {
        sal["DEBURAU"]["sante_svp"] = json!({"niveau": "1", "conjoint": false, "enfants": 0});
        sal["DEBURAU"]["prevoyance_branche_depuis"] = json!("2024-01-08");
        sal["DEBURAU"]["sante_dispense"] = json!("9.2.2 b : assurance individuelle jusqu'a son echeance");
        sal["DEBURAU"]["sante_dispense_jusqu_au"] = json!("2026-09-30");
    });
    let ctx = copie.contexte("2026-10-28");
    let s = saisie(&ctx, "2026-10", json!([cdi("Deburau", "DE5.26.09", "2026-09-01")])).unwrap();
    assert!(s.paies[0].lignes.iter().any(|l| l.id == "mutuelle"), "octobre : affilie, la mutuelle");
    let dsn = generer(&ctx, &s.paies, "2026-10", &[], &[], &Options::default()).unwrap();
    let a = affiliations(&dsn, "DE5.26.09");
    let sante = affiliation(&a, "101").unwrap_or_else(|| panic!("octobre : affilie a la sante ({:?})", a));
    assert_eq!(valeur(sante, "S21.G00.70.014").as_deref(), Some("01102026"));
}

/// La fiche propose la dispense et ses bornes aux permanents des deux conventions qui en ont (1285, 3090).
#[test]
fn la_fiche_propose_la_dispense_de_sante_en_ccneac_et_en_3090() {
    let d = paie::champs::description("salarie").unwrap();
    for cle in ["sante_dispense", "sante_dispense_depuis", "sante_dispense_jusqu_au"] {
        let c = d.as_array().unwrap().iter().find(|c| c["cle"] == json!(cle)).unwrap_or_else(|| panic!("{}", cle));
        assert_eq!(c["si_entreprise"], json!({"idcc": ["1285", "3090"]}), "{}", cle);
    }
}

/// Une fiche qui porte une dispense que le bulletin ne suit pas (calcule avant qu'elle soit posee) : la DSN ne choisit
/// pas entre l'affiliation et la dispense.
#[test]
fn une_dispense_de_sante_que_le_bulletin_ne_suit_pas_refuse() {
    let copie = copie_de("fictive-3090-grande", |sal, _| {
        sal["DEBURAU"]["sante_svp"] = json!({"niveau": "1", "conjoint": false, "enfants": 0});
        sal["DEBURAU"]["prevoyance_branche_depuis"] = json!("2024-01-08");
    });
    let ctx = copie.contexte("2026-10-28");
    let s = saisie(&ctx, "2026-10", json!([cdi("Deburau", "DE5.26.09", "2026-09-01")])).unwrap();
    let mut p = s.paies[0].clone();
    assert!(p.lignes.iter().any(|l| l.id == "mutuelle"));
    p.salarie["sante_dispense"] = json!("couverture ailleurs");
    let orgs = lire(&copie.0, "config/organismes.json");
    let bar = ctx.bareme(&Jour::lire("2026-10-31").unwrap()).unwrap();
    let mut f = Fichier::en_norme(paie::norme::de_la_mensuelle("2026-10").unwrap());
    let e = bloc_contrat(&mut f, &p, &ctx.entreprise, &orgs, 1, &bar, false, &[], "01", "01").unwrap_err();
    assert!(e.contains("sante_dispense"), "{}", e);
}

/// Q13 (tranchee par Leandre le 11/10/2026) : une dispense de sante a des bornes. Les accords de branche deposes en
/// donnent deux -- l'echeance du contrat individuel, la justification annuelle (3090 art. 9.2.2 b et d,
/// `atelier/reference/sources/kali-3090-accord-sante-permanents-20240719-20261011.md` ; CCNEAC XII.2.1.8) -- et 70.014
/// nomme « fin d'une dispense d'affiliation » (`bloc-S21.G00.71.txt`, p. 287). Dispense jusqu'au 30/06 : en juin pas
/// de mutuelle ni d'affiliation a la sante ; en juillet, affilie comme un autre, l'affiliation datee du 01/07.
#[test]
fn une_dispense_de_sante_jusqu_au_30_juin_laisse_le_salarie_affilie_en_juillet() {
    let copie = copie_de("fictive-1285-grande", |sal, _| {
        sal["DUMESNIL"]["sante_dispense"] = json!("assurance individuelle jusqu'a son echeance");
        sal["DUMESNIL"]["sante_dispense_jusqu_au"] = json!("2026-06-30");
    });
    let ctx = copie.contexte("2026-10-28");
    let juin = saisie(&ctx, "2026-06", json!([cdi("Dumesnil", "DU5.26.01", "2026-01-05")])).unwrap();
    assert!(juin.paies[0].lignes.iter().all(|l| l.id != "mutuelle"), "juin : dispense, pas de mutuelle");
    let dsn = generer(&ctx, &juin.paies, "2026-06", &[], &[], &Options::default()).unwrap();
    assert!(affiliation(&affiliations(&dsn, "DU5.26.01"), "107").is_none(), "juin : pas d'affiliation a la sante");

    let juillet = saisie(&ctx, "2026-07", json!([cdi("Dumesnil", "DU5.26.01", "2026-01-05")])).unwrap();
    assert!(juillet.paies[0].lignes.iter().any(|l| l.id == "mutuelle"), "juillet : affilie, la mutuelle");
    let dsn = generer(&ctx, &juillet.paies, "2026-07", &[], &[], &Options::default()).unwrap();
    let a = affiliations(&dsn, "DU5.26.01");
    let sante = affiliation(&a, "107").unwrap_or_else(|| panic!("juillet : affilie a la sante ({:?})", a));
    assert_eq!(valeur(sante, "S21.G00.70.014").as_deref(), Some("01072026"), "l'affiliation commence a la fin de la dispense");
}

/// Q13 : une borne de dispense en cours de periode payee ne se devine pas (la cotisation du mois se partagerait) : refus
/// qui dit la date et quoi faire. Une dispense qui commence avec le contrat n'est pas « en cours de periode ».
#[test]
fn une_dispense_qui_finit_en_cours_de_mois_refuse_et_une_qui_commence_avec_le_contrat_passe() {
    let copie = copie_de("fictive-1285-grande", |sal, _| {
        sal["DUMESNIL"]["sante_dispense"] = json!("assurance individuelle jusqu'a son echeance");
        sal["DUMESNIL"]["sante_dispense_jusqu_au"] = json!("2026-07-15");
    });
    let ctx = copie.contexte("2026-10-28");
    let e = saisie(&ctx, "2026-07", json!([cdi("Dumesnil", "DU5.26.01", "2026-01-05")])).map(|_| ()).unwrap_err();
    assert!(e.contains("15/07/2026") && e.contains("dispense"), "{}", e);
    let copie = copie_de("fictive-1285-grande", |sal, _| {
        sal["DUMESNIL"]["sante_dispense"] = json!("CDD de moins de douze mois");
        sal["DUMESNIL"]["sante_dispense_depuis"] = json!("2026-07-13");
    });
    let ctx = copie.contexte("2026-10-28");
    let s = saisie(&ctx, "2026-07", json!([cdi("Dumesnil", "DU5.26.07", "2026-07-13")])).unwrap();
    assert!(s.paies[0].lignes.iter().all(|l| l.id != "mutuelle"));
}

/// Q13 : la fiche controle les bornes d'une dispense -- des dates sans motif, une fin avant le debut.
#[test]
fn les_bornes_d_une_dispense_se_controlent_a_la_fiche() {
    let fiche = |v: Value| -> Vec<Value> {
        let base = lire(&crate::outils::fictive("fictive-1285-grande"), "config/salaries.json")["DUMESNIL"].clone();
        let mut m = base.as_object().unwrap().clone();
        for (k, x) in v.as_object().unwrap() {
            m.insert(k.clone(), x.clone());
        }
        paie::champs::verifier_fiche("salarie", &m).unwrap()
    };
    let sur = |a: &[Value], cle: &str| a.iter().any(|x| x["champ"] == json!(cle) && x["gravite"] == json!("erreur"));
    let a = fiche(json!({"sante_dispense_jusqu_au": "2026-06-30"}));
    assert!(sur(&a, "sante_dispense"), "des dates sans motif : {:?}", a);
    let a = fiche(json!({"sante_dispense": "assurance individuelle", "sante_dispense_depuis": "2026-07-01", "sante_dispense_jusqu_au": "2026-06-30"}));
    assert!(sur(&a, "sante_dispense_jusqu_au"), "la fin avant le debut : {:?}", a);
    let a = fiche(json!({"sante_dispense": "assurance individuelle", "sante_dispense_jusqu_au": "2026-06-30"}));
    assert!(!sur(&a, "sante_dispense") && !sur(&a, "sante_dispense_jusqu_au"), "{:?}", a);
}
