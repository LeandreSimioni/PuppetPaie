//! UNE COMPAGNIE NEUVE QUI REPREND LES BULLETINS DE SON LOGICIEL PRECEDENT. Ce que le moteur en decide :
//!
//! - l'apercu de l'import annonce, pour une fiche a creer, les anomalies que l'ecriture trouvera, et elles seules
//!   (`crate::routes::fiches::verifier_comme_a_l_ecriture`) : un nom ou une adresse que le bulletin imprime en minuscules
//!   (« Compagnie des Essais ») va dans la case qu'on lit, la forme declaree s'en derive ; il n'est pas « a mettre en
//!   majuscules » ;
//! - l'emploi imprime qui nomme un metier du catalogue, et un seul, donne a la fiche ce que le choix de ce metier
//!   donne (le code PCS-ESE : `emplois::par_intitule`) ;
//! - la nature du contrat qu'aucun bulletin n'imprime : un bulletin qui porte une marque du spectacle (numero
//!   d'objet, cotisation Congés Spectacles, due pour l'artiste ou le technicien du spectacle, D. 7121-41 du code du
//!   travail, `atelier/reference/sources/caisse-conges-affiliation/LISEZ-MOI.md`) laisse le contrat muet, un CDDU
//!   (regle 6) ; sans aucune, le contrat est celui d'un permanent, sa nature se demande apres l'import
//!   (`importation-etat`, `a_dire`) et se dit par `importation-nature`, qui refuse une nature qui n'est pas d'un
//!   permanent sans rien ecrire.
//!
//! Chaque test cree sa compagnie par `compagnies::ajouter_dans`, comme le « + ».

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use moteur::interface::compagnies::ajouter_dans;
use serde_json::{json, Value};

use crate::bulletins_fictifs::{a_heures, a_repetitions, page_a, pdf, A};
use crate::outils::{dossier_de_test, fiche_au, lire, reussi};

const JOUR: &str = "2026-10-03";

fn route(racine: &Path, nom: &str, charge: Value) -> Value {
    crate::outils::route(racine, nom, charge, JOUR)
}

/// La fiche entreprise ecrite, lue au jour du test.
fn entreprise_lue(racine: &Path) -> Value {
    fiche_au(&lire(racine, "config/entreprise.json"), JOUR)
}

/// Les fiches des salaries ecrites, lues au jour du test.
fn salaries_lus(racine: &Path) -> Value {
    let brutes = lire(racine, "config/salaries.json");
    Value::Object(brutes.as_object().unwrap().iter().filter(|(k, _)| !k.starts_with('_')).map(|(k, v)| (k.clone(), fiche_au(v, JOUR))).collect())
}

/// Les contrats attestes des engagements importes, chacun avec son salarie.
fn contrats_importes(racine: &Path) -> Vec<Value> {
    engagement::importe::contrats_lus(racine).unwrap()
}

/// Les fichiers des engagements, tels qu'ils sont sur le disque.
fn engagements_ecrits(racine: &Path) -> Vec<(String, Vec<u8>)> {
    let mut v: Vec<(String, Vec<u8>)> = std::fs::read_dir(racine.join("engagements"))
        .unwrap()
        .flatten()
        .filter(|e| e.path().is_file())
        .map(|e| (e.file_name().to_string_lossy().into_owned(), std::fs::read(e.path()).unwrap()))
        .collect();
    v.sort();
    v
}

fn neuve(test: &str) -> PathBuf {
    ajouter_dans(&dossier_de_test("import_neuve", test), "Compagnie des Essais").unwrap().racine
}

/// L'artiste, au cachet, sur un spectacle (numero d'objet, Congés Spectacles) ; son adresse en minuscules.
fn artiste() -> A {
    let mut a = a_repetitions();
    a.id.adresse = ["3 impasse des Essais", "99999 VILLE FICTIVE"];
    a
}

