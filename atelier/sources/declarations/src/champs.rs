//! Ce qu'est une fiche : les champs, leur forme, et ce qui
//! les rend valides. Seule la part que l'accueil lit est portee : la
//! description des champs, la forme lisible comblee, la forme declaree qui en
//! decoule (`apercu_derive`) et les anomalies d'une fiche (`verifier_fiche`).
//!
//! La table des champs est `donnees/champs-base.json` : une seule source. Chaque champ y porte, en
//! `_note`, son commentaire.
//!
//! **Les majuscules sans accents ne se saisissent PAS.** Les formats
//! declaratifs (AEM, DPAE, DSN) sont a largeur fixe et n'acceptent pas les
//! accents : `salaries.json` porte `MARTIN CLAIRE`, `AVALLON`. Mais un
//! contrat imprime « 2 rue des Lilas ». L'humain ecrit « Béjart Duparc » ;
//! c'est le programme qui fabrique `BEJART DUPARC`. Ce module ne corrige rien
//! en silence : il montre la forme declaree a cote de la forme lisible.

use std::sync::{Arc, OnceLock};

use serde_json::{json, Map, Value};

use socle::texte::sans_accent;
use socle::date::Jour;
use socle::valeur::valeur_en_texte;
use socle::echec::erreur;
use socle::fichiers::{dossier_du_code, json_en_cache, vrai};
use socle::echec::Echec;

type R<T> = Result<T, Echec>;

#[track_caller]
fn plain(m: impl Into<String>) -> Echec {
    socle::echec::noter_et(None, m)
}

/// `FORMES` : le libelle de chaque forme de champ.
fn forme_libelle(forme: &str) -> &'static str {
    match forme {
        "texte" => "texte libre",
        "officiel" => "MAJUSCULES SANS ACCENTS (format declaratif a largeur fixe)",
        "affichage" => "forme lisible, jamais declaree",
        "chiffres" => "chiffres uniquement",
        "date" => "AAAA-MM-JJ",
        "email" => "adresse de courriel",
        "montant" => "nombre decimal",
        "choix" => "une valeur de la liste",
        "pays" => "un pays de la liste (code ISO a deux lettres)",
        "booleen" => "oui ou non",
        "structure" => "sous-objet ou liste : edition libre",
        "histoire" => "ce que ce champ valait avant, et jusqu'a quand",
        _ => "",
    }
}

/// `Champ` : une ligne de la table.
#[derive(Debug, Clone)]
pub struct Champ {
    pub cle: String,
    pub libelle: String,
    pub forme: String,
    pub aide: String,
    pub obligatoire: bool,
    pub longueur: Option<i64>,
    pub exemple: String,
    pub choix: Vec<Value>,
    pub declare: bool,
    pub derive_de: Option<String>,
    pub rubrique: Option<String>,
    pub si_permanents: bool,
    pub si: Value,
    pub si_etablissements: bool,
    pub si_entreprise: Value,
    /// Quand la compagnie DOIT le remplir (`exige_si`, meme forme que `si`) : `Champ::exige`.
    pub exige_si: Value,
    pub booleen_defaut: bool,
    pub ecran: bool,
}

impl Champ {
    /// `Champ.calcule` : le champ se deduit d'une forme lisible.
    pub fn calcule(&self) -> bool {
        self.derive_de.as_deref().map_or(false, |d| !d.is_empty())
    }

    fn lire(v: &Value) -> Champ {
        let texte = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let option = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        let drapeau = |k: &str, defaut: bool| v.get(k).and_then(Value::as_bool).unwrap_or(defaut);
        Champ {
            cle: texte("cle"),
            libelle: texte("libelle"),
            forme: texte("forme"),
            aide: texte("aide"),
            obligatoire: drapeau("obligatoire", false),
            longueur: v.get("longueur").and_then(Value::as_i64),
            exemple: texte("exemple"),
            choix: v.get("choix").and_then(Value::as_array).cloned().unwrap_or_default(),
            declare: drapeau("declare", true),
            derive_de: option("derive_de"),
            rubrique: option("rubrique"),
            si_permanents: drapeau("si_permanents", false),
            si: v.get("si").cloned().unwrap_or(Value::Null),
            si_etablissements: drapeau("si_etablissements", false),
            si_entreprise: v.get("si_entreprise").cloned().unwrap_or(Value::Null),
            exige_si: v.get("exige_si").cloned().unwrap_or(Value::Null),
            booleen_defaut: drapeau("booleen_defaut", true),
            ecran: drapeau("ecran", true),
        }
    }

    /// Une ligne de `description` : ce que l'interface consomme.
    fn description(&self) -> Value {
        json!({
            "cle": self.cle, "libelle": self.libelle, "forme": self.forme,
            "aide": self.aide, "obligatoire": self.obligatoire,
            "longueur": self.longueur, "exemple": self.exemple, "choix": self.choix,
            "declare": self.declare, "forme_libelle": forme_libelle(&self.forme),
            "calcule": self.calcule(), "derive_de": self.derive_de,
            // La rubrique part a l'ecran : un compte rendu d'organisme parle de
            // << S21.G00.30.010 en anomalie >>, pas de << commune >>.
            "rubrique": self.rubrique,
            // Ce qui ne se montre qu'a une compagnie qui emploie des permanents :
            // le champ reste dans la description, c'est l'ecran qui le replie.
            "si_permanents": self.si_permanents, "si": self.si,
            "si_etablissements": self.si_etablissements,
            "si_entreprise": self.si_entreprise,
            "exige_si": self.exige_si,
            "booleen_defaut": self.booleen_defaut, "ecran": self.ecran,
        })
    }
}

/// Les trois fiches : `FICHES`.
pub struct Fiches {
    entreprise: Vec<Champ>,
    salarie: Vec<Champ>,
    spectacle: Vec<Champ>,
}

static TABLES: OnceLock<Result<Arc<Fiches>, String>> = OnceLock::new();

/// Les champs d'une fiche (`entreprise`, `salarie`, `spectacle`).
pub fn fiche(nom: &str) -> R<Vec<Champ>> {
    let t = TABLES
        .get_or_init(|| {
            let brut = json_en_cache(&dossier_du_code().join("champs-base.json"))?;
            let liste = |n: &str| -> Vec<Champ> {
                brut.get(n).and_then(Value::as_array).map(|a| a.iter().map(Champ::lire).collect()).unwrap_or_default()
            };
            Ok(Arc::new(Fiches { entreprise: liste("entreprise"), salarie: liste("salarie"), spectacle: liste("spectacle") }))
        })
        .as_ref()
        .map_err(|m| plain(m.clone()))?;
    // Lu une fois par processus (le code livre ne change pas sous la fenetre), mais note a chaque usage : un calcul
    // suivi dans un processus deja chaud a les memes entrees qu'un processus neuf.
    socle::fichiers::noter_si_besoin(&dossier_du_code().join("champs-base.json"));
    match nom {
        "entreprise" => Ok(t.entreprise.clone()),
        "salarie" => Ok(t.salarie.clone()),
        "spectacle" => Ok(t.spectacle.clone()),
        autre => Err(erreur("KeyError", format!("fiche inconnue : {}", socle::fichiers::repr(autre)))),
    }
}

// ------------------------------------------------------- les outils de texte

pub(crate) use socle::texte::{blanc as espace, strip};

/// `str.isdigit()` : les chiffres, et ceux que Unicode range avec eux.
pub fn est_chiffre(c: char) -> bool {
    c.is_ascii_digit() || matches!(c, '\u{b2}' | '\u{b3}' | '\u{b9}') || (c.is_numeric() && !c.is_alphabetic() && !matches!(c, '\u{bc}'..='\u{be}' | '\u{2150}'..='\u{218f}'))
}

