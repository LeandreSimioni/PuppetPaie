//! Corrections DSN, lot V4b : les rubriques que la norme 2027.1.1 ajoute ou change, ecrites ou refusees depuis les
//! textes, en P27V01 seulement (cahier technique 2027.1.1, `atelier/reference/norme-dsn/texte-2027/`).
//!
//! - `S21.G00.40.084` (proratisation du plafond a la quotite) : conditionnelle en mensuelle depuis le 2027.1.2 (journal
//!   de maintenance, evolutions 16 et 21), pour un temps partiel seulement. La proratisation est la regle (CSS L. 242-8 ;
//!   fiche net-entreprises 3369 : « par defaut ») ; l'etablissement peut y renoncer ; un artiste en est exclu
//!   (L. 242-10, 1° ; BOSS § 1090). `atelier/reference/sources/plafond-temps-partiel/LISEZ-MOI.md`.
//! - `S21.G00.40.086` et `S21.G00.41.068` (adhesion a une caisse de conges payes) : « A ne pas renseigner pour la
//!   version de norme P27V01 » (journal de maintenance 2027.1.2, evolutions 17 et 19).
//! - `S21.G00.40.012` : la quotite de reference « ne doit pas inclure les heures supplementaires structurelles » (p. 190).
//! - `S21.G00.58.001/.002` du montant net social : omises dans le cas nominal (fiche net-entreprises 2983), « en date de
//!   versement » dans une correction (p. 267-268) ; type 13.
//! - `S21.G00.65` : « absence a temps plein du salarie sans maintien de sa remuneration normale » (p. 285).
//!
//! Chaque cas se fait aussi en decembre 2026 (P26V01) : rien de tout cela n'y change. Sur la copie figee des
//! compagnies fictives (`tests/donnees/fictives/`, compagnies inventees), lue sans y rien ecrire : la compagnie retouchee et les
//! saisies s'ecrivent dans le dossier temporaire des tests.

