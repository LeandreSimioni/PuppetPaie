//! Le jour civil : pas de `chrono`, pas de fuseau, pas d'horloge. Un jour est un triplet (an, mois, jour) du calendrier
//! gregorien proleptique, des annees 1 a 9999.
//!
//! `lire` admet cinq formats, essayes dans un ordre fixe : `AAAA-MM-JJ`, `JJ/MM/AAAA`, `JJMMAAAA`, `AAAAMMJJ`,
//! `JJ-MM-AAAA`. Chaque champ admet sa forme courte (`2026-3-5`) ; le premier format qui lit la chaine entiere gagne,
//! et un format entame ne revient pas en arriere (`20260315` n'est pas lu en `JJMMAAAA`). Ce parcours est celui des
//! fichiers deja ecrits : une date lue aujourd'hui se lit comme elle s'est toujours lue.

use crate::num::Resultat;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Jour {
    pub an: i32,
    pub mois: u32,
    pub jour: u32,
}

const JOURS_PAR_MOIS: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

/// Le rang du 1970-01-01 a compter du 0001-01-01, qui vaut 1.
const ORDINAL_EPOQUE_UNIX: i64 = 719_163;

pub fn bissextile(an: i32) -> bool {
    (an % 4 == 0 && an % 100 != 0) || an % 400 == 0
}

/// `calendar.monthrange(an, mois)[1]`, pour un mois de 1 a 12.
pub fn jours_du_mois(an: i32, mois: u32) -> u32 {
    if mois == 2 && bissextile(an) {
        29
    } else {
        JOURS_PAR_MOIS[(mois - 1) as usize]
    }
}

impl Jour {
    /// `None` si le jour n'existe pas ; l'annee va de 1 a 9999.
    pub fn nouveau(an: i32, mois: u32, jour: u32) -> Option<Jour> {
        if !(1..=9999).contains(&an) || !(1..=12).contains(&mois) {
            return None;
        }
        if jour < 1 || jour > jours_du_mois(an, mois) {
            return None;
        }
        Some(Jour { an, mois, jour })
    }

    /// Une date ecrite dans l'un des cinq formats.
    pub fn lire(texte: &str) -> Resultat<Jour> {
        match Jour::du_texte(texte) {
            Some(j) => Ok(j),
            None => Err(format!("date illisible : {}", repr_texte(texte.trim()))),
        }
    }

    /// Le jour que `texte` ecrit, s'il en ecrit un (`lire` dit pourquoi il n'en ecrit pas) : une phrase qui cite un jour
    /// le dit en clair, et ce qui n'est pas une date tel qu'il est ecrit.
    pub fn du_texte(texte: &str) -> Option<Jour> {
        let s = texte.trim();
        // La forme qu'ecrit le moteur (`iso`), lue d'emblee : c'est celle que le premier format apparierait.
        let o = s.as_bytes();
        if o.len() == 10 && o[4] == b'-' && o[7] == b'-' && [0, 1, 2, 3, 5, 6, 8, 9].iter().all(|&i| o[i].is_ascii_digit()) {
            let n = |a: usize, b: usize| o[a..b].iter().fold(0u32, |n, c| n * 10 + u32::from(c - b'0'));
            if let Some(j) = Jour::nouveau(n(0, 4) as i32, n(5, 7), n(8, 10)) {
                return Some(j);
            }
        }
        let c: Vec<char> = s.chars().collect();
        FORMATS.iter().find_map(|format| lire_format(format, &c))
    }

