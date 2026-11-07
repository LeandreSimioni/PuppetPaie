//! LA FORME DU BAREME APPLIQUE : la pile qu'un contrat lit (`bareme::bareme_applique`), declaree une fois. L'empilement
//! se fait en JSON (le socle livre, la convention, la compagnie, puis chaque etage) ; la pile finie se lit ici, une fois
//! (`socle::forme::lire`), et le calcul de la paie comme la DSN lisent ces types, plus le JSON.
//!
//! La pile porte tout le bareme de l'annee resolu a la date (les sections que les etages ont lues comprises) : chaque cle
//! d'un fichier de bareme livre est donc connue d'un type ici, ou refusee. Les notes d'un bareme sont `source` (le texte
//! depose qui fonde la valeur, `source::Fondement`), `note` (une explication) et `quoi` (ce qu'est le fichier).

use std::collections::BTreeMap;

use crate::source::Fondement;
use serde::Deserialize;
use socle::forme::Nombre;
use socle::num::Decimal;

/// Les notes d'une valeur, communes a chaque type : elles ne se lisent pas au calcul.
macro_rules! forme {
    ($(#[$m:meta])* pub struct $nom:ident { $($corps:tt)* }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Default, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $nom {
            $($corps)*
            pub source: Option<Fondement>,
            pub note: Option<String>,
        }
    };
}

/// Un decimal qui se date, lu seul : un champ `Option<NombreDate>` n'a besoin d'aucun attribut (absent ou nul : vide).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NombreDate(pub Decimal);

impl<'de> Deserialize<'de> for NombreDate {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        datee::decimal(d).map(NombreDate)
    }
}

/// Un tarif qui se date, et qui peut n'exister encore a aucune date (une valeur nulle de la liste datee : aucun tarif
/// avant son premier avenant) : `Option<TarifDate>` ; `TarifDate(None)` est « aucun tarif a cette date ».
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TarifDate(pub Option<Decimal>);

impl<'de> Deserialize<'de> for TarifDate {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        datee::valeur::<D, Option<Nombre>>(d).map(|n| TarifDate(n.map(|n| n.0)))
    }
}

/// Un taux qui se date, lu seul (`Option<TauxDate>`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TauxDate(pub f64);

impl<'de> Deserialize<'de> for TauxDate {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        datee::valeur::<D, f64>(d).map(TauxDate)
    }
}

/// Une valeur datee qui reste une liste dans la pile (`smic_horaire`) : chaque valeur dit le texte depose qui la fonde.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Datee {
    pub depuis: Option<String>,
    #[serde(deserialize_with = "socle::forme::decimal")]
    pub valeur: Decimal,
    pub source: Fondement,
}

/// UNE VALEUR QUI CHANGE EN COURS D'ANNEE (un taux revalorise au 1er mai, un tarif qu'un accord de salaires releve) : la
/// valeur seule, ou la liste de ses valeurs datees, dont une au plus sans `depuis` (la valeur de tout temps). Seuls les
/// champs qui portent `datee::…` se datent ; ailleurs une liste datee est une forme fausse.
#[derive(Deserialize)]
#[serde(untagged)]
enum Ecrite<T> {
    Seule(T),
    Datee(Vec<Entree<T>>),
}

/// Une valeur d'une liste datee. `accord` : elle vient d'un accord de salaires (`minima-conventionnels.json`), et ne vaut
/// pour la compagnie que du jour ou il la lie (`minima::lie_depuis`).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entree<T> {
    depuis: Option<String>,
    valeur: T,
    accord: Option<String>,
    #[allow(dead_code)]
    source: Option<Fondement>,
    #[allow(dead_code)]
    note: Option<String>,
}

/// LA LECTURE EN COURS : le jour ou le bareme se lit, l'organisation patronale de la compagnie (un accord la lie peut-etre
/// plus tot), et le `depuis` de chaque valeur datee retenue.
struct Lecture {
    jour: socle::date::Jour,
    organisation: Option<String>,
    retenues: Vec<socle::date::Jour>,
}

thread_local! {
    static LECTURE: std::cell::RefCell<Option<Lecture>> = const { std::cell::RefCell::new(None) };
}

/// LE BAREME DE `fichier` LU A `jour` : chaque valeur datee y vaut ce qu'elle vaut ce jour-la (la plus recente dont le
/// `depuis` ne depasse pas `jour` ; a meme date, la derniere ecrite ; la valeur sans `depuis` vaut de tout temps), et le
/// `depuis` de chacune de celles qui s'appliquent est rendu, du plus ancien au plus recent, sans doublon (« règle du
/// JJ/MM/AAAA » d'une paie). Une valeur qui n'existe pas encore ne se devine pas : la lecture refuse.
pub(crate) fn lire_au<T: serde::de::DeserializeOwned>(
    fichier: &std::path::Path,
    jour: &socle::date::Jour,
    organisation: Option<&str>,
    phrase: &str,
) -> socle::num::Resultat<(T, Vec<socle::date::Jour>)> {
    LECTURE.with(|l| *l.borrow_mut() = Some(Lecture { jour: *jour, organisation: organisation.map(str::to_string), retenues: Vec::new() }));
    let lu = socle::forme::lire_fichier::<T>(fichier, phrase);
    let lecture = LECTURE.with(|l| l.borrow_mut().take());
    let mut retenues = lecture.ok_or_else(|| "La lecture du barème a perdu ses dates : le signaler au développeur.".to_string())?.retenues;
    retenues.sort();
    retenues.dedup();
    Ok((lu?, retenues))
}

/// Les champs qui se datent : `#[serde(deserialize_with = "datee::…")]`.
pub(crate) mod datee {
    use super::{Ecrite, Entree, LECTURE};
    use serde::de::Error;
    use serde::{Deserialize, Deserializer};
    use socle::date::Jour;
    use socle::num::Decimal;

    /// La valeur a la date de la lecture en cours.
    fn au_jour<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<T, D::Error> {
        let entrees = match Ecrite::<T>::deserialize(d).map_err(|_| {
            D::Error::custom("la valeur n'a pas la forme attendue : la valeur seule, ou la liste de ses valeurs datées [{depuis, valeur}]")
        })? {
            Ecrite::Seule(v) => return Ok(v),
            Ecrite::Datee(e) => e,
        };
        retenir_a_la_date::<D, T>(entrees)
    }

