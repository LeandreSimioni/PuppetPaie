//! La lecture des retours DSN (`moteur/retours.py`) : ce que les organismes
//! repondent apres un depot.
//!
//! Des octets ou du XML en entree, des valeurs JSON en sortie : les bilans de
//! depot (natures 10 et 11), leurs anomalies, la liste des retours et des
//! depots, le CRM 94 (taux du PAS) par ses quatre strategies, les AER de
//! nature 46 (une archive .zip, lue ici a la main), les fiches de
//! parametrage des organismes complementaires.
//!
//! Le XML se lit comme ElementTree : un arbre d'elements dont le texte est
//! celui d'avant le premier enfant (commentaires et instructions ignores), les
//! balises et les attributs a espace de noms ecrits `{uri}nom`. Un XML qui ne se
//! lit pas est une `ParseError` (la phrase est celle de `roxmltree`, pas celle
//! d'expat). Le Python reste a ses portes : `retours.py` appelle `paie retours`.

use serde_json::{json, Map, Value};

use crate::tables_retours::{CODECS_ASCII, CODECS_AUTRES, CODECS_CP1252, CODECS_LATIN1, CODECS_REFUSES, CP437, DECIMAUX, NOM_DEBUT, NOM_SUITE, WINDOWS_1252};

type Resultat<T> = Result<T, Echec>;

/// Un refus : sa phrase, et la classe d'exception Python que la porte releve
/// (`ParseError`, `BadZipFile`...) ; `None` pour un refus ordinaire.
#[derive(Debug)]
pub struct Echec {
    pub classe: Option<&'static str>,
    pub message: String,
}

impl From<String> for Echec {
    fn from(message: String) -> Self {
        Echec { classe: None, message }
    }
}

impl From<&str> for Echec {
    fn from(m: &str) -> Self {
        Echec { classe: None, message: m.to_string() }
    }
}

fn echec(classe: &'static str, message: impl Into<String>) -> Echec {
    Echec { classe: Some(classe), message: message.into() }
}

// ---------------------------------------------------------------- les tables

/// Natures de retour, guide d'implementation de l'API DSN v6.8, section 1.3.
pub const NATURES: &[(&str, &str)] = &[
    ("10", "Accuse d'enregistrement electronique / avis de rejet"),
    ("11", "Certificat de conformite / bilan d'anomalies"),
    ("20", "Bilan d'identification des salaries"),
    ("21", "Controles inter-declarations / signalement AT-RT"),
    ("22", "Controles arret de plus de 6 mois"),
    ("23", "Controles temps partiel therapeutique"),
    ("34", "Compte-rendu metier taux AT/MP (notification de la Carsat)"),
    ("40", "Accuse de reception France Travail"),
    ("42", "Compte-rendu metier CDDU-D"),
    ("43", "AER CDDU-D (archive PDF)"),
    ("44", "AER ou compte-rendu metier FCTU"),
    ("45", "Compte-rendu metier « cinema, spectacle, expatries »"),
    ("46", "AER « cinema, spectacle » (archive PDF)"),
    ("50", "Accuse de distribution a l'organisme complementaire"),
    ("51", "Compte-rendu metier de l'organisme complementaire"),
    ("60", "Accuse de reception URSSAF"),
    ("61", "Compte-rendu metier URSSAF, donnees agregees"),
    ("70", "Accuse de reception AGIRC-ARRCO"),
    ("71", "Compte-rendu metier AGIRC-ARRCO"),
    ("92", "Accuse de reception DGFiP"),
    ("93", "Compte-rendu metier DGFiP, donnees agregees"),
    ("94", "Compte-rendu metier DGFiP, donnees nominatives (taux du PAS)"),
    ("95", "Compte-rendu metier DGFiP, controles taux sur taux"),
    ("117", "Compte-rendu metier bonus-malus contribution chomage"),
    ("118", "Compte-rendu metier effectifs"),
    ("119", "Compte-rendu metier URSSAF H+4 (immediat)"),
    ("120", "Compte-rendu metier URSSAF J+5 (opposable)"),
    ("121", "Compte-rendu metier identite"),
    ("124", "Compte-rendu metier URSSAF de rappel"),
];

pub const NATURE_TAUX_PAS: &str = "94";
pub const NATURE_TAUX_ATMP: &str = "34";
pub const NATURE_CRM_SPECTACLE: &str = "45";
pub const NATURE_AER_SPECTACLE: &str = "46";
pub const ESSAI_TEST: &str = "01";
pub const ESSAI_REEL: &str = "02";
pub const NATURES_BILAN_DEPOT: [&str; 2] = ["10", "11"];
pub const A_ARCHIVER_ABSOLUMENT: [&str; 2] = ["10", "11"];
pub const RAPPORTS_DEPOT: [&str; 3] = ["CCO", "BAN", "AEE"];

/// Les rubriques DSN du taux de PAS, et ce qu'elles portent.
const RUBRIQUES_PAS: [(&str, &str); 3] = [
    ("S21.G00.50.006", "taux"),
    ("S21.G00.50.007", "type"),
    ("S21.G00.50.008", "identifiant"),
];

/// Noms metier NEODeS des memes rubriques (cahier technique 2026.1, p. 244).
const NOMS_METIER: [(&str, &str); 3] = [("tauxpas", "taux"), ("typetauxpas", "type"), ("idtauxpas", "identifiant")];

/// `retours.libelle_nature`, sur la representation texte du code.
pub fn libelle_nature(code: &str) -> String {
    match NATURES.iter().find(|(c, _)| *c == code) {
        Some((_, l)) => l.to_string(),
        None => format!("nature {} (inconnue de ce moteur)", code),
    }
}

// ------------------------------------------------------ le texte, a la Python

/// `str.isspace()` de Python (donc ce que `strip()` retire).
fn est_blanc(c: char) -> bool {
    matches!(c, '\u{9}'..='\u{d}' | '\u{1c}'..='\u{20}' | '\u{85}' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}')
}

/// `str.strip()`.
fn nettoye(s: &str) -> &str {
    s.trim_matches(est_blanc)
}

/// `\d` d'une expression reguliere de Python sur du texte : un chiffre decimal Unicode.
pub(crate) fn est_chiffre(c: char) -> bool {
    dans(DECIMAUX, c)
}

/// `re.compile(r"^[12][0-9]{12}([0-9]{2})?$").match(s)` : `$` accepte un saut de
/// ligne final.
fn est_nir(s: &str) -> bool {
    let t = s.strip_suffix('\n').unwrap_or(s);
    let n = t.chars().count();
    (n == 13 || n == 15) && t.starts_with(['1', '2']) && t.chars().all(|c| c.is_ascii_digit())
}

fn treize(s: &str) -> String {
    s.chars().take(13).collect()
}

fn texte_ou_null(t: &Option<String>) -> Value {
    t.as_ref().map(|s| Value::String(s.clone())).unwrap_or(Value::Null)
}

/// Les valeurs JSON que Python tient pour fausses.
fn faux(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::Bool(b) => !b,
        Value::Number(n) => n.as_f64() == Some(0.0),
        Value::String(s) => s.is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::Object(o) => o.is_empty(),
    }
}

/// Le nom de type d'une valeur, tel que Python l'ecrit dans ses refus.
fn type_py(v: &Value) -> &'static str {
    match v {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::Number(n) => {
            if n.is_f64() {
                "float"
            } else {
                "int"
            }
        }
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
    }
}

// ------------------------------------------------------------------- l'entree

/// Ce que la demande donne a lire : des octets (`fichier`, `base64`) ou un texte.
enum Entree {
    Octets(Vec<u8>),
    Texte(String),
}

impl Entree {
    /// `bytes.decode("utf-8", "replace")`, ou le `str` tel quel.
    fn utf8(&self) -> String {
        match self {
            Entree::Octets(o) => String::from_utf8_lossy(o).into_owned(),
            Entree::Texte(t) => t.clone(),
        }
    }

    /// `bytes.decode("latin-1")`, ou le `str` tel quel.
    fn latin1(&self) -> String {
        match self {
            Entree::Octets(o) => o.iter().map(|&b| b as char).collect(),
            Entree::Texte(t) => t.clone(),
        }
    }

