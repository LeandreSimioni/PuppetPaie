//! Le plafond d'un intermittent, au niveau de la paie, suit le texte : il se retient pour chaque paie, sur les jours
//! remuneres de la paie (BOSS, assiette generale, § 1090, tolerance appliquee :
//! atelier/reference/sources/boss/boss-assiette-generale-20260801.txt, l. 1472-1483).
//!
//! - Arrete du 24/01/1975, art. 3, al. 2 (atelier/reference/sources/arrete-1975-artistes-du-spectacle/) : « Pour les
//!   periodes d'engagement continu inferieures a cinq jours, le plafond [...] applicable a chaque journee de travail
//!   accomplie par un artiste du spectacle pour un meme employeur est egal a douze fois le plafond horaire, quels que
//!   soient le nombre d'heures et la nature du travail effectues dans ladite journee. »
//! - Sinon, et pour un technicien (l'arrete ne vise que les artistes, art. 1), CSS R. 242-2 : le plafond reduit « a due
//!   proportion du nombre de jours de la periode pendant laquelle les personnes sont employees »
//!   (atelier/reference/sources/code-de-la-securite-sociale/articles-20261010.md).
//!
//! Plafond 2026 : 4 005 par mois, 30 de l'heure ; 2025 : 3 925 et 29 (`logiciel/donnees/baremes/`). Sur la copie figee
//! des compagnies fictives (`tests/donnees/fictives/`, compagnies inventees), lue sans y rien ecrire : les saisies s'ecrivent dans
//! le dossier temporaire des tests.

use paie::calcul::Paie;
use droit::contexte::Contexte;
use socle::date::Jour;
use paie::dsn::{generer, Options};
use paie::engagement::{charger, Saisie};
use socle::num::{eur, Decimal, Resultat};
use serde_json::{json, Value};
use crate::outils::{dec, fictive, versement};

/// Les 78.004 du bloc `code` du versement du contrat `numero`.
fn base78(dsn: &str, numero: &str, code: &str) -> Vec<Decimal> {
    crate::outils::base78(&versement(dsn, numero), code)
}

fn ctx(date: &str) -> Contexte {
    Contexte::nouveau(&fictive("fictive-3090-grande"), Some(Jour::lire(date).unwrap())).unwrap()
}

fn essayer(ctx: &Contexte, s: Value) -> Resultat<Saisie> {
    let chemin = crate::outils::chemin_unique("plafond-texte.json");
    std::fs::write(&chemin, s.to_string()).unwrap();
    charger(ctx, &chemin)
}

/// Une saisie d'octobre 2026 a un seul engagement.
fn octobre(ctx: &Contexte, e: Value) -> Saisie {
    essayer(ctx, json!({"mois": "2026-10", "date_paiement": "2026-10-31", "engagements": [e]})).unwrap()
}

fn seule(s: &Saisie) -> &Paie {
    assert_eq!(s.paies.len(), 1);
    &s.paies[0]
}

fn cachets(n: u32, taux: f64, dates: &[&str]) -> Value {
    json!({"rubrique": "cachet", "quantite": n, "taux": taux, "lieu": "Theatre fictif", "dates": dates})
}

fn base(p: &Paie, id: &str) -> Decimal {
    p.lignes.iter().find(|l| l.id == id).unwrap_or_else(|| panic!("pas de ligne {} au bulletin", id)).base
}

#[test]
fn deux_cachets_le_meme_jour_font_une_journee() {
    // Deburau, deux cachets de 600,00 le 20/10/2026, contrat d'un jour : une journee de travail, 12 x 30 = 360,00
    // (« quels que soient le nombre d'heures et la nature du travail effectues dans ladite journee »). Avant : 720,00.
    let c = ctx("2026-10-28");
    let s = octobre(&c, json!({"salarie": "Deburau", "spectacle": "266Z00910022", "debut": "2026-10-20", "fin": "2026-10-20",
        "numero": "DE1.26.10", "date_paiement": "2026-10-20", "lignes": [cachets(2, 600.0, &["2026-10-20"])]}));
    let p = seule(&s);
    assert_eq!(p.plafond_urssaf(&c).unwrap(), dec("360.00"));
    assert_eq!(base(p, "vieil_plaf"), dec("360.00"));
    assert_eq!(base(p, "chomage"), dec("1200.00"), "4 x 360 = 1 440 : le chomage reste sur le brut");
    let dsn = generer(&c, &s.paies, "2026-10", &[], &[], &Options::default()).unwrap();
    assert_eq!(base78(&dsn, "DE1.26.10", "02"), vec![dec("360.00")]);
}

