//! Les donnees de la compagnie, lues une fois a une date : les fiches (entreprise, etablissements, salaries,
//! spectacles), l'effectif, et le bareme de l'annee empile (le socle livre, la convention, la surcouche de la
//! compagnie) puis lu a la date.
//!
//! Les fiches et les baremes restent des `serde_json::Value`. Un refus est une `String`.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, OnceLock};
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use socle::date::{d, en_texte, forme_ecrite, repr_texte, Jour};
use socle::fichiers::{dossier_du_code, vrai};
use socle::num::{flottant, Resultat};
use socle::texte::{cle_recherche, sigle};

// ------------------------------------------------------------ les petits outils

fn vrai_opt(v: Option<&Value>) -> bool {
    v.map_or(false, vrai)
}

/// `str(x or "")`.
fn texte_ou_vide(v: Option<&Value>) -> String {
    match v {
        Some(x) if vrai(x) => en_texte(x),
        _ => String::new(),
    }
}

/// La valeur de `cle`, exigee : son absence se refuse en la nommant.
fn exiger<'a>(v: &'a Value, cle: &str) -> Resultat<&'a Value> {
    v.get(cle).ok_or_else(|| socle::fichiers::absent(cle))
}

/// `str(x["cle"])`.
fn exiger_texte(v: &Value, cle: &str) -> Resultat<String> {
    Ok(en_texte(exiger(v, cle)?))
}

/// `int(x)` : le texte, le flottant tronque, le booleen.
fn entier(v: &Value) -> Resultat<i64> {
    match v {
        Value::Number(n) => n
            .as_i64()
            .or_else(|| n.as_f64().filter(|x| x.is_finite()).map(|x| x.trunc() as i64))
            .ok_or_else(|| socle::fichiers::entier_hors_limites(&n.to_string())),
        Value::String(s) => s
            .trim()
            .parse::<i64>()
            .map_err(|_| format!("nombre entier illisible : {}", repr_texte(s))),
        Value::Bool(b) => Ok(i64::from(*b)),
        autre => Err(socle::fichiers::attendu("un nombre entier", socle::valeur::nom_type(autre))),
    }
}

/// `"%g" % x`.
pub fn format_g(x: f64) -> String {
    if x == 0.0 {
        return "0".into();
    }
    if !x.is_finite() {
        return if x.is_nan() { "nan" } else if x > 0.0 { "inf" } else { "-inf" }.into();
    }
    let t = format!("{:.5e}", x);
    let (m, e) = t.split_once('e').unwrap();
    let e: i32 = e.parse().unwrap();
    let rogner = |s: String| -> String {
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            s
        }
    };
    if !(-4..6).contains(&e) {
        format!("{}e{}{:02}", rogner(m.to_string()), if e < 0 { '-' } else { '+' }, e.abs())
    } else {
        rogner(format!("{:.*}", (5 - e) as usize, x))
    }
}

/// Un nombre a virgule, sans zeros inutiles.
fn nombre_fr(x: f64) -> String {
    format_g(x).replace('.', ",")
}

/// Lu une fois tant que le fichier ne change pas (date, taille) : le calcul
/// d'un mois relit les memes fichiers des milliers de fois.
fn lire_json(p: &Path) -> Resultat<Value> {
    type Cache = Mutex<HashMap<PathBuf, ((u128, u64), Value)>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let sig = socle::fichiers::signature(p);
    if let Some(s) = sig {
        if let Some((vue, v)) = cache.lock().unwrap().get(p) {
            if *vue == s {
                socle::fichiers::noter_si_besoin(p);
                return Ok(v.clone());
            }
        }
    }
    let v = lire_json_sans_cache(p)?;
    if let Some(s) = sig {
        cache.lock().unwrap().insert(p.to_path_buf(), (s, v.clone()));
    }
    Ok(v)
}

fn lire_json_sans_cache(p: &Path) -> Resultat<Value> {
    let texte = socle::fichiers::lire_texte(p).map_err(|e| {
        socle::fichiers::panne_fichier(&e, p)
    })?;
    serde_json::from_str(&texte).map_err(|e| format!("{} ({})", e, p.display()))
}

fn objet(v: Value, nom: &str) -> Resultat<Map<String, Value>> {
    match v {
        Value::Object(o) => Ok(o),
        _ => Err(format!("{} : un objet JSON est attendu", nom)),
    }
}

// ------------------------------------------------- la recherche floue

/// La distance de Levenshtein, sur les caracteres.
fn levenshtein(a: &str, b: &str) -> usize {
    if a == b {
        return 0;
    }
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut prec: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cour = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            cour.push((prec[j + 1] + 1).min(cour[j] + 1).min(prec[j] + usize::from(ca != cb)));
        }
        prec = cour;
    }
    prec[b.len()]
}

// ------------------------------------------------------------ les fiches

/// `actif: false`. L'absence de la cle vaut actif.
pub fn archivee(fiche: &Value) -> bool {
    fiche.get("actif") == Some(&Value::Bool(false))
}

// ------------------------------------------------- la configuration datee

/// LA CONFIGURATION DE LA COMPAGNIE EST DATEE, comme les baremes : chaque valeur d'une fiche (l'entreprise et sa
/// convention, ses etablissements, un salarie, un spectacle) s'ecrit en liste datee `[{"depuis": "AAAA-MM-JJ",
/// "valeur": ...}, ...]` et se lit a la date de la periode payee, jamais « telle qu'aujourd'hui » : une valeur changee
/// a une date ne touche pas ce qui precede. Les cles `_...` sont des notes, sans date. Avant la premiere valeur d'un
/// champ, la fiche ne disait rien : le champ est absent. Une valeur non datee se refuse (`non_datee`) : la date
/// d'une valeur ne se devine pas.
pub fn fiche_au(fiche: &Value, jour: &Jour, ou: &str) -> Resultat<Value> {
    let o = fiche.as_object().ok_or_else(|| format!("{} : un objet JSON est attendu.", ou))?;
    let mut out = Map::new();
    for (k, v) in o {
        if k.starts_with('_') {
            out.insert(k.clone(), v.clone());
            continue;
        }
        if !est_liste_datee(v) {
            return Err(non_datee(ou, k));
        }
        if let Some(x) = valeur_au(v, jour)? {
            out.insert(k.clone(), x.clone());
        }
    }
    Ok(Value::Object(out))
}

/// Une fiche dont aucune valeur ne vaut encore a la date : elle n'existait pas.
pub fn fiche_vide(fiche: &Value) -> bool {
    fiche.as_object().map_or(true, |o| o.keys().all(|k| k.starts_with('_')))
}

/// Le refus d'une valeur de configuration non datee, et ce qu'il faut faire.
pub fn non_datee(ou: &str, cle: &str) -> String {
    format!(
        "{}, `{}` : cette valeur n'est pas datee. La configuration de la compagnie se lit a la date de la periode payee : \
chaque valeur s'ecrit [{{\"depuis\": \"AAAA-MM-JJ\", \"valeur\": ...}}], comme les baremes. Une compagnie ecrite a \
l'ancienne forme ne se lit plus : l'ajouter a nouveau (Ajouter une compagnie) et ressaisir ses fiches, ou dater chaque \
valeur de ce fichier a la main.",
        ou, cle
    )
}

/// Les valeurs d'un champ d'une fiche datee, du plus ancien `depuis` au plus recent. Un champ absent : aucune.
pub fn valeurs_datees<'a>(fiche: &'a Value, champ: &str) -> Resultat<Vec<(Jour, &'a Value)>> {
    let Some(v) = fiche.get(champ) else { return Ok(Vec::new()) };
    if !est_liste_datee(v) {
        return Err(non_datee("fiche", champ));
    }
    let mut out = Vec::new();
    for e in v.as_array().into_iter().flatten() {
        out.push((d(&e["depuis"])?, &e["valeur"]));
    }
    out.sort_by_key(|(j, _)| *j); // stable : a meme date, la derniere ecrite gagne
    Ok(out)
}

/// Toutes les valeurs qu'un champ a portees (une liste datee), ou la valeur telle quelle : pour les gardes qui cherchent
/// dans une fiche tout ce qui nomme une personne, a quelque date que ce soit.
pub fn toutes_les_valeurs(v: Option<&Value>) -> Vec<&Value> {
    match v {
        Some(l) if est_liste_datee(l) => l.as_array().into_iter().flatten().map(|e| &e["valeur"]).collect(),
        Some(x) => vec![x],
        None => Vec::new(),
    }
}

/// ECRIRE UNE VALEUR DATEE : `valeur` vaut pour `champ` a partir de `depuis` ; ce qui precede reste ce qu'il etait. Une
/// valeur deja ecrite au meme jour est remplacee (une correction du meme jour d'effet). `note` dit pourquoi.
pub fn dater(fiche: &mut Map<String, Value>, champ: &str, valeur: Value, depuis: &Jour, note: Option<&str>) {
    let mut liste: Vec<Value> = match fiche.get(champ) {
        Some(v) if est_liste_datee(v) => v.as_array().cloned().unwrap_or_default(),
        _ => Vec::new(),
    };
    liste.retain(|e| d(&e["depuis"]).map_or(true, |j| j != *depuis));
    let mut e = Map::new();
    e.insert("depuis".into(), Value::String(depuis.iso()));
    e.insert("valeur".into(), valeur);
    if let Some(n) = note.filter(|n| !n.trim().is_empty()) {
        e.insert("note".into(), Value::String(n.trim().to_string()));
    }
    liste.push(Value::Object(e));
    liste.sort_by_key(|e| en_texte(&e["depuis"]));
    fiche.insert(champ.to_string(), Value::Array(liste));
}

