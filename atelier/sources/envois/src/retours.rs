//! La lecture des retours DSN : ce que les organismes
//! repondent apres un depot.
//!
//! Des octets ou du XML en entree, des valeurs JSON en sortie : les bilans de
//! depot (natures 10 et 11), leurs anomalies, la liste des retours et des
//! depots, le CRM 94 (taux du PAS) par ses quatre strategies, les AER de
//! nature 46 (une archive .zip, lue ici a la main), les fiches de
//! parametrage des organismes complementaires.
//!
//! Le XML se lit en arbre d'elements dont le texte est
//! celui d'avant le premier enfant (commentaires et instructions ignores), les
//! balises et les attributs a espace de noms ecrits `{uri}nom`. Un XML qui ne se
//! lit pas est une `ParseError` (la phrase est celle de `roxmltree`).

use serde_json::{json, Map, Value};

use crate::tables_retours::{CODECS_ASCII, CODECS_AUTRES, CODECS_CP1252, CODECS_LATIN1, CODECS_REFUSES, CP437, DECIMAUX, NOM_DEBUT, NOM_SUITE, WINDOWS_1252};

type Resultat<T> = Result<T, Echec>;

use socle::echec::{erreur as echec, Echec};
use socle::texte::blanc;

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

// ------------------------------------------------------ le texte

/// `str.strip()`.
fn nettoye(s: &str) -> &str {
    s.trim_matches(blanc)
}

