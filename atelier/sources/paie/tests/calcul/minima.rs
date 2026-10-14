//! `minima::avertissements` et `minima::sous_le_smic` contre `minima.json` : les mois des demos, puis un balayage de
//! paies fabriquees (conventions, metiers, lignes, fiches) par empreinte, sur un echantillon a graine fixe (decision E)
//! et les cas lisibles ; et l'avertissement d'un cachet sous le minimum porte par `charger`.
//! `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit l'oracle.

use crate::outils::{demo, fnv, lire_oracle, oracle, reecrire_oracles, tire};
use paie::calcul::Paie;
use paie::contexte::Contexte;
use paie::date::Jour;
use paie::engagement::{charger, Avis};
use paie::minima;
use paie::num::Decimal;
use serde_json::{json, Map, Value};
use std::str::FromStr;

const GRAINE: u64 = 20261011;

fn attendus() -> Value {
    lire_oracle("minima.json")
}

fn contexte(racine: &str, date: &str) -> Contexte {
    Contexte::nouveau(&demo(racine), Some(Jour::lire(date).unwrap())).unwrap()
}

fn paies(ctx: &Contexte, fichier: &str) -> Vec<Paie> {
    charger(ctx, &demo(fichier)).unwrap().paies
}

fn en_json<T>(r: Result<Vec<Avis>, String>, f: impl Fn(&Avis) -> T) -> Value
where
    T: Into<Value>,
{
    match r {
        Ok(v) => Value::Array(v.iter().map(|a| f(a).into()).collect()),
        Err(e) => json!({ "err": e }),
    }
}

/// [avertissements, smic] comme le releveur les ecrit ; le SMIC se lit au
/// contexte d'origine (le bareme refuserait une entreprise fabriquee).
fn lire(ctx: &Contexte, p: &Paie, ctx_smic: &Contexte) -> Value {
    let a = en_json(minima::avertissements(ctx, p), |a| json!([a.texte, a.info]));
    let s = en_json(minima::sous_le_smic(ctx_smic, p), |a| Value::String(a.texte.clone()));
    json!([a, s])
}

#[test]
fn les_mois_des_demos() {
    let att = attendus();
    let date = att["date"].as_str().unwrap();
    let mois: Vec<Value> = att["mois"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| {
            let fichier = m[0].as_str().unwrap();
            let ctx = contexte(fichier.split('/').next().unwrap(), date);
            json!([fichier, paies(&ctx, fichier).iter().map(|p| lire(&ctx, p, &ctx)).collect::<Vec<_>>()])
        })
        .collect();
    oracle("minima.json", "/mois", &Value::Array(mois));
}

/// Aucun mois des demonstrations n'avertit : un cachet de 20,00 le fait, et `charger` le dit sur la paie.
#[test]
fn charger_porte_l_avertissement_d_un_cachet_sous_le_minimum() {
    let ctx = contexte("demo-3090-petite", "2026-10-02");
    let chemin = crate::outils::dossier_de_test("minima", "cachet-sous-le-minimum").join("2026-03.json");
    let cachet = json!({"rubrique": "cachet", "quantite": 1, "taux": 20.0, "lieu": "Scene fictive", "dates": ["2026-03-24"]});
    let e = json!({"salarie": "Brioche", "spectacle": "256Z00920011", "debut": "2026-03-24", "fin": "2026-03-24", "numero": "BR1.26.09", "lignes": [cachet]});
    std::fs::write(&chemin, json!({"mois": "2026-03", "date_paiement": "2026-03-31", "engagements": [e]}).to_string()).unwrap();
    let p = &charger(&ctx, &chemin).unwrap().paies[0];
    let attendus = minima::avertissements(&ctx, p).unwrap();
    assert!(!attendus.is_empty(), "un cachet de 20,00 est sous le minimum de la convention");
    for a in &attendus {
        assert!(p.avertissements.iter().any(|x| x.texte == a.texte), "{} manque a la paie chargee", a.texte);
    }
}

fn fusion(base: &Value, sur: &Value) -> Value {
    let mut m = base.as_object().cloned().unwrap_or_default();
    for (k, v) in sur.as_object().into_iter().flatten() {
        m.insert(k.clone(), v.clone());
    }
    Value::Object(m)
}

