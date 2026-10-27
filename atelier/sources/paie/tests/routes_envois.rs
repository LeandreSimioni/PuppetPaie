//! Les six routes d'envoi : `envoyer-documents`, `dossier-envoyer`, `casc-envoyer`, `demander-signature`,
//! `avenant-envoyer`, `avenants-envoyer`. Rien ne part sans la preuve (« Rien n'est parti »), un envoi qui casse au milieu
//! le dit et le note, les numeros de contrat ne s'avancent que jusqu'au dernier parti, ce qui part est l'archive. Aucun
//! reseau : la boite simulee de la compagnie fictive, ou un faux `Courriel` ; chaque test sur sa copie d'une compagnie fictive.
//!
//! Le jour et la cle du poste sont dans l'environnement du processus, poses une fois ; `avec_le` change le jour le temps
//! d'un appel et tourne seul (`tour_seul`).

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Once, OnceLock, RwLock, RwLockReadGuard, RwLockWriteGuard};

use outils::{compagnie, dossier_de_test, dossier_du_processus, ecrire, lire, noms, paires, FauxCourriel as Faux, Mail};
use paie::coffre::Coffre;
use droit::contexte::Contexte;
use socle::date::Jour;
use socle::echec::Echec;
use paie::{routes_documents, routes_ecritures_g4, routes_envois};
use rust_decimal::{Decimal, RoundingStrategy};
use serde_json::{json, Value};

#[path = "outils/mod.rs"]
mod outils;

const AUJOURDHUI: &str = "2026-10-03";
const MOT_DE_PASSE: &str = "sesame-123";

static VERROU: RwLock<()> = RwLock::new(());
static UNE_FOIS: Once = Once::new();

fn tour() -> RwLockReadGuard<'static, ()> {
    let g = VERROU.read().unwrap_or_else(|e| e.into_inner());
    preparer();
    g
}

fn tour_seul() -> RwLockWriteGuard<'static, ()> {
    let g = VERROU.write().unwrap_or_else(|e| e.into_inner());
    preparer();
    g
}

fn preparer() {
    UNE_FOIS.call_once(|| {
        for v in outils::RETIREES {
            std::env::remove_var(v);
        }
        std::env::set_var("PUPPETPAIE_AUJOURDHUI", AUJOURDHUI);
        std::env::set_var("PUPPETPAIE_CLE", dossier_poste());
    });
}

fn dossier_poste() -> PathBuf {
    static D: OnceLock<PathBuf> = OnceLock::new();
    D.get_or_init(|| dossier_du_processus("routes-envois-poste")).clone()
}

/// Une copie de `fictive-3090-petite`. `fictive` : elle garde son marqueur, ses envois sont joues (boite simulee, rien
/// a prouver) ; sinon c'est une compagnie ordinaire : la preuve est exigee, la boite est un faux client.
fn compagnie(nom: &str, fictive: bool) -> PathBuf {
    compagnie_de(nom, "fictive-3090-petite", fictive)
}

fn compagnie_de(nom: &str, modele: &str, fictive: bool) -> PathBuf {
    let racine = compagnie::entiere(modele, &dossier_de_test("routes_envois", nom).join("compagnie"));
    if !fictive {
        std::fs::remove_file(racine.join("COPIE-DE-DEVELOPPEMENT.json")).unwrap();
    }
    racine
}

/// Le mot de passe du logiciel, pose une fois par processus dans un coffre modele (scrypt est lent), recopie ensuite ; la
/// cle du poste est celle de `PUPPETPAIE_CLE`.
fn poser_le_mot_de_passe(racine: &Path) {
    static MODELE: OnceLock<PathBuf> = OnceLock::new();
    let modele = MODELE.get_or_init(|| {
        let c = Coffre { racine: dossier_du_processus("routes-envois-coffre"), dossier_cle: dossier_poste(), maintenant: "2026-10-03T10:00".into(), variables: BTreeMap::new() };
        c.poser_ouverture(MOT_DE_PASSE, None).unwrap();
        c.chemin()
    });
    std::fs::copy(modele, racine.join("config").join(modele.file_name().unwrap())).unwrap();
}

fn journal(racine: &Path) -> Vec<Value> {
    lire(racine, "journal/depots.json").as_array().unwrap().clone()
}

/// Une boite qui refuse le message de rang `n`.
fn refuse_le(n: usize) -> Faux {
    Faux { refuse_le: Some(n), ..Faux::default() }
}

fn options(racine: &Path) -> HashMap<String, String> {
    let mut o = HashMap::new();
    o.insert("racine".to_string(), racine.to_string_lossy().into_owned());
    o.insert("date".to_string(), AUJOURDHUI.to_string());
    o
}

fn contexte(racine: &Path) -> Contexte {
    contexte_le(racine, AUJOURDHUI)
}

fn contexte_le(racine: &Path, jour: &str) -> Contexte {
    Contexte::nouveau(racine, Some(Jour::lire(jour).unwrap())).unwrap()
}

/// Une route d'envoi un autre jour que `AUJOURDHUI` (un contrat qui a commence).
fn avec_le(racine: &Path, jour: &str, nom: &str, charge: Value, faux: &Faux) -> Result<Value, Echec> {
    let mut o = options(racine);
    o.insert("date".to_string(), jour.to_string());
    // `PUPPETPAIE_AUJOURDHUI` l'emporte sur la date du contexte : le poser le temps de l'appel (les tests se passent le relais).
    std::env::set_var("PUPPETPAIE_AUJOURDHUI", jour);
    let ctx = contexte_le(racine, jour);
    let r = routes_envois::route_avec(&o, &ctx, nom, &charge, &mut || Ok(Box::new(faux.clone())));
    std::env::set_var("PUPPETPAIE_AUJOURDHUI", AUJOURDHUI);
    r
}

/// Une route d'envoi avec ce faux client (la vraie boite n'est jamais ouverte).
fn avec(racine: &Path, nom: &str, charge: Value, faux: &Faux) -> Result<Value, Echec> {
    routes_envois::route_avec(&options(racine), &contexte(racine), nom, &charge, &mut || Ok(Box::new(faux.clone())))
}

/// Une route d'envoi avec la boite que `_client_courriel` choisit : la simulee dans une compagnie fictive, sinon aucune.
fn telle_quelle(racine: &Path, nom: &str, charge: Value) -> Result<Value, Echec> {
    routes_envois::route(&options(racine), &contexte(racine), nom, &charge)
}

fn phrase(r: Result<Value, Echec>) -> (String, String) {
    let e = r.expect_err("un refus etait attendu");
    (e.classe.unwrap_or("").to_string(), e.message)
}

/// Les routes d'ecriture et d'apercu dont les envois ont besoin.
fn ecrire_les_documents(racine: &Path, mois: &str) {
    routes_ecritures_g4::route(&contexte(racine), "ecrire-documents", &json!({"mois": mois})).unwrap();
}

fn apercu(racine: &Path, nom: &str, charge: Value) -> Value {
    routes_documents::route(&options(racine), &contexte(racine), nom, &charge).unwrap()
}

// ================================================================== envoyer-documents

#[test]
fn envoyer_documents_dans_une_fictive_ecrit_les_courriels_simules_et_le_journal() {
    let _t = tour();
    let racine = compagnie("documents-fictive", true);
    ecrire_les_documents(&racine, "2026-08");
    let avant = journal(&racine).len();
    let r = telle_quelle(&racine, "envoyer-documents", json!({"mois": "2026-08"})).unwrap();

    // La reponse : partis, messages, expediteur, avertissements, ligne.
    let mut cles: Vec<&str> = r.as_object().unwrap().keys().map(String::as_str).collect();
    cles.sort();
    assert_eq!(cles, ["avertissements", "expediteur", "ligne", "messages", "partis"]);
    assert_eq!(r["messages"], json!(3));
    assert_eq!(r["partis"].as_array().unwrap().len(), 3);
    assert_eq!(r["expediteur"], json!("direction@demo-3090-petite.invalid"));
    assert_eq!(r["avertissements"], json!([]));

    // Les trois messages sont ecrits, pas envoyes : un .eml par salarie, avec ses pieces.
    let emls: Vec<String> = noms(&racine.join("courriels-simules")).into_iter().filter(|n| n.ends_with(".eml")).collect();
    assert_eq!(emls.len(), 3, "{:?}", emls);
    let m = Mail(std::fs::read(racine.join("courriels-simules").join(&emls[0])).unwrap());
    assert!(m.entete("Subject").contains("2026"), "{}", m.entete("Subject"));
    assert!(m.entete("From").contains("direction@demo-3090-petite.invalid"));
    assert!(!m.pieces().is_empty());

    // Une ligne de plus au journal, acceptee, au mode `reel`, avec le detail.
    let j = journal(&racine);
    assert_eq!(j.len(), avant + 1);
    let l = j.last().unwrap();
    assert_eq!((l["quoi"].as_str(), l["accepte"].as_bool(), l["mode"].as_str(), l["mois"].as_str(), l["qui"].as_str()), (Some("documents"), Some(true), Some("reel"), Some("2026-08"), Some("interface")));
    assert_eq!(l["reponse"], json!("3 message(s) envoye(s)"));
    assert_eq!(l["detail"]["expediteur"], json!("direction@demo-3090-petite.invalid"));
    assert_eq!(l["detail"]["salaries"].as_array().unwrap().len(), 3);
    assert_eq!(l["detail"]["salaries"][0]["cle"], json!("BRIOCHE"));
    assert!(l["detail"]["salaries"][0]["pieces"].as_array().unwrap().iter().any(|p| p.as_str().unwrap().starts_with("Bulletin_BRIOCHE")));
    assert_eq!(&r["ligne"], l);
}

#[test]
fn envoyer_documents_sans_preuve_ou_avec_une_fausse_n_ecrit_rien() {
    let _t = tour();
    let racine = compagnie("documents-preuve", false);
    ecrire_les_documents(&racine, "2026-08");
    let j0 = journal(&racine);
    let sortie0 = noms(&racine.join("sorties/2026-08/envoi"));
    let faux = Faux::nouveau();

    // Aucun mot de passe du logiciel n'est pose : rien ne peut partir.
    let (classe, m) = phrase(avec(&racine, "envoyer-documents", json!({"mois": "2026-08", "motdepasse": MOT_DE_PASSE}), &faux));
    assert_eq!(classe, "PreuveInsuffisante");
    assert!(m.contains("Aucun mot de passe du logiciel n'est pose") && m.ends_with("Rien n'est parti."), "{}", m);

    poser_le_mot_de_passe(&racine);
    // Sans mot de passe retape, puis avec un faux.
    let (classe, m) = phrase(avec(&racine, "envoyer-documents", json!({"mois": "2026-08"}), &faux));
    assert_eq!(classe, "PreuveInsuffisante");
    // Rien n'a ete tape : le moteur demande lui-meme, et sans humain (ni ecran ni terminal) il refuse (lot J1).
    assert!(m.contains("aucun humain") && m.starts_with("Rien n'est parti"), "{}", m);
    let (classe, m) = phrase(avec(&racine, "envoyer-documents", json!({"mois": "2026-08", "motdepasse": "pas-le-bon"}), &faux));
    assert_eq!(classe, "PreuveInsuffisante");
    assert!(m.contains("Mot de passe du logiciel refuse"), "{}", m);
    // Rien n'est parti, rien n'est note, rien n'est ecrit.
    assert_eq!(faux.partis(), 0);
    assert_eq!(journal(&racine), j0);
    assert_eq!(noms(&racine.join("sorties/2026-08/envoi")), sortie0);
    assert!(!racine.join("courriels-simules").exists());

    // Le bon mot de passe : tout part, par le faux client, et le journal le dit.
    let r = avec(&racine, "envoyer-documents", json!({"mois": "2026-08", "motdepasse": MOT_DE_PASSE}), &faux).unwrap();
    assert_eq!(faux.partis(), 3);
    assert_eq!(r["partis"], json!(["fanchon.brioche@exemple.invalid", "valentin.seraphin@exemple.invalid", "nicole.lemercier@exemple.invalid"]));
    // La boite posee n'est pas celle du signataire : l'avertissement le dit.
    assert_eq!(r["expediteur"], json!("boite@exemple.invalid"));
    assert_eq!(
        r["avertissements"],
        json!(["Les messages partiront de boite@exemple.invalid (la boite posee), pas de direction@demo-3090-petite.invalid (le courriel du signataire, fiche Entreprise)."])
    );
    let m = faux.message(0);
    assert_eq!(m.entete("To"), "fanchon.brioche@exemple.invalid");
    assert_eq!(m.entete("From"), "boite@exemple.invalid");
    assert!(m.pieces().iter().any(|p| p.starts_with("Bulletin_BRIOCHE")), "{:?}", m.pieces());
    assert_eq!(journal(&racine).len(), j0.len() + 1);
}

