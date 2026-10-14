//! Les clauses d'une convention collective qui changent la fin d'un contrat
//! (`moteur/clauses.py`) : preavis, indemnites de licenciement et de retraite,
//! maintien de salaire, prime d'anciennete.
//!
//! Le module rend ce que dit la convention, ou `None` quand elle ne dit rien
//! ou n'est pas relevee ; il ne choisit pas entre la convention et la loi.
//! Les nombres vivent dans `clauses-conventionnelles-base.json`.
//!
//! Les `dict` du Python sont des `serde_json::Value` ; un `Decimal` y est
//! ecrit en chaine (`"149.66"`). Les phrases rendues sont celles du Python,
//! au caractere pres.

use crate::date::{en_texte, Jour};
use crate::emplois;
use crate::fichiers::{dossier_du_code, json_en_cache, repr, repr_valeur, vrai};
use crate::num::{dec, Decimal, Resultat};
pub use crate::d28::P;
use serde_json::{json, Map, Value};
use std::cmp::Ordering;
use std::sync::Arc;



static NUL: Value = Value::Null;

// ------------------------------------------------------------ les aides
/// `v.get(k)` de Python : `None` (ici `Null`) quand la cle manque.
fn get<'a>(v: &'a Value, k: &str) -> &'a Value {
    v.get(k).unwrap_or(&NUL)
}

/// `v[k]` : une cle absente est un `KeyError`.
fn cle<'a>(v: &'a Value, k: &str) -> Resultat<&'a Value> {
    v.get(k).ok_or_else(|| crate::fichiers::absent(k))
}

/// `a or b`.
fn ou<'a>(a: &'a Value, b: &'a Value) -> &'a Value {
    if vrai(a) {
        a
    } else {
        b
    }
}

/// `P(str(x))`.
fn nb(v: &Value) -> Resultat<P> {
    dec(v).map(P::depuis)
}

/// `P(str(v[k]))`.
fn nombre(v: &Value, k: &str) -> Resultat<P> {
    nb(cle(v, k)?)
}

fn tab(v: &Value) -> Resultat<&Vec<Value>> {
    v.as_array().ok_or_else(|| format!("liste attendue, lu {}", v))
}

/// `int(x)`.
fn entier(v: &Value) -> Resultat<i64> {
    match v {
        Value::Number(n) => n
            .as_i64()
            .or_else(|| n.as_f64().map(|x| x.trunc() as i64))
            .ok_or_else(|| crate::fichiers::entier_hors_limites(&n.to_string())),
        Value::String(s) => s
            .trim()
            .parse::<i64>()
            .map_err(|_| format!("nombre entier illisible : {}", repr(s))),
        autre => Err(crate::fichiers::attendu("un nombre entier", crate::etats::nom_type(autre))),
    }
}

fn dv(x: P) -> Value {
    Value::String(x.to_string())
}

fn add(a: P, b: P) -> P {
    a.add(b)
}

fn sub(a: P, b: P) -> P {
    a.sub(b)
}

fn mul(a: P, b: P) -> P {
    a.mul(b)
}

fn div(a: P, b: P) -> P {
    a.div(b)
}

fn eur2(x: P) -> P {
    x.eur2()
}

fn quantifier(x: P, dp: i32) -> P {
    x.quantifier(dp)
}

fn plancher(x: P) -> P {
    x.plancher()
}

/// `max(P("0"), x)`.
fn positif(x: P) -> P {
    if x > P::ZERO {
        x
    } else {
        P::ZERO
    }
}

fn comparer(a: &Value, b: &Value, k: &str) -> Resultat<Ordering> {
    Ok(nombre(a, k)?.cmp(&nombre(b, k)?))
}

fn texte(v: &Value) -> String {
    en_texte(v)
}

/// `str(x or "").strip()` d'un IDCC ou d'une annexe.
fn lu(v: &Value) -> String {
    if vrai(v) {
        texte(v).trim().to_string()
    } else {
        String::new()
    }
}

// ------------------------------------------------------------- la table
const FICHIER: &str = "clauses-conventionnelles-base.json";

pub fn table() -> Resultat<Arc<Value>> {
    json_en_cache(&dossier_du_code().join(FICHIER))
}

/// Les clauses d'une convention dans la table, ou `Null`.
fn conv<'a>(t: &'a Value, idcc: &Value) -> &'a Value {
    get(get(t, "conventions"), &lu(idcc))
}

/// Les clauses relevees de cette convention, ou `None` si elles ne le sont
/// pas (ou si la compagnie n'a pas d'IDCC).
pub fn releve(idcc: &Value) -> Resultat<Option<Value>> {
    let t = table()?;
    Ok(Some(conv(&t, idcc)).filter(|r| !r.is_null()).cloned())
}

/// Pour les phrases : « la convention 3090 », ou « la convention collective »
/// quand l'IDCC manque.
pub fn nom(idcc: &Value) -> String {
    let i = lu(idcc);
    if i.is_empty() {
        "la convention collective".to_string()
    } else {
        format!("la convention {}", i)
    }
}

fn non_relevees(idcc: &Value) -> String {
    format!("les clauses de {} ne sont pas relevees", nom(idcc))
}

