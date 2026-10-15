//! Le format des rubriques de la norme : longueur et expressions de controle
//! (CSL) du cahier technique (`moteur/formats.py`). Un releve par version de la
//! norme (`norme-dsn-base.json` le nomme) : un fichier se controle dans la table
//! de sa version (S10.G00.00.006) ; une saisie, dans celles de TOUTES les
//! versions livrees (la valeur d'une fiche part dans les fichiers de chacune).
//!
//! Pas de crate regex : `Regex` est un petit moteur a retour arriere pour le
//! sous-ensemble que la table des formats emploie (classes, groupes,
//! alternatives, `? * + {n,m}`, `\d \s`, `^ $`), prouve contre `re`.

use crate::fichiers::{dossier_du_code, json_en_cache, repr};
use crate::norme::Norme;
use crate::num::Resultat;
use crate::usages::{espace, lignes, strip};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

// Les rubriques de la table des formats qui se saisissent dans une fiche :
// (rubrique, fiche, champ), tels que `champs.champ_de_rubrique` les rend.
const SAISIES: [(&str, &str, &str); 32] = [
    ("S10.G00.01.003", "entreprise", "raison_sociale"),
    ("S21.G00.06.001", "entreprise", "siren"),
    ("S21.G00.06.002", "entreprise", "nic"),
    ("S21.G00.06.003", "entreprise", "naf"),
    ("S21.G00.06.004", "entreprise", "adresse"),
    ("S21.G00.06.005", "entreprise", "code_postal"),
    ("S21.G00.06.006", "entreprise", "commune"),
    ("S21.G00.11.022", "entreprise", "code_convention_dsn"),
    ("S21.G00.20.002", "entreprise", "france_travail_dossier_recouvrement"),
    ("S21.G00.30.001", "salarie", "nir"),
    ("S21.G00.30.002", "salarie", "nom"),
    ("S21.G00.30.004", "salarie", "prenom"),
    ("S21.G00.30.006", "salarie", "date_naissance"),
    ("S21.G00.30.007", "salarie", "lieu_naissance"),
    ("S21.G00.30.008", "salarie", "adresse"),
    ("S21.G00.30.009", "salarie", "code_postal"),
    ("S21.G00.30.010", "salarie", "commune"),
    ("S21.G00.30.013", "salarie", "code_nationalite_dsn"),
    ("S21.G00.30.014", "salarie", "departement_naissance"),
    ("S21.G00.30.015", "salarie", "pays_naissance"),
    ("S21.G00.30.018", "salarie", "email"),
    ("S21.G00.30.019", "salarie", "code_salarie"),
    ("S21.G00.40.002", "salarie", "statut_conventionnel"),
    ("S21.G00.40.003", "salarie", "statut_categoriel_rc"),
    ("S21.G00.40.004", "salarie", "pcs_ese"),
    ("S21.G00.40.005", "salarie", "code_emploi_ft"),
    ("S21.G00.40.006", "salarie", "emploi"),
    ("S21.G00.40.017", "entreprise", "idcc"),
    ("S21.G00.40.019", "entreprise", "siret"),
    ("S21.G00.40.022", "entreprise", "congesspectacles_affiliation"),
    ("S21.G00.40.050", "spectacle", "numero_objet"),
    ("S21.G00.40.072", "salarie", "statut_boeth"),
];

// ------------------------------------------------------ le moteur d'expressions

enum Elt {
    Car(char),
    Plage(char, char),
    Chiffre,
    Espace,
}

enum Noeud {
    Car(char),
    Tout,
    Classe(Vec<Elt>, bool),
    Chiffre,
    Espace,
    Debut,
    Fin,
    Groupe(Vec<Vec<Noeud>>),
    Rep(Box<Noeud>, usize, usize),
}

/// Une expression compilee ; `fullmatch` comme `re.fullmatch`.
pub struct Regex {
    pub motif: String,
    alts: Vec<Vec<Noeud>>,
}

struct Lecteur {
    c: Vec<char>,
    i: usize,
}