#[test]
fn envoyer_documents_qui_echoue_au_milieu_le_dit_et_le_note() {
    let _t = tour();
    let racine = compagnie("documents-echec", false);
    poser_le_mot_de_passe(&racine);
    ecrire_les_documents(&racine, "2026-08");
    let j0 = journal(&racine);
    // Le deuxieme message est refuse : le premier est parti.
    let faux = refuse_le(1);
    let (_, m) = phrase(avec(&racine, "envoyer-documents", json!({"mois": "2026-08", "motdepasse": MOT_DE_PASSE}), &faux));
    assert!(m.starts_with("valentin.seraphin@exemple.invalid : le serveur refuse ce message"), "{}", m);
    assert!(m.contains("ATTENTION, 1 message(s) etaient deja partis : fanchon.brioche@exemple.invalid"), "{}", m);
    assert_eq!(faux.partis(), 1);
    // Une seule ligne de plus : le refus, avec ce qui s'est passe et a quelle adresse.
    let j = journal(&racine);
    assert_eq!(j.len(), j0.len() + 1);
    let l = j.last().unwrap();
    assert_eq!((l["quoi"].as_str(), l["accepte"].as_bool(), l["mois"].as_str()), (Some("documents"), Some(false), Some("2026-08")));
    // Retours Windows, C : les partis comme destinataires, l'echec a part, jamais l'expediteur.
    assert_eq!(l["destinataire"], json!("fanchon.brioche@exemple.invalid"));
    assert!(l["reponse"].as_str().unwrap().contains("ATTENTION, 1 message(s) etaient deja partis"));
    assert_eq!(l["detail"], json!({"partis": ["fanchon.brioche@exemple.invalid"], "echec_a": "valentin.seraphin@exemple.invalid"}));

    // Le premier message refuse : rien n'est parti, l'erreur est celle du client.
    let faux = refuse_le(0);
    let (classe, m) = phrase(avec(&racine, "envoyer-documents", json!({"mois": "2026-08", "motdepasse": MOT_DE_PASSE}), &faux));
    assert_eq!((classe.as_str(), m.as_str()), ("ErreurCourriel", "le serveur refuse ce message"));
    assert_eq!(faux.partis(), 0);
    // Aucun message n'est parti : le destinataire du journal est vide (pas l'expediteur), l'echec est nomme a part.
    let l = journal(&racine).last().unwrap().clone();
    assert_eq!((l["accepte"].as_bool(), l["destinataire"].as_str()), (Some(false), Some("")));
    assert_eq!(l["detail"], json!({"partis": [], "echec_a": "fanchon.brioche@exemple.invalid"}));
    assert!(l["reponse"].as_str().unwrap().starts_with("echec de l'envoi a fanchon.brioche@exemple.invalid : "), "{}", l["reponse"]);
}

#[test]
fn envoyer_documents_refuse_une_boite_absente_et_un_mois_non_ecrit() {
    let _t = tour();
    let racine = compagnie("documents-boite", false);
    ecrire_les_documents(&racine, "2026-08");
    let j0 = journal(&racine);
    // Ni adresse ni mot de passe d'application : la boite est absente, avant toute preuve.
    let (classe, m) = phrase(telle_quelle(&racine, "envoyer-documents", json!({"mois": "2026-08", "motdepasse": MOT_DE_PASSE})));
    assert_eq!(classe, "BoiteAbsente");
    assert!(m.starts_with("Aucune boite courriel : poser le mot de passe d'application"), "{}", m);
    assert_eq!(journal(&racine), j0);
    // Un mois dont les documents ne sont pas decoupes par salarie : `sorties/<mois>/envoi` n'existe pas.
    let (classe, m) = phrase(telle_quelle(&racine, "envoyer-documents", json!({"mois": "2026-07"})));
    assert_eq!(classe, "SortieAbsente");
    assert!(m.ends_with("envoi n'existe pas."), "{}", m);
    // Un mois sans saisie, et une charge sans mois.
    assert!(phrase(telle_quelle(&racine, "envoyer-documents", json!({"mois": "2031-01"}))).1.contains("2031-01"));
    phrase(telle_quelle(&racine, "envoyer-documents", json!({})));
    assert_eq!(journal(&racine), j0);
}

/// Une route du mois (`etat-par-engagement`, `bilan-du-mois`) sur cette compagnie.
fn du_mois(racine: &Path, nom: &str, mois: &str) -> Value {
    paie::routes_mois::route(&options(racine), &contexte(racine), nom, &json!({"mois": mois})).unwrap()
}

/// La pastille Documents de chaque paie du mois, par salarie : (caduc, pourquoi).
fn pastilles_documents(racine: &Path, mois: &str) -> Vec<(String, bool, String)> {
    du_mois(racine, "etat-par-engagement", mois)["engagements"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            let d = &p["etats"]["documents"];
            assert!(d["fait"].is_string(), "{}", d);
            (p["salarie"].as_str().unwrap().to_string(), d["caduc"].as_bool().unwrap(), d["pourquoi"].as_str().unwrap().to_string())
        })
        .collect()
}

/// TICKET #240 : « j'ai change des infos sur les heures des salaries ; il m'a refait une DSN, mais les documents transmis
/// aux salaries ne sont plus bons, et il ne me propose pas de les renvoyer ». Les documents partis disent la paie du jour de
/// l'envoi : une paie qui change ensuite les rend caducs. La pastille et l'etape le disent, le renvoi ne part qu'aux
/// salaries dont la paie a change, avec un courriel qui dit l'ancien et le nouveau ; apres lui, tout est a jour.
#[test]
fn des_documents_partis_puis_une_paie_modifiee_se_renvoient_au_seul_salarie_concerne_en_disant_ce_qui_a_change() {
    let _t = tour();
    let racine = compagnie("documents-renvoi", false);
    poser_le_mot_de_passe(&racine);
    ecrire_les_documents(&racine, "2026-08");
    let faux = Faux::nouveau();
    avec(&racine, "envoyer-documents", json!({"mois": "2026-08", "motdepasse": MOT_DE_PASSE}), &faux).unwrap();
    assert_eq!(faux.partis(), 3);
    // Le journal garde, pour chacun, l'empreinte de sa paie et ce que ses documents disaient.
    let envoi = journal(&racine).last().unwrap().clone();
    let lemercier = envoi["detail"]["salaries"].as_array().unwrap().iter().find(|s| s["cle"] == "LEMERCIER").unwrap().clone();
    assert!(lemercier["empreinte"].as_str().is_some_and(|e| e.len() == 16), "{}", lemercier);
    assert_eq!(lemercier["resume"]["contrats"][0]["heures"], json!("32"));
    // Tout est a jour : aucune pastille a renvoyer, l'etape est verte.
    assert!(pastilles_documents(&racine, "2026-08").iter().all(|(_, caduc, _)| !caduc));
    assert_eq!(du_mois(&racine, "bilan-du-mois", "2026-08")["documents"]["couleur"], json!("bien"));

    // Les heures de Lemercier changent (32 -> 40), et les documents du mois se refont.
    let mut saisie = crate::outils::saisie(&racine, "2026-08");
    let rang = saisie["engagements"].as_array().unwrap().iter().position(|e| e["salarie"] == "Lemercier").unwrap();
    saisie["engagements"][rang]["lignes"][0]["quantite"] = json!(40);
    crate::outils::poser_saisie(&racine, "2026-08", &saisie);
    ecrire_les_documents(&racine, "2026-08");

    // Sa pastille n'est plus verte : la paie a change depuis l'envoi. Les deux autres n'ont pas bouge.
    let p = pastilles_documents(&racine, "2026-08");
    assert_eq!(p.iter().filter(|(_, c, _)| *c).count(), 1, "{:?}", p);
    assert!(p.iter().any(|(s, c, q)| s == "LEMERCIER" && *c && q == "la paie a changé depuis l'envoi"), "{:?}", p);
    // L'etape le dit, le compte, et propose le renvoi.
    let b = du_mois(&racine, "bilan-du-mois", "2026-08")["documents"].clone();
    assert_eq!((b["couleur"].as_str(), b["a_renvoyer"].as_i64(), b["courte"].as_str()), (Some("alerte"), Some(1), Some("1 à renvoyer")), "{}", b);
    assert_eq!(b["bouton"], json!("Renvoyer les documents corrigés (1)…"));
    assert!(b["phrase"].as_str().unwrap().starts_with("Documents à renvoyer à 1 salarié : la paie a changé depuis l'envoi (") && b["phrase"].as_str().unwrap().to_lowercase().contains("lemercier"), "{}", b);
    // L'apercu ne montre que lui, en renvoi, avec ce qui a change.
    let a = apercu(&racine, "envoi-apercu", json!({"mois": "2026-08"}));
    let m = a["messages"].as_array().unwrap();
    assert_eq!(m.len(), 1, "{}", a);
    assert_eq!((m[0]["cle"].as_str(), m[0]["renvoi"].as_bool()), (Some("LEMERCIER"), Some(true)));
    assert!(m[0]["changements"].as_array().unwrap().contains(&json!("heures : 32 → 40")), "{}", m[0]);
    assert_eq!((a["renvois"].as_i64(), a["bouton"].as_str(), &a["deja"]), (Some(1), Some("Renvoyer les documents corrigés à 1 salarié"), &Value::Null));

    // Le renvoi : un seul message, a lui, qui dit l'ancien et le nouveau et que les documents precedents sont remplaces.
    let faux = Faux::nouveau();
    let r = avec(&racine, "envoyer-documents", json!({"mois": "2026-08", "motdepasse": MOT_DE_PASSE}), &faux).unwrap();
    assert_eq!(faux.partis(), 1);
    assert_eq!(r["partis"], json!(["nicole.lemercier@exemple.invalid"]));
    let mail = faux.message(0);
    assert!(mail.entete("Subject").contains("documents corrigés"), "{}", mail.entete("Subject"));
    let (corps, _) = mail.textes();
    assert!(corps.contains("a été modifiée depuis l'envoi de tes documents le "), "{}", corps);
    assert!(corps.contains("  - heures : 32 → 40\n"), "{}", corps);
    assert!(corps.contains("  - salaire brut : 576,00 € → 720,00 €\n"), "{}", corps);
    assert!(corps.contains("  - net à payer : "), "{}", corps);
    assert!(corps.contains("Ils remplacent ceux que tu as reçus le "), "{}", corps);
    assert!(mail.pieces().iter().any(|p| p.starts_with("Bulletin_LEMERCIER")), "{:?}", mail.pieces());
    let l = journal(&racine).last().unwrap().clone();
    assert_eq!(l["detail"]["salaries"].as_array().unwrap().len(), 1);
    assert_eq!((l["detail"]["salaries"][0]["renvoi"].as_bool(), &l["detail"]["salaries"][0]["resume"]["contrats"][0]["heures"]), (Some(true), &json!("40")));

    // Apres le renvoi, tout est a jour.
    assert!(pastilles_documents(&racine, "2026-08").iter().all(|(_, caduc, _)| !caduc));
    assert_eq!(du_mois(&racine, "bilan-du-mois", "2026-08")["documents"]["couleur"], json!("bien"));
    // Un nouvel envoi, voulu, repart a tous, avec le premier courriel.
    let faux = Faux::nouveau();
    avec(&racine, "envoyer-documents", json!({"mois": "2026-08", "motdepasse": MOT_DE_PASSE}), &faux).unwrap();
    assert_eq!(faux.partis(), 3);
    assert!(!faux.message(0).entete("Subject").contains("corrigés"));
}

/// Le texte du bulletin individuel de Lemercier pour aout 2026, tel qu'il partirait.
fn bulletin_de_lemercier(racine: &Path) -> String {
    let f = racine.join("sorties/2026-08/envoi/Bulletin_LEMERCIER_Aout2026.pdf");
    fabrique_pdf::lecture::texte(&f.to_string_lossy()).unwrap().0.join("\n")
}

/// Les heures de Lemercier en aout 2026 portees a `heures`, par la route, en confirmant (la DSN d'aout ne se remplace plus).
fn modifier_lemercier(racine: &Path, heures: i64) {
    let mut saisie = crate::outils::saisie(racine, "2026-08");
    let rang = saisie["engagements"].as_array().unwrap().iter().position(|e| e["salarie"] == "Lemercier").unwrap();
    saisie["engagements"][rang]["lignes"][0]["quantite"] = json!(heures);
    let charge = json!({"mois": "2026-08", "saisie": saisie, "modifier_apres_echeance": true});
    paie::routes_ecritures::route(&options(racine), &contexte(racine), "enregistrer-saisie", &charge).unwrap();
}

