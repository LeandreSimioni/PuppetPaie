//! La vieillesse plafonnee, le FNAL et le chomage d'un intermittent s'assoient sur SON plafond, celui que le bas du bulletin imprime (`plafond::pour_paie`).
//!
//! Textes (atelier/reference/sources/plafond-intermittents/LISEZ-MOI.md) : CSS L. 241-3 (« dans la limite d'un
//! plafond »), R. 242-2 (le plafond reduit a due proportion des jours du contrat), arrete du 24 janvier 1975 (douze fois
//! le plafond horaire par journee de travail d'un artiste engage moins de cinq jours continus) ; annexes VIII et X du
//! 15/11/2024, art. 49 (le chomage dans la limite de quatre fois ce plafond). Norme DSN 2026.1 : « 02 - Assiette brute
//! plafonnee », « 07 - Assiette des contributions d'Assurance Chomage » (p. 294), « 921 - Assiette plafonnee » (p. 168).
//!
//! Sur la copie figee des compagnies fictives (`tests/donnees/fictives/`, compagnies inventees), lue sans y rien ecrire : les
//! saisies s'ecrivent dans le dossier temporaire des tests.

use paie::calcul::Paie;
use droit::contexte::Contexte;
use socle::date::Jour;
use paie::dsn::{generer, Options};
use paie::engagement::{charger, Saisie};
use socle::num::{eur, pct, Decimal};
use rust_decimal::RoundingStrategy;
use serde_json::{json, Value};
use crate::outils::{base78, compagnie::sans_les_pdf, ctp, dec, fictive, dossier_de_test, versement};
use std::path::PathBuf;

/// Le 81.003 de la cotisation `code81` du versement (la premiere).
fn base81(v: &[(String, String)], code81: &str) -> Decimal {
    crate::outils::base81(v, code81).unwrap_or_else(|| panic!("pas de 81.003 sous {}", code81))
}
use std::str::FromStr;

fn saisie(ctx: &Contexte, s: Value) -> Saisie {
    let chemin = crate::outils::chemin_unique("plafond-intermittents.json");
    std::fs::write(&chemin, s.to_string()).unwrap();
    charger(ctx, &chemin).unwrap()
}

fn paie<'a>(s: &'a Saisie, numero: &str) -> &'a Paie {
    s.paies.iter().find(|p| p.engagement.numero.as_deref() == Some(numero)).unwrap()
}

/// La ligne `id` du bulletin : (base, montant patronal, montant salarial).
fn ligne(p: &Paie, id: &str) -> (Decimal, Decimal, Decimal) {
    let l = p.lignes.iter().find(|l| l.id == id).unwrap_or_else(|| panic!("pas de ligne {} au bulletin", id));
    (l.base, l.montant_pat, l.montant_sal)
}

fn taux(p: &Paie, id: &str) -> (Decimal, Option<Decimal>) {
    let l = p.lignes.iter().find(|l| l.id == id).unwrap();
    let d = |t: f64| Decimal::from_str(&t.to_string()).unwrap();
    (d(l.taux_pat.unwrap()), l.taux_sal.map(d))
}

fn a_l_euro(x: Decimal) -> Decimal {
    x.round_dp_with_strategy(0, RoundingStrategy::MidpointNearestEven).round_dp(2)
}

