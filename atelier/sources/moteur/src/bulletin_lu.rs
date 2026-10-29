//! Le lecteur de bulletins (lot B du plan d'import, `prive/journal/PLAN-IMPORT-BULLETINS.md`) : une page de PDF
//! devient un `BulletinLu`, TEL QU'IL EST. Le bulletin fait foi (decide) : rien ne se recalcule, chaque montant est
//! un `Decimal` lu sur la page (« 1 421,13 » -> 1421.13).
//!
//! La page se lit par `fabrique_pdf::inspection::inspecter` : chaque mot et sa boite. Le texte dans l'ordre du flux
//! ne suffit pas : le tableau des cotisations a cinq colonnes, et seule la position en x dit dans laquelle tombe un
//! nombre.
//!
//! LA MISE EN PAGE N'EST PAS DANS LE CODE : elle est dans la table livree `bulletins-gabarits-base.json` (sa cle
//! `_forme` la decrit). Un gabarit, c'est une signature (les repères qui le reconnaissent), les repères de l'en-tete
//! et du pied, deux cadres (l'entreprise, le salarie), les colonnes du tableau et les grilles du pied. Un troisieme
//! logiciel s'ajoute dans la table, pas ici.
//!
//! RIEN NE SE DEVINE (regle 1). Une page :
//! - sans un mot (lettres en traces, scan : pas d'OCR) -> refus `sans texte` ;
//! - qu'aucun gabarit ne reconnait -> refus `gabarit inconnu` (deux qui la reconnaissent : refus aussi) ;
//! - ou ne se lisent pas ce qui identifie le bulletin (salarie, periode, brut, net a payer) -> refus `incomplet`.
//!
//! Le reste est facultatif. Deux controles, SANS CORRECTION : le brut vaut la somme des lignes soumises, le net a
//! payer se retrouve (net avant impot = brut - cotisations salariales + lignes non soumises ; net a payer = net
//! avant impot - impot preleve). Un ecart va dans `avertissements`, le bulletin est rendu quand meme. Un libelle de
//! ligne absent de la table est signale de meme, et la ligne gardee telle quelle (decide, plan §4 : pas de refus).
//!
//! LES LIGNES ET LES COMPTEURS (lot C). La table dit, par gabarit, la rubrique du catalogue que porte chaque libelle
//! de ligne (`null` : connu, sans rubrique sure). `compteurs` en tire les cachets, heures, heures d'enseignement et
//! jours d'un bulletin comme pour une saisie (le catalogue dit l'unite, le forfait de 12 h d'un cachet, ce qui compte
//! pour l'ouverture des droits), et les confronte aux cumuls que le bulletin imprime : un desaccord est un
//! avertissement, et le bulletin l'emporte sur ce qu'il compte lui-meme.

use std::path::Path;
use std::str::FromStr;

use serde_json::{json, Map, Value};

use socle::date::Jour;
use socle::num::{Decimal, Resultat};

/// La table des gabarits, dans `logiciel/donnees/`.
pub const FICHIER_TABLE: &str = "bulletins-gabarits-base.json";

/// Un mot plus loin que cela du precedent ne continue pas un repère (en points).
const ECART_MOTS: f64 = 15.0;

/// Deux morceaux d'un nombre (« 1 » et « 421,13 ») separes de moins que cela forment un seul nombre (l'espace des
/// milliers grandit avec le corps du texte : 2 points en 9, 4 en 14).
const ECART_MILLIERS: f64 = 5.0;

/// Un nombre « sous » un repère : au plus a cette distance verticale.
const DESSOUS_MAX: f64 = 40.0;

/// Une grille du pied : ses lignes sont au plus a cette distance sous son en-tete.
const GRILLE_HAUTEUR: f64 = 60.0;

/// Un nombre d'une grille va a la colonne dont l'en-tete est le plus proche, a moins de cette distance.
const GRILLE_ECART: f64 = 60.0;

/// Une periode de dates travaillees plus longue que cela n'est pas depliee en jours.
const INTERVALLE_MAX: i64 = 366;

// --------------------------------------------------------------------------------------------- le texte

/// Les jetons d'un texte pour comparer : minuscules, sans accents, apostrophe typographique lue droite, tiret lu
/// comme un espace, deux-points colle retire (un « : » isole reste un jeton).
pub fn jetons(texte: &str) -> Vec<String> {
    let t = texte.replace(['\u{2019}', '\u{2018}'], "'");
    socle::texte::ascii_nfkd(&t)
        .to_lowercase()
        .replace('-', " ")
        .split_whitespace()
        .filter_map(|m| {
            let m = if m == ":" { m } else { m.trim_end_matches(':') };
            (!m.is_empty()).then(|| m.to_string())
        })
        .collect()
}

fn canon(texte: &str) -> String {
    jetons(texte).join(" ")
}

/// Un nombre ecrit sur un bulletin : virgule decimale (les points et espaces sont alors des milliers), ou point
/// decimal s'il n'y a pas de virgule (« 0.000 % » du PAS de CoolPaie). Le signe moins en tete.
pub fn lire_nombre(texte: &str) -> Option<Decimal> {
    let t = texte.trim();
    let (negatif, t) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t),
    };
    if !t.starts_with(|c: char| c.is_ascii_digit()) || !t.chars().all(|c| c.is_ascii_digit() || matches!(c, ',' | '.' | ' ')) {
        return None;
    }
    let s = if t.contains(',') { t.replace(['.', ' '], "").replace(',', ".") } else { t.replace(' ', "") };
    if s.matches('.').count() > 1 || s.ends_with('.') {
        return None;
    }
    Decimal::from_str(&s).ok().map(|d| if negatif { -d } else { d })
}

fn ressemble_nombre(t: &str) -> bool {
    let t = t.strip_prefix('-').unwrap_or(t);
    t.starts_with(|c: char| c.is_ascii_digit()) && t.chars().all(|c| c.is_ascii_digit() || c == ',' || c == '.')
}

/// « 14/01/2026 » ou « 02/04/26 » (une virgule ou un point final tolere). Une annee en deux chiffres est de ce
/// siecle : un bulletin de paie lu ici est posterieur a l'an 2000 (la DSN n'existait pas avant).
pub fn lire_date(texte: &str) -> Option<Jour> {
    let t = texte.trim_end_matches([',', '.', ';']);
    let p: Vec<&str> = t.split('/').collect();
    if p.len() != 3 || p.iter().any(|x| x.is_empty() || !x.chars().all(|c| c.is_ascii_digit())) {
        return None;
    }
    let an: i32 = match p[2].len() {
        4 => p[2].parse().ok()?,
        2 => 2000 + p[2].parse::<i32>().ok()?,
        _ => return None,
    };
    if p[0].len() > 2 || p[1].len() > 2 {
        return None;
    }
    Jour::nouveau(an, p[1].parse().ok()?, p[0].parse().ok()?)
}

// --------------------------------------------------------------------------------------------- la page

#[derive(Clone, Debug)]
struct Mot {
    texte: String,
    jetons: Vec<String>,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

impl Mot {
    fn cy(&self) -> f64 {
        (self.y0 + self.y1) / 2.0
    }
}

/// Une boite : celle d'un repère trouve (union de ses mots), ou d'un nombre.
#[derive(Clone, Copy, Debug)]
struct Boite {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

impl Boite {
    fn de(m: &Mot) -> Boite {
        Boite { x0: m.x0, y0: m.y0, x1: m.x1, y1: m.y1 }
    }
    fn union(self, m: &Mot) -> Boite {
        Boite { x0: self.x0.min(m.x0), y0: self.y0.min(m.y0), x1: self.x1.max(m.x1), y1: self.y1.max(m.y1) }
    }
    fn cy(&self) -> f64 {
        (self.y0 + self.y1) / 2.0
    }
    fn cx(&self) -> f64 {
        (self.x0 + self.x1) / 2.0
    }
}

/// Un nombre de la page : ses morceaux recolles (« 1 » + « 421,13 »).
#[derive(Clone, Debug)]
struct Nombre {
    valeur: Decimal,
    boite: Boite,
}

struct Page {
    /// Tries par milieu vertical, puis par x.
    mots: Vec<Mot>,
    nombres: Vec<Nombre>,
    /// Pour chaque mot, son voisin de droite sur le meme rang (a moins de `ECART_MOTS`).
    voisins: Vec<Option<usize>>,
}

impl Page {
    fn nouvelle(mut mots: Vec<Mot>, tolerance: f64) -> Page {
        mots.sort_by(|a, b| a.cy().total_cmp(&b.cy()).then(a.x0.total_cmp(&b.x0)));
        let voisins = (0..mots.len())
            .map(|i| {
                let m = &mots[i];
                (0..mots.len())
                    .filter(|&j| j != i && (mots[j].cy() - m.cy()).abs() <= tolerance && mots[j].x0 >= m.x1 - 1.0 && mots[j].x0 - m.x1 <= ECART_MOTS)
                    .min_by(|&a, &b| mots[a].x0.total_cmp(&mots[b].x0))
            })
            .collect();
        let nombres = nombres_de(&mots);
        Page { mots, nombres, voisins }
    }

    /// Le repère `phrase` commence-t-il au mot `i` ? Sa boite, et le dernier mot.
    fn repere_en(&self, i: usize, phrase: &[String]) -> Option<(Boite, usize)> {
        if phrase.is_empty() || self.mots[i].jetons.first() != Some(&phrase[0]) {
            return None;
        }
        let mut boite = Boite::de(&self.mots[i]);
        let mut mot = i;
        let mut k = 0usize; // jeton dans le mot courant
        for attendu in phrase {
            while k >= self.mots[mot].jetons.len() {
                mot = self.voisins[mot]?;
                k = 0;
                boite = boite.union(&self.mots[mot]);
            }
            if &self.mots[mot].jetons[k] != attendu {
                return None;
            }
            k += 1;
        }
        Some((boite, mot))
    }

    /// La premiere occurrence (de haut en bas, puis de gauche a droite) d'une des phrases, parmi les mots que
    /// `garder` retient.
    fn trouver(&self, phrases: &[Vec<String>], garder: &dyn Fn(&Mot) -> bool) -> Option<(Boite, usize)> {
        let mut ordre: Vec<usize> = (0..self.mots.len()).filter(|&i| garder(&self.mots[i])).collect();
        ordre.sort_by(|&a, &b| self.mots[a].y0.total_cmp(&self.mots[b].y0).then(self.mots[a].x0.total_cmp(&self.mots[b].x0)));
        ordre.into_iter().find_map(|i| phrases.iter().find_map(|p| self.repere_en(i, p)))
    }

