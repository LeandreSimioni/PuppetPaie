//! L'arithmetique de la paie (`eur`, `pct`, les arrondis) et la lecture d'un nombre.
//!
//! Un flottant lu ne se convertit jamais directement en decimal : il passe par sa representation la plus courte
//! (`format!("{}", f64)`, a la forme pres : `24` contre `24.0`, jamais la valeur). `dec` lit donc `24.1` comme le
//! decimal 24.1, et non comme le binaire le plus proche.

use rust_decimal::prelude::*;
use crate::d28::P;
pub use rust_decimal::Decimal;
use serde_json::Value;

/// Un refus du moteur : le message dit quoi faire, comme celui du Python.
pub type Resultat<T> = Result<T, String>;

/// `Decimal(str(x))` pour un nombre JSON (entier ou flottant) ou une chaine.
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

/// `Decimal(str(x))` pour un flottant Python.
pub fn dec_f64(x: f64) -> Resultat<Decimal> {
    if !x.is_finite() {
        return Err(format!("nombre non fini : {}", x));
    }
    // `str(1.0)` vaut "1.0" en Python : un flottant entier garde sa decimale.
    let mut s = format!("{}", x);
    if !s.contains(['.', 'e', 'E']) && x.abs() < 1e16 {
        s.push_str(".0");
    }
    Decimal::from_str(&s)
        .or_else(|_| Decimal::from_scientific(&s))
        .map_err(|_| format!("nombre hors de portee : {}", s))
}

/// `Decimal(str(x))` : un flottant garde sa forme Python (`130.0`).
pub fn dec_py(v: &Value) -> Resultat<Decimal> {
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

/// `float(x)` d'une valeur JSON.
pub fn flottant(v: &Value) -> Resultat<f64> {
    match v {
        Value::Number(n) => n.as_f64().ok_or_else(|| format!("nombre illisible : {}", n)),
        Value::String(s) => s.trim().parse::<f64>().map_err(|_| illisible(s)),
        autre => Err(format!("nombre attendu, lu {}", autre)),
    }
}

/// `util.eur` : deux decimales, la moitie toujours vers le haut (jamais au pair).
pub fn eur(x: Decimal) -> Decimal {
    x.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}

/// `util.pct` : un taux en pourcentage applique, arrondi au centime.
pub fn pct(base: Decimal, taux: Decimal) -> Decimal {
    eur(div(mul(base, taux), Decimal::ONE_HUNDRED))
}

/// `round(x, 3)` de Python sur un flottant (arrondi correct, au pair sur le
/// demi exact) : passe par la chaine, comme CPython.
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

/// `a * b` de `decimal` : 28 chiffres significatifs, au pair sur le demi.
pub fn mul(a: Decimal, b: Decimal) -> Decimal {
    en_decimal(P::depuis(a).mul(P::depuis(b)))
}

/// `a / b` de `decimal`.
pub fn div(a: Decimal, b: Decimal) -> Decimal {
    en_decimal(P::depuis(a).div(P::depuis(b)))
}

/// `a + b` de `decimal`.
pub fn add(a: Decimal, b: Decimal) -> Decimal {
    en_decimal(P::depuis(a).add(P::depuis(b)))
}

/// `a - b` de `decimal`.
pub fn sub(a: Decimal, b: Decimal) -> Decimal {
    en_decimal(P::depuis(a).sub(P::depuis(b)))
}

thread_local! {
    /// Pose : `nombre` garde l'echelle du nombre (`48.0`, `0.3230`), comme l'ecrivent les valeurs d'un contrat et le
    /// calcul que sert `service::calculer_avec` ; sinon il l'ecrit sous sa forme la plus courte.
    pub static FIDELE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Un montant en texte : sa forme la plus courte (`48`, `0.323`), ou son echelle quand `FIDELE` est pose ; « -0 »
/// s'ecrit « 0 ».
pub fn nombre(x: Decimal) -> String {
    let s = if FIDELE.with(|f| f.get()) { x.to_string() } else { x.normalize().to_string() };
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn eur_arrondit_le_demi_vers_le_haut() {
        assert_eq!(eur(Decimal::from_str("2.675").unwrap()).to_string(), "2.68");
        assert_eq!(eur(Decimal::from_str("258.336").unwrap()).to_string(), "258.34");
        assert_eq!(eur(Decimal::from_str("-0.005").unwrap()).to_string(), "-0.01");
    }

    #[test]
    fn dec_passe_par_la_representation_la_plus_courte() {
        assert_eq!(dec(&json!(1.115)).unwrap().to_string(), "1.115");
        assert_eq!(dec(&json!(0.9825)).unwrap().to_string(), "0.9825");
        assert_eq!(dec(&json!(13)).unwrap().to_string(), "13");
        assert_eq!(dec(&json!(0.00001)).unwrap(), Decimal::from_str("0.00001").unwrap());
        assert_eq!(dec(&json!("5.23")).unwrap().to_string(), "5.23");
    }

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    // Attendus relevés dans Python (`decimal`, 28 chiffres significatifs).
    #[test]
    fn div_et_mul_sans_double_arrondi() {
        // 97/31 = 3.129032258064516129032258065 ; arrondi deux fois (28 decimales
        // puis 28 chiffres), le quotient devenait ...064 et le total 68,38.
        let q = div(d("97"), d("31"));
        assert_eq!(q.to_string(), "3.129032258064516129032258065");
        let m = mul(q, d("21.855"));
        assert_eq!(m.to_string(), "68.38500000000000000000000001");
        assert_eq!(eur(m).to_string(), "68.39");
    }

    #[test]
    fn div_a_plus_de_28_decimales() {
        // Python : 1/120 = 0.008333333333333333333333333333 (30 decimales) ;
        // Decimal n'en porte que 28, la valeur est ramenee a 0.0083333333333333333333333333.
        assert_eq!(div(d("1"), d("120")).to_string(), "0.0083333333333333333333333333");
        assert_eq!(div(d("10000"), d("12")).to_string(), "833.3333333333333333333333333");
        assert_eq!(div(d("1"), d("3")).to_string(), "0.3333333333333333333333333333");
        // 100 / 0.5 = 2.0E+2 : la valeur 200.
        assert_eq!(div(d("100"), d("0.5")), d("200"));
        assert_eq!(div(d("1.50"), d("3")).to_string(), "0.50");
    }

}
