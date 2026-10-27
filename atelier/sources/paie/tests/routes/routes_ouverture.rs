//! `paie route` : les routes de l'ouverture, `rappels` (sans aucun calcul : les baremes se retirent pour le prouver) et
//! `apercu` (relu quand la saisie est enregistree, calcule en memoire sinon, rien d'ecrit, numeros reserves sans les
//! ecrire). Sur la configuration de `fictive-3090-petite` sans compteurs, et les mois de chaque test.

use crate::outils::{compagnie, dossier_de_test, ecrire, lire, reussi, route};
use socle::date::Jour;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn compagnie(nom: &str, mois: &[&str]) -> PathBuf {
    let racine = compagnie::saisies("fictive-3090-petite", mois, &dossier_de_test("routes_ouverture", nom).join("compagnie"));
    std::fs::remove_file(racine.join("config/compteurs.json")).unwrap();
    racine
}

fn rappels(c: &Path, aujourdhui: &str) -> Vec<Value> {
    reussi(&route(c, "rappels", json!({}), aujourdhui))["rappels"].as_array().unwrap().clone()
}

fn saisie(c: &Path, mois: &str) -> Value {
    crate::outils::saisie(c, &mois)
}

fn journal(c: &Path, lignes: Value) {
    ecrire(c, "journal/depots.json", &lignes);
}

/// Plus aucun bareme : un calcul de paie refuserait, un rappel n'en fait pas.
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

fn par_cle<'a>(r: &'a [Value], cle: &str) -> &'a Value {
    r.iter().find(|x| x["cle"] == json!(cle)).unwrap_or_else(|| panic!("pas de rappel {} dans {:?}", cle, r.iter().map(|x| &x["cle"]).collect::<Vec<_>>()))
}

// ============================================================ les rappels

#[test]
fn la_dsn_du_mois_d_avant_se_rappelle_sans_aucun_calcul() {
    let c = &compagnie("dsn", &["2026-08"]);
    sans_baremes(c);
    // Un engagement valide sur les trois : c'est le seul chiffre que le rappel lit des engagements.
    modifier(c, "2026-08", 1, "2026-09-10");
    modifier(c, "2026-08", 2, "2026-09-10");
    let r = rappels(c, "2026-09-10");
    let dsn = par_cle(&r, "dsn:2026-08");
    assert_eq!(dsn["quoi"], "dsn");
    assert_eq!(dsn["etat"], "a_produire");
    assert_eq!(dsn["en_retard"], json!(false));
    assert_eq!(dsn["engagements"], json!(3));
    assert_eq!(dsn["validees"], json!(1));
    assert_eq!(dsn["titre"], "Il est temps d'envoyer la DSN de aout 2026");
    assert!(dsn["detail"].as_str().unwrap().contains("3 engagements saisis, 1 validé. Le mois n'est pas encore écrit"));
    assert!(dsn["detail"].as_str().unwrap().contains("Il reste 2 engagements non validés."));
    // Passee l'echeance du 15, c'est un retard.
    let r = rappels(c, "2026-09-16");
    assert_eq!(par_cle(&r, "dsn:2026-08")["en_retard"], json!(true));
}

#[test]
fn une_dsn_reelle_acceptee_fait_taire_le_rappel_une_refusee_le_corrige() {
    let c = &compagnie("journal", &["2026-08"]);
    journal(c, json!([dsn_reelle("2026-08", "OK")]));
    assert!(rappels(c, "2026-09-10").iter().all(|x| x["quoi"] != "dsn"), "la DSN d'aout est passee");
    journal(c, json!([dsn_reelle("2026-08", "KO")]));
    let r = rappels(c, "2026-09-10");
    let dsn = par_cle(&r, "dsn:2026-08");
    assert_eq!(dsn["etat"], "a_corriger");
    assert!(dsn["detail"].as_str().unwrap().contains("REFUSÉE : controle test. À corriger et à renvoyer."));
}

#[test]
fn une_compagnie_sans_saisie_ni_dsn_n_est_en_retard_de_rien() {
    let c = &compagnie("neuve", &[]);
    // Une compagnie qui a des salaries (ou des paies, ou des DSN) a deja une histoire ;
    // celle-ci n'a rien : le premier mois du est le mois courant.
    ecrire(c, "config/salaries.json", &json!({}));
    assert!(rappels(c, "2026-09-30").is_empty());
}

