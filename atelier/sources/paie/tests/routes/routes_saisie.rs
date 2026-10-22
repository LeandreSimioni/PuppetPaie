//! `paie route` : `saisie` et `echeancier`, qui ne calculent rien (les baremes se retirent pour le prouver), et
//! `echeance-faite`, qui coche une echeance. Sur la configuration de `fictive-3090-petite` et les mois de chaque test,
//! sans compteurs ni echeance cochee.

use crate::outils::{compagnie, dossier_de_test, ecrire, lire, refus, reussi, route};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn compagnie(nom: &str, mois: &[&str]) -> PathBuf {
    let racine = compagnie::saisies("fictive-3090-petite", mois, &dossier_de_test("routes_saisie", nom).join("compagnie"));
    std::fs::remove_file(racine.join("config/compteurs.json")).unwrap();
    std::fs::remove_file(racine.join("config/echeancier.json")).unwrap();
    racine
}

fn saisie_ouverte(c: &Path, mois: &str, aujourdhui: &str) -> Value {
    reussi(&route(c, "saisie", json!({ "mois": mois }), aujourdhui)).clone()
}

fn echeancier(c: &Path, aujourdhui: &str) -> Value {
    reussi(&route(c, "echeancier", json!({}), aujourdhui)).clone()
}

fn fichier(c: &Path, mois: &str) -> Value {
    lire(c, &format!("paies/{}.json", mois))
}

fn journal(c: &Path, lignes: Value) {
    ecrire(c, "journal/depots.json", &lignes);
}

/// Plus aucun bareme : un calcul de paie refuserait, ces routes n'en font pas.
fn sans_baremes(c: &Path) {
    compagnie::avec_le_code(c, true);
}

fn dsn_reelle(mois: &str, verdict: &str) -> Value {
    json!({
        "quoi": "dsn", "quand": "2026-09-10T10:00:00", "mois": mois, "fichier": format!("DSN_{}.dsn", mois),
        "mode": "reel", "qui": "essai", "accepte": true, "flux": "F1",
        "retours": [{"verdict": verdict, "nature": "11", "anomalies": ["controle test"]}],
    })
}

fn par_cle<'a>(liste: &'a Value, cle: &str) -> &'a Value {
    liste.as_array().unwrap().iter().find(|x| x["cle"] == json!(cle)).unwrap_or_else(|| {
        panic!("pas d'echeance {} dans {:?}", cle, liste.as_array().unwrap().iter().map(|x| &x["cle"]).collect::<Vec<_>>())
    })
}

// ============================================================ la saisie

#[test]
fn un_mois_neuf_s_ouvre_vide_au_dernier_taux_saisi() {
    let c = &compagnie("neuf", &["2026-08"]);
    sans_baremes(c);
    // Le dernier taux saisi : celui de la derniere ligne de cachet du mois le plus recent.
    let attendu = fichier(c, "2026-08")["engagements"]
        .as_array()
        .unwrap()
        .iter()
        .rev()
        .flat_map(|e| e["lignes"].as_array().unwrap().iter().rev().cloned().collect::<Vec<_>>())
        .find(|l| l["rubrique"] == json!("cachet"))
        .unwrap()["taux"]
        .clone();
    let r = saisie_ouverte(c, "2026-10", "2026-10-04");
    assert_eq!(r["mois"], "2026-10");
    assert_eq!(r["existe"], json!(false));
    assert_eq!(r["hors_delai"], Value::Null);
    assert_eq!(r["saisie"]["engagements"], json!([]));
    assert_eq!(r["saisie"]["date_paiement"], "2026-10-31");
    assert_eq!(r["saisie"]["taux_cachet"], attendu);
    // Sans aucune paie dans le dossier : aucun taux (R12, plus les 130 de CoolPaie).
    let vide = &compagnie("neuf-vide", &[]);
    assert_eq!(saisie_ouverte(vide, "2026-02", "2026-02-10")["saisie"]["taux_cachet"], Value::Null);
    assert_eq!(saisie_ouverte(vide, "2026-02", "2026-02-10")["saisie"]["date_paiement"], "2026-02-28");
    assert_eq!(saisie_ouverte(vide, "2028-02", "2028-02-10")["saisie"]["date_paiement"], "2028-02-29");
}