fn dec(v: &Value) -> Decimal {
    Decimal::from_str(v.as_str().unwrap()).unwrap()
}

/// `appliquer` du releveur : la paie et le contexte d'un cas.
fn appliquer(ctx: &Contexte, p: &Paie, pre: &Value, k: &[Value]) -> (Contexte, Paie) {
    let liste = |genre: &str, i: &Value| pre[genre][i.as_u64().unwrap() as usize].clone();
    let mut ctx2 = ctx.clone();
    ctx2.entreprise = fusion(&ctx.entreprise, &liste("ent", &k[1]));
    let mut p2 = p.clone();
    let mut sal = p.salarie.as_object().cloned().unwrap_or_default();
    sal.insert("code_emploi_ft".into(), k[6].clone());
    p2.salarie = fusion(&Value::Object(sal), &liste("sal", &k[2]));
    p2.spectacle = fusion(&p.spectacle, &liste("spec", &k[3]));
    let jour = |v: &Value| Jour::lire(v.as_str().unwrap()).unwrap();
    for (c, v) in liste("eng", &k[4]).as_object().unwrap() {
        match c.as_str() {
            "smic" => p2.smic = v.as_f64().unwrap(),
            "dates_travail" => p2.engagement.dates_travail = v.as_array().unwrap().iter().map(jour).collect(),
            "debut" => p2.engagement.debut = jour(v),
            "fin" => p2.engagement.fin = v.as_str().map(|_| jour(v)),
            "nature" => p2.engagement.nature = v.as_str().map(String::from),
            autre => panic!("preset inconnu : {autre}"),
        }
    }
    let lignes = liste("lignes", &k[5]);
    let modele = p.remuneration[0].clone();
    p2.remuneration = lignes
        .as_array()
        .unwrap()
        .iter()
        .map(|l| {
            let mut x = modele.clone();
            x.code = l["code"].as_str().unwrap().into();
            x.libelle = l["libelle"].as_str().unwrap().into();
            x.unite = l["unite"].as_str().unwrap().into();
            x.quantite = dec(&l["quantite"]);
            x.taux = dec(&l["taux"]);
            x.montant = dec(&l["montant"]);
            x.heures_annexes = dec(&l["heures_annexes"]);
            x
        })
        .collect();
    p2.engagement.lignes = lignes
        .as_array()
        .unwrap()
        .iter()
        .map(|l| {
            let mut m = Map::new();
            m.insert("rubrique".into(), l["code"].clone());
            m.insert("dates".into(), l["dates"].clone());
            Value::Object(m)
        })
        .collect();
    (ctx2, p2)
}

#[test]
fn le_balayage() {
    let att = attendus();
    let date = att["date"].as_str().unwrap();
    let bases: Vec<(Contexte, Paie)> = att["bases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| {
            let fichier = b[0].as_str().unwrap();
            let ctx = contexte(fichier.split('/').next().unwrap(), date);
            let p = paies(&ctx, fichier).remove(b[1].as_u64().unwrap() as usize);
            (ctx, p)
        })
        .collect();
    let textes = att["textes"].as_object().unwrap();
    let (mut cas, mut lisibles, mut non_vides) = (Vec::new(), Map::new(), 0);
    for (i, k) in att["cas"].as_array().unwrap().iter().enumerate() {
        let lisible = textes.contains_key(&i.to_string());
        if reecrire_oracles() && !lisible && !tire(GRAINE, k, 20) {
            continue;
        }
        let mut k = k.as_array().unwrap().clone();
        let (origine, p) = &bases[k[0].as_u64().unwrap() as usize];
        let (ctx, p) = appliquer(origine, p, &att["presets"], &k);
        let r = lire(&ctx, &p, origine);
        non_vides += (r != json!([[], []])) as u64;
        k[7] = Value::String(fnv(&serde_json::to_string(&r).unwrap()));
        if lisible {
            lisibles.insert(cas.len().to_string(), r);
        }
        cas.push(Value::Array(k));
    }
    assert!(non_vides > 1000, "le balayage ne dit presque rien : {}", non_vides);
    oracle("minima.json", "/cas", &Value::Array(cas));
    oracle("minima.json", "/textes", &Value::Object(lisibles));
    oracle("minima.json", "/non_vides", &json!(non_vides));
}