    fn retenir_a_la_date<'de, D: Deserializer<'de>, T>(entrees: Vec<Entree<T>>) -> Result<T, D::Error> {
        LECTURE.with(|l| {
            let mut l = l.borrow_mut();
            let lecture = l.as_mut().ok_or_else(|| D::Error::custom("une valeur datée ne se lit qu'à une date"))?;
            retenir(entrees, lecture).map_err(D::Error::custom)
        })
    }

    fn retenir<T>(entrees: Vec<Entree<T>>, lecture: &mut super::Lecture) -> Result<T, String> {
        let mut datees: Vec<(Option<Jour>, T)> = Vec::new();
        for e in entrees {
            let depuis = e.depuis.as_deref().map(Jour::lire).transpose()?;
            let depuis = match e.accord {
                None => depuis,
                Some(source) => {
                    let t = crate::minima::table()?;
                    let accord = crate::minima::accord(&t, &source).ok_or_else(|| socle::echec::livree("minima-conventionnels.json", &source))?;
                    let ecrit = depuis.map(|j| j.iso()).unwrap_or_default();
                    match crate::minima::lie_depuis(accord, lecture.organisation.as_deref(), &ecrit) {
                        Some(j) => Some(Jour::lire(&j)?),
                        None => continue,
                    }
                }
            };
            datees.push((depuis, e.valeur));
        }
        datees.sort_by_key(|(j, _)| *j);
        let jour = lecture.jour;
        let (depuis, v) = datees
            .into_iter()
            .filter(|(j, _)| j.is_none_or(|j| j <= jour))
            .last()
            .ok_or_else(|| format!("aucune valeur ne s'applique au {}", jour.jj_mm_aaaa()))?;
        lecture.retenues.extend(depuis);
        Ok(v)
    }

    /// Une valeur qui se date (`T`, nulle comprise quand `T` est une `Option`).
    pub fn valeur<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<T, D::Error> {
        au_jour(d)
    }

    /// `valeur`, pour une cle qui peut manquer (rien ne change) ou etre nulle (elle s'efface) : `socle::forme::dite`.
    pub fn dite<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
        au_jour(d).map(Some)
    }

    /// Un decimal qui se date (`socle::forme::decimal`).
    pub fn decimal<'de, D: Deserializer<'de>>(d: D) -> Result<Decimal, D::Error> {
        au_jour::<D, socle::forme::Nombre>(d).map(|n| n.0)
    }
}

forme! {
    /// LA PILE APPLIQUEE.
    pub struct Pile {
        pub annee: i32,
        pub quoi: Option<String>,
        pub smic_horaire: Vec<Datee>,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub heures_par_cachet: Decimal,
        pub logement_grand_deplacement: Option<FraisParZone>,
        pub repas_deplacement: Option<FraisDeRepas>,
        pub frais_hors_metropole: Option<FraisHorsMetropole>,
        pub taxe_sur_les_salaires: Option<TaxeSurLesSalaires>,
        pub solde_taxe_apprentissage: Option<Taux>,
        pub pas_taux_par_defaut: Option<PasParDefaut>,
        pub plafond_securite_sociale: Option<PlafondSecuriteSociale>,
        /// La part minimale de l'employeur a la complementaire sante collective (CSS L. 911-7, III).
        pub sante_obligatoire: Option<SanteObligatoire>,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub abattement_csg: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub majoration_conges_115: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub majoration_conges_110: Decimal,
        /// Le plafond journalier (T1 d'un jour travaille) des intermittents, et les multiples de la T2 et de l'Apec.
        pub plafond_journalier: Option<Nombre>,
        pub t2_multiple: Option<Nombre>,
        pub apec_multiple: Option<Nombre>,
        /// Les textes de ces plafonds.
        pub plafonds_des_intermittents: Option<Notes>,
        pub rubriques_bulletin: Vec<Rubrique>,
        pub cotisations_dsn: Vec<CotisationDsn>,
        pub cotisation_prevoyance_retraite_supp: Option<CotisationSupplementaire>,
        pub cotisation_sante_patronale: Option<CotisationSupplementaire>,
        pub reduction_generale: Option<ReductionGenerale>,
        pub majoration_chomage_cddu: Option<MajorationChomage>,
        pub statuts: Vec<String>,
        pub par_statut: BTreeMap<String, CoucheStatut>,
        pub deduction_forfaitaire: Option<DeductionForfaitaire>,
        pub par_categorie: BTreeMap<String, CoucheCategorie>,
        pub par_nature: BTreeMap<String, CoucheNature>,
        pub par_caisse_conges: Option<CaisseConges>,
        pub risques_atmp: Option<RisquesAtmp>,
        pub seuils_effectif: Option<SeuilsEffectif>,
        pub regime_local_alsace_moselle: Option<RegimeLocal>,
        /// Le versement mobilite de la compagnie ou de l'etablissement (sa fiche) : `bareme::versement_mobilite` le lit.
        pub versement_mobilite: Option<VersementMobiliteEcrit>,
        pub par_organisation_patronale: BTreeMap<String, Organisation>,
        pub retraite_t1_repartition: Option<RetraiteT1Repartition>,
        pub par_convention: BTreeMap<String, Convention>,
        // La couche de la convention de la compagnie, fondue dans le socle (`contexte::convention`) : le socle ne l'ecrit pas.
        pub contributions_conventionnelles: Option<Vec<Contribution>>,
        pub tarifs_conventionnels: Option<Tarifs>,
        pub contributions_dsn: Option<Vec<ContributionDsn>>,
        pub secteurs: Option<Secteurs>,
        // Le taux AT/MP de la surcouche de la compagnie (`Surcouche`), ou ce que le moteur en dit.
        #[serde(skip)]
        pub taux_atmp: Option<TauxAtmp>,
        // Ce que le moteur pose sur la pile : l'etablissement du salarie (`bareme::surcharger`). La compagnie, la pile la
        // recoit typee (`bareme::EntrepriseLue`), elle ne la porte pas.
        #[serde(skip)]
        pub etablissement: Option<String>,
    }
}

