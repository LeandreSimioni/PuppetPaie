//! La nationalité d'un salarié : le pays se choisit dans une liste, la codification UE (S21.G00.30.013) s'en déduit, un
//! pays hors de la liste laisse l'administrateur choisir la zone, le pays de naissance (30.015) se lit dans la table « Pays »
//! de la DSN ; à la fiche, au questionnaire, dans la DSN et au registre du personnel.
//!
//! Les textes : cahier technique DSN 2026.1, `prive/reference/norme-dsn/texte/bloc-S21.G00.30.txt` (30.013, p. 173,
//! l. 267-277 : « 01 - France / 02 - UE / 03 - EEE / 04 - Reste du Monde », « le code le plus précis applicable » ; 30.015,
//! p. 174, l. 297-303, CRE-11) ; les Etats : `prive/reference/sources/union-europeenne/LISEZ-MOI.md` ; la liste :
//! `logiciel/donnees/pays-base.json` (COG de l'INSEE 2026, `prive/reference/sources/insee-cog-pays/`) ; la table « Pays » :
//! `logiciel/donnees/pays-dsn-base.json` (`prive/reference/sources/dsn-nomenclatures/tables-PAYS-et-PAY-20260924.txt`) ;
//! le pays hors liste : décision de Léandre du 10/10/2026 ; le registre : C. trav., D1221-23
//! (`prive/reference/sources/code-du-travail/xml/D1221-23.xml`).

use crate::outils::{self, compagnie};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const JOUR: &str = "2026-10-03";

/// La DSN de juillet 2026 de la 3090-grande, les fiches retouchées.
fn dsn_de_juillet(test: &str, retouches: &[(&str, Value)]) -> Result<String, String> {
    let r = compagnie::ses_dsn("fictive-3090-grande", &[], &outils::dossier_de_test("nationalite", test));
    outils::retoucher_fiches(&r, retouches);
    outils::dsn_du_mois(&r, "2026-07", "2026-07-28")
}

/// 30.013 de chaque individu, par nom (30.002).
fn codifications(dsn: &str) -> HashMap<String, String> {
    let (mut nom, mut out) = (String::new(), HashMap::new());
    for (k, v) in outils::paires(dsn) {
        match k.as_str() {
            "S21.G00.30.002" => nom = v,
            "S21.G00.30.013" => {
                out.insert(nom.clone(), v);
            }
            _ => {}
        }
    }
    out
}

/// (30.013, 30.015) de l'individu `nom`.
fn zone_et_naissance(dsn: &str, nom: &str) -> (Option<String>, Option<String>) {
    let i = outils::individu(dsn, nom);
    (outils::valeur(&i, "S21.G00.30.013"), outils::valeur(&i, "S21.G00.30.015"))
}

fn de(a: &str, b: &str) -> (Option<String>, Option<String>) {
    (Some(a.into()), Some(b.into()))
}

/// La 3090-petite (configuration, août et septembre 2026), pour les routes.
fn petite(test: &str) -> PathBuf {
    compagnie::config_et_mois("fictive-3090-petite", &["2026-08", "2026-09"], &outils::dossier_de_test("nationalite", test))
}

fn route(racine: &Path, nom: &str, charge: Value) -> Value {
    outils::route(racine, nom, charge, JOUR)
}

fn premiere_cle(racine: &Path) -> (String, Value) {
    let t = outils::lire(racine, "config/salaries.json");
    let (k, v) = t.as_object().unwrap().iter().find(|(k, _)| !k.starts_with('_')).unwrap();
    (k.clone(), v.clone())
}

/// Les erreurs de `verifier-fiche` : (champ, message).
fn erreurs(racine: &Path, fiche: &Value) -> Vec<(String, String)> {
    let v = route(racine, "verifier-fiche", json!({"nom": "salarie", "donnees": fiche}));
    outils::reussi(&v)["anomalies"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["gravite"] == "erreur")
        .map(|a| (a["champ"].as_str().unwrap_or("").to_string(), a["message"].as_str().unwrap_or("").to_string()))
        .collect()
}

