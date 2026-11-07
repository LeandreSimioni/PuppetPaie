//! Les regles des permanents (`droit::permanents` : conges payes, fin de contrat, preavis et indemnites legales,
//! maintien de salaire) contre les references `tests/donnees/permanents/<fonction>.json` : une ligne par cas, l'entree et
//! ce que le droit en rend. `PUPPETPAIE_REECRIRE_ORACLES=1` les reecrit.

use crate::outils::typee::{cas_de, jouer, Rendu};
use droit::natures::nature;
use droit::permanents as p;
use serde::{Deserialize, Serialize};
use socle::date::Jour;
use socle::num::Decimal;

/// Ce que `indemnite_conges` rend : les deux calculs et la regle retenue.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IndemniteDesJours {
    dixieme: Decimal,
    maintien: Decimal,
    retenu: Decimal,
    regle: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SalaireDeReference {
    moyenne_12_mois: Decimal,
    tiers_3_mois: Decimal,
    retenu: Decimal,
    regle: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IndemniteLegale {
    montant: Decimal,
    du: bool,
    pourquoi: Option<String>,
    annees: Option<Decimal>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlancherDeLaRupture {
    plancher: Option<Decimal>,
    annees: Decimal,
    pourquoi: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IndemniteDuDepart {
    montant: Decimal,
    du: bool,
    mois: Decimal,
    palier: Option<i64>,
    pourquoi: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Tranche {
    jours: i64,
    taux: Decimal,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fin {
    cle: String,
    rubrique: String,
}

fn legale(i: p::IndemniteLegale) -> IndemniteLegale {
    IndemniteLegale { montant: i.montant, du: i.du, pourquoi: i.pourquoi, annees: i.annees }
}

// Les entrees de chaque fonction : ses arguments, dans l'ordre.
type Nature = String;
type Motif = Option<String>;

fn arrondir_jours(a: &(Decimal,)) -> Decimal {
    p::arrondir_jours(a.0)
}
fn plafonner(a: &(Decimal,)) -> Rendu<Decimal> {
    p::plafonner(a.0).into()
}
fn periode_de_reference(a: &(Jour,)) -> Rendu<(Jour, Jour)> {
    p::periode_de_reference(&a.0).into()
}
fn jours_acquis(a: &(Jour, Jour)) -> Rendu<Decimal> {
    p::jours_acquis(&a.0, &a.1).into()
}
fn indemnite_conges(a: &(Decimal, Decimal, Decimal, Decimal)) -> Rendu<IndemniteDesJours> {
    p::indemnite_conges(a.0, a.1, a.2, a.3).map(|i| IndemniteDesJours { dixieme: i.dixieme, maintien: i.maintien, retenu: i.retenu, regle: i.regle.to_string() }).into()
}

#[test]
fn les_conges() {
    jouer("permanents/arrondir_jours.json", cas_de("permanents/arrondir_jours.json"), arrondir_jours);
    jouer("permanents/plafonner.json", cas_de("permanents/plafonner.json"), plafonner);
    jouer("permanents/periode_de_reference.json", cas_de("permanents/periode_de_reference.json"), periode_de_reference);
    jouer("permanents/jours_acquis.json", cas_de("permanents/jours_acquis.json"), jours_acquis);
    jouer("permanents/indemnite_conges.json", cas_de("permanents/indemnite_conges.json"), indemnite_conges);
}

fn precarite_due(a: &(Nature, Motif)) -> Rendu<Option<String>> {
    p::precarite_due(&nature(&a.0).unwrap(), a.1.as_deref()).into()
}
fn indemnite_precarite(a: &(Decimal,)) -> Rendu<Decimal> {
    p::indemnite_precarite(a.0).into()
}
fn anciennete_mois(a: &(Jour, Jour)) -> Decimal {
    p::anciennete_mois(&a.0, &a.1)
}
fn preavis_mois(a: &(Decimal, Motif)) -> Rendu<Option<Decimal>> {
    p::preavis_mois(a.0, a.1.as_deref(), false).into()
}
fn salaire_de_reference(a: &(Vec<Decimal>, Vec<Decimal>)) -> SalaireDeReference {
    let s = p::salaire_de_reference(&a.0, &a.1);
    SalaireDeReference { moyenne_12_mois: s.moyenne_12_mois, tiers_3_mois: s.tiers_3_mois, retenu: s.retenu, regle: s.regle.to_string() }
}
fn indemnite_licenciement(a: &(Decimal, Decimal, Motif)) -> Rendu<IndemniteLegale> {
    p::indemnite_licenciement(a.0, a.1, a.2.as_deref()).map(legale).into()
}
fn indemnite_mise_a_la_retraite(a: &(Decimal, Decimal)) -> Rendu<IndemniteLegale> {
    p::indemnite_mise_a_la_retraite(a.0, a.1).map(legale).into()
}
fn indemnite_rupture_conventionnelle(a: &(Decimal, Decimal)) -> Rendu<PlancherDeLaRupture> {
    p::indemnite_rupture_conventionnelle(a.0, a.1).map(|i| PlancherDeLaRupture { plancher: i.plancher, annees: i.annees, pourquoi: i.pourquoi }).into()
}
fn indemnite_depart_retraite(a: &(Decimal, Decimal)) -> Rendu<IndemniteDuDepart> {
    p::indemnite_depart_retraite(a.0, a.1).map(|i| IndemniteDuDepart { montant: i.montant, du: i.du, mois: i.mois, palier: i.palier, pourquoi: i.pourquoi }).into()
}
fn rupture(a: &(Motif,)) -> Rendu<Option<Fin>> {
    p::rupture(a.0.as_deref()).map(|o| o.map(|(fin, rubrique)| Fin { cle: fin.cle().to_string(), rubrique })).into()
}
fn code_52_permis(a: &(Motif, Motif)) -> Rendu<Option<String>> {
    p::code_52_permis(a.0.as_deref(), a.1.as_deref()).into()
}

#[test]
fn la_fin_du_contrat() {
    jouer("permanents/precarite_due.json", cas_de("permanents/precarite_due.json"), precarite_due);
    jouer("permanents/indemnite_precarite.json", cas_de("permanents/indemnite_precarite.json"), indemnite_precarite);
    jouer("permanents/anciennete_mois.json", cas_de("permanents/anciennete_mois.json"), anciennete_mois);
    jouer("permanents/preavis_mois.json", cas_de("permanents/preavis_mois.json"), preavis_mois);
    jouer("permanents/salaire_de_reference.json", cas_de("permanents/salaire_de_reference.json"), salaire_de_reference);
    jouer("permanents/indemnite_licenciement.json", cas_de("permanents/indemnite_licenciement.json"), indemnite_licenciement);
    jouer("permanents/indemnite_mise_a_la_retraite.json", cas_de("permanents/indemnite_mise_a_la_retraite.json"), indemnite_mise_a_la_retraite);
    jouer("permanents/indemnite_rupture_conventionnelle.json", cas_de("permanents/indemnite_rupture_conventionnelle.json"), indemnite_rupture_conventionnelle);
    jouer("permanents/indemnite_depart_retraite.json", cas_de("permanents/indemnite_depart_retraite.json"), indemnite_depart_retraite);
    jouer("permanents/rupture.json", cas_de("permanents/rupture.json"), rupture);
    jouer("permanents/code_52_permis.json", cas_de("permanents/code_52_permis.json"), code_52_permis);
}

fn droit_au_maintien(a: &(Option<Nature>, Decimal)) -> Rendu<Option<String>> {
    let n = a.0.as_deref().map(|n| nature(n).unwrap());
    p::droit_au_maintien(n.as_deref(), a.1).into()
}
fn jours_de_carence(a: &(bool, bool)) -> Rendu<i64> {
    p::jours_de_carence(a.0, a.1).into()
}
fn duree_du_maintien(a: &(Decimal,)) -> Rendu<Vec<Tranche>> {
    p::duree_du_maintien(a.0).map(|t| t.into_iter().map(|t| Tranche { jours: t.jours, taux: t.taux }).collect()).into()
}

#[test]
fn le_maintien() {
    jouer("permanents/droit_au_maintien.json", cas_de("permanents/droit_au_maintien.json"), droit_au_maintien);
    jouer("permanents/jours_de_carence.json", cas_de("permanents/jours_de_carence.json"), jours_de_carence);
    jouer("permanents/duree_du_maintien.json", cas_de("permanents/duree_du_maintien.json"), duree_du_maintien);
}

/// Ce que `permanents-base.json` dit d'une indemnite (son code du bloc S21.G00.52, ce qu'elle supporte), sa rubrique
/// (`rubriques-base.json`) le porte a l'identique : la ligne que le solde ecrit se declare et se cotise comme le texte le dit.
#[test]
fn chaque_indemnite_a_le_code_et_le_regime_de_sa_rubrique() {
    let rg = p::regles().unwrap();
    let r = |code: &str| droit::catalogue::rubrique(std::path::Path::new("/nulle-part"), code, &Jour::lire("2026-10-01").unwrap()).unwrap();
    let codes = [
        (rg.conges_payes.indemnite_compensatrice.rubrique.clone(), &rg.conges_payes.indemnite_compensatrice.dsn_bloc52),
        (rg.indemnite_precarite.rubrique.clone(), &rg.indemnite_precarite.dsn_bloc52),
        (rg.preavis.delai_de_prevenance.rubrique.clone(), &rg.preavis.delai_de_prevenance.dsn_bloc52),
        (rg.indemnite_licenciement.rubrique.clone(), &rg.indemnite_licenciement.dsn_bloc52),
        (rg.rupture_conventionnelle.rubrique.clone(), &rg.rupture_conventionnelle.dsn_bloc52),
        (rg.mise_a_la_retraite.rubrique.clone(), &rg.mise_a_la_retraite.dsn_bloc52),
        (format!("{}_conventionnelle", rg.mise_a_la_retraite.rubrique), &rg.mise_a_la_retraite.dsn_bloc52_conventionnelle),
        (rg.depart_a_la_retraite.rubrique.clone(), &rg.depart_a_la_retraite.dsn_bloc52),
        (format!("{}_conventionnelle", rg.depart_a_la_retraite.rubrique), &rg.depart_a_la_retraite.dsn_bloc52_conventionnelle),
    ];
    for (rubrique, code) in codes {
        assert_eq!(r(&rubrique).dsn_bloc52.as_deref(), Some(code.code.as_str()), "{}", rubrique);
    }
    for (cle, regime) in &rg.regime_des_indemnites_de_rupture.par_indemnite {
        let lue = r(&regime.rubrique);
        assert_eq!((lue.imposable, lue.soumis), (regime.imposable, regime.soumis), "{} ({})", cle, regime.rubrique);
    }
}