pub fn tous_chiffres(s: &str) -> bool {
    !s.is_empty() && s.chars().all(est_chiffre)
}

/// `str.title()`.
pub fn titre(s: &str) -> String {
    let mut out = String::new();
    let mut precedent_cased = false;
    for c in s.chars() {
        let cased = c.is_lowercase() || c.is_uppercase();
        if cased {
            if precedent_cased {
                out.extend(c.to_lowercase());
            } else {
                out.extend(c.to_uppercase());
            }
        } else {
            out.push(c);
        }
        precedent_cased = cased;
    }
    out
}

/// `sans_accents` : NFD, marques retirees.
pub fn sans_accents(s: &str) -> String {
    sans_accent(s)
}

/// `texte[a:b]`, sur les caracteres.
fn tranche(s: &str, a: usize, b: usize) -> String {
    s.chars().skip(a).take(b.saturating_sub(a)).collect()
}

/// `float(texte)` : est-ce un nombre ? (`nan`, `inf`, `1e5`,
/// `1_000` en sont ; une virgule non.)
fn flottant_valide(t: &str) -> bool {
    let t = strip(t);
    if t.is_empty() {
        return false;
    }
    let sans = t.trim_start_matches(['+', '-']);
    if t.len() - sans.len() > 1 {
        return false;
    }
    let bas = sans.to_ascii_lowercase();
    if matches!(bas.as_str(), "nan" | "inf" | "infinity") {
        return true;
    }
    // Un tiret bas ne se tolere qu'entre deux chiffres.
    let c: Vec<char> = sans.chars().collect();
    for (i, x) in c.iter().enumerate() {
        if *x == '_' && !(i > 0 && i + 1 < c.len() && c[i - 1].is_ascii_digit() && c[i + 1].is_ascii_digit()) {
            return false;
        }
    }
    let net: String = sans.chars().filter(|x| *x != '_').collect();
    !net.is_empty() && net.parse::<f64>().is_ok() && !net.starts_with(['+', '-'])
}

/// Une date ISO : AAAA-MM-JJ, AAAAMMJJ, et les semaines ISO.
fn date_iso(s: &str) -> Option<Jour> {
    if !s.is_ascii() {
        return None;
    }
    let b = s.as_bytes();
    let chiffres = |a: usize, n: usize| -> Option<u32> {
        let x = b.get(a..a + n)?;
        if x.iter().all(u8::is_ascii_digit) {
            std::str::from_utf8(x).ok()?.parse().ok()
        } else {
            None
        }
    };
    let an = chiffres(0, 4)? as i32;
    let mut p = 4;
    let sep = b.get(p) == Some(&b'-');
    if sep {
        p += 1;
    }
    if b.get(p) == Some(&b'W') {
        p += 1;
        let semaine = chiffres(p, 2)?;
        p += 2;
        let jour = if p == b.len() {
            1
        } else {
            if sep {
                if b.get(p) != Some(&b'-') {
                    return None;
                }
                p += 1;
            }
            let j = chiffres(p, 1)?;
            p += 1;
            j
        };
        if p != b.len() || semaine < 1 || !(1..=7).contains(&jour) {
            return None;
        }
        // Le lundi de la semaine 1 est celui de la semaine du 4 janvier.
        let quatre = Jour::nouveau(an, 1, 4)?;
        let lundi1 = quatre.ordinal() - i64::from(quatre.jour_de_la_semaine());
        // Une annee compte 53 semaines si elle commence un jeudi, ou un
        // mercredi quand elle est bissextile ; 52 sinon.
        let premier = Jour::nouveau(an, 1, 1)?.jour_de_la_semaine();
        let semaines = if premier == 3 || (premier == 2 && socle::date::bissextile(an)) { 53 } else { 52 };
        if semaine > semaines {
            return None;
        }
        return Jour::depuis_ordinal(lundi1 + (i64::from(semaine) - 1) * 7 + i64::from(jour) - 1);
    }
    let mois = chiffres(p, 2)?;
    p += 2;
    if sep {
        if b.get(p) != Some(&b'-') {
            return None;
        }
        p += 1;
    }
    let jour = chiffres(p, 2)?;
    p += 2;
    if p != b.len() {
        return None;
    }
    Jour::nouveau(an, mois, jour)
}

// ------------------------------------------------------------ normalisation

const SEPARATEURS: &str = " -'.";
const CIVILITES: [&str; 6] = ["MONSIEUR ", "M ", "MADAME ", "MME ", "MLLE ", "MR "];

/// `EQUIVALENTS` : ce que l'on tape n'est pas ce que la norme lit.
fn equivalent(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{2019}' | '\u{2018}' | '\u{02bc}' | '\u{0060}' | '\u{00b4}' | '\u{2032}' => "'",
        '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2212}' => "-",
        '\u{00c6}' | '\u{00e6}' => "AE",
        '\u{0152}' | '\u{0153}' => "OE",
        '\u{00df}' => "SS",
        '\u{00d8}' | '\u{00f8}' => "O",
        '\u{00a0}' | '\u{202f}' | '\u{00b0}' => " ",
        _ => return None,
    })
}

/// `_ponctuation_propre` : capitales sans accents, signes ramenes a l'ASCII,
/// separateurs isoles.
fn ponctuation_propre(texte: &str, identite: bool) -> String {
    let mut v: String = String::new();
    for c in texte.chars() {
        match equivalent(c) {
            Some(e) => v.push_str(e),
            None => v.push(c),
        }
    }
    let v: String = sans_accents(&v).chars().flat_map(char::to_uppercase).collect();
    // Ce qui reste hors de l'ASCII imprimable ne se declare pas : une espace.
    let v: String = v.chars().map(|c| if (' '..='~').contains(&c) { c } else { ' ' }).collect();
    // `\s+` -> une espace, puis strip.
    let mut compact = String::new();
    let mut dans_espace = false;
    for c in v.chars() {
        if espace(c) {
            dans_espace = true;
        } else {
            if dans_espace {
                compact.push(' ');
            }
            dans_espace = false;
            compact.push(c);
        }
    }
    let v = strip(&compact).to_string();
    // Pas d'espace contre un trait d'union ni une apostrophe : ` ?([-']) ?`.
    let c: Vec<char> = v.chars().collect();
    let mut sans: Vec<char> = Vec::new();
    let mut i = 0;
    while i < c.len() {
        if c[i] == ' ' && i + 1 < c.len() && (c[i + 1] == '-' || c[i + 1] == '\'') {
            // l'espace avant le signe
            i += 1;
            continue;
        }
        sans.push(c[i]);
        if (c[i] == '-' || c[i] == '\'') && i + 1 < c.len() && c[i + 1] == ' ' {
            // l'espace apres le signe
            i += 2;
            continue;
        }
        i += 1;
    }
    // Deux separateurs accoles : on garde le premier -- sauf le double trait
    // d'union des noms doubles (une fois) et le point suivi d'une espace.
    let mut sortie: Vec<char> = Vec::new();
    let mut doubles = 0;
    for c in sans {
        let a = sortie.last().copied();
        if let Some(a) = a {
            if SEPARATEURS.contains(a) && SEPARATEURS.contains(c) {
                let avant = sortie.len().checked_sub(2).map(|k| sortie[k]);
                if a == c && a == '-' && doubles == 0 && avant != Some('-') {
                    doubles += 1;
                } else if !(a == '.' && c == ' ') {
                    continue;
                }
            }
        }
        sortie.push(c);
    }
    let mut v: String = sortie.into_iter().collect();
    // Une civilite n'a rien a faire dans un nom : le sexe a sa rubrique.
    if identite {
        for civilite in CIVILITES {
            if v.starts_with(civilite) && v.chars().count() > civilite.chars().count() {
                v = v[civilite.len()..].to_string();
                break;
            }
        }
    }
    strip(&v).to_string()
}

