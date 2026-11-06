//! LE REPAS ET LA NUITÉE EN DÉPLACEMENT, AU TARIF DE LA CONVENTION ET DANS LES LIMITES DE L'ARRÊTÉ.
//! - Une ligne sans taux prend le tarif de la convention : 3090, avenant du 21 juillet 2025
//!   (`atelier/reference/sources/legifrance-3090-avenant-salaires-20250721.md`, l. 429-432) : « Chambre et
//!   petit-déjeuner : 70,28 euros. Chaque repas principal : 17,85 euros. »
//! - Arrêté du 4 septembre 2025 (`atelier/reference/sources/dfs/arrete-2025-frais-professionnels/articles-20261103.md`),
//!   valeurs au 1er janvier 2026 (BOSS, Frais professionnels, `atelier/reference/sources/boss/boss-frais-professionnels-20261013.txt`) :
//!   le repas, 21,40 € quand le salarié est contraint de le prendre au restaurant, 10,40 € sinon (art. 3, 1° et 3° ;
//!   § 240-260) ; la nuitée, 76,60 € par jour à Paris et en petite couronne, 56,80 € ailleurs (art. 5, I, 1° ; § 1290) ;
//!   au-delà, la fraction excédentaire entre dans l'assiette sauf si l'employeur prouve son emploi (§ 270).
//! - Hors déduction forfaitaire spécifique, un artiste suit ces limites comme tout salarié : l'annexe 2 de l'arrêté ne
//!   joue qu'« en cas de déduction forfaitaire spécifique »
//!   (`atelier/reference/sources/dfs/annexes-2025/legi-arrete-20250904-frais-professionnels-annexe2-frais-hors-assiette-20250907.txt`, l. 5).
//! - Hors de la métropole : l'arrêté du 3 juillet 2006 (art. 1, a), et l'indemnité de mission à l'étranger de son annexe,
//!   réduite de 65 % quand le salarié est logé gratuitement (art. 2 ; BOSS § 1340-1390).
//! Les Tréteaux du Banc (3090), FULLER, comédienne sans DFS, en tournée en novembre 2026.

use serde_json::{json, Value};

use crate::outils::rejeu::{rejouer_dossier, scenario};
use crate::outils::{dec, dossier_de_test, reussi, route};

