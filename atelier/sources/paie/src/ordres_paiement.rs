//! LES ORDRES DE PAIEMENT D'UNE DSN, EN CLAIR (ticket #223, 19/10/2026 : « une espece de liste de paiement, on ne sait
//! pas a qui »). Chaque bloc S21.G00.20 du fichier est un ordre de prelevement ; la boite de depot le montrait en montant
//! nu. Ici, un seul endroit dit, par ordre : A QUI (l'organisme), POUR QUOI (ce qu'il couvre, et sur quelle periode),
//! COMBIEN, et POUR QUAND. La page affiche ce que rend `decrire`, rien d'autre (regle 2).
//!
//! CE QUI SE LIT (regle 1 : rien ne se devine) :
//!
//! - L'ORGANISME : S21.G00.20.001 du fichier. Son nom propre et ce que couvre son versement (`nom`, `objet`) se lisent
//!   dans la base livree, `donnees/organismes-base.json` (`organismes::versement_normatif`), qui cite pour chacun la
//!   table IVO des nomenclatures DSN et Audiens ; a defaut, le libelle de `config/organismes.json`. « DGFIP » est la
//!   valeur que la norme donne a la DGFiP (cahier technique 2026.1, `norme-dsn/texte/bloc-S21.G00.20.txt`, l. 47). Un
//!   identifiant que ni la base ni la configuration ne connaissent se dit tel quel, sans nom invente.
//! - L'URSSAF : l'organisme du bordereau (S21.G00.22.001) du meme fichier ; une Urssaf par region, le nom est generique.
//! - LA RETRAITE COMPLEMENTAIRE d'une autre institution qu'Audiens : l'organisme dont `codes81` porte le 131 ou le 105,
//!   « Cotisation regime unifie Agirc-Arrco » (`texte/bloc-S21.G00.81.txt`, l. 269 et 287).
//! - LE MONTANT ET LA PERIODE : S21.G00.20.005, .006 et .007.
//!
//! POUR QUAND, toujours avec le texte qui le dit :
//!
//! 1. S21.G00.20.011, quand le fichier la porte : « Date a prendre en compte par l'organisme dans le cas d'un
//!    prelevement SEPA » (`bloc-S21.G00.20.txt`, rubrique 011). C'est le cas de France Travail et d'Audiens Sante
//!    Prevoyance (P0983), que `dsn.rs` ecrit.
//! 2. L'Urssaf : CSS R. 243-6, II, le versement est fait « le mois suivant la periode de travail [...] au plus tard » le 5
//!    ou le 15 (`atelier/reference/sources/echeances-dsn/legi-securite-sociale-R243-6-LEGIARTI000034668597.xml`) : la date
//!    NOMINALE de `echeance_dsn::du_mois`, sans report (R. 133-14, I ne reporte que la declaration ;
//!    `echeances-dsn/LISEZ-MOI.md`, « Ce qui n'en depend pas »).
//! 3. La DGFiP : BOI-IR-PAS-30-10-30-20, § 50 (`atelier/reference/sources/bofip/BOI-IR-PAS-30-10-30-20-20190329.html`,
//!    extrait `...-extrait-reversement.md`) : pour un employeur en DSN, le reversement « intervient le mois suivant » et
//!    « doit être effectué au plus tard : le 15 du mois pour les entreprises comptant au maximum quarante-neuf salariés ;
//!    le 5 du mois pour les autres » -- le meme 5 ou 15 que `Contexte::echeance_dsn`, sans report. L'option trimestrielle
//!    sous onze salariés (§ 60) n'est pas modelisee : le moteur reverse chaque mois.
//! 4. Les organismes dont la base donne `jour_de_paiement` (Audiens : retraite, Conges Spectacles) : « L'echeance de
//!    paiement pour Audiens est pour un mois M, le 25 du mois M+1 » (Audiens, dossier DSN,
//!    `atelier/reference/sources/audiens-dsn/audiens-dossier-dsn-20261020.md`), M etant le mois declare.
//! 5. Une autre institution Agirc-Arrco : « le prelevement sera execute au plus tard le dernier jour ouvre du mois
//!    d'exigibilite des cotisations » (cahier d'aide a la codification Agirc-Arrco CT2026, p. 47,
//!    `atelier/reference/sources/agirc-arrco/agirc-arrco-aide-codification-dsn-ct2026-v1.0-20251201.txt`, l. 2401-2405) :
//!    le dernier jour ouvre du mois qui suit la fin de periode (S21.G00.20.007). « Ouvre » : ni samedi, ni dimanche, ni
//!    fete legale (`echeance_dsn::non_ouvre`).
//! 6. Tout autre organisme sans S21.G00.20.011 : aucun texte depose ne dit son jour. Le logiciel le dit.

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
const SOURCE_DGFIP: &str = "BOFiP, BOI-IR-PAS-30-10-30-20, § 50 : pour un employeur en DSN, le reversement de la retenue à la source intervient le mois suivant, au plus tard le 15 jusqu'à quarante-neuf salariés, le 5 au-delà (art. 1671 du CGI). L'option trimestrielle sous onze salariés (§ 60) n'est pas prise en compte.";
const SOURCE_AUDIENS: &str = "Audiens, dossier DSN : « L'échéance de paiement pour Audiens est pour un mois M, le 25 du mois M+1 », pour la retraite, les Congés Spectacles et la Santé Prévoyance.";
const SOURCE_AGIRC: &str = "Cahier d'aide à la codification Agirc-Arrco CT2026, p. 47 : « le prélèvement sera exécuté au plus tard le dernier jour ouvré du mois d'exigibilité des cotisations », le mois qui suit la période déclarée (ou le trimestre civil).";
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

