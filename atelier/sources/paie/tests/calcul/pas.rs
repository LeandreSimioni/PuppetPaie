//! Le prelevement a la source contre `pas.json` : la grille de l'article 204 H (`taux_par_defaut`, `du_versement`,
//! l'abattement, de 0 a 10 000 EUR, de part et d'autre des changements de bareme) et `oracle pas` sur les fictives et sur la
//! racine `racines/pas`. `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit l'oracle.

use crate::outils::{chemin_oracle, code_livre, fictive, lire_oracle, oracle};
use droit::bareme::canon;
use droit::contexte::Contexte;
use socle::date::Jour;
use socle::num::dec;
use droit::pas::{abattement, du_versement, taux_par_defaut, ErreurPas, Triplet, Versement};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;

fn attendus() -> Value {
    lire_oracle("pas.json")
}

fn jour(s: &str) -> Option<Jour> {
    if s == "None" {
        None
    } else {
        Some(Jour::lire(s).unwrap())
    }
}

/// Une ligne de resultats, un par montant de la grille : `*x` quand ils sont tous x.
fn ligne(r: Vec<String>) -> Value {
    Value::String(if r.iter().all(|x| *x == r[0]) { format!("*{}", r[0]) } else { r.join(";") })
}

fn montants(a: &Value) -> Vec<socle::num::Decimal> {
    a["grille"]["montants"].as_array().unwrap().iter().map(|m| dec(m).unwrap()).collect()
}

fn erreur(e: ErreurPas) -> String {
    match e {
        ErreurPas::SansGrille(m) => format!("SANS_GRILLE:{}", m),
        autre => format!("ERREUR:{}", autre.message()),
    }
}

fn texte(v: &Value) -> String {
    match canon(v) {
        Value::Null => "None".to_string(),
        Value::String(s) => s,
        autre => autre.to_string(),
    }
}

fn ligne_versement(r: Result<Versement, ErreurPas>) -> String {
    match r {
        Err(e) => erreur(e),
        Ok(v) => {
            let c = v.canon();
            ["taux", "type", "identifiant", "abattement", "montant_soumis", "prelevement"]
                .iter()
                .map(|k| texte(&c[*k]))
                .collect::<Vec<_>>()
                .join("|")
        }
    }
}

fn baremes() -> HashMap<&'static str, Value> {
    let socle = |an: i32| crate::outils::lire(&code_livre(), &format!("baremes/{}.json", an));
    HashMap::from([
        ("brut-2025", socle(2025)),
        ("brut-2026", socle(2026)),
        ("vide", json!({})),
        ("sans-grille", json!({"pas_taux_par_defaut": {"grilles": []}})),
    ])
}

fn triplet(v: &Value) -> Option<Triplet> {
    v.as_array().map(|a| (a[0].clone(), a[1].clone(), a[2].clone()))
}

#[test]
fn l_abattement_suit_la_date() {
    let b = baremes();
    let mut cas = attendus()["grille"]["abattement"].clone();
    for (cle, v) in cas.as_object_mut().unwrap() {
        let p: Vec<&str> = cle.split('|').collect();
        *v = Value::String(abattement(&b[p[0]], jour(p[1])).map_or_else(erreur, socle::num::nombre));
    }
    assert!(cas.as_object().unwrap().len() > 50);
    oracle("pas.json", "/grille/abattement", &cas);
}

#[test]
fn le_taux_par_defaut_de_zero_a_dix_mille_euros() {
    let (a, b) = (attendus(), baremes());
    let mut cas = a["grille"]["taux_par_defaut"].clone();
    for (cle, v) in cas.as_object_mut().unwrap() {
        let p: Vec<&str> = cle.split('|').collect();
        *v = ligne(montants(&a).into_iter().map(|m| taux_par_defaut(&b[p[0]], m, jour(p[1]), p[2]).map_or_else(erreur, |(t, ty)| format!("{},{}", t, ty))).collect());
    }
    assert!(cas.as_object().unwrap().len() * montants(&a).len() > 8000);
    oracle("pas.json", "/grille/taux_par_defaut", &cas);
}

#[test]
fn le_prelevement_d_un_versement() {
    let (a, b) = (attendus(), baremes());
    let triplets = a["grille"]["triplets"].as_array().unwrap();
    let mut cas = a["grille"]["du_versement"].clone();
    for (cle, v) in cas.as_object_mut().unwrap() {
        let p: Vec<&str> = cle.split('|').collect();
        let t = triplet(&triplets[p[4].parse::<usize>().unwrap()]);
        *v = ligne(montants(&a).into_iter().map(|m| ligne_versement(du_versement(t.as_ref(), &b[p[0]], m, jour(p[1]), p[3] == "True", p[2]))).collect());
    }
    assert!(cas.as_object().unwrap().len() > 40);
    oracle("pas.json", "/grille/du_versement", &cas);
}

#[test]
fn le_bareme_resolu_par_le_contexte() {
    let a = attendus();
    let ctx = Contexte::nouveau(&fictive("fictive-1285-grande"), Some(Jour::lire("2026-09-27").unwrap())).unwrap();
    let mut cas = a["grille"]["resolus"].clone();
    for (cle, v) in cas.as_object_mut().unwrap() {
        let p: Vec<&str> = cle.split('|').collect();
        let le = jour(p[0]).unwrap();
        let bareme = ctx.bareme(&Jour { jour: 1, ..le }).unwrap();
        *v = ligne(montants(&a).into_iter().map(|m| ligne_versement(du_versement(None, &bareme, m, Some(le), p[1] == "True", "metropole"))).collect());
    }
    oracle("pas.json", "/grille/resolus", &cas);
}

