//! L'AER D'UNE COMPAGNIE FICTIVE (tickets #225-#227, repris par #236). Une DSN mensuelle jouee en reel dans une fictive
//! livree recoit, comme chez une vraie compagnie, l'attestation employeur Cinema Spectacle de France Travail (retour de
//! nature 46, une archive dans `retours/`), fabriquee depuis la DSN (`aer_fictive`), rangee et signee au cadre 5 par le
//! meme chemin qu'une vraie (`routes_documents::ranger_aer`), et elle part avec les documents de l'intermittent. Elle
//! n'est due qu'a l'intermittent dont le contrat finit dans le mois (`aer::due`, d'apres
//! `atelier/reference/sources/francetravail-100-dsn/movinmotion-fin-des-aem-20260601.txt`, l. 152-166, et
//! `mescachets-aem-aer-cs-20260903.txt`, l. 47 et 69) : jamais a un permanent, qui n'a ni l'AER ni la phrase qui
//! l'annonce. Rien de tout cela dans une fictive ajoutee ni dans une compagnie reelle.
//!
//! Joue sur `fictive-1285-grande` (copie figee) : septembre 2026, deux permanents en CDI (NADAR, DORVAL) et quatre
//! intermittents dont le contrat finit le 30/09 (DUMESNIL, NOVERRE, TORELLI, TURLUPIN).

use std::path::{Path, PathBuf};

use droit::contexte::Contexte;
use socle::date::Jour;
use serde_json::{json, Value};

use crate::outils::{compagnie, dossier_de_test, ecrire, lire, noms};

const JOUR: &str = "2026-10-23";
const FICTIVE: &str = "fictive-1285-grande";
const MOIS: &str = "2026-09";
const INTERMITTENTS: [&str; 4] = ["DUMESNIL", "NOVERRE", "TORELLI", "TURLUPIN"];
const PERMANENTS: [&str; 2] = ["DORVAL", "NADAR"];

/// La fictive, sa DSN de septembre deposee en reel et acceptee, ses retours pas encore la (le journal d'une DSN jouee).
fn copie(test: &str) -> PathBuf {
    let r = compagnie::entiere(FICTIVE, &dossier_de_test("aer_fictive", test));
    let mut j = lire(&r, "journal/depots.json");
    let l = j.as_array_mut().unwrap();
    let mut d = l.iter().rev().find(|x| x["quoi"] == json!("dsn") && x["mode"] == json!("reel")).unwrap().clone();
    d["mois"] = json!(MOIS);
    d["fichier"] = json!("DSN_Septembre2026.dsn");
    d["quand"] = json!("2026-10-05T10:00:00");
    d["flux"] = json!("FICTIF-0000AER09");
    d.as_object_mut().unwrap().remove("retours");
    l.push(d);
    ecrire(&r, "journal/depots.json", &j);
    r
}

fn contexte(r: &Path) -> Contexte {
    Contexte::nouveau(r, Some(Jour::lire(JOUR).unwrap())).unwrap()
}

fn envoi(r: &Path) -> PathBuf {
    r.join("sorties").join(MOIS).join("envoi")
}

fn aer_de(r: &Path, cle: &str) -> PathBuf {
    envoi(r).join(format!("AER_{}_Septembre2026.pdf", cle))
}

fn archives_46(r: &Path) -> Vec<String> {
    noms(&r.join("retours")).into_iter().filter(|n| n.contains("_nature46_")).collect()
}

fn ligne_du_mois(r: &Path) -> Value {
    lire(r, "journal/depots.json").as_array().unwrap().iter().rev().find(|x| x["flux"] == json!("FICTIF-0000AER09")).unwrap().clone()
}

/// Les paies du mois, calculees.
fn paies(r: &Path) -> Vec<engagement::calcul::Paie> {
    engagement::charger(&contexte(r), MOIS).unwrap().paies
}

fn cle(p: &engagement::calcul::Paie) -> String {
    p.salarie["_cle"].as_str().unwrap().to_string()
}

#[test]
fn une_aer_cs_n_est_due_qu_a_l_intermittent_dont_le_contrat_finit_dans_le_mois() {
    let r = compagnie::entiere(FICTIVE, &dossier_de_test("aer_fictive", "due"));
    let mut dues: Vec<String> = Vec::new();
    for p in paies(&r) {
        if paie::aer::due(&p.engagement).unwrap() {
            dues.push(cle(&p));
        }
    }
    dues.sort();
    assert_eq!(dues, INTERMITTENTS, "le CDI (NADAR, DORVAL) n'a pas d'AER Cinema Spectacle");
    // Un contrat d'usage qui continue le mois suivant : son AER vient « à la fin du dernier mois concerné ».
    let mut p = paies(&r).into_iter().find(|p| cle(p) == "TURLUPIN").unwrap();
    p.engagement.fin = Some(Jour::lire("2026-10-02").unwrap());
    assert!(!paie::aer::due(&p.engagement).unwrap());
}

