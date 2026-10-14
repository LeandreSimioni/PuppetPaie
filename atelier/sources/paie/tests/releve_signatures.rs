//! La releve des reponses « Je signe » (lot I2a) : `releve_signatures::relever_signatures`, la route `signatures-relever`, la
//! boite de la demonstration, `contrat_recu_avec`, `contresigner`.
//!
//! Le tri strict des reponses, ce qui est note, archive, contresigne et renvoye, les pannes, les defauts corriges et les choix
//! de l'utilisateur du 07/10/2026. Fichier seul : le jour et la cle du poste passent par l'environnement (le moteur les y lit).
//! Chaque test monte une copie d'une compagnie de demonstration dans un dossier temporaire a lui (jamais le vrai `prive/` ni
//! `demo/`). AUCUN RESEAU, AUCUN COURRIEL : la boite est le faux client du trait `Courriel` (ou la boite simulee de la demonstration).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Once, OnceLock};

use paie::contexte::Contexte;
use outils::{copier, dossier_de_test, dossier_du_processus, ecrire, lire, FauxCourriel as Faux};
use paie::courriel::Courriel;
use paie::date::Jour;
use paie::releve_signatures as rs;
use serde_json::{json, Value};

#[path = "outils/mod.rs"]
mod outils;

const AUJOURDHUI: &str = "2026-10-07";
/// Le compte de la boite (l'expediteur des demandes et des contrats contresignes).
const BOITE: &str = "direction@exemple.invalid";

static UNE_FOIS: Once = Once::new();

/// L'environnement du processus, pose une fois avant le premier test : le jour, la cle du poste, aucun secret.
fn tour() {
    UNE_FOIS.call_once(|| {
        for v in outils::RETIREES {
            std::env::remove_var(v);
        }
        std::env::set_var("PUPPETPAIE_AUJOURDHUI", AUJOURDHUI);
        std::env::set_var("PUPPETPAIE_CLE", dossier_du_processus("releve-signatures-poste"));
    });
}

fn journal(racine: &Path) -> Vec<Value> {
    lire(racine, "journal/depots.json").as_array().unwrap().clone()
}

fn lignes(racine: &Path, quoi: &str) -> Vec<Value> {
    journal(racine).into_iter().filter(|l| l["quoi"] == json!(quoi)).collect()
}

fn options(racine: &Path) -> HashMap<String, String> {
    let mut o = HashMap::new();
    o.insert("racine".to_string(), racine.to_string_lossy().into_owned());
    o.insert("date".to_string(), AUJOURDHUI.to_string());
    o
}

fn contexte(racine: &Path) -> Contexte {
    Contexte::nouveau(racine, Some(Jour::lire(AUJOURDHUI).unwrap())).unwrap()
}

fn route(racine: &Path, nom: &str, charge: Value) -> Value {
    let r = paie::routes::commande(&options(racine), &json!({"route": nom, "charge": charge}).to_string()).unwrap();
    assert_eq!(r["ok"], json!(true), "{} : {}", nom, r);
    r["resultat"].clone()
}

// ------------------------------------------------------------------ les scenarios, montes une fois puis recopies

/// Une demande de signature, telle que le journal la garde.
#[derive(Clone, Debug)]
struct Demande {
    code: String,
    adresse: String,
    salarie: String,
    message: String,
    fichier: String,
    debuts: Vec<String>,
}

fn demandes(racine: &Path) -> Vec<Demande> {
    let mut out: Vec<Demande> = Vec::new();
    for l in lignes(racine, "contrat-signature") {
        for c in l["detail"]["contrats"].as_array().into_iter().flatten() {
            let Some(code) = c["code"].as_str() else { continue };
            match out.iter_mut().find(|d| d.code == code) {
                Some(d) => d.debuts.push(c["debut"].as_str().unwrap().to_string()),
                None => out.push(Demande {
                    code: code.to_string(),
                    adresse: c["adresse"].as_str().unwrap().to_string(),
                    salarie: c["salarie"].as_str().unwrap().to_string(),
                    message: c["message"].as_str().unwrap_or("").to_string(),
                    fichier: c["fichier"].as_str().unwrap().to_string(),
                    debuts: vec![c["debut"].as_str().unwrap().to_string()],
                }),
            }
        }
    }
    out
}

/// Monte (une fois) une copie de demo ou les contrats d'un mois sont partis a la signature.
fn modele(cle: &'static str, demo: &str, mois: &str, saisie: impl Fn(&mut Value)) -> PathBuf {
    let racine = dossier_du_processus(&format!("releve-signatures-modele-{}", cle));
    copier(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/donnees/demo").join(demo), &racine);
    let mut s = lire(&racine, &format!("paies/{}.json", mois));
    saisie(&mut s);
    ecrire(&racine, &format!("paies/{}.json", mois), &s);
    // Les demos ont deja note leurs contrats recus : on retire ceux du mois (dans la copie), pour que la signature soit attendue.
    let reste: Vec<Value> = journal(&racine).into_iter().filter(|l| !(l["quoi"] == json!("contrat-recu") && l["mois"] == json!(mois))).collect();
    ecrire(&racine, "journal/depots.json", &Value::Array(reste));
    let ap = route(&racine, "signature-apercu", json!({"mois": mois}));
    let contrats: Vec<Value> = ap["contrats"].as_array().unwrap().iter().map(|c| json!({"salarie": c["salarie"], "debut": c["debut"], "numero": c["numero"]})).collect();
    route(&racine, "demander-signature", json!({"mois": mois, "contrats": contrats, "motdepasse": ""}));
    racine
}

/// Trois salaries intermittents (Brioche, Seraphin, Lemercier), un contrat chacun, en mars 2026.
fn petite() -> &'static Path {
    static M: OnceLock<PathBuf> = OnceLock::new();
    M.get_or_init(|| modele("petite", "demo-3090-petite", "2026-03", |_| {}))
}

/// Une demo ou Biancolelli et Clairon ont deux contrats en mars 2026 : deux contrats dans un seul courriel (un PDF, deux pages).
fn grande() -> &'static Path {
    static M: OnceLock<PathBuf> = OnceLock::new();
    M.get_or_init(|| modele("grande", "demo-3090-grande", "2026-03", |_| {}))
}

/// Brioche a deux contrats qui commencent le meme jour (retours Windows, B).
fn meme_jour() -> &'static Path {
    static M: OnceLock<PathBuf> = OnceLock::new();
    M.get_or_init(|| {
        modele("meme-jour", "demo-3090-petite", "2026-03", |s| {
            let mut e = s["engagements"][0].clone();
            e["numero"] = json!("BR1.26.03");
            e["lignes"][0]["quantite"] = json!(1);
            s["engagements"].as_array_mut().unwrap().insert(1, e);
        })
    })
}

/// Une copie du scenario, a l'etat ou les demandes viennent de partir.
fn copie(modele: &Path, nom: &str) -> PathBuf {
    let racine = dossier_de_test("releve-signatures", nom);
    copier(modele, &racine);
    racine
}

// ------------------------------------------------------------------ le faux client

/// La boite de la direction, en memoire.
fn nouvelle_boite() -> Faux {
    Faux { compte: BOITE.into(), ..Faux::default() }
}

// ------------------------------------------------------------------ des messages ecrits a la main

fn corps_je_signe(code: &str) -> String {
    format!("Je soussigné(e) Nom Complet déclare avoir lu et accepter le contrat d'engagement.\n\nCode de signature : {}\n", code)
}

fn message(de: &str, sujet: &str, corps: &str, date: &str, id: &str, crlf: bool) -> Vec<u8> {
    let nl = if crlf { "\r\n" } else { "\n" };
    let corps = corps.replace("\r\n", "\n").replace('\n', nl);
    format!(
        "From: {de}{nl}To: {BOITE}{nl}Subject: {sujet}{nl}Date: {date}{nl}Message-ID: {id}{nl}MIME-Version: 1.0{nl}Content-Type: text/plain; charset=utf-8{nl}Content-Transfer-Encoding: 8bit{nl}{nl}{corps}"
    )
    .into_bytes()
}

const DATE_REPONSE: &str = "Sat, 03 Oct 2026 10:15:00 +0200";

/// La reponse qu'un salarie ecrit en appuyant sur « Je signe ».
fn je_signe(d: &Demande, id: &str) -> Vec<u8> {
    message(&d.adresse, "Re: Ton contrat", &corps_je_signe(&d.code), DATE_REPONSE, id, false)
}

