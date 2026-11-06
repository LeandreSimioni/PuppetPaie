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

/// LES TRETEAUX DU BANC : le petit scenario de la preuve, joue au banc. Une compagnie ajoutee par le « + »
/// d'un logiciel bride, configuree juste assez pour payer (convention 3090), une comedienne, deux cachets les 20 et
/// 21/11/2026 ; puis le geste d'atelier qui rend ses envois simules, et le calendrier avance au 20/11, au 01/12, au 16/12 :
/// l'horloge de la fictive valide l'engagement, depose la DPAE (test puis reel), fabrique et depose la DSN de novembre et
/// son signalement d'amorcage (test puis reel, chacun sur le bilan de son test), envoie les documents.
#[test]
fn les_treteaux_du_banc_se_rejouent_et_rendent_les_memes_sorties() {
    let releve = outils::rejeu::rejouer_et_comparer("treteaux-du-banc", &outils::dossier_de_test("rejeu", "treteaux"));
    let f = &releve["fichiers"];

    // Le contrat de la vie d'une fictive : la DPAE et la DSN du mois partent en reel (jouees), chacune sur le bilan de son
    // test (`depot_acte::exiger_repetition`) ; le mois a ses bulletins et ses certificats CS.
    let depots = f["journal/depots.jsonl"].as_array().expect("le journal des depots");
    let reel = |quoi: &str, fichier: &str| {
        depots.iter().any(|l| l["quoi"] == json!(quoi) && l["mode"] == json!("reel") && l["accepte"] == json!(true) && l["fichier"] == json!(fichier))
    };
    assert!(reel("dsn", "DSN_Novembre2026.dsn"), "la DSN de novembre doit partir en reel");
    assert!(reel("dsn", "Amorcage01_FULLER_20261120.dsn"), "le signalement d'amorcage doit partir en reel");
    assert!(depots.iter().any(|l| l["quoi"] == json!("dpae") && l["mode"] == json!("reel") && l["accepte"] == json!(true)), "la DPAE doit partir en reel");
    for doc in ["sorties/2026-11/envoi/Bulletin_FULLER_Novembre2026.pdf", "sorties/2026-11/envoi/CertificatCS_FULLER_Novembre2026.pdf"] {
        assert!(f.get(doc).is_some(), "{} manque", doc);
    }
    // La compagnie ajoutee le 02/11/2026 ne coche aucune echeance de 2026 fermee avant.
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
    // Il se lit en chemin, le soir de l'embauche et celui de l'echeance : la DSN de novembre est a deposer tant que
    // l'echeance n'est pas venue, puis faite ; elle declare alors le contrat de la comedienne, et son bulletin porte ses
    // deux cachets.
    let mut soirs = Vec::new();
    let (r2, racine2) = outils::rejeu::rejouer_et_lire("treteaux-du-banc", &outils::dossier_de_test("rejeu", "treteaux-bis"), &["2026-11-20", "2026-12-16"], |s| {
        let contrat = s.bloc_dsn("2026-11", "FULLER").into_iter().find(|(k, _)| k == "S21.G00.40.009").map(|(_, v)| v);
        soirs.push((s.jour.clone(), s.case("2026-11", "dsn"), contrat, s.ligne_de_bulletin("2026-11", "FULLER", "Cachet")));
    });
    assert_eq!(soirs[0].1, ["a_faire"], "le 20/11 : {:?}", soirs[0]);
    assert_eq!((&soirs[0].2, &soirs[0].3), (&None, &None), "le 20/11, ni DSN ni bulletin : {:?}", soirs[0]);
    assert_eq!(soirs[1].1, ["fait"], "le 16/12 : {:?}", soirs[1]);
    assert_eq!(soirs[1].2.as_deref(), Some("FU1.26.01"), "le 16/12 : {:?}", soirs[1]);
    assert!(soirs[1].3.as_deref().is_some_and(|l| l.contains("2")), "le 16/12 : {:?}", soirs[1]);
    let jour = socle::paris::iso_secondes(r2.fin.unwrap())[..10].to_string();
    let second: Value = outils::rejeu::sorties(&racine2, &jour);
    assert!(second == releve, "deux rejeux du meme scenario ne rendent pas les memes sorties");

    // UNE PIECE PARTIE DONT LE FICHIER MANQUE SE REFAIT A L'IDENTIQUE (`depart::piece_partie`, route `piece-partie`) :
    // `sorties/` effacee, chaque piece partie du mois a les octets d'avant ; l'AER se relit dans `retours/`.
    let pieces = [
        "2026-11/DPAE_Novembre2026_20261120-0700.txt",
        "2026-11/DPAE_Novembre2026_20261120-0700.txt.zip",
        "2026-11/Amorcage01_FULLER_20261120.dsn",
        "2026-11/envoi/Contrat_FULLER_FU1.26.01_Novembre2026.pdf",
        "2026-11/envoi/AER_FULLER_Novembre2026.pdf",
        "2026-11/DSN_Novembre2026.dsn",
        "2026-11/envoi/Bulletin_FULLER_Novembre2026.pdf",
        "2026-11/envoi/CertificatCS_FULLER_Novembre2026.pdf",
        "2026-11/Bulletins_Novembre2026.pdf",
        "2026-11/CertificatsCS_Novembre2026.pdf",
    ];
    let avant: Vec<Vec<u8>> = pieces.iter().map(|p| std::fs::read(racine2.join("sorties").join(p)).unwrap()).collect();
    std::fs::remove_dir_all(racine2.join("sorties")).unwrap();
    for (p, octets) in pieces.iter().zip(&avant) {
        let nom = p.rsplit('/').next().unwrap();
        let r = outils::route(&racine2, "piece-partie", json!({"mois": "2026-11", "fichier": nom}), &jour);
        assert_eq!(outils::reussi(&r)["chemin"], json!(format!("sorties/{}", p)), "{}", r);
        assert!(std::fs::read(racine2.join("sorties").join(p)).unwrap() == *octets, "{} ne se refait pas a l'identique", p);
    }
}