    /// Les octets, un `str` passant par `encode("utf-8")`.
    fn octets(&self) -> Vec<u8> {
        match self {
            Entree::Octets(o) => o.clone(),
            Entree::Texte(t) => t.clone().into_bytes(),
        }
    }
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_ecrire(o: &[u8]) -> String {
    let mut s = String::with_capacity(o.len().div_ceil(3) * 4);
    for bloc in o.chunks(3) {
        let n = (bloc[0] as u32) << 16 | (*bloc.get(1).unwrap_or(&0) as u32) << 8 | *bloc.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= bloc.len() {
                s.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

pub fn base64_lire(t: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(t.len() / 4 * 3);
    let (mut acc, mut n) = (0u32, 0);
    for c in t.bytes().filter(|c| !c.is_ascii_whitespace()) {
        if c == b'=' {
            break;
        }
        let v = ALPHABET.iter().position(|&a| a == c).ok_or("demande illisible : `base64` n'est pas du base64")?;
        acc = acc << 6 | v as u32;
        n += 6;
        if n >= 8 {
            n -= 8;
            out.push((acc >> n & 0xff) as u8);
        }
    }
    Ok(out)
}

fn entree_de(d: &Value) -> Resultat<Entree> {
    if let Some(f) = d.get("fichier").and_then(Value::as_str) {
        return std::fs::read(f).map(Entree::Octets).map_err(|e| format!("{} : {}", f, e).into());
    }
    if let Some(b) = d.get("base64").and_then(Value::as_str) {
        return Ok(Entree::Octets(base64_lire(b)?));
    }
    if let Some(t) = d.get("texte").and_then(Value::as_str) {
        return Ok(Entree::Texte(t.to_string()));
    }
    Err("demande illisible : il faut `fichier`, `base64` ou `texte`".into())
}

// ------------------------------------------------------------------ le XML

/// Un element, comme ElementTree le range.
#[derive(Debug)]
struct Noeud {
    /// `{uri}nom`, ou `nom` sans espace de noms.
    tag: String,
    attributs: Vec<(String, String)>,
    /// `.text` : le texte d'avant le premier enfant.
    texte: String,
    enfants: Vec<usize>,
}

/// L'arbre ; `n` est dans l'ordre du document, la racine d'abord (`.iter()`).
struct Arbre {
    n: Vec<Noeud>,
}

fn local(tag: &str) -> &str {
    tag.rsplit('}').next().unwrap_or(tag)
}

fn nom_etendu(ns: Option<&str>, nom: &str) -> String {
    match ns {
        Some(u) if !u.is_empty() => format!("{{{}}}{}", u, nom),
        _ => nom.to_string(),
    }
}

impl Arbre {
    fn racine(&self) -> &Noeud {
        &self.n[0]
    }

    fn enfants<'a>(&'a self, e: &'a Noeud) -> impl Iterator<Item = &'a Noeud> {
        e.enfants.iter().map(move |&i| &self.n[i])
    }

    /// `Element.iter()` : l'element, puis ses descendants dans l'ordre du document.
    fn sous<'a>(&'a self, e: &'a Noeud) -> impl Iterator<Item = &'a Noeud> {
        let mut pile = vec![e];
        let mut sortie = Vec::new();
        while let Some(x) = pile.pop() {
            sortie.push(x);
            for &i in x.enfants.iter().rev() {
                pile.push(&self.n[i]);
            }
        }
        sortie.into_iter()
    }

    fn tous(&self) -> impl Iterator<Item = &Noeud> {
        self.n.iter()
    }
}

impl Noeud {
    /// `Element.get(nom)`.
    fn get(&self, nom: &str) -> Option<&str> {
        self.attributs.iter().find(|(k, _)| k == nom).map(|(_, v)| v.as_str())
    }

    /// `(e.text or "").strip()`.
    fn texte_net(&self) -> &str {
        nettoye(&self.texte)
    }
}

fn ajouter(a: &mut Arbre, n: roxmltree::Node) -> usize {
    let idx = a.n.len();
    let mut texte = String::new();
    for c in n.children() {
        if c.is_element() {
            break;
        }
        if c.is_text() {
            texte.push_str(c.text().unwrap_or(""));
        }
    }
    a.n.push(Noeud {
        tag: nom_etendu(n.tag_name().namespace(), n.tag_name().name()),
        attributs: n
            .attributes()
            .map(|at| (nom_etendu(at.namespace(), at.name()), at.value().to_string()))
            .collect(),
        texte,
        enfants: Vec::new(),
    });
    let mut enfants = Vec::new();
    for c in n.children().filter(|c| c.is_element()) {
        enfants.push(ajouter(a, c));
    }
    a.n[idx].enfants = enfants;
    idx
}

fn dans(table: &[(u32, u32)], c: char) -> bool {
    let c = c as u32;
    table
        .binary_search_by(|&(a, b)| {
            if c < a {
                std::cmp::Ordering::Greater
            } else if c > b {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

/// Un nom d'element, d'attribut ou d'instruction tel qu'expat le lit (XML 1.0,
/// quatrieme edition ; au plus un deux-points, ni en tete ni en queue).
fn nom_valide(n: &[char]) -> bool {
    let (Some(&premier), Some(&dernier)) = (n.first(), n.last()) else {
        return false;
    };
    premier != ':' && dernier != ':' && n.iter().filter(|&&c| c == ':').count() <= 1 && dans(NOM_DEBUT, premier) && n[1..].iter().all(|&c| c == ':' || dans(NOM_SUITE, c))
}

fn commence_par(c: &[char], i: usize, motif: &str) -> bool {
    motif.chars().enumerate().all(|(k, m)| c.get(i + k) == Some(&m))
}

fn chercher(c: &[char], depuis: usize, motif: &str) -> Option<usize> {
    (depuis..c.len()).find(|&i| commence_par(c, i, motif))
}

fn blanc_xml(c: Option<&char>) -> bool {
    matches!(c, Some(' ' | '\t' | '\r' | '\n'))
}

fn mal_forme(m: &str) -> Echec {
    echec("ParseError", m)
}

/// La declaration `<?xml ...?>` comme expat la verifie : `version` d'abord, puis
/// `encoding` (qui commence par une lettre), puis `standalone` (yes ou no) ; chaque
/// valeur n'a que des lettres, chiffres, point, tiret et soulignement.
fn declaration_valide(d: &[char]) -> bool {
    // (nom, valeur, suite) ; None : plus rien a lire.
    fn pseudo(d: &[char], mut i: usize) -> Option<Option<(String, String, usize)>> {
        if i == d.len() {
            return Some(None);
        }
        if !blanc_xml(d.get(i)) {
            return None;
        }
        while blanc_xml(d.get(i)) {
            i += 1;
        }
        if i == d.len() {
            return Some(None);
        }
        let debut = i;
        loop {
            match d.get(i) {
                None => return None,
                Some('=') => break,
                Some(c) if blanc_xml(Some(c)) => {
                    while blanc_xml(d.get(i)) {
                        i += 1;
                    }
                    if d.get(i) != Some(&'=') {
                        return None;
                    }
                    break;
                }
                _ => i += 1,
            }
        }
        let nom_fin = d[debut..i].iter().position(|&c| c == '=' || blanc_xml(Some(&c))).map(|p| debut + p).unwrap_or(i);
        if i == debut {
            return None;
        }
        let nom: String = d[debut..nom_fin].iter().collect();
        i += 1;
        while blanc_xml(d.get(i)) {
            i += 1;
        }
        let guill = match d.get(i) {
            Some(&q @ ('"' | '\'')) => q,
            _ => return None,
        };
        i += 1;
        let v = i;
        loop {
            match d.get(i) {
                Some(&c) if c == guill => break,
                Some(c) if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') => i += 1,
                _ => return None,
            }
        }
        Some(Some((nom, d[v..i].iter().collect(), i + 1)))
    }
    let Some(Some((nom, _, mut i))) = pseudo(d, 0) else {
        return false;
    };
    if nom != "version" {
        return false;
    }
    let Some(suite) = pseudo(d, i) else {
        return false;
    };
    let Some((mut nom, mut val, mut j)) = suite else {
        return true;
    };
    if nom == "encoding" {
        if !val.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
            return false;
        }
        let Some(suite) = pseudo(d, j) else {
            return false;
        };
        let Some(x) = suite else {
            return true;
        };
        (nom, val, j) = x;
    }
    i = j;
    if nom != "standalone" || !(val == "yes" || val == "no") {
        return false;
    }
    while blanc_xml(d.get(i)) {
        i += 1;
    }
    i == d.len()
}

/// La declaration `<?xml ...?>` de tete, si le texte en commence par une.
fn controle_declaration(c: &[char]) -> Resultat<()> {
    if commence_par(c, 0, "<?xml") && blanc_xml(c.get(5)) {
        if let Some(fin) = chercher(c, 5, "?>") {
            if !declaration_valide(&c[5..fin]) {
                return Err(mal_forme("XML declaration not well-formed"));
            }
        }
    }
    Ok(())
}

/// Ce qu'expat refuse et que `roxmltree` laisse passer : la declaration mal
/// ecrite, un nom hors de XML 1.0 quatrieme edition, une reference a un
/// caractere impossible (`&#xD800;`, `&#x110000;`).
fn controle_expat(s: &str) -> Resultat<()> {
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut pile: Vec<String> = Vec::new();
    controle_declaration(&c)?;
    // Les prefixes declares quelque part : `p:xmlns` n'est lisible que si `p` l'est.
    let declares: std::collections::HashSet<String> = {
        let t: String = c.iter().collect();
        t.match_indices("xmlns:")
            .map(|(k, _)| t[k + 6..].chars().take_while(|x| !blanc_xml(Some(x)) && !matches!(x, '=' | '>' | '/')).collect())
            .collect()
    };
    let vide_apres = |c: &[char], fin: usize| -> bool {
        let mut k = fin;
        while blanc_xml(c.get(k)) {
            k += 1;
        }
        if c.get(k) != Some(&'=') {
            return false;
        }
        k += 1;
        while blanc_xml(c.get(k)) {
            k += 1;
        }
        matches!((c.get(k), c.get(k + 1)), (Some('"'), Some('"')) | (Some('\''), Some('\'')))
    };
    let nom_a = |i: usize, c: &[char], fin: &dyn Fn(char) -> bool| -> usize { (i..c.len()).find(|&k| fin(c[k])).unwrap_or(c.len()) };
    let reference = |c: &[char], i: usize| -> Resultat<()> {
        // `&#` en c[i..] : chiffres decimaux, ou `x` et chiffres hexadecimaux, puis `;`.
        let (base, mut k) = if c.get(i + 2) == Some(&'x') { (16, i + 3) } else { (10, i + 2) };
        let debut = k;
        let mut v: u64 = 0;
        while let Some(d) = c.get(k).and_then(|x| x.to_digit(base)).filter(|_| c[k].is_ascii_hexdigit()) {
            v = (v * base as u64 + d as u64).min(0x1FFFFF);
            k += 1;
        }
        if k > debut && c.get(k) == Some(&';') && ((0xD800..0xE000).contains(&v) || v > 0x10FFFF) {
            return Err(mal_forme("reference to invalid character number"));
        }
        Ok(())
    };
    while i < c.len() {
        if c[i] == '&' && c.get(i + 1) == Some(&'#') {
            reference(&c, i)?;
            i += 2;
            continue;
        }
        if c[i] != '<' {
            i += 1;
            continue;
        }
        if commence_par(&c, i, "<!--") {
            i = chercher(&c, i + 4, "-->").map(|k| k + 3).unwrap_or(c.len());
        } else if commence_par(&c, i, "<![CDATA[") {
            i = chercher(&c, i + 9, "]]>").map(|k| k + 3).unwrap_or(c.len());
        } else if commence_par(&c, i, "<?") {
            let fin = nom_a(i + 2, &c, &|x| blanc_xml(Some(&x)) || x == '?');
            let cible = &c[i + 2..fin];
            let est_declaration = i == 0 && cible == ['x', 'm', 'l'] && blanc_xml(c.get(fin));
            let xml = cible.len() == 3 && cible.iter().collect::<String>().eq_ignore_ascii_case("xml");
            if (!nom_valide(cible) || cible.contains(&':') || (xml && !est_declaration)) || !(blanc_xml(c.get(fin)) || commence_par(&c, fin, "?>")) {
                return Err(mal_forme("not well-formed (invalid token)"));
            }
            i = chercher(&c, fin, "?>").map(|k| k + 2).unwrap_or(c.len());
        } else if commence_par(&c, i, "<!") {
            // DOCTYPE : son nom se verifie, on saute le reste, crochets et guillemets compris.
            let (mut profondeur, mut guill) = (0i32, None);
            i += 2;
            if commence_par(&c, i, "DOCTYPE") {
                let mut k = i + 7;
                while blanc_xml(c.get(k)) {
                    k += 1;
                }
                let fin = nom_a(k, &c, &|x| blanc_xml(Some(&x)) || matches!(x, '[' | '>'));
                if !nom_valide(&c[k..fin]) {
                    return Err(mal_forme("not well-formed (invalid token)"));
                }
            }
            while i < c.len() {
                if c[i] == '&' && c.get(i + 1) == Some(&'#') {
                    reference(&c, i)?;
                }
                match (c[i], guill) {
                    (q, Some(g)) if q == g => guill = None,
                    (_, Some(_)) => {}
                    ('"' | '\'', None) => guill = Some(c[i]),
                    ('[', None) => profondeur += 1,
                    (']', None) => profondeur -= 1,
                    ('<', None) if commence_par(&c, i, "<!--") => {
                        i = chercher(&c, i + 4, "-->").map(|k| k + 2).unwrap_or(c.len());
                    }
                    ('>', None) if profondeur <= 0 => {
                        i += 1;
                        break;
                    }
                    _ => {}
                }
                i += 1;
            }
        } else if commence_par(&c, i, "</") {
            let fin = nom_a(i + 2, &c, &|x| blanc_xml(Some(&x)) || x == '>');
            let nom: String = c[i + 2..fin].iter().collect();
            if pile.pop().is_some_and(|ouvert| ouvert != nom) {
                return Err(mal_forme("mismatched tag"));
            }
            i = chercher(&c, i + 2, ">").map(|k| k + 1).unwrap_or(c.len());
        } else {
            let fin = nom_a(i + 1, &c, &|x| blanc_xml(Some(&x)) || x == '/' || x == '>');
            if fin > i + 1 && !nom_valide(&c[i + 1..fin]) {
                return Err(mal_forme("not well-formed (invalid token)"));
            }
            let nom_balise: String = c[i + 1..fin].iter().collect();
            i = fin;
            // les attributs
            loop {
                while blanc_xml(c.get(i)) {
                    i += 1;
                }
                match c.get(i) {
                    None => break,
                    Some('>') => {
                        i += 1;
                        break;
                    }
                    Some('/') => i += 1,
                    Some(_) => {
                        let fin = nom_a(i, &c, &|x| blanc_xml(Some(&x)) || matches!(x, '=' | '>' | '/'));
                        if fin == i {
                            break;
                        }
                        if !nom_valide(&c[i..fin]) {
                            return Err(mal_forme("not well-formed (invalid token)"));
                        }
                        let nom_attr: String = c[i..fin].iter().collect();
                        if let Some((prefixe, local)) = nom_attr.split_once(':') {
                            if (prefixe == "xmlns" && local == "xmlns") || (local == "xmlns" && prefixe != "xmlns" && !declares.contains(prefixe) && prefixe != "xml") {
                                return Err(mal_forme("unbound prefix"));
                            }
                            if prefixe == "xmlns" && vide_apres(&c, fin) {
                                return Err(mal_forme("must not undeclare prefix"));
                            }
                        }
                        i = fin;
                        while blanc_xml(c.get(i)) {
                            i += 1;
                        }
                        if c.get(i) != Some(&'=') {
                            break;
                        }
                        i += 1;
                        while blanc_xml(c.get(i)) {
                            i += 1;
                        }
                        let Some(&q @ ('"' | '\'')) = c.get(i) else {
                            break;
                        };
                        i += 1;
                        while i < c.len() && c[i] != q {
                            if c[i] == '&' && c.get(i + 1) == Some(&'#') {
                                reference(&c, i)?;
                            }
                            i += 1;
                        }
                        i += 1;
                    }
                }
            }
            if i >= 2 && c.get(i - 1) == Some(&'>') && c[i - 2] != '/' {
                pile.push(nom_balise);
            }
        }
    }
    Ok(())
}

/// `ET.fromstring(texte)` pour un `str` : la declaration d'encodage est sans effet.
fn lire_texte(s: &str) -> Resultat<Arbre> {
    lire_texte_nu(s.strip_prefix('\u{feff}').unwrap_or(s))
}

/// Le texte sans son BOM : un second est un caractere de tete que XML refuse.
fn lire_texte_nu(s: &str) -> Resultat<Arbre> {
    if s.starts_with('\u{feff}') {
        return Err(mal_forme("not well-formed (invalid token)"));
    }
    controle_expat(s)?;
    let options = roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() };
    let doc = roxmltree::Document::parse_with_options(s, options).map_err(|e| echec("ParseError", e.to_string()))?;
    let mut a = Arbre { n: Vec::new() };
    ajouter(&mut a, doc.root_element());
    Ok(a)
}

/// L'encodage declare par `<?xml ... encoding="..."?>` en tete du texte.
fn encodage_declare(t: &str) -> Option<String> {
    if !t.starts_with("<?xml") {
        return None;
    }
    let fin = t.find("?>")?;
    let decl = &t[..fin];
    let i = decl.find("encoding")?;
    let reste = decl[i + 8..].trim_start();
    let reste = reste.strip_prefix('=')?.trim_start();
    let guill = reste.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let reste = &reste[1..];
    let j = reste.find(guill)?;
    Some(reste[..j].to_string())
}

/// `encodings.normalize_encoding`, en minuscules.
fn normaliser(nom: &str) -> String {
    let (mut out, mut ponct) = (String::new(), false);
    for c in nom.to_lowercase().chars() {
        if c.is_alphanumeric() || c == '.' {
            if ponct && !out.is_empty() {
                out.push('_');
            }
            if c.is_ascii() {
                out.push(c);
            }
            ponct = false;
        } else {
            ponct = true;
        }
    }
    out
}

/// Ce que `pyexpat` fait d'un encodage declare.
#[derive(PartialEq)]
enum Codage {
    Utf8,
    Latin1,
    Ascii,
    Cp1252,
    /// `UTF-16` (`None`), `UTF-16LE` (`Some(true)`), `UTF-16BE` (`Some(false)`).
    Utf16(Option<bool>),
    /// Un nom que Python connait mais qui n'est pas sur 256 caracteres (multi-octets) :
    /// `pyexpat` leve une `ValueError`.
    Refuse,
    /// Une autre table sur 256 caracteres, que ce moteur ne porte pas : `ParseError`.
    Autre,
    /// Un nom que Python ne connait pas : `LookupError`.
    Introuvable,
}

fn codage(nom: &str) -> Codage {
    match nom.to_ascii_lowercase().as_str() {
        "utf-8" => return Codage::Utf8,
        "iso-8859-1" => return Codage::Latin1,
        "us-ascii" => return Codage::Ascii,
        "utf-16" => return Codage::Utf16(None),
        "utf-16le" => return Codage::Utf16(Some(true)),
        "utf-16be" => return Codage::Utf16(Some(false)),
        _ => {}
    }
    let n = normaliser(nom);
    let a = |t: &[&str]| t.binary_search(&n.as_str()).is_ok();
    if a(CODECS_LATIN1) {
        Codage::Latin1
    } else if a(CODECS_ASCII) {
        Codage::Ascii
    } else if a(CODECS_CP1252) {
        Codage::Cp1252
    } else if a(CODECS_REFUSES) {
        Codage::Refuse
    } else if a(CODECS_AUTRES) {
        Codage::Autre
    } else {
        Codage::Introuvable
    }
}

fn erreur_codage(c: &Codage, nom: &str) -> Echec {
    match c {
        Codage::Introuvable => echec("LookupError", format!("unknown encoding: {}", nom)),
        Codage::Refuse => echec("ValueError", "multi-byte encodings are not supported"),
        _ => echec("ParseError", "unknown encoding"),
    }
}

/// `ET.fromstring(octets)` : l'encodage declare est celui qui decode.
fn lire_octets(o: &[u8]) -> Resultat<Arbre> {
    let invalide = || echec("ParseError", "not well-formed (invalid token)");
    let incorrect = || echec("ParseError", "encoding specified in XML declaration is incorrect");
    let (o, bom8) = match o.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        Some(r) => (r, true),
        None => (o, false),
    };
    // UTF-16, comme expat le devine : un BOM, sinon un premier octet nul (gros
    // boutiste) ou un deuxieme (petit boutiste).
    let utf16 = if bom8 || o.len() < 2 {
        None
    } else if o.starts_with(&[0xFF, 0xFE]) {
        Some((true, 2))
    } else if o.starts_with(&[0xFE, 0xFF]) {
        Some((false, 2))
    } else if o[0] == 0 {
        Some((false, 0))
    } else if o[1] == 0 {
        Some((true, 0))
    } else {
        None
    };
    if let Some((le, saut)) = utf16 {
        let unites: Vec<u16> = o[saut..]
            .chunks_exact(2)
            .map(|p| if le { u16::from_le_bytes([p[0], p[1]]) } else { u16::from_be_bytes([p[0], p[1]]) })
            .collect();
        let t = String::from_utf16_lossy(&unites);
        controle_declaration(&t.chars().take(512).collect::<Vec<_>>())?;
        if let Some(n) = encodage_declare(&t) {
            match codage(&n) {
                Codage::Utf16(None) => {}
                Codage::Utf16(Some(l)) if l == le => {}
                c @ (Codage::Introuvable | Codage::Refuse) => return Err(erreur_codage(&c, &n)),
                _ => return Err(incorrect()),
            }
        }
        if (o.len() - saut) % 2 != 0 {
            return Err(invalide());
        }
        let t = String::from_utf16(&unites).map_err(|_| invalide())?;
        return lire_texte_nu(&t);
    }
    // Les octets d'une declaration valide sont de l'ASCII : on la lit telle quelle.
    let debut = String::from_utf8_lossy(&o[..o.len().min(512)]).into_owned();
    controle_declaration(&debut.chars().collect::<Vec<_>>())?;
    let nom = encodage_declare(&debut);
    let c = match &nom {
        Some(n) => codage(n),
        None => Codage::Utf8,
    };
    let texte = match c {
        Codage::Utf8 => std::str::from_utf8(o).map_err(|_| invalide())?.to_string(),
        Codage::Latin1 => o.iter().map(|&b| b as char).collect(),
        Codage::Ascii => {
            if !o.is_ascii() {
                return Err(invalide());
            }
            String::from_utf8_lossy(o).into_owned()
        }
        Codage::Cp1252 => {
            let mut t = String::with_capacity(o.len());
            for &b in o {
                match if b < 0x80 { b as u32 } else { WINDOWS_1252[b as usize - 0x80] } {
                    0xFFFD => return Err(invalide()),
                    u => t.push(char::from_u32(u).unwrap_or('\u{fffd}')),
                }
            }
            t
        }
        Codage::Utf16(_) => return Err(incorrect()),
        Codage::Refuse | Codage::Autre | Codage::Introuvable => return Err(erreur_codage(&c, &nom.unwrap_or_default())),
    };
    lire_texte_nu(&texte)
}

// ------------------------------------------------------- les bilans de depot

const CHAMPS_BILAN: [&str; 16] = [
    "flux",
    "recu_le",
    "essai_reel",
    "etat_envoi",
    "etat_declaration",
    "mois",
    "logiciel",
    "type_rapport",
    "type_declaration",
    "fraction",
    "ordre",
    "salaries",
    "contrats",
    "conformite",
    "certificat",
    "siret_declarant",
];

/// `'01082026'` -> `'2026-08'`.
fn mois_lisible(brut: &str) -> String {
    let c: Vec<char> = nettoye(brut).chars().collect();
    if c.len() == 8 && c[0] == '0' && c[1] == '1' && c[2..].iter().all(|&x| est_chiffre(x)) {
        let mm: String = c[2..4].iter().collect();
        let aaaa: String = c[4..8].iter().collect();
        return format!("{}-{}", aaaa, mm);
    }
    String::new()
}

fn bilan_depuis(champs: &[(String, String)], type_rapport: &str) -> Option<Value> {
    let get = |k: &str| champs.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str()).unwrap_or("");
    let flux = get("idflux");
    if flux.is_empty() {
        return None;
    }
    let (d, h) = (get("date_reception"), get("heure_reception"));
    let recu_le = if !d.is_empty() && !h.is_empty() { format!("{}T{}", d, h) } else { d.to_string() };
    let etat_declaration = if !get("etat").is_empty() { get("etat") } else { get("etat_conformite") };
    let valeurs = [
        flux.to_string(),
        recu_le,
        get("essai_reel").to_string(),
        get("envoi_etat").to_string(),
        etat_declaration.to_string(),
        mois_lisible(get("mois")),
        get("nom_logiciel").to_string(),
        type_rapport.to_string(),
        get("type").to_string(),
        get("fraction").to_string(),
        get("ordre").to_string(),
        get("salaries").to_string(),
        get("contrats").to_string(),
        get("etat_conformite").to_string(),
        get("certificat_conformite").to_string(),
        get("siret").to_string(),
    ];
    let mut m = Map::new();
    for (c, v) in CHAMPS_BILAN.iter().zip(valeurs) {
        m.insert((*c).into(), Value::String(v));
    }
    Some(Value::Object(m))
}

/// `parse_bilan_depot` : un rapport AEE, CCO ou BAN, ou `None` si ce n'en est pas un.
fn parse_bilan_depot(e: &Entree) -> Value {
    parse_bilan_depot_lu(e).unwrap_or(Value::Null)
}

/// `parse_bilan_depot`, mais un XML qui ne se lit pas est un refus (sa classe), pas « pas un bilan » : la reprise du
/// journal le nomme (`illisibles`). Un XML bien forme qui n'est pas un rapport de depot rend `Null`.
fn parse_bilan_depot_lu(e: &Entree) -> Resultat<Value> {
    let texte = e.utf8();
    let arbre = lire_texte(nettoye(&texte))?;
    let racine = arbre.racine();
    if local(&racine.tag) != "rapport" {
        return Ok(Value::Null);
    }
    let Some(type_rapport) = racine.get("type").filter(|t| RAPPORTS_DEPOT.contains(t)) else {
        return Ok(Value::Null);
    };
    // Un balayage a plat, premiere occurrence gagnante.
    let mut champs: Vec<(String, String)> = Vec::new();
    for el in arbre.tous() {
        let nom = local(&el.tag);
        let t = el.texte_net();
        if !t.is_empty() && !champs.iter().any(|(n, _)| n == nom) {
            champs.push((nom.to_string(), t.to_string()));
        }
    }
    Ok(bilan_depuis(&champs, type_rapport).unwrap_or(Value::Null))
}

/// Les declarations qu'un certificat de conformite ou un bilan d'anomalies (nature 11, rapport `CCO` ou `BAN`) identifie,
/// une par `declaration` : `nature`, `type`, `ordre` (S20.G00.05.001, .002, .004), `siren`, `nic` (d'affectation),
/// `etat`, `essai_reel` (de l'envoi : 01 essai, 02 reel) et les `codes` de ses anomalies. C'est TOUT ce que la plate-forme
/// dit d'une declaration : ni salarie, ni contrat, ni date d'evenement (schema des bilans `v01r08`, ceux qu'elle rend ;
/// specification VT2.2, `prive/reference/sources/gip-mds-api-dsn/` ; PASSATION-O1, point 1). Un XML illisible, ou qui
/// n'est pas un rapport de depot, n'en identifie aucune.
pub fn declarations_du_bilan(o: &[u8]) -> Vec<Value> {
    let Ok(arbre) = lire_octets(o) else { return Vec::new() };
    let racine = arbre.racine();
    if local(&racine.tag) != "rapport" || !racine.get("type").is_some_and(|t| RAPPORTS_DEPOT.contains(&t)) {
        return Vec::new();
    }
    let essai_reel = arbre.tous().find(|e| local(&e.tag) == "essai_reel").map(|e| e.texte_net().to_string()).unwrap_or_default();
    let mut out = Vec::new();
    for d in arbre.tous().filter(|e| local(&e.tag) == "declaration") {
        let (mut champs, mut codes): (Vec<(&str, &str)>, Vec<Value>) = (Vec::new(), Vec::new());
        for x in arbre.sous(d) {
            let (nom, t) = (local(&x.tag), x.texte_net());
            if nom == "code" {
                codes.push(json!(t));
            } else if !t.is_empty() && !champs.iter().any(|(n, _)| *n == nom) {
                champs.push((nom, t));
            }
        }
        let get = |k: &str| champs.iter().find(|(n, _)| *n == k).map_or("", |(_, v)| *v);
        out.push(json!({
            "nature": get("nature"), "type": get("type"), "ordre": get("ordre"), "siren": get("SIREN"),
            "nic": get("nic_affectation"), "etat": get("etat"), "essai_reel": essai_reel, "codes": codes,
        }));
    }
    out
}

/// L'arbre d'octets ; un XML mal forme n'en est pas un (`None`), tout autre refus (un
/// encodage inconnu) remonte.
fn octets_arbre(e: &Entree) -> Resultat<Option<Arbre>> {
    match lire_octets(&e.octets()) {
        Ok(a) => Ok(Some(a)),
        Err(Echec { classe: Some("ParseError"), .. }) => Ok(None),
        Err(autre) => Err(autre),
    }
}

/// `anomalies_du_bilan` : les motifs d'un BAN, en phrases lisibles.
fn anomalies_du_bilan(e: &Entree) -> Resultat<Vec<Value>> {
    let Some(arbre) = octets_arbre(e)? else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for a in arbre.tous() {
        if local(&a.tag) != "declaration_anomalie" {
            continue;
        }
        let (mut message, mut categorie, mut nom) = ("", "", "");
        for x in arbre.sous(a) {
            match local(&x.tag) {
                "message" => message = x.texte_net(),
                "categorie" => categorie = x.texte_net(),
                "nom_famille" => nom = x.texte_net(),
                _ => {}
            }
        }
        if !message.is_empty() {
            let cat = if categorie.is_empty() { String::new() } else { format!(" ({})", categorie) };
            let qui = if nom.is_empty() { String::new() } else { format!(" -- {}", nom) };
            out.push(Value::String(format!("{}{}{}", message, cat, qui)));
        }
    }
    Ok(out)
}

/// `anomalies_codees` : les anomalies d'un bilan, en codes.
fn anomalies_codees(e: &Entree) -> Resultat<Vec<Value>> {
    let Some(arbre) = octets_arbre(e)? else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for a in arbre.tous() {
        if !matches!(local(&a.tag), "declaration_anomalie" | "envoi_anomalie") {
            continue;
        }
        let (mut code, mut categorie, mut message, mut rubrique) = (String::new(), String::new(), String::new(), String::new());
        for x in arbre.sous(a) {
            let t = x.texte_net();
            match local(&x.tag) {
                "code" if code.is_empty() => code = t.to_string(),
                "categorie" if categorie.is_empty() => categorie = t.to_string(),
                "message" if message.is_empty() => message = t.to_string(),
                "identifiant" if rubrique.is_empty() => rubrique = t.to_string(),
                _ => {}
            }
        }
        if !code.is_empty() || !message.is_empty() {
            out.push(json!({"code": code, "categorie": categorie, "message": message, "rubrique": rubrique}));
        }
    }
    Ok(out)
}

/// `fusionner_bilans` : un seul bilan par flux, le plus complet. Le bilan de
/// declaration (CCO, BAN) l'emporte sur l'accuse de transport (AEE).
pub fn fusionner_bilans(bilans: &[Value]) -> Value {
    if bilans.is_empty() {
        return Value::Null;
    }
    let rang = |b: &Value| match b.get("type_rapport").and_then(Value::as_str) {
        Some("CCO" | "BAN") => 0,
        Some("AEE") => 1,
        _ => 2,
    };
    let mut tries: Vec<&Value> = bilans.iter().collect();
    tries.sort_by_key(|b| rang(b));
    let mut base = tries[0].clone();
    for autre in &tries[1..] {
        for champ in [
            "recu_le",
            "essai_reel",
            "etat_envoi",
            "etat_declaration",
            "mois",
            "logiciel",
            "type_declaration",
            "fraction",
            "ordre",
            "salaries",
            "contrats",
            "conformite",
            "certificat",
            "siret_declarant",
        ] {
            if base.get(champ).map(faux).unwrap_or(true) {
                if let Some(v) = autre.get(champ) {
                    base[champ] = v.clone();
                }
            }
        }
    }
    base
}

// ------------------------------------------------ reponses de recherche (liste)

fn json_lire(texte: &str) -> Resultat<Value> {
    serde_json::from_str(texte).map_err(|e| echec("JSONDecodeError", e.to_string()))
}

fn attribut_inconnu(v: &Value) -> Echec {
    echec("AttributeError", crate::fichiers::attendu("un objet", type_py(v)))
}

/// `d.get(cle, d)` puis `.get(...)` : Python refuse un non-dictionnaire.
fn lire_cle<'a>(v: &'a Value, cle: &str) -> Resultat<Option<&'a Value>> {
    match v {
        Value::Object(m) => Ok(m.get(cle)),
        autre => Err(attribut_inconnu(autre)),
    }
}

/// `x` si c'est un dictionnaire, la liste de ses elements si c'est une liste ; ce
/// qu'un `for` de Python lit sinon.
fn elements(v: &Value) -> Resultat<Vec<Value>> {
    match v {
        Value::Array(a) => Ok(a.clone()),
        Value::String(s) => Ok(s.chars().map(|c| Value::String(c.to_string())).collect()),
        Value::Object(m) => Ok(m.keys().map(|k| Value::String(k.clone())).collect()),
        autre => Err(echec("TypeError", crate::fichiers::attendu("une liste", type_py(autre)))),
    }
}

/// `dict(r)` : un dictionnaire tel quel, une suite de paires (deux caracteres ou
/// deux elements) sinon.
fn dict_de(r: &Value) -> Resultat<Map<String, Value>> {
    let elements: Vec<Value> = match r {
        Value::Object(m) => return Ok(m.clone()),
        Value::Array(a) => a.clone(),
        Value::String(s) => s.chars().map(|c| Value::String(c.to_string())).collect(),
        autre => return Err(echec("TypeError", crate::fichiers::attendu("une liste", type_py(autre)))),
    };
    let mut m = Map::new();
    for (i, e) in elements.iter().enumerate() {
        let paire: Vec<Value> = match e {
            Value::Array(a) => a.clone(),
            Value::String(s) => s.chars().map(|c| Value::String(c.to_string())).collect(),
            Value::Object(o) => o.keys().map(|k| Value::String(k.clone())).collect(),
            _ => return Err(echec("TypeError", crate::fichiers::pas_une_paire(i, type_py(e)))),
        };
        if paire.len() != 2 {
            return Err(echec("ValueError", crate::fichiers::paire_attendue(i, paire.len())));
        }
        let Value::String(k) = &paire[0] else {
            return Err(echec("TypeError", "unsupported dictionary key"));
        };
        m.insert(k.clone(), paire[1].clone());
    }
    Ok(m)
}

fn liste_retours_json(d: &Value) -> Resultat<Vec<Value>> {
    let racine = lire_cle(d, "retours")?.unwrap_or(d);
    let flux = lire_cle(racine, "flux")?.cloned().unwrap_or(json!([]));
    let flux = if flux.is_object() { vec![flux] } else { elements(&flux)? };
    let mut out = Vec::new();
    for f in &flux {
        let idf = lire_cle(f, "id")?.cloned().unwrap_or(Value::Null);
        let retours = lire_cle(f, "retour")?.cloned().unwrap_or(json!([]));
        let retours = if retours.is_object() { vec![retours] } else { elements(&retours)? };
        for r in &retours {
            // `dict(r, flux=idf)`
            let mut m = dict_de(r)?;
            m.insert("flux".into(), idf.clone());
            out.push(Value::Object(m));
        }
    }
    Ok(out)
}

/// `{local(c.tag): (c.text or "").strip() for c in r}`
fn enfants_en_dict(a: &Arbre, e: &Noeud) -> Map<String, Value> {
    let mut m = Map::new();
    for c in a.enfants(e) {
        m.insert(local(&c.tag).to_string(), Value::String(c.texte_net().to_string()));
    }
    m
}

/// `parse_liste_retours` : XML (1.0) ou JSON (2.0).
fn parse_liste_retours(e: &Entree) -> Resultat<Vec<Value>> {
    let texte = e.utf8();
    let texte = nettoye(&texte);
    if texte.starts_with('{') {
        return liste_retours_json(&json_lire(texte)?);
    }
    let arbre = lire_texte(texte)?;
    let mut out = Vec::new();
    for flux in arbre.tous() {
        if local(&flux.tag) != "flux" {
            continue;
        }
        let idf = match flux.get("id").filter(|s| !s.is_empty()) {
            Some(i) => Value::String(i.to_string()),
            None => arbre
                .enfants(flux)
                .find(|c| local(&c.tag) == "id")
                .map(|c| Value::String(c.texte_net().to_string()))
                .unwrap_or(Value::Null),
        };
        for r in arbre.enfants(flux) {
            if local(&r.tag) != "retour" {
                continue;
            }
            let mut m = enfants_en_dict(&arbre, r);
            m.insert("flux".into(), idf.clone());
            out.push(Value::Object(m));
        }
    }
    Ok(out)
}

/// `parse_liste_retours(octets)` : les retours que la plate-forme liste.
pub(crate) fn liste_retours_d_octets(o: &[u8]) -> Resultat<Vec<Value>> {
    parse_liste_retours(&Entree::Octets(o.to_vec()))
}

/// `parse_liste_depots(octets)` : les depots que la plate-forme liste.
pub(crate) fn liste_depots_d_octets(o: &[u8]) -> Resultat<Vec<Value>> {
    parse_liste_depots(&Entree::Octets(o.to_vec()))
}

/// `parse_liste_depots` : depots > declarant > depot{idflux, date, statut}.
fn parse_liste_depots(e: &Entree) -> Resultat<Vec<Value>> {
    let texte = e.utf8();
    let texte = nettoye(&texte);
    if texte.starts_with('{') {
        let d = json_lire(texte)?;
        let depots = lire_cle(&d, "depots")?.unwrap_or(&d);
        let decl = lire_cle(depots, "declarant")?.cloned().unwrap_or(json!({}));
        let depot = lire_cle(&decl, "depot")?.cloned().unwrap_or(json!([]));
        return if depot.is_object() { Ok(vec![depot]) } else { elements(&depot) };
    }
    let arbre = lire_texte(texte)?;
    Ok(arbre.tous().filter(|el| local(&el.tag) == "depot").map(|el| Value::Object(enfants_en_dict(&arbre, el))).collect())
}

// ------------------------------------------------------- CRM 94 : taux du PAS

#[derive(Clone, Default)]
struct Entree94 {
    nir: Option<String>,
    taux: Option<String>,
    type_: Option<String>,
    identifiant: Option<String>,
}

impl Entree94 {
    fn complete(&self) -> bool {
        self.identifiant.is_some() && self.nir.is_some()
    }

    fn champ(&mut self, nom: &str) -> &mut Option<String> {
        match nom {
            "taux" => &mut self.taux,
            "type" => &mut self.type_,
            _ => &mut self.identifiant,
        }
    }

    fn json(&self) -> Value {
        json!({"nir": texte_ou_null(&self.nir), "taux": texte_ou_null(&self.taux),
               "type": texte_ou_null(&self.type_), "identifiant": texte_ou_null(&self.identifiant)})
    }
}

/// `_plat` : (nom local, valeurs des attributs, texte net) de chaque element.
struct Plat {
    nom: String,
    attribs: Vec<(String, String)>,
    texte: String,
}

fn a_plat(a: &Arbre) -> Vec<Plat> {
    a.tous()
        .map(|e| Plat { nom: local(&e.tag).to_string(), attribs: e.attributs.clone(), texte: e.texte_net().to_string() })
        .collect()
}

fn valeurs_de(p: &Plat) -> Vec<&str> {
    p.attribs.iter().map(|(_, v)| v.as_str()).chain(std::iter::once(p.texte.as_str())).collect()
}

fn rubrique_pas(v: &str) -> Option<&'static str> {
    RUBRIQUES_PAS.iter().find(|(k, _)| *k == v).map(|(_, c)| *c)
}

fn pas_rapport_crm(a: &Arbre) -> Vec<Value> {
    let racine = a.racine();
    if local(&racine.tag) != "rapport" {
        return Vec::new();
    }
    let Some(identifiant) = racine.get("identifiant").filter(|s| !s.is_empty()) else {
        return Vec::new();
    };
    let mut releves = Vec::new();
    for el in a.tous() {
        if local(&el.tag) != "salarie" {
            continue;
        }
        let champs = enfants_en_dict(a, el);
        let lu = |k: &str| champs.get(k).and_then(Value::as_str).unwrap_or("");
        let nir = lu("NIR");
        if nir.is_empty() {
            continue;
        }
        let taux = lu("taux_imposition_PAS");
        if !taux.is_empty() {
            releves.push(json!({"nir": nir, "taux": taux, "type": "01", "identifiant": identifiant}));
        } else {
            releves.push(json!({"nir": nir, "taux": "0.00", "type": "13", "identifiant": "-1"}));
        }
    }
    releves
}

fn pas_par_code(a: &Arbre) -> Vec<Value> {
    let elements = a_plat(a);
    const RUB: &str = "S21.G00.50.008";
    // `str(attribs) + texte` : le dictionnaire s'ecrit avec ses cles et ses valeurs.
    let present = elements
        .iter()
        .any(|p| p.texte.contains(RUB) || p.attribs.iter().any(|(k, v)| k.contains(RUB) || v.contains(RUB)));
    if !present {
        return Vec::new();
    }
    let (mut releves, mut cur) = (Vec::new(), Entree94::default());
    let mut attente: Option<&str> = None;
    for p in &elements {
        let valeurs = valeurs_de(p);
        for v in &valeurs {
            if est_nir(v) {
                if cur.complete() {
                    releves.push(std::mem::take(&mut cur));
                }
                cur.nir = Some(treize(v));
            }
        }
        let mut trouve = false;
        for v in &valeurs {
            if let Some(champ) = rubrique_pas(v) {
                attente = Some(champ);
                if !p.texte.is_empty() && rubrique_pas(&p.texte).is_none() {
                    *cur.champ(champ) = Some(p.texte.clone());
                    attente = None;
                }
                trouve = true;
                break;
            }
        }
        if !trouve {
            if let (Some(champ), false) = (attente, p.texte.is_empty()) {
                *cur.champ(champ) = Some(p.texte.clone());
                attente = None;
            }
        }
        if cur.complete() && cur.taux.is_some() && cur.type_.is_some() {
            releves.push(std::mem::take(&mut cur));
        }
    }
    if cur.complete() {
        releves.push(cur);
    }
    releves.iter().filter(|r| r.complete()).map(Entree94::json).collect()
}

fn pas_par_nom_metier(a: &Arbre) -> Vec<Value> {
    let elements = a_plat(a);
    let metier = |nom: &str| NOMS_METIER.iter().find(|(k, _)| *k == nom).map(|(_, c)| *c);
    if !elements.iter().any(|p| metier(&p.nom.to_lowercase()).is_some()) {
        return Vec::new();
    }
    let (mut releves, mut cur) = (Vec::new(), Entree94::default());
    for p in &elements {
        for v in valeurs_de(p) {
            if est_nir(v) {
                if cur.complete() {
                    releves.push(std::mem::take(&mut cur));
                }
                cur.nir = Some(treize(v));
            }
        }
        if let (Some(champ), false) = (metier(&p.nom.to_lowercase()), p.texte.is_empty()) {
            if cur.champ(champ).is_some() {
                let nir = cur.nir.clone();
                releves.push(std::mem::replace(&mut cur, Entree94 { nir, ..Default::default() }));
            }
            *cur.champ(champ) = Some(p.texte.clone());
        }
    }
    if cur.complete() {
        releves.push(cur);
    }
    releves.iter().filter(|r| r.complete()).map(Entree94::json).collect()
}

/// `re.fullmatch(r"[0-9]{1,2}\.[0-9]{2}", v)`
fn est_taux(v: &str) -> bool {
    let c: Vec<char> = v.chars().collect();
    let Some(p) = c.iter().position(|&x| x == '.') else {
        return false;
    };
    (1..=2).contains(&p) && c.len() == p + 3 && c[..p].iter().all(char::is_ascii_digit) && c[p + 1..].iter().all(char::is_ascii_digit)
}

fn pas_par_balayage(a: &Arbre) -> Vec<Value> {
    let valeurs: Vec<String> = a_plat(a)
        .iter()
        .flat_map(|p| valeurs_de(p).into_iter().filter(|v| !v.is_empty()).map(str::to_string).collect::<Vec<_>>())
        .collect();
    let (mut releves, mut cur) = (Vec::new(), Entree94::default());
    for v in &valeurs {
        if est_nir(v) {
            if cur.complete() {
                releves.push(cur);
            }
            cur = Entree94 { nir: Some(treize(v)), ..Default::default() };
        } else if cur.nir.as_deref().is_some_and(|n| !n.is_empty()) {
            if cur.taux.is_none() && est_taux(v) {
                cur.taux = Some(v.clone());
            } else if cur.type_.is_none() && ["01", "13", "17", "23", "27", "33", "37", "99"].contains(&v.as_str()) {
                cur.type_ = Some(v.clone());
            } else if cur.identifiant.is_none() && (v == "-1" || ((1..=18).contains(&v.chars().count()) && v.chars().all(|c| c.is_ascii_digit()))) {
                cur.identifiant = Some(v.clone());
            }
        }
    }
    if cur.complete() {
        releves.push(cur);
    }
    releves.iter().filter(|r| r.complete()).map(Entree94::json).collect()
}

/// `extraire_taux_pas` : les quatre strategies, de la plus sure a la plus large.
fn extraire_taux_pas(e: &Entree) -> Resultat<Value> {
    let texte = e.utf8();
    let arbre = lire_texte(&texte)?;
    let strategies: [(&str, fn(&Arbre) -> Vec<Value>); 4] = [
        ("rapport CRM nominatif", pas_rapport_crm),
        ("rubriques codees", pas_par_code),
        ("noms metier NEODeS", pas_par_nom_metier),
        ("balayage large", pas_par_balayage),
    ];
    for (nom, f) in strategies {
        let releves = f(&arbre);
        if !releves.is_empty() {
            return Ok(json!({"strategie": nom, "releves": releves}));
        }
    }
    Ok(json!({"strategie": null, "releves": []}))
}

/// `releves_vers_salaries` : chaque releve a une fiche par le NIR ; `nirs` est la
/// liste `[cle, nir]` des fiches qui en ont un, dans l'ordre du fichier.
fn releves_vers_salaries(releves: &[Value], nirs: &[Value]) -> Value {
    let mut par_nir: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    for p in nirs {
        if let (Some(c), Some(n)) = (p.get(0).and_then(Value::as_str), p.get(1).and_then(Value::as_str)) {
            par_nir.insert(n, c);
        }
    }
    let (mut ok, mut orphelins) = (Vec::new(), Vec::new());
    for r in releves {
        let nir = r.get("nir").filter(|n| !faux(n)).and_then(Value::as_str).unwrap_or("");
        match par_nir.get(treize(nir).as_str()) {
            Some(cle) => {
                let mut m = r.as_object().cloned().unwrap_or_default();
                m.insert("salarie".into(), Value::String((*cle).to_string()));
                ok.push(Value::Object(m));
            }
            None => orphelins.push(r.clone()),
        }
    }
    json!({"rattaches": ok, "orphelins": orphelins})
}

// -------------------------------------------------- fiche de parametrage OC

/// Les versions du format des fiches de parametrage OC que le lecteur connait : l'attribut `Version` de `FICHE` (schema
/// XSD, enumeration `139` en R1.3.9, `140` en R1.4.0 ; `prive/reference/sources/fpoc-format/fpoc-r1.3.9.xsd`,
/// `fpoc-r1.4.0.xsd`). La 1.4.0 ajoute un plancher et un plafond facultatifs aux bases et montants specifiques
/// (`BasePlancher`, `BasePlafond`) et admet une condition tarifaire seule (specification R1.4.0, onglet Versions ;
/// `fpoc-specification-r1.4.0-20260721.txt` l. 49-75). Une autre valeur se lit comme avant, sans format nomme.
const FORMATS_FICHE: [(&str, &str); 2] = [("139", "1.3.9"), ("140", "1.4.0")];

const COMPTEURS_PAR_TYPE: &[&str] = &["S21.G00.70.008", "S21.G00.70.010", "S21.G00.70.011"];
const COMPTEUR_PAR_NOMBRE: &[&str] = &["S21.G00.70.009"];

/// Les conditions tarifaires d'une base ou d'un montant specifique (attribut `Condition`, le meme en 1.3.9 et en 1.4.0),
/// leur libelle et les compteurs du bloc 70 que la DSN doit alors valoriser. Specification R1.4.0, onglet Donnees
/// (`fpoc-specification-r1.4.0-20260721.txt` l. 504-531 ; R1.3.9 l. 395-423, la meme regle) : « Si la condition
/// tarifaire mentionne des ayants droit de type Conjoint, Autre, ou Enfant, les compteurs des rubriques 70.008, 70,010
/// et 70.011 sont a valoriser en DSN. Si la condition tarifaire mentionne un nombre d'ayants droit, sans plus de
/// precisions, le compteur de la rubrique 70.009 doit etre valorise. » Les conditions qui ne nomment aucun ayant droit
/// compte (01, l'assure et TOUS ses ayants droit ; 02, l'assure seul ; 90, l'assure ouvrant droit) n'en appellent
/// aucun : les recapitulatifs de la presentation V139-V140 (diapositives 16-17,
/// `fpoc-presentation-versions-139-140-20260720.txt` l. 49-52) ne leur donnent pas de compteur a elles seules. 92
/// (« chaque ayant droit », sans type) se range avec les nombres : INFERENCE, la diapositive 16 lui donne un seul
/// compteur, « = 0 a n », sans que la transcription dise sa colonne.
const CONDITIONS_TARIFAIRES: [(&str, &str, &[&str]); 15] = [
    ("01", "tarif unique pour l'assure et tous ses ayants droit", &[]),
    ("02", "tarif unique pour l'assure seul", &[]),
    ("03", "tarif unique pour l'assure et 1 ayant droit Conjoint", COMPTEURS_PAR_TYPE),
    ("04", "tarif unique pour l'assure et 1 ayant droit Enfant", COMPTEURS_PAR_TYPE),
    ("05", "tarif unique pour l'assure et 1 ayant droit", COMPTEUR_PAR_NOMBRE),
    ("06", "tarif unique pour l'assure et 2 ayants droit", COMPTEUR_PAR_NOMBRE),
    ("07", "tarif unique pour l'assure et 2 ayants droit ou plus", COMPTEUR_PAR_NOMBRE),
    ("09", "tarif unique pour l'assure et 3 ayants droit ou plus", COMPTEUR_PAR_NOMBRE),
    ("11", "tarif unique pour l'assure et 2 ayants droit Enfant", COMPTEURS_PAR_TYPE),
    ("12", "tarif unique pour l'assure et 3 ayants droit Enfant ou plus", COMPTEURS_PAR_TYPE),
    ("90", "tarif pour l'assure ouvrant droit", &[]),
    ("91", "tarif pour l'ayant droit Conjoint", COMPTEURS_PAR_TYPE),
    ("92", "tarif pour chaque ayant droit", COMPTEUR_PAR_NOMBRE),
    ("94", "tarif pour chaque ayant droit Enfant", COMPTEURS_PAR_TYPE),
    ("95", "tarif pour chaque ayant droit Autre", COMPTEURS_PAR_TYPE),
];

fn fiche_parametrage(e: &Entree) -> Resultat<Value> {
    let texte = e.latin1();
    let a = lire_texte(&texte)?;
    let racine = a.racine();
    let fils = |n: &Noeud, tag: &str| -> Option<usize> { n.enfants.iter().copied().find(|&i| a.n[i].tag == tag) };
    let prod = fils(racine, "ProducteurFiche").map(|i| &a.n[i]);
    let attr = |n: Option<&Noeud>, k: &str| -> Value { n.and_then(|n| n.get(k)).map(|s| Value::String(s.to_string())).unwrap_or(Value::Null) };
    // Le texte d'un enfant nomme, ou null.
    let texte_de = |n: &Noeud, tag: &str| -> Value {
        fils(n, tag).map(|i| nettoye(&a.n[i].texte)).filter(|t| !t.is_empty()).map(|t| Value::String(t.to_string())).unwrap_or(Value::Null)
    };
    // Un plancher ou un plafond de base (1.4.0) : un montant, ou un taux, un coefficient et une assiette.
    let borne = |n: &Noeud, tag: &str, suffixe: &str| -> Value {
        match fils(n, tag).map(|i| &a.n[i]) {
            None => Value::Null,
            Some(b) => json!({"montant": texte_de(b, &format!("Montant{}", suffixe)), "taux": texte_de(b, &format!("Taux{}", suffixe)),
                              "coef": texte_de(b, &format!("Coef{}", suffixe)), "assiette": texte_de(b, &format!("Assiette{}", suffixe))}),
        }
    };
    let version = attr(Some(racine), "Version");
    let format = FORMATS_FICHE.iter().find(|(v, _)| version.as_str() == Some(*v)).map(|(_, f)| Value::String(f.to_string())).unwrap_or(Value::Null);
    let mut groupes = Vec::new();
    for g in a.sous(racine).filter(|n| n.tag == "GROUPE") {
        let mut contrats = Vec::new();
        for pc in a.enfants(g).filter(|n| n.tag == "ParametresContrats") {
            let c = fils(pc, "Contrat").map(|i| &a.n[i]);
            let mut taux = Vec::new();
            let mut bases = Vec::new();
            let mut compteurs: Vec<&str> = Vec::new();
            if let Some(ec) = fils(pc, "ElementsDeCalculAttendus").map(|i| &a.n[i]) {
                for el in a.enfants(ec) {
                    for enfant in a.enfants(el) {
                        if local(&enfant.tag) == "Taux" && !enfant.texte.is_empty() {
                            taux.push(json!({"assiette": el.tag, "nature": attr(Some(el), "LibelleCodeNature"),
                                             "taux": nettoye(&enfant.texte)}));
                        }
                    }
                    if el.tag != "BaseMontantSpecifique" {
                        continue;
                    }
                    let condition = attr(Some(el), "Condition");
                    let libelle_condition = match condition.as_str() {
                        None => Value::Null,
                        Some(code) => {
                            let (_, libelle, appelle) = CONDITIONS_TARIFAIRES.iter().find(|(k, _, _)| *k == code).ok_or_else(|| {
                                format!(
                                    "Fiche de parametrage {} : la condition tarifaire « {} » d'une base ou d'un montant specifique (contrat {}) \
n'est pas une valeur des formats 1.3.9 et 1.4.0 (01 a 07, 09, 11, 12, 90, 91, 92, 94, 95) : les compteurs du bloc 70 \
qu'elle appelle ne se devinent pas. Rien n'est lu.",
                                    attr(Some(racine), "IdentifiantFiche").as_str().unwrap_or("?"),
                                    code,
                                    attr(c, "ReferenceContrat").as_str().unwrap_or("?")
                                )
                            })?;
                            for k in appelle.iter() {
                                if !compteurs.contains(k) {
                                    compteurs.push(k);
                                }
                            }
                            Value::String(libelle.to_string())
                        }
                    };
                    bases.push(json!({"code_nature": attr(Some(el), "ValeurCodeNature"), "libelle": attr(Some(el), "LibelleCodeNature"),
                                      "condition": condition, "libelle_condition": libelle_condition,
                                      "taux": texte_de(el, "Taux"), "montant": texte_de(el, "Montant"),
                                      "coef": texte_de(el, "Coef"), "assiette": texte_de(el, "Assiette"),
                                      "plancher": borne(el, "BasePlancher", "Plancher"), "plafond": borne(el, "BasePlafond", "Plafond")}));
                }
            }
            compteurs.sort();
            contrats.push(json!({"debut": attr(Some(pc), "DateDebutValidite"), "fin": attr(Some(pc), "DateFinValidite"),
                                 "libelle": attr(c, "LibelleContrat"), "reference": attr(c, "ReferenceContrat"),
                                 "option": attr(fils(pc, "Option").map(|i| &a.n[i]), "CodeOption"),
                                 "population": attr(fils(pc, "Population").map(|i| &a.n[i]), "CodePopulation"),
                                 "taux": taux, "bases_specifiques": bases, "compteurs": compteurs}));
        }
        groupes.push(json!({"numero": attr(Some(g), "NumeroGrp"), "libelle": attr(Some(g), "LibelleGrp"), "contrats": contrats}));
    }
    Ok(json!({"identifiant": attr(Some(racine), "IdentifiantFiche"), "version": version, "format": format,
              "creee_le": attr(Some(racine), "DateHeureCreation"),
              "producteur": attr(prod, "RaisonSocialeProducteur"), "groupes": groupes}))
}

// ------------------------------------------------------------ l'AER, le .zip

fn u16_le(b: &[u8], i: usize) -> usize {
    u16::from_le_bytes([b[i], b[i + 1]]) as usize
}

fn u32_le(b: &[u8], i: usize) -> u64 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]) as u64
}