fn releve(racine: &Path, faux: &Faux, mois: Option<&str>) -> rs::ReleveSignatures {
    let mut f = faux.clone();
    rs::relever_signatures(&options(racine), &contexte(racine), Some(&mut f), mois, "interface").unwrap()
}

fn nom_des_pieces(octets: &[u8]) -> Vec<String> {
    let m = mailparse::parse_mail(octets).unwrap();
    let mut out = Vec::new();
    fn parcourir(m: &mailparse::ParsedMail, out: &mut Vec<String>) {
        if let Some(n) = m.get_content_disposition().params.get("filename") {
            out.push(n.clone());
        }
        for s in &m.subparts {
            parcourir(s, out);
        }
    }
    parcourir(&m, &mut out);
    out
}

fn entete(octets: &[u8], nom: &str) -> String {
    let m = mailparse::parse_mail(octets).unwrap();
    mailparse::MailHeaderMap::get_first_value(m.headers.as_slice(), nom).unwrap_or_default()
}

fn texte(octets: &[u8]) -> String {
    fn parcourir(m: &mailparse::ParsedMail) -> Option<String> {
        if m.ctype.mimetype == "text/plain" {
            return m.get_body().ok();
        }
        m.subparts.iter().find_map(parcourir)
    }
    parcourir(&mailparse::parse_mail(octets).unwrap()).unwrap_or_default().replace("\r\n", "\n")
}

fn envoi_dir(racine: &Path, mois: &str) -> PathBuf {
    racine.join("sorties").join(mois).join("envoi")
}

fn existe(racine: &Path, mois: &str, nom: &str) -> bool {
    envoi_dir(racine, mois).join(nom).exists()
}

fn pdf_texte(chemin: &Path) -> String {
    // Le PDF est verrouille (AES-256, mot de passe vide) : `fabrique_pdf` sait le relire.
    let (pages, _) = fabrique_pdf::lecture::texte(&chemin.to_string_lossy()).unwrap();
    pages.join("\n")
}

// ================================================================== la reponse valide

#[test]
fn une_reponse_valide_est_notee_archivee_contresignee_et_renvoyee() {
    tour();
    let racine = copie(petite(), "valide");
    let dm = demandes(&racine);
    assert_eq!(dm.len(), 3);
    let b = &dm[0];
    let faux = nouvelle_boite();
    faux.pose(&b.code, "501", je_signe(b, "<un@exemple.invalid>"));
    let j0 = journal(&racine).len();
    let r = releve(&racine, &faux, Some("2026-03"));

    // Ce que les appelants lisent du resultat.
    assert_eq!((r.attendues, r.recues.len(), r.incidents.len()), (3, 1, 0));
    let s = &r.recues[0];
    assert_eq!((s.mois.as_str(), s.salarie.as_str(), s.code.as_str(), s.adresse.as_str()), ("2026-03", b.salarie.as_str(), b.code.as_str(), b.adresse.as_str()));
    assert_eq!(s.quand, "2026-10-03T10:15:00", "la date du MESSAGE, en heure de Paris");
    assert_eq!(s.preuve, format!("Signature_{}_{}.eml", b.salarie, b.code));
    assert_eq!(s.fichier_signe, b.fichier.replace(".pdf", "_signe.pdf"));
    assert!(s.renvoye);
    assert_eq!(r.resume(), format!("1 signature(s) reçue(s) : {}", s.nom));
    let j = r.en_json();
    assert_eq!(j["attendues"], json!(3));
    assert_eq!(j["recues"][0]["renvoye"], json!(true));
    assert_eq!(j["resume"], json!(r.resume()));

    // La recherche : un code, entre guillemets (la boite cherche le code partout).
    assert_eq!(*faux.requetes.borrow(), vec![format!("\"{}\"", dm[0].code), format!("\"{}\"", dm[1].code), format!("\"{}\"", dm[2].code)]);
    assert_eq!(*faux.fermetures.borrow(), 1);

    // La preuve archivee : le message tel qu'il est arrive, octet pour octet.
    let preuve = envoi_dir(&racine, "2026-03").join(&s.preuve);
    assert_eq!(std::fs::read(&preuve).unwrap(), je_signe(b, "<un@exemple.invalid>"));

    // Le journal : la ligne `contrat-recu` (datee de la SIGNATURE, origine = le message), puis `contrat-contresigne`.
    let j1 = journal(&racine);
    assert_eq!(j1.len(), j0 + 2);
    let recu = &j1[j0];
    assert_eq!(recu["quoi"], json!("contrat-recu"));
    assert_eq!((recu["quand"].as_str(), recu["origine"].as_str(), recu["qui"].as_str()), (Some("2026-10-03T10:15:00"), Some("courriel 501"), Some("interface")));
    assert_eq!(recu["reponse"], json!(format!("contrat signe par courriel ({}, {})", b.code, b.adresse)));
    let d = recu["detail"].as_object().unwrap();
    let cles: Vec<&str> = d.keys().map(String::as_str).collect();
    assert_eq!(cles, ["salarie", "debut", "recu", "numero", "nom", "origine", "code", "adresse", "quand", "message", "preuve", "fichier_signe", "acte"]);
    assert_eq!((d["origine"].as_str(), d["message"].as_str(), d["acte"].as_str()), (Some("courriel"), Some("501"), Some("contrat")));
    let cs = &j1[j0 + 1];
    assert_eq!((cs["quoi"].as_str(), cs["destinataire"].as_str(), cs["accepte"].as_bool()), (Some("contrat-contresigne"), Some(b.adresse.as_str()), Some(true)));
    assert_eq!(cs["reponse"], json!("contrat contresigne renvoye au salarie"));
    assert_eq!(cs["detail"]["fichiers"], json!([s.fichier_signe]));
    assert_eq!(cs["detail"]["code"], json!(b.code));

    // Le contrat contresigne : la mention dans la case de l'artiste, le code.
    let texte_pdf = pdf_texte(&envoi_dir(&racine, "2026-03").join(&s.fichier_signe));
    assert!(texte_pdf.contains("Signé électroniquement le 03/10/2026 à 10:15"), "{}", texte_pdf);
    assert!(texte_pdf.contains(&b.adresse) && texte_pdf.contains(&format!("code de signature {}", b.code)), "{}", texte_pdf);

    // Le courriel renvoye au salarie : de la boite, au salarie, avec le contrat contresigne.
    assert_eq!(faux.partis(), 1);
    let m = faux.recus.borrow()[0].clone();
    assert_eq!((entete(&m, "From").as_str(), entete(&m, "To").as_str()), (BOITE, b.adresse.as_str()));
    assert_eq!(entete(&m, "Subject"), "Ton contrat signé - Ubu roi");
    assert_eq!(nom_des_pieces(&m), [s.fichier_signe.clone()]);
    let t = texte(&m);
    assert!(t.starts_with("Bonjour Fanchon,\n\nNous avons bien reçu ta signature le 03/10/2026 à 10:15. Ci-joint ton contrat d'engagement, signé des deux parties"), "{}", t);

    // La relève suivante n'attend plus ce code (le journal d'abord) : deux attendues, rien de nouveau.
    let r2 = releve(&racine, &faux, Some("2026-03"));
    assert_eq!((r2.attendues, r2.recues.len()), (2, 0));
    assert_eq!(faux.partis(), 1);
}

#[test]
fn la_releve_sans_mois_prend_toutes_les_demandes_et_le_qui_est_celui_de_l_appelant() {
    tour();
    let racine = copie(petite(), "tous");
    let dm = demandes(&racine);
    let faux = nouvelle_boite();
    for (i, d) in dm.iter().enumerate() {
        faux.pose(&d.code, &format!("60{}", i), je_signe(d, &format!("<m{}@exemple.invalid>", i)));
    }
    let mut f = faux.clone();
    let r = rs::relever_signatures(&options(&racine), &contexte(&racine), Some(&mut f), None, "veille").unwrap();
    assert_eq!((r.attendues, r.recues.len(), r.incidents.len()), (3, 3, 0));
    assert!(r.recues.iter().all(|s| s.renvoye));
    assert_eq!(faux.partis(), 3);
    for l in lignes(&racine, "contrat-recu").iter().filter(|l| l["origine"].as_str().is_some_and(|o| o.starts_with("courriel"))) {
        assert_eq!(l["qui"], json!("veille"));
    }
    // Un autre mois : rien d'attendu, aucune recherche, aucune boite ouverte.
    let faux2 = nouvelle_boite();
    let r = releve(&racine, &faux2, Some("2025-01"));
    assert_eq!((r.attendues, r.recues.len(), r.incidents.len()), (0, 0, 0));
    assert!(faux2.requetes.borrow().is_empty());
    assert_eq!(r.resume(), "");
}