impl Lecteur {
    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }

    fn alt(&mut self) -> Resultat<Vec<Vec<Noeud>>> {
        let mut alts = vec![self.suite()?];
        while self.peek() == Some('|') {
            self.i += 1;
            alts.push(self.suite()?);
        }
        Ok(alts)
    }

    fn suite(&mut self) -> Resultat<Vec<Noeud>> {
        let mut seq = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let atome = self.atome()?;
            seq.push(self.quantifie(atome)?);
        }
        Ok(seq)
    }

    fn entier(&mut self) -> Option<usize> {
        let debut = self.i;
        while self.peek().map_or(false, |c| c.is_ascii_digit()) {
            self.i += 1;
        }
        if debut == self.i {
            return None;
        }
        self.c[debut..self.i].iter().collect::<String>().parse().ok()
    }

    fn quantifie(&mut self, atome: Noeud) -> Resultat<Noeud> {
        let (min, max) = match self.peek() {
            Some('?') => (0, 1),
            Some('*') => (0, usize::MAX),
            Some('+') => (1, usize::MAX),
            Some('{') => {
                self.i += 1;
                let min = self.entier().ok_or("accolade non prise en charge")?;
                let max = if self.peek() == Some(',') {
                    self.i += 1;
                    self.entier().unwrap_or(usize::MAX)
                } else {
                    min
                };
                if self.peek() != Some('}') {
                    return Err("accolade non prise en charge".into());
                }
                (min, max)
            }
            _ => return Ok(atome),
        };
        self.i += 1;
        if matches!(self.peek(), Some('?') | Some('+') | Some('*') | Some('{')) {
            return Err("quantificateur non prise en charge".into());
        }
        Ok(Noeud::Rep(Box::new(atome), min, max))
    }

    fn echappe(&mut self) -> Resultat<Elt> {
        let c = self.peek().ok_or("barre oblique finale")?;
        self.i += 1;
        match c {
            'd' => Ok(Elt::Chiffre),
            's' => Ok(Elt::Espace),
            c if c.is_ascii_alphanumeric() => Err(format!("echappement \\{} non pris en charge", c)),
            c => Ok(Elt::Car(c)),
        }
    }

    fn atome(&mut self) -> Resultat<Noeud> {
        let c = self.peek().unwrap();
        self.i += 1;
        match c {
            '(' => {
                if self.peek() == Some('?') {
                    if self.c.get(self.i + 1) != Some(&':') {
                        return Err("groupe special non pris en charge".into());
                    }
                    self.i += 2;
                }
                let a = self.alt()?;
                if self.peek() != Some(')') {
                    return Err("parenthese non fermee".into());
                }
                self.i += 1;
                Ok(Noeud::Groupe(a))
            }
            '[' => {
                let nie = self.peek() == Some('^');
                if nie {
                    self.i += 1;
                }
                let mut elts = Vec::new();
                loop {
                    let c = self.peek().ok_or("crochet non ferme")?;
                    self.i += 1;
                    if c == ']' && !elts.is_empty() {
                        break;
                    }
                    let e = if c == '\\' { self.echappe()? } else { Elt::Car(c) };
                    if let Elt::Car(a) = e {
                        if self.peek() == Some('-') && self.c.get(self.i + 1).map_or(false, |&x| x != ']') {
                            self.i += 1;
                            let b = self.peek().unwrap();
                            self.i += 1;
                            let b = if b == '\\' {
                                match self.echappe()? {
                                    Elt::Car(b) => b,
                                    _ => return Err("plage non prise en charge".into()),
                                }
                            } else {
                                b
                            };
                            elts.push(Elt::Plage(a, b));
                            continue;
                        }
                    }
                    elts.push(e);
                }
                Ok(Noeud::Classe(elts, nie))
            }
            '\\' => Ok(match self.echappe()? {
                Elt::Chiffre => Noeud::Chiffre,
                Elt::Espace => Noeud::Espace,
                Elt::Car(c) => Noeud::Car(c),
                Elt::Plage(..) => unreachable!(),
            }),
            '^' => Ok(Noeud::Debut),
            '$' => Ok(Noeud::Fin),
            '.' => Ok(Noeud::Tout),
            '*' | '+' | '?' | '{' => Err("quantificateur sans objet".into()),
            c => Ok(Noeud::Car(c)),
        }
    }
}

type K<'a> = &'a mut dyn FnMut(usize) -> bool;

fn dans(e: &Elt, c: char) -> bool {
    match e {
        Elt::Car(x) => *x == c,
        Elt::Plage(a, b) => *a <= c && c <= *b,
        Elt::Chiffre => c.is_ascii_digit(),
        Elt::Espace => espace(c),
    }
}

fn alt(a: &[Vec<Noeud>], s: &[char], p: usize, k: K) -> bool {
    a.iter().any(|seq| suite(seq, s, p, &mut *k))
}

