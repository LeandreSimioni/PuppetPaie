//! Le serveur de la fenetre sert la page coupee en fichiers (lot L, pas 3) : chaque `<script src>` d'`index.html` est
//! servi en `application/javascript`, l'empreinte suit les fichiers de `js/`, la cale precede le premier script, et un
//! chemin qui sort d'`interface/` rend 404.
//!
//! Aucune prise reseau : `Fenetre::traiter` est appele directement. Le logiciel est une COPIE de `logiciel/interface/`
//! dans `CARGO_TARGET_TMPDIR` (`PUPPETPAIE_CODE` y pointe) ; le jeton de la fenetre s'ecrit dans un dossier
//! utilisateur de ce meme dossier temporaire, jamais dans celui du poste. La liste des scripts se LIT dans
//! l'`index.html` copie : elle n'est pas recopiee ici.

use paie::interface::http::{Reponse, Requete};
use paie::interface::Fenetre;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use outils::{copier, dossier_du_processus};

#[path = "outils/mod.rs"]
mod outils;

struct Banc {
    logiciel: PathBuf,
    compagnie: PathBuf,
    fenetre: Arc<Fenetre>,
}

static BANC: OnceLock<Banc> = OnceLock::new();
/// Un test qui change un fichier de la copie ne croise pas un test qui la lit.
static VERROU: Mutex<()> = Mutex::new(());

fn banc() -> (&'static Banc, MutexGuard<'static, ()>) {
    let verrou = VERROU.lock().unwrap_or_else(|e| e.into_inner());
    let b = BANC.get_or_init(|| {
        let base = dossier_du_processus("interface-fichiers");
        let logiciel = base.join("logiciel");
        let origine = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../logiciel/interface");
        copier(&origine, &logiciel.join("interface"));
        std::fs::create_dir_all(logiciel.join("donnees")).unwrap();
        std::fs::write(logiciel.join("VERSION"), "2026.10.09\n").unwrap();
        // Hors d'`interface/` : ce qu'un chemin qui sort ne doit jamais atteindre.
        std::fs::write(logiciel.join("donnees").join("hors.js"), "hors de la page").unwrap();
        std::fs::write(base.join("hors.css"), "hors du logiciel").unwrap();
        // Dans `interface/`, mais d'une extension que la page ne sert pas.
        std::fs::write(logiciel.join("interface").join("note.txt"), "pas servi").unwrap();
        let utilisateur = base.join("utilisateur");
        std::fs::create_dir_all(&utilisateur).unwrap();
        std::env::set_var("PUPPETPAIE_CODE", logiciel.join("donnees"));
        for v in ["LOCALAPPDATA", "HOME", "USERPROFILE", "XDG_DATA_HOME"] {
            std::env::set_var(v, &utilisateur);
        }
        // Debloquee par le geste (ticket #246) : bridee, l'accueil n'ajouterait pas la compagnie reelle d'un test.
        outils::debloquer(&base);
        let compagnie = base.join("compagnie");
        std::fs::create_dir_all(compagnie.join("config")).unwrap();
        let fenetre = Fenetre::nouvelle(0, utilisateur.clone(), 1);
        Banc { logiciel, compagnie, fenetre }
    });
    (b, verrou)
}

fn get(b: &Banc, chemin: &str) -> Reponse {
    let (chemin, requete) = chemin.split_once('?').unwrap_or((chemin, ""));
    let params: HashMap<String, String> =
        requete.split('&').filter(|p| !p.is_empty()).map(|p| p.split_once('=').unwrap_or((p, ""))).map(|(k, v)| (k.into(), v.into())).collect();
    b.fenetre.traiter(Requete { methode: "GET".into(), chemin: chemin.into(), params, entetes: HashMap::new(), corps: Vec::new() })
}

fn index(b: &Banc) -> String {
    std::fs::read_to_string(b.logiciel.join("interface").join("index.html")).unwrap()
}

/// Les valeurs d'un attribut (`src="` ou `href="`) qui commencent par `/`, dans l'ordre de la page.
fn adresses(html: &str, attribut: &str) -> Vec<String> {
    html.split(attribut).skip(1).filter_map(|s| s.split('"').next()).filter(|s| s.starts_with('/')).map(String::from).collect()
}

