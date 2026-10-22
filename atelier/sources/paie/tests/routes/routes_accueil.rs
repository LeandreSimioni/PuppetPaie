//! `paie route etat` (et `configuration`) : ce que la page lit en s'ouvrant -- la configuration verifiee, l'historique
//! des mois, les listes ou l'on choisit, les referentiels. Sur `fictive-3090-petite` (config, aout et septembre 2026,
//! journal vide), que chaque test mute ; la boite ne s'essaie que par des variables factices, jamais par le coffre.

use crate::outils::{code_livre, compagnie, fictive, dossier_de_test, ecrire, lire, route_avec};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

fn compagnie(nom: &str) -> PathBuf {
    compagnie::saisies("fictive-3090-petite", &["2026-08", "2026-09"], &dossier_de_test("routes_accueil", nom).join("compagnie"))
}

/// Modifie un fichier de `config/` (un objet JSON).
fn changer(racine: &Path, fichier: &str, f: impl FnOnce(&mut Value)) {
    let rel = format!("config/{}", fichier);
    let mut v = lire(racine, &rel);
    f(&mut v);
    ecrire(racine, &rel, &v);
}

/// `etat` le jour `aujourdhui` (la date du contexte reste le 03/10/2026), avec les variables `env`.
fn route_le(racine: &Path, route: &str, aujourdhui: &str, env: &[(&str, &str)]) -> Value {
    let mut e = vec![("PUPPETPAIE_AUJOURDHUI", aujourdhui)];
    e.extend_from_slice(env);
    route_avec(racine, route, json!({}), "2026-10-03", &e)
}

fn etat_le(racine: &Path, aujourdhui: &str) -> Value {
    let r = route_le(racine, "etat", aujourdhui, &[]);
    assert_eq!(r["ok"], json!(true), "{}", r);
    r["resultat"].clone()
}

fn etat(racine: &Path) -> Value {
    etat_le(racine, "2026-10-03")
}

fn table_urssaf() -> Value {
    lire(&code_livre(), "urssaf-dpae-base.json")
}

fn natures(etat: &Value) -> Vec<String> {
    etat["natures"].as_array().unwrap().iter().map(|n| n["cle"].as_str().unwrap().to_string()).collect()
}

fn anomalies(fiche: &Value) -> Vec<(String, String, String)> {
    fiche["anomalies"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| (a["champ"].as_str().unwrap().into(), a["gravite"].as_str().unwrap().into(), a["message"].as_str().unwrap().into()))
        .collect()
}