// ------------------------------------------------------------ la categorie
/// La categorie conventionnelle d'un salarie pour CET engagement :
/// `{categorie, grille, source}` ; `categorie` vaut `null` quand rien ne
/// permet de la lire sans deviner, et `source` dit alors pourquoi.
pub fn profil(idcc: &Value, fiche: &Value, engagement: Option<&Value>) -> Resultat<Value> {
    let t = table()?;
    let e = engagement.unwrap_or(&NUL);
    let sec = get(conv(&t, idcc), "categories");
    let valeurs = get(sec, "valeurs");
    let code = ou(get(e, "metier"), get(fiche, "code_emploi_ft"));
    let m = if vrai(code) {
        let c = code.as_str().ok_or_else(|| format!("code emploi : {}", crate::fichiers::attendu("un texte", crate::etats::nom_type(code))))?;
        emplois::metier(c)?
    } else {
        None
    };
    let mv = m.as_ref().unwrap_or(&NUL);
    let c3090 = get(mv, "classification_3090");
    let grille = get(c3090, "grille").clone();

    let dit = ou(get(e, "categorie_conventionnelle"), get(fiche, "categorie_conventionnelle"));
    if vrai(dit) {
        let connue = dit.as_str().map_or(false, |s| valeurs.get(s).is_some());
        if vrai(valeurs) && !connue {
            let cles: Vec<&str> = valeurs.as_object().map_or(vec![], |o| o.keys().map(|k| k.as_str()).collect());
            return Ok(json!({"categorie": null, "grille": grille,
                "source": format!("categorie_conventionnelle {} inconnue de {} (valeurs : {})",
                    repr_valeur(dit), nom(idcc), cles.join(", "))}));
        }
        return Ok(json!({"categorie": dit, "grille": grille,
            "source": "la fiche (categorie_conventionnelle)"}));
    }

    let statut = if vrai(get(e, "statut")) {
        get(e, "statut")
    } else if vrai(get(e, "metier")) && vrai(get(mv, "statut")) {
        get(mv, "statut")
    } else {
        get(fiche, "statut")
    };
    if statut.as_str() == Some("artiste") {
        return Ok(json!({"categorie": "artiste", "grille": grille, "source": "le statut artiste"}));
    }
    let est_cadre = if !get(e, "cadre").is_null() {
        vrai(get(e, "cadre"))
    } else {
        lu_brut(get(fiche, "statut_categoriel_rc")) == "01"
    };
    let niveau = lu_brut(get(c3090, "niveau"));
    let selon_niveau = get(get(sec, "par_niveau_3090"), &niveau);
    if est_cadre {
        return Ok(json!({"categorie": "cadre", "grille": grille, "source": "la case cadre"}));
    }
    if vrai(get(sec, "cadre_ou_non_cadre")) {
        return Ok(json!({"categorie": "non_cadre", "grille": grille,
            "source": "la case cadre, non cochee"}));
    }
    if selon_niveau.as_str() == Some("cadre") {
        return Ok(json!({"categorie": null, "grille": grille,
            "source": format!("le metier {} est de niveau {} dans la grille 3090, et la fiche le \
                declare NON cadre : agent de maitrise ou employe, le moteur ne le devine pas",
                texte(code), niveau)}));
    }
    if vrai(selon_niveau) {
        return Ok(json!({"categorie": selon_niveau, "grille": grille,
            "source": format!("le niveau 3090 du metier {} ({})", texte(code), niveau)}));
    }
    let code_dit = if vrai(code) { texte(code) } else { "aucun code metier".to_string() };
    Ok(json!({"categorie": null, "grille": grille,
        "source": format!("ni la fiche, ni le metier ({}) ne disent la categorie", code_dit)}))
}

/// `str(x or "")`, sans `strip`.
fn lu_brut(v: &Value) -> String {
    if vrai(v) {
        texte(v)
    } else {
        String::new()
    }
}

