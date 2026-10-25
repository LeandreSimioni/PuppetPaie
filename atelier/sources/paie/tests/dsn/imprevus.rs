//! Les imprevus dans la DSN mensuelle (`imprevus`, 15/10/2026) : l'arret saisi une fois sort en bloc 60, la suspension
//! saisie une fois sort en bloc 65 dans CHAQUE mois qu'elle traverse, sans recopie ; un imprevu dont le contrat n'est pas
//! declare ce mois-ci refuse la DSN. Sur une copie de `fictive-3090-grande` avec ses DSN deposees (`compagnie::ses_dsn`),
//! dont le test ecrit le magasin `imprevus/`.
//!
//! Le texte : « En cas de suspension supra-mensuelle, la suspension doit etre declaree dans toutes les DSN mensuelles dont
//! le Mois Principal declare presente une periode commune avec la suspension concernee » (cahier technique DSN 2026.1,
//! bloc S21.G00.60 p. 265 et bloc S21.G00.65 p. 281 : `atelier/reference/norme-dsn/texte/bloc-S21.G00.60.txt` et
//! `bloc-S21.G00.65.txt`).

use crate::outils::{self, compagnie, ecrire};
use paie::calcul::Paie;
use droit::contexte::Contexte;
use socle::date::Jour;
use paie::dsn::{generer, Arret, Options};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn copie(test: &str) -> PathBuf {
    compagnie::ses_dsn("fictive-3090-grande", &[], &outils::dossier_de_test("dsn_imprevus", test))
}

fn contexte(racine: &Path) -> Contexte {
    Contexte::nouveau(racine, Some(Jour::lire("2026-10-15").unwrap())).unwrap()
}

fn paies(ctx: &Contexte, mois: &str) -> Vec<Paie> {
    paie::engagement::charger(ctx, &ctx.racine.join(format!("paies/{}.json", mois))).unwrap().paies
}

/// Les arrets que la DSN de `mois` reporte, tires du magasin (ce que fait la production du mois).
fn arrets(racine: &Path, mois: &str) -> Vec<Arret> {
    paie::imprevus::du_mois(racine, mois, Some(paie::imprevus::ARRET)).unwrap().into_iter().map(|v| Arret { v }).collect()
}

fn dsn(ctx: &Contexte, paies: &[Paie], mois: &str) -> Result<String, String> {
    let o = Options { date_constitution: Some(Jour::lire("2026-10-15").unwrap()), envoi_test: true, ..Options::default() };
    generer(ctx, paies, mois, &[], &arrets(&ctx.racine, mois), &o)
}

fn valeurs(texte: &str, rubrique: &str) -> Vec<String> {
    outils::paires(texte).into_iter().filter(|(r, _)| r == rubrique).map(|(_, v)| v).collect()
}

fn suspension(debut: &str, fin: &str) -> Value {
    json!({
        "genre": "suspension", "salarie": "NICOLET", "contrat_debut": "2024-03-01", "contrat_numero": "NI.24.01",
        "motif": "632", "debut": debut, "fin": fin,
    })
}

#[test]
fn la_suspension_saisie_une_fois_se_declare_entiere_dans_chaque_mois_traverse() {
    let racine = copie("suspension");
    ecrire(&racine, "imprevus/2026-08.json", &json!({"mois": "2026-08", "imprevus": [suspension("2026-08-17", "2026-09-10")]}));
    let ctx = contexte(&racine);
    for mois in ["2026-08", "2026-09"] {
        let t = dsn(&ctx, &paies(&ctx, mois), mois).unwrap_or_else(|e| panic!("{} : {}", mois, e));
        assert_eq!(valeurs(&t, "S21.G00.65.001"), vec!["632"], "{}", mois);
        assert_eq!(valeurs(&t, "S21.G00.65.002"), vec!["17082026"], "{}", mois);
        assert_eq!(valeurs(&t, "S21.G00.65.003"), vec!["10092026"], "{}", mois);
    }
    // Juillet et octobre n'ont aucun jour commun avec elle : la paie de Nicolet n'en porte rien (la DSN d'octobre
    // attend les numeros de ses contrats neufs, que seule la production reserve).
    for mois in ["2026-07", "2026-10"] {
        let p = paies(&ctx, mois);
        let nicolet = p.iter().find(|p| p.salarie["_cle"] == json!("NICOLET")).unwrap();
        assert!(nicolet.engagement.suspensions.is_empty(), "{} n'a aucun jour commun avec la suspension", mois);
    }
}

