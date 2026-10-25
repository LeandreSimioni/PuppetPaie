//! Le verrou des compteurs : le fichier `config/.compteurs.verrou` qui exclut deux attributions de numeros
//! simultanees ; son contenu dit qui le tient (`QUI pid N JJ/MM/AAAA HH:MM:SS`).
//!
//! Un verrou est un fichier cree en exclusif ; passe `delai` secondes le
//! detenteur est tenu pour mort et on reprend la main ; le sortant ne retire
//! que SA marque.

use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::num::Resultat;

/// Au-dela, un verrou est tenu pour abandonne (10 minutes).
pub const DELAI_VERROU: i64 = 600;

/// Le verrou est tenu par un autre : sa marque et son age en secondes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verrouille {
    pub chemin: PathBuf,
    pub detenteur: String,
    pub age: i64,
}

impl Verrouille {
    /// La phrase de `CompteursVerrouilles`.
    pub fn message(&self) -> String {
        format!(
            "Une autre production tient les compteurs depuis {} s ({}).\nAttendre qu'elle finisse. Si elle a ete interrompue, effacer :\n  {}",
            self.age,
            self.detenteur,
            self.chemin.display()
        )
    }
}

/// `config/.compteurs.verrou` d'une racine de donnees.
pub fn chemin_compteurs(racine: &Path) -> PathBuf {
    racine.join("config").join(".compteurs.verrou")
}

/// La marque que le detenteur ecrit : `QUI pid N JJ/MM/AAAA HH:MM:SS`.
pub fn marque(qui: &str, pid: u32, maintenant: &str) -> String {
    format!("{} pid {} {}", qui, pid, maintenant)
}

fn age_secondes(chemin: &Path) -> std::io::Result<i64> {
    let modifie = std::fs::metadata(chemin)?.modified()?;
    let en_s = |t: SystemTime| match t.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs_f64(),
        Err(e) => -e.duration().as_secs_f64(),
    };
    Ok((en_s(SystemTime::now()) - en_s(modifie)).trunc() as i64)
}

/// Prend le verrou, ou rend qui le tient. Un verrou plus vieux que `delai`
/// est leve, une fois, puis on retente. Une erreur d'ecriture (dossier
/// absent...) est une erreur, pas un verrou tenu.
pub fn prendre(chemin: &Path, marque: &str, delai: i64) -> Resultat<Result<(), Verrouille>> {
    for essai in [1, 2] {
        match OpenOptions::new().write(true).create_new(true).open(chemin) {
            Ok(mut f) => {
                f.write_all(marque.as_bytes()).map_err(|e| format!("{} : {}", chemin.display(), e))?;
                return Ok(Ok(()));
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                // Disparu entre-temps : on retente.
                let (age, detenteur) = match (
                    age_secondes(chemin),
                    std::fs::read(chemin).map(|b| String::from_utf8_lossy(&b).trim().to_string()),
                ) {
                    (Ok(a), Ok(d)) => (a, d),
                    _ => continue,
                };
                if age < delai || essai == 2 {
                    return Ok(Err(Verrouille { chemin: chemin.to_path_buf(), detenteur, age }));
                }
                let _ = std::fs::remove_file(chemin);
            }
            Err(e) => return Err(format!("{} : {}", chemin.display(), e)),
        }
    }
    Ok(Ok(()))
}

/// Retire le verrou s'il porte encore NOTRE marque : si le delai a joue, un
/// autre l'a peut-etre repris entre-temps.
pub fn rendre(chemin: &Path, marque: &str) {
    if let Ok(b) = std::fs::read(chemin) {
        if String::from_utf8_lossy(&b).trim() == marque {
            let _ = std::fs::remove_file(chemin);
        }
    }
}

/// Un verrou tenu : rendu a la sortie de la portee.
pub struct Verrou {
    chemin: PathBuf,
    marque: String,
}

impl Verrou {
    pub fn prendre(chemin: &Path, marque: &str, delai: i64) -> Resultat<Result<Verrou, Verrouille>> {
        Ok(prendre(chemin, marque, delai)?.map(|_| Verrou { chemin: chemin.to_path_buf(), marque: marque.to_string() }))
    }
}

impl Drop for Verrou {
    fn drop(&mut self) {
        rendre(&self.chemin, &self.marque);
    }
}

