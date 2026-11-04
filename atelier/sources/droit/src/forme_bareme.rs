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
use serde_json::Value;
use socle::num::Decimal;

/// Les notes d'une valeur, communes a chaque type : elles ne se lisent pas au calcul.
macro_rules! forme {
    ($(#[$m:meta])* pub struct $nom:ident { $($corps:tt)* }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Default, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $nom {
            $($corps)*
            #[serde(default)]
            pub source: Option<Fondement>,
            #[serde(default)]
            pub note: Option<String>,
        }
    };
}

/// Une valeur datee qui reste une liste dans la pile (`smic_horaire`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Datee {
    pub depuis: Option<String>,
    #[serde(deserialize_with = "socle::forme::decimal")]
    pub valeur: Decimal,
}

forme! {
    /// LA PILE APPLIQUEE.
    pub struct Pile {
        pub annee: i32,
        #[serde(default)]
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
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub abattement_csg: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub majoration_conges_115: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub majoration_conges_110: Decimal,
        /// Le plafond journalier (T1 d'un jour travaille) des intermittents, et les multiples de la T2 et de l'Apec.
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub plafond_journalier: Option<Decimal>,
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub t2_multiple: Option<Decimal>,
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub apec_multiple: Option<Decimal>,
        /// Les textes de ces plafonds.
        #[serde(default)]
        pub plafonds_des_intermittents: Option<Notes>,
        pub rubriques_bulletin: Vec<Rubrique>,
        #[serde(default)]
        pub cotisations_dsn: Vec<CotisationDsn>,
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub taux_chomage_aem: Option<Decimal>,
        #[serde(default)]
        pub cotisation_prevoyance_retraite_supp: Option<CotisationSupplementaire>,
        #[serde(default)]
        pub cotisation_sante_patronale: Option<CotisationSupplementaire>,
        #[serde(default)]
        pub reduction_generale: Option<ReductionGenerale>,
        #[serde(default)]
        pub majoration_chomage_cddu: Option<MajorationChomage>,
        #[serde(default)]
        pub statuts: Vec<String>,
        #[serde(default)]
        pub par_statut: BTreeMap<String, CoucheStatut>,
        #[serde(default)]
        pub deduction_forfaitaire: Option<DeductionForfaitaire>,
        #[serde(default)]
        pub par_categorie: BTreeMap<String, CoucheCategorie>,
        #[serde(default)]
        pub par_nature: BTreeMap<String, CoucheNature>,
        #[serde(default)]
        pub par_caisse_conges: Option<CaisseConges>,
        #[serde(default)]
        pub risques_atmp: Option<RisquesAtmp>,
        #[serde(default)]
        pub seuils_effectif: Option<SeuilsEffectif>,
        #[serde(default)]
        pub regime_local_alsace_moselle: Option<RegimeLocal>,
        /// Le versement mobilite de la compagnie ou de l'etablissement (sa fiche) : `bareme::versement_mobilite` le lit.
        #[serde(default)]
        pub versement_mobilite: Option<Value>,
        #[serde(default)]
        pub par_organisation_patronale: BTreeMap<String, Organisation>,
        #[serde(default)]
        pub retraite_t1_repartition: Option<RetraiteT1Repartition>,
        #[serde(default)]
        pub par_convention: BTreeMap<String, Convention>,
        // La couche de la convention de la compagnie, fondue dans le socle (`contexte::convention`).
        #[serde(default)]
        pub contributions_conventionnelles: Vec<Contribution>,
        #[serde(default)]
        pub tarifs_conventionnels: Option<Tarifs>,
        #[serde(default)]
        pub contributions_dsn: Vec<ContributionDsn>,
        #[serde(default)]
        pub secteurs: Option<Secteurs>,
        #[serde(default)]
        pub article_45: Option<Article45>,
        // La surcouche de la compagnie (`config/baremes/<an>.json`) : le socle qu'elle reprend, son taux AT/MP.
        #[serde(default)]
        pub socle: Option<Value>,
        #[serde(default)]
        pub taux_atmp: Option<TauxAtmp>,
        // Ce que le moteur pose sur la pile : la fiche entreprise que la paie lit, ses etablissements, celui du salarie, la
        // prevoyance que la convention a fixee a un permanent, les contributions de branche du mois (DSN).
        #[serde(rename = "_entreprise", default)]
        pub entreprise: Value,
        #[serde(rename = "_etablissements", default)]
        pub etablissements: Option<Value>,
        #[serde(rename = "_etablissement", default)]
        pub etablissement: Option<String>,
        #[serde(rename = "_prevoyance_appliquee", default)]
        pub prevoyance_appliquee: Option<Value>,
        #[serde(rename = "_contributions_du_mois", default)]
        pub contributions_du_mois: Option<Value>,
    }
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
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub abattement_contrats_courts: Decimal,
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
        #[serde(default)]
        pub famille: Option<String>,
        #[serde(default)]
        pub libelle: Option<String>,
        /// La cle de son assiette dans les bases du calcul (`brut`, `brut_x115`, `t1_journaliere`…).
        #[serde(default)]
        pub base: Option<String>,
        /// Les deux parts, en pourcent ; nulle : pas de part de ce cote. La cle s'ecrit : un taux qui manque ne se devine pas.
        #[serde(deserialize_with = "socle::forme::presente")]
        pub taux_pat: Option<f64>,
        #[serde(deserialize_with = "socle::forme::presente")]
        pub taux_sal: Option<f64>,
        /// Une part patronale qui se compose de plusieurs taux, chacun arrondi au centime (`composants`, ou
        /// `composants_pat`) ; la part salariale de meme (`composants_sal`).
        #[serde(default)]
        pub composants: Vec<f64>,
        #[serde(default)]
        pub composants_pat: Vec<f64>,
        #[serde(default)]
        pub composants_sal: Vec<f64>,
        /// Le complement de taux patronal au-dela d'un seuil de SMIC.
        #[serde(default)]
        pub complement: Option<Complement>,
        /// La CET des cadres, fondue dans la ligne quand elle est due.
        #[serde(default)]
        pub cet_si_depasse: Option<Cet>,
        #[serde(default)]
        pub masquer_base_et_taux: bool,
        /// Le type de composant de la base 31 de la DSN (S21.G00.79.001) que la sante pose (`bareme::bareme_sante`).
        #[serde(default)]
        pub composant_79: Option<String>,
        /// Le facteur du taux AT/MP de la compagnie (`bareme::bareme_atmp`) : 0,7 pour un artiste (abattement de 30 %).
        #[serde(default)]
        pub facteur_atmp: Option<f64>,
        /// Ce qui remplit le taux, quand ce n'est pas le bareme (le taux AT/MP, la prevoyance d'une convention).
        #[serde(default)]
        pub rempli_par: Option<String>,
        /// La ligne de la taxe d'apprentissage, que la pile retire d'une compagnie qui n'y est pas assujettie.
        #[serde(default)]
        pub taxe_apprentissage: bool,
        /// Une rubrique ajoutee : celle apres laquelle elle se range (`bareme::empiler`).
        #[serde(default)]
        pub apres: Option<String>,
        /// Les niveaux de la complementaire sante d'un permanent de la 3090 (`bareme::bareme_sante`).
        #[serde(default)]
        pub releve_svp_sante: Option<ReleveSante>,
        #[serde(default)]
        pub a_confirmer: Option<String>,
    }
}

