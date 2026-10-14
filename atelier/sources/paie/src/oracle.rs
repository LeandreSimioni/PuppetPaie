//! `paie oracle` : la preuve du portage. Ecrit le meme JSON que
//! `python -m moteur oracle`, octet pour octet (docs/PORTAGE-RUST.md §11, §13.3).

use crate::bareme::{bareme_applique, canon};
use crate::contexte::{telle_qu_au, Contexte};
use crate::contrats::nature;
use crate::cumul::{anniversaire_configure, derniere_fin, lire_engagements, naissance, position_droits, tableau_cumul, Constantes};
use crate::date::Jour;
use crate::emplois::{emploi_de, EmploiEngagement};
use crate::engagement::{charger, nombre};
use crate::fichiers::dossier_du_code;
use crate::num::Resultat;
use crate::pas::{self, ErreurPas};
use crate::plafond::plafond_urssaf;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::PathBuf;

// ------------------------------------------------------------ l'ecrivain JSON

fn chaine(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn ecrire(v: &Value, niveau: usize, out: &mut String) {
    let marge = |n: usize, out: &mut String| out.push_str(&" ".repeat(n));
    match v {
        Value::Array(a) if !a.is_empty() => {
            out.push_str("[\n");
            for (i, x) in a.iter().enumerate() {
                marge(niveau + 1, out);
                ecrire(x, niveau + 1, out);
                out.push_str(if i + 1 < a.len() { ",\n" } else { "\n" });
            }
            marge(niveau, out);
            out.push(']');
        }
        Value::Object(o) if !o.is_empty() => {
            // Tri par point de code, comme `sort_keys=True` (l'UTF-8 se range pareil).
            let mut cles: Vec<&String> = o.keys().collect();
            cles.sort();
            out.push_str("{\n");
            for (i, k) in cles.iter().enumerate() {
                marge(niveau + 1, out);
                chaine(k, out);
                out.push_str(": ");
                ecrire(&o[*k], niveau + 1, out);
                out.push_str(if i + 1 < cles.len() { ",\n" } else { "\n" });
            }
            marge(niveau, out);
            out.push('}');
        }
        Value::Array(_) => out.push_str("[]"),
        Value::Object(_) => out.push_str("{}"),
        Value::String(s) => chaine(s, out),
        autre => out.push_str(&autre.to_string()),
    }
}

/// `json.dumps(v, ensure_ascii=False, sort_keys=True, indent=1) + "\n"`.
pub fn json_canonique(v: &Value) -> String {
    let mut out = String::new();
    ecrire(v, 0, &mut out);
    out.push('\n');
    out
}

// ---------------------------------------------------------- les sous-commandes

/// Les options `--cle valeur`. `None` : un argument mal forme (usage).
fn options(args: &[String], connues: &[&str]) -> Option<HashMap<String, String>> {
    let mut m = HashMap::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let cle = a.strip_prefix("--").filter(|c| connues.contains(c))?;
        m.insert(cle.to_string(), it.next()?.clone());
    }
    Some(m)
}

fn racine(o: &HashMap<String, String>) -> PathBuf {
    match (o.get("racine"), std::env::var("PUPPETPAIE_DONNEES")) {
        (Some(r), _) if !r.is_empty() => PathBuf::from(r),
        (_, Ok(e)) if !e.is_empty() => PathBuf::from(e),
        _ => dossier_du_code(),
    }
}

fn bareme(o: &HashMap<String, String>) -> Resultat<Value> {
    let jour = Jour::lire(&o["date"])?;
    let ctx = Contexte::nouveau(&racine(o), Some(jour))?;
    let mut sal = ctx.salarie(&o["salarie"])?;
    if let Some(c) = o.get("cadre") {
        sal["statut_categoriel_rc"] = Value::String(if c == "oui" { "01" } else { "04" }.into());
    }
    let tqa = |f: &Value, j: &str| telle_qu_au(f, &Jour::lire(j)?);
    let sal = emploi_de(&sal, Some(&o["date"]), &EmploiEngagement::default(), &tqa)?;
    let nat = nature(o.get("nature").map(String::as_str).filter(|s| !s.is_empty()))?;
    let b = bareme_applique(ctx.bareme(&jour)?, &sal, &nat, Some(&jour))?;
    let mut m = Map::new();
    m.insert("bareme".into(), canon(&b));
    Ok(Value::Object(m))
}

fn plafond(o: &HashMap<String, String>, artiste: bool) -> Resultat<Value> {
    let dates = match o.get("dates").filter(|s| !s.is_empty()) {
        Some(s) => s.split(',').map(Jour::lire).collect::<Resultat<Vec<_>>>()?,
        None => Vec::new(),
    };
    let ctx = Contexte::nouveau(&racine(o), None)?;
    let (debut, fin) = (Jour::lire(&o["debut"])?, Jour::lire(&o["fin"])?);
    let pss = |a: i32| -> Resultat<(rust_decimal::Decimal, rust_decimal::Decimal)> {
        let (m, h) = ctx.plafond_securite_sociale(a)?;
        Ok((m.into(), h.into()))
    };
    let t = |j: &Jour| (j.an, j.mois, j.jour);
    let dates: Vec<_> = dates.iter().map(t).collect();
    let p = plafond_urssaf(t(&debut), t(&fin), artiste, &pss, &dates)?;
    let mut m = Map::new();
    m.insert("plafond".into(), Value::String(nombre(p)));
    Ok(Value::Object(m))
}

