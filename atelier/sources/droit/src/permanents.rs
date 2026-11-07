//! Ce qu'un contrat permanent a de plus : conges, absences, fin de contrat.
//!
//! Le module calcule, il ne decide pas : les nombres vivent dans `permanents-base.json`, avec l'article qui les porte,
//! lu une fois par sa forme (`regles`, la seule lecture : `Regles` et ses types).

use crate::natures::Nature;
use crate::source::Fondement;
use serde::{Deserialize, Serialize};
use socle::date::{jours_du_mois, Jour};
use socle::feries::date_ou_refus;
use socle::fichiers::{forme_en_cache, FormeGardee};
use socle::num::{add, sub, div, eur2, mul, zero2, Decimal, Resultat};
use rust_decimal::prelude::*;
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

/// Une tranche d'indemnisation du maintien : sa duree et son taux (en %).
#[derive(Debug, Clone, PartialEq)]
pub struct Tranche {
    pub jours: i64,
    pub taux: Decimal,
}

// ------------------------------------------------------------ la forme du fichier

/// Un objet du fichier et ses notes (`quoi`, `note`, `source`) : elles ne se lisent pas au calcul.
macro_rules! forme {
    ($(#[$m:meta])* pub struct $nom:ident { $($corps:tt)* }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $nom {
            $($corps)*
            pub quoi: Option<String>,
            pub note: Option<String>,
            pub source: Option<Fondement>,
        }
    };
}

/// Des codes de motif de rupture (DSN S21.G00.62.002) et ce que le texte en dit.
pub type ParMotif = BTreeMap<String, String>;

forme! {
    /// `permanents-base.json`, entier.
    pub struct Regles {
        pub conges_payes: CongesPayes,
        pub indemnite_precarite: Precarite,
        pub preavis: Preavis,
        pub indemnite_licenciement: Licenciement,
        pub indemnites_de_rupture: IndemnitesDeRupture,
        pub regime_des_indemnites_de_rupture: Regime,
        pub rupture_conventionnelle: RuptureConventionnelle,
        pub mise_a_la_retraite: MiseALaRetraite,
        pub depart_a_la_retraite: DepartALaRetraite,
        pub heures_en_plus: HeuresEnPlus,
        pub maintien_de_salaire: Maintien,
        pub indemnites_journalieres: IndemnitesJournalieres,
        pub suspensions: Suspensions,
    }
}

forme! {
    /// Le code d'indemnite du bloc S21.G00.52 qu'une section declare.
    pub struct CodeDsn {
        pub code: String,
    }
}

forme! {
    pub struct CongesPayes {
        pub acquisition: AcquisitionDesConges,
        pub periode_de_reference: PeriodeDeReference,
        pub indemnisation: Indemnisation,
        pub indemnite_compensatrice: IndemniteCompensatriceDeConges,
    }
}

forme! {
    /// Ce qui n'a pas de valeur a soi : le texte qui fonde ce que le moteur fait (`source`), et ce qu'il en dit.
    pub struct Fondee {}
}

forme! {
    pub struct AcquisitionDesConges {
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub jours_par_mois: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub maximum_annuel: Decimal,
        pub arrondi: ArrondiDuConge,
    }
}

/// Vers ou s'arrondit une duree de conge qui n'est pas un nombre entier de jours (L3141-7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersArrondi {
    EntierSuperieur,
}

forme! {
    /// `conges_payes.acquisition.arrondi` : L3141-7.
    pub struct ArrondiDuConge {
        pub vers: VersArrondi,
    }
}

forme! {
    pub struct PeriodeDeReference {
        pub debut_mois: i64,
        pub debut_jour: i64,
    }
}

forme! {
    pub struct Indemnisation {
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub dixieme: Decimal,
    }
}

forme! {
    pub struct IndemniteCompensatriceDeConges {
        /// La rubrique de la ligne que le solde ecrit (`rubriques-base.json`).
        pub rubrique: String,
        pub dsn_bloc52: CodeDsn,
    }
}

forme! {
    /// `indemnite_precarite` : L1243-8 a L1243-10.
    pub struct Precarite {
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub taux: Decimal,
        pub rubrique: String,
        pub dsn_bloc52: CodeDsn,
        /// Les natures de contrat (`contrats-base.json`) qui n'y ont pas droit, et pourquoi.
        pub natures_exclues: BTreeMap<String, String>,
        pub motifs_rupture_exclus: ParMotif,
    }
}

forme! {
    pub struct Preavis {
        pub tranches: Vec<TrancheDePreavis>,
        pub motifs_sans_preavis: ParMotif,
        pub rupture_anticipee_cdd: RuptureAnticipeeCdd,
        pub delai_de_prevenance: RegleDuDelaiDePrevenance,
        pub indemnite_compensatrice: IndemniteCompensatriceDePreavis,
    }
}

forme! {
    /// Une tranche d'anciennete (en mois, `max` exclu ; `None` : sans borne) et son preavis (`None` : la loi n'en donne pas).
    pub struct TrancheDePreavis {
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub anciennete_mois_min: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal_opt")]
        pub anciennete_mois_max: Option<Decimal>,
        #[serde(deserialize_with = "socle::forme::decimal_opt")]
        pub mois: Option<Decimal>,
    }
}

forme! {
    pub struct RuptureAnticipeeCdd {
        pub motifs_sans_preavis: ParMotif,
        pub motifs_a_dire: ParMotif,
    }
}

forme! {
    /// `preavis.delai_de_prevenance` : L1221-25.
    pub struct RegleDuDelaiDePrevenance {
        pub motif_rupture: String,
        pub rubrique: String,
        pub dsn_bloc52: CodeDsn,
        pub tranches: Vec<TrancheDeDelai>,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub cdd_essai_minimum_jours: Decimal,
    }
}

/// Une tranche de presence et son delai : la presence en jours (strictement moins) ou en mois (au plus), aucune pour la
/// derniere ; le delai en jours ou en mois.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrancheDeDelai {
    pub presence_moins_de_jours: Option<i64>,
    pub presence_jusqu_a_mois: Option<i64>,
    pub delai_jours: Option<i64>,
    pub delai_mois: Option<u32>,
    pub texte: String,
}

