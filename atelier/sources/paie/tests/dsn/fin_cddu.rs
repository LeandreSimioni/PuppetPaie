//! La fin d'un CDD d'usage du spectacle (lot C1, 14/10/2026) : arrive a son terme (031), elle part dans la DSN mensuelle
//! par le circuit derogatoire (S21.G00.62.017 = 01) et ne se signale pas ; rompu avant son terme (036, 037, 084…), il
//! ecrit 62.017 = 02 et sa fin se signale (nature 07). Textes : fiche net-entreprises 2062, critere d) et « Tous les
//! autres motifs de rupture doivent etre declares par l'emission d'un signalement FCTU »
//! (`atelier/reference/sources/net-entreprises-base-connaissances/net-entreprises-fiche2062-derogations-fin-de-contrat-fctu-20250905.txt`) ;
//! CCH-12 de S21.G00.62.017, « 01 » autorise « uniquement si […] le motif de la rupture […] est renseigne a 031 […] ou
//! 099 » (cahier technique DSN 2026.1, p. 277, `atelier/reference/norme-dsn/texte/bloc-S21.G00.62.txt`) ; CCH-11 de
//! S21.G00.62.002, les motifs de la nature 02 (p. 270-273). Sur `fictive-3090-grande`, mai 2026 : les CDDU BI4.26.05
//! (Biancolelli) et CL6.26.06 (Clairon), du 12 au 22 mai.

use crate::outils::{self, compagnie, refus, reussi, route_en_processus};
use paie::contexte::Contexte;
use paie::date::Jour;
use paie::engagement::charger;
use paie::fctu;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;

const DEMO: &str = "fictive-3090-grande";
const ROMPU: &str = "BI4.26.05";
const A_TERME: &str = "CL6.26.06";

/// La copie du cas, ou le CDDU de Biancolelli est rompu le 22 mai pour le motif `motif` (rien : il va a son terme).
fn compagnie_du_cas(test: &str, motif: Option<&str>) -> PathBuf {
    let racine = compagnie::ses_dsn(DEMO, &[], &outils::dossier_de_test("fin_cddu", test));
    let mut saisie = outils::lire(&racine, "paies/2026-05.json");
    let e = saisie["engagements"].as_array_mut().unwrap().iter_mut().find(|e| e["numero"] == ROMPU).unwrap();
    if let Some(m) = motif {
        e["motif_rupture"] = json!(m);
    }
    outils::ecrire(&racine, "paies/2026-05.json", &saisie);
    racine
}

/// Pose `champs` sur le CDDU rompu de la saisie de mai.
fn poser(racine: &PathBuf, champs: Value) {
    let mut saisie = outils::lire(racine, "paies/2026-05.json");
    let e = saisie["engagements"].as_array_mut().unwrap().iter_mut().find(|e| e["numero"] == ROMPU).unwrap();
    for (k, v) in champs.as_object().unwrap() {
        e[k] = v.clone();
    }
    outils::ecrire(racine, "paies/2026-05.json", &saisie);
}

fn contexte(racine: &PathBuf) -> Contexte {
    Contexte::nouveau(racine, Some(Jour::lire("2026-10-20").unwrap())).unwrap()
}

/// La mensuelle de mai 2026, ou son refus.
fn mensuelle(ctx: &Contexte) -> Result<String, String> {
    let s = charger(ctx, &ctx.racine.join("paies/2026-05.json")).unwrap();
    let o = paie::dsn::Options { date_constitution: Some(Jour::lire("2026-10-02").unwrap()), ..Default::default() };
    paie::dsn::generer(ctx, &s.paies, "2026-05", &[], &[], &o)
}

/// Les rubriques du bloc 62 de chaque contrat (S21.G00.40.009) : contrat -> rubrique -> valeur.
fn blocs_62(texte: &str) -> BTreeMap<String, BTreeMap<String, String>> {
    let (mut out, mut contrat) = (BTreeMap::new(), String::new());
    for l in texte.lines() {
        let (rubrique, valeur) = l.trim().split_once(',').unwrap();
        let valeur = valeur.trim_matches('\'').to_string();
        if rubrique == "S21.G00.40.009" {
            contrat = valeur;
        } else if rubrique.starts_with("S21.G00.62.") {
            out.entry(contrat.clone()).or_insert_with(BTreeMap::new).insert(rubrique.to_string(), valeur);
        }
    }
    out
}