// ================================================================== le tri strict

#[test]
fn un_code_faux_ou_absent_ou_dans_l_objet_seulement_ne_vaut_pas_signature_mais_les_minuscules_oui() {
    tour();
    let racine = copie(petite(), "code-faux");
    let d = demandes(&racine)[0].clone();
    let faux = nouvelle_boite();
    faux.pose(&d.code, "1", message(&d.adresse, "Re: contrat", &corps_je_signe("SIG-ZZZZZZ"), DATE_REPONSE, "<a@x.invalid>", false));
    faux.pose(&d.code, "2", message(&d.adresse, "Re: contrat", "Bonjour, je signe volontiers.\n", DATE_REPONSE, "<b@x.invalid>", false));
    faux.pose(&d.code, "3", message(&d.adresse, &format!("Re: {}", d.code), "Je signe.\n", DATE_REPONSE, "<c@x.invalid>", false));
    let r = releve(&racine, &faux, None);
    assert_eq!((r.recues.len(), r.incidents.len()), (0, 0));
    assert_eq!(faux.partis(), 0);
    // Le code en minuscules, dans le corps : retenu (la recherche est faite sur le texte mis en majuscules).
    faux.pose(&d.code, "4", message(&d.adresse, "Re: contrat", &corps_je_signe(&d.code.to_lowercase()), DATE_REPONSE, "<d@x.invalid>", false));
    let r = releve(&racine, &faux, None);
    assert_eq!(r.recues.len(), 1);
    assert_eq!(lignes(&racine, "contrat-recu").last().unwrap()["detail"]["message"], json!("4"));
}

#[test]
fn deux_reponses_valides_pour_un_code_la_premiere_seule_est_retenue() {
    tour();
    let racine = copie(petite(), "double");
    let d = demandes(&racine)[0].clone();
    let faux = nouvelle_boite();
    faux.pose(&d.code, "501", message(&d.adresse, "Re: contrat", &corps_je_signe(&d.code), "Sat, 03 Oct 2026 10:15:00 +0200", "<un@x.invalid>", false));
    faux.pose(&d.code, "502", message(&d.adresse, "Re: contrat", &corps_je_signe(&d.code), "Sat, 03 Oct 2026 11:30:00 +0200", "<deux@x.invalid>", false));
    let r = releve(&racine, &faux, None);
    assert_eq!(r.recues.len(), 1);
    assert_eq!(r.recues[0].quand, "2026-10-03T10:15:00");
    let recus: Vec<Value> = lignes(&racine, "contrat-recu").into_iter().filter(|l| l["origine"] == json!("courriel 501")).collect();
    assert_eq!(recus.len(), 1);
    assert_eq!(lignes(&racine, "contrat-contresigne").len(), 1);
    // La relève suivante ne retient pas la seconde.
    let r = releve(&racine, &faux, None);
    assert_eq!(r.recues.len(), 0);
    assert_eq!(faux.partis(), 1);
}

#[test]
fn une_reponse_d_une_autre_adresse_est_signalee_pas_retenue_et_l_adresse_se_lit_comme_email() {
    tour();
    let racine = copie(petite(), "autre-adresse");
    let dm = demandes(&racine);
    let d = dm[0].clone();
    let faux = nouvelle_boite();
    faux.pose(&d.code, "1", message("Quelqu'un D'autre <autre@exemple.invalid>", "Re: contrat", &corps_je_signe(&d.code), DATE_REPONSE, "<a@x.invalid>", false));
    let r = releve(&racine, &faux, None);
    assert_eq!((r.recues.len(), r.incidents.len()), (0, 1));
    assert_eq!(
        r.incidents[0],
        format!("Fanchon Brioche : une reponse portant {} vient de autre@exemple.invalid et non de {} ; elle n'est pas retenue.", d.code, d.adresse)
    );
    assert!(lignes(&racine, "contrat-recu").iter().all(|l| l["origine"] != json!("courriel 1")));
    // La bonne reponse vient ensuite : retenue, l'incident reste dit.
    faux.pose(&d.code, "2", je_signe(&d, "<b@x.invalid>"));
    let r = releve(&racine, &faux, None);
    assert_eq!((r.recues.len(), r.incidents.len()), (1, 1));
    // La meme personne ecrite autrement (majuscules, nom a virgule entre guillemets) : retenue.
    let e = dm[1].clone();
    faux.pose(&e.code, "3", message(&format!("\"{}, Prenom\" <{}>", e.salarie, e.adresse.to_uppercase()), "Re: contrat", &corps_je_signe(&e.code), DATE_REPONSE, "<c@x.invalid>", false));
    let r = releve(&racine, &faux, None);
    assert_eq!(r.recues.len(), 1);
    assert_eq!(r.recues[0].adresse, e.adresse);
}

const CITATIONS: [&str; 8] = [
    "> Code de signature : {c}",
    "Le sam. 3 oct. 2026 à 10:15, Direction <d@exemple.invalid> a écrit :\n> Code de signature : {c}",
    "On Sat, Oct 3, 2026 at 10:15 AM Direction <d@exemple.invalid> wrote:\n> Code de signature : {c}",
    "De : Direction <d@exemple.invalid>\nEnvoyé : samedi\nCode de signature : {c}",
    "From: Direction <d@exemple.invalid>\nCode de signature : {c}",
    "________________\nCode de signature : {c}",
    "-----Original Message-----\nCode de signature : {c}",
    "---- Message transféré ----\nCode de signature : {c}",
];

#[test]
fn un_code_cite_sans_etre_signe_ne_vaut_pas_signature_en_lf_comme_en_crlf() {
    tour();
    let racine = copie(petite(), "citations");
    let d = demandes(&racine)[0].clone();
    for crlf in [false, true] {
        for (i, modele) in CITATIONS.iter().enumerate() {
            let faux = nouvelle_boite();
            let corps = format!("Je ne suis pas d'accord, je ne signe pas.\n\n{}\n", modele.replace("{c}", &d.code));
            faux.pose(&d.code, "1", message(&d.adresse, "Re: contrat", &corps, DATE_REPONSE, "<a@x.invalid>", crlf));
            let r = releve(&racine, &faux, None);
            assert_eq!((r.recues.len(), r.incidents.len()), (0, 0), "citation {} (crlf = {})", i, crlf);
        }
    }
    assert_eq!(faux_partis_zero(&racine), 0);
}

fn faux_partis_zero(racine: &Path) -> usize {
    lignes(racine, "contrat-contresigne").len()
}

/// CHOIX 2 de l'utilisateur (07/10/2026) : la citation « -----Original Message----- » se coupe en CRLF comme en LF. Chez le Python,
/// le `$` du motif `-{3,}.*-{3,}$` ne passait pas devant le `\r` : la citation n'etait coupee qu'en LF, et un « Original Message »
/// d'Outlook (CRLF presque toujours en IMAP) valait signature. Meme octets, deux fins de ligne : le code CITE ne signe pas.
#[test]
fn choix_2_la_citation_original_message_se_coupe_en_crlf_comme_en_lf() {
    tour();
    let racine = copie(petite(), "original-message");
    let d = demandes(&racine)[0].clone();
    for crlf in [false, true] {
        let faux = nouvelle_boite();
        let corps = format!("Pas d'accord.\n\n-----Original Message-----\nFrom: Direction\nSent: samedi\nSubject: Ton contrat\n\nCode de signature : {}\n", d.code);
        faux.pose(&d.code, "1", message(&d.adresse, "Re: contrat", &corps, DATE_REPONSE, "<a@x.invalid>", crlf));
        let r = releve(&racine, &faux, None);
        assert_eq!(r.recues.len(), 0, "crlf = {}", crlf);
    }
    // Le code ECRIT avant la citation, lui, signe (CRLF, avec une citation derriere).
    let faux = nouvelle_boite();
    let corps = format!("Je signe.\nCode de signature : {c}\n\n-----Original Message-----\nCode de signature : {c}\n", c = d.code);
    faux.pose(&d.code, "2", message(&d.adresse, "Re: contrat", &corps, DATE_REPONSE, "<b@x.invalid>", true));
    assert_eq!(releve(&racine, &faux, None).recues.len(), 1);
}