forme! {
    pub struct IndemniteCompensatriceDePreavis {
        pub rubrique: String,
        pub rubrique_par_motif: ParMotif,
        pub motifs_sans_indemnite: ParMotif,
        pub motifs_par_un_autre_article: ParMotif,
        pub motifs_a_trancher: ParMotif,
    }
}

forme! {
    pub struct Licenciement {
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub anciennete_minimale_mois: Decimal,
        pub taux: Vec<TauxDeLicenciement>,
        pub salaire_de_reference: Fondee,
        pub motifs_sans_indemnite: ParMotif,
        pub rubrique: String,
        pub dsn_bloc52: CodeDsn,
    }
}

forme! {
    /// Une fraction de mois par annee d'anciennete, jusqu'a `jusqu_a_annees` (`None` : au-dela).
    pub struct TauxDeLicenciement {
        #[serde(deserialize_with = "socle::forme::decimal_opt")]
        pub jusqu_a_annees: Option<Decimal>,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub fraction_de_mois: Decimal,
    }
}

forme! {
    pub struct IndemnitesDeRupture {
        pub codes: BTreeMap<String, CodeDIndemnite>,
    }
}

forme! {
    /// Un code du bloc 52 que les controles CCH-21 a CCH-29 lient a des motifs de rupture.
    pub struct CodeDIndemnite {
        pub libelle: String,
        pub controle: String,
        pub motifs: Vec<String>,
    }
}

forme! {
    pub struct Regime {
        pub par_indemnite: BTreeMap<String, RegimeDUneIndemnite>,
        pub plafonds: Plafonds,
        pub contribution_patronale: ContributionPatronale,
    }
}

forme! {
    /// Ce qu'une indemnite supporte, que sa rubrique (`rubriques-base.json`) porte a l'identique.
    pub struct RegimeDUneIndemnite {
        pub rubrique: String,
        pub imposable: bool,
        pub soumis: bool,
    }
}

forme! {
    /// Les deux plafonds de L242-1 II 7°, en plafonds annuels de la Securite sociale.
    pub struct Plafonds {
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub cotisations_exoneration_pass: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub cotisations_assujettissement_total_pass: Decimal,
    }
}

forme! {
    pub struct ContributionPatronale {
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub taux: Decimal,
        pub motifs: Vec<String>,
    }
}

forme! {
    pub struct RuptureConventionnelle {
        pub motif_rupture: String,
        pub dsn_bloc52: CodeDsn,
        pub rubrique: String,
        pub indemnite: PlancherDeRupture,
        pub preavis: Fondee,
        pub calendrier: Fondee,
    }
}

forme! {
    /// Le plancher de l'indemnite de rupture conventionnelle : celle de L1234-9 ; sous son anciennete minimale, au prorata
    /// des mois de presence quand `prorata_sous_l_anciennete_minimale` (la circulaire DGT 2009-4, 5.3).
    pub struct PlancherDeRupture {
        pub prorata_sous_l_anciennete_minimale: bool,
    }
}

forme! {
    pub struct MiseALaRetraite {
        pub motif_rupture: String,
        pub dsn_bloc52: CodeDsn,
        pub dsn_bloc52_conventionnelle: CodeDsn,
        pub rubrique: String,
        pub indemnite: Fondee,
        pub preavis: Fondee,
        pub age: Fondee,
    }
}

forme! {
    pub struct DepartALaRetraite {
        pub motif_rupture: String,
        pub dsn_bloc52: CodeDsn,
        pub dsn_bloc52_conventionnelle: CodeDsn,
        pub rubrique: String,
        pub indemnite: IndemniteDeDepart,
        pub preavis: Fondee,
    }
}

forme! {
    pub struct IndemniteDeDepart {
        pub taux: Vec<PalierDeDepart>,
        pub salaire_de_reference: Fondee,
    }
}

forme! {
    /// D1237-1 : un forfait de mois de salaire a partir d'une anciennete.
    pub struct PalierDeDepart {
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub anciennete_annees: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub mois_de_salaire: Decimal,
    }
}

forme! {
    /// `heures_en_plus` : lu par `heures_en_plus`.
    pub struct HeuresEnPlus {
        pub heures_supplementaires: HeuresSupplementaires,
        pub heures_complementaires: HeuresComplementaires,
        pub reduction_salariale: ReductionSalariale,
        pub deduction_patronale: DeductionPatronale,
        pub exoneration_impot: ExonerationImpot,
    }
}

forme! {
    pub struct HeuresSupplementaires {
        pub paliers_hebdomadaires: Vec<PalierHebdomadaire>,
    }
}

/// Des heures de la semaine (`None` : les suivantes) a une majoration (en pourcent).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PalierHebdomadaire {
    #[serde(deserialize_with = "socle::forme::decimal_opt")]
    pub heures: Option<Decimal>,
    #[serde(deserialize_with = "socle::forme::decimal")]
    pub majoration: Decimal,
}

/// Une part des heures prevues au contrat (L3123-28, L3123-29).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartDuContrat {
    Dixieme,
    Tiers,
}

impl PartDuContrat {
    /// Le nom de la part, tel qu'une convention l'ecrit (`dixieme`, `tiers`).
    pub fn nom(self) -> &'static str {
        match self {
            PartDuContrat::Dixieme => "dixieme",
            PartDuContrat::Tiers => "tiers",
        }
    }
}

forme! {
    pub struct HeuresComplementaires {
        pub plafond_sans_accord: PartDuContrat,
        pub paliers: Vec<PalierComplementaire>,
    }
}

/// Les heures complementaires jusqu'a une part du contrat, a une majoration (en pourcent).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PalierComplementaire {
    pub jusqu_au: PartDuContrat,
    #[serde(deserialize_with = "socle::forme::decimal")]
    pub majoration: Decimal,
}

forme! {
    pub struct ReductionSalariale {
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub taux_maximum: Decimal,
    }
}

forme! {
    pub struct DeductionPatronale {
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub moins_de_20_salaries: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub au_moins_20_salaries: Decimal,
    }
}