#[test]
fn la_dsn_jouee_d_une_fictive_livree_recoit_son_aer_signee_dans_l_envoi_de_l_intermittent() {
    let r = copie("livree");
    assert!(paie::routes_accueil::simulation_active(&r));
    let (n, a_faire) = paie::releve_retours::rattacher_simulation_detail(&contexte(&r), 6).unwrap();
    assert_eq!((n, a_faire), (1, Vec::<String>::new()));

    // Le retour 46, et sa piece dans `retours/` (une archive, comme celle de France Travail).
    let natures: Vec<Value> = ligne_du_mois(&r)["retours"].as_array().unwrap().iter().map(|x| x["nature"].clone()).collect();
    assert_eq!(natures, [json!("10"), json!("11"), json!("46")]);
    let archives = archives_46(&r);
    assert_eq!(archives.len(), 1, "{:?}", archives);
    let archive = r.join("retours").join(&archives[0]);
    let lues = paie::retours::commande(&json!({"operation": "aer", "fichier": archive.to_string_lossy()}).to_string()).unwrap();
    let periodes: Vec<Value> = lues["attestations"].as_array().map(|a| a.iter().map(|x| x["periode"].clone()).collect()).unwrap_or_default();
    assert_eq!(periodes.len(), INTERMITTENTS.len(), "{}", lues);
    assert!(periodes.iter().all(|p| p == &json!(MOIS)));

    // Rangee et signee pour chaque intermittent, avec les chiffres de sa paie ; rien pour un permanent.
    let ent = lire(&r, "config/entreprise.json");
    let toutes = paies(&r);
    for cle_ in INTERMITTENTS {
        let pdf = std::fs::read(aer_de(&r, cle_)).unwrap_or_else(|_| panic!("pas d'AER pour {}", cle_));
        let champs = paie::aer::champs(&pdf).unwrap();
        assert_eq!(champs["EMPLOYEUR_NOM"]["valeur"], ent["signataire"]["nom"], "{} : cadre 5 non signe", cle_);
        let p = toutes.iter().find(|p| cle(p) == cle_).unwrap();
        let e = &p.engagement;
        let paie_aer = [paie::aer::PaieAer::du_mois(e).unwrap()];
        assert_eq!(paie::aer::verifier(&pdf, &paie_aer).unwrap(), Vec::<String>::new(), "{}", cle_);
        assert_eq!(paie::aer::lire_quantites(&champs).unwrap().len(), 1, "{} : une periode, celle du contrat", cle_);
    }
    for cle_ in PERMANENTS {
        assert!(!aer_de(&r, cle_).exists(), "{} est en CDI", cle_);
    }

    // Elle part avec ses documents ; le permanent recoit les siens sans la phrase de l'AER.
    let ctx = contexte(&r);
    let messages = paie::envoi::preparer(&toutes, &ctx.entreprise, MOIS, &envoi(&r), None).unwrap();
    for m in &messages {
        let nom_aer = format!("AER_{}_Septembre2026.pdf", m.cle);
        let a_l_aer = m.pieces.iter().any(|p| p.file_name().unwrap().to_string_lossy() == nom_aer);
        assert_eq!(a_l_aer, INTERMITTENTS.contains(&m.cle.as_str()), "{}", m.cle);
        assert!(!m.corps.contains(paie::envoi::NOTE_SANS_AER), "{} : l'AER est jointe ou n'est pas due", m.cle);
    }
    assert_eq!(messages.len(), INTERMITTENTS.len() + PERMANENTS.len());

    // Rejouee, la releve ne refait rien.
    assert_eq!(paie::releve_retours::rattacher_simulation_detail(&contexte(&r), 6).unwrap().0, 0);
    assert_eq!(archives_46(&r).len(), 1);
}

#[test]
fn sans_son_aer_l_intermittent_le_lit_et_le_permanent_n_en_entend_pas_parler() {
    let r = compagnie::entiere(FICTIVE, &dossier_de_test("aer_fictive", "note"));
    let ctx = contexte(&r);
    let messages = paie::envoi::preparer(&paies(&r), &ctx.entreprise, MOIS, &envoi(&r), None).unwrap();
    for m in &messages {
        let note = m.corps.contains(paie::envoi::NOTE_SANS_AER);
        assert_eq!(note, INTERMITTENTS.contains(&m.cle.as_str()), "{}", m.cle);
    }
}

