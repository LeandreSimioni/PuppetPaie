//! Le maintien de salaire que la convention 1285 garantit, lu par `droit::maintien` : X.4.6 « Maladie » et X.4.7
//! « Accident du travail » (`atelier/reference/sources/kali-1285-titres-ii-iii-x-xi-xii-20260923.md`, l. 579-590).

use droit::maintien::{conventionnel, Base, Droit, Duree, Origine, Tranche};
use rust_decimal::Decimal;

fn tranches(mois: i64) -> Vec<Tranche> {
    vec![Tranche { duree: Duree::Mois(mois), taux: Decimal::from(100) }, Tranche { duree: Duree::Mois(mois), taux: Decimal::from(75) }]
}

/// « à partir de 6 mois et jusqu'à 1 an d'ancienneté : 1 mois à 100 % et 1 mois à 75 % ; au-delà de 1 an d'ancienneté :
/// 3 mois à 100 % et 3 mois à 75 % », pour « tout salarié », avec la subrogation, sans carence écrite.
#[test]
fn la_1285_maintient_le_brut_selon_l_anciennete() {
    let idcc = Some("1285");
    let regime = |mois: i64, o: Origine| conventionnel(idcc, Decimal::from(mois), o).unwrap().unwrap();
    assert!(matches!(regime(5, Origine::Maladie), Droit::PasDu(_)), "rien avant six mois");
    for (anciennete, duree) in [(6, 1), (11, 1), (12, 3), (42, 3)] {
        let Droit::Du(r) = regime(anciennete, Origine::Maladie) else { panic!("{} mois : un maintien", anciennete) };
        assert_eq!((r.tranches, r.base, r.carence, r.subrogation), (tranches(duree), Base::Brut, 0, true), "{} mois", anciennete);
    }
    // X.4.7 : l'accident du travail et l'accident de trajet, le salaire brut garanti pendant les indemnités, sans
    // ancienneté ; la maladie professionnelle, que X.4.7 ne nomme pas, suit X.4.6.
    for o in [Origine::AccidentDuTravail, Origine::Trajet] {
        let Droit::Du(r) = regime(2, o) else { panic!("{:?} : un maintien", o) };
        assert_eq!(r.tranches, vec![Tranche { duree: Duree::PendantLesIndemnites, taux: Decimal::from(100) }]);
    }
    assert!(matches!(regime(2, Origine::MaladieProfessionnelle), Droit::PasDu(_)));
}