fn suite(seq: &[Noeud], s: &[char], p: usize, k: K) -> bool {
    match seq.split_first() {
        None => k(p),
        Some((n, reste)) => noeud(n, s, p, &mut |q| suite(reste, s, q, &mut *k)),
    }
}

fn un(s: &[char], p: usize, ok: impl Fn(char) -> bool, k: K) -> bool {
    p < s.len() && ok(s[p]) && k(p + 1)
}

fn noeud(n: &Noeud, s: &[char], p: usize, k: K) -> bool {
    match n {
        Noeud::Car(c) => un(s, p, |x| x == *c, k),
        Noeud::Tout => un(s, p, |x| x != '\n', k),
        Noeud::Classe(elts, nie) => un(s, p, |x| elts.iter().any(|e| dans(e, x)) != *nie, k),
        Noeud::Chiffre => un(s, p, |x| x.is_ascii_digit(), k),
        Noeud::Espace => un(s, p, espace, k),
        Noeud::Debut => p == 0 && k(p),
        Noeud::Fin => p == s.len() && k(p),
        Noeud::Groupe(a) => alt(a, s, p, k),
        Noeud::Rep(n, min, max) => rep(n, *min, *max, 0, s, p, k),
    }
}

fn rep(n: &Noeud, min: usize, max: usize, c: usize, s: &[char], p: usize, k: K) -> bool {
    if c < max
        && noeud(n, s, p, &mut |q| {
            if q == p && c >= min {
                false
            } else {
                rep(n, min, max, c + 1, s, q, &mut *k)
            }
        })
    {
        return true;
    }
    c >= min && k(p)
}

impl Regex {
    pub fn compiler(motif: &str) -> Resultat<Regex> {
        let mut l = Lecteur { c: motif.chars().collect(), i: 0 };
        let alts = l.alt().map_err(|e| format!("{} : {}", repr(motif), e))?;
        if l.i != l.c.len() {
            return Err(format!("{} : parenthese fermante sans ouvrante", repr(motif)));
        }
        Ok(Regex { motif: motif.to_string(), alts })
    }

    /// `re.fullmatch(valeur)` est-il non nul ?
    pub fn fullmatch(&self, valeur: &str) -> bool {
        let s: Vec<char> = valeur.chars().collect();
        let n = s.len();
        alt(&self.alts, &s, 0, &mut |q| q == n)
    }
}

// ----------------------------------------------------------------- la table

/// La table des formats de cette version de la norme.
pub fn table_de(norme: &Norme) -> Resultat<Arc<Value>> {
    json_en_cache(&dossier_du_code().join(norme.table_formats()?))
}

/// {'longueur': [min, max], 'csl': [expression...]} dans la table de cette version -- {} si inconnue.
pub fn format_en(norme: &Norme, rubrique: &str) -> Resultat<Value> {
    let t = table_de(norme)?;
    Ok(match t["formats"].get(rubrique) {
        Some(f) if crate::fichiers::vrai(f) => f.clone(),
        _ => Value::Object(Default::default()),
    })
}

/// Pour une saisie : le format de la premiere version livree qui en donne un -- {} si aucune.
pub fn format_de(rubrique: &str) -> Resultat<Value> {
    for n in crate::norme::versions()? {
        let f = format_en(&n, rubrique)?;
        if f.as_object().map_or(false, |o| !o.is_empty()) {
            return Ok(f);
        }
    }
    Ok(Value::Object(Default::default()))
}

fn expressions(norme: &Norme, rubrique: &str) -> Resultat<Arc<Vec<Regex>>> {
    static CACHE: OnceLock<Mutex<HashMap<(String, String), Arc<Vec<Regex>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let cle = (norme.table_formats()?, rubrique.to_string());
    if let Some(e) = cache.lock().unwrap().get(&cle) {
        return Ok(e.clone());
    }
    let mut v = Vec::new();
    if let Some(csl) = format_en(norme, rubrique)?.get("csl").and_then(|c| c.as_array()) {
        for c in csl {
            v.push(Regex::compiler(c.as_str().unwrap_or(""))?);
        }
    }
    let v = Arc::new(v);
    cache.lock().unwrap().insert(cle, v.clone());
    Ok(v)
}

/// Pour une saisie : pourquoi la valeur ne va pas dans l'une des versions livrees (la premiere qui la refuse) -- ""
/// si elle va dans toutes, ou si on ne sait pas.
pub fn mauvaise(rubrique: &str, valeur: &str) -> Resultat<String> {
    for n in crate::norme::versions()? {
        let m = mauvaise_en(&n, rubrique, valeur)?;
        if !m.is_empty() {
            return Ok(m);
        }
    }
    Ok(String::new())
}

