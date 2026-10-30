//! Les routes du mois a l'ecran : `saisie`, `apercu`, `enregistrer-saisie`, `ajuster`, `echeancier`, et ce que la saisie
//! consulte (`contrats-en-cours`, `contrats-passes`, `emploi`).
//!
//! `saisie` : le mois tel que l'ecran l'ouvre -- la saisie que ses engagements montrent (`engagement::stock::vue_du_mois`)
//! ou, pour un mois neuf, un squelette vide au dernier taux saisi --, avec les noms resolus en cles de fiche et les lignes
//! de paie telles que le moteur les comprend. Aucun calcul pour l'ecran : la saisie se charge jusqu'au contrat qui
//! deborde, sans chiffrer. Les engagements valides que l'ecran montre se lisent par `engagement::etats::du_mois`, qui les
//! recalcule s'ils ne sont plus justes.
//!
//! `apercu` : le mois pour l'ecran, rien d'ecrit. Un engagement valide montre la paie qu'il a ecrite ; un engagement en
//! saisie se calcule en memoire (`paies_de_l_ecran`) ; les numeros se reservent sans rien ecrire.
//!
//! `echeancier` : la date, ce qui est a faire ce mois-ci, et l'annee qui vient. Les echeances annuelles sont dans
//! `donnees/echeancier-base.json`. Les etats du journal se lisent directement (`etats.rs`).

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::routes::fiches::{maintenant, refus, texte_requis};
use crate::routes::rappels::{self, brut_valeur, texte, Etat};
use engagement::calcul::Paie;
use engagement::Avis;
use droit::catalogue;
use droit::contexte::{archivee, Contexte};
use droit::emplois;
use droit::jumeaux::{self};
use engagement::{self, Engagement};
use envois::etats::{self, get, items, ou, Dico};
use socle::date::{en_texte, jours_du_mois, Jour};
use socle::echec::{erreur, Echec, R};
use socle::fichiers::{dossier_du_code, repr, repr_valeur, vrai};
use socle::num::{dec, flottant, Decimal};
use socle::valeur::{nom_type, nombre, valeur_en_texte, valeurs_egales};



/// Les routes de ce module.
pub const ROUTES: &[&str] = &["saisie", "apercu", "echeancier", "contrats-en-cours", "contrats-passes", "emploi", "enregistrer-saisie", "ajuster"];

/// Celles qui ecrivent.
pub const ECRITURES: &[&str] = &["enregistrer-saisie", "ajuster"];

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "saisie" => saisie(ctx, charge),
        "apercu" => apercu(ctx, charge),
        "echeancier" => echeancier(ctx, charge),
        "contrats-en-cours" => contrats_en_cours_route(ctx, charge),
        "contrats-passes" => contrats_passes_route(ctx, charge),
        "emploi" => emploi(charge),
        "enregistrer-saisie" => enregistrer_saisie(ctx, charge),
        "ajuster" => ajuster(ctx, charge),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}
// ------------------------------------------------------------------ outils

fn en_echec(r: rappels::Refus) -> Echec {
    Echec { classe: r.classe, message: r.message }
}

fn vrai_opt(v: Option<&Value>) -> bool {
    v.map_or(false, vrai)
}

fn nombre_f(x: f64) -> Value {
    serde_json::Number::from_f64(x).map_or(Value::Null, Value::Number)
}

/// Un entier ecrit en texte (blancs autour admis).
fn entier(t: &str) -> R<i32> {
    t.trim().parse::<i32>().map_err(|_| erreur("ValueError", format!("nombre entier illisible : {}", socle::fichiers::repr(t))))
}

/// Les `n` premiers caracteres (`mois[:n]`) et ceux de `a` a `b` (`mois[a:b]`).
fn tranche(t: &str, a: usize, b: usize) -> String {
    t.chars().skip(a).take(b - a).collect()
}

/// Le fichier d'une saisie, lu en JSON ; un fichier illisible refuse en le nommant.
pub(crate) fn lire_saisie(chemin: &Path) -> R<Value> {
    let octets = std::fs::read(chemin).map_err(|e| erreur("OSError", format!("{} : {}", chemin.display(), e)))?;
    let texte = String::from_utf8(octets).map_err(|e| erreur("UnicodeDecodeError", format!("{} : {}", chemin.display(), e)))?;
    serde_json::from_str(&texte).map_err(|e| erreur("JSONDecodeError", e.to_string()))
}

// ============================================================ la saisie

/// La date limite de depot depassee (`echeance_dsn`, reportee), ou `None`.
pub(crate) fn mois_hors_delai(ctx: &Contexte, mois: &str) -> R<Option<Jour>> {
    let limite = declarations::echeance_dsn::de_la_compagnie(ctx, mois).map_err(|m| erreur("ValueError", m))?.jour;
    Ok(if ctx.aujourdhui > limite { Some(limite) } else { None })
}

/// UN MOIS D'AVANT LA PREMIERE EMBAUCHE N'A PAS DE DSN DUE (une compagnie ajoutee le 06/01/2025, premiere
/// embauche le 25/01, ne voit pas decembre 2024 « en retard »). La DSN est due par l'employeur pour les salaries qu'il emploie (`crate::routes::rappels::premier_mois_declarable`,
/// la borne que l'echeancier et les rappels lisent deja) : un mois passe, sans saisie, ni tenu avant ce logiciel, qui
/// precede le mois de la premiere embauche (`premiere_embauche` de la fiche Entreprise) n'est pas en retard, il n'a rien
/// a declarer. Rend la premiere embauche (`AAAA-MM-JJ`), ou `None`.
pub(crate) fn avant_la_premiere_embauche(ctx: &Contexte, mois: &str) -> R<Option<String>> {
    let embauche = ctx.entreprise.get("premiere_embauche").and_then(Value::as_str).unwrap_or("").trim().to_string();
    if embauche.len() < 7 || mois >= &embauche[..7] || engagement::stock::a_une_saisie(&ctx.racine, mois).map_err(Echec::from)? {
        return Ok(None);
    }
    if crate::import::mois::importe(&ctx.racine, mois)? || mois_hors_delai(ctx, mois)?.is_none() {
        return Ok(None);
    }
    Ok(Some(embauche))
}

/// Un mois neuf s'ouvre VIDE, avec le
/// dernier taux saisi -- `null` quand le dossier n'a encore aucun cachet (R12 :
/// aucun texte ne fixe un taux de cachet, la compagnie le saisit).
pub(crate) fn squelette(ctx: &Contexte, mois: &str) -> R<Value> {
    let fin = fin_du_mois(mois)?;
    let taux = dernier_taux(ctx)?.map_or(Value::Null, nombre_f);
    Ok(json!({ "mois": mois, "taux_cachet": taux, "date_paiement": fin, "engagements": [] }))
}

/// Le dernier jour du mois `AAAA-MM`, en ISO : la date de paiement d'un mois qui n'en dit pas.
fn fin_du_mois(mois: &str) -> R<String> {
    let (an, mo) = (entier(&tranche(mois, 0, 4))?, entier(&tranche(mois, 5, 7))?);
    if !(1..=12).contains(&mo) {
        return Err(erreur("ValueError", format!("bad month number {}; must be 1-12", mo)));
    }
    Ok(format!("{}-{:02}-{:02}", an, mo, jours_du_mois(an, mo as u32)))
}

/// `float(v)` d'une valeur de saisie, quand elle est vraie (hors `try`).
fn flottant_de(v: &Value) -> R<f64> {
    match v {
        Value::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
        Value::Number(n) => Ok(n.as_f64().unwrap_or(0.0)),
        Value::String(s) => s
            .trim()
            .parse::<f64>()
            .map_err(|_| erreur("ValueError", format!("nombre illisible : {}", socle::fichiers::repr(s)))),
        autre => Err(erreur("TypeError", format!("float() argument must be a string or a real number, not '{}'", nom_type(autre)))),
    }
}

/// Les elements de `valeur` (vide ou absente : aucun) : une liste, ou
/// les caracteres d'une chaine, les cles d'un dictionnaire (des textes sans
/// `.get`) ; autre chose refuse.
fn elements(v: Option<&Value>) -> R<Vec<Value>> {
    match v {
        None => Ok(Vec::new()),
        Some(x) if !vrai(x) => Ok(Vec::new()),
        Some(Value::Array(a)) => Ok(a.clone()),
        Some(Value::String(s)) => Ok(s.chars().map(|c| Value::String(c.to_string())).collect()),
        Some(Value::Object(o)) => Ok(o.keys().map(|k| Value::String(k.clone())).collect()),
        Some(autre) => Err(erreur("TypeError", format!("'{}' object is not reversible", nom_type(autre)))),
    }
}

#[track_caller]
fn pas_de_get(v: &Value) -> Echec {
    erreur("AttributeError", socle::fichiers::attendu("un objet", nom_type(v)))
}

/// Le taux du cachet le plus recemment saisi : une ligne de cachet d'abord, le `taux_cachet` du mois a defaut ; `None` si
/// aucun mois n'en dit rien -- aucun texte ne fixe un taux de cachet (regle 1, R12), la compagnie saisit le sien.
pub(crate) fn dernier_taux(ctx: &Contexte) -> R<Option<f64>> {
    let mut mois = engagement::stock::mois_saisis(&ctx.racine).map_err(Echec::from)?;
    mois.reverse();
    for m in mois {
        let Ok(Some(data)) = engagement::stock::vue_du_mois(&ctx.racine, &m) else { continue };
        let Some(objet) = data.as_object() else { return Err(pas_de_get(&data)) };
        for e in elements(objet.get("engagements"))?.iter().rev() {
            let Some(eng) = e.as_object() else { return Err(pas_de_get(e)) };
            for l in elements(eng.get("lignes"))?.iter().rev() {
                if let Some(t) = taux_d_une_ligne(ctx, l) {
                    return Ok(Some(t));
                }
            }
        }
        if vrai_opt(objet.get("taux_cachet")) {
            return flottant_de(&objet["taux_cachet"]).map(Some);
        }
    }
    Ok(None)
}

/// Le taux d'une ligne de cachet (le `try` de `dernier_taux`) : tout ce qui
/// leve est une ligne qui ne dit rien.
fn taux_d_une_ligne(ctx: &Contexte, l: &Value) -> Option<f64> {
    let code = l.get("rubrique")?.as_str()?;
    if droit::catalogue::rubrique(&ctx.racine, code, &ctx.lue_au).ok()?.unite != "cachet" {
        return None;
    }
    let taux = l.get("taux").filter(|t| vrai(t))?;
    flottant_de(taux).ok()
}


/// `float(Decimal)` : la valeur la plus proche, par sa representation.
fn decimal_en_f64(d: socle::num::Decimal) -> f64 {
    d.to_string().parse().unwrap_or(0.0)
}

/// « Celi » et « aaah zerbinette » deviennent les cles
/// du fichier. Une saisie qui ne se charge pas se rend telle quelle.
fn resoudre_noms(ctx: &Contexte, saisie: &Value) -> R<Value> {
    let Some(objet) = saisie.as_object() else { return Ok(saisie.clone()) };
    let Ok(paies) = engagement::lue_sans_paie(ctx, saisie) else { return Ok(saisie.clone()) };
    let mut sortie = objet.clone();
    // Le spectacle du MOIS n'est resolu que si la saisie en nommait un.
    if vrai_opt(objet.get("spectacle")) {
        let resolu = match paies.first() {
            Some(p) => {
                if !vrai(&p.spectacle) {
                    return Err(erreur("TypeError", format!("'spectacle' : {}", socle::fichiers::attendu("un objet", nom_type(&p.spectacle)))));
                }
                p.spectacle.get("_cle").cloned().ok_or_else(|| erreur("KeyError", socle::fichiers::absent("_cle")))?
            }
            None => objet["spectacle"].clone(),
        };
        sortie.insert("spectacle".into(), resolu);
    }
    let tous = elements(objet.get("engagements"))?;
    let mut engagements = Vec::new();
    for (e, p) in tous.iter().zip(&paies) {
        let Some(brut) = e.as_object() else { return Ok(saisie.clone()) };
        let mut resolu = brut.clone();
        resolu.insert("salarie".into(), p.salarie.get("_cle").cloned().ok_or_else(|| erreur("KeyError", socle::fichiers::absent("_cle")))?);
        // UN PERMANENT N'A PAS DE SPECTACLE : le numero d'objet lui est interdit.
        if vrai(&p.spectacle) {
            resolu.insert("spectacle".into(), p.spectacle.get("_cle").cloned().ok_or_else(|| erreur("KeyError", socle::fichiers::absent("_cle")))?);
        }
        // Les lignes d'un permanent sont CALCULEES, pas saisies.
        if !p.engagement.nat()?.mensuelle() {
            let brutes = elements(brut.get("lignes"))?;
            resolu.insert("lignes".into(), Value::Array(lignes_affichees(ctx, p, &brutes)?));
        }
        engagements.push(Value::Object(resolu));
    }
    sortie.insert("engagements".into(), Value::Array(engagements));
    Ok(Value::Object(sortie))
}

