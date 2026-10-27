//! Corrections DSN, vague 1f (point 20 de la relecture de la norme, fiche N3 des blocs 20 et 55) : les composants du
//! versement a un organisme complementaire disent a quel contrat ils servent.
//!
//! Norme DSN 2026.1, `atelier/reference/norme-dsn/texte/bloc-S21.G00.55.txt` : 55.001 « la part du versement affectee au
//! contrat Prevoyance reference en S21.G00.55.003 [...] pour la periode d'affectation » (p. 259, lignes 44-46) ; 55.003
//! « Le contrat mentionne doit faire l'objet d'un bloc Adhesion Prevoyance S21.G00.15 dans la declaration », CCH-11
//! (p. 260, lignes 72-79) ; 55.004 « une periode civile complete [...] contenant la ou les periodes cotisees » (p. 260,
//! lignes 85-87). `bloc-S21.G00.20.txt` : 20.005 d'un organisme complementaire = la somme des composants, ou zero si
//! elle est negative (p. 157, lignes 191-193, CCH-12 lignes 211-213).
//!
//! Avant : le bloc 55 etiquetait l'affectation 202 « TSINTERMITTENTS » et 203 « INTERBRANCHE » en dur, quelles que
//! soient la nature du contrat et l'annee ; les permanents partaient sous les contrats des intermittents, et en 2025 la
//! somme des 55.001 ne faisait pas le 20.005 (3090-grande, juin 2025 : 296,51 pour 195,23).
//!
//! Sur la copie figee des compagnies fictives (compagnies inventees) ; les essais qui changent config/organismes.json le font
//! sur une copie. La periode d'affectation et le code d'affectation se prouvent dans `src/dsn.rs`.

use crate::outils::{self, compagnie, dec, fictive, paires as lignes};
use socle::num::Decimal;
use serde_json::json;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn dsn_du_mois(racine: &Path, mois: &str) -> String {
    outils::dsn_du_mois(racine, mois, &format!("{}-28", mois)).unwrap()
}

/// Un versement 20 : son 20.005 et ses composants (55.003, 55.004, 55.001), dans l'ordre du fichier.
#[derive(Debug)]
struct Versement {
    montant: Decimal,
    composants: Vec<(String, String, Decimal)>,
}

fn versements(dsn: &str) -> BTreeMap<String, Versement> {
    let mut out: BTreeMap<String, Versement> = BTreeMap::new();
    let mut courant: Option<String> = None;
    let mut m55 = Decimal::ZERO;
    let mut c55 = String::new();
    for (k, v) in lignes(dsn) {
        match k.as_str() {
            "S21.G00.20.001" => {
                courant = Some(v.clone());
                out.insert(v, Versement { montant: Decimal::ZERO, composants: vec![] });
            }
            "S21.G00.20.005" => out.get_mut(courant.as_ref().unwrap()).unwrap().montant = dec(&v),
            "S21.G00.55.001" => m55 = dec(&v),
            "S21.G00.55.003" => c55 = v,
            "S21.G00.55.004" => out.get_mut(courant.as_ref().unwrap()).unwrap().composants.push((c55.clone(), v, m55)),
            k if !k.starts_with("S21.G00.20.") && !k.starts_with("S21.G00.55.") => courant = None,
            _ => {}
        }
    }
    out
}

/// Les adhesions du bloc 15 : 15.005 -> (15.001, 15.002).
fn adhesions_15(dsn: &str) -> BTreeMap<String, (String, String)> {
    let mut out = BTreeMap::new();
    let (mut lib, mut org) = (String::new(), String::new());
    for (k, v) in lignes(dsn) {
        match k.as_str() {
            "S21.G00.15.001" => lib = v,
            "S21.G00.15.002" => org = v,
            "S21.G00.15.005" => {
                out.insert(v, (lib.clone(), org.clone()));
            }
            _ => {}
        }
    }
    out
}

fn codes(v: &Versement) -> Vec<&str> {
    v.composants.iter().map(|(c, _, _)| c.as_str()).collect()
}

fn somme(v: &Versement) -> Decimal {
    v.composants.iter().map(|(_, _, m)| *m).sum()
}