fn champs_livres() -> Value {
    outils::lire(&outils::code_livre(), "champs-base.json")
}

// ------------------------------------------------------------------ la codification UE dans la DSN

#[test]
fn la_dsn_code_chaque_nationalite_par_son_pays() {
    let dsn = dsn_de_juillet(
        "trois-pays",
        &[
            ("BIANCOLELLI", json!({"code_nationalite_dsn": "03", "pays_nationalite": "NO"})),
            // Une fiche ancienne : 03 « un autre pays », et le pays connu : le moteur la recode depuis la table.
            ("SERVANDONI", json!({"code_nationalite_dsn": "03", "pays_nationalite": "MA"})),
            ("CLAIRON", json!({"code_nationalite_dsn": "02", "pays_nationalite": "IT"})),
            // L'ancien questionnaire codait 02 tout pays d'Europe.
            ("CHAMINADE", json!({"code_nationalite_dsn": "02", "pays_nationalite": "NO"})),
            // 04, valeur neuve, part sans pays.
            ("JACQUET", json!({"code_nationalite_dsn": "04", "pays_nationalite": null})),
        ],
    )
    .unwrap();
    let c = codifications(&dsn);
    assert_eq!(c["BIANCOLELLI"], "03", "la Norvege est dans l'EEE, hors de l'Union");
    assert_eq!(c["SERVANDONI"], "04", "le Maroc : reste du monde (l'ancien 03 relu par le pays)");
    assert_eq!(c["CLAIRON"], "02");
    assert_eq!(c["CHAMINADE"], "03");
    assert_eq!(c["JACQUET"], "04");
    let retouches = ["BIANCOLELLI", "SERVANDONI", "CLAIRON", "CHAMINADE", "JACQUET"];
    assert!(c.iter().filter(|(n, _)| !retouches.contains(&n.as_str())).all(|(_, v)| v == "01"), "{:?}", c);
}

#[test]
fn un_ancien_03_sans_pays_de_nationalite_ne_part_pas() {
    let e = dsn_de_juillet("ancien-03", &[("SERVANDONI", json!({"code_nationalite_dsn": "03", "pays_nationalite": null}))]).unwrap_err();
    assert!(e.contains("SERVANDONI") || e.contains("OCTAVE"), "la phrase nomme le salarie : {}", e);
    assert!(e.contains("pays de nationalité") && e.contains("04"), "la phrase dit quoi faire : {}", e);
}

#[test]
fn la_fiche_dit_l_ancien_03_avant_la_dsn() {
    let erreurs = |v: Value| -> Vec<Value> {
        paie::champs::verifier_fiche("salarie", v.as_object().unwrap())
            .unwrap()
            .into_iter()
            .filter(|a| a["champ"] == "code_nationalite_dsn" && a["gravite"] == "erreur")
            .collect()
    };
    let avec = |k: &[(&str, &str)]| {
        let mut b = json!({"nom": "ANDERSEN", "prenom": "INGRID", "sexe": "2"});
        for (c, v) in k {
            b[*c] = json!(v);
        }
        b
    };
    assert_eq!(erreurs(avec(&[("code_nationalite_dsn", "03")])).len(), 1, "un 03 sans pays se dit dans la fiche");
    assert!(erreurs(avec(&[("code_nationalite_dsn", "03"), ("pays_nationalite", "NO")])).is_empty());
    assert!(erreurs(avec(&[("code_nationalite_dsn", "04")])).is_empty(), "04 est un choix de la fiche");
}

// ------------------------------------------------------------------ la liste des pays

