//! `paie extraire formats <cahier.pdf | categories-controles-datatypes.xlsx> <sortie.json>` : le FORMAT de chaque rubrique
//! de la norme DSN, pour `logiciel/donnees/formats-dsn-base.json` (P26V01) et `formats-dsn-p27v01-base.json` (P27V01),
//! que lit `formats.rs`.
//!
//! Le tableau des usages (`extraire::usages`) dit si une rubrique a le droit d'etre la. Il ne dit pas ce qu'elle doit
//! CONTENIR. Or chaque rubrique du cahier technique porte, en face de sa definition, sa longueur `[min,max]` et une
//! expression reguliere de controle nommee CSL -- par exemple, pour un montant, `-?(([0-9]){1,12})\.[0-9]{2}`, et pour
//! une date `[0-9]{8}`. Le 19/09/2026, le montant d'une indemnite de rupture s'ecrivait dans `S21.G00.52.004`, qui est
//! une DATE : les quatre rubriques du bloc 52 avaient le droit d'etre la, c'est leur CONTENU qui etait faux. Une
//! expression de controle le dit tout de suite.
//!
//! Deux sources, selon l'extension, deposees dans `atelier/reference/norme-dsn/` (son `LISEZMOI.md`) :
//!
//! - LE CLASSEUR (depuis la norme 2027.1.2, fiche 3357 :
//!   `dsn-tableaux-categories-controles-datatypes-usages-<version>-<AAAAMMJJ>.xlsx`). Feuille « Fields », le type de
//!   donnees de chaque rubrique (colonne « DataType Id ») ; feuille « Data Types », pour chaque type son expression
//!   (« Regexp », la CSL 00 du cahier) et sa longueur (« Lg Min », « Lg Max »). Le releve recompose le meme JSON :
//!   longueur [Lg Min, Lg Max] quand le type en porte une, `csl` = [Regexp] quand il en porte une. Une rubrique que la
//!   feuille « Fields » type d'un type inconnu de « Data Types » n'a pas de format (aucun controle) : le releve la nomme
//!   (`_types_inconnus`), il ne la reprend pas d'ailleurs. Preuve de la methode : sur la norme 2027, le classeur 2027.1.2
//!   redonne le releve du PDF 2027.1.1 rubrique pour rubrique, a l'exception des deux CSL que le journal de maintenance
//!   2027.1.2 change (evolutions 15 et 18 : S21.G00.40.070, S21.G00.41.045) et de S89.G00.32.014, absente de « Fields ».
//!
//! - LE CAHIER TECHNIQUE EN PDF (`dsn-cahier-technique-<version>.pdf`), dont le texte est lu par la fabrique de PDF
//!   (`fabrique_pdf::lecture::texte`) : chaque identifiant de rubrique seul sur sa ligne ouvre un corps, qui va jusqu'au
//!   suivant ; on y lit la premiere longueur `[min,max]` et chaque expression `CSL nn : ...`.
//!
//! UNE EXPRESSION QUE LE MOTEUR NE SAIT PAS COMPILER (`formats::Regex::compiler`) EST LAISSEE TOMBER, plutot que reparee a
//! la main : une expression devinee refuserait des valeurs licites. Elle est nommee dans `_expressions_ecartees`. (Le
//! releve d'origine ecartait ce que le `re` de Python ne compilait pas ; le moteur, qui seul se sert de ces expressions,
//! en reconnait un sous-ensemble prouve contre `re` : le critere est desormais le sien. Sur les sources deposees, les deux
//! criteres ecartent les memes trois expressions, coupees par la mise en page.)
//!
//! CE QUE LE RELEVE VAUT. Il est lu dans un document, donc il peut etre faux, et un controle faux refuserait une
//! declaration licite -- pire que pas de controle du tout (regle 7). Une expression qui refuse une valeur deja acceptee
//! par net-entreprises est retiree du releve, pas imposee au moteur.
//!
//! Le PDF se lit une fois, a son depot : sa transcription (`transcrire`, `super::Transcription`) redonne le meme releve.

use std::path::Path;

use regex::Regex;
use serde_json::{json, Map, Value};

use super::usages::{en_classeur, version_du_cahier};
use super::{date_du_releve, en_transcription, nom_de, sha256, Releve, Transcription};
use socle::date::Jour;
use socle::lecture_xlsx::{self, Cellule};
use socle::num::Resultat;
use socle::texte::{blanc as espace, strip};