/// APRES LA LIMITE DE L'ANNULE-ET-REMPLACE (Leandre, 21/10/2026) : la DSN d'aout ne se refait plus, la paie modifiee se
/// declare dans une DSN suivante. Les documents corriges repartent quand meme -- refaits depuis la paie d'aujourd'hui sans
/// rien redeclarer --, et le courriel dit avec quelle paie la correction sera declaree : le mois de la DSN qui la porte une
/// fois ecrite (« Corriger ce mois »), « une paie suivante » tant qu'elle ne l'est pas.
#[test]
fn apres_la_limite_les_documents_corriges_repartent_et_le_courriel_dit_quand_la_correction_sera_declaree() {
    let _t = tour();
    let racine = compagnie("documents-apres-limite", false);
    poser_le_mot_de_passe(&racine);
    ecrire_les_documents(&racine, "2026-08");
    avec(&racine, "envoyer-documents", json!({"mois": "2026-08", "motdepasse": MOT_DE_PASSE}), &Faux::nouveau()).unwrap();
    assert!(bulletin_de_lemercier(&racine).contains("576,00"));

    // La paie change apres la limite : rien n'empeche le renvoi.
    modifier_lemercier(&racine, 40);
    let a = apercu(&racine, "envoi-apercu", json!({"mois": "2026-08"}));
    assert_eq!(a["empechements"], json!([]), "{}", a);
    assert_eq!(a["renvois"], json!(1), "{}", a);

    // Le renvoi : ses documents refaits (personne n'a reecrit les documents du mois), et la phrase de la declaration.
    let faux = Faux::nouveau();
    avec(&racine, "envoyer-documents", json!({"mois": "2026-08", "motdepasse": MOT_DE_PASSE}), &faux).unwrap();
    assert_eq!(faux.partis(), 1);
    assert!(bulletin_de_lemercier(&racine).contains("720,00"), "{}", bulletin_de_lemercier(&racine));
    let (corps, _) = faux.message(0).textes();
    assert!(corps.contains("  - salaire brut : 576,00 € → 720,00 €\n"), "{}", corps);
    assert!(corps.contains("ne garde que ceux-ci.\n\nTa paie d'août 2026 était déjà déclarée : la correction sera déclarée avec une paie suivante.\n\n"), "{}", corps);

    // La paie change encore, et sa correction s'ecrit (« Corriger ce mois ») : la DSN d'octobre la portera.
    modifier_lemercier(&racine, 48);
    let o = options(&racine);
    let ctx = contexte(&racine);
    let m = paie::routes_ecrites::route(&o, &ctx, "regularisation-mois", &json!({"mois": "2026-08"})).unwrap();
    let e = m["engagements"].as_array().unwrap().iter().find(|e| e["salarie"] == "LEMERCIER").unwrap().clone();
    let charge = json!({"mois": "2026-08", "salarie": "LEMERCIER", "debut": e["debut"], "engagement": {"lignes": e["engagement_saisi"]["lignes"]},
                        "motif": "heures de Lemercier", "motdepasse": MOT_DE_PASSE});
    paie::routes_ecritures_g2::route(&o, &ctx, "regularisation-ecrire", &charge).unwrap();
    let faux = Faux::nouveau();
    avec(&racine, "envoyer-documents", json!({"mois": "2026-08", "motdepasse": MOT_DE_PASSE}), &faux).unwrap();
    let (corps, _) = faux.message(0).textes();
    assert!(corps.contains("Ta paie d'août 2026 était déjà déclarée : la correction sera déclarée avec la paie d'octobre 2026.\n\n"), "{}", corps);
}

#[test]
fn le_courriel_de_renvoi_dit_ce_qui_a_change_contrat_par_contrat() {
    use paie::envoi::ce_qui_a_change;
    let contrat = |debut: &str, jours: &[&str], cachets: i64, heures: &str, brut: &str, net: &str| {
        json!({"debut": debut, "fin": jours.last(), "spectacle": "Le Spectacle", "jours": jours, "cachets": cachets, "heures": heures, "brut": brut, "net": net})
    };
    // Un seul contrat : une ligne par fait qui a change.
    let a = json!({"contrats": [contrat("2026-10-05", &["2026-10-05", "2026-10-06"], 1, "15", "300,00", "230,00")], "brut": "300,00", "net": "230,00"});
    let n = json!({"contrats": [contrat("2026-10-05", &["2026-10-05", "2026-10-06"], 1, "17,5", "340,00", "260,00")], "brut": "340,00", "net": "260,00"});
    assert_eq!(
        ce_qui_a_change(&a, &n),
        ["  - heures : 15 → 17,5", "  - salaire brut : 300,00 € → 340,00 €", "  - net à payer : 230,00 € → 260,00 €"]
    );
    // Rien de ce que le resume porte n'a bouge : rien a dire (le courriel le dit autrement).
    assert!(ce_qui_a_change(&a, &a).is_empty());
    // Deux contrats : celui qui a change, nomme, puis les totaux.
    let deux = |x: &Value, y: &Value, brut: &str| json!({"contrats": [x, y], "brut": brut, "net": "0,00"});
    let premier = contrat("2026-10-05", &["2026-10-05"], 1, "", "200,00", "0,00");
    let second = contrat("2026-10-20", &["2026-10-20"], 1, "", "200,00", "0,00");
    let second_change = contrat("2026-10-20", &["2026-10-20", "2026-10-21"], 2, "", "400,00", "0,00");
    assert_eq!(
        ce_qui_a_change(&deux(&premier, &second, "400,00"), &deux(&premier, &second_change, "600,00")),
        [
            "  - contrat du 20/10/2026 au 21/10/2026 (Le Spectacle) :",
            "      contrat du 20/10/2026 (Le Spectacle) → contrat du 20/10/2026 au 21/10/2026 (Le Spectacle)",
            "      jours travaillés : 20/10 → 20/10 et 21/10",
            "      cachets : 1 → 2",
            "      salaire brut : 200,00 € → 400,00 €",
            "  - total brut du mois : 400,00 € → 600,00 €",
        ]
    );
    // Un contrat de plus : avant, maintenant.
    let c = ce_qui_a_change(&json!({"contrats": [premier.clone()], "brut": "200,00", "net": "0,00"}), &deux(&premier, &second, "400,00"));
    assert_eq!(c[0], "  - avant : contrat du 05/10/2026 (Le Spectacle), 1 cachet, 200,00 € brut, 0,00 € net à payer");
    assert_eq!(c.len(), 4, "{:?}", c);
}

// ================================================================== demander-signature

/// Le mois d'octobre de la fictive, avec deux salaries et un salarie a deux contrats : de quoi essayer les envois groupes.
fn octobre_a_signer(racine: &Path) {
    let mut s = crate::outils::saisie(racine, "2026-10");
    let eng = s["engagements"][0].clone();
    let mut seraphin = eng.clone();
    seraphin["salarie"] = json!("Seraphin");
    let mut deuxieme = eng.clone();
    deuxieme["debut"] = json!("2026-10-27");
    deuxieme["fin"] = json!("2026-10-29");
    deuxieme["date_paiement"] = json!("2026-10-29");
    deuxieme["lignes"][0]["dates"] = json!(["2026-10-27", "2026-10-29"]);
    deuxieme["lignes"][0]["quantite"] = json!(2);
    deuxieme["lignes"][1]["quantite"] = json!(2);
    s["engagements"] = json!([eng, seraphin, deuxieme]);
    crate::outils::poser_saisie(racine, "2026-10", &s);
}

#[test]
fn demander_signature_sans_preuve_n_ecrit_rien_et_refuse_ce_qui_ne_va_pas() {
    let _t = tour();
    let racine = compagnie("signature-refus", false);
    octobre_a_signer(&racine);
    let saisie0 = std::fs::read(racine.join("engagements/2026-10.json")).unwrap();
    let compteurs0 = std::fs::read(racine.join("config/compteurs.json")).unwrap();
    let j0 = journal(&racine);
    let faux = Faux::nouveau();
    let charge = json!({"mois": "2026-10", "motdepasse": MOT_DE_PASSE});

    // Pas de mot de passe pose, puis un mauvais : la meme preuve que les autres envois.
    assert_eq!(phrase(avec(&racine, "demander-signature", charge.clone(), &faux)).0, "PreuveInsuffisante");
    poser_le_mot_de_passe(&racine);
    let (classe, m) = phrase(avec(&racine, "demander-signature", json!({"mois": "2026-10", "motdepasse": "non"}), &faux));
    assert_eq!((classe.as_str(), m.contains("Mot de passe du logiciel refuse")), ("PreuveInsuffisante", true), "{}", m);
    // Rien n'est parti, rien n'est ecrit : ni PDF, ni dossier, ni journal, ni numero reserve.
    assert_eq!(faux.partis(), 0);
    assert!(!racine.join("sorties/2026-10").exists());
    assert_eq!(journal(&racine), j0);
    assert_eq!(std::fs::read(racine.join("engagements/2026-10.json")).unwrap(), saisie0);
    assert_eq!(std::fs::read(racine.join("config/compteurs.json")).unwrap(), compteurs0);

    // Des contrats qui ne sont pas dans la saisie : refus avant tout, sous le mot de passe juste.
    let (_, m) = phrase(avec(
        &racine,
        "demander-signature",
        json!({"mois": "2026-10", "motdepasse": MOT_DE_PASSE, "contrats": [{"salarie": "BRIOCHE", "debut": "2026-10-13"}, {"salarie": "PERSONNE", "debut": "2026-10-01"}]}),
        &faux,
    ));
    assert!(m.contains("PERSONNE au 2026-10-01") && !m.contains("BRIOCHE"), "le refus nomme le seul contrat absent : {}", m);
    // Une relance d'un contrat jamais demande, un remplacement d'un contrat jamais parti.
    let (_, m) = phrase(avec(&racine, "demander-signature", json!({"mois": "2026-10", "motdepasse": MOT_DE_PASSE, "relance": true}), &faux));
    assert!(m.starts_with("Le contrat de BRIOCHE du 2026-10-13 n'a jamais ete demande : il s'envoie, il ne se relance pas."), "{}", m);
    let (_, m) = phrase(avec(&racine, "demander-signature", json!({"mois": "2026-10", "motdepasse": MOT_DE_PASSE, "remplace": true}), &faux));
    assert!(m.contains("13/10/2026") && m.contains("remplacer"), "le refus du remplacement : {}", m);
    let (_, m) = phrase(avec(&racine, "demander-signature", json!({"mois": "2026-10", "motdepasse": MOT_DE_PASSE, "remplace": true, "relance": true}), &faux));
    // La relance est verifiee AVANT : le contrat n'ayant jamais ete demande, c'est cette phrase.
    assert!(m.starts_with("Le contrat de BRIOCHE du 2026-10-13 n'a jamais ete demande : il s'envoie, il ne se relance pas."), "{}", m);
    // Un salarie sans courriel : son contrat ne peut pas partir, et rien n'est parti.
    let mut sal = lire(&racine, "config/salaries.json");
    sal["SERAPHIN"]["email"] = json!("");
    ecrire(&racine, "config/salaries.json", &sal);
    let (_, m) = phrase(avec(&racine, "demander-signature", charge.clone(), &faux));
    assert!(m.contains("SERAPHIN") && m.contains("courriel"), "{}", m);
    assert_eq!(faux.partis(), 0);
    // Retours Windows, B : toutes les adresses se verifient AVANT d'ecrire le moindre PDF (Brioche passe avant Seraphin).
    assert!(!racine.join("sorties/2026-10/envoi").exists(), "un PDF a ete ecrit avant le refus : {:?}", noms(&racine.join("sorties/2026-10")));
    assert_eq!(journal(&racine), j0);
    assert_eq!(std::fs::read(racine.join("config/compteurs.json")).unwrap(), compteurs0);
    // Une boite absente.
    let (_, m) = phrase(telle_quelle(&racine, "demander-signature", charge));
    assert!(m.starts_with("Aucune boite courriel"), "{}", m);
}

