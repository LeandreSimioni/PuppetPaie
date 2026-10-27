//! `paie generer-tables SOURCES ESPACE` : ecrit les quatre tables que le moteur tient de l'Unicode (et de quelques autres
//! textes) depuis les textes deposes dans `atelier/reference/sources/` (`SOURCES`), dans l'espace `atelier/sources/`
//! (`ESPACE`) :
//!
//! - `socle/src/tables_nfkc.rs` (pour `nfkc.rs`) : la decomposition de compatibilite complete, les classes de combinaison et les
//!   compositions canoniques ;
//! - `paie/src/nfkd_ascii.rs` (pour `dpae.rs`) : ce que la decomposition de compatibilite laisse d'ASCII, et `str.isdigit` ;
//! - `paie/src/tables_retours.rs` (pour `retours.rs`) : `\d`, cp437, windows-1252, les noms d'encodage que `pyexpat` connait, les
//!   caracteres d'un nom XML qu'expat admet ;
//! - `paie/src/tables_journal.rs` (pour `etats.rs`) : `str.isalnum`.
//!
//! Les textes (`LISEZ-MOI.md` de chaque dossier) :
//! - `unicode/` : la base de caracteres Unicode (`UnicodeData.txt`, `DerivedNormalizationProps.txt`,
//!   `extracted/DerivedNumericType.txt`), les pages de code cp437 et cp1252 de `Public/MAPPINGS`, et la transcription
//!   des definitions du chapitre 3 de la norme (D65 : decomposition de compatibilite ; D109 : ordre canonique ;
//!   D114 : composite primaire ; § 3.12 : syllabes Hangul) ;
//! - `python/` : la documentation de `str.isdigit`, `str.isalnum`, `str.isdecimal` et de `\d` (les proprietes Unicode
//!   qu'elles traduisent), et le releve des noms d'encodage par classe (`pyexpat.c`, `PyUnknownEncodingHandler`) ;
//! - `w3c/` : XML 1.0 quatrieme edition, annexe B (les classes de caracteres d'un nom, celles d'expat).
//!
//! Le moteur n'embarque pas de crate de normalisation : ces tables sont son Unicode. Une version plus recente d'Unicode
//! n'ajoute que des caracteres a la normalisation (politique de stabilite, `unicode/LISEZ-MOI.md`). Le test
//! `tables_generees` (`tests/fichiers/`) verifie que les tables commitees sont celles que ce module produit.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;

/// Les textes lus, relatifs a `atelier/reference/sources/`.
pub const UNICODE_DONNEES: &str = "unicode/unicode-ucd-18.0.0-unicodedata-20261015.txt";
pub const UNICODE_NORMALISATION: &str = "unicode/unicode-ucd-18.0.0-derivednormalizationprops-20261015.txt";
pub const UNICODE_NUMERIQUE: &str = "unicode/unicode-ucd-18.0.0-derivednumerictype-20261015.txt";
pub const CP437: &str = "unicode/unicode-mappings-microsoft-cp437-20261015.txt";
pub const CP1252: &str = "unicode/unicode-mappings-microsoft-cp1252-20261015.txt";
pub const XML_4E: &str = "w3c/w3c-xml-1.0-quatrieme-edition-20060816.html";
pub const CODECS: &str = "python/python-releve-codecs-pyexpat-3.11.17-20261015.txt";

/// Les quatre fichiers ecrits, dans l'ordre.
pub const FICHIERS: [&str; 4] = ["socle/src/tables_nfkc.rs", "paie/src/nfkd_ascii.rs", "paie/src/tables_retours.rs", "paie/src/tables_journal.rs"];

const FIN: u32 = 0x110000;

fn surrogat(cp: u32) -> bool {
    (0xD800..0xE000).contains(&cp)
}

// Les syllabes Hangul, par le calcul (norme Unicode, § 3.12, « Hangul Syllable Decomposition » :
// `unicode/unicode-norme-18.0.0-chapitre-3-extraits-20261015.md`).
const S_BASE: u32 = 0xAC00;
const L_BASE: u32 = 0x1100;
const V_BASE: u32 = 0x1161;
const T_BASE: u32 = 0x11A7;
const T_NB: u32 = 28;
const N_NB: u32 = 21 * T_NB;
const S_NB: u32 = 19 * N_NB;

