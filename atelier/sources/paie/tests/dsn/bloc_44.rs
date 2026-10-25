//! Corrections DSN, lot V3b (point 11 de la relecture de la norme, fiche N8) : le bloc S21.G00.44 « Assujettissement
//! fiscal », que le moteur n'ecrivait jamais, se declare dans la DSN de decembre.
//!
//! Norme DSN 2026.1 (`atelier/reference/norme-dsn/texte/`) : § 2.3.1, p. 58 (`chapitre-2-principes-de-constitution.txt`) :
//! « L'assujettissement ou le non-assujettissement doit obligatoirement être déclaré [...] au plus tard, dans la DSN
//! mensuelle de décembre » ; le bloc, p. 240-241 (`bloc-S21.G00.44.txt`, suite en tete de `bloc-S21.G00.45.txt`) :
//! CCH-11 de 44.001 (montant obligatoire pour 005, 009, 015, 016), CCH-12 (montant interdit pour 002, 004, 006, 008,
//! 010, 014). D'ou vient chaque code : `src/assujettissement.rs` ; les textes : `atelier/reference/sources/
//! bloc-44-assujettissement/LISEZ-MOI.md`.
//!
//! La contribution supplementaire a l'apprentissage, dont decembre dit l'assujettissement (003/004), se declare en mars
//! de l'annee suivante (lot D2, `src/csa.rs`) : ses tests sont a la fin.
//!
//! Sur des copies des compagnies fictives (compagnies inventees) : la fiche, l'effectif et la surcouche du bareme y changent.

use crate::outils::{self, compagnie, dec, ecrire, lire, paires as lignes};
use engagement::calcul::Paie;
use droit::contexte::Contexte;
use socle::date::Jour;
use paie::dsn::{generer, Options};
use engagement::charger;
use socle::num::Decimal;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Une copie d'une compagnie fictive figee, sans ses pieces, effacee a la fin du test.
struct Copie(PathBuf);

impl Copie {
    fn de(nom: &str) -> Copie {
        Copie(compagnie::ses_dsn(nom, &[], &outils::chemin_unique(&format!("bloc-44-{}", nom))))
    }

    /// La fiche entreprise, retouchee.
    fn fiche(self, f: impl FnOnce(&mut Value)) -> Copie {
        let mut v = lire(&self.0, "config/entreprise.json");
        f(&mut v);
        ecrire(&self.0, "config/entreprise.json", &v);
        self
    }

    /// Les organismes (`config/organismes.json`), retouches.
    fn organismes(self, f: impl FnOnce(&mut Value)) -> Copie {
        let mut v = lire(&self.0, "config/organismes.json");
        f(&mut v);
        ecrire(&self.0, "config/organismes.json", &v);
        self
    }

    /// L'effectif (CRM 118) d'une annee.
    fn effectif_de(self, an: &str, n: f64) -> Copie {
        let mut v = lire(&self.0, "config/effectifs.json");
        v["annees"][an]["A01"]["valeur"] = json!(n);
        ecrire(&self.0, "config/effectifs.json", &v);
        self
    }

    /// Le meme effectif (CRM 118) pour toutes les annees connues.
    fn effectif(self, n: f64) -> Copie {
        let mut v = lire(&self.0, "config/effectifs.json");
        for an in v["annees"].as_object_mut().unwrap().values_mut() {
            an["A01"]["valeur"] = json!(n);
        }
        ecrire(&self.0, "config/effectifs.json", &v);
        self
    }

    /// Une surcouche 2025 qui suit les seuils de 11 et de 50 salaries (`seuils_effectif` du socle) : ce que la
    /// compagnie ecrit quand son effectif les franchit, sinon le bareme refuse de payer. Les valeurs sont celles du
    /// socle au-dela du seuil ; le test ne lit que ce que la DSN declare.
    fn au_dela_de_cinquante(self) -> Copie {
        let mut v = lire(&self.0, "config/baremes/2025.json");
        v["versement_mobilite"] = json!("aucun");
        v["rubriques_bulletin"] = json!([{"id": "autres_2", "taux_pat": 0.35}]);
        v["cotisations_dsn"] = json!([{"code81": "049", "taux": 0.35}]);
        v["par_statut"] = json!({"technicien": {
            "rubriques_bulletin": [{"id": "autres_2", "taux_pat": 0.5}],
            "cotisations_dsn": [{"code81": "049", "taux": 0.5}],
            "reduction_generale": {"t": [{"depuis": "2025-01-01", "valeur": 0.3234}, {"depuis": "2025-05-01", "valeur": 0.3233}]}}});
        v["par_nature"] = json!({"permanent": {
            "cotisations_dsn": [{"code81": "049", "taux": 0.5}, {"bloc78": "03", "code81": "128", "taux": 1.0}],
            "rubriques": {"autres_2": {"taux_pat": 0.5}},
            "rubriques_ajoutees": [{"id": "autres_4", "taux_pat": 1.0, "composants": [1.0]}]}});
        ecrire(&self.0, "config/baremes/2025.json", &v);
        // A vingt salaries, la DSN d'avril declare l'OETH de l'annee precedente (L. 5212-1) : sans objet ici.
        ecrire(&self.0, "config/oeth.json", &json!({"2024": {"assujetti": false, "pourquoi": "compagnie fictive : test du bloc 44"}}));
        // Le FNAL des artistes a cinquante salaries : CTP 316 au lieu du 334 (lot R2). Le fichier des organismes de la copie
        // figee a ete compose avant : il prend les deux lignes du fichier livre, comme une recomposition les ecrirait.
        let base = lire(&outils::code_livre(), "organismes-base.json");
        let livre = |code: &str| base["ctp_urssaf"].as_array().unwrap().iter().find(|c| c["code"] == json!(code)).unwrap().clone();
        self.organismes(|o| {
            let ctps = o["ctp_urssaf"].as_array_mut().unwrap();
            let i = ctps.iter().position(|c| c["code"] == json!("334")).unwrap();
            ctps[i] = livre("334");
            ctps.insert(i + 1, livre("316"));
        })
    }