/// La demo Les Faisceaux sans la civilite du contact DSN. La DSN
/// de janvier ne se fabrique pas (le moteur la refuse) ; l'horloge n'envoie donc pas ses documents aux salaries -- elle
/// les envoie apres la DSN du mois, jamais avant.
#[test]
fn les_documents_d_un_mois_qui_n_est_pas_parti_ne_partent_pas() {
    let d = outils::dossier_de_test("rejeu", "faisceaux-sans-civilite");
    let (r, racine) = outils::rejeu::rejouer("faisceaux-sans-civilite", &d);
    assert!(r.horloge.iter().any(|p| p.contains("DSN de janvier 2026 déposée") && p.contains("non joué")), "{:?}", r.horloge);
    let depots = outils::journal(&racine);
    let documents = depots.as_array().unwrap().iter().filter(|l| l["quoi"] == json!("documents")).count();
    assert_eq!(documents, 0, "des documents sont partis pour un mois sans DSN : {}", depots);
    assert!(!r.horloge.iter().any(|p| p.contains("documents")), "{:?}", r.horloge);
}

/// Un engagement dont un envoi reel accepte est parti (la DPAE, la DSN de novembre des Treteaux) ne se supprime
/// pas. La route avertit et rien n'est ecrit : l'engagement reste tel qu'il etait.
#[test]
fn un_engagement_dont_un_envoi_est_parti_ne_se_supprime_pas() {
    let (_, racine) = outils::rejeu::rejouer("treteaux-du-banc", &outils::dossier_de_test("rejeu", "parti"));
    let ecrits = || [racine.join("engagements/2026-11.json"), racine.join("paies/2026-11.json")].map(|p| std::fs::read(p).unwrap());
    let avant = ecrits();
    let charge = json!({"mois": "2026-11", "engagement": outils::identite_a(&racine, "2026-11", 0)});
    let r = outils::route(&racine, "retirer-engagement", charge, "2026-12-16");
    assert!(outils::reussi(&r)["avertissement"].is_string(), "{}", r);
    assert!(ecrits() == avant, "l'avertissement a ecrit l'engagement");
}

