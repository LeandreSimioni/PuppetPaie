//! Les routes qui ECRIVENT une fiche ou une saisie : `activer-fiche`, `enregistrer-fiche`, `fiche-oublier-demande`,
//! `poser-identite`, `retirer-identite`, `enregistrer-saisie`, `ajuster`, `metier-retenu`, `metiers-ajoutes`. Ce qui
//! s'ecrit, avec sa copie `avant/` ; ce qui refuse n'ecrit rien. Sur `fictive-3090-petite` (config, journal, mars 2025 et
//! novembre 2026).

use crate::outils::{compagnie, dossier_de_test, lire, noms};
use paie::retours::base64_ecrire as base64;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn compagnie(nom: &str) -> PathBuf {
    compagnie::config_et_mois("fictive-3090-petite", &["2025-03", "2026-11"], &dossier_de_test("routes_ecritures", nom).join("compagnie"))
}

fn route(racine: &Path, nom: &str, charge: Value) -> Value {
    crate::outils::route(racine, nom, charge, "2026-10-03")
}

/// Les fichiers d'un dossier dont le nom commence par `debut`.
fn nommes(dossier: &Path, debut: &str) -> Vec<String> {
    noms(dossier).into_iter().filter(|n| n.starts_with(debut)).collect()
}

const PNG: [u8; 41] = [
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0x0d, 0x49, 0x48, 0x44, 0x52, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 0x1f, 0x15, 0xc4,
    0x89, 0, 0, 0, 0, 0x49, 0x45, 0x4e, 0x44,
];

#[test]
fn activer_fiche_archive_puis_reactive_et_garde_une_copie() {
    let racine = compagnie("activer");
    let r = route(&racine, "activer-fiche", json!({"nom": "salarie", "cle": "BRIOCHE", "actif": false}));
    assert_eq!(r["ok"], json!(true), "{}", r);
    assert_eq!(lire(&racine, "config/salaries.json")["BRIOCHE"]["actif"], json!(false));
    assert!(r["resultat"]["configuration"].is_object());
    assert_eq!(nommes(&racine.join("config/avant"), "salaries.json.").len(), 1);
    let r = route(&racine, "activer-fiche", json!({"nom": "salarie", "cle": "BRIOCHE", "actif": true}));
    assert_eq!(r["ok"], json!(true), "{}", r);
    assert_ne!(lire(&racine, "config/salaries.json")["BRIOCHE"]["actif"], json!(false));
}

#[test]
fn activer_fiche_refuse_ce_qui_n_est_pas_une_fiche() {
    let racine = compagnie("activer-refus");
    let avant = std::fs::read(racine.join("config/salaries.json")).unwrap();
    for charge in [
        json!({"nom": "salarie", "cle": "INCONNU", "actif": true}),
        json!({"nom": "entreprise", "cle": "x", "actif": true}),
    ] {
        let r = route(&racine, "activer-fiche", charge);
        assert_eq!(r["ok"], json!(false), "{}", r);
    }
    assert_eq!(std::fs::read(racine.join("config/salaries.json")).unwrap(), avant);
}

