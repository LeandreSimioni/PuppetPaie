//! La deduction forfaitaire specifique (DFS) en paie.
//!
//! Textes (`atelier/reference/sources/dfs/LISEZ-MOI.md`) :
//! - arrete du 4 septembre 2025 relatif aux frais professionnels, art. 9 (base LEGI,
//!   `dfs/arrete-2025-frais-professionnels/`) : les professions de l'article 5 de l'annexe IV du CGI au 31/12/2000 ;
//!   l'option de l'employeur (accord collectif, CE, CSE ou DP, a defaut chaque salarie) ; « dans la limite de 7 600 euros
//!   par annee civile » ; les taux du spectacle vivant d'annee en annee (II) ; l'assiette garde les remboursements de
//!   frais, sauf la liste de l'annexe 2 ; « sans prejudice du sixieme alinea de l'article R. 242-1 » ;
//! - l'annexe 2, I, 2° et 5° : les defraiements de tournee des artistes dramatiques, lyriques, choregraphiques et
//!   regisseurs de theatre, les frais de deplacement des musiciens, choristes et chefs d'orchestre restent hors de
//!   l'assiette ;
//! - BOSS « Frais professionnels », § 2320 (`atelier/reference/sources/boss/boss-frais-professionnels-20261013.txt`) : pour
//!   le spectacle vivant, par tolerance, tout remboursement de frais au sens de l'arrete se cumule avec la DFS, hors de
//!   l'assiette abattue ;
//! - CSS R. 242-1, al. 6, et BOSS « Assiette generale » § 410 : l'assiette abattue ne descend pas sous le SMIC ;
//! - annexes VIII et X du 15/11/2024, art. 49 : le chomage « avant application de l'abattement » ; norme DSN, bloc 78 :
//!   les Conges Spectacles « avant toute deduction forfaitaire specifique ».
//!
//! Sur une copie de la config et de trois mois de saisie de `fictive-3090-grande` (compagnie inventee), dans le dossier temporaire des
//! tests ; l'original n'est pas touche. Deburau (comedien) et Jacquet (musicien) y sont des artistes intermittents,
//! Chaminade (regisseur lumiere) un technicien, Montansier une cadre ; Deburau a un contrat en mars 2026. La reduction
//! generale sous DFS : arrete du 4 decembre 2019, art. 1er (`dfs/arrete-2019-reduction-generale/`).

use engagement::calcul::Paie;
use droit::contexte::Contexte;
use engagement::{charger, Saisie};
use socle::num::Decimal;
use serde_json::{json, Value};
use crate::outils::{chemin_unique, dec, fictive, ecrire, lire, Copie};
use std::str::FromStr;

/// La config de `fictive-3090-grande` et les saisies de mars, octobre et novembre 2026 (les mois que la deduction de
/// l'annee de Deburau lit), `retouche` appliquee a sa fiche entreprise et a ses fiches salaries.
fn copie(retouche: impl Fn(&mut Value, &mut Value)) -> Copie {
    let c = Copie::config_de("fictive-3090-grande");
    std::fs::create_dir_all(c.0.join("engagements")).unwrap();
    for mois in ["2026-03", "2026-10", "2026-11"] {
        let f = format!("engagements/{}.json", mois);
        std::fs::copy(fictive("fictive-3090-grande").join(&f), c.0.join(&f)).unwrap();
    }
    let (mut ent, mut sal) = (lire(&c.0, "config/entreprise.json"), lire(&c.0, "config/salaries.json"));
    retouche(&mut ent, &mut sal);
    ecrire(&c.0, "config/entreprise.json", &ent);
    ecrire(&c.0, "config/salaries.json", &sal);
    c
}

/// La compagnie opte par accord collectif ; Deburau est artiste dramatique.
fn deburau() -> Copie {
    copie(|ent, sal| {
        ent["deduction_forfaitaire_option"] = json!("accord_collectif");
        sal["DEBURAU"]["deduction_forfaitaire"] = json!("artiste_dramatique");
    })
}

fn essai(ctx: &Contexte, s: &Value) -> Result<Saisie, String> {
    let chemin = chemin_unique("dfs.json");
    std::fs::write(&chemin, s.to_string()).unwrap();
    let r = crate::outils::charger_fichier(ctx, &chemin);
    let _ = std::fs::remove_file(&chemin);
    r
}