fn hangul(cp: u32) -> bool {
    (S_BASE..S_BASE + S_NB).contains(&cp)
}

/// Ce que `UnicodeData.txt` dit d'un point de code (les plages `<..., First>`/`<..., Last>` deployees).
struct Ucd {
    /// La categorie generale, deux lettres (`Cn` si le point de code n'est pas attribue).
    categorie: Vec<[u8; 2]>,
    classe: Vec<u8>,
    /// La decomposition (champ 5) : `true` si de compatibilite (`<...>`), et les points de code.
    decomposition: HashMap<u32, (bool, Vec<u32>)>,
    /// `Numeric_Type` (`DerivedNumericType.txt`) : `Decimal`, `Digit` ou `Numeric`.
    numerique: HashMap<u32, String>,
    /// `Full_Composition_Exclusion` (`DerivedNormalizationProps.txt`).
    exclusion: Vec<bool>,
    version: String,
}

fn lire(sources: &Path, nom: &str) -> Result<String, String> {
    std::fs::read_to_string(sources.join(nom)).map_err(|e| format!("{} illisible ({})", sources.join(nom).display(), e))
}

fn hexa(s: &str, ou: &str) -> Result<u32, String> {
    u32::from_str_radix(s.trim(), 16).map_err(|_| format!("{} : {:?} n'est pas un point de code", ou, s))
}

/// `XXXX` ou `XXXX..YYYY`.
fn plage(s: &str, ou: &str) -> Result<(u32, u32), String> {
    match s.trim().split_once("..") {
        Some((a, b)) => Ok((hexa(a, ou)?, hexa(b, ou)?)),
        None => {
            let a = hexa(s, ou)?;
            Ok((a, a))
        }
    }
}

/// La version lue dans la premiere ligne d'un fichier derive (`# DerivedNormalizationProps-18.0.0.txt`).
fn version_de(texte: &str, prefixe: &str, ou: &str) -> Result<String, String> {
    texte
        .lines()
        .next()
        .and_then(|l| l.strip_prefix("# ")?.strip_prefix(prefixe)?.strip_prefix('-')?.strip_suffix(".txt"))
        .map(str::to_string)
        .ok_or_else(|| format!("{} : la premiere ligne ne dit pas la version", ou))
}

/// Les lignes `plage ; valeur [; ...] # commentaire` d'un fichier de la base, commentaires retires.
fn proprietes(texte: &str) -> impl Iterator<Item = Vec<&str>> {
    texte.lines().filter_map(|l| {
        let l = l.split('#').next().unwrap_or("").trim();
        (!l.is_empty()).then(|| l.split(';').map(str::trim).collect())
    })
}