/// Une fiche neuve, chaque valeur de `plate` ecrite a partir de `depuis` (les notes `_...` telles quelles).
pub fn fiche_datee(plate: &Map<String, Value>, depuis: &Jour) -> Map<String, Value> {
    let mut out = Map::new();
    for (k, v) in plate {
        if k.starts_with('_') {
            out.insert(k.clone(), v.clone());
        } else {
            dater(&mut out, k, v.clone(), depuis, None);
        }
    }
    out
}

// ------------------------------------------------ l'effectif

const CODE_SECURITE_SOCIALE: &str = "A01";
const ANNEES_CONSECUTIVES: i32 = 5;
const CLE_PASSE: &str = "jamais_au_dessus_de";

/// Les effectifs A01 connus, par annee civile (`config/effectifs.json`, que le
/// moteur ecrit depuis les retours de nature 118 ; on ne fait que le lire).
#[derive(Debug, Clone, Default)]
pub struct Effectif {
    pub annees: BTreeMap<i32, f64>,
    /// (seuil, derniere annee) : declare sous ce seuil jusqu'a cette annee.
    pub passe: Option<(i64, i64)>,
    /// L'annee de la premiere embauche (fiche entreprise) : avant elle, zero.
    pub premiere_embauche: Option<i32>,
}

/// Lit `config/effectifs.json` (absent : vide).
pub fn effectif_de(racine: &Path, fiche: &Value) -> Resultat<Effectif> {
    let p = racine.join("config").join("effectifs.json");
    let brut = if socle::fichiers::existe(&p) { lire_json(&p)? } else { Value::Object(Map::new()) };
    let mut annees = BTreeMap::new();
    if let Some(Value::Object(o)) = brut.get("annees") {
        for (an, c) in o {
            if an.is_empty() || !an.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            if let Some(a01) = c.get(CODE_SECURITE_SOCIALE) {
                annees.insert(an.parse::<i32>().map_err(|e| e.to_string())?, flottant(exiger(a01, "valeur")?)?);
            }
        }
    }
    let passe = match brut.get(CLE_PASSE) {
        Some(p) if vrai(p) => Some((entier(exiger(p, "seuil")?)?, entier(exiger(p, "jusqu_a")?)?)),
        _ => None,
    };
    Ok(Effectif { annees, passe, premiere_embauche: annee_de_premiere_embauche(fiche) })
}

/// L'annee de la premiere embauche (AAAA-MM-JJ ou AAAAMMJJ).
fn annee_de_premiere_embauche(fiche: &Value) -> Option<i32> {
    let v = texte_ou_vide(fiche.get("premiere_embauche")).trim().to_string();
    let chiffres = |s: &str| s.chars().all(|c| c.is_ascii_digit());
    let (a, m, j) = match v.len() {
        10 if v.is_ascii() && &v[4..5] == "-" && &v[7..8] == "-" => (&v[0..4], &v[5..7], &v[8..10]),
        8 if v.is_ascii() => (&v[0..4], &v[4..6], &v[6..8]),
        _ => return None,
    };
    if !(chiffres(a) && chiffres(m) && chiffres(j)) {
        return None;
    }
    Jour::nouveau(a.parse().ok()?, m.parse().ok()?, j.parse().ok()?).map(|x| x.an)
}

impl Effectif {
    pub fn annee(&self, a: i32) -> Option<f64> {
        if let Some(v) = self.annees.get(&a) {
            return Some(*v);
        }
        match self.premiere_embauche {
            Some(p) if a < p => Some(0.0),
            _ => None,
        }
    }

    /// L'effectif le plus recent connu pour payer en `annee_paie`, ou `None`.
    pub fn valeur(&self, annee_paie: i32) -> Option<f64> {
        match self.annees.range(..annee_paie).next_back() {
            Some((_, v)) => Some(*v),
            None => self.annee(annee_paie - 1),
        }
    }

    /// Le seuil est-il franchi pour payer en `annee_paie` ? (L. 130-1, II, CSS :
    /// cinq annees civiles consecutives.)
    pub fn atteint(&self, seuil: i64, annee_paie: i32) -> Resultat<bool> {
        let fenetre = annee_paie - ANNEES_CONSECUTIVES..annee_paie;
        let connues: BTreeMap<i32, f64> = fenetre.clone().filter_map(|a| self.annee(a).map(|v| (a, v))).collect();
        if connues.values().any(|v| *v < seuil as f64) {
            return Ok(false);
        }
        if let Some((s, jusqu)) = self.passe {
            if seuil >= s && fenetre.clone().any(|a| i64::from(a) <= jusqu && !connues.contains_key(&a)) {
                return Ok(false); // une annee declaree sous le seuil
            }
        }
        if connues.len() == ANNEES_CONSECUTIVES as usize {
            return Ok(true);
        }
        let manquent: Vec<String> = fenetre.filter(|a| !connues.contains_key(a)).map(|a| a.to_string()).collect();
        Err(format!(
            "Effectif de l'entreprise, pour payer en {} : {} -- le moteur ne \
sait pas si le seuil de {} salaries est franchi.\n\
\x20 Une compagnie de moins de cinq ans : renseigner la date de sa \
premiere embauche (Configuration > Entreprise), et c'est tout.\n\
\x20 Un seuil n'est franchi qu'apres CINQ annees civiles \
consecutives au-dessus (art. L. 130-1, II, CSS), et il manque \
l'effectif de {}. L'Urssaf le calcule et le renvoie en \
compte-rendu metier de nature 118 : relever les retours \
(`paie retours --depuis JJ/MM/AAAA`), le moteur \
l'ecrit seul dans config/effectifs.json.\n\
\x20 Une compagnie qui a embauche pour la premiere fois il y a moins \
de cinq ans n'a qu'a le dire : la date de sa premiere embauche \
(`premiere_embauche` de la fiche entreprise) suffit, les annees \
d'avant n'ont pas de salarie.\n\
\x20 Il ne se saisit pas et ne se devine pas : des taux en \
dependent (formation des permanents a onze salaries ; FNAL, \
reduction generale et echeance de la DSN a cinquante).",
            annee_paie,
            if connues.is_empty() {
                "rien de connu".to_string()
            } else {
                format!(
                    "connu pour {}",
                    connues.iter().map(|(a, v)| format!("{} ({})", a, nombre_fr(*v))).collect::<Vec<_>>().join(", ")
                )
            },
            seuil,
            manquent.join(", ")
        ))
    }
}

// ----------------------------------------- les baremes : socle, convention, fusion

/// Une erreur de bareme ; `introuvable` est le `FileNotFoundError` que
/// `plafond_securite_sociale` sait reformuler.
struct Refus {
    introuvable: bool,
    message: String,
}

impl From<String> for Refus {
    fn from(message: String) -> Refus {
        Refus { introuvable: false, message }
    }
}

fn introuvable(message: String) -> Refus {
    Refus { introuvable: true, message }
}

/// Les annees dont le socle est livre.
fn annees_du_socle(dossier: &Path) -> Vec<i32> {
    let mut v: Vec<i32> = socle::fichiers::lister(dossier)
        .into_iter()
        .flatten()
        .filter_map(|p| {
            if p.extension()? != "json" {
                return None;
            }
            let stem = p.file_stem()?.to_str()?;
            if stem.is_empty() || !stem.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            stem.parse().ok()
        })
        .collect();
    v.sort();
    v
}

/// Les annees disponibles, ecrites comme `", ".join(...) or "aucune"`.
fn disponibles(dossier: &Path) -> String {
    let a = annees_du_socle(dossier);
    if a.is_empty() {
        "aucune".to_string()
    } else {
        a.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ")
    }
}

/// Le socle livre de l'annee, ou `None`.
fn socle(dossier: &Path, an: i32) -> Result<Option<Value>, String> {
    let p = dossier.join(format!("{}.json", an));
    if !socle::fichiers::existe(&p) {
        return Ok(None);
    }
    lire_json(&p).map(Some)
}

/// (cle, valeur, affectation) d'une entree nommee.
fn identite(e: &Value) -> Option<(&'static str, String, String)> {
    if !e.is_object() {
        return None;
    }
    for cle in ["id", "code81"] {
        if let Some(x) = e.get(cle) {
            if !x.is_null() {
                return Some((cle, en_texte(x), texte_ou_vide(e.get("affectation"))));
            }
        }
    }
    None
}

