
//! CE QUE LA PAGE LIT D'UNE REPONSE DE ROUTE, suivi dans ses sources JS : la valeur que rend `api("route", …)` (ou
//! `appel`, `appelerMoteur`, `lu` quand il nomme une route du catalogue), puis ce qu'elle devient — une variable (`const
//! r = await api(…)`, `r = …`, `.then((r) => …)`), une deconstruction (`const { a, b } = r`, `const [x, y] = await
//! Promise.all([api(…), api(…)])`), une variable de la page (`IMPREVUS = r`, `ETAT.configuration = …`), l'element d'une
//! liste (`r.l.map((x) => …)`, `for (const x of r.l)`, `(r.l || []).forEach(f)`), le parametre d'une fonction de la
//! page (`peindre(r.x)`) — et chaque champ lu sur elle (`r.a.b`, `r?.a`, `r["a"]`, `r.l[0].c`). Un champ lu qui n'est pas
//! dans la forme de la reponse (`reponses`) est une faute, nommee par sa route, son chemin et sa ligne.
//!
//! UN PARAMETRE TIENT L'UNION de ce que lui passent ses appels suivis (un geste vient d'un rappel ou d'une etape) ; un
//! appel qui lui passe ce qui ne se suit pas, ou la fonction passee comme valeur (un gestionnaire, un rappel d'une liste
//! non suivie), le rend inconnu : il ne se verifie pas. Tout se recueille d'abord (`suivre`), puis se verifie.
//! Une route ne se suit que si sa reponse est une structure (`reponses`) : les autres se nomment, a typer.
//!
//! Ce qui ne se suit pas : une valeur rangee dans un objet ou une liste de la page (`{ a: r.x }`, `l.push(r)`), rendue
//! par une fonction (`return r`, `lireReponse(lu.lectures.x)`), lue par une cle calculee (`r[cle]`), passee a une
//! fonction qui n'a pas un nom unique dans la page ; une route que la page compose (`api(route, …)`).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::reponses::{Forme, Formes};

#[derive(Clone, Debug, PartialEq)]
pub enum K {
    Id(String),
    P(&'static str),
    Chaine(String),
    Autre,
}

const PONCTUATIONS: [&str; 52] = [
    "...", "===", "!==", "**=", "<<=", ">>=", ">>>", "&&=", "||=", "??=", "=>", "==", "!=", "<=", ">=", "&&", "||", "??", "?.", "++", "--", "+=",
    "-=", "*=", "/=", "%=", "**", "<<", ">>", "(", ")", "[", "]", "{", "}", ";", ",", "<", ">", "+", "-", "*", "/", "%", "&", "|", "^", "!",
    "~", "?", ":", "=",
];
const POINT: &str = ".";
const OUVRE_VALEUR: &str = "${";
const FERME_VALEUR: &str = "}$";

/// Les mots apres lesquels une barre ouvre une expression reguliere.
const AVANT_UNE_EXPRESSION: [&str; 11] = ["return", "typeof", "case", "in", "of", "new", "delete", "void", "throw", "else", "do"];

/// Les jetons d'un source JS (et la ligne de chacun) : commentaires et expressions regulieres retires ; un gabarit
/// rend le code de chaque `${…}` entre `${` et `}$`.
pub fn jetons(t: &str) -> (Vec<K>, Vec<usize>) {
    let c: Vec<char> = t.chars().collect();
    let (mut v, mut lignes) = (Vec::new(), Vec::new());
    let mut ligne = 1;
    let mut i = 0;
    // La profondeur d'accolades de chaque `${…}` ouvert.
    let mut valeurs: Vec<i32> = Vec::new();
    let mut gabarit = false;
    let pousse = |v: &mut Vec<K>, lignes: &mut Vec<usize>, k: K, l: usize| {
        v.push(k);
        lignes.push(l);
    };
    while i < c.len() {
        let x = c[i];
        if gabarit {
            if x == '\\' {
                i += 2;
            } else if x == '`' {
                gabarit = false;
                i += 1;
            } else if x == '$' && c.get(i + 1) == Some(&'{') {
                pousse(&mut v, &mut lignes, K::P(OUVRE_VALEUR), ligne);
                valeurs.push(0);
                gabarit = false;
                i += 2;
            } else {
                if x == '\n' {
                    ligne += 1;
                }
                i += 1;
            }
            continue;
        }
        if x == '\n' {
            ligne += 1;
            i += 1;
        } else if x.is_whitespace() {
            i += 1;
        } else if x == '/' && c.get(i + 1) == Some(&'/') {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
        } else if x == '/' && c.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < c.len() && !(c[i] == '*' && c[i + 1] == '/') {
                if c[i] == '\n' {
                    ligne += 1;
                }
                i += 1;
            }
            i += 2;
        } else if x == '/'
            && match v.last() {
                None => true,
                Some(K::P(p)) => !matches!(*p, ")" | "]" | "}" | FERME_VALEUR),
                Some(K::Id(m)) => AVANT_UNE_EXPRESSION.contains(&m.as_str()),
                _ => false,
            }
        {
            let mut classe = false;
            i += 1;
            while i < c.len() && (c[i] != '/' || classe) {
                match c[i] {
                    '\\' => i += 1,
                    '[' => classe = true,
                    ']' => classe = false,
                    _ => {}
                }
                i += 1;
            }
            i += 1;
            while i < c.len() && c[i].is_alphabetic() {
                i += 1;
            }
            pousse(&mut v, &mut lignes, K::Autre, ligne);
        } else if x == '"' || x == '\'' {
            let mut s = String::new();
            i += 1;
            while i < c.len() && c[i] != x {
                if c[i] == '\\' {
                    i += 1;
                }
                s.push(c[i]);
                i += 1;
            }
            i += 1;
            pousse(&mut v, &mut lignes, K::Chaine(s), ligne);
        } else if x == '`' {
            // Le gabarit vaut une chaine ; son code se lit entre `${` et `}$`.
            pousse(&mut v, &mut lignes, K::Chaine(String::new()), ligne);
            gabarit = true;
            i += 1;
        } else if x == '{' {
            if let Some(p) = valeurs.last_mut() {
                *p += 1;
            }
            pousse(&mut v, &mut lignes, K::P("{"), ligne);
            i += 1;
        } else if x == '}' {
            match valeurs.last_mut() {
                Some(0) => {
                    valeurs.pop();
                    pousse(&mut v, &mut lignes, K::P(FERME_VALEUR), ligne);
                    gabarit = true;
                }
                Some(p) => {
                    *p -= 1;
                    pousse(&mut v, &mut lignes, K::P("}"), ligne);
                }
                None => pousse(&mut v, &mut lignes, K::P("}"), ligne),
            }
            i += 1;
        } else if x.is_alphabetic() || x == '_' || x == '$' {
            let debut = i;
            while i < c.len() && (c[i].is_alphanumeric() || c[i] == '_' || c[i] == '$') {
                i += 1;
            }
            pousse(&mut v, &mut lignes, K::Id(c[debut..i].iter().collect()), ligne);
        } else if x.is_ascii_digit() || (x == '.' && c.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            while i < c.len() && (c[i].is_alphanumeric() || c[i] == '.' || c[i] == '_') {
                i += 1;
            }
            pousse(&mut v, &mut lignes, K::Autre, ligne);
        } else if x == '.' && !(c.get(i + 1) == Some(&'.') && c.get(i + 2) == Some(&'.')) {
            pousse(&mut v, &mut lignes, K::P(POINT), ligne);
            i += 1;
        } else {
            let p = PONCTUATIONS.iter().find(|p| c[i..].iter().take(p.len()).copied().eq(p.chars())).copied();
            match p {
                Some(p) => {
                    // `a?.5:b` n'est pas un chainage optionnel.
                    if p == "?." && c.get(i + 2).is_some_and(|d| d.is_ascii_digit()) {
                        pousse(&mut v, &mut lignes, K::P("?"), ligne);
                        i += 1;
                    } else {
                        pousse(&mut v, &mut lignes, K::P(p), ligne);
                        i += p.len();
                    }
                }
                None => {
                    pousse(&mut v, &mut lignes, K::Autre, ligne);
                    i += 1;
                }
            }
        }
    }
    (v, lignes)
}