impl Ucd {
    fn lire(sources: &Path) -> Result<Ucd, String> {
        let n = FIN as usize;
        let mut u = Ucd { categorie: vec![*b"Cn"; n], classe: vec![0; n], decomposition: HashMap::new(), numerique: HashMap::new(), exclusion: vec![false; n], version: String::new() };
        let donnees = lire(sources, UNICODE_DONNEES)?;
        let mut premier: Option<u32> = None;
        for l in donnees.lines().filter(|l| !l.is_empty()) {
            let f: Vec<&str> = l.split(';').collect();
            if f.len() != 15 {
                return Err(format!("{} : ligne de {} champs ({:?})", UNICODE_DONNEES, f.len(), l));
            }
            let cp = hexa(f[0], UNICODE_DONNEES)?;
            let categorie: [u8; 2] = f[2].as_bytes().try_into().map_err(|_| format!("{} : categorie {:?}", UNICODE_DONNEES, f[2]))?;
            let classe: u8 = f[3].parse().map_err(|_| format!("{} : classe {:?}", UNICODE_DONNEES, f[3]))?;
            let debut = if f[1].ends_with(", Last>") { premier.take().ok_or_else(|| format!("{} : {:?} sans First", UNICODE_DONNEES, l))? } else { cp };
            if f[1].ends_with(", First>") {
                premier = Some(cp);
                continue;
            }
            for c in debut..=cp {
                u.categorie[c as usize] = categorie;
                u.classe[c as usize] = classe;
            }
            if !f[5].is_empty() {
                let (compat, reste) = match f[5].strip_prefix('<') {
                    Some(r) => (true, r.split_once('>').map(|(_, r)| r).unwrap_or("")),
                    None => (false, f[5]),
                };
                let points = reste.split_whitespace().map(|p| hexa(p, UNICODE_DONNEES)).collect::<Result<Vec<_>, _>>()?;
                u.decomposition.insert(cp, (compat, points));
            }
        }
        let normalisation = lire(sources, UNICODE_NORMALISATION)?;
        u.version = version_de(&normalisation, "DerivedNormalizationProps", UNICODE_NORMALISATION)?;
        for f in proprietes(&normalisation) {
            if f.len() == 2 && f[1] == "Full_Composition_Exclusion" {
                let (a, b) = plage(f[0], UNICODE_NORMALISATION)?;
                (a..=b).for_each(|c| u.exclusion[c as usize] = true);
            }
        }
        let numerique = lire(sources, UNICODE_NUMERIQUE)?;
        let v = version_de(&numerique, "DerivedNumericType", UNICODE_NUMERIQUE)?;
        if v != u.version {
            return Err(format!("{} est d'Unicode {}, {} d'Unicode {}", UNICODE_NUMERIQUE, v, UNICODE_NORMALISATION, u.version));
        }
        for f in proprietes(&numerique) {
            let (a, b) = plage(f[0], UNICODE_NUMERIQUE)?;
            match f.get(1) {
                Some(&t @ ("Decimal" | "Digit" | "Numeric")) => (a..=b).for_each(|c| {
                    u.numerique.insert(c, t.to_string());
                }),
                autre => return Err(format!("{} : Numeric_Type {:?}", UNICODE_NUMERIQUE, autre)),
            }
        }
        Ok(u)
    }

    fn lettre(&self, cp: u32) -> bool {
        matches!(&self.categorie[cp as usize], b"Lu" | b"Ll" | b"Lt" | b"Lm" | b"Lo")
    }

    /// La decomposition de compatibilite d'un point de code (D65) : les correspondances canoniques et de compatibilite
    /// appliquees jusqu'au bout, les syllabes Hangul par le calcul, puis l'ordre canonique (D109 : un tri stable des
    /// signes de classe non nulle, entre deux caracteres de classe 0).
    fn nfkd(&self, cp: u32) -> Vec<u32> {
        fn deplier(u: &Ucd, cp: u32, out: &mut Vec<u32>) {
            if hangul(cp) {
                let s = cp - S_BASE;
                out.push(L_BASE + s / N_NB);
                out.push(V_BASE + (s % N_NB) / T_NB);
                if s % T_NB != 0 {
                    out.push(T_BASE + s % T_NB);
                }
            } else if let Some((_, d)) = u.decomposition.get(&cp) {
                d.iter().for_each(|&c| deplier(u, c, out));
            } else {
                out.push(cp);
            }
        }
        let mut out = Vec::new();
        deplier(self, cp, &mut out);
        let mut i = 0;
        while i < out.len() {
            let j = (i..out.len()).find(|&k| self.classe[out[k] as usize] == 0).unwrap_or(out.len());
            out[i..j].sort_by_key(|&c| self.classe[c as usize]);
            i = j + 1;
        }
        out
    }
}

/// Les intervalles fusionnes des points de code (hors surrogats) qui verifient `garde`.
fn intervalles(garde: impl Fn(u32) -> bool) -> Vec<(u32, u32)> {
    let mut out: Vec<(u32, u32)> = Vec::new();
    for cp in (0..FIN).filter(|&c| !surrogat(c) && garde(c)) {
        match out.last_mut() {
            Some(d) if d.1 + 1 == cp => d.1 = cp,
            _ => out.push((cp, cp)),
        }
    }
    out
}

fn ecrire_intervalles(s: &mut String, v: &[(u32, u32)]) {
    for (a, b) in v {
        let _ = writeln!(s, "    (0x{:X}, 0x{:X}),", a, b);
    }
    s.push_str("];\n");
}

