//! Les majorations des employeurs affilies a une caisse de conges payes, au bulletin et a la DSN.
//!
//! Textes (prive/reference/sources/majoration-caisses-conges/LISEZ-MOI.md, et le releve LEGI du 10/10/2026 dans
//! prive/reference/sources/code-de-la-securite-sociale/ et code-du-travail/) : CSS L. 243-1-3, 1° -- l'employeur
//! affilie a une caisse de conges s'acquitte du FNAL, du versement mobilite (CGCT L. 2333-64 et L. 2531-2) et des
//! contributions de L. 6131-2, L. 6242-1 et L. 6331-6 du code du travail par des majorations proportionnelles ;
//! D. 243-0-2 : 11,5 %, 10 % pour la taxe d'apprentissage d'un intermittent du spectacle. Lettre circulaire Acoss
//! 2013-019 : « une majoration de l'assiette afferente aux contributions VT et FNAL » (§ 1.3), seule la part plafonnee
//! majoree (annexe 1, R2), la majoration apres la deduction forfaitaire specifique (R6) ; seules les remunerations
//! declarees a la caisse (R5). Tissot (2017) : « pour le versement de transport, le CTP 900 ».
//!
//! Sur une copie de la compagnie fictive figee `fictive-3090-grande` (compagnie inventee), dans le dossier temporaire des
//! tests : sa surcouche y recoit un versement mobilite fictif, et rien n'est ecrit dans `tests/donnees/fictives/`.

use paie::calcul::Paie;
use paie::contexte::Contexte;
use paie::date::Jour;
use paie::dsn::{generer, Options};
use paie::engagement::{charger, Saisie};
use paie::num::{eur, Decimal};
use rust_decimal::RoundingStrategy;
use serde_json::{json, Value};
use crate::outils::{base78, chemin_unique, ctp, dec, fictive, ecrire, lire, versement, Copie};
use std::str::FromStr;

/// Le 81.003 de la cotisation `code81` du versement (la premiere).
fn base81(v: &[(String, String)], code81: &str) -> Decimal {
    crate::outils::base81(v, code81).unwrap_or_else(|| panic!("pas de 81.003 sous {}", code81))
}

const TAUX_VM: &str = "2.0";

/// La config de `fictive-3090-grande` (et sa saisie de juillet 2026), douze salaries depuis cinq ans : le versement mobilite
/// est du (CGCT L. 2333-64), fictif a 2 %, et la formation des permanents passe a 1 % (code du travail L. 6331-3), que la
/// compagnie ecrit dans sa surcouche ; `retouche` appliquee a la surcouche et aux fiches.
fn copie(retouche: impl Fn(&mut Value, &mut Value)) -> Copie {
    let c = Copie::config_de("fictive-3090-grande");
    let r = &c.0;
    std::fs::create_dir_all(r.join("paies")).unwrap();
    std::fs::copy(fictive("fictive-3090-grande").join("paies/2026-07.json"), r.join("paies/2026-07.json")).unwrap();
    let (mut bar, mut sal, mut eff) = (lire(r, "config/baremes/2026.json"), lire(r, "config/salaries.json"), lire(r, "config/effectifs.json"));
    for an in eff["annees"].as_object_mut().unwrap().values_mut() {
        an["A01"]["valeur"] = json!(12.0);
    }
    bar["versement_mobilite"] = json!({"taux": f64::from_str(TAUX_VM).unwrap(), "code_insee": "45234", "autorite": "Autorite fictive", "source": "test"});
    bar["par_nature"] = json!({"permanent": {
        "cotisations_dsn": [{"bloc78": "03", "code81": "128", "taux": 1.0}],
        "rubriques_ajoutees": [{"id": "autres_4", "taux_pat": 1.0, "composants": [1.0]}]}});
    retouche(&mut bar, &mut sal);
    ecrire(r, "config/effectifs.json", &eff);
    ecrire(r, "config/baremes/2026.json", &bar);
    ecrire(r, "config/salaries.json", &sal);
    c
}

fn saisie(ctx: &Contexte, s: Value) -> Saisie {
    let chemin = chemin_unique("majoration.json");
    std::fs::write(&chemin, s.to_string()).unwrap();
    let s = charger(ctx, &chemin).unwrap();
    let _ = std::fs::remove_file(&chemin);
    s
}

/// La ligne `id` du bulletin : (base, taux patronal).
fn ligne(p: &Paie, id: &str) -> (Decimal, Decimal) {
    let l = p.lignes.iter().find(|l| l.id == id).unwrap_or_else(|| panic!("pas de ligne {} au bulletin", id));
    (l.base, Decimal::from_str(&l.taux_pat.unwrap().to_string()).unwrap())
}

fn a_l_euro(x: Decimal) -> Decimal {
    x.round_dp_with_strategy(0, RoundingStrategy::MidpointNearestEven)
}

fn intermittent(p: &Paie) -> bool {
    p.lignes.iter().any(|l| l.id == "conges_spec")
}

