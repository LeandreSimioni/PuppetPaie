//! Ce qui part pour deboguer (`diagnostic`, Leandre 23/10/2026 : « un max d'info [...] Pas sur leurs salaries, mais sur
//! la machine un max ») : LA FRONTIERE -- aucune donnee de personne d'une charge de route connue ne passe, les montants,
//! dates, codes, spectacles et lieux passent --, le fil des trois minutes d'avant, et l'incident qui porte sa route, son
//! fil et sa machine, compte une fois quand il se repete.
//!
//! Les charges sont celles que la page envoie (`js/fiche.js` : `enregistrer-fiche` ; `js/saisie-mois.js` :
//! `enregistrer-saisie` ; `js/actes.js` : `avenant-envoyer`), montees sur la copie figee de la fictive
//! `fictive-3090-petite` (inventee).

use crate::outils::{copier, dossier_de_test};
use paie::diagnostic;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn fictive() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/donnees/fictives/fictive-3090-petite")
}

fn lire(p: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap()
}

/// Tout ce qui, dans les fiches de la fictive, designe une personne ou la compagnie.
fn ce_qui_designe(racine: &Path) -> Vec<String> {
    let mut v = Vec::new();
    for (cle, f) in lire(&racine.join("config/salaries.json")).as_object().unwrap().iter().filter(|(k, _)| !k.starts_with('_')) {
        v.push(cle.clone());
        for c in [
            "nom", "prenom", "nom_affichage", "prenom_affichage", "nir", "date_naissance", "lieu_naissance", "lieu_naissance_affichage",
            "adresse", "adresse_affichage", "email", "telephone", "iban", "bic",
        ] {
            if let Some(Value::String(s)) = f.get(c) {
                if s.chars().count() >= 3 {
                    v.push(s.clone());
                }
            }
        }
    }
    let e = lire(&racine.join("config/entreprise.json"));
    for c in ["raison_sociale", "siren", "siret", "adresse", "urssaf_compte"] {
        if let Some(Value::String(s)) = e.get(c) {
            v.push(s.clone());
        }
    }
    assert!(v.len() > 10, "la fictive doit avoir des salaries a cacher");
    v
}

fn sans_accents_minuscules(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'à' | 'â' | 'ä' => 'a',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            x => x,
        })
        .collect()
}

#[test]
fn aucune_donnee_de_personne_d_une_charge_de_route_ne_passe_et_les_montants_et_dates_passent() {
    let racine = fictive();
    let salaries = lire(&racine.join("config/salaries.json"));
    let mut charges = Vec::new();
    for (cle, fiche) in salaries.as_object().unwrap().iter().filter(|(k, _)| !k.starts_with('_')) {
        charges.push(json!({"route": "enregistrer-fiche", "charge": {"nom": "salarie", "cle": cle, "donnees": fiche}}));
    }
    let saisie = lire(&racine.join("paies/2026-10.json"));
    charges.push(json!({"route": "enregistrer-saisie", "charge": {"mois": "2026-10", "saisie": saisie}}));
    let un = saisie["engagements"][0]["salarie"].as_str().unwrap().to_string();
    charges.push(json!({"route": "avenant-envoyer", "charge": {"salarie": un, "debut": "2026-10-13", "mois": "2026-10", "motdepasse": "Tres-secret-42"}}));
    charges.push(json!({"route": "signalement", "charge": {"texte": format!(
        "La paie de {} ne passe pas : appeler le 06 12 34 56 78 ou {}, NIR 2 88 11 75 999 001 46, IBAN FR14 2004 1010 0505 0001 3M02 606",
        un, salaries[&un.to_uppercase()]["email"].as_str().unwrap_or("x@exemple.invalid")
    )}}));
    let forme = diagnostic::en_forme(Some(&racine), &Value::Array(charges.clone()));
    let texte = sans_accents_minuscules(&forme.to_string());
    for d in ce_qui_designe(&racine) {
        assert!(!texte.contains(&sans_accents_minuscules(&d)), "« {} » passe la frontiere : {}", d, forme);
    }
    for secret in ["tres-secret-42", "06 12 34 56 78", "fr14", "2 88 11 75"] {
        assert!(!texte.contains(secret), "« {} » passe : {}", secret, forme);
    }
    // Ce qui sert a reproduire reste : les montants, les quantites, les dates, le numero d'objet, le lieu, la rubrique.
    let saisie_forme = &forme[charges.len() - 3]["charge"]["saisie"];
    assert_eq!(saisie_forme["taux_cachet"], saisie["taux_cachet"]);
    assert_eq!(saisie_forme["engagements"][0]["lignes"], saisie["engagements"][0]["lignes"], "lignes de paie intactes");
    assert_eq!(saisie_forme["engagements"][0]["debut"], saisie["engagements"][0]["debut"]);
    assert_eq!(saisie_forme["engagements"][0]["spectacle"], saisie["engagements"][0]["spectacle"]);
    // Le salarie devient son numero (celui des bulletins), le meme partout ; un NIR dit sa forme.
    let numero = format!("salarié n°{}", salaries[&un.to_uppercase()]["code_salarie"].as_str().unwrap());
    assert_eq!(saisie_forme["engagements"][0]["salarie"], json!(numero));
    assert_eq!(forme[charges.len() - 2]["charge"]["salarie"], json!(numero));
    assert_eq!(forme[charges.len() - 2]["charge"]["motdepasse"], json!("[secret retiré]"));
    let fiche = &forme[0]["charge"]["donnees"];
    assert_eq!(fiche["nir"], json!("NIR présent, clé valide"));
    assert_eq!(fiche["emploi"], salaries.as_object().unwrap().values().next().unwrap()["emploi"], "le metier reste");
    assert_eq!(forme[0]["charge"]["nom"], json!("salarie"), "la sorte de fiche reste");
}