/// CCH-12 de 62.017 : le circuit derogatoire (01) n'est permis qu'au 031. La mensuelle ecrivait 01 pour tout CDDU,
/// meme rompu avant son terme : le controle de la plate-forme l'aurait refuse.
#[test]
fn la_mensuelle_ecrit_02_au_cddu_rompu_avant_son_terme_et_01_a_celui_qui_va_a_son_terme() {
    let ctx = contexte(&compagnie_du_cas("62-017", Some("037")));
    let blocs = blocs_62(&mensuelle(&ctx).unwrap());
    let bloc = |n: &str| blocs.get(n).unwrap_or_else(|| panic!("{} n'a pas de bloc 62 : {:?}", n, blocs)).clone();
    let (rompu, a_terme) = (bloc(ROMPU), bloc(A_TERME));
    assert_eq!((rompu["S21.G00.62.002"].as_str(), rompu["S21.G00.62.017"].as_str()), ("037", "02"), "{:?}", rompu);
    assert_eq!((a_terme["S21.G00.62.002"].as_str(), a_terme["S21.G00.62.017"].as_str()), ("031", "01"), "{:?}", a_terme);
}

/// CCH-11 de 62.002 : un CDDU est un contrat de nature 02, il n'a que les motifs du CDD. La rupture conventionnelle (043)
/// n'en est pas ; le moteur l'ecrivait sans controle.
#[test]
fn un_cddu_ne_prend_pas_un_motif_que_la_nature_02_n_admet_pas() {
    let ctx = contexte(&compagnie_du_cas("043", Some("043")));
    match mensuelle(&ctx) {
        Err(m) => assert!(m.contains("043") && m.contains("CCH-11"), "{}", m),
        Ok(t) => panic!("la mensuelle a ecrit {:?}", blocs_62(&t).get(ROMPU)),
    }
}

/// Fiche 2062 : la fin du CDDU rompu se signale, celle du CDDU arrive a son terme non. Les deux ecrans lisent la meme
/// liste (`fins-de-contrat` pour la ligne de commande et le bloc de la page, `rappels` pour l'accueil).
#[test]
fn la_fin_du_cddu_rompu_se_signale_et_celle_du_cddu_a_son_terme_non() {
    let racine = compagnie_du_cas("liste", Some("036"));
    let r = reussi(&route_en_processus(&racine, "fins-de-contrat", json!({"mois": "2026-05"}), "2026-05-26")).clone();
    let numeros: Vec<&str> = r["fins"].as_array().unwrap().iter().map(|f| f["numero"].as_str().unwrap()).collect();
    assert_eq!(numeros, [ROMPU], "{}", r);
    let rappels = reussi(&route_en_processus(&racine, "rappels", json!({"aujourdhui": "2026-05-26"}), "2026-05-26")).clone();
    let fins: Vec<&Value> = rappels["rappels"].as_array().unwrap().iter().filter(|x| x["cle"].as_str().is_some_and(|c| c.starts_with("fin-contrat:"))).collect();
    assert_eq!(fins.len(), 1, "{}", rappels);
    assert_eq!(fins[0]["cle"], json!("fin-contrat:Biancolelli:2026-05-22"));

    // Sans motif dit, le CDDU va a son terme (031) : rien a signaler, le cas nominal reste.
    let racine = compagnie_du_cas("liste-nominale", None);
    let r = reussi(&route_en_processus(&racine, "fins-de-contrat", json!({"mois": "2026-05"}), "2026-05-26")).clone();
    assert_eq!(r["fins"], json!([]), "{}", r);
}

/// Le signalement du CDDU rompu s'ecrit (les usages et les formats de la nature 07 le controlent dans `generer`) sans
/// l'avis « n'a pas besoin de signalement », qui reste pour un 031 (le salarie peut le demander : fiche 1902).
#[test]
fn le_signalement_du_cddu_rompu_s_ecrit_sans_l_avis_de_la_derogation() {
    let ctx = contexte(&compagnie_du_cas("fctu", Some("037")));
    let s = charger(&ctx, &ctx.racine.join("paies/2026-05.json")).unwrap();
    let fin = |motif: &str| {
        fctu::FinDeContrat::lire(&json!({
            "fin": "2026-05-22", "motif": motif, "mois_rattachement": "2026-05", "type_preavis": "90", "notification": "2026-05-20",
        }))
        .unwrap()
    };
    let paie = |n: &str| s.paies.iter().find(|p| p.engagement.numero.as_deref() == Some(n)).unwrap();
    let derogation = |avis: &[String]| avis.iter().any(|a| a.contains("n'a pas besoin de signalement"));

    let avis = fctu::verifier(&fin("037"), &paie(ROMPU).engagement, fctu::NORMALE).unwrap();
    assert!(!derogation(&avis), "{:?}", avis);
    let op = fctu::Options { envoi_test: true, date_constitution: Some(Jour::lire("2026-05-26").unwrap()), ..Default::default() };
    let texte = fctu::generer(&ctx, paie(ROMPU), &fin("037"), &op, &[], &[]).unwrap_or_else(|e| panic!("{}", e.message()));
    assert!(texte.contains("S21.G00.62.002,'037'"), "{}", texte);
    // 62.017 est interdite en nature 07 (tableau des usages) : le signalement ne l'ecrit pas.
    assert!(!texte.contains("S21.G00.62.017"), "{}", texte);

    let mut sans = fin("031");
    sans.notification = None;
    let avis = fctu::verifier(&sans, &paie(A_TERME).engagement, fctu::NORMALE).unwrap();
    assert!(derogation(&avis), "{:?}", avis);
}

