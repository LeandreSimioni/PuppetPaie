//! La fabrique de PDF, dans le meme processus : une description -> un fichier PDF.
//!
//! Meme contrat que `moteur/fabrique.py::ecrire` : le dossier parent est cree, un refus de
//! la fabrique est une `Err` portant sa ligne de francais telle quelle.

use std::path::{Path, PathBuf};

use serde_json::Value;

use socle::num::Resultat;

/// Ecrit le PDF de `description` dans `chemin` et rend le chemin.
pub fn ecrire(description: &Value, chemin: &Path) -> Resultat<PathBuf> {
    if let Some(parent) = chemin.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("ecriture impossible de {} : {}", chemin.display(), e))?;
    }
    let texte = serde_json::to_string(description).map_err(|e| e.to_string())?;
    let octets = fabrique_pdf::fabriquer(&texte).map_err(|r| r.0.replace('\n', " "))?;
    if let Err(e) = std::fs::write(chemin, &octets) {
        let _ = std::fs::remove_file(chemin);
        return Err(format!("ecriture impossible de {} : {}", chemin.display(), e));
    }
    Ok(chemin.to_path_buf())
}

/// `fabrique.chemin_description` : `X.pdf` -> `X.description.json`.
pub fn chemin_description(pdf: &Path) -> PathBuf {
    let souche = pdf.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    pdf.with_file_name(format!("{}.description.json", souche))
}

/// `fabrique.garder_description` : ecrit la description a cote du PDF (qui existe deja), avec l'empreinte de ce
/// PDF : on ne refait un contrat que si le fichier parti est bien celui que cette description a produit.
/// `json.dumps(..., ensure_ascii=False, indent=1)`, sans saut de ligne final.
pub fn garder_description(description: &Value, pdf: &Path) -> Resultat<PathBuf> {
    let octets = std::fs::read(pdf).map_err(|e| format!("{} : {}", pdf.display(), e))?;
    let empreinte = empreinte_sha256(&octets);
    let f = chemin_description(pdf);
    let contenu = serde_json::json!({"format": FORMAT_GARDE, "pdf_sha256": empreinte, "description": description});
    std::fs::write(&f, socle::ecriture::json_py(&contenu, 1)).map_err(|e| format!("{} : {}", f.display(), e))?;
    Ok(f)
}

/// `hashlib.sha256(...).hexdigest()`.
fn empreinte_sha256(octets: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(octets).iter().map(|o| format!("{:02x}", o)).collect()
}

/// `fabrique.description_gardee` : la description gardee de `pdf`, ou `None` -- absente, illisible, d'un autre format, ou
/// ne decrivant pas ce fichier (empreinte differente). On ne devine rien : `None` veut dire « renvoyez le contrat ».
pub fn description_gardee(pdf: &Path) -> Option<Value> {
    let brut: Value = serde_json::from_slice(&std::fs::read(chemin_description(pdf)).ok()?).ok()?;
    let format = brut.get("format")?.as_f64()?;
    let empreinte = brut.get("pdf_sha256")?.as_str()?;
    let description = brut.get("description")?;
    if format != FORMAT_GARDE as f64 || !description.is_object() || empreinte != empreinte_sha256(&std::fs::read(pdf).ok()?) {
        return None;
    }
    Some(description.clone())
}

/// `fabrique.avec_mention_artiste` : la description, avec la mention de signature de l'artiste dans la case de chaque
/// document (`modele.mention_artiste`). `Err` si la description n'a pas de `documents` ou un document pas de `modele`
/// (`KeyError` du Python).
pub fn avec_mention_artiste(description: &Value, mention: &str) -> Result<Value, String> {
    let mut nouvelle = description.clone();
    let documents = nouvelle.get_mut("documents").and_then(Value::as_array_mut).ok_or_else(|| socle::fichiers::absent("documents"))?;
    for doc in documents.iter_mut() {
        let modele = doc.get_mut("modele").and_then(Value::as_object_mut).ok_or_else(|| socle::fichiers::absent("modele"))?;
        modele.insert("mention_artiste".into(), Value::String(mention.to_string()));
    }
    Ok(nouvelle)
}

/// Les octets du PDF de `description`, sans fichier (`contrat._regenerer` : le Python passe par un dossier temporaire).
pub fn fabriquer_octets(description: &Value) -> Result<Vec<u8>, String> {
    let texte = serde_json::to_string(description).map_err(|e| e.to_string())?;
    fabrique_pdf::fabriquer(&texte).map_err(|r| r.0.replace('\n', " "))
}

/// `fabrique.verrouiller` : le PDF verrouille en AES-256 (mot de passe utilisateur vide, impression et copie seules).
pub fn verrouiller(octets: &[u8]) -> Result<Vec<u8>, String> {
    fabrique_pdf::chiffrement::verrouiller(octets).map_err(|r| r.0.replace('\n', " "))
}

/// `fabrique.FORMAT`.
const FORMAT_GARDE: i64 = 3;

#[cfg(test)]
mod tests {
    use super::ecrire;
    use serde_json::json;

    fn dossier(nom: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("paie-fabrique-{}-{}", nom, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn une_description_devient_un_pdf_lisible_et_le_dossier_parent_se_cree() {
        let d = dossier("bulletin");
        let description = socle::fichiers::lire_json(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/donnees/fabrique-bulletin.json")).unwrap();
        let sortie = d.join("sous/dossier/bulletin.pdf");
        assert_eq!(ecrire(&description, &sortie).unwrap(), sortie);
        assert!(std::fs::read(&sortie).unwrap().starts_with(b"%PDF"));
        let f = sortie.to_str().unwrap().to_string();
        assert_eq!(fabrique_pdf::lecture::pages(&[f.clone()]).unwrap(), vec![1]);
        let (textes, _) = fabrique_pdf::lecture::texte(&f).unwrap();
        assert!(textes[0].contains("Bulletin de paie") && textes[0].contains("BEAUCHAMP"), "{}", textes[0]);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn un_refus_de_la_fabrique_est_une_erreur_d_une_ligne_et_n_ecrit_rien() {
        let sortie = dossier("refus").join("refus.pdf");
        let e = ecrire(&json!({"format": 99}), &sortie).unwrap_err();
        assert!(!e.is_empty() && !e.contains('\n'), "{e}");
        assert!(!sortie.exists());
    }
}