fn scripts(html: &str) -> Vec<String> {
    html.split("<script src=\"").skip(1).filter_map(|s| s.split('"').next()).map(String::from).collect()
}

#[test]
fn chaque_script_de_la_page_est_servi_tel_quel() {
    let (b, _v) = banc();
    let html = index(b);
    let liste = scripts(&html);
    assert!(liste.iter().filter(|s| s.starts_with("/js/")).count() >= 18, "la page charge ses scripts de js/ : {:?}", liste);
    for s in &liste {
        let r = get(b, s);
        assert_eq!(r.code, 200, "{}", s);
        assert_eq!(r.type_mime, "application/javascript; charset=utf-8", "{}", s);
        let chemin = s.split('?').next().unwrap().trim_start_matches('/');
        assert_eq!(r.corps, std::fs::read(b.logiciel.join("interface").join(chemin)).unwrap(), "{} servi tel quel", s);
    }
    // Aucun fichier de js/ n'est oublie par la page (un fichier non charge, ce sont des fonctions qui manquent).
    for e in std::fs::read_dir(b.logiciel.join("interface").join("js")).unwrap().flatten() {
        let nom = e.file_name().to_string_lossy().into_owned();
        if nom.ends_with(".js") {
            assert!(liste.iter().any(|s| s.starts_with(&format!("/js/{}?", nom)) || *s == format!("/js/{}", nom)), "js/{} n'est pas charge par index.html", nom);
        }
    }
    // La feuille, et ce que la page appelle d'autre.
    let feuilles: Vec<String> = adresses(&html, "href=\"").into_iter().filter(|h| h.contains(".css")).collect();
    assert!(!feuilles.is_empty(), "la page a sa feuille de style");
    let mut autres: Vec<(&str, &str)> = vec![
        ("/html2canvas.min.js", "application/javascript; charset=utf-8"),
        ("/puppetpaie.png", "image/png"),
        ("/puppetpaie.svg", "image/svg+xml"),
    ];
    autres.extend(feuilles.iter().map(|h| (h.as_str(), "text/css; charset=utf-8")));
    for (chemin, mime) in autres {
        let r = get(b, chemin);
        assert_eq!((r.code, r.type_mime.as_str()), (200, mime), "{}", chemin);
    }
}

#[test]
fn un_chemin_qui_sort_de_la_page_rend_404() {
    let (b, _v) = banc();
    for chemin in [
        "/js/../x",
        "/../x",
        "/js/../index.html",
        "/js/../interface.css",
        "/./interface.css",
        "/js/./tuyau.js",
        "/../donnees/hors.js",
        "/js/../../donnees/hors.js",
        "/../../hors.css",
        "/..\\donnees\\hors.js",
        "/js\\..\\interface.css",
        "//interface.css",
        "/C:/Windows/win.ini",
        "/C:interface.css",
        "/interface.css:$DATA",
        "/js/",
        "/js",
        "/note.txt",
        "/js/absent.js",
        "/",
        "",
    ] {
        let r = get(b, chemin);
        if chemin == "/" {
            // La page elle-meme demande le jeton : elle ne passe pas par le service des fichiers.
            assert_eq!(r.code, 403, "{}", chemin);
            continue;
        }
        assert_eq!(r.code, 404, "{} rend {}", chemin, r.code);
        assert!(r.corps.is_empty(), "{}", chemin);
    }
    // Le meme fichier, par le bon chemin, est bien servi : c'est le chemin qui est refuse, pas le fichier.
    assert_eq!(get(b, "/interface.css").code, 200);
    assert_eq!(get(b, "/js/tuyau.js").code, 200);
}

