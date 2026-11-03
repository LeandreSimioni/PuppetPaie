//! LE SALAIRE D'UN MOIS INCOMPLET (`droit::periodes::part_du_mois`).

use droit::conges::{feries_non_payes, JoursTravailles};
use droit::periodes::{part_du_mois, regle_du_mois_incomplet, Horaire, RegleDuMoisIncomplet};
use serde_json::json;
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
    let (h, lv) = (RegleDuMoisIncomplet::HoraireReel, Horaire::du_lundi_au_vendredi());
    // L'entree au lundi 12/01/2026 : 15 jours ouvres sur les 22 de janvier (le jeudi 1er, ferie, en est un).
    let entree = part_du_mois(&j("2026-01-12"), &j("2026-01-31"), &j("2026-01-31"), h, &lv).unwrap();
    assert_eq!((entree.jours, entree.jours_du_mois), (15, 22));
    assert_eq!(entree.fraction, div(Decimal::from(15), Decimal::from(22)));
    assert_eq!(entree.sur_le_bulletin(Decimal::from(7)).as_deref(), Some("105 h sur les 154 h du mois"));
    // La sortie au mercredi 25/02/2026 : 18 jours ouvres sur 20, et non 25 jours sur 28.
    let sortie = part_du_mois(&j("2026-02-01"), &j("2026-02-25"), &j("2026-02-28"), h, &lv).unwrap();
    assert_eq!((sortie.jours, sortie.jours_du_mois), (18, 20));
    // Janvier 2024 entier : 23 jours ouvres, et tout le salaire.
    let entier = part_du_mois(&j("2024-01-01"), &j("2024-01-31"), &j("2024-01-31"), h, &lv).unwrap();
    assert_eq!((entier.jours_du_mois, entier.fraction), (23, Decimal::ONE));
    assert_eq!(regle_du_mois_incomplet("3090", "artiste", true), h);
    assert_eq!(regle_du_mois_incomplet("1285", "technicien", true), h);
    assert_eq!(regle_du_mois_incomplet("1285", "artiste", false), h);
}

/// Le meme arret : « l'horaire de travail dans l'entreprise » est celui du salarie, ses jours travailles (sa fiche). Un
/// administrateur du mardi au samedi entre le jeudi 15/10/2026 travaille 13 des 23 jours de son horaire en octobre (du
/// lundi au vendredi, ce serait 12 sur 22) ; 35 h sur cinq jours font 7 h par jour.
#[test]
fn les_heures_du_mois_se_comptent_sur_les_jours_travailles_de_la_fiche() {
    let jours = JoursTravailles::de_la_fiche(&json!({"jours_travailles": ["mardi", "mercredi", "jeudi", "vendredi", "samedi"]}))
        .unwrap();
    assert_eq!(jours.en_mots(), "du mardi au samedi");
    let horaire = Horaire { jours, feries_non_payes: Vec::new() };
    let p = part_du_mois(&j("2026-10-15"), &j("2026-10-31"), &j("2026-10-31"), RegleDuMoisIncomplet::HoraireReel, &horaire)
        .unwrap();
    assert_eq!((p.jours, p.jours_du_mois), (13, 23));
    assert_eq!(p.sur_le_bulletin(droit::conges::heures_par_jour(Decimal::new(15167, 2), &jours)).as_deref(), Some("91 h sur les 161 h du mois"));
    assert!(JoursTravailles::de_la_fiche(&json!({"jours_travailles": []})).is_err());
    assert!(JoursTravailles::de_la_fiche(&json!({"jours_travailles": ["lundi", "jour de fete"]})).is_err());
}

/// L3133-3 (`atelier/reference/sources/code-du-travail/articles-20261029.md`, l. 113) : le chomage d'un ferie n'entraine
/// aucune perte de salaire pour qui totalise trois mois d'anciennete ; pour les autres, le ferie chome n'est pas du.
/// L3133-5 (l. 123) : le 1er mai, jamais. Entree au 12/01/2026 dans une compagnie qui chome les onze feries : le lundi de
/// Paques 06/04 tombe avant les trois mois, avril se paie 21 jours sur 22 ; entree au 15/02 : en mai, le 1er est paye, le
/// 8 ne l'est pas, l'Ascension (14/05) tombe le jour des trois mois.
#[test]
fn un_ferie_chome_ne_se_paie_qu_apres_trois_mois_d_anciennete() {
    let tous = json!({"jours_feries_chomes": "tous"});
    let lv = JoursTravailles::du_lundi_au_vendredi();
    let avril = feries_non_payes(&j("2026-04-01"), &j("2026-04-30"), Some(&tous), &lv, &j("2026-01-12")).unwrap();
    assert_eq!(avril.iter().map(|(d, _)| d.iso()).collect::<Vec<_>>(), ["2026-04-06"]);
    let horaire = Horaire { jours: lv, feries_non_payes: avril };
    let p = part_du_mois(&j("2026-04-01"), &j("2026-04-30"), &j("2026-04-30"), RegleDuMoisIncomplet::HoraireReel, &horaire)
        .unwrap();
    assert_eq!((p.jours, p.jours_du_mois), (21, 22));
    let mai = feries_non_payes(&j("2026-05-01"), &j("2026-05-31"), Some(&tous), &lv, &j("2026-02-15")).unwrap();
    assert_eq!(mai.iter().map(|(d, _)| d.iso()).collect::<Vec<_>>(), ["2026-05-08"]);
    // Trois mois d'anciennete : rien. Un ferie que la compagnie travaille est un jour de travail.
    assert!(feries_non_payes(&j("2026-04-01"), &j("2026-04-30"), Some(&tous), &lv, &j("2025-12-01")).unwrap().is_empty());
    let premier_mai = json!({"jours_feries_chomes": "1er mai seulement"});
    assert!(feries_non_payes(&j("2026-04-01"), &j("2026-05-31"), Some(&premier_mai), &lv, &j("2026-03-02")).unwrap().is_empty());
}

/// CCN 1285, art. X.3 (`atelier/reference/sources/kali-1285-x3-remuneration-artistes-20260929.md`, l. 20) : le premier
/// ou le dernier mois incomplet d'un artiste se paie 1/21 du salaire mensuel par jour ouvre du contrat ; la convention qui
/// a sa regle l'emporte sur l'horaire reel.
#[test]
fn le_premier_mois_d_un_artiste_de_la_1285_se_paie_au_vingt_et_unieme() {
    let r = regle_du_mois_incomplet("1285", "artiste", true);
    assert_eq!(r, RegleDuMoisIncomplet::Ccneac1285Artiste);
    let p = part_du_mois(&j("2026-01-12"), &j("2026-01-31"), &j("2026-01-31"), r, &Horaire::du_lundi_au_vendredi()).unwrap();
    assert_eq!((p.jours, p.jours_du_mois), (15, 21));
    assert_eq!(p.fraction, div(Decimal::from(15), Decimal::from(21)));
}