/// #140 : les defauts que la page propose viennent du moteur -- le taux d'une ligne de cachet neuve (le dernier
/// cachet du mois, sinon le dernier saisi du dossier) et la date de paiement d'un mois qui n'en dit pas.
#[test]
fn le_moteur_dit_le_taux_et_la_date_de_paiement_par_defaut() {
    let c = &compagnie("defauts", &["2026-08"]);
    let mut s = fichier(c, "2026-08");
    s.as_object_mut().unwrap().remove("date_paiement");
    s["taux_cachet"] = json!(99.0);
    for e in s["engagements"].as_array_mut().unwrap() {
        for l in e["lignes"].as_array_mut().unwrap() {
            if l["rubrique"] == json!("cachet") {
                l["taux"] = json!(171.5);
            }
        }
    }
    ecrire(c, "paies/2026-08.json", &s);
    let r = saisie_ouverte(c, "2026-08", "2026-09-10");
    assert_eq!(r["defauts"], json!({"taux_cachet": 171.5, "date_paiement": "2026-08-31"}));
    // Un mois neuf : le dernier taux saisi du dossier, la fin du mois.
    let r = saisie_ouverte(c, "2026-09", "2026-09-10");
    assert_eq!(r["defauts"], json!({"taux_cachet": 171.5, "date_paiement": "2026-09-30"}));
    // Sans aucune ligne de cachet, le taux du mois.
    s["engagements"] = json!([]);
    ecrire(c, "paies/2026-08.json", &s);
    assert_eq!(saisie_ouverte(c, "2026-08", "2026-09-10")["defauts"]["taux_cachet"], json!(99.0));
}

/// R12 : le taux d'un cachet ne se devine pas (regle 1). Le moteur proposait 130 EUR quand le dossier n'avait encore
/// aucun cachet -- un chiffre herite de CoolPaie, qu'aucun texte ne fixe. Sans cachet saisi nulle part, il ne propose
/// rien (`null`, la page laisse le champ vide) ; des qu'un cachet existe, c'est le dernier saisi.
#[test]
fn sans_aucun_cachet_saisi_le_moteur_ne_propose_aucun_taux() {
    let c = &compagnie("sans-cachet", &[]);
    let r = saisie_ouverte(c, "2026-09", "2026-09-10");
    assert_eq!(r["saisie"]["taux_cachet"], Value::Null, "{}", r);
    assert_eq!(r["defauts"]["taux_cachet"], Value::Null, "{}", r);
    // Un mois de journees seulement, qui ne dit aucun taux de cachet : toujours rien.
    let journees = json!({"mois": "2026-08", "date_paiement": "2026-08-31", "engagements": [{
        "salarie": "BRIOCHE", "spectacle": "ubu-roi", "metier": "COM010",
        "lignes": [{"rubrique": "journee", "quantite": 1, "taux": 200.0, "dates": ["2026-08-20"]}]}]});
    ecrire(c, "paies/2026-08.json", &journees);
    let r = saisie_ouverte(c, "2026-09", "2026-09-10");
    assert_eq!((&r["saisie"]["taux_cachet"], &r["defauts"]["taux_cachet"]), (&Value::Null, &Value::Null), "{}", r);
    // Un cachet saisi : c'est lui qu'on propose.
    let mut cachet = journees.clone();
    cachet["engagements"][0]["lignes"] = json!([{"rubrique": "cachet", "quantite": 1, "taux": 145.0, "dates": ["2026-08-20"]}]);
    ecrire(c, "paies/2026-08.json", &cachet);
    let r = saisie_ouverte(c, "2026-09", "2026-09-10");
    assert_eq!((&r["saisie"]["taux_cachet"], &r["defauts"]["taux_cachet"]), (&json!(145.0), &json!(145.0)), "{}", r);
}