/// Un mois d'octobre 2026 : un cachet de chacun des `(salarie, numero, montant)`, le 20.
fn cachets(contrats: &[(&str, &str, &str)]) -> Value {
    let engagements: Vec<Value> = contrats
        .iter()
        .map(|(salarie, numero, montant)| {
            json!({"salarie": salarie, "spectacle": "266Z00910022", "debut": "2026-10-20", "fin": "2026-10-20", "numero": numero,
                   "date_paiement": "2026-10-20",
                   "lignes": [{"rubrique": "cachet", "quantite": 1, "taux": f64::from_str(montant).unwrap(), "lieu": "Theatre fictif",
                               "dates": ["2026-10-20"]}]})
        })
        .collect();
    json!({"mois": "2026-10", "date_paiement": "2026-10-31", "engagements": engagements})
}

/// Le refus d'une saisie (le message), ou l'echec du test si elle se calcule.
fn refus(ctx: &Contexte, s: &Value) -> String {
    match essai(ctx, s) {
        Ok(_) => panic!("la saisie se calcule, un refus etait attendu"),
        Err(e) => e,
    }
}

fn paie<'a>(s: &'a Saisie, numero: &str) -> &'a Paie {
    s.paies.iter().find(|p| p.engagement.numero.as_deref() == Some(numero)).unwrap()
}

/// La base de la ligne `id` du bulletin.
fn base(p: &Paie, id: &str) -> Decimal {
    p.lignes.iter().find(|l| l.id == id).unwrap_or_else(|| panic!("pas de ligne {}", id)).base
}

#[test]
fn un_artiste_sous_dfs_abat_la_securite_sociale_et_la_retraite_pas_le_chomage_ni_la_csg_ni_les_conges() {
    // Deburau, artiste dramatique (categorie 25 : 18 % en 2026), un cachet de 200,00 : 36,00 de deduction, 164,00 abattus.
    // Clairon, le meme cachet, sans DFS.
    let c = deburau();
    let ctx = c.contexte("2026-12-28");
    let s = essai(&ctx, &cachets(&[("Deburau", "DE2.26.10", "200.00"), ("Clairon", "CL6.26.10", "200.00")])).unwrap();
    let (d, sans) = (paie(&s, "DE2.26.10"), paie(&s, "CL6.26.10"));
    let dfs = d.deduction_forfaitaire.as_ref().expect("la DFS s'applique");
    assert_eq!((dfs.profession.as_str(), dfs.categorie.as_str(), dfs.taux, dfs.montant), ("artiste_dramatique", "25", dec("18"), dec("36.00")));
    for id in ["maladie", "atmp", "vieil_plaf", "vieil_deplaf", "famille", "autres_1", "retraite_t1"] {
        assert_eq!(base(d, id), dec("164.00"), "{} sur le brut abattu", id);
        assert_eq!(base(sans, id), dec("200.00"), "{} sans DFS", id);
    }
    // La vieillesse plafonnee : le brut abattu dans la limite du plafond (V1n la laissait sur le brut entier).
    assert_eq!(d.bases.get("brut_plafonne_dfs").copied(), Some(dec("164.00")));
    for id in ["chomage", "conges_spec", "prevoyance", "ccn"] {
        assert_eq!(base(d, id), base(sans, id), "{} ne s'abat pas", id);
    }
    assert_eq!(d.base_csg, sans.base_csg, "la CSG a son propre abattement");
    assert!(d.total_sal < sans.total_sal && d.total_pat < sans.total_pat);
    // Le net social n'est pas diminue de l'abattement (BOSS, montant net social, Q 26) : seules les cotisations changent.
    assert_eq!(d.net_social, d.brut - d.total_sal);
}

