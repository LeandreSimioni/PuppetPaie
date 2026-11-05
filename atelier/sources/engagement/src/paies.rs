//! OU VIVENT LES PAIES : `paies/AAAA-MM.json`, le fichier du mois paye (`FichierDesPaies` : `{"mois", "paies": [...]}`). Une
//! entree par engagement valide qui touche le mois, designee par son salarie et le numero de son contrat (`salarie`,
//! `numero`, tels que l'engagement les porte) : sa paie ecrite (`presente`, une `paie_ecrite::PaieEcrite`), et, sous
//! `versions`, la paie de ce mois que chaque version passee de l'engagement avait ecrite (`{"<n>": <paie ecrite>}`) : ce
//! qui est parti sur elle s'y relit. L'engagement ne porte que ses termes ; un mois se lit dans son seul fichier.
//!
//! UNE SEULE FONCTION LIT UN FICHIER DE PAIES (`fichier`), UNE SEULE L'ECRIT (`ecrire`), par le stock et son brouillon
//! (`stock::Brouillon`) : un engagement et ses paies s'ecrivent dans la meme ecriture.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::paie_ecrite::PaieEcrite;
use crate::stock::{self, Dossier, Objet};
use socle::mois::Mois;
use socle::num::Resultat;

/// Le dossier des paies, sous les donnees de la compagnie.
pub const DOSSIER: &str = "paies";

/// LE FICHIER DES PAIES D'UN MOIS.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FichierDesPaies {
    mois: Mois,
    #[serde(default)]
    paies: Vec<Entree>,
}

/// L'entree d'un engagement : qui elle est, sa paie presente, et celles des versions passees.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entree {
    salarie: String,
    numero: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    presente: Option<PaieEcrite>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    versions: BTreeMap<String, PaieEcrite>,
}

/// Le fichier des paies de `mois`, pour le lire tel qu'il est sur le disque (l'archive de l'exercice).
pub fn chemin(racine: &std::path::Path, mois: &str) -> std::path::PathBuf {
    stock::chemin_dans(racine, Dossier::Paies, mois)
}

/// Le salarie et le numero du contrat de l'engagement `o` ; `None` sans numero (un engagement que rien n'a valide).
fn cle(o: &Objet) -> Option<(String, String)> {
    let numero = o.carte().numero_dit()?.to_string();
    Some((o.carte().salarie.clone().unwrap_or_default(), numero))
}

/// LA LECTURE D'UN FICHIER DE PAIES : celui de `mois`, tel que le geste l'a lu ou que le brouillon le tient, lu par sa
/// forme ; les entrees, par salarie et numero, sont tirees une fois du meme contenu (`Arc::ptr_eq` : le geste et le
/// brouillon servent la meme valeur tant qu'elle n'a pas change).
fn fichier(racine: &std::path::Path, mois: &str) -> Resultat<Option<Arc<FichierDesPaies>>> {
    type Gardes = Mutex<HashMap<(std::path::PathBuf, String), (Arc<Value>, Arc<FichierDesPaies>)>>;
    static GARDES: OnceLock<Gardes> = OnceLock::new();
    let Some(v) = stock::lire_dans(racine, Dossier::Paies, mois)? else { return Ok(None) };
    let gardes = GARDES.get_or_init(Default::default);
    let cle_du_fichier = (racine.to_path_buf(), mois.to_string());
    if let Some((vu, f)) = gardes.lock().unwrap_or_else(|p| p.into_inner()).get(&cle_du_fichier) {
        if Arc::ptr_eq(vu, &v) {
            return Ok(Some(f.clone()));
        }
    }
    let f: Arc<FichierDesPaies> = Arc::new(socle::forme::lire(&v, socle::echec::PAIE_ILLISIBLE)?);
    socle::fichiers::ranger(gardes, cle_du_fichier, (v, f.clone()));
    Ok(Some(f))
}

/// L'entree de l'engagement `o` dans le fichier de `mois`, telle qu'elle est ecrite.
fn entree(o: &Objet, mois: &str) -> Resultat<Option<Entree>> {
    let Some(cle) = cle(o) else { return Ok(None) };
    Ok(fichier(&o.racine, mois)?.and_then(|f| f.paies.iter().find(|e| (&e.salarie, &e.numero) == (&cle.0, &cle.1)).cloned()))
}