/// L'engagement des Treteaux, sa DPAE et sa DSN de novembre parties, devient non actif apres la veille de
/// l'echeance de novembre. Ressort la DSN de novembre a corriger ; la DSN de decembre porte la
/// correction qui annule tout ce qui avait ete declare pour lui, et rien a la place (cahier technique DSN 2027.1,
/// `atelier/reference/norme-dsn/texte-2027/chapitre-2-principes-de-constitution.txt`, §2.1.2.1 et §2.1.3.6). Partie,
/// elle ne se porte plus.
///
/// La ligne DSN de decembre : la correction de novembre a porter la rend a deposer (§2.1.2.1 : une correction « donne
/// lieu à la déclaration d'une correction dans la DSN suivante »).
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
    let charge = json!({"mois": "2026-11", "engagement": outils::identite_a(&racine, "2026-11", 0), "confirme": true});
    outils::reussi(&outils::route(&racine, "retirer-engagement", charge, "2026-12-16"));
    assert_eq!(dsn_de_decembre(), ["a_faire"], "la DSN de decembre porte la correction de novembre");

    let ctx = |jour: &str| droit::contexte::Contexte::nouveau(&racine, Some(socle::date::Jour::lire(jour).unwrap())).unwrap();
    let na = engagement::etats::non_actifs_du_mois(&ctx("2026-12-16"), "2026-11").unwrap().pop().expect("l'engagement non actif");
    let a_refaire = na.a_refaire(&ctx("2026-12-16"), &envois::journal::lu(&racine).map(|j| j.departs).unwrap());
    assert!(a_refaire.iter().any(|a| matches!(a, ARefaire::Dsn { mois, .. } if mois == "2026-11")), "{:?}", a_refaire);

    // La DSN de decembre : le versement de la correction annule celui de novembre, montant pour montant.
    outils::reussi(&outils::route(&racine, "depot-apercu", json!({"quoi": "dsn", "mois": "2026-12"}), "2026-12-16"));
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
    let mut e = envois::journal::Envoi::nouveau(envois::journal::Mode::Reel, "test", true, "", "", envois::journal::DetailDsn::default());
    e.mois = Some(socle::mois::Mois::lire("2026-12").unwrap());
    e.quand = "2027-01-05T10:00:00".into();
    e.origine = "le test".into();
    e.versions = Some(serde_json::from_value(json!([{"salarie": "FULLER", "numero": "FU1.26.01", "version": na.version(), "mois": "2026-11"}])).unwrap());
    envois::journal::noter(&racine, envois::journal::Ligne::Dsn(e), Some(std::path::Path::new("DSN_Decembre2026.dsn")), "").unwrap();
    let a_refaire = na.a_refaire(&ctx("2027-01-20"), &envois::journal::lu(&racine).map(|j| j.departs).unwrap());
    assert!(!a_refaire.iter().any(|a| matches!(a, ARefaire::Dsn { .. })), "{:?}", a_refaire);
    assert!(declarations::produire_regularisations::regularisations_pour(&ctx("2027-01-20"), "2027-01", &envois::journal::lu(&racine).map(|j| j.departs).unwrap()).unwrap().is_empty());
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
    let paie_du_second = || -> Value { outils::paie_ecrite(&racine, "2026-11", "FULLER", Some("FU1.26.02")) };

    let second = json!({"salarie": "FULLER", "spectacle": "le-banc-des-menteurs", "lignes": lignes(1, &["2026-11-27"])});
    outils::reussi(&outils::route(&racine, "valider-engagement", json!({"mois": "2026-11", "saisie": second}), jour));
    assert_eq!(documents(), ["fait", "a_faire"], "le bulletin du contrat ajoute est a envoyer, celui du premier est parti");
    outils::reussi(&outils::route(&racine, "depot-apercu", json!({"quoi": "dsn", "mois": "2026-12"}), jour));
    let dsn = std::fs::read(racine.join("sorties/2026-12/DSN_Decembre2026.dsn")).unwrap().iter().map(|&o| o as char).collect::<String>();
    assert!(dsn.contains("S21.G00.40.009,'FU1.26.02'"), "la DSN de decembre declare le contrat ajoute");
    assert!(bulletins_hors_du_mois().is_empty(), "la DSN a fabrique un bulletin : {:?}", bulletins_hors_du_mois());
    outils::reussi(&outils::route(&racine, "documents-imprimer", json!({"mois": "2026-11"}), jour));
    assert_eq!(documents(), ["fait"]);
    assert_eq!(paie_du_second()["bulletin"], json!("2/2026"));

    // « Modifier l'engagement » : sans changement, rien ne s'ecrit et rien ne se dit ; son bulletin parti, le changement
    // vient avec l'avertissement et rien ne s'ecrit sans la confirmation.
    let modifier = |lignes: Value, confirme: bool| {
        let saisie = json!({"salarie": "FULLER", "spectacle": "le-banc-des-menteurs", "numero": "FU1.26.02", "lignes": lignes});
        outils::route(&racine, "modifier-engagement", json!({"mois": "2026-11", "confirme": confirme,
            "engagement": outils::identite(&racine, "2026-11", "FULLER", Some("FU1.26.02")), "saisie": saisie}), jour)
    };
    let ecrits = || [racine.join("engagements/2026-11.json"), racine.join("paies/2026-11.json")].map(|p| std::fs::read(p).unwrap());
    let avant = ecrits();
    let r = outils::reussi(&modifier(lignes(1, &["2026-11-27"]), false)).clone();
    assert_eq!((&r["change"], r.get("avertissement")), (&json!(false), None), "{}", r);
    assert!(ecrits() == avant, "une modification sans changement a ecrit");
    let r = outils::reussi(&modifier(lignes(2, &["2026-11-27", "2026-11-28"]), false)).clone();
    let consequences: Vec<Value> = r["avertissement"]["engagements"].as_array().into_iter().flatten().flat_map(|e| e["consequences"].as_array().cloned().unwrap_or_default()).collect();
    assert!(consequences.iter().any(|p| p.as_str().is_some_and(|p| p.contains("bulletin rectificatif"))), "{}", r);
    assert!(ecrits() == avant, "une modification non confirmee a ecrit");
    outils::reussi(&modifier(lignes(2, &["2026-11-27", "2026-11-28"]), true));
    assert_eq!(documents(), ["fait", "a_faire"], "le bulletin parti du second est a rectifier");
    std::fs::remove_dir_all(racine.join("sorties/2026-12")).unwrap();
    outils::reussi(&outils::route(&racine, "depot-apercu", json!({"quoi": "dsn", "mois": "2026-12"}), jour));
    assert!(bulletins_hors_du_mois().is_empty(), "la DSN a fabrique un bulletin : {:?}", bulletins_hors_du_mois());
    outils::reussi(&outils::route(&racine, "documents-imprimer", json!({"mois": "2026-11"}), jour));
    let p = paie_du_second();
    assert_eq!((&p["bulletin"], &p["rectifie"]), (&json!("3/2026"), &json!("2/2026")), "{}", p);
}

