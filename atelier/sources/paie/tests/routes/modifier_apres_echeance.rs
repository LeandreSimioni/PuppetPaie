//! MODIFIER LA PAIE D'UN MOIS DONT LA DSN EST DEPOSEE (Leandre, 21/10/2026). Tant que l'annule-et-remplace est admis
//! (jusqu'a la veille de l'echeance, minuit : cahier technique DSN §1.4.2, `echeance_dsn::Echeance::veille`), la paie se
//! modifie sans rien confirmer, et le moteur dit qu'un annule-et-remplace partira. Apres, la DSN deposee ne se remplace
//! plus : une ecriture qui change la paie se refuse sans confirmation explicite, et rien n'est ecrit ; avec, elle s'ecrit,
//! la saisie telle que declaree est gardee, et « Corriger ce mois » en part pour la DSN suivante. Sur
//! `fictive-3090-petite` (aout 2026, DSN reelle deposee et acceptee).

use crate::outils::{compagnie, dossier_de_test, lire, refus, reussi};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const MOIS: &str = "2026-08";
/// La limite de l'annule-et-remplace d'aout 2026 est passee (la DSN d'octobre embarque la correction).
const APRES: &str = "2026-10-03";

fn compagnie(nom: &str) -> PathBuf {
    compagnie::config_et_mois("fictive-3090-petite", &[MOIS], &dossier_de_test("modifier_apres_echeance", nom).join("compagnie"))
}

fn route(racine: &Path, nom: &str, charge: Value, jour: &str) -> Value {
    crate::outils::route(racine, nom, charge, jour)
}

fn octets(racine: &Path, rel: &str) -> Option<Vec<u8>> {
    std::fs::read(racine.join(rel)).ok()
}

/// La saisie d'aout, les heures de Lemercier portees a `heures`.
fn saisie_modifiee(racine: &Path, heures: i64) -> Value {
    let mut s = lire(racine, &format!("paies/{}.json", MOIS));
    let rang = s["engagements"].as_array().unwrap().iter().position(|e| e["salarie"] == "Lemercier").unwrap();
    s["engagements"][rang]["lignes"][0]["quantite"] = json!(heures);
    s
}

/// L'engagement de LEMERCIER dans `regularisation-mois`.
fn lemercier(racine: &Path) -> Value {
    let r = route(racine, "regularisation-mois", json!({"mois": MOIS}), APRES);
    reussi(&r)["engagements"].as_array().unwrap().iter().find(|e| e["salarie"] == "LEMERCIER").unwrap().clone()
}

