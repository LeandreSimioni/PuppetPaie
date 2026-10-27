//! La route `dsn-a-deposer` : la DSN du mois se fabrique a son depot, depuis les engagements valides, constituee le jour
//! meme, avec le contact du declarant et les emplois multiples ; un mois depose repart en annule-et-remplace, et ne
//! repart plus apres la veille de son echeance ; un exercice clos refuse sans rien ecrire. Sur `fictive-3090-grande` :
//! config, journal, toutes ses saisies, ses DSN deposees.

use crate::outils::{compagnie, copier, dossier_de_test, ecrire, fictive, lire, noms};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn compagnie(nom: &str, sorties: &[&str]) -> PathBuf {
    let racine = compagnie::config_et_mois("fictive-3090-grande", sorties, &dossier_de_test("dsn_a_deposer", nom).join("compagnie"));
    copier(&fictive("fictive-3090-grande").join("engagements"), &racine.join("engagements"));
    // Les DSN deposees, sans les PDF : l'annule-et-remplace relit ce qui est parti.
    let sorties = fictive("fictive-3090-grande").join("sorties");
    for m in noms(&sorties) {
        for f in noms(&sorties.join(&m)).into_iter().filter(|f| f.ends_with(".dsn")) {
            std::fs::create_dir_all(racine.join("sorties").join(&m)).unwrap();
            std::fs::copy(sorties.join(&m).join(&f), racine.join("sorties").join(&m).join(&f)).unwrap();
        }
    }
    racine
}

/// La DSN de `mois`, fabriquee le `jour`.
fn dsn(racine: &Path, jour: &str, mois: &str) -> Value {
    crate::outils::route(racine, "dsn-a-deposer", json!({"mois": mois}), jour)
}

/// La date de constitution (S20.G00.05.007) d'une DSN ecrite, `JJMMAAAA`.
fn constitution_de(racine: &Path, rel: &str) -> String {
    rubrique_de(racine, rel, "S20.G00.05.007")
}

#[test]
fn un_mois_deja_depose_repart_en_annule_et_remplace() {
    // Cahier technique DSN 2026.1, §1.4.2 : une DSN deja deposee se remplace en entier (type 03, S20.G00.05.002), a
    // l'ordre suivant, jusqu'a la veille de son echeance.
    let r = compagnie("ar", &["2026-03"]);
    let rep = dsn(&r, "2026-04-10", "2026-03");
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let res = &rep["resultat"];
    assert_eq!((&res["type"], &res["ordre"], &res["remplace"]), (&json!("03"), &json!(2), &json!(true)), "{}", res);
    assert!(res["chemin"].as_str().unwrap().ends_with("DSN_Mars2026_AR2.dsn"), "{}", res);
    assert!(res["informations"].as_array().unwrap().iter().any(|i| i.as_str().unwrap().contains("ANNULE-ET-REMPLACE")));
    // Constituee le jour ou elle se fabrique, pas a la fin de periode du mois.
    assert_eq!(constitution_de(&r, "sorties/2026-03/DSN_Mars2026_AR2.dsn"), "10042026");
    assert!(lire(&r, "journal/dsn-ecrites.json").get("2026-03/DSN_Mars2026_AR2.dsn").is_some());
    // La DSN deja partie reste dans le dossier ; c'est l'annule-et-remplace qui se depose, et la boite de depot le montre.
    let e = crate::outils::route(&r, "etat-du-mois", json!({"mois": "2026-03"}), "2026-04-10");
    assert_eq!(e["resultat"]["fichiers"]["dsn"][0]["nom"], json!("DSN_Mars2026_AR2.dsn"), "{}", e["resultat"]["fichiers"]);
    assert!(r.join("sorties/2026-03/DSN_Mars2026.dsn").is_file());
    let a = crate::outils::route(&r, "depot-apercu", json!({"quoi": "dsn", "fichier": res["chemin"]}), "2026-04-10");
    assert_eq!((&a["resultat"]["type_declaration"], &a["resultat"]["ordre"]), (&json!("03"), &json!("2")), "{}", a);
}

#[test]
fn apres_la_veille_de_l_echeance_un_mois_depose_ne_se_redepose_pas_et_rien_n_est_ecrit() {
    // Cahier technique DSN 2026.1, §1.4.2 : l'annule-et-remplace n'est plus admis apres la veille de l'echeance.
    let r = compagnie("delai", &["2026-08"]);
    let avant = noms(&r.join("sorties/2026-08"));
    let rep = dsn(&r, "2026-10-06", "2026-08");
    assert_eq!(rep["ok"], json!(false), "{}", rep);
    assert_eq!(rep["classe"], json!("AnnuleRemplaceHorsDelai"), "{}", rep);
    assert_eq!(noms(&r.join("sorties/2026-08")), avant);
}

#[test]
fn un_exercice_clos_ne_fabrique_pas_de_dsn() {
    let r = compagnie("clos", &[]);
    let rep = dsn(&r, "2025-07-10", "2025-06");
    assert_eq!(rep["ok"], json!(false));
    assert!(rep["erreur"].as_str().unwrap().contains("cloture"), "{}", rep);
    assert!(!r.join("sorties/2025-06").exists());
}

// ------------------------------------------------------------------ le contact emetteur


fn rubrique_de(racine: &Path, rel: &str, code: &str) -> String {
    let texte: String = std::fs::read(racine.join(rel)).unwrap().iter().map(|&o| o as char).collect();
    let ligne = texte.lines().find(|l| l.starts_with(&format!("{},", code))).unwrap_or_else(|| panic!("pas de {} dans {}", code, rel));
    ligne.trim_end()[code.len() + 1..].trim_matches('\'').to_string()
}