forme! {
    pub struct ExonerationImpot {
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub limite_annuelle: Decimal,
    }
}

forme! {
    /// `maintien_de_salaire` : L1226-1, D1226-1 a D1226-8.
    pub struct Maintien {
        pub anciennete_mois_minimum: i64,
        pub carence_jours: i64,
        pub tranches: Vec<TrancheDeMaintien>,
        pub majoration: Majoration,
    }
}

forme! {
    pub struct TrancheDeMaintien {
        pub jours: i64,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub taux: Decimal,
    }
}

forme! {
    /// D1226-2 : `jours` de plus par `par_annees` d'anciennete au-dela de la premiere, chaque tranche au plus `plafond_jours`.
    pub struct Majoration {
        pub par_annees: i64,
        pub jours: i64,
        pub plafond_jours: i64,
    }
}

forme! {
    /// `indemnites_journalieres` : lu par `engagement::maintien`.
    pub struct IndemnitesJournalieres {
        /// Le rang du premier jour indemnise d'une maladie (R323-1).
        pub debut_maladie: i64,
        /// Le rang du jour ou l'indemnite d'un accident se majore (R433-3).
        pub majoree_des_le_jour: i64,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub csg_deductible: Decimal,
        pub pas: PasDesIndemnites,
        pub principe: Principe,
        pub principe_accident: PrincipeAccident,
    }
}

forme! {
    /// Ce que le prelevement a la source retient des indemnites subrogees (BOI-IR-PAS-20-10-10).
    pub struct PasDesIndemnites {
        /// Une maladie : celles des `maladie_jours` premiers jours de l'arret.
        pub maladie_jours: i64,
        /// Un accident du travail ou une maladie professionnelle : ce pourcentage de leur montant.
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub accident: Decimal,
    }
}

forme! {
    /// Le montant de principe d'une maladie (R323-4, R323-5).
    pub struct Principe {
        pub mois: usize,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub diviseur: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub fraction: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub plafond_smic: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub heures_du_mois: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub csg: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub crds: Decimal,
    }
}

forme! {
    /// Le montant de principe d'un accident du travail (R433-1 a R433-4).
    pub struct PrincipeAccident {
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub diviseur: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub fraction: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub fraction_majoree: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub plafond_pass: Decimal,
        pub plafond_depuis: Jour,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub taux_forfaitaire: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub csg: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub crds: Decimal,
    }
}

forme! {
    pub struct Suspensions {
        pub motifs: Vec<MotifDeSuspension>,
    }
}

/// Comment une suspension compte dans l'anciennete (L1225-54 : pour moitie).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AncienneteDeSuspension {
    Moitie,
}

forme! {
    /// Un motif du bloc S21.G00.65 (S21.G00.65.001).
    pub struct MotifDeSuspension {
        pub code: String,
        pub libelle: String,
        pub remunere: bool,
        pub acquiert_conges: bool,
        pub anciennete: Option<AncienneteDeSuspension>,
        /// Pourquoi ce motif ne se declare pas (le 200, les conges payes), quand il ne se declare pas.
        pub ne_se_declare_pas: Option<String>,
    }
}

impl MotifDeSuspension {
    /// Le texte qui fonde ce que le motif dit (`source`) : « article : « citation » », vide sans source.
    pub fn texte(&self) -> String {
        self.source
            .iter()
            .flat_map(|f| f.sources())
            .map(|s| {
                let ou = match &s.article {
                    Some(article) => article.as_str(),
                    None => s.fichier.texte(),
                };
                format!("{} : « {} »", ou, s.citation)
            })
            .collect::<Vec<_>>()
            .join(" ; ")
    }
}

const ILLISIBLE: &str = "Les règles des contrats permanents livrées avec le logiciel ne se lisent pas : le signaler au développeur.";

/// LA SEULE LECTURE DE `permanents-base.json` : sa forme.
pub fn regles() -> Resultat<Arc<Regles>> {
    static CACHE: OnceLock<FormeGardee<Regles>> = OnceLock::new();
    forme_en_cache(&CACHE, "permanents-base.json", ILLISIBLE, Ok)
}

/// Le motif de suspension `code` (`suspensions.motifs`), s'il existe.
pub fn motif_de_suspension(code: &str) -> Resultat<Option<MotifDeSuspension>> {
    Ok(regles()?.suspensions.motifs.iter().find(|m| m.code == code).cloned())
}

// ------------------------------------------------------------ les aides
fn douze() -> Decimal {
    Decimal::from(12)
}

/// Le refus d'un motif de rupture que la fiche ne dit pas, quand la regle en a besoin.
fn motif_requis<'a>(motif: Option<&'a str>, pour: &str) -> Resultat<&'a str> {
    motif.filter(|m| !m.is_empty()).ok_or_else(|| format!("{} : le motif de la fin du contrat n'est pas dit (case « Motif de la fin »).", pour))
}

/// Ce que `liste` dit de ce motif, s'il y est et le dit.
fn dit(liste: &ParMotif, motif: Option<&str>) -> Option<String> {
    motif.and_then(|m| liste.get(m)).filter(|t| !t.is_empty()).cloned()
}

// --------------------------------------------------------------- les conges
/// Les bornes de la periode de reference qui contient `jour` : du 1er juin au 31 mai (`conges_payes.periode_de_reference`).
pub fn periode_de_reference(jour: &Jour) -> Resultat<(Jour, Jour)> {
    let rg = regles()?;
    let r = &rg.conges_payes.periode_de_reference;
    let (mois, dj) = (r.debut_mois, r.debut_jour);
    let an = if (jour.mois as i64, jour.jour as i64) >= (mois, dj) { jour.an } else { jour.an - 1 } as i64;
    let debut = date_ou_refus(an, mois, dj)?;
    let fin = date_ou_refus(an + 1, mois, dj)?.plus_jours(-1)?;
    Ok((debut, fin))
}

