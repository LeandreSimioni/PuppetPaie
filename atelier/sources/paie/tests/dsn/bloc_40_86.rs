//! Corrections DSN, vague V2b (points 8, 9 et 14 de la relecture) : le bloc 40 et le bloc 86 ne devinent plus rien.
//!
//! - `S21.G00.40.002` et `.003` se lisent (la fiche, l'engagement, a defaut le metier du catalogue) ; ce qui ne se lit
//!   pas refuse en disant quoi saisir, et la CCH-11 (40.003 = 01 => 40.002 dans 03, 04, 08) se verifie avant de
//!   produire. Cahier technique DSN 2026.1, p. 182-183 (`atelier/reference/norme-dsn/texte/bloc-S21.G00.40.txt`,
//!   l. 133-198).
//! - `S21.G00.86.003` : la date d'anciennete de la fiche (decision de Leandre du 23/09/2026 : « la date de la creation
//!   de la fiche », `prive/journal/archives/2026-09-23.md`, l. 669-673) ; sans elle, ou si elle tombe apres le mois,
//!   la DSN refuse. Plus de `2019-12-01`, date de la compagnie d'essai. Norme p. 322 (`bloc-S21.G00.86.txt`).
//! - `S21.G00.40.005` d'un permanent (natures `cdi`, `cdd`) : « 999SPT pour les emplois permanents du spectacle »
//!   (CCH-18, p. 185, `bloc-S21.G00.40.txt` l. 325-329), tranche par Leandre le 10/10/2026 ; un CDDU garde son code.
//!
//! Sur une copie de `fictive-3090-grande` (compagnie inventee). La nature qui fixe 999SPT et les statuts que la fiche admet
//! se prouvent dans `src/contrats.rs` et `src/champs.rs`.

use crate::outils::{self, compagnie, fictive};
use serde_json::json;
use std::path::PathBuf;

fn copie(test: &str) -> PathBuf {
    compagnie::ses_dsn("fictive-3090-grande", &[], &outils::dossier_de_test("bloc_40_86", test))
}

/// La DSN de juillet 2026 de la 3090-grande (deux CDI, MONTANSIER et NICOLET, et des CDDU), les fiches retouchees
/// par `retouches` (cle -> champs poses ; `null` efface).
fn dsn_de_juillet(test: &str, retouches: &[(&str, serde_json::Value)]) -> Result<String, String> {
    let r = copie(test);
    outils::retoucher_fiches(&r, retouches);
    outils::dsn_du_mois(&r, "2026-07", "2026-07-28")
}

/// Les rubriques de chaque contrat (bloc 40) et de chaque anciennete (bloc 86), par nom de l'individu
/// (`S21.G00.30.002`), dans l'ordre du fichier.
fn blocs(dsn: &str, prefixe: &str, ouvre: &str) -> Vec<(String, Vec<(String, String)>)> {
    let (mut nom, mut out): (String, Vec<(String, Vec<(String, String)>)>) = (String::new(), Vec::new());
    for l in dsn.lines() {
        let Some((k, v)) = l.trim().split_once(',') else { continue };
        let v = v.trim().trim_matches('\'').to_string();
        if k == "S21.G00.30.002" {
            nom = v.clone();
        }
        if k == ouvre {
            out.push((nom.clone(), Vec::new()));
        }
        if k.starts_with(prefixe) {
            if let Some(dernier) = out.last_mut() {
                dernier.1.push((k.to_string(), v));
            }
        }
    }
    out
}

fn contrats(dsn: &str) -> Vec<(String, Vec<(String, String)>)> {
    blocs(dsn, "S21.G00.40.", "S21.G00.40.001")
}

fn rubrique(bloc: &[(String, String)], r: &str) -> String {
    bloc.iter().find(|(k, _)| k == r).map(|(_, v)| v.clone()).unwrap_or_default()
}

/// (40.002, 40.003) de chaque contrat de `nom`.
fn statuts(dsn: &str, nom: &str) -> Vec<(String, String)> {
    contrats(dsn).iter().filter(|(n, _)| n == nom).map(|(_, b)| (rubrique(b, "S21.G00.40.002"), rubrique(b, "S21.G00.40.003"))).collect()
}

