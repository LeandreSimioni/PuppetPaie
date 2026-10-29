//! UNE FENETRE PAR INSTALLATION : une installation sur une cle USB s'ouvre sur n'importe quel poste, meme si ce poste a
//! deja un logiciel. Un logiciel = un
//! administrateur = une installation (le dossier qui porte `logiciel/`, et a cote `prive/`, `compagnies/*`, `fictives/*`,
//! `administrateur.json`). Deux installations s'ouvrent cote a cote sur le meme poste ; la meme, lancee deux fois, ramene
//! sa fenetre devant.
//!
//! LE CRENEAU. Une installation ouverte tient un CRENEAU : deux ports voisins de la boucle locale, dans une plage fixe
//! (`Plage::DU_POSTE` : 7330 a 7429, cinquante creneaux). Le premier est le VERROU (une prise d'ecoute que le systeme
//! rend quand le processus meurt, meme tue) ; le second, le PORT DE LA PAGE. Pourquoi un port de page STABLE : le
//! navigateur (WebView2, WebKit, Chromium) retient un mot de passe, et le reste, PAR ORIGINE (`http://hote:port`) ; un port
//! tire au hasard ferait de chaque lancement un site neuf. Pourquoi un port par installation : deux pages au meme port ne
//! peuvent pas tourner ensemble, et deux installations a la meme origine se meleraient leurs mots de passe.
//!
//! QUEL CRENEAU. Celui que l'installation a RETENU (`.fenetre.json`, a sa racine : il voyage avec elle, une cle USB garde
//! son port d'un poste a l'autre) ; a defaut, un creneau tire de l'emplacement de l'installation (une empreinte de son
//! chemin), pour que deux installations neuves ne visent pas le meme. Puis, a partir de lui, le premier creneau qui va :
//!   - son verrou est libre (et son port de page aussi) : il est pris, et retenu s'il ne l'etait pas ;
//!   - son verrou est tenu par CETTE installation (elle repond par son dossier, le meme) : elle revient devant, `Devant` ;
//!   - tenu par une AUTRE installation (un autre dossier), ou par un autre programme, ou son port de page est pris : le
//!     creneau suivant. Plus de page « deja ouvert » : l'autre installation n'empeche rien.
//! Aucun creneau ne va (cinquante pris) : la fenetre s'ouvre quand meme, a un port que le systeme donne, sans verrou --
//! un verrou ne doit jamais empecher une paie.
//!
//! Le dossier compare est le chemin canonique de l'installation : une copie (un autre dossier) est une autre
//! installation, meme si elle a emporte le `.fenetre.json` de l'original -- elle prend alors le creneau suivant, et le
//! retient pour elle.

use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

/// Le fichier, a la racine de l'installation, qui retient son creneau.
pub const FICHIER: &str = ".fenetre.json";
/// Ce qui appartient a une installation sur un poste et qu'on ne met pas dans ses donnees (le profil du navigateur de
/// la fenetre, le jeton de sa page, le journal d'avant toute compagnie) : en mode portable, a sa racine.
pub const DOSSIER_PORTABLE: &str = ".fenetre";

const DEMANDE_DEVANT: &str = "PuppetPaie devant";
const DEMANDE_QUI: &str = "PuppetPaie qui";
const REPONSE: &str = "PuppetPaie ok";
const DELAI: Duration = Duration::from_secs(2);

/// La plage des creneaux : `creneaux` paires de ports a partir de `premier` (le verrou, puis la page).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plage {
    pub premier: u16,
    pub creneaux: u16,
}

impl Plage {
    /// La plage du poste : 7330 a 7429. 7330/7331 etaient le verrou et la page d'avant (une seule fenetre par poste).
    pub const DU_POSTE: Plage = Plage { premier: 7330, creneaux: 50 };

    pub fn verrou(&self, creneau: u16) -> u16 {
        self.premier + 2 * creneau
    }

    pub fn page(&self, creneau: u16) -> u16 {
        self.premier + 2 * creneau + 1
    }
}

/// Ce que tient une fenetre ouverte : le verrou de son creneau (a garder tant qu'elle vit), l'ecoute de sa page, et son
/// port. `creneau` (et `verrou`) : `None` quand aucun creneau n'allait (pas de verrou, un port du systeme).
pub struct Prise {
    pub verrou: Option<Verrou>,
    pub page: TcpListener,
    pub port: u16,
    pub creneau: Option<u16>,
}

/// Le verrou d'un creneau : tant qu'il vit, le creneau est a la fenetre ; il le rend en tombant (et, bien sur, quand le
/// processus meurt).
pub struct Verrou {
    ferme: Arc<AtomicBool>,
}

