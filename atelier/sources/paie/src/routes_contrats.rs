//! Les routes de la saisie qui LISENT les contrats et les referentiels
//! (`moteur/web.py`, `operations_paies`, `operations_catalogues`, `emplois`) :
//! `contrats-en-cours`, `contrats-passes`, `emploi`, `rubriques-actives`,
//! `metiers-catalogue`.
//!
//! AUCUN CALCUL DE PAIE, et pas un seul fichier `calcul.json` : ni les contrats
//! en cours ni les contrats passes n'ont besoin d'un brut. Les bornes et les
//! quantites sont dans les fichiers de saisie (`paies/AAAA-MM.json`) ; les jours
//! exacts des mois passes aussi ; l'historique importe parle pour les autres
//! (`historique/engagements.json`). Le Python lisait exactement cela.
//!
//! `rubriques-actives` n'est pas une lecture : elle ecrit la preference
//! `config/rubriques-actives.json`, avec la copie horodatee que le Python en
//! gardait (`operations_socle._sauver_avant`, l'heure de Paris par `paris.rs`).
//!
//! Les refus gardent la classe d'exception du Python (`KeyError`, `TypeError`,
//! `AttributeError`, `ValueError`, `ErreurOperation`...), que `paie_rust.route`
//! releve sous le meme nom.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::catalogue;
use crate::contexte::Contexte;
use crate::ecriture::ecrire_json;
use crate::emplois;
use crate::etats::{self, erreur, get, items, nom_type, nombre, ou, py_str, Dico, R};
use crate::fichiers::{repr, repr_valeur, vrai};
use crate::jumeaux;
use crate::retours::Echec;
use crate::routes_accueil::{metier_visible, preferences_metiers, rubrique_json};
use crate::routes_saisie::lire_saisie;

/// Les routes que ce module sert.
pub const ROUTES: [&str; 5] = ["contrats-en-cours", "contrats-passes", "emploi", "rubriques-actives", "metiers-catalogue"];

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "contrats-en-cours" => contrats_en_cours_route(ctx, charge),
        "contrats-passes" => contrats_passes_route(ctx, charge),
        "emploi" => emploi(charge),
        "rubriques-actives" => rubriques_actives(ctx, charge),
        "metiers-catalogue" => metiers_catalogue(ctx),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

// ------------------------------------------------------------------ outils

#[track_caller]
fn plain(m: impl Into<String>) -> Echec {
    crate::etats::noter_et(None, m)
}

#[track_caller]
fn cle_manquante(cle: &str) -> Echec {
    erreur("KeyError", crate::fichiers::absent(cle))
}

/// `charge["cle"]`.
fn exiger<'a>(charge: &'a Value, cle: &str) -> R<&'a Value> {
    charge.get(cle).ok_or_else(|| cle_manquante(cle))
}

/// `a < b` de Python, pour ce que des fichiers de saisie portent : des textes,
/// ou des nombres entre eux ; le reste ne se compare pas.
fn comparer(a: &Value, b: &Value) -> R<Ordering> {
    match (a, b) {
        (Value::String(x), Value::String(y)) => Ok(x.cmp(y)),
        (Value::String(_), _) | (_, Value::String(_)) => Err(pas_comparable(a, b)),
        _ => match (nombre(a), nombre(b)) {
            (Some(x), Some(y)) => Ok(x.partial_cmp(&y).unwrap_or(Ordering::Equal)),
            _ => Err(pas_comparable(a, b)),
        },
    }
}

#[track_caller]
fn pas_comparable(a: &Value, b: &Value) -> Echec {
    erreur("TypeError", crate::fichiers::incomparables(nom_type(a), nom_type(b)))
}

/// `sorted(valeurs)`.
fn trier(mut v: Vec<Value>) -> R<Vec<Value>> {
    let mut refus = None;
    v.sort_by(|a, b| match comparer(a, b) {
        Ok(o) => o,
        Err(e) => {
            refus.get_or_insert(e);
            Ordering::Equal
        }
    });
    match refus {
        Some(e) => Err(e),
        None => Ok(v),
    }
}

/// `max(valeurs)` d'une liste non vide : le premier des plus grands.
fn maximum(v: &[Value]) -> R<Value> {
    let mut m = v[0].clone();
    for x in &v[1..] {
        if comparer(&m, x)? == Ordering::Less {
            m = x.clone();
        }
    }
    Ok(m)
}

/// `valeur or defaut`, puis `for x in ...`.
fn parcourir(v: Option<&Value>) -> R<Vec<Value>> {
    match v {
        Some(x) if vrai(x) => Ok(items(x)?.into_iter().map(|c| c.into_owned()).collect()),
        _ => Ok(Vec::new()),
    }
}

/// Les cles d'un dictionnaire de fiche (`dict.get` sur une valeur qui n'en est pas un : `AttributeError`).
fn objet<'a>(v: &'a Value) -> R<&'a Map<String, Value>> {
    v.as_object().ok_or_else(|| erreur("AttributeError", crate::fichiers::attendu("un objet", nom_type(v))))
}

/// `int(v)` de Python.
fn entier_py(v: &Value) -> R<i64> {
    match v {
        Value::Bool(b) => Ok(*b as i64),
        Value::Number(n) => match n.as_i64() {
            Some(i) => Ok(i),
            None => {
                let x = n.as_f64().unwrap_or(0.0);
                if x.is_nan() {
                    Err(erreur("ValueError", "cannot convert float NaN to integer"))
                } else if x.is_infinite() {
                    Err(erreur("OverflowError", crate::fichiers::entier_hors_limites("infini")))
                } else {
                    Ok(x.trunc() as i64)
                }
            }
        },
        Value::String(s) => s
            .trim()
            .replace('_', "")
            .parse::<i64>()
            .map_err(|_| erreur("ValueError", format!("nombre entier illisible : {}", repr(s)))),
        autre => Err(erreur(
            "TypeError",
            crate::fichiers::attendu("un nombre entier", nom_type(autre)),
        )),
    }
}