    /// `AAAA-MM-JJ`.
    pub fn iso(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.an, self.mois, self.jour)
    }

    /// `JJ/MM/AAAA`.
    pub fn jj_mm_aaaa(&self) -> String {
        format!("{:02}/{:02}/{:04}", self.jour, self.mois, self.an)
    }

    /// `JJ/MM`, sans l'annee.
    pub fn jj_mm(&self) -> String {
        format!("{:02}/{:02}", self.jour, self.mois)
    }

    /// `JJMMAAAA`.
    pub fn jjmmaaaa(&self) -> String {
        format!("{:02}{:02}{:04}", self.jour, self.mois, self.an)
    }

    /// `AAAAMMJJ`.
    pub fn aaaammjj(&self) -> String {
        format!("{:04}{:02}{:02}", self.an, self.mois, self.jour)
    }

    /// Le 0001-01-01 vaut 1. Formule des jours depuis
    /// l'epoque (annee commencant en mars, cycles de 400 ans).
    pub fn ordinal(&self) -> i64 {
        let a = i64::from(self.an) - i64::from(self.mois <= 2);
        let ere = a.div_euclid(400);
        let annee_d_ere = a - ere * 400;
        let m = i64::from(self.mois);
        let jour_d_annee = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(self.jour) - 1;
        let jour_d_ere = annee_d_ere * 365 + annee_d_ere / 4 - annee_d_ere / 100 + jour_d_annee;
        ere * 146_097 + jour_d_ere - 719_468 + ORDINAL_EPOQUE_UNIX
    }

    /// Le jour de rang `ordinal` (le 0001-01-01 vaut 1). `None` hors de 0001-01-01 .. 9999-12-31.
    pub fn depuis_ordinal(ordinal: i64) -> Option<Jour> {
        let z = ordinal - ORDINAL_EPOQUE_UNIX + 719_468;
        let ere = z.div_euclid(146_097);
        let jour_d_ere = z - ere * 146_097;
        let annee_d_ere = (jour_d_ere - jour_d_ere / 1460 + jour_d_ere / 36_524 - jour_d_ere / 146_096) / 365;
        let jour_d_annee = jour_d_ere - (365 * annee_d_ere + annee_d_ere / 4 - annee_d_ere / 100);
        let mp = (5 * jour_d_annee + 2) / 153;
        let jour = (jour_d_annee - (153 * mp + 2) / 5 + 1) as u32;
        let mois = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let an = annee_d_ere + ere * 400 + i64::from(mois <= 2);
        if !(1..=9999).contains(&an) {
            return None;
        }
        Jour::nouveau(an as i32, mois, jour)
    }

    /// Le jour du `n`-ieme jour apres l'epoque Unix (1970-01-01), pour l'horloge.
    pub fn depuis_jours_unix(jours: i64) -> Option<Jour> {
        Jour::depuis_ordinal(jours + ORDINAL_EPOQUE_UNIX)
    }

    /// `date + timedelta(days=n)`, refuse hors des annees 1 a 9999.
    pub fn plus_jours(&self, n: i64) -> Resultat<Jour> {
        Jour::depuis_ordinal(self.ordinal() + n).ok_or_else(|| HORS_CALENDRIER.to_string())
    }

    /// `(self - autre).days`.
    pub fn moins(&self, autre: &Jour) -> i64 {
        self.ordinal() - autre.ordinal()
    }

    pub fn jours_du_mois(&self) -> u32 {
        jours_du_mois(self.an, self.mois)
    }

    /// 0 = lundi ... 6 = dimanche.
    pub fn jour_de_la_semaine(&self) -> u32 {
        ((self.ordinal() + 6) % 7) as u32
    }

    /// Le mois civil du jour.
    pub fn mois_civil(&self) -> crate::mois::Mois {
        crate::mois::Mois::du_jour(self)
    }
}

/// Les refus d'une date qui n'existe pas, avec l'annee, le mois ou la date en cause.
pub fn annee_impossible(an: impl std::fmt::Display) -> String {
    format!("année impossible : {}", an)
}

use crate::mois::mois_impossible;

pub fn jour_impossible(an: impl Into<i64>, mois: impl Into<i64>, jour: impl Into<i64>) -> String {
    format!("date impossible : {:04}-{:02}-{:02}", an.into(), mois.into(), jour.into())
}

/// Ce qui rend une date impossible : l'annee d'abord, sinon le mois, sinon le jour.
pub fn date_impossible(an: impl Into<i64>, mois: impl Into<i64>, jour: impl Into<i64>) -> String {
    let (an, mois, jour) = (an.into(), mois.into(), jour.into());
    if !(1..=9999).contains(&an) {
        annee_impossible(an)
    } else if !(1..=12).contains(&mois) {
        mois_impossible(mois)
    } else {
        jour_impossible(an, mois, jour)
    }
}

pub const HORS_CALENDRIER: &str = "date hors du calendrier (années 1 à 9999)";


// ------------------------------------------------------------ la lecture d'une date

#[derive(Clone, Copy)]
enum Champ {
    An,
    Mois,
    Jour,
    Lit(char),
}

use Champ::*;

