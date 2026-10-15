//! Le catalogue des rubriques (`moteur/catalogue.py`) : ce que le calcul lit.
//!
//! Trois fichiers : `donnees/rubriques-base.json` (le logiciel),
//! `config/rubriques-compagnie.json` et `config/rubriques-actives.json` (les
//! donnees, sous la racine de la compagnie). Les refus gardent les messages
//! du Python.
//!
//! Non porte : `CHAMPS`, `Champ`, `non_appliques`, `verifier_les_champs` (de
//! l'ecran et de l'audit, pas du calcul), `Rubrique.jours` (qui ne fait que
//! refuser).

use crate::fichiers::{dossier_du_code, lire_json, repr, signature, vrai};
use crate::num::Resultat;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

/// Les unites que le moteur sait compter.
pub const UNITES: [&str; 8] =
    ["cachet", "heure", "jour", "semaine", "mois", "service", "forfait", "pourcentage"];

/// Les natures juridiques. Elles ne commandent pas l'assujettissement (`soumis`, `imposable` le disent, rubrique par
/// rubrique) ; d'une ligne HORS assiette, elles disent ce qu'elle est pour le montant net social
/// (`Rubrique::hors_assiette`).
pub const NATURES: [&str; 7] =
    ["salaire", "frais", "avantage_nature", "indemnite", "prime", "retenue", "absence"];

/// Ce qu'est une ligne hors assiette (`soumis` faux) pour le montant net social (S21.G00.58.004, type 03). Decret
/// n° 2023-1378 du 28/12/2023 : CSS R. 844-1 II (atelier/reference/sources/code-de-la-securite-sociale/
/// articles-20261010.md), C. trav. R. 3243-1, 9° bis ; FAQ du BOSS, Q 2.2, 2.11 et 2.12 (atelier/reference/sources/
/// montant-net-social/LISEZ-MOI.md, et la FAQ reproduite a cote).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HorsAssiette {
    /// Un remboursement de frais professionnels dans ses limites d'exoneration : n'y entre pas (Q 2.12).
    Frais,
    /// Une somme versee a l'occasion du travail, exoneree ou non (indemnites de rupture, Q 2.2) : y entre.
    Revenu,
    /// Un mouvement de tresorerie (acompte, avance, saisie) : pas un revenu, n'y entre pas (Q 2.12).
    Mouvement,
}

/// Les cinq avantages en nature du bloc 54.
pub const AVANTAGES_EN_NATURE: [&str; 5] = ["02", "03", "04", "05", "06"];

pub const COMPTE_POUR: [&str; 2] = ["droits_et_montant", "ouverture_des_droits"];

/// Les valeurs de S21.G00.54.001 (cahier technique NEODeS 2026.1).
pub const AUTRES_BRUTS: &[(&str, &str)] = &[
    ("01", "Somme versee par un tiers"),
    ("02", "Avantage en nature : repas"),
    ("03", "Avantage en nature : logement"),
    ("04", "Avantage en nature : vehicule"),
    ("05", "Avantage en nature : NTIC"),
    ("06", "Avantage en nature : autres"),
    ("07", "Frais professionnels rembourses au forfait"),
    ("08", "Frais professionnels pris en charge par l'employeur"),
    ("09", "Frais professionnels rembourses au reel"),
    ("10", "Deduction forfaitaire specifique"),
    ("11", "Participation y compris supplement"),
    ("12", "Interessement y compris supplement"),
    ("14", "Abondement au plan d'epargne entreprise (PEE)"),
    ("15", "Abondement au plan d'epargne interentreprises (PEI)"),
    ("16", "Abondement a un plan d'epargne pour la retraite collectif"),
    ("17", "Participation patronale au financement des titres-restaurant"),
    ("18", "Participation patronale aux frais de transports publics"),
    ("19", "Participation patronale aux frais de transports personnels"),
    ("25", "Droit d'auteur"),
    ("26", "Droit de doublage"),
    ("27", "Droit de rediffusion"),
    ("31", "Avantages de preretraite verses par l'employeur"),
    ("33", "Sommes d'un CET reaffectees a un plan d'epargne retraite"),
    ("34", "[FP] Prestations familiales"),
    ("35", "Frais professionnels reintegres dans l'assiette (DFS)"),
    ("36", "Sommes provenant d'un CET converties en points RAFP"),
    ("37", "Participation et interessement verses immediatement"),
    ("50", "Potentiel nouveau type A"),
    ("51", "Potentiel nouveau type B"),
    ("90", "Participation au financement des services a la personne"),
    ("91", "Participation de l'employeur aux cheques vacances"),
    ("92", "Cotisation patronale frais de sante"),
    ("93", "Cotisation prevoyance et retraite supplementaire"),
    ("94", "Potentiel nouveau type C"),
    ("95", "Potentiel nouveau type D"),
    ("96", "Potentiel nouveau type E"),
];

