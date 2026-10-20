//! `historique-mois` : ce qu'un mois a produit, envoye et recu, LU (tickets #219, #220, #222 du depot prive). Sur
//! `fictive-3090-petite` entiere, dont le journal de septembre 2026 et les pieces des retours sont fabriques ici, en
//! fictif : les fictives n'ont pas de retours archives.
//!
//! Le contrat : la route rend ce que la page de l'Historique relit -- les documents du mois avec le chemin que
//! `ouvrir-piece` ouvre, chaque envoi du journal (tests de ce logiciel compris), et sous chacun ses retours dans l'ordre
//! d'arrivee, traduits d'apres la specification du schema des bilans VT2.2 et le guide de l'API DSN v6.8
//! (`atelier/reference/sources/gip-mds-api-dsn/`), la piece brute quand le format n'y est pas.

use crate::outils::{compagnie, dossier_de_test, reussi, route};
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};

const MOIS: &str = "2026-09";
const JOUR: &str = "2026-10-20";
const REEL: &str = "FLUX-HIST-REEL";
const TEST: &str = "FLUX-HIST-TEST";

fn compagnie(nom: &str) -> PathBuf {
    compagnie::entiere("fictive-3090-petite", &dossier_de_test("historique_mois", nom).join("compagnie"))
}

/// Un rapport au format harmonise, comme la plate-forme le rend (`rapport/@type`, `envoi`, `declaration`), pour le SIREN
/// de `fictive-3090-petite`.
fn rapport(type_: &str, flux: &str, essai_reel: &str, etat: &str, anomalies: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<gipmds:rapport xmlns:gipmds=\"http://www.gip-mds.fr/\" profil=\"DSN\" type=\"{t}\" version=\"v01r08\">\
<envoi><envoi_identification><idflux>{f}</idflux><date_reception>2026-10-19</date_reception><heure_reception>17:11:00</heure_reception>\
<essai_reel>{e}</essai_reel></envoi_identification><envoi_bilan><envoi_etat>{x}</envoi_etat></envoi_bilan></envoi>\
<declaration><declaration_identification><nature>01</nature><type>01</type><fraction>11</fraction><ordre>1</ordre><mois>01092026</mois>\
<SIREN>123456923</SIREN></declaration_identification><declaration_bilan><etat>{x}</etat>{a}</declaration_bilan></declaration></gipmds:rapport>",
        t = type_, f = flux, e = essai_reel, x = etat, a = anomalies
    )
}

/// Une archive d'AER de nature 46, nommee comme France Travail les nomme (`retours::nom_aer`).
fn archive_aer() -> Vec<u8> {
    let mut o = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut o);
        let options = zip::write::SimpleFileOptions::default();
        z.start_file("AER_12345692300010__092026_X1_30092026_BRIOCHE_Fanchon_1_77.pdf", options).unwrap();
        z.write_all(b"%PDF-1.4 fictif").unwrap();
        z.finish().unwrap();
    }
    o.into_inner()
}

fn ligne(mode: &str, flux: &str, quand: &str, retours: Value) -> Value {
    json!({"quoi": "dsn", "quand": quand, "origine": "", "mois": MOIS, "fichier": "DSN_Septembre2026.dsn", "empreinte": "0000",
           "mode": mode, "qui": "test", "accepte": true, "destinataire": "net-entreprises", "flux": flux, "retours": retours,
           "reponse": "", "trace": null, "detail": {"nature": "01", "type": "01", "ordre": "1"}})
}

fn retour(nature: &str, statut: &str, publication: &str, extra: Value) -> Value {
    let mut r = json!({"nature": nature, "statut": statut, "publication": publication, "libelle": paie::retours::libelle_nature(nature)});
    for (k, v) in extra.as_object().unwrap() {
        r[k] = v.clone();
    }
    r
}

