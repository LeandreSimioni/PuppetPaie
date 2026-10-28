//! Un faux Drive en memoire (le trait `drive::Drive`), pour `fichiers/sauvegarde.rs`, `fichiers/archivage.rs`, `ouverture.rs` et
//! `routes/ouverture.rs`.
//!
//! AUCUN RESEAU : il ne tient que des dossiers et des fichiers en memoire, et un journal des appels. Il ne PEUT pas
//! supprimer (le trait n'en a pas). Comme le vrai client : `lister` rend les FICHIERS d'un dossier par nom (le dernier
//! de deux homonymes gagne), jamais les sous-dossiers ; `televerser` remplace le fichier `connu` ou en cree un nouveau
//! (un homonyme reste, rien n'est cherche par nom). Pour `fichiers/restauration.rs` : `sous_dossiers` rend les dossiers
//! d'un dossier (`root` compris), homonymes compris, sans rien creer ; `telecharger` rend le contenu tel qu'il est tenu.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use envois::drive::{Depose, Distant, Drive, ErreurGoogle};
use moteur::sauvegarde::empreinte;

pub struct FichierDrive {
    pub id: String,
    pub parent: String,
    pub nom: String,
    pub md5: Option<String>,
    pub contenu: Vec<u8>,
}

/// Une panne a la demande : `operation` (`chemin`, `lister`, `televerser`), un `motif` que doit contenir la cle
/// (le chemin du dossier, ou celui du fichier), et l'erreur rendue.
pub struct Panne {
    pub operation: &'static str,
    pub motif: String,
    pub erreur: ErreurGoogle,
}

#[derive(Default)]
pub struct FauxDrive {
    pub compte: Option<String>,
    /// id -> (parent, chemin complet depuis la racine)
    dossiers: Vec<(String, String, String)>,
    pub fichiers: Vec<FichierDrive>,
    /// Chaque appel, en une ligne : `chemin a/b`, `lister a/b`, `televerser a/b/nom connu=ID|-`.
    pub journal: Vec<String>,
    suivant: u32,
    /// Un Drive qui ne donne pas d'empreinte (un fichier Google Docs).
    pub sans_md5: bool,
    pub pannes: Vec<Panne>,
    /// Fichiers locaux a effacer quand `televerser` du nom indique est appele, AVANT qu'il lise le contenu.
    pub effacer_au_televerser: Vec<(String, PathBuf)>,
    /// Fichiers locaux a effacer quand `lister` d'un dossier dont le chemin Drive contient le motif est appele.
    pub effacer_au_lister: Vec<(String, PathBuf)>,
}

impl FauxDrive {
    pub fn nouveau() -> FauxDrive {
        FauxDrive { compte: Some("compte@exemple.invalid".into()), ..Default::default() }
    }

    fn prochain(&mut self, lettre: &str) -> String {
        self.suivant += 1;
        format!("{}{}", lettre, self.suivant)
    }

    pub fn id_du_dossier(&self, chemin: &str) -> String {
        self.dossiers.iter().find(|(_, _, c)| c == chemin).map(|(i, _, _)| i.clone()).unwrap_or_else(|| panic!("pas de dossier {}", chemin))
    }

    fn chemin_de(&self, id: &str) -> String {
        self.dossiers.iter().find(|(i, _, _)| i == id).map(|(_, _, c)| c.clone()).unwrap_or_else(|| panic!("dossier inconnu : {}", id))
    }

    /// Un fichier deja la (hors de toute sauvegarde) : dans le dossier `chemin` (cree au besoin).
    pub fn deposer(&mut self, chemin: &str, nom: &str, contenu: &[u8], avec_md5: bool) -> String {
        let segments: Vec<&str> = chemin.split('/').collect();
        let parent = self.chemin(&segments).unwrap();
        let id = self.prochain("F");
        let md5 = avec_md5.then(|| md5_de(contenu));
        self.fichiers.push(FichierDrive { id: id.clone(), parent, nom: nom.into(), md5, contenu: contenu.to_vec() });
        id
    }

