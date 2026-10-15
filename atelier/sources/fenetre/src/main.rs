//! PuppetPaie : la fenetre (lot J3). Le serveur de la fenetre (`paie::interface`) tourne dans ce processus, et sa page
//! s'affiche dans une fenetre Tauri (WebView2 sous Windows, WebKit sous Mac et Linux). Les routes du moteur tournent
//! chacune dans son processus, lance depuis ce meme programme (`puppetpaie route ...`, comme `paie route ...`).
//!
//! Sans Tauri (un poste sans moteur de rendu), `paie interface` ouvre la meme page dans le navigateur.

#![cfg_attr(windows, windows_subsystem = "windows")]

use std::sync::{Arc, Mutex, OnceLock};

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

static APPLICATION: OnceLock<tauri::AppHandle> = OnceLock::new();

/// La barre de titre que la page n'a pas (ticket #195) : pose dans chaque page de la fenetre, avant ses scripts.
/// Ce qu'il peut demander a la fenetre est borne par `capabilities/fenetre.json`.
const CADRE: &str = include_str!("cadre.js");

const LARGEUR_MIN: f64 = 700.0;
const HAUTEUR_MIN: f64 = 500.0;
/// La taille d'ouverture qu'on vise : de quoi voir un mois et ses tableaux sans prendre tout l'ecran.
const LARGEUR_VISEE: f64 = 1360.0;
const HAUTEUR_VISEE: f64 = 900.0;

/// La taille d'ouverture, en points : celle qu'on vise, sans depasser neuf dixiemes de la zone de travail de l'ecran
/// (l'ecran moins la barre des taches), ni descendre sous le minimum. Sans ecran connu, celle qu'on vise.
fn taille_d_ouverture(ecran: Option<tauri::Monitor>) -> (f64, f64) {
    let Some(m) = ecran else { return (LARGEUR_VISEE, HAUTEUR_VISEE) };
    let echelle = if m.scale_factor() > 0.0 { m.scale_factor() } else { 1.0 };
    let zone = m.work_area().size;
    let borne = |vise: f64, dispo: u32, min: f64| vise.min(dispo as f64 / echelle * 0.9).max(min);
    (borne(LARGEUR_VISEE, zone.width, LARGEUR_MIN), borne(HAUTEUR_VISEE, zone.height, HAUTEUR_MIN))
}

/// La fenetre revient devant (un second lancement).
fn devant() {
    if let Some(w) = APPLICATION.get().and_then(|a| a.get_webview_window("principale")) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Les sous-commandes du moteur, que la fenetre lance pour elle-meme : une route par processus, et la fabrique.
    match args.first().map(String::as_str) {
        Some("route") => match paie::service::commande("route", &args[1..]) {
            Some(code) => std::process::exit(code),
            None => std::process::exit(2),
        },
        Some("pdf") => return fabrique_pdf::ligne_de_commande::executer(&args[1..]),
        Some("interface") => std::process::exit(paie::interface::commande(&args[1..])),
        // Le lanceur Linux demande si la fenetre peut s'ouvrir : le programme se charge (WebKitGTK est la), et un ecran.
        Some("--peut-ouvrir") => {
            let ecran = |n: &str| std::env::var_os(n).is_some_and(|v| !v.is_empty());
            let ok = cfg!(any(windows, target_os = "macos")) || ecran("DISPLAY") || ecran("WAYLAND_DISPLAY");
            return if ok { std::process::ExitCode::SUCCESS } else { std::process::ExitCode::FAILURE };
        }
        // Le relais de la mise a jour (`paie::maj`) : lance depuis le `maj/` de l'installation, il bascule puis rouvre la fenetre.
        Some("maj-relais") => std::process::exit(paie::maj::relais(&args[1..])),
        // `puppetpaie maj` : chercher et preparer une version tout de suite (le bac a sable de l'atelier s'en sert).
        Some("maj") => std::process::exit(paie::maj::commande(&args[1..])),
        _ => {}
    }

    paie::commandes::compagnie::fixer_le_code();
    let Some(_verrou) = paie::interface::prendre_instance(devant) else {
        return std::process::ExitCode::SUCCESS;
    };
    // Une installation posee avant le lot U1 range ses compagnies fictives (`demo/` devient `fictives/`).
    paie::interface::preparer_l_installation();
    // Une version prete s'applique ICI, avant toute fenetre : le relais attend notre fin, bascule et ouvre la neuve.
    if paie::maj::au_lancement() {
        return std::process::ExitCode::SUCCESS;
    }
    // Sinon le logiciel cherche, en fond, une version plus recente pour l'installation : elle s'appliquera au prochain
    // lancement (lot U1 : c'est le logiciel qui se met a jour, une fois par installation, pas une compagnie).
    paie::maj::chercher_en_fond();
    let (fenetre, port) = match paie::interface::demarrer(paie::interface::MINUTES_VEILLE) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("La fenetre ne demarre pas : {}.", e);
            std::process::exit(1);
        }
    };
    let adresse = fenetre.adresse("127.0.0.1", port);
    let pour_fermer: Arc<Mutex<Option<tauri::AppHandle>>> = Arc::new(Mutex::new(None));
    let poignee = pour_fermer.clone();
    *fenetre.fermer.lock().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(move || {
        if let Some(a) = poignee.lock().ok().and_then(|a| a.clone()) {
            a.exit(0);
        }
    }));

    let resultat = tauri::Builder::default()
        .setup(move |app| {
            let _ = APPLICATION.set(app.handle().clone());
            *pour_fermer.lock().unwrap_or_else(|e| e.into_inner()) = Some(app.handle().clone());
            let url = adresse.parse().map_err(|e| format!("adresse illisible : {}", e))?;
            // UNE FENETRE SANS BARRE NOIRE, OUVERTE UNE FOIS (ticket #195). Pas de bordure systeme : le haut de la
            // fenetre est le bandeau de la page, et `cadre.js` en fait la barre de titre (tirer, double-clic, les trois
            // boutons). Elle s'ouvre a sa taille, centree, et n'en change plus : l'ancienne s'ouvrait en 1200 x 800
            // puis s'agrandissait aussitot, « ca a l'air de le faire et apres ca le fait plus ».
            let (largeur, hauteur) = taille_d_ouverture(app.primary_monitor().ok().flatten());
            WebviewWindowBuilder::new(app, "principale", WebviewUrl::External(url))
                .title("PuppetPaie")
                .decorations(false)
                .inner_size(largeur, hauteur)
                .min_inner_size(LARGEUR_MIN, HAUTEUR_MIN)
                .center()
                .initialization_script(CADRE)
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!());
    if let Err(e) = resultat {
        fenetre.ecrire(&format!("La fenetre Tauri ne s'ouvre pas ({}) ; le navigateur prend le relais.\n", e));
        let _ = paie::interface::navigateur::ouvrir(&fenetre.adresse("puppetpaie.localhost", port), &|t| fenetre.ecrire(t));
        loop {
            std::thread::sleep(std::time::Duration::from_secs(5));
            if fenetre.silence() > std::time::Duration::from_secs(300) {
                return std::process::ExitCode::SUCCESS;
            }
        }
    }
    std::process::ExitCode::SUCCESS
}