#[test]
fn la_liste_des_pays_se_sert_a_la_page_avec_la_nationalite_deduite() {
    let racine = petite("etat");
    let r = route(&racine, "etat", json!({}));
    let r = outils::reussi(&r);
    let pays = r["pays"].as_array().expect("l'etat sert la liste des pays").clone();
    assert!(pays.len() > 200, "{}", pays.len());
    let de = |code: &str| pays.iter().find(|p| p["code"] == code).unwrap_or_else(|| panic!("{} absent", code)).clone();
    assert_eq!(de("FR"), json!({"code": "FR", "nom": "France", "codification_ue": "01"}));
    assert_eq!(de("IT")["codification_ue"], "02");
    assert_eq!(de("NO")["codification_ue"], "03");
    assert_eq!(de("MA")["codification_ue"], "04");
    assert_eq!(de("GB")["codification_ue"], "04");
    // Un territoire suit son pays de rattachement (COG, CRPAY) : le Groenland, le Danemark ; Gibraltar, le Royaume-Uni.
    // Les Feroe font exception : « les traites ne s'appliquent pas aux iles Feroe » (TFUE art. 355 § 5 a,
    // `union-europeenne/LISEZ-MOI.md`, « Les iles Feroe »), donc 04.
    assert_eq!(de("GL")["codification_ue"], "02");
    assert_eq!(de("FO")["codification_ue"], "04");
    assert_eq!(de("GI")["codification_ue"], "04");
    // Triee par nom, accents ignores : l'Egypte avant l'Equateur et l'Espagne, pas apres le Zimbabwe.
    let noms: Vec<&str> = pays.iter().map(|p| p["nom"].as_str().unwrap()).collect();
    let rang = |n: &str| noms.iter().position(|x| *x == n).unwrap();
    assert!(rang("Afghanistan") < rang("Égypte") && rang("Égypte") < rang("Équateur") && rang("Équateur") < rang("Espagne"), "{:?}", noms);
    // Le pays de naissance se choisit dans la table « Pays » de la DSN (30.015) : 274 codes, XK Kosovo et SU URSS compris ;
    // le nom du COG quand il en a un, le libelle de la table sinon.
    let naissance = r["pays_naissance"].as_array().expect("l'etat sert la liste des pays de naissance").clone();
    assert_eq!(naissance.len(), 274);
    let de = |code: &str| naissance.iter().find(|p| p["code"] == code).unwrap_or_else(|| panic!("{} absent", code)).clone();
    assert_eq!(de("FR"), json!({"code": "FR", "nom": "France"}));
    assert_eq!(de("XK"), json!({"code": "XK", "nom": "KOSOVO"}));
    assert_eq!(de("SU"), json!({"code": "SU", "nom": "URSS"}));
    let champs = champs_livres();
    for cle in ["pays_naissance", "pays_nationalite"] {
        let c = champs["salarie"].as_array().unwrap().iter().find(|c| c["cle"] == cle).unwrap();
        assert_eq!(c["forme"], "pays", "{}", cle);
    }
}

#[test]
fn un_pays_choisi_ecrit_la_nationalite_dsn_a_l_enregistrement() {
    let racine = petite("enregistrer");
    let (cle, mut fiche) = premiere_cle(&racine);
    for (pays, attendu) in [("NO", "03"), ("MA", "04"), ("IT", "02"), ("FO", "04"), ("FR", "01")] {
        // La case dit encore 01 (ou ce que l'essai precedent a laisse) : le pays tranche.
        fiche["pays_nationalite"] = json!(pays);
        fiche["code_nationalite_dsn"] = json!("01");
        assert!(erreurs(&racine, &fiche).is_empty(), "{} : {:?}", pays, erreurs(&racine, &fiche));
        let r = route(&racine, "enregistrer-fiche", json!({"nom": "salarie", "cle": cle, "donnees": fiche, "depuis": JOUR}));
        outils::reussi(&r);
        let lu = outils::lire(&racine, "config/salaries.json");
        assert_eq!(lu[&cle]["code_nationalite_dsn"], json!(attendu), "{}", pays);
        assert_eq!(lu[&cle]["pays_nationalite"], json!(pays));
        fiche = lu[&cle].clone();
    }
}

