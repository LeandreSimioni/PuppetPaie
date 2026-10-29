//! Le lecteur de bulletins : une page de PDF devient un `BulletinLu`, TEL QU'IL EST, quel que soit le logiciel qui
//! l'a fait. Le bulletin fait foi : rien ne se recalcule, chaque montant est un `Decimal` lu sur la page
//! (« 1 421,13 » -> 1421.13).
//!
//! La page se lit par `fabrique_pdf::inspection::inspecter` : chaque mot et sa boite. Le texte dans l'ordre du flux
//! ne suffit pas : le tableau des cotisations a plusieurs colonnes, et seule la position en x dit dans laquelle tombe
//! un nombre.
//!
//! CE QUE TOUT BULLETIN IMPRIME. Le code du travail fixe ses mentions (art. R. 3243-1 : le nom et l'adresse de
//! l'employeur, son code APE et son Siret, la convention, le nom et l'emploi du salarie, la periode, le brut, les
//! cotisations, l'impot preleve, la somme versee, la date du paiement, le total verse par l'employeur ;
//! `atelier/reference/sources/code-du-travail/articles-20261028.md`) et le modele de l'arrete du 25 fevrier 2016 leurs
//! libelles, leur ordre et leurs regroupements (art. R. 3243-2 ; `atelier/reference/sources/montant-net-social/
//! legi-arrete-20160225-bulletin/LEGIARTI000049854119.xml`) : « Santé », « Retraite »..., le total des cotisations, le
//! net social, le net avant impot. Sa mise en page ne l'est pas. Le lecteur n'en connait donc aucune ; il trouve
//! chaque chose par ce qui l'annonce, dans la table livree `bulletins-lecture-base.json` (sa cle `_forme` la decrit) :
//! - une MENTION par son libelle (`mentions`) : sa valeur est ce qui le suit sur son rang ;
//! - le TABLEAU par son rang d'en-tetes, un « Libellé » suivi de « Base », « Taux », « Part salarié »... (`entetes`) :
//!   une colonne, c'est l'en-tete qui la titre, sur un rang ou sur deux ; un rang d'en-tetes plus bas (« Taux
//!   Montant » sous « Part employeur ») la precise ;
//! - les rangs du tableau par les libelles du modele (`resumes`, `familles`, `titres`) ;
//! - le pied : l'impot preleve et le net imposable sous leur rang d'en-tetes (« base », « taux », « montant »), les
//!   cumuls sous « Période » et « Cumul » (`cumuls`) ;
//! - les deux ADRESSES (employeur, salarie) sans libelle : chacune est un bloc de rangs alignes dont un porte un code
//!   postal ; celui de l'employeur est suivi de son Siret, de son code APE ou de sa convention.
//! Un logiciel connu se reconnait a sa signature (`logiciels`) : le compte rendu le nomme, la lecture n'en fait rien.
//!
//! RIEN NE SE DEVINE (regle 1). Une page :
//! - sans un mot (lettres en traces, scan : pas d'OCR) -> refus `sans texte` ;
//! - dont le rang d'en-tetes du tableau ne se trouve pas -> refus `sans en-têtes` ;
//! - qui se lit de deux facons (deux rangs d'en-tetes, une colonne « Montant » qui ne dit pas quelle part, deux
//!   colonnes de la meme part, deux adresses de salarie) -> refus `ambigu` ;
//! - ou ne se lisent pas ce qui identifie le bulletin (le salarie, la periode, le brut, le net a payer, ou a defaut
//!   le net avant impot et l'impot preleve qui le donnent) -> refus `incomplet`, qui nomme la mention ;
//! - l'annexe d'un bulletin (le detail de ses cotisations) n'est pas un bulletin -> refus `annexe`.
//!
//! Le reste est facultatif. Deux controles, SANS CORRECTION : le brut vaut la somme des lignes soumises (une ligne
//! dont la rubrique est hors de l'assiette, `soumis: false` du catalogue, n'en est pas, ou qu'elle soit imprimee), le net a
//! payer se retrouve (net avant impot = brut - cotisations salariales + lignes non soumises ; net a payer = net
//! avant impot - impot preleve). Un ecart va dans `avertissements`, le bulletin est rendu quand meme. Un libelle de
//! ligne absent de la table est signale de meme, et la ligne gardee telle quelle.
//!
//! LES LIGNES ET LES COMPTEURS. La table dit la rubrique du catalogue que porte chaque libelle de ligne (`lignes` ;
//! `null` : connu, sans rubrique sure). `compteurs` en tire les cachets, heures, heures d'enseignement et jours d'un
//! bulletin comme pour une saisie (le catalogue dit l'unite, le forfait de 12 h d'un cachet, ce qui compte pour
//! l'ouverture des droits), et les confronte aux cumuls que le bulletin imprime : un desaccord est un avertissement,
//! et le bulletin l'emporte sur ce qu'il compte lui-meme.

use std::path::Path;
use std::str::FromStr;

use serde_json::{json, Map, Value};

use socle::date::Jour;
use socle::num::{Decimal, Resultat};

/// La table de lecture, dans `logiciel/donnees/`.
pub const FICHIER_TABLE: &str = "bulletins-lecture-base.json";

/// Deux mots dont le milieu vertical s'ecarte de moins que cela sont sur un meme rang (en points).
const TOLERANCE: f64 = 3.5;

/// Un mot plus loin que cela du precedent ne continue pas un libelle.
const ECART_MOTS: f64 = 15.0;

/// Deux morceaux d'un nombre (« 1 » et « 421,13 ») separes de moins que cela forment un seul nombre (l'espace des
/// milliers grandit avec le corps du texte : 2 points en 9, 4 en 14).
const ECART_MILLIERS: f64 = 5.0;

/// Un montant « sous » son libelle : au plus a cette distance verticale.
const DESSOUS_MAX: f64 = 40.0;

/// Les rangs d'une grille du pied : au plus a cette distance sous son en-tete.
const GRILLE_HAUTEUR: f64 = 60.0;

/// Une periode de dates travaillees plus longue que cela n'est pas depliee en jours.
const INTERVALLE_MAX: i64 = 366;

/// Sur un rang de l'en-tete, deux mots separes de plus que cela sont deux morceaux (un libelle et sa valeur, deux
/// colonnes d'adresses) ; « 99999 » et sa commune, ecrits en deux, restent un.
const ECART_SEGMENT: f64 = 40.0;

/// La valeur d'une mention commence au plus a cette distance de son libelle, sur son rang.
const VALEUR_LOIN: f64 = 180.0;

/// Les mots d'une valeur se suivent a moins de cet ecart.
const VALEUR_SUITE: f64 = 40.0;

/// Une valeur sur plusieurs rangs : chaque rang au plus a cette distance du precedent.
const VALEUR_RANG: f64 = 14.0;

/// Les rangs d'un bloc d'adresse se suivent a moins de cet ecart vertical.
const BLOC_ECART: f64 = 30.0;

/// Ce qui suit un bloc d'adresse dans sa colonne (le Siret de l'employeur) : au plus a cet ecart vertical.
const BLOC_SUIVANT: f64 = 35.0;

/// Deux morceaux de l'en-tete commencent dans la meme colonne a moins de cela.
const MEME_COLONNE: f64 = 2.5;

/// Les rangs d'en-tetes du tableau : au plus a cet ecart vertical du « Libellé ».
const ENTETE_HAUTEUR: f64 = 12.0;

/// Les mots d'un meme en-tete de colonne, sur un rang (« Part Employeur ») : au plus cet ecart.
const ENTETE_MOTS: f64 = 4.0;

/// Un en-tete precise celui du dessus (« Taux » sous « Part employeur ») si son milieu tombe a moins de cela de lui.
const SOUS_ENTETE: f64 = 25.0;

/// Un en-tete aligne a droite sous un en-tete de part (« Montant » sous « Part employeur ») en depasse le bord droit
/// d'au plus cela.
const GROUPE_MARGE: f64 = 10.0;

/// Un nombre aligne a droite depasse le bord droit de l'en-tete de sa colonne d'au plus cela.
const COLONNE_MARGE: f64 = 20.0;

/// Le libelle d'un rang finit au moins a cette distance avant la premiere colonne.
const LIBELLE_MARGE: f64 = 15.0;

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
/// decimal s'il n'y a pas de virgule (« 0.000 % »). Le signe moins en tete.
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

/// Un mot qui n'est que ponctuation (« — », « · », « : », « - », « | ») : il borne une valeur, il n'en est pas.
fn ponctuation(t: &str) -> bool {
    !t.is_empty() && !t.chars().any(char::is_alphanumeric)
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

/// Une boite : celle d'un libelle trouve (union de ses mots), ou d'un nombre.
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
        self.unir(Boite::de(m))
    }
    fn unir(self, b: Boite) -> Boite {
        Boite { x0: self.x0.min(b.x0), y0: self.y0.min(b.y0), x1: self.x1.max(b.x1), y1: self.y1.max(b.y1) }
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
    fn nouvelle(mut mots: Vec<Mot>) -> Page {
        mots.sort_by(|a, b| a.cy().total_cmp(&b.cy()).then(a.x0.total_cmp(&b.x0)));
        let voisins = (0..mots.len())
            .map(|i| {
                let m = &mots[i];
                (0..mots.len())
                    .filter(|&j| j != i && (mots[j].cy() - m.cy()).abs() <= TOLERANCE && mots[j].x0 >= m.x1 - 1.0 && mots[j].x0 - m.x1 <= ECART_MOTS)
                    .min_by(|&a, &b| mots[a].x0.total_cmp(&mots[b].x0))
            })
            .collect();
        let nombres = nombres_de(&mots);
        Page { mots, nombres, voisins }
    }

    /// Le libelle `phrase` commence-t-il au mot `i` ? Sa boite, et son dernier mot.
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
        // Un libelle finit avec un mot : « Statut » ne se lit pas dans « Statutaire ».
        (k == self.mots[mot].jetons.len()).then_some((boite, mot))
    }

    /// Le plus long des libelles `phrases` qui commence au mot `i`.
    fn libelle_en(&self, i: usize, phrases: &[Vec<String>]) -> Option<(Boite, usize)> {
        phrases.iter().filter_map(|p| self.repere_en(i, p).map(|r| (p.len(), r))).max_by_key(|(n, _)| *n).map(|(_, r)| r)
    }

    /// Chaque occurrence d'un des libelles, de haut en bas puis de gauche a droite, parmi les mots que `garder`
    /// retient : (sa boite, son premier mot, son dernier mot).
    fn occurrences(&self, phrases: &[Vec<String>], garder: &dyn Fn(&Mot) -> bool) -> Vec<(Boite, usize, usize)> {
        let mut ordre: Vec<usize> = (0..self.mots.len()).filter(|&i| garder(&self.mots[i])).collect();
        ordre.sort_by(|&a, &b| self.mots[a].y0.total_cmp(&self.mots[b].y0).then(self.mots[a].x0.total_cmp(&self.mots[b].x0)));
        ordre.into_iter().filter_map(|i| self.libelle_en(i, phrases).map(|(b, f)| (b, i, f))).collect()
    }

    /// Le mot est-il un morceau d'un nombre dont le bord droit est a droite de `limite` (le « 1 » de « 1 421,13 ») ?
    fn dans_un_nombre(&self, m: &Mot, limite: f64) -> bool {
        self.nombres.iter().any(|n| {
            let b = n.boite;
            b.x1 > limite && m.x0 >= b.x0 - 0.01 && m.x1 <= b.x1 + 0.01 && (m.cy() - b.cy()).abs() < 1.0
        })
    }

    fn present(&self, phrase: &[String]) -> bool {
        (0..self.mots.len()).any(|i| self.repere_en(i, phrase).is_some())
    }

    fn indice(&self, m: &Mot) -> usize {
        self.mots.iter().position(|x| std::ptr::eq(x, m)).unwrap_or(0)
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

/// Des mots en rangs (milieu vertical a moins de `TOLERANCE` du premier mot du rang), chaque rang de gauche a droite.
fn en_rangs<'a>(mut mots: Vec<&'a Mot>) -> Vec<Vec<&'a Mot>> {
    mots.sort_by(|a, b| a.cy().total_cmp(&b.cy()));
    let mut rangs: Vec<Vec<&Mot>> = Vec::new();
    for m in mots {
        match rangs.last_mut() {
            Some(r) if (m.cy() - r[0].cy()).abs() <= TOLERANCE => r.push(m),
            _ => rangs.push(vec![m]),
        }
    }
    for r in rangs.iter_mut() {
        r.sort_by(|a, b| a.x0.total_cmp(&b.x0));
    }
    rangs
}