#[test]
fn ni_une_fictive_ajoutee_ni_une_compagnie_reelle_n_ont_d_aer_jouee() {
    // La fictive ajoutee : le marqueur qu'ecrit le « + » d'un logiciel bride.
    let ajoutee = copie("ajoutee");
    let inst = dossier_de_test("aer_fictive", "marque");
    let c = paie::interface::compagnies::ajouter_dans(&inst, "Marque").unwrap();
    std::fs::copy(c.racine.join("COPIE-DE-DEVELOPPEMENT.json"), ajoutee.join("COPIE-DE-DEVELOPPEMENT.json")).unwrap();
    // La reelle : sans le marqueur de compagnie fictive.
    let reelle = copie("reelle");
    std::fs::remove_file(reelle.join("COPIE-DE-DEVELOPPEMENT.json")).unwrap();
    for r in [&ajoutee, &reelle] {
        assert!(!paie::routes_accueil::simulation_active(r));
        let _ = paie::releve_retours::rattacher_simulation_detail(&contexte(r), 6).unwrap();
        assert!(!ligne_du_mois(r)["retours"].as_array().is_some_and(|a| a.iter().any(|x| x["nature"] == json!("46"))), "{}", r.display());
        assert!(archives_46(r).is_empty());
        assert!(noms(&envoi(r)).iter().all(|n| !n.starts_with("AER_")));
    }
}

/// LA FIN D'UN CDD DE DROIT COMMUN (`fictive-3090-grande`, AUDINOT, CDD du 01/09/2025 au 30/06/2026) : pas d'AER Cinema
/// Spectacle (elle est due a l'intermittent, `aer::due`), mais l'attestation employeur qu'edite France Travail en retour
/// du signalement de fin de contrat (fiche net-entreprises 3037 ; guide API DSN v6.8, §5.7.4). Elle n'est pas dans le
/// courriel des documents, et le courriel le dit -- a lui seul : les permanents qui restent n'ont aucune des deux notes.
#[test]
fn la_fin_d_un_cdd_le_dit_sans_annoncer_d_aer_cinema_spectacle() {
    let r = compagnie::entiere("fictive-3090-grande", &dossier_de_test("aer_fictive", "cdd"));
    let ctx = contexte(&r);
    let paies = engagement::charger(&ctx, "2026-06").unwrap().paies;
    let messages = paie::envoi::preparer(&paies, &ctx.entreprise, "2026-06", &r.join("sorties/2026-06/envoi"), None).unwrap();
    assert_eq!(messages.len(), 3);
    for m in &messages {
        assert!(!m.corps.contains(paie::envoi::NOTE_SANS_AER), "{} n'est pas intermittent", m.cle);
        assert_eq!(m.corps.contains(paie::envoi::NOTE_SANS_ATTESTATION_FCTU), m.cle == "AUDINOT", "{}", m.cle);
    }
}

/// CE QUE L'ECRAN NOMME (#236) : l'etape « Documents » d'un mois ne cite que les pieces qui existent pour ce mois (le
/// moteur les nomme, `etat_par_paie` / `bilan-du-mois`), et l'Historique cite les AER du mois quand il y en a.
#[test]
fn l_ecran_ne_nomme_que_les_pieces_du_mois_et_l_historique_cite_l_aer() {
    let r = copie("ecran");
    let detail = |mois: &str| crate::outils::reussi(&crate::outils::route(&r, "bilan-du-mois", json!({"mois": mois}), JOUR))["documents"]["detail"].clone();
    // Aout 2026 : deux permanents seuls ; septembre : quatre intermittents qui finissent leur contrat.
    assert_eq!(detail("2026-08"), json!("bulletin"));
    assert_eq!(detail(MOIS), json!("bulletin, certificat Congés Spectacles, AER"));
    let sortes = |r: &Path| {
        let e = crate::outils::reussi(&crate::outils::route(r, "etat", json!({}), JOUR)).clone();
        e["historique"].as_array().unwrap().iter().find(|x| x["mois"] == json!(MOIS)).unwrap()["sortes"].clone()
    };
    assert!(!sortes(&r).as_array().unwrap().contains(&json!("attestations AER")));
    paie::releve_retours::rattacher_simulation_detail(&contexte(&r), 6).unwrap();
    assert!(sortes(&r).as_array().unwrap().contains(&json!("attestations AER")), "{}", sortes(&r));
}

/// UNE DSN QUI ANNULE ET REMPLACE (type 03, S20.G00.05.002) : l'attestation que France Travail reedite le dit en tete,
/// avec l'ordre de la declaration -- la fausse AER aussi.
#[test]
fn apres_une_dsn_qui_annule_et_remplace_l_aer_le_dit() {
    let r = copie("remplace");
    let f = r.join("sorties").join(MOIS).join("DSN_Septembre2026.dsn");
    let texte = String::from_utf8_lossy(&std::fs::read(&f).unwrap()).into_owned();
    let texte = texte.replace("S20.G00.05.002,'01'", "S20.G00.05.002,'03'").replace("S20.G00.05.004,'1'", "S20.G00.05.004,'2'");
    std::fs::write(&f, texte).unwrap();
    paie::releve_retours::rattacher_simulation_detail(&contexte(&r), 6).unwrap();
    let champs = paie::aer::champs(&std::fs::read(aer_de(&r, "TURLUPIN")).unwrap()).unwrap();
    let entete = champs["ENTETE_PROVENANCE"]["valeur"].as_str().unwrap().to_string();
    assert!(entete.contains("annule et remplace") && entete.contains("N° d'ordre 2"), "{}", entete);
}