#[test]
fn apres_la_limite_une_modification_se_refuse_sans_confirmation_et_rien_n_est_ecrit() {
    let racine = compagnie("apres-la-limite");
    let saisie = format!("paies/{}.json", MOIS);
    let copie = format!("regularisations/{}.saisie-declaree.json", MOIS);

    // Le mois n'est pas ferme : il se modifie, en le confirmant. Le moteur le dit avant toute frappe.
    let s = route(&racine, "saisie", json!({"mois": MOIS}), APRES);
    let d = reussi(&s)["dsn_deposee"].clone();
    assert_eq!((d["confirmer"].as_bool(), &reussi(&s)["hors_delai"]), (Some(true), &Value::Null), "{}", s);
    assert!(d["phrase"].as_str().unwrap().contains("La DSN d'août 2026 est déposée et ne peut plus être remplacée"), "{}", d);

    // Sans confirmation : refuse, avec l'avertissement, et rien n'est ecrit.
    let avant = octets(&racine, &saisie).unwrap();
    let r = route(&racine, "enregistrer-saisie", json!({"mois": MOIS, "saisie": saisie_modifiee(&racine, 40)}), APRES);
    let (classe, texte) = refus(&r);
    assert_eq!(classe, "ModificationApresEcheance", "{}", r);
    assert!(texte.starts_with("La limite de la DSN d'août 2026 est passée : l'annule-et-remplace n'est plus admis depuis le "), "{}", texte);
    assert!(texte.contains("La DSN déposée ne changera pas : la correction se déclarera dans la DSN d'octobre 2026, une fois écrite par « Corriger ce mois »"), "{}", texte);
    assert!(texte.ends_with("Êtes-vous sûr de vouloir modifier cette paie ?"), "{}", texte);
    assert_eq!(octets(&racine, &saisie).unwrap(), avant);
    assert!(!racine.join(&copie).exists());

    // Ce qui ne change pas la paie (la meme saisie) n'a rien a confirmer.
    let meme = lire(&racine, &saisie);
    reussi(&route(&racine, "enregistrer-saisie", json!({"mois": MOIS, "saisie": meme}), APRES));
    assert!(!racine.join(&copie).exists());

    // Avec la confirmation : ecrite, et la saisie telle que declaree est gardee.
    let r = route(&racine, "enregistrer-saisie", json!({"mois": MOIS, "saisie": saisie_modifiee(&racine, 40), "modifier_apres_echeance": true}), APRES);
    assert_eq!(reussi(&r)["dsn_deposee"]["confirmer"], json!(true), "{}", r);
    assert_ne!(octets(&racine, &saisie).unwrap(), avant);
    assert_eq!(octets(&racine, &copie).unwrap(), avant);
    // Une seconde modification ne la remplace pas : elle reste la paie declaree.
    reussi(&route(&racine, "enregistrer-saisie", json!({"mois": MOIS, "saisie": saisie_modifiee(&racine, 41), "modifier_apres_echeance": true}), APRES));
    assert_eq!(octets(&racine, &copie).unwrap(), avant);
    reussi(&route(&racine, "enregistrer-saisie", json!({"mois": MOIS, "saisie": saisie_modifiee(&racine, 40), "modifier_apres_echeance": true}), APRES));

    // L'etape DSN le dit, en orange, tant que la correction n'est pas ecrite.
    let b = route(&racine, "bilan-du-mois", json!({"mois": MOIS}), APRES);
    let dsn = &reussi(&b)["dsn"];
    assert_eq!(dsn["couleur"], json!("alerte"), "{}", dsn);
    assert!(dsn["phrase"].as_str().unwrap().contains("1 contrat à corriger dans la DSN suivante (Corriger ce mois)"), "{}", dsn);
    assert_eq!(dsn["remplacement"]["correction_ecrite"], json!(false));

    // « Corriger ce mois » part de ce qui est declare, et propose ce qui est saisi.
    let e = lemercier(&racine);
    assert_eq!(e["engagement"]["lignes"][0]["quantite"], json!(32), "{}", e);
    assert_eq!(e["engagement_saisi"]["lignes"][0]["quantite"], json!(40), "{}", e);
    let r = route(
        &racine,
        "regularisation-ecrire",
        json!({"mois": MOIS, "salarie": "LEMERCIER", "debut": e["debut"], "engagement": {"lignes": e["engagement_saisi"]["lignes"]}, "motif": "heures de Lemercier"}),
        APRES,
    );
    assert_eq!(reussi(&r)["etat"], json!("en attente"), "{}", r);

    // La correction ecrite, l'etape n'a plus rien a faire : la DSN suivante l'embarque.
    let b = route(&racine, "bilan-du-mois", json!({"mois": MOIS}), APRES);
    let dsn = &reussi(&b)["dsn"];
    assert_eq!((dsn["couleur"].as_str(), dsn["remplacement"]["correction_ecrite"].as_bool()), (Some("bien"), Some(true)), "{}", dsn);
    assert!(dsn["phrase"].as_str().unwrap().contains("la correction est écrite (Corriger ce mois), elle part dans la DSN suivante"), "{}", dsn);
}