/// UNE DSN SE DIT UNE FOIS A L'OUVERTURE (ticket #244) : celle qu'un rappel reclame (avec son geste, « Verifier et
/// envoyer ») est marquee `rappel` dans « Ce mois-ci » de l'echeancier, qui la garde pour sa date (l'etape DSN du mois la
/// lit) mais ne la compte plus dans ce qui reste a faire ; une DSN faite n'a ni rappel ni marque.
#[test]
fn la_dsn_qu_un_rappel_reclame_ne_se_redit_pas_dans_l_echeancier() {
    let c = &compagnie("une-fois", &["2026-08"]);
    let jour = "2026-10-20";
    let verifier = |c: &Path| {
        let r = rappels(c, jour);
        let ech = reussi(&route(c, "echeancier", json!({}), jour)).clone();
        let ce_mois = ech["ce_mois"].as_array().unwrap().clone();
        let dsn: Vec<&Value> = ce_mois.iter().filter(|x| x["quoi"] == "dsn").collect();
        assert!(!dsn.is_empty(), "{}", ech);
        for x in &dsn {
            let rappelee = r.iter().any(|y| y["quoi"] == "dsn" && y["cle"] == x["cle"]);
            assert_eq!(x["rappel"], json!(rappelee), "{} : marquee si et seulement si un rappel la reclame", x["cle"]);
            assert!(x["date"].is_string(), "{} : la date reste", x["cle"]);
        }
        let restent = ce_mois.iter().filter(|x| x["rappel"] != json!(true) && (x["etat"] == "a_faire" || x["etat"] == "en_retard")).count();
        assert_eq!(ech["a_faire"], json!(restent));
        (r, dsn.into_iter().cloned().collect::<Vec<Value>>())
    };
    let (r, dsn) = verifier(c);
    let septembre = dsn.iter().find(|x| x["cle"] == "dsn:2026-09").unwrap_or_else(|| panic!("{:?}", dsn));
    assert_eq!(septembre["rappel"], json!(true), "septembre manque : son rappel le dit, lui seul");
    assert!(r.iter().any(|y| y["cle"] == "dsn:2026-09" && y["en_retard"] == json!(true)));
    // Deposee et acceptee : faite, plus de rappel, plus de marque.
    journal(c, json!([dsn_reelle("2026-08", "OK"), dsn_reelle("2026-09", "OK")]));
    let (r, dsn) = verifier(c);
    assert!(r.iter().all(|y| y["quoi"] != "dsn"));
    assert!(dsn.iter().all(|x| x["rappel"] == json!(false)));
}
#[test]
fn la_fin_d_annee_cloture_exporte_et_revoit_les_spectacles() {
    let c = &compagnie("annee", &["2026-12"]);
    journal(c, json!([dsn_reelle("2026-12", "OK")]));
    let r = rappels(c, "2027-01-20");
    assert_eq!(par_cle(&r, "cloture:2026")["quoi"], "cloture");
    assert_eq!(par_cle(&r, "export-compta:2026")["quoi"], "export-compta");
    let revue = par_cle(&r, "revue-spectacles:2027");
    assert!(!revue["spectacles"].as_array().unwrap().is_empty());
    // Exercice clos : plus de cloture. Export fait apres la derniere saisie : plus d'export.
    ecrire(c, "config/exercices.json", &json!({ "2026": { "le": "2027-01-15", "qui": "essai" } }));
    std::fs::create_dir_all(c.join("sorties/exercices")).unwrap();
    std::fs::write(c.join("sorties/exercices/Compta_2026.csv"), "journal;date\n").unwrap();
    let r = rappels(c, "2027-01-20");
    assert!(r.iter().all(|x| x["quoi"] != "cloture" && x["quoi"] != "export-compta"));
    // La revue de l'annee cochee : elle se tait. Passe mars, elle ne se pose plus.
    ecrire(c, "config/echeancier.json", &json!({ "fait": { "revue-spectacles:2027": "x" } }));
    assert!(rappels(c, "2027-01-20").iter().all(|x| x["quoi"] != "revue-spectacles"));
    std::fs::remove_file(c.join("config/echeancier.json")).unwrap();
    assert!(rappels(c, "2027-04-02").iter().all(|x| x["quoi"] != "revue-spectacles"));
}

