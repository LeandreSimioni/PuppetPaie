//! LE REJEU DES SCENARIOS DU BANC D'ESSAI. Un scenario enregistre dans l'interface (`atelier/outils/banc/`,
//! `moteur/src/atelier/scenario.rs`) se rejoue sans navigateur et reproduit la compagnie, octet pour octet d'un rejeu a l'autre ;
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
    // Ajoutee le 02/11/2026, la compagnie ne coche aucune echeance de 2026 fermee avant.
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

/// #271 : un engagement dont un envoi reel accepte est parti (la DPAE, la DSN de novembre des Treteaux) ne se supprime
/// plus. La route refuse et rien n'est ecrit : l'engagement reste tel qu'il etait.
#[test]
fn un_engagement_dont_un_envoi_est_parti_ne_se_supprime_pas() {
    let (_, racine) = outils::rejeu::rejouer("treteaux-du-banc", &outils::dossier_de_test("rejeu", "parti"));
    let avant = std::fs::read(racine.join("engagements/2026-11.json")).unwrap();
    let charge = json!({"mois": "2026-11", "salarie": "FULLER", "debut": "2026-11-20", "numero": "FU1.26.01"});
    outils::refus(&outils::route(&racine, "retirer-engagement", charge, "2026-12-16"));
    assert!(std::fs::read(racine.join("engagements/2026-11.json")).unwrap() == avant, "le refus a ecrit l'engagement");
}

/// #271 : l'engagement des Treteaux, sa DPAE et sa DSN de novembre parties, devient non actif apres la veille de
/// l'echeance de novembre. Ressort la DSN de novembre a corriger ; la DSN de decembre porte la
/// correction qui annule tout ce qui avait ete declare pour lui, et rien a la place (cahier technique DSN 2027.1,
/// `atelier/reference/norme-dsn/texte-2027/chapitre-2-principes-de-constitution.txt`, §2.1.2.1 et §2.1.3.6). Partie,
/// elle ne se porte plus.
///
/// La ligne DSN de decembre : sans engagement valide dans le mois ni correction a porter, rien n'est a declarer (#257 :
/// « à déposer dès qu'il y a des engagements validés dans le mois ») ; la correction de novembre a porter la rend a
/// deposer (§2.1.2.1 : une correction « donne lieu à la déclaration d'une correction dans la DSN suivante »).
#[test]
fn un_engagement_non_actif_fait_annuler_ce_qui_a_ete_declare_pour_lui() {
    use engagement::etats::ARefaire;
    let (_, racine) = outils::rejeu::rejouer("treteaux-du-banc", &outils::dossier_de_test("rejeu", "non-actif"));
    let dsn_de_decembre = || -> Vec<String> {
        let r = outils::route(&racine, "etat-du-mois", json!({"mois": "2026-12"}), "2026-12-16");
        let etapes = outils::reussi(&r)["etapes"].as_array().unwrap().clone();
        let dsn = etapes.iter().find(|e| e["cle"] == json!("dsn")).expect("la ligne DSN").clone();
        dsn["groupes"].as_array().unwrap().iter().map(|g| g["etat"].as_str().unwrap().to_string()).collect()
    };
    assert_eq!(dsn_de_decembre(), ["fait"], "un mois sans rien a declarer");
    let charge = json!({"mois": "2026-11", "salarie": "FULLER", "debut": "2026-11-20", "numero": "FU1.26.01", "non_actif": true});
    outils::reussi(&outils::route(&racine, "retirer-engagement", charge, "2026-12-16"));
    assert_eq!(dsn_de_decembre(), ["a_faire"], "la DSN de decembre porte la correction de novembre");

    let ctx = |jour: &str| droit::contexte::Contexte::nouveau(&racine, Some(socle::date::Jour::lire(jour).unwrap())).unwrap();
    let na = engagement::etats::non_actifs_du_mois(&ctx("2026-12-16"), "2026-11").unwrap().pop().expect("l'engagement non actif");
    let a_refaire = na.a_refaire(&ctx("2026-12-16"), &envois::journal::departs(&racine).unwrap());
    assert!(a_refaire.iter().any(|a| matches!(a, ARefaire::Dsn { mois, .. } if mois == "2026-11")), "{:?}", a_refaire);

    // La DSN de decembre : le versement de la correction annule celui de novembre, montant pour montant.
    outils::reussi(&outils::route(&racine, "dsn-a-deposer", json!({"mois": "2026-12"}), "2026-12-16"));
    let lire = |rel: &str| std::fs::read(racine.join(rel)).unwrap().iter().map(|&o| o as char).collect::<String>();
    let (novembre, decembre) = (lire("sorties/2026-11/DSN_Novembre2026.dsn"), lire("sorties/2026-12/DSN_Decembre2026.dsn"));
    let montants = |dsn: &str, rubriques: &[&str]| -> Vec<rust_decimal::Decimal> {
        outils::individu(dsn, "FULLER").into_iter().filter(|(k, _)| rubriques.contains(&k.as_str())).map(|(_, v)| outils::dec(&v)).collect()
    };
    let annules = |r: &[&str]| montants(&novembre, r).into_iter().map(|m| -m).collect::<Vec<_>>();
    for r in [&["S21.G00.50.002", "S21.G00.50.004"][..], &["S21.G00.78.004"], &["S21.G00.81.004"]] {
        assert!(!annules(r).is_empty(), "{:?}", r);
        assert_eq!(montants(&decembre, r), annules(r), "{:?}", r);
    }
    let brut = |dsn: &str| montants(dsn, &["S21.G00.51.013"]).into_iter().filter(|m| !m.is_zero()).collect::<Vec<_>>();
    assert_eq!(brut(&decembre), brut(&novembre).into_iter().map(|m| -m).collect::<Vec<_>>());

    // Partie sur la version du non actif, la correction ne ressort plus et ne se porte plus en janvier.
    let mut journal = outils::lire(&racine, "journal/depots.json");
    journal.as_array_mut().unwrap().push(json!({
        "quoi": "dsn", "quand": "2027-01-05T10:00:00", "mois": "2026-12", "fichier": "DSN_Decembre2026.dsn", "mode": "reel", "accepte": true,
        "versions": [{"salarie": "FULLER", "numero": "FU1.26.01", "version": na.version(), "mois": "2026-11"}],
    }));
    outils::ecrire(&racine, "journal/depots.json", &journal);
    let a_refaire = na.a_refaire(&ctx("2027-01-20"), &envois::journal::departs(&racine).unwrap());
    assert!(!a_refaire.iter().any(|a| matches!(a, ARefaire::Dsn { .. })), "{:?}", a_refaire);
    assert!(declarations::produire_regularisations::regularisations_pour(&ctx("2027-01-20"), "2027-01").unwrap().is_empty());
}