forme! {
    /// Le taux qui s'ajoute au taux patronal quand la remuneration depasse `seuil_smic` fois le SMIC (sans seuil : toujours).
    pub struct Complement {
        pub taux: f64,
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub seuil_smic: Option<Decimal>,
    }
}

forme! {
    /// Les taux patronal et salarial de la CET.
    pub struct Cet {
        pub pat: f64,
        pub sal: f64,
    }
}

forme! {
    /// Les niveaux de la complementaire sante releves (fiche de parametrage Audiens), et la part de l'employeur.
    pub struct ReleveSante {
        pub niveaux: BTreeMap<String, NiveauSante>,
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub part_employeur: Option<Decimal>,
        #[serde(default)]
        pub composant_79: Option<Composant79>,
    }
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
        #[serde(default)]
        pub famille: Option<String>,
        #[serde(default)]
        pub libelle: Option<String>,
        #[serde(default)]
        pub base: Option<String>,
        #[serde(default)]
        pub taux_pat: Option<f64>,
        #[serde(default)]
        pub taux_sal: Option<f64>,
        #[serde(default)]
        pub composants: Option<Vec<f64>>,
        #[serde(default)]
        pub composants_pat: Option<Vec<f64>>,
        #[serde(default)]
        pub composants_sal: Option<Vec<f64>>,
        #[serde(default)]
        pub cet_si_depasse: Option<Cet>,
        #[serde(default)]
        pub rempli_par: Option<String>,
        #[serde(default)]
        pub a_confirmer: Option<String>,
    }
}