#[test]
fn deux_repetitions_et_une_representation_sur_trois_jours() {
    // Deburau du 20 au 22/10/2026 : deux services de repetition (20 et 21) et un cachet (22). Trois journees de travail
    // accomplies, repetitions comprises : 3 x 360 = 1 080,00. Avant : un seul cachet compte, max(4 005 x 3 / 31, 360)
    // = 387,58.
    let c = ctx("2026-10-28");
    let s = octobre(&c, json!({"salarie": "Deburau", "spectacle": "266Z00910022", "debut": "2026-10-20", "fin": "2026-10-22",
        "numero": "DE2.26.10", "date_paiement": "2026-10-22", "lignes": [
            {"rubrique": "repetition", "quantite": 2, "taux": 150.0, "heures": 4, "lieu": "Theatre fictif", "dates": ["2026-10-20", "2026-10-21"]},
            cachets(1, 1200.0, &["2026-10-22"])]}));
    let p = seule(&s);
    assert_eq!(p.brut, dec("1500.00"));
    assert_eq!(p.plafond_urssaf(&c).unwrap(), dec("1080.00"));
    assert_eq!(base(p, "vieil_plaf"), dec("1080.00"));
    assert_eq!(base(p, "autres_2"), eur(dec("1080.00") * dec("1.115")));
}

#[test]
fn un_contrat_de_quatre_jours_un_jour_travaille() {
    // Deburau engage du 20 au 23/10/2026 (quatre jours), un cachet de 800,00 le 21 : une journee, 360,00, « est egal »
    // et non un plancher. Avant : max(4 005 x 4 / 31, 360) = 516,77.
    let c = ctx("2026-10-28");
    let s = octobre(&c, json!({"salarie": "Deburau", "spectacle": "266Z00910022", "debut": "2026-10-20", "fin": "2026-10-23",
        "numero": "DE3.26.10", "date_paiement": "2026-10-23", "lignes": [cachets(1, 800.0, &["2026-10-21"])]}));
    let p = seule(&s);
    assert_eq!(p.plafond_urssaf(&c).unwrap(), dec("360.00"));
    assert_eq!(base(p, "vieil_plaf"), dec("360.00"));
}

#[test]
fn un_contrat_de_sept_jours_quatre_cachets_a_le_prorata_des_jours_remuneres() {
    // Deburau engage du 19 au 25/10/2026 (sept jours), quatre cachets de 500,00 un jour sur deux : la periode
    // d'engagement continu n'est pas inferieure a cinq jours, l'arrete ne s'applique pas ; BOSS § 1090, les jours ayant
    // donne lieu a remuneration : 4 005 x 4 / 31 = 516,77. Vague 1l : R. 242-2 sur les sept jours du contrat, 904,35 ;
    // avant : « cachets isoles », max(904,35, 4 x 360) = 1 440,00.
    let c = ctx("2026-10-28");
    let s = octobre(&c, json!({"salarie": "Deburau", "spectacle": "266Z00910022", "debut": "2026-10-19", "fin": "2026-10-25",
        "numero": "DE4.26.10", "date_paiement": "2026-10-25",
        "lignes": [cachets(4, 500.0, &["2026-10-19", "2026-10-21", "2026-10-23", "2026-10-25"])]}));
    let p = seule(&s);
    assert_eq!(p.plafond_urssaf(&c).unwrap(), dec("516.77"));
    assert_eq!(base(p, "vieil_plaf"), dec("516.77"));
    assert_eq!(base(p, "chomage"), dec("2000.00"));
}

#[test]
fn un_contrat_de_cinq_jours_pile_a_le_prorata_des_jours_remuneres() {
    // Deburau du 20 au 24/10/2026, deux cachets de 600,00 les 20 et 21 : cinq jours ne sont pas « inferieurs a cinq
    // jours ». BOSS § 1090 : 4 005 x 2 / 31 = 258,39. Vague 1l : les cinq jours du contrat, 645,97 ; avant : max(645,97,
    // 720) = 720,00.
    let c = ctx("2026-10-28");
    let s = octobre(&c, json!({"salarie": "Deburau", "spectacle": "266Z00910022", "debut": "2026-10-20", "fin": "2026-10-24",
        "numero": "DE5.26.10", "date_paiement": "2026-10-24", "lignes": [cachets(2, 600.0, &["2026-10-20", "2026-10-21"])]}));
    let p = seule(&s);
    assert_eq!(p.plafond_urssaf(&c).unwrap(), dec("258.39"));
    assert_eq!(base(p, "vieil_plaf"), dec("258.39"));
    // Quatre jours, les memes cachets : l'arrete, deux journees.
    let s = octobre(&c, json!({"salarie": "Deburau", "spectacle": "266Z00910022", "debut": "2026-10-20", "fin": "2026-10-23",
        "numero": "DE6.26.10", "date_paiement": "2026-10-23", "lignes": [cachets(2, 600.0, &["2026-10-20", "2026-10-21"])]}));
    assert_eq!(seule(&s).plafond_urssaf(&c).unwrap(), dec("720.00"));
}