    /// Le mot est-il un morceau d'un nombre dont le milieu est a droite de `limite` (le « 1 » de « 1 421,13 ») ?
    fn dans_un_nombre(&self, m: &Mot, limite: f64) -> bool {
        self.nombres.iter().any(|n| {
            let b = n.boite;
            b.cx() > limite && m.x0 >= b.x0 - 0.01 && m.x1 <= b.x1 + 0.01 && (m.cy() - b.cy()).abs() < 1.0
        })
    }

    fn present(&self, phrase: &[String]) -> bool {
        (0..self.mots.len()).any(|i| self.repere_en(i, phrase).is_some())
    }
}

/// Les nombres de la page : un mot qui en a la forme, recolle a ses voisins de milliers (« 1 » + « 421,13 »,
/// separes de moins de `ECART_MILLIERS`).
fn nombres_de(mots: &[Mot]) -> Vec<Nombre> {
    let mut candidats: Vec<&Mot> = mots.iter().filter(|m| ressemble_nombre(&m.texte)).collect();
    candidats.sort_by(|a, b| a.cy().total_cmp(&b.cy()).then(a.x0.total_cmp(&b.x0)));
    let milliers = |t: &str| t.len() == 3 && t.chars().all(|c| c.is_ascii_digit());
    let tete = |t: &str| {
        let t = t.strip_prefix('-').unwrap_or(t);
        let mut groupes = t.split(' ');
        let premier = groupes.next().unwrap_or("");
        (1..=3).contains(&premier.len()) && premier.chars().all(|c| c.is_ascii_digit()) && groupes.all(milliers)
    };
    let mut sortie: Vec<(String, Boite)> = Vec::new();
    for m in candidats {
        if let Some((texte, boite)) = sortie.last_mut() {
            let suite = m.texte.split([',', '.']).next().unwrap_or("");
            let colle = (m.cy() - boite.cy()).abs() <= 1.5 && m.x0 >= boite.x1 - 0.5 && m.x0 - boite.x1 < ECART_MILLIERS;
            if colle && tete(texte) && milliers(suite) {
                texte.push(' ');
                texte.push_str(&m.texte);
                *boite = boite.union(m);
                continue;
            }
        }
        sortie.push((m.texte.clone(), Boite::de(m)));
    }
    sortie.into_iter().filter_map(|(t, b)| lire_nombre(&t).map(|valeur| Nombre { valeur, boite: b })).collect()
}

/// Des mots en rangs (milieu vertical a moins de `tol` du premier mot du rang), chaque rang de gauche a droite.
fn en_rangs<'a>(mut mots: Vec<&'a Mot>, tol: f64) -> Vec<Vec<&'a Mot>> {
    mots.sort_by(|a, b| a.cy().total_cmp(&b.cy()));
    let mut rangs: Vec<Vec<&Mot>> = Vec::new();
    for m in mots {
        match rangs.last_mut() {
            Some(r) if (m.cy() - r[0].cy()).abs() <= tol => r.push(m),
            _ => rangs.push(vec![m]),
        }
    }
    for r in rangs.iter_mut() {
        r.sort_by(|a, b| a.x0.total_cmp(&b.x0));
    }
    rangs
}

/// Une ligne de paie qui ecrit son taux dans son libelle, apres `mot` (« Cachets à 150,00 ») : le libelle sans lui, et
/// le taux. Sans nombre apres le dernier `mot`, le libelle tel quel.
fn taux_du_libelle(libelle: &str, mot: &str) -> (String, Option<Decimal>) {
    let mots: Vec<&str> = libelle.split_whitespace().collect();
    for k in (0..mots.len().saturating_sub(1)).rev() {
        if canon(mots[k]) == mot {
            if let Some(t) = lire_nombre(mots[k + 1]) {
                return (mots[..k].join(" "), Some(t));
            }
        }
    }
    (libelle.to_string(), None)
}

fn texte_de(mots: &[&Mot]) -> String {
    mots.iter().map(|m| m.texte.as_str()).collect::<Vec<_>>().join(" ")
}

fn milieu(rang: &[&Mot]) -> f64 {
    rang.iter().map(|m| m.cy()).sum::<f64>() / rang.len().max(1) as f64
}

// --------------------------------------------------------------------------------------------- la table

#[derive(Clone, Copy, Debug, PartialEq)]
enum Sorte {
    Texte,
    Compact,
    Date,
    Periode,
    Intervalles,
    Nombre,
}

/// Le type de chaque champ est fixe par son nom : la table dit ou il est, pas ce qu'il est.
const CHAMPS: [(&str, Sorte); 22] = [
    ("numero", Sorte::Texte),
    ("numero_objet", Sorte::Compact),
    ("spectacle", Sorte::Texte),
    ("analytique", Sorte::Texte),
    ("siret", Sorte::Compact),
    ("naf", Sorte::Texte),
    ("urssaf", Sorte::Compact),
    ("convention", Sorte::Texte),
    ("licence", Sorte::Texte),
    ("naissance", Sorte::Date),
    ("nir", Sorte::Compact),
    ("code", Sorte::Texte),
    ("emploi", Sorte::Texte),
    ("statut", Sorte::Texte),
    ("abattement", Sorte::Nombre),
    ("conges_spectacles", Sorte::Compact),
    ("periode", Sorte::Periode),
    ("dates_travaillees", Sorte::Intervalles),
    ("regle_le", Sorte::Date),
    ("mode_reglement", Sorte::Texte),
    ("net_a_payer", Sorte::Nombre),
    ("cout_employeur", Sorte::Nombre),
];

const COLONNES_CORPS: [&str; 5] = ["base", "taux_employeur", "montant_employeur", "taux_salarie", "montant_salarie"];

#[derive(Clone, Copy, Debug, PartialEq)]
enum Section {
    Entete,
    Pied,
    Page,
}

#[derive(Clone, Debug)]
struct Champ {
    nom: String,
    sorte: Sorte,
    reperes: Vec<Vec<String>>,
    dessous: bool,
    jusqu_a: f64,
    lignes: usize,
    apres: Option<String>,
    /// Le mot avant lequel la valeur s'arrete, sur un rang qui porte un autre champ apres elle.
    avant: Option<String>,
    section: Section,
}

#[derive(Clone, Debug)]
struct Cadre {
    x: (f64, f64),
    y: (f64, f64),
}

#[derive(Clone, Debug)]
struct Resume {
    libelles: Vec<String>,
    colonne: String,
}

#[derive(Clone, Debug)]
struct Corps {
    libelle_jusqu_a: f64,
    quantite_jusqu_a: f64,
    colonnes: Vec<(String, f64, f64)>,
    taux: String,
    montant: String,
    brut: Resume,
    total: Vec<String>,
    net_social: Resume,
    net_avant_impot: Resume,
    /// Le mot apres lequel une ligne de paie ecrit son taux dans son libelle (« 2 Cachets à 150,00 ») ; `None` : le taux
    /// est dans sa colonne.
    taux_dans_le_libelle: Option<String>,
    /// La derniere ligne de paie porte-t-elle le libelle du brut apres le sien (« 2 Cachets à 150,00 — MONTANT BRUT ») ?
    /// Ce rang est alors la ligne et le brut, son montant celui des deux.
    brut_en_fin_de_ligne: bool,
}

#[derive(Clone, Debug)]
enum Cible {
    Pas,
    /// `net_imposable`, lu dans cette colonne.
    NetImposable(String),
    Cumul(String),
}

#[derive(Clone, Debug)]
struct Grille {
    colonnes: Vec<(String, Vec<String>, bool)>,
    lignes: Vec<(Cible, Vec<String>)>,
}

/// Un libelle de ligne de paie que la table connait (lot C).
#[derive(Clone, Debug, PartialEq)]
pub struct LigneTable {
    /// Tel que la table l'ecrit.
    pub libelle: String,
    /// Canonique (`jetons`), pour comparer.
    canon: String,
    /// Le code du catalogue ; `None` : connu, sans rubrique sure.
    pub rubrique: Option<String>,
    /// Les heures d'une unite, quand le libelle les dit (« 4h ») et que la rubrique les fait saisir.
    pub heures: Option<Decimal>,
}

/// Un gabarit de la table, verifie a la lecture.
#[derive(Clone, Debug)]
pub struct Gabarit {
    pub nom: String,
    pub libelle: String,
    signature: Vec<Vec<String>>,
    tolerance: f64,
    tableau: Vec<Vec<String>>,
    entreprise: Cadre,
    salarie: Cadre,
    nom_prenom: bool,
    champs: Vec<Champ>,
    corps: Corps,
    grilles: Vec<Grille>,
    /// Les libelles de lignes de paie connus.
    lignes: Vec<LigneTable>,
}

/// La table entiere : les gabarits, et les familles de cotisations communes.
#[derive(Clone, Debug)]
pub struct Table {
    pub gabarits: Vec<Gabarit>,
    /// (cle, titres canoniques).
    familles: Vec<(String, Vec<String>)>,
}

fn nombre_de(v: &Value, ou: &str) -> Resultat<f64> {
    v.as_f64().ok_or_else(|| format!("{} : un nombre est attendu", ou))
}

fn intervalle(v: Option<&Value>, ou: &str) -> Resultat<(f64, f64)> {
    match v.and_then(Value::as_array).map(Vec::as_slice) {
        Some([a, b]) => Ok((nombre_de(a, ou)?, nombre_de(b, ou)?)),
        _ => Err(format!("{} : [min, max] attendu", ou)),
    }
}

/// Un texte, ou une liste de textes : leurs jetons.
fn phrases(v: Option<&Value>, ou: &str) -> Resultat<Vec<Vec<String>>> {
    let liste: Vec<&Value> = match v {
        Some(Value::String(_)) => vec![v.unwrap()],
        Some(Value::Array(a)) => a.iter().collect(),
        _ => return Err(format!("{} : un texte ou une liste de textes est attendu", ou)),
    };
    let mut sortie = Vec::new();
    for x in liste {
        let p = jetons(x.as_str().ok_or_else(|| format!("{} : un texte est attendu", ou))?);
        if p.is_empty() {
            return Err(format!("{} : un repère vide", ou));
        }
        sortie.push(p);
    }
    if sortie.is_empty() {
        return Err(format!("{} : aucun repère", ou));
    }
    Ok(sortie)
}

fn libelles(v: Option<&Value>, ou: &str) -> Resultat<Vec<String>> {
    Ok(phrases(v, ou)?.into_iter().map(|p| p.join(" ")).collect())
}