/// Une expression ecartee : (rubrique, expression).
type Ecartee = (String, String);

/// L'expression est-elle de celles que le moteur sait compiler ?
fn compile(expression: &str) -> bool {
    declarations::formats::Regex::compiler(expression).is_ok()
}

/// Les groupes de `CSL\s*\d*\s*:\s*(.*?)(?=\n[ \t]*\n)` (drapeau `S` : `.` prend aussi le saut de ligne), cherches de
/// gauche a droite comme `re.findall`. UNE EXPRESSION DE CONTROLE VA JUSQU'A LA LIGNE VIDE SUIVANTE : le PDF la coupe en
/// plein milieu d'un motif, sans espace (`(20)[0-` puis `9]{2}`) ; on la recolle ensuite en retirant les sauts de ligne.
/// La recherche refait pas a pas ce que fait le moteur a retour arriere de Python, y compris quand la ligne vide est
/// mangee par les blancs qui suivent les deux-points.
fn csl_trouvees(corps: &str) -> Vec<String> {
    let entete = Regex::new(r"\ACSL[\s\x1c-\x1f]*\d*[\s\x1c-\x1f]*:").expect("motif");
    let octets = corps.as_bytes();
    // Une ligne vide commence-t-elle a `p` : `\n[ \t]*\n` ?
    let ligne_vide = |p: usize| -> bool {
        if octets.get(p) != Some(&b'\n') {
            return false;
        }
        let mut q = p + 1;
        while matches!(octets.get(q), Some(b' ') | Some(b'\t')) {
            q += 1;
        }
        octets.get(q) == Some(&b'\n')
    };
    let mut trouvees = Vec::new();
    let mut depart = 0;
    while let Some(i) = corps[depart..].find("CSL").map(|i| depart + i) {
        let Some(m) = entete.find(&corps[i..]) else {
            depart = i + 1;
            continue;
        };
        let apres = i + m.end();
        // `\s*` gourmand : la fin des blancs, puis, a reculons, chaque point ou il aurait pu s'arreter.
        let mut fin_blancs = apres;
        for c in corps[apres..].chars() {
            if !espace(c) {
                break;
            }
            fin_blancs += c.len_utf8();
        }
        let mut positions = vec![fin_blancs];
        let mut q = fin_blancs;
        while q > apres {
            q -= corps[..q].chars().next_back().map_or(1, char::len_utf8);
            positions.push(q);
        }
        // `(.*?)` paresseux : la premiere ligne vide a partir du debut du groupe.
        let mut retenue = None;
        for debut_groupe in positions {
            if let Some(p) =
                (debut_groupe..=corps.len()).filter(|p| corps.is_char_boundary(*p)).find(|p| ligne_vide(*p))
            {
                retenue = Some((debut_groupe, p));
                break;
            }
        }
        match retenue {
            Some((a, b)) => {
                trouvees.push(corps[a..b].to_string());
                depart = b.max(i + 1);
            }
            None => depart = i + 1,
        }
    }
    trouvees
}