/// Un permanent (NIR et nom inventes) : ses heures, ni numero d'objet ni Congés Spectacles.
fn permanent() -> A {
    let mut a = a_heures();
    a.id.nom = "TARTEMPION";
    a.id.prenom = "ZOE";
    a.id.nir = "185079999912388";
    a.id.code = "8";
    a.id.naissance = "14/07/1985";
    a.id.conges_spectacles = None;
    a.cotisations.retain(|c| c.2 != "conges_spectacles");
    a.total = ["178,03", "112,18"];
    a.cumuls[3] = ["818,03", "2 622,50"];
    a.total_verse = "818,03";
    a
}

fn deposer(racine: &Path) {
    let import = racine.join("import");
    std::fs::create_dir_all(&import).unwrap();
    pdf(vec![page_a(&artiste())], &import.join("artiste.pdf"));
    pdf(vec![page_a(&permanent())], &import.join("permanent.pdf"));
}

fn fiches_a_creer(apercu: &Value) -> Vec<(String, &Value)> {
    let mut out = vec![("entreprise".to_string(), &apercu["entreprise"])];
    for s in apercu["salaries"].as_array().unwrap() {
        out.push(("salarie".to_string(), s));
    }
    out
}

/// L'apercu annonce les anomalies que l'ecriture trouvera, et aucune sur une case que le bulletin remplit.
#[test]
fn l_apercu_annonce_ce_que_l_ecriture_trouvera() {
    let racine = neuve("apercu");
    // Sa premiere embauche dit son effectif : ses paies importees se comparent.
    let mut e = lire(&racine, "config/entreprise.json");
    e["premiere_embauche"] = json!([{"valeur": "2025-01-01"}]);
    std::fs::write(racine.join("config/entreprise.json"), serde_json::to_string_pretty(&e).unwrap()).unwrap();
    deposer(&racine);
    let apercu = reussi(&route(&racine, "importation-apercu", json!({"dossier": ""}))).clone();
    let a_creer = fiches_a_creer(&apercu);
    assert_eq!(a_creer.len(), 3, "{}", apercu);
    for (nom, f) in &a_creer {
        assert_eq!(f["etat"], json!("a_creer"), "{}", f);
        for a in f["anomalies"].as_array().unwrap() {
            let champ = a["champ"].as_str().unwrap();
            let lu = &f["donnees"][champ];
            assert!(lu.is_null() || lu == &json!("") || lu == &json!([]), "{} : {} est rempli ({}) et pourtant signale : {}", nom, champ, lu, a);
        }
    }
    let r = reussi(&route(&racine, "importer", json!({"dossier": ""}))).clone();
    let entreprise = entreprise_lue(&racine);
    let salaries = salaries_lus(&racine);
    for (nom, f) in &a_creer {
        let ecrite = if nom == "entreprise" {
            entreprise.clone()
        } else {
            let nir = f["donnees"]["nir"].clone();
            salaries.as_object().unwrap().values().find(|s| s["nir"] == nir).cloned().unwrap_or_else(|| panic!("{} non ecrit : {}", nir, r))
        };
        let v = reussi(&route(&racine, "verifier-fiche", json!({"nom": nom, "donnees": ecrite}))).clone();
        let erreurs = |x: &Value| -> BTreeSet<String> {
            x.as_array().unwrap().iter().filter(|a| a["gravite"] == json!("erreur")).map(|a| a["champ"].as_str().unwrap().to_string()).collect()
        };
        assert_eq!(erreurs(&f["anomalies"]), erreurs(&v["anomalies"]), "{} : l'apercu et la fiche ecrite ne disent pas la meme chose", nom);
    }
    // L'emploi imprime « ARTISTE DRAMATIQUE » nomme un metier du catalogue : la fiche a son code PCS-ESE.
    let brioche = salaries.as_object().unwrap().values().find(|s| s["nir"] == json!("2881175999001")).unwrap();
    assert_eq!(brioche["pcs_ese"], json!("354c"), "{}", brioche);
    assert_eq!(brioche["statut"], json!("artiste"));

    // LES ECARTS DE CALCUL DE L'IMPORT PARTENT SANS RIEN QUI DESIGNE UNE PERSONNE : ni nom, prenom, NIR, naissance ou
    // adresse des fiches, ni brut, net ou cout d'une paie importee, dans ce qui partirait chez l'editeur.
    let partiraient: Vec<String> = moteur::compagnie::incidents::lire(&racine, None, false)
        .unwrap()
        .iter()
        .filter(|l| l["ou"] == json!(moteur::import::ecarts::OU))
        .map(|l| moteur::compagnie::incidents::pour_l_editeur(&racine, l).unwrap().to_string())
        .collect();
    assert!(!partiraient.is_empty(), "l'import n'a rien rapporte de ses ecarts");
    let mut interdits: Vec<String> = Vec::new();
    for s in salaries.as_object().unwrap().values() {
        for k in ["nom", "prenom", "nir", "date_naissance", "adresse", "commune"] {
            if let Some(v) = s[k].as_str().filter(|v| v.len() > 2) {
                interdits.push(v.to_string());
            }
        }
    }
    for p in engagement::importe::paies_lues(&racine).unwrap() {
        for k in ["brut", "net_a_payer", "net_imposable", "net_social", "cout_employeur"] {
            if let Some(v) = p[k].as_str() {
                interdits.extend([v.to_string(), v.replace('.', ",")]);
            }
        }
    }
    assert!(interdits.len() > 10, "{:?}", interdits);
    for texte in &partiraient {
        for x in &interdits {
            assert!(!texte.contains(x.as_str()), "« {} » partirait : {}", x, texte);
        }
    }
}

