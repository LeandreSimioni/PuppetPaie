//! Corrections DSN, vague 3d : le bloc S21.G00.31 (changements individu). Quand un identifiant de l'individu (NIR, nom
//! de famille, prenoms, date de naissance) differe de ce qui a ete DECLARE pour lui -- relu dans la derniere DSN
//! reelle acceptee qui le porte, jamais recalcule --, la DSN porte l'ANCIENNE valeur, datee par l'historique de la fiche
//! (cahier technique 2026.1, p. 177-179 ; § 2.1.1, p. 18 ; § 2.1.2.2-2.1.2.3, p. 20-21 ;
//! `atelier/reference/norme-dsn/texte/bloc-S21.G00.31.txt`, `chapitre-2-principes-de-constitution.txt`).
//!
//! Chaque test monte une copie de `fictive-3090-grande` (compagnie inventee) dans un dossier a lui : les DSN de janvier
//! 2025 a aout 2026 y sont deposees (journal et `sorties/`). Aucun reseau, aucun depot.

use crate::outils::{self, compagnie, ecrire as ecrire_json, lire, mois_pret};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn copie(test: &str) -> PathBuf {
    compagnie::ses_dsn("fictive-3090-grande", &[], &outils::dossier_de_test("changements_individu", test))
}

/// La DSN de `mois` est deposee en reel et acceptee.
fn deposee(racine: &Path, mois: &str, fichier: &str) {
    outils::dsn_deposee(racine, mois, fichier, &format!("{}-28T12:00:00", mois));
}

/// Change la fiche `cle` : chaque (champ, nouvelle valeur, date d'effet) ; la date d'effet absente ne date rien
/// (une fiche retouchee a la main), sinon l'historique garde l'ancienne valeur jusqu'a la veille.
fn changer(racine: &Path, cle: &str, changements: &[(&str, &str, Option<&str>)]) {
    let mut s = lire(racine, "config/salaries.json");
    let f = s[cle].as_object_mut().unwrap();
    let mut hist: Vec<Value> = f.get("historique").and_then(Value::as_array).cloned().unwrap_or_default();
    for (champ, neuf, depuis) in changements {
        let avant = f[*champ].clone();
        if let Some(d) = depuis {
            let (a, m, j): (i32, u32, u32) = (d[..4].parse().unwrap(), d[5..7].parse().unwrap(), d[8..10].parse().unwrap());
            let veille = if j > 1 { format!("{:04}-{:02}-{:02}", a, m, j - 1) } else { panic!("un 1er du mois : a ecrire a la main") };
            hist.push(json!({"champ": champ, "valeur": avant, "jusqu_au": veille, "note": ""}));
        }
        f.insert((*champ).into(), json!(neuf));
    }
    if !hist.is_empty() {
        f.insert("historique".into(), Value::Array(hist));
    }
    ecrire_json(racine, "config/salaries.json", &s);
}

fn produire(racine: &Path, mois: &str, jour: &str) -> Value {
    outils::route(racine, "produire", json!({"mois": mois, "saisie": crate::outils::saisie(racine, &mois)}), jour)
}

/// (rubrique, valeur) d'un fichier DSN.
fn lignes(chemin: &Path) -> Vec<(String, String)> {
    outils::paires(&std::fs::read(chemin).unwrap().iter().map(|b| *b as char).collect::<String>())
}

/// Les lignes du bloc 30 et des blocs 31 de l'individu dont le matricule (30.019) est `matricule`.
fn individu(toutes: &[(String, String)], matricule: &str) -> Vec<(String, String)> {
    let mut blocs: Vec<Vec<(String, String)>> = Vec::new();
    for (k, v) in toutes {
        let debut = k == "S21.G00.30.001" || (k == "S21.G00.30.002" && blocs.last().map_or(true, |b| b.iter().any(|(r, _)| r == "S21.G00.30.002")));
        if debut {
            blocs.push(Vec::new());
        }
        if (k.starts_with("S21.G00.30.") || k.starts_with("S21.G00.31.")) && !blocs.is_empty() {
            blocs.last_mut().unwrap().push((k.clone(), v.clone()));
        }
    }
    let mut trouves: Vec<Vec<(String, String)>> = blocs.into_iter().filter(|b| b.iter().any(|(k, v)| k == "S21.G00.30.019" && v == matricule)).collect();
    assert_eq!(trouves.len(), 1, "un individu de matricule {}", matricule);
    trouves.remove(0)
}

