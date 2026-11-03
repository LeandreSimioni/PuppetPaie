//! L'INDEMNITE JOURNALIERE DE MISSION A L'ETRANGER, limite d'exoneration des frais de repas et de logement d'un salarie
//! en grand deplacement a l'etranger : « la fraction qui n'excède pas le taux de l'indemnité journalière pour les
//! déplacements temporaires à l'étranger des personnels civils de l'État », fixe par l'annexe de l'arrete du 3 juillet
//! 2006 (BOSS, Frais professionnels, § 1380-1390,
//! `atelier/reference/sources/boss/boss-frais-professionnels-20261013.txt` l. 1620-1645 ; l'arrete :
//! `atelier/reference/sources/arrete-2006-indemnites-de-mission/legifrance-arrete-20060703-indemnites-de-mission-20261103.txt`).
//!
//! LES VALEURS DANS LE TEMPS se lisent dans les deux tables que la DGFiP publie (data.economie.gouv.fr,
//! `atelier/reference/sources/dgfip-frais-de-mission/`, livrees dans `logiciel/donnees/missions/`) :
//! - `indemnites.csv` : par pays (ou ville, `villes.json`), le montant journalier en monnaie locale (`groupe_1`) et la
//!   date d'effet (`date`) ; le montant d'un jour est celui de la ligne la plus recente dont la date ne le passe pas
//!   (une periode saisonniere, New York ou l'Afrique du Sud, est une suite de lignes datees) ;
//! - `change.csv` : le taux de chancellerie d'une monnaie pour un pays (`taux`, en euros pour une unite), date de meme.
//!   INFERENCE : la conversion en euros se fait au taux de chancellerie en vigueur le jour de la mission (aucun texte
//!   depose ne dit le taux ; la page Urssaf renvoie au site des taux de chancellerie de la DGFiP,
//!   `atelier/reference/sources/urssaf-frais-professionnels/urssaf-frais-professionnels-20261030.txt` l. 913-914).
//!
//! UNE PAIE NE DEPEND QUE DE LA VALEUR DE SES JOURS (`Indemnite::empreinte`) : une table qui grossit de lignes nouvelles
//! ne change rien a une paie passee. Les tables se lisent hors du registre du calcul, et l'engagement note la valeur
//! prise (`engagement::empreinte`, l'entree `mission <code> <jour>`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex, OnceLock};

use serde_json::Value;
use socle::date::Jour;
use socle::fichiers::dossier_du_code;
use socle::num::{Decimal, Resultat};

/// Le dossier des tables, sous les donnees livrees.
const DOSSIER: &str = "missions";

/// Une ligne datee d'une table : sa date d'effet, et ce qu'elle dit.
#[derive(Debug, Clone)]
struct Datee<T> {
    depuis: Jour,
    valeur: T,
}

/// Les deux tables lues, et les noms des lieux.
#[derive(Debug, Default)]
struct Tables {
    /// code -> (nom de la table, lignes datees (monnaie, montant)), du plus ancien au plus recent.
    indemnites: HashMap<String, (String, Vec<Datee<(String, Decimal)>>)>,
    /// (monnaie, code) -> taux de chancellerie dates.
    change: HashMap<(String, String), Vec<Datee<Decimal>>>,
    /// code de ville -> (nom, pays).
    villes: HashMap<String, (String, String)>,
}

/// L'indemnite d'un jour, en euros, et d'ou elle vient.
#[derive(Debug, Clone, PartialEq)]
pub struct Indemnite {
    pub euros: Decimal,
    pub montant: Decimal,
    pub monnaie: String,
    pub change: Decimal,
}

impl Indemnite {
    /// Ce que l'empreinte d'une paie retient de l'indemnite d'un jour.
    pub fn empreinte(&self) -> String {
        format!("{} {} {} {}", self.euros, self.montant, self.monnaie, self.change)
    }
}

fn champs(ligne: &str) -> Vec<&str> {
    ligne.trim_end_matches(['\r', '\n']).trim_start_matches('\u{feff}').split(';').map(str::trim).collect()
}

