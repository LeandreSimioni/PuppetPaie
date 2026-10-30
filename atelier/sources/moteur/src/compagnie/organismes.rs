//! L'ECRAN DES ORGANISMES ET DES CONTRATS DE LA DSN (Configuration › Organismes et contrats) : ce que la compagnie y
//! saisit, et seulement cela, s'ecrit dans `config/organismes.json`.
//!
//! Le reste se compose a la date de chaque periode (`droit::organismes`, `Contexte::organismes`) : la partie normative,
//! les contrats que les textes donnent entiers (ceux de toutes les compagnies, ceux de la convention, la CASC-SVP, les
//! contributions de branche du bareme). Une compagnie qui n'a rien a saisir n'a rien a enregistrer.
//!
//! LE FICHIER EST DATE comme toute la configuration (`contexte::fiche_au`) : sa cle `adhesions_prevoyance` est une
//! liste `{depuis, valeur}`. Enregistrer pour un mois ecrit ce qui change a partir du premier jour de ce mois
//! (`contexte::dater`) ; ce qui precede garde ses contrats. La premiere composition vaut de tout temps. Les identifiants
//! d'adhesion (S21.G00.15.005) attribues ne bougent plus.

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use droit::organismes::{attendus, base, composer, couvre, ligne, saisis};
use socle::date::{en_texte, Jour};
use socle::ecriture::ecrire_json;
use socle::fichiers::{lire_json, vrai};
use socle::echec::Echec;

type R<T> = Result<T, Echec>;

pub const ROUTE_LIRE: &str = "organismes-contrats";
pub const ROUTE_ECRIRE: &str = "organismes-enregistrer";

#[track_caller]
fn plain(m: impl Into<String>) -> Echec {
    socle::echec::noter_et(None, m)
}

#[track_caller]
fn refus(m: impl Into<String>) -> Echec {
    socle::echec::noter_et(Some("ErreurOperation"), m)
}

fn g<'a>(v: &'a Value, k: &str) -> &'a Value {
    v.get(k).unwrap_or(&Value::Null)
}

fn txt(v: &Value) -> String {
    if vrai(v) {
        en_texte(v).trim().to_string()
    } else {
        String::new()
    }
}

fn chemin(ctx: &Contexte) -> std::path::PathBuf {
    ctx.racine.join("config").join(droit::contexte::ORGANISMES)
}

/// Le fichier date tel qu'il est sur le disque ; vide s'il n'existe pas.
fn fichier_date(ctx: &Contexte) -> R<Map<String, Value>> {
    let p = chemin(ctx);
    if !p.is_file() {
        return Ok(Map::new());
    }
    match lire_json(&p).map_err(plain)? {
        Value::Object(m) => Ok(m),
        _ => Err(plain("config/organismes.json : un objet est attendu.")),
    }
}

/// Une ligne se montre-t-elle a cette compagnie ? Un contrat des permanents (`role` : prevoyance ou sante, les
/// affiliations d'un salarie paye au mois) ne se montre qu'a une compagnie qui en emploie (rien a remplir pour qui
/// n'emploie pas de permanent) : le curseur est `Contexte::emploie_des_permanents`, le meme que partout.
fn se_montre(ctx: &Contexte, spec: &Value) -> bool {
    txt(g(spec, "role")).is_empty() || ctx.emploie_des_permanents()
}

/// La periode que l'ecran regarde : le mois `mois` de la charge (celui de l'onglet Paie), sinon celui du jour.
fn periode(ctx: &Contexte, charge: &Value) -> R<(Jour, Jour)> {
    let mois = match txt(g(charge, "mois")) {
        m if m.is_empty() => ctx.aujourdhui.iso()[..7].to_string(),
        m => m,
    };
    let premier = socle::date::premier_du_mois(&mois).map_err(plain)?;
    let dernier = socle::date::dernier_du_mois(&mois).map_err(plain)?;
    Ok((premier, dernier))
}