#[test]
fn juillet_2026_les_intermittents_au_dessus_de_leur_plafond_cotisent_sur_lui() {
    // La fictive figee 3090-grande, juillet 2026 : trois artistes a 4 077,00 et deux techniciens a l'heure (3 496,00 et
    // 3 312,00), tous du 02 au 25/07. Vague 1o : chacun a le plafond de ses jours remuneres (BOSS, assiette generale,
    // § 1090), 4 005 x jours dates / 31 ; vague 1l : les 24 jours du contrat, 3 100,65 pour tous.
    let ctx = Contexte::nouveau(&fictive("fictive-3090-grande"), Some(Jour::lire("2026-07-28").unwrap())).unwrap();
    let s = charger(&ctx, &fictive("fictive-3090-grande").join("paies/2026-07.json")).unwrap();
    let dsn = generer(&ctx, &s.paies, "2026-07", &[], &[], &Options::default()).unwrap();
    for (numero, brut) in [("BI4.26.06", "4077.00"), ("CH8.26.04", "3496.00"), ("SE9.26.05", "3312.00")] {
        let p = paie(&s, numero);
        let jours = p.engagement.dates_travail.iter().collect::<std::collections::BTreeSet<_>>().len();
        assert!(jours >= 5 && jours < 24, "{} : {} jours remuneres", numero, jours);
        let plafond = eur(dec("4005") * Decimal::from(jours as i64) / dec("31"));
        assert_eq!(p.brut, dec(brut), "{}", numero);
        assert_eq!(p.plafond_urssaf(&ctx).unwrap(), plafond, "{} : le plafond du bas de bulletin", numero);
        // Au bulletin : la vieillesse plafonnee sur le plafond, le FNAL sur le plafond majore de 11,5 %.
        let (b, pat, sal) = ligne(p, "vieil_plaf");
        let (tp, ts) = taux(p, "vieil_plaf");
        assert_eq!(b, plafond, "{} : vieillesse plafonnee", numero);
        assert_eq!((pat, sal), (pct(plafond, tp), pct(plafond, ts.unwrap())), "{}", numero);
        let fnal = eur(plafond * dec("1.115"));
        assert_eq!(ligne(p, "autres_2").0, fnal, "{} : FNAL", numero);
        // Le deplafonne sur le brut, le chomage dans la limite de quatre plafonds.
        assert_eq!(ligne(p, "vieil_deplaf").0, p.brut, "{}", numero);
        assert_eq!(ligne(p, "chomage").0, p.brut.min(Decimal::from(4) * plafond), "{}", numero);
        // Dans la DSN : la base 02 est le plafond, la 03 le brut, la 07 le brut ; le 049 (FNAL) porte la base du bulletin.
        let v = versement(&dsn, numero);
        assert_eq!(base78(&v, "02"), vec![plafond], "{} : base 02", numero);
        assert_eq!(base78(&v, "03"), vec![p.brut], "{} : base 03", numero);
        assert_eq!(base78(&v, "07"), vec![p.brut.min(Decimal::from(4) * plafond)], "{} : base 07", numero);
        assert_eq!(base81(&v, "049"), fnal, "{} : 81.003 du FNAL", numero);
    }
    // Au bordereau : 312/921 (artistes) et 100/921 (techniciens) portent les assiettes plafonnees ; 312/920 et 100/920
    // gardent le brut. 334 et 332 (FNAL, 921) : le plafond majore.
    let somme = |statut: &str, f: &dyn Fn(&Paie) -> Decimal| {
        s.paies
            .iter()
            .filter(|p| !p.engagement.nat().unwrap().mensuelle() && p.salarie["statut"] == statut)
            .fold(Decimal::ZERO, |a, p| a + f(p))
    };
    let plaf = |p: &Paie| p.brut.min(p.plafond_urssaf(&ctx).unwrap());
    let plaf115 = |p: &Paie| if p.brut <= p.plafond_urssaf(&ctx).unwrap() { p.bases["brut_x115"] } else { eur(p.plafond_urssaf(&ctx).unwrap() * dec("1.115")) };
    assert_eq!(ctp(&dsn, "312", "921"), Some(a_l_euro(somme("artiste", &plaf))));
    assert_eq!(ctp(&dsn, "312", "920"), Some(a_l_euro(somme("artiste", &|p| p.brut))));
    assert_eq!(ctp(&dsn, "334", "921"), Some(a_l_euro(somme("artiste", &plaf115))));
    // Le premier 100/921 est celui des techniciens intermittents (le second, celui des permanents, porte leur tranche 1).
    assert_eq!(ctp(&dsn, "100", "921"), Some(a_l_euro(somme("technicien", &plaf))));
    assert_eq!(ctp(&dsn, "100", "920"), Some(a_l_euro(somme("technicien", &|p| p.brut))));
    assert_eq!(ctp(&dsn, "332", "921"), Some(a_l_euro(somme("technicien", &plaf115))));
    assert!(somme("artiste", &plaf) < somme("artiste", &|p| p.brut), "le cas eprouve bien le plafond");
    // Lot R2 : les complements maladie et allocations familiales « ABATT. 30 % » (636 et 432) sont ceux des artistes ;
    // un technicien releve du taux plein, 635 et 430, comme son nominatif (907 a 6,00, 102 a 1,80) : « Seuls les artistes
    // du spectacle sont concernes par l'application des taux reduits. Les techniciens ne sont pas concernes »
    // (atelier/reference/sources/urssaf-taux-reduits-artistes/urssaf-taux-reduits-cas-particuliers-20260715.txt, l. 24 ;
    // tableau DIDA, atelier/reference/sources/chomage-permanents/urssaf-equivalence-dida-20260611.csv, l. 838, 1410 et
    // 1450 : « Ce CTP artistes »). La copie figee a un fichier des organismes compose avant : il se lit juste sans etre
    // recompose. Le premier 430 et le premier 635 sont ceux des techniciens (les seconds, ceux des permanents).
    assert_eq!(ctp(&dsn, "432", "920"), Some(a_l_euro(somme("artiste", &|p| p.brut))));
    assert_eq!(ctp(&dsn, "636", "920"), Some(a_l_euro(somme("artiste", &|p| p.brut))));
    assert_eq!(ctp(&dsn, "430", "920"), Some(a_l_euro(somme("technicien", &|p| p.brut))));
    assert_eq!(ctp(&dsn, "635", "920"), Some(a_l_euro(somme("technicien", &|p| p.brut))));
    for (numero, code81, taux) in [("CH8.26.04", "102", "1.80"), ("CH8.26.04", "907", "6.00"), ("BI4.26.06", "102", "1.26"), ("BI4.26.06", "907", "4.20")] {
        let v = versement(&dsn, numero);
        let i = v.iter().position(|(k, x)| k == "S21.G00.81.001" && x == code81).unwrap();
        let t = v[i + 1..].iter().take_while(|(k, _)| k != "S21.G00.81.001").find(|(k, _)| k == "S21.G00.81.007").map(|(_, x)| dec(x));
        assert_eq!(t, Some(dec(taux)), "{} : taux du {} au nominatif", numero, code81);
    }
}

