//! Les obligations qu'un arret et une fin de contrat font naitre (`paie/src/obligations.rs`, `obligations-base.json`) :
//! chaque delai contre son texte depose, les quarante-huit heures qui sautent un dimanche et un ferie, l'etat « fait »
//! retenu. Sur `fictive-3090-petite` (config, aout 2026 ; elle chome tous les feries, `jours_feries_chomes: "tous"`).
//!
//! Les textes : CSS R. 133-14, II (`atelier/reference/sources/code-de-la-securite-sociale/xml/R133-14.xml`) et l'arrete du
//! 2 juillet 2013, art. 1er (`atelier/reference/sources/arrete-2013-delai-evenements-dsn/xml/1.xml`) : cinq jours ouvres
//! pour la fin du contrat et, sans subrogation, pour le debut et la fin de l'arret ; avec la DSN mensuelle du mois avec.
//! CSS R. 441-3 (`.../xml/R441-3.xml`) : la declaration d'accident « dans les quarante-huit heures non compris les dimanches
//! et jours feries » ; L. 441-2 : de « tout accident dont il a eu connaissance » ; R. 441-6 : les reserves, dix jours
//! francs. L. 461-5 et R. 461-5 : la maladie professionnelle se declare par la victime, en quinze jours.
//! Le deuil d'enfant (motif 19) : la page du compte entreprise de net-entreprises
//! (`atelier/reference/sources/accident-du-travail/net-entreprises-compte-entreprise-dat-20260331.txt`) -- la DSN ne couvre
//! pas « a ce jour » les conges pour deuil ; l'attestation de salaire (DSIJ) est « obligatoire pour tout arret de travail, quel
//! qu'en soit le motif », deuil d'enfant compris, « des reception de l'avis d'arret de travail », sur net-entreprises (ecran
//! ou EDI). CSS L. 331-9 (`.../code-de-la-securite-sociale/xml/L331-9.xml`) : les indemnites journalieres du conge de
//! deuil de C. trav. L. 3142-1-1 (`.../code-du-travail/xml/L3142-1-1.xml`).

use crate::outils::{compagnie, dossier_de_test, ecrire, lire, refus, reussi, route};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn compagnie(nom: &str) -> PathBuf {
    compagnie::saisies("fictive-3090-petite", &["2026-08"], &dossier_de_test("routes_obligations", nom).join("compagnie"))
}

/// Un arret de BRIOCHE, ecrit tel que `arret-enregistrer` l'ecrirait (`imprevus/<mois>.json`, genre arret).
fn arret(c: &Path, dernier: &str, motif: &str, subrogation: &str, accident: Option<&str>, reprise: Option<&str>, fin: &str) {
    let a = json!({
        "genre": "arret", "salarie": "BRIOCHE", "contrat_debut": "2026-01-02", "contrat_numero": "BR9.26.01", "motif": motif,
        "dernier_jour_travaille": dernier, "fin_previsionnelle": fin, "subrogation": subrogation,
        "subrogation_debut": null, "subrogation_fin": null, "reprise": reprise, "reprise_motif": reprise.map(|_| "01"),
        "date_accident": accident, "saisi_le": "2026-07-01 10:00", "qui": "essai", "note": "", "envois": [],
    });
    ecrire(c, &format!("imprevus/{}.json", &dernier[..7]), &json!({"mois": &dernier[..7], "imprevus": [a]}));
}

fn obligations(c: &Path, jour: &str) -> Vec<Value> {
    let r = reussi(&route(c, "arrets", json!({}), jour)).clone();
    r["arrets"][0]["obligations"].as_array().unwrap().clone()
}

fn la<'a>(o: &'a [Value], cle: &str) -> &'a Value {
    o.iter().find(|x| x["cle"] == json!(cle)).unwrap_or_else(|| panic!("pas d'obligation {} dans {:?}", cle, o.iter().map(|x| &x["cle"]).collect::<Vec<_>>()))
}

fn cles(o: &[Value]) -> Vec<&str> {
    o.iter().map(|x| x["cle"].as_str().unwrap()).collect()
}

// ---------------------------------------------------------------- l'accident

