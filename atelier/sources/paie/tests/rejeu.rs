//! LE REJEU DES SCENARIOS DU BANC D'ESSAI (#236, phase 2). Un scenario enregistre dans l'interface (`atelier/outils/banc/`,
//! `paie/src/scenario.rs`) se rejoue sans navigateur et reproduit la compagnie, octet pour octet d'un rejeu a l'autre ;
//! ses sorties (paies, DSN, DPAE, bulletins, certificats Conges Spectacles, journaux) se comparent a un oracle.
//!
//! SEUL DANS `tests/` : le rejeu simule le temps du moteur, qui est celui du processus (`paie/src/temps.rs`), comme un
//! fichier qui pose une variable d'environnement.
//!
//! Chaque fictive de la phase 4 aura son test ici, sur ce modele : `outils::rejeu::rejouer_et_comparer("<nom>", ...)`.

#[path = "outils/mod.rs"]
mod outils;

use serde_json::{json, Value};

/// LES TRETEAUX DU BANC : le petit scenario de la preuve, joue au banc le 23/10/2026. Une compagnie ajoutee par le « + »
/// d'un logiciel bride, configuree juste assez pour payer (convention 3090), une comedienne, deux cachets les 20 et
/// 21/11/2026 ; puis le geste d'atelier qui rend ses envois simules, et le calendrier avance au 20/11, au 01/12, au 16/12 :
/// l'horloge de la fictive ecrit le mois, depose la DPAE (test puis reel), la DSN de novembre et son signalement
/// d'amorcage (test puis reel, chacun sur le bilan de son test), envoie les documents.
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
    // Deux cachets de 150 € : 300 € de brut.
    let paie = &releve["paies"]["2026-11"]["paies"][0];
    assert_eq!(paie["brut"], json!("300"), "{}", paie);

    // UN SECOND REJEU, ailleurs, rend exactement le meme releve : rien n'y depend du hasard ni de l'heure de la machine.
    let (r2, racine2) = outils::rejeu::rejouer("treteaux-du-banc", &outils::dossier_de_test("rejeu", "treteaux-bis"));
    let jour = paie::paris::iso_secondes(r2.fin.unwrap())[..10].to_string();
    let second: Value = outils::rejeu::sorties(&racine2, &jour);
    assert!(second == releve, "deux rejeux du meme scenario ne rendent pas les memes sorties");
}
