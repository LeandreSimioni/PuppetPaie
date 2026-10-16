//! LA CARTE FIGEE D'UNE PAIE VALIDEE (Leandre, 16/10/2026 : « quand on clique sur Valider, ca doit creer la carte simple
//! qui, elle, va etre affichee tout le temps sans passer par des calculs -- sauf dans les cas specifiques ou il y a des
//! changements de taux »). `valider-paie` fige, dans le `valide` de l'engagement, la vue de la paie et l'empreinte de ce
//! dont son calcul depend (`calcul_du_mois::sceller`) ; une paie validee dont l'empreinte tient se lit sans calcul ; un taux
//! qui change fait recalculer et le dit ; rouvrir defait la carte. Et ce qu'un engagement porte fait l'aller-retour par
//! la saisie sans rien perdre. Sur `fictive-3090-grande` (octobre 2026 : deux CDI, quatre CDDU).

use crate::outils::{compagnie, dossier_de_test, ecrire, lire, reussi};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const MOIS: &str = "2026-10";
const JOUR: &str = "2026-10-16";

fn compagnie(nom: &str) -> PathBuf {
    compagnie::sans_les_pdf("fictive-3090-grande", &dossier_de_test("carte_figee", nom).join("compagnie"))
}

fn route(racine: &Path, nom: &str, charge: Value) -> Value {
    crate::outils::route(racine, nom, charge, JOUR)
}

fn calcul(racine: &Path) -> PathBuf {
    racine.join("paies").join(format!("{}.calcul.json", MOIS))
}

/// Valide chaque paie du mois, dans l'ordre du fichier.
fn tout_valider(racine: &Path) {
    let saisie = reussi(&route(racine, "saisie", json!({"mois": MOIS})))["saisie"].clone();
    for e in saisie["engagements"].as_array().unwrap() {
        reussi(&route(racine, "valider-paie", json!({"mois": MOIS, "salarie": e["salarie"], "debut": e["debut"]})));
    }
}

/// L'apercu de la saisie enregistree, tel que l'etat garde du mois le lit.
fn apercu(racine: &Path) -> Value {
    reussi(&route(racine, "apercu", json!({"saisie": MOIS}))).clone()
}

#[test]
fn valider_fige_la_carte_que_l_apercu_montrait() {
    let racine = compagnie("fige");
    let avant = apercu(&racine);
    tout_valider(&racine);
    let e = lire(&racine, &format!("paies/{}.json", MOIS))["engagements"].clone();
    for (i, eng) in e.as_array().unwrap().iter().enumerate() {
        let v = &eng["valide"];
        assert!(v["le"].is_string(), "{}", v);
        let (carte, vue) = (&v["carte"], &avant["paies"][i]);
        for k in ["brut", "net_a_payer", "cout_employeur", "remuneration", "dates_travail", "lignes_saisies"] {
            assert_eq!(carte[k], vue[k], "{} : {}", k, eng["salarie"]);
        }
        // Les numeros se lisent en direct : ils ne sont pas dans la carte.
        assert!(carte.get("numero_bulletin").is_none() && carte.get("numero_contrat").is_none(), "{}", carte);
        assert!(v["empreinte"]["entrees"].as_object().is_some_and(|m| !m.is_empty()), "{}", v);
    }
    // La validation se lit telle qu'avant : son horodatage.
    let etat = reussi(&route(&racine, "etat-par-paie", json!({"mois": MOIS}))).clone();
    for (l, eng) in etat["paies"].as_array().unwrap().iter().zip(e.as_array().unwrap()) {
        assert_eq!(l["valide"], eng["valide"]["le"]);
    }
}