/// Les formats lus dans le texte du cahier.
fn relever_pdf(texte: &str, ecartees: &mut Vec<Ecartee>) -> Resultat<Map<String, Value>> {
    let rubrique = Regex::new(r"(?m)^[\s\x1c-\x1f]*(S\d\d\.G00\.\d\d\.\d\d\d)[\s\x1c-\x1f]*$").expect("motif");
    let longueur = Regex::new(r"\[(\d+),(\d+)\]").expect("motif");
    // `RUBRIQUE.split(texte)` : [avant, id, corps, id, corps...].
    let mut morceaux: Vec<(&str, &str)> = Vec::new();
    let mut ouvert: Option<(&str, usize)> = None;
    for c in rubrique.captures_iter(texte) {
        let (tout, id) = (c.get(0).expect("match"), c.get(1).expect("groupe"));
        if let Some((rid, debut)) = ouvert {
            morceaux.push((rid, &texte[debut..tout.start()]));
        }
        ouvert = Some((id.as_str(), tout.end()));
    }
    if let Some((rid, debut)) = ouvert {
        morceaux.push((rid, &texte[debut..]));
    }
    let mut out = Map::new();
    // Chaque identifiant apparait deux fois : dans la liste des rubriques du bloc (sans format) et dans sa definition
    // (avec). On garde celle qui porte un format, et la derniere si plusieurs en portent.
    for (rid, corps) in morceaux {
        // ET ELLE S'ARRETE A LA PREMIERE ESPACE. Aucune CSL du cahier n'en contient ; en revanche la mise en page colle
        // parfois le libelle suivant derriere (S10.G00.01.002 recuperait « Table OES - Organismes Emetteurs... », ce qui
        // faisait refuser un NIC pourtant accepte cinquante fois).
        let csl: Vec<String> = csl_trouvees(corps)
            .iter()
            .map(|c| strip(&c.replace('\n', "")).split(' ').next().unwrap_or("").to_string())
            .collect();
        let lon = longueur.captures(corps);
        if csl.is_empty() && lon.is_none() {
            continue;
        }
        let mut fmt = Map::new();
        if let Some(l) = lon {
            let n = |i: usize| l[i].parse::<i64>().map_err(|_| format!("{} : longueur illisible {}", rid, &l[0]));
            fmt.insert("longueur".into(), json!([n(1)?, n(2)?]));
        }
        if !csl.is_empty() {
            // Plusieurs CSL = plusieurs formes admises (CSL 00, CSL 01...), sans doublon, dans l'ordre.
            let mut vues: Vec<&String> = Vec::new();
            let mut bonnes = Vec::new();
            for c in &csl {
                if vues.contains(&c) {
                    continue;
                }
                vues.push(c);
                if compile(c) {
                    bonnes.push(Value::String(c.clone()));
                } else {
                    ecartees.push((rid.to_string(), c.clone()));
                }
            }
            if !bonnes.is_empty() {
                fmt.insert("csl".into(), Value::Array(bonnes));
            }
        }
        out.insert(rid.to_string(), Value::Object(fmt));
    }
    Ok(out)
}

/// Une cle de dictionnaire comme Python la compare : `1 == 1.0 == True`, un texte n'egale qu'un texte.
fn cle(c: &Cellule) -> String {
    let nombre = |f: f64| {
        if f.fract() == 0.0 && f.abs() < 9.2e18 {
            format!("n{}", f as i64)
        } else {
            format!("f{}", f.to_bits())
        }
    };
    match c {
        Cellule::Texte(s) => format!("t{}", s),
        Cellule::Entier(i) => format!("n{}", i),
        Cellule::Flottant(f) => nombre(*f),
        Cellule::Booleen(b) => format!("n{}", *b as i64),
    }
}

/// `dict(zip(entete, ligne + [None] * ...))[nom]` : la valeur de la derniere colonne titree `nom`, ou `None`.
fn champ<'a>(entete: &[Option<Cellule>], ligne: &'a [Option<Cellule>], nom: &str) -> Option<Option<&'a Cellule>> {
    let j = entete.iter().rposition(|t| matches!(t, Some(Cellule::Texte(s)) if s == nom))?;
    Some(ligne.get(j).and_then(Option::as_ref))
}

fn champ_exige<'a>(
    entete: &[Option<Cellule>],
    ligne: &'a [Option<Cellule>],
    nom: &str,
    feuille: &str,
) -> Resultat<Option<&'a Cellule>> {
    champ(entete, ligne, nom).ok_or_else(|| format!("la feuille « {} » n'a pas de colonne « {} »", feuille, nom))
}

/// `int(valeur)` de Python : un entier, un flottant tronque, un booleen, un texte d'entier.
fn entier_py(c: &Cellule) -> Resultat<i64> {
    match c {
        Cellule::Entier(i) => Ok(*i),
        Cellule::Flottant(f) if f.is_finite() => Ok(f.trunc() as i64),
        Cellule::Booleen(b) => Ok(*b as i64),
        Cellule::Texte(s) => {
            strip(s).parse::<i64>().map_err(|_| format!("longueur illisible : {}", socle::fichiers::repr(s)))
        }
        Cellule::Flottant(f) => Err(format!("longueur illisible : {}", f)),
    }
}

