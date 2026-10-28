//! `envoi` sans reseau : `envoyer` contre un faux client (l'ordre, l'echec au troisieme, `identifiants`, `copies`),
//! `etiquette`, `nom_fichier`, `Message`. L'octet pres d'`en_mime` est dans `envoi_mime.rs`.

use std::path::PathBuf;

use paie::envoi::{self, Message};

use crate::outils::FauxCourriel;

/// Un faux client dont l'envoi de rang `refuse` (a partir de 0) echoue.
fn faux(refuse: Option<usize>) -> FauxCourriel {
    FauxCourriel { refuse_le: refuse, ..FauxCourriel::nouveau() }
}

fn travail(nom: &str) -> PathBuf {
    crate::outils::dossier_de_test("envoi", nom)
}

fn message(dest: &str) -> Message {
    Message {
        cle: dest.split('@').next().unwrap().into(),
        nom: "X".into(),
        destinataire: dest.into(),
        sujet: format!("Sujet pour {}", dest),
        corps: "Bonjour,\n".into(),
        ..Default::default()
    }
}

fn en_tete(mime: &[u8], nom: &str) -> String {
    let t = String::from_utf8_lossy(mime).into_owned();
    t.lines().find_map(|l| l.strip_prefix(&format!("{}: ", nom)).map(String::from)).unwrap_or_default()
}

#[test]
fn envoyer_un_par_un_dans_l_ordre() {
    let mut c = faux(None);
    let messages = [message("a@exemple.fr"), message("b@exemple.fr"), message("c@exemple.fr")];
    let (mut ids, mut copies) = (Vec::new(), Vec::new());
    let faits = envoi::envoyer(&messages, "compagnie@gmail.com", &mut c, Some(&mut ids), Some(&mut copies)).unwrap();
    assert_eq!(faits, vec!["a@exemple.fr", "b@exemple.fr", "c@exemple.fr"]);
    assert_eq!(ids, vec!["<faux-0@exemple.invalid>", "<faux-1@exemple.invalid>", "<faux-2@exemple.invalid>"]);
    // Les copies sont les messages exactement tels qu'ils sont partis.
    assert_eq!(copies, *c.recus.borrow());
    for (m, brut) in messages.iter().zip(c.recus.borrow().iter()) {
        assert_eq!(en_tete(brut, "To"), m.destinataire);
        assert_eq!(en_tete(brut, "From"), "compagnie@gmail.com");
        assert_eq!(en_tete(brut, "Subject"), m.sujet);
        assert!(en_tete(brut, "Message-ID").ends_with("@gmail.com>"));
    }
}

#[test]
fn envoyer_sans_identifiants_ni_copies() {
    let mut c = faux(None);
    let faits = envoi::envoyer(&[message("a@exemple.fr")], "compagnie@gmail.com", &mut c, None, None).unwrap();
    assert_eq!(faits, vec!["a@exemple.fr"]);
    assert!(envoi::envoyer(&[], "compagnie@gmail.com", &mut c, None, None).unwrap().is_empty());
}

#[test]
fn envoyer_l_echec_au_troisieme_dit_qui_est_deja_parti() {
    let mut c = faux(Some(2));
    let messages = [message("a@exemple.fr"), message("b@exemple.fr"), message("c@exemple.fr"), message("d@exemple.fr")];
    let (mut ids, mut copies) = (Vec::new(), Vec::new());
    let e = envoi::envoyer(&messages, "compagnie@gmail.com", &mut c, Some(&mut ids), Some(&mut copies)).unwrap_err();
    // L'erreur nomme le message refuse et ceux qui etaient deja partis, pas celui qui n'a pas ete tente.
    assert!(e.contains("c@exemple.fr") && e.contains("le serveur refuse ce message"), "{}", e);
    assert!(e.contains("a@exemple.fr") && e.contains("b@exemple.fr") && !e.contains("d@exemple.fr"), "{}", e);
    // Le quatrieme n'a pas ete tente ; les listes ne portent que ce qui est reellement parti.
    assert_eq!(c.partis(), 2);
    assert_eq!(ids.len(), 2);
    assert_eq!(copies.len(), 2);
}

#[test]
fn envoyer_l_echec_au_premier_est_l_erreur_telle_quelle() {
    let mut c = faux(Some(0));
    let mut ids = Vec::new();
    let e = envoi::envoyer(&[message("a@exemple.fr"), message("b@exemple.fr")], "compagnie@gmail.com", &mut c, Some(&mut ids), None).unwrap_err();
    assert_eq!(e, "le serveur refuse ce message");
    assert!(ids.is_empty() && c.partis() == 0);
}

