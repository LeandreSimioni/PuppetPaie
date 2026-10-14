//! `envoi` sans reseau : `envoyer` contre un faux client (l'ordre, l'echec au troisieme, `identifiants`, `copies`),
//! `preparer` et `recap_france_travail` sur la compagnie fictive, `etiquette`, `nom_fichier`, `Message`.
//! L'octet pres d'`en_mime` est dans `envoi_mime.rs`.

use std::path::{Path, PathBuf};

use paie::calcul::{calculer, Paie};
use paie::contexte::Contexte;
use paie::date::d;
use paie::engagement::{engagements_du_fichier, LignesPermanent};
use paie::envoi::{self, Message};
use serde_json::{json, Value};

use crate::outils::{lire_oracle, oracle, FauxCourriel};

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
    assert_eq!(
        e,
        "c@exemple.fr : le serveur refuse ce message\nATTENTION, 2 message(s) etaient deja partis : a@exemple.fr, b@exemple.fr"
    );
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

#[test]
fn les_tables_du_message() {
    assert_eq!(envoi::PIECES, ["bulletin", "rectificatif", "certificat_cs", "aer"]);
    for (cle, _) in envoi::LIBELLES {
        assert!(envoi::PIECES.contains(&cle));
    }
    assert_eq!(envoi::LIBELLES.len(), 4);
    assert_eq!(envoi::COURTS.len(), 4);
    assert_eq!(envoi::NOTES.len(), 4);
    assert_eq!(envoi::MOIS_ACCENTS[1], "février");
    assert_eq!(envoi::MOIS_ACCENTS[7], "août");
    assert!(envoi::NOTE_SANS_AER.starts_with("Ton attestation employeur France Travail"));
}

// -------------------------------------------------------------------------------------- preparer, sur la fictive

const MOIS: &str = "2025-10";

fn fictive() -> (PathBuf, Contexte) {
    let racine = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/donnees/fictives/fictive-3090-grande");
    let ctx = Contexte::nouveau(&racine, Some(d(&json!("2026-09-27")).unwrap())).unwrap();
    (racine, ctx)
}

fn paies_du_mois(racine: &Path, ctx: &Contexte) -> Vec<Paie> {
    let lignes: LignesPermanent = &paie::conges::lignes_du_mois;
    let (_, engs, _) = engagements_du_fichier(ctx, &racine.join("paies").join(format!("{}.json", MOIS)), lignes).unwrap();
    engs.iter().map(|e| calculer(ctx, e).unwrap()).collect()
}

fn salarie<'a>(paies: &'a [Paie], cle: &str) -> Vec<&'a Paie> {
    paies.iter().filter(|p| p.salarie["_cle"] == cle).collect()
}

#[test]
fn recap_france_travail_par_contrat() {
    let (racine, ctx) = fictive();
    let paies = paies_du_mois(&racine, &ctx);
    let ent = "Les Marées hautes (démonstration)";
    let r = envoi::recap_france_travail(&salarie(&paies, "BIANCOLELLI"), MOIS, ent).unwrap();
    // « d'octobre » : « de » s'elide devant une voyelle (avril, aout, octobre).
    assert_eq!(
        r,
        "Pour ton actualisation France Travail d'octobre 2025, employeur Les Marées hautes (démonstration) :\n\
         \x20 - du 07/10/2025 au 08/10/2025 : 2 cachets, 370,00 € brut\n\
         Ce sont les chiffres de ton bulletin ; vérifie que ce que France Travail pré-remplit est identique.\n"
    );
    // Des heures plutot que des cachets.
    let r = envoi::recap_france_travail(&salarie(&paies, "CHAMINADE"), MOIS, ent).unwrap();
    assert!(r.contains("  - du 07/10/2025 au 08/10/2025 : 16 heures, 304,00 € brut\n"), "{}", r);
    // Un permanent : rien.
    assert_eq!(envoi::recap_france_travail(&salarie(&paies, "MONTANSIER"), MOIS, ent).unwrap(), "");
    assert_eq!(envoi::recap_france_travail(&[], MOIS, ent).unwrap(), "");
}