forme! {
    /// Une cotisation individuelle de la DSN (bloc 81), et la base qu'elle declare au bloc 78.
    pub struct CotisationDsn {
        pub bloc78: String,
        #[serde(default)]
        pub code81: Option<String>,
        #[serde(default)]
        pub org: Option<String>,
        #[serde(default)]
        pub base: Option<String>,
        #[serde(default)]
        pub taux: Option<f64>,
        pub base78: String,
        /// La base de la ligne s'arrete au plafond de la periode.
        #[serde(default)]
        pub plafond: bool,
        #[serde(default)]
        pub composants: Vec<f64>,
        #[serde(default)]
        pub affectation: Option<String>,
        #[serde(default)]
        pub organisme: Option<String>,
        /// La cotisation n'est due qu'au-dela de ce multiple du SMIC.
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub seuil_smic: Option<Decimal>,
        #[serde(default)]
        pub cle_total: Option<String>,
        /// La commune du versement mobilite (`bareme::bareme_vm`).
        #[serde(default)]
        pub code_insee: Option<String>,
        /// Les lignes du bulletin dont la cotisation est la somme.
        #[serde(default)]
        pub lignes: Vec<String>,
        /// Une cotisation ajoutee : celle apres laquelle elle se range.
        #[serde(default)]
        pub apres: Option<String>,
    }
}

forme! {
    /// Une modification d'une cotisation du corps par une couche (`cotisations_dsn` d'une categorie, par code81 ou
    /// `code81:affectation`).
    pub struct CotisationModifiee {
        #[serde(default)]
        pub taux: Option<f64>,
        #[serde(default)]
        pub composants: Option<Vec<f64>>,
        #[serde(default)]
        pub base: Option<String>,
        #[serde(default)]
        pub lignes: Option<Vec<String>>,
    }
}

forme! {
    /// Une cotisation hors des lignes 81 (S21.G00.54, types 92 et 93) : un taux sur une base, ou la somme de lignes.
    pub struct CotisationSupplementaire {
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub taux_sur_brut: Option<Decimal>,
        #[serde(default)]
        pub base: Option<String>,
        #[serde(default)]
        pub lignes_patronales: Vec<String>,
    }
}

