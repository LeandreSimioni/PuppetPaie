//! Qui l'employeur declare a la caisse des Conges Spectacles (`droit::caisse_conges::affilie`). Textes
//! (`atelier/reference/sources/caisse-conges-affiliation/LISEZ-MOI.md`) : code du travail D. 7121-40 (« le personnel
//! artistique et technique ») et D. 7121-41 (« qu'ils n'ont pas employe de facon continue pendant les douze mois
//! precedant la demande de conge ») ; fiche Audiens (« quelle que soit [...] la nature du contrat de travail (CDD de
//! droit commun, CDD d'usage, etc) », sous contrat a duree determinee « de moins de 12 mois »). Le technicien en CDD :
//! l'annexe 8 range « les ouvriers et techniciens engages dans le cadre d'un contrat a duree determinee, pour assurer
//! l'une des fonctions enumerees dans la liste figurant au titre XIV, [...] dans les domaines d'activite [...] enumeres
//! dans la meme liste » (art. 1er, § 2,
//! `atelier/reference/sources/plafond-intermittents/unedic-annexe8-convention-20241115.txt`) ; la liste 6 (subventionne,
//! IDCC 1285) nomme le staffeur, la liste 7 (prive, IDCC 3090) non (art. 71, l. 3094 et 3145-3316 ; circulaire Unedic
//! 2016-25, `unedic-circulaire-2016-25-annexes-8-10.txt`, l. 567-573).

use droit::caisse_conges::{affilie, douze_mois_ou_plus};
use droit::contexte::IdSalarie;
use droit::fiche::{FicheSalarie, SalarieDuContrat};
use droit::natures::nature;
use socle::date::Jour;

use crate::outils::typee::lu;

#[test]
fn qui_est_declare_a_la_caisse() {
    let (cddu, cdd, cdi) = (nature("cddu_spectacle").unwrap(), nature("cdd").unwrap(), nature("cdi").unwrap());
    let j = |s: &str| Jour::lire(s).unwrap();
    let comedien = fiche(r#"{"code_emploi_ft": "COM010", "statut": "artiste"}"#, None);
    let regisseur = |idcc: &str| fiche(r#"{"code_emploi_ft": "REG130", "statut": "technicien"}"#, Some(idcc));
    let sans_metier = fiche(r#"{"statut": "technicien"}"#, Some("3090"));
    let (debut, trois_mois, un_an) = (j("2026-09-01"), j("2026-11-30"), j("2027-08-31"));
    assert!(douze_mois_ou_plus(&debut, &un_an).unwrap());
    assert!(affilie(&cddu, &comedien, &debut, Some(&trois_mois)).unwrap());
    assert!(!affilie(&cddu, &comedien, &debut, Some(&un_an)).unwrap(), "un CDDU de douze mois");
    assert!(affilie(&cdd, &comedien, &debut, Some(&trois_mois)).unwrap(), "un artiste en CDD de droit commun");
    assert!(!affilie(&cdd, &comedien, &debut, Some(&un_an)).unwrap(), "un CDD de douze mois");
    // Le regisseur lumiere est sur la liste du spectacle vivant prive (3090), pas sur celle de la radiodiffusion (1922).
    assert!(affilie(&cdd, &regisseur("3090"), &debut, Some(&trois_mois)).unwrap(), "un technicien de la liste en CDD");
    assert!(!affilie(&cdd, &regisseur("1922"), &debut, Some(&trois_mois)).unwrap(), "une fonction hors de la liste du domaine");
    assert!(affilie(&cddu, &regisseur("1922"), &debut, Some(&trois_mois)).unwrap(), "un technicien en CDDU");
    // Le staffeur est sur la liste du spectacle vivant subventionne (1285), pas sur celle du prive (3090).
    let staffeur = |idcc: &str| fiche(r#"{"code_emploi_ft": "STA010", "statut": "technicien"}"#, Some(idcc));
    assert!(affilie(&cdd, &staffeur("1285"), &debut, Some(&trois_mois)).unwrap(), "la liste 6 nomme le staffeur");
    assert!(!affilie(&cdd, &staffeur("3090"), &debut, Some(&trois_mois)).unwrap(), "la liste 7 ne le nomme pas");
    assert!(!affilie(&cdd, &sans_metier, &debut, Some(&trois_mois)).unwrap(), "un metier hors de la nomenclature");
    assert!(!affilie(&cdi, &comedien, &debut, None).unwrap(), "un CDI");
    assert!(affilie(&cdd, &comedien, &debut, None).unwrap_err().contains("sans terme"));
}

/// La fiche telle que la paie la lit : ses champs, et la convention de l'employeur.
fn fiche(champs: &str, idcc: Option<&str>) -> SalarieDuContrat {
    let champs = lu(champs);
    let mut s = SalarieDuContrat::de(FicheSalarie { id: IdSalarie::lire("X").unwrap(), champs });
    s.idcc_de_l_employeur = idcc.map(String::from);
    s
}