    /// Le contenu de tous les fichiers de ce chemin Drive (`a/b/nom`) : plusieurs si homonymes.
    pub fn contenus(&self, chemin_fichier: &str) -> Vec<String> {
        self.fichiers
            .iter()
            .filter(|f| format!("{}/{}", self.chemin_de(&f.parent), f.nom) == chemin_fichier)
            .map(|f| String::from_utf8_lossy(&f.contenu).into_owned())
            .collect()
    }

    /// Tous les fichiers : `chemin/nom` -> contenus, triee.
    pub fn arbre(&self) -> BTreeMap<String, Vec<String>> {
        let mut a: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for f in &self.fichiers {
            a.entry(format!("{}/{}", self.chemin_de(&f.parent), f.nom)).or_default().push(String::from_utf8_lossy(&f.contenu).into_owned());
        }
        a
    }

    /// Un second dossier de meme chemin que `chemin` (qui doit exister) : ce que deux postes qui sauvegardent en meme
    /// temps peuvent laisser dans le vrai Drive.
    pub fn dossier_homonyme(&mut self, chemin: &str) -> String {
        let parent = self.dossiers.iter().find(|(_, _, c)| c == chemin).map(|(_, p, _)| p.clone()).unwrap_or_else(|| panic!("pas de dossier {}", chemin));
        let id = self.prochain("D");
        self.dossiers.push((id.clone(), parent, chemin.to_string()));
        id
    }

    /// Le fichier de ce chemin Drive (`a/b/nom`), a modifier (le premier si homonymes).
    pub fn fichier_mut(&mut self, chemin_fichier: &str) -> &mut FichierDrive {
        let i = (0..self.fichiers.len())
            .find(|&i| format!("{}/{}", self.chemin_de(&self.fichiers[i].parent), self.fichiers[i].nom) == chemin_fichier)
            .unwrap_or_else(|| panic!("pas de fichier {}", chemin_fichier));
        &mut self.fichiers[i]
    }

    pub fn nombre_de_dossiers(&self) -> usize {
        self.dossiers.len()
    }

    /// Le nombre d'appels dont le journal commence par `debut`.
    pub fn appels(&self, debut: &str) -> usize {
        self.journal.iter().filter(|l| l.starts_with(debut)).count()
    }

    fn panne(&self, operation: &str, cle: &str) -> Option<ErreurGoogle> {
        self.pannes.iter().find(|p| p.operation == operation && cle.contains(&p.motif)).map(|p| p.erreur.clone())
    }
}

pub fn md5_de(octets: &[u8]) -> String {
    use md5::{Digest, Md5};
    Md5::digest(octets).iter().map(|b| format!("{:02x}", b)).collect()
}

impl Drive for FauxDrive {
    fn compte(&self) -> Option<String> {
        self.compte.clone()
    }

    fn chemin(&mut self, noms: &[&str]) -> Result<String, ErreurGoogle> {
        let cle = noms.join("/");
        self.journal.push(format!("chemin {}", cle));
        if let Some(e) = self.panne("chemin", &cle) {
            return Err(e);
        }
        let mut parent = "root".to_string();
        let mut complet = String::new();
        for nom in noms {
            if !complet.is_empty() {
                complet.push('/');
            }
            complet.push_str(nom);
            parent = match self.dossiers.iter().find(|(_, p, c)| *p == parent && *c == complet) {
                Some((i, _, _)) => i.clone(),
                None => {
                    let id = self.prochain("D");
                    self.dossiers.push((id.clone(), parent.clone(), complet.clone()));
                    id
                }
            };
        }
        Ok(parent)
    }

    fn lister(&mut self, parent: &str) -> Result<BTreeMap<String, Distant>, ErreurGoogle> {
        let chemin = self.chemin_de(parent);
        self.journal.push(format!("lister {}", chemin));
        if let Some(e) = self.panne("lister", &chemin) {
            return Err(e);
        }
        for (motif, p) in &self.effacer_au_lister {
            if chemin.contains(motif.as_str()) {
                let _ = std::fs::remove_file(p);
            }
        }
        let mut l = BTreeMap::new();
        for f in self.fichiers.iter().filter(|f| f.parent == parent) {
            l.insert(f.nom.clone(), Distant { id: f.id.clone(), md5: f.md5.clone(), octets: f.contenu.len() as u64 });
        }
        Ok(l)
    }

