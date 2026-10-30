//! UN SALARIE COCHE EN PLUS SUR LA CARTE OUVERTE D'UN ENGAGEMENT VALIDE (un bug qui a echappe : seule la premiere
//! saisie de la carte partait, et le salarie coche en plus disparaissait sans un mot). « Modifier l'engagement » envoie
//! toute la carte : l'engagement qu'elle designe, et le salarie coche en plus, sans engagement a designer ; le moteur
//! l'ajoute au mois et le valide avec l'autre.

use serde_json::{json, Value};

use crate::outils::rejeu::{rejouer_dossier, scenario};
use crate::outils::{dossier_de_test, reussi, route, saisie};

#[test]
fn le_salarie_coche_en_plus_part_avec_la_carte() {
    let d = dossier_de_test("carte_ouverte", "salarie_en_plus");
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    let gestes = std::fs::read_to_string(scenario("treteaux-du-banc").join("gestes.jsonl")).unwrap();
    let debut: Vec<&str> = gestes.lines().take_while(|l| !l.contains("\"geste\":\"jour\"")).collect();
    std::fs::write(sc.join("gestes.jsonl"), debut.join("\n") + "\n").unwrap();
    let (_, c) = rejouer_dossier(&sc, &d);
    let jour = "2026-11-02";

    reussi(&route(&c, "enregistrer-fiche", json!({"nom": "salarie", "cle": null, "donnees": {
        "prenom_affichage": "Isadora", "nom_affichage": "Duncan", "nir": "2820144999013", "nir_cle": "35",
        "date_naissance": "1982-01-15", "lieu_naissance_affichage": "Nantes", "departement_naissance": "44",
        "pays_naissance": "FR", "code_nationalite_dsn": "01", "adresse_affichage": "7 rue de la Paix", "code_postal": "44000",
        "commune_affichage": "Nantes", "email": "isadora.duncan@exemple.invalid", "emploi_affichage": "Comédienne",
        "statut": "artiste", "statut_categoriel_rc": "04", "pcs_ese": "354c", "code_emploi_ft": "COM010",
        "statut_conventionnel": "05", "date_anciennete": "2026-11-02", "actif": true}}), jour));

    let s = saisie(&c, "2026-11");
    let fuller: Value = s["engagements"].as_array().unwrap().iter().find(|e| e["salarie"] == "FULLER").cloned().unwrap();
    let debut = engagement::debut_brut(&fuller);
    let v = route(&c, "valider-engagement", json!({"mois": "2026-11", "salarie": "FULLER", "debut": debut, "numero": "", "rang": 0, "valide": true}), jour);
    assert!(reussi(&v)["valide"].is_string(), "{}", v);

    // La carte ouverte, DUNCAN coche en plus : chaque salarie recoit sa copie de ce que la carte dit.
    let mut duncan = fuller.clone();
    duncan["salarie"] = json!("DUNCAN");
    duncan.as_object_mut().unwrap().remove("numero");
    let r = route(&c, "modifier-engagement", json!({"mois": "2026-11", "confirme": true, "engagements": [
        {"salarie": "FULLER", "debut": debut, "numero": "", "rang": 0, "saisie": fuller},
        {"saisie": duncan}]}), jour);
    assert_eq!(reussi(&r)["change"], json!(true), "{}", r);

    let e = crate::outils::lire(&c, "engagements/2026-11.json");
    let valides: Vec<&str> = e["engagements"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|x| x["valide"].is_object())
        .filter_map(|x| x["salarie"].as_str())
        .collect();
    assert_eq!(valides, ["FULLER", "DUNCAN"], "{}", e);
}
