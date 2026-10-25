//! Les clauses conventionnelles (`clauses-conventionnelles-base.json`, 18 IDCC) contre `clauses.json` : profil, preavis,
//! lecture des baremes, paliers, clause d'annexe, indemnites, maintien, prime d'anciennete. Des blocs (une grille par
//! jeu de parametres) resumes par une empreinte, et des cas lisibles. Les grandes grilles (`profil`, `preavis_mois`,
//! `indemnite_licenciement`, `prime_anciennete`) se jouent sur un echantillon a graine fixe (decision E).
//! `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit l'oracle.

use crate::outils::{egal, fnv, oracle, oracle_selon, reecrire_oracles, tire};
use droit::clauses::{self as c, P};
use socle::date::Jour;
use socle::num::{Decimal, Resultat};
use serde_json::{json, Map, Value};
use std::str::FromStr;

const GRAINE: u64 = 20261011;

fn lire() -> Value {
    crate::outils::lire_oracle("clauses.json")
}

fn dec(v: &Value) -> Decimal {
    Decimal::from_str(v.as_str().unwrap()).unwrap()
}

fn dec_option(v: &Value) -> Option<Decimal> {
    v.as_str().map(|s| Decimal::from_str(s).unwrap())
}

fn chaines(v: &Value) -> Vec<Value> {
    v.as_array().unwrap().clone()
}

// ------------------------------------------------------- le JSON canonique
fn numerique(s: &str) -> bool {
    let (nombre, _) = s.split_once('E').unwrap_or((s, "0"));
    let s = nombre.strip_prefix('-').unwrap_or(nombre);
    let (e, d) = s.split_once('.').map_or((s, Some("0")), |(e, d)| (e, Some(d)));
    !e.is_empty()
        && e.chars().all(|c| c.is_ascii_digit())
        && d.map_or(true, |d| !d.is_empty() && d.chars().all(|c| c.is_ascii_digit()))
}

/// `format(Decimal(s).normalize(), "f")` : sans exposant, sans zeros de queue.
fn normaliser(s: &str) -> String {
    let (signe, s) = s.strip_prefix('-').map_or(("", s), |r| ("-", r));
    let (nombre, exposant) = s.split_once('E').map_or((s, 0i64), |(n, e)| (n, e.parse().unwrap()));
    let (entiere, fraction) = nombre.split_once('.').unwrap_or((nombre, ""));
    let chiffres = format!("{}{}", entiere, fraction);
    let point = entiere.len() as i64 + exposant;
    let (e, f) = if point <= 0 {
        ("0".to_string(), format!("{}{}", "0".repeat((-point) as usize), chiffres))
    } else if point as usize >= chiffres.len() {
        (format!("{}{}", chiffres, "0".repeat(point as usize - chiffres.len())), String::new())
    } else {
        (chiffres[..point as usize].to_string(), chiffres[point as usize..].to_string())
    };
    let (e, f) = (e.trim_start_matches('0'), f.trim_end_matches('0'));
    format!("{}{}{}{}", signe, if e.is_empty() { "0" } else { e }, if f.is_empty() { "" } else { "." }, f)
}

fn canon(v: &Value, out: &mut String) {
    match v {
        Value::String(s) if numerique(s) => {
            let n = normaliser(s);
            out.push_str(&serde_json::to_string(&n).unwrap());
        }
        Value::Array(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                canon(x, out);
            }
            out.push(']');
        }
        Value::Object(o) => {
            let mut cles: Vec<&String> = o.keys().collect();
            cles.sort();
            out.push('{');
            for (i, k) in cles.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(k).unwrap());
                out.push(':');
                canon(&o[*k], out);
            }
            out.push('}');
        }
        autre => out.push_str(&serde_json::to_string(autre).unwrap()),
    }
}

fn ligne(r: Resultat<Value>) -> String {
    match r {
        Ok(v) => {
            let mut s = String::new();
            canon(&v, &mut s);
            s
        }
        Err(_) => "ERR".to_string(),
    }
}