#[test]
fn passe_le_delai_le_mois_est_ferme() {
    let c = &compagnie("delai", &["2026-08"]);
    // La DSN d'aout est due le 15 septembre : le 15, le mois est ouvert ; le 16, il est ferme.
    assert_eq!(saisie_ouverte(c, "2026-08", "2026-09-15")["hors_delai"], Value::Null);
    let r = saisie_ouverte(c, "2026-08", "2026-09-16");
    assert_eq!(r["hors_delai"], "2026-09-15");
    assert_eq!(r["existe"], json!(true));
    // Decembre : l'echeance tombe en janvier de l'annee d'apres.
    assert_eq!(saisie_ouverte(c, "2026-12", "2027-01-16")["hors_delai"], "2027-01-15");
}

#[test]
fn les_noms_se_resolvent_et_les_lignes_viennent_du_moteur_sans_aucun_calcul() {
    let c = &compagnie("noms", &["2026-08"]);
    sans_baremes(c);
    let fiches = lire(c, "config/salaries.json");
    let cles: Vec<String> = fiches.as_object().unwrap().keys().filter(|k| !k.starts_with('_')).cloned().collect();
    // « Brioche » s'ecrit comme on veut dans le fichier ; la page recoit la cle de la fiche.
    let mut s = fichier(c, "2026-08");
    let nom = s["engagements"][0]["salarie"].as_str().unwrap().to_string();
    let cle = cles.iter().find(|k| k.to_lowercase() == nom.to_lowercase()).unwrap().clone();
    s["engagements"][0]["salarie"] = json!(format!("  {}", nom.to_lowercase()));
    ecrire(c, "paies/2026-08.json", &s);
    let r = saisie_ouverte(c, "2026-08", "2026-09-10");
    let e = &r["saisie"]["engagements"][0];
    assert_eq!(e["salarie"], json!(cle));
    // Les lignes sont celles du moteur : un nombre a virgule, les jours de la ligne de cachets,
    // et rien d'invente pour la ligne sans date.
    let l = e["lignes"].as_array().unwrap();
    assert_eq!(l[0]["rubrique"], "cachet");
    assert_eq!(l[0]["quantite"], json!(4.0));
    assert_eq!(l[0]["taux"], json!(155.0));
    assert_eq!(l[0]["dates"].as_array().unwrap().len(), 4);
    assert_eq!(l[0]["lieu"], "Fete fictive de la marionnette, Aurillac");
    assert_eq!(l[1]["rubrique"], "defraiement_forfait");
    assert!(l[1].get("dates").is_none() && l[1].get("lieu").is_none());
    // Le reste de la saisie est rendu tel quel.
    assert_eq!(r["saisie"]["taux_cachet"], s["taux_cachet"]);
    assert_eq!(e["debut"], s["engagements"][0]["debut"]);
}

/// #138 : la page reecrit les lignes que la route lui rend. Ce que le fichier
/// dit d'une ligne et que le moteur ne recalcule pas -- ses `heures` par unite,
/// le `genre` d'un avantage en nature, la `base` et le `taux_majoration` d'une
/// ligne en pourcentage -- revient tel quel, sinon l'enregistrement suivant le perd.
#[test]
fn les_heures_le_genre_et_la_base_d_une_ligne_reviennent_a_la_page() {
    let c = &compagnie("champs-de-ligne", &["2026-08"]);
    sans_baremes(c);
    let mut s = fichier(c, "2026-08");
    let lignes = s["engagements"][0]["lignes"].as_array_mut().unwrap();
    lignes.push(json!({"rubrique": "repetition", "quantite": 2, "taux": 50, "heures": 3}));
    lignes.push(json!({"rubrique": "majoration", "base": 300, "taux_majoration": 10}));
    lignes.push(json!({"rubrique": "avantage_nature", "quantite": 1, "taux": 40, "genre": "02"}));
    ecrire(c, "paies/2026-08.json", &s);
    let r = saisie_ouverte(c, "2026-08", "2026-09-10");
    let l = r["saisie"]["engagements"][0]["lignes"].as_array().unwrap().clone();
    let de = |code: &str| l.iter().find(|x| x["rubrique"] == json!(code)).unwrap_or_else(|| panic!("pas de {} dans {:?}", code, l)).clone();
    assert_eq!(de("repetition")["heures"], json!(3), "{:?}", l);
    assert_eq!(de("majoration")["base"], json!(300), "{:?}", l);
    assert_eq!(de("majoration")["taux_majoration"], json!(10), "{:?}", l);
    assert_eq!(de("avantage_nature")["genre"], json!("02"), "{:?}", l);
    // Une ligne qui ne les disait pas n'en recoit pas.
    assert!(de("cachet").get("heures").is_none() && de("cachet").get("genre").is_none());
}