/// Ce qu'une ligne du fichier dit et que le moteur ne recalcule pas : la page
/// les relit pour les remettre dans ses champs, et les reecrit ensuite.
/// `base` et `taux_majoration` d'une ligne en pourcentage, surtout : sans eux,
/// la page lirait le `taux` (0.1) comme un pourcentage (0,1 %).
const CHAMPS_DE_LIGNE_GARDES: [&str; 4] = ["heures", "genre", "base", "taux_majoration"];

/// Les lignes de paie telles que le MOTEUR les a comprises.
/// `remuneration` suit les lignes du fichier une a une (`calcul::lignes_brut`).
fn lignes_affichees(ctx: &Contexte, p: &engagement::calcul::Paie, brutes: &[Value]) -> R<Vec<Value>> {
    let lignes: Vec<Map<String, Value>> = p
        .remuneration
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let mut m = Map::new();
            m.insert("rubrique".into(), Value::String(l.code.clone()));
            m.insert("quantite".into(), nombre_f(decimal_en_f64(l.quantite)));
            m.insert("taux".into(), nombre_f(decimal_en_f64(l.taux)));
            if let Some(src) = brutes.get(i).and_then(Value::as_object) {
                for k in CHAMPS_DE_LIGNE_GARDES {
                    match src.get(k) {
                        None | Some(Value::Null) => {}
                        Some(v) => {
                            m.insert(k.into(), v.clone());
                        }
                    }
                }
            }
            m
        })
        .collect();
    Ok(jours_sur_les_lignes(ctx, &p.engagement, lignes, brutes)?.into_iter().map(Value::Object).collect())
}

/// `list(x)` d'une valeur de saisie.
fn liste_lue(v: &Value) -> R<Vec<Value>> {
    match v {
        Value::Array(a) => Ok(a.clone()),
        Value::String(s) => Ok(s.chars().map(|c| Value::String(c.to_string())).collect()),
        Value::Object(o) => Ok(o.keys().map(|k| Value::String(k.clone())).collect()),
        autre => Err(erreur("TypeError", socle::fichiers::attendu("une liste", nom_type(autre)))),
    }
}

/// Pose sur chaque ligne les jours qu'elle couvre,
/// et ou (les saisies ecrites avant le 12/09/2026 portaient les jours sur
/// l'engagement).
fn jours_sur_les_lignes(
    ctx: &Contexte,
    eng: &Engagement,
    mut lignes: Vec<Map<String, Value>>,
    brutes: &[Value],
) -> R<Vec<Map<String, Value>>> {
    // Ce que le fichier disait deja ligne par ligne : on le reprend tel quel.
    let mut porte = false;
    for (i, l) in lignes.iter_mut().enumerate() {
        let vide = Value::Object(Map::new());
        let src = brutes.get(i).unwrap_or(&vide);
        let Some(src) = src.as_object() else { return Err(pas_de_get(src)) };
        if vrai_opt(src.get("dates")) {
            l.insert("dates".into(), Value::Array(liste_lue(&src["dates"])?));
            porte = true;
        }
        if vrai_opt(src.get("planning")) {
            l.insert("planning".into(), Value::Array(liste_lue(&src["planning"])?));
        }
        if vrai_opt(src.get("lieu")) {
            l.insert("lieu".into(), src["lieu"].clone());
        }
    }
    if porte || lignes.is_empty() {
        return Ok(lignes);
    }
    let cachet = |l: &Map<String, Value>| {
        l.get("rubrique")
            .and_then(Value::as_str)
            .map_or(false, |c| droit::catalogue::rubrique(&ctx.racine, c, &ctx.lue_au).map_or(false, |r| r.unite == "cachet"))
    };
    let jours: Vec<Value> = eng.dates_travail.iter().map(|j| Value::String(j.iso())).collect();
    let Some(i) = lignes.iter().position(cachet) else {
        // Pas de cachet : une journee, une masterclass. Les jours vont a la
        // premiere ligne, faute de mieux.
        lignes[0].insert("dates".into(), Value::Array(jours));
        if !eng.lieu.is_empty() {
            lignes[0].insert("lieu".into(), Value::String(eng.lieu.clone()));
        }
        return Ok(lignes);
    };
    let groupes = eng.dates_par_lieu(true);
    if groupes.len() <= 1 {
        lignes[i].insert("dates".into(), Value::Array(jours));
        let ou = groupes.first().map_or(eng.lieu.clone(), |g| g.0.clone());
        if !ou.is_empty() {
            lignes[i].insert("lieu".into(), Value::String(ou));
        }
        return Ok(lignes);
    }
    // Plusieurs lieux : la ligne de cachets se SCINDE, une par lieu.
    let mut eclatees = Vec::new();
    for (ou, jours) in groupes {
        let mut neuve = lignes[i].clone();
        neuve.insert("quantite".into(), nombre_f(jours.len() as f64));
        neuve.insert("dates".into(), Value::Array(jours.iter().map(|j| Value::String(j.iso())).collect()));
        if !ou.is_empty() {
            neuve.insert("lieu".into(), Value::String(ou));
        }
        eclatees.push(neuve);
    }
    let apres = lignes.split_off(i + 1);
    lignes.truncate(i);
    lignes.extend(eclatees);
    lignes.extend(apres);
    Ok(lignes)
}

/// Ce que la page propose quand la saisie ne le dit pas :
/// - `taux_cachet` : le taux d'une ligne de cachet neuve, celui du dernier cachet de CE mois, sinon son
///   `taux_cachet`, sinon le dernier taux saisi du dossier (`dernier_taux`), sinon rien (`null`) ;
/// - `date_paiement` : la fin du mois, la date de paiement d'un mois qui n'en dit pas (celle du squelette).
fn defauts_de_la_saisie(ctx: &Contexte, mois: &str, saisie: &Value) -> Value {
    let du_mois = || -> Option<f64> {
        let objet = saisie.as_object()?;
        for e in elements(objet.get("engagements")).ok()?.iter().rev() {
            for l in elements(e.get("lignes")).ok()?.iter().rev() {
                if let Some(t) = taux_d_une_ligne(ctx, l) {
                    return Some(t);
                }
            }
        }
        objet.get("taux_cachet").filter(|t| vrai(t)).and_then(|t| flottant_de(t).ok())
    };
    // Un dossier qu'on ne sait pas relire ne propose rien (`null`) : la route rend la saisie telle quelle, et
    // c'est l'apercu qui dira pourquoi.
    let taux = du_mois().or_else(|| dernier_taux(ctx).ok().flatten()).map_or(Value::Null, nombre_f);
    let date_paiement = fin_du_mois(mois).map_or(Value::Null, Value::String);
    json!({"taux_cachet": taux, "date_paiement": date_paiement})
}

/// LE SPECTACLE ET LE LIEU DU MOIS DESCENDENT SUR CHAQUE ENGAGEMENT. Une saisie peut les ecrire au niveau
/// du mois, comme defaut de ses engagements (`engagement::engagements` : `ou(e.spectacle, data.spectacle)`,
/// `e.lieu or data.lieu`) ; l'ecran, lui, montre le spectacle d'un engagement et le lieu d'un contrat, et n'ecrit que ce
/// qu'il montre. Ils sont donc poses sur chaque engagement qui ne dit pas le sien -- le spectacle seulement sur un
/// contrat d'usage : un permanent n'en a pas (CCH-11) --, et retires du mois. Le calcul lit la meme chose.
fn descendre_le_mois(saisie: Value) -> Value {
    let Value::Object(mut m) = saisie else { return saisie };
    let spectacle = m.remove("spectacle").filter(vrai);
    let lieu = m.remove("lieu").filter(vrai);
    if let Some(Value::Array(engagements)) = m.get_mut("engagements") {
        for e in engagements.iter_mut() {
            let Value::Object(e) = e else { continue };
            let mensuel = droit::natures::nature(e.get("nature").and_then(Value::as_str)).map_or(false, |n| n.mensuelle());
            if let Some(s) = spectacle.as_ref().filter(|_| !mensuel && !e.get("spectacle").is_some_and(vrai)) {
                e.insert("spectacle".into(), s.clone());
            }
            if let Some(l) = lieu.as_ref().filter(|_| !e.get("lieu").is_some_and(vrai)) {
                e.insert("lieu".into(), l.clone());
            }
        }
    }
    Value::Object(m)
}

/// La route `saisie`.
fn saisie(ctx: &Contexte, charge: &Value) -> R<Value> {
    let brut = charge.get("mois").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("mois")))?;
    let mois = en_texte(brut);
    let existe = engagement::stock::a_une_saisie(&ctx.racine, &mois).map_err(Echec::from)?;
    // Un mois neuf s'ouvre VIDE, avec le dernier taux saisi ; un mois que des engagements touchent prend du squelette ce
    // qu'il ne dit pas de lui.
    let mut saisie = squelette(ctx, &mois)?;
    if let Some(Value::Object(vue)) = engagement::stock::vue_a_l_ecran(&ctx.racine, &mois).map_err(Echec::from)? {
        for (k, v) in vue {
            saisie[k] = v;
        }
    }
    // UN MOIS DECLARE DONT L'ANNULE-ET-REMPLACE EST PASSE n'est pas ferme : sa paie se modifie, en le
    // confirmant a la premiere modification (`crate::regularisation::modification_de_la_paie`) -- il n'est pas en retard.
    let mut m = crate::routes::etapes::Moteur::nouveau(ctx);
    let dsn_deposee = crate::regularisation::modification_de_la_paie(&mut m, ctx, &mois)?;
    let avant_embauche = avant_la_premiere_embauche(ctx, &mois)?;
    let saisie = descendre_le_mois(saisie);
    let defauts = defauts_de_la_saisie(ctx, &mois, &saisie);
    // UN MOIS TENU AVANT CE LOGICIEL : aucun engagement de la saisie, ses engagements importes -- en cartes, et l'etat vrai
    // de sa DSN (`import::mois`). Il se lit ; rien ne s'y ajoute.
    let importe = if existe { None } else { crate::import::mois::vue(ctx, &mois)? };
    // UN MOIS A LA FOIS IMPORTE ET SAISI : ses engagements importes se montrent a cote de ceux de la saisie, en cartes
    // fermees ; ils se lisent, rien ne les calcule.
    let importes = if existe { crate::import::mois::cartes(ctx, &mois)? } else { Vec::new() };
    Ok(json!({
        "mois": brut,
        "existe": existe,
        "importe": importe,
        "importes": importes,
        "defauts": defauts,
        // Un mois passe d'avant la premiere embauche : rien n'y est du, il se lit (`avant_la_premiere_embauche`).
        "avant_premiere_embauche": avant_embauche,
        // La DSN reelle du mois est deposee : ce qu'il faut savoir avant d'en modifier la paie (`null` sinon).
        "dsn_deposee": dsn_deposee,
        "saisie": resoudre_noms(ctx, &saisie)?,
    }))
}

// ======================================================== l'echeancier

const A_FAIRE: &str = "a_faire";
const FAIT: &str = "fait";
const EN_RETARD: &str = "en_retard";
const INFORMATION: &str = "information";

const JOURS: [&str; 7] = ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"];

/// Une echeance.
struct Echeance {
    cle: String,
    quoi: String,
    mois: String,
    titre: String,
    detail: String,
    source: String,
    etat: &'static str,
    date: Option<String>,
    cochable: bool,
    annee: Option<String>,
    /// La DSN seule : `echeance_dsn::Echeance::en_json` (le jour, midi, la date nominale, le report et ses textes).
    echeance_depot: Option<Value>,
    /// DITE PAR UN RAPPEL DE L'OUVERTURE : la DSN d'un mois que la route `rappels` reclame
    /// (`crate::routes::rappels::mois_a_declarer`, la meme lecture), avec son geste (« Verifier et envoyer »). La boite de
    /// l'ouverture ne la dit qu'une fois, par ce rappel ; l'echeance reste ici pour sa date (l'etape DSN du mois la lit).
    rappel: bool,
}

impl Echeance {
    /// `dataclasses.asdict`.
    fn json(&self) -> Value {
        json!({
            "cle": self.cle, "quoi": self.quoi, "mois": self.mois, "titre": self.titre, "detail": self.detail,
            "source": self.source, "etat": self.etat, "date": self.date, "cochable": self.cochable, "annee": self.annee,
            "echeance_depot": self.echeance_depot, "rappel": self.rappel,
            // Sa pastille : la couleur et les mots de son etat (`super::pastille`).
            "pastille": super::pastille(self.etat, match self.etat { EN_RETARD => "en retard", A_FAIRE => "à faire", FAIT => "fait", _ => "pour info" }),
        })
    }
}

