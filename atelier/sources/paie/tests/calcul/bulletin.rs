//! La description du bulletin contre `bulletin.json` : les formats, ce que `jeux_et_description` passe a la fabrique
//! (mois entier, une paie avec celles du mois, cumuls bornes, rectificatifs, refus), et la description de jeux fabriques
//! (prelevement ou plafond absents, conges, enseignement, logo). `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit l'oracle.

use crate::outils::{chemin_oracle, fictive, lire_oracle, oracle};
use droit::bareme::canon;
use paie::bulletin::{description, fmt_eur, fmt_heures, fmt_taux, jeux_et_description, jj_mm_aaaa, jours, remboursements, Jeu};
use droit::contexte::Contexte;
use droit::cumul::Colonne;
use engagement::conges::CompteurConges;
use socle::date::Jour;
use socle::num::{dec, Decimal};
use droit::pas::Versement;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn depot() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

fn attendus() -> Value {
    lire_oracle("bulletin.json")
}

fn racine_de(nom: &str) -> PathBuf {
    if nom == "cumul" {
        chemin_oracle("racines/cumul")
    } else {
        fictive(nom)
    }
}

/// La saisie du mois du cas, telle que ses engagements la montrent.
fn saisie_de(c: &Value) -> Value {
    let base = if c["racine"] == "cumul" { fictive("fictive-1285-grande") } else { racine_de(c["racine"].as_str().unwrap()) };
    crate::outils::saisie(&base, c["mois"].as_str().unwrap())
}

fn jour(s: &str) -> Jour {
    Jour::lire(s).unwrap()
}

/// Les chemins sous le depot s'ecrivent `@/...`.
fn relatif(v: &Value) -> Value {
    match v {
        // La copie figee des fictives se lit comme `fictives/`, ou les sorties attendues ont ete relevees.
        Value::String(s) if s.starts_with(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/donnees/fictives").to_str().unwrap()) => {
            Value::String(s.replacen(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/donnees/fictives").to_str().unwrap(), "@/fictives", 1))
        }
        Value::String(s) => match s.strip_prefix(depot().to_str().unwrap()) {
            Some(reste) => Value::String(format!("@{}", reste)),
            None => v.clone(),
        },
        Value::Array(a) => Value::Array(a.iter().map(relatif).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), relatif(x))).collect()),
        autre => autre.clone(),
    }
}

fn sortie(r: Result<Value, String>) -> Value {
    match r {
        Ok(v) => json!({"ok": relatif(&canon(&v))}),
        Err(e) => json!({"erreur": e}),
    }
}

#[test]
fn les_formats() {
    let mut f = attendus()["formats"].clone();
    let d = |s: &Value| dec(s).unwrap();
    for (cle, fonction) in [("eur", fmt_eur as fn(Decimal) -> String), ("taux", fmt_taux), ("heures", fmt_heures), ("jours", jours)] {
        assert!(f[cle].as_array().unwrap().len() > 10);
        for c in f[cle].as_array_mut().unwrap() {
            c[1] = json!(fonction(d(&c[0])));
        }
    }
    for c in f["dates"].as_array_mut().unwrap() {
        c[1] = json!(jj_mm_aaaa(&c[0]).unwrap());
    }
    oracle("bulletin.json", "/formats", &f);
}

#[test]
fn ce_que_jeux_et_description_passe_a_la_fabrique() {
    let mut cas = attendus()["ecrire"].clone();
    assert!(cas.as_array().unwrap().len() > 12);
    for c in cas.as_array_mut().unwrap() {
        let o = &c["options"];
        let ctx = Contexte::nouveau(&racine_de(o["racine"].as_str().unwrap()), Some(jour("2026-09-27"))).unwrap();
        let mut s = engagement::charger_donnees(&ctx, &saisie_de(o)).unwrap();
        let mut ent = ctx.entreprise_du_mois(&s.mois).unwrap();
        if o["licence"] == "liste" {
            ent["licence_spectacle"] = json!([{"depuis": "2026-01-01", "numero": "L-R-26-1"}]);
        }
        for i in o["sans_paie"].as_array().into_iter().flatten() {
            s.paies[i.as_u64().unwrap() as usize].engagement.lignes.clear();
        }
        let choisies: Vec<_> = match o["paies"].as_array() {
            None => s.paies.iter().filter(|p| !p.engagement.sans_paie()).cloned().collect(),
            Some(idx) => idx.iter().map(|i| s.paies[i.as_u64().unwrap() as usize].clone()).collect(),
        };
        let rects: HashMap<usize, Value> = o["rectificatifs"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(i, r)| (i.parse().unwrap(), r.clone()))
            .collect();
        let du_mois = (o["du_mois"] == true).then_some(s.paies.as_slice());
        c["attendu"] = sortie(jeux_et_description(&ctx, &choisies, &ent, &s.mois, du_mois, o["borne"] == true, &rects));
    }
    oracle("bulletin.json", "/ecrire", &cas);
}