fn en_tete(s: &mut String, fichiers: &[&str], version: Option<&str>) {
    let _ = writeln!(s, "//! GENERE par `paie generer-tables` : ne pas editer.{}", version.map(|v| format!(" Unicode {}.", v)).unwrap_or_default());
    for f in fichiers {
        let _ = writeln!(s, "//! Lu dans `atelier/reference/sources/{}`.", f);
    }
}

fn tables_nfkc(u: &Ucd) -> String {
    let mut s = String::new();
    en_tete(&mut s, &[UNICODE_DONNEES, UNICODE_NORMALISATION], Some(&u.version));
    s.push_str("//!\n");
    s.push_str("//! Les tables de `nfkc.rs`. `DECOMP_POINTS[i]` se decompose en `DECOMP_TEXTE`\n");
    s.push_str("//! de `DECOMP_PLACE[i] >> 7` sur `DECOMP_PLACE[i] & 127` octets (NFKD complet).\n\n");
    // La decomposition de compatibilite complete (D65) de chaque point de code qu'elle change, hors syllabes Hangul.
    let dec: Vec<(u32, String)> = (0x80..FIN)
        .filter(|&c| !surrogat(c) && !hangul(c) && u.decomposition.contains_key(&c))
        .map(|c| (c, u.nfkd(c).into_iter().filter_map(char::from_u32).collect()))
        .collect();
    let mut texte = String::new();
    let mut places = Vec::new();
    for (_, r) in &dec {
        assert!(r.len() < 128, "une decomposition de plus de 127 octets");
        places.push(((texte.len() as u32) << 7) | r.len() as u32);
        texte.push_str(r);
    }
    s.push_str("pub const DECOMP_POINTS: &[u32] = &[\n");
    for morceau in dec.chunks(12) {
        let _ = writeln!(s, "    {},", morceau.iter().map(|(c, _)| format!("0x{:X}", c)).collect::<Vec<_>>().join(", "));
    }
    s.push_str("];\n\npub const DECOMP_PLACE: &[u32] = &[\n");
    for morceau in places.chunks(12) {
        let _ = writeln!(s, "    {},", morceau.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", "));
    }
    s.push_str("];\n\npub const DECOMP_TEXTE: &str = \"");
    for c in texte.chars() {
        let _ = write!(s, "\\u{{{:x}}}", c as u32);
    }
    s.push_str("\";\n\n/// (debut, fin, classe de combinaison), par intervalles.\npub const CLASSES: &[(u32, u32, u8)] = &[\n");
    let mut classes: Vec<(u32, u32, u8)> = Vec::new();
    for cp in (0..FIN).filter(|&c| !surrogat(c) && u.classe[c as usize] != 0) {
        let k = u.classe[cp as usize];
        match classes.last_mut() {
            Some(d) if d.1 + 1 == cp && d.2 == k => d.1 = cp,
            _ => classes.push((cp, cp, k)),
        }
    }
    for (a, b, k) in classes {
        let _ = writeln!(s, "    (0x{:X}, 0x{:X}, {}),", a, b, k);
    }
    // Les composites primaires (D114) de decomposition canonique en deux points de code : ni exclusion de composition
    // complete (D113), ni syllabe Hangul (composee par le calcul).
    let mut compositions: Vec<(u32, u32, u32)> = u
        .decomposition
        .iter()
        .filter(|(&c, (compat, d))| c >= 0x80 && !hangul(c) && !compat && d.len() == 2 && !u.exclusion[c as usize])
        .map(|(&c, (_, d))| (d[0], d[1], c))
        .collect();
    compositions.sort();
    s.push_str("];\n\n/// (premier, second, compose), triees par (premier, second).\npub const COMPOSITIONS: &[(u32, u32, u32)] = &[\n");
    for (a, b, c) in compositions {
        let _ = writeln!(s, "    (0x{:X}, 0x{:X}, 0x{:X}),", a, b, c);
    }
    s.push_str("];\n");
    s
}