#[test]
fn un_artiste_engage_moins_de_cinq_jours_a_douze_fois_le_plafond_horaire_par_journee() {
    // Deburau, deux cachets de 600,00 les 20 et 21/10/2026 : 12 x 30 x 2 = 720,00 (et non 4 005 x 2 / 31). Brut 1 200,00 :
    // la vieillesse plafonnee sur 720,00, le FNAL sur 802,80 ; le chomage (4 x 720 = 2 880) reste sur le brut.
    // Clairon, un cachet de 1 500,00 le 22/10 : plafond 360,00, chomage dans la limite de 1 440,00.
    let ctx = Contexte::nouveau(&fictive("fictive-3090-grande"), Some(Jour::lire("2026-10-28").unwrap())).unwrap();
    let s = saisie(
        &ctx,
        json!({"mois": "2026-10", "date_paiement": "2026-10-31", "engagements": [
            {"salarie": "Deburau", "spectacle": "266Z00910022", "debut": "2026-10-20", "fin": "2026-10-21", "numero": "DE1.26.10",
             "date_paiement": "2026-10-21",
             "lignes": [{"rubrique": "cachet", "quantite": 2, "taux": 600.0, "lieu": "Theatre fictif", "dates": ["2026-10-20", "2026-10-21"]}]},
            {"salarie": "Clairon", "spectacle": "266Z00910022", "debut": "2026-10-22", "fin": "2026-10-22", "numero": "CL1.26.10",
             "date_paiement": "2026-10-22",
             "lignes": [{"rubrique": "cachet", "quantite": 1, "taux": 1500.0, "lieu": "Theatre fictif", "dates": ["2026-10-22"]}]}
        ]}),
    );
    let dsn = generer(&ctx, &s.paies, "2026-10", &[], &[], &Options::default()).unwrap();

    let p = paie(&s, "DE1.26.10");
    assert_eq!(p.plafond_urssaf(&ctx).unwrap(), dec("720.00"));
    assert_eq!(ligne(p, "vieil_plaf").0, dec("720.00"));
    assert_eq!(ligne(p, "autres_2").0, dec("802.80"));
    assert_eq!(ligne(p, "chomage").0, dec("1200.00"));
    let v = versement(&dsn, "DE1.26.10");
    assert_eq!(base78(&v, "02"), vec![dec("720.00")]);
    assert_eq!(base78(&v, "07"), vec![dec("1200.00")]);

    let q = paie(&s, "CL1.26.10");
    assert_eq!(q.plafond_urssaf(&ctx).unwrap(), dec("360.00"));
    assert_eq!(ligne(q, "vieil_plaf").0, dec("360.00"));
    let (b, pat, sal) = ligne(q, "chomage");
    let (tp, ts) = taux(q, "chomage");
    assert_eq!(b, dec("1440.00"), "le chomage dans la limite de quatre fois le plafond");
    assert_eq!((pat, sal), (pct(b, tp), pct(b, ts.unwrap())));
    let w = versement(&dsn, "CL1.26.10");
    assert_eq!(base78(&w, "02"), vec![dec("360.00")]);
    assert_eq!(base78(&w, "03"), vec![dec("1500.00")]);
    assert_eq!(base78(&w, "07"), vec![dec("1440.00")]);
    for code in ["040", "048", "041"] {
        assert_eq!(base81(&w, code), dec("1440.00"), "81.003 de la contribution {}", code);
    }
    // Le bordereau : 312/921 = 720 + 360, 312/920 = 1 200 + 1 500.
    assert_eq!(ctp(&dsn, "312", "921"), Some(dec("1080.00")));
    assert_eq!(ctp(&dsn, "312", "920"), Some(dec("2700.00")));
}