use engagement::calcul::Paie;
use crate::outils::{self, base78, blocs_58, compagnie, dec, ecrire, lire, porte, versement};
use droit::contexte::Contexte;
use socle::date::Jour;
use paie::dsn::{generer, Options};
use engagement::charger;
use socle::num::{Decimal, Resultat};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Une copie de `fictive-3090-grande` dont la surcouche de bareme couvre 2027 sur le socle 2026 (comme
/// `norme_par_fichier.rs`), les permanents au niveau 3 de la sante (le minimum 2027 monte), et la fiche entreprise
/// retouchee par `entreprise`.
fn copie(test: &str, entreprise: Value) -> PathBuf {
    let racine = compagnie::ses_dsn("fictive-3090-grande", &[], &outils::dossier_de_test("rubriques_2027", test));
    let mut b = lire(&racine, "config/baremes/2026.json");
    b["annee"] = json!(2027);
    b["socle"] = json!(2026);
    ecrire(&racine, "config/baremes/2027.json", &b);
    let mut sal = lire(&racine, "config/salaries.json");
    for nom in ["NICOLET", "MONTANSIER"] {
        sal[nom]["sante_svp"]["niveau"] = json!("3");
    }
    ecrire(&racine, "config/salaries.json", &sal);
    let mut ent = lire(&racine, "config/entreprise.json");
    // Un fichier P27V01 ne part qu'avec un code NAF 2025 (`naf::verifier_pour`, lot R5b).
    ent["naf"] = json!("9020Y");
    for (k, v) in entreprise.as_object().unwrap() {
        ent[k] = v.clone();
    }
    ecrire(&racine, "config/entreprise.json", &ent);
    racine
}

fn contexte(racine: &Path) -> Contexte {
    Contexte::nouveau(racine, Some(Jour::lire("2027-02-03").unwrap())).unwrap()
}

fn saisie(ctx: &Contexte, s: Value) -> Resultat<Vec<Paie>> {
    let chemin = outils::chemin_unique("rubriques-2027.json");
    std::fs::write(&chemin, s.to_string()).unwrap();
    charger(ctx, &chemin).map(|s| s.paies)
}

/// Nicolet (CDI, technicien, non cadre) le mois `mois`, a la quotite `heures`, payee `salaire`, avec `autres` lignes.
/// Un contrat qui commence le 1er du mois (numero `NT.<mois>`) : son premier mois n'a pas de bloc 41 a declarer (le
/// contrat NI.24.01 de la compagnie fictive a ete declare a 151,67 h en aout 2026).
fn nicolet(mois: &str, heures: f64, salaire: f64, autres: Vec<Value>) -> Value {
    let mut lignes = vec![json!({"rubrique": "mois", "quantite": 1, "taux": salaire, "heures": heures})];
    lignes.extend(autres);
    json!({"salarie": "Nicolet", "nature": "cdi", "debut": format!("{}-01", mois), "numero": format!("NT.{}", mois),
           "quotite_contrat": heures, "lignes": lignes})
}

/// Deburau (artiste, CDDU de deux jours du 20 au 21 du mois, deux cachets) : un contrat declare a la caisse des Conges
/// Spectacles (`caisse_conges::affilie`).
fn deburau(mois: &str) -> Value {
    let (j1, j2) = (format!("{}-20", mois), format!("{}-21", mois));
    json!({"salarie": "Deburau", "spectacle": "266Z00910022", "debut": j1, "fin": j2, "numero": format!("DE.{}", mois),
           "date_paiement": j2, "lignes": [
               {"rubrique": "cachet", "quantite": 2, "taux": 300.0, "lieu": "Theatre fictif", "dates": [j1, j2]}]})
}

fn mois(ctx: &Contexte, m: &str, engagements: Vec<Value>) -> Resultat<Vec<Paie>> {
    saisie(ctx, json!({"mois": m, "date_paiement": format!("{}-28", m), "engagements": engagements}))
}

fn options(jour: &str) -> Options {
    Options { date_constitution: Some(Jour::lire(jour).unwrap()), ..Options::default() }
}

/// La DSN du mois `m` (janvier 2027 ou decembre 2026), constituee le 03/02/2027 ou le 05/01/2027.
fn dsn(ctx: &Contexte, paies: &[Paie], m: &str) -> Result<String, String> {
    let jour = if m == "2027-01" { "2027-02-03" } else { "2027-01-05" };
    generer(ctx, paies, m, &[], &[], &options(jour))
}

fn lignes(texte: &str) -> Vec<(String, String)> {
    outils::paires(texte)
}

fn valeur(texte: &str, rubrique: &str) -> Option<String> {
    outils::valeur(&lignes(texte), rubrique)
}

/// Les lignes du bloc 40 du contrat `numero` (de son 40.001 a la premiere rubrique hors du bloc 40).
fn bloc_40(texte: &str, numero: &str) -> Vec<(String, String)> {
    let l = lignes(texte);
    let mut i = 0;
    while i < l.len() {
        if l[i].0 == "S21.G00.40.001" {
            let fin = l[i..].iter().position(|(k, _)| !k.starts_with("S21.G00.40.")).map_or(l.len(), |p| i + p);
            let bloc = l[i..fin].to_vec();
            if bloc.iter().any(|(k, v)| k == "S21.G00.40.009" && v == numero) {
                return bloc;
            }
            i = fin;
        } else {
            i += 1;
        }
    }
    panic!("aucun bloc 40 du contrat {}", numero)
}

fn dans(bloc: &[(String, String)], r: &str) -> Option<String> {
    outils::valeur(bloc, r)
}

/// Le montant d'assiette (23.004) de la ligne du bordereau Urssaf de CTP `ctp` et de qualifiant `qualifiant`.
fn bordereau(texte: &str, ctp: &str, qualifiant: &str) -> Option<Decimal> {
    outils::ctp(texte, ctp, qualifiant)
}

fn base_au_bulletin(p: &Paie, id: &str) -> Decimal {
    p.lignes.iter().find(|l| l.id == id).unwrap_or_else(|| panic!("pas de ligne {} au bulletin", id)).base
}

// ------------------------------------------------------------------ S21.G00.40.084

#[test]
fn le_40_084_ne_s_ecrit_que_pour_un_temps_partiel_en_2027() {
    // Norme 2027.1.2 : 40.084 conditionnelle en mensuelle (journal de maintenance, evolution 21 : O -> C), CCH-12
    // retiree (evolution 16) ; « Pour un salarie a temps partiel [...] Pour un salarie en forfait jour reduit [...] »
    // (note differentielle, § 4.5). Un temps plein et un CDDU : rien (le lot V4b y ecrivait 02).
    let racine = copie("40-084", json!({}));
    let ctx = contexte(&racine);
    let paies = mois(&ctx, "2027-01", vec![nicolet("2027-01", 151.67, 2310.0, vec![]), deburau("2027-01")]).unwrap();
    let texte = dsn(&ctx, &paies, "2027-01").unwrap();
    assert_eq!(valeur(&texte, "S10.G00.00.006").as_deref(), Some("P27V01"));
    assert_eq!(dans(&bloc_40(&texte, "NT.2027-01"), "S21.G00.40.084"), None, "un temps plein");
    assert_eq!(dans(&bloc_40(&texte, "DE.2027-01"), "S21.G00.40.084"), None, "un contrat d'usage");
    assert!(!porte(&texte, "S21.G00.40.084") && !porte(&texte, "S21.G00.40.085"));
    paie::usages::verifier_fichier(&texte).unwrap();
    paie::formats::verifier_fichier(&texte).unwrap();
    // Decembre 2026 : la rubrique n'existe pas dans la table P26V01.
    let paies = mois(&ctx, "2026-12", vec![nicolet("2026-12", 151.67, 2310.0, vec![]), deburau("2026-12")]).unwrap();
    let texte = dsn(&ctx, &paies, "2026-12").unwrap();
    assert_eq!(valeur(&texte, "S10.G00.00.006").as_deref(), Some("P26V01"));
    assert!(!porte(&texte, "S21.G00.40.084"));
}

/// Le plafond, la base 02 et le CTP 100/921 d'un temps partiel de 121,34 h paye 3 600 en janvier 2027 et en decembre
/// 2026, quand le plafond est proratise. R. 242-2, I, dernier alinea : plafond x duree contractuelle / la plus courte
/// des durees de L. 3123-1 (151,67 h, la fiche ne dit pas d'autre duree collective). 4 005 x 121,34 / 151,67 =
/// 3 204,11 (BOSS § 760 : « Plafond mensuel x 80 % » ; fiche net-entreprises 3369 : base 02 a 3 204 pour 80 %).
fn temps_partiel_proratise(racine: &Path) {
    let ctx = contexte(racine);
    let paies = mois(&ctx, "2027-01", vec![nicolet("2027-01", 121.34, 3600.0, vec![])]).unwrap();
    let p = &paies[0];
    assert_eq!(p.plafond_urssaf(&ctx).unwrap(), dec("3204.11"), "le plafond du bas de bulletin");
    assert_eq!(base_au_bulletin(p, "vieil_plaf"), dec("3204.11"));
    assert_eq!(base_au_bulletin(p, "retraite_t1"), dec("3204.11"));
    let texte = dsn(&ctx, &paies, "2027-01").unwrap();
    let ni = bloc_40(&texte, "NT.2027-01");
    assert_eq!(dans(&ni, "S21.G00.40.084").as_deref(), Some("01"));
    // Ce que la caisse Agirc-Arrco relit pour proratiser elle-meme (cahier CT2026, l. 3325-3333 : plafond x 40.013 /
    // 40.012) : 4 005 x 121,34 / 151,67, le plafond du bulletin.
    assert_eq!((dans(&ni, "S21.G00.40.012").as_deref(), dans(&ni, "S21.G00.40.013").as_deref()), (Some("151.67"), Some("121.34")));
    assert_eq!(dans(&ni, "S21.G00.40.014").as_deref(), Some("20"));
    assert_eq!(base78(&versement(&texte, "NT.2027-01"), "02"), vec![dec("3204.11")]);
    // Le bordereau Urssaf (CTP 100, qualifiant 921 « assiette plafonnee ») suit le nominatif, arrondi a l'euro (guide
    // Urssaf, `bordereau.rs`) ; il porterait 3 600 sans la proratisation.
    assert_eq!(bordereau(&texte, "100", "921"), Some(dec("3204.00")), "{}", texte);
    paie::usages::verifier_fichier(&texte).unwrap();
    paie::formats::verifier_fichier(&texte).unwrap();
    // Decembre 2026 : le calcul est le meme (L. 242-8 vaut en 2026 comme en 2027) ; la DSN n'a pas de 40.084.
    let paies = mois(&ctx, "2026-12", vec![nicolet("2026-12", 121.34, 3600.0, vec![])]).unwrap();
    assert_eq!(paies[0].plafond_urssaf(&ctx).unwrap(), dec("3204.11"));
    let texte = dsn(&ctx, &paies, "2026-12").unwrap();
    assert!(!porte(&texte, "S21.G00.40.084"));
    assert_eq!(base78(&versement(&texte, "NT.2026-12"), "02"), vec![dec("3204.11")]);
    // Une heure payee en plus du mois : peut-etre une heure complementaire, que R. 242-2 ajoute a la duree du contrat.
    let heure = json!({"rubrique": "heure", "quantite": 4, "taux": 23.0});
    let e = mois(&ctx, "2027-01", vec![nicolet("2027-01", 121.34, 3600.0, vec![heure])]).unwrap_err();
    assert!(e.contains("heures complementaires") && e.contains("R. 242-2"), "{}", e);
}

#[test]
fn un_temps_partiel_est_proratise_quand_la_fiche_entreprise_ne_dit_rien() {
    // CSS L. 242-8 : « il est opere un abattement d'assiette » ; fiche net-entreprises 3369 : « Par defaut, le salarie a
    // temps partiel cotise uniquement a hauteur de la quotite de travail de son contrat. » Le lot V4b gardait le plafond
    // entier et refusait la DSN de 2027 tant que la fiche ne disait rien.
    temps_partiel_proratise(&copie("temps-partiel-par-defaut", json!({})));
}

#[test]
fn un_temps_partiel_proratise_corrige_le_plafond_du_bulletin_de_la_dsn_et_du_bordereau() {
    // « proratisé » garde le sens que le lot V4b lui donnait : la regle, dite.
    temps_partiel_proratise(&copie("temps-partiel-proratise", json!({"plafond_temps_partiel": "proratisé"})));
}

#[test]
fn un_temps_partiel_dont_l_etablissement_renonce_declare_02() {
    // Fiche 3369 : « L'employeur peut choisir de ne pas proratiser le plafond [...] pour tous les salaries a temps partiel
    // de son etablissement » ; « 02 - Non » et la base 02 non proratisee (4 005 dans l'exemple ; ici le brut, 3 600).
    let racine = copie("temps-partiel-entier", json!({"plafond_temps_partiel": "entier"}));
    let ctx = contexte(&racine);
    let paies = mois(&ctx, "2027-01", vec![nicolet("2027-01", 121.34, 3600.0, vec![])]).unwrap();
    assert_eq!(paies[0].plafond_urssaf(&ctx).unwrap(), dec("4005.00"));
    let texte = dsn(&ctx, &paies, "2027-01").unwrap();
    assert_eq!(dans(&bloc_40(&texte, "NT.2027-01"), "S21.G00.40.084").as_deref(), Some("02"));
    assert_eq!(base78(&versement(&texte, "NT.2027-01"), "02"), vec![dec("3600.00")]);
    // Une heure en plus du mois ne refuse pas : sans proratisation, il n'y a pas de duree a majorer.
    let heure = json!({"rubrique": "heure", "quantite": 4, "taux": 23.0});
    mois(&ctx, "2027-01", vec![nicolet("2027-01", 121.34, 3600.0, vec![heure])]).unwrap();
    // Un choix que la fiche ne connait pas refuse.
    let racine = copie("temps-partiel-faux", json!({"plafond_temps_partiel": "oui"}));
    let ctx = contexte(&racine);
    let e = mois(&ctx, "2027-01", vec![nicolet("2027-01", 121.34, 3600.0, vec![])]).unwrap_err();
    assert!(e.contains("plafond_temps_partiel") && e.contains("oui") && e.contains("L. 242-8"), "{}", e);
}

#[test]
fn un_artiste_a_temps_partiel_garde_le_plafond_entier() {
    // L. 242-10, 1° : L. 242-8 « n'est pas applicable » aux salaries dont l'emploi donne lieu a des taux ou assiettes
    // specifiques ; BOSS § 1090 : ils « ne sont pas eligibles a la reduction du plafond », « notamment [...] les artistes
    // du spectacle ». Nicolet devient artiste : plafond entier, sans renonciation de l'etablissement ; 40.084 = 02 (une
    // inference : ce que le calcul fait).
    let racine = copie("temps-partiel-artiste", json!({}));
    let chemin = racine.join("config/salaries.json");
    let mut sal: Value = serde_json::from_slice(&std::fs::read(&chemin).unwrap()).unwrap();
    sal["NICOLET"]["statut"] = json!("artiste");
    std::fs::write(&chemin, serde_json::to_vec_pretty(&sal).unwrap()).unwrap();
    let ctx = contexte(&racine);
    let paies = mois(&ctx, "2027-01", vec![nicolet("2027-01", 121.34, 3600.0, vec![])]).unwrap();
    assert_eq!(paies[0].plafond_urssaf(&ctx).unwrap(), dec("4005.00"));
    let texte = dsn(&ctx, &paies, "2027-01").unwrap();
    let ni = bloc_40(&texte, "NT.2027-01");
    assert_eq!(dans(&ni, "S21.G00.40.014").as_deref(), Some("20"));
    assert_eq!(dans(&ni, "S21.G00.40.084").as_deref(), Some("02"));
}

/// Les 79.004 des blocs 79 de type `type_79` d'un versement.
fn base79(v: &[(String, String)], type_79: &str) -> Vec<Decimal> {
    let mut courant = String::new();
    let mut out = Vec::new();
    for (k, x) in v {
        if k == "S21.G00.79.001" {
            courant = x.clone();
        } else if k == "S21.G00.79.004" && courant == type_79 {
            out.push(dec(x));
        }
    }
    out
}

#[test]
fn la_sante_d_un_temps_partiel_se_pose_sur_le_plafond_mensuel_entier() {
    // Accord sante 3090 du 19/07/2024, art. 13 : la cotisation est un pourcentage « du PMSS », au titre du seul salarie ;
    // CSS R. 242-1-6, 2°, c (repris par l'art. 9.2.1) : un temps partiel peut se dispenser quand cette cotisation
    // atteindrait 10 % de sa remuneration -- elle ne baisse donc pas avec son temps de travail. R. 242-2, I, ne corrige
    // que le plafond des cotisations plafonnees. La prevoyance, sur la tranche 1 (« le plafond [...] vise a l'article
    // L. 241-3 »), et la retraite suivent le plafond corrige. Avant le lot D1 : la sante sur 3 204,11.
    let racine = copie("sante-temps-partiel", json!({}));
    let ctx = contexte(&racine);
    for m in ["2027-01", "2026-12"] {
        let paies = mois(&ctx, m, vec![nicolet(m, 121.34, 3600.0, vec![])]).unwrap();
        let p = &paies[0];
        assert_eq!(p.plafond_urssaf(&ctx).unwrap(), dec("3204.11"), "{} : le plafond corrige reste celui du bulletin", m);
        assert_eq!(base_au_bulletin(p, "mutuelle"), dec("4005.00"), "{} : le forfait sante sur le PMSS", m);
        assert_eq!(p.bases["pss_periode"], dec("4005.00"));
        assert_eq!(base_au_bulletin(p, "retraite_t1"), dec("3204.11"));
        let prevoyance = p.lignes.iter().find(|l| l.id == "prevoyance").expect("une prevoyance au bulletin");
        assert!(prevoyance.base <= dec("3204.11"), "{} : la prevoyance sur la tranche 1 corrigee ({})", m, prevoyance.base);
        let texte = dsn(&ctx, &paies, m).unwrap();
        assert_eq!(base79(&versement(&texte, &format!("NT.{}", m)), "18"), vec![dec("4005.00")], "{} : base 18 du bloc 79", m);
        paie::formats::verifier_fichier(&texte).unwrap();
    }
    // Un temps plein : rien ne change, le plafond est le meme.
    let paies = mois(&ctx, "2027-01", vec![nicolet("2027-01", 151.67, 3600.0, vec![])]).unwrap();
    assert_eq!(base_au_bulletin(&paies[0], "mutuelle"), dec("4005.00"));
    assert_eq!(base_au_bulletin(&paies[0], "retraite_t1"), dec("3600.00"));
}

// ------------------------------------------------------------------ S21.G00.40.086

#[test]
fn ni_40_086_ni_41_068_ne_se_renseignent_en_p27v01() {
    // Journal de maintenance 2027.1.2, evolutions 17 et 19 : « A ne pas renseigner pour la version de norme P27V01 »
    // (le lot V4b ecrivait 40.086 = 01 sur le contrat declare a la caisse, d'apres le cahier 2027.1.1). Le tableau des
    // usages 2027.1.2 les garde en C : la donnee de la version les refuse (`rubriques_a_ne_pas_renseigner`).
    let racine = copie("40-086", json!({}));
    let ctx = contexte(&racine);
    let paies = mois(&ctx, "2027-01", vec![nicolet("2027-01", 151.67, 2310.0, vec![]), deburau("2027-01")]).unwrap();
    let texte = dsn(&ctx, &paies, "2027-01").unwrap();
    let de = bloc_40(&texte, "DE.2027-01");
    assert!(dans(&de, "S21.G00.40.022").is_some(), "le CDDU est declare a la caisse");
    assert!(!porte(&texte, "S21.G00.40.086") && !porte(&texte, "S21.G00.41.068"));
    paie::usages::verifier_fichier(&texte).unwrap();
    paie::formats::verifier_fichier(&texte).unwrap();
    // Un fichier P27V01 qui les porterait refuse avant le depot.
    for (apres, ligne) in [("S21.G00.40.022", "S21.G00.40.086,'01'"), ("S21.G00.40.022", "S21.G00.41.068,'01'")] {
        let mut l: Vec<String> = texte.lines().map(String::from).collect();
        let i = l.iter().position(|x| x.starts_with(apres)).unwrap();
        l.insert(i + 1, ligne.into());
        let e = paie::formats::verifier_fichier(&(l.join("\n") + "\n")).unwrap_err();
        assert!(e.contains(&ligne[..14]) && e.contains("a ne pas renseigner") && e.contains("P27V01"), "{}", e);
    }
    // Decembre 2026 : la rubrique n'existe pas.
    let paies = mois(&ctx, "2026-12", vec![deburau("2026-12")]).unwrap();
    let texte = dsn(&ctx, &paies, "2026-12").unwrap();
    assert!(porte(&texte, "S21.G00.40.022") && !porte(&texte, "S21.G00.40.086"));
}

// ------------------------------------------------------------------ S21.G00.40.012

#[test]
fn une_quotite_de_reference_au_dessus_de_la_duree_legale_refuse_en_2027() {
    // 2027 p. 190 : la quotite de reference « ne doit pas inclure les heures supplementaires structurelles » ; au-dela
    // de 35 heures par semaine (L. 3121-27), une heure est une heure supplementaire (L. 3121-28). Une compagnie a 39 h
    // qui declare 169 h refuse ; 151,67 et moins (32 h) passent.
    let racine = copie("40-012-169", json!({"quotite_reference": 169}));
    let ctx = contexte(&racine);
    let paies = mois(&ctx, "2027-01", vec![nicolet("2027-01", 169.0, 2600.0, vec![])]).unwrap();
    let e = dsn(&ctx, &paies, "2027-01").unwrap_err();
    assert!(e.contains("S21.G00.40.012") && e.contains("169") && e.contains("heures supplementaires structurelles") && e.contains("151.67"), "{}", e);
    // Decembre 2026 : la norme 2026 ne le disait pas ; rien ne change.
    let paies = mois(&ctx, "2026-12", vec![nicolet("2026-12", 169.0, 2600.0, vec![])]).unwrap();
    let texte = dsn(&ctx, &paies, "2026-12").unwrap();
    assert_eq!(dans(&bloc_40(&texte, "NT.2026-12"), "S21.G00.40.012").as_deref(), Some("169.00"));
    // 32 heures : 138,67 h, au-dessous de la duree legale.
    let racine = copie("40-012-32h", json!({"quotite_reference": 138.67}));
    let ctx = contexte(&racine);
    let paies = mois(&ctx, "2027-01", vec![nicolet("2027-01", 138.67, 2310.0, vec![])]).unwrap();
    let texte = dsn(&ctx, &paies, "2027-01").unwrap();
    let ni = bloc_40(&texte, "NT.2027-01");
    assert_eq!(dans(&ni, "S21.G00.40.012").as_deref(), Some("138.67"));
    assert_eq!(dans(&ni, "S21.G00.40.014").as_deref(), Some("10"), "temps plein a la duree collective");
}

#[test]
fn a_35_heures_dans_une_compagnie_a_39_heures_le_salarie_est_a_temps_plein() {
    // C. trav. L. 3123-1, 1° : temps partiel = une duree inferieure a la duree legale ou, si elle est plus courte, a la
    // duree conventionnelle. La caisse Agirc-Arrco proratise elle-meme le plafond d'un « 20 - temps partiel » sur
    // 40.013 / 40.012 (cahier CT2026, l. 3325-3343) : un 35 h declare « 20 » sous une reference de 169 h (admise en
    // P26V01) aurait son plafond reduit par la caisse a 151,67 / 169, alors que la paie garde le plafond entier.
    let racine = copie("35h-sur-39h", json!({"quotite_reference": 169}));
    let ctx = contexte(&racine);
    let paies = mois(&ctx, "2026-12", vec![nicolet("2026-12", 151.67, 4200.0, vec![])]).unwrap();
    assert_eq!(paies[0].plafond_urssaf(&ctx).unwrap(), dec("4005.00"));
    let texte = dsn(&ctx, &paies, "2026-12").unwrap();
    let ni = bloc_40(&texte, "NT.2026-12");
    assert_eq!(dans(&ni, "S21.G00.40.012").as_deref(), Some("169.00"));
    assert_eq!(dans(&ni, "S21.G00.40.013").as_deref(), Some("151.67"));
    assert_eq!(dans(&ni, "S21.G00.40.014").as_deref(), Some("10"), "35 h : la duree legale, un temps plein");
    assert_eq!(base78(&versement(&texte, "NT.2026-12"), "02"), vec![dec("4005.00")]);
    // 32 h sous la meme reference : un temps partiel, proratise a 138,67 / 151,67 (la plus courte des durees).
    let paies = mois(&ctx, "2026-12", vec![nicolet("2026-12", 138.67, 4200.0, vec![])]).unwrap();
    assert_eq!(paies[0].plafond_urssaf(&ctx).unwrap(), dec("3661.72"), "4 005 x 138,67 / 151,67");
    let texte = dsn(&ctx, &paies, "2026-12").unwrap();
    assert_eq!(dans(&bloc_40(&texte, "NT.2026-12"), "S21.G00.40.014").as_deref(), Some("20"));
}

// ------------------------------------------------------------------ S21.G00.58

#[test]
fn le_montant_net_social_du_mois_ne_se_date_pas_en_2027() {
    // Fiche net-entreprises 2983 (09/10/2026) : le bloc 58 « heritant de la datation du bloc Versement individu
    // (parent par defaut), les rubriques de datation 58.001 et 58.002 ne sont pas obligatoires » ; l'exemple du cas
    // nominal les laisse « Non renseignee ». Le cahier 2027.1.1 (p. 267-268) : « a renseigner des lors que sa datation
    // est differente de celle du Versement individu » ; le type 13 « suit la meme regle de datation ». Le lot V4b y
    // ecrivait la date de versement deux fois. 2026 p. 265 : « en periode de versement », ecrites.
    // Le type 13 (« Part exoneree de cotisations sociales des indemnites de rupture ») n'existe qu'en 2027 : une
    // indemnite legale de licenciement, hors assiette des cotisations, le porte.
    let racine = copie("58", json!({}));
    let ctx = contexte(&racine);
    let licenciement = json!({"rubrique": "indemnite_licenciement", "quantite": 1, "taux": 1000.0});
    let paies = mois(&ctx, "2027-01", vec![nicolet("2027-01", 151.67, 2310.0, vec![licenciement.clone()]), deburau("2027-01")]).unwrap();
    let texte = dsn(&ctx, &paies, "2027-01").unwrap();
    let ni = paies.iter().find(|p| p.engagement.numero.as_deref() == Some("NT.2027-01")).unwrap();
    let v = versement(&texte, "NT.2027-01");
    assert_eq!(dans(&v, "S21.G00.50.001").as_deref(), Some("31012027"));
    assert_eq!(
        blocs_58(&v),
        vec![("".into(), "".into(), "03".into(), format!("{:.2}", ni.net_social)), ("".into(), "".into(), "13".into(), "1000.00".into())]
    );
    let b = blocs_58(&versement(&texte, "DE.2027-01"));
    assert_eq!(b.len(), 1, "pas d'indemnite de rupture, pas de type 13");
    assert_eq!((b[0].0.as_str(), b[0].1.as_str(), b[0].2.as_str()), ("", "", "03"));
    assert!(!porte(&texte, "S21.G00.58.001") && !porte(&texte, "S21.G00.58.002"));
    paie::usages::verifier_fichier(&texte).unwrap();
    paie::formats::verifier_fichier(&texte).unwrap();
    // Decembre 2026 : la periode de versement, et pas de type 13.
    let paies = mois(&ctx, "2026-12", vec![nicolet("2026-12", 151.67, 2310.0, vec![licenciement]), deburau("2026-12")]).unwrap();
    let texte = dsn(&ctx, &paies, "2026-12").unwrap();
    let ni = paies.iter().find(|p| p.engagement.numero.as_deref() == Some("NT.2026-12")).unwrap();
    assert_eq!(
        blocs_58(&versement(&texte, "NT.2026-12")),
        vec![("01122026".into(), "31122026".into(), "03".into(), format!("{:.2}", ni.net_social))]
    );
    let b = blocs_58(&versement(&texte, "DE.2026-12"));
    assert_eq!((b[0].0.as_str(), b[0].1.as_str()), ("20122026", "21122026"));
}

#[test]
fn la_correction_d_un_montant_net_social_de_2027_se_date_au_versement_d_origine() {
    // Fiche 2983 : la correction d'un MNS d'un mois anterieur se rattache « avec la date du mois du versement auquel ce
    // MNS etait associe » (58.001/58.002), en annule-et-remplace ou en differentiel. Janvier 2027 est declare sans dates
    // (cas nominal) ; la correction portee en fevrier annule ce 58 et le redeclare, les deux dates du versement de
    // janvier (31/01/2027) : sans elles, l'annulation heriterait de la date du versement de la correction.
    let racine = copie("58-correction", json!({}));
    let ctx = contexte(&racine);
    let jan = mois(&ctx, "2027-01", vec![nicolet("2027-01", 151.67, 2310.0, vec![])]).unwrap();
    let texte = dsn(&ctx, &jan, "2027-01").unwrap();
    let depose = racine.join("DSN_Janvier2027.dsn");
    std::fs::write(&depose, &texte).unwrap();
    let sal: Value = serde_json::from_slice(&std::fs::read(racine.join("config/salaries.json")).unwrap()).unwrap();
    let nir = sal["NICOLET"]["nir"].as_str().unwrap().to_string();
    let declare = paie::versement_declare::lire(&depose, "2027-01", &nir, "NT.2027-01").unwrap();
    let net = jan[0].net_social;
    let r = paie::dsn::Regularisation {
        mois: "2027-01".into(),
        avant: jan[0].clone(),
        declare,
        apres: jan[0].clone(),
        taux_pas_origine: None,
        net_deja_verse: jan[0].net_a_payer,
        decidee_le: None,
    };
    let ctx = Contexte::nouveau(&racine, Some(Jour::lire("2027-03-03").unwrap())).unwrap();
    let fev = mois(&ctx, "2027-02", vec![nicolet("2027-02", 151.67, 2310.0, vec![])]).unwrap();
    let texte = generer(&ctx, &fev, "2027-02", &[r], &[], &options("2027-03-03")).unwrap();
    // Le versement du mois : pas de dates ; la correction : l'annulation et le montant redeclare, dates de janvier.
    assert_eq!(blocs_58(&versement(&texte, "NT.2027-02"))[0].0, "");
    let l = lignes(&texte);
    let i = l.iter().position(|(k, v)| k == "S21.G00.51.010" && v == "NT.2027-01").expect("la correction de janvier");
    let d = l[..i].iter().rposition(|(k, _)| k == "S21.G00.50.001").unwrap();
    assert_eq!(l[d].1, "28022027", "le versement de la correction est date du mois de fevrier");
    let fin = l[d + 1..].iter().position(|(k, _)| k == "S21.G00.50.001" || k.starts_with("S21.G00.6")).map_or(l.len(), |p| d + 1 + p);
    let j = "31012027".to_string();
    assert_eq!(
        blocs_58(&l[d..fin]),
        vec![(j.clone(), j.clone(), "03".into(), format!("{:.2}", -net)), (j.clone(), j.clone(), "03".into(), format!("{:.2}", net))]
    );
}

// ------------------------------------------------------------------ S21.G00.65

/// Une suspension de deux jours (12 et 13 du mois) au motif `motif`, saisie comme imprevu du contrat `NT.<mois>` de
/// Nicolet (le magasin des imprevus, `imprevus/AAAA-MM.json`) : c'est elle qui fait le bloc 65 (`imprevus::suspensions_du_mois`).
fn suspension(racine: &Path, mois: &str, motif: &str) {
    ecrire(
        racine,
        &format!("imprevus/{}.json", mois),
        &json!({"mois": mois, "imprevus": [{
            "genre": "suspension", "salarie": "NICOLET", "contrat_debut": format!("{}-01", mois), "contrat_numero": format!("NT.{}", mois),
            "motif": motif, "debut": format!("{}-12", mois), "fin": format!("{}-13", mois)}]}),
    );
}

/// La ligne d'une absence non remuneree de ces deux jours, ecrite a la main dans la saisie de Nicolet.
fn absence() -> Value {
    json!({"rubrique": "absence_non_remuneree", "quantite": 14, "taux": 15.23})
}

#[test]
fn un_conge_pour_evenement_familial_ne_se_declare_plus_en_suspension_en_2027() {
    // 2027 p. 285 : le bloc 65 declare « une suspension "totale" de l'execution du contrat : absence a temps plein du
    // salarie sans maintien de sa remuneration normale par son employeur ». Le conge pour evenement familial
    // « n'entraine pas de reduction de la remuneration » (C. trav. L. 3142-2) : `permanents-base.json` le dit remunere.
    // Saisi comme imprevu, un 637 ne retire rien du salaire, et 2027 ne le declare plus : refus, plutot que de taire
    // l'imprevu. Un 501 (conge non remunere) se declare toujours.
    let racine = copie("65", json!({}));
    let ctx = contexte(&racine);
    suspension(&racine, "2027-01", "637");
    let paies = mois(&ctx, "2027-01", vec![nicolet("2027-01", 151.67, 2310.0, vec![])]).unwrap();
    let e = dsn(&ctx, &paies, "2027-01").unwrap_err();
    assert!(e.contains("637") && e.contains("sans maintien de sa remuneration normale") && e.contains("L. 3142-2"), "{}", e);
    suspension(&racine, "2027-01", "501");
    let paies = mois(&ctx, "2027-01", vec![nicolet("2027-01", 151.67, 2310.0, vec![absence()])]).unwrap();
    let texte = dsn(&ctx, &paies, "2027-01").unwrap();
    assert_eq!(valeur(&texte, "S21.G00.65.001").as_deref(), Some("501"));
    // Decembre 2026 : la definition de 2026 ne parlait pas de remuneration ; le 637 se declare comme avant.
    std::fs::remove_file(racine.join("imprevus/2027-01.json")).unwrap();
    suspension(&racine, "2026-12", "637");
    let paies = mois(&ctx, "2026-12", vec![nicolet("2026-12", 151.67, 2310.0, vec![])]).unwrap();
    let texte = dsn(&ctx, &paies, "2026-12").unwrap();
    assert_eq!(valeur(&texte, "S21.G00.65.001").as_deref(), Some("637"));
}
