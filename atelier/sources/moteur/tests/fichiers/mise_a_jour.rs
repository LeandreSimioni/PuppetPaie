//! La mise a jour d'une INSTALLATION (`moteur::maj`) : `--cle-github` lance depuis une installation. Le programme de l'archive
//! est le `moteur` des essais, copie sous le nom de la fenetre de ce systeme (`linux/puppetpaie` ici). Rien ne sort du dossier
//! du test, rien ne touche le reseau.

use std::path::{Path, PathBuf};

use moteur::maj;
use serde_json::json;

use crate::outils::{code_livre, copier, dossier_de_test};

/// Un `logiciel/` : VERSION, les donnees livrees, une page, et le programme de ce systeme (le `moteur` des essais, nomme
/// comme la fenetre : `linux/puppetpaie` ici).
fn logiciel(dossier: &Path, version: &str) -> PathBuf {
    copier(&code_livre(), &dossier.join("donnees"));
    std::fs::create_dir_all(dossier.join("interface")).unwrap();
    std::fs::write(dossier.join("interface").join("index.html"), format!("<p>{}</p>\n", version)).unwrap();
    std::fs::write(dossier.join("VERSION"), format!("{}\n", version)).unwrap();
    let exe = maj::programme(dossier, "puppetpaie");
    std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
    std::fs::copy(env!("CARGO_BIN_EXE_moteur"), &exe).unwrap();
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

/// LES MOIS SAISIS EN DEMO d'une compagnie gardee au deblocage (ticket #236) ne retiennent pas une version qui change un
/// calcul : le logiciel ne les a jamais declares. Un mois saisi apres qu'elle a ete gardee retient comme toute reelle.
#[test]
fn les_mois_faits_en_demo_d_une_compagnie_gardee_ne_retiennent_pas_la_version() {
    let base = dossier_de_test("mise_a_jour", "garde_fou_demo");
    let inst = base.join("installation");
    logiciel(&inst.join("logiciel"), "2026.10.09");
    logiciel(&inst.join("maj").join("2026.11.01").join("logiciel"), "2026.11.01");
    crate::outils::ecrire(&inst, "maj/prete.json", &json!({"version": "2026.11.01", "calcul": "la CSG"}));
    // Bride : le « + » ajoute une fictive ; on y saisit un mois, jamais declare.
    let ajoutee = moteur::interface::compagnies::ajouter_dans(&inst, "Les Pupazzi").unwrap();
    let mois = |m: &str| json!({"mois": m, "engagements": [{"salarie": "INVENTE"}]});
    crate::outils::ecrire(&ajoutee.racine, "engagements/2030-01.json", &mois("2030-01"));
    // Debloque, on la garde : une compagnie reelle.
    crate::outils::debloquer(&inst);
    let poste = moteur::administrateur::Poste { dossier_cle: base.join("cle"), maintenant: "2026-10-23T10:00".into() };
    let v = moteur::interface::suppression::repondre(&inst, std::slice::from_ref(&ajoutee.id), &[], &poste, &|_| {}).unwrap();
    assert_eq!(v["erreurs"], json!([]), "{}", v);
    let gardee = inst.join("compagnies").join("les-pupazzi");
    assert!(gardee.join("engagements/2030-01.json").is_file());
    assert_eq!(maj::applicable(&inst).as_deref(), Some("2026.11.01"), "le mois fait en demo ne retient rien");
    // Un mois saisi apres : il attend sa DSN.
    crate::outils::ecrire(&gardee, "engagements/2030-02.json", &mois("2030-02"));
    assert!(maj::applicable(&inst).is_none(), "le mois 2030-02, saisi apres, attend sa DSN");
    let refus = maj::appliquer(&inst, false).unwrap_err();
    assert!(refus.contains("2030-02 (les-pupazzi)") && !refus.contains("2030-01"), "{}", refus);
}
