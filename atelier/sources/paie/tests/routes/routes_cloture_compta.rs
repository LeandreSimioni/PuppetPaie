//! La cloture de l'exercice (archive relue octet pour octet, reouverture) et la comptabilite : `export-compta` (le PAS au
//! 442100 egal a l'etat du PAS, l'equilibre avec et sans PAS, l'acompte au 425), `retenir-logiciel`, `exporter` en
//! apercu. Sur `fictive-3090-petite` entiere, sans les archives de ses exercices.

use crate::outils::{compagnie, dossier_de_test, lire, refus, reussi};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};

fn compagnie(nom: &str) -> PathBuf {
    let racine = compagnie::entiere("fictive-3090-petite", &dossier_de_test("routes_cloture_compta", nom).join("compagnie"));
    std::fs::remove_dir_all(racine.join("sorties/exercices")).unwrap();
    racine
}

fn route(racine: &Path, nom: &str, charge: Value) -> Value {
    crate::outils::route(racine, nom, charge, "2026-10-03")
}

/// Les entrees d'une archive zip : (nom, contenu), dans l'ordre ou elles sont ecrites.
fn entrees_du_zip(octets: &[u8]) -> Vec<(String, Vec<u8>)> {
    let u16le = |o: usize| u16::from_le_bytes([octets[o], octets[o + 1]]) as usize;
    let u32le = |o: usize| u32::from_le_bytes([octets[o], octets[o + 1], octets[o + 2], octets[o + 3]]) as usize;
    let fin = (0..octets.len() - 3).rev().find(|&i| octets[i..i + 4] == [0x50, 0x4b, 0x05, 0x06]).expect("fin de l'archive");
    let (n, mut o) = (u16le(fin + 10), u32le(fin + 16));
    let mut out = Vec::new();
    for _ in 0..n {
        assert_eq!(&octets[o..o + 4], &[0x50, 0x4b, 0x01, 0x02]);
        let (methode, comp, nom_len, extra, comm, local) = (u16le(o + 10), u32le(o + 20), u16le(o + 28), u16le(o + 30), u16le(o + 32), u32le(o + 42));
        let nom = String::from_utf8(octets[o + 46..o + 46 + nom_len].to_vec()).unwrap();
        let debut = local + 30 + u16le(local + 26) + u16le(local + 28);
        let brut = &octets[debut..debut + comp];
        let contenu = match methode {
            0 => brut.to_vec(),
            8 => {
                let mut v = Vec::new();
                flate2::read::DeflateDecoder::new(brut).read_to_end(&mut v).unwrap();
                v
            }
            m => panic!("methode {}", m),
        };
        out.push((nom, contenu));
        o += 46 + nom_len + extra + comm;
    }
    out
}