/// Une liste de valeurs se remplace, une liste
/// d'objets NOMMES se fond (les entrees anonymes de la base sont gardees).
fn fusionner_listes(base: &[Value], dessus: &[Value]) -> Value {
    let mut sortie: Vec<Value> = base.to_vec();
    let mut rangs: Vec<((&'static str, String, String), usize)> = Vec::new();
    for (i, e) in sortie.iter().enumerate() {
        if let Some(id) = identite(e) {
            if !rangs.iter().any(|(r, _)| *r == id) {
                rangs.push((id, i));
            }
        }
    }
    if rangs.is_empty() {
        return Value::Array(dessus.to_vec());
    }
    for e in dessus {
        let rang = identite(e).and_then(|id| rangs.iter().find(|(r, _)| *r == id).map(|(_, i)| *i));
        match rang {
            None => sortie.push(e.clone()),
            Some(i) => sortie[i] = fusionner(&sortie[i], e),
        }
    }
    Value::Array(sortie)
}

/// `dessus` par-dessus `base`, en descendant.
pub fn fusionner(base: &Value, dessus: &Value) -> Value {
    match (base, dessus) {
        (Value::Object(b), Value::Object(h)) => {
            let mut out = b.clone();
            for (cle, val) in h {
                let v = match out.get(cle) {
                    Some(existant) => fusionner(existant, val),
                    None => val.clone(),
                };
                out.insert(cle.clone(), v);
            }
            Value::Object(out)
        }
        (Value::Array(b), Value::Array(h)) => fusionner_listes(b, h),
        _ => dessus.clone(),
    }
}

/// La couche de la convention collective de l'entreprise.
fn convention(socle_annee: &Value, idcc: &Value) -> Result<Value, String> {
    let par_conv = match socle_annee.get("par_convention") {
        Some(p) if vrai(p) => p,
        _ => return Ok(Value::Object(Map::new())),
    };
    let annee = texte_ou_vide_defaut(socle_annee.get("annee"), "?");
    let connues: Vec<&str> =
        par_conv.as_object().map(|o| o.keys().map(String::as_str).filter(|k| !k.starts_with('_')).collect()).unwrap_or_default();
    let idcc = texte_ou_vide(Some(idcc)).trim().to_string();
    if idcc.is_empty() {
        return Err(format!(
            "Bareme {} : la fiche entreprise ne dit pas son `idcc`, et les \
cotisations conventionnelles en dependent.\n\
\x20 Conventions couvertes : {}.\n\
\x20 A ecrire dans config/entreprise.json : \"idcc\": \"3090\".",
            annee,
            connues.join(", ")
        ));
    }
    let couche = match par_conv.get(&idcc) {
        Some(c) if c.is_object() => c,
        _ => {
            return Err(format!(
                "Bareme {} : la convention collective {} n'a pas de couche dans le \
socle, et le moteur refuse de payer avec celle d'une autre \
branche.\n\
\x20 Couvertes a ce jour : {}.\n\
\x20 Ce qui manque n'est pas du code : les cotisations propres a \
cette convention (celle qui remplace la CASC-SVP, la prevoyance \
de ses intermittents cadres), a relever puis a ajouter dans \
`par_convention` de donnees/baremes/{}.json, avec leur source.",
                annee,
                idcc,
                connues.join(", "),
                annee
            ));
        }
    };
    if couche.get("applicable") == Some(&Value::Bool(false)) {
        return Err(format!(
            "Bareme {}, convention {} ({}) : la couche est RELEVEE mais pas \
confirmee, le moteur ne paie donc pas.\n  {}",
            annee,
            idcc,
            texte_ou_vide_defaut(couche.get("libelle"), ""),
            texte_ou_vide_defaut(couche.get("_a_confirmer"), "")
        ));
    }
    Ok(match couche.get("bareme") {
        Some(b) if vrai(b) => b.clone(),
        _ => Value::Object(Map::new()),
    })
}

/// `str(x.get(cle, defaut))` : le defaut ne sert que si la cle manque (un `None` s'ecrit `None`).
fn texte_ou_vide_defaut(v: Option<&Value>, defaut: &str) -> String {
    v.map_or_else(|| defaut.to_string(), en_texte)
}

/// Le bareme de l'annee tel que le moteur le lira.
fn bareme_de_l_annee(dossier: &Path, an: i32, fichier: Option<Value>, idcc: Option<&Value>) -> Result<Value, Refus> {
    let ou = format!("config/baremes/{}.json", an);
    let fichier = match fichier {
        None => {
            if socle(dossier, an)?.is_none() {
                return Err(introuvable(format!(
                    "Pas de bareme pour {}.\n\
\x20 Le socle livre avec le logiciel couvre : {}. Un socle neuf \
s'ecrit en janvier, depuis le plus recent : SMIC, plafond de \
la Securite sociale, taux qui ont bouge (voir \
reference-privee/veille-taux.md (relevé sur une compagnie réelle, pièce hors du dépôt public : dépôt privé)).",
                    an,
                    disponibles(dossier)
                )));
            }
            return Err(introuvable(format!(
                "Taux AT/MP {} de la compagnie : a renseigner dans Configuration \
> Entreprise (Taux accident du travail), ou a la main dans \
{}, qui tient en quatre lignes :\n\
    {{\"annee\": {}, \"socle\": {}, \"taux_atmp\": {{\"valeur\": 1.50, \
\"code_risque\": \"923AD\"}}}}\n\
\x20 Ce taux est notifie chaque janvier par l'Assurance Maladie \
(compte AT/MP sur net-entreprises) : il est propre a \
l'etablissement, et aucun logiciel ne peut aller le chercher.",
                an, ou, an, an
            )));
        }
        Some(f) => f,
    };
    let marque = match fichier.get("socle") {
        Some(m) if vrai(m) => m,
        _ => return Ok(fichier),
    };
    let an_socle = if marque == &Value::Bool(true) { i64::from(an) } else { entier(marque)? };
    let s = match i32::try_from(an_socle).ok().map(|a| socle(dossier, a)).transpose()? {
        Some(Some(s)) => s,
        _ => {
            return Err(introuvable(format!(
                "Bareme {} : {} demande le socle {}, qui n'est pas livre.\n\
\x20 Socles disponibles : {}. Un socle neuf s'ecrit en janvier, \
depuis le plus recent.",
                an,
                ou,
                an_socle,
                disponibles(dossier)
            )));
        }
    };
    // Les trois etages, dans l'ordre : droit commun, convention, compagnie.
    // `idcc` absent = on ne demande pas la resolution par convention.
    let mut s = s;
    if let Some(idcc) = idcc {
        let couche = convention(&s, idcc)?;
        if vrai(&couche) {
            s = fusionner(&s, &couche);
        }
    }
    Ok(fusionner(&s, &fichier))
}

// ----------------------------------------- l'entreprise et ses etablissements

fn principal(fiche: &Value) -> String {
    texte_ou_vide(fiche.get("nic"))
}

/// Les etablissements autres que le principal.
fn secondaires(fiche: &Value) -> Vec<Value> {
    let nic = principal(fiche);
    let brut = fiche.get("etablissements").filter(|v| vrai(v)).cloned().unwrap_or_else(|| Value::Array(vec![]));
    let liste = match brut {
        Value::Array(a) => a,
        autre => vec![autre],
    };
    liste.into_iter().filter(|e| e.is_object() && texte_ou_vide(e.get("nic")) != nic).collect()
}

/// La cle de Luhn d'un SIRET (14 chiffres).
fn cle_siret(siret: &str) -> bool {
    if siret.len() != 14 || !siret.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let total: u32 = siret
        .chars()
        .rev()
        .enumerate()
        .map(|(i, c)| {
            let n = c.to_digit(10).unwrap() * if i % 2 == 1 { 2 } else { 1 };
            if n > 9 { n - 9 } else { n }
        })
        .sum();
    total % 10 == 0
}

/// Les etablissements de la fiche se tiennent-ils (SIRET, NIC, une fois chacun) ?
fn controler_etablissements(fiche: &Value, bareme: &Value) -> Resultat<()> {
    let siren = texte_ou_vide(fiche.get("siren"));
    let mut vus = vec![principal(fiche)];
    let mut griefs: Vec<String> = Vec::new();
    for e in secondaires(fiche) {
        let nic = texte_ou_vide(e.get("nic"));
        let nom = match e.get("libelle") {
            Some(l) if vrai(l) => en_texte(l),
            _ if !nic.is_empty() => nic.clone(),
            _ => "?".to_string(),
        };
        if nic.len() != 5 || !nic.chars().all(|c| c.is_ascii_digit()) {
            griefs.push(format!("{} : NIC {} (cinq chiffres attendus)", nom, repr_texte(&nic)));
            continue;
        }
        if vus.contains(&nic) {
            griefs.push(format!("{} : NIC {} en double", nom, nic));
        }
        vus.push(nic.clone());
        if !siren.is_empty() && !cle_siret(&format!("{}{}", siren, nic)) {
            griefs.push(format!("{} : le SIRET {}{} n'a pas une cle juste", nom, siren, nic));
        }
        for cle in ["adresse", "code_postal", "commune"] {
            if texte_ou_vide(e.get(cle)).trim().is_empty() {
                griefs.push(format!("{} : `{}` manque", nom, cle));
            }
        }
        let atmp = e.get("taux_atmp");
        let atmp_juste = match atmp {
            Some(a) if a.is_object() => a.get("valeur").map_or(false, |v| !v.is_null()) && vrai_opt(a.get("code_risque")),
            _ => false,
        };
        if !atmp_juste {
            griefs.push(format!(
                "{} : son taux AT/MP (`taux_atmp` : valeur et \
code_risque), notifie pour CET etablissement",
                nom
            ));
        }
        if vrai_opt(bareme.get("versement_mobilite")) && e.get("versement_mobilite").map_or(true, Value::is_null) {
            griefs.push(format!(
                "{} : le siege paie un versement mobilite ; dire celui \
de cet etablissement (`versement_mobilite`, ou \
\"aucun\"), qui depend de sa commune",
                nom
            ));
        }
    }
    if griefs.is_empty() {
        return Ok(());
    }
    Err(format!(
        "config/entreprise.json, etablissements : chaque etablissement se \
declare dans sa propre DSN, avec ce qui est a lui.\n  - {}",
        griefs.join("\n  - ")
    ))
}

/// Le secteur de la fiche est-il un de ceux du bareme ?
fn controler_secteur(bareme: &Value, fiche: &Value) -> Resultat<()> {
    let sec = match bareme.get("secteurs") {
        Some(s) if vrai(s) => s,
        _ => return Ok(()),
    };
    let champ = exiger(sec, "champ")?;
    let dit = texte_ou_vide(champ.as_str().and_then(|c| fiche.get(c))).trim().to_string();
    let portes: Vec<&Value> = exiger(sec, "portes")?.as_array().map(|a| a.iter().collect()).unwrap_or_default();
    if portes.iter().any(|p| p.as_str() == Some(&dit)) {
        return Ok(());
    }
    Err(format!(
        "Convention {} : {}\n  Fiche Entreprise, « {} » : {}. Portés : {}.",
        texte_ou_vide_defaut(fiche.get("idcc"), "None"),
        en_texte(exiger(sec, "refus")?),
        texte_ou_vide_defaut(sec.get("libelle"), &en_texte(champ)),
        if dit.is_empty() { "non renseigné".to_string() } else { dit },
        portes.iter().map(|p| en_texte(p)).collect::<Vec<_>>().join(", ")
    ))
}

/// La couche de l'organisation patronale `nom`.
fn couche_organisation<'a>(bareme: &'a Value, nom: &str) -> Option<&'a Value> {
    let cle = sigle(nom);
    if cle.is_empty() {
        return None;
    }
    for (k, c) in bareme.get("par_organisation_patronale")?.as_object()? {
        if k.starts_with('_') || !c.is_object() {
            continue;
        }
        let alias = c.get("alias").filter(|a| vrai(a)).and_then(Value::as_array);
        if cle == sigle(k) || alias.map_or(false, |a| a.iter().any(|x| sigle(&en_texte(x)) == cle)) {
            return Some(c);
        }
    }
    None
}