/// Les jours ouvrables acquis sur une periode de presence : 2,5 par mois,
/// au prorata des jours calendaires de chaque mois traverse.
pub fn jours_acquis(debut: &Jour, fin: &Jour) -> Resultat<Decimal> {
    let par_mois = regles()?.conges_payes.acquisition.jours_par_mois;
    if fin < debut {
        return Ok(Decimal::ZERO);
    }
    let (mut total, mut jour) = (Decimal::ZERO, *debut);
    while jour <= *fin {
        let dans_le_mois = jours_du_mois(jour.an, jour.mois);
        let derniere = Jour { an: jour.an, mois: jour.mois, jour: dans_le_mois };
        let borne = if *fin < derniere { *fin } else { derniere };
        let jours = Decimal::from(borne.moins(&jour) + 1);
        total = add(total, div(mul(par_mois, jours), Decimal::from(dans_le_mois)));
        jour = borne.plus_jours(1)?;
    }
    Ok(total)
}

/// Trente jours ouvrables au plus.
pub fn plafonner(acquis: Decimal) -> Resultat<Decimal> {
    Ok(acquis.min(regles()?.conges_payes.acquisition.maximum_annuel))
}

/// LA DUREE DU CONGE qu'une periode de reference a acquise, quand elle se clot (L3141-7 : un nombre de jours qui n'est
/// pas entier « est porté au nombre entier immédiatement supérieur ») : `acquis`, deja plafonne, arrondi
/// (`conges_payes.acquisition.arrondi`).
pub fn duree_du_conge(acquis: Decimal) -> Resultat<Decimal> {
    Ok(match regles()?.conges_payes.acquisition.arrondi.vers {
        // Un nombre deja entier garde son ecriture (`30.0`).
        VersArrondi::EntierSuperieur if acquis.fract().is_zero() => acquis,
        VersArrondi::EntierSuperieur => acquis.ceil(),
    })
}

/// Le compte affiche : au demi-jour superieur (un affichage, pas une regle).
pub fn arrondir_jours(jours: Decimal) -> Decimal {
    let demi = (jours * Decimal::from(2)).ceil();
    div(demi, Decimal::from(2)).normalize()
}

/// L'indemnite pour des jours de conges pris (L3141-24) : le dixieme de la remuneration de la periode, au prorata des
/// jours qu'elle a acquis, le maintien, et celui qui gagne.
pub fn indemnite_conges(
    jours_pris: Decimal,
    remuneration_periode: Decimal,
    jours_acquis_periode: Decimal,
    maintien: Decimal,
) -> Resultat<crate::conges::IndemniteDesJours> {
    let taux = regles()?.conges_payes.indemnisation.dixieme;
    let dixieme = if jours_acquis_periode > Decimal::ZERO {
        eur2(div(mul(mul(remuneration_periode, taux), jours_pris), jours_acquis_periode))
    } else {
        zero2()
    };
    Ok(crate::conges::la_plus_forte(dixieme, maintien))
}


/// Des jours de conges acquis sur une periode de reference, et la remuneration de cette periode.
#[derive(Debug, Clone, Copy)]
pub struct Acquisition {
    pub jours: Decimal,
    pub remuneration: Decimal,
}

/// L'indemnite de `jours` de conges (L3141-24), chaque jour au dixieme DE LA PERIODE OU IL A ETE ACQUIS : les
/// `report_restant` jours du report d'abord (au dixieme de `report`), puis la periode `en_cours`. Le maintien se
/// compare au TOTAL.
pub fn indemnite_des_jours(
    jours: Decimal,
    report_restant: Decimal,
    report: Acquisition,
    en_cours: Acquisition,
    maintien: Decimal,
) -> Resultat<crate::conges::IndemniteDesJours> {
    let sur_report = jours.min(report_restant);
    let reste = sub(jours, sur_report);
    let mut dixieme = zero2();
    let dixieme_de = |jours: Decimal, a: Acquisition| -> Resultat<Decimal> {
        Ok(indemnite_conges(jours, a.remuneration, if a.jours != Decimal::ZERO { a.jours } else { jours }, Decimal::ZERO)?.dixieme)
    };
    if sur_report != Decimal::ZERO {
        dixieme += dixieme_de(sur_report, report)?;
    }
    if reste != Decimal::ZERO {
        dixieme += dixieme_de(reste, en_cours)?;
    }
    Ok(crate::conges::la_plus_forte(dixieme, maintien))
}
// ------------------------------------------------------- la fin du contrat
/// `None` si l'indemnite de fin de CDD est due, sinon pourquoi elle ne l'est pas.
pub fn precarite_due(nature: &Nature, motif_rupture: Option<&str>) -> Resultat<Option<String>> {
    if nature.duree_indeterminee {
        return Ok(Some(
            "un contrat a duree indeterminee n'a pas d'indemnite de fin \
             de contrat : elle compense la precarite d'un terme."
                .to_string(),
        ));
    }
    let rg = regles()?;
    let r = &rg.indemnite_precarite;
    if let Some(raison) = r.natures_exclues.get(&nature.cle) {
        return Ok(Some(raison.clone()));
    }
    Ok(motif_rupture.and_then(|m| r.motifs_rupture_exclus.get(m)).cloned())
}

/// 10 % de la remuneration totale brute versee pendant le CDD (L1243-8 ; la fiche entreprise ne porte aucun accord de
/// L1243-9).
pub fn indemnite_precarite(remuneration_totale: Decimal) -> Resultat<Decimal> {
    Ok(eur2(mul(remuneration_totale, regles()?.indemnite_precarite.taux)))
}

/// L'anciennete en mois : les mois complets et la fraction du mois entame.
pub fn anciennete_mois(debut: &Jour, fin: &Jour) -> Decimal {
    if fin < debut {
        return Decimal::ZERO;
    }
    let mut mois = (fin.an as i64 - debut.an as i64) * 12 + (fin.mois as i64 - debut.mois as i64);
    let anniv = debut.jour as i64;
    let reste = if fin.jour as i64 + 1 >= anniv {
        fin.jour as i64 + 1 - anniv
    } else {
        mois -= 1;
        let (an, precedent) = if fin.mois > 1 { (fin.an, fin.mois - 1) } else { (fin.an - 1, 12) };
        jours_du_mois(an, precedent) as i64 + fin.jour as i64 + 1 - anniv
    };
    let dans_le_mois = Decimal::from(jours_du_mois(fin.an, fin.mois));
    add(Decimal::from(mois), div(Decimal::from(reste), dans_le_mois))
}