/// L'ECRITURE D'UNE PAIE : l'entree de `o` dans le fichier de `mois` (neuve s'il n'en a pas) passe par `change` ; une
/// entree qui ne porte plus rien s'en va. Le fichier ne s'ecrit que s'il change. Refus : un engagement sans numero de
/// contrat.
fn ecrire(o: &Objet, mois: &str, change: impl FnOnce(&mut Entree)) -> Resultat<()> {
    let (salarie, numero) = cle(o).ok_or_else(|| format!("{} : un engagement sans numero de contrat n'a pas de paie", mois))?;
    let lu = fichier(&o.racine, mois)?;
    let mut f = match &lu {
        Some(f) => (**f).clone(),
        None => FichierDesPaies { mois: Mois::lire(mois)?, paies: Vec::new() },
    };
    let place = f.paies.iter().position(|e| e.salarie == salarie && e.numero == numero);
    let mut e = match place {
        Some(i) => f.paies[i].clone(),
        None => Entree { salarie, numero, presente: None, versions: BTreeMap::new() },
    };
    change(&mut e);
    let vide = e.presente.is_none() && e.versions.is_empty();
    match (place, vide) {
        (Some(i), true) => {
            f.paies.remove(i);
        }
        (Some(i), false) => f.paies[i] = e,
        (None, false) => f.paies.push(e),
        (None, true) => return Ok(()),
    }
    if lu.as_deref() == Some(&f) {
        return Ok(());
    }
    let contenu = serde_json::to_value(&f).map_err(|e| e.to_string())?;
    stock::poser_dans(&o.racine, Dossier::Paies, mois, &contenu)?;
    Ok(())
}

/// La paie ecrite de `mois` de l'engagement `o`, celle de sa version presente ; `None` s'il n'en a pas.
pub fn presente(o: &Objet, mois: &str) -> Resultat<Option<Arc<PaieEcrite>>> {
    Ok(entree(o, mois)?.and_then(|e| e.presente).map(Arc::new))
}

/// La paie de `mois` que la version passee `n` de l'engagement `o` avait ecrite ; `None` si elle n'en a pas.
pub fn de_la_version(o: &Objet, n: u64, mois: &str) -> Resultat<Option<PaieEcrite>> {
    Ok(entree(o, mois)?.and_then(|mut e| e.versions.remove(&n.to_string())))
}

/// Toutes les paies ecrites de `mois` de l'engagement `o` : la presente, puis celles des versions passees.
pub fn ecrites(o: &Objet, mois: &str) -> Resultat<Vec<PaieEcrite>> {
    let Some(e) = entree(o, mois)? else { return Ok(Vec::new()) };
    Ok(e.presente.into_iter().chain(e.versions.into_values()).collect())
}

/// Les mois qui ont un fichier de paies, du plus ancien au plus recent.
pub(crate) fn mois_payes(racine: &std::path::Path) -> Vec<String> {
    stock::fichiers_dans(racine, Dossier::Paies)
}

/// Les mois ou l'engagement `o` a une paie ecrite (presente ou d'une version passee), du premier au dernier.
pub fn mois_ecrits(o: &Objet) -> Resultat<Vec<String>> {
    let mut out = Vec::new();
    for m in stock::fichiers_dans(&o.racine, Dossier::Paies).into_iter().filter(|m| m.as_str() >= o.fichier.as_str()) {
        if entree(o, &m)?.is_some() {
            out.push(m);
        }
    }
    Ok(out)
}

/// Les paies ecrites de la version presente de l'engagement `o`, par mois (`mois_ecrits`).
pub fn presentes(o: &Objet) -> Resultat<BTreeMap<String, PaieEcrite>> {
    let mut out = BTreeMap::new();
    for m in mois_ecrits(o)? {
        if let Some(p) = presente(o, &m)? {
            out.insert(m, (*p).clone());
        }
    }
    Ok(out)
}

/// Le nombre de paies ecrites de `mois`, celles des versions presentes des engagements.
pub fn nombre(racine: &std::path::Path, mois: &str) -> Resultat<usize> {
    Ok(fichier(racine, mois)?.map_or(0, |f| f.paies.iter().filter(|e| e.presente.is_some()).count()))
}

/// La paie ecrite de `mois` de l'engagement `o` devient `paie` ; `None` la retire (les versions passees restent).
pub(crate) fn poser(o: &Objet, mois: &str, paie: Option<PaieEcrite>) -> Resultat<()> {
    ecrire(o, mois, |e| e.presente = paie)
}

/// LA VERSION `n` DE L'ENGAGEMENT `o` DEVIENT PASSEE : la paie presente de chaque mois ou il en a une se garde sous son
/// numero (`de_la_version`).
pub(crate) fn garder_la_version(o: &Objet, n: u64) -> Resultat<()> {
    for m in mois_ecrits(o)? {
        if let Some(p) = presente(o, &m)? {
            ecrire(o, &m, |e| {
                e.versions.insert(n.to_string(), (*p).clone());
            })?;
        }
    }
    Ok(())
}

/// Les paies de l'engagement `o`, presentes et passees, s'effacent de tous ses mois.
pub(crate) fn effacer(o: &Objet) -> Resultat<()> {
    for m in mois_ecrits(o)? {
        ecrire(o, &m, |e| {
            e.presente = None;
            e.versions.clear();
        })?;
    }
    Ok(())
}