/// Les formats lus dans le classeur des types de donnees (fiche 3357).
fn relever_xlsx(
    chemin: &Path,
    ecartees: &mut Vec<Ecartee>,
    sans_type: &mut Vec<(String, Value)>,
) -> Resultat<Map<String, Value>> {
    let mut types: std::collections::HashMap<String, Vec<Option<Cellule>>> = std::collections::HashMap::new();
    let mut entete_types: Option<Vec<Option<Cellule>>> = None;
    for r in lecture_xlsx::lignes(chemin, "Data Types")? {
        let Some(entete) = &entete_types else {
            entete_types = Some(r);
            continue;
        };
        if let Some(Some(premiere)) = r.first() {
            if premiere.vrai() {
                let mut valeurs = r.clone();
                valeurs.resize(entete.len().max(valeurs.len()), None);
                types.insert(cle(premiere), valeurs);
            }
        }
    }
    let entete_types = entete_types.unwrap_or_default();
    let mut out = Map::new();
    let mut entete: Option<Vec<Option<Cellule>>> = None;
    for r in lecture_xlsx::lignes(chemin, "Fields")? {
        let Some(e) = &entete else {
            entete = Some(r);
            continue;
        };
        if !matches!(r.first(), Some(Some(c)) if c.vrai()) {
            continue;
        }
        let bloc = champ_exige(e, &r, "Block Id", "Fields")?;
        let id = champ_exige(e, &r, "Id", "Fields")?;
        let type_id = champ_exige(e, &r, "DataType Id", "Fields")?;
        let texte = |c: Option<&Cellule>| strip(&lecture_xlsx::cellule_en_texte(&c.cloned())).to_string();
        let rid = format!("{}.{}", texte(bloc), texte(id));
        let genre = type_id.and_then(|t| types.get(&cle(t)));
        let Some(genre) = genre else {
            sans_type.push((rid, type_id.map_or(Value::Null, Cellule::json)));
            continue;
        };
        let mut fmt = Map::new();
        let lg = |nom: &str| champ(&entete_types, genre, nom).flatten();
        if let (Some(min), Some(max)) = (lg("Lg Min"), lg("Lg Max")) {
            fmt.insert("longueur".into(), json!([entier_py(min)?, entier_py(max)?]));
        }
        let expression = match lg("Regexp") {
            Some(Cellule::Texte(s)) => strip(s).to_string(),
            Some(c) if c.vrai() => {
                return Err(format!("{} : l'expression du type n'est pas un texte ({})", rid, c.en_texte()))
            }
            _ => String::new(),
        };
        if !expression.is_empty() {
            if compile(&expression) {
                fmt.insert("csl".into(), json!([expression]));
            } else {
                ecartees.push((rid.clone(), expression));
            }
        }
        if !fmt.is_empty() {
            out.insert(rid, Value::Object(fmt));
        }
    }
    Ok(out)
}

/// LA TRANSCRIPTION DES DEFINITIONS du cahier en PDF (`super::Transcription`) : son texte, tel que la fabrique de PDF le
/// lit, les pages separees par un saut de ligne -- ce que `relever_pdf` lit.
pub fn transcrire(pdf: &Path) -> Resultat<String> {
    version_du_cahier(pdf)?;
    let (textes, _) = fabrique_pdf::lecture::texte(&pdf.to_string_lossy()).map_err(|r| r.0)?;
    let t = Transcription::de(pdf, Vec::new(), textes.join("\n"))?;
    Ok(t.texte(&format!("Le texte de {}, tel que la fabrique de PDF le lit (`paie extraire transcrire`)", nom_de(pdf))))
}