#[test]
fn un_technicien_entre_en_cours_de_mois_a_le_plafond_au_prorata_de_ses_jours() {
    // Chaminade, technicien a l'heure du 20 au 31/10/2026 : 120 h a 30,00 = 3 600,00 ; plafond 4 005 x 12 / 31 = 1 550,32.
    let ctx = Contexte::nouveau(&fictive("fictive-3090-grande"), Some(Jour::lire("2026-10-28").unwrap())).unwrap();
    let dates: Vec<String> = (20..=31).map(|d| format!("2026-10-{}", d)).collect();
    let s = saisie(
        &ctx,
        json!({"mois": "2026-10", "date_paiement": "2026-10-31", "engagements": [
            {"salarie": "Chaminade", "spectacle": "266Z00910022", "debut": "2026-10-20", "fin": "2026-10-31", "numero": "CH1.26.10",
             "date_paiement": "2026-10-31",
             "lignes": [{"rubrique": "heure", "quantite": 120, "taux": 30.0, "lieu": "Theatre fictif", "dates": dates}]}
        ]}),
    );
    let dsn = generer(&ctx, &s.paies, "2026-10", &[], &[], &Options::default()).unwrap();
    let p = paie(&s, "CH1.26.10");
    assert_eq!(p.brut, dec("3600.00"));
    assert_eq!(p.plafond_urssaf(&ctx).unwrap(), dec("1550.32"));
    assert_eq!(ligne(p, "vieil_plaf").0, dec("1550.32"));
    assert_eq!(ligne(p, "autres_2").0, eur(dec("1550.32") * dec("1.115")));
    assert_eq!(ligne(p, "chomage").0, dec("3600.00"));
    let v = versement(&dsn, "CH1.26.10");
    assert_eq!(base78(&v, "02"), vec![dec("1550.32")]);
    assert_eq!(ctp(&dsn, "100", "921"), Some(dec("1550.00")));
    assert_eq!(ctp(&dsn, "100", "920"), Some(dec("3600.00")));
    assert_eq!(ctp(&dsn, "332", "921"), Some(a_l_euro(eur(dec("1550.32") * dec("1.115")))));
}