fn u64_le(b: &[u8], i: usize) -> u64 {
    let mut o = [0u8; 8];
    o.copy_from_slice(&b[i..i + 8]);
    u64::from_le_bytes(o)
}

fn mauvais(m: impl Into<String>) -> Echec {
    echec("BadZipFile", m)
}

fn cp437(o: &[u8]) -> String {
    o.iter().map(|&b| if b < 0x80 { b as char } else { char::from_u32(CP437[b as usize - 0x80]).unwrap_or('\u{fffd}') }).collect()
}

/// Le nom d'un membre : UTF-8 si le drapeau 11 est leve, cp437 sinon.
fn nom_zip(o: &[u8], drapeaux: usize) -> Resultat<String> {
    if drapeaux & 0x800 != 0 {
        std::str::from_utf8(o)
            .map(str::to_string)
            .map_err(|e| echec("UnicodeDecodeError", format!("'utf-8' codec can't decode byte in position {} : invalid start byte", e.valid_up_to())))
    } else {
        Ok(cp437(o))
    }
}

/// `repr()` d'un `str` Python (les cas que les noms d'archive rencontrent).
fn repr_py(s: &str) -> String {
    let guill = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut r = String::from(guill);
    for c in s.chars() {
        match c {
            '\\' => r.push_str("\\\\"),
            '\n' => r.push_str("\\n"),
            '\r' => r.push_str("\\r"),
            '\t' => r.push_str("\\t"),
            c if c == guill => {
                r.push('\\');
                r.push(c)
            }
            c if (c as u32) < 0x20 || c as u32 == 0x7f => r.push_str(&format!("\\x{:02x}", c as u32)),
            c => r.push(c),
        }
    }
    r.push(guill);
    r
}