// --------------------------------------------------------------- le preavis
/// Le preavis de la convention : `{mois, article, pourquoi}` (et `jours` pour
/// un preavis dit en jours). `mois` vaut `null` quand la convention ne dit
/// rien -- pas relevee, ou pas de categorie -- et `pourquoi` le dit ; 0 quand
/// le motif n'ouvre pas de preavis.
pub fn preavis_mois(
    idcc: &Value,
    categorie: Option<&str>,
    anciennete: Decimal,
    motif: Option<&str>,
    age: Option<Decimal>,
) -> Resultat<Value> {
    let (anciennete, age) = (P::depuis(anciennete), age.map(P::depuis));
    let t = table()?;
    let sec = get(conv(&t, idcc), "preavis");
    if !vrai(sec) {
        return Ok(json!({"mois": null, "article": null, "pourquoi": non_relevees(idcc)}));
    }
    if vrai(get(sec, "loi_seule")) {
        return Ok(json!({"mois": null, "article": null, "pourquoi": get(sec, "loi_seule")}));
    }
    let article = format!("{}, art. {}", nom(idcc), texte(cle(sec, "article")?));
    let hors = get(get(sec, "motifs_hors_preavis"), motif.unwrap_or(""));
    if vrai(hors) {
        return Ok(json!({"mois": "0", "article": article, "pourquoi": hors}));
    }
    let toutes = get(sec, "toutes_categories");
    let categorie = categorie.filter(|c| !c.is_empty());
    if categorie.is_none() && !vrai(toutes) {
        return Ok(json!({"mois": null, "article": article,
            "pourquoi": "la categorie du salarie n'est pas connue"}));
    }
    let par_categorie = categorie.map_or(&NUL, |c| get(get(sec, "par_categorie"), c));
    let mut tranches = ou(toutes, par_categorie);
    let demission = motif == Some("059");
    if tranches.is_object() {
        tranches = get(tranches, if demission { "demission" } else { "licenciement" });
    }
    if !vrai(tranches) {
        return Ok(json!({"mois": null, "article": article,
            "pourquoi": format!("{} ne fixe pas de preavis pour la categorie {}",
                nom(idcc), categorie.unwrap_or("None"))}));
    }
    let a = anciennete;
    for t in tab(tranches)? {
        let haut = get(t, "anciennete_mois_max");
        if a >= nombre(t, "anciennete_mois_min")? && (haut.is_null() || a < nb(haut)?) {
            if !get(t, "jours_par_semaine").is_null() {
                // « 1 jour par semaine calendaire, dans la limite de 15 jours » :
                // des jours, rendus en mois de 30 jours.
                let semaines = plancher(div(mul(a, P::entier(52)), P::entier(12)));
                let plafond = nombre(t, "plafond_jours")?;
                let produit = mul(semaines, nb(get(t, "jours_par_semaine"))?);
                let jours = if produit < plafond { produit } else { plafond };
                let mois = quantifier(div(jours, P::entier(30)), 4);
                return Ok(json!({"mois": dv(mois), "article": article, "jours": dv(jours),
                    "pourquoi": null}));
            }
            let mut mois = nombre(t, "mois")?;
            let supplements = get(sec, "supplement_age");
            if vrai(supplements) && (!demission || vrai(get(sec, "supplement_age_demission"))) {
                let Some(age) = age else {
                    return Ok(json!({"mois": null, "article": article,
                        "pourquoi": format!("le preavis de {} depend de l'age du salarie, et sa \
                            date de naissance manque", article)}));
                };
                for sup in tab(supplements)? {
                    if nombre(sup, "age_min")? <= age && age < nombre(sup, "age_max")? {
                        mois = add(mois, nombre(sup, "mois")?);
                    }
                }
            }
            return Ok(json!({"mois": dv(mois), "article": article, "pourquoi": null}));
        }
    }
    Ok(json!({"mois": null, "article": article,
        "pourquoi": format!("aucune tranche de {} ne couvre {} mois", article, a)}))
}

// ------------------------------------------------------------- les baremes
/// En mois de salaire : chaque tranche d'annees a sa fraction (R1234-2).
pub fn par_tranches(taux: &[Value], annees: P, entieres: bool) -> Resultat<P> {
    let annees = if entieres { plancher(annees) } else { annees };
    let (mut total, mut bas) = (P::ZERO, P::ZERO);
    for t in taux {
        let haut = match get(t, "jusqu_a_annees") {
            Value::Null => annees,
            h => nb(h)?,
        };
        let part = positif(sub(annees.min(haut), bas));
        total = add(total, mul(part, nombre(t, "fraction_de_mois")?));
        bas = haut;
        if annees <= haut {
            break;
        }
    }
    Ok(total)
}

/// En mois de salaire : la bande atteinte donne sa fraction a TOUTES les annees.
pub fn par_bandes(bandes: &[Value], annees: P) -> Resultat<P> {
    let mut triees: Vec<&Value> = bandes.iter().collect();
    let mut erreur = None;
    triees.sort_by(|a, b| {
        comparer(a, b, "anciennete_annees_min").unwrap_or_else(|e| {
            erreur = Some(e);
            Ordering::Equal
        })
    });
    if let Some(e) = erreur {
        return Err(e);
    }
    let mut fraction = P::ZERO;
    for b in triees {
        if annees >= nombre(b, "anciennete_annees_min")? {
            fraction = nombre(b, "fraction_de_mois")?;
        }
    }
    Ok(mul(fraction, annees))
}

pub fn lecture(nom_lecture: &str, taux: &[Value], annees: P) -> Resultat<P> {
    if nom_lecture.starts_with("par_bandes") {
        return par_bandes(taux, annees);
    }
    let entieres = taux.iter().any(|t| vrai(get(t, "annees_entieres")));
    par_tranches(taux, annees, entieres)
}

// ----------------------------------------------- l'indemnite de licenciement
/// `(article, section)` de la clause d'annexe qui s'applique a ce salarie, ou
/// `None`. Le personnel technique de l'annexe I se lit sur la GRILLE du metier.
pub fn clause_d_annexe(
    idcc: &Value,
    annexe: &Value,
    prof: &Value,
    motif: Option<&str>,
) -> Resultat<Option<(String, Value)>> {
    let t = table()?;
    let sec = get(conv(&t, idcc), "indemnite_licenciement");
    let par = get(get(sec, "par_annexe"), &lu(annexe));
    let cat = get(prof, "categorie").as_str();
    let choix = if par.get("cadre_de_direction").is_some() && cat == Some("cadre_de_direction") {
        get(par, "cadre_de_direction")
    } else if par.get("technicien").is_some()
        && get(prof, "grille").as_str() == Some("technique")
        && cat != Some("artiste")
        && cat != Some("cadre_de_direction")
    {
        get(par, "technicien")
    } else if par.get("artiste").is_some() && cat == Some("artiste") {
        get(par, "artiste")
    } else {
        &NUL
    };
    let seulement = get(choix, "motifs_legaux_seulement");
    let exclu = seulement
        .as_array()
        .map_or(false, |a| a.iter().any(|x| x.as_str() == Some(motif.unwrap_or(""))));
    if !vrai(choix) || exclu {
        return Ok(None);
    }
    Ok(Some((texte(cle(choix, "article")?), choix.clone())))
}

