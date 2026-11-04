//! LES LIBELLES COURTS : ce qu'un format de logiciel comptable a largeur bornee ecrit a la place d'un libelle trop long
//! pour sa zone. Une zone ne se coupe jamais (`etats_compta`) : une valeur qui ne tient pas fait refuser l'export, tant
//! qu'une valeur courte n'a pas ete validee pour elle. La valeur courte se valide par la compagnie (la route qui les
//! enregistre), une par libelle, par compte et pour le nom de la compagnie, pour chaque format ; le moteur en propose
//! une (`proposer`), qui n'est qu'une proposition : rien ne s'ecrit dans un export sans etre dans
//! `config/libelles-courts.json`.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use socle::echec::{erreur, R};

/// Les libelles courts valides de la compagnie, sous sa racine.
pub const LIBELLES_COURTS: &str = "config/libelles-courts.json";

/// Les formats dont une zone de texte a une largeur que la notice borne (`etats_compta`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FormatBorne {
    Ximport,
    Oxygene,
    Sage,
    Quadra,
    Ebp,
}

impl FormatBorne {
    /// Le format d'un nom de format (`ximport`…), s'il borne ses zones de texte.
    pub fn de(format: &str) -> Option<FormatBorne> {
        serde_json::from_value(Value::String(format.to_string())).ok()
    }

    pub fn nom(self) -> &'static str {
        match self {
            FormatBorne::Ximport => "XIMPORT",
            FormatBorne::Oxygene => "Oxygène",
            FormatBorne::Sage => "Sage",
            FormatBorne::Quadra => "Quadra",
            FormatBorne::Ebp => "EBP",
        }
    }
}

/// Ce qu'une valeur courte remplace : le libelle d'une ecriture (sans son mois, qui s'y ajoute toujours), l'intitule
/// d'un compte, ou le nom de la compagnie (l'en-tete d'un fichier Sage).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Quoi {
    Libelle,
    Compte,
    Compagnie,
}

/// Le fichier : par format, les libelles (le libelle long -> le court), les comptes (le numero -> l'intitule court) et
/// le nom de la compagnie (le nom long -> le court).
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LibellesCourts {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub quoi: String,
    #[serde(default)]
    pub formats: BTreeMap<FormatBorne, Courts>,
}

/// Les valeurs courtes validees d'un format.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Courts {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub libelles: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub comptes: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub compagnie: BTreeMap<String, String>,
}

impl Courts {
    fn table(&mut self, quoi: Quoi) -> &mut BTreeMap<String, String> {
        match quoi {
            Quoi::Libelle => &mut self.libelles,
            Quoi::Compte => &mut self.comptes,
            Quoi::Compagnie => &mut self.compagnie,
        }
    }

    /// La valeur courte validee de `cle`, s'il y en a une.
    pub fn de(&self, quoi: Quoi, cle: &str) -> Option<&String> {
        match quoi {
            Quoi::Libelle => self.libelles.get(cle),
            Quoi::Compte => self.comptes.get(cle),
            Quoi::Compagnie => self.compagnie.get(cle),
        }
    }

    pub fn poser(&mut self, quoi: Quoi, cle: String, court: String) {
        self.table(quoi).insert(cle, court);
    }
}

/// Les libelles courts de la compagnie ; aucun si le fichier n'existe pas. Un fichier qui ne se lit pas refuse.
pub fn lire(racine: &Path) -> R<LibellesCourts> {
    let p = racine.join(LIBELLES_COURTS);
    if !p.exists() {
        return Ok(LibellesCourts::default());
    }
    let v = socle::fichiers::lire_json_echec(&p)?;
    socle::forme::lire(&v, "Les libellés courts de la compagnie ne se lisent pas : les enregistrer à nouveau depuis les Exports.").map_err(|m| erreur("ErreurOperation", m))
}