/// Une ligne de paie qui ecrit son taux dans son libelle (« Cachets à 150,00 ») : le libelle sans lui, et le taux.
/// Sans « à <nombre> » en fin de libelle, le libelle tel quel.
fn taux_du_libelle(libelle: &str) -> (String, Option<Decimal>) {
    let mots: Vec<&str> = libelle.split_whitespace().collect();
    // Le dernier « à » suivi d'un nombre seul, milliers compris (« à 2 950,00 »).
    for k in (1..mots.len().saturating_sub(1)).rev() {
        if canon(mots[k]) == "a" {
            if let Some(t) = lire_nombre(&mots[k + 1..].join(" ")) {
                return (mots[..k].join(" "), Some(t));
            }
            break;
        }
    }
    (libelle.to_string(), None)
}

/// Un libelle canonique est-il l'un de ces resumes, seul ou joint a un autre par « / » (« NET SOCIAL / NET A PAYER
/// AVANT IMPÔT ») ?
fn resume(canon: &str, liste: &[String]) -> bool {
    liste.iter().any(|x| x == canon) || canon.split(" / ").any(|p| liste.iter().any(|x| x == p))
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

#[derive(Clone, Copy, Debug, PartialEq)]
enum Section {
    /// Au-dessus du tableau.
    Entete,
    /// Sous le net avant impot.
    Pied,
    Page,
}

/// Chaque mention : son type, ou elle se cherche, combien de rangs sa valeur peut couvrir, et si elle dit
/// l'employeur (elle suit alors son adresse). Le type est fixe ici : la table dit comment la mention s'annonce, pas ce
/// qu'elle est.
const MENTIONS: [(&str, Sorte, Section, usize, bool); 23] = [
    ("numero", Sorte::Texte, Section::Entete, 1, false),
    ("numero_objet", Sorte::Compact, Section::Entete, 1, false),
    ("spectacle", Sorte::Texte, Section::Entete, 1, false),
    ("analytique", Sorte::Texte, Section::Entete, 1, false),
    ("siret", Sorte::Compact, Section::Entete, 1, true),
    ("naf", Sorte::Texte, Section::Entete, 1, true),
    ("urssaf", Sorte::Compact, Section::Entete, 1, true),
    ("convention", Sorte::Texte, Section::Entete, 2, true),
    ("licence", Sorte::Texte, Section::Entete, 1, true),
    ("naissance", Sorte::Date, Section::Entete, 1, false),
    ("nir", Sorte::Compact, Section::Entete, 1, false),
    ("code", Sorte::Texte, Section::Entete, 1, false),
    ("emploi", Sorte::Texte, Section::Entete, 1, false),
    ("statut", Sorte::Texte, Section::Entete, 1, false),
    ("abattement", Sorte::Nombre, Section::Entete, 1, false),
    ("conges_spectacles", Sorte::Compact, Section::Entete, 1, false),
    ("periode", Sorte::Periode, Section::Entete, 1, false),
    ("dates_travaillees", Sorte::Intervalles, Section::Entete, 12, false),
    ("regle_le", Sorte::Date, Section::Page, 1, false),
    ("mode_reglement", Sorte::Texte, Section::Page, 1, false),
    ("contrat", Sorte::Texte, Section::Page, 1, false),
    ("net_a_payer", Sorte::Nombre, Section::Pied, 1, false),
    ("cout_employeur", Sorte::Nombre, Section::Pied, 1, false),
];

#[derive(Clone, Debug)]
struct Mention {
    nom: &'static str,
    sorte: Sorte,
    section: Section,
    rangs: usize,
    employeur: bool,
    libelles: Vec<Vec<String>>,
}

/// Ce qu'un en-tete de colonne annonce.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Genre {
    Base,
    Taux,
    Montant,
    Periode,
    Cumul,
}

/// La part qu'une colonne porte.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Part {
    Salarie,
    Employeur,
}

/// Les mots des en-tetes, par ce qu'ils annoncent (`entetes` de la table).
#[derive(Clone, Debug, Default)]
struct Entetes {
    libelle: Vec<String>,
    base: Vec<String>,
    taux: Vec<String>,
    montant: Vec<String>,
    salarie: Vec<String>,
    employeur: Vec<String>,
    periode: Vec<String>,
    cumul: Vec<String>,
}

impl Entetes {
    fn tous(&self) -> impl Iterator<Item = &String> {
        [&self.libelle, &self.base, &self.taux, &self.montant, &self.salarie, &self.employeur, &self.periode, &self.cumul].into_iter().flatten()
    }

    /// Le genre et la part d'un en-tete, par ses jetons. Une part seule (« Salarié ») annonce un montant.
    fn lire(&self, jetons: &[String]) -> (Option<Genre>, Option<Part>) {
        let a = |l: &[String]| jetons.iter().any(|j| l.contains(j));
        let part = match (a(&self.salarie), a(&self.employeur)) {
            (true, false) => Some(Part::Salarie),
            (false, true) => Some(Part::Employeur),
            _ => None,
        };
        let genre = if a(&self.base) {
            Some(Genre::Base)
        } else if a(&self.taux) {
            Some(Genre::Taux)
        } else if a(&self.periode) {
            Some(Genre::Periode)
        } else if a(&self.cumul) {
            Some(Genre::Cumul)
        } else if a(&self.montant) || part.is_some() {
            Some(Genre::Montant)
        } else {
            None
        };
        (genre, part)
    }
}

/// Les libelles des rangs qui resument le tableau, canoniques.
#[derive(Clone, Debug, Default)]
struct Resumes {
    brut: Vec<String>,
    total: Vec<String>,
    net_social: Vec<String>,
    net_avant_impot: Vec<String>,
    pas: Vec<String>,
    net_imposable: Vec<String>,
}

/// Un libelle de ligne de paie que la table connait.
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
    /// La rubrique est hors de l'assiette (`soumis: false` du catalogue livre : un defraiement, un acompte) ; `false`
    /// aussi pour un code que le catalogue livre ne connait pas (les compteurs le refusent).
    pub non_soumise: bool,
}

/// La table de lecture, verifiee a la lecture.
#[derive(Clone, Debug)]
pub struct Table {
    mentions: Vec<Mention>,
    entetes: Entetes,
    resumes: Resumes,
    /// Les titres de sections du tableau qui ne sont ni une famille ni une ligne, canoniques.
    titres: Vec<String>,
    /// Ce qui ouvre l'annexe d'un bulletin.
    annexe: Vec<Vec<String>>,
    /// (cle, titres canoniques).
    familles: Vec<(String, Vec<String>)>,
    /// (cle, libelles canoniques).
    cumuls: Vec<(String, Vec<String>)>,
    /// (nom, signature : les libelles qui doivent tous se trouver).
    logiciels: Vec<(String, Vec<Vec<String>>)>,
    lignes: Vec<LigneTable>,
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
            return Err(format!("{} : un libellé vide", ou));
        }
        sortie.push(p);
    }
    if sortie.is_empty() {
        return Err(format!("{} : aucun libellé", ou));
    }
    Ok(sortie)
}

fn libelles(v: Option<&Value>, ou: &str) -> Resultat<Vec<String>> {
    Ok(phrases(v, ou)?.into_iter().map(|p| p.join(" ")).collect())
}

/// Des mots seuls (un en-tete de colonne se lit mot a mot).
fn mots_seuls(v: Option<&Value>, ou: &str) -> Resultat<Vec<String>> {
    let p = phrases(v, ou)?;
    if let Some(x) = p.iter().find(|x| x.len() != 1) {
        return Err(format!("{} : « {} » n'est pas un mot seul", ou, x.join(" ")));
    }
    Ok(p.into_iter().map(|mut x| x.remove(0)).collect())
}

/// Un objet de listes de libelles : (cle, libelles canoniques).
fn groupes(v: Option<&Value>, ou: &str) -> Resultat<Vec<(String, Vec<String>)>> {
    let o = v.and_then(Value::as_object).ok_or_else(|| format!("table de lecture : `{}` manque", ou))?;
    o.iter().filter(|(k, _)| !k.starts_with('_')).map(|(k, x)| Ok((k.clone(), libelles(Some(x), &format!("{}.{}", ou, k))?))).collect()
}

impl Table {
    /// La table livree (`logiciel/donnees/bulletins-lecture-base.json`).
    pub fn livree() -> Resultat<Table> {
        let chemin = socle::fichiers::dossier_du_code().join(FICHIER_TABLE);
        Table::depuis(&*socle::fichiers::json_en_cache(&chemin)?)
    }