/// LE VERSEMENT MOBILITE QUE LA COMPAGNIE OU L'ETABLISSEMENT ECRIT : son taux et la commune de l'etablissement, ou une
/// reponse en mots (« aucun », `seuils_effectif.non_portees`).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum VersementMobiliteEcrit {
    Dit(String),
    Ecrit(VersementMobiliteTaux),
}

forme! {
    /// Le taux du versement mobilite et la commune (code INSEE) de l'etablissement, l'autorite qui le fixe.
    pub struct VersementMobiliteTaux {
        pub taux: Option<Nombre>,
        pub code_insee: Option<String>,
        pub autorite: Option<String>,
    }
}

impl VersementMobiliteEcrit {
    /// Rien d'ecrit : une reponse vide.
    pub fn est_vide(&self) -> bool {
        matches!(self, VersementMobiliteEcrit::Dit(s) if s.is_empty())
    }
}

/// La date et l'effet d'une notification du taux AT/MP qu'un retour a posee (`moteur::routes::atmp::poser_notifie`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationAtmp {
    pub le: String,
    pub calcul: String,
    pub effet: String,
}

forme! {
    /// La majoration du chomage d'un CDD d'usage court (`majoration_chomage`) : la ligne du bulletin qui la porte, son
    /// taux, sa cotisation au nominatif.
    pub struct MajorationChomage {
        pub rubrique: String,
        pub taux: f64,
        pub code81: String,
    }
}

forme! {
    pub struct FraisParZone {
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub paris: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub autres: Decimal,
    }
}

forme! {
    pub struct FraisDeRepas {
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub restaurant: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub hors_restaurant: Decimal,
    }
}

forme! {
    pub struct LogementEtRepas {
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub logement: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub repas: Decimal,
    }
}

forme! {
    pub struct FraisHorsMetropole {
        pub dom: LogementEtRepas,
        pub pacifique: LogementEtRepas,
        pub monaco: LogementEtRepas,
    }
}

forme! {
    pub struct TaxeSurLesSalaires {
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub taux_normal: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub majoration_1: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub majoration_2: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub seuil_1: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub seuil_2: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub franchise: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub decote_jusqu_a: Decimal,
        #[serde(rename = "abattement_1679A", deserialize_with = "socle::forme::decimal")]
        pub abattement_1679a: Decimal,
        pub plafonds_franchise: PlafondsFranchise,
    }
}

forme! {
    pub struct PlafondsFranchise {
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub prestations_de_services: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub total: Decimal,
        pub en_vigueur_depuis: String,
    }
}

forme! {
    pub struct Taux {
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub taux: Decimal,
    }
}

forme! {
    pub struct PasParDefaut {
        pub abattement_contrats_courts: Option<NombreDate>,
        pub grilles: Vec<GrillePas>,
    }
}

forme! {
    /// Une grille du taux par defaut (CGI 204 H, III) : ses tranches `[jusqu'a, taux]`.
    pub struct GrillePas {
        pub depuis: String,
        pub zone: String,
        #[serde(rename = "type")]
        pub genre: String,
        pub tranches: Vec<Vec<Option<f64>>>,
    }
}

forme! {
    /// La part de l'employeur a la complementaire sante collective obligatoire, en pourcent : au moins la moitie.
    pub struct SanteObligatoire {
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub part_employeur_minimum: Decimal,
    }
}

forme! {
    pub struct PlafondSecuriteSociale {
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub mensuel: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub horaire: Decimal,
    }
}

forme! {
    /// Une ligne de cotisation du bulletin.
    pub struct Rubrique {
        pub id: String,
        pub famille: Option<String>,
        pub libelle: Option<String>,
        /// La cle de son assiette dans les bases du calcul (`brut`, `brut_x115`, `t1_journaliere`…).
        pub base: Option<String>,
        /// Les deux parts, en pourcent ; nulle : pas de part de ce cote. La cle s'ecrit : un taux qui manque ne se devine pas.
        #[serde(deserialize_with = "datee::valeur")]
        pub taux_pat: Option<f64>,
        pub taux_sal: Option<f64>,
        /// Une part patronale qui se compose de plusieurs taux, chacun arrondi au centime (`composants`, ou
        /// `composants_pat`) ; la part salariale de meme (`composants_sal`).
        pub composants: Option<Vec<f64>>,
        pub composants_pat: Option<Vec<f64>>,
        pub composants_sal: Option<Vec<f64>>,
        /// Le complement de taux patronal au-dela d'un seuil de SMIC.
        pub complement: Option<Complement>,
        /// La CET des cadres, fondue dans la ligne quand elle est due.
        pub cet_si_depasse: Option<Cet>,
        pub masquer_base_et_taux: Option<bool>,
        /// Le type de composant de la base 31 de la DSN (S21.G00.79.001) que la sante pose (`bareme::bareme_sante`).
        pub composant_79: Option<String>,
        /// Le facteur du taux AT/MP de la compagnie (`bareme::bareme_atmp`) : 0,7 pour un artiste (abattement de 30 %).
        pub facteur_atmp: Option<f64>,
        /// Ce qui remplit le taux, quand ce n'est pas le bareme (le taux AT/MP, la prevoyance d'une convention).
        pub rempli_par: Option<String>,
        /// La ligne de la taxe d'apprentissage, que la pile retire d'une compagnie qui n'y est pas assujettie.
        pub taxe_apprentissage: Option<bool>,
        /// Une rubrique ajoutee : celle apres laquelle elle se range (`bareme::empiler`).
        pub apres: Option<String>,
        /// Les niveaux de la complementaire sante d'un permanent de la 3090 (`bareme::bareme_sante`).
        pub releve_svp_sante: Option<ReleveSante>,
        pub a_confirmer: Option<String>,
    }
}

forme! {
    /// Le taux qui s'ajoute au taux patronal quand la remuneration depasse `seuil_smic` fois le SMIC (sans seuil : toujours).
    pub struct Complement {
        pub taux: f64,
        pub seuil_smic: Option<Nombre>,
    }
}

forme! {
    /// Les taux patronal et salarial de la CET.
    pub struct Cet {
        pub pat: f64,
        pub sal: f64,
    }
}

