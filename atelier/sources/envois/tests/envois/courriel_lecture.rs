//! La lecture d'un message recu (`courriel_lecture`) et le questionnaire de la fiche (`fiche_questionnaire`), contre
//! `envois/tests/donnees/courriel_lecture/cas.json` : 229 messages (texte propre, adresse, Message-ID, date, reponses lues, fiche
//! produite), 25 dates, 26 pays, le questionnaire mot pour mot. `PUPPETPAIE_REECRIRE_ORACLES=1` le reecrit depuis le
//! moteur (les `eml` restent).
//!
//! La ligne du sexe se lit apres son dernier deux-points ; « -----Original Message----- » coupe la citation, en LF comme
//! en CRLF.
//!
//! Heure de Paris, quel que soit le fuseau du poste ; un en-tete `Date` illisible rend l'heure de la releve (`MAINTENANT`).
//! Aucun reseau.

use crate::outils::oracle;
use envois::courriel_lecture::{adresse_de, date_de, identifiant_de, texte_propre};
use documents::fiche_questionnaire as fq;
use socle::base64::lire as base64_lire;
use serde_json::{json, Map, Value};

#[test]
fn la_lecture_des_reponses_et_le_questionnaire_rendent_l_oracle() {
    let mut c: Value = serde_json::from_slice(&std::fs::read(crate::outils::chemin_oracle("courriel_lecture/cas.json")).unwrap()).unwrap();
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
