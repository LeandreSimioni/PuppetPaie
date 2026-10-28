//! Le tableau des usages de la norme : ce qu'une nature de declaration admet
//! (`moteur/usages.py`). Un releve par version de la norme : le JSON que
//! `norme-dsn-base.json` nomme (`usages-dsn-base.json` pour P26V01,
//! `usages-dsn-p27v01-base.json` pour P27V01), choisi d'apres
//! `S10.G00.00.006` du fichier controle (`norme::du_fichier`).

use socle::fichiers::{dossier_du_code, json_en_cache};
use crate::norme::Norme;
use socle::num::Resultat;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Arc;

pub(crate) use socle::texte::{blanc as espace, lignes_py as lignes, strip};

/// `ligne.partition(",")` puis la valeur sans espaces ni apostrophes.
pub fn rubrique_valeur(ligne: &str) -> (&str, &str) {
    match ligne.split_once(',') {
        Some((r, v)) => (r, strip(v).trim_matches('\'')),
        None => (ligne, ""),
    }
}

/// Le tableau des usages de cette version de la norme.
pub fn table_de(norme: &Norme) -> Resultat<Arc<Value>> {
    json_en_cache(&dossier_du_code().join(norme.table_usages()?))
}

/// `S\d\d\.G00\.\d\d\.\d\d\d`, en entier.
pub fn est_rubrique(r: &str) -> bool {
    let c: Vec<char> = r.chars().collect();
    c.len() == 14
        && c[0] == 'S'
        && c[3] == '.'
        && c[4..7] == ['G', '0', '0']
        && c[7] == '.'
        && c[10] == '.'
        && [1, 2, 8, 9, 11, 12, 13].iter().all(|&i| c[i].is_ascii_digit())
}

fn entree<'a>(t: &'a Value, rubrique: &str) -> Option<&'a Value> {
    t["usages"].get(rubrique).filter(|v| socle::fichiers::vrai(v))
}

/// 'O', 'C', 'I', 'N' -- ou "" si la rubrique est inconnue du tableau de cette version.
pub fn usage_en(norme: &Norme, rubrique: &str, nature: &str) -> Resultat<String> {
    let t = table_de(norme)?;
    Ok(entree(&t, rubrique)
        .and_then(|e| e.get(nature))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string())
}

/// Les rubriques ecrites que cette nature n'admet pas : (rubrique, usage).
pub fn interdites(texte: &str, nature: &str, norme: &Norme) -> Resultat<Vec<(String, String)>> {
    let mut vues = HashSet::new();
    let mut out = Vec::new();
    for ligne in lignes(texte) {
        let r = strip(ligne.split(',').next().unwrap_or(""));
        if !est_rubrique(r) || !vues.insert(r.to_string()) {
            continue;
        }
        let u = usage_en(norme, r, nature)?;
        if u != "O" && u != "C" {
            out.push((r.to_string(), u));
        }
    }
    Ok(out)
}

/// Refuse avant le depot ce que le point de depot refuserait (CST-04).
pub fn verifier(texte: &str, nature: &str, norme: &Norme) -> Resultat<()> {
    let mauvaises = interdites(texte, nature, norme)?;
    if mauvaises.is_empty() {
        return Ok(());
    }
    let t = table_de(norme)?;
    let libelle = t["natures"].get(nature).and_then(|v| v.as_str()).unwrap_or(nature);
    let lignes: Vec<String> = mauvaises
        .iter()
        .map(|(r, u)| {
            let quoi = match u.as_str() {
                "I" => "interdite",
                "N" => "sans objet",
                "" => "inconnue du tableau",
                autre => autre,
            };
            let lib = entree(&t, r)
                .and_then(|e| e.get("libelle"))
                .filter(|v| socle::fichiers::vrai(v))
                .and_then(|v| v.as_str());
            match lib {
                Some(l) => format!("    {} : {} ({})", r, quoi, l),
                None => format!("    {} : {}", r, quoi),
            }
        })
        .collect();
    Err(format!(
        "Rubrique(s) que la nature {} ({}) n'admet pas -- anomalie bloquante CST-04, le fichier ne declarerait rien :\n{}\n  Tableau des usages du cahier technique NEODeS (donnees/{}).",
        nature,
        libelle,
        lignes.join("\n"),
        norme.table_usages()?
    ))
}

/// La nature declaree dans le fichier lui-meme (S20.G00.05.001).
pub fn nature_du_fichier(texte: &str) -> String {
    for ligne in lignes(texte) {
        if let Some(reste) = ligne.strip_prefix("S20.G00.05.001,") {
            return strip(reste).trim_matches('\'').to_string();
        }
    }
    String::new()
}

const CCH_14: &str = concat!(
    "CCH-14 : Si la \"Nature de la declaration - S20.G00.05.001\" est ",
    "\"07 - Signalement Fin du contrat de travail Unique\", alors la valeur ",
    "\"31 - Elements de cotisation Prevoyance, Sante, retraite ",
    "supplementaire\" est interdite pour la rubrique \"Code de base ",
    "assujettie - S21.G00.78.001\". ",
    "(Cahier technique NEODeS 2026.1, p. 293.)"
);

