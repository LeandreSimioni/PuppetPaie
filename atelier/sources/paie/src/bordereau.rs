//! Le montant du bordereau Urssaf, CALCULE depuis ses lignes (blocs 22 et 23).
//!
//! En DSN, le bordereau EST le bloc 22, et son montant est la somme de ses
//! lignes 23, chacune valorisee au taux que la table des CTP de l'Urssaf donne
//! pour la periode. CoolPaie ne fait pas autre chose : rejouee sur ses 75 DSN
//! deposees (2021-2026), la regle ci-dessous retrouve 74 bordereaux au centime.
//! Le 75e, octobre 2025, donne 394 pour 393 declares : les lignes arrondies une
//! a une font 394 (0 + 113 + 109 + 14 + 126 + 1 + 23 + 8), la somme brute
//! 393,23 -- un euro que CoolPaie arrondissait autrement ce mois-la, et
//! qu'aucune variante n'explique sans en casser d'autres.
//!
//! La regle, ligne par ligne :
//!
//! - format A, C, D, E, K (une assiette, S21.G00.23.004) : assiette x taux, le
//!   taux PLAFONNE pour le qualifiant 921, DEPLAFONNE pour le 920, plus le taux
//!   AT/MP que la ligne porte (S21.G00.23.003) VALORISE A PART : assiette x
//!   taux arrondi, plus assiette x taux AT arrondi -- 74 bordereaux sur 75 de
//!   CoolPaie au centime ainsi, 60 seulement en additionnant les taux ;
//! - format Z (versement mobilite) : l'Urssaf n'a pas de taux national, c'est
//!   la ligne qui le porte (S21.G00.23.003, le taux de l'autorite) ;
//! - format F (reductions et exonerations, un montant S21.G00.23.005) : le
//!   montant, en DEDUCTION (mai 2025 : 367, et non 382 en l'ajoutant) ;
//! - chaque ligne arrondie a l'euro (guide Urssaf v5.1, §1.6 : << 22 et 23
//!   arrondis a l'euro >>), puis la somme.
//!
//! Un code absent de la table, ou d'un format que ce module ne connait pas,
//! fait REFUSER : un ordre de telepaiement ne se devine pas (regle 1).
//!
//! La table vit a cote du code (`donnees/urssaf-codes-types-personnel.csv`, la
//! copie de la source `urssaf-codes-types-personnel` de `sources.json`) : le
//! calcul d'un mois depend de la version du logiciel, pas de la veille du
//! poste -- c'est ce qui rend le rejeu possible. ATTENTION, ses dates sont en
//! AAAA-JJ-MM (l'AGS passe a 0,25 le << 2024-01-07 >>, le 1er juillet 2024).
//! Preuves : `tests/calcul/bordereau.rs`, dont le rejeu des 75 bordereaux
//! (`tests/donnees/bordereaux-reels.json`).

use crate::date::Jour;
use crate::fichiers::{dossier_du_code, signature};
use crate::num::{div, mul, Decimal, Resultat};
use rust_decimal::RoundingStrategy;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex, OnceLock};

const NOM_TABLE: &str = "urssaf-codes-types-personnel.csv";

/// Une ligne 23, en donnee : ce que la DSN ecrit en 001 a 005. `assiette`
/// (formats A, C, D, E, K, Z) ou `montant` (format F), jamais les deux.
#[derive(Clone, Debug)]
pub struct LigneBordereau {
    pub code: String,
    pub qualifiant: String,
    /// Le taux AT/MP en texte, vide si la ligne n'en porte pas.
    pub taux_at: String,
    pub assiette: Option<Decimal>,
    pub montant: Option<Decimal>,
    pub commune: Option<String>,
}

/// Une ligne de la table : effet, fin, format, taux plafonne, taux deplafonne, libelle.
struct Ligne {
    effet: Jour,
    fin: Option<Jour>,
    format: String,
    plafonne: Option<Decimal>,
    deplafonne: Option<Decimal>,
    libelle: String,
}

type Table = HashMap<String, Vec<Ligne>>;