#[test]
fn la_dfs_est_une_option_de_l_employeur_et_l_accord_du_salarie_se_dit() {
    let ctx_de = |option: &str, accord: &str, profession: &str| {
        let (option, accord, profession) = (option.to_string(), accord.to_string(), profession.to_string());
        copie(move |ent, sal| {
            ent["deduction_forfaitaire_option"] = json!(option);
            sal["DEBURAU"]["deduction_forfaitaire"] = json!(profession);
            sal["DEBURAU"]["deduction_forfaitaire_accord"] = json!(accord);
        })
    };
    let un = cachets(&[("Deburau", "DE2.26.10", "200.00")]);
    // La compagnie n'a pas opte : la fiche du salarie ne suffit pas.
    let c = ctx_de("", "", "artiste_dramatique");
    let e = refus(&c.contexte("2026-12-28"), &un);
    assert!(e.contains("n'a pas") && e.contains("opte") && e.contains("art. 9"), "{}", e);
    // Une categorie n'est pas une profession.
    let c = ctx_de("accord_collectif", "", "25");
    let e = refus(&c.contexte("2026-12-28"), &un);
    assert!(e.contains("PROFESSION"), "{}", e);
    // Salarie par salarie : l'accord se dit, son refus ote la deduction.
    let c = ctx_de("accord_individuel", "", "artiste_dramatique");
    let e = refus(&c.contexte("2026-12-28"), &un);
    assert!(e.contains("accord") && e.contains("silence vaut accord"), "{}", e);
    let c = ctx_de("accord_individuel", "refus", "artiste_dramatique");
    let s = essai(&c.contexte("2026-12-28"), &un).unwrap();
    assert!(s.paies[0].deduction_forfaitaire.is_none() && base(&s.paies[0], "maladie") == dec("200.00"));
    for accord in ["contrat", "consentement_annuel"] {
        let c = ctx_de("accord_individuel", accord, "artiste_dramatique");
        let s = essai(&c.contexte("2026-12-28"), &un).unwrap();
        assert_eq!(base(&s.paies[0], "maladie"), dec("164.00"), "{}", accord);
    }
    // Le CSE : l'option vaut pour la profession, sans accord individuel.
    let c = ctx_de("accord_cse", "", "artiste_dramatique");
    assert_eq!(base(&essai(&c.contexte("2026-12-28"), &un).unwrap().paies[0], "maladie"), dec("164.00"));
    let c = ctx_de("autre_chose", "", "artiste_dramatique");
    assert!(refus(&c.contexte("2026-12-28"), &un).contains("deduction_forfaitaire_option"));
}

#[test]
fn le_plafond_annuel_de_7600_euros_se_suit_dans_le_mois_et_d_un_mois_a_l_autre() {
    let c = deburau();
    let ctx = c.contexte("2026-12-28");
    // Ce que mars 2026 a deja deduit (DE5.26.01, le seul contrat de Deburau avant octobre).
    let s = essai(&ctx, &cachets(&[("Deburau", "DE2.26.10", "200.00")])).unwrap();
    let mars = s.paies[0].deduction_forfaitaire.as_ref().unwrap().deja;
    assert!(mars > Decimal::ZERO, "mars est deduit");
    // Octobre : un gros contrat (40 000,00 : 7 200,00 de deduction), puis un second de 2 000,00 (360,00) que le plafond
    // ramene a ce qui reste.
    let s = essai(&ctx, &cachets(&[("Deburau", "DE2.26.10", "40000.00"), ("Deburau", "DE2.26.11", "2000.00")])).unwrap();
    let (a, b) = (paie(&s, "DE2.26.10"), paie(&s, "DE2.26.11"));
    let (da, db) = (a.deduction_forfaitaire.as_ref().unwrap(), b.deduction_forfaitaire.as_ref().unwrap());
    assert_eq!((da.deja, da.montant), (mars, dec("7200.00")));
    assert_eq!(db.deja, mars + dec("7200.00"));
    assert_eq!(db.montant, dec("7600.00") - mars - dec("7200.00"), "le plafond ramene le second contrat");
    assert_eq!(base(b, "maladie"), dec("2000.00") - db.montant);
    assert!(b.avertissements.iter().any(|x| x.texte.contains("7600") && x.texte.contains("annee civile")), "{:?}", b.avertissements);
    // Octobre enregistre, novembre (Deburau y a un contrat) ne deduit plus rien.
    let mut octobre = cachets(&[("Deburau", "DE2.26.10", "40000.00"), ("Deburau", "DE2.26.11", "2000.00")]);
    octobre["engagements"].as_array_mut().unwrap().extend(crate::outils::saisie(&c.0, "2026-10")["engagements"].as_array().unwrap().iter().cloned());
    crate::outils::poser_saisie(&c.0, "2026-10", &octobre);
    let ctx = c.contexte("2026-12-28");
    let nov = charger(&ctx, "2026-11").unwrap();
    let p = nov.paies.iter().find(|p| p.engagement.salarie == "Deburau").unwrap();
    let d = p.deduction_forfaitaire.as_ref().unwrap();
    assert_eq!((d.deja, d.montant), (dec("7600.00"), dec("0.00")));
    assert_eq!(base(p, "maladie"), p.brut, "plus de deduction en novembre");
}