/// Ce que `bareme::bareme_retraite_t1` lit d'une couche d'organisation.
const COUCHE_ORGANISATION_LUE: [&str; 5] = ["libelle", "applicable", "alias", "retraite_t1", "adhesion_avant"];

/// L'organisation patronale de la fiche est-elle une de celles du bareme ?
fn controler_organisation_patronale(bareme: &Value, fiche: &Value) -> Resultat<()> {
    let nom = texte_ou_vide(fiche.get("organisation_patronale")).trim().to_string();
    if nom.is_empty() {
        return Ok(());
    }
    let annee = texte_ou_vide_defaut(bareme.get("annee"), "?");
    let connues: Vec<&str> = bareme
        .get("par_organisation_patronale")
        .and_then(Value::as_object)
        .map(|o| o.keys().map(String::as_str).filter(|k| !k.starts_with('_')).collect())
        .unwrap_or_default();
    let couche = match couche_organisation(bareme, &nom) {
        Some(c) => c,
        None => {
            return Err(format!(
                "Bareme {} : la fiche entreprise declare une adhesion a \
l'organisation patronale {} , et le socle n'a pas de couche pour \
elle.\n\
\x20 Couvertes a ce jour : {}.\n\
\x20 Ce qui manque n'est pas du code : les taux que cette adhesion \
change, a relever a la source (l'accord lui-meme, la fiche de \
parametrage de l'organisme), puis a ecrire dans \
`par_organisation_patronale` de donnees/baremes/{}.json.\n\
\x20 Une compagnie qui n'adhere a rien laisse ce champ vide, et rien \
ne change pour elle.",
                annee,
                repr_texte(&nom),
                if connues.is_empty() { "aucune".to_string() } else { connues.join(", ") },
                annee
            )
            .replace(" , et le socle", ", et le socle"));
        }
    };
    if couche.get("applicable") != Some(&Value::Bool(true)) {
        return Err(format!(
            "Bareme {}, adhesion a l'organisation patronale {} ({}) : la \
couche existe mais n'est pas applicable, le moteur ne paie donc \
pas.\n  {}",
            annee,
            nom,
            texte_ou_vide_defaut(couche.get("libelle"), ""),
            texte_ou_vide_defaut(couche.get("_a_confirmer"), "")
        ));
    }
    // Le garde-fou du garde-fou : une cle de plus serait declaree et jamais appliquee.
    let mut inconnues: Vec<&str> = couche
        .as_object()
        .into_iter()
        .flat_map(|o| o.keys())
        .map(String::as_str)
        .filter(|k| !k.starts_with('_') && !COUCHE_ORGANISATION_LUE.contains(k))
        .collect();
    inconnues.sort();
    if !inconnues.is_empty() {
        let mut lues = COUCHE_ORGANISATION_LUE.to_vec();
        lues.sort();
        return Err(format!(
            "Bareme {}, adhesion {} : la couche porte {}, que rien n'empile -- \
ce serait declare et jamais applique.\n\
\x20 Ce que le moteur lit d'une couche : {}.",
            annee,
            nom,
            inconnues.join(", "),
            lues.join(", ")
        ));
    }
    Ok(())
}

/// La valeur au bout d'un chemin du bareme (`a.b`,
/// `[id ou code81]` dans une liste d'objets, `[rang]` dans une liste de valeurs).
fn valeur_du_chemin<'a>(bareme: &'a Value, chemin: &str) -> Option<&'a Value> {
    let c: Vec<char> = chemin.chars().collect();
    let mut morceaux: Vec<String> = Vec::new();
    let mut i = 0;
    while i < c.len() {
        if c[i] == '[' {
            match c[i + 1..].iter().position(|x| *x == ']') {
                Some(f) => {
                    morceaux.push(c[i..=i + 1 + f].iter().collect());
                    i += f + 2;
                }
                None => i += 1,
            }
        } else if c[i] == '.' || c[i] == ']' {
            i += 1;
        } else {
            let mut j = i;
            while j < c.len() && !".[]".contains(c[j]) {
                j += 1;
            }
            morceaux.push(c[i..j].iter().collect());
            i = j;
        }
    }
    let mut cur = bareme;
    for t in morceaux {
        if t.starts_with('[') {
            let liste = cur.as_array().filter(|l| !l.is_empty())?;
            let cle = &t[1..t.len() - 1];
            if liste[0].is_object() {
                cur = liste.iter().find(|e| {
                    e.is_object()
                        && [en_texte(e.get("id").unwrap_or(&Value::Null)), en_texte(e.get("code81").unwrap_or(&Value::Null))]
                            .iter()
                            .any(|x| x == cle)
                })?;
            } else {
                let chiffres = cle.trim_start_matches('-');
                if chiffres.is_empty() || !chiffres.chars().all(|c| c.is_ascii_digit()) {
                    return None;
                }
                let rang: i64 = cle.parse().ok()?;
                let n = liste.len() as i64;
                if rang >= n {
                    return None;
                }
                // Un rang negatif compte depuis la fin.
                cur = liste.get(usize::try_from(if rang < 0 { n + rang } else { rang }).ok()?)?;
            }
        } else {
            cur = cur.as_object()?.get(&t)?;
        }
    }
    Some(cur)
}

/// Le flottant d'une valeur, un booleen comptant 1 ou 0 ; `None` s'il ne se lit pas.
fn flottant_ou_rien(v: &Value) -> Option<f64> {
    match v {
        Value::Bool(b) => Some(f64::from(u8::from(*b))),
        autre => flottant(autre).ok(),
    }
}

/// Deux valeurs sont-elles le meme nombre ?
fn meme_nombre(a: Option<&Value>, b: &Value) -> bool {
    let Some(a) = a else { return false };
    match (flottant_ou_rien(a), flottant_ou_rien(b)) {
        (Some(x), Some(y)) => (x - y).abs() < 1e-9,
        _ => a == b,
    }
}

