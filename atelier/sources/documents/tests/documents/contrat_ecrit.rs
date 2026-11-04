//! LES MENTIONS QUE LA LOI EXIGE D'UN CONTRAT ECRIT : le CDD « est établi par écrit » avec ses mentions (code du travail
//! L. 1242-12, `atelier/reference/sources/code-du-travail/xml/L1242-12.xml`), tout salarié reçoit par écrit les
//! informations de R. 1221-34 (`xml/R1221-34.xml`). Chaque modèle livré se lit (chacun de ses marqueurs est connu) et
//! porte ces mentions ; un modèle qui en perd une est refusé, et le refus nomme la mention et son texte.

use std::path::Path;

use documents::marqueurs::{Marqueur, ModeleLu, Valeurs};

const MODELES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../logiciel/donnees/modeles");

fn modele(nom: &str) -> ModeleLu {
    ModeleLu::lire(&std::fs::read_to_string(Path::new(MODELES).join(nom)).unwrap(), nom).unwrap()
}

fn sans(lu: &ModeleLu, de: &str, par: &str) -> ModeleLu {
    ModeleLu::lire(&lu.texte.replace(de, par), "le modèle modifié").unwrap()
}

#[test]
fn chaque_modele_livre_se_lit_et_porte_les_mentions_de_l1242_12_et_r1221_34() {
    for f in std::fs::read_dir(MODELES).unwrap() {
        let p = f.unwrap().path();
        if p.extension().is_some_and(|e| e == "modele") {
            ModeleLu::du_fichier(&p).unwrap_or_else(|r| panic!("{} : {}", p.display(), r.message));
        }
    }
    for nature in ["cddu_spectacle", "cdd", "cdi"] {
        let nat = droit::natures::nature(Some(nature)).unwrap();
        let nom = nat.modele_contrat().unwrap();
        let chemin = Path::new(MODELES).join(&nom);
        assert!(nat.mentions_obligatoires().len() >= 15, "{} : ses mentions", nature);
        documents::contrat_ecrit::refuser_les_mentions_absentes(&chemin, &modele(&nom), &nat, &Valeurs::new())
            .unwrap_or_else(|e| panic!("{} : {}", nom, e));
    }
}

#[test]
fn un_modele_de_cdd_sans_son_motif_ni_le_terme_est_refuse() {
    let nat = droit::natures::nature(Some("cdd")).unwrap();
    let lu = modele("contrat-cdd.modele");
    let lu = sans(&sans(&lu, "{{contrat.motif}}", "un surcroît"), "{{contrat.fin}}", "la fin du projet");
    let propre = Path::new("/compagnie/gabarits/contrat-cdd.modele");
    let refus = documents::contrat_ecrit::refuser_les_mentions_absentes(propre, &lu, &nat, &Valeurs::new()).unwrap_err();
    assert!(refus.contains("{{contrat.motif}}") && refus.contains("L. 1242-12, premier alinéa"), "{}", refus);
    assert!(refus.contains("{{contrat.fin}}") && refus.contains("L. 1242-12, 2°"), "{}", refus);
}

#[test]
fn la_personne_remplacee_n_est_due_que_pour_un_remplacement() {
    let nat = droit::natures::nature(Some("cdd")).unwrap();
    let lu = sans(&modele("contrat-cdd.modele"), "{{contrat.personne_remplacee}}", "");
    let propre = Path::new("/compagnie/gabarits/contrat-cdd.modele");
    let vide = Valeurs::from([(Marqueur::ContratPersonneRemplacee, String::new())]);
    assert!(documents::contrat_ecrit::refuser_les_mentions_absentes(propre, &lu, &nat, &vide).is_ok());
    let avec = Valeurs::from([(Marqueur::ContratPersonneRemplacee, ", en remplacement de Mme X, chargée de production".to_string())]);
    let refus = documents::contrat_ecrit::refuser_les_mentions_absentes(propre, &lu, &nat, &avec).unwrap_err();
    assert!(refus.contains("L. 1242-12, 1°"), "{}", refus);
}
