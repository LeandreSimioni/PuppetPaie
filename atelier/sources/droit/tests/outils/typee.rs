//! Les aides des tests du droit, sans JSON brut : un fichier (une donnee livree, une reference) se lit PAR LA PORTE DU
//! SOCLE, en texte, directement dans un type (`socle::forme::lire_texte`) ; un test declare le type de son cas et celui
//! de son resultat (`Serialize`, `Deserialize`, `PartialEq`, `Debug`, nombres exacts en `Decimal`) ; la reference est une
//! liste de lignes `(cas, attendu)` de ces types ; la comparaison se fait ENTRE TYPES, sans tolerance, et un ecart nomme
//! le cas et le champ. `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit la reference, par l'ecriture du socle
//! (`socle::ecriture`).

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Les donnees livrees (`logiciel/donnees`).
pub fn code_livre() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../logiciel/donnees")
}

/// `tests/donnees/<nom>`.
pub fn chemin_donnee(nom: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/donnees").join(nom)
}

/// Un texte JSON lu par la porte du socle, dans son type.
pub fn lu<T: DeserializeOwned>(texte: &str) -> T {
    socle::forme::lire_texte(texte, "Le texte du test ne se lit pas.").unwrap_or_else(|e| panic!("{}", socle::echec::phrase_de(&e)))
}

/// Le fichier `chemin`, lu par la porte du socle, dans son type.
pub fn lire_chemin<T: DeserializeOwned>(chemin: &Path) -> T {
    let texte = std::fs::read_to_string(chemin).unwrap_or_else(|e| panic!("{} illisible : {}", chemin.display(), e));
    socle::forme::lire_texte(&texte, &format!("{} ne se lit pas.", chemin.display()))
        .unwrap_or_else(|e| panic!("{} : {}", chemin.display(), socle::echec::phrase_de(&e)))
}

/// Le fichier `rel` sous `racine`.
pub fn lire<T: DeserializeOwned>(racine: &Path, rel: &str) -> T {
    lire_chemin(&racine.join(rel))
}

/// Une ligne de reference : un cas (l'entree) et ce que le droit en rend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ligne<C, R> {
    pub cas: C,
    pub attendu: R,
}

/// `PUPPETPAIE_REECRIRE_ORACLES=1` : les tests reecrivent leurs references au lieu de les comparer. Lue, jamais posee par un test.
pub fn reecrire_oracles() -> bool {
    std::env::var("PUPPETPAIE_REECRIRE_ORACLES").is_ok_and(|v| v == "1")
}

/// La reference `tests/donnees/<nom>` : ses lignes.
pub fn reference<C: DeserializeOwned, R: DeserializeOwned>(nom: &str) -> Vec<Ligne<C, R>> {
    lire_chemin(&chemin_donnee(nom))
}

/// Les cas d'une reference, sans leurs resultats : ce que le test rejoue.
pub fn cas_de<C: DeserializeOwned>(nom: &str) -> Vec<C> {
    #[derive(Deserialize)]
    struct SeulLeCas<C> {
        cas: C,
        #[serde(rename = "attendu")]
        _attendu: serde::de::IgnoredAny,
    }
    lire_chemin::<Vec<SeulLeCas<C>>>(&chemin_donnee(nom)).into_iter().map(|l| l.cas).collect()
}

/// Les lignes `obtenu` contre la reference `nom` : meme nombre, memes cas, memes resultats, comparees entre types. Un
/// ecart nomme le cas et le champ. Sous `PUPPETPAIE_REECRIRE_ORACLES=1`, la reference est ecrite a la place.
pub fn verifier<C, R>(nom: &str, obtenu: Vec<Ligne<C, R>>)
where
    C: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
    R: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let chemin = chemin_donnee(nom);
    if reecrire_oracles() {
        std::fs::create_dir_all(chemin.parent().unwrap()).unwrap();
        socle::ecriture::ecrire_json(&chemin, &obtenu, 2).unwrap_or_else(|e| panic!("{} : {}", nom, socle::echec::phrase_de(&e)));
        return;
    }
    let attendu: Vec<Ligne<C, R>> = lire_chemin(&chemin);
    let mut ecarts = Vec::new();
    if attendu.len() != obtenu.len() {
        ecarts.push(format!("{} lignes attendues, {} obtenues", attendu.len(), obtenu.len()));
    }
    for (a, o) in attendu.iter().zip(&obtenu) {
        if a != o {
            let cas = socle::ecriture::json_en_ligne(&o.cas);
            let champ = if a.cas != o.cas { format!("le cas lui-meme : {}", premier_ecart(&a.cas, &o.cas)) } else { premier_ecart(&a.attendu, &o.attendu) };
            ecarts.push(format!("cas {}\n  {}", cas, champ));
        }
    }
    assert!(ecarts.is_empty(), "{} ecarts avec {} :\n{}", ecarts.len(), nom, ecarts.iter().take(8).cloned().collect::<Vec<_>>().join("\n"));
}

/// Le premier champ qui differe de deux valeurs, par son chemin (`a.b.c`), avec ce que chacune y dit : les deux sont
/// ecrites par le socle, ligne a ligne, et lues sur leur indentation.
fn premier_ecart<T: Serialize>(attendu: &T, obtenu: &T) -> String {
    let (a, o) = (socle::ecriture::json_indente(attendu, 2), socle::ecriture::json_indente(obtenu, 2));
    let mut chemin: Vec<String> = Vec::new();
    let (la, lo): (Vec<&str>, Vec<&str>) = (a.lines().collect(), o.lines().collect());
    for i in 0..la.len().max(lo.len()) {
        let (x, y) = (la.get(i).copied().unwrap_or(""), lo.get(i).copied().unwrap_or(""));
        let t = x.trim_start();
        let niveau = (x.len() - t.len()) / 2;
        chemin.truncate(niveau);
        let cle = t.strip_prefix('"').and_then(|r| r.split_once("\": ")).map(|(k, _)| k.to_string()).unwrap_or_else(|| format!("[{}]", i));
        if x != y {
            let mut p = chemin.join(".");
            if !p.is_empty() {
                p.push('.');
            }
            p.push_str(&cle);
            return format!("champ {}\n    attendu : {}\n    obtenu  : {}", p, t.chars().take(300).collect::<String>(), y.trim_start().chars().take(300).collect::<String>());
        }
        chemin.push(cle);
    }
    "valeurs differentes, ecritures identiques".to_string()
}

/// Ce que rend une fonction du droit qui peut refuser : sa valeur, ou la phrase du refus.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Rendu<T> {
    #[serde(rename = "ok")]
    Ok(T),
    #[serde(rename = "err")]
    Err(String),
}

impl<T> From<Result<T, String>> for Rendu<T> {
    fn from(r: Result<T, String>) -> Rendu<T> {
        match r {
            Ok(v) => Rendu::Ok(v),
            Err(e) => Rendu::Err(e),
        }
    }
}

/// Rejoue les cas de la reference `nom` (`cas_de`) par `f` et les compare (`verifier`).
pub fn jouer<A, R>(nom: &str, cas: Vec<A>, f: impl Fn(&A) -> R)
where
    A: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
    R: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
{
    verifier(nom, cas.into_iter().map(|a| Ligne { attendu: f(&a), cas: a }).collect());
}
