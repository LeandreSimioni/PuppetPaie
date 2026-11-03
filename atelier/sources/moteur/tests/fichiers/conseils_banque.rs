//! LA BANQUE DES CONSEILS (`logiciel/donnees/conseils.json`) : un contrat, pas une forme. Toute règle vient
//! d'un texte déposé (atelier/AGENTS.md, « Les sources ») : chaque source nomme un fichier qui existe dans le dépôt, et la
//! citation s'y lit à l'identique (à la casse, aux blancs, aux apostrophes et aux guillemets près ; « […] » sépare des
//! fragments). Un chiffre daté ne s'écrit pas en dur : les gabarits {bareme:…}, {minima:…} et {annee} se résolvent sur les
//! données livrées.

use crate::outils::code_livre;

use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

fn depot() -> PathBuf {
    code_livre().join("../..")
}

fn banque() -> Value {
    serde_json::from_str(&std::fs::read_to_string(code_livre().join("conseils.json")).unwrap()).unwrap()
}

fn conseils(b: &Value) -> Vec<Value> {
    b["conseils"].as_array().unwrap().clone()
}

fn sources(c: &Value) -> Vec<Value> {
    match &c["source"] {
        Value::Array(a) => a.clone(),
        o @ Value::Object(_) => vec![o.clone()],
        autre => panic!("{} : source illisible ({})", c["id"], autre),
    }
}

/// Le texte comparé : sans balises (seulement pour l'XML de LEGI), sans les marques de citation markdown en tête de
/// ligne, minuscules, apostrophes et guillemets d'une seule sorte, blancs réduits.
fn normaliser(t: &str, xml: bool) -> String {
    let mut s = String::with_capacity(t.len());
    for ligne in t.lines() {
        let l = if xml { ligne } else { ligne.trim_start().trim_start_matches('>') };
        s.push_str(l);
        s.push(' ');
    }
    if xml {
        let mut sans = String::with_capacity(s.len());
        let mut dedans = false;
        for c in s.chars() {
            match c {
                '<' => dedans = true,
                '>' if dedans => {
                    dedans = false;
                    sans.push(' ');
                }
                _ if !dedans => sans.push(c),
                _ => {}
            }
        }
        s = sans;
    }
    let s = s.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'");
    let mut out = String::with_capacity(s.len());
    let mut blanc = false;
    for c in s.chars().flat_map(char::to_lowercase) {
        let c = match c {
            '\u{2019}' => '\'',
            '«' | '»' | '“' | '”' => '"',
            '\u{a0}' | '\u{202f}' | '|' | '•' | '\t' | '\r' | '\n' => ' ',
            '\u{ad}' | '\u{200b}' | '*' | '\\' => continue,
            autre => autre,
        };
        if c == ' ' {
            if !blanc {
                out.push(' ');
            }
            blanc = true;
        } else {
            out.push(c);
            blanc = false;
        }
    }
    out
}

/// Les fragments d'une citation : « […] », « ... » et « … » les séparent ; les très courts ne prouvent rien et se taisent.
fn fragments(citation: &str) -> Vec<String> {
    citation
        .replace("[…]", "\u{1}")
        .replace("...", "\u{1}")
        .replace('…', "\u{1}")
        .split('\u{1}')
        .map(|f| normaliser(f, false).trim().to_string())
        .filter(|f| f.chars().count() >= 8)
        .collect()
}

type Cle = (String, String);

