//! L'arithmetique decimale a 28 chiffres significatifs, au pair sur le demi, apres chaque operation, avec un exposant
//! qui peut depasser 28 decimales (`0.08333333333333333333333333334`), ce que `rust_decimal` ne sait pas porter : celle
//! de toute la paie (`num::add`, `sub`, `mul`, `div` passent par elle) et des clauses des conventions (`droit::clauses`).

use rust_decimal::Decimal;
use std::cmp::Ordering;
use std::fmt;

/// `m * 10^-e`.
#[derive(Clone, Copy, Debug)]
pub struct P {
    pub m: i128,
    pub e: i32,
}

fn chiffres(m: i128) -> u32 {
    m.unsigned_abs().checked_ilog10().map_or(1, |l| l + 1)
}

fn dix(k: u32) -> Option<i128> {
    10i128.checked_pow(k)
}

/// Arrondit a 28 chiffres significatifs, au pair sur le demi.
fn arrondi28(m: i128, e: i32) -> P {
    let n = chiffres(m);
    if n <= 28 {
        return P { m, e };
    }
    let perte = n - 28;
    let p = 10i128.pow(perte);
    let (mut q, r) = (m / p, (m % p).abs());
    if 2 * r > p || (2 * r == p && q % 2 != 0) {
        q += m.signum();
    }
    let (mut q, mut e) = (q, e - perte as i32);
    if q.abs() == 10i128.pow(28) {
        q /= 10;
        e -= 1;
    }
    P { m: q, e }
}

/// Le produit de deux entiers de 94 bits au plus, en chiffres decimaux.
fn produit_large(a: u128, b: u128) -> String {
    let (a0, a1) = (a as u64 as u128, a >> 64);
    let (b0, b1) = (b as u64 as u128, b >> 64);
    let (p00, p01, p10, p11) = (a0 * b0, a0 * b1, a1 * b0, a1 * b1);
    let mid = p01 + p10 + (p00 >> 64);
    let haut = p11 + (mid >> 64);
    let mut l = [p00 as u64, mid as u64, haut as u64, (haut >> 64) as u64];
    let mut morceaux = Vec::new();
    while l.iter().any(|x| *x != 0) {
        let mut reste: u128 = 0;
        for i in (0..4).rev() {
            let cur = (reste << 64) | l[i] as u128;
            l[i] = (cur / 10_000_000_000_000_000_000u128) as u64;
            reste = cur % 10_000_000_000_000_000_000u128;
        }
        morceaux.push(reste as u64);
    }
    let mut s = String::new();
    for (i, m) in morceaux.iter().rev().enumerate() {
        if i == 0 {
            s.push_str(&m.to_string());
        } else {
            s.push_str(&format!("{:019}", m));
        }
    }
    s
}

/// Deux nombres a la meme echelle (la plus fine qui tienne).
fn aligner(a: P, b: P) -> (i128, i128, i32) {
    let mut e = a.e.max(b.e);
    loop {
        let mise = |x: P| -> Option<i128> {
            if x.e <= e {
                x.m.checked_mul(dix((e - x.e) as u32)?)
            } else {
                Some(reduire(x, e).m)
            }
        };
        if let (Some(x), Some(y)) = (mise(a), mise(b)) {
            return (x, y, e);
        }
        e -= 1;
    }
}

/// Ramene `x` a l'echelle `e` (plus grossiere), au pair sur le demi.
fn reduire(x: P, e: i32) -> P {
    let p = 10i128.pow((x.e - e) as u32);
    let (mut q, r) = (x.m / p, (x.m % p).abs());
    if 2 * r > p || (2 * r == p && q % 2 != 0) {
        q += x.m.signum();
    }
    P { m: q, e }
}

impl P {
    pub const ZERO: P = P { m: 0, e: 0 };

    pub fn entier(n: i64) -> P {
        P { m: n as i128, e: 0 }
    }

    pub fn depuis(d: Decimal) -> P {
        P { m: d.mantissa(), e: d.scale() as i32 }
    }

    /// En `Decimal`, quand l'echelle y tient.
    pub fn en_decimal(self) -> Option<Decimal> {
        (0..=28).contains(&self.e).then(|| Decimal::from_i128_with_scale(self.m, self.e as u32))
    }

    pub fn add(self, o: P) -> P {
        let (x, y, e) = aligner(self, o);
        match x.checked_add(y) {
            Some(m) => arrondi28(m, e),
            None => arrondi28(x / 10 + y / 10, e - 1),
        }
    }

    pub fn sub(self, o: P) -> P {
        self.add(P { m: -o.m, e: o.e })
    }

