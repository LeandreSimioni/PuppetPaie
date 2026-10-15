//! Chaque mois déposé des quatre démonstrations (3090 et 1285, intermittents et permanents, cadres compris), refait
//! aujourd'hui sur la copie figée, une fois : le fichier passe `usages` et `formats`, ne porte ni 105 ni bloc 41, déclare
//! 131 + 132 au montant du 105 déposé, et sa part patronale Agirc-Arrco en 142 et 146 à l'Urssaf (SIG-18).
//!
//! Le 105 n'est gardé que « pour régularisation des périodes antérieures à 2025 » (`atelier/reference/norme-dsn/texte/
//! bloc-S21.G00.81.txt`, p. 302, l. 79-81) : le bulletin n'a pas changé, l'Apec est seulement sortie du 105. Aucun contrat
//! n'y change de modalité (CDI de 2022 et CDDU à cheval compris) ; une valeur que la nature fixe (999SPT en 40.005, vague
//! V2b) ne fait pas de bloc 41 (`changements_contrat::blocs_41`).

use crate::agirc_arrco::{patronale_par_contrat, sig_18, versement_20_et_lignes, AUDIENS_RETRAITE};
use crate::outils;
use paie::contexte::Contexte;
use paie::date::Jour;
use paie::dsn::{generer, Options};
use paie::engagement::charger;
use paie::num::Decimal;
use std::collections::BTreeMap;

/// La somme des 81.004 des `codes`, contrat par contrat (78.006, ou le 40.009 d'un individu à contrat unique, qui
/// n'écrit pas 78.006 : cahier technique 2026.1 p. 298).
fn par_contrat(dsn: &str, codes: &[&str]) -> BTreeMap<String, Decimal> {
    let (mut contrat, mut contrat40, mut code, mut out) = (String::new(), String::new(), String::new(), BTreeMap::new());
    for (k, v) in outils::paires(dsn) {
        match k.as_str() {
            "S21.G00.40.009" => contrat40 = v,
            "S21.G00.78.001" => contrat = contrat40.clone(),
            "S21.G00.78.006" => contrat = v,
            "S21.G00.81.001" => code = v,
            "S21.G00.81.004" if codes.contains(&code.as_str()) => *out.entry(contrat.clone()).or_insert(Decimal::ZERO) += outils::dec(&v),
            _ => {}
        }
    }
    out
}

/// Rejoue chaque mois déposé de la compagnie fictive `nom` ; rend (mois rejoués, mois qui déclarent un 132, un 146).
fn rejouer(nom: &str) -> (usize, usize, usize) {
    let sorties = outils::fictive(nom).join("sorties");
    let mut mois: Vec<String> =
        std::fs::read_dir(&sorties).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|m| m.len() == 7 && m.starts_with("202")).collect();
    mois.sort();
    let (mut vus, mut avec_132, mut avec_146) = (0, 0, 0);
    for m in mois {
        let quoi = format!("{} {}", nom, m);
        let depose = std::fs::read_dir(sorties.join(&m))
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .find(|p| p.file_name().unwrap().to_string_lossy().starts_with("DSN_") && p.extension().is_some_and(|x| x == "dsn"))
            .unwrap_or_else(|| panic!("{} : pas de DSN deposee", quoi));
        let depose = String::from_utf8_lossy(&std::fs::read(depose).unwrap()).into_owned();
        let ctx = Contexte::nouveau(&outils::fictive(nom), Some(Jour::lire(&format!("{}-28", m)).unwrap())).unwrap();
        let s = charger(&ctx, &outils::fictive(nom).join(format!("paies/{}.json", m))).unwrap();
        let dsn = generer(&ctx, &s.paies, &m, &[], &[], &Options::default()).unwrap_or_else(|e| panic!("{} : {}", quoi, e));

        paie::usages::verifier_fichier(&dsn).unwrap_or_else(|e| panic!("{} : {}", quoi, e));
        paie::formats::verifier_fichier(&dsn).unwrap_or_else(|e| panic!("{} : {}", quoi, e));
        assert!(!dsn.contains("S21.G00.41."), "{} : {}", quoi, dsn.lines().filter(|l| l.starts_with("S21.G00.41.")).collect::<Vec<_>>().join(" "));
        assert!(!dsn.contains("S21.G00.81.001,'105'"), "{} : plus aucun 105", quoi);
        assert_eq!(par_contrat(&dsn, &["131", "132"]), par_contrat(&depose, &["105"]), "{} : 131 + 132, contrat par contrat, = le 105 depose", quoi);
        let (verse, lignes) = versement_20_et_lignes(&dsn, AUDIENS_RETRAITE, &["131", "132", "106"]);
        assert_eq!(verse, lignes, "{} : le versement Agirc-Arrco porte 131, 132 et 106", quoi);
        assert_eq!(par_contrat(&dsn, &["142", "146"]), patronale_par_contrat(&s.paies), "{} : 142 + 146 = la part patronale du bulletin", quoi);
        sig_18(&dsn, &quoi);
        vus += 1;
        avec_132 += usize::from(dsn.contains("S21.G00.81.001,'132'"));
        avec_146 += usize::from(dsn.contains("S21.G00.81.001,'146'"));
    }
    (vus, avec_132, avec_146)
}

#[test]
fn la_3090_grande_se_refait_mois_par_mois() {
    let (vus, avec_132, avec_146) = rejouer("fictive-3090-grande");
    assert!(vus > 20 && avec_132 > 0 && avec_146 > 0, "des cadres declarent l'Apec en 132, une tranche 2 sa part patronale en 146");
}

#[test]
fn la_3090_petite_se_refait_mois_par_mois() {
    assert!(rejouer("fictive-3090-petite").0 > 20);
}

#[test]
fn la_1285_grande_se_refait_mois_par_mois() {
    let (vus, avec_132, _) = rejouer("fictive-1285-grande");
    assert!(vus > 20 && avec_132 > 0, "une cadre declare l'Apec en 132");
}

#[test]
fn la_1285_petite_se_refait_mois_par_mois() {
    assert!(rejouer("fictive-1285-petite").0 > 20);
}