impl Drop for Verrou {
    fn drop(&mut self) {
        self.ferme.store(true, Ordering::SeqCst);
    }
}

/// Ce que dit le verrou.
pub enum Instance {
    /// Cette fenetre est celle de l'installation : sa prise.
    Tenue(Prise),
    /// La fenetre de CETTE installation est deja ouverte : elle vient d'etre ramenee devant.
    Devant,
}

/// Le dossier d'une installation, tel que le verrou le compare : canonique, sans le prefixe `\\?\` de Windows.
pub fn dossier_de(inst: &Path) -> String {
    let c = std::fs::canonicalize(inst).unwrap_or_else(|_| inst.to_path_buf());
    let t = c.to_string_lossy().into_owned();
    t.strip_prefix(r"\\?\").map(String::from).unwrap_or(t)
}

/// Le creneau retenu par l'installation, s'il se lit et tient dans la plage.
pub fn creneau_retenu(inst: &Path, plage: Plage) -> Option<u16> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(inst.join(FICHIER)).ok()?).ok()?;
    let k = u16::try_from(v.get("creneau")?.as_u64()?).ok()?;
    (k < plage.creneaux).then_some(k)
}

/// Retient `creneau` pour l'installation. Une installation ou l'on n'ecrit pas (un support en lecture seule) garde son
/// creneau tire de son emplacement : rien ne casse, le port est seulement moins stable.
fn retenir(inst: &Path, creneau: u16) {
    let v = json!({
        "creneau": creneau,
        "_commentaire": "Le creneau de la fenetre de cette installation (moteur/src/interface/instance.rs) : son port de page reste le meme d'un lancement a l'autre, et le navigateur y retrouve ce qu'il a retenu.",
    });
    let _ = socle::ecriture::ecrire_json(&inst.join(FICHIER), &v, 2);
}

/// Le creneau que vise une installation qui n'en a pas retenu : une empreinte de son dossier.
fn creneau_tire(dossier: &str, plage: Plage) -> u16 {
    use sha2::{Digest, Sha256};
    let h = Sha256::digest(dossier.as_bytes());
    u16::from_be_bytes([h[0], h[1]]) % plage.creneaux.max(1)
}

/// Le premier creneau a essayer : le retenu, sinon le tire.
pub fn creneau_vise(inst: &Path, plage: Plage) -> u16 {
    creneau_retenu(inst, plage).unwrap_or_else(|| creneau_tire(&dossier_de(inst), plage))
}

/// Qui tient le verrou au port `port` : `Some(dossier)` si c'est une fenetre de PuppetPaie (son installation), `None`
/// sinon (personne, ou un autre programme). `devant` : lui demander de revenir devant (elle ne le fait que si c'est la
/// sienne, `ici`).
fn demander(port: u16, ici: &str, devant: bool) -> Option<String> {
    let mut c = TcpStream::connect_timeout(&SocketAddr::from((Ipv4Addr::LOCALHOST, port)), DELAI).ok()?;
    let _ = c.set_read_timeout(Some(DELAI));
    let verbe = if devant { DEMANDE_DEVANT } else { DEMANDE_QUI };
    c.write_all(format!("{} {}\n", verbe, ici).as_bytes()).ok()?;
    let mut l = String::new();
    BufReader::new(c).read_line(&mut l).ok()?;
    l.trim_end().strip_prefix(REPONSE).map(|r| r.trim().to_string())
}

/// Le verrou tenu : il repond a qui demande, par le dossier de l'installation, et ramene la fenetre devant quand c'est
/// la MEME installation qui est relancee (une autre ne fait que demander qui est la). Le fil tient seul la prise ; il la
/// rend des que le `Verrou` tombe (la prise attend sans bloquer : le fil regarde le drapeau toutes les 50 ms).
fn repondre(ecoute: TcpListener, ici: String, devant: impl Fn() + Send + 'static) -> Verrou {
    let ferme = Arc::new(AtomicBool::new(false));
    let vu = ferme.clone();
    let _ = ecoute.set_nonblocking(true);
    std::thread::spawn(move || {
        while !vu.load(Ordering::SeqCst) {
            let c = match ecoute.accept() {
                Ok((c, _)) => c,
                Err(_) => {
                    std::thread::sleep(Duration::from_millis(50));
                    continue;
                }
            };
            let _ = c.set_nonblocking(false);
            let _ = c.set_read_timeout(Some(DELAI));
            let mut lecteur = BufReader::new(&c);
            let mut ligne = String::new();
            if lecteur.read_line(&mut ligne).is_err() {
                continue;
            }
            let ligne = ligne.trim_end();
            let (verbe, la_bas) = if let Some(r) = ligne.strip_prefix(DEMANDE_DEVANT) {
                (true, r.trim())
            } else if let Some(r) = ligne.strip_prefix(DEMANDE_QUI) {
                (false, r.trim())
            } else {
                continue;
            };
            let _ = (&c).write_all(format!("{} {}\n", REPONSE, ici).as_bytes());
            if verbe && la_bas == ici {
                devant();
            }
        }
    });
    Verrou { ferme }
}

