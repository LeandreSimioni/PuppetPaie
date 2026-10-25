//! Le lot « Incidents et canal editeur » de bout en bout : `paie route` contre un
//! faux serveur local (jamais le vrai serveur de l'editeur) -- le journal des
//! incidents, l'envoi chez l'editeur, « Un probleme, une idee ? », le filet DPAE
//! (le mot de passe du logiciel est verifie par le moteur) et la soudure de la fictive.
//! Chaque test monte sa compagnie dans un dossier a lui, copie de la fictive inventee.

use crate::outils::{copier, http_json, Recu};
use paie::coffre::Coffre;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// ------------------------------------------------------------ le faux serveur

/// Ce que le serveur de l'editeur a recu : l'action (le chemin), l'agent, la cle du poste (`X-Filet-Cle`), le corps JSON.
#[derive(Clone, Debug)]
struct Vu {
    action: String,
    ua: String,
    cle: String,
    corps: Value,
}

fn vus(r: &Recu) -> Vec<Vu> {
    r.lock()
        .unwrap()
        .iter()
        .map(|q| Vu { action: q.chemin.trim_start_matches('/').to_string(), ua: q.entete("user-agent").into(), cle: q.entete("x-filet-cle").into(), corps: q.json() })
        .collect()
}

/// L'identite que le faux serveur donne a tout poste qui la demande (`POST /poste`, serveur version 7).
const POSTE_DONNE: &str = "a1b2c3d4e5f60718a1b2c3d4e5f60718";
const CLE_DONNEE: &str = "0f1e2d3c4b5a69780f1e2d3c4b5a69780f1e2d3c4b5a69780f1e2d3c4b5a6978";

/// Le faux serveur d'`outils`, qui repond `f(action, corps)` ; `poste`, lui, rend toujours la meme identite.
fn serveur(f: impl Fn(&str, &Value) -> (u16, Value) + Send + Sync + 'static) -> (String, Recu) {
    crate::outils::serveur(move |q| {
        let action = q.chemin.trim_start_matches('/');
        let (statut, rep) = if action == "poste" { (200, json!({"poste": POSTE_DONNE, "cle": CLE_DONNEE})) } else { f(action, &q.json()) };
        Some(http_json(statut, &rep))
    })
}

fn actions(r: &Recu) -> Vec<String> {
    vus(r).into_iter().map(|v| v.action).collect()
}

// ----------------------------------------------------------------- le poste

struct Poste {
    base: PathBuf,
    racine: PathBuf,
    code: PathBuf,
    cle: PathBuf,
}

/// La fictive `fictive-3090-petite` (inventee), avec ces mois de paie ; le code livre
/// est dans `logiciel/donnees` (a cote de lui, `logiciel/EDITION.json` dirait `fictive`).
fn poste(nom: &str, mois: &[&str]) -> Poste {
    let base = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("incidents-filet-{}", nom));
    let _ = std::fs::remove_dir_all(&base);
    let fictive = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/donnees/fictives/fictive-3090-petite");
    let racine = base.join("compagnie");
    copier(&fictive.join("config"), &racine.join("config"));
    let _ = std::fs::remove_file(racine.join("config/compteurs.json"));
    let _ = std::fs::remove_file(racine.join("config/filet-dpae.json"));
    std::fs::create_dir_all(racine.join("paies")).unwrap();
    std::fs::create_dir_all(racine.join("journal")).unwrap();
    std::fs::write(racine.join("journal/depots.json"), "[]").unwrap();
    for m in mois {
        std::fs::copy(fictive.join("paies").join(format!("{}.json", m)), racine.join("paies").join(format!("{}.json", m))).unwrap();
    }
    let moteur = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../logiciel/donnees");
    let code = base.join("logiciel/donnees");
    std::fs::create_dir_all(&code).unwrap();
    for e in std::fs::read_dir(&moteur).unwrap().flatten() {
        if e.path().extension().is_some_and(|x| x == "json") {
            std::fs::copy(e.path(), code.join(e.file_name())).unwrap();
        }
    }
    copier(&moteur.join("baremes"), &code.join("baremes"));
    let cle = base.join("cle-du-poste");
    std::fs::create_dir_all(&cle).unwrap();
    Poste { base, racine, code, cle }
}

impl Poste {
    fn route(&self, route: &str, charge: Value) -> Value {
        self.route_au(route, charge, "2026-09-25")
    }

    fn route_au(&self, route: &str, charge: Value, aujourdhui: &str) -> Value {
        self.route_env(route, charge, aujourdhui, &[])
    }

    /// Par un faux mandataire local, nomme comme seule porte du garde-fou reseau des essais (`garde_reseau::VARIABLE`) : il
    /// voit a qui le moteur voulait parler, et refuse ; rien ne sort.
    fn route_par(&self, mandataire: &str, route: &str, charge: Value) -> Value {
        self.route_env(route, charge, "2026-09-25", &[(socle::garde_reseau::VARIABLE, mandataire)])
    }

    fn route_env(&self, route: &str, charge: Value, aujourdhui: &str, env: &[(&str, &str)]) -> Value {
        let mut p = Command::new(env!("CARGO_BIN_EXE_paie"))
            .args(["route", "--racine", self.racine.to_str().unwrap(), "--date", aujourdhui])
            .env("PUPPETPAIE_CODE", &self.code)
            .env("PUPPETPAIE_AUJOURDHUI", aujourdhui)
            .env("PUPPETPAIE_CLE", &self.cle)
            .env_remove("PUPPETPAIE_INCIDENTS")
            .env_remove("HTTPS_PROXY")
            .env_remove("https_proxy")
            .env_remove("HTTP_PROXY")
            .env_remove("http_proxy")
            .envs(env.iter().copied())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        p.stdin.take().unwrap().write_all(json!({"route": route, "charge": charge}).to_string().as_bytes()).unwrap();
        let sortie = p.wait_with_output().unwrap().stdout;
        serde_json::from_slice(&sortie).unwrap_or_else(|_| panic!("reponse illisible : {}", String::from_utf8_lossy(&sortie)))
    }

    /// Le resultat d'une route qui doit reussir.
    fn ok(&self, route: &str, charge: Value) -> Value {
        let r = self.route(route, charge);
        assert_eq!(r["ok"], json!(true), "{} : {}", route, r);
        r["resultat"].clone()
    }

    fn fichier(&self, relatif: &str) -> Value {
        serde_json::from_slice(&std::fs::read(self.racine.join(relatif)).unwrap()).unwrap()
    }

    fn ecrire(&self, relatif: &str, v: &Value) {
        let p = self.racine.join(relatif);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, serde_json::to_string_pretty(v).unwrap()).unwrap();
    }

    /// Ou parle la compagnie : `config/filet-dpae.json`.
    fn serveur(&self, url: &str, plus: Value) {
        let mut c = json!({"serveur": url});
        for (k, v) in plus.as_object().into_iter().flatten() {
            c[k] = v.clone();
        }
        self.ecrire("config/filet-dpae.json", &c);
    }

    fn incidents(&self) -> Vec<Value> {
        self.fichier("journal/incidents.json").as_array().unwrap().clone()
    }
}

fn incident(i: usize, envoi: &str) -> Value {
    json!({
        "quoi": "logiciel", "quand": format!("2026-10-05T10:00:{:02}", i), "version": "2026.10.02",
        "empreinte": format!("e{:02}", i % 2), "ou": format!("calcul.py:{}", i), "genre": "ValueError",
        "resume": "ValueError: boom bob@exemple.fr /home/bob/logiciel/x.py", "detail": {"i": i},
        "trace": "Traceback\n  File \"/home/bob/logiciel/x.py\", line 3", "caviarde": [],
        "envoi": {"etat": envoi, "quand": null},
    })
}

// ============================================================ les incidents