#[test]
fn une_demande_de_fiche_se_clot_dans_le_journal_sans_toucher_aux_autres() {
    let racine = compagnie("oublier");
    let base = |code: &str| {
        json!({"quoi": "fiche-demande", "quand": "2026-09-01T10:00:00", "origine": "", "mois": null, "fichier": null,
               "empreinte": null, "mode": "reel", "qui": "interface", "accepte": true, "destinataire": "x@y.fr",
               "flux": "", "retours": [], "reponse": "", "trace": null,
               "detail": {"code": code, "adresse": "a@b.fr", "prenom": "Jo", "message": "m"}})
    };
    std::fs::create_dir_all(racine.join("journal")).unwrap();
    std::fs::write(racine.join("journal/depots.json"), json!([base("AAAA-1"), base("BBBB-2")]).to_string()).unwrap();
    let r = route(&racine, "fiche-oublier-demande", json!({"code": "AAAA-1"}));
    assert_eq!(r["ok"], json!(true), "{}", r);
    // La reponse ne rend plus la configuration : ni NIR ni IBAN des fiches (la page relit `configuration`).
    let texte = r.to_string();
    let salaries = lire(&racine, "config/salaries.json");
    let mut vus = 0;
    for (_, f) in salaries.as_object().unwrap() {
        for cle in ["nir", "iban"] {
            if let Some(v) = f.get(cle).and_then(Value::as_str).filter(|v| !v.is_empty()) {
                vus += 1;
                assert!(!texte.contains(v), "la reponse porte un {} : {}", cle, texte);
            }
        }
    }
    assert!(vus > 0, "le test ne voit aucun NIR ni IBAN a chercher");
    assert!(r["resultat"].get("configuration").is_none(), "{}", r);
    // La demande se CLOT : une ligne `fiche-recue` marquee `abandon`, les autres lignes restent.
    let journal = lire(&racine, "journal/depots.json");
    assert_eq!(journal.as_array().unwrap().len(), 3);
    assert_eq!(journal[2]["quoi"], json!("fiche-recue"));
    assert_eq!(journal[2]["detail"], json!({"code": "AAAA-1", "abandon": true}));
    assert_eq!(journal[1]["detail"]["code"], json!("BBBB-2"));
    let r = route(&racine, "fiche-oublier-demande", json!({"code": "AAAA-1"}));
    assert_eq!(r["ok"], json!(false), "une demande close ne s'oublie pas deux fois : {}", r);
    let r = route(&racine, "fiche-oublier-demande", json!({"code": "ZZZZ"}));
    assert_eq!(r["ok"], json!(false), "{}", r);
}

#[test]
fn l_identite_se_pose_sous_un_nom_de_la_table_et_se_retire_en_avant() {
    let racine = compagnie("identite");
    let dossier = racine.join("gabarits/identite");
    let r = route(&racine, "poser-identite", json!({"quoi": "signature", "contenu": base64(&PNG)}));
    assert_eq!(r["ok"], json!(true), "{}", r);
    assert_eq!(std::fs::read(dossier.join("signature.png")).unwrap(), PNG.to_vec());
    // Le format se verifie sur les octets, pas sur ce que le client dit.
    let r = route(&racine, "poser-identite", json!({"quoi": "signature", "contenu": base64(b"%PDF-1.4\n")}));
    assert_eq!(r["ok"], json!(false), "{}", r);
    // Un nom hors de la table n'ecrit rien, ni ailleurs.
    let r = route(&racine, "poser-identite", json!({"quoi": "../x", "contenu": base64(&PNG)}));
    assert_eq!(r["ok"], json!(false), "{}", r);
    assert!(!racine.join("gabarits/x.png").exists());
    let r = route(&racine, "retirer-identite", json!({"quoi": "signature"}));
    assert_eq!(r["ok"], json!(true), "{}", r);
    assert!(!dossier.join("signature.png").exists());
    assert_eq!(nommes(&dossier, "signature.png.avant").len(), 1);
}

#[test]
fn un_metier_se_coche_et_se_decoche_dans_les_preferences() {
    let racine = compagnie("metiers");
    let r = route(&racine, "metiers-ajoutes", json!({"codes": ["ZZZ"]}));
    assert_eq!(r["ok"], json!(false), "un metier inconnu est refuse : {}", r);
    let r = route(&racine, "metier-retenu", json!({"code": "ZZZ", "retenu": true}));
    assert_eq!(r["ok"], json!(false), "{}", r);
    assert!(!racine.join("config/metiers-ajoutes.json").exists(), "un refus n'ecrit rien");
    let r = route(&racine, "metiers-ajoutes", json!({"codes": []}));
    assert_eq!(r["ok"], json!(true), "{}", r);
    assert!(r["resultat"]["emplois"].is_array() && r["resultat"]["emplois_autres"].is_array(), "{}", r);
}

#[test]
fn une_fiche_en_erreur_est_refusee_avec_ses_anomalies_et_rien_n_est_ecrit() {
    let racine = compagnie("fiche-refus");
    let avant = std::fs::read(racine.join("config/salaries.json")).unwrap();
    let mut donnees = lire(&racine, "config/salaries.json")["BRIOCHE"].clone();
    donnees["nir"] = json!("123");
    let r = route(&racine, "enregistrer-fiche", json!({"nom": "salarie", "cle": "BRIOCHE", "donnees": donnees}));
    assert_eq!(r["ok"], json!(false), "{}", r);
    assert_eq!(r["classe"], json!("FicheRefusee"), "{}", r);
    assert!(r["anomalies"].as_array().is_some_and(|a| !a.is_empty()), "{}", r);
    assert_eq!(std::fs::read(racine.join("config/salaries.json")).unwrap(), avant);
}

