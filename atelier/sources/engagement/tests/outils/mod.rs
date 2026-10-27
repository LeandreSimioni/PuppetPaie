//! Les aides des tests de l'engagement : une compagnie montee de la copie figee d'une fictive, dans le dossier du test.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use serde_json::Value;

/// La copie figee d'une compagnie fictive, celle de toutes les crates (`paie/tests/donnees/fictives/<nom>`), qu'aucun
/// test n'ecrit.
pub fn fictive(nom: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../paie/tests/donnees/fictives").join(nom)
}

/// `CARGO_TARGET_TMPDIR/<fichier>/<test>`, vide puis cree : le dossier d'un test, a lui seul.
pub fn dossier_de_test(fichier: &str, test: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("engagement").join(fichier).join(test);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn copier(de: &Path, vers: &Path) {
    std::fs::create_dir_all(vers).unwrap();
    for e in std::fs::read_dir(de).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            copier(&p, &vers.join(e.file_name()));
        } else {
            std::fs::copy(&p, vers.join(e.file_name())).unwrap();
        }
    }
}

/// La config et le journal de la fictive `nom`, sans engagement, dans `racine`.
pub fn config_et_journal(nom: &str, racine: &Path) -> PathBuf {
    let d = fictive(nom);
    for sous in ["config", "journal"] {
        if d.join(sous).is_dir() {
            copier(&d.join(sous), &racine.join(sous));
        }
    }
    racine.to_path_buf()
}

pub fn lire(racine: &Path, rel: &str) -> Value {
    serde_json::from_slice(&std::fs::read(racine.join(rel)).unwrap()).unwrap_or_else(|e| panic!("{} illisible : {}", rel, e))
}

pub fn ecrire(racine: &Path, rel: &str, v: &Value) {
    let p = racine.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, serde_json::to_vec_pretty(v).unwrap()).unwrap();
}

/// Ce qui est parti, tel que le test l'ecrit dans `departs.json` de la compagnie : `[{"salarie", "numero", "envoi",
/// "mois", "version", "le"}]`. Les tests de l'engagement posent ce lecteur a la place de celui de la couche des envois.
pub fn departs_du_test(racine: &Path) -> Result<engagement::departs::Departs, String> {
    use engagement::departs::{Depart, Departs, Envoi};
    let mut d = Departs::default();
    let Ok(o) = std::fs::read(racine.join("departs.json")) else { return Ok(d) };
    let lignes: Value = serde_json::from_slice(&o).map_err(|e| e.to_string())?;
    for l in lignes.as_array().into_iter().flatten() {
        let t = |k: &str| l[k].as_str().unwrap_or_default().to_string();
        let envoi = match l["envoi"].as_str() {
            Some("contrat") => Envoi::Contrat,
            Some("dpae") => Envoi::Dpae,
            Some("documents") => Envoi::Documents,
            _ => Envoi::Dsn,
        };
        d.noter(&t("salarie"), &t("numero"), Depart { envoi, mois: l["mois"].as_str().map(String::from), version: l["version"].as_u64().unwrap_or(1), le: t("le") });
    }
    Ok(d)
}
