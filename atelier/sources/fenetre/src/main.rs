//! PuppetPaie : la fenetre (lot J3). Le serveur de la fenetre (`paie::interface`) tourne dans ce processus, et sa page
//! s'affiche dans une fenetre Tauri (WebView2 sous Windows, WebKit sous Mac et Linux). Les routes du moteur tournent
//! chacune dans son processus, lance depuis ce meme programme (`puppetpaie route ...`, comme `paie route ...`).
//!
//! Sans Tauri (un poste sans moteur de rendu), `paie interface` ouvre la meme page dans le navigateur.

#![cfg_attr(windows, windows_subsystem = "windows")]

use std::sync::{Arc, Mutex, OnceLock};

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

static APPLICATION: OnceLock<tauri::AppHandle> = OnceLock::new();

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
        // Le relais de la mise a jour (`paie::maj`) : lance depuis prive/maj/, il bascule puis rouvre la fenetre.
        Some("maj-relais") => std::process::exit(paie::maj::relais(&args[1..])),
        // `puppetpaie maj` : chercher et preparer une version tout de suite (le bac a sable de l'atelier s'en sert).
        Some("maj") => std::process::exit(paie::maj::commande(&args[1..])),
        _ => {}
    }

    paie::commandes::compagnie::fixer_le_code();
    let Some(_verrou) = paie::interface::prendre_instance(devant) else {
        return std::process::ExitCode::SUCCESS;
    };
    // Une version prete s'applique ICI, avant toute fenetre : le relais attend notre fin, bascule et ouvre la neuve.
    if paie::maj::au_lancement() {
        return std::process::ExitCode::SUCCESS;
    }
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
            WebviewWindowBuilder::new(app, "principale", WebviewUrl::External(url))
                .title("PuppetPaie")
                .inner_size(1200.0, 800.0)
                .min_inner_size(700.0, 500.0)
                .center()
                .maximized(true)
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
