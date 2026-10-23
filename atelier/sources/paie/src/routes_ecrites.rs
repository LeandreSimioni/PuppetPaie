//! Les routes qui ECRIVENT, et celles qui relevent chez l'organisme
//! (`moteur/web.py`, `operations_exercice`, `operations_fiches`,
//! `operations_regularisation`, `operations_declarations`) :
//! `revue-spectacles`, la pose de `taux-atmp`, `regularisation-mois`,
//! `bilan-repetition` quand le compte rendu est « attendu ».
//!
//! Meme JSON que la route Python, memes refus sous la meme classe d'exception
//! (`paie_rust.route` les releve), memes fichiers ecrits, aux memes octets.
//!
//! - `revue-spectacles` archive les spectacles qui ne continuent pas
//!   (`activer_fiche`, avec la copie horodatee de `spectacles.json`) et coche
//!   l'echeance de la revue (`marquer_echeance`).
//! - `taux-atmp` : la POSE (`poser_taux_atmp`) ; la lecture est dans `routes_fiches`.
//! - `regularisation-mois` : l'etat du mois, ses corrections ecrites, et les
//!   engagements tels que declares (calcul de chaque paie, sans numero neuf).
//! - `bilan-repetition` : la lecture seulement ; le releve des retours (la part qui
//!   releve chez l'organisme et ecrit) reste en Python, a porter plus tard.

use std::collections::HashMap;

use serde_json::{json, Map, Value};

use crate::contexte::Contexte;
use crate::ecriture::ecrire_json;
use crate::etats::{self, eq_py, erreur, getd, items, nom_type, py_repr, py_str, R};
use crate::fichiers::{repr, vrai};
use crate::retours::Echec;
use crate::routes_fiches::{entier, exiger};
use crate::routes_mois::Moteur;
use crate::routes_regularisation as regul;
use crate::routes_saisie::lire_saisie;

/// Les routes que ce module sert.
pub const ROUTES: [&str; 2] = ["revue-spectacles", "regularisation-mois"];

