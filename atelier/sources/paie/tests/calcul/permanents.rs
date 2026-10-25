//! `feries` (Paques, jours feries, chomes) et `permanents` (conges, fin de contrat, preavis et indemnites legales,
//! maintien de salaire) contre `permanents.json`. `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit l'oracle. Et le preavis d'un
//! CDD rompu avant son terme (`solde::duree_du_preavis`, C. trav. L1243-1 et L1243-2), et l'indemnite d'un preavis non
//! effectue, motif par motif (`solde::solde_de_tout_compte`, lot P1), sur `fictive-3090-petite`.

use crate::outils::{egal, fnv, lire_oracle, oracle_selon};
use droit::natures::{nature, Nature};
use socle::date::Jour;
use socle::feries::{self, Jours};
use socle::num::{Decimal, Resultat};
use droit::permanents::{self as p, OptionsMaintien};
use serde_json::{json, Value};
use std::str::FromStr;

fn lire() -> Value {
    lire_oracle("permanents.json")
}

fn jour(v: &Value) -> Jour {
    Jour::lire(v.as_str().unwrap()).unwrap()
}

fn dec(v: &Value) -> Decimal {
    Decimal::from_str(v.as_str().unwrap()).unwrap()
}

fn motif(v: &Value) -> Option<&str> {
    v.as_str()
}

fn nat(v: &Value) -> std::sync::Arc<Nature> {
    nature(Some(v.as_str().unwrap())).unwrap()
}

fn en_json(j: Jours) -> Value {
    Value::Array(j.into_iter().map(|(d, n)| json!([d.iso(), n])).collect())
}

fn dec_texte(v: Decimal) -> Value {
    Value::String(v.to_string())
}

fn resultat(r: Resultat<Value>) -> Value {
    match r {
        Ok(v) => v,
        Err(e) => json!({"err": e}),
    }
}

/// Rejoue une section : `[arguments, resultat]` par cas.
fn section(rel: &Value, nom: &str, f: impl Fn(&[Value]) -> Resultat<Value>) {
    let cas: Vec<Value> = rel[nom]
        .as_array()
        .unwrap_or_else(|| panic!("section {} absente", nom))
        .iter()
        .map(|c| json!([c[0], resultat(f(c[0].as_array().unwrap()))]))
        .collect();
    oracle_selon("permanents.json", &format!("/{}", nom), &Value::Array(cas), egal);
}

/// Une section `[annee, valeur]`.
fn par_annee(rel: &Value, nom: &str, f: impl Fn(i32) -> Value) {
    let cas: Vec<Value> = rel[nom].as_array().unwrap().iter().map(|c| json!([c[0], f(c[0].as_i64().unwrap() as i32)])).collect();
    oracle_selon("permanents.json", &format!("/{}", nom), &Value::Array(cas), egal);
}

fn option(v: &Value) -> Option<&Value> {
    if v.is_null() {
        None
    } else {
        Some(v)
    }
}

#[test]
fn paques_connues() {
    for (an, m, j) in [(2024, 3, 31), (2025, 4, 20), (2026, 4, 5), (2027, 3, 28)] {
        assert_eq!(feries::paques(an).unwrap(), Jour::nouveau(an, m, j).unwrap());
    }
    assert!(feries::paques(0).is_err(), "l'an 0 n'a pas de Paques");
}

#[test]
fn les_feries() {
    let rel = lire();
    par_annee(&rel, "paques", |an| json!(feries::paques(an).unwrap().iso()));
    par_annee(&rel, "feries", |an| en_json(feries::feries(an).unwrap()));
    par_annee(&rel, "chomes_de_droit", |an| en_json(feries::chomes_de_droit(an).unwrap()));
    section(&rel, "chomes", |a| feries::chomes(a[0].as_i64().unwrap() as i32, option(&a[1])).map(en_json));
    section(&rel, "chomes_entre", |a| feries::chomes_entre(&jour(&a[0]), &jour(&a[1]), option(&a[2])).map(en_json));
}

#[test]
fn rien_n_est_dit() {
    let rel = lire();
    let cas: Vec<Value> = rel["rien_n_est_dit"].as_array().unwrap().iter().map(|c| json!([c[0], feries::rien_n_est_dit(option(&c[0]))])).collect();
    assert_eq!(cas.len(), 12);
    oracle_selon("permanents.json", "/rien_n_est_dit", &Value::Array(cas), egal);
}