/// Une fonction de la page : ses parametres (les jetons de chacun) et son corps.
#[derive(Debug)]
struct Fonction {
    fichier: usize,
    params: Vec<(usize, usize)>,
    corps: (usize, usize),
}

/// Ce que la page tient d'une reponse : ses formes possibles, la route, le chemin depuis la reponse.
#[derive(Clone, Debug, PartialEq)]
struct Val {
    formes: Vec<usize>,
    route: String,
    chemin: String,
    promesse: bool,
}

/// Une faute : le fichier, la ligne, la route, le chemin, la cle lue, ce que la forme porte.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Faute {
    pub fichier: String,
    pub ligne: usize,
    pub route: String,
    pub chemin: String,
    pub cle: String,
    pub sur: String,
}

/// Les methodes d'une liste qui rappellent une fonction sur chaque element (le parametre de rang `.1`).
const RAPPELS: [(&str, usize); 10] =
    [("map", 0), ("forEach", 0), ("filter", 0), ("some", 0), ("every", 0), ("find", 0), ("findIndex", 0), ("flatMap", 0), ("findLast", 0), ("reduce", 1)];
/// Les methodes d'une liste qui en rendent une partie (la meme forme).
const PARTIES: [&str; 7] = ["filter", "slice", "sort", "concat", "reverse", "toSorted", "toReversed"];
/// Les methodes d'une liste qui en rendent un element.
const ELEMENTS: [&str; 4] = ["find", "findLast", "at", "pop"];
/// Les proprietes d'une liste, d'un texte.
const PROPRIETES_DE_LISTE: [&str; 1] = ["length"];

pub struct Page<'f, 'e> {
    formes: &'f Formes<'e>,
    /// La forme de la reponse de chaque route typee.
    routes: &'f BTreeMap<String, usize>,
    /// Le nom de chaque route du catalogue.
    catalogue: &'f BTreeSet<String>,
    fichiers: Vec<String>,
    jetons: Vec<Vec<K>>,
    lignes: Vec<Vec<usize>>,
    /// Pour chaque fichier, l'ouvrant de chaque fermant (`)` `]` `}`) et le fermant de chaque ouvrant.
    paire: Vec<HashMap<usize, usize>>,
    fonctions: Vec<Fonction>,
    fonction_en: Vec<HashMap<usize, usize>>,
    par_nom: HashMap<String, Option<usize>>,
    /// Les variables du plus haut niveau de chaque fichier (la page les partage).
    globales_declarees: BTreeSet<String>,
    globales: BTreeMap<String, Val>,
    deja: BTreeSet<String>,
    /// Une valeur qui s'evalue sans rien verifier ni suivre (`valeur_seule`) : le parcours le fera.
    muet: bool,
    /// Recueillir sans verifier (`suivre`).
    collecte: bool,
    /// Ce que tient chaque parametre de chaque fonction : l'union de ce que ses appels suivis lui passent.
    parametres: HashMap<usize, Vec<Option<Val>>>,
    /// Les parametres (fonction, rang) qu'un appel au moins remplit de ce qui ne se suit pas.
    inconnus: BTreeSet<(usize, usize)>,
    /// Ceux que la passe en cours trouve (ils remplacent `inconnus` a la fin de chaque passe qui recueille).
    inconnus_neufs: BTreeSet<(usize, usize)>,
    /// Les champs lus et trouves dans une forme fermee.
    pub verifies: usize,
    pub fautes: BTreeSet<Faute>,
    /// Les routes que la page appelle par leur nom.
    pub appelees: BTreeSet<String>,
}