/// L'indemnite CONVENTIONNELLE de licenciement :
/// `{releve, du, bas, haut, lectures, article, pourquoi, hors_primes}`.
/// `bas` et `haut` sont egaux quand le texte n'a qu'une lecture ; quand ils
/// different, le montant n'est pas tranche et l'appelant demande.
pub fn indemnite_licenciement(
    idcc: &Value,
    annexe: &Value,
    prof: &Value,
    anciennete: Decimal,
    reference: Decimal,
    motif: Option<&str>,
    age: Option<Decimal>,
) -> Resultat<Value> {
    let (anciennete, reference, age) = (P::depuis(anciennete), P::depuis(reference), age.map(P::depuis));
    let t = table()?;
    let sec = get(conv(&t, idcc), "indemnite_licenciement");
    if !vrai(sec) {
        return Ok(json!({"releve": false, "du": false, "pourquoi": non_relevees(idcc)}));
    }
    if vrai(get(sec, "loi_seule")) {
        return Ok(json!({"releve": false, "du": false, "pourquoi": get(sec, "loi_seule")}));
    }
    let a = anciennete;
    let annees = div(a, P::entier(12));
    let ref_ = reference;
    let motif_t = motif.unwrap_or("");
    let dans = |v: &Value| v.as_array().map_or(false, |l| l.iter().any(|x| x.as_str() == Some(motif_t)));
    let mut lectures: Vec<(String, P)> = Vec::new();
    let mut article = format!("{}, art. {}", nom(idcc), texte(cle(sec, "article")?));
    let mut hors_primes = false;
    let mut pourquoi = Value::Null;
    let sortie = |lectures: &Vec<(String, P)>, article: &str, hors_primes: bool, pourquoi: &Value, du: bool| {
        let mut l = Map::new();
        for (k, v) in lectures {
            l.insert(k.clone(), dv(*v));
        }
        let (bas, haut) = if du {
            (
                lectures.iter().map(|x| x.1).min().map_or(Value::Null, dv),
                lectures.iter().map(|x| x.1).max().map_or(Value::Null, dv),
            )
        } else {
            (Value::Null, Value::Null)
        };
        json!({"releve": true, "du": du, "bas": bas, "haut": haut, "lectures": l,
            "hors_primes": hors_primes, "article": article, "pourquoi": pourquoi})
    };
    if dans(get(sec, "motifs_sans_indemnite")) {
        let p = format!("le motif {} n'ouvre pas d'indemnite de licenciement", motif_t);
        return Ok(sortie(&lectures, &article, hors_primes, &Value::String(p), false));
    }

    // La convention commune d'abord ; ou, pour une convention qui distingue le
    // motif, le bareme du motif.
    if a >= nombre(sec, "anciennete_minimale_mois")? {
        let mut mois = if vrai(get(sec, "par_motif")) {
            let pm = get(sec, "par_motif");
            let eco = get(pm, "economique");
            let choix = if dans(get(eco, "motifs")) { eco } else { cle(pm, "autre")? };
            if choix.get("bandes").is_some() {
                par_bandes(tab(cle(choix, "bandes")?)?, annees)?
            } else {
                lecture("tranches", tab(cle(choix, "taux")?)?, annees)?
            }
        } else {
            lecture("tranches", tab(cle(sec, "taux")?)?, annees)?
        };
        if !get(sec, "plafond_mois").is_null() {
            mois = mois.min(nombre(sec, "plafond_mois")?);
        }
        let mut commun = eur2(mul(mois, ref_));
        let avant = get(sec, "taux_avant");
        if vrai(avant) && a < nombre(avant, "mois")? {
            // Avant le bareme, une fraction a part.
            commun = eur2(mul(mul(annees, nombre(avant, "fraction_de_mois")?), ref_));
        }
        let maj = get(sec, "majoration_age");
        if let Value::Array(maj) = maj {
            // Plusieurs ages, chacun son pourcentage et son plancher, puis le
            // plafond. L'age en annees revolues ; sans lui, les deux lectures.
            let mut vises: Vec<&Value> = Vec::new();
            for m in maj {
                if a >= nombre(m, "anciennete_mois_min")? {
                    vises.push(m);
                }
            }
            match age {
                None if !vises.is_empty() => {
                    set(&mut lectures, "sans_majoration_age", commun);
                    pourquoi = Value::String(format!(
                        "l'indemnite de {} depend de l'age du salarie, et sa date de naissance manque",
                        article
                    ));
                    vises = vises.split_off(vises.len() - 1);
                }
                Some(age) => {
                    let mut gardes = Vec::new();
                    for m in vises {
                        let haut = get(m, "age_max");
                        if nombre(m, "age_min")? <= age && (haut.is_null() || age < nb(haut)?) {
                            gardes.push(m);
                        }
                    }
                    vises = gardes;
                }
                None => {}
            }
            let mut meilleur = commun;
            for m in vises {
                let p = get(m, "pourcentage");
                let p = if vrai(p) { nb(p)? } else { P::ZERO };
                let f = add(P::entier(1), div(p, P::entier(100)));
                let mut v = eur2(mul(mul(mois, ref_), f));
                if !get(m, "plancher_mois").is_null() {
                    v = v.max(eur2(mul(nombre(m, "plancher_mois")?, ref_)));
                }
                meilleur = meilleur.max(v);
            }
            if !get(sec, "plafond_majore_mois").is_null() {
                meilleur = meilleur.min(eur2(mul(nombre(sec, "plafond_majore_mois")?, ref_)));
            }
            commun = meilleur;
        } else if vrai(maj) && a >= nombre(maj, "anciennete_mois_min")? {
            // Une majoration a partir d'un age. Sans l'age, les deux lectures.
            let p = nombre(maj, "pourcentage")?;
            let majore = eur2(mul(commun, add(P::entier(1), div(p, P::entier(100)))));
            match age {
                None => {
                    set(&mut lectures, "sans_majoration_age", commun);
                    commun = majore;
                    pourquoi = Value::String(format!(
                        "la majoration de {} % depend de l'age du salarie, et sa date de \
                         naissance manque",
                        texte(cle(maj, "pourcentage")?)
                    ));
                }
                Some(age) if age >= nombre(maj, "age_min")? => commun = majore,
                Some(_) => {}
            }
        }
        set(&mut lectures, "convention", commun);
    }
    // Puis la clause de l'annexe, si elle vise ce salarie : elle REMPLACE la
    // clause commune pour lui, et la loi reste le plancher (l'appelant).
    if let Some((art, c)) = clause_d_annexe(idcc, annexe, prof, motif)? {
        article = format!("{}, {}", nom(idcc), art);
        hors_primes = vrai(get(&c, "hors_primes"));
        let minimale = ou(get(&c, "anciennete_minimale_mois"), &NUL);
        let minimale = if vrai(minimale) { nb(minimale)? } else { P::ZERO };
        if a < minimale {
            let p = format!(
                "{} demande {} mois d'anciennete",
                article,
                texte(cle(&c, "anciennete_minimale_mois")?)
            );
            return Ok(sortie(&Vec::new(), &article, hors_primes, &Value::String(p), false));
        }
        lectures = Vec::new();
        let lec = cle(&c, "lectures")?.as_object().ok_or("lectures attendues")?;
        for (n, taux) in lec {
            let mut mois = lecture(n, tab(taux)?, annees)?;
            if !get(&c, "plafond_mois").is_null() {
                mois = mois.min(nombre(&c, "plafond_mois")?);
            }
            set(&mut lectures, n, eur2(mul(mois, ref_)));
        }
    }
    if lectures.is_empty() {
        let p = format!(
            "{} mois d'anciennete, et {} en demande {}",
            quantifier(a, 1),
            article,
            texte(cle(sec, "anciennete_minimale_mois")?)
        );
        return Ok(sortie(&lectures, &article, hors_primes, &Value::String(p), false));
    }
    Ok(sortie(&lectures, &article, hors_primes, &pourquoi, true))
}