    fn contexte(&self, mois: &str) -> Contexte {
        Contexte::nouveau(&self.0, Some(Jour::lire(&format!("{}-28", mois)).unwrap())).unwrap()
    }

    fn paies(&self, mois: &str) -> Vec<Paie> {
        charger(&self.contexte(mois), &self.0.join(format!("paies/{}.json", mois))).unwrap().paies
    }

    /// La DSN du mois ; celle de decembre passe aussi les deux controles d'avant depot (regle 7 : `usages`, que la
    /// rubrique a le droit d'etre la ; `formats`, que sa valeur a la bonne forme).
    fn dsn(&self, mois: &str) -> Result<String, String> {
        let dsn = generer(&self.contexte(mois), &self.paies(mois), mois, &[], &[], &Options::default())?;
        if dsn.contains("S21.G00.44") {
            paie::usages::verifier_fichier(&dsn).unwrap();
            paie::formats::verifier_fichier(&dsn).unwrap();
        }
        Ok(dsn)
    }
}

impl Drop for Copie {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// La fiche d'une compagnie qui doit la taxe sur `pct` % de ses salaires (`Some`), ou qui ne la doit pas (`None`), dite
/// comme la compagnie la dit : sa TVA (ticket #216 ; CGI art. 231, 1). 100 % : exoneree de TVA ; moins : assujettie sur
/// le reste de ses recettes ; non due : assujettie sur toutes. Chiffre d'affaires au-dessus des plafonds de 293 B.
fn taxe_due_sur(pct: Option<f64>) -> impl FnOnce(&mut Value) {
    move |e: &mut Value| {
        e["taxe_sur_les_salaires"] = match pct {
            Some(p) if p >= 100.0 => json!({"tva": "exoneree", "sous_franchise": false}),
            Some(p) => json!({"tva": "assujettie", "part_tva": 100.0 - p, "sous_franchise": false}),
            None => json!({"tva": "assujettie", "part_tva": 100}),
        };
    }
}


/// Les blocs 44 du fichier : (44.001, 44.002, 44.003), dans l'ordre.
fn blocs_44(dsn: &str) -> Vec<(String, Option<String>, String)> {
    let mut out: Vec<(String, Option<String>, String)> = Vec::new();
    for (k, v) in lignes(dsn) {
        match k.as_str() {
            "S21.G00.44.001" => out.push((v, None, String::new())),
            "S21.G00.44.002" => out.last_mut().unwrap().1 = Some(v),
            "S21.G00.44.003" => out.last_mut().unwrap().2 = v,
            // Le motif d'un 002 (lot R2) : lu par `motif_44`.
            "S21.G00.44.004" => {}
            k if k.starts_with("S21.G00.44.") => panic!("rubrique inattendue {}", k),
            _ => {}
        }
    }
    out
}

/// Le motif de non-assujettissement (44.004) du bloc `code`, qui suit son millesime (44.003).
fn motif_44(dsn: &str, code: &str) -> Option<String> {
    let l = lignes(dsn);
    let i = l.iter().position(|(k, v)| k == "S21.G00.44.001" && v == code)?;
    let suite: Vec<&(String, String)> = l[i + 1..].iter().take_while(|(k, _)| k != "S21.G00.44.001" && k.starts_with("S21.G00.44.")).collect();
    let rubriques: Vec<&str> = suite.iter().map(|(k, _)| k.as_str()).collect();
    let j = rubriques.iter().position(|k| *k == "S21.G00.44.004")?;
    assert_eq!(rubriques[j - 1], "S21.G00.44.003", "44.004 suit 44.003");
    Some(suite[j].1.clone())
}

fn codes(dsn: &str) -> Vec<String> {
    blocs_44(dsn).into_iter().map(|b| b.0).collect()
}

fn montant(dsn: &str, code: &str) -> Option<Decimal> {
    blocs_44(dsn).into_iter().find(|b| b.0 == code).and_then(|b| b.1).map(|m| dec(&m))
}

/// La taxe sur les salaires de 2025 recomptee ici, mois par mois, salarie par salarie : l'assiette de la CSG sans sa
/// reduction de 1,75 % (CGI art. 231, 1), les fractions au-dela de 9 147 et de 18 259 EUR (2 bis ; seuils 2025 de
/// `donnees/baremes/2025.json`, notice 2501-SD), puis la part assujettie appliquee a chacune (BOFiP BOI-TPS-TS-30,
/// § 240).
fn taxe_sur_les_salaires_2025(c: &Copie, pct: Decimal) -> (Decimal, Decimal, Decimal) {
    let mut par: BTreeMap<String, Decimal> = BTreeMap::new();
    for m in 1..=12 {
        let mois = format!("2025-{:02}", m);
        if !c.0.join(format!("paies/{}.json", mois)).exists() {
            continue;
        }
        for p in c.paies(&mois).iter().filter(|p| !p.engagement.lignes.is_empty()) {
            let csg = p.bases.get("csg").copied().unwrap_or(Decimal::ZERO);
            let abattue = (p.brut * dec("0.9825")).round_dp_with_strategy(2, rust_decimal::RoundingStrategy::MidpointAwayFromZero);
            *par.entry(p.salarie["_cle"].as_str().unwrap().to_string()).or_default() += csg - abattue + p.brut;
        }
    }
    let (s1, s2) = (dec("9147"), dec("18259"));
    let (mut n, mut f1, mut f2) = (Decimal::ZERO, Decimal::ZERO, Decimal::ZERO);
    for b in par.values() {
        n += *b;
        f1 += ((*b).min(s2) - s1).max(Decimal::ZERO);
        f2 += (*b - s2).max(Decimal::ZERO);
    }
    let part = |x: Decimal| (x * pct / dec("100")).round_dp_with_strategy(2, rust_decimal::RoundingStrategy::MidpointAwayFromZero);
    (part(n), part(f1), part(f2))
}

#[test]
fn la_dsn_de_decembre_declare_chaque_taxe_avec_ses_assiettes() {
    // 3090-grande, 2025 : des CDI, un CDD de droit commun (motif 02), des intermittents ; cinq salaries ; la fiche dit
    // la compagnie redevable de la taxe sur les salaires sur toutes ses remunerations.
    let c = Copie::de("fictive-3090-grande").fiche(taxe_due_sur(Some(100.0)));
    let dsn = c.dsn("2025-12").unwrap();
    let (normal, premier, second) = taxe_sur_les_salaires_2025(&c, dec("100"));
    assert!(premier > Decimal::ZERO && second > Decimal::ZERO, "les permanents depassent les deux seuils : {} {}", premier, second);
    assert_eq!(
        blocs_44(&dsn),
        vec![
            // le bordereau paie la taxe d'apprentissage (CTP 992)
            ("001".to_string(), None, "2025".to_string()),
            // moins de 250 salaries (L. 6242-1)
            ("004".into(), None, "2025".into()),
            // moins de 50 salaries (CCH L. 313-1)
            ("006".into(), None, "2025".into()),
            // le bordereau paie la formation (CTP 983, 959)
            ("007".into(), None, "2025".into()),
            ("009".into(), Some(format!("{:.2}", normal)), "2025".into()),
            // le CDD d'Audinot, accroissement d'activite (L. 6331-6)
            ("013".into(), None, "2025".into()),
            ("015".into(), Some(format!("{:.2}", premier)), "2025".into()),
            ("016".into(), Some(format!("{:.2}", second)), "2025".into()),
        ]
    );
    // A sa place : apres le dernier bordereau (23), avant le premier individu (30).
    let l: Vec<String> = lignes(&dsn).into_iter().map(|(k, _)| k).collect();
    let premier_44 = l.iter().position(|k| k == "S21.G00.44.001").unwrap();
    assert!(l[premier_44 - 1].starts_with("S21.G00.23."), "{}", l[premier_44 - 1]);
    let dernier_44 = l.iter().rposition(|k| k.starts_with("S21.G00.44.")).unwrap();
    assert_eq!(l[dernier_44 + 1], "S21.G00.30.001");
}

#[test]
fn une_part_assujettie_se_porte_sur_chaque_assiette() {
    // Une compagnie assujettie a la TVA sur 40 % de son chiffre d'affaires : 60 % de chaque assiette (CGI art. 231, 1).
    let c = Copie::de("fictive-3090-grande").fiche(taxe_due_sur(Some(60.0)));
    let dsn = c.dsn("2025-12").unwrap();
    let (normal, premier, second) = taxe_sur_les_salaires_2025(&c, dec("60"));
    assert_eq!((montant(&dsn, "009"), montant(&dsn, "015"), montant(&dsn, "016")), (Some(normal), Some(premier), Some(second)));
}

#[test]
fn une_compagnie_qui_ne_doit_pas_la_taxe_et_n_a_que_des_intermittents() {
    // 3090-petite : trois intermittents, aucun CDD de droit commun ; la fiche dit la compagnie assujettie a la TVA.
    // Aucun montant : CCH-12 les interdit aux codes de non-assujettissement, et 001 et 007 n'en demandent pas.
    let c = Copie::de("fictive-3090-petite").fiche(taxe_due_sur(None));
    let dsn = c.dsn("2025-12").unwrap();
    assert_eq!(codes(&dsn), ["001", "004", "006", "007", "010", "014"]);
    assert!(blocs_44(&dsn).iter().all(|b| b.1.is_none() && b.2 == "2025"));
}

#[test]
fn un_autre_mois_n_a_pas_de_bloc_44() {
    // « une fois par an [...] au plus tard, dans la DSN mensuelle de décembre » (p. 58) : le moteur ne connait pas la
    // cessation d'activite, seul autre cas.
    let c = Copie::de("fictive-3090-grande").fiche(taxe_due_sur(Some(100.0)));
    for mois in ["2025-11", "2026-01", "2026-06"] {
        assert!(!c.dsn(mois).unwrap().contains("S21.G00.44"), "{}", mois);
    }
}

#[test]
fn une_fiche_muette_sur_la_taxe_sur_les_salaires_fait_refuser_decembre() {
    let c = Copie::de("fictive-3090-petite").fiche(|e| {
        e.as_object_mut().unwrap().remove("taxe_sur_les_salaires");
    });
    assert!(c.dsn("2025-12").is_err());
    // Novembre ne la demande pas.
    assert!(c.dsn("2025-11").is_ok());
    // Une reponse qui n'en est pas une, ou qui s'arrete avant la conclusion, ne vaut pas reponse : la TVA sans sa part,
    // une taxe qui serait due sans savoir si le chiffre d'affaires passe les plafonds de 293 B (CGI art. 231, 1).
    for incomplete in [
        json!({"tva": "oui"}),
        json!({"tva": "assujettie"}),
        json!({"tva": "assujettie", "part_tva": 140}),
        json!({"tva": "exoneree"}),
        json!({"tva": "assujettie", "part_tva": 50}),
    ] {
        let c = Copie::de("fictive-3090-petite").fiche(|e| e["taxe_sur_les_salaires"] = incomplete.clone());
        assert!(c.dsn("2025-12").is_err(), "{}", incomplete);
    }
}

#[test]
fn la_tva_de_la_compagnie_dit_si_la_taxe_sur_les_salaires_est_due() {
    // CGI art. 231, 1 (atelier/reference/sources/code-general-des-impots/xml/231.xml) : due par les employeurs qui ne
    // sont pas assujettis a la TVA « ou ne l'ont pas ete sur 90 % au moins de leur chiffre d'affaires » de l'annee
    // precedente ; exoneree sous les plafonds de l'article 293 B (2e alinea).
    let code_ts = |fiche: Value| {
        let c = Copie::de("fictive-3090-petite").fiche(move |e| e["taxe_sur_les_salaires"] = fiche);
        codes(&c.dsn("2025-12").unwrap()).into_iter().find(|k| k == "009" || k == "010").unwrap()
    };
    // Assujettie sur 90 % ou plus : non due (010), sans qu'on demande rien d'autre.
    assert_eq!(code_ts(json!({"tva": "assujettie", "part_tva": 90})), "010");
    assert_eq!(code_ts(json!({"tva": "assujettie", "part_tva": 100})), "010");
    // Sous 90 % : due (009) sur le reste.
    assert_eq!(code_ts(json!({"tva": "assujettie", "part_tva": 89.5, "sous_franchise": false})), "009");
    // Exoneree de TVA : due sur tout...
    assert_eq!(code_ts(json!({"tva": "exoneree", "sous_franchise": false})), "009");
    // ... sauf sous les plafonds de la franchise en base.
    assert_eq!(code_ts(json!({"tva": "exoneree", "sous_franchise": true})), "010");
    assert_eq!(code_ts(json!({"tva": "assujettie", "part_tva": 20, "sous_franchise": true})), "010");
}

#[test]
fn a_cinquante_salaries_la_peec_porte_l_assiette_de_l_annee() {
    // Soixante salaries depuis cinq ans (CRM 118) : la PEEC est due (CCH L. 313-1, effectif de L. 130-1 CSS), et son
    // montant est l'assiette brute deplafonnee (base 03) que les douze DSN de l'annee ont declaree, contrat par contrat.
    let c = Copie::de("fictive-3090-petite").fiche(taxe_due_sur(None)).effectif(60.0).au_dela_de_cinquante();
    let mut bases_03 = Decimal::ZERO;
    for m in 1..=11 {
        let mois = format!("2025-{:02}", m);
        if !c.0.join(format!("paies/{}.json", mois)).exists() {
            continue;
        }
        let dsn = c.dsn(&mois).unwrap();
        let l = lignes(&dsn);
        for (i, (k, v)) in l.iter().enumerate() {
            if k == "S21.G00.78.001" && v == "03" {
                assert_eq!(l[i + 3].0, "S21.G00.78.004");
                bases_03 += dec(&l[i + 3].1);
            }
        }
    }
    let dsn = c.dsn("2025-12").unwrap();
    let l = lignes(&dsn);
    for (i, (k, v)) in l.iter().enumerate() {
        if k == "S21.G00.78.001" && v == "03" {
            bases_03 += dec(&l[i + 3].1);
        }
    }
    assert!(bases_03 > Decimal::ZERO);
    assert_eq!(codes(&dsn), ["001", "004", "005", "007", "010", "014"]);
    assert_eq!(montant(&dsn, "005"), Some(bases_03), "CCH-11 : le montant du 005");
}

#[test]
fn a_deux_cent_cinquante_salaries_la_contribution_supplementaire_se_dit() {
    // L. 6242-1 : due a 250 salaries si moins de 5 % de l'effectif est en alternance, ce que le moteur ne compte pas.
    let c = Copie::de("fictive-3090-petite").fiche(taxe_due_sur(None)).effectif(300.0).au_dela_de_cinquante();
    let e = c.dsn("2025-12").unwrap_err();
    assert!(e.contains("contribution_supplementaire_apprentissage") && e.contains("250"), "{}", e);
    let c = c.fiche(|e| e["contribution_supplementaire_apprentissage"] = json!("due"));
    assert_eq!(codes(&c.dsn("2025-12").unwrap())[..3], ["001", "003", "005"]);
    let c = c.fiche(|e| e["contribution_supplementaire_apprentissage"] = json!("non_due"));
    assert_eq!(codes(&c.dsn("2025-12").unwrap())[..3], ["001", "004", "005"]);
}

#[test]
fn une_dsn_de_decembre_sans_salarie_porte_aussi_le_bloc_44() {
    // CoolPaie l'ecrivait dans la DSN « neant » de decembre 2020 : la declaration est due, salaries ou non.
    let c = Copie::de("fictive-3090-petite").fiche(taxe_due_sur(Some(100.0)));
    let ctx = c.contexte("2025-12");
    let dsn = generer(&ctx, &[], "2025-12", &[], &[], &Options::default()).unwrap();
    assert!(dsn.contains("S10.G00.00.008,'02'"), "une DSN sans individu");
    // L'annee de ses trois intermittents compte quand meme : la taxe porte sur ce qu'ils ont recu de janvier a novembre.
    assert_eq!(codes(&dsn), ["001", "004", "006", "007", "009", "014"]);
    assert!(montant(&dsn, "009").unwrap() > Decimal::ZERO);
}

#[test]
fn la_taxe_d_apprentissage_est_ce_que_le_bordereau_paie() {
    // Un etablissement d'Alsace-Moselle paie la taxe d'apprentissage sous le CTP 993 (0,44 %, code du travail
    // L. 6241-1-1, II) : il y est assujetti comme les autres.
    let c = Copie::de("fictive-3090-petite").fiche(taxe_due_sur(None)).organismes(|o| {
        for l in o["ctp_urssaf"].as_array_mut().unwrap() {
            if l["code"] == "992" {
                l["code"] = json!("993");
            }
        }
    });
    assert_eq!(codes(&c.dsn("2025-12").unwrap())[0], "001");
    // Un bordereau qui ne la paie pas declare 002, qui exige son motif (S21.G00.44.004, table MNATA) : la fiche ne le dit
    // pas, refus, en le disant.
    let c = c.organismes(|o| o["ctp_urssaf"].as_array_mut().unwrap().retain(|l| l["code"] != "993"));
    let e = c.dsn("2025-12").unwrap_err();
    assert!(e.contains("MNATA") && e.contains("992") && e.contains("taxe_apprentissage_motif_non_assujettissement") && e.contains("Rien n'est ecrit"), "{}", e);
}

/// Le motif de non-assujettissement a la taxe d'apprentissage de la fiche.
fn motif_ta(m: &'static str) -> impl FnOnce(&mut Value) {
    move |e: &mut Value| {
        e["taxe_sur_les_salaires"] = json!({"tva": "assujettie", "part_tva": 100});
        e["taxe_apprentissage_motif_non_assujettissement"] = json!(m);
    }
}

#[test]
fn une_association_non_redevable_en_2025_declare_002_et_son_motif_mnata() {
    // Lot R2 (Q8). La table MNATA est livree (`logiciel/donnees/mnata-base.json`, depuis la nomenclature NEODeS deposee,
    // atelier/reference/sources/dsn-nomenclatures/table-MNATA-20260924.txt) ; 44.004 est « obligatoire si [...] 002 »,
    // « CRE-11 : valeurs autorisees », « Table MNATA » (cahier technique DSN 2026.1, p. 241, bloc-S21.G00.45.txt, l. 42-52).
    // Une association a activite non lucrative ne devait pas la taxe d'apprentissage avant le 21/02/2026 (C. trav.
    // L. 6241-1 dans sa redaction du 16/02/2025, deposee) : son bordereau ne la paie pas, et decembre 2025 declare 002
    // avec le motif 007 « Associations sans but lucratif ».
    let sans_ta = |o: &mut Value| o["ctp_urssaf"].as_array_mut().unwrap().retain(|l| l["code"] != "992" && l["code"] != "993");
    let c = Copie::de("fictive-3090-petite").fiche(motif_ta("007")).organismes(sans_ta);
    let dsn = c.dsn("2025-12").unwrap();
    assert_eq!(codes(&dsn)[0], "002");
    assert_eq!(motif_44(&dsn, "002").as_deref(), Some("007"));
    assert!(motif_44(&dsn, "004").is_none(), "44.004 interdit hors 002 (CCH-11)");
    assert_eq!(montant(&dsn, "002"), None, "pas de montant pour 002 (CCH-12)");

    // Un motif hors de la table refuse (le bordereau du mois ne sait pas si la taxe est due ; R12) : des le bulletin, que
    // la meme decision du mois commande depuis le lot D1.
    let c = Copie::de("fictive-3090-petite").fiche(motif_ta("999")).organismes(sans_ta);
    let e = charger(&c.contexte("2025-12"), &c.0.join("paies/2025-12.json")).map(|_| ()).unwrap_err();
    assert!(e.contains("« 999 »") && e.contains("MNATA"), "{}", e);

    // Depuis le 21/02/2026, L. 6241-1 rend redevables « les associations [...] a activites non lucratives » : le motif
    // 007 ne vaut plus pour le millesime 2026.
    let c = Copie::de("fictive-3090-petite").fiche(motif_ta("007")).organismes(sans_ta);
    let ctx = c.contexte("2026-12");
    let e = generer(&ctx, &[], "2026-12", &[], &[], &Options::default()).unwrap_err();
    assert!(e.contains("L. 6241-1") && e.contains("21/02/2026") && e.contains("007"), "{}", e);
    // Un autre motif (006, l'enseignement : L. 6241-1, III, 1°) reste admis.
    let c = Copie::de("fictive-3090-petite").fiche(motif_ta("006")).organismes(sans_ta);
    let ctx = c.contexte("2026-12");
    let dsn = generer(&ctx, &[], "2026-12", &[], &[], &Options::default()).unwrap();
    assert_eq!(motif_44(&dsn, "002").as_deref(), Some("006"));
}

/// Les CTP du bordereau Urssaf (S21.G00.23.001), dans l'ordre.
fn ctp(dsn: &str) -> Vec<String> {
    lignes(dsn).into_iter().filter(|(k, _)| k == "S21.G00.23.001").map(|(_, v)| v).collect()
}

/// R12 (la question de R2 pour Leandre) : un motif dit que la taxe d'apprentissage n'est pas due, le moteur decide donc
/// des deux -- le bordereau ne la porte plus, et decembre declare 002 avec le motif. Le fichier des organismes, compose
/// comme l'ecran le compose (avec le 992), n'a pas a etre retouche a la main. Le 007 ne vaut que jusqu'au millesime 2025
/// (C. trav. L. 6241-1 depuis le 21/02/2026 ; INFERENCE de R2, `assujettissement::MILLESIME_ASSOCIATIONS_REDEVABLES`) :
/// en 2026 l'association la paie, et avril 2026 ne solde pas la taxe de 2025, qu'elle ne devait pas.
#[test]
fn le_motif_de_la_fiche_retire_la_taxe_d_apprentissage_du_bordereau() {
    let c = Copie::de("fictive-3090-petite").fiche(motif_ta("007"));
    let compose = lire(&c.0, "config/organismes.json");
    assert!(compose["ctp_urssaf"].as_array().unwrap().iter().any(|l| l["code"] == "992"), "le fichier compose porte le 992");
    for mois in ["2025-11", "2025-12"] {
        let dsn = c.dsn(mois).unwrap_or_else(|e| panic!("{} : {}", mois, e));
        assert!(!ctp(&dsn).iter().any(|x| x == "992" || x == "993"), "{} : pas de taxe d'apprentissage au bordereau {:?}", mois, ctp(&dsn));
    }
    let dsn = c.dsn("2025-12").unwrap();
    assert_eq!(codes(&dsn)[0], "002");
    assert_eq!(motif_44(&dsn, "002").as_deref(), Some("007"));
    // 2026 : l'association la doit, le bordereau la porte ; avril ne solde pas 2025 (ni CTP 995, ni bloc 82 « 076 »).
    let avril = c.dsn("2026-04").unwrap();
    assert!(ctp(&avril).iter().any(|x| x == "992"), "{:?}", ctp(&avril));
    assert!(!ctp(&avril).iter().any(|x| x == "995") && !avril.contains("S21.G00.82.002,'076'"), "{:?}", ctp(&avril));
    // Sans motif, le meme avril solde 2025 : c'est bien le motif qui l'a retire.
    let c = Copie::de("fictive-3090-petite").fiche(taxe_due_sur(None));
    let avril = c.dsn("2026-04").unwrap();
    assert!(ctp(&avril).iter().any(|x| x == "995") && avril.contains("S21.G00.82.002,'076'"), "{:?}", ctp(&avril));
    // Un motif qui vaut en 2026 (006, l'enseignement) retire aussi la taxe de 2026.
    let c = Copie::de("fictive-3090-petite").fiche(motif_ta("006"));
    assert!(!ctp(&c.dsn("2026-04").unwrap()).iter().any(|x| x == "992" || x == "995"));
}

/// Lot D1 : la taxe que le motif dit non due ne se calcule nulle part. Le bulletin (la ligne marquee
/// `"taxe_apprentissage"` du bareme, part principale et solde), le nominatif (81.001 = 130) et le bordereau (992, 993)
/// lisent la meme decision du mois (`assujettissement::taxe_apprentissage_non_due_au_mois`) ; R12 ne retirait que le
/// bordereau. Le 007 vaut jusqu'a la periode d'emploi de fevrier 2026 : « À compter de la période d'emploi de mars 2026
/// [...] la taxe d'apprentissage est également due par les associations [...] à activités non lucratives » (guide Urssaf
/// des contributions de formation et d'apprentissage du 12/03/2026, l. 1166-1176 et 1192-1196).
#[test]
fn la_taxe_non_due_ne_se_calcule_ni_au_bulletin_ni_au_nominatif_ni_au_bordereau() {
    let ta_au_bulletin = |c: &Copie, mois: &str| c.paies(mois).iter().any(|p| p.lignes.iter().any(|l| l.id == "autres_3"));
    let porte_130 = |dsn: &str| lignes(dsn).iter().any(|(k, v)| k == "S21.G00.81.001" && v == "130");
    let porte_992 = |dsn: &str| ctp(dsn).iter().any(|x| x == "992" || x == "993");
    // Sans motif : les trois faces la portent.
    let c = Copie::de("fictive-3090-petite").fiche(taxe_due_sur(None));
    let dsn = c.dsn("2025-11").unwrap();
    assert!(ta_au_bulletin(&c, "2025-11") && porte_130(&dsn) && porte_992(&dsn));
    // Un motif qui vaut toute l'annee (006, l'enseignement) : aucune des trois.
    let c = Copie::de("fictive-3090-petite").fiche(motif_ta("006"));
    for mois in ["2025-11", "2026-03"] {
        let dsn = c.dsn(mois).unwrap();
        assert!(!ta_au_bulletin(&c, mois), "{} : pas de taxe d'apprentissage au bulletin", mois);
        assert!(!porte_130(&dsn), "{} : pas de cotisation individuelle 130", mois);
        assert!(!porte_992(&dsn), "{} : pas de CTP 992 ni 993", mois);
        paie::usages::verifier_fichier(&dsn).unwrap();
        paie::formats::verifier_fichier(&dsn).unwrap();
    }
    // Une association (007) : rien jusqu'a fevrier 2026, les trois faces a partir de mars 2026.
    let c = Copie::de("fictive-3090-petite").fiche(motif_ta("007"));
    let fevrier = c.dsn("2026-02").unwrap();
    assert!(!ta_au_bulletin(&c, "2026-02") && !porte_130(&fevrier) && !porte_992(&fevrier), "fevrier 2026 : {:?}", ctp(&fevrier));
    let mars = c.dsn("2026-03").unwrap();
    assert!(ta_au_bulletin(&c, "2026-03") && porte_130(&mars) && porte_992(&mars), "mars 2026 : {:?}", ctp(&mars));
}

#[test]
fn l_effectif_de_la_peec_est_celui_de_l_annee_du_millesime() {
    // Soixante salaries de 2020 a 2024, trente en 2025 : la PEEC declaree en decembre 2025 porte sur les salaires de
    // 2025, et c'est l'effectif de 2025 qui la declenche (CCH L. 313-1, L. 130-1 CSS) -- 006.
    let c = Copie::de("fictive-3090-petite").fiche(taxe_due_sur(None)).effectif(60.0).effectif_de("2025", 30.0).au_dela_de_cinquante();
    assert_eq!(codes(&c.dsn("2025-12").unwrap()), ["001", "004", "006", "007", "010", "014"]);
}

// ------------------------------------------------------------ la contribution supplementaire de mars (lot D2)

impl Copie {
    /// La surcouche 2026 qui suit les seuils de 11 et de 50 salaries, comme `au_dela_de_cinquante` pour 2025 (le socle
    /// 2026 demande en plus le `tdelta` de la reduction generale a partir de cinquante).
    fn au_dela_de_cinquante_en_2026(self) -> Copie {
        let mut v = lire(&self.0, "config/baremes/2026.json");
        v["versement_mobilite"] = json!("aucun");
        v["rubriques_bulletin"] = json!([{"id": "autres_2", "taux_pat": 0.35}]);
        v["cotisations_dsn"] = json!([{"code81": "049", "taux": 0.35}]);
        v["par_statut"] = json!({"technicien": {
            "rubriques_bulletin": [{"id": "autres_2", "taux_pat": 0.5}],
            "cotisations_dsn": [{"code81": "049", "taux": 0.5}],
            "reduction_generale": {"tdelta": 0.3821}}});
        v["par_nature"] = json!({"permanent": {
            "cotisations_dsn": [{"code81": "049", "taux": 0.5}, {"bloc78": "03", "code81": "128", "taux": 1.0}],
            "rubriques": {"autres_2": {"taux_pat": 0.5}},
            "rubriques_ajoutees": [{"id": "autres_4", "taux_pat": 1.0, "composants": [1.0]}]}});
        ecrire(&self.0, "config/baremes/2026.json", &v);
        self
    }