fn id(v: &[K], i: usize) -> Option<&str> {
    match v.get(i) {
        Some(K::Id(m)) => Some(m),
        _ => None,
    }
}

fn p(v: &[K], i: usize, x: &str) -> bool {
    matches!(v.get(i), Some(K::P(y)) if *y == x)
}

const MOTS_CLES: [&str; 30] = [
    "if", "for", "while", "switch", "catch", "return", "typeof", "new", "await", "async", "function", "const", "let", "var", "of", "in",
    "else", "do", "try", "finally", "throw", "case", "default", "break", "continue", "delete", "void", "instanceof", "class", "this",
];

impl<'f, 'e> Page<'f, 'e> {
    pub fn lire(formes: &'f Formes<'e>, routes: &'f BTreeMap<String, usize>, catalogue: &'f BTreeSet<String>, sources: Vec<(String, String)>) -> Page<'f, 'e> {
        let mut pg = Page {
            formes,
            routes,
            catalogue,
            fichiers: Vec::new(),
            jetons: Vec::new(),
            lignes: Vec::new(),
            paire: Vec::new(),
            fonctions: Vec::new(),
            fonction_en: Vec::new(),
            par_nom: HashMap::new(),
            globales_declarees: BTreeSet::new(),
            globales: BTreeMap::new(),
            deja: BTreeSet::new(),
            muet: false,
            collecte: false,
            parametres: HashMap::new(),
            inconnus: BTreeSet::new(),
            inconnus_neufs: BTreeSet::new(),
            verifies: 0,
            fautes: BTreeSet::new(),
            appelees: BTreeSet::new(),
        };
        for (n, (nom, texte)) in sources.into_iter().enumerate() {
            let (v, l) = jetons(&texte);
            let mut paire = HashMap::new();
            let mut pile = Vec::new();
            for (k, j) in v.iter().enumerate() {
                match j {
                    K::P("(" | "[" | "{" | OUVRE_VALEUR) => pile.push(k),
                    K::P(")" | "]" | "}" | FERME_VALEUR) => {
                        if let Some(o) = pile.pop() {
                            paire.insert(o, k);
                            paire.insert(k, o);
                        }
                    }
                    _ => {}
                }
            }
            pg.fichiers.push(nom);
            pg.jetons.push(v);
            pg.lignes.push(l);
            pg.paire.push(paire);
            pg.lire_fonctions(n);
        }
        pg
    }

    fn lire_fonctions(&mut self, n: usize) {
        let v = &self.jetons[n];
        let paire = &self.paire[n];
        let mut en = HashMap::new();
        let mut profondeur = 0i32;
        for i in 0..v.len() {
            match &v[i] {
                K::P("{" | "(" | "[" | OUVRE_VALEUR) => profondeur += 1,
                K::P("}" | ")" | "]" | FERME_VALEUR) => profondeur -= 1,
                _ => {}
            }
            // Une variable du plus haut niveau : `let X`, `const X`, `var X`.
            if profondeur == 0 && matches!(id(v, i), Some("let" | "const" | "var")) {
                if let Some(x) = id(v, i + 1) {
                    self.globales_declarees.insert(x.to_string());
                }
            }
            let (params, apres): (Vec<(usize, usize)>, usize) = if id(v, i) == Some("function") {
                let mut k = i + 1;
                if p(v, k, "*") {
                    k += 1;
                }
                if id(v, k).is_some() {
                    k += 1;
                }
                if !p(v, k, "(") {
                    continue;
                }
                let f = paire[&k];
                (decouper(v, k + 1, f), f + 1)
            } else if p(v, i, "(") && paire.get(&i).is_some_and(|&f| p(v, f + 1, "=>")) {
                let f = paire[&i];
                (decouper(v, i + 1, f), f + 2)
            } else if id(v, i).is_some() && p(v, i + 1, "=>") && !p(v, i.wrapping_sub(1), POINT) {
                (vec![(i, i + 1)], i + 2)
            } else {
                continue;
            };
            let corps = if p(v, apres, "{") {
                (apres + 1, paire.get(&apres).copied().unwrap_or(v.len()))
            } else {
                // Le corps d'une fleche sans accolades : jusqu'a la virgule, au fermant ou au point-virgule de son niveau.
                let mut k = apres;
                let mut q = 0i32;
                while k < v.len() {
                    match &v[k] {
                        K::P("(" | "[" | "{" | OUVRE_VALEUR) => q += 1,
                        K::P(")" | "]" | "}" | FERME_VALEUR) if q == 0 => break,
                        K::P(")" | "]" | "}" | FERME_VALEUR) => q -= 1,
                        K::P("," | ";") if q == 0 => break,
                        _ => {}
                    }
                    k += 1;
                }
                (apres, k)
            };
            let nom = if id(v, i) == Some("function") {
                id(v, i + 1).map(String::from)
            } else {
                // `const nom = (…) =>`, `const nom = async (…) =>`.
                let mut k = i.wrapping_sub(1);
                if id(v, k) == Some("async") {
                    k = k.wrapping_sub(1);
                }
                (p(v, k, "=") && matches!(id(v, k.wrapping_sub(2)), Some("const" | "let" | "var"))).then(|| id(v, k.wrapping_sub(1)).map(String::from)).flatten()
            };
            let f = Fonction { fichier: n, params, corps };
            en.insert(i, self.fonctions.len());
            if let Some(x) = nom {
                let deja = self.par_nom.contains_key(&x);
                self.par_nom.insert(x, if deja { None } else { Some(self.fonctions.len()) });
            }
            self.fonctions.push(f);
        }
        self.fonction_en.push(en);
    }

