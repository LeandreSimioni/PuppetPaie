//! Les options de la charge de la route `ouverture` (`Options::de_la_charge`) : les defauts, les valeurs de travers.

use paie::ouverture::Options;
use serde_json::json;

#[test]
fn les_options_de_la_charge() {
    let d = Options::de_la_charge(&json!({}));
    assert!(d.sauvegarder && d.retours && d.signatures && d.filet && d.incidents_editeur);
    assert!(!d.veille && !d.sauvegarde_seule);
    assert_eq!((d.jours, d.pas_avant, d.racine_drive.as_str()), (30, 0, "PuppetPaie"));
    let o = Options::de_la_charge(&json!({
        "sauvegarder": false, "retours": false, "signatures": false, "filet": false, "jours": 7, "racine_drive": "Autre", "pas_avant": 15, "veille": true,
        "incidents_editeur": false, "sauvegarde_seule": true, "sources": false, "maj": false
    }));
    assert!(!o.sauvegarder && !o.retours && !o.signatures && !o.filet && o.veille && !o.incidents_editeur && o.sauvegarde_seule);
    assert_eq!((o.jours, o.pas_avant, o.racine_drive.as_str()), (7, 15, "Autre"));
    // Des valeurs de travers : les defauts, jamais une erreur.
    let o = Options::de_la_charge(&json!({"jours": "abc", "pas_avant": null, "racine_drive": "", "sauvegarder": null, "veille": 0, "filet": "oui"}));
    assert_eq!((o.jours, o.pas_avant, o.racine_drive.as_str()), (30, 0, "PuppetPaie"));
    assert!(o.sauvegarder && !o.veille && o.filet);
    assert_eq!(Options::de_la_charge(&json!({"jours": 7.9})).jours, 7);
    for charge in [json!(null), json!("x"), json!([1]), json!(3)] {
        assert_eq!(Options::de_la_charge(&charge).jours, 30);
    }
}

