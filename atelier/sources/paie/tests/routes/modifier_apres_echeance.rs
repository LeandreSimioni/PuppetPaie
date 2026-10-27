//! MODIFIER LA PAIE D'UN MOIS DONT LA DSN EST DEPOSEE. Tant que l'annule-et-remplace est admis
//! (jusqu'a la veille de l'echeance, minuit : cahier technique DSN §1.4.2, `echeance_dsn::Echeance::veille`), la paie se
//! modifie sans rien confirmer, et le moteur dit qu'un annule-et-remplace partira. Apres, la DSN deposee ne se remplace
//! plus : une ecriture qui change la paie se refuse sans confirmation explicite, et rien n'est ecrit ; avec, elle s'ecrit,
//! la saisie telle que declaree est gardee, et la DSN suivante porte la correction. Sur
//! `fictive-3090-petite` (aout 2026, DSN reelle deposee et acceptee).

use crate::outils::{compagnie, dossier_de_test, lire, refus, reussi};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const MOIS: &str = "2026-08";
/// La limite de l'annule-et-remplace d'aout 2026 est passee (la DSN d'octobre embarque la correction).
const APRES: &str = "2026-10-03";

/// Aout de la petite fictive, ses engagements remis en saisie (« Modifier… », la question du moteur acceptee) : sa paie se
/// modifie.
fn compagnie(nom: &str) -> PathBuf {
    let r = compagnie::config_et_mois("fictive-3090-petite", &[MOIS], &dossier_de_test("modifier_apres_echeance", nom).join("compagnie"));
    let etat = route(&r, "etat-par-engagement", json!({"mois": MOIS}), APRES);
    for e in reussi(&etat)["engagements"].as_array().unwrap() {
        let charge = json!({"mois": MOIS, "salarie": e["salarie"], "debut": e["debut"], "numero": e["numero"], "valide": false, "confirme": true});
        reussi(&route(&r, "valider-engagement", charge, APRES));
    }
    r
}

fn route(racine: &Path, nom: &str, charge: Value, jour: &str) -> Value {
    crate::outils::route(racine, nom, charge, jour)
}

fn octets(racine: &Path, rel: &str) -> Option<Vec<u8>> {
    std::fs::read(racine.join(rel)).ok()
}

/// La saisie d'aout, les heures de Lemercier portees a `heures`.
fn saisie_modifiee(racine: &Path, heures: i64) -> Value {
    let mut s = crate::outils::saisie(racine, &MOIS);
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
    let saisie = format!("engagements/{}.json", MOIS);
    let copie = format!("regularisations/{}.saisie-declaree.json", MOIS);

    // Le mois n'est pas ferme : il se modifie, en le confirmant. Le moteur le dit avant toute frappe.
    let s = route(&racine, "saisie", json!({"mois": MOIS}), APRES);
    let d = reussi(&s)["dsn_deposee"].clone();
    assert_eq!((d["confirmer"].as_bool(), &reussi(&s)["hors_delai"]), (Some(true), &Value::Null), "{}", s);
    assert!(d["phrase"].as_str().unwrap().contains("La DSN d'août 2026 est déposée et ne peut plus être remplacée"), "{}", d);

    // Sans confirmation : refuse, avec l'avertissement, et rien n'est ecrit.
    let avant = octets(&racine, &saisie).unwrap();
    let declaree = crate::outils::saisie(&racine, MOIS);
    let r = route(&racine, "enregistrer-saisie", json!({"mois": MOIS, "saisie": saisie_modifiee(&racine, 40)}), APRES);
    let (classe, texte) = refus(&r);
    assert_eq!(classe, "ModificationApresEcheance", "{}", r);
    assert!(texte.starts_with("La limite de la DSN d'août 2026 est passée : l'annule-et-remplace n'est plus admis depuis le "), "{}", texte);
    assert!(texte.ends_with("Êtes-vous sûr de vouloir modifier cette paie ?"), "{}", texte);
    assert_eq!(octets(&racine, &saisie).unwrap(), avant);
    assert!(!racine.join(&copie).exists());

    // Ce qui ne change pas la paie (la meme saisie) n'a rien a confirmer.
    let meme = crate::outils::saisie(&racine, MOIS);
    reussi(&route(&racine, "enregistrer-saisie", json!({"mois": MOIS, "saisie": meme}), APRES));
    assert!(!racine.join(&copie).exists());

    // Avec la confirmation : ecrite, et la saisie telle que declaree est gardee.
    let r = route(&racine, "enregistrer-saisie", json!({"mois": MOIS, "saisie": saisie_modifiee(&racine, 40), "modifier_apres_echeance": true}), APRES);
    assert_eq!(reussi(&r)["dsn_deposee"]["confirmer"], json!(true), "{}", r);
    assert_ne!(octets(&racine, &saisie).unwrap(), avant);
    assert_eq!(lire(&racine, &copie), declaree);
    // Une seconde modification ne la remplace pas : elle reste la paie declaree.
    reussi(&route(&racine, "enregistrer-saisie", json!({"mois": MOIS, "saisie": saisie_modifiee(&racine, 41), "modifier_apres_echeance": true}), APRES));
    assert_eq!(lire(&racine, &copie), declaree);
}

