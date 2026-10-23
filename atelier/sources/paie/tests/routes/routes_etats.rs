//! `paie route` : les petites lectures de l'ouverture (`exercices`, `coffre-etat`, `filet-etat`, `importation-etat`,
//! `maj-etat`, `signalements`, `pieces-parties`) et la decision `maj-malgre-calcul`. Sur `fictive-3090-petite` (config, aout
//! et septembre 2026, journal vide) ; le coffre ne se lit que par des presences, sans cle de poste.

use crate::outils::{compagnie, dossier_de_test, ecrire, lire, reussi, route_avec};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn compagnie(nom: &str) -> PathBuf {
    compagnie::saisies("fictive-3090-petite", &["2026-08", "2026-09"], &dossier_de_test("routes_etats", nom).join("compagnie"))
}

fn route(c: &Path, nom: &str, charge: Value, env: &[(&str, &str)]) -> Value {
    route_avec(c, nom, charge, "2026-10-03", env)
}

fn lu(c: &Path, nom: &str) -> Value {
    reussi(&route(c, nom, json!({}), &[])).clone()
}

fn refuse(c: &Path, nom: &str) -> bool {
    route(c, nom, json!({}), &[])["ok"] == json!(false)
}

fn ecrire_texte(c: &Path, rel: &str, t: &str) {
    let p = c.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, t).unwrap();
}

// ------------------------------------------------------------------ le filet

#[test]
fn le_filet_se_lit_sans_reseau_et_ne_rend_pas_la_cle() {
    let c = &compagnie("filet");
    assert_eq!(
        lu(c, "filet-etat"),
        json!({"actif": false, "adresse": "", "confirme": false, "serveur": "https://filet.puppetpaie.fr", "inscrit_le": "", "erreur": "", "reveils": 0})
    );
    ecrire(c, 
        "config/filet-dpae.json",
        &json!({"serveur": "https://racine.test", "compte": {"adresse": "a@exemple.test", "cle": "SECRETE", "confirme": 1, "inscrit_le": "2026-10-01T10:00"}, "reveils": {"a|b": {}, "c|d": {}}}),
    );
    let e = lu(c, "filet-etat");
    assert_eq!(e["actif"], json!(true));
    assert_eq!(e["serveur"], json!("https://racine.test"), "le serveur de la racine quand le compte n'en a pas");
    assert_eq!(e["confirme"], json!(true));
    assert_eq!(e["reveils"], json!(2));
    assert!(!e.to_string().contains("SECRETE"), "la cle du compte ne sort pas");
}

/// Le serveur du filet (ses rappels) quand la compagnie ne dit rien : celui du moteur, `filet.puppetpaie.fr` (lot S1 ;
/// `editeur::SERVEUR_PAR_DEFAUT`). Ce qu'elle pose le remplace, et celui de son compte passe avant : c'est le serveur
/// auquel `filet-activer` s'inscrit (`filet::serveur`). Rien ne part pour le dire.
#[test]
fn le_serveur_du_filet_est_celui_du_moteur_tant_que_la_compagnie_n_en_pose_pas() {
    let c = &compagnie("filet-serveur");
    for rien in [json!({}), json!({"serveur": ""}), json!({"serveur": null, "compte": null})] {
        ecrire(c, "config/filet-dpae.json", &rien);
        assert_eq!(lu(c, "filet-etat")["serveur"], json!("https://filet.puppetpaie.fr"), "{}", rien);
    }
    ecrire(c, "config/filet-dpae.json", &json!({"serveur": "https://pose.test"}));
    assert_eq!(lu(c, "filet-etat")["serveur"], json!("https://pose.test"), "la valeur posee remplace celle du moteur");
    ecrire(c, "config/filet-dpae.json", &json!({"serveur": "https://pose.test", "compte": {"adresse": "a@exemple.test", "serveur": "https://compte.test"}}));
    assert_eq!(lu(c, "filet-etat")["serveur"], json!("https://compte.test"), "le serveur du compte passe avant");
}

#[test]
fn un_fichier_du_filet_inattendu_refuse_et_un_fichier_abime_ne_dit_aucun_compte() {
    let c = &compagnie("reveils");
    for f in [json!({"reveils": 5}), json!({"compte": "x"}), json!([1])] {
        ecrire(c, "config/filet-dpae.json", &f);
        assert!(refuse(c, "filet-etat"), "{}", f);
    }
    ecrire_texte(c, "config/filet-dpae.json", "{abime");
    assert_eq!(lu(c, "filet-etat")["actif"], json!(false));
}