#[test]
fn la_cloture_ecrit_une_archive_fidele_et_se_rouvre() {
    let racine = compagnie("cloture");
    // La compagnie fictive est deja close sur 2025 : on la rouvre d'abord.
    let r = route(&racine, "cloture", json!({"annee": "2025", "rouvrir": true}));
    assert_eq!(reussi(&r)["annee"], json!("2025"), "{}", r);
    assert!(lire(&racine, "config/exercices.json").get("2025").is_none());
    assert_eq!(lire(&racine, "config/exercices.json")["_rouverts"].as_array().unwrap().len(), 1);
    // La compagnie fictive a des DSN reelles au journal ; sans elles : refus, sauf `forcer`.
    std::fs::write(racine.join("journal").join("depots.json"), "[]").unwrap();
    let (_, m) = refus(&route(&racine, "cloture", json!({"annee": "2025", "forcer": false})));
    assert!(m.contains("Pas de DSN réelle acceptée"), "{}", m);
    assert!(!racine.join("sorties").join("exercices").join("Exercice_2025.zip").exists());
    let r = route(&racine, "cloture", json!({"annee": "2025", "forcer": true}));
    let c = reussi(&r);
    let archive = racine.join("sorties").join("exercices").join("Exercice_2025.zip");
    let octets = std::fs::read(&archive).unwrap();
    // L'empreinte et la taille rendues sont celles de l'archive ecrite, et celles d'exercices.json.
    assert_eq!(c["empreinte"], json!(format!("{:x}", Sha256::digest(&octets))));
    assert_eq!(c["octets"], json!(octets.len()));
    let entrees = entrees_du_zip(&octets);
    assert_eq!(c["fichiers"], json!(entrees.len()));
    assert_eq!(c["manques"], json!([]));
    let noms: Vec<&str> = entrees.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(noms[0], "engagements/2025-01.json");
    assert_eq!(*noms.last().unwrap(), "LISEZ-MOI.txt");
    assert!(noms.contains(&"compta/Compta_2025.csv") && noms.contains(&"journal/depots.json") && noms.contains(&"config/baremes/2025.json"));
    // Chaque entree est le fichier lui-meme, octet pour octet.
    for (nom, contenu) in &entrees {
        if nom == "LISEZ-MOI.txt" {
            assert!(!contenu.is_empty(), "le mot qui accompagne l'archive");
        } else if let Some(c) = nom.strip_prefix("compta/") {
            assert_eq!(&std::fs::read(racine.join("sorties").join("exercices").join(c)).unwrap(), contenu, "{}", nom);
        } else if nom == "journal/depots.json" {
            assert_eq!(&std::fs::read(racine.join("journal").join("depots.json")).unwrap(), contenu);
        } else {
            assert_eq!(&std::fs::read(racine.join(nom)).unwrap(), contenu, "{}", nom);
        }
    }
    let e = &lire(&racine, "config/exercices.json")["2025"];
    assert_eq!((e["empreinte"].clone(), e["octets"].clone(), e["fichiers"].clone()), (c["empreinte"].clone(), c["octets"].clone(), c["fichiers"].clone()));
    assert_eq!(e["forcee"], json!(true));
    assert_eq!(e["sans_dsn"].as_array().unwrap().len(), 12);
    assert_eq!(e["qui"], json!("interface"));
    // Deja close, annee en cours, annee non close a rouvrir : refus.
    assert!(refus(&route(&racine, "cloture", json!({"annee": "2025"}))).1.contains("déjà clos"));
    assert!(refus(&route(&racine, "cloture", json!({"annee": "2026"}))).1.contains("n'est pas fini"));
    assert!(refus(&route(&racine, "cloture", json!({"annee": "2024", "rouvrir": true}))).1.contains("n'est pas clos"));
    // Rouvrir : l'archive reste, la reponse dit depuis quand.
    let r = route(&racine, "cloture", json!({"annee": 2025, "rouvrir": true}));
    assert_eq!(reussi(&r)["archive"], json!("sorties/exercices/Exercice_2025.zip"), "{}", r);
    assert!(archive.exists());
}

/// « 23.51 » en centimes.
fn centimes(t: &str) -> i64 {
    let (e, c) = t.split_once('.').unwrap_or((t, "0"));
    let signe = if e.starts_with('-') { -1 } else { 1 };
    signe * (e.trim_start_matches('-').parse::<i64>().unwrap() * 100 + format!("{:0<2}", c)[..2].parse::<i64>().unwrap())
}

/// Les ecritures d'un export : par mois, (debit, credit) de tout le mois, et le credit du 421000 et du 442100.
fn sommes_par_mois(c: &Value) -> Vec<(String, i64, i64, i64, i64, usize)> {
    let ecritures = c["ecritures"].as_array().unwrap();
    c["mois"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| {
            let du_mois: Vec<&Value> = ecritures.iter().filter(|e| e["mois"] == *m).collect();
            let somme = |champ: &str, compte: Option<&str>| -> i64 {
                du_mois.iter().filter(|e| compte.map_or(true, |k| e["compte"] == json!(k))).map(|e| centimes(e[champ].as_str().unwrap())).sum()
            };
            assert_eq!(somme("debit", Some("442100")), 0, "{} : du PAS au debit", m);
            let lignes_pas = du_mois.iter().filter(|e| e["compte"] == json!("442100")).count();
            (m.as_str().unwrap().to_string(), somme("debit", None), somme("credit", None), somme("credit", Some("421000")), somme("credit", Some("442100")), lignes_pas)
        })
        .collect()
}

