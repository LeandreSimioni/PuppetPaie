//! LE CDD D'UN ARTISTE PAYE AU MOIS, de la validation a son attestation (Les Faisceaux : FULLER, artiste choregraphique,
//! en CDD de droit commun de trois mois ; SVOBODA en CDI a cote).
//! - Un motif de recours se declare par son code (S21.G00.40.021 : une liste de codes, cahier technique DSN 2026.1,
//!   p. 194-195, `atelier/reference/norme-dsn/texte/bloc-S21.G00.40.txt`) : un motif que la nature n'a pas se refuse a
//!   la validation, et rien n'est ecrit.
//! - Une paie ne lit, des engagements de son mois, que ceux de son salarie : le refus du calcul de l'un nomme celui-la,
//!   et n'empeche pas la paie de l'autre.
//! - L'artiste declare a la caisse des Conges Spectacles n'a pas d'indemnite compensatrice de l'employeur : « Les
//!   articles L. 3141-28 et L. 3141-29 ne sont pas applicables lorsque l'employeur est tenu d'adherer a une caisse de
//!   conges » (C. trav. L. 3141-30, `atelier/reference/sources/code-du-travail/articles-20261102.md`).
//! - L'indemnite de fin de contrat n'est due que si « les relations contractuelles de travail ne se poursuivent pas par
//!   un contrat a duree indeterminee » (C. trav. L1243-8, meme fichier).
//! - L'annexe 10 range « les artistes [...] engages au titre d'un contrat de travail a duree determinee » (art. 1er, § 2,
//!   `atelier/reference/sources/plafond-intermittents/unedic-annexe10-convention-20241115.txt`) : leur attestation
//!   Cinema Spectacle compte leurs heures, et se signe.

use serde_json::json;

use crate::outils::rejeu::rejouer;
use crate::outils::{refus, reussi, route};

#[test]
fn de_la_validation_a_l_attestation() {
    let d = crate::outils::dossier_de_test("cdd_d_artiste", "fuller");
    // Les Faisceaux le 20/02/2026 : le CDI de SVOBODA valide, janvier paye.
    let (_, c) = rejouer("faisceaux-sans-civilite", &d);

    let saisie = |motif: &str| {
        json!({"mois": "2026-03", "saisie": {"salarie": "FULLER", "nature": "cdd", "debut": "2026-03-02", "fin": "2026-05-29",
            "salaire_mensuel": 2500, "quotite_contrat": 151.67, "metier": "ART040", "motif_recours": motif}})
    };
    let (_, phrase) = refus(&route(&c, "valider-engagement", saisie("accroissement"), "2026-02-25"));
    assert!(phrase.contains("02 ("), "la phrase dit les motifs a choisir : {}", phrase);
    let ecrit = c.join("engagements").join("2026-03.json");
    assert!(!ecrit.exists(), "un motif refuse : rien n'est ecrit");
    reussi(&route(&c, "valider-engagement", saisie("02"), "2026-02-25"));

    // L'engagement ecrit de FULLER, abime a la main : sa paie de mai ne se calcule pas, celle de SVOBODA s'ecrit.
    let texte = std::fs::read_to_string(&ecrit).unwrap();
    assert!(texte.contains("\"salaire_mensuel\": 2500"), "le salaire ecrit : {}", texte);
    std::fs::write(&ecrit, texte.replace("\"salaire_mensuel\": 2500", "\"salaire_mensuel\": \"deux mille\"")).unwrap();
    let (_, phrase) = refus(&route(&c, "apercu", json!({"mois": "2026-05"}), "2026-05-20"));
    assert!(phrase.contains("FULLER") && !phrase.contains("SVOBODA"), "le refus est celui de FULLER : {}", phrase);
    assert!(std::fs::read_to_string(c.join("paies").join("2026-05.json")).unwrap().contains("SV1.26.01"), "la paie de mai de SVOBODA");
    std::fs::write(&ecrit, texte).unwrap();

    let ap = route(&c, "apercu", json!({"mois": "2026-05"}), "2026-05-20");
    let paies = reussi(&ap)["paies"].as_array().unwrap().clone();
    let fuller = paies.iter().find(|p| p.to_string().contains("FULLER")).expect("la paie de mai de FULLER");
    assert!(!fuller.to_string().contains("indemnite_compensatrice_conges"), "une indemnite compensatrice : {}", fuller);
    assert!(fuller.to_string().contains("indemnite_precarite"), "l'indemnite de fin de contrat : {}", fuller);

    // Un CDI le lundi qui suit le terme (un vendredi) : la relation se poursuit, l'indemnite de fin de contrat tombe.
    let cdi = json!({"mois": "2026-06", "saisie": {"salarie": "FULLER", "nature": "cdi", "debut": "2026-06-01", "salaire_mensuel": 2500,
        "quotite_contrat": 151.67, "metier": "ART040"}});
    reussi(&route(&c, "valider-engagement", cdi, "2026-05-20"));
    let ap = route(&c, "apercu", json!({"mois": "2026-05"}), "2026-05-20");
    let fuller = reussi(&ap)["paies"].as_array().unwrap().iter().find(|p| p.to_string().contains("FULLER")).cloned().expect("FULLER");
    assert!(!fuller.to_string().contains("indemnite_precarite"), "L1243-8 : un CDI suit le CDD : {}", fuller);

    // La DSN de mai porte la fin ; l'horloge range l'attestation que France Travail en edite, signee.
    let civilite = json!({"nom": "entreprise", "confirme": true, "donnees": {"declarant_dsn": {"civilite": "02", "nom_prenom": "APPIA ADOLPHE",
        "tel": "0100000052", "email": "DIRECTION@LES-FAISCEAUX.INVALID", "qualite": "04"}}});
    reussi(&route(&c, "enregistrer-fiche", civilite, "2026-05-20"));
    let fait = reussi(&route(&c, "horloge-rattraper", json!({"aujourdhui": "2026-06-20", "retenir": false}), "2026-06-20")).to_string();
    assert!(!fait.contains("NON SIGNEE"), "l'attestation de FULLER : {}", fait);
    assert!(c.join("sorties").join("2026-05").join("envoi").join("AER_FULLER_Mai2026.pdf").exists(), "l'attestation signee, rangee : {}", fait);
}
