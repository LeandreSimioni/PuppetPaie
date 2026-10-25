//! Le jour civil, ecrit nous-memes (PORTAGE-RUST §3) : pas de `chrono`, pas
//! de fuseau, pas d'horloge. Un jour est un triplet (an, mois, jour) du
//! calendrier gregorien proleptique, comme `datetime.date` de Python.
//!
//! `lire` reproduit `util.d` : les cinq formats, essayes dans le meme ordre,
//! avec le meme message d'erreur. Python passe par `strptime`, dont les
//! champs sont des expressions regulieres a alternatives ordonnees (`%d` :
//! `3[01]|[12]\d|0[1-9]|[1-9]| [1-9]`), et qui accepte donc `2026-3-5` ;
//! et le premier appariement gagne SANS retour une fois la chaine entamee
//! (`20260315` n'est pas lu en `JJMMAAAA`). On refait le meme parcours.

use serde_json::Value;

use crate::num::Resultat;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Jour {
    pub an: i32,
    pub mois: u32,
    pub jour: u32,
}

const JOURS_PAR_MOIS: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

/// Ordinal Python (`toordinal`) du 1970-01-01 : le 0001-01-01 vaut 1.
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
    /// `None` si le jour n'existe pas ; l'annee va de 1 a 9999 comme en Python.
    pub fn nouveau(an: i32, mois: u32, jour: u32) -> Option<Jour> {
        if !(1..=9999).contains(&an) || !(1..=12).contains(&mois) {
            return None;
        }
        if jour < 1 || jour > jours_du_mois(an, mois) {
            return None;
        }
        Some(Jour { an, mois, jour })
    }

    /// `util.d` sur une chaine.
    pub fn lire(texte: &str) -> Resultat<Jour> {
        let s = texte.trim();
        let c: Vec<char> = s.chars().collect();
        for format in FORMATS {
            if let Some(j) = lire_format(format, &c) {
                return Ok(j);
            }
        }
        Err(format!("date illisible : {}", repr_texte(s)))
    }

    /// `AAAA-MM-JJ`, ce que `isoformat()` et `str(date)` ecrivent.
    pub fn iso(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.an, self.mois, self.jour)
    }

    /// `AAAA-MM`, la cle d'un mois de paie.
    pub fn mois_iso(&self) -> String {
        format!("{:04}-{:02}", self.an, self.mois)
    }

    /// `util.jj_mm_aaaa`.
    pub fn jj_mm_aaaa(&self) -> String {
        format!("{:02}/{:02}/{:04}", self.jour, self.mois, self.an)
    }

    /// `util.jjmmaaaa`.
    pub fn jjmmaaaa(&self) -> String {
        format!("{:02}{:02}{:04}", self.jour, self.mois, self.an)
    }

    /// `util.aaaammjj`.
    pub fn aaaammjj(&self) -> String {
        format!("{:04}{:02}{:02}", self.an, self.mois, self.jour)
    }

    /// `date.toordinal()` : le 0001-01-01 vaut 1. Formule des jours depuis
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

    /// `date.fromordinal`. `None` hors de 0001-01-01 .. 9999-12-31.
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

    /// `date.weekday()` : 0 = lundi ... 6 = dimanche.
    pub fn jour_de_la_semaine(&self) -> u32 {
        ((self.ordinal() + 6) % 7) as u32
    }

    /// Le mois suivant, `AAAA-MM` (`Engagement.mois_du_contrat` incremente ainsi).
    pub fn mois_suivant(&self) -> String {
        suivant(self.an, self.mois)
    }
}

fn suivant(an: i32, mois: u32) -> String {
    if mois == 12 {
        format!("{:04}-01", an + 1)
    } else {
        format!("{:04}-{:02}", an, mois + 1)
    }
}

/// Les refus d'une date qui n'existe pas, avec l'annee, le mois ou la date en cause.
pub fn annee_impossible(an: impl std::fmt::Display) -> String {
    format!("année impossible : {}", an)
}

pub fn mois_impossible(mois: impl std::fmt::Display) -> String {
    format!("mois impossible : {}", mois)
}

pub fn jour_impossible(an: impl Into<i64>, mois: impl Into<i64>, jour: impl Into<i64>) -> String {
    format!("date impossible : {:04}-{:02}-{:02}", an.into(), mois.into(), jour.into())
}

