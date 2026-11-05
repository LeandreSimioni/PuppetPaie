//! LE VERIFICATEUR DES SOURCES : toute valeur qu'un fichier livre fonde sur un texte le cite (`droit::source`), et la citation
//! s'y lit (atelier/AGENTS.md, « Les sources »). Pour chaque `source` des baremes (`logiciel/donnees/baremes/`), des
//! regles des permanents (`permanents-base.json`), des natures de contrat (`contrats-base.json`), de la prevoyance de branche (`prevoyance-conventionnelle-base.json`), des organismes (`organismes-base.json`), de la CASC-SVP (`casc-svp-base.json`), des clauses conventionnelles (`clauses-conventionnelles-base.json`) et de la banque des conseils (`conseils.json`) : le fichier est depose dans le depot, la citation s'y lit a l'identique (a la
//! casse, aux blancs, aux apostrophes et aux guillemets pres ; « […] » separe des fragments ; l'article d'un entrepot de la
//! DILA, toutes ses versions deposees) ; les textes d'un fondement `Texte` ont un seul emetteur, une convergence en a au
//! moins deux. Un conseil nomme le titre lisible de chacun de ses documents.

use crate::outils::code_livre;

use droit::source::{Fondement, Source};
use serde_json::Value;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

fn depot() -> PathBuf {
    code_livre().join("../..")
}

fn lu(nom: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(code_livre().join(nom)).unwrap()).unwrap()
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
        .replace("[...]", "\u{1}")
        .replace("...", "\u{1}")
        .replace('…', "\u{1}")
        .split('\u{1}')
        .map(|f| normaliser(f, false).trim().to_string())
        .filter(|f| f.chars().count() >= 8)
        .collect()
}

type Cle = (String, String);

/// Le texte des articles `voulus` d'un entrepot de la DILA (`articles.jsonl.gz`), toutes leurs versions deposees. Une
/// ligne de LEGI s'ecrit cle en tete et les lignes se rangent par cle (`moteur/src/atelier/releve/entrepot_dila.rs`) : seule
/// sa cle se lit, et la lecture s'arrete passe la plus grande voulue. Une ligne de KALI se lit entiere.
fn articles(chemin: &Path, voulus: &HashSet<Cle>) -> HashMap<Cle, String> {
    let mut lus: HashMap<Cle, String> = HashMap::new();
    let Some(derniere) = voulus.iter().max() else { return lus };
    let f = std::fs::File::open(chemin).unwrap();
    let mut lecteur = BufReader::with_capacity(1 << 16, flate2::read::MultiGzDecoder::new(f));
    let mut ligne = String::new();
    while lecteur.read_line(&mut ligne).unwrap() > 0 {
        let cle: Cle = match ligne.strip_prefix("{\"cle\": ") {
            Some(reste) => {
                let parts: Vec<String> = serde_json::Deserializer::from_str(reste).into_iter().next().unwrap().unwrap();
                let cle = (parts.first().cloned().unwrap_or_default(), parts.get(1).cloned().unwrap_or_default());
                if &cle > derniere {
                    break;
                }
                cle
            }
            None => {
                let v: Value = serde_json::from_str(&ligne).unwrap();
                (v["cle"][0].as_str().unwrap_or_default().to_string(), v["cle"][1].as_str().unwrap_or_default().to_string())
            }
        };
        if voulus.contains(&cle) {
            let v: Value = serde_json::from_str(&ligne).unwrap();
            lus.entry(cle).or_default().push_str(v["xml"].as_str().or(v["contenu"].as_str()).unwrap_or(""));
        }
        ligne.clear();
    }
    lus
}

/// Les fondements d'une valeur JSON : chaque `source` qui s'y trouve, avec son chemin.
fn fondements(v: &Value, chemin: &str, out: &mut Vec<(String, Fondement)>) {
    match v {
        Value::Object(o) => {
            for (k, x) in o {
                let ici = format!("{}/{}", chemin, k);
                if k == "source" && x.is_object() {
                    out.push((ici, socle::forme::lire(x, "source illisible").unwrap_or_else(|e| panic!("{} : {}", chemin, e))));
                } else {
                    fondements(x, &ici, out);
                }
            }
        }
        Value::Array(a) => a.iter().enumerate().for_each(|(i, x)| fondements(x, &format!("{}/{}", chemin, i), out)),
        _ => {}
    }
}