#[test]
fn le_fil_dit_les_trois_minutes_d_avant_une_ligne_par_action_sans_personne() {
    let racine = fictive();
    diagnostic::oublier_le_fil();
    let t = diagnostic::maintenant_ms();
    let lot = json!({
        "page": {"ecran": "1920×1080", "echelle": 1.25, "fuseau": "Europe/Paris", "agent": "Mozilla/5.0 Edg/129.0"},
        "gestes": [
            {"t": t - 161_000, "quoi": "clic", "cible": {"balise": "button", "id": "", "classe": "carte", "texte": "Fanchon Brioche"}, "vue": "paie"},
            {"t": t - 120_000, "quoi": "champ", "nom": "f-prenom", "type": "text", "valeur": "Fanchon"},
            {"t": t - 100_000, "quoi": "champ", "nom": "quantite", "type": "number", "valeur": "4"},
            {"t": t - 90_000, "quoi": "champ", "nom": "mdp", "type": "password", "secret": true},
            {"t": t - 400_000, "quoi": "clic", "cible": {"balise": "a", "texte": "trop vieux"}},
        ],
    });
    diagnostic::noter_gestes(&racine, &lot);
    for _ in 0..10 {
        diagnostic::noter_route(&racine, "page", "mois", &json!({"mois": "2026-10", "attendre": true}), std::time::Duration::from_millis(3), Err(("ErreurOperation", "calcul du mois : une paie de moins que la saisie")));
    }
    let fil = diagnostic::fil(diagnostic::maintenant_ms());
    let tout = fil.join("\n");
    assert!(fil[0].starts_with("-2:4"), "{}", tout);
    assert!(tout.contains("clic « [salarié n°1] » (button.carte) — écran paie"), "{}", tout);
    assert!(tout.contains("champ quantite (number) = \"4\""), "{}", tout);
    assert!(tout.contains("champ mdp (password) = \"[secret retiré]\""), "{}", tout);
    assert!(!tout.to_lowercase().contains("fanchon") && !tout.to_lowercase().contains("brioche"), "{}", tout);
    assert!(!tout.contains("trop vieux"), "plus de trois minutes : hors du fil\n{}", tout);
    // Dix refus identiques en une seconde : une ligne, comptee.
    let refus: Vec<&String> = fil.iter().filter(|l| l.contains("route mois")).collect();
    assert_eq!(refus.len(), 1, "{}", tout);
    assert!(refus[0].contains("{\"mois\":\"2026-10\",\"attendre\":true}") && refus[0].contains("(×10"), "{}", refus[0]);
    assert_eq!(diagnostic::page()["fuseau"], json!("Europe/Paris"));
}

