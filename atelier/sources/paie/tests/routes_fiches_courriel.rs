//! `fiche-demander` et `fiches-relever` : la fiche remplie par le salarie lui-meme, par courriel. La demande exige une
//! adresse valide, une boite et la preuve ; la releve ne retient que la reponse de la bonne adresse, le code hors de la
//! citation, et complete une fiche existante dans ses cases vides. Aucun reseau : la boite simulee de la compagnie fictive ou
//! un faux `Courriel` ; chaque test sur sa copie de `fictive-3090-petite`.
//!
//! L'environnement est celui du processus, pose une fois (`tour`) : le jour, la cle du poste, aucune variable de service.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Once};

use paie::coffre::Coffre;
use droit::contexte::Contexte;
use outils::{copier, dossier_de_test, dossier_du_processus, FauxCourriel as Faux, Mail};
use socle::date::Jour;
use socle::echec::Echec;
use paie::routes_fiches_courriel as rfc;
use serde_json::{json, Value};

#[path = "outils/mod.rs"]
mod outils;

const AUJOURDHUI: &str = "2026-10-03";
const MOT_DE_PASSE: &str = "sesame-123";
const C1: &str = "FIC-ABCDEF";
const C2: &str = "FIC-GHJKMN";
const ZOE: &str = "zoe@exemple.invalid";

static UNE_FOIS: Once = Once::new();

/// L'environnement du processus, pose une fois avant le premier test : le jour, la cle du poste, aucun secret.
fn tour() {
    UNE_FOIS.call_once(|| {
        for v in outils::RETIREES {
            std::env::remove_var(v);
        }
        std::env::set_var("PUPPETPAIE_AUJOURDHUI", AUJOURDHUI);
        std::env::set_var("PUPPETPAIE_CLE", dossier_poste());
    });
}

fn dossier_poste() -> PathBuf {
    static D: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    D.get_or_init(|| dossier_du_processus("routes-fiches-poste")).clone()
}

/// Une copie de `fictive-3090-petite`. `fictive` : elle garde son marqueur (boite simulee, rien a prouver) ; sinon c'est
/// une compagnie ordinaire : la preuve est exigee (le mot de passe du logiciel est pose), la boite est un faux client.
fn compagnie(nom: &str, fictive: bool) -> PathBuf {
    let racine = dossier_de_test("routes-fiches-courriel", nom).join("compagnie");
    copier(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/donnees/fictives/fictive-3090-petite"), &racine);
    if !fictive {
        std::fs::remove_file(racine.join("COPIE-DE-DEVELOPPEMENT.json")).unwrap();
        // Le mot de passe du logiciel : pose une fois (scrypt est lent hors optimisations), le coffre est recopie ensuite.
        let modele = coffre_modele();
        std::fs::create_dir_all(racine.join("config")).unwrap();
        std::fs::copy(&modele, racine.join("config").join(modele.file_name().unwrap())).unwrap();
    }
    racine
}

fn coffre_modele() -> PathBuf {
    static MODELE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    MODELE
        .get_or_init(|| {
            let base = dossier_du_processus("routes-fiches-modele");
            let c = Coffre { racine: base.clone(), dossier_cle: dossier_poste(), maintenant: "2026-10-03T10:00".into(), variables: BTreeMap::new() };
            c.poser_ouverture(MOT_DE_PASSE, None).unwrap();
            c.chemin()
        })
        .clone()
}

fn lire(racine: &Path, rel: &str) -> Value {
    serde_json::from_slice(&std::fs::read(racine.join(rel)).unwrap()).unwrap()
}

fn ecrire(racine: &Path, rel: &str, v: &Value) {
    std::fs::write(racine.join(rel), serde_json::to_vec_pretty(v).unwrap()).unwrap();
}

fn journal(racine: &Path) -> Vec<Value> {
    lire(racine, "journal/depots.json").as_array().unwrap().clone()
}

fn lignes_du_journal(racine: &Path, quoi: &str) -> Vec<Value> {
    journal(racine).into_iter().filter(|l| l["quoi"] == json!(quoi)).collect()
}

fn options(racine: &Path) -> HashMap<String, String> {
    let mut o = HashMap::new();
    o.insert("racine".to_string(), racine.to_string_lossy().into_owned());
    o.insert("date".to_string(), AUJOURDHUI.to_string());
    o
}

fn contexte(racine: &Path) -> Contexte {
    Contexte::nouveau(racine, Some(Jour::lire(AUJOURDHUI).unwrap())).unwrap()
}

// ------------------------------------------------------------------ des messages ecrits a la main

/// Les lignes du questionnaire, remplies comme une personne le ferait ; `sauf` : les reponses a changer ou vider.
fn reponses(code: &str, changements: &[(&str, &str)]) -> String {
    let mut rep: HashMap<&str, String> = HashMap::from([
        ("prenom", "Zoé".to_string()),
        ("nom", "Dubois".to_string()),
        ("naissance", "14/03/1991".to_string()),
        ("lieu", "Lyon".to_string()),
        ("pays", "France".to_string()),
        ("nationalite", "française".to_string()),
        ("nir", "291036912345653".to_string()),
        ("adresse", "5 rue des Lilas".to_string()),
        ("code_postal", "69003".to_string()),
        ("commune", "Lyon".to_string()),
    ]);
    for (k, v) in changements {
        rep.insert(k, v.to_string());
    }
    let mut lignes: Vec<String> = paie::fiche_questionnaire::QUESTIONS
        .iter()
        .map(|(cle, lib, fac)| format!("{}{} : {}", lib, if *fac { " (facultatif)" } else { "" }, rep.get(cle).cloned().unwrap_or_default()))
        .collect();
    lignes.push(String::new());
    lignes.push(format!("Code (ne pas modifier) : {}", code));
    lignes.join("\r\n") + "\r\n"
}

fn message(de: &str, corps: &str, date: &str, id: &str) -> Vec<u8> {
    format!(
        "From: {}\r\nTo: boite@exemple.invalid\r\nSubject: Re: Ta fiche pour Les Pupazzi\r\nDate: {}\r\nMessage-ID: {}\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\n{}",
        de, date, id, corps
    )
    .into_bytes()
}

fn reponse(de: &str, code: &str, changements: &[(&str, &str)], id: &str) -> Vec<u8> {
    message(de, &reponses(code, changements), "Sat, 03 Oct 2026 10:15:00 +0200", id)
}

/// Ecrit au journal une demande de fiche d'un code donne (celui des messages ecrits a la main).
fn noter_demande(racine: &Path, code: &str, adresse: &str, prenom: &str, message_id: &str) {
    let mut lignes = journal(racine);
    lignes.push(json!({
        "quoi": "fiche-demande", "quand": "2026-10-02T09:00:00", "origine": "", "mois": null, "fichier": null, "empreinte": null,
        "mode": "reel", "qui": "interface", "accepte": true, "destinataire": adresse, "flux": "", "retours": [],
        "reponse": format!("fiche demandee a {}", adresse), "trace": null,
        "detail": {"code": code, "adresse": adresse, "prenom": prenom, "message": message_id, "expediteur": "boite@exemple.invalid"}
    }));
    ecrire(racine, "journal/depots.json", &Value::Array(lignes));
}

// ------------------------------------------------------------------ appeler une route

fn demande_avec(racine: &Path, charge: Value, faux: &Faux) -> Result<Value, Echec> {
    let mut f = faux.clone();
    rfc::route_avec(&options(racine), &contexte(racine), "fiche-demander", &charge, Some(&mut f))
}

fn demande_telle_quelle(racine: &Path, charge: Value) -> Result<Value, Echec> {
    rfc::route(&options(racine), &contexte(racine), "fiche-demander", &charge)
}

fn releve_avec(racine: &Path, faux: &Faux) -> Value {
    let mut f = faux.clone();
    rfc::route_avec(&options(racine), &contexte(racine), "fiches-relever", &json!({}), Some(&mut f)).unwrap()
}

fn releve_telle_quelle(racine: &Path) -> Value {
    rfc::route(&options(racine), &contexte(racine), "fiches-relever", &json!({})).unwrap()
}

/// Retours Windows, A : la reponse d'une route se lit en ligne de commande, dans un fil, dans un journal : ni NIR ni IBAN
/// (aucune valeur de `config/salaries.json` de la compagnie n'y figure, et aucune cle `nir` ou `iban`).
fn sans_donnees_personnelles(racine: &Path, reponse: &Value) {
    let texte = reponse.to_string();
    let sal = lire(racine, "config/salaries.json");
    let mut vus = 0;
    for (_, fiche) in sal.as_object().unwrap() {
        for cle in ["nir", "iban"] {
            if let Some(v) = fiche.get(cle).and_then(Value::as_str).filter(|v| !v.is_empty()) {
                vus += 1;
                assert!(!texte.contains(v), "la reponse porte un {} : {}", cle, cle);
            }
        }
    }
    assert!(vus > 0, "la compagnie d'essai doit avoir des NIR et des IBAN");
    assert!(!texte.contains("\"nir\"") && !texte.contains("\"iban\"") && !texte.contains("configuration"), "{}", texte);
}

fn phrase(r: Result<Value, Echec>) -> (String, String) {
    let e = r.expect_err("un refus etait attendu");
    (e.classe.unwrap_or("").to_string(), e.message)
}

/// Les cles d'un objet, triees : la page les lit par leur nom, pas par leur rang.
fn cles(v: &Value) -> Vec<&str> {
    let mut c: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    c.sort();
    c
}

fn textes(octets: &[u8]) -> (String, String) {
    Mail(octets.to_vec()).textes()
}

fn entete(octets: &[u8], nom: &str) -> String {
    Mail(octets.to_vec()).entete(nom)
}

// ================================================================== fiche-demander

#[test]
fn demander_dans_une_fictive_ecrit_le_courriel_simule_et_le_journal() {
    tour();
    let racine = compagnie("demander-fictive", true);
    let r = demande_telle_quelle(&racine, json!({"adresse": "  Nouvelle@Exemple.INVALID ", "prenom": " Zoé "})).unwrap();
    // Retours Windows, A : code et adresse seulement, aucune configuration (donc ni NIR ni IBAN).
    assert_eq!(cles(&r), ["adresse", "code"]);
    sans_donnees_personnelles(&racine, &r);
    let code = r["code"].as_str().unwrap();
    assert!(paie::fiche_questionnaire::est_un_code(code), "{}", code);
    assert_eq!(r["adresse"], json!("nouvelle@exemple.invalid"));
    // Le courriel joue.
    let dossier = racine.join("courriels-simules");
    let joues: Vec<PathBuf> = std::fs::read_dir(&dossier).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "eml")).collect();
    let nouveaux: Vec<&PathBuf> = joues.iter().filter(|p| std::fs::read_to_string(p).unwrap().contains(code)).collect();
    assert_eq!(nouveaux.len(), 1);
    let octets = std::fs::read(nouveaux[0]).unwrap();
    assert_eq!(entete(&octets, "To"), "nouvelle@exemple.invalid");
    assert_eq!(entete(&octets, "Subject"), "Ta fiche pour Le Fil tendu (démonstration)");
    let (texte, html) = textes(&octets);
    assert!(texte.starts_with("Bonjour Zoé,\n"), "{}", texte);
    assert!(texte.contains(&format!("Code (ne pas modifier) : {}", code)));
    assert!(texte.contains("Prénom : \nNom de famille (de naissance) : \n"));
    assert!(html.contains("Remplir ma fiche"));
    assert!(html.contains("href=\"mailto:"));
    // Une ligne du journal, sans que rien soit exige : la compagnie fictive ne prouve rien.
    let l = lignes_du_journal(&racine, "fiche-demande");
    assert_eq!(l.len(), 1);
    assert_eq!(l[0]["accepte"], json!(true));
    assert_eq!(l[0]["qui"], json!("interface"));
    assert_eq!(l[0]["destinataire"], json!("nouvelle@exemple.invalid"));
    assert_eq!(l[0]["reponse"], json!("fiche demandee a nouvelle@exemple.invalid"));
    assert_eq!(cles(&l[0]["detail"]), ["adresse", "code", "expediteur", "message", "prenom"]);
    assert_eq!(l[0]["detail"]["code"], json!(code));
    assert_eq!(l[0]["detail"]["prenom"], json!("Zoé"));
    assert!(l[0]["detail"]["message"].as_str().unwrap().starts_with('<'));
}

