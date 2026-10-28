//! Lire une feuille d'un classeur .xlsx, pour les releves de l'atelier (`moteur extraire`, `extraire/`).
//!
//! Depuis la norme 2027.1.2, net-entreprises ne sert plus le tableau des usages et les formats des rubriques que dans des
//! classeurs (fiches 3357 et 3358 de la base de connaissances : `atelier/reference/norme-dsn/dsn-tableau-des-usages-*.xlsx`,
//! `dsn-tableaux-categories-controles-datatypes-usages-*.xlsx`), et l'INSEE sert la NAF 2025 de meme
//! (`atelier/reference/sources/insee-naf-2025/*.xlsx`). Un .xlsx est une archive zip de XML (ECMA-376, SpreadsheetML) :
//! `zip` et `roxmltree` suffisent. Ce qui est lu est la valeur de la cellule telle que le classeur la porte : texte
//! partage, texte en ligne, nombre, booleen. RIEN N'EST INTERPRETE : pas de formule recalculee (la valeur gardee par le
//! classeur, `<v>`), pas de format applique (une date reste le nombre que le classeur ecrit), pas de cellule fusionnee
//! recopiee (seule la premiere case d'une fusion porte une valeur ; les autres sont vides).
//!
//! `lignes(chemin, "Fields")` rend `[[cellule, ...], ...]`, `None` pour une case vide.

use std::io::Read;
use std::path::Path;

use crate::num::Resultat;

const NS_M: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const NS_P: &str = "http://schemas.openxmlformats.org/package/2006/relationships";

/// La valeur d'une case renseignee. `t="s"`, `"inlineStr"`, `"str"` et `"e"` donnent du texte ; `"b"` un booleen ; le
/// reste (`"n"`, rien) un nombre : flottant s'il porte `.`, `e` ou `E`, entier sinon.
#[derive(Clone, Debug, PartialEq)]
pub enum Cellule {
    Texte(String),
    Entier(i64),
    Flottant(f64),
    Booleen(bool),
}

/// Une ligne : ses cases de la premiere colonne a la derniere renseignee (`None` : case vide).
pub type Ligne = Vec<Option<Cellule>>;

impl Cellule {
    /// La case en texte : le texte tel quel, un flottant sous sa forme ecrite (`1.0`, `1e-05`), `True`/`False`.
    pub fn en_texte(&self) -> String {
        match self {
            Cellule::Texte(s) => s.clone(),
            Cellule::Entier(i) => i.to_string(),
            Cellule::Flottant(f) => crate::ecriture::repr_flottant(*f),
            Cellule::Booleen(b) => if *b { "True" } else { "False" }.to_string(),
        }
    }

    /// Un texte vide, un zero et `False` sont faux.
    pub fn vrai(&self) -> bool {
        match self {
            Cellule::Texte(s) => !s.is_empty(),
            Cellule::Entier(i) => *i != 0,
            Cellule::Flottant(f) => *f != 0.0,
            Cellule::Booleen(b) => *b,
        }
    }

    /// La valeur en JSON (le flottant reste flottant, l'entier entier).
    pub fn json(&self) -> serde_json::Value {
        match self {
            Cellule::Texte(s) => serde_json::Value::String(s.clone()),
            Cellule::Entier(i) => serde_json::Value::from(*i),
            Cellule::Flottant(f) => {
                serde_json::Number::from_f64(*f).map_or(serde_json::Value::Null, serde_json::Value::Number)
            }
            Cellule::Booleen(b) => serde_json::Value::Bool(*b),
        }
    }
}

/// Une case lue en texte : `None` pour une case vide.
pub fn cellule_en_texte(c: &Option<Cellule>) -> String {
    c.as_ref().map_or_else(|| "None".to_string(), Cellule::en_texte)
}

struct Classeur {
    archive: zip::ZipArchive<std::fs::File>,
    nom: String,
}

impl Classeur {
    fn ouvrir(chemin: &Path) -> Resultat<Classeur> {
        let f = std::fs::File::open(chemin).map_err(|e| format!("{} illisible ({})", chemin.display(), e))?;
        let archive = zip::ZipArchive::new(f)
            .map_err(|e| format!("{} n'est pas un classeur .xlsx lisible ({})", chemin.display(), e))?;
        Ok(Classeur { archive, nom: chemin.display().to_string() })
    }