/// Ce qui est faux de ces fondements : un fichier absent, une citation introuvable, un emetteur de trop ou de moins.
fn verifier(tous: &[(String, Fondement)]) -> Vec<String> {
    let mut voulus: HashMap<PathBuf, HashSet<Cle>> = HashMap::new();
    let sources: Vec<(&String, &Source)> = tous.iter().flat_map(|(ou, f)| f.sources().into_iter().map(move |s| (ou, s))).collect();
    for (_, s) in &sources {
        if let (Some(code), Some(article)) = (s.fichier.code(), &s.article) {
            voulus.entry(depot().join(s.fichier.fichier())).or_default().insert((code.to_string(), article.clone()));
        }
    }
    let mut entrepots: HashMap<PathBuf, HashMap<Cle, String>> = HashMap::new();
    let mut textes: HashMap<PathBuf, String> = HashMap::new();
    let mut fautes = Vec::new();
    for (ou, f) in tous {
        let emetteurs: BTreeSet<_> = f.sources().iter().map(|s| s.emetteur).collect();
        match f {
            Fondement::Texte(_) if emetteurs.len() != 1 => fautes.push(format!("{} : des textes d'emetteurs distincts font une convergence, avec son raisonnement", ou)),
            Fondement::Convergence { .. } if emetteurs.len() < 2 => fautes.push(format!("{} : une convergence a au moins deux emetteurs distincts", ou)),
            _ => {}
        }
    }
    for (ou, s) in sources {
        let chemin = depot().join(s.fichier.fichier());
        if !chemin.is_file() {
            fautes.push(format!("{} : le fichier cite n'est pas depose : {}", ou, s.fichier.fichier()));
            continue;
        }
        let texte = match s.fichier.code() {
            Some(code) => {
                let article = s.article.clone().unwrap_or_default();
                let lus = entrepots.entry(chemin.clone()).or_insert_with(|| articles(&chemin, &voulus[&chemin]));
                let xml = lus.get(&(code.to_string(), article.clone())).map(String::as_str).unwrap_or("");
                if xml.is_empty() {
                    fautes.push(format!("{} : l'article {} {} n'est pas dans l'entrepot depose", ou, code, article));
                    continue;
                }
                normaliser(xml, true)
            }
            // Une page HTML se lit sans ses balises (`1<sup>er</sup>`) ; un XML, avec (une fiche de parametrage cite ses
            // `<taux>`) ou sans (un article LEGI cite ses alineas d'un `<p>` a l'autre).
            None => {
                let nom = s.fichier.fichier();
                textes
                    .entry(chemin.clone())
                    .or_insert_with(|| {
                        let brut = String::from_utf8_lossy(&std::fs::read(&chemin).unwrap()).into_owned();
                        match (nom.ends_with(".html"), nom.ends_with(".xml")) {
                            (true, _) => normaliser(&brut, true),
                            (_, true) => format!("{}\u{0}{}", normaliser(&brut, false), normaliser(&brut, true)),
                            _ => normaliser(&brut, false),
                        }
                    })
                    .clone()
            }
        };
        let fr = fragments(&s.citation);
        if fr.is_empty() {
            fautes.push(format!("{} : une citation trop courte ne prouve rien : {}", ou, s.citation));
        }
        for f in fr {
            if !texte.contains(&f) {
                fautes.push(format!("{} ({}) : « {} »", ou, s.fichier.texte(), f));
            }
        }
    }
    fautes
}

#[test]
fn chaque_source_est_un_texte_depose_et_sa_citation_s_y_lit() {
    let mut tous = Vec::new();
    for nom in ["baremes/2025.json", "baremes/2026.json", "permanents-base.json", "contrats-base.json", "prevoyance-conventionnelle-base.json", "organismes-base.json", "casc-svp-base.json", "clauses-conventionnelles-base.json"] {
        fondements(&lu(nom), nom, &mut tous);
    }
    let banque = lu("conseils.json");
    for c in banque["conseils"].as_array().unwrap() {
        let id = c["id"].as_str().unwrap();
        let sources: Vec<Source> = socle::forme::lire(&c["source"], "").unwrap_or_else(|e| panic!("{} : {}", id, e));
        for s in &sources {
            assert!(banque["documents"].get(s.fichier.texte()).is_some(), "{} : le document {} n'a pas de titre lisible", id, s.fichier.texte());
        }
        tous.extend(sources.into_iter().map(|s| (format!("conseils.json/{}", id), Fondement::Texte(vec![s]))));
    }
    assert!(tous.len() > 300, "{}", tous.len());
    let fautes = verifier(&tous);
    assert!(fautes.is_empty(), "{} source(s) fausse(s) :\n{}", fautes.len(), fautes.join("\n"));
}
