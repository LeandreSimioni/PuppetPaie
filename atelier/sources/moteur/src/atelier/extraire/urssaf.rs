//! `moteur extraire urssaf <table-des-urssaf.csv> <liste-des-sites.csv> <cog-2015.zip> <sortie.json>` : les organismes de
//! recouvrement, leur code a trois chiffres et le departement dont chacun releve, pour `logiciel/donnees/urssaf-dpae-base.json`
//! (que lisent `dpae.rs`, `champs.rs`, `routes/fiches.rs`, `routes/ouverture.rs`).
//!
//! LA SOURCE : la « Table des Urssaf », publiee par l'Urssaf Caisse nationale sur
//! https://fichierdirect.declaration.urssaf.fr/TablesReference.htm parmi les tables de reference « proposees pour le
//! parametrage urssaf des logiciels de paye », deposee dans `atelier/reference/sources/` (`urssaf-table-des-urssaf-*.csv` ;
//! `logiciel/donnees/sources.json`, clef `urssaf-table-des-urssaf`, dit sa date et son condensat). On en tire la colonne
//! `Codification` (U117, U200, U917...) : les trois chiffres qui suivent le U sont le `C_URSSAF` de la DPAE -- positions
//! 133-136 du format, annexe II.1 de la norme. C'est la liste des codes VALIDES, celle que le point de depot oppose au rejet
//! « Code URSSAF invalide -- n'appartenant pas a la liste des Codes Urssaf ».
//!
//! LE TERRITOIRE SE LIT AILLEURS (22/09/2026). La Table des Urssaf ne porte que le code postal du SIEGE de chaque
//! organisme, pas sa circonscription. Deux sources publiees, deposees a cote, ferment la chaine sans rien inventer :
//!   * la Liste des sites et des activites du reseau (open data Urssaf, `urssaf-liste-des-sites-et-des-activites.csv`)
//!     porte `code_organisme_ur_regionale` A COTE de `code_ancienne_region` : c'est le pont region -> code Urssaf, publie
//!     (11 -> 117, 82 -> 827, 94 -> 200) ;
//!   * le COG 2015 de l'Insee (`insee-cog-2015-departements.zip`) donne les 101 departements avec leur region. Le
//!     millesime 2015 expres : le dernier AVANT la reforme, donc celui des ANCIENNES regions -- et les codes Urssaf sont
//!     restes sur ce decoupage-la (827 Rhone-Alpes et 837 Auvergne coexistent alors que la region est une seule depuis
//!     2016).
//! La chaine est donc departement -> ancienne region -> code Urssaf. Trois precisions :
//!   - la NORMANDIE porte 237 et 257 dans la liste des sites, heritage de la Haute et de la Basse ; seul 287 figure dans la
//!     Table des Urssaf, et c'est lui qu'on retient : un code qui n'est pas dans la table officielle n'entre pas ;
//!   - l'OUTRE-MER n'a pas de code d'ancienne region utilisable ici : le code de la CGSS EST celui du departement (971
//!     Guadeloupe, 974 Reunion...), comme la Table des Urssaf le montre ;
//!   - CE N'EST QU'UNE SUGGESTION. Une compagnie en versement en lieu unique, une grande entreprise rattachee a un pole
//!     TGE, un employeur qui passe par le GUSO ne suivent pas la geographie : l'ecran presente le code deduit, la compagnie
//!     le corrige si le sien differe.
//!
//! Portage de `atelier/outils/extraire_urssaf.py`, qui ignorait ses arguments et ecrivait des qu'on le lancait : ici les
//! trois sources et la sortie se nomment.

use std::collections::BTreeSet;
use std::io::Read;
use std::path::Path;

use serde_json::{json, Map, Value};

use super::{date_du_releve, nom_de, Releve};
use socle::date::Jour;
use socle::fichiers::repr;
use socle::num::Resultat;
use socle::texte::strip;

