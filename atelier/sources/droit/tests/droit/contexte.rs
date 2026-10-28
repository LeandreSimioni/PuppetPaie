//! Le `Contexte` des quatre compagnies fictives contre `contexte.json` : SMIC, licence, PSS et l'empreinte du bareme resolu a
//! quatre dates (le bareme lui-meme est prouve par `bareme.rs`), la recherche des fiches et des spectacles, refus
//! compris. `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit l'oracle. Et la configuration datee, lue a la date de la periode.

use crate::outils::{fictive, egal, lire_oracle, oracle_selon, sceller};
use droit::contexte::Contexte;
use socle::date::d;
use serde_json::{json, Value};
use std::path::Path;

#[test]
fn le_contexte_des_fictives() {
    let mut cas = lire_oracle("contexte.json");
    for c in cas.as_array_mut().unwrap() {
        let ctx = Contexte::nouveau(&fictive(c["racine"].as_str().unwrap()), Some(d(&json!("2026-09-27")).unwrap())).unwrap();
        let o = c.as_object_mut().unwrap();
        let r = if let Some(j) = o.get("jour") {
            let jour = d(j).unwrap();
            let (m, h) = ctx.plafond_securite_sociale(jour.an).unwrap();
            o.remove("bareme");
            json!({"empreinte_bareme": sceller(&ctx.bareme(&jour).unwrap()), "smic": ctx.smic(&jour).unwrap(),
                   "licence": ctx.licence(&jour).unwrap(), "pss": [m, h]})
        } else {
            let r = match o.get("salarie") {
                Some(n) => ctx.salarie(n.as_str().unwrap()),
                None => ctx.spectacle(o["spectacle"].as_str().unwrap()),
            };
            o.remove("ok");
            o.remove("erreur");
            r.map_or_else(|e| json!({"erreur": e}), |v| json!({"ok": v["_cle"]}))
        };
        o.extend(r.as_object().unwrap().clone());
    }
    oracle_selon("contexte.json", "", &cas, egal);
}


/// TOUTE LA CONFIGURATION EST DATEE : une valeur changee a une date ne touche pas ce qui precede. La compagnie passe de la
/// 3090 a la 1285 et demenage le 01/11/2026 : octobre se lit sous la 3090 et l'ancienne adresse (la fiche, le contexte
/// du mois, la convention que le bareme applique), novembre sous les nouvelles ; un salarie n'existe pas avant sa
/// premiere valeur ; une valeur non datee se refuse.
#[test]
fn une_valeur_changee_a_une_date_ne_touche_pas_ce_qui_precede() {
    use droit::contexte::{dater, fiche_au, fiche_datee};
    use socle::date::Jour;
    let racine = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("configuration-datee-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&racine);
    std::fs::create_dir_all(racine.join("config")).unwrap();
    let j = |s: &str| Jour::lire(s).unwrap();
    let plate = json!({"raison_sociale": "CIE DATEE", "idcc": "3090", "annexe_convention": "4", "adresse": "21 RUE DU PORT",
                       "premiere_embauche": "2026-01-01"});
    let mut ent = fiche_datee(plate.as_object().unwrap(), &j("2026-01-01"));
    dater(&mut ent, "idcc", json!("1285"), &j("2026-11-01"), Some("passage en CCNEAC"));
    dater(&mut ent, "annexe_convention", json!(""), &j("2026-11-01"), None);
    dater(&mut ent, "adresse", json!("8 PLACE DU MARCHE"), &j("2026-11-01"), None);
    let sal = fiche_datee(json!({"nom": "BRIOCHE", "prenom": "FANCHON"}).as_object().unwrap(), &j("2026-10-15"));
    let ecrire = |rel: &str, v: Value| std::fs::write(racine.join("config").join(rel), v.to_string()).unwrap();
    ecrire("entreprise.json", Value::Object(ent));
    ecrire("salaries.json", json!({ "BRIOCHE": sal }));
    ecrire("spectacles.json", json!({}));
    std::fs::create_dir_all(racine.join("config").join("baremes")).unwrap();
    ecrire("baremes/2026.json", json!({"annee": 2026, "socle": 2026, "taux_atmp": {"valeur": 1.25, "code_risque": "923AD"}}));

    // Le contexte d'aujourd'hui (decembre) et celui d'octobre : la meme compagnie, chacun sous ses valeurs.
    let ctx = Contexte::nouveau(&racine, Some(j("2026-12-15"))).unwrap();
    assert_eq!(ctx.entreprise["idcc"], json!("1285"));
    let octobre = ctx.du_mois("2026-10").unwrap();
    assert_eq!((&octobre.entreprise["idcc"], &octobre.entreprise["adresse"]), (&json!("3090"), &json!("21 RUE DU PORT")));
    assert_eq!(ctx.entreprise_du_mois("2026-11").unwrap()["adresse"], json!("8 PLACE DU MARCHE"));
    // La convention que le bareme applique est celle du jour lu, quel que soit le contexte qui le lit.
    let convention = |jour: &str| ctx.bareme(&j(jour)).unwrap()["_entreprise"]["idcc"].clone();
    assert_eq!((convention("2026-10-31"), convention("2026-11-01")), (json!("3090"), json!("1285")));
    // Un salarie n'existe qu'a partir de sa premiere valeur.
    assert!(ctx.au(&j("2026-10-14")).unwrap().salaries.is_empty());
    assert_eq!(ctx.salarie_au("BRIOCHE", &j("2026-10-15")).unwrap()["nom"], json!("BRIOCHE"));
    assert!(ctx.salarie_au("BRIOCHE", &j("2026-10-14")).is_err());
    // Une valeur non datee se refuse, et la phrase dit quoi faire.
    let e = fiche_au(&json!({"idcc": "3090"}), &j("2026-10-01"), "config/entreprise.json").unwrap_err();
    assert!(e.contains("`idcc`") && e.contains("Ajouter une compagnie"), "{}", e);
    std::fs::remove_dir_all(&racine).unwrap();
}
