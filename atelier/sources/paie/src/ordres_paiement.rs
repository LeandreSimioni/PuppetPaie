//! LES ORDRES DE PAIEMENT D'UNE DSN, EN CLAIR (ticket #223, 19/10/2026 : « une espece de liste de paiement, on ne sait
//! pas a qui »). Chaque bloc S21.G00.20 du fichier est un ordre de prelevement ; la boite de depot le montait en montant
//! nu. Ici, un seul endroit dit, par ordre : A QUI (l'organisme), POUR QUOI (ce qu'il couvre, et sur quelle periode),
//! COMBIEN, et POUR QUAND. La page affiche ce que rend `decrire`, rien d'autre (regle 2).
//!
//! CE QUI SE LIT (regle 1 : rien ne se devine) :
//!
//! - L'ORGANISME : S21.G00.20.001 du fichier ; son nom, dans `config/organismes.json` (`versements`, par `siret`) ;
//!   « DGFIP » est la valeur que la norme donne a la DGFiP (cahier technique 2026.1, bloc S21.G00.20, rubrique 001,
//!   `atelier/reference/norme-dsn/texte/bloc-S21.G00.20.txt`, l. 47). Un identifiant que la configuration ne connait
//!   pas se dit tel quel, sans nom invente.
//! - L'URSSAF : l'organisme du bordereau (S21.G00.22.001) du meme fichier ; le bordereau est propre a l'Urssaf.
//! - LA RETRAITE COMPLEMENTAIRE : l'organisme dont `codes81` porte le 131 ou le 105, les codes « Cotisation regime unifie
//!   Agirc-Arrco » de la liste S21.G00.81.001 (`texte/bloc-S21.G00.81.txt`, l. 269 et 287).
//! - LE MONTANT ET LA PERIODE : S21.G00.20.005, .006 et .007.
//!
//! POUR QUAND, du plus sur au moins sur, et toujours avec le texte qui le dit :
//!
//! 1. S21.G00.20.011, quand le fichier la porte : « Date a prendre en compte par l'organisme dans le cas d'un
//!    prelevement SEPA » (`bloc-S21.G00.20.txt`, rubrique 011).
//! 2. L'Urssaf (la norme : « non concerne » par 011) : CSS R. 243-6, II, le versement est fait « le mois suivant la
//!    periode de travail [...] au plus tard » le 5 ou le 15 (`atelier/reference/sources/echeances-dsn/
//!    legi-securite-sociale-R243-6-LEGIARTI000034668597.xml`) : la date NOMINALE de `echeance_dsn::du_mois`. Elle ne
//!    se reporte pas : R. 133-14, I ne reporte que la declaration, et aucun texte depose ne dit qu'un prelevement se
//!    reporte (`echeances-dsn/LISEZ-MOI.md`, « Ce qui n'en depend pas » ; la meme regle que `dsn.rs` pour 20.011).
//! 3. Agirc-Arrco (« non concerne » par 011) : « le prelevement sera execute au plus tard le dernier jour ouvre du mois
//!    d'exigibilite des cotisations, soit, pour un versement mensuel : dernier jour ouvre du mois qui suit le mois
//!    principal declare ; trimestriel : dernier jour ouvre du mois qui suit le trimestre civil » (cahier d'aide a la
//!    codification Agirc-Arrco CT2026, p. 47, `atelier/reference/sources/agirc-arrco/
//!    agirc-arrco-aide-codification-dsn-ct2026-v1.0-20251201.txt`, l. 2401-2405). Le mois qui suit la fin de periode
//!    (S21.G00.20.007) couvre les deux cas. « Ouvre » : ni samedi, ni dimanche, ni fete legale (`echeance_dsn::non_ouvre`).
//! 4. La DGFiP : CGI art. 1671, 2 : « Ce versement intervient a une date fixee par decret le mois suivant celui au cours
//!    duquel a eu lieu le prelevement » (`atelier/reference/sources/code-general-des-impots/articles-20261015.md`). Le
//!    decret n'est pas releve : le mois se dit, le jour non.
//! 5. Tout autre organisme sans S21.G00.20.011 : aucun texte depose ne dit son jour. Le logiciel le dit.

use serde_json::{json, Map, Value};

use crate::date::Jour;
use crate::num::Resultat;

/// Un bloc S21.G00.20, tel que le fichier l'ecrit.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Ordre {
    pub organisme: String,
    pub montant: String,
    pub debut: String,
    pub fin: String,
    pub date_paiement: String,
}