/// `_sans_caracteres_refuses` : retire de la forme declaree ce que SA rubrique
/// n'admet pas -- par une espace d'abord, par rien ensuite -- et rend la forme
/// naive si rien ne passe.
fn sans_caracteres_refuses(c: &Champ, v: String) -> R<String> {
    let Some(rubrique) = c.rubrique.as_deref().filter(|r| !r.is_empty()) else { return Ok(v) };
    if v.is_empty() {
        return Ok(v);
    }
    let refuses = crate::formats::caracteres_refuses(rubrique, &v).map_err(plain)?;
    if refuses.is_empty() {
        return Ok(v);
    }
    for remplacant in [" ", ""] {
        let mut essai = v.clone();
        for r in &refuses {
            essai = essai.replace(*r, remplacant);
        }
        let essai = essai.split(espace).filter(|m| !m.is_empty()).collect::<Vec<_>>().join(" ");
        if !essai.is_empty() && crate::formats::mauvaise(rubrique, &essai).map_err(plain)?.is_empty() {
            return Ok(essai);
        }
    }
    Ok(v)
}

/// `normaliser(champ, valeur)` : la forme que le champ DEVRAIT avoir. Une
/// valeur qui n'est pas un texte revient telle quelle.
pub fn normaliser(c: &Champ, valeur: &Value) -> R<Value> {
    let Value::String(brut) = valeur else { return Ok(valeur.clone()) };
    let v = strip(brut).to_string();
    Ok(Value::String(match c.forme.as_str() {
        "officiel" => {
            let v = ponctuation_propre(&v, c.cle == "nom" || c.cle == "prenom");
            let v = sans_caracteres_refuses(c, v)?;
            v.trim_matches(|x| " '.-".contains(x)).to_string()
        }
        "chiffres" => v.chars().filter(|x| !espace(*x)).collect(),
        // Un pays se garde par son code ISO, en capitales.
        "pays" => v.to_uppercase(),
        _ => v,
    }))
}

// Plus de defauts (10/10/2026) : `statut_conventionnel` et `statut_categoriel_rc` se montraient a 05 et
// 04 sur une fiche qui ne les portait pas, et l'import les ecrivait ainsi. Ils se lisent du metier ou se saisissent ;
// la DSN refuse ce qui ne se lit pas (`dsn::statuts_du_contrat`).

/// `PERSONNES` : le signataire et le contact DSN, avec les cles declarees qui
/// ont une forme lisible `<cle>_affichage`.
const PERSONNES: [(&str, &[&str]); 3] = [("signataire", &["nom", "prenom"]), ("declarant_dsn", &[]), ("contact_sante_travail", &[])];

fn faux_ou_absent(d: &Map<String, Value>, k: &str) -> bool {
    d.get(k).map_or(true, |v| !vrai(v))
}

/// `combler_affichage` : donne une forme lisible de depart aux fiches qui n'en
/// ont pas. Rien n'est ecrit : c'est une vue, pas une migration.
pub fn combler_affichage(nom: &str, donnees: &Map<String, Value>) -> R<Map<String, Value>> {
    let mut sortie = donnees.clone();
    for c in fiche(nom)? {
        let Some(source) = c.derive_de.as_deref().filter(|d| !d.is_empty()) else { continue };
        if !faux_ou_absent(&sortie, source) {
            continue;
        }
        if let Some(Value::String(officiel)) = sortie.get(&c.cle) {
            if !officiel.is_empty() {
                let t = titre(officiel);
                sortie.insert(source.to_string(), Value::String(t));
            }
        }
    }
    if nom == "entreprise" {
        for (cle, officiels) in PERSONNES {
            let Some(Value::Object(p)) = sortie.get(cle) else { continue };
            let mut p = p.clone();
            if (cle == "declarant_dsn" || cle == "contact_sante_travail") && faux_ou_absent(&p, "nom_affichage") && faux_ou_absent(&p, "prenom_affichage") {
                // << NOM PRENOM >> en un seul champ : le premier mot fait le nom,
                // le reste le prenom.
                let np = p.get("nom_prenom").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
                let mots: Vec<&str> = np.split(espace).filter(|m| !m.is_empty()).collect();
                if !mots.is_empty() {
                    p.insert("nom_affichage".into(), Value::String(titre(mots[0])));
                    p.insert("prenom_affichage".into(), Value::String(titre(&mots[1..].join(" "))));
                }
            }
            for k in officiels {
                let lisible = format!("{}_affichage", k);
                if faux_ou_absent(&p, &lisible) {
                    if let Some(Value::String(o)) = p.get(*k) {
                        let t = titre(o);
                        p.insert(lisible, Value::String(t));
                    }
                }
            }
            sortie.insert(cle.to_string(), Value::Object(p));
        }
    }
    Ok(sortie)
}

/// `apercu_derive` : ce qui SERAIT declare, sans rien modifier.
pub fn apercu_derive(nom: &str, donnees: &Map<String, Value>) -> R<Value> {
    let mut out = Map::new();
    for c in fiche(nom)? {
        let Some(source) = c.derive_de.as_deref().filter(|d| !d.is_empty()) else { continue };
        let Some(valeur) = donnees.get(source).filter(|v| vrai(v)) else { continue };
        out.insert(c.cle.clone(), normaliser(&c, valeur)?);
    }
    Ok(Value::Object(out))
}

/// `description(nom_fiche)` : la description en dictionnaires.

// ------------------------------------------------- quand un champ se montre

/// La valeur d'une cle de fiche comme la page la compare (`fiche.js`, `conditionRemplie` : `String(v ?? "").trim()`).
fn comme_la_page(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.trim().to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Number(n)) => n.to_string(),
        Some(autre) => autre.to_string(),
    }
}

/// UNE CONDITION DE `champs-base.json` (`si`, `exige_si`) remplie par la fiche `donnees` : chaque cle de la condition
/// vaut l'une des valeurs listees ; `"*"` admet toute valeur non vide (et pas `false`). LA MEME REGLE que la page applique
/// pour replier un champ (`fiche.js`, `conditionRemplie`) : un manque de « Pour demarrer » et un refus de la DSN se
/// jugent sur ce que l'ecran montre (#236, « aucun manque ne pointe vers un champ masque »). Une condition absente est
/// remplie.
pub fn condition_remplie(si: &Value, donnees: &Value) -> bool {
    let Some(o) = si.as_object() else { return true };
    o.iter().all(|(k, vals)| {
        let v = comme_la_page(donnees.get(k));
        let vals = vals.as_array().map(Vec::as_slice).unwrap_or(&[]);
        vals.iter().any(|x| x.as_str() == Some(v.as_str())) || (vals.iter().any(|x| x == "*") && !v.is_empty() && v != "false")
    })
}

impl Champ {
    /// Le champ de la fiche entreprise `donnees` est-il A L'ECRAN ? Ce que la page decide pour la fiche entreprise
    /// (`fiche.js`, `champHtml`) : sa condition `si`, la case des permanents (`si_permanents`) et la liste des
    /// etablissements (`si_etablissements`).
    pub fn montre(&self, donnees: &Value, permanents: bool, etablissements: bool) -> bool {
        self.ecran
            && !self.calcule()
            && condition_remplie(&self.si, donnees)
            && !(self.si_permanents && !permanents)
            && !(self.si_etablissements && !etablissements)
    }