#[test]
fn les_conges() {
    let rel = lire();
    section(&rel, "arrondir_jours", |a| Ok(dec_texte(p::arrondir_jours(dec(&a[0])))));
    section(&rel, "plafonner", |a| p::plafonner(dec(&a[0])).map(dec_texte));
    section(&rel, "periode_de_reference", |a| {
        p::periode_de_reference(&jour(&a[0]), option(&a[1])).map(|(d, f)| json!([d.iso(), f.iso()]))
    });
    section(&rel, "jours_acquis", |a| p::jours_acquis(&jour(&a[0]), &jour(&a[1])).map(dec_texte));
    section(&rel, "indemnite_conges", |a| {
        p::indemnite_conges(dec(&a[0]), dec(&a[1]), dec(&a[2]), dec(&a[3]))
    });
}

#[test]
fn la_fin_du_contrat() {
    let rel = lire();
    section(&rel, "precarite_due", |a| {
        p::precarite_due(&nat(&a[0]), motif(&a[1])).map(|o| o.map_or(Value::Null, Value::String))
    });
    section(&rel, "indemnite_precarite", |a| {
        p::indemnite_precarite(dec(&a[0]), a[1].as_f64()).map(dec_texte)
    });
    section(&rel, "anciennete_mois", |a| Ok(dec_texte(p::anciennete_mois(&jour(&a[0]), &jour(&a[1])))));
    section(&rel, "preavis_mois", |a| {
        p::preavis_mois(dec(&a[0]), motif(&a[1]), false).map(|o| o.map_or(Value::Null, dec_texte))
    });
    section(&rel, "salaire_de_reference", |a| {
        let liste = |v: &Value| v.as_array().unwrap().iter().map(dec).collect::<Vec<_>>();
        Ok(p::salaire_de_reference(&liste(&a[0]), &liste(&a[1])))
    });
    section(&rel, "indemnite_licenciement", |a| {
        p::indemnite_licenciement(dec(&a[0]), dec(&a[1]), motif(&a[2]))
    });
    section(&rel, "indemnite_mise_a_la_retraite", |a| {
        p::indemnite_mise_a_la_retraite(dec(&a[0]), dec(&a[1]))
    });
    section(&rel, "indemnite_rupture_conventionnelle", |a| {
        p::indemnite_rupture_conventionnelle(dec(&a[0]), dec(&a[1]))
    });
    section(&rel, "indemnite_depart_retraite", |a| p::indemnite_depart_retraite(dec(&a[0]), dec(&a[1])));
    section(&rel, "rupture", |a| p::rupture(motif(&a[0])).map(|o| o.unwrap_or(Value::Null)));
    section(&rel, "code_52_permis", |a| {
        p::code_52_permis(motif(&a[0]), motif(&a[1])).map(|o| o.map_or(Value::Null, Value::String))
    });
}

fn tranches(t: Vec<p::Tranche>) -> Value {
    Value::Array(t.into_iter().map(|t| json!({"jours": t.jours, "taux": dec_texte(t.taux)})).collect())
}

fn options_maintien(dj: i64, at: bool, tr: bool) -> OptionsMaintien {
    OptionsMaintien { deja_indemnises: dj, accident_du_travail: at, accident_de_trajet: tr }
}

/// La meme ligne que `ligne()` du releve.
fn ligne(r: &Value) -> String {
    let tr: Vec<String> = r["tranches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            format!("{}:{}:{}", t["jours"], dec(&t["taux"]).normalize(), t["montant"].as_str().unwrap())
        })
        .collect();
    format!(
        "{},{},{},{},{},{}",
        r["carence"],
        r["jours_indemnisables"],
        r["jours_indemnises"],
        r["jours_au_dela_du_droit"],
        r["brut_avant_deduction"].as_str().unwrap(),
        tr.join(";")
    )
}