#[test]
fn notre_propre_envoi_n_est_jamais_pris_pour_une_reponse_de_trois_facons() {
    tour();
    let racine = copie(petite(), "propre-envoi");
    let d = demandes(&racine)[0].clone();
    // La demande telle qu'elle est partie : courriels-simules/.
    let demande = std::fs::read_dir(racine.join("courriels-simules"))
        .unwrap()
        .flatten()
        .map(|e| std::fs::read(e.path()).unwrap())
        .find(|o| String::from_utf8_lossy(o).contains(&d.code))
        .unwrap();
    let faux = nouvelle_boite();
    // (a) l'identifiant que rend la recherche est celui de la demande, note au journal.
    faux.pose(&d.code, &d.message, demande.clone());
    // (b) un UID, mais l'en-tete Message-ID est celui de la demande.
    faux.pose(&d.code, "601", demande.clone());
    // (c) la phrase du bouton : la demande, d'un autre Message-ID (envoyee avant que son identifiant soit note).
    let autre = String::from_utf8_lossy(&demande).replace(&d.message, "<autre-id@exemple.invalid>").into_bytes();
    faux.pose(&d.code, "602", autre);
    // (d) notre contresigne : de = l'expediteur, pas l'adresse du salarie.
    faux.pose(&d.code, "603", message(BOITE, "Ton contrat signé", &format!("Voici. Code de signature : {}\n", d.code), DATE_REPONSE, "<cs@x.invalid>", false));
    let r = releve(&racine, &faux, None);
    assert_eq!((r.recues.len(), r.incidents.len()), (0, 0));
    assert_eq!(faux.partis(), 0);
}

#[test]
fn en_test_le_salarie_c_est_nous_sa_reponse_est_retenue() {
    tour();
    let racine = copie(petite(), "le-salarie-c-est-nous");
    let d = demandes(&racine)[0].clone();
    let mut faux = nouvelle_boite();
    faux.compte = d.adresse.clone();
    faux.pose(&d.code, "1", je_signe(&d, "<a@x.invalid>"));
    let r = releve(&racine, &faux, None);
    assert_eq!((r.recues.len(), r.incidents.len()), (1, 0));
}

// ================================================================== les pannes

#[test]
fn sans_boite_la_releve_le_dit_et_n_ecrit_rien() {
    tour();
    let racine = copie(petite(), "boite-absente");
    std::fs::remove_file(racine.join("COPIE-DE-DEVELOPPEMENT.json")).unwrap();
    let j0 = journal(&racine);
    let avant = std::fs::read_dir(envoi_dir(&racine, "2026-03")).unwrap().count();
    let r = rs::relever_signatures(&options(&racine), &contexte(&racine), None, None, "veille").unwrap();
    assert_eq!((r.attendues, r.recues.len()), (3, 0));
    assert_eq!(
        r.incidents,
        ["Aucune boite courriel : les signatures ne peuvent pas etre relevees. Poser le mot de passe d'application de la boite (Configuration > Mots de passe)."]
    );
    assert_eq!(journal(&racine), j0);
    assert_eq!(std::fs::read_dir(envoi_dir(&racine, "2026-03")).unwrap().count(), avant);
    // Par la route : la meme reponse, jamais une erreur.
    let v = route(&racine, "signatures-relever", json!({"mois": "2026-03"}));
    assert_eq!(v["incidents"].as_array().unwrap().len(), 1);
    assert_eq!(v["recues"], json!([]));
    // Rien d'attendu et pas de boite : rien a dire.
    let vide = copie(petite(), "boite-absente-vide");
    std::fs::remove_file(vide.join("COPIE-DE-DEVELOPPEMENT.json")).unwrap();
    let ecrire_vide: Vec<Value> = journal(&vide).into_iter().filter(|l| l["quoi"] != json!("contrat-signature")).collect();
    ecrire(&vide, "journal/depots.json", &Value::Array(ecrire_vide));
    let r = rs::relever_signatures(&options(&vide), &contexte(&vide), None, None, "veille").unwrap();
    assert_eq!((r.attendues, r.incidents.len()), (0, 0));
}

#[test]
fn une_recherche_en_erreur_arrete_la_boucle_une_lecture_en_erreur_passe_au_message_suivant() {
    tour();
    let racine = copie(petite(), "erreurs");
    let dm = demandes(&racine);
    let mut faux = nouvelle_boite();
    faux.recherche_en_echec_pour = vec![dm[1].code.clone()];
    faux.pose(&dm[2].code, "3", je_signe(&dm[2], "<c@x.invalid>"));
    let r = releve(&racine, &faux, None);
    assert_eq!((r.recues.len(), r.incidents.len()), (0, 1));
    assert_eq!(r.incidents[0], format!("Recherche de {} : la recherche echoue", dm[1].code));
    assert_eq!(faux.requetes.borrow().len(), 2, "la boucle s'arrete au code en erreur");
    // Une lecture en erreur : l'incident, puis le message suivant du meme code.
    let mut faux = nouvelle_boite();
    faux.pose(&dm[0].code, "1", je_signe(&dm[0], "<a@x.invalid>"));
    faux.pose(&dm[0].code, "2", je_signe(&dm[0], "<b@x.invalid>"));
    faux.lecture_en_echec = vec!["1".into()];
    let r = releve(&racine, &faux, None);
    assert_eq!((r.recues.len(), r.incidents.len()), (1, 1));
    assert_eq!(r.incidents[0], "Lecture du message 1 : la lecture echoue");
    assert_eq!(lignes(&racine, "contrat-recu").last().unwrap()["detail"]["message"], json!("2"));
}

#[test]
fn un_renvoi_qui_echoue_est_dit_la_signature_reste_notee_sans_ligne_contresigne() {
    tour();
    let racine = copie(petite(), "renvoi-echoue");
    let dm = demandes(&racine);
    let mut faux = nouvelle_boite();
    faux.refuse_a_partir_de = Some(0);
    faux.pose(&dm[0].code, "1", je_signe(&dm[0], "<a@x.invalid>"));
    let r = releve(&racine, &faux, None);
    assert_eq!((r.recues.len(), r.incidents.len()), (1, 1));
    assert!(!r.recues[0].renvoye);
    assert_eq!(
        r.incidents[0],
        format!("Fanchon Brioche : le contrat contresigne n'a pas pu etre renvoye (le serveur refuse ce message). Il est dans {}.", r.recues[0].fichier_signe)
    );
    assert_eq!(lignes(&racine, "contrat-contresigne").len(), 0);
    assert_eq!(lignes(&racine, "contrat-recu").iter().filter(|l| l["origine"] == json!("courriel 1")).count(), 1);
    assert!(existe(&racine, "2026-03", &r.recues[0].fichier_signe));
}

// ================================================================== le contrat contresigne

#[test]
fn le_pdf_envoye_absent_la_description_absente_ou_illisible_sont_dits_et_la_signature_est_notee() {
    tour();
    let racine = copie(petite(), "fichiers");
    let dm = demandes(&racine);
    let dossier = envoi_dir(&racine, "2026-03");
    std::fs::remove_file(dossier.join(&dm[0].fichier)).unwrap();
    std::fs::remove_file(dossier.join(dm[1].fichier.replace(".pdf", ".description.json"))).unwrap();
    std::fs::write(dossier.join(dm[2].fichier.replace(".pdf", ".description.json")), "pas du json").unwrap();
    let faux = nouvelle_boite();
    for (i, d) in dm.iter().enumerate() {
        faux.pose(&d.code, &format!("{}", i), je_signe(d, &format!("<m{}@x.invalid>", i)));
    }
    let r = releve(&racine, &faux, None);
    assert_eq!((r.attendues, r.recues.len(), r.incidents.len()), (3, 3, 3));
    assert!(r.recues.iter().all(|s| !s.renvoye && s.fichier_signe.is_empty()));
    assert!(r.incidents[0].starts_with(&format!("Fanchon Brioche : le contrat envoye ({}) n'est plus dans", dm[0].fichier)), "{}", r.incidents[0]);
    assert!(r.incidents[0].ends_with("; la signature est notee, sans exemplaire contresigne."), "{}", r.incidents[0]);
    assert!(r.incidents[1].starts_with("Valentin Seraphin : contrat contresigne impossible ("), "{}", r.incidents[1]);
    assert!(r.incidents[2].starts_with("Nicole Lemercier : contrat contresigne impossible ("), "{}", r.incidents[2]);
    // Les preuves sont archivees, les signatures notees, rien n'est renvoye.
    for d in &dm {
        assert!(existe(&racine, "2026-03", &format!("Signature_{}_{}.eml", d.salarie, d.code)));
    }
    assert_eq!(faux.partis(), 0);
    assert_eq!(lignes(&racine, "contrat-recu").iter().filter(|l| l["origine"].as_str().is_some_and(|o| o.starts_with("courriel"))).count(), 3);
    assert!(lignes(&racine, "contrat-recu").iter().rev().take(3).all(|l| l["detail"]["fichier_signe"] == json!("")));
}

