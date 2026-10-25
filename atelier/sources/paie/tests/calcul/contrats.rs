//! Les natures de contrat (`contrats-base.json`) contre `contrats.json` : chaque attribut et chaque code des natures,
//! la nature par defaut, l'inconnue refusee, `contrat_court`. `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit l'oracle.

use crate::outils::{egal, lire_oracle, oracle_selon};
use droit::natures::{contrat_court, defaut, nature, natures, Jour};
use serde_json::{json, Value};

fn jour(s: &str) -> Jour {
    let p: Vec<&str> = s.split('-').collect();
    (p[0].parse().unwrap(), p[1].parse().unwrap(), p[2].parse().unwrap())
}

#[test]
fn les_natures_et_contrat_court() {
    let mut rel = lire_oracle("contrats.json");
    rel["defaut"] = json!(defaut().unwrap());
    rel["inconnue"] = json!(nature(Some("zzz")).unwrap_err());
    // Un engagement muet (sans nature, ou nature vide) est de la nature par defaut (regle 6 : un CDDU).
    for muet in [None, Some("")] {
        assert_eq!(nature(muet).unwrap().cle, defaut().unwrap(), "{:?}", muet);
    }
    assert_eq!(natures().unwrap().len(), rel["natures"].as_object().unwrap().len());
    for (cle, v) in rel["natures"].as_object_mut().unwrap() {
        let n = nature(Some(cle)).unwrap();
        assert_eq!(n.cle, *cle);
        let codes: serde_json::Map<String, Value> =
            v["codes"].as_object().unwrap().keys().map(|r| (r.clone(), json!([n.code(r), n.interdit(r), n.ecrit(r)]))).collect();
        *v = json!({
            "libelle": n.libelle, "paie": n.paie, "duree_indeterminee": n.duree_indeterminee, "unite_quotite": n.unite_quotite,
            "plafond_quotite": n.plafond_quotite, "quotite_reference_defaut": n.quotite_reference_defaut, "bareme": n.bareme,
            "mensuelle": n.mensuelle(), "permanent": n.permanent(), "codes": codes,
            "motif_rupture_par_defaut": n.motif_rupture_par_defaut(), "motifs_rupture": n.motifs_rupture(),
            "motifs_recours": n.motifs_recours(), "motif_recours_exige": n.motif_recours_exige(),
            "signalement_fctu": n.signalement_fctu(None), "codes_fin": n.codes_fin(None),
            "signalement_par_motif": n.motifs_rupture().iter().map(|m| {
                let code = m["code"].as_str().unwrap();
                (code.to_string(), json!([n.signalement_fctu(Some(code)), n.codes_fin(Some(code))]))
            }).collect::<serde_json::Map<String, Value>>(), "type_contrat_dpae": n.type_contrat_dpae(),
            "date_fin_dpae": n.date_fin_dpae(), "types_remuneration": n.types_remuneration(),
            "remunerations_au_brut": n.remunerations_au_brut(), "unites_activite": n.unites_activite(),
            "unite_absence": n.unite_absence(),
        });
    }
    for c in rel["court"].as_array_mut().unwrap() {
        let n = nature(c[0].as_str()).unwrap();
        c[4] = json!(contrat_court(&n, jour(c[1].as_str().unwrap()), c[2].as_str().map(jour), jour(c[3].as_str().unwrap())));
    }
    assert!(rel["court"].as_array().unwrap().len() >= 39);
    oracle_selon("contrats.json", "", &rel, egal);
}