#[test]
fn envoyer_refuse_un_message_qu_on_ne_sait_pas_assembler_avant_d_envoyer() {
    let mut c = faux(None);
    let mut mauvais = message("b@exemple.fr");
    mauvais.sujet = "deux\nlignes".into();
    assert!(envoi::envoyer(&[message("a@exemple.fr"), mauvais], "compagnie@gmail.com", &mut c, None, None).is_err());
    // Le premier est parti, le second non.
    assert_eq!(c.partis(), 1);
    let mut inconnue = message("a@exemple.fr");
    inconnue.pieces = vec![PathBuf::from("/nulle/part/absent.pdf")];
    assert!(envoi::envoyer(&[inconnue], "compagnie@gmail.com", &mut c, None, None).is_err());
    assert!(envoi::envoyer(&[message("pas une adresse")], "compagnie@gmail.com", &mut c, None, None).is_err());
}

#[test]
fn le_message_pese_ses_pieces() {
    let t = travail("taille");
    std::fs::write(t.join("a.pdf"), vec![0u8; 100]).unwrap();
    std::fs::write(t.join("b.pdf"), vec![0u8; 23]).unwrap();
    let mut m = message("a@exemple.fr");
    m.pieces = vec![t.join("a.pdf"), t.join("b.pdf"), t.join("absente.pdf")];
    assert_eq!(m.taille(), 123);
}

// ----------------------------------------------------------------------------------------------- les noms

#[test]
fn etiquette_et_nom_de_fichier() {
    assert_eq!(envoi::etiquette("2026-08").unwrap(), "Aout2026");
    assert_eq!(envoi::etiquette("2026-02").unwrap(), "Fevrier2026");
    assert_eq!(envoi::etiquette("2026-12").unwrap(), "Decembre2026");
    assert!(envoi::etiquette("2026-13").is_err());
    assert!(envoi::etiquette("pas un mois").is_err());
    assert_eq!(envoi::nom_fichier("Bulletin", "DUPONT", "Aout2026"), "Bulletin_DUPONT_Aout2026.pdf");
}












// ------------------------------------------------------------------------- `Date`, aux changements d'heure

/// Les jours depuis le 01/01/1970 du jour civil (a, m, j) (`days_from_civil`, H. Hinnant).
fn jours(a: i64, m: i64, j: i64) -> i64 {
    let a = if m <= 2 { a - 1 } else { a };
    let ere = a.div_euclid(400);
    let an = a - ere * 400;
    let jour_an = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + j - 1;
    ere * 146_097 + (an * 365 + an / 4 - an / 100 + jour_an) - 719_468
}

/// La regle europeenne de l'heure d'ete (directive 2000/84/CE, texte non depose) : UTC+2 du dernier dimanche de mars au
/// dernier dimanche d'octobre, la bascule a 01:00 UTC. Verifiee sur quelques annees, de part et d'autre de chaque bascule.
#[test]
fn date_de_rend_l_heure_de_paris_aux_changements_d_heure() {
    for annee in [1996, 2026, 2050, 2100] {
        for (mois, nom, avant, apres) in [(3, "Mar", ("01:59:59", "+0100"), ("03:00:00", "+0200")), (10, "Oct", ("02:59:59", "+0200"), ("02:00:00", "+0100"))] {
            let fin = jours(annee, mois, 31);
            let dimanche = fin - (fin + 4).rem_euclid(7);
            let t = dimanche * 86_400 + 3_600;
            let jour = 31 - (fin - dimanche);
            assert_eq!(envoi::date_de(t - 1), format!("Sun, {:02} {} {} {} {}", jour, nom, annee, avant.0, avant.1));
            assert_eq!(envoi::date_de(t), format!("Sun, {:02} {} {} {} {}", jour, nom, annee, apres.0, apres.1));
        }
    }
    for (t, texte) in [
        (946080000, "Sat, 25 Dec 1999 01:00:00 +0100"),
        (1790000000, "Mon, 21 Sep 2026 16:13:20 +0200"),
        (1767225599, "Thu, 01 Jan 2026 00:59:59 +0100"),
    ] {
        assert_eq!(envoi::date_de(t), texte, "instant {}", t);
    }
}

// ------------------------------------------------------------- preparer sur des compagnies fictives (lot H3a, la preuve)