/// `{nom}` remplace, `{{` et `}}` s'ecrivent `{` et `}`.
fn formater(modele: &str, valeurs: &[(&str, String)]) -> R<String> {
    let mut out = String::new();
    let mut it = modele.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '{' if it.peek() == Some(&'{') => {
                it.next();
                out.push('{');
            }
            '}' if it.peek() == Some(&'}') => {
                it.next();
                out.push('}');
            }
            '{' => {
                let mut nom = String::new();
                loop {
                    match it.next() {
                        Some('}') => break,
                        Some(x) => nom.push(x),
                        None => return Err(erreur("ValueError", "une accolade ouverte n'est pas fermée avant la fin du texte")),
                    }
                }
                let v = valeurs.iter().find(|(k, _)| *k == nom).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(&nom)))?;
                out.push_str(&v.1);
            }
            '}' => return Err(erreur("ValueError", "Single '}' encountered in format string")),
            c => out.push(c),
        }
    }
    Ok(out)
}

fn libelle(mois: &str) -> R<String> {
    let n = entier(&tranche(mois, 5, 7))?;
    let nom = socle::date::nom_du_mois(n as i64, socle::date::Graphie::Accentuee).map_err(|e| erreur("IndexError", e))?;
    Ok(format!("{} {}", nom, tranche(mois, 0, 4)))
}

/// « de septembre 2026 », « d'août 2026 », « d'octobre 2026 ».
fn de(mois: &str) -> R<String> {
    let l = libelle(mois)?;
    let voyelle = l.chars().next().map_or(false, |c| "aeiouy".contains(c));
    Ok(format!("{}{}", if voyelle { "d'" } else { "de " }, l))
}

/// `_mois_plus` : `mois` decale de `n` mois.
fn mois_plus(mois: &str, n: i32) -> String {
    let an: i32 = mois[..4].parse().unwrap_or(0);
    let mo: i32 = mois[5..7].parse::<i32>().unwrap_or(1) - 1 + n;
    format!("{:04}-{:02}", an + mo.div_euclid(12), mo.rem_euclid(12) + 1)
}

/// « mercredi 23 septembre 2026 ».
fn date_en_clair(j: &Jour) -> String {
    // Le mois d'un `Jour` va de 1 a 12 : le repli (vide) ne peut pas arriver.
    let mois = socle::date::nom_du_mois(j.mois as i64, socle::date::Graphie::Accentuee).unwrap_or_default();
    format!("{} {} {} {}", JOURS[j.jour_de_la_semaine() as usize], j.jour, mois, j.an)
}

/// La DSN de `mois`, due dans le mois suivant, a midi, le jour que dit `echeance_dsn` (reporte au jour ouvre).
fn dsn(st: &Etat, jour_dsn: u32, mois: &str, modele: &Value, aujourdhui: &Jour) -> R<Echeance> {
    let echeance = declarations::echeance_dsn::du_mois(mois, jour_dsn).map_err(|m| erreur("ValueError", m))?;
    let limite = echeance.jour;
    let e = st.etat_dsn(mois).map_err(en_echec)?;
    let etat = if vrai(&e) && !vrai_opt(e.get("refusee")) {
        FAIT
    } else if *aujourdhui > limite {
        EN_RETARD
    } else {
        A_FAIRE
    };
    let suivant = mois_plus(mois, 1);
    let champ = |c: &str| -> R<&str> { modele.get(c).and_then(Value::as_str).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(c))) };
    Ok(Echeance {
        cle: format!("dsn:{}", mois),
        quoi: "dsn".into(),
        mois: suivant.clone(),
        titre: formater(champ("titre")?, &[("de_mois", de(mois)?)])?,
        detail: formater(
            champ("detail")?,
            &[("jour", limite.jour.to_string()), ("mois_suivant", libelle(&limite.mois_iso())?), ("report", echeance.pourquoi().map(|p| format!(" {}", p)).unwrap_or_default())],
        )?,
        source: champ("source")?.to_string(),
        etat,
        date: Some(limite.iso()),
        cochable: false,
        annee: None,
        echeance_depot: Some(echeance.en_json()),
        rappel: false,
    })
}

/// `config/echeancier.json` : ce que l'utilisateur a coche (`fait`).
fn cochees(ctx: &Contexte) -> R<Map<String, Value>> {
    let p = ctx.racine.join("config").join("echeancier.json");
    if !p.exists() {
        return Ok(Map::new());
    }
    let v = lire_saisie(&p)?;
    let Some(o) = v.as_object() else { return Err(pas_de_get(&v)) };
    match o.get("fait") {
        None => Ok(Map::new()),
        Some(Value::Object(f)) => Ok(f.clone()),
        // `.get("fait", {})` rend ce qui s'y trouve, et `in` s'y applique.
        Some(autre) => Err(erreur("TypeError", format!("'fait' : {}", socle::fichiers::attendu("un objet", nom_type(autre))))),
    }
}

/// Le taux AT/MP de l'annee est-il pose ?
fn atmp_pose(ctx: &Contexte, annee: i32) -> R<bool> {
    // Dit pour l'annee, ou un taux collectif que l'annee reprend d'elle-meme : rien a relever (`Contexte::taux_atmp_de`).
    Ok(matches!(ctx.taux_atmp_de(annee), Some(Ok(t)) if t.get("valeur").is_some_and(|x| !x.is_null())))
}

/// Vrai, faux, ou `None` quand on ne peut pas le savoir.
fn fait_annuel(ctx: &Contexte, e: &Value, annee: i32, faites: &Map<String, Value>) -> R<Option<bool>> {
    let comment = e.get("fait_par").and_then(Value::as_str);
    let cle = e.get("cle").and_then(Value::as_str).unwrap_or("");
    if comment == Some("exercice_clos") {
        let clos = rappels::exercice_clos(ctx, &(annee - 1).to_string()).map_err(en_echec)?;
        return Ok(Some(vrai_opt(clos.as_ref())));
    }
    // Le taux AT/MP se saisit dans la page : pose pour l'annee, il vaut releve.
    if cle == "at-mp" && atmp_pose(ctx, annee)? {
        return Ok(Some(true));
    }
    if comment == Some("coche") {
        return Ok(Some(faites.contains_key(&format!("{}:{}", cle, annee))));
    }
    Ok(None)
}

fn mois_de_l_echeance(e: &Value, rang: Option<usize>) -> R<i64> {
    let mois = e.get("mois").and_then(Value::as_array).ok_or_else(|| erreur("KeyError", socle::fichiers::absent("mois")))?;
    let x = match rang {
        Some(i) => mois.get(i),
        None => mois.last(),
    };
    x.and_then(Value::as_i64).ok_or_else(|| erreur("IndexError", socle::fichiers::trop_courte("les mois de l'échéance")))
}

/// Une echeance annuelle ; `None` quand elle ne concerne pas ce dossier.
fn annuelle(
    st: &Etat,
    jour_dsn: u32,
    e: &Value,
    annee: i32,
    faites: &Map<String, Value>,
    aujourdhui: &Jour,
    premier: &Option<String>,
) -> R<Option<Echeance>> {
    let ctx = st.ctx;
    let cle = e.get("cle").and_then(Value::as_str).ok_or_else(|| erreur("KeyError", socle::fichiers::absent("cle")))?;
    let comment = e.get("fait_par").and_then(Value::as_str);
    // La cloture d'une annee dont aucun mois n'est dans PuppetPaie (tenue
    // ailleurs) ne concerne pas ce dossier.
    if comment == Some("exercice_clos") && rappels::mois_de_l_annee(ctx, &(annee - 1).to_string()).is_empty() {
        return Ok(None);
    }
    // Une revue des spectacles sans spectacle n'a rien a revoir.
    if cle == "revue-spectacles" && ctx.spectacles.is_empty() {
        return Ok(None);
    }
    let dernier = format!("{}-{:02}", annee, mois_de_l_echeance(e, None)?);
    // Ce qui tombait avant l'arrivee de la compagnie ne la concerne pas -- sauf
    // le taux AT/MP de l'annee, sans lequel rien ne se calcule.
    if let Some(borne) = premier.as_ref().filter(|b| !b.is_empty()) {
        if cle != "at-mp" && dernier < *borne {
            return Ok(None);
        }
    }
    // `{jour_dsn}` : le jour de janvier ou la DSN de decembre de l'annee d'avant est due, reporte (`echeance_dsn`).
    let jour_dsn = declarations::echeance_dsn::du_mois(&format!("{:04}-12", annee - 1), jour_dsn).map_err(|m| erreur("ValueError", m))?.jour.jour;
    let valeurs = [("annee", annee.to_string()), ("annee_precedente", (annee - 1).to_string()), ("jour_dsn", jour_dsn.to_string())];
    let mois = format!("{}-{:02}", annee, mois_de_l_echeance(e, Some(0))?);
    let courant = format!("{:04}-{:02}", aujourdhui.an, aujourdhui.mois);
    let etat = if comment == Some("information") {
        INFORMATION
    } else {
        let fait = fait_annuel(ctx, e, annee, faites)?;
        // Une compagnie arrivee apres l'echeance n'est pas « en retard » : elle
        // a a le faire (le taux AT/MP d'une compagnie neuve).
        let arrivee_apres = premier.as_ref().map_or(false, |b| !b.is_empty() && dernier < *b);
        if fait == Some(true) {
            FAIT
        } else if courant > dernier && fait == Some(false) && !arrivee_apres {
            EN_RETARD
        } else {
            A_FAIRE
        }
    };
    let champ = |c: &str| -> R<&str> { e.get(c).and_then(Value::as_str).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(c))) };
    Ok(Some(Echeance {
        cle: format!("{}:{}", cle, annee),
        quoi: cle.to_string(),
        mois,
        titre: formater(champ("titre")?, &valeurs)?,
        detail: formater(champ("detail")?, &valeurs)?,
        source: champ("source")?.to_string(),
        etat,
        date: None,
        cochable: comment == Some("coche"),
        annee: Some(annee.to_string()),
        echeance_depot: None,
        rappel: false,
    }))
}

/// Les echeances annuelles de la convention de la fiche (`idcc`).
pub(crate) fn annuelles_de<'a>(base: &'a Value, idcc: Option<&Value>) -> R<Vec<&'a Value>> {
    let toutes = base.get("annuelles").and_then(Value::as_array).ok_or_else(|| erreur("KeyError", socle::fichiers::absent("annuelles")))?;
    let voulu = idcc.map_or("None".to_string(), en_texte);
    Ok(toutes
        .iter()
        .filter(|e| match e.get("idcc") {
            Some(l) if vrai(l) => l.as_array().map_or(false, |a| a.iter().any(|x| en_texte(x) == voulu)),
            _ => true,
        })
        .collect())
}

