//! Un changement de statut, de categorie ou de duree du travail en cours de mois. La paie du mois se decoupe
//! en periodes elementaires, chacune sur sa pile de bareme et sa part du plafond, et la DSN declare les remunerations
//! (51) et les bases (78, 79, 81) de chacune, datees de ses bornes, avec le bloc 41 date du jour du changement.
//!
//! Textes : FAQ Agirc-Arrco « Changement de situation en cours de mois » (v2, exemples 1 et 2,
//! `atelier/reference/sources/agirc-arrco/`) ; fiche net-entreprises 1265, l. 52 ; norme DSN 2026.1, § 2.1.3.2 a 2.1.3.5,
//! p. 28-29, et § 2.1.2.5, p. 22-23 ; CSS R. 242-2, I ; BOSS, assiette generale, § 700-770.
//!
//! Sur la copie figee de `fictive-1285-grande` (`tests/donnees/fictives/`, compagnie inventee : deux CDI, NADAR depuis 2022,
//! cadre, 3 010 EUR ; DORVAL depuis le 01/04/2025, technicienne non cadre, 2 200 EUR ; toutes les DSN deposees jusqu'a
//! aout 2026), copiee dans le dossier temporaire des tests avant d'y retoucher les fiches et les saisies.

use engagement::Saisie;
use engagement::calcul::Paie;
use socle::num::Decimal;
use serde_json::json;
use crate::outils::permanents::{bloc, blocs_41, contexte, deposer, dorval_cadre_au, dsn, retoucher_saisie, saisie};
use crate::outils::{compagnie::sans_les_pdf, dec, dossier_de_test, individu};
use std::path::{Path, PathBuf};

/// Une copie de la 1285-grande sans ses PDF, propre a un test.
fn compagnie(dossier: &str) -> PathBuf {
    let r = sans_les_pdf("fictive-1285-grande", &dossier_de_test("periodes", dossier));
    // La DSN d'aout, deposee : elle dit sur quelle version de chaque engagement elle est partie.
    crate::outils::dsn_deposee(&r, "2026-08", "DSN_Aout2026.dsn", "2026-09-05T12:00:00");
    r
}

/// La valeur de la rubrique, ou rien.
fn valeur(lignes: &[(String, String)], r: &str) -> String {
    crate::outils::valeur(lignes, r).unwrap_or_default()
}

fn paie_de<'a>(s: &'a Saisie, cle: &str) -> &'a Paie {
    s.paies.iter().find(|p| p.salarie["_cle"] == cle).unwrap_or_else(|| panic!("pas de paie pour {}", cle))
}

fn bornes(p: &Paie) -> Vec<(String, String)> {
    p.periodes.iter().map(|q| { let (d, f) = q.engagement.periode.unwrap(); (d.iso(), f.iso()) }).collect()
}

fn ids(p: &Paie) -> Vec<String> {
    p.lignes.iter().map(|l| l.id.clone()).collect()
}

/// DORVAL passe a temps partiel (70 h, 1 015,36 EUR) par un avenant qui prend effet le 16/09/2026 ; avant, 151,67 h et
/// 2 200 EUR (la saisie le dit : `avant`). FAQ Agirc-Arrco, exemple 2 (le 16 avril, 151,67 h -> 70 h).
fn dorval_temps_partiel_au_16(racine: &Path) {
    retoucher_saisie(
        racine,
        "2026-09",
        "Dorval",
        json!({
            "salaire_mensuel": 1015.36,
            "quotite_contrat": 70.0,
            "date_modification": "2026-09-16",
            "avant": {"salaire_mensuel": 2200.0, "quotite_contrat": 151.67}
        }),
    );
}

// ------------------------------------------------------------------ la paie