    /// Le champ est-il EXIGE de la compagnie `donnees` ? Sa condition `exige_si` de `champs-base.json`, la seule : la
    /// fiche (« Pour demarrer », `demarrage::manques`) et la declaration qui le lit (la DSN, pour la licence) la lisent
    /// toutes deux. Un champ sans `exige_si` n'est exige de personne par cette voie. Un champ exige est toujours montre
    /// (`tests/routes/demarrage.rs` le verifie sur toutes les conventions).
    pub fn exige(&self, donnees: &Value) -> bool {
        !self.exige_si.is_null() && condition_remplie(&self.exige_si, donnees) && condition_remplie(&self.si, donnees)
    }
}

/// Le champ `cle` de la fiche `nom`, s'il existe.
pub fn champ(nom: &str, cle: &str) -> R<Option<Champ>> {
    Ok(fiche(nom)?.into_iter().find(|c| c.cle == cle))
}

pub fn description(nom: &str) -> R<Value> {
    Ok(Value::Array(fiche(nom)?.iter().map(Champ::description).collect()))
}

// ------------------------------------------------------------- verification

const RUBRIQUE_NIR: &str = "S21.G00.30.001";

/// `MORCEAUX_NIR` : ce que vaut chaque morceau d'un NIR (cahier technique
/// NEODeS 2026.1, definition de S21.G00.30.001) : SAAMMDDCCCNNN.
const MORCEAUX_NIR: [(usize, usize, &str, &str, &str); 6] = [
    (0, 1, "le sexe", "1 (homme) ou 2 (femme)", "sexe"),
    (1, 3, "l'annee de naissance", "00 a 99, les deux derniers chiffres", "annee"),
    (3, 5, "le mois de naissance", "01 a 12 -- ou 20, 30 a 42, 50 a 99 pour une immatriculation particuliere", "mois"),
    (5, 7, "le departement de naissance", "01 a 99, ou 2A ou 2B", "departement"),
    (7, 10, "la commune de naissance", "001 a 999", "commune"),
    (10, 13, "le numero d'ordre a l'etat civil", "001 a 999", "ordre a l'etat civil"),
];

/// `_morceau_de_nir_va` : les bornes du cahier technique, morceau par morceau.
fn morceau_de_nir_va(debut: usize, morceau: &str) -> bool {
    match debut {
        0 => morceau == "1" || morceau == "2",
        1 => tous_chiffres(morceau),
        3 => {
            if !tous_chiffres(morceau) {
                return false;
            }
            let m: i64 = morceau.parse().unwrap_or(-1);
            (1..=12).contains(&m) || m == 20 || (30..=42).contains(&m) || (50..=99).contains(&m)
        }
        5 => morceau == "2A" || morceau == "2B" || (tous_chiffres(morceau) && morceau.parse::<i64>().map_or(false, |n| (1..=99).contains(&n))),
        _ => tous_chiffres(morceau) && morceau.parse::<i64>().map_or(false, |n| n >= 1),
    }
}

/// `[0-9]{5}(2A|2B)[0-9]{6}`, en entier.
fn nir_de_corse(v: &str) -> bool {
    let c: Vec<char> = v.chars().collect();
    c.len() == 13
        && c[..5].iter().all(char::is_ascii_digit)
        && c[5] == '2'
        && (c[6] == 'A' || c[6] == 'B')
        && c[7..].iter().all(char::is_ascii_digit)
}

/// `explication_nir` : pourquoi ce NIR ne peut pas en etre un -- en francais.
fn explication_nir(nir: &str) -> R<Option<String>> {
    let v = strip(nir);
    if v.is_empty() {
        return Ok(None);
    }
    if !tous_chiffres(v) && !nir_de_corse(v) {
        return Ok(Some("un NIR ne porte que des chiffres (le departement de Corse excepte : 2A ou 2B)".into()));
    }
    let n = v.chars().count() as i64;
    if n != 13 {
        let manque = 13 - n;
        let morceaux: Vec<String> = MORCEAUX_NIR.iter().map(|(deb, fin, _, _, court)| format!("{} ({})", court, fin - deb)).collect();
        return Ok(Some(format!(
            "{} chiffres saisis, 13 attendus : il en {}. Un NIR se compose de {} ; sa cle, 2 chiffres de plus, se saisit dans la case d'a cote.",
            n,
            if manque > 0 { format!("manque {}", manque) } else { format!("a {} de trop", -manque) },
            morceaux.join(", ")
        )));
    }
    if v.starts_with(['7', '8']) {
        return Ok(Some(
            "le premier chiffre dit le sexe : 1 ou 2. Un numero provisoire commencant par 7 ou 8 ne se declare pas -- le cahier technique demande le NIA notifie par l'organisme, qui a la forme d'un NIR ; a defaut de NIR et de NIA, laisser cette case VIDE et renseigner le sexe : le numero technique temporaire (S21.G00.30.020) se fabrique alors tout seul".into(),
        ));
    }
    if v == "1999999999999" || v == "2999999999999" {
        return Ok(Some(
            "ce numero de remplissage est explicitement interdit par la norme (CCH-13). Pour un salarie pas encore immatricule, c'est le NIA notifie par l'organisme qui se saisit ici ; a defaut, laisser vide : la norme exige alors le numero technique temporaire (S21.G00.30.020), que le moteur fabrique du sexe, du SIREN et du matricule".into(),
        ));
    }
    if v.len() >= 6 && v.chars().rev().take_while(|c| *c == '9').count() >= 6 {
        return Ok(Some(
            "plus de cinq « 9 » a la suite en fin de numero : la norme ne l'admet pas (CCH-16). C'est la marque d'un numero complete au juge -- il vaut mieux une fiche qui refuse qu'une declaration qui identifie la mauvaise personne".into(),
        ));
    }
    if crate::formats::mauvaise(RUBRIQUE_NIR, v).map_err(plain)?.is_empty() {
        return Ok(None);
    }
    for (deb, fin, nom, attendu, _) in MORCEAUX_NIR {
        let morceau = tranche(v, deb, fin);
        if morceau_de_nir_va(deb, &morceau) {
            continue;
        }
        return Ok(Some(format!(
            "{} (caractere{} {}{}) vaut {} : la norme attend {}",
            nom,
            if fin - deb > 1 { "s" } else { "" },
            deb + 1,
            if fin - deb > 1 { format!(" a {}", fin) } else { String::new() },
            morceau,
            attendu
        )));
    }
    Ok(None)
}

/// `cle_nir` : 97 - (NIR mod 97), sur deux chiffres (`reference/sources/insee-nir-siret/`, INSEE, schema NIR, 4.1). En Corse,
/// 2A et 2B au 6e-7e rang valent 19 et 18 (equivalent, par inference, au zero moins 1 000 000 ou 2 000 000 du texte).
pub fn cle_nir(nir: &str) -> Option<String> {
    let mut n = strip(nir).to_uppercase();
    if n.chars().count() != 13 {
        return None;
    }
    if tranche(&n, 5, 7) == "2A" {
        n = n.replacen("2A", "19", 1);
    }
    if tranche(&n, 5, 7) == "2B" {
        n = n.replacen("2B", "18", 1);
    }
    // Les chiffres decimaux de toutes les ecritures comptent comme des chiffres.
    let mut x: u64 = 0;
    for c in n.chars() {
        x = x * 10 + valeur_decimale(c)? as u64;
    }
    Some(format!("{:02}", 97 - x % 97))
}