/// Une date de la table : AAAA-JJ-MM, sauf quand le jour depasse 12 (alors
/// AAAA-MM-JJ) ; vide, rien.
fn date_table(texte: &str) -> Resultat<Option<Jour>> {
    let texte = texte.trim();
    if texte.is_empty() {
        return Ok(None);
    }
    let p: Vec<&str> = texte.split('-').collect();
    if p.len() != 3 {
        return Err(format!("{} valeurs a decompacter, 3 attendues : {:?}", p.len(), texte));
    }
    let n = |s: &str| -> Resultat<i64> {
        s.trim().parse::<i64>().map_err(|_| format!("nombre entier illisible : {:?}", s))
    };
    let (an, x, y) = (n(p[0])?, n(p[1])?, n(p[2])?);
    let (mois, jour) = if y > 12 { (x, y) } else { (y, x) };
    let j = if (1..=9999).contains(&an) && (1..=12).contains(&mois) && (1..=31).contains(&jour) {
        Jour::nouveau(an as i32, mois as u32, jour as u32)
    } else {
        None
    };
    j.map(Some).ok_or_else(|| crate::date::date_impossible(an, mois, jour))
}

fn taux(texte: &str) -> Resultat<Option<Decimal>> {
    let texte = texte.trim();
    if texte.is_empty() {
        return Ok(None);
    }
    Decimal::from_str(texte).map(Some).map_err(|_| crate::num::illisible(texte))
}

/// Les enregistrements d'un CSV a point-virgule (guillemets doubles admis).
fn enregistrements(texte: &str) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    let mut rec: Vec<String> = Vec::new();
    let mut champ = String::new();
    let mut guillemets = false;
    let mut vu = false;
    let mut it = texte.chars().peekable();
    while let Some(c) = it.next() {
        if guillemets {
            if c == '"' {
                if it.peek() == Some(&'"') {
                    champ.push('"');
                    it.next();
                } else {
                    guillemets = false;
                }
            } else {
                champ.push(c);
            }
            continue;
        }
        match c {
            '"' if champ.is_empty() => {
                guillemets = true;
                vu = true;
            }
            ';' => {
                rec.push(std::mem::take(&mut champ));
                vu = true;
            }
            '\r' | '\n' => {
                if c == '\r' && it.peek() == Some(&'\n') {
                    it.next();
                }
                if vu || !champ.is_empty() {
                    rec.push(std::mem::take(&mut champ));
                    out.push(std::mem::take(&mut rec));
                }
                vu = false;
            }
            c => {
                champ.push(c);
                vu = true;
            }
        }
    }
    if vu || !champ.is_empty() {
        rec.push(champ);
        out.push(rec);
    }
    out
}

/// `_lire` : code -> lignes de la table.
fn lire(chemin: &Path) -> Resultat<Table> {
    let octets = std::fs::read(chemin).map_err(|e| format!("{} : {}", chemin.display(), e))?;
    let texte = String::from_utf8(octets).map_err(|e| format!("{} : {}", chemin.display(), e))?;
    let texte = texte.strip_prefix('\u{feff}').unwrap_or(&texte);
    let mut recs = enregistrements(texte).into_iter();
    let entete = recs.next().unwrap_or_default();
    let col = |nom: &str| entete.iter().position(|c| c == nom);
    let (c_code, c_effet, c_fin, c_fmt) = (col("Code"), col("Date d'effet"), col("Date de fin"), col("Format"));
    let (c_pl, c_dep, c_lib) = (col("Taux plafonné"), col("Taux déplafonné"), col("Libellé"));
    let mut table = Table::new();
    for r in recs {
        let v = |c: Option<usize>| c.and_then(|i| r.get(i)).map(String::as_str).unwrap_or("");
        let code = v(c_code).trim().trim_matches('\'').to_string();
        if code.is_empty() {
            continue;
        }
        let Some(effet) = date_table(v(c_effet))? else { continue };
        let ligne = Ligne {
            effet,
            fin: date_table(v(c_fin))?,
            format: v(c_fmt).trim().to_string(),
            plafonne: taux(v(c_pl))?,
            deplafonne: taux(v(c_dep))?,
            libelle: v(c_lib).to_string(),
        };
        table.entry(code).or_default().push(ligne);
    }
    Ok(table)
}

/// La table, lue une fois tant que le fichier ne change pas.
fn table() -> Resultat<Arc<Table>> {
    type Cache = Mutex<HashMap<PathBuf, ((u128, u64), Arc<Table>)>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let chemin = dossier_du_code().join(NOM_TABLE);
    let sig = signature(&chemin);
    if let Some(s) = sig {
        if let Some((vue, t)) = cache.lock().unwrap().get(&chemin) {
            if *vue == s {
                return Ok(t.clone());
            }
        }
    }
    let t = Arc::new(lire(&chemin)?);
    if let Some(s) = sig {
        cache.lock().unwrap().insert(chemin, (s, t.clone()));
    }
    Ok(t)
}