/// Les champs d'une rubrique (`Rubrique.__dataclass_fields__`) : tout autre
/// champ dans un fichier est refuse.
const CHAMPS_CONNUS: [&str; 28] = [
    "code", "libelle", "unite", "nature", "saisie", "soumis", "imposable",
    "conges_spectacles", "source", "sens", "heures_forfait", "plafond", "aem",
    "dsn_type", "dsn_autres_bruts", "annexe", "plafond_mensuel",
    "plafond_annuel_heures", "compte_pour", "salaire_de_base", "retabli",
    "suspension", "arret_de_travail", "conges_pris", "dsn_bloc52", "origine",
    "usages", "active",
];

/// Une forme de paie : ce que le bulletin appelle une ligne.
#[derive(Debug, Clone, PartialEq)]
pub struct Rubrique {
    pub code: String,
    pub libelle: String,
    pub unite: String,
    pub nature: String,
    pub saisie: Vec<String>,
    pub soumis: bool,
    pub imposable: bool,
    pub conges_spectacles: bool,
    pub source: String,
    pub sens: String,
    pub heures_forfait: Option<f64>,
    pub plafond: Option<String>,
    pub aem: Option<String>,
    pub dsn_type: Option<String>,
    pub dsn_autres_bruts: Option<String>,
    pub annexe: Option<String>,
    pub plafond_mensuel: Option<i64>,
    pub plafond_annuel_heures: Option<Value>,
    pub compte_pour: String,
    pub salaire_de_base: bool,
    pub retabli: bool,
    pub suspension: bool,
    pub arret_de_travail: bool,
    pub conges_pris: bool,
    pub dsn_bloc52: Option<String>,
    pub origine: String,
    pub usages: i64,
    pub active: bool,
}

fn joindre(l: &[&str]) -> String {
    l.join(", ")
}

impl Rubrique {
    /// `__post_init__` : les refus d'une rubrique mal formee.
    fn verifier(&self) -> Resultat<()> {
        let code = &self.code;
        if !UNITES.contains(&self.unite.as_str()) {
            return Err(format!(
                "Rubrique {} : unite {} inconnue. Connues : {}.",
                repr(code), repr(&self.unite), joindre(&UNITES)));
        }
        if !NATURES.contains(&self.nature.as_str()) {
            return Err(format!(
                "Rubrique {} : nature {} inconnue. Connues : {}.",
                repr(code), repr(&self.nature), joindre(&NATURES)));
        }
        if !COMPTE_POUR.contains(&self.compte_pour.as_str()) {
            return Err(format!(
                "Rubrique {} : compte_pour {} inconnu. Connus : {}.",
                repr(code), repr(&self.compte_pour), joindre(&COMPTE_POUR)));
        }
        if self.source.is_empty() {
            return Err(format!(
                "Rubrique {} : `source` est vide. Une rubrique dit d'ou vient \
                 sa regle, sinon personne ne pourra la verifier.", repr(code)));
        }
        if self.conges_spectacles && !self.soumis {
            return Err(format!(
                "Rubrique {} : `conges_spectacles` sans `soumis`. Les Conges \
                 Spectacles sont une cotisation : leur assiette ne peut pas \
                 depasser ce qui entre dans l'assiette des cotisations.", repr(code)));
        }
        if self.heures_forfait.is_some() && self.unite == "heure" {
            return Err(format!(
                "Rubrique {} : `heures_forfait` sur une rubrique dont l'unite \
                 est l'heure. Les heures y sont un fait, pas un forfait : \
                 elles compteraient deux fois.", repr(code)));
        }
        if let Some(c) = &self.dsn_autres_bruts {
            if !AUTRES_BRUTS.iter().any(|(k, _)| k == c) {
                return Err(format!(
                    "Rubrique {} : `dsn_autres_bruts` vaut {}, qui n'est pas une \
                     valeur de S21.G00.54.001. Voir AUTRES_BRUTS, releve dans le \
                     cahier technique NEODeS 2026.1.", repr(code), repr(c)));
            }
        }
        Ok(())
    }

