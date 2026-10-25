//! Le signalement d'arret (04) et de reprise (05) : le texte ou le refus de chaque cas de `donnees/signalement.json`
//! (dix scenarios sur deux compagnies fictives), un instantane du moteur que le test reecrit sur demande ; et
//! l'etablissement d'un salarie d'un secondaire, l'arret repris le premier du mois.

use crate::outils;
use droit::contexte::Contexte;
use socle::date::Jour;
use paie::dsn::Arret;
use paie::signalement::{generer, Echec, Options};
use serde_json::{json, Value};
use std::path::Path;

fn options(o: &Value) -> Options {
    let mut op = Options::default();
    let texte = |c: &str| o.get(c).and_then(Value::as_str).map(str::to_string);
    if let Some(v) = texte("nature") {
        op.nature = v;
    }
    if let Some(v) = texte("type_declaration") {
        op.type_declaration = v;
    }
    if let Some(n) = o.get("ordre").and_then(Value::as_i64) {
        op.ordre = n;
    }
    op.remplace = texte("remplace");
    op.envoi_test = o.get("envoi_test").and_then(Value::as_bool).unwrap_or(false);
    op.date_constitution = texte("date_constitution").map(|d| Jour::lire(&d).unwrap());
    op
}

fn oracle() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/donnees/signalement.json")
}

#[test]
fn chaque_signalement_d_arret_ou_de_reprise_ecrit_son_texte_ou_refuse() {
    let mut cas: Vec<Value> = serde_json::from_slice(&std::fs::read(oracle()).unwrap()).unwrap();
    let (mut textes, mut refus) = (0, 0);
    for c in cas.iter_mut() {
        let ctx = Contexte::nouveau(&outils::fictive(c["racine"].as_str().unwrap()), Some(Jour::lire(c["date"].as_str().unwrap()).unwrap())).unwrap();
        c["attendu"] = match generer(&ctx, &Arret { v: c["arret"].clone() }, &options(&c["options"])) {
            Ok(t) => {
                textes += 1;
                json!({"texte": t})
            }
            Err(Echec::Impossible(m)) => {
                refus += 1;
                json!({"refus": m})
            }
            Err(e) => panic!("{} : {:?}", c["nom"], e),
        };
    }
    assert!(textes >= 10 && refus >= 10, "{} textes, {} refus", textes, refus);
    outils::oracle("signalement.json", "", &Value::Array(cas));
}

// ------------------------------------------------------------------ vague V2a

fn valeur(texte: &str, rubrique: &str) -> Option<String> {
    outils::valeur(&outils::paires(texte), rubrique)
}

/// V2a, point 4 de la relecture (fiche N6, S21.G00.11.001 des signalements) : « Dans le cadre d'un contrat de travail,
/// il s'agit de l'etablissement de rattachement administratif du salarie » (cahier technique DSN 2026.1, p. 144) ; un
/// signalement porte les donnees identifiantes, NIC compris, de la derniere mensuelle (§ 2.5.1, p. 67). Le
/// signalement d'arret ecrivait toujours le NIC (et le SIRET, 40.019) du principal.
#[test]
fn le_signalement_d_arret_d_un_salarie_d_un_secondaire_porte_son_etablissement() {
    let cas: Vec<Value> = serde_json::from_slice(&std::fs::read(oracle()).unwrap()).unwrap();
    let c = cas.iter().find(|c| c["attendu"].get("texte").is_some()).unwrap();
    let mut ctx = Contexte::nouveau(&outils::fictive(c["racine"].as_str().unwrap()), Some(Jour::lire(c["date"].as_str().unwrap()).unwrap())).unwrap();
    ctx.entreprise["etablissements"] = json!([{
        "nic": "00099", "naf": ctx.entreprise["naf"].clone(), "adresse": "1 RUE DE L'ESSAI",
        "code_postal": "69001", "commune": "LYON",
        "taux_atmp": {"valeur": 1.25, "code_risque": "923AD"}
    }]);
    let siren = ctx.entreprise["siren"].as_str().unwrap().to_string();
    let principal = ctx.entreprise["nic"].as_str().unwrap().to_string();
    let cle = ctx.salarie(c["arret"]["salarie"].as_str().unwrap()).unwrap()["_cle"].as_str().unwrap().to_string();
    ctx.salaries[&cle]["etablissement"] = Value::String("00099".into());
    let arret = Arret { v: c["arret"].clone() };
    let t = generer(&ctx, &arret, &options(&c["options"])).unwrap();
    assert_eq!(valeur(&t, "S21.G00.06.002"), Some(principal), "le siege reste le siege");
    assert_eq!(valeur(&t, "S21.G00.11.001").as_deref(), Some("00099"));
    assert_eq!(valeur(&t, "S21.G00.11.003").as_deref(), Some("1 RUE DE L'ESSAI"));
    assert_eq!(valeur(&t, "S21.G00.11.005").as_deref(), Some("LYON"));
    assert_eq!(valeur(&t, "S21.G00.40.019"), Some(format!("{}00099", siren)));

    // Un etablissement que la fiche entreprise ne porte pas : refus.
    ctx.salaries[&cle]["etablissement"] = Value::String("00077".into());
    match generer(&ctx, &arret, &options(&c["options"])) {
        Err(e) => assert!(format!("{:?}", e).contains("00077"), "{:?}", e),
        Ok(t) => panic!("un etablissement inconnu est passe : {}", t),
    }
}

/// V2a, point 24 de la relecture (fiche N7, bloc 60 reporte) : la suspension se declare « dans toutes les DSN
/// mensuelles dont le Mois Principal declare presente une periode commune avec la suspension » (bloc S21.G00.60, cahier
/// technique DSN 2026.1, p. 265), et la reprise est le premier jour travaille (S21.G00.60.010, p. 269) : un arret repris
/// le 1er juillet ne couvre pas juillet (`Arret::couvre`, `src/dsn.rs`). La DSN de juillet 2026 de fictive-3090-grande : le
/// CDD AU.25.01 a fini le 30/06, son arret repris le 01/07 ne la fait pas refuser, et elle n'en porte rien.
#[test]
fn un_arret_repris_le_premier_du_mois_ne_se_reporte_pas_dans_ce_mois() {
    let repris_le_1er = Arret {
        v: json!({
            "salarie": "Audinot", "contrat_debut": "2025-09-01", "contrat_numero": "AU.25.01", "motif": "01",
            "dernier_jour_travaille": "2026-06-20", "fin_previsionnelle": "2026-07-03", "subrogation": "02",
            "reprise": "2026-07-01", "reprise_motif": "01", "date_accident": null, "envois": [],
        }),
    };
    let ctx = Contexte::nouveau(&outils::fictive("fictive-3090-grande"), Some(Jour::lire("2026-07-28").unwrap())).unwrap();
    let s = engagement::charger(&ctx, &outils::fictive("fictive-3090-grande").join("paies/2026-07.json")).unwrap();
    assert!(s.paies.iter().all(|p| p.engagement.numero.as_deref() != Some("AU.25.01")));
    let t = paie::dsn::generer(&ctx, &s.paies, "2026-07", &[], &[repris_le_1er], &paie::dsn::Options::default())
        .unwrap_or_else(|m| panic!("la DSN de juillet est refusee : {}", m));
    assert!(!t.contains("S21.G00.60."), "un bloc 60 sans jour commun avec juillet");
}