/// CE QUE LES ORGANISMES REPONDENT A LA FICTIVE (`envois::organismes_simules`), sur la DSN de novembre des Treteaux, deposee
/// en reel par l'horloge : la DGFiP communique le taux que le scenario lui a annonce (compte rendu 94, lu par le lecteur des
/// vrais) et la releve l'ecrit dans la fiche du salarie -- un taux se lit, il ne se devine pas (regle 1) ; puis net-entreprises
/// rend un bilan non conforme annonce (rapport BAN, nature 11) : la DSN du mois est refusee, avec l'anomalie dite, et la
/// DGFiP ne repond plus.
#[test]
fn les_organismes_de_la_fictive_rendent_le_taux_du_pas_et_le_refus_annonces() {
    let (r, racine) = outils::rejeu::rejouer("treteaux-du-banc", &outils::dossier_de_test("rejeu", "organismes"));
    let fin = r.fin.unwrap();
    let journal = envois::journal::chemin(&racine);
    let dsn_de_novembre = |d: &Value| -> Value {
        d.as_array().unwrap().iter().find(|l| l["quoi"] == json!("dsn") && l["mode"] == json!("reel") && l["fichier"] == json!("DSN_Novembre2026.dsn")).unwrap().clone()
    };
    // Le depot attend sa reponse : ce qui en est revenu s'efface du journal.
    let en_attente = || {
        let mut siens: Vec<Value> = Vec::new();
        let mut texte = String::new();
        for l in std::fs::read_to_string(&journal).unwrap().lines() {
            let mut e: Value = serde_json::from_str(l).unwrap();
            if e["ligne"]["fichier"] == json!("DSN_Novembre2026.dsn") && e["ligne"]["mode"] == json!("reel") {
                siens.push(e["id"].clone());
                e["ligne"]["retours"] = json!([]);
            } else if siens.contains(&e["a"]) {
                continue;
            }
            texte.push_str(&format!("{}\n", e));
        }
        std::fs::write(&journal, texte).unwrap();
    };
    let annoncer = |a: Value| {
        // Annoncee avant le depot : le 1er novembre.
        let le = socle::paris::naif_de_iso("2026-11-01T12:00:00").unwrap() as f64;
        socle::temps::pendant(le, || envois::organismes_simules::annoncer(&racine, moteur::compagnie::bridage::branchement(&racine), &a)).unwrap();
    };
    let relever = || {
        socle::temps::pendant(fin, || {
            let ctx = droit::contexte::Contexte::nouveau(&racine, None).unwrap();
            moteur::compagnie::releve::rattacher_simulation(&ctx).unwrap();
        })
    };
    let ctx = droit::contexte::Contexte::nouveau(&racine, None).unwrap();
    let (salarie, fiche) = ctx.salaries.iter().next().map(|(c, f)| (c.clone(), f.clone())).unwrap();
    assert!(fiche.nir.est_dite(), "la fiche de la comedienne porte son NIR");

    en_attente();
    annoncer(json!({"taux_pas": {"salarie": salarie, "taux": "4.30"}}));
    relever();
    let natures: Vec<Value> = dsn_de_novembre(&outils::journal(&racine))["retours"].as_array().unwrap().iter().map(|x| x["nature"].clone()).collect();
    assert!(natures.contains(&json!("94")) && natures.contains(&json!("11")), "{:?}", natures);
    let fiches = outils::lire(&racine, "config/salaries.json");
    let recu = fiches[salarie.as_str()]["taux_pas"].as_array().and_then(|l| l.last()).map(|e| e["valeur"].clone()).unwrap_or_default();
    assert_eq!(recu["taux"], json!("4.30"), "{}", fiches[salarie.as_str()]);
    assert_eq!(recu["type"], json!("01"), "{}", recu);

    en_attente();
    annoncer(json!({"refus": {"code": "CST-04", "message": "Code URSSAF invalide", "rubrique": "S21.G00.20.001", "mois": "2026-11"}}));
    relever();
    let ligne = dsn_de_novembre(&outils::journal(&racine));
    let bilan = ligne["retours"].as_array().unwrap().iter().find(|x| x["nature"] == json!("11")).cloned().unwrap();
    assert_eq!(bilan["verdict"], json!("KO"), "{}", ligne);
    assert!(bilan["anomalies"].to_string().contains("Code URSSAF invalide"), "{}", bilan);
    assert!(!ligne["retours"].as_array().unwrap().iter().any(|x| x["nature"] == json!("94")), "une DSN refusee n'a pas de taux : {}", ligne);
    let etat = envois::etats::etat_dsn_du_mois(&envois::journal::lu(&racine).unwrap().lignes, socle::mois::Mois::lire("2026-11").unwrap(), None, &ctx.entreprise.nic_principal()).unwrap();
    assert!(etat.refusee, "{:?}", etat);

    // UN FICHIER REFUSE NE REPART PAS TOUT SEUL : a l'ouverture suivante, l'horloge refait la DSN a son echeance, octet
    // pour octet, et ne la redepose pas ; une fois corrigee (le telephone du declarant, S20.G00.07.002), elle part, et le
    // mois est accepte.
    let de_novembre = || outils::journal(&racine).as_array().unwrap().iter().filter(|l| l["fichier"] == json!("DSN_Novembre2026.dsn")).count();
    let ouvrir = |jour: &str| {
        let le = socle::paris::naif_de_iso(&format!("{}T09:00:00", jour)).unwrap() as f64;
        socle::temps::pendant(le, || moteur::horloge::rattraper(&racine, Some(socle::date::Jour::lire(jour).unwrap()), true)).unwrap()
    };
    let avant = de_novembre();
    ouvrir("2026-12-20");
    assert_eq!(de_novembre(), avant, "le fichier refuse ne repart pas");
    let mut entreprise = outils::lire(&racine, "config/entreprise.json");
    entreprise["declarant_dsn"][0]["valeur"]["tel"] = json!("0100000053");
    std::fs::write(racine.join("config/entreprise.json"), serde_json::to_string_pretty(&entreprise).unwrap()).unwrap();
    ouvrir("2026-12-21");
    assert_eq!(de_novembre(), avant + 2, "le fichier corrige part, en test puis en reel");
    let etat = envois::etats::etat_dsn_du_mois(&envois::journal::lu(&racine).unwrap().lignes, socle::mois::Mois::lire("2026-11").unwrap(), None, &ctx.entreprise.nic_principal()).unwrap();
    assert!(!etat.refusee, "{:?}", etat);
    ouvrir("2026-12-22");
    assert_eq!(de_novembre(), avant + 2, "une DSN acceptee ne repart pas");
}

