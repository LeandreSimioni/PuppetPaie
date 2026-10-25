//! Le prelevement a la source d'un mois depose : il se relit dans sa DSN, il ne se recalcule pas.

use droit::contexte::Contexte;
use serde_json::json;
use socle::date::Jour;
use std::collections::HashMap;

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
    let s = engagement::charger(&ctx, &racine.join(format!("paies/{}.json", mois))).unwrap();
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
    let s = engagement::charger(&ctx, &depose.join(format!("paies/{}.json", mois))).unwrap();
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
