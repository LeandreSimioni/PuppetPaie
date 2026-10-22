//! La ligne de commande, rangée par commande : ce que chacune fait sans réseau, et ses refus (une phrase et un code,
//! jamais une trace ni un nom d'exception). Chaque `paie` part de `env_clear` (ni mot de passe, ni mandataire), sur une
//! copie sous `CARGO_TARGET_TMPDIR`. Le module `atelier` (`--features atelier`) joue `paie-atelier` contre un faux
//! net-entreprises local.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use crate::outils::{compagnie, copier, fictive, dossier_de_test, dossier_du_processus, lire};
use serde_json::{json, Value};

const AUJOURDHUI: &str = "2026-10-08";

/// `logiciel/donnees` et `VERSION`, copiés une fois par processus : `prive/` voisin de `logiciel/` est la compagnie par
/// défaut, et `--dev` cherche son marqueur dans `cle/`. Jamais le dépôt.
fn monde() -> &'static Path {
    static MONDE: OnceLock<PathBuf> = OnceLock::new();
    MONDE.get_or_init(|| {
        let m = dossier_du_processus("commandes");
        let livre = crate::outils::code_livre();
        copier(&livre, &m.join("logiciel/donnees"));
        std::fs::copy(livre.join("../VERSION"), m.join("logiciel/VERSION")).unwrap();
        std::fs::create_dir_all(m.join("cle")).unwrap();
        std::fs::create_dir_all(m.join("home")).unwrap();
        m
    })
}

/// La configuration et le journal d'une compagnie fictive (avec ses DSN déposées si `sorties`), marqueur compris.
fn copie(nom_fictive: &str, test: &str, sorties: bool) -> PathBuf {
    let racine = dossier_de_test("commandes", test);
    if sorties {
        compagnie::entiere(nom_fictive, &racine);
    } else {
        compagnie::config_et_mois(nom_fictive, &[], &racine);
        std::fs::copy(fictive(nom_fictive).join("COPIE-DE-DEVELOPPEMENT.json"), racine.join("COPIE-DE-DEVELOPPEMENT.json")).unwrap();
    }
    racine
}

/// La compagnie fictive devenue compagnie ordinaire (déclarant et compte Urssaf posés).
fn ordinaire(test: &str) -> PathBuf {
    let racine = copie("fictive-3090-petite", test, false);
    compagnie::ordinaire(&racine);
    racine
}

struct Sortie {
    code: i32,
    out: String,
    err: String,
}

impl Sortie {
    fn derniere_ligne_err(&self) -> &str {
        self.err.lines().filter(|l| !l.trim().is_empty()).last().unwrap_or("")
    }

    fn json(&self) -> Value {
        serde_json::from_str(&self.out).unwrap_or_else(|e| panic!("sortie non JSON ({}) : {}", e, self.out))
    }

    /// Ni trace, ni nom d'exception, ni plantage à l'écran.
    fn propre(&self) {
        for mot in ["Traceback", "Error:", "AttributeError", "JSONDecodeError", "KeyError", "panicked"] {
            assert!(!self.err.contains(mot) && !self.out.contains(mot), "{} a l'ecran :\n{}\n{}", mot, self.out, self.err);
        }
    }
}

fn lancer(binaire: &str, donnees: Option<&Path>, args: &[&str], env: &[(&str, &str)]) -> Sortie {
    let m = monde();
    let mut c = Command::new(binaire);
    c.env_clear()
        .env(paie::garde_reseau::VARIABLE, "1") // le garde-fou reseau des essais : rien ne sort de la machine
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", m.join("home"))
        .env("PUPPETPAIE_CLE", m.join("cle"))
        .env("PUPPETPAIE_CODE", m.join("logiciel/donnees"))
        .env("PUPPETPAIE_AUJOURDHUI", AUJOURDHUI)
        .envs(env.iter().copied())
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(d) = donnees {
        c.env("PUPPETPAIE_DONNEES", d);
    }
    let p = c.output().unwrap();
    Sortie { code: p.status.code().unwrap_or(-1), out: String::from_utf8_lossy(&p.stdout).into_owned(), err: String::from_utf8_lossy(&p.stderr).into_owned() }
}

fn paie(cie: &Path, args: &[&str]) -> Sortie {
    lancer(env!("CARGO_BIN_EXE_paie"), Some(cie), args, &[])
}

fn ecrire(racine: &Path, chemin: &str, contenu: &str) {
    let p = racine.join(chemin);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, contenu).unwrap();
}

// ------------------------------------------------------------------ les dates des options