#[test]
fn l_export_compta_porte_le_prelevement_a_la_source_et_s_equilibre_avec_et_sans() {
    let racine = compagnie("compta");
    // AVEC PAS : la compagnie fictive a des taux DGFiP (config/pas.json) ; chaque mois de paie en retient.
    let r = route(&racine, "export-compta", json!({"annee": "2025", "apercu": true}));
    let c = reussi(&r);
    assert!(c["fichier"].is_null(), "un apercu n'ecrit rien : {}", r);
    assert!(!racine.join("sorties").join("exercices").join("Compta_2025.csv").exists());
    assert_eq!(c["total_debit"], c["total_credit"], "{}", r);
    // Le PAS de chaque mois est celui de l'etat « prelevement a la source » (le meme calcul que le bulletin).
    let etat = route(&racine, "exporter", json!({"etat": "prelevement-a-la-source", "annee": 2025, "apercu": true}));
    let preleve: std::collections::BTreeMap<String, i64> = reussi(&etat)["sections"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            let m = s["lignes"][0]["mois"].as_str().unwrap().to_string();
            (m, centimes(s["total"]["prelevement"].as_str().unwrap()))
        })
        .collect();
    let avec = sommes_par_mois(c);
    let mut mois_avec_pas = 0;
    for (m, debit, credit, _, pas, lignes_pas) in &avec {
        assert_eq!(debit, credit, "{} : l'ecriture avec PAS ne s'equilibre pas", m);
        assert_eq!(*pas, preleve.get(m).copied().unwrap_or(0), "{} : PAS de l'export et de l'etat", m);
        assert_eq!(*lignes_pas, usize::from(*pas != 0), "{} : une ligne de PAS par mois qui en retient, aucune sinon", m);
        mois_avec_pas += usize::from(*pas != 0);
    }
    assert!(mois_avec_pas > 0, "la compagnie fictive ne retient aucun PAS");
    // Des taux DGFiP a zero, recus apres coup : les mois deposes gardent le PAS que leur DSN a declare (lot D2 : il se
    // lit, il ne se recalcule pas ; CGI art. 1671).
    let chemin = racine.join("config").join("pas.json");
    let mut magasin = lire(&racine, "config/pas.json");
    for (_, e) in magasin["salaries"].as_object_mut().unwrap() {
        e["taux"] = json!("0.00");
    }
    std::fs::write(&chemin, serde_json::to_vec_pretty(&magasin).unwrap()).unwrap();
    let r = route(&racine, "export-compta", json!({"annee": "2025", "apercu": true}));
    assert_eq!(sommes_par_mois(reussi(&r)), avec, "un nouveau taux ne change pas le PAS d'un mois depose");
    // SANS PAS : les memes paies, jamais deposees, des taux DGFiP a zero. Rien au 442100, l'ecriture s'equilibre
    // encore, et le net au 421000 est le net d'avant impot : celui d'avec PAS, plus le PAS.
    std::fs::write(racine.join("journal").join("depots.json"), b"[]").unwrap();
    // Rien n'est parti : chaque mois, a son ouverture, recalcule ses paies au taux recu.
    for m in 1..=12 {
        reussi(&route(&racine, "saisie", json!({"mois": format!("2025-{:02}", m)})));
    }
    let r = route(&racine, "export-compta", json!({"annee": "2025", "apercu": true}));
    let sans = sommes_par_mois(reussi(&r));
    assert_eq!(sans.len(), avec.len());
    for ((m, debit, credit, net, pas, lignes_pas), (_, _, _, net_avec, pas_avec, _)) in sans.iter().zip(&avec) {
        assert_eq!(debit, credit, "{} : l'ecriture sans PAS ne s'equilibre pas", m);
        assert_eq!((*pas, *lignes_pas), (0, 0), "{} : du PAS sans taux", m);
        assert_eq!(*net, net_avec + pas_avec, "{} : net apres PAS + PAS = net avant", m);
    }
    // Le fichier : la ligne du PAS y est, sous son compte et son libelle.
    let racine = compagnie("compta-fichier");
    let r = route(&racine, "export-compta", json!({"annee": "2025"}));
    assert_eq!(reussi(&r)["fichier"], json!("Compta_2025.csv"), "{}", r);
    let csv = std::fs::read(racine.join("sorties").join("exercices").join("Compta_2025.csv")).unwrap();
    assert!(csv.starts_with("\u{feff}Journal;Date;Compte;Libelle;Debit;Credit;Analytique\r\n".as_bytes()));
    let texte = String::from_utf8(csv).unwrap();
    assert!(texte.ends_with("\r\n"));
    assert!(
        texte.lines().any(|l| l.starts_with("OD;31/03/2025;442100;Prelevement a la source -- Mars 2025;;") && l.ends_with(";")),
        "pas de ligne PAS en mars : {}",
        texte
    );
    // Du/au : les mois pris, et le nom du fichier.
    let r = route(&racine, "export-compta", json!({"annee": "2025", "du": 3, "au": 5}));
    assert_eq!(reussi(&r)["fichier"], json!("Compta_2025_03-05.csv"), "{}", r);
    assert_eq!(reussi(&r)["mois"], json!(["2025-03", "2025-04", "2025-05"]));
    // Rien de saisi : refus.
    let (_, m) = refus(&route(&racine, "export-compta", json!({"annee": "2024"})));
    assert!(m.contains("2024"), "le refus nomme l'annee : {}", m);
}