// ------------------------------------------------------------------ point 8 : 40.002 et 40.003

#[test]
fn une_fiche_sans_metier_du_catalogue_ni_categorie_refuse_en_nommant_les_deux_codes() {
    // ACC050 (accordeoniste) est dans la nomenclature France Travail, pas dans le catalogue : rien n'y dit le classement.
    let e = dsn_de_juillet(
        "sans-categorie",
        &[("BIANCOLELLI", json!({"code_emploi_ft": "ACC050", "statut_conventionnel": null, "statut_categoriel_rc": null}))],
    )
    .unwrap_err();
    assert!(e.contains("S21.G00.40.002") && e.contains("S21.G00.40.003"), "la phrase nomme les deux codes : {}", e);
    assert!(e.contains("BIANCOLELLI"), "la phrase nomme le salarie : {}", e);
    assert!(e.contains("statut_categoriel_rc") && e.contains("statut_conventionnel"), "la phrase dit quoi saisir : {}", e);
}

#[test]
fn un_metier_du_catalogue_dit_le_classement_que_la_fiche_tait() {
    // Une habilleuse : ouvrier (07) au catalogue, non cadre. Le defaut d'avant posait 05.
    let dsn = dsn_de_juillet(
        "habilleuse",
        &[("SERVANDONI", json!({"code_emploi_ft": "HAB010", "pcs_ese": "637C", "statut_conventionnel": null, "statut_categoriel_rc": null}))],
    )
    .unwrap();
    let s = statuts(&dsn, "SERVANDONI");
    assert!(!s.is_empty());
    assert!(s.iter().all(|x| x == &("07".to_string(), "04".to_string())), "{:?}", s);
}

#[test]
fn un_cadre_permanent_ecrit_01_et_un_statut_que_la_cch_11_admet() {
    // La fiche dit cadre (01) et rien du classement : le catalogue (ADM020, cadre) le dit, 04.
    let dsn = dsn_de_juillet("cadre", &[("MONTANSIER", json!({"statut_conventionnel": null, "statut_categoriel_rc": "01"}))]).unwrap();
    let s = statuts(&dsn, "MONTANSIER");
    assert_eq!(s, vec![("04".to_string(), "01".to_string())]);
    // La fiche de la compagnie fictive, telle quelle, ne change pas.
    let dsn = dsn_de_juillet("cadre-tel-quel", &[]).unwrap();
    assert_eq!(statuts(&dsn, "MONTANSIER"), vec![("04".to_string(), "01".to_string())]);
}

#[test]
fn un_cadre_classe_profession_intermediaire_ne_part_pas() {
    // CCH-11 : 40.003 = 01 exige 40.002 dans 03, 04 ou 08 ; contrôle bloquant, verifie avant de produire.
    let e = dsn_de_juillet("cch-11", &[("MONTANSIER", json!({"statut_conventionnel": "05", "statut_categoriel_rc": "01"}))]).unwrap_err();
    assert!(e.contains("CCH-11") && e.contains("MONTANSIER") && e.contains("05"), "{}", e);
}

#[test]
fn un_metier_cadre_au_catalogue_sans_categorie_sur_la_fiche_refuse() {
    // ADM020 est cadre au catalogue ; la fiche ne dit ni cadre ni non cadre, et la paie s'est calculee non cadre :
    // ni 01 (les cotisations des cadres n'y sont pas) ni 04 (le catalogue dit cadre) ne se lisent.
    let e = dsn_de_juillet("cadre-muet", &[("MONTANSIER", json!({"statut_conventionnel": null, "statut_categoriel_rc": null}))]).unwrap_err();
    assert!(e.contains("S21.G00.40.003") && e.contains("statut_categoriel_rc") && e.contains("MONTANSIER"), "{}", e);
}

// ------------------------------------------------------------------ point 9 : 86.003