/// UN CONTRAT DECLARE NE SE RETIRE PAS APRES LA LIMITE : la norme DSN n'annule pas un contrat declare (seule une fin
/// de contrat s'annule, S21.G00.62.002 = 099, fiche net-entreprises 1722) ; ses montants s'annulent par un rappel
/// (cahier technique DSN 2026.1, §2.1.3.6). Le retrait se refuse, confirme ou non, avec ce qu'il faut faire ; rien
/// n'est ecrit.
#[test]
fn retirer_un_contrat_declare_apres_la_limite_se_refuse_et_dit_quoi_faire() {
    let racine = compagnie("retrait");
    let saisie = format!("paies/{}.json", MOIS);
    let avant = octets(&racine, &saisie).unwrap();
    let mut s = lire(&racine, &saisie);
    s["engagements"].as_array_mut().unwrap().pop();
    for confirme in [false, true] {
        let r = route(&racine, "enregistrer-saisie", json!({"mois": MOIS, "saisie": s, "modifier_apres_echeance": confirme}), APRES);
        let (classe, texte) = refus(&r);
        assert_eq!(classe, "ModificationNonDeclarable", "{}", r);
        assert!(texte.contains("ne se retire plus"), "{}", texte);
        assert!(texte.contains("motif 099, fiche net-entreprises 1722"), "{}", texte);
        assert!(texte.contains("cahier technique DSN §2.1.3.6"), "{}", texte);
        assert_eq!(octets(&racine, &saisie).unwrap(), avant);
    }
    assert!(!racine.join(format!("regularisations/{}.saisie-declaree.json", MOIS)).exists());
}

/// LA CORRECTION ECRITE DOIT DECLARER LA PAIE D'AUJOURD'HUI : la saisie change encore apres elle, l'etape DSN repasse a
/// l'orange ; la correction reecrite remplace celle en attente (rien n'etait parti), et l'etape revient au vert.
#[test]
fn une_saisie_qui_change_apres_la_correction_ecrite_est_a_corriger_a_nouveau() {
    let racine = compagnie("encore");
    let ecrire_saisie = |h: i64| reussi(&route(&racine, "enregistrer-saisie", json!({"mois": MOIS, "saisie": saisie_modifiee(&racine, h), "modifier_apres_echeance": true}), APRES)).clone();
    let corriger = || {
        let e = lemercier(&racine);
        let r = route(&racine, "regularisation-ecrire",
            json!({"mois": MOIS, "salarie": "LEMERCIER", "debut": e["debut"], "engagement": {"lignes": e["engagement_saisi"]["lignes"]}, "motif": "heures de Lemercier"}), APRES);
        reussi(&r)["correction"].clone()
    };
    let dsn = || reussi(&route(&racine, "bilan-du-mois", json!({"mois": MOIS}), APRES))["dsn"].clone();
    ecrire_saisie(40);
    assert_eq!(corriger()["numero"], json!(1));
    assert_eq!(dsn()["remplacement"]["correction_ecrite"], json!(true));

    // La paie change encore : la correction n° 1 ne la declare plus.
    ecrire_saisie(44);
    let d = dsn();
    assert_eq!((d["couleur"].as_str(), d["remplacement"]["correction_ecrite"].as_bool()), (Some("alerte"), Some(false)), "{}", d);
    assert!(d["phrase"].as_str().unwrap().contains("la paie a encore changé depuis la correction écrite — à corriger à nouveau"), "{}", d);

    // Reecrite, elle remplace celle en attente.
    assert_eq!(corriger()["numero"], json!(2));
    let f = lire(&racine, &format!("regularisations/{}.json", MOIS));
    assert!(f["corrections"][0]["annulee_le"].is_string(), "{}", f);
    assert_eq!(f["corrections"][0]["remplacee_par"], json!(2));
    assert_eq!(f["corrections"][1]["engagement"]["lignes"][0]["quantite"], json!(44));
    assert_eq!(dsn()["remplacement"]["correction_ecrite"], json!(true));
}

