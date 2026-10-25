//! LE TEMPS DU MOTEUR (#236, phase 2) : l'horloge de la machine, ou un temps SIMULE, et les aleas qui vont avec.
//!
//! Tout ce que le moteur ECRIT avec l'heure dedans -- le « quand » d'une ligne du journal, l'horodatage d'une DSN ou d'une
//! DPAE, le nom d'un courriel joue, la sauvegarde d'une saisie dans `paies/avant/` -- lit l'heure ICI (`secondes`), et le
//! jour qu'on est aussi (`contexte::aujourdhui`, `contexte::jour_reel`). Sur un poste, rien n'est jamais pose : c'est
//! l'horloge de la machine, au meme prix qu'avant (un verrou libre, puis `SystemTime::now`).
//!
//! DEUX USAGES DE L'ATELIER, et eux seuls posent le temps (aucune variable d'environnement, aucune commande du binaire
//! livre ne le fait) :
//! - LE BANC D'ESSAI (`paie-atelier interface --jour J`, `scenario::banc`) : le temps COULE depuis le jour J a 9 h (Paris), au
//!   pas de la montre ; « avancer au jour J' » le repose. Pendant chaque route servie, il est FIGE a l'instant ou elle a
//!   commence (`figer`) : la route lit une seule heure du debut a la fin, comme au rejeu. Les aleas y suivent la meme suite
//!   fixe qu'au rejeu (le banc n'est jamais un poste) : `scenario::verifier` compare la compagnie du banc a son rejeu.
//! - LE REJEU d'un scenario (`scenario::rejouer`) : chaque geste rejoue a l'instant ou il a ete enregistre, fige ; et les
//!   aleas (`aleas`) deviennent une suite fixe, tiree de l'instant du geste et d'un compteur remis a zero a chaque geste :
//!   deux rejeux ecrivent les memes octets, et le banc ecrit ceux de son rejeu.
//!
//! Ce qui MESURE une duree (une attente, l'age d'un verrou, la veille qui repasse, les caches tenus par la date d'un
//! fichier) garde la montre de la machine : ce n'est pas une heure qu'on ecrit.
//!
//! Un seul temps par processus : la fenetre de l'atelier sert une compagnie a la fois, et un rejeu tourne seul dans son
//! processus (un essai par processus sous `cargo nextest`, l'exemple `vie_fictive`).

use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use crate::date::Jour;

struct Simule {
    /// Le temps qui coule : l'instant simule pose, et la montre a ce moment-la. `None` : on fige seulement (le rejeu).
    coule: Option<(f64, Instant)>,
    /// Le temps fige, par-dessus : l'instant que lisent les routes tant qu'il est pose.
    fige: Option<f64>,
    /// Les aleas en suite fixe (le rejeu) : le nombre de tirages faits. `None` : les aleas du systeme.
    tirages: Option<u64>,
}

static SIMULE: Mutex<Option<Simule>> = Mutex::new(None);

fn etat() -> std::sync::MutexGuard<'static, Option<Simule>> {
    SIMULE.lock().unwrap_or_else(|p| p.into_inner())
}

fn montre() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64())
}

/// L'heure du mur, en millisecondes depuis 1970 : celle de la page (`Date.now()`), jamais le temps simule. Elle date ce
/// qu'on garde en memoire pour un diagnostic (les echecs, le fil des actions), pas ce qu'on ecrit.
pub fn maintenant_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// L'instant present, en secondes depuis 1970 (UTC) : simule s'il est pose, sinon la montre de la machine.
pub fn secondes() -> f64 {
    match etat().as_ref() {
        Some(Simule { fige: Some(t), .. }) => *t,
        Some(Simule { coule: Some((depart, pose)), .. }) => depart + pose.elapsed().as_secs_f64(),
        _ => montre(),
    }
}

/// Le temps est-il simule (le banc, le rejeu) ?
pub fn simule() -> bool {
    etat().is_some()
}

/// LE BANC : le temps coule depuis `instant` (secondes UTC), au pas de la montre. Ce qui etait fige ne l'est plus.
pub fn couler_depuis(instant: f64) {
    let mut e = etat();
    let tirages = e.as_ref().and_then(|s| s.tirages);
    *e = Some(Simule { coule: Some((instant, Instant::now())), fige: None, tirages });
}