#[test]
fn un_technicien_engage_moins_de_cinq_jours_a_le_prorata() {
    // Chaminade, technicien, du 20 au 21/10/2026, 20 h a 40,00 : l'arrete ne vise que les artistes ; BOSS § 1090, deux
    // jours remuneres, 4 005 x 2 / 31 = 258,39 (vague 1l : les deux jours du contrat, le meme montant).
    let c = ctx("2026-10-28");
    let s = octobre(&c, json!({"salarie": "Chaminade", "spectacle": "266Z00910022", "debut": "2026-10-20", "fin": "2026-10-21",
        "numero": "CH1.26.10", "date_paiement": "2026-10-21",
        "lignes": [{"rubrique": "heure", "quantite": 20, "taux": 40.0, "lieu": "Theatre fictif", "dates": ["2026-10-20", "2026-10-21"]}]}));
    let p = seule(&s);
    assert_eq!(p.salarie["statut"], "technicien");
    assert_eq!(p.plafond_urssaf(&c).unwrap(), dec("258.39"));
    assert_eq!(base(p, "vieil_plaf"), dec("258.39"));
    assert_eq!(base(p, "chomage"), dec("800.00"));
    // Le meme technicien paye au cachet, un jour : l'arrete ne le vise toujours pas, 4 005 / 31 = 129,19. Avant : le
    // plancher des cachets s'appliquait a tout intermittent, 360,00.
    let s = octobre(&c, json!({"salarie": "Chaminade", "spectacle": "266Z00910022", "debut": "2026-10-20", "fin": "2026-10-20",
        "numero": "CH2.26.10", "date_paiement": "2026-10-20", "lignes": [cachets(1, 500.0, &["2026-10-20"])]}));
    let p = seule(&s);
    assert_eq!(p.plafond_urssaf(&c).unwrap(), dec("129.19"));
    assert_eq!(base(p, "vieil_plaf"), dec("129.19"));
}

#[test]
fn une_journee_dite_et_non_datee_refuse() {
    // Deburau, contrat de deux jours, `jours` : 2, une seule date : la seconde journee ne se lit pas, elle ne se devine
    // pas. Avant : 2 cachets x 360 = 720,00, sans rien demander.
    let c = ctx("2026-10-28");
    let err = essayer(&c, json!({"mois": "2026-10", "date_paiement": "2026-10-31", "engagements": [
        {"salarie": "Deburau", "spectacle": "266Z00910022", "debut": "2026-10-20", "fin": "2026-10-21", "numero": "DE7.26.10",
         "date_paiement": "2026-10-21", "jours": 2, "lignes": [cachets(2, 600.0, &["2026-10-20"])]}]}))
    .err()
    .expect("une journee non datee refuse");
    assert!(err.contains("arrete du 24/01/1975") && err.contains("2 jour(s) travaille(s) pour 1 date(s)"), "{}", err);
}

#[test]
fn un_mois_2025_au_dessus_du_plafond_cotise_sur_le_plafond() {
    // Barème 2025 : Deburau, un cachet de 1 500,00 le 07/10/2025, contrat d'un jour : plafond 12 x 29 = 348,00. La
    // vieillesse plafonnee et la base 02 sur 348,00, le FNAL sur 348 x 1,115 = 388,02, le chomage et la base 07 dans la
    // limite de 4 x 348 = 1 392,00. Avant : le barème 2025 assied tout sur le brut, 1 500,00.
    let c = ctx("2025-10-28");
    let s = essayer(&c, json!({"mois": "2025-10", "date_paiement": "2025-10-31", "engagements": [
        {"salarie": "Deburau", "spectacle": "256Z00910011", "debut": "2025-10-07", "fin": "2025-10-07", "numero": "DE9.25.10",
         "date_paiement": "2025-10-07", "lignes": [cachets(1, 1500.0, &["2025-10-07"])]}]})).unwrap();
    let p = seule(&s);
    assert_eq!(p.plafond_urssaf(&c).unwrap(), dec("348.00"));
    assert_eq!(base(p, "vieil_plaf"), dec("348.00"));
    assert_eq!(base(p, "autres_2"), dec("388.02"));
    assert_eq!(base(p, "chomage"), dec("1392.00"));
    assert_eq!(base(p, "vieil_deplaf"), dec("1500.00"));
    let dsn = generer(&c, &s.paies, "2025-10", &[], &[], &Options::default()).unwrap();
    assert_eq!(base78(&dsn, "DE9.25.10", "02"), vec![dec("348.00")]);
    assert_eq!(base78(&dsn, "DE9.25.10", "03"), vec![dec("1500.00")]);
    assert_eq!(base78(&dsn, "DE9.25.10", "07"), vec![dec("1392.00")]);
}