    /// Tout suivre : chaque fichier depuis son plus haut niveau, jusqu'a ce que les variables de la page ne changent plus.
    pub fn suivre(&mut self) {
        // Recueillir : les variables de la page et les parametres des fonctions, jusqu'a ce qu'ils ne changent plus.
        self.collecte = true;
        for _ in 0..8 {
            let avant = (self.globales.clone(), format!("{:?}", self.parametres_tries()), self.inconnus.clone());
            self.passe();
            if (self.globales.clone(), format!("{:?}", self.parametres_tries()), self.inconnus.clone()) == avant {
                break;
            }
        }
        // Verifier, sur ce qui a ete recueilli.
        self.collecte = false;
        self.passe();
    }

    fn parametres_tries(&self) -> BTreeMap<usize, Vec<Option<(Vec<usize>, String)>>> {
        self.parametres.iter().map(|(k, v)| (*k, v.iter().map(|x| x.as_ref().map(|x| (x.formes.clone(), x.route.clone()))).collect())).collect()
    }

    fn passe(&mut self) {
        self.deja.clear();
        self.inconnus_neufs.clear();
        for n in 0..self.fichiers.len() {
            let fin = self.jetons[n].len();
            self.parcourir(n, 0, fin, &mut HashMap::new());
        }
        if self.collecte {
            self.inconnus = std::mem::take(&mut self.inconnus_neufs);
        }
    }

    fn faute(&mut self, n: usize, k: usize, val: &Val, cle: &str, sur: String) {
        if self.muet || self.collecte {
            return;
        }
        self.fautes.insert(Faute {
            fichier: self.fichiers[n].clone(),
            ligne: self.lignes[n][k],
            route: val.route.clone(),
            chemin: val.chemin.clone(),
            cle: cle.to_string(),
            sur,
        });
    }

    /// Les formes sans leurs unions.
    fn a_plat(&self, formes: &[usize]) -> Vec<usize> {
        let mut out = Vec::new();
        let mut pile: Vec<usize> = formes.to_vec();
        let mut vu = BTreeSet::new();
        while let Some(f) = pile.pop() {
            if !vu.insert(f) {
                continue;
            }
            match &self.formes.arene[f] {
                Forme::Union(m) => pile.extend(m),
                _ => out.push(f),
            }
        }
        out
    }

    /// La valeur de la propriete `cle` de `val`, ou `None` (ouverte, une feuille) ; une faute si `val` est un objet ferme
    /// sans cette cle.
    fn propriete(&mut self, n: usize, k: usize, val: &Val, cle: &str) -> Option<Val> {
        let plat = self.a_plat(&val.formes);
        let mut trouve = Vec::new();
        let mut ouverte = false;
        let mut objets = Vec::new();
        let mut liste = false;
        for &f in &plat {
            match &self.formes.arene[f] {
                Forme::Objet { champs, ouvert } => {
                    if let Some(&c) = champs.get(cle) {
                        trouve.push(c);
                    }
                    ouverte |= *ouvert;
                    objets.push(f);
                }
                Forme::Table(c) => trouve.push(*c),
                Forme::Ouverte => ouverte = true,
                Forme::Liste(_) => liste = true,
                Forme::Feuille | Forme::Union(_) => {}
            }
        }
        if !trouve.is_empty() {
            if !self.muet && !self.collecte && !ouverte {
                self.verifies += 1;
            }
            let chemin = if val.chemin.is_empty() { cle.to_string() } else { format!("{}.{}", val.chemin, cle) };
            return Some(Val { formes: trouve, route: val.route.clone(), chemin, promesse: false });
        }
        if ouverte || PROPRIETES_DE_LISTE.contains(&cle) {
            return None;
        }
        if !objets.is_empty() {
            let mut champs: BTreeSet<String> = BTreeSet::new();
            for f in objets {
                champs.extend(self.formes.champs(f));
            }
            self.faute(n, k, val, cle, format!("elle a : {}", champs.into_iter().collect::<Vec<_>>().join(", ")));
        } else if liste {
            self.faute(n, k, val, cle, "c'est une liste".into());
        }
        None
    }

    fn element(&self, val: &Val) -> Option<Val> {
        let e: Vec<usize> = self
            .a_plat(&val.formes)
            .into_iter()
            .filter_map(|f| match &self.formes.arene[f] {
                Forme::Liste(e) | Forme::Table(e) => Some(*e),
                _ => None,
            })
            .collect();
        (!e.is_empty()).then(|| Val { formes: e, route: val.route.clone(), chemin: format!("{}[]", val.chemin), promesse: false })
    }

    fn est_liste(&self, val: &Val) -> bool {
        self.a_plat(&val.formes).iter().any(|&f| matches!(self.formes.arene[f], Forme::Liste(_)))
    }

    /// La valeur qu'une variable tient a cet endroit : locale, sinon de la page.
    fn tenue(&self, env: &HashMap<String, Val>, x: &str) -> Option<Val> {
        env.get(x).cloned().or_else(|| self.globales.get(x).cloned())
    }

