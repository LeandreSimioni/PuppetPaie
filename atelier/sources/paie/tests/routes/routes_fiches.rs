//! `paie route` : les routes de la fiche, qui n'ecrivent rien (`taux-atmp`, `verifier-fiche`, `changements-declares`) ;
//! une charge qui ne se lit pas refuse. `normaliser` se teste dans `src/routes_fiches.rs`. Sur la configuration de
//! `fictive-3090-petite`, que chaque test mute.

use crate::outils::{compagnie, fictive, dossier_de_test, ecrire, lire, reussi, route};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn compagnie(nom: &str) -> PathBuf {
    compagnie::saisies("fictive-3090-petite", &[], &dossier_de_test("routes_fiches", nom).join("compagnie"))
}

fn resultat(c: &Path, nom: &str, charge: Value) -> Value {
    reussi(&route(c, nom, charge, "2026-10-03")).clone()
}

fn refuse(c: &Path, nom: &str, charge: Value) -> bool {
    route(c, nom, charge, "2026-10-03")["ok"] == json!(false)
}

fn ecrire_texte(c: &Path, rel: &str, t: &str) {
    let p = c.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, t).unwrap();
}

// ------------------------------------------------------------------ taux-atmp

#[test]
fn le_taux_atmp_se_lit_dans_la_surcouche_de_l_annee() {
    let c = &compagnie("atmp");
    let lire = |an: Value| resultat(c, "taux-atmp", json!({"annee": an}));
    ecrire(c, "config/baremes/2031.json", &json!({"annee": 2031, "taux_atmp": {"valeur": 1.5, "code_risque": "923AD"}}));
    assert_eq!(lire(json!(2031)), json!({"annee": 2031, "taux": {"valeur": 1.5, "code_risque": "923AD"}}));
    assert_eq!(lire(json!(2031.9))["annee"], json!(2031));
    // pas de code de risque : une chaine vide
    ecrire(c, "config/baremes/2031.json", &json!({"taux_atmp": {"valeur": 0, "code_risque": null}}));
    assert_eq!(lire(json!(2031))["taux"], json!({"valeur": 0, "code_risque": ""}));
    // ni valeur, ni fichier, ni JSON lisible : pas de taux
    ecrire(c, "config/baremes/2031.json", &json!({"taux_atmp": {"valeur": null}}));
    assert_eq!(lire(json!(2031))["taux"], Value::Null);
    ecrire(c, "config/baremes/2031.json", &json!({"taux_atmp": "x"}));
    assert_eq!(lire(json!(2031))["taux"], Value::Null);
    ecrire_texte(c, "config/baremes/2031.json", "{abime");
    assert_eq!(lire(json!(2031))["taux"], Value::Null);
    assert_eq!(lire(json!(2099))["taux"], Value::Null);
}

#[test]
fn le_taux_atmp_refuse_une_annee_illisible_ou_un_fichier_qui_n_est_pas_un_objet() {
    let c = &compagnie("atmp-refus");
    for charge in [json!({}), json!({"annee": "abc"}), json!({"annee": null}), json!({"annee": [2026]})] {
        assert!(refuse(c, "taux-atmp", charge.clone()), "{}", charge);
    }
    ecrire(c, "config/baremes/2031.json", &json!([1]));
    assert!(refuse(c, "taux-atmp", json!({"annee": 2031})));
}

// ------------------------------------------------------------ verifier-fiche

fn fiche_salarie() -> Value {
    let t = lire(&fictive("fictive-3090-petite"), "config/salaries.json");
    t.as_object().unwrap().iter().find(|(k, _)| !k.starts_with('_')).unwrap().1.clone()
}