#[test]
fn un_permanent_garde_sa_tranche_1_et_son_chomage_sur_le_brut() {
    // Montansier (CDI) a 5 000,00 en octobre 2026 : vieillesse plafonnee et FNAL sur la tranche 1 (4 005,00), chomage
    // sur le brut, qui est sous quatre plafonds (16 020,00) ; aucune assiette d'intermittent dans ses bases. Lot V2e :
    // `brut_chomage` est desormais pose pour un permanent aussi (quatre plafonds de la periode), et vaut ici le brut ;
    // le config/organismes.json fige de la fictive, compose avant le lot (772 et 937 sur `brut_permanent`), reste juste.
    let ctx = Contexte::nouveau(&fictive("fictive-3090-grande"), Some(Jour::lire("2026-10-28").unwrap())).unwrap();
    let s = saisie(
        &ctx,
        json!({"mois": "2026-10", "date_paiement": "2026-10-31", "engagements": [
            {"salarie": "Montansier", "nature": "cdi", "debut": "2023-09-04", "numero": "MO.23.01", "quotite_contrat": 151.67,
             "lignes": [{"rubrique": "mois", "quantite": 1, "taux": 5000.0, "heures": 151.67}]}
        ]}),
    );
    let dsn = generer(&ctx, &s.paies, "2026-10", &[], &[], &Options::default()).unwrap();
    let p = paie(&s, "MO.23.01");
    assert_eq!(p.bases["t1_mensuelle"], dec("4005.00"));
    assert_eq!(ligne(p, "vieil_plaf").0, dec("4005.00"));
    assert_eq!(ligne(p, "autres_2").0, dec("4005.00"));
    assert_eq!(ligne(p, "chomage").0, dec("5000.00"));
    assert_eq!(p.bases["brut_chomage"], dec("5000.00"));
    for cle in ["brut_plafonne", "brut_x115_plafonne"] {
        assert!(!p.bases.contains_key(cle), "{} : pas d'assiette d'intermittent pour un permanent", cle);
    }
    let v = versement(&dsn, "MO.23.01");
    assert_eq!(base78(&v, "02"), vec![dec("4005.00")]);
    assert_eq!(base78(&v, "07"), vec![dec("5000.00")]);
    assert_eq!(ctp(&dsn, "100", "921"), Some(dec("4005.00")));
    assert_eq!(ctp(&dsn, "100", "920"), Some(dec("5000.00")));
    assert_eq!(ctp(&dsn, "772", "920"), Some(dec("5000.00")));
    assert_eq!(ctp(&dsn, "937", "920"), Some(dec("5000.00")));
}

// ------------------------------------------------------------------------------------------------------------------
// Lot V2e : le chomage d'un permanent dans la limite de quatre plafonds, au bulletin, en DSN et au bordereau.
//
// Textes (atelier/reference/sources/chomage-permanents/LISEZ-MOI.md) : C. trav. L. 5422-9 (« dans la limite d'un
// plafond »), reglement general annexe a la convention d'assurance chomage du 15/11/2024, art. 49, al. 2 (« quatre fois
// le plafond mentionne a l'article L. 241-3 »), L. 3253-18 (l'AGS sur la meme assiette), CSS R. 242-2, al. 3 (le plafond
// reduit au prorata des jours d'emploi) ; tableau d'equivalence de l'Urssaf : 772 -> base 07 / 040, 937 -> base 07 /
// 048, qualifiant 920.

/// Une copie de la fictive (sans ses PDF) dans le dossier du test, son config/organismes.json recompose depuis
/// `donnees/organismes-base.json` comme le fait l'ecran (Configuration › Organismes et contrats, Enregistrer).
fn fictive_recomposee(nom: &str, cible: &str) -> PathBuf {
    let racine = sans_les_pdf(nom, &dossier_de_test("plafond_intermittents", cible));
    let ctx = Contexte::nouveau(&racine, Some(Jour::lire("2026-10-28").unwrap())).unwrap();
    paie::organismes::enregistrer(&ctx, &json!({"contrats": {}})).unwrap();
    racine
}

fn montansier(ctx: &Contexte, debut: &str, numero: &str, brut: f64) -> Saisie {
    montansier_en(ctx, "2026-10", debut, numero, brut)
}

/// Montansier (CDI) payee `brut` sur le mois `mois` (AAAA-MM), payee le 31.
fn montansier_en(ctx: &Contexte, mois: &str, debut: &str, numero: &str, brut: f64) -> Saisie {
    saisie(
        ctx,
        json!({"mois": mois, "date_paiement": format!("{}-31", mois), "engagements": [
            {"salarie": "Montansier", "nature": "cdi", "debut": debut, "numero": numero, "quotite_contrat": 151.67,
             "lignes": [{"rubrique": "mois", "quantite": 1, "taux": brut, "heures": 151.67}]}
        ]}),
    )
}

/// La ligne `code`/`qualifiant` du bordereau Urssaf de la DSN, telle que `bordereau::montant_ligne` la valorise.
fn montant_ctp(dsn: &str, code: &str, qualifiant: &str) -> Decimal {
    montant_ctp_au(dsn, code, qualifiant, "2026-10-01")
}

/// De meme, au taux de la table des CTP en vigueur le `jour`.
fn montant_ctp_au(dsn: &str, code: &str, qualifiant: &str, jour: &str) -> Decimal {
    let assiette = ctp(dsn, code, qualifiant).unwrap_or_else(|| panic!("pas de CTP {}/{}", code, qualifiant));
    let l = paie::bordereau::LigneBordereau {
        code: code.into(),
        qualifiant: qualifiant.into(),
        taux_at: String::new(),
        assiette: Some(assiette),
        montant: None,
        commune: None,
    };
    paie::bordereau::montant_ligne(&l, &Jour::lire(jour).unwrap()).unwrap()
}

