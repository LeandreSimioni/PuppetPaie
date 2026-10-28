//! L'affiliation prevoyance d'un salarie (bloc S21.G00.70) dit l'option de sante que
//! l'organisme demande, ses ayants droit, la date ou elle commence quand ce n'est pas celle du contrat -- et un salarie
//! qui n'est pas affilie n'est pas declare affilie.
//!
//! Textes :
//! - cahier technique DSN 2026.1, bloc S21.G00.70 (`atelier/reference/norme-dsn/texte/bloc-S21.G00.70.txt`, p. 285-286) :
//!   70.004 « Les valeurs de code sont fournies par l'organisme de prévoyance et sont à renseigner seulement sur sa
//!   demande » ; 70.008 et 70.011 « doit être renseignée si elle intervient dans le calcul de la cotisation » ; 70.009
//!   la somme des trois compteurs « si ces rubriques sont toutes les trois valorisées » ; 70.014 et 70.015
//!   (`bloc-S21.G00.71.txt`, p. 287) : « lorsque la date de début d'affiliation au contrat OC ne correspond pas à la
//!   date de début du contrat de travail » ; le bloc 15 (`bloc-S21.G00.15.txt`, CCH-13 et CCH-14 de 15.004) ;
//! - bloc S21.G00.79 (`bloc-S21.G00.79.txt`, p. 298-300) : « Pour la Prévoyance, ce bloc est à alimenter par les
//!   éléments constituant les paramètres qui entrent dans le calcul de la cotisation », « 18 - Base forfaitaire
//!   Prévoyance » ;
//! - fiche de parametrage Audiens version 139 (`atelier/reference/fiches-parametrage/audiens-2025-12-13-fiche139-expurgee.xml`),
//!   groupes 00003 a 00008 : `ReferenceContrat="FM SPECT VIVANT PRIV"`, `CodeOption` NIV1/I/GE ... NIV3F/IACE/GE,
//!   `BaseMontantSpecifique` `ValeurCodeNature="18"`, `Assiette` PMSS, par assure, conjoint et enfant.
//!

use serde_json::json;


/// La fiche propose la dispense et ses bornes aux permanents des deux conventions qui en ont (1285, 3090).
#[test]
fn la_fiche_propose_la_dispense_de_sante_en_ccneac_et_en_3090() {
    let d = paie::champs::description("salarie").unwrap();
    for cle in ["sante_dispense", "sante_dispense_depuis", "sante_dispense_jusqu_au"] {
        let c = d.as_array().unwrap().iter().find(|c| c["cle"] == json!(cle)).unwrap_or_else(|| panic!("{}", cle));
        assert_eq!(c["si_entreprise"], json!({"idcc": ["1285", "3090"]}), "{}", cle);
    }
}