/// Fiche 1722 (`atelier/reference/sources/net-entreprises-base-connaissances/net-entreprises-fiche1722-annulation-fin-de-contrat-transmise-a-tort-20250326.txt`) :
/// une fin declaree a tort s'annule par un bloc 62 de motif 099 dans la DSN mensuelle DU MOIS SUIVANT, « identique au
/// bloc " Fin de contrat " a annuler », et le 099 est interdit en nature 07 (CCH-13 de 62.002). Le moteur ne sait pas
/// ecrire ce bloc (il n'ecrit le 62 que le dernier mois du contrat) : il refuse un 099 saisi, plutot que de le poser au
/// mois de la fin ; le FCTU le refuse aussi. Ce qu'il sait : le FCTU « 04 - annule » (`fctu::ANNULE`).
#[test]
fn un_099_saisi_ne_s_ecrit_ni_dans_la_mensuelle_ni_dans_le_signalement() {
    let ctx = contexte(&compagnie_du_cas("099", Some("099")));
    match mensuelle(&ctx) {
        Err(m) => assert!(m.contains("099") && m.contains("CCH-11"), "{}", m),
        Ok(t) => panic!("la mensuelle a ecrit {:?}", blocs_62(&t).get(ROMPU)),
    }
    let s = charger(&ctx, &ctx.racine.join("paies/2026-05.json")).unwrap();
    let paie = s.paies.iter().find(|p| p.engagement.numero.as_deref() == Some(ROMPU)).unwrap();
    let fin = fctu::FinDeContrat::lire(&json!({"fin": "2026-05-22", "motif": "099", "mois_rattachement": "2026-05", "type_preavis": "90"})).unwrap();
    match fctu::verifier(&fin, &paie.engagement, fctu::NORMALE) {
        Err(fctu::Echec::Impossible(m)) => assert!(m.contains("099"), "{}", m),
        r => panic!("le signalement : {:?}", r),
    }
}

/// La saisie ecrit le nom comme on le dit (`Biancolelli`), la fiche a sa cle (`BIANCOLELLI`, `config/salaries.json`). Le
/// signalement et les papiers de fin retrouvent le salarie comme le calcul de sa paie (`Contexte::salarie`), quelle que
/// soit la casse de la saisie ou de la commande ; ils partaient en echec, sans un mot (14/10/2026). Les fichiers portent
/// la cle de la fiche : c'est elle que le dossier du salarie cherche (`CertificatTravail_<cle>_`).
#[test]
fn le_signalement_retrouve_le_salarie_quelle_que_soit_la_casse_du_nom() {
    let racine = compagnie_du_cas("casse", Some("036"));
    poser(&racine, json!({"preavis": {"type": "90"}, "notification_rupture": "2026-05-20"}));
    for nom in ["Biancolelli", "BIANCOLELLI", "biancolelli"] {
        let r = route_en_processus(&racine, "fin-contrat-signaler", json!({"mois": "2026-05", "salarie": nom, "test": true}), "2026-05-26");
        assert_eq!(reussi(&r)["nom"], json!("SignalementFinContrat_BIANCOLELLI_20260522.dsn"), "{}", r);
        let texte = String::from_utf8_lossy(&std::fs::read(racine.join("sorties/2026-05/SignalementFinContrat_BIANCOLELLI_20260522.dsn")).unwrap()).into_owned();
        assert!(texte.contains("S21.G00.40.009,'BI4.26.05'") && texte.contains("S21.G00.62.002,'036'"), "{}", texte);
    }
    let r = route_en_processus(&racine, "fin-contrat-documents", json!({"mois": "2026-05", "salarie": "biancolelli"}), "2026-05-26");
    assert_eq!(reussi(&r)["noms"], json!(["CertificatTravail_BIANCOLELLI_20260522.pdf", "RecuSoldeToutCompte_BIANCOLELLI_20260522.pdf"]), "{}", r);
}