struct Membre {
    nom: String,
    original: String,
    drapeaux: usize,
    methode: usize,
    crc: u64,
    taille_comp: u64,
    taille: u64,
    decalage: i128,
    /// `_end_offset` : l'octet ou le membre ne peut pas aller plus loin.
    fin: i128,
}

fn description_compression(t: usize) -> &'static str {
    match t {
        0 => "store",
        1 => "shrink",
        2..=5 => "reduce",
        6 | 10 => "implode",
        7 => "tokenize",
        8 => "deflate",
        9 => "deflate64",
        12 => "bzip2",
        14 => "lzma",
        18 => "terse",
        19 => "lz77",
        97 => "wavpack",
        98 => "ppmd",
        _ => "unknown",
    }
}

/// `ZipInfo._decodeExtra` : la zone zip64 des tailles et du decalage.
fn extra_zip64(extra: &[u8], m: &mut Membre) -> Resultat<()> {
    let mut e = extra;
    while e.len() >= 4 {
        let (tp, ln) = (u16_le(e, 0), u16_le(e, 2));
        if ln + 4 > e.len() {
            return Err(mauvais(format!("Corrupt extra field {:04x} (size={})", tp, ln)));
        }
        if tp == 0x0001 {
            let mut d = &e[4..ln + 4];
            let mut lire = |champ: &str| -> Resultat<u64> {
                if d.len() < 8 {
                    return Err(mauvais(format!("Corrupt zip64 extra field. {} not found.", champ)));
                }
                let v = u64_le(d, 0);
                d = &d[8..];
                Ok(v)
            };
            if m.taille == 0xFFFF_FFFF {
                m.taille = lire("File size")?;
            }
            if m.taille_comp == 0xFFFF_FFFF {
                m.taille_comp = lire("Compress size")?;
            }
            if m.decalage == 0xFFFF_FFFF {
                m.decalage = lire("Header offset")? as i128;
            }
        }
        e = &e[ln + 4..];
    }
    Ok(())
}