    /// Une table, verifiee : une mention, un en-tete, un resume ou une forme inconnue est un refus qui le nomme.
    pub fn depuis(v: &Value) -> Resultat<Table> {
        let m = v.get("mentions").and_then(Value::as_object).ok_or("table de lecture : `mentions` manque")?;
        let mut mentions = Vec::new();
        for (n, x) in m.iter().filter(|(k, _)| !k.starts_with('_')) {
            let &(nom, sorte, section, rangs, employeur) =
                MENTIONS.iter().find(|c| c.0 == n).ok_or_else(|| format!("mentions.{} : mention inconnue", n))?;
            mentions.push(Mention { nom, sorte, section, rangs, employeur, libelles: phrases(Some(x), &format!("mentions.{}", n))? });
        }
        let e = v.get("entetes").ok_or("table de lecture : `entetes` manque")?;
        if let Some(k) = e.as_object().and_then(|o| o.keys().find(|k| !k.starts_with('_') && !matches!(k.as_str(), "libelle" | "base" | "taux" | "montant" | "salarie" | "employeur" | "periode" | "cumul"))) {
            return Err(format!("entetes.{} : en-tête inconnu", k));
        }
        let mot = |k: &str| mots_seuls(e.get(k), &format!("entetes.{}", k));
        let entetes = Entetes {
            libelle: mot("libelle")?,
            base: mot("base")?,
            taux: mot("taux")?,
            montant: mot("montant")?,
            salarie: mot("salarie")?,
            employeur: mot("employeur")?,
            periode: mot("periode")?,
            cumul: mot("cumul")?,
        };
        let r = groupes(v.get("resumes"), "resumes")?;
        if let Some((k, _)) = r.iter().find(|(k, _)| !matches!(k.as_str(), "brut" | "total" | "net_social" | "net_avant_impot" | "pas" | "net_imposable")) {
            return Err(format!("resumes.{} : résumé inconnu", k));
        }
        let resume = |k: &str| r.iter().find(|(x, _)| x == k).map(|(_, l)| l.clone()).ok_or_else(|| format!("resumes.{} manque", k));
        let resumes = Resumes {
            brut: resume("brut")?,
            total: resume("total")?,
            net_social: resume("net_social")?,
            net_avant_impot: resume("net_avant_impot")?,
            pas: resume("pas")?,
            net_imposable: resume("net_imposable")?,
        };
        let mut logiciels = Vec::new();
        if let Some(o) = v.get("logiciels").and_then(Value::as_object) {
            for (k, x) in o.iter().filter(|(k, _)| !k.starts_with('_')) {
                logiciels.push((k.clone(), phrases(x.get("signature"), &format!("logiciels.{}.signature", k))?));
            }
        }
        Ok(Table {
            mentions,
            entetes,
            resumes,
            titres: libelles(v.get("titres"), "titres")?,
            annexe: phrases(v.get("annexe"), "annexe")?,
            familles: groupes(v.get("familles"), "familles")?,
            cumuls: groupes(v.get("cumuls"), "cumuls")?,
            logiciels,
            lignes: lignes_de(v.get("lignes"))?,
        })
    }

    /// Les libelles de lignes de paie que la table connait, et leur rubrique.
    pub fn lignes(&self) -> &[LigneTable] {
        &self.lignes
    }

    fn famille(&self, libelle: &str) -> Option<&str> {
        self.familles.iter().find(|(_, t)| t.iter().any(|x| x == libelle)).map(|(c, _)| c.as_str())
    }

    /// Un titre de famille commence-t-il par ce libelle (titre ecrit sur deux rangs) ?
    fn debut_de_famille(&self, libelle: &str) -> bool {
        let debut = format!("{} ", libelle);
        self.familles.iter().any(|(_, t)| t.iter().any(|x| x.starts_with(&debut)))
    }

    fn mention(&self, nom: &str) -> Option<&Mention> {
        self.mentions.iter().find(|m| m.nom == nom)
    }
}

fn lignes_de(v: Option<&Value>) -> Resultat<Vec<LigneTable>> {
    let catalogue = socle::fichiers::json_en_cache(&socle::fichiers::dossier_du_code().join("rubriques-base.json"))?;
    let hors_assiette = |code: &str| catalogue["rubriques"][code]["soumis"] == Value::Bool(false);
    let mut lignes: Vec<LigneTable> = Vec::new();
    let Some(l) = v.and_then(Value::as_object) else { return Err("table de lecture : `lignes` manque".into()) };
    for (libelle, valeur) in l.iter().filter(|(k, _)| !k.starts_with('_')) {
        let o = format!("lignes.{}", libelle);
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
        let non_soumise = rubrique.as_deref().is_some_and(hors_assiette);
        lignes.push(LigneTable { libelle: libelle.clone(), canon: c, rubrique, heures, non_soumise });
    }
    Ok(lignes)
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
    /// Le nom tel qu'il est ecrit (premier rang de son adresse).
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

/// Une ligne de paie : soumise (imprimee avant le brut, d'une rubrique dans l'assiette) ou non (un defraiement, apres
/// les cotisations ou au-dessus du brut).
#[derive(Clone, Debug, PartialEq)]
pub struct LignePaie {
    pub quantite: Option<Decimal>,
    pub libelle: String,
    pub taux: Option<Decimal>,
    pub montant: Option<Decimal>,
    pub soumis: bool,
    /// La rubrique du catalogue que la table donne a ce libelle ; `None` : libelle connu sans rubrique sure, ou absent
    /// de la table (alors signale).
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
    /// Le logiciel que sa signature nomme (`logiciels` de la table) ; `None` : un autre.
    pub logiciel: Option<String>,
    pub numero: Option<String>,
    pub entreprise: Entreprise,
    pub salarie: Salarie,
    pub numero_objet: Option<String>,
    /// Le titre du spectacle.
    pub spectacle: Option<String>,
    /// Le code analytique.
    pub analytique: Option<String>,
    /// Le numero du contrat, quand le bulletin l'imprime.
    pub contrat: Option<String>,
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
    /// Les cumuls, dans l'ordre de la page : `cachets`, `jours`, `heures`, `brut`, `plafond`, `cout_employeur`...
    pub cumuls: Vec<(String, Cumul)>,
    pub avertissements: Vec<String>,
}

/// Pourquoi une page n'est pas un bulletin lu.
#[derive(Clone, Debug, PartialEq)]
pub enum Refus {
    /// Aucun mot : lettres en traces ou page scannee (pas d'OCR). Le detail dit ce que la fabrique a vu.
    SansTexte(String),
    /// Le rang d'en-tetes du tableau ne se trouve pas : le detail dit ce qui manquait.
    SansEntetes(String),
    /// La page se lit de deux facons : le detail dit lesquelles.
    Ambigu(String),
    /// L'annexe d'un bulletin, qui en detaille les cotisations : rien a reprendre.
    Annexe,
    /// Un libelle de ligne de paie absent de la table. Il n'est que SIGNALE (son message va dans les avertissements) :
    /// la ligne est gardee telle quelle, la page lue.
    LibelleInconnu(String),
    /// Une mention qui identifie le bulletin ne se lit pas (salarie, periode, brut, net a payer).
    Incomplet(String),
    /// La page se contredit au point de ne pas se lire.
    Incoherent(String),
}

impl Refus {
    pub fn code(&self) -> &'static str {
        match self {
            Refus::SansTexte(_) => "sans_texte",
            Refus::SansEntetes(_) => "sans_entetes",
            Refus::Ambigu(_) => "ambigu",
            Refus::Annexe => "annexe",
            Refus::LibelleInconnu(_) => "libelle_inconnu",
            Refus::Incomplet(_) => "incomplet",
            Refus::Incoherent(_) => "incoherent",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Refus::SansTexte(d) if d.is_empty() => "sans texte".into(),
            Refus::SansTexte(d) => format!("sans texte : {}", d),
            Refus::SansEntetes(d) => format!("sans en-têtes : {}", d),
            Refus::Ambigu(d) => format!("ambigu : {}", d),
            Refus::Annexe => "annexe d'un bulletin : le détail de ses cotisations, rien à reprendre".into(),
            Refus::LibelleInconnu(l) => format!("libellé inconnu : « {} »", l),
            Refus::Incomplet(d) => format!("incomplet : {}", d),
            Refus::Incoherent(d) => format!("incohérent : {}", d),
        }
    }

    /// La raison seule, sans le sort : le compte rendu de l'import montre le sort en pastille, à côté.
    pub fn raison(&self) -> String {
        match self {
            Refus::SansTexte(d) if d.is_empty() => "aucun mot sur la page".into(),
            Refus::SansTexte(d) | Refus::SansEntetes(d) | Refus::Ambigu(d) | Refus::Incomplet(d) | Refus::Incoherent(d) => {
                d.clone()
            }
            Refus::Annexe => "le détail des cotisations d'un bulletin : rien à reprendre".into(),
            Refus::LibelleInconnu(l) => format!("« {} » n'est pas dans la table de lecture", l),
        }
    }
}

/// Une page lue : un bulletin, ou le refus qui dit pourquoi pas.
#[derive(Clone, Debug)]
pub struct Lecture {
    /// La page dans le PDF, a partir de 1.
    pub page: usize,
    pub resultat: Result<BulletinLu, Refus>,
    /// Une page refusee dont le salarie s'est lu : ce qu'il en a rendu (nom, NIR, adresse...), pour la caviarder.
    /// `None` pour un bulletin lu, ou quand rien n'a ete reconnu.
    pub partiel: Option<Salarie>,
}