#[test]
fn la_page_signale_un_defaut_et_la_route_ne_refuse_jamais() {
    let p = poste("navigateur", &[]);
    let r = p.ok(
        "incident-navigateur",
        json!({"genre": "TypeError", "message": "NIR 1 85 05 78 006 084 36 inconnu", "fichier": "http://puppetpaie.localhost:7331/interface.js?v=3", "ligne": 12, "colonne": 5, "pile": "a\nb", "canal": "erreur"}),
    );
    assert_eq!(r["note"], json!(true));
    let l = &p.incidents()[0];
    assert_eq!(l["quoi"], json!("logiciel"));
    assert_eq!(l["ou"], json!("interface.js:12"), "ni l'hote ni la requete (le jeton) ne se gardent");
    assert_eq!(l["genre"], json!("TypeError"));
    assert!(l["resume"].as_str().unwrap().contains("[NIR]") && !l["resume"].as_str().unwrap().contains("85 05"));
    assert_eq!(l["caviarde"], json!(["NIR"]));
    assert_eq!(l["envoi"]["etat"], json!("en-attente"));
    assert_eq!(r["empreinte"], l["empreinte"]);
    // Un rapport absurde ne fait pas tomber la route (la page vient de planter).
    for charge in [json!({}), json!({"ligne": "pas un nombre"}), json!({"fichier": null, "pile": 12}), json!(null), json!([1])] {
        assert_eq!(p.ok("incident-navigateur", charge)["note"], json!(true));
    }
    // Deux fois le meme defaut : une seule sorte, un compteur.
    let g = p.ok("incidents-regrouper", json!({"quoi": "logiciel"}));
    assert_eq!(g["paquets"].as_array().unwrap().iter().map(|x| x["combien"].as_i64().unwrap()).sum::<i64>(), 6);
}

#[test]
fn le_journal_se_lit_par_nature_et_s_oublie_par_nature() {
    let p = poste("oublier", &[]);
    let mut lignes: Vec<Value> = (0..3).map(|i| incident(i, "en-attente")).collect();
    let mut dsn = incident(7, "depose");
    dsn["quoi"] = json!("dsn");
    lignes.push(dsn);
    p.ecrire("journal/incidents.json", &json!(lignes));
    assert_eq!(p.ok("incidents-lire", json!({}))["lignes"].as_array().unwrap().len(), 4);
    assert_eq!(p.ok("incidents-lire", json!({"quoi": "dsn"}))["lignes"].as_array().unwrap().len(), 1);
    assert_eq!(p.ok("incidents-lire", json!({"en_attente": true}))["lignes"].as_array().unwrap().len(), 3);
    assert_eq!(p.ok("incidents-oublier", json!({"quoi": "dsn"}))["retires"], json!(1));
    assert_eq!(p.ok("incidents-oublier", json!({}))["retires"], json!(3));
    assert!(p.incidents().is_empty());
}

#[test]
fn ce_qui_attend_part_par_lots_et_passe_a_depose() {
    let p = poste("envoi", &[]);
    let lignes: Vec<Value> = (0..45).map(|i| incident(i, "en-attente")).collect();
    p.ecrire("journal/incidents.json", &json!(lignes));
    let (url, recu) = serveur(|_, corps| {
        let paires: Vec<Value> = corps["incidents"].as_array().into_iter().flatten().map(|x| json!([x["empreinte"], x["quand"]])).collect();
        (200, json!({"recus": paires}))
    });
    p.serveur(&url, json!({}));
    let r = p.ok("diagnostic-envoyer", json!({}));
    assert_eq!(r["bilan"], json!({"envoyes": 45, "reste": 0, "erreur": "", "sans_serveur": false}));
    assert_eq!(actions(&recu), ["poste", "rapport", "rapport", "rapport"], "l'identite d'abord, puis des lots de vingt");
    let vus = vus(&recu);
    assert_eq!(vus[1].ua, "PuppetPaie-filet");
    assert_eq!(vus[1].cle, CLE_DONNEE, "la cle donnee par le serveur part en en-tete");
    assert_eq!(vus[1].corps["poste"], json!(POSTE_DONNE), "et son numero dans le corps");
    assert_eq!(vus[1].corps["incidents"].as_array().unwrap().len(), 20);
    // Ce qui part est re-nettoye : ni le dossier de l'utilisateur, ni une adresse.
    let parti = vus[1].corps.to_string();
    assert!(!parti.contains("bob@exemple.fr") && !parti.contains("/home/bob"), "{}", parti);
    assert!(parti.contains("[adresse]") && parti.contains("logiciel/x.py"));
    assert!(vus[1].corps["incidents"][0].get("envoi").is_none());
    // L'ecran dit ce qui est parti, et le journal ne dit plus rien en attente.
    assert!(p.incidents().iter().all(|l| l["envoi"]["etat"] == json!("depose")));
    let e = p.ok("diagnostic-etat", json!({}));
    assert_eq!((e["total"].clone(), e["partis"].clone(), e["en_attente"].clone()), (json!(45), json!(45), json!(0)));
    assert!(e["dernier_envoi"].is_string());
    assert_eq!(e["groupes"].as_array().unwrap().len(), 2, "deux empreintes");
    // Rien deux fois.
    assert_eq!(p.ok("incidents-envoyer", json!({}))["envoyes"], json!(0));
    assert_eq!(recu.lock().unwrap().len(), 4);
}

#[test]
fn l_identite_du_poste_se_demande_une_fois_se_garde_et_sa_cle_ne_sort_jamais() {
    let p = poste("identite", &[]);
    p.ecrire("journal/incidents.json", &json!([incident(1, "en-attente")]));
    let (url, recu) = serveur(|action, corps| match action {
        "rapport" => (200, json!({"recus": corps["incidents"].as_array().unwrap().iter().map(|x| json!([x["empreinte"], x["quand"]])).collect::<Vec<_>>()})),
        _ => (200, json!({})),
    });
    // Un numero tire par une version d'avant n'est connu de personne : il se remplace.
    p.ecrire("config/diagnostic.json", &json!({"poste": "0123456789abcdef0123456789abcdef", "envoi": true}));
    p.serveur(&url, json!({}));
    assert_eq!(p.ok("incidents-envoyer", json!({}))["envoyes"], json!(1));
    assert_eq!(actions(&recu), ["poste", "rapport"]);
    let vu = vus(&recu)[0].clone();
    assert_eq!(vu.corps, json!({}), "la demande d'identite n'a pas de corps");
    assert_eq!(vu.cle, "", "ni de cle");
    let d = p.fichier("config/diagnostic.json");
    assert_eq!((d["poste"].clone(), d["cle"].clone(), d["serveur"].clone(), d["envoi"].clone()), (json!(POSTE_DONNE), json!(CLE_DONNEE), json!(url), json!(true)));
    // Gardee : le deuxieme envoi ne la redemande pas.
    p.ecrire("journal/incidents.json", &json!([incident(2, "en-attente")]));
    assert_eq!(p.ok("incidents-envoyer", json!({}))["envoyes"], json!(1));
    assert_eq!(actions(&recu), ["poste", "rapport", "rapport"]);
    // Ce que la page lit ne porte jamais la cle.
    for route in ["diagnostic-etat", "signalement-apercu", "filet-etat"] {
        let charge = if route == "signalement-apercu" { json!({"texte": "x"}) } else { json!({}) };
        let r = p.ok(route, charge);
        assert!(!r.to_string().contains(CLE_DONNEE), "{} : {}", route, r);
    }
    assert_eq!(p.ok("signalement-apercu", json!({"texte": "x"}))["corps"]["poste"], json!(POSTE_DONNE), "le numero, lui, se montre");
    // Un autre serveur : une identite chez lui.
    let (url2, recu2) = serveur(|_, _| (200, json!({"recus": []})));
    p.serveur(&url2, json!({}));
    p.ecrire("journal/incidents.json", &json!([incident(3, "en-attente")]));
    p.ok("incidents-envoyer", json!({}));
    assert_eq!(actions(&recu2), ["poste", "rapport"]);
    assert_eq!(p.fichier("config/diagnostic.json")["serveur"], json!(url2));
    // Le serveur ne reconnait plus ce poste (revoque, ou base changee) : le message dit quoi faire.
    let (url3, _) = serveur(|_, _| (401, json!({"erreur": "Poste inconnu, révoqué, ou clé fausse."})));
    let mut d = p.fichier("config/diagnostic.json");
    d["serveur"] = json!(url3);
    p.ecrire("config/diagnostic.json", &d);
    p.serveur(&url3, json!({}));
    p.ecrire("journal/incidents.json", &json!([incident(4, "en-attente")]));
    let b = p.ok("incidents-envoyer", json!({}));
    assert_eq!(b["envoyes"], json!(0));
    assert!(b["erreur"].as_str().unwrap().contains("révoqué") && b["erreur"].as_str().unwrap().contains("config/diagnostic.json"), "{}", b);
    p.ok("signalement-envoyer", json!({"texte": "x"}));
    let b = p.ok("signalements-partir", json!({}));
    assert!(b["erreur"].as_str().unwrap().contains("config/diagnostic.json"), "{}", b);
    assert!(p.ok("signalements", json!({}))["signalements"][0]["raison"].as_str().unwrap().contains("config/diagnostic.json"));
}