/// Pour une saisie : les caracteres dont le retrait fait passer la valeur, dans la premiere version livree qui la
/// refuse.
pub fn caracteres_refuses(rubrique: &str, valeur: &str) -> Resultat<Vec<char>> {
    for n in crate::norme::versions()? {
        if !mauvaise_en(&n, rubrique, valeur)?.is_empty() {
            return caracteres_refuses_en(&n, rubrique, valeur);
        }
    }
    Ok(vec![])
}

/// Pourquoi cette valeur ne va pas dans cette version -- "" si elle va, ou si on ne sait pas.
pub fn mauvaise_en(norme: &Norme, rubrique: &str, valeur: &str) -> Resultat<String> {
    let fmt = format_en(norme, rubrique)?;
    if fmt.as_object().map_or(true, |o| o.is_empty()) {
        return Ok(String::new());
    }
    if let Some(lon) = fmt.get("longueur").and_then(|l| l.as_array()).filter(|l| !l.is_empty()) {
        let (mini, maxi) = (lon[0].as_i64().unwrap_or(0), lon[1].as_i64().unwrap_or(0));
        let n = valeur.chars().count() as i64;
        if !(mini <= n && n <= maxi) {
            return Ok(format!("longueur {}, attendue entre {} et {}", n, mini, maxi));
        }
    }
    let exprs = expressions(norme, rubrique)?;
    if !exprs.is_empty() && !exprs.iter().any(|e| e.fullmatch(valeur)) {
        let motifs: Vec<&str> = exprs.iter().map(|e| e.motif.as_str()).collect();
        return Ok(format!("ne suit pas {}", motifs.join(" ni ")));
    }
    Ok(String::new())
}

/// Les caracteres dont le retrait suffit a faire passer la valeur, dans cette version.
pub fn caracteres_refuses_en(norme: &Norme, rubrique: &str, valeur: &str) -> Resultat<Vec<char>> {
    let exprs = expressions(norme, rubrique)?;
    if exprs.is_empty() || exprs.iter().any(|e| e.fullmatch(valeur)) {
        return Ok(vec![]);
    }
    let mut suspects: Vec<char> = Vec::new();
    for c in valeur.chars() {
        if !suspects.contains(&c) && !exprs.iter().any(|e| e.fullmatch(&c.to_string())) {
            suspects.push(c);
        }
    }
    let reste: String = valeur.chars().filter(|c| !suspects.contains(c)).collect();
    if suspects.is_empty() || reste.is_empty() || !exprs.iter().any(|e| e.fullmatch(&reste)) {
        return Ok(vec![]);
    }
    Ok(suspects)
}

/// `^(S\d\d\.G00\.\d\d\.\d\d\d),'(.*)'$` : (rubrique, valeur).
fn ligne_valeur(ligne: &str) -> Option<(&str, &str)> {
    let c: Vec<(usize, char)> = ligne.char_indices().take(17).collect();
    if c.len() < 17 || !ligne.ends_with('\'') {
        return None;
    }
    let rubrique = &ligne[..c[14].0];
    if !crate::usages::est_rubrique(rubrique) || c[14].1 != ',' || c[15].1 != '\'' {
        return None;
    }
    let milieu = &ligne[c[16].0..ligne.len() - 1];
    if milieu.contains('\n') {
        return None;
    }
    Some((rubrique, milieu))
}

/// [(rubrique, valeur, pourquoi)] pour tout le fichier, dans la table de cette version.
pub fn mauvaises(texte: &str, norme: &Norme) -> Resultat<Vec<(String, String, String)>> {
    let mut out = Vec::new();
    for ligne in lignes(texte) {
        if let Some((r, v)) = ligne_valeur(strip(ligne)) {
            let pourquoi = mauvaise_en(norme, r, v)?;
            if !pourquoi.is_empty() {
                out.push((r.to_string(), v.to_string(), pourquoi));
            }
        }
    }
    Ok(out)
}

fn libelles(norme: &Norme) -> Resultat<HashMap<String, String>> {
    let t = crate::usages::table_de(norme)?;
    let mut m = HashMap::new();
    if let Some(u) = t["usages"].as_object() {
        for (r, d) in u {
            if let Some(l) = d.get("libelle").and_then(|l| l.as_str()).filter(|l| !l.is_empty()) {
                m.insert(r.clone(), format!(" ({})", l));
            }
        }
    }
    Ok(m)
}