fn valeur<'a>(b: &'a [(String, String)], k: &str) -> Option<&'a str> {
    b.iter().find(|(r, _)| r == k).map(|(_, v)| v.as_str())
}

/// Les blocs 31 d'un individu, chacun en (rubrique, valeur).
fn blocs_31(b: &[(String, String)]) -> Vec<Vec<(String, String)>> {
    let mut out: Vec<Vec<(String, String)>> = Vec::new();
    for (k, v) in b {
        if k == "S21.G00.31.001" {
            out.push(Vec::new());
        }
        if k.starts_with("S21.G00.31.") {
            out.last_mut().expect("un bloc 31 commence par sa date (31.001)").push((k.clone(), v.clone()));
        }
    }
    out
}

fn paire(k: &str, v: &str) -> (String, String) {
    (k.to_string(), v.to_string())
}

const SEPTEMBRE: &str = "sorties/2026-09/DSN_Septembre2026.dsn";
const OCTOBRE: &str = "sorties/2026-10/DSN_Octobre2026.dsn";

#[test]
fn un_nom_change_le_15_porte_un_bloc_31_ce_mois_la_et_plus_le_mois_suivant() {
    let r = copie("nom");
    // Clairon (matricule 6) a ete declaree en juillet 2026 ; son nom change le 15 septembre (mariage).
    changer(&r, "CLAIRON", &[("nom", "DUVAL", Some("2026-09-15"))]);
    mois_pret(&r, "2026-09");
    let rep = produire(&r, "2026-09", "2026-10-05");
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let b = individu(&lignes(&r.join(SEPTEMBRE)), "6");
    assert_eq!(valeur(&b, "S21.G00.30.002"), Some("DUVAL"), "le bloc 30 porte le nouveau nom");
    assert_eq!(blocs_31(&b), vec![vec![paire("S21.G00.31.001", "15092026"), paire("S21.G00.31.009", "CLAIRON")]]);
    // Le bloc 31 suit le bloc 30, avant le premier contrat.
    let toutes = lignes(&r.join(SEPTEMBRE));
    let i = toutes.iter().position(|(k, _)| k == "S21.G00.31.001").unwrap();
    assert!(toutes[i - 1].0.starts_with("S21.G00.30."), "{:?}", toutes[i - 1]);
    assert_eq!(toutes[i + 2].0, "S21.G00.40.001");
    // Les autres individus n'en portent pas.
    assert_eq!(toutes.iter().filter(|(k, _)| k == "S21.G00.31.001").count(), 1);

    // Septembre part et revient accepte : octobre declare DUVAL sans bloc 31.
    deposee(&r, "2026-09", "DSN_Septembre2026.dsn");
    mois_pret(&r, "2026-10");
    let rep = produire(&r, "2026-10", "2026-11-05");
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let toutes = lignes(&r.join(OCTOBRE));
    let b = individu(&toutes, "6");
    assert_eq!(valeur(&b, "S21.G00.30.002"), Some("DUVAL"));
    assert!(!toutes.iter().any(|(k, _)| k.starts_with("S21.G00.31.")), "octobre ne repete pas le changement");
}

#[test]
fn deux_identifiants_a_la_meme_date_font_un_bloc_et_deux_dates_deux_blocs() {
    let r = copie("dates");
    // Une date de naissance rectifiee et le nom le meme jour ; le prenom, cinq jours plus tard.
    changer(&r, "CLAIRON", &[("nom", "DUVAL", Some("2026-09-15")), ("date_naissance", "1990-10-03", Some("2026-09-15")), ("prenom", "LUCIE", Some("2026-09-20"))]);
    mois_pret(&r, "2026-09");
    let rep = produire(&r, "2026-09", "2026-10-05");
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let b = individu(&lignes(&r.join(SEPTEMBRE)), "6");
    assert_eq!(valeur(&b, "S21.G00.30.004"), Some("LUCIE"));
    assert_eq!(valeur(&b, "S21.G00.30.006"), Some("03101990"));
    assert_eq!(
        blocs_31(&b),
        vec![
            vec![paire("S21.G00.31.001", "15092026"), paire("S21.G00.31.009", "CLAIRON"), paire("S21.G00.31.011", "02101990")],
            vec![paire("S21.G00.31.001", "20092026"), paire("S21.G00.31.010", "LUCILE")],
        ]
    );
}