/// Le bareme fondu dit-il ce qu'il faut pour
/// l'effectif de l'entreprise ? Un bareme sans `seuils_effectif` ne se controle pas.
fn controler_effectif(bareme: &Value, eff: &Effectif, ou: &str) -> Resultat<()> {
    let decl = match bareme.get("seuils_effectif") {
        Some(dc) if dc.is_object() => dc,
        _ => return Ok(()),
    };
    let an = i32::try_from(entier(bareme.get("annee").unwrap_or(&Value::Null))?).map_err(|e| e.to_string())?;
    let mut atteints: BTreeMap<i64, bool> = BTreeMap::new();
    let mut atteint = |seuil: &Value| -> Resultat<bool> {
        let s = entier(seuil)?;
        if let Some(b) = atteints.get(&s) {
            return Ok(*b);
        }
        let b = eff.atteint(s, an)?;
        atteints.insert(s, b);
        Ok(b)
    };
    let (mut griefs, mut noms): (Vec<String>, Vec<String>) = (vec![], vec![]);
    let vide = Vec::new();
    let liste = |cle: &str| decl.get(cle).filter(|v| vrai(v)).and_then(Value::as_array).unwrap_or(&vide);

    for v in liste("valeurs") {
        if !atteint(exiger(v, "seuil")?)? {
            // SOUS LE SEUIL : quand le socle dit que sa valeur est alors la seule due (`sous_le_seuil`, et le
            // texte qu'il cite : pour la formation, C. trav. L. 6331-1 et L. 6331-1 A, CSS L. 130-1, II), une surcouche
            // qui en ecrit une autre -- restee d'avant un franchissement a la baisse -- ferait payer ce qui n'est pas du.
            if let (Some(texte), Some(Value::Object(chemins))) = (v.get("sous_le_seuil").filter(|t| vrai(t)), v.get("chemins")) {
                let mut autres: Vec<String> = chemins
                    .iter()
                    .filter_map(|(c, attendu)| {
                        let ecrit = valeur_du_chemin(bareme, c)?;
                        (!meme_nombre(Some(ecrit), attendu)).then(|| format!("{} = {} (le socle : {})", c, en_texte(ecrit), en_texte(attendu)))
                    })
                    .collect();
                if !autres.is_empty() {
                    autres.sort();
                    let cle = texte_ou_vide_defaut(v.get("cle"), "None");
                    noms.push(format!("{} (sous le seuil)", cle));
                    griefs.push(format!(
                        "  - {} : l'effectif n'atteint pas le seuil de {} salaries, et la surcouche ecrit une autre valeur que celle du socle
    {}
    Ecrit : {}
    A FAIRE : effacer ces valeurs de la surcouche ({}) ; le socle porte celle qui est due.
    Source : {}",
                        cle,
                        en_texte(exiger(v, "seuil")?),
                        texte_ou_vide_defaut(v.get("quoi"), ""),
                        autres.join(" ; "),
                        if ou.is_empty() { "config/baremes/<annee>.json" } else { ou },
                        en_texte(texte)
                    ));
                }
            }
            continue;
        }
        let mut restees: Vec<&str> = Vec::new();
        if let Some(Value::Object(chemins)) = v.get("chemins") {
            for (c, attendu) in chemins {
                if meme_nombre(valeur_du_chemin(bareme, c), attendu) {
                    restees.push(c);
                }
            }
        }
        if restees.is_empty() {
            continue;
        }
        restees.sort();
        noms.push(texte_ou_vide_defaut(v.get("cle"), "None"));
        griefs.push(format!(
            "  - {} (a partir de {} salaries)\n    {}\n    A ECRIRE : {}\n    Encore au taux du socle : {}\n    Source : {}",
            texte_ou_vide_defaut(v.get("cle"), "None"),
            en_texte(exiger(v, "seuil")?),
            texte_ou_vide_defaut(v.get("quoi"), ""),
            texte_ou_vide_defaut(v.get("au_dela"), ""),
            restees.join(", "),
            texte_ou_vide_defaut(v.get("source"), "")
        ));
    }

    for v in liste("non_portees") {
        let cle_v = match v.get("cle_bareme") {
            Some(c) if vrai(c) => Some(c),
            _ => v.get("cle"),
        };
        let cle = cle_v.map_or("None".to_string(), en_texte);
        let rep = cle_v.and_then(Value::as_str).and_then(|c| bareme.get(c));
        let porte_si_taux = vrai_opt(v.get("porte_si_taux"));
        let rep_objet = rep.map_or(false, Value::is_object);
        if !atteint(exiger(v, "seuil")?)? {
            if porte_si_taux && rep_objet {
                // Un taux ecrit sous le seuil : la compagnie paierait ce qu'elle ne doit pas.
                noms.push(format!("{} (sous le seuil)", cle));
                griefs.push(format!(
                    "  - {} : un taux est ecrit dans la surcouche, et il n'est \
du qu'a partir de {} salaries (CGCT L2333-64, L2531-2). \
L'effacer, ou ecrire \"aucun\".",
                    cle,
                    en_texte(exiger(v, "seuil")?)
                ));
            }
            continue;
        }
        let mut attendues: Vec<&str> = match v.get("reponses") {
            Some(Value::Object(o)) => o.keys().map(String::as_str).filter(|r| !r.starts_with('_')).collect(),
            _ => vec![],
        };
        if let Some(Value::String(r)) = rep {
            if attendues.contains(&r.as_str()) {
                continue;
            }
        }
        if porte_si_taux && rep_objet {
            let r = rep.unwrap();
            if r.get("taux").map_or(false, |t| !t.is_null()) && texte_ou_vide(r.get("code_insee")).trim().chars().count() == 5 {
                continue; // porte : un taux et une commune
            }
        }
        noms.push(format!("{} (non porte)", cle));
        let constat = match rep {
            None | Some(Value::Null) => "Rien n'est ecrit sur ce point.".to_string(),
            Some(r) if porte_si_taux && r.is_object() => format!(
                "La valeur ecrite ({}) n'a pas son taux ET le code INSEE (cinq \
caracteres) de la commune de l'etablissement : les deux se \
lisent sur la deliberation de l'autorite ou sur le bordereau \
Urssaf.",
                forme_ecrite(r)
            ),
            Some(r) => format!(
                "La valeur ecrite ({}) ne le debloque pas : il n'y a ni \
rubrique, ni ligne DSN, ni CTP pour la porter -- ecrire un \
taux ne le ferait pas cotiser.",
                forme_ecrite(r)
            ),
        };
        attendues.sort();
        griefs.push(format!(
            "  - {} (a partir de {} salaries) -- LE MOTEUR NE LE PORTE PAS\n    {}\n    {}\n    A ECRIRE dans {} : \"{}\": {}, et rien d'autre ne le debloque.\n    Source : {}",
            texte_ou_vide_defaut(v.get("cle"), "None"),
            en_texte(exiger(v, "seuil")?),
            texte_ou_vide_defaut(v.get("quoi"), ""),
            constat,
            if ou.is_empty() { "config/baremes/<annee>.json" } else { ou },
            cle,
            attendues.iter().map(|r| format!("\"{}\"", r)).collect::<Vec<_>>().join(" ou "),
            texte_ou_vide_defaut(v.get("source"), "")
        ));
    }

    if griefs.is_empty() {
        return Ok(());
    }
    Err(format!(
        "Bareme {}, entreprise de {} salaries : le moteur REFUSE de payer \
-- {}.\n\
\x20 Le socle livre avec le logiciel est ecrit pour le plus petit \
palier -- moins de onze salaries --, et ce qui change au-dessus n'est \
pas ecrit ici. Produire quand meme donnerait des bulletins et une DSN \
faux, sans un mot : c'est ce que le moteur faisait avant le \
21/09/2026.\n\
\x20 Ce qui manque, et ou :\n{}\n\
\x20 Une valeur SURCHARGEE est tenue pour decidee : le moteur verifie \
qu'une decision a ete prise, pas laquelle -- comme pour le taux \
AT/MP, que lui seul ne peut pas connaitre.",
        texte_ou_vide_defaut(bareme.get("annee"), "?"),
        eff.valeur(an).map_or("?".to_string(), format_g),
        noms.join(", "),
        griefs.join("\n")
    ))
}

/// Les quatre, dans l'ordre du plus structurel au plus chiffre.
fn controler(bareme: &Value, fiche: &Value, eff: &Effectif, ou: &str) -> Resultat<()> {
    controler_secteur(bareme, fiche)?;
    controler_etablissements(fiche, bareme)?;
    controler_organisation_patronale(bareme, fiche)?;
    controler_effectif(bareme, eff, ou)
}
// ----------------------------------------------------- les listes datees

/// Une liste datee : `[{depuis, valeur}, ...]`. La forme des baremes et de toute la configuration de la compagnie.
fn est_liste_datee(v: &Value) -> bool {
    match v {
        Value::Array(a) => {
            !a.is_empty() && a.iter().all(|e| e.is_object() && e.get("depuis").is_some() && e.get("valeur").is_some())
        }
        _ => false,
    }
}

/// LA LECTURE D'UNE LISTE DATEE, pour les baremes comme pour la configuration : la valeur dont le `depuis` est le plus
/// recent sans depasser `jour` (a meme date, la derniere ecrite) ; `None` avant la premiere.
fn valeur_au<'a>(liste: &'a Value, jour: &Jour) -> Resultat<Option<&'a Value>> {
    let mut entrees: Vec<&Value> = liste.as_array().into_iter().flatten().collect();
    entrees.sort_by_key(|e| en_texte(&e["depuis"]));
    let mut retenu = None;
    for e in entrees {
        if d(&e["depuis"])? <= *jour {
            retenu = Some(&e["valeur"]);
        }
    }
    Ok(retenu)
}

/// Remplace chaque liste datee `[{depuis, valeur}]` par
/// sa valeur au jour. Une liste sans entree applicable est une erreur : un taux
/// qui n'existe pas encore ne se devine pas. Les cles de `garder` restent des listes.
fn a_la_date(obj: &Value, jour: &Jour, garder: &[&str]) -> Resultat<Value> {
    if let Value::Object(o) = obj {
        let mut out = Map::new();
        for (k, v) in o {
            out.insert(k.clone(), if garder.contains(&k.as_str()) { v.clone() } else { a_la_date(v, jour, garder)? });
        }
        return Ok(Value::Object(out));
    }
    if est_liste_datee(obj) {
        return valeur_au(obj, jour)?
            .cloned()
            .ok_or_else(|| format!("Bareme : aucune valeur applicable au {} dans {}.", jour.iso(), forme_ecrite(obj)));
    }
    if let Value::Array(a) = obj {
        return Ok(Value::Array(a.iter().map(|v| a_la_date(v, jour, garder)).collect::<Resultat<Vec<_>>>()?));
    }
    Ok(obj.clone())
}


// --------------------------------------------------------------- le Contexte

/// Le jour courant (UTC : sans fuseau, un jour local differe
/// entre minuit et l'heure locale). Au temps du moteur (`temps`) : la montre, ou le temps simule du banc et du rejeu.
pub fn jour_reel() -> Jour {
    socle::temps::jour_utc()
}

/// `PUPPETPAIE_AUJOURDHUI` (AAAA-MM-JJ) remplace l'horloge.
pub fn aujourdhui() -> Resultat<Jour> {
    let jour = aujourdhui_brut()?;
    socle::fichiers::noter_valeur("aujourdhui", jour.iso());
    Ok(jour)
}

/// Sans le noter : `Contexte::nouveau` le garde, le calcul ne le lit pas.
pub fn aujourdhui_sans_note() -> Resultat<Jour> {
    aujourdhui_brut()
}

fn aujourdhui_brut() -> Resultat<Jour> {
    // LE TEMPS SIMULE (le banc d'essai, le rejeu d'un scenario : `temps`) fait foi : c'est lui qui dit quel jour on est.
    if socle::temps::simule() {
        return Ok(socle::temps::jour_de_paris());
    }
    match std::env::var("PUPPETPAIE_AUJOURDHUI") {
        Ok(v) if !v.is_empty() => Jour::lire(&v),
        _ => Ok(jour_reel()),
    }
}

/// LE CURSEUR DES PERMANENTS : une fiche Entreprise dit-elle que la compagnie emploie des
/// salaries payes au mois (`emploie_des_permanents`) ? Absent vaut NON. C'est la seule question qui ouvre ce qui ne vaut
/// que pour eux : les natures payees au mois, les champs `si_permanents` des fiches, les contrats de prevoyance et de
/// sante des permanents (`organismes`), les etats qui ne concernent qu'eux. La page ne le deduit jamais : elle le lit
/// (`configuration.permanents`) ou, la case a peine cochee, le redemande (route `permanents`).
pub fn emploie_des_permanents(entreprise: &Value) -> bool {
    entreprise.get("emploie_des_permanents") == Some(&Value::Bool(true))
}

