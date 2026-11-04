//! UN SALARIE DECLARE SANS NIR, SOUS SON NTT, DONT LE NIR ARRIVE ENSUITE. La fiche du Fil de Fer de janvier 2026, avec
//! un technicien sans NIR (SANSNIR) engage et declare en janvier (DPAE, DSN parties) ; son NIR arrive le 05/02.

use serde_json::json;

use crate::outils::rejeu::{reference, rejouer_dossier};
use crate::outils::{copier, dossier_de_test, reussi, route};

/// La premiere valeur d'une case vide vaut de tout temps, meme apres un mois parti (sa DPAE de decembre la lisait vide
/// quand elle se datait au mois suivant) ; et le NTT sous lequel il a ete declare accompagne son NIR dans la DSN suivante,
/// un signalement compris (« Le NTT doit etre renseigne sur la premiere DSN ou le NIR est attribue. Ceci permettra de
/// faire le lien entre les deux identites declarees par l'employeur » : `norme-dsn/texte/bloc-S21.G00.30.txt`, l. 353-354).
#[test]
fn le_nir_qui_arrive_vaut_de_tout_temps_et_le_signalement_porte_le_ntt_declare() {
    let d = dossier_de_test("identite_qui_arrive", "nir");
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    let fil = reference("fictive-fil-de-fer");
    copier(&fil.join("papiers-2025"), &sc.join("papiers-2025"));
    std::fs::copy(fil.join("description.json"), sc.join("description.json")).unwrap();
    let gestes = std::fs::read_to_string(fil.join("gestes.jsonl")).unwrap();
    let gestes: Vec<&str> = gestes.lines().collect();
    let fiche = json!({"route": "enregistrer-fiche", "charge": {"nom": "salarie", "confirme": true, "donnees": {
        "nom_affichage": "Sansnir", "prenom_affichage": "Armel", "sexe": "1", "emploi_affichage": "Technicien son",
        "adresse_affichage": "3 rue Gloriette", "code_postal": "71100", "commune_affichage": "Chalon-sur-Saône",
        "statut": "technicien", "pcs_ese": "465b", "code_emploi_ft": "TEC250", "statut_conventionnel": "07",
        "statut_categoriel_rc": "04", "date_naissance": "1990-05-04", "lieu_naissance_affichage": "Chalon-sur-Saône",
        "departement_naissance": "71", "pays_naissance": "FR", "code_nationalite_dsn": "01"}}});
    let janvier = json!({"route": "valider-engagement", "charge": {"mois": "2026-01", "saisie": {"salarie": "SANSNIR",
        "spectacle": "objet-256Z00931108", "lignes": [{"rubrique": "heure", "quantite": 10, "taux": 23,
        "lieu": "Atelier de la compagnie, 71100 Chalon-sur-Saône", "dates": ["2026-01-19", "2026-01-20"]}]}}});
    // Jusqu'a la DSN de janvier partie (la premiere `envoyer-documents`).
    let fin = gestes.iter().position(|l| l.contains("\"envoyer-documents\"")).unwrap();
    let debut = gestes.iter().position(|l| l.contains("\"valider-engagement\"")).unwrap();
    let mut joues: Vec<String> = gestes[..debut].iter().map(|l| l.to_string()).collect();
    joues.push(fiche.to_string());
    joues.extend(gestes[debut..debut + 2].iter().map(|l| l.to_string()));
    joues.push(janvier.to_string());
    joues.extend(gestes[debut + 2..=fin].iter().map(|l| l.to_string()));
    std::fs::write(sc.join("gestes.jsonl"), joues.join("\n") + "\n").unwrap();
    let (_, c) = rejouer_dossier(&sc, &d);
    let jour = "2026-02-05";

    // Le NIR arrive : une case vide qu'on remplit, sans date ni avertissement, apres la DSN partie de janvier.
    let r = reussi(&route(&c, "enregistrer-fiche", json!({"nom": "salarie", "cle": "SANSNIR",
        "donnees": {"nir": "1900571123456", "nir_cle": "33"}}), jour)).clone();
    assert!(r.get("avertissement").is_none(), "la premiere valeur d'une case se date : {}", r);
    let fiches: serde_json::Value = serde_json::from_slice(&std::fs::read(c.join("config/salaries.json")).unwrap()).unwrap();
    assert_eq!(fiches["SANSNIR"]["nir"], json!([{"valeur": "1900571123456"}]), "le NIR vaut de tout temps");

    reussi(&route(&c, "valider-engagement", json!({"mois": "2026-02", "saisie": {"salarie": "SANSNIR",
        "spectacle": "objet-256Z00931107", "lignes": [{"rubrique": "heure", "quantite": 15, "taux": 23,
        "lieu": "Salle Jean-Moulin, 39100 Dole", "dates": ["2026-02-06", "2026-02-07", "2026-02-08"]}]}}), jour));
    reussi(&route(&c, "arret-enregistrer", json!({"salarie": "SANSNIR", "dernier_jour": "2026-02-06", "debut": "2026-02-07", "fin": "2026-02-08",
        "motif": "01"}), jour));
    let f = reussi(&route(&c, "arret-signaler", json!({"salarie": "SANSNIR", "dernier_jour": "2026-02-06"}), jour))["fichier"]
        .as_str()
        .unwrap()
        .to_string();
    let texte = std::fs::read_to_string(&f).unwrap();
    assert!(texte.contains("S21.G00.30.001,'1900571123456'"), "le signalement porte le NIR :\n{}", texte);
    assert!(texte.contains("S21.G00.30.020,'1"), "le signalement porte le NTT declare en janvier a cote du NIR :\n{}", texte);
    // Son apercu se range au mois de l'evenement, et dit son jour.
    let a = reussi(&route(&c, "depot-apercu", json!({"quoi": "dsn", "fichier": f}), jour)).clone();
    assert_eq!((&a["mois"], &a["evenement"]), (&json!("2026-02"), &json!("2026-02-06")), "{}", a);

    // LE SIGNALEMENT PARTI (son depot de test, rejoue en reel au journal), l'arret prolonge : il ne se refait pas, et le
    // fichier reste tel qu'il est parti (regle 4 de `atelier/AGENTS.md` ; « en cas de prolongation d'arret, il ne faut
    // pas envoyer de nouveau signalement », `net-entreprises-fiche1983-prolongation-arret-dates-de-subrogation-20250322.txt`).
    reussi(&route(&c, "deposer", json!({"quoi": "dsn", "fichier": f, "reel": false}), jour));
    let journal = c.join("journal").join("depots.jsonl");
    let lignes = std::fs::read_to_string(&journal).unwrap();
    let mut reel: serde_json::Value = serde_json::from_str(lignes.lines().rev().find(|l| l.contains("\"ligne\"")).unwrap()).unwrap();
    reel["ligne"]["mode"] = json!("reel");
    reel["id"] = json!("reel-du-test");
    std::fs::write(&journal, format!("{}{}\n", lignes, reel)).unwrap();
    let parti = std::fs::read(&f).unwrap();
    reussi(&route(&c, "arret-prolonger", json!({"salarie": "SANSNIR", "dernier_jour": "2026-02-06", "fin": "2026-02-10"}), jour));
    let r = route(&c, "arret-signaler", json!({"salarie": "SANSNIR", "dernier_jour": "2026-02-06"}), jour);
    assert_eq!(r["ok"], json!(false), "{}", r);
    assert_eq!(std::fs::read(&f).unwrap(), parti, "le signalement parti a ete reecrit");
    // Effacé, il se refait tel qu'il est parti : d'après l'arrêt d'avant la prolongation.
    std::fs::remove_file(&f).unwrap();
    reussi(&route(&c, "piece-partie", json!({"mois": "2026-02", "fichier": std::path::Path::new(&f).file_name().unwrap().to_str().unwrap()}), jour));
    assert_eq!(std::fs::read(&f).unwrap(), parti, "le signalement parti ne se refait pas tel qu'il est parti");
}