#[test]
fn l_etat_rend_tout_ce_que_la_page_lit_en_s_ouvrant() {
    let c = &compagnie("cles");
    let e = etat(c);
    let mut cles: Vec<&str> = e.as_object().unwrap().keys().map(String::as_str).collect();
    cles.sort();
    assert_eq!(
        cles,
        [
            "configuration", "conventions", "emplois", "emplois_autres", "familles_emplois", "historique", "nature_defaut",
            "natures", "niveaux_sante", "organismes_urssaf", "pays", "pays_naissance", "rubriques", "salaries", "simulation", "spectacles", "transaction", "types_preavis",
            "urssaf_deduit"
        ]
    );
    let cf = &e["configuration"];
    let mut cles: Vec<&str> = cf.as_object().unwrap().keys().map(String::as_str).collect();
    cles.sort();
    assert_eq!(
        cles,
        ["boite_liee", "compteurs", "description", "entreprise", "etablissements", "fiches_attendues", "identite", "permanents", "salaries", "salaries_au_mois", "spectacles"]
    );
    // Les trois fiches, avec leur description champ par champ.
    assert_eq!(cf["salaries"].as_array().unwrap().len(), 3);
    assert_eq!(cf["spectacles"].as_array().unwrap().len(), 2);
    assert_eq!(cf["entreprise"]["nom"], "entreprise");
    assert_eq!(cf["entreprise"]["cle"], Value::Null);
    assert_eq!(cf["salaries"][0]["cle"], "BRIOCHE");
    let decrits = |fiche: &str| -> Vec<String> { cf["description"][fiche].as_array().unwrap().iter().map(|c| c["cle"].as_str().unwrap().to_string()).collect() };
    for (fiche, cles) in [
        ("entreprise", &["taxe_sur_les_salaires", "contribution_supplementaire_apprentissage", "plafond_temps_partiel", "modele_bulletin", "deduction_forfaitaire_option"][..]),
        ("salarie", &["pays_nationalite", "pays_nationalite_hors_liste", "modele_bulletin", "sante_dispense_depuis", "deduction_forfaitaire_accord"]),
        ("spectacle", &["titre", "numero_objet"]),
    ] {
        let d = decrits(fiche);
        assert!(cles.iter().all(|c| d.iter().any(|x| x == c)), "{} : {:?}", fiche, d);
    }
    // La forme declaree se deduit de la forme lisible, sans rien ecrire.
    assert_eq!(cf["salaries"][0]["declare"]["nom"], "BRIOCHE");
    assert_eq!(cf["permanents"], false);
    assert_eq!(cf["etablissements"], false);
    assert_eq!(cf["salaries_au_mois"], json!([]));
    // Sans marqueur de fictive ni boite posee : ni envoi joue, ni boite liee.
    assert_eq!(e["simulation"], false);
    assert_eq!(cf["boite_liee"], false);
    // Les listes ou l'on CHOISIT.
    assert_eq!(e["salaries"][0], json!({"cle": "BRIOCHE", "libelle": "BRIOCHE Fanchon"}));
    assert_eq!(e["salaries"].as_array().unwrap().len(), 3);
    assert_eq!(e["spectacles"].as_array().unwrap().len(), 2);
    assert!(e["spectacles"][0]["numero_objet"].is_string());
    // Une compagnie qui n'a pas dit employer des permanents n'a que le CDDU.
    assert_eq!(e["nature_defaut"], "cddu_spectacle");
    assert_eq!(natures(&e), ["cddu_spectacle"]);
    // L'Urssaf du territoire, quand la fiche n'en dit rien : celui du Puy-de-Dome.
    let table = table_urssaf();
    assert_eq!(e["urssaf_deduit"], table["par_departement"]["63"]);
    assert!(e["organismes_urssaf"].as_array().unwrap().iter().any(|o| o["code"] == e["urssaf_deduit"]));
    // Les referentiels livres.
    assert_eq!(e["familles_emplois"][0], "Artistes");
    assert!(e["transaction"].as_array().unwrap().len() >= 2);
    assert!(e["types_preavis"].as_array().unwrap().iter().all(|t| t["saisissable"] == true));
    assert!(e["conventions"].as_array().unwrap().iter().any(|v| v["idcc"] == "3090" && v["applicable"] == true));
    // Le catalogue qui paie : les cachets d'abord, la plus utilisee en tete.
    let rubriques = e["rubriques"].as_array().unwrap();
    assert!(rubriques.len() > 20);
    assert_eq!(rubriques[0]["active"], true);
    let par = |code: &str| rubriques.iter().find(|r| r["code"] == code).unwrap().clone();
    assert_eq!(par("cachet")["reservee_a"], "activite");
    assert_eq!(par("mois")["reservee_a"], "mensuelle");
    assert_eq!(par("heure")["reservee_a"], Value::Null);
    assert_eq!(par("cachet")["heures_forfait"], 12);
}

#[test]
fn la_route_configuration_est_la_part_configuration_de_l_etat() {
    let c = &compagnie("configuration");
    let e = etat(c);
    let r = route_le(c, "configuration", "2026-10-03", &[]);
    assert_eq!(r["ok"], json!(true), "{}", r);
    assert_eq!(r["resultat"], e["configuration"]);
}