/// Fige l'instant (le rejeu d'un geste, une route servie au banc) ; le temps qui coule, s'il y en a un, reprend a
/// `defiger`.
pub fn figer(instant: f64) {
    let mut e = etat();
    match e.as_mut() {
        Some(s) => {
            s.fige = Some(instant);
            // LES ALEAS DE CHAQUE GESTE repartent de zero a son instant : un geste tire la meme suite au banc et au rejeu,
            // quelles que soient les lectures (que le rejeu ecarte) tirees avant lui.
            if s.tirages.is_some() {
                s.tirages = Some(0);
            }
        }
        None => *e = Some(Simule { coule: None, fige: Some(instant), tirages: None }),
    }
}

/// Le temps ne reste plus fige : celui qui coule reprend (le banc) ; sans lui, c'est encore l'instant fige.
pub fn defiger() {
    if let Some(s) = etat().as_mut() {
        if s.coule.is_some() {
            s.fige = None;
        }
    }
}

/// LE REJEU commence : les aleas deviennent une suite fixe, a partir du premier tirage.
pub fn aleas_fixes() {
    let mut e = etat();
    match e.as_mut() {
        Some(s) => s.tirages = Some(0),
        None => *e = Some(Simule { coule: None, fige: None, tirages: Some(0) }),
    }
}

/// Retour a la montre de la machine et aux aleas du systeme.
pub fn liberer() {
    *etat() = None;
}

/// Le jour qu'on est a Paris, au temps du moteur.
pub fn jour_de_paris() -> Jour {
    let t = secondes().floor() as i64;
    let local = t + crate::paris::decalage(t);
    Jour::depuis_jours_unix(local.div_euclid(86_400)).expect("horloge")
}

/// Le jour UTC, au temps du moteur (`contexte::jour_reel`).
pub fn jour_utc() -> Jour {
    Jour::depuis_jours_unix(secondes().div_euclid(86_400.0) as i64).expect("horloge")
}

/// L'instant de `jour` a `heure` h, heure de Paris, en secondes UTC.
pub fn instant_de(jour: Jour, heure: i64) -> f64 {
    let local = (jour.ordinal() - 719_163) * 86_400 + heure * 3_600;
    // Le decalage de Paris a cet instant : celui de l'instant local lu comme UTC, corrige une fois (un changement d'heure
    // ne tombe jamais a 9 h).
    let approx = local - crate::paris::decalage(local);
    (local - crate::paris::decalage(approx)) as f64
}

/// DES ALEAS POUR UN IDENTIFIANT (le nom d'un courriel joue, une frontiere MIME, le flux d'un depot joue) : ceux du
/// systeme, ou, au rejeu, la suite fixe (SHA-256 du rang du tirage). Jamais pour un secret : le coffre, la cle d'une
/// mise a jour, l'autorisation Google tirent les leurs du systeme, toujours.
pub fn aleas(o: &mut [u8]) -> Result<(), String> {
    let tirage = {
        let mut e = etat();
        match e.as_mut() {
            Some(Simule { tirages: Some(n), fige, .. }) => {
                *n += 1;
                Some((*n, fige.map_or(0, f64::to_bits)))
            }
            _ => None,
        }
    };
    let Some((rang, graine)) = tirage else {
        return getrandom::fill(o).map_err(|e| e.to_string());
    };
    let mut bloc = 0u64;
    for morceau in o.chunks_mut(32) {
        let h = Sha256::digest(format!("puppetpaie-rejeu-{}-{}-{}", graine, rang, bloc).as_bytes());
        morceau.copy_from_slice(&h[..morceau.len()]);
        bloc += 1;
    }
    Ok(())
}

/// UNE DUREE QU'ON ECRIT (le temps qu'a pris un envoi, un passage de la veille) : celle de la montre ; au rejeu, zero --
/// elle ne dit rien de la compagnie, et deux rejeux doivent ecrire les memes octets.
pub fn duree(debut: Instant) -> std::time::Duration {
    if etat().as_ref().is_some_and(|s| s.tirages.is_some()) {
        std::time::Duration::ZERO
    } else {
        debut.elapsed()
    }
}

/// Le numero du processus, pour un identifiant (un `Message-ID`) : au rejeu, toujours 1.
pub fn numero_de_processus() -> u32 {
    if etat().as_ref().is_some_and(|s| s.tirages.is_some()) {
        1
    } else {
        std::process::id()
    }
}