/// La route `echeancier` : ce que l'ouverture montre. `charge.aujourdhui`
/// (AAAA-MM-JJ) remplace le jour du contexte (la ligne de commande `echeancier`).
fn echeancier(ctx: &Contexte, charge: &Value) -> R<Value> {
    let aujourdhui = match charge.get("aujourdhui").filter(|v| vrai(v)) {
        Some(v) => Jour::lire(&en_texte(v))?,
        None => ctx.aujourdhui,
    };
    let mois_a_venir = 12;
    let base: Value = {
        let p = dossier_du_code().join("echeancier-base.json");
        lire_saisie(&p)?
    };
    let faites = cochees(ctx)?;
    let courant = format!("{:04}-{:02}", aujourdhui.an, aujourdhui.mois);
    let annuelles = annuelles_de(&base, ctx.entreprise.get("idcc"))?;
    let jour_dsn = ctx.echeance_dsn()?;
    let st = Etat { ctx, aujourdhui, journal: etats::charger(&ctx.racine)?, principal: rappels::nic_principal(ctx) };
    let modele = base.get("dsn").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("dsn")))?;

    let mut toutes: Vec<Echeance> = Vec::new();
    // Les DSN : celle qui tombe ce mois-ci (celle du mois d'avant), l'annee qui
    // vient, et les mois plus anciens QUE LES RAPPELS DISENT MANQUANTS -- la
    // meme lecture : on ne reclame pas a une compagnie qui commence les DSN
    // d'avant sa premiere embauche.
    let precedent = mois_plus(&courant, -1);
    let premier = rappels::premier_mois_declarable(&st);
    let reclames = rappels::mois_a_declarer(&st).map_err(en_echec)?;
    for m in reclames.iter().cloned() {
        if m < precedent {
            toutes.push(dsn(&st, jour_dsn, &m, modele, &aujourdhui)?);
        }
    }
    for n in -1..mois_a_venir {
        let m = mois_plus(&courant, n);
        if premier.as_ref().map_or(false, |p| !p.is_empty() && m < *p) {
            continue; // avant la premiere embauche : rien a declarer
        }
        toutes.push(dsn(&st, jour_dsn, &m, modele, &aujourdhui)?);
    }
    let derniere_annee: i32 = mois_plus(&courant, mois_a_venir)[..4].parse().unwrap_or(aujourdhui.an);
    for annee in aujourdhui.an..=derniere_annee {
        for e in &annuelles {
            if let Some(x) = annuelle(&st, jour_dsn, e, annee, &faites, &aujourdhui, &premier)? {
                toutes.push(x);
            }
        }
    }

    // UNE DSN SE DIT UNE FOIS : celle qu'un rappel reclame, avec son geste, ne se repete pas dans « Ce mois-ci ».
    for x in toutes.iter_mut() {
        x.rappel = x.quoi == "dsn" && x.etat != FAIT && reclames.iter().any(|m| x.cle == format!("dsn:{}", m));
    }
    let fin = mois_plus(&courant, mois_a_venir);
    let (mut ce_mois, mut a_venir) = (Vec::new(), Vec::new());
    for x in toutes {
        if x.mois == courant || (x.mois < courant && x.etat == EN_RETARD) {
            ce_mois.push(x);
        } else if courant < x.mois && x.mois <= fin {
            a_venir.push(x);
        }
    }
    // Pour l'annee EN COURS, une echeance annuelle etalee sur plusieurs mois (le
    // FCAP-SVP, de janvier a mars) se dit a chacun d'eux tant qu'elle n'est pas faite.
    for e in &annuelles {
        let mois_de: Vec<String> = e
            .get("mois")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_i64).map(|m| format!("{}-{:02}", aujourdhui.an, m)).collect())
            .unwrap_or_default();
        if mois_de.iter().skip(1).any(|m| *m == courant) {
            if let Some(x) = annuelle(&st, jour_dsn, e, aujourdhui.an, &faites, &aujourdhui, &premier)? {
                if x.etat == A_FAIRE {
                    ce_mois.push(x);
                }
            }
        }
    }
    let ordre = |etat: &str| match etat {
        EN_RETARD => 0,
        A_FAIRE => 1,
        INFORMATION => 2,
        _ => 3,
    };
    ce_mois.sort_by(|a, b| {
        (ordre(a.etat), a.date.clone().unwrap_or_else(|| a.mois.clone())).cmp(&(ordre(b.etat), b.date.clone().unwrap_or_else(|| b.mois.clone())))
    });
    a_venir.sort_by(|a, b| (a.mois.clone(), a.date.clone().unwrap_or_default()).cmp(&(b.mois.clone(), b.date.clone().unwrap_or_default())));

    let texte = |c: &str| ctx.entreprise.get(c).filter(|v| vrai(v)).map(en_texte);
    let nom = texte("nom_affichage").or_else(|| texte("raison_sociale")).unwrap_or_default();
    Ok(json!({
        "aujourdhui": aujourdhui.iso(),
        "date": date_en_clair(&aujourdhui),
        "bienvenue": "Bienvenue sur PuppetPaie",
        "compagnie": nom,
        "idcc": ctx.entreprise.get("idcc").cloned().unwrap_or(Value::Null),
        "ce_mois": ce_mois.iter().map(Echeance::json).collect::<Vec<_>>(),
        "a_venir": a_venir.iter().map(Echeance::json).collect::<Vec<_>>(),
        "a_faire": ce_mois.iter().filter(|x| !x.rappel && (x.etat == A_FAIRE || x.etat == EN_RETARD)).count(),
    }))
}

#[track_caller]
fn plain(m: impl Into<String>) -> Echec {
    socle::echec::noter_et(None, m)
}

#[track_caller]
fn cle_manquante(cle: &str) -> Echec {
    erreur("KeyError", socle::fichiers::absent(cle))
}

/// `charge["cle"]`.
fn exiger<'a>(charge: &'a Value, cle: &str) -> R<&'a Value> {
    charge.get(cle).ok_or_else(|| cle_manquante(cle))
}

/// L'ordre de ce que des fichiers de saisie portent : des textes,
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
    erreur("TypeError", socle::fichiers::incomparables(nom_type(a), nom_type(b)))
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
    v.as_object().ok_or_else(|| erreur("AttributeError", socle::fichiers::attendu("un objet", nom_type(v))))
}

/// Un entier, de ce qu'un JSON porte.
fn entier_de(v: &Value) -> R<i64> {
    match v {
        Value::Bool(b) => Ok(*b as i64),
        Value::Number(n) => match n.as_i64() {
            Some(i) => Ok(i),
            None => {
                let x = n.as_f64().unwrap_or(0.0);
                if x.is_nan() {
                    Err(erreur("ValueError", "un nombre indefini (NaN) n'est pas un entier"))
                } else if x.is_infinite() {
                    Err(erreur("OverflowError", socle::fichiers::entier_hors_limites("infini")))
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
            socle::fichiers::attendu("un nombre entier", nom_type(autre)),
        )),
    }
}

// ------------------------------------------------------- les jours d'un engagement

/// Les jours d'un engagement (`cle` = "dates") et
/// `planning_engagement` (`cle` = "planning") : les jours des lignes, sans
/// doublon, nus ou detailles `{jour, lieu}`. Les refus :
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

/// (debut, fin) d'un engagement brut,
/// AAAA-MM-JJ, comme `Engagement` les deduirait ; vides sans aucun jour.
pub(crate) fn bornes_brutes(e: &Map<String, Value>) -> R<(String, String)> {
    let jours = trier(jours_des_lignes(e, "dates")?.iter().map(jour_de).collect())?;
    let plan = trier(jours_des_lignes(e, "planning")?.iter().map(jour_de).collect())?;
    let ecrit = |k: &str| e.get(k).filter(|x| vrai(x)).cloned();
    let debut = ecrit("debut").or_else(|| jours.first().cloned()).map(|v| valeur_en_texte(&v)).unwrap_or_default();
    let fin = if !jours.is_empty() || !plan.is_empty() {
        let mut tous = jours.clone();
        tous.extend(plan);
        match ecrit("fin") {
            Some(f) => valeur_en_texte(&f),
            None => valeur_en_texte(&maximum(&tous)?),
        }
    } else {
        ecrit("fin").map(|f| valeur_en_texte(&f)).unwrap_or_default()
    };
    Ok((debut, fin))
}

// ------------------------------------------------------------ les fichiers de mois

/// Les engagements bruts que le mois montre (`engagement::stock::vue_du_mois`), avec la cle du salarie ; vide si aucun
/// engagement ne le touche.
/// Un engagement dont le salarie est inconnu (ou qui n'est pas un dictionnaire)
/// est sauté.
pub(crate) fn engagements_du_mois(ctx: &Contexte, mois: &str) -> R<Vec<(String, Map<String, Value>)>> {
    engagements_de(ctx, engagement::stock::vue_du_mois(&ctx.racine, mois).map_err(Echec::from)?)
}

/// `engagements_du_mois` de ce que l'ecran du mois montre (`engagement::stock::vue_a_l_ecran`) : avec ceux auxquels il
/// manque un fait.
fn engagements_a_l_ecran(ctx: &Contexte, mois: &str) -> R<Vec<(String, Map<String, Value>)>> {
    engagements_de(ctx, engagement::stock::vue_a_l_ecran(&ctx.racine, mois).map_err(Echec::from)?)
}

fn engagements_de(ctx: &Contexte, vue: Option<Value>) -> R<Vec<(String, Map<String, Value>)>> {
    let Some(data) = vue else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for e in parcourir(get(&data, "engagements")?.into())? {
        let Some(m) = e.as_object() else { continue };
        let Some(cle) = engagement::cle_de_l_engagement(ctx, &e) else { continue };
        out.push((cle, m.clone()));
    }
    Ok(out)
}

// ------------------------------------------------------------- contrats en cours

/// Un contrat ouvert un mois precedent et
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

/// Les contrats des mois precedents dont
/// la fin depasse le 1er de `mois`. Lu dans les fichiers de saisie des mois qui
/// precedent, jamais dans la DSN.
fn contrats_en_cours(ctx: &Contexte, mois: &Value) -> R<Vec<EnCours>> {
    let texte = valeur_en_texte(mois);
    let premier = format!("{}-01", texte);
    // LE CONTRAT, PAS LE SALARIE DU JOUR (`jumeaux`) : deux contrats du meme salarie qui commencent le meme jour se
    // reprennent chacun, et chacun se dit repris ou non.
    let mut deja: Vec<Option<jumeaux::Ident>> = Vec::new();
    for (c, e) in engagements_a_l_ecran(ctx, &texte)? {
        deja.push(Some((c, bornes_brutes(&e)?.0, jumeaux::numero_du_champ(e.get("numero")))));
    }
    let precedents = engagement::stock::mois_saisis(&ctx.racine).map_err(Echec::from)?;
    let Some(texte_mois) = mois.as_str() else {
        // `x.mois < mois` : un texte et autre chose ne se comparent pas.
        return match precedents.first() {
            Some(_) => Err(erreur("TypeError", socle::fichiers::incomparables("str", nom_type(mois)))),
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
            // UN CDI SANS TERME COURT TOUJOURS : sa fin vide se comparait comme « avant le mois », et le contrat
            // d'un permanent ne se reprenait jamais au mois suivant. Une fin vide ne vaut « en cours » que pour une
            // nature a duree indeterminee : un contrat d'usage sans jour ni fin ne court pas.
            let sans_terme = fin.is_empty()
                && e.get("nature").and_then(Value::as_str).and_then(|n| droit::natures::nature(Some(n)).ok()).is_some_and(|n| n.duree_indeterminee);
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
            let Ok(nat) = droit::natures::nature(nature) else { continue };
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
                nom: format!("{} {}", valeur_en_texte(&texte_de("nom")?), valeur_en_texte(&prenom)),
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
    // LE CONTRAT D'UN PERMANENT REPRIS D'UN AUTRE LOGICIEL : aucun fichier de saisie ne le porte, l'historique
    // si (`import::nature::repris_en_cours`, quand sa nature, son debut et son numero ont ete dits). Il se reprend
    // comme les autres, avec sa nature, son debut, son numero, son salaire mensuel et sa duree du travail.
    // Ensuite, il se recopie d'un mois a l'autre comme tout contrat paye au mois.
    for r in crate::import::nature::repris_en_cours(&ctx.racine, texte_mois) {
        let ident = vec![json!(r.salarie), json!(r.debut), json!(r.numero)];
        if vus.contient(&ident)? {
            continue;
        }
        vus.poser(ident, ())?;
        let Ok(sal) = ctx.salarie(&r.salarie) else { continue };
        let t = |k: &str| sal.get(k).filter(|x| vrai(x)).map(valeur_en_texte).unwrap_or_default();
        let prenom = if t("prenom_affichage").is_empty() { t("prenom") } else { t("prenom_affichage") };
        let mut mensuel = Map::new();
        mensuel.insert("nature".into(), json!(r.nature));
        if let Some(s) = &r.salaire_mensuel {
            mensuel.insert("salaire_mensuel".into(), s.clone());
        }
        if let Some(q) = &r.quotite {
            mensuel.insert("quotite_contrat".into(), q.clone());
        }
        out.push(EnCours {
            salarie: r.salarie.clone(),
            nom: format!("{} {}", t("nom"), prenom),
            spectacle: Value::Null,
            debut: r.debut,
            fin: r.fin,
            numero: json!(r.numero),
            depuis: r.depuis,
            metier: Value::Null,
            cadre: Value::Null,
            taux_cachet: Value::Null,
            lieu: json!(""),
            deja_repris: false,
            lignes: Vec::new(),
            mensuel: Some(mensuel),
        });
    }
    // LE CONTRAT D'USAGE COMMENCE DANS L'ANCIEN LOGICIEL ET CONTINUE ICI (`engagement::importe::Importe::continue_apres`) :
    // ses mois verses sont importes, les suivants se paient ici, sous le numero et les dates que la DSN deposee lui a
    // donnes (regle 4). Le spectacle est celui de son numero d'objet, le cachet celui que l'ancien logiciel a paye.
    for i in engagement::importe::qui_continuent(&ctx.racine, texte_mois).map_err(Echec::from)? {
        let b = &i.objet().brut;
        let t = |x: &Map<String, Value>, k: &str| x.get(k).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
        let (salarie, debut, fin, numero) = (t(b, "salarie"), t(b, "debut"), t(b, "fin"), t(b, "numero"));
        let ident = vec![json!(salarie), json!(debut), json!(numero)];
        if vus.contient(&ident)? {
            continue;
        }
        vus.poser(ident, ())?;
        let Ok(sal) = ctx.salarie(&salarie) else { continue };
        let paies = i.paies();
        let derniere = paies.values().last().copied().cloned().unwrap_or_default();
        let objet_du = |x: &Map<String, Value>| ["numero_objet", "objet"].iter().map(|k| t(x, k)).find(|o| !o.is_empty()).unwrap_or_default();
        let numero_objet = i.contrat().map(objet_du).filter(|o| !o.is_empty()).unwrap_or_else(|| objet_du(&derniere));
        let spectacle = ctx
            .spectacles
            .iter()
            .filter(|(k, _)| !k.starts_with('_'))
            .find(|(_, s)| !numero_objet.is_empty() && s.get("numero_objet").map(valeur_en_texte).is_some_and(|n| n.eq_ignore_ascii_case(&numero_objet)))
            .map_or(Value::Null, |(k, _)| json!(k));
        let taux_cachet = derniere
            .get("lignes")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .find(|l| l.get("rubrique").and_then(Value::as_str) == Some("cachet"))
            .and_then(|l| l.get("taux").and_then(|x| valeur_en_texte(x).parse::<f64>().ok()))
            .map_or(Value::Null, nombre_f);
        let ts = |k: &str| sal.get(k).filter(|x| vrai(x)).map(valeur_en_texte).unwrap_or_default();
        let prenom = if ts("prenom_affichage").is_empty() { ts("prenom") } else { ts("prenom_affichage") };
        out.push(EnCours {
            salarie: salarie.clone(),
            nom: format!("{} {}", ts("nom"), prenom),
            spectacle,
            debut,
            fin,
            numero: json!(numero),
            depuis: i.continue_apres().unwrap_or_default(),
            metier: Value::Null,
            cadre: Value::Null,
            taux_cachet,
            lieu: json!(""),
            deja_repris: false,
            lignes: Vec::new(),
            mensuel: None,
        });
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
    let Some(data) = engagement::stock::vue_du_mois(&ctx.racine, mois).map_err(Echec::from)? else {
        return Ok(None);
    };
    Ok(data.get("spectacle").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(String::from))
}

/// Les 7 premiers caracteres (`jour[:7]`) d'un jour de planning.
fn mois_du_jour(j: &Value) -> R<String> {
    match jour_de_planning(j)? {
        Value::String(s) => Ok(s.chars().take(7).collect()),
        autre => Err(erreur("TypeError", socle::fichiers::attendu("un texte", nom_type(&autre)))),
    }
}

fn jour_de_planning(x: &Value) -> R<Value> {
    match x {
        Value::Object(m) => m.get("jour").cloned().ok_or_else(|| cle_manquante("jour")),
        autre => Ok(autre.clone()),
    }
}

/// L'engagement d'un contrat en
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
            Some(code) => catalogue::rubrique(&ctx.racine, code, &ctx.lue_au).map(|r| r.unite).unwrap_or_default(),
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
        // Le taux du cachet du contrat, sinon le dernier saisi : la page ne choisit pas de taux. Un dossier
        // qu'on ne sait pas relire, ou qui n'a encore aucun cachet (R12), n'en propose aucun (la ligne reste a
        // completer).
        let taux = if vrai(&c.taux_cachet) {
            c.taux_cachet.clone()
        } else {
            crate::routes::mois::dernier_taux(ctx).ok().flatten().map_or(Value::Null, |t| json!(t))
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
        m.insert("engagement".into(), engagement_de_reprise(ctx, &c, &valeur_en_texte(mois))?);
        contrats.push(Value::Object(m));
    }
    Ok(json!({"mois": mois, "contrats": contrats}))
}

// ------------------------------------------------------------- contrats passes

const TITRE_A_COMPLETER: &str = "A COMPLETER";

/// Un contrat qui touche
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

    /// `x` se hache.
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
    let t = spec.and_then(|s| s.get("titre")).filter(|x| vrai(x)).map(valeur_en_texte).unwrap_or_default();
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
        Value::Array(_) | Value::Object(_) => return Err(erreur("TypeError", socle::fichiers::pas_une_cle(nom_type(cle)))),
        _ => None,
    };
    let (nom, prenom) = match sal {
        Some(s) => (
            s.get("nom").map(valeur_en_texte).unwrap_or_else(|| valeur_en_texte(cle)),
            match s.get("prenom_affichage").filter(|x| vrai(x)) {
                Some(p) => valeur_en_texte(p),
                None => s.get("prenom").map(valeur_en_texte).unwrap_or_default(),
            },
        ),
        None => (valeur_en_texte(cle), String::new()),
    };
    Ok(format!("{} {}", nom, prenom))
}

/// Une quantite saisie, en flottant.
fn quantite_de(v: &Value) -> Result<f64, Echec> {
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
        let Ok(r) = catalogue::rubrique(&ctx.racine, code, &ctx.lue_au) else { continue };
        if r.unite != "cachet" {
            continue;
        }
        let q = match l.get("quantite").filter(|x| vrai(x)) {
            Some(q) => match quantite_de(q) {
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
            return Err(erreur("OverflowError", socle::fichiers::entier_hors_limites("infini")));
        }
        cachets += q.trunc() as i64;
    }
    Ok(cachets)
}

/// Les contrats qui touchent `mois`, pour
/// le calendrier des mois passes. Deux sources, la meme regle que
/// Un mois qui a son fichier de paie est lu la (avec
/// ses jours exacts), l'historique importe des AEM parle pour les autres.
/// Trie par salarie puis debut.
fn contrats_passes(ctx: &Contexte, mois: &Value, salaries: Option<&Value>) -> R<Vec<Passe>> {
    let texte = valeur_en_texte(mois);
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
    let mut connus = engagement::stock::mois_saisis(&ctx.racine).map_err(Echec::from)?;
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
                None => json!(valeur_en_texte(&ref_spec)),
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
                Some(j) => entier_de(j)?,
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
        let texte_de = |k: &str| e.get(k).filter(|x| vrai(x)).map(valeur_en_texte).unwrap_or_default();
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
                return Err(erreur("TypeError", socle::fichiers::pas_une_cle(nom_type(&cle))));
            }
            if !voulu(&cle) {
                continue;
            }
        }
        let numero = texte_de("numero_objet");
        let compte = |k: &str| e.get(k).filter(|x| vrai(x)).map_or(Ok(0), entier_de);
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