#[test]
fn le_versement_mobilite_d_un_intermittent_se_majore_de_11_5_pour_cent_pas_celui_d_un_permanent() {
    // Juillet 2026 de la fictive : deux CDI (Montansier, Nicolet), trois artistes et deux techniciens intermittents, tous
    // au-dessus de leur plafond -- le versement mobilite, deplafonne, n'en tient pas compte.
    let copie = copie(|_, _| {});
    let ctx = Contexte::nouveau(&copie.0, Some(Jour::lire("2026-07-28").unwrap())).unwrap();
    let s = charger(&ctx, &copie.0.join("paies/2026-07.json")).unwrap();
    let dsn = generer(&ctx, &s.paies, "2026-07", &[], &[], &Options::default()).unwrap();
    let (mut artistes, mut autres) = (Decimal::ZERO, Decimal::ZERO);
    let (mut vus_i, mut vus_p) = (0, 0);
    for p in &s.paies {
        let numero = p.engagement.numero.clone().unwrap();
        let attendu = if intermittent(p) { eur(p.brut * dec("1.115")) } else { p.brut };
        let (base, taux) = ligne(p, "versement_mobilite");
        assert_eq!(base, attendu, "{} : l'assiette du versement mobilite au bulletin", numero);
        let v = versement(&dsn, &numero);
        assert_eq!(base81(&v, "081"), attendu, "{} : le 81.003 du versement mobilite", numero);
        // Le 78.004 de la base 03 reste le brut : la majoration ne vit que dans l'assiette de la contribution.
        assert_eq!(base78(&v, "03"), vec![p.brut], "{} : base 03", numero);
        if taux == dec(TAUX_VM) * dec("0.7") {
            artistes += attendu;
        } else {
            assert_eq!(taux, dec(TAUX_VM), "{}", numero);
            autres += attendu;
        }
        if intermittent(p) {
            vus_i += 1;
            // Le FNAL garde sa majoration sur l'assiette plafonnee (lot 1i), et le versement mobilite la porte sur le
            // brut entier : deux bases differentes au-dessus du plafond.
            assert!(ligne(p, "autres_2").0 < base, "{}", numero);
        } else {
            vus_p += 1;
        }
    }
    assert_eq!((vus_i, vus_p), (5, 2));
    // Le bordereau : CTP 900 (hors artistes) et 921 (artistes), sous le qualifiant 920, sur l'assiette majoree.
    assert_eq!(ctp(&dsn, "900", "920"), Some(a_l_euro(autres)));
    assert_eq!(ctp(&dsn, "921", "920"), Some(a_l_euro(artistes)));
}

#[test]
fn un_permanent_seul_n_a_aucune_majoration() {
    // Montansier (CDI) a 5 000,00 : son versement mobilite et son bordereau sur le brut, son FNAL sur la tranche 1.
    let copie = copie(|_, _| {});
    let ctx = Contexte::nouveau(&copie.0, Some(Jour::lire("2026-10-28").unwrap())).unwrap();
    let s = saisie(
        &ctx,
        json!({"mois": "2026-10", "date_paiement": "2026-10-31", "engagements": [
            {"salarie": "Montansier", "nature": "cdi", "debut": "2023-09-04", "numero": "MO.23.01", "quotite_contrat": 151.67,
             "lignes": [{"rubrique": "mois", "quantite": 1, "taux": 5000.0, "heures": 151.67}]}
        ]}),
    );
    let dsn = generer(&ctx, &s.paies, "2026-10", &[], &[], &Options::default()).unwrap();
    let p = &s.paies[0];
    assert_eq!(ligne(p, "versement_mobilite").0, dec("5000.00"));
    assert_eq!(ligne(p, "autres_3").0, dec("5000.00"), "la taxe d'apprentissage d'un permanent n'est pas majoree");
    assert_eq!(base81(&versement(&dsn, "MO.23.01"), "081"), dec("5000.00"));
    assert_eq!(ctp(&dsn, "900", "920"), Some(dec("5000")));
    assert_eq!(ctp(&dsn, "921", "920"), None);
}

#[test]
fn sous_deduction_forfaitaire_les_assiettes_majorees_partent_du_brut_abattu() {
    // Deburau, comedien (categorie 25 : 18 % en 2026), un cachet de 200,00 sous son plafond (360,00) : brut abattu
    // 164,00. Le FNAL et le versement mobilite sur 164,00 x 1,115 = 182,86, la taxe d'apprentissage sur 164,00 x 1,10
    // = 180,40 -- comme la fiche Audiens (300 abattus, FNAL sur 334,50) et la lettre circulaire Acoss 2013-019, R6.
    // Lot R9 : la DFS est une option de l'employeur (fiche entreprise) et la fiche dit la profession (arrete du
    // 4 septembre 2025, art. 9) ; le socle la porte verifiee.
    let copie = copie(|_, sal| {
        sal["DEBURAU"]["deduction_forfaitaire"] = json!("artiste_dramatique");
    });
    let mut ent = lire(&copie.0, "config/entreprise.json");
    ent["deduction_forfaitaire_option"] = json!("accord_collectif");
    ecrire(&copie.0, "config/entreprise.json", &ent);
    let ctx = Contexte::nouveau(&copie.0, Some(Jour::lire("2026-10-28").unwrap())).unwrap();
    let s = saisie(
        &ctx,
        json!({"mois": "2026-10", "date_paiement": "2026-10-31", "engagements": [
            {"salarie": "Deburau", "spectacle": "266Z00910022", "debut": "2026-10-20", "fin": "2026-10-20", "numero": "DE2.26.10",
             "date_paiement": "2026-10-20",
             "lignes": [{"rubrique": "cachet", "quantite": 1, "taux": 200.0, "lieu": "Theatre fictif", "dates": ["2026-10-20"]}]}
        ]}),
    );
    let p = &s.paies[0];
    assert_eq!(p.brut, dec("200.00"));
    assert_eq!(ligne(p, "maladie").0, dec("164.00"), "la deduction s'applique");
    assert_eq!(ligne(p, "autres_2").0, dec("182.86"), "FNAL");
    assert_eq!(ligne(p, "versement_mobilite").0, dec("182.86"), "versement mobilite");
    assert_eq!(ligne(p, "autres_3").0, dec("180.40"), "taxe d'apprentissage");
}
