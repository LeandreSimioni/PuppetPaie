//! `paie route` : les routes du mois, sans calcul de paie (`etat-par-paie`, `etat-du-mois`, `embauches-a-declarer`,
//! `mois-a-ouvrir`, `bilan-du-mois`, `fichiers-deposables`). Sur `fictive-3090-petite` (config, aout 2026), avec le journal
//! des depots que chaque test ecrit ; le contexte est au 03/10/2026, le jour du test varie.

use crate::outils::{compagnie, dossier_de_test, ecrire, lire, refus, reussi, route_avec, route_en_processus};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

const MOIS: &str = "2026-08";

fn compagnie(nom: &str) -> PathBuf {
    compagnie::saisies("fictive-3090-petite", &[MOIS], &dossier_de_test("routes_mois", nom).join("compagnie"))
}

fn journal(c: &Path, lignes: &[Value]) {
    ecrire(c, "journal/depots.json", &json!(lignes));
}

fn saisie(c: &Path, mois: &str) -> PathBuf {
    c.join("paies").join(format!("{}.json", mois))
}

/// Un mois sans aucun engagement : saisi, mais sans paie.
fn mois_vide(c: &Path, mois: &str) {
    ecrire(c, &format!("paies/{}.json", mois), &json!({"mois": mois, "engagements": []}));
}

