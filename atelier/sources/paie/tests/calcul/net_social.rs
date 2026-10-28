//! Le montant net social : ce que chaque rubrique livree hors assiette est pour lui (decret n° 2023-1378, CSS R. 844-1 II ;
//! C. trav. R. 3243-1, 9° bis ; FAQ du BOSS). Le moteur lit ce qu'est une ligne hors assiette dans la `nature` de sa
//! rubrique ; une autre nature se refuse (regle 1).

use droit::catalogue::{catalogue, HorsAssiette};

#[test]
fn chaque_rubrique_livree_hors_assiette_dit_ce_qu_elle_est_pour_le_net_social() {
    let cat = catalogue(&crate::outils::dossier_de_test("net_social", "catalogue"), &socle::date::Jour::lire("2026-10-28").unwrap()).unwrap();
    let mut vus: Vec<(String, HorsAssiette)> = Vec::new();
    for r in cat.iter().filter(|r| !r.soumis && r.origine == "base") {
        vus.push((r.code.clone(), r.hors_assiette().unwrap_or_else(|e| panic!("{} : {}", r.code, e))));
    }
    vus.sort_by(|a, b| a.0.cmp(&b.0));
    let attendu: Vec<(String, HorsAssiette)> = [
        ("acompte", HorsAssiette::Mouvement),
        ("defraiement_forfait", HorsAssiette::Frais),
        ("defraiement_reel", HorsAssiette::Frais),
        ("indemnite_licenciement", HorsAssiette::Revenu),
        ("indemnite_licenciement_conventionnelle", HorsAssiette::Revenu),
        ("indemnite_mise_a_la_retraite", HorsAssiette::Revenu),
        ("indemnite_mise_a_la_retraite_conventionnelle", HorsAssiette::Revenu),
        ("indemnite_rupture_conventionnelle", HorsAssiette::Revenu),
        ("indemnite_rupture_conventionnelle_imposable", HorsAssiette::Revenu),
        ("indemnite_rupture_conventionnelle_supplement", HorsAssiette::Revenu),
        ("indemnite_rupture_conventionnelle_supplement_imposable", HorsAssiette::Revenu),
    ]
    .iter()
    .map(|(c, h)| (c.to_string(), *h))
    .collect();
    assert_eq!(vus, attendu);
}
