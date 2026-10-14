//! La restauration du dossier prive depuis le Drive (`paie restaurer`) : le retour de `sauvegarde.rs` (lot R1, 13/10/2026).
//!
//! Le plan (`atelier/PLAN-DEPOT-PUBLIC.md` § 4, premier trou) : « aucune commande ne redescend ». Le jour ou le depot Git
//! ne porte plus les donnees, la sauvegarde Drive est la seule copie de `prive/` ; sans retour, ce n'est pas une copie.
//!
//! CE QU'ELLE FAIT, dans cet ordre, et rien n'est ecrit avant la troisieme etape :
//! 1. elle retrouve `<racine_drive>/sauvegarde` dans le Drive du compte relie, SANS RIEN Y CREER (`Drive::sous_dossiers`,
//!    pas `Drive::chemin`), et en fait l'inventaire complet : chaque fichier, son empreinte (`md5Checksum`), son dossier ;
//! 2. elle compare le dossier local a cet inventaire (voir « un dossier qui n'est pas vide ») ;
//! 3. elle redescend ce qui manque, fichier par fichier : le contenu recu est compare a l'empreinte que Drive publie AVANT
//!    d'etre ecrit (un `.part`, renomme ensuite : un fichier coupe en route n'a jamais le nom du vrai) ;
//! 4. elle reconstruit l'index local (`config/.sauvegarde.json`) : la sauvegarde suivante connait les dossiers Drive sans
//!    les rechercher, et `rien_n_a_bouge` dit oui tant que rien n'a change.
//!
//! RIEN D'EFFACE, NI ICI NI LA-BAS. Le trait `Drive` n'a pas de suppression ; ici, un fichier qui existe n'est jamais
//! remplace.
//!
//! UN DOSSIER QUI N'EST PAS VIDE (le choix du lot R1) : il est accepte si tout ce que la sauvegarde y porterait (son
//! `inventaire`) est DEJA identique a la copie du Drive (meme empreinte), et refuse sinon, avant toute ecriture, en nommant
//! les fichiers. Pourquoi pas « ne recevoir que ce qui manque » : ce serait fabriquer une compagnie qu'aucun poste n'a eue
//! (un journal de depots d'ici, des compteurs de la-bas : regle 4, un numero de contrat se lit, il ne se recalcule pas), et
//! l'index reconstruit dirait a la sauvegarde suivante que tout est a jour. Pourquoi pas « vide ou rien » : (1) un poste neuf
//! a deja relie son Drive, et le jeton (`google-jeton.bin`) est dans `config/` — ecarte de la sauvegarde, il ne compte pas ;
//! (2) une restauration coupee (reseau, fermeture) se RELANCE et reprend ou elle en etait ; (3) un `google.json` recopie
//! a la main pour relier la bonne application Google n'empeche rien s'il est celui de la sauvegarde.
//!
//! Ce qu'elle ne redescend pas : ce que la sauvegarde n'envoie pas (`sauvegarde::ecarte`), un document sans empreinte
//! (un Google Docs : rien a comparer), un nom qui ne peut pas etre un nom de fichier ici, et les dossiers homonymes (deux
//! dossiers de meme nom au meme endroit du Drive : elle ne choisit pas, elle le dit).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use md5::{Digest, Md5};
use serde_json::{json, Map, Value};

use crate::coffre::{Coffre, SERVICES};
use crate::drive::{Drive, ErreurGoogle};
use crate::sauvegarde::{ecarte, ecrire_index, empreinte, hors_ligne, inventaire, DOSSIER_DRIVE};

/// Un fichier redescendu.
#[derive(Debug, Clone, PartialEq)]
pub struct Redescendu {
    pub relatif: String,
    pub octets: u64,
}

/// Le compte rendu d'une restauration.
#[derive(Debug, Clone, PartialEq)]
pub struct Restauration {
    pub cible: PathBuf,
    pub racine_drive: String,
    pub compte: Option<String>,
    pub redescendus: Vec<Redescendu>,
    /// Deja la, identiques a la copie du Drive : rien a faire.
    pub deja_la: usize,
    pub octets: u64,
    /// Dans le Drive, mais de ce que la sauvegarde n'envoie jamais : pas redescendus.
    pub ecartes: usize,
    pub incidents: Vec<String>,
    /// Le reseau est tombe en route : arretee. La relancer reprend.
    pub hors_ligne: bool,
}