#[test]
fn les_natures_payees_au_mois_descendent_quand_la_compagnie_dit_en_employer() {
    let c = &compagnie("natures");
    changer(c, "entreprise.json", |e| e["emploie_des_permanents"] = json!(true));
    let e = etat(c);
    // Le defaut en tete : le deroulant s'ouvre sur ce qu'on saisit toujours.
    assert_eq!(natures(&e), ["cddu_spectacle", "cdi", "cdd"]);
    let par = |cle: &str| e["natures"].as_array().unwrap().iter().find(|n| n["cle"] == cle).unwrap().clone();
    let (cdi, cdd, cddu) = (par("cdi"), par("cdd"), par("cddu_spectacle"));
    // UN PERMANENT N'A PAS DE SPECTACLE : le numero d'objet lui est INTERDIT (CCH-11).
    assert_eq!((cdi["spectacle"].clone(), cddu["spectacle"].clone()), (json!(false), json!(true)));
    assert_eq!(cdi["mensuelle"], true);
    assert_eq!((cdi["signalement_fctu"].clone(), cddu["signalement_fctu"].clone()), (json!(true), json!(false)));
    // La page lit motif par motif si la fin se signale (lot C1, fiche 2062) : un CDDU a son terme (031) non, rompu avant
    // son terme (037) oui ; un CDD ordinaire toujours.
    assert_eq!((cddu["signalement_par_motif"]["031"].clone(), cddu["signalement_par_motif"]["037"].clone()), (json!(false), json!(true)));
    assert_eq!(cdd["signalement_par_motif"]["031"], json!(true));
    // Un CDI ne finit pas tout seul : aucun motif par defaut.
    assert_eq!(cdi["motif_rupture_defaut"], Value::Null);
    assert_eq!(cddu["motif_rupture_defaut"], "031");
    // Les faits du motif : le moteur dit a la page quoi demander.
    let faits = &cdi["faits_de_rupture"];
    assert_eq!(faits["043"], json!(["en_droit_a_la_retraite", "signature_convention"]));
    assert!(!faits["087"].as_array().unwrap().contains(&json!("preavis_non_effectue")));
    assert!(faits["020"].as_array().unwrap().contains(&json!("preavis_non_effectue")));
    assert_eq!(cdd["faits_de_rupture"]["031"], json!(["precarite_due"]));
    assert_eq!(cdd["faits_de_rupture"]["037"], json!(["notification_rupture"]));
    // Tout motif dont le signalement exige la date de notification (CCH-12 de S21.G00.62.003, `fctu::notification_exigee`)
    // la fait demander a la page : la force majeure (083) aussi, que la page taisait alors que le signalement la veut.
    for m in ["036", "083", "095"] {
        assert!(cddu["faits_de_rupture"][m].as_array().unwrap().contains(&json!("notification_rupture")), "{} : {}", m, cddu["faits_de_rupture"]);
    }
    // Les dates et la transaction selon le motif.
    assert_eq!(faits["020"].as_array().unwrap()[1..], [json!("notification_rupture"), json!("engagement_licenciement"), json!("transaction")]);
    // La case « preavis non effectue, paye en indemnite » n'est proposee que la ou le texte paie (lot P1) : ni la
    // demission (L1237-1), ni le depart a la retraite (L1237-10), ni l'inaptitude non professionnelle (L1226-4).
    assert_eq!(faits["059"], json!(["notification_rupture"]));
    for m in ["039", "091"] {
        assert!(!faits[m].as_array().unwrap().contains(&json!("preavis_non_effectue")), "{}", m);
    }
    for m in ["092", "038"] {
        assert!(faits[m].as_array().unwrap().contains(&json!("preavis_non_effectue")), "{}", m);
    }
    assert_eq!(faits["065"], json!(["preavis_non_effectue"]));
    // Le motif de recours : obligatoire pour un CDD ordinaire, sans objet ailleurs.
    assert_eq!(cdd["motif_recours_exige"], true);
    let codes: Vec<&str> = cdd["motifs_recours"].as_array().unwrap().iter().map(|m| m["code"].as_str().unwrap()).collect();
    assert!(codes.contains(&"01") && !codes.contains(&"05"), "{:?}", codes);
    assert_eq!(cdi["motifs_recours"], json!([]));
    assert_eq!(cdi["quotite_defaut"], 151.67);
    assert_eq!(cddu["quotite_defaut"], Value::Null);
    assert_eq!(e["configuration"]["permanents"], true);
}

#[test]
fn les_salaries_payes_au_mois_se_lisent_dans_les_saisies() {
    let c = &compagnie("au-mois");
    changer(c, "entreprise.json", |e| e["emploie_des_permanents"] = json!(true));
    // Un contrat a duree indeterminee pour LEMERCIER, dit par son nom approchant.
    let mut saisie = lire(c, "paies/2026-09.json");
    saisie["engagements"] = json!([{"salarie": "Lemercier", "nature": "cdi", "debut": "2026-09-01", "lignes": []}, {"salaries": ["Brioche", "", "Inconnu Total"], "nature": "cddu_spectacle"}, "pas un engagement"]);
    ecrire(c, "paies/2026-09.json", &saisie);
    // Un fichier abime et une saisie sans engagements ne bloquent rien.
    std::fs::write(c.join("paies/2026-07.json"), "pas du json").unwrap();
    ecrire(c, "paies/2026-06.json", &json!({"mois": "2026-06"}));
    let e = etat(c);
    assert_eq!(e["configuration"]["salaries_au_mois"], json!(["LEMERCIER"]));
}