fn texte_requis<'a>(v: &'a Value, cle: &str, ou: &str) -> Resultat<&'a str> {
    v.get(cle).and_then(Value::as_str).ok_or_else(|| format!("{} : `{}` manque", ou, cle))
}

fn resume(v: Option<&Value>, colonnes: &[(String, f64, f64)], ou: &str) -> Resultat<Resume> {
    let v = v.ok_or_else(|| format!("{} manque", ou))?;
    let colonne = texte_requis(v, "colonne", ou)?.to_string();
    if !colonnes.iter().any(|c| c.0 == colonne) {
        return Err(format!("{} : colonne inconnue « {} »", ou, colonne));
    }
    Ok(Resume { libelles: libelles(v.get("libelles"), ou)?, colonne })
}

impl Table {
    /// La table livree (`logiciel/donnees/bulletins-gabarits-base.json`).
    pub fn livree() -> Resultat<Table> {
        let chemin = socle::fichiers::dossier_du_code().join(FICHIER_TABLE);
        Table::depuis(&*socle::fichiers::json_en_cache(&chemin)?)
    }

    /// Une table, verifiee : un champ, une colonne ou une forme inconnue est un refus qui la nomme.
    pub fn depuis(v: &Value) -> Resultat<Table> {
        let mut familles = Vec::new();
        if let Some(f) = v.get("familles").and_then(Value::as_object) {
            for (cle, titres) in f.iter().filter(|(k, _)| !k.starts_with('_')) {
                familles.push((cle.clone(), libelles(Some(titres), &format!("familles.{}", cle))?));
            }
        }
        let liste = v.get("gabarits").and_then(Value::as_array).ok_or("table des gabarits : `gabarits` manque")?;
        let gabarits = liste.iter().enumerate().map(|(i, g)| gabarit(g, i)).collect::<Resultat<Vec<_>>>()?;
        Ok(Table { gabarits, familles })
    }

    fn famille(&self, libelle: &str) -> Option<&str> {
        self.familles.iter().find(|(_, t)| t.iter().any(|x| x == libelle)).map(|(c, _)| c.as_str())
    }

    /// Un titre de famille commence-t-il par ce libelle (titre ecrit sur deux rangs) ?
    fn debut_de_famille(&self, libelle: &str) -> bool {
        let debut = format!("{} ", libelle);
        self.familles.iter().any(|(_, t)| t.iter().any(|x| x.starts_with(&debut)))
    }
}

impl Gabarit {
    /// Les libelles de lignes de paie que ce gabarit connait, et leur rubrique.
    pub fn lignes(&self) -> &[LigneTable] {
        &self.lignes
    }
}

fn gabarit(g: &Value, i: usize) -> Resultat<Gabarit> {
    let nom = texte_requis(g, "nom", &format!("gabarit {}", i + 1))?.to_string();
    let ou = |s: &str| format!("gabarit {} : {}", nom, s);
    let tolerance = match g.get("tolerance") {
        Some(t) => nombre_de(t, &ou("tolerance"))?,
        None => 3.5,
    };
    let blocs = g.get("blocs").ok_or_else(|| ou("`blocs` manque"))?;
    let cadre = |cle: &str| -> Resultat<Cadre> {
        let b = blocs.get(cle).ok_or_else(|| ou(&format!("blocs.{} manque", cle)))?;
        Ok(Cadre { x: intervalle(b.get("x"), &ou(&format!("blocs.{}.x", cle)))?, y: intervalle(b.get("y"), &ou(&format!("blocs.{}.y", cle)))? })
    };
    let nom_prenom = match blocs.get("salarie").and_then(|s| s.get("ordre")).and_then(Value::as_str) {
        Some("nom_prenom") => true,
        Some("prenom_nom") => false,
        _ => return Err(ou("blocs.salarie.ordre : `nom_prenom` ou `prenom_nom`")),
    };

    let mut champs = Vec::new();
    if let Some(cs) = g.get("champs").and_then(Value::as_object) {
        for (n, c) in cs.iter().filter(|(k, _)| !k.starts_with('_')) {
            let o = ou(&format!("champs.{}", n));
            let sorte = CHAMPS.iter().find(|(x, _)| x == n).map(|(_, s)| *s).ok_or_else(|| format!("{} : champ inconnu", o))?;
            let dessous = match c.get("position").and_then(Value::as_str) {
                None | Some("droite") => false,
                Some("dessous") => true,
                Some(a) => return Err(format!("{} : position « {} » (droite ou dessous)", o, a)),
            };
            if dessous && sorte != Sorte::Nombre {
                return Err(format!("{} : seul un montant se lit dessous", o));
            }
            let section = match c.get("section").and_then(Value::as_str) {
                None | Some("entete") => Section::Entete,
                Some("pied") => Section::Pied,
                Some("page") => Section::Page,
                Some(a) => return Err(format!("{} : section « {} » (entete, pied ou page)", o, a)),
            };
            champs.push(Champ {
                nom: n.clone(),
                sorte,
                reperes: phrases(c.get("repere"), &o)?,
                dessous,
                jusqu_a: match c.get("jusqu_a") {
                    Some(x) => nombre_de(x, &o)?,
                    None => f64::INFINITY,
                },
                lignes: c.get("lignes").and_then(Value::as_u64).unwrap_or(1).max(1) as usize,
                apres: c.get("apres").and_then(Value::as_str).map(canon),
                avant: c.get("avant").and_then(Value::as_str).map(canon),
                section,
            });
        }
    }

    let c = g.get("corps").ok_or_else(|| ou("`corps` manque"))?;
    let mut colonnes = Vec::new();
    for (n, b) in c.get("colonnes").and_then(Value::as_object).ok_or_else(|| ou("corps.colonnes manque"))? {
        if !COLONNES_CORPS.contains(&n.as_str()) {
            return Err(ou(&format!("corps.colonnes : colonne inconnue « {} »", n)));
        }
        let (a, z) = intervalle(Some(b), &ou(&format!("corps.colonnes.{}", n)))?;
        colonnes.push((n.clone(), a, z));
    }
    let ligne = c.get("ligne").ok_or_else(|| ou("corps.ligne manque"))?;
    let col = |cle: &str| -> Resultat<String> {
        let n = texte_requis(ligne, cle, &ou("corps.ligne"))?;
        if colonnes.iter().any(|x| x.0 == n) {
            Ok(n.to_string())
        } else {
            Err(ou(&format!("corps.ligne.{} : colonne inconnue « {} »", cle, n)))
        }
    };
    let corps = Corps {
        libelle_jusqu_a: nombre_de(c.get("libelle_jusqu_a").unwrap_or(&Value::Null), &ou("corps.libelle_jusqu_a"))?,
        quantite_jusqu_a: nombre_de(c.get("quantite_jusqu_a").unwrap_or(&Value::Null), &ou("corps.quantite_jusqu_a"))?,
        taux: col("taux")?,
        montant: col("montant")?,
        brut: resume(c.get("brut"), &colonnes, &ou("corps.brut"))?,
        total: libelles(c.get("total"), &ou("corps.total"))?,
        net_social: resume(c.get("net_social"), &colonnes, &ou("corps.net_social"))?,
        net_avant_impot: resume(c.get("net_avant_impot"), &colonnes, &ou("corps.net_avant_impot"))?,
        taux_dans_le_libelle: c.get("taux_dans_le_libelle").and_then(Value::as_str).map(canon),
        brut_en_fin_de_ligne: c.get("brut_en_fin_de_ligne").and_then(Value::as_bool).unwrap_or(false),
        colonnes,
    };

    let mut grilles = Vec::new();
    for (k, gr) in g.get("grilles").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]).iter().enumerate() {
        let o = ou(&format!("grilles[{}]", k));
        let mut cols = Vec::new();
        for x in gr.get("colonnes").and_then(Value::as_array).ok_or_else(|| format!("{} : colonnes manque", o))? {
            let n = texte_requis(x, "nom", &o)?.to_string();
            let entete = phrases(x.get("entete"), &o)?.remove(0);
            cols.push((n, entete, x.get("facultative").and_then(Value::as_bool).unwrap_or(false)));
        }
        if cols.is_empty() || cols[0].2 {
            return Err(format!("{} : la premiere colonne est obligatoire", o));
        }
        let a_col = |n: &str| cols.iter().any(|c| c.0 == n);
        let mut lignes = Vec::new();
        for x in gr.get("lignes").and_then(Value::as_array).ok_or_else(|| format!("{} : lignes manque", o))? {
            let cible = match (x.get("champ").and_then(Value::as_str), x.get("cumul").and_then(Value::as_str)) {
                (Some("pas"), None) => {
                    if !(a_col("base") && a_col("taux") && a_col("montant")) {
                        return Err(format!("{} : la ligne du PAS veut les colonnes base, taux et montant", o));
                    }
                    Cible::Pas
                }
                (Some("net_imposable"), None) => {
                    let colonne = texte_requis(x, "colonne", &o)?;
                    if !a_col(colonne) {
                        return Err(format!("{} : colonne inconnue « {} »", o, colonne));
                    }
                    Cible::NetImposable(colonne.into())
                }
                (None, Some(nom)) => {
                    if !(a_col("periode") && a_col("cumul")) {
                        return Err(format!("{} : un cumul veut les colonnes periode et cumul", o));
                    }
                    Cible::Cumul(nom.into())
                }
                _ => return Err(format!("{} : une ligne remplit `champ` (pas, net_imposable) ou `cumul`", o)),
            };
            lignes.push((cible, libelles(x.get("libelles"), &o)?));
        }
        grilles.push(Grille { colonnes: cols, lignes });
    }

    let mut lignes: Vec<LigneTable> = Vec::new();
    if let Some(l) = g.get("lignes").and_then(Value::as_object) {
        for (libelle, valeur) in l.iter().filter(|(k, _)| !k.starts_with('_')) {
            let o = ou(&format!("lignes.{}", libelle));
            let (rubrique, heures) = match valeur {
                Value::Null => (None, None),
                Value::String(s) if !s.is_empty() => (Some(s.clone()), None),
                Value::Object(m) => {
                    if let Some(k) = m.keys().find(|k| !k.starts_with('_') && !matches!(k.as_str(), "rubrique" | "heures")) {
                        return Err(format!("{} : cle inconnue « {} » (rubrique, heures)", o, k));
                    }
                    let r = m.get("rubrique").and_then(Value::as_str).filter(|s| !s.is_empty());
                    let r = r.ok_or_else(|| format!("{} : `rubrique` manque", o))?.to_string();
                    let h = match m.get("heures") {
                        None => None,
                        Some(x) => match socle::num::dec(x) {
                            Ok(h) if x.is_number() && h > Decimal::ZERO => Some(h),
                            _ => return Err(format!("{} : `heures` est un nombre d'heures positif", o)),
                        },
                    };
                    (Some(r), h)
                }
                _ => return Err(format!("{} : une rubrique, null, ou {{rubrique, heures}}", o)),
            };
            let c = canon(libelle);
            if lignes.iter().any(|x| x.canon == c) {
                return Err(format!("{} : libellé en double", o));
            }
            lignes.push(LigneTable { libelle: libelle.clone(), canon: c, rubrique, heures });
        }
    }

    Ok(Gabarit {
        libelle: g.get("libelle").and_then(Value::as_str).unwrap_or(&nom).to_string(),
        signature: phrases(g.get("signature"), &ou("signature"))?,
        tolerance,
        tableau: phrases(g.get("tableau"), &ou("tableau"))?,
        entreprise: cadre("entreprise")?,
        salarie: cadre("salarie")?,
        nom_prenom,
        champs,
        corps,
        grilles,
        lignes,
        nom,
    })
}

