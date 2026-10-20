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