/// Aucune erreur de la chaine du signalement ne part sans un mot : un salarie sans paie ce mois-la le dit, avec son nom et
/// le mois ; un salarie paye ce mois-la dont aucun contrat ne s'y termine le dit aussi, autrement.
#[test]
fn un_salarie_sans_paie_ce_mois_la_se_dit_avec_son_nom_et_le_mois() {
    let racine = compagnie_du_cas("sans-paie", Some("036"));
    for route in ["fin-contrat-signaler", "fin-contrat-documents"] {
        let (_, m) = refus(&route_en_processus(&racine, route, json!({"mois": "2026-05", "salarie": "Deburau"}), "2026-05-26"));
        assert!(m.contains("Deburau") && m.contains("aucune paie") && m.contains("mai 2026"), "{} : {}", route, m);
        let (_, m) = refus(&route_en_processus(&racine, route, json!({"mois": "2026-05", "salarie": "Montansier"}), "2026-05-26"));
        assert!(m.contains("Montansier") && m.contains("aucun contrat qui se termine") && m.contains("mai 2026"), "{} : {}", route, m);
    }
}

/// CCH-12 de S21.G00.62.003 (cahier technique DSN 2026.1, p. 275, `atelier/reference/norme-dsn/texte/bloc-S21.G00.62.txt` ;
/// la meme liste en 2027.1.1, `texte-2027/`) : la date de notification de la rupture est obligatoire pour les motifs
/// 011, 012, 014, 015, 020, 025, 034, 035, 036, 037, 058, 059, 082, 083, 087, 088, 089, 095, 096, 097, 111, 112, 113, 115
/// et 117. Un FCTU de 036 sans elle a ete refuse par le point de depot (certificat de conformite KO, 14/10/2026) ; la
/// liste le disait « pret a signaler ». Elle le demande maintenant, le signalement ne s'ecrit pas sans elle, et un motif
/// hors de la liste (084, rupture d'un commun accord) n'en a pas besoin.
#[test]
fn la_rupture_qui_se_notifie_ne_se_signale_pas_sans_sa_date_de_notification() {
    let racine = compagnie_du_cas("notification", Some("036"));
    poser(&racine, json!({"preavis": {"type": "90"}}));
    let fins = reussi(&route_en_processus(&racine, "fins-de-contrat", json!({"mois": "2026-05"}), "2026-05-26")).clone();
    assert!(fins["fins"][0]["a_saisir"].to_string().contains("notification_rupture"), "{}", fins);
    let rappels = reussi(&route_en_processus(&racine, "rappels", json!({"aujourdhui": "2026-05-26"}), "2026-05-26")).clone();
    let rappel = rappels["rappels"].as_array().unwrap().iter().find(|x| x["cle"] == json!("fin-contrat:Biancolelli:2026-05-22")).unwrap();
    assert!(rappel["a_saisir"].to_string().contains("notification_rupture"), "{}", rappel);
    let signaler = json!({"mois": "2026-05", "salarie": "Biancolelli", "test": true});
    let (_, m) = refus(&route_en_processus(&racine, "fin-contrat-signaler", signaler.clone(), "2026-05-26"));
    assert!(m.contains("notification_rupture") && m.contains("S21.G00.62.003"), "{}", m);
    let fichier = racine.join("sorties/2026-05/SignalementFinContrat_BIANCOLELLI_20260522.dsn");
    assert!(!fichier.exists());

    // Le format lui-meme le refuse, quel que soit le chemin qui l'appelle.
    let ctx = contexte(&racine);
    let s = charger(&ctx, &ctx.racine.join("paies/2026-05.json")).unwrap();
    let paie = s.paies.iter().find(|p| p.engagement.numero.as_deref() == Some(ROMPU)).unwrap();
    let fin = |motif: &str| {
        fctu::FinDeContrat::lire(&json!({"fin": "2026-05-22", "motif": motif, "mois_rattachement": "2026-05", "type_preavis": "90"})).unwrap()
    };
    match fctu::verifier(&fin("036"), &paie.engagement, fctu::NORMALE) {
        Err(fctu::Echec::Impossible(m)) => assert!(m.contains("S21.G00.62.003"), "{}", m),
        r => panic!("le signalement : {:?}", r),
    }
    assert!(fctu::verifier(&fin("084"), &paie.engagement, fctu::NORMALE).is_ok());

    poser(&racine, json!({"notification_rupture": "2026-05-20"}));
    let fins = reussi(&route_en_processus(&racine, "fins-de-contrat", json!({"mois": "2026-05"}), "2026-05-26")).clone();
    assert_eq!(fins["fins"][0]["a_saisir"], json!([]), "{}", fins);
    reussi(&route_en_processus(&racine, "fin-contrat-signaler", signaler, "2026-05-26"));
    let texte = String::from_utf8_lossy(&std::fs::read(&fichier).unwrap()).into_owned();
    assert!(texte.contains("S21.G00.62.003,'20052026'"), "{}", texte);
}