// --------------------------------------------------------------------------------------------- le bulletin lu

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Entreprise {
    pub nom: Option<String>,
    pub adresse: Vec<String>,
    pub siret: Option<String>,
    pub naf: Option<String>,
    pub urssaf: Option<String>,
    pub convention: Option<String>,
    pub licence: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Salarie {
    /// Le nom tel qu'il est ecrit (premier rang du cadre).
    pub nom_complet: Option<String>,
    pub nom: Option<String>,
    pub prenom: Option<String>,
    pub nir: Option<String>,
    pub naissance: Option<Jour>,
    pub adresse: Vec<String>,
    pub code: Option<String>,
    pub emploi: Option<String>,
    pub statut: Option<String>,
    pub abattement: Option<Decimal>,
    pub conges_spectacles: Option<String>,
}

/// Une ligne de paie : soumise (avant le brut) ou non (apres les cotisations : un defraiement).
#[derive(Clone, Debug, PartialEq)]
pub struct LignePaie {
    pub quantite: Option<Decimal>,
    pub libelle: String,
    pub taux: Option<Decimal>,
    pub montant: Option<Decimal>,
    pub soumis: bool,
    /// La rubrique du catalogue que la table donne a ce libelle (lot C) ; `None` : libelle connu sans rubrique sure,
    /// ou absent de la table (alors signale).
    pub rubrique: Option<String>,
    /// Les heures d'une unite que le libelle dit (« 4h »), d'apres la table : pour une rubrique dont les heures se
    /// saisissent (un service de repetition).
    pub heures: Option<Decimal>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cotisation {
    /// La cle de la famille (`familles` de la table) sous laquelle la ligne est ecrite.
    pub famille: Option<String>,
    pub libelle: String,
    pub base: Option<Decimal>,
    pub taux_employeur: Option<Decimal>,
    pub montant_employeur: Option<Decimal>,
    pub taux_salarie: Option<Decimal>,
    pub montant_salarie: Option<Decimal>,
    /// Ecrite apres le total des cotisations (une famille facultative) : hors de ce total.
    pub hors_total: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pas {
    pub base: Option<Decimal>,
    pub taux: Option<Decimal>,
    pub montant: Option<Decimal>,
    pub cumul: Option<Decimal>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cumul {
    pub periode: Option<Decimal>,
    pub cumul: Option<Decimal>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BulletinLu {
    pub gabarit: String,
    pub numero: Option<String>,
    pub entreprise: Entreprise,
    pub salarie: Salarie,
    pub numero_objet: Option<String>,
    /// Le titre du spectacle (CoolPaie).
    pub spectacle: Option<String>,
    /// Le code analytique (gabarit B).
    pub analytique: Option<String>,
    pub debut: Jour,
    pub fin: Jour,
    /// Les intervalles « du ... au ... » des dates travaillees, tels qu'ecrits.
    pub dates_travaillees: Vec<(Jour, Jour)>,
    /// Les memes, jour par jour.
    pub jours_travailles: Vec<Jour>,
    pub lignes: Vec<LignePaie>,
    pub brut: Decimal,
    pub cotisations: Vec<Cotisation>,
    pub total_employeur: Option<Decimal>,
    pub total_salarie: Option<Decimal>,
    pub net_social: Option<Decimal>,
    pub net_avant_impot: Option<Decimal>,
    pub net_imposable: Option<Decimal>,
    pub pas: Option<Pas>,
    pub net_a_payer: Decimal,
    /// « Total verse par l'employeur ».
    pub cout_employeur: Option<Decimal>,
    pub regle_le: Option<Jour>,
    pub mode_reglement: Option<String>,
    /// Les cumuls, dans l'ordre de la table : `cachets`, `jours`, `heures`, `brut`, `plafond`, `cout_employeur`...
    pub cumuls: Vec<(String, Cumul)>,
    pub avertissements: Vec<String>,
}

/// Pourquoi une page n'est pas un bulletin lu.
#[derive(Clone, Debug, PartialEq)]
pub enum Refus {
    /// Aucun mot : lettres en traces ou page scannee (pas d'OCR). Le detail dit ce que la fabrique a vu.
    SansTexte(String),
    /// Aucun gabarit de la table ne reconnait la page.
    GabaritInconnu,
    /// Plusieurs gabarits la reconnaissent : la table est a corriger, on ne choisit pas.
    PlusieursGabarits(Vec<String>),
    /// Un libelle de ligne de paie absent de la table. Il n'est que SIGNALE (son message va dans les avertissements) :
    /// la ligne est gardee telle quelle, la page lue (decide, plan §4).
    LibelleInconnu(String),
    /// Ce qui identifie le bulletin ne se lit pas (salarie, periode, brut, net a payer).
    Incomplet(String),
    /// La page se contredit au point de ne pas se lire.
    Incoherent(String),
}

impl Refus {
    pub fn code(&self) -> &'static str {
        match self {
            Refus::SansTexte(_) => "sans_texte",
            Refus::GabaritInconnu => "gabarit_inconnu",
            Refus::PlusieursGabarits(_) => "plusieurs_gabarits",
            Refus::LibelleInconnu(_) => "libelle_inconnu",
            Refus::Incomplet(_) => "incomplet",
            Refus::Incoherent(_) => "incoherent",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Refus::SansTexte(d) if d.is_empty() => "sans texte".into(),
            Refus::SansTexte(d) => format!("sans texte : {}", d),
            Refus::GabaritInconnu => "gabarit inconnu".into(),
            Refus::PlusieursGabarits(n) => format!("plusieurs gabarits : {}", n.join(", ")),
            Refus::LibelleInconnu(l) => format!("libellé inconnu : « {} »", l),
            Refus::Incomplet(d) => format!("incomplet : {}", d),
            Refus::Incoherent(d) => format!("incohérent : {}", d),
        }
    }
}

/// Une page lue : un bulletin, ou le refus qui dit pourquoi pas.
#[derive(Clone, Debug)]
pub struct Lecture {
    /// La page dans le PDF, a partir de 1.
    pub page: usize,
    pub resultat: Result<BulletinLu, Refus>,
    /// Une page refusee apres que le cadre du salarie a ete lu : ce qu'il en a rendu (nom, NIR, adresse...), pour la
    /// caviarder (lot E). `None` pour un bulletin lu, ou quand rien n'a ete reconnu.
    pub partiel: Option<Salarie>,
}

impl Lecture {
    pub fn to_json(&self) -> Value {
        match &self.resultat {
            Ok(b) => json!({"page": self.page, "bulletin": b.to_json()}),
            Err(r) => json!({"page": self.page, "refus": r.code(), "raison": r.message()}),
        }
    }
}

fn d(x: &Option<Decimal>) -> Value {
    x.map_or(Value::Null, |v| Value::String(v.to_string()))
}

fn t(x: &Option<String>) -> Value {
    x.as_ref().map_or(Value::Null, |v| Value::String(v.clone()))
}

fn j(x: &Option<Jour>) -> Value {
    x.map_or(Value::Null, |v| Value::String(v.iso()))
}

impl BulletinLu {
    /// Les montants en chaines (`"1421.13"`, exacts, comme `str(Decimal)`), les dates en ISO, l'absent en `null`.
    pub fn to_json(&self) -> Value {
        let e = &self.entreprise;
        let s = &self.salarie;
        let mut cumuls = Map::new();
        for (n, c) in &self.cumuls {
            cumuls.insert(n.clone(), json!({"periode": d(&c.periode), "cumul": d(&c.cumul)}));
        }
        json!({
            "gabarit": self.gabarit,
            "numero": t(&self.numero),
            "entreprise": {
                "nom": t(&e.nom), "adresse": e.adresse, "siret": t(&e.siret), "naf": t(&e.naf),
                "urssaf": t(&e.urssaf), "convention": t(&e.convention), "licence": t(&e.licence),
            },
            "salarie": {
                "nom_complet": t(&s.nom_complet), "nom": t(&s.nom), "prenom": t(&s.prenom), "nir": t(&s.nir),
                "naissance": j(&s.naissance), "adresse": s.adresse, "code": t(&s.code), "emploi": t(&s.emploi),
                "statut": t(&s.statut), "abattement": d(&s.abattement), "conges_spectacles": t(&s.conges_spectacles),
            },
            "numero_objet": t(&self.numero_objet),
            "spectacle": t(&self.spectacle),
            "analytique": t(&self.analytique),
            "debut": self.debut.iso(),
            "fin": self.fin.iso(),
            "dates_travaillees": self.dates_travaillees.iter().map(|(a, b)| json!({"debut": a.iso(), "fin": b.iso()})).collect::<Vec<_>>(),
            "jours_travailles": self.jours_travailles.iter().map(Jour::iso).collect::<Vec<_>>(),
            "lignes": self.lignes.iter().map(|l| json!({
                "quantite": d(&l.quantite), "libelle": l.libelle, "taux": d(&l.taux), "montant": d(&l.montant),
                "soumis": l.soumis, "rubrique": t(&l.rubrique), "heures": d(&l.heures),
            })).collect::<Vec<_>>(),
            "brut": self.brut.to_string(),
            "cotisations": self.cotisations.iter().map(|c| json!({
                "famille": t(&c.famille), "libelle": c.libelle, "base": d(&c.base),
                "taux_employeur": d(&c.taux_employeur), "montant_employeur": d(&c.montant_employeur),
                "taux_salarie": d(&c.taux_salarie), "montant_salarie": d(&c.montant_salarie), "hors_total": c.hors_total,
            })).collect::<Vec<_>>(),
            "total_employeur": d(&self.total_employeur),
            "total_salarie": d(&self.total_salarie),
            "net_social": d(&self.net_social),
            "net_avant_impot": d(&self.net_avant_impot),
            "net_imposable": d(&self.net_imposable),
            "pas": self.pas.as_ref().map_or(Value::Null, |p| json!({
                "base": d(&p.base), "taux": d(&p.taux), "montant": d(&p.montant), "cumul": d(&p.cumul),
            })),
            "net_a_payer": self.net_a_payer.to_string(),
            "cout_employeur": d(&self.cout_employeur),
            "regle_le": j(&self.regle_le),
            "mode_reglement": t(&self.mode_reglement),
            "cumuls": cumuls,
            "avertissements": self.avertissements,
        })
    }
}

// --------------------------------------------------------------------------------------------- la lecture

/// `lire(chemin)` : chaque page du PDF, lue avec la table livree. Une erreur : le PDF ne s'ouvre pas, ou la table
/// ne se lit pas.
pub fn lire(chemin: &Path) -> Resultat<Vec<Lecture>> {
    lire_avec(chemin, &Table::livree()?)
}

/// La meme, avec une table donnee.
pub fn lire_avec(chemin: &Path, table: &Table) -> Resultat<Vec<Lecture>> {
    let (texte, avertissements) = fabrique_pdf::inspection::inspecter(&chemin.to_string_lossy())
        .map_err(|r| format!("PDF que la fabrique n'ouvre pas : {}", r.0))?;
    let releve: Value = serde_json::from_str(&texte).map_err(|e| format!("releve du PDF illisible ({})", e))?;
    let pages = releve.get("pages").and_then(Value::as_array).cloned().unwrap_or_default();
    Ok(pages
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let prefixe = format!("page {} : ", i + 1);
            let signale: Vec<&str> = avertissements.iter().filter_map(|a| a.strip_prefix(&prefixe)).collect();
            let (resultat, lu) = lire_page(p, &signale, table);
            // Un bulletin lu porte son salarie : `partiel` ne sert qu'a une page refusee.
            let partiel = if resultat.is_ok() { None } else { lu };
            Lecture { page: i + 1, resultat, partiel }
        })
        .collect())
}

fn mots_de(p: &Value) -> Vec<Mot> {
    let mut mots: Vec<Mot> = Vec::new();
    for m in p.get("mots").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]) {
        let texte = m.get("texte").and_then(Value::as_str).unwrap_or("").to_string();
        let r: Vec<f64> = m.get("rect").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
        if texte.is_empty() || r.len() != 4 {
            continue;
        }
        // Un meme mot ecrit deux fois au meme endroit (CoolPaie double ses cumuls) ne compte qu'une fois.
        let double = mots.iter().any(|x| x.texte == texte && (x.x0 - r[0]).abs() < 0.05 && (x.y0 - r[1]).abs() < 0.05);
        if !double {
            mots.push(Mot { jetons: jetons(&texte), texte, x0: r[0], y0: r[1], x1: r[2], y1: r[3] });
        }
    }
    mots
}

