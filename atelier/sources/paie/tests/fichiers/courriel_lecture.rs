//! La lecture d'un message recu (`courriel_lecture`) et le questionnaire de la fiche (`fiche_questionnaire`), contre
//! `tests/donnees/courriel_lecture/cas.json` : 229 messages (texte propre, adresse, Message-ID, date, reponses lues, fiche
//! produite), 25 dates, 26 pays, le questionnaire mot pour mot. `PUPPETPAIE_REECRIRE_ORACLES=1` le reecrit depuis le
//! moteur (les `eml` restent).
//!
//! Deux ecarts avec l'ancien Python sont voulus (Leandre, 07/10/2026) : la ligne du sexe se lit apres son dernier
//! deux-points, et « -----Original Message----- » coupe la citation aussi en CRLF.
//!
//! Heure de Paris, quel que soit le fuseau du poste ; un en-tete `Date` illisible rend l'heure de la releve (`MAINTENANT`).
//! Aucun reseau.

use std::path::{Path, PathBuf};

use crate::outils::oracle;
use paie::courriel_lecture::{adresse_de, date_de, identifiant_de, texte_propre};
use paie::fiche_questionnaire as fq;
use paie::retours::base64_lire;
use serde_json::{json, Map, Value};

fn chemin() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("donnees").join("courriel_lecture").join("cas.json")
}

#[test]
fn la_lecture_des_reponses_et_le_questionnaire_rendent_l_oracle() {
    let mut c: Value = serde_json::from_slice(&std::fs::read(chemin()).unwrap()).unwrap();
    assert!(c["messages"].as_array().unwrap().len() > 200, "le fichier de cas est complet");
    for cas in c["messages"].as_array_mut().unwrap() {
        let brut = base64_lire(cas["eml"].as_str().unwrap()).unwrap();
        let m = mailparse::parse_mail(&brut).unwrap_or_else(|e| panic!("{} : {}", cas["nom"], e));
        let propre = texte_propre(&m);
        let lues = fq::lire_reponses(&propre);
        let reponses: Map<String, Value> = fq::QUESTIONS.iter().filter_map(|(k, _, _)| lues.get(k).map(|v| (k.to_string(), json!(v)))).collect();
        assert_eq!(reponses.len(), lues.len(), "{} : une reponse hors du questionnaire", cas["nom"]);
        let (fiche, illisible) = fq::fiche_depuis_reponses(&lues);
        cas["texte"] = json!(propre);
        cas["adresse"] = json!(adresse_de(&m));
        cas["message_id"] = json!(identifiant_de(&m));
        cas["date"] = json!(date_de(&m, "MAINTENANT"));
        cas["reponses"] = Value::Object(reponses);
        cas["fiche"] = Value::Object(fiche);
        cas["illisible"] = json!(illisible);
    }
    for d in c["dates"].as_array_mut().unwrap() {
        d["attendu"] = json!(fq::date_lue(d["texte"].as_str().unwrap()));
    }
    for p in c["pays"].as_array_mut().unwrap() {
        p["attendu"] = json!(fq::pays_lu(p["texte"].as_str().unwrap()));
    }
    c["questionnaire"] = json!(fq::questionnaire("FIC-ABCDEF"));
    c["sujet"] = json!(fq::SUJET);
    c["corps"] = json!(fq::CORPS);
    c["html"] = json!(fq::HTML);
    c["alphabet"] = json!(fq::ALPHABET_CODE);
    c["questions"] = json!(fq::QUESTIONS);
    oracle("courriel_lecture/cas.json", "", &c);
}

#[test]
fn un_code_de_fiche_a_la_forme_attendue() {
    for _ in 0..200 {
        let code = fq::code_fiche();
        assert!(fq::est_un_code(&code), "{}", code);
        assert_eq!(code.len(), 10);
        assert!(code.starts_with("FIC-"));
        assert!(code[4..].chars().all(|c| fq::ALPHABET_CODE.contains(c)));
    }
    assert!(!fq::est_un_code("FIC-ABCDE"));
    assert!(!fq::est_un_code("FIC-ABCDEI"));
    assert!(!fq::est_un_code("SIG-ABCDEF"));
}

#[test]
fn un_message_en_crlf_coupe_la_citation_a_l_anglaise() {
    // « -----Original Message----- » coupe la citation, meme en CRLF.
    let brut = b"From: zoe@exemple.invalid\r\nSubject: Re\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nPr\xc3\xa9nom : Zo\xc3\xa9\r\nCommune : Lyon\r\n\r\n-----Original Message-----\r\nAdresse (num\xc3\xa9ro et rue) : 1 rue de la Citation\r\nFrom: Le Fil tendu <boite@exemple.invalid>\r\n";
    let m = mailparse::parse_mail(brut).unwrap();
    let t = texte_propre(&m);
    assert!(!t.contains("Citation"), "{:?}", t);
    assert!(t.contains("Commune : Lyon"));
    let rep = fq::lire_reponses(&t);
    assert!(!rep.contains_key("adresse"));
}

