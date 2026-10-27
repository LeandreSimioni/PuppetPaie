//! LA DSN ECRITE D'UN MOIS (#236) : le rappel de la DSN et l'etape DSN de la page Paie la lisent au journal des
//! ecritures (`depart::mois_ecrit`), jamais d'apres un fichier pose dans `sorties/<mois>/` ; la paie changee depuis
//! l'ecriture ne se lit que de l'empreinte notee. Sur `fictive-3090-petite`.

use crate::outils::{compagnie, dossier_de_test, ecrire, lire, reussi, route};
use serde_json::json;

/// UN SEUL CRITERE POUR « LA DSN EST ECRITE » (#236) : le rappel de la DSN et l'etape DSN de la page Paie lisent
/// `depart::mois_ecrit` : la DSN mensuelle notee au journal
/// des ecritures, et toujours dans `sorties/<mois>/`. Un fichier DSN pose sans que le moteur l'ait note n'ecrit pas le
/// mois ; fichier et note qui concordent l'ecrivent ; une note dont le fichier a disparu ne l'ecrit plus. Sur
/// `fictive-3090-petite`, dont octobre 2026 est saisi et pas ecrit.
#[test]
fn le_rappel_et_la_page_paie_lisent_la_dsn_ecrite_au_journal() {
    let c = compagnie::ses_dsn("fictive-3090-petite", &[], &dossier_de_test("dsn_ecrite", "un-critere").join("compagnie"));
    const LE: &str = "2026-11-03";
    let rappel = |c: &std::path::Path| {
        let r = reussi(&route(c, "rappels", json!({}), LE)).clone();
        r["rappels"].as_array().unwrap().iter().find(|x| x["cle"] == json!("dsn:2026-10")).cloned().unwrap_or_else(|| panic!("le rappel de la DSN d'octobre : {}", r))
    };
    let etape = |c: &std::path::Path| reussi(&route(c, "bilan-du-mois", json!({"mois": "2026-10"}), LE))["dsn"]["etape"].clone();

    // Un fichier DSN pose a la main, que le moteur n'a pas note : le mois n'est pas ecrit, nulle part.
    let mut octets = std::fs::read(c.join("sorties/2026-09/DSN_Septembre2026.dsn")).unwrap();
    octets.extend_from_slice(b"\r\n");
    std::fs::create_dir_all(c.join("sorties/2026-10")).unwrap();
    std::fs::write(c.join("sorties/2026-10/DSN_Octobre2026.dsn"), &octets).unwrap();
    assert_ne!(rappel(&c)["etat"], json!("a_deposer"));
    assert!(etape(&c).is_null(), "{}", etape(&c));

    // Le fichier et sa note concordent : ecrit, partout.
    let mut journal = lire(&c, "journal/dsn-ecrites.json");
    journal["2026-10/DSN_Octobre2026.dsn"] = json!({"quand": "2026-11-02T10:00:00"});
    ecrire(&c, "journal/dsn-ecrites.json", &journal);
    let r = rappel(&c);
    assert_eq!(r["etat"], json!("a_deposer"), "{}", r);
    assert!(r["fichier"].as_str().unwrap().ends_with("DSN_Octobre2026.dsn"), "{}", r);
    assert!(!etape(&c).is_null(), "l'etape DSN dit ou en est la DSN ecrite");

    // La note reste, le fichier est parti : plus rien a deposer.
    std::fs::remove_file(c.join("sorties/2026-10/DSN_Octobre2026.dsn")).unwrap();
    assert_ne!(rappel(&c)["etat"], json!("a_deposer"));
    assert!(etape(&c).is_null());
}