#[test]
fn le_nom_du_salarie_qui_a_des_caracteres_que_la_police_n_a_pas_refuse_le_contresigne_les_autres_s_impriment() {
    tour();
    let racine = copie(petite(), "polices");
    let dm = demandes(&racine);
    let mut sal = lire(&racine, "config/salaries.json");
    sal[&dm[0].salarie]["nom"] = json!("Brioche");
    sal[&dm[0].salarie]["prenom"] = json!("日本");
    sal[&dm[0].salarie].as_object_mut().unwrap().remove("prenom_affichage");
    sal[&dm[0].salarie].as_object_mut().unwrap().remove("nom_affichage");
    sal[&dm[1].salarie]["nom"] = json!("Wójcik");
    sal[&dm[1].salarie]["prenom"] = json!("Łukasz");
    sal[&dm[1].salarie].as_object_mut().unwrap().remove("prenom_affichage");
    sal[&dm[1].salarie].as_object_mut().unwrap().remove("nom_affichage");
    ecrire(&racine, "config/salaries.json", &sal);
    let faux = nouvelle_boite();
    for (i, d) in dm.iter().enumerate().take(2) {
        faux.pose(&d.code, &format!("{}", i), je_signe(d, &format!("<m{}@x.invalid>", i)));
    }
    let r = releve(&racine, &faux, None);
    assert_eq!((r.recues.len(), r.incidents.len()), (2, 1));
    assert!(r.incidents[0].starts_with("日本 Brioche : contrat contresigne impossible ("), "{}", r.incidents[0]);
    assert!(!r.recues[0].renvoye && r.recues[0].fichier_signe.is_empty());
    assert!(r.recues[1].renvoye);
    assert!(pdf_texte(&envoi_dir(&racine, "2026-03").join(&r.recues[1].fichier_signe)).contains("Łukasz"));
}

#[test]
fn des_contrats_groupes_sont_un_seul_pdf_une_ligne_par_contrat_et_un_seul_renvoi() {
    tour();
    let racine = copie(grande(), "groupes");
    let dm = demandes(&racine);
    let g = dm.iter().find(|d| d.debuts.len() == 2).expect("un salarie a deux contrats").clone();
    let faux = nouvelle_boite();
    faux.pose(&g.code, "1", je_signe(&g, "<a@x.invalid>"));
    let j0 = lignes(&racine, "contrat-recu").len();
    let r = releve(&racine, &faux, Some("2026-03"));
    assert_eq!((r.recues.len(), r.incidents.len()), (1, 0));
    assert_eq!(r.recues[0].contrats.len(), 2);
    assert!(r.recues[0].fichier_signe.starts_with("Contrats_") && r.recues[0].fichier_signe.ends_with("_signe.pdf"), "{}", r.recues[0].fichier_signe);
    let nouveaux: Vec<Value> = lignes(&racine, "contrat-recu").into_iter().skip(j0).collect();
    assert_eq!(nouveaux.len(), 2, "une ligne par contrat");
    assert_eq!(nouveaux[0]["detail"]["debut"], json!(g.debuts[0]));
    assert_eq!(nouveaux[1]["detail"]["debut"], json!(g.debuts[1]));
    assert_eq!(nouveaux[0]["detail"]["fichier_signe"], nouveaux[1]["detail"]["fichier_signe"]);
    assert_eq!(faux.partis(), 1);
    let m = faux.recus.borrow()[0].clone();
    assert!(entete(&m, "Subject").contains("Tes contrats"), "{}", entete(&m, "Subject"));
    assert!(texte(&m).contains("tes 2 contrats d'engagement (un par page)"), "{}", texte(&m));
    assert_eq!(nom_des_pieces(&m).len(), 1);
    // Un PDF de deux pages, la mention dans chacune.
    let t = pdf_texte(&envoi_dir(&racine, "2026-03").join(&r.recues[0].fichier_signe));
    assert_eq!(t.matches("Signé électroniquement").count(), 2, "{}", t);
}

#[test]
fn deux_contrats_du_meme_salarie_au_meme_debut_une_reponse_vaut_pour_les_deux() {
    tour();
    let racine = copie(meme_jour(), "meme-jour");
    let dm = demandes(&racine);
    let b = dm.iter().find(|d| d.salarie == "BRIOCHE").unwrap().clone();
    assert_eq!(b.debuts.len(), 2);
    assert_eq!(b.debuts[0], b.debuts[1], "deux contrats du meme jour");
    let faux = nouvelle_boite();
    faux.pose(&b.code, "1", je_signe(&b, "<a@x.invalid>"));
    let j0 = lignes(&racine, "contrat-recu").len();
    let r = releve(&racine, &faux, Some("2026-03"));
    assert_eq!((r.recues.len(), r.incidents.len()), (1, 0));
    let nouveaux: Vec<Value> = lignes(&racine, "contrat-recu").into_iter().skip(j0).collect();
    assert_eq!(nouveaux.len(), 2);
    // Retours Windows, B (suite) : chaque ligne porte le numero de SON contrat (avant, les deux portaient celui de la premiere paie).
    assert_eq!(nouveaux[0]["detail"]["debut"], nouveaux[1]["detail"]["debut"]);
    assert_eq!(nouveaux[0]["detail"]["numero"], json!("BR1.26.02"));
    assert_eq!(nouveaux[1]["detail"]["numero"], json!("BR1.26.03"));
}

/// Retours Windows, B (suite) : Brioche a deux contrats le meme jour, SANS aucune demande encore.
fn meme_jour_sans_demande(nom: &str) -> PathBuf {
    let racine = copie(meme_jour(), nom);
    // On repart d'avant les demandes : ni demande, ni signature, ni courriel parti.
    let reste: Vec<Value> = journal(&racine).into_iter().filter(|l| l["quoi"] != json!("contrat-signature") && l["quoi"] != json!("contrat-recu")).collect();
    ecrire(&racine, "journal/depots.json", &Value::Array(reste));
    racine
}

fn ligne_de(racine: &Path, numero: &str) -> Value {
    let ap = route(racine, "signature-apercu", json!({"mois": "2026-03"}));
    ap["contrats"].as_array().unwrap().iter().find(|c| c["numero"] == json!(numero)).cloned().unwrap_or_else(|| panic!("pas de contrat {} : {}", numero, ap))
}