    /// La reponse d'un appel `api("route", …)` qui commence en `i` (le nom de la fonction), et le fermant de l'appel.
    fn appel(&mut self, n: usize, i: usize) -> Option<(Option<Val>, usize)> {
        let v = &self.jetons[n];
        if !matches!(id(v, i), Some("api" | "appel" | "appelerMoteur" | "lu")) || !p(v, i + 1, "(") || p(v, i.wrapping_sub(1), POINT) || id(v, i.wrapping_sub(1)) == Some("function") {
            return None;
        }
        let fin = self.paire[n][&(i + 1)];
        let route = match v.get(i + 2) {
            Some(K::Chaine(r)) if self.catalogue.contains(r) => r.clone(),
            // `lu("nir")` : une fonction de la page qui ne lit pas une route.
            _ if id(v, i) == Some("lu") => return None,
            _ => return Some((None, fin)),
        };
        let route = route.clone();
        self.appelees.insert(route.clone());
        let promesse = id(v, i.wrapping_sub(1)) != Some("await");
        let val = self.routes.get(&route).map(|&f| Val { formes: vec![f], route, chemin: String::new(), promesse });
        Some((val, fin))
    }

    /// La chaine qui commence en `i` sur `val` (`.a`, `?.a`, `[0]`, `.map(…)`) : les champs lus verifies, les rappels
    /// suivis ; rend ou elle s'arrete et ce qu'elle vaut alors.
    fn chaine(&mut self, n: usize, mut i: usize, mut val: Val, env: &mut HashMap<String, Val>) -> (usize, Option<Val>) {
        loop {
            let v = &self.jetons[n];
            if (p(v, i, POINT) || p(v, i, "?.")) && id(v, i + 1).is_some() {
                let cle = id(v, i + 1).unwrap().to_string();
                if p(v, i + 2, "(") {
                    let fin = self.paire[n][&(i + 2)];
                    if val.promesse {
                        if cle == "then" {
                            let mut porte = val.clone();
                            porte.promesse = false;
                            self.rappel(n, i + 3, fin, 0, Some(porte), env);
                        } else {
                            self.parcourir(n, i + 3, fin, env);
                        }
                        val.promesse = cle != "then";
                        if cle == "then" {
                            return (fin + 1, None);
                        }
                        i = fin + 1;
                        continue;
                    }
                    if !self.est_liste(&val) {
                        // Une methode d'un texte, d'un nombre, d'une table : la chaine s'arrete la.
                        return (i, None);
                    }
                    let elem = self.element(&val);
                    match RAPPELS.iter().find(|(m, _)| *m == cle) {
                        Some(&(_, rang)) => self.rappel(n, i + 3, fin, rang, elem.clone(), env),
                        None => self.parcourir(n, i + 3, fin, env),
                    }
                    let suite = if PARTIES.contains(&cle.as_str()) {
                        Some(val.clone())
                    } else if ELEMENTS.contains(&cle.as_str()) {
                        elem
                    } else {
                        None
                    };
                    match suite {
                        Some(s) => {
                            val = s;
                            i = fin + 1;
                        }
                        None => return (fin + 1, None),
                    }
                    continue;
                }
                match self.propriete(n, i + 1, &val, &cle) {
                    Some(s) => {
                        val = s;
                        i += 2;
                    }
                    None => return (i + 2, None),
                }
                continue;
            }
            if p(v, i, "[") || (p(v, i, "?.") && p(v, i + 1, "[")) {
                let o = if p(v, i, "[") { i } else { i + 1 };
                let fin = self.paire[n][&o];
                if fin == o + 2 {
                    if let Some(K::Chaine(cle)) = v.get(o + 1) {
                        let cle = cle.clone();
                        match self.propriete(n, o + 1, &val, &cle) {
                            Some(s) => {
                                val = s;
                                i = fin + 1;
                                continue;
                            }
                            None => return (fin + 1, None),
                        }
                    }
                }
                self.parcourir(n, o + 1, fin, env);
                match self.element(&val) {
                    Some(e) => {
                        val = e;
                        i = fin + 1;
                        continue;
                    }
                    None => return (fin + 1, None),
                }
            }
            return (i, Some(val));
        }
    }

    /// Le rappel passe en argument (de rang 0 dans `debut..fin`) : une fonction ecrite la, ou une fonction de la page
    /// nommee ; son parametre de rang `rang` tient `elem`. Les autres arguments se parcourent.
    fn rappel(&mut self, n: usize, debut: usize, fin: usize, rang: usize, elem: Option<Val>, env: &mut HashMap<String, Val>) {
        if self.muet {
            return;
        }
        let args = decouper(&self.jetons[n], debut, fin);
        for (k, &(a, b)) in args.iter().enumerate() {
            let a = if id(&self.jetons[n], a) == Some("async") { a + 1 } else { a };
            if k == 0 {
                if let Some(&f) = self.fonction_en[n].get(&a) {
                    let mut vals = vec![None; rang + 1];
                    vals[rang] = elem.clone();
                    self.dans_la_fonction(f, &vals, env.clone());
                    continue;
                }
                if b == a + 1 {
                    if let Some(f) = id(&self.jetons[n], a).and_then(|x| self.par_nom.get(x).copied().flatten()) {
                        let mut vals = vec![None; rang + 1];
                        vals[rang] = elem.clone();
                        self.dans_la_fonction(f, &vals, HashMap::new());
                        continue;
                    }
                }
            }
            self.parcourir(n, a, b, env);
        }
    }