#[test]
fn demander_sans_prenom_dit_bonjour_tout_court() {
    tour();
    let racine = compagnie("demander-sans-prenom", false);
    let faux = Faux::nouveau();
    demande_avec(&racine, json!({"adresse": "z@exemple.invalid", "motdepasse": MOT_DE_PASSE}), &faux).unwrap();
    let (texte, _) = textes(&faux.recus.borrow()[0]);
    assert!(texte.starts_with("Bonjour,\n"), "{}", texte);
    assert_eq!(lignes_du_journal(&racine, "fiche-demande")[0]["detail"]["prenom"], json!(""));
}

#[test]
fn une_adresse_invalide_est_refusee_avant_la_boite_et_avant_la_preuve() {
    tour();
    let racine = compagnie("demander-adresses", false);
    let faux = Faux::nouveau();
    for (adresse, vue) in [
        ("", ""),
        ("   ", ""),
        ("sans-arobase", "sans-arobase"),
        ("deux@@arobases.fr", "deux@@arobases.fr"),
        ("a@b@c.fr", "a@b@c.fr"),
        ("a b@c.fr", "a b@c.fr"),
        ("a@b", "a@b"),
        ("a@.b.", "a@.b."),
        ("a@b.", "a@b."),
        ("@b.c", "@b.c"),
        ("a@.c", "a@.c"),
        ("a\u{a0}b@c.fr", "a\u{a0}b@c.fr"),
        ("A@B", "a@b"),
    ] {
        // Meme sans mot de passe : la phrase est celle de l'adresse.
        let r = demande_avec(&racine, json!({"adresse": adresse}), &faux);
        let (classe, m) = phrase(r);
        assert!(classe != "PreuveInsuffisante" && m.contains(&format!("« {} »", vue)), "le refus est celui de l'adresse, lue {:?} : {}", vue, m);
    }
    let (classe, m) = phrase(demande_avec(&racine, json!({}), &faux));
    assert!(classe != "PreuveInsuffisante" && m.contains("«  »"), "{}", m);
    assert_eq!(faux.partis(), 0);
    assert!(lignes_du_journal(&racine, "fiche-demande").is_empty(), "rien au journal : rien n'a ete tente");
    // Les adresses ordinaires, dont celle qui a plusieurs points et celle en majuscules, passent.
    for adresse in ["a@b.c", "A@B.C", "prenom.nom@sous.domaine.fr", "x+y@z.org"] {
        demande_avec(&racine, json!({"adresse": adresse, "motdepasse": MOT_DE_PASSE}), &faux).unwrap();
    }
    assert_eq!(faux.partis(), 4);
}