/// Un chiffre decimal Unicode (le `\d` d'une expression reguliere sur du texte).
pub fn est_chiffre(c: char) -> bool {
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

/// Les valeurs JSON tenues pour fausses : `null`, `false`, `0`, `""`, `[]`, `{}`.
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
                return Err(mal_forme("XML mal forme : la declaration <?xml ?>"));
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
            return Err(mal_forme("XML mal forme : un numero de caractere invalide"));
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
                return Err(mal_forme("XML mal forme : un caractere inattendu"));
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
                    return Err(mal_forme("XML mal forme : un caractere inattendu"));
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
                return Err(mal_forme("XML mal forme : une balise fermee sans etre ouverte"));
            }
            i = chercher(&c, i + 2, ">").map(|k| k + 1).unwrap_or(c.len());
        } else {
            let fin = nom_a(i + 1, &c, &|x| blanc_xml(Some(&x)) || x == '/' || x == '>');
            if fin > i + 1 && !nom_valide(&c[i + 1..fin]) {
                return Err(mal_forme("XML mal forme : un caractere inattendu"));
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
                            return Err(mal_forme("XML mal forme : un caractere inattendu"));
                        }
                        let nom_attr: String = c[i..fin].iter().collect();
                        if let Some((prefixe, local)) = nom_attr.split_once(':') {
                            if (prefixe == "xmlns" && local == "xmlns") || (local == "xmlns" && prefixe != "xmlns" && !declares.contains(prefixe) && prefixe != "xml") {
                                return Err(mal_forme("XML mal forme : un prefixe sans espace de noms"));
                            }
                            if prefixe == "xmlns" && vide_apres(&c, fin) {
                                return Err(mal_forme("XML mal forme : un prefixe retire"));
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
        return Err(mal_forme("XML mal forme : un caractere inattendu"));
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
    /// Un nom d'encodage connu mais qui n'est pas sur 256 caracteres (multi-octets) : refuse
    /// (`ValueError`).
    Refuse,
    /// Une autre table sur 256 caracteres, que ce moteur ne porte pas : `ParseError`.
    Autre,
    /// Un nom d'encodage inconnu : `LookupError`.
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
        Codage::Introuvable => echec("LookupError", format!("encodage inconnu : {}", nom)),
        Codage::Refuse => echec("ValueError", "multi-byte encodings are not supported"),
        _ => echec("ParseError", "encodage inconnu"),
    }
}

/// `ET.fromstring(octets)` : l'encodage declare est celui qui decode.
fn lire_octets(o: &[u8]) -> Resultat<Arbre> {
    let invalide = || echec("ParseError", "XML mal forme : un caractere inattendu");
    let incorrect = || echec("ParseError", "l'encodage declare par le XML n'est pas le sien");
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
fn parse_bilan_depot(o: &[u8]) -> Value {
    parse_bilan_depot_lu(o).unwrap_or(Value::Null)
}

/// `parse_bilan_depot`, mais un XML qui ne se lit pas est un refus (sa classe), pas « pas un bilan » : la reprise du
/// journal le nomme (`illisibles`). Un XML bien forme qui n'est pas un rapport de depot rend `Null`.
fn parse_bilan_depot_lu(o: &[u8]) -> Resultat<Value> {
    let texte = String::from_utf8_lossy(o);
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
/// specification VT2.2, `atelier/reference/sources/gip-mds-api-dsn/`). Un XML illisible, ou qui
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
fn octets_arbre(o: &[u8]) -> Resultat<Option<Arbre>> {
    match lire_octets(o) {
        Ok(a) => Ok(Some(a)),
        Err(Echec { classe: Some("ParseError"), .. }) => Ok(None),
        Err(autre) => Err(autre),
    }
}

/// `anomalies_du_bilan` : les motifs d'un BAN, en phrases lisibles.
fn anomalies_du_bilan(o: &[u8]) -> Resultat<Vec<Value>> {
    let Some(arbre) = octets_arbre(o)? else {
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
fn anomalies_codees(o: &[u8]) -> Resultat<Vec<Value>> {
    let Some(arbre) = octets_arbre(o)? else {
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

/// La valeur de `cle` dans un objet JSON ; une autre forme est refusee.
fn lire_cle<'a>(v: &'a Value, cle: &str) -> Resultat<Option<&'a Value>> {
    match v {
        Value::Object(m) => Ok(m.get(cle)),
        autre => Err(echec("TypeError", socle::fichiers::attendu("un objet", socle::valeur::nom_type(autre)))),
    }
}

/// Un element seul (un objet) ou une liste d'elements : la reponse JSON de la plate-forme ecrit un objet quand il n'y
/// en a qu'un. Rien (`null`) n'en fait aucun.
fn un_ou_plusieurs(v: Option<&Value>) -> Resultat<Vec<Value>> {
    match v {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(o @ Value::Object(_)) => Ok(vec![o.clone()]),
        Some(Value::Array(a)) => Ok(a.clone()),
        Some(autre) => Err(echec("TypeError", socle::fichiers::attendu("une liste", socle::valeur::nom_type(autre)))),
    }
}

fn liste_retours_json(d: &Value) -> Resultat<Vec<Value>> {
    let racine = lire_cle(d, "retours")?.unwrap_or(d);
    let mut out = Vec::new();
    for f in &un_ou_plusieurs(lire_cle(racine, "flux")?)? {
        let idf = lire_cle(f, "id")?.cloned().unwrap_or(Value::Null);
        for r in &un_ou_plusieurs(lire_cle(f, "retour")?)? {
            let Value::Object(m) = r else {
                return Err(echec("TypeError", socle::fichiers::attendu("un objet", socle::valeur::nom_type(r))));
            };
            let mut m = m.clone();
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
fn parse_liste_retours(o: &[u8]) -> Resultat<Vec<Value>> {
    let texte = String::from_utf8_lossy(o);
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
pub fn liste_retours_d_octets(o: &[u8]) -> Resultat<Vec<Value>> {
    parse_liste_retours(o)
}

/// `parse_liste_depots(octets)` : les depots que la plate-forme liste.
pub fn liste_depots_d_octets(o: &[u8]) -> Resultat<Vec<Value>> {
    parse_liste_depots(o)
}

/// `parse_liste_depots` : depots > declarant > depot{idflux, date, statut}.
fn parse_liste_depots(o: &[u8]) -> Resultat<Vec<Value>> {
    let texte = String::from_utf8_lossy(o);
    let texte = nettoye(&texte);
    if texte.starts_with('{') {
        let d = json_lire(texte)?;
        let depots = lire_cle(&d, "depots")?.unwrap_or(&d);
        let decl = lire_cle(depots, "declarant")?.cloned().unwrap_or(json!({}));
        return un_ou_plusieurs(lire_cle(&decl, "depot")?);
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
fn extraire_taux_pas(o: &[u8]) -> Resultat<Value> {
    let texte = String::from_utf8_lossy(o);
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
pub fn releves_vers_salaries(releves: &[Value], nirs: &[Value]) -> Value {
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
/// XSD, enumeration `139` en R1.3.9, `140` en R1.4.0 ; `atelier/reference/sources/fpoc-format/fpoc-r1.3.9.xsd`,
/// `fpoc-r1.4.0.xsd`). La 1.4.0 ajoute un plancher et un plafond facultatifs aux bases et montants specifiques
/// (`BasePlancher`, `BasePlafond`) et admet une condition tarifaire seule (specification R1.4.0, onglet Versions ;
/// `fpoc-specification-r1.4.0-20260721.txt` l. 49-75). Une autre valeur se lit sans format nomme.
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

fn fiche_parametrage(o: &[u8]) -> Resultat<Value> {
    let texte: String = o.iter().map(|&b| b as char).collect();
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
            .map_err(|e| echec("UnicodeDecodeError", format!("texte illisible en UTF-8 a l'octet {}", e.valid_up_to())))
    } else {
        Ok(cp437(o))
    }
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
            return Err(mauvais(format!("archive zip abimee : champ supplementaire {:04x} (taille {})", tp, ln)));
        }
        if tp == 0x0001 {
            let mut d = &e[4..ln + 4];
            let mut lire = |champ: &str| -> Resultat<u64> {
                if d.len() < 8 {
                    return Err(mauvais(format!("archive zip64 abimee : {} absent du champ supplementaire", champ)));
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
    let pas_un_zip = || mauvais("ce n'est pas une archive zip");
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
            return Err(mauvais("archive zip sur plusieurs disques : non lue"));
        }
        let offset = loc as i128 - 56;
        if reloff > offset {
            return Err(mauvais("archive zip64 abimee : le reperage de la fin du repertoire"));
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
            return Err(mauvais("archive zip64 sans fin de repertoire"));
        }
        let (sz, dirsize, diroffset) = (u64_le(data, 4) as i128, u64_le(data, 40) as i128, u64_le(data, 48) as i128);
        if diroffset + dirsize != reloff || sz + 12 != 56 + extrasz {
            return Err(mauvais("archive zip64 abimee : la fin du repertoire"));
        }
        taille_cd = dirsize;
        decalage_cd = diroffset;
        location = offset - extrasz;
    }
    let concat = location - taille_cd - decalage_cd;
    let debut_cd = decalage_cd + concat;
    if debut_cd < 0 {
        return Err(mauvais("archive zip abimee : la position du repertoire"));
    }
    let debut_cd = (debut_cd as usize).min(b.len());
    let cd = &b[debut_cd..(debut_cd as i128 + taille_cd).min(b.len() as i128) as usize];
    let mut membres = Vec::new();
    let (mut total, mut p) = (0i128, 0usize);
    while total < taille_cd {
        if cd.len() < p + 46 {
            return Err(mauvais("archive zip tronquee : le repertoire"));
        }
        let c = &cd[p..p + 46];
        if &c[0..4] != b"PK\x01\x02" {
            return Err(mauvais("archive zip abimee : la signature du repertoire"));
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
            return Err(echec("NotImplementedError", format!("archive zip de version {:.1} : non lue", c[6] as f64 / 10.0)));
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
        return Err(mauvais("archive zip tronquee : l'en-tete d'un fichier"));
    }
    let h = &b[dec..dec + 30];
    if &h[0..4] != b"PK\x03\x04" {
        return Err(mauvais("archive zip abimee : la signature de l'en-tete d'un fichier"));
    }
    let (ln_nom, ln_extra) = (u16_le(h, 26), u16_le(h, 28));
    let debut_nom = dec + 30;
    let nom_brut = &b[debut_nom.min(b.len())..(debut_nom + ln_nom).min(b.len())];
    let debut_donnees = debut_nom + ln_nom + ln_extra;
    if m.drapeaux & 0x20 != 0 {
        return Err(echec("NotImplementedError", "archive zip a donnees corrigees (bit 5) : non lue"));
    }
    if m.drapeaux & 0x40 != 0 {
        return Err(echec("NotImplementedError", "archive zip chiffree (bit 6) : non lue"));
    }
    let nom_en_tete = nom_zip(nom_brut, u16_le(h, 6))?;
    if nom_en_tete != m.original {
        return Err(mauvais(format!("archive zip abimee : le repertoire nomme « {} », l'en-tete « {} »", m.original, nom_en_tete)));
    }
    if (debut_donnees as i128).saturating_add(m.taille_comp as i128) > m.fin {
        return Err(mauvais(format!("archive zip abimee : « {} » chevauche un autre fichier", m.original)));
    }
    if m.drapeaux & 1 != 0 {
        return Err(echec("RuntimeError", format!("File « {} » is encrypted, password required for extraction", m.nom)));
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
                    Err(e) => return Err(echec("error", format!("decompression impossible : {}", e))),
                }
            }
            out
        }
        t => {
            return Err(echec("NotImplementedError", format!("compression {} ({}) : non lue", t, description_compression(t))));
        }
    };
    if crc32(&donnees) != m.crc {
        return Err(mauvais(format!("archive zip abimee : le CRC-32 de « {} » ne correspond pas", m.nom)));
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
pub fn aer_spectacle(o: &[u8]) -> Resultat<Value> {
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
        out.push(json!({"nom_archive": m.nom, "periode": periode, "qui": qui, "donnees": socle::base64::ecrire(&donnees)}));
    }
    Ok(Value::Array(out))
}

/// `rattacher` : pour chaque nom d'attestation, la cle du salarie ou `null`. `index`
/// est la liste `[cle, "NOM PRENOM"]` des fiches, dans l'ordre du fichier.
pub fn rattacher(quis: &[Value], index: &[Value]) -> Value {
    let mut idx: Vec<(String, &str)> = index
        .iter()
        .filter_map(|p| Some((socle::texte::cle_recherche(p.get(1)?.as_str()?), p.get(0)?.as_str()?)))
        .collect();
    idx.sort_by_key(|(n, _)| std::cmp::Reverse(n.chars().count()));
    let cles: Vec<Value> = quis
        .iter()
        .map(|q| {
            let cible = socle::texte::cle_recherche(q.as_str().unwrap_or(""));
            idx.iter()
                .find(|(nom, _)| !nom.is_empty() && (cible.contains(nom.as_str()) || nom.contains(cible.as_str())))
                .map(|(_, cle)| Value::String((*cle).to_string()))
                .unwrap_or(Value::Null)
        })
        .collect();
    Value::Array(cles)
}

// ------------------------------------------------- ce que le journal en lit

/// `parse_bilan_depot(octets)` : le bilan (`null` si ce n'est pas un rapport).
pub fn bilan_d_octets(o: &[u8]) -> Value {
    parse_bilan_depot(o)
}

/// Le bilan d'octets ; `Err(classe)` si le XML ne se lit pas (`journal-reprise` nomme la piece au lieu de la sauter).
pub fn bilan_ou_illisible(o: &[u8]) -> Result<Value, &'static str> {
    parse_bilan_depot_lu(o).map_err(|e| e.classe.unwrap_or("Exception"))
}

/// `anomalies_du_bilan(octets)`.
pub fn anomalies_lisibles_d_octets(o: &[u8]) -> Result<Vec<Value>, Echec> {
    anomalies_du_bilan(o)
}

/// `anomalies_codees(octets)`.
pub fn anomalies_codees_d_octets(o: &[u8]) -> Result<Vec<Value>, Echec> {
    anomalies_codees(o)
}

/// Les taux de PAS d'un compte rendu de nature 94 : `{"strategie", "releves"}`.
pub fn taux_pas_d_octets(o: &[u8]) -> Result<Value, Echec> {
    extraire_taux_pas(o)
}

/// Une fiche de parametrage d'un organisme complementaire.
pub fn fiche_d_octets(o: &[u8]) -> Result<Value, Echec> {
    fiche_parametrage(o)
}

// --------------------------------------------- un rapport au format harmonise, lu pour l'Historique

/// Les types de rapport que la specification du schema des bilans nomme (« Modele conceptuel » : AEE, certificat de
/// conformite, avis de rejet, bilan d'anomalies, BIS, controle inter-declarations ;
/// `atelier/reference/sources/gip-mds-api-dsn/gip-mds-specifications-schema-bilans-vt2.2-20201118.txt`). Les codes
/// courts AEE, CCO, BAN sont ceux que la plate-forme ecrit dans `rapport/@type` (pieces recues ; `RAPPORTS_DEPOT`) ;
/// ARE, BIS et CID sont les noms des exemples du guide v6.8 (§6.1, `exemple_retour_10_ARE.xml`, `_20_BIS_`, `_21_CID`) :
/// une inference sur le code, qu'aucune piece recue n'a encore eprouvee.
const TYPES_RAPPORT: [(&str, &str); 6] = [
    ("AEE", "Accusé d'enregistrement électronique"),
    ("ARE", "Avis de rejet"),
    ("CCO", "Certificat de conformité"),
    ("BAN", "Bilan d'anomalies"),
    ("BIS", "Bilan d'identification des salariés"),
    ("CID", "Contrôle inter-déclarations"),
];

fn premier_texte<'a>(a: &'a Arbre, e: &'a Noeud, noms: &[&str]) -> &'a str {
    for nom in noms {
        if let Some(x) = a.sous(e).find(|x| local(&x.tag) == *nom && !x.texte_net().is_empty()) {
            return x.texte_net();
        }
    }
    ""
}

/// Un rapport au FORMAT DSN HARMONISE (la racine `rapport` du schema des bilans) lu pour qu'on le lise : son type, le
/// message d'en-tete (`rapport/@message`), l'etat de l'envoi (`envoi_etat` : OK, KO ou ANO), chaque declaration
/// (nature, type, ordre, mois, etat), chaque anomalie (code, categorie, message, rubrique declenchante, numero de
/// ligne, nombre, et le salarie de son environnement), et les informations collectives (`communication`, §2.6 : le
/// type et les donnees, nom et valeur). Tout vient de la specification VT2.2 deposee
/// (`atelier/reference/sources/gip-mds-api-dsn/gip-mds-specifications-schema-bilans-vt2.2-20201118.txt`) ; les noms
/// d'elements sont ceux des pieces recues (`v01r08`) et des exemples du guide. `None` : ce n'est pas un rapport
/// harmonise (un XML d'un autre format, un PDF, une archive), ou il ne se lit pas.
pub fn lire_rapport(o: &[u8]) -> Option<Value> {
    let arbre = lire_octets(o).ok()?;
    let racine = arbre.racine();
    if local(&racine.tag) != "rapport" {
        return None;
    }
    let type_rapport = racine.get("type").unwrap_or("").to_string();
    let libelle = TYPES_RAPPORT.iter().find(|(c, _)| *c == type_rapport).map(|(_, l)| l.to_string()).unwrap_or_default();
    let envoi = arbre.tous().find(|e| local(&e.tag) == "envoi");
    let etat_envoi = envoi.map(|e| premier_texte(&arbre, e, &["envoi_etat"])).unwrap_or("");
    let essai_reel = envoi.map(|e| premier_texte(&arbre, e, &["essai_reel"])).unwrap_or("");
    let mut declarations = Vec::new();
    for d in arbre.tous().filter(|e| local(&e.tag) == "declaration") {
        let id = arbre.sous(d).find(|x| local(&x.tag) == "declaration_identification").unwrap_or(d);
        let bilan = arbre.sous(d).find(|x| local(&x.tag) == "declaration_bilan").unwrap_or(d);
        declarations.push(json!({
            "nature": premier_texte(&arbre, id, &["nature"]), "type": premier_texte(&arbre, id, &["type"]),
            "ordre": premier_texte(&arbre, id, &["ordre"]), "mois": mois_lisible(premier_texte(&arbre, id, &["mois"])),
            "etat": premier_texte(&arbre, bilan, &["etat", "etat_conformite"]),
        }));
    }
    let mut anomalies = Vec::new();
    for a in arbre.tous().filter(|e| {
        let n = local(&e.tag);
        n == "anomalie" || n.ends_with("_anomalie")
    }) {
        let rubrique = match arbre.sous(a).find(|x| local(&x.tag) == "rubrique_declenchante") {
            Some(r) => premier_texte(&arbre, r, &["identifiant", "rubrique"]).to_string(),
            None => premier_texte(&arbre, a, &["identifiant"]).to_string(),
        };
        let salarie = [premier_texte(&arbre, a, &["nom_famille"]), premier_texte(&arbre, a, &["prenoms"])]
            .iter()
            .filter(|s| !s.is_empty())
            .cloned()
            .collect::<Vec<&str>>()
            .join(" ");
        let (code, message) = (premier_texte(&arbre, a, &["code"]), premier_texte(&arbre, a, &["message"]));
        if code.is_empty() && message.is_empty() {
            continue;
        }
        anomalies.push(json!({
            "code": code, "categorie": premier_texte(&arbre, a, &["categorie"]), "message": message, "rubrique": rubrique,
            "ligne": premier_texte(&arbre, a, &["numero_ligne"]), "nombre": premier_texte(&arbre, a, &["nombre"]),
            "salarie": salarie,
        }));
    }
    let mut informations = Vec::new();
    for c in arbre.tous().filter(|e| local(&e.tag) == "information_collective") {
        let donnees: Vec<Value> = arbre
            .sous(c)
            .skip(1)
            .filter(|x| x.enfants.is_empty() && !x.texte_net().is_empty() && local(&x.tag) != "type")
            .map(|x| json!({"nom": local(&x.tag), "valeur": x.texte_net()}))
            .collect();
        informations.push(json!({"type": premier_texte(&arbre, c, &["type"]), "donnees": donnees}));
    }
    Some(json!({
        "type": type_rapport, "libelle": libelle, "message": racine.get("message").unwrap_or(""),
        "etat_envoi": etat_envoi, "essai_reel": essai_reel, "declarations": declarations, "anomalies": anomalies,
        "informations": informations,
    }))
}

/// Les noms des membres d'une archive ZIP (une archive d'AER, nature 43 ou 46) ; vide si ce n'en est pas une.
pub fn membres_du_zip(o: &[u8]) -> Vec<String> {
    repertoire_zip(o).map(|m| m.into_iter().map(|x| x.nom).filter(|n| !n.ends_with('/')).collect()).unwrap_or_default()
}

/// Le nombre de taux de PAS qu'un compte rendu de nature 94 communique (`extraire_taux_pas`, les quatre strategies).
pub fn nombre_de_taux_pas(o: &[u8]) -> usize {
    extraire_taux_pas(o).ok().and_then(|v| v["releves"].as_array().map(Vec::len)).unwrap_or(0)
}

// ------------------------------------------------ un compte rendu NEOReS, lu

/// `RO.B001.R001` « Declencheur du retour organisme » : cahier technique NEOReS 2026.1.2, « Rubriques », bloc RO.B001
/// (`atelier/reference/sources/neores/gip-mds-neores-cahier-technique-2026.1.2-20251024.txt`) ; 05 et 06 viennent du
/// cahier 2027.1.0 (`gip-mds-neores-cahier-technique-2027.1.0-20260303.txt`, et la note differentielle, § II).
const DECLENCHEURS_NEORES: [(&str, &str); 6] = [
    ("01", "retour sur une DSN mensuelle ou un signalement d'événement"),
    ("02", "retour en fin d'exercice"),
    ("03", "rappel d'actions non réalisées"),
    ("04", "paramètre à prendre en compte en paie"),
    ("05", "récapitulatif des substitutions de données opérées par l'organisme"),
    ("06", "retour informatif"),
];

/// `RO.B004.R003` « Type d'action » (cahier 2026.1.2, « Rubriques », bloc RO.B004).
const TYPES_ACTION_NEORES: [(&str, &str); 5] = [
    ("01", "correction de données DSN précédemment déclarées"),
    ("02", "correction de données d'identification d'individus"),
    ("03", "émission de l'ensemble des données attendues par l'organisme pour un mois principal déclaré"),
    ("04", "prise en compte d'un paramètre"),
    ("05", "régularisation d'un paramètre"),
];

/// `RO.B004.R006` et `RO.B007.R009` « Resultat du controle » (cahier 2026.1.2, blocs RO.B004 et RO.B007).
const RESULTATS_NEORES: [(&str, &str); 7] = [
    ("01", "valeur incohérente"),
    ("02", "rubrique absente"),
    ("03", "rubrique présente à tort"),
    ("04", "bloc absent"),
    ("05", "bloc présent à tort"),
    ("06", "cumul incohérent"),
    ("07", "ensemble des données attendues pour le mois absentes"),
];

/// `RO.B004.R016` (cahier 2026.1.2) puis `RO.B005.R005` (cahier 2027.1.0) « Caractere substituable de la donnee ».
const SUBSTITUABLE_NEORES: [(&str, &str); 2] = [
    ("01", "donnée substituable : sans correction ni opposition de votre part, l'Urssaf ou la MSA la corrigera elle-même"),
    ("02", "donnée non substituable"),
];

/// La table des types de parametres (`RO.B008.R001`), telle que net-entreprises la sert
/// (`atelier/reference/sources/neores/gip-mds-neores-table-types-de-parametres-20230703.csv`).
const TYPES_PARAMETRE_NEORES: [(&str, &str); 6] = [
    ("A01", "effectif moyen annuel (niveau entreprise)"),
    ("A02", "effectif moyen BOETH annuel (niveau entreprise)"),
    ("A03", "effectif moyen ECAP annuel (niveau entreprise)"),
    ("A04", "effectif moyen d'assujettissement OETH annuel (niveau entreprise)"),
    ("A05", "effectif moyen CSA CFIP annuel (niveau entreprise)"),
    ("A07", "effectif moyen CSA annuel (niveau entreprise)"),
];

fn dans_table(table: &[(&str, &'static str)], code: &str) -> &'static str {
    table.iter().find(|(c, _)| *c == code).map(|(_, l)| *l).unwrap_or("")
}

/// Le texte de la rubrique `nom` (`RO_B004_R001`), enfant direct du bloc `e`, ou `""`.
fn rubrique_de<'a>(a: &'a Arbre, e: &'a Noeud, nom: &str) -> &'a str {
    a.enfants(e).find(|x| local(&x.tag) == nom).map(Noeud::texte_net).unwrap_or("")
}

/// Les blocs `nom` sous `e` (lui exclu), dans l'ordre du document.
fn blocs_sous<'a>(a: &'a Arbre, e: &'a Noeud, nom: &'a str) -> Vec<&'a Noeud> {
    a.sous(e).skip(1).filter(|x| local(&x.tag) == nom).collect()
}

/// Un compte rendu au FORMAT NEOReS (CRM 118 a 121, 124, 125, 130, 132, 135, 136 : guide de l'API DSN v6.8, §6.8 et
/// tableau des retours), lu d'apres son cahier technique : CT 2026.1.2 et CT 2027.1.0
/// (`atelier/reference/sources/neores/`). Les blocs et rubriques s'y nomment `RO.Bnnn.Rnnn`, ecrits `RO_Bnnn_Rnnn` dans le
/// fichier transmis (CT 2026.1.2, §2.2). Un bloc est un element qui porte ses rubriques et ses blocs enfants (les
/// arborescences, §5) : une INFERENCE sur la forme XML (le schema XSD n'est pas public : net-entreprises sert le cahier
/// technique, pas le schema), celle que le lecteur des effectifs eprouve sur les 118 recus depuis 2022
/// (`releve_retours::reconstruire_effectifs`). Rend :
/// - `retour` : le code, le libelle, l'organisme emetteur (`RO.B002.R002`, `.R003`, `.R005`), le declencheur
///   (`RO.B001.R001`), l'annule-et-remplace (`RO.B002.R004` = 02), la periode (`.R008`, `.R009`) ;
/// - `conformite` et `metier` (`RO.B003.R002`, `.R001`) ;
/// - `actions` : chaque cible d'action (`RO.B004`), ses details (`RO.B005`), leurs individus (`RO.B006`), leurs
///   donnees DSN (`RO.B007`, valeurs acceptees `RO.B010`) ;
/// - `parametres` (`RO.B008`) et `identites` (`RO.B009`) ; `informations` (`RO.B011`, `RO.B012`, CT 2027.1.0).
///
/// `None` : aucun bloc NEOReS (`RO_B000` a `RO_B012`), ou le fichier ne se lit pas.
pub fn lire_neores(o: &[u8]) -> Option<Value> {
    let a = lire_octets(o).ok()?;
    let est_bloc = |n: &Noeud| {
        let t = local(&n.tag);
        t.len() == 7 && t.starts_with("RO_B") && t[4..].chars().all(|c| c.is_ascii_digit())
    };
    if !a.tous().any(est_bloc) {
        return None;
    }
    let premier = |nom: &str| a.tous().find(|x| local(&x.tag) == nom);
    let r = |bloc: Option<&Noeud>, nom: &str| bloc.map(|b| rubrique_de(&a, b, nom).to_string()).unwrap_or_default();
    let (b000, b001, b002, b003) = (premier("RO_B000"), premier("RO_B001"), premier("RO_B002"), premier("RO_B003"));
    let version = a.tous().find(|x| local(&x.tag) == "RO_R001").map(|x| x.texte_net().to_string()).unwrap_or_default();
    let declencheur = r(b001, "RO_B001_R001");
    let individu = |b: &Noeud, p: &str| {
        json!({
            "nom": rubrique_de(&a, b, &format!("{}_R003", p)), "prenom": rubrique_de(&a, b, &format!("{}_R004", p)),
            "matricule": rubrique_de(&a, b, &format!("{}_R005", p)), "debut_contrat": rubrique_de(&a, b, &format!("{}_R006", p)),
            "contrat": rubrique_de(&a, b, &format!("{}_R007", p)),
        })
    };
    let mut actions = Vec::new();
    for c in a.tous().filter(|x| local(&x.tag) == "RO_B004") {
        let rc = |nom: &str| rubrique_de(&a, c, nom).to_string();
        let mut details = Vec::new();
        for d in blocs_sous(&a, c, "RO_B005") {
            let rd = |nom: &str| rubrique_de(&a, d, nom).to_string();
            let individus: Vec<Value> = blocs_sous(&a, d, "RO_B006").into_iter().map(|b| individu(b, "RO_B006")).collect();
            let donnees: Vec<Value> = blocs_sous(&a, d, "RO_B007")
                .into_iter()
                .map(|b| {
                    let acceptees: Vec<String> = blocs_sous(&a, b, "RO_B010").into_iter().map(|v| rubrique_de(&a, v, "RO_B010_R001").to_string()).collect();
                    let resultat = rubrique_de(&a, b, "RO_B007_R009");
                    json!({
                        "rubrique": rubrique_de(&a, b, "RO_B007_R001"), "declaree": rubrique_de(&a, b, "RO_B007_R002"),
                        "attendue": rubrique_de(&a, b, "RO_B007_R003"), "cumul": rubrique_de(&a, b, "RO_B007_R004") == "01",
                        "debut": rubrique_de(&a, b, "RO_B007_R005"), "fin": rubrique_de(&a, b, "RO_B007_R006"),
                        "resultat": dans_table(&RESULTATS_NEORES, resultat), "acceptees": acceptees,
                    })
                })
                .collect();
            details.push(json!({
                "atypie": rd("RO_B005_R001"), "debut": rd("RO_B005_R002"), "fin": rd("RO_B005_R003"), "montant": rd("RO_B005_R004"),
                // CT 2027.1.0 : le caractere substituable et le rappel descendent au detail de l'action (note differentielle, § III).
                "substituable": dans_table(&SUBSTITUABLE_NEORES, &rd("RO_B005_R005")), "rappel": rd("RO_B005_R006"),
                "interlocuteur": rd("RO_B005_R007"), "individus": individus, "donnees": donnees,
            }));
        }
        actions.push(json!({
            "code": rc("RO_B004_R001"), "libelle": rc("RO_B004_R002"), "type_code": rc("RO_B004_R003"), "type": dans_table(&TYPES_ACTION_NEORES, &rc("RO_B004_R003")),
            "resultat": dans_table(&RESULTATS_NEORES, &rc("RO_B004_R006")), "individus": rc("RO_B004_R009"), "contrats": rc("RO_B004_R010"),
            "controle": rc("RO_B004_R011"), "reference": rc("RO_B004_R012"), "action": rc("RO_B004_R013"), "aide": rc("RO_B004_R014"),
            "consequences": rc("RO_B004_R015"), "substituable": dans_table(&SUBSTITUABLE_NEORES, &rc("RO_B004_R016")),
            "rappel": rc("RO_B004_R017"), "exception": rc("RO_B004_R018"), "details": details,
        }));
    }
    let parametres: Vec<Value> = a
        .tous()
        .filter(|x| local(&x.tag) == "RO_B008")
        .map(|p| {
            let t = rubrique_de(&a, p, "RO_B008_R001");
            json!({
                "type": t, "libelle": dans_table(&TYPES_PARAMETRE_NEORES, t), "valeur": rubrique_de(&a, p, "RO_B008_R002"),
                "effet": rubrique_de(&a, p, "RO_B008_R003"), "fin": rubrique_de(&a, p, "RO_B008_R004"),
                "rubrique": rubrique_de(&a, p, "RO_B008_R005"), "calcule_le": rubrique_de(&a, p, "RO_B008_R006"),
            })
        })
        .collect();
    let identites: Vec<Value> = a
        .tous()
        .filter(|x| local(&x.tag) == "RO_B009")
        .map(|b| {
            let x = |n: &str| rubrique_de(&a, b, n);
            let declare = if x("RO_B009_R001").is_empty() { x("RO_B009_R002") } else { x("RO_B009_R001") };
            json!({
                "nir_declare": declare, "nir_reference": x("RO_B009_R003"),
                "nom_declare": x("RO_B009_R004"), "nom_reference": x("RO_B009_R005"),
                "prenom_declare": x("RO_B009_R006"), "prenom_reference": x("RO_B009_R007"),
                "naissance_declaree": x("RO_B009_R008"), "naissance_reference": x("RO_B009_R009"), "matricule": x("RO_B009_R010"),
            })
        })
        .collect();
    let informations: Vec<Value> = a
        .tous()
        .filter(|x| local(&x.tag) == "RO_B011")
        .map(|b| {
            let x = |n: &str| rubrique_de(&a, b, n);
            let individus: Vec<Value> = blocs_sous(&a, b, "RO_B012").into_iter().map(|i| individu(i, "RO_B012")).collect();
            json!({
                "code": x("RO_B011_R001"), "libelle": x("RO_B011_R002"), "numero": x("RO_B011_R003"), "information": x("RO_B011_R004"),
                "reference": x("RO_B011_R005"), "aide": x("RO_B011_R006"), "individus": individus,
            })
        })
        .collect();
    Some(json!({
        "version": version,
        "declencheur": declencheur, "declencheur_libelle": dans_table(&DECLENCHEURS_NEORES, &declencheur),
        "code": r(b002, "RO_B002_R002"), "libelle": r(b002, "RO_B002_R003"), "remplace": r(b002, "RO_B002_R004") == "02",
        "organisme": r(b002, "RO_B002_R005"), "debut": r(b002, "RO_B002_R008"), "fin": r(b002, "RO_B002_R009"),
        "produit_le": r(b002, "RO_B002_R011"),
        "mois": r(b000, "RO_B000_R006"), "essai_reel": r(b000, "RO_B000_R013"), "ordre": r(b000, "RO_B000_R012"),
        "metier": r(b003, "RO_B003_R001"), "conformite": r(b003, "RO_B003_R002"),
        "actions": actions, "parametres": parametres, "identites": identites, "informations": informations,
    }))
}

// ------------------------------- un compte rendu d'organisme complementaire, lu

/// `Declaration/@CodeRetourDecla`, le code retour global : schema CRM DSN OC P3 V1.1.4
/// (`atelier/reference/sources/gip-mds-api-dsn/ctip-crm-dsn-oc-p3-v1.1.4-schema-20230915.xsd`, l. 1001-1013) et
/// specification V1.1.4 du 17/06/2025 (`ctip-crm-dsn-oc-p3-v1.1.4-20250617.txt`, niveau 2, « Code retour global »).
const RETOURS_GLOBAUX_OC: [(&str, &str); 4] = [
    ("0", "DSN acceptée"),
    ("1", "DSN acceptée, avec des corrections à apporter (anomalies non bloquantes)"),
    ("2", "DSN partiellement prise en compte (des erreurs empêchent la prise en compte de certains salariés ou de certaines données)"),
    ("3", "DSN non prise en compte par l'organisme (erreurs au niveau de la déclaration)"),
];

/// `RetourMetier/@CriticiteRetour` (schema V1.1.4, l. 514-523).
const CRITICITES_OC: [(&str, &str); 2] = [("C", "critique : à corriger impérativement dans la DSN suivante"), ("P", "passable : à corriger dès que possible")];

/// Un compte rendu metier d'organisme complementaire (nature 51 : guide de l'API DSN v6.8, tableau des retours,
/// « CRM-DSN-OC-P3-V1.1.4-17062025 »), lu d'apres son schema et sa specification
/// (`atelier/reference/sources/gip-mds-api-dsn/ctip-crm-dsn-oc-p3-v1.1.4-schema-20230915.xsd`,
/// `ctip-crm-dsn-oc-p3-v1.1.4-20250617.txt`) : l'element `Fichier`, ses `Declaration` (code retour global, complement,
/// producteur, contact, compteurs), leurs mandats SEPA a creer (`MandatNetE`, `MandatProp`) et leurs `RetourMetier`
/// (code « SXXXnn », libelle, description, criticite, rubrique ou bloc a l'origine, valeur declaree, valeur corrigee ou
/// attendue, date d'evenement, regroupement) avec leur `ContexteRetourMetier` (rubrique, libelle, valeur) -- d'ou le
/// salarie (S21.G00.30.002 et .004). `None` : ce n'en est pas un, ou il ne se lit pas (il est en ISO-8859-1, qu'il
/// declare : guide v6.8, §7.1).
pub fn lire_crm_oc(o: &[u8]) -> Option<Value> {
    let a = lire_octets(o).ok()?;
    let racine = a.racine();
    if local(&racine.tag) != "Fichier" || !a.enfants(racine).any(|x| local(&x.tag) == "Declaration") {
        return None;
    }
    let at = |n: &Noeud, k: &str| n.get(k).map(nettoye).unwrap_or("").to_string();
    let mut declarations = Vec::new();
    for d in a.enfants(racine).filter(|x| local(&x.tag) == "Declaration") {
        let mandats: Vec<Value> = a
            .enfants(d)
            .filter(|x| matches!(local(&x.tag), "MandatNetE" | "MandatProp"))
            .map(|m| json!({"net_entreprises": local(&m.tag) == "MandatNetE", "iban": at(m, "IBAN"), "bic": at(m, "BIC"), "ics": at(m, "ICS"), "url": at(m, "URLMandatProp")}))
            .collect();
        let mut retours = Vec::new();
        for rm in a.enfants(d).filter(|x| local(&x.tag) == "RetourMetier") {
            let contexte: Vec<Value> = a
                .enfants(rm)
                .filter(|x| local(&x.tag) == "ContexteRetourMetier")
                .map(|c| json!({"rubrique": at(c, "Rubrique"), "libelle": at(c, "Libelle"), "valeur": at(c, "Valeur"), "libelle_valeur": at(c, "LibelVal")}))
                .collect();
            let du_contexte = |r: &str| contexte.iter().find(|c| c["rubrique"] == json!(r)).map(|c| texte_ou_vide(&c["valeur"])).unwrap_or_default();
            let salarie = [du_contexte("S21.G00.30.002"), du_contexte("S21.G00.30.004")].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(" ");
            retours.push(json!({
                "code": at(rm, "CodeRetourMetier"), "libelle": at(rm, "LibelCodeRetourMetier"), "description": at(rm, "DescriptionRetour"),
                "criticite": dans_table(&CRITICITES_OC, &at(rm, "CriticiteRetour")), "nombre": at(rm, "NbErreursSimilaires"),
                "rubrique": at(rm, "RubriqueBlocRetour"), "libelle_rubrique": at(rm, "LibelRubBlocRetour"),
                "valeur": at(rm, "ValeurRubriqueErreur"), "libelle_valeur": at(rm, "LibelValRubriqueErreur"),
                "corrigee": at(rm, "ValeurCorrigeeAjoutee"), "libelle_corrigee": at(rm, "LibelValCorrigeeAjoutee"),
                "date_evenement": at(rm, "DateEvt"), "salarie": salarie, "contexte": contexte,
            }));
        }
        let code = at(d, "CodeRetourDecla");
        declarations.push(json!({
            "nature": at(d, "NatureDecla"), "type": at(d, "TypeDecla"), "ordre": at(d, "NoOrdreDecla"), "mois": mois_de_jjmmaaaa(&at(d, "DateMoisDec")),
            "code_oc": at(d, "CodeOC"), "code_delegataire": at(d, "CodeDELEG"), "producteur": at(d, "NomProducteurCRM"),
            "code_producteur": at(d, "CodeProducteurCRM"), "contact": at(d, "NomContact"), "courriel": at(d, "MailContact"), "telephone": at(d, "TelContact"),
            "code_retour": code, "retour_global": dans_table(&RETOURS_GLOBAUX_OC, &code), "complement": at(d, "ComplementRetourDecla"),
            "erreurs": at(d, "NbreErreur"), "critiques": at(d, "NbreCritique"), "affiliations": at(d, "NbreAff"), "radiations": at(d, "NbreRad"),
            "salaries": at(d, "NbreSal"), "mandats": mandats, "retours": retours,
        }));
    }
    Some(json!({"type_crm": at(racine, "TypCRM"), "essai_reel": at(racine, "CodeEnvoi"), "version": at(racine, "VersionCRM"), "declarations": declarations}))
}

fn texte_ou_vide(v: &Value) -> String {
    v.as_str().unwrap_or("").to_string()
}

/// `DateMoisDec` est le dernier jour du mois au format JJMMAAAA (exemples de la fiche 692 : « 31032023 ») : `AAAA-MM`.
fn mois_de_jjmmaaaa(brut: &str) -> String {
    let c: Vec<char> = brut.chars().collect();
    if c.len() == 8 && c.iter().all(|x| x.is_ascii_digit()) {
        return format!("{}-{}", c[4..8].iter().collect::<String>(), c[2..4].iter().collect::<String>());
    }
    brut.to_string()
}