/// Les niveaux de la complementaire sante releves (fiche de parametrage Audiens), et la part de l'employeur, que le
/// releve dit (L. 911-7 : au moins la moitie) : sans valeur par defaut.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleveSante {
    pub niveaux: BTreeMap<String, NiveauSante>,
    pub part_employeur: Nombre,
    pub composant_79: Option<Composant79>,
    pub source: Option<Fondement>,
    pub note: Option<String>,
}

forme! {
    pub struct NiveauSante {
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub assure: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub conjoint: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub enfant: Decimal,
    }
}

forme! {
    pub struct Composant79 {
        #[serde(rename = "type")]
        pub genre: String,
    }
}

forme! {
    /// Une modification d'une rubrique du corps par une couche (`rubriques` d'une section) : ce qu'elle remplace.
    pub struct RubriqueModifiee {
        pub famille: Option<String>,
        pub libelle: Option<String>,
        pub base: Option<String>,
        #[serde(default, deserialize_with = "datee::dite")]
        pub taux_pat: Option<Option<f64>>,
        #[serde(default, deserialize_with = "socle::forme::dite")]
        pub taux_sal: Option<Option<f64>>,
        #[serde(default, deserialize_with = "datee::dite")]
        pub composants: Option<Vec<f64>>,
        pub composants_pat: Option<Vec<f64>>,
        pub composants_sal: Option<Vec<f64>>,
        pub cet_si_depasse: Option<Cet>,
        pub rempli_par: Option<String>,
        pub a_confirmer: Option<String>,
    }
}

forme! {
    /// Une cotisation individuelle de la DSN (bloc 81), et la base qu'elle declare au bloc 78.
    pub struct CotisationDsn {
        pub bloc78: String,
        pub code81: Option<String>,
        pub org: Option<String>,
        pub base: Option<String>,
        pub taux: Option<TauxDate>,
        pub base78: String,
        /// La base de la ligne s'arrete au plafond de la periode.
        pub plafond: Option<bool>,
        pub composants: Option<Vec<f64>>,
        pub affectation: Option<String>,
        pub organisme: Option<String>,
        /// La cotisation n'est due qu'au-dela de ce multiple du SMIC.
        pub seuil_smic: Option<Nombre>,
        pub cle_total: Option<String>,
        /// La commune du versement mobilite (`bareme::bareme_vm`).
        pub code_insee: Option<String>,
        /// Les lignes du bulletin dont la cotisation est la somme.
        pub lignes: Option<Vec<String>>,
        /// Une cotisation ajoutee : celle apres laquelle elle se range.
        pub apres: Option<String>,
    }
}

forme! {
    /// Une modification d'une cotisation du corps par une couche (`cotisations_dsn` d'une categorie, par code81 ou
    /// `code81:affectation`).
    pub struct CotisationModifiee {
        #[serde(default, deserialize_with = "socle::forme::dite")]
        pub taux: Option<Option<f64>>,
        pub composants: Option<Vec<f64>>,
        pub base: Option<String>,
        pub base78: Option<String>,
        pub lignes: Option<Vec<String>>,
    }
}

forme! {
    /// Une cotisation hors des lignes 81 (S21.G00.54, types 92 et 93) : un taux sur une base, ou la somme de lignes.
    pub struct CotisationSupplementaire {
        pub taux_sur_brut: Option<Nombre>,
        pub base: Option<String>,
        pub lignes_patronales: Option<Vec<String>>,
    }
}

forme! {
    /// La reduction generale des cotisations (CSS L. 241-13), ou ce qu'une couche en complete.
    pub struct ReductionGenerale {
        pub verifie: Option<bool>,
        pub statuts: Option<Vec<String>>,
        pub formule: Option<String>,
        pub tmin: Option<Nombre>,
        pub tdelta: Option<Nombre>,
        pub puissance: Option<Nombre>,
        pub t: Option<NombreDate>,
        pub plafond_smic: Option<Nombre>,
        pub smic_horaire_reference: Option<Nombre>,
        pub imputation: Option<Imputation>,
        pub dsn: Option<ReductionDsn>,
        /// La fraction du coefficient d'un salarie declare a la caisse des Conges Spectacles (`100/90`).
        pub majoration_caisse_conges: Option<String>,
    }
}

forme! {
    pub struct Imputation {
        pub chomage: Option<NombreDate>,
        pub agirc_arrco: Option<Nombre>,
    }
}

forme! {
    /// Ce que la DSN declare de la reduction : les codes 81 de ses parts, le CTP du bordereau.
    pub struct ReductionDsn {
        pub code81_urssaf: Option<String>,
        pub code81_chomage: Option<String>,
        pub code81_retraite: Option<String>,
        pub ctp: Option<String>,
        pub ctp_regularisation: Option<String>,
        pub qualifiant: Option<String>,
        pub cle_total_chomage: Option<String>,
        pub cle_total_urssaf: Option<String>,
    }
}

forme! {
    /// La section d'un statut (`par_statut`) : ses sections remplacent celles du corps (`bareme::bareme_du_statut`).
    pub struct CoucheStatut {
        pub verifie: Option<bool>,
        pub rubriques_bulletin: Option<Vec<Rubrique>>,
        pub cotisations_dsn: Option<Vec<CotisationDsn>>,
        pub reduction_generale: Option<ReductionGenerale>,
    }
}

forme! {
    /// La section d'une categorie (`par_categorie` : cadre, cadre permanent), et ses sections par statut.
    pub struct CoucheCategorie {
        pub verifie: Option<bool>,
        pub plafond_journalier: Option<Nombre>,
        pub t2_multiple: Option<Nombre>,
        pub apec_multiple: Option<Nombre>,
        pub rubriques: Option<BTreeMap<String, RubriqueModifiee>>,
        pub rubriques_ajoutees: Option<Vec<Rubrique>>,
        pub cotisation_prevoyance_retraite_supp: Option<CotisationSupplementaire>,
        pub cotisations_ajoutees: Option<Vec<CotisationDsn>>,
        pub cotisations_dsn: Option<BTreeMap<String, CotisationModifiee>>,
        pub par_statut: Option<BTreeMap<String, CoucheCategorie>>,
    }
}

