//! La fin du contrat d'un permanent, au solde de tout compte (`solde`) sur `fictive-3090-petite` : le preavis d'un CDD
//! rompu avant son terme (C. trav. L1243-1 et L1243-2), l'indemnite d'un preavis non effectue motif par motif, le delai
//! de prevenance d'une fin d'essai (L1221-25, L1221-26). Textes : `code-du-travail/articles-20261013.md`.

use droit::permanents as p;
use serde_json::{json, Value};
use socle::date::Jour;
use socle::num::Decimal;

#[test]
fn le_solde_d_un_cdd_rompu_avant_son_terme_ne_paie_pas_le_preavis_du_licenciement() {
    // `solde::duree_du_preavis`, que le solde de tout compte et `paie preavis` lisent : un CDD de plus de deux ans rompu
    // pour faute grave (L1243-1) n'a aucun preavis ; rompu par le salarie (L1243-2), la loi en donne un en jours que le
    // moteur ne compte pas, sans la convention du licenciement. Textes : `code-du-travail/articles-20261013.md`.
    let ctx = droit::contexte::Contexte::nouveau(&crate::outils::fictive("fictive-3090-petite"), Some(Jour::lire("2026-08-31").unwrap())).unwrap();
    let e = |motif: &str| json!({"salarie": "LEMERCIER", "nature": "cdd", "debut": "2019-03-04", "fin": "2026-08-20", "motif_rupture": motif});
    let anciennete = p::anciennete_mois(&Jour::lire("2019-03-04").unwrap(), &Jour::lire("2026-08-20").unwrap());
    let faute = engagement::solde::duree_du_preavis(&ctx, &e("095"), anciennete, Some("095")).unwrap();
    assert_eq!(faute.mois, Some(Decimal::ZERO));
    assert_eq!(faute.legal, Some(Decimal::ZERO));
    let cdi = engagement::solde::duree_du_preavis(&ctx, &e("037"), anciennete, Some("037")).unwrap();
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
    let (lignes, avis) = engagement::solde::solde_de_tout_compte(&ctx, &e, "2026-08", &[]).unwrap();
    (lignes, avis.iter().map(|a| a.texte.clone()).collect::<Vec<_>>().join("\n"))
}

fn preavis_paye(lignes: &[Value]) -> Option<&Value> {
    lignes.iter().find(|l| l["rubrique"] == "indemnite_preavis")
}

#[test]
fn les_fins_de_contrat_qui_ne_paient_aucune_indemnite_de_preavis() {
    // 037 : L1243-2 donne au salarie en CDD un preavis a respecter, L1243-3 des dommages et interets a l'employeur ; rien
    // au Titre IV ne lui donne d'indemnite compensatrice. 059 : L1237-1 fixe l'existence et la duree du preavis du
    // demissionnaire, rien de plus. 039 : L1237-10 renvoie a L1234-1 pour la duree du preavis, pas a L1234-5. 091 :
    // L1226-4, al. 3, « par derogation a l'article L. 1234-5 ».
    for (nature, motif, article) in [("cdd", "037", "L1243-2"), ("cdi", "059", "L1237-1"), ("cdi", "039", "L1237-10"), ("cdi", "091", "L1226-4")] {
        let (lignes, avis) = solde_sans_preavis_effectue(nature, motif);
        assert!(preavis_paye(&lignes).is_none(), "{} : {:?}", motif, lignes);
        assert!(avis.contains("AUCUNE indemnite compensatrice") && avis.contains(article), "{} : {}", motif, avis);
    }
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
    let (lignes, avis) = engagement::solde::solde_de_tout_compte(&ctx, &e, "2026-08", &[])?;
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
fn le_delai_de_prevenance_d_une_fin_d_essai_suit_la_presence() {
    // L1221-25 : 1° sept jours de presence (14 au 20/08) ; le delai expire le 21, le contrat finit le 20 : un jour d'aout.
    let (lignes, avis) = solde_d_une_fin_d_essai("cdi", "034", "2026-08-14", "2026-08-20", json!({})).unwrap();
    assert_eq!(prevenance(&lignes).map(|(_, m)| m), Some(100.0), "{:?}", lignes);
    assert!(avis.contains("vingt-quatre heures") && avis.contains("1 jour(s) du delai") && avis.contains("100.00 EUR"), "{}", avis);
    // 2° : huit jours de presence (13 au 20/08), la borne : le delai expire le 22 ; deux jours d'aout. Un mois de presence
    // tout juste (21/07 au 20/08) : toujours quarante-huit heures, « apres un mois » est au-dela.
    for debut in ["2026-08-13", "2026-07-21"] {
        let (lignes, avis) = solde_d_une_fin_d_essai("cdi", "034", debut, "2026-08-20", json!({})).unwrap();
        assert_eq!(prevenance(&lignes).map(|(_, m)| m), Some(200.0), "{} : {:?}", debut, lignes);
        assert!(avis.contains("quarante-huit heures"), "{}", avis);
    }
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