impl Lecture {
    pub fn to_json(&self) -> Value {
        match &self.resultat {
            Ok(b) => json!({"page": self.page, "bulletin": b.to_json()}),
            Err(r) => json!({"page": self.page, "refus": r.code(), "raison": r.raison()}),
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
    /// Les montants en chaines (`"1421.13"`, exacts), les dates en ISO, l'absent en `null`.
    pub fn to_json(&self) -> Value {
        let e = &self.entreprise;
        let s = &self.salarie;
        let mut cumuls = Map::new();
        for (n, c) in &self.cumuls {
            cumuls.insert(n.clone(), json!({"periode": d(&c.periode), "cumul": d(&c.cumul)}));
        }
        json!({
            "logiciel": t(&self.logiciel),
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
            "contrat": t(&self.contrat),
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
        // Un meme mot ecrit deux fois au meme endroit (des cumuls imprimes en double) ne compte qu'une fois.
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
    let page = Page::nouvelle(mots);
    if table.annexe.iter().any(|a| page.present(a)) {
        return (Err(Refus::Annexe), None);
    }
    let logiciel = table.logiciels.iter().find(|(_, s)| s.iter().all(|x| page.present(x))).map(|(n, _)| n.clone());
    let debuts = (0..page.mots.len())
        .map(|i| table.mentions.iter().position(|m| page.libelle_en(i, &m.libelles).is_some()))
        .collect();
    let mut l = Lecteur { t: table, page, debuts, pris: Vec::new(), avertissements: Vec::new(), partiel: None };
    let r = l.lire(logiciel);
    (r, l.partiel)
}

/// Une valeur de mention, lue.
#[derive(Clone, Debug)]
enum Valeur {
    Texte(String),
    Date(Jour),
    Periode(Jour, Jour),
    Intervalles(Vec<(Jour, Jour)>),
    Nombre(Decimal),
}

/// Une colonne de tableau : ce que son en-tete annonce, et sa boite.
#[derive(Clone, Debug)]
struct Colonne {
    genre: Genre,
    part: Option<Part>,
    boite: Boite,
    texte: String,
}

/// Les colonnes, de gauche a droite par leur bord droit : un nombre aligne a droite va a la premiere dont le bord
/// droit (plus `COLONNE_MARGE`) le couvre.
fn colonne_de<'c>(cols: &'c [Colonne], n: &Nombre) -> Option<&'c Colonne> {
    cols.iter().find(|c| n.boite.x1 <= c.boite.x1 + COLONNE_MARGE)
}

fn trier_colonnes(cols: &mut [Colonne]) {
    cols.sort_by(|a, b| a.boite.x1.total_cmp(&b.boite.x1));
}

/// Les en-tetes de colonnes d'un ensemble de mots : les mots voisins d'un rang s'unissent (« Part Employeur »), puis
/// ceux de rangs differents qui se chevauchent (« Taux » sur « salarial »).
fn etiquettes(mots: &[&Mot], e: &Entetes) -> Vec<(Boite, Vec<String>, String)> {
    let part_dans = |j: &[String]| j.iter().any(|x| e.salarie.contains(x) || e.employeur.contains(x));
    let mut sortie: Vec<(Boite, Vec<String>, String)> = Vec::new();
    for rang in en_rangs(mots.to_vec()) {
        let mut du_rang: Vec<(Boite, Vec<String>, String)> = Vec::new();
        for m in rang {
            match du_rang.last_mut() {
                Some((b, j, t)) if m.x0 - b.x1 <= ENTETE_MOTS => {
                    *b = b.union(m);
                    j.extend(m.jetons.iter().cloned());
                    t.push(' ');
                    t.push_str(&m.texte);
                }
                _ => du_rang.push((Boite::de(m), m.jetons.clone(), m.texte.clone())),
            }
        }
        // Un en-tete de part du dessus qui en titre plusieurs de ce rang les groupe (« Part Employeur » sur « Taux
        // Montant ») : chacun prend ses mots, et il s'efface. Les colonnes etant alignees a droite, un en-tete du
        // dessous va au premier en-tete de part dont le bord droit (plus `GROUPE_MARGE`) le couvre. Sinon, un en-tete
        // qui en chevauche un du dessus est le meme, ecrit sur deux rangs (« Taux » sur « salarial »).
        let chevauche = |a: &Boite, b: &Boite| a.x0 < b.x1 && b.x0 < a.x1;
        let mut parts: Vec<usize> = (0..sortie.len()).filter(|&k| part_dans(&sortie[k].1)).collect();
        parts.sort_by(|&a, &b| sortie[a].0.x1.total_cmp(&sortie[b].0.x1));
        let titre: Vec<Option<usize>> = du_rang.iter().map(|e| parts.iter().copied().find(|&k| e.0.x1 <= sortie[k].0.x1 + GROUPE_MARGE)).collect();
        let groupes: Vec<usize> = parts.iter().copied().filter(|&k| titre.iter().filter(|t| **t == Some(k)).count() >= 2).collect();
        let mut enfants: Vec<bool> = vec![false; du_rang.len()];
        for (i, e) in du_rang.iter_mut().enumerate() {
            if let Some(k) = titre[i].filter(|k| groupes.contains(k)) {
                e.1.extend(sortie[k].1.iter().cloned());
                enfants[i] = true;
            }
        }
        let mut k = 0;
        sortie.retain(|_| {
            k += 1;
            !groupes.contains(&(k - 1))
        });
        for (e, enfant) in du_rang.into_iter().zip(enfants) {
            match sortie.iter_mut().find(|(b, _, _)| !enfant && chevauche(b, &e.0)) {
                Some((b, j, t)) => {
                    *b = b.unir(e.0);
                    j.extend(e.1);
                    t.push(' ');
                    t.push_str(&e.2);
                }
                None => sortie.push(e),
            }
        }
    }
    sortie.sort_by(|a, b| a.0.x0.total_cmp(&b.0.x0));
    sortie
}

/// Le rang d'en-tetes du tableau : ses colonnes, et les bornes verticales de ses rangs.
struct Tableau {
    colonnes: Vec<Colonne>,
    haut: f64,
    bas: f64,
}

/// Un morceau d'un rang de l'en-tete : des mots qui se suivent.
struct Morceau<'a> {
    mots: Vec<&'a Mot>,
    /// Commence par le libelle d'une mention, ou porte la valeur d'une mention : il coupe un bloc d'adresse.
    mention: Option<usize>,
    coupe: bool,
}

impl Morceau<'_> {
    fn x0(&self) -> f64 {
        self.mots[0].x0
    }
    fn cy(&self) -> f64 {
        milieu(&self.mots)
    }
    fn lignes(&self) -> Vec<String> {
        texte_de(&self.mots).split(" — ").map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
    }
}

/// Une ligne d'adresse « 99999 Commune ».
fn code_postal(ligne: &str) -> bool {
    let mut it = ligne.splitn(2, ' ');
    let cp = it.next().unwrap_or("");
    cp.len() == 5 && cp.chars().all(|c| c.is_ascii_digit()) && it.next().is_some_and(|r| !r.trim().is_empty())
}

/// Un bloc d'adresse : le nom au premier rang, l'adresse ensuite.
struct Bloc {
    lignes: Vec<String>,
    employeur: bool,
}

struct Lecteur<'a> {
    t: &'a Table,
    page: Page,
    /// Pour chaque mot, la mention dont un libelle commence la.
    debuts: Vec<Option<usize>>,
    /// Les mots pris par la valeur d'une mention.
    pris: Vec<usize>,
    avertissements: Vec<String>,
    /// Ce que l'adresse et les mentions du salarie ont rendu, des qu'ils sont lus (meme si la page est refusee ensuite).
    partiel: Option<Salarie>,
}

/// Un rang du corps : son libelle et ses nombres, chacun avec sa colonne.
struct RangCorps {
    libelle: String,
    /// La quantite en tete du libelle, et le libelle sans elle (une ligne de paie : « 2 Cachets isolés »).
    quantite: Option<(Decimal, String)>,
    nombres: Vec<(Option<(Genre, Option<Part>)>, Decimal)>,
    cy: f64,
}

impl RangCorps {
    fn colonne(&self, g: Genre, p: Option<Part>) -> Option<Decimal> {
        self.nombres.iter().find(|(c, _)| *c == Some((g, p))).map(|(_, v)| *v)
    }
    /// Le dernier nombre du rang : la valeur d'un resume, le montant d'une ligne.
    fn dernier(&self) -> Option<Decimal> {
        self.nombres.last().map(|(_, v)| *v)
    }
}