/// La nature : muette (un CDDU) pour l'artiste ; a dire pour le permanent, et seulement une nature de permanent.
#[test]
fn la_nature_d_un_contrat_de_permanent_se_demande_apres_l_import() {
    let racine = neuve("nature");
    deposer(&racine);
    reussi(&route(&racine, "importer", json!({"dossier": ""})));
    let salaries = salaries_lus(&racine);
    let cle = |nir: &str| salaries.as_object().unwrap().iter().find(|(_, s)| s["nir"] == json!(nir)).map(|(k, _)| k.clone()).unwrap();
    let (artiste, permanent) = (cle("2881175999001"), cle("1850799999123"));
    let engagements = || contrats_importes(&racine);
    for e in engagements() {
        if e["salarie"] == json!(artiste) {
            assert!(e.get("nature").is_none() && e.get("nature_a_dire").is_none(), "{}", e);
        }
    }
    let etat = reussi(&route(&racine, "importation-etat", json!({}))).clone();
    let a_dire = etat["a_dire"].as_array().unwrap();
    assert_eq!(a_dire.len(), 1, "{}", etat["a_dire"]);
    assert_eq!(a_dire[0]["salarie"], json!(permanent));
    let natures: BTreeSet<String> = a_dire[0]["natures"].as_array().unwrap().iter().map(|n| n["cle"].as_str().unwrap().to_string()).collect();
    assert!(natures.contains("cdi") && natures.contains("cdd") && !natures.contains("cddu_spectacle"), "{:?}", natures);

    // Une nature qui n'est pas d'un permanent, un salarie sans contrat a dire : refus, rien n'est ecrit.
    let avant = engagements_ecrits(&racine);
    assert_eq!(route(&racine, "importation-nature", json!({"salarie": permanent, "nature": "cddu_spectacle"}))["ok"], json!(false));
    assert_eq!(route(&racine, "importation-nature", json!({"salarie": artiste, "nature": "cdi"}))["ok"], json!(false));
    assert_eq!(engagements_ecrits(&racine), avant);

    // Dire la nature d'un permanent, c'est dire que la compagnie emploie des salaries au mois : la case s'en coche.
    assert_ne!(entreprise_lue(&racine)["emploie_des_permanents"], json!(true));
    let r = reussi(&route(&racine, "importation-nature", json!({"salarie": permanent, "nature": "cdi"}))).clone();
    assert!(r["ecrits"].as_u64().unwrap() >= 1, "{}", r);
    assert_eq!(entreprise_lue(&racine)["emploie_des_permanents"], json!(true));
    assert_eq!(r["a_dire"], json!([]));
    for e in engagements() {
        if e["salarie"] == json!(permanent) {
            assert_eq!((e["nature"].clone(), e.get("nature_a_dire").cloned()), (json!("cdi"), None), "{}", e);
        }
    }
    assert_eq!(reussi(&route(&racine, "importation-etat", json!({})))["a_dire"], json!([]));
}

