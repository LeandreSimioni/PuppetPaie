//! Les clauses conventionnelles (`clauses-conventionnelles-base.json`) contre `clauses.json` : profil, preavis, lecture
//! des baremes, paliers, clause d'annexe, indemnites, prime d'anciennete. Des blocs (une grille par jeu de parametres)
//! resumes par une empreinte, et des cas lisibles ; les resultats s'y ecrivent en JSON (`en_valeur_*`), les nombres en
//! texte. Les grandes grilles (`profil`, `preavis_mois`, `indemnite_licenciement`, `prime_anciennete`) se jouent sur un
//! echantillon a graine fixe. `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit l'oracle.

use crate::outils::{egal, fnv, oracle, oracle_selon, reecrire_oracles, tire};
use droit::clauses::{self as c, Bande, DitParLEngagement, Indemnite, PreavisDit, Prime, Profil, Retraite, Tranche, P};
use socle::date::Jour;
use socle::num::{Decimal, Resultat};
use serde_json::{json, Map, Value};
use std::str::FromStr;

const GRAINE: u64 = 20261011;

/// Un IDCC ou une annexe de l'oracle, en texte (vide : la case nulle ou vide).
fn lu(v: &Value) -> String {
    if socle::fichiers::vrai(v) { socle::date::en_texte(v).trim().to_string() } else { String::new() }
}

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

// ------------------------------------------------------- le JSON des resultats
fn texte(x: &Option<String>) -> Value {
    x.as_ref().map_or(Value::Null, |s| json!(s))
}

fn nombre(x: Option<P>) -> Value {
    x.map_or(Value::Null, |p| json!(p.to_string()))
}

fn en_valeur_profil(p: &Profil) -> Value {
    json!({"categorie": texte(&p.categorie), "grille": texte(&p.grille), "source": p.source})
}

fn en_valeur_preavis(p: &PreavisDit) -> Value {
    let mut v = json!({"mois": nombre(p.mois), "article": texte(&p.article), "pourquoi": texte(&p.pourquoi)});
    if let Some(j) = p.jours {
        v["jours"] = json!(j.to_string());
    }
    v
}

fn en_valeur_indemnite(i: &Indemnite) -> Value {
    if !i.releve {
        return json!({"releve": false, "du": false, "pourquoi": texte(&i.pourquoi)});
    }
    let lectures: Map<String, Value> = i.lectures.iter().map(|(k, v)| (k.clone(), json!(v.to_string()))).collect();
    json!({"releve": true, "du": i.du, "bas": nombre(i.bas), "haut": nombre(i.haut), "lectures": lectures,
        "hors_primes": i.hors_primes, "article": texte(&i.article), "pourquoi": texte(&i.pourquoi)})
}

/// `depart` : les cles du depart volontaire (`mois`, `palier`), sinon celles de la mise a la retraite.
fn en_valeur_retraite(r: &Retraite, depart: bool) -> Value {
    if !r.releve {
        return json!({"releve": false, "montant": null, "pourquoi": texte(&r.pourquoi)});
    }
    if r.montant.is_none() {
        return json!({"releve": true, "montant": null, "article": null, "pourquoi": texte(&r.pourquoi)});
    }
    if depart {
        json!({"releve": true, "montant": nombre(r.montant), "mois": nombre(r.mois), "palier": r.palier, "article": texte(&r.article),
            "prevenance_mois": r.prevenance_mois})
    } else {
        json!({"releve": true, "montant": nombre(r.montant), "article": texte(&r.article), "hors_primes": r.hors_primes,
            "prevenance_mois": r.prevenance_mois, "cadre_de_direction": texte(&r.cadre_de_direction)})
    }
}

fn en_valeur_prime(p: Option<Prime>) -> Value {
    p.map_or(Value::Null, |p| {
        json!({"pourcentage": p.pourcentage.to_string(), "base": nombre(p.base), "montant": null, "article": p.article,
            "pourquoi": texte(&p.pourquoi), "annees": p.annees})
    })
}

