//! Les pieces de `calculer` contre `calcul-pieces.json` : les lignes de remuneration de chaque engagement des demos et
//! leur designation, `_code_54`, la `Paie` fabriquee, la reduction generale et le seuil de SMIC sur une grille (par
//! empreinte, en entier pour quelques groupes). Les refus sont des attendus. `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit
//! l'oracle. La deduction forfaitaire specifique est prouvee par `dfs.rs` ; ici, une fiche sans profession.

use crate::outils::{dec, demo, lire_oracle, oracle, sceller};
use paie::calcul::{au_dela_du_seuil, code_54, deduction_forfaitaire, lignes_brut, reduction_generale, LigneBrut, LigneCotisation, Paie};
use paie::catalogue::rubrique;
use paie::contexte::Contexte;
use paie::date::d;
use paie::engagement::{canon_valeur, engagements_du_fichier};
use paie::num::Decimal;
use serde_json::{json, Map, Value};
use std::cell::Cell;
use std::path::Path;

const ORACLE: &str = "calcul-pieces.json";

fn contexte(racine: &Path) -> Contexte {
    Contexte::nouveau(racine, Some(d(&json!("2026-09-27")).unwrap())).unwrap()
}

/// `{"ok": canon(v)}` ou `{"refus": message}`.
fn essai(r: Result<Value, String>) -> Value {
    match r {
        Ok(v) => json!({"ok": canon_valeur(&v).unwrap()}),
        Err(e) => json!({"refus": e}),
    }
}

// ------------------------------------------------------------------- les mois
fn paie_fabriquee(ctx: &Contexte, e: &paie::engagement::Engagement, lignes: &[LigneBrut], a: &Value) -> Result<Value, String> {
    let brut = dec(a["brut"].as_str().unwrap());
    let spectacle = match &e.spectacle {
        Some(s) => json!({"titre": s}),
        None => json!({}),
    };
    let mut p = Paie::nouveau(
        e.clone(), ctx.salarie(&e.salarie)?, spectacle, brut, brut, dec("4005.00"), 12.02, dec("7"));
    p.remuneration = lignes.to_vec();
    p.net_a_payer = dec(a["net"].as_str().unwrap());
    p.cout_employeur = dec(a["cout"].as_str().unwrap());
    p.lignes = vec![
        LigneCotisation {
            id: "maladie".into(), famille: "SANTE".into(), libelle: "Maladie".into(), base: brut,
            taux_pat: Some(7.0), montant_pat: dec("1.50"), taux_sal: None, montant_sal: dec("0.00"),
            masquer_base_et_taux: false,
        },
        LigneCotisation {
            id: "vieil_plaf".into(), famille: "RETRAITE".into(), libelle: "Vieillesse".into(), base: brut,
            taux_pat: Some(8.55), montant_pat: dec("2.00"), taux_sal: Some(6.9), montant_sal: dec("3.00"),
            masquer_base_et_taux: true,
        },
    ];
    let ligne = |id: &str| -> Result<Value, String> {
        p.ligne(id).map_or(Ok(Value::Null), |l| l.canon())
    };
    let mut m = Map::new();
    m.insert("titre".into(), essai(p.titre().map(Value::String)));
    m.insert("resume".into(), essai(p.resume().map(Value::String)));
    m.insert("ligne_maladie".into(), essai(ligne("maladie")));
    m.insert("ligne_vieil".into(), essai(ligne("vieil_plaf")));
    m.insert("ligne_absente".into(), essai(ligne("absente")));
    Ok(Value::Object(m))
}

#[test]
fn les_lignes_de_remuneration_et_la_paie_fabriquee() {
    let mut mois = lire_oracle(ORACLE)["mois"].clone();
    // Les lignes de permanent sont une entree de `engagement.json` (celles que `conges::lignes_du_mois` rendait).
    let eng = lire_oracle("engagement.json");
    let (mut n, mut lignes_n) = (0, 0);
    for c in mois.as_array_mut().unwrap() {
        let (nom, fichier) = (c["racine"].as_str().unwrap().to_string(), c["fichier"].as_str().unwrap().to_string());
        let racine = demo(&nom);
        let ctx = contexte(&racine);
        let permanents = eng["mois"].as_array().unwrap().iter().find(|m| m["racine"] == nom.as_str() && m["fichier"] == fichier.as_str()).unwrap()["permanents"].as_array().unwrap();
        let appel = Cell::new(0);
        let lignes = |_: &Contexte, _: &Value, _: &str| {
            let i = appel.get();
            appel.set(i + 1);
            Ok((permanents[i].as_array().unwrap().clone(), Vec::new()))
        };
        let (_, engs, _) = engagements_du_fichier(&ctx, &racine.join("paies").join(&fichier), &lignes).unwrap();
        let attendus = c["engagements"].as_array().unwrap().clone();
        let vus: Vec<Value> = engs
            .iter()
            .zip(attendus.iter().map(Some).chain(std::iter::repeat(None)))
            .map(|(e, a)| {
                n += 1;
                let lb = match lignes_brut(e, &ctx) {
                    Ok(l) => l,
                    Err(m) => return json!({"refus": m}),
                };
                lignes_n += lb.len();
                // La Paie fabriquee : ses entrees (brut, net, cout) sont dans l'oracle.
                let entrees = a.map_or(Value::Null, |a| a["paie"]["ok"].clone());
                let paie = match paie_fabriquee(&ctx, e, &lb, &entrees) {
                    Ok(Value::Object(mut m)) => {
                        for k in ["brut", "net", "cout"] {
                            m.insert(k.into(), entrees[k].clone());
                        }
                        json!({"ok": m})
                    }
                    Ok(_) => unreachable!(),
                    Err(m) => json!({"refus": m}),
                };
                json!({
                    "lignes": canon_valeur(&Value::Array(lb.iter().map(|l| l.canon()).collect())).unwrap(),
                    "designations": lb.iter().map(|l| l.designation()).collect::<Vec<_>>(),
                    "paie": paie,
                })
            })
            .collect();
        c["engagements"] = Value::Array(vus);
    }
    assert!(n >= 300 && lignes_n >= 300, "{} {}", n, lignes_n);
    oracle(ORACLE, "/mois", &mois);
}

