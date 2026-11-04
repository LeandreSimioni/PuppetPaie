//! Le journal des envois n'a que des ajouts (ce qui est parti se lit, regle 4), et l'empreinte d'un fichier y reste
//! comparable a celles deja ecrites : c'est elle qui dit qu'un fichier est deja parti.

use envois::journal::{self, Acte, DetailDsn, Envoi, Ligne, Mode, Revenu, RetourFiche};
use socle::mois::Mois;

use crate::outils::dossier_de_test;

fn envoi<D, R>(mode: Mode, detail: D) -> Envoi<D, R> {
    let mut e = Envoi::nouveau(mode, "test", true, "", "", detail);
    e.mois = Some(Mois::lire("2026-09").unwrap());
    e.flux = "F1".into();
    e
}

fn dsn(mode: Mode) -> Ligne {
    Ligne::Dsn(envoi(mode, DetailDsn::default()))
}

fn fiche(code: &str) -> Ligne {
    Ligne::FicheDemande(envoi(Mode::Reel, journal::DetailFiche { code: code.into(), adresse: "a@b.invalid".into(), prenom: "A".into(), message: None, expediteur: None }))
}

#[test]
fn noter_ajoute_a_la_fin_et_lire_filtre() {
    let d = dossier_de_test("journal", "ajouts");
    let l1 = journal::noter(&d, dsn(Mode::Reel), None, "2026-10-03T10:00:00").unwrap();
    assert_eq!((l1.quand(), l1.nombre_de_retours()), ("2026-10-03T10:00:00", 0));
    let avant = std::fs::read(journal::chemin(&d)).unwrap();
    journal::noter(&d, Ligne::Dpae(envoi(Mode::Test, journal::DetailDpae::default())), None, "2026-10-03T11:00:00").unwrap();
    assert!(std::fs::read(journal::chemin(&d)).unwrap().starts_with(&avant), "ce qui etait ecrit au journal a ete reecrit");
    assert_eq!(journal::lire(&d, None, None, false).unwrap().len(), 2);
    assert_eq!(journal::lire(&d, Some(Acte::Dpae), None, false).unwrap().len(), 1);
    assert_eq!(journal::lire(&d, None, None, true).unwrap().len(), 1);
    assert_eq!(journal::lire(&d, None, Some(Mois::lire("2026-08").unwrap()), false).unwrap().len(), 0);

    // DEUX POSTES (regle 9 b) : chacun ajoute au meme journal ce qu'il a fait, et range le meme retour ; leurs journaux mis
    // bout a bout disent tout, sans rien compter deux fois.
    journal::noter(&d, fiche("F-1"), None, "2026-10-03T12:00:00").unwrap();
    let e = dossier_de_test("journal", "second-poste");
    std::fs::create_dir_all(e.join("journal")).unwrap();
    std::fs::copy(journal::chemin(&d), journal::chemin(&e)).unwrap();
    let retour = RetourFiche { code: "F-1".into(), abandon: true, salarie: None, illisible: None, erreurs: None, quand: "2026-10-04T08:00:00".into(), qui: "t".into(), de: None, message: None, reponse: "".into() };
    for (poste, quoi) in [(&d, fiche("F-2")), (&e, dsn(Mode::Test))] {
        journal::noter(poste, quoi, None, "2026-10-04T09:00:00").unwrap();
        journal::rattacher(poste, |l| l.acte() == Acte::FicheDemande, Revenu::Fiche(retour.clone())).unwrap();
    }
    let mut reunis = std::fs::read(journal::chemin(&d)).unwrap();
    reunis.extend(std::fs::read(journal::chemin(&e)).unwrap());
    std::fs::write(journal::chemin(&e), reunis).unwrap();
    let lignes = journal::lire(&e, None, None, false).unwrap();
    let quoi: Vec<Acte> = lignes.iter().map(Ligne::acte).collect();
    assert_eq!(quoi, [Acte::Dsn, Acte::Dpae, Acte::FicheDemande, Acte::FicheDemande, Acte::Dsn]);
    assert_eq!(lignes[2].fiche().unwrap().retours, [retour.clone()]);
    assert_eq!(lignes[3].fiche().unwrap().retours, [retour]);

    // UNE LIGNE QUI NE SE LIT PAS STRICTEMENT arrete la lecture, sa ligne et sa cle nommees : rien ne se devine.
    let f = dossier_de_test("journal", "abime");
    std::fs::create_dir_all(f.join("journal")).unwrap();
    let mut texte = std::fs::read_to_string(journal::chemin(&d)).unwrap();
    texte.push_str(&texte.lines().next().unwrap().replace("\"flux\":", "\"fluxx\":").replace("\"id\":\"", "\"id\":\"autre-"));
    texte.push('\n');
    std::fs::write(journal::chemin(&f), texte).unwrap();
    let refus = journal::lu(&f).unwrap_err();
    assert!(refus.starts_with("Le journal des envois de la compagnie ne se lit pas : sa ligne 6 est abîmée"), "{refus}");
    assert!(refus.contains("fluxx"), "{refus}");
}

#[test]
fn l_empreinte_reste_comparable_a_celles_du_journal() {
    // Les 16 premiers chiffres hexadecimaux du SHA-256, comme celles deja ecrites au journal.
    assert_eq!(journal::empreinte(b"abc"), "ba7816bf8f01cfea");
    let d = dossier_de_test("journal", "empreinte");
    let f = d.join("DSN.dsn");
    std::fs::write(&f, b"abc").unwrap();
    let l = journal::noter(&d, dsn(Mode::Reel), Some(&f), "t").unwrap();
    assert_eq!((l.fichier(), l.empreinte()), (Some("DSN.dsn"), Some("ba7816bf8f01cfea")));
    let l = journal::noter(&d, dsn(Mode::Reel), Some(&d.join("absent.dsn")), "t").unwrap();
    assert_eq!((l.fichier(), l.empreinte()), (Some("absent.dsn"), None));
}
