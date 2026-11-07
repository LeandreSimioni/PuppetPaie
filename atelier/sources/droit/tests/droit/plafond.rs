//! Le plafond d'un contrat, depuis le texte : arrete du 24/01/1975, art. 3, al. 2 (un artiste
//! engage moins de cinq jours continus : douze fois le plafond horaire par journee de travail accomplie) et, sinon,
//! CSS R. 242-2 (le plafond mensuel reduit a due proportion des jours du contrat). Hors de l'arrete, un
//! intermittent a le plafond proratise sur les jours ayant donne lieu a remuneration (BOSS, assiette generale, § 1090,
//! tolerance appliquee). Textes : atelier/reference/sources/arrete-1975-artistes-du-spectacle/,
//! code-de-la-securite-sociale/, boss/ (§ 680, 690, 700, 1090, 1100).
//!
//! Le moteur se confronte a un oracle ecrit depuis le texte (`oracle`, sans le moteur) sur une grille de 23 808 cas
//! (2025-2026, tous les mois, deux departs, durees 1 a 62 jours, 0, 1, 3 ou 6 cachets, artiste ou technicien), et a
//! `plafond-dates.json`, ecrit a la main depuis les memes textes.

use socle::num::Decimal;
use droit::plafond::{plafond_urssaf, pour_paie, EngagementPlafond, Jour};
use socle::num::Resultat;
use serde::Deserialize;
use std::collections::HashMap;
use crate::outils::typee::{chemin_donnee, code_livre, lire, lire_chemin};

fn dec(s: &str) -> Decimal {
    s.parse().unwrap()
}

/// `plafond-dates.json`, ecrit a la main depuis les textes : ses lignes sont des listes, lues par leur forme.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cas {
    #[serde(rename = "_note")]
    _note: String,
    /// debut, fin, statut, journees remunerees, plafond attendu, ce que le cas montre.
    dates: Vec<(String, String, String, Vec<String>, Decimal, String)>,
    /// debut, fin de la periode d'un permanent, plafond attendu.
    mensuel: Vec<(String, String, Decimal)>,
    /// debut, fin, statut, ce que le cas montre : un contrat sans journee remuneree datee.
    sans_date: Vec<(String, String, String, String)>,
}

/// Du bareme livre, la seule section que ce test lit : le plafond de la securite sociale.
#[derive(Deserialize)]
struct BaremePss {
    plafond_securite_sociale: Pss,
}

#[derive(Deserialize)]
struct Pss {
    mensuel: i64,
    horaire: i64,
}

/// Le plafond de chaque annee d'une table `{annee: (mensuel, horaire)}`.
fn pss_depuis_table(table: &HashMap<i32, (i64, i64)>) -> impl Fn(i32) -> Resultat<(Decimal, Decimal)> + '_ {
    move |annee| table.get(&annee).map(|(m, h)| (Decimal::from(*m), Decimal::from(*h))).ok_or_else(|| format!("pas de plafond pour {}", annee))
}

fn jour(s: &str) -> Jour {
    let p: Vec<&str> = s.split('-').collect();
    (p[0].parse().unwrap(), p[1].parse().unwrap(), p[2].parse().unwrap())
}

/// Le PSS : la section `plafond_securite_sociale` du bareme de l'annee (`donnees/baremes/AAAA.json`).
fn table() -> HashMap<i32, (i64, i64)> {
    let mut t = HashMap::new();
    for annee in [2025, 2026] {
        let b: BaremePss = lire(&code_livre(), &format!("baremes/{}.json", annee));
        t.insert(annee, (b.plafond_securite_sociale.mensuel, b.plafond_securite_sociale.horaire));
    }
    t
}

fn ajouter(j: Jour, n: u32) -> Jour {
    // n < 62 : au plus deux mois plus tard.
    let (mut a, mut m, mut d) = j;
    for _ in 0..n {
        let dim = match m {
            4 | 6 | 9 | 11 => 30,
            2 if (a % 4 == 0 && a % 100 != 0) || a % 400 == 0 => 29,
            2 => 28,
            _ => 31,
        };
        if d == dim {
            d = 1;
            if m == 12 {
                m = 1;
                a += 1;
            } else {
                m += 1;
            }
        } else {
            d += 1;
        }
    }
    (a, m, d)
}