/// La cle d'un NIR : 97 - (NIR mod 97).
fn cle_nir(nir: &str) -> String {
    format!("{:02}", 97 - nir.parse::<u64>().unwrap() % 97)
}

#[test]
fn un_nir_corrige_ecrit_l_ancien_nir() {
    let r = copie("nir");
    // Jacquet (matricule 7) declaree sous 2870229999010 ; la caisse notifie le bon numero, 2870229999011.
    changer(&r, "JACQUET", &[("nir", "2870229999011", Some("2026-09-10")), ("nir_cle", &cle_nir("2870229999011"), None)]);
    mois_pret(&r, "2026-09");
    let rep = produire(&r, "2026-09", "2026-10-05");
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let b = individu(&lignes(&r.join(SEPTEMBRE)), "7");
    assert_eq!(valeur(&b, "S21.G00.30.001"), Some("2870229999011"));
    assert_eq!(valeur(&b, "S21.G00.30.020"), None, "un NIR qui remplace un NIR ne porte pas de NTT");
    assert_eq!(blocs_31(&b), vec![vec![paire("S21.G00.31.001", "10092026"), paire("S21.G00.31.008", "2870229999010")]]);
}

#[test]
fn un_changement_que_la_fiche_ne_date_pas_refuse_en_le_nommant() {
    let r = copie("sans-date");
    // Le nom de Biancolelli (matricule 4) est retouche dans le fichier, sans historique.
    changer(&r, "BIANCOLELLI", &[("nom", "BIANCO", None)]);
    mois_pret(&r, "2026-09");
    let rep = produire(&r, "2026-09", "2026-10-05");
    assert_eq!(rep["ok"], json!(false), "{}", rep);
    let e = rep["erreur"].as_str().unwrap_or_default().to_string() + &rep.to_string();
    for mot in ["nom de famille", "BIANCOLELLI", "BIANCO", "2026-07", "dater", "S21.G00.31"] {
        assert!(e.contains(mot), "le refus nomme {} : {}", mot, e);
    }
    assert!(!r.join(SEPTEMBRE).exists() || lignes(&r.join(SEPTEMBRE)).iter().all(|(_, v)| v != "BIANCO"), "rien n'est ecrit");
}

#[test]
fn un_intermittent_qui_revient_declare_le_changement_au_premier_contrat_suivant() {
    let r = copie("retour");
    // Chaminade (matricule 8) declaree en juillet, absente en aout et septembre ; son prenom est rectifie le 20 aout.
    changer(&r, "CHAMINADE", &[("prenom", "DORINA", Some("2026-08-20"))]);
    deposee(&r, "2026-09", "DSN_Septembre2026.dsn");
    mois_pret(&r, "2026-10");
    let rep = produire(&r, "2026-10", "2026-11-05");
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let b = individu(&lignes(&r.join(OCTOBRE)), "8");
    assert_eq!(valeur(&b, "S21.G00.30.004"), Some("DORINA"));
    assert_eq!(blocs_31(&b), vec![vec![paire("S21.G00.31.001", "20082026"), paire("S21.G00.31.010", "DORINE")]]);
}

