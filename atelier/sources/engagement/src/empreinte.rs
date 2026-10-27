//! CE DONT UNE PAIE DEPEND. Le calcul d'un mois note tout ce qu'il lit (`socle::fichiers::suivre`) : le contenu de chaque
//! fichier (baremes du socle et de la compagnie, catalogues livres, fiches, effectifs, imprevus, taux de prelevement a la
//! source...), les fichiers absents, les dossiers parcourus, les fiches telles qu'elles valent a la date du calcul, le
//! jour s'il le lit, et la saisie de chaque mois qu'il lit (`stock::vue_du_mois`, la saisie, pas le fichier : ecrire la
//! validation d'un engagement ne change rien a ce que les autres lisent). S'y ajoutent la marque du code qui calcule
//! (`MOTEUR`) et les regles datees qui s'appliquent a la periode payee (`regles`).
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

/// Les fiches telles qu'elles valent a la date du contexte.
fn empreinte_fiches(ctx: &Contexte) -> String {
    let v = Value::Array(vec![ctx.entreprise.clone(), Value::Object(ctx.salaries.clone()), Value::Object(ctx.spectacles.clone())]);
    fichiers::sha256_hex(v.to_string().as_bytes())
}

/// Une entree de fichier qui est la saisie d'un mois : elle se compte par la saisie (`stock::cle_de_saisie`), pas par
/// les octets de son fichier.
fn est_un_fichier_d_engagements(cle: &str) -> bool {
    cle.split_once(' ').is_some_and(|(_, c)| c.starts_with(&format!("racine:{}/", stock::DOSSIER)) || c == format!("racine:{}", stock::DOSSIER))
}

/// `f`, qui calcule depuis la saisie `data` d'un mois, et tout ce qu'il a lu.
pub fn suivre<T>(ctx: &Contexte, data: &Value, f: impl FnOnce() -> Resultat<T>) -> Resultat<(T, BTreeMap<String, String>)> {
    fichiers::suivre(&ctx.racine, &dossier_du_code());
    if let Some(m) = data.get("mois").and_then(Value::as_str) {
        fichiers::noter_valeur(&stock::cle_de_saisie(m), stock::empreinte_de_saisie(data));
    }
    let r = f();
    fichiers::noter_valeur("fiches", empreinte_fiches(ctx));
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

fn entree_tient(ctx: &Contexte, k: &str, v: &str) -> bool {
    let (racine, code) = (&ctx.racine, dossier_du_code());
    match k {
        "moteur" => v == MOTEUR,
        "fiches" => v == empreinte_fiches(ctx),
        "date" => v == ctx.date.iso(),
        "aujourdhui" => droit::contexte::aujourdhui_sans_note().is_ok_and(|j| v == j.iso()),
        _ => {
            if let Some(mois) = k.strip_prefix("saisie ") {
                return stock::vue_du_mois(racine, mois).ok().flatten().is_some_and(|vue| v == stock::empreinte_de_saisie(&vue));
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

/// L'empreinte telle qu'elle s'ecrit : les entrees, et les regles datees.
pub fn en_json(entrees: &BTreeMap<String, String>, regles: &[String]) -> Value {
    let mut m = Map::new();
    m.insert("entrees".into(), Value::Object(entrees.iter().map(|(k, v)| (k.clone(), Value::String(v.clone()))).collect()));
    m.insert("regles".into(), Value::Array(regles.iter().cloned().map(Value::String).collect()));
    Value::Object(m)
}

/// Les entrees d'une empreinte ecrite ; `None` si elle ne se lit pas.
pub fn entrees_de(v: &Value) -> Option<BTreeMap<String, String>> {
    v.get("entrees")?.as_object()?.iter().map(|(k, x)| x.as_str().map(|s| (k.clone(), s.to_string()))).collect()
}

/// Les regles datees d'une empreinte ecrite.
pub fn regles_de(v: &Value) -> Vec<String> {
    v.get("regles").and_then(Value::as_array).map_or_else(Vec::new, |a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
}