/// Les contrats que les engagements importes attestent (`engagement::importe::contrats_lus`).
fn historique(ctx: &Contexte) -> R<Vec<Value>> {
    engagement::importe::contrats_lus(&ctx.racine).map_err(Echec::from)
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
    // LE METIER AU FEMININ pour une salariee : le catalogue porte les deux formes (`feminin`), la fiche dit le sexe (le
    // premier chiffre du NIR, sinon la case `sexe` : `ntt::sexe`). `feminin` donne seul suffit aussi.
    let feminin = charge.get("feminin").is_some_and(vrai)
        || charge.get("salarie").is_some_and(|s| declarations::ntt::sexe(s) == "2");
    let texte = match code {
        Value::String(s) => s.clone(),
        autre if !vrai(autre) => String::new(),
        autre => return Err(erreur("AttributeError", socle::fichiers::attendu("un texte", nom_type(autre)))),
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

/// La copie horodatee du fichier, avant de le
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
    let maintenant = socle::temps::secondes();
    let horodatage = socle::paris::iso_secondes(maintenant).replace(['-', ':'], "").replace('T', "-");
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

// ------------------------------------------------------------------ le catalogue des metiers


/// `refuser_si_clos` : un exercice cloture ne bouge plus.
pub(crate) fn refuser_si_clos(ctx: &Contexte, mois: &str) -> R<()> {
    let annee: String = mois.chars().take(4).collect();
    if let Some(clos) = crate::routes::rappels::exercice_clos(ctx, &annee)?.filter(vrai) {
        let le: String = clos.get("le").filter(|v| vrai(v)).map_or_else(|| "?".to_string(), valeur_en_texte).chars().take(10).collect();
        return Err(refus(format!(
            "L'exercice {} est cloture depuis le {} : {} ne se modifie plus. Rouvrir l'exercice (Historique > L'exercice, ou \
             `moteur cloture {} --rouvrir`) avant de toucher a ce mois.",
            annee, le, mois, annee
        )));
    }
    Ok(())
}

/// UN MOIS TENU AVANT CE LOGICIEL NE SE SAISIT PAS : ses paies ont ete calculees et declarees ailleurs (ses engagements
/// sont importes, `import::mois`). Une saisie le ferait recalculer ici et proposerait d'en ecrire la DSN -- une
/// declaration deja faite ne se regenere pas (regle 4). Rien ne passe outre.
pub(crate) fn refuser_si_importe(ctx: &Contexte, mois: &str) -> R<()> {
    if crate::import::mois::importe(&ctx.racine, mois)? {
        return Err(refus(format!(
            "{} a ete tenu avant PuppetPaie : ses paies ont ete faites par l'ancien logiciel, ses engagements sont \
             importes. Il se lit dans la page Paie ; il ne se saisit pas ici.",
            mois
        )));
    }
    Ok(())
}

/// UN MOIS D'AVANT LA PREMIERE EMBAUCHE NE SE SAISIT PAS (`crate::routes::mois::avant_la_premiere_embauche`) : aucune DSN n'y
/// est due.
pub(crate) fn refuser_si_avant_la_premiere_embauche(ctx: &Contexte, mois: &str) -> R<()> {
    if let Some(embauche) = crate::routes::mois::avant_la_premiere_embauche(ctx, mois)? {
        return Err(refus(format!(
            "{} precede la premiere embauche de la compagnie ({}) : aucune DSN n'y est due, rien ne s'y saisit. Si la \
             compagnie employait deja, corriger la date de premiere embauche (Configuration › Entreprise). Rien n'est ecrit.",
            mois,
            Jour::lire(&embauche).map(|j| j.jj_mm_aaaa()).unwrap_or(embauche)
        )));
    }
    Ok(())
}

fn enregistrer_saisie(ctx: &Contexte, charge: &Value) -> R<Value> {
    let mois = texte_requis(charge, "mois")?;
    let confirme = charge.get(crate::regularisation::CONFIRMATION).is_some_and(vrai);
    let Value::Object(brute) = exiger(charge, "saisie")? else {
        return Err(erreur("TypeError", "`saisie` : un objet est attendu"));
    };
    let p = ecrire_saisie_confirmee(ctx, mois, brute, confirme)?;
    // Ce que la page dit du mois deja declare, a jour apres l'ecriture (`crate::regularisation::modification_de_la_paie`).
    let mut m = crate::routes::etapes::Moteur::nouveau(ctx);
    let dsn = crate::regularisation::modification_de_la_paie(&mut m, ctx, mois)?;
    // Ce qui manque encore, engagement par engagement, dans l'ordre de l'ecran (`null` : rien).
    let a_completer: Vec<Value> = engagement::stock::vue_a_l_ecran(&ctx.racine, mois)
        .map_err(Echec::from)?
        .and_then(|v| v.get("engagements").and_then(Value::as_array).cloned())
        .unwrap_or_default()
        .iter()
        .map(|e| e.get(engagement::stock::A_COMPLETER).cloned().unwrap_or(Value::Null))
        .collect();
    // « Enregistré à HH:MM » : l'heure du moteur (`temps.rs`), pas celle du navigateur (au banc, le temps est simule).
    Ok(json!({"chemin": p.display().to_string(), "dsn_deposee": dsn, "enregistre_a": maintenant(), "a_completer": a_completer}))
}

/// LA SAISIE D'UN MOIS DONT LA DSN EST DEPOSEE ET L'ANNULE-ET-REMPLACE PASSE : une ecriture qui en change la paie
/// (`crate::routes::etapes::empreinte_de` : ni la validation ni ce que le moteur tient seul) se refuse sans `confirme`
/// (`crate::regularisation::CONFIRMATION` dans la charge), sous `CLASSE_A_CONFIRMER`, avec l'avertissement que la page
/// montre ; rien n'est ecrit. Avec, elle s'ecrit ; revalide, l'engagement fait partir sa correction dans la DSN d'un
/// mois suivant (`produire_regularisations::corrections_dues`).
///
/// UNE SAISIE S'ECRIT A LA FOIS : la fenetre sert ses routes en parallele, et la page enregistre une seconde apres la
/// derniere frappe, meme si l'enregistrement d'avant n'a pas fini. Deux ecritures du meme mois passent l'une apres
/// l'autre, dans ce processus (`SAISIE_EN_COURS`).
pub(crate) fn ecrire_saisie_confirmee(ctx: &Contexte, mois: &str, brute: &Map<String, Value>, confirme: bool) -> R<PathBuf> {
    static SAISIE_EN_COURS: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _une_a_la_fois = SAISIE_EN_COURS.lock().unwrap_or_else(|e| e.into_inner());
    ecrire_saisie_seule(ctx, mois, brute, confirme)
}

/// `ecrire_saisie_confirmee`, la seule ecriture en cours.
fn ecrire_saisie_seule(ctx: &Contexte, mois: &str, brute: &Map<String, Value>, confirme: bool) -> R<PathBuf> {
    let mut donnees = brute.clone();
    donnees.insert("mois".into(), Value::String(mois.to_string()));
    let donnees = Value::Object(donnees);
    refuser_si_importe(ctx, mois)?;
    refuser_si_clos(ctx, mois)?;
    let mut m = crate::routes::etapes::Moteur::nouveau(ctx);
    let declaree = crate::regularisation::modification_de_la_paie(&mut m, ctx, mois)?;
    let a_confirmer = declaree.get("confirmer").is_some_and(vrai);
    refuser_si_avant_la_premiere_embauche(ctx, mois)?;
    // Ce qui manque a chaque engagement pour se calculer : il s'ecrit avec, et seule sa validation refuse.
    let manques = manques_de_la_saisie(ctx, &donnees);
    if a_confirmer {
        // Ce qui change LA PAIE (`sans_bruit`) ; un mois sans saisie n'a rien de declare a changer.
        let actuelle = engagement::stock::vue_du_mois(&ctx.racine, mois).map_err(plain)?;
        let change = actuelle.as_ref().is_some_and(|a| crate::routes::etapes::empreinte_de(a) != crate::routes::etapes::empreinte_de(&donnees));
        // Ce que la norme ne permet pas de declarer apres la limite (un contrat declare retire, ses dates changees) se
        // refuse, confirme ou non : accepte, il ne serait jamais declare (`crate::regularisation::refus_non_declarable`).
        if change {
            let base = crate::regularisation::saisie_declaree(ctx, mois)?.or_else(|| actuelle.clone());
            if let Some(base) = base {
                if let Some(texte) = crate::regularisation::refus_non_declarable(ctx, mois, &base, &donnees)? {
                    return Err(erreur(crate::regularisation::CLASSE_NON_DECLARABLE, texte));
                }
            }
        }
        if change && !confirme {
            let texte = declaree.get("avertissement").map(valeur_en_texte).unwrap_or_default();
            return Err(erreur(crate::regularisation::CLASSE_A_CONFIRMER, texte));
        }
    }
    engagement::stock::ecrire_vue_du_mois(ctx, mois, &donnees, &manques).map_err(|m| {
        if manques.iter().flatten().any(|x| *x == m) {
            refus_du_calcul(ctx, &donnees, m)
        } else {
            erreur("ErreurOperation", m)
        }
    })?;
    // UN MOIS FAIT EN DEMO, retravaille dans la compagnie gardee : il cesse d'etre « de demo » (`suppression::sortir_de_la_demo`).
    crate::interface::suppression::sortir_de_la_demo(&ctx.racine, mois).map_err(|m| erreur("OSError", m))?;
    Ok(engagement::stock::chemin(&ctx.racine, mois))
}

/// CE QUI MANQUE A CHAQUE ENGAGEMENT DE LA SAISIE `data` pour se calculer, dans son ordre (`None` : rien). Une saisie
/// qui se calcule entiere n'a rien qui manque. Sinon, chaque engagement se calcule a la suite de ceux d'avant qui se
/// calculent (le calcul d'un engagement lit ceux qui le precedent) : celui qui refuse porte le refus, et les suivants se
/// calculent sans lui.
pub(crate) fn manques_de_la_saisie(ctx: &Contexte, data: &Value) -> Vec<Option<String>> {
    let tous = data.get("engagements").and_then(Value::as_array).cloned().unwrap_or_default();
    if engagement::pour_l_ecran(ctx, data).is_ok() {
        return vec![None; tous.len()];
    }
    let mut gardes: Vec<Value> = Vec::new();
    let mut manques = Vec::new();
    for (i, e) in tous.iter().enumerate() {
        let mut essai = data.clone();
        essai["engagements"] = Value::Array(gardes.iter().cloned().chain(std::iter::once(e.clone())).collect());
        match engagement::pour_l_ecran(ctx, &essai) {
            Ok(_) => {
                gardes.push(e.clone());
                manques.push(None);
            }
            // Le refus nomme l'engagement par son rang dans l'essai : c'est son rang dans la saisie.
            Err(m) => manques.push(Some(m.replacen(&format!("L'engagement {} de", gardes.len() + 1), &format!("L'engagement {} de", i + 1), 1))),
        }
    }
    manques
}

/// CE QUI MANQUAIT, RELU APRES UNE ECRITURE : un fait se complete aussi hors de la saisie (la sante sur la fiche, le
/// contrat de prevoyance de l'entreprise). Chaque mois ou un engagement attend un fait (`engagement::stock::A_COMPLETER`)
/// se recalcule sous la configuration de ce mois ; ce qui manque se reecrit s'il a change. Rien d'autre ne change : la
/// saisie reecrite est celle que l'ecran montre. Sans engagement qui attend, rien ne se calcule.
pub(crate) fn relire_les_manques(base: &Contexte) -> R<()> {
    let mut mois: Vec<String> = engagement::stock::objets(&base.racine)
        .map_err(Echec::from)?
        .iter()
        .filter(|o| o.a_completer().is_some())
        .filter_map(|o| o.brut.get(engagement::stock::A_COMPLETER)?.get("mois")?.as_str().map(String::from))
        .collect();
    mois.sort();
    mois.dedup();
    for m in mois {
        let ctx = base.du_mois(&m)?;
        let Some(vue) = engagement::stock::vue_a_l_ecran(&ctx.racine, &m).map_err(Echec::from)? else { continue };
        let dits: Vec<Option<String>> = vue["engagements"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|e| e.get(engagement::stock::A_COMPLETER).and_then(Value::as_str).map(String::from))
            .collect();
        let manques = manques_de_la_saisie(&ctx, &vue);
        if manques != dits {
            engagement::stock::ecrire_vue_du_mois(&ctx, &m, &vue, &manques).map_err(plain)?;
        }
    }
    Ok(())
}

fn centimes(x: Decimal) -> Decimal {
    let mut r = x.round_dp(2);
    r.rescale(2);
    r
}

/// Une valeur saisie : ni absente, ni `null`, ni vide.
fn rempli(v: Option<&Value>) -> bool {
    !matches!(v, None | Some(Value::Null)) && v.and_then(Value::as_str) != Some("")
}

fn lignes_de(e: &Value) -> Vec<Value> {
    match e.get("lignes") {
        Some(Value::Array(a)) => a.clone(),
        _ => Vec::new(),
    }
}

/// Ce qui a change, engagement par engagement.
fn ce_qui_a_change(avant: &Value, apres: &Value, index: usize) -> Value {
    let nul = Value::Null;
    let lire = |v: &'_ Value, k: &str| v.get(k).cloned().unwrap_or(Value::Null);
    let mut d = Map::new();
    d.insert("index".into(), json!(index));
    if !valeurs_egales(&lire(apres, "salaire_mensuel"), &lire(avant, "salaire_mensuel")) {
        d.insert("salaire_mensuel".into(), lire(apres, "salaire_mensuel"));
    }
    let mut lignes = Vec::new();
    for (rang, (a, b)) in lignes_de(avant).iter().zip(lignes_de(apres).iter()).enumerate() {
        let (ta, tb) = (a.get("taux").unwrap_or(&nul), b.get("taux").unwrap_or(&nul));
        if !rempli(a.get("taux")) && !vrai(tb) {
            continue; // restee sans taux : rien a ecrire
        }
        if !valeurs_egales(ta, tb) {
            lignes.push(json!({
                "rang": rang,
                "rubrique": b.get("rubrique").cloned().unwrap_or_else(|| json!("cachet")),
                "taux_avant": ta, "taux": tb,
            }));
        }
    }
    if !lignes.is_empty() {
        d.insert("lignes".into(), Value::Array(lignes));
    }
    Value::Object(d)
}

/// Le taux qu'il faudrait, pour que ces engagements coutent CE montant. N'ecrit rien.
fn ajuster(ctx: &Contexte, charge: &Value) -> R<Value> {
    let source = exiger(charge, "saisie")?;
    let engagements = exiger(charge, "engagements")?;
    let cible = texte_requis(charge, "cible")?;
    let montant = exiger(charge, "montant")?;
    let cible_visee = match cible {
        "net" => engagement::viser::Cible::Net,
        "brut" => engagement::viser::Cible::Brut,
        "cout" => engagement::viser::Cible::Cout,
        _ => return Err(refus(format!("Cible inconnue : {}. Les trois qui se visent sont brut, cout, net.", socle::fichiers::repr(cible)))),
    };
    let vise = centimes(dec(montant).map_err(|m| erreur("InvalidOperation", m))?);
    if vise <= Decimal::ZERO {
        return Err(refus(format!("Un montant vise se dit en euros et il est positif (recu : {}).", vise)));
    }
    if !source.is_object() {
        return Err(erreur("TypeError", "`saisie` : un objet est attendu"));
    }
    let data = source.clone();
    let mut rangs: Vec<i64> = Vec::new();
    for i in envois::etats::items(engagements)? {
        rangs.push(crate::routes::fiches::entier(&i)?);
    }
    rangs.sort_unstable();
    rangs.dedup();
    let total = data.get("engagements").and_then(Value::as_array).map_or(0, Vec::len);
    if rangs.is_empty() || rangs[0] < 0 || rangs[rangs.len() - 1] >= total as i64 {
        let noms: Vec<String> = rangs.iter().map(i64::to_string).collect();
        return Err(refus(format!(
            "Engagement(s) {} : la saisie en porte {}.",
            if noms.is_empty() { "aucun".to_string() } else { noms.join(", ") },
            total
        )));
    }
    let rangs: Vec<usize> = rangs.into_iter().map(|i| i as usize).collect();
    let v = engagement::viser::viser(ctx, &data, &rangs, cible_visee, vise).map_err(|r| match r {
        engagement::viser::RefusVisee::Refus(m) => refus(m),
        engagement::viser::RefusVisee::Calcul(m) => refus_du_calcul(ctx, &data, m),
    })?;
    // Les paies de ces engagements, telles que la page les montre.
    let mut vues = Vec::new();
    for &i in &rangs {
        vues.push(vue_paie(ctx, &v.paies[i], &v.saisie["engagements"][i], &v.saisie, false).map_err(Echec::from)?);
    }
    let atteint = centimes(v.atteint);
    let mut f = v.facteur.round_dp(6);
    f.rescale(6);
    Ok(json!({
        "cible": cible,
        "vise": vise.to_string(), "atteint": atteint.to_string(), "depart": centimes(v.depart).to_string(),
        "ecart": (atteint - vise).to_string(),
        "facteur": f.to_string(), "tours": v.tours,
        "engagements": rangs.iter().map(|&i| ce_qui_a_change(&data["engagements"][i], &v.saisie["engagements"][i], i)).collect::<Vec<_>>(),
        "paies": vues,
    }))
}

// ============================================================== l'apercu

/// Le refus du calcul d'une saisie : le moteur refuse avec sa phrase, mais
/// quand la saisie elle-meme est encore incomplete (un spectacle pas encore
/// choisi, des jours ou des heures pas encore saisis :
/// `engagement::saisie_incomplete`) c'est cette phrase-la, sous son nom
/// (`SaisieIncomplete`), qui monte. La page les
/// dit sobrement ; l'ecran enregistre a chaque frappe, donc ce ne sont pas des
/// defauts du logiciel.
pub(crate) fn refus_du_calcul(ctx: &Contexte, data: &Value, message: String) -> Echec {
    if engagement::saisie_incomplete(&message) {
        return erreur("SaisieIncomplete", message);
    }
    if let Some(m) = engagement::refus_de_saisie_incomplete(ctx, data) {
        return erreur("SaisieIncomplete", m);
    }
    Echec::from(message)
}

/// Une saisie recue : un dictionnaire (l'ecran), ou un mois (`AAAA-MM`, la saisie que ses engagements montrent).
fn saisie_recue(ctx: &Contexte, charge: &Value) -> R<Value> {
    match charge.get("saisie") {
        None => Err(erreur("KeyError", socle::fichiers::absent("saisie"))),
        Some(d) if d.is_object() => Ok(d.clone()),
        // La saisie enregistree, telle que l'ecran la montre (`vue_a_l_ecran`) : ce qui manque a un engagement se
        // retrouve en le calculant.
        Some(Value::String(s)) => {
            let mut v = engagement::stock::vue_a_l_ecran(&ctx.racine, s)
                .map_err(|m| erreur("FileNotFoundError", m))?
                .ok_or_else(|| erreur("FileNotFoundError", format!("{} : aucun engagement ne touche ce mois ({}/).", s, engagement::stock::DOSSIER)))?;
            for e in v.get_mut("engagements").and_then(Value::as_array_mut).into_iter().flatten() {
                if let Some(o) = e.as_object_mut() {
                    o.remove(engagement::stock::A_COMPLETER);
                }
            }
            v["mois"] = Value::String(s.clone());
            Ok(v)
        }
        Some(autre) => Err(erreur("TypeError", format!("saisie illisible : {}", autre))),
    }
}

/// Une paie que l'ecran montre, et pourquoi elle s'est recalculee d'elle-meme (`{"le", "raison"}`), s'il y a lieu.
pub(crate) struct PaieMontree {
    pub(crate) paie: Paie,
    pub(crate) recalcul: Option<Value>,
}

/// LES PAIES QUE L'ECRAN MONTRE POUR LA SAISIE `data` : un engagement valide se lit (sa paie ecrite, `etats::Valide::paie`),
/// un engagement en saisie se calcule, sans rien ecrire. Un mois tout valide ne calcule rien.
///
/// L'ecran rend chaque engagement valide tel que le moteur le lui a montre (`valide` pose) : il retrouve le sien, sur le
/// disque, par le salarie et le premier jour, dans l'ordre.
pub(crate) fn paies_de_l_ecran(ctx: &Contexte, data: &Value, mois: &str) -> R<Vec<PaieMontree>> {
    let tous = data.get("engagements").and_then(Value::as_array).cloned().unwrap_or_default();
    let disque = engagement::stock::vue_du_mois(&ctx.racine, mois)?;
    let etats = if disque.is_some() { engagement::etats::du_mois(ctx, mois)? } else { Vec::new() };
    let mut pris = vec![false; etats.len()];
    let mut lues: Vec<Option<PaieMontree>> = Vec::new();
    for e in &tous {
        let mut lue = None;
        if engagement::etats::est_validee(e) {
            let qui = (e.get("salarie").map(en_texte), engagement::debut_brut(e));
            let trouve = etats.iter().enumerate().find(|(j, x)| {
                let b = Value::Object(x.objet().brut.clone());
                !pris[*j] && x.valide().is_some() && (b.get("salarie").map(en_texte), engagement::debut_brut(&b)) == qui
            });
            if let (Some((j, x)), Some(d)) = (trouve, disque.as_ref()) {
                let v = x.valide().ok_or("engagement en saisie")?;
                pris[j] = true;
                lue = Some(PaieMontree { paie: v.paie(ctx, d, j)?, recalcul: v.recalcul(mois).cloned() });
            }
        }
        lues.push(lue);
    }
    let calculees = if lues.iter().any(Option::is_none) {
        engagement::pour_l_ecran(ctx, data).map_err(|m| refus_du_calcul(ctx, data, m))?.paies
    } else {
        Vec::new()
    };
    let mut out = Vec::new();
    for (i, l) in lues.into_iter().enumerate() {
        match l {
            Some(l) => out.push(l),
            None => out.push(PaieMontree { paie: calculees.get(i).cloned().ok_or("le calcul du mois n'a pas cette paie")?, recalcul: None }),
        }
    }
    Ok(out)
}

/// `Decimal.quantize(Decimal("0.01"))` : deux decimales, la moitie au pair.
pub(crate) fn q2(x: Decimal) -> Decimal {
    let mut r = x.round_dp(2);
    r.rescale(2);
    r
}

/// Une ligne de la saisie, avec ce que la rubrique en dit.
fn ligne_saisie(ctx: &Contexte, l: &Value) -> Value {
    let rubrique = l.get("rubrique");
    let infos = || -> Option<(String, String, f64)> {
        let r = droit::catalogue::rubrique(&ctx.racine, l.get("rubrique")?.as_str()?, &ctx.lue_au).ok()?;
        let quantite = match l.get("quantite") {
            None | Some(Value::Null) => 0.0,
            Some(q) if !vrai(q) => 0.0,
            Some(q) => flottant(q).ok()?,
        };
        let saisies = match l.get("heures") {
            None | Some(Value::Null) => None,
            Some(Value::Number(n)) => n.as_f64(),
            Some(_) => return None,
        };
        let total = r.heures(quantite, saisies);
        Some((r.libelle, r.unite, total))
    };
    let (libelle, unite, total) = match infos() {
        Some((l, u, t)) => (Value::String(l), Value::String(u), Some(t)),
        None => (rubrique.cloned().unwrap_or(Value::Null), Value::Null, None),
    };
    let jours = |cle: &str| -> Value {
        Value::Array(l.get(cle).and_then(Value::as_array).map_or_else(Vec::new, |a| a.iter().map(brut_valeur).collect()))
    };
    json!({
        "rubrique": rubrique.cloned().unwrap_or(Value::Null),
        "libelle": libelle, "unite": unite,
        "quantite": l.get("quantite").cloned().unwrap_or(Value::Null),
        "taux": l.get("taux").cloned().unwrap_or(Value::Null),
        "heures_par_unite": l.get("heures").cloned().unwrap_or(Value::Null),
        "heures": total.filter(|t| *t != 0.0).map_or(Value::Null, nombre_f),
        "lieu": match l.get("lieu") { Some(x) if vrai(x) => x.clone(), _ => Value::String(String::new()) },
        "dates": jours("dates"),
        "planning": jours("planning"),
    })
}

/// LES CASES DU CONTRAT ECRIT QUE CET ENGAGEMENT APPELLE (`engagement::CHAMPS_DU_CONTRAT_ECRIT`), et elles seules : la
/// periode d'essai et les clauses particulieres de tout contrat ecrit ; la personne remplacee d'un CDD de remplacement
/// (L1242-12, 1° ; `Nature::motif_de_remplacement`) ; la repartition des horaires d'un temps partiel (L3123-6, 1° : la
/// modalite 20 de S21.G00.40.014, `Engagement::modalite_temps_travail`). Un engagement dont le contrat ne se fait pas
/// signer ici (`engagement::contrat_a_signer`) n'en a aucune.
fn champs_du_contrat_ecrit(ctx: &Contexte, e: &engagement::Engagement, nat: &droit::natures::Nature) -> R<Vec<&'static str>> {
    if !engagement::contrat_a_signer(&ctx.racine, e)? {
        return Ok(Vec::new());
    }
    let mut v = vec!["periode_essai_jours"];
    if e.motif_recours.as_deref().is_some_and(|m| nat.motif_de_remplacement(m)) {
        v.push("personne_remplacee");
    }
    if nat.mensuelle() && e.modalite_temps_travail()? == "20" {
        v.push("repartition_horaire");
    }
    v.push("clauses_particulieres");
    Ok(v)
}

/// Une paie pour l'ecran. `brut` : l'engagement tel qu'il
/// est ecrit dans la saisie ; `mois` : le mois du fichier (pour `lieu` et
/// `taux_cachet`, gardes tels qu'ecrits).
pub(crate) fn vue_paie(ctx: &Contexte, p: &Paie, brut: &Value, data: &Value, avec_lignes: bool) -> R<Value> {
    let e = &p.engagement;
    let nat = e.nat()?;
    let iso = |j: &Jour| Value::String(j.iso());
    let s = &p.salarie;
    let requis = |v: &Value, k: &str| v.get(k).cloned().ok_or_else(|| erreur("KeyError", socle::fichiers::absent(k)));
    let sal_vue = json!({
        "cle": s.get("_cle").cloned().unwrap_or(Value::Null),
        "nom": requis(s, "nom")?,
        "prenom": match s.get("prenom_affichage") { Some(x) if vrai(x) => x.clone(), _ => requis(s, "prenom")? },
    });
    let spectacle = if vrai(&p.spectacle) {
        json!({ "titre": requis(&p.spectacle, "titre")?, "numero_objet": p.spectacle.get("numero_objet").cloned().unwrap_or(Value::Null) })
    } else {
        Value::Null
    };
    let mois_du_contrat = e.mois_du_contrat()?;
    // Le taux du cachet, tel que le contrat l'imprime : celui de la premiere
    // ligne a l'unite << cachet >> ; sans cachet, la valeur ecrite, telle quelle,
    // et rien quand rien n'est ecrit (regle 1 : plus de 130 EUR).
    let premier_cachet = e
        .lignes
        .iter()
        .find(|l| l.get("rubrique").and_then(Value::as_str).and_then(|c| droit::catalogue::rubrique(&ctx.racine, c, &ctx.lue_au).ok()).map_or(false, |r| r.unite == "cachet"));
    let taux_cachet = match premier_cachet {
        Some(l) => nombre_f(flottant(l.get("taux").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("taux")))?)?),
        None => brut.get("taux_cachet").or_else(|| data.get("taux_cachet")).cloned().unwrap_or(Value::Null),
    };
    let jours = match brut.get("jours") {
        Some(j) if !j.is_null() => j.clone(),
        _ => Value::from(e.jours),
    };
    let lieu = brut.get("lieu").or_else(|| data.get("lieu")).cloned().unwrap_or_else(|| Value::String(String::new()));
    let avis: Vec<&Avis> = p.avertissements.iter().collect();
    let mut d = json!({
        "numero_bulletin": p.numero_bulletin.clone().map_or(Value::Null, Value::String),
        "numero_contrat": e.numero.clone().map_or(Value::Null, Value::String),
        "salarie": sal_vue,
        // UN PERMANENT N'A PAS DE SPECTACLE, et `None` est la reponse juste.
        "spectacle": spectacle,
        "nature": { "cle": nat.cle, "libelle": nat.libelle, "mensuelle": nat.mensuelle() },
        "debut": iso(&e.debut), "fin": e.fin.as_ref().map_or(Value::Null, iso),
        // Le contrat par rapport au mois : un contrat qui deborde n'a pas de
        // DPAE ni de contrat imprime hors de son premier mois.
        "contrat": {
            "premier_mois": e.premier_mois()?, "dernier_mois": e.dernier_mois()?, "a_cheval": e.a_cheval()?,
            "sans_paie": e.sans_paie(),
            "depuis": mois_du_contrat.first().cloned().ok_or_else(|| socle::fichiers::trop_courte("les mois du contrat"))?,
            "jusqu_a": mois_du_contrat.last().cloned().ok_or_else(|| socle::fichiers::trop_courte("les mois du contrat"))?,
            "periode": { "debut": iso(&e.debut_periode()?), "fin": iso(&e.fin_periode()?) },
            "ecrits": champs_du_contrat_ecrit(ctx, e, &nat)?,
        },
        "dates_travail": e.dates_travail.iter().map(iso).collect::<Vec<_>>(),
        "cachets": e.cachets, "taux_cachet": taux_cachet,
        "heures": e.heures.map_or(Value::Null, nombre_f),
        "remuneration": p.remuneration.iter().map(|l| json!({
            "rubrique": l.code, "designation": l.designation(),
            "quantite": l.quantite.to_string(), "taux": l.taux.to_string(), "montant": q2(l.montant).to_string(),
        })).collect::<Vec<_>>(),
        "lignes_saisies": e.lignes.iter().map(|l| ligne_saisie(ctx, l)).collect::<Vec<_>>(),
        "jours": jours, "lieu": lieu,
        "brut": q2(p.brut).to_string(), "base_csg": q2(p.base_csg).to_string(),
        "plafond": q2(p.plafond).to_string(), "smic": nombre_f(p.smic),
        "total_patronal": q2(p.total_pat).to_string(), "total_salarial": q2(p.total_sal).to_string(),
        "net_social": q2(p.net_social).to_string(), "net_a_payer": q2(p.net_a_payer).to_string(),
        "net_imposable": q2(p.net_imposable).to_string(), "cout_employeur": q2(p.cout_employeur).to_string(),
        "avertissements": avis.iter().filter(|a| !a.info).map(|a| a.texte.clone()).collect::<Vec<_>>(),
        "informations": avis.iter().filter(|a| a.info).map(|a| a.texte.clone()).collect::<Vec<_>>(),
        "emploi": {
            "libelle": s.get("emploi").cloned().unwrap_or(Value::Null),
            "code_emploi_ft": s.get("code_emploi_ft").cloned().unwrap_or(Value::Null),
            "statut": s.get("statut").cloned().unwrap_or(Value::Null),
            "cadre": s.get("statut_categoriel_rc").and_then(Value::as_str) == Some("01"),
        },
    });
    if avec_lignes {
        let taux = |t: Option<f64>| t.map_or(Value::Null, nombre_f);
        d["lignes"] = Value::Array(
            p.lignes
                .iter()
                .map(|l| {
                    json!({
                        "id": l.id, "famille": l.famille, "libelle": l.libelle, "base": q2(l.base).to_string(),
                        "taux_pat": taux(l.taux_pat), "montant_pat": q2(l.montant_pat).to_string(),
                        "taux_sal": taux(l.taux_sal), "montant_sal": q2(l.montant_sal).to_string(),
                        "masquer_base_et_taux": l.masquer_base_et_taux,
                    })
                })
                .collect(),
        );
    }
    Ok(d)
}