fn lire_page(p: &Value, signale: &[&str], table: &Table) -> (Result<BulletinLu, Refus>, Option<Salarie>) {
    let mots = mots_de(p);
    if mots.is_empty() {
        let n = |k: &str| p.get(k).and_then(Value::as_array).map_or(0, Vec::len);
        let mut detail = format!("aucun mot ({} police(s), {} image(s), {} trait(s))", n("polices"), n("images"), n("traits"));
        if !signale.is_empty() {
            detail.push_str(&format!(" ; la fabrique a signale : {}", signale.join(" ; ")));
        }
        return (Err(Refus::SansTexte(detail)), None);
    }
    let reconnus: Vec<&Gabarit> = table
        .gabarits
        .iter()
        .filter(|g| {
            let page = Page::nouvelle(mots.clone(), g.tolerance);
            g.signature.iter().all(|s| page.present(s))
        })
        .collect();
    match reconnus.as_slice() {
        [] => (Err(Refus::GabaritInconnu), None),
        [g] => {
            let mut l = Lecteur { g, table, page: Page::nouvelle(mots, g.tolerance), avertissements: Vec::new(), partiel: None };
            let r = l.lire();
            (r, l.partiel)
        }
        plusieurs => (Err(Refus::PlusieursGabarits(plusieurs.iter().map(|g| g.nom.clone()).collect())), None),
    }
}

/// Une valeur de champ, lue.
#[derive(Clone, Debug)]
enum Valeur {
    Texte(String),
    Date(Jour),
    Periode(Jour, Jour),
    Intervalles(Vec<(Jour, Jour)>),
    Nombre(Decimal),
}

struct Lecteur<'a> {
    g: &'a Gabarit,
    table: &'a Table,
    page: Page,
    avertissements: Vec<String>,
    /// Ce que le cadre du salarie a rendu, des qu'il est lu (meme si la page est refusee ensuite).
    partiel: Option<Salarie>,
}

/// Les bornes verticales des sections : l'en-tete finit au tableau, le pied commence au net avant impot.
#[derive(Clone, Copy)]
struct Sections {
    tableau: Boite,
    pied: Option<Boite>,
}

/// Un rang du corps : son libelle (la quantite en tete mise a part) et ses nombres, par colonne.
struct RangCorps {
    libelle: String,
    quantite: Option<Decimal>,
    nombres: Vec<(String, Decimal)>,
    cy: f64,
}

impl RangCorps {
    fn colonne(&self, c: &str) -> Option<Decimal> {
        self.nombres.iter().find(|(n, _)| n == c).map(|(_, v)| *v)
    }
}

impl<'a> Lecteur<'a> {
    fn lire(&mut self) -> Result<BulletinLu, Refus> {
        let tol = self.g.tolerance;
        let tableau = self
            .page
            .trouver(&self.g.tableau, &|_| true)
            .map(|(b, _)| b)
            .ok_or_else(|| Refus::Incomplet(format!("l'en-tête du tableau (« {} ») ne se trouve pas", self.g.tableau[0].join(" "))))?;
        let fin_corps: Vec<Vec<String>> = self.g.corps.net_avant_impot.libelles.iter().map(|l| l.split(' ').map(String::from).collect()).collect();
        let pied = self.page.trouver(&fin_corps, &|m: &Mot| m.cy() > tableau.cy() + tol).map(|(b, _)| b);
        let sections = Sections { tableau, pied };

        // L'en-tete et le pied.
        let mut valeurs: Vec<(String, Valeur)> = Vec::new();
        for c in &self.g.champs {
            if let Some(v) = self.champ(c, sections) {
                valeurs.push((c.nom.clone(), v));
            }
        }
        let texte = |n: &str| {
            valeurs.iter().find(|(k, _)| k == n).and_then(|(_, v)| match v {
                Valeur::Texte(s) => Some(s.clone()),
                _ => None,
            })
        };
        let date = |n: &str| valeurs.iter().find(|(k, _)| k == n).and_then(|(_, v)| if let Valeur::Date(j) = v { Some(*j) } else { None });
        let nombre = |n: &str| valeurs.iter().find(|(k, _)| k == n).and_then(|(_, v)| if let Valeur::Nombre(x) = v { Some(*x) } else { None });
        let periode = valeurs.iter().find_map(|(k, v)| if let (true, Valeur::Periode(a, b)) = (k == "periode", v) { Some((*a, *b)) } else { None });
        let dates_travaillees = valeurs
            .iter()
            .find_map(|(k, v)| if let (true, Valeur::Intervalles(l)) = (k == "dates_travaillees", v) { Some(l.clone()) } else { None })
            .unwrap_or_default();

        // Les deux cadres.
        let (nom_e, adresse_e) = self.cadre(&self.g.entreprise, sections);
        let (nom_s, adresse_s) = self.cadre(&self.g.salarie, sections);
        let (nom, prenom) = match &nom_s {
            Some(n) => self.separer_nom(n),
            None => (None, None),
        };

        // Le corps.
        let corps = self.corps(sections);
        // Le pied : les grilles.
        let mut pas: Option<Pas> = None;
        let mut net_imposable: Option<Decimal> = None;
        let mut cumuls: Vec<(String, Cumul)> = Vec::new();
        for gr in &self.g.grilles {
            self.grille(gr, sections, &mut pas, &mut net_imposable, &mut cumuls);
        }

        let mut jours_travailles = Vec::new();
        for (a, b) in &dates_travaillees {
            let n = b.moins(a);
            if !(0..=INTERVALLE_MAX).contains(&n) {
                self.avertissements.push(format!("dates travaillées du {} au {} : intervalle non déplié", a.iso(), b.iso()));
                continue;
            }
            for k in 0..=n {
                if let Ok(x) = a.plus_jours(k) {
                    jours_travailles.push(x);
                }
            }
        }

        // Ce qui identifie le bulletin. Ce que le cadre du salarie a deja rendu est garde (`partiel`) : une page refusee
        // plus bas se caviarde depuis ce qu'on a reconnu (lot E).
        let nir = texte("nir");
        let salarie = Salarie {
            nom_complet: nom_s.clone(),
            nom,
            prenom,
            nir: nir.clone(),
            naissance: date("naissance"),
            adresse: adresse_s,
            code: texte("code"),
            emploi: texte("emploi"),
            statut: texte("statut"),
            abattement: nombre("abattement"),
            conges_spectacles: texte("conges_spectacles"),
        };
        self.partiel = Some(salarie.clone());
        if nom_s.is_none() && nir.is_none() {
            return Err(Refus::Incomplet("ni le nom du salarié ni son NIR ne se lisent".into()));
        }
        let (debut, fin) = periode.ok_or_else(|| Refus::Incomplet("la période (du ... au ...) ne se lit pas".into()))?;
        let brut = corps.brut.ok_or_else(|| Refus::Incomplet(format!("le brut (« {} ») ne se lit pas", self.g.corps.brut.libelles[0])))?;
        let net_a_payer = nombre("net_a_payer").ok_or_else(|| Refus::Incomplet("le net à payer ne se lit pas".into()))?;

        let mut b = BulletinLu {
            gabarit: self.g.nom.clone(),
            numero: texte("numero"),
            entreprise: Entreprise {
                nom: nom_e,
                adresse: adresse_e,
                siret: texte("siret"),
                naf: texte("naf"),
                urssaf: texte("urssaf"),
                convention: texte("convention"),
                licence: texte("licence"),
            },
            salarie,
            numero_objet: texte("numero_objet"),
            spectacle: texte("spectacle"),
            analytique: texte("analytique"),
            debut,
            fin,
            dates_travaillees,
            jours_travailles,
            lignes: corps.lignes,
            brut,
            cotisations: corps.cotisations,
            total_employeur: corps.total_employeur,
            total_salarie: corps.total_salarie,
            net_social: corps.net_social,
            net_avant_impot: corps.net_avant_impot,
            net_imposable,
            pas,
            net_a_payer,
            cout_employeur: nombre("cout_employeur"),
            regle_le: date("regle_le"),
            mode_reglement: texte("mode_reglement"),
            cumuls,
            avertissements: std::mem::take(&mut self.avertissements),
        };
        controler(&mut b);
        Ok(b)
    }