#[test]
fn deux_contrats_du_meme_jour_ont_chacun_leur_etat_leur_relance_et_leur_signature() {
    tour();
    let racine = meme_jour_sans_demande("meme-jour-etats");
    let ap = route(&racine, "signature-apercu", json!({"mois": "2026-03"}));
    let b: Vec<Value> = ap["contrats"].as_array().unwrap().iter().filter(|c| c["salarie"] == json!("BRIOCHE")).cloned().collect();
    assert_eq!(b.len(), 2, "{}", ap);
    let (na, nb) = (b[0]["numero"].as_str().unwrap().to_string(), b[1]["numero"].as_str().unwrap().to_string());
    assert_ne!(na, nb);
    let debut = b[0]["debut"].clone();

    // Seul A est demande : l'ecran le dit pour A, et pas pour B.
    route(&racine, "demander-signature", json!({"mois": "2026-03", "contrats": [{"salarie": "BRIOCHE", "debut": debut, "numero": na}], "motdepasse": ""}));
    assert!(ligne_de(&racine, &na)["demande_le"].is_string());
    assert!(ligne_de(&racine, &nb)["demande_le"].is_null(), "B n'a pas ete demande : {}", ligne_de(&racine, &nb));
    // On ne relance pas B (jamais demande), on relance A.
    let refus = paie::routes::commande(
        &options(&racine),
        &json!({"route": "demander-signature", "charge": {"mois": "2026-03", "contrats": [{"salarie": "BRIOCHE", "debut": debut, "numero": nb}], "motdepasse": "", "relance": true}}).to_string(),
    )
    .unwrap();
    assert_eq!(refus["ok"], json!(false), "{}", refus);
    assert!(refus["erreur"].to_string().contains("n'a jamais ete demande"), "{}", refus);
    // Pas d'avenant ni de remplacement pour B, qui n'est jamais parti.
    let r = paie::routes::commande(&options(&racine), &json!({"route": "avenant-apercu", "charge": {"salarie": "BRIOCHE", "debut": debut, "numero": nb, "mois": "2026-03"}}).to_string()).unwrap();
    assert_eq!(r["ok"], json!(false), "{}", r);
    assert!(r["erreur"].to_string().contains("Aucun contrat n'est parti"), "{}", r);
    route(&racine, "avenant-apercu", json!({"salarie": "BRIOCHE", "debut": debut, "numero": na, "mois": "2026-03"}));
    let r = paie::routes::commande(
        &options(&racine),
        &json!({"route": "demander-signature", "charge": {"mois": "2026-03", "contrats": [{"salarie": "BRIOCHE", "debut": debut, "numero": nb}], "motdepasse": "", "remplace": true}}).to_string(),
    )
    .unwrap();
    assert_eq!(r["ok"], json!(false), "{}", r);
    assert!(r["erreur"].to_string().contains("n'est jamais parti"), "{}", r);

    // La reponse de A ne releve que A : une ligne `contrat-recu`, a son numero ; B reste ni demande ni recu.
    let dm = demandes(&racine);
    assert_eq!(dm.len(), 1);
    let faux = nouvelle_boite();
    faux.pose(&dm[0].code, "1", je_signe(&dm[0], "<a@x.invalid>"));
    let r = releve(&racine, &faux, Some("2026-03"));
    assert_eq!((r.recues.len(), r.incidents.len()), (1, 0), "{:?}", r.incidents);
    let recus = lignes(&racine, "contrat-recu");
    assert_eq!(recus.len(), 1);
    assert_eq!(recus[0]["detail"]["numero"], json!(na));
    assert!(ligne_de(&racine, &na)["recu_le"].is_string());
    assert!(ligne_de(&racine, &nb)["recu_le"].is_null());
    assert!(ligne_de(&racine, &nb)["demande_le"].is_null());

    // L'ecran des paies du mois (pastilles) dit la meme chose, contrat par contrat.
    let paies = route(&racine, "etat-par-paie", json!({"mois": "2026-03"}));
    let sig = |n: &str| paies["paies"].as_array().unwrap().iter().find(|p| p["numero"] == json!(n)).unwrap()["etats"]["signature"].clone();
    assert!(sig(&na)["fait"].is_string() && sig(&na)["recu"].is_string(), "{}", sig(&na));
    assert!(sig(&nb)["fait"].is_null() && sig(&nb)["recu"].is_null(), "{}", sig(&nb));

    // B part a son tour : il attend SA signature, A n'attend plus rien.
    route(&racine, "demander-signature", json!({"mois": "2026-03", "contrats": [{"salarie": "BRIOCHE", "debut": debut, "numero": nb}], "motdepasse": ""}));
    let ap = route(&racine, "signature-apercu", json!({"mois": "2026-03"}));
    assert_eq!(ap["attendues"], json!(1), "{}", ap);
    assert!(ligne_de(&racine, &nb)["demande_le"].is_string());
    assert!(ligne_de(&racine, &nb)["recu_le"].is_null());
    assert!(ligne_de(&racine, &na)["recu_le"].is_string());
    // Le clic « contrat signe recu » de B (par son numero) ne note que B.
    route(&racine, "contrat-recu", json!({"mois": "2026-03", "salarie": "BRIOCHE", "debut": debut, "numero": nb, "recu": true}));
    let recus = lignes(&racine, "contrat-recu");
    assert_eq!(recus.len(), 2);
    assert_eq!(recus[1]["detail"]["numero"], json!(nb));
    assert!(ligne_de(&racine, &nb)["recu_le"].is_string());
}

#[test]
fn un_avenant_signe_se_note_avec_son_numero_et_part_sous_son_propre_sujet() {
    tour();
    let racine = copie(petite(), "avenant");
    let dm = demandes(&racine);
    let faux = nouvelle_boite();
    for (i, d) in dm.iter().enumerate() {
        faux.pose(&d.code, &format!("{}", i), je_signe(d, &format!("<m{}@x.invalid>", i)));
    }
    assert_eq!(releve(&racine, &faux, None).recues.len(), 3);
    // Le lieu du contrat de Brioche change : un avenant part a la signature.
    let mut s = lire(&racine, "paies/2026-03.json");
    s["engagements"][0]["lignes"][0]["lieu"] = json!("Theatre de la Ville");
    ecrire(&racine, "paies/2026-03.json", &s);
    route(&racine, "avenant-envoyer", json!({"salarie": "BRIOCHE", "debut": dm[0].debuts[0], "motdepasse": ""}));
    let av = demandes(&racine).into_iter().find(|d| d.code != dm[0].code && d.salarie == "BRIOCHE").expect("la demande d'avenant");
    let faux = nouvelle_boite();
    faux.pose(&av.code, "9", message(&av.adresse, "Re: Avenant", &corps_je_signe(&av.code), DATE_REPONSE, "<av@x.invalid>", false));
    let r = releve(&racine, &faux, Some("2026-03"));
    assert_eq!((r.attendues, r.recues.len(), r.incidents.len()), (1, 1, 0));
    let l = lignes(&racine, "contrat-recu").last().unwrap().clone();
    assert_eq!(l["detail"]["acte"], json!("avenant"));
    assert_eq!(l["detail"]["avenant"], json!(1));
    let m = faux.recus.borrow()[0].clone();
    assert_eq!(entete(&m, "Subject"), "Ton avenant signé - Ubu roi");
    assert!(texte(&m).contains("l'avenant n° 1 à ton contrat"), "{}", texte(&m));
    assert!(nom_des_pieces(&m)[0].starts_with("Avenant1_BRIOCHE_"), "{:?}", nom_des_pieces(&m));
}

// ================================================================== DEFAUT 1 du Python, corrige

/// DEFAUT DU PYTHON (07/10/2026, corrige) : un contrat signe que `contrat_recu` refuse (sa paie n'est plus dans la saisie du mois)
/// faisait lever TOUTE la releve (apres la preuve et le PDF, avant la ligne du journal) : les signatures suivantes n'etaient pas
/// relevees, et la releve suivante retombait sur le meme refus, pour toujours. Ici : un incident nomme, les autres sont relevees.
#[test]
fn defaut_1_un_contrat_signe_qui_ne_peut_pas_s_enregistrer_est_un_incident_les_autres_signatures_sont_relevees() {
    tour();
    let racine = copie(petite(), "defaut-1");
    let dm = demandes(&racine);
    // La paie de Brioche (le premier de la boucle) n'est plus dans la saisie.
    let mut s = lire(&racine, "paies/2026-03.json");
    let reste: Vec<Value> = s["engagements"].as_array().unwrap().iter().filter(|e| e["salarie"] != json!("Brioche")).cloned().collect();
    s["engagements"] = Value::Array(reste);
    ecrire(&racine, "paies/2026-03.json", &s);
    let faux = nouvelle_boite();
    for (i, d) in dm.iter().enumerate() {
        faux.pose(&d.code, &format!("{}", i), je_signe(d, &format!("<m{}@x.invalid>", i)));
    }
    let r = releve(&racine, &faux, Some("2026-03"));
    assert_eq!(r.recues.iter().map(|s| s.salarie.as_str()).collect::<Vec<_>>(), ["SERAPHIN", "LEMERCIER"], "les autres sont relevees");
    assert!(r.recues.iter().all(|s| s.renvoye));
    assert_eq!(r.incidents.len(), 1);
    assert_eq!(
        r.incidents[0],
        format!(
            "Fanchon Brioche : la signature de {} est arrivee mais le contrat du {} n'a pas pu etre enregistre (Aucune paie de BRIOCHE commencant le {} (n° BR1.26.02) dans la saisie enregistree de 2026-03.) ; il n'est pas note signe.",
            dm[0].code, dm[0].debuts[0], dm[0].debuts[0]
        )
    );
    // Brioche n'a ni ligne `contrat-recu`, ni contrat renvoye : il reste attendu.
    assert!(lignes(&racine, "contrat-recu").iter().all(|l| l["detail"]["salarie"] != json!("BRIOCHE") || !l["origine"].as_str().unwrap().starts_with("courriel")));
    assert_eq!(faux.partis(), 2);
    // La releve suivante : l'incident se redit, rien n'est renvoye, rien n'est bloque.
    let r2 = releve(&racine, &faux, Some("2026-03"));
    assert_eq!((r2.attendues, r2.recues.len(), r2.incidents.len()), (1, 0, 1));
    assert_eq!(faux.partis(), 2);
    // Le retour de la paie dans la saisie : la signature est enfin relevee.
    ecrire(&racine, "paies/2026-03.json", &lire(petite(), "paies/2026-03.json"));
    let r3 = releve(&racine, &faux, Some("2026-03"));
    assert_eq!((r3.recues.len(), r3.incidents.len()), (1, 0));
    assert_eq!(faux.partis(), 3);
}