// ------------------------------------------------------------------ les signalements

#[test]
fn les_signalements_viennent_du_plus_recent_et_comptent_les_reponses_non_lues() {
    let c = &compagnie("signalements");
    assert_eq!(lu(c, "signalements"), json!({"signalements": [], "non_lues": 0}));
    ecrire(c, 
        "journal/signalements.json",
        &json!([{"numero": "1"}, {"numero": "2", "reponse": "merci", "lu": false}, {"numero": "3", "reponse": "ok", "lu": true}, {"numero": "4", "reponse": ""}]),
    );
    let r = lu(c, "signalements");
    let numeros: Vec<&str> = r["signalements"].as_array().unwrap().iter().map(|s| s["numero"].as_str().unwrap()).collect();
    assert_eq!(numeros, ["4", "3", "2", "1"]);
    assert_eq!(r["non_lues"], json!(1));
    ecrire(c, "journal/signalements.json", &json!([1]));
    assert!(refuse(c, "signalements"), "la liste se lit, le compte des reponses refuse un element sans `get`");
    ecrire_texte(c, "journal/signalements.json", "{pas du json");
    assert_eq!(lu(c, "signalements"), json!({"signalements": [], "non_lues": 0}));
    ecrire(c, "journal/signalements.json", &json!({"a": 1}));
    assert_eq!(lu(c, "signalements")["signalements"], json!([]), "un objet n'est pas une liste");
}

#[test]
fn le_journal_des_incidents_deplace_emporte_les_signalements() {
    let c = &compagnie("signalements-ailleurs");
    ecrire(c, "ailleurs/signalements.json", &json!([{"numero": "9", "reponse": "x"}]));
    let ailleurs = c.join("ailleurs");
    let r = route(c, "signalements", json!({}), &[("PUPPETPAIE_INCIDENTS", ailleurs.to_str().unwrap())]);
    assert_eq!(r["resultat"]["non_lues"], json!(1));
}

// ------------------------------------------------------------------ l'importation

#[test]
fn l_importation_compte_les_contrats_les_salaries_et_les_objets_distincts() {
    let c = &compagnie("importation");
    assert_eq!(lu(c, "importation-etat")["contrats"], json!(0));
    let e = |salarie: &str, objet: &str, debut: &str, fin: &str| json!({"salarie": salarie, "numero_objet": objet, "debut": debut, "fin": fin});
    ecrire(
        c,
        "historique/engagements.json",
        &json!({
            "importe_le": "2026-09-01", "source": "/export",
            "engagements": [
                e("A", "O1", "2025-03-01", "2026-01-01"),
                e("A", "", "2024-12-31", "2025-02-01"),
                e("A", "O1", "2025-06-01", ""),
                e("B", "O2", "2025-07-01", "2026-02-01"),
            ]
        }),
    );
    let r = lu(c, "importation-etat");
    assert_eq!((r["contrats"].clone(), r["salaries"].clone(), r["objets"].clone()), (json!(4), json!(2), json!(2)));
    assert_eq!((r["du"].as_str(), r["au"].as_str()), (Some("2024-12-31"), Some("2026-02-01")));
    for engagements in [json!([{"salarie": ["x"], "numero_objet": "O", "debut": "d", "fin": "f"}]), json!([5])] {
        ecrire(c, "historique/engagements.json", &json!({"engagements": engagements}));
        assert!(refuse(c, "importation-etat"), "{}", engagements);
    }
}

