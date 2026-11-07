//! L'arithmetique de la paie (`eur`, `pct`, les arrondis) et la lecture d'un nombre.
//!
//! Un flottant lu ne se convertit jamais directement en decimal : il passe par sa representation la plus courte
//! (`format!("{}", f64)`, a la forme pres : `24` contre `24.0`, jamais la valeur). `dec` lit donc `24.1` comme le
//! decimal 24.1, et non comme le binaire le plus proche.

use rust_decimal::prelude::*;
use crate::d28::P;
pub use rust_decimal::Decimal;

/// Un refus du moteur : le message dit quoi faire.
pub type Resultat<T> = Result<T, String>;

/// Le decimal d'un nombre ecrit en texte (`"12.5"`, `" 1e3 "`), blancs autour ignores.
pub fn dec_texte(s: &str) -> Resultat<Decimal> {
    Decimal::from_str(s.trim()).or_else(|_| Decimal::from_scientific(s.trim())).map_err(|_| illisible(s))
}

/// Le refus d'un nombre qui ne se lit pas, avec le texte en cause.
pub fn illisible(t: &str) -> String {
    format!("nombre illisible : {}", crate::fichiers::repr(t))
}

/// UN ENTIER ECRIT EN CHIFFRES SEULS, de un a dix-huit : sa valeur, comptee chiffre a chiffre ; tout autre texte (vide, un
/// autre caractere, plus long) n'est pas un entier ainsi ecrit, et rend `None`.
pub fn entier_de_chiffres(t: &str) -> Option<i64> {
    let b = t.as_bytes();
    if b.is_empty() || b.len() > 18 || !b.iter().all(u8::is_ascii_digit) {
        return None;
    }
    Some(b.iter().fold(0i64, |n, c| n * 10 + i64::from(c - b'0')))
}

/// Le flottant d'un decimal, par sa forme ecrite (`d.to_string()` relu) : un decimal s'ecrit toujours en un nombre que
/// le flottant lit.
pub fn flottant(d: Decimal) -> f64 {
    match d.to_string().parse::<f64>() {
        Ok(x) => x,
        Err(e) => unreachable!("un décimal s'écrit en nombre : {} ({})", d, e),
    }
}

/// Le decimal d'un flottant, par sa forme ecrite la plus courte.
pub fn dec_f64(x: f64) -> Resultat<Decimal> {
    if !x.is_finite() {
        return Err(format!("nombre non fini : {}", x));
    }
    // Un flottant entier garde sa decimale : `1.0`.
    let mut s = format!("{}", x);
    if !s.contains(['.', 'e', 'E']) && x.abs() < 1e16 {
        s.push_str(".0");
    }
    Decimal::from_str(&s)
        .or_else(|_| Decimal::from_scientific(&s))
        .map_err(|_| format!("nombre hors de portee : {}", s))
}

/// Deux decimales, la moitie toujours vers le haut (jamais au pair).
pub fn eur(x: Decimal) -> Decimal {
    x.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}

/// Un taux en pourcentage applique, arrondi au centime.
pub fn pct(base: Decimal, taux: Decimal) -> Decimal {
    eur(div(mul(base, taux), Decimal::ONE_HUNDRED))
}

/// Un flottant a trois decimales (arrondi correct, au pair sur le
/// demi exact), par sa forme ecrite.
pub fn round3(x: f64) -> f64 {
    // Un flottant ecrit par Rust se relit toujours (`NaN` et `inf` compris).
    match format!("{:.3}", x).parse() {
        Ok(y) => y,
        Err(e) => unreachable!("un flottant écrit par Rust se relit : {} ({})", x, e),
    }
}

/// Une somme de montants, à deux décimales au moins.
pub fn somme<I: IntoIterator<Item = Decimal>>(it: I) -> Decimal {
    it.into_iter().fold(Decimal::new(0, 2), add)
}