/// R. 441-3 : quarante-huit heures, dimanches et jours feries non compris. Su le samedi 23/05/2026 a 10 h : le samedi
/// compte (14 h), le dimanche 24 non, le lundi de Pentecote 25 (ferie, `feries-base.json`) non, le mardi 26 (24 h), et
/// le delai expire le mercredi 27 a 10 h -- sans les deux jours sautes, il expirait le lundi 25.
#[test]
fn les_quarante_huit_heures_sautent_le_dimanche_et_le_ferie() {
    let c = &compagnie("quarante-huit");
    arret(c, "2026-05-22", "06", "02", Some("2026-05-23"), None, "2026-06-10");
    // Sans connaissance saisie : le jour de l'accident, des minuit, presume et dit.
    let o = obligations(c, "2026-05-26");
    let dat = la(&o, "declaration_accident");
    assert_eq!(dat["depart"]["jour"], json!("2026-05-23"));
    assert_eq!(dat["depart"]["saisi"], json!(false));
    assert!(dat["depart"]["presume"].as_str().unwrap().contains("le jour de l'accident"), "{}", dat);
    // Samedi des minuit (24 h), dimanche et lundi ferie sautes, mardi (24 h) : le delai s'acheve le mardi 26 a minuit.
    assert_eq!((dat["butoir"].as_str(), dat["butoir_heure"].as_str()), (Some("2026-05-26"), Some("24:00")), "{}", dat);
    assert_eq!(dat["etat"], json!("a_faire"));
    assert!(dat["textes"].to_string().contains("xml/R441-3.xml"), "le delai cite son texte depose");
    assert!(dat["ou"].as_array().unwrap().iter().any(|x| x["code"] == json!("net-entreprises")));
    assert!(!dat["a_avoir"].as_array().unwrap().is_empty(), "ce qu'il faut avoir sous la main");

    // La connaissance saisie, avec l'heure : le samedi 23 a 10 h -> le mercredi 27 a 10 h.
    let r = route(c, "arret-connaissance", json!({"salarie": "Brioche", "dernier_jour": "2026-05-22", "de": "accident", "jour": "2026-05-23", "heure": "10:00"}), "2026-05-26");
    let dat = la(reussi(&r)["obligations"].as_array().unwrap(), "declaration_accident").clone();
    assert_eq!((dat["butoir"].as_str(), dat["butoir_heure"].as_str()), (Some("2026-05-27"), Some("10:00")), "{}", dat);
    assert_eq!(dat["depart"]["saisi"], json!(true));
    // Le 28, c'est en retard ; un jour de retard.
    let dat = la(&obligations(c, "2026-05-28"), "declaration_accident").clone();
    assert_eq!((dat["etat"].as_str(), dat["jours_restants"].as_i64()), (Some("en_retard"), Some(-1)), "{}", dat);

    // Un vendredi 18 h 30 (le 22/05) : vendredi 5 h 30, samedi 24 h, dimanche et lundi sautes, mardi 18 h 30.
    let r = route(c, "arret-obligations", json!({"motif": "04", "dernier_jour": "2026-05-22", "date_accident": "2026-05-22", "subrogation": "02",
        "connaissance_accident": "2026-05-22", "connaissance_heure": "18:30"}), "2026-05-22");
    let dat = la(reussi(&r)["obligations"].as_array().unwrap(), "declaration_accident").clone();
    assert_eq!((dat["butoir"].as_str(), dat["butoir_heure"].as_str()), (Some("2026-05-26"), Some("18:30")), "{}", dat);
}

/// L'etat « fait » : dit par la compagnie (`accident-declare`), garde dans `imprevus/obligations.json`, relu par la liste ; les
/// reserves motivees courent dix jours francs depuis la declaration (R. 441-6). Retiree, l'obligation redevient a faire.
#[test]
fn la_declaration_d_accident_faite_est_retenue() {
    let c = &compagnie("faite");
    arret(c, "2026-05-22", "04", "02", Some("2026-05-23"), None, "2026-06-10");
    let declarer = json!({"salarie": "Brioche", "dernier_jour": "2026-05-22", "le": "2026-05-25", "depot": "courrier", "reference": "1A 234 567 8901 2"});
    reussi(&route(c, "accident-declare", declarer, "2026-05-26"));
    let f = lire(c, "imprevus/obligations.json");
    assert_eq!(f["arrets"][0]["accident_declare"]["le"], json!("2026-05-25"));
    assert_eq!(f["arrets"][0]["accident_declare"]["depot"], json!("courrier"));
    let dat = la(&obligations(c, "2026-06-20"), "declaration_accident").clone();
    assert_eq!(dat["etat"], json!("fait"), "{}", dat);
    assert_eq!(dat["dans_les_temps"], json!(true));
    assert_eq!(dat["fait"]["reference"], json!("1A 234 567 8901 2"));
    let reserves = dat["suites"].as_array().unwrap().iter().find(|s| s["cle"] == json!("reserves")).unwrap().clone();
    assert_eq!(reserves["jusqu_au"], json!("2026-06-04"));
    assert!(reserves["texte"]["fichier"].as_str().unwrap().ends_with("xml/R441-6.xml"));
    // Le fichier des arrets lui-meme n'est pas touche.
    assert!(lire(c, "imprevus/2026-05.json")["imprevus"][0].get("accident_declare").is_none());

    reussi(&route(c, "accident-declare", json!({"salarie": "Brioche", "dernier_jour": "2026-05-22", "le": ""}), "2026-05-26"));
    assert_eq!(la(&obligations(c, "2026-05-26"), "declaration_accident")["etat"], json!("a_faire"));
    assert!(lire(c, "imprevus/obligations.json")["arrets"].as_array().unwrap().is_empty());
}