#[test]
fn le_repas_et_la_nuitee_au_dela_de_leur_limite_entrent_dans_l_assiette() {
    let d = dossier_de_test("frais_de_deplacement", "treteaux");
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    let gestes = std::fs::read_to_string(scenario("treteaux-du-banc").join("gestes.jsonl")).unwrap();
    let debut: Vec<&str> = gestes.lines().take_while(|l| !l.contains("\"route\":\"apercu\"")).collect();
    std::fs::write(sc.join("gestes.jsonl"), debut.join("\n") + "\n").unwrap();
    let (_, c) = rejouer_dossier(&sc, &d);

    let jours = json!(["2026-11-20", "2026-11-21"]);
    // La paie de la carte de FULLER aux lignes `lignes`, ecrites (`enregistrer-saisie`) puis lues par l'apercu.
    let carte = std::cell::RefCell::new(json!({"neuve": "cddu_spectacle"}));
    let apercu_de = |lignes: Vec<Value>| -> Value {
        let changements = json!([{"case": "salarie", "valeur": "FULLER"}, {"case": "spectacle", "valeur": "le-banc-des-menteurs"}, {"case": "lignes", "valeur": lignes}]);
        let carte_visee = carte.borrow().clone();
        let r = reussi(&route(&c, "enregistrer-saisie", json!({"mois": "2026-11", "carte": carte_visee, "changements": changements}), "2026-11-02")).clone();
        *carte.borrow_mut() = json!({"ecrite": r["engagement"]});
        reussi(&route(&c, "apercu", json!({"mois": "2026-11", "modifications": []}), "2026-11-02"))["paies"][0].clone()
    };
    // `frais` : les lignes de frais de la carte (« repas », « nuitee »), chacune avec ses cases cochées.
    let paie = |frais: &[(&str, &[&str])]| -> Value {
        let mut lignes = vec![json!({"rubrique": "cachet", "quantite": 2, "taux": 150, "lieu": null, "dates": jours, "planning": []})];
        for (rubrique, cases) in frais {
            lignes.push(json!({"rubrique": rubrique, "quantite": 2, "taux": null, "territoire": null, "cases": cases, "lieu": null, "dates": jours, "planning": []}));
        }
        apercu_de(lignes)
    };
    let ligne = |p: &Value, code: &str| p["remuneration"].as_array().and_then(|l| l.iter().find(|x| x["rubrique"] == code)).cloned();
    let n = |v: &Value| dec(v.as_str().unwrap());
    let cases = |p: &Value, i: u64| -> Vec<String> {
        p["cases"].as_array().unwrap().iter().filter(|c| c["ligne"] == i).map(|c| c["case"].as_str().unwrap().to_string()).collect()
    };
    let sans = paie(&[]);

    // Un repas sans nuitée, sans case : la limite du repas hors des locaux, 10,40 (7,45 au-delà, soumis).
    let repas = paie(&[("repas", &[])]);
    assert_eq!(n(&ligne(&repas, "repas").unwrap()["taux"]), dec("10.4"));
    assert_eq!(n(&ligne(&repas, "repas_au_dela").expect("la part du repas au-delà")["montant"]), dec("14.90"));
    assert_eq!(cases(&repas, 1), ["restaurant", "justificatif"]);
    // Au restaurant : 17,85 sous 21,40, hors assiette au tarif de la convention.
    let restaurant = paie(&[("repas", &["restaurant"])]);
    assert!(ligne(&restaurant, "repas_au_dela").is_none());
    assert_eq!((n(&ligne(&restaurant, "repas").unwrap()["taux"]), n(&restaurant["brut"])), (dec("17.85"), n(&sans["brut"])));

    // En tournée hors de Paris : le repas en grand déplacement à 21,40 sans case ; la nuitée à 56,80, 13,48 au-delà.
    let tournee = paie(&[("repas", &[]), ("nuitee", &[])]);
    assert!(ligne(&tournee, "repas_au_dela").is_none());
    assert_eq!(n(&ligne(&tournee, "nuitee").unwrap()["taux"]), dec("56.8"));
    assert_eq!(n(&ligne(&tournee, "nuitee_au_dela").expect("la part de la nuitée au-delà")["montant"]), dec("26.96"));
    assert_eq!((cases(&tournee, 1), cases(&tournee, 2)), (Vec::<String>::new(), vec!["paris".to_string(), "justificatif".into()]));
    // À Paris : 70,28 sous 76,60 ; tout est hors assiette.
    let paris = paie(&[("repas", &[]), ("nuitee", &["paris"])]);
    assert!(ligne(&paris, "nuitee_au_dela").is_none());
    assert_eq!(n(&paris["brut"]), n(&sans["brut"]));
    assert_eq!(n(&paris["net_a_payer"]) - n(&sans["net_a_payer"]), dec("176.26"));
    assert_eq!(n(&paris["cout_employeur"]), n(&sans["cout_employeur"]));
    // Le justificatif conservé : la fraction excédentaire reste hors de l'assiette.
    let justifie = paie(&[("repas", &[]), ("nuitee", &["justificatif"])]);
    assert!(ligne(&justifie, "nuitee_au_dela").is_none());
    assert_eq!(n(&justifie["brut"]), n(&sans["brut"]));

    // HORS DE LA MÉTROPOLE, le lieu de la ligne (`territoire`) dit la limite. `frais` : (rubrique, taux, lieu, cases).
    let ailleurs = |frais: &[(&str, &str, &str, &[&str])]| -> Value {
        let mut lignes = vec![json!({"rubrique": "cachet", "quantite": 2, "taux": 150, "lieu": null, "dates": jours, "planning": []})];
        for (rubrique, taux, lieu, cases) in frais {
            let q = if *rubrique == "repas" { 4 } else { 2 };
            lignes.push(json!({"rubrique": rubrique, "quantite": q, "taux": taux, "territoire": lieu, "cases": cases, "lieu": null, "dates": jours, "planning": []}));
        }
        apercu_de(lignes)
    };
    // Outre-mer : arrêté du 3 juillet 2006, art. 1, a)
    // (`atelier/reference/sources/arrete-2006-indemnites-de-mission/legifrance-arrete-20060703-indemnites-de-mission-20261103.txt`,
    // l. 17-30 ; BOSS § 1350-1360) : 120 € la nuit, 20 € le repas aux Antilles, 24 € en Nouvelle-Calédonie.
    let dom = ailleurs(&[("nuitee", "130", "dom", &[]), ("repas", "25", "dom", &[])]);
    assert_eq!((n(&ligne(&dom, "nuitee").unwrap()["taux"]), n(&ligne(&dom, "repas").unwrap()["taux"])), (dec("120"), dec("20")));
    assert_eq!(n(&ligne(&dom, "repas_au_dela").unwrap()["montant"]), dec("20.00"));
    let noumea = ailleurs(&[("repas", "25", "pacifique", &[])]);
    assert_eq!(n(&ligne(&noumea, "repas").unwrap()["taux"]), dec("24"));
    // À l'étranger : une limite par jour pour la nuitée et les repas, l'indemnité de mission du lieu (BOSS § 1380-1390 ;
    // annexe de l'arrêté : New York, du 1er septembre au 31 décembre, 490 USD), au taux de chancellerie du jour
    // (`atelier/reference/sources/dgfip-frais-de-mission/`, 0,8807 au 01/10/2026) : 431,54 € ; 400 + 2 × 60 = 520 €
    // par jour, 88,46 € au-delà, sur la nuitée. Logé par la compagnie, réduite de 65 % (art. 2) : 151,04 €.
    let new_york = ailleurs(&[("nuitee", "400", "NY", &[]), ("repas", "60", "NY", &[])]);
    assert_eq!(n(&ligne(&new_york, "nuitee_au_dela").unwrap()["montant"]), dec("176.92"));
    assert!(ligne(&new_york, "repas_au_dela").is_none());
    let loge = ailleurs(&[("repas", "90", "NY", &["loge"])]);
    assert_eq!(n(&ligne(&loge, "repas_au_dela").unwrap()["montant"]), dec("57.92"));
}