    /// Le corps de la fonction `f`, ses parametres tenant `vals`, dans `env` (moins ce que ses parametres cachent).
    fn dans_la_fonction(&mut self, f: usize, vals: &[Option<Val>], mut env: HashMap<String, Val>) {
        if self.muet {
            return;
        }
        // UN PARAMETRE TIENT TOUT CE QU'ON LUI PASSE : l'union des valeurs de chaque appel suivi (une fonction qui
        // dessine une pastille, un geste, quelle que soit la route d'ou il vient). Elle se recueille avant la
        // verification (`suivre`).
        let union = self.parametres.entry(f).or_default();
        for (k, v) in vals.iter().enumerate() {
            let Some(v) = v else { continue };
            if union.len() <= k {
                union.resize(k + 1, None);
            }
            match &mut union[k] {
                Some(u) => {
                    for x in &v.formes {
                        if !u.formes.contains(x) {
                            u.formes.push(*x);
                        }
                    }
                    u.route = reunir(&u.route, &v.route);
                    u.chemin = reunir(&u.chemin, &v.chemin);
                }
                vide => *vide = Some(Val { promesse: false, ..v.clone() }),
            }
        }
        let vals = union.clone();
        let cle = format!("{}|{:?}|{:?}", f, vals, {
            let mut e: Vec<_> = env.iter().map(|(k, v)| format!("{}={}{:?}", k, v.route, v.formes)).collect();
            e.sort();
            e
        });
        if !self.deja.insert(cle) {
            return;
        }
        let (n, params, corps) = {
            let x = &self.fonctions[f];
            (x.fichier, x.params.clone(), x.corps)
        };
        for (k, &(a, b)) in params.iter().enumerate() {
            let v = &self.jetons[n];
            let tenu = if self.inconnus.contains(&(f, k)) { None } else { vals.get(k).cloned().flatten() };
            match (id(v, a), tenu) {
                (Some(x), Some(val)) if b == a + 1 || p(v, a + 1, "=") => {
                    env.insert(x.to_string(), val);
                }
                (_, Some(val)) if p(v, a, "{") => {
                    self.deconstruire(n, a, &val, &mut env);
                }
                _ => {
                    for j in a..b {
                        if let Some(x) = id(&self.jetons[n], j) {
                            env.remove(x);
                        }
                    }
                }
            }
        }
        self.parcourir(n, corps.0, corps.1, &mut env);
    }

    /// `{ a, b: c, d = 1 }` (qui ouvre en `o`) sur `val` : chaque cle lue, chaque nom lie.
    fn deconstruire(&mut self, n: usize, o: usize, val: &Val, env: &mut HashMap<String, Val>) {
        let fin = self.paire[n][&o];
        for (a, _) in decouper(&self.jetons[n], o + 1, fin) {
            let v = &self.jetons[n];
            if p(v, a, "...") {
                continue;
            }
            let Some(cle) = id(v, a).map(String::from).or_else(|| match v.get(a) {
                Some(K::Chaine(s)) => Some(s.clone()),
                _ => None,
            }) else {
                continue;
            };
            let sous = self.propriete(n, a, val, &cle);
            let v = &self.jetons[n];
            let lie = if p(v, a + 1, ":") { a + 2 } else { a };
            if p(v, lie, "{") {
                if let Some(s) = sous {
                    self.deconstruire(n, lie, &s, env);
                }
            } else if let Some(x) = id(v, lie) {
                match sous {
                    Some(s) => env.insert(x.to_string(), s),
                    None => env.remove(x),
                };
            }
        }
    }

    /// Ce que vaut l'expression qui commence en `i` (et ou elle finit) : une reponse, une variable qui en tient une, une
    /// chaine sur elles, `(… || [])`, `await Promise.all([…])` (une liste de valeurs). Rien n'y est verifie : le parcours
    /// le fait.
    fn valeur(&mut self, n: usize, mut i: usize, env: &HashMap<String, Val>) -> Option<Vec<Option<Val>>> {
        let v = &self.jetons[n];
        while id(v, i) == Some("await") {
            i += 1;
        }
        if id(v, i) == Some("Promise") && p(v, i + 1, POINT) && id(v, i + 2) == Some("all") && p(v, i + 3, "(") && p(v, i + 4, "[") {
            let fin = self.paire[n][&(i + 4)];
            let args = decouper(v, i + 5, fin);
            let mut out = Vec::new();
            for (a, b) in args {
                out.push(self.valeur_seule(n, a, b, env, true));
            }
            return Some(out);
        }
        None
    }

    /// La valeur de l'expression `a..b`, si elle est toute une chaine sur une reponse ; `attendue` : sa promesse vaut sa
    /// valeur (`Promise.all`).
    fn valeur_seule(&mut self, n: usize, mut a: usize, mut b: usize, env: &HashMap<String, Val>, attendue: bool) -> Option<Val> {
        let v = &self.jetons[n];
        while id(v, a) == Some("await") {
            a += 1;
        }
        // `x || []`, `x ?? {}` : la valeur est `x`.
        if b >= a + 3 && matches!(v.get(b - 3), Some(K::P("||" | "??"))) && (p(v, b - 2, "[") && p(v, b - 1, "]") || p(v, b - 2, "{") && p(v, b - 1, "}")) {
            b -= 3;
        }
        while p(v, a, "(") && self.paire[n].get(&a) == Some(&(b - 1)) {
            a += 1;
            b -= 1;
            while id(&self.jetons[n], a) == Some("await") {
                a += 1;
            }
            let v = &self.jetons[n];
            if b >= a + 3 && matches!(v.get(b - 3), Some(K::P("||" | "??"))) && (p(v, b - 2, "[") && p(v, b - 1, "]") || p(v, b - 2, "{") && p(v, b - 1, "}")) {
                b -= 3;
            }
        }
        let mut copie = env.clone();
        let avant = std::mem::replace(&mut self.muet, true);
        let r = self.expression(n, a, &mut copie);
        self.muet = avant;
        let (fin, val) = r?;
        let mut val = val?;
        if attendue {
            val.promesse = false;
        }
        (fin == b).then_some(val)
    }