#[test]
fn un_serveur_qui_refuse_l_identite_laisse_tout_en_attente() {
    let p = poste("identite-refusee", &[]);
    p.ecrire("journal/incidents.json", &json!([incident(1, "en-attente")]));
    // Trop de postes neufs aujourd'hui : 429, comme une panne.
    let (url, recu) = crate::outils::serveur(|_| Some(http_json(429, &json!({"erreur": "Trop de demandes : réessayez plus tard."}))));
    p.serveur(&url, json!({}));
    let b = p.ok("incidents-envoyer", json!({}));
    assert_eq!((b["envoyes"].clone(), b["reste"].clone(), b["sans_serveur"].clone()), (json!(0), json!(1), json!(false)));
    assert!(b["erreur"].as_str().unwrap().contains("Trop de demandes"), "{}", b);
    assert_eq!(actions(&recu), ["poste"], "rien d'autre ne part");
    assert!(!p.racine.join("config/diagnostic.json").exists(), "aucune identite ecrite");
    // Le message s'enregistre, et attend : la raison se dit, rien ne se perd.
    let id = p.ok("signalement-envoyer", json!({"texte": "x"}))["id"].as_str().unwrap().to_string();
    let b = p.ok("signalements-partir", json!({}));
    assert_eq!((b["partis"].clone(), b["reste"].clone()), (json!(0), json!(1)), "{}", b);
    let m = p.ok("signalements", json!({}))["signalements"][0].clone();
    assert_eq!(m["etat"], json!("pas-parti"));
    assert!(m["raison"].as_str().unwrap().contains("Trop de demandes"), "{}", m);
    assert!(p.racine.join(format!("journal/signalements/{}.json", id)).exists(), "son corps attend");
    assert_eq!(p.ok("signalements-reponses", json!({}))["nouvelles"], json!(0));
    assert_eq!(actions(&recu), ["poste", "poste"], "rien d'autre ne part");
    // Une identite qui n'en est pas une (un serveur d'avant la version 7, qui ne connait pas `/poste`) : pareil.
    let (url2, _) = serveur(|_, _| (404, json!({"erreur": "Action inconnue."})));
    p.serveur(&url2, json!({}));
    let b = p.ok("incidents-envoyer", json!({}));
    assert_eq!(b["envoyes"], json!(0));
    assert!(b["erreur"].as_str().unwrap().contains("Action inconnue"), "{}", b);
}

/// Sans serveur pose (ni `config/filet-dpae.json`, ni `EDITEUR.json` dans le code), le moteur vise le sien,
/// `filet.puppetpaie.fr` (lot S1, `editeur::SERVEUR_PAR_DEFAUT`) : un faux mandataire local le voit et refuse, rien ne part
/// et rien ne se perd. Un serveur pose le remplace (le reste du test, et tout ce fichier).
#[test]
fn une_panne_ou_le_serveur_du_moteur_injoignable_laisse_tout_en_attente() {
    let p = poste("panne", &[]);
    p.ecrire("journal/incidents.json", &json!((0..25).map(|i| incident(i, "en-attente")).collect::<Vec<_>>()));
    let (mandataire, demandes) = crate::outils::mandataire();
    let r = p.route_par(&mandataire, "diagnostic-envoyer", json!({}));
    assert_eq!(r["ok"], json!(true), "{}", r);
    let b = r["resultat"]["bilan"].clone();
    assert_eq!((b["sans_serveur"].clone(), b["reste"].clone(), b["envoyes"].clone()), (json!(false), json!(25), json!(0)));
    assert!(!b["erreur"].as_str().unwrap().is_empty(), "{}", b);
    assert_eq!(*demandes.lock().unwrap(), ["CONNECT filet.puppetpaie.fr:443 HTTP/1.1"], "le serveur du moteur, et lui seul");
    assert_eq!(p.ok("diagnostic-etat", json!({}))["serveur"], json!("https://filet.puppetpaie.fr"));
    // Une panne au deuxieme lot : le premier est parti, le reste attend.
    let (url, recu) = serveur(|_, corps| {
        let n = corps["incidents"].as_array().map_or(0, Vec::len);
        if n == 20 {
            (200, json!({"recus": corps["incidents"].as_array().unwrap().iter().map(|x| json!([x["empreinte"], x["quand"]])).collect::<Vec<_>>()}))
        } else {
            (500, json!({"erreur": "boom"}))
        }
    });
    p.serveur(&url, json!({}));
    let b = p.ok("incidents-envoyer", json!({}));
    assert_eq!((b["envoyes"].clone(), b["reste"].clone()), (json!(20), json!(5)));
    assert!(b["erreur"].as_str().unwrap().contains("boom"), "{}", b);
    assert_eq!(actions(&recu), ["poste", "rapport", "rapport"]);
    assert_eq!(p.ok("incidents-lire", json!({"en_attente": true}))["lignes"].as_array().unwrap().len(), 5);
}

/// UNE FICTIVE AJOUTEE (ticket #236) n'envoie rien a un organisme ni a une personne, mais ce qui va a l'editeur part :
/// ses incidents, et « Un probleme, une idee ? » (le corps dit qu'il vient d'une fictive : `demonstration`). L'inscription
/// au filet, dont le serveur ecrirait a l'adresse, est refusee.
#[test]
fn une_fictive_ajoutee_envoie_a_l_editeur_et_pas_au_filet() {
    let p = poste("fictive-ajoutee", &[]);
    let inst = Path::new(env!("CARGO_TARGET_TMPDIR")).join("incidents-filet-fictive-ajoutee-installation");
    let _ = std::fs::remove_dir_all(&inst);
    let c = paie::interface::compagnies::ajouter_dans(&inst, "Marque").unwrap();
    std::fs::copy(c.racine.join("COPIE-DE-DEVELOPPEMENT.json"), p.racine.join("COPIE-DE-DEVELOPPEMENT.json")).unwrap();
    assert!(paie::interface::compagnies::est_ajoutee(&p.racine));
    p.ecrire("journal/incidents.json", &json!([incident(1, "en-attente")]));
    let (url, recu) = serveur(|action, corps| match action {
        "rapport" => (200, json!({"recus": corps["incidents"].as_array().unwrap().iter().map(|x| json!([x["empreinte"], x["quand"]])).collect::<Vec<_>>()})),
        "retour" => (200, json!({"numero": "R-000001"})),
        _ => (200, json!({})),
    });
    p.serveur(&url, json!({}));
    assert_eq!(p.ok("incidents-envoyer", json!({}))["envoyes"], json!(1));
    p.ok("signalement-envoyer", json!({"texte": "x"}));
    assert_eq!(p.ok("signalements-partir", json!({}))["partis"], json!(1));
    assert_eq!(actions(&recu), ["poste", "rapport", "retour"]);
    assert_eq!(vus(&recu)[2].corps["contexte"]["demonstration"], json!(true));
    let r = p.route("filet-activer", json!({"adresse": "a@exemple.fr"}));
    assert_eq!(r["classe"], json!(paie::bridage::CLASSE_REFUS_D_ENVOI), "{}", r);
    assert_eq!(actions(&recu).len(), 3, "rien n'est parti au filet");
}

