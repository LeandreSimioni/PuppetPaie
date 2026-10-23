//! Le type du contact chez le declare (S20.G00.07.004, #236). Une compagnie neuve ne le saisit pas : la fiche Entreprise
//! ne montre pas cette case, et « Pour demarrer » ne la demande plus. Le moteur ne declare qu'UN contact, celui qui
//! repond aux organismes pour toute la declaration : le type « 04 - Contact chez le declare recouvrant, entre autres,
//! des cotisations de Securite Sociale (Urssaf CN, MSA) et contributions assurance chomage et cotisations AGS (France
//! Travail) » (cahier technique 2027.1.1, `atelier/reference/norme-dsn/texte-2027/bloc-S20.G00.07.txt`, l. 67-80 ;
//! `dsn::type_du_contact`). Une valeur posee dans la fiche reste lue telle quelle.
//!
//! Sur une copie de `fictive-3090-grande` (compagnie inventee), la DSN de juillet 2026.

use crate::outils::{self, compagnie};
use serde_json::{json, Value};

fn dsn_avec_le_declarant(test: &str, qualite: Option<Value>) -> String {
    let r = compagnie::ses_dsn("fictive-3090-grande", &[], &outils::dossier_de_test("contact_declare", test));
    let mut ent = outils::lire(&r, "config/entreprise.json");
    let decl = ent["declarant_dsn"].as_object_mut().unwrap();
    match qualite {
        Some(q) => decl.insert("qualite".into(), q),
        None => decl.remove("qualite"),
    };
    outils::ecrire(&r, "config/entreprise.json", &ent);
    outils::dsn_du_mois(&r, "2026-07", "2026-07-28").unwrap()
}

#[test]
fn une_fiche_sans_type_de_contact_declare_le_contact_04() {
    let dsn = dsn_avec_le_declarant("sans", None);
    assert_eq!(outils::valeur(&outils::paires(&dsn), "S20.G00.07.004").as_deref(), Some("04"));
}

#[test]
fn un_type_de_contact_pose_dans_la_fiche_reste_le_sien() {
    let dsn = dsn_avec_le_declarant("pose", Some(json!("05")));
    assert_eq!(outils::valeur(&outils::paires(&dsn), "S20.G00.07.004").as_deref(), Some("05"));
    let vide = dsn_avec_le_declarant("vide", Some(json!("")));
    assert_eq!(outils::valeur(&outils::paires(&vide), "S20.G00.07.004").as_deref(), Some("04"), "une case vide vaut l'absence");
}
