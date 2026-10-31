//! CE DONT UNE PAIE DEPEND : l'empreinte est celle d'un engagement, pas d'un mois. Le calcul d'un mois note tout ce qu'il
//! lit (`socle::fichiers::suivre`) : le contenu de chaque fichier (baremes du socle et de la compagnie, catalogues livres,
//! effectifs, imprevus, taux de prelevement a la source...), les fichiers absents, les dossiers parcourus, le jour s'il le
//! lit, et les paies validees des autres mois qu'il
//! lit (`paies <mois>`, `etats::paies_validees`). De la saisie de son mois, chaque paie ne garde que ce dont elle depend :
//! ce que le mois pose pour tous, les engagements valides de son salarie (`saisie <mois> <salarie>`,
//! `etats::calculer_les_rangs`) et les fiches de l'entreprise, de son salarie et de son spectacle au dernier jour du mois
//! (`fiches`). S'y ajoutent la marque du code qui calcule (`MOTEUR`) et les regles datees qui
//! s'appliquent a la periode payee (`regles`).
//!
//! L'empreinte TIENT quand chacune de ses entrees vaut encore ce qu'elle valait (`tient`) : aucun calcul, une relecture
//! du disque. Elle ne tient plus : la paie se recalcule, et ce qui a change dit pourquoi (`etats::rafraichir`).

use std::collections::{BTreeMap, HashMap};


use serde_json::{Map, Value};

use droit::contexte::Contexte;
use socle::date::Jour;
use socle::fichiers::{self, dossier_du_code};
use socle::num::Resultat;

use crate::stock;

/// La marque du code qui calcule : la version du crate et celle de ses sources et de celles du socle et du droit
/// (`build.rs`). Un autre code recalcule ce qui n'est pas parti.
pub const MOTEUR: &str = concat!(env!("CARGO_PKG_VERSION"), "+", env!("ENGAGEMENT_SOURCES"));

/// L'entree des fiches dont depend la paie de `salarie` pour `mois`, sur `spectacle` : l'entreprise, la fiche du salarie
/// et celle du spectacle, telles qu'elles valent au dernier jour du mois (`Contexte::du_mois`). La fiche d'un autre
/// salarie, une valeur datee apres le mois n'en sont pas : elles ne font pas recalculer cette paie.
pub(crate) fn fiches(ctx: &Contexte, mois: &str, salarie: &str, spectacle: Option<&str>) -> Resultat<(String, String)> {
    let cle = Value::Array(vec![mois.into(), salarie.into(), spectacle.map_or(Value::Null, Value::from)]);
    Ok((format!("fiches {}", cle), empreinte_fiches(&ctx.du_mois(mois)?, salarie, spectacle)))
}

fn empreinte_fiches(ctx: &Contexte, salarie: &str, spectacle: Option<&str>) -> String {
    let fiche = |m: &Map<String, Value>, k: Option<&str>| k.and_then(|k| m.get(k)).cloned().unwrap_or(Value::Null);
    let v = Value::Array(vec![ctx.entreprise.clone(), fiche(&ctx.salaries, Some(salarie)), fiche(&ctx.spectacles, spectacle)]);
    fichiers::sha256_hex(v.to_string().as_bytes())
}

/// L'entree `fiches [mois, salarie, spectacle]` tient-elle encore ?
fn fiches_tiennent(ctx: &Contexte, cle: &str, v: &str) -> bool {
    let Ok(Value::Array(c)) = serde_json::from_str::<Value>(cle) else { return false };
    match (c.first().and_then(Value::as_str), c.get(1).and_then(Value::as_str)) {
        (Some(mois), Some(salarie)) => fiches(ctx, mois, salarie, c.get(2).and_then(Value::as_str)).is_ok_and(|(_, e)| e == v),
        _ => false,
    }
}

/// Une entree de fichier qui est la saisie d'un mois : elle se compte par la saisie (`stock::cle_de_saisie`), pas par
/// les octets de son fichier.
fn est_un_fichier_d_engagements(cle: &str) -> bool {
    cle.split_once(' ').is_some_and(|(_, c)| c.starts_with(&format!("racine:{}/", stock::DOSSIER)) || c == format!("racine:{}", stock::DOSSIER))
}

/// `f`, et tout ce qu'il a lu. Les fiches s'y ajoutent paie par paie (`fiches`).
pub(crate) fn suivre<T>(ctx: &Contexte, f: impl FnOnce() -> Resultat<T>) -> Resultat<(T, BTreeMap<String, String>)> {
    fichiers::suivre(&ctx.racine, &dossier_du_code());
    let r = f();
    let mut entrees = fichiers::rendre();
    entrees.retain(|k, _| !est_un_fichier_d_engagements(k));
    entrees.insert("moteur".into(), MOTEUR.into());
    Ok((r?, entrees))
}

/// Ce qu'une passe de verification a deja relu : une entree qui revient d'une paie a l'autre (les baremes, les fiches, la
/// saisie du mois) ne se relit qu'une fois.
#[derive(Default)]
pub struct Relues(HashMap<(String, String), bool>);

