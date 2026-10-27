//! LE REJEU DES SCENARIOS DU BANC D'ESSAI (#236, phase 2). Un scenario enregistre dans l'interface (`atelier/outils/banc/`,
//! `paie/src/scenario.rs`) se rejoue sans navigateur et reproduit la compagnie, octet pour octet d'un rejeu a l'autre ;
//! ses sorties (paies, DSN, DPAE, bulletins, certificats Conges Spectacles, journaux) se comparent a un oracle.
//!
//! SEUL DANS `tests/` : le rejeu simule le temps du moteur, qui est celui du processus (`socle/src/temps.rs`), comme un
//! fichier qui pose une variable d'environnement.
//!
//! Chaque fictive de la phase 4 aura son test ici, sur ce modele : `outils::rejeu::rejouer_et_comparer("<nom>", ...)`.

#[path = "outils/mod.rs"]
mod outils;

use serde_json::{json, Value};

/// LES TRETEAUX DU BANC : le petit scenario de la preuve, joue au banc le 23/10/2026. Une compagnie ajoutee par le « + »
/// d'un logiciel bride, configuree juste assez pour payer (convention 3090), une comedienne, deux cachets les 20 et
/// 21/11/2026 ; puis le geste d'atelier qui rend ses envois simules, et le calendrier avance au 20/11, au 01/12, au 16/12 :
/// l'horloge de la fictive valide l'engagement, depose la DPAE (test puis reel), fabrique et depose la DSN de novembre et
/// son signalement d'amorcage (test puis reel, chacun sur le bilan de son test), envoie les documents.
#[test]
fn les_treteaux_du_banc_se_rejouent_et_rendent_les_memes_sorties() {
    let releve = outils::rejeu::rejouer_et_comparer("treteaux-du-banc", &outils::dossier_de_test("rejeu", "treteaux"));
    let f = &releve["fichiers"];

    // Le contrat de la vie d'une fictive : la DPAE et la DSN du mois partent en reel (jouees), chacune sur le bilan de son
    // test (`depot_acte::exiger_repetition`, tickets #221 et #222) ; le mois a ses bulletins et ses certificats CS.
    let depots = f["journal/depots.json"].as_array().expect("le journal des depots");
    let reel = |quoi: &str, fichier: &str| {
        depots.iter().any(|l| l["quoi"] == json!(quoi) && l["mode"] == json!("reel") && l["accepte"] == json!(true) && l["fichier"] == json!(fichier))
    };
    assert!(reel("dsn", "DSN_Novembre2026.dsn"), "la DSN de novembre doit partir en reel");
    assert!(reel("dsn", "Amorcage01_FULLER_20261120.dsn"), "le signalement d'amorcage doit partir en reel");
    assert!(depots.iter().any(|l| l["quoi"] == json!("dpae") && l["mode"] == json!("reel") && l["accepte"] == json!(true)), "la DPAE doit partir en reel");
    for doc in ["sorties/2026-11/envoi/Bulletin_FULLER_Novembre2026.pdf", "sorties/2026-11/envoi/CertificatCS_FULLER_Novembre2026.pdf"] {
        assert!(f.get(doc).is_some(), "{} manque", doc);
    }
    // Ajoutee le 02/11/2026, la compagnie ne coche aucune echeance de 2026 fermee avant (#236, ticket 8 du banc).
    let cochees = f["config/echeancier.json"]["fait"].as_object().map(|m| m.keys().cloned().collect::<Vec<_>>()).unwrap_or_default();
    assert!(cochees.is_empty(), "echeances cochees : {:?}", cochees);
    // L'AER CINEMA SPECTACLE (`aer::due`, `aer_fictive`) : la DSN mensuelle de novembre, qui porte la fin du contrat de la
    // comedienne, recoit son retour 46 et l'AER signee part avec ses documents ; le signalement d'amorcage (nature 08) n'en
    // recoit aucun -- France Travail n'edite l'AER que depuis la mensuelle.
    let natures = |fichier: &str| -> Vec<Value> {
        depots
            .iter()
            .filter(|l| l["quoi"] == json!("dsn") && l["mode"] == json!("reel") && l["fichier"] == json!(fichier))
            .flat_map(|l| l["retours"].as_array().cloned().unwrap_or_default())
            .map(|r| r["nature"].clone())
            .collect()
    };
    assert!(natures("DSN_Novembre2026.dsn").contains(&json!("46")), "{:?}", natures("DSN_Novembre2026.dsn"));
    assert!(!natures("Amorcage01_FULLER_20261120.dsn").contains(&json!("46")), "{:?}", natures("Amorcage01_FULLER_20261120.dsn"));
    assert!(f.get("sorties/2026-11/envoi/AER_FULLER_Novembre2026.pdf").is_some(), "l'AER de novembre manque");
    // Deux cachets de 150 € : 300 € de brut.
    let paie = &releve["paies"]["2026-11"]["paies"][0];
    assert_eq!(paie["brut"], json!("300"), "{}", paie);

    // UN SECOND REJEU, ailleurs, rend exactement le meme releve : rien n'y depend du hasard ni de l'heure de la machine.
    let (r2, racine2) = outils::rejeu::rejouer("treteaux-du-banc", &outils::dossier_de_test("rejeu", "treteaux-bis"));
    let jour = socle::paris::iso_secondes(r2.fin.unwrap())[..10].to_string();
    let second: Value = outils::rejeu::sorties(&racine2, &jour);
    assert!(second == releve, "deux rejeux du meme scenario ne rendent pas les memes sorties");
}