    /// Ce qu'une ligne de cette rubrique, hors assiette, est pour le montant net social : la nature le dit
    /// (`frais`, `indemnite`, `retenue`). Une autre nature ne le dit pas d'une ligne hors assiette (une prime, un
    /// salaire exoneres sont-ils un revenu ? un avantage en nature hors assiette, quoi ?) : refus, regle 1.
    pub fn hors_assiette(&self) -> Resultat<HorsAssiette> {
        match self.nature.as_str() {
            "frais" => Ok(HorsAssiette::Frais),
            "indemnite" => Ok(HorsAssiette::Revenu),
            "retenue" => Ok(HorsAssiette::Mouvement),
            autre => Err(format!(
                "Rubrique {} : hors assiette (`soumis` faux) et de nature {}. Le montant net social \
                 (S21.G00.58.004, bulletin) ne sait pas si cette ligne est un revenu, un remboursement de frais \
                 ou un mouvement de tresorerie, et ne le devine pas. Hors assiette, la nature doit etre `frais` \
                 (remboursement de frais professionnels : hors du montant net social), `indemnite` (une somme \
                 versee a l'occasion du travail, exoneree ou non : dans le montant net social) ou `retenue` \
                 (acompte, avance, saisie : hors du montant net social).",
                repr(&self.code), repr(autre))),
        }
    }

    /// `Rubrique.reservee_a` : `"mensuelle"`, `"activite"` ou rien.
    pub fn reservee_a(&self) -> Option<&'static str> {
        if self.salaire_de_base
            || self.retabli
            || self.suspension
            || self.arret_de_travail
            || self.conges_pris
            || self.dsn_bloc52.as_deref().map_or(false, |s| !s.is_empty())
        {
            return Some("mensuelle");
        }
        if self.unite == "cachet" {
            return Some("activite");
        }
        None
    }

    /// Les heures que cette ligne fait compter, au total : le forfait
    /// l'emporte, sinon les heures saisies, sinon la quantite si l'unite est
    /// l'heure.
    ///
    /// UNE convention : le `heures` d'une ligne de paie, comme
    /// `heures_forfait`, compte les heures PAR UNITE, et la quantite les
    /// multiplie. 14 services de 4 h : `quantite` 14, `heures` 4, soit 56 h.
    /// Un mois de permanent : `quantite` = la fraction du mois payee,
    /// `heures` = la quotite du contrat (`conges::lignes_du_mois`). Le produit
    /// se compte au centieme d'heure, comme la DSN l'ecrit.
    pub fn heures(&self, quantite: f64, heures_saisies: Option<f64>) -> f64 {
        let centieme = |x: f64| (x * 100.0).round() / 100.0;
        if let Some(f) = self.heures_forfait {
            return centieme(f * quantite);
        }
        if let Some(h) = heures_saisies {
            return centieme(h * quantite);
        }
        if self.unite == "heure" {
            return quantite;
        }
        0.0
    }
}

// ------------------------------------------------------------------- lecture

/// `_lire` : un fichier absent est un dictionnaire vide.
fn lire(p: &Path) -> Resultat<Value> {
    if !crate::fichiers::existe(p) {
        return Ok(Value::Object(Map::new()));
    }
    lire_json(p)
}

fn base_json() -> Resultat<Value> {
    lire(&dossier_du_code().join("rubriques-base.json"))
}