#[test]
fn intermittents_et_permanents_chez_audiens_chacun_sous_son_contrat() {
    // 3090-grande, mars 2026 : des CDDU d'intermittents et des permanents. Le releve de la norme (fiche N3, 55.001 a) : 55
    // TSINTERMITTENTS = 92,26 (74,52 de prevoyance SVP des CDI dedans) et INTERBRANCHE = 225,00 (115,36 de sante des CDI).
    let dsn = dsn_du_mois(&fictive("fictive-3090-grande"), "2026-03");
    let v = &versements(&dsn)["P0983"];
    let adh = adhesions_15(&dsn);
    // Chaque code d'affectation est la reference d'un contrat declare en bloc 15 chez le meme organisme (CCH-11, p. 260).
    for (c, _, _) in &v.composants {
        assert!(adh.values().any(|(lib, org)| lib == c && org == "P0983"), "55.003 {} sans bloc 15 chez P0983", c);
    }
    assert_eq!(codes(v), ["PREVOYANCE SVP", "FM SPECT VIVANT PRIV", "CASCSVP", "TSINTERMITTENTS", "INTERBRANCHE"]);
    // La somme des composants fait le versement (CCH-12 du 20.005, p. 157).
    assert_eq!(somme(v), v.montant);
    assert_eq!(v.montant, dec("375.44"));
    // Chaque composant est ce que le nominatif rattache a ce contrat ; ceux des intermittents ne portent rien des permanents.
    let nom = outils::par_contrat_de_prevoyance(&lignes(&dsn));
    for (c, p, m) in &v.composants {
        assert_eq!(p, "2026T01");
        let attendu: Decimal = nom.get(c).copied().unwrap_or_default();
        assert_eq!(*m, attendu, "composant {}", c);
    }
    let m = |c: &str| v.composants.iter().find(|x| x.0 == c).unwrap().2;
    assert_eq!(m("PREVOYANCE SVP"), dec("74.52"), "la prevoyance SVP des permanents, plus dans TSINTERMITTENTS");
    assert_eq!(m("FM SPECT VIVANT PRIV"), dec("173.04"), "la sante des permanents, plus dans INTERBRANCHE");
}

#[test]
fn en_2025_ni_thalie_ni_composant_compte_deux_fois() {
    // 3090-grande, juin 2025 : avant, 72,75 + 101,28 + 101,28 + 21,20 = 296,51 pour un 20.005 de 195,23, sous
    // « TSINTERMITTENTS » (aucun bloc 15 : l'adhesion date de 2026) et « INTERBRANCHE » + « CASCSVP » pour le meme 203.
    let dsn = dsn_du_mois(&fictive("fictive-3090-grande"), "2025-06");
    let v = &versements(&dsn)["P0983"];
    assert!(!adhesions_15(&dsn).values().any(|(l, _)| l == "TSINTERMITTENTS"), "pas d'adhesion Thalie en 2025");
    assert_eq!(v.composants, vec![
        ("PREVOYANCE SVP".to_string(), "2025T02".to_string(), dec("72.75")),
        ("FM SPECT VIVANT PRIV".to_string(), "2025T02".to_string(), dec("101.28")),
        ("CASCSVP".to_string(), "2025T02".to_string(), dec("21.20")),
    ]);
    assert_eq!(somme(v), v.montant);
    assert_eq!(v.montant, dec("195.23"));
}

/// Les lignes 55 d'un fichier, telles quelles.
fn lignes_55(dsn: &str) -> Vec<(String, String)> {
    lignes(dsn).into_iter().filter(|(k, _)| k.starts_with("S21.G00.55.")).collect()
}

fn fige(nom: &str, mois: &str) -> String {
    let dossier = fictive(nom).join("sorties").join(mois);
    let f = std::fs::read_dir(&dossier).unwrap().flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|x| x == "dsn")).unwrap();
    String::from_utf8_lossy(&std::fs::read(f).unwrap()).into_owned()
}

#[test]
fn un_mois_d_intermittents_de_2026_ne_change_pas() {
    // Les adhesions 202 et 203 des intermittents sont en 2026 TSINTERMITTENTS et INTERBRANCHE : ce que les etiquettes en
    // dur disaient. La norme n'exige rien d'autre : le bloc 55 est celui que le moteur ecrivait (copie figee des sorties).
    for (nom, mois) in [("fictive-3090-petite", "2026-02"), ("fictive-3090-petite", "2026-08"), ("fictive-1285-petite", "2026-01")] {
        let dsn = dsn_du_mois(&fictive(nom), mois);
        assert_eq!(lignes_55(&dsn), lignes_55(&fige(nom, mois)), "{} {}", nom, mois);
        let v = &versements(&dsn)["P0983"];
        assert_eq!(somme(v), v.montant);
    }
}

