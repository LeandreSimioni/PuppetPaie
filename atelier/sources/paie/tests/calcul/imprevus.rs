//! Les imprevus d'un salarie (`imprevus`, 15/10/2026) : un arret ou une suspension saisi UNE fois, et le moteur en tire
//! la ligne de paie de chaque mois qu'il traverse ; un jour paye pendant un imprevu est nomme. Sur une copie de
//! `fictive-3090-grande` (config et saisies de juillet a octobre 2026 : Nicolet, CDI NI.24.01 a 2 310 EUR pour 151,67 h ;
//! Biancolelli, CDDU du 02 au 25/07/2026), dont le test ecrit le magasin `imprevus/`.

use crate::outils::{compagnie, dossier_de_test, ecrire};
use droit::contexte::Contexte;
use socle::date::Jour;
use socle::num::{eur2, mul, Decimal};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::str::FromStr;

fn compagnie(nom: &str) -> PathBuf {
    compagnie::saisies(
        "fictive-3090-grande",
        &["2026-07", "2026-08", "2026-09", "2026-10"],
        &dossier_de_test("calcul_imprevus", nom).join("compagnie"),
    )
}

fn jour(s: &str) -> Jour {
    Jour::lire(s).unwrap()
}

fn contexte(racine: &Path) -> Contexte {
    Contexte::nouveau(racine, Some(jour("2026-10-15"))).unwrap()
}

/// La paie de `salarie` dans la saisie du mois, calculee comme pour la DSN (`engagement::charger`).
fn paie(ctx: &Contexte, mois: &str, salarie: &str) -> engagement::calcul::Paie {
    let s = engagement::charger(ctx, &mois).unwrap();
    s.paies.into_iter().find(|p| p.salarie["_cle"] == json!(salarie)).unwrap()
}

fn lignes_de(p: &engagement::calcul::Paie, rubrique: &str) -> Vec<Value> {
    p.engagement.lignes.iter().filter(|l| l["rubrique"] == json!(rubrique)).cloned().collect()
}

fn d(v: &Value) -> Decimal {
    socle::num::dec(v).unwrap()
}

/// Les heures que retire une absence du `debut` au `fin` d'un temps plein de 151,67 h (35 h par semaine) : au reel,
/// 7 h par jour oeuvre, du lundi au vendredi moins les feries chomes (fiche DSN 2640, « Nb d'heures travaillees/jours
/// oeuvres x Nb de jours theoriquement oeuvres » ; fiche 2693, exemple 10 : 22 x 7 h ;
/// `atelier/reference/sources/suspension-non-remuneree/`).
fn heures(ctx: &Contexte, debut: &str, fin: &str) -> Decimal {
    let oeuvres = droit::conges::jours_oeuvres(&jour(debut), &jour(fin), Some(&ctx.entreprise)).unwrap();
    eur2(mul(Decimal::from(7), Decimal::from(oeuvres)))
}

fn arret_de_nicolet(maintien: Value) -> Value {
    json!({
        "genre": "arret", "salarie": "NICOLET", "contrat_debut": "2024-03-01", "contrat_numero": "NI.24.01", "motif": "01",
        "dernier_jour_travaille": "2026-07-14", "fin_previsionnelle": "2026-08-09", "subrogation": "02",
        "maintien": maintien, "envois": [],
    })
}