#[test]
fn la_preuve_est_exigee_hors_fictive() {
    tour();
    let racine = compagnie("demander-preuve", false);
    let faux = Faux::nouveau();
    let charge = |extra: Value| {
        let mut c = json!({"adresse": "z@exemple.invalid"});
        for (k, v) in extra.as_object().unwrap() {
            c[k] = v.clone();
        }
        c
    };
    assert_eq!(
        phrase(demande_avec(&racine, charge(json!({})), &faux)),
        ("PreuveInsuffisante".to_string(), paie::demande::SANS_HUMAIN.to_string())
    );
    assert_eq!(phrase(demande_avec(&racine, charge(json!({"motdepasse": ""})), &faux)).0, "PreuveInsuffisante");
    assert_eq!(phrase(demande_avec(&racine, charge(json!({"motdepasse": null})), &faux)).0, "PreuveInsuffisante");
    assert_eq!(phrase(demande_avec(&racine, charge(json!({"motdepasse": "pas-le-bon"})), &faux)).0, "PreuveInsuffisante");
    assert_eq!(faux.partis(), 0);
    assert!(lignes_du_journal(&racine, "fiche-demande").is_empty());
    demande_avec(&racine, charge(json!({"motdepasse": MOT_DE_PASSE})), &faux).unwrap();
    assert_eq!(faux.partis(), 1);
}

#[test]
fn sans_mot_de_passe_du_logiciel_rien_ne_part() {
    tour();
    let racine = compagnie("demander-sans-mdp-logiciel", false);
    // Le coffre de la copie perd le mot de passe du logiciel.
    let c = Coffre { racine: racine.clone(), dossier_cle: dossier_poste(), maintenant: "2026-10-03T10:00".into(), variables: BTreeMap::new() };
    c.oublier_ouverture(Some(MOT_DE_PASSE)).unwrap();
    let faux = Faux::nouveau();
    let (classe, message) = phrase(demande_avec(&racine, json!({"adresse": "z@exemple.invalid", "motdepasse": MOT_DE_PASSE}), &faux));
    assert_eq!(classe, "PreuveInsuffisante");
    assert!(message.starts_with("Aucun mot de passe du logiciel n'est pose : rien ne peut partir en reel."), "{}", message);
    assert_eq!(faux.partis(), 0);
}

#[test]
fn une_version_exigee_arrete_la_demande() {
    tour();
    let racine = compagnie("demander-version", false);
    // La version prete est celle de l'INSTALLATION (lot U1), pas de la compagnie : celle du contexte (`ctx.code`).
    let inst = outils::installation(&racine.parent().unwrap().join("installation"), "2026.10.09");
    std::fs::create_dir_all(inst.join("maj/9999.0.0/logiciel/donnees")).unwrap();
    ecrire(&inst, "maj/prete.json", &json!({"version": "9999.0.0", "exigee": "calcul de la CSG corrige"}));
    let mut ctx = contexte(&racine);
    ctx.code = inst.join("logiciel").join("donnees");
    let faux = Faux::nouveau();
    let mut f = faux.clone();
    let r = rfc::route_avec(&options(&racine), &ctx, "fiche-demander", &json!({"adresse": "z@exemple.invalid", "motdepasse": MOT_DE_PASSE}), Some(&mut f));
    let (classe, message) = phrase(r);
    assert_eq!(classe, "VersionExigee");
    assert!(message.starts_with("Rien n'est parti : la version 9999.0.0 est indispensable avant de declarer (calcul de la CSG corrige)"), "{}", message);
    assert_eq!(faux.partis(), 0);
}

#[test]
fn sans_boite_hors_fictive_la_demande_est_refusee() {
    tour();
    let racine = compagnie("demander-boite-absente", false);
    // La boite d'abord, la preuve ensuite : meme avec le bon mot de passe, et meme sans.
    for charge in [json!({"adresse": "z@exemple.invalid", "motdepasse": MOT_DE_PASSE}), json!({"adresse": "z@exemple.invalid"})] {
        let (classe, message) = phrase(demande_telle_quelle(&racine, charge));
        assert_eq!(classe, "BoiteAbsente");
        assert!(message.starts_with("Aucune boite courriel : poser le mot de passe d'application de la boite"), "{}", message);
    }
    // Une adresse invalide passe avant la boite.
    assert!(phrase(demande_telle_quelle(&racine, json!({"adresse": "non", "motdepasse": MOT_DE_PASSE}))).1.contains("n'est pas une adresse de courriel"));
    assert!(lignes_du_journal(&racine, "fiche-demande").is_empty());
}

#[test]
fn un_envoi_refuse_laisse_une_ligne_d_echec_et_aucune_demande_attendue() {
    tour();
    let racine = compagnie("demander-echec", false);
    let mut faux = Faux::nouveau();
    faux.refuse_le = Some(0);
    let (classe, message) = phrase(demande_avec(&racine, json!({"adresse": "z@exemple.invalid", "prenom": "Zoé", "motdepasse": MOT_DE_PASSE}), &faux));
    assert_eq!(classe, "ErreurCourriel");
    assert!(message.contains("le serveur refuse ce message"), "{}", message);
    let l = lignes_du_journal(&racine, "fiche-demande");
    assert_eq!(l.len(), 1);
    assert_eq!(l[0]["accepte"], json!(false));
    assert!(l[0]["reponse"].as_str().unwrap().contains("le serveur refuse ce message"));
    assert_eq!(cles(&l[0]["detail"]), ["adresse", "code", "prenom"], "l'echec ne porte ni message ni expediteur");
    // Une demande refusee n'est pas attendue : la releve n'a rien a chercher.
    assert!(rfc::fiches_attendues(&contexte(&racine)).unwrap().is_empty());
    // Et la suivante, acceptee, a un autre code.
    faux.refuse_le = None;
    let r = demande_avec(&racine, json!({"adresse": "z@exemple.invalid", "prenom": "Zoé", "motdepasse": MOT_DE_PASSE}), &faux).unwrap();
    assert_ne!(r["code"], l[0]["detail"]["code"]);
    assert_eq!(rfc::fiches_attendues(&contexte(&racine)).unwrap().len(), 1);
}