impl<'a> Lecteur<'a> {
    fn lire(&mut self, logiciel: Option<String>) -> Result<BulletinLu, Refus> {
        let tableau = self.tableau();
        // L'en-tete finit au tableau ; sans tableau, toute la page en est (le salarie se lit pour le caviardage).
        let limite = match &tableau {
            Ok(Some(t)) => t.haut - TOLERANCE,
            _ => f64::INFINITY,
        };
        let mut valeurs: Vec<(&'static str, Valeur)> = Vec::new();
        let pied_provisoire = None;
        for k in 0..self.t.mentions.len() {
            if self.t.mentions[k].section == Section::Entete {
                if let Some(v) = self.mention(k, limite, pied_provisoire) {
                    valeurs.push((self.t.mentions[k].nom, v));
                }
            }
        }
        let blocs = self.blocs(limite);
        let (entreprise, salarie) = match (&tableau, blocs) {
            (Err(_), Ok(b)) => b,
            (Err(_), Err(_)) => (None, None),
            (Ok(_), b) => b?,
        };
        let texte = |valeurs: &[(&str, Valeur)], n: &str| {
            valeurs.iter().find(|(k, _)| *k == n).and_then(|(_, v)| if let Valeur::Texte(s) = v { Some(s.clone()) } else { None })
        };
        let date = |valeurs: &[(&str, Valeur)], n: &str| valeurs.iter().find(|(k, _)| *k == n).and_then(|(_, v)| if let Valeur::Date(j) = v { Some(*j) } else { None });
        let nombre = |valeurs: &[(&str, Valeur)], n: &str| valeurs.iter().find(|(k, _)| *k == n).and_then(|(_, v)| if let Valeur::Nombre(x) = v { Some(*x) } else { None });
        let (nom_s, adresse_s) = match salarie {
            Some(b) => {
                let mut l = b.lignes.into_iter();
                (l.next(), l.collect())
            }
            None => (None, Vec::new()),
        };
        let (nom, prenom) = match &nom_s {
            Some(n) => self.separer_nom(n),
            None => (None, None),
        };
        let nir = texte(&valeurs, "nir");
        let salarie = Salarie {
            nom_complet: nom_s.clone(),
            nom,
            prenom,
            nir: nir.clone(),
            naissance: date(&valeurs, "naissance"),
            adresse: adresse_s,
            code: texte(&valeurs, "code"),
            emploi: texte(&valeurs, "emploi"),
            statut: texte(&valeurs, "statut"),
            abattement: nombre(&valeurs, "abattement"),
            conges_spectacles: texte(&valeurs, "conges_spectacles"),
        };
        if nom_s.is_some() || nir.is_some() {
            self.partiel = Some(salarie.clone());
        }
        let Some(tableau) = tableau? else {
            return Err(Refus::SansEntetes(format!(
                "aucun rang ne titre le tableau des lignes (« {} », suivi de « {} » et des parts) : le brut et les cotisations ne se rangent pas",
                self.t.entetes.libelle.first().map_or("libellé", String::as_str),
                self.t.entetes.base.first().map_or("base", String::as_str)
            )));
        };

        // Le corps, puis le pied.
        let corps = self.corps(&tableau)?;
        let pied = corps.fin;
        for k in 0..self.t.mentions.len() {
            if self.t.mentions[k].section != Section::Entete {
                if let Some(v) = self.mention(k, limite, pied) {
                    valeurs.push((self.t.mentions[k].nom, v));
                }
            }
        }
        let mut pas: Option<Pas> = None;
        let mut net_imposable: Option<Decimal> = None;
        let mut cumuls: Vec<(String, Cumul)> = Vec::new();
        if let Some(p) = pied {
            self.grilles(p, &mut pas, &mut net_imposable, &mut cumuls);
            self.sans_grille(p, &mut pas, &mut net_imposable);
        }

        let periode = valeurs.iter().find_map(|(k, v)| if let (true, Valeur::Periode(a, b)) = (*k == "periode", v) { Some((*a, *b)) } else { None });
        let dates_travaillees = valeurs
            .iter()
            .find_map(|(k, v)| if let (true, Valeur::Intervalles(l)) = (*k == "dates_travaillees", v) { Some(l.clone()) } else { None })
            .unwrap_or_default();
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

        // Ce qui identifie le bulletin (R. 3243-1 : 4° le nom du salarie, 5° la periode, 7° le brut, 10° la somme recue).
        if nom_s.is_none() && nir.is_none() {
            return Err(Refus::Incomplet("ni le nom du salarié (son adresse) ni son NIR ne se trouvent".into()));
        }
        let (debut, fin) = periode.ok_or_else(|| Refus::Incomplet(format!("la période (« {} du ... au ... ») ne se trouve pas", self.libelle_de("periode"))))?;
        let brut = corps.lu.brut.ok_or_else(|| Refus::Incomplet(format!("le brut (« {} ») ne se trouve pas", self.t.resumes.brut.first().map_or("", String::as_str))))?;
        // La somme recue (10°) est le net avant impot moins l'impot preleve (R. 3243-1, 9° et 10° ; le modele de
        // l'arrete du 25/02/2016 les imprime l'un sous l'autre) : un bulletin qui imprime les deux termes sans la
        // somme (un impot nul) la dit par cette identite, et le compte rendu dit qu'elle a ete lue ainsi.
        let net_a_payer = match (nombre(&valeurs, "net_a_payer"), corps.lu.net_avant_impot, pas.as_ref().and_then(|p| p.montant)) {
            (Some(n), _, _) => n,
            (None, Some(avant), Some(impot)) => {
                self.avertissements.push(format!(
                    "le net à payer n'est pas imprimé : lu comme le net avant impôt ({}) moins l'impôt prélevé ({}), imprimés",
                    avant, impot
                ));
                avant - impot
            }
            _ => {
                return Err(Refus::Incomplet(format!(
                    "le net à payer (« {} ») ne se trouve pas, ni le net avant impôt et l'impôt prélevé",
                    self.libelle_de("net_a_payer")
                )))
            }
        };
        // Le mode de reglement : sa mention, sinon ce qui suit « par » dans celle de la date (« le 30/11/2026 par virement »).
        let mode_reglement = texte(&valeurs, "mode_reglement").or_else(|| self.mode_de_la_date(limite, pied));

        let mut b = BulletinLu {
            logiciel,
            numero: texte(&valeurs, "numero"),
            entreprise: Entreprise {
                nom: entreprise.as_ref().and_then(|e| e.lignes.first().cloned()),
                adresse: entreprise.map(|e| e.lignes.into_iter().skip(1).collect()).unwrap_or_default(),
                siret: texte(&valeurs, "siret"),
                naf: texte(&valeurs, "naf"),
                urssaf: texte(&valeurs, "urssaf"),
                convention: texte(&valeurs, "convention"),
                licence: texte(&valeurs, "licence"),
            },
            salarie,
            numero_objet: texte(&valeurs, "numero_objet"),
            spectacle: texte(&valeurs, "spectacle"),
            analytique: texte(&valeurs, "analytique"),
            contrat: texte(&valeurs, "contrat"),
            debut,
            fin,
            dates_travaillees,
            jours_travailles,
            lignes: corps.lu.lignes,
            brut,
            cotisations: corps.lu.cotisations,
            total_employeur: corps.lu.total_employeur,
            total_salarie: corps.lu.total_salarie,
            net_social: corps.lu.net_social,
            net_avant_impot: corps.lu.net_avant_impot,
            net_imposable,
            pas,
            net_a_payer,
            cout_employeur: nombre(&valeurs, "cout_employeur"),
            regle_le: date(&valeurs, "regle_le"),
            mode_reglement,
            cumuls,
            avertissements: std::mem::take(&mut self.avertissements),
        };
        controler(&mut b);
        Ok(b)
    }

    /// Le premier libelle d'une mention, pour le dire dans un refus.
    fn libelle_de(&self, nom: &str) -> String {
        self.t.mention(nom).and_then(|m| m.libelles.first()).map(|p| p.join(" ")).unwrap_or_else(|| nom.to_string())
    }

    fn dans_section(&self, cy: f64, section: Section, limite: f64, pied: Option<f64>) -> bool {
        match section {
            Section::Entete => cy < limite,
            Section::Pied => pied.is_some_and(|p| cy > p + TOLERANCE),
            Section::Page => true,
        }
    }

    // ----------------------------------------------------------------------------------------- les mentions

    /// La valeur d'une mention : la premiere de ses occurrences qui en porte une.
    fn mention(&mut self, k: usize, limite: f64, pied: Option<f64>) -> Option<Valeur> {
        let m = self.t.mentions[k].clone();
        let occ = self.page.occurrences(&m.libelles, &|x: &Mot| self.dans_section(x.cy(), m.section, limite, pied));
        for (rep, _, dernier) in occ {
            let v = if m.sorte == Sorte::Nombre && m.section == Section::Pied {
                self.montant_de(&rep, dernier)
            } else {
                let mots = self.valeur(k, &rep, dernier, m.rangs);
                if mots.is_empty() {
                    None
                } else {
                    let v = self.lire_valeur(&m, &mots.iter().map(|&i| self.page.mots[i].texte.clone()).collect::<Vec<_>>());
                    if v.is_some() {
                        self.pris.extend(mots);
                    }
                    v
                }
            };
            if v.is_some() {
                return v;
            }
        }
        None
    }

    /// Les mots qui suivent un libelle sur son rang (et les rangs dessous, pour une valeur qui en couvre plusieurs) :
    /// le premier a moins de `VALEUR_LOIN`, les suivants a moins de `VALEUR_SUITE`, jusqu'au libelle d'une autre
    /// mention. La ponctuation qui borne la valeur (« — », « · », « : ») n'en est pas.
    fn valeur(&self, k: usize, rep: &Boite, dernier: usize, rangs: usize) -> Vec<usize> {
        let p = &self.page;
        let suite = |depart: f64, cy: f64, x_min: f64, loin: f64| -> Vec<usize> {
            let mut rang: Vec<usize> = (0..p.mots.len()).filter(|&i| i != dernier && (p.mots[i].cy() - cy).abs() <= TOLERANCE && p.mots[i].x0 >= x_min).collect();
            rang.sort_by(|&a, &b| p.mots[a].x0.total_cmp(&p.mots[b].x0));
            let mut pris = Vec::new();
            let mut bord = depart;
            for i in rang {
                let commencee = pris.iter().any(|&x: &usize| !ponctuation(&p.mots[x].texte));
                let ecart = p.mots[i].x0 - bord;
                if ecart > if commencee { VALEUR_SUITE } else { loin } {
                    break;
                }
                // Le libelle d'une autre mention, ou le separateur de deux mentions sur un rang (« — », « · », « | »).
                if self.debuts[i].is_some_and(|x| x != k) || (commencee && matches!(p.mots[i].texte.as_str(), "—" | "·" | "|")) {
                    break;
                }
                pris.push(i);
                bord = p.mots[i].x1;
            }
            pris
        };
        let mut mots = suite(rep.x1, rep.cy(), rep.x1 - 0.5, VALEUR_LOIN);
        let x_valeur = mots.iter().find(|&&i| !ponctuation(&p.mots[i].texte)).map_or(rep.x0, |&i| p.mots[i].x0);
        let x_suite = x_valeur.min(rep.x0) - 3.0;
        let mut cy = rep.cy();
        let mut n = 1;
        while n < rangs && !mots.is_empty() {
            let haut = (0..p.mots.len())
                .filter(|&i| p.mots[i].cy() > cy + TOLERANCE && p.mots[i].cy() <= cy + TOLERANCE + VALEUR_RANG && p.mots[i].x0 >= x_suite)
                .min_by(|&a, &b| p.mots[a].cy().total_cmp(&p.mots[b].cy()).then(p.mots[a].x0.total_cmp(&p.mots[b].x0)));
            let Some(h) = haut else { break };
            // Le rang suivant commence ou commence la valeur (ou son libelle), et pas par le libelle d'une mention.
            let premier = (0..p.mots.len())
                .filter(|&i| (p.mots[i].cy() - p.mots[h].cy()).abs() <= TOLERANCE && p.mots[i].x0 >= x_suite)
                .min_by(|&a, &b| p.mots[a].x0.total_cmp(&p.mots[b].x0))
                .unwrap_or(h);
            let aligne = (p.mots[premier].x0 - x_valeur).abs() <= 3.0 || (p.mots[premier].x0 - rep.x0).abs() <= 3.0;
            if self.debuts[premier].is_some() || !aligne {
                break;
            }
            let mut rang = vec![premier];
            rang.extend(suite(p.mots[premier].x1, p.mots[premier].cy(), p.mots[premier].x1 - 0.5, VALEUR_SUITE));
            cy = p.mots[h].cy();
            mots.extend(rang);
            n += 1;
        }
        while mots.first().is_some_and(|&i| ponctuation(&p.mots[i].texte)) {
            mots.remove(0);
        }
        while mots.last().is_some_and(|&i| ponctuation(&p.mots[i].texte)) {
            mots.pop();
        }
        mots
    }

    fn lire_valeur(&mut self, m: &Mention, textes: &[String]) -> Option<Valeur> {
        let textes: Vec<&str> = textes.iter().map(|s| s.trim_end_matches(['.', '·'])).filter(|s| !s.is_empty()).collect();
        match m.sorte {
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
                        self.avertissements.push(format!("{} : « {} » n'est pas une période du ... au ...", m.nom, textes.join(" ")));
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
                    self.avertissements.push(format!("{} : « {} » ne se lit pas en dates", m.nom, textes.join(" ")));
                    return None;
                }
                Some(Valeur::Intervalles(l))
            }
        }
    }

    /// Un montant du pied : le nombre qui suit son libelle sur son rang, sinon le plus proche dessous.
    fn montant_de(&self, rep: &Boite, dernier: usize) -> Option<Valeur> {
        let p = &self.page;
        let bord_libelle = (0..p.mots.len())
            .filter(|&i| i != dernier && self.debuts[i].is_some() && (p.mots[i].cy() - rep.cy()).abs() <= TOLERANCE && p.mots[i].x0 > rep.x1)
            .map(|i| p.mots[i].x0)
            .fold(f64::INFINITY, f64::min);
        let droite = p
            .nombres
            .iter()
            .filter(|n| (n.boite.cy() - rep.cy()).abs() <= TOLERANCE && n.boite.x0 > rep.x1 && n.boite.x1 < bord_libelle)
            .min_by(|a, b| a.boite.x0.total_cmp(&b.boite.x0));
        if let Some(n) = droite {
            return Some(Valeur::Nombre(n.valeur));
        }
        let fenetre = |n: &&Nombre| {
            let b = n.boite;
            b.cy() > rep.cy() + 2.0 && b.cy() - rep.cy() <= DESSOUS_MAX && b.cx() >= rep.x0 - 20.0 && b.cx() <= rep.x1 + 80.0
        };
        let dist = |n: &Nombre| (n.boite.cx() - rep.cx()).hypot(n.boite.cy() - rep.cy());
        p.nombres.iter().filter(fenetre).min_by(|a, b| dist(a).total_cmp(&dist(b))).map(|n| Valeur::Nombre(n.valeur))
    }

    /// Le mode de reglement ecrit apres « par » dans la mention de la date du paiement.
    fn mode_de_la_date(&self, limite: f64, pied: Option<f64>) -> Option<String> {
        let k = self.t.mentions.iter().position(|m| m.nom == "regle_le")?;
        let m = &self.t.mentions[k];
        for (rep, _, dernier) in self.page.occurrences(&m.libelles, &|x: &Mot| self.dans_section(x.cy(), m.section, limite, pied)) {
            let mots = self.valeur(k, &rep, dernier, 1);
            let textes: Vec<&str> = mots.iter().map(|&i| self.page.mots[i].texte.as_str()).collect();
            if !textes.iter().any(|x| lire_date(x).is_some()) {
                continue;
            }
            let apres: Vec<&str> = textes.iter().skip_while(|x| canon(x) != "par").skip(1).copied().collect();
            let mode = apres.join(" ").trim_end_matches(['.', ';', ',']).trim().to_string();
            return (!mode.is_empty()).then_some(mode);
        }
        None
    }

    // ----------------------------------------------------------------------------------------- les adresses

    /// Les adresses de l'employeur et du salarie : les blocs de rangs alignes de l'en-tete dont un rang, apres le
    /// premier, porte un code postal. Celui de l'employeur est suivi, dans sa colonne, d'une mention de l'employeur
    /// (son Siret, son code APE, sa convention) ; celui du salarie est l'autre. Deux adresses de salarie : refus.
    fn blocs(&mut self, limite: f64) -> Result<(Option<Bloc>, Option<Bloc>), Refus> {
        let p = &self.page;
        let mots: Vec<&Mot> = p.mots.iter().filter(|m| m.cy() < limite).collect();
        let mut morceaux: Vec<Morceau> = Vec::new();
        for rang in en_rangs(mots) {
            let mut courant: Vec<&Mot> = Vec::new();
            for m in rang {
                if courant.last().is_some_and(|x| m.x0 - x.x1 > ECART_SEGMENT) {
                    morceaux.push(self.morceau(std::mem::take(&mut courant)));
                }
                courant.push(m);
            }
            if !courant.is_empty() {
                morceaux.push(self.morceau(courant));
            }
        }
        let mut colonnes: Vec<Vec<&Morceau>> = Vec::new();
        for m in &morceaux {
            match colonnes.iter_mut().find(|c| (c[0].x0() - m.x0()).abs() < MEME_COLONNE) {
                Some(c) => c.push(m),
                None => colonnes.push(vec![m]),
            }
        }
        let mut blocs: Vec<(f64, Bloc)> = Vec::new();
        for c in colonnes.iter_mut() {
            c.sort_by(|a, b| a.cy().total_cmp(&b.cy()));
            let mut k = 0;
            while k < c.len() {
                if c[k].coupe {
                    k += 1;
                    continue;
                }
                let debut = k;
                while k + 1 < c.len() && !c[k + 1].coupe && c[k + 1].cy() - c[k].cy() <= BLOC_ECART {
                    k += 1;
                }
                let lignes: Vec<String> = c[debut..=k].iter().flat_map(|m| m.lignes()).collect();
                if lignes.len() >= 2 && lignes[1..].iter().any(|l| code_postal(l)) {
                    let suivant = c.get(k + 1).filter(|s| s.cy() - c[k].cy() <= BLOC_SUIVANT);
                    let employeur = suivant.and_then(|s| s.mention).is_some_and(|x| self.t.mentions[x].employeur);
                    blocs.push((c[debut].cy(), Bloc { lignes, employeur }));
                }
                k += 1;
            }
        }
        blocs.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (employeurs, salaries): (Vec<_>, Vec<_>) = blocs.into_iter().map(|(_, b)| b).partition(|b| b.employeur);
        if employeurs.len() > 1 {
            self.avertissements.push(format!("{} adresses d'employeur : la première est gardée", employeurs.len()));
        }
        let mut salaries = salaries.into_iter();
        let salarie = salaries.next();
        if salaries.next().is_some() {
            return Err(Refus::Ambigu("deux adresses sans mention de l'employeur après elles : laquelle est celle du salarié ne se dit pas".into()));
        }
        Ok((employeurs.into_iter().next(), salarie))
    }

    fn morceau<'m>(&self, mots: Vec<&'m Mot>) -> Morceau<'m> {
        let i = self.page.indice(mots[0]);
        let mention = self.debuts[i];
        let pris = mots.iter().any(|m| self.pris.contains(&self.page.indice(m)));
        Morceau { coupe: mention.is_some() || pris, mention, mots }
    }

    /// Le nom et le prenom : les mots en capitales sont le nom quand les autres ne le sont pas (« Fanchon BRIOCHE ») ;
    /// tout en capitales et en deux mots, le nom vient d'abord (l'usage administratif, « BRIOCHE FANCHON » : une
    /// inference, relevee sur les bulletins lus). Sinon, le nom complet seul et un avertissement.
    fn separer_nom(&mut self, complet: &str) -> (Option<String>, Option<String>) {
        let mots: Vec<&str> = complet.split_whitespace().collect();
        let capitales = |w: &str| w.chars().any(char::is_alphabetic) && !w.chars().any(char::is_lowercase);
        let k = mots.iter().filter(|w| capitales(w)).count();
        if k == mots.len() {
            if let [nom, prenom] = mots.as_slice() {
                return (Some(nom.to_string()), Some(prenom.to_string()));
            }
        } else if k > 0 {
            let debut = mots.iter().take_while(|w| capitales(w)).count();
            let fin = mots.iter().rev().take_while(|w| capitales(w)).count();
            if debut == k {
                return (Some(mots[..k].join(" ")), Some(mots[k..].join(" ")));
            }
            if fin == k {
                let n = mots.len() - k;
                return (Some(mots[n..].join(" ")), Some(mots[..n].join(" ")));
            }
        }
        self.avertissements.push(format!("nom et prénom non séparés : « {} »", complet));
        (None, None)
    }

    // ----------------------------------------------------------------------------------------- le tableau

    /// Le rang d'en-tetes du tableau : un mot « Libellé » (`entetes.libelle`) et, a sa hauteur, des rangs faits
    /// d'en-tetes seulement, dont au moins une colonne de montant. Aucun : `None` ; deux : refus.
    fn tableau(&self) -> Result<Option<Tableau>, Refus> {
        let p = &self.page;
        let e = &self.t.entetes;
        let est_entete = |m: &Mot| !m.jetons.is_empty() && m.jetons.iter().all(|j| e.tous().any(|x| x == j));
        let mut trouves: Vec<Tableau> = Vec::new();
        let mut derniere_erreur: Option<Refus> = None;
        for c in p.mots.iter().filter(|m| m.jetons.first().is_some_and(|j| e.libelle.contains(j))) {
            let zone: Vec<&Mot> = p.mots.iter().filter(|m| (m.cy() - c.cy()).abs() <= ENTETE_HAUTEUR && m.x0 >= c.x0).collect();
            let rangs: Vec<Vec<&Mot>> = en_rangs(zone).into_iter().filter(|r| r.iter().all(|m| est_entete(m))).collect();
            if !rangs.iter().any(|r| r.iter().any(|m| std::ptr::eq(*m, c))) {
                continue;
            }
            let mots: Vec<&Mot> = rangs.iter().flatten().copied().filter(|m| !std::ptr::eq(*m, c) && !m.jetons.iter().any(|j| e.libelle.contains(j))).collect();
            match self.colonnes_du_corps(&mots, None) {
                Ok(colonnes) => {
                    let haut = rangs.iter().flatten().map(|m| m.cy()).fold(f64::INFINITY, f64::min);
                    let bas = rangs.iter().flatten().map(|m| m.cy()).fold(f64::NEG_INFINITY, f64::max);
                    trouves.push(Tableau { colonnes, haut, bas });
                }
                Err(r) => derniere_erreur = Some(r),
            }
        }
        match trouves.len() {
            0 => match derniere_erreur {
                Some(r @ Refus::Ambigu(_)) => Err(r),
                _ => Ok(None),
            },
            1 => Ok(trouves.pop()),
            n => Err(Refus::Ambigu(format!("{} rangs d'en-têtes titrent un tableau : lequel porte les lignes ne se dit pas", n))),
        }
    }

    /// Les colonnes que des mots d'en-tetes annoncent : base, taux et montant de chaque part. Sous un rang deja lu
    /// (`dessus`), un en-tete prend la part de celui qu'il precise (« Taux » sous « Part employeur ») et le remplace.
    /// Un taux qui ne dit pas sa part, seul de son genre, est celui du salarie : c'est le seul taux que le code du
    /// travail exige sur le bulletin (R. 3243-1, 8°, a ; le modele de l'arrete du 25/02/2016 n'a qu'une colonne
    /// « Taux »). Un montant sans part, deux colonnes de la meme part : refus (ambigu).
    fn colonnes_du_corps(&self, mots: &[&Mot], dessus: Option<&[Colonne]>) -> Result<Vec<Colonne>, Refus> {
        let mut cols: Vec<Colonne> = Vec::new();
        for (boite, jetons, texte) in etiquettes(mots, &self.t.entetes) {
            let (genre, part) = self.t.entetes.lire(&jetons);
            if let Some(g @ (Genre::Base | Genre::Taux | Genre::Montant)) = genre {
                cols.push(Colonne { genre: g, part, boite, texte });
            }
        }
        if let Some(dessus) = dessus {
            let mut precises: Vec<bool> = vec![false; dessus.len()];
            for c in cols.iter_mut() {
                let parent = dessus
                    .iter()
                    .enumerate()
                    .filter(|(_, d)| c.boite.cx() >= d.boite.x0 - SOUS_ENTETE && c.boite.cx() <= d.boite.x1 + SOUS_ENTETE && d.genre == Genre::Montant)
                    .min_by(|a, b| (a.1.boite.cx() - c.boite.cx()).abs().total_cmp(&(b.1.boite.cx() - c.boite.cx()).abs()));
                if let Some((k, d)) = parent {
                    if c.part.is_none() {
                        c.part = d.part;
                    }
                    precises[k] = true;
                }
            }
            for (k, d) in dessus.iter().enumerate() {
                if !precises[k] && !cols.iter().any(|c| c.genre == d.genre && c.part == d.part) {
                    cols.push(d.clone());
                }
            }
        }
        let taux_sans_part = cols.iter().filter(|c| c.genre == Genre::Taux).count() == 1;
        for c in cols.iter_mut() {
            match (c.genre, c.part) {
                (Genre::Taux, None) if taux_sans_part => c.part = Some(Part::Salarie),
                (Genre::Taux | Genre::Montant, None) => {
                    return Err(Refus::Ambigu(format!("la colonne « {} » ne dit pas si elle est la part du salarié ou celle de l'employeur", c.texte)));
                }
                (Genre::Base, _) => c.part = None,
                _ => {}
            }
        }
        for (i, a) in cols.iter().enumerate() {
            if let Some(b) = cols[i + 1..].iter().find(|b| b.genre == a.genre && b.part == a.part) {
                return Err(Refus::Ambigu(format!("deux colonnes disent la même chose : « {} » et « {} »", a.texte, b.texte)));
            }
        }
        if !cols.iter().any(|c| c.genre == Genre::Montant) {
            return Err(Refus::SansEntetes("le rang d'en-têtes n'annonce aucune colonne de montant (« Part salarié », « Employeur »...)".into()));
        }
        trier_colonnes(&mut cols);
        Ok(cols)
    }

    /// Les rangs du corps : libelles a gauche de la premiere colonne, nombres dans les colonnes, chaque nombre au rang
    /// le plus proche. Un rang fait d'en-tetes seulement, sans libelle, precise les colonnes pour la suite.
    fn rangs_du_corps(&mut self, t: &Tableau) -> Result<Vec<RangCorps>, Refus> {
        let p = &self.page;
        let e = &self.t.entetes;
        let seuil = t.colonnes.iter().map(|c| c.boite.x0).fold(f64::INFINITY, f64::min) - LIBELLE_MARGE;
        let haut = t.bas + TOLERANCE;
        let gauche: Vec<&Mot> = p.mots.iter().filter(|m| m.x1 <= seuil && m.cy() > haut && !p.dans_un_nombre(m, seuil)).collect();
        let rangs = en_rangs(gauche);
        // Le rang qui precise les en-tetes (le premier) : sans libelle, des en-tetes seulement, a droite du libelle.
        let droite: Vec<&Mot> = p.mots.iter().filter(|m| m.x1 > seuil && m.cy() > haut).collect();
        let mut sous: Option<(f64, Vec<Colonne>)> = None;
        for r in en_rangs(droite) {
            let sans_libelle = !rangs.iter().any(|g| (milieu(g) - milieu(&r)).abs() <= TOLERANCE);
            let entetes = r.len() >= 2 && r.iter().all(|m| !m.jetons.is_empty() && m.jetons.iter().all(|j| e.tous().any(|x| x == j)));
            if sans_libelle && entetes {
                sous = Some((milieu(&r), self.colonnes_du_corps(&r, Some(&t.colonnes))?));
                break;
            }
        }
        let mut sortie: Vec<RangCorps> = rangs
            .iter()
            .map(|r| {
                let quantite = match r.split_first() {
                    Some((q, reste)) if !reste.is_empty() && ressemble_nombre(&q.texte) => lire_nombre(&q.texte).map(|v| (v, texte_de(reste))),
                    _ => None,
                };
                RangCorps { libelle: texte_de(r), quantite, nombres: Vec::new(), cy: milieu(r) }
            })
            .collect();
        // Le corps finit au net avant impot (compris) : le pied commence sous lui.
        if let Some(k) = sortie.iter().position(|r| resume(&canon(&r.libelle), &self.t.resumes.net_avant_impot)) {
            sortie.truncate(k + 1);
        }
        let bas = sortie.last().map_or(f64::INFINITY, |r| r.cy + TOLERANCE);
        if sous.as_ref().is_some_and(|(y, _)| *y > bas) {
            sous = None;
        }
        let nombres: Vec<Nombre> = p.nombres.iter().filter(|n| n.boite.x1 > seuil && n.boite.cy() > haut && n.boite.cy() <= bas).cloned().collect();
        let mut places: Vec<Vec<(f64, Option<(Genre, Option<Part>)>, Decimal)>> = vec![Vec::new(); sortie.len()];
        for n in nombres {
            let cols: &[Colonne] = match &sous {
                Some((y, c)) if n.boite.cy() > *y => c,
                _ => &t.colonnes,
            };
            let col = colonne_de(cols, &n).map(|c| (c.genre, c.part));
            let proche = (0..sortie.len())
                .filter(|&k| (sortie[k].cy - n.boite.cy()).abs() <= TOLERANCE)
                .min_by(|&a, &b| (sortie[a].cy - n.boite.cy()).abs().total_cmp(&(sortie[b].cy - n.boite.cy()).abs()));
            match proche {
                Some(k) if col.is_some() && places[k].iter().any(|(_, c, _)| *c == col) => {
                    self.avertissements.push(format!("« {} » : deux nombres dans une même colonne", sortie[k].libelle));
                }
                Some(k) => {
                    if col.is_none() {
                        self.avertissements.push(format!("« {} » : le nombre {} est hors des colonnes du tableau", sortie[k].libelle, n.valeur));
                    }
                    places[k].push((n.boite.x1, col, n.valeur));
                }
                None => self.avertissements.push(format!("nombre {} sans libellé à sa hauteur", n.valeur)),
            }
        }
        for (r, mut l) in sortie.iter_mut().zip(places) {
            l.sort_by(|a, b| a.0.total_cmp(&b.0));
            r.nombres = l.into_iter().map(|(_, c, v)| (c, v)).collect();
        }
        Ok(sortie)
    }

    /// Le corps, rang par rang, dans l'ordre du modele : les lignes soumises jusqu'au brut ; les cotisations, sous leur
    /// famille, jusqu'au total ; apres le total, une famille facultative (hors total) ou le net social, puis les lignes
    /// non soumises ; le net avant impot le ferme.
    fn corps(&mut self, t: &Tableau) -> Result<CorpsLu, Refus> {
        let rangs = self.rangs_du_corps(t)?;
        let table = self.t;
        let r_ = &table.resumes;
        let est = |libelle: &str, liste: &[String]| liste.iter().any(|x| x == libelle);
        let mut lu = Corps::default();
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
        let mut fin: Option<f64> = None;
        // Une ligne de paie : sa quantite en tete, son montant au dernier nombre, son taux a l'avant-dernier ou dans son
        // libelle (« Cachets à 150,00 »).
        let ligne = |libelle: &str, quantite: Option<Decimal>, r: &RangCorps, soumis: bool, avertissements: &mut Vec<String>| {
            let (libelle, taux_ecrit) = taux_du_libelle(libelle);
            let cl = canon(&libelle);
            // Une ligne hors de l'assiette (un defraiement) n'est pas dans le brut, ou que le bulletin l'imprime.
            let (rubrique, heures, soumis) = match table.lignes.iter().find(|x| x.canon == cl) {
                Some(x) => (x.rubrique.clone(), x.heures, soumis && !x.non_soumise),
                None => {
                    avertissements.push(Refus::LibelleInconnu(libelle.clone()).message());
                    (None, None, soumis)
                }
            };
            let n = r.nombres.len();
            let taux = if n >= 2 { Some(r.nombres[n - 2].1) } else { taux_ecrit };
            LignePaie { quantite, libelle, taux, montant: r.dernier(), soumis, rubrique, heures }
        };
        let de_ligne = |r: &RangCorps| match &r.quantite {
            Some((q, reste)) => (reste.clone(), Some(*q)),
            None => (r.libelle.clone(), None),
        };
        for r in rangs {
            let cl = canon(&r.libelle);
            if resume(&cl, &r_.net_avant_impot) {
                lu.net_avant_impot = r.dernier();
                // « NET SOCIAL / NET A PAYER AVANT IMPÔT » : un rang pour deux nets egaux.
                if lu.net_social.is_none() && cl.split(" / ").any(|p| r_.net_social.iter().any(|x| x == p)) {
                    lu.net_social = r.dernier();
                }
                fin = Some(r.cy);
                break;
            }
            if r.libelle.is_empty() {
                continue;
            }
            if phase == Phase::Lignes {
                // La derniere ligne et le brut sur un meme rang (« 2 Cachets à 150,00 — MONTANT BRUT ») : la ligne, puis le brut.
                let (libelle, quantite) = de_ligne(&r);
                let cll = canon(&libelle);
                if let Some(b) = r_.brut.iter().find(|b| cll.ends_with(&format!(" {}", b))) {
                    let n = b.split(' ').count();
                    let mots: Vec<&str> = libelle.split_whitespace().collect();
                    let garde = mots.len().saturating_sub(n);
                    let seule = mots[..garde].join(" ").trim_end_matches(|ch: char| !ch.is_alphanumeric()).trim().to_string();
                    if !seule.is_empty() && canon(&seule) != "salaire" {
                        lu.lignes.push(ligne(&seule, quantite, &r, true, &mut avertissements));
                    }
                    lu.brut = r.dernier();
                    phase = Phase::Cotisations;
                    continue;
                }
                if est(&cl, &r_.brut) {
                    lu.brut = r.dernier();
                    phase = Phase::Cotisations;
                } else {
                    lu.lignes.push(ligne(&libelle, quantite, &r, true, &mut avertissements));
                }
                continue;
            }
            if est(&cl, &table.titres) && r.nombres.is_empty() {
                continue;
            }
            match phase {
                Phase::Lignes => {}
                Phase::NonSoumis if est(&cl, &r_.net_social) => lu.net_social = r.dernier(),
                Phase::NonSoumis => {
                    let (libelle, quantite) = de_ligne(&r);
                    lu.lignes.push(ligne(&libelle, quantite, &r, false, &mut avertissements));
                }
                Phase::Cotisations | Phase::ApresTotal => {
                    if phase == Phase::Cotisations && est(&cl, &r_.total) {
                        lu.total_employeur = r.colonne(Genre::Montant, Some(Part::Employeur));
                        lu.total_salarie = r.colonne(Genre::Montant, Some(Part::Salarie));
                        phase = Phase::ApresTotal;
                        if let Some((_, ecrit)) = titre_en_cours.take() {
                            avertissements.push(format!("rang sans montant : « {} »", ecrit));
                        }
                        continue;
                    }
                    if est(&cl, &r_.net_social) {
                        lu.net_social = r.dernier();
                        phase = Phase::NonSoumis;
                        continue;
                    }
                    // Un titre de famille, peut-etre ecrit sur deux rangs.
                    if let Some((tc, te)) = titre_en_cours.take() {
                        let (cle, ecrit) = (format!("{} {}", tc, cl), format!("{} {}", te, r.libelle));
                        if let Some(f) = table.famille(&cle) {
                            famille = Some(f.to_string());
                            famille_apres_total |= phase == Phase::ApresTotal;
                            if !r.nombres.is_empty() {
                                lu.ajouter(cotisation(&famille, ecrit, &r), phase == Phase::ApresTotal);
                            }
                            continue;
                        }
                        if r.nombres.is_empty() && table.debut_de_famille(&cle) {
                            titre_en_cours = Some((cle, ecrit));
                            continue;
                        }
                        avertissements.push(format!("rang sans montant : « {} »", te));
                    }
                    if let Some(f) = table.famille(&cl) {
                        famille = Some(f.to_string());
                        famille_apres_total |= phase == Phase::ApresTotal;
                        if !r.nombres.is_empty() {
                            lu.ajouter(cotisation(&famille, r.libelle.clone(), &r), phase == Phase::ApresTotal);
                        }
                        continue;
                    }
                    if r.nombres.is_empty() && table.debut_de_famille(&cl) {
                        titre_en_cours = Some((cl, r.libelle.clone()));
                        continue;
                    }
                    if phase == Phase::ApresTotal && !famille_apres_total {
                        // Ni famille ni net social apres le total : les lignes non soumises commencent.
                        phase = Phase::NonSoumis;
                        let (libelle, quantite) = de_ligne(&r);
                        lu.lignes.push(ligne(&libelle, quantite, &r, false, &mut avertissements));
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
        Ok(CorpsLu { lu, fin })
    }

    // ----------------------------------------------------------------------------------------- le pied

    /// Les grilles du pied, chacune sous son rang d'en-tetes : l'impot preleve et le net imposable (« base »,
    /// « taux », « montant »), les cumuls (« Période », « Cumul »).
    fn grilles(&mut self, pied: f64, pas: &mut Option<Pas>, net_imposable: &mut Option<Decimal>, cumuls: &mut Vec<(String, Cumul)>) {
        let p = &self.page;
        let t = self.t;
        let mots: Vec<&Mot> = p.mots.iter().filter(|m| m.cy() > pied + TOLERANCE).collect();
        let mut entetes: Vec<(f64, Vec<Colonne>, bool)> = Vec::new();
        for r in en_rangs(mots) {
            let mut cols: Vec<Colonne> = Vec::new();
            for (boite, jetons, texte) in etiquettes(&r, &t.entetes) {
                if let (Some(genre), part) = t.entetes.lire(&jetons) {
                    cols.push(Colonne { genre, part, boite, texte });
                }
            }
            let a = |g: Genre| cols.iter().any(|c| c.genre == g);
            if a(Genre::Periode) && a(Genre::Cumul) {
                cols.retain(|c| matches!(c.genre, Genre::Periode | Genre::Cumul));
                trier_colonnes(&mut cols);
                entetes.push((milieu(&r), cols, true));
            } else if a(Genre::Taux) && (a(Genre::Base) || a(Genre::Montant)) {
                cols.retain(|c| matches!(c.genre, Genre::Base | Genre::Taux | Genre::Montant | Genre::Cumul));
                trier_colonnes(&mut cols);
                entetes.push((milieu(&r), cols, false));
            }
        }
        for (k, (y, cols, cumul)) in entetes.iter().enumerate() {
            let jusqu_a = entetes.get(k + 1).map_or(y + GRILLE_HAUTEUR, |e| e.0.min(y + GRILLE_HAUTEUR));
            let seuil = cols.iter().map(|c| c.boite.x0).fold(f64::INFINITY, f64::min) - LIBELLE_MARGE;
            let dans = |cy: f64| cy > y + TOLERANCE && cy <= jusqu_a + if k + 1 < entetes.len() { -TOLERANCE } else { 0.0 };
            let libelles: Vec<&Mot> = p.mots.iter().filter(|m| dans(m.cy()) && m.x1 <= seuil && !p.dans_un_nombre(m, seuil)).collect();
            let rangs: Vec<(String, f64)> = en_rangs(libelles).iter().map(|r| (canon(&texte_de(r)), milieu(r))).collect();
            let nombres: Vec<&Nombre> = p.nombres.iter().filter(|n| dans(n.boite.cy()) && n.boite.x1 > seuil).collect();
            let valeurs_du_rang = |cy: f64| -> Vec<(Genre, Decimal)> {
                let mut v: Vec<(Genre, Decimal)> = Vec::new();
                for n in &nombres {
                    // Le rang le plus proche de ce nombre doit etre celui-ci.
                    let ecart = (n.boite.cy() - cy).abs();
                    if ecart > TOLERANCE || rangs.iter().any(|(_, autre)| (n.boite.cy() - autre).abs() < ecart) {
                        continue;
                    }
                    if let Some(c) = colonne_de(cols, n) {
                        if !v.iter().any(|(g, _)| *g == c.genre) {
                            v.push((c.genre, n.valeur));
                        }
                    }
                }
                v
            };
            for (libelle, cy) in &rangs {
                let v = valeurs_du_rang(*cy);
                let col = |g: Genre| v.iter().find(|(x, _)| *x == g).map(|(_, y)| *y);
                if *cumul {
                    if let Some((nom, _)) = t.cumuls.iter().find(|(_, l)| l.contains(libelle)) {
                        if !cumuls.iter().any(|(x, _)| x == nom) {
                            cumuls.push((nom.clone(), Cumul { periode: col(Genre::Periode), cumul: col(Genre::Cumul) }));
                        }
                    }
                    if net_imposable.is_none() && t.resumes.net_imposable.contains(libelle) {
                        *net_imposable = col(Genre::Periode);
                    }
                } else if pas.is_none() && t.resumes.pas.contains(libelle) {
                    *pas = Some(Pas { base: col(Genre::Base), taux: col(Genre::Taux), montant: col(Genre::Montant), cumul: col(Genre::Cumul) });
                } else if net_imposable.is_none() && t.resumes.net_imposable.contains(libelle) {
                    *net_imposable = col(Genre::Montant);
                }
            }
        }
    }
}

impl Lecteur<'_> {
    /// L'impot preleve et le net imposable ecrits sans grille, sur un rang du pied : « Net imposable 435,55 »,
    /// « Impôt sur le revenu prélevé à la source (taux 4,300 %) 18,73 ». Le montant est le dernier nombre du rang ; le
    /// taux, celui qui suit le mot « taux ».
    fn sans_grille(&self, pied: f64, pas: &mut Option<Pas>, net_imposable: &mut Option<Decimal>) {
        let p = &self.page;
        let r_ = &self.t.resumes;
        let mots: Vec<&Mot> = p.mots.iter().filter(|m| m.cy() > pied + TOLERANCE).collect();
        for rang in en_rangs(mots) {
            let nombres: Vec<&Nombre> = p.nombres.iter().filter(|n| (n.boite.cy() - milieu(&rang)).abs() <= TOLERANCE).collect();
            let Some(dernier) = nombres.iter().max_by(|a, b| a.boite.x1.total_cmp(&b.boite.x1)) else { continue };
            let libelle: Vec<&Mot> = rang.iter().copied().filter(|m| m.x1 < dernier.boite.x0 && !ressemble_nombre(&m.texte)).collect();
            let cl = canon(&texte_de(&libelle));
            if net_imposable.is_none() && r_.net_imposable.iter().any(|x| *x == cl) {
                *net_imposable = Some(dernier.valeur);
            }
            if pas.is_none() && r_.pas.iter().any(|x| cl.starts_with(x.as_str())) {
                let taux = rang
                    .iter()
                    .position(|m| canon(&m.texte).trim_start_matches('(') == "taux")
                    .and_then(|k| rang.get(k + 1))
                    .and_then(|m| lire_nombre(m.texte.trim_end_matches([')', '%'])));
                *pas = Some(Pas { base: None, taux, montant: Some(dernier.valeur), cumul: None });
            }
        }
    }
}

#[derive(Default)]
struct Corps {
    lignes: Vec<LignePaie>,
    brut: Option<Decimal>,
    cotisations: Vec<Cotisation>,
    total_employeur: Option<Decimal>,
    total_salarie: Option<Decimal>,
    net_social: Option<Decimal>,
    net_avant_impot: Option<Decimal>,
}

/// Le corps lu, et le milieu du rang qui le ferme (le net avant impot) : le pied commence sous lui.
struct CorpsLu {
    lu: Corps,
    fin: Option<f64>,
}

impl Corps {
    fn ajouter(&mut self, mut c: Cotisation, hors_total: bool) {
        c.hors_total = hors_total;
        self.cotisations.push(c);
    }
}

fn cotisation(famille: &Option<String>, libelle: String, r: &RangCorps) -> Cotisation {
    Cotisation {
        famille: famille.clone(),
        libelle,
        base: r.colonne(Genre::Base, None),
        taux_employeur: r.colonne(Genre::Taux, Some(Part::Employeur)),
        montant_employeur: r.colonne(Genre::Montant, Some(Part::Employeur)),
        taux_salarie: r.colonne(Genre::Taux, Some(Part::Salarie)),
        montant_salarie: r.colonne(Genre::Montant, Some(Part::Salarie)),
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

// --------------------------------------------------------------------------------------------- les compteurs

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
    /// Les jours travailles : les dates du bulletin, ou son cumul « Jours ».
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
/// ses heures quand aucun cachet ne s'y mele. Un total d'heures ou des cachets sont convertis se confronte a heures +
/// cachets x forfait, mais ne se decompose pas : on ne devine pas la conversion qu'un logiciel a faite (un cachet
/// groupe a 8 h, la regle d'avant le decret du 13 juillet 2016) ; les compteurs restent ceux des lignes. Une erreur :
/// une rubrique que le catalogue ne connait pas.
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