#[test]
fn une_fiche_valide_s_ecrit_avec_sa_copie_et_rend_la_configuration() {
    let racine = compagnie("fiche-ok");
    let mut donnees = lire(&racine, "config/salaries.json")["BRIOCHE"].clone();
    donnees["prenom_affichage"] = json!("Fanchon-Marie");
    let r = route(&racine, "enregistrer-fiche", json!({"nom": "salarie", "cle": "BRIOCHE", "donnees": donnees}));
    assert_eq!(r["ok"], json!(true), "{}", r);
    assert_eq!(r["resultat"]["fiche"]["cle"], json!("BRIOCHE"));
    assert!(r["resultat"]["configuration"].is_object());
    assert_eq!(lire(&racine, "config/salaries.json")["BRIOCHE"]["prenom_affichage"], json!("Fanchon-Marie"));
    assert_eq!(nommes(&racine.join("config/avant"), "salaries.json.").len(), 1);
}

#[test]
fn une_saisie_s_enregistre_a_l_octet_pres_et_un_mois_clos_se_refuse() {
    let racine = compagnie("saisie");
    // Novembre 2026 est ouvert : la saisie relue et renvoyee telle quelle s'ecrit sans changer un octet.
    let avant = std::fs::read(racine.join("paies/2026-11.json")).unwrap();
    let r = route(&racine, "enregistrer-saisie", json!({"mois": "2026-11", "saisie": lire(&racine, "paies/2026-11.json"), "forcer": true}));
    assert!(r["resultat"]["chemin"].as_str().is_some_and(|c| c.ends_with("2026-11.json")), "{}", r);
    assert_eq!(std::fs::read(racine.join("paies/2026-11.json")).unwrap(), avant);
    // 2025 est clos (`config/exercices.json`) : mars refuse, meme force, et rien ne s'ecrit.
    let avant = std::fs::read(racine.join("paies/2025-03.json")).unwrap();
    let r = route(&racine, "enregistrer-saisie", json!({"mois": "2025-03", "saisie": lire(&racine, "paies/2025-03.json"), "forcer": true}));
    assert_eq!(r["ok"], json!(false), "{}", r);
    assert_eq!(std::fs::read(racine.join("paies/2025-03.json")).unwrap(), avant);
    let r = route(&racine, "enregistrer-saisie", json!({"mois": "pas-un-mois", "saisie": {"engagements": []}}));
    assert_eq!(r["ok"], json!(false), "{}", r);
    assert!(!racine.join("paies/pas-un-mois.json").exists());
}

/// Deux cachets de 130 EUR le meme mois.
fn saisie_deux_cachets(racine: &Path) -> Value {
    let spectacles = lire(racine, "config/spectacles.json");
    let spec = spectacles.as_object().unwrap().iter().find(|(k, v)| !k.starts_with('_') && v["numero_objet"].as_str().is_some_and(|n| !n.is_empty())).unwrap().0.clone();
    json!({"mois": "2026-11", "spectacle": spec, "taux_cachet": 130.0, "date_paiement": "2026-11-30",
           "engagements": [{"salarie": "BRIOCHE", "spectacle": spec,
                            "lignes": [{"rubrique": "cachet", "quantite": 2, "taux": 130.0, "dates": ["2026-11-05", "2026-11-06"]}]}]})
}

fn ajuste(racine: &Path, saisie: &Value, cible: &str, montant: Value) -> Value {
    route(racine, "ajuster", json!({"saisie": saisie, "engagements": [0], "cible": cible, "montant": montant}))
}

fn nombre(v: &Value) -> f64 {
    // Les montants se rendent en texte (`"499.98"`), comme `sortie_json.brut`.
    v.as_f64().or_else(|| v.as_str().and_then(|t| t.parse().ok())).unwrap_or_else(|| panic!("un nombre etait attendu : {}", v))
}