/// Rejoue des blocs : `f` donne les lignes d'un bloc a partir de ses parametres (tout sauf l'empreinte, le dernier
/// element). Sous reecriture, un bloc sur `un_sur` seulement (l'echantillon) ; les blocs se repartissent sur les coeurs.
fn blocs(rel: &Value, nom: &str, un_sur: u64, f: impl Fn(&[Value]) -> Vec<String> + Sync) {
    let liste: Vec<&[Value]> = rel["blocs"][nom]
        .as_array()
        .unwrap_or_else(|| panic!("blocs {} absents", nom))
        .iter()
        .map(|b| b.as_array().unwrap().split_last().unwrap().1)
        .filter(|p| !reecrire_oracles() || tire(GRAINE, &Value::Array(p.to_vec()), un_sur))
        .collect();
    let coeurs = std::thread::available_parallelism().map_or(1, |n| n.get());
    let f = &f;
    let obtenus: Vec<Value> = std::thread::scope(|s| {
        let fils: Vec<_> = liste
            .chunks(liste.len().div_ceil(coeurs).max(1))
            .map(|lot| s.spawn(move || lot.iter().map(|p| Value::Array([p.to_vec(), vec![Value::String(fnv(&f(p).join("\n")))]].concat())).collect::<Vec<_>>()))
            .collect();
        fils.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    oracle("clauses.json", &format!("/blocs/{}", nom), &Value::Array(obtenus));
}

// ------------------------------------------------------------- les cas
fn section(rel: &Value, nom: &str, f: impl Fn(&[Value]) -> Resultat<Value>) {
    let cas: Vec<Value> = rel["cas"][nom]
        .as_array()
        .unwrap_or_else(|| panic!("section {} absente", nom))
        .iter()
        .map(|c| json!([c[0], f(c[0].as_array().unwrap()).unwrap_or_else(|_| json!({"err": 1}))]))
        .collect();
    oracle_selon("clauses.json", &format!("/cas/{}", nom), &Value::Array(cas), egal);
}

fn option(v: &Value) -> Option<&Value> {
    if v.is_null() {
        None
    } else {
        Some(v)
    }
}

fn paire(r: Option<(String, Value)>) -> Value {
    r.map_or(Value::Null, |(a, s)| json!([a, s]))
}

fn texte_option(r: Option<String>) -> Value {
    r.map_or(Value::Null, Value::String)
}

fn jour(v: &Value) -> Jour {
    Jour::lire(v.as_str().unwrap()).unwrap()
}

fn avec(base: &Value, k: &str, v: Value) -> Value {
    let mut m: Map<String, Value> = base.as_object().cloned().unwrap_or_default();
    m.insert(k.to_string(), v);
    Value::Object(m)
}

// ----------------------------------------------------------------- les tests
#[test]
fn profil() {
    let rel = lire();
    let g = &rel["grilles"];
    let (fiches, engs) = (chaines(&g["fiches"]), chaines(&g["engagements"]));
    blocs(&rel, "profil", 10, |p| {
        let mut out = Vec::new();
        for fi in &fiches {
            for en in &engs {
                for place in ["eng", "fiche"] {
                    let (mut fiche, mut eng) = (fi.clone(), option(en).cloned());
                    if place == "eng" {
                        eng = Some(avec(eng.as_ref().unwrap_or(&json!({})), "metier", p[1].clone()));
                    } else {
                        fiche = avec(&fiche, "code_emploi_ft", p[1].clone());
                    }
                    out.push(ligne(c::profil(&p[0], &fiche, eng.as_ref())));
                }
            }
        }
        out
    });
    section(&rel, "profil", |a| c::profil(&a[0], &a[1], option(&a[2])));
}

#[test]
fn preavis() {
    let rel = lire();
    let g = &rel["grilles"];
    let (anc, ages) = (chaines(&g["anc_p"]), chaines(&g["ages_p"]));
    blocs(&rel, "preavis_mois", 20, |p| {
        let mut out = Vec::new();
        for a in &anc {
            for age in &ages {
                out.push(ligne(c::preavis_mois(&p[0], p[1].as_str(), dec(a), p[2].as_str(), dec_option(age))));
            }
        }
        out
    });
    section(&rel, "preavis_mois", |a| {
        c::preavis_mois(&a[0], a[1].as_str(), dec(&a[2]), a[3].as_str(), dec_option(&a[4]))
    });
}

#[test]
fn les_baremes() {
    let rel = lire();
    let g = &rel["grilles"];
    let annees: Vec<P> = g["annees"].as_array().unwrap().iter().map(|a| P::depuis(dec(a))).collect();
    let listes = chaines(&g["listes"]);
    let noms = ["tranches", "par_tranches", "par_bandes", "par_bandes_x"];
    let mut lecture = Vec::new();
    for l in &listes {
        let liste = l[1].as_array().unwrap();
        for nom in noms {
            let mut lignes = Vec::new();
            for a in &annees {
                let d = |r: Resultat<P>| ligne(r.map(|x| Value::String(x.to_string())));
                lignes.push(d(c::lecture(nom, liste, *a)));
                if nom == "tranches" {
                    lignes.push(d(c::par_tranches(liste, *a, true)));
                    lignes.push(d(c::par_tranches(liste, *a, false)));
                    lignes.push(d(c::par_bandes(liste, *a)));
                }
            }
            lecture.push(json!([l[0], nom, fnv(&lignes.join("\n"))]));
        }
    }
    oracle("clauses.json", "/blocs/lecture", &Value::Array(lecture));
    let paliers: Vec<Value> = chaines(&g["paliers"])
        .iter()
        .map(|s| {
            let lignes: Vec<String> = annees.iter().map(|a| ligne(c::paliers(&s[1], *a).map(|(m, p)| json!([m.to_string(), p])))).collect();
            json!([s[0], fnv(&lignes.join("\n"))])
        })
        .collect();
    oracle("clauses.json", "/blocs/paliers", &Value::Array(paliers));

    section(&rel, "pct", |a| Ok(Value::String(c::pct(&a[0])?)));
}

#[test]
fn clause_d_annexe() {
    let rel = lire();
    let g = &rel["grilles"];
    let (profs, motifs) = (chaines(&g["profs"]), chaines(&g["motifs"]));
    blocs(&rel, "clause_d_annexe", 1, |p| {
        let mut out = Vec::new();
        for prof in &profs {
            for m in &motifs {
                out.push(ligne(c::clause_d_annexe(&p[0], &p[1], prof, m.as_str()).map(paire)));
            }
        }
        out
    });
    section(&rel, "clause_d_annexe", |a| {
        c::clause_d_annexe(&a[0], &a[1], &a[2], a[3].as_str()).map(paire)
    });
}

#[test]
fn indemnite_de_licenciement() {
    let rel = lire();
    let g = &rel["grilles"];
    let (motifs, anc, refs, ages) =
        (chaines(&g["motifs_l"]), chaines(&g["anc_l"]), chaines(&g["refs_peu"]), chaines(&g["ages_l"]));
    blocs(&rel, "indemnite_licenciement", 8, |p| {
        let mut out = Vec::new();
        for m in &motifs {
            for a in &anc {
                for r in &refs {
                    for age in &ages {
                        out.push(ligne(c::indemnite_licenciement(
                            &p[0], &p[1], &p[2], dec(a), dec(r), m.as_str(), dec_option(age),
                        )));
                    }
                }
            }
        }
        out
    });
    section(&rel, "indemnite_licenciement", |a| {
        c::indemnite_licenciement(&a[0], &a[1], &a[2], dec(&a[3]), dec(&a[4]), a[5].as_str(), dec_option(&a[6]))
    });
}

#[test]
fn retraite() {
    let rel = lire();
    let g = &rel["grilles"];
    let (anc, refs) = (chaines(&g["anc"]), chaines(&g["refs"]));
    blocs(&rel, "indemnite_mise_a_la_retraite", 1, |p| {
        let mut out = Vec::new();
        for a in &anc {
            for r in &refs {
                out.push(ligne(c::indemnite_mise_a_la_retraite(&p[0], dec(a), dec(r))));
            }
        }
        out
    });
    blocs(&rel, "indemnite_depart_retraite", 1, |p| {
        let mut out = Vec::new();
        for a in &anc {
            for r in &refs {
                out.push(ligne(c::indemnite_depart_retraite(&p[0], dec(a), dec(r))));
            }
        }
        out
    });
    section(&rel, "indemnite_mise_a_la_retraite", |a| {
        c::indemnite_mise_a_la_retraite(&a[0], dec(&a[1]), dec(&a[2]))
    });
    section(&rel, "indemnite_depart_retraite", |a| {
        c::indemnite_depart_retraite(&a[0], dec(&a[1]), dec(&a[2]))
    });
}

#[test]
fn maintien() {
    let rel = lire();
    let anc = chaines(&rel["grilles"]["anc"]);
    blocs(&rel, "maintien", 1, |p| {
        anc.iter()
            .map(|a| ligne(c::maintien(&p[0], dec(a), p[1].as_bool().unwrap()).map(texte_option)))
            .collect()
    });
    section(&rel, "maintien", |a| {
        c::maintien(&a[0], dec(&a[1]), a[2].as_bool().unwrap()).map(texte_option)
    });
}

#[test]
fn prime_d_anciennete() {
    let rel = lire();
    let g = &rel["grilles"];
    let (fiches, engs, jours, jauges) =
        (chaines(&g["fiches_p"]), chaines(&g["eng_p"]), chaines(&g["jours"]), chaines(&g["jauges"]));
    let jours: Vec<Jour> = jours.iter().map(jour).collect();
    let prime = |idcc: &Value, annexe: &Value, fiche: &Value, eng: Option<&Value>, j: &Jour, jg: Option<&str>| {
        c::prime_anciennete(idcc, annexe, fiche, eng, j, jg).map(|r| r.unwrap_or(Value::Null))
    };
    blocs(&rel, "prime_anciennete", 20, |p| {
        let mut out = Vec::new();
        for fi in &fiches {
            for en in &engs {
                let fiche = avec(fi, "code_emploi_ft", p[2].clone());
                for j in &jours {
                    for jg in &jauges {
                        out.push(ligne(prime(&p[0], &p[1], &fiche, option(en), j, jg.as_str())));
                    }
                }
            }
        }
        out
    });
    for nom in ["prime_anciennete", "prime_anciennete_chaine"] {
        section(&rel, nom, |a| prime(&a[0], &a[1], &a[2], option(&a[3]), &jour(&a[4]), a[5].as_str()));
    }
}
