//! LA PAIE ECRITE : ce que la validation d'un engagement ecrit de sa paie d'un mois (`etats::calculer_les_rangs`), dans le
//! fichier de ce mois (`paies`), et ce que ses envois y posent ensuite (le numero du bulletin, le premier N CE, le
//! bulletin qu'elle rectifie, pourquoi elle s'est recalculee). Une seule forme, lue avec le fichier des paies (`paies`) et ecrite
//! par serde ; une cle inconnue refuse en se nommant.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use socle::date::Jour;
use socle::num::Decimal;

use crate::{Avis, ContratEcrit, LignePaie};

/// UNE PAIE ECRITE.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaieEcrite {
    /// La paie, sous sa forme ecrite (`relire::ecrire`).
    pub paie: FormeDePaie<Avis>,
    /// Les fiches du salarie telles que le calcul les a lues.
    pub fiches: crate::relire::FichesDuCalcul,
    /// Ce que la saisie du mois posait pour tous ses engagements.
    pub saisie_du_mois: crate::saisie::CeQueLeMoisDit,
    /// Ce dont le calcul dependait (`empreinte`).
    pub empreinte: Empreinte,
    /// Le prelevement a la source (`etats::prelevement`).
    pub pas: Option<droit::pas::Versement>,
    /// Le compteur de conges d'un permanent a la fin du mois, et ce qu'il tient pour le mois suivant.
    pub conges: Option<crate::conges::CompteurConges>,
    pub conges_avant: Option<crate::conges::Avant>,
    /// Le numero du bulletin et le premier N CE, ecrits quand ses documents sont partis (`etats::poser_numeros_des_documents`).
    pub bulletin: Option<String>,
    pub ce: Option<i64>,
    /// Le numero du bulletin d'une paie partie que celle-ci rectifie.
    pub rectifie: Option<String>,
    /// Pourquoi la paie s'est recalculee d'elle-meme, et quand.
    pub recalcul: Option<Recalcul>,
}

/// CE DONT LE CALCUL D'UNE PAIE DEPENDAIT : l'empreinte de chaque chose lue, les regles datees et les valeurs datees de la
/// compagnie (`empreinte`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Empreinte {
    pub entrees: BTreeMap<String, String>,
    pub regles: Vec<String>,
    pub configuration: Vec<String>,
}

/// Une paie recalculee d'elle-meme : quand, et pourquoi (`etats::raison`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recalcul {
    pub le: String,
    pub raison: String,
}

impl PaieEcrite {
    /// Le compteur de conges sans ses avertissements, pour comparer deux paies sur leurs chiffres.
    pub fn conges_sans_avis(&self) -> Option<crate::conges::CompteurConges> {
        self.conges.clone().map(|c| crate::conges::CompteurConges { avertissements: Vec::new(), ..c })
    }
}

/// UNE PAIE SOUS SA FORME ECRITE, champ par champ, dans l'ordre ou elle s'ecrit ; les montants en texte normalise
/// (`socle::num::nombre`). C'est sa forme canonique (`calcul::Paie::canon`, pour les empreintes et les oracles), un avis
/// `A` par son seul texte, et sa forme ecrite (`relire::ecrire`), un avis `{texte, info}`, avec les lignes de
/// l'engagement telles quelles et le plafond d'un temps partiel ; `relire` la relit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(bound(deserialize = "A: Deserialize<'de>"))]
#[serde(deny_unknown_fields)]
pub struct FormeDePaie<A> {
    #[serde(with = "nombre")]
    pub assiette_chomage: Decimal,
    pub avertissements: Vec<A>,
    #[serde(with = "nombre")]
    pub base_csg: Decimal,
    #[serde(with = "nombres")]
    pub bases: BTreeMap<String, Decimal>,
    #[serde(with = "nombre")]
    pub brut: Decimal,
    #[serde(with = "nombre")]
    pub cout_employeur: Decimal,
    pub deduction_forfaitaire: Option<FormeDeLaDeduction>,
    pub engagement: FormeDEngagement,
    #[serde(with = "nombre")]
    pub heures: Decimal,
    #[serde(with = "nombre")]
    pub heures_absence: Decimal,
    #[serde(with = "nombre")]
    pub heures_travaillees: Decimal,
    pub lignes: Vec<FormeDeCotisation>,
    #[serde(with = "nombre")]
    pub net_a_payer: Decimal,
    #[serde(with = "nombre")]
    pub net_imposable: Decimal,
    #[serde(with = "nombre")]
    pub net_social: Decimal,
    pub numero_bulletin: Option<String>,
    pub reduction_generale: Option<droit::reduction::ReductionGenerale>,
    pub remuneration: Vec<FormeDeRemuneration>,
    #[serde(with = "nombre")]
    pub salaire_de_base: Decimal,
    #[serde(with = "nombre")]
    pub salaire_retabli: Decimal,
    pub salarie: Cle,
    #[serde(with = "nombre")]
    pub smic: Decimal,
    pub spectacle: Option<Cle>,
    #[serde(with = "nombre")]
    pub total_pat: Decimal,
    #[serde(with = "nombre")]
    pub total_sal: Decimal,
    #[serde(with = "nombre")]
    pub plafond: Decimal,
    #[serde(with = "nombre")]
    pub plafond_urssaf: Decimal,
    /// Un mois decoupe : ses periodes elementaires, chacune une paie. Vide pour un mois d'un seul tenant.
    pub periodes: Vec<FormeDePaie<A>>,
    /// Le plafond d'un salarie a temps partiel tel que le calcul l'a applique (`calcul::PlafondTempsPartiel::code`) :
    /// la forme ecrite seule le porte.
    pub plafond_temps_partiel: Option<String>,
}