impl Restauration {
    pub fn resume(&self) -> String {
        let mut t = format!("Restauration : {} fichier(s) redescendu(s), {}, {} deja la.", self.redescendus.len(), lisible(self.octets), self.deja_la);
        if self.hors_ligne {
            t.push_str(" Hors connexion en route : relancer `paie restaurer` reprend ou elle en est.");
        } else if !self.incidents.is_empty() {
            t.push_str(&format!(" {} incident(s).", self.incidents.len()));
        }
        t
    }

    pub fn en_json(&self) -> Value {
        json!({
            "resume": self.resume(),
            "cible": self.cible.display().to_string(),
            "racine_drive": self.racine_drive,
            "redescendus": self.redescendus.iter().map(|f| f.relatif.clone()).collect::<Vec<_>>(),
            "deja_la": self.deja_la,
            "octets": self.octets,
            "ecartes": self.ecartes,
            "incidents": self.incidents,
            "hors_ligne": self.hors_ligne,
        })
    }
}

fn lisible(octets: u64) -> String {
    if octets >= 1024 * 1024 {
        format!("{:.1} Mo", octets as f64 / (1024.0 * 1024.0))
    } else if octets >= 1024 {
        format!("{} Ko", octets / 1024)
    } else {
        format!("{} o", octets)
    }
}

fn md5_de(octets: &[u8]) -> String {
    Md5::digest(octets).iter().map(|b| format!("{:02x}", b)).collect()
}

/// Un nom venu du Drive peut-il etre un nom de fichier ici, sans sortir du dossier ? Ni vide, ni `.` ou `..`, ni barre
/// oblique (d'un cote ou de l'autre), ni NUL ; sous Windows, ni deux-points (`C:x` changerait de lecteur) ni ce que
/// Windows refuse.
fn nom_sur(nom: &str) -> bool {
    !nom.is_empty()
        && nom != "."
        && nom != ".."
        && !nom.contains(['/', '\\', '\0'])
        && !(cfg!(windows) && nom.contains([':', '<', '>', '"', '|', '?', '*']))
}

/// Un fichier de la sauvegarde, tel que l'inventaire du Drive le voit.
struct Entree {
    id: String,
    md5: Option<String>,
    /// Le dossier, relatif a `sauvegarde/` (`""` a la racine).
    dossier: String,
}

/// Le seul dossier de ce nom sous `parent`, sans rien creer. `Ok(None)` s'il n'y en a pas ; un refus s'il y en a deux.
fn le_dossier(client: &mut dyn Drive, parent: &str, nom: &str, chemin: &str) -> Result<Option<String>, ErreurGoogle> {
    let trouves: Vec<String> = client.sous_dossiers(parent)?.into_iter().filter(|(n, _)| n == nom).map(|(_, i)| i).collect();
    match trouves.len() {
        0 => Ok(None),
        1 => Ok(trouves.into_iter().next()),
        n => Err(ErreurGoogle::nouvelle(
            "SauvegardeAmbigue",
            format!("{} dossiers « {} » dans le Drive : la restauration ne choisit pas. Les reunir a la main dans le Drive, puis relancer.", n, chemin),
        )),
    }
}

