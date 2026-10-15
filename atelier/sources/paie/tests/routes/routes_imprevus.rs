//! Les routes des imprevus (`routes_imprevus`, 15/10/2026) : `imprevus`, `suspension-enregistrer`, `-modifier`,
//! `-oublier`, `arret-maintien`, et `arret-contrats` pour un contrat paye au mois. Un refus n'ecrit rien ; deux imprevus
//! du meme contrat ne se chevauchent pas ; un imprevu deja declare ne s'oublie pas. Sur `fictive-3090-grande` (config,
//! saisies de juillet a octobre 2026 : Nicolet en CDI NI.24.01, Biancolelli en CDDU du 02 au 25/07).

use crate::outils::{compagnie, dossier_de_test, ecrire, lire, refus, reussi};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn compagnie(nom: &str) -> PathBuf {
    compagnie::saisies(
        "fictive-3090-grande",
        &["2026-07", "2026-08", "2026-09", "2026-10"],
        &dossier_de_test("routes_imprevus", nom).join("compagnie"),
    )
}

fn route(racine: &Path, nom: &str, charge: Value) -> Value {
    crate::outils::route(racine, nom, charge, "2026-10-15")
}

fn refuse(r: &Value) -> String {
    refus(r).1
}

fn conge_parental(debut: &str, fin: &str) -> Value {
    json!({"salarie": "Nicolet", "debut": debut, "fin": fin, "motif": "632"})
}

#[test]
fn un_cdi_qui_court_couvre_tout_jour_depuis_son_debut_et_se_dit_paye_au_mois() {
    let racine = compagnie("contrats");
    let r = route(&racine, "arret-contrats", json!({"salarie": "Nicolet", "jour": "2026-12-20"}));
    let c = &reussi(&r)["contrats"];
    assert_eq!(c.as_array().unwrap().len(), 1, "{}", r);
    assert_eq!((c[0]["numero"].clone(), c[0]["mensuel"].clone(), c[0]["fin"].clone()), (json!("NI.24.01"), json!(true), json!("")));
    let r = route(&racine, "arret-contrats", json!({"salarie": "Biancolelli", "jour": "2026-07-10"}));
    assert_eq!(reussi(&r)["contrats"][0]["mensuel"], json!(false), "{}", r);
}

#[test]
fn une_suspension_se_saisit_une_fois_se_modifie_et_s_oublie() {
    let racine = compagnie("suspension");
    let r = route(&racine, "suspension-enregistrer", conge_parental("2026-07-20", "2026-09-10"));
    let s = &reussi(&r)["suspension"];
    assert_eq!(s["genre"], json!("suspension"));
    assert_eq!((s["salarie"].clone(), s["contrat"].clone(), s["contrat_debut"].clone()), (json!("NICOLET"), json!("NI.24.01"), json!("2024-03-01")));
    assert_eq!((s["motif"].clone(), s["remunere"].clone()), (json!("632"), json!(false)));
    assert_eq!(s["reporte_par"], json!([]));
    // Le fichier : rangee au mois de son premier jour, tous ses champs.
    let f = lire(&racine, "imprevus/2026-07.json");
    let mut champs: Vec<&String> = f["imprevus"][0].as_object().unwrap().keys().collect();
    champs.sort();
    assert_eq!(champs, ["contrat_debut", "contrat_numero", "debut", "envois", "fin", "genre", "motif", "note", "qui", "saisi_le", "salarie"]);

    // La liste : d'un mois qu'elle traverse, oui ; d'un autre, non.
    let l = reussi(&route(&racine, "imprevus", json!({"mois": "2026-08"}))).clone();
    assert_eq!(l["imprevus"].as_array().unwrap().len(), 1, "{}", l);
    assert!(l["motifs_suspension"].as_array().unwrap().iter().all(|m| m["code"] != json!("200")), "le 200 ne se declare pas");
    assert!(!l["motifs_arret"].as_array().unwrap().is_empty() && l["subrogation"].is_array());
    assert!(reussi(&route(&racine, "imprevus", json!({"mois": "2026-10"})))["imprevus"].as_array().unwrap().is_empty());

    // Une seconde qui la chevauche, un arret qui la chevauche : refuses ; le meme debut : refuse.
    assert!(refuse(&route(&racine, "suspension-enregistrer", conge_parental("2026-09-01", "2026-09-30"))).contains("chevauche"));
    let arret = json!({"salarie": "Nicolet", "dernier_jour": "2026-08-10", "fin": "2026-08-20", "motif": "01", "subrogation": "02"});
    assert!(refuse(&route(&racine, "arret-enregistrer", arret)).contains("chevauche"));
    assert!(refuse(&route(&racine, "suspension-enregistrer", conge_parental("2026-07-20", "2026-07-25"))).contains("deja enregistree"));

    // Plus courte : elle finit le 31/08 ; septembre est libre.
    let r = route(&racine, "suspension-modifier", json!({"salarie": "Nicolet", "debut": "2026-07-20", "fin": "2026-08-31"}));
    assert_eq!(reussi(&r)["suspension"]["fin"], json!("2026-08-31"), "{}", r);
    assert_eq!(reussi(&route(&racine, "suspension-enregistrer", conge_parental("2026-09-01", "2026-09-30")))["suspension"]["debut"], json!("2026-09-01"));
    assert_eq!(lire(&racine, "imprevus/2026-09.json")["imprevus"].as_array().unwrap().len(), 1);

    // Oubliee : jamais declaree, elle se retire ; declaree par une DSN, elle ne se retire plus.
    reussi(&route(&racine, "suspension-oublier", json!({"salarie": "Nicolet", "debut": "2026-09-01"})));
    assert_eq!(lire(&racine, "imprevus/2026-09.json")["imprevus"], json!([]));
    let mut f = lire(&racine, "imprevus/2026-07.json");
    f["imprevus"][0]["envois"] = json!([{"nature": "01", "type": "01", "ordre": 0, "mois": "2026-07", "fichier": "x", "quand": "2026-08-05 10:00", "identifiant": ""}]);
    ecrire(&racine, "imprevus/2026-07.json", &f);
    assert_eq!(reussi(&route(&racine, "imprevus", json!({"salarie": "Nicolet"})))["imprevus"][0]["reporte_par"], json!(["2026-07"]));
    assert!(refuse(&route(&racine, "suspension-oublier", json!({"salarie": "Nicolet", "debut": "2026-07-20"}))).contains("deja ete declaree"));
}