// ============================================== « Un probleme, une idee ? »

#[test]
fn le_signalement_s_apercoit_s_envoie_et_sa_reponse_se_releve_puis_se_lit() {
    let p = poste("signalement", &[]);
    p.ecrire("journal/incidents.json", &json!((0..12).map(|i| incident(i, "en-attente")).collect::<Vec<_>>()));
    let (url, recu) = serveur(|action, corps| match action {
        "retour" => (200, json!({"numero": "R-000001", "ticket": "T-9", "ticket_numero": 42})),
        "reponses" => {
            let n = corps["numeros"].as_array().map_or(0, Vec::len);
            (200, json!({"retours": [{"numero": "R-000001", "reponse": format!("Merci ({})", n), "repondu_le": "2026-10-06"}]}))
        }
        _ => (200, json!({})),
    });
    p.serveur(&url, json!({}));
    let png = "iVBORw0KGgo=";
    let charge = json!({"texte": "  Mon NIR 1 85 05 78 006 084 36 se trompe  ", "ecran": "Paie", "captures": [{"type": "image/png", "donnees": format!("data:image/png;base64,{}", png)}]});
    let a = p.ok("signalement-apercu", charge.clone());
    assert_eq!((a["caviarde"].clone(), a["captures"].clone(), a["incidents"].clone(), a["peut_envoyer"].clone()), (json!(true), json!(1), json!(10), json!(true)));
    assert_eq!(a["corps"]["texte"], json!("Mon NIR [NIR] se trompe"));
    assert_eq!(a["corps"]["captures"][0]["donnees"], json!("(1 Ko, non affichés)"));
    assert!(recu.lock().unwrap().is_empty(), "l'apercu n'envoie rien");
    let r = p.ok("signalement-envoyer", charge);
    assert!(recu.lock().unwrap().is_empty(), "envoyer enregistre, rien ne part");
    assert_eq!(r["signalements"][0]["etat"], json!("en-cours"));
    let b = p.ok("signalements-partir", json!({}));
    assert_eq!((b["partis"].clone(), b["reste"].clone(), b["erreur"].clone()), (json!(1), json!(0), json!("")));
    assert_eq!(actions(&recu), ["poste", "retour"]);
    let ligne = p.fichier("journal/signalements.json")[0].clone();
    assert_eq!((ligne["numero"].clone(), ligne["ticket_numero"].clone(), ligne["envoi"]["etat"].clone()), (json!("R-000001"), json!(42), json!("depose")));
    let vu = vus(&recu)[1].clone();
    assert_eq!(vu.corps["captures"][0]["donnees"], json!(png), "la capture part, sans son prefixe");
    assert_eq!(vu.corps["incidents"].as_array().unwrap().len(), 10);
    assert_eq!((vu.cle.as_str(), vu.corps["poste"].clone()), (CLE_DONNEE, json!(POSTE_DONNE)));
    // La boite garde ce que ce poste a signale.
    let boite = p.ok("signalements", json!({}));
    assert_eq!((boite["signalements"][0]["numero"].clone(), boite["signalements"][0]["etat"].clone()), (json!("R-000001"), json!("arrive")));
    assert!(std::fs::read_dir(p.racine.join("journal/signalements")).unwrap().next().is_none(), "parties, les captures ne restent pas sur le poste");
    // La reponse se releve (une seule fois), arrive non lue, puis se lit.
    assert_eq!(p.ok("signalements-reponses", json!({}))["nouvelles"], json!(1));
    assert_eq!(p.ok("signalements-reponses", json!({}))["nouvelles"], json!(0));
    assert_eq!(p.fichier("journal/signalements.json")[0]["lu"], json!(false));
    assert_eq!(p.ok("signalements-lus", json!({}))["lues"], json!(1));
    assert_eq!(p.fichier("journal/signalements.json")[0]["lu"], json!(true));
    assert_eq!(p.ok("signalements-lus", json!({}))["lues"], json!(0));
}

#[test]
fn un_signalement_invalide_est_refuse_et_sans_serveur_pose_il_vise_celui_du_moteur() {
    let p = poste("signalement-refus", &[]);
    let (url, recu) = serveur(|_, _| (200, json!({"numero": "N"})));
    p.serveur(&url, json!({}));
    for charge in [
        json!({"texte": "   "}),
        json!({"texte": "x".repeat(5001)}),
        json!({"texte": "x", "captures": [{"type": "image/gif", "donnees": "QUJD"}]}),
        json!({"texte": "x", "captures": [{"type": "image/png", "donnees": "pas du base64!"}]}),
        json!({"texte": "x", "captures": (0..6).map(|_| json!({"type": "image/png", "donnees": "QUJD"})).collect::<Vec<_>>()}),
    ] {
        let r = p.route("signalement-envoyer", charge.clone());
        assert_eq!((r["ok"].clone(), r["classe"].clone()), (json!(false), json!("ValueError")), "{}", charge.to_string().chars().take(80).collect::<String>());
    }
    assert!(recu.lock().unwrap().is_empty());
    assert!(!p.racine.join("journal/signalements.json").exists());
    // Sans serveur pose : l'apercu nomme celui du moteur (lot S1), et l'envoi va vers lui -- ici un faux mandataire local
    // le voit et refuse : le signalement n'est pas parti, et il le dit.
    let _ = std::fs::remove_file(p.racine.join("config/filet-dpae.json"));
    let a = p.ok("signalement-apercu", json!({"texte": "x"}));
    assert_eq!((a["peut_envoyer"].clone(), a["serveur"].clone()), (json!(true), json!("https://filet.puppetpaie.fr")));
    let (mandataire, demandes) = crate::outils::mandataire();
    let r = p.route_par(&mandataire, "signalement-envoyer", json!({"texte": "x"}));
    assert_eq!(r["ok"], json!(true), "{}", r);
    assert!(demandes.lock().unwrap().is_empty(), "envoyer ne parle a personne");
    let r = p.route_par(&mandataire, "signalements-partir", json!({}));
    assert_eq!((r["resultat"]["partis"].clone(), r["resultat"]["reste"].clone()), (json!(0), json!(1)), "{}", r);
    assert!(!r["resultat"]["erreur"].as_str().unwrap().is_empty(), "{}", r);
    assert!(demandes.lock().unwrap().iter().all(|l| l.starts_with("CONNECT filet.puppetpaie.fr:443")), "{:?}", demandes.lock().unwrap());
    assert!(!demandes.lock().unwrap().is_empty());
}

#[test]
fn ce_qui_part_dit_la_machine_et_rien_de_la_personne() {
    let p = poste("machine", &[]);
    let a = p.ok("signalement-apercu", json!({"texte": "x"}));
    let machine = a["corps"]["contexte"]["systeme"].as_str().unwrap().to_string();
    assert!(machine.contains("moteur Rust"), "{}", machine);
    assert!(!machine.contains(',') && machine.chars().count() <= 120, "{}", machine);
    for nom in [std::env::var("USER").unwrap_or_default(), std::env::var("HOME").unwrap_or_default()] {
        assert!(nom.len() < 2 || !machine.contains(&nom), "{} dans {}", nom, machine);
    }
    assert_eq!(a["corps"]["contexte"]["demonstration"], json!(false));
    assert_eq!(a["corps"]["contexte"]["compagnie"], json!(""));
}