    /// Les effectifs de la contribution supplementaire d'une annee, tels que l'Urssaf les renvoie en CRM 118 (fiche
    /// net-entreprises 2624) : A05, les alternants ; A07, le denominateur du taux d'alternants.
    fn alternants(self, an: &str, a05: f64, a07: f64) -> Copie {
        let mut v = lire(&self.0, "config/effectifs.json");
        v["annees"][an]["A05"]["valeur"] = json!(a05);
        v["annees"][an]["A07"]["valeur"] = json!(a07);
        ecrire(&self.0, "config/effectifs.json", &v);
        self
    }

    /// Trois cents salaries depuis cinq ans, assujettie a la taxe d'apprentissage : la contribution supplementaire de
    /// 2025 se declare en mars 2026.
    fn trois_cents() -> Copie {
        Copie::de("fictive-3090-petite")
            .fiche(taxe_due_sur(None))
            .fiche(|e| e["contribution_supplementaire_apprentissage"] = json!("due"))
            .effectif(300.0)
            .au_dela_de_cinquante()
            .au_dela_de_cinquante_en_2026()
    }

    /// L'assiette de la taxe d'apprentissage que les DSN deposees de 2025 ont declaree (CTP 992, 23.004), relue ici.
    fn assiette_ta_2025(&self) -> Decimal {
        let ctx = self.contexte("2026-03");
        let mut total = Decimal::ZERO;
        for m in 1..=12 {
            let Some(f) = paie::casc::dsn_deposee(&ctx, &format!("2025-{:02}", m), None).unwrap() else { continue };
            let texte: String = std::fs::read(&f).unwrap().iter().map(|&b| b as char).collect();
            let l = lignes(&texte);
            for (i, (k, v)) in l.iter().enumerate() {
                if k == "S21.G00.23.001" && v == "992" {
                    let a = l[i + 1..].iter().take_while(|(k, _)| k != "S21.G00.23.001").find(|(k, _)| k == "S21.G00.23.004").unwrap();
                    total += dec(&a.1);
                }
            }
        }
        total
    }
}

/// Les blocs S21.G00.82 du fichier : (82.001, 82.002, 82.003, 82.004).
fn blocs_82(dsn: &str) -> Vec<(String, String, String, String)> {
    let mut out: Vec<(String, String, String, String)> = Vec::new();
    for (k, v) in lignes(dsn) {
        match k.as_str() {
            "S21.G00.82.001" => out.push((v, String::new(), String::new(), String::new())),
            "S21.G00.82.002" => out.last_mut().unwrap().1 = v,
            "S21.G00.82.003" => out.last_mut().unwrap().2 = v,
            "S21.G00.82.004" => out.last_mut().unwrap().3 = v,
            _ => {}
        }
    }
    out
}

/// La ligne du CTP `code` au bordereau (23.004), s'il y est.
fn ctp_23(dsn: &str, code: &str) -> Option<String> {
    let l = lignes(dsn);
    let i = l.iter().position(|(k, v)| k == "S21.G00.23.001" && v == code)?;
    l[i + 1..].iter().take_while(|(k, _)| k != "S21.G00.23.001").find(|(k, _)| k == "S21.G00.23.004").map(|(_, v)| v.clone())
}

/// Le montant de la contribution : l'assiette au taux, a l'euro, la moitie vers le haut (L. 6242-1, VII ; L. 133-10 CSS).
fn a_l_euro(assiette: Decimal, taux: &str) -> String {
    format!("{:.2}", (assiette * dec(taux) / dec("100")).round_dp_with_strategy(0, rust_decimal::RoundingStrategy::MidpointAwayFromZero))
}

#[test]
fn la_dsn_de_mars_declare_la_contribution_supplementaire_de_l_annee_precedente() {
    // L. 6242-1 : 250 salaries et plus, redevable de la taxe d'apprentissage, alternants a 1 % de l'effectif (3 sur 300,
    // codes A05 et A07 du CRM 118, fiche 2624) : 0,2 % de l'assiette de la taxe d'apprentissage de 2025 (IV, 2°). Fiches
    // net-entreprises 2588 et 2963 : la DSN de mars 2026, un bloc 82 de code 079 du 01/01 au 31/12/2025 ; guide Urssaf du
    // 12/03/2026 : le CTP 998, son montant en 23.004.
    let c = Copie::trois_cents().alternants("2025", 3.0, 300.0);
    let dsn = c.dsn("2026-03").unwrap();
    let assiette = c.assiette_ta_2025();
    assert!(assiette > Decimal::ZERO);
    let montant = a_l_euro(assiette, "0.2");
    let csa: Vec<_> = blocs_82(&dsn).into_iter().filter(|b| b.1 == "079" || b.1 == "080").collect();
    assert_eq!(csa, [(montant.clone(), "079".to_string(), "01012025".to_string(), "31122025".to_string())]);
    assert_eq!(ctp_23(&dsn, "998"), Some(montant));
    paie::usages::verifier_fichier(&dsn).unwrap();
    paie::formats::verifier_fichier(&dsn).unwrap();
    // Un autre mois ne la porte pas.
    let fevrier = c.dsn("2026-02").unwrap();
    assert!(blocs_82(&fevrier).iter().all(|b| b.1 != "079") && ctp_23(&fevrier, "998").is_none());
}

#[test]
fn sous_1_pourcent_d_alternants_le_taux_est_de_0_4_pourcent() {
    let c = Copie::trois_cents().alternants("2025", 2.0, 300.0);
    let dsn = c.dsn("2026-03").unwrap();
    assert_eq!(ctp_23(&dsn, "998"), Some(a_l_euro(c.assiette_ta_2025(), "0.4")));
}

#[test]
fn a_5_pourcent_d_alternants_la_contribution_est_exoneree() {
    // Fiche 2588 : l'exoneration se declare en code 080, a 0.00 ; rien au bordereau.
    let c = Copie::trois_cents().alternants("2025", 15.0, 300.0);
    let dsn = c.dsn("2026-03").unwrap();
    let csa: Vec<_> = blocs_82(&dsn).into_iter().filter(|b| b.1 == "079" || b.1 == "080").collect();
    assert_eq!(csa, [("0.00".to_string(), "080".to_string(), "01012025".to_string(), "31122025".to_string())]);
    assert_eq!(ctp_23(&dsn, "998"), None);
    paie::usages::verifier_fichier(&dsn).unwrap();
    paie::formats::verifier_fichier(&dsn).unwrap();
}

#[test]
fn entre_3_et_5_pourcent_une_progression_de_10_pourcent_exonere() {
    // L. 6242-1, II, dernier alinea : 3 % atteints et l'effectif des alternants en hausse d'au moins 10 % sur l'annee
    // precedente (guide Urssaf, l. 2431 : « [(17-15) x 100] / 15 = 13,33 % »).
    let code = |avant: Option<f64>| -> Result<String, String> {
        let mut c = Copie::trois_cents().alternants("2025", 11.0, 300.0);
        if let Some(a) = avant {
            c = c.alternants("2024", a, 300.0);
        }
        let dsn = c.dsn("2026-03")?;
        Ok(blocs_82(&dsn).into_iter().find(|b| b.1 == "079" || b.1 == "080").unwrap().1)
    };
    assert_eq!(code(Some(10.0)).unwrap(), "080", "11 contre 10 : + 10 %");
    assert_eq!(code(Some(10.5)).unwrap(), "079", "11 contre 10,5 : + 4,8 %");
    let e = code(None).unwrap_err();
    assert!(e.contains("2024") && e.contains("A05") && e.contains("Rien n'est ecrit"), "{}", e);
}

#[test]
fn sans_les_effectifs_du_crm_118_la_contribution_supplementaire_refuse() {
    // Regle 1 : le quotient des alternants se lit du CRM 118 (fiche 2624), il ne se saisit ni ne se devine.
    let c = Copie::trois_cents();
    let e = c.dsn("2026-03").unwrap_err();
    assert!(e.contains("A05") && e.contains("A07") && e.contains("118") && e.contains("Rien n'est ecrit"), "{}", e);
    // Sous 250 salaries, rien a declarer, rien a lire.
    let petite = Copie::de("fictive-3090-petite").fiche(taxe_due_sur(None));
    let dsn = petite.dsn("2026-03").unwrap();
    assert!(blocs_82(&dsn).iter().all(|b| b.1 != "079" && b.1 != "080") && ctp_23(&dsn, "998").is_none());
}