/// La ligne de la table en vigueur ce jour-la pour ce CTP : la date d'effet la
/// plus recente l'emporte, a egalite la ligne encore ouverte ; fermee a ce jour,
/// aucune.
fn ligne_de_table<'a>(t: &'a Table, code: &str, jour: &Jour) -> Option<&'a Ligne> {
    let mut meilleure: Option<&Ligne> = None;
    for l in t.get(code)?.iter().filter(|l| l.effet <= *jour) {
        let mieux = match meilleure {
            None => true,
            Some(m) => (l.effet, l.fin.is_none()) > (m.effet, m.fin.is_none()),
        };
        if mieux {
            meilleure = Some(l);
        }
    }
    meilleure.filter(|l| l.fin.map_or(true, |f| *jour < f))
}

/// L'arrondi a l'euro des montants et des assiettes declares aux organismes de securite sociale (CSS L. 133-10,
/// `atelier/reference/sources/code-de-la-securite-sociale/articles-20261013.md` : « arrondi a l'euro le plus proche. La
/// fraction d'euro egale a 0,50 est comptee pour 1 ») : la moitie vers le haut. Le seul endroit qui arrondit a l'euro, pour
/// les lignes 23 du bordereau comme pour les versements 20 et les corrections de la DSN.
pub fn euro(x: Decimal) -> Decimal {
    x.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
}

/// Le montant d'une ligne 23, a l'euro.
pub fn montant_ligne(l: &LigneBordereau, jour: &Jour) -> Resultat<Decimal> {
    let t = table()?;
    let Some(t) = ligne_de_table(&t, &l.code, jour) else {
        return Err(format!(
            "CTP {} absent de la table de l'Urssaf au {} : le montant du \
bordereau ne peut pas se calculer. Mettre a jour la table (source \
urssaf-codes-types-personnel). Rien n'est ecrit.",
            l.code,
            jour.jj_mm_aaaa()
        ));
    };
    let at = if l.taux_at.is_empty() {
        Decimal::ZERO
    } else {
        Decimal::from_str(l.taux_at.trim()).map_err(|_| crate::num::illisible(&l.taux_at))?
    };
    let cent = Decimal::ONE_HUNDRED;
    match t.format.as_str() {
        "F" => {
            let m = l.montant.ok_or_else(|| crate::fichiers::absent("montant"))?;
            Ok(-euro(m))
        }
        "Z" => {
            let a = l.assiette.ok_or_else(|| crate::fichiers::absent("assiette"))?;
            Ok(euro(div(mul(a, at), cent)))
        }
        "A" | "C" | "D" | "E" | "K" => {
            let plafonne = l.qualifiant == "921";
            let Some(tx) = (if plafonne { t.plafonne } else { t.deplafonne }) else {
                return Err(format!(
                    "CTP {} ({}), qualifiant {} : la table de l'Urssaf ne donne pas de taux {}. \
Rien n'est ecrit.",
                    l.code,
                    t.libelle,
                    l.qualifiant,
                    if plafonne { "plafonne" } else { "deplafonne" }
                ));
            };
            let a = l.assiette.ok_or_else(|| crate::fichiers::absent("assiette"))?;
            Ok(euro(div(mul(a, tx), cent)) + euro(div(mul(a, at), cent)))
        }
        fmt => Err(format!(
            "CTP {} ({}) : format {} de la table de l'Urssaf, que le moteur ne sait pas \
valoriser. Rien n'est ecrit.",
            l.code, t.libelle, fmt
        )),
    }
}

/// Le montant du bordereau (S21.G00.22.005) : la somme de ses lignes, a deux decimales.
pub fn montant(lignes: &[LigneBordereau], jour: &Jour) -> Resultat<Decimal> {
    let mut s = Decimal::ZERO;
    for l in lignes {
        s += montant_ligne(l, jour)?;
    }
    // Un zero n'a pas de signe : `Decimal("0") + Decimal("-0")` vaut `0`.
    if s.is_zero() {
        s = Decimal::ZERO;
    }
    s.rescale(2);
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une date dont le jour depasse 12 est en AAAA-MM-JJ, les autres en
    /// AAAA-JJ-MM.
    #[test]
    fn une_date_dont_le_jour_depasse_12_est_en_aaaa_mm_jj() {
        assert_eq!(date_table("2003-05-28").unwrap(), Jour::nouveau(2003, 5, 28));
        assert_eq!(date_table("2024-01-07").unwrap(), Jour::nouveau(2024, 7, 1));
    }
}