/// Une route de ce module.
pub fn route(o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "revue-spectacles" => revue_spectacles(o, ctx, charge),
        "regularisation-mois" => regularisation_mois(o, ctx, charge),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

fn plain(m: impl Into<String>) -> Echec {
    Echec { classe: None, message: m.into() }
}

fn en_os(m: String) -> Echec {
    erreur("OSError", m)
}

/// `Path.mkdir(parents=True, exist_ok=True)`.
fn creer_dossier(d: &std::path::Path) -> R<()> {
    std::fs::create_dir_all(d).map_err(|e| erreur("OSError", format!("{} : {}", d.display(), e)))
}

// ------------------------------------------------------------- revue-spectacles

/// `operations_exercice.revue_spectacles` : archive les spectacles actifs qui ne sont pas dans
/// `continuent`, note la revue de l'annee faite, rend la configuration.
fn revue_spectacles(o: &HashMap<String, String>, ctx: &Contexte, charge: &Value) -> R<Value> {
    let annee = entier(exiger(charge, "annee")?)?;
    let continuent = etats::ou(getd(charge, "continuent", &etats::LISTE_VIDE)?, &etats::LISTE_VIDE);
    // `set(continuent)` : l'ordre ne compte pas, les doublons non plus.
    let mut garder: Vec<Value> = Vec::new();
    for c in items(continuent)? {
        if matches!(c.as_ref(), Value::Array(_) | Value::Object(_)) {
            return Err(erreur("TypeError", crate::fichiers::pas_une_cle(nom_type(&c))));
        }
        if !garder.iter().any(|g| eq_py(g, &c)) {
            garder.push(c.into_owned());
        }
    }
    let actifs = ctx.spectacles_actifs();
    let est_actif = |v: &Value| v.as_str().is_some_and(|s| actifs.contains(&s));
    let mut inconnus: Vec<&Value> = garder.iter().filter(|g| !est_actif(g)).collect();
    if !inconnus.is_empty() {
        if let Some((i, autre)) = inconnus.iter().enumerate().find(|(_, c)| !c.is_string()) {
            return Err(erreur("TypeError", crate::fichiers::element(i, &crate::fichiers::attendu("un texte", nom_type(autre)))));
        }
        inconnus.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
        let noms: Vec<&str> = inconnus.iter().filter_map(|c| c.as_str()).collect();
        return Err(erreur("ErreurOperation", format!("Spectacle actif inconnu : {}.", noms.join(", "))));
    }
    let archives: Vec<String> = actifs.iter().filter(|c| !garder.iter().any(|g| g.as_str() == Some(c))).map(|c| c.to_string()).collect();
    for cle in &archives {
        crate::routes_ecritures::activer_fiche(ctx, "spectacle", cle, false)?;
    }
    marquer_echeance(ctx, &format!("revue-spectacles:{}", annee), true, "interface")?;
    let mut continuent: Vec<String> = garder.iter().filter_map(|g| g.as_str().map(String::from)).collect();
    continuent.sort();
    let neuf = Contexte::nouveau(&ctx.racine, None).map_err(plain)?;
    let configuration = crate::routes_accueil::route(o, &neuf, "configuration", &json!({}))?;
    Ok(json!({"annee": annee, "archives": archives, "continuent": continuent, "configuration": configuration}))
}

/// `operations_echeancier.marquer_echeance` : coche (ou decoche) une echeance que seul
/// l'utilisateur peut dire faite.
pub(crate) fn marquer_echeance(ctx: &Contexte, cle: &str, fait: bool, qui: &str) -> R<()> {
    let (quoi, annee) = match cle.split_once(':') {
        Some((q, a)) => (q, a),
        None => (cle, ""),
    };
    let base = lire_saisie(&crate::fichiers::dossier_du_code().join("echeancier-base.json"))?;
    let annuelles = base.get("annuelles").and_then(Value::as_array).ok_or_else(|| erreur("KeyError", crate::fichiers::absent("annuelles")))?;
    let e = annuelles.iter().find(|x| x.get("cle").and_then(Value::as_str) == Some(quoi));
    let chiffres = !annee.is_empty() && annee.chars().all(|c| c.is_ascii_digit());
    let Some(e) = e.filter(|_| chiffres) else {
        return Err(erreur("ErreurOperation", format!("Echeance inconnue : {}.", repr(cle))));
    };
    if e.get("fait_par").and_then(Value::as_str) != Some("coche") {
        let titre = e.get("titre").and_then(Value::as_str).unwrap_or("");
        let n: i64 = annee.parse().unwrap_or(0);
        let titre = titre.replace("{annee}", annee).replace("{annee_precedente}", &(n - 1).to_string()).replace("{jour_dsn}", "");
        return Err(erreur(
            "ErreurOperation",
            format!("« {} » ne se coche pas : le logiciel le voit fait quand c'est fait.", titre),
        ));
    }
    let p = ctx.racine.join("config").join("echeancier.json");
    let mut d = if p.exists() { lire_saisie(&p)? } else { json!({}) };
    let Value::Object(m) = &mut d else {
        return Err(erreur("AttributeError", crate::fichiers::attendu("un objet", nom_type(&d))));
    };
    m.entry("_origine").or_insert_with(|| json!("Les echeances que l'utilisateur a dites faites (operations_echeancier.marquer_echeance)."));
    let faits = m.entry("fait").or_insert_with(|| json!({}));
    let Value::Object(faits) = faits else {
        return Err(match (faits, fait) {
            (Value::Array(_), true) => erreur("TypeError", format!("'fait' : {}", crate::fichiers::attendu("un objet", "list"))),
            (autre, true) => erreur("TypeError", crate::fichiers::non_modifiable(nom_type(autre))),
            (autre, false) => erreur("AttributeError", crate::fichiers::attendu("un objet", nom_type(autre))),
        });
    };
    if fait {
        faits.insert(cle.to_string(), json!({"le": ctx.aujourdhui.iso(), "qui": qui}));
    } else {
        faits.shift_remove(cle);
    }
    if let Some(parent) = p.parent() {
        creer_dossier(parent)?;
    }
    ecrire_json(&p, &d, 2).map_err(en_os)
}

// ------------------------------------------------------------ la pose du taux AT/MP

/// Un `Decimal` de Python : de quoi le lire, le comparer a 0 et 20, l'ecrire comme `str()`, le
/// convertir en flottant.
struct Dec {
    negatif: bool,
    chiffres: String,
    exposant: i64,
    /// `Infinity` ou `NaN`.
    special: Option<&'static str>,
}

impl Dec {
    fn nul(&self) -> bool {
        self.special.is_none() && self.chiffres == "0"
    }

    /// `Decimal.__str__`.
    fn texte(&self) -> String {
        let signe = if self.negatif { "-" } else { "" };
        if let Some(s) = self.special {
            return format!("{}{}", signe, s);
        }
        let (d, e) = (&self.chiffres, self.exposant);
        let n = d.len() as i64;
        let ajuste = e + n - 1;
        let corps = if e <= 0 && ajuste >= -6 {
            if e == 0 {
                d.clone()
            } else if n > -e {
                format!("{}.{}", &d[..(n + e) as usize], &d[(n + e) as usize..])
            } else {
                format!("0.{}{}", "0".repeat((-e - n) as usize), d)
            }
        } else {
            let mantisse = if n > 1 { format!("{}.{}", &d[..1], &d[1..]) } else { d.clone() };
            format!("{}E{}{}", mantisse, if ajuste < 0 { "-" } else { "+" }, ajuste.abs())
        };
        format!("{}{}", signe, corps)
    }

    /// `0 < v < 20`.
    fn entre_0_et_20(&self) -> bool {
        if self.special.is_some() || self.negatif || self.nul() {
            return false;
        }
        let ajuste = self.exposant + self.chiffres.len() as i64 - 1;
        match ajuste {
            a if a < 1 => true,
            1 => {
                let d = self.chiffres.trim_end_matches('0');
                d.starts_with('1')
            }
            _ => false,
        }
    }

    /// `float(v)`.
    fn flottant(&self) -> f64 {
        let t = format!("{}{}e{}", if self.negatif { "-" } else { "" }, self.chiffres, self.exposant);
        t.parse::<f64>().unwrap_or(0.0)
    }
}

/// Le chiffre (0 a 9) que `c` represente pour Python (`int`, `Decimal`) : les chiffres ASCII, et
/// ceux des ecritures les plus courantes (arabe, devanagari, pleine chasse).
pub(crate) fn chiffre_py(c: char) -> Option<u32> {
    const DEBUTS: [u32; 4] = [0x0660, 0x06F0, 0x0966, 0xFF10];
    let u = c as u32;
    if c.is_ascii_digit() {
        return Some(u - 0x30);
    }
    DEBUTS.iter().find(|d| u >= **d && u < **d + 10).map(|d| u - d)
}

/// Des chiffres (les `_` s'ignorent, comme dans `_decimal`), en ASCII ; `None` s'il n'y en a pas
/// ou si autre chose s'y trouve.
fn chiffres_py(t: &str) -> Option<String> {
    let mut out = String::new();
    for x in t.chars().filter(|x| *x != '_') {
        out.push(char::from(b'0' + chiffre_py(x)? as u8));
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// `Decimal(texte)` ; `None` quand Python leve `InvalidOperation`.
fn decimal_py(texte: &str) -> Option<Dec> {
    let sans: String = texte.chars().filter(|c| *c != '_').collect();
    let t = sans.trim_matches(crate::retours::blanc_py);
    let (negatif, reste) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let bas = reste.to_lowercase();
    if bas == "inf" || bas == "infinity" {
        return Some(Dec { negatif, chiffres: "0".into(), exposant: 0, special: Some("Infinity") });
    }
    for pref in ["snan", "nan"] {
        if let Some(suite) = bas.strip_prefix(pref) {
            if suite.chars().all(|c| chiffre_py(c).is_some()) {
                return Some(Dec { negatif, chiffres: "0".into(), exposant: 0, special: Some("NaN") });
            }
        }
    }
    let (mantisse, exp) = match reste.find(['e', 'E']) {
        Some(i) => (&reste[..i], Some(&reste[i + 1..])),
        None => (reste, None),
    };
    let (entier, frac) = match mantisse.split_once('.') {
        Some((a, b)) => (a, b),
        None => (mantisse, ""),
    };
    let entier = if entier.is_empty() { String::new() } else { chiffres_py(entier)? };
    let frac = if frac.is_empty() { String::new() } else { chiffres_py(frac)? };
    if entier.is_empty() && frac.is_empty() {
        return None;
    }
    let exposant_lu: i64 = match exp {
        None => 0,
        Some(e) => {
            let (neg, e) = match e.strip_prefix('-') {
                Some(r) => (true, r),
                None => (false, e.strip_prefix('+').unwrap_or(e)),
            };
            let n: i64 = chiffres_py(e)?.parse().ok()?;
            if neg {
                -n
            } else {
                n
            }
        }
    };
    let tout = format!("{}{}", entier, frac);
    let sans_zeros = tout.trim_start_matches('0');
    let chiffres = if sans_zeros.is_empty() { "0".to_string() } else { sans_zeros.to_string() };
    Some(Dec { negatif, chiffres, exposant: exposant_lu - frac.len() as i64, special: None })
}

/// Le jour d'aujourd'hui a Paris, `JJ/MM/AAAA` (`date.today().strftime("%d/%m/%Y")`).
fn aujourdhui_jj_mm_aaaa() -> String {
    let t = crate::temps::secondes();
    let iso = crate::paris::iso_secondes(t);
    format!("{}/{}/{}", &iso[8..10], &iso[5..7], &iso[..4])
}

/// `operations_fiches.poser_taux_atmp` : pose le taux AT/MP de l'annee dans la surcouche de la
/// compagnie (`config/baremes/<annee>.json`), le reste de la surcouche est garde.
pub(crate) fn poser_taux_atmp(ctx: &Contexte, an: i64, valeur: &Value, code_risque: &Value) -> R<()> {
    let brut = py_str(valeur).replace(',', ".").replace('%', "");
    let Some(v) = decimal_py(&brut) else {
        return Err(erreur("ErreurOperation", format!("Taux AT/MP illisible : {}.", py_repr(valeur))));
    };
    if v.special == Some("NaN") {
        return Err(erreur("InvalidOperation", format!("Taux AT/MP illisible : {}.", py_repr(valeur))));
    }
    if !v.entre_0_et_20() {
        return Err(erreur(
            "ErreurOperation",
            format!("Taux AT/MP de {} % : il se donne en pourcentage, par exemple 1,25 pour 1,25 %.", v.texte()),
        ));
    }
    let code = match code_risque {
        Value::String(s) => s.trim_matches(crate::retours::blanc_py).to_uppercase(),
        autre if !vrai(autre) => String::new(),
        autre => return Err(erreur("AttributeError", crate::fichiers::attendu("un texte", nom_type(autre)))),
    };
    if !code.is_empty() && !code_risque_valide(&code) {
        return Err(erreur(
            "ErreurOperation",
            format!(
                "Code risque « {} » : trois chiffres et deux caractères, comme 923AD, tel qu'il figure sur la notification.",
                code
            ),
        ));
    }
    let p = ctx.racine.join("config").join("baremes").join(format!("{}.json", an));
    let lu = if p.exists() { lire_saisie(&p)? } else { json!({}) };
    let mut data = if vrai(&lu) { lu } else { json!({"annee": an, "socle": an}) };
    let Value::Object(d) = &mut data else {
        return Err(erreur("AttributeError", crate::fichiers::attendu("un objet", nom_type(&data))));
    };
    let ancien: Map<String, Value> = match d.get("taux_atmp") {
        Some(Value::Object(a)) => a.clone(),
        _ => Map::new(),
    };
    let mut nouveau: Map<String, Value> = ancien.iter().filter(|(k, _)| k.starts_with('_')).map(|(k, w)| (k.clone(), w.clone())).collect();
    let ancien_code = ancien.get("code_risque").filter(|c| vrai(c)).cloned();
    let code_final = if !code.is_empty() {
        Value::String(code)
    } else {
        ancien_code.unwrap_or_else(|| json!(""))
    };
    nouveau.insert("valeur".into(), serde_json::Number::from_f64(v.flottant()).map_or(Value::Null, Value::Number));
    nouveau.insert("code_risque".into(), code_final.clone());
    nouveau.insert("_source".into(), json!(format!("Saisi dans PuppetPaie le {} (notification AT/MP).", aujourdhui_jj_mm_aaaa())));
    if !vrai(&code_final) {
        nouveau.shift_remove("code_risque");
    }
    d.insert("taux_atmp".into(), Value::Object(nouveau));
    if let Some(parent) = p.parent() {
        creer_dossier(parent)?;
    }
    if p.exists() {
        crate::routes_contrats::sauver_avant(&p)?;
    }
    ecrire_json(&p, &data, 2).map_err(en_os)
}

/// `re.fullmatch(r"\d{3}[0-9A-Z]{2}", code)`.
fn code_risque_valide(code: &str) -> bool {
    let c: Vec<char> = code.chars().collect();
    c.len() == 5 && c[..3].iter().all(|x| x.is_ascii_digit()) && c[3..].iter().all(|x| x.is_ascii_digit() || x.is_ascii_uppercase())
}

// ------------------------------------------------------- les corrections ecrites

/// Les champs d'une `Correction`, dans l'ordre de la classe.
const CHAMPS_CORRECTION: [&str; 11] =
    ["mois", "numero", "salarie", "debut", "motif", "engagement", "decidee_le", "qui", "preparee_par", "portee_par", "annulee_le"];

/// Les champs qu'une correction ne porte que s'ils disent quelque chose (21/10/2026) : `ajout` (un contrat ajoute apres
/// la limite, `routes_regularisation::apercu_d_un_ajout`), `remplacee_par` (le numero de la correction qui l'a
/// remplacee, en attente, quand la paie a encore change).
const CHAMPS_SI_PRESENTS: [&str; 2] = ["ajout", "remplacee_par"];

/// Une `Correction` : les champs, dans l'ordre, ceux qui manquent a `None` ; une cle inconnue ou
/// une cle obligatoire absente est un `TypeError`.
fn correction_de(c: &Value) -> R<Map<String, Value>> {
    let Value::Object(o) = c else {
        return Err(erreur("TypeError", format!("Correction : un objet est attendu, pas {}", nom_type(c))));
    };
    if let Some(k) = o.keys().find(|k| !CHAMPS_CORRECTION.contains(&k.as_str()) && !CHAMPS_SI_PRESENTS.contains(&k.as_str())) {
        return Err(erreur("TypeError", format!("Correction : champ inconnu {}", repr(k))));
    }
    let manque: Vec<String> = CHAMPS_CORRECTION[..8].iter().filter(|k| !o.contains_key(**k)).map(|k| repr(k)).collect();
    if !manque.is_empty() {
        return Err(erreur("TypeError", format!("Correction : champ manquant {}", manque.join(", "))));
    }
    let mut c: Map<String, Value> = CHAMPS_CORRECTION.iter().map(|k| (k.to_string(), o.get(*k).cloned().unwrap_or(Value::Null))).collect();
    for k in CHAMPS_SI_PRESENTS {
        if let Some(v) = o.get(k) {
            c.insert(k.to_string(), v.clone());
        }
    }
    Ok(c)
}

/// `lire_corrections(ctx, mois)`.
pub(crate) fn lire_corrections(ctx: &Contexte, mois: &str) -> R<Vec<Map<String, Value>>> {
    let p = ctx.racine.join("regularisations").join(format!("{}.json", mois));
    if !p.exists() {
        return Ok(Vec::new());
    }
    let d = lire_saisie(&p)?;
    let corrections = getd(&d, "corrections", &etats::LISTE_VIDE)?;
    items(corrections)?.iter().map(|c| correction_de(c)).collect()
}

/// `toutes_les_corrections(ctx)` : un fichier `AAAA-MM.json` apres l'autre.
pub(crate) fn toutes_les_corrections(ctx: &Contexte) -> R<Vec<Map<String, Value>>> {
    let dossier = ctx.racine.join("regularisations");
    if !dossier.exists() {
        return Ok(Vec::new());
    }
    let mut noms: Vec<String> = match std::fs::read_dir(&dossier) {
        Ok(l) => l.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect(),
        Err(_) => Vec::new(),
    };
    noms.retain(|n| {
        let c: Vec<char> = n.chars().collect();
        c.len() == 12 && c[4] == '-' && n.ends_with(".json")
    });
    noms.sort();
    let mut out = Vec::new();
    for n in noms {
        out.extend(lire_corrections(ctx, &n[..n.len() - 5])?);
    }
    Ok(out)
}

/// `Correction.etat()`.
pub(crate) fn etat_correction(c: &Map<String, Value>) -> &'static str {
    let vrai_de = |k: &str| c.get(k).is_some_and(vrai);
    if vrai_de("annulee_le") {
        "annulee"
    } else if vrai_de("portee_par") {
        "portee"
    } else if vrai_de("preparee_par") {
        "preparee"
    } else {
        "en attente"
    }
}

/// `corrections_par_mois(ctx)`, reduit au mois voulu : les corrections dont le mois d'origine est
/// `mois`, chacune avec son etat.
fn corrections_du_mois(ctx: &Contexte, mois: &Value) -> R<Vec<Value>> {
    let mut out = Vec::new();
    for c in toutes_les_corrections(ctx)? {
        let m = c.get("mois").cloned().unwrap_or(Value::Null);
        if matches!(m, Value::Array(_) | Value::Object(_)) {
            return Err(erreur("TypeError", crate::fichiers::pas_une_cle(nom_type(&m))));
        }
        if eq_py(&m, mois) {
            let etat = etat_correction(&c);
            let mut v = c;
            v.insert("etat".into(), json!(etat));
            out.push(Value::Object(v));
        }
    }
    Ok(out)
}

// ----------------------------------------------------------- regularisation-mois

/// `engagements_d_origine` : les engagements du mois d'origine tels que DECLARES -- ou tels que la
/// derniere correction PORTEE les a laisses --, avec leur cle (salarie, debut) et l'etat de leur
/// correction en cours s'il y en a une.
fn engagements_d_origine(o: &HashMap<String, String>, m: &mut Moteur, ctx: &Contexte, mois: &str) -> R<Vec<Value>> {
    let donnees = regul::saisie_d_origine(ctx, mois)?;
    let mut paies = regul::paies_de(o, ctx, &donnees)?;
    regul::poser_numeros(m, ctx, mois, &mut paies)?;
    let mut portees: Vec<((Value, Value), Map<String, Value>)> = Vec::new();
    let mut en_cours: Vec<((Value, Value), Map<String, Value>)> = Vec::new();
    let poser = |liste: &mut Vec<((Value, Value), Map<String, Value>)>, cle: (Value, Value), c: Map<String, Value>| {
        match liste.iter_mut().find(|(k, _)| eq_py(&k.0, &cle.0) && eq_py(&k.1, &cle.1)) {
            Some(existant) => existant.1 = c,
            None => liste.push((cle, c)),
        }
    };
    for c in lire_corrections(ctx, mois)? {
        let cle = (c["salarie"].clone(), c["debut"].clone());
        if matches!(cle.0, Value::Array(_) | Value::Object(_)) || matches!(cle.1, Value::Array(_) | Value::Object(_)) {
            return Err(erreur("TypeError", crate::fichiers::pas_une_cle(nom_type(if cle.0.is_array() || cle.0.is_object() { &cle.0 } else { &cle.1 }))));
        }
        if c.get("portee_par").is_some_and(vrai) {
            poser(&mut portees, cle, c);
        } else if !c.get("annulee_le").is_some_and(vrai) {
            poser(&mut en_cours, cle, c);
        }
    }
    let engs = donnees.get("engagements").ok_or_else(|| erreur("KeyError", crate::fichiers::absent("engagements")))?;
    let engs = items(engs)?;
    let trouve = |liste: &'_ [((Value, Value), Map<String, Value>)], cle: &(Value, Value)| -> Option<usize> {
        liste.iter().position(|(k, _)| eq_py(&k.0, &cle.0) && eq_py(&k.1, &cle.1))
    };
    // LA SAISIE MODIFIEE APRES LA LIMITE (21/10/2026) : la correction d'un contrat dont la paie a change part de la saisie
    // d'aujourd'hui (`engagement_saisi`), la page s'en sert pour preremplir ; le declare reste ce qui est declare.
    let modifies = regul::ce_qui_a_change(o, ctx, mois)?.map(|c| c.contrats).unwrap_or_default();
    let mut out = Vec::new();
    for (p, e) in paies.iter().zip(engs.iter()) {
        let cle_s = py_str(regul::requis(&p.salarie, "_cle")?);
        let modifie = modifies.iter().find(|c| !c.ajoute && c.salarie == cle_s && c.debut == p.engagement.debut.iso());
        let saisi = modifie.map(|c| c.engagement.clone());
        // SES DATES CHANGEES DANS LA SAISIE (21/10/2026) : la correction les reprend, la page le dit.
        let dates_saisies = modifie.filter(|c| c.dates_changees).map(|c| json!(regul::bornes_en_clair(&c.bornes))).unwrap_or(Value::Null);
        let cle = (Value::String(cle_s.clone()), Value::String(p.engagement.debut.iso()));
        let portee = trouve(&portees, &cle);
        let declare = match portee {
            Some(i) => portees[i].1["engagement"].clone(),
            None => e.as_ref().clone(),
        };
        let sal = &p.salarie;
        let prenom = match sal.get("prenom_affichage").filter(|v| vrai(v)) {
            Some(v) => py_str(v),
            None => py_str(regul::requis(sal, "prenom")?),
        };
        let en_cours_ici = trouve(&en_cours, &cle).map(|i| Value::Object(en_cours[i].1.clone())).unwrap_or(Value::Null);
        out.push(json!({
            "salarie": cle_s,
            "debut": p.engagement.debut.iso(),
            "nom": format!("{} {}", py_str(regul::requis(sal, "nom")?), prenom),
            "spectacle": p.titre()?,
            "numero": p.engagement.numero,
            "fin": p.engagement.fin.map(|f| f.iso()),
            "engagement": declare,
            "brut": regul::q2(p.brut),
            "net_a_payer": regul::q2(p.net_a_payer),
            "deja_corrige": portee.is_some(),
            "correction_en_cours": en_cours_ici,
            "engagement_saisi": saisi.unwrap_or(Value::Null),
            "dates_saisies": dates_saisies,
            "ajoute": false,
        }));
    }
    // LES CONTRATS AJOUTES APRES LA LIMITE (un contrat oublie) : rien de declare ; leur correction les declare en entier
    // dans la DSN suivante (`routes_regularisation::apercu_correction`).
    for a in modifies.iter().filter(|c| c.ajoute) {
        let p = regul::paie_d_un_engagement_en(o, m, ctx, &donnees, &a.engagement, true)?;
        let sal = &p.salarie;
        let prenom = match sal.get("prenom_affichage").filter(|v| vrai(v)) {
            Some(v) => py_str(v),
            None => py_str(regul::requis(sal, "prenom")?),
        };
        let cle = (Value::String(a.salarie.clone()), Value::String(a.debut.clone()));
        let en_cours_ici = trouve(&en_cours, &cle).map(|i| Value::Object(en_cours[i].1.clone())).unwrap_or(Value::Null);
        out.push(json!({
            "salarie": a.salarie,
            "debut": a.debut,
            "nom": format!("{} {}", py_str(regul::requis(sal, "nom")?), prenom),
            "spectacle": p.titre()?,
            "numero": p.engagement.numero,
            "fin": p.engagement.fin.map(|f| f.iso()),
            "engagement": a.engagement,
            "brut": "0.00",
            "net_a_payer": "0.00",
            "deja_corrige": false,
            "correction_en_cours": en_cours_ici,
            "engagement_saisi": a.engagement,
            "ajoute": true,
        }));
    }
    Ok(out)
}

/// `regularisation-mois` : l'etat du mois, ses corrections ecrites, les engagements declares
/// quand le mois se corrige.
fn regularisation_mois(o: &HashMap<String, String>, ctx: &Contexte, charge: &Value) -> R<Value> {
    let mois_v = exiger(charge, "mois")?;
    let mut m = Moteur::nouveau(o, ctx);
    let etat = regul::etat_mois(&mut m, ctx, mois_v)?;
    let ecrites = corrections_du_mois(ctx, mois_v)?;
    let coffre = crate::coffre::Coffre::du_poste(&ctx.racine, "");
    let mot_de_passe_pose = coffre.ouverture_exigee().map_err(crate::routes_etats::vers_route)?;
    let engagements = if etat["regularisable"].as_bool().unwrap_or(false) {
        engagements_d_origine(o, &mut m, ctx, &py_str(mois_v))?
    } else {
        Vec::new()
    };
    let corrections = match ecrites.first() {
        Some(Value::Object(_)) => Value::Array(ecrites),
        _ => json!([]),
    };
    Ok(json!({
        "mois": mois_v,
        "etat": etat,
        "mot_de_passe_pose": mot_de_passe_pose,
        "engagements": engagements,
        "corrections": corrections,
    }))
}