#[test]
fn l_assiette_abattue_ne_descend_pas_sous_le_smic() {
    // Un cachet de 100,00 : 18 % le ramenerait a 82,00, sous le SMIC du contrat (12,31 x 7 h = 86,17 ; le SMIC que la DSN
    // retient pour un cachet, `heures_par_cachet`). L'assiette s'arrete a 86,17, la deduction a 13,83.
    let c = deburau();
    let s = essai(&c.contexte("2026-12-28"), &cachets(&[("Deburau", "DE2.26.10", "100.00")])).unwrap();
    let p = &s.paies[0];
    assert_eq!(p.plafond, dec("86.17"));
    assert_eq!(p.deduction_forfaitaire.as_ref().unwrap().montant, dec("13.83"));
    assert_eq!(base(p, "maladie"), dec("86.17"));
    assert!(p.avertissements.iter().any(|x| x.texte.contains("R. 242-1")), "{:?}", p.avertissements);
}

/// Une ligne de frais `frais` ajoutee a un cachet de 200,00 du 20 octobre 2026.
fn avec_frais(salarie: &str, numero: &str, frais: &[Value]) -> Value {
    let mut s = cachets(&[(salarie, numero, "200.00")]);
    s["engagements"][0]["lignes"].as_array_mut().unwrap().extend(frais.iter().cloned());
    s
}

/// Les blocs 54 du versement du contrat `numero` : (54.001, 54.002, 54.005).
fn blocs_54(dsn: &str, numero: &str) -> Vec<(String, String, String)> {
    let mut out: Vec<(String, String, String)> = Vec::new();
    for (k, v) in crate::outils::versement(dsn, numero) {
        match k.as_str() {
            "S21.G00.54.001" => out.push((v, String::new(), String::new())),
            "S21.G00.54.002" => out.last_mut().unwrap().1 = v,
            "S21.G00.54.005" => out.last_mut().unwrap().2 = v,
            _ => {}
        }
    }
    out
}