/// (86.003, 86.005) de chaque anciennete de `nom`.
fn anciennetes(dsn: &str, nom: &str) -> Vec<(String, String)> {
    blocs(dsn, "S21.G00.86.", "S21.G00.86.001")
        .iter()
        .filter(|(n, _)| n == nom)
        .map(|(_, b)| (rubrique(b, "S21.G00.86.003"), rubrique(b, "S21.G00.86.005")))
        .collect()
}

#[test]
fn l_anciennete_part_de_la_date_de_la_fiche() {
    let dsn = dsn_de_juillet("anciennete", &[]).unwrap();
    // MONTANSIER : 2023-09-04 au 31/07/2026, bornes comprises.
    let a = anciennetes(&dsn, "MONTANSIER");
    assert!(!a.is_empty() && a.iter().all(|(j, _)| j == "1062"), "{:?}", a);
}

#[test]
fn une_fiche_sans_date_d_anciennete_refuse_au_lieu_de_prendre_2019() {
    let e = dsn_de_juillet("sans-anciennete", &[("BIANCOLELLI", json!({"date_anciennete": null}))]).unwrap_err();
    assert!(e.contains("S21.G00.86.003") && e.contains("date_anciennete") && e.contains("BIANCOLELLI"), "{}", e);
    assert!(!e.contains("2019"), "{}", e);
}

#[test]
fn une_date_d_anciennete_apres_le_mois_refuse() {
    // La fiche creee le 1er aout pour un contrat de juillet : zero jour, que la CCH-12 interdit (p. 322).
    let e = dsn_de_juillet("anciennete-apres", &[("BIANCOLELLI", json!({"date_anciennete": "2026-08-01"}))]).unwrap_err();
    assert!(e.contains("S21.G00.86.003") && e.contains("01/08/2026") && e.contains("BIANCOLELLI"), "{}", e);
}

// ------------------------------------------------------------------ point 14 : 40.005 d'un permanent

#[test]
fn un_cdi_ecrit_999spt_un_cddu_garde_son_code() {
    let dsn = dsn_de_juillet("999spt", &[]).unwrap();
    let c = contrats(&dsn);
    let code = |nom: &str| -> Vec<(String, String)> {
        c.iter().filter(|(n, _)| n == nom).map(|(_, b)| (rubrique(b, "S21.G00.40.007"), rubrique(b, "S21.G00.40.005"))).collect()
    };
    // MONTANSIER (ADM020) et NICOLET (CHA080) sont en CDI (40.007 = 01).
    assert_eq!(code("MONTANSIER"), vec![("01".to_string(), "999SPT".to_string())]);
    assert_eq!(code("NICOLET"), vec![("01".to_string(), "999SPT".to_string())]);
    // Un intermittent garde le code de la nomenclature des emplois du spectacle.
    let b = code("BIANCOLELLI");
    assert!(!b.is_empty() && b.iter().all(|x| x == &("02".to_string(), "COM010".to_string())), "{:?}", b);
}

#[test]
fn un_permanent_sans_code_emploi_france_travail_se_declare() {
    // La nature decide : le code de la nomenclature n'est plus exige d'un permanent.
    let dsn = dsn_de_juillet("999spt-sans-code", &[("NICOLET", json!({"code_emploi_ft": null}))]).unwrap();
    let c = contrats(&dsn);
    let n: Vec<String> = c.iter().filter(|(n, _)| n == "NICOLET").map(|(_, b)| rubrique(b, "S21.G00.40.005")).collect();
    assert_eq!(n, vec!["999SPT".to_string()]);
}

/// La DSN de juillet 2026 de la 3090-grande reduite a MONTANSIER, la quotite de son contrat (CDI, en heures) posee a
/// `quotite`.
fn dsn_de_juillet_quotite(test: &str, quotite: f64) -> Result<String, String> {
    let r = copie(test);
    compagnie::seulement(&r, &["Montansier"]);
    let mut saisie = crate::outils::saisie(&r, "2026-07");
    for e in saisie["engagements"].as_array_mut().unwrap() {
        if e["numero"] == json!("MO.23.01") {
            e["quotite_contrat"] = json!(quotite);
            e["date_modification"] = json!("2026-07-01");
        }
    }
    crate::outils::poser_saisie(&r, "2026-07", &saisie);
    outils::dsn_du_mois(&r, "2026-07", "2026-07-28")
}