#[test]
fn defaut_1_dans_un_courriel_groupe_le_contrat_refuse_est_un_incident_l_autre_est_note_et_le_renvoi_part() {
    tour();
    let racine = copie(grande(), "defaut-1-groupe");
    let dm = demandes(&racine);
    let g = dm.iter().find(|d| d.debuts.len() == 2).unwrap().clone();
    // Le second contrat du salarie n'est plus dans la saisie (on retire l'engagement qui commence a ce jour).
    let mut s = lire(&racine, "paies/2026-03.json");
    let sal = g.salarie.clone();
    let second = g.debuts[1].clone();
    let reste: Vec<Value> = s["engagements"].as_array().unwrap().iter().filter(|e| !(e["salarie"].as_str().unwrap().to_uppercase() == sal && e["debut"] == json!(second))).cloned().collect();
    s["engagements"] = Value::Array(reste);
    ecrire(&racine, "paies/2026-03.json", &s);
    let faux = nouvelle_boite();
    faux.pose(&g.code, "1", je_signe(&g, "<a@x.invalid>"));
    let r = releve(&racine, &faux, Some("2026-03"));
    assert_eq!(r.recues.len(), 1);
    assert!(r.recues[0].renvoye);
    assert_eq!(r.incidents.len(), 1);
    assert!(r.incidents[0].contains(&format!("le contrat du {} n'a pas pu etre enregistre", second)), "{}", r.incidents[0]);
    let notes: Vec<Value> = lignes(&racine, "contrat-recu").into_iter().filter(|l| l["detail"]["code"] == json!(g.code)).collect();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0]["detail"]["debut"], json!(g.debuts[0]));
}

// ================================================================== la route

#[test]
fn la_route_signatures_relever_rend_le_json_des_appelants_avec_un_faux_client() {
    tour();
    let racine = copie(petite(), "route");
    let dm = demandes(&racine);
    let mut faux = nouvelle_boite();
    faux.pose(&dm[0].code, "1", je_signe(&dm[0], "<a@x.invalid>"));
    let v = rs::route_avec(&options(&racine), &contexte(&racine), "signatures-relever", &json!({"mois": "2026-03", "qui": "ligne de commande"}), Some(&mut faux)).unwrap();
    let cles: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(cles, ["quand", "attendues", "recues", "incidents", "resume"]);
    let c: Vec<&str> = v["recues"][0].as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(c, ["mois", "salarie", "nom", "code", "adresse", "quand", "contrats", "fichier_signe", "preuve", "renvoye"]);
    assert_eq!(lignes(&racine, "contrat-recu").last().unwrap()["qui"], json!("ligne de commande"));
    // Un `qui` absent ou vide vaut `interface` ; une route inconnue est refusee.
    let racine2 = copie(petite(), "route-2");
    let dm2 = demandes(&racine2);
    let mut faux = nouvelle_boite();
    faux.pose(&dm2[0].code, "1", je_signe(&dm2[0], "<a@x.invalid>"));
    rs::route_avec(&options(&racine2), &contexte(&racine2), "signatures-relever", &json!({"qui": ""}), Some(&mut faux)).unwrap();
    assert_eq!(lignes(&racine2, "contrat-recu").last().unwrap()["qui"], json!("interface"));
    assert!(rs::route(&options(&racine2), &contexte(&racine2), "signatures-inconnue", &json!({})).is_err());
}

// ================================================================== la demonstration

/// Les demandes datent d'hier : plus vieilles que le delai de la demonstration (20 s), plus jeunes que les sept
/// jours du dernier cas de `la_boite_de_la_demonstration_rejoue...`. Une date en dur (le 07/10/2026, jusqu'au
/// 14/10/2026) a fait tomber ce cas le jour ou les sept jours ont ete depasses.
fn vieillir(racine: &Path) {
    let hier = paie::releve_retours::iso_de_naif(paie::releve_retours::maintenant_naif() - 86_400);
    let mut j = journal(racine);
    for l in j.iter_mut() {
        if l["quoi"] == json!("contrat-signature") {
            l["quand"] = json!(hier);
        }
    }
    ecrire(racine, "journal/depots.json", &Value::Array(j));
}

#[test]
fn en_demonstration_les_salaries_signent_apres_le_delai_avec_le_bouton_de_leur_courriel() {
    tour();
    // Jeune : rien ne repond encore (delai de 20 s).
    let jeune = copie(petite(), "demo-jeune");
    let r = rs::relever_signatures_demonstration(&options(&jeune), &contexte(&jeune), rs::DELAI_DEMONSTRATION).unwrap();
    assert_eq!((r.attendues, r.recues.len()), (3, 0));
    // Vieux : tous signent, la vraie releve lit, contresigne et renvoie (dans courriels-simules/).
    let racine = copie(petite(), "demo-vieux");
    vieillir(&racine);
    let avant = std::fs::read_dir(racine.join("courriels-simules")).unwrap().count();
    let r = rs::relever_signatures_demonstration(&options(&racine), &contexte(&racine), rs::DELAI_DEMONSTRATION).unwrap();
    assert_eq!((r.attendues, r.recues.len(), r.incidents.len()), (3, 3, 0));
    assert!(r.recues.iter().all(|s| s.renvoye && s.adresse.ends_with("@exemple.invalid")));
    assert_eq!(std::fs::read_dir(racine.join("courriels-simules")).unwrap().count(), avant + 3);
    for l in lignes(&racine, "contrat-recu").iter().rev().take(3) {
        assert_eq!(l["qui"], json!("demonstration"));
    }
    // Par la route, sans client : la boite de la demonstration, comme l'ouverture (`veille`).
    let r2 = copie(petite(), "demo-route");
    vieillir(&r2);
    let v = rs::route(&options(&r2), &contexte(&r2), "signatures-relever", &json!({"qui": "veille"})).unwrap();
    assert_eq!((v["attendues"].as_u64(), v["recues"].as_array().unwrap().len()), (Some(3), 3));
}

#[test]
fn la_boite_de_la_demonstration_rejoue_le_bouton_de_la_demande_a_defaut_la_reponse_de_repli() {
    tour();
    let racine = copie(petite(), "demo-boite");
    vieillir(&racine);
    let dm = demandes(&racine);
    let mut boite = rs::boite_des_signatures(&contexte(&racine), 20).unwrap();
    let ids = boite.rechercher(&format!("\"{}\"", dm[0].code), 10).unwrap();
    assert_eq!(ids, [dm[0].code.clone()]);
    let brut = boite.message_brut(&ids[0]).unwrap();
    assert_eq!(entete(&brut, "From"), dm[0].adresse);
    assert!(entete(&brut, "Message-ID").ends_with("@demonstration.puppetpaie>"));
    assert!(texte(&brut).contains(&format!("Code de signature : {}", dm[0].code)));
    assert!(boite.rechercher("\"SIG-INCONNU\"", 10).unwrap().is_empty());
    // Sans le courriel de la demande : la reponse de repli, « Re: contrat ».
    std::fs::remove_dir_all(racine.join("courriels-simules")).unwrap();
    let mut boite = rs::boite_des_signatures(&contexte(&racine), 20).unwrap();
    let brut = boite.message_brut(&dm[0].code).unwrap();
    assert_eq!(entete(&brut, "Subject"), "Re: contrat");
    // Delai non atteint : aucune reponse.
    let mut boite = rs::boite_des_signatures(&contexte(&racine), 7 * 24 * 3600).unwrap();
    assert!(boite.rechercher(&format!("\"{}\"", dm[0].code), 10).unwrap().is_empty());
}