#[test]
fn le_maintien() {
    let rel = lire();
    section(&rel, "droit_au_maintien", |a| {
        let n = if a[0].is_null() { None } else { Some(nat(&a[0])) };
        p::droit_au_maintien(n.as_deref(), dec(&a[1])).map(|o| o.map_or(Value::Null, Value::String))
    });
    section(&rel, "jours_de_carence", |a| {
        p::jours_de_carence(a[0].as_bool().unwrap(), a[1].as_bool().unwrap()).map(|n| json!(n))
    });
    section(&rel, "duree_du_maintien", |a| p::duree_du_maintien(dec(&a[0])).map(tranches));
    // `[arguments, resultat, [deja_indemnises, accident_du_travail, accident_de_trajet]]`.
    let detail: Vec<Value> = rel["maintien_detail"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            let a = c[0].as_array().unwrap();
            let o = options_maintien(c[2][0].as_i64().unwrap(), c[2][1].as_bool().unwrap(), c[2][2].as_bool().unwrap());
            json!([c[0], resultat(p::maintien_de_salaire(dec(&a[0]), a[1].as_i64().unwrap(), dec(&a[2]), &o)), c[2]])
        })
        .collect();
    oracle_selon("permanents.json", "/maintien_detail", &Value::Array(detail), egal);

    // `[anciennete, accident_du_travail, accident_de_trajet, deja_indemnises, empreinte des 401 jours]`.
    let blocs: Vec<Value> = rel["maintien_blocs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| {
            let (anc, at, tr, dj) = (dec(&b[0]), b[1].as_bool().unwrap(), b[2].as_bool().unwrap(), b[3].as_i64().unwrap());
            let lignes: Vec<String> =
                (0..=400).map(|j| ligne(&p::maintien_de_salaire(anc, j, Decimal::from_str("137.45").unwrap(), &options_maintien(dj, at, tr)).unwrap())).collect();
            json!([b[0], b[1], b[2], b[3], fnv(&lignes.join("\n"))])
        })
        .collect();
    oracle_selon("permanents.json", "/maintien_blocs", &Value::Array(blocs), egal);
}

#[test]
fn le_solde_d_un_cdd_rompu_avant_son_terme_ne_paie_pas_le_preavis_du_licenciement() {
    // `solde::duree_du_preavis`, que le solde de tout compte et `paie preavis` lisent : un CDD de plus de deux ans rompu
    // pour faute grave (L1243-1) n'a aucun preavis ; rompu par le salarie (L1243-2), la loi en donne un en jours que le
    // moteur ne compte pas, sans la convention du licenciement. Textes : `code-du-travail/articles-20261013.md`.
    let ctx = droit::contexte::Contexte::nouveau(&crate::outils::fictive("fictive-3090-petite"), Some(Jour::lire("2026-08-31").unwrap())).unwrap();
    let e = |motif: &str| json!({"salarie": "LEMERCIER", "nature": "cdd", "debut": "2019-03-04", "fin": "2026-08-20", "motif_rupture": motif});
    let anciennete = p::anciennete_mois(&Jour::lire("2019-03-04").unwrap(), &Jour::lire("2026-08-20").unwrap());
    let faute = paie::solde::duree_du_preavis(&ctx, &e("095"), anciennete, Some("095")).unwrap();
    assert_eq!(faute.mois, Some(Decimal::ZERO));
    assert_eq!(faute.legal, Some(Decimal::ZERO));
    let cdi = paie::solde::duree_du_preavis(&ctx, &e("037"), anciennete, Some("037")).unwrap();
    assert_eq!((cdi.mois, cdi.legal, cdi.convention), (None, None, None));
    assert!(cdi.pourquoi.as_deref().is_some_and(|t| t.contains("L1243-2") && t.contains("preavis_mois")), "{:?}", cdi.pourquoi);
    // Le meme motif de force majeure sur un CDI ne lit pas la regle du CDD.
    assert_eq!(p::raison_sans_preavis(Some("083"), false).unwrap(), None);
    assert!(p::raison_sans_preavis(Some("083"), true).unwrap().is_some_and(|t| t.contains("L1243-4")));
}

// ------------------------------------------------ le preavis non effectue, motif par motif (lot P1)
// Textes : `code-du-travail/articles-20261013.md`, « Le preavis non effectue : ce que chaque fin de contrat en fait »,
// et `permanents-base.json`, `preavis.indemnite_compensatrice`.