/// Un message qui n'a pas pu partir repart au passage suivant, une seule fois : deux messages, une panne, puis le serveur
/// revient -- chacun arrive une fois, dans l'ordre, et un passage de plus n'envoie rien.
#[test]
fn un_message_qui_n_est_pas_parti_repart_une_seule_fois() {
    let p = poste("signalement-reprise", &[]);
    let panne = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let en_panne = panne.clone();
    let (url, recu) = serveur(move |action, corps| match action {
        "retour" if en_panne.load(std::sync::atomic::Ordering::SeqCst) => (500, json!({"erreur": "Action inconnue."})),
        "retour" => (200, json!({"numero": format!("R-{}", corps["texte"].as_str().unwrap())})),
        _ => (200, json!({})),
    });
    p.serveur(&url, json!({}));
    p.ok("signalement-envoyer", json!({"texte": "un"}));
    p.ok("signalement-envoyer", json!({"texte": "deux"}));
    let b = p.ok("signalements-partir", json!({}));
    assert_eq!((b["partis"].clone(), b["reste"].clone()), (json!(0), json!(2)), "{}", b);
    assert_eq!(actions(&recu), ["poste", "retour"], "la panne arrete le passage");
    for m in p.ok("signalements", json!({}))["signalements"].as_array().unwrap() {
        assert_eq!(m["etat"], json!("pas-parti"));
        assert!(m["raison"].as_str().unwrap().contains("Action inconnue"), "la raison vaut pour tous : {}", m);
    }
    panne.store(false, std::sync::atomic::Ordering::SeqCst);
    let b = p.ok("signalements-partir", json!({}));
    assert_eq!((b["partis"].clone(), b["reste"].clone(), b["erreur"].clone()), (json!(2), json!(0), json!("")));
    let partis: Vec<String> = vus(&recu).iter().filter(|v| v.action == "retour").skip(1).map(|v| v.corps["texte"].as_str().unwrap().to_string()).collect();
    assert_eq!(partis, ["un", "deux"], "du plus ancien au plus recent");
    let boite = p.fichier("journal/signalements.json");
    let numeros: Vec<Value> = boite.as_array().unwrap().iter().map(|l| l["numero"].clone()).collect();
    assert_eq!(numeros, [json!("R-un"), json!("R-deux")]);
    assert!(boite.as_array().unwrap().iter().all(|l| l["envoi"]["etat"] == json!("depose") && l["envoi"]["erreur"] == json!("")));
    let n = recu.lock().unwrap().len();
    assert_eq!(p.ok("signalements-partir", json!({}))["partis"], json!(0));
    assert_eq!(recu.lock().unwrap().len(), n, "un passage de plus n'envoie rien");
}

/// Un seul passage a la fois : un autre tient le verrou d'envoi, celui-ci rend `occupe` et n'envoie rien ; le message
/// attend, enregistre. Un fichier d'attente que la boite ne connait plus (parti, ou jamais inscrit) s'efface.
#[test]
fn un_seul_envoi_a_la_fois_et_rien_ne_reste_de_ce_qui_est_parti() {
    let p = poste("signalement-verrou", &[]);
    let (url, recu) = serveur(|_, _| (200, json!({"numero": "R-1"})));
    p.serveur(&url, json!({}));
    let id = p.ok("signalement-envoyer", json!({"texte": "x"}))["id"].as_str().unwrap().to_string();
    let verrou = p.racine.join("journal/.signalements-envoi.verrou");
    std::fs::write(&verrou, "signalements-envoi pid 1 2026-10-14T10:00:00").unwrap();
    let b = p.ok("signalements-partir", json!({}));
    assert_eq!((b["occupe"].clone(), b["partis"].clone(), b["reste"].clone()), (json!(true), json!(0), json!(1)));
    assert!(recu.lock().unwrap().is_empty());
    std::fs::remove_file(&verrou).unwrap();
    let orphelin = p.racine.join("journal/signalements/20261014T090000-000000000000.json");
    std::fs::write(&orphelin, "{}").unwrap();
    assert_eq!(p.ok("signalements-partir", json!({}))["partis"], json!(1));
    assert_eq!(actions(&recu), ["poste", "retour"]);
    assert!(!p.racine.join(format!("journal/signalements/{}.json", id)).exists());
    assert!(!orphelin.exists(), "un fichier que la boite ne connait pas s'efface");
    assert!(!verrou.exists(), "le passage rend son verrou");
}

/// Ticket #229 : le meme message envoye deux fois (un double clic, une requete rejouee par la fenetre) ne s'enregistre
/// qu'une fois et ne part qu'une fois, meme revenu juste apres son depart ; un autre texte est un autre message, et
/// chaque message qui attend a son corps sur le poste (le corps et sa ligne s'ecrivent sous le meme verrou).
#[test]
fn un_meme_message_envoye_deux_fois_ne_s_enregistre_et_ne_part_qu_une_fois() {
    let p = poste("signalement-double", &[]);
    let (url, recu) = serveur(|_, _| (200, json!({"numero": "R-1"})));
    p.serveur(&url, json!({}));
    let a = p.ok("signalement-envoyer", json!({"texte": "la DSN ne part pas"}));
    let b = p.ok("signalement-envoyer", json!({"texte": "la DSN ne part pas"}));
    assert_eq!((b["id"].clone(), b["deja"].clone()), (a["id"].clone(), json!(true)), "{}", b);
    assert_eq!(p.fichier("journal/signalements.json").as_array().unwrap().len(), 1);
    assert_eq!(p.ok("signalements-partir", json!({}))["partis"], json!(1));
    let c = p.ok("signalement-envoyer", json!({"texte": "la DSN ne part pas"}));
    assert_eq!(c["deja"], json!(true), "parti il y a un instant, c'est le meme message : {}", c);
    assert_eq!(p.ok("signalements-partir", json!({}))["partis"], json!(0));
    assert_eq!(actions(&recu).iter().filter(|x| *x == "retour").count(), 1, "il n'est parti qu'une fois");
    let d = p.ok("signalement-envoyer", json!({"texte": "autre chose"}));
    assert_eq!(d["deja"], json!(false));
    assert!(p.racine.join(format!("journal/signalements/{}.json", d["id"].as_str().unwrap())).is_file());
    assert_eq!(p.fichier("journal/signalements.json").as_array().unwrap().len(), 2);
}

// ======================================================= la soudure de la fictive