    pub fn mul(self, o: P) -> P {
        match self.m.checked_mul(o.m) {
            Some(m) => arrondi28(m, self.e + o.e),
            None => {
                let s = produit_large(self.m.unsigned_abs(), o.m.unsigned_abs());
                let perte = s.len() - 28;
                let (tete, queue) = s.split_at(28);
                let mut q: i128 = tete.parse().unwrap();
                let (premier, suite) = (queue.as_bytes()[0], &queue[1..]);
                if premier > b'5' || (premier == b'5' && (suite.bytes().any(|c| c != b'0') || q % 2 != 0)) {
                    q += 1;
                }
                let mut e = self.e + o.e - perte as i32;
                if q == 10i128.pow(28) {
                    q /= 10;
                    e -= 1;
                }
                P { m: q * (self.m.signum() * o.m.signum()), e }
            }
        }
    }

    /// `self / o` : un dividende d'exposant plus grossier que le diviseur est
    /// d'abord mis a l'echelle (`100 / 0.5` rend `200`, la valeur de `2E+2`).
    pub fn div(self, o: P) -> P {
        let (mut ma, mb) = (self.m.abs(), o.m.abs());
        let mut s = self.e - o.e;
        while s < 0 {
            ma *= 10;
            s += 1;
        }
        let (mut q, mut r) = (ma / mb, ma % mb);
        let compte = |q: i128| if q == 0 { 0 } else { chiffres(q) };
        while r != 0 && compte(q) < 28 {
            r *= 10;
            q = q * 10 + r / mb;
            r %= mb;
            s += 1;
        }
        if r != 0 && (2 * r > mb || (2 * r == mb && q % 2 != 0)) {
            q += 1;
            if q == 10i128.pow(28) {
                q /= 10;
                s -= 1;
            }
        }
        arrondi28(q * (self.m.signum() * o.m.signum()), s)
    }

    /// Arrondi à `dp` décimales, au pair sur le demi.
    pub fn quantifier(self, dp: i32) -> P {
        if self.e > dp {
            reduire(self, dp)
        } else {
            P { m: self.m * 10i128.pow((dp - self.e) as u32), e: dp }
        }
    }

    /// Deux decimales, la moitie vers le haut.
    pub fn eur2(self) -> P {
        if self.e > 2 {
            let p = 10i128.pow((self.e - 2) as u32);
            let (mut q, r) = (self.m / p, (self.m % p).abs());
            if 2 * r >= p {
                q += self.m.signum();
            }
            P { m: q, e: 2 }
        } else {
            P { m: self.m * 10i128.pow((2 - self.e) as u32), e: 2 }
        }
    }

    /// `to_integral_value(rounding=ROUND_FLOOR)`.
    pub fn plancher(self) -> P {
        if self.e <= 0 {
            return self;
        }
        P { m: self.m.div_euclid(10i128.pow(self.e as u32)), e: 0 }
    }

    pub fn est_entier(self) -> bool {
        self == self.plancher()
    }

    /// Le plus grand des deux ; le premier a egalite.
    pub fn max_premier(self, o: P) -> P {
        if o > self {
            o
        } else {
            self
        }
    }

    /// Le plus petit des deux ; le premier a egalite.
    pub fn min_premier(self, o: P) -> P {
        if o < self {
            o
        } else {
            self
        }
    }
}

impl PartialEq for P {
    fn eq(&self, o: &P) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl Eq for P {}
impl PartialOrd for P {
    fn partial_cmp(&self, o: &P) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for P {
    fn cmp(&self, o: &P) -> Ordering {
        let (x, y, _) = aligner(*self, *o);
        x.cmp(&y)
    }
}

/// Le décimal en texte.
impl fmt::Display for P {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let coeff = self.m.unsigned_abs().to_string();
        let exp = -(self.e as i64);
        let gauche = exp + coeff.len() as i64;
        let point = if exp <= 0 && gauche > -6 { gauche } else { 1 };
        let (entiere, fraction) = if point <= 0 {
            ("0".to_string(), format!(".{}{}", "0".repeat((-point) as usize), coeff))
        } else if point >= coeff.len() as i64 {
            (format!("{}{}", coeff, "0".repeat(point as usize - coeff.len())), String::new())
        } else {
            (coeff[..point as usize].to_string(), format!(".{}", &coeff[point as usize..]))
        };
        let echelle = if gauche == point { String::new() } else { format!("E{:+}", gauche - point) };
        let signe = if self.m < 0 { "-" } else { "" };
        write!(f, "{}{}{}{}", signe, entiere, fraction, echelle)
    }
}