#[test]
fn un_passage_cadre_au_15_decoupe_la_paie_en_deux_periodes_chacune_sur_sa_pile_et_sa_part_du_plafond() {
    let racine = compagnie("cadre-15");
    dorval_cadre_au(&racine, "2026-09-14");
    let ctx = contexte(&racine, "2026-09");
    let s = saisie(&racine, "2026-09").unwrap();
    let p = paie_de(&s, "DORVAL");
    assert_eq!(bornes(p), vec![("2026-09-01".into(), "2026-09-14".into()), ("2026-09-15".into(), "2026-09-30".into())]);
    let (avant, apres) = (&p.periodes[0], &p.periodes[1]);
    // Le salaire et le plafond de chaque periode, au prorata de ses jours (FAQ Agirc-Arrco, exemple 1 ; R. 242-2, I) :
    // 2 200 x 14/30 et 16/30 ; 4 005 x 14/30 et 16/30.
    assert_eq!((avant.brut, apres.brut, p.brut), (dec("1026.67"), dec("1173.33"), dec("2200.00")));
    assert_eq!((avant.plafond_urssaf(&ctx).unwrap(), apres.plafond_urssaf(&ctx).unwrap()), (dec("1869.00"), dec("2136.00")));
    assert_eq!(p.plafond_urssaf(&ctx).unwrap(), dec("4005.00"), "le plafond du mois est la somme des deux parts");
    assert_eq!((avant.bases["t1_mensuelle"], apres.bases["t1_mensuelle"]), (dec("1026.67"), dec("1173.33")));
    // La pile de chaque periode : non cadre, puis cadre (l'Apec, la prevoyance des cadres).
    assert!(!ids(avant).contains(&"apec_t1".to_string()) && ids(apres).contains(&"apec_t1".to_string()));
    assert_eq!(avant.salarie["statut_categoriel_rc"], "04");
    assert_eq!(apres.salarie["statut_categoriel_rc"], "01");
    let prev = |q: &Paie| q.ligne("prevoyance").map(|l| (l.base, l.taux_pat)).unwrap();
    assert_eq!(prev(avant), (dec("1026.67"), Some(0.95)));
    assert_eq!(prev(apres), (dec("1173.33"), Some(1.5)));
    // La paie du mois en est la somme : une ligne par taux (deux prevoyances), l'Apec sur la seule periode cadre, la
    // fiche de la derniere periode.
    let prevoyances: Vec<_> = p.lignes.iter().filter(|l| l.id == "prevoyance").map(|l| (l.base, l.taux_pat)).collect();
    assert_eq!(prevoyances, vec![(dec("1026.67"), Some(0.95)), (dec("1173.33"), Some(1.5))]);
    assert_eq!(p.ligne("apec_t1").unwrap().base, dec("1173.33"));
    assert_eq!(p.ligne("retraite_t1").unwrap().base, dec("2200.00"));
    assert_eq!(p.salarie["statut_categoriel_rc"], "01");
    let somme = |f: &dyn Fn(&Paie) -> Decimal| p.periodes.iter().map(|q| f(q)).sum::<Decimal>();
    assert_eq!(p.total_sal, somme(&|q| q.total_sal));
    assert_eq!(p.total_pat, somme(&|q| q.total_pat));
    assert_eq!(p.net_a_payer, somme(&|q| q.net_a_payer));
    assert_eq!(p.net_imposable, somme(&|q| q.net_imposable));
    assert_eq!(p.lignes.last().unwrap().id, "reduction_generale", "la reduction generale reste la derniere ligne");
    // La reduction generale : un coefficient pour le mois (2 200 EUR pour 151,67 h), chaque periode a ce coefficient.
    let rg = p.reduction_generale.as_ref().unwrap();
    let (r1, r2) = (avant.reduction_generale.as_ref().unwrap(), apres.reduction_generale.as_ref().unwrap());
    assert_eq!((r1.coefficient, r2.coefficient), (rg.coefficient, rg.coefficient));
    assert_eq!(rg.montant, r1.montant + r2.montant);
    assert_eq!(rg.heures, dec("151.67"));
    // Le meme mois sans changement (cadre au 01/10) : meme brut, meme coefficient.
    let racine = compagnie("cadre-octobre");
    dorval_cadre_au(&racine, "2026-09-30");
    let s = saisie(&racine, "2026-09").unwrap();
    let seul = paie_de(&s, "DORVAL");
    assert!(seul.periodes.is_empty());
    assert!(!ids(seul).contains(&"apec_t1".to_string()), "cadre au 01/10 : septembre reste non cadre, d'un seul tenant");
    assert_eq!(seul.reduction_generale.as_ref().unwrap().coefficient, rg.coefficient);
    assert_eq!(seul.reduction_generale.as_ref().unwrap().montant, rg.montant);
}