#[test]
fn l_importation_dit_les_reprises_par_source_et_ce_qui_attend_l_editeur() {
    let c = &compagnie("importation-reprises");
    let r = lu(c, "importation-etat");
    assert_eq!(r["reprises"], json!([]));
    assert_eq!((r["editeur"].clone(), r["attend_editeur"].clone()), (json!([]), json!(0)));
    assert!(r["dossier"].as_str().unwrap().ends_with("import"));
    ecrire(c, 
        "historique/engagements.json",
        &json!({"importe_le": "2026-09-01", "source": "/export", "engagements": [
            {"salarie": "A", "debut": "2024-03-01", "fin": "2024-03-02", "numero_objet": "O1"},
            {"salarie": "A", "debut": "2025-05-04", "fin": "2025-05-06", "numero_objet": "O1", "atteste_par": "bulletin", "importe_le": "2026-10-10"},
        ]}),
    );
    ecrire(c, 
        "historique/paies.json",
        &json!({"importe_le": "2026-09-01", "paies": [
            {"mois": "2024-03", "salarie": "A", "contrat": "1", "declare_par": "DSN"},
            {"mois": "2024-03", "salarie": "B", "contrat": "3", "declare_par": "DSN"},
            {"mois": "2024-11", "salarie": "A", "contrat": "2", "declare_par": "DSN", "bulletin": {"fichier": "b.pdf", "page": 1}, "importe_le": "2026-10-10"},
            {"mois": "2025-05", "salarie": "A", "contrat": "bulletin 5/2025", "declare_par": "bulletin", "importe_le": "2026-10-10"},
            5,
        ]}),
    );
    ecrire(c, "historique/bulletins/x.pdf", &json!("pas un vrai pdf"));
    ecrire(c, 
        "journal/incidents.json",
        &json!([
            {"quoi": "logiciel", "ou": "importation", "genre": "fichiers non repris", "resume": "2 fichiers", "quand": "2026-10-10T09:00:00", "envoi": {"etat": "depose"}},
            {"quoi": "refus", "ou": "importation-apercu", "genre": "ErreurOperation", "resume": "dossier absent", "envoi": {"etat": "depose"}},
            {"quoi": "logiciel", "ou": "mois", "genre": "panne", "resume": "autre chose", "envoi": {"etat": "en-attente"}},
        ]),
    );
    let r = lu(c, "importation-etat");
    assert_eq!(r["contrats"], json!(2), "les cles d'avant gardent leur sens");
    assert_eq!(
        r["reprises"],
        json!([
            {"source": "DSN", "quoi": "versements déclarés", "nombre": 3, "du": "2024-03", "au": "2024-11", "le": "2026-10-10", "declarations": 2},
            {"source": "AEM", "quoi": "contrats attestés", "nombre": 1, "du": "2024-03-01", "au": "2024-03-02", "le": "2026-09-01"},
            {"source": "bulletins", "quoi": "bulletins de paie", "nombre": 2, "du": "2024-11", "au": "2025-05", "le": "2026-10-10", "contrats": 1, "copies": 1},
        ])
    );
    let editeur = r["editeur"].as_array().unwrap();
    assert_eq!(editeur.len(), 1, "un refus et un incident d'autre chose n'attendent pas l'editeur sur l'import : {}", r["editeur"]);
    assert_eq!((editeur[0]["canal"].clone(), editeur[0]["parti"].clone()), (json!("incident"), json!(true)));
    assert_eq!(r["attend_editeur"], json!(1));
}


// ------------------------------------------------------------------ la mise a jour

/// La mise a jour est celle de l'INSTALLATION (lot U1) : la compagnie et, a cote, une installation (son `logiciel/`, son
/// `maj/`), que la route trouve par `PUPPETPAIE_CODE`.
fn installation(c: &Path) -> PathBuf {
    crate::outils::installation(&c.parent().unwrap().join("installation"), "2026.10.09")
}

fn lu_dans(c: &Path, inst: &Path, nom: &str) -> Value {
    reussi(&route(c, nom, json!({}), &[("PUPPETPAIE_CODE", &crate::outils::code_de(inst))])).clone()
}

fn prete(inst: &Path, extra: Value) {
    std::fs::create_dir_all(inst.join("maj/2099.01.01/logiciel/donnees")).unwrap();
    let mut p = json!({"version": "2099.01.01", "calcul": "un calcul", "exigee": ""});
    for (k, v) in extra.as_object().unwrap() {
        p[k] = v.clone();
    }
    ecrire(inst, "maj/prete.json", &p);
}

