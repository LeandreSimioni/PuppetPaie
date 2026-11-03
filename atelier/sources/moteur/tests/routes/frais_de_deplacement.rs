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
    // `frais` : les lignes de frais de la carte (« repas », « nuitee »), chacune avec ses cases cochées.
    let paie = |frais: &[(&str, &[&str])]| -> Value {
        let mut lignes = vec![json!({"rubrique": "cachet", "quantite": 2, "taux": 150, "dates": jours})];
        for (rubrique, cases) in frais {
            let mut l = json!({"rubrique": rubrique, "quantite": 2, "taux": "", "dates": jours});
            for k in *cases {
                l[*k] = json!(true);
            }
            lignes.push(l);
        }
        let saisie = json!({"salarie": "FULLER", "spectacle": "le-banc-des-menteurs", "lignes": lignes});
        reussi(&route(&c, "apercu", json!({"mois": "2026-11", "cartes": [{"saisie": saisie}]}), "2026-11-02"))["paies"][0].clone()
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
}
