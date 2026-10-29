//! La fenetre sans Tauri (`moteur/fenetre.py`) : la page dans un navigateur de la famille Chromium en mode application
//! (`--app`, une fenetre sans onglets ni barre d'adresse, son propre profil), sinon dans le navigateur par defaut.
//! `PUPPETPAIE_NAVIGATEUR` choisit a la main : une commande (l'adresse est ajoutee a la fin), ou `defaut`.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

/// Ce qui appartient a l'utilisateur de ce poste (le profil du navigateur) : ni dans les donnees, ni a cote du code.
pub fn dossier_utilisateur() -> PathBuf {
    let env = |n: &str| std::env::var_os(n).filter(|v| !v.is_empty()).map(PathBuf::from);
    let maison = env("HOME").or_else(|| env("USERPROFILE")).unwrap_or_default();
    if cfg!(windows) {
        env("LOCALAPPDATA").unwrap_or_else(|| maison.join("AppData").join("Local")).join("PuppetPaie")
    } else if cfg!(target_os = "macos") {
        maison.join("Library").join("Application Support").join("PuppetPaie")
    } else {
        env("XDG_DATA_HOME").unwrap_or_else(|| maison.join(".local").join("share")).join("puppetpaie")
    }
}

fn candidats() -> Vec<PathBuf> {
    if cfg!(windows) {
        let racines: Vec<PathBuf> =
            ["PROGRAMFILES(X86)", "PROGRAMFILES", "LOCALAPPDATA"].iter().filter_map(|v| std::env::var_os(v).map(PathBuf::from)).collect();
        let relatifs = [
            r"Microsoft\Edge\Application\msedge.exe",
            r"Google\Chrome\Application\chrome.exe",
            r"Chromium\Application\chrome.exe",
            r"BraveSoftware\Brave-Browser\Application\brave.exe",
        ];
        relatifs.iter().flat_map(|rel| racines.iter().map(move |r| r.join(rel))).collect()
    } else if cfg!(target_os = "macos") {
        ["Google Chrome", "Microsoft Edge", "Chromium", "Brave Browser"]
            .iter()
            .map(|n| PathBuf::from(format!("/Applications/{n}.app/Contents/MacOS/{n}")))
            .collect()
    } else {
        let chemins: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
        ["google-chrome", "google-chrome-stable", "chromium", "chromium-browser", "microsoft-edge", "microsoft-edge-stable", "brave-browser", "vivaldi"]
            .iter()
            .flat_map(|n| chemins.iter().map(move |d| d.join(n)))
            .collect()
    }
}

/// Le premier navigateur de la famille Chromium installe.
pub fn chromium() -> Option<PathBuf> {
    candidats().into_iter().find(|c| c.is_file())
}

fn sans_console(c: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
}

/// Ouvre la page. Rend le processus du navigateur quand on le tient (un Chromium a nous), sinon `None`. `profil` : le
/// profil du Chromium a nous, un par fenetre (`instance::dossier_local`) : deux installations ouvertes ensemble ne
/// partagent pas le leur (un second Chromium sur le meme profil passe la main au premier et s'arrete aussitot).
pub fn ouvrir(url: &str, profil: &std::path::Path, journal: &dyn Fn(&str)) -> Option<Child> {
    let choix = std::env::var("PUPPETPAIE_NAVIGATEUR").unwrap_or_default();
    let choix = choix.trim();
    if !choix.eq_ignore_ascii_case("defaut") {
        let commande: Option<(PathBuf, Vec<String>)> = if !choix.is_empty() {
            let mut morceaux = choix.split_whitespace().map(String::from);
            morceaux.next().map(|exe| (PathBuf::from(exe), morceaux.chain([url.to_string()]).collect()))
        } else {
            chromium().map(|exe| {
                let mut a = vec![format!("--app={}", url), "--no-first-run".into(), "--no-default-browser-check".into(), "--window-size=940,640".into()];
                // Un Chromium en snap n'ecrit pas hors de son bac : il garde son profil.
                let reel = std::fs::canonicalize(&exe).unwrap_or_else(|_| exe.clone());
                if !reel.to_string_lossy().contains("/snap/") {
                    let _ = std::fs::create_dir_all(profil);
                    a.insert(0, format!("--user-data-dir={}", profil.display()));
                }
                (exe, a)
            })
        };
        if let Some((exe, args)) = commande {
            match sans_console(Command::new(&exe).args(&args)).spawn() {
                Ok(p) => {
                    journal(&format!("Fenetre : {}\n", exe.display()));
                    return Some(p);
                }
                Err(e) => journal(&format!("Navigateur {} injoignable ({}) ; navigateur par defaut.\n", exe.display(), e)),
            }
        }
    }
    journal("Fenetre : le navigateur par defaut.\n");
    let _ = super::api::ouvrir_adresse(url);
    None
}