#[test]
fn une_fiche_fautive_remonte_ses_anomalies() {
    let c = &compagnie("anomalies");
    changer(c, "salaries.json", |s| {
        let f = &mut s["BRIOCHE"];
        f["nir"] = json!("2881175999000"); // la cle ne suit pas
        f["nir_cle"] = json!("12");
        f["sexe"] = json!("1");
        f["email"] = json!("pas-un-courriel");
        f["nom"] = json!("Brioche-");
        f["date_naissance"] = json!("1990-02-30");
        f["code_emploi_ft"] = json!("ZZZ999");
    });
    changer(c, "entreprise.json", |e| e["siret"] = json!("12345692300011"));
    let e = etat(c);
    let a = anomalies(&e["configuration"]["salaries"][0]);
    let a: Vec<(&str, &str, &str)> = a.iter().map(|(x, y, z)| (x.as_str(), y.as_str(), z.as_str())).collect();
    // Chaque champ fautif est nomme, avec sa gravite. La forme declaree (`nom`) se deduit de la forme lisible : on pointe
    // celle-ci.
    let a_le = |champ: &str, gravite: &str| a.iter().any(|(c, g, _)| *c == champ && *g == gravite);
    for champ in ["nom_affichage", "email", "date_naissance", "sexe", "nir_cle"] {
        assert!(a_le(champ, "erreur"), "{} : {:?}", champ, a);
    }
    assert!(a.iter().any(|(c, g, m)| *c == "code_emploi_ft" && *g == "avertissement" && m.contains("ZZZ999")), "{:?}", a);
    // La fiche entreprise : SIRET != SIREN + NIC (le SIRET attendu est dit), et une cle de Luhn fausse.
    let a = anomalies(&e["configuration"]["entreprise"]);
    assert_eq!(a.iter().filter(|(c, _, _)| c == "siret").count(), 2, "{:?}", a);
    assert!(a.iter().any(|(c, _, m)| c == "siret" && m.contains("12345692300010")), "{:?}", a);
    // Les autres fiches ne disent rien.
    assert!(anomalies(&e["configuration"]["salaries"][1]).is_empty());
    assert!(anomalies(&e["configuration"]["spectacles"][0]).is_empty());
}

#[test]
fn une_fiche_archivee_crie_moins_fort_et_sort_des_listes_ou_l_on_choisit() {
    let c = &compagnie("archives");
    changer(c, "salaries.json", |s| {
        s["SERAPHIN"]["actif"] = json!(false);
        s["SERAPHIN"]["nir"] = json!("123");
    });
    changer(c, "spectacles.json", |s| {
        let k = s.as_object().unwrap().keys().find(|k| !k.starts_with('_')).unwrap().clone();
        s[k.as_str()]["actif"] = json!(false);
    });
    let e = etat(c);
    let cles: Vec<&str> = e["salaries"].as_array().unwrap().iter().map(|s| s["cle"].as_str().unwrap()).collect();
    assert_eq!(cles, ["BRIOCHE", "LEMERCIER"]);
    assert_eq!(e["spectacles"].as_array().unwrap().len(), 1);
    // L'archive reste dans la configuration, ou on peut la relire et la reactiver.
    assert_eq!(e["configuration"]["salaries"].as_array().unwrap().len(), 3);
    assert_eq!(e["configuration"]["spectacles"].as_array().unwrap().len(), 2);
    let a = anomalies(&e["configuration"]["salaries"][1]);
    assert!(!a.is_empty());
    assert!(a.iter().all(|(_, g, m)| g == "avertissement" && m.ends_with("(fiche inactive : a completer avant toute paie)")), "{:?}", a);
}