/// La valeur d'un chiffre decimal Unicode (categorie Nd).
fn valeur_decimale(c: char) -> Option<u32> {
    if let Some(d) = c.to_digit(10) {
        return Some(d);
    }
    // Le zero de chaque suite de dix chiffres decimaux (Unicode 15).
    const ZEROS: [u32; 65] = [
        0x660, 0x6F0, 0x7C0, 0x966, 0x9E6, 0xA66, 0xAE6, 0xB66, 0xBE6, 0xC66, 0xCE6, 0xD66, 0xDE6, 0xE50, 0xED0, 0xF20, 0x1040, 0x1090, 0x17E0,
        0x1810, 0x1946, 0x19D0, 0x1A80, 0x1A90, 0x1B50, 0x1BB0, 0x1C40, 0x1C50, 0xA620, 0xA8D0, 0xA900, 0xA9D0, 0xA9F0, 0xAA50, 0xABF0, 0xFF10,
        0x104A0, 0x10D30, 0x11066, 0x110F0, 0x11136, 0x111D0, 0x112F0, 0x11450, 0x114D0, 0x11650, 0x116C0, 0x11730, 0x118E0, 0x11950, 0x11C50,
        0x11D50, 0x11DA0, 0x16A60, 0x16AC0, 0x16B50, 0x1D7CE, 0x1D7D8, 0x1D7E2, 0x1D7EC, 0x1D7F6, 0x1E140, 0x1E2F0, 0x1E4F0, 0x1E950,
    ];
    let p = c as u32;
    ZEROS.iter().find(|z| (**z..**z + 10).contains(&p)).map(|z| p - z).or_else(|| (0x1FBF0..0x1FBFA).contains(&p).then(|| p - 0x1FBF0))
}

/// `luhn` : la cle du SIRET (`reference/sources/insee-nir-siret/`, INSEE, schema SIRET, 8.1) : somme des chiffres, multiple de 10.
pub fn luhn(numero: &str) -> bool {
    if !tous_chiffres(numero) {
        return false;
    }
    let (mut total, mut pair) = (0u32, false);
    for c in numero.chars().rev() {
        let Some(mut n) = c.to_digit(10) else { return false };
        if pair {
            n *= 2;
            if n > 9 {
                n -= 9;
            }
        }
        total += n;
        pair = !pair;
    }
    total % 10 == 0
}

/// `forme_declarative` : la valeur telle qu'elle PARTIRA. Une date se saisit
/// en AAAA-MM-JJ et se declare en JJMMAAAA ; une date impossible rend vide.
fn forme_declarative(c: &Champ, valeur: &str) -> String {
    if c.forme == "date" {
        return match date_iso(valeur) {
            Some(j) => j.jjmmaaaa(),
            None => String::new(),
        };
    }
    valeur.to_string()
}

/// `faute_de_norme` : ce que la norme reproche a cette valeur -- `None` si elle passe.
pub fn faute_de_norme(c: &Champ, valeur: &str) -> R<Option<String>> {
    let Some(rubrique) = c.rubrique.as_deref().filter(|r| !r.is_empty()) else { return Ok(None) };
    if valeur.is_empty() {
        return Ok(None);
    }
    let v = forme_declarative(c, valeur);
    if v.is_empty() {
        return Ok(None);
    }
    let mut pourquoi = crate::formats::mauvaise(rubrique, &v).map_err(plain)?;
    if pourquoi.is_empty() {
        return Ok(None);
    }
    let refuses = crate::formats::caracteres_refuses(rubrique, &v).map_err(plain)?;
    if !refuses.is_empty() {
        let liste: Vec<String> = refuses.iter().map(|c| format!("\u{ab} {} \u{bb}", c)).collect();
        pourquoi = format!("{} n'y {} pas admis", liste.join(", "), if refuses.len() > 1 { "sont" } else { "est" });
    }
    Ok(Some(format!("{} (rubrique {} du cahier technique NEODeS) : le point de depot refuserait la declaration", pourquoi, rubrique)))
}

/// `_faute_de_forme_officielle` : les regles de position du cahier technique
/// DSN, section 4.
fn faute_de_forme_officielle(v: &str) -> Option<String> {
    let c: Vec<char> = v.chars().collect();
    if c.is_empty() {
        return None;
    }
    if c[0] == '-' || c[0] == ' ' {
        return Some("ne peut pas commencer par un trait d'union ni un espace (cahier technique DSN, section 4)".into());
    }
    if SEPARATEURS.contains(c[c.len() - 1]) {
        return Some("ne peut pas finir par un espace, un trait d'union, une apostrophe ou un point (cahier technique DSN, section 4)".into());
    }
    for w in c.windows(2) {
        let (a, b) = (w[0], w[1]);
        if SEPARATEURS.contains(a) && SEPARATEURS.contains(b) {
            // Deux exceptions prevues : le double trait d'union des noms
            // doubles, et le point suivi d'un espace.
            if (a == b && a == '-') || (a == '.' && b == ' ') {
                continue;
            }
            return Some(format!(
                "\u{ab} {}{} \u{bb} : espace, trait d'union, apostrophe et point s'emploient isoles, jamais l'un a cote de l'autre (cahier technique DSN, section 4)",
                a, b
            ));
        }
    }
    if v.matches("--").count() > 1 {
        return Some("le double trait d'union ne peut apparaitre qu'une fois, entre le premier et le second nom".into());
    }
    let haut = v.to_uppercase();
    if CIVILITES.iter().any(|civ| haut.starts_with(civ)) {
        return Some("une civilite dans une rubrique d'identite est une anomalie DSN : le sexe a sa propre rubrique".into());
    }
    None
}

/// `^[^@\s]+@[^@\s]+\.[^@\s]+$`.
fn courriel_valide(v: &str) -> bool {
    let c: Vec<char> = v.chars().collect();
    if c.iter().filter(|x| **x == '@').count() != 1 || c.iter().any(|x| espace(*x)) {
        return false;
    }
    let a = c.iter().position(|x| *x == '@').unwrap();
    if a == 0 {
        return false;
    }
    let domaine = &c[a + 1..];
    // un point avec au moins un caractere avant et un apres, dans le domaine
    (1..domaine.len().saturating_sub(1)).any(|i| domaine[i] == '.')
}