// ------------------------------------------------------- les jours d'un engagement

/// `operations_paies.dates_engagement` (`cle` = "dates") et
/// `planning_engagement` (`cle` = "planning") : les jours des lignes, sans
/// doublon, nus ou detailles `{jour, lieu}`. Les refus sont ceux de Python :
/// une ligne qui n'est pas un dictionnaire (`AttributeError`), un jour qui ne se
/// hache pas (`TypeError`), un jour detaille sans `jour` (`KeyError`).
fn jours_des_lignes(e: &Map<String, Value>, cle: &str) -> R<Vec<Value>> {
    let mut out = Vec::new();
    let mut vus: Dico<()> = Dico::nouveau();
    for l in parcourir(e.get("lignes"))? {
        let lm = objet(&l)?;
        for j in parcourir(lm.get(cle))? {
            let jour = match &j {
                Value::Object(m) => m.get("jour").cloned().ok_or_else(|| cle_manquante("jour"))?,
                autre => autre.clone(),
            };
            if vus.contient(&[jour.clone()])? {
                continue;
            }
            vus.poser(vec![jour.clone()], ())?;
            let lieu_ligne = lm.get("lieu").filter(|x| vrai(x)).cloned();
            let lieu = match &j {
                Value::Object(m) => m.get("lieu").filter(|x| vrai(x)).cloned().or(lieu_ligne),
                _ => lieu_ligne,
            };
            out.push(match lieu {
                Some(ou) => json!({"jour": jour, "lieu": ou}),
                None => jour,
            });
        }
    }
    Ok(out)
}

/// `j["jour"] if isinstance(j, dict) else j`.
fn jour_de(j: &Value) -> Value {
    match j {
        Value::Object(m) => m.get("jour").cloned().unwrap_or(Value::Null),
        autre => autre.clone(),
    }
}

/// `operations_paies._bornes_brutes` : (debut, fin) d'un engagement brut,
/// AAAA-MM-JJ, comme `Engagement` les deduirait ; vides sans aucun jour.
pub(crate) fn bornes_brutes(e: &Map<String, Value>) -> R<(String, String)> {
    let jours = trier(jours_des_lignes(e, "dates")?.iter().map(jour_de).collect())?;
    let plan = trier(jours_des_lignes(e, "planning")?.iter().map(jour_de).collect())?;
    let ecrit = |k: &str| e.get(k).filter(|x| vrai(x)).cloned();
    let debut = ecrit("debut").or_else(|| jours.first().cloned()).map(|v| py_str(&v)).unwrap_or_default();
    let fin = if !jours.is_empty() || !plan.is_empty() {
        let mut tous = jours.clone();
        tous.extend(plan);
        match ecrit("fin") {
            Some(f) => py_str(&f),
            None => py_str(&maximum(&tous)?),
        }
    } else {
        ecrit("fin").map(|f| py_str(&f)).unwrap_or_default()
    };
    Ok((debut, fin))
}

// ------------------------------------------------------------ les fichiers de mois

/// Les mois dont un fichier de saisie existe (`operations_paies.mois_connus`,
/// ceux qui ont une `saisie`) : le radical de chaque `paies/*.json`, comme
/// `Path.stem` -- `2026-09.calcul.json` y est le mois « 2026-09.calcul ». Dans
/// l'ordre croissant.
pub(crate) fn mois_saisis(ctx: &Contexte) -> Vec<String> {
    let mut mois: Vec<String> = match std::fs::read_dir(ctx.racine.join("paies")) {
        Ok(l) => l
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".json"))
            .map(|n| if n.len() > 5 { n[..n.len() - 5].to_string() } else { n })
            .collect(),
        Err(_) => Vec::new(),
    };
    mois.sort();
    mois.dedup();
    mois
}

/// `operations_paies._engagements_du_mois` : les engagements bruts d'un fichier
/// de mois, avec la cle du salarie ; vide si le mois n'est pas sur le disque.
/// Un engagement dont le salarie est inconnu (ou qui n'est pas un dictionnaire)
/// est sauté.
pub(crate) fn engagements_du_mois(ctx: &Contexte, mois: &str) -> R<Vec<(String, Map<String, Value>)>> {
    let p = ctx.racine.join("paies").join(format!("{}.json", mois));
    if !p.exists() {
        return Ok(Vec::new());
    }
    let data = lire_saisie(&p)?;
    let mut out = Vec::new();
    for e in parcourir(get(&data, "engagements")?.into())? {
        let Some(m) = e.as_object() else { continue };
        let Some(cle) = crate::engagement::cle_de_l_engagement(ctx, &e) else { continue };
        out.push((cle, m.clone()));
    }
    Ok(out)
}

// ------------------------------------------------------------- contrats en cours

/// `operations_paies.ContratEnCours` : un contrat ouvert un mois precedent et
/// qui court encore sur celui-ci.
struct EnCours {
    salarie: String,
    nom: String,
    spectacle: Value,
    debut: String,
    fin: String,
    numero: Value,
    depuis: String,
    metier: Value,
    cadre: Value,
    taux_cachet: Value,
    lieu: Value,
    deja_repris: bool,
    lignes: Vec<Value>,
    /// Un contrat paye au mois (CDD de droit commun...) : ce qui le definit, recopie tel quel
    /// (`DEFINIT_LE_MENSUEL`). `None` pour un contrat d'usage, qui se reprend par ses lignes.
    mensuel: Option<Map<String, Value>>,
}

/// Ce qui definit un contrat paye au mois et qu'un mois suivant ne redit pas : sa nature, son
/// motif, sa duree du travail, son salaire et son emploi (ce que `engagement::engagements` et
/// `conges::lignes_du_mois` lisent). Ni ses lignes (le moteur les compose chaque mois), ni ce qui
/// n'appartient qu'a un mois (conges pris, absences, primes, rupture, date de paiement).
const DEFINIT_LE_MENSUEL: [&str; 12] = [
    "nature",
    "motif_recours",
    "quotite_contrat",
    "quotite_reference",
    "modalite_temps_travail",
    "salaire_mensuel",
    "emploi",
    "pcs_ese",
    "code_emploi_ft",
    "statut",
    "statut_conventionnel",
    "statut_categoriel_rc",
];