#[test]
fn le_courriel_dit_la_compagnie_et_echappe_ce_qu_il_faut() {
    tour();
    let racine = compagnie("demander-compagnie", false);
    let mut ent = lire(&racine, "config/entreprise.json");
    ent["nom_affichage"] = json!("Les Pupazzi d'Été & Cie <test>");
    ent["signataire"] = json!({"nom": "SAND", "prenom": "GEORGE", "prenom_affichage": "Geörge"});
    ecrire(&racine, "config/entreprise.json", &ent);
    let faux = Faux::nouveau();
    demande_avec(&racine, json!({"adresse": "z@exemple.invalid", "prenom": "Zoé <b>&", "motdepasse": MOT_DE_PASSE}), &faux).unwrap();
    let octets = faux.recus.borrow()[0].clone();
    assert_eq!(entete(&octets, "Subject"), "Ta fiche pour Les Pupazzi d'Été & Cie <test>");
    let (texte, html) = textes(&octets);
    assert!(texte.starts_with("Bonjour Zoé <b>&,\n"), "{}", texte);
    assert!(texte.ends_with("Bien à toi,\nGeörge\nLes Pupazzi d'Été & Cie <test>\n"), "{:?}", texte);
    assert!(html.contains("Bonjour Zoé &lt;b&gt;&amp;,"), "{}", html);
    assert!(html.contains("Les Pupazzi d&#x27;Été &amp; Cie &lt;test&gt;"), "{}", html);
    assert!(!html.contains("<test>"));
    // Sans nom d'affichage : la raison sociale ; sans prenom d'affichage du signataire : le prenom capitalise.
    let mut ent = lire(&racine, "config/entreprise.json");
    ent["nom_affichage"] = json!("");
    ent["raison_sociale"] = json!("SARL L'ÉCLAT DU SOIR");
    ent["signataire"] = json!({"nom": "SAND", "prenom": "éLODIE-anne"});
    ecrire(&racine, "config/entreprise.json", &ent);
    let faux = Faux::nouveau();
    demande_avec(&racine, json!({"adresse": "z@exemple.invalid", "motdepasse": MOT_DE_PASSE}), &faux).unwrap();
    let octets = faux.recus.borrow()[0].clone();
    assert_eq!(entete(&octets, "Subject"), "Ta fiche pour SARL L'ÉCLAT DU SOIR");
    assert!(textes(&octets).0.ends_with("Bien à toi,\nÉlodie-anne\nSARL L'ÉCLAT DU SOIR\n"));
}

// ================================================================== fiches-relever

#[test]
fn la_bonne_reponse_cree_la_fiche() {
    tour();
    let racine = compagnie("relever-bonne", false);
    noter_demande(&racine, C1, ZOE, "Zoé", "<demande-1@exemple.invalid>");
    let brut = reponse(ZOE, C1, &[], "<rep-1@exemple.invalid>");
    let faux = Faux::avec(vec![("m1", brut.clone())]);
    let r = releve_avec(&racine, &faux);
    assert_eq!(cles(&r), ["attendues", "incidents", "recues"]);
    sans_donnees_personnelles(&racine, &r);
    assert_eq!(r["attendues"], json!(1));
    assert_eq!(r["incidents"], json!([]));
    let recue = &r["recues"][0];
    assert_eq!(cles(recue), ["cle", "code", "erreurs", "illisible", "nom"]);
    assert_eq!((recue["code"].as_str(), recue["cle"].as_str(), recue["nom"].as_str()), (Some(C1), Some("DUBOIS"), Some("Zoé Dubois")));
    assert_eq!(recue["illisible"], json!([]));
    // La fiche : ce que la personne a ecrit, puis ce que `enregistrer-fiche` complete (numero, majuscules, initiales).
    let sal = lire(&racine, "config/salaries.json");
    let f = &sal["DUBOIS"];
    assert_eq!(
        cles(f),
        [
            "actif", "adresse", "adresse_affichage", "code_nationalite_dsn", "code_postal", "code_salarie", "commune", "commune_affichage", "date_anciennete",
            "date_naissance", "departement_naissance", "email", "fiche_recue", "initiales_contrat", "lieu_naissance", "lieu_naissance_affichage", "nir", "nir_cle",
            "nom", "nom_affichage", "pays_naissance", "pays_nationalite", "prenom", "prenom_affichage",
        ]
    );
    assert_eq!(f["date_naissance"], json!("1991-03-14"));
    assert_eq!((f["nir"].as_str(), f["nir_cle"].as_str()), (Some("2910369123456"), Some("53")));
    assert_eq!((f["pays_naissance"].as_str(), f["code_nationalite_dsn"].as_str(), f["departement_naissance"].as_str()), (Some("FR"), Some("01"), Some("69")));
    assert_eq!(f["fiche_recue"], json!("Fiche remplie par la personne, par courriel, le 03/10/2026. Reste à choisir l'emploi."));
    assert_eq!((f["nom"].as_str(), f["prenom"].as_str(), f["adresse"].as_str()), (Some("DUBOIS"), Some("ZOE"), Some("5 RUE DES LILAS")));
    assert_eq!(f["actif"], json!(true));
    // La reponse recue est gardee telle quelle, octet pour octet.
    assert_eq!(std::fs::read(racine.join("sorties/fiches-salaries").join(format!("Fiche_{}.eml", C1))).unwrap(), brut);
    // Le journal : la ligne porte la date de la reponse (heure de Paris), pas celle de la releve.
    let l = lignes_du_journal(&racine, "fiche-recue");
    assert_eq!(l.len(), 1);
    assert_eq!(l[0]["quand"], json!("2026-10-03T10:15:00"));
    assert_eq!(l[0]["origine"], json!("courriel m1"));
    assert_eq!((l[0]["qui"].as_str(), l[0]["destinataire"].as_str(), l[0]["accepte"].as_bool()), (Some("interface"), Some(ZOE), Some(true)));
    assert_eq!(l[0]["reponse"], json!("fiche creee par courriel"));
    assert_eq!(l[0]["detail"], json!({"code": C1, "salarie": "DUBOIS", "illisible": [], "erreurs": recue["erreurs"]}));
    // La demande est close : une deuxieme releve n'a plus rien a attendre, ni a chercher.
    let r2 = releve_avec(&racine, &faux);
    assert_eq!((r2["attendues"].clone(), r2["recues"].clone()), (json!(0), json!([])));
    assert_eq!(lignes_du_journal(&racine, "fiche-recue").len(), 1);
}

#[test]
fn les_heures_de_la_reponse_se_lisent_a_paris() {
    tour();
    for (date, attendu, jour) in [
        ("Fri, 02 Oct 2026 23:30:00 -0500", "2026-10-03T06:30:00", "03/10/2026"),
        ("Sat, 03 Oct 2026 08:15:00 +0000", "2026-10-03T10:15:00", "03/10/2026"),
        ("Mon, 05 Jan 2026 10:15:00 +0100", "2026-01-05T10:15:00", "05/01/2026"),
        // « -0000 » est une heure UT (RFC 5322 § 3.3, `atelier/reference/sources/ietf/LISEZ-MOI.md`) : 12 h 15 a Paris.
        ("Sat, 03 Oct 2026 10:15:00 -0000", "2026-10-03T12:15:00", "03/10/2026"),
        ("Sat, 03 Oct 2026 22:30:00 +0000", "2026-10-04T00:30:00", "04/10/2026"),
    ] {
        let racine = compagnie("relever-date", false);
        noter_demande(&racine, C1, ZOE, "Zoé", "<demande-1@exemple.invalid>");
        let faux = Faux::avec(vec![("m1", message(ZOE, &reponses(C1, &[]), date, "<rep-1@exemple.invalid>"))]);
        releve_avec(&racine, &faux);
        assert_eq!(lignes_du_journal(&racine, "fiche-recue")[0]["quand"], json!(attendu), "{}", date);
        assert!(lire(&racine, "config/salaries.json")["DUBOIS"]["fiche_recue"].as_str().unwrap().contains(&format!("le {}.", jour)), "{}", date);
    }
}