/// Le releve des formats, depuis le classeur, le cahier en PDF ou sa transcription.
pub fn relever(source: &Path, jour: &Jour) -> Resultat<Releve> {
    let date = date_du_releve(jour);
    let (mut ecartees, mut sans_type) = (Vec::new(), Vec::new());
    let classeur = en_classeur(source);
    let (formats, provenance, empreinte) = if classeur {
        let chemin = source.to_string_lossy();
        let v = Regex::new(r"datatypes-usages-(\d[\d.]*\d)-\d{8}\.xlsx$")
            .expect("motif")
            .captures(&chemin)
            .map(|c| c[1].to_string())
            .ok_or_else(|| {
                format!(
                    "version illisible dans le nom du classeur : {} (attendu \
                     dsn-tableaux-categories-controles-datatypes-usages-<version>-<AAAAMMJJ>.xlsx)",
                    chemin
                )
            })?;
        let formats = relever_xlsx(source, &mut ecartees, &mut sans_type)?;
        let provenance = format!(
            "Formats des rubriques de la norme DSN {}, classeur {} (feuilles « Fields » et « Data Types »). Releve \
             automatiquement par `paie extraire formats` le {}.",
            v,
            nom_de(source),
            date
        );
        (formats, provenance, sha256(source)?)
    } else {
        // Le texte du cahier : sa transcription (`dsn-cahier-technique-<version>-formats.txt`), ou le PDF lui-meme.
        let (pdf, texte, empreinte) = if en_transcription(source) {
            let t = Transcription::lire(source)?;
            (t.pdf, t.corps, t.sha256)
        } else {
            let (textes, _) = fabrique_pdf::lecture::texte(&source.to_string_lossy()).map_err(|r| r.0)?;
            (nom_de(source), textes.join("\n"), sha256(source)?)
        };
        let v = version_du_cahier(Path::new(&pdf))?;
        let formats = relever_pdf(&texte, &mut ecartees)?;
        let provenance = format!(
            "Cahier technique NEODeS {} ({}), definitions des rubriques. Releve automatiquement par \
             `paie extraire formats` le {}.",
            v,
            pdf,
            date
        );
        (formats, provenance, empreinte)
    };
    let mut bilan = vec![format!(
        "{} rubriques, dont {} avec expression de controle",
        formats.len(),
        formats.values().filter(|f| f["csl"].as_array().is_some_and(|a| !a.is_empty())).count()
    )];
    for (r, c) in &ecartees {
        bilan.push(format!(
            "  ecartee (le moteur ne la compile pas) : {} -- {}",
            r,
            c.chars().take(40).collect::<String>()
        ));
    }
    for (r, t) in &sans_type {
        bilan.push(format!("  type de donnees inconnu : {} -- {}", r, t));
    }
    let mut donnees = Map::new();
    donnees.insert(
        "_quoi".into(),
        json!(
            "Le format de chaque rubrique du cahier technique NEODeS : sa longueur [min,max] et ses expressions de \
               controle CSL. Le tableau des usages dit si une rubrique a le droit d'etre la ; ceci dit ce qu'elle doit \
               contenir."
        ),
    );
    donnees.insert("_source".into(), Value::String(provenance));
    donnees.insert(
        (if classeur { "_sha256_du_classeur" } else { "_sha256_du_pdf" }).into(),
        Value::String(empreinte),
    );
    donnees.insert(
        "_comment_le_verifier".into(),
        json!(
            "tests/test_formats.py confronte ce releve a ce qui a ete REELLEMENT ACCEPTE : les DSN de l'historique \
               CoolPaie et la mensuelle d'aout 2026 deposee en reel. Une expression qui refuserait une de ces valeurs \
               serait fausse, et c'est elle qu'on retire -- un controle faux refuse une declaration licite, ce qui est \
               pire que pas de controle."
        ),
    );
    donnees.insert(
        "_ce_que_ce_controle_n_est_pas".into(),
        json!(
            "Un validateur. Il verifie la FORME d'une valeur, pas sa justesse : un montant faux au bon format passe."
        ),
    );
    donnees.insert(
        "_expressions_ecartees".into(),
        Value::Array(ecartees.iter().map(|(r, c)| json!({"rubrique": r, "coupee": c})).collect()),
    );
    if !sans_type.is_empty() {
        donnees.insert(
            "_types_inconnus".into(),
            Value::Array(sans_type.iter().map(|(r, t)| json!({"rubrique": r, "type": t})).collect()),
        );
    }
    donnees.insert("formats".into(), Value::Object(formats));
    Ok(Releve { donnees: Value::Object(donnees), indent: 1, saut_final: false, bilan })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_csl_va_jusqu_a_la_ligne_vide_et_se_recolle() {
        let corps = "Format\nCSL 00 : (20)[0-\n9]{2}\n \nautre\nCSL 01:[A-Z]+ Table OES\n\n";
        assert_eq!(csl_trouvees(corps), vec!["(20)[0-\n9]{2}".to_string(), "[A-Z]+ Table OES".to_string()]);
    }

    #[test]
    fn une_csl_dont_les_blancs_mangent_la_ligne_vide_rend_un_groupe_vide() {
        // Les blancs gourmands avalent « \n\n » ; Python recule jusqu'a la ligne vide et rend un groupe vide.
        assert_eq!(csl_trouvees("CSL 00 :\n\n"), vec![String::new()]);
        assert!(csl_trouvees("CSL 00 : [0-9]").is_empty());
    }
}