/// LE SCENARIO FIGE SANS SES LECTURES (`scenario::elaguer`, phase 4) : les routes qui n'ont rien ecrit s'otent, et le rejeu
/// rend exactement le meme releve que le scenario entier.
#[test]
fn un_scenario_elague_se_rejoue_a_l_identique() {
    let d = outils::dossier_de_test("rejeu", "elague");
    let (gardes, otees) = paie::scenario::elaguer(&outils::rejeu::scenario("treteaux-du-banc"), &d.join("scenario")).unwrap();
    assert!(otees > 0 && gardes > 0, "{} gardes, {} otees", gardes, otees);
    let texte = std::fs::read_to_string(d.join("scenario").join(paie::scenario::GESTES)).unwrap();
    assert!(texte.lines().all(|l| !l.contains("\"ecrit\":false")), "une lecture est restee");
    let (r, racine) = outils::rejeu::rejouer_dossier(&d.join("scenario"), &d.join("rejeu"));
    let jour = socle::paris::iso_secondes(r.fin.unwrap())[..10].to_string();
    assert!(outils::rejeu::sorties(&racine, &jour) == outils::lire_oracle("scenarios/treteaux-du-banc/sorties.json"));
}

/// #236, ticket 9 du banc : la demo Les Faisceaux telle qu'elle etait ecrite, sans la civilite du contact DSN. La DSN
/// de janvier ne se fabrique pas (le moteur la refuse) ; l'horloge n'envoie donc pas ses documents aux salaries -- elle
/// les envoie apres la DSN du mois, jamais avant.
#[test]
fn les_documents_d_un_mois_qui_n_est_pas_parti_ne_partent_pas() {
    let d = outils::dossier_de_test("rejeu", "faisceaux-sans-civilite");
    let (r, racine) = outils::rejeu::rejouer("faisceaux-sans-civilite", &d);
    assert!(r.horloge.iter().any(|p| p.contains("DSN de janvier 2026 déposée") && p.contains("non joué")), "{:?}", r.horloge);
    let depots: Value = serde_json::from_str(&std::fs::read_to_string(racine.join("journal/depots.json")).unwrap()).unwrap();
    let documents = depots.as_array().unwrap().iter().filter(|l| l["quoi"] == json!("documents")).count();
    assert_eq!(documents, 0, "des documents sont partis pour un mois sans DSN : {}", depots);
    assert!(!r.horloge.iter().any(|p| p.contains("documents")), "{:?}", r.horloge);
}
