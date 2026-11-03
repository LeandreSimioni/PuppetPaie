//! UNE DECLARATION NE LIT QUE L'ENGAGEMENT VALIDE : un engagement en saisie du mois ne change rien a la DSN. Ici, un
//! second engagement de FULLER sur un autre metier, en saisie : la DSN de novembre declare son seul contrat valide en
//! emploi unique (S21.G00.40.036 = 01, `atelier/reference/norme-dsn/texte/bloc-S21.G00.40.txt`), comme sans lui.

use serde_json::json;

use crate::outils::rejeu::{rejouer_dossier, scenario};
use crate::outils::{dossier_de_test, individu, reussi, route, saisie, valeur, valider_la_carte};

#[test]
fn un_engagement_en_saisie_ne_change_pas_la_dsn() {
    let d = dossier_de_test("brouillon", "dsn");
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    let gestes = std::fs::read_to_string(scenario("treteaux-du-banc").join("gestes.jsonl")).unwrap();
    let debut: Vec<&str> = gestes.lines().take_while(|l| !l.contains("\"geste\":\"jour\"")).collect();
    std::fs::write(sc.join("gestes.jsonl"), debut.join("\n") + "\n").unwrap();
    let (_, c) = rejouer_dossier(&sc, &d);
    let jour = "2026-12-01";
    reussi(&valider_la_carte(&c, "2026-11", "FULLER", jour));
    let dsn = || {
        let r = route(&c, "depot-apercu", json!({"quoi": "dsn", "mois": "2026-11"}), jour);
        std::fs::read_to_string(reussi(&r)["fichier"].as_str().unwrap()).unwrap()
    };
    let seule = dsn();

    let s = saisie(&c, "2026-11");
    let spectacle = s["engagements"][0]["spectacle"].clone();
    let nouveau = json!({
        "salarie": "FULLER", "spectacle": spectacle, "metier": "ACC010",
        "lignes": [{"rubrique": "cachet", "quantite": 1, "taux": 150, "dates": ["2026-11-27"]}],
    });
    reussi(&route(&c, "enregistrer-saisie", json!({"mois": "2026-11", "saisie": nouveau}), jour));
    let avec = dsn();

    let emploi = |texte: &str| valeur(&individu(texte, "FULLER"), "S21.G00.40.036");
    assert_eq!(emploi(&seule), Some("01".to_string()), "{}", seule);
    assert_eq!(emploi(&avec), Some("01".to_string()), "l'engagement en saisie compte comme un second emploi");
    assert_eq!(individu(&avec, "FULLER"), individu(&seule, "FULLER"));
}