/// UN CONTRAT DECLARE NE SE RETIRE PAS APRES LA LIMITE : la norme DSN n'annule pas un contrat declare (seule une fin
/// de contrat s'annule, S21.G00.62.002 = 099, fiche net-entreprises 1722) ; ses montants s'annulent par un rappel
/// (cahier technique DSN 2026.1, §2.1.3.6). Le retrait se refuse, confirme ou non, avec ce qu'il faut faire ; rien
/// n'est ecrit.
#[test]
fn retirer_un_contrat_declare_apres_la_limite_se_refuse_et_dit_quoi_faire() {
    let racine = compagnie("retrait");
    let saisie = format!("engagements/{}.json", MOIS);
    let avant = octets(&racine, &saisie).unwrap();
    let mut s = crate::outils::saisie(&racine, MOIS);
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

#[test]
fn avant_la_limite_la_modification_ne_demande_rien_et_l_annule_et_remplace_se_dit() {
    let racine = compagnie("avant-la-limite");
    let s = route(&racine, "saisie", json!({"mois": MOIS}), "2026-09-10");
    let d = reussi(&s)["dsn_deposee"].clone();
    assert_eq!(d["confirmer"], json!(false), "{}", s);
    let limite = socle::date::Jour::lire(d["limite"].as_str().unwrap()).unwrap();
    assert!(limite.iso().as_str() >= "2026-09-10", "{}", d);
    assert_eq!(
        d["phrase"],
        json!(format!("La DSN d'août 2026 est déposée : la modifier fera partir un annule-et-remplace, jusqu'au {} à minuit.", limite.jj_mm_aaaa()))
    );
    let r = route(&racine, "enregistrer-saisie", json!({"mois": MOIS, "saisie": saisie_modifiee(&racine, 40)}), "2026-09-10");
    reussi(&r);
    let ecrite = crate::outils::saisie(&racine, &MOIS);
    let rang = ecrite["engagements"].as_array().unwrap().iter().position(|e| e["salarie"] == "Lemercier").unwrap();
    assert_eq!(ecrite["engagements"][rang]["lignes"][0]["quantite"], json!(40));
    // Rien n'est garde : la DSN refaite remplacera la deposee.
    assert!(!racine.join(format!("regularisations/{}.saisie-declaree.json", MOIS)).exists());
}

/// La saisie d'aout, le contrat de Lemercier (LE3.26.04, du 19 au 22 aout, 8 heures par jour) a d'autres dates.
fn lemercier_aux_dates(racine: &Path, debut: &str, fin: &str, jours: &[&str]) -> Value {
    let mut s = crate::outils::saisie(racine, &MOIS);
    let rang = s["engagements"].as_array().unwrap().iter().position(|e| e["salarie"] == "Lemercier").unwrap();
    let e = &mut s["engagements"][rang];
    e["debut"] = json!(debut);
    e["fin"] = json!(fin);
    e["date_paiement"] = json!(fin);
    e["lignes"][0]["dates"] = json!(jours);
    e["lignes"][0]["quantite"] = json!(8 * jours.len());
    s
}

/// LES DATES D'UN CONTRAT DECLARE SE CORRIGENT APRES LA LIMITE. La modification se confirme comme toute autre, sans
/// refus, et l'etape DSN le dit ; l'embauche avancee appelle sa DPAE. La DSN suivante porte la correction (l'ancienne
/// date de debut en S21.G00.41.021, fiche net-entreprises 1952 ; la fin en S21.G00.62, fiche 1722) :
/// `tests/dsn/regularisation_declaree.rs`.
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

    // LA DPAE : l'embauche avancee n'a pas ete precedee de sa declaration (code du travail, L. 1221-10), et la DPAE porte
    // la date d'embauche (R. 1221-1, 3°) : une DPAE au 18 est due, et l'etape DPAE dit pourquoi.
    let attendu = "Embauche avancée du 19/08/2026 au 18/08/2026 : la DPAE déposée pour le 19/08/2026 ne l'a pas précédée (code du travail, art. L. 1221-10)";
    let b = reussi(&route(&racine, "bilan-du-mois", json!({"mois": MOIS}), APRES)).clone();
    assert!(b["dpae"]["phrase"].as_str().unwrap().contains(attendu), "{}", b["dpae"]);
    assert_ne!(b["dpae"]["couleur"], json!("bien"), "{}", b["dpae"]);
    let dues = reussi(&route(&racine, "embauches-a-declarer", json!({"mois": MOIS}), APRES))["embauches"].clone();
    let due = dues.as_array().unwrap().iter().find(|x| x["salarie"] == json!("LEMERCIER")).expect("une DPAE due").clone();
    assert_eq!(due["debut"], json!("2026-08-18"));
    assert!(due["pourquoi"].as_str().unwrap().starts_with(attendu), "{}", due);
}