#[test]
fn demander_signature_envoie_un_courriel_par_salarie_et_garde_ce_qu_il_faut() {
    let _t = tour();
    let racine = compagnie("signature-envoi", false);
    poser_le_mot_de_passe(&racine);
    octobre_a_signer(&racine);
    let faux = Faux::nouveau();
    let j0 = journal(&racine);
    let compteurs0 = std::fs::read(racine.join("config/compteurs.json")).unwrap();
    let r = avec(&racine, "demander-signature", json!({"mois": "2026-10", "motdepasse": MOT_DE_PASSE}), &faux).unwrap();

    // La reponse : partis, contrats, expediteur, ligne.
    let mut cles: Vec<&str> = r.as_object().unwrap().keys().map(String::as_str).collect();
    cles.sort();
    assert_eq!(cles, ["contrats", "expediteur", "ligne", "partis"]);
    // Un courriel par salarie : Brioche (ses deux contrats dans un seul PDF) et Seraphin.
    assert_eq!(r["partis"], json!(["fanchon.brioche@exemple.invalid", "valentin.seraphin@exemple.invalid"]));
    assert_eq!(faux.partis(), 2);
    let contrats = r["contrats"].as_array().unwrap();
    assert_eq!(contrats.len(), 3);
    let numeros: Vec<&str> = contrats.iter().map(|c| c["numero"].as_str().unwrap()).collect();
    assert_eq!(numeros, ["BR1.26.05", "BR1.26.06", "SE2.26.05"]);
    assert_eq!(contrats[0]["fichier"], json!("Contrats_BRIOCHE_BR1.26.05+BR1.26.06_Octobre2026.pdf"));
    assert_eq!(contrats[2]["fichier"], json!("Contrat_SERAPHIN_SE2.26.05_Octobre2026.pdf"));
    let code = contrats[0]["code"].as_str().unwrap();
    assert!(code.starts_with("SIG-") && code.len() == 10 && code[4..].chars().all(|c| "ABCDEFGHJKMNPQRSTUVWXYZ23456789".contains(c)), "{}", code);
    assert_eq!(code, contrats[1]["code"].as_str().unwrap(), "un seul code par courriel");
    assert_ne!(code, contrats[2]["code"].as_str().unwrap());
    assert_eq!(contrats[0]["adresse"], json!("fanchon.brioche@exemple.invalid"));
    assert_eq!(contrats[0]["message"], json!("<faux-0@exemple.invalid>"));
    assert_eq!(contrats[2]["message"], json!("<faux-1@exemple.invalid>"));
    assert_eq!(contrats[0]["relance"], json!(false));
    assert!(contrats[0]["empreinte"].as_str().unwrap().len() == 16);
    assert!(contrats[0]["clauses"]["contrat.debut"].as_str().unwrap() == "13/10/2026");
    assert_eq!(contrats[0]["mois"], json!("2026-10"));
    assert!(contrats[0]["spectacle"].is_string());

    // Les PDF et leur description gardee, a cote.
    let envoi = racine.join("sorties/2026-10/envoi");
    assert_eq!(
        noms(&envoi),
        [
            "Contrat_SERAPHIN_SE2.26.05_Octobre2026.description.json",
            "Contrat_SERAPHIN_SE2.26.05_Octobre2026.pdf",
            "Contrats_BRIOCHE_BR1.26.05+BR1.26.06_Octobre2026.description.json",
            "Contrats_BRIOCHE_BR1.26.05+BR1.26.06_Octobre2026.pdf"
        ]
    );
    let gardee = lire(&envoi, "Contrat_SERAPHIN_SE2.26.05_Octobre2026.description.json");
    assert_eq!(gardee["format"], json!(3));
    assert_eq!(gardee["pdf_sha256"].as_str().unwrap().len(), 64);
    assert_eq!(gardee["description"]["documents"][0]["modele"]["valeurs"]["contrat.numero"], json!("SE2.26.05"));
    // La description garde l'ordre de `contrat.marqueurs` : l'employeur d'abord.
    assert_eq!(gardee["description"]["documents"][0]["modele"]["valeurs"].as_object().unwrap().keys().next().unwrap(), "employeur.raison_sociale");

    // Le courriel : objet, bouton, code, piece jointe.
    let m = faux.message(0);
    assert_eq!(m.entete("To"), "fanchon.brioche@exemple.invalid");
    // Les deux contrats de Brioche ont le MEME spectacle : l'objet le nomme (le pluriel vaut pour des spectacles differents).
    assert_eq!(m.entete("Subject"), "Contrat d'engagement Les Pupazzi - octobre 2026");
    assert_eq!(m.pieces(), ["Contrats_BRIOCHE_BR1.26.05+BR1.26.06_Octobre2026.pdf"]);
    let (texte, html) = m.textes();
    assert!(texte.starts_with("Bonjour Fanchon,") && texte.contains("2 contrats"), "{}", texte);
    let serre = texte.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(serre.contains("- Les Pupazzi, du 13/10/2026 au 22/10/2026 - Les Pupazzi, du 27/10/2026 au 29/10/2026"), "{}", texte);
    assert!(texte.contains(&format!("suivi de ce code : {}", code)));
    assert!(texte.contains("George"), "le signataire : {}", texte);
    assert!(html.contains(">Je signe</a>") && html.contains("href=\"mailto:boite@exemple.invalid?subject=Re%3A%20Contrat%20d%27engagement"), "{}", html);
    assert!(html.contains(&format!("<b>{}</b>", code)));
    assert_eq!(faux.message(1).entete("Subject"), "Contrat d'engagement Les Pupazzi - octobre 2026");

    // Le journal : une ligne, avec les contrats et leur code ; le numero parti est ecrit dans la saisie et le compteur passe au-dessus.
    let j = journal(&racine);
    assert_eq!(j.len(), j0.len() + 1);
    let l = j.last().unwrap();
    assert_eq!((l["quoi"].as_str(), l["accepte"].as_bool(), l["mois"].as_str()), (Some("contrat-signature"), Some(true), Some("2026-10")));
    // Retours Windows, B : le journal compte les CONTRATS (trois : deux de Brioche, un de Seraphin), pas les courriels (deux).
    assert_eq!(l["reponse"], json!("3 contrat(s) envoye(s) a la signature"));
    assert_eq!(l["destinataire"], json!("fanchon.brioche@exemple.invalid, valentin.seraphin@exemple.invalid"));
    assert_eq!(l["detail"]["contrats"], r["contrats"]);
    assert_eq!(l["detail"]["expediteur"], json!("boite@exemple.invalid"));
    assert_eq!(l["detail"]["relance"], json!(false));
    let saisie = crate::outils::saisie(&racine, "2026-10");
    let ecrits: Vec<&str> = saisie["engagements"].as_array().unwrap().iter().map(|e| e["numero"].as_str().unwrap()).collect();
    assert_eq!(ecrits, ["BR1.26.05", "SE2.26.05", "BR1.26.06"]);
    let compteurs = lire(&racine, "config/compteurs.json");
    // Les numeros justes : seuls les compteurs de contrat avancent, jusqu'au dernier numero ecrit ; `bulletin` ne bouge pas.
    let avant: Value = serde_json::from_slice(&compteurs0).unwrap();
    assert_eq!(compteurs["2026"]["contrat_BRIOCHE"], json!(6));
    assert_eq!(compteurs["2026"]["contrat_SERAPHIN"], json!(5));
    assert_eq!(compteurs["2026"]["bulletin"], avant["2026"]["bulletin"]);
    assert_eq!(compteurs["2026"]["contrat_LEMERCIER"], avant["2026"]["contrat_LEMERCIER"]);
    assert_eq!(compteurs["2025"], avant["2025"]);
    assert!(racine.join("engagements/avant").exists());

    // Le contrat est parti : il se relance (objet « Rappel »), il ne se renvoie pas sans le dire.
    let relance = avec(&racine, "demander-signature", json!({"mois": "2026-10", "motdepasse": MOT_DE_PASSE, "relance": true, "contrats": [{"salarie": "SERAPHIN", "debut": "2026-10-13"}]}), &faux).unwrap();
    assert_eq!(relance["contrats"][0]["relance"], json!(true));
    assert_eq!(faux.message(2).entete("Subject"), "Rappel - Contrat d'engagement Les Pupazzi - octobre 2026");
    let derniere = journal(&racine).last().unwrap().clone();
    assert_eq!(derniere["reponse"], json!("1 contrat(s) relance(s) a la signature"));
    assert_eq!(derniere["detail"]["relance"], json!(true));

    // Le contrat de Brioche du 27/10 n'a pas commence : il peut etre refait en entier, il annule et remplace.
    let remplace = avec(&racine, "demander-signature", json!({"mois": "2026-10", "motdepasse": MOT_DE_PASSE, "remplace": true, "contrats": [{"salarie": "BRIOCHE", "debut": "2026-10-27"}]}), &faux).unwrap();
    assert_eq!(remplace["contrats"][0]["acte"], json!("remplace"));
    assert_eq!(remplace["contrats"][0]["remplace"], json!("BR1.26.06"));
    assert_eq!(remplace["contrats"][0]["fichier"], json!("Contrat_BRIOCHE_BR1.26.06_Octobre2026_remplace.pdf"));
    assert!(faux.message(3).textes().0.contains("annule et remplace"));
    assert!(racine.join("sorties/2026-10/envoi/Contrat_BRIOCHE_BR1.26.06_Octobre2026_remplace.pdf").exists());
    // Celui du 13/10 n'a pas commence non plus (on est le 03/10) : il se remplace aussi.
    assert!(avec(&racine, "demander-signature", json!({"mois": "2026-10", "motdepasse": MOT_DE_PASSE, "remplace": true, "contrats": [{"salarie": "BRIOCHE", "debut": "2026-10-13"}]}), &faux).is_ok());
}

#[test]
fn demander_signature_qui_echoue_le_note_sans_rien_reserver() {
    let _t = tour();
    let racine = compagnie("signature-echec", false);
    poser_le_mot_de_passe(&racine);
    octobre_a_signer(&racine);
    let saisie0 = std::fs::read(racine.join("engagements/2026-10.json")).unwrap();
    let compteurs0 = std::fs::read(racine.join("config/compteurs.json")).unwrap();
    let j0 = journal(&racine);
    // Le second courriel est refuse : le premier est parti.
    let faux = refuse_le(1);
    let (_, m) = phrase(avec(&racine, "demander-signature", json!({"mois": "2026-10", "motdepasse": MOT_DE_PASSE}), &faux));
    assert!(m.contains("ATTENTION, 1 message(s) etaient deja partis : fanchon.brioche@exemple.invalid"), "{}", m);
    // Le journal garde le refus avec les contrats qui auraient du partir ; aucun numero n'est reserve.
    let j = journal(&racine);
    assert_eq!(j.len(), j0.len() + 1);
    let l = j.last().unwrap();
    assert_eq!((l["quoi"].as_str(), l["accepte"].as_bool(), l["destinataire"].as_str()), (Some("contrat-signature"), Some(false), Some("fanchon.brioche@exemple.invalid")));
    assert_eq!(l["detail"]["echec_a"], json!("valentin.seraphin@exemple.invalid"));
    assert_eq!(l["detail"]["partis"], json!(["fanchon.brioche@exemple.invalid"]));
    assert_eq!(l["detail"]["contrats"].as_array().unwrap().len(), 3);
    assert!(l["detail"]["contrats"][0].get("message").is_none());
    assert_eq!(std::fs::read(racine.join("engagements/2026-10.json")).unwrap(), saisie0);
    assert_eq!(std::fs::read(racine.join("config/compteurs.json")).unwrap(), compteurs0);
}

#[test]
fn demander_signature_dans_une_fictive_joue_le_courriel() {
    let _t = tour();
    let racine = compagnie("signature-fictive", true);
    let r = telle_quelle(&racine, "demander-signature", json!({"mois": "2026-10"})).unwrap();
    assert_eq!(r["partis"], json!(["fanchon.brioche@exemple.invalid"]));
    assert_eq!(r["expediteur"], json!("direction@demo-3090-petite.invalid"));
    let emls: Vec<String> = noms(&racine.join("courriels-simules")).into_iter().filter(|n| n.ends_with(".eml")).collect();
    assert_eq!(emls.len(), 1);
    let m = Mail(std::fs::read(racine.join("courriels-simules").join(&emls[0])).unwrap());
    assert!(m.textes().1.contains("Je signe"));
    assert!(racine.join("sorties/2026-10/envoi/Contrat_BRIOCHE_BR1.26.05_Octobre2026.pdf").exists());
}

/// Le gabarit de contrat de la compagnie (`gabarits/contrat-cddu.modele`), recopie du modele livre SANS le marqueur
/// des defraiements : `remuneration.detail` ne cite que le brut, ce qui est verse en plus n'est que dans
/// `{{remuneration.hors_brut}}`.
fn gabarit_sans_hors_brut(racine: &Path) -> PathBuf {
    let livre = std::fs::read_to_string(socle::fichiers::dossier_du_code().join("modeles").join("contrat-cddu.modele")).unwrap();
    assert!(livre.contains("{{remuneration.hors_brut}}"));
    let gabarit = racine.join("gabarits").join("contrat-cddu.modele");
    std::fs::create_dir_all(gabarit.parent().unwrap()).unwrap();
    std::fs::write(&gabarit, livre.replace("{{remuneration.hors_brut}}", "")).unwrap();
    gabarit
}

/// REGLE 1 : un gabarit de compagnie qui n'a pas le marqueur des defraiements ne fait pas partir un contrat qui les
/// tait (octobre de la petite : 4 cachets et 4 defraiements au forfait) -- le contrat ne s'ecrit pas, et le refus dit
/// quel fichier et quel marqueur. Sans defraiement, le meme gabarit passe comme avant.
#[test]
fn un_gabarit_sans_le_marqueur_des_defraiements_refuse_de_les_taire() {
    let _t = tour();
    let racine = compagnie("gabarit-sans-hors-brut", true);
    let gabarit = gabarit_sans_hors_brut(&racine);
    let r = telle_quelle(&racine, "demander-signature", json!({"mois": "2026-10"}));
    if r.is_ok() {
        // Ce que le contrat parti dit : sa description gardee, ses marqueurs remplis.
        let gardee = lire(&racine, "sorties/2026-10/envoi/Contrat_BRIOCHE_BR1.26.05_Octobre2026.description.json");
        let modele = &gardee["description"]["documents"][0]["modele"];
        let valeurs: fabrique_pdf::modele::Valeurs =
            modele["valeurs"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string())).collect();
        let lu = fabrique_pdf::modele::lire_modele(modele["texte"].as_str().unwrap(), "gabarit").unwrap();
        let remuneration = &lu.articles.iter().find(|(titre, _)| titre == "Rémunération").unwrap().1[0];
        let article = fabrique_pdf::modele::texte_brut(remuneration, &valeurs).unwrap();
        panic!("contrat produit sans ses defraiements (hors_brut = {:?}) : {}", valeurs.get("remuneration.hors_brut"), article);
    }
    let (_, message) = phrase(r);
    assert!(message.contains(gabarit.to_str().unwrap()), "le fichier du modele est nomme : {}", message);
    assert!(message.contains("{{remuneration.hors_brut}}"), "le marqueur a ajouter est nomme : {}", message);
    assert!(!racine.join("sorties/2026-10/envoi/Contrat_BRIOCHE_BR1.26.05_Octobre2026.pdf").exists(), "aucun contrat ecrit");
    assert!(noms(&racine.join("courriels-simules")).iter().all(|n| !n.ends_with(".eml")), "rien n'est parti");

    // Le meme gabarit, un mois sans defraiement : rien a taire, le contrat part comme avant.
    let racine = compagnie("gabarit-sans-hors-brut-ni-defraiement", true);
    gabarit_sans_hors_brut(&racine);
    let mut s = crate::outils::saisie(&racine, "2026-10");
    s["engagements"][0]["lignes"].as_array_mut().unwrap().retain(|l| l["rubrique"] != json!("defraiement_forfait"));
    crate::outils::poser_saisie(&racine, "2026-10", &s);
    let r = telle_quelle(&racine, "demander-signature", json!({"mois": "2026-10"})).unwrap();
    assert_eq!(r["partis"], json!(["fanchon.brioche@exemple.invalid"]));
    assert!(racine.join("sorties/2026-10/envoi/Contrat_BRIOCHE_BR1.26.05_Octobre2026.pdf").exists());
}

