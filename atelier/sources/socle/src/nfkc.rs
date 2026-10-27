//! La forme normale NFKC d'un texte, sans crate : les tables (`tables_nfkc.rs`) sont ecrites par
//! `paie generer-tables` (`generer_tables.rs`) depuis la base Unicode deposee (`atelier/reference/sources/unicode/`).
//!
//! Les trois temps de la norme (Unicode, chapitre 3, D65, D109 et D117 :
//! `atelier/reference/sources/unicode/unicode-norme-18.0.0-chapitre-3-extraits-20261015.md`) : decomposition de
//! compatibilite complete, mise en ordre canonique des signes combinants (tri stable par classe), puis composition
//! canonique. Les syllabes Hangul se decomposent et se recomposent par le calcul (§ 3.12), pas par la table.

use crate::tables_nfkc::{CLASSES, COMPOSITIONS, DECOMP_PLACE, DECOMP_POINTS, DECOMP_TEXTE};

const S_BASE: u32 = 0xAC00;
const L_BASE: u32 = 0x1100;
const V_BASE: u32 = 0x1161;
const T_BASE: u32 = 0x11A7;
const L_NB: u32 = 19;
const V_NB: u32 = 21;
const T_NB: u32 = 28;
const N_NB: u32 = V_NB * T_NB;
const S_NB: u32 = L_NB * N_NB;

/// La classe de combinaison d'un point de code (0 : un caractere de base).
fn classe(c: char) -> u8 {
    let cp = c as u32;
    if cp < 0x300 {
        return 0;
    }
    match CLASSES.binary_search_by(|&(a, b, _)| {
        if cp < a {
            std::cmp::Ordering::Greater
        } else if cp > b {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Equal
        }
    }) {
        Ok(i) => CLASSES[i].2,
        Err(_) => 0,
    }
}

fn decomposer(c: char, sortie: &mut Vec<char>) {
    let cp = c as u32;
    if cp < 0x80 {
        sortie.push(c);
    } else if (S_BASE..S_BASE + S_NB).contains(&cp) {
        let s = cp - S_BASE;
        sortie.push(char::from_u32(L_BASE + s / N_NB).expect("hangul"));
        sortie.push(char::from_u32(V_BASE + (s % N_NB) / T_NB).expect("hangul"));
        if s % T_NB != 0 {
            sortie.push(char::from_u32(T_BASE + s % T_NB).expect("hangul"));
        }
    } else if let Ok(i) = DECOMP_POINTS.binary_search(&cp) {
        let place = DECOMP_PLACE[i];
        let (debut, long) = ((place >> 7) as usize, (place & 127) as usize);
        sortie.extend(DECOMP_TEXTE[debut..debut + long].chars());
    } else {
        sortie.push(c);
    }
}

fn composer_deux(a: char, b: char) -> Option<char> {
    let (x, y) = (a as u32, b as u32);
    if (L_BASE..L_BASE + L_NB).contains(&x) && (V_BASE..V_BASE + V_NB).contains(&y) {
        return char::from_u32(S_BASE + ((x - L_BASE) * V_NB + (y - V_BASE)) * T_NB);
    }
    if (S_BASE..S_BASE + S_NB).contains(&x) && (x - S_BASE) % T_NB == 0 && (T_BASE + 1..T_BASE + T_NB).contains(&y) {
        return char::from_u32(x + (y - T_BASE));
    }
    COMPOSITIONS
        .binary_search_by(|&(p, s, _)| (p, s).cmp(&(x, y)))
        .ok()
        .and_then(|i| char::from_u32(COMPOSITIONS[i].2))
}

/// `unicodedata.normalize("NFKC", texte)`.
pub fn nfkc(texte: &str) -> String {
    if texte.is_ascii() {
        return texte.to_string();
    }
    let mut d: Vec<char> = Vec::with_capacity(texte.len());
    for c in texte.chars() {
        decomposer(c, &mut d);
    }
    // L'ordre canonique : dans chaque suite de signes combinants, tri stable par classe.
    let mut i = 0;
    while i < d.len() {
        if classe(d[i]) == 0 {
            i += 1;
            continue;
        }
        let mut j = i;
        while j < d.len() && classe(d[j]) != 0 {
            j += 1;
        }
        d[i..j].sort_by_key(|&c| classe(c));
        i = j;
    }
    // La composition.
    let mut sortie: Vec<char> = Vec::with_capacity(d.len());
    let mut base: Option<usize> = None;
    let mut derniere = 0u8;
    for c in d {
        let k = classe(c);
        if let Some(b) = base {
            let adjacent = sortie.len() == b + 1;
            if adjacent || derniere < k {
                if let Some(m) = composer_deux(sortie[b], c) {
                    sortie[b] = m;
                    continue;
                }
            }
        }
        if k == 0 {
            base = Some(sortie.len());
            derniere = 0;
        } else {
            derniere = k;
        }
        sortie.push(c);
    }
    sortie.into_iter().collect()
}