#[test]
fn l_historique_dit_ou_en_est_chaque_mois_du_plus_recent_au_plus_ancien() {
    let c = &compagnie("historique");
    let dsn = json!({"quoi": "dsn", "quand": "2026-09-14T10:00:00", "origine": "", "mois": "2026-08", "fichier": "DSN_Aout2026.dsn", "empreinte": "aaaa",
                     "mode": "reel", "qui": "interface", "accepte": true, "destinataire": "x", "flux": "F1",
                     "retours": [{"nature": "11", "verdict": "OK", "anomalies": []}], "reponse": "", "trace": null, "detail": {}});
    ecrire(c, "journal/depots.json", &json!([dsn]));
    let dossier = c.join("sorties/2026-08");
    std::fs::create_dir_all(&dossier).unwrap();
    for n in ["Bulletins_Aout2026.pdf", "DSN_Aout2026.dsn", "DPAE_x.txt", "Bulletins_b.pdf", "note.txt"] {
        std::fs::write(dossier.join(n), "x").unwrap();
    }
    std::fs::create_dir_all(c.join("sorties/exercices")).unwrap();
    ecrire(c, "config/exercices.json", &json!({"2026": {"le": "2026-12-01T10:00:00"}, "_x": 1}));
    let e = etat(c);
    let h = e["historique"].as_array().unwrap();
    let mois: Vec<&str> = h.iter().map(|m| m["mois"].as_str().unwrap()).collect();
    // Les mois saisis et les mois produits : `sorties/exercices` n'est pas un mois.
    assert_eq!(mois, ["2026-09", "2026-08"]);
    // Septembre : saisi, rien de parti, l'echeance (15/10) n'est pas passee.
    assert_eq!(h[0]["dsn"], "a_ecrire");
    assert_eq!(h[0]["echeance"], "2026-10-15");
    assert_eq!(h[0]["produit"], false);
    assert_eq!(h[0]["clos"], true);
    assert_eq!(h[0]["corrigeable"], false);
    // Aout : DSN reelle acceptee ; deposee et l'echeance derriere : elle se corrige.
    assert_eq!(h[1]["dsn"], "passee");
    assert_eq!(h[1]["depose_le"], "2026-09-14T10:00:00");
    assert_eq!(h[1]["corrigeable"], true);
    assert_eq!(h[1]["produit"], true);
    assert_eq!(h[1]["sortes"], json!(["bulletins", "DPAE", "DSN"]));
    assert_eq!(h[1]["fichiers"], json!(["Bulletins_Aout2026.pdf", "Bulletins_b.pdf", "DPAE_x.txt", "DSN_Aout2026.dsn", "note.txt"]));
    // Plus tard, septembre passe l'echeance sans DSN : en retard.
    let tard = etat_le(c, "2026-10-20");
    assert_eq!(tard["historique"][0]["dsn"], "en_retard");
}

#[test]
fn des_effectifs_illisibles_ne_bloquent_pas_l_historique_un_journal_abime_refuse() {
    let c = &compagnie("repli");
    std::fs::write(c.join("config/effectifs.json"), "{ pas du json").unwrap();
    let e = etat(c);
    // On retombe sur la liste nue : le mois et ses fichiers, sans etat de DSN ; la page Paie reste a un clic (`a_faire`).
    assert_eq!(e["historique"][0], json!({"mois": "2026-09", "produit": false, "dsn": null, "corrigeable": false, "a_faire": true, "sortes": [], "fichiers": []}));
    assert_eq!(e["historique"].as_array().unwrap().len(), 2);
    // Un journal abime, lui, refuse la page : c'est une donnee illisible.
    let c = &compagnie("repli-journal");
    std::fs::write(c.join("journal/depots.json"), "[x").unwrap();
    let r = route_le(c, "etat", "2026-10-03", &[]);
    assert_eq!(r["ok"], false, "{}", r);
}

#[test]
fn les_fiches_demandees_en_attente_se_lisent_au_journal() {
    let c = &compagnie("attente");
    let ligne = |quoi: &str, accepte: bool, detail: Value| json!({"quoi": quoi, "quand": "2026-09-30T10:00:00", "mode": "reel", "accepte": accepte, "detail": detail});
    ecrire(c, 
        "journal/depots.json",
        &json!([
            ligne("fiche-demande", true, json!({"code": "A1", "adresse": "ana@exemple.invalid", "prenom": "Ana", "message": "m1"})),
            ligne("fiche-demande", true, json!({"code": "A2", "adresse": "bob@exemple.invalid", "prenom": "Bob"})),
            ligne("fiche-recue", true, json!({"code": "A2"})),
            ligne("fiche-demande", false, json!({"code": "A3"})),
            ligne("fiche-demande", true, json!({"adresse": "sans-code"})),
        ]),
    );
    let e = etat(c);
    assert_eq!(
        e["configuration"]["fiches_attendues"],
        json!([{"code": "A1", "adresse": "ana@exemple.invalid", "prenom": "Ana", "quand": "2026-09-30T10:00:00"}])
    );
}