/// `zipfile.ZipFile(...)` : le repertoire central, dans l'ordre de l'archive.
fn repertoire_zip(b: &[u8]) -> Resultat<Vec<Membre>> {
    let pas_un_zip = || mauvais("File is not a zip file");
    let taille_fichier = b.len();
    // `_EndRecData`
    let pos = if taille_fichier >= 22 && b[taille_fichier - 22..].starts_with(b"PK\x05\x06") && b[taille_fichier - 2..] == [0, 0] {
        taille_fichier - 22
    } else {
        let debut = taille_fichier.saturating_sub((1 << 16) + 22);
        match b[debut..].windows(4).rposition(|w| w == b"PK\x05\x06").map(|p| debut + p) {
            Some(p) if p + 22 <= taille_fichier => p,
            _ => return Err(pas_un_zip()),
        }
    };
    let rec = &b[pos..pos + 22];
    let (mut taille_cd, mut decalage_cd) = (u32_le(rec, 12) as i128, u32_le(rec, 16) as i128);
    let mut location = pos as i128;
    // `_EndRecData64`
    if pos >= 20 && &b[pos - 20..pos - 16] == b"PK\x06\x07" {
        let loc = pos - 20;
        let (disque, reloff, disques) = (u32_le(b, loc + 4), u64_le(b, loc + 8) as i128, u32_le(b, loc + 16));
        if disque != 0 || disques > 1 {
            return Err(mauvais("zipfiles that span multiple disks are not supported"));
        }
        let offset = loc as i128 - 56;
        if reloff > offset {
            return Err(mauvais("Corrupt zip64 end of central directory locator"));
        }
        let lire56 = |de: i128| -> Option<&[u8]> {
            let de = de as usize;
            (de.checked_add(56)? <= b.len()).then(|| &b[de..de + 56])
        };
        let mut extrasz = offset - reloff;
        let mut data = lire56(reloff).ok_or_else(pas_un_zip)?;
        if !data.starts_with(b"PK\x06\x06") && reloff != offset {
            extrasz = 0;
            data = lire56(offset).ok_or_else(pas_un_zip)?;
        }
        if !data.starts_with(b"PK\x06\x06") {
            return Err(mauvais("Zip64 end of central directory record not found"));
        }
        let (sz, dirsize, diroffset) = (u64_le(data, 4) as i128, u64_le(data, 40) as i128, u64_le(data, 48) as i128);
        if diroffset + dirsize != reloff || sz + 12 != 56 + extrasz {
            return Err(mauvais("Corrupt zip64 end of central directory record"));
        }
        taille_cd = dirsize;
        decalage_cd = diroffset;
        location = offset - extrasz;
    }
    let concat = location - taille_cd - decalage_cd;
    let debut_cd = decalage_cd + concat;
    if debut_cd < 0 {
        return Err(mauvais("Bad offset for central directory"));
    }
    let debut_cd = (debut_cd as usize).min(b.len());
    let cd = &b[debut_cd..(debut_cd as i128 + taille_cd).min(b.len() as i128) as usize];
    let mut membres = Vec::new();
    let (mut total, mut p) = (0i128, 0usize);
    while total < taille_cd {
        if cd.len() < p + 46 {
            return Err(mauvais("Truncated central directory"));
        }
        let c = &cd[p..p + 46];
        if &c[0..4] != b"PK\x01\x02" {
            return Err(mauvais("Bad magic number for central directory"));
        }
        p += 46;
        let (ln_nom, ln_extra, ln_comm) = (u16_le(c, 28), u16_le(c, 30), u16_le(c, 32));
        let prendre = |p: &mut usize, n: usize| -> &[u8] {
            let a = (*p).min(cd.len());
            let z = (*p + n).min(cd.len());
            *p += n;
            &cd[a..z]
        };
        let nom_brut = prendre(&mut p, ln_nom);
        let extra = prendre(&mut p, ln_extra);
        let _ = prendre(&mut p, ln_comm);
        let drapeaux = u16_le(c, 8);
        let original = nom_zip(nom_brut, drapeaux)?;
        let nom = original.split('\0').next().unwrap_or("").to_string();
        let mut m = Membre {
            nom,
            original,
            drapeaux,
            methode: u16_le(c, 10),
            crc: u32_le(c, 16),
            taille_comp: u32_le(c, 20),
            taille: u32_le(c, 24),
            decalage: u32_le(c, 42) as i128,
            fin: 0,
        };
        if c[6] > 63 {
            return Err(echec("NotImplementedError", format!("zip file version {:.1}", c[6] as f64 / 10.0)));
        }
        extra_zip64(extra, &mut m)?;
        m.decalage += concat;
        membres.push(m);
        total += (46 + ln_nom + ln_extra + ln_comm) as i128;
    }
    // `_end_offset` : chaque membre finit ou le suivant commence (le repertoire pour le dernier).
    let mut ordre: Vec<usize> = (0..membres.len()).collect();
    ordre.sort_by_key(|&i| std::cmp::Reverse(membres[i].decalage));
    let mut fin = debut_cd as i128;
    for i in ordre {
        membres[i].fin = fin;
        fin = membres[i].decalage;
    }
    Ok(membres)
}

