//! La configuration datee, lue a la date de la periode.

use droit::contexte::Contexte;
use serde_json::{json, Value};
use std::path::Path;

/// TOUTE LA CONFIGURATION EST DATEE : une valeur changee a une date ne touche pas ce qui precede. La compagnie passe de la
/// 3090 a la 1285 et demenage le 01/11/2026 : octobre se lit sous la 3090 et l'ancienne adresse (la fiche, le contexte
/// du mois, la convention que le bareme applique), novembre sous les nouvelles ; la premiere valeur vaut de tout temps
/// (un salarie saisi le 15/10 se trouve pour un contrat d'avant), meme ecrite avec une date ; une valeur non datee se
/// refuse.
#[test]
fn une_valeur_changee_a_une_date_ne_touche_pas_ce_qui_precede() {
    use droit::fiche::{lire_champs, ChampsEntreprise, ChampsSalarie, FicheDatee, FicheEntrepriseDatee, FicheSalarieDatee};
    use socle::date::Jour;
    let racine = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("configuration-datee-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&racine);
    std::fs::create_dir_all(racine.join("config")).unwrap();
    let j = |s: &str| Jour::lire(s).unwrap();
    let plate = json!({"raison_sociale": "CIE DATEE", "idcc": "3090", "annexe_convention": "4", "adresse": "21 RUE DU PORT",
                       "premiere_embauche": "2026-01-01"});
    let e = |v: Value| lire_champs::<ChampsEntreprise>(&v, "la fiche").unwrap();
    let mut ent = FicheEntrepriseDatee::neuve(&e(plate));
    assert!(ent.dater("idcc", &e(json!({"idcc": "1285"})), Some(&j("2026-11-01")), Some("passage en CCNEAC")));
    assert!(ent.dater("annexe_convention", &e(json!({"annexe_convention": ""})), Some(&j("2026-11-01")), None));
    assert!(ent.dater("adresse", &e(json!({"adresse": "8 PLACE DU MARCHE"})), Some(&j("2026-11-01")), None));
    let s = |v: Value| lire_champs::<ChampsSalarie>(&v, "la fiche").unwrap();
    let mut sal = FicheSalarieDatee::neuve(&s(json!({"nom": "BRIOCHE", "prenom": "FANCHON"})));
    assert!(sal.dater("emploi", &s(json!({"emploi": "COMEDIENNE"})), Some(&j("2026-10-15")), None));
    let ecrire = |rel: &str, v: Value| std::fs::write(racine.join("config").join(rel), v.to_string()).unwrap();
    ecrire("entreprise.json", socle::ecriture::en_valeur(&ent));
    ecrire("salaries.json", json!({ "BRIOCHE": socle::ecriture::en_valeur(&sal) }));
    ecrire("spectacles.json", json!({}));
    std::fs::create_dir_all(racine.join("config").join("baremes")).unwrap();
    ecrire("baremes/2026.json", json!({"annee": 2026, "socle": 2026, "taux_atmp": {"valeur": 1.25, "code_risque": "923AD"}}));

    // Le contexte d'aujourd'hui (decembre) et celui d'octobre : la meme compagnie, chacun sous ses valeurs.
    let ctx = Contexte::nouveau(&racine, Some(j("2026-12-15"))).unwrap();
    assert_eq!(ctx.entreprise.idcc.texte(), "1285");
    let octobre = ctx.du_mois("2026-10").unwrap();
    assert_eq!((octobre.entreprise.idcc.texte(), octobre.entreprise.adresse.texte()), ("3090", "21 RUE DU PORT"));
    assert_eq!(ctx.entreprise_du_mois("2026-11").unwrap().adresse.texte(), "8 PLACE DU MARCHE");
    // La convention que le bareme applique est celle du jour lu, quel que soit le contexte qui le lit.
    let convention = |jour: &str| ctx.entreprise_lue(&j(jour)).unwrap().fiche.idcc.texte().to_string();
    assert_eq!((convention("2026-10-31"), convention("2026-11-01")), ("3090".to_string(), "1285".to_string()));
    // La premiere valeur vaut de tout temps, meme ecrite avec une date.
    assert_eq!(ctx.salarie_au("BRIOCHE", &j("2020-03-01")).unwrap().nom.texte(), "BRIOCHE");
    assert_eq!(ctx.au(&j("2020-03-01")).unwrap().salaries["BRIOCHE"].emploi.texte(), "COMEDIENNE");
    assert_eq!(ctx.entreprise_au(&j("2019-01-01")).idcc.texte(), "3090");
    // Une valeur non datee se refuse, et la phrase dit quoi faire.
    let e = lire_champs::<FicheEntrepriseDatee>(&json!({"idcc": "3090"}), "config/entreprise.json").unwrap_err();
    assert!(e.contains("« idcc »") && e.contains("Ajouter une compagnie"), "{}", e);
    std::fs::remove_dir_all(&racine).unwrap();
}
