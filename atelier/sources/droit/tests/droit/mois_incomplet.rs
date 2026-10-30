//! LE SALAIRE D'UN MOIS INCOMPLET (`droit::periodes::part_du_mois`).

use droit::periodes::{part_du_mois, regle_du_mois_incomplet, RegleDuMoisIncomplet};
use socle::date::Jour;
use socle::num::{div, Decimal};

fn j(t: &str) -> Jour {
    Jour::lire(t).unwrap()
}

/// Cass. soc., 20/01/1999, n° 96-45.042 (`atelier/reference/sources/mois-incomplet/
/// legifrance-cass-soc-19990120-96-45042-20261030.md`) : une sortie au 25 fevrier ne se paie pas « le 25/30e » ; le salaire
/// du mois est « la remuneration horaire » (le salaire mensuel sur les heures du mois dans l'entreprise) « par le nombre
/// d'heures de travail reellement effectuees ». Les heures se comptent aux jours ouvres, feries compris (fiche DSN 2693,
/// exemple 4 : janvier 2024, 23 jours, le 1er janvier compris).
#[test]
fn un_mois_incomplet_se_paie_a_l_horaire_reel() {
    let h = RegleDuMoisIncomplet::HoraireReel;
    // L'entree au lundi 12/01/2026 : 15 jours ouvres sur les 22 de janvier (le jeudi 1er, ferie, en est un).
    let entree = part_du_mois(&j("2026-01-12"), &j("2026-01-31"), &j("2026-01-31"), h).unwrap();
    assert_eq!((entree.jours, entree.jours_du_mois), (15, 22));
    assert_eq!(entree.fraction, div(Decimal::from(15), Decimal::from(22)));
    assert_eq!(entree.sur_le_bulletin(Decimal::from(7)).as_deref(), Some("105 h sur les 154 h du mois"));
    // La sortie au mercredi 25/02/2026 : 18 jours ouvres sur 20, et non 25 jours sur 28.
    let sortie = part_du_mois(&j("2026-02-01"), &j("2026-02-25"), &j("2026-02-28"), h).unwrap();
    assert_eq!((sortie.jours, sortie.jours_du_mois), (18, 20));
    // Janvier 2024 entier : 23 jours ouvres, et tout le salaire.
    let entier = part_du_mois(&j("2024-01-01"), &j("2024-01-31"), &j("2024-01-31"), h).unwrap();
    assert_eq!((entier.jours_du_mois, entier.fraction), (23, Decimal::ONE));
    assert_eq!(regle_du_mois_incomplet("3090", "artiste", true), h);
    assert_eq!(regle_du_mois_incomplet("1285", "technicien", true), h);
    assert_eq!(regle_du_mois_incomplet("1285", "artiste", false), h);
}

/// CCN 1285, art. X.3 (`atelier/reference/sources/kali-1285-x3-remuneration-artistes-20260929.md`, l. 20) : le premier
/// ou le dernier mois incomplet d'un artiste se paie 1/21 du salaire mensuel par jour ouvre du contrat ; la convention qui
/// a sa regle l'emporte sur l'horaire reel.
#[test]
fn le_premier_mois_d_un_artiste_de_la_1285_se_paie_au_vingt_et_unieme() {
    let r = regle_du_mois_incomplet("1285", "artiste", true);
    assert_eq!(r, RegleDuMoisIncomplet::Ccneac1285Artiste);
    let p = part_du_mois(&j("2026-01-12"), &j("2026-01-31"), &j("2026-01-31"), r).unwrap();
    assert_eq!((p.jours, p.jours_du_mois), (15, 21));
    assert_eq!(p.fraction, div(Decimal::from(15), Decimal::from(21)));
}
