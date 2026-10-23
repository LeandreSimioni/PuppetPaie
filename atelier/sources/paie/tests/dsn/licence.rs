//! LA LICENCE D'ENTREPRENEUR DE SPECTACLES (S21.G00.40.049, #236). « La licence est obligatoire pour toute structure [...]
//! dont l'activite principale est la production ou la diffusion de spectacle ou l'exploitation de lieu de spectacle »
//! (cahier technique 2027.1.1, `atelier/reference/norme-dsn/texte-2027/bloc-S21.G00.40.txt`, l. 1418-1426) ; « est
//! entrepreneur de spectacles vivants toute personne qui exerce une activite d'exploitation de lieux de spectacles, de
//! production ou de diffusion de spectacles » (L7122-2), et sa declaration « donne lieu a la delivrance d'un recepisse
//! [...] valant licence » (L7122-3 ; `atelier/reference/sources/code-du-travail/articles-20261023.md`). Qui la doit :
//! `demarrage::licence_exigee`, que « Pour demarrer » lit aussi (`tests/routes/demarrage.rs`, sur toutes les conventions).
//!
//! Sur une copie de `fictive-3090-grande` (compagnie inventee) : juillet 2026 porte des contrats de spectacle (CDDU),
//! aout 2026 seulement des CDI.

use crate::outils::{self, compagnie};
use serde_json::json;

fn dsn(test: &str, idcc: &str, licence: Option<&str>, mois: &str) -> Result<String, String> {
    let r = compagnie::ses_dsn("fictive-3090-grande", &[], &outils::dossier_de_test("licence", test));
    let mut ent = outils::lire(&r, "config/entreprise.json");
    ent["idcc"] = json!(idcc);
    match licence {
        Some(l) => ent["licence_spectacle"] = json!(l),
        None => {
            ent.as_object_mut().unwrap().remove("licence_spectacle");
        }
    }
    outils::ecrire(&r, "config/entreprise.json", &ent);
    if idcc != "3090" {
        // Une compagnie qui n'a jamais declare sous une autre convention : sans ses DSN passees, aucun changement de
        // contrat (S21.G00.41) a dater ni regularisation a relire.
        std::fs::remove_dir_all(r.join("sorties")).unwrap();
        outils::ecrire(&r, "journal/depots.json", &json!([]));
    }
    outils::dsn_du_mois(&r, mois, &format!("{}-28", mois))
}

const LICENCE: &str = "PLATESV-R-2023-101101";

fn licence_ecrite(d: &str) -> Option<String> {
    outils::valeur(&outils::paires(d), "S21.G00.40.049")
}

#[test]
fn une_compagnie_du_spectacle_vivant_sans_licence_ne_fait_pas_sa_dsn() {
    for mois in ["2026-07", "2026-08"] {
        let e = dsn(&format!("3090-sans-{}", mois), "3090", None, mois).unwrap_err();
        assert!(e.contains("S21.G00.40.049"), "{}", e);
    }
    let d = dsn("3090-avec", "3090", Some(LICENCE), "2026-07").unwrap();
    assert_eq!(licence_ecrite(&d).as_deref(), Some(LICENCE));
}

#[test]
fn un_lieu_en_1790_la_doit_des_qu_il_engage_pour_un_spectacle() {
    // Des contrats de spectacle : l'entrepreneur de spectacles la declare, sans elle la DSN refuse.
    let e = dsn("1790-spectacle-sans", "1790", None, "2026-07").unwrap_err();
    assert!(e.contains("S21.G00.40.049"), "{}", e);
    let d = dsn("1790-spectacle-avec", "1790", Some(LICENCE), "2026-07").unwrap();
    assert_eq!(licence_ecrite(&d).as_deref(), Some(LICENCE));
    // Seulement des salaries au mois, aucun spectacle : ni refus, ni valeur vide (X [1,100]).
    let d = dsn("1790-cdi-sans", "1790", None, "2026-08").unwrap();
    assert!(!outils::porte(&d, "S21.G00.40.049"));
}
