//! Les natures de contrat (`contrats-base.json`) contre `natures.json` : chaque attribut et chaque code des natures,
//! l'inconnue refusee, `contrat_court` (`natures-court.json`). `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit les references.

use std::collections::BTreeMap;

use crate::outils::typee::{cas_de, reference, verifier, Ligne};
use droit::natures::{contrat_court, nature, natures, Jour, MotifDeRecours, MotifDeRupture, Paie, TypeDeRemuneration, UniteDAbsence, UniteDActivite};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Un nombre du droit (un `f64` du moteur) en decimal exact, par son ecriture la plus courte.
fn dec(x: f64) -> Decimal {
    x.to_string().parse().unwrap()
}

/// Les codes d'une rubrique pour une nature : la valeur fixe, le controle qui l'interdit, si la nature l'ecrit.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CodeDit {
    fixe: Option<String>,
    interdit: Option<String>,
    ecrit: bool,
}

/// Ce que le motif de rupture ecrit en fin de contrat : le signalement FCTU, les codes de fin.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FinDit {
    signalement_fctu: bool,
    codes_fin: BTreeMap<String, String>,
}

/// Le cas : la nature, et les rubriques dont on lit les codes.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CasNature {
    nature: String,
    rubriques: Vec<String>,
}

/// Tout ce qu'une nature dit.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fiche {
    libelle: String,
    paie: Paie,
    duree_indeterminee: bool,
    unite_quotite: String,
    plafond_quotite: Decimal,
    quotite_reference_defaut: Option<Decimal>,
    bareme: Option<String>,
    mensuelle: bool,
    permanent: bool,
    codes: BTreeMap<String, CodeDit>,
    motif_rupture_par_defaut: Option<String>,
    motifs_rupture: Vec<MotifDeRupture>,
    motifs_recours: Vec<MotifDeRecours>,
    motif_recours_exige: bool,
    fin_sans_motif: FinDit,
    fin_par_motif: BTreeMap<String, FinDit>,
    type_contrat_dpae: String,
    date_fin_dpae: bool,
    types_remuneration: Vec<TypeDeRemuneration>,
    remunerations_au_brut: bool,
    unites_activite: Vec<UniteDActivite>,
    unite_absence: Option<UniteDAbsence>,
}

fn fin(n: &droit::natures::Nature, motif: Option<&str>) -> FinDit {
    FinDit { signalement_fctu: n.signalement_fctu(motif), codes_fin: n.codes_fin(motif) }
}

fn fiche(c: &CasNature) -> Fiche {
    let n = nature(&c.nature).unwrap();
    assert_eq!(n.cle, c.nature);
    Fiche {
        libelle: n.libelle.clone(),
        paie: n.paie,
        duree_indeterminee: n.duree_indeterminee,
        unite_quotite: n.unite_quotite().to_string(),
        plafond_quotite: dec(n.plafond_quotite()),
        quotite_reference_defaut: n.quotite_reference_defaut().map(dec),
        bareme: n.bareme().map(String::from),
        mensuelle: n.mensuelle(),
        permanent: n.permanent(),
        codes: c.rubriques.iter().map(|r| (r.clone(), CodeDit { fixe: n.code(r), interdit: n.interdit(r), ecrit: n.ecrit(r) })).collect(),
        motif_rupture_par_defaut: n.motif_rupture_par_defaut(),
        motifs_rupture: n.motifs_rupture().to_vec(),
        motifs_recours: n.motifs_recours().unwrap_or_default().to_vec(),
        motif_recours_exige: n.motif_recours_exige(),
        fin_sans_motif: fin(&n, None),
        fin_par_motif: n.motifs_rupture().iter().map(|m| (m.code.clone(), fin(&n, Some(&m.code)))).collect(),
        type_contrat_dpae: n.type_contrat_dpae(),
        date_fin_dpae: n.date_fin_dpae(),
        types_remuneration: n.types_remuneration().to_vec(),
        remunerations_au_brut: n.remunerations_au_brut(),
        unites_activite: n.unites_activite().to_vec(),
        unite_absence: n.unite_absence().cloned(),
    }
}

/// Un contrat et le jour ou on le lit : est-il court ?
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CasCourt {
    nature: String,
    debut: Jour,
    fin: Option<Jour>,
    le: Jour,
}

#[test]
fn les_natures() {
    let cas: Vec<CasNature> = cas_de("natures.json");
    assert_eq!(natures().unwrap().len(), cas.len());
    verifier("natures.json", cas.iter().map(|c| Ligne { cas: CasNature { nature: c.nature.clone(), rubriques: c.rubriques.clone() }, attendu: fiche(c) }).collect());
    assert_eq!(nature("zzz").unwrap_err(), "Nature de contrat inconnue : 'zzz'. Connues : cdd, cddu_spectacle, cdi.");
}

#[test]
fn contrat_court_des_natures() {
    let lignes: Vec<Ligne<CasCourt, bool>> = reference("natures-court.json");
    assert!(lignes.len() >= 39);
    let obtenu = lignes
        .into_iter()
        .map(|l| {
            let n = nature(&l.cas.nature).unwrap();
            let court = contrat_court(&n, l.cas.debut, l.cas.fin, l.cas.le);
            Ligne { cas: l.cas, attendu: court }
        })
        .collect();
    verifier("natures-court.json", obtenu);
}