/// Une fiche nommee par sa cle (le salarie, le spectacle d'une paie).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cle {
    pub cle: String,
}

/// Une ligne de cotisation (`calcul::LigneCotisation`) sous sa forme ecrite.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormeDeCotisation {
    pub id: String,
    pub famille: String,
    pub libelle: String,
    #[serde(with = "nombre")]
    pub base: Decimal,
    #[serde(with = "nombre_opt")]
    pub taux_pat: Option<Decimal>,
    #[serde(with = "nombre")]
    pub montant_pat: Decimal,
    #[serde(with = "nombre_opt")]
    pub taux_sal: Option<Decimal>,
    #[serde(with = "nombre")]
    pub montant_sal: Decimal,
    pub masquer_base_et_taux: bool,
}

/// Une ligne de remuneration (`calcul::LigneBrut`) sous sa forme ecrite, avec sa designation au bulletin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormeDeRemuneration {
    pub code: String,
    pub libelle: String,
    pub unite: String,
    #[serde(with = "nombre")]
    pub quantite: Decimal,
    #[serde(with = "nombre")]
    pub taux: Decimal,
    #[serde(with = "nombre")]
    pub montant: Decimal,
    #[serde(with = "nombre")]
    pub heures_annexes: Decimal,
    pub soumis: bool,
    pub imposable: bool,
    pub conges_spectacles: bool,
    pub salaire_de_base: bool,
    pub retabli: bool,
    pub conges_pris: bool,
    pub dsn_bloc52: Option<String>,
    pub arret_de_travail: bool,
    pub dsn_autres_bruts: Option<String>,
    pub precision: Option<String>,
    pub designation: String,
}

/// La deduction forfaitaire specifique (`calcul::DeductionForfaitaire`) sous sa forme ecrite.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormeDeLaDeduction {
    pub categorie: String,
    pub profession: String,
    #[serde(with = "nombre")]
    pub taux: Decimal,
    pub rubriques: Vec<String>,
    #[serde(with = "nombre")]
    pub montant: Decimal,
    #[serde(with = "nombre")]
    pub deja: Decimal,
}