#[test]
fn un_refus_repete_fait_un_incident_compte_qui_porte_sa_route_son_fil_et_sa_machine() {
    let racine = dossier_de_test("diagnostic", "incident").join("compagnie");
    copier(&fictive().join("config"), &racine.join("config"));
    std::fs::create_dir_all(racine.join("journal")).unwrap();
    diagnostic::oublier_le_fil();
    diagnostic::noter_route(&racine, "page", "enregistrer-saisie", &json!({"mois": "2026-10"}), std::time::Duration::from_millis(12), Ok(()));
    let detail = json!({"route": "mois", "charge": {"mois": "2026-10", "salarie": "Brioche"}});
    for _ in 0..10 {
        paie::incidents::depuis_exception(&racine, "ErreurOperation", "calcul du mois : une paie de moins que la saisie (Brioche)", None, "mois", detail.clone(), Some("endroit : src/routes_mois.rs:625"));
    }
    let lignes = paie::incidents::lire(&racine, None, false).unwrap();
    assert_eq!(lignes.len(), 1, "dix fois le meme refus : une ligne");
    let l = &lignes[0];
    assert_eq!(l["fois"], json!(10));
    assert_eq!(l["detail"]["charge"]["salarie"], json!("salarié n°1"));
    assert!(!l.to_string().contains("Brioche"), "{}", l);
    assert!(l["fil"].as_array().unwrap().iter().any(|x| x.as_str().unwrap().contains("route enregistrer-saisie {\"mois\":\"2026-10\"} → ok")), "{}", l["fil"]);
    assert!(l["machine_detail"]["version"].is_string() && l["machine_detail"]["compagnie"].is_string());
    let part = paie::incidents::pour_l_editeur(&racine, l).unwrap();
    assert_eq!((part["fois"].clone(), part["fil"].clone()), (json!(10), l["fil"].clone()));
    assert_eq!(part["detail"]["charge"]["salarie"], json!("salarié n°1"), "une forme reste une forme au depart");
    // Une ligne notee avant la frontiere (le journal d'un poste qui a connu l'ancien moteur) part sans son nom.
    let mut ancienne = l.clone();
    ancienne["resume"] = json!("ErreurOperation: Aucun document sur la période (décembre 2026) pour BRIOCHE.");
    ancienne["trace"] = json!("Traceback ...\nErreurOperation: Aucun document pour Brioche Fanchon.");
    let part = paie::incidents::pour_l_editeur(&racine, &ancienne).unwrap().to_string().to_lowercase();
    assert!(!part.contains("brioche") && !part.contains("fanchon") && part.contains("décembre 2026"), "{}", part);
    assert_eq!(paie::incidents::regrouper(&racine, None).unwrap()[0]["combien"], json!(10));
    // La saisie ecrite avant le refus part entiere (mise en forme) : de quoi le rejouer.
    assert_eq!(l["ecritures"][0]["route"], json!("enregistrer-saisie"), "{}", l["ecritures"]);
    assert_eq!(l["ecritures"][0]["charge"], json!({"mois": "2026-10"}));
}

/// La trace d'un refus dit l'endroit du code ou son echec a ete construit, et la chaine des echecs d'avant sur le meme fil.
#[test]
fn la_trace_d_un_refus_dit_l_endroit_du_code() {
    let racine = fictive();
    let debut = diagnostic::maintenant_ms();
    diagnostic::noter_echec(Some("KeyError"), "cle absente : taux");
    diagnostic::noter_echec(Some("ErreurOperation"), "le mois de Brioche ne se calcule pas");
    let trace = diagnostic::trace_du_refus(&racine, "le mois de Brioche ne se calcule pas", debut).unwrap();
    let ici = format!("tests/fichiers/diagnostic.rs:{}", line!() - 2);
    assert!(trace.starts_with(&format!("endroit : {} (ErreurOperation)", ici)), "{}", trace);
    assert!(trace.contains("KeyError") && trace.contains("[salarié n°1]") && !trace.contains("Brioche"), "{}", trace);
}