    /// Une expression qui commence en `i` : une reponse (`api(…)`), une variable qui en tient une, `(await api(…))`, et la
    /// chaine qui suit.
    fn expression(&mut self, n: usize, i: usize, env: &mut HashMap<String, Val>) -> Option<(usize, Option<Val>)> {
        if let Some((val, fin)) = self.appel(n, i) {
            self.parcourir(n, i + 2, fin, env);
            return Some(match val {
                Some(val) => self.chaine(n, fin + 1, val, env),
                None => (fin + 1, None),
            });
        }
        let v = &self.jetons[n];
        // `(await api(…)).x`, `(r.l || []).map(…)` : la parenthese vaut ce qu'elle tient.
        if p(v, i, "(") {
            let fin = self.paire[n][&i];
            let suite = p(v, fin + 1, POINT) || p(v, fin + 1, "?.") || p(v, fin + 1, "[");
            if suite {
                if let Some(val) = self.valeur_seule(n, i, fin + 1, env, true) {
                    self.parcourir(n, i + 1, fin, env);
                    return Some(self.chaine(n, fin + 1, val, env));
                }
            }
            return None;
        }
        let x = id(v, i)?;
        if p(v, i.wrapping_sub(1), POINT) || p(v, i.wrapping_sub(1), "?.") {
            return None;
        }
        if let Some(val) = self.tenue(env, x) {
            return Some(self.chaine(n, i + 1, val, env));
        }
        // `ETAT.configuration` : un champ d'une variable de la page qui tient une reponse.
        if p(v, i + 1, POINT) {
            if let Some(y) = id(v, i + 2) {
                if let Some(val) = self.globales.get(&format!("{}.{}", x, y)).cloned() {
                    return Some(self.chaine(n, i + 3, val, env));
                }
            }
        }
        None
    }

    /// Parcourt `debut..fin` avec `env` : les liaisons, les chaines, les fonctions nichees, les appels de fonctions de
    /// la page.
    fn parcourir(&mut self, n: usize, debut: usize, fin: usize, env: &mut HashMap<String, Val>) {
        if self.muet {
            return;
        }
        let mut i = debut;
        while i < fin {
            let v = &self.jetons[n];
            // Une fonction nichee : son corps, ce que ses parametres cachent retire.
            if let Some(&f) = self.fonction_en[n].get(&i) {
                let corps = self.fonctions[f].corps;
                self.dans_la_fonction(f, &[], env.clone());
                i = if p(&self.jetons[n], corps.1, "}") { corps.1 + 1 } else { corps.1 };
                continue;
            }
            // `for (const x of …)` : `x` tient un element.
            if id(v, i) == Some("for") && p(v, i + 1, "(") && matches!(id(v, i + 2), Some("const" | "let" | "var")) && id(v, i + 4) == Some("of") {
                let x = id(v, i + 3).unwrap_or_default().to_string();
                let ferme = self.paire[n][&(i + 1)];
                let elem = self.valeur_seule(n, i + 5, ferme, env, false).and_then(|l| self.element(&l));
                match elem {
                    Some(e) => env.insert(x, e),
                    None => env.remove(&x),
                };
                i += 5;
                continue;
            }
            // Une liaison : `const x = …`, `x = …`, `X.y = …`, `const { a } = …`, `const [a, b] = …`.
            if let Some((cible, egal)) = self.liaison(n, i) {
                let rhs_fin = fin_d_expression(&self.jetons[n], egal + 1, fin);
                let tout = self.valeur(n, egal + 1, env);
                let seule = if tout.is_none() { self.valeur_seule(n, egal + 1, rhs_fin, env, false) } else { None };
                let v = &self.jetons[n];
                let neutre = rhs_fin == egal + 2 && matches!(id(v, egal + 1), Some("null" | "undefined" | "false"))
                    || rhs_fin == egal + 3 && (p(v, egal + 1, "[") || p(v, egal + 1, "{"));
                match cible {
                    Cible::Nom(x) => match seule {
                        Some(val) => {
                            if !env.contains_key(&x) && self.globales_declarees.contains(&x) && !self.est_locale(n, i, &x) {
                                self.globales.insert(x, val);
                            } else {
                                env.insert(x, val);
                            }
                        }
                        None if !neutre => {
                            env.remove(&x);
                        }
                        None => {}
                    },
                    Cible::Champ(x) => {
                        if let Some(val) = seule {
                            self.globales.insert(x, val);
                        }
                    }
                    Cible::Objet(o) => {
                        if let Some(val) = seule {
                            self.deconstruire(n, o, &val, env);
                        }
                    }
                    Cible::Liste(o) => {
                        let ferme = self.paire[n][&o];
                        let noms = decouper(&self.jetons[n], o + 1, ferme);
                        let vals = tout.unwrap_or_default();
                        for (k, (a, _)) in noms.into_iter().enumerate() {
                            if let Some(x) = id(&self.jetons[n], a).map(String::from) {
                                match vals.get(k).cloned().flatten() {
                                    Some(val) => env.insert(x, val),
                                    None => env.remove(&x),
                                };
                            }
                        }
                    }
                }
                i = egal + 1;
                continue;
            }
            if let Some((suite, _)) = self.expression(n, i, env) {
                i = suite.max(i + 1);
                continue;
            }
            // Un appel d'une fonction de la page : ses parametres tiennent ce que tiennent ses arguments.
            let v = &self.jetons[n];
            if let (Some(x), true) = (id(v, i), p(v, i + 1, "(")) {
                if !p(v, i.wrapping_sub(1), POINT) && !p(v, i.wrapping_sub(1), "?.") && !MOTS_CLES.contains(&x) && !env.contains_key(x) {
                    if let Some(f) = self.par_nom.get(x).copied().flatten() {
                        let ferme = self.paire[n][&(i + 1)];
                        let args = decouper(&self.jetons[n], i + 2, ferme);
                        let vals: Vec<Option<Val>> = args.iter().map(|&(a, b)| self.valeur_seule(n, a, b, env, false)).collect();
                        // Un argument qui ne se suit pas : son parametre peut tenir autre chose, il ne se verifie pas.
                        for (k, v) in vals.iter().enumerate() {
                            if v.is_none() {
                                self.inconnus_neufs.insert((f, k));
                            }
                        }
                        if vals.iter().any(Option::is_some) {
                            self.dans_la_fonction(f, &vals, HashMap::new());
                        }
                    }
                }
            } else if let Some(x) = id(v, i) {
                // Une fonction de la page passee comme valeur (un rappel qu'on ne suit pas, un gestionnaire) : ses
                // parametres peuvent tenir n'importe quoi.
                if !p(v, i.wrapping_sub(1), POINT) && !p(v, i.wrapping_sub(1), "?.") && id(v, i.wrapping_sub(1)) != Some("function") && !env.contains_key(x) {
                    if let Some(f) = self.par_nom.get(x).copied().flatten() {
                        let n_params = self.fonctions[f].params.len();
                        for k in 0..n_params {
                            self.inconnus_neufs.insert((f, k));
                        }
                    }
                }
            }
            i += 1;
        }
    }