/// `d[k] = v` d'un dict ordonne : une cle existante garde sa place.
fn set(l: &mut Vec<(String, P)>, k: &str, v: P) {
    match l.iter_mut().find(|x| x.0 == k) {
        Some(x) => x.1 = v,
        None => l.push((k.to_string(), v)),
    }
}

// ------------------------------------------------------------ la retraite
/// `(mois de salaire, palier atteint)` d'un bareme par paliers, avec la part
/// au prorata qui le precede quand la convention en a une.
pub fn paliers(sec: &Value, annees: P) -> Resultat<(P, Option<i64>)> {
    let mut triees: Vec<&Value> = tab(cle(sec, "paliers")?)?.iter().collect();
    let mut erreur = None;
    triees.sort_by(|a, b| {
        comparer(a, b, "anciennete_annees").unwrap_or_else(|e| {
            erreur = Some(e);
            Ordering::Equal
        })
    });
    if let Some(e) = erreur {
        return Err(e);
    }
    let mut palier = None;
    for p in triees {
        if annees >= nombre(p, "anciennete_annees")? {
            palier = Some(p);
        }
    }
    if let Some(p) = palier {
        return Ok((nombre(p, "mois_de_salaire")?, Some(entier(cle(p, "anciennete_annees")?)?)));
    }
    let pr = get(sec, "prorata");
    if vrai(pr) && annees >= nombre(pr, "anciennete_annees_min")? {
        return Ok((mul(annees, nombre(pr, "fraction_de_mois")?), None));
    }
    Ok((P::ZERO, None))
}