fn poser_le_mois(racine: &Path) {
    let p = racine.join("journal/depots.json");
    let mut j: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    j.retain(|l| l["mois"] != json!(MOIS));
    j.push(ligne("test", TEST, "2026-10-19T16:50:00", json!([retour("11", "OK", "20261019165500", json!({"verdict": "OK", "anomalies": []}))])));
    j.push(ligne("reel", REEL, "2026-10-19T17:11:00", json!([
        // Dans le desordre du journal : la route les rend dans l'ordre d'arrivee.
        retour("71", "OK", "20261020110000", json!({})),
        retour("11", "KO", "20261019172000", json!({"verdict": "KO", "anomalies": ["Code URSSAF invalide (Bloquante) -- BRIOCHE"]})),
        retour("10", "OK", "20261019171500", json!({"verdict": "OK", "anomalies": []})),
        retour("45", "ANO", "20261020090000", json!({})),
        retour("46", "OK", "20261020100000", json!({})),
        retour("60", "OK", "20261020080000", json!({})),
    ])));
    std::fs::write(&p, serde_json::to_string_pretty(&j).unwrap()).unwrap();
    let r = racine.join("retours");
    let ecrire = |nom: &str, o: &[u8]| std::fs::write(r.join(nom), o).unwrap();
    ecrire(&format!("20261019-165500_nature11_{}.xml", TEST), rapport("CCO", TEST, "01", "OK", "").as_bytes());
    ecrire(&format!("20261019-171500_nature10_{}.xml", REEL), rapport("AEE", REEL, "02", "OK", "").as_bytes());
    ecrire(
        &format!("20261019-172000_nature11_{}.xml", REEL),
        rapport(
            "BAN", REEL, "02", "KO",
            "<declaration_anomalie><code>CST-04</code><categorie>Bloquante</categorie><message>Code URSSAF invalide</message>\
             <rubrique_declenchante><identifiant>S21.G00.20.001</identifiant></rubrique_declenchante><numero_ligne>42</numero_ligne>\
             <salarie><nom_famille>BRIOCHE</nom_famille><prenoms>Fanchon</prenoms></salarie></declaration_anomalie>",
        )
        .as_bytes(),
    );
    ecrire(
        &format!("20261020-090000_nature45_{}.xml", REEL),
        rapport("CRM", REEL, "02", "ANO", "<declaration_anomalie><code>FT-12</code><message>Numéro d'objet inconnu</message></declaration_anomalie>").as_bytes(),
    );
    ecrire(&format!("20261020-100000_nature46_{}.txt", REEL), &archive_aer());
    ecrire(&format!("20261020-110000_nature71_{}.xml", REEL), b"<?xml version=\"1.0\"?><crm_agirc_arrco><statut>OK</statut></crm_agirc_arrco>");
}