#[test]
fn une_suspension_que_le_moteur_refuse_n_ecrit_rien() {
    let racine = compagnie("suspension-refus");
    let mut conges_payes = conge_parental("2026-07-20", "2026-07-25");
    conges_payes["motif"] = json!("200");
    let mut inconnu = conge_parental("2026-07-20", "2026-07-25");
    inconnu["motif"] = json!("999");
    for (nom, charge) in [
        ("conges payes", conges_payes),
        ("motif inconnu", inconnu),
        ("fin avant le debut", conge_parental("2026-07-20", "2026-07-10")),
        ("aucun contrat ce jour-la", json!({"salarie": "Biancolelli", "debut": "2026-09-01", "fin": "2026-09-05", "motif": "501"})),
        ("au-dela du contrat", json!({"salarie": "Biancolelli", "debut": "2026-07-20", "fin": "2026-07-30", "motif": "501"})),
        ("salarie inconnu", json!({"salarie": "Personne", "debut": "2026-07-20", "fin": "2026-07-25", "motif": "632"})),
    ] {
        assert!(!refuse(&route(&racine, "suspension-enregistrer", charge)).is_empty(), "{}", nom);
    }
    assert!(!racine.join("imprevus").exists(), "un refus ne laisse aucun fichier");
}

/// Le maintien de salaire se saisit SUR L'ARRET, mois par mois, et seulement pour un contrat paye au mois : L1226-1
/// « ne s'appliquent pas [...] aux salaries intermittents » (`atelier/reference/sources/code-du-travail/articles-20261015.md`).
/// Les jours deja indemnises (D1226-4) ne s'attendent que pour une maladie ou un accident.
#[test]
fn le_maintien_d_un_arret_se_saisit_sur_l_arret_et_seulement_pour_un_permanent() {
    let racine = compagnie("maintien");
    let arret = json!({"salarie": "Nicolet", "dernier_jour": "2026-07-14", "fin": "2026-08-09", "motif": "01", "subrogation": "02", "jours_deja_indemnises": 4});
    let a = reussi(&route(&racine, "arret-enregistrer", arret)).clone();
    let a = &a["arret"];
    assert_eq!((a["maintien_ouvert"].clone(), a["jours_deja_indemnises_attendus"].clone()), (json!(true), json!(true)), "{}", a);
    assert_eq!(a["mois_du_maintien"], json!(["2026-07", "2026-08"]));
    assert_eq!(a["jours_deja_indemnises"], json!(4));
    let r = route(&racine, "arret-maintien", json!({"salarie": "Nicolet", "dernier_jour": "2026-07-14", "mois": "2026-07", "montant": 300}));
    assert_eq!(reussi(&r)["arret"]["maintien"], json!({"2026-07": 300.0}), "{}", r);
    assert_eq!(lire(&racine, "imprevus/2026-07.json")["imprevus"][0]["maintien"], json!({"2026-07": 300.0}));
    // Un mois que l'arret ne traverse pas, un montant negatif : refuses. Un montant vide retire celui du mois.
    assert!(refuse(&route(&racine, "arret-maintien", json!({"salarie": "Nicolet", "dernier_jour": "2026-07-14", "mois": "2026-09", "montant": 10}))).contains("ne traverse pas"));
    assert!(!refuse(&route(&racine, "arret-maintien", json!({"salarie": "Nicolet", "dernier_jour": "2026-07-14", "mois": "2026-08", "montant": -5}))).is_empty());
    let r = route(&racine, "arret-maintien", json!({"salarie": "Nicolet", "dernier_jour": "2026-07-14", "mois": "2026-07", "montant": ""}));
    assert_eq!(reussi(&r)["arret"]["maintien"], json!({}), "{}", r);

    // Une maternite : le maintien d'un permanent reste ouvert (la convention peut maintenir), pas les jours deja indemnises.
    let maternite = json!({"salarie": "Nicolet", "dernier_jour": "2026-09-14", "fin": "2026-09-30", "motif": "02", "subrogation": "02"});
    let a = reussi(&route(&racine, "arret-enregistrer", maternite.clone())).clone();
    assert_eq!((a["arret"]["maintien_ouvert"].clone(), a["arret"]["jours_deja_indemnises_attendus"].clone()), (json!(true), json!(false)));
    reussi(&route(&racine, "arret-oublier", json!({"salarie": "Nicolet", "dernier_jour": "2026-09-14"})));
    let mut avec_deja = maternite;
    avec_deja["jours_deja_indemnises"] = json!(2);
    assert!(refuse(&route(&racine, "arret-enregistrer", avec_deja)).contains("D1226-4"));

    // Un intermittent : ni maintien, ni jours deja indemnises ; ses cachets dans l'arret sont nommes.
    let b = json!({"salarie": "Biancolelli", "dernier_jour": "2026-07-09", "fin": "2026-07-12", "motif": "01", "subrogation": "02"});
    let a = reussi(&route(&racine, "arret-enregistrer", b)).clone();
    assert_eq!((a["arret"]["maintien_ouvert"].clone(), a["arret"]["mois_du_maintien"].clone()), (json!(false), json!([])));
    let dates: Vec<Value> = a["arret"]["cachets"].as_array().unwrap().iter().flat_map(|c| c["dates"].as_array().unwrap().clone()).collect();
    assert_eq!(dates, vec![json!("2026-07-10"), json!("2026-07-11")], "le 12 n'est pas joue : {}", a);
    let m = refuse(&route(&racine, "arret-maintien", json!({"salarie": "Biancolelli", "dernier_jour": "2026-07-09", "mois": "2026-07", "montant": 50})));
    assert!(m.contains("L1226-1"), "{}", m);
}