/// Une date illisible est un refus : code 1, rien sur la sortie, une phrase sans nom de classe, et l'incident est un
/// refus (pas un défaut du logiciel).
#[test]
fn une_date_illisible_est_un_refus() {
    let cie = copie("fictive-1285-petite", "dates", false);
    let mut appels = 0;
    for le in ["2027-13-45", "2027-02-30", "2027-04-31", "0000-01-01", "2027-1-5", "31/12/2026", "pas-une-date"] {
        for commande in ["echeancier", "rappels"] {
            let r = paie(&cie, &[commande, "--le", le]);
            assert_eq!(r.code, 1, "{} --le {} : {}", commande, le, r.err);
            assert!(r.out.is_empty() && !r.derniere_ligne_err().is_empty(), "{} --le {} : {}{}", commande, le, r.out, r.err);
            r.propre();
            appels += 1;
        }
    }
    let r = lancer(env!("CARGO_BIN_EXE_paie"), Some(&cie), &["retours", "--depuis", "pas-une-date"], &[("NET_ENTREPRISES_MOTDEPASSE", "faux-motdepasse-commandes-0002")]);
    assert_eq!(r.code, 1);
    assert!(r.err.contains("pas-une-date"), "le refus nomme la date lue : {}", r.err);
    appels += 1;
    let incidents = lire(&cie, "journal/incidents.json");
    assert_eq!(incidents.as_array().unwrap().len(), appels);
    assert!(incidents.as_array().unwrap().iter().all(|i| i["quoi"] == "refus"), "{}", incidents);
    // AAAAMMJJ se lit aussi.
    for le in ["20270315", "2027-03-15"] {
        let r = paie(&cie, &["echeancier", "--le", le]);
        assert_eq!(r.code, 0, "{}", r.err);
        assert!(r.out.starts_with("Nous sommes le lundi 15 mars 2027."), "{}", r.out);
    }
}

// ------------------------------------------------------------------ echeancier

/// Marquer une échéance faite, puis la défaire, rend l'échéancier tel qu'il était.
#[test]
fn une_echeance_se_marque_faite_puis_se_defait() {
    let cie = copie("fictive-1285-petite", "echeance", false);
    let avant = std::fs::read(cie.join("config/echeancier.json")).ok();
    let fait = paie(&cie, &["echeancier", "--fait", "fcap-svp:2026"]);
    assert_eq!(fait.code, 0, "--fait : {}", fait.err);
    assert_ne!(std::fs::read(cie.join("config/echeancier.json")).ok(), avant, "--fait n'a rien ecrit");
    let defait = paie(&cie, &["echeancier", "--defait", "fcap-svp:2026"]);
    assert_eq!(defait.code, 0, "--defait : {}", defait.err);
    let lu = |o: Option<Vec<u8>>| o.map(|b| serde_json::from_slice::<Value>(&b).unwrap()).unwrap_or(Value::Null);
    let marques = |v: &Value| v.get("fait").cloned().unwrap_or(Value::Null);
    assert_eq!(marques(&lu(std::fs::read(cie.join("config/echeancier.json")).ok())), marques(&lu(avant)));
}

// ------------------------------------------------------------------ droits

#[test]
fn droits_plafond_des_28_cachets_enseignement_et_naissance_absente() {
    let cie = copie("fictive-1285-petite", "droits", false);
    let fiches = lire(&cie, "config/salaries.json");
    let ks: Vec<String> = fiches.as_object().unwrap().keys().filter(|k| !k.starts_with('_')).cloned().collect();
    assert!(ks.len() >= 3);
    let ligne = |cle: &str, debut: &str, fin: &str, heures: i64, cachets: i64, ens: i64| {
        json!({"salarie": cle, "debut": debut, "fin": fin, "numero_objet": "266Z00000011", "heures": heures,
               "cachets": cachets, "jours": 20, "brut": "1000", "heures_enseignement": ens})
    };
    // sans paies, l'historique seul compte
    let engagements = vec![
        ligne(&ks[0], "2026-05-04", "2026-05-30", 0, 34, 0),    // 34 cachets en un mois : le plafond de 28 mord
        ligne(&ks[1], "2026-02-03", "2026-02-27", 250, 0, 150), // enseignement au-dela du plafond
        ligne(&ks[2], "2026-04-06", "2026-04-24", 120, 0, 100),
    ];
    crate::outils::ecrire(&cie, "historique/engagements.json", &json!({"engagements": engagements}));
    // la naissance : un salarie de 50 ans et plus (plafond haut), un sans date
    let mut fiches = fiches;
    fiches[&ks[1]]["date_naissance"] = json!("1970-03-01");
    fiches[&ks[1]]["anniversaire_droits"] = json!("2026-03-15");
    fiches[&ks[2]].as_object_mut().unwrap().remove("date_naissance");
    crate::outils::ecrire(&cie, "config/salaries.json", &fiches);

    let r = paie(&cie, &["droits", "--detail", "--mois", "2026-05"]);
    assert_eq!(r.code, 0, "{}", r.err);
    for attendu in [
        "retenus (28 par mois au plus)",
        "(6 cachet(s) depassent les 28 d'un mois (2026-05) :",
        "dont enseignement",
        "POUR L'OUVERTURE des droits",
        "retenues (plafond 120 h)",
        "(anniversaire des droits)",
        "retenues (plafond 70 h)",
        "(30 h d'enseignement depassent le plafond",
    ] {
        assert!(r.out.contains(attendu), "{} : {}", attendu, r.out);
    }
    let j = paie(&cie, &["droits", "--mois", "2026-05", "--json"]).json();
    assert_eq!((&j["commande"], &j["resultat"]["mois"]), (&json!("droits"), &json!("2026-05")));
    assert!(j["resultat"]["salaries"].as_array().unwrap().len() >= 3);
}