/// `organismes-contrats` : une ligne par contrat attendu qui vaut pour la compagnie (`se_montre`) et pour la periode
/// regardee (`couvre`) ; `a_saisir` : une de ces lignes attend encore ce que la compagnie seule peut dire.
pub fn lire(ctx: &Contexte, charge: &Value) -> R<Value> {
    let periode = periode(ctx, charge)?;
    let ctx = &ctx.au(&periode.1).map_err(plain)?;
    let (saisi, _) = saisis(ctx).map_err(plain)?;
    let mut lignes: Vec<Value> = Vec::new();
    for s in attendus(ctx, &base(ctx).map_err(plain)?).map_err(plain)? {
        if se_montre(ctx, &s) && couvre(&s, &periode).map_err(plain)? {
            lignes.push(ligne(&s, &saisi));
        }
    }
    let a_saisir = lignes.iter().any(|l| txt(g(l, "libelle")).is_empty() || txt(g(l, "organisme")).is_empty());
    Ok(json!({
        "idcc": txt(g(&ctx.entreprise, "idcc")),
        "a_saisir": a_saisir,
        "mois": periode.0.mois_iso(),
        "contrats": lignes,
    }))
}

/// `organismes-enregistrer` : `{"contrats": {cle: {libelle, organisme}}, "mois": "AAAA-MM"}` ; compose les adhesions
/// du mois (sous la configuration du mois) avec ce qui est saisi, refuse un contrat propose dont la reference manque,
/// ecrit ce qui change a partir du premier jour du mois (`contexte::dater` : la premiere composition vaut de tout
/// temps), et rend la lecture. Ce qui precede le mois ne change pas.
pub fn enregistrer(ctx: &Contexte, charge: &Value) -> R<Value> {
    let saisies = g(charge, "contrats").as_object().cloned().unwrap_or_default();
    let periode = periode(ctx, charge)?;
    let du_mois = ctx.au(&periode.1).map_err(plain)?;
    let (saisi, prises) = saisis(&du_mois).map_err(plain)?;
    let compose = composer(&du_mois, &saisi, prises, &saisies, Some(&periode)).map_err(refus)?;
    // Ce qu'un texte donne entier (sa reference de contrat et son identifiant) ne s'ecrit pas : il se recompose a la
    // date de chaque mois, sous la convention de ce mois.
    let fixes: Vec<String> = attendus(&du_mois, &base(&du_mois).map_err(plain)?)
        .map_err(plain)?
        .iter()
        .filter(|s| vrai(g(s, "libelle")))
        .map(|s| txt(g(s, "cle")))
        .collect();
    let adhesions: Vec<Value> = compose
        .get("adhesions_prevoyance")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|a| !fixes.contains(&txt(g(a, "cle"))))
        .cloned()
        .collect();
    let adhesions = Value::Array(adhesions);
    let (en_vigueur, _) = saisis(&du_mois.au(&periode.0).map_err(plain)?).map_err(plain)?;
    // Le fichier ne garde que la saisie : tout autre cle se recompose des textes (`droit::organismes`).
    let mut date = fichier_date(ctx)?;
    date.retain(|k, _| k == "adhesions_prevoyance");
    date.insert(
        "_quoi".into(),
        json!("Ce que la compagnie a saisi dans Configuration › Organismes et contrats, et les identifiants d'adhesion \
attribues, date : chaque cle est une liste {depuis, valeur}. Le reste se compose des textes livres a la date de chaque \
mois. Ne pas modifier a la main."),
    );
    if en_vigueur.get("adhesions_prevoyance") != Some(&adhesions) {
        droit::contexte::dater(&mut date, "adhesions_prevoyance", adhesions, Some(&periode.0), None);
    }
    let p = chemin(ctx);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| plain(e.to_string()))?;
    }
    crate::routes::mois::sauver_avant(&p)?;
    ecrire_json(&p, &Value::Object(date), 2).map_err(plain)?;
    lire(ctx, charge)
}