/// Ce qui, dans ce mois, porte sur une
/// fiche archivee. On n'INTERDIT pas : rejouer une paie passee, reimprimer un
/// bulletin perdu, cela doit marcher. Mais une paie NEUVE sur une fiche mise de
/// cote est presque toujours une erreur, et elle ne se verrait nulle part ailleurs.
pub(crate) fn archives_employees(paies: &[Paie]) -> R<Vec<String>> {
    let mut out = Vec::new();
    for p in paies {
        if archivee(&p.salarie) {
            let nom = p.salarie.get("nom").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("nom")))?;
            let prenom = match p.salarie.get("prenom_affichage") {
                Some(x) if vrai(x) => x.clone(),
                _ => p.salarie.get("prenom").cloned().ok_or_else(|| erreur("KeyError", socle::fichiers::absent("prenom")))?,
            };
            out.push(format!(
                "{} {} : sa fiche est archivee. Pour refaire une paie passee, rien a faire ; pour une nouvelle embauche, la \
                 reactiver (Configuration › Salaries, « Reactiver »).",
                en_texte(nom),
                en_texte(&prenom)
            ));
        }
        // Un permanent n'a pas de spectacle -- le numero d'objet lui est interdit
        // (CCH-11) -- donc rien a verifier de ce cote.
        if vrai(&p.spectacle) && archivee(&p.spectacle) {
            let titre = p.spectacle.get("titre").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("titre")))?;
            out.push(format!(
                "Le spectacle « {} » (objet {}) est archive : pour le jouer a nouveau, le reactiver (Configuration › Spectacles).",
                en_texte(titre),
                texte(p.spectacle.get("numero_objet"))
            ));
        }
    }
    Ok(out)
}

