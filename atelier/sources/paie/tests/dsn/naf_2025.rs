//! La NAF 2025 (lot R5b) : la fiche entreprise admet un code de la NAF rev. 2 ou de la NAF 2025 ; un fichier DSN en norme
//! P27V01 refuse un code NAF rev. 2, en disant vers quels codes NAF 2025 il passe, sans choisir a la place de l'INSEE.
//!
//! Textes : decret n° 2025-736 (art. 2 et 6 : NAF 2025 en vigueur, NAF rev. 2 abrogee, au 1er janvier 2027) ; table de
//! correspondance de l'INSEE (`atelier/reference/sources/insee-naf-2025/`, relevee dans `logiciel/donnees/naf-base.json`) ;
//! actualite net-entreprises du 08/10/2026 (`atelier/reference/sources/net-entreprises-actualites/`) : apres la mise en
//! production de janvier 2027, la NAF 2025 s'applique en P26 comme en P27. Compagnie : une copie de `fictive-3090-grande`
//! (compagnie inventee). La table livree et la fiche se prouvent dans `src/naf.rs` et `src/champs.rs` ; un fichier P27V01
//! en NAF 2025 part sans code APE : `norme_par_fichier.rs`.

use crate::outils::{self, compagnie, ecrire, lire};
use paie::calcul::Paie;
use droit::contexte::Contexte;
use socle::date::Jour;
use paie::dsn::{generer, Options};
use paie::engagement::charger;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// `fictive-3090-grande`, sa surcouche de bareme 2026 valant aussi pour 2027 (comme `norme_par_fichier`), et la fiche
/// entreprise modifiee par `entreprise`.
fn copie(test: &str, entreprise: Value) -> PathBuf {
    let racine = compagnie::ses_dsn("fictive-3090-grande", &[], &outils::dossier_de_test("naf_2025", test));
    let mut b = lire(&racine, "config/baremes/2026.json");
    b["annee"] = json!(2027);
    b["socle"] = json!(2026);
    ecrire(&racine, "config/baremes/2027.json", &b);
    let mut sal = lire(&racine, "config/salaries.json");
    sal["NICOLET"]["sante_svp"]["niveau"] = json!("3");
    ecrire(&racine, "config/salaries.json", &sal);
    let mut ent = lire(&racine, "config/entreprise.json");
    for (k, v) in entreprise.as_object().unwrap() {
        ent[k] = v.clone();
    }
    ecrire(&racine, "config/entreprise.json", &ent);
    racine
}

fn mois_de_nicolet(ctx: &Contexte, mois: &str) -> Vec<Paie> {
    let s = json!({"mois": mois, "date_paiement": format!("{}-28", mois), "engagements": [
        {"salarie": "Nicolet", "nature": "cdi", "debut": "2024-03-01", "numero": "NI.24.01", "quotite_contrat": 151.67,
         "lignes": [{"rubrique": "mois", "quantite": 1, "taux": 2310.0, "heures": 151.67}]}
    ]});
    let chemin = outils::chemin_unique("naf-2025.json");
    std::fs::write(&chemin, s.to_string()).unwrap();
    charger(ctx, &chemin).unwrap().paies
}

fn dsn_du_mois(racine: &Path, mois: &str, constitution: &str) -> Result<String, String> {
    let ctx = Contexte::nouveau(racine, Some(Jour::lire(constitution).unwrap())).unwrap();
    let paies = mois_de_nicolet(&ctx, mois);
    let o = Options { date_constitution: Some(Jour::lire(constitution).unwrap()), ..Options::default() };
    generer(&ctx, &paies, mois, &[], &[], &o)
}

fn valeur(texte: &str, rubrique: &str) -> Option<String> {
    outils::valeur(&outils::paires(texte), rubrique)
}

#[test]
fn une_dsn_p27v01_refuse_un_code_naf_rev2_sans_choisir_le_code_2025() {
    let racine = copie("rev2-en-2027", json!({"naf": "9001Z"}));
    let e = dsn_du_mois(&racine, "2027-01", "2027-02-03").unwrap_err();
    assert!(e.starts_with("Code APE (NAF) de la fiche entreprise : « 9001Z » (Arts du spectacle vivant) est un code de la NAF rev. 2"), "{}", e);
    assert!(e.contains("decret n° 2025-736, art. 2 et 6") && e.contains("net-entreprises, 08/10/2026"), "{}", e);
    // Les deux codes vers lesquels le contenu passe, et ce qui y passe ; le logiciel ne choisit pas.
    assert!(e.contains("90.20Y (Activités de spectacle : à l’exception de la production de spectacle"), "{}", e);
    assert!(e.contains("90.39G (Activités de production et organisation d'évènements culturels : la production de spectacle)"), "{}", e);
    assert!(e.contains("C'est l'INSEE qui attribue le code") && e.contains("Configuration › Entreprise") && e.ends_with("Rien n'est ecrit."), "{}", e);
}

#[test]
fn une_dsn_p26v01_ecrit_le_code_de_la_fiche_dans_l_une_ou_l_autre_nomenclature() {
    // Avant la mise en production de janvier 2027, NAF rev. 2 ; apres, NAF 2025 « quelle que soit la version de norme » :
    // le jour n'est pas publie (D1), le moteur ne controle pas un fichier P26V01 et ecrit le code de la fiche.
    for code in ["9001Z", "9020Y"] {
        let racine = copie(&format!("p26-{}", code), json!({"naf": code}));
        let t = dsn_du_mois(&racine, "2026-12", "2027-01-05").unwrap();
        assert_eq!(valeur(&t, "S10.G00.00.006").as_deref(), Some("P26V01"));
        assert_eq!(valeur(&t, "S21.G00.06.003").as_deref(), Some(code));
    }
}

#[test]
fn un_etablissement_en_naf_rev2_refuse_aussi_en_p27v01() {
    let racine = copie(
        "etablissement-rev2",
        json!({"naf": "9020Y", "etablissements": [{"nic": "00099", "naf": "9004Z", "adresse": "1 RUE DE L'ESSAI",
                                                     "code_postal": "69001", "commune": "LYON",
                                                     "taux_atmp": {"valeur": 1.25, "code_risque": "923AD"}}]}),
    );
    let e = dsn_du_mois(&racine, "2027-01", "2027-02-03").unwrap_err();
    assert!(e.starts_with("Code APE (NAF) de l'etablissement 00099 : « 9004Z »"), "{}", e);
    assert!(e.contains("90.31G") && e.contains("90.31H"), "{}", e);
}
