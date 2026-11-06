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
//!   Cinema Spectacle compte leurs heures, et se signe ; le certificat Conges Spectacles est remis a l'issue du contrat
//!   (fiche Audiens), et l'indemnite de fin de contrat compte les mois d'avant meme quand ils n'ont pas ete lus.

use serde_json::json;

use crate::outils::rejeu::rejouer;
use crate::outils::{refus, reussi, route, valider};

#[test]
fn de_la_validation_a_l_attestation() {
    let d = crate::outils::dossier_de_test("cdd_d_artiste", "fuller");
    // Les Faisceaux le 20/02/2026 : le CDI de SVOBODA valide, janvier paye.
    let (_, c) = rejouer("faisceaux-sans-civilite", &d);

    let saisie = |motif: &str| {
        json!({"salarie": "FULLER", "nature": "cdd", "debut": "2026-03-02", "fin": "2026-05-29",
            "salaire_mensuel": 2500, "quotite_contrat": 151.67, "metier": "ART040", "motif_recours": motif})
    };
    let (_, phrase) = refus(&valider(&c, "2026-03", None, &saisie("accroissement"), "2026-02-25"));
    assert!(phrase.contains("02 ("), "la phrase dit les motifs a choisir : {}", phrase);
    // Les changements d'un appel s'ecrivent tout ou rien : un motif refuse, la carte ne s'ecrit pas.
    assert!(engagement::stock::vue_du_mois(&c, "2026-03").unwrap().is_none_or(|v| v.engagements.iter().all(|e| e.salarie.as_deref() != Some("FULLER"))), "un motif refuse : rien n'est ecrit");
    reussi(&valider(&c, "2026-03", None, &saisie("02"), "2026-02-25"));
    // Le contrat CDD se fabrique depuis l'engagement valide : un modele qui tairait une mention de L1242-12 se refuse
    // (`refuser_les_mentions_absentes`), le PDF imprime porte donc la nature, le motif de recours et le terme.
    let contrat = json!({"mois": "2026-03", "contrats": [{"salarie": "FULLER", "debut": "2026-03-02"}]});
    reussi(&route(&c, "contrats-imprimer", contrat, "2026-02-25"));
    assert!(c.join("sorties/2026-03/envoi/Contrat_FULLER_FU2.26.01_Mars2026.pdf").exists(), "le contrat CDD de FULLER");

    // La fiche de FULLER dit une profession de deduction forfaitaire specifique que la compagnie n'a pas choisie : sa
    // paie de mai ne se calcule pas (`calcul::deduction_forfaitaire`), le refus le nomme, et celle de SVOBODA s'ecrit.
    // La profession se retire depuis le debut de son contrat.
    let dfs = |profession: &str, depuis: &str| {
        json!({"nom": "salarie", "cle": "FULLER", "confirme": true, "depuis": depuis, "donnees": {"deduction_forfaitaire": profession}})
    };
    reussi(&route(&c, "enregistrer-fiche", dfs("artiste_choregraphique", "2026-05-01"), "2026-05-20"));
    let (_, phrase) = refus(&route(&c, "apercu", json!({"mois": "2026-05", "modifications": []}), "2026-05-20"));
    assert!(phrase.contains("FULLER") && !phrase.contains("SVOBODA"), "le refus est celui de FULLER : {}", phrase);
    assert!(std::fs::read_to_string(c.join("paies").join("2026-05.json")).unwrap().contains("SV1.26.01"), "la paie de mai de SVOBODA");
    reussi(&route(&c, "enregistrer-fiche", dfs("", "2026-03-02"), "2026-05-20"));

    let ap = route(&c, "apercu", json!({"mois": "2026-05", "modifications": []}), "2026-05-20");
    let paies = reussi(&ap)["paies"].as_array().unwrap().clone();
    let fuller = paies.iter().find(|p| p.to_string().contains("FULLER")).expect("la paie de mai de FULLER");
    assert!(!fuller.to_string().contains("indemnite_compensatrice_conges"), "une indemnite compensatrice : {}", fuller);
    let precarite = |p: &serde_json::Value| p["remuneration"].as_array().unwrap().iter().find(|l| l["rubrique"] == "indemnite_precarite").map(|l| l["montant"].clone());
    let avant = precarite(fuller).expect("l'indemnite de fin de contrat");
    // Mai lu avant mars et avril : la remuneration totale du contrat compte deja leurs paies (L1243-8).
    for mois in ["2026-03", "2026-04"] {
        reussi(&route(&c, "apercu", json!({"mois": mois, "modifications": []}), "2026-05-20"));
    }
    let ap = route(&c, "apercu", json!({"mois": "2026-05", "modifications": []}), "2026-05-20");
    let fuller = reussi(&ap)["paies"].as_array().unwrap().iter().find(|p| p.to_string().contains("FULLER")).cloned().expect("FULLER");
    assert_eq!(precarite(&fuller), Some(avant), "la precarite de mai ne change pas quand mars et avril sont lus");

    // Un CDI le lundi qui suit le terme (un vendredi) : la relation se poursuit, l'indemnite de fin de contrat tombe.
    let cdi = json!({"salarie": "FULLER", "nature": "cdi", "debut": "2026-06-01", "salaire_mensuel": 2500,
        "quotite_contrat": 151.67, "metier": "ART040"});
    reussi(&valider(&c, "2026-06", None, &cdi, "2026-05-20"));
    let ap = route(&c, "apercu", json!({"mois": "2026-05", "modifications": []}), "2026-05-20");
    let fuller = reussi(&ap)["paies"].as_array().unwrap().iter().find(|p| p.to_string().contains("FULLER")).cloned().expect("FULLER");
    assert!(!fuller.to_string().contains("indemnite_precarite"), "L1243-8 : un CDI suit le CDD : {}", fuller);

    // La DSN de mai porte la fin ; l'horloge range l'attestation que France Travail en edite, signee.
    let civilite = json!({"nom": "entreprise", "confirme": true, "donnees": {"declarant_dsn": {"civilite": "02", "nom_prenom": "APPIA ADOLPHE",
        "tel": "0100000052", "email": "DIRECTION@LES-FAISCEAUX.INVALID", "qualite": "04"}}});
    reussi(&route(&c, "enregistrer-fiche", civilite, "2026-05-20"));
    // Le certificat d'emploi Conges Spectacles est remis « a l'issue de chacun de ses contrats de travail » d'artiste
    // embauche sous CDD de moins de douze mois (fiche Audiens, `majoration-caisses-conges/audiens-fiche-pratique-conges-
    // spectacles-202302.txt`, l. 27-31). L'adresse de FULLER s'ecrit sans accent pour que ses documents partent.
    let adresse = json!({"nom": "salarie", "cle": "FULLER", "confirme": true, "donnees": {"email": "loie.fuller@exemple.invalid"}});
    reussi(&route(&c, "enregistrer-fiche", adresse, "2026-05-20"));
    let fait = reussi(&route(&c, "horloge-rattraper", json!({"aujourdhui": "2026-06-20", "retenir": false}), "2026-06-20")).to_string();
    assert!(!fait.contains("NON SIGNEE"), "l'attestation de FULLER : {}", fait);
    assert!(c.join("sorties").join("2026-05").join("envoi").join("AER_FULLER_Mai2026.pdf").exists(), "l'attestation signee, rangee : {}", fait);
    assert!(c.join("sorties").join("2026-05").join("envoi").join("CertificatCS_FULLER_Mai2026.pdf").exists(), "le certificat Conges Spectacles : {}", fait);

    // SVOBODA demissionne, dernier jour le 02/06/2026 : ses conges non pris lui sont payes (L3141-28), au moins ce qu'il
    // aurait percu en travaillant ces jours (L3141-24, II, `atelier/reference/sources/code-du-travail/xml/L3141-24.xml`) ;
    // un jour de travail est une part du salaire du mois entier, pas de ses deux jours travailles. Un mois a au moins
    // 24 jours ouvrables, et le dixieme d'un salaire constant paie un jour au vingt-cinquieme du salaire : un jour vaut au
    // plus le vingt-quatrieme du salaire.
    let demission = json!({"fin": "2026-06-02", "motif_rupture": "059", "date_paiement": "2026-06-30"});
    let svoboda = json!({"salarie": "SVOBODA", "debut": "2026-01-05", "rang": 0});
    reussi(&crate::outils::modifier(&c, "2026-06", &svoboda, &demission, true, None, "2026-06-20"));
    let ap = route(&c, "apercu", json!({"mois": "2026-06", "modifications": []}), "2026-06-20");
    let svoboda = reussi(&ap)["paies"].as_array().unwrap().iter().find(|p| p.to_string().contains("SVOBODA")).cloned().expect("SVOBODA");
    let iccp = svoboda["remuneration"].as_array().unwrap().iter().find(|l| l["rubrique"] == "indemnite_compensatrice_conges").cloned().expect("l'indemnite compensatrice");
    let taux: f64 = iccp["taux"].to_string().trim_matches('"').parse().unwrap();
    assert!(taux <= 3400.0 / 24.0, "un jour de conge paye plus qu'un jour de travail : {}", iccp);
}