/// Les `duree` premiers jours du contrat.
fn jours_du_contrat(debut: Jour, duree: u32) -> Vec<Jour> {
    (0..duree).map(|i| ajouter(debut, i)).collect()
}

/// L'oracle du texte, ecrit sans le moteur. `annee -> (mensuel, horaire)` : la table du bareme.
fn oracle(t: &HashMap<i32, (i64, i64)>, duree: u32, artiste: bool, journees: &[Jour]) -> Result<Decimal, ()> {
    let pss = |a: i32| t.get(&a).copied().ok_or(());
    if artiste && duree < 5 {
        if journees.is_empty() {
            return Err(());
        }
        // Arrete du 24/01/1975, art. 3, al. 2 : 12 x le plafond horaire par journee de travail accomplie.
        let mut total = Decimal::ZERO;
        for j in journees {
            total += Decimal::from(12 * pss(j.0)?.1);
        }
        return Ok(total.round_dp(2));
    }
    // BOSS § 1090 (tolerance) et § 700 : le plafond mensuel x jours remuneres dans le mois / jours du mois, mois par
    // mois. Sans jour remunere, rien a compter : refus.
    if journees.is_empty() {
        return Err(());
    }
    let mut par_mois: std::collections::BTreeMap<(i32, u32), i64> = Default::default();
    for j in journees.iter().collect::<std::collections::BTreeSet<_>>() {
        *par_mois.entry((j.0, j.1)).or_default() += 1;
    }
    let mut total = Decimal::ZERO;
    for ((a, m), n) in par_mois {
        let dim = match m {
            4 | 6 | 9 | 11 => 30,
            2 if (a % 4 == 0 && a % 100 != 0) || a % 400 == 0 => 29,
            2 => 28,
            _ => 31,
        };
        total += Decimal::from(pss(a)?.0) * Decimal::from(n) / Decimal::from(dim);
    }
    Ok(total.round_dp_with_strategy(2, rust_decimal::RoundingStrategy::MidpointAwayFromZero))
}

