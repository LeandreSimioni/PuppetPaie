//! Corrections DSN, vague V2d : la deduction forfaitaire specifique (DFS) dans la DSN.
//!
//! Cahier technique DSN 2026.1 (`atelier/reference/norme-dsn/texte/`) :
//! - `bloc-S21.G00.40.txt`, p. 180 (ligne 39) et la rubrique, p. 195, lignes 936-943 : « Taux de deduction forfaitaire specifique
//!   pour frais professionnels - S21.G00.40.023 », « Abattement pour frais professionnels applique a la part de
//!   remuneration afferente a l'activite ouvrant droit a la deduction », format `N [4,6]`, deux decimales ;
//!   `tableaux-des-usages.txt` p. 365, ligne 245 : conditionnelle en mensuelle ;
//! - `bloc-S21.G00.54.txt`, p. 257 (ligne 56) : « 10 - Deduction forfaitaire specifique », un type d'« Autre element de
//!   revenu brut » ; `chapitre-2-principes-de-constitution.txt` p. 27, lignes 461-463 : le bloc 54 « vehicule [...] frais
//!   professionnels (y compris le montant de la Deduction Forfaitaire Specifique si applicable) » ;
//! - `bloc-S21.G00.50.txt`, p. 243, CCH-16 (lignes 82-86) : le 54 de type 10 porte son numero de contrat (54.005), « Ce controle
//!   vise a declarer le montant de deduction forfaitaire specifique par contrat ».
//!
//! Le reste (le type 35, l'ancien taux du bloc 41) ne s'ecrit pas : rien ici ne les appelle.
//!
//! Lot R9 : la compagnie fictive opte pour la DFS (fiche entreprise) et Deburau est artiste dramatique, dans une copie
//! de `fictive-3090-grande`.

use crate::outils::{self, compagnie, contrat, paires, versement};
use engagement::calcul::Paie;
use droit::contexte::Contexte;
use socle::date::Jour;
use paie::dsn::{generer, Options};
use socle::num::Decimal;
use serde_json::json;
use std::str::FromStr;

/// Octobre 2026 dans une copie de `fictive-3090-grande` ou Deburau (comedien, categorie 25 : 18 % en 2026) exerce la DFS :
/// Deburau, un cachet de 200,00 sous DFS (brut abattu 164,00, deduction 36,00) ; Clairon, le meme cachet le meme jour,
/// sans DFS. Rend les paies et la DSN.
fn octobre_sous_dfs() -> (Vec<Paie>, String) {
    let r = compagnie::ses_dsn("fictive-3090-grande", &[], &outils::dossier_de_test("dfs_dsn", "octobre"));
    // Lot R9 : la DFS est une option de l'employeur (fiche entreprise), la fiche salarie dit la profession (arrete
    // du 4 septembre 2025, art. 9) ; le socle la porte verifiee.
    let mut ent = outils::lire(&r, "config/entreprise.json");
    ent["deduction_forfaitaire_option"] = json!("accord_collectif");
    outils::ecrire(&r, "config/entreprise.json", &ent);
    outils::retoucher_fiches(&r, &[("DEBURAU", json!({"deduction_forfaitaire": "artiste_dramatique"}))]);
    let cachet = |salarie: &str, numero: &str| {
        json!({"salarie": salarie, "spectacle": "266Z00910022", "debut": "2026-10-20", "fin": "2026-10-20", "numero": numero,
               "date_paiement": "2026-10-20",
               "lignes": [{"rubrique": "cachet", "quantite": 1, "taux": 200.0, "lieu": "Theatre fictif", "dates": ["2026-10-20"]}]})
    };
    let saisie = json!({"mois": "2026-10", "date_paiement": "2026-10-31", "engagements": [cachet("Deburau", "DE2.26.10"), cachet("Clairon", "CL6.26.10")]});
    outils::ecrire(&r, "saisie-dfs.json", &saisie);
    let ctx = Contexte::nouveau(&r, Some(Jour::lire("2026-10-28").unwrap())).unwrap();
    let s = crate::outils::charger_fichier(&ctx, &r.join("saisie-dfs.json")).unwrap();
    let dsn = generer(&ctx, &s.paies, "2026-10", &[], &[], &Options::default()).unwrap();
    (s.paies, dsn)
}

fn paie_de<'a>(paies: &'a [Paie], numero: &str) -> &'a Paie {
    paies.iter().find(|p| p.engagement.numero.as_deref() == Some(numero)).unwrap()
}

