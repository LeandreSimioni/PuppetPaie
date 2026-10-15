//! Le code APE (NAF) de la fiche entreprise, en NAF rev. 2 ou en NAF 2025 (lot R5b, 11/10/2026).
//!
//! La NAF 2025 « entre en vigueur le 1er janvier 2027 » et remplace la NAF rev. 2 a la meme date (decret n° 2025-736 du
//! 31/07/2025, art. 2 et 6 ; `atelier/reference/sources/insee-naf-2025/legifrance-decret-2025-736-naf-2025-20250731.txt`).
//! Les deux nomenclatures n'ont aucun code en commun ; la table livree (`logiciel/donnees/naf-base.json`, relevee par
//! `atelier/outils/extraire_naf.py` sur les fichiers de l'INSEE) dit, pour chaque code rev. 2, vers quels codes 2025 son
//! contenu passe. Elle ne choisit pas : `90.01Z` (arts du spectacle vivant) passe en `90.20Y` (activites de spectacle)
//! ou en `90.39G` (production et organisation d'evenements culturels) selon l'activite, et c'est l'INSEE qui attribue le
//! code (decret, art. 5 ; notification, avis de situation Sirene).
//!
//! Deux usages :
//! - une SAISIE (`faute_de_saisie`) : un code qu'aucune des deux nomenclatures ne connait refuse ; les deux sont admises,
//!   la fiche ne porte qu'un code et le moteur ne decide pas du jour ou la compagnie le change.
//! - un fichier DSN en norme P27V01 (`verifier_pour`) : « Il convient neanmoins de s'assurer que le code NAF soit modifie
//!   en adequation avec cette nouvelle nomenclature (NAF 2025) », et « a compter de la mise en production DSN de janvier
//!   2027, il est prevu que la nomenclature NAF 2025 s'applique quelle que soit la version de norme (P27 et P26) »
//!   (actualite net-entreprises du 08/10/2026, `atelier/reference/sources/net-entreprises-actualites/
//!   net-entreprises-actualite-naf-2025-en-dsn-janvier-2027-20261008.txt`). Un fichier P27V01 n'existe qu'apres cette
//!   mise en production : la fiche entreprise (et chaque etablissement qui porte son code) doit alors etre en NAF 2025.
//!   La regle se lit dans la version (`regles.code_naf_en_naf_2025` de `norme-dsn-base.json`). Un fichier P26V01 ne se
//!   controle pas : avant la mise en production il se depose en NAF rev. 2, apres en NAF 2025, et le jour de la mise en
//!   production n'est pas publie (D1).

use crate::fichiers::{dossier_du_code, json_en_cache};
use crate::norme::Norme;
use crate::num::Resultat;
use serde_json::Value;
use std::sync::Arc;

const BASE: &str = "naf-base.json";

/// La regle de `norme-dsn-base.json` qui demande la NAF 2025.
pub const REGLE: &str = "code_naf_en_naf_2025";

fn base() -> Resultat<Arc<Value>> {
    json_en_cache(&dossier_du_code().join(BASE))
}

/// Le code tel que la fiche le porte : sans espace, en majuscules.
fn net(code: &str) -> String {
    code.trim().to_uppercase()
}

/// Le code est-il une sous-classe de la NAF 2025 ?
pub fn en_naf_2025(code: &str) -> Resultat<bool> {
    Ok(base()?["naf_2025"].get(net(code)).is_some())
}

/// Le code est-il une sous-classe de la NAF rev. 2 ?
pub fn en_naf_rev2(code: &str) -> Resultat<bool> {
    Ok(base()?["naf_rev2"].get(net(code)).is_some())
}

/// Ce qu'on reproche a un code APE saisi, ou `None`.
pub fn faute_de_saisie(code: &str) -> Resultat<Option<String>> {
    let c = net(code);
    if en_naf_2025(&c)? || en_naf_rev2(&c)? {
        return Ok(None);
    }
    let sans_point = c.replace('.', "");
    if sans_point != c && (en_naf_2025(&sans_point)? || en_naf_rev2(&sans_point)?) {
        return Ok(Some(format!("le code APE s'ecrit sans le point : {}", sans_point)));
    }
    Ok(Some(format!(
        "« {} » n'est un code ni de la NAF rev. 2 ni de la NAF 2025 (INSEE, {}) : quatre chiffres et une lettre, sans \
point, tels que l'avis de situation Sirene les donne (9001Z en NAF rev. 2 ; 9020Y ou 9039G en NAF 2025, a partir du \
1er janvier 2027)",
        code.trim(),
        BASE
    )))
}

