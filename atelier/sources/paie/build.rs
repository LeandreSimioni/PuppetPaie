//! Identifie les sources du binaire : `PAIE_SOURCES` (FNV-1a 64 bits sur les
//! fichiers de `src/` et `Cargo.lock`, dans l'ordre des noms). Le calcul
//! stocke a cote de la saisie (`calcul_du_mois`) porte cette marque : un
//! autre moteur ne reprend pas un resultat qu'il n'a pas calcule.

use std::path::{Path, PathBuf};

fn fichiers(dossier: &Path, out: &mut Vec<PathBuf>) {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dossier).into_iter().flatten().flatten().map(|e| e.path()).collect();
    v.sort();
    for p in v {
        if p.is_dir() {
            fichiers(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn main() {
    let mut liste = Vec::new();
    fichiers(Path::new("src"), &mut liste);
    liste.push(PathBuf::from("Cargo.lock"));
    let mut h: u64 = 0xcbf29ce484222325;
    for p in &liste {
        let nom = p.to_string_lossy().replace('\\', "/");
        let contenu = std::fs::read(p).unwrap_or_default();
        for o in nom.bytes().chain([0]).chain(contenu) {
            h = (h ^ o as u64).wrapping_mul(0x100000001b3);
        }
    }
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=Cargo.lock");
    println!("cargo:rustc-env=PAIE_SOURCES={:016x}", h);
}