/// Ce que `preavis.rupture_anticipee_cdd` dit du motif d'un contrat a duree determinee : `Some(true)` aucun preavis,
/// `Some(false)` un preavis que le moteur ne compte pas (L1243-2), avec le texte ; `None` hors de la section.
fn regle_du_cdd(cdd: bool, motif_rupture: Option<&str>) -> Resultat<Option<(bool, String)>> {
    if !cdd {
        return Ok(None);
    }
    let rg = regles()?;
    let sec = &rg.preavis.rupture_anticipee_cdd;
    let Some(m) = motif_rupture else { return Ok(None) };
    for (liste, sans) in [(&sec.motifs_sans_preavis, true), (&sec.motifs_a_dire, false)] {
        if let Some(t) = liste.get(m) {
            return Ok(Some((sans, t.clone())));
        }
    }
    Ok(None)
}

/// La duree du preavis en mois, ou `None` quand la loi ne la donne pas (ou que le moteur ne sait pas la compter :
/// `regle_du_preavis`). `cdd` : le contrat est a duree determinee, et ses motifs de rupture anticipee ont leur regle.
pub fn preavis_mois(anciennete: Decimal, motif_rupture: Option<&str>, cdd: bool) -> Resultat<Option<Decimal>> {
    if let Some((sans, _)) = regle_du_cdd(cdd, motif_rupture)? {
        return Ok(if sans { Some(Decimal::ZERO) } else { None });
    }
    let rg = regles()?;
    let r = &rg.preavis;
    if dit(&r.motifs_sans_preavis, motif_rupture).is_some() {
        return Ok(Some(Decimal::ZERO));
    }
    for t in &r.tranches {
        if anciennete >= t.anciennete_mois_min && t.anciennete_mois_max.is_none_or(|h| anciennete < h) {
            return Ok(t.mois);
        }
    }
    Ok(None)
}

/// Le preavis que la loi donne pour ce motif d'un CDD sans que le moteur sache le compter (`motifs_a_dire` : L1243-2).
pub fn regle_du_preavis(motif_rupture: Option<&str>, cdd: bool) -> Resultat<Option<String>> {
    Ok(regle_du_cdd(cdd, motif_rupture)?.filter(|(sans, _)| !sans).map(|(_, t)| t))
}

/// Pourquoi la loi ne donne aucun preavis pour ce motif (`preavis.motifs_sans_preavis`, et pour un CDD
/// `preavis.rupture_anticipee_cdd.motifs_sans_preavis`), quand il est de cette liste.
pub fn raison_sans_preavis(motif_rupture: Option<&str>, cdd: bool) -> Resultat<Option<String>> {
    if let Some((sans, t)) = regle_du_cdd(cdd, motif_rupture)? {
        return Ok(if sans { Some(t) } else { None });
    }
    Ok(dit(&regles()?.preavis.motifs_sans_preavis, motif_rupture))
}

/// Ce motif est-il de `preavis.motifs_sans_preavis` (sans la regle d'un CDD) ?
pub fn sans_preavis(motif_rupture: Option<&str>) -> Resultat<bool> {
    let rg = regles()?;
    Ok(motif_rupture.is_some_and(|m| rg.preavis.motifs_sans_preavis.contains_key(m)))
}

/// Ce que le texte fait du preavis non effectue, pour ce motif (`preavis.indemnite_compensatrice`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndemniteDePreavis {
    /// L1234-5, l'indemnite du salarie licencie : le motif n'est dans aucune des listes.
    Licenciement,
    /// Aucune indemnite (`motifs_sans_indemnite`), avec le texte qui le dit.
    Aucune(String),
    /// Une indemnite d'un montant egal, par un autre article (`motifs_par_un_autre_article`).
    AutreArticle(String),
    /// Aucun texte depose ne tranche (`motifs_a_trancher`) : le moteur paie l'indemnite (L1234-5), et le dit.
    ATrancher(String),
}

/// L'indemnite compensatrice d'un preavis non effectue, selon le motif de rupture. L1234-5 est au chapitre du
/// licenciement d'un CDI ; les autres fins de contrat ont leurs articles (`code-du-travail/articles-20261013.md`,
/// « Le preavis non effectue : ce que chaque fin de contrat en fait »). Un motif sans preavis ne passe pas par ici.
pub fn indemnite_de_preavis(motif_rupture: Option<&str>) -> Resultat<IndemniteDePreavis> {
    let rg = regles()?;
    let ic = &rg.preavis.indemnite_compensatrice;
    let m = motif_requis(motif_rupture, "Le préavis non effectué")?;
    Ok(if let Some(t) = ic.motifs_sans_indemnite.get(m) {
        IndemniteDePreavis::Aucune(t.clone())
    } else if let Some(t) = ic.motifs_par_un_autre_article.get(m) {
        IndemniteDePreavis::AutreArticle(t.clone())
    } else if let Some(t) = ic.motifs_a_trancher.get(m) {
        IndemniteDePreavis::ATrancher(t.clone())
    } else {
        IndemniteDePreavis::Licenciement
    })
}

/// La rubrique de la ligne qui paie le preavis non effectue, selon le motif : `preavis.indemnite_compensatrice.rubrique`
/// (`indemnite_preavis`, DSN 52 « 023 »), sauf le motif qui a la sienne (`rubrique_par_motif` : le 092, L1226-14,
/// `indemnite_preavis_inaptitude`, DSN 52 « 032 », cahier technique 2026.1 p. 253, `norme-dsn/texte/bloc-S21.G00.52.txt`).
/// Le code du bloc 52 est celui de la rubrique : il suit le motif comme l'indemnite.
pub fn rubrique_de_l_indemnite_de_preavis(motif_rupture: Option<&str>) -> Resultat<String> {
    let rg = regles()?;
    let ic = &rg.preavis.indemnite_compensatrice;
    let m = motif_requis(motif_rupture, "L'indemnité de préavis")?;
    Ok(match ic.rubrique_par_motif.get(m) {
        Some(propre) => propre.clone(),
        None => ic.rubrique.clone(),
    })
}

