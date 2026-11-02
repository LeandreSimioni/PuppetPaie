//! UN MOTIF DE RECOURS SE DECLARE PAR SON CODE : S21.G00.40.021 est une liste de codes de deux caracteres (cahier
//! technique DSN 2026.1, p. 194-195, `atelier/reference/norme-dsn/texte/bloc-S21.G00.40.txt`). Un CDD dont le motif
//! n'est pas l'un de ceux de sa nature (`contrats-base.json`) se refuse a la validation, et rien n'est ecrit.

use serde_json::json;

use crate::outils::rejeu::rejouer_le_premier_jour;
use crate::outils::{refus, route};

#[test]
fn un_motif_hors_de_la_liste_se_refuse_a_la_validation() {
    let d = crate::outils::dossier_de_test("motif_de_recours", "hors_liste");
    let c = rejouer_le_premier_jour("faisceaux-sans-civilite", &d);
    let saisie = json!({"salarie": "FULLER", "nature": "cdd", "debut": "2026-03-02", "fin": "2026-05-29", "salaire_mensuel": 2500,
        "quotite_contrat": 151.67, "metier": "ART040", "motif_recours": "accroissement"});
    let (_, phrase) = refus(&route(&c, "valider-engagement", json!({"mois": "2026-03", "saisie": saisie}), "2026-02-25"));
    assert!(phrase.contains("02 ("), "la phrase dit les motifs a choisir : {}", phrase);
    assert!(!c.join("engagements").join("2026-03.json").exists(), "rien n'est ecrit");
}