#[test]
fn notre_propre_envoi_n_est_jamais_pris_pour_une_reponse() {
    tour();
    let racine = compagnie("relever-notre-envoi", false);
    noter_demande(&racine, C1, ZOE, "Zoé", "<demande-1@exemple.invalid>");
    // Le message de la demande (meme Message-ID, le code dans le texte) puis une copie dont l'identifiant est autre mais qui
    // porte « Remplir ma fiche » : les deux sont ecartes, sans incident.
    let nous = message("boite@exemple.invalid", &format!("Remplir ma fiche\n{}", reponses(C1, &[])), "Sat, 03 Oct 2026 09:00:00 +0200", "<demande-1@exemple.invalid>");
    let copie = message("boite@exemple.invalid", &format!("Remplir ma fiche\n{}", reponses(C1, &[])), "Sat, 03 Oct 2026 09:00:00 +0200", "<autre@exemple.invalid>");
    let faux = Faux::avec(vec![("m0", nous), ("m0b", copie)]);
    let r = releve_avec(&racine, &faux);
    assert_eq!((r["attendues"].clone(), r["recues"].clone(), r["incidents"].clone()), (json!(1), json!([]), json!([])));
    assert!(lignes_du_journal(&racine, "fiche-recue").is_empty());
    // Notre envoi dont l'identifiant est celui qu'on a garde avec la demande est ecarte avant meme d'etre lu.
    let mut faux = Faux::avec(vec![("m0", message("boite@exemple.invalid", "x", "Sat, 03 Oct 2026 09:00:00 +0200", "<demande-1@exemple.invalid>"))]);
    faux.lecture_en_echec = vec!["m0".to_string()];
    assert_eq!(releve_avec(&racine, &faux)["incidents"].as_array().unwrap().len(), 1, "un message illisible est un incident");
    // Le salarie, c'est nous (essai de la compagnie) : sa reponse, depuis la meme adresse, est retenue.
    let racine = compagnie("relever-salarie-nous", false);
    noter_demande(&racine, C1, "boite@exemple.invalid", "Boris", "<demande-1@exemple.invalid>");
    let faux = Faux::avec(vec![
        ("m0", message("boite@exemple.invalid", &format!("Remplir ma fiche\n{}", reponses(C1, &[])), "Sat, 03 Oct 2026 09:00:00 +0200", "<demande-1@exemple.invalid>")),
        ("m1", reponse("Boite <BOITE@exemple.invalid>", C1, &[("prenom", "Boris"), ("nom", "Lenoir")], "<rep-1@exemple.invalid>")),
    ]);
    let r = releve_avec(&racine, &faux);
    assert_eq!(r["recues"][0]["cle"], json!("LENOIR"));
    // Et nous ecrivant depuis la boite pour une demande a une autre adresse : ecarte, sans incident.
    let racine = compagnie("relever-nous-autre", false);
    noter_demande(&racine, C1, ZOE, "Zoé", "<demande-1@exemple.invalid>");
    let r = releve_avec(&racine, &Faux::avec(vec![("m1", reponse("boite@exemple.invalid", C1, &[], "<rep-1@exemple.invalid>"))]));
    assert_eq!((r["recues"].clone(), r["incidents"].clone()), (json!([]), json!([])));
}

#[test]
fn le_code_doit_etre_dans_ce_que_la_personne_a_ecrit() {
    tour();
    // Seulement dans la citation : ni l'une ni l'autre des mises en forme n'est une reponse.
    let sans_code = reponses(C1, &[]).split("\r\nCode").next().unwrap().to_string();
    for (nom, citation) in [
        ("gmail", format!("\n\nLe sam. 3 oct. 2026 à 09:00, Le Fil tendu <boite@exemple.invalid> a écrit :\n> Remplir ma fiche\n> Code (ne pas modifier) : {}\n", C1)),
        ("outlook", format!("\n\n-----Original Message-----\nFrom: Le Fil tendu <boite@exemple.invalid>\nCode (ne pas modifier) : {}\n", C1)),
        ("outlook-fr", format!("\n\n________________________________\nDe : Le Fil tendu\nCode (ne pas modifier) : {}\n", C1)),
        ("apple", format!("\n\nOn Oct 3, 2026, at 9:00 AM, Le Fil tendu wrote:\n\nCode (ne pas modifier) : {}\n", C1)),
    ] {
        let racine = compagnie("relever-citation", false);
        noter_demande(&racine, C1, ZOE, "Zoé", "<demande-1@exemple.invalid>");
        let faux = Faux::avec(vec![("m1", message(ZOE, &format!("{}{}", sans_code, citation), "Sat, 03 Oct 2026 10:15:00 +0200", "<rep-1@exemple.invalid>"))]);
        let r = releve_avec(&racine, &faux);
        assert_eq!(r["recues"], json!([]), "{}", nom);
        assert_eq!(r["incidents"], json!([]), "{}", nom);
    }
    // Seulement avec le bouton « Remplir ma fiche » dans le texte : un envoi du logiciel, pas une reponse.
    let racine = compagnie("relever-bouton", false);
    noter_demande(&racine, C1, ZOE, "Zoé", "<demande-1@exemple.invalid>");
    let faux = Faux::avec(vec![("m1", message(ZOE, &format!("Remplir ma fiche\n{}", reponses(C1, &[])), "Sat, 03 Oct 2026 10:15:00 +0200", "<rep-1@exemple.invalid>"))]);
    assert_eq!(releve_avec(&racine, &faux)["recues"], json!([]));
    // La citation apres les reponses : les reponses comptent, la citation non (le code est avant).
    let racine = compagnie("relever-citation-apres", false);
    noter_demande(&racine, C1, ZOE, "Zoé", "<demande-1@exemple.invalid>");
    let corps = format!("{}\n\nLe sam. 3 oct. 2026 à 09:00, Le Fil tendu a écrit :\n> Adresse (numéro et rue) : 1 rue de la Citation\n", reponses(C1, &[]));
    releve_avec(&racine, &Faux::avec(vec![("m1", message(ZOE, &corps, "Sat, 03 Oct 2026 10:15:00 +0200", "<rep-1@exemple.invalid>"))]));
    assert_eq!(lire(&racine, "config/salaries.json")["DUBOIS"]["adresse_affichage"], json!("5 rue des Lilas"));
    // Le code en minuscules, ou dans l'objet seul : les minuscules se reconnaissent, l'objet seul non.
    let racine = compagnie("relever-minuscules", false);
    noter_demande(&racine, C1, ZOE, "Zoé", "<demande-1@exemple.invalid>");
    let faux = Faux::avec(vec![("m1", message(ZOE, &reponses(&C1.to_lowercase(), &[]), "Sat, 03 Oct 2026 10:15:00 +0200", "<rep-1@exemple.invalid>"))]);
    assert_eq!(releve_avec(&racine, &faux)["recues"].as_array().unwrap().len(), 1);
    let racine = compagnie("relever-objet", false);
    noter_demande(&racine, C1, ZOE, "Zoé", "<demande-1@exemple.invalid>");
    let objet_seul = format!("From: {}\r\nSubject: Re: {}\r\nMessage-ID: <rep@exemple.invalid>\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{}", ZOE, C1, sans_code);
    assert_eq!(releve_avec(&racine, &Faux::avec(vec![("m1", objet_seul.into_bytes())]))["recues"], json!([]));
}

