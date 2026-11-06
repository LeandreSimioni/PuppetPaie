//! UN CDI NE SE REFAIT PAS D'UNE CARTE NEUVE : le debut ecrit sur la carte neuve d'un salarie dont le CDI commence le
//! meme jour et court deja se refuse et n'ecrit rien : un CDI revalide chaque mois reste un seul contrat, et le CDI se
//! modifie depuis sa carte.

use serde_json::json;

use crate::outils::rejeu::{rejouer_dossier, scenario};
use crate::outils::{dossier_de_test, identite, refus, reussi, route, saisir, valider};

#[test]
fn une_carte_neuve_ne_refait_pas_le_cdi_qui_court() {
    let d = dossier_de_test("second_contrat_au_mois", "cdi_revalide");
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    // Les Faisceaux jusqu'a la carte de janvier : SVOBODA, en CDI depuis le 05/01/2026.
    let gestes = std::fs::read_to_string(scenario("faisceaux-sans-civilite").join("gestes.jsonl")).unwrap();
    let gestes: Vec<&str> = gestes.lines().take_while(|l| !l.contains("\"geste\":\"jour\"")).collect();
    std::fs::write(sc.join("gestes.jsonl"), gestes.join("\n") + "\n").unwrap();
    let (_, c) = rejouer_dossier(&sc, &d);

    let saisie = json!({"salarie": "SVOBODA", "nature": "cdi", "debut": "2026-01-05", "salaire_mensuel": 3400, "quotite_contrat": 151.67});
    let id = json!(identite(&c, "2026-01", "SVOBODA", None));
    reussi(&valider(&c, "2026-01", Some(&id), &saisie, "2026-01-05"));

    let ecrits = || {
        let mut v: Vec<_> = std::fs::read_dir(c.join("engagements")).unwrap().map(|f| f.unwrap().path()).collect();
        v.sort();
        v.into_iter().map(|p| (p.clone(), std::fs::read(p).unwrap())).collect::<Vec<_>>()
    };
    // Une carte neuve de fevrier : son debut, qui dit le meme contrat, se refuse quand il arrive.
    let sans_debut = json!({"salarie": "SVOBODA", "nature": "cdi", "salaire_mensuel": 3400, "quotite_contrat": 151.67});
    let neuve = reussi(&saisir(&c, "2026-02", None, &sans_debut, "2026-02-02"))["engagement"].clone();
    let avant = ecrits();
    let (classe, phrase) = refus(&route(
        &c,
        "enregistrer-saisie",
        json!({"mois": "2026-02", "carte": {"ecrite": neuve}, "changements": [{"case": "debut", "valeur": "2026-01-05"}]}),
        "2026-02-02",
    ));
    assert_eq!(classe, "ErreurOperation", "{}", phrase);
    assert!(ecrits() == avant, "le refus a ecrit : {}", phrase);
}