forme! {
    /// La section d'une nature de contrat (`par_nature` : permanent) : remplacer, retirer, ajouter des rubriques.
    pub struct CoucheNature {
        pub verifie: Option<bool>,
        pub rubriques: Option<BTreeMap<String, RubriqueModifiee>>,
        pub rubriques_retirees: Option<Vec<String>>,
        pub rubriques_ajoutees: Option<Vec<Rubrique>>,
        pub majoration_conges_115: Option<Nombre>,
        pub majoration_conges_110: Option<Nombre>,
        pub cotisation_prevoyance_retraite_supp: Option<CotisationSupplementaire>,
        pub cotisation_sante_patronale: Option<CotisationSupplementaire>,
        pub cpf_cdd: Option<CpfCdd>,
        pub reduction_generale: Option<ReductionGenerale>,
        pub cotisations_dsn: Option<Vec<CotisationDsn>>,
    }
}

forme! {
    /// La contribution CPF-CDD d'un CDD de droit commun (C. trav. L. 6331-6) : un composant de plus a une rubrique.
    pub struct CpfCdd {
        pub rubrique: String,
        pub taux: f64,
        pub cotisation_dsn: CotisationDsn,
    }
}

forme! {
    pub struct CaisseConges {
        pub affilie: CoucheCaisse,
        pub non_affilie: CoucheCaisse,
    }
}

forme! {
    /// Ce que la caisse des Conges Spectacles change a la pile (`bareme::bareme_caisse_conges`).
    pub struct CoucheCaisse {
        pub verifie: Option<bool>,
        pub du_corps: Option<DuCorps>,
        pub rubriques_retirees: Option<Vec<String>>,
        pub cotisations_dsn_retirees: Option<Vec<String>>,
        pub majoration_conges_115: Option<Nombre>,
        pub majoration_conges_110: Option<Nombre>,
    }
}

forme! {
    pub struct DuCorps {
        pub rubriques_bulletin: Vec<String>,
        pub cotisations_dsn: Vec<String>,
    }
}

forme! {
    pub struct DeductionForfaitaire {
        pub verifie: Option<bool>,
        pub professions: BTreeMap<String, Profession>,
        pub frais_cumulables: bool,
        pub taux: BTreeMap<String, socle::forme::Nombre>,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub plafond_annuel: Decimal,
        pub rubriques_abattues: Vec<String>,
    }
}

forme! {
    pub struct Profession {
        pub libelle: String,
        pub categorie: String,
        pub frais_hors_assiette: Vec<String>,
        pub taux: Option<Nombre>,
    }
}

forme! {
    pub struct RisquesAtmp {
        pub arrete: String,
        pub par_defaut: Option<String>,
        pub codes: Vec<RisqueAtmp>,
    }
}

forme! {
    pub struct RisqueAtmp {
        pub code: String,
        pub code_arrete: Option<String>,
        pub taux: Option<f64>,
        pub tc: bool,
        pub alsace_moselle: Option<AtmpAlsaceMoselle>,
        pub libelle: Option<String>,
        pub precisions: Option<String>,
    }
}

forme! {
    pub struct AtmpAlsaceMoselle {
        pub groupe: i64,
        pub taux: f64,
    }
}

forme! {
    /// Les valeurs du bareme qui changent avec l'effectif (`contexte::controler_effectif`).
    pub struct SeuilsEffectif {
        pub valeurs: Vec<SeuilEffectif>,
        pub non_portees: Vec<SeuilNonPorte>,
    }
}

forme! {
    pub struct SeuilEffectif {
        pub cle: String,
        pub seuil: i64,
        pub quoi: String,
        pub au_dela: String,
        pub sous_le_seuil: Option<String>,
        /// Les textes qui fondent le seuil, en clair.
        pub texte: String,
        /// Chaque valeur du bareme que le seuil change, et sa valeur sous le seuil.
        pub effets: Vec<Effet>,
    }
}

/// UNE VALEUR DU BAREME QU'UN SEUIL D'EFFECTIF CHANGE, et ce qu'elle vaut sous le seuil (`contexte::controler_effectif`) :
/// la pile la lit (`Effet::lu`) pour savoir si la compagnie l'a decidee.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Effet {
    /// Le taux d'une cotisation de la DSN (bloc 81), par son code.
    TauxDsn { section: Section, code81: String, taux: f64, source: Fondement },
    /// Le taux patronal d'une rubrique du bulletin (de sa modification, pour une nature).
    TauxPatronal { section: Section, rubrique: String, taux: f64, source: Fondement },
    /// Un composant du taux patronal d'une rubrique qu'une nature ajoute.
    ComposantAjoute { nature: String, rubrique: String, rang: usize, taux: f64, source: Fondement },
    /// Un parametre de la reduction generale d'un statut.
    ReductionGenerale {
        statut: String,
        parametre: ParametreReduction,
        #[serde(deserialize_with = "datee::decimal")]
        valeur: Decimal,
        source: Fondement,
    },
}

/// La section du bareme ou un effet porte : le corps, la section d'un statut, celle d'une nature.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Section {
    Corps,
    Statut(String),
    Nature(String),
}

/// Le parametre de la reduction generale qu'un seuil change.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParametreReduction {
    T,
    Tdelta,
}

/// Ce que la pile porte a la place d'un effet : un taux, ou un parametre de la reduction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Lu {
    Taux(f64),
    Parametre(Decimal),
}

impl Lu {
    /// Le meme nombre ?
    pub fn meme(&self, autre: &Lu) -> bool {
        match (self, autre) {
            (Lu::Taux(a), Lu::Taux(b)) => (a - b).abs() < 1e-9,
            (Lu::Parametre(a), Lu::Parametre(b)) => a == b,
            _ => false,
        }
    }
}

impl std::fmt::Display for Lu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Lu::Taux(t) => write!(f, "{}", socle::ecriture::repr_flottant(*t)),
            Lu::Parametre(p) => write!(f, "{}", p),
        }
    }
}

impl Effet {
    /// Ce que l'effet vaut sous le seuil.
    pub fn sous_le_seuil(&self) -> Lu {
        match self {
            Effet::TauxDsn { taux, .. } | Effet::TauxPatronal { taux, .. } | Effet::ComposantAjoute { taux, .. } => Lu::Taux(*taux),
            Effet::ReductionGenerale { valeur, .. } => Lu::Parametre(*valeur),
        }
    }

