//! UN CONTRAT AU MOIS CHANGE « A PARTIR D'UN MOIS » : le salaire d'un CDI dont la DSN de janvier est partie, et sa
//! limite passee, change a partir de fevrier. « Modifier l'engagement » pose une seule
//! question, « a partir de quel mois ? », qui nomme le salarie et dit ce qui arriverait a
//! janvier ; repondu fevrier, la paie de janvier reste celle qui est partie, fevrier prend le nouveau salaire, date du
//! 01/02 (le document qui dit la modification la date de sa prise d'effet : code du travail R1221-40,
//! `atelier/reference/sources/code-du-travail/articles-20261029.md`, l. 856), et la carte de janvier ne dit pas
//! « Modifié le… ». Ce qui vaut « a partir d'un mois » vaut pour ce mois et tous ceux d'apres, meme quand ils portaient
//! deja un autre chiffre. Le jour se dit quand ce n'est pas le 1er : le mois se coupe alors a ce jour.

use serde_json::{json, Value};

use crate::outils::rejeu::{rejouer_dossier, scenario};
use crate::outils::{dossier_de_test, reussi, route};

#[test]
fn le_salaire_d_un_cdi_parti_change_a_partir_du_mois_choisi() {
    let d = dossier_de_test("a_partir_du", "cdi_parti");
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    // Les Faisceaux, la personne que les organismes contactent dotee de sa civilite : la DSN de janvier part a l'echeance.
    let gestes = std::fs::read_to_string(scenario("faisceaux-sans-civilite").join("gestes.jsonl")).unwrap();
    let gestes: Vec<String> = gestes
        .lines()
        .map(|l| if l.contains("\"declarant_dsn\":{") { l.replacen("\"declarant_dsn\":{", "\"declarant_dsn\":{\"civilite\":\"01\",", 1) } else { l.to_string() })
        .collect();
    std::fs::write(sc.join("gestes.jsonl"), gestes.join("\n") + "\n").unwrap();
    let (r, c) = rejouer_dossier(&sc, &d);
    assert!(r.horloge.iter().any(|p| p.contains("DSN de janvier 2026") && !p.contains("non joué")), "{:?}", r.horloge);

    let jour = "2026-02-20";
    let lire = || -> Value {
        let e: Value = serde_json::from_slice(&std::fs::read(c.join("engagements/2026-01.json")).unwrap()).unwrap();
        e["engagements"][0].clone()
    };
    let paie_de_janvier = lire()["valide"]["paies"]["2026-01"]["paie"].clone();
    assert!(!paie_de_janvier.is_null(), "{}", lire());
    let svoboda = json!({"mois": "2026-01", "engagement": crate::outils::identite(&c, "2026-01", "SVOBODA", None)});
    let ligne_de_janvier = || -> Value {
        let l = reussi(&route(&c, "etat-par-engagement", json!({"mois": "2026-01"}), jour)).clone();
        l["engagements"].as_array().and_then(|l| l.iter().find(|x| x["salarie"] == "SVOBODA")).cloned().unwrap_or(l)
    };
    assert!(!ligne_de_janvier()["etats"]["documents"]["fait"].is_null(), "{}", ligne_de_janvier());
    // « Modifier l'engagement » avec le salaire de la carte ouverte : la question vient dans l'avertissement, et rien
    // n'est ecrit sans la reponse.
    let modifier = |salaire: u32, x: Value| {
        let saisie = reussi(&route(&c, "saisie", json!({"mois": "2026-01"}), jour))["saisie"]["engagements"][0].clone();
        let mut e = svoboda.clone();
        e["saisie"] = saisie;
        e["saisie"]["salaire_mensuel"] = json!(salaire);
        let mut charge = e;
        charge["mois"] = json!("2026-01");
        charge.as_object_mut().unwrap().extend(x.as_object().unwrap().clone());
        route(&c, "modifier-engagement", charge, jour)
    };
    let ecrits = || std::fs::read(c.join("engagements/2026-01.json")).unwrap();
    let avant = ecrits();
    let q = reussi(&modifier(3600, json!({}))).clone();
    assert!(ecrits() == avant, "la question a ecrit");
    assert!(!ligne_de_janvier()["etats"]["documents"]["fait"].is_null(), "{}", ligne_de_janvier()["etats"]);
    let question = &q["avertissement"]["a_partir_du"];
    let phrase = question["phrase"].as_str().unwrap_or_default();
    assert!(q["avertissement"]["engagements"][0]["qui"].as_str().is_some_and(|n| n.contains("SVOBODA Josef")), "{}", q);
    assert!(phrase.contains("janvier 2026"), "{}", q);
    let choix: Vec<&str> = question["choix"].as_array().unwrap().iter().map(|c| c["mois"].as_str().unwrap()).collect();
    assert!(choix.starts_with(&["2026-01", "2026-02"]), "{}", q);

    reussi(&modifier(3600, json!({"confirme": true, "a_partir_du": "2026-02"})));
    let e = lire();
    assert_eq!(e["valide"]["paies"]["2026-01"]["paie"], paie_de_janvier, "janvier a change");
    assert_eq!(e["salaire_mensuel"], json!(3400));
    assert_eq!(e["mois_suivants"]["2026-02"]["salaire_mensuel"], json!(3600));
    assert_eq!(e["mois_suivants"]["2026-02"]["date_modification"], json!("2026-02-01"));

    let etapes = reussi(&route(&c, "etat-par-engagement", json!({"mois": "2026-01"}), jour)).clone();
    let ligne = etapes["engagements"].as_array().and_then(|l| l.iter().find(|x| x["salarie"] == "SVOBODA")).cloned().unwrap_or(Value::Null);
    assert!(ligne.is_object(), "{}", etapes);
    assert!(ligne["versions"].is_null(), "janvier n'a pas change : {}", ligne["versions"]);

    // Modifie encore : 3 800 tape dans la carte de janvier, « a partir de fevrier », remplace les 3 600 que fevrier
    // portait. Rien de ce qui est tape ne se perd.
    reussi(&modifier(3800, json!({"confirme": true, "a_partir_du": "2026-02"})));
    let e = lire();
    assert_eq!(e["valide"]["paies"]["2026-01"]["paie"], paie_de_janvier, "janvier a change");
    assert_eq!(e["salaire_mensuel"], json!(3400));
    assert_eq!(e["mois_suivants"]["2026-02"]["salaire_mensuel"], json!(3800), "{}", e);

    // L'AVENANT AU JOUR : 4 000 « a partir de fevrier, le 16 ». L'avenant se date du 16/02 (R1221-40, ci-dessus), et
    // fevrier se coupe en deux parts, chacune payee pour ses heures (Cass. soc., 20/01/1999, n° 96-45.042 :
    // `atelier/reference/sources/mois-incomplet/legifrance-cass-soc-19990120-96-45042-20261030.md`) : 10 des 20 jours
    // ouvres de fevrier 2026 aux 3 800 d'avant, les 10 autres aux 4 000.
    reussi(&modifier(4000, json!({"confirme": true, "a_partir_du": "2026-02", "a_partir_du_jour": 16})));
    let e = lire();
    assert_eq!(e["valide"]["paies"]["2026-01"]["paie"], paie_de_janvier, "janvier a change");
    let fevrier = &e["mois_suivants"]["2026-02"];
    assert_eq!(fevrier["date_modification"], json!("2026-02-16"), "{}", e);
    assert_eq!(fevrier["avant"]["salaire_mensuel"], json!(3800), "{}", e);
    let montants: Vec<&str> = e["valide"]["paies"]["2026-02"]["paie"]["remuneration"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|l| l["code"] == "mois")
        .map(|l| l["montant"].as_str().unwrap())
        .collect();
    assert_eq!(montants, ["1900", "2000"], "{}", e["valide"]["paies"]["2026-02"]["paie"]["remuneration"]);
}