/// Ce qui ne se dit pas n'est pas ecrit : un arret qui n'est pas un accident, une declaration dans le futur ou avant
/// l'accident, un lieu inconnu, une connaissance avant l'accident.
#[test]
fn une_declaration_impossible_n_ecrit_rien() {
    let c = &compagnie("refus");
    arret(c, "2026-05-22", "06", "02", Some("2026-05-23"), None, "2026-06-10");
    let base = json!({"salarie": "Brioche", "dernier_jour": "2026-05-22", "le": "2026-05-25", "depot": "net-entreprises"});
    let avec = |k: &str, v: Value| {
        let mut b = base.clone();
        b[k] = v;
        b
    };
    for (nom, charge) in [
        ("dans le futur", avec("le", json!("2026-05-27"))),
        ("avant l'accident", avec("le", json!("2026-05-22"))),
        ("lieu inconnu", avec("depot", json!("pigeon voyageur"))),
        ("arret inconnu", avec("dernier_jour", json!("2026-05-21"))),
        ("date illisible", avec("le", json!("le 25 mai"))),
    ] {
        assert!(!refus(&route(c, "accident-declare", charge, "2026-05-26")).1.is_empty(), "{}", nom);
    }
    let connu = json!({"salarie": "Brioche", "dernier_jour": "2026-05-22", "de": "accident", "jour": "2026-05-20"});
    assert!(refus(&route(c, "arret-connaissance", connu, "2026-05-26")).1.contains("avant l'accident"));
    assert!(!c.join("imprevus/obligations.json").exists(), "un refus ne laisse aucun fichier");

    // Une maladie (01) n'appelle pas de declaration d'accident : ni la liste ne la rend, ni la route ne l'ecrit.
    arret(c, "2026-05-22", "01", "02", None, None, "2026-06-10");
    assert!(!cles(&obligations(c, "2026-05-26")).contains(&"declaration_accident"));
    assert!(refus(&route(c, "accident-declare", base, "2026-05-26")).1.contains("n'est pas un accident"));
    assert!(!c.join("imprevus/obligations.json").exists());
}

/// L. 461-5 et R. 461-5 : la maladie professionnelle (05) se declare par la victime, quinze jours apres la cessation du
/// travail ; la compagnie n'a pas de declaration d'accident a faire.
#[test]
fn la_maladie_professionnelle_se_declare_par_le_salarie() {
    let c = &compagnie("maladie-pro");
    arret(c, "2026-05-22", "05", "02", Some("2026-05-01"), None, "2026-06-10");
    let o = obligations(c, "2026-05-26");
    assert_eq!(cles(&o), ["signalement_arret", "declaration_maladie_professionnelle"]);
    let mp = la(&o, "declaration_maladie_professionnelle");
    assert_eq!((mp["qui"].as_str(), mp["etat"].as_str()), (Some("salarie"), Some("a_la_charge_du_salarie")));
    // Cessation du travail le 23/05 (lendemain du dernier jour travaille), quinze jours : le 07/06.
    assert_eq!(mp["butoir"], json!("2026-06-07"));
    assert!(mp["textes"].to_string().contains("xml/R461-5.xml"));
}

// ------------------------------------------------------- les signalements DSN