forme! {
    /// La reduction generale des cotisations (CSS L. 241-13), ou ce qu'une couche en complete.
    pub struct ReductionGenerale {
        #[serde(default)]
        pub verifie: Option<bool>,
        #[serde(default)]
        pub statuts: Vec<String>,
        #[serde(default)]
        pub formule: Option<String>,
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub tmin: Option<Decimal>,
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub tdelta: Option<Decimal>,
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub puissance: Option<Decimal>,
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub t: Option<Decimal>,
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub plafond_smic: Option<Decimal>,
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub smic_horaire_reference: Option<Decimal>,
        #[serde(default)]
        pub imputation: Option<Imputation>,
        #[serde(default)]
        pub dsn: Option<ReductionDsn>,
        /// La fraction du coefficient d'un salarie declare a la caisse des Conges Spectacles (`100/90`).
        #[serde(default)]
        pub majoration_caisse_conges: Option<String>,
    }
}

forme! {
    pub struct Imputation {
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub chomage: Option<Decimal>,
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub agirc_arrco: Option<Decimal>,
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
        #[serde(default)]
        pub verifie: bool,
        #[serde(default)]
        pub rubriques_bulletin: Vec<Rubrique>,
        #[serde(default)]
        pub cotisations_dsn: Vec<CotisationDsn>,
        #[serde(default)]
        pub reduction_generale: Option<ReductionGenerale>,
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub taux_chomage_aem: Option<Decimal>,
    }
}

forme! {
    /// La section d'une categorie (`par_categorie` : cadre, cadre permanent), et ses sections par statut.
    pub struct CoucheCategorie {
        #[serde(default)]
        pub verifie: bool,
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub plafond_journalier: Option<Decimal>,
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub t2_multiple: Option<Decimal>,
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub apec_multiple: Option<Decimal>,
        #[serde(default)]
        pub rubriques: BTreeMap<String, RubriqueModifiee>,
        #[serde(default)]
        pub rubriques_ajoutees: Vec<Rubrique>,
        #[serde(default)]
        pub cotisation_prevoyance_retraite_supp: Option<CotisationSupplementaire>,
        #[serde(default)]
        pub cotisations_ajoutees: Vec<CotisationDsn>,
        #[serde(default)]
        pub cotisations_dsn: BTreeMap<String, CotisationModifiee>,
        #[serde(default)]
        pub par_statut: BTreeMap<String, CoucheCategorie>,
    }
}

forme! {
    /// La section d'une nature de contrat (`par_nature` : permanent) : remplacer, retirer, ajouter des rubriques.
    pub struct CoucheNature {
        #[serde(default)]
        pub verifie: bool,
        #[serde(default)]
        pub rubriques: BTreeMap<String, RubriqueModifiee>,
        #[serde(default)]
        pub rubriques_retirees: Vec<String>,
        #[serde(default)]
        pub rubriques_ajoutees: Vec<Rubrique>,
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub majoration_conges_115: Option<Decimal>,
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub majoration_conges_110: Option<Decimal>,
        #[serde(default)]
        pub cotisation_prevoyance_retraite_supp: Option<CotisationSupplementaire>,
        #[serde(default)]
        pub cotisation_sante_patronale: Option<CotisationSupplementaire>,
        #[serde(default)]
        pub cpf_cdd: Option<CpfCdd>,
        #[serde(default)]
        pub reduction_generale: Option<ReductionGenerale>,
        #[serde(default)]
        pub cotisations_dsn: Vec<CotisationDsn>,
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
        #[serde(default)]
        pub verifie: bool,
        #[serde(default)]
        pub du_corps: Option<DuCorps>,
        #[serde(default)]
        pub rubriques_retirees: Vec<String>,
        #[serde(default)]
        pub cotisations_dsn_retirees: Vec<String>,
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub majoration_conges_115: Option<Decimal>,
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub majoration_conges_110: Option<Decimal>,
    }
}

forme! {
    pub struct DuCorps {
        #[serde(default)]
        pub rubriques_bulletin: Vec<String>,
        #[serde(default)]
        pub cotisations_dsn: Vec<String>,
    }
}

forme! {
    pub struct DeductionForfaitaire {
        #[serde(default)]
        pub verifie: bool,
        pub professions: BTreeMap<String, Profession>,
        #[serde(default)]
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
        #[serde(default)]
        pub frais_hors_assiette: Vec<String>,
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub taux: Option<Decimal>,
    }
}