#[test]
fn le_rappel_d_une_fin_de_cdd_dit_le_preavis_de_son_motif_de_rupture() {
    // Le meme preavis que l'ecran des fins de contrat (`routes_listes`) : L1243-1 (aucun pour une faute grave), L1243-2
    // (la regle du salarie qui part pour un CDI, sans duree) ; textes dans `code-du-travail/articles-20261013.md`.
    let c = &compagnie("rappel-fin-cdd", &["2026-08"]);
    let mut s = saisie(c, "2026-08");
    for (salarie, motif) in [("LEMERCIER", "095"), ("BRIOCHE", "037")] {
        s["engagements"].as_array_mut().unwrap().push(json!({"salarie": salarie, "nature": "cdd", "debut": "2019-03-04",
            "fin": "2026-08-20", "numero": format!("{}.26.01", &salarie[..2]), "motif_rupture": motif}));
    }
    crate::outils::poser_saisie(c, "2026-08", &s);
    let r = rappels(c, "2026-08-24");
    let a_saisir = |salarie: &str| par_cle(&r, &format!("fin-contrat:{}:2026-08-20", salarie))["a_saisir"].to_string();
    let faute = a_saisir("LEMERCIER");
    assert!(faute.contains("La loi ne donne aucun preavis pour ce motif") && faute.contains("L1243-1"), "{}", faute);
    assert!(!faute.contains("La loi en donne"), "{}", faute);
    let cdi = a_saisir("BRIOCHE");
    assert!(cdi.contains("La loi en donne un que le moteur ne compte pas") && cdi.contains("L1243-2"), "{}", cdi);
    assert!(!cdi.contains("(L1234-1)") && !cdi.contains("c'est elle qui s'applique"), "{}", cdi);
}

#[test]
fn une_embauche_sans_trace_de_dpae_se_rappelle_dans_sa_fenetre() {
    let c = &compagnie("dpae", &["2026-10"]);
    let debut = saisie(c, "2026-10")["engagements"][0]["lignes"][0]["dates"][0].as_str().unwrap().to_string();
    let jour = Jour::lire(&debut).unwrap();
    // La veille de l'embauche : « il reste 1 jour ».
    let r = rappels(c, &jour.plus_jours(-1).unwrap().iso());
    let dpae = par_cle(&r, &format!("dpae:{}", debut));
    assert_eq!(dpae["quoi"], "dpae");
    assert_eq!(dpae["en_retard"], json!(false));
    assert!(dpae["detail"].as_str().unwrap().contains("il reste 1 jour."), "{}", dpae["detail"]);
    // Le jour meme, puis en retard.
    assert!(par_cle(&rappels(c, &debut), &format!("dpae:{}", debut))["detail"].as_str().unwrap().contains("L'embauche est aujourd'hui"));
    let apres = jour.plus_jours(1).unwrap().iso();
    let r = rappels(c, &apres);
    assert_eq!(par_cle(&r, &format!("dpae:{}", debut))["en_retard"], json!(true));
}

// =========================================================== l'apercu

const MOIS: &str = "2026-08";

fn apercu(c: &Path, saisie: &Value, lignes: bool) -> Value {
    route(c, "apercu", json!({ "saisie": saisie, "lignes": lignes }), "2026-09-10")
}

/// Remet en saisie l'engagement de rang `i` du mois (« Modifier… », la question du moteur acceptee).
fn modifier(c: &Path, mois: &str, i: usize, jour: &str) {
    let e = saisie(c, mois)["engagements"][i].clone();
    // La cle de la fiche, comme l'ecran la rend (les fiches de la fictive : le nom en capitales).
    let charge = json!({"mois": mois, "salarie": e["salarie"].as_str().unwrap().to_uppercase(), "debut": e["debut"], "numero": e["numero"], "valide": false, "confirme": true});
    reussi(&route(c, "valider-engagement", charge, jour));
}

#[test]
fn l_apercu_d_un_engagement_valide_lit_sa_paie_ecrite() {
    let c = &compagnie("relu", &[MOIS]);
    let saisie = saisie(c, MOIS);
    let premier = apercu(c, &saisie, false);
    assert_eq!(premier["ok"], json!(true), "{}", premier);
    let r = &premier["resultat"];
    assert_eq!(r["mois"], MOIS);
    assert_eq!(r["libelle"], "Aout 2026");
    assert_eq!(r["paies"].as_array().unwrap().len(), saisie["engagements"].as_array().unwrap().len());
    // Le numero de bulletin se reserve sans rien ecrire : compteurs absents -> 1/2026.
    assert_eq!(r["paies"][0]["numero_bulletin"], "1/2026");
    assert!(!c.join("config/compteurs.json").exists());

    // La paie ecrite fait foi : retouchee dans le fichier (son empreinte tient), l'apercu la montre telle quelle.
    let rel = format!("engagements/{}.json", MOIS);
    let mut f = lire(c, &rel);
    f["engagements"][0]["valide"]["paies"][MOIS]["paie"]["brut"] = json!("999999.99");
    ecrire(c, &rel, &f);
    let second = apercu(c, &saisie, true);
    assert_eq!(second["resultat"]["paies"][0]["brut"], "999999.99", "lue, pas recalculee");
    assert!(second["resultat"]["paies"][0]["lignes"].as_array().is_some_and(|l| !l.is_empty()), "`lignes` demande les cotisations");
    assert!(premier["resultat"]["paies"][0].get("lignes").is_none());
    // Le total ne se retouche pas : il se refait des paies lues.
    let somme: f64 = second["resultat"]["paies"].as_array().unwrap().iter().map(|p| p["brut"].as_str().unwrap().parse::<f64>().unwrap()).sum();
    assert!((second["resultat"]["total_brut"].as_str().unwrap().parse::<f64>().unwrap() - somme).abs() < 0.01);
}

