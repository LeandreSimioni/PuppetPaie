//! DEUX SALARIES DE MEME NOM ET DE MEME PRENOM SONT DEUX SALARIES : la fiche recapitulative du dossier de chacun ne
//! porte que ses contrats et ses cumuls, et le cout par spectacle a une ligne pour chacun, reconnus par la cle de sa
//! fiche, jamais ceux de l'autre.

use serde_json::{json, Value};

use crate::outils::rejeu::{rejouer_dossier, scenario};
use crate::outils::{dossier_de_test, ecrire, lire, reussi, route, saisie};

#[test]
fn un_homonyme_ne_porte_rien_de_l_autre() {
    let d = dossier_de_test("homonymes", "fiche_recapitulative");
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    let gestes = std::fs::read_to_string(scenario("treteaux-du-banc").join("gestes.jsonl")).unwrap();
    let debut: Vec<&str> = gestes.lines().take_while(|l| !l.contains("\"geste\":\"jour\"")).collect();
    std::fs::write(sc.join("gestes.jsonl"), debut.join("\n") + "\n").unwrap();
    let (_, c) = rejouer_dossier(&sc, &d);
    let jour = "2026-11-30";

    // L'homonyme : la fiche de FULLER, un autre NIR, d'autres initiales de contrat.
    let mut salaries = lire(&c, "config/salaries.json");
    let mut autre = salaries["FULLER"].clone();
    let valeur = |v: &str| json!([{"valeur": v}]);
    for (champ, v) in [("nir", "2820144999013"), ("nir_cle", "35"), ("initiales_contrat", "FV"), ("code_salarie", "2")] {
        autre[champ] = valeur(v);
    }
    salaries["FULLER-2"] = autre;
    ecrire(&c, "config/salaries.json", &salaries);

    let mut s = saisie(&c, "2026-11");
    let mut deux = s["engagements"].as_array().unwrap().iter().find(|e| e["salarie"] == "FULLER").unwrap().clone();
    deux["salarie"] = json!("FULLER-2");
    deux["debut"] = json!("2026-11-24");
    deux["fin"] = json!("2026-11-26");
    deux.as_object_mut().unwrap().remove("numero");
    deux.as_object_mut().unwrap().remove("valide");
    deux["lignes"] = json!([{"rubrique": "cachet", "quantite": 3, "taux": 160, "dates": ["2026-11-24", "2026-11-25", "2026-11-26"]}]);
    s["engagements"].as_array_mut().unwrap().push(deux);
    reussi(&route(&c, "enregistrer-saisie", json!({"mois": "2026-11", "saisie": s}), jour));
    for (salarie, debut) in [("FULLER", "2026-11-20"), ("FULLER-2", "2026-11-24")] {
        reussi(&route(&c, "valider-engagement", json!({"mois": "2026-11", "salarie": salarie, "debut": debut}), jour));
    }
    let numeros: Vec<(String, String)> = lire(&c, "engagements/2026-11.json")["engagements"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| (e["salarie"].as_str().unwrap().to_string(), e["numero"].as_str().unwrap().to_string()))
        .collect();
    let numero = |k: &str| numeros.iter().find(|(s, _)| s == k).unwrap().1.clone();

    let fiche = |cle: &str| -> String {
        let r = reussi(&route(&c, "dossier-salarie", json!({"salaries": [cle], "annee": "2026", "categories": ["recapitulatif"]}), jour)).clone();
        let chemin = r["fichiers"].as_array().unwrap().iter().filter_map(Value::as_str).find(|f| f.contains("Fiche-recapitulative")).unwrap().to_string();
        fabrique_pdf::lecture::texte(&chemin).unwrap_or_else(|e| panic!("{} illisible : {}", chemin, e.0)).0.join("\n")
    };
    for (lui, l_autre) in [("FULLER", "FULLER-2"), ("FULLER-2", "FULLER")] {
        let t = fiche(lui);
        assert!(t.contains(&numero(lui)), "la fiche de {} n'a pas son contrat {} :\n{}", lui, numero(lui), t);
        assert!(!t.contains(&numero(l_autre)), "la fiche de {} porte le contrat {} de {} :\n{}", lui, numero(l_autre), l_autre, t);
    }

    // Le cout par spectacle : une ligne par salarie, reconnu par la cle de sa fiche.
    let r = reussi(&route(&c, "exporter", json!({"etat": "cout-par-spectacle", "annee": "2026", "apercu": true}), jour)).clone();
    let lignes: Vec<Value> = r["sections"].as_array().unwrap().iter().flat_map(|s| s["lignes"].as_array().cloned().unwrap_or_default()).collect();
    let nom = salaries["FULLER"]["nom"][0]["valeur"].as_str().unwrap().to_string();
    let siennes = lignes.iter().filter(|l| l["salarie"].as_str().is_some_and(|s| s.starts_with(&nom))).count();
    assert_eq!(siennes, 2, "le coût par spectacle mêle les deux homonymes : {:?}", lignes);
    let cles: Vec<String> = lignes.iter().filter_map(|l| l["cle_salarie"].as_str().map(str::to_string)).collect();
    for lui in ["FULLER", "FULLER-2"] {
        assert_eq!(cles.iter().filter(|k| *k == lui).count(), 1, "le coût par spectacle n'a pas une ligne pour {} : {:?}", lui, cles);
    }
}