/// LES DATES DES FICHIERS NE DISENT PAS QUE LA PAIE A CHANGE (#236). Une fictive livree, decompressee chez un visiteur,
/// a des dates de fichier quelconques : ses DSN peuvent paraitre plus vieilles que ses saisies. « La paie a change depuis
/// l'ecriture » ne se lit que de l'empreinte de la saisie notee au journal des ecritures (`depart::fabriquer_dsn`) ;
/// sans empreinte, le moteur ne sait pas et ne le dit pas. Sur une copie de `fictive-3090-petite` (rien de depose, pour
/// que chaque mois montre son etape), DSN datees de 2020, saisies d'aujourd'hui.
#[test]
fn des_dates_de_fichier_touchees_ne_font_changer_aucune_paie() {
    let c = compagnie::ses_dsn("fictive-3090-petite", &[], &dossier_de_test("dsn_ecrite", "dates-touchees").join("compagnie"));
    ecrire(&c, "journal/depots.json", &json!([]));
    let vieux = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_577_836_800); // 2020-01-01
    let toucher = |p: &std::path::Path, t: std::time::SystemTime| std::fs::File::options().write(true).open(p).unwrap().set_modified(t).unwrap();
    let mut mois: Vec<String> = Vec::new();
    for e in std::fs::read_dir(c.join("sorties")).unwrap().flatten().filter(|e| e.path().is_dir()) {
        for f in std::fs::read_dir(e.path()).unwrap().flatten().filter(|f| f.file_name().to_string_lossy().starts_with("DSN_")) {
            toucher(&f.path(), vieux);
            mois.push(e.file_name().to_string_lossy().into_owned());
        }
    }
    for e in std::fs::read_dir(c.join("engagements")).unwrap().flatten().filter(|e| e.path().is_file()) {
        toucher(&e.path(), std::time::SystemTime::now());
    }
    assert!(mois.len() > 10, "{:?}", mois);
    let changees = |c: &std::path::Path| -> Vec<String> {
        mois.iter()
            .filter(|m| reussi(&route(c, "bilan-du-mois", json!({"mois": m}), "2026-10-03"))["dsn"]["etape"] == json!("paie_changee"))
            .cloned()
            .collect()
    };
    let etape = |m: &str| reussi(&route(&c, "bilan-du-mois", json!({"mois": m}), "2026-10-03"))["dsn"]["etape"].clone();
    assert!(mois.iter().all(|m| !etape(m).is_null()), "chaque mois montre l'etape de sa DSN ecrite");
    assert_eq!(changees(&c), Vec::<String>::new(), "les empreintes notees concordent : aucun mois n'a change");
    // Un journal dont les notes n'ont pas d'empreinte : le moteur ne sait pas, il ne dit pas que la paie a change.
    let mut journal = lire(&c, "journal/dsn-ecrites.json");
    for note in journal.as_object_mut().unwrap().values_mut() {
        note.as_object_mut().unwrap().remove("empreinte_saisie");
    }
    ecrire(&c, "journal/dsn-ecrites.json", &journal);
    assert_eq!(changees(&c), Vec::<String>::new(), "sans empreinte, les dates des fichiers ne disent rien");
}

/// L'EXPORT COMPTA DE L'ANNEE EST A JOUR D'APRES CE QUE LE MOTEUR A CONSIGNE (#236), pas d'apres les dates des fichiers :
/// un seul critere (`exercice_ecrit::export_compta_a_jour`) pour le rappel « Exporter les chiffres » et pour l'horloge
/// d'une fictive. L'export note au journal des ecritures les empreintes des saisies qu'il a lues ; il est perime quand
/// une saisie de l'annee n'est plus celle-la. Sur une copie de `fictive-3090-petite` (son exercice 2025 rouvert pour que
/// le rappel puisse parler), l'export date de 2020 et les saisies d'aujourd'hui.
#[test]
fn l_export_compta_est_a_jour_d_apres_le_journal_et_non_les_dates() {
    let c = compagnie::ses_dsn("fictive-3090-petite", &["exercices"], &dossier_de_test("dsn_ecrite", "export-compta").join("compagnie"));
    ecrire(&c, "config/exercices.json", &json!({}));
    let export = c.join("sorties/exercices/Compta_2025.csv");
    let vieux = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_577_836_800); // 2020-01-01
    let toucher = |p: &std::path::Path, t: std::time::SystemTime| std::fs::File::options().write(true).open(p).unwrap().set_modified(t).unwrap();
    let dater = || {
        toucher(&export, vieux);
        for e in std::fs::read_dir(c.join("engagements")).unwrap().flatten().filter(|e| e.path().is_file()) {
            toucher(&e.path(), std::time::SystemTime::now());
        }
    };
    let rappel = |c: &std::path::Path| {
        reussi(&route(c, "rappels", json!({}), "2026-01-20"))["rappels"].as_array().unwrap().iter().any(|x| x["cle"] == json!("export-compta:2025"))
    };
    // L'export livre, que le journal ne decrit pas : le moteur ne sait pas que les saisies ont change, il se tait.
    dater();
    assert!(!rappel(&c), "un export plus vieux que les saisies par ses dates n'est pas perime pour autant");
    // Refait : il note les saisies lues ; les dates touchees n'y changent rien.
    reussi(&route(&c, "export-compta", json!({"annee": "2025"}), "2026-01-20"));
    let note = &lire(&c, "journal/dsn-ecrites.json")["exercices/Compta_2025.csv"]["empreinte_saisie"];
    assert_eq!(note.as_object().map(|o| o.len()), Some(12), "les douze saisies de 2025 : {}", note);
    dater();
    assert!(!rappel(&c));
    // Une saisie de 2025 change : l'export est perime, le rappel revient.
    let mut mars = crate::outils::saisie(&c, "2025-03");
    mars["date_paiement"] = json!("2025-03-28");
    crate::outils::poser_saisie(&c, "2025-03", &mars);
    assert!(rappel(&c), "la saisie de mars n'est plus celle que l'export a lue");
}