    fn televerser(&mut self, fichier: &Path, parent: &str, nom: Option<&str>, connu: Option<&Distant>) -> Result<Depose, ErreurGoogle> {
        let nom = nom.map(str::to_string).unwrap_or_else(|| fichier.file_name().unwrap().to_string_lossy().into_owned());
        let chemin = format!("{}/{}", self.chemin_de(parent), nom);
        self.journal.push(format!("televerser {} connu={}", chemin, connu.map(|c| c.id.as_str()).unwrap_or("-")));
        for (n, p) in &self.effacer_au_televerser {
            if *n == nom {
                let _ = std::fs::remove_file(p);
            }
        }
        if let Some(e) = self.panne("televerser", &chemin) {
            return Err(e);
        }
        let contenu = std::fs::read(fichier).map_err(|e| ErreurGoogle::fichier(&e, fichier))?;
        let md5 = if self.sans_md5 { None } else { Some(empreinte(fichier).unwrap()) };
        if let Some(c) = connu {
            let f = self.fichiers.iter_mut().find(|f| f.id == c.id).expect("`connu` doit exister dans le Drive");
            assert_eq!(f.parent, parent, "on ne remplace qu'un fichier du dossier");
            f.contenu = contenu;
            f.md5 = md5;
            return Ok(Depose { id: Some(f.id.clone()), nom: Some(nom), lien: Some(format!("lien/{}", f.id)), remplace: true });
        }
        let id = self.prochain("F");
        self.fichiers.push(FichierDrive { id: id.clone(), parent: parent.into(), nom: nom.clone(), md5, contenu });
        Ok(Depose { id: Some(id.clone()), nom: Some(nom), lien: Some(format!("lien/{}", id)), remplace: false })
    }

    fn sous_dossiers(&mut self, parent: &str) -> Result<Vec<(String, String)>, ErreurGoogle> {
        let chemin = if parent == "root" { "root".to_string() } else { self.chemin_de(parent) };
        self.journal.push(format!("sous_dossiers {}", chemin));
        if let Some(e) = self.panne("sous_dossiers", &chemin) {
            return Err(e);
        }
        Ok(self
            .dossiers
            .iter()
            .filter(|(_, p, _)| p == parent)
            .map(|(i, _, c)| (c.rsplit('/').next().unwrap_or(c).to_string(), i.clone()))
            .collect())
    }

    fn telecharger(&mut self, id: &str) -> Result<Vec<u8>, ErreurGoogle> {
        let f = self.fichiers.iter().find(|f| f.id == id).unwrap_or_else(|| panic!("fichier inconnu : {}", id));
        let chemin = format!("{}/{}", self.chemin_de(&f.parent), f.nom);
        let contenu = f.contenu.clone();
        self.journal.push(format!("telecharger {}", chemin));
        if let Some(e) = self.panne("telecharger", &chemin) {
            return Err(e);
        }
        Ok(contenu)
    }
}

// ------------------------------------------------------------ le disque local

pub fn racine(prefixe: &str, nom: &str) -> PathBuf {
    let r = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{}-{}", prefixe, nom));
    let _ = std::fs::remove_dir_all(&r);
    std::fs::create_dir_all(&r).unwrap();
    r
}

pub fn ecrire(racine: &Path, relatif: &str, contenu: &str) {
    let p = racine.join(relatif);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, contenu).unwrap();
    fixer_date(&p, 1_700_000_000, 0);
}

/// La date de modification d'un fichier : `secondes` et `nanos` apres 1970.
pub fn fixer_date(p: &Path, secondes: u64, nanos: u32) {
    let f = std::fs::OpenOptions::new().write(true).open(p).unwrap();
    f.set_modified(std::time::UNIX_EPOCH + std::time::Duration::new(secondes, nanos)).unwrap();
}