/// Le numero du mois d'un « AAAA-MM », de 1 a 12 ; « 00 » (ou « 13 ») n'est aucun mois et se refuse, comme
/// `premier_du_mois`, `dernier_du_mois` et `envoi::etiquette` (lot F1). Le mois d'une paie est celui que sa DSN declare :
/// « le premier jour du mois civil au titre duquel est etablie la paie », de forme `(01)(0[1-9]|1[0-2])(20)[0-9]{2}`
/// (S20.G00.05.005, atelier/reference/norme-dsn/texte-2027/bloc-S20.G00.05.txt l. 122-129). La norme n'admet « 00 »
/// qu'au mois d'une date de naissance inconnue (S21.G00.30.006, bloc-S21.G00.30.txt l. 143-148), ce que `champs.rs` lit
/// du mois de naissance d'un NIR : jamais au mois d'une paie.
pub fn numero_du_mois(mois: &str) -> Resultat<u32> {
    let t: String = mois.chars().skip(5).take(2).collect();
    let n: i64 = t.trim().parse().map_err(|_| format!("nombre entier illisible : {}", repr_texte(&t)))?;
    if !(1..=12).contains(&n) {
        return Err(mois_impossible(n));
    }
    Ok(n as u32)
}

/// Les trois graphies d'un nom de mois que le moteur ecrit, chacune la ou elle s'ecrivait deja :
/// - `Titre` (« Aout ») : noms de fichiers (`DSN_Aout2026.dsn`), titres et sections des etats ;
/// - `Minuscule` (« aout ») : sans accent, dans une phrase (le libelle d'un mois d'engagement, d'un dossier) ;
/// - `Accentuee` (« août ») : dans une phrase en francais (courriels, contrats, avenants, ecrans).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Graphie {
    Titre,
    Minuscule,
    Accentuee,
}

const MOIS_TITRE: [&str; 12] = ["Janvier", "Fevrier", "Mars", "Avril", "Mai", "Juin", "Juillet", "Aout", "Septembre", "Octobre", "Novembre", "Decembre"];
const MOIS_MINUSCULE: [&str; 12] = ["janvier", "fevrier", "mars", "avril", "mai", "juin", "juillet", "aout", "septembre", "octobre", "novembre", "decembre"];
const MOIS_ACCENTUE: [&str; 12] = ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"];

/// Le nom du mois `mois` (1 a 12) dans la graphie `g` : la seule table des noms de mois du moteur. Un mois hors 1-12
/// n'est aucun mois et se refuse (`mois_impossible`), comme `numero_du_mois` : jamais l'indice -1 d'une liste (le Python
/// lisait « 00 » decembre) ni un mois ramene de force dans l'annee.
pub fn nom_du_mois(mois: i64, g: Graphie) -> Resultat<&'static str> {
    if !(1..=12).contains(&mois) {
        return Err(mois_impossible(mois));
    }
    let table = match g {
        Graphie::Titre => &MOIS_TITRE,
        Graphie::Minuscule => &MOIS_MINUSCULE,
        Graphie::Accentuee => &MOIS_ACCENTUE,
    };
    Ok(table[(mois - 1) as usize])
}

/// « octobre 2026 » d'un « AAAA-MM », dans la graphie `g` ; un mois illisible ou hors 01-12 se refuse.
pub fn libelle_du_mois(mois: &str, g: Graphie) -> Resultat<String> {
    let n = numero_du_mois(mois)?;
    Ok(format!("{} {}", nom_du_mois(n as i64, g)?, mois.chars().take(4).collect::<String>()))
}

/// `libelle_du_mois` pour une phrase qui ne peut pas refuser pour son mois (un bandeau, un conseil, un avertissement) :
/// un mois illisible ou hors 01-12 s'ecrit tel quel (« 2026-00 »), jamais sous le nom d'un autre mois.
pub fn libelle_du_mois_ou_tel_quel(mois: &str, g: Graphie) -> String {
    libelle_du_mois(mois, g).unwrap_or_else(|_| mois.to_string())
}

/// L'annee, sinon le mois, sinon le jour : ce que `datetime.date` refuse en premier.
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

/// `Engagement.debut_mois_de` : `d(mois + "-01")`.
pub fn premier_du_mois(mois: &str) -> Resultat<Jour> {
    Jour::lire(&format!("{}-01", mois))
}