#[test]
fn la_garde_de_la_demonstration_ne_joue_rien_dans_une_compagnie_ordinaire() {
    tour();
    let racine = copie(petite(), "demo-garde");
    vieillir(&racine);
    std::fs::remove_file(racine.join("COPIE-DE-DEVELOPPEMENT.json")).unwrap();
    let j0 = journal(&racine);
    let r = rs::relever_signatures_demonstration(&options(&racine), &contexte(&racine), 0).unwrap();
    assert_eq!((r.attendues, r.recues.len(), r.incidents.len()), (0, 0, 0));
    assert_eq!(journal(&racine), j0);
    assert_eq!(r.resume(), "");
}

/// CHOIX 3 de l'utilisateur (07/10/2026) : en demonstration, une adresse de salarie avec nom d'affichage est acceptee, pour les
/// signatures comme pour les fiches (le Python l'ecrit telle quelle ; le Rust ne savait ecrire que des adresses simples).
#[test]
fn choix_3_une_adresse_de_salarie_avec_nom_d_affichage_est_acceptee_en_demonstration() {
    tour();
    let racine = copie(petite(), "demo-affichage");
    vieillir(&racine);
    let mut j = journal(&racine);
    for l in j.iter_mut() {
        if l["quoi"] == json!("contrat-signature") {
            for c in l["detail"]["contrats"].as_array_mut().unwrap() {
                c["adresse"] = json!(format!("Brioche <{}>", c["adresse"].as_str().unwrap()));
            }
        }
    }
    ecrire(&racine, "journal/depots.json", &Value::Array(j));
    let dm = demandes(&racine);
    // La boite se fabrique (avant le choix : `Adresse illisible`) ; la reponse porte le `From` tel que le Python l'ecrit.
    let mut boite = rs::boite_des_signatures(&contexte(&racine), 20).unwrap();
    let brut = boite.message_brut(&dm[0].code).unwrap();
    assert!(brut.starts_with(format!("From: {}\n", dm[0].adresse).as_bytes()), "{}", String::from_utf8_lossy(&brut[..80]));
    // Un nom d'affichage non ASCII : mot encode, comme le Python.
    let octets = paie_reponse("José Brûlé <f@x.invalid>");
    assert!(octets.starts_with(b"From: =?utf-8?b?Sm9zw6kgQnLDu2zDqQ==?= <f@x.invalid>\n"), "{}", String::from_utf8_lossy(&octets[..90]));
    // Et la releve s'en sert : la reponse vient de l'adresse nue, le journal garde l'adresse avec son nom (incident dit, comme le Python).
    let r = rs::relever_signatures_demonstration(&options(&racine), &contexte(&racine), 20).unwrap();
    assert_eq!(r.recues.len(), 0);
    assert_eq!(r.incidents.len(), 3);
    assert!(r.incidents[0].contains("vient de fanchon.brioche@exemple.invalid et non de Brioche <fanchon.brioche@exemple.invalid>"), "{}", r.incidents[0]);
}

fn paie_reponse(de: &str) -> Vec<u8> {
    paie::simulation_courriels::reponse_jouee(de, "boite@exemple.invalid", "Re: x", "c", "essai").unwrap()
}

// ================================================================== les petites pieces

#[test]
fn le_resume_et_le_json_d_une_releve_vide_ou_sans_arrivee() {
    let vide = rs::ReleveSignatures { quand: "2026-10-07T10:00:00".into(), attendues: 0, recues: vec![], incidents: vec![] };
    assert_eq!(vide.resume(), "");
    let attente = rs::ReleveSignatures { attendues: 2, ..vide.clone() };
    assert_eq!(attente.resume(), "2 signature(s) attendue(s), aucune arrivée");
    assert_eq!(attente.en_json()["recues"], json!([]));
    assert_eq!(attente.en_json()["resume"], json!("2 signature(s) attendue(s), aucune arrivée"));
}

// ================================================================== un message mal forme se lit quand meme

#[test]
fn un_bloc_d_entetes_rompu_se_recoupe_avant_la_ligne_rompue() {
    use paie::courriel_lecture::lisible_comme_python;
    // Sain : lu tel quel.
    let sain = b"From: a@x.invalid\nSubject: x\n\nbonjour\n";
    assert!(matches!(lisible_comme_python(sain), std::borrow::Cow::Borrowed(_)));
    // Une ligne qui n'est pas un en-tete : les en-tetes finissent avant elle, tout le reste est le corps.
    let rompu = b"From: a@x.invalid\nGARBAGE LINE\nSubject: x\n\nCode SIG-ABCDEF\n";
    let lu = lisible_comme_python(rompu);
    let m = mailparse::parse_mail(&lu).unwrap();
    assert_eq!(mailparse::MailHeaderMap::get_first_value(m.headers.as_slice(), "From").as_deref(), Some("a@x.invalid"));
    assert!(m.get_body().unwrap().starts_with("GARBAGE LINE\nSubject: x\n\nCode SIG-ABCDEF"), "{:?}", m.get_body());
    // Rien qui ressemble a un en-tete : aucun en-tete, tout est le corps.
    let m2 = lisible_comme_python("\u{0}\u{ff} pas un courriel SIG-ABCDEF".as_bytes());
    let p = mailparse::parse_mail(&m2).unwrap();
    assert!(p.headers.is_empty() && p.get_body().unwrap().contains("SIG-ABCDEF"));
    // Une suite de ligne (espace, tabulation) et un `From ` d'mbox sont des en-tetes.
    assert!(matches!(lisible_comme_python(b"From moi Sat Oct 3\nSubject: un\n suite\n\nx"), std::borrow::Cow::Borrowed(_)));
}

#[test]
fn un_corps_base64_abime_se_decode_au_mieux_ou_reste_tel_quel() {
    use paie::simulation_courriels::decoder_b64_python as d;
    assert_eq!(d(b"Q29kZSBTSUc="), b"Code SIG");
    assert_eq!(d(b"Q29k\r\nZSBT\r\nSUc="), b"Code SIG", "les lignes se joignent");
    assert_eq!(d(b"Q29kZSBTSUc"), b"Code SIG", "bourrage manquant : complete");
    assert_eq!(d(b"Q29k!!ZSBTSUc="), b"Code SIG", "signes hors alphabet : ignores");
    // Longueur impossible (1 de plus qu'un multiple de 4) : le texte lui-meme, lignes jointes.
    assert_eq!(d(b"!!!!Code SIG-ABCDEF\n"), b"!!!!Code SIG-ABCDEF");
}

#[test]
/// Choix assume : un message mal forme (en-tete rompu, base64 invalide) qui porte le code vaut signature.
fn un_message_mal_forme_qui_porte_le_code_vaut_signature() {
    tour();
    let racine = copie(petite(), "lecture-indulgente");
    let dm = demandes(&racine);
    let faux = nouvelle_boite();
    // Un en-tete rompu avant le corps : l'adresse est lue, le code est dans le corps.
    faux.pose(&dm[0].code, "1", format!("From: {}\nGARBAGE LINE\nSubject: x\n\nCode {}\n", dm[0].adresse, dm[0].code).into_bytes());
    // Un corps base64 invalide dont le texte brut porte le code.
    faux.pose(&dm[1].code, "2", format!("From: {}\nContent-Type: text/plain; charset=utf-8\nContent-Transfer-Encoding: base64\n\n!!!!Code {}\n", dm[1].adresse, dm[1].code).into_bytes());
    // Du charabia sans aucun en-tete : lu comme un corps, sans expediteur (incident).
    faux.pose(&dm[2].code, "3", format!("\u{0}\u{ff}\u{fe} pas un courriel {}", dm[2].code).into_bytes());
    let r = releve(&racine, &faux, None);
    assert_eq!(r.recues.len(), 2);
    assert_eq!(r.incidents.len(), 1);
    assert!(r.incidents[0].contains("et non de"), "{}", r.incidents[0]);
}