/// UN CONTRAT OUBLIE s'ajoute apres la limite en le confirmant ; « Corriger ce mois » le liste comme ajoute, sans rien de
/// declare (l'avant est nul), et sa correction lui donne son numero (S21.G00.40.009, regle 4 : il ne se recalcule plus).
/// La DSN qui le porte : `tests/dsn/regularisation_declaree.rs`.
#[test]
fn un_contrat_ajoute_apres_la_limite_se_declare_par_sa_correction() {
    let racine = compagnie("ajout");
    let mut s = lire(&racine, &format!("paies/{}.json", MOIS));
    let mut e = s["engagements"][2].clone();
    let o = e.as_object_mut().unwrap();
    o.remove("numero");
    o.insert("debut".into(), json!("2026-08-26"));
    o.insert("fin".into(), json!("2026-08-27"));
    o.insert("date_paiement".into(), json!("2026-08-27"));
    o.insert("lignes".into(), json!([{"rubrique": "heure", "quantite": 16, "taux": 18.0, "lieu": "Ailleurs", "dates": ["2026-08-26", "2026-08-27"]}]));
    s["engagements"].as_array_mut().unwrap().push(e);
    assert_eq!(refus(&route(&racine, "enregistrer-saisie", json!({"mois": MOIS, "saisie": s}), APRES)).0, "ModificationApresEcheance");
    reussi(&route(&racine, "enregistrer-saisie", json!({"mois": MOIS, "saisie": s, "modifier_apres_echeance": true}), APRES));
    let dsn = || reussi(&route(&racine, "bilan-du-mois", json!({"mois": MOIS}), APRES))["dsn"].clone();
    assert!(dsn()["phrase"].as_str().unwrap().contains("1 contrat à corriger dans la DSN suivante"), "{}", dsn());

    let m = route(&racine, "regularisation-mois", json!({"mois": MOIS}), APRES);
    let a = reussi(&m)["engagements"].as_array().unwrap().iter().find(|e| e["ajoute"] == json!(true)).unwrap().clone();
    assert_eq!((a["salarie"].as_str(), a["debut"].as_str(), &a["numero"]), (Some("LEMERCIER"), Some("2026-08-26"), &Value::Null), "{}", a);
    let charge = json!({"mois": MOIS, "salarie": "LEMERCIER", "debut": "2026-08-26", "engagement": {"lignes": a["engagement_saisi"]["lignes"]}});
    let ap = route(&racine, "regularisation-apercu", charge.clone(), APRES);
    let ap = reussi(&ap);
    assert_eq!((ap["ajout"].as_bool(), ap["avant"]["brut"].as_str(), ap["apres"]["brut"].as_str()), (Some(true), Some("0.00"), Some("288.00")), "{}", ap);
    let compteurs = lire(&racine, "config/compteurs.json");
    let mut charge = charge;
    charge["motif"] = json!("contrat oublie");
    let c = reussi(&route(&racine, "regularisation-ecrire", charge, APRES))["correction"].clone();
    assert_eq!(c["ajout"], json!(true));
    let numero = c["engagement"]["numero"].as_str().unwrap().to_string();
    assert!(numero.starts_with("LE3.26."), "{}", numero);
    assert_ne!(lire(&racine, "config/compteurs.json"), compteurs, "le numero se reserve");
    assert!(lire(&racine, &format!("paies/{}.json", MOIS))["engagements"].as_array().unwrap().iter().any(|e| e["numero"] == json!(numero)));
    let d = dsn();
    assert_eq!((d["couleur"].as_str(), d["remplacement"]["correction_ecrite"].as_bool()), (Some("bien"), Some(true)), "{}", d);
}