/// Le bordereau Urssaf que l'apercu montre (celui du PRINCIPAL quand la compagnie a plusieurs etablissements ;
/// chacun des autres a le sien, dans sa DSN), et l'avertissement quand il ne se calcule pas. Partage avec `produire`.
pub(crate) fn urssaf_de_l_apercu(ctx: &Contexte, paies: &[Paie], mois: &str) -> R<(Value, Vec<String>)> {
    // Le bordereau montre est celui du PRINCIPAL quand la compagnie a plusieurs
    // etablissements ; chacun des autres a le sien, dans sa DSN.
    let principal = droit::bareme::nic_principal(&ctx.entreprise);
    let plusieurs = !droit::bareme::secondaires(&ctx.entreprise).is_empty();
    let mut paies_p: Vec<Paie> = Vec::new();
    for p in paies {
        if !plusieurs || droit::bareme::etablissement_du_salarie(&ctx.entreprise, &p.salarie).map_err(|m| erreur("ValueError", m))? == principal {
            paies_p.push(p.clone());
        }
    }
    let mut avert: Vec<String> = Vec::new();
    // L'apercu n'a pas a refuser : c'est la production qui le fera.
    let urssaf = match declarations::dsn::montant_urssaf(ctx, &paies_p, mois, None) {
        Ok(m) => Value::String(m.to_string()),
        Err(e) => {
            avert.push(format!("Bordereau Urssaf non calculable : {}", e));
            Value::Null
        }
    };

    Ok((urssaf, avert))
}