#[test]
fn une_paie_validee_se_lit_sans_calcul() {
    let racine = compagnie("sans-calcul");
    tout_valider(&racine);
    let cartes: Vec<Value> = lire(&racine, &format!("paies/{}.json", MOIS))["engagements"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["valide"]["carte"].clone())
        .collect();
    // Le calcul du mois est la trace d'un calcul : s'il fallait calculer pour montrer le mois, il se recreerait.
    let _ = std::fs::remove_file(calcul(&racine));
    let a = apercu(&racine);
    let etat = reussi(&route(&racine, "etat-par-paie", json!({"mois": MOIS}))).clone();
    let du_mois = reussi(&route(&racine, "mois", json!({"mois": MOIS}))).clone();
    assert!(!calcul(&racine).exists(), "un mois tout valide s'est calcule pour s'afficher");
    for (i, carte) in cartes.iter().enumerate() {
        assert_eq!(a["paies"][i]["net_a_payer"], carte["net_a_payer"]);
        assert!(a["paies"][i].get("revoir").is_none(), "{}", a["paies"][i]);
        assert!(a["paies"][i]["numero_bulletin"].is_string(), "les numeros du moment : {}", a["paies"][i]);
        assert_eq!(etat["paies"][i]["brut"], carte["brut"]);
    }
    // Le total du mois est la somme des cartes.
    let somme: f64 = cartes.iter().map(|c| c["brut"].as_str().unwrap().parse::<f64>().unwrap()).sum();
    assert_eq!(a["total_brut"].as_str().unwrap().parse::<f64>().unwrap(), (somme * 100.0).round() / 100.0);
    assert_eq!(du_mois["lectures"]["apercu"]["resultat"]["paies"], a["paies"]);
}

#[test]
fn un_taux_qui_change_fait_recalculer_et_le_dit() {
    let racine = compagnie("taux");
    tout_valider(&racine);
    let fichier = format!("paies/{}.json", MOIS);
    let saisie = lire(&racine, &fichier);
    // La surcouche de la compagnie : son taux AT/MP de l'annee (`calcul::bareme_atmp`).
    let octets = std::fs::read(racine.join("config/baremes/2026.json")).unwrap();
    let mut couche = lire(&racine, "config/baremes/2026.json");
    let ancien = couche["taux_atmp"]["valeur"].as_f64().unwrap();
    couche["taux_atmp"]["valeur"] = json!(ancien + 1.0);
    ecrire(&racine, "config/baremes/2026.json", &couche);
    let a = apercu(&racine);
    for (i, e) in saisie["engagements"].as_array().unwrap().iter().enumerate() {
        let (p, carte) = (&a["paies"][i], &e["valide"]["carte"]);
        // La carte reste celle qu'on a validee, et dit ce que le calcul donne maintenant.
        assert_eq!(p["cout_employeur"], carte["cout_employeur"], "{}", p);
        let revoir = &p["revoir"];
        assert!(revoir.is_object(), "un taux AT/MP plus haut change le cout : {}", p);
        let (valide, maintenant) =
            (carte["cout_employeur"].as_str().unwrap().parse::<f64>().unwrap(), revoir["cout_employeur"].as_str().unwrap().parse::<f64>().unwrap());
        assert!(maintenant > valide, "{} -> {}", valide, maintenant);
    }
    // Rien ne change en silence : la saisie, cartes comprises, est celle d'avant.
    assert_eq!(lire(&racine, &fichier), saisie);
    // La couche telle qu'a la validation : la carte tient de nouveau, sans calcul.
    std::fs::write(racine.join("config/baremes/2026.json"), octets).unwrap();
    let _ = std::fs::remove_file(calcul(&racine));
    let a = apercu(&racine);
    assert!(a["paies"].as_array().unwrap().iter().all(|p| p.get("revoir").is_none()), "{}", a);
    assert!(!calcul(&racine).exists());
}

#[test]
fn rouvrir_defait_la_carte() {
    let racine = compagnie("rouvrir");
    tout_valider(&racine);
    let fichier = format!("paies/{}.json", MOIS);
    let e = lire(&racine, &fichier)["engagements"][2].clone();
    reussi(&route(&racine, "valider-paie", json!({"mois": MOIS, "salarie": "BIANCOLELLI", "debut": e["debut"], "valide": false})));
    let rouverte = lire(&racine, &fichier)["engagements"][2].clone();
    assert!(rouverte.get("valide").is_none(), "{}", rouverte);
    assert!(rouverte["rouverte"].is_string(), "{}", rouverte);
    // Une paie de plus a calculer : le mois se calcule.
    let _ = std::fs::remove_file(calcul(&racine));
    apercu(&racine);
    assert!(calcul(&racine).exists());
}

/// Une saisie enregistree garde la carte du fichier, quoi que la page renvoie de `valide` : une paie se valide et se
/// rouvre par `valider-paie`, jamais par une saisie.
#[test]
fn la_saisie_ne_touche_pas_a_une_paie_validee() {
    let racine = compagnie("saisie-validee");
    tout_valider(&racine);
    let fichier = format!("paies/{}.json", MOIS);
    let avant = lire(&racine, &fichier);
    let mut s = reussi(&route(&racine, "saisie", json!({"mois": MOIS})))["saisie"].clone();
    s["engagements"][2]["valide"] = json!("2020-01-01T00:00:00");
    s["engagements"][2]["lignes"][0]["taux"] = json!(500);
    s["engagements"][3].as_object_mut().unwrap().remove("valide");
    reussi(&route(&racine, "enregistrer-saisie", json!({"mois": MOIS, "saisie": s})));
    let apres = lire(&racine, &fichier);
    assert_eq!(apres["engagements"][2], avant["engagements"][2]);
    assert_eq!(apres["engagements"][3], avant["engagements"][3]);
}