fn crc32(o: &[u8]) -> u64 {
    let mut c = flate2::Crc::new();
    c.update(o);
    c.sum() as u64
}

/// `ZipFile.read(nom)`.
fn lire_membre(b: &[u8], m: &Membre) -> Resultat<Vec<u8>> {
    if m.decalage < 0 {
        return Err(echec("ValueError", format!("negative seek value {}", m.decalage)));
    }
    let dec = m.decalage as usize;
    if dec.saturating_add(30) > b.len() {
        return Err(mauvais("Truncated file header"));
    }
    let h = &b[dec..dec + 30];
    if &h[0..4] != b"PK\x03\x04" {
        return Err(mauvais("Bad magic number for file header"));
    }
    let (ln_nom, ln_extra) = (u16_le(h, 26), u16_le(h, 28));
    let debut_nom = dec + 30;
    let nom_brut = &b[debut_nom.min(b.len())..(debut_nom + ln_nom).min(b.len())];
    let debut_donnees = debut_nom + ln_nom + ln_extra;
    if m.drapeaux & 0x20 != 0 {
        return Err(echec("NotImplementedError", "compressed patched data (flag bit 5)"));
    }
    if m.drapeaux & 0x40 != 0 {
        return Err(echec("NotImplementedError", "strong encryption (flag bit 6)"));
    }
    let nom_en_tete = nom_zip(nom_brut, u16_le(h, 6))?;
    if nom_en_tete != m.original {
        let mut brut = String::from("b'");
        for &o in nom_brut {
            match o {
                b'\\' => brut.push_str("\\\\"),
                b'\'' => brut.push_str("\\'"),
                0x20..=0x7e => brut.push(o as char),
                b'\n' => brut.push_str("\\n"),
                b'\r' => brut.push_str("\\r"),
                b'\t' => brut.push_str("\\t"),
                _ => brut.push_str(&format!("\\x{:02x}", o)),
            }
        }
        brut.push('\'');
        return Err(mauvais(format!("File name in directory {} and header {} differ.", repr_py(&m.original), brut)));
    }
    if (debut_donnees as i128).saturating_add(m.taille_comp as i128) > m.fin {
        return Err(mauvais(format!("Overlapped entries: {} (possible zip bomb)", repr_py(&m.original))));
    }
    if m.drapeaux & 1 != 0 {
        return Err(echec("RuntimeError", format!("File {} is encrypted, password required for extraction", repr_py(&m.nom))));
    }
    let comprime = &b[debut_donnees.min(b.len())..(debut_donnees.saturating_add(m.taille_comp as usize)).min(b.len())];
    let donnees = match m.methode {
        0 => {
            let n = (m.taille as usize).min(m.taille_comp as usize);
            if comprime.len() < n {
                return Err(echec("EOFError", ""));
            }
            comprime[..n].to_vec()
        }
        8 => {
            // Du deflate brut. Un flux tronque rend ce qu'il a (le CRC dira le reste, comme
            // zlib) ; un flux faux est une `zlib.error`.
            use flate2::{Decompress, FlushDecompress, Status};
            let mut d = Decompress::new(false);
            let mut out: Vec<u8> = Vec::with_capacity((comprime.len() * 3).max(4096));
            loop {
                let lu = d.total_in() as usize;
                if out.capacity() == out.len() {
                    out.reserve(out.len().max(4096));
                }
                match d.decompress_vec(&comprime[lu..], &mut out, FlushDecompress::None) {
                    Ok(Status::StreamEnd) => break,
                    Ok(Status::Ok) => {}
                    Ok(Status::BufError) => {
                        if d.total_in() as usize >= comprime.len() {
                            break;
                        }
                        out.reserve(out.len().max(4096));
                    }
                    Err(e) => return Err(echec("error", format!("Error -3 while decompressing data: {}", e))),
                }
            }
            out
        }
        t => {
            return Err(echec("NotImplementedError", format!("compression type {} ({})", t, description_compression(t))));
        }
    };
    if crc32(&donnees) != m.crc {
        return Err(mauvais(format!("Bad CRC-32 for file {}", repr_py(&m.nom))));
    }
    Ok(donnees)
}