/// Le solde de tout compte d'un contrat fini le 20/08/2026 pour `motif`, preavis declare non effectue (trois mois saisis,
/// au moins ce que la loi et la convention donnent), salaire de 2 000 EUR : les lignes, et les avis mis bout a bout.
fn solde_sans_preavis_effectue(nature: &str, motif: &str) -> (Vec<Value>, String) {
    let ctx = droit::contexte::Contexte::nouveau(&crate::outils::fictive("fictive-3090-petite"), Some(Jour::lire("2026-08-31").unwrap())).unwrap();
    let e = json!({"salarie": "LEMERCIER", "numero": "P1", "nature": nature, "debut": "2019-03-04", "fin": "2026-08-20",
                   "motif_rupture": motif, "salaire_mensuel": 2000, "preavis_non_effectue": true, "preavis_mois": 3});
    let (lignes, avis) = paie::solde::solde_de_tout_compte(&ctx, &e, "2026-08", &[]).unwrap();
    (lignes, avis.iter().map(|a| a.texte.clone()).collect::<Vec<_>>().join("\n"))
}

fn preavis_paye(lignes: &[Value]) -> Option<&Value> {
    lignes.iter().find(|l| l["rubrique"] == "indemnite_preavis")
}

#[test]
fn un_cdd_rompu_par_le_salarie_ne_paie_aucune_indemnite_de_preavis() {
    // 037 : L1243-2 donne au salarie un preavis a respecter, L1243-3 des dommages et interets a l'employeur ; rien au
    // Titre IV ne lui donne d'indemnite compensatrice, et L1234-5 est au chapitre du licenciement.
    let (lignes, avis) = solde_sans_preavis_effectue("cdd", "037");
    assert!(preavis_paye(&lignes).is_none(), "{:?}", lignes);
    assert!(avis.contains("AUCUNE indemnite compensatrice") && avis.contains("L1243-2"), "{}", avis);
}

#[test]
fn une_demission_ne_paie_aucune_indemnite_de_preavis() {
    // 059 : L1237-1 fixe l'existence et la duree du preavis du demissionnaire, rien de plus.
    let (lignes, avis) = solde_sans_preavis_effectue("cdi", "059");
    assert!(preavis_paye(&lignes).is_none(), "{:?}", lignes);
    assert!(avis.contains("AUCUNE indemnite compensatrice") && avis.contains("L1237-1"), "{}", avis);
}

#[test]
fn un_depart_a_la_retraite_ne_paie_aucune_indemnite_de_preavis() {
    // 039 : L1237-10 renvoie a L1234-1 pour la duree du preavis que le salarie respecte, pas a L1234-5.
    let (lignes, avis) = solde_sans_preavis_effectue("cdi", "039");
    assert!(preavis_paye(&lignes).is_none(), "{:?}", lignes);
    assert!(avis.contains("AUCUNE indemnite compensatrice") && avis.contains("L1237-10"), "{}", avis);
}

#[test]
fn un_licenciement_pour_inaptitude_non_professionnelle_ne_paie_aucune_indemnite_de_preavis() {
    // 091 : L1226-4, al. 3, « par derogation a l'article L. 1234-5 ».
    let (lignes, avis) = solde_sans_preavis_effectue("cdi", "091");
    assert!(preavis_paye(&lignes).is_none(), "{:?}", lignes);
    assert!(avis.contains("AUCUNE indemnite compensatrice") && avis.contains("L1226-4"), "{}", avis);
}

#[test]
fn un_licenciement_paie_le_preavis_non_effectue_au_titre_de_l1234_5() {
    // 020 : le salarie licencie qui n'execute pas son preavis, L1234-5 lui-meme. Trois mois de 2 000 EUR.
    let (lignes, avis) = solde_sans_preavis_effectue("cdi", "020");
    let l = preavis_paye(&lignes).unwrap_or_else(|| panic!("{:?}", lignes));
    assert_eq!((l["quantite"].as_f64(), l["taux"].as_f64()), (Some(3.0), Some(2000.0)));
    assert!(avis.contains("6000.00 EUR") && avis.contains("(L1234-5)") && !avis.contains("A trancher"), "{}", avis);
}

#[test]
fn un_licenciement_pour_inaptitude_professionnelle_paie_le_montant_de_l1234_5_par_l1226_14() {
    // 092 : L1226-14, une indemnite compensatrice « d'un montant egal a celui de l'indemnite compensatrice de preavis ».
    // Le meme montant, sur sa rubrique a lui (DSN 52 : 032, lot P2).
    let (lignes, avis) = solde_sans_preavis_effectue("cdi", "092");
    assert!(preavis_paye(&lignes).is_none(), "{:?}", lignes);
    let l = lignes.iter().find(|l| l["rubrique"] == "indemnite_preavis_inaptitude").unwrap_or_else(|| panic!("{:?}", lignes));
    assert_eq!((l["quantite"].as_f64(), l["taux"].as_f64()), (Some(3.0), Some(2000.0)));
    assert!(avis.contains("L1226-14"), "{}", avis);
}