/// `verifier_champ` : un message si le champ est fautif, sinon `None`.
fn verifier_champ(c: &Champ, valeur: Option<&Value>) -> R<Option<String>> {
    let vide = match valeur {
        None | Some(Value::Null) => true,
        Some(Value::String(s)) => strip(s).is_empty(),
        _ => false,
    };
    if vide {
        return Ok(if c.obligatoire { Some("obligatoire".into()) } else { None });
    }
    let Some(Value::String(brut)) = valeur else { return Ok(None) };
    // La signature des courriels est du HTML colle : sa seule regle est son poids (150 Ko, images comprises).
    if c.cle == CLE_SIGNATURE {
        return Ok(trop_lourde(brut));
    }
    if c.forme == "structure" || c.forme == "booleen" {
        return Ok(None);
    }
    let v = strip(brut);
    if c.forme == "officiel" {
        if let Some(faute) = faute_de_forme_officielle(v) {
            return Ok(Some(faute));
        }
        let attendu: String = sans_accents(v).chars().flat_map(char::to_uppercase).collect();
        if v != attendu {
            return Ok(Some(format!(
                "doit etre en MAJUSCULES SANS ACCENTS : ce champ part dans un format a largeur fixe (AEM, DPAE, DSN). Forme attendue : {}",
                attendu
            )));
        }
    }
    // LE NIR A SON EXPLICATION A LUI, et elle passe devant tout le reste.
    if c.rubrique.as_deref() == Some(RUBRIQUE_NIR) {
        if let Some(explique) = explication_nir(v)? {
            return Ok(Some(explique));
        }
    }
    if c.forme == "chiffres" && !tous_chiffres(v) {
        return Ok(Some("chiffres uniquement".into()));
    }
    if c.forme == "date" && date_iso(v).is_none() {
        return Ok(Some("date attendue au format AAAA-MM-JJ".into()));
    }
    if c.forme == "email" && !courriel_valide(v) {
        return Ok(Some("adresse de courriel attendue".into()));
    }
    if c.forme == "montant" && !flottant_valide(&v.replace(',', ".")) {
        return Ok(Some("nombre attendu".into()));
    }
    if c.forme == "choix" && !c.choix.is_empty() && !c.choix.iter().any(|x| x.as_str() == Some(v)) {
        let liste: Vec<String> = c.choix.iter().map(valeur_en_texte).collect();
        return Ok(Some(format!("valeur attendue parmi : {}", liste.join(", "))));
    }
    // UN PAYS SE CHOISIT DANS LA LISTE LIVREE (`pays.rs`) : un code qu'elle ne connait pas refuse.
    // Hors de la liste : un pays de nationalite s'ecrit a part. Le pays de naissance se lit dans la table
    // « Pays » de la DSN (`pays::de_la_table_dsn`), le Kosovo et les Etats disparus compris.
    let connu = if c.cle == "pays_naissance" {
        socle::pays::de_la_table_dsn(v).map_err(plain)?.is_some()
    } else {
        socle::pays::du_code(v).map_err(plain)?.is_some()
    };
    if c.forme == "pays" && !connu {
        let quoi = format!("Le {}", c.libelle.to_lowercase());
        return Ok(Some(match c.cle.as_str() {
            "pays_naissance" => socle::pays::inconnu_naissance(&quoi, v),
            "pays_nationalite" => socle::pays::inconnu_nationalite(&quoi, v),
            _ => socle::pays::inconnu(&quoi, v),
        }));
    }
    // LE NUMERO D'ADHERENT CONGES SPECTACLES : dix chiffres et une lettre, dont la DSN tire S21.G00.40.022 (97 + ce
    // numero) ; la forme et sa source sont a un seul endroit, `conges_spectacles::faute_adherent`.
    if c.cle == documents::conges_spectacles::CLE_ADHERENT {
        if let Some(faute) = documents::conges_spectacles::faute_adherent(&documents::conges_spectacles::forme_adherent(v)) {
            return Ok(Some(faute));
        }
    }
    // LE CODE APE SE RECONNAIT DANS LA NAF REV. 2 OU LA NAF 2025 (`naf.rs`) : les deux sont admises a la saisie,
    // un code qu'aucune ne connait refuse.
    if c.cle == "naf" {
        if let Some(faute) = crate::naf::faute_de_saisie(v).map_err(plain)? {
            return Ok(Some(faute));
        }
    }
    if let Some(l) = c.longueur.filter(|l| *l != 0) {
        let n = v.chars().count() as i64;
        if n != l {
            return Ok(Some(format!("{} caracteres attendus, {} saisis", l, n)));
        }
    }
    // EN DERNIER : la norme ne parle que de ce qui a deja la bonne forme generale.
    faute_de_norme(c, v)
}

/// `a[debut:fin]` sur ce qu'un JSON porte : un texte et une liste se
/// tranchent, un dictionnaire ne se tranche pas, le reste n'est pas indicable.
fn tranche_de(v: &Value, a: usize, b: usize) -> R<Value> {
    match v {
        Value::String(t) => Ok(Value::String(tranche(t, a, b))),
        Value::Array(l) => Ok(Value::Array(l.iter().skip(a).take(b.saturating_sub(a)).cloned().collect())),
        Value::Object(_) => Err(erreur("TypeError", socle::fichiers::attendu("un texte", "dict"))),
        autre => Err(erreur("TypeError", socle::fichiers::attendu("un texte", socle::valeur::nom_type(autre)))),
    }
}

/// `tranche == texte`, pour une tranche (texte ou liste).
fn eq_py_texte(t: &Value, texte: &str) -> bool {
    t.as_str() == Some(texte)
}

/// `a + b` sur ce qu'un JSON porte.
fn additionner(a: &Value, b: &Value) -> R<Value> {
    use socle::valeur::nom_type;
    match (a, b) {
        (Value::String(x), Value::String(y)) => Ok(Value::String(format!("{}{}", x, y))),
        (Value::String(_), _) => Err(erreur("TypeError", format!("can only concatenate str (not \"{}\") to str", nom_type(b)))),
        (Value::Array(x), Value::Array(y)) => Ok(Value::Array(x.iter().chain(y).cloned().collect())),
        (Value::Array(_), _) => Err(erreur("TypeError", format!("can only concatenate list (not \"{}\") to list", nom_type(b)))),
        (Value::Number(_) | Value::Bool(_), Value::Number(_) | Value::Bool(_)) => {
            let ent = |v: &Value| match v {
                Value::Bool(t) => Some(*t as i64),
                Value::Number(n) => n.as_i64(),
                _ => None,
            };
            match (ent(a), ent(b)) {
                (Some(x), Some(y)) => Ok(json!(x + y)),
                _ => Ok(json!(socle::valeur::nombre(a).unwrap_or(0.0) + socle::valeur::nombre(b).unwrap_or(0.0))),
            }
        }
        _ => Err(erreur("TypeError", format!("addition impossible : {} et {}", socle::fichiers::nature(nom_type(a)), socle::fichiers::nature(nom_type(b))))),
    }
}

/// `d.get(cle) or ""`, lu comme un texte : un autre type n'a pas de `strip`.
fn texte_de(d: &Map<String, Value>, cle: &str) -> R<String> {
    match d.get(cle) {
        None | Some(Value::Null) => Ok(String::new()),
        Some(Value::String(s)) => Ok(s.clone()),
        Some(v) if !vrai(v) => Ok(String::new()),
        Some(v) => Err(erreur("AttributeError", socle::fichiers::attendu("un texte", socle::valeur::nom_type(v)))),
    }
}

fn anomalie(champ: &str, gravite: &str, message: impl Into<String>) -> Value {
    json!({"champ": champ, "gravite": gravite, "message": message.into()})
}

