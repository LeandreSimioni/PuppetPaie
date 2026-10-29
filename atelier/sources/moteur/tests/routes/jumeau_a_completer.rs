//! DEUX CONTRATS DU MEME SALARIE LE MEME JOUR, DONT L'UN EST A COMPLETER : la page compte le rang de chacun sur ses
//! cartes, celui a completer compris ; `etat-par-engagement` le donne dans le meme ordre, et `valider-engagement` valide
//! celui que ce rang designe, l'un comme l'autre.

use serde_json::{json, Value};

use crate::outils::rejeu::{rejouer_dossier, scenario};
use crate::outils::{dossier_de_test, refus, reussi, route, saisie};

#[test]
fn le_jumeau_calcule_se_valide_a_cote_de_celui_a_completer() {
    let d = dossier_de_test("jumeau_a_completer", "rang_a_l_ecran");
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    let gestes = std::fs::read_to_string(scenario("treteaux-du-banc").join("gestes.jsonl")).unwrap();
    let debut: Vec<&str> = gestes.lines().take_while(|l| !l.contains("\"geste\":\"jour\"")).collect();
    std::fs::write(sc.join("gestes.jsonl"), debut.join("\n") + "\n").unwrap();
    let (_, c) = rejouer_dossier(&sc, &d);
    let jour = "2026-11-02";

    // Devant le contrat de FULLER, un second contrat du meme jour auquel il manque un fait.
    let mut s = saisie(&c, "2026-11");
    let es = s["engagements"].as_array_mut().unwrap();
    let k = es.iter().position(|e| e["salarie"] == "FULLER").unwrap();
    let debut = es[k]["debut"].as_str().map(str::to_string).unwrap_or_else(|| "2026-11-20".into());
    es.insert(k, json!({"salarie": "FULLER", "nature": "cdd", "debut": debut, "fin": debut, "motif_recours": "02", "quotite_contrat": 7}));
    let r = reussi(&route(&c, "enregistrer-saisie", json!({"mois": "2026-11", "saisie": s}), jour)).clone();
    assert!(r["a_completer"][k].is_string(), "le second contrat devait être à compléter : {}", r);

    let lignes: Vec<Value> = reussi(&route(&c, "etat-par-engagement", json!({"mois": "2026-11"}), jour))["engagements"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|l| l["salarie"] == "FULLER" && l["debut"] == debut.as_str())
        .cloned()
        .collect();
    let rangs: Vec<(u64, bool)> = lignes.iter().map(|l| (l["rang"].as_u64().unwrap(), l["a_completer"].is_string())).collect();
    assert_eq!(rangs, vec![(0, true), (1, false)], "les rangs ne suivent pas l'ordre de l'écran : {:?}", lignes);

    let designe = |rang: u64| json!({"mois": "2026-11", "salarie": "FULLER", "debut": debut, "numero": "", "rang": rang, "valide": true});
    let (_, m) = refus(&route(&c, "valider-engagement", designe(0), jour));
    let manque = r["a_completer"][k].as_str().unwrap();
    assert!(m.contains(manque), "celui à compléter doit dire ce qui lui manque ({}) : {}", manque, m);
    let v = route(&c, "valider-engagement", designe(1), jour);
    let v = reussi(&v);
    assert!(v["valide"].is_string(), "le jumeau calculé n'est pas validé : {}", v);
}
