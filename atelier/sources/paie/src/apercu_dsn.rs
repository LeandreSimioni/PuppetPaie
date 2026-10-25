//! `depot-apercu` pour une DSN : ce qui partirait, en clair, sans rien envoyer ni demander.
//!
//! C'est le Python de `operations_declarations.apercu_depot_dsn` et de ce qu'il appelle
//! (`dsn_api.lire`, `_remplacement_du_fichier`, `_recapitulatif_sur`, `recapitulatif_dsn`,
//! `dsn_recap.lire`, `operations_documents.saisie_aux_numeros_declares`,
//! `operations_documents.numeroter_depuis_archive`, `bulletin.numeros_archives`), porte a
//! l'identique : meme JSON, memes refus sous la meme classe.
//!
//! Aucun reseau, aucun mot de passe, aucune ecriture de ce module. Une seule ecriture, et
//! elle est celle que `paie calculer` faisait deja quand le Python l'appelait : le cache
//! `paies/AAAA-MM.calcul.json` du calcul du mois (`calcul_du_mois::calcul_avec`), que la
//! route `apercu` ecrit de la meme facon.
//!
//! Les pieces reprises du reste du moteur : `Moteur::etat_remplacement` (l'annule-et-
//! remplace d'un etablissement), `routes_ouverture::{paie_du_mois, engagements_calcules,
//! paie_de_json, numeroter}` (la saisie calculee, numerotee), `depot::lire` (la lecture du
//! fichier), `etats::{deja_depose, repetition}` (le journal).
//!
//! DEUX ECARTS ASSUMES AVEC `decimal.Decimal` : `NaN` et `Infinity` ne se lisent pas ici
//! (le Python les accepte puis echoue plus loin), et un exposant qui sortirait des 28
//! chiffres de `rust_decimal` est refuse comme un nombre illisible.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::str::FromStr;

use serde_json::{json, Value};

use crate::calcul::Paie;
use droit::contexte::Contexte;
use crate::depot::{self, Apercu};
use crate::engagement;
use crate::etats;
use socle::echec::erreur;
use socle::fichiers::{repr, vrai};
use socle::num::Decimal;
use socle::texte::blanc;
use socle::echec::Echec;
use crate::routes_apercus::{lire_octets, nom_du_chemin, resoudre};
use crate::routes_mois::Moteur;
use crate::routes_ouverture::{engagements_calcules, numeroter, paie_de_json, paie_du_mois, Refus};

type R<T> = Result<T, Echec>;

const TYPES: [(&str, &str); 5] = depot::TYPES;

// ------------------------------------------------------------------ les petits outils de Python

/// `int(texte)` : le signe, les chiffres, un tiret bas entre deux chiffres, les blancs autour.
/// Python n'a pas de plafond : un entier trop long pour `i128` se rend au plus grand.
pub(crate) fn int_py(t: &str) -> Option<i128> {
    let t = t.trim_matches(blanc);
    let (neg, reste) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    if reste.is_empty() || reste.starts_with('_') || reste.ends_with('_') || reste.contains("__") {
        return None;
    }
    let chiffres: String = reste.chars().filter(|c| *c != '_').collect();
    if !chiffres.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let n = chiffres.parse::<i128>().unwrap_or(i128::MAX);
    Some(if neg { -n } else { n })
}

/// `Decimal(texte)` : le signe, les chiffres avec un point et des tirets bas entre deux
/// chiffres, un exposant. `None` quand Python leve `InvalidOperation`.
pub(crate) fn decimal_py(t: &str) -> Option<Decimal> {
    let t = t.trim_matches(blanc);
    let c: Vec<char> = t.chars().collect();
    let mut i = 0;
    let mut propre = String::new();
    if i < c.len() && (c[i] == '+' || c[i] == '-') {
        if c[i] == '-' {
            propre.push('-');
        }
        i += 1;
    }
    // une suite de chiffres, avec des tirets bas seulement entre deux chiffres
    let suite = |i: &mut usize, sortie: &mut String| -> usize {
        let mut n = 0;
        while *i < c.len() {
            if c[*i].is_ascii_digit() {
                sortie.push(c[*i]);
                n += 1;
            } else if c[*i] == '_' && n > 0 && *i + 1 < c.len() && c[*i + 1].is_ascii_digit() {
            } else {
                break;
            }
            *i += 1;
        }
        n
    };
    let mut entier = String::new();
    let ne = suite(&mut i, &mut entier);
    let mut frac = String::new();
    let mut nf = 0;
    if i < c.len() && c[i] == '.' {
        i += 1;
        nf = suite(&mut i, &mut frac);
    }
    if ne + nf == 0 {
        return None;
    }
    let mut exposant = String::new();
    if i < c.len() && (c[i] == 'e' || c[i] == 'E') {
        i += 1;
        if i < c.len() && (c[i] == '+' || c[i] == '-') {
            exposant.push(c[i]);
            i += 1;
        }
        if suite(&mut i, &mut exposant) == 0 {
            return None;
        }
    }
    if i != c.len() {
        return None;
    }
    propre.push_str(if entier.is_empty() { "0" } else { &entier });
    if !frac.is_empty() {
        propre.push('.');
        propre.push_str(&frac);
    }
    if exposant.is_empty() {
        Decimal::from_str(&propre).ok()
    } else {
        Decimal::from_scientific(&format!("{}e{}", propre, exposant)).ok()
    }
}

/// `format(d.normalize(), "f")`, avec `0` pour zero.
fn norm(d: Decimal) -> String {
    if d.is_zero() {
        "0".to_string()
    } else {
        d.normalize().to_string()
    }
}

/// `str(d.quantize(Decimal("0.01")))` : deux decimales, la moitie au pair.
fn q2(d: Decimal) -> String {
    let mut r = d.round_dp(2);
    r.rescale(2);
    r.to_string()
}

/// `float(d)`.
fn flottant(d: Decimal) -> f64 {
    d.to_string().parse::<f64>().unwrap_or(0.0)
}