const MOIS: [&str; 12] =
    ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"];

const SOURCE_011: &str = "Date de paiement écrite dans la DSN (S21.G00.20.011) : « date à prendre en compte par l'organisme dans le cas d'un prélèvement SEPA » (cahier technique DSN 2026.1, bloc S21.G00.20).";
const SOURCE_URSSAF: &str = "Code de la sécurité sociale, art. R. 243-6, II : le versement est effectué le mois suivant la période de travail, au plus tard le 5 ou le 15 (sans report : seul le dépôt de la DSN se reporte un jour non ouvré, R. 133-14, I).";
const SOURCE_AGIRC: &str = "Cahier d'aide à la codification Agirc-Arrco CT2026, p. 47 : « le prélèvement sera exécuté au plus tard le dernier jour ouvré du mois d'exigibilité des cotisations », le mois qui suit la période déclarée (ou le trimestre civil).";
const SOURCE_DGFIP: &str = "Code général des impôts, art. 1671, 2 : le prélèvement à la source est versé « à une date fixée par décret le mois suivant ». Ce décret n'est pas encore relevé dans le logiciel : le mois est sûr, le jour n'est pas dit.";
const SOURCE_AUCUNE: &str = "La DSN ne porte pas de date de paiement pour cet organisme (S21.G00.20.011 absente), et aucun texte relevé par le logiciel ne dit quand il prélève : l'organisme prélève à sa propre échéance.";

/// Les blocs S21.G00.20 du texte, dans l'ordre du fichier : un bloc commence a chaque S21.G00.20.001.
pub fn lire(texte: &str) -> Vec<Ordre> {
    let mut out: Vec<Ordre> = Vec::new();
    for ligne in texte.split('\n') {
        let ligne = ligne.trim_end_matches('\r');
        let Some(reste) = ligne.strip_prefix("S21.G00.20.") else { continue };
        let Some((rub, valeur)) = reste.split_once(",'") else { continue };
        let valeur = valeur.strip_suffix('\'').unwrap_or(valeur).to_string();
        if rub == "001" {
            out.push(Ordre { organisme: valeur, ..Ordre::default() });
            continue;
        }
        let Some(o) = out.last_mut() else { continue };
        match rub {
            "005" => o.montant = valeur,
            "006" => o.debut = valeur,
            "007" => o.fin = valeur,
            "011" => o.date_paiement = valeur,
            _ => {}
        }
    }
    out
}

/// `JJMMAAAA` -> `Jour`, ou `None`.
fn jour_dsn(v: &str) -> Option<Jour> {
    if v.len() != 8 || !v.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Jour::nouveau(v[4..8].parse().ok()?, v[2..4].parse().ok()?, v[0..2].parse().ok()?)
}

fn nom_du_mois(an: i32, mois: u32) -> String {
    format!("{} {}", MOIS[(mois as usize).saturating_sub(1).min(11)], an)
}

/// « d'août 2026 », « de septembre 2026 », ou « du 01/07/2026 au 30/09/2026 ».
fn periode_lisible(debut: Option<Jour>, fin: Option<Jour>) -> String {
    match (debut, fin) {
        (Some(d), Some(f)) if d.jour == 1 && d.an == f.an && d.mois == f.mois && f.jour == f.jours_du_mois() => {
            let n = nom_du_mois(d.an, d.mois);
            if n.starts_with(['a', 'o']) {
                format!("d'{}", n)
            } else {
                format!("de {}", n)
            }
        }
        (Some(d), Some(f)) => format!("du {} au {}", d.jj_mm_aaaa(), f.jj_mm_aaaa()),
        _ => "d'une période illisible dans le fichier".to_string(),
    }
}

/// Le dernier jour ouvre (ni samedi, ni dimanche, ni fete legale) du mois qui suit `fin`.
fn dernier_ouvre_du_mois_suivant(fin: &Jour) -> Resultat<Jour> {
    let (an, mois) = if fin.mois == 12 { (fin.an + 1, 1) } else { (fin.an, fin.mois + 1) };
    let mut j = Jour::nouveau(an, mois, crate::date::jours_du_mois(an, mois)).ok_or_else(|| crate::date::jour_impossible(an, mois, 31))?;
    while crate::echeance_dsn::non_ouvre(&j)?.is_some() {
        j = j.plus_jours(-1)?;
    }
    Ok(j)
}

