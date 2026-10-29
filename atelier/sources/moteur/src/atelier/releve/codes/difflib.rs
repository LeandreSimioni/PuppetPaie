//! Le diff unifie de deux listes de lignes : la comparaison d'un article a sa version d'avant (`comparer`), en-tetes
//! `avant` et `apres`, une ligne de contexte. L'algorithme est celui de Ratcliff et Obershelp (le plus long bloc commun,
//! puis recursivement a gauche et a droite ; une ligne trop frequente d'une longue liste ne sert pas d'ancre), et ses
//! regroupements en blocs `@@`.

use std::collections::{HashMap, HashSet};

/// (debut dans a, debut dans b, longueur).
type Bloc = (usize, usize, usize);
/// (etiquette, i1, i2, j1, j2).
type Op = (&'static str, usize, usize, usize, usize);

struct Comparateur<'a> {
    a: &'a [String],
    b: &'a [String],
    b2j: HashMap<&'a str, Vec<usize>>,
}

impl<'a> Comparateur<'a> {
    fn nouveau(a: &'a [String], b: &'a [String]) -> Self {
        let mut b2j: HashMap<&str, Vec<usize>> = HashMap::new();
        for (i, x) in b.iter().enumerate() {
            b2j.entry(x.as_str()).or_default().push(i);
        }
        // `autojunk` : dans une sequence d'au moins 200 elements, ce qui revient plus d'une fois sur cent (et une) est
        // « populaire » et ne sert pas d'ancre.
        let n = b.len();
        if n >= 200 {
            let ntest = n / 100 + 1;
            let populaires: HashSet<&str> = b2j.iter().filter(|(_, v)| v.len() > ntest).map(|(k, _)| *k).collect();
            for p in populaires {
                b2j.remove(p);
            }
        }
        Comparateur { a, b, b2j }
    }

    /// `find_longest_match` (sans element rebut : `isjunk` est `None`).
    fn plus_long(&self, alo: usize, ahi: usize, blo: usize, bhi: usize) -> Bloc {
        let (mut besti, mut bestj, mut bestsize) = (alo, blo, 0usize);
        let mut j2len: HashMap<usize, usize> = HashMap::new();
        for i in alo..ahi {
            let mut nouveau: HashMap<usize, usize> = HashMap::new();
            if let Some(js) = self.b2j.get(self.a[i].as_str()) {
                for &j in js {
                    if j < blo {
                        continue;
                    }
                    if j >= bhi {
                        break;
                    }
                    let k = j.checked_sub(1).and_then(|p| j2len.get(&p)).copied().unwrap_or(0) + 1;
                    nouveau.insert(j, k);
                    if k > bestsize {
                        besti = i + 1 - k;
                        bestj = j + 1 - k;
                        bestsize = k;
                    }
                }
            }
            j2len = nouveau;
        }
        // L'extension par les elements egaux (les populaires compris : ils ne sont pas du rebut).
        while besti > alo && bestj > blo && self.a[besti - 1] == self.b[bestj - 1] {
            besti -= 1;
            bestj -= 1;
            bestsize += 1;
        }
        while besti + bestsize < ahi && bestj + bestsize < bhi && self.a[besti + bestsize] == self.b[bestj + bestsize] {
            bestsize += 1;
        }
        (besti, bestj, bestsize)
    }

    /// `get_matching_blocks`.
    fn blocs(&self) -> Vec<Bloc> {
        let (la, lb) = (self.a.len(), self.b.len());
        let mut file = vec![(0, la, 0, lb)];
        let mut blocs = Vec::new();
        while let Some((alo, ahi, blo, bhi)) = file.pop() {
            let (i, j, k) = self.plus_long(alo, ahi, blo, bhi);
            if k > 0 {
                blocs.push((i, j, k));
                if alo < i && blo < j {
                    file.push((alo, i, blo, j));
                }
                if i + k < ahi && j + k < bhi {
                    file.push((i + k, ahi, j + k, bhi));
                }
            }
        }
        blocs.sort();
        let (mut i1, mut j1, mut k1) = (0, 0, 0);
        let mut fusionnes = Vec::new();
        for (i2, j2, k2) in blocs {
            if i1 + k1 == i2 && j1 + k1 == j2 {
                k1 += k2;
            } else {
                if k1 > 0 {
                    fusionnes.push((i1, j1, k1));
                }
                (i1, j1, k1) = (i2, j2, k2);
            }
        }
        if k1 > 0 {
            fusionnes.push((i1, j1, k1));
        }
        fusionnes.push((la, lb, 0));
        fusionnes
    }