#[test]
fn viser_un_cout_remonte_au_taux_du_cachet_au_centime_pres() {
    let racine = compagnie("ajuster-cout");
    let saisie = saisie_deux_cachets(&racine);
    let (mois, compteurs) = (racine.join("paies/2026-11.json"), racine.join("config/compteurs.json"));
    let avant = (std::fs::read(&mois).ok(), std::fs::read(&compteurs).ok());
    let r = ajuste(&racine, &saisie, "cout", json!(500));
    assert_eq!(r["ok"], json!(true), "{}", r);
    let a = &r["resultat"];
    let ligne = &a["engagements"][0]["lignes"][0];
    assert_eq!(nombre(&ligne["taux_avant"]), 130.0);
    let taux = nombre(&ligne["taux"]);
    assert!(taux > 130.0 && taux < 250.0, "ni 130 ni 250, mais le taux dont le cout vaut 500 : {}", taux);
    assert!((nombre(&a["atteint"]) - nombre(&a["vise"])).abs() <= 0.02, "{}", a);
    // Rien n'est ecrit : un calcul, pas une production.
    assert_eq!((std::fs::read(&mois).ok(), std::fs::read(&compteurs).ok()), avant, "ni la saisie ni les compteurs ne bougent");
}

#[test]
fn les_trois_cibles_ne_se_confondent_pas() {
    let racine = compagnie("ajuster-cibles");
    let saisie = saisie_deux_cachets(&racine);
    let mut taux = std::collections::HashMap::new();
    for cible in ["net", "brut", "cout"] {
        let r = ajuste(&racine, &saisie, cible, json!(400));
        assert_eq!(r["ok"], json!(true), "{}", r);
        let a = &r["resultat"];
        assert!((nombre(&a["atteint"]) - nombre(&a["vise"])).abs() <= 0.02, "{} : {}", cible, a);
        taux.insert(cible, nombre(&a["engagements"][0]["lignes"][0]["taux"]));
    }
    assert!(taux["net"] > taux["brut"] && taux["brut"] > taux["cout"], "{:?}", taux);
    // Le brut EST proportionnel : 400 EUR pour deux cachets, c'est 200.
    assert_eq!(taux["brut"], 200.0);
}

#[test]
fn un_defraiement_ne_se_hausse_pas_avec_l_enveloppe() {
    let racine = compagnie("ajuster-defraiement");
    let mut saisie = saisie_deux_cachets(&racine);
    saisie["engagements"][0]["lignes"].as_array_mut().unwrap().push(
        json!({"rubrique": "defraiement_forfait", "quantite": 2, "taux": 20.0, "dates": ["2026-11-05", "2026-11-06"]}),
    );
    let r = ajuste(&racine, &saisie, "cout", json!(600));
    assert_eq!(r["ok"], json!(true), "{}", r);
    let rangs: Vec<Value> = r["resultat"]["engagements"][0]["lignes"].as_array().unwrap().iter().map(|l| l["rang"].clone()).collect();
    assert_eq!(rangs, vec![json!(0)], "seul le cachet bouge");
}

#[test]
fn sans_salaire_ou_avec_une_demande_impossible_on_refuse_au_lieu_d_inventer() {
    let racine = compagnie("ajuster-refus");
    let mut sans_salaire = saisie_deux_cachets(&racine);
    sans_salaire["engagements"][0]["lignes"] = json!([{"rubrique": "defraiement_reel", "montant": 40.0, "dates": ["2026-11-05"]}]);
    assert_eq!(ajuste(&racine, &sans_salaire, "cout", json!(500))["ok"], json!(false));
    let saisie = saisie_deux_cachets(&racine);
    assert_eq!(ajuste(&racine, &saisie, "marge", json!(500))["ok"], json!(false), "cible inconnue");
    assert_eq!(ajuste(&racine, &saisie, "cout", json!(0))["ok"], json!(false), "montant nul");
    let r = route(&racine, "ajuster", json!({"saisie": saisie, "engagements": [7], "cible": "cout", "montant": 500}));
    assert_eq!(r["ok"], json!(false), "rang inconnu : {}", r);
    // Deux lignes de salaire sans taux : le montant ne dit pas comment les partager.
    let mut deux = saisie_deux_cachets(&racine);
    deux["engagements"][0]["lignes"][0]["taux"] = json!("");
    deux["engagements"][0]["lignes"].as_array_mut().unwrap().push(
        json!({"rubrique": "repetition", "quantite": 1, "taux": "", "heures": 2, "dates": ["2026-11-04"]}),
    );
    let r = ajuste(&racine, &deux, "cout", json!(500));
    assert_eq!(r["ok"], json!(false), "{}", r);
    assert!(r["erreur"].as_str().unwrap().contains("2 lignes de salaire sans taux"), "{}", r);
}