/// R. 133-14, II et l'arrete du 2 juillet 2013 : sans subrogation, cinq jours ouvres. Arret commence le vendredi 10/07/2026
/// (presume connu ce jour-la) : lundi 13, le 14 juillet chome saute, 15, 16, 17, lundi 20. La compagnie qui ne chome que le
/// 1er mai compte le 14 : vendredi 17.
#[test]
fn le_signalement_d_arret_sans_subrogation_court_cinq_jours_ouvres() {
    let c = &compagnie("cinq-jours");
    arret(c, "2026-07-09", "01", "02", None, None, "2026-07-31");
    let s = la(&obligations(c, "2026-07-13"), "signalement_arret").clone();
    assert_eq!(s["depart"]["jour"], json!("2026-07-10"));
    assert_eq!(s["butoir"], json!("2026-07-20"), "{}", s);
    assert_eq!(s["subrogation"], json!(false));
    assert!(s["textes"].to_string().contains("arrete-2013-delai-evenements-dsn/xml/1.xml"));
    let mut e = lire(c, "config/entreprise.json");
    e["jours_feries_chomes"] = json!("1er mai seulement");
    ecrire(c, "config/entreprise.json", &e);
    assert_eq!(la(&obligations(c, "2026-07-13"), "signalement_arret")["butoir"], json!("2026-07-17"));

    // Connu plus tard, le lundi 13 : le delai court de la.
    reussi(&route(c, "arret-connaissance", json!({"salarie": "BRIOCHE", "dernier_jour": "2026-07-09", "de": "arret", "jour": "2026-07-13"}), "2026-07-13"));
    let s = la(&obligations(c, "2026-07-13"), "signalement_arret").clone();
    assert_eq!((s["depart"]["saisi"].as_bool(), s["butoir"].as_str()), (Some(true), Some("2026-07-20")), "{}", s);

    // Le depot reel trace sur l'arret (`envois`, nature 04) : fait.
    let mut f = lire(c, "imprevus/2026-07.json");
    f["imprevus"][0]["envois"] = json!([{"nature": "04", "type": "01", "ordre": 3, "fichier": "x.dsn", "quand": "2026-07-15 09:12", "identifiant": ""}]);
    ecrire(c, "imprevus/2026-07.json", &f);
    let s = la(&obligations(c, "2026-07-16"), "signalement_arret").clone();
    assert_eq!((s["etat"].as_str(), s["fait"]["le"].as_str(), s["dans_les_temps"].as_bool()), (Some("fait"), Some("2026-07-15"), Some(true)), "{}", s);
}

/// R. 133-14, II : subroge, l'arret part au plus tard avec la DSN du mois ou il commence -- le 15 du mois suivant pour une
/// compagnie de moins de cinquante salaries (R. 243-6, `Contexte::echeance_dsn`), reporte comme elle (R. 133-14, I,
/// `atelier/reference/sources/code-de-la-securite-sociale/xml/R133-14.xml`) : le samedi 15/08/2026, Assomption, devient le
/// lundi 17/08/2026, a midi (cahier technique § 1.4.2, `atelier/reference/norme-dsn/texte/chapitre-1-la-norme.txt`).
#[test]
fn le_signalement_d_arret_subroge_part_avec_la_mensuelle() {
    let c = &compagnie("subroge");
    arret(c, "2026-07-09", "01", "01", None, None, "2026-07-31");
    let s = la(&obligations(c, "2026-07-13"), "signalement_arret").clone();
    assert_eq!((s["subrogation"].as_bool(), s["butoir"].as_str(), s["butoir_heure"].as_str()), (Some(true), Some("2026-08-17"), Some("12:00")), "{}", s);
    // Une subrogation qui n'est pas dite : le delai ne se devine pas.
    arret(c, "2026-07-09", "01", "", None, None, "2026-07-31");
    let s = la(&obligations(c, "2026-07-13"), "signalement_arret").clone();
    assert_eq!((s["etat"].as_str(), s["butoir"].clone()), (Some("indetermine"), Value::Null), "{}", s);
}

/// Cahier technique DSN 2026.1, § 5.4 (`atelier/reference/norme-dsn/texte/chapitre-5-modeles.txt`) : la reprise ne se signale
/// qu'anticipee ; alors cinq jours ouvres (R. 133-14, II, « la fin de l'arret »). Reprise le lundi 20/07 : 21 -> 27/07.
#[test]
fn seule_la_reprise_anticipee_fait_naitre_un_signalement() {
    let c = &compagnie("reprise");
    arret(c, "2026-07-09", "01", "02", None, Some("2026-07-20"), "2026-07-31");
    let r = la(&obligations(c, "2026-07-20"), "signalement_reprise").clone();
    assert_eq!((r["depart"]["jour"].as_str(), r["butoir"].as_str()), (Some("2026-07-20"), Some("2026-07-27")), "{}", r);
    arret(c, "2026-07-09", "01", "02", None, Some("2026-07-31"), "2026-07-31");
    assert!(!cles(&obligations(c, "2026-07-20")).contains(&"signalement_reprise"), "une reprise a la date prevue n'appelle rien");
}

// ---------------------------------------------------------- la fin de contrat

