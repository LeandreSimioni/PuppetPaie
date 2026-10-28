//! La mise a jour d'une INSTALLATION (`paie::maj`) : `--cle-github` lance depuis une installation. Le programme de l'archive
//! est le `paie` des essais, copie sous le nom de la fenetre de ce systeme (`linux/puppetpaie` ici). Rien ne sort du dossier
//! du test, rien ne touche le reseau.

use std::path::{Path, PathBuf};

use paie::maj;

use crate::outils::{code_livre, copier, dossier_de_test};

/// Un `logiciel/` : VERSION, les donnees livrees, une page, et le programme de ce systeme (le `paie` des essais, nomme
/// comme la fenetre : `linux/puppetpaie` ici).
fn logiciel(dossier: &Path, version: &str) -> PathBuf {
    copier(&code_livre(), &dossier.join("donnees"));
    std::fs::create_dir_all(dossier.join("interface")).unwrap();
    std::fs::write(dossier.join("interface").join("index.html"), format!("<p>{}</p>\n", version)).unwrap();
    std::fs::write(dossier.join("VERSION"), format!("{}\n", version)).unwrap();
    let exe = maj::programme(dossier, "puppetpaie");
    std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
    std::fs::copy(env!("CARGO_BIN_EXE_paie"), &exe).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    dossier.to_path_buf()
}

/// `--cle-github` SANS DOSSIER, lance depuis une installation (le programme dans `<installation>/logiciel/`) : la graine
/// va dans `<installation>/maj/`, pas dans le dossier ou le programme a ete compile. Le 14/10/2026, la commande prenait
/// le chemin de compilation (`installation()` avant `fixer_le_code`) et Windows a ecrit la graine dans
/// `D:\home\user\puppetpaieprivate\maj\`.
#[test]
fn la_cle_de_github_va_dans_le_dossier_maj_de_l_installation() {
    let inst = dossier_de_test("mise_a_jour", "cle_github_installation");
    let code = logiciel(&inst.join("logiciel"), "2026.10.14");
    let s = std::process::Command::new(maj::programme(&code, "puppetpaie"))
        .args(["maj", "--cle-github"])
        .env_remove("PUPPETPAIE_CODE")
        .env_remove(maj::VARIABLE_GRAINE)
        .current_dir(&inst)
        .output()
        .unwrap();
    let dit = format!("{}{}", String::from_utf8_lossy(&s.stdout), String::from_utf8_lossy(&s.stderr));
    assert_eq!(s.status.code(), Some(0), "{}", dit);
    assert!(inst.join("maj").join("cle-github-graine.txt").is_file(), "{}", dit);
    assert!(inst.join("maj").join("cle-github-publique.txt").is_file(), "{}", dit);
}