#[test]
fn une_seule_ligne_de_salaire_sans_taux_se_trouve() {
    let racine = compagnie("ajuster-sans-taux");
    let mut saisie = saisie_deux_cachets(&racine);
    saisie["engagements"][0]["lignes"][0]["taux"] = json!("");
    let r = ajuste(&racine, &saisie, "brut", json!(400));
    assert_eq!(r["ok"], json!(true), "{}", r);
    let ligne = &r["resultat"]["engagements"][0]["lignes"][0];
    assert_eq!(nombre(&ligne["taux"]), 200.0, "{}", ligne);
    assert_eq!(ligne["taux_avant"], json!(""), "le taux de depart est celui de l'ecran : vide");
}

#[test]
fn la_signature_posee_dit_pour_qui_elle_l_a_ete() {
    let racine = compagnie("identite-registre");
    let r = route(&racine, "poser-identite", json!({"quoi": "signature", "contenu": base64(&PNG)}));
    assert_eq!(r["ok"], json!(true), "{}", r);
    let registre = lire(&racine, "gabarits/identite/identite.json");
    assert!(registre["signature"]["de"].is_string(), "{}", registre);
    assert!(registre["signature"]["le"].as_str().is_some_and(|d| d.len() == 10), "{}", registre);
    let r = route(&racine, "identite", json!({}));
    let signature = &r["resultat"]["identite"]["signature"];
    assert_eq!(signature["pose"], json!(true), "{}", r);
    assert_eq!(signature["posee_pour"], signature["signataire"]["nom"], "{}", r);
    assert_eq!(signature["autre_personne"], json!(false), "{}", r);
    assert_eq!(r["resultat"]["identite"]["cachet"]["automatique"], json!(true), "{}", r);
}

#[test]
fn un_metier_de_la_convention_se_decoche_puis_se_recoche() {
    let racine = compagnie("metier-cycle");
    let codes = |r: &Value| -> Vec<String> { r["resultat"]["emplois"].as_array().unwrap().iter().map(|m| m["code"].as_str().unwrap().to_string()).collect() };
    let r = route(&racine, "metier-retenu", json!({"code": "COM010", "retenu": true}));
    assert_eq!(r["ok"], json!(true), "{}", r);
    assert!(codes(&r).contains(&"COM010".to_string()), "{}", r);
    let r = route(&racine, "metier-retenu", json!({"code": "COM010", "retenu": false}));
    assert!(!codes(&r).contains(&"COM010".to_string()), "decoche : {}", r);
    assert!(racine.join("config/metiers-ajoutes.json").exists());
    let r = route(&racine, "metier-retenu", json!({"code": "COM010", "retenu": true}));
    assert!(codes(&r).contains(&"COM010".to_string()), "recoche : {}", r);
}

#[test]
fn enregistrer_une_saisie_garde_la_validation_que_l_ecran_ne_renvoie_pas() {
    let racine = compagnie("saisie-fusion");
    let mois = "2026-11";
    let chemin = racine.join(format!("paies/{}.json", mois));
    let mut fichier = lire(&racine, &format!("paies/{}.json", mois));
    assert!(fichier["engagements"].as_array().is_some_and(|e| !e.is_empty()), "la compagnie fictive a des engagements en {}", mois);
    fichier["_origine"] = json!("ecrite a la main");
    fichier["engagements"][0]["valide"] = json!("2026-09-12T11:30:00");
    fichier["engagements"][0]["numero"] = json!("ZZ1.26.01");
    std::fs::write(&chemin, serde_json::to_vec_pretty(&fichier).unwrap()).unwrap();
    // L'ecran renvoie la saisie sans ce qu'il ne voit pas.
    let mut saisie = fichier.clone();
    saisie.as_object_mut().unwrap().remove("_origine");
    saisie["engagements"][0].as_object_mut().unwrap().remove("valide");
    saisie["engagements"][0].as_object_mut().unwrap().remove("numero");
    let r = route(&racine, "enregistrer-saisie", json!({"mois": mois, "saisie": saisie, "forcer": true}));
    assert_eq!(r["ok"], json!(true), "{}", r);
    let ecrit = lire(&racine, &format!("paies/{}.json", mois));
    assert_eq!(ecrit["_origine"], json!("ecrite a la main"));
    assert_eq!(ecrit["engagements"][0]["valide"], json!("2026-09-12T11:30:00"));
    assert_eq!(ecrit["engagements"][0]["numero"], json!("ZZ1.26.01"));
    assert_eq!(nommes(&racine.join("paies/avant"), "2026-11.json.").len(), 1, "la copie d'avant");
}