#[test]
fn un_passage_a_temps_partiel_au_16_proratise_le_salaire_et_le_plafond_de_la_seconde_periode() {
    let racine = compagnie("temps-partiel-16");
    dorval_temps_partiel_au_16(&racine);
    let ctx = contexte(&racine, "2026-09");
    let s = saisie(&racine, "2026-09").unwrap();
    let p = paie_de(&s, "DORVAL");
    assert_eq!(bornes(p), vec![("2026-09-01".into(), "2026-09-15".into()), ("2026-09-16".into(), "2026-09-30".into())]);
    let (avant, apres) = (&p.periodes[0], &p.periodes[1]);
    // Avant : 2 200 x 15/30, plafond 4 005 x 15/30. Apres : 1 015,36 x 15/30, plafond 4 005 x 15/30 x 70 / 151,67 (le
    // temps partiel proratise par defaut, lot R5a ; BOSS § 730, les ajustements se cumulent ; FAQ Agirc-Arrco, ex. 2).
    assert_eq!((avant.brut, apres.brut), (dec("1100.00"), dec("507.68")));
    assert_eq!((avant.plafond_urssaf(&ctx).unwrap(), apres.plafond_urssaf(&ctx).unwrap()), (dec("2002.50"), dec("924.21")));
    assert_eq!(p.plafond_urssaf(&ctx).unwrap(), dec("2926.71"));
    assert_eq!((avant.engagement.quotite().unwrap(), apres.engagement.quotite().unwrap()), (dec("151.67"), dec("70")));
    assert_eq!((avant.engagement.modalite_temps_travail().unwrap(), apres.engagement.modalite_temps_travail().unwrap()), ("10".into(), "20".into()));
    // Les heures de chaque periode : la duree du travail x la part du mois (75,83 h : les heures d'une ligne se lisent au
    // centieme, `catalogue::Rubrique::heures`).
    assert_eq!((avant.heures_travaillees, apres.heures_travaillees), (dec("75.83"), dec("35")));
    assert_eq!(p.brut, dec("1607.68"));
}

#[test]
fn un_avenant_en_cours_de_mois_dit_ses_termes_d_avant_entiers() {
    let racine = compagnie("avant-incomplet");
    retoucher_saisie(&racine, "2026-09", "Dorval", json!({"quotite_contrat": 70.0, "date_modification": "2026-09-16", "avant": {"quotite_contrat": 151.67}}));
    let e = saisie(&racine, "2026-09").err().expect("le salaire d'avant manque");
    assert!(e.contains("`salaire_mensuel`") && e.contains("ne les devine pas"), "{}", e);
    retoucher_saisie(&racine, "2026-09", "Dorval", json!({"avant": {"quotite_contrat": 151.67, "salaire_mensuel": 2200.0, "nature": "cdd"}}));
    let e = saisie(&racine, "2026-09").err().expect("une nature d'avant ne se decoupe pas");
    assert!(e.contains("`nature`"), "{}", e);
}

// ------------------------------------------------------------------ la DSN

/// Les blocs (rubriques d'un meme bloc, ouverts par `ouvre`) d'un individu : `prefixe` dit le bloc ("S21.G00.51.").
fn blocs(lignes: &[(String, String)], prefixe: &str, ouvre: &str) -> Vec<Vec<(String, String)>> {
    let mut out: Vec<Vec<(String, String)>> = Vec::new();
    for (k, v) in lignes {
        if k == ouvre {
            out.push(Vec::new());
        }
        if k.starts_with(prefixe) {
            if let Some(b) = out.last_mut() {
                b.push((k.clone(), v.clone()));
            }
        }
    }
    out
}

/// Les bases 78 d'un individu : (code, debut, fin, montant, codes 81 rattaches, composants 79).
fn bases_78(lignes: &[(String, String)]) -> Vec<(String, String, String, String, Vec<String>, Vec<(String, String)>)> {
    let mut out: Vec<(String, String, String, String, Vec<String>, Vec<(String, String)>)> = Vec::new();
    for (k, v) in lignes {
        match k.as_str() {
            "S21.G00.78.001" => out.push((v.clone(), String::new(), String::new(), String::new(), Vec::new(), Vec::new())),
            "S21.G00.78.002" => out.last_mut().unwrap().1 = v.clone(),
            "S21.G00.78.003" => out.last_mut().unwrap().2 = v.clone(),
            "S21.G00.78.004" => out.last_mut().unwrap().3 = v.clone(),
            "S21.G00.81.001" => out.last_mut().unwrap().4.push(v.clone()),
            "S21.G00.79.001" => out.last_mut().unwrap().5.push((v.clone(), String::new())),
            "S21.G00.79.004" => out.last_mut().unwrap().5.last_mut().unwrap().1 = v.clone(),
            k if k.starts_with("S21.G00.5") || k.starts_with("S21.G00.86") => {
                // Un autre bloc commence : plus rien ne se rattache a la derniere base.
                if !out.is_empty() && !k.starts_with("S21.G00.81") {
                    out.push(("--".into(), String::new(), String::new(), String::new(), Vec::new(), Vec::new()));
                }
            }
            _ => {}
        }
    }
    out.retain(|b| b.0 != "--");
    out
}