/// Les donnees de la compagnie, lues une fois, et resolues a une date (`lue_au`).
#[derive(Debug, Clone)]
pub struct Contexte {
    pub racine: PathBuf,
    /// `logiciel/donnees` : le socle des baremes est dans `code/baremes/`.
    pub code: PathBuf,
    /// Le jour de la declaration (celui ou l'on demande si un taux de PAS vaut encore).
    pub date: Jour,
    /// La date a laquelle la configuration est lue : `date`, ou la periode payee (`au`).
    pub lue_au: Jour,
    /// Quel jour on est, pour les gardes du calendrier (distinct de `date`).
    pub aujourdhui: Jour,
    /// La fiche entreprise a `lue_au`.
    pub entreprise: Value,
    entreprise_brute: Value,
    /// Les fiches salaries a `lue_au` (celles qui existaient ce jour-la).
    pub salaries: Map<String, Value>,
    /// Les fiches telles qu'ecrites, datees : une periode se lit a sa date (`salarie_au`, `au`).
    salaries_bruts: Map<String, Value>,
    pub spectacles: Map<String, Value>,
    spectacles_bruts: Map<String, Value>,
}

const ENTREPRISE: &str = "config/entreprise.json";

/// Les fiches d'un fichier (`salaries.json`, `spectacles.json`) a `jour` : celles qui existaient ce jour-la.
fn fiches_au(bruts: &Map<String, Value>, jour: &Jour, fichier: &str) -> Resultat<Map<String, Value>> {
    let mut out = Map::new();
    for (k, v) in bruts {
        let f = fiche_au(v, jour, &format!("config/{}, {}", fichier, k))?;
        if !fiche_vide(&f) {
            out.insert(k.clone(), f);
        }
    }
    Ok(out)
}

impl Contexte {
    /// Le contexte de `racine` a la date `date`. Sans `date`, celle de l'horloge du moteur (`jour_reel`).
    pub fn nouveau(racine: &Path, date: Option<Jour>) -> Resultat<Contexte> {
        let date = date.unwrap_or_else(jour_reel);
        let aujourdhui = aujourdhui_brut()?;
        let cfg = racine.join("config");
        let entreprise_brute = lire_json(&cfg.join("entreprise.json"))?;
        let bruts = |nom: &str| -> Resultat<Map<String, Value>> {
            Ok(objet(lire_json(&cfg.join(nom))?, nom)?.into_iter().filter(|(k, _)| !k.starts_with('_')).collect())
        };
        let (salaries_bruts, spectacles_bruts) = (bruts("salaries.json")?, bruts("spectacles.json")?);
        Ok(Contexte {
            racine: racine.to_path_buf(),
            code: dossier_du_code(),
            date,
            lue_au: date,
            aujourdhui,
            entreprise: fiche_au(&entreprise_brute, &date, ENTREPRISE)?,
            entreprise_brute,
            salaries: fiches_au(&salaries_bruts, &date, "salaries.json")?,
            salaries_bruts,
            spectacles: fiches_au(&spectacles_bruts, &date, "spectacles.json")?,
            spectacles_bruts,
        })
    }

    /// LE CONTEXTE D'UNE PERIODE : la meme compagnie, sa configuration lue a `jour` (la fin de la periode payee). Le
    /// calcul d'une paie, la DSN d'un mois et leurs documents lisent tout par lui, jamais « tel qu'aujourd'hui ».
    pub fn au(&self, jour: &Jour) -> Resultat<Contexte> {
        if *jour == self.lue_au {
            return Ok(self.clone());
        }
        Ok(Contexte {
            lue_au: *jour,
            entreprise: fiche_au(&self.entreprise_brute, jour, ENTREPRISE)?,
            salaries: fiches_au(&self.salaries_bruts, jour, "salaries.json")?,
            spectacles: fiches_au(&self.spectacles_bruts, jour, "spectacles.json")?,
            ..self.clone()
        })
    }

    /// Le contexte du mois `AAAA-MM` : sa configuration lue au dernier jour du mois.
    pub fn du_mois(&self, mois: &str) -> Resultat<Contexte> {
        self.au(&socle::date::dernier_du_mois(mois)?)
    }

    /// La fiche entreprise a `jour`.
    pub fn entreprise_au(&self, jour: &Jour) -> Resultat<Value> {
        if *jour == self.lue_au {
            return Ok(self.entreprise.clone());
        }
        fiche_au(&self.entreprise_brute, jour, ENTREPRISE)
    }

    /// La fiche salarie `cle` a `jour`, avec `_cle`. Refus : la fiche ne dit rien ce jour-la.
    pub fn salarie_au(&self, cle: &str, jour: &Jour) -> Resultat<Value> {
        let brute = self.salaries_bruts.get(cle).ok_or_else(|| format!("Salarie inconnu : {}.", repr_texte(cle)))?;
        let mut f = fiche_au(brute, jour, &format!("config/salaries.json, {}", cle))?;
        if fiche_vide(&f) {
            return Err(format!(
                "La fiche de {} ne dit rien au {} : sa premiere valeur est posterieure. Dater sa fiche a partir de ce jour \
(Configuration, « a partir du »).",
                cle,
                jour.jj_mm_aaaa()
            ));
        }
        f.as_object_mut().map(|o| o.insert("_cle".into(), Value::String(cle.to_string())));
        Ok(f)
    }

    /// Les cles des salaries qu'on peut encore embaucher (les archives restent dans `salaries`).
    pub fn salaries_actifs(&self) -> Vec<&str> {
        self.salaries.iter().filter(|(_, v)| !archivee(v)).map(|(k, _)| k.as_str()).collect()
    }

    /// La compagnie emploie-t-elle des permanents (des salaries payes au mois, contrats `cdi` ou `cdd`) ? Une decision de
    /// la fiche Entreprise (`emploie_des_permanents`), pas une deduction : absent vaut NON. C'est le fait qui ouvre tout ce
    /// qui ne vaut que pour eux, a l'ecran comme dans le calcul ; un engagement paye au mois est refuse sans lui
    /// (`engagement::charger`) : il n'y a donc pas de permanent sans cette case.
    pub fn emploie_des_permanents(&self) -> bool {
        emploie_des_permanents(&self.entreprise)
    }

    /// Idem pour les spectacles.
    pub fn spectacles_actifs(&self) -> Vec<&str> {
        self.spectacles.iter().filter(|(_, v)| !archivee(v)).map(|(k, _)| k.as_str()).collect()
    }

    /// L'effectif de la compagnie (`config/effectifs.json`).
    pub fn effectif(&self) -> Resultat<Effectif> {
        effectif_de(&self.racine, &self.entreprise)
    }

    /// Le 5 a partir de cinquante salaries, le 15 sinon
    /// (un effectif inconnu donne le 15).
    pub fn echeance_dsn(&self) -> Resultat<u32> {
        socle::fichiers::noter_valeur("aujourdhui", self.aujourdhui.iso());
        Ok(match self.effectif()?.atteint(50, self.aujourdhui.an) {
            Ok(true) => 5,
            _ => 15,
        })
    }

    /// Le socle livre de l'annee, ou `None`.
    pub fn socle_livre(&self, an: i32) -> Resultat<Option<Value>> {
        socle(&self.code.join("baremes"), an)
    }

    /// Le dossier des gabarits de la compagnie.
    pub fn gabarits(&self) -> PathBuf {
        self.racine.join("gabarits")
    }

    /// La fiche entreprise et le dossier `gabarits/` de CETTE compagnie (`_gabarits`) : ce qu'un document lit pour
    /// trouver son modele, la signature, le logo ou le cachet. La compagnie vient du contexte, jamais d'une variable
    /// du processus : la fenetre sert plusieurs compagnies dans le meme processus.
    pub fn entreprise_et_gabarits(&self) -> Value {
        let mut ent = self.entreprise.clone();
        if let Some(o) = ent.as_object_mut() {
            o.insert("_gabarits".into(), Value::String(self.gabarits().to_string_lossy().into_owned()));
        }
        ent
    }

    /// Ce dont depend le bareme lu a `jour` : la compagnie, sa fiche entreprise a ce jour, et la signature (date, taille)
    /// des fichiers qui le composent.
    pub fn empreinte_bareme(&self, jour: &Jour) -> Resultat<String> {
        Ok(self.empreinte_bareme_de(jour.an, &self.entreprise_au(jour)?))
    }

    fn empreinte_bareme_de(&self, an: i32, ent: &Value) -> String {
        let cfg = self.racine.join("config");
        let sig = [
            socle::fichiers::signature(&cfg.join("baremes").join(format!("{}.json", an))),
            socle::fichiers::signature(&self.code.join("baremes").join(format!("{}.json", an))),
            socle::fichiers::signature(&cfg.join("effectifs.json")),
        ];
        format!("{}|{}|{}|{:?}|{}|g{}", self.racine.display(), self.code.display(), an, sig, ent, socle::fichiers::generation())
    }

    /// Le socle livre + la couche de la convention que la fiche `ent` nomme + la surcouche de la compagnie, confrontes
    /// aux faits d'entreprise de cette fiche. Les listes datees restent des listes. `ent` est la fiche entreprise au
    /// jour lu : une compagnie qui change de convention en cours d'annee lit chaque jour sous la sienne.
    fn bareme_brut(&self, an: i32, ent: &Value) -> Result<Arc<Value>, Refus> {
        // Garde tant que les fichiers et la fiche sont les memes. Un refus ne se garde pas.
        type Cache = Mutex<HashMap<String, Arc<Value>>>;
        static CACHE: OnceLock<Cache> = OnceLock::new();
        let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        let cle = self.empreinte_bareme_de(an, ent);
        if let Some(v) = cache.lock().unwrap().get(&cle) {
            return Ok(v.clone());
        }
        let fondu = Arc::new(self.bareme_brut_sans_cache(an, ent)?);
        socle::fichiers::ranger(cache, cle, fondu.clone());
        Ok(fondu)
    }