/// LA DEMANDE DE TAUX D'UNE EMBAUCHE PART, ET SA REPONSE FAIT L'ENGAGEMENT (le parcours du joueur L8, sa compagnie du Fil
/// de Fer, mars 2026). Deux comediennes des Treteaux, FULLER et une seconde, sans taux, engagees les 20 et 21/11 ; leur
/// DPAE est deposee en test le 16/11. Le 17/11, un taux arrive pour FULLER : son engagement valide se refait de lui-meme
/// (son calcul depend du taux de son salarie au jour du versement, `droit::pas::taux_dgfip`), sans geste. Le 20/11,
/// l'horloge depose la DPAE en reel : la seconde, seule sans taux, voit sa demande partir et sa reponse revenir ; FULLER,
/// qui a le sien, n'en a pas (une demande, une fois, pour qui en a besoin). La DSN de novembre les declare toutes deux au
/// taux transmis par la DGFiP (type 01).
#[test]
fn la_demande_de_taux_d_une_embauche_part_et_sa_reponse_fait_l_engagement() {
    let d = outils::dossier_de_test("rejeu", "demande_de_taux");
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    let gestes = std::fs::read_to_string(outils::rejeu::scenario("treteaux-du-banc").join("gestes.jsonl")).unwrap();
    let debut: Vec<&str> = gestes.lines().take_while(|l| !l.contains("\"geste\":\"jour\"")).collect();
    std::fs::write(sc.join("gestes.jsonl"), debut.join("\n") + "\n").unwrap();
    let (_, racine) = outils::rejeu::rejouer_dossier(&sc, &d);

    // La seconde comedienne : la fiche de FULLER, un autre NIR, la meme carte.
    let mut fiches = outils::lire(&racine, "config/salaries.json");
    let mut seconde = fiches["FULLER"].clone();
    let nir = seconde["nir"][0]["valeur"].as_str().unwrap().to_string();
    let nir = format!("{}{}", &nir[..10], if &nir[10..] == "001" { "003" } else { "001" });
    let cle = 97 - nir.parse::<u64>().unwrap() % 97;
    for (k, v) in [("nir", nir.clone()), ("nir_cle", format!("{:02}", cle)), ("nom", "DUNCAN".into()), ("nom_affichage", "Duncan".into()), ("prenom", "ISADORA".into()), ("prenom_affichage", "Isadora".into()), ("code_salarie", "2".into()), ("initiales_contrat", "DU".into()), ("email", "isadora.duncan@exemple.invalid".into())] {
        seconde[k] = json!([{"valeur": v}]);
    }
    fiches["DUNCAN"] = seconde;
    outils::ecrire(&racine, "config/salaries.json", &fiches);
    let vue = socle::ecriture::en_valeur(&engagement::stock::vue_du_mois(&racine, "2026-11").unwrap().unwrap());
    let mut carte = vue["engagements"][0].clone();
    carte["salarie"] = json!("DUNCAN");
    for k in ["numero", "numero_pour", "valide"] {
        carte.as_object_mut().unwrap().remove(k);
    }
    outils::reussi(&outils::route(&racine, "enregistrer-saisie", json!({"mois": "2026-11", "saisie": carte}), "2026-11-16"));
    for s in ["FULLER", "DUNCAN"] {
        outils::reussi(&outils::valider_la_carte(&racine, "2026-11", s, "2026-11-16"));
    }
    outils::reussi(&outils::route(&racine, "deposer", json!({"quoi": "dpae", "mois": "2026-11", "reel": false}), "2026-11-16"));

    let type_ecrit = |s: &str| -> Value { outils::paie_ecrite(&racine, "2026-11", s, None)["pas"]["type"].clone() };
    assert_eq!(type_ecrit("FULLER"), json!("13"), "sans taux recu, la grille");
    // Le taux de FULLER arrive le 17/11 dans sa fiche ; lire le mois suffit a refaire son engagement.
    let mut fiches = outils::lire(&racine, "config/salaries.json");
    fiches["FULLER"]["taux_pas"] = json!([{"valeur": ""}, {"depuis": "2026-11-17", "valeur": {"taux": "4.30", "type": "01", "identifiant": "190151266", "mis_a_disposition": "20261117100000"}}]);
    outils::ecrire(&racine, "config/salaries.json", &fiches);
    outils::reussi(&outils::route(&racine, "apercu", json!({"mois": "2026-11"}), "2026-11-17"));
    assert_eq!(type_ecrit("FULLER"), json!("01"), "le taux arrive refait l'engagement");

    for jour in ["2026-11-20", "2026-12-16"] {
        let le = socle::paris::naif_de_iso(&format!("{}T09:00:00", jour)).unwrap() as f64;
        socle::temps::pendant(le, || moteur::horloge::rattraper(&racine, Some(socle::date::Jour::lire(jour).unwrap()), true)).unwrap();
    }
    let depots = outils::journal(&racine);
    let demandes = |s: &str| {
        depots.as_array().unwrap().iter().filter(|l| l["mode"] == json!("reel") && l["accepte"] == json!(true) && l["fichier"].as_str().is_some_and(|f| f.starts_with(&format!("Amorcage01_{}_", s)))).count()
    };
    assert_eq!((demandes("FULLER"), demandes("DUNCAN")), (0, 1), "une demande, une fois, pour qui n'a pas de taux");
    let fiches = outils::lire(&racine, "config/salaries.json");
    assert!(fiches["DUNCAN"]["taux_pas"].as_array().is_some_and(|l| l.iter().any(|e| e["valeur"]["type"] == json!("01"))), "la reponse est dans sa fiche : {}", fiches["DUNCAN"]);
    let dsn = String::from_utf8_lossy(&std::fs::read(racine.join("sorties/2026-11/DSN_Novembre2026.dsn")).unwrap()).into_owned();
    let types: Vec<&str> = dsn.lines().filter_map(|l| l.strip_prefix("S21.G00.50.007,")).collect();
    assert_eq!(types, ["'01'", "'01'"], "la DSN de novembre declare les deux au taux transmis");
}