#[test]
fn avant_la_limite_la_modification_ne_demande_rien_et_l_annule_et_remplace_se_dit() {
    let racine = compagnie("avant-la-limite");
    let s = route(&racine, "saisie", json!({"mois": MOIS}), "2026-09-10");
    let d = reussi(&s)["dsn_deposee"].clone();
    assert_eq!(d["confirmer"], json!(false), "{}", s);
    let limite = paie::date::Jour::lire(d["limite"].as_str().unwrap()).unwrap();
    assert!(limite.iso().as_str() >= "2026-09-10", "{}", d);
    assert_eq!(
        d["phrase"],
        json!(format!("La DSN d'août 2026 est déposée : la modifier fera partir un annule-et-remplace, jusqu'au {} à minuit.", limite.jj_mm_aaaa()))
    );
    let r = route(&racine, "enregistrer-saisie", json!({"mois": MOIS, "saisie": saisie_modifiee(&racine, 40)}), "2026-09-10");
    reussi(&r);
    let ecrite = lire(&racine, &format!("paies/{}.json", MOIS));
    let rang = ecrite["engagements"].as_array().unwrap().iter().position(|e| e["salarie"] == "Lemercier").unwrap();
    assert_eq!(ecrite["engagements"][rang]["lignes"][0]["quantite"], json!(40));
    // Rien n'est garde : la DSN refaite remplacera la deposee.
    assert!(!racine.join(format!("regularisations/{}.saisie-declaree.json", MOIS)).exists());
}

/// La saisie d'aout, le contrat de Lemercier (LE3.26.04, du 19 au 22 aout, 8 heures par jour) a d'autres dates.
fn lemercier_aux_dates(racine: &Path, debut: &str, fin: &str, jours: &[&str]) -> Value {
    let mut s = lire(racine, &format!("paies/{}.json", MOIS));
    let rang = s["engagements"].as_array().unwrap().iter().position(|e| e["salarie"] == "Lemercier").unwrap();
    let e = &mut s["engagements"][rang];
    e["debut"] = json!(debut);
    e["fin"] = json!(fin);
    e["date_paiement"] = json!(fin);
    e["lignes"][0]["dates"] = json!(jours);
    e["lignes"][0]["quantite"] = json!(8 * jours.len());
    s
}