#[test]
fn une_reponse_d_une_autre_adresse_est_un_incident_et_n_est_pas_retenue() {
    tour();
    for de in ["intrus@exemple.invalid", "Intrus <INTRUS@exemple.invalid>"] {
        let racine = compagnie("relever-autre-adresse", false);
        noter_demande(&racine, C1, ZOE, "Zoé", "<demande-1@exemple.invalid>");
        let r = releve_avec(&racine, &Faux::avec(vec![("m1", reponse(de, C1, &[], "<rep-1@exemple.invalid>"))]));
        assert_eq!(r["recues"], json!([]));
        let i = r["incidents"].as_array().unwrap();
        assert!(i.len() == 1 && i[0].as_str().unwrap().contains(C1) && i[0].as_str().unwrap().contains("intrus@exemple.invalid"), "{:?}", i);
        assert!(!racine.join("sorties/fiches-salaries").exists());
    }
    // L'adresse de la reponse peut s'ecrire avec un nom d'affichage ou des majuscules.
    for de in ["ZOE@EXEMPLE.INVALID", "Zoé Dubois <zoe@exemple.invalid>", "\"Dubois, Zoé\" <zoe@exemple.invalid>", "<zoe@exemple.invalid>"] {
        let racine = compagnie("relever-adresse-ecrite", false);
        noter_demande(&racine, C1, ZOE, "Zoé", "<demande-1@exemple.invalid>");
        let r = releve_avec(&racine, &Faux::avec(vec![("m1", reponse(de, C1, &[], "<rep-1@exemple.invalid>"))]));
        assert_eq!(r["recues"].as_array().unwrap().len(), 1, "{}", de);
    }
    // Un premier message d'un intrus n'empeche pas la bonne reponse qui suit.
    let racine = compagnie("relever-intrus-puis-bonne", false);
    noter_demande(&racine, C1, ZOE, "Zoé", "<demande-1@exemple.invalid>");
    let r = releve_avec(&racine, &Faux::avec(vec![("m1", reponse("intrus@exemple.invalid", C1, &[], "<a@exemple.invalid>")), ("m2", reponse(ZOE, C1, &[], "<b@exemple.invalid>")), ("m3", reponse(ZOE, C1, &[("prenom", "Autre")], "<c@exemple.invalid>"))]));
    assert_eq!((r["recues"].as_array().unwrap().len(), r["incidents"].as_array().unwrap().len()), (1, 1));
    assert_eq!(r["recues"][0]["nom"], json!("Zoé Dubois"), "la premiere reponse valable est retenue");
}

#[test]
fn une_reponse_sans_nom_ni_prenom_ne_cree_rien() {
    tour();
    let racine = compagnie("relever-sans-nom", false);
    noter_demande(&racine, C1, ZOE, "Zoé", "<demande-1@exemple.invalid>");
    let r = releve_avec(&racine, &Faux::avec(vec![("m1", reponse(ZOE, C1, &[("prenom", ""), ("nom", "")], "<rep-1@exemple.invalid>"))]));
    assert_eq!(r["recues"], json!([]));
    let i = r["incidents"].as_array().unwrap();
    assert!(i.len() == 1 && i[0].as_str().unwrap().contains(C1) && i[0].as_str().unwrap().contains(ZOE), "{:?}", i);
    assert!(lignes_du_journal(&racine, "fiche-recue").is_empty());
    assert!(!racine.join("sorties/fiches-salaries").exists(), "rien n'est garde non plus");
    // Un nom seul suffit.
    let racine = compagnie("relever-nom-seul", false);
    noter_demande(&racine, C1, ZOE, "Zoé", "<demande-1@exemple.invalid>");
    let r = releve_avec(&racine, &Faux::avec(vec![("m1", reponse(ZOE, C1, &[("prenom", "")], "<rep-1@exemple.invalid>"))]));
    assert_eq!(r["recues"][0]["nom"], json!("Dubois"));
}

#[test]
fn une_reponse_mal_remplie_laisse_les_cases_vides_et_le_dit() {
    tour();
    let racine = compagnie("relever-mal-remplie", false);
    noter_demande(&racine, C1, ZOE, "Zoé", "<demande-1@exemple.invalid>");
    let mal = [("naissance", "le 3 mai"), ("code_postal", "6900"), ("nir", "29103691234567"), ("pays", "Atlantide"), ("nationalite", "martienne")];
    let r = releve_avec(&racine, &Faux::avec(vec![("m1", reponse(ZOE, C1, &mal, "<rep-1@exemple.invalid>"))]));
    let illisible = json!([
        "date naissance : « le 3 mai »",
        "code postal : « 6900 »",
        "numéro de sécurité sociale : « 29103691234567 »",
        "pays de naissance : « Atlantide »",
        "nationalité : « martienne »"
    ]);
    assert_eq!(r["recues"][0]["illisible"], illisible);
    let f = &lire(&racine, "config/salaries.json")["DUBOIS"];
    for absent in ["date_naissance", "code_postal", "nir", "pays_naissance", "code_nationalite_dsn", "pays_nationalite", "departement_naissance"] {
        assert!(f.get(absent).is_none(), "{} ne se devine pas", absent);
    }
    assert_eq!(
        f["fiche_recue"],
        json!("Fiche remplie par la personne, par courriel, le 03/10/2026. Reste à choisir l'emploi. Non lu, à compléter : date naissance : « le 3 mai » ; code postal : « 6900 » ; numéro de sécurité sociale : « 29103691234567 » ; pays de naissance : « Atlantide » ; nationalité : « martienne ».")
    );
    assert_eq!(lignes_du_journal(&racine, "fiche-recue")[0]["detail"]["illisible"], illisible);
    // La fiche est ecrite MEME AVEC DES ERREURS (`forcer`) : il lui manque l'emploi, elle n'en est pas moins gardee.
    assert!(r["recues"][0]["erreurs"].as_u64().unwrap() > 0);
}

#[test]
fn la_ligne_du_sexe_se_lit_sans_numero_de_securite_sociale() {
    tour();
    // Sans NIR, la reponse a la ligne « femme ou homme ? » donne le sexe : celle de la personne. Avec un NIR, la ligne ne
    // sert pas.
    for (reponse_sexe, nir, attendu) in [("homme", "", Some("1")), ("femme", "", Some("2")), ("M", "", Some("1")), ("F", "", Some("2")), ("", "", None), ("homme", "291036912345653", None)] {
        let racine = compagnie("relever-sexe", false);
        noter_demande(&racine, C1, ZOE, "Zoé", "<demande-1@exemple.invalid>");
        releve_avec(&racine, &Faux::avec(vec![("m1", reponse(ZOE, C1, &[("sexe", reponse_sexe), ("nir", nir)], "<rep-1@exemple.invalid>"))]));
        let f = &lire(&racine, "config/salaries.json")["DUBOIS"];
        assert_eq!(f.get("sexe").and_then(Value::as_str), attendu, "{:?} {:?}", reponse_sexe, nir);
    }
    let racine = compagnie("relever-sexe-illisible", false);
    noter_demande(&racine, C1, ZOE, "Zoé", "<demande-1@exemple.invalid>");
    let r = releve_avec(&racine, &Faux::avec(vec![("m1", reponse(ZOE, C1, &[("sexe", "peu importe"), ("nir", "")], "<rep-1@exemple.invalid>"))]));
    assert_eq!(r["recues"][0]["illisible"], json!(["sexe : « peu importe »"]));
}