// ------------------------------------------------ le delai de prevenance d'une fin d'essai (L1221-25)

/// La section `preavis.delai_de_prevenance` si ce motif est le sien (034 : la fin d'essai a l'initiative de l'employeur),
/// sinon `None` : le 035 (L1221-26) n'ouvre rien au salarie.
pub fn regle_du_delai_de_prevenance(motif_rupture: Option<&str>) -> Resultat<Option<RegleDuDelaiDePrevenance>> {
    let rg = regles()?;
    let s = &rg.preavis.delai_de_prevenance;
    Ok((motif_rupture == Some(s.motif_rupture.as_str())).then(|| s.clone()))
}

/// Le delai de prevenance que L1221-25 donne a cette presence, et le jour ou il expire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelaiDePrevenance {
    /// Ce que la tranche dit (« quarante-huit heures (L1221-25, 2° : ...) »).
    pub texte: String,
    /// Les jours de presence, du premier jour du contrat a la notification, l'un et l'autre compris.
    pub presence_jours: i64,
    /// Le dernier jour du delai : le salarie l'a recu si le contrat finit ce jour-la ou apres.
    pub expiration: Jour,
}

/// Le meme quantieme `n` mois plus tard, ou le dernier jour du mois s'il ne l'a pas.
fn plus_mois(j: &Jour, n: u32) -> Resultat<Jour> {
    let rang = j.mois - 1 + n;
    let (an, mois) = (j.an + (rang / 12) as i32, rang % 12 + 1);
    Jour::nouveau(an, mois, j.jour.min(jours_du_mois(an, mois))).ok_or_else(|| socle::date::date_impossible(an, mois, j.jour))
}

/// L1221-25 : le delai de prevenance d'une fin d'essai notifiee le `notification` dans un contrat commence le `debut`
/// (`preavis.delai_de_prevenance.tranches`). Le decompte est une lecture, ecrite dans la `note` de la section : la
/// presence va du debut a la notification compris ; le delai court du lendemain de la notification, en jours entiers, ou
/// jusqu'au meme quantieme du mois suivant.
pub fn delai_de_prevenance(debut: &Jour, notification: &Jour) -> Resultat<DelaiDePrevenance> {
    let rg = regles()?;
    let presence_jours = notification.moins(debut) + 1;
    let presence_mois = anciennete_mois(debut, notification);
    let tranche = rg
        .preavis
        .delai_de_prevenance
        .tranches
        .iter()
        .find(|t| match (t.presence_moins_de_jours, t.presence_jusqu_a_mois) {
            (Some(j), _) => presence_jours < j,
            (None, Some(m)) => presence_mois <= Decimal::from(m),
            (None, None) => true,
        })
        .ok_or_else(|| socle::echec::livree("permanents-base.json", "preavis.delai_de_prevenance"))?;
    let expiration = match (tranche.delai_jours, tranche.delai_mois) {
        (Some(j), _) => notification.plus_jours(j)?,
        (None, Some(m)) => plus_mois(notification, m)?,
        (None, None) => return Err(socle::fichiers::absent("delai_jours")),
    };
    Ok(DelaiDePrevenance { texte: tranche.texte.clone(), presence_jours, expiration })
}

/// Le salaire de reference de l'indemnite de licenciement (R1234-4) : la moyenne des douze derniers mois, le tiers des
/// trois derniers, et le plus favorable des deux (`retenu`, par la regle `regle`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SalaireDeReference {
    pub moyenne_12_mois: Decimal,
    pub tiers_3_mois: Decimal,
    pub retenu: Decimal,
    /// `moyenne_12_mois` ou `tiers_3_mois`.
    pub regle: &'static str,
}

/// Une indemnite legale (licenciement, mise a la retraite) : son montant, si elle est due, et pourquoi elle ne l'est
/// pas ; les annees d'anciennete qui la comptent, quand elle l'est.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IndemniteLegale {
    pub montant: Decimal,
    pub du: bool,
    pub pourquoi: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annees: Option<Decimal>,
}

impl IndemniteLegale {
    fn non_due(pourquoi: String) -> IndemniteLegale {
        IndemniteLegale { montant: zero2(), du: false, pourquoi: Some(pourquoi), annees: None }
    }
}

/// Le plancher legal de l'indemnite de rupture conventionnelle (`None` : la loi n'en donne pas), les annees qui le
/// comptent, et la phrase qui l'explique.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlancherDeLaRupture {
    pub plancher: Option<Decimal>,
    pub annees: Decimal,
    pub pourquoi: Option<String>,
}

/// L'indemnite legale de depart a la retraite : son montant, les mois de salaire et le palier (en annees) qui la
/// donnent.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IndemniteDuDepart {
    pub montant: Decimal,
    pub du: bool,
    pub mois: Decimal,
    pub palier: Option<i64>,
    pub pourquoi: Option<String>,
}

/// Le salaire de reference de l'indemnite de licenciement : la moyenne des
/// douze derniers mois ou le tiers des trois derniers, le plus favorable.
pub fn salaire_de_reference(douze_derniers: &[Decimal], trois_derniers: &[Decimal]) -> SalaireDeReference {
    let moyenne = if douze_derniers.is_empty() {
        zero2()
    } else {
        let s = douze_derniers.iter().fold(Decimal::ZERO, |a, b| add(a, *b));
        eur2(div(s, Decimal::from(douze_derniers.len() as i64)))
    };
    let tiers = if trois_derniers.is_empty() {
        zero2()
    } else {
        let s = trois_derniers.iter().fold(Decimal::ZERO, |a, b| add(a, *b));
        eur2(div(s, Decimal::from(3)))
    };
    SalaireDeReference {
        moyenne_12_mois: moyenne,
        tiers_3_mois: tiers,
        retenu: moyenne.max(tiers),
        regle: if moyenne >= tiers { "moyenne_12_mois" } else { "tiers_3_mois" },
    }
}

