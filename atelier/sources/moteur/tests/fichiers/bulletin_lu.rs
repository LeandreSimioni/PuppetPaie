//! `bulletin_lu` : une page de PDF devient un bulletin lu, tel qu'il est, par un seul lecteur pour tout logiciel.
//!
//! LES PDF SONT FICTIFS, fabriques par la fabrique (`bulletins_fictifs/mod.rs`, partage avec `routes::import`) (`texte_fixe`, positions du haut-gauche de la page) a la
//! maniere de deux logiciels : CoolPaie (A) et sPAIEctacle (B) ; le lecteur n'en connait aucune mise en page. Noms,
//! adresses, NIR, Siret, numeros d'objet, titres et montants sont inventes ; rien de `prive/`, aucun des PDF reels.
//!
//! Les formes : A cachets isoles + services de repetition, A cachets groupes (2021, avec un impot preleve), A
//! masterclass, A heures d'un « non cadre », A 2021 sans numero d'objet, B cachets isoles + cotisation facultative
//! + defraiement non soumis, B 2020 activite partielle (connue sans rubrique ; sans elle dans la table, libelles
//! inconnus gardes). Puis les refus : sans texte, sans en-tetes, ambigu, incomplet ; et un avertissement
//! d'incoherence.
//!
//! Les compteurs (cachets, heures, enseignement, jours) de chaque forme, confrontes aux cumuls du bulletin ;
//! un cumul qui ne colle pas (le bulletin l'emporte sur ce qu'il compte lui-meme, pas sur un total ou des cachets
//! sont convertis : un cachet groupe a 8 h) ; la table ne nomme que des rubriques du catalogue.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use moteur::import::bulletin_lu::{compteurs, lire, BulletinLu, Lecture};

use crate::bulletins_fictifs;
use bulletins_fictifs::*;