/// LE COMPTE RENDU D'UNE DSN NE REFAIT PAS LA PAIE QU'ELLE DECLARE (le parcours du joueur L9, sa compagnie du Fil de Fer,
/// decembre 2026 : la DSN deposee le 21/12 pour un versement du 31/12, son compte rendu du 21/12 refaisait la paie et la
/// DSN redevenait « à refaire »). Les Treteaux, FULLER payee le 20/12 pour novembre (CCH-13 : au plus tard le dernier jour
/// du mois suivant), au taux arrive le 17/11. La DSN de novembre est deposee le 14/12, avant le versement, ses documents
/// envoyes ensuite ; son compte rendu, en retour de la DSN de novembre, vaut pour les mois qui suivent (BOI-IR-PAS-30-10-20,
/// § 70, exemple 1 ; `droit::pas::en_vigueur`) : la paie de novembre garde son taux, et sa DSN reste faite.
#[test]
fn le_compte_rendu_d_une_dsn_ne_refait_pas_la_paie_qu_elle_declare() {
    let d = outils::dossier_de_test("rejeu", "compte_rendu_de_sa_dsn");
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    let gestes = std::fs::read_to_string(outils::rejeu::scenario("treteaux-du-banc").join("gestes.jsonl")).unwrap();
    let debut: Vec<&str> = gestes.lines().take_while(|l| !l.contains("\"geste\":\"jour\"")).collect();
    std::fs::write(sc.join("gestes.jsonl"), debut.join("\n") + "\n").unwrap();
    let (_, racine) = outils::rejeu::rejouer_dossier(&sc, &d);

    let mut fiches = outils::lire(&racine, "config/salaries.json");
    fiches["FULLER"]["taux_pas"] = json!([{"valeur": ""}, {"depuis": "2026-11-17", "valeur": {"taux": "4.30", "type": "01", "identifiant": "190151266", "mis_a_disposition": "20261117100000"}}]);
    outils::ecrire(&racine, "config/salaries.json", &fiches);
    let vue = socle::ecriture::en_valeur(&engagement::stock::vue_du_mois(&racine, "2026-11").unwrap().unwrap());
    let mut carte = vue["engagements"][0].clone();
    carte["date_paiement"] = json!("2026-12-20");
    for k in ["numero_pour", "valide"] {
        carte.as_object_mut().unwrap().remove(k);
    }
    outils::reussi(&outils::route(&racine, "enregistrer-saisie", json!({"mois": "2026-11", "saisie": carte}), "2026-11-16"));
    outils::reussi(&outils::valider_la_carte(&racine, "2026-11", "FULLER", "2026-11-16"));
    for jour in ["2026-11-20", "2026-12-01"] {
        let le = socle::paris::naif_de_iso(&format!("{}T09:00:00", jour)).unwrap() as f64;
        socle::temps::pendant(le, || moteur::horloge::rattraper(&racine, Some(socle::date::Jour::lire(jour).unwrap()), true)).unwrap();
    }
    for reel in [false, true] {
        outils::reussi(&outils::route(&racine, "deposer", json!({"quoi": "dsn", "mois": "2026-11", "reel": reel}), "2026-12-14"));
    }
    outils::reussi(&outils::route(&racine, "envoyer-documents", json!({"mois": "2026-11"}), "2026-12-14"));
    let etat = outils::reussi(&outils::route(&racine, "etat-du-mois", json!({"mois": "2026-11"}), "2026-12-14")).to_string();
    assert!(!etat.contains("à refaire"), "la DSN de novembre reste faite : {}", etat);
    let fiches = outils::lire(&racine, "config/salaries.json");
    let en_retour = fiches["FULLER"]["taux_pas"].as_array().unwrap().iter().find(|e| e["valeur"]["en_retour_de"] == json!("2026-11")).cloned();
    assert!(en_retour.is_some(), "le compte rendu de la DSN de novembre est dans sa fiche : {}", fiches["FULLER"]);
    assert_eq!(outils::paie_ecrite(&racine, "2026-11", "FULLER", None)["pas"]["identifiant"], json!("190151266"), "la paie de novembre garde son taux");
}