/// `Path(nom).name` d'un chemin POSIX.
fn nom_de_fichier(nom: &str) -> &str {
    nom.split('/').rfind(|p| !p.is_empty() && *p != ".").unwrap_or("")
}

/// `Path(nom).stem`
fn radical(nom: &str) -> &str {
    let n = nom_de_fichier(nom);
    match n.rfind('.') {
        Some(i) if i > 0 && i < n.len() - 1 => &n[..i],
        _ => n,
    }
}

/// Le motif `_NOM_AER` : `AER_<siret>__<MMAAAA>_<code>_<JJMMAAAA>_<NOM>_<rang>_<id>.pdf`,
/// majuscules ou minuscules. Rend `(MMAAAA, NOM)`.
fn nom_aer(nom: &str) -> Option<(String, String)> {
    let s = nom.strip_suffix('\n').unwrap_or(nom);
    let c: Vec<char> = s.chars().collect();
    if c.len() < 4 || !"AER_".chars().zip(&c).all(|(a, b)| a.eq_ignore_ascii_case(b)) {
        return None;
    }
    let mut i = 4;
    let debut = i;
    while i < c.len() && est_chiffre(c[i]) {
        i += 1;
    }
    if i == debut {
        return None;
    }
    let debut = i;
    while i < c.len() && c[i] == '_' {
        i += 1;
    }
    if i == debut {
        return None;
    }
    let chiffres = |i: usize, n: usize| i + n <= c.len() && c[i..i + n].iter().all(|&x| est_chiffre(x));
    if !chiffres(i, 6) {
        return None;
    }
    let periode: String = c[i..i + 6].iter().collect();
    i += 6;
    if c.get(i) != Some(&'_') {
        return None;
    }
    i += 1;
    while i < c.len() && c[i] != '_' {
        i += 1;
    }
    if c.get(i) != Some(&'_') {
        return None;
    }
    i += 1;
    if !chiffres(i, 8) || c.get(i + 8) != Some(&'_') {
        return None;
    }
    i += 9;
    // `(?P<qui>.+?)` : le plus court dont la suite est `_<rang>_<id>.pdf`.
    let queue = |r: &[char]| -> bool {
        if r.first() != Some(&'_') {
            return false;
        }
        let mut j = 1;
        let d = j;
        while j < r.len() && est_chiffre(r[j]) {
            j += 1;
        }
        if j == d || r.get(j) != Some(&'_') {
            return false;
        }
        j += 1;
        let d = j;
        while j < r.len() && est_chiffre(r[j]) {
            j += 1;
        }
        j > d && r.len() == j + 4 && ".pdf".chars().zip(&r[j..]).all(|(a, b)| a.eq_ignore_ascii_case(b))
    };
    for p in i + 1..=c.len() {
        if c[p - 1] == '\n' {
            return None;
        }
        if queue(&c[p..]) {
            return Some((periode, c[i..p].iter().collect()));
        }
    }
    None
}