/// Octobre a quatre engagements : Brioche deux fois (le 13 et le 27), Seraphin, Lemercier. `deux_spectacles` : le
/// second contrat de Brioche est un autre spectacle.
fn octobre_a_quatre(racine: &Path, deux_spectacles: bool) {
    let mut s = crate::outils::saisie(racine, "2026-10");
    let eng = s["engagements"][0].clone();
    let mut seraphin = eng.clone();
    seraphin["salarie"] = json!("Seraphin");
    let mut deuxieme = eng.clone();
    deuxieme["debut"] = json!("2026-10-27");
    deuxieme["fin"] = json!("2026-10-29");
    deuxieme["date_paiement"] = json!("2026-10-29");
    deuxieme["lignes"][0]["dates"] = json!(["2026-10-27", "2026-10-29"]);
    deuxieme["lignes"][0]["quantite"] = json!(2);
    deuxieme["lignes"][1]["quantite"] = json!(2);
    if deux_spectacles {
        deuxieme["spectacle"] = json!("256Z00920011");
    }
    let mut lemercier = eng.clone();
    lemercier["salarie"] = json!("Lemercier");
    s["engagements"] = json!([eng, seraphin, deuxieme, lemercier]);
    crate::outils::poser_saisie(racine, "2026-10", &s);
}

fn compteurs_2026(racine: &Path) -> Value {
    lire(racine, "config/compteurs.json")["2026"].clone()
}

#[test]
fn demander_signature_n_avance_que_les_compteurs_de_contrat_jusqu_au_dernier_numero_envoye() {
    let _t = tour();
    // Decision de Leandre du 07/10/2026 : seuls les compteurs de contrat avancent, jusqu'au dernier numero parti.
    let racine = compagnie("signature-compteurs", false);
    poser_le_mot_de_passe(&racine);
    octobre_a_quatre(&racine, false);
    let avant_tout = lire(&racine, "config/compteurs.json");
    let avant = avant_tout["2026"].clone();
    assert_eq!((avant["bulletin"].clone(), avant["contrat_BRIOCHE"].clone(), avant["contrat_SERAPHIN"].clone(), avant["contrat_LEMERCIER"].clone()), (json!(12), json!(4), json!(4), json!(4)));
    let faux = Faux::nouveau();
    let r = avec(&racine, "demander-signature", json!({"mois": "2026-10", "motdepasse": MOT_DE_PASSE}), &faux).unwrap();
    let numeros: Vec<&str> = r["contrats"].as_array().unwrap().iter().map(|c| c["numero"].as_str().unwrap()).collect();
    assert_eq!(numeros, ["BR1.26.05", "BR1.26.06", "SE2.26.05", "LE3.26.05"]);
    let apres = lire(&racine, "config/compteurs.json");
    assert_eq!((apres["2026"]["bulletin"].clone(), apres["2026"]["contrat_BRIOCHE"].clone(), apres["2026"]["contrat_SERAPHIN"].clone(), apres["2026"]["contrat_LEMERCIER"].clone()), (json!(12), json!(6), json!(5), json!(5)));
    // Tout le reste du fichier est intact.
    let mut attendu = avant_tout.clone();
    attendu["2026"]["contrat_BRIOCHE"] = json!(6);
    attendu["2026"]["contrat_SERAPHIN"] = json!(5);
    attendu["2026"]["contrat_LEMERCIER"] = json!(5);
    assert_eq!(apres, attendu);
    // Une relance ne bouge plus rien.
    let octets = std::fs::read(racine.join("config/compteurs.json")).unwrap();
    avec(&racine, "demander-signature", json!({"mois": "2026-10", "motdepasse": MOT_DE_PASSE, "relance": true}), &faux).unwrap();
    assert_eq!(std::fs::read(racine.join("config/compteurs.json")).unwrap(), octets);

    // Un numero deja porte par la saisie n'est pas reecrit : son compteur ne bouge pas.
    let racine = compagnie("signature-compteurs-porte", false);
    poser_le_mot_de_passe(&racine);
    octobre_a_quatre(&racine, false);
    let mut saisie = crate::outils::saisie(&racine, "2026-10");
    saisie["engagements"][1]["numero"] = json!("SE2.26.09");
    crate::outils::poser_saisie(&racine, "2026-10", &saisie);
    avec(&racine, "demander-signature", json!({"mois": "2026-10", "motdepasse": MOT_DE_PASSE}), &Faux::nouveau()).unwrap();
    let c = compteurs_2026(&racine);
    assert_eq!((c["bulletin"].clone(), c["contrat_SERAPHIN"].clone(), c["contrat_BRIOCHE"].clone()), (json!(12), json!(4), json!(6)));

    // Un compteur deja haut : les numeros partent au-dessus de lui, le compteur s'arrete au dernier ; `bulletin` ne bouge pas.
    let racine = compagnie("signature-compteurs-haut", false);
    poser_le_mot_de_passe(&racine);
    octobre_a_quatre(&racine, false);
    let mut c = lire(&racine, "config/compteurs.json");
    c["2026"]["contrat_BRIOCHE"] = json!(40);
    c["2026"]["bulletin"] = json!(77);
    ecrire(&racine, "config/compteurs.json", &c);
    avec(&racine, "demander-signature", json!({"mois": "2026-10", "motdepasse": MOT_DE_PASSE}), &Faux::nouveau()).unwrap();
    let c = compteurs_2026(&racine);
    // Les contrats de Brioche partent en 41 et 42 : le compteur va jusqu'au dernier numero ecrit.
    assert_eq!((c["bulletin"].clone(), c["contrat_BRIOCHE"].clone(), c["contrat_SERAPHIN"].clone()), (json!(77), json!(42), json!(5)));

    // Sans `compteurs.json` : il est cree avec les seuls compteurs de contrat.
    let racine = compagnie("signature-sans-compteurs", false);
    poser_le_mot_de_passe(&racine);
    octobre_a_quatre(&racine, false);
    std::fs::remove_file(racine.join("config/compteurs.json")).unwrap();
    avec(&racine, "demander-signature", json!({"mois": "2026-10", "motdepasse": MOT_DE_PASSE}), &Faux::nouveau()).unwrap();
    assert_eq!(lire(&racine, "config/compteurs.json"), json!({"2026": {"contrat_BRIOCHE": 2, "contrat_SERAPHIN": 1, "contrat_LEMERCIER": 1}}));
}

#[test]
fn demander_signature_selection_deux_spectacles_relance_caduque_et_contrat_commence() {
    let _t = tour_seul();
    let racine = compagnie("signature-selection", false);
    poser_le_mot_de_passe(&racine);
    octobre_a_quatre(&racine, true);
    let faux = Faux::nouveau();
    // L'ancienne selection par salarie : seul Seraphin part.
    let r = avec(&racine, "demander-signature", json!({"mois": "2026-10", "motdepasse": MOT_DE_PASSE, "salaries": ["SERAPHIN"]}), &faux).unwrap();
    assert_eq!(r["partis"], json!(["valentin.seraphin@exemple.invalid"]));
    assert_eq!(r["contrats"].as_array().unwrap().len(), 1);
    assert_eq!(faux.partis(), 1);
    // Deux spectacles pour Brioche : l'objet est au pluriel, sans nom de spectacle.
    let r = avec(&racine, "demander-signature", json!({"mois": "2026-10", "motdepasse": MOT_DE_PASSE, "salaries": ["BRIOCHE"]}), &faux).unwrap();
    assert_eq!(r["contrats"].as_array().unwrap().len(), 2);
    assert_eq!(faux.message(1).entete("Subject"), "Contrats d'engagement - octobre 2026");
    // Le contrat du 13/10 a commence le 14/10 : il ne s'annule pas, il se modifie par un avenant.
    let (_, m) = phrase(avec_le(
        &racine,
        "2026-10-14",
        "demander-signature",
        json!({"mois": "2026-10", "motdepasse": MOT_DE_PASSE, "remplace": true, "contrats": [{"salarie": "BRIOCHE", "debut": "2026-10-13"}]}),
        &faux,
    ));
    assert!(m.contains("13/10/2026") && m.contains("avenant"), "un contrat commence se modifie par un avenant : {}", m);
    // La paie change apres l'envoi : le contrat n'est plus le bon, il se renvoie a jour, il ne se relance pas.
    let mut saisie = crate::outils::saisie(&racine, "2026-10");
    saisie["engagements"][0]["lignes"][0]["lieu"] = json!("Ailleurs");
    crate::outils::poser_saisie(&racine, "2026-10", &saisie);
    let partis = faux.partis();
    let (_, m) = phrase(avec(&racine, "demander-signature", json!({"mois": "2026-10", "motdepasse": MOT_DE_PASSE, "relance": true, "contrats": [{"salarie": "BRIOCHE", "debut": "2026-10-13"}]}), &faux));
    assert!(m.contains("a change depuis son envoi : il se renvoie a jour, il ne se relance pas. Rien n'est parti."), "{}", m);
    assert_eq!(faux.partis(), partis);
}

/// Retours Windows, B : Brioche a DEUX contrats qui commencent le meme jour (le 13/10), le second a un autre lieu.
fn octobre_a_deux_le_meme_jour(racine: &Path) {
    let mut s = crate::outils::saisie(racine, "2026-10");
    let eng = s["engagements"][0].clone();
    let mut jumeau = eng.clone();
    jumeau["lignes"][0]["lieu"] = json!("Ailleurs");
    s["engagements"] = json!([eng, jumeau]);
    crate::outils::poser_saisie(racine, "2026-10", &s);
}

#[test]
fn demander_signature_distingue_deux_contrats_du_meme_jour_et_compte_des_contrats() {
    let _t = tour();
    // Un seul des deux est coche (par son numero) : un seul part, un seul PDF, le journal dit « 1 contrat ».
    let racine = compagnie("signature-jumeaux", false);
    poser_le_mot_de_passe(&racine);
    octobre_a_deux_le_meme_jour(&racine);
    let apercu = apercu(&racine, "signature-apercu", json!({"mois": "2026-10"}));
    let numeros: Vec<&str> = apercu["contrats"].as_array().unwrap().iter().map(|c| c["numero"].as_str().unwrap()).collect();
    assert_eq!(numeros, ["BR1.26.05", "BR1.26.06"], "la page recoit de quoi les separer");
    let faux = Faux::nouveau();
    let r = avec(&racine, "demander-signature", json!({"mois": "2026-10", "motdepasse": MOT_DE_PASSE, "contrats": [{"salarie": "BRIOCHE", "debut": "2026-10-13", "numero": "BR1.26.06"}]}), &faux).unwrap();
    assert_eq!(r["contrats"].as_array().unwrap().len(), 1, "seul le contrat coche part");
    assert_eq!(r["contrats"][0]["numero"], json!("BR1.26.06"));
    assert_eq!(r["contrats"][0]["fichier"], json!("Contrat_BRIOCHE_BR1.26.06_Octobre2026.pdf"));
    assert_eq!(faux.partis(), 1);
    assert_eq!(faux.message(0).pieces(), ["Contrat_BRIOCHE_BR1.26.06_Octobre2026.pdf"]);
    assert_eq!(noms(&racine.join("sorties/2026-10/envoi")).iter().filter(|n| n.ends_with(".pdf")).count(), 1);
    let l = journal(&racine).last().unwrap().clone();
    assert_eq!(l["reponse"], json!("1 contrat(s) envoye(s) a la signature"));
    // Le numero parti s'ecrit sur CE contrat-la, pas sur son jumeau qui n'est pas parti.
    let saisie = crate::outils::saisie(&racine, "2026-10");
    assert!(saisie["engagements"][0].get("numero").is_none(), "{}", saisie["engagements"][0]);
    assert_eq!(saisie["engagements"][1]["numero"], json!("BR1.26.06"));
    // Un numero que la saisie ne porte pas : refus, rien ne part.
    let (_, m) = phrase(avec(&racine, "demander-signature", json!({"mois": "2026-10", "motdepasse": MOT_DE_PASSE, "contrats": [{"salarie": "BRIOCHE", "debut": "2026-10-13", "numero": "BR1.26.99"}]}), &faux));
    assert!(m.contains("BR1.26.99"), "le refus nomme le numero : {}", m);
    assert_eq!(faux.partis(), 1);

    // Les deux cochés : un courriel, un PDF, et le journal compte DEUX contrats (pas un courriel).
    let racine = compagnie("signature-jumeaux-deux", false);
    poser_le_mot_de_passe(&racine);
    octobre_a_deux_le_meme_jour(&racine);
    let faux = Faux::nouveau();
    let r = avec(&racine, "demander-signature", json!({"mois": "2026-10", "motdepasse": MOT_DE_PASSE}), &faux).unwrap();
    assert_eq!((faux.partis(), r["contrats"].as_array().unwrap().len()), (1, 2));
    assert_eq!(journal(&racine).last().unwrap()["reponse"], json!("2 contrat(s) envoye(s) a la signature"));
    let empreintes: Vec<&str> = r["contrats"].as_array().unwrap().iter().map(|c| c["empreinte"].as_str().unwrap()).collect();
    assert_ne!(empreintes[0], empreintes[1], "chaque contrat porte sa propre empreinte");
}