#[test]
fn la_cale_precede_le_premier_script_et_l_empreinte_suit_js() {
    let (b, _v) = banc();
    let page = || paie::interface::page_logiciel(&b.compagnie, "jeton-du-banc-0123456789abcdef").unwrap();
    let p = page();
    assert!(!p.contains("{{"), "aucun gabarit oublie");
    let cale = p.find("window.pywebview = { api }").expect("la cale est dans la page");
    assert_eq!(p.matches("window.pywebview = { api }").count(), 1, "une seule cale");
    let premier = p.find("<script src=\"/js/").expect("un script de js/");
    assert!(cale < premier, "la cale vient avant le premier script de js/");
    assert!(p[cale..premier].contains("</script>"), "la cale est fermee avant le premier script");

    let v = |p: &str| -> Vec<String> { scripts(p).iter().map(|s| s.split_once("?v=").map(|(_, e)| e.to_string()).unwrap_or_default()).collect() };
    let avant = v(&p);
    assert!(avant.iter().all(|e| e.len() == 12 && e == &avant[0]), "une seule empreinte, de douze caracteres : {:?}", avant);
    assert_eq!(v(&page()), avant, "la meme d'un appel a l'autre");

    // Un fichier de js/ change : l'empreinte change ; il revient : elle revient.
    let fichier = b.logiciel.join("interface").join("js").join("boites.js");
    let original = std::fs::read(&fichier).unwrap();
    let mut change = original.clone();
    change.extend_from_slice(b"\n/* change */\n");
    std::fs::write(&fichier, &change).unwrap();
    let apres = v(&page());
    std::fs::write(&fichier, &original).unwrap();
    assert_ne!(apres[0], avant[0], "un js/ change, l'empreinte aussi");
    assert_eq!(v(&page())[0], avant[0], "le fichier revenu, l'empreinte revient");

    // Un fichier de js/ en plus (ce que le pas suivant ajoutera) change aussi l'empreinte.
    let neuf = b.logiciel.join("interface").join("js").join("zz-neuf.js");
    std::fs::write(&neuf, "/* neuf */\n").unwrap();
    let avec = v(&page());
    std::fs::remove_file(&neuf).unwrap();
    assert_ne!(avec[0], avant[0], "un js/ de plus, l'empreinte change");

    // La feuille de style aussi.
    let css = b.logiciel.join("interface").join("interface.css");
    let original = std::fs::read(&css).unwrap();
    std::fs::write(&css, [original.as_slice(), b"\n/* change */\n"].concat()).unwrap();
    let apres = v(&page());
    std::fs::write(&css, &original).unwrap();
    assert_ne!(apres[0], avant[0], "la feuille change, l'empreinte aussi");
}

/// La version se lit JJ.MM.AAAA (Leandre, 14/10/2026 ; `prive/journal/ETAT.md`, « DECIDE LE 14/10/2026 ») : le banc a
/// `VERSION` = `2026.10.09`, la page dit `version 09.10.2026`, et rien n'y commence par l'annee.
#[test]
fn la_page_montre_la_version_jour_mois_annee() {
    let (b, _v) = banc();
    let p = paie::interface::page_logiciel(&b.compagnie, "jeton-du-banc-0123456789abcdef").unwrap();
    assert!(p.contains("version 09.10.2026"), "la version lue est sous le nom");
    let annee_d_abord = p.as_bytes().windows(10).any(|w| {
        w.iter().enumerate().all(|(i, c)| if i == 4 || i == 7 { *c == b'.' } else { c.is_ascii_digit() })
    });
    assert!(!annee_d_abord, "aucune version AAAA.MM.JJ dans la page");
}

/// Ticket #211 : l'accueil des compagnies dit la version aussi, lue JJ.MM.AAAA comme dans la page d'une compagnie ; et ce
/// qu'il appelle (la feuille et le script de l'indicateur de la mise a jour, ticket #210) est servi.
#[test]
fn l_accueil_montre_la_version_et_sert_ce_qu_il_appelle() {
    let (b, _v) = banc();
    let adresse = b.fenetre.adresse("127.0.0.1", 1);
    let jeton = adresse.split("jeton=").nth(1).unwrap();
    let r = get(b, &format!("/index.html?jeton={}", jeton));
    assert_eq!(r.code, 200);
    let html = String::from_utf8(r.corps).unwrap();
    assert!(html.contains("version 09.10.2026"), "la version lue est sous le nom");
    assert!(!html.contains("{{"), "aucune marque laissee");
    let appels: Vec<String> = adresses(&html, "src=\"").into_iter().chain(adresses(&html, "href=\"")).filter(|a| a.contains(".js") || a.contains(".css")).collect();
    assert!(appels.iter().any(|a| a.starts_with("/js/mise-a-jour.js")) && appels.iter().any(|a| a.starts_with("/mise-a-jour.css")), "{:?}", appels);
    for a in &appels {
        assert_eq!(get(b, a).code, 200, "{}", a);
    }
}

