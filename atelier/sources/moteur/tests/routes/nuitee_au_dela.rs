//! LA NUITÉE AU-DELÀ DE LA LIMITE DU JOUR. BOSS, Frais professionnels
//! (`atelier/reference/sources/boss/boss-frais-professionnels-20261013.txt`) : § 1290 (l. 1535-1546), le logement et le
//! petit-déjeuner en grand déplacement sont hors de l'assiette dans la limite de 76,60 € par jour à Paris et dans les
//! Hauts-de-Seine, la Seine-Saint-Denis et le Val-de-Marne, 56,80 € ailleurs (au 1er janvier 2026) ; § 270 (l. 332-333),
//! « la fraction excédentaire est exclue de l'assiette des cotisations si l'employeur prouve qu'elle a été utilisée
//! conformément à son objet ». Arrêté du 4 septembre 2025, annexe 2, I, 2° (BOSS § 2250, l. 2808-2819) : les
//! défraiements des artistes dramatiques en tournée restent hors de l'assiette. Les Tréteaux du Banc (3090, nuitée de la
//! convention à 70,28 €), novembre 2026 : un technicien à Nantes (13,48 € par nuit au-delà, soumis), le même avec le
//! justificatif conservé, le même à Paris, et une comédienne à Nantes.

use serde_json::{json, Value};

use crate::outils::rejeu::{rejouer_dossier, scenario};
use crate::outils::{dec, dossier_de_test, reussi, route};

#[test]
fn la_nuitee_d_un_technicien_au_dela_de_la_limite_entre_dans_l_assiette() {
    let d = dossier_de_test("nuitee_au_dela", "treteaux");
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    let gestes = std::fs::read_to_string(scenario("treteaux-du-banc").join("gestes.jsonl")).unwrap();
    let debut: Vec<&str> = gestes.lines().take_while(|l| !l.contains("\"route\":\"apercu\"")).collect();
    std::fs::write(sc.join("gestes.jsonl"), debut.join("\n") + "\n").unwrap();
    let (_, c) = rejouer_dossier(&sc, &d);
    let technicien = reussi(&route(
        &c,
        "enregistrer-fiche",
        json!({"nom": "salarie", "cle": null, "donnees": {"prenom_affichage": "Adolphe", "nom_affichage": "Appia",
            "nir": "1620144999003", "nir_cle": "08", "date_naissance": "1962-01-15", "lieu_naissance_affichage": "Nantes",
            "departement_naissance": "44", "pays_naissance": "FR", "code_nationalite_dsn": "01", "adresse_affichage": "1 rue Kervégan",
            "code_postal": "44000", "commune_affichage": "Nantes", "statut": "technicien", "statut_categoriel_rc": "04",
            "pcs_ese": "637c", "code_emploi_ft": "TEC250", "statut_conventionnel": "07", "date_anciennete": "2026-11-02", "actif": true},
            "forcer": true}),
        "2026-11-02",
    ))["cle"]
        .as_str()
        .unwrap()
        .to_string();

    let paie = |salarie: &str, lieu: &str, justificatif: bool| -> Value {
        let mut nuitee = json!({"rubrique": "nuitee", "quantite": 2, "taux": "", "dates": ["2026-11-20", "2026-11-21"], "lieu": lieu});
        if justificatif {
            nuitee["justificatif"] = json!(true);
        }
        let saisie = json!({"salarie": salarie, "spectacle": "le-banc-des-menteurs", "lignes": [
            {"rubrique": "heure", "quantite": 16, "taux": 18.75, "dates": ["2026-11-20", "2026-11-21"], "lieu": lieu}, nuitee]});
        reussi(&route(&c, "apercu", json!({"mois": "2026-11", "cartes": [{"saisie": saisie}]}), "2026-11-02"))["paies"][0].clone()
    };
    let ligne = |p: &Value, code: &str| p["remuneration"].as_array().and_then(|l| l.iter().find(|x| x["rubrique"] == code)).cloned();
    let n = |v: &Value| dec(v.as_str().unwrap());

    // Hors Paris : 56,80 hors de l'assiette, 13,48 par nuit soumis ; la case « justificatif conservé » a lieu d'être.
    let nantes = paie(&technicien, "Nantes", false);
    assert_eq!(n(&ligne(&nantes, "nuitee").unwrap()["taux"]), dec("56.8"));
    assert_eq!(n(&ligne(&nantes, "nuitee_au_dela").expect("la part au-delà de la limite")["montant"]), dec("26.96"));
    assert_eq!(nantes["justificatifs"], json!([1]));
    // Le justificatif conservé : la nuitée entière hors de l'assiette, 26,96 de brut en moins.
    let justifiee = paie(&technicien, "Nantes", true);
    assert!(ligne(&justifiee, "nuitee_au_dela").is_none());
    assert_eq!(n(&nantes["brut"]) - n(&justifiee["brut"]), dec("26.96"));
    // À Paris, 70,28 est sous 76,60 : rien au-delà, pas de case.
    let paris = paie(&technicien, "Théâtre de la Ville, Paris", false);
    assert!(ligne(&paris, "nuitee_au_dela").is_none());
    assert_eq!(paris["justificatifs"], json!([]));
    // Une comédienne en tournée (annexe 2, I, 2°) : la nuitée entière hors de l'assiette.
    let artiste = paie("FULLER", "Nantes", false);
    assert!(ligne(&artiste, "nuitee_au_dela").is_none());
    assert_eq!(n(&ligne(&artiste, "nuitee").unwrap()["taux"]), dec("70.28"));
}