fn nfkd_ascii(u: &Ucd) -> String {
    let mut s = String::new();
    en_tete(&mut s, &[UNICODE_DONNEES, UNICODE_NUMERIQUE], Some(&u.version));
    s.push_str("//!\n");
    s.push_str("//! Ce que `unicodedata.normalize(\"NFKD\", c).encode(\"ascii\", \"ignore\")` laisse\n");
    s.push_str("//! de chaque caractere non ASCII. `(debut, fin, texte, avance)` : de `debut` a `fin`,\n");
    s.push_str("//! `texte` ; si `avance`, la lettre de `texte` avance d'un cran a chaque point de code.\n\n");
    // Les suites : meme texte d'un caractere sur plusieurs points de code (« ÀÁÂ » -> A), ou texte d'un caractere qui
    // avance d'une lettre a chaque point de code (les alphabets pleine chasse, mathematiques...).
    let mut suites: Vec<(u32, u32, String, Option<u32>)> = Vec::new();
    for cp in (0x80..FIN).filter(|c| u.decomposition.contains_key(c) || hangul(*c)) {
        let r: String = u.nfkd(cp).into_iter().filter(|&c| c < 0x80).filter_map(char::from_u32).collect();
        if r.is_empty() {
            continue;
        }
        if let Some(d) = suites.last_mut() {
            if r.len() == 1 && d.2.len() == 1 && d.1 + 1 == cp {
                let (lettre, t) = (r.as_bytes()[0] as i64, d.2.as_bytes()[0] as i64);
                let avance = match d.3 {
                    Some(a) => Some(a).filter(|&a| lettre == t + a as i64 * (cp - d.0) as i64),
                    None => Some(lettre - t).filter(|a| *a == 0 || *a == 1).map(|a| a as u32),
                };
                if let Some(a) = avance {
                    d.1 = cp;
                    d.3 = Some(a);
                    continue;
                }
            }
        }
        suites.push((cp, cp, r, None));
    }
    s.push_str("pub const TABLE: &[(u32, u32, &str, bool)] = &[\n");
    for (debut, fin, t, avance) in &suites {
        let _ = writeln!(s, "    (0x{:X}, 0x{:X}, \"{}\", {}),", debut, fin, t.replace('\\', "\\\\").replace('"', "\\\""), avance == &Some(1) && fin > debut);
    }
    let _ = write!(s, "];\n\n/// Les intervalles de points de code ou `str.isdigit()` est vrai (Unicode {}).\npub const CHIFFRES: &[(u32, u32)] = &[\n", u.version);
    // `str.isdigit` : Numeric_Type=Digit ou Numeric_Type=Decimal (`python/LISEZ-MOI.md`).
    ecrire_intervalles(&mut s, &intervalles(|c| matches!(u.numerique.get(&c).map(String::as_str), Some("Decimal" | "Digit"))));
    s
}

/// Une page de code de `Public/MAPPINGS` (`0xXX<tab>0xXXXX<tab>#NOM`) : les octets 0x80 a 0xFF, U+FFFD pour un octet
/// que la page ne definit pas (`#UNDEFINED`, comme le `replace` de Python que `pyexpat` lit).
fn page_de_code(sources: &Path, nom: &str) -> Result<[u32; 128], String> {
    let texte = lire(sources, nom)?;
    let mut t = [0u32; 128];
    let mut vus = 0;
    // CP437.TXT finit par 0x1A (la fin de fichier de DOS) : ce n'est pas une ligne.
    for l in texte.lines().map(|l| l.trim_end_matches(['\r', '\u{1a}'])).filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let mut f = l.split('\t');
        let octet = f.next().and_then(|o| o.strip_prefix("0x")).map(|o| hexa(o, nom)).transpose()?.ok_or_else(|| format!("{} : {:?}", nom, l))?;
        let point = f.next().map(str::trim).unwrap_or("");
        if octet < 0x80 {
            continue;
        }
        t[octet as usize - 0x80] = match point.strip_prefix("0x") {
            Some(p) => hexa(p, nom)?,
            None if point.is_empty() => 0xFFFD,
            None => return Err(format!("{} : {:?}", nom, l)),
        };
        vus += 1;
    }
    if vus != 128 {
        return Err(format!("{} : {} octets de 0x80 a 0xFF, 128 attendus", nom, vus));
    }
    Ok(t)
}