    fn present(&self, membre: &str) -> bool {
        self.archive.index_for_name(membre).is_some()
    }

    fn lire(&mut self, membre: &str) -> Resultat<String> {
        let mut f = self
            .archive
            .by_name(membre)
            .map_err(|e| format!("{} : {} absent du classeur ({})", self.nom, membre, e))?;
        let mut octets = Vec::new();
        f.read_to_end(&mut octets).map_err(|e| format!("{} : {} illisible ({})", self.nom, membre, e))?;
        String::from_utf8(octets).map_err(|_| format!("{} : {} n'est pas en UTF-8", self.nom, membre))
    }
}

fn analyser<'a>(nom: &str, membre: &str, texte: &'a str) -> Resultat<roxmltree::Document<'a>> {
    let options = roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() };
    roxmltree::Document::parse_with_options(texte, options)
        .map_err(|e| format!("{} : {} illisible ({})", nom, membre, e))
}

fn est(n: &roxmltree::Node, ns: &str, nom: &str) -> bool {
    n.is_element() && n.tag_name().namespace() == Some(ns) && n.tag_name().name() == nom
}

/// Le premier enfant `m:<nom>` (`find("m:nom")`).
fn enfant<'a, 'i>(n: roxmltree::Node<'a, 'i>, nom: &str) -> Option<roxmltree::Node<'a, 'i>> {
    n.children().find(|c| est(c, NS_M, nom))
}

/// Les enfants `m:<nom>` (`findall("m:nom")`).
fn enfants<'a, 'i: 'a>(n: roxmltree::Node<'a, 'i>, nom: &'a str) -> impl Iterator<Item = roxmltree::Node<'a, 'i>> + 'a {
    n.children().filter(move |c| est(c, NS_M, nom))
}

/// Le texte d'un `<si>` ou d'un `<is>` : tous ses `<t>`, bout a bout (texte riche et lecture phonetique compris, comme
/// `iter("{m}t")`).
fn texte_de(n: roxmltree::Node) -> String {
    n.descendants().filter(|d| est(d, NS_M, "t")).map(|t| t.text().unwrap_or("")).collect()
}

/// `C12` -> 2 (colonnes comptees depuis 0).
fn colonne(reference: &str) -> Resultat<usize> {
    let lettres: Vec<char> = reference.chars().take_while(|c| c.is_ascii_uppercase()).collect();
    if lettres.is_empty() {
        return Err(format!("reference de case illisible : {}", crate::fichiers::repr(reference)));
    }
    let mut n: usize = 0;
    for c in lettres {
        n = n
            .checked_mul(26)
            .and_then(|n| n.checked_add(c as usize - 64))
            .ok_or_else(|| format!("colonne hors d'atteinte : {}", reference))?;
    }
    Ok(n - 1)
}

/// Un entier ecrit en texte (blancs autour admis).
fn entier(texte: &str, quoi: &str) -> Resultat<i64> {
    texte.trim_matches(crate::texte::blanc)
        .parse::<i64>()
        .map_err(|_| format!("{} illisible : {}", quoi, crate::fichiers::repr(texte)))
}

/// Les noms des feuilles du classeur, dans l'ordre.
pub fn feuilles(chemin: &Path) -> Resultat<Vec<String>> {
    let mut c = Classeur::ouvrir(chemin)?;
    let texte = c.lire("xl/workbook.xml")?;
    noms_des_feuilles(&c.nom, &texte).map(|v| v.into_iter().map(|(nom, _)| nom).collect())
}

/// (nom, `r:id`) de chaque feuille de `xl/workbook.xml`.
fn noms_des_feuilles(nom: &str, texte: &str) -> Resultat<Vec<(String, Option<String>)>> {
    let doc = analyser(nom, "xl/workbook.xml", texte)?;
    let sheets = enfant(doc.root_element(), "sheets")
        .ok_or_else(|| format!("{} : xl/workbook.xml sans liste de feuilles", nom))?;
    Ok(sheets
        .children()
        .filter(|n| n.is_element())
        .map(|f| (f.attribute("name").unwrap_or("").to_string(), f.attribute((NS_R, "id")).map(str::to_string)))
        .collect())
}