#[test]
fn une_mise_a_la_retraite_paie_comme_avant_et_dit_qu_aucun_texte_ne_tranche() {
    // 038 : L1237-6 donne le preavis de L1234-1, sans renvoyer a L1234-5 ; aucun texte depose ne tranche.
    let (lignes, avis) = solde_sans_preavis_effectue("cdi", "038");
    assert!(preavis_paye(&lignes).is_some(), "{:?}", lignes);
    assert!(avis.contains("A trancher") && avis.contains("L1237-6"), "{}", avis);
}

// ------------------------------------------------ le delai de prevenance d'une fin d'essai (lot P2)
// L1221-25 et L1221-26 : `code-du-travail/articles-20261013.md` ; le decompte (une lecture) : `permanents-base.json`,
// `preavis.delai_de_prevenance._lecture`. 3 100 EUR par mois : 100 EUR par jour d'aout, 103,33 par jour de septembre.

/// Le solde d'un contrat `nature` commence le `debut`, fini le 20/08/2026 pour `motif`, la rupture notifiee le
/// `notification`, avec les `champs` en plus : les lignes et les avis, ou le refus.
fn solde_d_une_fin_d_essai(nature: &str, motif: &str, debut: &str, notification: &str, champs: Value) -> Result<(Vec<Value>, String), String> {
    let ctx = droit::contexte::Contexte::nouveau(&crate::outils::fictive("fictive-3090-petite"), Some(Jour::lire("2026-08-31").unwrap())).unwrap();
    let mut e = json!({"salarie": "LEMERCIER", "numero": "P2", "nature": nature, "debut": debut, "fin": "2026-08-20",
                       "motif_rupture": motif, "salaire_mensuel": 3100, "notification_rupture": notification});
    for (k, v) in champs.as_object().unwrap() {
        e[k] = v.clone();
    }
    let (lignes, avis) = paie::solde::solde_de_tout_compte(&ctx, &e, "2026-08", &[])?;
    Ok((lignes, avis.iter().map(|a| a.texte.clone()).collect::<Vec<_>>().join("\n")))
}

/// La ligne du delai de prevenance et son montant (quantite x taux, au centime).
fn prevenance(lignes: &[Value]) -> Option<(f64, f64)> {
    lignes.iter().find(|l| l["rubrique"] == "indemnite_delai_prevenance").map(|l| {
        let (q, t) = (l["quantite"].as_f64().unwrap(), l["taux"].as_f64().unwrap());
        (q, (q * t * 100.0).round() / 100.0)
    })
}

#[test]
fn une_fin_d_essai_sous_huit_jours_de_presence_doit_vingt_quatre_heures() {
    // 1° : sept jours de presence (14 au 20/08) ; le delai expire le 21, le contrat finit le 20 : un jour d'aout.
    let (lignes, avis) = solde_d_une_fin_d_essai("cdi", "034", "2026-08-14", "2026-08-20", json!({})).unwrap();
    assert_eq!(prevenance(&lignes).map(|(_, m)| m), Some(100.0), "{:?}", lignes);
    assert!(avis.contains("vingt-quatre heures") && avis.contains("1 jour(s) du delai") && avis.contains("100.00 EUR"), "{}", avis);
}

#[test]
fn une_fin_d_essai_de_huit_jours_a_un_mois_de_presence_doit_quarante_huit_heures() {
    // 2° : huit jours de presence (13 au 20/08), la borne : le delai expire le 22 ; deux jours d'aout.
    let (lignes, avis) = solde_d_une_fin_d_essai("cdi", "034", "2026-08-13", "2026-08-20", json!({})).unwrap();
    assert_eq!(prevenance(&lignes).map(|(_, m)| m), Some(200.0), "{:?}", lignes);
    assert!(avis.contains("quarante-huit heures"), "{}", avis);
    // Un mois de presence tout juste (21/07 au 20/08) : toujours quarante-huit heures, « apres un mois » est au-dela.
    let (lignes, avis) = solde_d_une_fin_d_essai("cdi", "034", "2026-07-21", "2026-08-20", json!({})).unwrap();
    assert_eq!(prevenance(&lignes).map(|(_, m)| m), Some(200.0), "{:?}", lignes);
    assert!(avis.contains("quarante-huit heures"), "{}", avis);
}

