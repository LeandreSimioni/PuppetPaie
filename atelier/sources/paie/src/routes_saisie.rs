//! Les routes `saisie` et `echeancier` de l'interface.
//!
//! `saisie` : le mois tel que l'ecran l'ouvre -- la saisie que ses engagements montrent (`engagement::stock::vue_du_mois`)
//! ou, pour un mois neuf, un squelette vide au dernier taux saisi --, avec les noms resolus en cles de fiche et les lignes
//! de paie telles que le moteur les comprend. Aucun calcul de paie pour l'ecran : la saisie se charge jusqu'au contrat qui
//! deborde, sans chiffrer. Les paies validees que l'ecran montre se lisent par
//! `engagement::etats::du_mois`, qui les recalcule si elles ne sont plus justes.
//!
//! `echeancier` : la date, ce qui est a faire ce mois-ci, et l'annee qui vient. Les echeances annuelles sont dans
//! `donnees/echeancier-base.json`. Les etats du journal se lisent directement (`etats.rs`).

use std::collections::HashMap;
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::amorcage;
use engagement::calcul::lignes_brut;
use droit::contexte::Contexte;
use socle::date::{en_texte, jours_du_mois, Jour};
use engagement::{self, Engagement};
use crate::etats;
use socle::valeur::nom_type;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::{dossier_du_code, vrai};
use socle::echec::Echec;
use crate::routes_ouverture::{self as ouverture, Etat};

/// Les routes que ce module sert.
pub const ROUTES: [&str; 2] = ["saisie", "echeancier"];

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "saisie" => saisie(_o, ctx, charge),
        "echeancier" => echeancier(ctx, charge),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

// ------------------------------------------------------------------ outils

fn en_echec(r: ouverture::Refus) -> Echec {
    Echec { classe: r.classe, message: r.message }
}

fn vrai_opt(v: Option<&Value>) -> bool {
    v.map_or(false, vrai)
}

fn nombre_f(x: f64) -> Value {
    serde_json::Number::from_f64(x).map_or(Value::Null, Value::Number)
}

/// `int(texte)` de Python, a quelques ecarts de forme pres.
fn entier(t: &str) -> R<i32> {
    t.trim().parse::<i32>().map_err(|_| erreur("ValueError", format!("nombre entier illisible : {}", socle::fichiers::repr(t))))
}

/// Les `n` premiers caracteres (`mois[:n]`) et ceux de `a` a `b` (`mois[a:b]`).
fn tranche(t: &str, a: usize, b: usize) -> String {
    t.chars().skip(a).take(b - a).collect()
}

/// `operations_paies.lire_saisie` : le fichier, avec la classe d'exception que
/// Python levait pour un fichier illisible.
pub(crate) fn lire_saisie(chemin: &Path) -> R<Value> {
    let octets = std::fs::read(chemin).map_err(|e| erreur("OSError", format!("{} : {}", chemin.display(), e)))?;
    let texte = String::from_utf8(octets).map_err(|e| erreur("UnicodeDecodeError", format!("{} : {}", chemin.display(), e)))?;
    serde_json::from_str(&texte).map_err(|e| erreur("JSONDecodeError", e.to_string()))
}

// ============================================================ la saisie

/// `operations_socle.mois_hors_delai` : la date limite de depot depassee (`echeance_dsn`, reportee), ou `None`.
pub(crate) fn mois_hors_delai(ctx: &Contexte, mois: &str) -> R<Option<Jour>> {
    let limite = crate::echeance_dsn::de_la_compagnie(ctx, mois).map_err(|m| erreur("ValueError", m))?.jour;
    Ok(if ctx.aujourdhui > limite { Some(limite) } else { None })
}