/// `_controles_croises` : ce qu'aucun champ ne peut verifier seul.
fn controles_croises(nom: &str, d: &Map<String, Value>) -> R<Vec<Value>> {
    let mut out = Vec::new();

    // UNE AUTRE URSSAF QUE CELLE DE L'ADRESSE : permis, mais dit, et par le moteur, pour que la
    // ligne de commande l'entende comme la fenetre. C'est l'Urssaf du territoire qui recoit DPAE et DSN.
    if nom == "entreprise" {
        let choisi = strip(&d.get("code_urssaf").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default()).to_string();
        if !choisi.is_empty() {
            if let Ok(base) = socle::fichiers::json_en_cache(&socle::fichiers::dossier_du_code().join("urssaf-dpae-base.json")) {
                let deduit = urssaf_du_code_postal(&base, d.get("code_postal"));
                if !deduit.is_empty() && deduit != choisi {
                    out.push(anomalie(
                        "code_urssaf",
                        "avertissement",
                        format!(
                            "l'Urssaf de votre adresse est la {} ; c'est elle qui reçoit normalement vos DPAE et DSN. Une Urssaf qui ne vous gère pas peut les faire rejeter.",
                            deduit
                        ),
                    ));
                }
            }
        }
    }

    if nom == "spectacle" {
        let code = strip(&d.get("code_analytique").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default()).to_string();
        if !code.is_empty() && !code.chars().all(|c| c.is_ascii() && (c.is_alphanumeric() || c == '-' || c == '_')) {
            out.push(anomalie(
                "code_analytique",
                "erreur",
                "lettres sans accent, chiffres, tiret ou souligné seulement : les fichiers des logiciels de comptabilité coupent sur une espace, une virgule ou un point-virgule.",
            ));
        }
    }

    if nom == "salarie" {
        let nir = strip(&texte_de(d, "nir")?).to_string();
        let cle = strip(&texte_de(d, "nir_cle")?).to_string();
        // SANS NIR, LA FICHE N'EST PAS INCOMPLETE : elle est celle d'un salarie
        // pas encore immatricule, sous son numero technique temporaire, qui
        // commence par le sexe.
        let sexe_saisi = strip(&texte_de(d, "sexe")?).to_string();
        if nir.is_empty() && sexe_saisi.is_empty() {
            out.push(anomalie(
                "sexe",
                "erreur",
                "obligatoire tant que le NIR manque : sans NIR ni NIA, la norme declare le salarie sous un numero technique temporaire (S21.G00.30.020) qui commence par le code sexe. Le reste -- le SIREN, le matricule -- le moteur l'a deja.",
            ));
        } else if nir.is_empty() {
            out.push(anomalie(
                "nir",
                "avertissement",
                "pas de NIR : ce salarie sera declare sous un numero technique temporaire, qui l'identifie dans la DSN mais n'ouvre aucun droit. Des que la caisse notifie le NIR ou le NIA, le saisir ici -- la premiere declaration suivante portera les deux, pour faire le lien.",
            ));
        }
        let premier = tranche(&nir, 0, 1);
        if !nir.is_empty() && !sexe_saisi.is_empty() && (premier == "1" || premier == "2") && sexe_saisi != premier {
            out.push(anomalie(
                "sexe",
                "erreur",
                format!("le NIR commence par {}, cette case dit {}. Les deux ne peuvent pas etre vrais ; c'est le NIR qui a ete declare.", premier, sexe_saisi),
            ));
        }
        let attendue = if nir.is_empty() { None } else { cle_nir(&nir) };
        if let Some(a) = &attendue {
            if !cle.is_empty() && *a != cle {
                out.push(anomalie(
                    "nir_cle",
                    "erreur",
                    format!("cle incoherente avec le NIR : {} attendue. Un NIR faux passe les controles de format et se voit des mois plus tard, dans un bilan d'identification.", a),
                ));
            }
        }
        if nir.is_empty() != cle.is_empty() {
            out.push(anomalie(
                if nir.is_empty() { "nir" } else { "nir_cle" },
                "erreur",
                format!(
                    "le NIR et sa cle vont ensemble : {}",
                    if nir.is_empty() {
                        "une cle sans NIR ne verifie rien -- si le salarie n'est pas immatricule, effacer aussi la cle"
                    } else {
                        "il manque la cle, les deux chiffres qui le suivent"
                    }
                ),
            ));
        }
        if d.get("date_naissance").map_or(false, vrai) && nir.chars().count() == 13 && tous_chiffres(&tranche(&nir, 1, 5)) {
            let (an, mo) = (tranche(&nir, 1, 3), tranche(&nir, 3, 5));
            let nee = d.get("date_naissance").cloned().unwrap_or(Value::Null);
            let (n24, n57) = (tranche_de(&nee, 2, 4)?, tranche_de(&nee, 5, 7)?);
            if !eq_py_texte(&n24, &an) || (mo != "00" && !eq_py_texte(&n57, &mo)) {
                out.push(anomalie(
                    "nir",
                    "avertissement",
                    format!("le NIR annonce {}/{}, la date de naissance dit {}/{}", mo, an, valeur_en_texte(&n57), valeur_en_texte(&n24)),
                ));
            }
        }
        // LA CODIFICATION UE (S21.G00.30.013) : un ancien 03 sans pays de nationalite, ou un pays qui contredit
        // la case, se dit ici, avant la DSN qui le refuserait. La regle est dans `codification_ue`, pas ici.
        // Un pays de nationalite hors de la liste se dit sur sa propre case (forme « pays »), pas deux fois.
        // Un pays ecrit « hors de la liste » qui n'en est pas un, ou a cote d'un pays de la liste, se dit sur
        // sa case a lui ; sinon c'est la nationalite choisie qui manque ou qui ne va pas, sur la sienne.
        let fiche = Value::Object(d.clone());
        let pays_inconnu = match d.get("pays_nationalite").filter(|v| vrai(v)) {
            Some(Value::String(p)) => socle::pays::du_code(p).map_err(plain)?.is_none(),
            _ => false,
        };
        let hors_liste = d.get("pays_nationalite_hors_liste").map_or(false, vrai);
        if let Some(m) = socle::codification_ue::faute_hors_liste(&fiche).map_err(plain)? {
            out.push(anomalie("pays_nationalite_hors_liste", "erreur", m));
        } else if !pays_inconnu
            && (hors_liste || d.get("code_nationalite_dsn").map_or(false, vrai) || d.get("pays_nationalite").map_or(false, vrai))
        {
            if let Err(m) = socle::codification_ue::codification_ue(&fiche) {
                out.push(anomalie("code_nationalite_dsn", "erreur", m));
            }
        }
        // LES BORNES D'UNE DISPENSE DE SANTE (Q13) : elles bornent un motif, et la fin ne precede pas le debut. Ce que
        // le mois en fait est dans `calcul::dispense_de_la_periode`.
        let borne = |k: &str| d.get(k).filter(|v| vrai(v)).map(valeur_en_texte);
        let (depuis, jusqu_au) = (borne("sante_dispense_depuis"), borne("sante_dispense_jusqu_au"));
        if (depuis.is_some() || jusqu_au.is_some()) && !d.get("sante_dispense").is_some_and(vrai) {
            out.push(anomalie(
                "sante_dispense",
                "erreur",
                "la dispense a des dates mais pas de motif : écrire le motif de la dispense (la demande écrite du salarié), ou retirer ses dates".to_string(),
            ));
        }
        if let (Some(a), Some(b)) = (&depuis, &jusqu_au) {
            if date_iso(a).is_some() && date_iso(b).is_some() && b < a {
                out.push(anomalie("sante_dispense_jusqu_au", "erreur", format!("la dispense finirait ({}) avant de commencer ({})", b, a)));
            }
        }
        // Le code emploi, le statut et le PCS de la fiche, confrontes au
        // catalogue des metiers. Avertissements, pas erreurs.
        for msg in droit::emplois::controler_fiche(d)? {
            out.push(anomalie("code_emploi_ft", "avertissement", msg));
        }
    }

    if nom == "entreprise" {
        let brut = |k: &str| d.get(k).filter(|v| vrai(v)).cloned().unwrap_or_else(|| Value::String(String::new()));
        let (siret, siren, nic) = (brut("siret"), brut("siren"), brut("nic"));
        if vrai(&siret) && vrai(&siren) && vrai(&nic) {
            let somme = additionner(&siren, &nic)?;
            if !socle::valeur::valeurs_egales(&siret, &somme) {
                out.push(anomalie("siret", "erreur", format!("SIRET != SIREN + NIC ({})", valeur_en_texte(&somme))));
            }
        }
        if vrai(&siret) {
            let longueur = match &siret {
                Value::String(t) => t.chars().count(),
                Value::Array(a) => a.len(),
                Value::Object(o) => o.len(),
                autre => return Err(erreur("TypeError", socle::fichiers::attendu("un texte ou une liste", socle::valeur::nom_type(autre)))),
            };
            if longueur == 14 {
                match &siret {
                    Value::String(t) => {
                        if !luhn(t) {
                            out.push(anomalie("siret", "erreur", "ce numéro ne peut pas être un SIRET : un chiffre est probablement mal tapé"));
                        }
                    }
                    autre => return Err(erreur("AttributeError", socle::fichiers::attendu("un texte", socle::valeur::nom_type(autre)))),
                }
            }
        }
    }
    Ok(out)
}