    /// Le nom `x` est-il declare dans la fonction qui contient `i` (et non au plus haut niveau) ?
    fn est_locale(&self, n: usize, i: usize, x: &str) -> bool {
        let v = &self.jetons[n];
        self.fonctions.iter().filter(|f| f.fichier == n && f.corps.0 <= i && i <= f.corps.1).any(|f| {
            (f.corps.0..f.corps.1).any(|k| matches!(id(v, k), Some("let" | "const" | "var")) && id(v, k + 1) == Some(x))
                || f.params.iter().any(|&(a, b)| (a..b).any(|k| id(v, k) == Some(x)))
        })
    }

    /// Une liaison qui commence en `i` : sa cible, et son `=`.
    fn liaison(&self, n: usize, i: usize) -> Option<(Cible, usize)> {
        let v = &self.jetons[n];
        let decl = matches!(id(v, i), Some("const" | "let" | "var"));
        let k = if decl { i + 1 } else { i };
        if !decl && (p(v, i.wrapping_sub(1), POINT) || p(v, i.wrapping_sub(1), "?.")) {
            return None;
        }
        if decl && (p(v, k, "{") || p(v, k, "[")) {
            let f = self.paire[n][&k];
            return p(v, f + 1, "=").then(|| (if p(v, k, "{") { Cible::Objet(k) } else { Cible::Liste(k) }, f + 1));
        }
        let x = id(v, k).filter(|x| !MOTS_CLES.contains(x))?;
        if p(v, k + 1, "=") {
            return Some((Cible::Nom(x.to_string()), k + 1));
        }
        if !decl && p(v, k + 1, POINT) && id(v, k + 2).is_some() && p(v, k + 3, "=") && self.globales_declarees.contains(x) {
            return Some((Cible::Champ(format!("{}.{}", x, id(v, k + 2).unwrap())), k + 3));
        }
        None
    }
}

enum Cible {
    Nom(String),
    Champ(String),
    Objet(usize),
    Liste(usize),
}

/// Les morceaux de `debut..fin` separes par une virgule de leur niveau.
fn decouper(v: &[K], debut: usize, fin: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let (mut q, mut a) = (0i32, debut);
    for k in debut..fin {
        match &v[k] {
            K::P("(" | "[" | "{" | OUVRE_VALEUR) => q += 1,
            K::P(")" | "]" | "}" | FERME_VALEUR) => q -= 1,
            K::P(",") if q == 0 => {
                out.push((a, k));
                a = k + 1;
            }
            _ => {}
        }
    }
    if a < fin {
        out.push((a, fin));
    }
    out
}

/// La fin de l'expression qui commence en `i` (avant `fin`) : le point-virgule, la virgule ou le fermant de son niveau.
fn fin_d_expression(v: &[K], i: usize, fin: usize) -> usize {
    let mut q = 0i32;
    let mut k = i;
    while k < fin {
        match &v[k] {
            K::P("(" | "[" | "{" | OUVRE_VALEUR) => q += 1,
            K::P(")" | "]" | "}" | FERME_VALEUR) if q == 0 => return k,
            K::P(")" | "]" | "}" | FERME_VALEUR) => q -= 1,
            K::P(";" | ",") if q == 0 => return k,
            _ => {}
        }
        k += 1;
    }
    fin
}

/// Deux noms (routes, chemins) reunis, « a | b », chacun une fois, dans l'ordre.
fn reunir(a: &str, b: &str) -> String {
    let mut v: Vec<&str> = a.split(" | ").collect();
    for x in b.split(" | ") {
        if !v.contains(&x) {
            v.push(x);
        }
    }
    v.sort();
    v.join(" | ")
}