#[test]
fn un_code_inconnu_de_la_liste_refuse_avec_une_phrase() {
    let racine = petite("inconnu");
    let (cle, mut fiche) = premiere_cle(&racine);
    fiche["pays_naissance"] = json!("ZZ");
    fiche["pays_nationalite"] = json!("UK");
    let e = erreurs(&racine, &fiche);
    let sur = |champ: &str| -> Vec<&String> { e.iter().filter(|(c, _)| c == champ).map(|(_, m)| m).collect() };
    let n = sur("pays_naissance");
    assert_eq!(n.len(), 1, "{:?}", e);
    assert!(n[0].contains("« ZZ »") && n[0].contains("S21.G00.30.015") && n[0].contains("table « Pays »"), "{}", n[0]);
    let n = sur("pays_nationalite");
    assert_eq!(n.len(), 1, "{:?}", e);
    assert!(n[0].contains("« UK »"), "{}", n[0]);
    assert!(sur("code_nationalite_dsn").is_empty(), "le pays inconnu se dit une fois, sur sa case : {:?}", e);
    let r = route(&racine, "enregistrer-fiche", json!({"nom": "salarie", "cle": cle, "donnees": fiche, "depuis": JOUR}));
    outils::refus(&r);
    // Un code deja saisi en minuscules se relit tel quel.
    let f: Map<String, Value> = json!({"nom": "X", "prenom": "Y", "pays_naissance": "it", "pays_nationalite": "it", "code_nationalite_dsn": "02"})
        .as_object()
        .unwrap()
        .clone();
    let a = paie::champs::verifier_fiche("salarie", &f).unwrap();
    assert!(a.iter().all(|x| !(x["champ"].as_str().unwrap_or("").starts_with("pays") && x["gravite"] == "erreur")), "{:?}", a);
}

#[test]
fn la_dsn_refuse_une_nationalite_hors_de_la_liste_et_declare_celle_d_un_territoire() {
    let e = dsn_de_juillet("nationalite-uk", &[("CLAIRON", json!({"pays_nationalite": "UK", "code_nationalite_dsn": "04"}))]).unwrap_err();
    assert!(e.contains("pays de nationalité") && e.contains("« UK »"), "{}", e);
    // Les Feroe : 30.015 dit FO (la table « Pays »), 30.013 dit 04 (hors de l'Union, TFUE art. 355 § 5 a).
    let dsn = dsn_de_juillet("feroe", &[("CLAIRON", json!({"pays_naissance": "FO", "pays_nationalite": "FO", "code_nationalite_dsn": "04"}))]).unwrap();
    assert_eq!(zone_et_naissance(&dsn, "CLAIRON"), de("04", "FO"));
}