/// CE QU'UN ENGAGEMENT PEUT PORTER FAIT L'ALLER-RETOUR (16/10/2026, « ce qu'on voit est ce qui est enregistre ») : ecrit
/// par `enregistrer-saisie`, il est dans le fichier, et la route `saisie` le rend a l'ecran tel quel -- les champs que le
/// moteur lit sur un engagement (`engagement::engagements`, `conges::lignes_du_mois`, `solde.rs`, `periodes.rs`), ceux
/// d'une ligne (`lignes_affichees`) et d'une prime.
#[test]
fn chaque_champ_d_un_engagement_fait_l_aller_retour() {
    let racine = compagnie("aller-retour");
    let mut s = reussi(&route(&racine, "saisie", json!({"mois": MOIS})))["saisie"].clone();
    let rares = json!({
        "date_paiement": "2026-10-10", "lieu": "Theatre fictif de Saumur", "taux_cachet": 190, "jours": 4,
        "emploi": "COMEDIENNE", "pcs_ese": "354g", "code_emploi_ft": "COM010", "statut": "artiste",
        "statut_conventionnel": "07", "statut_categoriel_rc": "02", "quotite_reference": 12,
        "modalite_temps_travail": "10", "metier": "ART040", "cadre": true,
    });
    let cddu = s["engagements"][2].as_object_mut().unwrap();
    for (k, v) in rares.as_object().unwrap() {
        cddu.insert(k.clone(), v.clone());
    }
    cddu["lignes"][0]["dates"] = json!(["2026-10-06", {"jour": "2026-10-07", "lieu": "Angers"}, "2026-10-08", "2026-10-09"]);
    cddu["lignes"][0]["heures"] = json!(48);
    let cdi = s["engagements"][0].as_object_mut().unwrap();
    cdi.insert("primes".into(), json!([{"montant": 150, "libelle": "Prime de fin de tournee"}]));
    cdi.insert("conges_pris".into(), json!([{"debut": "2026-10-19", "fin": "2026-10-23"}]));
    // Un avenant en cours de mois, sur l'autre CDI : la paie se decoupe (`periodes.rs`), ce qui ne se fait que sur le
    // salaire mensuel seul.
    let avenant = s["engagements"][1].as_object_mut().unwrap();
    avenant.insert("date_modification".into(), json!("2026-10-16"));
    avenant.insert("avant".into(), json!({"salaire_mensuel": 2200, "quotite_contrat": 151.67}));
    let cdi = s["engagements"][0].as_object_mut().unwrap();
    cdi.insert("indemnite_licenciement".into(), json!(1200));
    cdi.insert("faute_grave".into(), json!(true));
    reussi(&route(&racine, "enregistrer-saisie", json!({"mois": MOIS, "saisie": s})));
    let ecrit = lire(&racine, &format!("paies/{}.json", MOIS));
    let relu = reussi(&route(&racine, "saisie", json!({"mois": MOIS})))["saisie"].clone();
    for e in [&ecrit, &relu] {
        let (cddu, cdi) = (&e["engagements"][2], &e["engagements"][0]);
        for (k, v) in rares.as_object().unwrap() {
            assert_eq!(&cddu[k], v, "{} : {}", k, cddu);
        }
        assert_eq!(cddu["lignes"][0]["dates"][1], json!({"jour": "2026-10-07", "lieu": "Angers"}), "{}", cddu);
        assert_eq!(cddu["lignes"][0]["heures"], json!(48), "{}", cddu);
        assert_eq!(cdi["primes"], json!([{"montant": 150, "libelle": "Prime de fin de tournee"}]), "{}", cdi);
        assert_eq!(cdi["conges_pris"], json!([{"debut": "2026-10-19", "fin": "2026-10-23"}]));
        assert_eq!(e["engagements"][1]["avant"], json!({"salaire_mensuel": 2200, "quotite_contrat": 151.67}));
        assert_eq!(e["engagements"][1]["date_modification"], json!("2026-10-16"));
        for k in ["indemnite_licenciement", "faute_grave"] {
            assert!(cdi[k] != Value::Null, "{} : {}", k, cdi);
        }
    }
}