// ------------------------------------------------------------------ incidents

#[test]
fn incidents_regroupes_par_empreinte_et_lignes_abimees() {
    let cie = copie("fictive-1285-petite", "incidents", false);
    let incident = |quand: Value, ou: &str, genre: &str, resume: &str, quoi: &str| {
        json!({"quoi": quoi, "quand": quand, "version": "2026.10.02", "empreinte": format!("{:x}", genre.len() * 7919 + ou.len()),
               "ou": ou, "genre": genre, "resume": resume, "detail": {}, "trace": null, "caviarde": [],
               "envoi": {"etat": "en-attente", "quand": null}})
    };
    let lignes = json!([
        incident(json!("2026-09-12T18:18:08"), "calcul.py:614", "KeyError", "'taux_at_mp'", "logiciel"),
        incident(json!("2026-09-14T09:02:11"), "calcul.py:614", "KeyError", "'taux_at_mp'", "logiciel"),
        incident(json!("2026-10-01T22:40:00"), "calcul.py:614", "KeyError", "'taux_at_mp'", "logiciel"),
        incident(json!("2026-09-21T10:30:00"), "fiche/nir", "FicheRefusee", "NIR invalide", "refus"),
        incident(Value::Null, "x.py:1", "ValueError", "quand est null", "logiciel"),
    ]);
    crate::outils::ecrire(&cie, "journal/incidents.json", &lignes);
    let r = paie(&cie, &["incidents"]);
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(r.out.contains("KeyError x3"), "{}", r.out);
    assert!(r.out.contains("du 12/09/2026 à 18:18 au 01/10/2026 à 22:40"), "{}", r.out);
    assert!(r.out.contains("3 sorte(s) d'incident, 5 au total."), "{}", r.out);
    let j = paie(&cie, &["incidents", "--json", "--quoi", "logiciel"]).json();
    let regroupes = j["resultat"].as_array().unwrap();
    assert_eq!(regroupes.len(), 2);
    assert!(regroupes.iter().any(|g| g["combien"] == 3 && g["genre"] == "KeyError"));
    let r = paie(&cie, &["incidents", "--detail", "--quoi", "refus"]);
    assert!(r.out.contains("FicheRefusee") || r.out.contains("fiche/nir"), "{}", r.out);
    assert!(r.out.contains("1 incident(s)."), "{}", r.out);
    // un journal vide ou illisible : aucun incident, code 0
    for contenu in ["[]", "ceci n'est pas du JSON", "{}"] {
        ecrire(&cie, "journal/incidents.json", contenu);
        let r = paie(&cie, &["incidents"]);
        assert_eq!(r.code, 0, "{} : {}", contenu, r.err);
        assert!(r.out.starts_with("Aucun incident."), "{} : {}", contenu, r.out);
    }
}

// ------------------------------------------------------------------ version

#[test]
fn version_dit_la_signature_et_refuse_un_maj_json_illisible() {
    let cie = copie("fictive-1285-petite", "version", false);
    let r = paie(&cie, &["version", "--json"]);
    assert_eq!(r.code, 0, "{}", r.err);
    let j = r.json();
    assert_eq!(j["commande"], "version");
    assert_eq!(j["resultat"]["signee"], true, "la cle publique est ecrite en dur : toujours vrai");
    assert!(j["resultat"].get("atelier").is_none(), "le binaire livre ne parle pas de l'atelier : {}", r.out);
    let texte = paie(&cie, &["version"]);
    assert!(texte.out.starts_with("PuppetPaie ") && !texte.out.contains("ATELIER"), "{}", texte.out);
    // L'identifiant (JSON) garde AAAA.MM.JJ ; l'humain lit JJ.MM.AAAA (Leandre, 14/10/2026).
    let id = j["resultat"]["courante"].as_str().unwrap().to_string();
    let lue = paie::maj::version_lue(&id);
    assert!(id.len() == 10 && lue.len() == 10 && lue[6..] == id[..4], "{} -> {}", id, lue);
    assert!(texte.out.starts_with(&format!("PuppetPaie {}", lue)), "{}", texte.out);
    // un reglage de l'installation illisible : un refus (code 1, la phrase en derniere ligne), pas un silence
    let inst = crate::outils::installation(&cie.parent().unwrap().join("version-installation"), "2026.10.09");
    let code = crate::outils::code_de(&inst);
    let dans = |args: &[&str]| lancer(env!("CARGO_BIN_EXE_paie"), Some(&cie), args, &[("PUPPETPAIE_CODE", &code)]);
    ecrire(&inst, "maj/reglage.json", "{ceci n'est pas du json\n");
    let r = dans(&["version"]);
    assert_eq!(r.code, 1);
    assert!(r.derniere_ligne_err().starts_with("maj/reglage.json illisible"), "{}", r.err);
    std::fs::remove_file(inst.join("maj/reglage.json")).unwrap();
    assert_eq!(dans(&["version"]).code, 0);
}