/// UN CONTRAT QUI DEBORDE SUR LE MOIS PRECEDENT (21/10/2026) : Lemercier a commence le 31 juillet. Rien ne se refuse ;
/// l'apercu de la correction d'aout dit que juillet se corrige aussi, pour lui-meme, et que la DSN suivante porte un
/// versement par mois civil (cahier technique DSN 2026.1, §2.2.2.5).
#[test]
fn un_contrat_qui_deborde_sur_un_autre_mois_se_corrige_mois_par_mois() {
    let racine = compagnie("deborde");
    let s = lemercier_aux_dates(&racine, "2026-07-31", "2026-08-22", &["2026-08-19", "2026-08-20", "2026-08-21", "2026-08-22"]);
    reussi(&route(&racine, "enregistrer-saisie", json!({"mois": MOIS, "saisie": s, "modifier_apres_echeance": true}), APRES));
    let e = lemercier(&racine);
    let charge = json!({"mois": MOIS, "salarie": "LEMERCIER", "debut": "2026-08-19", "engagement": {"lignes": e["engagement_saisi"]["lignes"]}});
    let a = route(&racine, "regularisation-apercu", charge, APRES);
    let phrase = reussi(&a)["dates"].as_str().unwrap().to_string();
    assert!(phrase.contains("Le contrat court aussi sur juillet 2026 : chaque mois se corrige pour lui-même"), "{}", phrase);
}


