//! La fabrique de PDF, dans le meme processus : une description -> un fichier PDF.
//!
//! Le dossier parent est cree, un refus de
//! la fabrique est une `Err` portant sa ligne de francais telle quelle.

use std::path::{Path, PathBuf};

use fabrique_pdf::assemblage::{Assemblage, Piece};
use fabrique_pdf::Description;
use socle::echec::Refus;

use socle::num::Resultat;

/// Ecrit le PDF de `description` dans `chemin` et rend le chemin.
pub fn ecrire(description: &Description, chemin: &Path) -> Resultat<PathBuf> {
    if let Some(parent) = chemin.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| socle::echec::du_disque(chemin, &e))?;
    }
    let octets = fabriquer_octets(description)?;
    if let Err(e) = std::fs::write(chemin, &octets) {
        let _ = std::fs::remove_file(chemin);
        return Err(socle::echec::du_disque(chemin, &e));
    }
    Ok(chemin.to_path_buf())
}

/// Le PDF de `description` ecrit a `chemin` ; un refus de la fabrique, sur une ligne, sous la classe `FabriqueRefus`.
pub fn ecrire_pdf(description: &Description, chemin: &Path) -> socle::echec::R<()> {
    ecrire(description, chemin).map(|_| ()).map_err(|m| socle::echec::erreur("FabriqueRefus", m.replace('\n', " ").trim().to_string()))
}

/// Les octets du PDF de `description`, sans fichier.
pub fn fabriquer_octets(description: &Description) -> Result<Vec<u8>, String> {
    fabrique_pdf::fabriquer(description).map_err(|r| r.0.replace('\n', " "))
}

/// Le PDF verrouille en AES-256 (mot de passe utilisateur vide, impression et copie seules).
pub fn verrouiller(octets: &[u8]) -> Result<Vec<u8>, String> {
    fabrique_pdf::chiffrement::verrouiller_avec(octets, socle::temps::aleas).map_err(|r| r.0.replace('\n', " "))
}

// ------------------------------------------------------------------ l'assemblage

/// Des pieces (un PDF, ses pages 0-based ou toutes) fusionnees par la fabrique ;
/// un refus est une `ErreurOperation`, aucun PDF n'est assemble autrement.
pub fn fusion_pieces(pieces: &[(PathBuf, Option<Vec<usize>>)]) -> Result<Vec<u8>, Refus> {
    let assemblage = Assemblage {
        format: Some(1),
        pieces: pieces
            .iter()
            .map(|(f, pages)| Piece {
                fichier: f.to_string_lossy().into_owned(),
                pages: pages.as_ref().map(|p| p.iter().map(|n| *n as u32 + 1).collect()),
            })
            .collect(),
    };
    let refus = |m: &str| {
        avec("ErreurOperation", format!(
            "Assemblage impossible : la fabrique de PDF refuse la fusion ({}). Aucun PDF n'est assemblé autrement.",
            m.replace('\n', " ").trim()
        ))
    };
    match fabrique_pdf::assemblage::assembler(&assemblage).map_err(|r| refus(&r.0))? {
        // Une piece verrouillee (un PDF venu d'ailleurs, tel qu'il a ete depose) garde son verrou dans ce qui la porte.
        (octets, true) => verrouiller(&octets).map_err(|m| refus(&m)),
        (octets, false) => Ok(octets),
    }
}

/// La fusion de PDF en memoire : chacun va dans un fichier de `tmp` le temps de la fusion.
pub fn fusion_octets(morceaux: &[Vec<u8>], tmp: &Path) -> Result<Vec<u8>, Refus> {
    let dossier = Temporaire::nouveau_dans(tmp, "assemblage")?;
    let mut pieces = Vec::new();
    for (i, octets) in morceaux.iter().enumerate() {
        let f = dossier.0.join(format!("piece{}.pdf", i));
        std::fs::write(&f, octets).map_err(|e| avec("OSError", socle::echec::du_disque(&f, &e)))?;
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
        std::fs::create_dir_all(&p).map_err(|e| avec("OSError", socle::echec::du_disque(&p, &e)))?;
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