/// L'indemnite legale de licenciement.
pub fn indemnite_licenciement(
    anciennete: Decimal,
    reference: Decimal,
    motif_rupture: Option<&str>,
) -> Resultat<IndemniteLegale> {
    let rg = regles()?;
    let r = &rg.indemnite_licenciement;
    if let Some(e) = dit(&r.motifs_sans_indemnite, motif_rupture) {
        return Ok(IndemniteLegale::non_due(e));
    }
    let mini = r.anciennete_minimale_mois;
    if anciennete < mini {
        return Ok(IndemniteLegale::non_due(format!(
            "{} mois d'anciennete, et l'indemnite legale de licenciement en demande {} (L1234-9).",
            mois_complets(anciennete), mini)));
    }
    let (montant, annees) = bareme_licenciement(anciennete, reference)?;
    Ok(IndemniteLegale { montant, du: true, pourquoi: None, annees: Some(annees) })
}

/// Le montant de R1234-2, sans condition ni exclusion : un quart de mois
/// par annee jusqu'a dix ans, un tiers au-dela. Sert au licenciement, a la
/// mise a la retraite et a la rupture conventionnelle.
/// R1234-1 : « En cas d'année incomplète, l'indemnité est calculée proportionnellement au nombre de mois complets » ; le
/// mois entame ne compte pas.
fn bareme_licenciement(anciennete: Decimal, reference: Decimal) -> Resultat<(Decimal, Decimal)> {
    let rg = regles()?;
    let annees = div(mois_complets(anciennete), douze());
    let (mut total, mut bas) = (Decimal::ZERO, Decimal::ZERO);
    for t in &rg.indemnite_licenciement.taux {
        // La derniere tranche n'a pas de borne : elle va jusqu'a l'anciennete.
        let haut = match t.jusqu_a_annees {
            Some(h) => h,
            None => annees,
        };
        let part = add(annees.min(haut), -bas).max(Decimal::ZERO);
        total = add(total, mul(part, t.fraction_de_mois));
        bas = haut;
        if annees <= haut {
            break;
        }
    }
    Ok((eur2(mul(total, reference)), annees))
}

/// Les mois complets d'une anciennete en mois.
fn mois_complets(anciennete: Decimal) -> Decimal {
    anciennete.floor()
}

/// L'anciennete minimale de l'indemnite de licenciement (L1234-9), en mois.
fn anciennete_minimale() -> Resultat<Decimal> {
    Ok(regles()?.indemnite_licenciement.anciennete_minimale_mois)
}

// ------------------------------------- les trois autres fins d'un CDI
/// Les trois autres fins d'un CDI, chacune sa section de `permanents-base.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinDeCdi {
    RuptureConventionnelle,
    MiseALaRetraite,
    DepartALaRetraite,
}

impl FinDeCdi {
    /// La cle de sa section (`rupture_conventionnelle`, `mise_a_la_retraite`, `depart_a_la_retraite`).
    pub fn cle(self) -> &'static str {
        match self {
            FinDeCdi::RuptureConventionnelle => "rupture_conventionnelle",
            FinDeCdi::MiseALaRetraite => "mise_a_la_retraite",
            FinDeCdi::DepartALaRetraite => "depart_a_la_retraite",
        }
    }
}

/// La fin d'un CDI que ce motif designe (043 rupture conventionnelle, 038 mise a la retraite, 039 depart a la
/// retraite), et la rubrique de son indemnite.
pub fn rupture(motif_rupture: Option<&str>) -> Resultat<Option<(FinDeCdi, String)>> {
    let rg = regles()?;
    let Some(m) = motif_rupture else { return Ok(None) };
    let fin = [
        (FinDeCdi::RuptureConventionnelle, &rg.rupture_conventionnelle.motif_rupture, &rg.rupture_conventionnelle.rubrique),
        (FinDeCdi::MiseALaRetraite, &rg.mise_a_la_retraite.motif_rupture, &rg.mise_a_la_retraite.rubrique),
        (FinDeCdi::DepartALaRetraite, &rg.depart_a_la_retraite.motif_rupture, &rg.depart_a_la_retraite.rubrique),
    ]
    .into_iter()
    .find(|(_, motif, _)| motif.as_str() == m)
    .map(|(fin, _, rubrique)| (fin, rubrique.clone()));
    Ok(fin)
}

/// `None` si ce code d'indemnite peut accompagner ce motif (CCH-21 a
/// CCH-24), sinon pourquoi non.
pub fn code_52_permis(code: Option<&str>, motif_rupture: Option<&str>) -> Resultat<Option<String>> {
    let code = match code {
        Some(c) if !c.is_empty() => c,
        _ => return Ok(None),
    };
    let rg = regles()?;
    let Some(dit) = rg.indemnites_de_rupture.codes.get(code) else {
        return Ok(None);
    };
    let Some(motif) = motif_rupture.filter(|m| !m.is_empty()) else {
        return Ok(None);
    };
    if dit.motifs.iter().any(|m| m == motif) {
        return Ok(None);
    }
    Ok(Some(format!(
        "{} ({}) n'est permis qu'avec les motifs de rupture {}, et celui-ci est {} -- {} du cahier technique.",
        code,
        dit.libelle,
        dit.motifs.join(", "),
        motif,
        dit.controle
    )))
}

/// L'indemnite de mise a la retraite : le bareme du licenciement, sans ses
/// exclusions de motif.
pub fn indemnite_mise_a_la_retraite(anciennete: Decimal, reference: Decimal) -> Resultat<IndemniteLegale> {
    let mini = anciennete_minimale()?;
    if anciennete < mini {
        return Ok(IndemniteLegale::non_due(format!(
            "{} mois d'anciennete : L1237-7 renvoie a l'indemnite de L1234-9, qui en demande {}. \
             La convention collective peut en prevoir une.",
            mois_complets(anciennete), mini)));
    }
    let (montant, annees) = bareme_licenciement(anciennete, reference)?;
    Ok(IndemniteLegale { montant, du: true, pourquoi: None, annees: Some(annees) })
}