/// L'indemnite conventionnelle de mise a la retraite :
/// `{releve, montant, article, hors_primes, prevenance_mois, cadre_de_direction}`.
pub fn indemnite_mise_a_la_retraite(idcc: &Value, anciennete: Decimal, reference: Decimal) -> Resultat<Value> {
    let (anciennete, reference) = (P::depuis(anciennete), P::depuis(reference));
    let t = table()?;
    let sec = get(conv(&t, idcc), "mise_a_la_retraite");
    if !vrai(sec) {
        return Ok(json!({"releve": false, "montant": null, "pourquoi": non_relevees(idcc)}));
    }
    if vrai(get(sec, "loi_seule")) {
        return Ok(json!({"releve": true, "montant": null, "article": null,
            "pourquoi": get(sec, "loi_seule")}));
    }
    let annees = div(anciennete, P::entier(12));
    let article = |s: &Value| -> Resultat<String> { Ok(format!("{}, art. {}", nom(idcc), texte(cle(s, "article")?))) };
    if vrai(get(sec, "paliers")) {
        let (mois, _) = paliers(sec, annees)?;
        return Ok(json!({"releve": true, "montant": dv(eur2(mul(mois, reference))),
            "article": article(sec)?, "hors_primes": false,
            "prevenance_mois": get(sec, "prevenance_mois"), "cadre_de_direction": null}));
    }
    if vrai(get(sec, "taux")) {
        // Le bareme du licenciement, par tranches, avec son plafond.
        let mut mois = lecture("tranches", tab(get(sec, "taux"))?, annees)?;
        if !get(sec, "plafond_mois").is_null() {
            mois = mois.min(nombre(sec, "plafond_mois")?);
        }
        return Ok(json!({"releve": true, "montant": dv(eur2(mul(mois, reference))),
            "article": article(sec)?, "hors_primes": vrai(get(sec, "hors_primes")),
            "prevenance_mois": get(sec, "prevenance_mois"), "cadre_de_direction": null}));
    }
    let mut mois = mul(annees, nombre(sec, "par_annee")?);
    let au_dela = sub(annees, nombre(sec, "au_dela_de_annees")?);
    if au_dela > P::ZERO {
        mois = add(mois, mul(au_dela, nombre(sec, "en_plus_par_annee_au_dela")?));
    }
    Ok(json!({"releve": true, "montant": dv(eur2(mul(mois, reference))),
        "article": article(sec)?, "hors_primes": vrai(get(sec, "hors_primes")),
        "prevenance_mois": get(sec, "prevenance_mois"),
        "cadre_de_direction": get(sec, "cadre_de_direction")}))
}

/// L'indemnite conventionnelle de depart volontaire a la retraite : un
/// palier, pas un cumul. `{releve, montant, mois, palier, article, prevenance_mois}`.
pub fn indemnite_depart_retraite(idcc: &Value, anciennete: Decimal, reference: Decimal) -> Resultat<Value> {
    let (anciennete, reference) = (P::depuis(anciennete), P::depuis(reference));
    let t = table()?;
    let sec = get(conv(&t, idcc), "depart_a_la_retraite");
    if !vrai(sec) {
        return Ok(json!({"releve": false, "montant": null, "pourquoi": non_relevees(idcc)}));
    }
    if vrai(get(sec, "loi_seule")) {
        return Ok(json!({"releve": true, "montant": null, "article": null,
            "pourquoi": get(sec, "loi_seule")}));
    }
    let annees = div(anciennete, P::entier(12));
    let article = format!("{}, art. {}", nom(idcc), texte(cle(sec, "article")?));
    if vrai(get(sec, "taux")) {
        // Le bareme d'un licenciement, par tranches.
        let minimale = ou(get(sec, "anciennete_minimale_mois"), &NUL);
        let minimale = if vrai(minimale) { nb(minimale)? } else { P::ZERO };
        let mois = if mul(annees, P::entier(12)) >= minimale {
            lecture("tranches", tab(get(sec, "taux"))?, annees)?
        } else {
            P::ZERO
        };
        return Ok(json!({"releve": true, "montant": dv(eur2(mul(mois, reference))),
            "mois": dv(mois), "palier": null, "article": article,
            "prevenance_mois": get(sec, "prevenance_mois")}));
    }
    let (mois, palier) = paliers(sec, annees)?;
    Ok(json!({"releve": true, "montant": dv(eur2(mul(mois, reference))),
        "mois": dv(mois), "palier": palier, "article": article,
        "prevenance_mois": get(sec, "prevenance_mois")}))
}

// ------------------------------------------------- le maintien de salaire
/// `4` pour 4,0 ; `66,66` pour 66.66.
pub fn pct(x: &Value) -> Resultat<String> {
    let x = nb(x)?;
    pct_decimal(x)
}

fn pct_decimal(x: P) -> Resultat<String> {
    Ok(if x == plancher(x) { plancher(x).to_string() } else { x.to_string().replace('.', ",") })
}