/// Ticket #218 : la capture d'ecran du « Un probleme, une idee ? » dessine la page par html2canvas 1.4.1, qui ne lit
/// que les couleurs hexadecimales, nommees, `rgb()`/`rgba()` et `hsl()`/`hsla()` ; une fonction de couleur plus recente
/// (`color-mix()`, que Chromium calcule en `color(srgb ...)`, `color()`, `lab()`, `lch()`, `oklab()`, `oklch()`,
/// `hwb()`) la fait echouer sur toute la page (« Attempting to parse an unsupported color function "color" »). Ni la
/// page servie (feuilles, scripts, `index.html`), ni les pages du moteur n'en emploient.
#[test]
fn la_page_n_emploie_aucune_couleur_que_la_capture_ne_lit_pas() {
    let racine = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut fichiers: Vec<PathBuf> = Vec::new();
    for dossier in [racine.join("../../../logiciel/interface"), racine.join("../../../logiciel/interface/js"), racine.join("src/interface/pages")] {
        for e in std::fs::read_dir(&dossier).unwrap() {
            let p = e.unwrap().path();
            let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("");
            let nom = p.file_name().and_then(|x| x.to_str()).unwrap_or("");
            if ["css", "js", "html"].contains(&ext) && nom != "html2canvas.min.js" {
                fichiers.push(p);
            }
        }
    }
    assert!(fichiers.len() > 10, "{:?}", fichiers);
    let mut fautes = Vec::new();
    for f in &fichiers {
        let t = std::fs::read_to_string(f).unwrap();
        let o = t.as_bytes();
        for fonction in ["color-mix(", "color(", "lab(", "lch(", "oklab(", "oklch(", "hwb("] {
            let mut depuis = 0;
            while let Some(i) = t[depuis..].find(fonction) {
                let i = depuis + i;
                let avant = if i == 0 { b' ' } else { o[i - 1] };
                if !(avant.is_ascii_alphanumeric() || avant == b'_' || avant == b'-' || avant == b'.') {
                    fautes.push(format!("{} : {}", f.display(), fonction));
                }
                depuis = i + fonction.len();
            }
        }
    }
    assert!(fautes.is_empty(), "{:?}", fautes);
}

fn poste(f: &Fenetre, jeton: &str, nom: &str, args: serde_json::Value) -> serde_json::Value {
    let entetes = HashMap::from([("x-jeton".to_string(), jeton.to_string())]);
    let corps = args.to_string().into_bytes();
    let r = f.traiter(Requete { methode: "POST".into(), chemin: format!("/lanceur/{}", nom), params: HashMap::new(), entetes, corps });
    assert_eq!(r.code, 200, "{}", nom);
    serde_json::from_slice(&r.corps).unwrap()
}

fn racine_servie(f: &Fenetre, jeton: &str) -> String {
    let params = HashMap::from([("jeton".to_string(), jeton.to_string())]);
    let r = f.traiter(Requete { methode: "GET".into(), chemin: "/index.html".into(), params, entetes: HashMap::new(), corps: Vec::new() });
    assert_eq!(r.code, 200);
    String::from_utf8(r.corps).unwrap()
}

/// Ce que la page appelle (`window.pywebview.api.<nom>`) est defini par la cale servie avec elle : un geste absent de
/// la liste n'existe pas dans la page, et le bouton qui l'appelle leve sans rien faire.
fn chaque_geste_appele_est_dans_la_cale(html: &str, page: &str) {
    let liste = html.split("for (const nom of ").nth(1).and_then(|s| s.split(") api[nom]").next()).expect("la cale et sa liste");
    let noms: Vec<String> = serde_json::from_str(liste).unwrap();
    for appel in html.split("window.pywebview.api.").skip(1) {
        let nom: String = appel.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
        assert!(noms.contains(&nom), "{} appelle `{}`, que sa cale ne definit pas : {:?}", page, nom, noms);
    }
}