#[test]
fn une_fiche_qui_existe_deja_n_est_completee_que_dans_ses_cases_vides() {
    tour();
    let racine = compagnie("relever-existante", false);
    let mut sal = lire(&racine, "config/salaries.json");
    sal["ZOE_EXISTE"] = json!({"nom": "DUBOIS", "prenom": "ZOE", "prenom_affichage": "Zoé", "nom_affichage": "Dubois", "email": "ZOE@Exemple.Invalid ", "emploi": "COMEDIEN",
        "statut": "artiste", "actif": true, "code_salarie": "9", "initiales_contrat": "DU", "adresse_affichage": "1 rue Pleine", "code_postal": "75001"});
    ecrire(&racine, "config/salaries.json", &sal);
    noter_demande(&racine, C1, ZOE, "Zoé", "<demande-1@exemple.invalid>");
    let r = releve_avec(&racine, &Faux::avec(vec![("m1", reponse(ZOE, C1, &[], "<rep-1@exemple.invalid>"))]));
    assert_eq!(r["recues"][0]["cle"], json!("ZOE_EXISTE"), "la fiche de cette adresse, sans en creer une autre");
    let f = &lire(&racine, "config/salaries.json")["ZOE_EXISTE"];
    assert_eq!(f["adresse_affichage"], json!("1 rue Pleine"), "ce qu'elle porte a peut-etre deja ete declare");
    assert_eq!(f["code_postal"], json!("75001"));
    assert_eq!(f["commune_affichage"], json!("Lyon"), "une case vide est completee");
    assert_eq!(f["emploi"], json!("COMEDIEN"));
    assert!(f["fiche_recue"].as_str().unwrap().starts_with("Fiche remplie par la personne"));
    assert_eq!(lignes_du_journal(&racine, "fiche-recue")[0]["reponse"], json!("fiche completee par courriel"));
    assert_eq!(lire(&racine, "config/salaries.json").as_object().unwrap().keys().filter(|k| k.starts_with("ZOE") || k.starts_with("DUBOIS")).count(), 1);
}

#[test]
fn deux_demandes_deux_reponses_dans_la_meme_releve() {
    tour();
    let racine = compagnie("relever-deux", false);
    noter_demande(&racine, C1, ZOE, "Zoé", "<demande-1@exemple.invalid>");
    noter_demande(&racine, C2, "paul@exemple.invalid", "Paul", "<demande-2@exemple.invalid>");
    let faux = Faux::avec(vec![
        ("m2", reponse("paul@exemple.invalid", C2, &[("prenom", "Paul"), ("nom", "Martin"), ("nir", ""), ("lieu", "Nice")], "<b@exemple.invalid>")),
        ("m1", reponse(ZOE, C1, &[], "<a@exemple.invalid>")),
    ]);
    let r = releve_avec(&racine, &faux);
    assert_eq!(r["attendues"], json!(2));
    let noms: Vec<&str> = r["recues"].as_array().unwrap().iter().map(|x| x["nom"].as_str().unwrap()).collect();
    assert_eq!(noms, ["Zoé Dubois", "Paul Martin"], "dans l'ordre des demandes, les plus anciennes d'abord");
    assert_eq!(lignes_du_journal(&racine, "fiche-recue").len(), 2);
    // Garde tel quel (decision de Leandre) : les fiches ne sont pas relues entre deux reponses ; deux reponses d'une meme
    // adresse dans la meme releve donnent deux fiches.
    let racine = compagnie("relever-meme-adresse", false);
    noter_demande(&racine, C1, ZOE, "Zoé", "<demande-1@exemple.invalid>");
    noter_demande(&racine, C2, ZOE, "Zoé", "<demande-2@exemple.invalid>");
    let faux = Faux::avec(vec![("m1", reponse(ZOE, C1, &[], "<a@exemple.invalid>")), ("m2", reponse(ZOE, C2, &[], "<b@exemple.invalid>"))]);
    let r = releve_avec(&racine, &faux);
    assert_eq!(r["recues"].as_array().unwrap().len(), 2);
    let cles: Vec<String> = lire(&racine, "config/salaries.json").as_object().unwrap().keys().filter(|k| k.contains("DUBOIS")).cloned().collect();
    assert_eq!(cles.len(), 2, "{:?}", cles);
}

#[test]
fn les_demandes_attendues_sont_les_acceptees_et_pas_encore_revenues() {
    tour();
    let racine = compagnie("relever-attendues", false);
    noter_demande(&racine, C1, ZOE, "Zoé", "<demande-1@exemple.invalid>");
    noter_demande(&racine, C2, "paul@exemple.invalid", "Paul", "");
    let a = rfc::fiches_attendues(&contexte(&racine)).unwrap();
    assert_eq!(a.iter().map(|x| x.code.as_str()).collect::<Vec<_>>(), [C1, C2]);
    assert_eq!((a[0].adresse.as_str(), a[0].prenom.as_str(), a[0].quand.as_str(), a[0].envoyes.clone()), (ZOE, "Zoé", "2026-10-02T09:00:00", vec!["<demande-1@exemple.invalid>".to_string()]));
    assert!(a[1].envoyes.is_empty(), "sans identifiant garde, rien n'est ecarte par lui");
    // Une demande a laquelle on renonce se retire (`fiche-oublier-demande`).
    let mut ligne = lignes_du_journal(&racine, "fiche-demande")[0].clone();
    ligne["accepte"] = json!(false);
    let mut tout = journal(&racine);
    tout.push(json!({"quoi": "fiche-recue", "quand": "2026-10-02T10:00:00", "origine": "", "mois": null, "fichier": null, "empreinte": null, "mode": "reel", "qui": "interface", "accepte": true,
        "destinataire": "", "flux": "", "retours": [], "reponse": "demande abandonnee, sans reponse", "trace": null, "detail": {"code": C1, "abandon": true}}));
    ecrire(&racine, "journal/depots.json", &Value::Array(tout));
    let a = rfc::fiches_attendues(&contexte(&racine)).unwrap();
    assert_eq!(a.iter().map(|x| x.code.as_str()).collect::<Vec<_>>(), [C2]);
    // Sans demande, la releve ne rend que sa reponse vide, sans ouvrir la boite.
    let racine = compagnie("relever-sans-demande", false);
    let faux = Faux::avec(vec![("m1", reponse(ZOE, C1, &[], "<rep@exemple.invalid>"))]);
    let r = releve_avec(&racine, &faux);
    assert_eq!((r["attendues"].clone(), r["recues"].clone(), r["incidents"].clone()), (json!(0), json!([]), json!([])));
    assert_eq!(faux.recherches(), 0);
    // Une demande par code : une recherche chacune.
    let racine = compagnie("relever-recherches", false);
    noter_demande(&racine, C1, ZOE, "Zoé", "<demande-1@exemple.invalid>");
    noter_demande(&racine, C2, "paul@exemple.invalid", "Paul", "<demande-2@exemple.invalid>");
    let faux = Faux::avec(vec![]);
    releve_avec(&racine, &faux);
    assert_eq!(faux.recherches(), 2);
}

