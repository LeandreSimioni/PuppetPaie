//! Qui l'employeur declare a la caisse des Conges Spectacles (`droit::caisse_conges::affilie`). Textes
//! (`atelier/reference/sources/caisse-conges-affiliation/LISEZ-MOI.md`) : code du travail D. 7121-40 (« le personnel
//! artistique et technique ») et D. 7121-41 (« qu'ils n'ont pas employe de facon continue pendant les douze mois
//! precedant la demande de conge ») ; fiche Audiens (« quelle que soit [...] la nature du contrat de travail (CDD de
//! droit commun, CDD d'usage, etc) », sous contrat a duree determinee « de moins de 12 mois »).

use droit::caisse_conges::{affilie, douze_mois_ou_plus};
use droit::natures::nature;
use serde_json::json;
use socle::date::Jour;

#[test]
fn qui_est_declare_a_la_caisse() {
    let (cddu, cdd, cdi) = (nature(Some("cddu_spectacle")).unwrap(), nature(Some("cdd")).unwrap(), nature(Some("cdi")).unwrap());
    let j = |s: &str| Jour::lire(s).unwrap();
    let comedien = json!({"code_emploi_ft": "COM010", "statut": "artiste"});
    let regisseur = json!({"code_emploi_ft": "REG130", "statut": "technicien"});
    let sans_metier = json!({"statut": "technicien"});
    let (debut, trois_mois, un_an) = (j("2026-09-01"), j("2026-11-30"), j("2027-08-31"));
    assert!(douze_mois_ou_plus(&debut, &un_an).unwrap());
    assert!(affilie(&cddu, &comedien, &debut, Some(&trois_mois)).unwrap());
    assert!(!affilie(&cddu, &comedien, &debut, Some(&un_an)).unwrap(), "un CDDU de douze mois");
    assert!(affilie(&cdd, &comedien, &debut, Some(&trois_mois)).unwrap(), "un artiste en CDD de droit commun");
    assert!(!affilie(&cdd, &comedien, &debut, Some(&un_an)).unwrap(), "un CDD de douze mois");
    // Le technicien en CDD de droit commun : non declare tant que n'est pas decide quel technicien la caisse attend
    // (l'annexe 8 range aussi l'attache de production parmi les techniciens ; PASSATION-V2f.md).
    assert!(!affilie(&cdd, &regisseur, &debut, Some(&trois_mois)).unwrap(), "un technicien en CDD de droit commun");
    assert!(affilie(&cddu, &regisseur, &debut, Some(&trois_mois)).unwrap(), "un technicien en CDDU");
    assert!(!affilie(&cdd, &sans_metier, &debut, Some(&trois_mois)).unwrap(), "un metier hors de la nomenclature");
    assert!(!affilie(&cdi, &comedien, &debut, None).unwrap(), "un CDI");
    assert!(affilie(&cdd, &comedien, &debut, None).unwrap_err().contains("sans terme"));
}