fn colonne(c: &Value) -> Colonne {
    let entier = |k: &str| c[k].as_i64().unwrap();
    Colonne {
        titre: c["titre"].as_str().unwrap().to_string(),
        sous_titre: c["sous_titre"].as_str().unwrap().to_string(),
        cachets: entier("cachets"),
        heures: entier("heures"),
        heures_equivalentes: entier("heures_equivalentes"),
        brut: dec(&c["brut"]).unwrap(),
        reste: c["reste"].as_i64(),
        echeance: c["echeance"].as_str().map(jour),
        enseignement: c["enseignement"].as_i64().unwrap_or(0),
        heures_ouverture: c["heures_ouverture"].as_i64(),
    }
}

fn conges(c: &Value) -> CompteurConges {
    let zero = Decimal::ZERO;
    CompteurConges {
        salarie: String::new(),
        periode_debut: jour(c["periode_debut"].as_str().unwrap()),
        periode_fin: jour(c["periode_fin"].as_str().unwrap()),
        acquis: dec(&c["acquis"]).unwrap(),
        pris: dec(&c["pris"]).unwrap(),
        remuneration_periode: zero,
        reports: dec(&c["reports"]).unwrap(),
        avertissements: Vec::new(),
        report_debut: None,
        report_fin: None,
        acquis_report: zero,
        pris_sur_la_periode: zero,
    }
}

fn versement(v: &Value) -> Versement {
    Versement {
        taux: v["taux"].clone(),
        type_taux: Value::Null,
        identifiant: Value::Null,
        abattement: Decimal::ZERO,
        montant_soumis: Decimal::ZERO,
        prelevement: dec(&v["prelevement"]).unwrap(),
    }
}

#[test]
fn la_description_de_jeux_fabriques() {
    let mut cas = attendus()["description"].clone();
    assert!(cas.as_array().unwrap().len() > 12);
    for c in cas.as_array_mut().unwrap() {
        let o = &c["options"].clone();
        let ctx = Contexte::nouveau(&racine_de(o["racine"].as_str().unwrap()), Some(jour("2026-09-27"))).unwrap();
        let s = engagement::charger_donnees(&ctx, &saisie_de(o)).unwrap();
        let mut ent = ctx.entreprise_du_mois(&s.mois).unwrap();
        if let Some(g) = o["gabarits"].as_str() {
            ent["_gabarits"] = json!(chemin_oracle("racines/bulletin").join(g).to_str().unwrap());
        }
        let specs = o["jeux"].as_array().unwrap();
        let colonnes: Vec<Option<Vec<Colonne>>> =
            specs.iter().map(|j| j["colonnes"].as_array().map(|a| a.iter().map(colonne).collect())).collect();
        let compteurs: Vec<Option<CompteurConges>> =
            specs.iter().map(|j| (!j["conges"].is_null()).then(|| conges(&j["conges"]))).collect();
        let pas: Vec<Option<Versement>> =
            specs.iter().map(|j| (!j["pas"].is_null()).then(|| versement(&j["pas"]))).collect();
        let jeux: Vec<Jeu> = specs
            .iter()
            .enumerate()
            .map(|(i, j)| Jeu {
                paie: &s.paies[j["paie"].as_u64().unwrap() as usize],
                colonnes: colonnes[i].as_deref(),
                plafond: (!j["plafond"].is_null()).then(|| dec(&j["plafond"]).unwrap()),
                conges: compteurs[i].as_ref(),
                pas: pas[i].as_ref(),
                rectificatif: (!j["rectificatif"].is_null()).then(|| &j["rectificatif"]),
                remboursements: remboursements(&ctx, &s.paies[j["paie"].as_u64().unwrap() as usize]).unwrap(),
            })
            .collect();
        let attendu = sortie(description(&jeux, &ent));
        c["attendu"] = attendu;
    }
    oracle("bulletin.json", "/description", &cas);
}