/// Le texte des articles `voulus` de l'entrepôt LEGI (`articles.jsonl.gz`), toutes leurs versions déposées. L'entrepôt
/// écrit chaque ligne clés triées, `cle` en tête, et range les lignes par clé (`moteur/src/atelier/releve/entrepot_dila.rs`,
/// « LES FICHIERS ») : seule la clé de chaque ligne se lit, l'article entier seulement s'il est voulu, et la lecture
/// s'arrête passé la plus grande clé voulue.
fn articles_legi(chemin: &Path, voulus: &HashSet<Cle>) -> HashMap<Cle, String> {
    let mut lus: HashMap<Cle, String> = HashMap::new();
    let Some(derniere) = voulus.iter().max() else { return lus };
    let f = std::fs::File::open(chemin).unwrap();
    let mut lecteur = BufReader::with_capacity(1 << 16, flate2::read::MultiGzDecoder::new(f));
    let mut ligne = String::new();
    while lecteur.read_line(&mut ligne).unwrap() > 0 {
        let reste = ligne.strip_prefix("{\"cle\": ").unwrap_or_else(|| panic!("une ligne de l'entrepôt ne commence pas par sa clé"));
        let parts: Vec<String> = serde_json::Deserializer::from_str(reste).into_iter().next().unwrap().unwrap();
        let cle = (parts.first().cloned().unwrap_or_default(), parts.get(1).cloned().unwrap_or_default());
        if &cle > derniere {
            break;
        }
        if voulus.contains(&cle) {
            let v: Value = serde_json::from_str(&ligne).unwrap();
            lus.entry(cle).or_default().push_str(v["xml"].as_str().or(v["contenu"].as_str()).unwrap_or(""));
        }
        ligne.clear();
    }
    lus
}

#[test]
fn chaque_source_est_un_texte_depose_et_sa_citation_s_y_lit() {
    let b = banque();
    let mut legi: HashMap<PathBuf, HashMap<Cle, String>> = HashMap::new();
    let mut voulus: HashMap<PathBuf, HashSet<Cle>> = HashMap::new();
    for c in conseils(&b) {
        for s in sources(&c) {
            if let (Some(fichier), Some(code), Some(numero)) = (s["fichier"].as_str(), s["code"].as_str(), s["article"].as_str()) {
                if fichier.ends_with(".jsonl.gz") {
                    voulus.entry(depot().join(fichier)).or_default().insert((code.to_string(), numero.to_string()));
                }
            }
        }
    }
    let mut textes: HashMap<PathBuf, String> = HashMap::new();
    let mut fautes = Vec::new();
    for c in conseils(&b) {
        let id = c["id"].as_str().unwrap().to_string();
        for s in sources(&c) {
            let fichier = s["fichier"].as_str().unwrap_or_else(|| panic!("{} : source sans fichier", id));
            let chemin = depot().join(fichier);
            assert!(chemin.is_file(), "{} : le fichier cité n'est pas déposé : {}", id, fichier);
            let cle_titre = match s.get("code") {
                Some(code) => format!("{}#{}", fichier, code.as_str().unwrap()),
                None => fichier.to_string(),
            };
            assert!(b["documents"].get(&cle_titre).is_some(), "{} : le document {} n'a pas de titre lisible", id, cle_titre);
            let texte = if fichier.ends_with(".jsonl.gz") {
                let code = s["code"].as_str().unwrap_or_else(|| panic!("{} : une source LEGI nomme son code", id));
                let numero = s["article"].as_str().unwrap();
                let lus = legi.entry(chemin.clone()).or_insert_with(|| articles_legi(&chemin, &voulus[&chemin]));
                let xml = lus.get(&(code.to_string(), numero.to_string())).map(String::as_str).unwrap_or("");
                assert!(!xml.is_empty(), "{} : l'article {} {} n'est pas dans l'entrepôt déposé", id, code, numero);
                normaliser(xml, true)
            } else {
                textes.entry(chemin.clone()).or_insert_with(|| normaliser(&String::from_utf8_lossy(&std::fs::read(&chemin).unwrap()), false)).clone()
            };
            let citation = s["citation"].as_str().unwrap_or_else(|| panic!("{} : source sans citation", id));
            let fr = fragments(citation);
            assert!(!fr.is_empty(), "{} : une citation trop courte ne prouve rien : {}", id, citation);
            for f in fr {
                if !texte.contains(&f) {
                    fautes.push(format!("{} ({}) : « {} »", id, fichier, f));
                }
            }
        }
    }
    assert!(fautes.is_empty(), "citations introuvables dans le texte déposé :\n{}", fautes.join("\n"));
}