#[test]
fn la_grille_depuis_le_texte() {
    let t = table();
    let pss = pss_depuis_table(&t);
    let mut cas = 0;
    for annee in [2025, 2026] {
        for mois in 1..=12u32 {
            for dep in [1, 20] {
                let debut = (annee, mois, dep);
                for duree in 1..=62u32 {
                    let fin = ajouter(debut, duree - 1);
                    for c in [0, 1, 3, 6] {
                        // Les journees : un cachet par jour depuis le debut, tant qu'il y a des jours.
                        let journees = jours_du_contrat(debut, c.min(duree));
                        for artiste in [true, false] {
                            cas += 1;
                            let voulu = oracle(&t, duree, artiste, &journees);
                            let lu = plafond_urssaf(debut, fin, artiste, &pss, &journees);
                            match (&voulu, &lu) {
                                (Ok(v), Ok(l)) => assert_eq!(l, v, "{:?}->{:?} {} cachets, artiste {}", debut, fin, c, artiste),
                                (Err(()), Err(m)) if journees.is_empty() => {
                                    let regle = if artiste && duree < 5 { "arrete du 24/01/1975" } else { "BOSS, assiette generale, § 1090" };
                                    assert!(m.contains(regle), "{}", m)
                                }
                                (Err(()), Err(m)) => assert!(m.contains("2027"), "pas de plafond pour 2027 : {}", m),
                                _ => panic!("{:?}->{:?} {} cachets, artiste {} : texte {:?}, moteur {:?}", debut, fin, c, artiste, voulu, lu),
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(cas, 2 * 12 * 2 * 62 * 4 * 2);
}

#[test]
fn les_dates_et_le_mensuel() {
    let c: Cas = lire_chemin(&chemin_donnee("plafond-dates.json"));
    let t = table();
    let pss = pss_depuis_table(&t);
    for (debut, fin, statut, journees, voulu, quoi) in &c.dates {
        let dates: Vec<Jour> = journees.iter().map(|d| jour(d)).collect();
        let lu = plafond_urssaf(jour(debut), jour(fin), statut == "artiste", &pss, &dates).unwrap();
        assert_eq!(lu, *voulu, "{}", quoi);
    }
    for (debut, fin, voulu) in &c.mensuel {
        let e = EngagementPlafond {
            mensuelle: true,
            artiste: true,
            debut_periode: jour(debut),
            fin_periode: jour(fin),
            debut: (0, 1, 1), fin: (0, 1, 1), dates_travail: vec![], jours: 0,
        };
        assert_eq!(pour_paie(&e, &pss).unwrap(), *voulu, "{} {}", debut, fin);
    }
    // pour_paie d'un intermittent = plafond_urssaf avec ses dates.
    let e = EngagementPlafond {
        mensuelle: false, artiste: true, debut_periode: (2026, 5, 20), fin_periode: (2026, 5, 23),
        debut: (2026, 5, 20), fin: (2026, 5, 23),
        dates_travail: vec![(2026, 5, 20), (2026, 5, 22)], jours: 2,
    };
    assert_eq!(pour_paie(&e, &pss).unwrap(), dec("720.00"));
    assert_eq!(pour_paie(&e, &pss).unwrap(), plafond_urssaf(e.debut, e.fin, true, &pss, &e.dates_travail).unwrap());
    // Une journee dite (`jours`) et non datee refuse ; aucune journee dans la paie, aucun plafond.
    let err = pour_paie(&EngagementPlafond { jours: 3, ..e.clone() }, &pss).unwrap_err();
    assert!(err.contains("3 jour(s) travaille(s) pour 2 date(s)"), "{}", err);
    assert_eq!(pour_paie(&EngagementPlafond { jours: 0, dates_travail: vec![], ..e.clone() }, &pss).unwrap(), dec("0.00"));
    // Un technicien : le prorata de ses jours remuneres (BOSS § 1090), 4005 x 2 / 31.
    assert_eq!(pour_paie(&EngagementPlafond { artiste: false, ..e.clone() }, &pss).unwrap(), dec("258.39"));
    // Une paie de janvier d'un contrat du 20/01 au 10/02/2026 ne compte que ses jours (R. 242-2, « pour
    // chaque paie ») ; une date hors de la periode de paie refuse.
    let janvier = EngagementPlafond {
        mensuelle: false, artiste: true, debut_periode: (2026, 1, 20), fin_periode: (2026, 1, 31),
        debut: (2026, 1, 20), fin: (2026, 2, 10), dates_travail: vec![(2026, 1, 20), (2026, 1, 22), (2026, 1, 24)], jours: 3,
    };
    assert_eq!(pour_paie(&janvier, &pss).unwrap(), dec("387.58"));
    let err = pour_paie(&EngagementPlafond { dates_travail: vec![(2026, 1, 20), (2026, 1, 22), (2026, 2, 3)], ..janvier }, &pss).unwrap_err();
    assert!(err.contains("hors de la periode de paie"), "{}", err);
    // Sans jour remunere date, rien ne se compte : refus.
    for (debut, fin, statut, quoi) in &c.sans_date {
        let err = plafond_urssaf(jour(debut), jour(fin), statut == "artiste", &pss, &[]).unwrap_err();
        assert!(err.contains("0 jour(s) travaille(s) pour 0 date(s)"), "{} : {}", quoi, err);
    }
}

#[test]
fn bornes_incluses_et_exemples_des_bulletins() {
    let t = table();
    let pss = pss_depuis_table(&t);
    // FAVART 24/05/2026 : un jour, une journee -> 30 x 12 = 360.
    let r = plafond_urssaf((2026, 5, 24), (2026, 5, 24), true, &pss, &[(2026, 5, 24)]).unwrap();
    assert_eq!(r, dec("360"));
    // FAVART 13-22/05/2026 : dix jours, un jour remunere, 4005 / 31 (BOSS § 1090).
    let r = plafond_urssaf((2026, 5, 13), (2026, 5, 22), true, &pss, &[(2026, 5, 15)]).unwrap();
    assert_eq!(r, dec("129.19"));
}