/// `aer_spectacle` : les attestations d'une archive de nature 46 ; liste vide si
/// le contenu n'est pas une archive lisible.
pub(crate) fn aer_spectacle(o: &[u8]) -> Resultat<Value> {
    let membres = match repertoire_zip(o) {
        Ok(m) => m,
        Err(e) if e.classe == Some("BadZipFile") => return Ok(json!([])),
        Err(e) => return Err(e),
    };
    // `NameToInfo` garde le dernier membre de chaque nom.
    let mut out = Vec::new();
    for m in &membres {
        if m.nom.ends_with('/') {
            continue;
        }
        let (periode, qui) = match nom_aer(nom_de_fichier(&m.nom)) {
            Some((mmaaaa, qui)) => {
                let c: Vec<char> = mmaaaa.chars().collect();
                (format!("{}-{}", c[2..].iter().collect::<String>(), c[..2].iter().collect::<String>()), qui.replace('_', " "))
            }
            None => (String::new(), radical(&m.nom).to_string()),
        };
        let dernier = membres.iter().rev().find(|x| x.nom == m.nom).unwrap_or(m);
        let donnees = lire_membre(o, dernier)?;
        out.push(json!({"nom_archive": m.nom, "periode": periode, "qui": qui, "donnees": base64_ecrire(&donnees)}));
    }
    Ok(Value::Array(out))
}

/// `rattacher` : pour chaque nom d'attestation, la cle du salarie ou `null`. `index`
/// est la liste `[cle, "NOM PRENOM"]` des fiches, dans l'ordre du fichier.
pub(crate) fn rattacher(quis: &[Value], index: &[Value]) -> Value {
    let mut idx: Vec<(String, &str)> = index
        .iter()
        .filter_map(|p| Some((crate::contexte::cle_recherche(p.get(1)?.as_str()?), p.get(0)?.as_str()?)))
        .collect();
    idx.sort_by_key(|(n, _)| std::cmp::Reverse(n.chars().count()));
    let cles: Vec<Value> = quis
        .iter()
        .map(|q| {
            let cible = crate::contexte::cle_recherche(q.as_str().unwrap_or(""));
            idx.iter()
                .find(|(nom, _)| !nom.is_empty() && (cible.contains(nom.as_str()) || nom.contains(cible.as_str())))
                .map(|(_, cle)| Value::String((*cle).to_string()))
                .unwrap_or(Value::Null)
        })
        .collect();
    Value::Array(cles)
}

// ---------------------------------------------------------------- la commande

fn liste<'a>(d: &'a Value, cle: &str) -> Resultat<&'a Vec<Value>> {
    d.get(cle).and_then(Value::as_array).ok_or_else(|| format!("demande illisible : il faut `{}`", cle).into())
}

fn operation(d: &Value) -> Resultat<Value> {
    let op = d.get("operation").and_then(Value::as_str).ok_or("demande illisible : il faut `operation`")?;
    let mut m = Map::new();
    match op {
        "natures" => {
            m.insert("natures".into(), Value::Object(NATURES.iter().map(|(c, l)| ((*c).to_string(), json!(l))).collect()));
        }
        "constantes" => {
            // Ce que `retours.py` garde en constantes : un test Python verifie qu'elles sont celles-ci.
            for (k, v) in [
                ("NATURE_TAUX_PAS", NATURE_TAUX_PAS),
                ("NATURE_TAUX_ATMP", NATURE_TAUX_ATMP),
                ("NATURE_CRM_SPECTACLE", NATURE_CRM_SPECTACLE),
                ("NATURE_AER_SPECTACLE", NATURE_AER_SPECTACLE),
                ("ESSAI_TEST", ESSAI_TEST),
                ("ESSAI_REEL", ESSAI_REEL),
            ] {
                m.insert(k.into(), json!(v));
            }
            m.insert("NATURES_BILAN_DEPOT".into(), json!(NATURES_BILAN_DEPOT));
            m.insert("A_ARCHIVER_ABSOLUMENT".into(), json!(A_ARCHIVER_ABSOLUMENT));
            m.insert("RAPPORTS_DEPOT".into(), json!(RAPPORTS_DEPOT));
            m.insert("CHAMPS_BILAN".into(), json!(CHAMPS_BILAN));
        }
        "libelle" => {
            let code = d.get("code").and_then(Value::as_str).ok_or("demande illisible : il faut `code`")?;
            m.insert("libelle".into(), Value::String(libelle_nature(code)));
        }
        "bilan" => {
            m.insert("bilan".into(), parse_bilan_depot(&entree_de(d)?));
        }
        "anomalies" => {
            m.insert("anomalies".into(), Value::Array(anomalies_du_bilan(&entree_de(d)?)?));
        }
        "anomalies_codees" => {
            m.insert("anomalies".into(), Value::Array(anomalies_codees(&entree_de(d)?)?));
        }
        "fusionner" => {
            m.insert("bilan".into(), fusionner_bilans(liste(d, "bilans")?));
        }
        "liste_retours" => {
            m.insert("retours".into(), Value::Array(parse_liste_retours(&entree_de(d)?)?));
        }
        "liste_depots" => {
            m.insert("depots".into(), Value::Array(parse_liste_depots(&entree_de(d)?)?));
        }
        "taux_pas" => {
            m.extend(match extraire_taux_pas(&entree_de(d)?)? {
                Value::Object(o) => o,
                _ => Map::new(),
            });
        }
        "releves" => {
            if let Value::Object(o) = releves_vers_salaries(liste(d, "releves")?, liste(d, "nirs")?) {
                m.extend(o);
            }
        }
        "fiche" => {
            m.insert("fiche".into(), fiche_parametrage(&entree_de(d)?)?);
        }
        "aer" => {
            m.insert("attestations".into(), aer_spectacle(&entree_de(d)?.octets())?);
        }
        "rattacher" => {
            m.insert("cles".into(), rattacher(liste(d, "quis")?, liste(d, "index")?));
        }
        autre => return Err(format!("demande illisible : operation `{}` inconnue", autre).into()),
    }
    Ok(Value::Object(m))
}

/// `paie retours` : la demande JSON (`operation` et ses donnees) sur l'entree
/// standard ; un XML ou une archive illisibles portent leur `classe`.
pub fn commande(entree: &str) -> Result<Value, String> {
    let d: Value = serde_json::from_str(entree).map_err(|e| format!("demande illisible : {}", e))?;
    let mut o = Map::new();
    match operation(&d) {
        Ok(Value::Object(m)) => {
            o.insert("ok".into(), Value::Bool(true));
            o.extend(m);
        }
        Ok(_) => {}
        Err(Echec { classe: None, message }) => return Err(message),
        Err(Echec { classe: Some(c), message }) => {
            o.insert("ok".into(), Value::Bool(false));
            o.insert("erreur".into(), Value::String(message));
            o.insert("classe".into(), Value::String(c.into()));
        }
    }
    Ok(Value::Object(o))
}

// ------------------------------------------------- ce que le journal en lit

/// `str.isspace()` de Python.
pub fn blanc_py(c: char) -> bool {
    est_blanc(c)
}

/// `parse_bilan_depot(octets)` : le bilan (`null` si ce n'est pas un rapport).
pub fn bilan_d_octets(o: &[u8]) -> Value {
    parse_bilan_depot(&Entree::Octets(o.to_vec()))
}

/// Le bilan d'octets ; `Err(classe)` si le XML ne se lit pas (`journal-reprise` nomme la piece au lieu de la sauter).
pub fn bilan_ou_illisible(o: &[u8]) -> Result<Value, &'static str> {
    parse_bilan_depot_lu(&Entree::Octets(o.to_vec())).map_err(|e| e.classe.unwrap_or("Exception"))
}

/// `anomalies_du_bilan(octets)`.
pub fn anomalies_lisibles_d_octets(o: &[u8]) -> Result<Vec<Value>, Echec> {
    anomalies_du_bilan(&Entree::Octets(o.to_vec()))
}

/// `anomalies_codees(octets)`.
pub fn anomalies_codees_d_octets(o: &[u8]) -> Result<Vec<Value>, Echec> {
    anomalies_codees(&Entree::Octets(o.to_vec()))
}