#[test]
fn un_remboursement_de_frais_sous_dfs_reste_hors_de_l_assiette_abattue_par_la_tolerance_du_boss() {
    // BOSS « Frais professionnels », § 2320 (atelier/reference/sources/boss/boss-frais-professionnels-20261013.txt,
    // l. 2917-2921) : pour les huit secteurs, dont le spectacle vivant, « l'ensemble des remboursements de frais
    // professionnels definis par l'arrete du 4 septembre 2025 peut faire l'objet d'un cumul avec la deduction forfaitaire
    // specifique » ; leur integration dans l'assiette avant l'abattement « n'est pas obligatoire ». Un remboursement que
    // l'annexe 2 de l'arrete ne nomme pas (le reel d'un artiste dramatique : ce n'est pas une « indemnite journaliere de
    // defraiement ») reste donc hors de l'assiette, comme ceux de l'annexe ; la paie se produit (elle se refusait).
    let c = copie(|ent, sal| {
        ent["deduction_forfaitaire_option"] = json!("accord_collectif");
        sal["DEBURAU"]["deduction_forfaitaire"] = json!("artiste_dramatique");
        sal["JACQUET"]["deduction_forfaitaire"] = json!("musicien");
    });
    // Une rubrique de la compagnie : un remboursement de frais (nature `frais`) que le catalogue ne nomme pas. Elle se
    // classe par ce qu'elle est, pas par son nom.
    ecrire(&c.0, "config/rubriques-compagnie.json", &json!({"rubriques": {"frais_kilometriques": {
        "libelle": "Indemnites kilometriques", "unite": "forfait", "nature": "frais", "saisie": ["quantite", "taux"],
        "dsn_autres_bruts": "08", "soumis": false, "imposable": false, "conges_spectacles": false,
        "source": "Bareme kilometrique du test."}}}));
    let ctx = c.contexte("2026-12-28");
    let forfait = json!({"rubrique": "defraiement_forfait", "quantite": 2, "taux": 21.40});
    let reel = json!({"rubrique": "defraiement_reel", "quantite": 1, "taux": 35.0, "montant": 35.0});
    let km = json!({"rubrique": "frais_kilometriques", "quantite": 100, "taux": 0.5});
    // Annexe 2, I, 2° : le defraiement journalier d'un artiste dramatique en tournee.
    let s = essai(&ctx, &avec_frais("Deburau", "DE2.26.10", &[forfait.clone()])).unwrap();
    assert_eq!((s.paies[0].brut, base(&s.paies[0], "maladie")), (dec("200.00"), dec("164.00")));
    // Le reel et les kilometres, que l'annexe ne nomme pas pour lui : la paie se produit, hors de l'assiette abattue.
    let mut saisie = avec_frais("Deburau", "DE2.26.10", &[reel.clone(), km.clone()]);
    let clairon = avec_frais("Clairon", "CL6.26.10", &[reel.clone(), km.clone()]);
    saisie["engagements"].as_array_mut().unwrap().push(clairon["engagements"][0].clone());
    let s = essai(&ctx, &saisie).expect("la tolerance du BOSS, § 2320 : plus de refus");
    let (d, sans) = (paie(&s, "DE2.26.10"), paie(&s, "CL6.26.10"));
    let dfs = d.deduction_forfaitaire.as_ref().expect("la DFS s'applique");
    assert_eq!((d.brut, dfs.montant, base(d, "maladie")), (dec("200.00"), dec("36.00"), dec("164.00")), "les frais n'entrent pas dans l'assiette abattue");
    assert_eq!(d.bases.get("brut_dfs").copied(), Some(dec("164.00")));
    for id in ["chomage", "conges_spec"] {
        assert_eq!(base(d, id), base(sans, id), "{} : les frais n'y entrent pas plus que hors DFS", id);
    }
    // Le bulletin les porte comme hors DFS : leurs lignes, payees en plus du net, hors du net imposable et du net social.
    for p in [d, sans] {
        let frais: Vec<(&str, Decimal, Option<&str>)> =
            p.remuneration.iter().filter(|l| !l.soumis).map(|l| (l.code.as_str(), l.montant, l.dsn_autres_bruts.as_deref())).collect();
        assert_eq!(frais, vec![("defraiement_reel", dec("35.00"), Some("09")), ("frais_kilometriques", dec("50.00"), Some("08"))]);
        assert_eq!(p.net_a_payer, p.brut - p.total_sal + dec("85.00"));
        assert_eq!(p.net_social, p.brut - p.total_sal);
    }
    // La DSN : les blocs 54 de type 09 et 08, comme hors DFS, et le type 10 a la seule deduction ; aucun type 35 (rien
    // n'est reintegre). Les autres types (93, la prevoyance) n'en dependent pas.
    let dsn = paie::dsn::generer(&ctx, &s.paies, "2026-10", &[], &[], &paie::dsn::Options::default()).unwrap();
    let a_lui = |t: &str, m: &str, n: &str| (t.to_string(), m.to_string(), n.to_string());
    let frais_et_deduction = |numero: &str| -> Vec<(String, String, String)> {
        blocs_54(&dsn, numero).into_iter().filter(|b| ["08", "09", "10", "35"].contains(&b.0.as_str())).collect()
    };
    assert_eq!(frais_et_deduction("DE2.26.10"), vec![a_lui("09", "35.00", "DE2.26.10"), a_lui("08", "50.00", "DE2.26.10"), a_lui("10", "36.00", "DE2.26.10")]);
    assert_eq!(frais_et_deduction("CL6.26.10"), vec![a_lui("09", "35.00", "CL6.26.10"), a_lui("08", "50.00", "CL6.26.10")]);
    // Annexe 2, I, 5° : les frais de deplacement d'un musicien, au forfait comme au reel (inchange).
    for frais in [forfait, reel] {
        let s = essai(&ctx, &avec_frais("Jacquet", "JA7.26.10", &[frais])).unwrap();
        let p = &s.paies[0];
        assert_eq!(p.deduction_forfaitaire.as_ref().unwrap().categorie, "20");
        assert_eq!(base(p, "maladie"), dec("168.00"), "16 % en 2026");
    }
}