    fn bareme_brut_sans_cache(&self, an: i32, ent: &Value) -> Result<Value, Refus> {
        let p = self.racine.join("config").join("baremes").join(format!("{}.json", an));
        let fichier = if socle::fichiers::existe(&p) { Some(lire_json(&p)?) } else { None };
        let idcc = ent.get("idcc").filter(|v| !v.is_null());
        let fondu = bareme_de_l_annee(&self.code.join("baremes"), an, fichier, idcc)?;
        // Le controle est ici, le seul endroit qui compose un bareme.
        controler(&fondu, ent, &effectif_de(&self.racine, ent)?, &format!("config/baremes/{}.json", an))?;
        Ok(fondu)
    }

    /// Le bareme applicable A CETTE DATE. Les listes
    /// `{depuis, valeur}` deviennent la valeur en vigueur (sauf `smic_horaire`,
    /// qui se resout a la fin de la periode payee : voir `smic`), et le bareme
    /// porte `_entreprise` (et `_etablissements` s'il y en a plusieurs) : ce
    /// que le calcul et la DSN doivent savoir de la fiche.
    /// LES REGLES DATEES QUI S'APPLIQUENT A CE JOUR : le `depuis` de la valeur en vigueur de chaque liste datee
    /// `[{depuis, valeur}]` du bareme de l'annee (le socle, la convention, la compagnie), du plus ancien au plus recent,
    /// sans doublon. Une paie qui s'est calculee sous d'autres regles le dit a sa date (« regle du JJ/MM/AAAA »).
    pub fn regles_datees(&self, jour: &Jour) -> Resultat<Vec<Jour>> {
        fn parcourir(v: &Value, jour: &Jour, out: &mut Vec<Jour>) -> Resultat<()> {
            if est_liste_datee(v) {
                let mut retenu: Option<Jour> = None;
                for e in v.as_array().into_iter().flatten() {
                    let depuis = d(&e["depuis"])?;
                    if depuis <= *jour && retenu.map_or(true, |r| depuis > r) {
                        retenu = Some(depuis);
                    }
                }
                out.extend(retenu);
                return Ok(());
            }
            match v {
                Value::Object(o) => o.values().try_for_each(|x| parcourir(x, jour, out)),
                Value::Array(a) => a.iter().try_for_each(|x| parcourir(x, jour, out)),
                _ => Ok(()),
            }
        }
        let brut = self.bareme_brut(jour.an, &self.entreprise_au(jour)?).map_err(|r| r.message)?;
        let mut out = Vec::new();
        parcourir(&brut, jour, &mut out)?;
        out.sort();
        out.dedup();
        Ok(out)
    }

    pub fn bareme(&self, jour: &Jour) -> Resultat<Value> {
        // Le bareme resolu a ce jour est garde, comme `_baremes_resolus` ; on en
        // rend une copie, que l'appelant peut modifier.
        Ok((*self.bareme_partage(jour)?).clone())
    }

    fn bareme_partage(&self, jour: &Jour) -> Resultat<Arc<Value>> {
        type Cache = Mutex<HashMap<String, Arc<Value>>>;
        static CACHE: OnceLock<Cache> = OnceLock::new();
        let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        let cle = format!("{}|{}", self.empreinte_bareme_de(jour.an, &self.entreprise_au(jour)?), jour.iso());
        if let Some(v) = cache.lock().unwrap().get(&cle) {
            return Ok(v.clone());
        }
        let bar = Arc::new(self.bareme_sans_cache(jour)?);
        socle::fichiers::ranger(cache, cle, bar.clone());
        Ok(bar)
    }

    fn bareme_sans_cache(&self, jour: &Jour) -> Resultat<Value> {
        let ent = &self.entreprise_au(jour)?;
        let brut = self.bareme_brut(jour.an, ent).map_err(|r| r.message)?;
        let mut bar = objet(a_la_date(&brut, jour, &["smic_horaire"])?, "bareme")?;
        let ou_vide = |cle: &str, vide: Value| match ent.get(cle) {
            Some(v) if vrai(v) => v.clone(),
            _ => vide,
        };
        let copie = |cle: &str| ent.get(cle).cloned().unwrap_or(Value::Null);
        let effectif = match effectif_de(&self.racine, ent)?.valeur(jour.an) {
            Some(x) => serde_json::Number::from_f64(x).map_or(Value::Null, Value::Number),
            None => Value::Null,
        };
        let mut e = Map::new();
        e.insert("idcc".into(), copie("idcc"));
        e.insert("annexe_convention".into(), copie("annexe_convention"));
        e.insert("representation_personnel".into(), copie("representation_personnel"));
        e.insert("casc_svp_exemptee".into(), Value::Bool(vrai_opt(ent.get("casc_svp_exemptee"))));
        e.insert("effectif".into(), effectif);
        e.insert("sante_permanents".into(), ou_vide("sante_permanents", Value::Object(Map::new())));
        e.insert("prevoyance_permanents".into(), ou_vide("prevoyance_permanents", Value::Object(Map::new())));
        e.insert("prevoyance_techniciens_3097".into(), copie("prevoyance_techniciens_3097"));
        // Ce que le contrat de sante fait d'un mois entierement suspendu sans indemnisation (`calcul::sante_de_la_suspension`).
        // Absent : rien (la pile, et son empreinte, ne changent pas pour une compagnie qui ne l'a pas dit).
        if let Some(v) = ent.get(crate::bareme::SANTE_SUSPENSION).filter(|v| vrai(v)) {
            e.insert(crate::bareme::SANTE_SUSPENSION.into(), v.clone());
        }
        e.insert("organisation_patronale".into(), copie("organisation_patronale"));
        e.insert("organisation_patronale_depuis".into(), copie("organisation_patronale_depuis"));
        e.insert("premiere_embauche".into(), copie("premiere_embauche"));
        e.insert("code_postal".into(), copie("code_postal"));
        e.insert("etablissements".into(), ou_vide("etablissements", Value::Array(vec![])));
        // Le motif de non-assujettissement a la taxe d'apprentissage : la pile retire la taxe du bulletin et du
        // nominatif quand il vaut pour le mois (`bareme::bareme_taxe_apprentissage`). Absent : rien.
        if let Some(m) = ent.get(crate::apprentissage::CHAMP_MOTIF_TAXE_APPRENTISSAGE).filter(|v| vrai(v)) {
            e.insert(crate::apprentissage::CHAMP_MOTIF_TAXE_APPRENTISSAGE.into(), m.clone());
        }
        bar.insert("_entreprise".into(), Value::Object(e));
        // Les etablissements secondaires : leur taux AT/MP et leur versement
        // mobilite remplacent ceux du siege pour leurs salaries.
        let seconds = secondaires(ent);
        if !seconds.is_empty() {
            let mut m = Map::new();
            m.insert("nic".into(), Value::String(principal(ent)));
            m.insert("etablissements".into(), Value::Array(seconds));
            bar.insert("_etablissements".into(), Value::Object(m));
        }
        Ok(Value::Object(bar))
    }

    /// (PSS mensuel, PSS horaire) de l'annee.
    pub fn plafond_securite_sociale(&self, annee: i32) -> Resultat<(i64, i64)> {
        let brut = match self.bareme_brut(annee, &self.entreprise) {
            Ok(b) => b,
            Err(r) if r.introuvable => {
                return Err(format!(
                    "{} Le plafond de Securite sociale de {} se releve sur la table \
des parametres du BOSS (PSS_M_METROP, PSS_H_METROP) et se pose \
dans ce bareme, section plafond_securite_sociale.",
                    r.message.lines().next().unwrap_or(""),
                    annee
                ))
            }
            Err(r) => return Err(r.message),
        };
        let sec = brut.get("plafond_securite_sociale").filter(|v| vrai(v)).cloned().unwrap_or(Value::Null);
        match (sec.get("mensuel").filter(|v| vrai(v)), sec.get("horaire").filter(|v| vrai(v))) {
            (Some(m), Some(h)) => Ok((entier(m)?, entier(h)?)),
            _ => Err(format!(
                "Pas de plafond de Securite sociale pour {} dans \
config/baremes/{}.json : ajouter la section \
plafond_securite_sociale ({{\"mensuel\": ..., \"horaire\": ...}}), \
relevee sur la table des parametres du BOSS (PSS_M_METROP, \
PSS_H_METROP).",
                annee, annee
            )),
        }
    }

    /// Le SMIC horaire applicable a la date (le bareme liste
    /// les revalorisations ; la derniere dont `depuis <= date` gagne).
    pub fn smic(&self, jour: &Jour) -> Resultat<f64> {
        let brut = self.bareme_brut(jour.an, &self.entreprise_au(jour)?).map_err(|r| r.message)?;
        let liste = exiger(&brut, "smic_horaire")?.as_array().ok_or_else(|| "smic_horaire : une liste est attendue".to_string())?;
        let mut entrees: Vec<&Value> = liste.iter().collect();
        for e in &entrees {
            exiger(e, "depuis")?;
        }
        entrees.sort_by_key(|e| en_texte(&e["depuis"]));
        let mut val = None;
        for e in entrees {
            if d(&e["depuis"])? <= *jour {
                val = Some(exiger(e, "valeur")?);
            }
        }
        match val {
            Some(v) => flottant(v),
            None => Err(format!("Aucun SMIC applicable au {} dans le bareme.", jour.iso())),
        }
    }