#[test]
fn l_apercu_d_un_engagement_en_saisie_se_calcule_en_memoire_sans_rien_ecrire() {
    let c = &compagnie("memoire", &[MOIS]);
    let attendu = apercu(c, &saisie(c, MOIS), false);
    modifier(c, MOIS, 0, "2026-09-10");
    let avant = std::fs::read(c.join(format!("engagements/{}.json", MOIS))).unwrap();
    let saisie = saisie(c, MOIS);
    let r = apercu(c, &saisie, false);
    assert_eq!(r["resultat"]["paies"][0]["brut"], attendu["resultat"]["paies"][0]["brut"], "le calcul rend la paie validee");
    // Un nombre de plus ou de moins : une autre paie, calculee, rien d'ecrit.
    let mut autre = saisie.clone();
    autre["engagements"][0]["lignes"][0]["taux"] = json!(131);
    let r = apercu(c, &autre, false);
    assert_ne!(r["resultat"]["paies"][0]["brut"], attendu["resultat"]["paies"][0]["brut"]);
    assert_eq!(std::fs::read(c.join(format!("engagements/{}.json", MOIS))).unwrap(), avant);
}

#[test]
fn les_numeros_se_reservent_a_la_suite_des_compteurs_sans_les_ecrire() {
    let c = &compagnie("numeros", &[MOIS]);
    let compteurs = json!({ "2026": { "bulletin": 41 } });
    ecrire(c, "config/compteurs.json", &compteurs);
    let r = apercu(c, &saisie(c, MOIS), false);
    let paies = r["resultat"]["paies"].as_array().unwrap();
    for (i, p) in paies.iter().enumerate() {
        assert_eq!(p["numero_bulletin"], json!(format!("{}/2026", 42 + i)));
        assert!(p["numero_contrat"].as_str().unwrap().contains(".26."), "{}", p["numero_contrat"]);
    }
    assert_eq!(lire(c, "config/compteurs.json"), compteurs, "l'apercu n'ecrit aucun compteur");
}

#[test]
fn une_saisie_incomplete_est_un_refus_qui_garde_son_nom() {
    let c = &compagnie("incomplete", &[MOIS]);
    // L'apercu d'un engagement valide lit sa paie ecrite : celui-ci repasse en saisie, l'apercu le calcule.
    modifier(c, MOIS, 0, "2026-09-10");
    let mut s = saisie(c, MOIS);
    s["engagements"][0]["lignes"][0]["dates"] = json!([]);
    let r = apercu(c, &s, false);
    assert_eq!(r["ok"], json!(false));
    assert_eq!(r["classe"], "SaisieIncomplete");
    assert!(r["erreur"].as_str().unwrap().contains("ne dit aucun jour joue"));
    // Un bloc dont le spectacle n'est pas choisi : meme nom.
    let mut s = saisie(c, MOIS);
    s["engagements"][0].as_object_mut().unwrap().remove("spectacle");
    s.as_object_mut().unwrap().remove("spectacle");
    let r = apercu(c, &s, false);
    assert_eq!(r["classe"], "SaisieIncomplete");
    // Un salarie inconnu : un refus du moteur, sans le nom que la page reconnait.
    let mut s = saisie(c, MOIS);
    s["engagements"][0]["salarie"] = json!("Personne Inventee");
    let r = apercu(c, &s, false);
    assert_eq!(r["ok"], json!(false));
    assert!(r.get("classe").is_none());
}

#[test]
fn sans_les_organismes_le_bordereau_attend_la_paie_non() {
    let c = &compagnie("organismes", &[MOIS]);
    std::fs::remove_file(c.join("config/organismes.json")).unwrap();
    // Le bordereau ne se calcule que demande (`urssaf`, la boite « Fermer le mois »).
    let r = route(c, "apercu", json!({ "saisie": saisie(c, MOIS), "urssaf": true }), "2026-09-10");
    assert_eq!(r["resultat"]["urssaf"], Value::Null);
    assert!(r["resultat"]["avertissements"][0].as_str().unwrap().starts_with("Bordereau Urssaf pas encore calculable"));
    assert!(!r["resultat"]["paies"].as_array().unwrap().is_empty());
}
