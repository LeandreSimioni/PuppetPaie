//! `moteur extraire naf <structure.xlsx> <correspondances.xlsx> <sortie.json>` : la NAF 2025 et sa table de correspondance
//! depuis la NAF rev. 2 (INSEE), pour `logiciel/donnees/naf-base.json` (que lit `naf.rs`).
//!
//! Les deux classeurs sont ceux de la page INSEE « NAF 2025 » (https://www.insee.fr/fr/information/8181066), deposes dans
//! `atelier/reference/sources/insee-naf-2025/` (son `LISEZ-MOI.md`) :
//!   - la structure (`insee-naf-2025-structure-20241004.xlsx`), feuille « Sous-classes » : le code (`90.20Y`) et
//!     l'intitule de chaque sous-classe ;
//!   - la correspondance (`insee-naf-2025-correspondances-naf-rev2-vers-naf-2025-20260107.xlsx`), feuille « NAF rév. 2
//!     vers NAF 2025 » : pour chaque sous-classe NAF rev. 2, une ligne par sous-classe NAF 2025 ou passe une partie de son
//!     contenu, avec ce contenu et le type (« Unique », « Multiple »).
//!
//! Les codes s'ecrivent dans le JSON comme la fiche entreprise les porte : quatre chiffres et une lettre, sans le point
//! (`9020Y`) ; le code INSEE, avec son point, est garde a cote. Rien n'est interprete : le releve recopie. La date
//! d'entree en vigueur vient du decret n° 2025-736 du 31/07/2025, depose a cote
//! (`legifrance-decret-2025-736-naf-2025-20250731.txt`, art. 2 et 6). Portage de `atelier/outils/extraire_naf.py`.

use std::path::Path;

use regex::Regex;
use serde_json::{json, Map, Value};

use super::{date_du_releve, nom_de, propre, sha256, Releve};
use socle::date::Jour;
use socle::fichiers::repr;
use socle::lecture_xlsx;
use socle::num::Resultat;

fn sans_point(code: &str) -> String {
    code.replace('.', "")
}

/// Le releve : `naf_2025` (sous-classes) et `naf_rev2` (chaque sous-classe rev. 2 et ses destinations).
pub fn relever(structure: &Path, correspondances: &Path, jour: &Jour) -> Resultat<Releve> {
    let code = Regex::new(r"\A\d\d\.\d\d[A-Z]\z").expect("motif");
    let mut naf_2025 = Map::new();
    for r in lecture_xlsx::lignes(structure, "Sous-classes")? {
        let mut r: Vec<String> = r.iter().map(propre).collect();
        r.extend(["".to_string(), "".to_string()]);
        if code.is_match(&r[0]) {
            naf_2025.insert(sans_point(&r[0]), json!({"code": r[0], "intitule": r[1]}));
        }
    }
    let mut naf_rev2 = Map::new();
    for r in lecture_xlsx::lignes(correspondances, "NAF rév. 2 vers NAF 2025")? {
        let mut r: Vec<String> = r.iter().map(propre).collect();
        r.extend(std::iter::repeat_n(String::new(), 6));
        if !code.is_match(&r[0]) {
            continue;
        }
        if !code.is_match(&r[3]) || !naf_2025.contains_key(&sans_point(&r[3])) {
            return Err(format!("correspondance de {} vers un code absent de la structure : {}", r[0], repr(&r[3])));
        }
        let entree = naf_rev2
            .entry(sans_point(&r[0]))
            .or_insert_with(|| json!({"code": r[0], "intitule": r[1], "type": r[5], "vers": []}));
        if entree["type"].as_str() != Some(r[5].as_str()) {
            return Err(format!(
                "type de correspondance incoherent pour {} : {} et {}",
                r[0],
                repr(entree["type"].as_str().unwrap_or("")),
                repr(&r[5])
            ));
        }
        if let Some(vers) = entree["vers"].as_array_mut() {
            vers.push(json!({"code": sans_point(&r[3]), "code_insee": r[3], "intitule": r[4], "contenu": r[2]}));
        }
    }
    let mut communs: Vec<&String> = naf_2025.keys().filter(|k| naf_rev2.contains_key(*k)).collect();
    communs.sort();
    if !communs.is_empty() {
        let liste: Vec<String> = communs.iter().map(|c| repr(c)).collect();
        return Err(format!(
            "codes communs aux deux nomenclatures, le moteur ne saurait pas les dater : [{}]",
            liste.join(", ")
        ));
    }
    let (n_2025, n_rev2) = (naf_2025.len(), naf_rev2.len());
    let (nom_s, nom_c) = (nom_de(structure), nom_de(correspondances));
    let mut empreintes = Map::new();
    empreintes.insert(nom_s.clone(), Value::String(sha256(structure)?));
    empreintes.insert(nom_c.clone(), Value::String(sha256(correspondances)?));
    let donnees = json!({
        "_quoi": "La NAF 2025 (sous-classes) et la correspondance de chaque sous-classe NAF rev. 2 vers la NAF 2025. \
                  La fiche entreprise porte le code APE sans point (`9020Y`). Le moteur s'en sert pour reconnaitre un \
                  code saisi et pour refuser, dans un fichier DSN P27V01, un code NAF rev. 2 qui n'a plus cours, en \
                  disant vers quels codes NAF 2025 il passe : il ne choisit jamais entre eux, c'est l'INSEE qui \
                  attribue le code (`declarations/src/naf.rs`).",
        "_source": format!(
            "INSEE, page « NAF 2025 » (https://www.insee.fr/fr/information/8181066) : {} (structure, feuille \
             « Sous-classes ») et {} (feuille « NAF rév. 2 vers NAF 2025 »), deposes dans \
             atelier/reference/sources/insee-naf-2025/. Releve automatiquement par `moteur extraire naf` le {}.",
            nom_s, nom_c, date_du_releve(jour)
        ),
        "_sha256": empreintes,
        "en_vigueur_naf_2025": "2027-01-01",
        "_source_en_vigueur": "Decret n° 2025-736 du 31/07/2025 (atelier/reference/sources/insee-naf-2025/\
                               legifrance-decret-2025-736-naf-2025-20250731.txt) : art. 2, la NAF 2025 « entre en \
                               vigueur le 1er janvier 2027 » ; art. 6, le decret de 2007 (NAF rev. 2) est abroge a la \
                               meme date pour la nomenclature d'activites.",
        "naf_2025": naf_2025,
        "naf_rev2": naf_rev2,
    });
    Ok(Releve {
        donnees,
        indent: 1,
        saut_final: false,
        bilan: vec![format!("{} sous-classes NAF 2025, {} sous-classes NAF rev. 2", n_2025, n_rev2)],
    })
}