/// Les lignes d'un texte comme les compte `str.splitlines()` de Python (le numéro qu'on affiche est le sien) : `\n`,
/// `\r`, `\r\n`, et les séparateurs `\x0b`, `\x0c`, `\x1c`-`\x1e`, `\u{85}`, `\u{2028}`, `\u{2029}` ; une fin de ligne
/// finale ne fait pas de ligne vide. La seule copie du moteur (DSN, retours, DILA, garde avant commit).
pub fn lignes_py(t: &str) -> Vec<&str> {
    let mut v = Vec::new();
    let mut debut = 0;
    let mut it = t.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        if matches!(c, '\n' | '\r' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}') {
            v.push(&t[debut..i]);
            debut = i + c.len_utf8();
            if c == '\r' && it.peek().is_some_and(|&(_, d)| d == '\n') {
                it.next();
                debut += 1;
            }
        }
    }
    if debut < t.len() {
        v.push(&t[debut..]);
    }
    v
}

pub(crate) fn texte_latin1(o: &[u8]) -> String {
    o.iter().map(|&b| b as char).collect()
}

// ------------------------------------------------------------------ dsn_recap.lire

/// `dsn_recap._date` : JJMMAAAA -> AAAA-MM-JJ.
fn date_dsn(v: &str) -> String {
    let c: Vec<char> = v.chars().collect();
    if c.len() == 8 {
        let t = |a: usize, b: usize| c[a..b].iter().collect::<String>();
        format!("{}-{}-{}", t(4, 8), t(2, 4), t(0, 2))
    } else {
        v.to_string()
    }
}

#[derive(Default)]
pub(crate) struct Contrat {
    pub(crate) numero: String,
    pub(crate) debut: String,
    pub(crate) fin: String,
    pub(crate) emploi: String,
    pub(crate) nature: String,
    pub(crate) objet: String,
    pub(crate) brut: Decimal,
    pub(crate) jours: Decimal,
    pub(crate) cachets: Decimal,
    pub(crate) heures: Decimal,
    pub(crate) fin_declaree: String,
    pub(crate) motif_fin: String,
    pub(crate) remunere: bool,
}

#[derive(Default)]
pub(crate) struct Versement {
    pub(crate) date: String,
    pub(crate) net_verse: String,
    pub(crate) net_fiscal: String,
    pub(crate) taux_pas: String,
    pub(crate) montant_pas: String,
}

#[derive(Default)]
pub(crate) struct Individu {
    pub(crate) nir: String,
    pub(crate) nom: String,
    pub(crate) prenoms: String,
    pub(crate) contrats: Vec<Contrat>,
    pub(crate) versements: Vec<Versement>,
    pub(crate) par_numero: HashMap<String, (usize, usize)>,
}

/// Les sorties de `dsn_recap.lire` sous la forme que le recapitulatif complete : les montants
/// en texte, comme le Python les rend.
impl Contrat {
    fn brut_texte(&self) -> String {
        q2(self.brut)
    }
    fn mesure_texte(&self, cle: &str) -> String {
        norm(match cle {
            "jours" => self.jours,
            "cachets" => self.cachets,
            _ => self.heures,
        })
    }
}

