//! UN CONTRAT IMPRIME NE SE RELANCE PAS : il est parti en main propre, il n'y a pas de courriel a renvoyer. La relance
//! se refuse avant que la boite ou le mot de passe soient demandes, et rien n'est ecrit (ni ligne au journal, ni PDF).

use serde_json::json;

use crate::outils::rejeu::{rejouer_dossier, scenario};
use crate::outils::{dossier_de_test, noms, refus, reussi, route, valider_la_carte};

#[test]
fn la_relance_d_un_contrat_imprime_se_refuse_et_rien_n_est_ecrit() {
    let d = dossier_de_test("contrat_imprime", "relance");
    // Les Treteaux du Banc jusqu'a la saisie de novembre et la boite simulee (le geste « simuler ») : rien n'est parti.
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    let gestes = std::fs::read_to_string(scenario("treteaux-du-banc").join("gestes.jsonl")).unwrap();
    let debut: Vec<&str> = gestes.lines().take_while(|l| !l.contains("\"geste\":\"jour\"")).collect();
    std::fs::write(sc.join("gestes.jsonl"), debut.join("\n") + "\n").unwrap();
    let (_, c) = rejouer_dossier(&sc, &d);
    let jour = "2026-11-02";
    let contrat = json!({"mois": "2026-11", "salarie": "FULLER", "debut": "2026-11-20"});
    reussi(&valider_la_carte(&c, "2026-11", "FULLER", jour));
    reussi(&route(&c, "contrats-imprimer", json!({"mois": "2026-11", "contrats": [contrat]}), jour));

    let journal = std::fs::read(c.join("journal/depots.json")).unwrap();
    let pieces = noms(&c.join("sorties/2026-11/envoi"));
    // Plus fictive : sans boite et sans mot de passe, la relance refusee se dit avant de demander l'une ou l'autre.
    std::fs::remove_file(c.join("COPIE-DE-DEVELOPPEMENT.json")).unwrap();
    let r = route(&c, "demander-signature", json!({"mois": "2026-11", "contrats": [contrat], "relance": true}), jour);
    let (_, phrase) = refus(&r);
    assert!(phrase.contains("imprime"), "{}", phrase);
    assert_eq!(std::fs::read(c.join("journal/depots.json")).unwrap(), journal, "la relance refusee a ecrit au journal");
    assert_eq!(noms(&c.join("sorties/2026-11/envoi")), pieces, "la relance refusee a ecrit un PDF");
}