/// UNE SEULE LISTE POUR LA PAGE (15/10/2026) : chaque arret que rend `imprevus` porte ses `obligations`, les memes que la
/// route `arrets` (`obligations::de_l_arret`, regles de `logiciel/donnees/obligations-base.json`) ; une suspension n'en
/// porte pas -- le bloc 65 part avec la DSN du mois, rien ne nait a cote. Un accident du travail (06) fait naitre la
/// declaration d'accident en plus du signalement (CSS L. 441-2, R. 441-3 :
/// `atelier/reference/sources/code-de-la-securite-sociale/xml/R441-3.xml`).
#[test]
fn chaque_arret_de_la_liste_porte_ses_obligations_et_une_suspension_aucune() {
    let racine = compagnie("obligations");
    let accident = json!({"salarie": "Biancolelli", "dernier_jour": "2026-07-09", "fin": "2026-07-12", "motif": "06",
        "subrogation": "02", "date_accident": "2026-07-09"});
    reussi(&route(&racine, "arret-enregistrer", accident));
    reussi(&route(&racine, "suspension-enregistrer", conge_parental("2026-07-20", "2026-08-10")));

    let l = reussi(&route(&racine, "imprevus", json!({"mois": "2026-07"}))).clone();
    let liste = l["imprevus"].as_array().unwrap();
    let arret = liste.iter().find(|i| i["genre"] == json!("arret")).expect("l'arret");
    let suspension = liste.iter().find(|i| i["genre"] == json!("suspension")).expect("la suspension");
    let cles: Vec<&str> = arret["obligations"].as_array().unwrap().iter().map(|o| o["cle"].as_str().unwrap()).collect();
    assert_eq!(cles, ["signalement_arret", "declaration_accident"], "{}", arret);
    assert!(suspension.get("obligations").is_none(), "{}", suspension);

    // Les memes que la route `arrets` : un seul calcul.
    let a = reussi(&route(&racine, "arrets", json!({}))).clone();
    assert_eq!(a["arrets"][0]["obligations"], arret["obligations"]);
    // Et par salarie, pour son dossier.
    let s = reussi(&route(&racine, "imprevus", json!({"salarie": "Biancolelli"}))).clone();
    assert_eq!(s["imprevus"][0]["obligations"], arret["obligations"]);
}