fn paie(o: &HashMap<String, String>, fichier: &str, sans_minima: bool) -> Resultat<Value> {
    let jour = o.get("date").map(|d| Jour::lire(d)).transpose()?;
    let ctx = Contexte::nouveau(&racine(o), jour)?;
    crate::minima::sans_minima(sans_minima);
    let s = charger(&ctx, std::path::Path::new(fichier))?;
    let mut paies = Vec::new();
    for p in &s.paies {
        paies.push(p.canon(&ctx)?);
    }
    let mut m = Map::new();
    m.insert("mois".into(), Value::String(s.mois));
    m.insert("paies".into(), Value::Array(paies));
    Ok(Value::Object(m))
}

fn jour_facultatif(o: &HashMap<String, String>, cle: &str) -> Resultat<Option<Jour>> {
    o.get(cle).map(|d| Jour::lire(d)).transpose()
}

pub fn droits(o: &HashMap<String, String>) -> Resultat<Value> {
    let ctx = Contexte::nouveau(&racine(o), jour_facultatif(o, "date")?)?;
    let salarie = o["salarie"].as_str();
    let c = Constantes::lire(&ctx.racine)?;
    let engs = lire_engagements(&ctx, None)?;
    let (anniv, a_la_date) = (jour_facultatif(o, "anniversaire")?, jour_facultatif(o, "a-la-date")?);
    let pos = position_droits(&ctx, salarie, Some(&engs), anniv, a_la_date)?;
    let jour = |j: Option<Jour>| j.map_or(Value::Null, |j| Value::String(j.iso()));
    let mut m = Map::new();
    m.insert("engagements".into(), Value::Array(engs.iter().map(|e| e.canon(&c)).collect()));
    m.insert("position".into(), pos.canon(ctx.date)?);
    let tableau = tableau_cumul(&ctx, salarie, &o["mois"], Some(&engs))?;
    m.insert("tableau".into(), Value::Array(tableau.iter().map(|col| col.canon()).collect()));
    m.insert("derniere_fin".into(), jour(derniere_fin(&engs, salarie)));
    m.insert("naissance".into(), jour(naissance(&ctx, salarie)?));
    m.insert("anniversaire_configure".into(), jour(anniversaire_configure(&ctx, salarie)?));
    Ok(Value::Object(m))
}

pub fn pas_cmd(o: &HashMap<String, String>, fichier: &str) -> Resultat<Value> {
    let ctx = Contexte::nouveau(&racine(o), jour_facultatif(o, "date")?)?;
    let s = charger(&ctx, std::path::Path::new(fichier))?;
    let table = pas::des_paies(&ctx, &s.paies)?;
    let mut cles: std::collections::BTreeSet<String> = ctx.salaries.keys().cloned().collect();
    if let Some(m) = pas::charger(&ctx)?.get("salaries").and_then(Value::as_object) {
        cles.extend(m.keys().cloned());
    }
    let refus = |e: ErreurPas| e.message().to_string();
    let mut par_salarie = Map::new();
    for c in &cles {
        let mut e = Map::new();
        e.insert("situation".into(), pas::situation(&ctx, c, None)?);
        e.insert("connu".into(), Value::Bool(pas::connu(&ctx, c)?));
        e.insert("demande".into(), pas::demande(&ctx, c)?.unwrap_or(Value::Null));
        let taux = pas::taux_dgfip(&ctx, c, None)?;
        e.insert("taux_dgfip".into(), taux.map_or(Value::Null, |(a, b, i)| Value::Array(vec![a, b, i])));
        match pas::etat(&ctx, c, None) {
            Ok(v) => e.insert("etat".into(), v),
            Err(ErreurPas::Inconnu(m)) => e.insert("etat_erreur".into(), Value::String(m)),
            Err(autre) => return Err(refus(autre)),
        };
        match pas::rubriques(&ctx, c) {
            Ok((a, b, i)) => e.insert("rubriques".into(), Value::Array(vec![a, b, i])),
            Err(ErreurPas::Autre(m)) | Err(ErreurPas::Inconnu(m)) => e.insert("rubriques_erreur".into(), Value::String(m)),
            Err(autre) => return Err(refus(autre)),
        };
        par_salarie.insert(c.clone(), Value::Object(e));
    }
    let mut m = Map::new();
    m.insert("mois".into(), Value::String(s.mois));
    m.insert("des_paies".into(), Value::Array(table.iter().map(|v| v.as_ref().map_or(Value::Null, |v| v.canon())).collect()));
    m.insert("salaries".into(), Value::Object(par_salarie));
    m.insert("diagnostic".into(), Value::Array(pas::diagnostic(&ctx, None, None)?));
    let avert = pas::avertissements(&ctx, None, None)?;
    m.insert("avertissements".into(), Value::Array(avert.into_iter().map(Value::String).collect()));
    Ok(Value::Object(m))
}