/// Les blocs 54 d'un versement : (54.001, 54.002, 54.003, 54.004, 54.005).
fn blocs_54(v: &[(String, String)]) -> Vec<[String; 5]> {
    let mut out: Vec<[String; 5]> = Vec::new();
    for (k, x) in v {
        if let Some(n) = k.strip_prefix("S21.G00.54.00") {
            let i: usize = n.parse().unwrap();
            if i == 1 {
                out.push(Default::default());
            }
            out.last_mut().unwrap()[i - 1] = x.clone();
        }
    }
    out
}

#[test]
fn une_paie_sous_dfs_declare_son_taux_au_contrat_son_montant_en_54_type_10_et_ses_bases_abattues() {
    let (paies, dsn) = octobre_sous_dfs();
    assert_eq!(paie_de(&paies, "DE2.26.10").bases.get("brut_dfs").copied(), Some(Decimal::from_str("164.00").unwrap()), "la DFS s'applique au calcul");

    // Le contrat : 40.023 = 18.00, a sa place (apres 40.022, avant 40.024).
    let c = contrat(&dsn, "DE2.26.10");
    let rubriques: Vec<&str> = c.iter().map(|(k, _)| k.as_str()).collect();
    let i = rubriques.iter().position(|k| *k == "S21.G00.40.023").expect("le taux de DFS au contrat");
    assert_eq!(c[i].1, "18.00");
    assert!(rubriques[..i].contains(&"S21.G00.40.022") && rubriques[i + 1..].contains(&"S21.G00.40.024"), "{:?}", rubriques);
    assert!(!contrat(&dsn, "CL6.26.10").iter().any(|(k, _)| k == "S21.G00.40.023"), "pas de DFS, pas de 40.023");

    // Le versement : un 54 de type 10 au montant de la deduction, date de la periode et rattache au contrat.
    let v = blocs_54(&versement(&dsn, "DE2.26.10"));
    let dix: Vec<&[String; 5]> = v.iter().filter(|b| b[0] == "10").collect();
    assert_eq!(dix.len(), 1, "{:?}", v);
    assert_eq!(dix[0], &["10", "36.00", "20102026", "20102026", "DE2.26.10"].map(String::from));
    assert!(!v.iter().any(|b| b[0] == "35"), "aucun frais professionnel reintegre : pas de type 35");
    assert!(!blocs_54(&versement(&dsn, "CL6.26.10")).iter().any(|b| b[0] == "10"));
    assert!(!dsn.contains("S21.G00.41.017"), "aucun ancien taux de DFS");

    // Les bases (lot R9) :
    // - le 51 de type 001 : le guide Urssaf du 04/06/2026, p. 36, recompose les revenus bruts du montant net social par
    //   « 51 de type 001 + 54 de type 10 - 54 de type 35 », et le BOSS (montant net social, Q 26) veut le brut entier :
    //   le 51 de type 001 est donc le brut abattu (164,00 + 36,00 = 200,00) ;
    // - les bases 02 et 03 (securite sociale) et 23, 43 (Agirc-Arrco) : le cahier Agirc-Arrco CT2026 declare la « Base VRP
    //   multicartes apres abattement pour frais professionnels » en 02 et 03 ; la norme, chapitre 2, part pour le net
    //   fiscal du « montant brut [...] servant de base au calcul des cotisations de securite sociale » et y AJOUTE « le
    //   montant de l'abattement forfaitaire » ; chaque 81.003 suit son bulletin ;
    // - pas les bases 04 (CSG), 07 (chomage, « avant application de l'abattement »), 27 (Conges Spectacles, « avant
    //   toute deduction forfaitaire specifique ») ni la prevoyance (31, 059) ;
    // - le bordereau suit les bases : les CTP de la securite sociale sur l'abattu, la CSG (260) sur sa propre assiette.
    let (vd, vc) = (versement(&dsn, "DE2.26.10"), versement(&dsn, "CL6.26.10"));
    assert_eq!(remuneration(&vd, "001"), "164.00", "le 51 de type 001 est le brut abattu");
    assert_eq!(remuneration(&vc, "001"), "200.00");
    assert_eq!(remuneration(&vd, "002"), remuneration(&vc, "002"), "le salaire du chomage n'est pas abattu");
    let (bd, bc) = (bases_78(&vd), bases_78(&vc));
    let base = |b: &[(String, String, Vec<(String, String)>)], code: &str| -> String {
        b.iter().find(|x| x.0 == code).unwrap_or_else(|| panic!("pas de base {}", code)).1.clone()
    };
    for code in ["02", "03", "23", "43"] {
        assert_eq!(base(&bd, code), "164.00", "la base {} est abattue", code);
        assert_eq!(base(&bc, code), "200.00", "la base {} sans DFS", code);
    }
    for code in ["04", "07", "27"] {
        assert_eq!(base(&bd, code), base(&bc, code), "la base {} n'est pas abattue", code);
    }
    // Chaque 81.003 : celui du bulletin (la prevoyance, sous 31, n'en ecrit pas ; sa ligne reste sur le brut, `tests/dfs.rs`).
    let assiette = |b: &[(String, String, Vec<(String, String)>)], bloc: &str, code: &str| -> String {
        b.iter().filter(|x| x.0 == bloc).flat_map(|x| x.2.iter()).find(|c| c.0 == code).unwrap_or_else(|| panic!("pas de {} sous {}", code, bloc)).1.clone()
    };
    let deburau = paie_de(&paies, "DE2.26.10");
    let b = |k: &str| format!("{:.2}", deburau.bases[k]);
    assert_eq!(assiette(&bd, "03", "075"), "164.00");
    assert_eq!(assiette(&bd, "03", "130"), b("brut_x110_dfs"), "l'apprentissage sur le brut abattu majore");
    assert_eq!(assiette(&bd, "02", "049"), b("brut_x115_plafonne_dfs"), "le FNAL sur le brut abattu majore et plafonne");
    assert_eq!(assiette(&bd, "23", "142"), "164.00", "la part patronale Agirc-Arrco sous la base 23 abattue");
    assert_eq!(assiette(&bd, "07", "040"), "200.00");
    // Le bordereau : 312 (artistes) sur 164,00 + 200,00 ; 334 sur les deux bruts majores ; 027 et 992 de meme ; 260 non.
    let x115 = deburau.bases["brut_x115_plafonne_dfs"] + paie_de(&paies, "CL6.26.10").bases["brut_x115_plafonne"];
    assert_eq!(assiette_ctp(&dsn, "312", "920"), "364.00");
    assert_eq!(assiette_ctp(&dsn, "312", "921"), "364.00");
    assert_eq!(assiette_ctp(&dsn, "027", "920"), "364.00");
    // Les assiettes du bordereau s'arrondissent a l'euro, la fraction egale a 0,50 comptee pour 1 (CSS L. 133-10,
    // `code-de-la-securite-sociale/articles-20261013.md`) : pas `Decimal::round`, qui arrondit au pair.
    let euro = |x: Decimal| x.round_dp_with_strategy(0, rust_decimal::RoundingStrategy::MidpointAwayFromZero);
    assert_eq!(assiette_ctp(&dsn, "334", "921"), format!("{:.2}", euro(x115)));
    let csg: Decimal = paies.iter().map(|p| p.base_csg).sum();
    assert_eq!(assiette_ctp(&dsn, "260", "920"), format!("{:.2}", euro(csg)));
}