#[test]
fn une_version_qui_change_un_calcul_attend_les_mois_saisis_sans_dsn() {
    let c = &compagnie("maj");
    let inst = &installation(c);
    let r = lu_dans(c, inst, "maj-etat");
    assert_eq!((r["prete"].clone(), r["attend"].clone(), r["signee"].clone()), (Value::Null, json!([]), json!(true)));
    assert_eq!(r["courante"], json!("2026.10.09"), "la version de l'installation");
    for mois in ["2026-08", "2026-09"] {
        ecrire(c, &format!("paies/{}.json", mois), &json!({"mois": mois, "engagements": [{"salarie": "INVENTE"}]}));
    }
    prete(inst, json!({}));
    let r = lu_dans(c, inst, "maj-etat");
    assert_eq!(r["prete"], json!("2099.01.01"));
    assert_eq!(r["calcul"], json!("un calcul"));
    assert_eq!(r["attend"], json!(["2026-08", "2026-09"]), "les deux mois saisis, sans DSN reelle acceptee");
    ecrire(c, "paies/2026-09.calcul.json", &json!({"engagements": [1]}));
    assert_eq!(lu_dans(c, inst, "maj-etat")["attend"], json!(["2026-08", "2026-09"]), "`AAAA-MM.calcul.json` n'est pas un mois");
    // Une compagnie fictive (ou une copie) ne retient rien : elle ne depose jamais.
    ecrire(c, "COPIE-DE-DEVELOPPEMENT.json", &json!({"copie_de": "compagnie fictive essai"}));
    assert_eq!(lu_dans(c, inst, "maj-etat")["attend"], json!([]), "une fictive ne retient pas la version");
    std::fs::remove_file(c.join("COPIE-DE-DEVELOPPEMENT.json")).unwrap();
    prete(inst, json!({"malgre_calcul": true}));
    assert_eq!(lu_dans(c, inst, "maj-etat")["attend"], json!([]), "passer outre");
    prete(inst, json!({"calcul": ""}));
    assert_eq!(lu_dans(c, inst, "maj-etat")["attend"], json!([]), "sans note de calcul, rien ne retient");
    std::fs::remove_dir_all(inst.join("maj/2099.01.01")).unwrap();
    assert_eq!(lu_dans(c, inst, "maj-etat")["prete"], Value::Null, "le dossier a disparu : rien de pret");
    assert!(!c.join("maj").exists(), "rien ne se range dans la compagnie");
}

/// Le depot des mises a jour quand le reglage de l'installation ne le dit pas : celui du moteur, sous `/maj/` du serveur
/// du filet (lot S1 ; `maj::DEPOT_PAR_DEFAUT`). Un depot pose le remplace ; le reglage d'avant le lot U1
/// (`prive/config/maj.json`) se lit en repli, et le reglage de l'installation passe avant lui.
#[test]
fn le_depot_des_mises_a_jour_est_celui_du_moteur_tant_qu_aucun_n_est_pose() {
    let c = &compagnie("maj-depot");
    let inst = &installation(c);
    for rien in [None, Some(json!({"editeur": false})), Some(json!({"depot": "", "editeur": false}))] {
        match &rien {
            None => {
                let _ = std::fs::remove_file(inst.join("maj/reglage.json"));
            }
            Some(v) => ecrire(inst, "maj/reglage.json", v),
        }
        assert_eq!(lu_dans(c, inst, "maj-etat")["depot"], json!("https://github.com/LeandreSimioni/PuppetPaie/releases/latest/download"), "{:?}", rien);
    }
    ecrire(c, "config/maj.json", &json!({"depot": "https://de-la-compagnie.test"}));
    assert_eq!(lu_dans(c, inst, "maj-etat")["depot"], json!("https://github.com/LeandreSimioni/PuppetPaie/releases/latest/download"), "une compagnie ne regle plus rien");
    std::fs::remove_file(inst.join("maj/reglage.json")).unwrap();
    ecrire(inst, "prive/config/maj.json", &json!({"depot": "D:/depot-d-avant", "editeur": true}));
    let r = lu_dans(c, inst, "maj-etat");
    assert_eq!((r["depot"].clone(), r["editeur"].clone()), (json!("D:/depot-d-avant"), json!(true)), "le reglage d'avant, en repli");
    ecrire(inst, "maj/reglage.json", &json!({"depot": "https://ailleurs.test/versions"}));
    let r = lu_dans(c, inst, "maj-etat");
    assert_eq!((r["depot"].clone(), r["editeur"].clone()), (json!("https://ailleurs.test/versions"), json!(false)));
}