fn decimal(t: &str) -> Option<Decimal> {
    Decimal::from_str(&t.replace(',', ".")).ok()
}

fn lire_texte(nom: &str) -> Resultat<String> {
    let p = dossier_du_code().join(DOSSIER).join(nom);
    std::fs::read_to_string(&p).map_err(|e| socle::echec::du_disque(&p, &e))
}

fn construire() -> Resultat<Tables> {
    construire_de(&lire_texte("indemnites.csv")?, &lire_texte("change.csv")?, &lire_texte("villes.json")?)
}

/// Les tables, depuis leurs textes.
fn construire_de(indemnites: &str, change: &str, villes: &str) -> Resultat<Tables> {
    let mut t = Tables::default();
    let texte = indemnites;
    let mut lignes = texte.lines();
    let tete = champs(lignes.next().unwrap_or_default());
    let col = |nom: &str| tete.iter().position(|c| *c == nom).ok_or_else(|| socle::echec::livree("missions/indemnites.csv", nom));
    let (c_code, c_nom, c_monnaie, c_date, c_montant) = (col("code_pays")?, col("nom_pays")?, col("code_monnaie")?, col("date")?, col("groupe_1")?);
    for l in lignes.filter(|l| !l.trim().is_empty()) {
        let c = champs(l);
        let (Some(code), Some(date), Some(montant)) = (c.get(c_code), c.get(c_date).and_then(|d| Jour::lire(d).ok()), c.get(c_montant).and_then(|m| decimal(m))) else { continue };
        let e = t.indemnites.entry(code.to_string()).or_insert_with(|| (String::new(), Vec::new()));
        e.1.push(Datee { depuis: date, valeur: (c.get(c_monnaie).unwrap_or(&"").to_string(), montant) });
        e.1.sort_by_key(|d| d.depuis);
        if e.1.last().is_some_and(|d| d.depuis == date) {
            e.0 = c.get(c_nom).unwrap_or(&"").to_string();
        }
    }
    let texte = change;
    let mut lignes = texte.lines();
    let tete = champs(lignes.next().unwrap_or_default());
    let col = |nom: &str| tete.iter().position(|c| *c == nom).ok_or_else(|| socle::echec::livree("missions/change.csv", nom));
    let (c_monnaie, c_code, c_date, c_taux) = (col("monnaie_source")?, col("code_pays")?, col("date")?, col("taux")?);
    for l in lignes.filter(|l| !l.trim().is_empty()) {
        let c = champs(l);
        let (Some(m), Some(code), Some(date), Some(taux)) = (c.get(c_monnaie), c.get(c_code), c.get(c_date).and_then(|d| Jour::lire(d).ok()), c.get(c_taux).and_then(|x| decimal(x))) else { continue };
        let e = t.change.entry((m.to_string(), code.to_string())).or_default();
        e.push(Datee { depuis: date, valeur: taux });
        e.sort_by_key(|d| d.depuis);
    }
    let villes: Value = serde_json::from_str(villes).map_err(|_| socle::echec::livree("missions/villes.json", "villes"))?;
    for (code, v) in villes.get("villes").and_then(Value::as_object).into_iter().flatten() {
        let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        t.villes.insert(code.clone(), (s("nom"), s("pays")));
    }
    Ok(t)
}

/// Les tables, lues une fois tant que leurs fichiers ne changent pas. Leur lecture ne se note pas au registre du calcul.
fn tables() -> Resultat<Arc<Tables>> {
    type Garde = Mutex<Option<(PathBuf, Vec<Option<(u128, u64)>>, Arc<Tables>)>>;
    static CACHE: OnceLock<Garde> = OnceLock::new();
    let dossier = dossier_du_code().join(DOSSIER);
    let sig: Vec<_> = ["indemnites.csv", "change.csv", "villes.json"].iter().map(|f| socle::fichiers::signature(&dossier.join(f))).collect();
    let cache = CACHE.get_or_init(|| Mutex::new(None));
    if let Some((d, s, t)) = cache.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        if *d == dossier && *s == sig && sig.iter().all(Option::is_some) {
            return Ok(t.clone());
        }
    }
    let t = Arc::new(construire()?);
    *cache.lock().unwrap_or_else(|e| e.into_inner()) = Some((dossier, sig, t.clone()));
    Ok(t)
}