/// `verifier_fiche` : toutes les anomalies d'une fiche, champ par champ, puis
/// les croisees. `gravite` vaut « erreur » quand la donnee est certainement
/// fausse, « avertissement » quand elle est seulement suspecte.
pub fn verifier_fiche(nom: &str, donnees: &Map<String, Value>) -> R<Vec<Value>> {
    let champs = fiche(nom)?;
    let donnees = combler_affichage(nom, donnees)?;
    let mut anomalies: Vec<Value> = Vec::new();
    for c in &champs {
        let Some(msg) = verifier_champ(c, donnees.get(&c.cle))? else { continue };
        // Un champ derive n'est pas saisi : le signaler a l'humain reviendrait
        // a lui reprocher le travail de la machine. On pointe la forme
        // lisible, qui est ce qu'il peut corriger.
        if c.calcule() {
            let message = if msg == "obligatoire" { msg } else { format!("{} (la forme declaree, « {} », s'en deduit)", msg, c.libelle) };
            anomalies.push(anomalie(c.derive_de.as_deref().unwrap_or(""), "erreur", message));
        } else {
            anomalies.push(anomalie(&c.cle, "erreur", msg));
        }
    }
    anomalies.extend(controles_croises(nom, &donnees)?);
    // CE QUI MANQUE A UN SALARIE OU A UN SPECTACLE NE SE FORCE PAS. Une regle de forme peut se tromper et la valeur
    // etre juste : son erreur se passe outre a l'ecran (`forcable`). Une valeur obligatoire absente, non : le metier d'un
    // salarie (ses codes emploi, PCS-ESE, statut), son nom, sa naissance, son adresse, le numero d'objet d'un spectacle
    // partent dans la DPAE et la DSN, qui ne se font pas sans eux, et rien ne se devine (regle 1). La fiche entreprise se
    // remplit en plusieurs fois (« Pour demarrer » dit ce qui manque) : ses manques restent forcables.
    for a in anomalies.iter_mut() {
        let manque = a["gravite"] == "erreur" && a["message"] == "obligatoire" && nom != "entreprise";
        a["forcable"] = json!(!manque);
    }
    // Une fiche mise de cote ne doit pas crier. Ses manques restent visibles --
    // et redeviennent bloquants a la reactivation, ce que le message dit.
    if donnees.get("actif") == Some(&Value::Bool(false)) {
        for a in anomalies.iter_mut() {
            if a["gravite"] == "erreur" {
                a["gravite"] = json!("avertissement");
                let m = format!("{} (fiche inactive : a completer avant toute paie)", a["message"].as_str().unwrap_or(""));
                a["message"] = json!(m);
            }
        }
    }
    Ok(anomalies)
}

/// `urssaf.pour_departement` : l'organisme du TERRITOIRE de ce departement. Une
/// suggestion, pas une decision.
fn urssaf_du_departement(base: &Value, dep: &str) -> String {
    let d = dep.trim().to_uppercase();
    let table = base.get("par_departement").and_then(Value::as_object);
    let lu = |k: &str| table.and_then(|t| t.get(k)).filter(|v| vrai(v)).map(valeur_en_texte);
    lu(&d).or_else(|| if d == "2A" || d == "2B" { lu("20") } else { None }).unwrap_or_default()
}

/// `urssaf.pour_code_postal` : idem, depuis un code postal francais. L'outre-mer
/// se lit sur TROIS chiffres (97139 -> 971), la metropole sur deux.
pub fn urssaf_du_code_postal(base: &Value, cp: Option<&Value>) -> String {
    let v: Vec<char> = cp.filter(|x| vrai(x)).map(valeur_en_texte).unwrap_or_default().chars().filter(|c| c.is_ascii_digit()).collect();
    if v.len() < 2 {
        return String::new();
    }
    let n = if v[0] == '9' && (v[1] == '7' || v[1] == '8') { 3 } else { 2 };
    urssaf_du_departement(base, &v.iter().take(n).collect::<String>())
}

/// La signature des courriels de la fiche Entreprise : du HTML colle, dont la seule regle est le poids.
pub const CLE_SIGNATURE: &str = "courriel_signature";
/// 150 Ko, tout le HTML compris.
pub const LIMITE_OCTETS: usize = 150 * 1024;

/// La phrase d'une signature trop lourde, ou rien.
pub fn trop_lourde(html: &str) -> Option<String> {
    let n = html.len();
    if n > LIMITE_OCTETS {
        Some(format!(
            "la signature pese {} Ko, 150 Ko au plus, images comprises. Une image collee dans la signature pese tout son poids ; un lien vers une image en ligne presque rien",
            (n + 1023) / 1024
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La fiche offre ce que la norme 2026.1 admet : 40.003, 01, 02, 04, 98, 99, pas 03 ; 40.002, 03 « cadre dirigeant »,
    /// que la CCH-11 de 40.003 admet pour un cadre (cahier technique 2026.1, p. 182-183, `bloc-S21.G00.40.txt` l. 133-196).
    /// La fiche entreprise admet un code de la NAF rev. 2 ou de la NAF 2025 (decret n° 2025-736), et refuse le reste
    /// (`atelier/reference/sources/insee-naf-2025/legifrance-decret-2025-736-naf-2025-20250731.txt`).
    #[test]
    fn la_fiche_entreprise_admet_un_code_des_deux_nomenclatures() {
        let faute_naf = |code: &str| -> Option<String> {
            let mut d = Map::new();
            d.insert("naf".into(), Value::String(code.into()));
            verifier_fiche("entreprise", &d).unwrap().into_iter().find(|a| a["champ"] == "naf").map(|a| a["message"].as_str().unwrap().to_string())
        };
        assert_eq!(faute_naf("9001Z"), None, "NAF rev. 2");
        assert_eq!(faute_naf("9020Y"), None, "NAF 2025");
        assert_eq!(faute_naf("9039G"), None, "NAF 2025");
        let e = faute_naf("9099Z").expect("un code qu'aucune nomenclature ne connait refuse");
        assert!(e.contains("ni de la NAF rev. 2 ni de la NAF 2025"), "{}", e);
        let e = faute_naf("90.20Y").expect("le point refuse");
        assert!(e.contains("sans le point : 9020Y"), "{}", e);
    }

    #[test]
    fn la_fiche_offre_les_statuts_que_la_norme_admet() {
        let erreur_sur = |champ: &str, valeur: &str| -> bool {
            let mut f = Map::new();
            f.insert(champ.into(), Value::String(valeur.into()));
            verifier_fiche("salarie", &f).unwrap().iter().any(|a| a["champ"] == champ && a["gravite"] == "erreur")
        };
        assert!(erreur_sur("statut_categoriel_rc", "03"), "03 n'est pas un statut categoriel de retraite complementaire en 2026.1");
        for rc in ["01", "02", "04"] {
            assert!(!erreur_sur("statut_categoriel_rc", rc), "{}", rc);
        }
        assert!(!erreur_sur("statut_conventionnel", "03"), "03 cadre dirigeant est un classement conventionnel");
    }

    #[test]
    fn la_cle_du_nir_et_celle_de_la_corse() {
        // insee-nir-siret : le complement a 97, 2A et 2B retranches de 1 000 000 et 2 000 000.
        assert_eq!(cle_nir("1850578999048"), Some("19".into()));
        assert!(cle_nir("185057899904").is_none());
        // 2A vaut 19 et 2B vaut 18 : un NIR corse a sa cle.
        assert_eq!(cle_nir("185052A999048"), Some("60".into()));
        assert_eq!(cle_nir("185052B999048"), Some("87".into()));
        assert!(explication_nir("185052A999048").unwrap().is_none());
        assert!(cle_nir("18505ZZ999048").is_none());
    }

    #[test]
    fn le_siret_se_controle_par_luhn() {
        // insee-nir-siret : la somme des chiffres (rang impair doubles) est un multiple de 10.
        assert!(luhn("73282932000074"));
        assert!(!luhn("73282932000075"));
    }
}