/// R. 133-14, II et l'arrete du 2 juillet 2013 (cahier technique § 5.2) : cinq jours ouvres apres la fin. Un CDD qui finit
/// le jeudi 20/08/2026 : vendredi 21, lundi 24 ... jeudi 27. Fait quand le journal note le depot REEL accepte de CE
/// signalement ; celui d'un autre salarie ne compte pas. Le rappel de l'accueil lit la meme obligation.
#[test]
fn la_fin_d_un_cdd_se_signale_en_cinq_jours_ouvres() {
    let c = &compagnie("fin");
    let mut s = lire(c, "paies/2026-08.json");
    s["engagements"].as_array_mut().unwrap().push(json!({"salarie": "LEMERCIER", "nature": "cdd", "debut": "2026-03-02",
        "fin": "2026-08-20", "numero": "LE1.26.01", "motif_rupture": "031", "preavis": {"type": "90"}}));
    ecrire(c, "paies/2026-08.json", &s);
    let fin = |jour: &str| reussi(&route(c, "fins-de-contrat", json!({"mois": "2026-08"}), jour))["fins"][0]["obligation"].clone();
    let o = fin("2026-08-24");
    assert_eq!((o["butoir"].as_str(), o["etat"].as_str(), o["jours_restants"].as_i64()), (Some("2026-08-27"), Some("a_faire"), Some(3)), "{}", o);
    assert!(o["textes"].to_string().contains("xml/R133-14.xml"));
    let o = fin("2026-08-28");
    assert_eq!(o["etat"], json!("en_retard"));
    let rappels = reussi(&route(c, "rappels", json!({}), "2026-08-28"))["rappels"].clone();
    let rappel = rappels.as_array().unwrap().iter().find(|x| x["cle"] == json!("fin-contrat:LEMERCIER:2026-08-20")).unwrap().clone();
    assert_eq!(rappel["en_retard"], json!(true), "{}", rappel);

    let ligne = |fichier: &str| json!({"quoi": "dsn", "quand": "2026-08-26T11:00:00", "origine": "", "mois": "2026-08-20", "fichier": fichier,
        "empreinte": null, "mode": "reel", "qui": "essai", "accepte": true, "destinataire": "net-entreprises", "flux": "F9",
        "retours": [], "reponse": "", "trace": null, "detail": {"nature": "07", "ordre": 4}});
    ecrire(c, "journal/depots.json", &json!([ligne("SignalementFinContrat_BRIOCHE_20260820.dsn")]));
    assert_eq!(fin("2026-08-28")["etat"], json!("en_retard"), "le signalement d'un autre salarie ne compte pas");
    ecrire(c, "journal/depots.json", &json!([ligne("SignalementFinContrat_LEMERCIER_20260820.dsn")]));
    let o = fin("2026-08-28");
    assert_eq!((o["etat"].as_str(), o["fait"]["le"].as_str(), o["dans_les_temps"].as_bool()), (Some("fait"), Some("2026-08-26"), Some(true)), "{}", o);
    let rappels = reussi(&route(c, "rappels", json!({}), "2026-08-28"))["rappels"].clone();
    assert!(rappels.as_array().unwrap().iter().all(|x| x["cle"] != json!("fin-contrat:LEMERCIER:2026-08-20")), "le rappel se tait");
}

// ---------------------------------------------------------- le deuil d'enfant