/// Ce que la convention garantit pendant un arret, en une phrase -- ou `None`
/// si elle n'est pas relevee. Dit, pas paye : le complement se calcule apres
/// les indemnites journalieres, qui viennent de la caisse.
pub fn maintien(idcc: &Value, anciennete_mois: Decimal, accident_du_travail: bool) -> Resultat<Option<String>> {
    let anciennete_mois = P::depuis(anciennete_mois);
    let t = table()?;
    let sec = get(conv(&t, idcc), "maintien_de_salaire");
    let a = anciennete_mois;
    let mut at = if accident_du_travail { get(sec, "accident_du_travail") } else { &NUL };
    if vrai(at)
        && !get(at, "comme_maladie_apres_mois").is_null()
        && a >= nombre(at, "comme_maladie_apres_mois")?
    {
        // Passe un an, l'accident suit le bareme de la maladie.
        at = &NUL;
    }
    let brut_ou_net = |s: &Value| if get(s, "base").as_str() == Some("brut") { "BRUT" } else { "NET" };
    if vrai(at) {
        let s = at;
        let art = texte(cle(s, "article")?);
        let minimum = match s.get("anciennete_mois_minimum") {
            Some(m) => nb(m)?,
            None => P::entier(12),
        };
        if a < minimum && minimum == P::entier(12) {
            return Ok(Some(format!(
                "{} (art. {}) : rien avant un an d'anciennete -- l'article est etendu sous \
                 reserve de L1226-1.",
                nom(idcc),
                art
            )));
        }
        if a < minimum {
            return Ok(Some(format!(
                "{} (art. {}) : rien avant {} mois d'anciennete.",
                nom(idcc),
                art,
                pct_decimal(minimum)?
            )));
        }
        let mut tr = cle(s, "tranches")?;
        if vrai(get(s, "tranches_deux_ans")) && a >= P::entier(24) {
            tr = get(s, "tranches_deux_ans");
        }
        let apres = get(s, "tranches_apres");
        if vrai(apres) && a > nombre(apres, "anciennete_mois")? {
            tr = cle(apres, "tranches")?;
        }
        let tr = tab(tr)?;
        if tr.len() == 1 {
            return Ok(Some(format!(
                "{} (art. {}) garantit {} % du salaire {} pendant {} jours, sans carence, sous \
                 deduction des indemnites journalieres et de la prevoyance.",
                nom(idcc),
                art,
                pct(cle(&tr[0], "taux")?)?,
                brut_ou_net(s),
                texte(cle(&tr[0], "jours")?)
            )));
        }
        let deux = || crate::fichiers::trop_courte("les tranches (deux attendues)");
        let (t0, t1) = (tr.first().ok_or_else(deux)?, tr.get(1).ok_or_else(deux)?);
        return Ok(Some(format!(
            "{} (art. {}) garantit {} % du salaire {} pendant {} jours puis {} % pendant {} \
             jours, sans carence, sous deduction des indemnites journalieres et de la \
             prevoyance.",
            nom(idcc),
            art,
            pct(cle(t0, "taux")?)?,
            brut_ou_net(s),
            texte(cle(t0, "jours")?),
            pct(cle(t1, "taux")?)?,
            texte(cle(t1, "jours")?)
        )));
    }
    let s = get(sec, "maladie");
    if !vrai(s) {
        return Ok(None);
    }
    let art = texte(cle(s, "article")?);
    if a < nombre(s, "anciennete_mois_minimum")? {
        return Ok(Some(format!(
            "{} (art. {}) : rien avant {} mois d'anciennete.",
            nom(idcc),
            art,
            texte(cle(s, "anciennete_mois_minimum")?)
        )));
    }
    let annees = div(a, P::entier(12));
    let mut lignes: Vec<&Value> = tab(cle(s, "par_anciennete")?)?.iter().collect();
    let mut erreur = None;
    lignes.sort_by(|x, y| {
        comparer(x, y, "anciennete_annees_min").unwrap_or_else(|e| {
            erreur = Some(e);
            Ordering::Equal
        })
    });
    if let Some(e) = erreur {
        return Err(e);
    }
    let mut ligne: &Value = &NUL;
    for p in lignes {
        if annees >= nombre(p, "anciennete_annees_min")? {
            ligne = p;
        }
    }
    let carence = cle(s, "carence_jours")?;
    if vrai(ligne) && vrai(get(ligne, "tranches")) {
        // Des tranches dites en toutes lettres, et non les 90 / 66,66 % du decret.
        let mut suite = Vec::new();
        for t in tab(get(ligne, "tranches"))? {
            suite.push(format!("{} % pendant {} jours", pct(cle(t, "taux")?)?, texte(cle(t, "jours")?)));
        }
        let c = if !vrai(carence) {
            "sans carence".to_string()
        } else {
            format!("apres {} jours de carence", texte(carence))
        };
        // La phrase sur la carence n'est vraie que si elle est plus courte que
        // celle de la loi.
        let c_dec = if vrai(carence) { nb(carence)? } else { P::ZERO };
        let conclusion = if c_dec < P::entier(7) {
            " La carence est plus courte que celle de la loi (sept jours) : c'est elle qu'il \
             faut appliquer."
        } else {
            " La carence est celle de la loi ; ce sont les taux et les durees qui vont \
             au-dela."
        };
        let assiette = ou(get(s, "assiette"), &NUL);
        let assiette = if vrai(assiette) { texte(assiette) } else { "le salaire fixe, hors primes".to_string() };
        return Ok(Some(format!(
            "{} (art. {}) garantit, {}, {} du salaire {} (sur douze mois), sous deduction des \
             indemnites journalieres et de la prevoyance ; assiette : {}.{}",
            nom(idcc),
            art,
            c,
            suite.join(" puis "),
            brut_ou_net(s),
            assiette,
            conclusion
        )));
    }
    let (j90, j66) = (cle(ligne, "jours_90")?, cle(ligne, "jours_66")?);
    if get(s, "base").as_str() == Some("brut") {
        // Du BRUT, comme la loi -- ce qui change, c'est la carence (trois
        // jours, pas sept) et le taux des 30 premiers jours.
        return Ok(Some(format!(
            "{} (art. {}) garantit, apres {} jours de carence, 90 % du salaire BRUT pendant {} \
             jours puis 66,66 % pendant {} jours (sur douze mois), sous deduction des \
             indemnites journalieres et de la prevoyance ; assiette : le salaire fixe, hors \
             primes. La carence est plus courte que celle de la loi (sept jours) : c'est elle \
             qu'il faut appliquer.",
            nom(idcc),
            art,
            texte(carence),
            texte(j90),
            texte(j66)
        )));
    }
    Ok(Some(format!(
        "{} (art. {}) garantit, apres {} jours de carence, 90 % du salaire NET pendant {} jours \
         puis 66,66 % pendant {} jours (sur douze mois), sous deduction des indemnites \
         journalieres et de la prevoyance ; assiette : le salaire fixe, hors primes. C'est plus \
         que la loi (sept jours de carence, un pourcentage du brut) : c'est elle qu'il faut \
         appliquer.",
        nom(idcc),
        art,
        texte(carence),
        texte(j90),
        texte(j66)
    )))
}

