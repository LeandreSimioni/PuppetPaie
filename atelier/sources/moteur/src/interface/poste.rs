//! LES GESTES DU POSTE : ouvrir un fichier ou un dossier dans l'explorateur, choisir un dossier dans la boite du
//! systeme, ouvrir une adresse dans le navigateur. Ce ne sont pas des gestes du logiciel mais de l'outil qui regarde :
//! une route rend le chemin, la fenetre l'ouvre (`window.pywebview.api.ouvrir`, `choisir_dossier`, servis par
//! `interface::Fenetre::lanceur`) ; en ligne de commande, le LLM lit le chemin.

use std::path::{Component, Path};
use std::process::{Command, Stdio};

/// Ouvre un fichier ou un dossier dans le programme du poste (l'explorateur, le lecteur PDF), et rend ce que l'ecran en
/// dit : « Dossier ouvert : … » (l'explorateur peut s'ouvrir derriere la fenetre), rien pour un fichier ouvert, ou, quand
/// aucun programme ne l'a ouvert, ou il est (une information : il est ecrit). `Err` : il n'existe pas.
pub fn ouvrir(chemin: &Path) -> Result<String, String> {
    // Une piece de l'Historique arrive en `retours/...` (barres droites) jointe a la racine : Windows lit le fichier,
    // mais l'explorateur ne reconnait pas le chemin et ouvre « Documents » a la place.
    #[cfg(windows)]
    let r = Command::new("explorer").arg(separateurs_windows(&chemin.to_string_lossy())).spawn();
    #[cfg(target_os = "macos")]
    let r = Command::new("open").arg(chemin).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let r = Command::new("xdg-open").arg(chemin).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    match r {
        Ok(_) if chemin.is_dir() => Ok(format!("Dossier ouvert : {}", chemin.display())),
        // Ce qui s'ouvre se nomme : si le poste n'a pas de programme pour le montrer, on sait ou le prendre.
        Ok(_) => Ok(format!("Ouvert : {}", chemin.display())),
        Err(e) => pas_ouvert(chemin, &e),
    }
}

/// Ce que dit un fichier qu'aucun programme du poste n'a ouvert : s'il est ecrit, ou il est ; sinon, qu'il manque.
fn pas_ouvert(chemin: &Path, e: &std::io::Error) -> Result<String, String> {
    if !chemin.exists() {
        return Err(format!("{} n'existe pas : rien à ouvrir ({}).", chemin.display(), e));
    }
    let quoi = if chemin.is_dir() {
        "Le dossier"
    } else if chemin.extension().is_some_and(|x| x.eq_ignore_ascii_case("pdf")) {
        "Le PDF"
    } else {
        "Le fichier"
    };
    Ok(format!("{} est là : {}. Aucun programme du poste ne l'a ouvert.", quoi, chemin.display()))
}

/// Un chemin tel que l'explorateur de Windows le comprend : rien que des barres inverses.
pub fn separateurs_windows(chemin: &str) -> String {
    chemin.replace('/', "\\")
}

/// Ouvre une adresse dans le navigateur du poste.
pub fn ouvrir_adresse(url: &str) -> Result<(), String> {
    // En essai, le garde-fou reseau : aucun navigateur vers une adresse reelle.
    socle::garde_reseau::directe(&socle::garde_reseau::hote_de(url))?;
    #[cfg(windows)]
    let r = Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).spawn();
    #[cfg(target_os = "macos")]
    let r = Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let r = Command::new("xdg-open").arg(url).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    r.map(|_| ()).map_err(|e| format!("Le navigateur ne s'ouvre pas : {}.", e))
}

/// Un texte pour une chaine PowerShell entre apostrophes : chaque apostrophe se double, et PowerShell en reconnait
/// cinq (l'apostrophe droite et les quatre typographiques U+2018 a U+201B) ; rien d'autre n'y est special.
pub fn entre_apostrophes_powershell(t: &str) -> String {
    let mut s = String::with_capacity(t.len());
    for c in t.chars() {
        if matches!(c, '\'' | '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}') {
            s.push(c);
        }
        s.push(c);
    }
    s
}

