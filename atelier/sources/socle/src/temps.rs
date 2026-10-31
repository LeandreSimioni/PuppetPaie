//! LE TEMPS DU MOTEUR : l'horloge de la machine, ou un temps SIMULE, et les aleas qui vont avec.
//!
//! Tout ce que le moteur ECRIT avec l'heure dedans -- le « quand » d'une ligne du journal, l'horodatage d'une DSN ou d'une
//! DPAE, le nom d'un courriel joue -- lit l'heure ICI (`secondes`), et le
//! jour qu'on est aussi (`contexte::aujourdhui`, `contexte::jour_reel`). Sur un poste, c'est l'horloge de la machine, au
//! jour de Paris que la commande pose (`PUPPETPAIE_AUJOURDHUI`, le jour meme), pour le prix d'un verrou libre puis de
//! `SystemTime::now`.
//!
//! LE JOUR POSE : quand `PUPPETPAIE_AUJOURDHUI` nomme un autre jour (un essai, une compagnie fictive qu'on fait vivre a une
//! date), l'heure est celle de la montre, ce jour-la : un fichier ou une ligne de journal ecrits ce jour-la en portent la
//! date, comme les calculs qui lisent `aujourdhui`.
//!
//! DEUX USAGES DE L'ATELIER SIMULENT le temps, et passent devant le jour pose (aucune commande du binaire livre ne le
//! fait) :
//! - LE BANC D'ESSAI (`moteur-atelier interface --jour J`, `scenario::banc`) : le temps COULE depuis le jour J a 9 h (Paris), au
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
//! LE TEMPS QUI COULE est un par processus (la fenetre de l'atelier sert une compagnie a la fois, et un rejeu tourne seul
//! dans son processus). L'INSTANT FIGE ET LE COMPTEUR DES ALEAS sont propres a chaque route servie, donc a son fil : des
//! routes servies en meme temps (la page, la veille, l'etat refait en fond) ne se figent ni ne se comptent l'une l'autre.

use std::cell::Cell;
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use crate::date::Jour;

/// Le temps du processus, quand il est simule.
#[derive(Clone, Copy)]
struct Simule {
    /// Le temps qui coule : l'instant simule pose, et la montre a ce moment-la. `None` : on fige seulement (le rejeu).
    coule: Option<(f64, Instant)>,
    /// Les aleas en suite fixe (le rejeu, le banc) ; sinon, les aleas du systeme.
    aleas_fixes: bool,
}

static SIMULE: Mutex<Option<Simule>> = Mutex::new(None);

thread_local! {
    /// L'instant que lit la route servie sur ce fil, tant qu'il est pose.
    static FIGE: Cell<Option<f64>> = const { Cell::new(None) };
    /// Les tirages deja faits par la route servie sur ce fil (aleas en suite fixe).
    static TIRAGES: Cell<u64> = const { Cell::new(0) };
}

fn etat() -> std::sync::MutexGuard<'static, Option<Simule>> {
    SIMULE.lock().unwrap_or_else(|p| p.into_inner())
}

fn aleas_fixes_poses() -> bool {
    etat().as_ref().is_some_and(|s| s.aleas_fixes)
}

fn montre() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64())
}

/// L'heure du mur, en millisecondes depuis 1970 : celle de la page (`Date.now()`), jamais le temps simule. Elle date ce
/// qu'on garde en memoire pour un diagnostic (les echecs, le fil des actions), pas ce qu'on ecrit.
pub fn maintenant_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// L'instant present, en secondes depuis 1970 (UTC) : fige pour la route de ce fil, ou simule s'il coule, sinon la montre
/// de la machine, portee au jour que `PUPPETPAIE_AUJOURDHUI` pose quand il y en a un (a la meme heure de Paris).
pub fn secondes() -> f64 {
    if let Some(t) = FIGE.get() {
        return t;
    }
    match etat().as_ref() {
        Some(Simule { coule: Some((depart, pose)), .. }) => depart + pose.elapsed().as_secs_f64(),
        _ => au_jour_pose(montre()),
    }
}