/// Le contrat d'un permanent paye le dernier mois repris continue : sa nature se dit avec la date de debut et le numero
/// que la DSN de l'ancien logiciel lui a donnes (S21.G00.40.001 et .009, `norme-dsn/texte/bloc-S21.G00.40.txt` ; un
/// numero ne se recalcule pas, regle 4). Sans eux, ou avec un debut que les bulletins dementent : refus, rien n'est
/// ecrit. Dits, le contrat se reprend au mois suivant, avec sa nature, son debut et son numero.
#[test]
fn le_contrat_d_un_permanent_qui_continue_se_reprend_au_mois_suivant() {
    let racine = neuve("continue");
    let import = racine.join("import");
    std::fs::create_dir_all(&import).unwrap();
    pdf(vec![page_a(&permanent())], &import.join("permanent.pdf"));
    reussi(&route(&racine, "importer", json!({"dossier": ""})));
    let etat = reussi(&route(&racine, "importation-etat", json!({}))).clone();
    let x = &etat["a_dire"][0];
    let cle = x["salarie"].as_str().unwrap().to_string();
    assert_eq!((x["en_cours"].clone(), x["suite"].clone()), (json!(true), json!("2025-06")), "{}", x);
    let demande = |n: &str| x["natures"].as_array().unwrap().iter().find(|v| v["cle"] == json!(n)).unwrap()["demande"].clone();
    assert_eq!(demande("cdi"), json!(["debut", "numero", "salaire", "quotite"]));
    assert_eq!(demande("cdd"), json!(["debut", "fin", "numero", "salaire", "quotite"]));

    let avant = engagements_ecrits(&racine);
    for charge in [
        json!({"salarie": cle, "nature": "cdi"}),
        json!({"salarie": cle, "nature": "cdi", "debut": "2024-01-15"}),
        json!({"salarie": cle, "nature": "cdi", "debut": "2025-05-20", "numero": "00042", "salaire": "2400", "quotite": "151,67"}),
        json!({"salarie": cle, "nature": "cdd", "debut": "2024-01-15", "numero": "00042", "salaire": "2400", "quotite": "151,67"}),
        json!({"salarie": cle, "nature": "cdd", "debut": "2024-01-15", "fin": "2025-05-01", "numero": "00042", "salaire": "2400", "quotite": "151,67"}),
        json!({"salarie": cle, "nature": "cdi", "debut": "2024-01-15", "numero": "00042"}),
        json!({"salarie": cle, "nature": "cdi", "debut": "2024-01-15", "numero": "00042", "salaire": "0", "quotite": "151.67"}),
        json!({"salarie": cle, "nature": "cdi", "debut": "2024-01-15", "numero": "00042", "salaire": "2400"}),
    ] {
        assert_eq!(route(&racine, "importation-nature", charge.clone())["ok"], json!(false), "{}", charge);
    }
    assert_eq!(engagements_ecrits(&racine), avant);
    assert!(!racine.join("config/entreprise.json").exists() || entreprise_lue(&racine)["emploie_des_permanents"] != json!(true));

    reussi(&route(&racine, "importation-nature", json!({"salarie": cle, "nature": "cdi", "debut": "15/01/2024", "numero": "00042", "salaire": "2 400,50", "quotite": "151,67"})));
    let r = reussi(&route(&racine, "contrats-en-cours", json!({"mois": "2025-06"}))).clone();
    let c = r["contrats"].as_array().unwrap().iter().find(|c| c["salarie"] == json!(cle)).unwrap_or_else(|| panic!("{}", r)).clone();
    assert_eq!((c["debut"].clone(), c["numero"].clone(), c["engagement"]["nature"].clone()), (json!("2024-01-15"), json!("00042"), json!("cdi")), "{}", c);
    assert_eq!((c["engagement"]["salaire_mensuel"].clone(), c["engagement"]["quotite_contrat"].clone()), (json!(2400.5), json!(151.67)), "{}", c);
    // Le mois repris lui-meme ne le reprend pas : il est dans l'historique.
    let mai = reussi(&route(&racine, "contrats-en-cours", json!({"mois": "2025-05"}))).clone();
    assert!(mai["contrats"].as_array().unwrap().iter().all(|c| c["salarie"] != json!(cle)), "{}", mai);
}