/// La fictive recomposee a soixante salaries depuis cinq ans (config/effectifs.json), et sa surcouche ecrit ce que
/// `seuils_effectif` du socle demande a ce seuil : le FNAL sur le brut majore entier (0,35 % pour un artiste, 0,50 %
/// sinon), le `tdelta` de la reduction generale, la formation des permanents a 1 %, le versement mobilite « aucun ».
fn fictive_a_soixante(cible: &str) -> PathBuf {
    let racine = fictive_recomposee("fictive-3090-grande", cible);
    let lire = |rel: &str| -> Value { serde_json::from_str(&std::fs::read_to_string(racine.join(rel)).unwrap()).unwrap() };
    let mut eff = lire("config/effectifs.json");
    for an in eff["annees"].as_object_mut().unwrap().values_mut() {
        an["A01"]["valeur"] = json!(60.0);
    }
    std::fs::write(racine.join("config/effectifs.json"), eff.to_string()).unwrap();
    let mut bar = lire("config/baremes/2026.json");
    let fnal = |taux: f64| json!([{"bloc78": "02", "code81": "049", "taux": taux, "base": "brut_x115"}]);
    let rubrique = |taux: f64| json!([{"id": "autres_2", "taux_pat": taux, "base": "brut_x115"}]);
    bar["rubriques_bulletin"] = rubrique(0.35);
    bar["cotisations_dsn"] = fnal(0.35);
    bar["par_statut"] = json!({"technicien": {"rubriques_bulletin": rubrique(0.5), "cotisations_dsn": fnal(0.5), "reduction_generale": {"tdelta": 0.3821}}});
    bar["par_nature"] = json!({"permanent": {
        "cotisations_dsn": [{"bloc78": "03", "code81": "128", "taux": 1.0}, {"bloc78": "02", "code81": "049", "taux": 0.5, "base": "brut_x115"}],
        "rubriques": {"autres_2": {"taux_pat": 0.5, "base": "brut_x115"}},
        "rubriques_ajoutees": [{"id": "autres_4", "taux_pat": 1.0, "composants": [1.0]}]}});
    bar["versement_mobilite"] = json!("aucun");
    std::fs::write(racine.join("config/baremes/2026.json"), bar.to_string()).unwrap();
    racine
}

#[test]
fn a_cinquante_salaries_le_fnal_des_artistes_se_declare_sous_le_ctp_316_sur_la_totalite() {
    // Lot R2 : « Fnal : Effectif de moins de 50 salaries : 0,07 % dans la limite du plafond ; Effectif 50 salaries et
    // plus : 0,35 % » ; « CTP 316 : FNAL artistes totalite ; CTP 334 : FNAL artistes plafonne » (Urssaf, « Les taux
    // reduits cas particuliers », atelier/reference/sources/urssaf-taux-reduits-artistes/, l. 89-92 et 148-149 du .txt).
    // Juillet 2026 de la fictive 3090-grande a soixante salaries : le 316 sous 920 porte le brut majore de 11,5 % des
    // artistes, entier, comme leur 81.003 du 049 ; plus de 334.
    let racine = fictive_a_soixante("fnal-artistes-cinquante");
    let ctx = Contexte::nouveau(&racine, Some(Jour::lire("2026-07-28").unwrap())).unwrap();
    let s = charger(&ctx, &racine.join("paies/2026-07.json")).unwrap();
    let dsn = generer(&ctx, &s.paies, "2026-07", &[], &[], &Options::default()).unwrap();
    let artistes: Vec<&Paie> = s.paies.iter().filter(|p| !p.engagement.nat().unwrap().mensuelle() && p.salarie["statut"] == "artiste").collect();
    assert!(!artistes.is_empty());
    let x115: Decimal = artistes.iter().map(|p| p.bases["brut_x115"]).sum();
    assert!(artistes.iter().any(|p| p.bases["brut_x115"] > eur(p.plafond_urssaf(&ctx).unwrap() * dec("1.115"))), "le cas eprouve la totalite");
    for p in &artistes {
        let v = versement(&dsn, p.engagement.numero.as_deref().unwrap());
        assert_eq!(base81(&v, "049"), p.bases["brut_x115"], "le nominatif du FNAL de l'artiste, sur la totalite");
    }
    assert_eq!(ctp(&dsn, "316", "920"), Some(a_l_euro(x115)));
    assert_eq!(ctp(&dsn, "334", "921"), None, "plus de FNAL plafonne des artistes");

    // Sous cinquante (la fictive recomposee telle quelle) : le 334, pas de 316.
    let racine = fictive_recomposee("fictive-3090-grande", "fnal-artistes-sous-cinquante");
    let ctx = Contexte::nouveau(&racine, Some(Jour::lire("2026-07-28").unwrap())).unwrap();
    let s = charger(&ctx, &racine.join("paies/2026-07.json")).unwrap();
    let dsn = generer(&ctx, &s.paies, "2026-07", &[], &[], &Options::default()).unwrap();
    assert!(ctp(&dsn, "334", "921").is_some());
    assert_eq!(ctp(&dsn, "316", "920"), None);
}

