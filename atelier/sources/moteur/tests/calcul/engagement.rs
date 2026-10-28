//! L'`Engagement` contre `engagement.json` : des engagements fabriques (refus compris) sur une compagnie de test vide.
//! `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit l'oracle.

use crate::outils::{dossier_de_test, lire_oracle, oracle};
use engagement::Engagement;
use serde_json::{json, Map, Value};
use socle::date::Jour;

fn iso(v: &[Jour]) -> Value {
    json!(v.iter().map(|j| j.iso()).collect::<Vec<_>>())
}

/// `{"ok": v}` ou `{"refus": message}`.
fn essai(r: Result<Value, String>) -> Value {
    match r {
        Ok(v) => json!({"ok": v}),
        Err(e) => json!({"refus": e}),
    }
}

/// La forme canonique d'un engagement et ce qu'il dit de lui-meme.
fn releve(e: &Engagement) -> Value {
    let jours = e.dates_du_contrat();
    let mut lieux: Vec<Jour> = jours.clone();
    lieux.push(e.debut);
    let par_lieu = |p: bool| {
        Value::Array(e.dates_par_lieu(p).into_iter().map(|(l, ds)| json!([l, iso(&ds)])).collect())
    };
    let nombre = |o: Option<f64>| match o {
        None => Ok(Value::Null),
        Some(x) => socle::num::dec_f64(x).map(|v| Value::String(socle::num::nombre(v))),
    };
    let mut extras = Map::new();
    extras.insert("dates_du_contrat".into(), essai(Ok(iso(&jours))));
    extras.insert("lieux_joues".into(), essai(Ok(json!(e.lieux_joues()))));
    extras.insert("dates_par_lieu".into(), essai(Ok(par_lieu(true))));
    extras.insert("dates_par_lieu_mois".into(), essai(Ok(par_lieu(false))));
    extras.insert("modalite_temps_travail".into(), essai(e.modalite_temps_travail().map(Value::String)));
    extras.insert("quotite_reference".into(), essai(e.quotite_reference().and_then(nombre)));
    extras.insert("a_cheval".into(), essai(e.a_cheval().map(Value::Bool)));
    extras.insert("planning_connu".into(), essai(Ok(Value::Bool(e.planning_connu()))));
    extras.insert(
        "lieu_de".into(),
        essai(Ok(json!(lieux.iter().map(|j| e.lieu_de(j)).collect::<Vec<_>>()))),
    );
    json!({"canon": essai(e.canon()), "extras": extras})
}

#[test]
fn les_engagements_fabriques_et_leurs_refus() {
    let mut directs = lire_oracle("engagement.json")["directs"].clone();
    let racine = dossier_de_test("engagement", "directs");
    let mut refus = 0;
    for c in directs.as_array_mut().unwrap() {
        c["attendu"] = Engagement::nouveau(&racine, &c["kwargs"]).map_or_else(|m| json!({"refus": m}), |e| releve(&e));
        refus += c["attendu"].get("refus").is_some() as usize;
    }
    // un refus par cas `refus_*`
    assert!(refus >= 30, "{}", refus);
    oracle("engagement.json", "/directs", &directs);
}
