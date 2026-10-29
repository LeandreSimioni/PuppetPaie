//! LA BANQUE DES CONSEILS (`logiciel/donnees/conseils.json`) : un contrat, pas une forme. Toute règle vient
//! d'un texte déposé (atelier/CLAUDE.md, « Les sources ») : chaque source nomme un fichier qui existe dans le dépôt, et la
//! citation s'y lit à l'identique (à la casse, aux blancs, aux apostrophes et aux guillemets près ; « […] » sépare des
//! fragments). Un chiffre daté ne s'écrit pas en dur : les gabarits {bareme:…}, {minima:…} et {annee} se résolvent sur les
//! données livrées. Chaque `quand` est « toujours » ou une condition que le moteur sait évaluer (`conseils::CONDITIONS`).

use crate::outils::code_livre;

use serde_json::Value;
use std::collections::HashMap;
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

/// Le texte d'un article de l'entrepôt LEGI (`articles.jsonl.gz`), toutes ses versions déposées.
fn article_legi(chemin: &Path, code: &str, numero: &str, cache: &mut HashMap<(String, String), String>) -> String {
    if cache.is_empty() {
        let f = std::fs::File::open(chemin).unwrap();
        for ligne in BufReader::new(flate2::read::GzDecoder::new(f)).lines() {
            let ligne = ligne.unwrap();
            let v: Value = serde_json::from_str(&ligne).unwrap();
            let cle = (v["cle"][0].as_str().unwrap_or("").to_string(), v["cle"][1].as_str().unwrap_or("").to_string());
            let xml = v["xml"].as_str().or(v["contenu"].as_str()).unwrap_or("").to_string();
            cache.entry(cle).or_default().push_str(&xml);
        }
    }
    cache.get(&(code.to_string(), numero.to_string())).cloned().unwrap_or_default()
}

#[test]
fn chaque_source_est_un_texte_depose_et_sa_citation_s_y_lit() {
    let b = banque();
    let mut legi = HashMap::new();
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
                let xml = article_legi(&chemin, code, numero, &mut legi);
                assert!(!xml.is_empty(), "{} : l'article {} {} n'est pas dans l'entrepôt déposé", id, code, numero);
                normaliser(&xml, true)
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