/// `eur` avec ses deux décimales toujours écrites.
pub fn eur2(x: Decimal) -> Decimal {
    let mut e = eur(x);
    e.rescale(2);
    e
}

pub fn zero2() -> Decimal {
    eur2(Decimal::ZERO)
}

/// Un `P` (28 chiffres significatifs, exposant libre) en `Decimal`. Au-dela
/// de 28 decimales (`1/120`), `Decimal` ne tient pas : on ramene a 28 decimales,
/// au pair sur le demi (le seul second arrondi, inevitable). Un exposant
/// positif (`2E+2`) devient l'entier ; s'il depasse les 96 bits, on garde la
/// mantisse la plus juste que `Decimal` porte.
fn en_decimal(p: P) -> Decimal {
    let p = if p.e > 28 { p.quantifier(28) } else { p };
    if let Some(d) = p.en_decimal() {
        return d;
    }
    let (mut m, mut e) = (p.m, p.e);
    while e < 0 {
        match m.checked_mul(10) {
            Some(x) if x.unsigned_abs() < 1u128 << 96 => {
                m = x;
                e += 1;
            }
            _ => break,
        }
    }
    // Une mantisse au-dela de 96 bits, ou plus de 28 decimales : le plus grand decimal.
    if m.unsigned_abs() >= 1u128 << 96 || e > 28 {
        return Decimal::MAX;
    }
    Decimal::from_i128_with_scale(m, e.max(0) as u32)
}

/// `a * b` a 28 chiffres significatifs, au pair sur le demi (`d28`).
pub fn mul(a: Decimal, b: Decimal) -> Decimal {
    en_decimal(P::depuis(a).mul(P::depuis(b)))
}

/// `a / b` a 28 chiffres significatifs (`d28`).
pub fn div(a: Decimal, b: Decimal) -> Decimal {
    en_decimal(P::depuis(a).div(P::depuis(b)))
}

/// `a + b` a 28 chiffres significatifs (`d28`).
pub fn add(a: Decimal, b: Decimal) -> Decimal {
    en_decimal(P::depuis(a).add(P::depuis(b)))
}

/// `a - b` a 28 chiffres significatifs (`d28`).
pub fn sub(a: Decimal, b: Decimal) -> Decimal {
    en_decimal(P::depuis(a).sub(P::depuis(b)))
}

