//! UN SIGNALEMENT DE FIN DONT LE DELAI FINIT APRES L'ECHEANCE DE LA DSN DU MOIS PART QUAND MEME. Le signalement de fin de
//! contrat se transmet dans les cinq jours ouvres qui suivent la fin (arrete du 2 juillet 2013, art. 1er, pris pour CSS
//! R. 133-14, II : `atelier/reference/sources/arrete-2013-delai-evenements-dsn/xml/1.xml` ; le « delai legal de
//! transmission du signalement » de la norme DSN, `atelier/reference/norme-dsn/texte/chapitre-2-principes-de-constitution.txt`,
//! § 2.5.3). Une compagnie de cinquante salaries et plus depose sa DSN le 5 : un CDD d'usage rompu le 30/01/2026 (un
//! vendredi) se signale jusqu'au 06/02, la DSN de janvier part le 05/02. Le mois ne se ferme pas, pour l'horloge de la
//! compagnie fictive, tant que ce signalement n'est pas parti.

use serde_json::{json, Value};

use crate::outils::rejeu::{rejouer_dossier, scenario};
use crate::outils::{dossier_de_test, ecrire, lire, reussi, route};

#[test]
fn la_fin_signalee_apres_l_echeance_de_la_dsn_part_le_jour_de_son_delai() {
    let d = dossier_de_test("signalement_apres_echeance", "fin_au_30_janvier");
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    // Les Faisceaux, configures, sans engagement ; la personne que les organismes contactent dotee de sa civilite, et
    // CRAIG entre dans la compagnie a ce contrat.
    let gestes = std::fs::read_to_string(scenario("faisceaux-sans-civilite").join("gestes.jsonl")).unwrap();
    let gestes: Vec<String> = gestes
        .lines()
        .take_while(|l| !l.contains("\"enregistrer-saisie\""))
        .map(|l| l.replacen("\"declarant_dsn\":{", "\"cinquante_salaries_et_plus\":true,\"declarant_dsn\":{\"civilite\":\"01\",", 1))
        .map(|l| if l.contains("\"cle\":\"CRAIG\"") { l.replacen("{\"case\":\"date_anciennete\",\"valeur\":\"2026-02-02\"}", "{\"case\":\"date_anciennete\",\"valeur\":\"2026-01-29\"}", 1) } else { l })
        .collect();
    std::fs::write(sc.join("gestes.jsonl"), gestes.join("\n") + "\n").unwrap();
    let (_, c) = rejouer_dossier(&sc, &d);

    // Cinquante salaries et plus depuis cinq ans (CSS L. 130-1, II), et la fiche le dit : la DSN est due le 5. La surcouche dit ce que le
    // bareme exige a cet effectif (`seuils_effectif` du socle 2026).
    ecrire(&c, "config/effectifs.json", &json!({"annees": (2021..=2025).map(|a| (a.to_string(), json!({"A01": {"valeur": 60}}))).collect::<serde_json::Map<String, Value>>()}));
    let mut bareme = lire(&c, "config/baremes/2026.json");
    let surcouche = json!({
        "versement_mobilite": "aucun",
        "cotisations_dsn": {"049": {"taux": 0.35, "base": "brut_x115"}},
        "rubriques": {"autres_2": {"taux_pat": 0.35, "base": "brut_x115"}},
        "par_statut": {"technicien": {
            "cotisations_dsn": {"049": {"taux": 0.5, "base": "brut_x115"}},
            "rubriques": {"autres_2": {"taux_pat": 0.5, "base": "brut_x115"}},
            "reduction_generale": {"tdelta": 0.3821}
        }},
        "par_nature": {"permanent": {
            "cotisations_dsn": {"049": {"taux": 0.5, "base": "brut_x115", "base78": "brut"}, "128": {"taux": 1.0}},
            "rubriques": {"autres_2": {"taux_pat": 0.5, "base": "brut_x115"}},
            "rubriques_ajoutees": {"autres_4": {"taux_pat": 1.0, "composants": [1.0]}}
        }}
    });
    bareme.as_object_mut().unwrap().extend(surcouche.as_object().unwrap().clone());
    ecrire(&c, "config/baremes/2026.json", &bareme);

    // Un CDD d'usage rompu a l'initiative de la salariee (motif 037), notifie le 29, dernier jour le 30/01.
    reussi(&crate::outils::valider(
        &c,
        "2026-01",
        None,
        &json!({"salarie": "CRAIG", "nature": "cddu_spectacle", "spectacle": "nuits-d-ete",
            "lignes": [{"rubrique": "heure", "quantite": 16, "taux": 18.75, "lieu": null, "dates": ["2026-01-29", "2026-01-30"], "planning": []}],
            "motif_rupture": "037", "preavis": {"type": "90", "debut": null, "fin": null}, "dernier_jour_paye": "2026-01-30", "notification_rupture": "2026-01-29"}),
        "2026-01-05",
    ));
    let horloge = |jour: &str| -> Vec<String> {
        let r = route(&c, "horloge-rattraper", json!({"aujourdhui": jour}), jour);
        reussi(&r).as_array().unwrap().iter().map(|p| p.as_str().unwrap_or_default().to_string()).collect()
    };
    let le_5 = horloge("2026-02-05");
    assert!(le_5.iter().any(|p| p.contains("DSN de janvier 2026 déposée") && !p.contains("non joué")), "{:?}", le_5);
    let le_6 = horloge("2026-02-06");
    assert!(
        le_6.iter().any(|p| p.starts_with("Le 06/02/2026 : signalement de fin du contrat de CRAIG (fin le 30/01/2026) déposé") && !p.contains("non joué")),
        "le 05/02 : {:?}\nle 06/02 : {:?}",
        le_5,
        le_6
    );
}