#[test]
fn un_mois_d_intermittents_de_2025_nomme_les_contrats_de_2025() {
    // 2025, sans Thalie : l'affectation 202 des intermittents est l'interbranche, la 203 la CASC-SVP. La sortie figee les
    // nommait TSINTERMITTENTS et INTERBRANCHE, un contrat sans bloc 15 (CCH-11) et l'autre au montant d'un autre.
    let fige = fige("fictive-3090-petite", "2025-01");
    assert!(lignes_55(&fige).iter().any(|(_, v)| v == "TSINTERMITTENTS"), "le defaut, dans la sortie figee");
    let dsn = dsn_du_mois(&fictive("fictive-3090-petite"), "2025-01");
    let v = &versements(&dsn)["P0983"];
    assert_eq!(v.composants, vec![
        ("INTERBRANCHE".to_string(), "2025T01".to_string(), dec("19.61")),
        ("CASCSVP".to_string(), "2025T01".to_string(), dec("6.76")),
    ]);
    assert_eq!(somme(v), v.montant);
}

#[test]
fn la_1285_declare_le_fnas_et_le_fcap_sous_leurs_adhesions() {
    // 1285-grande : FNAS et FCAP (contributions de branche) sous leurs adhesions, chaque mois ou il y a un versement.
    let racine = fictive("fictive-1285-grande");
    let mut vus = 0;
    for mois in ["2025-03", "2025-10", "2026-03", "2026-09"] {
        let dsn = dsn_du_mois(&racine, mois);
        let adh = adhesions_15(&dsn);
        let Some(v) = versements(&dsn).remove("P0983") else { continue };
        vus += 1;
        assert_eq!(somme(&v), v.montant, "{}", mois);
        for (c, _, _) in &v.composants {
            assert!(adh.values().any(|(lib, org)| lib == c && org == "P0983"), "{} : 55.003 {} sans bloc 15", mois, c);
        }
        let nom = outils::par_contrat_de_prevoyance(&lignes(&dsn));
        for (c, _, m) in &v.composants {
            let attendu: Decimal = nom.get(c).copied().unwrap_or_default();
            assert_eq!(*m, attendu, "{} composant {}", mois, c);
        }
    }
    assert!(vus >= 2);
}

/// Une copie de `fictive-3090-grande` ou la prevoyance et la sante des permanents sont chez un autre organisme (P1234),
/// avec ou sans versement a cet organisme dans config/organismes.json.
fn deux_organismes(test: &str, avec_versement: bool) -> PathBuf {
    let racine = compagnie::ses_dsn("fictive-3090-grande", &[], &outils::dossier_de_test("composants_versement", test));
    let mut o = outils::lire(&racine, "config/organismes.json");
    for a in o["adhesions_prevoyance"].as_array_mut().unwrap() {
        if a["libelle"] == "PREVOYANCE SVP" || a["libelle"] == "FM SPECT VIVANT PRIV" {
            a["delegataire"] = json!("P1234");
        }
    }
    if avec_versement {
        o["versements"].as_array_mut().unwrap().push(json!({"siret": "P1234", "libelle": "Autre prevoyance", "codes81": ["059"]}));
    }
    outils::ecrire(&racine, "config/organismes.json", &o);
    racine
}

#[test]
fn deux_organismes_complementaires_chacun_ses_composants() {
    let racine = deux_organismes("deux", true);
    let dsn = dsn_du_mois(&racine, "2026-03");
    let vs = versements(&dsn);
    let (audiens, autre) = (&vs["P0983"], &vs["P1234"]);
    assert_eq!(codes(audiens), ["CASCSVP", "TSINTERMITTENTS", "INTERBRANCHE"]);
    assert_eq!(codes(autre), ["PREVOYANCE SVP", "FM SPECT VIVANT PRIV"]);
    assert_eq!(somme(audiens), audiens.montant);
    assert_eq!(somme(autre), autre.montant);
    assert_eq!(autre.montant, dec("247.56"), "74,52 + 173,04");
    assert_eq!(audiens.montant + autre.montant, dec("375.44"), "le total de mars, partage");
    let adh = adhesions_15(&dsn);
    assert!(adh.values().any(|(l, o)| l == "PREVOYANCE SVP" && o == "P1234"), "CCH-11 : le triplet 20.001 / 55.003 en 15");
}

#[test]
fn un_organisme_complementaire_sans_versement_refuse() {
    let e = outils::dsn_du_mois(&deux_organismes("sans-versement", false), "2026-03", "2026-03-28").unwrap_err();
    assert!(e.contains("P1234") && e.contains("n'a pas de versement a cet organisme"), "{}", e);
}