/// LE PASSE FAIT PAR PUPPETPAIE SE REIMPORTE comme celui d'un autre logiciel, et UN CONTRAT D'USAGE QUI Y A COMMENCE
/// CONTINUE ICI. Le bulletin et la DSN que le banc sort (lu par `bulletin_lu`, la
/// DSN relue par `importation`), la DSN declarant le contrat jusqu'au 03/12 (S21.G00.40.010), font sur une compagnie
/// neuve un engagement importe : le numero que la DSN deposee lui a donne (S21.G00.40.009, regle 4), ce que le bulletin
/// imprime, compte dans les cumuls des annexes 8 et 10 ; aucun geste ne le modifie, et rien n'est ecrit. Decembre ne
/// s'importe pas : le contrat s'y reprend, sous le meme numero et les memes dates (`norme-dsn/texte/bloc-S21.G00.40.txt`),
/// sa DSN declare la quotite du contrat entier (S21.G00.40.013), et son bulletin suit le numero du bulletin importe.
#[test]
fn le_contrat_d_usage_reimporte_de_puppetpaie_continue_au_mois_suivant() {
    let d = dossier_de_test("import_neuve", "puppetpaie");
    let (_, banc) = crate::outils::rejeu::rejouer("treteaux-du-banc", &d.join("banc"));
    let racine = ajouter_dans(&d.join("reprise"), "Les Tréteaux du Banc").unwrap().racine;
    let import = racine.join("import");
    std::fs::create_dir_all(&import).unwrap();
    let novembre = banc.join("sorties/2026-11");
    std::fs::copy(novembre.join("envoi/Bulletin_FULLER_Novembre2026.pdf"), import.join("Bulletin_FULLER_Novembre2026.pdf")).unwrap();
    let dsn = std::fs::read(novembre.join("DSN_Novembre2026.dsn")).unwrap();
    // Le taux que la DGFiP a transmis a l'ancien logiciel (S21.G00.50.007 = 01, son identifiant en .008).
    let fin = if dsn.contains(&b'\r') { "\r\n" } else { "\n" };
    let dsn = String::from_utf8_lossy(&dsn)
        .replace("S21.G00.40.010,'21112026'", "S21.G00.40.010,'03122026'")
        .replace("S21.G00.50.007,'13'", &format!("S21.G00.50.007,'01'{}S21.G00.50.008,'123456789'", fin));
    assert!(dsn.contains("S21.G00.50.008,'123456789'"), "{}", dsn);
    std::fs::write(import.join("DSN_Novembre2026.dsn"), dsn.as_bytes()).unwrap();
    let jour = "2026-12-20";
    let route = |nom: &str, charge: Value| crate::outils::route(&racine, nom, charge, jour);
    reussi(&route("importer", json!({"dossier": ""})));

    // La licence que la DSN declare (S21.G00.40.049) devient le numero de la fiche, celui que le bulletin imprime.
    let licence = dsn.lines().find_map(|l| l.strip_prefix("S21.G00.40.049,'")).map(|l| l.trim_end_matches(['\'', '\r'])).unwrap();
    assert_eq!(entreprise_lue(&racine)["licence_spectacle"], json!(licence));

    let importes = engagement::importe::tous(&racine).unwrap();
    assert_eq!(importes.len(), 1, "{:?}", importes);
    let i = &importes[0];
    assert_eq!((i.objet().brut["numero"].clone(), i.objet().brut["fin"].clone()), (json!("FU1.26.01"), json!("2026-12-03")));
    let paie = i.paies()["2026-11"].clone();
    assert_eq!((paie["declare_par"].clone(), paie["net_a_payer"].clone()), (json!("DSN"), json!("232.30")), "{:?}", paie);
    assert!(paie["bulletin"].is_object(), "le bulletin ne complete pas la paie de la DSN : {:?}", paie);

    let avant = engagements_ecrits(&racine);
    let fuller = json!({"mois": "2026-11", "salarie": "FULLER", "debut": "2026-11-20"});
    assert_eq!(route("valider-engagement", json!({"mois": "2026-11", "salarie": "FULLER", "debut": "2026-11-20", "valide": false}))["ok"], json!(false));
    assert_eq!(route("retirer-engagement", fuller)["ok"], json!(false));
    assert_eq!(engagements_ecrits(&racine), avant);

    // Decembre se paie ici : le contrat s'y reprend avec son numero et ses dates.
    for f in ["entreprise.json", "organismes.json", "baremes/2026.json"] {
        let cible = racine.join("config").join(f);
        std::fs::create_dir_all(cible.parent().unwrap()).unwrap();
        std::fs::copy(banc.join("config").join(f), cible).unwrap();
    }
    assert_eq!(reussi(&route("saisie", json!({"mois": "2026-12"})))["importe"], Value::Null);
    let r = reussi(&route("contrats-en-cours", json!({"mois": "2026-12"}))).clone();
    let c = &r["contrats"][0];
    assert_eq!((c["debut"].clone(), c["fin"].clone(), c["numero"].clone()), (json!("2026-11-20"), json!("2026-12-03"), json!("FU1.26.01")), "{}", r);
    let mut e = c["engagement"].clone();
    e["lignes"] = json!([{"rubrique": "cachet", "quantite": 2, "taux": 150, "dates": ["2026-12-02", "2026-12-03"]}]);
    let saisie = json!({"mois": "2026-12", "taux_cachet": 150, "date_paiement": "2026-12-31", "engagements": [e]});
    reussi(&route("enregistrer-saisie", json!({"mois": "2026-12", "saisie": saisie, "forcer": false, "modifier_apres_echeance": false})));
    reussi(&route("valider-engagement", json!({"mois": "2026-12", "salarie": "FULLER", "debut": "2026-11-20", "valide": true})));
    // Le contrat a ete signe dans l'ancien logiciel : il ne se refait pas signer ici.
    let signature = reussi(&route("signature-apercu", json!({"mois": "2026-12"}))).clone();
    let a_signer: Vec<&Value> = signature["contrats"].as_array().into_iter().flatten().filter(|c| c["salarie"] == "FULLER").collect();
    assert!(a_signer.is_empty(), "le contrat repris se fait signer : {:?}", a_signer);
    // Le bulletin de decembre suit le 1/2026 que le bulletin importe imprime : deux bulletins de l'annee n'ont jamais le
    // meme numero.
    let apercu = reussi(&route("apercu", json!({"saisie": saisie, "lignes": false, "urssaf": false}))).clone();
    assert_eq!(apercu["paies"][0]["numero_bulletin"], json!("2/2026"), "{}", apercu["paies"][0]);
    let ecrite = reussi(&route("dsn-a-deposer", json!({"mois": "2026-12"}))).clone();
    let decembre = std::fs::read(ecrite["chemin"].as_str().unwrap()).unwrap();
    let decembre = String::from_utf8_lossy(&decembre);
    // Le taux transmis que la DSN reprise appliquait vaut encore en decembre (CGI, art. 1671, 2) : il continue.
    for attendu in ["S21.G00.40.001,'20112026'", "S21.G00.40.009,'FU1.26.01'", "S21.G00.40.013,'4.00'", "S21.G00.50.007,'01'", "S21.G00.50.008,'123456789'"] {
        assert!(decembre.contains(attendu), "{} manque :\n{}", attendu, decembre);
    }
    let ctx = droit::contexte::Contexte::nouveau(&racine, None).unwrap();
    let declares = engagement::cumul::lire_engagements(&ctx, None).unwrap();
    assert_eq!(declares.iter().map(|e| e.cachets).sum::<i64>(), 4, "{:?}", declares);
}