/// `restaurer` : rend `cible` tel que la sauvegarde `racine_drive` du Drive le porte. Rend le compte rendu, n'imprime rien.
///
/// Un `Err` est un refus : rien n'a ete ecrit (sauvegarde introuvable ou ambigue, reseau tombe pendant l'inventaire,
/// dossier local qui n'est pas vide). Un incident en route (un fichier) n'arrete pas les autres et laisse l'index juste.
pub fn restaurer(cible: &Path, racine_drive: &str, client: &mut dyn Drive) -> Result<Restauration, ErreurGoogle> {
    let compte = client.compte();
    let mut resultat = Restauration {
        cible: cible.to_path_buf(),
        racine_drive: racine_drive.to_string(),
        compte: compte.clone(),
        redescendus: Vec::new(),
        deja_la: 0,
        octets: 0,
        ecartes: 0,
        incidents: Vec::new(),
        hors_ligne: false,
    };

    // 1. L'inventaire du Drive, sans rien y creer.
    let absente = || {
        ErreurGoogle::nouvelle(
            "SauvegardeAbsente",
            format!(
                "Pas de sauvegarde dans le Drive{} : le dossier {}/{} est introuvable. Le compte relie est-il celui de la \
                 compagnie ? Seuls les dossiers crees par PuppetPaie, avec la meme application Google, sont visibles.",
                compte.as_deref().map(|c| format!(" de {}", c)).unwrap_or_default(),
                racine_drive,
                DOSSIER_DRIVE
            ),
        )
    };
    let racine_id = le_dossier(client, "root", racine_drive, racine_drive)?.ok_or_else(absente)?;
    let sauvegarde_id = le_dossier(client, &racine_id, DOSSIER_DRIVE, &format!("{}/{}", racine_drive, DOSSIER_DRIVE))?.ok_or_else(absente)?;

    let mut distants: BTreeMap<String, Entree> = BTreeMap::new();
    let mut dossiers: BTreeMap<String, String> = BTreeMap::new();
    let mut a_voir: Vec<(String, String)> = vec![(String::new(), sauvegarde_id)];
    while let Some((dossier, id)) = a_voir.pop() {
        dossiers.insert(dossier.clone(), id.clone());
        let joindre = |nom: &str| if dossier.is_empty() { nom.to_string() } else { format!("{}/{}", dossier, nom) };
        for (nom, d) in client.lister(&id)? {
            let relatif = joindre(&nom);
            if !nom_sur(&nom) {
                resultat.incidents.push(format!("{} : ce nom ne peut pas etre un nom de fichier ici, il n'est pas redescendu.", relatif));
                continue;
            }
            if ecarte(Path::new(&relatif)) {
                resultat.ecartes += 1;
                continue;
            }
            distants.insert(relatif, Entree { id: d.id, md5: d.md5.filter(|m| !m.is_empty()), dossier: dossier.clone() });
        }
        let mut par_nom: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (nom, i) in client.sous_dossiers(&id)? {
            par_nom.entry(nom).or_default().push(i);
        }
        for (nom, ids) in par_nom {
            let relatif = joindre(&nom);
            if !nom_sur(&nom) {
                resultat.incidents.push(format!("{}/ : ce nom ne peut pas etre un nom de dossier ici, rien n'en est redescendu.", relatif));
            } else if ids.len() > 1 {
                resultat.incidents.push(format!(
                    "{}/ : {} dossiers de ce nom dans le Drive, rien n'en est redescendu (la restauration ne choisit pas : les reunir a la main dans le Drive, puis relancer).",
                    relatif,
                    ids.len()
                ));
            } else {
                a_voir.push((relatif, ids.into_iter().next().unwrap_or_default()));
            }
        }
    }

    // 2. Le dossier local : ce que la sauvegarde y porterait doit deja etre la copie du Drive.
    let (presents, _) = inventaire(cible);
    let mut deja: BTreeSet<String> = BTreeSet::new();
    let mut ecarts: Vec<String> = Vec::new();
    for (relatif, _) in &presents {
        let locale = empreinte(&cible.join(relatif)).ok();
        match distants.get(relatif) {
            Some(d) if d.md5.is_some() && d.md5 == locale => {
                deja.insert(relatif.clone());
            }
            Some(_) => ecarts.push(format!("{} (differe de la copie du Drive)", relatif)),
            None => ecarts.push(format!("{} (absent de la sauvegarde)", relatif)),
        }
    }
    if !ecarts.is_empty() {
        let mut liste: Vec<String> = ecarts.iter().take(10).map(|e| format!("  {}", e)).collect();
        if ecarts.len() > 10 {
            liste.push(format!("  ... et {} autre(s)", ecarts.len() - 10));
        }
        return Err(ErreurGoogle::nouvelle(
            "DossierNonVide",
            format!(
                "{} n'est pas vide : {} fichier(s) que la sauvegarde porterait y sont deja, et ne sont pas la copie du Drive :\n{}\n\
                 Rien n'a ete redescendu, rien n'a ete efface. Restaurer dans un dossier vide (le jeton Google peut y etre), ou \
                 mettre ces fichiers de cote, puis relancer.",
                cible.display(),
                ecarts.len(),
                liste.join("\n")
            ),
        ));
    }
    resultat.deja_la = deja.len();

    // 3. Ce qui manque, verifie avant d'etre ecrit.
    let mut verifies = deja;
    for (relatif, d) in &distants {
        if verifies.contains(relatif) {
            continue;
        }
        let Some(attendue) = d.md5.as_deref() else {
            resultat.incidents.push(format!("{} : le Drive n'en donne pas d'empreinte (un document Google ?), il n'est pas redescendu.", relatif));
            continue;
        };
        let chemin = relatif.split('/').fold(cible.to_path_buf(), |p, s| p.join(s));
        if std::fs::symlink_metadata(&chemin).is_ok() {
            resultat.incidents.push(format!("{} : il existe deja ici, il est garde tel quel.", relatif));
            continue;
        }
        let contenu = match client.telecharger(&d.id) {
            Ok(c) => c,
            Err(e) if hors_ligne(&e.message) => {
                resultat.hors_ligne = true;
                resultat.incidents.push(format!("{} : {}", relatif, e));
                break;
            }
            Err(e) => {
                resultat.incidents.push(format!("{} : {}", relatif, e));
                continue;
            }
        };
        let recue = md5_de(&contenu);
        if recue != attendue {
            resultat.incidents.push(format!(
                "{} : l'empreinte ne colle pas (le Drive annonce {}, le contenu recu donne {}) : rien n'est ecrit. Relancer `paie restaurer`.",
                relatif, attendue, recue
            ));
            continue;
        }
        match ecrire_sans_remplacer(&chemin, &contenu) {
            Ok(true) => {
                resultat.octets += contenu.len() as u64;
                resultat.redescendus.push(Redescendu { relatif: relatif.clone(), octets: contenu.len() as u64 });
                verifies.insert(relatif.clone());
            }
            Ok(false) => resultat.incidents.push(format!("{} : il est apparu ici pendant la restauration, il est garde tel quel.", relatif)),
            Err(e) => resultat.incidents.push(format!("{} : {}", relatif, e)),
        }
    }

    // 4. L'index, comme la sauvegarde l'ecrit, pour ce qui est verifie seulement.
    let (presents, _) = inventaire(cible);
    let mut fichiers = Map::new();
    let mut index_dossiers = Map::new();
    for (relatif, etat) in &presents {
        if !verifies.contains(relatif) {
            continue;
        }
        fichiers.insert(relatif.clone(), json!({"octets": etat.octets, "mtime": etat.mtime}));
        if let Some(d) = distants.get(relatif) {
            let mut cle = format!("{}/{}", racine_drive, DOSSIER_DRIVE);
            if !d.dossier.is_empty() {
                cle = format!("{}/{}", cle, d.dossier);
            }
            if let Some(id) = dossiers.get(&d.dossier) {
                index_dossiers.insert(cle, json!(id));
            }
        }
    }
    let maintenant = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
    let index = json!({
        "quand": crate::paris::iso_minutes(maintenant).replacen('T', " ", 1),
        "racine_drive": racine_drive,
        "dossiers": Value::Object(index_dossiers),
        "fichiers": Value::Object(fichiers),
    });
    ecrire_index(cible, &index)?;
    Ok(resultat)
}