#[test]
fn un_acompte_retenu_solde_l_avance_au_425_et_pas_les_charges() {
    let racine = compagnie("compta-acompte");
    // Mars 2025 : un acompte de 100 EUR retenu sur la paie de BRIOCHE (rubrique livree `acompte`, nature `retenue`).
    let mut saisie = crate::outils::saisie(&racine, "2025-03");
    let e = saisie["engagements"].as_array_mut().unwrap().iter_mut().find(|e| e["salarie"] == json!("Brioche")).unwrap();
    e["lignes"].as_array_mut().unwrap().push(json!({"rubrique": "acompte", "quantite": 1, "taux": 100.0}));
    crate::outils::poser_saisie(&racine, "2025-03", &saisie);
    crate::outils::tout_valider(&racine, "2025-03", "2025-04-05");
    let r = route(&racine, "export-compta", json!({"annee": "2025", "du": 3, "au": 3, "apercu": true}));
    let c = reussi(&r);
    let ecritures = c["ecritures"].as_array().unwrap();
    let au = |compte: &str| -> Vec<&Value> { ecritures.iter().filter(|e| e["compte"] == json!(compte)).collect() };
    // L'avance deja versee se solde au credit du 425 (plan de comptes : « Personnel - Avances et acomptes »), au nom
    // du salarie ; elle ne diminue pas les charges de personnel (641400, « Indemnites et avantages divers »).
    let avances = au("425000");
    assert_eq!(avances.len(), 1, "{}", r);
    assert_eq!((avances[0]["debit"].clone(), avances[0]["credit"].clone()), (json!("0.00"), json!("100.00")), "{}", r);
    assert_eq!(avances[0]["analytique"], json!("BRIOCHE Fanchon"));
    assert!(au("641400").is_empty(), "l'acompte est passe en indemnites : {}", r);
    assert_eq!(c["total_debit"], c["total_credit"], "{}", r);
    // Une retenue sans compte au plan : refus, on ne devine pas un compte.
    std::fs::write(racine.join("config").join("plan-comptable.json"), json!({"retenues": {"acompte": null}}).to_string()).unwrap();
    let (_, m) = refus(&route(&racine, "export-compta", json!({"annee": "2025", "du": 3, "au": 3, "apercu": true})));
    assert!(m.contains("« acompte »") && m.contains("retenues"), "{}", m);
}

#[test]
fn retenir_un_logiciel_ecrit_la_config_et_refuse_l_inconnu() {
    let racine = compagnie("logiciel");
    let r = route(&racine, "retenir-logiciel", json!({"logiciel": "quadra", "analytique": true}));
    reussi(&r);
    let c = lire(&racine, "config/compta.json");
    assert_eq!(c["logiciel"], json!("quadra"));
    assert_eq!(c["analytique"], json!(true));
    let r = route(&racine, "retenir-logiciel", json!({"logiciel": "inconnu-de-nous"}));
    refus(&r);
    assert_eq!(lire(&racine, "config/compta.json")["logiciel"], json!("quadra"), "un refus n'ecrit rien");
    reussi(&route(&racine, "retenir-logiciel", json!({})));
    assert_eq!(lire(&racine, "config/compta.json")["logiciel"], json!(""));
}

#[test]
fn exporter_refuse_un_etat_inconnu_et_un_apercu_n_ecrit_pas() {
    let racine = compagnie("exporter");
    refus(&route(&racine, "exporter", json!({"etat": "etat-qui-n-existe-pas"})));
    let r = route(&racine, "exporter", json!({"etat": "masse-salariale", "annee": 2025, "apercu": true}));
    let res = reussi(&r);
    assert_eq!(res["etat"], json!("masse-salariale"), "{}", r);
    assert!(!racine.join("exports").exists(), "un apercu ne doit rien ecrire");
}