/// `Engagement.fin_mois_de` : `int(mois[:4])`, `int(mois[5:7])`, puis
/// `calendar.monthrange`, qui refuse un mois hors de 1 a 12.
pub fn dernier_du_mois(mois: &str) -> Resultat<Jour> {
    let c: Vec<char> = mois.chars().collect();
    let tranche = |a: usize, b: usize| -> String { c.iter().skip(a).take(b - a).collect() };
    let entier = |t: String| -> Resultat<i64> {
        t.trim().parse::<i64>().map_err(|_| format!("nombre entier illisible : {}", repr_texte(&t)))
    };
    let an = entier(tranche(0, 4))?;
    let mo = entier(tranche(5, 7))?;
    if !(1..=12).contains(&mo) {
        return Err(mois_impossible(mo));
    }
    let an = i32::try_from(an).unwrap_or(0);
    Jour::lire(&format!("{}-{:02}-{:02}", an, mo, jours_du_mois(an.max(1), mo as u32)))
}

/// Les mois `AAAA-MM` de `debut` a `fin` inclus (`Engagement.mois_du_contrat`).
pub fn mois_entre(debut: &Jour, fin: &Jour) -> Vec<String> {
    let mut sortie = Vec::new();
    let (mut an, mut mo) = (debut.an, debut.mois);
    while (an, mo) <= (fin.an, fin.mois) {
        sortie.push(format!("{:04}-{:02}", an, mo));
        if mo == 12 {
            an += 1;
            mo = 1;
        } else {
            mo += 1;
        }
    }
    sortie
}

/// `util.d` sur une valeur JSON : une chaine, un nombre (`str(x)`), ou la
/// date detaillee `{"jour": ..., "lieu": ...}` qui se lit par son `jour`.
pub fn d(v: &Value) -> Resultat<Jour> {
    match v {
        Value::Object(o) => match o.get("jour") {
            Some(j) => d(j),
            None => Err(format!("date detaillee sans `jour` : {}", repr_py(v))),
        },
        Value::String(s) => Jour::lire(s),
        autre => Jour::lire(&en_texte(autre)),
    }
}

// ------------------------------------------------------------ le strptime

#[derive(Clone, Copy)]
enum Champ {
    An,
    Mois,
    Jour,
    Lit(char),
}

use Champ::*;

/// `%Y-%m-%d`, `%d/%m/%Y`, `%d%m%Y`, `%Y%m%d`, `%d-%m-%Y`, dans l'ordre de `util.d`.
const FORMATS: [&[Champ]; 5] = [
    &[An, Lit('-'), Mois, Lit('-'), Jour],
    &[Jour, Lit('/'), Mois, Lit('/'), An],
    &[Jour, Mois, An],
    &[An, Mois, Jour],
    &[Jour, Lit('-'), Mois, Lit('-'), An],
];

fn chiffre(c: &[char], i: usize) -> bool {
    c.get(i).map_or(false, |x| x.is_ascii_digit())
}

/// Les longueurs que le champ peut avaler en `pos`, dans l'ordre des
/// alternatives de l'expression reguliere de `_strptime`.
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

fn nombre(c: &[char], de: usize, len: usize) -> u32 {
    c[de..de + len].iter().collect::<String>().trim().parse().unwrap_or(0)
}

/// Premier appariement de l'expression reguliere (avec retour sur les
/// alternatives), sans exiger la fin de chaine : comme `re.match`.
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

/// `strptime(s, format).date()` ; `None` pour tout `ValueError` (pas
/// d'appariement, reste de chaine, jour inexistant).
fn lire_format(format: &[Champ], c: &[char]) -> Option<Jour> {
    let mut vus = [0u32; 3];
    let fin = apparier(format, c, 0, &mut vus)?;
    if fin != c.len() {
        return None;
    }
    Jour::nouveau(vus[0] as i32, vus[1], vus[2])
}

/// `datetime.strptime(s, "%Y%m%d").date()` : `None` pour tout `ValueError`.
pub fn strptime_aaaammjj(s: &str) -> Option<Jour> {
    let c: Vec<char> = s.chars().collect();
    lire_format(FORMATS[3], &c)
}

// ------------------------------------------------------ les reprs Python

/// `repr(str)` : les guillemets simples, sauf si le texte porte une apostrophe
/// et aucun guillemet double. Sert aux messages (`{nom!r}`, `%r`).
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

/// `repr(float)` de Python : le plus court qui revient au meme flottant,
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
        // `{:e}` donne `1e-5` ; Python ecrit `1e-05`.
        let t = format!("{:e}", x);
        let (m, e) = t.split_once('e').unwrap_or((&t, "0"));
        let e: i32 = e.parse().unwrap_or(0);
        return format!("{}e{}{:02}", m, if e < 0 { '-' } else { '+' }, e.abs());
    }
    let t = format!("{}", x);
    if t.contains('.') {
        t
    } else {
        format!("{}.0", t)
    }
}