#[test]
fn un_permanent_au_dessus_de_quatre_plafonds_cotise_le_chomage_sur_quatre_plafonds() {
    // Montansier (CDI) a 20 000,00 sur tout octobre 2026 : quatre plafonds = 4 x 4 005,00 = 16 020,00. Le chomage (4,00)
    // et l'AGS (0,25) du bulletin, la base 07 et les 81.003 de 040 et 048 de la DSN, les CTP 772 et 937 du bordereau :
    // tous sur 16 020,00. Le reste ne bouge pas : 100/920 sur le brut, 100/921 sur la tranche 1.
    let racine = fictive_recomposee("fictive-3090-grande", "chomage-permanent-20000");
    let ctx = Contexte::nouveau(&racine, Some(Jour::lire("2026-10-28").unwrap())).unwrap();
    let s = montansier(&ctx, "2023-09-04", "MO.23.01", 20000.0);
    let dsn = generer(&ctx, &s.paies, "2026-10", &[], &[], &Options::default()).unwrap();
    let p = paie(&s, "MO.23.01");
    let quatre = dec("16020.00");
    assert_eq!(p.brut, dec("20000.00"));
    assert_eq!(p.bases["pss_periode"], dec("4005.00"));
    assert_eq!(p.bases["brut_chomage"], quatre);
    let (base, pat, sal) = ligne(p, "chomage");
    assert_eq!(base, quatre);
    assert_eq!(pat, pct(quatre, dec("4.00")) + pct(quatre, dec("0.25")), "4,00 + 0,25, arrondis separement");
    assert_eq!(pat, dec("680.85"));
    assert_eq!(sal, dec("0.00"));
    // La DSN : la base 07, et les cotisations 040 et 048 dessus.
    let v = versement(&dsn, "MO.23.01");
    assert_eq!(base78(&v, "07"), vec![quatre]);
    assert_eq!(base81(&v, "040"), quatre);
    assert_eq!(base81(&v, "048"), quatre);
    assert_eq!(base78(&v, "03"), vec![dec("20000.00")]);
    assert_eq!(base78(&v, "02"), vec![dec("4005.00")]);
    // Le bordereau : 772 et 937 sur la meme assiette, et leur montant est celui du bulletin.
    assert_eq!(ctp(&dsn, "772", "920"), Some(quatre));
    assert_eq!(ctp(&dsn, "937", "920"), Some(quatre));
    assert_eq!(ctp(&dsn, "100", "920"), Some(dec("20000.00")));
    assert_eq!(ctp(&dsn, "100", "921"), Some(dec("4005.00")));
    assert_eq!(montant_ctp(&dsn, "772", "920") + montant_ctp(&dsn, "937", "920"), dec("681.00"));
    // L'Apec d'un cadre s'assied deja sur quatre plafonds de la periode : la meme valeur, posee au meme endroit.
    assert_eq!(p.bases["apec_mensuelle"], quatre);
}