/// Les lignes de la feuille nommee, de haut en bas, chacune la liste de ses cases de la premiere colonne a la derniere
/// renseignee ; une ligne absente du classeur est rendue vide. Une feuille inconnue refuse, en disant les feuilles.
pub fn lignes(chemin: &Path, feuille: &str) -> Resultat<Vec<Ligne>> {
    let mut c = Classeur::ouvrir(chemin)?;
    let classeur = c.lire("xl/workbook.xml")?;
    let liens = c.lire("xl/_rels/workbook.xml.rels")?;
    let noms = noms_des_feuilles(&c.nom, &classeur)?;
    let mut rid = None;
    for (nom, id) in &noms {
        if nom == feuille {
            rid = id.clone();
        }
    }
    let Some(rid) = rid else {
        let liste: Vec<&str> = noms.iter().map(|(n, _)| n.as_str()).collect();
        return Err(format!("feuille « {} » absente de {} (feuilles : {})", feuille, c.nom, liste.join(", ")));
    };
    let doc_liens = analyser(&c.nom, "xl/_rels/workbook.xml.rels", &liens)?;
    let mut cible = None;
    for l in doc_liens.root_element().children().filter(|n| est(n, NS_P, "Relationship")) {
        if l.attribute("Id") == Some(rid.as_str()) {
            cible = l.attribute("Target").map(str::to_string);
        }
    }
    let cible = cible.ok_or_else(|| format!("{} : la feuille « {} » ({}) n'a pas de cible", c.nom, feuille, rid))?;
    let mut chemin_feuille = cible.trim_start_matches('/').to_string();
    if !chemin_feuille.starts_with("xl/") {
        chemin_feuille = format!("xl/{}", chemin_feuille);
    }
    let mut partages = Vec::new();
    if c.present("xl/sharedStrings.xml") {
        let texte = c.lire("xl/sharedStrings.xml")?;
        let doc = analyser(&c.nom, "xl/sharedStrings.xml", &texte)?;
        partages = enfants(doc.root_element(), "si").map(texte_de).collect();
    }
    let texte = c.lire(&chemin_feuille)?;
    let doc = analyser(&c.nom, &chemin_feuille, &texte)?;
    let donnees = enfant(doc.root_element(), "sheetData")
        .ok_or_else(|| format!("{} : {} sans sheetData", c.nom, chemin_feuille))?;
    let mut rangs: Vec<Ligne> = Vec::new();
    for rang in enfants(donnees, "row") {
        let numero = entier(
            rang.attribute("r").ok_or_else(|| format!("{} : une ligne sans numero", c.nom))?,
            "numero de ligne",
        )?;
        let mut cellules: std::collections::BTreeMap<usize, Cellule> = std::collections::BTreeMap::new();
        for case in enfants(rang, "c") {
            let genre = case.attribute("t");
            let v = enfant(case, "v").and_then(|v| v.text());
            let valeur = if genre == Some("inlineStr") {
                let is = enfant(case, "is").ok_or_else(|| format!("{} : texte en ligne sans <is>", c.nom))?;
                Cellule::Texte(texte_de(is))
            } else if let Some(v) = v {
                match genre {
                    Some("s") => {
                        let i = entier(v, "indice de texte partage")?;
                        let i = usize::try_from(i)
                            .map_err(|_| format!("{} : indice de texte partage {} hors de la table", c.nom, i))?;
                        Cellule::Texte(
                            partages
                                .get(i)
                                .cloned()
                                .ok_or_else(|| format!("{} : indice de texte partage {} hors de la table", c.nom, i))?,
                        )
                    }
                    Some("str") | Some("e") => Cellule::Texte(v.to_string()),
                    Some("b") => Cellule::Booleen(v == "1"),
                    _ if v.contains(['.', 'e', 'E']) => Cellule::Flottant(
                        v.trim_matches(crate::texte::blanc)
                            .parse::<f64>()
                            .map_err(|_| format!("{} : nombre illisible {}", c.nom, crate::fichiers::repr(v)))?,
                    ),
                    _ => Cellule::Entier(entier(v, "nombre")?),
                }
            } else {
                continue;
            };
            let r = case.attribute("r").ok_or_else(|| format!("{} : une case sans reference", c.nom))?;
            cellules.insert(colonne(r)?, valeur);
        }
        while (rangs.len() as i64) < numero - 1 {
            rangs.push(Vec::new());
        }
        let largeur = cellules.keys().next_back().map_or(0, |m| m + 1);
        rangs.push((0..largeur).map(|j| cellules.get(&j).cloned()).collect());
    }
    Ok(rangs)
}