impl EnCours {
    /// `sortie_json.contrat_en_cours`.
    fn json(&self) -> Map<String, Value> {
        let mut m = Map::new();
        m.insert("salarie".into(), json!(self.salarie));
        m.insert("nom".into(), json!(self.nom));
        m.insert("spectacle".into(), self.spectacle.clone());
        m.insert("debut".into(), json!(self.debut));
        m.insert("fin".into(), json!(self.fin));
        m.insert("numero".into(), self.numero.clone());
        m.insert("depuis".into(), json!(self.depuis));
        m.insert("metier".into(), self.metier.clone());
        m.insert("cadre".into(), self.cadre.clone());
        m.insert("taux_cachet".into(), self.taux_cachet.clone());
        m.insert("lieu".into(), self.lieu.clone());
        m.insert("deja_repris".into(), json!(self.deja_repris));
        m
    }
}

/// `operations_paies.contrats_en_cours` : les contrats des mois precedents dont
/// la fin depasse le 1er de `mois`. Lu dans les fichiers de saisie des mois qui
/// precedent, jamais dans la DSN.
fn contrats_en_cours(ctx: &Contexte, mois: &Value) -> R<Vec<EnCours>> {
    let texte = py_str(mois);
    let premier = format!("{}-01", texte);
    // LE CONTRAT, PAS LE SALARIE DU JOUR (`jumeaux`) : deux contrats du meme salarie qui commencent le meme jour se
    // reprennent chacun, et chacun se dit repris ou non.
    let mut deja: Vec<Option<jumeaux::Ident>> = Vec::new();
    for (c, e) in engagements_du_mois(ctx, &texte)? {
        deja.push(Some((c, bornes_brutes(&e)?.0, jumeaux::numero_du_champ(e.get("numero")))));
    }
    let precedents = mois_saisis(ctx);
    let Some(texte_mois) = mois.as_str() else {
        // `x.mois < mois` : un texte et autre chose ne se comparent pas.
        return match precedents.first() {
            Some(_) => Err(erreur("TypeError", crate::fichiers::incomparables("str", nom_type(mois)))),
            None => Ok(Vec::new()),
        };
    };
    let mut out = Vec::new();
    let mut vus: Dico<()> = Dico::nouveau();
    for m in precedents.iter().filter(|m| m.as_str() < texte_mois) {
        let spectacle_du_mois = spectacle_du_fichier(ctx, m)?;
        let du_mois = engagements_du_mois(ctx, m)?;
        let bornes = du_mois.iter().map(|(_, e)| bornes_brutes(e)).collect::<R<Vec<_>>>()?;
        let ids: Vec<jumeaux::Ident> = du_mois.iter().zip(&bornes).map(|((c, e), b)| (c.clone(), b.0.clone(), jumeaux::numero_du_champ(e.get("numero")))).collect();
        for (((cle, e), (debut, fin)), (_, _, marque)) in du_mois.into_iter().zip(bornes).zip(jumeaux::cles(&ids)) {
            // UN CDI SANS TERME COURT TOUJOURS (#236) : sa fin vide se comparait comme « avant le mois », et le contrat
            // d'un permanent ne se reprenait jamais au mois suivant. Une fin vide ne vaut « en cours » que pour une
            // nature a duree indeterminee : un contrat d'usage sans jour ni fin ne court pas.
            let sans_terme = fin.is_empty()
                && e.get("nature").and_then(Value::as_str).and_then(|n| crate::contrats::nature(Some(n)).ok()).is_some_and(|n| n.duree_indeterminee);
            if debut.is_empty() || (fin < premier && !sans_terme) {
                continue;
            }
            let ident = vec![json!(cle), json!(debut), json!(marque)];
            if vus.contient(&ident)? {
                continue;
            }
            vus.poser(ident.clone(), ())?;
            let Ok(sal) = ctx.salarie(&cle) else { continue };
            // La nature d'abord : un contrat paye au mois ne se rattache a aucun spectacle
            // (S21.G00.40.050, CCH-11) et se reprend avec ce qui le definit, pas en cachets.
            let nature = match e.get("nature") {
                None | Some(Value::Null) => None,
                Some(Value::String(s)) => Some(s.as_str()),
                Some(_) => continue,
            };
            let Ok(nat) = crate::contrats::nature(nature) else { continue };
            let mensuel: Option<Map<String, Value>> = nat.mensuelle().then(|| {
                DEFINIT_LE_MENSUEL.iter().filter_map(|k| e.get(*k).filter(|v| !v.is_null()).map(|v| (k.to_string(), v.clone()))).collect()
            });
            // Un contrat d'usage joue le spectacle qu'il nomme, ou celui du mois (`engagement::engagements`).
            // Sans l'un ni l'autre, il se reprend sans spectacle : il se choisit, il ne se devine pas.
            let nom_spectacle = match e.get("spectacle").filter(|x| vrai(x)) {
                _ if mensuel.is_some() => None,
                None => spectacle_du_mois.clone(),
                Some(Value::String(s)) => Some(s.clone()),
                Some(_) => continue,
            };
            let spec = match &nom_spectacle {
                None => Value::Null,
                Some(n) => match ctx.spectacle(n) {
                    Ok(s) => s.get("_cle").filter(|x| vrai(x)).cloned().unwrap_or_else(|| json!(n)),
                    Err(_) => continue,
                },
            };
            let texte_de = |k: &str| -> R<Value> { sal.get(k).cloned().ok_or_else(|| cle_manquante(k)) };
            let prenom = match sal.get("prenom_affichage").filter(|x| vrai(x)) {
                Some(p) => p.clone(),
                None => texte_de("prenom")?,
            };
            let taux_cachet = e
                .get("lignes")
                .filter(|x| vrai(x))
                .map(|l| items(l))
                .transpose()?
                .unwrap_or_default()
                .iter()
                .find(|l| l.get("rubrique").and_then(Value::as_str) == Some("cachet"))
                .map(|l| l.get("taux").cloned().unwrap_or(Value::Null))
                .unwrap_or(Value::Null);
            out.push(EnCours {
                salarie: cle.clone(),
                nom: format!("{} {}", py_str(&texte_de("nom")?), py_str(&prenom)),
                spectacle: spec,
                debut: debut.clone(),
                fin,
                numero: e.get("numero").cloned().unwrap_or(Value::Null),
                depuis: m.clone(),
                metier: e.get("metier").cloned().unwrap_or(Value::Null),
                cadre: e.get("cadre").cloned().unwrap_or(Value::Null),
                taux_cachet,
                lieu: e.get("lieu").filter(|x| vrai(x)).cloned().unwrap_or_else(|| json!("")),
                deja_repris: false, // ci-dessous, contrat par contrat
                lignes: match e.get("lignes").filter(|x| vrai(x)) {
                    Some(Value::Array(a)) => a.clone(),
                    Some(autre) => vec![autre.clone()],
                    None => Vec::new(),
                },
                mensuel,
            });
        }
    }
    let en_cours: Vec<Option<jumeaux::Ident>> = out.iter().map(|c| Some((c.salarie.clone(), c.debut.clone(), jumeaux::numero_du_champ(Some(&c.numero))))).collect();
    for (c, repris) in out.iter_mut().zip(jumeaux::apparier(&en_cours, &deja)) {
        c.deja_repris = repris.is_some();
    }
    Ok(out)
}

