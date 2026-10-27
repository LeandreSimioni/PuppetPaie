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
//!
//! LE TROP-VERSE (21/10/2026) : un ordre n'est jamais negatif ; quand une correction a la baisse laisse un credit, l'ordre
//! le dit, avec ce que la compagnie en fait (`credit`). Le credit se LIT dans le fichier, a la maille ou la norme le
//! porte : les bordereaux negatifs des mois corriges (22.005) pour l'Urssaf, les composants (55.001) pour un organisme
//! complementaire, le prelevement retenu (50.009 et 56.007) pour la DGFiP, les cotisations individuelles (81.004) de
//! l'organisme pour les autres -- par 81.002 quand la cotisation le nomme, par son code (`codes81` de la configuration)
//! quand la norme dit 81.002 « non concerne » (Agirc-Arrco, Conges Spectacles : `texte/bloc-S21.G00.81.txt`, l. 372 et
//! 379). Ce qu'il en advient est dit a un seul endroit, `bordereau::credit`.

use serde_json::{json, Map, Value};

use crate::bordereau::{self, Destinataire};
use socle::date::Jour;
use socle::num::{Decimal, Resultat};

/// Un bloc S21.G00.20, tel que le fichier l'ecrit.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Ordre {
    pub organisme: String,
    pub montant: String,
    pub debut: String,
    pub fin: String,
    pub date_paiement: String,
    /// La somme des composants de versement (55.001) qui suivent le bloc, s'il en a.
    pub composants: Option<Decimal>,
}

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
        if let Some(v) = ligne.strip_prefix("S21.G00.55.001,'").and_then(|v| v.strip_suffix('\'')) {
            if let (Some(o), Ok(m)) = (out.last_mut(), v.parse::<Decimal>()) {
                o.composants = Some(o.composants.unwrap_or_default() + m);
            }
            continue;
        }
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
    // `mois` vient d'un `Jour`, donc de 1 a 12 : le repli (vide) ne peut pas arriver.
    format!("{} {}", socle::date::nom_du_mois(mois as i64, socle::date::Graphie::Accentuee).unwrap_or_default(), an)
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
    let mut j = Jour::nouveau(an, mois, socle::date::jours_du_mois(an, mois)).ok_or_else(|| socle::date::jour_impossible(an, mois, 31))?;
    while crate::echeance_dsn::non_ouvre(&j)?.is_some() {
        j = j.plus_jours(-1)?;
    }
    Ok(j)
}

/// Le `jour` du mois qui suit le mois declare `mois` (`AAAA-MM`).
fn jour_du_mois_suivant(mois: &str, jour: u32) -> Resultat<Jour> {
    let d = socle::date::premier_du_mois(mois)?;
    let (an, mo) = if d.mois == 12 { (d.an + 1, 1) } else { (d.an, d.mois + 1) };
    Jour::nouveau(an, mo, jour).ok_or_else(|| socle::date::jour_impossible(an, mo, jour))
}