/// Les entrees valent-elles encore ce qu'elles valaient ? Chacune est relue sur le disque ; la saisie d'un mois se relit
/// et se hache ; rien ne se calcule.
pub fn tient(ctx: &Contexte, entrees: &BTreeMap<String, String>, relues: &mut Relues) -> bool {
    entrees.iter().all(|(k, v)| {
        if let Some(t) = relues.0.get(&(k.clone(), v.clone())) {
            return *t;
        }
        let t = entree_tient(ctx, k, v);
        relues.0.insert((k.clone(), v.clone()), t);
        t
    })
}

/// Une decision de l'editeur rend-elle l'empreinte ecrite `ecrite` (d'une paie de `mois`) fausse ? Le code qui calcule
/// a change (`MOTEUR`, ou un fichier livre avec le code), ou une regle datee s'applique au mois qu'elle n'avait pas.
/// Rien ne se calcule.
pub fn code_ou_regle_change(ctx: &Contexte, ecrite: &Value, mois: &str, relues: &mut Relues) -> bool {
    let code = |k: &str| k == "moteur" || k.split_once(' ').is_some_and(|(_, c)| c.starts_with("code:"));
    let entrees: BTreeMap<String, String> = entrees_de(ecrite).unwrap_or_default().into_iter().filter(|(k, _)| code(k)).collect();
    if !tient(ctx, &entrees, relues) {
        return true;
    }
    let ecrites = regles_de(ecrite);
    socle::date::dernier_du_mois(mois)
        .ok()
        .and_then(|fin| regles(ctx, &fin).ok())
        .is_some_and(|r| r.iter().any(|x| !ecrites.contains(x)))
}

fn entree_tient(ctx: &Contexte, k: &str, v: &str) -> bool {
    let (racine, code) = (&ctx.racine, dossier_du_code());
    match k {
        "moteur" => v == MOTEUR,
        k if k.starts_with("fiches ") => fiches_tiennent(ctx, &k["fiches ".len()..], v),
        "date" => v == ctx.date.iso(),
        "aujourdhui" => droit::contexte::aujourdhui_sans_note().is_ok_and(|j| v == j.iso()),
        _ => {
            if let Some(mois) = k.strip_prefix("paies ") {
                return crate::etats::empreinte_des_paies_validees(ctx, mois).is_some_and(|e| v == e);
            }
            if let Some(reste) = k.strip_prefix("saisie ") {
                let (mois, salarie) = reste.split_once(' ').map_or((reste, None), |(m, s)| (m, Some(s)));
                return stock::empreinte_de_la_saisie_du_mois(racine, mois, salarie).ok().flatten().is_some_and(|e| v == e);
            }
            match fichiers::chemin_de(k, racine, &code) {
                Some((genre, p)) => match genre.as_str() {
                    "fichier" => v == fichiers::empreinte_fichier(&p),
                    "existe" => v == if p.exists() { "oui" } else { "non" },
                    "dossier?" => v == if p.is_dir() { "oui" } else { "non" },
                    "dossier" => v == fichiers::empreinte_dossier(&p),
                    _ => false,
                },
                None => false,
            }
        }
    }
}

/// Les regles datees qui s'appliquent au jour `jour` (`Contexte::regles_datees`), en AAAA-MM-JJ.
pub fn regles(ctx: &Contexte, jour: &Jour) -> Resultat<Vec<String>> {
    Ok(ctx.regles_datees(jour)?.into_iter().map(|j| j.iso()).collect())
}

/// Les valeurs datees de la compagnie qui s'appliquent au jour `jour` a l'engagement de `salarie` sur `spectacle`
/// (`Contexte::configuration_datee`), en AAAA-MM-JJ. Elles ne font pas recalculer (les fiches sont deja dans les
/// entrees) : elles disent a quelle date s'applique une valeur nouvelle.
pub fn configuration(ctx: &Contexte, jour: &Jour, salarie: &str, spectacle: Option<&str>) -> Resultat<Vec<String>> {
    Ok(ctx.configuration_datee(jour, salarie, spectacle)?.into_iter().map(|j| j.iso()).collect())
}

/// L'empreinte telle qu'elle s'ecrit : les entrees, les regles datees, et les valeurs datees de la compagnie.
pub fn en_json(entrees: &BTreeMap<String, String>, regles: &[String], configuration: &[String]) -> Value {
    let mut m = Map::new();
    m.insert("entrees".into(), Value::Object(entrees.iter().map(|(k, v)| (k.clone(), Value::String(v.clone()))).collect()));
    m.insert("regles".into(), Value::Array(regles.iter().cloned().map(Value::String).collect()));
    m.insert("configuration".into(), Value::Array(configuration.iter().cloned().map(Value::String).collect()));
    Value::Object(m)
}

/// Les valeurs datees de la compagnie d'une empreinte ecrite.
pub fn configuration_de(v: &Value) -> Vec<String> {
    v.get("configuration").and_then(Value::as_array).map_or_else(Vec::new, |a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
}

/// Les entrees d'une empreinte ecrite ; `None` si elle ne se lit pas.
pub fn entrees_de(v: &Value) -> Option<BTreeMap<String, String>> {
    v.get("entrees")?.as_object()?.iter().map(|(k, x)| x.as_str().map(|s| (k.clone(), s.to_string()))).collect()
}

/// Les regles datees d'une empreinte ecrite.
pub fn regles_de(v: &Value) -> Vec<String> {
    v.get("regles").and_then(Value::as_array).map_or_else(Vec::new, |a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
}