/// UN MOIS D'AVANT LA PREMIERE EMBAUCHE N'A PAS DE DSN DUE (#236, banc du Petit Castelet : ajoutee le 06/01/2025, premiere
/// embauche le 25/01, la compagnie voyait decembre 2024 « Rouvrir ce mois (DSN en retard) — sa DSN était due le
/// 15/01/2025 »). La DSN est due par l'employeur pour les salaries qu'il emploie (`routes_ouverture::premier_mois_declarable`,
/// la borne que l'echeancier et les rappels lisent deja) : un mois passe, sans saisie, ni tenu avant ce logiciel, qui
/// precede le mois de la premiere embauche (`premiere_embauche` de la fiche Entreprise) n'est pas en retard, il n'a rien
/// a declarer. Rend la premiere embauche (`AAAA-MM-JJ`), ou `None`.
pub(crate) fn avant_la_premiere_embauche(ctx: &Contexte, mois: &str) -> R<Option<String>> {
    let embauche = ctx.entreprise.get("premiere_embauche").and_then(Value::as_str).unwrap_or("").trim().to_string();
    if embauche.len() < 7 || mois >= &embauche[..7] || engagement::stock::a_une_saisie(&ctx.racine, mois).map_err(Echec::from)? {
        return Ok(None);
    }
    if crate::mois_importe::importe(&ctx.racine, mois)? || mois_hors_delai(ctx, mois)?.is_none() {
        return Ok(None);
    }
    Ok(Some(embauche))
}

/// `operations_paies.squelette(mois, ctx)` : un mois neuf s'ouvre VIDE, avec le
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
fn flottant_py(v: &Value) -> R<f64> {
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

/// Les elements qu'une boucle Python verrait dans `valeur or []` : une liste, ou
/// ce que `reversed` accepte d'autre (une chaine, un dictionnaire : leurs
/// cles, des textes sans `.get`).
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
            return flottant_py(&objet["taux_cachet"]).map(Some);
        }
    }
    Ok(None)
}

/// Le taux d'une ligne de cachet (le `try` de `dernier_taux`) : tout ce qui
/// leve est une ligne qui ne dit rien.
fn taux_d_une_ligne(ctx: &Contexte, l: &Value) -> Option<f64> {
    let code = l.get("rubrique")?.as_str()?;
    if droit::catalogue::rubrique(&ctx.racine, code).ok()?.unite != "cachet" {
        return None;
    }
    let taux = l.get("taux").filter(|t| vrai(t))?;
    flottant_py(taux).ok()
}

/// La saisie chargee SANS calculer : `engagement::charger_donnees` jusqu'au
/// contrat qui deborde, plus les lignes de remuneration de chaque engagement
/// paye au cachet (elles ne dependent que de l'engagement). `None` quand le
/// Python aurait refuse la saisie (la page la montre alors telle quelle).
fn charger_sans_calcul(ctx: &Contexte, data: &Value) -> Option<Vec<engagement::calcul::Paie>> {
    let _derivation = engagement::conges::Derivation::ouvrir();
    let (engs, _) = engagement::engagements(ctx, data, &engagement::conges::lignes_du_mois).ok()?;
    let mut paies = Vec::new();
    for e in &engs {
        paies.push(amorcage::embauche(ctx, e).ok()?);
    }
    for p in paies.iter_mut() {
        engagement::suivre_contrat(ctx, &mut p.engagement).ok()?;
        if !p.engagement.nat().ok()?.mensuelle() {
            p.remuneration = lignes_brut(&p.engagement, ctx).ok()?;
        }
    }
    Some(paies)
}

/// `float(Decimal)` : la valeur la plus proche, par sa representation.
fn decimal_en_f64(d: socle::num::Decimal) -> f64 {
    d.to_string().parse().unwrap_or(0.0)
}