/// `dsn_recap.lire` : les salaries du fichier, leurs contrats, leurs versements.
pub(crate) fn lire_recap(octets: &[u8]) -> Result<Vec<Individu>, String> {
    let texte = texte_latin1(octets);
    let mut sals: Vec<Individu> = Vec::new();
    let mut contrat: Option<(usize, usize)> = None;
    let mut versement: Option<(usize, usize)> = None;
    let mut fin: Option<(usize, usize)> = None;
    // (numero de contrat, type) de la remuneration en cours
    let mut remu: Option<(String, String)> = None;
    // (type, mesure) de l'activite en cours
    let mut activite: Option<(String, Decimal)> = None;
    for ligne in lignes_py(&texte) {
        let Some((k, v)) = ligne.split_once(',') else { continue };
        let v = v.trim_matches(blanc).trim_matches('\'');
        let kc: Vec<char> = k.chars().collect();
        let bloc: String = kc.iter().take(10).collect();
        let champ: String = kc.iter().skip(11).collect();
        if bloc == "S21.G00.30" {
            if champ == "001" || sals.is_empty() {
                sals.push(Individu::default());
            }
            let s = sals.last_mut().unwrap();
            match champ.as_str() {
                "001" => s.nir = v.to_string(),
                "002" => s.nom = v.to_string(),
                "004" => s.prenoms = v.to_string(),
                _ => {}
            }
            continue;
        }
        if sals.is_empty() {
            continue;
        }
        let si = sals.len() - 1;
        match bloc.as_str() {
            "S21.G00.40" => {
                if champ == "001" {
                    sals[si].contrats.push(Contrat { debut: date_dsn(v), ..Contrat::default() });
                    contrat = Some((si, sals[si].contrats.len() - 1));
                } else if let Some((cs, cc)) = contrat {
                    match champ.as_str() {
                        "006" => sals[cs].contrats[cc].emploi = v.to_string(),
                        "007" => sals[cs].contrats[cc].nature = v.to_string(),
                        "009" => {
                            sals[cs].contrats[cc].numero = v.to_string();
                            sals[si].par_numero.insert(v.to_string(), (cs, cc));
                        }
                        "010" => sals[cs].contrats[cc].fin = date_dsn(v),
                        "050" => sals[cs].contrats[cc].objet = v.to_string(),
                        _ => {}
                    }
                }
            }
            "S21.G00.62" => {
                if champ == "001" && contrat.is_some() {
                    fin = contrat;
                    let (fs, fc) = fin.unwrap();
                    sals[fs].contrats[fc].fin_declaree = date_dsn(v);
                } else if champ == "002" {
                    if let Some((fs, fc)) = fin {
                        sals[fs].contrats[fc].motif_fin = v.to_string();
                    }
                }
            }
            "S21.G00.50" => {
                if champ == "001" {
                    sals[si].versements.push(Versement { date: date_dsn(v), ..Versement::default() });
                    versement = Some((si, sals[si].versements.len() - 1));
                } else if let Some((vs, vi)) = versement {
                    let x = &mut sals[vs].versements[vi];
                    match champ.as_str() {
                        "002" => x.net_fiscal = v.to_string(),
                        "004" => x.net_verse = v.to_string(),
                        "006" => x.taux_pas = v.to_string(),
                        "009" => x.montant_pas = v.to_string(),
                        _ => {}
                    }
                }
            }
            "S21.G00.51" => {
                if champ == "001" {
                    remu = Some((String::new(), String::new()));
                } else if let Some(r) = remu.as_mut() {
                    match champ.as_str() {
                        "010" => r.0 = v.to_string(),
                        "011" => r.1 = v.to_string(),
                        "013" => {
                            let montant = decimal_py(v).ok_or_else(|| socle::num::illisible(v))?;
                            let (numero, genre) = (r.0.clone(), r.1.clone());
                            if let Some(&(cs, cc)) = sals[si].par_numero.get(&numero) {
                                let c = &mut sals[cs].contrats[cc];
                                c.remunere = true;
                                if genre == "001" {
                                    c.brut += montant;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            "S21.G00.53" => {
                if champ == "001" {
                    activite = Some((v.to_string(), Decimal::ZERO));
                } else if activite.is_some() && champ == "002" {
                    activite.as_mut().unwrap().1 = decimal_py(v).ok_or_else(|| socle::num::illisible(v))?;
                } else if activite.is_some() && champ == "003" {
                    let (genre, mesure) = activite.clone().unwrap();
                    let c = remu.as_ref().and_then(|r| sals[si].par_numero.get(&r.0).copied());
                    let unite = match v {
                        "10" | "35" => Some("heures"),
                        "12" => Some("jours"),
                        "37" => Some("cachets"),
                        _ => None,
                    };
                    if let (Some((cs, cc)), Some(u)) = (c, unite) {
                        if genre == "01" {
                            let c = &mut sals[cs].contrats[cc];
                            match u {
                                "jours" => c.jours += mesure,
                                "cachets" => c.cachets += mesure,
                                _ => c.heures += mesure,
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(sals)
}

// ------------------------------------------------------------------ Bulletins_<Mois>.pdf

const MARQUE_NUMERO: &str = "Bulletin de paie n";
const MARQUE_CONTRAT: &str = "Contrat n";


fn suite_de(m: &[char], de: usize, f: impl Fn(char) -> bool) -> usize {
    m.get(de..).map_or(0, |r| r.iter().take_while(|c| f(**c)).count())
}

fn commence_par(m: &[char], de: usize, t: &str) -> bool {
    let u: Vec<char> = t.chars().collect();
    m.len() >= de + u.len() && m[de..de + u.len()] == u[..]
}

/// `(\d+/\d{4})` a cette position.
fn numero_a(m: &[char], p: usize) -> Option<String> {
    let d = suite_de(m, p, |c| c.is_ascii_digit());
    if d == 0 || m.get(p + d) != Some(&'/') {
        return None;
    }
    let an = suite_de(m, p + d + 1, |c| c.is_ascii_digit());
    if an < 4 {
        return None;
    }
    Some(m[p..p + d + 5].iter().collect())
}

/// `Bulletin de paie n\S*\s*(\d+/\d{4})`, au debut de `m`.
fn forme_numero(m: &[char]) -> Option<String> {
    if !commence_par(m, 0, MARQUE_NUMERO) {
        return None;
    }
    let p0 = MARQUE_NUMERO.chars().count();
    for j in (0..=suite_de(m, p0, |c| !blanc(c))).rev() {
        let p1 = p0 + j;
        for w in (0..=suite_de(m, p1, blanc)).rev() {
            if let Some(n) = numero_a(m, p1 + w) {
                return Some(n);
            }
        }
    }
    None
}

/// `Contrat n\S*\s*(\S+?)\s*—`, la premiere occurrence qui s'accorde.
fn forme_contrat(m: &[char]) -> Option<String> {
    let p_marque = MARQUE_CONTRAT.chars().count();
    for debut in 0..m.len() {
        if !commence_par(m, debut, MARQUE_CONTRAT) {
            continue;
        }
        let p0 = debut + p_marque;
        for j in (0..=suite_de(m, p0, |c| !blanc(c))).rev() {
            let p1 = p0 + j;
            for w in (0..=suite_de(m, p1, blanc)).rev() {
                let p2 = p1 + w;
                let max = suite_de(m, p2, |c| !blanc(c));
                for g in 1..=max {
                    let p3 = p2 + g;
                    for w2 in (0..=suite_de(m, p3, blanc)).rev() {
                        if m.get(p3 + w2) == Some(&'\u{2014}') {
                            return Some(m[p2..p3].iter().collect());
                        }
                    }
                }
            }
        }
    }
    None
}

/// `\d\d/\d\d/\d{4}` a cette position.
fn date_a(m: &[char], p: usize) -> Option<String> {
    let motif = [0usize, 1, 3, 4, 6, 7, 8, 9];
    if m.len() < p + 10 || m[p + 2] != '/' || m[p + 5] != '/' || !motif.iter().all(|&i| m[p + i].is_ascii_digit()) {
        return None;
    }
    Some(m[p..p + 10].iter().collect())
}

/// `P\S*riode\s*:?\s*du\s*(\d\d/\d\d/\d{4})\s*au\s*(\d\d/\d\d/\d{4})`, la premiere occurrence.
fn forme_periode(m: &[char]) -> Option<(String, String)> {
    for p in 0..m.len() {
        if m[p] != 'P' {
            continue;
        }
        for k in (0..=suite_de(m, p + 1, |c| !blanc(c))).rev() {
            let mut q = p + 1 + k;
            if !commence_par(m, q, "riode") {
                continue;
            }
            q += 5;
            q += suite_de(m, q, blanc);
            if m.get(q) == Some(&':') {
                q += 1;
            }
            q += suite_de(m, q, blanc);
            if !commence_par(m, q, "du") {
                continue;
            }
            q += 2;
            q += suite_de(m, q, blanc);
            let Some(debut) = date_a(m, q) else { continue };
            q += 10;
            q += suite_de(m, q, blanc);
            if !commence_par(m, q, "au") {
                continue;
            }
            q += 2;
            q += suite_de(m, q, blanc);
            let Some(fin) = date_a(m, q) else { continue };
            return Some((debut, fin));
        }
    }
    None
}

#[derive(Clone)]
pub(crate) struct Lu {
    pub(crate) bulletin: String,
    pub(crate) contrat: String,
    pub(crate) debut: String,
    pub(crate) fin: String,
    /// Le net a payer avant impot, quand il se lit (`_FORME_NET`) : une correction y lit le net deja verse. (Le brut ne
    /// se lit plus : seul le dossier du salarie le confrontait a la paie recalculee, et il envoie l'archive depuis le lot
    /// V1p.)
    pub(crate) net: Option<Decimal>,
    /// Les pages du fichier qui portent ce bulletin (0 = la premiere) : de celle ou il commence a celle qui precede le
    /// bulletin suivant. Le dossier du salarie n'envoie que celles-la.
    pub(crate) pages: Vec<usize>,
    /// Un autre bulletin commence sur l'une de ces pages : elles ne se donnent pas a un seul salarie. Le moteur ecrit un
    /// bulletin par page (`bulletin::description`, un document par jeu) ; un fichier qui en met deux sur une page n'est
    /// pas de lui.
    pub(crate) partage: bool,
}

/// `\s*\n\s*(-?[\d\u00a0\u202f .]+,\d\d)` a partir de `p` : le montant ecrit, tel quel.
fn montant_apres(m: &[char], p: usize) -> Option<String> {
    let blancs = suite_de(m, p, blanc);
    if !m[p..p + blancs].contains(&'\n') {
        return None;
    }
    let debut = p + blancs;
    let mut q = debut;
    if m.get(q) == Some(&'-') {
        q += 1;
    }
    let n = suite_de(m, q, |c| c.is_ascii_digit() || c == '\u{a0}' || c == '\u{202f}' || c == ' ' || c == '.');
    let r = q + n;
    if n == 0 || m.get(r) != Some(&',') || !m.get(r + 1).is_some_and(char::is_ascii_digit) || !m.get(r + 2).is_some_and(char::is_ascii_digit) {
        return None;
    }
    Some(m[debut..r + 3].iter().collect())
}

/// `_montant_lu` : « 1 234,56 » -> 1234.56.
fn montant_lu(t: &str) -> Option<Decimal> {
    let net: String = t.chars().filter(|c| !blanc(*c) && *c != '\u{a0}' && *c != '\u{202f}' && *c != '.').map(|c| if c == ',' { '.' } else { c }).collect();
    Decimal::from_str(&net).ok()
}

/// Les positions de `marque` dans `m`.
fn occurrences(m: &[char], marque: &str) -> Vec<usize> {
    (0..m.len()).filter(|&i| commence_par(m, i, marque)).collect()
}

/// `_FORME_NET.search(m)` : le premier « NET A PAYER AVANT IMP... » ou « NET À PAYER AVANT IMP... » suivi d'un montant.
/// Le bulletin imprime le net a payer avant impot sur la ligne « MONTANT NET A PAYER AVANT IMPOT SUR LE REVENU » depuis
/// la vague 1o (le modele de l'arrete du 25/02/2016) ; avant, sur « NET SOCIAL / NET A PAYER AVANT IMPÔT » quand il etait
/// egal au montant net social (et sur tout bulletin d'avant la vague 1b), sinon sur « NET À PAYER AVANT IMPÔT », sous
/// « MONTANT NET SOCIAL » (vague 1b). Tous se lisent ; le montant net social, non.
fn forme_net(m: &[char]) -> Option<Decimal> {
    const MARQUES: [&str; 3] = ["NET A PAYER AVANT IMP", "NET À PAYER AVANT IMP", "NET A PAYER AVANT IMPOT SUR LE REVENU"];
    let mut trouvees: Vec<(usize, usize)> = MARQUES
        .iter()
        .flat_map(|marque| {
            let n = marque.chars().count();
            occurrences(m, marque).into_iter().map(move |i| (i, n))
        })
        .collect();
    trouvees.sort_unstable();
    trouvees.into_iter().find_map(|(i, n)| montant_apres(m, i + n + suite_de(m, i + n, |c| !blanc(c)))).and_then(|t| montant_lu(&t))
}

/// `bulletin.numeros_archives` : les numeros que porte un `Bulletins_<Mois>.pdf` ecrit (ou un bulletin rectificatif),
/// avec le net a payer avant impot quand il se lit et les pages de chacun : le recapitulatif ne les emploie pas ; une
/// correction y lit le net deja verse (`produire_regularisations`), le dossier du salarie y prend SES pages.
pub(crate) fn numeros_archives(chemin: &Path) -> Result<Vec<Lu>, Refus> {
    let nom = chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let (pages, _) = fabrique_pdf::lecture::texte(&chemin.to_string_lossy())
        .map_err(|r| Refus { classe: Some("FabriqueRefus"), message: r.0.replace('\n', " ").trim().to_string() })?;
    let texte = pages.join("\n");
    // La page ou commence chaque bulletin : celle ou se trouve sa marque dans le texte joint.
    let mut debuts_de_page = Vec::with_capacity(pages.len());
    let mut position = 0usize;
    for p in &pages {
        debuts_de_page.push(position);
        position += p.len() + 1;
    }
    let page_de = |octet: usize| debuts_de_page.iter().rposition(|d| *d <= octet).unwrap_or(0);
    let commencent: Vec<usize> = texte.match_indices(MARQUE_NUMERO).map(|(i, _)| page_de(i)).collect();
    let mut out = Vec::new();
    for (k, morceau) in texte.split(MARQUE_NUMERO).skip(1).enumerate() {
        let m: Vec<char> = format!("{}{}", MARQUE_NUMERO, morceau).chars().collect();
        let (Some(num), Some(contrat), Some((debut, fin))) = (forme_numero(&m), forme_contrat(&m), forme_periode(&m)) else {
            return Err(Refus {
                classe: Some("ValueError"),
                message: format!(
                    "Bulletin illisible dans {} : il faut le numero de bulletin, le numero de contrat et la periode, il en manque au moins un. Le fichier n'a peut-etre pas ete ecrit par ce moteur.",
                    nom
                ),
            });
        };
        let jour = |t: &str| socle::date::Jour::lire(t).map(|j| j.iso()).map_err(Refus::from);
        let premiere = commencent[k];
        let suivante = commencent.get(k + 1).copied();
        let derniere = match suivante {
            Some(s) if s > premiere => s - 1,
            Some(_) => premiere,
            None => pages.len().saturating_sub(1).max(premiere),
        };
        let partage = suivante == Some(premiere) || (k > 0 && commencent[k - 1] == premiere);
        out.push(Lu {
            bulletin: num,
            contrat,
            debut: jour(&debut)?,
            fin: jour(&fin)?,
            net: forme_net(&m),
            pages: (premiere..=derniere).collect(),
            partage,
        });
    }
    if out.is_empty() {
        return Err(Refus { classe: Some("ValueError"), message: format!("Aucun bulletin lisible dans {}.", nom) });
    }
    Ok(out)
}

/// `texte(x)` d'une valeur de fiche, `""` si la cle manque (`dict.get(cle, "")` puis `%s`).
fn texte_fiche(sal: &Value, cle: &str) -> String {
    match sal.get(cle) {
        None => String::new(),
        Some(v) => socle::valeur::valeur_en_texte(v),
    }
}

/// `operations_documents.numeroter_depuis_archive` : les numeros LUS dans le fichier groupe archive,
/// poses sur les paies. Une paie sans page correspondante fait echouer.
pub(crate) fn numeroter_depuis_archive(ctx: &Contexte, paies: &mut [Paie], groupe: &Path) -> Result<(), Refus> {
    numeroter_et_lire(ctx, paies, groupe).map(|_| ())
}

/// La meme, qui rend aussi ce qui a ete lu pour chaque paie (son rang) et les avertissements (les
/// bulletins de l'archive que la saisie ne porte plus) : le dossier du salarie s'en sert.
pub(crate) fn numeroter_et_lire(ctx: &Contexte, paies: &mut [Paie], groupe: &Path) -> Result<(HashMap<usize, Lu>, Vec<String>), Refus> {
    let mut par_prefixe: HashMap<String, String> = HashMap::new();
    for (cle, sal) in &ctx.salaries {
        if cle.starts_with('_') {
            continue;
        }
        par_prefixe.insert(format!("{}{}", texte_fiche(sal, "initiales_contrat"), texte_fiche(sal, "code_salarie")), cle.clone());
    }
    let cle_de = |p: &Paie| -> Result<String, Refus> {
        p.salarie.get("_cle").map(socle::valeur::valeur_en_texte).ok_or_else(|| Refus { classe: Some("KeyError"), message: socle::fichiers::absent("_cle") })
    };
    let mut par_numero: HashMap<String, String> = HashMap::new();
    for p in paies.iter() {
        if let Some(n) = p.engagement.numero.as_deref().filter(|n| !n.is_empty()) {
            par_numero.insert(n.to_string(), cle_de(p)?);
        }
    }
    // Le rang de premiere insertion garde l'ordre d'un dictionnaire Python.
    let mut archives: HashMap<(String, String, String), (usize, Lu)> = HashMap::new();
    for (rang_page, lu) in numeros_archives(groupe)?.into_iter().enumerate() {
        let prefixe = lu.contrat.split('.').next().unwrap_or("").to_string();
        let cle = par_numero.get(&lu.contrat).filter(|c| !c.is_empty()).or_else(|| par_prefixe.get(&prefixe)).cloned().ok_or_else(|| Refus {
            classe: Some("ErreurOperation"),
            message: format!(
                "Le bulletin {} porte le contrat {}, dont le prefixe ne correspond a aucun salarie de config/salaries.json.",
                lu.bulletin, lu.contrat
            ),
        })?;
        let ident = (cle, lu.debut.clone(), lu.fin.clone());
        let place = archives.get(&ident).map_or(rang_page, |(r, _)| *r);
        archives.insert(ident, (place, lu));
    }
    let mut lus: HashMap<usize, Lu> = HashMap::new();
    for (rang, p) in paies.iter_mut().enumerate() {
        if p.engagement.sans_paie() {
            continue;
        }
        let (debut, fin) = (p.engagement.debut_periode()?, p.engagement.fin_periode()?);
        let ident = (cle_de(p)?, debut.iso(), fin.iso());
        let (_, lu) = archives.remove(&ident).ok_or_else(|| Refus {
            classe: Some("ErreurOperation"),
            message: format!(
                "{}, periode du {} au {} : aucune page correspondante dans {}. La saisie et le fichier archive ne parlent pas du meme mois -- rien n'est reecrit, plutot que d'ecrire un bulletin sous un numero devine.",
                texte_fiche(&p.salarie, "nom"),
                debut.jj_mm_aaaa(),
                fin.jj_mm_aaaa(),
                groupe.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
            ),
        })?;
        p.numero_bulletin = Some(lu.bulletin.clone());
        p.engagement.numero = Some(lu.contrat.clone());
        lus.insert(rang, lu);
    }
    let nom = groupe.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut restes: Vec<(usize, Lu)> = archives.into_values().collect();
    restes.sort_by_key(|(r, _)| *r);
    let avertissements = restes
        .into_iter()
        .map(|(_, r)| {
            format!(
                "Le bulletin {} (contrat {}) est dans {} mais la saisie ne le porte plus : aucune piece n'est ecrite pour lui.",
                r.bulletin, r.contrat, nom
            )
        })
        .collect();
    Ok((lus, avertissements))
}

// ------------------------------------------------------------------ la saisie aux numeros declares

/// `saisie_aux_numeros_declares` : la saisie du mois (le fichier lu, la reponse du calcul), et ses paies,
/// chaque contrat sous le numero qu'il porte VRAIMENT (figes dans la saisie, lus dans l'archive, ou ceux
/// que la production prendrait).
pub(crate) fn paies_aux_numeros_declares(o: &HashMap<String, String>, ctx: &Contexte, mois: &str) -> Result<(Value, Value, Vec<Paie>), Refus> {
    paies_aux_numeros_declares_avec(o, ctx, mois, None, true)
}

/// La meme, pour un ENVOI : `charger` du Python RESERVE (en memoire du `Contexte`) les numeros qu'il propose, et la
/// route d'envoi les ecrit plus loin (`reserver_numeros_envoyes`) ; `reserves` les garde. `cache` : `false` calcule
/// sans toucher a `paies/AAAA-MM.calcul.json` (le `charger` du Python ne l'ecrit jamais).
pub(crate) fn paies_aux_numeros_declares_avec(
    o: &HashMap<String, String>,
    ctx: &Contexte,
    mois: &str,
    reserves: Option<&mut crate::produire_mois::Reserves>,
    cache: bool,
) -> Result<(Value, Value, Vec<Paie>), Refus> {
    let chemin = ctx.racine.join("paies").join(format!("{}.json", mois));
    let data = engagement::lire_saisie(&chemin)?;
    let rep = if cache {
        paie_du_mois(o, ctx, &data)?
    } else {
        crate::service::calculer_avec(ctx, &json!({ "saisie": data })).map_err(|m| crate::routes_ouverture::refus_du_calcul(ctx, &data, m))?
    };
    let engs = engagements_calcules(ctx, &data, &rep)?;
    let paies_json = rep.get("paies").and_then(Value::as_array).ok_or("reponse du calcul illisible : `paies`")?;
    let mut paies = Vec::new();
    for (e, v) in engs.into_iter().zip(paies_json) {
        paies.push(paie_de_json(ctx, e, v)?);
    }
    if data.get("numeros_figes").is_some_and(vrai) {
        // Un contrat ajoute apres la production n'a pas de numero fige : il prend celui que le compteur propose,
        // comme un mois pas encore produit. Le Python l'imprimait « n° None » (defaut non reproduit) ; les autres
        // gardent leurs numeros, bulletin compris.
        let sans: Vec<usize> = (0..paies.len()).filter(|&i| paies[i].engagement.numero.as_deref().map_or(true, str::is_empty)).collect();
        if !sans.is_empty() {
            let mois_saisie = data.get("mois").map(socle::date::en_texte).ok_or_else(|| socle::fichiers::absent("mois"))?;
            let annee = int_py(&mois_saisie.chars().take(4).collect::<String>()).ok_or_else(|| format!("mois illisible : {}", repr(&mois_saisie)))?;
            let mut neufs: Vec<Paie> = sans.iter().map(|&i| paies[i].clone()).collect();
            match reserves {
                Some(r) => r.numeroter(ctx, &mut neufs, annee as i32)?,
                None => numeroter(ctx, &mut neufs, annee as i32)?,
            }
            for (i, p) in sans.into_iter().zip(neufs) {
                paies[i] = p;
            }
        }
        return Ok((data, rep, paies));
    }
    let groupe = ctx.racine.join("sorties").join(mois).join(format!("Bulletins_{}.pdf", crate::envoi::etiquette(mois)?));
    if groupe.exists() {
        numeroter_depuis_archive(ctx, &mut paies, &groupe)?;
    } else {
        let mois_saisie = data.get("mois").map(socle::date::en_texte).ok_or_else(|| socle::fichiers::absent("mois"))?;
        let annee = int_py(&mois_saisie.chars().take(4).collect::<String>()).ok_or_else(|| format!("mois illisible : {}", repr(&mois_saisie)))?;
        match reserves {
            Some(r) => r.numeroter(ctx, &mut paies, annee as i32)?,
            None => numeroter(ctx, &mut paies, annee as i32)?,
        }
    }
    Ok((data, rep, paies))
}

/// La meme, sans le detail du refus : le moindre refus, quel qu'il soit, rend `Err`.
fn saisie_aux_numeros_declares(o: &HashMap<String, String>, ctx: &Contexte, mois: &str) -> Result<Vec<Paie>, String> {
    paies_aux_numeros_declares(o, ctx, mois).map(|t| t.2).map_err(|r| r.message)
}

// ------------------------------------------------------------------ recapitulatif_dsn

fn requis(v: &Value, cle: &str) -> Result<String, String> {
    v.get(cle).map(socle::valeur::valeur_en_texte).ok_or_else(|| socle::fichiers::absent(cle))
}

/// `recapitulatif_dsn` : ce que la DSN declare, contrat par contrat, comparé aux contrats saisis.
fn recapitulatif_dsn(o: &HashMap<String, String>, ctx: &Contexte, octets: &[u8], mois: &str, nic: &str) -> Result<Value, String> {
    let sals = lire_recap(octets)?;
    let (mut compare, liste) = match saisie_aux_numeros_declares(o, ctx, mois) {
        Ok(l) => (true, l),
        Err(_) => (false, Vec::new()),
    };
    // `paies` : numero -> paie, dans l'ordre ou les numeros sont apparus
    let mut ordre: Vec<String> = Vec::new();
    let mut par_numero: HashMap<String, usize> = HashMap::new();
    for (i, p) in liste.iter().enumerate() {
        if !nic.is_empty() {
            match droit::bareme::etablissement_du_salarie(&ctx.entreprise, &p.salarie) {
                Ok(e) if e == nic => {}
                Ok(_) => continue,
                Err(_) => {
                    compare = false;
                    break;
                }
            }
        }
        let n = p.engagement.numero.clone().unwrap_or_else(|| "None".to_string());
        if !par_numero.contains_key(&n) {
            ordre.push(n.clone());
        }
        par_numero.insert(n, i);
    }
    let mut vus: HashSet<String> = HashSet::new();
    let mut salaries_json = Vec::new();
    let (mut n_contrats, mut n_ecarts, mut n_corrections) = (0usize, 0usize, 0usize);
    for sal in &sals {
        let mut contrats_json = Vec::new();
        for c in &sal.contrats {
            vus.insert(c.numero.clone());
            let p = par_numero.get(&c.numero).map(|&i| &liste[i]);
            let mut ecarts: Vec<String> = Vec::new();
            let brut = c.brut_texte();
            // UNE CORRECTION D'UN MOIS DEJA DECLARE (21/10/2026) : un contrat que la paie du mois ne porte pas, et dont la
            // fin (40.010) precede le mois, n'est dans le fichier que pour sa regularisation (« Corriger ce mois »,
            // `dsn::generer` et ses `regs`). Ce n'est pas un ecart avec la paie : il se dit pour ce qu'il est.
            let correction = p.is_none() && !c.fin.is_empty() && c.fin.as_str() < format!("{}-01", mois).as_str();
            match p {
                None if correction => {}
                None => {
                    if compare {
                        ecarts.push("aucune paie du mois ne porte ce contrat".to_string());
                    }
                }
                Some(p) => {
                    let e = &p.engagement;
                    let mut attendu: HashMap<&str, String> = HashMap::new();
                    // `paie_rust._euros` : le brut de la paie est quantifie a deux decimales.
                    attendu.insert("brut", q2(p.brut));
                    for k in ["jours", "cachets", "heures"] {
                        attendu.insert(k, "0".to_string());
                    }
                    let nat = e.nat()?;
                    for u in nat.unites_activite() {
                        let genre_ok = match u.get("type") {
                            None => true,
                            Some(Value::String(s)) => s == "01",
                            Some(_) => false,
                        };
                        if !genre_ok {
                            continue;
                        }
                        let code = u.get("code").ok_or_else(|| socle::fichiers::absent("code"))?;
                        let Some(cle) = (match code.as_str() {
                            Some("10") | Some("35") => Some("heures"),
                            Some("12") => Some("jours"),
                            Some("37") => Some("cachets"),
                            _ => None,
                        }) else {
                            continue;
                        };
                        let mesure = u.get("mesure").and_then(Value::as_str).ok_or_else(|| socle::fichiers::absent("mesure"))?;
                        let valeur = match mesure {
                            "jours" => e.jours as f64,
                            "cachets" => e.cachets as f64,
                            "heures" => e.heures.unwrap_or(0.0),
                            "heures_travaillees" => flottant(p.heures_travaillees),
                            autre => return Err(format!("mesure inconnue : {}", repr(autre))),
                        };
                        let avant: f64 = attendu[cle].parse().unwrap_or(0.0);
                        let somme = avant + valeur;
                        let t = format!("{}", if somme == 0.0 { 0.0 } else { somme });
                        let d = decimal_py(&t).ok_or_else(|| socle::num::illisible(&t))?;
                        attendu.insert(cle, norm(d));
                    }
                    for cle in ["brut", "jours", "cachets", "heures"] {
                        let fichier = if cle == "brut" { brut.clone() } else { c.mesure_texte(cle) };
                        let a = decimal_py(&attendu[cle]).ok_or_else(|| socle::num::illisible(&attendu[cle]))?;
                        let f = decimal_py(&fichier).ok_or_else(|| socle::num::illisible(&fichier))?;
                        if norm(f) != norm(a) {
                            ecarts.push(format!("{} : la paie dit {}, le fichier {}", cle, attendu[cle], fichier));
                        }
                    }
                    let emploi = p.salarie.get("emploi");
                    let attendu_emploi: Option<&str> = match emploi {
                        None | Some(Value::Null) => Some(""),
                        Some(Value::String(s)) => Some(s.as_str()),
                        Some(v) if !vrai(v) => Some(""),
                        Some(_) => None,
                    };
                    if attendu_emploi != Some(c.emploi.as_str()) {
                        let dit = match emploi {
                            None | Some(Value::Null) => "None".to_string(),
                            Some(v) => socle::valeur::valeur_en_texte(v),
                        };
                        ecarts.push(format!("emploi : la paie dit {}, le fichier {}", dit, c.emploi));
                    }
                }
            }
            if correction {
                n_corrections += 1;
            } else {
                n_contrats += 1;
            }
            if !ecarts.is_empty() {
                n_ecarts += 1;
            }
            contrats_json.push(json!({
                "numero": c.numero, "debut": c.debut, "fin": c.fin, "emploi": c.emploi,
                "nature": c.nature, "objet": c.objet, "brut": brut,
                "jours": c.mesure_texte("jours"), "cachets": c.mesure_texte("cachets"),
                "heures": c.mesure_texte("heures"),
                "fin_declaree": c.fin_declaree, "motif_fin": c.motif_fin, "remunere": c.remunere,
                "paie": p.is_some(), "ecarts": ecarts, "correction": correction,
            }));
        }
        let versements: Vec<Value> = sal
            .versements
            .iter()
            .map(|v| json!({"date": v.date, "net_verse": v.net_verse, "net_fiscal": v.net_fiscal, "taux_pas": v.taux_pas, "montant_pas": v.montant_pas}))
            .collect();
        salaries_json.push(json!({"nir": sal.nir, "nom": sal.nom, "prenoms": sal.prenoms, "contrats": contrats_json, "versements": versements}));
    }
    let mut absents = Vec::new();
    for n in &ordre {
        if vus.contains(n) {
            continue;
        }
        let p = &liste[par_numero[n]];
        let nom = requis(&p.salarie, "nom")?;
        let prenom = match p.salarie.get("prenom_affichage") {
            Some(v) if vrai(v) => socle::valeur::valeur_en_texte(v),
            _ => requis(&p.salarie, "prenom")?,
        };
        absents.push(json!({
            "salarie": p.salarie.get("_cle").cloned().ok_or_else(|| socle::fichiers::absent("_cle"))?,
            "nom": format!("{} {}", nom, prenom),
            "numero": n,
            "debut": p.engagement.debut.iso(),
            "fin": p.engagement.fin.map_or(String::new(), |f| f.iso()),
        }));
    }
    let conforme = compare && absents.is_empty() && n_ecarts == 0;
    Ok(json!({
        "salaries": salaries_json, "absents": absents,
        "contrats": n_contrats, "corrections": n_corrections, "contrats_saisis": par_numero.len(),
        "compare": compare, "conforme": conforme,
    }))
}

// ------------------------------------------------------------------ l'apercu

/// `_remplacement_du_fichier` : l'annule-et-remplace du mois, ou `null` s'il ne se lit pas.
fn remplacement_du_fichier(m: &mut Moteur, a: &Apercu, mois: &str) -> R<Value> {
    let nic = a.champ("etablissement");
    let nic = if nic.is_empty() { Value::Null } else { json!(nic) };
    let Ok(mut e) = m.etat_remplacement(mois, &nic) else { return Ok(Value::Null) };
    let genre = a.champ("type_declaration");
    let remplace = if (genre == "03" || genre == "05") && vrai(&e["deposee"]) {
        let ordre = a.champ("ordre");
        let n = if ordre.is_empty() {
            0
        } else {
            int_py(ordre).ok_or_else(|| erreur("ValueError", format!("nombre entier illisible : {}", repr(ordre))))?
        };
        n > i128::from(e["dernier_ordre"].as_i64().unwrap_or(0))
    } else {
        false
    };
    e["fichier_remplace"] = json!(remplace);
    Ok(e)
}

/// `apercu_depot_dsn` : ce qui partirait, en clair, sans rien envoyer ni demander.
pub(crate) fn apercu_depot_dsn(o: &HashMap<String, String>, ctx: &Contexte, fichier: &Value) -> R<Value> {
    let chemin = resoudre(&ctx.racine, fichier)?;
    let contenu = lire_octets(&chemin)?;
    let a = depot::lire(&contenu);
    let mut m = Moteur::nouveau(o, ctx);
    let lignes = m.lignes()?;
    let empreinte = crate::journal::empreinte(&contenu);
    let deja = etats::deja_depose(&lignes, "dsn", &empreinte)?;
    let nom = nom_du_chemin(&chemin);
    let signalement = a.est_signalement();
    let mois = a.mois_lisible();
    let remplacement = if signalement { Value::Null } else { remplacement_du_fichier(&mut m, &a, &mois)? };
    let recapitulatif = if signalement {
        Value::Null
    } else {
        match recapitulatif_dsn(o, ctx, &contenu, &mois, a.champ("etablissement")) {
            Ok(v) => v,
            Err(message) => json!({ "erreur": message }),
        }
    };
    let repetition = etats::repetition(&lignes, "dsn", &empreinte, &nom)?;
    // LES ORDRES DE PAIEMENT EN CLAIR (ticket #223) : a qui, pour quoi, combien, pour quand.
    let chemin_orgs = ctx.racine.join("config").join("organismes.json");
    let organismes = if chemin_orgs.is_file() { socle::fichiers::lire_json(&chemin_orgs)? } else { Value::Null };
    let ordres = crate::ordres_paiement::decrire(&texte_latin1(&contenu), &mois, &organismes, ctx.echeance_dsn()?)?;
    let genre = a.champ("type_declaration");
    let nature = a.champ("nature");
    Ok(json!({
        "fichier": chemin, "nom": nom,
        "mois": mois, "nature_envoi": a.champ("nature_envoi"),
        "reel": a.reel(),
        "signalement": signalement,
        "nature": if nature.is_empty() { depot::MENSUELLE } else { nature },
        "libelle_nature": a.nature_lisible(),
        "evenement": if signalement { a.evenement_lisible() } else { String::new() },
        "type_declaration": genre,
        "libelle_type": TYPES.iter().find(|(c, _)| *c == genre).map_or("?", |(_, l)| *l),
        "ordre": a.champ("ordre"), "fraction": a.champ("fraction"),
        "remplace": a.champ("remplace"), "constitution": a.champ("constitution"),
        "siren": a.champ("siren"), "salaries": a.salaries, "lignes": a.lignes,
        "pas_versements": a.pas_versements, "pas_bareme": a.pas_bareme,
        "ordres": ordres,
        "deja_depose": deja,
        "remplacement": remplacement,
        "recapitulatif": recapitulatif,
        "repetition": repetition,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_comme_python() {
        assert_eq!(decimal_py(" 12.50 ").unwrap().to_string(), "12.50");
        assert_eq!(decimal_py("1_000").unwrap().to_string(), "1000");
        assert_eq!(decimal_py(".5").unwrap().to_string(), "0.5");
        assert_eq!(decimal_py("5.").unwrap().to_string(), "5");
        assert_eq!(decimal_py("-1E+2").unwrap().to_string(), "-100");
        for mauvais in ["", "-", "abc", "1 2", "1__0", "_1", "1.2.3", "1e", "12,5"] {
            assert!(decimal_py(mauvais).is_none(), "{}", mauvais);
        }
    }

    #[test]
    fn normaliser_et_arrondir() {
        assert_eq!(norm(Decimal::from_str("100.00").unwrap()), "100");
        assert_eq!(norm(Decimal::from_str("0.00").unwrap()), "0");
        assert_eq!(q2(Decimal::from_str("2.345").unwrap()), "2.34");
        assert_eq!(q2(Decimal::from_str("2.355").unwrap()), "2.36");
        assert_eq!(q2(Decimal::from_str("1500").unwrap()), "1500.00");
    }

    #[test]
    fn les_lignes_se_comptent_comme_splitlines() {
        assert_eq!(lignes_py("a\r\nb\rc\n\nd\u{85}e\u{2028}f\n"), ["a", "b", "c", "", "d", "e", "f"]);
        assert_eq!(lignes_py("a\n"), ["a"]);
        assert!(lignes_py("").is_empty());
    }

    fn chars(t: &str) -> Vec<char> {
        t.chars().collect()
    }

    #[test]
    fn formes_du_bulletin() {
        let m = chars("Bulletin de paie n\u{b0} 25/2026\nContrat n\u{b0} FA1.26.13 \u{2014} CDDU\nP\u{e9}riode : du 01/08/2026 au 31/08/2026");
        assert_eq!(forme_numero(&m).as_deref(), Some("25/2026"));
        assert_eq!(forme_contrat(&m).as_deref(), Some("FA1.26.13"));
        assert_eq!(forme_periode(&m), Some(("01/08/2026".to_string(), "31/08/2026".to_string())));
        let serre = chars("Bulletin de paie n\u{b0}7/2026 Contrat n\u{b0}X.1\u{2014}Periode du 1/8 au 2/8");
        assert_eq!(forme_numero(&serre).as_deref(), Some("7/2026"));
        // `\S*` rend le moins possible : sans espace, c'est `1` que Python lit aussi.
        assert_eq!(forme_contrat(&serre).as_deref(), Some("1"));
        assert_eq!(forme_periode(&serre), None);
    }

    #[test]
    fn recapitulatif_lu_dans_le_fichier() {
        let f = "S21.G00.30.001,'1850'\nS21.G00.30.002,'FAVART'\nS21.G00.40.001,'01082026'\nS21.G00.40.009,'FA1.26.13'\n\
                 S21.G00.51.001,'01082026'\nS21.G00.51.010,'FA1.26.13'\nS21.G00.51.011,'001'\nS21.G00.51.013,'150.5'\nS21.G00.51.013,'49.5'\n\
                 S21.G00.53.001,'01'\nS21.G00.53.002,'2'\nS21.G00.53.003,'37'\nS21.G00.62.001,'31082026'\n";
        let s = lire_recap(f.as_bytes()).unwrap();
        assert_eq!(s.len(), 1);
        let c = &s[0].contrats[0];
        assert_eq!((c.brut_texte().as_str(), c.mesure_texte("cachets").as_str()), ("200.00", "2"));
        assert_eq!((c.debut.as_str(), c.fin_declaree.as_str(), c.remunere), ("2026-08-01", "2026-08-31", true));
        assert_eq!(lire_recap(b"S21.G00.30.001,'1'\nS21.G00.40.001,'01082026'\nS21.G00.40.009,'A'\nS21.G00.51.001,'x'\nS21.G00.51.010,'A'\nS21.G00.51.013,'12,5'\n").err(), Some(socle::num::illisible("12,5")));
    }
}