const CCH_15: &str = concat!(
    "CCH-15 : Si la \u{ab} Nature de la declaration - S20.G00.05.001 \u{bb} ",
    "est \u{ab} 07 \u{bb} alors, pour un meme bloc \u{ab} Versement ",
    "individu - S21.G00.50 \u{bb}, au moins trois blocs \u{ab} Remuneration ",
    "- S21.G00.51 \u{bb} enfants sont requis : un de type (S21.G00.51.011) ",
    "\u{ab} 001 - Remuneration brute non plafonnee \u{bb}, un de type ",
    "\u{ab} 002 - Salaire brut servant aux calculs des droits de l'Assurance ",
    "chomage \u{bb} et un de type \u{ab} 010 - Salaire de base \u{bb}. ",
    "(Cahier technique NEODeS 2026.1, p. 248.)"
);

const TYPES_51_FCTU: [&str; 3] = ["001", "002", "010"];

/// Les deux controles que la nature 07 ajoute au tableau des usages.
pub fn verifier_fctu(texte: &str) -> Resultat<()> {
    let mut versements: Vec<HashSet<String>> = Vec::new();
    for ligne in lignes(texte) {
        let (rubrique, valeur) = rubrique_valeur(ligne);
        if rubrique == "S21.G00.50.001" {
            versements.push(HashSet::new());
        } else if rubrique == "S21.G00.51.011" && !versements.is_empty() {
            versements.last_mut().unwrap().insert(valeur.to_string());
        } else if rubrique == "S21.G00.78.001" && valeur == "31" {
            return Err(format!(
                "Base assujettie de code 31 dans un signalement de fin de contrat.\n  {}",
                CCH_14
            ));
        }
    }
    for (i, types) in versements.iter().enumerate() {
        let manquants: Vec<&str> = TYPES_51_FCTU.iter().copied().filter(|x| !types.contains(*x)).collect();
        if !manquants.is_empty() {
            return Err(format!(
                "Versement individu n\u{b0} {} : il manque le(s) bloc(s) 51 de type {}.\n  {}",
                i + 1,
                manquants.join(", "),
                CCH_15
            ));
        }
    }
    Ok(())
}

/// Confronte un fichier produit a sa propre nature, dans le tableau de sa propre version (S10.G00.00.006) : un
/// fichier sans version, ou d'une version que le logiciel ne livre pas, refuse.
pub fn verifier_fichier(texte: &str) -> Resultat<()> {
    let norme = crate::norme::du_fichier(texte)?;
    let nature = nature_du_fichier(texte);
    if table_de(&norme)?["natures"].get(&nature).is_some() {
        verifier(texte, &nature, &norme)?;
    }
    if nature == "07" {
        verifier_fctu(texte)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::norme::Norme;

    #[test]
    fn un_fichier_sans_version_connue_ne_se_controle_pas() {
        let e = verifier_fichier("S10.G00.00.006,'P25V01'\r\nS20.G00.05.001,'01'\r\n").unwrap_err();
        assert!(e.contains("Version de la norme DSN « P25V01 » inconnue du logiciel (versions livrees : P26V01, P27V01"), "{}", e);
    }

    /// Le releve de `paie extraire usages`, sur le PDF 2027.1.1 puis sur les classeurs
    /// 2027.1.2 (fiches 3357 et 3358) : 20 rubriques ajoutees, 2 retirees, 11.024 qui change d'usage en
    /// mensuelle (N9) ; le 2027.1.2 passe 40.084 de O a C en mensuelle (journal de maintenance, evolution 21).
    /// `atelier/reference/norme-dsn/dsn-tableau-des-usages-2027.1.2-20251224.xlsx`, `dsn-journal-maintenance-2027.1.2-20260615.txt`.
    #[test]
    fn les_usages_2027_sont_ceux_du_cahier_2027() {
        let (p26, p27) = (Norme::connue("P26V01").unwrap(), Norme::connue("P27V01").unwrap());
        let (u26, u27) = (table_de(&p26).unwrap(), table_de(&p27).unwrap());
        let (a, b) = (u26["usages"].as_object().unwrap(), u27["usages"].as_object().unwrap());
        let retirees: Vec<&String> = a.keys().filter(|k| !b.contains_key(*k)).collect();
        let ajoutees: Vec<&String> = b.keys().filter(|k| !a.contains_key(*k)).collect();
        assert_eq!(retirees, ["S21.G00.06.003", "S21.G00.11.002"]);
        assert_eq!(ajoutees.len(), 20, "{:?}", ajoutees);
        assert_eq!(usage_en(&p27, "S21.G00.40.084", "01").unwrap(), "C");
        assert_eq!(usage_en(&p26, "S21.G00.11.024", "01").unwrap(), "I");
        assert_eq!(usage_en(&p27, "S21.G00.11.024", "01").unwrap(), "C");
    }
}