// ================================================================== avenant-envoyer, avenants-envoyer

/// Octobre a signer et deja parti ; puis le lieu du premier contrat de Brioche change : il faut un avenant.
fn contrats_partis_puis_modifies(racine: &Path, faux: &Faux) {
    octobre_a_signer(racine);
    avec(racine, "demander-signature", json!({"mois": "2026-10", "motdepasse": MOT_DE_PASSE}), faux).unwrap();
    let mut s = crate::outils::saisie(racine, "2026-10");
    s["engagements"][0]["lignes"][0]["lieu"] = json!("Theatre de la Ville");
    s["engagements"][1]["lignes"][0]["lieu"] = json!("Salle des fetes de Murat");
    crate::outils::poser_saisie(racine, "2026-10", &s);
}

#[test]
fn les_avenants_partent_un_courriel_par_salarie_avec_un_pdf_chacun() {
    let _t = tour();
    let racine = compagnie("avenants", false);
    poser_le_mot_de_passe(&racine);
    let faux = Faux::nouveau();
    contrats_partis_puis_modifies(&racine, &faux);
    let partis_avant = faux.partis();

    // Ce que l'apercu montre : un article modifie, le numero 1.
    let a = apercu(&racine, "avenant-apercu", json!({"salarie": "BRIOCHE", "debut": "2026-10-13"}));
    assert_eq!(a["numero"], json!(1));
    assert!(!a["modifications"].as_array().unwrap().is_empty());
    assert_eq!(a["contrat"], json!("BR1.26.05"));

    // Sans preuve, rien : l'avenant n'est pas ecrit, le journal ne bouge pas.
    let j0 = journal(&racine);
    let sans = Faux::nouveau();
    assert_eq!(phrase(avec(&racine, "avenant-envoyer", json!({"salarie": "BRIOCHE", "debut": "2026-10-13"}), &sans)).0, "PreuveInsuffisante");
    assert_eq!(phrase(avec(&racine, "avenants-envoyer", json!({"avenants": [{"salarie": "BRIOCHE", "debut": "2026-10-13"}], "motdepasse": "non"}), &sans)).0, "PreuveInsuffisante");
    assert_eq!(sans.partis(), 0);
    assert_eq!(journal(&racine), j0);
    assert!(!noms(&racine.join("sorties/2026-10/envoi")).iter().any(|n| n.starts_with("Avenant")));

    // Un seul avenant : la reponse (partis, fichier, numero, ligne).
    let r = avec(&racine, "avenant-envoyer", json!({"salarie": "BRIOCHE", "debut": "2026-10-13", "motdepasse": MOT_DE_PASSE}), &faux).unwrap();
    let mut cles: Vec<&str> = r.as_object().unwrap().keys().map(String::as_str).collect();
    cles.sort();
    assert_eq!(cles, ["fichier", "ligne", "numero", "partis"]);
    assert_eq!(r["fichier"], json!("Avenant1_BRIOCHE_BR1.26.05_Octobre2026.pdf"));
    assert_eq!(r["numero"], json!(1));
    assert_eq!(r["partis"], json!(["fanchon.brioche@exemple.invalid"]));
    let m = faux.message(partis_avant);
    assert!(m.entete("Subject").starts_with("Avenant n° 1 - Contrat d'engagement Les Pupazzi - octobre 2026"), "{}", m.entete("Subject"));
    assert_eq!(m.pieces(), ["Avenant1_BRIOCHE_BR1.26.05_Octobre2026.pdf"]);
    assert!(m.textes().1.contains("Je signe"));
    let envoi = racine.join("sorties/2026-10/envoi");
    assert!(envoi.join("Avenant1_BRIOCHE_BR1.26.05_Octobre2026.pdf").exists());
    assert!(envoi.join("Avenant1_BRIOCHE_BR1.26.05_Octobre2026.description.json").exists());
    let l = journal(&racine).last().unwrap().clone();
    assert_eq!((l["quoi"].as_str(), l["accepte"].as_bool(), l["mois"].as_str()), (Some("contrat-signature"), Some(true), Some("2026-10")));
    assert_eq!(l["detail"]["acte"], json!("avenant"));
    assert_eq!(l["detail"]["contrats"][0]["acte"], json!("avenant"));
    assert_eq!(l["detail"]["contrats"][0]["avenant"], json!(1));
    assert_eq!(l["detail"]["contrats"][0]["effet"], json!("2026-10-13"));
    assert!(l["detail"]["contrats"][0]["code"].as_str().unwrap().starts_with("SIG-"));
    assert_eq!(l["detail"]["contrats"][0]["message"], json!(format!("<faux-{}@exemple.invalid>", partis_avant)));
    assert!(l["reponse"].as_str().unwrap().starts_with("avenant n° 1 au contrat BR1.26.05"));
    assert_eq!(r["ligne"], l);

    // Un avenant envoye compte deja : le suivant porte le numero 2 (l'apercu part des avenants signes seulement :
    // le numero 1 n'est pas signe, il reste 1) ; les deux contrats de Brioche et celui de Seraphin partent ensemble :
    // un courriel par salarie.
    let r = avec(
        &racine,
        "avenants-envoyer",
        json!({"avenants": [{"salarie": "SERAPHIN", "debut": "2026-10-13"}, {"salarie": "BRIOCHE", "debut": "2026-10-13", "effet": "2026-10-14"}], "motdepasse": MOT_DE_PASSE}),
        &faux,
    )
    .unwrap();
    let mut cles: Vec<&str> = r.as_object().unwrap().keys().map(String::as_str).collect();
    cles.sort();
    assert_eq!(cles, ["avenants", "partis"]);
    assert_eq!(r["partis"], json!(["valentin.seraphin@exemple.invalid", "fanchon.brioche@exemple.invalid"]));
    assert_eq!(r["avenants"].as_array().unwrap().len(), 2);
    assert_eq!(r["avenants"][0]["salarie"], json!("SERAPHIN"));
    assert_eq!(r["avenants"][1]["debut"], json!("2026-10-13"));
    let j = journal(&racine);
    let derniers = &j[j.len() - 2..];
    assert_eq!(derniers[1]["detail"]["contrats"][0]["effet"], json!("2026-10-14"));
    assert_eq!(derniers[0]["destinataire"], json!("valentin.seraphin@exemple.invalid"));

    // Refus : rien de coche, une date illisible, un contrat qui n'est pas parti.
    let (_, m) = phrase(avec(&racine, "avenants-envoyer", json!({"avenants": [], "motdepasse": MOT_DE_PASSE}), &faux));
    assert!(m.contains("avenant"), "{}", m);
    let (_, m) = phrase(avec(&racine, "avenant-envoyer", json!({"salarie": "BRIOCHE", "debut": "2026-10-13", "effet": "demain", "motdepasse": MOT_DE_PASSE}), &faux));
    assert!(m.contains("'demain'"), "le refus nomme la date lue : {}", m);
    let (_, m) = phrase(avec(&racine, "avenant-envoyer", json!({"salarie": "BRIOCHE", "debut": "2026-11-02", "motdepasse": MOT_DE_PASSE}), &faux));
    assert!(m.starts_with("Aucun contrat n'est parti à la signature pour BRIOCHE du 02/11/2026"), "{}", m);
}

#[test]
fn les_avenants_qui_echouent_le_notent_et_gardent_ce_qui_est_deja_parti() {
    let _t = tour();
    let racine = compagnie("avenants-echec", false);
    poser_le_mot_de_passe(&racine);
    let ok = Faux::nouveau();
    contrats_partis_puis_modifies(&racine, &ok);
    let j0 = journal(&racine).len();
    // Le courriel du second salarie est refuse : celui du premier est parti, et son avenant est au journal.
    let faux = refuse_le(1);
    let (classe, m) = phrase(avec(
        &racine,
        "avenants-envoyer",
        json!({"avenants": [{"salarie": "BRIOCHE", "debut": "2026-10-13"}, {"salarie": "SERAPHIN", "debut": "2026-10-13"}], "motdepasse": MOT_DE_PASSE}),
        &faux,
    ));
    assert_eq!(classe, "ErreurCourriel", "{}", m);
    assert_eq!(faux.partis(), 1);
    let j = journal(&racine);
    assert_eq!(j.len(), j0 + 2);
    assert_eq!((j[j0]["accepte"].as_bool(), j[j0]["detail"]["acte"].as_str()), (Some(true), Some("avenant")));
    assert_eq!((j[j0 + 1]["accepte"].as_bool(), j[j0 + 1]["destinataire"].as_str()), (Some(false), Some("")));
    assert_eq!(j[j0 + 1]["detail"]["echec_a"], json!("valentin.seraphin@exemple.invalid"));
    assert_eq!(j[j0 + 1]["detail"]["contrats"][0]["salarie"], json!("SERAPHIN"));
}

// ================================================================== dossier-envoyer

#[test]
fn dossier_envoyer_refait_le_dossier_puis_l_envoie_a_chaque_salarie() {
    let _t = tour();
    let racine = compagnie("dossier", false);
    poser_le_mot_de_passe(&racine);
    let charge = json!({"salaries": ["BRIOCHE", "SERAPHIN"], "mois": "2026-08", "categories": ["bulletins"], "motdepasse": MOT_DE_PASSE});
    let j0 = journal(&racine);
    let sans = Faux::nouveau();
    // Sans preuve : le dossier est refait (ecrit) avant, mais rien ne part et rien n'est note.
    let mut sans_mdp = charge.clone();
    sans_mdp["motdepasse"] = json!("non");
    assert_eq!(phrase(avec(&racine, "dossier-envoyer", sans_mdp, &sans)).0, "PreuveInsuffisante");
    assert_eq!(sans.partis(), 0);
    assert_eq!(journal(&racine), j0);

    let faux = Faux::nouveau();
    let r = avec(&racine, "dossier-envoyer", charge.clone(), &faux).unwrap();
    // La reponse : le dossier (`dossier_json`) et, sous `envoi`, ce qui est parti.
    assert_eq!(r["salaries"], json!(["BRIOCHE", "SERAPHIN"]));
    assert_eq!(r["periode"], json!("aout 2026"));
    assert_eq!(r["categories"], json!(["bulletins"]));
    assert_eq!(r["fichiers"].as_array().unwrap().len(), 2);
    assert!(r["fichiers"][0].as_str().unwrap().ends_with("Dossier_BRIOCHE_2026-08_Bulletins.pdf"), "{}", r["fichiers"][0]);
    assert!(r["libelles"]["bulletins"].is_string());
    let envoi = &r["envoi"];
    assert_eq!(envoi["partis"], json!(["fanchon.brioche@exemple.invalid", "valentin.seraphin@exemple.invalid"]));
    assert_eq!(envoi["messages"], json!(2));
    assert_eq!(envoi["expediteur"], json!("boite@exemple.invalid"));
    assert_eq!(faux.partis(), 2);
    let m = faux.message(0);
    assert_eq!(m.entete("Subject"), "Ton dossier de salarié — aout 2026");
    assert_eq!(m.pieces(), ["Dossier_BRIOCHE_2026-08_Bulletins.pdf"]);
    assert!(m.textes().0.contains("(aout 2026) : bulletins de paie"), "{}", m.textes().0);
    assert!(racine.join("exports/dossiers/Dossier_BRIOCHE_2026-08_Bulletins.pdf").exists());
    // Le journal : sans mois ni reponse, avec la periode et les pieces.
    let l = journal(&racine).last().unwrap().clone();
    assert_eq!((l["quoi"].as_str(), l["accepte"].as_bool(), l["mois"].is_null(), l["reponse"].as_str()), (Some("dossier-salarie"), Some(true), true, Some("")));
    assert_eq!(l["detail"]["periode"], json!("aout 2026"));
    assert_eq!(l["detail"]["salaries"], json!([{"cle": "BRIOCHE", "piece": "Dossier_BRIOCHE_2026-08_Bulletins.pdf"}, {"cle": "SERAPHIN", "piece": "Dossier_SERAPHIN_2026-08_Bulletins.pdf"}]));
    assert_eq!(&envoi["ligne"], &l);

    // Un envoi qui casse au milieu : le premier est parti, le journal garde le refus.
    let casse = refuse_le(1);
    let (_, m) = phrase(avec(&racine, "dossier-envoyer", charge.clone(), &casse));
    assert!(m.contains("ATTENTION, 1 message(s) etaient deja partis"), "{}", m);
    let l = journal(&racine).last().unwrap().clone();
    assert_eq!((l["quoi"].as_str(), l["accepte"].as_bool(), l["destinataire"].as_str()), (Some("dossier-salarie"), Some(false), Some("fanchon.brioche@exemple.invalid")));
    assert_eq!(l["detail"]["echec_a"], json!("valentin.seraphin@exemple.invalid"));

    // Un salarie inconnu, un dossier sans document, une boite absente.
    let (_, m) = phrase(avec(&racine, "dossier-envoyer", json!({"salaries": ["PERSONNE"], "mois": "2026-08", "motdepasse": MOT_DE_PASSE}), &faux));
    assert!(m.starts_with("Salarié inconnu : PERSONNE"), "{}", m);
    let (classe, _) = phrase(avec(&racine, "dossier-envoyer", json!({"salaries": ["BRIOCHE"], "mois": "2031-01", "motdepasse": MOT_DE_PASSE}), &faux));
    assert_eq!(classe, "AucunDocument");
    let (_, m) = phrase(telle_quelle(&racine, "dossier-envoyer", charge));
    assert!(m.starts_with("Aucune boite courriel"), "{}", m);
}

