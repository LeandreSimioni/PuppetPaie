//! La fabrique de PDF, dans le meme processus : une description -> un fichier PDF.
//!
//! Le dossier parent est cree, un refus de
//! la fabrique est une `Err` portant sa ligne de francais telle quelle.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use socle::echec::Refus;

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

/// Le PDF de `description` ecrit a `chemin` ; un refus de la fabrique, sur une ligne, sous la classe `FabriqueRefus`.
pub fn ecrire_pdf(description: &Value, chemin: &Path) -> socle::echec::R<()> {
    ecrire(description, chemin).map(|_| ()).map_err(|m| socle::echec::erreur("FabriqueRefus", m.replace('\n', " ").trim().to_string()))
}

/// La description, avec la mention de signature de l'artiste dans la case de chaque
/// document (`modele.mention_artiste`). `Err` si la description n'a pas de `documents` ou un document pas de `modele`
/// (refus sous la forme `'cle'`).
pub fn avec_mention_artiste(description: &Value, mention: &str) -> Result<Value, String> {
    let mut nouvelle = description.clone();
    let documents = nouvelle.get_mut("documents").and_then(Value::as_array_mut).ok_or_else(|| socle::fichiers::absent("documents"))?;
    for doc in documents.iter_mut() {
        let modele = doc.get_mut("modele").and_then(Value::as_object_mut).ok_or_else(|| socle::fichiers::absent("modele"))?;
        modele.insert("mention_artiste".into(), Value::String(mention.to_string()));
    }
    Ok(nouvelle)
}

/// Les octets du PDF de `description`, sans fichier (`contrat._regenerer`).
pub fn fabriquer_octets(description: &Value) -> Result<Vec<u8>, String> {
    let texte = serde_json::to_string(description).map_err(|e| e.to_string())?;
    fabrique_pdf::fabriquer(&texte).map_err(|r| r.0.replace('\n', " "))
}

/// Le PDF verrouille en AES-256 (mot de passe utilisateur vide, impression et copie seules).
pub fn verrouiller(octets: &[u8]) -> Result<Vec<u8>, String> {
    fabrique_pdf::chiffrement::verrouiller_avec(octets, socle::temps::aleas).map_err(|r| r.0.replace('\n', " "))
}

// ------------------------------------------------------------------ l'assemblage

/// Des pieces (un PDF, ses pages 0-based ou toutes) fusionnees par la fabrique ;
/// un refus est une `ErreurOperation`, aucun PDF n'est assemble autrement.
pub fn fusion_pieces(pieces: &[(PathBuf, Option<Vec<usize>>)]) -> Result<Vec<u8>, Refus> {
    let liste: Vec<Value> = pieces
        .iter()
        .map(|(f, pages)| match pages {
            Some(p) => json!({"fichier": f.to_string_lossy(), "pages": p.iter().map(|n| n + 1).collect::<Vec<_>>()}),
            None => json!({"fichier": f.to_string_lossy()}),
        })
        .collect();
    let description = json!({"format": 1, "pieces": liste});
    fabrique_pdf::assemblage::assembler(&description.to_string()).map_err(|r| {
        avec("ErreurOperation", format!(
            "Assemblage impossible : la fabrique de PDF refuse la fusion ({}). Aucun PDF n'est assemblé autrement.",
            r.0.replace('\n', " ").trim()
        ))
    })
}

/// `_fusion_fabrique` de PDF en memoire : chacun va dans un fichier de `tmp` le temps de la fusion.
pub fn fusion_octets(morceaux: &[Vec<u8>], tmp: &Path) -> Result<Vec<u8>, Refus> {
    let dossier = Temporaire::nouveau_dans(tmp, "assemblage")?;
    let mut pieces = Vec::new();
    for (i, octets) in morceaux.iter().enumerate() {
        let f = dossier.0.join(format!("piece{}.pdf", i));
        std::fs::write(&f, octets).map_err(|e| avec("OSError", format!("{} : {}", f.display(), e)))?;
        pieces.push((f, None));
    }
    fusion_pieces(&pieces)
}

/// Un dossier temporaire, retire quand on a fini (`tempfile.mkdtemp` puis `shutil.rmtree`).
pub struct Temporaire(pub PathBuf);

impl Temporaire {
    pub fn nouveau_dans(parent: &Path, prefixe: &str) -> Result<Temporaire, Refus> {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let p = parent.join(format!("{}-{}-{}", prefixe, std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)));
        std::fs::create_dir_all(&p).map_err(|e| avec("OSError", format!("{} : {}", p.display(), e)))?;
        Ok(Temporaire(p))
    }
}

impl Drop for Temporaire {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn avec(classe: &'static str, message: impl Into<String>) -> Refus {
    Refus { classe: Some(classe), message: message.into() }
}

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