#[test]
fn un_mois_rend_ses_documents_ses_envois_et_leurs_retours_lus() {
    let racine = compagnie("lu");
    poser_le_mois(&racine);
    let r = reussi(&route(&racine, "historique-mois", json!({"mois": MOIS}), JOUR)).clone();

    // Les documents du mois, chacun avec le chemin qu'ouvre `ouvrir-piece`.
    let docs = r["documents"].as_array().unwrap();
    let dsn = docs.iter().find(|d| d["nom"] == json!("DSN_Septembre2026.dsn")).unwrap();
    assert_eq!(dsn["chemin"], json!("sorties/2026-09/DSN_Septembre2026.dsn"));
    assert_eq!(dsn["sorte"], json!("DSN"));
    assert!(racine.join(dsn["chemin"].as_str().unwrap()).is_file());

    // Les deux envois, le reel (le plus recent) d'abord, le test de ce logiciel montre aussi.
    let envois = r["envois"].as_array().unwrap();
    assert_eq!(envois.len(), 2, "{}", r);
    assert_eq!((envois[0]["mode"].clone(), envois[0]["flux"].clone()), (json!("reel"), json!(REEL)));
    assert_eq!(envois[0]["acte"], json!("DSN mensuelle"));
    assert_eq!(envois[0]["attend_retour"], json!(true));
    assert_eq!((envois[1]["mode"].clone(), envois[1]["flux"].clone()), (json!("test"), json!(TEST)));

    // Les retours dans l'ordre d'arrivee (la publication), chacun avec son organisme.
    let ret = envois[0]["retours"].as_array().unwrap();
    let natures: Vec<&str> = ret.iter().map(|x| x["nature"].as_str().unwrap()).collect();
    assert_eq!(natures, ["10", "11", "60", "45", "46", "71"]);
    let n = |c: &str| ret.iter().find(|x| x["nature"] == json!(c)).unwrap();

    // L'accuse et le bilan d'anomalies, traduits d'apres le schema des bilans.
    assert_eq!(n("10")["titre"], json!("Accusé d'enregistrement électronique"));
    let ban = n("11");
    assert_eq!(ban["titre"], json!("Bilan d'anomalies"));
    assert_eq!(ban["traduite"], json!(true));
    assert!(ban["phrases"].as_array().unwrap().iter().any(|p| p == "DSN mensuelle de 2026-09, ordre 1 : refusée (KO)."), "{}", ban);
    assert_eq!(ban["anomalies"][0]["code"], json!("CST-04"));
    assert_eq!(ban["anomalies"][0]["message"], json!("Code URSSAF invalide"));
    assert_eq!(ban["anomalies"][0]["rubrique"], json!("S21.G00.20.001"));
    assert_eq!(ban["anomalies"][0]["ligne"], json!("42"));
    assert_eq!(ban["anomalies"][0]["salarie"], json!("BRIOCHE Fanchon"));
    assert_eq!(ban["piece"], json!(format!("retours/20261019-172000_nature11_{}.xml", REEL)));
    assert_eq!(ban["quand"], json!("2026-10-19T17:20:00"));

    // Le compte rendu de France Travail (format harmonise) : son organisme, son statut, son anomalie.
    let crm = n("45");
    assert_eq!(crm["organisme"], json!("France Travail"));
    assert!(crm["phrases"][0].as_str().unwrap().starts_with("Statut ANO"), "{}", crm);
    assert_eq!(crm["anomalies"][0]["code"], json!("FT-12"));

    // L'archive d'AER : qui elle atteste.
    let aer = n("46");
    assert!(aer["phrases"].as_array().unwrap().iter().any(|p| p.as_str().unwrap().starts_with("BRIOCHE Fanchon")), "{}", aer);

    // Un accuse sans document, et un format dont la norme n'est pas deposee : le statut, la piece brute.
    assert_eq!(n("60")["piece"], Value::Null);
    assert!(n("60")["phrases"].as_array().unwrap().iter().any(|p| p.as_str().unwrap().contains("sans document")));
    let agirc = n("71");
    assert_eq!(agirc["traduite"], json!(false));
    assert_eq!(agirc["organisme"], json!("l'AGIRC-ARRCO"));
    assert!(agirc["piece"].as_str().unwrap().ends_with("_nature71_FLUX-HIST-REEL.xml"));

    // Le bilan du test se lit comme celui du reel, marque essai.
    let cco = &envois[1]["retours"][0];
    assert_eq!(cco["titre"], json!("Certificat de conformité"));
    assert!(cco["phrases"].as_array().unwrap().iter().any(|p| p.as_str().unwrap().starts_with("Envoi d'essai")));

    // `ouvrir-piece` n'ouvre que ce que la route a liste.
    assert!(paie::historique_mois::piece_listee(&r, "sorties/2026-09/DSN_Septembre2026.dsn"));
    assert!(paie::historique_mois::piece_listee(&r, &format!("retours/20261020-110000_nature71_{}.xml", REEL)));
    assert!(!paie::historique_mois::piece_listee(&r, "config/entreprise.json"));
    assert!(!paie::historique_mois::piece_listee(&r, "sorties/2026-09/../../config/entreprise.json"));
}

#[test]
fn un_mois_illisible_est_refuse_et_rien_ne_s_ecrit() {
    let racine = compagnie("refus");
    let avant = std::fs::read(racine.join("journal/depots.json")).unwrap();
    for m in ["../config", "2026-9", ""] {
        let r = route(&racine, "historique-mois", json!({"mois": m}), JOUR);
        assert_eq!(r["ok"], json!(false), "{}", r);
    }
    assert_eq!(std::fs::read(racine.join("journal/depots.json")).unwrap(), avant);
}