fn versement_de<'a>(organismes: &'a Value, id: &str) -> Option<&'a Map<String, Value>> {
    organismes.get("versements")?.as_array()?.iter().filter_map(Value::as_object).find(|v| {
        v.get("siret").map(crate::date::en_texte).as_deref() == Some(id)
    })
}

fn est_agirc_arrco(v: Option<&Map<String, Value>>) -> bool {
    v.and_then(|v| v.get("codes81")).and_then(Value::as_array).is_some_and(|c| {
        c.iter().filter_map(Value::as_str).any(|x| x == "131" || x == "105")
    })
}

/// Les ordres du fichier DSN `texte`, mois principal declare `mois` (`AAAA-MM`), en clair : `organisme` (le nom, ou
/// l'identifiant quand la configuration ne le connait pas), `identifiant`, `objet`, `montant`, `periode` (du, au),
/// `date` (ISO, ou `null` quand aucun texte ne la dit), `quand` (la phrase de la page) et `source` (d'ou vient la date).
/// `organismes` : `config/organismes.json` (Null s'il manque) ; `jour_echeance` : 5 ou 15 (`Contexte::echeance_dsn`).
pub fn decrire(texte: &str, mois: &str, organismes: &Value, jour_echeance: u32) -> Resultat<Vec<Value>> {
    let urssaf = texte
        .split('\n')
        .find_map(|l| l.trim_end_matches('\r').strip_prefix("S21.G00.22.001,'").and_then(|v| v.strip_suffix('\'')))
        .unwrap_or("")
        .to_string();
    let mut out = Vec::new();
    for o in lire(texte) {
        let v = versement_de(organismes, &o.organisme);
        let dgfip = o.organisme == "DGFIP";
        let est_urssaf = !urssaf.is_empty() && o.organisme == urssaf;
        let libelle = v.and_then(|v| v.get("libelle")).and_then(Value::as_str).unwrap_or("");
        let organisme = if dgfip {
            "DGFiP (impôts)".to_string()
        } else if !libelle.is_empty() {
            libelle.to_string()
        } else if est_urssaf {
            "Urssaf".to_string()
        } else {
            format!("Organisme {} (absent de la configuration)", o.organisme)
        };
        let (debut, fin) = (jour_dsn(&o.debut), jour_dsn(&o.fin));
        let periode = periode_lisible(debut, fin);
        let regularisation = debut.is_some_and(|d| d.mois_iso() != mois);
        let quoi = if dgfip {
            "l'impôt sur le revenu prélevé à la source sur les salaires"
        } else if est_urssaf {
            "les cotisations et contributions sociales du bordereau Urssaf"
        } else if est_agirc_arrco(v) {
            "les cotisations de retraite complémentaire"
        } else {
            "les cotisations déclarées"
        };
        let objet = format!("{}{} {}", if regularisation { "Régularisation : " } else { "" }, quoi, periode);
        let objet = {
            let mut c = objet.chars();
            c.next().map(|p| p.to_uppercase().chain(c).collect::<String>()).unwrap_or_default()
        };
        let (date, quand, source): (Option<Jour>, String, &str) = if let Some(j) = jour_dsn(&o.date_paiement) {
            (Some(j), format!("le {}", j.jj_mm_aaaa()), SOURCE_011)
        } else if est_urssaf {
            let e = crate::echeance_dsn::du_mois(mois, jour_echeance)?;
            (Some(e.nominale), format!("au plus tard le {}", e.nominale.jj_mm_aaaa()), SOURCE_URSSAF)
        } else if est_agirc_arrco(v) && fin.is_some() {
            let j = dernier_ouvre_du_mois_suivant(&fin.expect("fin lue"))?;
            (Some(j), format!("au plus tard le {}", j.jj_mm_aaaa()), SOURCE_AGIRC)
        } else if dgfip {
            let d = crate::date::premier_du_mois(mois)?;
            let suivant = d.mois_suivant();
            let (an, mo) = (suivant[..4].parse().unwrap_or(d.an), suivant[5..7].parse().unwrap_or(d.mois));
            (None, format!("en {} (jour non relevé)", nom_du_mois(an, mo)), SOURCE_DGFIP)
        } else {
            (None, "date non dite par la DSN".to_string(), SOURCE_AUCUNE)
        };
        out.push(json!({
            "organisme": organisme,
            "identifiant": o.organisme,
            "objet": objet,
            "montant": o.montant,
            "periode": {"du": debut.map(|j| j.iso()), "au": fin.map(|j| j.iso())},
            "date": date.map(|j| j.iso()),
            "quand": quand,
            "source": source,
        }));
    }
    Ok(out)
}