forme! {
    pub struct RisquesAtmp {
        pub arrete: String,
        #[serde(default)]
        pub par_defaut: Option<String>,
        pub codes: Vec<RisqueAtmp>,
    }
}

forme! {
    pub struct RisqueAtmp {
        pub code: String,
        #[serde(default)]
        pub code_arrete: Option<String>,
        #[serde(default)]
        pub taux: Option<f64>,
        #[serde(default)]
        pub tc: bool,
        #[serde(default)]
        pub alsace_moselle: Option<AtmpAlsaceMoselle>,
        #[serde(default)]
        pub libelle: Option<String>,
        #[serde(default)]
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
        #[serde(default)]
        pub valeurs: Vec<SeuilEffectif>,
        #[serde(default)]
        pub non_portees: Vec<SeuilNonPorte>,
    }
}

forme! {
    pub struct SeuilEffectif {
        pub cle: String,
        pub seuil: i64,
        pub quoi: String,
        pub au_dela: String,
        #[serde(default)]
        pub sous_le_seuil: Option<String>,
        /// Les textes qui fondent le seuil, en clair.
        pub texte: String,
        /// Chaque valeur du bareme (son chemin) que le seuil change, et sa valeur sous le seuil.
        pub chemins: BTreeMap<String, Value>,
    }
}

forme! {
    pub struct SeuilNonPorte {
        pub cle: String,
        #[serde(default)]
        pub cle_bareme: Option<String>,
        #[serde(default)]
        pub porte_si_taux: bool,
        pub seuil: i64,
        pub quoi: String,
        #[serde(default)]
        pub texte: Option<String>,
        #[serde(default)]
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
        #[serde(default)]
        pub retraite_t1: Option<RetraiteT1ParStatut>,
        #[serde(default)]
        pub adhesion_avant: Option<String>,
        #[serde(default)]
        pub alias: Vec<String>,
        #[serde(default)]
        pub a_confirmer: Option<String>,
    }
}

forme! {
    pub struct RetraiteT1ParStatut {
        #[serde(default)]
        pub artiste: Option<f64>,
        #[serde(default)]
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
        #[serde(default)]
        pub permanent: Option<String>,
        #[serde(default)]
        pub technicien: Option<String>,
        #[serde(default)]
        pub artiste: Option<String>,
    }
}

forme! {
    /// La couche d'une convention collective (`par_convention`) : son bareme, fondu dans le socle pour la compagnie qui
    /// la nomme, et ce qu'elle fait payer hors du bulletin.
    pub struct Convention {
        pub libelle: String,
        pub applicable: bool,
        #[serde(default)]
        pub bareme: Option<CoucheConvention>,
        #[serde(default)]
        pub hors_bulletin: Option<HorsBulletin>,
        #[serde(default)]
        pub a_confirmer: Option<String>,
    }
}

forme! {
    pub struct CoucheConvention {
        #[serde(default)]
        pub tarifs_conventionnels: Option<Tarifs>,
        #[serde(default)]
        pub contributions_conventionnelles: Vec<Contribution>,
        #[serde(default)]
        pub par_nature: BTreeMap<String, CoucheNature>,
        #[serde(default)]
        pub par_categorie: BTreeMap<String, CoucheCategorie>,
        #[serde(default)]
        pub contributions_dsn: Vec<ContributionDsn>,
        #[serde(default)]
        pub secteurs: Option<Secteurs>,
        #[serde(default)]
        pub rubriques_bulletin: Vec<Rubrique>,
        #[serde(default)]
        pub article_45: Option<Article45>,
    }
}

forme! {
    /// Les seules notes d'un groupe de valeurs (les textes de valeurs qui ne portent pas de note a elles).
    pub struct Notes {}
}