/// Les caracteres de windows-1252 de 0x80 a 0x9F (`None` : non attribue, que le codec de Python refuse).
const CP1252: [Option<char>; 32] = [
    Some('\u{20ac}'),
    None,
    Some('\u{201a}'),
    Some('\u{192}'),
    Some('\u{201e}'),
    Some('\u{2026}'),
    Some('\u{2020}'),
    Some('\u{2021}'),
    Some('\u{2c6}'),
    Some('\u{2030}'),
    Some('\u{160}'),
    Some('\u{2039}'),
    Some('\u{152}'),
    None,
    Some('\u{17d}'),
    None,
    None,
    Some('\u{2018}'),
    Some('\u{2019}'),
    Some('\u{201c}'),
    Some('\u{201d}'),
    Some('\u{2022}'),
    Some('\u{2013}'),
    Some('\u{2014}'),
    Some('\u{2dc}'),
    Some('\u{2122}'),
    Some('\u{161}'),
    Some('\u{203a}'),
    Some('\u{153}'),
    None,
    Some('\u{17e}'),
    Some('\u{178}'),
];

fn cp1252(octets: &[u8], nom: &str) -> Resultat<String> {
    octets
        .iter()
        .map(|&b| match b {
            0x80..=0x9f => {
                CP1252[(b - 0x80) as usize].ok_or_else(|| format!("{} : octet 0x{:02x} hors de windows-1252", nom, b))
            }
            _ => Ok(b as char),
        })
        .collect()
}

fn latin1(octets: &[u8]) -> String {
    octets.iter().map(|&b| b as char).collect()
}

/// `csv.reader(texte, delimiter=d)` de Python (guillemets doubles, un guillemet n'ouvre qu'en debut de champ ; une ligne
/// vide ne donne aucun rang, comme `DictReader` les saute).
fn rangs_csv(texte: &str, delim: char) -> Vec<Vec<String>> {
    let (mut rangs, mut rang, mut champ): (Vec<Vec<String>>, Vec<String>, String) =
        (Vec::new(), Vec::new(), String::new());
    let (mut entre_guillemets, mut debut_de_champ) = (false, true);
    let mut it = texte.chars().peekable();
    while let Some(c) = it.next() {
        if entre_guillemets {
            if c == '"' {
                if it.peek() == Some(&'"') {
                    champ.push('"');
                    it.next();
                } else {
                    entre_guillemets = false;
                }
            } else {
                champ.push(c);
            }
            continue;
        }
        match c {
            '"' if debut_de_champ => {
                entre_guillemets = true;
                debut_de_champ = false;
            }
            c if c == delim => {
                rang.push(std::mem::take(&mut champ));
                debut_de_champ = true;
            }
            '\r' | '\n' => {
                if c == '\r' && it.peek() == Some(&'\n') {
                    it.next();
                }
                if !(rang.is_empty() && champ.is_empty() && debut_de_champ) {
                    rang.push(std::mem::take(&mut champ));
                    rangs.push(std::mem::take(&mut rang));
                }
                debut_de_champ = true;
            }
            c => {
                champ.push(c);
                debut_de_champ = false;
            }
        }
    }
    if !(rang.is_empty() && champ.is_empty() && debut_de_champ) {
        rang.push(champ);
        rangs.push(rang);
    }
    rangs
}

/// `csv.DictReader` : l'entete, puis chaque rang en (nom de colonne -> valeur) ; une colonne absente du rang vaut `None`.
struct Table {
    entete: Vec<String>,
    rangs: Vec<Vec<String>>,
}

impl Table {
    fn lire(texte: &str, delim: char) -> Table {
        let mut rangs = rangs_csv(texte, delim);
        let entete = if rangs.is_empty() { Vec::new() } else { rangs.remove(0) };
        Table { entete, rangs }
    }

    /// `l.get(nom)` : la valeur de la derniere colonne de ce nom, `None` si le rang est trop court ou la colonne absente.
    fn get<'a>(&self, rang: &'a [String], nom: &str) -> Option<&'a str> {
        let j = self.entete.iter().rposition(|t| t == nom)?;
        rang.get(j).map(String::as_str)
    }

    fn exige<'a>(&self, rang: &'a [String], nom: &str, fichier: &str) -> Resultat<&'a str> {
        self.get(rang, nom).ok_or_else(|| format!("{} : colonne {} absente d'un rang", fichier, repr(nom)))
    }
}

/// `texte.zfill(n)` de Python.
fn zfill(texte: &str, n: usize) -> String {
    let long = texte.chars().count();
    if long >= n {
        return texte.to_string();
    }
    let zeros = "0".repeat(n - long);
    match texte.chars().next() {
        Some(s @ ('+' | '-')) => format!("{}{}{}", s, zeros, &texte[1..]),
        _ => format!("{}{}", zeros, texte),
    }
}