#[test]
fn une_fin_d_essai_apres_un_mois_de_presence_doit_deux_semaines() {
    // 3° : du 01/06 au 20/08 ; le delai expire le 03/09 : onze jours d'aout (1 100) et trois de septembre (310).
    let (lignes, avis) = solde_d_une_fin_d_essai("cdi", "034", "2026-06-01", "2026-08-20", json!({})).unwrap();
    let (q, m) = prevenance(&lignes).unwrap_or_else(|| panic!("{:?}", lignes));
    assert_eq!(m, 1410.0);
    assert!((q - (11.0 / 31.0 + 3.0 / 30.0)).abs() < 1e-9, "{}", q);
    assert!(avis.contains("deux semaines") && avis.contains("03/09/2026") && avis.contains("14 jour(s)"), "{}", avis);
    // Notifiee le 01/08 : le delai expire le 15/08, avant la fin du contrat ; rien a payer.
    let (lignes, avis) = solde_d_une_fin_d_essai("cdi", "034", "2026-06-01", "2026-08-01", json!({})).unwrap();
    assert!(prevenance(&lignes).is_none(), "{:?}", lignes);
    assert!(avis.contains("delai respecte"), "{}", avis);
}

#[test]
fn une_fin_d_essai_apres_trois_mois_de_presence_doit_un_mois() {
    // 4° : du 01/04, notifiee le 10/08 ; le delai expire le 10/09 : onze jours d'aout (1 100) et dix de septembre (1 033,33).
    let (lignes, avis) = solde_d_une_fin_d_essai("cdi", "034", "2026-04-01", "2026-08-10", json!({})).unwrap();
    assert_eq!(prevenance(&lignes).map(|(_, m)| m), Some(2133.33), "{:?}", lignes);
    assert!(avis.contains("un mois (L1221-25, 4°") && avis.contains("10/09/2026") && avis.contains("2133.33 EUR"), "{}", avis);
}

#[test]
fn une_fin_d_essai_a_l_initiative_du_salarie_ne_lui_donne_rien() {
    // 035 : L1221-26, le salarie respecte un delai ; aucun article ne lui donne d'indemnite, meme notifiee le dernier jour.
    let (lignes, _) = solde_d_une_fin_d_essai("cdi", "035", "2026-04-01", "2026-08-20", json!({})).unwrap();
    assert!(prevenance(&lignes).is_none() && preavis_paye(&lignes).is_none(), "{:?}", lignes);
}

#[test]
fn le_delai_de_prevenance_ne_se_devine_pas() {
    // Sans la date de notification, le moteur refuse ; pour un CDD, sans la duree d'essai stipulee aussi (L1221-25 : un
    // essai « d'au moins une semaine ») ; moins d'une semaine, ou une faute grave : rien a payer.
    let refus = solde_d_une_fin_d_essai("cdi", "034", "2026-06-01", "", json!({"notification_rupture": null})).unwrap_err();
    assert!(refus.contains("notification_rupture"), "{}", refus);
    let refus = solde_d_une_fin_d_essai("cdd", "034", "2026-06-01", "2026-08-20", json!({})).unwrap_err();
    assert!(refus.contains("periode_essai_jours"), "{}", refus);
    let (lignes, _) = solde_d_une_fin_d_essai("cdd", "034", "2026-08-14", "2026-08-20", json!({"periode_essai_jours": 6})).unwrap();
    assert!(prevenance(&lignes).is_none(), "{:?}", lignes);
    let (lignes, _) = solde_d_une_fin_d_essai("cdd", "034", "2026-08-13", "2026-08-20", json!({"periode_essai_jours": 7})).unwrap();
    assert_eq!(prevenance(&lignes).map(|(_, m)| m), Some(200.0), "{:?}", lignes);
    let (lignes, avis) = solde_d_une_fin_d_essai("cdi", "034", "2026-06-01", "2026-08-20", json!({"faute_grave": true})).unwrap();
    assert!(prevenance(&lignes).is_none() && avis.contains("faute grave"), "{:?}", lignes);
}