/// Le releve des noms d'encodage par classe (`[latin1]`, puis un nom par ligne ; `#` : commentaire).
fn codecs(sources: &Path) -> Result<Vec<(&'static str, Vec<String>)>, String> {
    let mut classes: Vec<(&'static str, Vec<String>)> = ["latin1", "ascii", "cp1252", "autres", "refuses"].iter().map(|c| (*c, Vec::new())).collect();
    let mut courante: Option<usize> = None;
    for l in lire(sources, CODECS)?.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        if let Some(c) = l.strip_prefix('[').and_then(|c| c.strip_suffix(']')) {
            courante = Some(classes.iter().position(|(n, _)| *n == c).ok_or_else(|| format!("{} : classe inconnue {:?}", CODECS, c))?);
        } else {
            let i = courante.ok_or_else(|| format!("{} : {:?} avant toute classe", CODECS, l))?;
            classes[i].1.push(l.to_string());
        }
    }
    let mut tous: Vec<&String> = classes.iter().flat_map(|(_, v)| v).collect();
    let n = tous.len();
    tous.sort();
    tous.dedup();
    if tous.len() != n {
        return Err(format!("{} : un nom figure deux fois", CODECS));
    }
    classes.iter_mut().for_each(|(_, v)| v.sort());
    Ok(classes)
}

/// Une production de l'annexe B de XML 1.0, quatrieme edition (`[#xXXXX-#xYYYY] | #xZZZZ | ...`), lue dans la page :
/// le code qui suit `::=` apres l'ancre `NT-<nom>`.
fn production(page: &str, nom: &str) -> Result<Vec<(u32, u32)>, String> {
    let ou = format!("{} : production {}", XML_4E, nom);
    let apres = page.split_once(&format!("id=\"NT-{}\"", nom)).map(|(_, r)| r).ok_or_else(|| format!("{} absente", ou))?;
    let apres = apres.split_once("::=").map(|(_, r)| r).ok_or_else(|| ou.clone())?;
    let code = apres.split_once("<code>").and_then(|(_, r)| r.split_once("</code>")).map(|(c, _)| c).ok_or_else(|| ou.clone())?;
    let mut out = Vec::new();
    for alt in code.split('|').map(str::trim) {
        let p = match alt.strip_prefix('[').and_then(|a| a.strip_suffix(']')) {
            Some(a) => a.split_once('-').ok_or_else(|| format!("{} : {:?}", ou, alt))?,
            None => (alt, alt),
        };
        let point = |x: &str| x.strip_prefix("#x").ok_or_else(|| format!("{} : {:?}", ou, alt)).and_then(|x| hexa(x, &ou));
        out.push((point(p.0)?, point(p.1)?));
    }
    Ok(out)
}