#[test]
fn verifier_fiche_derive_avant_de_verifier_et_propose_pour_les_cases_vides() {
    let c = &compagnie("verifier");
    // Une fiche entreprise : la forme declaree se deduit de la forme lisible.
    let r = resultat(c, 
        "verifier-fiche",
        json!({"nom": "entreprise", "donnees": {"commune_affichage": "Saint-Jean-d’Arvey", "code_postal": "73190"}}),
    );
    assert_eq!(r["declare"]["commune"], json!("SAINT JEAN D ARVEY"));
    assert!(r["anomalies"].as_array().unwrap().iter().all(|a| a["champ"] != "commune"), "la commune derivee n'est pas reprochee");
    assert_eq!(r["suggestions"]["code_urssaf"]["valeur"].as_str().map(str::len), Some(3), "l'organisme du departement, d'apres le code postal");
    // Une case remplie ne recoit pas de suggestion.
    let r = resultat(c, "verifier-fiche", json!({"nom": "entreprise", "donnees": {"code_postal": "73190", "code_urssaf": "117"}}));
    assert_eq!(r["suggestions"], json!({}));
    // Le NIR dit le departement de naissance.
    let mut s = fiche_salarie();
    s["nir"] = json!("1881105999048");
    s["departement_naissance"] = json!("");
    s["pays_naissance"] = json!(null);
    let r = resultat(c, "verifier-fiche", json!({"nom": "salarie", "donnees": s}));
    assert_eq!(r["suggestions"]["departement_naissance"]["valeur"], json!("05"));
    assert_eq!(r["suggestions"]["pays_naissance"]["valeur"], json!("FR"));
    // Un spectacle n'a rien a proposer.
    assert_eq!(resultat(c, "verifier-fiche", json!({"nom": "spectacle", "donnees": {"titre": "x"}}))["suggestions"], json!({}));
}

#[test]
fn verifier_fiche_refuse_une_charge_ou_des_valeurs_qui_ne_se_lisent_pas() {
    let c = &compagnie("verifier-refus");
    let mut code_entier = fiche_salarie();
    code_entier["code_emploi_ft"] = json!(12);
    for charge in [
        json!({}),
        json!({"nom": "salarie"}),
        json!({"nom": "autre", "donnees": {}}),
        json!({"nom": "salarie", "donnees": null}),
        json!({"nom": "salarie", "donnees": []}),
        json!({"nom": "entreprise", "donnees": {"siret": true}}),
        json!({"nom": "entreprise", "donnees": {"siret": "1", "siren": 12, "nic": "3"}}),
        json!({"nom": "salarie", "donnees": code_entier}),
    ] {
        assert!(refuse(c, "verifier-fiche", charge.clone()), "{}", charge);
    }
}

// ------------------------------------------------------ changements-declares

fn premiere_cle_salarie() -> String {
    let t = lire(&fictive("fictive-3090-petite"), "config/salaries.json");
    t.as_object().unwrap().keys().find(|k| !k.starts_with('_')).unwrap().clone()
}

#[test]
fn changements_declares_derive_d_abord_et_ne_date_que_le_declare() {
    let c = &compagnie("changements");
    let cle = premiere_cle_salarie();
    let mut s = fiche_salarie();
    let avant = s["commune"].clone();
    // Inchange : rien a dater. Un champ non declare : rien non plus.
    assert_eq!(resultat(c, "changements-declares", json!({"nom": "salarie", "cle": cle, "donnees": s}))["changements"], json!([]));
    s["initiales_contrat"] = json!("ZZ");
    assert_eq!(resultat(c, "changements-declares", json!({"nom": "salarie", "cle": cle, "donnees": s}))["changements"], json!([]));
    // La forme lisible change, la forme declaree est retiree : elle est derivee, comparee, montree sous sa forme lisible.
    s.as_object_mut().unwrap().remove("commune");
    s["commune_affichage"] = json!("Une Commune Qui N Existe Pas");
    let r = resultat(c, "changements-declares", json!({"nom": "salarie", "cle": cle, "donnees": s}));
    let ch = &r["changements"][0];
    assert_eq!(ch["champ"], json!("commune"));
    assert_eq!(ch["avant"], avant);
    assert_eq!(ch["apres"], json!("UNE COMMUNE QUI N EXISTE PAS"));
    assert_eq!(ch["apres_lisible"], json!("Une Commune Qui N Existe Pas"));
    assert_eq!(ch["silencieux"], json!(false));
    assert_eq!(ch["deja_declare"], json!({"declarations": 0, "du": null, "au": null}));
}