#[test]
fn la_dsn_d_un_passage_cadre_au_15_declare_les_remunerations_et_les_bases_de_chaque_periode() {
    let racine = compagnie("cadre-15-dsn");
    dorval_cadre_au(&racine, "2026-09-14");
    let sept = dsn(&racine, "2026-09").unwrap();
    paie::usages::verifier_fichier(&sept).unwrap();
    paie::formats::verifier_fichier(&sept).unwrap();
    let d = individu(&sept, "DORVAL");
    // Le bloc 40 porte la valeur de fin de mois (§ 2.1.2.4, p. 22) ; le bloc 41 l'ancienne, datee du jour (41.001), la
    // profondeur de recalcul au premier du mois (§ 2.1.2.5, p. 22-23, l'exemple du 15 mars).
    assert_eq!((valeur(&d, "S21.G00.40.002"), valeur(&d, "S21.G00.40.003")), ("04".into(), "01".into()));
    assert_eq!(
        blocs_41(&d),
        vec![
            bloc(&[("S21.G00.41.001", "15092026"), ("S21.G00.41.002", "05"), ("S21.G00.41.028", "01092026")]),
            bloc(&[("S21.G00.41.001", "15092026"), ("S21.G00.41.003", "04"), ("S21.G00.41.028", "01092026")]),
        ]
    );
    // Les remunerations : quatre types par periode elementaire, datees de ses bornes (§ 2.1.3.2, p. 28).
    let r51: Vec<(String, String, String, String)> = blocs(&d, "S21.G00.51.", "S21.G00.51.001")
        .iter()
        .map(|b| (valeur(b, "S21.G00.51.001"), valeur(b, "S21.G00.51.002"), valeur(b, "S21.G00.51.011"), valeur(b, "S21.G00.51.013")))
        .filter(|r| r.2 == "001")
        .collect();
    assert_eq!(
        r51,
        vec![
            ("01092026".into(), "14092026".into(), "001".into(), "1026.67".into()),
            ("15092026".into(), "30092026".into(), "001".into(), "1173.33".into()),
        ]
    );
    // L'activite, sous le 002 de chaque periode (§ 2.1.3.5, p. 29) : les heures de la periode.
    let heures: Vec<String> = d.iter().filter(|(k, _)| k == "S21.G00.53.002").map(|(_, v)| v.clone()).collect();
    assert_eq!(heures, vec!["70.78".to_string(), "80.89".to_string()]);
    // Les bases : 02 et 03 de chaque periode, datees de ses bornes (FAQ Agirc-Arrco, exemple 1) ; l'Apec (132) sous la
    // seule periode cadre ; le SMIC de reference (79.004) des heures de la periode sous chaque base 03.
    let b = bases_78(&d);
    let de = |code: &str, debut: &str| b.iter().find(|x| x.0 == code && x.1 == debut).unwrap_or_else(|| panic!("pas de base {} au {} : {:?}", code, debut, b));
    assert_eq!((de("02", "01092026").2.as_str(), de("02", "01092026").3.as_str()), ("14092026", "1026.67"));
    assert_eq!((de("02", "15092026").2.as_str(), de("02", "15092026").3.as_str()), ("30092026", "1173.33"));
    assert_eq!(de("03", "01092026").3, "1026.67");
    assert_eq!(de("03", "15092026").3, "1173.33");
    let apec: Vec<&String> = b.iter().filter(|x| x.4.contains(&"132".to_string())).map(|x| &x.1).collect();
    assert_eq!(apec, vec!["15092026"], "l'Apec sur la seule periode cadre : {:?}", b);
    assert!(b.iter().all(|x| x.1 == "01092026" || x.1 == "15092026"), "aucune base datee du mois entier : {:?}", b);
    // La reduction generale du mois, partagee : 018 sous la base 03 de chaque periode, au coefficient du mois.
    assert!(de("03", "01092026").4.contains(&"018".to_string()) && de("03", "15092026").4.contains(&"018".to_string()));
    assert_eq!(de("03", "01092026").5, vec![("01".to_string(), "850.78".to_string())]);
    assert_eq!(de("03", "15092026").5, vec![("01".to_string(), "972.30".to_string())]);
    // NADAR, sans changement : ses bases sont datees du mois entier.
    let n = bases_78(&individu(&sept, "NADAR"));
    assert!(n.iter().all(|x| x.1 == "01092026" && x.2 == "30092026"), "{:?}", n);
    // Septembre depose, octobre ne redeclare rien et ne se decoupe pas.
    deposer(&racine, "2026-09", "DSN_Septembre2026.dsn", &sept);
    let mut s = crate::outils::saisie(&racine, "2026-10");
    s["engagements"].as_array_mut().unwrap().retain(|e| e["nature"] == "cdi");
    crate::outils::poser_saisie(&racine, "2026-10", &s);
    let oct = dsn(&racine, "2026-10").unwrap();
    assert!(!oct.contains("S21.G00.41."), "octobre ne redeclare pas le changement");
    let o = bases_78(&individu(&oct, "DORVAL"));
    assert!(o.iter().all(|x| x.1 == "01102026" && x.2 == "31102026"), "{:?}", o);
    assert!(o.iter().any(|x| x.4.contains(&"132".to_string())), "octobre, cadre tout le mois");
}