/// Le plancher de l'indemnite de rupture conventionnelle (L1237-13 : celle de L1234-9). Sous l'anciennete minimale de
/// L1234-9, au prorata des mois complets quand `rupture_conventionnelle.indemnite` le dit (la circulaire DGT 2009-4, au
/// taux de R1234-2 en vigueur), avec la phrase qui le dit (`pourquoi`) ; sinon pas de plancher.
pub fn indemnite_rupture_conventionnelle(anciennete: Decimal, reference: Decimal) -> Resultat<PlancherDeLaRupture> {
    let mini = anciennete_minimale()?;
    if anciennete < mini {
        if regles()?.rupture_conventionnelle.indemnite.prorata_sous_l_anciennete_minimale && !mois_complets(anciennete).is_zero() {
            let (plancher, annees) = bareme_licenciement(anciennete, reference)?;
            return Ok(PlancherDeLaRupture { plancher: Some(plancher), annees, pourquoi: Some(format!(
                "{} mois complets d'ancienneté, moins que les {} de L1234-9 : la circulaire DGT 2009-4 (5.3) dit \
                 l'indemnité spécifique due « au prorata du nombre de mois de présence » ; le plancher est celui de \
                 R1234-2 en vigueur (un quart de mois par année) sur ces mois. C'est la lecture de l'administration, \
                 qui homologue la convention, pas la loi.",
                mois_complets(anciennete), mini)) });
        }
        return Ok(PlancherDeLaRupture { plancher: None, annees: div(anciennete, Decimal::from(12)), pourquoi: Some(format!(
            "{} mois d'anciennete : L1237-13 renvoie a l'indemnite de L1234-9, qui en demande {}, \
             donc la loi ne donne pas de plancher. Le montant de la convention se saisit.",
            mois_complets(anciennete), mini)) });
    }
    let (plancher, annees) = bareme_licenciement(anciennete, reference)?;
    Ok(PlancherDeLaRupture { plancher: Some(plancher), annees, pourquoi: None })
}

/// L'indemnite de depart volontaire a la retraite : un PALIER, pas un cumul,
/// et rien sous le premier.
pub fn indemnite_depart_retraite(anciennete: Decimal, reference: Decimal) -> Resultat<IndemniteDuDepart> {
    let rg = regles()?;
    let annees = div(anciennete, douze());
    let mut tries: Vec<&PalierDeDepart> = rg.depart_a_la_retraite.indemnite.taux.iter().collect();
    tries.sort_by(|a, b| a.anciennete_annees.cmp(&b.anciennete_annees));
    let Some(premier) = tries.first() else {
        return Err(socle::echec::livree("permanents-base.json", "depart_a_la_retraite.indemnite.taux"));
    };
    let Some(palier) = tries.iter().filter(|t| annees >= t.anciennete_annees).last() else {
        return Ok(IndemniteDuDepart { montant: zero2(), du: false, mois: Decimal::ZERO, palier: None, pourquoi: Some(format!(
            "{} an(s) d'anciennete, et l'indemnite legale de depart a la retraite commence a {} ans \
             (D1237-1) -- il n'y a pas de prorata en dessous. La convention collective, elle, \
             en prevoit souvent une des la premiere annee.",
            annees_lisibles(annees), premier.anciennete_annees)) });
    };
    let mois = palier.mois_de_salaire;
    Ok(IndemniteDuDepart {
        montant: eur2(mul(mois, reference)),
        du: true,
        mois,
        palier: Some(palier.anciennete_annees.trunc().to_i64().ok_or_else(|| socle::num::illisible(&palier.anciennete_annees.to_string()))?),
        pourquoi: None,
    })
}

/// Les années à un chiffre après la virgule, au pair sur le demi, en texte.
fn annees_lisibles(annees: Decimal) -> String {
    let mut a = annees.round_dp_with_strategy(1, RoundingStrategy::MidpointNearestEven);
    a.rescale(1);
    a.to_string()
}

// -------------------------------------------------- le maintien de salaire
/// Pourquoi le maintien de salaire n'est PAS du, `None` s'il l'est
/// (`nature` absente : seule l'anciennete compte).
pub fn droit_au_maintien(nature: Option<&Nature>, anciennete_mois: Decimal) -> Resultat<Option<String>> {
    if let Some(n) = nature {
        if !n.mensuelle() {
            return Ok(Some(format!(
                "L1226-1 ne s'applique pas a un {} : l'indemnite complementaire est due aux \
                 salaries permanents, pas aux intermittents.",
                n.libelle
            )));
        }
    }
    let minimum = regles()?.maintien_de_salaire.anciennete_mois_minimum;
    if anciennete_mois < Decimal::from(minimum) {
        return Ok(Some(format!(
            "L1226-1 demande {} mois d'anciennete au premier jour de l'absence (D1226-8) ; il y en a {}.",
            minimum,
            arrondir_jours(anciennete_mois)
        )));
    }
    Ok(None)
}

/// Sept jours de carence, sauf accident du travail (l'accident de trajet
/// garde la sienne).
pub fn jours_de_carence(accident_du_travail: bool, accident_de_trajet: bool) -> Resultat<i64> {
    if accident_du_travail && !accident_de_trajet {
        return Ok(0);
    }
    Ok(regles()?.maintien_de_salaire.carence_jours)
}

/// Les tranches d'indemnisation, allongees par l'anciennete.
pub fn duree_du_maintien(anciennete_mois: Decimal) -> Resultat<Vec<Tranche>> {
    let rg = regles()?;
    let r = &rg.maintien_de_salaire;
    let maj = &r.majoration;
    let reste = anciennete_mois % douze();
    let annees = div(sub(anciennete_mois, reste), douze()).trunc().to_i64().ok_or_else(|| socle::num::illisible(&anciennete_mois.to_string()))?;
    let au_dela = (annees - 1).max(0);
    let bonus = (au_dela / maj.par_annees) * maj.jours;
    Ok(r.tranches.iter().map(|t| Tranche { jours: (t.jours + bonus).min(maj.plafond_jours), taux: t.taux }).collect())
}