/// UN ENGAGEMENT SOUS SA FORME CANONIQUE (`Engagement::canon`) : ses champs dans l'ordre ou ils s'ecrivent, les nombres
/// en texte normalise, puis ce qui s'en deduit (sa periode, sa quotite, ses mois).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormeDEngagement {
    pub salarie: droit::contexte::IdSalarie,
    pub spectacle: Option<droit::contexte::IdSpectacle>,
    pub dates_travail: Vec<Jour>,
    pub lignes: Vec<LignePaie>,
    pub debut: Jour,
    pub fin: Option<Jour>,
    pub numero: Option<crate::numeros::NumeroContrat>,
    pub mois: String,
    pub planning: Vec<Jour>,
    pub planning_lieux: BTreeMap<Jour, String>,
    #[serde(with = "entier")]
    pub jours: i64,
    #[serde(with = "entier")]
    pub jours_contrat: i64,
    pub date_paiement: Option<String>,
    #[serde(with = "nombre_opt")]
    pub taux_cachet: Option<Decimal>,
    #[serde(with = "entier")]
    pub cachets: i64,
    #[serde(with = "nombre_opt")]
    pub heures: Option<Decimal>,
    pub lieu: String,
    pub lieux: BTreeMap<Jour, String>,
    pub emploi: Option<String>,
    pub emploi_affichage: Option<String>,
    pub pcs_ese: Option<String>,
    pub code_emploi_ft: Option<String>,
    pub statut: Option<String>,
    pub statut_conventionnel: Option<String>,
    pub statut_categoriel_rc: Option<String>,
    pub etablissement: Option<String>,
    pub nature: String,
    #[serde(with = "nombre_opt")]
    pub quotite_contrat: Option<Decimal>,
    #[serde(with = "nombre_opt")]
    pub quotite_reference_saisie: Option<Decimal>,
    pub modalite_temps_travail_saisie: Option<String>,
    pub motif_recours: Option<String>,
    pub motif_rupture: Option<String>,
    pub notification_rupture: Option<String>,
    pub preavis: Option<crate::saisie::Preavis>,
    pub transaction: Option<String>,
    pub suspensions: Vec<crate::imprevus::Suspension>,
    pub contrat_ecrit: Option<ContratEcrit>,
    pub debut_periode: Jour,
    pub fin_periode: Jour,
    #[serde(with = "nombre")]
    pub quotite: Decimal,
    pub sans_paie: bool,
    pub premier_mois: bool,
    pub dernier_mois: bool,
    pub mois_du_contrat: Vec<String>,
}

/// Un montant en texte normalise (`socle::num::nombre`), relu en decimal.
pub(crate) mod nombre {
    use socle::num::Decimal;
    pub fn serialize<S: serde::Serializer>(x: &Decimal, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&socle::num::nombre(*x))
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Decimal, D::Error> {
        socle::forme::decimal(d)
    }
}

/// `nombre`, ou nul.
pub(crate) mod nombre_opt {
    use socle::num::Decimal;
    pub fn serialize<S: serde::Serializer>(x: &Option<Decimal>, s: S) -> Result<S::Ok, S::Error> {
        match x {
            Some(x) => s.serialize_str(&socle::num::nombre(*x)),
            None => s.serialize_none(),
        }
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Decimal>, D::Error> {
        socle::forme::decimal_opt(d)
    }
}

/// Des montants par cle, chacun en `nombre`.
pub(crate) mod nombres {
    use std::collections::BTreeMap;
    use serde::{Deserialize, Serialize};
    use socle::num::Decimal;
    pub fn serialize<S: serde::Serializer>(m: &BTreeMap<String, Decimal>, s: S) -> Result<S::Ok, S::Error> {
        m.iter().map(|(k, x)| (k, socle::num::nombre(*x))).collect::<BTreeMap<_, _>>().serialize(s)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<BTreeMap<String, Decimal>, D::Error> {
        Ok(BTreeMap::<String, socle::forme::Nombre>::deserialize(d)?.into_iter().map(|(k, n)| (k, n.0)).collect())
    }
}

/// Un entier en texte.
pub(crate) mod entier {
    use serde::Deserialize;
    pub fn serialize<S: serde::Serializer>(x: &i64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&x.to_string())
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
        let t = String::deserialize(d)?;
        t.trim().parse().map_err(|_| serde::de::Error::custom(format!("{} n'est pas un entier", socle::ecriture::json_en_ligne(&t))))
    }
}

/// Un decimal en texte tel qu'il est (`Decimal::to_string`, son echelle gardee), relu exactement.
pub(crate) mod decimal_texte {
    use serde::Deserialize;
    use socle::num::Decimal;
    pub fn serialize<S: serde::Serializer>(x: &Decimal, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&x.to_string())
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Decimal, D::Error> {
        let t = String::deserialize(d)?;
        t.parse().map_err(|_| serde::de::Error::custom(format!("{} n'est pas un nombre", socle::ecriture::json_en_ligne(&t))))
    }
}