    /// Ce que la pile porte a sa place, `None` si elle ne le porte pas.
    pub fn lu(&self, pile: &Pile) -> Option<Lu> {
        let cotisation = |l: &[CotisationDsn], code: &str| l.iter().find(|c| c.code81.as_deref() == Some(code)).and_then(|c| c.taux).map(|t| Lu::Taux(t.0));
        let rubrique = |l: &[Rubrique], id: &str| l.iter().find(|r| r.id == id).and_then(|r| r.taux_pat).map(Lu::Taux);
        match self {
            Effet::TauxDsn { section: Section::Corps, code81, .. } => cotisation(&pile.cotisations_dsn, code81),
            Effet::TauxDsn { section: Section::Statut(s), code81, .. } => cotisation(pile.par_statut.get(s)?.cotisations_dsn.as_deref()?, code81),
            Effet::TauxDsn { section: Section::Nature(n), code81, .. } => cotisation(pile.par_nature.get(n)?.cotisations_dsn.as_deref()?, code81),
            Effet::TauxPatronal { section: Section::Corps, rubrique: id, .. } => rubrique(&pile.rubriques_bulletin, id),
            Effet::TauxPatronal { section: Section::Statut(s), rubrique: id, .. } => rubrique(pile.par_statut.get(s)?.rubriques_bulletin.as_deref()?, id),
            Effet::TauxPatronal { section: Section::Nature(n), rubrique: id, .. } => pile.par_nature.get(n)?.rubriques.as_ref()?.get(id)?.taux_pat.flatten().map(Lu::Taux),
            Effet::ComposantAjoute { nature, rubrique: id, rang, .. } => {
                pile.par_nature.get(nature)?.rubriques_ajoutees.as_ref()?.iter().find(|r| r.id == *id)?.composants.as_ref()?.get(*rang).copied().map(Lu::Taux)
            }
            Effet::ReductionGenerale { statut, parametre, .. } => {
                let r = pile.par_statut.get(statut)?.reduction_generale.as_ref()?;
                match parametre {
                    ParametreReduction::T => r.t.map(|n| n.0),
                    ParametreReduction::Tdelta => r.tdelta.map(|n| n.0),
                }
                .map(Lu::Parametre)
            }
        }
    }
}

impl std::fmt::Display for Effet {
    /// La valeur, nommee par sa place dans le bareme (`par_nature.permanent.cotisations_dsn[128].taux`).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let devant = |s: &Section| match s {
            Section::Corps => String::new(),
            Section::Statut(x) => format!("par_statut.{}.", x),
            Section::Nature(x) => format!("par_nature.{}.", x),
        };
        match self {
            Effet::TauxDsn { section, code81, .. } => write!(f, "{}cotisations_dsn[{}].taux", devant(section), code81),
            Effet::TauxPatronal { section: Section::Nature(n), rubrique, .. } => write!(f, "par_nature.{}.rubriques.{}.taux_pat", n, rubrique),
            Effet::TauxPatronal { section, rubrique, .. } => write!(f, "{}rubriques_bulletin[{}].taux_pat", devant(section), rubrique),
            Effet::ComposantAjoute { nature, rubrique, rang, .. } => write!(f, "par_nature.{}.rubriques_ajoutees[{}].composants[{}]", nature, rubrique, rang),
            Effet::ReductionGenerale { statut, parametre, .. } => write!(
                f,
                "par_statut.{}.reduction_generale.{}",
                statut,
                match parametre {
                    ParametreReduction::T => "t",
                    ParametreReduction::Tdelta => "tdelta",
                }
            ),
        }
    }
}

/// CE QUE LE MOTEUR NE PORTE PAS au-dela d'un seuil, et que la compagnie dit elle-meme dans sa surcouche.
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NonPortee {
    /// Le versement mobilite (`Pile::versement_mobilite`) : un taux et la commune, ou une reponse.
    #[default]
    VersementMobilite,
}

impl std::fmt::Display for NonPortee {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NonPortee::VersementMobilite => write!(f, "versement_mobilite"),
        }
    }
}

forme! {
    pub struct SeuilNonPorte {
        pub cle: NonPortee,
        pub seuil: i64,
        pub quoi: String,
        pub texte: Option<String>,
        pub reponses: BTreeMap<String, String>,
    }
}

forme! {
    pub struct RegimeLocal {
        pub taux_sal: f64,
        pub code81: String,
        pub ctp: String,
    }
}

forme! {
    /// La couche d'une organisation patronale : la retraite T1 de ses adherents (`bareme::bareme_retraite_t1`).
    pub struct Organisation {
        pub libelle: String,
        pub applicable: bool,
        pub retraite_t1: Option<RetraiteT1ParStatut>,
        pub adhesion_avant: Option<String>,
        pub alias: Option<Vec<String>>,
        pub a_confirmer: Option<String>,
    }
}

forme! {
    pub struct RetraiteT1ParStatut {
        pub artiste: Option<f64>,
        pub technicien: Option<f64>,
    }
}

forme! {
    pub struct RetraiteT1Repartition {
        pub taux_droit_commun: f64,
        pub ceg: Cet,
        pub avant_1999: String,
        pub par_idcc: BTreeMap<String, RepartitionDeBranche>,
    }
}

forme! {
    pub struct RepartitionDeBranche {
        pub permanent: Option<String>,
        pub technicien: Option<String>,
        pub artiste: Option<String>,
    }
}

forme! {
    /// La couche d'une convention collective (`par_convention`) : son bareme, fondu dans le socle pour la compagnie qui
    /// la nomme, et ce qu'elle fait payer hors du bulletin.
    pub struct Convention {
        pub libelle: String,
        pub applicable: bool,
        pub bareme: Option<CoucheConvention>,
        pub hors_bulletin: Option<HorsBulletin>,
        pub a_confirmer: Option<String>,
    }
}