#[test]
fn le_paquet_soude_ne_parle_qu_a_l_editeur() {
    let p = poste("fictive", &[]);
    std::fs::write(p.base.join("logiciel/EDITION.json"), r#"{"edition": "bride"}"#).unwrap();
    let (url, recu) = serveur(|_, corps| {
        let paires: Vec<Value> = corps["incidents"].as_array().into_iter().flatten().map(|x| json!([x["empreinte"], x["quand"]])).collect();
        (200, json!({"recus": paires, "numero": "R-1", "cle": "k", "confirme": true}))
    });
    p.serveur(&url, json!({}));
    p.ecrire("journal/incidents.json", &json!([incident(1, "en-attente")]));
    // Les trois actions de l'editeur passent : rapport, retour, reponses.
    assert_eq!(p.ok("incidents-envoyer", json!({}))["envoyes"], json!(1));
    p.ok("signalement-envoyer", json!({"texte": "bonjour"}));
    assert_eq!(p.ok("signalements-partir", json!({}))["partis"], json!(1));
    p.ok("signalements-reponses", json!({}));
    assert_eq!(actions(&recu), ["poste", "rapport", "retour", "reponses"]);
    // Celles du filet DPAE ne partent pas : la porte est soudee.
    let r = p.route("filet-activer", json!({"adresse": "a@b.fr"}));
    assert_eq!((r["ok"].clone(), r["classe"].clone()), (json!(false), json!("EditionBridee")), "{}", r);
    assert_eq!(recu.lock().unwrap().len(), 4, "rien de plus n'est parti");
}

// ============================================================== le filet DPAE

fn compte(url: &str, confirme: bool) -> Value {
    json!({"serveur": url, "adresse": "a@exemple.fr", "cle": "c-1", "confirme": confirme, "inscrit_le": "2026-10-01T09:00"})
}

/// Le garde-fou reseau des essais (21/10/2026, `src/garde_reseau.rs`, pose par `.cargo/config.toml`) : sans serveur pose
/// et sans porte nommee, `filet-activer` n'atteint pas `filet.puppetpaie.fr` -- c'est ainsi qu'un essai s'y etait inscrit,
/// et que des courriels de confirmation partaient a `a@exemple.fr` (15, 16, 20 et 21/10/2026). Un mandataire de
/// l'environnement qui laisserait passer (ici un faux, qui note) n'est pas pris : le refus vient du garde-fou, avant toute
/// connexion, et rien n'est ecrit. Les incidents non plus ne partent pas.
#[test]
fn sans_serveur_pose_rien_ne_sort_vers_le_serveur_de_l_editeur() {
    let p = poste("garde-reseau", &[]);
    let (m, demandes) = crate::outils::mandataire();
    let env: Vec<(&str, &str)> = ["HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy", "ALL_PROXY", "all_proxy"].iter().map(|v| (*v, m.as_str())).collect();
    let r = p.route_env("filet-activer", json!({"adresse": "a@exemple.fr"}), "2026-09-25", &env);
    assert_eq!((r["ok"].clone(), r["classe"].clone()), (json!(false), json!("ErreurOperation")), "{}", r);
    assert!(r["erreur"].as_str().unwrap().contains(socle::garde_reseau::VARIABLE), "le refus vient du garde-fou : {}", r);
    let pose = p.racine.join("config/filet-dpae.json");
    assert!(!pose.exists() || p.fichier("config/filet-dpae.json")["compte"].is_null(), "aucun compte sans inscription");
    p.ecrire("journal/incidents.json", &json!([incident(0, "en-attente")]));
    let r = p.route_env("diagnostic-envoyer", json!({}), "2026-09-25", &env);
    let b = r["resultat"]["bilan"].clone();
    assert_eq!((b["reste"].clone(), b["envoyes"].clone()), (json!(1), json!(0)), "{}", r);
    assert!(b["erreur"].as_str().unwrap().contains(socle::garde_reseau::VARIABLE), "{}", b);
    assert!(demandes.lock().unwrap().is_empty(), "rien n'est parti, pas meme vers un mandataire : {:?}", demandes.lock().unwrap());
}

#[test]
fn le_filet_s_active_se_verifie_et_se_desactive_sans_rendre_la_cle() {
    let p = poste("option", &[]);
    let (url, recu) = serveur(|action, _| match action {
        "inscrire" => (200, json!({"cle": "cle-secrete", "confirme": false})),
        "etat" => (200, json!({"confirme": true})),
        _ => (200, json!({"efface": true})),
    });
    // Sans serveur pose, l'inscription va au serveur du moteur (lot S1, `editeur::SERVEUR_PAR_DEFAUT`) -- ici un faux
    // mandataire local le voit et refuse : rien ne sort, l'activation est refusee et aucun compte n'est ecrit.
    let (mandataire, demandes) = crate::outils::mandataire();
    let r = p.route_par(&mandataire, "filet-activer", json!({"adresse": "a@exemple.fr"}));
    assert_eq!((r["ok"].clone(), r["classe"].clone()), (json!(false), json!("ErreurOperation")), "{}", r);
    assert_eq!(*demandes.lock().unwrap(), ["CONNECT filet.puppetpaie.fr:443 HTTP/1.1"], "le serveur du moteur, et lui seul");
    let pose = p.racine.join("config/filet-dpae.json");
    assert!(!pose.exists() || p.fichier("config/filet-dpae.json")["compte"].is_null(), "aucun compte sans inscription");
    p.serveur(&url, json!({}));
    let r = p.route("filet-activer", json!({"adresse": "sans-arobase"}));
    assert_eq!(r["ok"], json!(false));
    let e = p.ok("filet-activer", json!({"adresse": " Bob@Exemple.FR "}));
    assert_eq!((e["actif"].clone(), e["adresse"].clone(), e["confirme"].clone()), (json!(true), json!("bob@exemple.fr"), json!(false)));
    assert!(!e.to_string().contains("cle-secrete"), "la cle du compte ne revient jamais a la page");
    let inscrire = vus(&recu)[1].clone();
    assert_eq!((inscrire.cle.as_str(), inscrire.corps["poste"].clone()), (CLE_DONNEE, json!(POSTE_DONNE)), "`inscrire` est une action du poste : son identite part avec");
    assert_eq!(p.ok("filet-verifier", json!({}))["confirme"], json!(true));
    let e = p.ok("filet-desactiver", json!({}));
    assert_eq!(e["actif"], json!(false));
    assert_eq!(actions(&recu), ["poste", "inscrire", "etat", "desinscrire"]);
    assert_eq!(vus(&recu)[2].cle, "", "les actions du compte ne portent pas la cle du poste");
    // Un serveur qui refuse (hors 404) laisse le compte en place.
    p.ok("filet-activer", json!({"adresse": "a@exemple.fr"}));
    let (url2, _) = serveur(|_, _| (500, json!({"erreur": "boom"})));
    let mut etat = p.fichier("config/filet-dpae.json");
    etat["compte"]["serveur"] = json!(url2);
    p.ecrire("config/filet-dpae.json", &etat);
    let r = p.route("filet-desactiver", json!({}));
    assert_eq!((r["ok"].clone(), r["classe"].clone()), (json!(false), json!("ErreurOperation")));
    assert!(p.fichier("config/filet-dpae.json")["compte"].is_object());
}

#[test]
fn le_mot_de_passe_du_logiciel_est_verifie_par_le_moteur() {
    let p = poste("mot-de-passe", &[]);
    let (url, recu) = serveur(|_, _| (200, json!({"cle": "k", "confirme": true})));
    p.serveur(&url, json!({"compte": compte(&url, true)}));
    let coffre = Coffre { racine: p.racine.clone(), dossier_cle: p.cle.clone(), maintenant: "2026-10-05T10:00".into(), variables: BTreeMap::new() };
    coffre.poser_ouverture("sesame-123", None).unwrap();
    for (route, charge) in [("filet-activer", json!({"adresse": "a@exemple.fr"})), ("filet-desactiver", json!({}))] {
        for mdp in [json!(null), json!(""), json!("faux-faux"), json!("sesame-123 ")] {
            let mut c = charge.clone();
            c["motdepasse"] = mdp.clone();
            let r = p.route(route, c);
            assert_eq!((r["ok"].clone(), r["classe"].clone()), (json!(false), json!("PreuveInsuffisante")), "{} avec {}", route, mdp);
        }
    }
    assert!(recu.lock().unwrap().is_empty(), "refuse avant tout envoi");
    assert!(p.fichier("config/filet-dpae.json")["compte"].is_object(), "rien n'a ete modifie");
    // Le bon mot de passe passe.
    let e = p.ok("filet-desactiver", json!({"motdepasse": "sesame-123"}));
    assert_eq!(e["actif"], json!(false));
    assert_eq!(actions(&recu), vec!["desinscrire"]);
}

#[test]
fn le_passage_arme_la_veille_se_tait_si_rien_ne_change_et_desarme_quand_la_dpae_part() {
    let p = poste("passage", &["2026-10"]);
    let (url, recu) = serveur(|_, _| (200, json!({"arme": true, "desarme": true, "confirme": true})));
    p.serveur(&url, json!({"compte": compte(&url, true)}));
    let suivies = p.ok("filet-suivies", json!({}))["suivies"].as_array().unwrap().clone();
    let brioche = suivies.iter().find(|e| e["salarie"] == json!("BRIOCHE") && e["debut"] == json!("2026-10-13")).expect("BRIOCHE est suivie").clone();
    assert_eq!(brioche["contrat_parti"], json!(false), "saisie seulement : pas encore decidee");
    // Le contrat part a la signature : le reveil s'arme pour la veille a 9 h.
    p.ecrire(
        "journal/depots.json",
        &json!([{"quoi": "contrat-signature", "quand": "2026-09-20T10:00:00", "mois": "2026-10", "accepte": true, "detail": {"contrats": [{"salarie": "BRIOCHE", "debut": "2026-10-13"}]}}]),
    );
    let r = p.ok("filet-passage", json!({"maintenant": "2026-09-25T10:00", "tests": {"B|2026-10-13": {"empreinte": "x"}}}));
    assert_eq!(r["armes"], json!(["BRIOCHE|2026-10-13"]), "{}", r);
    assert!(r["incidents"].as_array().unwrap().is_empty());
    let vu = vus(&recu)[0].clone();
    assert_eq!((vu.action.as_str(), vu.corps["echeance"].clone(), vu.corps["embauche"].clone()), ("armer", json!("2026-10-12T09:00"), json!("2026-10-13")));
    assert!(vu.corps.get("objet").is_none(), "l'objet ne part que pour la DSN");
    let etat = p.fichier("config/filet-dpae.json");
    assert_eq!(etat["reveils"]["BRIOCHE|2026-10-13"]["echeance"], json!("2026-10-12T09:00"));
    assert!(etat["tests"]["B|2026-10-13"].is_object(), "les tests donnes par la charge s'ajoutent");
    // Rien de neuf : rien ne part.
    let avant = recu.lock().unwrap().len();
    let r = p.ok("filet-passage", json!({"maintenant": "2026-09-25T10:00"}));
    assert_eq!(r["armes"], json!([]));
    assert_eq!(recu.lock().unwrap().len(), avant);
    // Les tests des embauches qui ne sont plus suivies s'effacent (`vivantes`).
    p.ok("filet-passage", json!({"maintenant": "2026-09-25T10:00", "vivantes": ["BRIOCHE|2026-10-13"]}));
    assert!(p.fichier("config/filet-dpae.json")["tests"].as_object().unwrap().is_empty());
    // La DPAE part (depot reel) : le reveil se desarme.
    let code = p.fichier("config/filet-dpae.json")["reveils"]["BRIOCHE|2026-10-13"]["code"].as_str().unwrap().to_string();
    let r = p.ok("filet-desarmer", json!({"salarie": "BRIOCHE", "debut": "2026-10-13"}));
    assert_eq!(r["desarme"], json!(true));
    let dernier = vus(&recu).last().unwrap().clone();
    assert_eq!((dernier.action.as_str(), dernier.corps["code"].clone()), ("desarmer", json!(code)));
    assert!(p.fichier("config/filet-dpae.json")["reveils"].as_object().unwrap().is_empty());
    assert_eq!(p.ok("filet-desarmer", json!({"salarie": "BRIOCHE", "debut": "2026-10-13"}))["desarme"], json!(false));
}

#[test]
fn une_panne_du_serveur_ne_leve_pas_et_le_compte_efface_se_dit_sans_boucler() {
    let p = poste("passage-panne", &["2026-10"]);
    p.ecrire(
        "journal/depots.json",
        &json!([{"quoi": "contrat-signature", "quand": "2026-09-20T10:00:00", "mois": "2026-10", "accepte": true, "detail": {"contrats": [{"salarie": "BRIOCHE", "debut": "2026-10-13"}]}}]),
    );
    let (url, _) = serveur(|_, _| (500, json!({"erreur": "boom"})));
    p.serveur(&url, json!({"compte": compte(&url, true)}));
    let r = p.ok("filet-passage", json!({"maintenant": "2026-09-25T10:00"}));
    assert_eq!(r["incidents"].as_array().unwrap().len(), 1);
    assert!(p.fichier("config/filet-dpae.json")["reveils"].as_object().is_none_or(|r| r.is_empty()), "rien de note : le passage suivant reprend");
    // Le compte n'existe plus chez le serveur : on le dit, les reveils notes sautent.
    let (url404, _) = serveur(|_, _| (404, json!({"erreur": "compte inconnu"})));
    let mut etat = p.fichier("config/filet-dpae.json");
    etat["compte"] = compte(&url404, true);
    etat["reveils"] = json!({"x|2026-10-13": {"code": "z", "echeance": "2026-10-12T09:00"}});
    p.ecrire("config/filet-dpae.json", &etat);
    let r = p.ok("filet-passage", json!({"maintenant": "2026-09-25T10:00"}));
    assert_eq!(r["incidents"].as_array().unwrap().len(), 1);
    let etat = p.fichier("config/filet-dpae.json");
    assert_eq!((etat["compte"]["confirme"].clone(), etat["reveils"].clone()), (json!(false), json!({})));
    assert!(etat["compte"]["erreur"].as_str().unwrap().contains("compte inconnu"));
}

// ================================================= le nombre de paies de l'annee (ticket #246)

/// Une compagnie de test pour le nombre de paies : les mois de 2025, janvier et fevrier 2026 de la fictive inventee, ses
/// depots (DSN reelles comprises) SAUF la DSN de 2025-12, et une seule paie de 2025-12 validee a la main. Le compte de 2025 :
/// 20 paies dans des mois dont la DSN reelle est partie, moins les 3 de 2025-12, plus la validee a la main = 18 ; les 3 de
/// fevrier 2026 (sa DSN reelle partie) ne comptent pas pour 2025. Le logiciel est debloque (le geste de l'accueil), sauf
/// ce qu'un test change.
fn poste_des_paies(nom: &str) -> Poste {
    let mois: Vec<String> = (1..=12).map(|m| format!("2025-{:02}", m)).chain(["2026-01".to_string(), "2026-02".to_string()]).collect();
    let p = poste(nom, &mois.iter().map(String::as_str).collect::<Vec<_>>());
    let fictive = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/donnees/fictives/fictive-3090-petite");
    let depots: Value = serde_json::from_slice(&std::fs::read(fictive.join("journal/depots.json")).unwrap()).unwrap();
    let sans_decembre: Vec<Value> = depots.as_array().unwrap().iter().filter(|l| !(l["quoi"] == json!("dsn") && l["mois"] == json!("2025-12"))).cloned().collect();
    p.ecrire("journal/depots.json", &json!(sans_decembre));
    let mut decembre = p.fichier("paies/2025-12.json");
    decembre["engagements"][0]["valide"] = json!({"le": "2026-01-05T10:00:00"});
    p.ecrire("paies/2025-12.json", &decembre);
    p.edition("debloque");
    p
}

impl Poste {
    /// Le geste de l'accueil, retenu sur le poste (`bridage`) : `bride` ou `debloque`.
    fn edition(&self, etat: &str) {
        std::fs::write(self.base.join(".edition.json"), json!({"edition": etat}).to_string()).unwrap();
    }

    /// Un passage d'ouverture (ou de veille) ce jour-la, sans releve ni sauvegarde : ce qui part chez l'editeur.
    fn ouvrir_le(&self, aujourdhui: &str) {
        let r = self.route_au("ouverture", json!({"retours": false, "signatures": false, "sauvegarder": false}), aujourdhui);
        assert_eq!(r["ok"], json!(true), "{}", r);
    }
}

/// Le faux serveur de l'editeur qui recoit le nombre de paies (`paies`), et repond `{}` au reste.
fn serveur_des_paies() -> (String, Recu) {
    serveur(|action, corps| match action {
        "paies" => (200, json!({"recu": true, "annee": corps["annee"], "paies": corps["paies"]})),
        _ => (200, json!({})),
    })
}

fn paies_parties(recu: &Recu) -> Vec<Vu> {
    vus(recu).into_iter().filter(|v| v.action == "paies").collect()
}

#[test]
fn le_nombre_de_paies_de_l_annee_part_le_16_janvier_une_seule_fois() {
    let p = poste_des_paies("paies-annee");
    let (url, recu) = serveur_des_paies();
    p.serveur(&url, json!({}));
    // Avant le 16 janvier : rien.
    p.ouvrir_le("2026-01-15");
    assert!(paies_parties(&recu).is_empty(), "{:?}", actions(&recu));
    // Le 16 : l'annee passee, comptee, avec l'identite du poste, et rien d'autre.
    p.ouvrir_le("2026-01-16");
    let partis = paies_parties(&recu);
    assert_eq!(partis.len(), 1, "{:?}", actions(&recu));
    assert_eq!(partis[0].corps, json!({"annee": 2025, "paies": 18, "poste": POSTE_DONNE}), "ni nom, ni SIREN, ni NIR, ni montant");
    assert_eq!(partis[0].cle, CLE_DONNEE, "la cle du poste en en-tete, comme les incidents");
    // Retenu : il ne repart ni au passage suivant, ni le mois d'apres.
    let d = p.fichier("config/diagnostic.json");
    assert_eq!(d["paies_annuelles"]["2025"]["paies"], json!(18));
    p.ouvrir_le("2026-01-16");
    p.ouvrir_le("2026-03-02");
    assert_eq!(paies_parties(&recu).len(), 1);
    // L'ecran de ce qui est parti le montre, tel qu'il est parti, sans le compter parmi les incidents.
    let e = p.ok("diagnostic-etat", json!({}));
    assert_eq!(e["total"], json!(0));
    let g = e["groupes"].as_array().unwrap().iter().find(|g| g["quoi"] == json!("paies")).cloned().expect("l'annee partie est dans la liste");
    assert_eq!(g["contenu"], json!({"annee": 2025, "paies": 18, "poste": POSTE_DONNE}));
    assert!(g["parti_le"].is_string());
    // L'annee suivante, le 16 janvier : 2026 (les 3 paies de fevrier 2026, validees par sa DSN reelle).
    p.ouvrir_le("2027-01-16");
    let partis = paies_parties(&recu);
    assert_eq!(partis.len(), 2);
    assert_eq!(partis[1].corps["annee"], json!(2026));
    assert_eq!(partis[1].corps["paies"], json!(3));
}

#[test]
fn zero_paie_part_aussi() {
    let p = poste("paies-zero", &[]);
    p.edition("debloque");
    let (url, recu) = serveur_des_paies();
    p.serveur(&url, json!({}));
    p.ouvrir_le("2026-02-01");
    let partis = paies_parties(&recu);
    assert_eq!(partis.len(), 1);
    assert_eq!(partis[0].corps["paies"], json!(0));
}

#[test]
fn rien_ne_part_si_l_envoi_est_coupe_d_une_fictive_d_une_copie_de_developpement_ni_d_un_logiciel_bride() {
    let (url, recu) = serveur_des_paies();
    // L'envoi coupe (`config/diagnostic.json`, `envoi` : faux) : rien ; rallume, il part.
    let p = poste_des_paies("paies-coupe");
    p.serveur(&url, json!({}));
    p.ok("diagnostic-regler", json!({"actif": false}));
    p.ouvrir_le("2026-01-20");
    assert!(paies_parties(&recu).is_empty());
    p.ok("diagnostic-regler", json!({"actif": true}));
    p.ouvrir_le("2026-01-20");
    assert_eq!(paies_parties(&recu).len(), 1);
    // Une compagnie fictive, et la copie de developpement (le meme marqueur, sans la marque fictive).
    for (nom, marque) in [("paies-fictive", "compagnie fictive 3090 petite"), ("paies-copie", "prive")] {
        let p = poste_des_paies(nom);
        p.serveur(&url, json!({}));
        std::fs::write(p.racine.join("COPIE-DE-DEVELOPPEMENT.json"), json!({"copie_de": marque}).to_string()).unwrap();
        p.ouvrir_le("2026-01-20");
        assert_eq!(paies_parties(&recu).len(), 1, "{}", nom);
    }
    // Un logiciel bride (le geste), et le paquet soude.
    let p = poste_des_paies("paies-bride");
    p.serveur(&url, json!({}));
    p.edition("bride");
    p.ouvrir_le("2026-01-20");
    std::fs::write(p.base.join("logiciel/EDITION.json"), r#"{"edition": "bride"}"#).unwrap();
    p.edition("debloque");
    p.ouvrir_le("2026-01-20");
    assert_eq!(paies_parties(&recu).len(), 1, "rien de plus n'est parti");
}

#[test]
fn un_envoi_qui_echoue_se_refait_au_passage_suivant() {
    let p = poste_des_paies("paies-echec");
    // Le serveur en panne : rien n'est retenu.
    let (url, recu) = serveur(|action, _| if action == "paies" { (503, json!({"erreur": "en panne"})) } else { (200, json!({})) });
    p.serveur(&url, json!({}));
    p.ouvrir_le("2026-01-16");
    assert_eq!(paies_parties(&recu).len(), 1, "il a essaye");
    assert!(p.fichier("config/diagnostic.json").get("paies_annuelles").is_none(), "et n'a rien retenu");
    // Hors ligne (le serveur ne repond plus du tout) : rien non plus.
    let mut d = p.fichier("config/diagnostic.json");
    d["serveur"] = json!("http://127.0.0.1:9");
    p.ecrire("config/diagnostic.json", &d);
    p.serveur("http://127.0.0.1:9", json!({}));
    p.ouvrir_le("2026-01-17");
    assert!(p.fichier("config/diagnostic.json").get("paies_annuelles").is_none());
    // Le serveur revenu : le passage suivant le refait, une fois.
    let (url2, recu2) = serveur_des_paies();
    p.serveur(&url2, json!({}));
    p.ouvrir_le("2026-01-18");
    p.ouvrir_le("2026-01-18");
    let partis = paies_parties(&recu2);
    assert_eq!(partis.len(), 1);
    assert_eq!(partis[0].corps["paies"], json!(18));
}

/// Aucun essai ne sort de la machine : sans serveur pose, l'envoi viserait celui de l'editeur, et le garde-fou des essais le
/// refuse avant toute connexion ; le faux mandataire, seule porte, voit qu'on voulait y aller, et rien n'est retenu.
#[test]
fn sans_serveur_pose_le_nombre_de_paies_ne_sort_pas_de_la_machine() {
    let p = poste_des_paies("paies-garde");
    let (mandataire, vus) = crate::outils::mandataire();
    let r = p.route_env("ouverture", json!({"retours": false, "signatures": false, "sauvegarder": false}), "2026-01-16", &[(socle::garde_reseau::VARIABLE, mandataire.as_str())]);
    assert_eq!(r["ok"], json!(true), "{}", r);
    let vus = vus.lock().unwrap().clone();
    assert!(!vus.is_empty() && vus.iter().all(|v| v.contains("filet.puppetpaie.fr")), "{:?}", vus);
    let d = p.racine.join("config/diagnostic.json");
    assert!(!d.exists() || p.fichier("config/diagnostic.json").get("paies_annuelles").is_none());
}