/// Ecrit `contenu` a `chemin` par un `.part` renomme, SANS JAMAIS REMPLACER : `Ok(false)` si `chemin` existe deja.
fn ecrire_sans_remplacer(chemin: &Path, contenu: &[u8]) -> Result<bool, String> {
    if let Some(parent) = chemin.parent() {
        std::fs::create_dir_all(parent).map_err(|e| crate::fichiers::panne_fichier(&e, parent))?;
    }
    let mut part = chemin.as_os_str().to_owned();
    part.push(".part");
    let part = PathBuf::from(part);
    std::fs::write(&part, contenu).map_err(|e| crate::fichiers::panne_fichier(&e, &part))?;
    if std::fs::symlink_metadata(chemin).is_ok() {
        let _ = std::fs::remove_file(&part);
        return Ok(false);
    }
    std::fs::rename(&part, chemin).map_err(|e| crate::fichiers::panne_fichier(&e, chemin))?;
    Ok(true)
}

// ------------------------------------------------------------ ce qu'il reste a faire a la main

/// Ce qu'il reste a faire a la main apres une restauration, d'apres ce qui est revenu dans `coffre.racine` : le coffre (ce
/// qu'il porte, s'il s'ouvre a ce poste, par quoi), la cle de signature des mises a jour (par quoi elle s'ouvre : le coffre
/// depuis le lot K1, ou la cle de l'ancien poste seule ; `maj::cle_scellee_par_le_coffre` ne lit que la forme du fichier). Ne lit aucun secret : les fiches
/// du coffre sont en clair (ce qui est pose, depuis quand), et `ouvre_a_ce_poste` ne rend qu'un oui ou un non. Le coffre
/// est celui du poste (`Coffre::du_poste`) ; la preuve en donne un dont la cle est ailleurs.
pub fn a_faire(coffre: &Coffre) -> Vec<String> {
    let mut lignes = Vec::new();
    let cible = coffre.racine.as_path();
    let a_poser = |manquants: &[&str]| -> Vec<String> {
        SERVICES
            .iter()
            .filter(|s| manquants.contains(&s.nom))
            .map(|s| if s.avec_compte { format!("paie coffre poser {} --compte <adresse>", s.nom) } else { format!("paie coffre poser {}", s.nom) })
            .collect()
    };
    let tous: Vec<&str> = SERVICES.iter().map(|s| s.nom).collect();
    if !coffre.chemin().exists() {
        lignes.push(
            "Le coffre n'est pas dans la sauvegarde : poser le mot de passe du logiciel (`paie coffre ouverture`), puis ceux des organismes :".into(),
        );
        lignes.extend(a_poser(&tous).into_iter().map(|c| format!("  {}", c)));
    } else {
        match (coffre.ouverture_exigee(), coffre.etat()) {
            (Err(e), _) | (_, Err(e)) => lignes.push(format!("Le coffre est revenu, mais il ne se lit pas : {}", e.message)),
            (Ok(avec_mot_de_passe), Ok(etat)) => {
                let ici = coffre.ouvre_a_ce_poste().unwrap_or(false);
                let poses: Vec<String> = etat
                    .iter()
                    .filter(|f| f["pose"] == json!(true))
                    .map(|f| {
                        let le = f["pose_le"].as_str().unwrap_or("");
                        let nom = f["service"].as_str().unwrap_or("");
                        if le.is_empty() { nom.to_string() } else { format!("{} (pose le {})", nom, le) }
                    })
                    .collect();
                let manquants: Vec<&str> = etat.iter().filter(|f| f["pose"] != json!(true)).filter_map(|f| f["service"].as_str()).collect();
                let porte = if poses.is_empty() { "aucun mot de passe d'organisme".to_string() } else { poses.join(", ") };
                if ici {
                    lignes.push(format!("Le coffre est revenu et la cle de ce poste l'ouvre. Il porte : {}.", porte));
                } else if avec_mot_de_passe {
                    lignes.push(format!("Le coffre est revenu ; il porte : {}.", porte));
                    lignes.push(
                        "La cle de ce poste ne l'ouvre pas (elle ne voyage jamais) : le mot de passe du logiciel, oui. `paie coffre ouverture` \
                         le demande (l'actuel, puis le nouveau, qui peut etre le meme) et repose une cle ici."
                            .into(),
                    );
                } else {
                    lignes.push(format!(
                        "Le coffre est revenu ({}), mais sans mot de passe du logiciel : seule la cle de l'ancien poste l'ouvrait, il ne s'ouvrira pas ici. \
                         Le mettre de cote (config/coffre.bin), poser le mot de passe du logiciel (`paie coffre ouverture`), puis ceux des organismes :",
                        porte
                    ));
                    lignes.extend(a_poser(&tous).into_iter().map(|c| format!("  {}", c)));
                }
                if ici || avec_mot_de_passe {
                    let reste = a_poser(&manquants);
                    if !reste.is_empty() {
                        lignes.push("Ce qu'il ne porte pas, a poser s'il le faut :".into());
                        lignes.extend(reste.into_iter().map(|c| format!("  {}", c)));
                    }
                }
            }
        }
    }
    // La cle de signature (lot K1) : scellee par la cle maitresse du coffre, elle s'ouvre quand il s'ouvre, et seulement
    // avec lui ; scellee a l'ancienne, par la cle du poste qui l'a faite seule, elle ne s'ouvre nulle part ailleurs.
    match crate::maj::cle_scellee_par_le_coffre(cible) {
        None => {}
        Some(true) if coffre.chemin().exists() => lignes.push(
            "config/maj-cle-privee.bin (la cle qui signe les mises a jour) est revenu, scelle par le coffre : il s'ouvre quand le coffre \
             s'ouvre, ici par le mot de passe du logiciel, puis par la cle de ce poste une fois `paie coffre ouverture` passe."
                .into(),
        ),
        Some(true) => lignes.push(
            "config/maj-cle-privee.bin (la cle qui signe les mises a jour) est revenu, mais le coffre qui le scelle n'est pas revenu : \
             rien d'autre ne l'ouvre, et un coffre neuf (celui que `paie coffre ouverture` poserait) ne l'ouvrira pas. Remettre \
             d'abord config/coffre.bin de l'ancien poste s'il en reste une copie ; sinon, `paie maj --cle --remplacer` sur le poste \
             de l'editeur, et la nouvelle cle publique a faire parvenir aux compagnies."
                .into(),
        ),
        Some(false) => lignes.push(
            "config/maj-cle-privee.bin (la cle qui signe les mises a jour) est revenu, mais il n'est scelle que par la cle du poste qui \
             l'a fait, pas par le mot de passe du logiciel : sur un autre poste il ne s'ouvre pas, et `paie maj --publier` le dira."
                .into(),
        ),
    }
    lignes
}