/// `catalogue.annexes()` : seuil et fenetre des annexes VIII et X.
pub fn annexes() -> Resultat<Value> {
    let b = base_json()?;
    let a = b.get("annexes").cloned().unwrap_or(Value::Object(Map::new()));
    let ok = |c: &str| a.get(c).map_or(false, vrai);
    if !ok("seuil_heures") || !ok("fenetre_jours") {
        return Err("rubriques-base.json : la section `annexes` \
                    (seuil_heures, fenetre_jours) manque."
            .into());
    }
    Ok(a)
}

/// `d["cle"]` : un `KeyError` Python se lit `'cle'`.
fn requis<'a>(d: &'a Map<String, Value>, cle: &str) -> Resultat<&'a Value> {
    d.get(cle).ok_or_else(|| crate::fichiers::absent(cle))
}

fn chaine_opt(d: &Map<String, Value>, cle: &str) -> Option<String> {
    match d.get(cle) {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.clone()),
        Some(autre) => Some(autre.to_string()),
    }
}

fn entier(v: &Value) -> i64 {
    // int(x) de Python pour un nombre ou une chaine.
    match v {
        Value::Number(n) => n.as_i64().unwrap_or_else(|| n.as_f64().unwrap_or(0.0) as i64),
        Value::String(s) => s.trim().parse().unwrap_or(0),
        Value::Bool(b) => *b as i64,
        _ => 0,
    }
}

fn construire(code: &str, d: &Map<String, Value>, origine: &str, usages: i64, active: bool)
    -> Resultat<Rubrique>
{
    let mut inconnus: Vec<&str> = d
        .keys()
        .map(|k| k.as_str())
        .filter(|k| !CHAMPS_CONNUS.contains(k))
        .collect();
    if !inconnus.is_empty() {
        inconnus.sort();
        return Err(format!(
            "Rubrique {} : champ(s) inconnu(s) {}. Le schema est celui de \
             moteur/catalogue.Rubrique.", repr(code), inconnus.join(", ")));
    }
    let texte = |v: &Value| match v {
        Value::String(s) => s.clone(),
        autre => autre.to_string(),
    };
    let libelle = texte(requis(d, "libelle")?);
    let unite = texte(requis(d, "unite")?);
    let nature = texte(requis(d, "nature")?);
    let saisie = d
        .get("saisie")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().map(texte).collect())
        .unwrap_or_default();
    let soumis = vrai(requis(d, "soumis")?);
    let imposable = vrai(requis(d, "imposable")?);
    let conges_spectacles = vrai(requis(d, "conges_spectacles")?);
    let source = match requis(d, "source")? {
        Value::String(s) => s.clone(),
        v if vrai(v) => v.to_string(),
        _ => String::new(),
    };
    let drapeau = |c: &str| d.get(c).map_or(false, vrai);
    let r = Rubrique {
        code: code.to_string(),
        libelle,
        unite,
        nature,
        saisie,
        soumis,
        imposable,
        conges_spectacles,
        source,
        sens: chaine_opt(d, "sens").unwrap_or_else(|| "+".into()),
        heures_forfait: d.get("heures_forfait").and_then(|v| v.as_f64()),
        plafond: chaine_opt(d, "plafond"),
        aem: chaine_opt(d, "aem"),
        dsn_type: chaine_opt(d, "dsn_type"),
        dsn_autres_bruts: chaine_opt(d, "dsn_autres_bruts"),
        annexe: chaine_opt(d, "annexe"),
        plafond_mensuel: d.get("plafond_mensuel").filter(|v| !v.is_null()).map(entier),
        plafond_annuel_heures: d.get("plafond_annuel_heures").filter(|v| !v.is_null()).cloned(),
        compte_pour: chaine_opt(d, "compte_pour").unwrap_or_else(|| "droits_et_montant".into()),
        salaire_de_base: drapeau("salaire_de_base"),
        retabli: drapeau("retabli"),
        suspension: drapeau("suspension"),
        arret_de_travail: drapeau("arret_de_travail"),
        conges_pris: drapeau("conges_pris"),
        dsn_bloc52: chaine_opt(d, "dsn_bloc52"),
        origine: origine.to_string(),
        usages,
        active,
    };
    r.verifier()?;
    Ok(r)
}