/// La phrase d'un code NAF rev. 2 dans un fichier qui veut la NAF 2025 : vers quels codes il passe, et quoi faire.
fn refus_rev2(ou: &str, code: &str, version: &str) -> Resultat<String> {
    let b = base()?;
    let e = &b["naf_rev2"][code];
    let vers: Vec<String> = e["vers"]
        .as_array()
        .map(|v| {
            v.iter()
                .map(|x| {
                    format!(
                        "{} ({} : {})",
                        x["code_insee"].as_str().unwrap_or(""),
                        x["intitule"].as_str().unwrap_or(""),
                        x["contenu"].as_str().unwrap_or("").trim_start_matches("- ")
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let devient = match vers.len() {
        0 => "n'a pas de correspondance dans la table de l'INSEE".to_string(),
        1 => format!("devient {}", vers[0]),
        _ => format!("devient, selon l'activite, {}", vers.join(" ou ")),
    };
    Ok(format!(
        "Code APE (NAF) {} : « {} » ({}) est un code de la NAF rev. 2, qui n'a plus cours depuis le 1er janvier 2027 \
(decret n° 2025-736, art. 2 et 6). Un fichier DSN en norme {} se depose apres la mise en production de janvier 2027, \
quand « la nomenclature NAF 2025 s'applique quelle que soit la version de norme » (net-entreprises, 08/10/2026). En NAF \
2025, {} {}. C'est l'INSEE qui attribue le code (notification, avis de situation Sirene) : le logiciel ne le choisit \
pas. Saisir le code notifie dans Configuration › Entreprise, « Code APE (NAF) », sans le point. Rien n'est ecrit.",
        ou,
        code,
        e["intitule"].as_str().unwrap_or(""),
        version,
        e["code"].as_str().unwrap_or(code),
        devient
    ))
}

/// Les codes APE que porte la fiche entreprise : le sien, et celui de chaque etablissement qui en a un.
fn codes_de(ent: &Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Some(c) = ent.get("naf").and_then(Value::as_str).filter(|c| !c.trim().is_empty()) {
        out.push(("de la fiche entreprise".to_string(), net(c)));
    }
    for e in crate::bareme::secondaires(ent) {
        if let Some(c) = e.get("naf").and_then(Value::as_str).filter(|c| !c.trim().is_empty()) {
            let nic = e.get("nic").map(|v| v.as_str().map(String::from).unwrap_or_else(|| v.to_string())).unwrap_or_default();
            out.push((format!("de l'etablissement {}", nic), net(c)));
        }
    }
    out
}

/// Un fichier DSN de la version `norme` peut-il partir avec les codes APE de cette fiche entreprise ? Seule une version
/// qui porte la regle `code_naf_en_naf_2025` les controle : un code NAF rev. 2 refuse avec la phrase qui dit vers quels
/// codes il passe ; un code que la NAF 2025 ne connait pas refuse.
pub fn verifier_pour(norme: &Norme, ent: &Value) -> Resultat<()> {
    if norme.regle(REGLE)?.is_none() {
        return Ok(());
    }
    for (ou, code) in codes_de(ent) {
        if en_naf_2025(&code)? {
            continue;
        }
        if en_naf_rev2(&code)? {
            return Err(refus_rev2(&ou, &code, &norme.version)?);
        }
        return Err(format!(
            "Code APE (NAF) {} : « {} » n'est pas un code de la NAF 2025 (INSEE, logiciel/donnees/{}), que tout fichier DSN \
en norme {} demande (net-entreprises, 08/10/2026). Saisir dans Configuration › Entreprise, « Code APE (NAF) », le code que \
l'INSEE a notifie, sans le point. Rien n'est ecrit.",
            ou, code, BASE, norme.version
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La table livree (`logiciel/donnees/naf-base.json`, correspondance de l'INSEE) : 9001Z est de la NAF rev. 2, et passe
    /// en 9020Y ou 9039G, de la NAF 2025.
    #[test]
    fn la_table_livree_dit_vers_quels_codes_passe_le_9001z() {
        assert!(en_naf_rev2("9001Z").unwrap() && !en_naf_2025("9001Z").unwrap());
        assert!(en_naf_2025("9020Y").unwrap() && en_naf_2025("9039G").unwrap());
        assert!(!en_naf_rev2("9020Y").unwrap());
    }
}