#[test]
fn changements_declares_compte_les_dsn_deja_parties_et_date_le_nir_qui_arrive() {
    let c = &compagnie("changements-dsn");
    let cle = premiere_cle_salarie();
    let mut s = fiche_salarie();
    let avant = s["commune"].as_str().unwrap().to_string();
    // Deux DSN deposees portent la valeur sortante (fins de ligne mixtes) ; une troisieme non.
    ecrire_texte(c, "sorties/2026-01/a.dsn", &format!("S20.G00.05.005,'01012026'\r\nS21.G00.30.010,'{}'\r\n", avant));
    ecrire_texte(c, "sorties/2025-03/b.dsn", &format!("S20.G00.05.005,'01032025'\nx,'{}'\n", avant));
    ecrire_texte(c, "sorties/2025-04/c.dsn", "S20.G00.05.005,'01042025'\nx,'AUTRE'\n");
    s["commune_affichage"] = json!("Ailleurs");
    let r = resultat(c, "changements-declares", json!({"nom": "salarie", "cle": cle, "donnees": s}));
    assert_eq!(r["changements"][0]["deja_declare"], json!({"declarations": 2, "du": "2025-03", "au": "2026-01"}));
    // Le NIR qui arrive sur une fiche qui n'en avait pas est la seule case vide a dater.
    let mut sal = lire(c, "config/salaries.json");
    sal[&cle]["nir"] = json!("");
    ecrire(c, "config/salaries.json", &sal);
    let mut s = fiche_salarie();
    s["nir"] = json!("2881175999001");
    let r = resultat(c, "changements-declares", json!({"nom": "salarie", "cle": cle, "donnees": s}));
    assert_eq!(r["changements"][0]["champ"], json!("nir"));
    assert_eq!(r["changements"][0]["avant"], json!(""));
    // Une fiche neuve (cle inconnue) ne date aucune case vide.
    let r = resultat(c, "changements-declares", json!({"nom": "salarie", "cle": "NEUF", "donnees": {"nir": "2881175999001", "nom": "X"}}));
    assert_eq!(r["changements"], json!([]));
}

#[test]
fn changements_declares_garde_les_notes_internes_de_la_fiche() {
    let c = &compagnie("changements-notes");
    let mut e = lire(c, "config/entreprise.json");
    e["raison_sociale"] = json!("LE FIL TENDU (DEMONSTRATION)");
    e["signataire"]["_note"] = json!("pourquoi");
    ecrire(c, "config/entreprise.json", &e);
    // L'ecran renvoie la fiche sans notes : aucun changement declare n'apparait.
    let mut envoi = e.clone();
    envoi.as_object_mut().unwrap().retain(|k, _| !k.starts_with('_'));
    envoi["signataire"].as_object_mut().unwrap().remove("_note");
    assert_eq!(resultat(c, "changements-declares", json!({"nom": "entreprise", "donnees": envoi}))["changements"], json!([]));
    // La forme lisible d'une personne ne se declare pas.
    envoi["signataire"]["prenom_affichage"] = json!("Géorge");
    assert_eq!(resultat(c, "changements-declares", json!({"nom": "entreprise", "donnees": envoi}))["changements"], json!([]));
}

#[test]
fn changements_declares_refuse_une_fiche_inconnue_ou_une_charge_qui_ne_se_lit_pas() {
    let c = &compagnie("changements-refus");
    let r = route(c, "changements-declares", json!({"nom": "autre", "donnees": {}}), "2026-10-03");
    assert!(r["ok"] == json!(false) && r["erreur"].as_str().unwrap().contains("autre"), "le refus nomme la fiche : {}", r);
    for charge in [json!({}), json!({"nom": "salarie"}), json!({"nom": ["a"], "donnees": {}}), json!({"nom": "salarie", "cle": ["a"], "donnees": {}}), json!({"nom": "salarie", "donnees": null})] {
        assert!(refuse(c, "changements-declares", charge.clone()), "{}", charge);
    }
}

// ------------------------------------------------------------ permanents

/// Le curseur des permanents (ticket #214) : la page le redemande au moteur quand on coche la case, sur la fiche en
/// cours ; sans fiche, c'est celle de la compagnie. Absent vaut non.
#[test]
fn le_curseur_des_permanents_est_la_reponse_du_moteur() {
    let c = &compagnie("permanents");
    let oui = |charge: Value| resultat(c, "permanents", charge)["permanents"].clone();
    assert_eq!(oui(json!({"entreprise": {"emploie_des_permanents": true}})), json!(true));
    assert_eq!(oui(json!({"entreprise": {"emploie_des_permanents": false}})), json!(false));
    assert_eq!(oui(json!({"entreprise": {}})), json!(false));
    // 3090-petite n'emploie que des intermittents.
    assert_eq!(oui(json!({})), json!(false));
    assert!(refuse(c, "permanents", json!({"entreprise": [true]})));
}

