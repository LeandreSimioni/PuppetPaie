//! La configuration datee, lue a la date de la periode.

use droit::contexte::Contexte;
use droit::fiche::Lue;
use crate::outils::typee::lu;
use std::collections::BTreeMap;
use std::path::Path;

/// TOUTE LA CONFIGURATION EST DATEE : une valeur changee a une date ne touche pas ce qui precede. La compagnie passe de la
/// 3090 a la 1285 et demenage le 01/11/2026 : octobre se lit sous la 3090 et l'ancienne adresse (la fiche, le contexte
/// du mois, la convention que le bareme applique), novembre sous les nouvelles ; la premiere valeur vaut de tout temps
/// (un salarie saisi le 15/10 se trouve pour un contrat d'avant), meme ecrite avec une date.
#[test]
fn une_valeur_changee_a_une_date_ne_touche_pas_ce_qui_precede() {
    use droit::fiche::{ChampsEntreprise, ChampsSalarie, ChangementEntreprise, ChangementSalarie, FicheDatee, FicheEntrepriseDatee, FicheSalarieDatee};
    use socle::date::Jour;
    let racine = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("configuration-datee-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&racine);
    std::fs::create_dir_all(racine.join("config")).unwrap();
    let j = |s: &str| Jour::lire(s).unwrap();
    let plate = r#"{"raison_sociale": "CIE DATEE", "idcc": "3090", "annexe_convention": "4", "adresse": "21 RUE DU PORT",
                    "premiere_embauche": "2026-01-01"}"#;
    let e = |texte: &str| lu::<ChampsEntreprise>(texte);
    let mut ent = FicheEntrepriseDatee::neuve(&e(plate));
    let ce = |texte: &str| lu::<ChangementEntreprise>(texte);
    ent.changer(ce(r#"{"case": "idcc", "valeur": "1285"}"#), Some(&j("2026-11-01")), Some("passage en CCNEAC"));
    ent.changer(ce(r#"{"case": "annexe_convention", "valeur": null}"#), Some(&j("2026-11-01")), None);
    ent.changer(ce(r#"{"case": "adresse", "valeur": "8 PLACE DU MARCHE"}"#), Some(&j("2026-11-01")), None);
    let s = |texte: &str| lu::<ChampsSalarie>(texte);
    let mut sal = FicheSalarieDatee::neuve(&s(r#"{"nom": "BRIOCHE", "prenom": "FANCHON"}"#));
    sal.changer(lu::<ChangementSalarie>(r#"{"case": "emploi", "valeur": "COMEDIENNE"}"#), Some(&j("2026-10-15")), None);
    let ecrire = |rel: &str, texte: String| std::fs::write(racine.join("config").join(rel), texte).unwrap();
    ecrire("entreprise.json", socle::ecriture::json_indente(&ent, 2));
    ecrire("salaries.json", socle::ecriture::json_indente(&BTreeMap::from([("BRIOCHE", &sal)]), 2));
    ecrire("spectacles.json", "{}".to_string());
    std::fs::create_dir_all(racine.join("config").join("baremes")).unwrap();
    ecrire("baremes/2026.json", r#"{"annee": 2026, "socle": 2026, "taux_atmp": {"valeur": 1.25, "code_risque": "923AD"}}"#.to_string());

    // Le contexte d'aujourd'hui (decembre) et celui d'octobre : la meme compagnie, chacun sous ses valeurs.
    let ctx = Contexte::nouveau(&racine, Some(j("2026-12-15"))).unwrap();
    assert_eq!(ctx.entreprise.idcc.portee(), Some("1285"));
    let octobre = ctx.du_mois("2026-10").unwrap();
    assert_eq!((octobre.entreprise.idcc.portee(), octobre.entreprise.adresse.portee()), (Some("3090"), Some("21 RUE DU PORT")));
    assert_eq!(ctx.entreprise_du_mois("2026-11").unwrap().adresse.portee(), Some("8 PLACE DU MARCHE"));
    // La convention que le bareme applique est celle du jour lu, quel que soit le contexte qui le lit.
    let convention = |jour: &str| ctx.entreprise_lue(&j(jour)).unwrap().fiche.idcc.en_texte();
    assert_eq!((convention("2026-10-31"), convention("2026-11-01")), (Some("3090".to_string()), Some("1285".to_string())));
    // La premiere valeur vaut de tout temps, meme ecrite avec une date.
    assert_eq!(ctx.salarie_au("BRIOCHE", &j("2020-03-01")).unwrap().nom.portee(), Some("BRIOCHE"));
    assert_eq!(ctx.au(&j("2020-03-01")).unwrap().salaries["BRIOCHE"].emploi.portee(), Some("COMEDIENNE"));
    assert_eq!(ctx.entreprise_au(&j("2019-01-01")).idcc.portee(), Some("3090"));
    std::fs::remove_dir_all(&racine).unwrap();
}