#[test]
fn une_saisie_qui_ne_se_charge_pas_se_rend_telle_quelle() {
    let c = &compagnie("telle-quelle", &["2026-08"]);
    let mut s = fichier(c, "2026-08");
    s["engagements"][0]["salarie"] = json!("Personne d'inconnu");
    ecrire(c, "paies/2026-08.json", &s);
    // Le salarie ne se resout pas : c'est l'apercu qui dira pourquoi, avec son message a lui.
    assert_eq!(saisie_ouverte(c, "2026-08", "2026-09-10")["saisie"], s);
    // Un fichier qui n'est pas un dictionnaire aussi.
    std::fs::write(c.join("paies/2026-08.json"), "[1, 2]").unwrap();
    let r = saisie_ouverte(c, "2026-08", "2026-09-10");
    assert_eq!(r["saisie"], json!([1, 2]));
    // Le dossier ne se relit pas : aucun taux propose, la fin du mois reste dite.
    assert_eq!(r["defauts"], json!({"taux_cachet": null, "date_paiement": "2026-08-31"}));
}

#[test]
fn un_spectacle_du_mois_descend_en_cle_sur_ses_engagements() {
    let c = &compagnie("spectacle", &["2026-08"]);
    let mut s = fichier(c, "2026-08");
    let numero = s["engagements"][0]["spectacle"].clone();
    // La page recoit la cle de la fiche du spectacle, pas son numero d'objet, sur chaque engagement : elle montre le
    // spectacle d'un engagement, et n'ecrit que ce qu'elle montre (`routes_saisie::descendre_le_mois`).
    let fiches = lire(c, "config/spectacles.json");
    let cle = fiches.as_object().unwrap().iter().find(|(k, v)| !k.starts_with('_') && v["numero_objet"] == numero).unwrap().0.clone();
    for e in s["engagements"].as_array_mut().unwrap() {
        e.as_object_mut().unwrap().remove("spectacle");
    }
    s["spectacle"] = numero.clone();
    ecrire(c, "paies/2026-08.json", &s);
    let r = saisie_ouverte(c, "2026-08", "2026-09-10");
    assert!(r["saisie"].get("spectacle").is_none(), "{}", r["saisie"]);
    for e in r["saisie"]["engagements"].as_array().unwrap() {
        assert_eq!(e["spectacle"], json!(cle));
    }
    // Le spectacle du mois n'est pas ajoute quand la saisie n'en nommait pas.
    s.as_object_mut().unwrap().remove("spectacle");
    for e in s["engagements"].as_array_mut().unwrap() {
        e["spectacle"] = numero.clone();
    }
    ecrire(c, "paies/2026-08.json", &s);
    assert!(saisie_ouverte(c, "2026-08", "2026-09-10")["saisie"].get("spectacle").is_none());
}

#[test]
fn un_mois_illisible_ou_absent_de_la_charge_refuse() {
    let c = &compagnie("refus", &[]);
    for charge in [json!({}), json!({ "mois": "abcd-ef" }), json!({ "mois": "2026-13" })] {
        refus(&route(c, "saisie", charge, "2026-09-10"));
    }
    let c = &compagnie("refus-json", &["2026-08"]);
    std::fs::write(c.join("paies/2026-08.json"), "{ \"mois\": ").unwrap();
    refus(&route(c, "saisie", json!({ "mois": "2026-08" }), "2026-09-10"));
}

