//! UNE LIGNE « REPAS » OU « NUITÉE » SANS TAUX PREND LE TARIF DE LA CONVENTION, HORS ASSIETTE. Convention 3090, avenant du
//! 21 juillet 2025 relatif aux salaires minimaux applicables au 1er septembre 2025
//! (`atelier/reference/sources/legifrance-3090-avenant-salaires-20250721.md`, l. 175 et 429-432) : « Chambre et
//! petit-déjeuner : 70,28 euros. Chaque repas principal : 17,85 euros. » Remboursements de frais au forfait, hors de
//! l'assiette des cotisations (BOSS, Frais professionnels, § 240 et 1270-1290,
//! `atelier/reference/sources/boss/boss-frais-professionnels-20261013.txt`). Les Tréteaux du Banc (3090), FULLER en
//! novembre 2026.

use serde_json::{json, Value};

use crate::outils::rejeu::{rejouer_dossier, scenario};
use crate::outils::{dec, dossier_de_test, reussi, route};

#[test]
fn repas_et_nuitee_au_tarif_de_la_convention_hors_assiette() {
    let d = dossier_de_test("tarif_conventionnel", "repas_nuitee");
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    let gestes = std::fs::read_to_string(scenario("treteaux-du-banc").join("gestes.jsonl")).unwrap();
    let debut: Vec<&str> = gestes.lines().take_while(|l| !l.contains("\"route\":\"apercu\"")).collect();
    std::fs::write(sc.join("gestes.jsonl"), debut.join("\n") + "\n").unwrap();
    let (_, c) = rejouer_dossier(&sc, &d);

    let saisie = json!({"salarie": "FULLER", "spectacle": "le-banc-des-menteurs", "lignes": [
        {"rubrique": "cachet", "quantite": 2, "taux": 150, "dates": ["2026-11-20", "2026-11-21"]},
        {"rubrique": "repas", "quantite": 2, "taux": "", "dates": ["2026-11-20", "2026-11-21"]},
        {"rubrique": "nuitee", "quantite": 1, "taux": "", "dates": ["2026-11-20"]},
    ]});
    let r = route(&c, "apercu", json!({"mois": "2026-11", "cartes": [{"saisie": saisie}]}), "2026-11-02");
    let a = reussi(&r);
    let paie: &Value = &a["paies"][0];
    let ligne = |code: &str| -> &Value {
        paie["remuneration"].as_array().and_then(|l| l.iter().find(|x| x["rubrique"] == code)).unwrap_or_else(|| panic!("pas de ligne {} : {}", code, paie))
    };
    assert_eq!((ligne("repas")["designation"].as_str(), dec(ligne("repas")["taux"].as_str().unwrap())), (Some("Repas"), dec("17.85")));
    assert_eq!(dec(ligne("repas")["montant"].as_str().unwrap()), dec("35.70"));
    assert_eq!((ligne("nuitee")["designation"].as_str(), dec(ligne("nuitee")["taux"].as_str().unwrap())), (Some("Nuitée"), dec("70.28")));

    // Hors assiette : la meme carte sans eux a le meme brut, et le meme cout (aucune cotisation de plus), et un net plus haut de leurs 105,98 EUR.
    let sans = json!({"salarie": "FULLER", "spectacle": "le-banc-des-menteurs", "lignes": [saisie["lignes"][0].clone()]});
    let s = reussi(&route(&c, "apercu", json!({"mois": "2026-11", "cartes": [{"saisie": sans}]}), "2026-11-02")).clone();
    let q = &s["paies"][0];
    let n = |p: &Value, k: &str| dec(p[k].as_str().unwrap());
    assert_eq!(n(paie, "brut"), n(q, "brut"));
    assert_eq!(n(paie, "net_a_payer") - n(q, "net_a_payer"), dec("105.98"));
    assert_eq!(n(paie, "cout_employeur"), n(q, "cout_employeur"));
}