/// La personne que les organismes contactent n'est plus celle qui signe : l'administrateur fait la paie, la directrice signe.
fn contact_administrateur(racine: &Path, civilite: Option<&str>) {
    let mut e = lire(racine, "config/entreprise.json");
    let mut d = json!({"nom_prenom": "MOLIERE JEAN", "tel": "0100000099", "email": "ADMINISTRATION@DEMO.INVALID", "qualite": "04"});
    if let Some(c) = civilite {
        d["civilite"] = json!(c);
    }
    e["declarant_dsn"] = d;
    ecrire(racine, "config/entreprise.json", &e);
}

#[test]
fn le_contact_de_la_dsn_est_une_seule_personne_avec_sa_civilite() {
    // Cahier technique DSN 2026.1, p. 132 : S10.G00.02 est l'« Acteur a contacter », 02.001 « 01 - monsieur / 02 - madame »,
    // 02.002 « Nom, prenom de l'agent de l'emetteur ». Le moteur ecrivait 01 en dur et le nom du signataire (ici la
    // directrice, LECOUVREUR ADRIENNE) a cote du courriel et du telephone du declarant.
    let r = compagnie("contact", &[]);
    contact_administrateur(&r, Some("01"));
    let rep = dsn(&r, "2026-11-10", "2026-10");
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let f = "sorties/2026-10/DSN_Octobre2026.dsn";
    assert_eq!(
        ["S10.G00.02.001", "S10.G00.02.002", "S10.G00.02.004", "S10.G00.02.005", "S20.G00.07.001"].map(|c| rubrique_de(&r, f, c)),
        ["01", "MOLIERE JEAN", "ADMINISTRATION@DEMO.INVALID", "0100000099", "MOLIERE JEAN"].map(String::from)
    );
}

#[test]
fn sans_civilite_du_contact_la_dsn_refuse_et_dit_ou_la_saisir() {
    let r = compagnie("contact-sans-civilite", &[]);
    contact_administrateur(&r, None);
    let rep = dsn(&r, "2026-11-10", "2026-10");
    assert_eq!(rep["ok"], json!(false), "{}", rep);
    let m = rep["erreur"].as_str().unwrap_or_default();
    assert!(m.contains("civilité") && m.contains("Qui les organismes contactent") && m.contains("S10.G00.02.001"), "{}", m);
    assert!(!r.join("sorties/2026-10/DSN_Octobre2026.dsn").exists());
}

// ------------------------------------------------------------------ les emplois multiples (V1h, point 7)

/// `S21.G00.40.036` de chaque contrat, par nom de salarie (`S21.G00.30.002`), dans l'ordre du fichier.
fn emplois_multiples_par_salarie(racine: &Path, rel: &str) -> Vec<(String, String)> {
    let texte: String = std::fs::read(racine.join(rel)).unwrap().iter().map(|&o| o as char).collect();
    let mut nom = String::new();
    let mut v = Vec::new();
    for l in texte.lines() {
        let valeur = || l.split_once(',').unwrap().1.trim_end().trim_matches('\'').to_string();
        if l.starts_with("S21.G00.30.002,") {
            nom = valeur();
        } else if l.starts_with("S21.G00.40.036,") {
            v.push((nom.clone(), valeur()));
        }
    }
    v
}

#[test]
fn un_salarie_qui_tient_deux_emplois_dans_le_mois_est_en_emplois_multiples() {
    // Cahier technique DSN 2026.1, p. 199 : « Cette rubrique précise si le salarié a plusieurs emplois chez un même
    // employeur au cours d'une même période de paie » (02 - emplois multiples). Le moteur ecrivait 01 en constante.
    // Biancolelli joue (son emploi de comedienne) puis assiste la mise en scene (MET042) en octobre : 02 sur ses deux
    // contrats. Clairon a deux contrats du meme emploi : 01. Les autres, un contrat : 01.
    // à trancher, §226 : deux contrats du meme emploi en 01 est une inference ; la norme ne tranche pas ce cas.
    let r = compagnie("emplois-multiples", &[]);
    let mut s = crate::outils::saisie(&r, "2026-10");
    let engagements = s["engagements"].as_array_mut().unwrap();
    for (qui, metier) in [("Biancolelli", Some("MET042")), ("Clairon", None)] {
        let mut second = engagements.iter().find(|e| e["salarie"] == json!(qui)).unwrap().clone();
        second["debut"] = json!("2026-10-20");
        second["fin"] = json!("2026-10-21");
        second["date_paiement"] = json!("2026-10-21");
        second["lignes"] = json!([{"rubrique": "cachet", "quantite": 2, "taux": 190.0, "lieu": "Theatre fictif de Saumur", "dates": ["2026-10-20", "2026-10-21"]}]);
        if let Some(m) = metier {
            second["metier"] = json!(m);
        }
        engagements.push(second);
    }
    crate::outils::poser_saisie(&r, "2026-10", &s);
    crate::outils::tout_valider(&r, "2026-10", "2026-11-10");
    let rep = dsn(&r, "2026-11-10", "2026-10");
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let vu = emplois_multiples_par_salarie(&r, "sorties/2026-10/DSN_Octobre2026.dsn");
    let de = |qui: &str| -> Vec<String> { vu.iter().filter(|(n, _)| n == qui).map(|(_, c)| c.clone()).collect() };
    assert_eq!(de("BIANCOLELLI"), ["02", "02"], "{:?}", vu);
    assert_eq!(de("CLAIRON"), ["01", "01"], "{:?}", vu);
    assert!(vu.iter().filter(|(n, _)| n != "BIANCOLELLI").all(|(_, c)| c == "01"), "{:?}", vu);
}