fn versement_de<'a>(organismes: &'a Value, id: &str) -> Option<&'a Map<String, Value>> {
    organismes.get("versements")?.as_array()?.iter().filter_map(Value::as_object).find(|v| {
        v.get("siret").map(socle::date::en_texte).as_deref() == Some(id)
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

/// Les paires (rubrique, valeur) du fichier, dans l'ordre.
fn paires(texte: &str) -> Vec<(&str, &str)> {
    texte
        .split('\n')
        .filter_map(|l| {
            let (k, v) = l.trim_end_matches('\r').split_once(",'")?;
            Some((k, v.strip_suffix('\'').unwrap_or(v)))
        })
        .collect()
}

fn montant_lu(v: &str) -> Option<Decimal> {
    v.parse::<Decimal>().ok()
}

/// Les cotisations individuelles (81.004) dues a `organisme` : celles dont 81.002 le nomme, et, sans 81.002, celles dont
/// le code est l'un de `codes` (les codes simples du versement configure).
fn cotisations_de(p: &[(&str, &str)], organisme: &str, codes: &[String]) -> Decimal {
    let mut s = Decimal::ZERO;
    let mut courant: Option<(&str, Option<&str>, Option<Decimal>)> = None;
    let fermer = |c: Option<(&str, Option<&str>, Option<Decimal>)>, s: &mut Decimal| {
        if let Some((code, org, Some(m))) = c {
            let a_lui = match org {
                Some(o) => o == organisme,
                None => codes.iter().any(|x| x == code),
            };
            if a_lui {
                *s += m;
            }
        }
    };
    for (k, v) in p {
        match *k {
            "S21.G00.81.001" => {
                fermer(courant.take(), &mut s);
                courant = Some((v, None, None));
            }
            "S21.G00.81.002" => {
                if let Some(c) = courant.as_mut() {
                    c.1 = Some(v);
                }
            }
            "S21.G00.81.004" => {
                if let Some(c) = courant.as_mut() {
                    c.2 = montant_lu(v);
                }
            }
            k if k.starts_with("S21.G00.81.") => {}
            _ => fermer(courant.take(), &mut s),
        }
    }
    fermer(courant.take(), &mut s);
    s
}

/// Le credit d'un ordre (`bordereau::credit`), lu dans le fichier ; `None` quand il n'y en a pas.
fn credit_de(
    o: &Ordre, p: &[(&str, &str)], texte: &str, mois: &str, organisme: &str, urssaf: &str, config: Option<&Map<String, Value>>,
) -> Resultat<Option<bordereau::Credit>> {
    let montant = montant_lu(&o.montant).unwrap_or_default();
    let mois_de = |jjmmaaaa: &str| jour_dsn(jjmmaaaa).map(|j| j.mois_iso());
    if o.organisme == "DGFIP" {
        if !montant.is_zero() {
            return Ok(None);
        }
        let lignes: Vec<String> = texte.split('\n').map(|l| l.trim_end_matches('\r').to_string()).collect();
        let pas = crate::dsn::pas_retenu(&lignes)?;
        return Ok(bordereau::credit(Destinataire::Dgfip, organisme, -pas, Decimal::ZERO));
    }
    if !urssaf.is_empty() && o.organisme == urssaf {
        if mois_de(&o.debut).as_deref() != Some(mois) {
            return Ok(None);
        }
        // Les bordereaux (22.003 debut, 22.005 montant) du fichier.
        let mut bordereaux: Vec<(Option<String>, Decimal)> = Vec::new();
        for (k, v) in p {
            match *k {
                "S21.G00.22.003" => bordereaux.push((mois_de(v), Decimal::ZERO)),
                "S21.G00.22.005" => {
                    if let Some(b) = bordereaux.last_mut() {
                        b.1 = montant_lu(v).unwrap_or_default();
                    }
                }
                _ => {}
            }
        }
        let courant = bordereaux.iter().filter(|(m, _)| m.as_deref() == Some(mois)).map(|(_, x)| *x).sum::<Decimal>();
        let negatifs = -bordereaux
            .iter()
            .filter(|(m, x)| m.as_deref() != Some(mois) && *x < Decimal::ZERO)
            .map(|(_, x)| *x)
            .sum::<Decimal>();
        if negatifs <= Decimal::ZERO {
            return Ok(None);
        }
        let deduit = (courant - montant).max(Decimal::ZERO);
        return Ok(bordereau::credit(Destinataire::Urssaf, organisme, negatifs - deduit, deduit));
    }
    if let Some(c) = o.composants {
        return Ok(bordereau::credit(Destinataire::Complementaire, organisme, -c, Decimal::ZERO));
    }
    if !montant.is_zero() {
        return Ok(None);
    }
    let codes: Vec<String> = match config {
        Some(v) => crate::dsn::codes_du_versement(&Value::Object(v.clone())).into_iter().filter(|c| !c.contains(':')).collect(),
        None => Vec::new(),
    };
    let du = cotisations_de(p, &o.organisme, &codes);
    Ok(bordereau::credit(Destinataire::Autre, organisme, -du, Decimal::ZERO))
}

/// Les ordres du fichier DSN `texte`, mois principal declare `mois` (`AAAA-MM`), en clair : `organisme` (le nom propre,
/// ou l'identifiant quand rien ne le connait), `identifiant`, `objet`, `montant`, `periode` (du, au), `date` (ISO, ou
/// `null` quand aucun texte ne la dit), `quand` (la phrase de la page), `source` (d'ou vient la date) et `credit` (le
/// trop-verse que l'ordre ne reprend pas ou reprend, et ce que la compagnie en fait ; `null` sans trop-verse).
/// `organismes` : `config/organismes.json` (Null s'il manque) ; `jour_echeance` : 5 ou 15 (`Contexte::echeance_dsn`).
pub fn decrire(texte: &str, mois: &str, organismes: &Value, jour_echeance: u32) -> Resultat<Vec<Value>> {
    let urssaf = texte
        .split('\n')
        .find_map(|l| l.trim_end_matches('\r').strip_prefix("S21.G00.22.001,'").and_then(|v| v.strip_suffix('\'')))
        .unwrap_or("")
        .to_string();
    let p = paires(texte);
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
        let credit = credit_de(&o, &p, texte, mois, &organisme, &urssaf, config)?.map(|c| {
            json!({"reste": crate::dsn::mt(c.reste), "deduit": crate::dsn::mt(c.deduit), "titre": c.titre, "que_faire": c.que_faire,
                "source": c.source})
        });
        out.push(json!({
            "organisme": organisme,
            "identifiant": o.organisme,
            "objet": objet,
            "montant": o.montant,
            "periode": {"du": debut.map(|j| j.iso()), "au": fin.map(|j| j.iso())},
            "date": date.map(|j| j.iso()),
            "quand": quand,
            "source": source,
            "credit": credit,
        }));
    }
    Ok(out)
}