/// `%Y-%m-%d`, `%d/%m/%Y`, `%d%m%Y`, `%Y%m%d`, `%d-%m-%Y`, dans cet ordre.
const FORMATS: [&[Champ]; 5] = [
    &[An, Lit('-'), Mois, Lit('-'), Jour],
    &[Jour, Lit('/'), Mois, Lit('/'), An],
    &[Jour, Mois, An],
    &[An, Mois, Jour],
    &[Jour, Lit('-'), Mois, Lit('-'), An],
];

fn chiffre(c: &[char], i: usize) -> bool {
    c.get(i).is_some_and(|x| x.is_ascii_digit())
}

/// Les longueurs que le champ peut avaler en `pos`, dans l'ordre des
/// formes admises : la plus longue d'abord.
fn longueurs(champ: Champ, c: &[char], pos: usize) -> Vec<usize> {
    let a = c.get(pos).copied();
    let b = c.get(pos + 1).copied();
    let mut l = Vec::new();
    match champ {
        An => {
            if (0..4).all(|k| chiffre(c, pos + k)) {
                l.push(4);
            }
        }
        // 1[0-2] | 0[1-9] | [1-9]
        Mois => {
            if a == Some('1') && matches!(b, Some('0'..='2')) {
                l.push(2);
            }
            if a == Some('0') && matches!(b, Some('1'..='9')) {
                l.push(2);
            }
            if matches!(a, Some('1'..='9')) {
                l.push(1);
            }
        }
        // 3[01] | [12]\d | 0[1-9] | [1-9] | espace[1-9]
        Jour => {
            if a == Some('3') && matches!(b, Some('0'..='1')) {
                l.push(2);
            }
            if matches!(a, Some('1'..='2')) && chiffre(c, pos + 1) {
                l.push(2);
            }
            if a == Some('0') && matches!(b, Some('1'..='9')) {
                l.push(2);
            }
            if matches!(a, Some('1'..='9')) {
                l.push(1);
            }
            if a == Some(' ') && matches!(b, Some('1'..='9')) {
                l.push(2);
            }
        }
        Lit(x) => {
            if a == Some(x) {
                l.push(1);
            }
        }
    }
    l
}

/// Le nombre de `len` caracteres de `c` a partir de `de`, ses blancs autour retires ; 0 quand ce n'est pas un nombre.
fn nombre(c: &[char], de: usize, len: usize) -> u32 {
    let t: String = c[de..de + len].iter().collect();
    let t = t.trim();
    if t.is_empty() || !t.bytes().all(|b| b.is_ascii_digit()) {
        return 0;
    }
    t.bytes().fold(0u32, |n, b| n.saturating_mul(10).saturating_add(u32::from(b - b'0')))
}

/// Premier appariement de l'expression reguliere (avec retour sur les
/// alternatives), sans exiger la fin de chaine.
fn apparier(format: &[Champ], c: &[char], pos: usize, vus: &mut [u32; 3]) -> Option<usize> {
    let Some((champ, reste)) = format.split_first() else {
        return Some(pos);
    };
    for len in longueurs(*champ, c, pos) {
        let ancien = *vus;
        match champ {
            An => vus[0] = nombre(c, pos, len),
            Mois => vus[1] = nombre(c, pos, len),
            Jour => vus[2] = nombre(c, pos, len),
            Lit(_) => {}
        }
        if let Some(fin) = apparier(reste, c, pos + len, vus) {
            return Some(fin);
        }
        *vus = ancien;
    }
    None
}

/// Une date dans un format ; `None` s'il ne s'y lit pas (pas
/// d'appariement, reste de chaine, jour inexistant).
fn lire_format(format: &[Champ], c: &[char]) -> Option<Jour> {
    let mut vus = [0u32; 3];
    let fin = apparier(format, c, 0, &mut vus)?;
    if fin != c.len() {
        return None;
    }
    Jour::nouveau(vus[0] as i32, vus[1], vus[2])
}

/// Une date `AAAAMMJJ`, ou `None`.
pub fn lire_aaaammjj(s: &str) -> Option<Jour> {
    let c: Vec<char> = s.chars().collect();
    lire_format(FORMATS[3], &c)
}

// ------------------------------------------------------ les formes ecrites