    fn dans_section(&self, m_cy: f64, section: Section, s: Sections) -> bool {
        let tol = self.g.tolerance;
        match section {
            Section::Entete => m_cy < s.tableau.cy() - tol,
            Section::Pied => s.pied.is_some_and(|p| m_cy >= p.cy() - tol),
            Section::Page => true,
        }
    }

    /// Un champ de l'en-tete ou du pied.
    fn champ(&mut self, c: &Champ, s: Sections) -> Option<Valeur> {
        let tol = self.g.tolerance;
        let (rep, _) = self.page.trouver(&c.reperes, &|m: &Mot| self.dans_section(m.cy(), c.section, s))?;
        if c.dessous {
            let fenetre = |n: &&Nombre| {
                let b = n.boite;
                b.cy() > rep.cy() + 2.0 && b.cy() - rep.cy() <= DESSOUS_MAX && b.cx() >= rep.x0 - 20.0 && b.cx() <= rep.x1 + 80.0
            };
            let dist = |n: &Nombre| (n.boite.cx() - rep.cx()).hypot(n.boite.cy() - rep.cy());
            return self.page.nombres.iter().filter(fenetre).min_by(|a, b| dist(a).total_cmp(&dist(b))).map(|n| Valeur::Nombre(n.valeur));
        }
        // A droite, sur le meme rang, puis les rangs qui suivent (`lignes`).
        let dans = |m: &Mot, x_min: f64| m.x0 >= x_min && m.x1 <= c.jusqu_a;
        let mut rangs: Vec<Vec<&Mot>> = Vec::new();
        let premier: Vec<&Mot> = self.page.mots.iter().filter(|m| (m.cy() - rep.cy()).abs() <= tol && dans(m, rep.x1 - 0.5)).collect();
        let x_suite = premier.iter().map(|m| m.x0).fold(rep.x0, f64::min) - 3.0;
        let mut cy = rep.cy();
        rangs.push(premier);
        while rangs.len() < c.lignes {
            let suivants: Vec<&Mot> =
                self.page.mots.iter().filter(|m| m.cy() > cy + tol && m.cy() <= cy + tol + 14.0 && dans(m, x_suite)).collect();
            let Some(haut) = suivants.iter().map(|m| m.cy()).min_by(f64::total_cmp) else { break };
            let mut rang: Vec<&Mot> = suivants.into_iter().filter(|m| (m.cy() - haut).abs() <= tol).collect();
            rang.sort_by(|a, b| a.x0.total_cmp(&b.x0));
            // Un rang qui commence par le repère d'un autre champ n'est pas une suite.
            let i = self.page.mots.iter().position(|m| std::ptr::eq(m, rang[0])).unwrap_or(0);
            if self.g.champs.iter().any(|x| x.reperes.iter().any(|p| self.page.repere_en(i, p).is_some())) {
                break;
            }
            cy = haut;
            rangs.push(rang);
        }
        let mut mots: Vec<&Mot> = rangs.into_iter().flatten().collect();
        if let Some(a) = &c.apres {
            let k = mots.iter().position(|m| &canon(&m.texte) == a)?;
            mots.drain(..=k);
        }
        if let Some(a) = &c.avant {
            if let Some(k) = mots.iter().position(|m| &canon(&m.texte) == a) {
                mots.truncate(k);
            }
        }
        // Le deux-points isole qui suit un repère n'est pas la valeur.
        while mots.first().is_some_and(|m| m.texte == ":") {
            mots.remove(0);
        }
        if mots.is_empty() {
            return None;
        }
        let textes: Vec<&str> = mots.iter().map(|m| m.texte.as_str()).collect();
        match c.sorte {
            Sorte::Texte => Some(Valeur::Texte(textes.join(" "))),
            Sorte::Compact => Some(Valeur::Texte(textes.concat())),
            Sorte::Date => textes.iter().find_map(|x| lire_date(x)).map(Valeur::Date),
            Sorte::Nombre => {
                let chiffres: Vec<&str> = textes.iter().copied().filter(|x| !matches!(*x, "%" | "€")).collect();
                lire_nombre(&chiffres.join(" ")).map(Valeur::Nombre)
            }
            Sorte::Periode => {
                let dates: Vec<Jour> = textes.iter().filter_map(|x| lire_date(x)).collect();
                match dates.as_slice() {
                    [a, b] => Some(Valeur::Periode(*a, *b)),
                    // « du A au B (contrat du C au D) » : la periode est la premiere.
                    [..] if textes.len() >= 4 && canon(textes[0]) == "du" && canon(textes[2]) == "au" && lire_date(textes[1]).is_some() && lire_date(textes[3]).is_some() => {
                        Some(Valeur::Periode(lire_date(textes[1])?, lire_date(textes[3])?))
                    }
                    _ => {
                        self.avertissements.push(format!("{} : « {} » n'est pas une période du ... au ...", c.nom, textes.join(" ")));
                        None
                    }
                }
            }
            Sorte::Intervalles => {
                let canons: Vec<String> = textes.iter().map(|x| canon(x)).collect();
                let mut l = Vec::new();
                let mut k = 0;
                while k + 3 < textes.len() {
                    if canons[k] == "du" && canons[k + 2] == "au" {
                        if let (Some(a), Some(b)) = (lire_date(textes[k + 1]), lire_date(textes[k + 3])) {
                            l.push((a, b));
                            k += 4;
                            continue;
                        }
                    }
                    k += 1;
                }
                if l.is_empty() {
                    // Une liste de jours (« 20/11/2026, 21/11/2026 ») : chacun est un intervalle d'un jour.
                    l = textes.iter().filter_map(|x| lire_date(x)).map(|j| (j, j)).collect();
                }
                if l.is_empty() {
                    self.avertissements.push(format!("{} : « {} » ne se lit pas en dates", c.nom, textes.join(" ")));
                    return None;
                }
                Some(Valeur::Intervalles(l))
            }
        }
    }

    /// Un cadre sans repère (entreprise, salarie) : le premier rang est le nom, les suivants l'adresse.
    fn cadre(&self, c: &Cadre, s: Sections) -> (Option<String>, Vec<String>) {
        let mots: Vec<&Mot> = self
            .page
            .mots
            .iter()
            .filter(|m| m.x0 >= c.x.0 && m.x1 <= c.x.1 && m.cy() >= c.y.0 && m.cy() <= c.y.1 && self.dans_section(m.cy(), Section::Entete, s))
            .collect();
        let mut rangs = en_rangs(mots, self.g.tolerance).into_iter().map(|r| texte_de(&r));
        (rangs.next(), rangs.collect())
    }

    /// Le nom et le prenom, selon l'ordre du gabarit ; sinon, le nom complet seul et un avertissement.
    fn separer_nom(&mut self, complet: &str) -> (Option<String>, Option<String>) {
        let mots: Vec<&str> = complet.split_whitespace().collect();
        let capitales = |w: &str| w.chars().any(char::is_alphabetic) && !w.chars().any(char::is_lowercase);
        if self.g.nom_prenom {
            if let [nom, prenom] = mots.as_slice() {
                return (Some(nom.to_string()), Some(prenom.to_string()));
            }
        } else if let Some(k) = mots.iter().position(|w| capitales(w)) {
            if k > 0 && mots[k..].iter().all(|w| capitales(w)) {
                return (Some(mots[k..].join(" ")), Some(mots[..k].join(" ")));
            }
        }
        self.avertissements.push(format!("nom et prénom non séparés : « {} »", complet));
        (None, None)
    }

    /// Les rangs du corps : libelles a gauche, nombres a droite, chaque nombre au rang le plus proche.
    fn rangs_du_corps(&mut self, s: Sections) -> Vec<RangCorps> {
        let tol = self.g.tolerance;
        let c = &self.g.corps;
        let haut = s.tableau.cy() + tol;
        let bas = s.pied.map_or(f64::INFINITY, |p| p.cy() + tol);
        let dans = |y: f64| y > haut && y <= bas;
        let gauche: Vec<&Mot> = self.page.mots.iter().filter(|m| m.x1 <= c.libelle_jusqu_a && dans(m.cy()) && !self.page.dans_un_nombre(m, c.libelle_jusqu_a)).collect();
        let rangs = en_rangs(gauche, tol);
        let mut sortie: Vec<RangCorps> = rangs
            .iter()
            .map(|r| {
                let (quantite, libelle) = match r.split_first() {
                    Some((q, reste)) if !reste.is_empty() && q.x0 <= c.quantite_jusqu_a && ressemble_nombre(&q.texte) => (lire_nombre(&q.texte), texte_de(reste)),
                    _ => (None, texte_de(r)),
                };
                RangCorps { libelle, quantite, nombres: Vec::new(), cy: milieu(r) }
            })
            .collect();
        let nombres: Vec<Nombre> = self.page.nombres.iter().filter(|n| n.boite.cx() > c.libelle_jusqu_a && dans(n.boite.cy())).cloned().collect();
        for n in nombres {
            let Some(col) = c.colonnes.iter().find(|(_, a, z)| n.boite.x1 > *a && n.boite.x1 <= *z) else {
                self.avertissements.push(format!("nombre {} hors des colonnes du tableau (x {:.0})", n.valeur, n.boite.x1));
                continue;
            };
            let proche = sortie.iter_mut().filter(|r| (r.cy - n.boite.cy()).abs() <= tol).min_by(|a, b| (a.cy - n.boite.cy()).abs().total_cmp(&(b.cy - n.boite.cy()).abs()));
            match proche {
                Some(r) if r.colonne(&col.0).is_none() => r.nombres.push((col.0.clone(), n.valeur)),
                Some(r) => self.avertissements.push(format!("« {} » : deux nombres dans la colonne {}", r.libelle, col.0)),
                None => self.avertissements.push(format!("nombre {} sans libellé à sa hauteur", n.valeur)),
            }
        }
        sortie
    }