#[test]
fn le_questionnaire_lit_le_pays_dans_la_liste_et_en_deduit_la_codification() {
    use paie::fiche_questionnaire::{fiche_depuis_reponses, pays_lu};
    let lire = |cle: &'static str, reponse: &str| fiche_depuis_reponses(&[(cle, reponse.to_string())].into_iter().collect());
    for (reponse, pays, code) in [("norvégienne", "NO", "03"), ("Marocain", "MA", "04"), ("italienne", "IT", "02"), ("liechtensteinoise", "LI", "03"), ("britannique", "GB", "04"), ("Suisse", "CH", "04"), ("Islande", "IS", "03"), ("française", "FR", "01")] {
        let (d, illisible) = lire("nationalite", reponse);
        assert!(illisible.is_empty(), "{} : {:?}", reponse, illisible);
        assert_eq!((&d["pays_nationalite"], &d["code_nationalite_dsn"]), (&json!(pays), &json!(code)), "{}", reponse);
    }
    // Des noms que l'ancienne table courte ne connaissait pas.
    assert_eq!(pays_lu("Sénégal").map(|p| p.0), Some("SN".into()));
    assert_eq!(pays_lu("Géorgie").map(|p| p.0), Some("GE".into()));
    assert_eq!(pays_lu("Islande"), Some(("IS".into(), Some("03"))));
    assert_eq!(pays_lu("Côte d’Ivoire").map(|p| p.0), Some("CI".into()));
    assert_eq!(pays_lu("République tchèque").map(|p| p.0), Some("CZ".into()));
    // Un code que la liste ne connait pas ne se garde plus.
    assert_eq!(pays_lu("zz"), None);
    let rep: HashMap<&'static str, String> = [("pays", "Atlantide".to_string()), ("nationalite", "Vietnam".to_string())].into_iter().collect();
    let (d, illisible) = fiche_depuis_reponses(&rep);
    assert!(d.get("pays_naissance").is_none(), "{:?}", d);
    assert_eq!(illisible, vec!["pays de naissance : « Atlantide »".to_string()]);
    assert_eq!((d["pays_nationalite"].as_str(), d["code_nationalite_dsn"].as_str()), (Some("VN"), Some("04")));
    // Un pays de naissance que le COG n'a pas, mais la table « Pays » de la DSN si, se lit par son libelle.
    let (d, illisible) = lire("pays", "Kosovo");
    assert_eq!(d.get("pays_naissance"), Some(&json!("XK")), "{:?} {:?}", d, illisible);
    assert!(illisible.is_empty(), "{:?}", illisible);
}

// ------------------------------------------------------------------ un pays hors de la liste

#[test]
fn une_fiche_kosovare_avec_la_zone_04_s_enregistre_et_se_declare_04() {
    // Le champ est livre : du texte, pas declare (la DSN ne porte que 30.013).
    let champs = champs_livres();
    let c = champs["salarie"].as_array().unwrap().iter().find(|c| c["cle"] == "pays_nationalite_hors_liste").expect("le champ du pays hors liste");
    assert_eq!((c["forme"].as_str(), c["declare"].as_bool()), (Some("texte"), Some(false)));

    let racine = petite("kosovo");
    let (cle, mut fiche) = premiere_cle(&racine);
    fiche["pays_nationalite"] = json!("");
    fiche["pays_nationalite_hors_liste"] = json!("Kosovo");
    fiche["code_nationalite_dsn"] = json!("04");
    assert!(erreurs(&racine, &fiche).is_empty(), "{:?}", erreurs(&racine, &fiche));
    let r = route(&racine, "enregistrer-fiche", json!({"nom": "salarie", "cle": cle, "donnees": fiche, "depuis": JOUR}));
    outils::reussi(&r);
    let lu = outils::lire(&racine, "config/salaries.json");
    assert_eq!(lu[&cle]["code_nationalite_dsn"], json!("04"), "le choix de l'administrateur reste");
    assert_eq!(lu[&cle]["pays_nationalite_hors_liste"], json!("Kosovo"));

    // 03 se choisit aussi pour un pays hors liste (Svalbard, rattache a la Norvege, ecarte du COG faute de code ISO) :
    // le pays nomme leve l'ambiguite de l'ancien 03 « autre pays », qui sans pays refuse toujours.
    fiche["pays_nationalite_hors_liste"] = json!("Svalbard");
    fiche["code_nationalite_dsn"] = json!("03");
    assert!(erreurs(&racine, &fiche).is_empty(), "{:?}", erreurs(&racine, &fiche));

    // La DSN : 30.013 dit le choix, 30.015 le code du pays de naissance (ici la Serbie, dans la liste).
    let dsn = dsn_de_juillet(
        "kosovo",
        &[("CLAIRON", json!({"pays_naissance": "RS", "pays_nationalite": "", "pays_nationalite_hors_liste": "Kosovo", "code_nationalite_dsn": "04"}))],
    )
    .unwrap();
    assert_eq!(zone_et_naissance(&dsn, "CLAIRON"), de("04", "RS"));
}