fn annee_de(mois: &str) -> R<i32> {
    Ok(mois.get(..4).and_then(|a| a.parse().ok()).ok_or_else(|| format!("mois illisible : {}", socle::fichiers::repr(mois)))?)
}

/// `apercu` : le mois tel que l'ecran le montre, rien d'ecrit -- ni fichier, ni compteur. Charge : `saisie` (le mois tel
/// que l'ecran le tient), `lignes` (vrai : les lignes de cotisation de chaque paie, pour le bulletin affiche), `urssaf`
/// (vrai : le bordereau Urssaf du mois, que la boite « Fermer le mois » montre ; il ne se calcule que la).
///
/// Un engagement valide montre la paie qu'il a ecrite, avec ses numeros du moment ; s'il s'est recalcule de lui-meme,
/// ses informations disent quand et pourquoi (`etats::phrase_du_recalcul`). Un engagement en saisie se calcule, et
/// porte ses conseils de saisie.
/// L'APERCU D'UNE SAISIE DONT DES ENGAGEMENTS NE SE CALCULENT PAS ENCORE (`manques_de_la_saisie`) : les autres se
/// calculent, et chacun de ceux-la tient sa place dans `paies` avec ce qui lui manque (`a_completer`), sans montant ;
/// la carte qui en porte un n'a pas de total. Une saisie dont aucun refus ne se rapporte a un engagement refuse.
fn apercu(ctx: &Contexte, charge: &Value) -> R<Value> {
    let data = saisie_recue(ctx, charge)?;
    let refus = match apercu_de(ctx, charge, &data) {
        Ok(v) => return Ok(v),
        Err(refus) => refus,
    };
    let manques = manques_de_la_saisie(ctx, &data);
    if manques.iter().all(Option::is_none) {
        return Err(refus);
    }
    let tous = data.get("engagements").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut complete = data.clone();
    complete["engagements"] = Value::Array(tous.iter().zip(&manques).filter(|(_, m)| m.is_none()).map(|(e, _)| e.clone()).collect());
    let mut sans_blocs = charge.clone();
    if let Some(c) = sans_blocs.as_object_mut() {
        c.remove("blocs");
    }
    let mut r = apercu_de(ctx, &sans_blocs, &complete)?;
    let mut calculees = r["paies"].as_array().cloned().unwrap_or_default().into_iter();
    let paies: Vec<Value> = tous
        .iter()
        .zip(&manques)
        .map(|(e, m)| match m {
            None => calculees.next().unwrap_or(Value::Null),
            Some(m) => json!({
                "a_completer": m, "salarie": {"_cle": e.get("salarie").cloned().unwrap_or(Value::Null)},
                "avertissements": [], "informations": [],
            }),
        })
        .collect();
    let tailles: Vec<usize> = match charge.get("blocs").and_then(Value::as_array) {
        Some(a) => a.iter().map(|x| x.as_u64().unwrap_or(0) as usize).collect(),
        None => vec![1; paies.len()],
    };
    let mut blocs = Vec::new();
    let (mut rang, mut rang_calcule) = (0, 0);
    let blocs_calcules = r["blocs"].as_array().cloned().unwrap_or_default();
    for n in tailles {
        let fin = (rang + n).min(paies.len());
        let part = &manques[rang..fin];
        if part.iter().any(Option::is_some) || part.is_empty() {
            blocs.push(Value::Null);
        } else {
            // Une carte toute calculee : la somme de ses engagements, calcules un par un.
            let vs = &blocs_calcules[rang_calcule..(rang_calcule + part.len()).min(blocs_calcules.len())];
            let somme = |cle: &str| -> R<String> {
                let mut t = Decimal::new(0, 2);
                for v in vs {
                    t += dec(v.get(cle).ok_or_else(|| socle::fichiers::absent(cle))?)?;
                }
                Ok(q2(t).to_string())
            };
            blocs.push(json!({"net_a_payer": somme("net_a_payer")?, "brut": somme("brut")?, "cout_employeur": somme("cout_employeur")?, "engagements": part.len()}));
        }
        rang_calcule += part.iter().filter(|m| m.is_none()).count();
        rang = fin;
    }
    r["paies"] = Value::Array(paies);
    r["blocs"] = Value::Array(blocs);
    Ok(r)
}

/// L'apercu de la saisie `data` : chaque engagement se calcule.
fn apercu_de(ctx: &Contexte, charge: &Value, data: &Value) -> R<Value> {
    let data = data.clone();
    let avec_lignes = vrai_opt(charge.get("lignes"));
    let avec_urssaf = vrai_opt(charge.get("urssaf"));
    let mois = data.get("mois").map(en_texte).ok_or_else(|| socle::fichiers::absent("mois"))?;
    let annee = annee_de(&mois)?;
    let montrees = paies_de_l_ecran(ctx, &data, &mois)?;
    let mut paies: Vec<Paie> = montrees.iter().map(|m| m.paie.clone()).collect();
    documents::ecrire_documents::numeroter(ctx, &mut paies, annee)?;
    let tous = data.get("engagements").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut vues = Vec::new();
    for (i, p) in paies.iter().enumerate() {
        vues.push(vue_paie(ctx, p, tous.get(i).unwrap_or(&Value::Null), &data, avec_lignes)?);
    }
    // LES CONSEILS DE SAISIE : au plus un par carte, quand l'option de la compagnie est cochee, pour un
    // engagement en saisie seulement. Ils ne vivent qu'ici, dans la vue de l'ecran : ni dans les avertissements de la
    // paie, ni au bulletin, ni a `produire`.
    for (i, (v, c)) in vues.iter_mut().zip(crate::compagnie::conseils::de_saisie(ctx, &paies)?).enumerate() {
        if let Some(c) = c.filter(|_| !tous.get(i).is_some_and(engagement::etats::est_validee)) {
            v["conseil"] = c;
        }
    }
    for (v, m) in vues.iter_mut().zip(&montrees) {
        if let Some(phrase) = m.recalcul.as_ref().and_then(engagement::etats::phrase_du_recalcul) {
            if let Some(Value::Array(a)) = v.get_mut("informations") {
                a.insert(0, Value::String(phrase));
            }
        }
    }
    let (urssaf, avert) = if avec_urssaf { urssaf_de_l_apercu(&ctx, &paies, &mois)? } else { (Value::Null, Vec::new()) };
    let somme = |cle: &str| -> R<Decimal> {
        let mut t = Decimal::new(0, 2);
        for v in &vues {
            t += dec(v.get(cle).ok_or_else(|| socle::fichiers::absent(cle))?)?;
        }
        Ok(q2(t))
    };
    let (total_brut, total_cout, total_net) = (somme("brut")?, somme("cout_employeur")?, somme("net_a_payer")?);
    // LES TROIS NOMBRES, sommes ici : par bloc de l'ecran (`blocs` : combien d'engagements chaque carte porte, dans
    // l'ordre ; sans lui, une carte par engagement) et par salarie (la colonne « Le mois »).
    let trois = |vs: &[&Value]| -> R<Value> {
        let mut t = [Decimal::new(0, 2), Decimal::new(0, 2), Decimal::new(0, 2)];
        for v in vs {
            for (k, cle) in ["net_a_payer", "brut", "cout_employeur"].iter().enumerate() {
                t[k] += dec(v.get(*cle).ok_or_else(|| socle::fichiers::absent(cle))?)?;
            }
        }
        Ok(json!({"net_a_payer": q2(t[0]).to_string(), "brut": q2(t[1]).to_string(), "cout_employeur": q2(t[2]).to_string(), "engagements": vs.len()}))
    };
    let tailles: Vec<usize> = match charge.get("blocs").and_then(Value::as_array) {
        Some(a) => a.iter().map(|x| x.as_u64().unwrap_or(0) as usize).collect(),
        None => vec![1; vues.len()],
    };
    let mut blocs = Vec::new();
    let mut rang = 0;
    for n in tailles {
        let fin = (rang + n).min(vues.len());
        blocs.push(trois(&vues[rang..fin].iter().collect::<Vec<_>>())?);
        rang = fin;
    }
    let mut salaries: Vec<(String, Value, String, Vec<&Value>)> = Vec::new();
    for (v, p) in vues.iter().zip(&paies) {
        let s = v.get("salarie").cloned().unwrap_or(Value::Null);
        let cle = format!("{}|{}", en_texte(s.get("nom").unwrap_or(&Value::Null)), en_texte(s.get("prenom").unwrap_or(&Value::Null)));
        match salaries.iter_mut().find(|(c, _, _, _)| *c == cle) {
            Some((_, _, _, vs)) => vs.push(v),
            None => salaries.push((cle, s, super::nom_a_l_ecran(&p.salarie), vec![v])),
        }
    }
    let mut par_salarie = Vec::new();
    for (_, s, nom, vs) in &salaries {
        let mut t = trois(vs)?;
        t["salarie"] = s.clone();
        t["nom"] = json!(nom);
        par_salarie.push(t);
    }
    let mut a_regarder = avert;
    a_regarder.extend(archives_employees(&paies)?);
    let mut informations = Vec::new();
    for v in &vues {
        let textes = |k: &str| v.get(k).and_then(Value::as_array).cloned().unwrap_or_default();
        a_regarder.extend(textes("avertissements").iter().map(en_texte));
        informations.extend(textes("informations").iter().map(en_texte));
    }
    let n: usize = mois[5..7].parse().map_err(|_| format!("mois illisible : {}", socle::fichiers::repr(&mois)))?;
    Ok(json!({
        "mois": mois,
        "libelle": format!("{} {}", socle::date::nom_du_mois(n as i64, socle::date::Graphie::Titre)?, annee),
        "paies": vues,
        "total_brut": total_brut.to_string(), "total_cout": total_cout.to_string(), "total_net": total_net.to_string(),
        "blocs": blocs, "par_salarie": par_salarie,
        "urssaf": urssaf,
        "avertissements": a_regarder,
        "informations": informations,
    }))
}