fn dire(norme: &Norme, r: &str, v: &str, pourquoi: &str, libelle: &str, ou: Option<&(&str, &str, &str)>) -> Resultat<String> {
    let ou = match ou {
        None => return Ok(format!("    {} = '{}' : {}{}", r, v, pourquoi, libelle)),
        Some(o) => o,
    };
    let refuses = caracteres_refuses_en(norme, r, v)?;
    let pourquoi = if refuses.is_empty() {
        pourquoi.to_string()
    } else {
        let liste: Vec<String> = refuses.iter().map(|c| format!("<< {} >>", c)).collect();
        format!("{} n'y {} pas admis", liste.join(", "), if refuses.len() > 1 { "sont" } else { "est" })
    };
    Ok(format!(
        "    {} : {}{} -- fiche {}, champ {} (la valeur n'est pas recopiee ici : elle est dans la fiche)",
        r, pourquoi, libelle, ou.1, ou.2
    ))
}

/// Refuse avant le depot ce que le point de depot refuserait, dans la table de la version que le fichier porte
/// (S10.G00.00.006) ; puis les valeurs que cette version a retirees de leur liste (`norme::verifier_les_valeurs`). Le
/// texte de l'erreur est celui de `str(ValeurHorsNorme)`.
pub fn verifier_fichier(texte: &str) -> Resultat<()> {
    let norme = crate::norme::du_fichier(texte)?;
    let mauv = mauvaises(texte, &norme)?;
    if mauv.is_empty() {
        return crate::norme::verifier_les_valeurs(texte, &norme);
    }
    let libelles = libelles(&norme)?;
    let mut saisies: Vec<&(&str, &str, &str)> = Vec::new();
    for (r, _, _) in &mauv {
        if let Some(s) = SAISIES.iter().find(|s| s.0 == r) {
            if !saisies.iter().any(|x| x.0 == s.0) {
                saisies.push(s);
            }
        }
    }
    let trou = if saisies.is_empty() {
        String::new()
    } else {
        format!(
            "\n  TROU DANS LA TABLE DE SAISIE : {} se saisi(ssen)t dans une fiche et auraient du etre refuse(s) au clavier, pas ici. Voir `champs.RUBRIQUES`.",
            saisies.iter().map(|s| s.0).collect::<Vec<_>>().join(", ")
        )
    };
    let mut dites = Vec::new();
    for (r, v, p) in mauv.iter().take(20) {
        let lib = libelles.get(r).map(|s| s.as_str()).unwrap_or("");
        dites.push(dire(&norme, r, v, p, lib, saisies.iter().find(|s| s.0 == r).copied())?);
    }
    Err(format!(
        "Valeur(s) qui ne suivent pas le format de la norme :\n{}\n  Formats du cahier technique NEODeS (donnees/{}).{}",
        dites.join("\n"),
        norme.table_formats()?,
        trou
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::norme::Norme;

    #[test]
    fn un_fichier_sans_version_ne_se_controle_pas() {
        let e = verifier_fichier("S20.G00.05.001,'01'\r\n").unwrap_err();
        assert!(e.starts_with("Le fichier ne porte pas S10.G00.00.006"), "{}", e);
    }

    /// Le releve de `paie extraire formats` sur les classeurs 2027.1.2 : les CSL de 40.070 et 41.045 s'elargissent
    /// (evolutions 15 et 18) ; la date de naissance perd les annees 18xx en 2027 (p. 172). Une saisie se controle dans
    /// toutes les versions livrees, un fichier dans la sienne.
    #[test]
    fn les_formats_2027_sont_ceux_du_cahier_2027() {
        let (p26, p27) = (Norme::connue("P26V01").unwrap(), Norme::connue("P27V01").unwrap());
        assert!(mauvaise_en(&p26, "S21.G00.40.070", "13").unwrap().starts_with("ne suit pas"));
        assert_eq!(mauvaise_en(&p27, "S21.G00.40.070", "13").unwrap(), "");
        assert_eq!(mauvaise_en(&p27, "S21.G00.41.045", "45").unwrap(), "");
        assert_eq!(mauvaise_en(&p26, "S21.G00.30.006", "01011899").unwrap(), "");
        assert!(mauvaise_en(&p27, "S21.G00.30.006", "01011899").unwrap().starts_with("ne suit pas"));
        assert!(!mauvaise("S21.G00.30.006", "01011899").unwrap().is_empty());
        assert_eq!(mauvaise("S21.G00.30.006", "01011990").unwrap(), "");
    }
}