/// UNE EMBAUCHE REPORTEE (21/10/2026) : Lemercier a commence le 20 aout, pas le 19. La DPAE deposee pour le 19 a precede
/// l'embauche (code du travail, art. L. 1221-10) et portait sa date previsible (art. R. 1221-4,
/// `atelier/reference/sources/code-du-travail/xml/`) : rien a refaire, et l'etape DPAE le dit ; aucune DPAE n'est due.
#[test]
fn une_embauche_reportee_garde_sa_dpae_et_l_etape_le_dit() {
    let racine = compagnie("embauche-reportee");
    let s = lemercier_aux_dates(&racine, "2026-08-20", "2026-08-22", &["2026-08-20", "2026-08-21", "2026-08-22"]);
    reussi(&route(&racine, "enregistrer-saisie", json!({"mois": MOIS, "saisie": s, "modifier_apres_echeance": true}), APRES));
    let b = reussi(&route(&racine, "bilan-du-mois", json!({"mois": MOIS}), APRES)).clone();
    assert_eq!(b["dpae"]["couleur"], json!("bien"), "{}", b["dpae"]);
    assert!(
        b["dpae"]["phrase"].as_str().unwrap().contains("Embauche reportée du 19/08/2026 au 20/08/2026 : la DPAE déposée pour le 19/08/2026 vaut"),
        "{}",
        b["dpae"]
    );
    let dues = reussi(&route(&racine, "embauches-a-declarer", json!({"mois": MOIS}), APRES))["embauches"].clone();
    assert!(!dues.as_array().unwrap().iter().any(|x| x["salarie"] == json!("LEMERCIER")), "{}", dues);
}

/// UN ENGAGEMENT REMIS EN SAISIE GARDE SON IDENTITE QUAND SON PREMIER JOUR CHANGE (#253, banc Le Petit Castelet). Un jour
/// ajoute avant son premier jour change le couple (salarie, debut) : le numero de contrat pose le designe
/// (`jumeaux::apparier_meme_numero`, regle 4). Il reste en saisie, et se lit avec ses chiffres d'apres.
#[test]
fn un_engagement_en_saisie_garde_son_identite_quand_son_premier_jour_change() {
    const PENDANT: &str = "2026-09-10";
    let racine = compagnie("premier-jour-253");
    let brioche = |racine: &Path| -> Value {
        let etat = route(racine, "etat-par-engagement", json!({"mois": MOIS}), PENDANT);
        reussi(&etat)["engagements"].as_array().unwrap().iter().find(|p| p["numero"] == "BR1.26.04").unwrap().clone()
    };
    let avant = brioche(&racine);
    assert_eq!(avant["valide"], Value::Null, "remis en saisie : {}", avant);

    // La page ajoute le 18 : un cachet de plus, et un premier jour plus tot.
    let mut s = crate::outils::saisie(&racine, MOIS);
    let rang = s["engagements"].as_array().unwrap().iter().position(|e| e["numero"] == "BR1.26.04").unwrap();
    let e = s["engagements"][rang].as_object_mut().unwrap();
    // Le contrat se borne a ses jours, comme dans la page : ni debut ni fin poses.
    e.remove("debut");
    e.remove("fin");
    e.insert("lignes".into(), {
        let mut l = e["lignes"].clone();
        l[0]["quantite"] = json!(5);
        l[0]["dates"] = json!(["2026-08-18", "2026-08-19", "2026-08-20", "2026-08-21", "2026-08-22"]);
        l
    });
    reussi(&route(&racine, "enregistrer-saisie", json!({"mois": MOIS, "saisie": s}), PENDANT));

    let engagements = crate::outils::saisie(&racine, MOIS)["engagements"].as_array().unwrap().len();
    assert_eq!(engagements, 3, "le meme engagement, pas un nouveau");
    let apres = brioche(&racine);
    assert_eq!(apres["valide"], Value::Null, "{}", apres);
    assert_eq!(apres["debut"], "2026-08-18", "{}", apres);
    let brut = |p: &Value| p["brut"].as_str().unwrap().parse::<f64>().unwrap();
    assert!(brut(&apres) > brut(&avant), "les chiffres d'apres : {} puis {}", avant["brut"], apres["brut"]);
}