fn dossier(nom: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("bulletin-lu").join(nom);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

// ------------------------------------------------------------------------------------------- les preuves

fn lu(l: &Lecture) -> &BulletinLu {
    match &l.resultat {
        Ok(b) => b,
        Err(r) => panic!("page {} refusee : {}", l.page, r.message()),
    }
}

fn s(v: &Value, chemin: &str) -> Value {
    chemin.split('.').fold(v.clone(), |v, k| v.get(k).cloned().unwrap_or(Value::Null))
}

/// Les cotisations rendues, en tuples (famille, libelle, base, taux_e, montant_e, taux_s, montant_s).
fn cotisations(v: &Value) -> Vec<Vec<Value>> {
    v["cotisations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| ["famille", "libelle", "base", "taux_employeur", "montant_employeur", "taux_salarie", "montant_salarie"].iter().map(|k| c[k].clone()).collect())
        .collect()
}

/// Un montant de la table fictive tel que le lecteur le rend : « 1 512,40 » -> "1512.40", « 0.000 » -> "0.000".
fn dec(x: &str) -> Value {
    if x.is_empty() {
        return Value::Null;
    }
    let s = x.replace(' ', "");
    Value::String(if s.contains(',') { s.replace(',', ".") } else { s })
}

#[test]
fn bulletin_lu_coolpaie_cachets_et_services_de_repetition_chaque_champ() {
    let d = dossier("a-2026");
    let chemin = d.join("bulletins-2026.pdf");
    pdf(vec![page_a(&a_repetitions()), page_a(&a_masterclass()), page_a(&a_heures())], &chemin);
    let l = lire(&chemin).unwrap();
    assert_eq!(l.len(), 3);
    assert_eq!(l.iter().map(|x| x.page).collect::<Vec<_>>(), [1, 2, 3]);
    let b = lu(&l[0]);
    assert_eq!(b.avertissements, Vec::<String>::new());
    let v = b.to_json();
    let mut sans_cotisations = v.clone();
    sans_cotisations.as_object_mut().unwrap().remove("cotisations");
    assert_eq!(
        sans_cotisations,
        json!({
            "logiciel": "coolpaie",
            "numero": "7/2026",
            "entreprise": {
                "nom": "Compagnie des Essais", "adresse": ["12 rue Imaginaire", "99998 Nullepart"],
                "siret": "99999999900017", "naf": "9001Z", "urssaf": "999999999999",
                "convention": "IDCC n° 3090 - Spectacle vivant du secteur privé", "licence": "L-R-99-9",
            },
            "salarie": {
                "nom_complet": "BRIOCHE FANCHON", "nom": "BRIOCHE", "prenom": "FANCHON", "nir": "288117599900146",
                "naissance": "1988-11-04", "adresse": ["3 IMPASSE DES ESSAIS", "99999 VILLE FICTIVE"], "code": "7",
                "emploi": "ARTISTE DRAMATIQUE", "statut": "artiste", "abattement": "0", "conges_spectacles": "Z0000001",
            },
            "numero_objet": "999Z99999901",
            "spectacle": "Les Fictifs en Tournée",
            "analytique": null,
            "contrat": null,
            "debut": "2026-01-14",
            "fin": "2026-01-24",
            "dates_travaillees": [
                {"debut": "2026-01-14", "fin": "2026-01-14"}, {"debut": "2026-01-18", "fin": "2026-01-18"},
                {"debut": "2026-01-21", "fin": "2026-01-21"}, {"debut": "2026-01-23", "fin": "2026-01-24"},
            ],
            "jours_travailles": ["2026-01-14", "2026-01-18", "2026-01-21", "2026-01-23", "2026-01-24"],
            "lignes": [
                {"quantite": "5", "libelle": "Service(s) répét. 4h (contrat<5j)", "taux": "62.00", "montant": "310.00", "soumis": true, "rubrique": "repetition", "heures": "4"},
                {"quantite": "2", "libelle": "Cachets isolés", "taux": "140.00", "montant": "280.00", "soumis": true, "rubrique": "cachet", "heures": null},
            ],
            "brut": "590.00",
            "total_employeur": "255.53",
            "total_salarie": "112.18",
            "net_social": "477.82",
            "net_avant_impot": "477.82",
            "net_imposable": "491.90",
            "pas": {"base": "491.90", "taux": "0.000", "montant": "0.00", "cumul": "0.00"},
            "net_a_payer": "477.82",
            "cout_employeur": "845.53",
            "regle_le": "2026-01-24",
            "mode_reglement": "Virement",
            "cumuls": {
                "cachets": {"periode": "2.00", "cumul": "2.00"},
                "brut": {"periode": "590.00", "cumul": "590.00"},
                "plafond": {"periode": "1512.40", "cumul": "1512.40"},
                "cout_employeur": {"periode": "845.53", "cumul": "845.53"},
            },
            "avertissements": [],
        })
    );
    // Chaque cotisation, sous sa famille ; les titres ecrits sur deux rangs se recollent.
    let attendu: Vec<Vec<Value>> = COTISATIONS_A
        .iter()
        .enumerate()
        .filter(|(_, c)| !c.2.is_empty())
        .map(|(k, c)| {
            let base = match c.3 {
                "B" => "590,00",
                "C" => "579,68",
                autre => autre,
            };
            vec![json!(c.2), json!(libelle_lu(k)), dec(base), dec(c.4), dec(c.5), dec(c.6), dec(c.7)]
        })
        .collect();
    assert_eq!(cotisations(&v), attendu);
    assert_eq!(attendu.len(), 14);
    assert!(v["cotisations"].as_array().unwrap().iter().all(|c| c["hors_total"] == json!(false)));

    // La masterclass : l'enseignement du catalogue.
    let m = lu(&l[1]).to_json();
    assert_eq!(m["lignes"][1], json!({"quantite": "3", "libelle": "Masterclass / Action Culturelle", "taux": "85.00", "montant": "255.00", "soumis": true, "rubrique": "enseignement", "heures": null}));
    assert_eq!(m["brut"], json!("660.00"));
    assert_eq!(m["net_imposable"], json!("560.00"));
    assert_eq!(m["cumuls"]["cout_employeur"], json!({"periode": "915.53", "cumul": "1761.06"}));
    assert_eq!(lu(&l[1]).avertissements, Vec::<String>::new());

    // Les heures d'un non cadre : ni numero d'objet ni spectacle, le cumul des heures.
    let h = lu(&l[2]).to_json();
    assert_eq!(s(&h, "salarie.statut"), json!("non cadre"));
    assert_eq!(s(&h, "salarie.emploi"), json!("CHARGE DE PRODUCTION"));
    assert_eq!(h["numero_objet"], Value::Null);
    assert_eq!(h["spectacle"], Value::Null);
    assert_eq!(h["lignes"], json!([{"quantite": "40", "libelle": "Heures", "taux": "16.00", "montant": "640.00", "soumis": true, "rubrique": "heure", "heures": null}]));
    assert_eq!(h["cumuls"]["heures"], json!({"periode": "40.00", "cumul": "112.00"}));
    assert!(h["cumuls"].get("cachets").is_none());
    assert_eq!(h["jours_travailles"].as_array().unwrap().len(), 5);
    assert_eq!(lu(&l[2]).avertissements, Vec::<String>::new());
}

#[test]
fn bulletin_lu_spaiectacle_cachets_facultative_et_defraiement_non_soumis() {
    let d = dossier("b-2026");
    let chemin = d.join("[BP] 2026-04-03 FICTIF.pdf");
    pdf(vec![page_b(&b_cachets())], &chemin);
    let l = lire(&chemin).unwrap();
    assert_eq!(l.len(), 1);
    let b = lu(&l[0]);
    assert_eq!(b.avertissements, Vec::<String>::new());
    let v = b.to_json();
    let mut sans_cotisations = v.clone();
    sans_cotisations.as_object_mut().unwrap().remove("cotisations");
    assert_eq!(
        sans_cotisations,
        json!({
            "logiciel": "spaiectacle",
            "numero": null,
            "entreprise": {
                "nom": "Les Ateliers Fictifs", "adresse": ["Maison des Essais", "99998 Fictiveville"],
                "siret": "99999999900025", "naf": "9001Z", "urssaf": null,
                "convention": "Entreprises du secteur privé du spectacle vivant", "licence": null,
            },
            "salarie": {
                "nom_complet": "Fanchon BRIOCHE", "nom": "BRIOCHE", "prenom": "Fanchon", "nir": "288117599900146",
                "naissance": null, "adresse": ["3 impasse des Essais", "99999 FICTIVE", "FRANCE"], "code": null,
                "emploi": "Artiste dramatique", "statut": "Artiste", "abattement": null, "conges_spectacles": "Z0000001",
            },
            "numero_objet": "186Z99999901",
            "spectacle": null,
            "analytique": "FICTION",
            "contrat": null,
            "debut": "2026-04-02",
            "fin": "2026-04-03",
            "dates_travaillees": [],
            "jours_travailles": [],
            "lignes": [
                {"quantite": "3", "libelle": "Cachets isolés", "taux": "110.00", "montant": "330.00", "soumis": true, "rubrique": "cachet", "heures": null},
                {"quantite": "2", "libelle": "Défraiement non soumis", "taux": "18.50", "montant": "37.00", "soumis": false, "rubrique": "defraiement_forfait", "heures": null},
            ],
            "brut": "330.00",
            "total_employeur": "182.51",
            "total_salarie": "74.12",
            "net_social": "255.88",
            "net_avant_impot": "291.23",
            "net_imposable": "263.90",
            "pas": {"base": "263.90", "taux": "1.10", "montant": "2.90", "cumul": "2.90"},
            "net_a_payer": "288.33",
            "cout_employeur": "514.16",
            "regle_le": "2026-04-29",
            "mode_reglement": "virement",
            "cumuls": {
                "cachets": {"periode": "3.00", "cumul": "7.00"},
                "jours": {"periode": "2.00", "cumul": "6.00"},
                "brut": {"periode": "330.00", "cumul": "1120.00"},
            },
            "avertissements": [],
        })
    );
    let p = b_cachets();
    let attendu: Vec<Vec<Value>> = p
        .cotisations
        .iter()
        .chain(p.apres_total.iter())
        .filter(|c| !c.2.is_empty())
        .map(|c| vec![json!(c.2), json!(c.1), dec(c.3), Value::Null, dec(c.6), dec(c.4), dec(c.5)])
        .collect();
    assert_eq!(cotisations(&v), attendu);
    // La cotisation facultative est ecrite apres le total : hors de ce total, et le net se retrouve quand meme.
    let hors: Vec<bool> = v["cotisations"].as_array().unwrap().iter().map(|c| c["hors_total"].as_bool().unwrap()).collect();
    assert_eq!(hors.iter().filter(|h| **h).count(), 1);
    assert!(*hors.last().unwrap());

    // Le meme defraiement imprime au-dessus du brut : hors de l'assiette (`soumis: false` du catalogue), il n'est pas
    // dans le brut, et les deux controles se retrouvent sans avertissement.
    let mut p = b_cachets();
    p.lignes.push(p.non_soumis.remove(0));
    let dessus = d.join("dessus.pdf");
    pdf(vec![page_b(&p)], &dessus);
    let l = lire(&dessus).unwrap();
    let b = lu(&l[0]);
    assert_eq!(b.avertissements, Vec::<String>::new());
    assert_eq!((b.lignes[1].libelle.as_str(), b.lignes[1].soumis), ("Défraiement non soumis", false));
}

#[test]
fn bulletin_lu_un_ecart_est_un_avertissement_pas_une_correction() {
    let d = dossier("incoherent");
    let chemin = d.join("incoherent.pdf");
    let mut a = a_repetitions();
    a.brut = "600,00"; // les lignes font 590,00
    a.net = "487,82";
    a.net_a_payer = "487,80"; // le net avant impot moins un impot nul fait 487,82
    pdf(vec![page_a(&a)], &chemin);
    let l = lire(&chemin).unwrap();
    let b = lu(&l[0]);
    assert_eq!(
        b.avertissements,
        vec![
            "incohérent : le brut lu (600.00) n'est pas la somme des lignes soumises (590.00)",
            "incohérent : le net à payer lu (487.80) n'est pas le net avant impôt moins l'impôt prélevé (487.82)",
        ]
    );
    // Rien n'est corrige : les montants sont ceux de la page.
    assert_eq!(b.brut.to_string(), "600.00");
    assert_eq!(b.net_a_payer.to_string(), "487.80");
}

// ------------------------------------------------------------------------------------------------- les compteurs

/// Un dossier de compagnie vide : le catalogue livre, sans rubrique propre.
fn racine() -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("bulletin-lu").join("racine-vide");
    std::fs::create_dir_all(&d).unwrap();
    d
}


/// UN BULLETIN PAR CONTRAT, UNE COLONNE « PERIODE » POUR LE MOIS : deux contrats d'un salarie le meme mois (3 cachets,
/// puis un contrat sans cachet), chaque bulletin imprimant les 3 cachets du mois. Le contrat sans cachet en garde zero ;
/// la colonne ne l'emporte pas sur les lignes d'un des deux.
#[test]
fn bulletin_lu_la_colonne_du_mois_ne_se_met_pas_dans_chaque_contrat() {
    let d = dossier("colonne-du-mois");
    let chemin = d.join("b.pdf");
    pdf(vec![page_b(&b_cachets())], &chemin);
    let l = lire(&chemin).unwrap();
    let cachets = lu(&l[0]).clone();
    let mut sans_cachet = cachets.clone();
    sans_cachet.lignes.clear();
    sans_cachet.numero = Some("autre".into());
    assert!(cachets.cumuls.iter().any(|(n, c)| n == "cachets" && c.periode.is_some_and(|x| x.normalize().to_string() == "3")));
    let de = |b: &BulletinLu, frere: &BulletinLu| compteurs(b, &[frere], &racine()).unwrap().to_json();
    assert_eq!(de(&sans_cachet, &cachets)["cachets"], json!("0"));
    assert_eq!(de(&cachets, &sans_cachet)["cachets"], json!("3"));
    assert!(!de(&sans_cachet, &cachets)["avertissements"].to_string().contains("cachets :"));
}