/// LE JOUR POSE (`PUPPETPAIE_AUJOURDHUI`, AAAA-MM-JJ) : la montre, decalee d'autant de jours qu'il y en a entre le jour
/// de Paris ou il a ete lu pour la premiere fois et le jour pose : la meme heure (a une heure pres d'un changement d'heure
/// entre les deux), ce jour-la, et minuit passe comme a la montre. Sans jour pose, ou illisible, `t` tel quel.
fn au_jour_pose(t: f64) -> f64 {
    static DECALAGE: Mutex<Option<(String, f64)>> = Mutex::new(None);
    let Some(pose) = std::env::var("PUPPETPAIE_AUJOURDHUI").ok().filter(|v| !v.is_empty()) else { return t };
    let mut d = DECALAGE.lock().unwrap_or_else(|p| p.into_inner());
    if let Some((_, decalage)) = d.as_ref().filter(|(lu, _)| *lu == pose) {
        return t + decalage;
    }
    let Ok(jour) = Jour::lire(&pose) else { return t };
    let entier = t.floor() as i64;
    let jour_de_la_montre = (entier + crate::paris::decalage(entier)).div_euclid(86_400);
    let decalage = ((jour.ordinal() - 719_163) - jour_de_la_montre) as f64 * 86_400.0;
    *d = Some((pose, decalage));
    t + decalage
}

/// Le temps est-il simule (le banc, le rejeu, un geste joue a sa date) ?
pub fn simule() -> bool {
    FIGE.get().is_some() || etat().is_some()
}

/// LE BANC : le temps coule depuis `instant` (secondes UTC), au pas de la montre. Ce qui etait fige sur ce fil ne l'est
/// plus.
pub fn couler_depuis(instant: f64) {
    let mut e = etat();
    let aleas_fixes = e.as_ref().is_some_and(|s| s.aleas_fixes);
    *e = Some(Simule { coule: Some((instant, Instant::now())), aleas_fixes });
    FIGE.set(None);
}

/// Fige l'instant pour la route de ce fil (le rejeu d'un geste, une route servie au banc) ; le temps qui coule, s'il y en
/// a un, reprend a `defiger`.
pub fn figer(instant: f64) {
    FIGE.set(Some(instant));
    // LES ALEAS DE CHAQUE GESTE repartent de zero a son instant : un geste tire la meme suite au banc et au rejeu,
    // quelles que soient les lectures (que le rejeu ecarte) tirees avant lui.
    TIRAGES.set(0);
}

/// Le temps ne reste plus fige sur ce fil : celui qui coule reprend (le banc) ; sans lui, c'est encore l'instant fige.
pub fn defiger() {
    if etat().as_ref().is_some_and(|s| s.coule.is_some()) {
        FIGE.set(None);
    }
}

/// `f`, le temps fige a `instant` sur ce fil ; puis le temps redevient ce qu'il etait (la montre, le banc qui coule,
/// l'instant du geste rejoue), les aleas poursuivant leur suite. Un geste joue a sa date par l'horloge d'une compagnie
/// fictive.
pub fn pendant<T>(instant: f64, f: impl FnOnce() -> T) -> T {
    let avant = FIGE.replace(Some(instant));
    let r = f();
    FIGE.set(avant);
    r
}

/// LE REJEU commence : les aleas deviennent une suite fixe, a partir du premier tirage.
pub fn aleas_fixes() {
    let mut e = etat();
    match e.as_mut() {
        Some(s) => s.aleas_fixes = true,
        None => *e = Some(Simule { coule: None, aleas_fixes: true }),
    }
    TIRAGES.set(0);
}

/// Retour a la montre de la machine et aux aleas du systeme.
pub fn liberer() {
    *etat() = None;
    FIGE.set(None);
    TIRAGES.set(0);
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
    let tirage = aleas_fixes_poses().then(|| {
        let rang = TIRAGES.get() + 1;
        TIRAGES.set(rang);
        (rang, FIGE.get().map_or(0, f64::to_bits))
    });
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
    if aleas_fixes_poses() {
        std::time::Duration::ZERO
    } else {
        debut.elapsed()
    }
}

/// Le numero du processus, pour un identifiant (un `Message-ID`) : au rejeu, toujours 1.
pub fn numero_de_processus() -> u32 {
    if aleas_fixes_poses() {
        1
    } else {
        std::process::id()
    }
}
