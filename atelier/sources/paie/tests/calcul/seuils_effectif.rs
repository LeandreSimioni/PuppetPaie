//! Le seuil de onze salaries de la contribution a la formation professionnelle des permanents, dans les deux sens.
//!
//! Textes (releve LEGI, atelier/reference/sources/code-du-travail/articles-20261013.md et
//! code-de-la-securite-sociale/articles-20261013.md) : C. trav. L. 6331-1 (« L'employeur de moins de onze salaries […]
//! 0,55 % »), L. 6331-3 (« d'au moins onze salaries […] 1 % ») et L. 6331-1 A (« l'effectif salarie et le
//! franchissement d'un seuil d'effectif salarie sont determines selon les modalites prevues a l'article L. 130-1 du code
//! de la securite sociale ») ; CSS L. 130-1, II : le franchissement a la hausse compte apres cinq annees civiles
//! consecutives, et « Le franchissement a la baisse d'un seuil d'effectif sur une annee civile a pour effet de faire a
//! nouveau courir » ce delai. Le guide Urssaf des contributions de formation (12/03/2026,
//! atelier/reference/sources/majoration-caisses-conges/urssaf-guide-contributions-formation-apprentissage-20260312.txt,
//! l. 568-586) le dit de la CFP : sous le seuil, le taux de L. 6331-1, 0,55 %.
//!
//! Sur une copie de la compagnie fictive figee `fictive-3090-grande` (compagnie inventee, cinq salaries chaque annee), dans le
//! dossier temporaire des tests : rien n'est ecrit dans `tests/donnees/fictives/`.

use crate::outils::{fictive, ecrire, lire, Copie};
use paie::contexte::Contexte;
use paie::date::Jour;
use paie::engagement::charger;
use serde_json::{json, Value};

/// La config de la fictive et sa saisie de juillet 2026, la surcouche portant la formation des permanents a 1 % (ce que
/// L. 6331-3 demande a partir de onze salaries), l'effectif de chaque annee civile donne par `effectif`.
fn copie(effectif: impl Fn(&str) -> f64) -> Copie {
    let c = Copie::config_de("fictive-3090-grande");
    let r = &c.0;
    std::fs::create_dir_all(r.join("paies")).unwrap();
    std::fs::copy(fictive("fictive-3090-grande").join("paies/2026-07.json"), r.join("paies/2026-07.json")).unwrap();
    let (mut bar, mut eff) = (lire(r, "config/baremes/2026.json"), lire(r, "config/effectifs.json"));
    for (an, v) in eff["annees"].as_object_mut().unwrap().iter_mut() {
        v["A01"]["valeur"] = json!(effectif(an));
    }
    // Au-dessus de onze, le versement mobilite se tranche aussi (CGCT L. 2333-64) : aucune autorite ne le reclame ici.
    bar["versement_mobilite"] = json!("aucun");
    bar["par_nature"] = json!({"permanent": {
        "cotisations_dsn": [{"bloc78": "03", "code81": "128", "taux": 1.0}],
        "rubriques_ajoutees": [{"id": "autres_4", "taux_pat": 1.0, "composants": [1.0]}]}});
    ecrire(r, "config/effectifs.json", &eff);
    ecrire(r, "config/baremes/2026.json", &bar);
    c
}

/// Le mois de juillet 2026 se charge-t-il ? `Err` : la phrase du refus.
fn juillet(c: &Copie) -> Result<(), String> {
    let ctx = Contexte::nouveau(&c.0, Some(Jour::lire("2026-07-28").unwrap())).map_err(|e| e.to_string())?;
    charger(&ctx, &c.0.join("paies/2026-07.json")).map(|_| ())
}

fn refus_de_la_formation(r: Result<(), String>) {
    let m = r.expect_err("une formation a 1 % sous le seuil de onze salaries");
    assert!(m.contains("formation") && m.contains("L. 6331-1 A") && (m.contains("0,55") || m.contains("0.55")), "{}", m);
}

#[test]
fn une_formation_a_un_pour_cent_sous_le_seuil_de_onze_salaries_se_refuse() {
    // Cinq salaries chaque annee : le seuil n'a jamais ete franchi, la compagnie doit 0,55 % (L. 6331-1).
    refus_de_la_formation(juillet(&copie(|_| 5.0)));
}

#[test]
fn une_annee_sous_le_seuil_fait_recourir_le_delai_et_la_formation_revient_a_0_55() {
    // Douze salaries depuis des annees, neuf en 2024 : le franchissement a la baisse fait recourir les cinq ans (CSS
    // L. 130-1, II, al. 2) ; en 2026 le seuil n'est plus franchi, la surcouche a 1 % d'avant se refuse.
    refus_de_la_formation(juillet(&copie(|an| if an == "2024" { 9.0 } else { 12.0 })));
}

#[test]
fn au_dela_du_seuil_la_formation_a_un_pour_cent_passe() {
    // Douze salaries cinq annees de suite : le seuil est franchi, 1 % est ce que L. 6331-3 demande.
    juillet(&copie(|_| 12.0)).unwrap();
}

#[test]
fn sous_le_seuil_le_taux_du_socle_passe() {
    let c = copie(|_| 5.0);
    let mut bar: Value = lire(&c.0, "config/baremes/2026.json");
    bar.as_object_mut().unwrap().remove("par_nature");
    ecrire(&c.0, "config/baremes/2026.json", &bar);
    juillet(&c).unwrap();
}