/// UNE FICTIVE DE REFERENCE (`atelier/fictives/<nom>/`), PREUVE COMMUNE EN LIGNE DE COMMANDE : une compagnie venue d'un
/// logiciel precedent, qui importe ses papiers de 2025, remplit « Pour démarrer », puis vit 2026 par les routes seulement :
/// chaque engagement valide avant son premier jour, sa DPAE (test puis reel), ses contrats a la signature, la DSN de chaque
/// mois apres son versement (test puis reel), les documents. LIVREE le 02/11/2026 comme « Publier » la livre
/// (`scenario::livrer`) : l'horloge n'y trouve rien a faire ce jour-la, les cartes de decembre sont ecrites et non
/// validees (`decembre` : combien) ; puis le temps passe au 31/01/2027, et l'horloge seule valide, declare, envoie et clot
/// 2026. Chaque mois de 2026 a toutes ses etapes faites et sa DSN partie en reel, chaque mois de `dpae` sa DPAE ; le
/// releve (paies, DSN, depots) se compare a l'oracle `fictives/<nom>.json`. Rend la racine et le jour de la fin.
fn la_fictive_tient_son_annee_et_se_livre(nom: &str, decembre: usize, dpae: &[&str]) -> (std::path::PathBuf, &'static str) {
    let d = outils::dossier_de_test("rejeu", nom);
    let racine = outils::rejeu::livrer(&outils::rejeu::reference(nom), &d, "2026-11-02");
    assert!(moteur::interface::compagnies::est_fictive_livree(&racine), "la fictive posee n'est pas une fictive livree");
    assert!(!racine.join("journal/incidents.json").exists(), "les incidents du rejeu ne se livrent pas");
    let horloge = |jour: &str| outils::reussi(&outils::route(&racine, "horloge-rattraper", json!({}), jour)).clone();
    assert_eq!(horloge("2026-11-02"), json!([]), "l'horloge a trouve a faire derriere l'administratrice");
    let cartes = outils::lire(&racine, "engagements/2026-12.json");
    let cartes = cartes["engagements"].as_array().unwrap();
    assert!(cartes.len() == decembre && cartes.iter().all(|e| e["valide"].is_null()), "les cartes de decembre, ecrites et non validees : {:?}", cartes);
    let jour = "2027-01-31";
    let jouees = horloge(jour);
    assert!(jouees.to_string().contains("exercice 2026 clos"), "{}", jouees);

    // Chaque mois de 2026 a toutes ses etapes faites ; un mois sans engagement n'a que sa DSN, sans individu.
    for m in 1..=12 {
        let mois = format!("2026-{:02}", m);
        let e = outils::reussi(&outils::route(&racine, "etat-du-mois", json!({"mois": mois}), jour)).clone();
        let etats: Vec<&str> = e["etapes"].as_array().unwrap().iter().flat_map(|x| x["groupes"].as_array().unwrap().iter().filter_map(|g| g["etat"].as_str())).collect();
        assert!(etats.contains(&"fait") && etats.iter().all(|x| *x == "fait" || *x == "rien"), "{} : {}", mois, e["phrase"]);
    }
    let exercices = outils::lire(&racine, "config/exercices.json");
    assert_eq!(exercices["2026"]["sans_dsn"], json!([]), "2026 se clot avec ses douze DSN : {}", exercices);

    // LE RELEVE, compare a l'oracle : chaque paie (brut, net, cout, et son PAS : taux, type, prelevement), chaque DSN
    // (chaque individu : contrat, taux et type de PAS, montant preleve ; chaque versement), chaque depot.
    let mut paies = serde_json::Map::new();
    // Les paies ecrites, mois par mois (`paies/AAAA-MM.json`), rangees sous l'engagement qui les porte.
    let mut ecrites: Vec<(String, Value)> = Vec::new();
    let mut fichiers: Vec<String> = std::fs::read_dir(racine.join("paies")).unwrap().flatten().map(|f| f.file_name().to_string_lossy().into_owned()).collect();
    fichiers.sort();
    for f in fichiers {
        let m = outils::lire(&racine, &format!("paies/{}", f));
        ecrites.extend(m["paies"].as_array().unwrap().iter().map(|p| (m["mois"].as_str().unwrap().to_string(), p.clone())));
    }
    // Un mois que traverse un CDI sans carte neuve (aout) n'a pas de fichier : ses paies sont sous le mois du CDI.
    for mois in engagement::stock::mois_saisis(&racine).unwrap().into_iter().filter(|m| racine.join(format!("engagements/{}.json", m)).exists()) {
        let m = outils::lire(&racine, &format!("engagements/{}.json", mois));
        let v: Vec<Value> = m["engagements"].as_array().unwrap().iter().flat_map(|e| {
            ecrites.iter().filter(move |(_, p)| p["salarie"] == e["saisie"]["carte"]["salarie"] && p["numero"] == e["saisie"]["carte"]["numero"] && !e["valide"].is_null()).map(move |(pm, p)| {
                let q = &p["presente"]["paie"];
                // L'identifiant du compte rendu qui porte le taux ne dit rien de la paie.
                let mut pas = p["presente"]["pas"].clone();
                if let Some(o) = pas.as_object_mut() {
                    o.remove("identifiant");
                }
                json!({"salarie": e["saisie"]["carte"]["salarie"], "numero": e["saisie"]["carte"]["numero"], "mois": pm, "brut": q["brut"], "net_a_payer": q["net_a_payer"],
                       "net_imposable": q["net_imposable"], "cout_employeur": q["cout_employeur"], "pas": pas})
            })
        }).collect();
        paies.insert(mois.to_string(), json!(v));
    }
    let mut dsn = serde_json::Map::new();
    for m in 1..=12 {
        let mois = format!("2026-{:02}", m);
        let f = racine.join("sorties").join(&mois).join(format!("DSN_{}.dsn", socle::mois::Mois::lire(&mois).map(socle::mois::Mois::etiquette).unwrap()));
        let texte: String = std::fs::read(&f).unwrap_or_else(|_| panic!("{} manque", f.display())).iter().map(|&o| o as char).collect();
        let (mut individus, mut versements, mut types) = (Vec::new(), Vec::new(), Vec::new());
        let mut courant: Option<serde_json::Map<String, Value>> = None;
        for (k, v) in outils::paires(&texte) {
            let cle = match k.as_str() {
                "S21.G00.30.002" => {
                    individus.extend(courant.take().map(Value::Object));
                    courant = Some(serde_json::Map::new());
                    "nom"
                }
                "S21.G00.40.009" => "contrat",
                "S21.G00.50.006" => "taux_pas",
                "S21.G00.50.007" => {
                    types.push(v.clone());
                    "type_pas"
                }
                "S21.G00.50.009" => "pas",
                "S21.G00.20.001" => {
                    versements.push(json!([v]));
                    continue;
                }
                "S21.G00.20.005" => {
                    if let Some(Value::Array(a)) = versements.last_mut() {
                        a.push(json!(v));
                    }
                    continue;
                }
                _ => continue,
            };
            if let Some(c) = courant.as_mut() {
                match c.get_mut(cle) {
                    Some(Value::Array(a)) => a.push(json!(v)),
                    _ => {
                        c.insert(cle.to_string(), json!([v]));
                    }
                }
            }
        }
        individus.extend(courant.take().map(Value::Object));
        // Chaque salarie declare l'est au taux que la DGFiP a transmis (type 01) : un taux se lit, il ne se devine pas.
        assert!(types.iter().all(|t| t == "01"), "{} : types de PAS {:?}", mois, types);
        dsn.insert(mois, json!({"individus": individus, "versements": versements}));
    }
    let depots: Vec<Value> = outils::journal(&racine).as_array().unwrap().iter()
        .map(|l| json!([l["quoi"], l["mois"], l["fichier"], l["mode"], l["accepte"]])).collect();
    let reel = |quoi: &str, mois: &str| depots.iter().any(|l| l[0] == json!(quoi) && l[1] == json!(mois) && l[3] == json!("reel") && l[4] == json!(true));
    for m in 1..=12 {
        assert!(reel("dsn", &format!("2026-{:02}", m)), "la DSN de 2026-{:02} n'est pas partie en reel", m);
    }
    for m in dpae {
        assert!(reel("dpae", &format!("2026-{}", m)), "la DPAE de 2026-{} n'est pas partie en reel", m);
    }
    outils::oracle(&format!("fictives/{}.json", nom), "", &json!({"paies": paies, "dsn": dsn, "depots": depots}));
    (racine, jour)
}