/// L'identifiant de la tuile de la compagnie ajoutee (`cie:<dossier>`), lu dans l'accueil servi.
fn tuile_ajoutee(html: &str) -> String {
    let dossier = html.split("data-id=\"cie:").nth(1).and_then(|s| s.split('"').next()).expect("la tuile de la compagnie ajoutee");
    format!("cie:{}", dossier)
}

/// Capture de Leandre (Windows, version 20.10.2026) : sur la page du mot de passe d'une compagnie, « Retour » ne
/// faisait rien (le geste `accueil` manquait a la cale de la page). Les deux pages du mot de passe (le choix, a la
/// premiere ouverture, puis la saisie) n'appellent que des gestes que leur cale definit, offrent Retour et Echap, et
/// le geste `accueil` ramene la fenetre au choix de la compagnie : `/` sert de nouveau l'accueil.
#[test]
fn retour_et_echap_ramenent_des_pages_du_mot_de_passe_au_choix_de_la_compagnie() {
    let (_b, _v) = banc();
    let f = Fenetre::nouvelle(0, _b.logiciel.parent().unwrap().join("utilisateur"), 1);
    let adresse = f.adresse("127.0.0.1", 1);
    let jeton = adresse.split("jeton=").nth(1).unwrap().to_string();
    let est_l_accueil = |html: &str| html.contains("data-id=\"cie:") && !html.contains("id=\"mdp\"");
    let rien = serde_json::json!([]);
    chaque_geste_appele_est_dans_la_cale(&racine_servie(&f, &jeton), "l'accueil");

    // Une compagnie reelle neuve : le choix du mot de passe.
    let v = poste(&f, &jeton, "ajouter", serde_json::json!(["Compagnie du Retour"]));
    assert_eq!(v["ok"], serde_json::json!(true), "{}", v);
    let choix = racine_servie(&f, &jeton);
    assert!(choix.contains("Poser et ouvrir"), "la page du choix du mot de passe");
    chaque_geste_appele_est_dans_la_cale(&choix, "le choix du mot de passe");
    assert!(choix.contains("class=\"retour\"") && choix.contains("\"Escape\""), "Retour et Echap");
    let v = poste(&f, &jeton, "accueil", rien.clone());
    assert_eq!((&v["ok"], &v["recharger"]), (&serde_json::json!(true), &serde_json::json!(true)), "{}", v);
    let accueil = racine_servie(&f, &jeton);
    assert!(est_l_accueil(&accueil), "Retour ramene au choix de la compagnie");

    // Le mot de passe pose, la compagnie rouverte : la saisie du mot de passe.
    let id = tuile_ajoutee(&accueil);
    assert_eq!(poste(&f, &jeton, "choisir", serde_json::json!([id]))["ok"], serde_json::json!(true));
    let v = poste(&f, &jeton, "poser", serde_json::json!(["secret-du-retour", "secret-du-retour"]));
    assert_eq!(v["ok"], serde_json::json!(true), "{}", v);
    assert_eq!(poste(&f, &jeton, "accueil", rien.clone())["ok"], serde_json::json!(true));
    assert_eq!(poste(&f, &jeton, "choisir", serde_json::json!([id]))["ok"], serde_json::json!(true));
    let saisie = racine_servie(&f, &jeton);
    assert!(saisie.contains("id=\"mdp\"") && !saisie.contains("id=\"mdp2\""), "la page de la saisie du mot de passe");
    chaque_geste_appele_est_dans_la_cale(&saisie, "la saisie du mot de passe");
    assert!(saisie.contains("class=\"retour\"") && saisie.contains("\"Escape\""), "Retour et Echap");
    assert_eq!(poste(&f, &jeton, "accueil", rien)["ok"], serde_json::json!(true));
    assert!(est_l_accueil(&racine_servie(&f, &jeton)), "Retour ramene au choix de la compagnie");
}