forme! {
    /// LA COUCHE D'UNE CONVENTION, fondue dans le socle pour la compagnie qui la nomme (`contexte::fondre_la_convention`) :
    /// ce qu'elle ajoute (ses tarifs, ses contributions, son champ) et ce qu'elle change aux rubriques du socle.
    pub struct CoucheConvention {
        pub tarifs_conventionnels: Option<Tarifs>,
        pub contributions_conventionnelles: Option<Vec<Contribution>>,
        pub contributions_dsn: Option<Vec<ContributionDsn>>,
        pub secteurs: Option<Secteurs>,
        /// Ce qu'elle change aux rubriques du corps, par leur `id`.
        pub rubriques: Option<BTreeMap<String, RubriqueModifiee>>,
        pub par_nature: Option<BTreeMap<String, NatureModifiee>>,
        pub par_categorie: Option<BTreeMap<String, CategorieModifiee>>,
    }
}

forme! {
    /// Ce qu'une couche (la convention, la compagnie) change a la section d'une nature : ses rubriques modifiees et
    /// ajoutees, par leur `id`, ses cotisations de la DSN, par leur code (`code81` ou `code81:affectation`).
    pub struct NatureModifiee {
        pub rubriques: Option<BTreeMap<String, RubriqueModifiee>>,
        pub rubriques_ajoutees: Option<BTreeMap<String, RubriqueModifiee>>,
        pub cotisations_dsn: Option<BTreeMap<String, CotisationModifiee>>,
    }
}

forme! {
    /// Ce que la convention change a la section d'une categorie : ses rubriques modifiees, et la section entiere d'un
    /// de ses statuts.
    pub struct CategorieModifiee {
        pub rubriques: Option<BTreeMap<String, RubriqueModifiee>>,
        pub par_statut: Option<BTreeMap<String, CoucheCategorie>>,
    }
}

forme! {
    /// Ce que la compagnie change a la section d'un statut : ses rubriques et ses cotisations, la reduction generale.
    pub struct StatutModifie {
        pub rubriques: Option<BTreeMap<String, RubriqueModifiee>>,
        pub cotisations_dsn: Option<BTreeMap<String, CotisationModifiee>>,
        pub reduction_generale: Option<ReductionGenerale>,
    }
}

forme! {
    /// LA SURCOUCHE DE LA COMPAGNIE (`config/baremes/<an>.json`) : le socle qu'elle reprend, son taux AT/MP, son versement
    /// mobilite, et ce qu'elle decide au-dela d'un seuil d'effectif (`seuils_effectif`) : les rubriques et les cotisations
    /// qu'elle change, par leur `id` ou leur code (`code81` ou `code81:affectation`).
    pub struct Surcouche {
        pub annee: i32,
        pub socle: i32,
        pub quoi: Option<String>,
        pub taux_atmp: Option<TauxAtmpEcrit>,
        pub versement_mobilite: Option<VersementMobiliteEcrit>,
        pub rubriques: Option<BTreeMap<String, RubriqueModifiee>>,
        pub cotisations_dsn: Option<BTreeMap<String, CotisationModifiee>>,
        pub par_statut: Option<BTreeMap<String, StatutModifie>>,
        pub par_nature: Option<BTreeMap<String, NatureModifiee>>,
    }
}

forme! {
    /// Les seules notes d'un groupe de valeurs (les textes de valeurs qui ne portent pas de note a elles).
    pub struct Notes {}
}

forme! {
    /// Une contribution conventionnelle, rubrique du bulletin que la convention ajoute ou remplace
    /// (`bareme::bareme_convention`).
    pub struct Contribution {
        pub id: String,
        pub famille: Option<String>,
        pub libelle: Option<String>,
        pub base: Option<BaseParPopulation>,
        #[serde(default, deserialize_with = "socle::forme::dite")]
        pub taux_pat: Option<Option<f64>>,
        #[serde(default, deserialize_with = "socle::forme::dite")]
        pub taux_sal: Option<Option<f64>>,
        pub masquer_base_et_taux: Option<bool>,
        pub par_representation: Option<BTreeMap<String, TauxParPopulation>>,
        pub apres: Option<String>,
    }
}

/// Une base, la meme pour tous ou selon la population (intermittent, permanent).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum BaseParPopulation {
    Une(String),
    Par { intermittent: String, permanent: String },
}

/// Un taux, le meme pour tous ou selon la population.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum TauxParPopulation {
    Un(f64),
    Par { intermittent: f64, permanent: f64 },
}

forme! {
    /// Les tarifs des indemnites de deplacement que la convention fixe, a la date (nuls : aucun).
    pub struct Tarifs {
        pub repas: Option<TarifDate>,
        pub nuitee: Option<TarifDate>,
    }
}

forme! {
    /// Une contribution de branche que la DSN declare (bloc 82), la somme de lignes du bulletin.
    pub struct ContributionDsn {
        pub contribution: String,
        pub lignes: Vec<String>,
        pub en_dsn_depuis: String,
    }
}

forme! {
    /// Le champ de la convention, quand il depend du secteur de la compagnie (`contexte::controler_secteur`).
    pub struct Secteurs {
        pub champ: ChampDuSecteur,
        pub libelle: String,
        pub portes: Vec<String>,
        pub refus: String,
    }
}

/// La case de la fiche entreprise qui dit le secteur d'une convention.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChampDuSecteur {
    #[default]
    #[serde(rename = "secteur_2121")]
    Secteur2121,
}

impl ChampDuSecteur {
    /// Ce que la fiche y dit, vide si rien.
    pub fn dit(self, fiche: &crate::fiche::ChampsEntreprise) -> String {
        match self {
            ChampDuSecteur::Secteur2121 => fiche.secteur_2121.code().to_string(),
        }
    }
}

forme! {
    /// Ce que la convention fait payer hors du bulletin (les etats annuels).
    pub struct HorsBulletin {
        pub contribution_conventionnelle_permanents: Option<ParEffectif>,
        pub financement_paritarisme: Option<Paritarisme>,
        pub cpnef_dialogue_social: Option<Paritarisme>,
    }
}

forme! {
    pub struct ParEffectif {
        pub par_effectif: Vec<TauxJusquA>,
    }
}

forme! {
    pub struct TauxJusquA {
        pub jusqu_a: Option<i64>,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub taux: Decimal,
    }
}

forme! {
    pub struct Paritarisme {
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub taux: Decimal,
        pub plancher: Option<Nombre>,
        pub plafond: Option<Nombre>,
        pub assiette: Option<String>,
        pub collecteur: Option<String>,
        pub avertissement: Option<String>,
    }
}