// ------------------------------------------------- la prime d'anciennete
/// `date.fromisoformat` sur les dix premiers caracteres : `AAAA-MM-JJ`.
fn iso(s: &str) -> Resultat<Jour> {
    let c: Vec<char> = s.chars().take(10).collect();
    let nombre = |a: usize, b: usize| -> Option<u32> {
        let t: String = c.get(a..b)?.iter().collect();
        if t.chars().all(|x| x.is_ascii_digit()) {
            t.parse().ok()
        } else {
            None
        }
    };
    let ok = c.len() == 10 && c[4] == '-' && c[7] == '-';
    ok.then(|| ())
        .and_then(|_| Jour::nouveau(nombre(0, 4)? as i32, nombre(5, 7)?, nombre(8, 10)?))
        .ok_or_else(|| format!("date illisible : {}", repr(s)))
}

/// La prime d'anciennete d'un technicien de l'annexe I de la 3090 pour le
/// mois qui finit `jour`, ou `None` si elle ne le concerne pas.
///
/// Rend `{pourcentage, base, montant, article, pourquoi, annees}` ; `montant`
/// vaut `null` et `pourquoi` dit ce qui manque (la jauge de la salle, une
/// grille non relevee pour cette date, le niveau du metier).
pub fn prime_anciennete(
    idcc: &Value,
    annexe: &Value,
    fiche: &Value,
    engagement: Option<&Value>,
    jour: &Jour,
    jauge: Option<&str>,
) -> Resultat<Option<Value>> {
    let t = table()?;
    let sec = get(conv(&t, idcc), "prime_anciennete");
    if !vrai(get(sec, "annexe")) || lu(annexe) != texte(cle(sec, "annexe")?) {
        return Ok(None);
    }
    let e = engagement.unwrap_or(&NUL);
    let code = ou(get(e, "metier"), get(fiche, "code_emploi_ft"));
    let m = if vrai(code) {
        let c = code.as_str().ok_or_else(|| format!("code emploi : {}", crate::fichiers::attendu("un texte", crate::etats::nom_type(code))))?;
        emplois::metier(c)?
    } else {
        None
    };
    let mv = m.as_ref().unwrap_or(&NUL);
    let c3090 = get(mv, "classification_3090");
    let statut = ou(get(e, "statut"), get(fiche, "statut"));
    if statut.as_str() == Some("artiste") || get(c3090, "grille").as_str() != Some("technique") {
        return Ok(None);
    }
    let article = format!("{}, {}", nom(idcc), texte(cle(sec, "article")?));
    let depuis = ou(get(fiche, "date_anciennete"), get(e, "debut"));
    if !vrai(depuis) {
        return Ok(None);
    }
    let debut = iso(&texte(depuis))?;
    let annees = jour.an as i64
        - debut.an as i64
        - ((jour.mois, jour.jour) < (debut.mois, debut.jour)) as i64;
    let pourcent = entier(cle(sec, "plafond_pourcentage")?)?.min(
        (annees - (entier(cle(sec, "apres_annees")?)? - 1)).max(0) * entier(cle(sec, "pourcentage_par_annee")?)?,
    );
    if pourcent <= 0 {
        return Ok(None);
    }
    let mut out = json!({"pourcentage": pourcent.to_string(), "base": null, "montant": null,
        "article": article, "pourquoi": null, "annees": annees});
    let ce_jour = jour.iso();
    let mut grilles = Vec::new();
    for g in tab(cle(sec, "salaires_conventionnels")?)? {
        if texte(cle(g, "depuis")?) <= ce_jour {
            grilles.push(g);
        }
    }
    let Some(g) = grilles.last() else {
        let premiere = texte(cle(&tab(cle(sec, "salaires_conventionnels")?)?[0], "depuis")?);
        out["pourquoi"] = Value::String(format!(
            "les grilles de salaire conventionnel de l'annexe I ne sont relevees qu'a partir du {}",
            premiere
        ));
        return Ok(Some(out));
    };
    let niveau = get(get(sec, "niveaux"), &lu_brut(get(c3090, "niveau")));
    if !vrai(niveau) {
        out["pourquoi"] = Value::String(format!("le niveau 3090 du metier {} n'est pas connu", texte(code)));
        return Ok(Some(out));
    }
    let niveau = niveau.as_str().ok_or("niveau illisible")?;
    let famille = get(mv, "famille");
    let costume = get(sec, "familles_costume")
        .as_array()
        .map_or(false, |l| l.iter().any(|x| x == famille));
    let base = if costume {
        cle(cle(g, "costume")?, niveau)?
    } else {
        let technique = cle(g, "technique")?;
        let Some(j) = jauge.filter(|j| technique.get(*j).is_some()) else {
            let cles: Vec<String> = technique
                .as_object()
                .map_or(vec![], |o| o.keys().map(|k| format!("\"{}\"", k)).collect());
            out["pourquoi"] = Value::String(format!(
                "le salaire conventionnel d'un technicien de l'annexe I depend de la JAUGE de la \
                 salle, et la fiche de la compagnie ne la dit pas (`jauge_annexe_1` : {})",
                cles.join(", ")
            ));
            return Ok(Some(out));
        };
        cle(cle(technique, j)?, niveau)?
    };
    out["base"] = dv(nb(base)?);
    Ok(Some(out))
}
