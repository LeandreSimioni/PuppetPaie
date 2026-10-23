//! LA LICENCE D'ENTREPRENEUR DE SPECTACLES (S21.G00.40.049, #236). « La licence est obligatoire pour toute structure [...]
//! dont l'activite principale est la production ou la diffusion de spectacle ou l'exploitation de lieu de spectacle, a
//! partir de 7 representations annuelles si l'activite principale de l'entreprise n'est pas d'organiser des spectacles »
//! (cahier technique 2027.1.1, `atelier/reference/norme-dsn/texte-2027/bloc-S21.G00.40.txt`, l. 1418-1426). La fiche dit
//! de qui elle est EXIGEE (`exige_si` du champ, `champs-base.json`) : la DSN la lit par `demarrage::licence_exigee`, comme
//! « Pour demarrer » (`tests/routes/demarrage.rs`, sur toutes les conventions).
//!
//! Sur une copie de `fictive-3090-grande` (compagnie inventee), la DSN de juillet 2026.

use crate::outils::{self, compagnie};
use serde_json::json;

fn dsn(test: &str, idcc: &str, licence: Option<&str>) -> Result<String, String> {
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
    outils::dsn_du_mois(&r, "2026-07", "2026-07-28")
}

#[test]
fn une_compagnie_du_spectacle_vivant_sans_licence_ne_fait_pas_sa_dsn() {
    let e = dsn("3090-sans", "3090", None).unwrap_err();
    assert!(e.contains("S21.G00.40.049"), "{}", e);
    let d = dsn("3090-avec", "3090", Some("PLATESV-R-2023-101101")).unwrap();
    assert_eq!(outils::valeur(&outils::paires(&d), "S21.G00.40.049").as_deref(), Some("PLATESV-R-2023-101101"));
}

#[test]
fn hors_des_conventions_qui_l_exigent_la_licence_s_ecrit_si_elle_est_dite_et_rien_sinon() {
    // 1790 : un lieu peut en avoir une (la case se montre) sans qu'elle soit exigee de toutes.
    let d = dsn("1790-sans", "1790", None).unwrap();
    assert!(!outils::porte(&d, "S21.G00.40.049"), "ni refus ni valeur vide (X [1,100])");
    let d = dsn("1790-avec", "1790", Some("PLATESV-R-2023-101101")).unwrap();
    assert_eq!(outils::valeur(&outils::paires(&d), "S21.G00.40.049").as_deref(), Some("PLATESV-R-2023-101101"));
}