#[test]
fn la_ligne_du_sexe_se_lit_apres_son_dernier_deux_points() {
    // « femme ou homme ? (facultatif) : homme » se lit « homme », pas « femme ou homme ? (facultatif) : homme ».
    let rep = fq::lire_reponses("Sans numéro de sécurité sociale : femme ou homme ? (facultatif) : homme\n");
    assert_eq!(rep.get("sexe").map(String::as_str), Some("homme"));
    let (fiche, illisible) = fq::fiche_depuis_reponses(&rep);
    assert_eq!(fiche.get("sexe").and_then(Value::as_str), Some("1"));
    assert!(illisible.is_empty());
    // Une ligne laissee vide ne vaut rien.
    let vide = fq::lire_reponses("Sans numéro de sécurité sociale : femme ou homme ? (facultatif) : \n");
    assert!(!vide.contains_key("sexe"));
    let (fiche, _) = fq::fiche_depuis_reponses(&vide);
    assert!(!fiche.contains_key("sexe"));
    // La personne simplifie la ligne : le prefixe du libelle la reconnait toujours.
    let simple = fq::lire_reponses("Sans numéro de sécurité sociale : femme\n");
    assert_eq!(fq::fiche_depuis_reponses(&simple).0.get("sexe").and_then(Value::as_str), Some("2"));
    // Avec un NIR, la ligne du sexe ne sert pas.
    let avec_nir = fq::lire_reponses("Numéro de sécurité sociale (15 chiffres) : 291036912345678\nSans numéro de sécurité sociale : femme ou homme ? (facultatif) : homme\n");
    assert!(!fq::fiche_depuis_reponses(&avec_nir).0.contains_key("sexe"));
}

#[test]
fn un_charset_inconnu_ou_absent_garde_les_lignes_en_ascii() {
    // Les accents peuvent sortir de travers, les lignes en ASCII se lisent.
    for entete in ["text/plain", "text/plain; charset=x-inconnu-9"] {
        let brut = format!("From: zoe@exemple.invalid\r\nContent-Type: {}\r\nContent-Transfer-Encoding: 8bit\r\n\r\nCommune : Lyon\r\nNationalité : française\r\n", entete);
        let t = texte_propre(&mailparse::parse_mail(brut.as_bytes()).unwrap());
        assert!(t.contains("Commune : Lyon"), "{} : {:?}", entete, t);
    }
}

#[test]
fn les_dates_d_en_tete_en_heure_de_paris() {
    let date = |entete: &str| {
        let brut = format!("From: a@b.c\r\nDate: {}\r\n\r\nx\r\n", entete);
        date_de(&mailparse::parse_mail(brut.as_bytes()).unwrap(), "MAINTENANT")
    };
    assert_eq!(date("Sat, 03 Oct 2026 10:15:00 +0200"), "2026-10-03T10:15:00");
    assert_eq!(date("Fri, 02 Oct 2026 23:30:00 -0500"), "2026-10-03T06:30:00");
    assert_eq!(date("Sat, 03 Oct 2026 10:15"), "2026-10-03T10:15:00");
    assert_eq!(date("Sat, 03 Oct 2026 10:15:00 (CEST)"), "2026-10-03T10:15:00");
    // RFC 5322 § 3.3 (`ietf/LISEZ-MOI.md`) : « -0000 » est une heure UT comme « +0000 » (10:15 UT = 12:15 a Paris en
    // octobre, 11:15 en janvier), et non l'heure de Paris.
    assert_eq!(date("Sat, 03 Oct 2026 10:15:00 -0000"), "2026-10-03T12:15:00");
    assert_eq!(date("Sat, 03 Oct 2026 10:15:00 +0000"), "2026-10-03T12:15:00");
    assert_eq!(date("Mon, 05 Jan 2026 10:15:00 -0000"), "2026-01-05T11:15:00");
    assert_eq!(date("Sat, 03 Oct 2026 23:30:00 -0000"), "2026-10-04T01:30:00");
    assert_eq!(date("Sat, 03 Oct 2026 10:15:00 Z"), "2026-10-03T12:15:00");
    // RFC 5322 § 4.3 : un nom de fuseau inconnu et les zones militaires « SHOULD be considered equivalent to "-0000" » --
    // de l'UT ; les zones americaines gardent leur decalage. Sans fuseau, ou un commentaire seul : l'heure locale (plus haut).
    assert_eq!(date("Sat, 03 Oct 2026 10:15:00 CEST"), "2026-10-03T12:15:00");
    assert_eq!(date("Sat, 03 Oct 2026 10:15:00 a"), "2026-10-03T12:15:00");
    assert_eq!(date("Sat, 03 Oct 2026 10:15:00 EST"), "2026-10-03T17:15:00");
    assert_eq!(date("Mon, 05 Jan 2026 10:15:00 +0100"), "2026-01-05T10:15:00");
    // Illisible : l'heure de la releve.
    for illisible in ["hier soir", "", "Sat, 31 Feb 2026 10:15:00 +0200", "Sat, 03 Oct 2026 24:00:00 +0200", "Sat, 03 Oct 2026 10:15:60 +0200", "Sat, 03 Oct 2026 10:15:00 +2400"] {
        assert_eq!(date(illisible), "MAINTENANT", "{:?}", illisible);
    }
    assert_eq!(date_de(&mailparse::parse_mail(b"From: a@b.c\r\n\r\nx").unwrap(), "MAINTENANT"), "MAINTENANT");
}