/// Ticket #222 : une DSN deposee et passee n'a plus de bouton « Deposer la DSN… » (`vide`) ; une DSN ecrite et pas
/// deposee en a un. Et l'Historique ne mene a la page Paie que pour un mois ou il reste quelque chose a faire.
#[test]
fn une_dsn_passee_n_a_plus_de_depot_a_proposer() {
    let racine = compagnie("passee");
    let b = reussi(&route(&racine, "bilan-du-mois", json!({"mois": "2026-08"}), JOUR)).clone();
    assert_eq!(b["dsn"]["couleur"], json!("bien"), "{}", b);
    assert_eq!(b["dsn"]["vide"], json!(true), "{}", b);
    let b = reussi(&route(&racine, "bilan-du-mois", json!({"mois": "2026-09"}), JOUR)).clone();
    assert!(b["dsn"].get("vide").is_none(), "{}", b);
    let e = reussi(&route(&racine, "etat", json!({}), JOUR)).clone();
    let h = |m: &str| e["historique"].as_array().unwrap().iter().find(|x| x["mois"] == json!(m)).unwrap()["a_faire"].clone();
    assert_eq!(h("2026-08"), json!(false));
    assert_eq!(h("2026-09"), json!(true));
}

// ------------------------------------------------- NEOReS et organismes complementaires

/// Un CRM 118 (effectifs) au format NEOReS, fabrique d'apres le cahier technique 2026.1.2
/// (`atelier/reference/sources/neores/gip-mds-neores-cahier-technique-2026.1.2-20251024.txt` : profil 04 « parametre utile
/// en paie », bloc `RO.B008`, rubriques ecrites `RO_Bnnn_Rnnn` en transmission, §2.2 ; type A01 de la table des types).
fn crm_118() -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<RO><RO_R001>P26V01</RO_R001>\
<RO_B000><RO_B000_R001>{f}</RO_B000_R001><RO_B000_R002>1</RO_B000_R002><RO_B000_R004>12345692300010</RO_B000_R004><RO_B000_R005>11</RO_B000_R005>\
<RO_B000_R006>2026-09-01</RO_B000_R006><RO_B000_R013>02</RO_B000_R013></RO_B000>\
<RO_B001><RO_B001_R001>04</RO_B001_R001><RO_B002><RO_B002_R001>EFF-FICTIF-1</RO_B002_R001><RO_B002_R002>118</RO_B002_R002>\
<RO_B002_R003>CRM Effectifs</RO_B002_R003><RO_B002_R004>01</RO_B002_R004><RO_B002_R005>URSSAF CAISSE NATIONALE</RO_B002_R005>\
<RO_B002_R006>12345692300099</RO_B002_R006><RO_B002_R007>12345692300099</RO_B002_R007><RO_B002_R011>2026-10-20T07:00:00</RO_B002_R011>\
<RO_B004><RO_B004_R001>EFF</RO_B004_R001><RO_B004_R002>Effectifs de l'entreprise</RO_B004_R002><RO_B004_R003>04</RO_B004_R003>\
<RO_B004_R011>Calcul des effectifs annuels</RO_B004_R011><RO_B004_R013>Prendre en compte ces effectifs en paie.</RO_B004_R013>\
<RO_B008><RO_B008_R001>A01</RO_B008_R001><RO_B008_R002>3.5</RO_B008_R002><RO_B008_R003>2026-01-01</RO_B008_R003>\
<RO_B008_R004>2026-12-31</RO_B008_R004><RO_B008_R006>2026-01-15T10:00:00</RO_B008_R006></RO_B008></RO_B004></RO_B002></RO_B001></RO>",
        f = REEL
    )
}