#[test]
fn la_configuration_de_la_mise_a_jour_illisible_refuse() {
    let c = &compagnie("maj-config");
    let inst = &installation(c);
    let code = crate::outils::code_de(inst);
    let env = [("PUPPETPAIE_CODE", code.as_str())];
    ecrire(inst, "maj/reglage.json", &json!({"depot": "/depot", "editeur": true}));
    let r = lu_dans(c, inst, "maj-etat");
    assert_eq!((r["depot"].clone(), r["editeur"].clone()), (json!("/depot"), json!(true)));
    ecrire_texte(inst, "maj/reglage.json", "{abime");
    assert_eq!(route(c, "maj-etat", json!({}), &env)["ok"], json!(false));
    ecrire(inst, "maj/reglage.json", &json!([1]));
    assert_eq!(route(c, "maj-etat", json!({}), &env)["ok"], json!(false));
}

// ------------------------------------------------------------------ les exercices

#[test]
fn les_exercices_disent_les_mois_la_cloture_et_l_export() {
    let c = &compagnie("exercices");
    ecrire(c, "config/exercices.json", &json!({"2025": {"le": "2026-01-10T09:00:00"}, "_rouverts": []}));
    ecrire_texte(c, "sorties/exercices/Compta_2026.csv", "a;b\n");
    let r = lu(c, "exercices");
    assert!(r["dossier"].as_str().unwrap().ends_with("sorties/exercices"));
    let annees: Vec<&str> = r["exercices"].as_array().unwrap().iter().map(|x| x["annee"].as_str().unwrap()).collect();
    assert_eq!(annees, ["2026", "2025"], "du plus recent au plus ancien, l'annee close sans mois comprise");
    let y26 = &r["exercices"][0];
    assert_eq!(y26["mois"], json!(["2026-08", "2026-09"]));
    assert_eq!(y26["clos"], Value::Null);
    assert_eq!(y26["export"]["fichier"], json!("Compta_2026.csv"));
    assert_eq!(r["exercices"][1]["clos"]["le"], json!("2026-01-10T09:00:00"));
    ecrire(c, "config/exercices.json", &json!([1]));
    assert!(refuse(c, "exercices"));
}

// ------------------------------------------------------------------ les pieces parties

#[test]
fn les_pieces_parties_se_lisent_au_journal_et_se_trient_par_date() {
    let c = &compagnie("pieces");
    let (sal, deb, mois) = ("TESTEUR", "2026-09-02", "2026-09");
    ecrire_texte(c, "sorties/2026-09/DPAE_T.txt", "x");
    ecrire_texte(c, "sorties/2026-09/envoi/Contrat_T.pdf", "x");
    let ligne = |quoi: &str, quand: &str, detail: Value| json!({"quoi": quoi, "mode": "reel", "accepte": true, "mois": mois, "quand": quand, "fichier": "DPAE_T.txt", "detail": detail});
    ecrire(c, 
        "journal/depots.json",
        &json!([
            ligne("contrat-recu", "2026-09-09T10:00:00", json!({"salarie": sal, "debut": deb, "fichier_signe": "Signe.pdf", "preuve": "P.eml", "acte": "avenant", "avenant": 3})),
            ligne("contrat-signature", "2026-09-03T10:00:00", json!({"contrats": [
                {"salarie": sal, "debut": deb, "mois": mois, "fichier": "Contrat_T.pdf", "acte": "remplace", "remplace": 7},
                {"salarie": sal, "debut": deb, "fichier": "../evasion.pdf"},
                {"salarie": "AUTRE", "debut": deb, "fichier": "Autre.pdf"}]})),
            ligne("dpae", "2026-09-01T10:00:00", json!({"embauches": [{"salarie": sal, "debut": deb}]})),
            json!({"quoi": "dpae", "mode": "test", "accepte": true, "mois": mois, "fichier": "DPAE_T.txt", "detail": {"embauches": [{"salarie": sal, "debut": deb}]}}),
        ]),
    );
    let r = route(c, "pieces-parties", json!({"salarie": sal, "debut": deb}), &[]);
    let r = &r["resultat"];
    assert_eq!(r["dpae"].as_array().unwrap().len(), 1, "un depot de test n'est pas une DPAE partie");
    assert_eq!(r["dpae"][0], json!({"quoi": "DPAE déposée", "quand": "2026-09-01T10:00:00", "mois": mois, "fichier": "DPAE_T.txt", "existe": true}));
    let quoi: Vec<&str> = r["contrat"].as_array().unwrap().iter().map(|p| p["quoi"].as_str().unwrap()).collect();
    assert_eq!(
        quoi,
        [
            "Contrat qui annule et remplace le n° 7, envoyé à la signature",
            "Contrat envoyé à la signature",
            "Avenant n° 3 signé, contresigné",
            "Réponse « Je signe » du salarié (preuve)"
        ]
    );
    let existe: Vec<bool> = r["contrat"].as_array().unwrap().iter().map(|p| p["existe"].as_bool().unwrap()).collect();
    assert_eq!(existe, [true, false, false, false], "le contrat est dans `envoi/`, un nom avec `..` ne se resout pas");
    assert_eq!(route(c, "pieces-parties", json!({"salarie": sal}), &[])["ok"], json!(false), "sans debut");
}