fn construire_catalogue(racine: &Path) -> Resultat<Vec<Rubrique>> {
    let b = base_json()?;
    // code -> (champs, propre a la compagnie)
    let mut brut: Vec<(String, Map<String, Value>, bool)> = Vec::new();
    if let Some(Value::Object(m)) = b.get("rubriques") {
        for (c, v) in m {
            brut.push((c.clone(), v.as_object().cloned().unwrap_or_default(), false));
        }
    }
    let mut choix: Vec<String> = Vec::new();
    let mut usages = Map::new();
    if crate::fichiers::est_dossier(racine) {
        let propres = lire(&racine.join("config").join("rubriques-compagnie.json"))?;
        if let Some(Value::Object(m)) = propres.get("rubriques") {
            for (c, v) in m {
                let v = v.as_object().cloned().unwrap_or_default();
                if let Some(e) = brut.iter_mut().find(|e| &e.0 == c) {
                    // ajustement d'une rubrique de base
                    for (k, x) in v {
                        e.1.insert(k, x);
                    }
                } else {
                    brut.push((c.clone(), v, true));
                }
            }
        }
        let actives = lire(&racine.join("config").join("rubriques-actives.json"))?;
        if let Some(Value::Array(a)) = actives.get("actives") {
            choix = a.iter().filter_map(|x| x.as_str().map(String::from)).collect();
        }
        if let Some(Value::Object(u)) = actives.get("usages") {
            usages = u.clone();
        }
    }
    let mut out = Vec::new();
    for (code, mut d, propre) in brut {
        d.remove("_propre");
        let u = usages.get(&code).map_or(0, entier);
        out.push(construire(
            &code,
            &d,
            if propre { "compagnie" } else { "base" },
            u,
            !choix.is_empty() && choix.contains(&code),
        )?);
    }
    out.sort_by(|a, b| {
        (!a.active, -a.usages, a.libelle.to_lowercase())
            .cmp(&(!b.active, -b.usages, b.libelle.to_lowercase()))
    });
    Ok(out)
}

fn empreinte(p: &Path) -> String {
    format!("{}:{:?}", p.display(), signature(p))
}

static CATALOGUES: OnceLock<Mutex<HashMap<String, Arc<Vec<Rubrique>>>>> = OnceLock::new();

/// Les rubriques disponibles : les actives d'abord (la plus utilisee en tete),
/// puis les autres par libelle. `racine` est la racine des donnees de la
/// compagnie (`ctx.racine`). Construit une fois par etat des trois fichiers.
pub fn catalogue(racine: &Path) -> Resultat<Arc<Vec<Rubrique>>> {
    let cle = format!(
        "{}|{}|{}",
        empreinte(&dossier_du_code().join("rubriques-base.json")),
        empreinte(&racine.join("config").join("rubriques-compagnie.json")),
        empreinte(&racine.join("config").join("rubriques-actives.json")),
    );
    let cache = CATALOGUES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(c) = cache.lock().unwrap().get(&cle) {
        // Un catalogue servi par le cache a lu ses fichiers quand meme, pour l'empreinte du calcul.
        crate::fichiers::est_dossier(racine);
        for p in [
            dossier_du_code().join("rubriques-base.json"),
            racine.join("config").join("rubriques-compagnie.json"),
            racine.join("config").join("rubriques-actives.json"),
        ] {
            if crate::fichiers::existe(&p) {
                crate::fichiers::noter_si_besoin(&p);
            }
        }
        return Ok(c.clone());
    }
    let c = Arc::new(construire_catalogue(racine)?);
    let mut g = cache.lock().unwrap();
    if g.len() > 64 {
        g.clear();
    }
    g.insert(cle, c.clone());
    Ok(c)
}

/// Une rubrique par son code. Refuse un code inconnu : mieux vaut s'arreter
/// que payer une ligne dont on ignore le regime.
pub fn rubrique(racine: &Path, code: &str) -> Resultat<Rubrique> {
    let cat = catalogue(racine)?;
    if let Some(r) = cat.iter().find(|r| r.code == code) {
        return Ok(r.clone());
    }
    let mut connues: Vec<&str> = cat.iter().map(|r| r.code.as_str()).collect();
    connues.sort();
    Err(format!(
        "Rubrique inconnue : {}. Connues : {}.", repr(code), connues.join(", ")))
}