#[test]
fn les_designations() {
    let mut cas = lire_oracle(ORACLE)["designations"].clone();
    for c in cas.as_array_mut().unwrap() {
        let (unite, libelle, q) = (c[0].as_str().unwrap(), c[1].as_str().unwrap(), c[2].as_str().unwrap());
        let l = LigneBrut {
            code: "x".into(), libelle: libelle.into(), unite: unite.into(), quantite: dec(q),
            taux: Decimal::ONE, montant: Decimal::ONE, heures_annexes: Decimal::ZERO, soumis: true,
            imposable: true, conges_spectacles: true, salaire_de_base: false, retabli: false,
            conges_pris: false, dsn_bloc52: None, arret_de_travail: false, dsn_autres_bruts: None,
        };
        c[3] = json!(l.designation());
    }
    oracle(ORACLE, "/designations", &cas);
}

#[test]
fn le_code_du_bloc_54() {
    let mut cas = lire_oracle(ORACLE)["code_54"].clone();
    let racine = demo("demo-1285-grande");
    let mut refus = 0;
    for c in cas.as_array_mut().unwrap() {
        let r = rubrique(&racine, c[0].as_str().unwrap()).unwrap();
        let vu = code_54(&r, &c[1]);
        refus += vu.is_err() as usize;
        c[2] = essai(vu.map(|o| o.map_or(Value::Null, Value::String)));
    }
    assert!(refus > 10, "{}", refus);
    oracle(ORACLE, "/code_54", &cas);
}

// ----------------------------------------------------------------- la grille
/// Un groupe de la grille : l'empreinte de ses resultats, et ses resultats entiers quand l'oracle les garde.
fn groupe(g: &mut Value, vus: Vec<Value>) {
    let vus = Value::Array(vus);
    g["empreinte"] = Value::String(sceller(&vus));
    if g.get("attendu").is_some() {
        g["attendu"] = vus;
    }
}

#[test]
fn la_reduction_generale() {
    let rel = lire_oracle(ORACLE);
    let baremes = rel["baremes"].as_array().unwrap();
    let mut rg = rel["rg"].clone();
    let (mut points, mut appliquees) = (0, 0);
    for g in rg.as_array_mut().unwrap() {
        let cle = g["grille"].as_str().unwrap().to_string();
        let (rang, h) = cle.split_once('|').unwrap();
        let bar = &baremes[rang.parse::<usize>().unwrap()];
        let (statut, caisse) = (g["statut"].as_str().map(String::from), g["caisse"].as_bool().unwrap());
        let mut vus = Vec::new();
        for b in rel["grilles"][&cle].as_array().unwrap() {
            points += 1;
            let r = reduction_generale(bar, statut.as_deref(), dec(b.as_str().unwrap()), dec(h), caisse);
            appliquees += matches!(r, Ok(Some(_))) as usize;
            vus.push(essai(r.map(|o| o.map_or(Value::Null, |x| x.canon()))));
        }
        groupe(g, vus);
    }
    assert!(points > 30000 && appliquees > 5000, "{} {}", points, appliquees);
    oracle(ORACLE, "/rg", &rg);
}

#[test]
fn le_seuil_de_smic() {
    let rel = lire_oracle(ORACLE);
    let baremes = rel["baremes"].as_array().unwrap();
    let mut seuil = rel["seuil"].clone();
    let mut points = 0;
    for g in seuil.as_array_mut().unwrap() {
        let cle = g["grille"].as_str().unwrap().to_string();
        let (rang, h) = cle.split_once('|').unwrap();
        let bar = &baremes[rang.parse::<usize>().unwrap()];
        let mut vus = Vec::new();
        for b in rel["grilles"][&cle].as_array().unwrap() {
            points += 1;
            vus.push(essai(au_dela_du_seuil(bar, dec(b.as_str().unwrap()), dec(h), Some(&g["seuil"])).map(Value::Bool)));
        }
        groupe(g, vus);
    }
    assert!(points > 30000, "{}", points);
    oracle(ORACLE, "/seuil", &seuil);
}

/// Une fiche sans deduction forfaitaire n'en a pas ; une fiche qui dit une categorie (« 20 », « 25 ») et non une
/// PROFESSION refuse, quelle que soit la section du bareme (arrete du 4 septembre 2025, art. 9). La regle : `dfs.rs`.
#[test]
fn la_deduction_forfaitaire_d_une_fiche_sans_profession() {
    let rel = lire_oracle(ORACLE);
    let ctx = contexte(&demo("demo-1285-grande"));
    for bar in [&rel["baremes"][0], &rel["baremes"][42]] {
        assert!(deduction_forfaitaire(bar, &ctx.salarie("NADAR").unwrap(), &Value::Null).unwrap().is_none());
        assert!(deduction_forfaitaire(bar, &json!({}), &Value::Null).unwrap().is_none());
        for cat in ["0", "20", "25", "30"] {
            let e = deduction_forfaitaire(bar, &json!({"deduction_forfaitaire": cat}), &Value::Null).unwrap_err();
            assert!(e.contains("PROFESSION"), "{} : {}", cat, e);
        }
    }
}