    /// `get_opcodes`.
    fn operations(&self) -> Vec<Op> {
        let (mut i, mut j) = (0, 0);
        let mut ops = Vec::new();
        for (ai, bj, taille) in self.blocs() {
            let etiquette = if i < ai && j < bj {
                "replace"
            } else if i < ai {
                "delete"
            } else if j < bj {
                "insert"
            } else {
                ""
            };
            if !etiquette.is_empty() {
                ops.push((etiquette, i, ai, j, bj));
            }
            i = ai + taille;
            j = bj + taille;
            if taille > 0 {
                ops.push(("equal", ai, i, bj, j));
            }
        }
        ops
    }

    /// `get_grouped_opcodes(n)`.
    fn groupes(&self, n: usize) -> Vec<Vec<Op>> {
        let mut codes = self.operations();
        if codes.is_empty() {
            codes = vec![("equal", 0, 1, 0, 1)];
        }
        if codes[0].0 == "equal" {
            let (t, i1, i2, j1, j2) = codes[0];
            codes[0] = (t, i1.max(i2.saturating_sub(n)), i2, j1.max(j2.saturating_sub(n)), j2);
        }
        let dernier = codes.len() - 1;
        if codes[dernier].0 == "equal" {
            let (t, i1, i2, j1, j2) = codes[dernier];
            codes[dernier] = (t, i1, i2.min(i1 + n), j1, j2.min(j1 + n));
        }
        let nn = n + n;
        let mut groupes = Vec::new();
        let mut groupe: Vec<Op> = Vec::new();
        for (t, mut i1, i2, mut j1, j2) in codes {
            if t == "equal" && i2 - i1 > nn {
                groupe.push((t, i1, i2.min(i1 + n), j1, j2.min(j1 + n)));
                groupes.push(std::mem::take(&mut groupe));
                i1 = i1.max(i2.saturating_sub(n));
                j1 = j1.max(j2.saturating_sub(n));
            }
            groupe.push((t, i1, i2, j1, j2));
        }
        if !groupe.is_empty() && !(groupe.len() == 1 && groupe[0].0 == "equal") {
            groupes.push(groupe);
        }
        groupes
    }
}

/// `_format_range_unified`.
fn plage(debut: usize, fin: usize) -> String {
    let mut commencement = debut + 1;
    let longueur = fin - debut;
    if longueur == 1 {
        return commencement.to_string();
    }
    if longueur == 0 {
        commencement -= 1;
    }
    format!("{},{}", commencement, longueur)
}

/// `difflib.unified_diff(a, b, de, vers, lineterm="", n=n)`.
pub fn unifie(a: &[String], b: &[String], de: &str, vers: &str, n: usize) -> Vec<String> {
    let c = Comparateur::nouveau(a, b);
    let mut out = Vec::new();
    for (g, groupe) in c.groupes(n).into_iter().enumerate() {
        if g == 0 {
            out.push(format!("--- {}", de));
            out.push(format!("+++ {}", vers));
        }
        let (premier, dernier) = (groupe[0], groupe[groupe.len() - 1]);
        out.push(format!("@@ -{} +{} @@", plage(premier.1, dernier.2), plage(premier.3, dernier.4)));
        for (t, i1, i2, j1, j2) in groupe {
            if t == "equal" {
                out.extend(a[i1..i2].iter().map(|l| format!(" {}", l)));
                continue;
            }
            if t == "replace" || t == "delete" {
                out.extend(a[i1..i2].iter().map(|l| format!("-{}", l)));
            }
            if t == "replace" || t == "insert" {
                out.extend(b[j1..j2].iter().map(|l| format!("+{}", l)));
            }
        }
    }
    out
}
