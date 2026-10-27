//! Les regles des permanents (`droit::permanents` : conges payes, fin de contrat, preavis et indemnites legales,
//! maintien de salaire) contre `permanents.json`. `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit l'oracle.

use crate::outils::{egal, fnv, lire_oracle, oracle_selon};
use droit::natures::{nature, Nature};
use socle::date::Jour;

use socle::num::{Decimal, Resultat};
use droit::permanents::{self as p, OptionsMaintien};
use serde_json::{json, Value};
use std::str::FromStr;

fn lire() -> Value {
    lire_oracle("permanents.json")
}

fn jour(v: &Value) -> Jour {
    Jour::lire(v.as_str().unwrap()).unwrap()
}

fn dec(v: &Value) -> Decimal {
    Decimal::from_str(v.as_str().unwrap()).unwrap()
}

fn motif(v: &Value) -> Option<&str> {
    v.as_str()
}

fn nat(v: &Value) -> std::sync::Arc<Nature> {
    nature(Some(v.as_str().unwrap())).unwrap()
}


fn dec_texte(v: Decimal) -> Value {
    Value::String(v.to_string())
}

fn resultat(r: Resultat<Value>) -> Value {
    match r {
        Ok(v) => v,
        Err(e) => json!({"err": e}),
    }
}

/// Rejoue une section : `[arguments, resultat]` par cas.
fn section(rel: &Value, nom: &str, f: impl Fn(&[Value]) -> Resultat<Value>) {
    let cas: Vec<Value> = rel[nom]
        .as_array()
        .unwrap_or_else(|| panic!("section {} absente", nom))
        .iter()
        .map(|c| json!([c[0], resultat(f(c[0].as_array().unwrap()))]))
        .collect();
    oracle_selon("permanents.json", &format!("/{}", nom), &Value::Array(cas), egal);
}

fn option(v: &Value) -> Option<&Value> {
    if v.is_null() {
        None
    } else {
        Some(v)
    }
}

#[test]
fn les_conges() {
    let rel = lire();
    section(&rel, "arrondir_jours", |a| Ok(dec_texte(p::arrondir_jours(dec(&a[0])))));
    section(&rel, "plafonner", |a| p::plafonner(dec(&a[0])).map(dec_texte));
    section(&rel, "periode_de_reference", |a| {
        p::periode_de_reference(&jour(&a[0]), option(&a[1])).map(|(d, f)| json!([d.iso(), f.iso()]))
    });
    section(&rel, "jours_acquis", |a| p::jours_acquis(&jour(&a[0]), &jour(&a[1])).map(dec_texte));
    section(&rel, "indemnite_conges", |a| {
        p::indemnite_conges(dec(&a[0]), dec(&a[1]), dec(&a[2]), dec(&a[3]))
    });
}

#[test]
fn la_fin_du_contrat() {
    let rel = lire();
    section(&rel, "precarite_due", |a| {
        p::precarite_due(&nat(&a[0]), motif(&a[1])).map(|o| o.map_or(Value::Null, Value::String))
    });
    section(&rel, "indemnite_precarite", |a| {
        p::indemnite_precarite(dec(&a[0]), a[1].as_f64()).map(dec_texte)
    });
    section(&rel, "anciennete_mois", |a| Ok(dec_texte(p::anciennete_mois(&jour(&a[0]), &jour(&a[1])))));
    section(&rel, "preavis_mois", |a| {
        p::preavis_mois(dec(&a[0]), motif(&a[1]), false).map(|o| o.map_or(Value::Null, dec_texte))
    });
    section(&rel, "salaire_de_reference", |a| {
        let liste = |v: &Value| v.as_array().unwrap().iter().map(dec).collect::<Vec<_>>();
        Ok(p::salaire_de_reference(&liste(&a[0]), &liste(&a[1])))
    });
    section(&rel, "indemnite_licenciement", |a| {
        p::indemnite_licenciement(dec(&a[0]), dec(&a[1]), motif(&a[2]))
    });
    section(&rel, "indemnite_mise_a_la_retraite", |a| {
        p::indemnite_mise_a_la_retraite(dec(&a[0]), dec(&a[1]))
    });
    section(&rel, "indemnite_rupture_conventionnelle", |a| {
        p::indemnite_rupture_conventionnelle(dec(&a[0]), dec(&a[1]))
    });
    section(&rel, "indemnite_depart_retraite", |a| p::indemnite_depart_retraite(dec(&a[0]), dec(&a[1])));
    section(&rel, "rupture", |a| p::rupture(motif(&a[0])).map(|o| o.unwrap_or(Value::Null)));
    section(&rel, "code_52_permis", |a| {
        p::code_52_permis(motif(&a[0]), motif(&a[1])).map(|o| o.map_or(Value::Null, Value::String))
    });
}