/// Le depot des mises a jour d'une installation qui n'en pose pas : celui du moteur (`maj::DEPOT_PAR_DEFAUT`, lot P1),
/// la derniere Release du depot public -- un faux mandataire local voit la demande et refuse, rien ne sort. Un depot pose
/// dans le reglage de l'installation le remplace : c'est lui qu'on interroge. `paie maj` n'a besoin d'aucune compagnie.
#[test]
fn maj_cherche_au_depot_du_moteur_tant_qu_aucun_n_est_pose() {
    use crate::outils::{http_json, mandataire, serveur};
    let inst = crate::outils::installation(&dossier_de_test("commandes", "maj-depot"), "2026.10.09");
    let code = crate::outils::code_de(&inst);
    let (proxy, demandes) = mandataire();
    let r = lancer(env!("CARGO_BIN_EXE_paie"), None, &["maj", "--sans-telecharger"], &[(paie::garde_reseau::VARIABLE, &proxy), ("PUPPETPAIE_CODE", &code)]);
    assert_eq!(r.code, 0, "{}{}", r.out, r.err);
    assert!(r.out.contains("depot injoignable") && r.out.contains("https://github.com/LeandreSimioni/PuppetPaie/releases/latest/download/derniere.json"), "{}", r.out);
    assert_eq!(*demandes.lock().unwrap(), ["CONNECT github.com:443 HTTP/1.1"]);
    // Un depot pose : le moteur ne parle qu'a lui.
    let derniere = json!({"version": "2000.01.01", "archives": {"tout-systeme": {"archive": "a.zip", "sha256": "x", "signature": "s", "systemes": ["windows", "linux", "mac"]}}});
    let (base, recu) = serveur(move |_| Some(http_json(200, &derniere)));
    ecrire(&inst, "maj/reglage.json", &json!({"depot": base}).to_string());
    let r = lancer(env!("CARGO_BIN_EXE_paie"), None, &["maj", "--sans-telecharger"], &[("PUPPETPAIE_CODE", &code)]);
    assert!(r.out.contains("a jour"), "{}{}", r.out, r.err);
    assert_eq!(recu.lock().unwrap().iter().map(|q| q.chemin.clone()).collect::<Vec<_>>(), ["/derniere.json"]);
}

// ------------------------------------------------------------------ pas