    fn corps(&mut self, s: Sections) -> CorpsLu {
        let rangs = self.rangs_du_corps(s);
        let c = &self.g.corps;
        let est = |libelle: &str, liste: &[String]| liste.iter().any(|x| x == libelle);
        let mut lu = CorpsLu::default();
        /// Les lignes soumises jusqu'au brut ; les cotisations jusqu'au total ; apres le total, une famille
        /// facultative (hors total) ou le net social, puis les lignes non soumises.
        #[derive(PartialEq, Clone, Copy)]
        enum Phase {
            Lignes,
            Cotisations,
            ApresTotal,
            NonSoumis,
        }
        let mut phase = Phase::Lignes;
        let mut famille: Option<String> = None;
        let mut famille_apres_total = false;
        let mut titre_en_cours: Option<(String, String)> = None; // (canonique, tel qu'ecrit)
        let mut avertissements = Vec::new();
        let ligne = |r: &RangCorps, soumis: bool, avertissements: &mut Vec<String>| {
            let (libelle, taux_ecrit) = match &c.taux_dans_le_libelle {
                Some(mot) => taux_du_libelle(&r.libelle, mot),
                None => (r.libelle.clone(), None),
            };
            let cl = canon(&libelle);
            let (rubrique, heures) = match self.g.lignes.iter().find(|x| x.canon == cl) {
                Some(x) => (x.rubrique.clone(), x.heures),
                None => {
                    avertissements.push(Refus::LibelleInconnu(libelle.clone()).message());
                    (None, None)
                }
            };
            LignePaie { quantite: r.quantite, libelle, taux: taux_ecrit.or(r.colonne(&c.taux)), montant: r.colonne(&c.montant), soumis, rubrique, heures }
        };
        for r in rangs {
            let cl = canon(&r.libelle);
            if est(&cl, &c.net_avant_impot.libelles) {
                lu.net_avant_impot = r.colonne(&c.net_avant_impot.colonne);
                break;
            }
            if r.libelle.is_empty() {
                continue;
            }
            // La derniere ligne et le brut sur un meme rang : la ligne, puis le brut.
            if phase == Phase::Lignes && c.brut_en_fin_de_ligne {
                if let Some(b) = c.brut.libelles.iter().find(|b| cl.ends_with(&format!(" {}", b))) {
                    let n = b.split(' ').count();
                    let mots: Vec<&str> = r.libelle.split_whitespace().collect();
                    let garde = mots.len().saturating_sub(n);
                    let libelle = mots[..garde].join(" ").trim_end_matches(|ch: char| !ch.is_alphanumeric()).trim().to_string();
                    let seule = RangCorps { libelle, quantite: r.quantite, nombres: r.nombres.clone(), cy: r.cy };
                    lu.lignes.push(ligne(&seule, true, &mut avertissements));
                    lu.brut = r.colonne(&c.brut.colonne);
                    phase = Phase::Cotisations;
                    continue;
                }
            }
            match phase {
                Phase::Lignes if est(&cl, &c.brut.libelles) => {
                    lu.brut = r.colonne(&c.brut.colonne);
                    phase = Phase::Cotisations;
                }
                Phase::Lignes => lu.lignes.push(ligne(&r, true, &mut avertissements)),
                Phase::NonSoumis if est(&cl, &c.net_social.libelles) => lu.net_social = r.colonne(&c.net_social.colonne),
                Phase::NonSoumis => lu.lignes.push(ligne(&r, false, &mut avertissements)),
                Phase::Cotisations | Phase::ApresTotal => {
                    if phase == Phase::Cotisations && est(&cl, &c.total) {
                        lu.total_employeur = r.colonne("montant_employeur");
                        lu.total_salarie = r.colonne("montant_salarie");
                        phase = Phase::ApresTotal;
                        if let Some((_, ecrit)) = titre_en_cours.take() {
                            avertissements.push(format!("rang sans montant : « {} »", ecrit));
                        }
                        continue;
                    }
                    if phase == Phase::ApresTotal && est(&cl, &c.net_social.libelles) {
                        lu.net_social = r.colonne(&c.net_social.colonne);
                        phase = Phase::NonSoumis;
                        continue;
                    }
                    // Un titre de famille, peut-etre ecrit sur deux rangs.
                    if let Some((tc, te)) = titre_en_cours.take() {
                        let (cle, ecrit) = (format!("{} {}", tc, cl), format!("{} {}", te, r.libelle));
                        if let Some(f) = self.table.famille(&cle) {
                            famille = Some(f.to_string());
                            famille_apres_total |= phase == Phase::ApresTotal;
                            if !r.nombres.is_empty() {
                                lu.ajouter(cotisation(&famille, ecrit, &r), phase == Phase::ApresTotal);
                            }
                            continue;
                        }
                        if r.nombres.is_empty() && self.table.debut_de_famille(&cle) {
                            titre_en_cours = Some((cle, ecrit));
                            continue;
                        }
                        avertissements.push(format!("rang sans montant : « {} »", te));
                    }
                    if let Some(f) = self.table.famille(&cl) {
                        famille = Some(f.to_string());
                        famille_apres_total |= phase == Phase::ApresTotal;
                        if !r.nombres.is_empty() {
                            lu.ajouter(cotisation(&famille, r.libelle.clone(), &r), phase == Phase::ApresTotal);
                        }
                        continue;
                    }
                    if r.nombres.is_empty() && self.table.debut_de_famille(&cl) {
                        titre_en_cours = Some((cl, r.libelle.clone()));
                        continue;
                    }
                    if phase == Phase::ApresTotal && !famille_apres_total {
                        // Ni famille ni net social apres le total : les lignes non soumises commencent.
                        phase = Phase::NonSoumis;
                        lu.lignes.push(ligne(&r, false, &mut avertissements));
                        continue;
                    }
                    if r.nombres.is_empty() {
                        avertissements.push(format!("cotisation sans montant : « {} »", r.libelle));
                    }
                    lu.ajouter(cotisation(&famille, r.libelle.clone(), &r), phase == Phase::ApresTotal);
                }
            }
        }
        self.avertissements.extend(avertissements);
        lu
    }

    /// Une grille du pied : l'en-tete (toutes ses colonnes obligatoires sur un meme rang), puis ses lignes.
    fn grille(&mut self, gr: &Grille, s: Sections, pas: &mut Option<Pas>, net_imposable: &mut Option<Decimal>, cumuls: &mut Vec<(String, Cumul)>) {
        let tol = self.g.tolerance;
        let page = &self.page;
        let pied = |m: &Mot| self.dans_section(m.cy(), Section::Pied, s);
        // Chaque occurrence de la premiere colonne, de haut en bas : la premiere dont le rang porte les autres.
        let mut premiers: Vec<(Boite, usize)> =
            (0..page.mots.len()).filter(|&i| pied(&page.mots[i])).filter_map(|i| page.repere_en(i, &gr.colonnes[0].1)).collect();
        premiers.sort_by(|a, b| a.0.y0.total_cmp(&b.0.y0));
        let mut entete: Option<Vec<(String, Boite, usize)>> = None;
        for (b0, i0) in premiers {
            let mut cols = vec![(gr.colonnes[0].0.clone(), b0, i0)];
            let mut complet = true;
            for (n, phrase, facultative) in &gr.colonnes[1..] {
                let x_min = cols.last().map_or(0.0, |c| c.1.x1);
                let trouve = (0..page.mots.len())
                    .filter(|&i| (page.mots[i].cy() - b0.cy()).abs() <= tol && page.mots[i].x0 >= x_min)
                    .filter_map(|i| page.repere_en(i, phrase))
                    .min_by(|a, b| a.0.x0.total_cmp(&b.0.x0));
                match trouve {
                    Some((b, i)) => cols.push((n.clone(), b, i)),
                    None if *facultative => {}
                    None => {
                        complet = false;
                        break;
                    }
                }
            }
            if complet {
                entete = Some(cols);
                break;
            }
        }
        let Some(mut cols) = entete else { return };
        // L'en-tete d'une colonne continue sur les mots qui le suivent, jusqu'a la colonne suivante (« taux
        // personnalise », « cumul annuel ») : pas au-dela d'un blanc, ni sur ce qui se trouve plus loin sur le rang.
        for k in 0..cols.len() {
            let fin = cols.get(k + 1).map_or(f64::INFINITY, |c| c.1.x0);
            let mut i = cols[k].2;
            while let Some(v) = page.voisins[i].filter(|&v| page.mots[v].x0 < fin) {
                cols[k].1 = cols[k].1.union(&page.mots[v]);
                i = v;
            }
        }
        let cols: Vec<(String, Boite)> = cols.into_iter().map(|(n, b, _)| (n, b)).collect();
        let haut = cols[0].1.cy() + tol;
        let gauche = cols.iter().map(|c| c.1.x0).fold(f64::INFINITY, f64::min) - 2.0;
        let libelles: Vec<&Mot> = page.mots.iter().filter(|m| m.cy() > haut && m.cy() <= haut + GRILLE_HAUTEUR && m.x1 <= gauche && !page.dans_un_nombre(m, gauche)).collect();
        let rangs = en_rangs(libelles, tol);
        let rangs: Vec<(String, f64)> = rangs.iter().map(|r| (canon(&texte_de(r)), milieu(r))).collect();
        let nombres: Vec<&Nombre> = page.nombres.iter().filter(|n| n.boite.cy() > haut - tol && n.boite.cx() > gauche).collect();
        let valeurs_du_rang = |cy: f64| -> Vec<(String, Decimal)> {
            let mut v: Vec<(String, Decimal)> = Vec::new();
            for n in &nombres {
                // Le rang le plus proche de ce nombre doit etre celui-ci.
                let d = (n.boite.cy() - cy).abs();
                if d > tol || rangs.iter().any(|(_, autre)| (n.boite.cy() - autre).abs() < d) {
                    continue;
                }
                let proche = cols.iter().map(|(c, b)| (c, (b.cx() - n.boite.cx()).abs())).min_by(|a, b| a.1.total_cmp(&b.1));
                if let Some((c, e)) = proche {
                    if e <= GRILLE_ECART && !v.iter().any(|(x, _)| x == c) {
                        v.push((c.clone(), n.valeur));
                    }
                }
            }
            v
        };
        for (cible, libs) in &gr.lignes {
            let Some((_, cy)) = rangs.iter().find(|(l, _)| libs.contains(l)) else { continue };
            let v = valeurs_du_rang(*cy);
            let col = |c: &str| v.iter().find(|(x, _)| x == c).map(|(_, y)| *y);
            match cible {
                Cible::Pas if pas.is_none() => *pas = Some(Pas { base: col("base"), taux: col("taux"), montant: col("montant"), cumul: col("cumul") }),
                Cible::NetImposable(c) if net_imposable.is_none() => *net_imposable = col(c),
                Cible::Cumul(n) if !cumuls.iter().any(|(x, _)| x == n) => {
                    cumuls.push((n.clone(), Cumul { periode: col("periode"), cumul: col("cumul") }));
                }
                _ => {}
            }
        }
    }
}