#[test]
fn la_boite_absente_est_un_incident_pas_une_exception() {
    tour();
    let racine = compagnie("relever-boite-absente", false);
    noter_demande(&racine, C1, ZOE, "Zoé", "<demande-1@exemple.invalid>");
    let r = releve_telle_quelle(&racine);
    assert_eq!(r["attendues"], json!(1));
    assert_eq!(r["recues"], json!([]));
    assert_eq!(r["incidents"].as_array().unwrap().len(), 1, "sans boite, un incident le dit");
    // Sans demande, aucune boite n'est necessaire et rien n'est dit.
    let racine = compagnie("relever-boite-absente-sans-demande", false);
    assert_eq!(releve_telle_quelle(&racine)["incidents"], json!([]));
}

#[test]
fn une_recherche_ou_une_lecture_qui_echoue_est_un_incident() {
    tour();
    let racine = compagnie("relever-recherche-echoue", false);
    noter_demande(&racine, C1, ZOE, "Zoé", "<demande-1@exemple.invalid>");
    noter_demande(&racine, C2, "paul@exemple.invalid", "Paul", "<demande-2@exemple.invalid>");
    let mut faux = Faux::avec(vec![("m1", reponse(ZOE, C1, &[], "<a@exemple.invalid>"))]);
    faux.recherche_en_echec = true;
    let r = releve_avec(&racine, &faux);
    // La recherche echoue : la releve s'arrete (les autres codes ne seraient pas mieux servis).
    let i = r["incidents"].as_array().unwrap();
    assert!(i.len() == 1 && i[0].as_str().unwrap().contains(C1), "{:?}", i);
    assert_eq!(faux.recherches(), 1);
    assert_eq!(r["recues"], json!([]));
    // Une lecture qui echoue : incident, et la suite est lue.
    let racine = compagnie("relever-lecture-echoue", false);
    noter_demande(&racine, C1, ZOE, "Zoé", "<demande-1@exemple.invalid>");
    let mut faux = Faux::avec(vec![("m0", message("x@exemple.invalid", "rien", "Sat, 03 Oct 2026 10:00:00 +0200", "<x@exemple.invalid>")), ("m1", reponse(ZOE, C1, &[], "<a@exemple.invalid>"))]);
    faux.lecture_en_echec = vec!["m0".to_string()];
    let r = releve_avec(&racine, &faux);
    let i = r["incidents"].as_array().unwrap();
    assert!(i.len() == 1 && i[0].as_str().unwrap().contains("m0"), "{:?}", i);
    assert_eq!(r["recues"].as_array().unwrap().len(), 1);
}

#[test]
fn la_releve_de_la_veille_se_note_sous_son_nom() {
    tour();
    let racine = compagnie("relever-veille", false);
    noter_demande(&racine, C1, ZOE, "Zoé", "<demande-1@exemple.invalid>");
    let mut faux = Faux::avec(vec![("m1", reponse(ZOE, C1, &[], "<a@exemple.invalid>"))]);
    // La veille (lot I) appelle la meme fonction avec son `qui`.
    let r = rfc::relever_fiches(&contexte(&racine), Some(&mut faux), "veille").unwrap();
    assert_eq!((r.attendues, r.recues.len(), r.incidents.len()), (1, 1, 0));
    assert_eq!(r.resume(), "1 fiche(s) salarié reçue(s) : Zoé Dubois");
    assert_eq!(lignes_du_journal(&racine, "fiche-recue")[0]["qui"], json!("veille"));
    assert_eq!(rfc::ReleveFiches::default().resume(), "");
}

// ================================================================== la compagnie fictive

/// Date la demande (la ligne du journal) `secondes` avant maintenant, a l'heure du moteur (le jour pose).
fn dater_la_demande(racine: &Path, secondes: i64) {
    let t = socle::temps::secondes() as i64 - secondes;
    let quand = socle::paris::iso_secondes(t as f64);
    let mut lignes = journal(racine);
    for l in lignes.iter_mut().filter(|l| l["quoi"] == json!("fiche-demande")) {
        l["quand"] = json!(quand);
        l["origine"] = json!("test");
    }
    ecrire(racine, "journal/depots.json", &Value::Array(lignes));
}

#[test]
fn dans_une_fictive_la_personne_fictive_repond_et_la_vraie_releve_la_lit() {
    tour();
    let racine = compagnie("fictive-releve", true);
    let a = demande_telle_quelle(&racine, json!({"adresse": "marie@exemple.invalid", "prenom": "Marie"})).unwrap()["code"].as_str().unwrap().to_string();
    demande_telle_quelle(&racine, json!({"adresse": "sans.prenom@exemple.invalid"})).unwrap();
    // Trop tot : elle n'a pas encore repondu (20 secondes).
    dater_la_demande(&racine, 2);
    let r = releve_telle_quelle(&racine);
    assert_eq!((r["attendues"].clone(), r["recues"].clone(), r["incidents"].clone()), (json!(2), json!([]), json!([])));
    assert!(lignes_du_journal(&racine, "fiche-recue").is_empty());
    // Une minute plus tard : les deux reponses sont la, lues par la vraie releve.
    dater_la_demande(&racine, 60);
    let r = releve_telle_quelle(&racine);
    assert_eq!(r["recues"].as_array().unwrap().len(), 2, "{}", r);
    assert_eq!(r["incidents"], json!([]));
    assert_eq!(r["recues"][0]["code"], json!(a));
    assert!(r["recues"][0]["nom"].as_str().unwrap().starts_with("Marie "), "le prenom de la demande sert a la reponse : {}", r["recues"][0]["nom"]);
    let sal = lire(&racine, "config/salaries.json");
    let nouveaux: Vec<&String> = sal.as_object().unwrap().keys().filter(|k| !["BRIOCHE", "SERAPHIN", "LEMERCIER"].contains(&k.as_str())).collect();
    assert_eq!(nouveaux.len(), 2);
    for k in nouveaux {
        let f = &sal[k.as_str()];
        assert!(f["nir"].as_str().unwrap().len() == 13 && f["email"].as_str().unwrap().ends_with("@exemple.invalid"), "{}", f);
        assert_eq!(f["code_nationalite_dsn"], json!("01"));
        assert!(f["fiche_recue"].as_str().unwrap().starts_with("Fiche remplie par la personne, par courriel"));
    }
    assert!(racine.join("sorties/fiches-salaries").join(format!("Fiche_{}.eml", a)).is_file());
    // Elles sont closes.
    assert_eq!(releve_telle_quelle(&racine)["attendues"], json!(0));
}

#[test]
fn dans_une_fictive_une_demande_sans_son_courriel_joue_est_quand_meme_repondue() {
    tour();
    let racine = compagnie("fictive-sans-bouton", true);
    demande_telle_quelle(&racine, json!({"adresse": "marie@exemple.invalid", "prenom": "Marie"})).unwrap();
    dater_la_demande(&racine, 60);
    std::fs::remove_dir_all(racine.join("courriels-simules")).unwrap();
    let r = releve_telle_quelle(&racine);
    assert_eq!(r["recues"].as_array().unwrap().len(), 1);
}