// ======================================================== l'echeancier

#[test]
fn la_date_la_bienvenue_et_la_dsn_du_mois_d_avant() {
    let c = &compagnie("echeancier", &["2026-08"]);
    sans_baremes(c);
    let r = echeancier(c, "2026-09-10");
    assert_eq!(r["date"], "jeudi 10 septembre 2026");
    assert_eq!(r["aujourdhui"], "2026-09-10");
    assert_eq!(r["bienvenue"], "Bienvenue sur PuppetPaie");
    assert_eq!(r["compagnie"], "Le Fil tendu (démonstration)");
    let dsn = par_cle(&r["ce_mois"], "dsn:2026-08");
    assert_eq!(dsn["titre"], "Déposer la DSN d'août 2026");
    assert_eq!(dsn["date"], "2026-09-15");
    assert_eq!(dsn["etat"], "a_faire");
    assert_eq!(dsn["mois"], "2026-09");
    assert_eq!(dsn["cochable"], json!(false));
    assert_eq!(dsn["annee"], Value::Null);
    // Le mardi 15/09/2026 est ouvre : pas de report, a midi (cahier technique § 1.4.2).
    assert!(dsn["detail"].as_str().unwrap().starts_with("Au plus tard le 15 septembre 2026 à midi. Elle part"), "{}", dsn["detail"]);
    // Ce qui reste a faire, hors la DSN qu'un rappel de l'ouverture reclame (elle se dit une fois, par lui : #244).
    let a_faire = |r: &Value| r["ce_mois"].as_array().unwrap().iter().filter(|x| x["rappel"] != json!(true) && (x["etat"] == "a_faire" || x["etat"] == "en_retard")).count();
    assert_eq!(r["a_faire"], json!(a_faire(&r)));
    // Passee l'echeance, elle est en retard ; septembre prend le relais, aout reste -- quand
    // la derniere DSN passee est celle de juillet (sans aucune DSN, on ne reclame que le mois d'avant).
    journal(c, json!([dsn_reelle("2026-07", "OK")]));
    let r = echeancier(c, "2026-10-04");
    assert_eq!(par_cle(&r["ce_mois"], "dsn:2026-08")["etat"], "en_retard");
    assert_eq!(par_cle(&r["ce_mois"], "dsn:2026-09")["etat"], "a_faire");
    // Les mois suivants se disent, un par un.
    assert_eq!(par_cle(&r["a_venir"], "dsn:2026-10")["titre"], "Déposer la DSN d'octobre 2026");
    assert_eq!(r["a_venir"].as_array().unwrap().iter().filter(|x| x["quoi"] == "dsn").count(), 12);
}

#[test]
fn une_dsn_reelle_acceptee_fait_l_echeance_une_refusee_la_rouvre() {
    let c = &compagnie("echeancier-journal", &["2026-08"]);
    journal(c, json!([dsn_reelle("2026-08", "OK")]));
    let r = echeancier(c, "2026-09-20");
    assert_eq!(par_cle(&r["ce_mois"], "dsn:2026-08")["etat"], "fait");
    journal(c, json!([dsn_reelle("2026-08", "KO")]));
    let r = echeancier(c, "2026-09-20");
    assert_eq!(par_cle(&r["ce_mois"], "dsn:2026-08")["etat"], "en_retard");
    // La DSN refusee reste a faire : dite par le rappel de l'ouverture qui la reclame, sinon comptee ici (#244).
    let rouverte = r["ce_mois"].as_array().unwrap().iter().filter(|x| x["rappel"] != json!(true) && (x["etat"] == "a_faire" || x["etat"] == "en_retard")).count();
    let aout = par_cle(&r["ce_mois"], "dsn:2026-08");
    assert!(aout["rappel"] == json!(true) || rouverte >= 1, "{}", aout);
    assert_eq!(r["a_faire"], json!(rouverte));
}