/// Le `jour` du mois qui suit le mois declare `mois` (`AAAA-MM`).
fn jour_du_mois_suivant(mois: &str, jour: u32) -> Resultat<Jour> {
    let d = crate::date::premier_du_mois(mois)?;
    let (an, mo) = if d.mois == 12 { (d.an + 1, 1) } else { (d.an, d.mois + 1) };
    Jour::nouveau(an, mo, jour).ok_or_else(|| crate::date::jour_impossible(an, mo, jour))
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

fn texte_de<'a>(v: Option<&'a Value>, cle: &str) -> Option<&'a str> {
    v.and_then(|v| v.get(cle)).and_then(Value::as_str).filter(|s| !s.is_empty())
}

/// Les ordres du fichier DSN `texte`, mois principal declare `mois` (`AAAA-MM`), en clair : `organisme` (le nom propre,
/// ou l'identifiant quand rien ne le connait), `identifiant`, `objet`, `montant`, `periode` (du, au), `date` (ISO, ou
/// `null` quand aucun texte ne la dit), `quand` (la phrase de la page) et `source` (d'ou vient la date).
/// `organismes` : `config/organismes.json` (Null s'il manque) ; `jour_echeance` : 5 ou 15 (`Contexte::echeance_dsn`).
pub fn decrire(texte: &str, mois: &str, organismes: &Value, jour_echeance: u32) -> Resultat<Vec<Value>> {
    let urssaf = texte
        .split('\n')
        .find_map(|l| l.trim_end_matches('\r').strip_prefix("S21.G00.22.001,'").and_then(|v| v.strip_suffix('\'')))
        .unwrap_or("")
        .to_string();
    let mut out = Vec::new();
    for o in lire(texte) {
        let config = versement_de(organismes, &o.organisme);
        let norme = crate::organismes::versement_normatif(&o.organisme)?;
        let norme = norme.as_ref();
        let dgfip = o.organisme == "DGFIP";
        let est_urssaf = !urssaf.is_empty() && o.organisme == urssaf;
        let organisme = if dgfip {
            "DGFiP".to_string()
        } else if let Some(n) = texte_de(norme, "nom") {
            n.to_string()
        } else if est_urssaf {
            "Urssaf".to_string()
        } else if let Some(l) = config.and_then(|v| v.get("libelle")).and_then(Value::as_str).filter(|s| !s.is_empty()) {
            l.to_string()
        } else {
            format!("Organisme {} (inconnu de la configuration)", o.organisme)
        };
        let (debut, fin) = (jour_dsn(&o.debut), jour_dsn(&o.fin));
        let periode = periode_lisible(debut, fin);
        let regularisation = debut.is_some_and(|d| d.mois_iso() != mois);
        let quoi = if dgfip {
            "l'impôt sur le revenu prélevé à la source sur les salaires"
        } else if let Some(q) = texte_de(norme, "objet") {
            q
        } else if est_urssaf {
            "les cotisations et contributions sociales du bordereau Urssaf"
        } else if est_agirc_arrco(config) {
            "la retraite complémentaire Agirc-Arrco"
        } else {
            "les cotisations déclarées"
        };
        let objet = format!("{}{} {}", if regularisation { "Régularisation : " } else { "" }, quoi, periode);
        let objet = {
            let mut c = objet.chars();
            c.next().map(|p| p.to_uppercase().chain(c).collect::<String>()).unwrap_or_default()
        };
        let jour_norme = norme.and_then(|v| v.get("jour_de_paiement")).and_then(Value::as_u64);
        let (date, quand, source): (Option<Jour>, String, &str) = if let Some(j) = jour_dsn(&o.date_paiement) {
            (Some(j), format!("le {}", j.jj_mm_aaaa()), SOURCE_011)
        } else if est_urssaf || dgfip {
            let e = crate::echeance_dsn::du_mois(mois, jour_echeance)?;
            (Some(e.nominale), format!("au plus tard le {}", e.nominale.jj_mm_aaaa()), if dgfip { SOURCE_DGFIP } else { SOURCE_URSSAF })
        } else if let Some(jour) = jour_norme {
            let j = jour_du_mois_suivant(mois, jour as u32)?;
            (Some(j), format!("le {}", j.jj_mm_aaaa()), SOURCE_AUDIENS)
        } else if est_agirc_arrco(config) && fin.is_some() {
            let j = dernier_ouvre_du_mois_suivant(&fin.expect("fin lue"))?;
            (Some(j), format!("au plus tard le {}", j.jj_mm_aaaa()), SOURCE_AGIRC)
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