/// Les organismes de la Table des Urssaf (windows-1252 : « Numéro », « Dénomination »), tries par code.
fn organismes(table: &Path) -> Resultat<Vec<Value>> {
    let nom = nom_de(table);
    let octets = std::fs::read(table).map_err(|e| format!("{} illisible ({})", table.display(), e))?;
    let t = Table::lire(&cp1252(&octets, &nom)?, ';');
    let mut liste: Vec<(String, Value)> = Vec::new();
    for l in &t.rangs {
        let cod = strip(t.get(l, "Codification").unwrap_or(""));
        if !cod.starts_with('U') {
            continue;
        }
        let code: String = cod.chars().skip(1).collect();
        // Les valeurs du CSV sont entourees d'apostrophes pour qu'un tableur ne mange pas les zeros de tete.
        let sans_apostrophes = |c: &str| strip(t.get(l, c).unwrap_or("")).trim_matches('\'').to_string();
        liste.push((
            code.clone(),
            json!({
                "code": code,
                "codification": cod,
                "nom": strip(t.get(l, "Dénomination").unwrap_or("")),
                "siret": sans_apostrophes("SIRET"),
                "emetteur": sans_apostrophes("Numéro National Emetteur"),
            }),
        ));
    }
    liste.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(liste.into_iter().map(|(_, o)| o).collect())
}

/// Ancienne region -> code Urssaf, depuis la liste des sites du reseau. Le filtre `codes` (la Table des Urssaf) tranche
/// le cas de la Normandie : la liste des sites y porte encore 237 et 257, la Table ne connait que 287.
fn par_region(sites: &Path, codes: &BTreeSet<String>) -> Resultat<Map<String, Value>> {
    let octets = std::fs::read(sites).map_err(|e| format!("{} illisible ({})", sites.display(), e))?;
    let texte = String::from_utf8(octets).map_err(|_| format!("{} n'est pas en UTF-8", sites.display()))?;
    let t = Table::lire(texte.strip_prefix('\u{feff}').unwrap_or(&texte), ';');
    let mut trouve: Vec<(String, BTreeSet<String>)> = Vec::new();
    for l in &t.rangs {
        let ur = strip(t.get(l, "code_organisme_ur_regionale").unwrap_or(""));
        let reg = zfill(strip(t.get(l, "code_ancienne_region").unwrap_or("")), 2);
        if !ur.is_empty() && !reg.is_empty() && codes.contains(ur) {
            match trouve.iter_mut().find(|(r, _)| *r == reg) {
                Some((_, v)) => {
                    v.insert(ur.to_string());
                }
                None => trouve.push((reg, BTreeSet::from([ur.to_string()]))),
            }
        }
    }
    let ambigus: Vec<String> = trouve
        .iter()
        .filter(|(_, v)| v.len() != 1)
        .map(|(r, v)| format!("{}: [{}]", repr(r), v.iter().map(|c| repr(c)).collect::<Vec<_>>().join(", ")))
        .collect();
    if !ambigus.is_empty() {
        return Err(format!(
            "Plusieurs codes Urssaf pour une meme ancienne region : {{{}}}.\n  La deduction ne peut pas trancher toute seule ; \
             relire la source avant d'aller plus loin.",
            ambigus.join(", ")
        ));
    }
    Ok(trouve.into_iter().map(|(r, v)| (r, Value::String(v.into_iter().next().unwrap_or_default()))).collect())
}