/// Ecrit les libelles courts de la compagnie.
pub fn ecrire(racine: &Path, l: &LibellesCourts) -> R<()> {
    let v = serde_json::to_value(l).map_err(|e| erreur("ErreurOperation", e.to_string()))?;
    socle::ecriture::ecrire_json(&racine.join(LIBELLES_COURTS), &v, 2).map_err(|m| erreur("OSError", m))
}

/// Une valeur trop longue pour sa zone, telle qu'un export l'a rencontree : ce qu'elle est, sa cle, son texte long, la
/// place qu'elle a (le mois d'un libelle compte a part), et la valeur courte validee qui la remplace, si elle tient.
#[derive(Debug, Clone)]
pub struct Depasse {
    pub quoi: Quoi,
    pub cle: String,
    pub long: String,
    pub largeur: usize,
    pub court: Option<String>,
}

/// Les mots qu'une proposition laisse tomber d'abord.
const MOTS_VIDES: [&str; 22] =
    ["le", "la", "les", "l'", "de", "des", "du", "d'", "et", "à", "a", "au", "aux", "en", "un", "une", "pour", "par", "sur", "sans", "avec", "ou"];

/// LA PROPOSITION D'UNE VALEUR COURTE, de `largeur` caracteres au plus, pour `long`. Une regle simple, dans cet ordre,
/// chaque pas seulement si le precedent ne suffit pas : ce qui est entre parentheses et la ponctuation s'en vont ; puis
/// les mots vides (articles, prepositions, « et ») ; puis le mot le plus long de plus de cinq lettres (le dernier, a
/// egalite) se reduit a ses quatre premieres et un point, et ainsi de suite ; puis les derniers mots tombent ; un mot seul
/// qui ne tient toujours pas garde ses premieres lettres. Ce n'est qu'une proposition : la personne la garde ou la change.
pub fn proposer(long: &str, largeur: usize) -> String {
    let net = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    let tient = |m: &[String]| m.join(" ").chars().count() <= largeur;
    let entier = net(long);
    if entier.chars().count() <= largeur {
        return entier;
    }
    // Ce qui est entre parentheses, et la ponctuation.
    let mut sans = String::new();
    let mut profondeur = 0usize;
    for c in entier.chars() {
        match c {
            '(' => profondeur += 1,
            ')' => profondeur = profondeur.saturating_sub(1),
            ',' | ';' | ':' | '.' if profondeur == 0 => sans.push(' '),
            _ if profondeur == 0 => sans.push(c),
            _ => {}
        }
    }
    let mut mots: Vec<String> = sans.split_whitespace().map(str::to_string).collect();
    if mots.is_empty() {
        mots = entier.split_whitespace().map(str::to_string).collect();
    }
    // Les mots vides, et l'elision d'un mot (l'Hotel -> Hotel).
    if !tient(&mots) {
        let pleins: Vec<String> = mots
            .iter()
            .filter(|m| !MOTS_VIDES.contains(&m.to_lowercase().as_str()))
            .map(|m| {
                let bas = m.to_lowercase();
                match ["l'", "d'", "l’", "d’"].iter().find(|e| bas.starts_with(*e)) {
                    Some(e) if m.chars().count() > e.chars().count() => m.chars().skip(e.chars().count()).collect(),
                    _ => m.clone(),
                }
            })
            .collect();
        if !pleins.is_empty() {
            mots = pleins;
        }
    }
    // Le mot le plus long, reduit a quatre lettres et un point, tant que cela ne suffit pas.
    while !tient(&mots) {
        let Some(i) = mots
            .iter()
            .enumerate()
            .filter(|(_, m)| m.chars().count() > 5 && !m.ends_with('.'))
            .max_by_key(|(i, m)| (m.chars().count(), *i))
            .map(|(i, _)| i)
        else {
            break;
        };
        mots[i] = format!("{}.", mots[i].chars().take(4).collect::<String>());
    }
    // Les derniers mots tombent.
    while !tient(&mots) && mots.len() > 1 {
        mots.pop();
    }
    let t = mots.join(" ");
    if t.chars().count() <= largeur {
        t
    } else {
        t.chars().take(largeur).collect()
    }
}