/// Motif 19 : la DSN ne couvre pas le conge de deuil (net-entreprises) -- ni signalement d'arret, ni de reprise anticipee ;
/// a leur place, l'attestation de salaire hors DSN, due « des reception de l'avis d'arret », soit -- inference dite -- le
/// jour meme ou la compagnie connait l'arret (presume au premier jour, le vendredi 10/07/2026 ; saisi, le lundi 13).
#[test]
fn le_deuil_fait_naitre_l_attestation_de_salaire_et_pas_le_signalement() {
    let c = &compagnie("deuil");
    arret(c, "2026-07-09", "19", "02", None, Some("2026-07-15"), "2026-07-17");
    let o = obligations(c, "2026-07-10");
    assert_eq!(cles(&o), ["attestation_salaire"], "ni signalement d'arret, ni de reprise anticipee");
    let at = la(&o, "attestation_salaire").clone();
    assert_eq!((at["depart"]["jour"].as_str(), at["depart"]["saisi"].as_bool()), (Some("2026-07-10"), Some(false)), "{}", at);
    assert_eq!((at["butoir"].as_str(), at["etat"].as_str(), at["jours_restants"].as_i64()), (Some("2026-07-10"), Some("a_faire"), Some(0)), "{}", at);
    assert_eq!(at["qui"], json!("employeur"));
    assert!(at["textes"].to_string().contains("accident-du-travail/net-entreprises-compte-entreprise-dat-20260331.txt"), "{}", at);
    assert!(at["textes"].to_string().contains("xml/L331-9.xml"), "{}", at);
    assert!(at["inferences"].to_string().contains("inférence"), "« des reception » n'est pas un delai chiffre : le moteur le dit");
    let ou: Vec<&str> = at["ou"].as_array().unwrap().iter().map(|x| x["code"].as_str().unwrap()).collect();
    assert_eq!(ou, ["net-entreprises-ecran", "net-entreprises-edi"]);
    assert_eq!(la(&obligations(c, "2026-07-13"), "attestation_salaire")["etat"], json!("en_retard"));

    // La connaissance saisie (le lundi 13) : c'est ce jour-la.
    reussi(&route(c, "arret-connaissance", json!({"salarie": "BRIOCHE", "dernier_jour": "2026-07-09", "de": "arret", "jour": "2026-07-13"}), "2026-07-13"));
    let at = la(&obligations(c, "2026-07-13"), "attestation_salaire").clone();
    assert_eq!((at["depart"]["saisi"].as_bool(), at["butoir"].as_str(), at["etat"].as_str()), (Some(true), Some("2026-07-13"), Some("a_faire")), "{}", at);

    // La liste ne le dit pas « a signaler ».
    let r = reussi(&route(c, "arrets", json!({}), "2026-07-13")).clone();
    assert_eq!((r["arrets"][0]["signalement_du"].as_bool(), r["arrets"][0]["etat"].as_str()), (Some(false), Some("sans signalement")), "{}", r["arrets"][0]);
    assert!(r["a_signaler"].as_array().unwrap().is_empty());

    // Ce qu'un arret en cours de saisie ferait naitre : la meme chose.
    let r = route(c, "arret-obligations", json!({"motif": "19", "dernier_jour": "2026-07-09", "subrogation": "02"}), "2026-07-10");
    assert_eq!(cles(reussi(&r)["obligations"].as_array().unwrap()), ["attestation_salaire"]);
    // Une maladie, elle, n'appelle pas l'attestation hors DSN : le signalement la porte.
    let r = route(c, "arret-obligations", json!({"motif": "01", "dernier_jour": "2026-07-09", "subrogation": "02"}), "2026-07-10");
    assert_eq!(cles(reussi(&r)["obligations"].as_array().unwrap()), ["signalement_arret"]);
}

/// L'attestation dite faite (`attestation-declare`) : gardee dans `imprevus/obligations.json`, relue par la liste, « fait » ;
/// retiree, l'obligation redevient a faire et la fiche ne garde que ce qu'elle dit encore.
#[test]
fn l_attestation_de_salaire_dite_faite_est_retenue_et_se_retire() {
    let c = &compagnie("attestation-faite");
    arret(c, "2026-07-09", "19", "02", None, None, "2026-07-17");
    reussi(&route(c, "arret-connaissance", json!({"salarie": "BRIOCHE", "dernier_jour": "2026-07-09", "de": "arret", "jour": "2026-07-13"}), "2026-07-13"));
    let dire = json!({"salarie": "Brioche", "dernier_jour": "2026-07-09", "le": "2026-07-13", "depot": "net-entreprises-ecran", "reference": "DSIJ 2026-0713-01"});
    let r = route(c, "attestation-declare", dire, "2026-07-14");
    assert_eq!(la(reussi(&r)["obligations"].as_array().unwrap(), "attestation_salaire")["etat"], json!("fait"), "{}", r);
    let f = lire(c, "imprevus/obligations.json");
    assert_eq!(f["arrets"][0]["attestation_salaire"]["le"], json!("2026-07-13"));
    assert_eq!(f["arrets"][0]["attestation_salaire"]["depot"], json!("net-entreprises-ecran"));
    let at = la(&obligations(c, "2026-07-20"), "attestation_salaire").clone();
    assert_eq!((at["etat"].as_str(), at["dans_les_temps"].as_bool()), (Some("fait"), Some(true)), "{}", at);
    assert_eq!(at["fait"]["reference"], json!("DSIJ 2026-0713-01"));
    assert!(at["phrase"].as_str().unwrap().contains("saisie à l'écran"), "{}", at);
    // Le fichier des arrets lui-meme n'est pas touche.
    assert!(lire(c, "imprevus/2026-07.json")["imprevus"][0].get("attestation_salaire").is_none());

    reussi(&route(c, "attestation-declare", json!({"salarie": "Brioche", "dernier_jour": "2026-07-09", "le": ""}), "2026-07-14"));
    assert_eq!(la(&obligations(c, "2026-07-13"), "attestation_salaire")["etat"], json!("a_faire"));
    let f = lire(c, "imprevus/obligations.json");
    assert!(f["arrets"][0].get("attestation_salaire").is_none() && f["arrets"][0].get("connaissance_arret").is_some(), "{}", f);
}

