
//! CE QUE REND UNE ROUTE TYPEE, lu dans les sources Rust de l'espace, tel que `serde` l'ecrit. Le bras qui sert une route
//! est celui du module qui declare sa fiche (son `const ROUTES`) ; s'il finit par `en_json(f(…)?)`, `en_json(x.f(…)?)`,
//! `en_json(Nom { … })` ou `en_json(r)` (`r` lie par `let r = f(…)?`), la route rend ce type (sans son `Result`) : ses
//! champs (`rename`, `rename_all`, `flatten`, `skip`), ses variantes (`untagged`, `tag`) et ceux qu'il porte, jusqu'aux
//! feuilles, chaque nom trouve par son module, son fichier ou son `use`. Un type ecrit a la main (`impl Serialize`,
//! `serialize_with`) ou un `Value` est une forme ouverte : rien ne s'y verifie. Ce qui ne se lit pas ici (un type
//! introuvable, ambigu, un bras `en_json` d'une autre tournure) refuse : la forme d'une reponse typee se lit toujours.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

/// Un jeton d'un source Rust : un mot, une ponctuation (`::`, `->`, `=>` d'un tenant), un litteral de chaine (son
/// contenu), une duree de vie, un nombre ou un caractere.
#[derive(Clone, Debug, PartialEq)]
pub enum J {
    Mot(String),
    P(&'static str),
    Chaine(String),
    Autre,
}

const PONCTUATIONS: [&str; 26] = [
    "::", "->", "=>", "#", "[", "]", "(", ")", "{", "}", "<", ">", ",", ";", ":", "=", "&", "|", "?", "!", ".", "*", "+", "-", "/", "@",
];

/// Les jetons d'un source Rust, commentaires retires.
pub fn jetons(t: &str) -> Vec<J> {
    let c: Vec<char> = t.chars().collect();
    let mut v = Vec::new();
    let mut i = 0;
    while i < c.len() {
        let x = c[i];
        if x.is_whitespace() {
            i += 1;
        } else if x == '/' && c.get(i + 1) == Some(&'/') {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
        } else if x == '/' && c.get(i + 1) == Some(&'*') {
            let mut p = 0;
            while i < c.len() {
                if c[i] == '/' && c.get(i + 1) == Some(&'*') {
                    p += 1;
                    i += 2;
                } else if c[i] == '*' && c.get(i + 1) == Some(&'/') {
                    p -= 1;
                    i += 2;
                    if p == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
        } else if (x == 'r' || (x == 'b' && c.get(i + 1) == Some(&'r'))) && {
            let j = if x == 'b' { i + 2 } else { i + 1 };
            let mut k = j;
            while c.get(k) == Some(&'#') {
                k += 1;
            }
            c.get(k) == Some(&'"') && (k > j || c.get(j) == Some(&'"'))
        } {
            // Une chaine brute : r"…", r#"…"#, br"…".
            let mut k = if x == 'b' { i + 2 } else { i + 1 };
            let mut dieses = 0;
            while c[k] == '#' {
                dieses += 1;
                k += 1;
            }
            k += 1;
            let debut = k;
            loop {
                if c[k] == '"' && (0..dieses).all(|d| c.get(k + 1 + d) == Some(&'#')) {
                    break;
                }
                k += 1;
            }
            v.push(J::Chaine(c[debut..k].iter().collect()));
            i = k + 1 + dieses;
        } else if x == '"' || (x == 'b' && c.get(i + 1) == Some(&'"')) {
            let mut k = if x == 'b' { i + 2 } else { i + 1 };
            let mut s = String::new();
            while c[k] != '"' {
                if c[k] == '\\' {
                    k += 1;
                }
                s.push(c[k]);
                k += 1;
            }
            v.push(J::Chaine(s));
            i = k + 1;
        } else if x == '\'' || (x == 'b' && c.get(i + 1) == Some(&'\'')) {
            // Un caractere ('a', '\n', '\u{…}') ou une duree de vie ('a, 'static).
            let k = if x == 'b' { i + 1 } else { i };
            if c.get(k + 1) == Some(&'\\') {
                let mut m = k + 2;
                while c[m] != '\'' {
                    m += 1;
                }
                i = m + 1;
            } else if c.get(k + 2) == Some(&'\'') {
                i = k + 3;
            } else {
                i = k + 1;
                while i < c.len() && (c[i].is_alphanumeric() || c[i] == '_') {
                    i += 1;
                }
            }
            v.push(J::Autre);
        } else if x.is_alphabetic() || x == '_' {
            let debut = i;
            while i < c.len() && (c[i].is_alphanumeric() || c[i] == '_') {
                i += 1;
            }
            v.push(J::Mot(c[debut..i].iter().collect()));
        } else if x.is_ascii_digit() {
            while i < c.len() && (c[i].is_alphanumeric() || c[i] == '_' || (c[i] == '.' && c.get(i + 1).is_some_and(|d| d.is_ascii_digit()))) {
                i += 1;
            }
            v.push(J::Autre);
        } else {
            let deux: String = c[i..(i + 2).min(c.len())].iter().collect();
            if let Some(p) = PONCTUATIONS.iter().find(|p| p.len() == 2 && **p == deux) {
                v.push(J::P(p));
                i += 2;
            } else if let Some(p) = PONCTUATIONS.iter().find(|p| p.len() == 1 && p.starts_with(x)) {
                v.push(J::P(p));
                i += 1;
            } else {
                v.push(J::Autre);
                i += 1;
            }
        }
    }
    v
}

fn mot(j: Option<&J>) -> Option<&str> {
    match j {
        Some(J::Mot(m)) => Some(m),
        _ => None,
    }
}

fn est(j: Option<&J>, p: &str) -> bool {
    matches!(j, Some(J::P(x)) if *x == p)
}

/// L'indice qui suit le groupe ouvert en `i` (`(`, `[`, `{`, ou `<` si `chevrons`).
fn apres_le_groupe(v: &[J], i: usize, chevrons: bool) -> usize {
    let mut p = 0i32;
    let mut k = i;
    while k < v.len() {
        match &v[k] {
            J::P("(" | "[" | "{") => p += 1,
            J::P(")" | "]" | "}") => p -= 1,
            J::P("<") if chevrons => p += 1,
            J::P(">") if chevrons => p -= 1,
            _ => {}
        }
        k += 1;
        if p == 0 {
            return k;
        }
    }
    v.len()
}

/// Les morceaux de `v` separes par `sep` hors de tout groupe (`<…>` compte si `chevrons`).
fn morceaux<'a>(v: &'a [J], sep: &str, chevrons: bool) -> Vec<&'a [J]> {
    let mut out = Vec::new();
    let (mut p, mut debut) = (0i32, 0);
    for (k, j) in v.iter().enumerate() {
        match j {
            J::P("(" | "[" | "{") => p += 1,
            J::P(")" | "]" | "}") => p -= 1,
            J::P("<") if chevrons => p += 1,
            J::P(">") if chevrons => p -= 1,
            J::P(x) if p == 0 && *x == sep => {
                out.push(&v[debut..k]);
                debut = k + 1;
            }
            _ => {}
        }
    }
    if debut < v.len() {
        out.push(&v[debut..]);
    }
    out
}

/// Un type Rust tel qu'il s'ecrit : un chemin et ses arguments, un tableau ou une tranche, un n-uplet ; une reference
/// est son type.
#[derive(Clone, Debug)]
pub enum Ty {
    Chemin(Vec<String>, Vec<Ty>),
    Liste(Box<Ty>),
    Uplet(Vec<Ty>),
    Illisible(String),
}

fn texte(v: &[J]) -> String {
    v.iter()
        .map(|j| match j {
            J::Mot(m) => m.clone(),
            J::P(p) => p.to_string(),
            J::Chaine(s) => format!("{:?}", s),
            J::Autre => "'".into(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn lire_ty(v: &[J]) -> Ty {
    let mut v = v;
    // `&'a mut T`, `&T`, `dyn`, la duree de vie et `mut` : le type est ce qui suit.
    while let Some(j) = v.first() {
        match j {
            J::P("&") | J::Autre => v = &v[1..],
            J::Mot(m) if m == "mut" => v = &v[1..],
            _ => break,
        }
    }
    match v.first() {
        Some(J::P("[")) => {
            let fin = apres_le_groupe(v, 0, false);
            let dedans = &v[1..fin - 1];
            let t = morceaux(dedans, ";", true);
            Ty::Liste(Box::new(lire_ty(t.first().copied().unwrap_or(&[]))))
        }
        Some(J::P("(")) => {
            let fin = apres_le_groupe(v, 0, false);
            Ty::Uplet(morceaux(&v[1..fin - 1], ",", true).into_iter().filter(|m| !m.is_empty()).map(lire_ty).collect())
        }
        Some(J::Mot(_)) => {
            let mut segments = Vec::new();
            let mut args = Vec::new();
            let mut k = 0;
            while let Some(m) = mot(v.get(k)) {
                segments.push(m.to_string());
                k += 1;
                if est(v.get(k), "::") && est(v.get(k + 1), "<") {
                    k += 1;
                }
                if est(v.get(k), "<") {
                    let fin = apres_le_groupe(v, k, true);
                    args = morceaux(&v[k + 1..fin - 1], ",", true)
                        .into_iter()
                        .filter(|m| !m.is_empty() && !matches!(m.first(), Some(J::Autre)))
                        .map(lire_ty)
                        .collect();
                    k = fin;
                }
                if est(v.get(k), "::") {
                    k += 1;
                } else {
                    break;
                }
            }
            if k < v.len() {
                return Ty::Illisible(texte(v));
            }
            Ty::Chemin(segments, args)
        }
        _ => Ty::Illisible(texte(v)),
    }
}

/// Un item de l'espace : une structure, une enumeration ou un alias, et ce que `serde` en dit.
#[derive(Debug)]
pub struct Item {
    pub fichier: usize,
    pub nom: String,
    pub genre: &'static str,
    pub generiques: Vec<String>,
    /// Les attributs `serde` de l'item (`rename_all`, `untagged`, `tag`, `into`, `transparent`).
    pub serde: Vec<(String, Option<String>)>,
    pub serialise: bool,
    pub corps: Vec<J>,
}

/// Une fonction de l'espace : son nom et son type de retour.
#[derive(Debug)]
pub struct Fonction {
    pub fichier: usize,
    pub nom: String,
    pub retour: Vec<J>,
}

/// Les attributs `serde` d'un attribut `#[…]` (ses jetons, sans `#[` ni `]`) : `(cle, valeur)`.
fn attributs_serde(a: &[J]) -> Vec<(String, Option<String>)> {
    if mot(a.first()) != Some("serde") || !est(a.get(1), "(") {
        return Vec::new();
    }
    let fin = apres_le_groupe(a, 1, false);
    morceaux(&a[2..fin - 1], ",", false)
        .into_iter()
        .filter_map(|m| {
            let cle = mot(m.first())?.to_string();
            let valeur = match m.get(2) {
                Some(J::Chaine(s)) if est(m.get(1), "=") => Some(s.clone()),
                _ => None,
            };
            Some((cle, valeur))
        })
        .collect()
}

fn derive_serialize(a: &[J]) -> bool {
    mot(a.first()) == Some("derive") && a.iter().any(|j| matches!(j, J::Mot(m) if m == "Serialize"))
}

/// Ce que l'espace declare : ses fichiers, ses items, ses fonctions, les types qui s'ecrivent a la main.
pub struct Espace {
    pub fichiers: Vec<PathBuf>,
    pub jetons: Vec<Vec<J>>,
    pub items: Vec<Item>,
    pub fonctions: Vec<Fonction>,
    pub a_la_main: BTreeSet<String>,
    /// Ce que chaque fichier importe (`use`).
    pub uses: Vec<HashMap<String, Vec<String>>>,
}

impl Espace {
    pub fn lire(fichiers: Vec<PathBuf>) -> Espace {
        let mut e = Espace { fichiers: Vec::new(), jetons: Vec::new(), items: Vec::new(), fonctions: Vec::new(), a_la_main: BTreeSet::new(), uses: Vec::new() };
        for (n, f) in fichiers.into_iter().enumerate() {
            let v = jetons(&std::fs::read_to_string(&f).unwrap());
            e.lire_items(n, &v);
            e.uses.push(imports(&v));
            e.fichiers.push(f);
            e.jetons.push(v);
        }
        e
    }

    fn lire_items(&mut self, fichier: usize, v: &[J]) {
        let mut attrs: Vec<&[J]> = Vec::new();
        let mut i = 0;
        while i < v.len() {
            match (&v[i], v.get(i + 1)) {
                (J::P("#"), Some(J::P("["))) => {
                    let fin = apres_le_groupe(v, i + 1, false);
                    attrs.push(&v[i + 2..fin - 1]);
                    i = fin;
                    continue;
                }
                (J::P("#"), Some(J::P("!"))) => {
                    i = apres_le_groupe(v, i + 2, false);
                    continue;
                }
                (J::Mot(m), _) if m == "pub" => {
                    i += 1;
                    if est(v.get(i), "(") {
                        i = apres_le_groupe(v, i, false);
                    }
                    continue;
                }
                (J::Mot(m), Some(J::Mot(nom))) if m == "struct" || m == "enum" || m == "type" => {
                    let genre: &'static str = if m == "struct" { "struct" } else if m == "enum" { "enum" } else { "type" };
                    let mut k = i + 2;
                    let mut generiques = Vec::new();
                    if est(v.get(k), "<") {
                        let fin = apres_le_groupe(v, k, true);
                        for g in morceaux(&v[k + 1..fin - 1], ",", true) {
                            if let Some(n) = mot(g.first()).filter(|n| *n != "const") {
                                generiques.push(n.to_string());
                            }
                        }
                        k = fin;
                    }
                    // Le corps : `{…}`, `(…);`, `;` ou `= T;`, apres une eventuelle clause `where`.
                    while k < v.len() && !matches!(v[k], J::P("{" | "(" | ";" | "=")) {
                        k += 1;
                    }
                    let (corps, fin) = match v.get(k) {
                        Some(J::P("{" | "(")) => {
                            let f = apres_le_groupe(v, k, false);
                            (v[k..f].to_vec(), f)
                        }
                        Some(J::P("=")) => {
                            let mut f = k;
                            while f < v.len() && !est(v.get(f), ";") {
                                f += 1;
                            }
                            (v[k + 1..f].to_vec(), f)
                        }
                        _ => (Vec::new(), k),
                    };
                    let serde = attrs.iter().flat_map(|a| attributs_serde(a)).collect();
                    let serialise = attrs.iter().any(|a| derive_serialize(a));
                    self.items.push(Item { fichier, nom: nom.clone(), genre, generiques, serde, serialise, corps });
                    attrs.clear();
                    i = fin;
                    continue;
                }
                (J::Mot(m), Some(J::Mot(nom))) if m == "fn" => {
                    let mut k = i + 2;
                    if est(v.get(k), "<") {
                        k = apres_le_groupe(v, k, true);
                    }
                    if est(v.get(k), "(") {
                        k = apres_le_groupe(v, k, false);
                        let mut retour = Vec::new();
                        if est(v.get(k), "->") {
                            k += 1;
                            let mut p = 0i32;
                            while k < v.len() {
                                match &v[k] {
                                    J::P("{" | ";") if p == 0 => break,
                                    J::Mot(w) if w == "where" && p == 0 => break,
                                    J::P("<" | "(" | "[") => p += 1,
                                    J::P(">" | ")" | "]") => p -= 1,
                                    _ => {}
                                }
                                retour.push(v[k].clone());
                                k += 1;
                            }
                        }
                        self.fonctions.push(Fonction { fichier, nom: nom.clone(), retour });
                    }
                    attrs.clear();
                    i = k;
                    continue;
                }
                (J::Mot(m), _) if m == "impl" => {
                    // `impl … Serialize for Nom` : un type qui s'ecrit a la main.
                    let mut k = i + 1;
                    while k < v.len() && !matches!(v[k], J::P("{" | ";")) {
                        k += 1;
                    }
                    let tete = &v[i..k];
                    if let Some(p) = tete.iter().position(|j| matches!(j, J::Mot(w) if w == "for")) {
                        if tete[..p].iter().any(|j| matches!(j, J::Mot(w) if w == "Serialize")) {
                            if let Ty::Chemin(s, _) = lire_ty(&tete[p + 1..]) {
                                self.a_la_main.insert(s.last().unwrap().clone());
                            }
                        }
                    }
                    attrs.clear();
                    i += 1;
                    continue;
                }
                _ => {}
            }
            if !matches!(v[i], J::Mot(ref m) if m == "unsafe" || m == "async" || m == "const") {
                attrs.clear();
            }
            i += 1;
        }
    }

    /// Le chemin d'un fichier depuis l'espace, pour un message.
    pub fn nom_du_fichier(&self, n: usize, espace: &Path) -> String {
        self.fichiers[n].strip_prefix(espace).unwrap_or(&self.fichiers[n]).display().to_string()
    }

    /// Le fichier qu'un chemin de module designe (`crate::routes::mois`, `droit::fiche`), s'il en designe un.
    fn fichier_du_module(&self, fichier: usize, module: &[String]) -> Option<usize> {
        if module.is_empty() {
            return None;
        }
        let crate_de = |n: usize| -> String {
            let p = self.fichiers[n].to_string_lossy().replace('\\', "/");
            p.split("/src/").next().unwrap().rsplit('/').next().unwrap().to_string()
        };
        // Le module du fichier lui-meme (`routes/mois.rs` : routes, mois), pour `self` et `super`.
        let ici: Vec<String> = {
            let p = self.fichiers[fichier].to_string_lossy().replace('\\', "/");
            let apres = p.split("/src/").nth(1).unwrap_or("").trim_end_matches(".rs").to_string();
            let mut m: Vec<String> = apres.split('/').map(String::from).collect();
            if matches!(m.last().map(String::as_str), Some("mod" | "lib" | "main")) {
                m.pop();
            }
            m
        };
        let (racine, reste): (String, Vec<String>) = match module[0].as_str() {
            "crate" => (crate_de(fichier), module[1..].to_vec()),
            "self" => (crate_de(fichier), ici.iter().chain(&module[1..]).cloned().collect()),
            "super" => {
                let mut m = ici.clone();
                let mut k = 0;
                while module.get(k).map(String::as_str) == Some("super") {
                    m.pop();
                    k += 1;
                }
                (crate_de(fichier), m.into_iter().chain(module[k..].iter().cloned()).collect())
            }
            autre => (autre.to_string(), module[1..].to_vec()),
        };
        let chemin = reste.join("/");
        (0..self.fichiers.len()).find(|&n| {
            let p = self.fichiers[n].to_string_lossy().replace('\\', "/");
            crate_de(n) == racine
                && (p.ends_with(&format!("/src/{}.rs", chemin)) || p.ends_with(&format!("/src/{}/mod.rs", chemin)) || (chemin.is_empty() && p.ends_with("/src/lib.rs")))
        })
    }

    /// L'item `nom`, vu depuis `fichier` : celui du module que son chemin nomme, sinon celui du fichier, sinon celui que
    /// le fichier importe (`use`), sinon le seul de l'espace qui s'ecrive. `Err` dit pourquoi il ne se trouve pas.
    fn item(&self, fichier: usize, module: &[String], nom: &str) -> Result<Option<usize>, String> {
        let (module, nom) = match self.uses[fichier].get(nom) {
            Some(chemin) if module.is_empty() => (&chemin[..chemin.len() - 1], chemin.last().unwrap().as_str()),
            _ => (module, nom),
        };
        let tous: Vec<usize> = (0..self.items.len()).filter(|&n| self.items[n].nom == nom).collect();
        if let Some(f) = self.fichier_du_module(fichier, module) {
            if let Some(n) = tous.iter().copied().find(|&n| self.items[n].fichier == f) {
                return Ok(Some(n));
            }
        }
        if let Some(&n) = tous.iter().find(|&&n| self.items[n].fichier == fichier) {
            return Ok(Some(n));
        }
        let ecrits: Vec<usize> = tous.iter().copied().filter(|&n| self.items[n].serialise || self.items[n].genre == "type" || self.a_la_main.contains(nom)).collect();
        match ecrits.len() {
            0 if tous.is_empty() => Ok(None),
            0 => Ok(Some(tous[0])),
            1 => Ok(Some(ecrits[0])),
            _ => Err(format!("le type {} est declare plusieurs fois dans l'espace et {} ne dit pas lequel", nom, self.fichiers[fichier].display())),
        }
    }

    /// La fonction `nom`, vue depuis `fichier` : celle du module nomme ou importe, sinon du fichier, sinon la seule de
    /// l'espace.
    fn fonction(&self, fichier: usize, module: &[String], nom: &str) -> Result<&Fonction, String> {
        let (module, nom) = match self.uses[fichier].get(nom) {
            Some(chemin) if module.is_empty() => (&chemin[..chemin.len() - 1], chemin.last().unwrap().as_str()),
            _ => (module, nom),
        };
        let f = self.fichier_du_module(fichier, module).unwrap_or(fichier);
        let ici: Vec<&Fonction> = self.fonctions.iter().filter(|x| x.nom == nom && x.fichier == f).collect();
        let toutes: Vec<&Fonction> = self.fonctions.iter().filter(|x| x.nom == nom).collect();
        match (ici.len(), toutes.len()) {
            (1, _) => Ok(ici[0]),
            (0, 1) => Ok(toutes[0]),
            (0, 0) => Err(format!("la fonction {} est introuvable", nom)),
            _ => Err(format!("la fonction {} est declaree plusieurs fois ; nomme-la par son module", nom)),
        }
    }
}

/// Les noms qu'un `use` importe : `nom` (ou son `as`) -> son chemin complet. `use a::{b, c::d as e, f::*}`.
fn imports(v: &[J]) -> HashMap<String, Vec<String>> {
    fn arbre(v: &[J], prefixe: &[String], out: &mut HashMap<String, Vec<String>>) {
        let mut chemin = prefixe.to_vec();
        let mut k = 0;
        while k < v.len() {
            match &v[k] {
                J::Mot(m) if m == "as" => {
                    if let Some(alias) = mot(v.get(k + 1)) {
                        out.insert(alias.to_string(), chemin.clone());
                    }
                    return;
                }
                J::Mot(m) => chemin.push(m.clone()),
                J::P("::") => {}
                J::P("{") => {
                    let fin = apres_le_groupe(v, k, false);
                    for m in morceaux(&v[k + 1..fin - 1], ",", false) {
                        arbre(m, &chemin, out);
                    }
                    return;
                }
                _ => return,
            }
            k += 1;
        }
        match chemin.last().map(String::as_str) {
            Some("self") => {
                chemin.pop();
                if let Some(n) = chemin.last().cloned() {
                    out.insert(n, chemin);
                }
            }
            Some(n) => {
                out.insert(n.to_string(), chemin.clone());
            }
            None => {}
        }
    }
    let mut out = HashMap::new();
    let mut i = 0;
    while i < v.len() {
        if mot(v.get(i)) == Some("use") {
            let mut f = i + 1;
            while f < v.len() && !est(v.get(f), ";") {
                if est(v.get(f), "{") {
                    f = apres_le_groupe(v, f, false);
                } else {
                    f += 1;
                }
            }
            arbre(&v[i + 1..f], &[], &mut out);
            i = f;
        }
        i += 1;
    }
    out
}

/// Une forme de reponse, telle que `serde` l'ecrit ; ses parties sont des indices dans `Formes::arene`.
#[derive(Clone, Debug)]
pub enum Forme {
    /// Un objet : ses champs ; `ouvert` s'il en porte d'autres qu'on ne connait pas (un `flatten` d'une forme ouverte).
    Objet { champs: BTreeMap<String, usize>, ouvert: bool },
    Liste(usize),
    /// Une table : des cles quelconques, chacune de cette forme.
    Table(usize),
    /// Un texte, un nombre, un booleen, rien.
    Feuille,
    /// Ce qui ne se verifie pas : un `Value`, un type ecrit a la main.
    Ouverte,
    /// L'une ou l'autre (une enumeration).
    Union(Vec<usize>),
}

/// Les formes lues, et le type que chacune ecrit (pour un message).
pub struct Formes<'e> {
    pub espace: &'e Espace,
    pub arene: Vec<Forme>,
    pub type_de: Vec<String>,
    deja: HashMap<String, usize>,
}

const FEUILLES: [&str; 22] = [
    "String", "str", "bool", "char", "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize", "f32", "f64",
    "Decimal", "PathBuf", "Path", "Number",
];

fn renomme(nom: &str, regle: Option<&str>) -> String {
    match regle {
        Some("snake_case") => {
            let mut s = String::new();
            for (k, c) in nom.chars().enumerate() {
                if c.is_uppercase() && k > 0 {
                    s.push('_');
                }
                s.extend(c.to_lowercase());
            }
            s
        }
        Some("lowercase") => nom.to_lowercase(),
        Some("UPPERCASE") => nom.to_uppercase(),
        Some(autre) => panic!("rename_all = {:?} : la regle n'est pas lue ici ; ajoute-la a `renomme`", autre),
        None => nom.to_string(),
    }
}

impl<'e> Formes<'e> {
    pub fn new(espace: &'e Espace) -> Formes<'e> {
        Formes { espace, arene: Vec::new(), type_de: Vec::new(), deja: HashMap::new() }
    }

    fn poser(&mut self, f: Forme, t: &str) -> usize {
        self.arene.push(f);
        self.type_de.push(t.to_string());
        self.arene.len() - 1
    }

    /// La forme de ce que rend la route du bras `bras` (ses jetons, depuis `=>`) dans `fichier` : `Ok(None)` si le bras ne
    /// finit pas par `en_json(…)` (une route non typee).
    pub fn du_bras(&mut self, fichier: usize, bras: &[J]) -> Result<Option<usize>, String> {
        let (corps, bloc) = if est(bras.first(), "{") { (&bras[1..bras.len() - 1], true) } else { (bras, false) };
        let instructions = morceaux(corps, ";", false);
        let fin = if bloc && corps.last().is_some_and(|j| est(Some(j), ";")) { &[][..] } else { instructions.last().copied().unwrap_or(&[]) };
        let tete: Vec<&str> = fin.iter().take_while(|j| !est(Some(j), "(")).filter_map(|j| mot(Some(j))).collect();
        if tete.last() != Some(&"en_json") || !fin.iter().take_while(|j| !est(Some(j), "(")).all(|j| matches!(j, J::Mot(_)) || est(Some(j), "::")) {
            return Ok(None);
        }
        let ouvre = fin.iter().position(|j| est(Some(j), "(")).unwrap();
        let ferme = apres_le_groupe(fin, ouvre, false);
        let mut arg = &fin[ouvre + 1..ferme - 1];
        while est(arg.first(), "&") {
            arg = &arg[1..];
        }
        // `en_json(r)` : `r` se lie plus haut dans le bloc (`let r = f(…)?;`, `let mut r: T = …;`).
        if arg.len() == 1 {
            let nom = mot(arg.first()).ok_or("en_json(…) ne nomme ni une fonction ni une variable")?;
            let liaison = instructions
                .iter()
                .find_map(|i| {
                    let mut k = 0;
                    (mot(i.first()) == Some("let")).then_some(())?;
                    k += 1;
                    if mot(i.get(k)) == Some("mut") {
                        k += 1;
                    }
                    (mot(i.get(k)) == Some(nom)).then_some(())?;
                    Some(&i[k + 1..])
                })
                .ok_or_else(|| format!("en_json({}) : `{}` ne se lie pas dans le bras par `let`", nom, nom))?;
            if est(liaison.first(), ":") {
                let egal = liaison.iter().position(|j| est(Some(j), "=")).unwrap_or(liaison.len());
                return self.du_type(fichier, &lire_ty(&liaison[1..egal]), &HashMap::new()).map(Some);
            }
            arg = &liaison[1..];
        }
        // `Nom { … }` : une structure ecrite la.
        if est(arg.last(), "}") {
            let o = arg.iter().position(|j| est(Some(j), "{")).unwrap_or(0);
            if o > 0 && apres_le_groupe(arg, o, false) == arg.len() && arg[..o].iter().all(|j| matches!(j, J::Mot(_)) || est(Some(j), "::")) {
                return self.du_type(fichier, &lire_ty(&arg[..o]), &HashMap::new()).map(Some);
            }
        }
        // `f(…)?`, `module::f(…)?`, `f::<T>(…)?`, `x.methode(…)?` : le type que rend la fonction, sans son `Result`.
        let illisible = || format!("en_json({}) : la reponse se lit d'un appel `f(…)?`, d'une structure ecrite la ou d'une variable liee par `let`", texte(arg));
        if arg.len() < 4 || !est(arg.last(), "?") || !est(arg.get(arg.len() - 2), ")") {
            return Err(illisible());
        }
        let ouvre = (0..arg.len() - 2).rev().find(|&k| est(arg.get(k), "(") && apres_le_groupe(arg, k, false) == arg.len() - 1).ok_or_else(illisible)?;
        let mut k = ouvre;
        if est(arg.get(k - 1), ">") {
            // `f::<T>(…)` : le turbofish ne compte pas.
            let mut p = 0i32;
            while k > 0 {
                k -= 1;
                match &arg[k] {
                    J::P(">") => p += 1,
                    J::P("<") => {
                        p -= 1;
                        if p == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
            }
            k -= 1;
        }
        let nom = mot(arg.get(k - 1)).ok_or_else(illisible)?.to_string();
        let mut chemin = Vec::new();
        let mut d = k - 1;
        if !est(arg.get(d.wrapping_sub(1)), ".") {
            while d >= 2 && est(arg.get(d - 1), "::") {
                chemin.insert(0, mot(arg.get(d - 2)).ok_or_else(illisible)?.to_string());
                d -= 2;
            }
            if d != 0 {
                return Err(illisible());
            }
        }
        let f = self.espace.fonction(fichier, &chemin, &nom)?;
        let (ff, retour) = (f.fichier, f.retour.clone());
        let t = match lire_ty(&retour) {
            Ty::Chemin(s, mut a) if !a.is_empty() && ["R", "Result", "Resultat"].contains(&s.last().unwrap().as_str()) => a.remove(0),
            autre => return Err(format!("la fonction {} ne rend pas un `Result` lisible ({:?})", nom, autre)),
        };
        self.du_type(ff, &t, &HashMap::new()).map(Some)
    }

    /// La forme du type `t`, vu depuis `fichier`, ses parametres generiques remplaces (`subst`).
    pub fn du_type(&mut self, fichier: usize, t: &Ty, subst: &HashMap<String, (usize, Ty)>) -> Result<usize, String> {
        match t {
            Ty::Illisible(s) => Err(format!("le type « {} » ne se lit pas ici", s)),
            Ty::Liste(x) => {
                let e = self.du_type(fichier, x, subst)?;
                Ok(self.poser(Forme::Liste(e), "[…]"))
            }
            Ty::Uplet(v) if v.is_empty() => Ok(self.poser(Forme::Feuille, "()")),
            Ty::Uplet(v) => {
                let membres = v.iter().map(|x| self.du_type(fichier, x, subst)).collect::<Result<Vec<_>, _>>()?;
                let u = self.poser(Forme::Union(membres), "(…)");
                Ok(self.poser(Forme::Liste(u), "(…)"))
            }
            Ty::Chemin(segments, args) => {
                let nom = segments.last().unwrap().as_str();
                if segments.len() == 1 {
                    if let Some((f, x)) = subst.get(nom) {
                        return self.du_type(*f, &x.clone(), &HashMap::new());
                    }
                }
                let arg = |n: usize| args.get(n).cloned().ok_or_else(|| format!("{} sans son argument", nom));
                match nom {
                    _ if FEUILLES.contains(&nom) => return Ok(self.poser(Forme::Feuille, nom)),
                    "Value" => return Ok(self.poser(Forme::Ouverte, nom)),
                    "Map" => {
                        let o = self.poser(Forme::Ouverte, "Value");
                        return Ok(self.poser(Forme::Table(o), nom));
                    }
                    "Option" | "Box" | "Rc" | "Arc" | "Cow" => return self.du_type(fichier, &arg(0)?, subst),
                    "Vec" | "VecDeque" | "BTreeSet" | "HashSet" => {
                        let e = self.du_type(fichier, &arg(0)?, subst)?;
                        return Ok(self.poser(Forme::Liste(e), nom));
                    }
                    "BTreeMap" | "HashMap" | "IndexMap" => {
                        let e = self.du_type(fichier, &arg(1)?, subst)?;
                        return Ok(self.poser(Forme::Table(e), nom));
                    }
                    _ => {}
                }
                let module = &segments[..segments.len() - 1];
                let Some(n) = self.espace.item(fichier, module, nom)? else {
                    return Err(format!("le type {} est introuvable dans l'espace", nom));
                };
                let cle = format!("{}|{:?}", n, args.iter().map(|a| format!("{:?}", a)).collect::<Vec<_>>());
                if let Some(&id) = self.deja.get(&cle) {
                    return Ok(id);
                }
                let id = self.poser(Forme::Ouverte, nom);
                self.deja.insert(cle, id);
                let it = &self.espace.items[n];
                let sub: HashMap<String, (usize, Ty)> = it.generiques.iter().cloned().zip(args.iter().map(|a| (fichier, a.clone()))).collect();
                let f = self.de_l_item(n, &sub)?;
                self.arene[id] = f;
                Ok(id)
            }
        }
    }

    fn de_l_item(&mut self, n: usize, subst: &HashMap<String, (usize, Ty)>) -> Result<Forme, String> {
        let it = &self.espace.items[n];
        let (fichier, nom) = (it.fichier, it.nom.clone());
        let serde: HashMap<String, Option<String>> = it.serde.iter().cloned().collect();
        let corps = it.corps.clone();
        if it.genre == "type" {
            let id = self.du_type(fichier, &lire_ty(&corps), subst)?;
            return Ok(self.arene[id].clone());
        }
        if self.espace.a_la_main.contains(&nom) {
            return Ok(Forme::Ouverte);
        }
        if let Some(Some(into)) = serde.get("into") {
            let id = self.du_type(fichier, &lire_ty(&jetons(into)), subst)?;
            return Ok(self.arene[id].clone());
        }
        if !it.serialise {
            return Err(format!("le type {} ({}) ne derive pas Serialize", nom, self.espace.fichiers[fichier].display()));
        }
        let regle = serde.get("rename_all").cloned().flatten();
        if it.genre == "struct" {
            return match corps.first() {
                Some(J::P("{")) => self.objet(fichier, &corps[1..corps.len() - 1], regle.as_deref(), subst),
                Some(J::P("(")) => {
                    let champs: Vec<&[J]> = morceaux(&corps[1..corps.len() - 1], ",", true).into_iter().filter(|m| !m.is_empty()).collect();
                    let tys: Vec<Ty> = champs.iter().map(|c| lire_ty(sans_attributs_ni_visibilite(c).1)).collect();
                    if tys.len() == 1 {
                        let id = self.du_type(fichier, &tys[0], subst)?;
                        Ok(self.arene[id].clone())
                    } else {
                        self.du_type(fichier, &Ty::Uplet(tys), subst).map(|id| self.arene[id].clone())
                    }
                }
                _ => Ok(Forme::Feuille),
            };
        }
        // Une enumeration : ses variantes, selon qu'elle est etiquetee dehors (par defaut), dedans (`tag`) ou pas.
        let dedans = serde.get("tag").cloned().flatten();
        let sans_etiquette = serde.contains_key("untagged");
        let mut membres = Vec::new();
        for v in morceaux(&corps[1..corps.len() - 1], ",", true).into_iter().filter(|m| !m.is_empty()) {
            let (attrs, reste) = sans_attributs_ni_visibilite(v);
            if attrs.iter().any(|(c, _)| c == "skip" || c == "skip_serializing") {
                continue;
            }
            let variante = mot(reste.first()).ok_or_else(|| format!("une variante de {} ne se lit pas", nom))?.to_string();
            let dite = attrs.iter().find(|(c, _)| c == "rename").and_then(|(_, v)| v.clone()).unwrap_or_else(|| renomme(&variante, regle.as_deref()));
            let forme = match reste.get(1) {
                Some(J::P("{")) => {
                    let f = apres_le_groupe(reste, 1, false);
                    let mut o = self.objet(fichier, &reste[2..f - 1], None, subst)?;
                    if let (Some(t), Forme::Objet { champs, .. }) = (&dedans, &mut o) {
                        let feuille = self.poser(Forme::Feuille, "étiquette");
                        champs.insert(t.clone(), feuille);
                    }
                    self.poser(o, &nom)
                }
                Some(J::P("(")) => {
                    let f = apres_le_groupe(reste, 1, false);
                    let tys: Vec<Ty> = morceaux(&reste[2..f - 1], ",", true).into_iter().filter(|m| !m.is_empty()).map(lire_ty).collect();
                    let t = if tys.len() == 1 { tys[0].clone() } else { Ty::Uplet(tys) };
                    let id = self.du_type(fichier, &t, subst)?;
                    if let Some(t) = &dedans {
                        let mut o = self.arene[id].clone();
                        if let Forme::Objet { champs, .. } = &mut o {
                            let feuille = self.poser(Forme::Feuille, "étiquette");
                            champs.insert(t.clone(), feuille);
                        }
                        self.poser(o, &nom)
                    } else {
                        id
                    }
                }
                _ => match &dedans {
                    Some(t) => {
                        let feuille = self.poser(Forme::Feuille, "étiquette");
                        self.poser(Forme::Objet { champs: BTreeMap::from([(t.clone(), feuille)]), ouvert: false }, &nom)
                    }
                    None => self.poser(Forme::Feuille, &dite),
                },
            };
            let unite = !matches!(reste.get(1), Some(J::P("{" | "(")));
            let forme = if sans_etiquette || dedans.is_some() || unite {
                forme
            } else {
                self.poser(Forme::Objet { champs: BTreeMap::from([(dite, forme)]), ouvert: false }, &nom)
            };
            membres.push(forme);
        }
        Ok(Forme::Union(membres))
    }

    /// Les champs nommes `champs` (les jetons entre les accolades) : un objet.
    fn objet(&mut self, fichier: usize, champs: &[J], regle: Option<&str>, subst: &HashMap<String, (usize, Ty)>) -> Result<Forme, String> {
        let mut o = BTreeMap::new();
        let mut ouvert = false;
        for c in morceaux(champs, ",", true).into_iter().filter(|m| !m.is_empty()) {
            let (attrs, reste) = sans_attributs_ni_visibilite(c);
            if attrs.iter().any(|(c, _)| c == "skip" || c == "skip_serializing") {
                continue;
            }
            let nom = mot(reste.first()).ok_or_else(|| format!("un champ ne se lit pas : {}", texte(c)))?.trim_start_matches("r#").to_string();
            if !est(reste.get(1), ":") {
                return Err(format!("un champ ne se lit pas : {}", texte(c)));
            }
            let ty = lire_ty(&reste[2..]);
            let a_la_main = attrs.iter().any(|(c, _)| c == "serialize_with" || c == "with");
            let forme = if a_la_main { self.poser(Forme::Ouverte, "serialize_with") } else { self.du_type(fichier, &ty, subst)? };
            if attrs.iter().any(|(c, _)| c == "flatten") {
                match self.arene[forme].clone() {
                    Forme::Objet { champs, ouvert: o2 } => {
                        o.extend(champs);
                        ouvert |= o2;
                    }
                    _ => ouvert = true,
                }
                continue;
            }
            let dit = attrs.iter().find(|(c, _)| c == "rename").and_then(|(_, v)| v.clone()).unwrap_or_else(|| renomme(&nom, regle));
            o.insert(dit, forme);
        }
        Ok(Forme::Objet { champs: o, ouvert })
    }

    /// Les champs de la forme `id` (de chaque membre d'une union), pour un message.
    pub fn champs(&self, id: usize) -> Vec<String> {
        match &self.arene[id] {
            Forme::Objet { champs, .. } => champs.keys().cloned().collect(),
            Forme::Union(v) => {
                let mut s: BTreeSet<String> = BTreeSet::new();
                for &m in v {
                    s.extend(self.champs(m));
                }
                s.into_iter().collect()
            }
            _ => Vec::new(),
        }
    }
}

/// Les attributs `serde` d'un champ ou d'une variante, et ce qui suit ses attributs et sa visibilite.
fn sans_attributs_ni_visibilite(c: &[J]) -> (Vec<(String, Option<String>)>, &[J]) {
    let mut attrs = Vec::new();
    let mut k = 0;
    loop {
        if est(c.get(k), "#") && est(c.get(k + 1), "[") {
            let fin = apres_le_groupe(c, k + 1, false);
            attrs.extend(attributs_serde(&c[k + 2..fin - 1]));
            k = fin;
        } else if mot(c.get(k)) == Some("pub") {
            k += 1;
            if est(c.get(k), "(") {
                k = apres_le_groupe(c, k, false);
            }
        } else {
            break;
        }
    }
    (attrs, &c[k..])
}

/// Les bras d'aiguillage de l'espace : `Route::X => …` (et `Route::A | Route::B => …`, avec une garde `if`), la route
/// (sa variante), le fichier et les jetons du bras depuis `=>`.
pub fn bras(e: &Espace) -> Vec<(String, usize, Vec<J>)> {
    let mut out = Vec::new();
    for (n, v) in e.jetons.iter().enumerate() {
        let mut i = 0;
        while i + 2 < v.len() {
            if mot(v.get(i)) != Some("Route") || !est(v.get(i + 1), "::") || est(v.get(i.wrapping_sub(1)), "::") {
                i += 1;
                continue;
            }
            // Les variantes du motif, jusqu'a `=>` (ou rien : ce n'est pas un bras).
            let mut variantes = Vec::new();
            let mut k = i;
            let fleche = loop {
                if mot(v.get(k)) == Some("Route") && est(v.get(k + 1), "::") {
                    match mot(v.get(k + 2)) {
                        Some(x) => variantes.push(x.to_string()),
                        None => break None,
                    }
                    k += 3;
                    if est(v.get(k), "|") {
                        k += 1;
                        continue;
                    }
                }
                if mot(v.get(k)) == Some("if") {
                    let mut p = 0i32;
                    while k < v.len() {
                        match &v[k] {
                            J::P("(" | "[" | "{") => p += 1,
                            J::P(")" | "]" | "}") => p -= 1,
                            J::P("=>") if p == 0 => break,
                            _ => {}
                        }
                        k += 1;
                    }
                }
                break est(v.get(k), "=>").then_some(k);
            };
            let Some(f) = fleche else {
                i += 3;
                continue;
            };
            let debut = f + 1;
            let fin = if est(v.get(debut), "{") {
                apres_le_groupe(v, debut, false)
            } else {
                let mut p = 0i32;
                let mut k = debut;
                while k < v.len() {
                    match &v[k] {
                        J::P("(" | "[" | "{") => p += 1,
                        J::P(")" | "]" | "}") if p == 0 => break,
                        J::P(")" | "]" | "}") => p -= 1,
                        J::P(",") if p == 0 => break,
                        _ => {}
                    }
                    k += 1;
                }
                k
            };
            for x in variantes {
                out.push((x, n, v[debut..fin].to_vec()));
            }
            i = fin;
        }
    }
    out
}
/// Le fichier qui declare la fiche de chaque route (dans son `const ROUTES`, `route: Route::X` ou `fiche(Route::X, …)`) :
/// son module la sert, et seul son bras dit ce que la route rend.
pub fn declarees(e: &Espace) -> HashMap<String, usize> {
    let mut out = HashMap::new();
    for (n, v) in e.jetons.iter().enumerate() {
        for d in 0..v.len() {
            if !(mot(v.get(d)) == Some("const") && mot(v.get(d + 1)) == Some("ROUTES")) {
                continue;
            }
            let Some(egal) = (d..v.len()).find(|&k| est(v.get(k), "=")) else { continue };
            let Some(o) = (egal..v.len()).find(|&k| est(v.get(k), "[")) else { continue };
            let fin = apres_le_groupe(v, o, false);
            for k in o..fin.saturating_sub(3) {
                let tete = (mot(v.get(k)) == Some("route") && est(v.get(k + 1), ":")) || (mot(v.get(k)).is_some() && est(v.get(k + 1), "("));
                if tete && mot(v.get(k + 2)) == Some("Route") && est(v.get(k + 3), "::") {
                    if let Some(x) = mot(v.get(k + 4)) {
                        out.insert(x.to_string(), n);
                    }
                }
            }
        }
    }
    out
}