/// Une saisie d'un mois a un seul engagement.
fn du_mois(ctx: &Contexte, mois: &str, paiement: &str, e: Value) -> Saisie {
    essayer(ctx, json!({"mois": mois, "date_paiement": paiement, "engagements": [e]})).unwrap()
}

#[test]
fn un_contrat_sur_deux_mois_a_le_plafond_de_chaque_paie() {
    // Vague 1o. Deburau engage du 20/01 au 10/02/2026 (22 jours), trois cachets de 1 500,00 en janvier (20, 22, 24),
    // trois en fevrier (3, 5, 10). Le plafond est « retenu pour chaque paie » (R. 242-2), sur les jours ayant donne lieu
    // a remuneration (BOSS § 1090) : janvier 4 005 x 3 / 31 = 387,58 ; fevrier 4 005 x 3 / 28 = 429,11. Avant : chaque
    // paie recevait le prorata du contrat entier, 4 005 x 12 / 31 + 4 005 x 10 / 28 = 2 980,68, et rien n'etait plafonne.
    let contrat = |lignes: Value, paiement: &str| json!({"salarie": "Deburau", "spectacle": "266Z00910022",
        "debut": "2026-01-20", "fin": "2026-02-10", "numero": "DE1.26.01", "date_paiement": paiement, "lignes": lignes});
    let c = ctx("2026-01-31");
    let s = du_mois(&c, "2026-01", "2026-01-31", contrat(json!([cachets(3, 1500.0, &["2026-01-20", "2026-01-22", "2026-01-24"])]), "2026-01-31"));
    let p = seule(&s);
    assert_eq!(p.brut, dec("4500.00"));
    assert_eq!(p.plafond_urssaf(&c).unwrap(), dec("387.58"), "janvier : le bas du bulletin");
    assert_eq!(base(p, "vieil_plaf"), dec("387.58"));
    assert_eq!(base(p, "chomage"), dec("1550.32"), "4 x 387,58");
    let dsn = generer(&c, &s.paies, "2026-01", &[], &[], &Options::default()).unwrap();
    assert_eq!(base78(&dsn, "DE1.26.01", "02"), vec![dec("387.58")]);
    assert_eq!(base78(&dsn, "DE1.26.01", "07"), vec![dec("1550.32")]);

    let c = ctx("2026-02-28");
    let s = du_mois(&c, "2026-02", "2026-02-28", contrat(json!([cachets(3, 1500.0, &["2026-02-03", "2026-02-05", "2026-02-10"])]), "2026-02-28"));
    let p = seule(&s);
    assert_eq!(p.plafond_urssaf(&c).unwrap(), dec("429.11"), "fevrier : le bas du bulletin");
    assert_eq!(base(p, "vieil_plaf"), dec("429.11"));
    let dsn = generer(&c, &s.paies, "2026-02", &[], &[], &Options::default()).unwrap();
    assert_eq!(base78(&dsn, "DE1.26.01", "02"), vec![dec("429.11")]);
}

#[test]
fn un_technicien_engage_une_semaine_a_le_prorata_de_ses_jours_remuneres() {
    // Vague 1o, BOSS § 1090 : Chaminade, technicien, engage du 19 au 25/10/2026, deux journees de 10 h (20 et 22) : le
    // plafond des deux jours remuneres, 4 005 x 2 / 31 = 258,39. Vague 1l : les sept jours du contrat, 904,35.
    let c = ctx("2026-10-28");
    let s = octobre(&c, json!({"salarie": "Chaminade", "spectacle": "266Z00910022", "debut": "2026-10-19", "fin": "2026-10-25",
        "numero": "CH3.26.10", "date_paiement": "2026-10-25",
        "lignes": [{"rubrique": "heure", "quantite": 20, "taux": 60.0, "lieu": "Theatre fictif", "dates": ["2026-10-20", "2026-10-22"]}]}));
    let p = seule(&s);
    assert_eq!(p.plafond_urssaf(&c).unwrap(), dec("258.39"));
    assert_eq!(base(p, "vieil_plaf"), dec("258.39"));
}