/// LE TAUX AT/MP DE LA COMPAGNIE OU DE L'ETABLISSEMENT : la surcouche de l'annee (`moteur::routes::atmp`), ce que le
/// moteur en reprend d'une annee d'avant (`bareme::taux_atmp_reporte`) ou en dit quand il manque.
#[derive(Debug, Clone, Default)]
pub struct TauxAtmp {
    pub valeur: Option<Decimal>,
    pub code_risque: Option<String>,
    pub provenance: Option<String>,
    pub pose_a_la_creation: Option<bool>,
    pub notification: Option<NotificationAtmp>,
    pub manque: Option<String>,
    pub reporte_de: Option<i32>,
    pub alsace_moselle: Option<bool>,
    pub etablissement_neuf: Option<bool>,
}

/// LE TAUX AT/MP TEL QUE LA SURCOUCHE DE L'ANNEE L'ECRIT (`moteur::routes::atmp`), ou que le moteur le reprend d'une
/// annee d'avant (`bareme::taux_atmp_reporte`) : sa valeur, ou la liste datee de ses valeurs quand une notification le
/// change en cours d'annee.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TauxAtmpEcrit {
    pub valeur: Option<ValeurAtmp>,
    pub code_risque: Option<String>,
    #[serde(rename = "note")]
    pub provenance: Option<String>,
    pub pose_a_la_creation: Option<bool>,
    pub notification: Option<NotificationAtmp>,
    pub manque: Option<String>,
    pub reporte_de: Option<i32>,
    pub alsace_moselle: Option<bool>,
    pub etablissement_neuf: Option<bool>,
}

/// Le taux AT/MP ecrit : un nombre, ou ses valeurs datees `[{depuis, valeur}]`.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ValeurAtmp {
    Seule(socle::forme::Nombre),
    Datee(Vec<TauxAtmpDate>),
}

/// Une valeur datee du taux AT/MP : sa date d'effet.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TauxAtmpDate {
    pub depuis: String,
    pub valeur: socle::forme::Nombre,
}

impl ValeurAtmp {
    /// Les valeurs, de la plus ancienne a la plus recente : `(depuis, valeur)`, sans date pour un nombre seul.
    pub fn valeurs(&self) -> Vec<(Option<&str>, Decimal)> {
        match self {
            ValeurAtmp::Seule(n) => vec![(None, n.0)],
            ValeurAtmp::Datee(l) => {
                let mut v: Vec<(Option<&str>, Decimal)> = l.iter().map(|e| (Some(e.depuis.as_str()), e.valeur.0)).collect();
                v.sort_by(|a, b| a.0.cmp(&b.0));
                v
            }
        }
    }

    /// LE TAUX EN VIGUEUR CE JOUR-LA, ramene dans l'annee pour l'ecran : la valeur dont le `depuis` est le plus recent au
    /// plus tard ce jour ; avant la premiere, la premiere.
    pub fn au(&self, jour: &socle::date::Jour) -> Option<Decimal> {
        let v = self.valeurs();
        let iso = jour.iso();
        v.iter().rev().find(|(d, _)| d.is_none_or(|d| d <= iso.as_str())).or(v.first()).map(|(_, x)| *x)
    }

    /// LE TAUX QUE LA PAIE DE CE JOUR APPLIQUE, et le `depuis` de la valeur retenue : avant la premiere date d'effet, il
    /// n'y en a pas (il ne se devine pas).
    pub fn en_vigueur(&self, jour: &socle::date::Jour) -> socle::num::Resultat<(Decimal, Option<socle::date::Jour>)> {
        let mut retenue = None;
        for (d, x) in self.valeurs() {
            let depuis = d.map(socle::date::Jour::lire).transpose()?;
            if depuis.is_none_or(|j| j <= *jour) {
                retenue = Some((x, depuis));
            }
        }
        retenue.ok_or_else(|| format!("Le taux AT/MP de la compagnie n'a aucune valeur au {} : sa première date d'effet est postérieure.", jour.jj_mm_aaaa()))
    }
}

impl TauxAtmpEcrit {
    /// Le taux tel que la paie de `jour` le lit (`ValeurAtmp::en_vigueur`), et le `depuis` de la valeur retenue.
    pub fn au_jour(&self, jour: &socle::date::Jour) -> socle::num::Resultat<(TauxAtmp, Option<socle::date::Jour>)> {
        let (valeur, depuis) = match &self.valeur {
            Some(v) => {
                let (x, d) = v.en_vigueur(jour)?;
                (Some(x), d)
            }
            None => (None, None),
        };
        Ok((
            TauxAtmp {
                valeur,
                code_risque: self.code_risque.clone(),
                provenance: self.provenance.clone(),
                pose_a_la_creation: self.pose_a_la_creation,
                notification: self.notification.clone(),
                manque: self.manque.clone(),
                reporte_de: self.reporte_de,
                alsace_moselle: self.alsace_moselle,
                etablissement_neuf: self.etablissement_neuf,
            },
            depuis,
        ))
    }
}

/// LE SOCLE LIVRE `fichier` LU A `jour` (`lire_au`) : la phrase d'un refus est celle d'un bareme que le moteur ne sait pas
/// lire, la cle a part.
pub(crate) fn lire_le_socle(fichier: &std::path::Path, jour: &socle::date::Jour, organisation: Option<&str>) -> socle::num::Resultat<(Pile, Vec<socle::date::Jour>)> {
    lire_au(fichier, jour, organisation, "Le barème de l'année porte une clé ou une valeur que le moteur ne sait pas lire : le signaler au développeur.")
}

/// LA SURCOUCHE DE LA COMPAGNIE `fichier` : la phrase d'un refus dit ce que la compagnie corrige, la cle a part.
pub(crate) fn lire_la_surcouche(fichier: &std::path::Path) -> socle::num::Resultat<Surcouche> {
    socle::forme::lire_fichier(fichier, "Le barème propre à la compagnie (config/baremes) porte une clé ou une valeur que le moteur ne sait pas lire : la corriger, ou le signaler au développeur.")
}

impl Pile {
    /// La rubrique `id`, si la pile la porte.
    pub fn rubrique(&self, id: &str) -> Option<&Rubrique> {
        self.rubriques_bulletin.iter().find(|r| r.id == id)
    }
}