/// LE FIL DE FER (`atelier/fictives/fictive-fil-de-fer/`) : une compagnie de marionnettes (convention 3090), ses repas et
/// nuitees, le FCAP-SVP paye et coche.
#[test]
fn la_fictive_du_fil_de_fer_tient_son_annee_et_se_livre() {
    let (racine, jour) = la_fictive_tient_son_annee_et_se_livre("fictive-fil-de-fer", 2, &["01", "02", "03", "04", "05", "06", "09", "12"]);
    // LES MOIS IMPORTES SE LISENT (le bug du joueur L9 : le FCAP-SVP de 2025 refusait « aucune paie validée ») : la
    // tranche A de 2025 est celle des paies importees de septembre a decembre, et l'annee se dit partielle.
    let fcap = outils::reussi(&outils::route(&racine, "exporter", json!({"etat": "fcap-svp", "annee": "2025"}), jour)).clone();
    let ligne = &fcap["sections"][0]["lignes"][0];
    assert_eq!(ligne["mois"], json!(4), "{}", fcap);
    assert!(ligne["regle"].as_str().is_some_and(|r| r.contains("partielle")), "{}", fcap);
    // LA DSN D'UN MOIS IMPORTE NE SE FABRIQUE PAS ICI (le bug : septembre 2025, sans trace de sa DSN, s'ecrivait « 0
    // salarié ») : sa DSN importee (decembre) ou non (septembre), le refus dit quoi faire et n'ecrit rien.
    for (mois, dit) in [("2025-09", "« Importer »"), ("2025-12", "Elle ne se refait pas ici.")] {
        let r = outils::route(&racine, "depot-apercu", json!({"quoi": "dsn", "mois": mois}), jour);
        assert!(r["classe"] == json!("DsnDeLAncienLogiciel") && r["erreur"].as_str().is_some_and(|e| e.contains(dit)), "{} : {}", mois, r);
        assert!(!racine.join("sorties").join(mois).exists(), "{} : la DSN refusee s'est ecrite", mois);
    }
}

/// L'HOTEL DE BOURGOGNE (`atelier/fictives/fictive-hotel-de-bourgogne/`) : une compagnie de theatre (convention 1285),
/// trois permanents en CDI et cinq intermittents ; l'arret de Marivaux et son maintien subroge, ses heures
/// supplementaires, la demission de Clairon, les conges d'ete, l'augmentation de Champmesle et son avenant, une date
/// ajoutee a des contrats partis et leurs avenants, aout sans intermittent, la tournee de mai et ses frais.
#[test]
fn la_fictive_de_l_hotel_de_bourgogne_tient_son_annee_et_se_livre() {
    let (racine, jour) = la_fictive_tient_son_annee_et_se_livre("fictive-hotel-de-bourgogne", 8, &["01", "02", "03", "04", "05", "06", "07", "09", "10", "11", "12"]);
    // LES MOIS IMPORTES NE DEMANDENT PAS L'EFFECTIF D'AVANT EUX (le bug : l'export comptable de toutes les annees
    // refusait « effectif de la compagnie non connu », des 2025) : 2025, paye par l'ancien logiciel, lit ses taux sur ses pieces,
    // et le FNAS de la CCNEAC n'a pas a dire l'effectif de 2024 (`bareme::EntrepriseLue::taux_lus`).
    let balance = outils::route(&racine, "exporter", json!({"etat": "balance-paie", "annee": "2025", "apercu": true}), jour);
    assert_eq!(balance["ok"], json!(true), "{}", balance);
    // LES MEMES PIECES REIMPORTEES N'ANNONCENT RIEN (le bug : chaque reimport annoncait 24 contrats, les mois des deux CDI
    // sans terme, et 3 paies de decembre « completees ») : le contrat d'un permanent s'atteste mois par mois, et la paie
    // du mois porte ce que son bulletin compte.
    let apercu = outils::reussi(&outils::route(&racine, "importation-apercu", json!({}), jour)).clone();
    assert_eq!(apercu["rien_a_faire"], json!(true), "{}", apercu["phrase"]);
}
