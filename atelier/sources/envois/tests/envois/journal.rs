//! Le journal des envois n'a que des ajouts (ce qui est parti se lit, regle 4), et l'empreinte d'un fichier y reste
//! comparable a celles deja ecrites : c'est elle qui dit qu'un fichier est deja parti.

use envois::journal::{self, Note};
use serde_json::{json, Value};

use crate::outils::dossier_de_test;


fn note<'a>(quoi: &'a str) -> Note<'a> {
    Note {
        quoi,
        mois: Some("2026-09"),
        fichier: None,
        mode: "reel",
        qui: "test",
        accepte: true,
        destinataire: "",
        flux: "F1",
        reponse: "",
        trace: None,
        detail: None,
        versions: None,
        quand: "",
        origine: "",
    }
}

#[test]
fn noter_ajoute_a_la_fin_et_lire_filtre() {
    let d = dossier_de_test("journal", "ajouts");
    let l1 = journal::noter(&d, &note("dsn"), "2026-10-03T10:00:00").unwrap();
    assert_eq!(l1["quand"], "2026-10-03T10:00:00");
    assert_eq!(l1["retours"], json!([]));
    let avant = std::fs::read(journal::chemin(&d)).unwrap();
    let mut n2 = note("dpae");
    n2.mode = "test";
    journal::noter(&d, &n2, "2026-10-03T11:00:00").unwrap();
    assert!(std::fs::read(journal::chemin(&d)).unwrap().starts_with(&avant), "ce qui etait ecrit au journal a ete reecrit");
    assert_eq!(journal::lire(&d, None, None, false).unwrap().len(), 2);
    assert_eq!(journal::lire(&d, Some("dpae"), None, false).unwrap().len(), 1);
    assert_eq!(journal::lire(&d, None, None, true).unwrap().len(), 1);
    assert_eq!(journal::lire(&d, Some(""), Some("2026-08"), false).unwrap().len(), 0);

    // DEUX POSTES (regle 9 b) : chacun ajoute au meme journal ce qu'il a fait, et range le meme retour ; leurs journaux mis
    // bout a bout disent tout, sans rien compter deux fois.
    let e = dossier_de_test("journal", "second-poste");
    std::fs::create_dir_all(e.join("journal")).unwrap();
    std::fs::copy(journal::chemin(&d), journal::chemin(&e)).unwrap();
    let retour = json!({"nature": "11", "publication": "P1", "statut": "fictive"});
    for (poste, quoi) in [(&d, "documents"), (&e, "contrat-signature")] {
        journal::noter(poste, &note(quoi), "2026-10-04T09:00:00").unwrap();
        journal::rattacher(poste, |l| l["quoi"] == "dsn", retour.clone()).unwrap().unwrap();
    }
    let mut reunis = std::fs::read(journal::chemin(&d)).unwrap();
    reunis.extend(std::fs::read(journal::chemin(&e)).unwrap());
    std::fs::write(journal::chemin(&e), reunis).unwrap();
    let lignes = journal::lire(&e, None, None, false).unwrap();
    let quoi: Vec<&str> = lignes.iter().filter_map(|l| l["quoi"].as_str()).collect();
    assert_eq!(quoi, ["dsn", "dpae", "documents", "contrat-signature"]);
    assert_eq!(lignes[0]["retours"], json!([retour]));
}

#[test]
fn l_empreinte_reste_comparable_a_celles_du_journal() {
    // Les 16 premiers chiffres hexadecimaux du SHA-256, comme celles deja ecrites au journal.
    assert_eq!(journal::empreinte(b"abc"), "ba7816bf8f01cfea");
    let d = dossier_de_test("journal", "empreinte");
    let f = d.join("DSN.dsn");
    std::fs::write(&f, b"abc").unwrap();
    let mut n = note("dsn");
    let chemin = f.to_string_lossy().to_string();
    n.fichier = Some(&chemin);
    let l = journal::noter(&d, &n, "t").unwrap();
    assert_eq!(l["fichier"], "DSN.dsn");
    assert_eq!(l["empreinte"], "ba7816bf8f01cfea");
    let absent = d.join("absent.dsn").to_string_lossy().to_string();
    n.fichier = Some(&absent);
    let l = journal::noter(&d, &n, "t").unwrap();
    assert_eq!((l["fichier"].as_str(), &l["empreinte"]), (Some("absent.dsn"), &Value::Null));
}