fn en_vigueur<T: Clone>(lignes: &[Datee<T>], jour: &Jour) -> Option<T> {
    lignes.iter().rev().find(|d| d.depuis <= *jour).map(|d| d.valeur.clone())
}

/// L'indemnite journaliere du lieu `code` (un pays, ou une ville que la table distingue) le jour `jour`, en euros
/// (arrondie au centime). Refus : un lieu que la table ne connait pas, un jour qu'elle ne couvre pas, une monnaie sans
/// taux de chancellerie a ce jour (rien ne se devine).
pub fn du_jour(code: &str, jour: &Jour) -> Resultat<Indemnite> {
    let t = tables()?;
    let (nom, lignes) = t.indemnites.get(code).ok_or_else(|| format!("« {} » n'est pas un lieu de la table des indemnités de mission à l'étranger.", code))?;
    let (monnaie, montant) = en_vigueur(lignes, jour).ok_or_else(|| format!("La table des indemnités de mission ne dit rien pour {} au {}.", nom, jour.jj_mm_aaaa()))?;
    let change = if monnaie == "EUR" {
        Decimal::ONE
    } else {
        t.change
            .get(&(monnaie.clone(), code.to_string()))
            .and_then(|l| en_vigueur(l, jour))
            .ok_or_else(|| format!("La table des taux de chancellerie ne dit pas le taux de {} pour {} au {} : la limite ne se calcule pas.", monnaie, nom, jour.jj_mm_aaaa()))?
    };
    Ok(Indemnite { euros: socle::num::eur2(montant * change), montant, monnaie, change })
}

/// Les lieux a l'etranger que le choix « Lieu » d'une ligne de frais propose : `(code, nom)`, par nom. Un pays se nomme
/// comme la liste des pays du logiciel (`socle::pays`) ; une ville, par `villes.json`, suivie de son pays. Un code que ni
/// l'une ni l'autre ne nomme (un Etat disparu, une ligne d'avant 2006) n'est pas propose.
pub fn lieux() -> Resultat<Vec<(String, String)>> {
    let t = tables()?;
    let mut out = Vec::new();
    for code in t.indemnites.keys() {
        let nom = match t.villes.get(code) {
            Some((ville, pays)) => socle::pays::du_code(pays)?.map(|p| format!("{} ({})", ville, p.nom)),
            None => socle::pays::du_code(code)?.map(|p| p.nom),
        };
        if let Some(n) = nom {
            out.push((code.clone(), n));
        }
    }
    out.sort_by_key(|(_, n)| socle::texte::sans_accent(n).to_lowercase());
    Ok(out)
}

/// Le nom d'un lieu de la table, tel que `lieux` le propose.
pub fn nom(code: &str) -> Resultat<Option<String>> {
    Ok(lieux()?.into_iter().find(|(c, _)| c == code).map(|(_, n)| n))
}

/// DEUX TABLES RELEVEES, relues comme le moteur les lit (`moteur releve-missions`) : le nombre de lieux de la table des
/// indemnites et de taux de chancellerie, ou ce qui ne se lit pas. Une table vide refuse.
pub fn verifier(indemnites: &str, change: &str) -> Resultat<(usize, usize)> {
    let t = construire_de(indemnites, change, &lire_texte("villes.json")?)?;
    let taux: usize = t.change.values().map(Vec::len).sum();
    if t.indemnites.is_empty() || taux == 0 {
        return Err("Une table relevée n'a aucune ligne lisible : rien n'est remplacé.".to_string());
    }
    Ok((t.indemnites.len(), taux))
}