#[test]
fn un_changement_date_apres_le_mois_declare_n_entre_pas_dans_sa_dsn() {
    let r = copie("apres");
    // Le nom de Clairon change le 2 octobre, et septembre se produit le 5 octobre : septembre la declare encore sous
    // CLAIRON, sans bloc 31 (31.001 ne peut pas depasser le dernier jour du mois principal declare, p. 99).
    changer(&r, "CLAIRON", &[("nom", "DUVAL", Some("2026-10-02"))]);
    mois_pret(&r, "2026-09");
    let rep = produire(&r, "2026-09", "2026-10-05");
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let toutes = lignes(&r.join(SEPTEMBRE));
    let b = individu(&toutes, "6");
    assert_eq!(valeur(&b, "S21.G00.30.002"), Some("CLAIRON"));
    assert!(!toutes.iter().any(|(k, _)| k.starts_with("S21.G00.31.")));
    // Octobre, lui, le declare.
    deposee(&r, "2026-09", "DSN_Septembre2026.dsn");
    mois_pret(&r, "2026-10");
    let rep = produire(&r, "2026-10", "2026-11-05");
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let b = individu(&lignes(&r.join(OCTOBRE)), "6");
    assert_eq!(valeur(&b, "S21.G00.30.002"), Some("DUVAL"));
    assert_eq!(blocs_31(&b), vec![vec![paire("S21.G00.31.001", "02102026"), paire("S21.G00.31.009", "CLAIRON")]]);
}

#[test]
fn le_bloc_31_n_est_admis_que_dans_la_dsn_mensuelle() {
    // Le tableau des usages (p. 363 et suivantes ; `usages-dsn-base.json`) : 31.001 obligatoire et 31.008-011
    // conditionnelles en nature 01, sans objet (N) dans tous les signalements. De meme dans le tableau 2027 (p. 371 et
    // suivantes ; `usages-dsn-p27v01-base.json`) : la table se lit par version de la norme depuis le lot V4a.
    for version in ["P26V01", "P27V01"] {
        let norme = paie::norme::Norme::connue(version).unwrap();
        for r in ["S21.G00.31.001", "S21.G00.31.008", "S21.G00.31.009", "S21.G00.31.010", "S21.G00.31.011"] {
            let u = paie::usages::usage_en(&norme, r, "01").unwrap();
            assert!(u == "O" || u == "C", "{} en mensuelle {} : {}", r, version, u);
            for n in ["04", "05", "07", "08", "10"] {
                assert_eq!(paie::usages::usage_en(&norme, r, n).unwrap(), "N", "{} en nature {} {}", r, n, version);
            }
        }
    }
}

#[test]
fn le_nir_qui_arrive_apres_un_mois_sans_contrat_porte_le_ntt_declare() {
    // Fiche N4, 30.020 : « Le NTT doit etre renseigne sur la premiere DSN ou le NIR est attribue » (p. 175). Chaminade
    // (matricule 8) a ete declaree en juillet sous son NTT ; son NIR arrive le 10 aout, elle ne retravaille qu'en
    // octobre. Octobre porte le NIR et, a cote, le NTT declare en juillet ; pas de bloc 31 (il n'y avait pas d'ancien
    // NIR).
    let r = copie("ntt");
    let juillet = r.join("sorties/2026-07/DSN_Juillet2026.dsn");
    let texte: String = std::fs::read(&juillet).unwrap().iter().map(|b| *b as char).collect();
    let ntt = "21234569158";
    assert!(texte.contains("S21.G00.30.001,'2850444999004'\r\n"));
    let texte = texte
        .replace("S21.G00.30.001,'2850444999004'\r\n", "")
        .replace("S21.G00.30.019,'8'\r\n", &format!("S21.G00.30.019,'8'\r\nS21.G00.30.020,'{}'\r\n", ntt));
    std::fs::write(&juillet, texte.chars().map(|c| c as u8).collect::<Vec<u8>>()).unwrap();
    let mut s = lire(&r, "config/salaries.json");
    s["CHAMINADE"]["historique"] = json!([{"champ": "nir", "valeur": "", "jusqu_au": "2026-08-09", "note": ""}]);
    ecrire_json(&r, "config/salaries.json", &s);
    mois_pret(&r, "2026-10");
    let rep = produire(&r, "2026-10", "2026-11-05");
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let b = individu(&lignes(&r.join(OCTOBRE)), "8");
    assert_eq!(valeur(&b, "S21.G00.30.001"), Some("2850444999004"));
    assert_eq!(valeur(&b, "S21.G00.30.020"), Some(ntt), "le NTT declare accompagne le NIR");
    assert!(blocs_31(&b).is_empty());
}