fn tranches(t: Vec<p::Tranche>) -> Value {
    Value::Array(t.into_iter().map(|t| json!({"jours": t.jours, "taux": dec_texte(t.taux)})).collect())
}

fn options_maintien(dj: i64, at: bool, tr: bool) -> OptionsMaintien {
    OptionsMaintien { deja_indemnises: dj, accident_du_travail: at, accident_de_trajet: tr }
}

/// La meme ligne que `ligne()` du releve.
fn ligne(r: &Value) -> String {
    let tr: Vec<String> = r["tranches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            format!("{}:{}:{}", t["jours"], dec(&t["taux"]).normalize(), t["montant"].as_str().unwrap())
        })
        .collect();
    format!(
        "{},{},{},{},{},{}",
        r["carence"],
        r["jours_indemnisables"],
        r["jours_indemnises"],
        r["jours_au_dela_du_droit"],
        r["brut_avant_deduction"].as_str().unwrap(),
        tr.join(";")
    )
}

#[test]
fn le_maintien() {
    let rel = lire();
    section(&rel, "droit_au_maintien", |a| {
        let n = if a[0].is_null() { None } else { Some(nat(&a[0])) };
        p::droit_au_maintien(n.as_deref(), dec(&a[1])).map(|o| o.map_or(Value::Null, Value::String))
    });
    section(&rel, "jours_de_carence", |a| {
        p::jours_de_carence(a[0].as_bool().unwrap(), a[1].as_bool().unwrap()).map(|n| json!(n))
    });
    section(&rel, "duree_du_maintien", |a| p::duree_du_maintien(dec(&a[0])).map(tranches));
    // `[arguments, resultat, [deja_indemnises, accident_du_travail, accident_de_trajet]]`.
    let detail: Vec<Value> = rel["maintien_detail"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            let a = c[0].as_array().unwrap();
            let o = options_maintien(c[2][0].as_i64().unwrap(), c[2][1].as_bool().unwrap(), c[2][2].as_bool().unwrap());
            json!([c[0], resultat(p::maintien_de_salaire(dec(&a[0]), a[1].as_i64().unwrap(), dec(&a[2]), &o)), c[2]])
        })
        .collect();
    oracle_selon("permanents.json", "/maintien_detail", &Value::Array(detail), egal);

    // `[anciennete, accident_du_travail, accident_de_trajet, deja_indemnises, empreinte des 401 jours]`.
    let blocs: Vec<Value> = rel["maintien_blocs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| {
            let (anc, at, tr, dj) = (dec(&b[0]), b[1].as_bool().unwrap(), b[2].as_bool().unwrap(), b[3].as_i64().unwrap());
            let lignes: Vec<String> =
                (0..=400).map(|j| ligne(&p::maintien_de_salaire(anc, j, Decimal::from_str("137.45").unwrap(), &options_maintien(dj, at, tr)).unwrap())).collect();
            json!([b[0], b[1], b[2], b[3], fnv(&lignes.join("\n"))])
        })
        .collect();
    oracle_selon("permanents.json", "/maintien_blocs", &Value::Array(blocs), egal);
}