/// Departement -> code Urssaf, par l'ancienne region du COG 2015 (le premier `.txt` de l'archive, en latin-1, tabule).
fn par_departement(cog: &Path, regions: &Map<String, Value>, codes: &BTreeSet<String>) -> Resultat<Map<String, Value>> {
    let f = std::fs::File::open(cog).map_err(|e| format!("{} illisible ({})", cog.display(), e))?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| format!("{} : archive illisible ({})", cog.display(), e))?;
    let nom = (0..z.len())
        .filter_map(|i| z.by_index_raw(i).ok().map(|e| e.name().to_string()))
        .find(|n| n.ends_with(".txt"))
        .ok_or_else(|| format!("{} : aucun .txt dans l'archive", cog.display()))?;
    let mut octets = Vec::new();
    z.by_name(&nom)
        .and_then(|mut e| e.read_to_end(&mut octets).map_err(Into::into))
        .map_err(|e| format!("{} : {} illisible ({})", cog.display(), nom, e))?;
    let t = Table::lire(&latin1(&octets), '\t');
    let mut sortie: Vec<(String, String)> = Vec::new();
    let mut sans = Vec::new();
    for l in &t.rangs {
        let dep = strip(t.exige(l, "DEP", &nom)?).to_string();
        let code = if dep.starts_with("97") {
            // Outre-mer : le code de la CGSS EST celui du departement.
            codes.contains(&dep).then(|| dep.clone())
        } else {
            let region = zfill(strip(t.exige(l, "REGION", &nom)?), 2);
            regions.get(&region).and_then(Value::as_str).map(str::to_string)
        };
        match code {
            Some(c) if !c.is_empty() => match sortie.iter_mut().find(|(d, _)| *d == dep) {
                Some(x) => x.1 = c,
                None => sortie.push((dep, c)),
            },
            _ => sans.push(dep),
        }
    }
    if !sans.is_empty() {
        return Err(format!(
            "Departements sans code Urssaf : {}.\n  Une source a bouge ; ne pas combler a la main.",
            sans.join(", ")
        ));
    }
    // La Corse se saisit par son code postal, qui commence par 20 : 2A et 2B n'y apparaissent jamais. Les deux vont au meme
    // organisme.
    let de = |d: &str, s: &[(String, String)]| s.iter().find(|(x, _)| x == d).map(|(_, c)| c.clone());
    if let (Some(a), Some(b)) = (de("2A", &sortie), de("2B", &sortie)) {
        if a == b {
            match sortie.iter_mut().find(|(d, _)| d == "20") {
                Some(x) => x.1 = a,
                None => sortie.push(("20".into(), a)),
            }
        }
    }
    sortie.sort();
    Ok(sortie.into_iter().map(|(d, c)| (d, Value::String(c))).collect())
}

/// Le releve : les organismes, les anciennes regions, les departements.
pub fn relever(table: &Path, sites: &Path, cog: &Path, jour: &Jour) -> Resultat<Releve> {
    let organismes = organismes(table)?;
    let codes: BTreeSet<String> = organismes.iter().filter_map(|o| o["code"].as_str().map(str::to_string)).collect();
    let regions = par_region(sites, &codes)?;
    let departements = par_departement(cog, &regions, &codes)?;
    let bilan = vec![format!("{} organismes, {} departements", organismes.len(), departements.len())];
    let donnees = json!({
        "_quoi": "Les organismes de recouvrement et leur code a trois chiffres, tel que la DPAE l'attend en C_URSSAF \
                  (positions 133-136, annexe II.1 de la norme), et le departement dont chacun releve.",
        "_source": format!(
            "Trois sources relevees et datees -- `moteur sources` : urssaf-table-des-urssaf (les codes, {}), \
             urssaf-liste-des-sites (region -> code, {}), insee-cog-2015-departements (departement -> ancienne region, {}), \
             deposees dans atelier/reference/sources/. Extrait par `moteur extraire urssaf` le {} ; ne pas editer a la main.",
            nom_de(table), nom_de(sites), nom_de(cog), date_du_releve(jour)
        ),
        "_par_departement_est_une_suggestion": "Le rattachement d'une entreprise a une Urssaf est un fait ADMINISTRATIF, \
            pas une consequence de son adresse : versement en lieu unique, pole grandes entreprises, GUSO. Cette table dit \
            l'organisme du territoire, ce qui est le cas ordinaire ; l'ecran la propose, il ne l'impose pas, et la valeur \
            de la fiche gagne toujours.",
        "_defaut": "117 -- Urssaf Ile-de-France, la valeur du gabarit quand ni la fiche ni le departement ne disent rien.",
        "organismes": organismes,
        "par_ancienne_region": regions,
        "par_departement": departements,
    });
    Ok(Releve { donnees, indent: 2, saut_final: true, bilan })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zfill_comme_python() {
        assert_eq!(zfill("1", 2), "01");
        assert_eq!(zfill("", 2), "00");
        assert_eq!(zfill("-1", 3), "-01");
        assert_eq!(zfill("94", 2), "94");
    }

    #[test]
    fn un_csv_se_lit_comme_python() {
        let r = rangs_csv("a;\"b;c\";d\r\n\r\ne;f\"g\n", ';');
        assert_eq!(r, vec![vec!["a", "b;c", "d"], vec!["e", "f\"g"]]);
    }
}