fn en_valeur_annexe(a: Option<c::ClauseDAnnexe>) -> Value {
    a.map_or(Value::Null, |a| {
        json!([a.article, {"lectures": a.lectures.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>(), "hors_primes": a.hors_primes,
            "anciennete_minimale_mois": a.anciennete_minimale_mois.map(|m| m.0.to_string()), "plafond_mois": a.plafond_mois.map(|m| m.0.to_string()),
            "motifs_legaux_seulement": a.motifs_legaux_seulement}])
    })
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

/// Un décimal écrit sans exposant, sans zéros de queue.
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

fn jour(v: &Value) -> Jour {
    Jour::lire(v.as_str().unwrap()).unwrap()
}

fn avec(base: &Value, k: &str, v: Value) -> Value {
    let mut m: Map<String, Value> = base.as_object().cloned().unwrap_or_default();
    m.insert(k.to_string(), v);
    Value::Object(m)
}

/// Ce qu'un engagement de la grille dit du salarie, lu comme le lit `engagement::solde` : une case vide ne dit rien,
/// un code emploi qui n'est pas un texte et un debut illisible refusent.
fn dit(e: Option<&Value>) -> Resultat<DitParLEngagement> {
    let Some(e) = e else { return Ok(DitParLEngagement::default()) };
    let dite = |k: &str| e.get(k).filter(|v| socle::fichiers::vrai(v));
    let metier = match dite("metier") {
        Some(v) => Some(v.as_str().ok_or("code emploi : pas un texte")?.to_string()),
        None => None,
    };
    let debut = match dite("debut") {
        Some(v) => {
            let s = socle::date::en_texte(v);
            Some(s.get(..10).filter(|j| j.as_bytes().get(4) == Some(&b'-') && j.as_bytes().get(7) == Some(&b'-')).and_then(|j| Jour::lire(j).ok()).ok_or("date illisible")?)
        }
        None => None,
    };
    Ok(DitParLEngagement {
        categorie_conventionnelle: dite("categorie_conventionnelle").map(socle::date::en_texte),
        statut: dite("statut").and_then(Value::as_str).map(str::to_string),
        metier,
        cadre: e.get("cadre").filter(|v| !v.is_null()).map(socle::fichiers::vrai),
        debut,
    })
}

/// Un profil de la grille (`{categorie, grille}`).
fn prof(v: &Value) -> Profil {
    Profil { categorie: v["categorie"].as_str().map(str::to_string), grille: v["grille"].as_str().map(str::to_string), source: String::new() }
}

fn profil_lu(idcc: &Value, fiche: &Value, eng: Option<&Value>) -> Resultat<Value> {
    let f = champs(fiche)?;
    c::profil(&lu(idcc), &f, &dit(eng)?).map(|p| en_valeur_profil(&p))
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
                    out.push(ligne(profil_lu(&p[0], &fiche, eng.as_ref())));
                }
            }
        }
        out
    });
    section(&rel, "profil", |a| profil_lu(&a[0], &a[1], option(&a[2])));
    section(&rel, "nom", |a| Ok(json!(c::nom(&lu(&a[0])))));
    section(&rel, "releve", |a| Ok(json!(c::releve(&lu(&a[0]))?.is_some())));
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
                out.push(ligne(c::preavis_mois(&lu(&p[0]), p[1].as_str(), dec(a), p[2].as_str(), dec_option(age)).map(|r| en_valeur_preavis(&r))));
            }
        }
        out
    });
    section(&rel, "preavis_mois", |a| {
        c::preavis_mois(&lu(&a[0]), a[1].as_str(), dec(&a[2]), a[3].as_str(), dec_option(&a[4])).map(|r| en_valeur_preavis(&r))
    });
}