#[test]
fn les_echeances_de_janvier_se_cochent_et_se_trient() {
    let c = &compagnie("echeancier-janvier", &["2026-12"]);
    journal(c, json!([dsn_reelle("2026-12", "OK")]));
    let r = echeancier(c, "2027-01-12");
    let ce_mois = &r["ce_mois"];
    // La cloture de 2026 (des mois de 2026 sont dans le dossier), la revue des spectacles,
    // le taux AT/MP, le FCAP-SVP de la convention 3090.
    let cloture = par_cle(ce_mois, "cloture:2027");
    assert_eq!(cloture["titre"], "Clore l'année 2026");
    assert_eq!(cloture["etat"], "a_faire");
    assert_eq!(cloture["cochable"], json!(false));
    assert_eq!(cloture["annee"], "2027");
    assert_eq!(par_cle(ce_mois, "revue-spectacles:2027")["cochable"], json!(true));
    assert_eq!(par_cle(ce_mois, "fcap-svp:2027")["etat"], "a_faire");
    // En retard d'abord, puis ce qui reste a faire, l'information, le fait.
    let etats: Vec<&str> = ce_mois.as_array().unwrap().iter().map(|x| x["etat"].as_str().unwrap()).collect();
    let rang = |e: &str| ["en_retard", "a_faire", "information", "fait"].iter().position(|x| *x == e).unwrap();
    assert!(etats.windows(2).all(|p| rang(p[0]) <= rang(p[1])), "{:?}", etats);
    // Cocher la revue (une echeance que seul l'utilisateur sait faite), cloturer l'exercice.
    ecrire(c, "config/echeancier.json", &json!({ "fait": { "revue-spectacles:2027": { "le": "2027-01-05" } } }));
    ecrire(c, "config/exercices.json", &json!({ "2026": { "le": "2027-01-11", "qui": "essai" } }));
    let r = echeancier(c, "2027-01-12");
    assert_eq!(par_cle(&r["ce_mois"], "revue-spectacles:2027")["etat"], "fait");
    assert_eq!(par_cle(&r["ce_mois"], "cloture:2027")["etat"], "fait");
    // En fevrier, le FCAP-SVP (janvier a mars) se redit tant qu'il n'est pas fait ; la revue cochee se tait.
    let r = echeancier(c, "2027-02-10");
    assert_eq!(par_cle(&r["ce_mois"], "fcap-svp:2027")["etat"], "a_faire");
    assert!(r["ce_mois"].as_array().unwrap().iter().all(|x| x["cle"] != json!("revue-spectacles:2027") || x["etat"] == "fait"));
    // Passe mars, ce qui n'est pas fait est en retard.
    let r = echeancier(c, "2027-04-05");
    assert_eq!(par_cle(&r["ce_mois"], "fcap-svp:2027")["etat"], "en_retard");
}

#[test]
fn une_convention_n_a_que_ses_echeances_et_un_taux_atmp_pose_vaut_releve() {
    let c = &compagnie("echeancier-convention", &["2026-12"]);
    let mut e = lire(c, "config/entreprise.json");
    e["idcc"] = json!("1285");
    ecrire(c, "config/entreprise.json", &e);
    let r = echeancier(c, "2027-01-12");
    assert!(r["ce_mois"].as_array().unwrap().iter().all(|x| x["quoi"] != "fcap-svp"));
    assert_eq!(par_cle(&r["ce_mois"], "at-mp:2027")["etat"], "a_faire");
    // Le taux AT/MP pose dans la surcouche de l'annee : l'echeance est faite sans case a cocher.
    std::fs::create_dir_all(c.join("config/baremes")).unwrap();
    ecrire(c, "config/baremes/2027.json", &json!({ "annee": 2027, "taux_atmp": { "valeur": 1.25, "code_risque": "923AD" } }));
    let r = echeancier(c, "2027-01-12");
    assert_eq!(par_cle(&r["ce_mois"], "at-mp:2027")["etat"], "fait");
}