#[test]
fn dossier_envoyer_dans_une_fictive_joue_les_courriels() {
    let _t = tour();
    let racine = compagnie("dossier-fictive", true);
    let r = telle_quelle(&racine, "dossier-envoyer", json!({"salaries": ["LEMERCIER"], "mois": "2026-08", "categories": ["bulletins"]})).unwrap();
    assert_eq!(r["envoi"]["partis"], json!(["nicole.lemercier@exemple.invalid"]));
    assert_eq!(r["envoi"]["expediteur"], json!("direction@demo-3090-petite.invalid"));
    assert_eq!(noms(&racine.join("courriels-simules")).iter().filter(|n| n.ends_with(".eml")).count(), 1);
    // Ce qui part est l'archive du mois, et seulement sa page a elle : le bulletin n° 12/2026, troisieme page de
    // Bulletins_Aout2026.pdf (le moteur d'aujourd'hui recalcule un autre net : 523,22 au lieu des 519,14 payes).
    let archive = textes_fichier(&racine.join("sorties/2026-08/Bulletins_Aout2026.pdf"));
    let parti = textes_fichier(&racine.join("exports/dossiers/Dossier_LEMERCIER_2026-08_Bulletins.pdf"));
    assert_eq!(parti, vec![archive[2].clone()]);
    assert!(parti[0].contains("12/2026") && parti[0].contains("LE3.26.04"), "{}", parti[0]);
}

/// Le texte de chaque page d'un PDF, lu par la fabrique (le lecteur du moteur).
fn textes_fichier(chemin: &Path) -> Vec<String> {
    fabrique_pdf::lecture::texte(&chemin.to_string_lossy()).unwrap().0
}

/// Le texte de chaque page d'un PDF en memoire : il passe par un fichier, le lecteur lit un chemin.
fn textes_octets(octets: &[u8], nom: &str) -> Vec<String> {
    let f = outils::chemin_unique(&format!("{}.pdf", nom));
    std::fs::write(&f, octets).unwrap();
    let t = textes_fichier(&f);
    std::fs::remove_file(&f).unwrap();
    t
}

/// Un mois dont le calcul a change depuis sa production s'envoie quand meme, et ce qui part est l'archive : les pages
/// memes de `Bulletins_<Mois>.pdf` et de `CertificatsCS_<Mois>.pdf` qui portent le salarie, rien de recalcule, rien
/// de reimprime, rien d'un autre salarie.
#[test]
fn dossier_envoyer_un_mois_dont_le_calcul_a_change_envoie_les_pages_archivees_du_salarie() {
    let _t = tour();
    let racine = compagnie("dossier-archive", false);
    poser_le_mot_de_passe(&racine);
    // Le cachet de Brioche passe de 155 a 175 apres la production : 4 cachets, un brut de 700 au lieu des 620 payes.
    let mut saisie = crate::outils::saisie(&racine, "2026-08");
    assert_eq!((saisie["engagements"][0]["numero"].as_str(), saisie["engagements"][0]["lignes"][0]["taux"].as_f64()), (Some("BR1.26.04"), Some(155.0)));
    saisie["engagements"][0]["lignes"][0]["taux"] = json!(175.0);
    crate::outils::poser_saisie(&racine, "2026-08", &saisie);
    let bulletins = racine.join("sorties/2026-08/Bulletins_Aout2026.pdf");
    let certificats = racine.join("sorties/2026-08/CertificatsCS_Aout2026.pdf");
    let (octets_bulletins, octets_certificats) = (std::fs::read(&bulletins).unwrap(), std::fs::read(&certificats).unwrap());
    let (archive_b, archive_cs) = (textes_fichier(&bulletins), textes_fichier(&certificats));
    assert_eq!((archive_b.len(), archive_cs.len()), (3, 3));
    assert!(archive_b[0].contains("10/2026") && archive_b[0].contains("BR1.26.04"), "{}", archive_b[0]);

    // Les bulletins : un PDF, la premiere page de l'archive et elle seule (pas celles de Seraphin ni de Lemercier).
    let faux = Faux::nouveau();
    let r = avec(&racine, "dossier-envoyer", json!({"salaries": ["BRIOCHE"], "mois": "2026-08", "categories": ["bulletins"], "motdepasse": MOT_DE_PASSE}), &faux).unwrap();
    assert_eq!(r["envoi"]["partis"], json!(["fanchon.brioche@exemple.invalid"]));
    assert_eq!(r["pages"], json!({"BRIOCHE": {"bulletins": 1}}));
    assert_eq!(r["refus"], json!([]));
    let m = faux.message(0);
    assert_eq!(m.pieces(), ["Dossier_BRIOCHE_2026-08_Bulletins.pdf"]);
    let piece = &m.pieces_octets()[0];
    assert_eq!(piece, &std::fs::read(racine.join("exports/dossiers/Dossier_BRIOCHE_2026-08_Bulletins.pdf")).unwrap());
    let parti = textes_octets(piece, "bulletins");
    assert_eq!(parti, vec![archive_b[0].clone()]);
    assert!(!parti[0].contains("Réédité") && !parti[0].contains("SE2.26.04") && !parti[0].contains("LE3.26.04"), "{}", parti[0]);
    // Le courriel ne dit plus que les bulletins sont reedites.
    assert!(!m.textes().0.contains("réédités"), "{}", m.textes().0);

    // Les certificats Conges Spectacles : la page du N° CE 30, la sienne, avec le brut paye (620), pas celui d'aujourd'hui.
    let faux = Faux::nouveau();
    let r = avec(&racine, "dossier-envoyer", json!({"salaries": ["BRIOCHE"], "mois": "2026-08", "categories": ["certificats-cs"], "motdepasse": MOT_DE_PASSE}), &faux).unwrap();
    assert_eq!(r["pages"], json!({"BRIOCHE": {"certificats-cs": 1}}));
    assert_eq!(r["refus"], json!([]));
    let parti = textes_octets(&faux.message(0).pieces_octets()[0], "certificats");
    assert_eq!(parti, vec![archive_cs[0].clone()]);
    assert!(parti[0].contains("620") && !parti[0].contains("700") && !parti[0].contains("SERAPHIN"), "{}", parti[0]);

    // L'archive n'a pas bouge.
    assert_eq!((std::fs::read(&bulletins).unwrap(), std::fs::read(&certificats).unwrap()), (octets_bulletins, octets_certificats));

    // Ce qui ne se lit pas refuse, sans repli sur un recalcul : un fichier du mois qui ne porte plus la periode de Brioche.
    let mut saisie = crate::outils::saisie(&racine, "2026-08");
    saisie["engagements"][0]["debut"] = json!("2026-08-18");
    crate::outils::poser_saisie(&racine, "2026-08", &saisie);
    let faux = Faux::nouveau();
    let (_, m) = phrase(avec(&racine, "dossier-envoyer", json!({"salaries": ["BRIOCHE"], "mois": "2026-08", "categories": ["bulletins"], "motdepasse": MOT_DE_PASSE}), &faux));
    assert!(m.contains("aucune page correspondante dans Bulletins_Aout2026.pdf"), "{}", m);
    assert_eq!(faux.partis(), 0);
}

#[test]
fn dossier_envoyer_refuse_un_salarie_sans_courriel_et_envoie_un_seul_salarie_sur_une_annee() {
    let _t = tour();
    let racine = compagnie("dossier-cas", false);
    poser_le_mot_de_passe(&racine);
    let j0 = journal(&racine);
    let mut sal = lire(&racine, "config/salaries.json");
    sal["SERAPHIN"]["email"] = json!("");
    ecrire(&racine, "config/salaries.json", &sal);
    let faux = Faux::nouveau();
    // Sans courriel : rien ne part, meme pour les salaries qui en ont un, et rien n'est note.
    let (_, m) = phrase(avec(&racine, "dossier-envoyer", json!({"salaries": ["BRIOCHE", "SERAPHIN"], "mois": "2026-08", "categories": ["bulletins"], "motdepasse": MOT_DE_PASSE}), &faux));
    assert!(m.starts_with("Pas d'adresse e-mail pour ") && m.ends_with("compléter sa fiche avant d'envoyer. Rien n'est parti."), "{}", m);
    assert_eq!(faux.partis(), 0);
    assert_eq!(journal(&racine), j0);
    // Un seul salarie, toute une annee.
    let r = avec(&racine, "dossier-envoyer", json!({"salaries": ["BRIOCHE"], "annee": 2026, "categories": ["bulletins"], "motdepasse": MOT_DE_PASSE}), &faux).unwrap();
    assert_eq!(r["envoi"]["partis"], json!(["fanchon.brioche@exemple.invalid"]));
    assert_eq!(r["envoi"]["messages"], json!(1));
    assert_eq!(faux.partis(), 1);
    assert_eq!(journal(&racine).len(), j0.len() + 1);
    // Ce qui part : les pages des archives de 2026 qui portent ses contrats (BR1.), et elles seules, dans l'ordre des mois.
    let mut siennes: Vec<String> = Vec::new();
    for mois in noms(&racine.join("sorties")).into_iter().filter(|m| m.starts_with("2026-")) {
        for f in noms(&racine.join("sorties").join(&mois)).into_iter().filter(|n| n.starts_with("Bulletins_")) {
            siennes.extend(textes_fichier(&racine.join("sorties").join(&mois).join(f)).into_iter().filter(|t| t.contains("Contrat n° BR1.")));
        }
    }
    assert!(siennes.len() >= 2, "{}", siennes.len());
    assert_eq!(textes_octets(&faux.message(0).pieces_octets()[0], "annee"), siennes);
    // Toute la periode : le courriel ne dit pas de periode (une compagnie qui n'a qu'aout 2026, pour ne pas tout relire).
    let racine = compagnie::config_et_mois("fictive-3090-petite", &["2026-08"], &dossier_de_test("routes_envois", "dossier-toutes-les-annees").join("compagnie"));
    poser_le_mot_de_passe(&racine);
    let tout = Faux::nouveau();
    avec(&racine, "dossier-envoyer", json!({"salaries": ["BRIOCHE"], "toutes_les_annees": true, "categories": ["bulletins"], "motdepasse": MOT_DE_PASSE}), &tout).unwrap();
    assert!(tout.message(0).textes().0.contains("Tu trouveras ci-joint ton dossier de salarié : bulletins de paie."), "{}", tout.message(0).textes().0);
}

// ================================================================== casc-envoyer