/// Un CRM 119 (recouvrement, H+4) au format NEOReS, statut ANO : une cible d'action (`RO.B004`), son detail (`RO.B005`),
/// le salarie (`RO.B006`, sans NIR), la donnee DSN en ecart (`RO.B007`) ; et un ecart d'identite (`RO.B009`).
fn crm_119() -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<RO><RO_R001>P26V01</RO_R001>\
<RO_B000><RO_B000_R001>{f}</RO_B000_R001><RO_B000_R006>2026-09-01</RO_B000_R006><RO_B000_R013>02</RO_B000_R013></RO_B000>\
<RO_B001><RO_B001_R001>01</RO_B001_R001><RO_B002><RO_B002_R002>119</RO_B002_R002><RO_B002_R003>CRM recouvrement H+4</RO_B002_R003>\
<RO_B002_R004>01</RO_B002_R004><RO_B002_R005>URSSAF CAISSE NATIONALE</RO_B002_R005><RO_B002_R008>2026-09-01</RO_B002_R008>\
<RO_B002_R009>2026-09-30</RO_B002_R009><RO_B003><RO_B003_R001>Recouvrement</RO_B003_R001><RO_B003_R002>ANO</RO_B003_R002></RO_B003>\
<RO_B004><RO_B004_R001>CTL-FICTIF-42</RO_B004_R001><RO_B004_R002>Taux de cotisation incohérent</RO_B004_R002><RO_B004_R003>01</RO_B004_R003>\
<RO_B004_R004>05</RO_B004_R004><RO_B004_R006>01</RO_B004_R006><RO_B004_R009>1</RO_B004_R009>\
<RO_B004_R011>Le taux déclaré est comparé au taux attendu pour ce code de cotisation.</RO_B004_R011>\
<RO_B004_R013>Corriger le taux dans la DSN du mois suivant.</RO_B004_R013>\
<RO_B005><RO_B005_R001>Le taux déclaré pour ce contrat n'est pas le taux attendu.</RO_B005_R001>\
<RO_B006><RO_B006_R003>BRIOCHE</RO_B006_R003><RO_B006_R004>Fanchon</RO_B006_R004><RO_B006_R006>2026-09-02</RO_B006_R006>\
<RO_B006_R007>BR1.26.09</RO_B006_R007><RO_B006_R008>1990-01-01</RO_B006_R008></RO_B006>\
<RO_B007><RO_B007_R001>S21.G00.81.007</RO_B007_R001><RO_B007_R002>0.50</RO_B007_R002><RO_B007_R003>0.60</RO_B007_R003>\
<RO_B007_R009>01</RO_B007_R009></RO_B007></RO_B005></RO_B004>\
<RO_B004><RO_B004_R001>ID-FICTIF</RO_B004_R001><RO_B004_R002>Identité en écart</RO_B004_R002><RO_B004_R003>02</RO_B004_R003>\
<RO_B009><RO_B009_R002>12345692399</RO_B009_R002><RO_B009_R003>2900199123456</RO_B009_R003><RO_B009_R004>BRIOCHE</RO_B009_R004>\
<RO_B009_R005>BRIOCHE</RO_B009_R005><RO_B009_R006>Fanchon</RO_B009_R006><RO_B009_R007>Françoise</RO_B009_R007></RO_B009></RO_B004>\
</RO_B002></RO_B001></RO>",
        f = REEL
    )
}

/// Un compte rendu complet (CRC) d'organisme complementaire, nature 51, fabrique d'apres le schema et la specification
/// CRM DSN OC P3 V1.1.4 (`atelier/reference/sources/gip-mds-api-dsn/ctip-crm-dsn-oc-p3-v1.1.4-schema-20230915.xsd`),
/// en ISO-8859-1 comme le guide le dit (§7.1) : code retour global 2, un mandat SEPA a creer, deux retours metier.
fn crm_oc() -> Vec<u8> {
    let t = format!(
        "<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?>\n<Fichier VersionCRM=\"114\" IdEnvoi=\"{f}\" TypCRM=\"3\" PointDepot=\"01\" CodeEnvoi=\"02\" VersionNorme=\"P26V01\">\
<Declaration IdDec=\"1\" NatureDecla=\"01\" TypeDecla=\"01\" FractionDecla=\"11\" NoOrdreDecla=\"1\" DateMoisDec=\"30092026\" SirenDecla=\"123456923\" \
CodeOC=\"P9999\" CodeProducteurCRM=\"P9999\" NomProducteurCRM=\"Prévoyance fictive des arts\" NbreSal=\"3\" NbreErreur=\"1\" NbreRad=\"1\" \
NomContact=\"Service DSN fictif\" MailContact=\"dsn@exemple.invalid\" CodeRetourDecla=\"2\" ComplementRetourDecla=\"Un salarié n'a pas pu être pris en compte.\">\
<MandatNetE BIC=\"AAAAFRPPXXX\" IBAN=\"FR7630001000000000000000185\" ICS=\"FR00ZZZ000000\"/>\
<RetourMetier IdRetour=\"1\" NbErreursSimilaires=\"1\" CodeRetourMetier=\"EADH02\" LibelCodeRetourMetier=\"Référence Contrat de l'adhésion erronée\" \
CriticiteRetour=\"C\" RubriqueBlocRetour=\"S21.G00.15.001\" LibelRubBlocRetour=\"Référence du contrat de Prévoyance\" ValeurRubriqueErreur=\"C123\" ValeurCorrigeeAjoutee=\"C321\">\
<ContexteRetourMetier Rubrique=\"S21.G00.30.002\" Libelle=\"Nom de famille\" Valeur=\"BRIOCHE\"/>\
<ContexteRetourMetier Rubrique=\"S21.G00.30.004\" Libelle=\"Prénoms\" Valeur=\"Fanchon\"/></RetourMetier>\
<RetourMetier IdRetour=\"2\" CodeRetourMetier=\"IEVT07\" LibelCodeRetourMetier=\"Radiation prise en compte suite a fin de contrat\" \
RubriqueBlocRetour=\"S21.G00.62\" LibelRubBlocRetour=\"Fin du contrat\" DateEvt=\"15092026\"/>\
</Declaration></Fichier>",
        f = REEL
    );
    // ISO-8859-1 : chaque caractere de ce texte tient dans un octet.
    t.chars().map(|c| c as u32 as u8).collect()
}