/// LE MOIS MIXTE : une saisie posee sur novembre avant l'import, puis l'import du bulletin et de la DSN de novembre que
/// l'ancien logiciel a faits. Le bulletin complete la paie que la DSN deposee declare (la saisie ne l'en empeche pas) ;
/// la DSN de novembre ne se refait pas ici (regle 4) ; l'engagement saisi se declare dans la DSN suivante, comme une
/// correction (« Il donne lieu a la declaration d'une correction dans la DSN suivante »,
/// `atelier/reference/norme-dsn/texte/chapitre-2-principes-de-constitution.txt`), qui ne fabrique aucun bulletin : il a
/// un seul bulletin, celui des documents de novembre, numerote apres le bulletin importe.
#[test]
fn le_mois_saisi_puis_importe_declare_la_saisie_dans_la_dsn_suivante() {
    let d = dossier_de_test("import_neuve", "mixte");
    let (_, banc) = crate::outils::rejeu::rejouer("treteaux-du-banc", &d.join("banc"));
    let racine = ajouter_dans(&d.join("mixte"), "Les Tréteaux du Banc").unwrap().racine;
    for f in ["entreprise.json", "organismes.json", "pas.json", "baremes/2026.json", "salaries.json", "spectacles.json"] {
        let cible = racine.join("config").join(f);
        std::fs::create_dir_all(cible.parent().unwrap()).unwrap();
        std::fs::copy(banc.join("config").join(f), cible).unwrap();
    }
    let jour = "2026-12-20";
    let route = |nom: &str, charge: Value| crate::outils::route(&racine, nom, charge, jour);
    let ligne = json!([{"rubrique": "cachet", "quantite": 1, "taux": 150, "dates": ["2026-11-27"]}]);
    let saisie = json!({"mois": "2026-11", "taux_cachet": 150, "date_paiement": "2026-11-30", "engagements": [
        {"salarie": "FULLER", "spectacle": "le-banc-des-menteurs", "lignes": ligne}]});
    reussi(&route("enregistrer-saisie", json!({"mois": "2026-11", "saisie": saisie, "forcer": false, "modifier_apres_echeance": false})));

    let import = racine.join("import");
    std::fs::create_dir_all(&import).unwrap();
    for f in ["envoi/Bulletin_FULLER_Novembre2026.pdf", "DSN_Novembre2026.dsn"] {
        let source = banc.join("sorties/2026-11").join(f);
        std::fs::copy(&source, import.join(source.file_name().unwrap())).unwrap();
    }
    reussi(&route("importer", json!({"dossier": ""})));
    let importes = engagement::importe::tous(&racine).unwrap();
    assert_eq!(importes.len(), 1, "{:?}", importes);
    let paie = importes[0].paies()["2026-11"].clone();
    assert_eq!(paie["declare_par"], json!("DSN"), "{:?}", paie);
    assert_eq!(paie["bulletin"]["numero"], json!("1/2026"), "le bulletin ne complete pas la paie de la DSN : {:?}", paie);

    // La page Paie et la page du mois disent la DSN de l'ancien logiciel avec une seule phrase, celle du moteur.
    let etat = reussi(&route("etat-du-mois", json!({"mois": "2026-11"}))).clone();
    let dsn = etat["etapes"].as_array().unwrap().iter().find(|e| e["cle"] == json!("dsn")).expect("l'étape DSN");
    let case = dsn["groupes"][0]["phrase"].clone();
    let avis = reussi(&route("saisie", json!({"mois": "2026-11"})))["dsn_deposee"].clone();
    assert!(case.is_string() && avis["phrase"] == case && avis["confirmer"] == json!(false), "{} / {}", case, avis);

    reussi(&route("valider-engagement", json!({"mois": "2026-11", "salarie": "FULLER", "debut": "2026-11-27", "valide": true})));

    // Le mois porte les deux engagements, dans les cumuls des annexes 8 et 10 comme au livre de paie : l'importe avec ce
    // que ses pieces attestent, le saisi avec sa paie validee.
    let ctx = droit::contexte::Contexte::nouveau(&racine, Some(socle::date::Jour::lire(jour).unwrap())).unwrap();
    let cumules: BTreeSet<(String, i64)> =
        engagement::cumul::lire_engagements(&ctx, None).unwrap().iter().map(|e| (e.debut.iso(), e.cachets)).collect();
    assert_eq!(cumules, BTreeSet::from([("2026-11-20".to_string(), 2), ("2026-11-27".to_string(), 1)]));
    let livre = reussi(&route("exporter", json!({"etat": "livre-de-paie", "mois": "2026-11", "apercu": true}))).clone();
    let contrats: BTreeSet<String> =
        livre["groupes"].as_array().unwrap().iter().flat_map(|g| g["lignes"].as_array().unwrap().iter().map(|l| l["contrat"].as_str().unwrap_or("").to_string())).collect();
    assert_eq!(contrats, BTreeSet::from(["FU1.26.01".to_string(), "FU1.26.02".to_string()]), "{}", livre);

    assert_eq!(route("dsn-a-deposer", json!({"mois": "2026-11"}))["ok"], json!(false));
    assert!(!racine.join("sorties/2026-11/DSN_Novembre2026.dsn").exists(), "la DSN de l'ancien logiciel s'est refaite");
    let ecrite = reussi(&route("dsn-a-deposer", json!({"mois": "2026-12"}))).clone();
    let decembre = std::fs::read(ecrite["chemin"].as_str().unwrap()).unwrap();
    let decembre = String::from_utf8_lossy(&decembre);
    for attendu in ["S21.G00.40.001,'27112026'", "S21.G00.40.009,'FU1.26.02'"] {
        assert!(decembre.contains(attendu), "{} manque :\n{}", attendu, decembre);
    }
    let pdfs = |dossier: &str| -> Vec<String> {
        std::fs::read_dir(racine.join(dossier)).into_iter().flatten().flatten().map(|f| f.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".pdf")).collect()
    };
    assert!(pdfs("sorties/2026-12").is_empty(), "la DSN a fabrique un bulletin : {:?}", pdfs("sorties/2026-12"));

    reussi(&route("documents-imprimer", json!({"mois": "2026-11"})));
    let e = lire(&racine, "engagements/2026-11.json");
    let saisi = e["engagements"].as_array().unwrap().iter().find(|x| x["numero"] == json!("FU1.26.02")).expect("l'engagement saisi").clone();
    let p = &saisi["valide"]["paies"]["2026-11"];
    assert_eq!((&p["bulletin"], &p["rectifie"]), (&json!("2/2026"), &Value::Null), "{}", p);
    assert!(pdfs("sorties/2026-12").is_empty(), "{:?}", pdfs("sorties/2026-12"));
}
