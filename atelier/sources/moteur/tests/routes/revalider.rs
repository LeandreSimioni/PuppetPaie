//! UNE REVALIDATION QUI NE CHANGE RIEN NE TOUCHE NI LA VERSION NI SA DATE : un engagement parti sur sa version 1, remis
//! en saisie puis revalide tel quel des semaines plus tard, reste la version 1 du jour ou elle a ete validee (la frise
//! disait « Version 1 — le 20/12 » pour une version partie le 20/11).

use serde_json::{json, Value};

use crate::outils::rejeu::{rejouer_dossier, scenario};
use crate::outils::{dossier_de_test, reussi, route};

#[test]
fn revalider_sans_rien_changer_garde_la_version_et_sa_date() {
    let d = dossier_de_test("revalider", "sans_changement");
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    let gestes = std::fs::read_to_string(scenario("treteaux-du-banc").join("gestes.jsonl")).unwrap();
    let debut: Vec<&str> = gestes.lines().take_while(|l| !l.contains("\"geste\":\"jour\"")).collect();
    std::fs::write(sc.join("gestes.jsonl"), debut.join("\n") + "\n").unwrap();
    let (_, c) = rejouer_dossier(&sc, &d);
    let fuller = json!({"mois": "2026-11", "salarie": "FULLER", "debut": "2026-11-20"});
    let avec = |x: Value| {
        let mut f = fuller.clone();
        f.as_object_mut().unwrap().extend(x.as_object().unwrap().clone());
        f
    };
    reussi(&route(&c, "valider-engagement", avec(json!({"valide": true})), "2026-11-02"));
    reussi(&route(&c, "contrats-imprimer", json!({"mois": "2026-11", "contrats": [fuller]}), "2026-11-02"));
    let valide = || -> Value {
        let e: Value = serde_json::from_slice(&std::fs::read(c.join("engagements/2026-11.json")).unwrap()).unwrap();
        let e = e["engagements"].as_array().unwrap().iter().find(|x| x["salarie"] == "FULLER").unwrap().clone();
        assert!(e.get("versions").is_none(), "une revalidation sans changement a fait une version : {}", e["versions"]);
        e["valide"].clone()
    };
    let avant = valide();

    reussi(&route(&c, "valider-engagement", avec(json!({"valide": false})), "2026-12-20"));
    reussi(&route(&c, "valider-engagement", avec(json!({"valide": true})), "2026-12-20"));
    let apres = valide();
    assert_eq!((&apres["version"], &apres["le"]), (&avant["version"], &avant["le"]));
}