/// Les baremes : chaque liste de la grille lue comme des tranches (en annees revolues ou non, et comme elle le dit) ou
/// comme des bandes, selon sa forme ; les paliers de retraite de chaque section relevee.
#[test]
fn les_baremes() {
    let rel = lire();
    let g = &rel["grilles"];
    let annees: Vec<P> = g["annees"].as_array().unwrap().iter().map(|a| P::depuis(dec(a))).collect();
    let mut lecture = Vec::new();
    for l in chaines(&g["listes"]) {
        let d = |x: P| ligne(Ok(Value::String(x.to_string())));
        if let Ok(taux) = socle::forme::lire::<Vec<Tranche>>(&l[1], "") {
            let lignes: Vec<String> = annees.iter().flat_map(|a| [c::par_tranches(&taux, *a, true), c::par_tranches(&taux, *a, false), c::Bareme { motifs: vec![], taux: Some(taux.clone()), bandes: None }.mois(*a).unwrap()]).map(d).collect();
            lecture.push(json!([l[0], "tranches", fnv(&lignes.join("\n"))]));
        }
        if let Ok(bandes) = socle::forme::lire::<Vec<Bande>>(&l[1], "") {
            let lignes: Vec<String> = annees.iter().map(|a| d(c::par_bandes(&bandes, *a))).collect();
            lecture.push(json!([l[0], "bandes", fnv(&lignes.join("\n"))]));
        }
    }
    oracle("clauses.json", "/blocs/lecture", &Value::Array(lecture));
    let t = c::table().unwrap();
    let chemins: Vec<Value> = chaines(&g["paliers"]).iter().map(|s| s[0].clone()).collect();
    let paliers: Vec<Value> = chemins
        .iter()
        .map(|s| {
            let conv = t.convention(s[0].as_str().unwrap()).unwrap();
            let (paliers, prorata) = match s[1].as_str().unwrap() {
                "mise_a_la_retraite" => conv.mise_a_la_retraite.as_ref().map(|m| (&m.paliers, m.prorata.as_ref())).unwrap(),
                _ => conv.depart_a_la_retraite.as_ref().map(|m| (&m.paliers, m.prorata.as_ref())).unwrap(),
            };
            let lignes: Vec<String> = annees.iter().map(|a| ligne(Ok(c::paliers(paliers, prorata, *a)).map(|(m, p)| json!([m.to_string(), p])))).collect();
            json!([s, fnv(&lignes.join("\n"))])
        })
        .collect();
    oracle("clauses.json", "/blocs/paliers", &Value::Array(paliers));
    // La grille ne porte que le chemin de chaque section : la section se lit dans le fichier livre.
    oracle("clauses.json", "/grilles/paliers", &Value::Array(chemins.into_iter().map(|c| json!([c])).collect()));
}

#[test]
fn clause_d_annexe() {
    let rel = lire();
    let g = &rel["grilles"];
    let (profs, motifs) = (chaines(&g["profs"]), chaines(&g["motifs"]));
    blocs(&rel, "clause_d_annexe", 1, |p| {
        let mut out = Vec::new();
        for pr in &profs {
            for m in &motifs {
                out.push(ligne(c::clause_d_annexe(&lu(&p[0]), &lu(&p[1]), &prof(pr), m.as_str()).map(en_valeur_annexe)));
            }
        }
        out
    });
    section(&rel, "clause_d_annexe", |a| c::clause_d_annexe(&lu(&a[0]), &lu(&a[1]), &prof(&a[2]), a[3].as_str()).map(en_valeur_annexe));
}

#[test]
fn indemnite_de_licenciement() {
    let rel = lire();
    let g = &rel["grilles"];
    let (motifs, anc, refs, ages) =
        (chaines(&g["motifs_l"]), chaines(&g["anc_l"]), chaines(&g["refs_peu"]), chaines(&g["ages_l"]));
    let indemnite = |a: &[Value], anc: &Value, r: &Value, m: &Value, age: &Value| {
        c::indemnite_licenciement(&lu(&a[0]), &lu(&a[1]), &prof(&a[2]), dec(anc), dec(r), m.as_str(), dec_option(age)).map(|i| en_valeur_indemnite(&i))
    };
    blocs(&rel, "indemnite_licenciement", 8, |p| {
        let mut out = Vec::new();
        for m in &motifs {
            for a in &anc {
                for r in &refs {
                    for age in &ages {
                        out.push(ligne(indemnite(p, a, r, m, age)));
                    }
                }
            }
        }
        out
    });
    section(&rel, "indemnite_licenciement", |a| indemnite(a, &a[3], &a[4], &a[5], &a[6]));
}

