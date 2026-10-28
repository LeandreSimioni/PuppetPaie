//! L'archivage d'un mois dans le Drive (`operations_documents.archiver`).
//!
//! Ecrit au lot H4b, non prouve. Les sorties d'un mois (`sorties/<mois>/`, sans le sous-dossier
//! `envoi/` : memes documents redecoupes par salarie, deja partis) vont dans
//! `PuppetPaie/<mois>` du Drive du compte autorise. Le fichier groupe y a toute sa place :
//! c'est l'archive de la compagnie, pas le courrier d'un salarie.
//!
//! Relancer `archiver` REMPLACE le contenu des fichiers de meme nom (`Depose::remplace`), au
//! lieu d'empiler des copies : on reproduit un mois quand un montant arrive ou qu'un bareme change.
//!
//! Le client Drive se passe en parametre (`drive::Drive`), comme pour la sauvegarde.

use std::path::Path;

use envois::drive::{Client, Drive, ErreurGoogle};

/// `FichierArchive`.
#[derive(Debug, Clone, PartialEq)]
pub struct FichierArchive {
    pub nom: Option<String>,
    pub octets: u64,
    pub remplace: bool,
}

/// `Archivage`.
#[derive(Debug, Clone, PartialEq)]
pub struct Archivage {
    pub mois: String,
    pub racine_drive: String,
    pub compte: Option<String>,
    pub fichiers: Vec<FichierArchive>,
}

/// `SortieAbsente` : on demande a archiver un mois qui n'a pas ete produit.
fn sortie_absente(dossier: &Path) -> ErreurGoogle {
    ErreurGoogle::nouvelle("SortieAbsente", format!("{} n'existe pas.", dossier.display()))
}

/// DEFAUT 7 DU PYTHON, CORRIGE : `archiver` ne controlait pas `mois`, et `sorties/<mois>` se lisait tel quel : `..` ou `../config`
/// archivait le dossier `config` (coffre et jeton compris), `2026-06/envoi` archivait les fiches de salaries une a une.
/// Seul `AAAA-MM` (quatre chiffres, un tiret, un mois de 01 a 12) est un mois.
pub fn mois_valide(mois: &str) -> bool {
    let b = mois.as_bytes();
    b.len() == 7
        && b[4] == b'-'
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[5..].iter().all(u8::is_ascii_digit)
        && (1..=12).contains(&mois[5..].parse::<u32>().unwrap_or(0))
}

/// `archiver` : depose les sorties d'un mois dans le Drive du compte autorise.
///
/// `client` : `None` pour le vrai (`Client::nouveau`, construit APRES les controles du dossier,
/// comme le Python : un mois absent se dit avant tout refus du client).
pub fn archiver(racine: &Path, mois: &str, racine_drive: &str, client: Option<&mut dyn Drive>) -> Result<Archivage, ErreurGoogle> {
    if !mois_valide(mois) {
        return Err(ErreurGoogle::nouvelle("ErreurOperation", format!("Mois illisible : '{}' (AAAA-MM attendu).", mois)));
    }
    let dossier = racine.join("sorties").join(mois);
    if !dossier.exists() {
        return Err(sortie_absente(&dossier));
    }
    let mut fichiers: Vec<std::path::PathBuf> = std::fs::read_dir(&dossier)
        .map_err(|e| ErreurGoogle::fichier(&e, &dossier))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    fichiers.sort();
    if fichiers.is_empty() {
        return Err(sortie_absente(&dossier));
    }

    let mut propre: Client;
    let client: &mut dyn Drive = match client {
        Some(c) => c,
        None => {
            propre = crate::routes_google::client_drive(racine)?;
            &mut propre
        }
    };
    let cible = client.chemin(&[racine_drive, mois])?;
    // Un seul listage du dossier du mois : c'est lui qui dit quels fichiers se remplacent (defauts 3 et 4 du Python :
    // une recherche par fichier, par nom seul, dossiers compris).
    let deja = client.lister(&cible)?;
    let mut deposes = Vec::new();
    for f in &fichiers {
        let nom_local = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let r = client.televerser(f, &cible, None, deja.get(&nom_local))?;
        let octets = std::fs::metadata(f).map_err(|e| ErreurGoogle::fichier(&e, f))?.len();
        let nom = r.nom.or_else(|| f.file_name().map(|n| n.to_string_lossy().into_owned()));
        deposes.push(FichierArchive { nom, octets, remplace: r.remplace });
    }
    Ok(Archivage { mois: mois.to_string(), racine_drive: racine_drive.to_string(), compte: client.compte(), fichiers: deposes })
}