#[derive(Default)]
struct CorpsLu {
    lignes: Vec<LignePaie>,
    brut: Option<Decimal>,
    cotisations: Vec<Cotisation>,
    total_employeur: Option<Decimal>,
    total_salarie: Option<Decimal>,
    net_social: Option<Decimal>,
    net_avant_impot: Option<Decimal>,
}

impl CorpsLu {
    fn ajouter(&mut self, mut c: Cotisation, hors_total: bool) {
        c.hors_total = hors_total;
        self.cotisations.push(c);
    }
}

fn cotisation(famille: &Option<String>, libelle: String, r: &RangCorps) -> Cotisation {
    Cotisation {
        famille: famille.clone(),
        libelle,
        base: r.colonne("base"),
        taux_employeur: r.colonne("taux_employeur"),
        montant_employeur: r.colonne("montant_employeur"),
        taux_salarie: r.colonne("taux_salarie"),
        montant_salarie: r.colonne("montant_salarie"),
        hors_total: false,
    }
}

/// Les controles, sans correction : un ecart est un avertissement.
fn controler(b: &mut BulletinLu) {
    let soumises: Vec<&LignePaie> = b.lignes.iter().filter(|l| l.soumis).collect();
    if soumises.iter().any(|l| l.montant.is_none()) {
        b.avertissements.push("une ligne soumise n'a pas de montant : le brut ne se contrôle pas".into());
    } else if !soumises.is_empty() {
        let somme: Decimal = soumises.iter().filter_map(|l| l.montant).sum();
        if somme != b.brut {
            b.avertissements.push(format!("incohérent : le brut lu ({}) n'est pas la somme des lignes soumises ({})", b.brut, somme));
        }
    }
    if let (Some(avant), Some(salarie)) = (b.net_avant_impot, b.total_salarie) {
        let non_soumis: Decimal = b.lignes.iter().filter(|l| !l.soumis).filter_map(|l| l.montant).sum();
        let hors_total: Decimal = b.cotisations.iter().filter(|c| c.hors_total).filter_map(|c| c.montant_salarie).sum();
        let attendu = b.brut - salarie - hors_total + non_soumis;
        if attendu != avant {
            b.avertissements.push(format!(
                "incohérent : le net avant impôt lu ({}) n'est pas brut - cotisations salariales + lignes non soumises ({})",
                avant, attendu
            ));
        }
    }
    match (b.net_avant_impot, b.pas.as_ref().and_then(|p| p.montant)) {
        (Some(avant), Some(impot)) => {
            if avant - impot != b.net_a_payer {
                b.avertissements.push(format!(
                    "incohérent : le net à payer lu ({}) n'est pas le net avant impôt moins l'impôt prélevé ({})",
                    b.net_a_payer,
                    avant - impot
                ));
            }
        }
        _ => b.avertissements.push("le net à payer ne se contrôle pas (net avant impôt ou impôt prélevé absent)".into()),
    }
}

// --------------------------------------------------------------------------------------------- les compteurs (lot C)

/// Ce qu'un bulletin compte pour les droits, sous la forme de `cumul::Engagement` : les heures sont celles de
/// travail HORS cachets (un cachet se convertit au moment du decompte), l'enseignement est dedans, pas en plus.
/// `None` : inconnu, pas devine.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Compteurs {
    pub cachets: Option<Decimal>,
    /// Les heures de travail hors cachets : heures, services de repetition, enseignement.
    pub heures: Option<Decimal>,
    /// La part de `heures` qui est de l'enseignement (`compte_pour = ouverture_des_droits`).
    pub heures_enseignement: Option<Decimal>,
    /// Les jours travailles : les dates du bulletin (CoolPaie), ou son cumul « Jours » (sPAIEctacle).
    pub jours: Option<Decimal>,
    /// Les desaccords avec les cumuls du bulletin, et les lignes qui n'entrent dans aucun compteur.
    pub avertissements: Vec<String>,
}

impl Compteurs {
    pub fn to_json(&self) -> Value {
        json!({
            "cachets": d(&self.cachets),
            "heures": d(&self.heures),
            "heures_enseignement": d(&self.heures_enseignement),
            "jours": d(&self.jours),
            "avertissements": self.avertissements,
        })
    }
}

fn nombre_ecrit(x: Option<Decimal>) -> String {
    x.map_or_else(|| "?".to_string(), |v| v.normalize().to_string())
}

/// `compteurs(b, racine)` : les cachets, heures, heures d'enseignement et jours d'un bulletin lu.
///
/// Comme pour une saisie (`engagement.rs`, `cumul.rs`), chaque ligne qui porte une rubrique la lit dans le catalogue
/// de la compagnie (`racine`, comme partout dans le moteur ; sans `config/`, le catalogue livre) : une rubrique a
/// l'unite `cachet` compte ses cachets ; une autre, ses heures (le forfait du catalogue, sinon les heures que le
/// libelle dit, sinon la quantite si l'unite est l'heure, sinon rien) ; `compte_pour = ouverture_des_droits` en fait
/// des heures d'enseignement. Une ligne sans rubrique n'entre dans rien, et le dit.
///
/// Puis les cumuls du bulletin (`cachets`, `heures`, `jours`, colonne de la periode), quand il les imprime. Un
/// desaccord est un avertissement, et LE BULLETIN L'EMPORTE sur ce qu'il compte lui-meme : ses cachets, ses jours,
/// ses heures quand aucun cachet ne s'y mele. Un total d'heures ou des cachets sont convertis (sPAIEctacle) se
/// confronte a heures + cachets x forfait, mais ne se decompose pas : on ne devine pas la conversion qu'un logiciel a
/// faite (un cachet groupe a 8 h, la regle d'avant le decret du 13 juillet 2016) ; les compteurs restent ceux des
/// lignes. Une erreur : une rubrique que le catalogue ne connait pas.
pub fn compteurs(b: &BulletinLu, racine: &Path) -> Resultat<Compteurs> {
    let zero = Decimal::ZERO;
    let mut avertissements = Vec::new();
    let (mut cachets, mut heures, mut enseignement) = (Some(zero), Some(zero), Some(zero));
    for l in &b.lignes {
        let Some(code) = &l.rubrique else {
            avertissements.push(format!("« {} » : sans rubrique du catalogue, la ligne n'entre dans aucun compteur", l.libelle));
            continue;
        };
        let r = droit::catalogue::rubrique(racine, code, &b.fin)?;
        let cachet = r.unite == "cachet";
        let forfait = r.heures_forfait.map(socle::num::dec_f64).transpose()?;
        let par_unite = forfait.or(l.heures).or_else(|| (r.unite == "heure").then_some(Decimal::ONE));
        if !cachet && par_unite.is_none() {
            continue; // un defraiement, un acompte : ni cachet ni heure
        }
        let ens = r.compte_pour == "ouverture_des_droits";
        let Some(q) = l.quantite else {
            avertissements.push(format!("« {} » : sans quantité, ses {} ne se comptent pas", l.libelle, if cachet { "cachets" } else { "heures" }));
            if cachet {
                cachets = None;
            } else {
                heures = None;
                if ens {
                    enseignement = None;
                }
            }
            continue;
        };
        if cachet {
            cachets = cachets.map(|c| c + q);
        } else if let Some(u) = par_unite {
            heures = heures.map(|x| x + q * u);
            if ens {
                enseignement = enseignement.map(|x| x + q * u);
            }
        }
    }
    let mut dates = b.jours_travailles.clone();
    dates.sort();
    dates.dedup();
    let mut jours = (!dates.is_empty()).then(|| Decimal::from(dates.len()));

    let imprime = |nom: &str| b.cumuls.iter().find(|(n, _)| n == nom).and_then(|(_, c)| c.periode);
    let mut confronter = |quoi: &str, de: &str, bulletin: Decimal, nous: &mut Option<Decimal>| {
        if nous.is_some_and(|x| x != bulletin) {
            avertissements.push(format!(
                "{} : le bulletin en compte {}, {} {} ; le bulletin l'emporte",
                quoi,
                nombre_ecrit(Some(bulletin)),
                de,
                nombre_ecrit(*nous)
            ));
        }
        *nous = Some(bulletin);
    };
    if let Some(x) = imprime("cachets") {
        confronter("cachets", "ses lignes", x, &mut cachets);
    }
    if let Some(x) = imprime("jours") {
        confronter("jours", "ses dates travaillées", x, &mut jours);
    }
    if let Some(x) = imprime("heures") {
        if cachets == Some(zero) {
            confronter("heures", "ses lignes", x, &mut heures);
        } else {
            let forfait = droit::catalogue::rubrique(racine, "cachet", &b.fin)?.heures_forfait.map(socle::num::dec_f64).transpose()?;
            let ensemble = match (heures, cachets, forfait) {
                (Some(h), Some(c), Some(f)) => Some(h + c * f),
                _ => None,
            };
            if ensemble != Some(x) {
                avertissements.push(format!(
                    "heures : le bulletin en compte {}, cachets convertis compris ; ses lignes {} ({} cachet(s) à {} h et {} h) ; un total où des cachets sont convertis ne se décompose pas : les compteurs restent ceux des lignes",
                    nombre_ecrit(Some(x)),
                    nombre_ecrit(ensemble),
                    nombre_ecrit(cachets),
                    nombre_ecrit(forfait),
                    nombre_ecrit(heures)
                ));
            }
        }
    }
    // L'enseignement est une part des heures : jamais plus qu'elles (comme `cumul::engagement_de_paie`).
    if let (Some(e), Some(h)) = (enseignement, heures) {
        enseignement = Some(e.min(h));
    }
    let net = |x: Option<Decimal>| x.map(|v| v.normalize());
    Ok(Compteurs { cachets: net(cachets), heures: net(heures), heures_enseignement: net(enseignement), jours: net(jours), avertissements })
}