/// `web._resoudre_noms` : « Celi » et « aaah zerbinette » deviennent les cles
/// du fichier. Une saisie qui ne se charge pas se rend telle quelle.
fn resoudre_noms(ctx: &Contexte, saisie: &Value) -> R<Value> {
    let Some(objet) = saisie.as_object() else { return Ok(saisie.clone()) };
    let Some(paies) = charger_sans_calcul(ctx, saisie) else { return Ok(saisie.clone()) };
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
/// les relit pour les remettre dans ses champs, et les reecrit ensuite (#138).
/// `base` et `taux_majoration` d'une ligne en pourcentage, surtout : sans eux,
/// la page lirait le `taux` (0.1) comme un pourcentage (0,1 %).
const CHAMPS_DE_LIGNE_GARDES: [&str; 4] = ["heures", "genre", "base", "taux_majoration"];

/// `web._lignes_affichees` : les lignes de paie telles que le MOTEUR les a comprises.
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
fn liste_py(v: &Value) -> R<Vec<Value>> {
    match v {
        Value::Array(a) => Ok(a.clone()),
        Value::String(s) => Ok(s.chars().map(|c| Value::String(c.to_string())).collect()),
        Value::Object(o) => Ok(o.keys().map(|k| Value::String(k.clone())).collect()),
        autre => Err(erreur("TypeError", socle::fichiers::attendu("une liste", nom_type(autre)))),
    }
}

/// `web._jours_sur_les_lignes` : pose sur chaque ligne les jours qu'elle couvre,
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
            l.insert("dates".into(), Value::Array(liste_py(&src["dates"])?));
            porte = true;
        }
        if vrai_opt(src.get("planning")) {
            l.insert("planning".into(), Value::Array(liste_py(&src["planning"])?));
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
            .map_or(false, |c| droit::catalogue::rubrique(&ctx.racine, c).map_or(false, |r| r.unite == "cachet"))
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

/// Ce que la page propose quand la saisie ne le dit pas (#140, regle 2 : la page ne le decide pas) :
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
        objet.get("taux_cachet").filter(|t| vrai(t)).and_then(|t| flottant_py(t).ok())
    };
    // Un dossier qu'on ne sait pas relire ne propose rien (`null`) : la route rend la saisie telle quelle, et
    // c'est l'apercu qui dira pourquoi.
    let taux = du_mois().or_else(|| dernier_taux(ctx).ok().flatten()).map_or(Value::Null, nombre_f);
    let date_paiement = fin_du_mois(mois).map_or(Value::Null, Value::String);
    json!({"taux_cachet": taux, "date_paiement": date_paiement})
}

/// LE SPECTACLE ET LE LIEU DU MOIS DESCENDENT SUR CHAQUE ENGAGEMENT (16/10/2026). Une saisie peut les ecrire au niveau
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
fn saisie(o: &HashMap<String, String>, ctx: &Contexte, charge: &Value) -> R<Value> {
    let brut = charge.get("mois").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("mois")))?;
    let mois = en_texte(brut);
    let existe = engagement::stock::a_une_saisie(&ctx.racine, &mois).map_err(Echec::from)?;
    // Un mois neuf s'ouvre VIDE, avec le dernier taux saisi ; un mois que des engagements touchent prend du squelette ce
    // qu'il ne dit pas de lui.
    let mut saisie = squelette(ctx, &mois)?;
    if let Some(Value::Object(vue)) = engagement::stock::vue_du_mois(&ctx.racine, &mois).map_err(Echec::from)? {
        for (k, v) in vue {
            saisie[k] = v;
        }
    }
    // UN MOIS DECLARE DONT L'ANNULE-ET-REMPLACE EST PASSE (21/10/2026) n'est pas ferme : sa paie se modifie, en le
    // confirmant a la premiere modification (`routes_regularisation::modification_de_la_paie`) -- il n'est pas en retard.
    let mut m = crate::routes_mois::Moteur::nouveau(o, ctx);
    let dsn_deposee = crate::routes_regularisation::modification_de_la_paie(&mut m, ctx, &mois)?;
    let avant_embauche = avant_la_premiere_embauche(ctx, &mois)?;
    let saisie = descendre_le_mois(saisie);
    let defauts = defauts_de_la_saisie(ctx, &mois, &saisie);
    // UN MOIS TENU AVANT CE LOGICIEL (ticket #215) : pas de fichier ici, mais l'historique importe le porte -- ses paies,
    // en cartes, et l'etat vrai de sa DSN (`mois_importe`). Il se lit ; rien ne s'y ajoute.
    let importe = if existe { None } else { crate::mois_importe::vue(ctx, &mois)? };
    Ok(json!({
        "mois": brut,
        "existe": existe,
        "importe": importe,
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

/// Une echeance (`operations_echeancier.Echeance`).
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
    /// DITE PAR UN RAPPEL DE L'OUVERTURE (ticket #244) : la DSN d'un mois que la route `rappels` reclame
    /// (`routes_ouverture::mois_a_declarer`, la meme lecture), avec son geste (« Verifier et envoyer »). La boite de
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
        })
    }
}