#[test]
fn la_dsn_d_un_passage_a_temps_partiel_au_16_declare_l_ancienne_duree_au_jour_et_les_bases_de_chaque_periode() {
    let racine = compagnie("temps-partiel-16-dsn");
    dorval_temps_partiel_au_16(&racine);
    let sept = dsn(&racine, "2026-09").unwrap();
    paie::usages::verifier_fichier(&sept).unwrap();
    paie::formats::verifier_fichier(&sept).unwrap();
    let d = individu(&sept, "DORVAL");
    assert_eq!((valeur(&d, "S21.G00.40.013"), valeur(&d, "S21.G00.40.014")), ("70.00".into(), "20".into()));
    assert_eq!(
        blocs_41(&d),
        vec![
            bloc(&[("S21.G00.41.001", "16092026"), ("S21.G00.41.007", "151.67"), ("S21.G00.41.028", "01092026")]),
            bloc(&[("S21.G00.41.001", "16092026"), ("S21.G00.41.008", "10"), ("S21.G00.41.028", "01092026")]),
        ]
    );
    let b = bases_78(&d);
    let de = |code: &str, debut: &str| b.iter().find(|x| x.0 == code && x.1 == debut).unwrap_or_else(|| panic!("pas de base {} au {} : {:?}", code, debut, b));
    assert_eq!((de("03", "01092026").2.as_str(), de("03", "01092026").3.as_str()), ("15092026", "1100.00"));
    assert_eq!((de("03", "16092026").2.as_str(), de("03", "16092026").3.as_str()), ("30092026", "507.68"));
    let heures: Vec<String> = d.iter().filter(|(k, _)| k == "S21.G00.53.002").map(|(_, v)| v.clone()).collect();
    assert_eq!(heures, vec!["75.83".to_string(), "35.00".to_string()]);
}

#[test]
fn le_bulletin_d_un_mois_decoupe_porte_les_lignes_des_deux_periodes_et_le_plafond_du_mois() {
    let racine = compagnie("cadre-15-bulletin");
    dorval_cadre_au(&racine, "2026-09-14");
    let ctx = contexte(&racine, "2026-09");
    let s = saisie(&racine, "2026-09").unwrap();
    let p = paie_de(&s, "DORVAL").clone();
    let ent = ctx.entreprise_du_mois("2026-09").unwrap();
    let d = paie::bulletin::jeux_et_description(&ctx, &[p], &ent, "2026-09", None, true, &std::collections::HashMap::new()).unwrap();
    let t = d.to_string();
    // Le plafond du mois (la somme des deux parts), les deux prevoyances, l'Apec de la periode cadre.
    assert!(t.contains("4 005,00") || t.contains("4005"), "le plafond du mois : {}", t);
    assert!(t.contains("1 173,33") && t.contains("1 026,67"), "les deux periodes : {}", t);
}