/// UN ENGAGEMENT AJOUTE A UN MOIS DEJA DECLARE A UN SEUL BULLETIN ET UN SEUL PAIEMENT. Aux Treteaux, apres la veille de
/// l'echeance de novembre (DSN et documents de novembre partis), un second contrat de la comedienne est ajoute au
/// 27/11. Son bulletin est un bulletin ordinaire de novembre, que la case Documents de novembre propose ; la DSN de
/// decembre le declare (cahier technique DSN 2026.1, § 2.1.2.1 : « une correction est un evenement connu apres la date
/// de cloture de la paie. Il donne lieu a la declaration d'une correction dans la DSN suivante »,
/// `atelier/reference/norme-dsn/texte/chapitre-2-principes-de-constitution.txt`) sans fabriquer de bulletin. Son
/// bulletin parti puis l'engagement change : un seul rectificatif, celui des documents de novembre.
#[test]
fn un_engagement_ajoute_a_un_mois_declare_a_un_seul_bulletin() {
    let (_, racine) = outils::rejeu::rejouer("treteaux-du-banc", &outils::dossier_de_test("rejeu", "ajoute"));
    let jour = "2026-12-16";
    let lignes = |q: u32, dates: &[&str]| json!([{"rubrique": "cachet", "quantite": q, "taux": 150, "lieu": "Salle fictive de Rezé", "dates": dates}]);
    let saisie = |second: Value| {
        json!({"mois": "2026-11", "saisie": {"mois": "2026-11", "taux_cachet": 150, "date_paiement": "2026-11-30", "engagements": [
            {"salarie": "FULLER", "spectacle": "le-banc-des-menteurs", "numero": "FU1.26.01", "lignes": lignes(2, &["2026-11-20", "2026-11-21"])},
            second,
        ]}, "forcer": false, "modifier_apres_echeance": true})
    };
    let second = json!({"mois": "2026-11", "salarie": "FULLER", "debut": "2026-11-27"});
    let avec = |x: Value| {
        let mut f = second.clone();
        f.as_object_mut().unwrap().extend(x.as_object().unwrap().clone());
        f
    };
    let documents = || -> Vec<String> {
        let r = outils::route(&racine, "etat-du-mois", json!({"mois": "2026-11"}), jour);
        let etapes = outils::reussi(&r)["etapes"].as_array().unwrap().clone();
        let d = etapes.iter().find(|e| e["cle"] == json!("documents")).expect("la ligne Documents").clone();
        d["groupes"].as_array().unwrap().iter().map(|g| g["etat"].as_str().unwrap().to_string()).collect()
    };
    let bulletins_hors_du_mois = || -> Vec<String> {
        let mut out = Vec::new();
        for d in ["sorties/2026-12", "sorties/2026-12/envoi"] {
            for f in std::fs::read_dir(racine.join(d)).into_iter().flatten().flatten() {
                let n = f.file_name().to_string_lossy().into_owned();
                if n.ends_with(".pdf") {
                    out.push(n);
                }
            }
        }
        out
    };
    let paie_du_second = || -> Value {
        let e = outils::lire(&racine, "engagements/2026-11.json");
        let e = e["engagements"].as_array().unwrap().iter().find(|x| x["numero"] == json!("FU1.26.02")).expect("le second contrat").clone();
        e["valide"]["paies"]["2026-11"].clone()
    };

    outils::reussi(&outils::route(&racine, "enregistrer-saisie", saisie(json!({"salarie": "FULLER", "spectacle": "le-banc-des-menteurs", "lignes": lignes(1, &["2026-11-27"])})), jour));
    outils::reussi(&outils::route(&racine, "valider-engagement", avec(json!({"valide": true})), jour));
    assert_eq!(documents(), ["a_faire"], "le bulletin du contrat ajoute est a envoyer");
    outils::reussi(&outils::route(&racine, "dsn-a-deposer", json!({"mois": "2026-12"}), jour));
    let dsn = std::fs::read(racine.join("sorties/2026-12/DSN_Decembre2026.dsn")).unwrap().iter().map(|&o| o as char).collect::<String>();
    assert!(dsn.contains("S21.G00.40.009,'FU1.26.02'"), "la DSN de decembre declare le contrat ajoute");
    assert!(bulletins_hors_du_mois().is_empty(), "la DSN a fabrique un bulletin : {:?}", bulletins_hors_du_mois());
    outils::reussi(&outils::route(&racine, "documents-imprimer", json!({"mois": "2026-11"}), jour));
    assert_eq!(documents(), ["fait"]);
    assert_eq!(paie_du_second()["bulletin"], json!("2/2026"));

    outils::reussi(&outils::route(&racine, "valider-engagement", avec(json!({"valide": false, "confirme": true})), jour));
    outils::reussi(&outils::route(&racine, "enregistrer-saisie", saisie(json!({"salarie": "FULLER", "spectacle": "le-banc-des-menteurs", "numero": "FU1.26.02", "lignes": lignes(2, &["2026-11-27", "2026-11-28"])})), jour));
    outils::reussi(&outils::route(&racine, "valider-engagement", avec(json!({"valide": true})), jour));
    assert_eq!(documents(), ["a_faire"], "le bulletin parti est a rectifier");
    std::fs::remove_dir_all(racine.join("sorties/2026-12")).unwrap();
    outils::reussi(&outils::route(&racine, "dsn-a-deposer", json!({"mois": "2026-12"}), jour));
    assert!(bulletins_hors_du_mois().is_empty(), "la DSN a fabrique un bulletin : {:?}", bulletins_hors_du_mois());
    outils::reussi(&outils::route(&racine, "documents-imprimer", json!({"mois": "2026-11"}), jour));
    let p = paie_du_second();
    assert_eq!((&p["bulletin"], &p["rectifie"]), (&json!("3/2026"), &json!("2/2026")), "{}", p);
}