/// Reste R1 #58 : « CCH-13 : Si la rubrique « Unite de mesure de la quotite de travail - S21.G00.40.011 » est
/// renseignee avec la valeur « 10 - heure » ou « 21 - forfait heure » alors les rubriques [40.012] et [40.013] doivent
/// etre inferieures ou egales a « 250.00 ». » (cahier technique 2026.1, p. 189, `bloc-S21.G00.40.txt` l. 552-555 ; meme
/// texte en 2027.1.1). Le garde-fou des natures en heures etait a 3 250 (le plafond de la MESURE d'activite du bloc 53,
/// CCH-11 et CCH-13 de `bloc-S21.G00.53.txt`), et laissait partir une quotite que le point de depot rejette.
#[test]
fn une_quotite_en_heures_au_dela_de_250_refuse_selon_la_cch_13_du_bloc_40() {
    let e = dsn_de_juillet_quotite("quotite-260", 260.0).unwrap_err();
    assert!(e.contains("250.00") && e.contains("CCH-13"), "{}", e);
    let dsn = dsn_de_juillet_quotite("quotite-250", 250.0).unwrap();
    let c = contrats(&dsn);
    let m: Vec<String> = c.iter().filter(|(n, _)| n == "MONTANSIER").map(|(_, b)| rubrique(b, "S21.G00.40.013")).collect();
    assert_eq!(m, vec!["250.00".to_string()]);
}

/// Par individu (30.002) : (nombre de blocs 40, nombre de 78.006).
fn contrats_et_78006(dsn: &str) -> std::collections::BTreeMap<String, (usize, usize)> {
    let mut out = std::collections::BTreeMap::new();
    let mut nom = String::new();
    for l in dsn.lines() {
        let Some((k, v)) = l.split_once(',') else { continue };
        let v = v.trim().trim_matches('\'');
        match k {
            "S21.G00.30.002" => nom = v.to_string(),
            "S21.G00.40.001" => out.entry(nom.clone()).or_insert((0, 0)).0 += 1,
            "S21.G00.78.006" => out.entry(nom.clone()).or_insert((0, 0)).1 += 1,
            _ => {}
        }
    }
    out
}

/// Reste R1 #26 (fiche N3, S21.G00.78.006) : « Dans le cas de la declaration de plusieurs blocs "Contrat - S21.G00.40"
/// impliquant un decoupage des regles de calcul des cotisations, cette rubrique permet d'identifier le contrat concerne
/// pour chaque base assujettie. Dans le cas de la declaration d'un contrat unique, cette rubrique n'est pas a
/// renseigner. » (cahier technique 2026.1, p. 298, `bloc-S21.G00.79.txt` l. 9-14). Le moteur l'ecrivait sur chaque base
/// -- le depot fige de juillet 2026, fait par lui, le montre.
#[test]
fn le_numero_du_contrat_ne_s_ecrit_sur_les_bases_qu_avec_plusieurs_contrats() {
    let depose = String::from_utf8(std::fs::read(fictive("fictive-3090-grande").join("sorties/2026-07/DSN_Juillet2026.dsn")).unwrap()).unwrap();
    let avant = contrats_et_78006(&depose);
    assert!(avant.values().any(|(c, n)| *c == 1 && *n > 0), "le depot d'avant ecrivait 78.006 pour un contrat unique : {:?}", avant);
    // Mars 2026 : Biancolelli et Clairon y ont chacun deux contrats ; les autres, un.
    let dsn = outils::dsn_du_mois(&copie("78-006"), "2026-03", "2026-03-28").unwrap();
    let apres = contrats_et_78006(&dsn);
    assert!(apres.values().any(|(c, _)| *c == 1) && apres.values().any(|(c, _)| *c > 1), "{:?}", apres);
    for (nom, (contrats, numeros)) in &apres {
        if *contrats == 1 {
            assert_eq!(*numeros, 0, "{} : un contrat unique, pas de 78.006", nom);
        } else {
            assert!(*numeros > 0, "{} : {} contrats, 78.006 sur ses bases", nom, contrats);
        }
    }
}