/// Un texte cite dans un message : entre guillemets simples, sauf s'il porte une apostrophe
/// et aucun guillemet double.
pub fn repr_texte(s: &str) -> String {
    let guillemet = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut o = String::new();
    o.push(guillemet);
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if c == guillemet => {
                o.push('\\');
                o.push(c);
            }
            c if (c as u32) < 0x20 || c as u32 == 0x7f => o.push_str(&format!("\\x{:02x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push(guillemet);
    o
}

/// Un flottant ecrit : le plus court qui revient au meme flottant,
/// `1.0` pour un entier, l'exposant des 1e16 et des 1e-5.
pub fn repr_flottant(x: f64) -> String {
    if x.is_nan() {
        return "nan".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "inf" } else { "-inf" }.into();
    }
    let a = x.abs();
    if a != 0.0 && !(1e-4..1e16).contains(&a) {
        // `{:e}` donne `1e-5` ; la forme ecrite est `1e-05`.
        let t = format!("{:e}", x);
        let (m, e) = crate::ecriture::scientifique(&t);
        return format!("{}e{}{:02}", m, if e < 0 { '-' } else { '+' }, e.abs());
    }
    let t = format!("{}", x);
    if t.contains('.') {
        t
    } else {
        format!("{}.0", t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn j(an: i32, mois: u32, jour: u32) -> Jour {
        Jour::nouveau(an, mois, jour).unwrap()
    }

    #[test]
    fn ajouter_et_soustraire() {
        let x = j(2026, 1, 31);
        assert_eq!(x.plus_jours(1).unwrap(), j(2026, 2, 1));
        assert_eq!(x.plus_jours(-31).unwrap(), j(2025, 12, 31));
        assert_eq!(j(2024, 2, 28).plus_jours(1).unwrap(), j(2024, 2, 29));
        assert_eq!(j(2026, 7, 1).moins(&j(2026, 6, 30)), 1);
        assert_eq!(j(2026, 1, 1).moins(&j(2025, 1, 1)), 365);
        assert_eq!(j(9999, 12, 31).plus_jours(1).unwrap_err(), HORS_CALENDRIER);
        assert_eq!(j(1, 1, 1).plus_jours(-1).unwrap_err(), HORS_CALENDRIER);
    }

}

/// `AAAA-MM-JJ` ou `AAAAMMJJ`.
pub fn jour_iso(texte: &str) -> Option<Jour> {
    let chiffres = |t: &str| t.chars().all(|c| c.is_ascii_digit());
    let (a, m, j) = match texte.len() {
        10 if texte.is_char_boundary(4) && texte.is_char_boundary(7) && &texte[4..5] == "-" && &texte[7..8] == "-" => (&texte[..4], &texte[5..7], &texte[8..10]),
        8 if texte.is_ascii() => (&texte[..4], &texte[4..6], &texte[6..8]),
        _ => return None,
    };
    if !(chiffres(a) && chiffres(m) && chiffres(j)) {
        return None;
    }
    // Des chiffres seuls, quatre ou deux : leur valeur se compte, sans lecture qui puisse echouer.
    let valeur = |t: &str| t.bytes().fold(0u32, |n, c| n * 10 + u32::from(c - b'0'));
    Jour::nouveau(valeur(a) as i32, valeur(m), valeur(j))
}

/// `JJ/MM/AAAA` d'un `AAAA-MM-JJ` (ou d'un instant, par ses dix premiers caracteres) ; le texte tel quel s'il ne se lit pas.
pub fn jj_mm_aaaa(iso: &str) -> String {
    match jour_iso(&iso.chars().take(10).collect::<String>()) {
        Some(j) => j.jj_mm_aaaa(),
        None => iso.to_string(),
    }
}

/// UN JOUR DANS UNE DONNEE LUE (`serde`) : la forme qu'ecrit le moteur, `AAAA-MM-JJ`, et elle seule ; il s'ecrit de meme.
/// Les autres formats de `lire` sont ceux d'une saisie ou d'une piece, qui se lisent avant d'entrer dans une donnee.
impl serde::Serialize for Jour {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.iso())
    }
}

impl<'de> serde::Deserialize<'de> for Jour {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Jour, D::Error> {
        let s = String::deserialize(d)?;
        match Jour::lire(&s) {
            Ok(j) if j.iso() == s => Ok(j),
            _ => Err(serde::de::Error::custom(format!("{} n'est pas une date AAAA-MM-JJ", repr_texte(&s)))),
        }
    }
}