#[test]
fn retraite() {
    let rel = lire();
    let g = &rel["grilles"];
    let (anc, refs) = (chaines(&g["anc"]), chaines(&g["refs"]));
    let mise = |idcc: &Value, a: &Value, r: &Value| c::indemnite_mise_a_la_retraite(&lu(idcc), dec(a), dec(r)).map(|x| en_valeur_retraite(&x, false));
    let depart = |idcc: &Value, a: &Value, r: &Value| c::indemnite_depart_retraite(&lu(idcc), dec(a), dec(r)).map(|x| en_valeur_retraite(&x, true));
    blocs(&rel, "indemnite_mise_a_la_retraite", 1, |p| anc.iter().flat_map(|a| refs.iter().map(move |r| ligne(mise(&p[0], a, r)))).collect());
    blocs(&rel, "indemnite_depart_retraite", 1, |p| anc.iter().flat_map(|a| refs.iter().map(move |r| ligne(depart(&p[0], a, r)))).collect());
    section(&rel, "indemnite_mise_a_la_retraite", |a| mise(&a[0], &a[1], &a[2]));
    section(&rel, "indemnite_depart_retraite", |a| depart(&a[0], &a[1], &a[2]));
}

#[test]
fn prime_d_anciennete() {
    let rel = lire();
    let g = &rel["grilles"];
    let (fiches, engs, jours, jauges) =
        (chaines(&g["fiches_p"]), chaines(&g["eng_p"]), chaines(&g["jours"]), chaines(&g["jauges"]));
    let jours: Vec<Jour> = jours.iter().map(jour).collect();
    let prime = |idcc: &Value, annexe: &Value, fiche: &Value, eng: Option<&Value>, j: &Jour, jg: Option<&str>| {
        let f = champs(fiche)?;
        c::prime_anciennete(&lu(idcc), &lu(annexe), &f, &dit(eng)?, j, jg).map(en_valeur_prime)
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

/// LA REMISE DU CONTRAT A DUREE DETERMINEE : « au plus tard, dans les deux jours ouvrables suivant l'embauche »
/// (code du travail, L. 1242-13, `atelier/reference/sources/code-du-travail/xml/L1242-13.xml`) ; pour l'artiste
/// musicien de la 1285, « au plus tard 15 jours ouvrables précédant l'engagement » (titre XV, `remise_du_contrat` de sa
/// couche, `atelier/reference/sources/chomage-intermittents/kali-1285-organisation-travail-repetitions-representations-
/// 20261020.md`, l. 763). Un jour ouvrable va du lundi au samedi.
#[test]
fn la_remise_du_contrat_suit_l1242_13_et_la_1285_pour_ses_musiciens() {
    let j = |s: &str| Jour::lire(s).unwrap();
    // Un vendredi : le samedi, puis le lundi (le dimanche n'est pas ouvrable).
    let loi = droit::echeances::remise_du_contrat(&j("2026-11-20"), "3090", "ART170", None).unwrap();
    assert_eq!(loi.jour, j("2026-11-23"));
    assert!(loi.source.contains("L. 1242-13"), "{}", loi.source);
    // Le meme musicien sous la 1285 : quinze jours ouvrables avant le jeudi 10/12, sans compter les dimanches.
    let musicien = droit::echeances::remise_du_contrat(&j("2026-12-10"), "1285", "ART170", None).unwrap();
    assert_eq!(musicien.jour, j("2026-11-23"));
    assert!(musicien.source.contains("1285") && musicien.source.contains("XV"), "{}", musicien.source);
    // Un metier hors du titre XV, sous la 1285 : la loi seule.
    let autre = droit::echeances::remise_du_contrat(&j("2026-12-10"), "1285", "ZZZ999", None).unwrap();
    assert_eq!(autre.jour, j("2026-12-12"));
    assert!(autre.source.contains("L. 1242-13"), "{}", autre.source);
}

/// La fiche du releve, lue par sa forme : celle que la forme refuse (un statut en nombre, une categorie hors de la liste)
/// ne parvient plus au profil.
fn champs(v: &Value) -> Resultat<droit::fiche::ChampsSalarie> {
    droit::fiche::lire_champs(v, "la fiche du releve")
}