/// CHOISIR UN DOSSIER. La boite du systeme, lancee par la fenetre : Windows par PowerShell (`FolderBrowserDialog`, au
/// premier plan), macOS par `osascript`, Linux par `zenity` ou `kdialog`. Rend le chemin choisi, ou `None` si on a annule.
pub fn choisir_dossier(titre: &str) -> Result<Option<String>, String> {
    // Le titre garde son apostrophe : chaque boite l'echappe a sa facon -- PowerShell double l'apostrophe
    // dans sa chaine entre apostrophes (ou `$` et l'accent grave sont litteraux), AppleScript echappe `\` et `"`
    // dans sa chaine entre guillemets ; zenity le recoit en argument, tel quel.
    let titre: String = if titre.trim().is_empty() { "Choisir un dossier" } else { titre }.chars().filter(|c| !c.is_control()).collect();
    #[cfg(windows)]
    let sortie = {
        use std::os::windows::process::CommandExt;
        let script = format!(
            "[Console]::OutputEncoding=[Text.Encoding]::UTF8; Add-Type -AssemblyName System.Windows.Forms;              $f=New-Object System.Windows.Forms.Form -Property @{{TopMost=$true}};              $d=New-Object System.Windows.Forms.FolderBrowserDialog; $d.Description='{}'; $d.ShowNewFolderButton=$false;              if($d.ShowDialog($f) -eq 'OK'){{ $d.SelectedPath }}",
            entre_apostrophes_powershell(&titre)
        );
        Command::new("powershell")
            .args(["-NoProfile", "-STA", "-Command", &script])
            .creation_flags(0x0800_0000)
            .stdin(Stdio::null())
            .output()
    };
    #[cfg(target_os = "macos")]
    let sortie = Command::new("osascript")
        .args(["-e", &format!("POSIX path of (choose folder with prompt \"{}\")", titre.replace('\\', "\\\\").replace('"', "\\\""))])
        .stdin(Stdio::null())
        .output();
    #[cfg(all(unix, not(target_os = "macos")))]
    let sortie = Command::new("zenity")
        .args(["--file-selection", "--directory", &format!("--title={}", titre)])
        .stdin(Stdio::null())
        .output()
        .or_else(|_| Command::new("kdialog").args(["--getexistingdirectory", "."]).stdin(Stdio::null()).output());
    let sortie = sortie.map_err(|e| format!("La boite de choix du dossier ne s'ouvre pas : {}. Tapez le chemin a la main.", e))?;
    let chemin = String::from_utf8_lossy(&sortie.stdout).trim().trim_end_matches(['/', '\\']).to_string();
    Ok((!chemin.is_empty()).then_some(chemin))
}

/// Ce que la fenetre ouvre pour la page d'une compagnie : `chemin`, tel qu'une route l'a rendu, relatif a la compagnie
/// ou absolu ; jamais par `..`. Un chemin absolu hors de la compagnie n'est ouvert que s'il est un dossier (celui que
/// l'on a choisi pour l'import). `creer` : un dossier de la compagnie qui manque se cree (la boite `import/`).
pub fn ouvrir_pour_la_page(racine: &Path, chemin: &str, creer: bool) -> Result<String, String> {
    let p = Path::new(chemin.trim());
    if chemin.trim().is_empty() || p.components().any(|c| c == Component::ParentDir) {
        return Err(format!("{} : rien à ouvrir.", chemin));
    }
    let f = if p.is_absolute() { p.to_path_buf() } else { racine.join(p) };
    let dans = f.starts_with(racine);
    if creer && dans && !f.exists() {
        std::fs::create_dir_all(&f).map_err(|e| format!("Le dossier {} ne se crée pas : {}.", f.display(), e))?;
    }
    if !dans && !f.is_dir() {
        return Err(format!("{} n'est pas un dossier : rien à ouvrir.", f.display()));
    }
    ouvrir(&f)
}

#[cfg(test)]
mod tests {
    /// « Ouvrir la piece telle quelle » ouvrait « Documents » : l'explorateur recevait `...\prive\retours/x.xml`.
    #[test]
    fn l_explorateur_recoit_un_chemin_windows() {
        assert_eq!(
            super::separateurs_windows(r"C:\PuppetPaie\prive\retours/20261019-191514_nature45_FLUX.xml"),
            r"C:\PuppetPaie\prive\retours\20261019-191514_nature45_FLUX.xml"
        );
    }
}