/// `modele.format(**valeurs)` : `{nom}` remplace, `{{` et `}}` s'ecrivent `{` et `}`.
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
                        None => return Err(erreur("ValueError", "expected '}' before end of string")),
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
    let echeance = crate::echeance_dsn::du_mois(mois, jour_dsn).map_err(|m| erreur("ValueError", m))?;
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

/// `operations_fiches.taux_atmp(ctx, annee)` est-il pose ?
fn atmp_pose(ctx: &Contexte, annee: i32) -> R<bool> {
    let p = ctx.racine.join("config").join("baremes").join(format!("{}.json", annee));
    if !p.exists() {
        return Ok(false);
    }
    let v = match lire_saisie(&p) {
        Ok(v) => v,
        Err(e) if matches!(e.classe, Some("JSONDecodeError") | Some("UnicodeDecodeError")) => return Ok(false),
        Err(e) => return Err(e),
    };
    let Some(o) = v.as_object() else { return Err(pas_de_get(&v)) };
    Ok(match o.get("taux_atmp") {
        Some(Value::Object(t)) => t.get("valeur").map_or(false, |x| !x.is_null()),
        _ => false,
    })
}

/// Vrai, faux, ou `None` quand on ne peut pas le savoir.
fn fait_annuel(ctx: &Contexte, e: &Value, annee: i32, faites: &Map<String, Value>) -> R<Option<bool>> {
    let comment = e.get("fait_par").and_then(Value::as_str);
    let cle = e.get("cle").and_then(Value::as_str).unwrap_or("");
    if comment == Some("exercice_clos") {
        let clos = ouverture::exercice_clos(ctx, &(annee - 1).to_string()).map_err(en_echec)?;
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
    if comment == Some("exercice_clos") && ouverture::mois_de_l_annee(ctx, &(annee - 1).to_string()).is_empty() {
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
    let jour_dsn = crate::echeance_dsn::du_mois(&format!("{:04}-12", annee - 1), jour_dsn).map_err(|m| erreur("ValueError", m))?.jour.jour;
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
    let st = Etat { ctx, aujourdhui, journal: etats::charger(&ctx.racine)?, principal: ouverture::nic_principal(ctx) };
    let modele = base.get("dsn").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("dsn")))?;

    let mut toutes: Vec<Echeance> = Vec::new();
    // Les DSN : celle qui tombe ce mois-ci (celle du mois d'avant), l'annee qui
    // vient, et les mois plus anciens QUE LES RAPPELS DISENT MANQUANTS -- la
    // meme lecture : on ne reclame pas a une compagnie qui commence les DSN
    // d'avant sa premiere embauche.
    let precedent = mois_plus(&courant, -1);
    let premier = ouverture::premier_mois_declarable(&st);
    let reclames = ouverture::mois_a_declarer(&st).map_err(en_echec)?;
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

    // UNE DSN SE DIT UNE FOIS (ticket #244) : celle qu'un rappel reclame, avec son geste, ne se repete pas dans « Ce mois-ci ».
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