/// Le `spectacle` d'un fichier de mois : celui que joue un engagement qui n'en nomme pas
/// (`engagement::engagements`). `None` s'il n'en dit pas.
fn spectacle_du_fichier(ctx: &Contexte, mois: &str) -> R<Option<String>> {
    let p = ctx.racine.join("paies").join(format!("{}.json", mois));
    if !p.exists() {
        return Ok(None);
    }
    Ok(lire_saisie(&p)?.get("spectacle").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(String::from))
}

/// Les 7 premiers caracteres (`jour[:7]`) d'un jour de planning.
fn mois_du_jour(j: &Value) -> R<String> {
    match jour_de_planning(j)? {
        Value::String(s) => Ok(s.chars().take(7).collect()),
        autre => Err(erreur("TypeError", crate::fichiers::attendu("un texte", nom_type(&autre)))),
    }
}

fn jour_de_planning(x: &Value) -> R<Value> {
    match x {
        Value::Object(m) => m.get("jour").cloned().ok_or_else(|| cle_manquante("jour")),
        autre => Ok(autre.clone()),
    }
}

/// `operations_paies.engagement_de_reprise` : l'engagement d'un contrat en
/// cours, pour le fichier de `mois`. Le contrat tel qu'il a ete signe, et ses
/// lignes de paie : les jours de `mois` du planning deviennent les dates de la
/// ligne (au meme taux, au meme lieu), ceux des mois d'apres restent en
/// planning.
///
/// Un contrat paye au mois se reprend avec sa nature et ce qui la definit, sans spectacle et sans
/// ligne : le moteur compose chaque mois celles d'un permanent (`conges::lignes_du_mois`), et des
/// lignes ecrites prendraient leur place.
fn engagement_de_reprise(ctx: &Contexte, c: &EnCours, mois: &str) -> R<Value> {
    let mut e = Map::new();
    e.insert("salarie".into(), json!(c.salarie));
    if !c.spectacle.is_null() {
        e.insert("spectacle".into(), c.spectacle.clone());
    }
    e.insert("debut".into(), json!(c.debut));
    e.insert("fin".into(), json!(c.fin));
    if vrai(&c.numero) {
        e.insert("numero".into(), c.numero.clone());
    }
    if vrai(&c.metier) {
        e.insert("metier".into(), c.metier.clone());
    }
    if !c.cadre.is_null() {
        e.insert("cadre".into(), c.cadre.clone());
    }
    if vrai(&c.lieu) {
        e.insert("lieu".into(), c.lieu.clone());
    }
    if let Some(definit) = &c.mensuel {
        e.extend(definit.clone());
        return Ok(Value::Object(e));
    }
    let mut lignes = Vec::new();
    for src in &c.lignes {
        let src = objet(src)?;
        let plan = parcourir(src.get("planning"))?;
        let mut dedans = Vec::new();
        let mut apres = Vec::new();
        for x in &plan {
            let m = mois_du_jour(x)?;
            if m == mois {
                dedans.push(x.clone());
            }
        }
        for x in &plan {
            if mois_du_jour(x)?.as_str() > mois {
                apres.push(x.clone());
            }
        }
        if dedans.is_empty() && apres.is_empty() {
            continue;
        }
        let mut l = Map::new();
        l.insert("rubrique".into(), src.get("rubrique").cloned().unwrap_or_else(|| json!("cachet")));
        l.insert("taux".into(), src.get("taux").cloned().unwrap_or(Value::Null));
        if let Some(lieu) = src.get("lieu").filter(|x| vrai(x)) {
            l.insert("lieu".into(), lieu.clone());
        }
        // Un cachet, une journee ou un service par date cochee ; les autres
        // unites (des heures) ne se deduisent pas d'un nombre de jours.
        let unite = match l["rubrique"].as_str() {
            Some(code) => catalogue::rubrique(&ctx.racine, code).map(|r| r.unite).unwrap_or_default(),
            None => String::new(),
        };
        let par_date = matches!(unite.as_str(), "cachet" | "jour" | "service") && !dedans.is_empty();
        l.insert("quantite".into(), if par_date { json!(dedans.len() as f64) } else { json!(0) });
        if par_date {
            if let Some(h) = src.get("heures").filter(|x| vrai(x)) {
                l.insert("heures".into(), h.clone());
            }
        }
        l.insert("dates".into(), Value::Array(dedans));
        if !apres.is_empty() {
            l.insert("planning".into(), Value::Array(apres));
        }
        lignes.push(Value::Object(l));
    }
    if lignes.is_empty() {
        // Le taux du cachet du contrat, sinon le dernier saisi (#140) : la page ne choisit pas de taux. Un dossier
        // qu'on ne sait pas relire, ou qui n'a encore aucun cachet (R12), n'en propose aucun (la ligne reste a
        // completer).
        let taux = if vrai(&c.taux_cachet) {
            c.taux_cachet.clone()
        } else {
            crate::routes_saisie::dernier_taux(ctx).ok().flatten().map_or(Value::Null, |t| json!(t))
        };
        lignes.push(json!({"rubrique": "cachet", "quantite": 0, "taux": taux, "dates": []}));
    }
    e.insert("lignes".into(), Value::Array(lignes));
    Ok(Value::Object(e))
}