#[test]
fn une_suspension_dont_le_contrat_n_est_pas_declare_refuse_la_dsn() {
    let racine = copie("suspension-orpheline");
    ecrire(&racine, "imprevus/2026-08.json", &json!({"imprevus": [suspension("2026-08-03", "2026-08-07")]}));
    let ctx = contexte(&racine);
    let sans_nicolet: Vec<Paie> = paies(&ctx, "2026-08").into_iter().filter(|p| p.salarie["_cle"] != json!("NICOLET")).collect();
    let e = dsn(&ctx, &sans_nicolet, "2026-08").unwrap_err();
    assert!(e.contains("Suspension a declarer") && e.contains("NICOLET") && e.contains("Rien n'est ecrit"), "{}", e);
}

/// L'arret d'un permanent saisi une fois : la paie en tire la ligne qui retire le salaire, et la DSN le bloc 60 sous son
/// contrat -- sans absence a taper dans le mois, et sans le refus « absence pour arret de travail [...] aucun arret saisi
/// en face » (decision du 20/09/2026), qui ne peut plus se produire : la ligne vient de l'arret.
#[test]
fn l_arret_saisi_une_fois_sort_en_bloc_60_sous_le_contrat_et_la_paie_le_retire() {
    let racine = copie("arret");
    ecrire(
        &racine,
        "imprevus/2026-07.json",
        &json!({"imprevus": [{
            "genre": "arret", "salarie": "NICOLET", "contrat_debut": "2024-03-01", "contrat_numero": "NI.24.01", "motif": "01",
            "dernier_jour_travaille": "2026-07-14", "fin_previsionnelle": "2026-08-09", "subrogation": "02"}]}),
    );
    let ctx = contexte(&racine);
    for mois in ["2026-07", "2026-08"] {
        let p = paies(&ctx, mois);
        let nicolet = p.iter().find(|p| p.salarie["_cle"] == json!("NICOLET")).unwrap();
        assert!(nicolet.engagement.lignes.iter().any(|l| l["rubrique"] == json!("absence_maladie")), "{}", mois);
        let t = dsn(&ctx, &p, mois).unwrap_or_else(|e| panic!("{} : {}", mois, e));
        assert_eq!(valeurs(&t, "S21.G00.60.001"), vec!["01"], "{}", mois);
        assert_eq!(valeurs(&t, "S21.G00.60.002"), vec!["14072026"], "{}", mois);
        assert_eq!(valeurs(&t, "S21.G00.60.003"), vec!["09082026"], "{}", mois);
    }
    let t = dsn(&ctx, &paies(&ctx, "2026-09"), "2026-09").unwrap();
    assert!(valeurs(&t, "S21.G00.60.001").is_empty());
}

/// « Le bloc Arret de travail se declare SOUS le contrat suspendu » : un arret dont le contrat manque a la DSN du mois la
/// refuse toujours.
#[test]
fn un_arret_dont_le_contrat_n_est_pas_declare_refuse_toujours_la_dsn() {
    let racine = copie("arret-orphelin");
    ecrire(
        &racine,
        "imprevus/2026-08.json",
        &json!({"imprevus": [{
            "genre": "arret", "salarie": "NICOLET", "contrat_debut": "2024-03-01", "contrat_numero": "NI.24.01", "motif": "01",
            "dernier_jour_travaille": "2026-08-10", "fin_previsionnelle": "2026-08-12", "subrogation": "02"}]}),
    );
    let ctx = contexte(&racine);
    let sans_nicolet: Vec<Paie> = paies(&ctx, "2026-08").into_iter().filter(|p| p.salarie["_cle"] != json!("NICOLET")).collect();
    let e = dsn(&ctx, &sans_nicolet, "2026-08").unwrap_err();
    assert!(e.contains("n'est pas declare ce mois-ci") && e.contains("S21.G00.60"), "{}", e);
}
