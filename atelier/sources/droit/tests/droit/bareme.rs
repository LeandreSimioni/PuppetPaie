//! La pile de bareme (`bareme_applique`) contre `bareme.json` : les salaries des quatre compagnies fictives x trois dates x
//! chaque nature x cadre oui / non / defaut, et des cas retouches pour les branches que les fictives ne couvrent pas
//! (Alsace-Moselle, sante conventionnelle, prevoyance de branche, refus). Douze baremes en entier, les autres par
//! empreinte (cle par cle, element par element pour les listes) ; les refus sont des attendus.
//! `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit l'oracle.

use crate::outils::{fictive, lire_oracle, oracle, sceller};
use droit::bareme::{bareme_applique, canon};
use droit::contexte::Contexte;
use droit::natures::nature;
use socle::date::Jour;
use droit::emplois::{emploi_de, EmploiEngagement};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

fn empreinte(bareme: &Value) -> Value {
    let mut cles: Vec<&String> = bareme.as_object().unwrap().keys().collect();
    cles.sort();
    let mut m = Map::new();
    for k in cles {
        let v = &bareme[k];
        let e = match v {
            Value::Array(a) => Value::Array(a.iter().map(|x| Value::String(sceller(x))).collect()),
            autre => Value::String(sceller(autre)),
        };
        m.insert(k.clone(), e);
    }
    Value::Object(m)
}

/// Une retouche : le chemin se suit par cles et par rangs ; `{"$supprimer": true}` retire la cle.
fn retoucher(v: &mut Value, chemin: &[Value], valeur: &Value) {
    let (dernier, debut) = chemin.split_last().unwrap();
    let mut cur = v;
    for c in debut {
        cur = match c {
            Value::Number(n) => &mut cur[n.as_u64().unwrap() as usize],
            Value::String(s) => {
                let o = cur.as_object_mut().unwrap();
                o.entry(s.clone()).or_insert_with(|| Value::Object(Map::new()))
            }
            _ => panic!("chemin illisible"),
        };
    }
    let supprimer = valeur.get("$supprimer").is_some();
    match (dernier, cur) {
        (Value::Number(n), Value::Array(a)) => {
            let i = n.as_u64().unwrap() as usize;
            if supprimer {
                a.remove(i);
            } else {
                a[i] = valeur.clone();
            }
        }
        (Value::String(s), Value::Object(o)) => {
            if supprimer {
                o.shift_remove(s);
            } else {
                o.insert(s.clone(), valeur.clone());
            }
        }
        _ => panic!("retouche impossible"),
    }
}

#[test]
fn la_pile_de_bareme() {
    let rel = lire_oracle("bareme.json");
    let mut cas = rel["cas"].clone();
    assert!(cas.as_array().unwrap().len() > 3000);
    let anciennes = rel["sorties"].as_array().unwrap();
    // Une sortie par resultat distinct : la place de l'ancienne quand le resultat est le meme pour tous ses cas.
    let mut sorties: Vec<Option<Value>> = vec![None; anciennes.len()];
    let mut contextes: HashMap<(String, String), (Contexte, Value)> = HashMap::new();
    let (mut ok, mut refus) = (0, 0);
    for c in cas.as_array_mut().unwrap() {
        let racine = c["racine"].as_str().unwrap().to_string();
        let date = c["date"].as_str().unwrap().to_string();
        let jour = Jour::lire(&date).unwrap();
        let (ctx, bar) = contextes.entry((racine.clone(), date.clone())).or_insert_with(|| {
            let ctx = Contexte::nouveau(&fictive(&racine), Some(jour)).unwrap();
            let bar = ctx.bareme(&jour).unwrap();
            (ctx, bar)
        });
        // La fiche, le cadre force, l'emploi a la date.
        let mut sal = ctx.salarie(c["salarie"].as_str().unwrap()).unwrap();
        if let Some(cadre) = c["cadre"].as_str() {
            sal["statut_categoriel_rc"] = Value::String(if cadre == "oui" { "01" } else { "04" }.into());
        }
        let mut sal = emploi_de(&sal, &EmploiEngagement::default()).unwrap();
        let nat = nature(c["nature"].as_str()).unwrap();
        let mut bar = bar.clone();
        for r in c["retouches"].as_array().unwrap() {
            let cible = if r[0] == "bar" { &mut bar } else { &mut sal };
            retoucher(cible, r[1].as_array().unwrap(), &r[2]);
        }
        let fin = match c.get("fin") {
            None => Some(jour),
            Some(Value::Null) => None,
            Some(f) => Some(Jour::lire(f.as_str().unwrap()).unwrap()),
        };
        let rang = c["sortie"].as_u64().unwrap() as usize;
        let sortie = match bareme_applique(bar, &sal, &nat, fin.as_ref()) {
            Err(e) => {
                refus += 1;
                json!({"ok": false, "erreur": e})
            }
            Ok(b) => {
                ok += 1;
                let b = canon(&b);
                let mut s = json!({"ok": true, "empreinte": empreinte(&b)});
                if anciennes.get(rang).is_some_and(|a| a.get("bareme").is_some()) {
                    s["bareme"] = b;
                }
                s
            }
        };
        let place = match sorties.get(rang) {
            Some(None) => Some(rang),
            Some(Some(x)) if *x == sortie => Some(rang),
            _ => sorties.iter().position(|x| x.as_ref() == Some(&sortie)),
        };
        let place = place.unwrap_or_else(|| {
            sorties.push(None);
            sorties.len() - 1
        });
        sorties[place] = Some(sortie);
        c["sortie"] = json!(place);
    }
    assert!(ok > 2000 && refus > 500, "{} {}", ok, refus);
    let sorties: Vec<Value> = sorties.into_iter().enumerate().map(|(i, s)| s.unwrap_or_else(|| anciennes[i].clone())).collect();
    oracle("bareme.json", "/sorties", &Value::Array(sorties));
    oracle("bareme.json", "/cas", &cas);
}