/// Ce qui ne se dit pas n'est pas ecrit : un jour futur ou illisible, un lieu que l'attestation ne connait pas (le courrier
/// est celui de la declaration d'accident), un arret inconnu, un jour avant la connaissance saisie de l'arret, un arret
/// que la DSN couvre (maladie, accident).
#[test]
fn une_attestation_impossible_n_ecrit_rien() {
    let c = &compagnie("attestation-refus");
    arret(c, "2026-07-09", "19", "02", None, None, "2026-07-17");
    let base = json!({"salarie": "Brioche", "dernier_jour": "2026-07-09", "le": "2026-07-13", "depot": "net-entreprises-edi"});
    let avec = |k: &str, v: Value| {
        let mut b = base.clone();
        b[k] = v;
        b
    };
    for (nom, charge) in [
        ("dans le futur", avec("le", json!("2026-07-15"))),
        ("lieu inconnu", avec("depot", json!("courrier"))),
        ("arret inconnu", avec("dernier_jour", json!("2026-07-08"))),
        ("date illisible", avec("le", json!("le 13 juillet"))),
    ] {
        assert!(!refus(&route(c, "attestation-declare", charge, "2026-07-14")).1.is_empty(), "{}", nom);
    }
    assert!(!c.join("imprevus/obligations.json").exists(), "un refus ne laisse aucun fichier");

    reussi(&route(c, "arret-connaissance", json!({"salarie": "BRIOCHE", "dernier_jour": "2026-07-09", "de": "arret", "jour": "2026-07-13"}), "2026-07-14"));
    let avant = std::fs::read(c.join("imprevus/obligations.json")).unwrap();
    assert!(refus(&route(c, "attestation-declare", avec("le", json!("2026-07-10")), "2026-07-14")).1.contains("avant que la compagnie ait connu l'arret"));
    assert_eq!(std::fs::read(c.join("imprevus/obligations.json")).unwrap(), avant, "rien n'est ecrit");

    // Une maladie (01), un accident (04) : la DSN les couvre, il n'y a pas d'attestation hors DSN a dire.
    for (motif, accident) in [("01", None), ("04", Some("2026-07-09"))] {
        arret(c, "2026-07-09", motif, "02", accident, None, "2026-07-17");
        assert!(!cles(&obligations(c, "2026-07-14")).contains(&"attestation_salaire"), "motif {}", motif);
        assert!(refus(&route(c, "attestation-declare", base.clone(), "2026-07-14")).1.contains("couvert par la DSN"), "motif {}", motif);
        assert_eq!(std::fs::read(c.join("imprevus/obligations.json")).unwrap(), avant, "motif {} : rien n'est ecrit", motif);
    }
    // Et le deuil n'est pas un accident : pas de declaration d'accident a dire.
    arret(c, "2026-07-09", "19", "02", None, None, "2026-07-17");
    assert!(refus(&route(c, "accident-declare", json!({"salarie": "Brioche", "dernier_jour": "2026-07-09", "le": "2026-07-13", "depot": "net-entreprises"}), "2026-07-14")).1.contains("n'est pas un accident"));
}

// ------------------------------------------------------ le rappel de l'ouverture

fn rappels(c: &Path, jour: &str) -> Vec<Value> {
    reussi(&route(c, "rappels", json!({}), jour))["rappels"].as_array().unwrap().clone()
}

fn rappel<'a>(r: &'a [Value], cle: &str) -> Option<&'a Value> {
    r.iter().find(|x| x["cle"] == json!(cle))
}