fn contrats_en_cours_route(ctx: &Contexte, charge: &Value) -> R<Value> {
    let mois = exiger(charge, "mois")?;
    let mut contrats = Vec::new();
    for c in contrats_en_cours(ctx, mois)? {
        let mut m = c.json();
        m.insert("engagement".into(), engagement_de_reprise(ctx, &c, &py_str(mois))?);
        contrats.push(Value::Object(m));
    }
    Ok(json!({"mois": mois, "contrats": contrats}))
}

// ------------------------------------------------------------- contrats passes

const TITRE_A_COMPLETER: &str = "A COMPLETER";

/// `operations_paies.ContratPasse` et `sortie_json.brut` : un contrat qui touche
/// un mois passe, tel que le calendrier le montre. LECTURE SEULE, ET UN
/// PLANCHER : c'est ce que la compagnie a declare, pas la position du salarie.
struct Passe {
    salarie: Value,
    nom: String,
    debut: String,
    fin: String,
    cachets: i64,
    heures: i64,
    jours: i64,
    titre: Value,
    numero_objet: Value,
    dates: Vec<Value>,
    source: &'static str,
}

impl Passe {
    fn json(&self) -> Value {
        json!({
            "salarie": self.salarie, "nom": self.nom, "debut": self.debut, "fin": self.fin,
            "cachets": self.cachets, "heures": self.heures, "jours": self.jours,
            "titre": self.titre, "numero_objet": self.numero_objet,
            "dates": self.dates, "source": self.source,
        })
    }
}

/// Les spectacles retrouves par leur cle ou par leur numero d'objet.
struct Spectacles<'a> {
    par_cle: &'a Map<String, Value>,
    par_objet: Dico<&'a Value>,
}

impl<'a> Spectacles<'a> {
    fn nouveau(ctx: &'a Contexte) -> R<Spectacles<'a>> {
        let mut par_objet: Dico<&Value> = Dico::nouveau();
        for (k, v) in &ctx.spectacles {
            if k.starts_with('_') {
                continue;
            }
            if let Some(n) = objet(v)?.get("numero_objet").filter(|x| vrai(x)) {
                par_objet.poser(vec![n.clone()], v)?;
            }
        }
        Ok(Spectacles { par_cle: &ctx.spectacles, par_objet })
    }

    /// `par_cle.get(x) or par_objet.get(x)` : `x` se hache.
    fn trouver(&self, x: &Value) -> R<Option<&'a Value>> {
        if let Value::String(s) = x {
            if let Some(v) = self.par_cle.get(s).filter(|v| vrai(v)) {
                return Ok(Some(v));
            }
        }
        Ok(self.par_objet.get(&[x.clone()])?.copied())
    }
}

/// `_titre` : le titre de la fiche, ou le numero d'objet a defaut.
fn titre_de(spec: Option<&Value>, numero: &Value) -> Value {
    let t = spec.and_then(|s| s.get("titre")).filter(|x| vrai(x)).map(py_str).unwrap_or_default();
    let t = t.trim();
    if !t.is_empty() && t != TITRE_A_COMPLETER {
        json!(t)
    } else if vrai(numero) {
        numero.clone()
    } else {
        json!("")
    }
}

/// `_nom` : « NOM Prenom » d'une cle de salarie (la cle elle-meme si elle est inconnue).
fn nom_de(ctx: &Contexte, cle: &Value) -> R<String> {
    let sal = match cle {
        Value::String(s) => ctx.salaries.get(s).filter(|x| vrai(x)),
        Value::Array(_) | Value::Object(_) => return Err(erreur("TypeError", crate::fichiers::pas_une_cle(nom_type(cle)))),
        _ => None,
    };
    let (nom, prenom) = match sal {
        Some(s) => (
            s.get("nom").map(py_str).unwrap_or_else(|| py_str(cle)),
            match s.get("prenom_affichage").filter(|x| vrai(x)) {
                Some(p) => py_str(p),
                None => s.get("prenom").map(py_str).unwrap_or_default(),
            },
        ),
        None => (py_str(cle), String::new()),
    };
    Ok(format!("{} {}", nom, prenom))
}

/// `float(v)` de Python, d'une quantite saisie.
fn flottant_py(v: &Value) -> Result<f64, Echec> {
    match v {
        Value::Bool(b) => Ok(*b as i64 as f64),
        Value::Number(n) => Ok(n.as_f64().unwrap_or(0.0)),
        Value::String(s) => {
            s.trim().replace('_', "").parse::<f64>().map_err(|_| erreur("ValueError", format!("nombre illisible : {}", repr(s))))
        }
        autre => Err(erreur("TypeError", format!("float() argument must be a string or a real number, not '{}'", nom_type(autre)))),
    }
}