#[test]
fn les_comptes_rendus_neores_et_des_organismes_complementaires_se_traduisent() {
    let racine = compagnie("neores-oc");
    let p = racine.join("journal/depots.json");
    let mut j: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    j.retain(|l| l["mois"] != json!(MOIS));
    j.push(ligne("reel", REEL, "2026-10-19T17:11:00", json!([
        retour("118", "OK", "20261020070000", json!({})),
        retour("119", "ANO", "20261020080000", json!({})),
        retour("51", "OK", "20261020090000", json!({})),
    ])));
    std::fs::write(&p, serde_json::to_string_pretty(&j).unwrap()).unwrap();
    let r = racine.join("retours");
    std::fs::write(r.join(format!("20261020-070000_nature118_{}.xml", REEL)), crm_118()).unwrap();
    std::fs::write(r.join(format!("20261020-080000_nature119_{}.xml", REEL)), crm_119()).unwrap();
    std::fs::write(r.join(format!("20261020-090000_nature51_{}.xml", REEL)), crm_oc()).unwrap();

    let h = reussi(&route(&racine, "historique-mois", json!({"mois": MOIS}), JOUR)).clone();
    let ret = h["envois"][0]["retours"].as_array().unwrap().clone();
    let n = |c: &str| ret.iter().find(|x| x["nature"] == json!(c)).unwrap().clone();
    let a_la_phrase = |x: &Value, debut: &str| x["phrases"].as_array().unwrap().iter().any(|p| p.as_str().unwrap().starts_with(debut));

    // Le 118 : un parametre (RO.B008), dit avec sa periode d'effet et sa date de calcul ; ce n'est pas une anomalie.
    let eff = n("118");
    assert_eq!(eff["traduite"], json!(true), "{}", eff);
    assert_eq!(eff["titre"], json!("CRM Effectifs"));
    assert_eq!(eff["organisme"], json!("URSSAF CAISSE NATIONALE"));
    assert!(a_la_phrase(&eff, "Effectif moyen annuel (niveau entreprise) (A01) : 3.5, du 01/01/2026 au 31/12/2026 (calculé le 15/01/2026)."), "{}", eff);
    assert_eq!(eff["anomalies"], json!([]), "{}", eff);
    // Le meme 118, lu par le meme lecteur, fait l'effectif de la compagnie (`config/effectifs.json`).
    paie::releve_retours::reconstruire_effectifs(&racine).unwrap();
    let e: Value = serde_json::from_str(&std::fs::read_to_string(racine.join("config/effectifs.json")).unwrap()).unwrap();
    assert_eq!(e["annees"]["2026"]["A01"]["valeur"], json!(3.5), "{}", e);

    // Le 119 : la cible d'action en anomalie (code du traitement, rubrique, salarie et contrat), l'action attendue en
    // phrase ; l'ecart d'identite (RO.B009) dit l'identite de reference.
    let rec = n("119");
    let a = &rec["anomalies"][0];
    assert_eq!(a["code"], json!("CTL-FICTIF-42"), "{}", rec);
    assert_eq!(a["rubrique"], json!("S21.G00.81.007"));
    assert_eq!(a["salarie"], json!("BRIOCHE Fanchon (contrat BR1.26.09)"));
    assert!(a["message"].as_str().unwrap().starts_with("Taux de cotisation incohérent"), "{}", a);
    assert!(a_la_phrase(&rec, "Des écarts ont été constatés"), "{}", rec);
    assert!(a_la_phrase(&rec, "CTL-FICTIF-42 — Action attendue : Corriger le taux"), "{}", rec);
    assert!(a_la_phrase(&rec, "CTL-FICTIF-42 — Détail : Le taux déclaré pour ce contrat n'est pas le taux attendu ; S21.G00.81.007 : déclaré « 0.50 », attendu « 0.60 »"), "{}", rec);
    assert!(rec["phrases"].as_array().unwrap().iter().any(|p| p.as_str().unwrap().contains("prénom de référence « Françoise »")), "{}", rec);

    // Le 51 : l'organisme qui l'a produit, le code retour global, le mandat a creer, chaque retour metier.
    let oc = n("51");
    assert_eq!(oc["traduite"], json!(true), "{}", oc);
    assert_eq!(oc["organisme"], json!("Prévoyance fictive des arts"));
    assert_eq!(oc["titre"], json!("Compte rendu complet (CRC)"));
    assert!(a_la_phrase(&oc, "DSN mensuelle de 2026-09, vue par Prévoyance fictive des arts, code P9999 : DSN partiellement prise en compte"), "{}", oc);
    assert!(a_la_phrase(&oc, "Pas de mandat de prélèvement SEPA pour le compte FR7630001000000000000000185 : à créer sur net-entreprises"), "{}", oc);
    let adh = &oc["anomalies"][0];
    assert_eq!(adh["code"], json!("EADH02"));
    assert_eq!(adh["salarie"], json!("BRIOCHE Fanchon"));
    assert_eq!(adh["categorie"], json!("erreur, critique : à corriger impérativement dans la DSN suivante"));
    assert!(adh["message"].as_str().unwrap().contains("déclaré « C123 » — attendu « C321 »"), "{}", adh);
    assert!(oc["anomalies"][1]["message"].as_str().unwrap().contains("événement du 15/09/2026"), "{}", oc);
}