// ------------------------------------------------- taxe-sur-les-salaires-regime

/// Ticket #216 : la compagnie dit sa TVA, le moteur conclut (CGI art. 231, 1, depose :
/// `atelier/reference/sources/code-general-des-impots/xml/231.xml`) et dit quelles cases suivent. Chaque case n'apparait
/// que si la precedente l'appelle.
#[test]
fn la_taxe_sur_les_salaires_se_deduit_de_la_tva() {
    let c = &compagnie("taxe-salaires");
    let r = |ts: Value| resultat(c, "taxe-sur-les-salaires-regime", json!({ "taxe_sur_les_salaires": ts }));
    let champs = |v: &Value| v["champs"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect::<Vec<_>>();
    // Rien de dit : la seule question est la TVA, et la reponse est incomplete.
    let v = r(Value::Null);
    assert_eq!((champs(&v), v["complet"].clone()), (vec!["tva".to_string()], json!(false)));
    // Assujettie : la part de ses recettes soumise a la TVA. A 90 % ou plus, rien n'est du et rien d'autre ne se demande.
    let v = r(json!({"tva": "assujettie"}));
    assert_eq!((champs(&v), v["complet"].clone()), (vec!["tva".to_string(), "part_tva".to_string()], json!(false)));
    let v = r(json!({"tva": "assujettie", "part_tva": 92}));
    assert_eq!((champs(&v), v["complet"].clone(), v["due"].clone()), (vec!["tva".to_string(), "part_tva".to_string()], json!(true), json!(false)));
    // Sous 90 % : due sur le reste, sauf sous les plafonds de 293 B (2e alinea) -- la question suit.
    let v = r(json!({"tva": "assujettie", "part_tva": 40}));
    assert_eq!((champs(&v).last().cloned(), v["complet"].clone()), (Some("sous_franchise".to_string()), json!(false)));
    let v = r(json!({"tva": "assujettie", "part_tva": 40, "sous_franchise": false}));
    assert_eq!((v["due"].clone(), v["part_taxee"].clone()), (json!(true), json!("60")));
    // L'abattement des associations (art. 1679 A) ne se demande que si la taxe est due.
    assert_eq!(champs(&v).last().map(String::as_str), Some("association_1679A"));
    let v = r(json!({"tva": "assujettie", "part_tva": 40, "sous_franchise": true}));
    assert_eq!((v["due"].clone(), v["complet"].clone(), champs(&v).contains(&"association_1679A".to_string())), (json!(false), json!(true), false));
    // Exoneree de TVA : due sur tout, sans part a dire.
    let v = r(json!({"tva": "exoneree", "sous_franchise": false, "part_tva": 12}));
    assert_eq!((v["part_taxee"].clone(), champs(&v).contains(&"part_tva".to_string())), (json!("100"), false));
    // Une conclusion se dit toujours en clair.
    assert!(v["consequence"].as_str().is_some_and(|s| !s.is_empty()));
    // Sans charge : la fiche de la compagnie (assujettie sur toutes ses recettes).
    let v = resultat(c, "taxe-sur-les-salaires-regime", json!({}));
    assert_eq!((v["complet"].clone(), v["due"].clone()), (json!(true), json!(false)));
    assert!(refuse(c, "taxe-sur-les-salaires-regime", json!({"taxe_sur_les_salaires": "assujettie"})));
}

/// Ticket #239 : « on pourrait pas avoir les plafonds directement dans le logiciel ? ». Les plafonds de la franchise en
/// base de TVA auxquels renvoie CGI art. 231, 1, 2e alinea sont ceux de 293 B, I, ligne « Annee civile precedente »
/// (version en vigueur depuis le 01/03/2025, deposee par `paie releve-codes` :
/// `atelier/reference/sources/code-general-des-impots/xml/293_B.xml`). Ils sont au bareme de l'annee, dont le `_source`
/// cite ce fichier ; la route les ecrit dans l'aide et dans les deux reponses, et la page n'en porte aucun.
#[test]
fn les_plafonds_de_la_franchise_viennent_du_bareme_qui_cite_le_texte_depose() {
    let c = &compagnie("franchise-293B");
    let sources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference/sources");
    let xml = std::fs::read_to_string(sources.join("code-general-des-impots/xml/293_B.xml")).unwrap();
    // Le I de 293 B : la premiere ligne « Annee civile precedente », puis le total et les prestations de services.
    let i = xml.find("Année civile précédente").expect("293 B, I : la ligne de l'annee civile precedente");
    let nombres: Vec<i64> = regex::Regex::new(r"(\d{2}) (\d{3})")
        .unwrap()
        .captures_iter(&xml[i..])
        .take(2)
        .map(|m| format!("{}{}", &m[1], &m[2]).parse().unwrap())
        .collect();
    let (total, services) = (nombres[0], nombres[1]);
    for annee in [2025, 2026] {
        let bar = lire(&crate::outils::code_livre(), &format!("baremes/{}.json", annee));
        let pf = &bar["taxe_sur_les_salaires"]["plafonds_franchise"];
        assert!(pf["_source"].as_str().unwrap().contains("code-general-des-impots/xml/293_B.xml"), "{} : _source", annee);
        assert_eq!((pf["total"].as_i64(), pf["prestations_de_services"].as_i64()), (Some(total), Some(services)), "{}", annee);
        let v = resultat(c, "taxe-sur-les-salaires-regime", json!({"annee": annee, "taxe_sur_les_salaires": {"tva": "exoneree"}}));
        let f = &v["franchise"];
        assert_eq!(
            (f["plafonds"]["total"].clone(), f["plafonds"]["prestations_de_services"].clone()),
            (json!(total.to_string()), json!(services.to_string())),
            "{}",
            annee
        );
        // Les montants sont ecrits, en euros, la ou on les lit : l'aide et les deux reponses ; le chiffre d'affaires est
        // celui de l'annee d'avant.
        let en_euros = |n: i64| format!("{}\u{a0}{:03}\u{a0}€", n / 1000, n % 1000);
        for texte in [&f["aide"], &f["choix"]["true"], &f["choix"]["false"]] {
            let t = texte.as_str().unwrap();
            assert!(t.contains(&en_euros(services)) && t.contains(&en_euros(total)), "{} : {}", annee, t);
            assert!(t.contains(&(annee - 1).to_string()), "{} : {}", annee, t);
        }
        assert!(f["aide"].as_str().unwrap().contains("293 B"));
        // Ticket #245 : la question est posee a la compagnie exoneree de TVA (la taxe serait due sur tout), et la
        // conclusion sous les cases nomme les deux plafonds, qu'on reponde ou non.
        assert_eq!(v["champs"].as_array().unwrap().last(), Some(&json!("sous_franchise")), "{}", annee);
        for sous in [None, Some(true)] {
            let mut ts = json!({"tva": "exoneree"});
            if let Some(s) = sous {
                ts["sous_franchise"] = json!(s);
            }
            let v = resultat(c, "taxe-sur-les-salaires-regime", json!({"annee": annee, "taxe_sur_les_salaires": ts}));
            let t = v["consequence"].as_str().unwrap();
            assert!(t.contains(&en_euros(services)) && t.contains(&en_euros(total)), "{} : {}", annee, t);
        }
        // Assujettie sur 90 % ou plus de ses recettes : la taxe n'est pas due, la question ne se pose pas.
        let v = resultat(c, "taxe-sur-les-salaires-regime", json!({"annee": annee, "taxe_sur_les_salaires": {"tva": "assujettie", "part_tva": 95}}));
        assert!(!v["champs"].as_array().unwrap().contains(&json!("sous_franchise")), "{}", annee);
    }
    // Sans annee : celle du jour (03/10/2026 pour ces routes).
    assert_eq!(resultat(c, "taxe-sur-les-salaires-regime", json!({}))["franchise"]["annee"], json!(2026));
    assert!(refuse(c, "taxe-sur-les-salaires-regime", json!({"annee": "l'an dernier"})));
}
