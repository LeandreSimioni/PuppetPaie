//! L'arithmetique de la paie (`eur`, `pct`, les arrondis) et la lecture d'un nombre.
//!
//! Un flottant lu ne se convertit jamais directement en decimal : il passe par sa representation la plus courte
//! (`format!("{}", f64)`, a la forme pres : `24` contre `24.0`, jamais la valeur). `dec` lit donc `24.1` comme le
//! decimal 24.1, et non comme le binaire le plus proche.

use rust_decimal::prelude::*;
use crate::d28::P;
pub use rust_decimal::Decimal;
use serde_json::Value;

/// Un refus du moteur : le message dit quoi faire.
pub type Resultat<T> = Result<T, String>;

/// Le decimal d'un nombre JSON (entier ou flottant, par sa forme ecrite la plus courte) ou d'une chaine.
pub fn dec(v: &Value) -> Resultat<Decimal> {
    match v {
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Decimal::from(i))
            } else if let Some(u) = n.as_u64() {
                Ok(Decimal::from(u))
            } else {
                dec_f64(n.as_f64().unwrap_or(f64::NAN))
            }
        }
        Value::String(s) => Decimal::from_str(s.trim())
            .or_else(|_| Decimal::from_scientific(s.trim()))
            .map_err(|_| illisible(s)),
        autre => Err(format!("nombre attendu, lu {}", autre)),
    }
}

/// Le refus d'un nombre qui ne se lit pas, avec le texte en cause.
pub fn illisible(t: &str) -> String {
    format!("nombre illisible : {}", crate::fichiers::repr(t))
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

/// Le decimal d'un nombre JSON dont un flottant garde son echelle (`130.0`).
pub fn dec_avec_echelle(v: &Value) -> Resultat<Decimal> {
    match v {
        Value::Number(n) if n.as_i64().is_none() && n.as_u64().is_none() => {
            let s = crate::date::repr_flottant(n.as_f64().unwrap_or(f64::NAN));
            Decimal::from_str(&s)
                .or_else(|_| Decimal::from_scientific(&s))
                .map_err(|_| format!("nombre hors de portee : {}", s))
        }
        autre => dec(autre),
    }
}

/// Le flottant d'une valeur JSON (un nombre, ou un nombre ecrit en texte).
pub fn flottant(v: &Value) -> Resultat<f64> {
    match v {
        Value::Number(n) => n.as_f64().ok_or_else(|| format!("nombre illisible : {}", n)),
        Value::String(s) => s.trim().parse::<f64>().map_err(|_| illisible(s)),
        autre => Err(format!("nombre attendu, lu {}", autre)),
    }
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
    format!("{:.3}", x).parse().unwrap_or(x)
}

/// Une somme de montants, a deux decimales comme `sum(..., Decimal("0.00"))`.
pub fn somme<I: IntoIterator<Item = Decimal>>(it: I) -> Decimal {
    it.into_iter().fold(Decimal::new(0, 2), add)
}

/// `eur` avec ses deux decimales toujours ecrites (`quantize`).
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
    Decimal::try_from_i128_with_scale(m, e.max(0) as u32).unwrap_or(Decimal::MAX)
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

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    // L'arithmetique a 28 chiffres significatifs (`d28`).
    #[test]
    fn div_et_mul_sans_double_arrondi() {
        // 97/31 = 3.129032258064516129032258065 ; arrondi deux fois (28 decimales
        // puis 28 chiffres), le quotient serait ...064 et le total 68,38.
        let q = div(d("97"), d("31"));
        assert_eq!(q.to_string(), "3.129032258064516129032258065");
        let m = mul(q, d("21.855"));
        assert_eq!(m.to_string(), "68.38500000000000000000000001");
        assert_eq!(eur(m).to_string(), "68.39");
    }

}

/// `int(texte)` : le signe, les chiffres, un tiret bas entre deux chiffres, les blancs autour.
/// Python n'a pas de plafond : un entier trop long pour `i128` se rend au plus grand.
pub fn entier_lu(t: &str) -> Option<i128> {
    let t = t.trim_matches(crate::texte::blanc);
    let (neg, reste) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    if reste.is_empty() || reste.starts_with('_') || reste.ends_with('_') || reste.contains("__") {
        return None;
    }
    let chiffres: String = reste.chars().filter(|c| *c != '_').collect();
    if !chiffres.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let n = chiffres.parse::<i128>().unwrap_or(i128::MAX);
    Some(if neg { -n } else { n })
}

/// `str(d.quantize(Decimal("0.01")))` : deux decimales, la moitie au pair.
pub fn q2(d: Decimal) -> String {
    let mut r = d.round_dp(2);
    r.rescale(2);
    r.to_string()
}

/// `Decimal(texte)` : le signe, les chiffres avec un point et des tirets bas entre deux
/// chiffres, un exposant. `None` quand Python leve `InvalidOperation`.
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
    propre.push_str(if entier.is_empty() { "0" } else { &entier });
    if !frac.is_empty() {
        propre.push('.');
        propre.push_str(&frac);
    }
    if exposant.is_empty() {
        Decimal::from_str(&propre).ok()
    } else {
        Decimal::from_scientific(&format!("{}e{}", propre, exposant)).ok()
    }
}