#[test]
fn un_pays_de_la_liste_ne_laisse_pas_choisir_la_zone() {
    // Un pays de la liste dont la case contredit le code refuse, a la DSN.
    let e = dsn_de_juillet("maroc-02", &[("CLAIRON", json!({"pays_nationalite": "MA", "code_nationalite_dsn": "02"}))]).unwrap_err();
    assert!(e.contains("MA") && e.contains("04"), "{}", e);

    let racine = petite("liste");
    let (_, mut fiche) = premiere_cle(&racine);
    // Un pays de la liste ecrit « hors de la liste » : il se choisit dans la liste, la zone s'en deduit.
    for ecrit in ["Italie", "maroc", "NO"] {
        fiche["pays_nationalite"] = json!("");
        fiche["pays_nationalite_hors_liste"] = json!(ecrit);
        fiche["code_nationalite_dsn"] = json!("04");
        let e = erreurs(&racine, &fiche);
        assert_eq!(e.len(), 1, "{} : {:?}", ecrit, e);
        assert_eq!(e[0].0, "pays_nationalite_hors_liste", "{:?}", e);
        assert!(e[0].1.contains("dans la liste"), "{}", e[0].1);
    }
    // Deux pays de nationalite, l'un dans la liste, l'autre hors : l'un ou l'autre.
    fiche["pays_nationalite"] = json!("RS");
    fiche["pays_nationalite_hors_liste"] = json!("Kosovo");
    let e = erreurs(&racine, &fiche);
    assert_eq!(e.len(), 1, "{:?}", e);
    assert!(e[0].0 == "pays_nationalite_hors_liste" && e[0].1.contains("RS") && e[0].1.contains("Kosovo"), "{:?}", e);
    // Hors de la liste, ce n'est pas la France.
    fiche["pays_nationalite"] = json!("");
    fiche["code_nationalite_dsn"] = json!("01");
    let e = erreurs(&racine, &fiche);
    assert_eq!(e.len(), 1, "{:?}", e);
    assert!(e[0].0 == "code_nationalite_dsn" && e[0].1.contains("France"), "{:?}", e);
}

#[test]
fn une_fiche_hors_liste_sans_zone_refuse_en_nommant_la_nationalite() {
    let racine = petite("sans-zone");
    let (cle, mut fiche) = premiere_cle(&racine);
    fiche["pays_nationalite"] = json!("");
    fiche["pays_nationalite_hors_liste"] = json!("Kosovo");
    fiche.as_object_mut().unwrap().remove("code_nationalite_dsn");
    let e = erreurs(&racine, &fiche);
    assert_eq!(e.len(), 1, "{:?}", e);
    assert_eq!(e[0].0, "code_nationalite_dsn");
    assert!(e[0].1.contains("« Kosovo »") && e[0].1.contains("nationalité") && e[0].1.contains("S21.G00.30.013"), "{}", e[0].1);
    let r = route(&racine, "enregistrer-fiche", json!({"nom": "salarie", "cle": cle, "donnees": fiche, "depuis": JOUR}));
    outils::refus(&r);
    let e = dsn_de_juillet("sans-zone", &[("CLAIRON", json!({"pays_nationalite": "", "pays_nationalite_hors_liste": "Kosovo", "code_nationalite_dsn": ""}))]).unwrap_err();
    assert!(e.contains("« Kosovo »") && e.contains("S21.G00.30.013"), "{}", e);
    // Le questionnaire lit « kosovare » (le pays hors de la liste se garde dans sa case) et ne choisit pas la zone.
    let rep: HashMap<&'static str, String> = [("nationalite", "kosovare".to_string())].into_iter().collect();
    let (d, illisible) = paie::fiche_questionnaire::fiche_depuis_reponses(&rep);
    assert!(d.get("code_nationalite_dsn").is_none() && d.get("pays_nationalite").is_none(), "{:?}", d);
    assert_eq!(d.get("pays_nationalite_hors_liste"), Some(&json!("Kosovo")), "{:?}", d);
    assert!(illisible.is_empty(), "{:?}", illisible);
    // Une nationalite que rien ne nomme reste illisible : rien ne se devine.
    let rep: HashMap<&'static str, String> = [("nationalite", "atlante".to_string())].into_iter().collect();
    let (d, illisible) = paie::fiche_questionnaire::fiche_depuis_reponses(&rep);
    assert!(d.get("pays_nationalite_hors_liste").is_none(), "{:?}", d);
    assert_eq!(illisible, vec!["nationalité : « atlante »".to_string()]);
}