/// La saisie sans le `valide` de ses engagements, ecrite telle qu'elle est lue (ordre des cles et ecriture des nombres compris).
fn sans_valide(saisie: &Value) -> String {
    let mut s = saisie.clone();
    for e in s["engagements"].as_array_mut().unwrap() {
        e.as_object_mut().unwrap().remove("valide");
    }
    s.to_string()
}

/// LES PAIES VALIDEES AVANT LA CARTE FIGEE (Leandre, 16/10/2026 : tout ce qui est deja sur le poste profite du gain, sans
/// rien rouvrir ni revalider) : un `valide` qui n'est qu'un horodatage recoit sa carte au premier passage du moteur sur le
/// mois (`calcul_du_mois::sceller_les_anciennes`), garde sa date, et seul `valide` change dans la saisie. Le passage
/// suivant ne calcule plus. `parti` : la DSN du mois est deja partie, et la paie se scelle de meme.
fn l_ancienne_validation_recoit_sa_carte(nom: &str, parti: bool) {
    let racine = compagnie(nom);
    let fichier = format!("paies/{}.json", MOIS);
    tout_valider(&racine);
    // La saisie telle qu'avant ce lot : chaque `valide` est un horodatage.
    let mut ancienne = lire(&racine, &fichier);
    let le: Vec<String> = (0..ancienne["engagements"].as_array().unwrap().len()).map(|i| format!("2026-10-0{}T09:30:00", i + 1)).collect();
    for (e, le) in ancienne["engagements"].as_array_mut().unwrap().iter_mut().zip(&le) {
        e["valide"] = json!(le);
    }
    ecrire(&racine, &fichier, &ancienne);
    if parti {
        crate::outils::dsn_deposee(&racine, MOIS, "dsn-fictive.dsn", "2026-10-15T10:00:00");
    }
    // Ce que l'ecran montrait de chaque paie, calculee.
    let montre = apercu(&racine);

    // Le premier passage : chaque paie recoit sa carte, a sa date de validation.
    reussi(&route(&racine, "mois", json!({"mois": MOIS})));
    let scellee = lire(&racine, &fichier);
    for (i, e) in scellee["engagements"].as_array().unwrap().iter().enumerate() {
        let v = &e["valide"];
        assert_eq!(v["le"], json!(le[i]), "{}", v);
        assert!(v["carte"].is_object() && v["empreinte"]["entrees"].is_object(), "{}", v);
        for k in ["brut", "net_a_payer", "cout_employeur"] {
            assert_eq!(v["carte"][k], montre["paies"][i][k], "{} : {}", k, e["salarie"]);
        }
    }
    // Rien d'autre de la saisie ne change : numeros de contrat compris.
    assert_eq!(sans_valide(&scellee), sans_valide(&ancienne));

    // Le passage suivant lit les cartes : le mois ne se calcule plus, et l'ecran montre la meme chose.
    let _ = std::fs::remove_file(calcul(&racine));
    let du_mois = reussi(&route(&racine, "mois", json!({"mois": MOIS}))).clone();
    assert!(!calcul(&racine).exists(), "une paie scellee s'est recalculee");
    let etat = reussi(&route(&racine, "etat-par-paie", json!({"mois": MOIS}))).clone();
    let relue = &du_mois["lectures"]["apercu"]["resultat"];
    for (i, l) in etat["paies"].as_array().unwrap().iter().enumerate() {
        assert_eq!(l["valide"], json!(le[i]), "{}", l);
        for k in ["brut", "net_a_payer", "cout_employeur", "numero_contrat", "numero_bulletin"] {
            assert_eq!(relue["paies"][i][k], montre["paies"][i][k], "{} : {}", k, l["salarie"]);
        }
        assert!(relue["paies"][i].get("revoir").is_none(), "{}", relue["paies"][i]);
    }
    assert_eq!(lire(&racine, &fichier), scellee);
}

#[test]
fn une_paie_validee_a_l_ancienne_recoit_sa_carte_au_premier_passage() {
    l_ancienne_validation_recoit_sa_carte("ancienne", false);
}

#[test]
fn une_paie_validee_a_l_ancienne_d_un_mois_parti_recoit_sa_carte() {
    l_ancienne_validation_recoit_sa_carte("ancienne-partie", true);
}