#[test]
fn un_journal_abime_refuse_l_echeancier() {
    let c = &compagnie("echeancier-abime", &["2026-08"]);
    std::fs::write(c.join("journal/depots.json"), "[{ pas du json").unwrap();
    refus(&route(c, "echeancier", json!({}), "2026-09-10"));
}

// ======================================================== echeance-faite

/// La compagnie avec l'echeancier de la compagnie fictive (des echeances deja cochees).
fn avec_son_echeancier(nom: &str) -> PathBuf {
    compagnie::saisies("fictive-3090-petite", &[], &dossier_de_test("routes_saisie", nom).join("compagnie"))
}

fn faite(racine: &Path, charge: Value) -> Value {
    route(racine, "echeance-faite", charge, "2026-10-03")
}

#[test]
fn une_echeance_se_coche_se_decoche_et_garde_l_ordre_du_fichier() {
    let racine = avec_son_echeancier("echeance");
    let avant: Vec<String> = lire(&racine, "config/echeancier.json")["fait"].as_object().unwrap().keys().cloned().collect();
    // Une echeance neuve se range en dernier, avec le jour du moteur et `interface`.
    let r = faite(&racine, json!({"cle": "fcap-svp:2027"}));
    assert_eq!(r["resultat"], json!({"cle": "fcap-svp:2027", "fait": true}), "{}", r);
    let f = lire(&racine, "config/echeancier.json");
    assert_eq!(f["fait"]["fcap-svp:2027"], json!({"le": "2026-10-03", "qui": "interface"}));
    let cles: Vec<String> = f["fait"].as_object().unwrap().keys().cloned().collect();
    assert_eq!(cles[..avant.len()], avant[..]);
    assert_eq!(cles.last().unwrap(), "fcap-svp:2027");
    // Decochee, elle disparait ; les autres restent dans leur ordre.
    let r = faite(&racine, json!({"cle": "at-mp:2025", "fait": false}));
    assert_eq!(r["resultat"], json!({"cle": "at-mp:2025", "fait": false}), "{}", r);
    let cles: Vec<String> = lire(&racine, "config/echeancier.json")["fait"].as_object().unwrap().keys().cloned().collect();
    assert!(!cles.contains(&"at-mp:2025".to_string()));
    let attendu: Vec<String> = avant.iter().filter(|c| *c != "at-mp:2025").cloned().chain(["fcap-svp:2027".to_string()]).collect();
    assert_eq!(cles, attendu);
    // Seul `false` decoche : `null` coche.
    let r = faite(&racine, json!({"cle": "afdas-2642:2027", "fait": null}));
    assert_eq!(r["resultat"]["fait"], json!(true), "{}", r);
    // Sans fichier, il se cree.
    std::fs::remove_file(racine.join("config/echeancier.json")).unwrap();
    assert_eq!(faite(&racine, json!({"cle": "at-mp:2026"}))["ok"], json!(true));
    let f = lire(&racine, "config/echeancier.json");
    assert!(f["_origine"].as_str().unwrap().starts_with("Les echeances que l'utilisateur a dites faites"));
    assert_eq!(f["fait"].as_object().unwrap().len(), 1);
}

#[test]
fn une_echeance_que_le_logiciel_voit_lui_meme_ou_qui_n_existe_pas_ne_se_coche_pas() {
    let racine = avec_son_echeancier("echeance-refus");
    let avant = std::fs::read(racine.join("config/echeancier.json")).unwrap();
    for cle in ["cloture:2026", "fnas-fcap-1285:2026", "inconnue:2026", "at-mp", "at-mp:abc", "at-mp:"] {
        let r = faite(&racine, json!({"cle": cle}));
        assert!(!refus(&r).1.is_empty(), "{}", cle);
    }
    assert_eq!(std::fs::read(racine.join("config/echeancier.json")).unwrap(), avant);
    assert!(refus(&faite(&racine, json!({}))).1.contains("cle"));
}