    /// La fiche entreprise d'un mois `AAAA-MM` telle que les documents la lisent : lue au dernier jour du mois, avec les
    /// gabarits de cette compagnie (`_gabarits`).
    pub fn entreprise_du_mois(&self, mois: &str) -> Resultat<Value> {
        let mut ent = self.entreprise_au(&socle::date::dernier_du_mois(mois)?)?;
        if let Value::Object(m) = &mut ent {
            m.insert("_gabarits".into(), Value::String(self.gabarits().to_string_lossy().into_owned()));
        }
        Ok(ent)
    }

    /// Le numero de licence de la fiche entreprise a la date (vide : pas de licence).
    pub fn licence(&self, jour: &Jour) -> Resultat<String> {
        Ok(self.entreprise_au(jour)?.get("licence_spectacle").filter(|v| vrai(v)).map(en_texte).unwrap_or_default())
    }

    /// La fiche entreprise telle qu'ecrite, datee.
    pub fn entreprise_brute(&self) -> &Value {
        &self.entreprise_brute
    }

    /// La fiche salarie `cle` telle qu'ecrite, datee, `None` si la cle est inconnue.
    pub fn salarie_brut(&self, cle: &str) -> Option<&Value> {
        self.salaries_bruts.get(cle)
    }

    /// La fiche spectacle `cle` telle qu'ecrite, datee, `None` si la cle est inconnue.
    pub fn spectacle_brut(&self, cle: &str) -> Option<&Value> {
        self.spectacles_bruts.get(cle)
    }

    // -------------------------------------------------------------- recherche

    /// Retrouve un salarie par cle, NIR, nom, prenom ou
    /// faute de frappe. Le nom EXACT retrouve n'importe qui, archive compris ;
    /// la recherche APPROCHANTE (prefixe, Levenshtein <= 3) ne va que parmi les actifs.
    /// Rend la fiche avec `_cle`, ou le refus.
    pub fn salarie(&self, nom: &str) -> Resultat<Value> {
        let q = cle_recherche(nom);
        let avec_cle = |cle: &str| -> Value {
            let mut f = self.salaries[cle].clone();
            f.as_object_mut().map(|o| o.insert("_cle".into(), Value::String(cle.to_string())));
            f
        };
        for cle in self.salaries.keys() {
            if cle_recherche(cle) == q {
                return Ok(avec_cle(cle));
            }
        }
        // Le NIR, quand c'est lui qu'on donne : 13 caracteres, ou 15 avec la cle.
        let brut: String = nom.replace(' ', "").to_uppercase();
        if forme_de_nir(&brut) {
            for (cle, s) in &self.salaries {
                if s.get("nir").map_or(false, |n| vrai(n) && en_texte(n).to_uppercase() == brut[..13]) {
                    return Ok(avec_cle(cle));
                }
            }
        }
        let mut trouves: Vec<&String> = Vec::new();
        for (cle, s) in &self.salaries {
            let (n, p) = (exiger_texte(s, "nom")?, exiger_texte(s, "prenom")?);
            let ensemble = [cle_recherche(&n), cle_recherche(&p), cle_recherche(&format!("{} {}", n, p)), cle_recherche(&format!("{} {}", p, n))];
            if ensemble.contains(&q) {
                trouves.push(cle);
            }
        }
        if trouves.len() > 1 {
            return Err(format!(
                "Salarie ambigu : {} designe {} fiches ({}). L'appeler par son nom, ou par sa cle.",
                repr_texte(nom),
                trouves.len(),
                trouves.iter().map(|c| c.as_str()).collect::<Vec<_>>().join(", ")
            ));
        }
        if let Some(cle) = trouves.first() {
            return Ok(avec_cle(cle));
        }
        // Tolerance : prefixe (Celi -> CELIMENE) puis distance d'edition, sur les seuls actifs.
        let actifs = self.salaries_actifs();
        let noms_de = |cle: &str| -> Resultat<[String; 2]> {
            let s = &self.salaries[cle];
            Ok([cle_recherche(&exiger_texte(s, "prenom")?), cle_recherche(&exiger_texte(s, "nom")?)])
        };
        let mut prefixes: Vec<&str> = Vec::new();
        for cle in &actifs {
            if q.chars().count() >= 3 && noms_de(cle)?.iter().any(|c| c.starts_with(&q)) {
                prefixes.push(cle);
            }
        }
        if prefixes.len() > 1 {
            return Err(format!(
                "Salarie ambigu : {} commence {} fiches ({}). Ecrire le nom en entier.",
                repr_texte(nom),
                prefixes.len(),
                prefixes.join(", ")
            ));
        }
        if let Some(cle) = prefixes.first() {
            return Ok(avec_cle(cle));
        }
        let (mut meilleur, mut score) = (None, 99);
        for cle in &actifs {
            for c in noms_de(cle)? {
                let dist = levenshtein(&q, &c);
                if dist < score {
                    meilleur = Some(*cle);
                    score = dist;
                }
            }
        }
        if let Some(cle) = meilleur.filter(|c| !c.is_empty() && score <= 3) {
            return Ok(avec_cle(cle));
        }
        // Rien parmi les actifs : si une archive ressemble, le dire plutot que « inconnu ».
        for (cle, s) in &self.salaries {
            if !archivee(s) {
                continue;
            }
            let (n, p) = (exiger_texte(s, "nom")?, exiger_texte(s, "prenom")?);
            let ressemble = [p.as_str(), n.as_str(), cle.as_str()].iter().any(|x| {
                let c = cle_recherche(x);
                c.starts_with(&q) || levenshtein(&q, &c) <= 3
            });
            if ressemble {
                return Err(format!(
                    "Salarie archive : {} ressemble a {} {} (cle {}), dont la fiche porte \
`actif: false`. L'appeler par sa cle exacte pour rejouer \
une paie passee ; pour l'embaucher a nouveau, remettre \
`actif: true` dans config/salaries.json.",
                    repr_texte(nom),
                    n,
                    p,
                    repr_texte(cle)
                ));
            }
        }
        Err(format!("Salarie inconnu : {}. Connus : {}.", repr_texte(nom), actifs.join(", ")))
    }

    /// Par cle, titre, alias ou numero d'objet (les fiches
    /// actives d'abord), puis par inclusion dans un sens ou l'autre. Rend la fiche
    /// avec `_cle`, ou le refus.
    /// Un nom vide (ou blanc) ne designe AUCUN spectacle : la recherche par inclusion le trouverait
    /// dans tous (« » est dans tout titre) et rendrait le premier actif, au hasard -- c'est ainsi
    /// qu'un permanent repris se rattachait a un spectacle. Il se refuse.
    pub fn spectacle(&self, nom: &str) -> Resultat<Value> {
        let q = cle_recherche(nom);
        if q.is_empty() && !self.spectacles.contains_key(nom.trim()) {
            return Err(format!(
                "Aucun spectacle n'est nomme ({}). Le spectacle porte le numero d'objet, qui part en DSN, \
                 en AEM et sur le bulletin : il se choisit, il ne se devine pas. Un contrat paye au mois \
                 (CDI, CDD de droit commun) n'en a pas.",
                repr_texte(nom)
            ));
        }
        let avec_cle = |cle: &str| -> Value {
            let mut f = self.spectacles[cle].clone();
            f.as_object_mut().map(|o| o.insert("_cle".into(), Value::String(cle.to_string())));
            f
        };
        let mut ordre: Vec<(&String, &Value)> = self.spectacles.iter().collect();
        ordre.sort_by_key(|(_, v)| archivee(v)); // stable : les actives d'abord
        let net = nom.trim();
        if self.spectacles.contains_key(net) {
            return Ok(avec_cle(net));
        }
        let noms_de = |cle: &str, sp: &Value| -> Resultat<Vec<String>> {
            let mut noms = vec![cle.to_string(), exiger_texte(sp, "titre")?];
            match sp.get("alias") {
                Some(Value::Array(a)) => noms.extend(a.iter().map(en_texte)),
                Some(Value::String(s)) => noms.extend(s.chars().map(String::from)), // list("abc")
                _ => {}
            }
            Ok(noms)
        };
        for (cle, sp) in &ordre {
            if noms_de(cle, sp)?.iter().any(|n| cle_recherche(n) == q) {
                return Ok(avec_cle(cle));
            }
            if sp.get("numero_objet").and_then(Value::as_str).unwrap_or("").to_uppercase() == net.to_uppercase() {
                return Ok(avec_cle(cle));
            }
        }
        for (cle, sp) in &ordre {
            let ressemble = noms_de(cle, sp)?.iter().filter(|n| !n.is_empty()).any(|n| {
                let c = cle_recherche(n);
                c.contains(&q) || q.contains(&c)
            });
            if ressemble {
                return Ok(avec_cle(cle));
            }
        }
        let mut titres = Vec::new();
        for v in self.spectacles.values() {
            titres.push(exiger_texte(v, "titre")?);
        }
        Err(format!(
            "Spectacle inconnu : {}. Connus : {}.\nAjouter une entree dans config/spectacles.json avec son numero d'objet \
France Travail.",
            repr_texte(nom),
            titres.join(", ")
        ))
    }
}

/// `re.fullmatch(r"[12]\d{4}(\d{2}|2A|2B)\d{6}(\d{2})?", brut)`.
fn forme_de_nir(s: &str) -> bool {
    let c: Vec<char> = s.chars().collect();
    if !(c.len() == 13 || c.len() == 15) || !matches!(c[0], '1' | '2') {
        return false;
    }
    let chiffres = |de: usize, a: usize| c[de..a].iter().all(char::is_ascii_digit);
    let dep = chiffres(5, 7) || (c[5] == '2' && matches!(c[6], 'A' | 'B'));
    chiffres(1, 5) && dep && chiffres(7, 13) && (c.len() == 13 || chiffres(13, 15))
}