/// Le nombre de cachets d'un engagement : la somme des quantites de ses lignes
/// dont la rubrique est un cachet. Une ligne qui ne dit rien de lisible
/// (rubrique absente ou inconnue, quantite illisible) ne compte pas.
fn cachets_de(ctx: &Contexte, e: &Map<String, Value>) -> R<i64> {
    let mut cachets = 0;
    for l in parcourir(e.get("lignes"))? {
        let Some(code) = l.get("rubrique").and_then(Value::as_str) else { continue };
        let Ok(r) = catalogue::rubrique(&ctx.racine, code) else { continue };
        if r.unite != "cachet" {
            continue;
        }
        let q = match l.get("quantite").filter(|x| vrai(x)) {
            Some(q) => match flottant_py(q) {
                Ok(x) => x,
                Err(e) if e.classe == Some("ValueError") => continue,
                Err(e) => return Err(e),
            },
            None => 0.0,
        };
        if q.is_nan() {
            continue;
        }
        if q.is_infinite() {
            return Err(erreur("OverflowError", crate::fichiers::entier_hors_limites("infini")));
        }
        cachets += q.trunc() as i64;
    }
    Ok(cachets)
}

/// `operations_paies.contrats_passes` : les contrats qui touchent `mois`, pour
/// le calendrier des mois passes. Deux sources, la meme regle que
/// `cumul.lire_engagements` : un mois qui a son fichier de paie est lu la (avec
/// ses jours exacts), l'historique importe des AEM parle pour les autres.
/// Trie par salarie puis debut.
fn contrats_passes(ctx: &Contexte, mois: &Value, salaries: Option<&Value>) -> R<Vec<Passe>> {
    let texte = py_str(mois);
    let (premier, dernier) = (format!("{}-01", texte), format!("{}-31", texte));
    let voulus: HashSet<String> = parcourir(salaries)?.into_iter().filter_map(|v| v.as_str().map(String::from)).collect();
    let voulu = |cle: &Value| voulus.is_empty() || cle.as_str().is_some_and(|c| voulus.contains(c));
    let spectacles = Spectacles::nouveau(ctx)?;

    let mut couverts: HashSet<String> = HashSet::new();
    // Un contrat sur plusieurs mois est dans chacun de ses fichiers de mois, avec
    // les memes bornes : on le garde une fois, de preference tel que le fichier du
    // mois regarde l'ecrit (c'est lui qui a les jours joues).
    let mut vus: Dico<usize> = Dico::nouveau();
    let mut paie: Vec<Passe> = Vec::new();
    let mut connus = mois_saisis(ctx);
    connus.reverse();
    for mc in &connus {
        couverts.insert(mc.clone());
        for (cle, e) in engagements_du_mois(ctx, mc)? {
            let (debut, fin) = bornes_brutes(&e)?;
            if debut.is_empty() || debut > dernier || (if fin.is_empty() { &debut } else { &fin }).as_str() < premier.as_str() {
                continue;
            }
            let cle = Value::String(cle);
            if !voulu(&cle) {
                continue;
            }
            let ref_spec = ou(e.get("spectacle").unwrap_or(&Value::Null), &Value::String(String::new())).clone();
            let spec = spectacles.trouver(&ref_spec)?;
            let numero = match spec.and_then(|s| s.get("numero_objet")).filter(|x| vrai(x)) {
                Some(n) => n.clone(),
                None => json!(py_str(&ref_spec)),
            };
            let cachets = cachets_de(ctx, &e)?;
            let mut dates: Vec<Value> = trier(jours_des_lignes(&e, "dates")?.iter().map(jour_de).collect())?;
            let fin = if fin.is_empty() { debut.clone() } else { fin };
            let ident = vec![cle.clone(), json!(debut), json!(fin)];
            let deja = vus.get(&ident)?.copied();
            if deja.is_some() && mc.as_str() != texte {
                continue;
            }
            let jours = match e.get("jours").filter(|x| vrai(x)) {
                Some(j) => entier_py(j)?,
                None => dates.len() as i64,
            };
            let mut du_mois = Vec::new();
            for j in dates.drain(..) {
                if mois_du_jour(&j)? == texte_ou(mois, &texte) {
                    du_mois.push(j);
                }
            }
            let contrat = Passe {
                nom: nom_de(ctx, &cle)?,
                salarie: cle,
                debut,
                fin,
                cachets,
                heures: 0,
                jours,
                titre: titre_de(spec, &numero),
                numero_objet: numero,
                dates: du_mois,
                source: "paie",
            };
            match deja {
                Some(i) => paie[i] = contrat,
                None => {
                    vus.poser(ident, paie.len())?;
                    paie.push(contrat);
                }
            }
        }
    }
    let mut out = paie;
    for e in historique(ctx)? {
        let e = objet(&e)?;
        let texte_de = |k: &str| e.get(k).filter(|x| vrai(x)).map(py_str).unwrap_or_default();
        let (debut, fin) = (texte_de("debut"), texte_de("fin"));
        if debut.is_empty() || debut > dernier || fin < premier {
            continue;
        }
        // Un mois produit ici se tait dans l'historique : ce sont les memes
        // contrats, declares deux fois pendant le double run.
        if couverts.contains(&fin.chars().take(7).collect::<String>()) {
            continue;
        }
        let cle = e.get("salarie").cloned().unwrap_or_else(|| json!(""));
        if !voulus.is_empty() {
            if matches!(cle, Value::Array(_) | Value::Object(_)) {
                return Err(erreur("TypeError", crate::fichiers::pas_une_cle(nom_type(&cle))));
            }
            if !voulu(&cle) {
                continue;
            }
        }
        let numero = texte_de("numero_objet");
        let compte = |k: &str| e.get(k).filter(|x| vrai(x)).map_or(Ok(0), entier_py);
        let (cachets, heures, jours) = (compte("cachets")?, compte("heures")?, compte("jours")?);
        let numero = json!(numero);
        let spec = spectacles.par_objet.get(&[numero.clone()])?.copied();
        out.push(Passe {
            nom: nom_de(ctx, &cle)?,
            salarie: cle,
            debut,
            fin,
            cachets,
            heures,
            jours,
            titre: titre_de(spec, &numero),
            numero_objet: numero,
            dates: Vec::new(),
            source: "historique",
        });
    }
    // `out.sort(key=(salarie, debut, fin))`, stable.
    let mut refus = None;
    out.sort_by(|a, b| {
        let o = comparer(&a.salarie, &b.salarie).unwrap_or_else(|e| {
            refus.get_or_insert(e);
            Ordering::Equal
        });
        o.then_with(|| a.debut.cmp(&b.debut)).then_with(|| a.fin.cmp(&b.fin))
    });
    match refus {
        Some(e) => Err(e),
        None => Ok(out),
    }
}