#[test]
fn ne_au_kosovo_ou_en_urss_se_declare_avec_le_code_de_la_table_pays_de_la_dsn() {
    let f: Map<String, Value> = json!({"nom": "X", "prenom": "Y", "pays_naissance": "XK", "pays_nationalite_hors_liste": "Kosovo", "code_nationalite_dsn": "04"})
        .as_object()
        .unwrap()
        .clone();
    let a = paie::champs::verifier_fiche("salarie", &f).unwrap();
    assert!(a.iter().all(|x| !(x["gravite"] == "erreur" && x["champ"] == "pays_naissance")), "{:?}", a);
    let dsn = dsn_de_juillet("ne-au-kosovo", &[("CLAIRON", json!({"pays_naissance": "XK", "pays_nationalite": "", "pays_nationalite_hors_liste": "Kosovo", "code_nationalite_dsn": "04"}))]).unwrap();
    assert_eq!(zone_et_naissance(&dsn, "CLAIRON"), de("04", "XK"), "la nationalite garde la zone choisie");
    let dsn = dsn_de_juillet("ne-en-urss", &[("CLAIRON", json!({"pays_naissance": "su"}))]).unwrap();
    assert_eq!(zone_et_naissance(&dsn, "CLAIRON").1.as_deref(), Some("SU"));
    // Un code que la table n'a pas refuse, en nommant la table.
    let e = dsn_de_juillet("ne-nulle-part", &[("CLAIRON", json!({"pays_naissance": "ZZ"}))]).unwrap_err();
    assert!(e.contains("« ZZ »") && e.contains("S21.G00.30.015") && e.contains("table « Pays »"), "{}", e);
}

/// Le registre unique du personnel porte la nationalite (C. trav., D1221-23, 1°) : le pays de la fiche
/// (`pays_nationalite` ou `pays_nationalite_hors_liste`), a defaut la categorie declaree en DSN.
#[test]
fn le_registre_montre_le_pays_de_nationalite_de_la_fiche() {
    let racine = petite("registre");
    outils::retoucher_fiches(
        &racine,
        &[
            ("BRIOCHE", json!({"pays_nationalite": "IT", "code_nationalite_dsn": "02"})),
            ("SERAPHIN", json!({"pays_nationalite_hors_liste": "Kosovo", "code_nationalite_dsn": "04"})),
        ],
    );
    let r = route(&racine, "exporter", json!({"etat": "registre-du-personnel", "mois": "2026-08", "apercu": true}));
    let r = outils::reussi(&r);
    let lignes = r["sections"][0]["lignes"].as_array().unwrap().clone();
    let nationalite = |nom: &str| -> String {
        lignes.iter().find(|l| l["nom"] == json!(nom)).unwrap_or_else(|| panic!("{} : {:?}", nom, lignes))["nationalite"].as_str().unwrap().to_string()
    };
    assert_eq!(nationalite("Brioche"), "Italie (Union européenne)");
    assert_eq!(nationalite("Seraphin"), "Kosovo (hors Union européenne et EEE)");
    assert_eq!(nationalite("Lemercier"), "française");
    let avert = r["avertissements"].as_array().unwrap();
    assert!(avert.iter().all(|a| !a.as_str().unwrap().contains("ne porte pas le pays")), "{:?}", avert);
}
