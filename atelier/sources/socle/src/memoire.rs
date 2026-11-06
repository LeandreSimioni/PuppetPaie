//! CE QUE LE PROCESSUS GARDE EN MEMOIRE (un cache, l'etat du temps simule, les echecs) sous un `Mutex` : un fil qui a
//! panique en le tenant ne le rend pas illisible aux autres, ce qu'il garde se lit tel quel.

use std::sync::{Mutex, MutexGuard};

/// Le `Mutex` tenu, empoisonne ou non.
pub fn tenu<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    match m.lock() {
        Ok(g) => g,
        Err(empoisonne) => empoisonne.into_inner(),
    }
}