fn tables_retours(sources: &Path, u: &Ucd) -> Result<String, String> {
    let mut s = String::new();
    en_tete(&mut s, &[UNICODE_DONNEES, CP437, CP1252, CODECS, XML_4E], Some(&u.version));
    // `\d` de `re` sur du texte : la categorie Nd (`python/LISEZ-MOI.md`).
    s.push_str("\n/// Les intervalles de points de code ou `str.isdecimal()` est vrai (le `\\d` de `re`).\npub const DECIMAUX: &[(u32, u32)] = &[\n");
    ecrire_intervalles(&mut s, &intervalles(|c| &u.categorie[c as usize] == b"Nd"));
    for (nom, fichier, doc) in [("CP437", CP437, "cp437 des octets 0x80 a 0xFF"), ("WINDOWS_1252", CP1252, "windows-1252 des octets 0x80 a 0xFF (U+FFFD : non defini)")] {
        let _ = write!(s, "\n/// {}.\npub const {}: [u32; 128] = [\n", doc, nom);
        for ligne in page_de_code(sources, fichier)?.chunks(8) {
            let _ = writeln!(s, "    {},", ligne.iter().map(|x| format!("0x{:X}", x)).collect::<Vec<_>>().join(", "));
        }
        s.push_str("];\n");
    }
    let docs = ["la table identite", "l'ASCII, le reste refuse", "windows-1252", "une autre table de 256 caracteres (non portee)", "pas de table : expat refuse"];
    for ((classe, noms), doc) in codecs(sources)?.into_iter().zip(docs) {
        let _ = write!(s, "\n/// Noms d'encodage normalises que `pyexpat` lit comme {}.\npub const CODECS_{}: &[&str] = &[\n", doc, classe.to_uppercase());
        for n in noms {
            let _ = writeln!(s, "    \"{}\",", n);
        }
        s.push_str("];\n");
    }
    // XML 1.0 quatrieme edition : [5] Name ::= (Letter | '_' | ':') (NameChar)* ; [4] NameChar ::= Letter | Digit | '.' |
    // '-' | '_' | ':' | CombiningChar | Extender ; [84] Letter ::= BaseChar | Ideographic. Le deux-points est traite a part
    // (`retours::nom_valide`).
    let page = lire(sources, XML_4E)?;
    let dans = |v: &[(u32, u32)], c: u32| v.iter().any(|&(a, b)| (a..=b).contains(&c));
    let lettre: Vec<(u32, u32)> = [production(&page, "BaseChar")?, production(&page, "Ideographic")?].concat();
    let suite: Vec<(u32, u32)> = [lettre.clone(), production(&page, "Digit")?, production(&page, "CombiningChar")?, production(&page, "Extender")?].concat();
    let debut = intervalles(|c| c == '_' as u32 || dans(&lettre, c));
    let suite = intervalles(|c| matches!(char::from_u32(c), Some('.' | '-' | '_')) || dans(&suite, c));
    s.push_str("\n/// Les intervalles de points de code qui peuvent commencer un nom (hors deux-points).\npub const NOM_DEBUT: &[(u32, u32)] = &[\n");
    ecrire_intervalles(&mut s, &debut);
    s.push_str("\n/// Les intervalles de points de code qui peuvent suivre la premiere lettre d'un nom (hors deux-points).\npub const NOM_SUITE: &[(u32, u32)] = &[\n");
    ecrire_intervalles(&mut s, &suite);
    Ok(s)
}

fn tables_journal(u: &Ucd) -> String {
    let mut s = String::new();
    en_tete(&mut s, &[UNICODE_DONNEES, UNICODE_NUMERIQUE], Some(&u.version));
    // `str.isalnum` : une lettre (Lu, Ll, Lt, Lm, Lo), ou un Numeric_Type (Decimal, Digit, Numeric) : `python/LISEZ-MOI.md`.
    s.push_str("\n/// Les intervalles de points de code ou `str.isalnum()` est vrai.\npub const ALNUM: &[(u32, u32)] = &[\n");
    ecrire_intervalles(&mut s, &intervalles(|c| u.lettre(c) || u.numerique.contains_key(&c)));
    s
}

/// Les quatre fichiers (`FICHIERS`, dans l'ordre) et leur contenu, depuis les textes de `sources`.
pub fn generer(sources: &Path) -> Result<Vec<(&'static str, String)>, String> {
    let u = Ucd::lire(sources)?;
    Ok(vec![(FICHIERS[0], tables_nfkc(&u)), (FICHIERS[1], nfkd_ascii(&u)), (FICHIERS[2], tables_retours(sources, &u)?), (FICHIERS[3], tables_journal(&u))])
}

/// `paie generer-tables SOURCES SRC`.
pub fn commande(args: &[String]) -> i32 {
    let [sources, espace] = args else {
        eprintln!("usage : paie generer-tables SOURCES ESPACE   (SOURCES : atelier/reference/sources ; ESPACE : atelier/sources)");
        return 2;
    };
    match generer(Path::new(sources)) {
        Ok(fichiers) => {
            for (nom, contenu) in fichiers {
                let chemin = Path::new(espace).join(nom);
                if let Err(e) = std::fs::write(&chemin, contenu) {
                    eprintln!("{} : {}", chemin.display(), e);
                    return 1;
                }
                println!("{}", chemin.display());
            }
            0
        }
        Err(m) => {
            eprintln!("{}", m);
            1
        }
    }
}