fn pieces_ecrites(dossier: &Path, cle: &str, noms: &[&str]) {
    for (prefixe, ok) in [("Bulletin", noms.contains(&"bulletin")), ("CertificatCS", noms.contains(&"cs")), ("AER", noms.contains(&"aer")), ("BulletinRectificatif", noms.contains(&"rectificatif"))] {
        if ok {
            std::fs::write(dossier.join(envoi::nom_fichier(prefixe, cle, "Octobre2025")), b"%PDF-1.4\n").unwrap();
        }
    }
}

#[test]
fn preparer_un_message_par_salarie() {
    let (racine, ctx) = fictive();
    let paies = paies_du_mois(&racine, &ctx);
    let t = travail("preparer");
    pieces_ecrites(&t, "BIANCOLELLI", &["bulletin", "cs"]);
    pieces_ecrites(&t, "MONTANSIER", &["bulletin", "aer"]);
    let messages = envoi::preparer(&paies, &ctx.entreprise, MOIS, &t, None).unwrap();
    // Un par salarie (sept, avec leurs cles), dans l'ordre de la saisie.
    let cles: Vec<&str> = messages.iter().map(|m| m.cle.as_str()).collect();
    assert_eq!(cles, ["MONTANSIER", "NICOLET", "AUDINOT", "BIANCOLELLI", "DEBURAU", "CLAIRON", "CHAMINADE"]);
    let m = &messages[3];
    assert_eq!(m.nom, "Dominique BIANCOLELLI");
    assert_eq!(m.destinataire, "dominique.biancolelli@exemple.invalid");
    assert_eq!(m.sujet, "Octobre 2025 — bulletin et certificat Congés Spectacles");
    assert_eq!(m.pieces, vec![t.join("Bulletin_BIANCOLELLI_Octobre2025.pdf"), t.join("CertificatCS_BIANCOLELLI_Octobre2025.pdf")]);
    assert!(m.html.is_empty());
    // « d'octobre » : « de » s'elide devant une voyelle (avril, aout, octobre).
    assert_eq!(
        m.corps,
        "Bonjour Dominique,\n\nTu trouveras ci-joint tes documents pour octobre 2025 :\n\n\
         \x20 - le bulletin de paie\n  - le certificat d'emploi Congés Spectacles\n\n\
         Le bulletin est à conserver sans limitation de durée.\n\n\
         Le certificat d'emploi Congés Spectacles te sert à faire valoir tes droits à congé auprès d'Audiens : garde-le, c'est la pièce qu'ils demandent.\n\n\
         Ton attestation employeur France Travail (AER Cinéma Spectacle) n'est pas encore revenue : France Travail l'édite à partir de notre déclaration, quelques jours après le dépôt. Elle te sera envoyée dès réception.\n\n\
         Pour ton actualisation France Travail d'octobre 2025, employeur Les Marées hautes (démonstration) :\n\
         \x20 - du 07/10/2025 au 08/10/2025 : 2 cachets, 370,00 € brut\n\
         Ce sont les chiffres de ton bulletin ; vérifie que ce que France Travail pré-remplit est identique.\n\n\
         Bien à toi,\nAdrienne\nLes Marées hautes (démonstration)\n"
    );
    // Un permanent avec son AER : pas de note « pas encore revenue », pas de recapitulatif.
    let p = &messages[0];
    assert_eq!(p.sujet, "Octobre 2025 — bulletin et attestation France Travail");
    assert!(!p.corps.contains("pas encore revenue") && !p.corps.contains("actualisation"));
    assert!(p.corps.contains("Elle est éditée par France Travail"));
    // Rien de joint : l'objet ne promet que des « documents », et la note le dit.
    let nu = &messages[1];
    assert!(nu.pieces.is_empty());
    assert_eq!(nu.sujet, "Octobre 2025 — documents");
    assert!(nu.corps.contains("pour octobre 2025 :\n\n\n\nTon attestation"), "{:?}", nu.corps);
}