/// `repr(valeur)` : dict, liste, texte, nombre, `True`, `None`.
pub fn repr_py(v: &Value) -> String {
    match v {
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.to_string()
            } else if let Some(u) = n.as_u64() {
                u.to_string()
            } else {
                repr_flottant(n.as_f64().unwrap_or(f64::NAN))
            }
        }
        Value::String(s) => repr_texte(s),
        Value::Array(a) => format!("[{}]", a.iter().map(repr_py).collect::<Vec<_>>().join(", ")),
        Value::Object(o) => format!(
            "{{{}}}",
            o.iter().map(|(k, v)| format!("{}: {}", repr_texte(k), repr_py(v))).collect::<Vec<_>>().join(", ")
        ),
    }
}

/// `str(valeur)` : le texte tel quel, le reste comme `repr`.
pub fn en_texte(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        autre => repr_py(autre),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn j(an: i32, mois: u32, jour: u32) -> Jour {
        Jour::nouveau(an, mois, jour).unwrap()
    }

    #[test]
    fn les_cinq_formats() {
        let attendu = j(2026, 3, 5);
        for s in ["2026-03-05", "05/03/2026", "05032026", "20260305", "05-03-2026", " 2026-03-05 ", "2026-3-5", "5/3/2026"] {
            assert_eq!(Jour::lire(s).unwrap(), attendu, "{}", s);
        }
        assert_eq!(d(&json!({"jour": "05/03/2026", "lieu": "Tours"})).unwrap(), attendu);
        assert_eq!(d(&json!(20260305)).unwrap(), attendu);
    }

    #[test]
    fn le_premier_appariement_gagne_comme_strptime() {
        // Valeurs relevees dans Python : `util.d`.
        // 1032026 : %d%m%Y apparie 10, 3, 2026.
        assert_eq!(Jour::lire("1032026").unwrap(), j(2026, 3, 10));
        // 20260315 : %d%m%Y apparie 20, 2, 6031 puis laisse un reste -> format suivant.
        assert_eq!(Jour::lire("20260315").unwrap(), j(2026, 3, 15));
        // 19010101 : %d%m%Y l'emporte (19, 01, 0101), l'ambiguite est celle du Python.
        assert_eq!(Jour::lire("19010101").unwrap(), j(101, 1, 19));
        assert_eq!(Jour::lire("20261231").unwrap(), j(2026, 12, 31));
        // 31/02 n'existe pas : le ValueError de date() passe au format suivant, puis echoue.
        assert!(Jour::lire("31/02/2026").is_err());
        assert_eq!(Jour::lire("29/02/2024").unwrap(), j(2024, 2, 29));
        assert!(Jour::lire("29/02/2026").is_err());
    }

    #[test]
    fn ce_qui_n_est_pas_une_date_se_refuse() {
        for s in ["demain", "l'an 2", ""] {
            assert!(Jour::lire(s).is_err(), "{}", s);
        }
        assert!(d(&json!(null)).is_err());
        assert!(d(&json!({"lieu": "Tours", "n": 1.0})).is_err(), "une date detaillee sans `jour`");
    }

    #[test]
    fn les_ecritures() {
        let x = j(2026, 7, 1);
        assert_eq!(x.iso(), "2026-07-01");
        assert_eq!(x.jj_mm_aaaa(), "01/07/2026");
        assert_eq!(x.jjmmaaaa(), "01072026");
        assert_eq!(x.aaaammjj(), "20260701");
        assert_eq!(x.mois_iso(), "2026-07");
        assert!(j(2026, 6, 30) < x && j(2025, 12, 31) < j(2026, 1, 1));
    }

    #[test]
    fn valeurs_relevees_dans_python() {
        // date.toordinal() et date.weekday() de Python.
        assert_eq!(j(2026, 7, 1).ordinal(), 739_798);
        assert_eq!(j(2026, 7, 1).jour_de_la_semaine(), 2);
        assert_eq!(j(1900, 1, 1).ordinal(), 693_596);
        assert_eq!(j(2100, 12, 31).ordinal(), 767_009);
        assert_eq!(j(9999, 12, 31).ordinal(), 3_652_059);
        assert_eq!(j(1, 1, 1).ordinal(), 1);
        assert_eq!(j(1, 1, 1).jour_de_la_semaine(), 0);
    }

    /// Formule independante : jours avant l'annee + jours avant le mois + jour,
    /// comme `_days_before_year` de CPython (et non la formule des cycles).
    fn ordinal_naif(an: i32, mois: u32, jour: u32) -> i64 {
        let y = i64::from(an) - 1;
        let mut n = y * 365 + y / 4 - y / 100 + y / 400;
        for m in 1..mois {
            n += i64::from(jours_du_mois(an, m));
        }
        n + i64::from(jour)
    }

    #[test]
    fn ordinal_exhaustif_1900_2100() {
        let mut precedent = None;
        let mut semaine = j(1900, 1, 1).jour_de_la_semaine();
        assert_eq!(semaine, 0); // le 1er janvier 1900 etait un lundi
        for an in 1900..=2100 {
            for mois in 1..=12 {
                for jour in 1..=jours_du_mois(an, mois) {
                    let x = j(an, mois, jour);
                    let o = x.ordinal();
                    assert_eq!(o, ordinal_naif(an, mois, jour), "{}", x.iso());
                    assert_eq!(Jour::depuis_ordinal(o), Some(x), "{}", x.iso());
                    assert_eq!(Jour::lire(&x.iso()).unwrap(), x);
                    assert_eq!(Jour::lire(&x.jj_mm_aaaa()).unwrap(), x);
                    assert_eq!(Jour::lire(&x.jjmmaaaa()).unwrap(), x);
                    if let Some(p) = precedent {
                        assert_eq!(o - p, 1, "{}", x.iso());
                        semaine = (semaine + 1) % 7;
                    }
                    assert_eq!(x.jour_de_la_semaine(), semaine, "{}", x.iso());
                    precedent = Some(o);
                }
            }
        }
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

    #[test]
    fn le_nom_d_un_mois_hors_01_12_se_refuse_et_garde_sa_graphie() {
        // « 00 » n'est pas decembre (l'indice -1 du Python), ni « 13 » janvier : aucun mois, un refus.
        for n in [0, 13, -1] {
            for g in [Graphie::Titre, Graphie::Minuscule, Graphie::Accentuee] {
                assert_eq!(nom_du_mois(n, g).unwrap_err(), mois_impossible(n));
            }
        }
        assert_eq!((nom_du_mois(8, Graphie::Titre).unwrap(), nom_du_mois(8, Graphie::Minuscule).unwrap(), nom_du_mois(8, Graphie::Accentuee).unwrap()), ("Aout", "aout", "août"));
        assert_eq!(libelle_du_mois("2026-12", Graphie::Accentuee).unwrap(), "décembre 2026");
        assert!(libelle_du_mois("2026-00", Graphie::Accentuee).is_err());
        assert_eq!(libelle_du_mois_ou_tel_quel("2026-00", Graphie::Minuscule), "2026-00");
        assert_eq!(libelle_du_mois_ou_tel_quel("2026-02", Graphie::Minuscule), "fevrier 2026");
    }

    #[test]
    fn les_mois() {
        assert_eq!(jours_du_mois(2024, 2), 29);
        assert_eq!(jours_du_mois(1900, 2), 28);
        assert_eq!(jours_du_mois(2000, 2), 29);
        assert_eq!(j(2026, 4, 9).jours_du_mois(), 30);
        assert_eq!(premier_du_mois("2026-03").unwrap(), j(2026, 3, 1));
        assert_eq!(dernier_du_mois("2026-02").unwrap(), j(2026, 2, 28));
        assert_eq!(dernier_du_mois("2024-02").unwrap(), j(2024, 2, 29));
        assert_eq!(dernier_du_mois("2026-12").unwrap(), j(2026, 12, 31));
        assert_eq!(dernier_du_mois("2026-13").unwrap_err(), mois_impossible(13));
        assert_eq!(j(2026, 12, 15).mois_suivant(), "2027-01");
        assert_eq!(j(2026, 3, 15).mois_suivant(), "2026-04");
        assert_eq!(mois_entre(&j(2025, 11, 20), &j(2026, 2, 3)), ["2025-11", "2025-12", "2026-01", "2026-02"]);
    }

    #[test]
    fn les_flottants_s_ecrivent_comme_str_de_python() {
        // `num::dec_py` en tire un Decimal (`Decimal(str(x))`) : la forme du flottant y est une valeur.
        assert_eq!(repr_flottant(1.0), "1.0");
        assert_eq!(repr_flottant(0.25), "0.25");
        assert_eq!(repr_flottant(1e-5), "1e-05");
        assert_eq!(repr_flottant(1e16), "1e+16");
        assert_eq!(repr_flottant(123456789012345.0), "123456789012345.0");
    }
}