fn ecouter(port: u16) -> Option<TcpListener> {
    let l = TcpListener::bind((Ipv4Addr::LOCALHOST, port)).ok()?;
    super::http::sans_heritage(&l);
    Some(l)
}

/// LE VERROU DE L'INSTALLATION `inst` (voir l'en-tete). `devant` : ce que fait la fenetre tenue quand la meme
/// installation est relancee.
pub fn prendre_de(inst: &Path, plage: Plage, devant: impl Fn() + Send + 'static) -> Instance {
    let ici = dossier_de(inst);
    let retenu = creneau_retenu(inst, plage);
    let depart = retenu.unwrap_or_else(|| creneau_tire(&ici, plage));
    for i in 0..plage.creneaux {
        let k = (depart + i) % plage.creneaux;
        let Some(verrou) = ecouter(plage.verrou(k)) else {
            // Tenu : par cette installation (elle revient devant), par une autre, ou par un autre programme.
            if demander(plage.verrou(k), &ici, true).as_deref() == Some(ici.as_str()) {
                return Instance::Devant;
            }
            continue;
        };
        // Le port de la page doit etre libre aussi : sinon le creneau ne va pas (un autre programme y est).
        let Some(page) = ecouter(plage.page(k)) else { continue };
        if retenu != Some(k) {
            retenir(inst, k);
        }
        let verrou = repondre(verrou, ici.clone(), devant);
        return Instance::Tenue(Prise { verrou: Some(verrou), page, port: plage.page(k), creneau: Some(k) });
    }
    // Aucun creneau : la fenetre s'ouvre quand meme, sans verrou, a un port du systeme.
    match ecouter(0) {
        Some(page) => {
            let port = page.local_addr().map(|a| a.port()).unwrap_or(0);
            Instance::Tenue(Prise { verrou: None, page, port, creneau: None })
        }
        None => Instance::Devant,
    }
}

/// Le verrou de CETTE installation (`compagnies::base()`), dans la plage du poste.
pub fn prendre(devant: impl Fn() + Send + 'static) -> Instance {
    prendre_de(&super::compagnies::base(), Plage::DU_POSTE, devant)
}

/// LA FENETRE DE L'INSTALLATION `inst` EST-ELLE OUVERTE ? (la mise a jour ne s'applique jamais sous elle). On demande au
/// creneau qu'elle vise -- le retenu, sinon le tire -- et a ceux qui suivent tant qu'une autre installation les tient :
/// les memes creneaux que `prendre_de` parcourrait. Un creneau libre arrete la recherche (elle l'aurait pris).
pub fn ouverte(inst: &Path, plage: Plage) -> bool {
    let ici = dossier_de(inst);
    let depart = creneau_vise(inst, plage);
    for i in 0..plage.creneaux {
        let k = (depart + i) % plage.creneaux;
        if let Some(l) = ecouter(plage.verrou(k)) {
            drop(l);
            return false;
        }
        match demander(plage.verrou(k), &ici, false) {
            Some(d) if d == ici => return true,
            _ => continue,
        }
    }
    false
}

/// Le dossier propre a une fenetre sur ce poste : le profil de son navigateur, le jeton de sa page, son journal d'avant
/// toute compagnie. EN PORTABLE (`PORTABLE.json`), dans l'installation (`.fenetre/`) : rien ne reste sur le poste. SINON,
/// dans le dossier de l'utilisateur, un par PORT de page (`fenetres/<port>/`) -- le port est l'origine que le
/// navigateur connait : ce qu'il a retenu pour elle reste avec elle, et deux fenetres ouvertes ensemble (deux ports)
/// n'ont jamais le meme profil.
pub fn dossier_local(inst: &Path, port: u16) -> PathBuf {
    if inst.join(crate::compagnie::coffre::MARQUEUR_PORTABLE).is_file() {
        return inst.join(DOSSIER_PORTABLE);
    }
    super::navigateur::dossier_utilisateur().join("fenetres").join(port.to_string())
}