#[test]
fn un_mois_impossible_se_refuse_meme_force() {
    let racine = compagnie("saisie-mois");
    let saisie = lire(&racine, "paies/2026-11.json");
    let r = route(&racine, "enregistrer-saisie", json!({"mois": "2026-13", "saisie": saisie, "forcer": true}));
    assert_eq!(r["ok"], json!(false), "{}", r);
    assert!(!racine.join("paies/2026-13.json").exists());
}

#[test]
fn un_historique_qui_n_est_pas_une_liste_se_refuse_et_rien_n_est_ecrit() {
    // L'ignorer effacerait l'historique de la fiche sans le dire : on refuse.
    let racine = compagnie("fiche-historique");
    let avant = std::fs::read(racine.join("config/spectacles.json")).unwrap();
    for h in [json!("12345"), json!(null), json!({"champ": "titre"})] {
        let mut donnees = lire(&racine, "config/spectacles.json")["ubu-roi"].clone();
        donnees["historique"] = h;
        let r = route(&racine, "enregistrer-fiche", json!({"nom": "spectacle", "cle": "ubu-roi", "donnees": donnees}));
        assert_eq!(r["ok"], json!(false), "{}", r);
    }
    assert_eq!(std::fs::read(racine.join("config/spectacles.json")).unwrap(), avant);
}

/// Novembre de la petite, avec un JUMEAU : un second contrat de Brioche qui commence le meme jour que le premier
/// (un seul cachet, le 24). Le premier porte `BR.26.01`, le jumeau `BR.26.02` et une validation.
fn novembre_avec_un_jumeau(racine: &Path) -> Value {
    let mut fichier = lire(racine, "paies/2026-11.json");
    let mut jumeau = fichier["engagements"][0].clone();
    jumeau["fin"] = json!("2026-11-24");
    jumeau["date_paiement"] = json!("2026-11-24");
    jumeau["lignes"][0]["quantite"] = json!(1);
    jumeau["lignes"][0]["dates"] = json!(["2026-11-24"]);
    jumeau["numero"] = json!("BR.26.02");
    jumeau["valide"] = json!("2026-09-12T11:30:00");
    fichier["engagements"][0]["numero"] = json!("BR.26.01");
    fichier["engagements"].as_array_mut().unwrap().insert(1, jumeau);
    std::fs::write(racine.join("paies/2026-11.json"), serde_json::to_vec_pretty(&fichier).unwrap()).unwrap();
    fichier
}

#[test]
fn deux_contrats_du_meme_jour_gardent_chacun_leur_numero_et_leur_validation() {
    // Les jumeaux (meme salarie, meme debut) se confondaient sur (salarie, debut) : le jumeau valide remplacait
    // l'autre, et le dernier numero lu allait aux deux.
    let racine = compagnie("saisie-jumeaux");
    let fichier = novembre_avec_un_jumeau(&racine);
    // L'ecran renvoie la saisie sans numeros ni validation.
    let mut saisie = fichier.clone();
    for e in saisie["engagements"].as_array_mut().unwrap() {
        let e = e.as_object_mut().unwrap();
        e.remove("numero");
        e.remove("valide");
    }
    let r = route(&racine, "enregistrer-saisie", json!({"mois": "2026-11", "saisie": saisie, "forcer": true}));
    assert_eq!(r["ok"], json!(true), "{}", r);
    let ecrit = lire(&racine, "paies/2026-11.json");
    let e = ecrit["engagements"].as_array().unwrap();
    assert_eq!(e.len(), 4);
    assert_eq!(e[0]["numero"], json!("BR.26.01"), "{}", ecrit);
    assert_eq!(e[0]["lignes"][0]["quantite"], json!(4), "le premier contrat reste le sien : {}", ecrit);
    assert!(e[0].get("valide").is_none(), "le premier n'a jamais ete valide : {}", ecrit);
    assert_eq!(e[1]["numero"], json!("BR.26.02"), "{}", ecrit);
    assert_eq!(e[1]["lignes"][0]["quantite"], json!(1), "{}", ecrit);
    assert_eq!(e[1]["valide"], json!("2026-09-12T11:30:00"), "{}", ecrit);
}