/// Les blocs 78 d'un versement : (78.001, 78.004, [(81.001, 81.003)]) ; 81.003 vide quand la cotisation ne l'ecrit pas.
fn bases_78(v: &[(String, String)]) -> Vec<(String, String, Vec<(String, String)>)> {
    let mut out: Vec<(String, String, Vec<(String, String)>)> = Vec::new();
    for (k, x) in v {
        match k.as_str() {
            "S21.G00.78.001" => out.push((x.clone(), String::new(), Vec::new())),
            "S21.G00.78.004" => out.last_mut().unwrap().1 = x.clone(),
            "S21.G00.81.001" => out.last_mut().unwrap().2.push((x.clone(), String::new())),
            "S21.G00.81.003" => out.last_mut().unwrap().2.last_mut().unwrap().1 = x.clone(),
            _ => {}
        }
    }
    out
}

/// La valeur du 51.013 de type `type_51` d'un versement.
fn remuneration(v: &[(String, String)], type_51: &str) -> String {
    let i = v.iter().position(|(k, x)| k == "S21.G00.51.011" && x == type_51).unwrap();
    v[i + 1..].iter().find(|(k, _)| k == "S21.G00.51.013").unwrap().1.clone()
}

/// L'assiette (23.004) du CTP `code` sous le qualifiant `qualifiant` du bordereau Urssaf.
fn assiette_ctp(dsn: &str, code: &str, qualifiant: &str) -> String {
    let l = paires(dsn);
    let i = l
        .iter()
        .enumerate()
        .position(|(i, (k, v))| k == "S21.G00.23.001" && v == code && l.get(i + 1).map_or(false, |(_, q)| q == qualifiant))
        .unwrap_or_else(|| panic!("pas de CTP {}/{}", code, qualifiant));
    l[i + 1..].iter().take_while(|(k, _)| k != "S21.G00.23.001").find(|(k, _)| k == "S21.G00.23.004").unwrap().1.clone()
}