thread_local! {
    /// Pose : `nombre` garde l'echelle du nombre (`48.0`, `0.3230`), comme l'ecrivent les valeurs d'un contrat et le
    /// calcul que sert `service::calculer_avec` ; sinon il l'ecrit sous sa forme la plus courte.
    pub static ECHELLE_GARDEE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Un montant en texte : sa forme la plus courte (`48`, `0.323`), ou son echelle quand `ECHELLE_GARDEE` est pose ; « -0 »
/// s'ecrit « 0 ».
pub fn nombre(x: Decimal) -> String {
    let s = if ECHELLE_GARDEE.with(|f| f.get()) { x.to_string() } else { x.normalize().to_string() };
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eur_arrondit_le_demi_vers_le_haut() {
        assert_eq!(eur(Decimal::from_str("2.675").unwrap()).to_string(), "2.68");
        assert_eq!(eur(Decimal::from_str("258.336").unwrap()).to_string(), "258.34");
        assert_eq!(eur(Decimal::from_str("-0.005").unwrap()).to_string(), "-0.01");
    }

}

/// Un entier ecrit en texte : le signe, les chiffres, un tiret bas entre deux chiffres, les blancs autour.
/// Un entier trop long pour `i128` se rend au plus grand.
pub fn entier_lu(t: &str) -> Option<i128> {
    let t = t.trim_matches(crate::texte::blanc);
    let (neg, reste) = match (t.strip_prefix('-'), t.strip_prefix('+')) {
        (Some(r), _) => (true, r),
        (None, Some(r)) => (false, r),
        (None, None) => (false, t),
    };
    // Des groupes de chiffres, chacun non vide, separes d'un tiret bas.
    if !reste.split('_').all(|g| !g.is_empty() && g.chars().all(|c| c.is_ascii_digit())) {
        return None;
    }
    let lu = reste.chars().filter(|c| *c != '_').try_fold(0i128, |n, c| n.checked_mul(10)?.checked_add(i128::from(c.to_digit(10)?)));
    let n = match lu {
        Some(n) => n,
        None => i128::MAX,
    };
    Some(if neg { -n } else { n })
}

/// Deux décimales, la moitié au pair, en texte.
pub fn q2(d: Decimal) -> String {
    let mut r = d.round_dp(2);
    r.rescale(2);
    r.to_string()
}

/// Un decimal ecrit en texte : le signe, les chiffres avec un point et des tirets bas entre deux
/// chiffres, un exposant. `None` quand le texte n'est pas un nombre.
pub fn decimal_lu(t: &str) -> Option<Decimal> {
    let t = t.trim_matches(crate::texte::blanc);
    let c: Vec<char> = t.chars().collect();
    let mut i = 0;
    let mut propre = String::new();
    if i < c.len() && (c[i] == '+' || c[i] == '-') {
        if c[i] == '-' {
            propre.push('-');
        }
        i += 1;
    }
    // une suite de chiffres, avec des tirets bas seulement entre deux chiffres
    let suite = |i: &mut usize, sortie: &mut String| -> usize {
        let mut n = 0;
        while *i < c.len() {
            if c[*i].is_ascii_digit() {
                sortie.push(c[*i]);
                n += 1;
            } else if c[*i] == '_' && n > 0 && *i + 1 < c.len() && c[*i + 1].is_ascii_digit() {
            } else {
                break;
            }
            *i += 1;
        }
        n
    };
    let mut entier = String::new();
    let ne = suite(&mut i, &mut entier);
    let mut frac = String::new();
    let mut nf = 0;
    if i < c.len() && c[i] == '.' {
        i += 1;
        nf = suite(&mut i, &mut frac);
    }
    if ne + nf == 0 {
        return None;
    }
    let mut exposant = String::new();
    if i < c.len() && (c[i] == 'e' || c[i] == 'E') {
        i += 1;
        if i < c.len() && (c[i] == '+' || c[i] == '-') {
            exposant.push(c[i]);
            i += 1;
        }
        if suite(&mut i, &mut exposant) == 0 {
            return None;
        }
    }
    if i != c.len() {
        return None;
    }
    // LE DECIMAL QUE LE TEXTE ECRIT : sa mantisse (ses chiffres) et son echelle (ses decimales, moins l'exposant). Un
    // nombre bien ecrit que le decimal ne tient pas (une mantisse au-dela de 96 bits, plus de 28 decimales, un exposant
    // hors de sa portee) n'est pas un nombre qu'on lise.
    let chiffres = |s: &str| s.chars().try_fold(0i128, |n, c| n.checked_mul(10)?.checked_add(i128::from(c.to_digit(10)?)));
    let mut mantisse = chiffres(&format!("{}{}", entier, frac))?;
    let exposant = match exposant.strip_prefix('-') {
        Some(e) => -chiffres(e)?,
        None => chiffres(exposant.trim_start_matches('+'))?,
    };
    let mut echelle = (frac.len() as i128).checked_sub(exposant)?;
    if echelle < 0 {
        // Au-dela de 10^38, l'entier ne tient dans aucun decimal.
        if echelle < -38 {
            return None;
        }
        mantisse = mantisse.checked_mul(10i128.checked_pow((-echelle) as u32)?)?;
        echelle = 0;
    }
    if echelle > 28 || mantisse >= 1i128 << 96 {
        return None;
    }
    Some(Decimal::from_i128_with_scale(if propre == "-" { -mantisse } else { mantisse }, echelle as u32))
}

