//! LE REFUS DU CALCUL D'UN SALARIE EST LE SIEN : une paie ne lit, des engagements de son mois, que ceux de son salarie
//! (`engagement::etats::calculer_les_rangs`). Que la paie de l'un ne se calcule plus ne nomme pas un autre salarie, et
//! n'empeche pas sa paie.

use serde_json::json;

use crate::outils::rejeu::rejouer_le_premier_jour;
use crate::outils::{refus, reussi, route};

#[test]
fn le_refus_nomme_le_salarie_dont_la_paie_refuse() {
    let d = crate::outils::dossier_de_test("refus_d_un_salarie", "deux_salaries");
    let c = rejouer_le_premier_jour("faisceaux-sans-civilite", &d);
    let saisie = json!({"salarie": "FULLER", "nature": "cdd", "debut": "2026-03-02", "fin": "2026-05-29", "salaire_mensuel": 2500,
        "quotite_contrat": 151.67, "metier": "ART040", "motif_recours": "02"});
    reussi(&route(&c, "valider-engagement", json!({"mois": "2026-03", "saisie": saisie}), "2026-02-25"));
    // L'engagement ecrit de FULLER devient illisible (un fichier abime a la main) : sa paie de mai ne se calcule plus.
    let f = c.join("engagements").join("2026-03.json");
    let texte = std::fs::read_to_string(&f).unwrap();
    assert!(texte.contains("\"salaire_mensuel\": 2500"), "le salaire ecrit : {}", texte);
    std::fs::write(&f, texte.replace("\"salaire_mensuel\": 2500", "\"salaire_mensuel\": \"deux mille\"")).unwrap();

    let (_, phrase) = refus(&route(&c, "apercu", json!({"mois": "2026-05"}), "2026-05-20"));
    assert!(phrase.contains("FULLER") && !phrase.contains("SVOBODA"), "le refus est celui de FULLER : {}", phrase);
    let paies = std::fs::read_to_string(c.join("paies").join("2026-05.json")).unwrap();
    assert!(paies.contains("SV1.26.01"), "la paie de mai de SVOBODA s'est ecrite : {}", paies);
}