#[test]
fn casc_envoyer_envoie_les_deux_courriels_garde_les_eml_et_note_les_annees() {
    let _t = tour();
    let racine = compagnie("casc", false);
    poser_le_mot_de_passe(&racine);
    let charge = json!({"vire_le": {"2025": "2026-09-01", "2026": "2026-09-02"}, "motdepasse": MOT_DE_PASSE});
    let j0 = journal(&racine);
    let suivi0 = racine.join("config/casc-svp.json").exists();
    let sans = Faux::nouveau();

    // Des refus avant toute ecriture de courriel : pas de date, une date illisible, une date a venir, une annee sans dossier.
    let (_, m) = phrase(avec(&racine, "casc-envoyer", json!({"motdepasse": MOT_DE_PASSE}), &sans));
    assert!(m.contains("virement"), "{}", m);
    let (_, m) = phrase(avec(&racine, "casc-envoyer", json!({"vire_le": {"2025": "hier"}, "motdepasse": MOT_DE_PASSE}), &sans));
    assert!(m.contains("2025") && m.contains("'hier'"), "{}", m);
    let (_, m) = phrase(avec(&racine, "casc-envoyer", json!({"vire_le": {"2025": "2099-01-01"}, "motdepasse": MOT_DE_PASSE}), &sans));
    assert!(m.contains("2025") && m.contains("01/01/2099"), "un virement a venir n'est pas fait : {}", m);
    let (_, m) = phrase(avec(&racine, "casc-envoyer", json!({"vire_le": {"2024": "2026-09-01"}, "motdepasse": MOT_DE_PASSE}), &sans));
    assert!(m.contains("2025-01"), "rien a regulariser depuis le premier mois sans CASC-SVP : {}", m);
    // Sans preuve : le dossier s'ecrit avant la preuve, rien ne part, rien n'est note.
    let mut mauvais = charge.clone();
    mauvais["motdepasse"] = json!("non");
    assert_eq!(phrase(avec(&racine, "casc-envoyer", mauvais, &sans)).0, "PreuveInsuffisante");
    assert_eq!(sans.partis(), 0);
    assert_eq!(journal(&racine), j0);
    assert_eq!(racine.join("config/casc-svp.json").exists(), suivi0);
    assert!(!racine.join("exports/casc-svp/envoyes").exists());

    let faux = Faux::nouveau();
    let r = avec(&racine, "casc-envoyer", charge.clone(), &faux).unwrap();
    let mut cles: Vec<&str> = r.as_object().unwrap().keys().map(String::as_str).collect();
    cles.sort();
    assert_eq!(cles, ["annees", "dossier", "eml", "expediteur", "ligne", "partis", "periode", "total"]);
    assert_eq!(r["partis"], json!(["comptes-tiers-dsn@audiens.org", "entreprises@casc-svp.fr"]));
    assert_eq!(r["expediteur"], json!("boite@exemple.invalid"));
    assert_eq!(faux.partis(), 2);
    // Audiens : le texte et sa version HTML, le tableau des masses en vrai tableau. Le CASC-SVP : le classeur en piece jointe.
    let (texte, html) = faux.message(0).textes();
    assert!(texte.contains("| mois | masse | cotisation |"), "{}", texte);
    assert!(html.contains("<table style=\"border-collapse:collapse\"><tr><th style="), "{}", html);
    assert_eq!(faux.message(0).pieces(), Vec::<String>::new());
    assert_eq!(faux.message(1).pieces(), ["CASC-SVP_liste-nominative.xlsx"]);
    assert_eq!(faux.message(1).entete("To"), "entreprises@casc-svp.fr");
    // Les deux messages tels qu'ils sont partis, gardes ; les annees notees regularisees.
    let garde = racine.join("exports/casc-svp/envoyes");
    let emls = noms(&garde);
    assert_eq!(emls.len(), 2);
    assert!(emls[0].ends_with("_CASC-SVP_audiens.eml") && emls[1].ends_with("_CASC-SVP_casc.eml"), "{:?}", emls);
    assert_eq!(std::fs::read(garde.join(&emls[0])).unwrap(), faux.recus.borrow()[0]);
    assert_eq!(r["eml"], json!(emls));
    assert_eq!(r["dossier"].as_str().unwrap(), garde.to_string_lossy());
    assert_eq!(r["annees"].as_array().unwrap().len(), 2);
    assert_eq!(r["annees"][0]["annee"], json!("2025"));
    assert_eq!(r["annees"][0]["vire_le"], json!("2026-09-01"));
    assert_eq!(r["annees"][0]["qui"], json!("interface"));
    let suivi = lire(&racine, "config/casc-svp.json");
    assert_eq!(suivi["regularisations"].as_array().unwrap().len(), 2);
    let l = journal(&racine).last().unwrap().clone();
    assert_eq!((l["quoi"].as_str(), l["accepte"].as_bool(), l["mois"].is_null()), (Some("regularisation-casc"), Some(true), true));
    assert_eq!(l["reponse"], json!("2 courriel(s) envoye(s)"));
    assert_eq!(l["detail"]["annees"][0], json!({"annee": "2025", "vire_le": "2026-09-01", "montant": "0.00"}));
    assert_eq!(l["detail"]["eml"], json!(emls));
    assert_eq!(&r["ligne"], &l);

    // Une fois les annees regularisees, il n'y a plus rien a envoyer.
    let (_, m) = phrase(avec(&racine, "casc-envoyer", charge, &faux));
    assert!(m.starts_with("Tout est regularise : 2025 (0.00 €, viré le 01/09/2026), 2026 (0.00 €, viré le 02/09/2026)."), "{}", m);
}

#[test]
fn casc_envoyer_qui_echoue_ne_garde_ni_eml_ni_annee() {
    let _t = tour();
    let racine = compagnie("casc-echec", false);
    poser_le_mot_de_passe(&racine);
    let j0 = journal(&racine).len();
    let faux = refuse_le(1);
    let (_, m) = phrase(avec(&racine, "casc-envoyer", json!({"vire_le": {"2025": "2026-09-01"}, "motdepasse": MOT_DE_PASSE}), &faux));
    assert!(m.contains("ATTENTION, 1 message(s) etaient deja partis : comptes-tiers-dsn@audiens.org"), "{}", m);
    assert!(!racine.join("exports/casc-svp/envoyes").exists());
    assert!(!racine.join("config/casc-svp.json").exists());
    let j = journal(&racine);
    assert_eq!(j.len(), j0 + 1);
    let l = j.last().unwrap();
    assert_eq!((l["quoi"].as_str(), l["accepte"].as_bool(), l["destinataire"].as_str()), (Some("regularisation-casc"), Some(false), Some("comptes-tiers-dsn@audiens.org")));
    assert_eq!(l["detail"], json!({"annees": ["2025"], "partis": ["comptes-tiers-dsn@audiens.org"], "echec_a": "entreprises@casc-svp.fr"}));
    // Une boite absente.
    let (_, m) = phrase(telle_quelle(&racine, "casc-envoyer", json!({"vire_le": {"2025": "2026-09-01"}, "motdepasse": MOT_DE_PASSE})));
    assert!(m.starts_with("Aucune boite courriel"), "{}", m);
}

#[test]
fn casc_envoyer_dans_une_fictive_joue_les_courriels() {
    let _t = tour();
    let racine = compagnie("casc-fictive", true);
    let r = telle_quelle(&racine, "casc-envoyer", json!({"vire_le": {"2025": "2026-09-01"}})).unwrap();
    assert_eq!(r["partis"].as_array().unwrap().len(), 2);
    assert_eq!(noms(&racine.join("courriels-simules")).iter().filter(|n| n.ends_with(".eml")).count(), 2);
}

/// Une compagnie « grande » dont les DSN de ces mois ne portent plus la CASC-SVP (la reference d'adhesion `100`
/// devient `900`) : il y a des masses a regulariser.
fn sans_casc(racine: &Path, mois: &[&str]) {
    for m in mois {
        for e in std::fs::read_dir(racine.join("sorties").join(m)).unwrap().flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with("DSN_") && n.ends_with(".dsn") {
                // latin-1 : un octet, un caractere.
                let t: String = std::fs::read(e.path()).unwrap().iter().map(|&b| b as char).collect();
                let t2 = t.replace("S21.G00.15.004,'01'\r\nS21.G00.15.005,'100'", "S21.G00.15.004,'01'\r\nS21.G00.15.005,'900'").replace("S21.G00.15.004,'01'\nS21.G00.15.005,'100'", "S21.G00.15.004,'01'\nS21.G00.15.005,'900'");
                assert_ne!(t, t2, "{}", n);
                let b: Vec<u8> = t2.chars().map(|c| c as u32 as u8).collect();
                std::fs::write(e.path(), b).unwrap();
            }
        }
    }
}

/// La CASC-SVP d'un mois : `taux` % du brut (51.013 de type 001) que sa DSN declare, au centime.
fn casc_du_mois(racine: &Path, mois: &str, taux: Decimal) -> Decimal {
    let dossier = racine.join("sorties").join(mois);
    let dsn = noms(&dossier).into_iter().find(|n| n.starts_with("DSN_") && n.ends_with(".dsn")).unwrap();
    let texte: String = std::fs::read(dossier.join(dsn)).unwrap().iter().map(|&o| o as char).collect();
    let (mut type_, mut masse) = (String::new(), Decimal::ZERO);
    for (k, v) in paires(&texte) {
        match k.as_str() {
            "S21.G00.51.011" => type_ = v,
            "S21.G00.51.013" if type_ == "001" => masse += outils::dec(&v),
            _ => {}
        }
    }
    (masse * taux / Decimal::from(100)).round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}

#[test]
fn casc_envoyer_avec_des_masses_les_montants_non_nuls_et_le_classeur() {
    let _t = tour();
    let racine = compagnie_de("casc-masses", "fictive-3090-grande", false);
    poser_le_mot_de_passe(&racine);
    sans_casc(&racine, &["2025-03", "2025-10", "2025-11"]);
    let faux = Faux::nouveau();
    let r = avec(&racine, "casc-envoyer", json!({"vire_le": {"2025": "2026-10-01"}, "motdepasse": MOT_DE_PASSE}), &faux).unwrap();
    // 118,84 EUR : le taux de 2025 (`donnees/casc-svp-base.json`, 0,40 %) du brut que chacune des trois DSN declare.
    let taux = Decimal::from_str_exact(&lire(&socle::fichiers::dossier_du_code(), "casc-svp-base.json")["taux"]["2025"].to_string()).unwrap();
    let total: Decimal = ["2025-03", "2025-10", "2025-11"].iter().map(|m| casc_du_mois(&racine, m, taux)).sum();
    assert_eq!(total.to_string(), "118.84");
    assert_eq!(r["total"], json!("118.84"));
    assert_eq!(r["periode"], json!("mars 2025 à novembre 2025"));
    assert_eq!(r["annees"][0]["montant"], json!("118.84"));
    assert_eq!(r["annees"][0]["mois"], json!(["2025-03", "2025-10", "2025-11"]));
    assert_eq!(r["partis"], json!(["comptes-tiers-dsn@audiens.org", "entreprises@casc-svp.fr"]));
    let (texte, html) = faux.message(0).textes();
    assert!(texte.contains("118,84") || texte.contains("118.84"), "{}", texte);
    assert!(html.contains("<table"));
    assert_eq!(faux.message(1).pieces(), ["CASC-SVP_liste-nominative.xlsx"]);
    // Le classeur et le suivi gardes ; l'annee ne se regularise pas deux fois.
    assert!(racine.join("exports/casc-svp/CASC-SVP_liste-nominative.xlsx").exists());
    assert_eq!(lire(&racine, "config/casc-svp.json")["regularisations"].as_array().unwrap().len(), 1);
    let (_, m) = phrase(avec(&racine, "casc-envoyer", json!({"vire_le": {"2025": "2026-10-01"}, "motdepasse": MOT_DE_PASSE}), &faux));
    assert!(m.starts_with("Tout est regularise : 2025 (118.84 €, viré le 01/10/2026)."), "{}", m);
    // Une date de virement en semaine ISO ne se lit pas.
    let (_, m) = phrase(avec(&racine, "casc-envoyer", json!({"vire_le": {"2025": "2026-W40-3"}, "motdepasse": MOT_DE_PASSE}), &faux));
    assert!(m.contains("'2026-W40-3'"), "{}", m);
}

// ================================================================== l'aiguillage

#[test]
fn une_route_inconnue_ou_sans_charge_est_refusee() {
    let _t = tour();
    let racine = compagnie("aiguillage", true);
    assert!(phrase(telle_quelle(&racine, "envoyer-autre-chose", json!({}))).1.contains("route inconnue"));
    // `avenants-envoyer` sans la liste, `avenant-envoyer` sans salarie.
    phrase(telle_quelle(&racine, "avenants-envoyer", json!({})));
    phrase(telle_quelle(&racine, "avenant-envoyer", json!({"debut": "2026-10-13"})));
}

/// #136 : l'avenant a le controle du contrat. Ses articles sont ceux du modele de contrat de la compagnie : un gabarit
/// sans `{{remuneration.hors_brut}}` ne fait pas partir un avenant qui tairait les defraiements. Rien n'est ecrit, rien
/// ne part, et le refus nomme le fichier et le marqueur.
#[test]
fn un_avenant_sur_un_gabarit_sans_le_marqueur_des_defraiements_refuse_de_les_taire() {
    let _t = tour();
    let racine = compagnie("avenant-sans-hors-brut", false);
    poser_le_mot_de_passe(&racine);
    let faux = Faux::nouveau();
    contrats_partis_puis_modifies(&racine, &faux);
    let partis_avant = faux.partis();
    let gabarit = gabarit_sans_hors_brut(&racine);
    let r = avec(&racine, "avenant-envoyer", json!({"salarie": "BRIOCHE", "debut": "2026-10-13", "motdepasse": MOT_DE_PASSE}), &faux);
    let (classe, message) = phrase(r);
    assert_eq!(classe, "ModeleIncomplet", "{}", message);
    assert!(message.contains(gabarit.to_str().unwrap()) && message.contains("{{remuneration.hors_brut}}"), "{}", message);
    assert!(!noms(&racine.join("sorties/2026-10/envoi")).iter().any(|n| n.starts_with("Avenant")), "aucun avenant ecrit");
    assert_eq!(faux.partis(), partis_avant, "rien n'est parti");
}