fn racine_de(nom: &str) -> PathBuf {
    if nom == "pas" {
        chemin_oracle("racines/pas")
    } else {
        fictive(nom)
    }
}

#[test]
fn oracle_pas_sur_les_fictives_et_la_racine_montee() {
    let mut cas = attendus()["oracle"].clone();
    for c in cas.as_array_mut().unwrap() {
        let mut o: HashMap<String, String> =
            c["options"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())).collect();
        let fichier = fictive(&o["fictive"]).join("paies").join(&o["fichier"]);
        o.insert("racine".into(), racine_de(&o["racine"]).to_string_lossy().into_owned());
        c["attendu"] = paie::oracle::pas_cmd(&o, fichier.to_str().unwrap()).map_or_else(|e| json!({"erreur": e}), |v| json!({"ok": canon(&v)}));
    }
    assert!(cas.as_array().unwrap().len() > 10);
    oracle("pas.json", "/oracle", &cas);
}

// ------------------------------------------------------------ le PAS d'un mois depose (lot D2)

/// (NIR, numero de contrat) -> (S21.G00.50.006, S21.G00.50.009) des versements d'une DSN deposee, relus ici.
fn pas_de_la_dsn(chemin: &std::path::Path) -> HashMap<(String, String), (String, String)> {
    let texte: String = std::fs::read(chemin).unwrap().iter().map(|&b| b as char).collect();
    let mut out = HashMap::new();
    let (mut nir, mut taux, mut montant) = (String::new(), String::new(), String::new());
    for l in texte.lines() {
        let Some((k, v)) = l.trim_end_matches('\r').split_once(',') else { continue };
        let v = v.trim_matches('\'').to_string();
        match k {
            "S21.G00.30.001" => nir = v,
            "S21.G00.50.006" => taux = v,
            "S21.G00.50.009" => montant = v,
            "S21.G00.51.010" => {
                out.insert((nir.clone(), v), (taux.clone(), montant.clone()));
            }
            _ => {}
        }
    }
    out
}

/// Tous les taux du magasin (`config/pas.json`) portes a 33,30 %, valables au jour du contexte.
fn changer_les_taux(racine: &std::path::Path) {
    let mut m = crate::outils::lire(racine, "config/pas.json");
    for e in m["salaries"].as_object_mut().unwrap().values_mut() {
        e["taux"] = json!("33.30");
    }
    crate::outils::ecrire(racine, "config/pas.json", &m);
}

/// (taux, prelevement) de chaque paie qui en a un.
fn pas_des_paies(racine: &std::path::Path, mois: &str) -> Vec<(String, String, String)> {
    let ctx = Contexte::nouveau(racine, Some(Jour::lire("2027-02-15").unwrap())).unwrap();
    let s = paie::engagement::charger(&ctx, &racine.join(format!("paies/{}.json", mois))).unwrap();
    let v = paie::prelevement::des_paies(&ctx, &s.paies).unwrap();
    s.paies
        .iter()
        .zip(v)
        .filter_map(|(p, v)| v.map(|v| (p.engagement.numero.clone().unwrap(), v.taux.as_str().unwrap().to_string(), format!("{:.2}", v.prelevement))))
        .collect()
}

#[test]
fn un_mois_depose_relu_apres_un_changement_de_taux_garde_son_pas() {
    // CGI art. 1671, 1 et 2 (atelier/reference/sources/code-general-des-impots/articles-20261013.md, l. 99 et 102) : la
    // retenue est faite au paiement, au taux transmis. Le mois depose a preleve ce que sa DSN dit ; relu plus tard,
    // apres un nouveau taux de la DGFiP, il le garde (regles 1 et 4). Jusqu'au lot D2 : recalcule au taux du jour.
    let mois = "2025-04";
    let depose = crate::outils::compagnie::config_et_mois("fictive-3090-petite", &[mois], &crate::outils::dossier_de_test("pas", "depose"));
    changer_les_taux(&depose);
    let lus = pas_des_paies(&depose, mois);
    assert!(!lus.is_empty());
    let ctx = Contexte::nouveau(&depose, Some(Jour::lire("2027-02-15").unwrap())).unwrap();
    let dsn = pas_de_la_dsn(&paie::casc::dsn_deposee(&ctx, mois, None).unwrap().expect("le mois est depose"));
    let s = paie::engagement::charger(&ctx, &depose.join(format!("paies/{}.json", mois))).unwrap();
    for (numero, taux, montant) in &lus {
        let p = s.paies.iter().find(|p| p.engagement.numero.as_ref() == Some(numero)).unwrap();
        let nir = p.salarie["nir"].as_str().unwrap().to_string();
        assert_eq!(dsn.get(&(nir, numero.clone())), Some(&(taux.clone(), montant.clone())), "le contrat {} : ce que la DSN deposee dit", numero);
        assert_ne!(taux, "33.30", "le contrat {} : pas le taux du jour", numero);
    }
    // Le meme mois, jamais depose : il se calcule, au taux du jour.
    let neuf = crate::outils::compagnie::saisies("fictive-3090-petite", &[mois], &crate::outils::dossier_de_test("pas", "neuf"));
    changer_les_taux(&neuf);
    assert!(pas_des_paies(&neuf, mois).iter().any(|(_, t, _)| t == "33.30"), "un mois non depose suit le taux du jour");
    let _ = std::fs::remove_dir_all(&depose);
    let _ = std::fs::remove_dir_all(&neuf);
}