/// Le mois tel que `j[:7] == mois` le compare : le texte du mois.
fn texte_ou(mois: &Value, texte: &str) -> String {
    if mois.is_string() {
        texte.to_string()
    } else {
        // Un mois qui n'est pas un texte n'egale aucun `j[:7]`.
        "\u{0}".to_string()
    }
}

/// `operations_importation.lire_historique` : les contrats de l'historique
/// importe, ou rien.
fn historique(ctx: &Contexte) -> R<Vec<Value>> {
    let p = ctx.racine.join("historique").join("engagements.json");
    if !p.exists() {
        return Ok(Vec::new());
    }
    let data = lire_saisie(&p)?;
    let Value::Object(o) = &data else {
        return Err(erreur("AttributeError", crate::fichiers::attendu("un objet", nom_type(&data))));
    };
    match o.get("engagements") {
        None => Ok(Vec::new()),
        Some(Value::Array(a)) => Ok(a.clone()),
        Some(autre) => Ok(items(autre)?.into_iter().map(|c| c.into_owned()).collect()),
    }
}

fn contrats_passes_route(ctx: &Contexte, charge: &Value) -> R<Value> {
    let mois = exiger(charge, "mois")?;
    let salaries = charge.get("salaries");
    let contrats: Vec<Value> = contrats_passes(ctx, mois, salaries)?.iter().map(Passe::json).collect();
    Ok(json!({"mois": mois, "contrats": contrats}))
}

// ------------------------------------------------------------------------ emploi

/// `emplois.resoudre(charge["metier"], cadre=charge.get("cadre"),
/// feminin=bool(charge.get("feminin")))` : un metier du catalogue traduit en
/// champs de fiche, pour le formulaire salarie.
fn emploi(charge: &Value) -> R<Value> {
    let code = exiger(charge, "metier")?;
    let cadre = match charge.get("cadre") {
        None | Some(Value::Null) => None,
        Some(c) => Some(vrai(c)),
    };
    let feminin = charge.get("feminin").is_some_and(vrai);
    let texte = match code {
        Value::String(s) => s.clone(),
        autre if !vrai(autre) => String::new(),
        autre => return Err(erreur("AttributeError", crate::fichiers::attendu("un texte", nom_type(autre)))),
    };
    let inconnu = |t: &str| {
        erreur(
            "ValueError",
            format!(
                "Metier {} inconnu : ni dans le catalogue (emplois-base.json), ni \
                 dans la nomenclature France Travail (nomenclature-emplois-ft.json).",
                if code.is_string() { repr(t) } else { repr_valeur(code) }
            ),
        )
    };
    if emplois::metier(&texte).map_err(plain)?.is_none() {
        return Err(inconnu(&texte));
    }
    emplois::resoudre(&texte, cadre, feminin).map_err(plain)
}

// ---------------------------------------------------------------- rubriques cochees

const GARDER_COPIES: usize = 20;

/// `operations_socle._sauver_avant` : la copie horodatee du fichier, avant de le
/// remplacer (l'heure de Paris), et seulement les `GARDER_COPIES` dernieres.
pub(crate) fn sauver_avant(chemin: &Path) -> R<()> {
    if !chemin.exists() {
        return Ok(());
    }
    let io = |e: std::io::Error| erreur("OSError", e.to_string());
    let parent = chemin.parent().unwrap_or(Path::new("."));
    let dossier = parent.join("avant");
    std::fs::create_dir_all(&dossier).map_err(io)?;
    let nom = chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let maintenant = crate::temps::secondes();
    let horodatage = crate::paris::iso_secondes(maintenant).replace(['-', ':'], "").replace('T', "-");
    std::fs::copy(chemin, dossier.join(format!("{}.{}", nom, horodatage))).map_err(io)?;
    let prefixe = format!("{}.", nom);
    let mut anciennes: Vec<String> = std::fs::read_dir(&dossier)
        .map_err(io)?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(&prefixe))
        .collect();
    anciennes.sort();
    if anciennes.len() > GARDER_COPIES {
        for vieille in &anciennes[..anciennes.len() - GARDER_COPIES] {
            let _ = std::fs::remove_file(dossier.join(vieille));
        }
    }
    Ok(())
}

/// Ce que `catalogue._construire_catalogue` refuse dans les fichiers de la
/// compagnie, sous le nom de l'exception de Python (`JSONDecodeError`,
/// `AttributeError`, `TypeError`) : un fichier abime, qui n'est pas un
/// dictionnaire, des `usages` qui n'en sont pas, des `actives` qui ne se
/// parcourent pas ou ne se hachent pas. Le catalogue Rust, lui, les tolere.
fn verifier_les_fichiers_du_catalogue(racine: &Path) -> R<()> {
    for nom in ["rubriques-compagnie.json", "rubriques-actives.json"] {
        let p = racine.join("config").join(nom);
        if !p.exists() {
            continue;
        }
        let v = lire_saisie(&p)?;
        let Value::Object(o) = &v else {
            return Err(erreur("AttributeError", crate::fichiers::attendu("un objet", nom_type(&v))));
        };
        if nom == "rubriques-actives.json" {
            if let Some(a) = o.get("actives") {
                for c in items(a)? {
                    if matches!(c.as_ref(), Value::Array(_) | Value::Object(_)) {
                        return Err(erreur("TypeError", crate::fichiers::pas_une_cle(nom_type(&c))));
                    }
                }
            }
            if let Some(u) = o.get("usages").filter(|u| !u.is_object()) {
                return Err(erreur("AttributeError", crate::fichiers::attendu("un objet", nom_type(u))));
            }
        }
    }
    Ok(())
}

