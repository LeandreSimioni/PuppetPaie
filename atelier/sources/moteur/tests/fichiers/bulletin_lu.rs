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

use moteur::bulletin_lu::{compteurs, lire, lire_avec, BulletinLu, Lecture, Refus, Table};

use crate::bulletins_fictifs;
use bulletins_fictifs::*;

fn dossier(nom: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("bulletin-lu").join(nom);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

// ------------------------------------------------------------------------------------------- les preuves

/// La table livree, en JSON, pour en tirer des variantes.
fn table_livree_json() -> Value {
    let chemin = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../logiciel/donnees/bulletins-lecture-base.json");
    serde_json::from_str(&std::fs::read_to_string(chemin).unwrap()).unwrap()
}

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

    // La masterclass : l'enseignement du catalogue (lot C).
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
fn bulletin_lu_coolpaie_2021_sans_numero_d_objet_et_cachets_groupes() {
    let d = dossier("a-2021");
    let chemin = d.join("Bulletin de paie 2021.pdf");
    pdf(vec![page_a(&a_sans_objet()), page_a(&a_groupes())], &chemin);
    let l = lire(&chemin).unwrap();
    assert_eq!(l.len(), 2);

    let a = lu(&l[0]).to_json();
    assert_eq!(a["numero"], json!("1/2021"));
    assert_eq!(a["numero_objet"], Value::Null);
    assert_eq!(a["spectacle"], Value::Null, "« Spectacle vivant » de la convention n'est pas un titre");
    assert_eq!(a["mode_reglement"], Value::Null);
    assert_eq!(a["regle_le"], json!("2021-01-10"));
    assert_eq!((a["debut"].clone(), a["fin"].clone()), (json!("2021-01-09"), json!("2021-01-10")));
    assert_eq!(lu(&l[0]).avertissements, Vec::<String>::new());

    // Les cachets groupes : des cachets (lot C : 12 h depuis 2016) ; un impot preleve ; les milliers.
    let g = lu(&l[1]);
    assert_eq!(g.avertissements, Vec::<String>::new());
    let g = g.to_json();
    assert_eq!(g["lignes"], json!([{"quantite": "15", "libelle": "Cachets groupés", "taux": "160.00", "montant": "2400.00", "soumis": true, "rubrique": "cachet", "heures": null}]));
    assert_eq!(g["brut"], json!("2400.00"));
    assert_eq!(g["pas"], json!({"base": "2360.10", "taux": "1.500", "montant": "35.40", "cumul": "35.40"}));
    assert_eq!(g["net_avant_impot"], json!("2287.82"));
    assert_eq!(g["net_a_payer"], json!("2252.42"));
    assert_eq!(g["net_imposable"], json!("2360.10"));
    assert_eq!(g["cout_employeur"], json!("2655.53"));
    assert_eq!(g["cumuls"]["plafond"], json!({"periode": "2900.00", "cumul": "7000.00"}));
    assert_eq!(g["dates_travaillees"], json!([{"debut": "2021-12-02", "fin": "2021-12-05"}, {"debut": "2021-12-18", "fin": "2021-12-20"}]));
    assert_eq!(g["jours_travailles"].as_array().unwrap().len(), 7);
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
fn bulletin_lu_spaiectacle_2020_activite_partielle_libelles_inconnus_gardes() {
    let d = dossier("b-2020");
    let chemin = d.join("[BP] 2020-06-14 FICTIF.pdf");
    pdf(vec![page_b(&b_activite_partielle())], &chemin);
    // Avec la table livree, l'activite partielle est connue sans rubrique (`null`) : rien a signaler.
    let l = lire(&chemin).unwrap();
    assert_eq!(lu(&l[0]).avertissements, Vec::<String>::new());
    // Sans elle, trois libelles absents de la table : signales, les lignes gardees telles quelles, la page lue
    // (decide, plan §4 : pas de refus).
    let mut sans = table_livree_json();
    let lignes = sans["lignes"].as_object_mut().unwrap();
    lignes.retain(|k, _| !k.contains("activité partielle"));
    let l = lire_avec(&chemin, &Table::depuis(&sans).unwrap()).unwrap();
    let b = lu(&l[0]);
    assert_eq!(
        b.avertissements,
        vec![
            "libellé inconnu : « Cachet en activité partielle »",
            "libellé inconnu : « Absence cachet en activité partielle »",
            "libellé inconnu : « Indemnisation cachet en activité partielle »",
        ]
    );
    assert_eq!(b.to_json()["lignes"], lu(&lire(&chemin).unwrap()[0]).to_json()["lignes"]);
    let v = b.to_json();
    assert_eq!(
        v["lignes"],
        json!([
            {"quantite": "3", "libelle": "Cachet en activité partielle", "taux": "160.00", "montant": "480.00", "soumis": true, "rubrique": null, "heures": null},
            {"quantite": "3", "libelle": "Absence cachet en activité partielle", "taux": "-160.00", "montant": "-480.00", "soumis": true, "rubrique": null, "heures": null},
            {"quantite": "3", "libelle": "Indemnisation cachet en activité partielle", "taux": "112.00", "montant": "336.00", "soumis": false, "rubrique": null, "heures": null},
        ])
    );
    assert_eq!(v["brut"], json!("0.00"));
    assert_eq!(v["debut"], json!("2020-06-08"));
    assert_eq!(v["regle_le"], json!("2020-06-12"));
    assert_eq!(v["mode_reglement"], Value::Null);
    assert_eq!(v["numero_objet"], json!("177Z99999902"));
    assert_eq!(s(&v, "entreprise.convention"), json!("Entreprises Artistiques et Culturelles"));
    assert_eq!(s(&v, "entreprise.adresse"), json!(["2-4 rue du Général Fictif", "99997 FICTIVILLE"]));
    assert_eq!(s(&v, "salarie.conges_spectacles"), Value::Null);
    assert_eq!(s(&v, "salarie.emploi"), json!("Artiste Musicien"));
    assert_eq!(v["net_social"], Value::Null);
    // Un titre de famille sans montant, puis la meme ligne avec ses montants ; l'ecretement reste sous la CSG.
    let p = b_activite_partielle();
    let attendu: Vec<Vec<Value>> =
        p.cotisations.iter().filter(|c| !c.2.is_empty()).map(|c| vec![json!(c.2), json!(c.1), dec(c.3), Value::Null, dec(c.6), dec(c.4), dec(c.5)]).collect();
    assert_eq!(cotisations(&v), attendu);
    assert_eq!((v["total_salarie"].clone(), v["total_employeur"].clone()), (json!("0.10"), json!("53.50")));
    assert_eq!(v["net_avant_impot"], json!("335.90"));
    // Le PAS sans colonne de cumul ; le net imposable lu dans les cumuls ; le cout sous « Total verse par ».
    assert_eq!(v["pas"], json!({"base": "0.00", "taux": "0.00", "montant": "0.00", "cumul": null}));
    assert_eq!(v["net_imposable"], json!("335.98"));
    assert_eq!(v["net_a_payer"], json!("335.90"));
    assert_eq!(v["cout_employeur"], json!("53.50"));
    assert_eq!(
        v["cumuls"],
        json!({
            "jours": {"periode": "3.00", "cumul": "4.00"},
            "brut": {"periode": null, "cumul": null},
            "net_imposable": {"periode": "335.98", "cumul": "410.00"},
            "pas": {"periode": "0.00", "cumul": "0.00"},
        })
    );
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

#[test]
fn bulletin_lu_les_refus_sont_nommes() {
    let d = dossier("refus");
    // Sans texte : pas d'OCR.
    let traces = d.join("traces.pdf");
    std::fs::write(&traces, pdf_en_traces()).unwrap();
    let l = lire(&traces).unwrap();
    assert_eq!(l.len(), 1);
    let refus = l[0].resultat.clone().unwrap_err();
    assert!(matches!(refus, Refus::SansTexte(_)), "{:?}", refus);
    assert_eq!((l[0].to_json()["page"].clone(), l[0].to_json()["refus"].clone()), (json!(1), json!("sans_texte")));

    // Sans en-tetes : une page avec du texte, sans rang qui titre le tableau des lignes.
    let inconnu = d.join("inconnu.pdf");
    let mut b = Vec::new();
    t(&mut b, 50.0, 60.0, "Bulletin de salaire d'un autre logiciel");
    t(&mut b, 50.0, 80.0, "Salaire de base 1 000,00");
    pdf(vec![b], &inconnu);
    let l = lire(&inconnu).unwrap();
    assert!(matches!(l[0].resultat.clone().unwrap_err(), Refus::SansEntetes(_)));
    assert_eq!(l[0].to_json()["refus"], json!("sans_entetes"));

    // Ambigu : une colonne « Montant » qui ne dit pas quelle part elle porte ; on ne la devine pas.
    let ambigu = d.join("ambigu.pdf");
    let mut page = page_a(&a_repetitions());
    for x in page.iter_mut() {
        if x["texte"] == json!("Part Salarié") {
            x["texte"] = json!("Montant");
        }
    }
    pdf(vec![page], &ambigu);
    let l = lire(&ambigu).unwrap();
    assert!(matches!(l[0].resultat.clone().unwrap_err(), Refus::Ambigu(_)), "{:?}", l[0].resultat);
    // Le salarie s'est lu avant le refus : de quoi caviarder la page.
    assert_eq!(l[0].partiel.as_ref().and_then(|s| s.nir.clone()).as_deref(), Some("288117599900146"));

    // Un PDF que la fabrique n'ouvre pas : une erreur du fichier, pas d'une page.
    let casse = d.join("casse.pdf");
    std::fs::write(&casse, b"%PDF-1.7\nceci n'est pas un PDF\n").unwrap();
    assert!(lire(&casse).unwrap_err().starts_with("PDF que la fabrique n'ouvre pas"));

    // La somme versee non imprimee (un impot nul) : le net avant impot moins l'impot preleve, imprimes l'un et l'autre
    // (R. 3243-1, 9° et 10° ; le modele de l'arrete du 25/02/2016), et le compte rendu dit qu'elle a ete lue ainsi.
    let sans_net = d.join("sans-net.pdf");
    let mut page = page_a(&a_repetitions());
    page.retain(|x| !x["texte"].as_str().unwrap_or("").ends_with('€'));
    pdf(vec![page.clone()], &sans_net);
    let l = lire(&sans_net).unwrap();
    let b = lu(&l[0]);
    assert_eq!(b.net_a_payer.to_string(), "477.82");
    assert!(b.avertissements.iter().any(|a| a.starts_with("le net à payer n'est pas imprimé")), "{:?}", b.avertissements);

    // Incomplet : ni le net a payer, ni l'impot preleve qui le donnerait.
    page.retain(|x| x["texte"] != json!("0.00"));
    pdf(vec![page], &sans_net);
    let l = lire(&sans_net).unwrap();
    match l[0].resultat.clone().unwrap_err() {
        Refus::Incomplet(m) => assert!(m.contains("net à payer"), "{}", m),
        r => panic!("{:?}", r),
    }
}

// ------------------------------------------------------------------------------------------- lot C : les compteurs

/// Un dossier de compagnie vide : le catalogue livre, sans rubrique propre.
fn racine() -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("bulletin-lu").join("racine-vide");
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn compte(b: &BulletinLu) -> Value {
    compteurs(b, &racine()).unwrap().to_json()
}

/// B : 2019, un cachet groupe et des services de repetition ; son cumul « Heures » convertit le groupe a 8 h.
fn b_groupe_et_services() -> B {
    let mut p = b_cachets();
    p.periode = "du 23/04/19 au 27/04/19";
    p.periode_courte = "du 23 au 27/04/19";
    p.lignes = vec![("1", "Cachets groupés", "150,00", "150,00"), ("8", "Service de répétition 4 heures", "40,00", "320,00")];
    p.brut = ("Salaire brut", "470,00");
    p.non_soumis = vec![];
    p.net_avant_impot = "394,23"; // 470,00 - 74,12 - 1,65
    p.net_paye = "391,33";
    p.cumuls = vec![("Jours", "5,00", "9,00"), ("Heures", "40,00", "90,00"), ("Salaire brut", "470,00", "900,00")];
    p
}

/// B : des heures, sans cachet ; son cumul « Heures » en dit 28 pour 30 sur la ligne.
fn b_heures() -> B {
    let mut p = b_cachets();
    p.lignes = vec![("30", "Heures", "15,00", "450,00")];
    p.brut = ("MONTANT BRUT", "450,00");
    p.non_soumis = vec![];
    p.net_avant_impot = "374,23"; // 450,00 - 74,12 - 1,65
    p.net_paye = "371,33";
    p.cumuls = vec![("Jours", "4,00", "4,00"), ("Heures", "28,00", "28,00"), ("Salaire brut", "450,00", "450,00")];
    p
}

#[test]
fn bulletin_lu_compteurs_de_chaque_forme() {
    let d = dossier("compteurs");
    let chemin = d.join("formes.pdf");
    let pages = vec![
        page_a(&a_repetitions()),
        page_a(&a_groupes()),
        page_a(&a_masterclass()),
        page_a(&a_heures()),
        page_a(&a_sans_objet()),
        page_b(&b_cachets()),
        page_b(&b_activite_partielle()),
        page_b(&b_groupe_et_services()),
    ];
    pdf(pages, &chemin);
    let l = lire(&chemin).unwrap();
    for x in &l {
        assert_eq!(lu(x).avertissements, Vec::<String>::new(), "page {}", x.page);
    }
    let c = |k: usize| compte(lu(&l[k]));
    let sans = Vec::<String>::new();
    // A : 5 services de 4 h (le libelle) et 2 cachets ; les jours sont les dates travaillees ; le cumul « Cachets: » colle.
    assert_eq!(c(0), json!({"cachets": "2", "heures": "20", "heures_enseignement": "0", "jours": "5", "avertissements": sans}));
    // A : 15 cachets groupes sont 15 cachets (12 h chacun depuis 2016, au decompte).
    assert_eq!(c(1), json!({"cachets": "15", "heures": "0", "heures_enseignement": "0", "jours": "7", "avertissements": sans}));
    // A : la masterclass, 3 heures d'enseignement, dedans les heures, pas en plus.
    assert_eq!(c(2), json!({"cachets": "3", "heures": "3", "heures_enseignement": "3", "jours": "3", "avertissements": sans}));
    // A : 40 heures, et le cumul « Heures: » les dit.
    assert_eq!(c(3), json!({"cachets": "0", "heures": "40", "heures_enseignement": "0", "jours": "5", "avertissements": sans}));
    assert_eq!(c(4), json!({"cachets": "2", "heures": "0", "heures_enseignement": "0", "jours": "2", "avertissements": sans}));
    // B : pas de dates, les jours du cumul « Jours » ; le defraiement ne compte rien.
    assert_eq!(c(5), json!({"cachets": "3", "heures": "0", "heures_enseignement": "0", "jours": "2", "avertissements": sans}));
    // B : l'activite partielle, connue sans rubrique : la ligne reste, n'entre dans rien, et le dit.
    assert_eq!(
        c(6),
        json!({"cachets": "0", "heures": "0", "heures_enseignement": "0", "jours": "3", "avertissements": [
            "« Cachet en activité partielle » : sans rubrique du catalogue, la ligne n'entre dans aucun compteur",
            "« Absence cachet en activité partielle » : sans rubrique du catalogue, la ligne n'entre dans aucun compteur",
            "« Indemnisation cachet en activité partielle » : sans rubrique du catalogue, la ligne n'entre dans aucun compteur",
        ]})
    );
    // B 2019 : 1 cachet groupe et 8 services de 4 h. Le bulletin dit 40 h, cachets convertis a 8 h ; le decompte
    // dit 1 cachet (12 h) et 32 h. Un total ou des cachets sont convertis ne se decompose pas : avertissement, et
    // les compteurs restent ceux des lignes.
    assert_eq!(
        c(7),
        json!({"cachets": "1", "heures": "32", "heures_enseignement": "0", "jours": "5", "avertissements": [
            "heures : le bulletin en compte 40, cachets convertis compris ; ses lignes 44 (1 cachet(s) à 12 h et 32 h) ; un total où des cachets sont convertis ne se décompose pas : les compteurs restent ceux des lignes",
        ]})
    );
}

/// Un cumul qui ne colle pas : un avertissement, et le bulletin l'emporte sur ce qu'il compte lui-meme.
#[test]
fn bulletin_lu_un_cumul_qui_ne_colle_pas_est_un_avertissement() {
    let d = dossier("cumuls");
    let chemin = d.join("cumuls.pdf");
    let mut a = a_repetitions();
    a.cumuls[0] = ["3,00", "3,00"]; // 2 cachets sur les lignes
    pdf(vec![page_a(&a), page_b(&b_heures())], &chemin);
    let l = lire(&chemin).unwrap();
    assert_eq!(lu(&l[0]).avertissements, Vec::<String>::new());
    assert_eq!(lu(&l[1]).avertissements, Vec::<String>::new());
    assert_eq!(
        compte(lu(&l[0])),
        json!({"cachets": "3", "heures": "20", "heures_enseignement": "0", "jours": "5", "avertissements": [
            "cachets : le bulletin en compte 3, ses lignes 2 ; le bulletin l'emporte",
        ]})
    );
    // Des heures sans cachet : le total du bulletin est fait d'heures seulement, il l'emporte.
    assert_eq!(
        compte(lu(&l[1])),
        json!({"cachets": "0", "heures": "28", "heures_enseignement": "0", "jours": "4", "avertissements": [
            "heures : le bulletin en compte 28, ses lignes 30 ; le bulletin l'emporte",
        ]})
    );
}

/// Un libelle absent de la table : la ligne reste sans rubrique, sans erreur ; le cumul du bulletin compte quand meme.
#[test]
fn bulletin_lu_un_libelle_inconnu_reste_sans_rubrique() {
    let d = dossier("inconnu-c");
    let chemin = d.join("b.pdf");
    pdf(vec![page_b(&b_cachets())], &chemin);
    let mut sans = table_livree_json();
    sans["lignes"].as_object_mut().unwrap().remove("Cachets isolés");
    let l = lire_avec(&chemin, &Table::depuis(&sans).unwrap()).unwrap();
    let b = lu(&l[0]);
    assert_eq!(b.avertissements, vec!["libellé inconnu : « Cachets isolés »"]);
    assert_eq!(b.lignes[0].libelle, "Cachets isolés");
    assert_eq!(b.lignes[0].rubrique, None);
    assert_eq!(b.brut.to_string(), "330.00");
    assert_eq!(
        compte(b),
        json!({"cachets": "3", "heures": "0", "heures_enseignement": "0", "jours": "2", "avertissements": [
            "« Cachets isolés » : sans rubrique du catalogue, la ligne n'entre dans aucun compteur",
            "cachets : le bulletin en compte 3, ses lignes 0 ; le bulletin l'emporte",
        ]})
    );
    // Une rubrique que le catalogue ne connait pas : la lecture passe (la table ne lit pas le catalogue), le compte
    // refuse et la nomme.
    let mut fausse = table_livree_json();
    fausse["lignes"]["Cachets isolés"] = json!("cachet_devine");
    let l = lire_avec(&chemin, &Table::depuis(&fausse).unwrap()).unwrap();
    assert!(compteurs(lu(&l[0]), &racine()).unwrap_err().starts_with("Rubrique inconnue : 'cachet_devine'"));
}

/// La table ne nomme que des rubriques du catalogue, et ne dit des heures qu'aux rubriques qui les font saisir.
#[test]
fn bulletin_lu_la_table_ne_nomme_que_des_rubriques_du_catalogue() {
    let r = racine();
    let t = Table::livree().unwrap();
    let mut vues = Vec::new();
    for l in t.lignes() {
        let Some(code) = &l.rubrique else { continue };
        let rub = droit::catalogue::rubrique(&r, code, &socle::date::Jour::lire("2026-10-28").unwrap()).unwrap_or_else(|e| panic!("{} : {}", l.libelle, e));
        if l.heures.is_some() {
            assert!(rub.saisie.iter().any(|s| s == "heures") && rub.heures_forfait.is_none(), "{}", l.libelle);
        }
        vues.push((l.libelle.as_str(), code.as_str(), l.heures.map(|h| h.to_string())));
    }
    let quatre = Some("4".to_string());
    for attendu in [
        ("Cachets isolés", "cachet", None),
        ("Cachets groupés", "cachet", None),
        ("Service(s) répét. 4h (contrat<5j)", "repetition", quatre.clone()),
        ("Masterclass / Action Culturelle", "enseignement", None),
        ("Heures", "heure", None),
        ("Service de répétition 4 heures", "repetition", quatre.clone()),
        ("Défraiement non soumis", "defraiement_forfait", None),
        ("Acompte", "acompte", None),
    ] {
        assert!(vues.contains(&attendu), "{:?}", attendu);
    }
    // Les rubriques que les compteurs lisent : l'unite, le forfait du cachet, l'enseignement.
    assert_eq!(droit::catalogue::rubrique(&r, "cachet", &socle::date::Jour::lire("2026-10-28").unwrap()).unwrap().heures_forfait, Some(12.0));
    assert_eq!(droit::catalogue::rubrique(&r, "enseignement", &socle::date::Jour::lire("2026-10-28").unwrap()).unwrap().compte_pour, "ouverture_des_droits");

    // Une valeur de `lignes` mal formee est un refus de la table, qui la nomme.
    for (valeur, erreur) in [
        (json!(42), "lignes.Heures : une rubrique, null, ou {rubrique, heures}"),
        (json!({"heures": 4}), "lignes.Heures : `rubrique` manque"),
        (json!({"rubrique": "heure", "heures": -1}), "lignes.Heures : `heures` est un nombre d'heures positif"),
        (json!({"rubrique": "heure", "duree": 4}), "lignes.Heures : cle inconnue « duree » (rubrique, heures)"),
    ] {
        let mut v = table_livree_json();
        v["lignes"]["Heures"] = valeur;
        let e = Table::depuis(&v).unwrap_err();
        assert!(e.contains("lignes.Heures"), "le refus de {} nomme la ligne : {}", erreur, e);
    }
}