#[test]
fn la_fictive_joue_les_envois_et_une_vraie_compagnie_dit_si_sa_boite_est_liee() {
    let c = &compagnie("boite");
    // Une compagnie fictive : tout est joue, la boite se dit liee.
    assert_eq!(etat(c)["simulation"], false, "sans marqueur, ce n'est pas une compagnie fictive");
    std::fs::copy(fictive("fictive-3090-petite").join("COPIE-DE-DEVELOPPEMENT.json"), c.join("COPIE-DE-DEVELOPPEMENT.json")).unwrap();
    let e = etat(c);
    assert_eq!((e["simulation"].clone(), e["configuration"]["boite_liee"].clone()), (json!(true), json!(true)));
    // Une copie de developpement n'est pas une compagnie fictive.
    ecrire(c, "COPIE-DE-DEVELOPPEMENT.json", &json!({"copie_de": "prive", "quand": "2026-09-30T10:00:00"}));
    let e = etat(c);
    assert_eq!((e["simulation"].clone(), e["configuration"]["boite_liee"].clone()), (json!(false), json!(false)));
    // Un marqueur illisible ne fait pas une compagnie fictive.
    std::fs::write(c.join("COPIE-DE-DEVELOPPEMENT.json"), "pas du json").unwrap();
    assert_eq!(etat(c)["simulation"], false);
    // La boite est liee quand l'adresse ET le secret sont la : ici par
    // l'environnement, avec des valeurs factices que rien n'affiche.
    let r = route_le(c, "etat", "2026-10-03", &[("COURRIEL_COMPTE", "essai@exemple.invalid"), ("COURRIEL_MOTDEPASSE", "pas-un-secret")]);
    assert_eq!(r["resultat"]["configuration"]["boite_liee"], true);
    assert!(!r.to_string().contains("pas-un-secret"));
    // L'adresse seule ne suffit pas : sans secret, la demande ne peut pas partir.
    let r = route_le(c, "etat", "2026-10-03", &[("COURRIEL_COMPTE", "essai@exemple.invalid")]);
    assert_eq!(r["resultat"]["configuration"]["boite_liee"], false);
}

