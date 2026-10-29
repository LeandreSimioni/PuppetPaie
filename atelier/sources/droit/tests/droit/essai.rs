//! La duree maximale de la periode d'essai (`droit::essai`). Textes : code du travail L. 1242-10 (« un jour par semaine,
//! dans la limite de deux semaines lorsque la durée initialement prévue au contrat est au plus égale à six mois et d'un
//! mois dans les autres cas », `atelier/reference/sources/code-du-travail/xml/L1242-10.xml`), L. 1221-19 (deux, trois,
//! quatre mois, `xml/L1221-19.xml`) ; convention 3090, art. 7.4 (« artistes interprètes : 2 mois de travail effectif »,
//! `atelier/reference/sources/kali-3090-titres-vii-x-xi-annexes-20260924.md`).

use droit::essai::{plafond, refuser_au_dela};
use droit::natures::nature;
use socle::date::Jour;

#[test]
fn la_periode_d_essai_ne_depasse_pas_ce_que_le_texte_permet() {
    let (cddu, cdd, cdi) = (nature(Some("cddu_spectacle")).unwrap(), nature(Some("cdd")).unwrap(), nature(Some("cdi")).unwrap());
    let j = |s: &str| Jour::lire(s).unwrap();
    let jours = |p: Option<droit::essai::Plafond>| p.map(|p| p.jours);

    // Un CDD de six mois au plus : un jour par semaine, deux semaines au plus.
    let jules = plafond(&cdd, &j("2026-01-12"), Some(&j("2026-06-30")), Some("05"), false, "3090").unwrap();
    assert_eq!(jours(jules.clone()), Some(14));
    let refus = refuser_au_dela(30, jules.as_ref(), "VARENNE Jules").unwrap_err();
    assert!(refus.contains("L. 1242-10") && refus.contains("14 jours"), "{}", refus);
    assert!(refuser_au_dela(14, jules.as_ref(), "VARENNE Jules").is_ok());
    // Trois semaines de contrat : trois jours ; un contrat d'usage de trois jours : un jour.
    assert_eq!(jours(plafond(&cdd, &j("2026-03-02"), Some(&j("2026-03-22")), None, false, "").unwrap()), Some(3));
    assert_eq!(jours(plafond(&cddu, &j("2026-11-20"), Some(&j("2026-11-22")), None, true, "3090").unwrap()), Some(1));
    // Plus de six mois : un jour par semaine, un mois au plus.
    assert_eq!(jours(plafond(&cdd, &j("2026-01-12"), Some(&j("2026-07-20")), None, false, "").unwrap()), Some(28));
    assert_eq!(jours(plafond(&cdd, &j("2026-01-12"), Some(&j("2026-12-31")), None, false, "").unwrap()), Some(31));
    // Un CDD sans terme : sa duree minimale n'est pas dite, rien ne se juge.
    assert_eq!(plafond(&cdd, &j("2026-01-12"), None, None, false, "").unwrap(), None);

    // Un CDI : deux mois pour un employe, trois pour un agent de maitrise, quatre pour un cadre ; un artiste de la 3090,
    // deux mois ; un classement que la loi ne nomme pas, rien.
    let debut = j("2026-01-12");
    assert_eq!(jours(plafond(&cdi, &debut, None, Some("06"), false, "").unwrap()), Some(59));
    assert_eq!(jours(plafond(&cdi, &debut, None, Some("05"), false, "").unwrap()), Some(90));
    assert_eq!(jours(plafond(&cdi, &debut, None, Some("04"), false, "").unwrap()), Some(120));
    assert_eq!(jours(plafond(&cdi, &debut, None, Some("04"), true, "3090").unwrap()), Some(59));
    assert_eq!(plafond(&cdi, &debut, None, Some("01"), false, "").unwrap(), None);
    let cadre = plafond(&cdi, &debut, None, Some("04"), false, "").unwrap();
    assert!(refuser_au_dela(121, cadre.as_ref(), "X").unwrap_err().contains("L. 1221-19"));
}