// ------------------------------------------------------------------ le coffre

#[test]
fn le_coffre_ne_dit_que_des_presences() {
    let c = &compagnie("coffre");
    let r = route(c, "coffre-etat", json!({}), &[("URSSAF_MOTDEPASSE", "mot-de-passe-factice-123")]);
    assert_eq!(r["ok"], json!(true), "{}", r);
    let e = &r["resultat"];
    assert!(!r.to_string().contains("mot-de-passe-factice-123"), "la valeur d'une variable ne sort jamais");
    assert_eq!(e["mot_de_passe_pose"], json!(false));
    assert_eq!(e["cle_poste"], json!(false));
    let services: Vec<(&str, bool, bool)> =
        e["services"].as_array().unwrap().iter().map(|s| (s["service"].as_str().unwrap(), s["pose"].as_bool().unwrap(), s["environnement"].as_bool().unwrap())).collect();
    assert_eq!(services, [("net-entreprises", false, false), ("urssaf", false, true), ("courriel", false, false)]);
    let net = &e["services"][0]["identifiant"];
    assert_eq!(net["ici"], json!(false));
    assert_eq!(net["ou"], json!("fiche Entreprise › Qui les organismes contactent"));
    assert_eq!(e["services"][1]["identifiant"]["ici"], json!(true));
    assert_eq!(e["services"][2]["identifiant"], json!({"texte": "", "manque": [], "ici": false}));
}

#[test]
fn l_identifiant_du_compte_dit_ce_qui_manque() {
    let c = &compagnie("coffre-compte");
    let mut ent: Value = serde_json::from_str(&std::fs::read_to_string(c.join("config/entreprise.json")).unwrap()).unwrap();
    ent["declarant_dsn"] = json!({"nom_prenom": "DUPONT Jeanne Marie"});
    ent["siret"] = json!("12345678900011");
    ent["api_dpae"] = json!({"nom": "Jean"});
    ecrire(c, "config/entreprise.json", &ent);
    let e = lu(c, "coffre-etat");
    assert_eq!(e["services"][0]["identifiant"]["texte"], json!("compte de DUPONT Jeanne Marie, SIRET 12345678900011"));
    assert_eq!(e["services"][0]["identifiant"]["manque"], json!([]));
    let u = &e["services"][1]["identifiant"];
    assert_eq!((u["texte"].clone(), u["manque"].clone(), u["nom"].clone(), u["prenom"].clone()), (json!(""), json!(["prenom"]), json!("Jean"), json!("")));
    ent["api_dpae"] = json!("x");
    ecrire(c, "config/entreprise.json", &ent);
    assert!(refuse(c, "coffre-etat"), "api_dpae doit etre un objet");
}

#[test]
fn passer_outre_un_calcul_s_ecrit_dans_la_version_prete() {
    let racine = compagnie("maj-malgre-calcul");
    let inst = installation(&racine);
    let code = crate::outils::code_de(&inst);
    let env = [("PUPPETPAIE_CODE", code.as_str())];
    let r = route(&racine, "maj-malgre-calcul", json!({}), &env);
    assert_eq!(r["ok"], json!(false), "aucune version prete : refus ; {}", r);
    std::fs::create_dir_all(inst.join("maj").join("9.9.9").join("logiciel").join("donnees")).unwrap();
    std::fs::write(inst.join("maj").join("9.9.9").join("logiciel").join("donnees").join("x.json"), "{}\n").unwrap();
    std::fs::write(inst.join("maj").join("prete.json"), r#"{"version": "9.9.9", "note_calcul": "é"}"#).unwrap();
    reussi(&route(&racine, "maj-malgre-calcul", json!({}), &env));
    assert_eq!(lire(&inst, "maj/prete.json"), json!({"version": "9.9.9", "note_calcul": "é", "malgre_calcul": true}));
}