#[test]
fn un_permanent_entre_en_cours_de_mois_a_quatre_plafonds_au_prorata_de_ses_jours() {
    // Entre le 16/10/2026 : 16 jours sur 31, plafond de la periode 4 005 x 16 / 31 = 2 067,10 (CSS R. 242-2, al. 3) ;
    // quatre plafonds 8 268,40. Avec 10 000,00 de brut, le chomage s'assied sur 8 268,40, partout.
    let racine = fictive_recomposee("fictive-3090-grande", "chomage-permanent-entree");
    let ctx = Contexte::nouveau(&racine, Some(Jour::lire("2026-10-28").unwrap())).unwrap();
    let s = montansier(&ctx, "2026-10-16", "MO.26.10", 10000.0);
    let dsn = generer(&ctx, &s.paies, "2026-10", &[], &[], &Options::default()).unwrap();
    let p = paie(&s, "MO.26.10");
    let quatre = dec("8268.40");
    assert_eq!(p.bases["pss_periode"], dec("2067.10"));
    assert_eq!(ligne(p, "chomage").0, quatre);
    let v = versement(&dsn, "MO.26.10");
    assert_eq!(base78(&v, "07"), vec![quatre]);
    assert_eq!(base81(&v, "040"), quatre);
    assert_eq!(ctp(&dsn, "772", "920"), Some(a_l_euro(quatre)));
    assert_eq!(ctp(&dsn, "937", "920"), Some(a_l_euro(quatre)));
}

// ------------------------------------------------------------------------------------------------------------------
// Lot R8 (#117) : la meme regle au bareme 2025. Le reglement general annexe a la convention du 15/11/2024 fixe « la
// reglementation d'assurance chomage applicable a compter du 1er janvier 2025, relative tant a l'indemnisation qu'aux
// contributions » (circulaire Unedic 2025-05, l. 33-37 du .txt) ; sa fiche technique 1.1 : « Les remunerations
// depassant 4 fois le plafond [...] demeurent exclues de l'assiette des contributions (15 700 EUR par mois, valeur au
// 1er janvier 2025) » (l. 109-117). Textes : atelier/reference/sources/chomage-permanents/LISEZ-MOI.md.

#[test]
fn en_2025_un_permanent_au_dessus_de_quatre_plafonds_cotise_le_chomage_sur_quatre_plafonds() {
    // Montansier (CDI) a 20 000,00 sur tout octobre 2025 : PSS 3 925,00, quatre plafonds = 15 700,00. Le chomage (4,00
    // depuis le 01/05/2025) et l'AGS (0,25) du bulletin, la base 07 et les 81.003 de 040 et 048, les CTP 772 et 937 :
    // tous sur 15 700,00 ; 100/920 sur le brut, 100/921 sur la tranche 1.
    let racine = fictive_recomposee("fictive-3090-grande", "chomage-permanent-2025-20000");
    let ctx = Contexte::nouveau(&racine, Some(Jour::lire("2025-10-28").unwrap())).unwrap();
    let s = montansier_en(&ctx, "2025-10", "2023-09-04", "MO.23.01", 20000.0);
    let dsn = generer(&ctx, &s.paies, "2025-10", &[], &[], &Options::default()).unwrap();
    let p = paie(&s, "MO.23.01");
    let quatre = dec("15700.00");
    assert_eq!(p.brut, dec("20000.00"));
    assert_eq!(p.bases["pss_periode"], dec("3925.00"));
    assert_eq!(p.bases["brut_chomage"], quatre);
    let (base, pat, sal) = ligne(p, "chomage");
    assert_eq!(base, quatre);
    assert_eq!(taux(p, "chomage").0, dec("4.25"));
    assert_eq!(pat, pct(quatre, dec("4.00")) + pct(quatre, dec("0.25")), "4,00 + 0,25, arrondis separement");
    assert_eq!(pat, dec("667.25"));
    assert_eq!(sal, dec("0.00"));
    let v = versement(&dsn, "MO.23.01");
    assert_eq!(base78(&v, "07"), vec![quatre]);
    assert_eq!(base81(&v, "040"), quatre);
    assert_eq!(base81(&v, "048"), quatre);
    assert_eq!(base78(&v, "03"), vec![dec("20000.00")]);
    assert_eq!(base78(&v, "02"), vec![dec("3925.00")]);
    assert_eq!(ctp(&dsn, "772", "920"), Some(quatre));
    assert_eq!(ctp(&dsn, "937", "920"), Some(quatre));
    assert_eq!(ctp(&dsn, "100", "920"), Some(dec("20000.00")));
    assert_eq!(ctp(&dsn, "100", "921"), Some(dec("3925.00")));
    assert_eq!(montant_ctp_au(&dsn, "772", "920", "2025-10-01") + montant_ctp_au(&dsn, "937", "920", "2025-10-01"), dec("667.00"));
}