#[test]
fn une_indemnite_hors_assiette_qui_n_est_pas_un_remboursement_de_frais_refuse_sous_dfs() {
    // La tolerance du BOSS (§ 2320) ne vaut que pour les remboursements de frais au sens de l'arrete ; le rescrit
    // BOSS-RES-000010 (§ 2325, l. 2923-2934) : « l'indemnite de transport prevue par une CCN [...] ne constitue pas un
    // remboursement de frais professionnels au sens de l'arrete du 4 septembre 2025 [...] Il faut donc l'integrer dans
    // l'assiette des cotisations et contributions sociales des le 1er euro ». Une rubrique de la compagnie, de nature
    // `indemnite` et hors assiette : aucun texte depose ne dit ce qu'elle devient sous DFS (reintegree ou exoneree par sa
    // propre regle), la paie se refuse et dit pourquoi ; hors DFS, elle se paie comme avant.
    let c = deburau();
    ecrire(&c.0, "config/rubriques-compagnie.json", &json!({"rubriques": {"indemnite_transport_ccn": {
        "libelle": "Indemnite de transport conventionnelle", "unite": "forfait", "nature": "indemnite", "saisie": ["quantite", "taux"],
        "soumis": false, "imposable": false, "conges_spectacles": false, "source": "Convention du test."}}}));
    let ctx = c.contexte("2026-12-28");
    let transport = json!({"rubrique": "indemnite_transport_ccn", "quantite": 1, "taux": 12.0});
    let e = refus(&ctx, &avec_frais("Deburau", "DE2.26.10", &[transport.clone()]));
    assert!(e.contains("Indemnite de transport conventionnelle") && e.contains("BOSS-RES-000010") && e.contains("n'est pas un remboursement de frais"), "{}", e);
    let s = essai(&ctx, &avec_frais("Clairon", "CL6.26.10", &[transport])).unwrap();
    assert_eq!(s.paies[0].net_a_payer, s.paies[0].brut - s.paies[0].total_sal + dec("12.00"), "sans DFS, rien ne change");
}

#[test]
fn ce_que_les_textes_deposes_ne_disent_pas_refuse_en_le_disant() {
    let c = copie(|ent, sal| {
        ent["deduction_forfaitaire_option"] = json!("accord_collectif");
        sal["MONTANSIER"]["deduction_forfaitaire"] = json!("regisseur_theatre");
    });
    let ctx = c.contexte("2026-12-28");
    // Un salarie au mois.
    let cdi = json!({"mois": "2026-10", "date_paiement": "2026-10-31", "engagements": [
        {"salarie": "Montansier", "nature": "cdi", "debut": "2023-09-04", "numero": "MO.23.01", "quotite_contrat": 151.67,
         "lignes": [{"rubrique": "mois", "quantite": 1, "taux": 3000.0, "heures": 151.67}]}]});
    let e = refus(&ctx, &cdi);
    assert!(e.contains("paye au mois") && e.contains("Ce qui manque"), "{}", e);
    // Une cadre intermittente : l'Apec.
    let journee = |salarie: &str, numero: &str, taux: f64| {
        json!({"mois": "2026-10", "date_paiement": "2026-10-31", "engagements": [
            {"salarie": salarie, "spectacle": "266Z00910022", "debut": "2026-10-20", "fin": "2026-10-20", "numero": numero,
             "date_paiement": "2026-10-20",
             "lignes": [{"rubrique": "journee", "quantite": 1, "taux": taux, "heures": 8, "lieu": "Theatre fictif", "dates": ["2026-10-20"]}]}]})
    };
    let e = refus(&ctx, &journee("Montansier", "MO2.26.10", 300.0));
    assert!(e.contains("Apec"), "{}", e);
}