#[test]
fn l_identite_posee_se_lit_avec_sa_date_a_l_heure_de_paris() {
    let c = &compagnie("identite");
    let g = c.join("gabarits/identite");
    assert_eq!(etat(c)["configuration"]["identite"]["signature"]["pose"], false);
    std::fs::create_dir_all(&g).unwrap();
    std::fs::write(g.join("signature.png"), [0x89u8, b'P', b'N', b'G']).unwrap();
    std::fs::write(g.join("logo.svg"), "<svg/>").unwrap();
    std::fs::write(g.join("logo.png"), "x").unwrap();
    // 15/01/2026 12:00 UTC : 13:00 a Paris l'hiver. 14/07/2026 12:00 UTC : 14:00 l'ete.
    let date = |fichier: &str, t: u64| std::fs::File::options().write(true).open(g.join(fichier)).unwrap().set_modified(UNIX_EPOCH + Duration::from_secs(t)).unwrap();
    date("signature.png", 1_768_478_400);
    date("logo.svg", 1_784_030_400);
    date("logo.png", 1_784_030_400);
    std::fs::write(g.join("identite.json"), r#"{"signature": {"de": "Quelqu'un d'autre"}}"#).unwrap();
    let e = etat(c);
    let id = &e["configuration"]["identite"];
    assert_eq!(id["signature"]["pose"], true);
    assert_eq!(id["signature"]["quand"], "2026-01-15T13:00:00");
    assert_eq!(id["signature"]["octets"], 4);
    assert_eq!(id["signature"]["format"], "png");
    assert_eq!(id["signature"]["automatique"], false);
    assert!(id["signature"]["fichier"].as_str().unwrap().ends_with("gabarits/identite/signature.png"));
    // Le cachet est dessine tout seul quand rien n'est pose.
    assert_eq!(id["cachet"]["pose"], false);
    assert_eq!(id["cachet"]["automatique"], true);
    assert!(id["cachet"]["dossier"].as_str().unwrap().ends_with("gabarits/identite"));
    // Deux extensions : le png gagne sur le svg.
    assert_eq!(id["logo"]["format"], "png");
    assert_eq!(id["logo"]["quand"], "2026-07-14T14:00:00");
    // La signature posee pour une autre personne que celle qui signe aujourd'hui.
    assert_eq!(id["signature"]["signataire"], json!({"nom": "George Sand", "fonction": "Présidente"}));
    assert_eq!(id["signature"]["posee_pour"], "Quelqu'un d'autre");
    assert_eq!(id["signature"]["autre_personne"], true);
    assert_eq!(id["signature"]["documents"].as_array().unwrap().len(), 5);
}

#[test]
fn les_metiers_suivent_la_convention_et_les_choix_de_la_compagnie() {
    let c = &compagnie("metiers");
    let e = etat(c);
    let (siens, autres) = (e["emplois"].as_array().unwrap().len(), e["emplois_autres"].as_array().unwrap().len());
    assert!(siens > 10 && autres > 0, "{} {}", siens, autres);
    // Les artistes d'abord, puis les familles dans l'ordre du catalogue.
    assert_eq!(e["emplois"][0]["famille"], "Artistes");
    // Un metier d'une autre convention : on l'ajoute a son deroulant.
    let un = e["emplois_autres"][0].clone();
    assert!(un["de"].as_str().unwrap().contains('('), "{}", un);
    let code = un["code"].as_str().unwrap().to_string();
    ecrire(c, "config/metiers-ajoutes.json", &json!({"codes": [code], "retires": ["REG120"]}));
    let apres = etat(c);
    let codes = |v: &Value| -> Vec<String> { v.as_array().unwrap().iter().map(|m| m["code"].as_str().unwrap().to_string()).collect() };
    assert!(codes(&apres["emplois"]).contains(&code));
    assert!(!codes(&apres["emplois_autres"]).contains(&code));
    assert!(!codes(&apres["emplois"]).contains(&"REG120".to_string()));
    assert!(codes(&apres["emplois_autres"]).contains(&"REG120".to_string()));
    // Rien ne se perd : ce qui est propose et ce qui ne l'est pas font le catalogue.
    assert_eq!(codes(&apres["emplois"]).len() + codes(&apres["emplois_autres"]).len(), siens + autres);
    // Sans IDCC (une compagnie qui s'installe), rien ne se cache.
    changer(c, "entreprise.json", |e| {
        e.as_object_mut().unwrap().remove("idcc");
    });
    ecrire(c, "config/metiers-ajoutes.json", &json!({}));
    let sans = etat(c);
    assert_eq!(sans["emplois_autres"], json!([]));
    assert_eq!(sans["emplois"].as_array().unwrap().len(), siens + autres);
}

#[test]
fn le_catalogue_de_la_compagnie_passe_devant_celui_du_logiciel() {
    let c = &compagnie("rubriques");
    ecrire(c, 
        "config/rubriques-compagnie.json",
        &json!({"rubriques": {
            "prime_maison": {"libelle": "Prime de la maison", "unite": "forfait", "nature": "prime", "saisie": ["quantite", "taux"],
                             "soumis": true, "imposable": true, "conges_spectacles": true, "source": "essai"},
            "avantage_repas": {"libelle": "Avantage repas", "unite": "jour", "nature": "avantage_nature", "saisie": ["quantite", "taux", "genre"],
                               "soumis": true, "imposable": true, "conges_spectacles": false, "source": "essai", "dsn_autres_bruts": "02",
                               "aem": "x", "plafond": "jour", "annexe": "VIII"},
            "cachet": {"libelle": "Cachet ajuste"}
        }}),
    );
    ecrire(c, "config/rubriques-actives.json", &json!({"actives": ["prime_maison", "cachet"], "usages": {"cachet": 12, "prime_maison": 40}}));
    let e = etat(c);
    let r = e["rubriques"].as_array().unwrap();
    // Les actives d'abord, la plus utilisee en tete.
    assert_eq!((r[0]["code"].clone(), r[1]["code"].clone()), (json!("prime_maison"), json!("cachet")));
    assert_eq!(r[0]["origine"], "compagnie");
    assert_eq!(r[1]["origine"], "base");
    assert_eq!(r[1]["libelle"], "Cachet ajuste");
    assert_eq!(r[1]["usages"], 12);
    let repas = r.iter().find(|x| x["code"] == "avantage_repas").unwrap();
    // Les genres du bloc 54, le code de la rubrique par defaut.
    let genres = repas["genres"].as_array().unwrap();
    assert_eq!(genres.len(), 5);
    assert_eq!(genres.iter().filter(|g| g["defaut"] == true).count(), 1);
    assert_eq!(genres[0], json!({"code": "02", "libelle": "Avantage en nature : repas", "defaut": true}));
    // Ce que la rubrique promet sans que rien ne le tienne.
    assert_eq!(repas["non_appliques"], json!(["annexe", "plafond", "aem"]));
    assert_eq!(r[0]["non_appliques"], json!([]));
}

#[test]
fn les_etablissements_et_le_code_postal_commandent_l_ecran() {
    let c = &compagnie("etablissements");
    changer(c, "entreprise.json", |e| e["etablissements"] = json!([{"nic": "00010"}]));
    assert_eq!(etat(c)["configuration"]["etablissements"], false, "le principal seul n'est pas un second etablissement");
    changer(c, "entreprise.json", |e| e["etablissements"] = json!([{"nic": "00010"}, {"nic": "00028", "adresse": "2 RUE B"}]));
    assert_eq!(etat(c)["configuration"]["etablissements"], true);
    // L'organisme du territoire se lit sur le code postal : trois chiffres outre-mer.
    let table = table_urssaf();
    for (cp, dep) in [("97139", "971"), ("2A004", "20"), ("75008", "75")] {
        changer(c, "entreprise.json", |e| e["code_postal"] = json!(cp));
        let attendu = table["par_departement"][dep].as_str().unwrap_or("117");
        assert_eq!(etat(c)["urssaf_deduit"], attendu, "{}", cp);
    }
    // Un code postal inexploitable ne suggere rien : le defaut du gabarit.
    changer(c, "entreprise.json", |e| e["code_postal"] = json!("7"));
    assert_eq!(etat(c)["urssaf_deduit"], "117");
}

#[test]
fn un_salarie_sans_prenom_refuse_la_page() {
    let c = &compagnie("refus");
    changer(c, "salaries.json", |s| {
        s["BRIOCHE"].as_object_mut().unwrap().remove("prenom");
        s["BRIOCHE"].as_object_mut().unwrap().remove("prenom_affichage");
    });
    assert_eq!(route_le(c, "etat", "2026-10-03", &[])["ok"], json!(false));
}

#[test]
fn les_courriels_joues_de_la_fictive_se_lisent_et_une_vraie_compagnie_refuse() {
    let c = &compagnie("courriels-joues");
    let lus = |apres: &str| route_avec(c, "courriels-fictifs", json!({"apres": apres}), "2026-10-03", &[]);
    assert_eq!(lus("")["ok"], json!(false), "sans marqueur, ce n'est pas une compagnie fictive");
    std::fs::copy(fictive("fictive-3090-petite").join("COPIE-DE-DEVELOPPEMENT.json"), c.join("COPIE-DE-DEVELOPPEMENT.json")).unwrap();
    std::fs::create_dir_all(c.join("courriels-simules")).unwrap();
    let eml = "From: Direction <direction@exemple.invalid>\r\nTo: zoe@exemple.invalid\r\nSubject: Ta fiche\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nBonjour Zoe\r\n";
    std::fs::write(c.join("courriels-simules/2026-10-03_01.eml"), eml).unwrap();
    let r = lus("");
    let m = &r["resultat"]["courriels"][0];
    assert_eq!((m["sujet"].as_str(), m["a"].as_str()), (Some("Ta fiche"), Some("zoe@exemple.invalid")), "{}", r);
    assert!(m["texte"].as_str().unwrap().contains("Bonjour Zoe"), "{}", r);
    assert_eq!(r["resultat"]["dernier"], json!("2026-10-03_01.eml"));
    // Ce que la page a deja lu ne revient pas.
    assert_eq!(lus("2026-10-03_01.eml")["resultat"]["courriels"], json!([]));
}