/// Un contrat revenu signe dit de qui il est, et lequel : le nom que la ligne porte, sinon celui de la fiche.
#[test]
fn un_contrat_revenu_signe_dit_de_qui_il_est() {
    let racine = compagnie("contrat-recu");
    let p = racine.join("journal/depots.json");
    let mut j: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    j.retain(|l| l["mois"] != json!(MOIS));
    let recu = |quand: &str, detail: Value| json!({"quoi": "contrat-recu", "quand": quand, "origine": "", "mois": MOIS, "fichier": null, "empreinte": null,
        "mode": "reel", "qui": "interface", "accepte": true, "destinataire": "", "flux": "", "retours": [], "reponse": "", "trace": null, "detail": detail});
    j.push(recu("2026-09-03T10:00:00", json!({"salarie": "BRIOCHE", "debut": "2026-09-02", "recu": true, "numero": "BR1.26.09", "nom": "BRIOCHE Fanchon", "origine": "courriel"})));
    // Une ligne d'avant les noms : la fiche le donne.
    j.push(recu("2026-09-04T10:00:00", json!({"salarie": "BRIOCHE", "debut": "2026-09-10", "recu": true})));
    std::fs::write(&p, serde_json::to_string_pretty(&j).unwrap()).unwrap();
    let h = reussi(&route(&racine, "historique-mois", json!({"mois": MOIS}), JOUR)).clone();
    let envois = h["envois"].as_array().unwrap();
    assert_eq!(envois[0]["acte"], json!("Contrat revenu signé — BRIOCHE Fanchon"), "{}", h);
    assert_eq!(envois[0]["resume"], json!("contrat commençant le 10/09/2026 — reçu signé le 04/09/2026 à 10:00, noté à la main"));
    assert_eq!(envois[1]["acte"], json!("Contrat revenu signé — BRIOCHE Fanchon"));
    assert_eq!(envois[1]["resume"], json!("contrat n° BR1.26.09 commençant le 02/09/2026 — signé d'un clic le 03/09/2026 à 10:00"));
}