#[test]
fn preparer_refuse_un_salarie_sans_adresse() {
    let (racine, ctx) = fictive();
    let mut paies = paies_du_mois(&racine, &ctx);
    for p in paies.iter_mut() {
        if p.salarie["_cle"] == "CLAIRON" {
            p.salarie["email"] = json!("");
        }
    }
    let e = envoi::preparer(&paies, &ctx.entreprise, MOIS, &travail("sans-adresse"), None).unwrap_err();
    assert_eq!(e, "Pas d'adresse e-mail pour CLAIRON LUCILE : completer config/salaries.json avant d'envoyer.");
}

#[test]
fn preparer_ramasse_le_rectificatif_de_qui_n_a_pas_de_paie() {
    let (racine, ctx) = fictive();
    let paies = paies_du_mois(&racine, &ctx);
    let t = travail("rectificatif");
    pieces_ecrites(&t, "JACQUET", &["rectificatif"]);
    // Un rectificatif d'un salarie qui a deja une paie : pas de second message.
    pieces_ecrites(&t, "AUDINOT", &["bulletin", "rectificatif"]);
    let fiches = ctx.salaries.clone();
    let fiche = |cle: &str| -> Value { fiches[cle].clone() };
    let messages = envoi::preparer(&paies, &ctx.entreprise, MOIS, &t, Some(&fiche)).unwrap();
    assert_eq!(messages.len(), 8);
    let m = messages.last().unwrap();
    assert_eq!(m.cle, "JACQUET");
    assert_eq!(m.destinataire, "isabelle.jacquet@exemple.invalid");
    assert_eq!(m.sujet, "Octobre 2025 — bulletin rectificatif");
    assert!(m.corps.contains("Le bulletin rectificatif remplace, pour le mois qu'il indique"));
    let audinot = messages.iter().find(|m| m.cle == "AUDINOT").unwrap();
    assert_eq!(audinot.sujet, "Octobre 2025 — bulletin et bulletin rectificatif");
    assert_eq!(messages.iter().filter(|m| m.cle == "AUDINOT").count(), 1);
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

/// `tests/donnees/preparer_fictives.json` : les messages de deux mois de deux compagnies fictives, avec les fiches (un bulletin
/// rectificatif, des AER, un salarie sans paie) ; le dossier ne porte que les noms de fichiers listes. (Le mois 2025-10 de
/// `fictive-3090-grande` est prouve plus haut, a la main.) L'elision (« d'octobre », « d'avril », « d'aout ») est dans
/// `src/envoi.rs` (`de_mois`) et dans `recap_france_travail_par_contrat`.
/// `PUPPETPAIE_REECRIRE_ORACLES=1` le reecrit depuis le moteur.
#[test]
fn preparer_rend_les_messages_attendus_sur_deux_fictives() {
    let mut cas: Value = lire_oracle("preparer_fictives.json");
    for un in cas.as_array_mut().unwrap() {
        let (nom_fictive, mois) = (un["fictive"].as_str().unwrap().to_string(), un["mois"].as_str().unwrap().to_string());
        let racine = crate::outils::fictive(&nom_fictive);
        let ctx = Contexte::nouveau(&racine, Some(d(&json!("2026-09-27")).unwrap())).unwrap();
        let lignes: LignesPermanent = &paie::conges::lignes_du_mois;
        let (_, engs, _) = engagements_du_fichier(&ctx, &racine.join("paies").join(format!("{}.json", mois)), lignes).unwrap();
        let paies: Vec<Paie> = engs.iter().map(|e| calculer(&ctx, e).unwrap()).collect();
        let t = travail(&format!("fictive-{}-{}", nom_fictive, mois));
        for f in un["fichiers"].as_array().unwrap() {
            std::fs::write(t.join(f.as_str().unwrap()), b"%PDF-1.4\n").unwrap();
        }
        let fiches = ctx.salaries.clone();
        let fiche = |cle: &str| -> Value { fiches[cle].clone() };
        let messages = envoi::preparer(&paies, &ctx.entreprise, &mois, &t, Some(&fiche)).unwrap();
        un["messages"] = messages
            .iter()
            .map(|m| {
                let pieces: Vec<String> = m.pieces.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
                json!({"cle": m.cle, "nom": m.nom, "dest": m.destinataire, "sujet": m.sujet, "corps": m.corps, "html": m.html, "pieces": pieces})
            })
            .collect();
    }
    oracle("preparer_fictives.json", "", &cas);
}