#[test]
fn la_reduction_generale_d_un_technicien_sous_dfs_ne_depasse_pas_130_pour_cent_de_celle_du_brut_entier() {
    // Chaminade, regisseur (de theatre, 16 % en 2026), une journee de 150,00 pour 8 h : brut abattu 126,00. La reduction
    // generale se calcule sur 126,00 (27,35 environ), dans la limite de 130 % de celle de 150,00 (arrete du 4 decembre 2019,
    // art. 1er, redaction du 4 septembre 2025 ; CSS L. 241-13, III).
    let journee = json!({"mois": "2026-10", "date_paiement": "2026-10-31", "engagements": [
        {"salarie": "Chaminade", "spectacle": "266Z00910022", "debut": "2026-10-20", "fin": "2026-10-20", "numero": "CH8.26.10",
         "date_paiement": "2026-10-20",
         "lignes": [{"rubrique": "journee", "quantite": 1, "taux": 150.0, "heures": 8, "lieu": "Theatre fictif", "dates": ["2026-10-20"]}]}]});
    let sans = copie(|_, _| {});
    let rg_sans = essai(&sans.contexte("2026-12-28"), &journee).unwrap().paies[0].reduction_generale.clone().unwrap();
    let avec = copie(|ent, sal| {
        ent["deduction_forfaitaire_option"] = json!("accord_cse");
        sal["CHAMINADE"]["deduction_forfaitaire"] = json!("regisseur_theatre");
    });
    let s = essai(&avec.contexte("2026-12-28"), &journee).unwrap();
    let p = &s.paies[0];
    assert_eq!(p.deduction_forfaitaire.as_ref().unwrap().montant, dec("24.00"));
    let rg = p.reduction_generale.clone().unwrap();
    let limite = socle::num::eur(rg_sans.montant * dec("1.30"));
    assert_eq!(rg.montant, limite, "la limite de 130 % s'applique (sans elle : la reduction de 126,00)");
    assert_eq!(rg.urssaf + rg.chomage + rg.agirc_arrco, rg.montant, "la reduction se repartit encore");
    let ligne = p.lignes.iter().find(|l| l.id == "reduction_generale").unwrap();
    assert_eq!(ligne.montant_pat, -limite);
}

#[test]
fn la_fiche_salarie_dit_une_profession_et_son_accord_la_fiche_entreprise_l_option() {
    // Lot R9 : la fiche salarie dit la PROFESSION de l'article 5 de l'annexe IV du CGI (plus une categorie, « 20 » ou
    // « 25 »), et l'accord du salarie quand l'option repose sur lui ; la fiche entreprise dit l'option de l'employeur
    // (arrete du 4 septembre 2025, art. 9, I, al. 2 et 3). Le moteur verifie les valeurs (`champs::verifier_fiche`).
    let erreur = |nom: &str, cle: &str, v: &str| -> bool {
        let mut f = serde_json::Map::new();
        f.insert(cle.into(), json!(v));
        paie::champs::verifier_fiche(nom, &f).unwrap().iter().any(|a| a["champ"] == cle && a["gravite"] == "erreur")
    };
    assert!(erreur("salarie", "deduction_forfaitaire", "25"), "une categorie n'est plus une profession");
    for profession in ["musicien", "choriste", "chef_orchestre", "regisseur_theatre", "artiste_dramatique", "artiste_lyrique",
                       "artiste_choregraphique", "artiste_cinematographique", ""] {
        assert!(!erreur("salarie", "deduction_forfaitaire", profession), "{}", profession);
    }
    for accord in ["contrat", "consentement_annuel", "refus", ""] {
        assert!(!erreur("salarie", "deduction_forfaitaire_accord", accord), "{}", accord);
    }
    assert!(erreur("salarie", "deduction_forfaitaire_accord", "oui"));
    for option in ["accord_collectif", "accord_cse", "accord_individuel", ""] {
        assert!(!erreur("entreprise", "deduction_forfaitaire_option", option), "{}", option);
    }
    assert!(erreur("entreprise", "deduction_forfaitaire_option", "oui"));
    // Les professions de la fiche sont celles du bareme de l'annee, et aucune autre.
    let bar: Value = lire(&crate::outils::code_livre(), "baremes/2026.json");
    let mut du_bareme: Vec<String> = bar["deduction_forfaitaire"]["professions"].as_object().unwrap().keys().cloned().collect();
    du_bareme.sort();
    let champs = paie::champs::fiche("salarie").unwrap();
    let c = champs.iter().find(|c| c.cle == "deduction_forfaitaire").unwrap();
    let mut de_la_fiche: Vec<String> = c.choix.iter().filter_map(|x| x.as_str()).filter(|x| !x.is_empty()).map(String::from).collect();
    de_la_fiche.sort();
    assert_eq!(de_la_fiche, du_bareme);
}
