//! LES MENTIONS QUE LA LOI EXIGE D'UN CONTRAT ECRIT : le CDD « est établi par écrit » avec ses mentions (code du travail
//! L. 1242-12, `atelier/reference/sources/code-du-travail/xml/L1242-12.xml`), tout salarié reçoit par écrit les
//! informations de R. 1221-34 (`xml/R1221-34.xml`). Chaque modèle livré les porte toutes ; un modèle qui en perd une est
//! refusé, et le refus nomme la mention et son texte.

use std::path::Path;

use serde_json::{json, Value};

const MODELES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../logiciel/donnees/modeles");

fn modele(nom: &str) -> String {
    std::fs::read_to_string(Path::new(MODELES).join(nom)).unwrap()
}

#[test]
fn chaque_modele_livre_porte_les_mentions_de_l1242_12_et_r1221_34() {
    for nature in ["cddu_spectacle", "cdd", "cdi"] {
        let nat = droit::natures::nature(Some(nature)).unwrap();
        let nom = nat.modele_contrat().unwrap();
        let chemin = Path::new(MODELES).join(&nom);
        assert!(nat.mentions_obligatoires().len() >= 15, "{} : ses mentions", nature);
        documents::contrat_ecrit::refuser_les_mentions_absentes(&chemin, &modele(&nom), &nat, &json!({}))
            .unwrap_or_else(|e| panic!("{} : {}", nom, e));
    }
}

#[test]
fn un_modele_de_cdd_sans_son_motif_ni_le_terme_est_refuse() {
    let nat = droit::natures::nature(Some("cdd")).unwrap();
    let texte = modele("contrat-cdd.modele").replace("{{contrat.motif}}", "un surcroît").replace("{{contrat.fin}}", "la fin du projet");
    let propre = Path::new("/compagnie/gabarits/contrat-cdd.modele");
    let refus = documents::contrat_ecrit::refuser_les_mentions_absentes(propre, &texte, &nat, &json!({})).unwrap_err();
    assert!(refus.contains("{{contrat.motif}}") && refus.contains("L. 1242-12, premier alinéa"), "{}", refus);
    assert!(refus.contains("{{contrat.fin}}") && refus.contains("L. 1242-12, 2°"), "{}", refus);
}

#[test]
fn la_personne_remplacee_n_est_due_que_pour_un_remplacement() {
    let nat = droit::natures::nature(Some("cdd")).unwrap();
    let texte = modele("contrat-cdd.modele").replace("{{contrat.personne_remplacee}}", "");
    let propre = Path::new("/compagnie/gabarits/contrat-cdd.modele");
    let sans: Value = json!({"contrat.personne_remplacee": ""});
    assert!(documents::contrat_ecrit::refuser_les_mentions_absentes(propre, &texte, &nat, &sans).is_ok());
    let avec: Value = json!({"contrat.personne_remplacee": ", en remplacement de Mme X, chargée de production"});
    let refus = documents::contrat_ecrit::refuser_les_mentions_absentes(propre, &texte, &nat, &avec).unwrap_err();
    assert!(refus.contains("L. 1242-12, 1°"), "{}", refus);
}