fn dossier_sorties(c: &Path, mois: &str) -> PathBuf {
    let d = c.join("sorties").join(mois);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// La reponse entiere de la route, le jour `aujourdhui`.
fn route(c: &Path, nom: &str, charge: Value, aujourdhui: &str) -> Value {
    route_avec(c, nom, charge, "2026-10-03", &[("PUPPETPAIE_AUJOURDHUI", aujourdhui)])
}

fn rendu(c: &Path, nom: &str, charge: Value, aujourdhui: &str) -> Value {
    reussi(&route(c, nom, charge, aujourdhui)).clone()
}

fn ligne(quoi: &str, mois: &str, mode: &str, retours: Value, detail: Value) -> Value {
    json!({"quoi": quoi, "quand": "2026-09-05T10:00:00", "origine": "", "mois": mois, "fichier": null, "empreinte": null,
           "mode": mode, "qui": "interface", "accepte": true, "destinataire": "x", "flux": "WSX", "retours": retours,
           "reponse": "", "trace": null, "detail": detail})
}

fn dpae_declaree(cles: &[&str], retours: Value) -> Value {
    let embauches: Vec<Value> = cles.iter().map(|c| json!({"salarie": c, "debut": "2026-08-19", "fin": "2026-08-22"})).collect();
    ligne("dpae", MOIS, "reel", retours, json!({"embauches": embauches}))
}

fn dsn_reelle(mois: &str, verdict: Option<&str>, detail: Value) -> Value {
    let retours = match verdict {
        Some(v) => json!([{"nature": "11", "verdict": v, "anomalies": if v == "KO" { json!(["B1-208-10"]) } else { json!([]) }}]),
        None => json!([]),
    };
    ligne("dsn", mois, "reel", retours, detail)
}

const CLES: [&str; 3] = ["BRIOCHE", "SERAPHIN", "LEMERCIER"];

#[test]
fn etat_par_paie_une_ligne_par_engagement_avec_le_brut_du_calcul_stocke() {
    let c = &compagnie("etat-par-paie");
    journal(c, &[dpae_declaree(&CLES, json!([{"nature": "11", "verdict": "OK", "anomalies": []}]))]);
    let r = rendu(c, "etat-par-paie", json!({"mois": MOIS}), "2026-10-03");
    assert_eq!(r["mois"], MOIS);
    let paies = r["paies"].as_array().unwrap();
    assert_eq!(paies.len(), 3);
    let p = &paies[0];
    assert_eq!(p["salarie"], "BRIOCHE");
    assert_eq!(p["nom"], "BRIOCHE Fanchon");
    assert_eq!(p["spectacle"], "Ubu roi");
    assert_eq!(p["numero"], "BR1.26.04");
    assert_eq!(p["debut"], "2026-08-19");
    assert_eq!(p["fin"], "2026-08-22");
    assert_eq!(p["contrat"], json!({"premier_mois": true, "dernier_mois": true, "depuis": "2026-08", "jusqu_a": "2026-08"}));
    assert_eq!(p["etats"]["dpae"]["passee"], true);
    assert_eq!(p["etats"]["dpae"]["fait"], "2026-09-05T10:00:00");
    assert_eq!(p["etats"]["dpae"]["maille"], "paie");
    assert_eq!(p["etats"]["signature"]["maille"], "paie");
    assert_eq!(p["etats"]["documents"], json!({"fait": null, "maille": "salarie", "caduc": false, "pourquoi": ""}));
    assert_eq!(p["etats"]["dsn"], json!({"fait": null, "maille": "mois"}));
    assert_eq!(p["valide"], Value::Null);
    // Le brut est celui du calcul de la saisie, a deux decimales.
    let o = HashMap::from([("racine".to_string(), c.to_string_lossy().into_owned()), ("date".to_string(), "2026-10-03".to_string())]);
    let (direct, _) = paie::calcul_du_mois::calculer_suivi(&o, &json!({"fichier": saisie(c, MOIS).to_str().unwrap()})).unwrap();
    let nombre = |v: &Value| v.as_str().unwrap().parse::<f64>().unwrap();
    for (i, p) in paies.iter().enumerate() {
        assert_eq!(nombre(&p["brut"]), nombre(&direct["paies"][i]["brut"]), "paie {}", i);
        assert_eq!(p["brut"].as_str().unwrap().split('.').nth(1).unwrap().len(), 2, "deux decimales");
    }
    assert_eq!(paies[0]["brut"], "620.00");
    // Le calcul est ecrit a cote de la saisie ; la saisie n'a pas bouge.
    assert!(c.join("paies/2026-08.calcul.json").exists());
    assert_eq!(rendu(c, "etat-par-paie", json!({"mois": MOIS}), "2026-10-03"), r);
}

#[test]
fn un_mois_sans_saisie_n_a_pas_de_paie_et_le_bilan_n_a_rien_a_dire() {
    let c = &compagnie("sans-saisie");
    assert_eq!(rendu(c, "etat-par-paie", json!({"mois": "2031-01"}), "2026-10-03"), json!({"mois": "2031-01", "paies": []}));
    let b = rendu(c, "bilan-du-mois", json!({"mois": "2031-01"}), "2026-10-03");
    let rien = json!({"couleur": "bien", "courte": "rien ce mois-ci", "vide": true, "phrase": "aucune paie ce mois-ci : rien à faire à cette étape"});
    assert_eq!(b["dpae"], rien);
    assert_eq!(b["contrat-signature"], rien);
    assert_eq!(b["documents"], rien);
    // L'etape DSN dit aussi son echeance de depot (`echeance_dsn`) : le samedi 15/02/2031 se reporte au lundi 17/02/2031,
    // a midi (CSS R. 133-14, I, `atelier/reference/sources/code-de-la-securite-sociale/xml/R133-14.xml`).
    let mut dsn = b["dsn"].clone();
    let echeance = dsn.as_object_mut().unwrap().remove("echeance_depot").unwrap();
    assert_eq!((echeance["jour"].as_str(), echeance["heure"].as_str()), (Some("2031-02-17"), Some("12:00")), "{}", echeance);
    assert_eq!(dsn, json!({"couleur": "", "phrase": "DSN pas encore déposée"}));
}

#[test]
fn le_bilan_dit_l_etat_de_chaque_acte() {
    let c = &compagnie("bilan");
    // Rien n'est parti.
    let b = rendu(c, "bilan-du-mois", json!({"mois": MOIS}), "2026-10-03");
    assert_eq!(b["dpae"], json!({"couleur": "", "phrase": "aucune DPAE déclarée — 3 à faire"}));
    assert_eq!(b["contrat-signature"], json!({"couleur": "", "phrase": "aucun contrat envoyé — 3 à faire signer"}));
    // Ce qui part, nomme par le moteur : trois contrats d'usage qui finissent dans le mois (bulletin, certificat CS, AER).
    assert_eq!(b["documents"], json!({"couleur": "", "phrase": "Documents pas encore envoyés — 3 salariés", "detail": "bulletin, certificat Congés Spectacles, AER"}));
    // Deux DPAE passees, une partie sans retour.
    journal(c, &[
        dpae_declaree(&CLES[..2], json!([{"nature": "41", "dpae": []}])),
        ligne("dpae", MOIS, "reel", json!([]), json!({"embauches": [{"salarie": "LEMERCIER", "debut": "2026-08-19", "fin": "2026-08-22"}]})),
    ]);
    let b = rendu(c, "bilan-du-mois", json!({"mois": MOIS}), "2026-10-03");
    assert_eq!(b["dpae"]["couleur"], "alerte");
    assert_eq!(b["dpae"]["attente"], true);
    assert_eq!(b["dpae"]["courte"], "parties — retour attendu");
    assert_eq!(b["dpae"]["phrase"], "Toutes les DPAE sont parties (3) — 1 retour attendu");
    // Une DPAE refusee.
    journal(c, &[dpae_declaree(&CLES[..1], json!([{"nature": "11", "verdict": "KO", "anomalies": ["E1"]}]))]);
    let b = rendu(c, "bilan-du-mois", json!({"mois": MOIS}), "2026-10-03");
    assert_eq!(b["dpae"], json!({"couleur": "refus", "phrase": "1 DPAE refusée — à corriger et à renvoyer"}));
    // Les contrats : envoyes puis signes ; les documents pour deux salaries sur trois.
    let contrats: Vec<Value> = CLES.iter().map(|k| json!({"salarie": k, "debut": "2026-08-19", "mois": MOIS})).collect();
    journal(c, &[
        ligne("contrat-signature", MOIS, "reel", json!([]), json!({"contrats": contrats})),
        ligne("contrat-recu", MOIS, "reel", json!([]), json!({"salarie": "BRIOCHE", "debut": "2026-08-19", "recu": true})),
        ligne("documents", MOIS, "reel", json!([]), json!({"salaries": [{"cle": "BRIOCHE"}, {"cle": "SERAPHIN"}]})),
    ]);
    let b = rendu(c, "bilan-du-mois", json!({"mois": MOIS}), "2026-10-03");
    assert_eq!(b["contrat-signature"]["phrase"], "Tous les contrats sont envoyés (3) — 1 reçu signé, 2 à recevoir");
    assert_eq!(b["contrat-signature"]["attente"], true);
    assert_eq!(b["documents"], json!({"couleur": "alerte", "phrase": "Documents envoyés à 2 salariés sur 3", "detail": "bulletin, certificat Congés Spectacles, AER"}));
}

#[test]
fn une_dsn_reelle_ferme_le_mois_et_une_paie_modifiee_la_rouvre_tant_que_la_norme_l_admet() {
    let c = &compagnie("annule-remplace");
    // La paie du mois n'a pas bouge depuis la DSN : l'empreinte notee est la sienne.
    journal(c, &[dsn_reelle(MOIS, Some("OK"), json!({"type": "01", "ordre": "1"}))]);
    let r = rendu(c, "etat-par-paie", json!({"mois": MOIS}), "2026-09-10");
    for p in r["paies"].as_array().unwrap() {
        assert_eq!(p["valide"], "2026-09-05T10:00:00", "un mois parti se lit valide au jour du depot");
        assert_eq!(p["etats"]["dsn"]["fait"], "2026-09-05T10:00:00");
    }
    let b = rendu(c, "bilan-du-mois", json!({"mois": MOIS}), "2026-09-10");
    assert_eq!(b["dsn"]["phrase"], "DSN déposée le 05/09/2026 à 10:00 — passée");
    assert_eq!(b["dsn"]["couleur"], "bien");
    assert_eq!(b["dsn"]["remplacement"]["limite"], "2026-09-14");
    assert_eq!(b["dsn"]["remplacement"]["admis"], true);
    assert_eq!(b["dsn"]["remplacement"]["dernier_ordre"], 1);
    assert_eq!(b["dsn"]["remplacement"]["prochain_ordre"], 2);
    assert_eq!(
        b["dsn"]["remplacement"]["phrase"],
        "Une modification reste possible : la DSN refaite partira en annule-et-remplace (ordre 2), jusqu'au 14/09/2026 à minuit"
    );
    // Une paie reouverte a la main n'est plus lue validee.
    modifier_saisie(c, |v| v["engagements"][1]["rouverte"] = json!("2026-09-06T10:00:00"));
    let r = rendu(c, "etat-par-paie", json!({"mois": MOIS}), "2026-09-10");
    assert_eq!(r["paies"][1]["valide"], Value::Null);
    assert_eq!(r["paies"][0]["valide"], "2026-09-05T10:00:00");
    // La paie a change depuis la DSN : annule-et-remplace a envoyer, puis plus admis.
    journal(c, &[dsn_reelle(MOIS, Some("OK"), json!({"type": "01", "ordre": 3, "empreinte_saisie": "0000000000000000"}))]);
    let b = rendu(c, "bilan-du-mois", json!({"mois": MOIS}), "2026-09-10");
    assert_eq!(b["dsn"]["couleur"], "alerte");
    assert_eq!(b["dsn"]["attente"], false);
    assert_eq!(b["dsn"]["courte"], "annule-et-remplace à envoyer");
    assert_eq!(b["dsn"]["remplacement"]["saisie_modifiee"], true);
    assert_eq!(b["dsn"]["remplacement"]["dernier_ordre"], 3);
    assert_eq!(
        b["dsn"]["phrase"],
        "Paie modifiée depuis la DSN du 05/09/2026 à 10:00 — annule-et-remplace à envoyer, jusqu'au 14/09/2026 à minuit"
    );
    let b = rendu(c, "bilan-du-mois", json!({"mois": MOIS}), "2026-09-20");
    assert_eq!(b["dsn"]["courte"], "modifiée — à corriger le mois suivant");
    assert_eq!(b["dsn"]["remplacement"]["a_envoyer"], false);
    // Une DSN refusee n'ouvre rien.
    journal(c, &[dsn_reelle(MOIS, Some("KO"), json!({"type": "01", "ordre": "1"}))]);
    let b = rendu(c, "bilan-du-mois", json!({"mois": MOIS}), "2026-09-10");
    // (L'echeance de depot du mois, `echeance_depot`, se dit toujours : elle n'ouvre rien.)
    let mut dsn = b["dsn"].clone();
    assert!(dsn.as_object_mut().unwrap().remove("echeance_depot").is_some());
    assert_eq!(dsn, json!({"couleur": "refus", "phrase": "DSN REFUSÉE — B1-208-10"}));
}

fn modifier_saisie(c: &Path, f: impl FnOnce(&mut Value)) {
    let rel = format!("paies/{}.json", MOIS);
    let mut v = lire(c, &rel);
    f(&mut v);
    ecrire(c, &rel, &v);
}

#[test]
fn le_chemin_d_un_depot_se_lit_dans_le_bilan() {
    let c = &compagnie("etapes");
    let dsn = dossier_sorties(c, MOIS).join("DSN_Aout2026.dsn");
    std::fs::write(&dsn, "S10.G00.00.005,'02'\nS20.G00.05.001,'01'\nS20.G00.05.002,'01'\nS20.G00.05.004,'1'\n").unwrap();
    // Ecrite, c'est notee au journal des ecritures (`produire_mois::mois_ecrit`) : le moteur la note en l'ecrivant.
    std::fs::write(c.join("journal/dsn-ecrites.json"), json!({"2026-08/DSN_Aout2026.dsn": {"quand": "2026-10-01T09:00:00"}}).to_string()).unwrap();
    let empreinte = paie::journal::empreinte_fichier(&dsn).unwrap();
    let essai = |retours: Value, accepte: bool| {
        let mut l = ligne("dsn", MOIS, "test", retours, json!({"type": "01", "ordre": "1"}));
        l["fichier"] = json!("DSN_Aout2026.dsn");
        l["empreinte"] = json!(empreinte);
        l["accepte"] = json!(accepte);
        l
    };
    // Ecrite, jamais testee.
    let b = rendu(c, "bilan-du-mois", json!({"mois": MOIS}), "2026-10-03");
    assert_eq!(b["dsn"]["etape"], "ecrite");
    assert_eq!(b["dsn"]["courte"], "écrite — à tester");
    assert_eq!(b["dsn"]["phrase"], "DSN pas encore déposée. Le fichier est écrit : le dépôt de test vient d'abord.");
    assert_eq!(b["dsn"]["fichier"], dsn.to_str().unwrap());
    // Deposee en test, compte rendu attendu, puis conforme.
    journal(c, &[essai(json!([]), true)]);
    assert_eq!(rendu(c, "bilan-du-mois", json!({"mois": MOIS}), "2026-10-03")["dsn"]["etape"], "en_test");
    journal(c, &[essai(json!([{"nature": "11", "verdict": "OK", "anomalies": []}]), true)]);
    let b = rendu(c, "bilan-du-mois", json!({"mois": MOIS}), "2026-10-03");
    assert_eq!(b["dsn"]["etape"], "testee");
    assert_eq!(b["dsn"]["courte"], "testée — prête à envoyer");
    // Refusee en test : alerte.
    journal(c, &[essai(json!([]), false)]);
    let b = rendu(c, "bilan-du-mois", json!({"mois": MOIS}), "2026-10-03");
    assert_eq!(b["dsn"]["etape"], "test_refuse");
    assert_eq!(b["dsn"]["couleur"], "alerte");
    // La paie a change depuis l'ecriture du fichier : il faut le reecrire.
    journal(c, &[essai(json!([{"nature": "11", "verdict": "OK", "anomalies": []}]), true)]);
    std::fs::write(
        c.join("journal/dsn-ecrites.json"),
        json!({"2026-08/DSN_Aout2026.dsn": {"empreinte_saisie": "0000000000000000", "quand": "2026-10-01T09:00:00"}}).to_string(),
    )
    .unwrap();
    let b = rendu(c, "bilan-du-mois", json!({"mois": MOIS}), "2026-10-03");
    assert_eq!(b["dsn"]["etape"], "paie_changee");
    assert_eq!(b["dsn"]["couleur"], "alerte");
    assert_eq!(b["dsn"]["courte"], "la paie a changé — à réécrire et tester");
}

/// L'ORDRE DES FICHIERS DEPOSABLES (#236, `routes_mois::ordre_de_depot`) ne depend plus de la date d'un fichier : les
/// mensuelles d'abord, par leur nom ; puis les signalements, dans l'ordre de leur numero d'ordre (S20.G00.05.004, cahier
/// technique DSN 2026.1 p. 136 : unique, jamais remis a zero) ; les DPAE, la plus recente d'abord par son nom. `ecrit`
/// dit QUAND LE MOTEUR L'A ECRIT, tel que son journal des ecritures le consigne (`journal/dsn-ecrites.json`), et jamais
/// la date du fichier : une fictive decompressee chez le visiteur dirait le jour de l'installation (#236).
#[test]
fn les_fichiers_deposables_dans_l_ordre_de_ce_qu_ils_declarent() {
    let c = &compagnie("fichiers");
    let d = dossier_sorties(c, MOIS);
    let poser = |nom: &str, contenu: &str, secondes: u64| {
        std::fs::write(d.join(nom), contenu).unwrap();
        let f = std::fs::OpenOptions::new().write(true).open(d.join(nom)).unwrap();
        f.set_modified(UNIX_EPOCH + Duration::from_secs(secondes)).unwrap();
    };
    // 15/07/2026 10:30 UTC (ete : 12:30 a Paris), 15/01/2026 10:30 UTC (hiver : 11:30). Les dates ne decident plus rien.
    poser("DSN_Aout2026.dsn", "DSN_Aout2026.dsn", 1_784_111_400);
    poser("DSN_Aout2026_AR2.dsn", "DSN_Aout2026_AR2.dsn", 1_768_473_000);
    poser("autre.DSN", "autre.DSN", 1_784_111_460);
    poser("Amorcage01_X.dsn", "S20.G00.05.001,'08'\nS20.G00.05.004,'2'\n", 1_768_473_000);
    poser("Fin_Y.dsn", "S20.G00.05.001,'04'\nS20.G00.05.004,'1'\n", 1_784_111_460);
    poser("DPAE_Aout2026.txt", "x", 1_784_111_400);
    poser("DPAE_Aout2026_20260815-0900.txt", "x", 1_768_473_000);
    poser("DPAE_Aout2026_recapitulatif.txt", "x", 1_784_111_400);
    poser("DPAE_Aout2026.txt.zip", "x", 1_784_111_400);
    poser("Bulletins_Aout2026.pdf", "x", 1_784_111_400);
    ecrire(c, "journal/dsn-ecrites.json", &json!({format!("{}/DSN_Aout2026.dsn", MOIS): {"quand": "2026-09-01T10:15:42", "empreinte_saisie": null}}));
    let r = rendu(c, "fichiers-deposables", json!({"mois": MOIS}), "2026-10-03");
    let noms = |k: &str| r[k].as_array().unwrap().iter().map(|f| f["nom"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(noms("dsn"), ["DSN_Aout2026.dsn", "DSN_Aout2026_AR2.dsn", "autre.DSN", "Fin_Y.dsn", "Amorcage01_X.dsn"]);
    assert_eq!(noms("dpae"), ["DPAE_Aout2026_20260815-0900.txt", "DPAE_Aout2026.txt"]);
    assert_eq!(r["dsn"][0]["ecrit"], "2026-09-01T10:15", "la date consignee, pas celle du fichier");
    assert_eq!(r["dsn"][1]["ecrit"], Value::Null, "rien de consigne : rien a dire, meme si le fichier a une date");
    assert_eq!(r["dsn"][1]["octets"], "DSN_Aout2026_AR2.dsn".len());
    assert_eq!(r["dsn"][1]["chemin"], d.join("DSN_Aout2026_AR2.dsn").to_str().unwrap());
    // Un mois sans dossier de sorties : rien.
    assert_eq!(rendu(c, "fichiers-deposables", json!({"mois": "2031-01"}), "2026-10-03"), json!({"dsn": [], "dpae": []}));
}

#[test]
fn les_embauches_a_declarer_sont_celles_sans_trace_d_envoi() {
    let c = &compagnie("embauches");
    mois_vide(c, "2026-07");
    journal(c, &[dpae_declaree(&["BRIOCHE"], json!([]))]);
    let r = rendu(c, "embauches-a-declarer", json!({"mois": MOIS}), "2026-10-03");
    let e = r["embauches"].as_array().unwrap();
    assert_eq!(e.len(), 2);
    assert_eq!(
        e[0],
        json!({"mois": MOIS, "salarie": "SERAPHIN", "nom": "SERAPHIN Valentin", "spectacle": "Ubu roi", "debut": "2026-08-19", "fin": "2026-08-22"})
    );
    assert_eq!(e[1]["salarie"], "LEMERCIER");
    // Sans mois : tous les mois saisis, le plus recent d'abord ; le calcul stocke n'est pas un mois.
    std::fs::write(c.join("paies/2026-08.calcul.json"), "{}").unwrap();
    assert_eq!(rendu(c, "embauches-a-declarer", json!({}), "2026-10-03"), r);
}

#[test]
fn le_mois_a_ouvrir_est_le_plus_ancien_mois_passe_dont_une_etape_n_est_pas_verte() {
    let c = &compagnie("mois-a-ouvrir");
    std::fs::remove_file(saisie(c, MOIS)).unwrap();
    mois_vide(c, "2026-08");
    mois_vide(c, "2026-09");
    let vert = |m: &str| dsn_reelle(m, Some("OK"), json!({"type": "01", "ordre": "1"}));
    // Tout vert : le mois du calendrier.
    journal(c, &[vert("2026-08"), vert("2026-09")]);
    assert_eq!(rendu(c, "mois-a-ouvrir", json!({}), "2026-10-03"), json!({"mois": "2026-10", "courant": "2026-10", "reste": []}));
    // Une DSN sans retour n'est pas verte.
    journal(c, &[vert("2026-08"), dsn_reelle("2026-09", None, json!({}))]);
    assert_eq!(rendu(c, "mois-a-ouvrir", json!({}), "2026-10-03"), json!({"mois": "2026-09", "courant": "2026-10", "reste": ["dsn"]}));
    // Le plus ancien d'abord.
    journal(c, &[dsn_reelle("2026-08", Some("KO"), json!({}))]);
    assert_eq!(rendu(c, "mois-a-ouvrir", json!({}), "2026-10-03")["mois"], "2026-08");
    // Une saisie en avance ne fait pas ouvrir en avance.
    mois_vide(c, "2026-11");
    journal(c, &[vert("2026-08"), vert("2026-09")]);
    assert_eq!(rendu(c, "mois-a-ouvrir", json!({}), "2026-10-03")["mois"], "2026-10");
    // Un exercice clos ne rappelle plus rien.
    journal(c, &[vert("2026-09")]);
    assert_eq!(rendu(c, "mois-a-ouvrir", json!({}), "2026-10-03")["mois"], "2026-08");
    std::fs::write(c.join("config/exercices.json"), json!({"2026": {"le": "2026-09-30T10:00:00"}}).to_string()).unwrap();
    assert_eq!(rendu(c, "mois-a-ouvrir", json!({}), "2026-10-03")["mois"], "2026-10");
}

#[test]
fn l_etat_du_mois_rassemble_les_actes_les_resumes_et_le_bilan() {
    let c = &compagnie("etat-du-mois");
    journal(c, &[
        dpae_declaree(&CLES, json!([{"nature": "41", "dpae": []}])),
        ligne("dpae", MOIS, "test", json!([]), json!({"embauches": []})),
    ]);
    let r = rendu(c, "etat-du-mois", json!({"mois": MOIS}), "2026-10-03");
    assert_eq!(r["mois"], MOIS);
    assert_eq!(r["actes"]["dpae"]["mode"], "reel");
    assert_eq!(r["actes"]["dsn"], Value::Null);
    assert_eq!(r["resumes"]["dsn"], "pas encore envoyé");
    assert!(r["resumes"]["dpae"].as_str().unwrap().contains("05/09/2026"));
    // Le fil des envois n'est plus ici : il se lit dans l'Historique (`historique-mois`).
    assert!(r.get("derniers").is_none());
    assert_eq!(r["fichiers"], json!({"dsn": [], "dpae": []}));
    assert_eq!(r["bilans"]["dpae"]["couleur"], "bien");
    assert_eq!(r["lignes"].as_array().unwrap().len(), 2);
    // Pas de coffre dans cette compagnie : aucun mot de passe du logiciel pose.
    assert_eq!(r["mot_de_passe_pose"], false);
}

#[test]
fn les_numeros_de_contrat_manquants_sont_proposes_en_memoire_sans_toucher_aux_compteurs() {
    let c = &compagnie("numeros");
    let compteurs = std::fs::read(c.join("config/compteurs.json")).unwrap();
    modifier_saisie(c, |v| {
        for i in 0..2 {
            v["engagements"][i].as_object_mut().unwrap().remove("numero");
        }
        // Un deuxieme contrat de Brioche, sans numero non plus.
        let mut copie = v["engagements"][0].clone();
        copie["debut"] = json!("2026-08-25");
        copie["fin"] = json!("2026-08-25");
        copie["lignes"][0]["dates"] = json!(["2026-08-25"]);
        copie["lignes"][0]["quantite"] = json!(1);
        copie["lignes"][1]["quantite"] = json!(1);
        v["engagements"].as_array_mut().unwrap().push(copie);
    });
    let r = rendu(c, "etat-par-paie", json!({"mois": MOIS}), "2026-10-03");
    let numeros: Vec<&str> = r["paies"].as_array().unwrap().iter().map(|p| p["numero"].as_str().unwrap()).collect();
    // Les compteurs de 2026 : BRIOCHE a 4, SERAPHIN a 4.
    assert_eq!(numeros, ["BR1.26.05", "SE2.26.05", "LE3.26.04", "BR1.26.06"]);
    assert_eq!(std::fs::read(c.join("config/compteurs.json")).unwrap(), compteurs);
}

#[test]
fn une_saisie_en_cours_garde_son_nom_et_un_journal_abime_refuse() {
    let c = &compagnie("refus");
    assert_eq!(route(c, "etat-par-paie", json!({}), "2026-10-03")["ok"], json!(false), "sans mois");
    let r = route(c, "n-importe-quoi", json!({}), "2026-10-03");
    assert_eq!(r["ok"], false);
    assert_eq!(r["erreur"], "route inconnue : n-importe-quoi");
    assert!(r.get("classe").is_none());
    // Un spectacle pas encore choisi est une saisie en cours, pas un defaut : la page la reconnait a son nom.
    modifier_saisie(c, |v| {
        v["engagements"][0].as_object_mut().unwrap().remove("spectacle");
    });
    let r = route(c, "etat-par-paie", json!({"mois": MOIS}), "2026-10-03");
    assert_eq!(r["classe"], "SaisieIncomplete");
    assert!(r["erreur"].as_str().unwrap().starts_with("L'engagement 1 de 2026-08 ne dit pas quel spectacle il joue"), "{}", r);
    // Le bilan et les embauches voient la meme saisie : l'un refuse, l'autre passe son mois.
    assert_eq!(route(c, "bilan-du-mois", json!({"mois": MOIS}), "2026-10-03")["classe"], "SaisieIncomplete");
    assert_eq!(rendu(c, "embauches-a-declarer", json!({"mois": MOIS}), "2026-10-03"), json!({"embauches": []}));
    // Une ligne qui doit dire ses heures et les tait (#125) : une saisie en cours elle aussi, que la route ne calcule pas.
    let c = &compagnie("sans-heures");
    modifier_saisie(c, |v| {
        v["engagements"][0]["lignes"] = json!([{"rubrique": "repetition", "quantite": 4, "taux": 120.0,
                                                 "dates": ["2026-08-19", "2026-08-20", "2026-08-21", "2026-08-22"]}]);
    });
    let avant = std::fs::read(saisie(c, MOIS)).unwrap();
    for nom in ["etat-par-paie", "bilan-du-mois"] {
        let r = route(c, nom, json!({"mois": MOIS}), "2026-10-03");
        assert_eq!(r["classe"], "SaisieIncomplete", "{}", r);
        assert!(r["erreur"].as_str().unwrap().starts_with("L'engagement 1 de 2026-08 : sa ligne 1 "), "{}", r);
    }
    assert_eq!(std::fs::read(saisie(c, MOIS)).unwrap(), avant);
    let c = &compagnie("journal-abime");
    std::fs::write(c.join("journal/depots.json"), "[1, 2]").unwrap();
    assert_eq!(route(c, "etat-du-mois", json!({"mois": MOIS}), "2026-10-03")["ok"], false);
}

/// Un CDI n'a pas de fin : les routes du mois la rendent `null`, comme `Engagement::canon`, et non le texte « None »
/// (reste R1 #6). La copie figee `fictive-3090-grande` : Montansier et Nicolet sont en CDI en octobre 2026 ; Nicolet y
/// est embauchee le 1er octobre, pour que sa DPAE soit a faire.
#[test]
fn un_cdi_sans_fin_se_rend_null_et_pas_none() {
    let c = &compagnie::saisies("fictive-3090-grande", &[], &dossier_de_test("routes_mois", "cdi-sans-fin").join("compagnie"));
    let mut saisie = lire(&crate::outils::fictive("fictive-3090-grande"), "paies/2026-10.json");
    for e in saisie["engagements"].as_array_mut().unwrap() {
        if e["numero"] == json!("NI.24.01") {
            e["debut"] = json!("2026-10-01");
        }
    }
    ecrire(c, "paies/2026-10.json", &saisie);
    let paies = rendu(c, "etat-par-paie", json!({"mois": "2026-10"}), "2026-10-03")["paies"].as_array().unwrap().clone();
    let cdi: Vec<&Value> = paies.iter().filter(|p| p["numero"] == json!("MO.23.01") || p["numero"] == json!("NI.24.01")).collect();
    assert_eq!(cdi.len(), 2, "{:?}", paies);
    for p in cdi {
        assert_eq!(p["fin"], Value::Null, "etat-par-paie : {}", p);
    }
    let e = rendu(c, "embauches-a-declarer", json!({"mois": "2026-10"}), "2026-10-03")["embauches"].as_array().unwrap().clone();
    let nicolet = e.iter().find(|x| x["debut"] == json!("2026-10-01")).unwrap_or_else(|| panic!("{:?}", e));
    assert_eq!(nicolet["fin"], Value::Null, "embauches-a-declarer : {}", nicolet);
}

/// `config/exercices.json` se lit une fois tant qu'il ne change pas (ticket #244 : `etat` le relisait pour chaque mois),
/// dans un meme processus, comme dans la fenetre. Un fichier illisible refuse comme avant, a chaque fois (un refus ne se
/// garde pas) ; un fichier change se relit.
#[test]
fn les_exercices_se_gardent_et_un_fichier_illisible_refuse_toujours() {
    let c = &compagnie("exercices-en-cache");
    let p = c.join("config/exercices.json");
    let jour = "2026-10-03";
    let mois = |c: &Path| reussi(&route_en_processus(c, "mois-a-ouvrir", json!({}), jour))["mois"].clone();
    std::fs::write(&p, "{}").unwrap();
    assert_eq!(mois(c), MOIS, "rien de clos : le mois d'aout reste a ouvrir");
    assert_eq!(mois(c), MOIS, "servi du cache, meme reponse");
    // Illisible comme JSON : le refus d'avant, deux fois de suite.
    std::fs::write(&p, "{\"2026\": ").unwrap();
    for _ in 0..2 {
        assert_eq!(refus(&route_en_processus(c, "mois-a-ouvrir", json!({}), jour)).0, "JSONDecodeError");
    }
    // Illisible comme texte : le refus du disque.
    std::fs::write(&p, b"{\"2026\": \"\xff\xfe\"}").unwrap();
    assert_eq!(refus(&route_en_processus(c, "mois-a-ouvrir", json!({}), jour)).0, "OSError");
    // Pas un objet.
    std::fs::write(&p, "[1, 2]").unwrap();
    assert_eq!(refus(&route_en_processus(c, "mois-a-ouvrir", json!({}), jour)).0, "AttributeError");
    // Clos : relu, aout ne se rappelle plus ; rouvert : relu encore.
    std::fs::write(&p, json!({"2026": {"le": "2026-09-30T10:00:00"}}).to_string()).unwrap();
    assert_ne!(mois(c), MOIS, "l'exercice 2026 clos, relu malgre le cache");
    std::fs::write(&p, json!({"_rouverts": [], "2026": false}).to_string()).unwrap();
    assert_eq!(mois(c), MOIS, "rouvert, relu encore");
}