/// L'ARRET SAISI ENGENDRE LA LIGNE DE PAIE (15/10/2026) : la saisie du mois de Nicolet ne porte aucune absence, et
/// chaque mois que l'arret traverse retire le salaire de ses jours d'absence, du lendemain du dernier jour travaille
/// (S21.G00.60.002) a la fin previsionnelle (60.003), en heures au prorata des jours ouvrables. Le maintien de salaire
/// saisi sur l'arret pour juillet est verse en juillet ; non saisi pour aout, le moteur le dit (L1226-1, D1226-1 a
/// D1226-3, `atelier/reference/sources/code-du-travail/articles-20261015.md`).
#[test]
fn l_arret_d_un_permanent_retire_le_salaire_de_chaque_mois_qu_il_traverse() {
    let racine = compagnie("arret-permanent");
    ecrire(&racine, "imprevus/2026-07.json", &json!({"mois": "2026-07", "imprevus": [arret_de_nicolet(json!({"2026-07": 300.0}))]}));
    let ctx = contexte(&racine);

    let juillet = paie(&ctx, "2026-07", "NICOLET");
    let absence = lignes_de(&juillet, "absence_maladie");
    assert_eq!(absence.len(), 1, "{:?}", juillet.engagement.lignes);
    assert_eq!(d(&absence[0]["quantite"]), heures(&ctx, "2026-07-15", "2026-07-31"));
    let maintien = lignes_de(&juillet, "maintien_salaire");
    assert_eq!(maintien.len(), 1);
    // Le maintien saisi est verse au centime : son taux n'est pas arrondi avant de multiplier.
    let verse: Decimal = juillet.remuneration.iter().filter(|l| l.code == "maintien_salaire").map(|l| l.montant).sum();
    assert_eq!(verse, Decimal::from_str("300.00").unwrap());
    assert_eq!(juillet.heures_absence, d(&absence[0]["quantite"]));
    assert!(juillet.avertissements.iter().any(|a| a.texte.contains("arret de travail (maladie) du 15/07/2026 au 09/08/2026")));

    let aout = paie(&ctx, "2026-08", "NICOLET");
    let absence = lignes_de(&aout, "absence_maladie");
    assert_eq!(d(&absence[0]["quantite"]), heures(&ctx, "2026-08-01", "2026-08-09"));
    assert!(lignes_de(&aout, "maintien_salaire").is_empty(), "le maintien d'aout n'est pas saisi : rien n'est verse");
    assert!(aout.avertissements.iter().any(|a| !a.info && a.texte.contains("maintien de salaire de aout 2026 n'est pas saisi")), "{:?}", aout.avertissements);

    let septembre = paie(&ctx, "2026-09", "NICOLET");
    assert!(lignes_de(&septembre, "absence_maladie").is_empty(), "l'arret ne traverse pas septembre");
}

/// La reprise est le « premier jour travaille et paye suite a l'arret » (S21.G00.60.010, cahier technique DSN 2026.1,
/// p. 269, `atelier/reference/norme-dsn/texte/bloc-S21.G00.60.txt`) : repris le 01/08, l'arret ne retire rien en aout.
#[test]
fn un_arret_repris_le_premier_du_mois_ne_retire_rien_ce_mois_la() {
    let racine = compagnie("arret-repris");
    let mut a = arret_de_nicolet(json!({}));
    a["reprise"] = json!("2026-08-01");
    a["reprise_motif"] = json!("01");
    ecrire(&racine, "imprevus/2026-07.json", &json!({"imprevus": [a]}));
    let ctx = contexte(&racine);
    assert_eq!(lignes_de(&paie(&ctx, "2026-07", "NICOLET"), "absence_maladie").len(), 1);
    assert!(lignes_de(&paie(&ctx, "2026-08", "NICOLET"), "absence_maladie").is_empty());
}

fn suspension(motif: &str, debut: &str, fin: &str) -> Value {
    json!({
        "genre": "suspension", "salarie": "NICOLET", "contrat_debut": "2024-03-01", "contrat_numero": "NI.24.01",
        "motif": motif, "debut": debut, "fin": fin,
    })
}

/// UNE SUSPENSION SAISIE UNE FOIS, DECLAREE PARTOUT : « en cas de suspension supra-mensuelle, la suspension doit etre
/// declaree dans toutes les DSN mensuelles dont le Mois Principal declare presente une periode commune avec la suspension »
/// (bloc S21.G00.65, cahier technique DSN 2026.1, p. 281, `atelier/reference/norme-dsn/texte/bloc-S21.G00.65.txt`). Un conge
/// parental (632, non remunere) du 20/07 au 10/09 : chaque mois traverse en retire les jours, et chaque paie porte la
/// suspension ENTIERE, ses deux dates, pour le bloc 65 -- sans recopie. Octobre n'en porte rien.
#[test]
fn une_suspension_se_retire_et_se_declare_dans_chaque_mois_traverse() {
    let racine = compagnie("suspension");
    ecrire(&racine, "imprevus/2026-07.json", &json!({"imprevus": [suspension("632", "2026-07-20", "2026-09-10")]}));
    let ctx = contexte(&racine);
    for (mois, debut, fin) in [("2026-07", "2026-07-20", "2026-07-31"), ("2026-08", "2026-08-01", "2026-08-31"), ("2026-09", "2026-09-01", "2026-09-10")] {
        let p = paie(&ctx, mois, "NICOLET");
        let l = lignes_de(&p, "absence_non_remuneree");
        assert_eq!(l.len(), 1, "{} : {:?}", mois, p.engagement.lignes);
        assert_eq!(d(&l[0]["quantite"]), heures(&ctx, debut, fin), "{}", mois);
        assert_eq!(p.engagement.suspensions, vec![json!({"motif": "632", "debut": "2026-07-20", "fin": "2026-09-10"})], "{}", mois);
    }
    let octobre = paie(&ctx, "2026-10", "NICOLET");
    assert!(octobre.engagement.suspensions.is_empty() && lignes_de(&octobre, "absence_non_remuneree").is_empty());
}