/// `operations_catalogues.choisir_rubriques` : cocher les rubriques que la
/// compagnie veut voir dans le deroulant. Une preference d'affichage : cela
/// change l'ordre du deroulant de saisie, jamais un calcul. Une rubrique
/// inconnue est refusee plutot qu'ignoree.
fn rubriques_actives(ctx: &Contexte, charge: &Value) -> R<Value> {
    let brut = exiger(charge, "actives")?;
    verifier_les_fichiers_du_catalogue(&ctx.racine)?;
    let cat = catalogue::catalogue(&ctx.racine).map_err(plain)?;
    // `for c in actives` : une liste, les cles d'un dictionnaire, les caracteres d'un texte.
    let actives: Vec<Value> = items(brut)?.into_iter().map(|c| c.into_owned()).collect();
    let mut connus: Vec<&str> = cat.iter().map(|r| r.code.as_str()).collect();
    let mut inconnues: Vec<&Value> = Vec::new();
    for c in &actives {
        // `c not in connus` : le code se hache.
        if matches!(c, Value::Array(_) | Value::Object(_)) {
            return Err(erreur("TypeError", crate::fichiers::pas_une_cle(nom_type(c))));
        }
        if !c.as_str().is_some_and(|s| connus.contains(&s)) {
            inconnues.push(c);
        }
    }
    connus.sort();
    if !inconnues.is_empty() {
        if let Some((i, autre)) = inconnues.iter().enumerate().find(|(_, c)| !c.is_string()) {
            return Err(erreur("TypeError", crate::fichiers::element(i, &crate::fichiers::attendu("un texte", nom_type(autre)))));
        }
        let noms: Vec<&str> = inconnues.iter().filter_map(|c| c.as_str()).collect();
        return Err(erreur(
            "ErreurOperation",
            format!("Rubrique(s) inconnue(s) : {}. Connues : {}.", noms.join(", "), connus.join(", ")),
        ));
    }
    let p = ctx.racine.join("config").join("rubriques-actives.json");
    let mut donnees = if p.exists() { lire_saisie(&p)? } else { json!({}) };
    let Value::Object(d) = &mut donnees else {
        return Err(erreur("TypeError", format!("config/rubriques-actives.json : {}", crate::fichiers::attendu("un objet", nom_type(&donnees)))));
    };
    let mut uniques: Vec<Value> = Vec::new();
    for c in actives {
        if !uniques.contains(&c) {
            uniques.push(c);
        }
    }
    d.insert("actives".into(), Value::Array(uniques));
    d.entry("usages").or_insert_with(|| json!({}));
    d.insert(
        "_quoi".into(),
        json!(
            "Les rubriques que cette compagnie a cochees, et combien de fois \
             chacune a servi. Preference d'affichage : cela change l'ordre du \
             deroulant de saisie, jamais un calcul. Le catalogue lui-meme est \
             livre avec le logiciel (donnees/rubriques-base.json)."
        ),
    );
    std::fs::create_dir_all(p.parent().unwrap_or(Path::new("."))).map_err(|e| erreur("OSError", e.to_string()))?;
    sauver_avant(&p)?;
    ecrire_json(&p, &donnees, 2).map_err(plain)?;
    let rubriques: Vec<Value> = catalogue::catalogue(&ctx.racine).map_err(plain)?.iter().map(rubrique_json).collect();
    Ok(json!({"rubriques": rubriques}))
}

// ------------------------------------------------------------------ le catalogue des metiers

/// `operations_catalogues.catalogue_metiers` et `emplois.familles` : l'onglet
/// Metiers, tout le catalogue, chaque metier coche ou non.
fn metiers_catalogue(ctx: &Contexte) -> R<Value> {
    let familles = emplois::familles().map_err(plain)?;
    let ordre: HashMap<&str, usize> = familles.iter().enumerate().map(|(i, f)| (f.as_str(), i)).collect();
    let noms = emplois::noms_des_conventions().map_err(plain)?;
    let (ajoutes, retires) = preferences_metiers(&ctx.racine)?;
    let idcc = ctx.entreprise.get("idcc").filter(|x| vrai(x)).map(py_str).unwrap_or_default();
    let mut liste: Vec<Map<String, Value>> = Vec::new();
    for m in emplois::catalogue().map_err(plain)? {
        let conventions: Vec<Value> = m.get("conventions").and_then(Value::as_array).cloned().unwrap_or_default();
        // `_de_la_convention` : sans IDCC (une compagnie qui s'installe), rien ne se cache.
        let conv = conventions.is_empty() || idcc.is_empty() || conventions.iter().any(|c| c.as_str() == Some(idcc.as_str()));
        let de = if conv || conventions.is_empty() {
            String::new()
        } else {
            conventions.iter().map(|c| noms.get(&py_str(c)).map(py_str).unwrap_or_else(|| py_str(c))).collect::<Vec<_>>().join(", ")
        };
        let mut e = m.as_object().cloned().unwrap_or_default();
        e.insert("retenu".into(), json!(metier_visible(&m, &idcc, &ajoutes, &retires)));
        e.insert("de_la_convention".into(), json!(conv));
        e.insert("de".into(), json!(de));
        liste.push(e);
    }
    let texte = |m: &Map<String, Value>, k: &str| m.get(k).map(py_str).unwrap_or_default();
    liste.sort_by_cached_key(|m| (ordre.get(texte(m, "famille").as_str()).copied().unwrap_or(ordre.len()), texte(m, "libelle").to_lowercase()));
    let _ = etats::s("");
    Ok(json!({"metiers": liste, "familles": familles}))
}