/// #205 : a l'ouverture, toute obligation due d'un arret se rappelle, avec la date butoir que `obligations.rs` lui donne et
/// ce qu'il faut pour la faire. L'accident de trajet du samedi 23/05/2026 (R. 441-3, quarante-huit heures sans dimanche ni
/// ferie : le mardi 26 a minuit) et le signalement de l'arret, sans subrogation (R. 133-14, II, et l'arrete du 2 juillet
/// 2013 : cinq jours ouvres apres le premier jour d'arret). En retard le 28, la declaration passe devant ; dite faite
/// (`accident-declare`), elle se tait, le signalement reste.
#[test]
fn les_obligations_d_un_arret_se_rappellent_a_l_ouverture() {
    let c = &compagnie("rappel-accident");
    arret(c, "2026-05-22", "06", "02", Some("2026-05-23"), None, "2026-06-10");
    let r = rappels(c, "2026-05-26");
    let dat = rappel(&r, "declaration_accident:BRIOCHE:2026-05-22")
        .unwrap_or_else(|| panic!("pas de rappel de la declaration : {:?}", r.iter().map(|x| &x["cle"]).collect::<Vec<_>>()));
    assert_eq!(
        (dat["quoi"].as_str(), dat["jour"].as_str(), dat["etat"].as_str(), dat["en_retard"].as_bool()),
        (Some("obligation"), Some("2026-05-26"), Some("a_faire"), Some(false)),
        "{}",
        dat
    );
    assert_eq!(dat["obligation"]["butoir_heure"], json!("24:00"));
    assert!(dat["obligation"]["textes"].to_string().contains("xml/R441-3.xml"), "la date butoir cite son texte depose");
    assert!(dat["obligation"]["ou"].as_array().unwrap().iter().any(|x| x["code"] == json!("net-entreprises")), "ou la faire");
    // Ce sur quoi elle porte : l'arret, resume comme la colonne du mois le lit, pour que le geste soit le meme.
    assert_eq!((dat["objet"]["genre"].as_str(), dat["objet"]["dernier_jour_travaille"].as_str()), (Some("arret"), Some("2026-05-22")), "{}", dat);
    let sig = rappel(&r, "signalement_arret:BRIOCHE:2026-05-22").expect("le signalement d'arret se rappelle aussi");
    let liste = reussi(&route(c, "arrets", json!({}), "2026-05-26"))["arrets"][0]["obligations"].clone();
    let butoir = liste.as_array().unwrap().iter().find(|o| o["cle"] == json!("signalement_arret")).unwrap()["butoir"].clone();
    assert_eq!(sig["jour"], butoir, "la meme date butoir que la colonne du mois : {}", sig);

    // En retard, la declaration passe devant tout ce qui ne l'est pas.
    let r = rappels(c, "2026-05-28");
    let i = r.iter().position(|x| x["cle"] == json!("declaration_accident:BRIOCHE:2026-05-22")).unwrap();
    assert!(r[i]["en_retard"].as_bool().unwrap());
    assert!(r[..i].iter().all(|x| x["en_retard"] == json!(true)), "le plus presse d'abord");

    // Dite faite, elle se tait ; le signalement, lui, n'est pas fait.
    let declarer = json!({"salarie": "Brioche", "dernier_jour": "2026-05-22", "le": "2026-05-26", "depot": "net-entreprises"});
    reussi(&route(c, "accident-declare", declarer, "2026-05-28"));
    let r = rappels(c, "2026-05-28");
    assert!(rappel(&r, "declaration_accident:BRIOCHE:2026-05-22").is_none(), "faite, elle ne se rappelle plus");
    assert!(rappel(&r, "signalement_arret:BRIOCHE:2026-05-22").is_some());
}

/// Ce qui n'est pas du par la compagnie ne se rappelle pas : la maladie professionnelle se declare par le salarie
/// (L. 461-5, R. 461-5). Le deuil d'enfant (motif 19, la page du compte entreprise de net-entreprises) : pas de
/// signalement, mais l'attestation de salaire, due le jour meme ou la compagnie connait l'arret -- elle se rappelle.
#[test]
fn seul_ce_qui_est_du_par_la_compagnie_se_rappelle() {
    let c = &compagnie("rappel-mp");
    arret(c, "2026-07-09", "05", "02", None, None, "2026-07-31");
    let r = rappels(c, "2026-07-10");
    assert!(r.iter().all(|x| !x["cle"].as_str().unwrap().starts_with("declaration_maladie_professionnelle:")), "{:?}", r.iter().map(|x| &x["cle"]).collect::<Vec<_>>());
    assert!(rappel(&r, "signalement_arret:BRIOCHE:2026-07-09").is_some());

    let c = &compagnie("rappel-deuil");
    arret(c, "2026-07-09", "19", "02", None, None, "2026-07-17");
    let r = rappels(c, "2026-07-10");
    let at = rappel(&r, "attestation_salaire:BRIOCHE:2026-07-09").expect("l'attestation du deuil se rappelle");
    assert_eq!((at["jour"].as_str(), at["etat"].as_str()), (Some("2026-07-10"), Some("a_faire")), "{}", at);
    assert!(rappel(&r, "signalement_arret:BRIOCHE:2026-07-09").is_none(), "le deuil ne se signale pas en DSN");
}