forme! {
    /// L'option de la prevoyance des techniciens non cadres de la 3097 (`bareme::bareme_article_45`).
    pub struct Article45 {
        pub qui: String,
        pub prevoyance: RubriqueModifiee,
        pub cotisation_dsn: CotisationModifieeAffectee,
    }
}

forme! {
    pub struct CotisationModifieeAffectee {
        pub affectation: String,
        pub taux: f64,
        #[serde(default)]
        pub composants: Vec<f64>,
    }
}

forme! {
    /// Une contribution conventionnelle, rubrique du bulletin que la convention ajoute ou remplace
    /// (`bareme::bareme_convention`).
    pub struct Contribution {
        pub id: String,
        #[serde(default)]
        pub famille: Option<String>,
        #[serde(default)]
        pub libelle: Option<String>,
        #[serde(default)]
        pub base: Option<BaseParPopulation>,
        #[serde(default)]
        pub taux_pat: Option<f64>,
        #[serde(default)]
        pub taux_sal: Option<f64>,
        #[serde(default)]
        pub masquer_base_et_taux: bool,
        #[serde(default)]
        pub par_representation: BTreeMap<String, TauxParPopulation>,
        #[serde(default)]
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
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub repas: Option<Decimal>,
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub nuitee: Option<Decimal>,
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
        pub champ: String,
        pub libelle: String,
        pub portes: Vec<String>,
        pub refus: String,
    }
}

forme! {
    /// Ce que la convention fait payer hors du bulletin (les etats annuels).
    pub struct HorsBulletin {
        #[serde(default)]
        pub contribution_conventionnelle_permanents: Option<ParEffectif>,
        #[serde(default)]
        pub financement_paritarisme: Option<Paritarisme>,
        #[serde(default)]
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
        pub taux: f64,
    }
}

forme! {
    pub struct Paritarisme {
        pub taux: f64,
        #[serde(default)]
        pub plancher: Option<f64>,
        #[serde(default)]
        pub plafond: Option<f64>,
        #[serde(default)]
        pub assiette: Option<String>,
        #[serde(default)]
        pub collecteur: Option<String>,
        #[serde(default)]
        pub avertissement: Option<String>,
    }
}

/// LE TAUX AT/MP DE LA COMPAGNIE OU DE L'ETABLISSEMENT : la surcouche de l'annee (`moteur::routes::atmp`), ce que le
/// moteur en reprend d'une annee d'avant (`bareme::taux_atmp_reporte`) ou en dit quand il manque. Les cles en `_` sont
/// celles que la compagnie a ecrites.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TauxAtmp {
    #[serde(default)]
    pub valeur: Option<Value>,
    #[serde(default)]
    pub code_risque: Option<String>,
    #[serde(rename = "_source", default)]
    pub provenance: Option<String>,
    #[serde(rename = "_pose_a_la_creation", default)]
    pub pose_a_la_creation: Option<bool>,
    #[serde(rename = "_notification", default)]
    pub notification: Option<Value>,
    #[serde(rename = "_manque", default)]
    pub manque: Option<String>,
    #[serde(rename = "_reporte_de", default)]
    pub reporte_de: Option<Value>,
    #[serde(rename = "_alsace_moselle", default)]
    pub alsace_moselle: Option<bool>,
    #[serde(rename = "_etablissement_neuf", default)]
    pub etablissement_neuf: Option<bool>,
}

/// LA PILE LUE : la phrase d'un refus est celle d'un bareme que le moteur ne sait pas lire, la cle a part.
pub fn lire(pile: &Value) -> socle::num::Resultat<Pile> {
    socle::forme::lire(pile, "Le barème de l'année porte une clé ou une valeur que le moteur ne sait pas lire : le signaler au développeur.")
}

impl Pile {
    /// La rubrique `id`, si la pile la porte.
    pub fn rubrique(&self, id: &str) -> Option<&Rubrique> {
        self.rubriques_bulletin.iter().find(|r| r.id == id)
    }
}
