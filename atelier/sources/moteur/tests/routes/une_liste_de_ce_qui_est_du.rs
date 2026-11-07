//! UNE SEULE LISTE DE CE QUI EST DU. L'echeancier dit tout, date ; les rappels disent ce qui est du maintenant. Les deux
//! se lisent dans la meme composition (`routes::rappels::echeances`) : a tout jour, ce que disent les rappels est dans
//! l'echeancier, et toute echeance de l'echeancier en retard, ou dont le jour est passe sans qu'elle soit faite, est dans
//! les rappels. Les Treteaux du Banc (3090 : le FCAP-SVP), FULLER embauche les 20 et 21 janvier 2026, puis rien.

use std::collections::BTreeSet;

use serde_json::{json, Value};

use crate::outils::rejeu::{rejouer_dossier, scenario};
use crate::outils::{dossier_de_test, reussi, route};

#[test]
fn les_rappels_et_l_echeancier_disent_la_meme_chose() {
    let d = dossier_de_test("une_liste_de_ce_qui_est_du", "treteaux");
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    let gestes = std::fs::read_to_string(scenario("treteaux-du-banc").join("gestes.jsonl")).unwrap();
    let debut: Vec<&str> = gestes.lines().take_while(|l| !l.contains("\"route\":\"apercu\"")).collect();
    std::fs::write(sc.join("gestes.jsonl"), debut.join("\n") + "\n").unwrap();
    let (_, c) = rejouer_dossier(&sc, &d);
    reussi(&crate::outils::valider(
        &c,
        "2026-01",
        None,
        &json!({"salarie": "FULLER", "nature": "cddu_spectacle", "spectacle": "le-banc-des-menteurs",
            "lignes": [{"rubrique": "cachet", "quantite": 2, "taux": 150, "lieu": null, "dates": ["2026-01-20", "2026-01-21"], "planning": []}]}),
        "2026-01-05",
    ));

    for jour in ["2026-01-10", "2026-01-25", "2026-06-20", "2027-01-20"] {
        let rappels: Vec<Value> = reussi(&route(&c, "rappels", json!({}), jour))["rappels"].as_array().cloned().unwrap_or_default();
        let e = reussi(&route(&c, "echeancier", json!({}), jour)).clone();
        let echeancier: Vec<&Value> = ["ce_mois", "a_venir"].iter().flat_map(|k| e[*k].as_array().into_iter().flatten()).collect();
        let cle = |x: &Value| x["cle"].as_str().unwrap_or_default().to_string();
        let dits: BTreeSet<String> = rappels.iter().map(cle).collect();
        let listes: BTreeSet<String> = echeancier.iter().map(|x| cle(x)).collect();
        let hors: Vec<&String> = dits.difference(&listes).collect();
        assert!(
            hors.is_empty(),
            "au {} : les rappels disent {:?}, que l'echeancier ne dit pas. Une echeance ne se compose qu'a un endroit (`rappels::echeances`) : \
             une liste qui en ajoute une a cote se ferme, et l'echeance s'ajoute a la composition.",
            jour,
            hors
        );
        let passee = |x: &Value| {
            x["etat"] == "en_retard" || (x["etat"] == "a_faire" && x["date"].as_str().is_some_and(|d| d < jour)) || x["rappel"] == true
        };
        let tues: Vec<String> = echeancier.iter().filter(|x| passee(x) && !dits.contains(&cle(x))).map(|x| cle(x)).collect();
        assert!(
            tues.is_empty(),
            "au {} : l'echeancier dit {:?} dues ou en retard, que les rappels taisent. Les rappels sont les echeances dues de la meme \
             composition (`rappels::echeances`, `due`) : une echeance passee et pas faite y est due.",
            jour,
            tues
        );
    }
}