/// LES DATES D'UN CONTRAT DECLARE SE CORRIGENT APRES LA LIMITE (21/10/2026). Le contrat garde son numero : la saisie le
/// retrouve a ses nouvelles dates, et la modification se confirme comme toute autre, sans refus. « Corriger ce mois » le
/// designe a son premier jour declare et en reprend les dates ; l'apercu dit ce que la DSN suivante en declarera
/// (l'ancienne date de debut en S21.G00.41.021, fiche net-entreprises 1952 ; la fin en S21.G00.62, fiche 1722) ; la
/// correction ecrite, l'etape DSN passe au vert. Que la saisie change encore de dates la repasse a l'orange. La DSN qui
/// la porte : `tests/dsn/regularisation_declaree.rs`.
#[test]
fn les_dates_d_un_contrat_declare_se_corrigent_apres_la_limite() {
    let racine = compagnie("dates");
    let jours = ["2026-08-18", "2026-08-19", "2026-08-20", "2026-08-21", "2026-08-22"];
    let s = lemercier_aux_dates(&racine, "2026-08-18", "2026-08-22", &jours);
    assert_eq!(refus(&route(&racine, "enregistrer-saisie", json!({"mois": MOIS, "saisie": s}), APRES)).0, "ModificationApresEcheance");
    reussi(&route(&racine, "enregistrer-saisie", json!({"mois": MOIS, "saisie": s, "modifier_apres_echeance": true}), APRES));
    let dsn = || reussi(&route(&racine, "bilan-du-mois", json!({"mois": MOIS}), APRES))["dsn"].clone();
    let d = dsn();
    assert_eq!(d["couleur"], json!("alerte"), "{}", d);
    assert!(d["phrase"].as_str().unwrap().contains("1 contrat à corriger dans la DSN suivante"), "{}", d);

    let e = lemercier(&racine);
    assert_eq!((e["debut"].as_str(), e["numero"].as_str(), e["ajoute"].as_bool()), (Some("2026-08-19"), Some("LE3.26.04"), Some(false)), "{}", e);
    assert_eq!(e["dates_saisies"], json!("du 18/08/2026 au 22/08/2026"), "{}", e);
    assert_eq!(e["engagement_saisi"]["debut"], json!("2026-08-18"));
    let charge = json!({"mois": MOIS, "salarie": "LEMERCIER", "debut": "2026-08-19", "engagement": {"lignes": e["engagement_saisi"]["lignes"]}});
    let a = route(&racine, "regularisation-apercu", charge.clone(), APRES);
    let a = reussi(&a);
    assert_eq!((a["engagement"]["debut"].as_str(), a["engagement"]["fin"].as_str()), (Some("2026-08-18"), Some("2026-08-22")), "{}", a);
    assert_eq!((a["avant"]["heures"].as_str(), a["apres"]["heures"].as_str(), a["rien_ne_change"].as_bool()), (Some("32.0"), Some("40.0"), Some(false)), "{}", a);
    assert_eq!(
        a["dates"],
        json!("Dates corrigées : du 18/08/2026 au 22/08/2026 (déclaré : du 19/08/2026 au 22/08/2026). La DSN suivante déclare la nouvelle date de début avec l'ancienne, dans un bloc « Changements contrat » (S21.G00.41.021, fiche net-entreprises 1952).")
    );
    let mut charge = charge;
    charge["motif"] = json!("Lemercier a commence le 18");
    let c = reussi(&route(&racine, "regularisation-ecrire", charge, APRES))["correction"].clone();
    assert_eq!((c["debut"].as_str(), c["engagement"]["debut"].as_str(), c["engagement"]["numero"].as_str()), (Some("2026-08-19"), Some("2026-08-18"), Some("LE3.26.04")), "{}", c);
    let d = dsn();
    assert_eq!((d["couleur"].as_str(), d["remplacement"]["correction_ecrite"].as_bool()), (Some("bien"), Some(true)), "{}", d);

    // La fin change encore dans la saisie, sans rien changer d'autre a la paie : la correction ecrite ne la declare plus.
    let mut s = lemercier_aux_dates(&racine, "2026-08-18", "2026-08-23", &jours);
    s["engagements"].as_array_mut().unwrap().iter_mut().find(|e| e["salarie"] == "Lemercier").unwrap()["date_paiement"] = json!("2026-08-22");
    reussi(&route(&racine, "enregistrer-saisie", json!({"mois": MOIS, "saisie": s, "modifier_apres_echeance": true}), APRES));
    let d = dsn();
    assert_eq!((d["couleur"].as_str(), d["remplacement"]["correction_ecrite"].as_bool()), (Some("alerte"), Some(false)), "{}", d);
    assert!(d["phrase"].as_str().unwrap().contains("à corriger à nouveau"), "{}", d);
}

/// Des dates qui sortent du mois declare ne se corrigent pas ici : le contrat serait aussi a declarer dans la DSN d'un
/// autre mois. Refus, confirme ou non, et rien n'est ecrit.
#[test]
fn des_dates_qui_sortent_du_mois_declare_se_refusent() {
    let racine = compagnie("dates-hors-du-mois");
    let saisie = format!("paies/{}.json", MOIS);
    let avant = octets(&racine, &saisie).unwrap();
    // Le contrat commencerait le 31 juillet : la DSN de juillet aurait a le declarer.
    let s = lemercier_aux_dates(&racine, "2026-07-31", "2026-08-22", &["2026-08-19", "2026-08-20", "2026-08-21", "2026-08-22"]);
    for confirme in [false, true] {
        let r = route(&racine, "enregistrer-saisie", json!({"mois": MOIS, "saisie": s, "modifier_apres_echeance": confirme}), APRES);
        let (classe, texte) = refus(&r);
        assert_eq!(classe, "ModificationNonDeclarable", "{}", r);
        assert!(texte.contains("ses nouvelles dates sortent du mois d'août 2026"), "{}", texte);
        assert!(texte.contains("S21.G00.41.021, fiche net-entreprises 1952"), "{}", texte);
        assert_eq!(octets(&racine, &saisie).unwrap(), avant);
    }
}