#[test]
fn pas_amorcer_ecrit_une_fois_puis_ne_reecrit_rien() {
    let cie = copie("fictive-1285-petite", "amorcer", true);
    // le magasin remis a zero, et les DSN de la compagnie fictive rangees ou l'amorcage les cherche
    ecrire(&cie, "config/pas.json", r#"{"salaries": {}, "journal": []}"#);
    for f in std::fs::read_dir(cie.join("sorties")).unwrap().flatten().filter(|f| f.path().is_dir()) {
        for d in std::fs::read_dir(f.path()).unwrap().flatten().filter(|d| d.path().extension().is_some_and(|e| e == "dsn")) {
            let cible = cie.join("historique/coolpaie/DSN").join(f.file_name());
            std::fs::create_dir_all(&cible).unwrap();
            std::fs::copy(d.path(), cible.join(d.file_name())).unwrap();
        }
    }
    let r = paie(&cie, &["pas", "--amorcer"]);
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(r.out.contains("Amorcage depuis les DSN deposees (") && r.out.contains(" : premier releve - taux "), "{}", r.out);
    let octets = std::fs::read(cie.join("config/pas.json")).unwrap();
    let v: Value = serde_json::from_slice(&octets).unwrap();
    let un = v["salaries"].as_object().unwrap().values().next().expect("un salarie amorce");
    let r = un["releve_le"].as_str().unwrap();
    assert!(r.len() >= 10 && r.as_bytes()[4] == b'-' && r.as_bytes()[7] == b'-', "le jour du releve, que `pas::` relit en AAAA-MM-JJ : {}", r);
    assert!(un["source"].as_str().unwrap().starts_with("amorcage depuis les DSN deposees"));
    // la deuxieme fois : rien a changer, et le fichier n'est pas reecrit
    let r2 = paie(&cie, &["pas", "--amorcer"]);
    assert_eq!(r2.code, 0);
    assert!(r2.out.contains("rien a changer."), "{}", r2.out);
    assert_eq!(std::fs::read(cie.join("config/pas.json")).unwrap(), octets);
    // sans dossier de DSN : introuvable, code 1
    std::fs::remove_dir_all(cie.join("historique/coolpaie")).unwrap();
    let r3 = paie(&cie, &["pas", "--amorcer"]);
    assert_eq!(r3.code, 1, "{}{}", r3.out, r3.err);
}

// ------------------------------------------------------------------ la compagnie par defaut, --dev

#[test]
fn la_compagnie_par_defaut_est_prive_a_cote_du_code_et_dev_est_refuse_eteint() {
    let cie = copie("fictive-1285-petite", "defaut", false);
    let prive = monde().join("prive");
    let _ = std::fs::remove_dir_all(&prive);
    std::fs::rename(&cie, &prive).unwrap();
    let r = lancer(env!("CARGO_BIN_EXE_paie"), None, &["fiche"], &[]);
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(r.out.starts_with("Entreprise : "), "{}", r.out);
    assert_eq!(paie(&prive, &["fiche"]).out, r.out);
    // --dev : eteint par defaut (aucun .mode-developpeur.json dans cle/)
    let d = lancer(env!("CARGO_BIN_EXE_paie"), None, &["--dev", "fiche"], &[]);
    assert_eq!(d.code, 1);
    assert!(d.derniere_ligne_err().starts_with("Mode developpeur eteint : "), "{}", d.err);
    assert!(d.out.is_empty());
    std::fs::remove_dir_all(&prive).unwrap();
}

// ------------------------------------------------------------------ arrivee

#[test]
fn arrivee_donne_les_etapes_dans_l_ordre() {
    let cie = copie("fictive-1285-petite", "arrivee", false);
    let r = paie(&cie, &["arrivee", "--json"]);
    let j = r.json();
    let etapes = j["resultat"]["etapes"].as_array().unwrap();
    let cles: Vec<&str> = etapes.iter().map(|e| e["cle"].as_str().unwrap()).collect();
    assert_eq!(&cles[..6], ["entreprise", "import", "salaries", "effectif", "atmp", "pas"]);
    // `--json` rend toujours 0 ; le texte rend 1 s'il reste quelque chose a faire
    assert_eq!(r.code, 0);
    let reste = etapes.iter().any(|e| e["etat"] == "a_faire");
    let texte = paie(&cie, &["arrivee"]);
    assert_eq!(texte.code, if reste { 1 } else { 0 });
}

/// L'onglet s'appelle « Reprendre l'historique » (§217), et aucun logiciel n'est nommé à l'écran.
#[test]
fn arrivee_renvoie_a_l_onglet_reprendre_l_historique_sans_nommer_de_logiciel() {
    let cie = copie("fictive-1285-petite", "arrivee-export", false);
    std::fs::create_dir_all(cie.join("export").join("DSN")).unwrap();
    let j = paie(&cie, &["arrivee", "--json"]).json();
    let import = j["resultat"]["etapes"].as_array().unwrap().iter().find(|e| e["cle"] == "import").unwrap().clone();
    assert_eq!(import["etat"], "a_faire", "{}", import);
    assert!(import["commandes"][0].as_str().unwrap().starts_with("Configuration › Reprendre l'historique"), "{}", import);
    let texte = paie(&cie, &["arrivee"]).out;
    assert!(!texte.contains("CoolPaie") && !texte.contains("› Importer"), "{}", texte);
}

// ------------------------------------------------------------------ filet

#[test]
fn filet_sur_un_fichier_qui_n_est_pas_un_objet_refuse_proprement() {
    let cie = ordinaire("filet-liste");
    ecrire(&cie, "config/filet-dpae.json", "[]");
    for args in [&["filet"][..], &["filet", "etat"], &["filet", "--json"]] {
        let r = paie(&cie, args);
        assert_eq!(r.code, 1, "{:?} : {}{}", args, r.out, r.err);
        assert!(r.out.starts_with("! ") && r.out.contains("filet-dpae.json est illisible"), "{:?} : {}", args, r.out);
        assert!(r.err.is_empty(), "{:?} : {}", args, r.err);
        r.propre();
    }
    // Aucun incident de logiciel n'est note pour un fichier que l'utilisateur peut corriger.
    assert!(!cie.join("journal/incidents.json").exists());
}

#[test]
fn filet_suivies_sur_un_journal_illisible_refuse_proprement() {
    let cie = ordinaire("journal-illisible");
    ecrire(&cie, "journal/depots.json", "pas du json");
    let r = paie(&cie, &["filet", "suivies"]);
    assert_eq!(r.code, 1, "{}{}", r.out, r.err);
    assert!(r.out.starts_with("! Le journal des depots") && r.out.contains("est illisible"), "{}", r.out);
    assert!(r.err.is_empty(), "{}", r.err);
    r.propre();
}

// ------------------------------------------------------------------ google

/// La phrase et le code 2, comme tout refus de Google (le jeton sans `access_token` : `src/routes_google.rs`).
#[test]
fn google_sur_un_google_json_illisible_refuse_proprement() {
    let cie = ordinaire("google-json");
    for contenu in ["pas du json", "[1]", ""] {
        ecrire(&cie, "config/google.json", contenu);
        for args in [&["google"][..], &["google", "etat"]] {
            let r = paie(&cie, args);
            assert_eq!(r.code, 2, "{:?} : {}{}", args, r.out, r.err);
            assert!(r.out.contains("google.json est illisible"), "{}", r.out);
            assert!(r.err.is_empty(), "{}", r.err);
            r.propre();
        }
        let j = paie(&cie, &["google", "--json"]);
        assert_eq!(j.code, 2);
        let v = j.json();
        assert_eq!((&v["ok"], &v["erreur"]["type"]), (&json!(false), &json!("ErreurGoogle")));
    }
}

/// `paie restaurer` sans Drive relie : la phrase, la commande a lancer, le code 2, et rien d'ecrit (ni dans le dossier
/// donne, ni l'index de la compagnie). La restauration elle-meme : `fichiers/restauration.rs`, contre le faux Drive.
#[test]
fn restaurer_sans_drive_relie_refuse_et_n_ecrit_rien() {
    let cie = ordinaire("restaurer");
    let cible = dossier_de_test("commandes", "restaurer-cible").join("prive");
    let r = paie(&cie, &["restaurer", cible.to_str().unwrap()]);
    assert_eq!(r.code, 2, "{}{}", r.out, r.err);
    assert!(r.out.contains("Compte Google non autorise") && r.out.contains("paie google autoriser"), "{}", r.out);
    r.propre();
    assert!(!cible.exists() && !cie.join("config/.sauvegarde.json").exists());
    let j = paie(&cie, &["restaurer", "--json"]);
    assert_eq!(j.code, 2);
    assert_eq!((&j.json()["ok"], &j.json()["erreur"]["type"]), (&json!(false), &json!("ErreurGoogle")));
    // Elle figure dans l'aide.
    assert!(paie(&cie, &["--aide"]).out.contains("restaurer"));
}

// ------------------------------------------------------------------ sources (le poste de l'editeur)

/// Le poste de l'editeur : une compagnie ordinaire, et une installation dont le reglage dit `"editeur": true`, ses
/// donnees livrees portant un `sources.json` remplace par un manifeste fictif. Rend (racine, code).
fn poste_de_l_editeur(test: &str, sources: Value) -> (PathBuf, PathBuf) {
    let racine = ordinaire(test);
    let inst = racine.parent().unwrap().join(format!("{}-installation", test));
    let _ = std::fs::remove_dir_all(&inst);
    ecrire(&inst, "maj/reglage.json", &json!({"editeur": true}).to_string());
    let code = inst.join("logiciel").join("donnees");
    copier(&monde().join("logiciel/donnees"), &code);
    let manifeste = json!({"_quoi": "manifeste fictif", "sources": sources});
    std::fs::write(code.join("sources.json"), serde_json::to_string_pretty(&manifeste).unwrap()).unwrap();
    (racine, code)
}

fn source(url: &str, fichier: &str, contenu_lu: &[u8], verification: &str) -> Value {
    use sha2::{Digest, Sha256};
    let sha: String = Sha256::digest(contenu_lu).iter().map(|b| format!("{:02x}", b)).collect();
    json!({"titre": format!("Texte fictif {}", fichier), "editeur": "Editeur fictif", "url": url, "contenu": "un texte invente",
           "version": "2026-01-01", "releve_le": "2026-09-08", "fichier": fichier, "sha256": sha, "verification": verification,
           "nourrit": ["donnee-fictive.json"], "page_de_veille": "https://exemple.invalid/veille"})
}

fn sources(racine: &Path, code: &Path, args: &[&str]) -> Sortie {
    let mut tout = vec!["sources"];
    tout.extend_from_slice(args);
    lancer(env!("CARGO_BIN_EXE_paie"), Some(racine), &tout, &[("PUPPETPAIE_CODE", code.to_str().unwrap())])
}

/// La verification des sources est un outil de l'editeur (atelier/CLAUDE.md, « Publier une version ») : ailleurs que sur
/// son poste -- une compagnie, une compagnie fictive --, la commande refuse, rien n'est lu ni ecrit.
#[test]
fn sources_refusee_hors_du_poste_de_l_editeur() {
    let (cie, code) = poste_de_l_editeur("sources-refus", json!({}));
    let inst = code.parent().unwrap().parent().unwrap().to_path_buf();
    ecrire(&inst, "maj/reglage.json", &json!({"editeur": false}).to_string());
    let r = sources(&cie, &code, &["--verifier"]);
    assert_eq!(r.code, 1, "{}{}", r.out, r.err);
    assert!(r.out.is_empty() && r.err.contains("n'est pas celui de l'editeur"), "{}", r.err);
    r.propre();
    ecrire(&inst, "maj/reglage.json", &json!({"editeur": true}).to_string());
    let fictive = copie("fictive-3090-petite", "sources-fictive", false);
    let r = sources(&fictive, &code, &[]);
    assert_eq!(r.code, 1, "une copie ne verifie rien : {}", r.out);
    assert!(r.err.contains("Copie de developpement"), "{}", r.err);
    assert!(!inst.join("atelier/reference").exists() && !cie.join("reference").exists() && !fictive.join("reference").exists());
}

/// Sans reseau : la copie d'`atelier/reference/sources/` de l'installation (le depot, chez l'editeur : les textes du
/// logiciel, pas ceux d'une compagnie) confrontee au condensat du manifeste -- conforme, differente, absente -- et l'age du
/// releve au jour du poste. Une copie posee dans la compagnie n'est pas lue.
#[test]
fn sources_confronte_chaque_copie_au_condensat_du_manifeste() {
    let mut transcrite = source("https://exemple.invalid/d", "d.pdf", b"original", "manuelle");
    transcrite["transcription"] = json!("d.txt");
    let (cie, code) = poste_de_l_editeur(
        "sources-copies",
        json!({
            "a-conforme": source("https://exemple.invalid/a", "a.pdf", b"texte lu", "telechargement"),
            "b-differente": source("https://exemple.invalid/b", "sous/b.csv", b"texte lu", "telechargement"),
            "c-absente": source("https://exemple.invalid/c", "c.md", b"texte lu", "manuelle"),
            "d-transcrite": transcrite,
        }),
    );
    let inst = code.parent().unwrap().parent().unwrap().to_path_buf();
    ecrire(&inst, "atelier/reference/sources/a.pdf", "texte lu");
    ecrire(&inst, "atelier/reference/sources/sous/b.csv", "texte retouche a la main");
    ecrire(&cie, "reference/sources/c.md", "texte lu");
    ecrire(&inst, "atelier/reference/sources/d.txt", "transcription de l'original");
    let r = sources(&cie, &code, &["--json"]);
    assert_eq!(r.code, 0, "{}", r.err);
    let j = r.json();
    let l = j["resultat"]["sources"].as_array().unwrap();
    let vu: Vec<(Value, Value, Value)> = l.iter().map(|e| (e["ident"].clone(), e["copie_presente"].clone(), e["copie_conforme"].clone())).collect();
    assert_eq!(
        vu,
        [
            (json!("a-conforme"), json!(true), json!(true)),
            (json!("b-differente"), json!(true), json!(false)),
            (json!("c-absente"), json!(false), Value::Null),
            (json!("d-transcrite"), json!(false), Value::Null)
        ]
    );
    // L'original d'un organisme prive reste hors du depot : sa transcription est dite, sans condensat a confronter.
    assert_eq!((l[2]["transcription_presente"].clone(), l[3]["transcription_presente"].clone()), (Value::Null, json!(true)));
    assert_eq!(l[0]["age_jours"], json!(30), "releve le 2026-09-08, le poste est au {}", AUJOURDHUI);
    assert!(l.iter().all(|e| e["en_ligne"].is_null()), "sans --verifier, rien n'est retourne voir");
    let texte = sources(&cie, &code, &[]);
    assert!(texte.out.contains("copie conforme") && texte.out.contains("COPIE DIFFERENTE") && texte.out.contains("copie absente (atelier/reference/sources/c.md)")
            && texte.out.contains("original hors du depot public, transcription : atelier/reference/sources/d.txt"), "{}", texte.out);
    // Un manifeste a qui il manque un champ : un refus qui le nomme, pas une valeur devinee.
    std::fs::write(code.join("sources.json"), json!({"sources": {"x": {"titre": "x"}}}).to_string()).unwrap();
    let r = sources(&cie, &code, &[]);
    assert_eq!(r.code, 1);
    assert!(r.err.contains("« x »") && r.err.contains("fichier"), "{}", r.err);
}

/// `--verifier` retelecharge (ici d'un faux serveur local, et d'un port ferme) : `inchangee` quand le condensat colle,
/// `changee` quand il ne colle plus, `injoignable` quand rien ne repond, `manuelle` pour ce qui ne se reverifie pas.
/// `--relever` pose la nouvelle version A COTE, suffixee du jour ; la copie lue ne bouge pas.
#[test]
fn sources_verifier_compare_en_ligne_et_relever_pose_a_cote() {
    use crate::outils::{http, port_ferme, serveur};
    let (base, recu) = serveur(|q| Some(if q.chemin == "/a" { http(200, "text/plain", b"texte lu") } else { http(200, "text/csv", b"texte neuf") }));
    let (cie, code) = poste_de_l_editeur(
        "sources-verifier",
        json!({
            "a": source(&format!("{}/a", base), "a.pdf", b"texte lu", "telechargement"),
            "b": source(&format!("{}/b", base), "sous/b.csv", b"texte lu", "telechargement"),
            "c": source(&format!("{}/c", port_ferme()), "c.md", b"texte lu", "telechargement"),
            "d": source("https://exemple.invalid/d", "d.md", b"texte lu", "manuelle"),
        }),
    );
    let inst = code.parent().unwrap().parent().unwrap().to_path_buf();
    ecrire(&inst, "atelier/reference/sources/sous/b.csv", "texte lu");
    let r = sources(&cie, &code, &["--relever", "--json"]);
    assert_eq!(r.code, 0, "{}", r.err);
    let j = r.json();
    let etat: Vec<Value> = j["resultat"]["sources"].as_array().unwrap().iter().map(|e| e["en_ligne"].clone()).collect();
    assert_eq!(etat, [json!("inchangee"), json!("changee"), json!("injoignable"), json!("manuelle")]);
    let b = &j["resultat"]["sources"][1];
    assert!(b["detail"].as_str().unwrap().contains("donnee-fictive.json") && b["detail"].as_str().unwrap().contains("b.2026-10-08.csv"), "{}", b["detail"]);
    let copies = inst.join("atelier/reference/sources");
    assert_eq!(std::fs::read(copies.join("sous/b.csv")).unwrap(), b"texte lu", "la copie lue ne bouge pas");
    assert_eq!(std::fs::read(copies.join("sous/b.2026-10-08.csv")).unwrap(), b"texte neuf");
    assert!(!copies.join("a.2026-10-08.pdf").exists(), "une source inchangee ne se releve pas");
    assert!(!cie.join("reference").exists(), "rien ne s'ecrit dans la compagnie");
    assert_eq!(recu.lock().unwrap().iter().filter(|q| q.entetes.iter().any(|(k, v)| k == "user-agent" && v.starts_with("PuppetPaie"))).count(), 2);
    // Le texte : la marque de chaque etat, et la page a ouvrir pour ce qui se relit a la main.
    let t = sources(&cie, &code, &["--verifier"]);
    for marque in ["en ligne : OK", "en ligne : A RELIRE", "en ligne : ??", "en ligne : A LA MAIN", "ouvrir : https://exemple.invalid/veille"] {
        assert!(t.out.contains(marque), "{} : {}", marque, t.out);
    }
}

// ================================================================== paie-atelier, contre un faux serveur local
#[cfg(feature = "atelier")]
mod atelier {
    use super::*;
    use crate::outils::{http, serveur};

    const FAUX_MOTDEPASSE: &str = "faux-motdepasse-commandes-0001";

    /// Un faux net-entreprises : `coupe` ferme la connexion sans réponse à la consultation ; sinon il liste, le
    /// 07/10/2026, un retour `nature` (R1) que `telecharger` rend avec `contenu`.
    fn faux_dsn(nature: &'static str, contenu: &'static [u8], coupe: bool) -> String {
        serveur(move |q| {
            let c = format!("{}?{}", q.chemin, q.requete);
            Some(if c.contains("authentifier") {
                http(200, "text/plain", b"JETON-FICTIF")
            } else if c.contains("lister-retours-declarant") {
                if coupe {
                    return None;
                }
                if !c.contains("20261007") {
                    return Some(http(404, "text/plain", b"Aucun retour"));
                }
                let xml = format!(
                    "<retours><flux><id>FLUX-1</id><retour><publication>{p}</publication><production>{p}</production><nature>{n}</nature><statut>OK</statut><id>R1</id><url>{b}/dsn/telecharger/R1</url></retour></flux></retours>",
                    p = "20261007080000",
                    n = nature,
                    b = q.base
                );
                http(200, "application/xml", xml.as_bytes())
            } else if c.contains("telecharger/R1") {
                http(200, "application/xml", contenu)
            } else {
                http(404, "text/plain", b"inconnu")
            })
        })
        .0
    }

    fn retours(cie: &Path, base: &str) -> Sortie {
        let dsn = format!("{}/dsn", base);
        let r = lancer(
            env!("CARGO_BIN_EXE_paie-atelier"),
            Some(cie),
            &["retours", "--depuis", "06/10/2026", "--jusqu-a", "08/10/2026", "--appliquer"],
            &[("NET_ENTREPRISES_MOTDEPASSE", FAUX_MOTDEPASSE), ("PUPPETPAIE_ATELIER_DSN", &dsn)],
        );
        assert!(!r.out.contains(FAUX_MOTDEPASSE) && !r.err.contains(FAUX_MOTDEPASSE), "un mot de passe est apparu a l'ecran");
        r
    }

    #[test]
    fn une_connexion_coupee_en_cours_de_consultation_est_une_phrase() {
        let cie = ordinaire("coupure");
        let r = retours(&cie, &faux_dsn("10", b"", true));
        assert_eq!(r.code, 1, "{}{}", r.out, r.err);
        assert!(r.out.contains("Consultation interrompue : le reseau"), "{}", r.out);
        assert!(r.err.is_empty(), "{}", r.err);
        r.propre();
        // Ce n'est pas un defaut du logiciel : aucun incident.
        assert!(!cie.join("journal/incidents.json").exists());
    }

    #[test]
    fn un_crm_94_illisible_est_une_phrase_et_le_fichier_brut_est_garde() {
        let cie = ordinaire("crm-illisible");
        let r = retours(&cie, &faux_dsn("94", b"\xff\xfe pas du xml", false));
        assert_eq!(r.code, 1, "{}{}", r.out, r.err);
        assert!(r.out.contains("Ce fichier est illisible") && r.out.contains("Le fichier brut est garde"), "{}", r.out);
        assert!(r.err.is_empty(), "{}", r.err);
        r.propre();
        let archives: Vec<_> = std::fs::read_dir(cie.join("retours")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert!(archives.iter().any(|n| n.contains("nature94")), "{:?}", archives);
        // Rien n'a ete ecrit dans la configuration du PAS.
        assert!(std::fs::read_to_string(cie.join("config/pas.json")).map_or(true, |t| !t.contains("CRM DGFiP")));
    }
}