/// Un conge pour evenement familial (637) « n'entraine pas de reduction de la remuneration » (C. trav. L3142-2,
/// `atelier/reference/sources/code-du-travail/articles-20261010.md`) : saisi comme suspension, il se declare (bloc 65 en
/// 2026) et ne retire rien du salaire. Le 200 (conges payes) ne se declare pas : la paie refuse.
#[test]
fn une_suspension_remuneree_ne_retire_rien_et_le_200_refuse() {
    let racine = compagnie("suspension-payee");
    ecrire(&racine, "imprevus/2026-07.json", &json!({"imprevus": [suspension("637", "2026-07-20", "2026-07-21")]}));
    let ctx = contexte(&racine);
    let p = paie(&ctx, "2026-07", "NICOLET");
    assert!(lignes_de(&p, "absence_non_remuneree").is_empty());
    assert_eq!(p.engagement.suspensions.len(), 1);
    ecrire(&racine, "imprevus/2026-07.json", &json!({"imprevus": [suspension("200", "2026-07-20", "2026-07-21")]}));
    let e = engagement::charger(&ctx, "2026-07").err().unwrap();
    assert!(e.contains("200"), "{}", e);
}

/// UN CACHET PAYE UN JOUR D'ARRET EST NOMME (regle 1 : le moteur montre, il ne retire rien). Un arret de travail est
/// « une suspension temporaire [...] durant laquelle le salarie ne peut exercer son activite » (bloc S21.G00.60, cahier
/// technique DSN 2026.1, p. 265) : Biancolelli, arrete du 10 au 12/07, a des cachets dates le 10 et le 11 (le 12
/// n'est pas joue).
#[test]
fn un_cachet_d_intermittent_un_jour_d_arret_est_un_avertissement_nomme() {
    let racine = compagnie("cachets");
    ecrire(
        &racine,
        "imprevus/2026-07.json",
        &json!({"imprevus": [{
            "genre": "arret", "salarie": "BIANCOLELLI", "contrat_debut": "2026-07-02", "contrat_numero": "", "motif": "01",
            "dernier_jour_travaille": "2026-07-09", "fin_previsionnelle": "2026-07-12", "subrogation": "02"}]}),
    );
    let ctx = contexte(&racine);
    let p = paie(&ctx, "2026-07", "BIANCOLELLI");
    let avis: Vec<&String> = p.avertissements.iter().filter(|a| !a.info && a.texte.contains("pendant l'arret de travail")).map(|a| &a.texte).collect();
    assert!(!avis.is_empty(), "{:?}", p.avertissements);
    assert!(avis.iter().any(|t| t.contains("Cachet le 10/07/2026, 11/07/2026 paye(s) pendant")), "{:?}", avis);
    assert!(lignes_de(&p, "absence_maladie").is_empty(), "un CDDU n'a pas de salaire du mois a retirer");
    // Et le moteur dit, ligne par ligne, quelles dates de la saisie tombent dans l'arret.
    let imp = engagement::imprevus::tous(&racine).unwrap().remove(0);
    let cachets = engagement::imprevus::cachets_dans(&ctx, &imp).unwrap();
    let dates: Vec<Value> = cachets.iter().flat_map(|c| c["dates"].as_array().unwrap().clone()).collect();
    assert_eq!(dates, vec![json!("2026-07-10"), json!("2026-07-11")], "le 12 n'est pas joue : {:?}", cachets);
    assert_eq!(cachets[0]["mois"], json!("2026-07"));
}

/// Deux imprevus du meme contrat qui se recouvrent retireraient deux fois le meme jour : le moteur les lit, le magasin
/// ecrit a la main peut les porter, mais les routes les refusent (`routes_imprevus::sans_chevauchement`, prouve dans
/// `tests/routes/routes_imprevus.rs`). Ici : un genre inconnu dans le magasin refuse la paie, plutot que d'etre tu.
#[test]
fn un_magasin_abime_refuse_la_paie() {
    let racine = compagnie("abime");
    ecrire(&racine, "imprevus/2026-07.json", &json!({"imprevus": [{"genre": "conge", "salarie": "NICOLET"}]}));
    let ctx = contexte(&racine);
    let e = engagement::charger(&ctx, "2026-07").err().unwrap();
    assert!(e.contains("genre"), "{}", e);
}