/// L'`ent` de `operations_paies.produire_mois` : la fiche entreprise, la licence
/// resolue au premier du mois, et les gabarits de CETTE compagnie.
pub fn entreprise_du_mois(ctx: &Contexte, mois: &str) -> Resultat<Value> {
    let licence = ctx.licence(&crate::date::premier_du_mois(mois)?)?;
    let mut ent = ctx.entreprise.clone();
    if let Value::Object(m) = &mut ent {
        m.insert("licence_spectacle".into(), Value::String(licence));
        m.insert("_gabarits".into(), Value::String(ctx.gabarits().to_string_lossy().into_owned()));
    }
    Ok(ent)
}

/// La description que `bulletin.ecrire` passerait a la fabrique, comme
/// `operations_paies.produire_mois` la prepare ; `paie` : une seule paie
/// imprimee, avec toutes celles du mois pour les cumuls.
pub fn bulletin_cmd(o: &HashMap<String, String>, fichier: &str) -> Resultat<Value> {
    let ctx = Contexte::nouveau(&racine(o), jour_facultatif(o, "date")?)?;
    let s = charger(&ctx, std::path::Path::new(fichier))?;
    let ent = entreprise_du_mois(&ctx, &s.mois)?;
    let aucun = HashMap::new();
    let description = match o.get("paie") {
        None => {
            let payes: Vec<_> = s.paies.iter().filter(|p| !p.engagement.sans_paie()).cloned().collect();
            crate::bulletin::jeux_et_description(&ctx, &payes, &ent, &s.mois, None, false, &aucun)?
        }
        Some(n) => {
            let p = n
                .parse::<usize>()
                .ok()
                .and_then(|n| s.paies.get(n))
                .ok_or_else(|| crate::fichiers::trop_courte(&format!("la paie n° {}", n)))?;
            crate::bulletin::jeux_et_description(&ctx, std::slice::from_ref(p), &ent, &s.mois, Some(&s.paies), false, &aucun)?
        }
    };
    let mut m = Map::new();
    m.insert("mois".into(), Value::String(s.mois.clone()));
    m.insert("nb_paies".into(), Value::from(s.paies.len()));
    m.insert("description".into(), description);
    Ok(Value::Object(m))
}

/// `paie oracle <paie|bareme|plafond> ...` ; `None` : usage.
pub fn commande(args: &[String]) -> Option<i32> {
    let (quoi, reste) = args.split_first()?;
    let corps = match quoi.as_str() {
        "paie" => {
            // `--sans-minima` est un drapeau, sans valeur.
            let sans_minima = reste.iter().any(|a| a == "--sans-minima");
            let reste: Vec<String> = reste.iter().filter(|a| *a != "--sans-minima").cloned().collect();
            let (fichier, reste) = reste.split_first()?;
            let o = options(reste, &["racine", "date"])?;
            paie(&o, fichier, sans_minima)
        }
        "bareme" => {
            let o = options(reste, &["racine", "date", "salarie", "nature", "cadre"])?;
            if !o.contains_key("date") || !o.contains_key("salarie") {
                return None;
            }
            if o.get("cadre").is_some_and(|c| c != "oui" && c != "non") {
                return None;
            }
            bareme(&o)
        }
        "plafond" => {
            // `--statut artiste|technicien` (artiste par defaut) ; `--dates` : les journees de travail.
            let o = options(reste, &["racine", "debut", "fin", "statut", "dates"])?;
            let artiste = match o.get("statut").map(String::as_str) {
                None | Some("artiste") => true,
                Some("technicien") => false,
                Some(_) => return None,
            };
            if !o.contains_key("debut") || !o.contains_key("fin") {
                return None;
            }
            plafond(&o, artiste)
        }
        "droits" => {
            let o = options(reste, &["racine", "date", "salarie", "mois", "anniversaire", "a-la-date"])?;
            if !o.contains_key("salarie") || !o.contains_key("mois") {
                return None;
            }
            droits(&o)
        }
        "pas" => {
            let (fichier, reste) = reste.split_first()?;
            let o = options(reste, &["racine", "date"])?;
            pas_cmd(&o, fichier)
        }
        "bulletin" => {
            let (fichier, reste) = reste.split_first()?;
            let o = options(reste, &["racine", "date", "paie"])?;
            bulletin_cmd(&o, fichier)
        }
        _ => return None,
    };
    let mut m = match corps {
        Ok(Value::Object(m)) => {
            let mut m = m;
            m.insert("ok".into(), Value::Bool(true));
            m
        }
        Ok(_) => unreachable!(),
        Err(e) => {
            let mut m = Map::new();
            m.insert("ok".into(), Value::Bool(false));
            m.insert("erreur".into(), Value::String(e));
            m
        }
    };
    let v = canon(&Value::Object(std::mem::take(&mut m)));
    print!("{}", json_canonique(&v));
    Some(0)
}
