//! LA LIGNE DE PAIE : une rubrique, et les valeurs que sa forme porte. Une variante par forme, declaree ici, l'etiquette
//! `forme` ecrite par le moteur :
//! - celles qu'une personne (ou un LLM) saisit, rangees a la porte (`LigneDonnee::ranger`) selon ce que le catalogue
//!   demande de leur rubrique (`saisie`, et la nature `frais`) : `unites`, `unites_et_heures`, `frais`, `pourcentage`,
//!   `montant`, `avantage_en_nature` ;
//! - celles que le moteur compose (`composee` : un contrat paye au mois, `conges::lignes_du_mois`, `heures_en_plus`, le
//!   solde, la part des frais au-dela de leur limite), et celles d'un arret (`retenue_arret`, `imprevus`), de ses
//!   indemnites journalieres et de son maintien (`indemnite_journaliere`, `maintien`, `maintien.rs`) ;
//! - celle qu'un bulletin importe imprime (`importee`, `importe::LigneImprimee`).
//!
//! Ses nombres sont des decimaux exacts, ecrits en texte ; une case pas encore remplie est `null`. Une fois rangee, la
//! ligne se relit strictement, sans retourner au catalogue.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use socle::forme::NombreLu;

use socle::date::Jour;
use socle::mois::Mois;
use socle::num::{Decimal, Resultat};

/// UN NOMBRE TEL QU'IL EST SAISI : un nombre, ou un nombre ecrit en texte. La page envoie `""` pour une case vide ; la
/// forme canonique ecrit ses nombres en texte (`ContratEcrit`) ; un flottant garde son ecriture (`130.0`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Chiffre {
    Nombre(NombreLu),
    Texte(String),
}

impl Chiffre {
    /// Un flottant, en nombre.
    pub fn de_f64(x: f64) -> Chiffre {
        match NombreLu::de_f64(x) {
            Some(n) => Chiffre::Nombre(n),
            None => Chiffre::Texte(socle::date::repr_flottant(x)),
        }
    }

    /// Un entier, en nombre.
    pub fn entier(n: i64) -> Chiffre {
        Chiffre::Nombre(n.into())
    }

    /// Une case vide : un texte blanc.
    pub fn vide(&self) -> bool {
        matches!(self, Chiffre::Texte(s) if s.trim().is_empty())
    }

    /// Ni zero, ni un texte vide.
    pub fn vrai(&self) -> bool {
        match self {
            Chiffre::Nombre(n) => n.en_f64() != 0.0,
            Chiffre::Texte(s) => !s.is_empty(),
        }
    }

    /// Le decimal : un entier tel quel, un flottant par sa forme la plus courte, un texte lu.
    pub fn dec(&self) -> Resultat<Decimal> {
        match self {
            Chiffre::Nombre(n) => match (n.as_i64(), n.as_u64()) {
                (Some(i), _) => Ok(Decimal::from(i)),
                (_, Some(u)) => Ok(Decimal::from(u)),
                _ => socle::num::dec_f64(n.en_f64()),
            },
            Chiffre::Texte(s) => socle::num::dec_texte(s),
        }
    }

    /// Le decimal dont un flottant garde son echelle (`130.0`, sa forme ecrite).
    pub fn dec_avec_echelle(&self) -> Resultat<Decimal> {
        match self {
            Chiffre::Nombre(n) if n.as_i64().is_none() && n.as_u64().is_none() => {
                let s = socle::date::repr_flottant(n.en_f64());
                socle::num::dec_texte(&s).map_err(|_| format!("nombre hors de portee : {}", s))
            }
            autre => autre.dec(),
        }
    }

    /// Le flottant : un nombre, ou un nombre ecrit en texte.
    pub fn flottant(&self) -> Resultat<f64> {
        match self {
            Chiffre::Nombre(n) => n.as_f64().ok_or_else(|| format!("nombre illisible : {}", n)),
            Chiffre::Texte(s) => s.trim().parse::<f64>().map_err(|_| socle::num::illisible(s)),
        }
    }

    /// Le nombre en texte, tel qu'un message le cite : un texte tel quel, un nombre sous sa forme ecrite.
    pub fn en_texte(&self) -> String {
        match self {
            Chiffre::Texte(s) => s.clone(),
            Chiffre::Nombre(n) => match (n.as_i64(), n.as_u64()) {
                (Some(i), _) => i.to_string(),
                (_, Some(u)) => u.to_string(),
                _ => socle::date::repr_flottant(n.en_f64()),
            },
        }
    }

    /// Sa forme canonique : un nombre en texte normalise (`socle::num::nombre`), un texte tel quel.
    pub fn canon(&self) -> Resultat<Chiffre> {
        Ok(match self {
            Chiffre::Nombre(_) => Chiffre::Texte(socle::num::nombre(self.dec()?)),
            Chiffre::Texte(s) => Chiffre::Texte(s.clone()),
        })
    }
}

/// Une case dite et non vide (`Chiffre::vide`).
pub fn dite(c: &Option<Chiffre>) -> Option<&Chiffre> {
    c.as_ref().filter(|x| !x.vide())
}

/// UNE DATE D'UNE LIGNE : le jour nu (`"2026-10-02"`), ou detaille quand il dit ou il se joue
/// (`{"jour": "2026-10-02", "lieu": "Berlin"}`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DateDeLigne {
    Nue(String),
    Detaillee(JourEtLieu),
}

/// Le jour d'une date detaillee, et son lieu : une date qui ne dit pas de lieu est nue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JourEtLieu {
    pub jour: String,
    pub lieu: String,
}

impl DateDeLigne {
    /// Le jour, en texte.
    pub fn jour(&self) -> &str {
        match self {
            DateDeLigne::Nue(j) => j,
            DateDeLigne::Detaillee(d) => &d.jour,
        }
    }

    /// Le jour lu.
    pub fn lu(&self) -> Resultat<Jour> {
        Jour::lire(self.jour())
    }

    /// Le lieu que la date dit d'elle-meme, tel qu'il est ecrit ; `None` s'il est vide.
    pub fn lieu(&self) -> Option<String> {
        match self {
            DateDeLigne::Detaillee(JourEtLieu { lieu, .. }) if !lieu.is_empty() => Some(lieu.clone()),
            _ => None,
        }
    }

    /// Le jour `jour`, au lieu `lieu` s'il est dit.
    pub fn au(jour: String, lieu: String) -> DateDeLigne {
        if lieu.is_empty() {
            DateDeLigne::Nue(jour)
        } else {
            DateDeLigne::Detaillee(JourEtLieu { jour, lieu })
        }
    }
}

/// UNE CASE D'UNE LIGNE DE FRAIS, cochee : la compagnie garde le justificatif de la depense ; la nuit est a Paris ou en
/// petite couronne ; le salarie est contraint de prendre son repas au restaurant ; la compagnie a paye l'hotel d'un repas
/// a l'etranger (`crate::frais_au_dela`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseDeFrais {
    Justificatif,
    Paris,
    Restaurant,
    Loge,
}

impl CaseDeFrais {
    /// Sa cle, telle que la ligne et l'ecran l'ecrivent.
    pub fn cle(self) -> &'static str {
        match self {
            CaseDeFrais::Justificatif => "justificatif",
            CaseDeFrais::Paris => "paris",
            CaseDeFrais::Restaurant => "restaurant",
            CaseDeFrais::Loge => "loge",
        }
    }
}

/// Un nombre d'une ligne saisie : `null` tant que la case n'est pas remplie.
type Case = Option<Decimal>;

/// UNE LIGNE EN UNITES : quantite x taux (`saisie` : `quantite`, `taux`), ses jours, ou elle se joue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Unites {
    pub rubrique: String,
    #[serde(deserialize_with = "socle::forme::decimal_opt")]
    pub quantite: Case,
    #[serde(deserialize_with = "socle::forme::decimal_opt")]
    pub taux: Case,
    #[serde(deserialize_with = "socle::forme::presente")]
    pub lieu: Option<String>,
    /// Ses jours du mois.
    pub dates: Vec<DateDeLigne>,
    /// Ses jours des mois suivants.
    pub planning: Vec<DateDeLigne>,
}

/// UNE LIGNE EN UNITES QUI DIT SES HEURES par unite (`saisie` : `quantite`, `taux`, `heures` ; `Rubrique::heures`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitesEtHeures {
    pub rubrique: String,
    #[serde(deserialize_with = "socle::forme::decimal_opt")]
    pub quantite: Case,
    #[serde(deserialize_with = "socle::forme::decimal_opt")]
    pub taux: Case,
    #[serde(deserialize_with = "socle::forme::decimal_opt")]
    pub heures: Case,
    #[serde(deserialize_with = "socle::forme::presente")]
    pub lieu: Option<String>,
    pub dates: Vec<DateDeLigne>,
    pub planning: Vec<DateDeLigne>,
}

/// UNE LIGNE DE FRAIS AU FORFAIT (nature `frais`, `saisie` : `quantite`, `taux`) : son lieu hors de la metropole
/// (`crate::TERRITOIRES`, ou un code de `droit::missions` ; `null` : la metropole) et ses cases cochees.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frais {
    pub rubrique: String,
    #[serde(deserialize_with = "socle::forme::decimal_opt")]
    pub quantite: Case,
    #[serde(deserialize_with = "socle::forme::decimal_opt")]
    pub taux: Case,
    #[serde(deserialize_with = "socle::forme::presente")]
    pub territoire: Option<String>,
    pub cases: Vec<CaseDeFrais>,
    #[serde(deserialize_with = "socle::forme::presente")]
    pub lieu: Option<String>,
    pub dates: Vec<DateDeLigne>,
    pub planning: Vec<DateDeLigne>,
    /// Ce que le moteur pose sur elle pour l'ecran ; rien ne s'ecrit.
    #[serde(skip)]
    pub marques: Marques,
}

impl Frais {
    /// La case `case` est-elle cochee ?
    pub fn cochee(&self, case: CaseDeFrais) -> bool {
        self.cases.contains(&case)
    }
}

/// UNE LIGNE EN POURCENTAGE (`saisie` : `base`, `taux_majoration`) : sa base, en euros, et son pourcentage ; elle se
/// paie en quantite (la base) et taux (le pourcentage sur cent).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pourcentage {
    pub rubrique: String,
    #[serde(deserialize_with = "socle::forme::decimal_opt")]
    pub base: Case,
    #[serde(deserialize_with = "socle::forme::decimal_opt")]
    pub taux_majoration: Case,
}

/// UNE LIGNE AU MONTANT (`saisie` : `montant` : une prime, un acompte, une indemnite, un defraiement au reel) : une fois
/// son montant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Montant {
    pub rubrique: String,
    #[serde(deserialize_with = "socle::forme::decimal_opt")]
    pub montant: Case,
}

/// UN AVANTAGE EN NATURE (`saisie` : `montant`, `genre`) : son montant, et son genre (S21.G00.54.001,
/// `droit::catalogue::code_54` ; `null` : celui de la rubrique).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AvantageEnNature {
    pub rubrique: String,
    #[serde(deserialize_with = "socle::forme::decimal_opt")]
    pub montant: Case,
    #[serde(deserialize_with = "socle::forme::presente")]
    pub genre: Option<String>,
}

/// UNE LIGNE QUE LE MOTEUR COMPOSE : quantite x taux, ses heures par unite quand elle en dit, ce qu'elle dit d'elle-meme
/// sur le bulletin (`calcul::LigneBrut::precision`), ses jours.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Composee {
    pub rubrique: String,
    #[serde(deserialize_with = "socle::forme::decimal")]
    pub quantite: Decimal,
    #[serde(deserialize_with = "socle::forme::decimal")]
    pub taux: Decimal,
    #[serde(deserialize_with = "socle::forme::decimal_opt")]
    pub heures: Option<Decimal>,
    #[serde(deserialize_with = "socle::forme::presente")]
    pub precision: Option<String>,
    pub dates: Vec<DateDeLigne>,
}

/// LA RETENUE D'UN ARRET DE TRAVAIL (`imprevus::lignes_du_mois`) : ses heures d'absence au taux horaire ; elle tombe du
/// calcul du mois sans l'absence (`maintien::au_net`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetenueArret {
    pub rubrique: String,
    #[serde(deserialize_with = "socle::forme::decimal")]
    pub quantite: Decimal,
    #[serde(deserialize_with = "socle::forme::decimal")]
    pub taux: Decimal,
}

/// UNE INDEMNITE JOURNALIERE SUBROGEE (`maintien::lignes`) : ses jours a son taux, aucune heure (S21.G00.53 : les heures
/// de l'absence restent celles de sa retenue) ; la nette dit l'assiette du prelevement a la source qu'elle reverse
/// (`maintien::IJ_ASSIETTE_PAS`). Elle tombe d'un calcul sans indemnite (`maintien::au_net`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndemniteJournaliere {
    pub rubrique: String,
    #[serde(deserialize_with = "socle::forme::decimal")]
    pub jours: Decimal,
    #[serde(deserialize_with = "socle::forme::decimal")]
    pub taux: Decimal,
    #[serde(deserialize_with = "socle::forme::decimal_opt")]
    pub assiette_pas: Option<Decimal>,
}

/// LE MAINTIEN DU SALAIRE (`maintien::lignes`) : une somme, pas des heures de travail. Garanti en net, il porte ce qu'il
/// faut pour le reconstituer (`maintien::au_net`), qui ne s'ecrit pas : le calcul le remplace par son montant
/// (`maintien::avec_le_montant`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Maintien {
    pub rubrique: String,
    #[serde(deserialize_with = "socle::forme::decimal")]
    pub quantite: Decimal,
    #[serde(deserialize_with = "socle::forme::decimal")]
    pub taux: Decimal,
    #[serde(skip)]
    pub reconstitution: Option<Box<crate::maintien::Reconstitution>>,
}

/// LA LIGNE QU'UN BULLETIN IMPORTE IMPRIME (`importe::LigneImprimee`), rattachee par l'import a une rubrique : sa
/// quantite, son taux et ses heures, tels qu'il les lit. Une ligne imprimee qu'aucune rubrique ne rattache garde son
/// libelle et son montant dans la paie importee (`importe::remuneration_lue`) : elle n'est pas une ligne de paie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Importee {
    pub rubrique: String,
    #[serde(deserialize_with = "socle::forme::decimal_opt")]
    pub quantite: Option<Decimal>,
    #[serde(deserialize_with = "socle::forme::decimal_opt")]
    pub taux: Option<Decimal>,
    #[serde(deserialize_with = "socle::forme::decimal_opt")]
    pub heures: Option<Decimal>,
}

/// UNE LIGNE DE PAIE, sa forme dite (`forme`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "forme", rename_all = "snake_case", deny_unknown_fields)]
pub enum LignePaie {
    Unites(Unites),
    UnitesEtHeures(UnitesEtHeures),
    Frais(Frais),
    Pourcentage(Pourcentage),
    Montant(Montant),
    AvantageEnNature(AvantageEnNature),
    Composee(Composee),
    RetenueArret(RetenueArret),
    IndemniteJournaliere(IndemniteJournaliere),
    Maintien(Maintien),
    Importee(Importee),
}

/// CE QUE LE MOTEUR POSE SUR UNE LIGNE DE FRAIS CALCULEE, pour l'ecran (`frais_au_dela`) : son rang parmi les lignes de
/// la carte, les cases qui ont lieu d'etre et leur aide, et le taux saisi avant que sa part au-dela de la limite s'en
/// detache.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Marques {
    pub rang: Option<usize>,
    pub cases: Vec<CaseDeFrais>,
    pub aides: BTreeMap<CaseDeFrais, String>,
    pub taux_saisi: Option<Decimal>,
}

/// La forme canonique d'un decimal : sa forme ecrite normalisee (`socle::num::nombre`).
fn canon(x: Decimal) -> Resultat<Decimal> {
    socle::num::dec_texte(&socle::num::nombre(x))
}

impl LignePaie {
    /// LA LIGNE QUE LE MOTEUR COMPOSE de `rubrique`, `quantite` x `taux`, sans heures, sans precision, sans jour.
    pub fn composee(rubrique: &str, quantite: Decimal, taux: Decimal) -> LignePaie {
        LignePaie::Composee(Composee { rubrique: rubrique.to_string(), quantite, taux, heures: None, precision: None, dates: Vec::new() })
    }

    /// Le code de sa rubrique au catalogue (`droit::catalogue::rubrique`).
    pub fn rubrique(&self) -> &str {
        match self {
            LignePaie::Unites(l) => &l.rubrique,
            LignePaie::UnitesEtHeures(l) => &l.rubrique,
            LignePaie::Frais(l) => &l.rubrique,
            LignePaie::Pourcentage(l) => &l.rubrique,
            LignePaie::Montant(l) => &l.rubrique,
            LignePaie::AvantageEnNature(l) => &l.rubrique,
            LignePaie::Composee(l) => &l.rubrique,
            LignePaie::RetenueArret(l) => &l.rubrique,
            LignePaie::IndemniteJournaliere(l) => &l.rubrique,
            LignePaie::Maintien(l) => &l.rubrique,
            LignePaie::Importee(l) => &l.rubrique,
        }
    }

    /// SA QUANTITE : celle qu'elle dit, sa base en pourcentage, une fois un montant ; `None` : la case n'est pas remplie.
    pub fn quantite(&self) -> Option<Decimal> {
        match self {
            LignePaie::Unites(l) => l.quantite,
            LignePaie::UnitesEtHeures(l) => l.quantite,
            LignePaie::Frais(l) => l.quantite,
            LignePaie::Pourcentage(l) => l.base,
            LignePaie::Montant(_) | LignePaie::AvantageEnNature(_) => Some(Decimal::ONE),
            LignePaie::Composee(l) => Some(l.quantite),
            LignePaie::RetenueArret(l) => Some(l.quantite),
            LignePaie::IndemniteJournaliere(l) => Some(l.jours),
            LignePaie::Maintien(l) => Some(l.quantite),
            LignePaie::Importee(l) => l.quantite,
        }
    }

    /// SON TAUX : celui qu'elle dit, son pourcentage sur cent, son montant ; `None` : la case n'est pas remplie.
    pub fn taux(&self) -> Option<Decimal> {
        match self {
            LignePaie::Unites(l) => l.taux,
            LignePaie::UnitesEtHeures(l) => l.taux,
            LignePaie::Frais(l) => l.taux,
            LignePaie::Pourcentage(l) => l.taux_majoration.map(sur_cent),
            LignePaie::Montant(l) => l.montant,
            LignePaie::AvantageEnNature(l) => l.montant,
            LignePaie::Composee(l) => Some(l.taux),
            LignePaie::RetenueArret(l) => Some(l.taux),
            LignePaie::IndemniteJournaliere(l) => Some(l.taux),
            LignePaie::Maintien(l) => Some(l.taux),
            LignePaie::Importee(l) => l.taux,
        }
    }

    /// Le taux de la ligne devient `taux` (le tarif de la convention, la limite d'un frais) ; une ligne dont le taux n'est
    /// pas une case n'en change pas : refus.
    pub fn poser_taux(&mut self, taux: Decimal) -> Resultat<()> {
        match self {
            LignePaie::Unites(l) => l.taux = Some(taux),
            LignePaie::UnitesEtHeures(l) => l.taux = Some(taux),
            LignePaie::Frais(l) => l.taux = Some(taux),
            LignePaie::Composee(l) => l.taux = taux,
            _ => return Err(format!("la ligne {} ne prend pas de taux : le signaler au développeur", self.rubrique())),
        }
        Ok(())
    }

    /// SES HEURES PAR UNITE, quand elle en dit (`Rubrique::heures`) ; aucune pour une indemnite journaliere et le
    /// maintien, qui ne sont pas du travail.
    pub fn heures(&self) -> Option<Decimal> {
        match self {
            LignePaie::UnitesEtHeures(l) => l.heures,
            LignePaie::Composee(l) => l.heures,
            LignePaie::Importee(l) => l.heures,
            LignePaie::IndemniteJournaliere(_) | LignePaie::Maintien(_) => Some(Decimal::ZERO),
            LignePaie::Unites(_) | LignePaie::Frais(_) | LignePaie::Pourcentage(_) | LignePaie::Montant(_) | LignePaie::AvantageEnNature(_) | LignePaie::RetenueArret(_) => None,
        }
    }

    /// Ses heures dites, en flottant (`None` : rien n'est dit).
    pub fn heures_lues(&self) -> Option<f64> {
        self.heures().map(socle::num::flottant)
    }

    /// Ses jours du mois.
    pub fn dates(&self) -> &[DateDeLigne] {
        match self {
            LignePaie::Unites(l) => &l.dates,
            LignePaie::UnitesEtHeures(l) => &l.dates,
            LignePaie::Frais(l) => &l.dates,
            LignePaie::Composee(l) => &l.dates,
            _ => &[],
        }
    }

    /// Ses jours des mois suivants.
    pub fn planning(&self) -> &[DateDeLigne] {
        match self {
            LignePaie::Unites(l) => &l.planning,
            LignePaie::UnitesEtHeures(l) => &l.planning,
            LignePaie::Frais(l) => &l.planning,
            _ => &[],
        }
    }

    /// Ses jours du mois et ceux des mois suivants deviennent `dates` et `planning` ; une ligne qui n'a pas de jours n'en
    /// prend pas.
    pub fn avec_les_jours(&self, dates: Vec<DateDeLigne>, planning: Vec<DateDeLigne>) -> LignePaie {
        let mut l = self.clone();
        match &mut l {
            LignePaie::Unites(x) => (x.dates, x.planning) = (dates, planning),
            LignePaie::UnitesEtHeures(x) => (x.dates, x.planning) = (dates, planning),
            LignePaie::Frais(x) => (x.dates, x.planning) = (dates, planning),
            _ => {}
        }
        l
    }

    /// La ligne sans sa quantite (celle d'une ligne coupee a la fin d'un contrat, `crate::lignes_coupees_a_la_fin`) ; une
    /// ligne dont la quantite n'est pas une case telle quelle.
    pub fn sans_quantite(&self) -> LignePaie {
        let mut l = self.clone();
        match &mut l {
            LignePaie::Unites(x) => x.quantite = None,
            LignePaie::UnitesEtHeures(x) => x.quantite = None,
            LignePaie::Frais(x) => x.quantite = None,
            _ => {}
        }
        l
    }

    /// La ligne sans ce que le moteur pose pour l'ecran (`Marques`), telle qu'elle s'ecrit.
    pub fn sans_marques(&self) -> LignePaie {
        let mut l = self.clone();
        if let LignePaie::Frais(f) = &mut l {
            f.marques = Marques::default();
        }
        l
    }

    /// Ou se joue la ligne, quand elle le dit.
    pub fn lieu(&self) -> Option<&str> {
        match self {
            LignePaie::Unites(l) => l.lieu.as_deref(),
            LignePaie::UnitesEtHeures(l) => l.lieu.as_deref(),
            LignePaie::Frais(l) => l.lieu.as_deref(),
            _ => None,
        }
    }

    /// Ce que la ligne dit d'elle-meme sur le bulletin (`calcul::LigneBrut::precision`).
    pub fn precision(&self) -> Option<&str> {
        match self {
            LignePaie::Composee(l) => l.precision.as_deref(),
            _ => None,
        }
    }

    /// Le genre d'un avantage en nature, s'il est dit (`droit::catalogue::code_54`).
    pub fn genre(&self) -> Option<&str> {
        match self {
            LignePaie::AvantageEnNature(l) => l.genre.as_deref(),
            _ => None,
        }
    }

    /// La ligne de frais, si c'en est une.
    pub fn frais(&self) -> Option<&Frais> {
        match self {
            LignePaie::Frais(l) => Some(l),
            _ => None,
        }
    }

    /// La ligne de frais, a changer.
    pub fn frais_mut(&mut self) -> Option<&mut Frais> {
        match self {
            LignePaie::Frais(l) => Some(l),
            _ => None,
        }
    }

    /// La case `case` de la ligne de frais est-elle cochee ?
    pub fn cochee(&self, case: CaseDeFrais) -> bool {
        self.frais().is_some_and(|f| f.cases.contains(&case))
    }

    /// Son territoire hors de la metropole, s'il est dit.
    pub fn territoire_dit(&self) -> Option<&str> {
        self.frais().and_then(|f| f.territoire.as_deref())
    }

    /// Ce que le moteur a pose sur elle pour l'ecran (une ligne de frais).
    pub fn marques(&self) -> Option<&Marques> {
        self.frais().map(|f| &f.marques)
    }

    /// Une indemnite journaliere subrogee ?
    pub fn est_indemnite_journaliere(&self) -> bool {
        matches!(self, LignePaie::IndemniteJournaliere(_))
    }

    /// La retenue d'un arret de travail ?
    pub fn est_retenue_arret(&self) -> bool {
        matches!(self, LignePaie::RetenueArret(_))
    }

    /// L'assiette du prelevement a la source d'une indemnite journaliere nette reversee.
    pub fn assiette_pas(&self) -> Option<Decimal> {
        match self {
            LignePaie::IndemniteJournaliere(l) => l.assiette_pas,
            _ => None,
        }
    }

    /// Ce qu'il faut pour reconstituer un maintien garanti en net.
    pub fn reconstitution(&self) -> Option<&crate::maintien::Reconstitution> {
        match self {
            LignePaie::Maintien(l) => l.reconstitution.as_deref(),
            _ => None,
        }
    }

    /// Ses jours du mois, lus.
    pub fn jours(&self) -> Resultat<Vec<Jour>> {
        self.dates().iter().map(DateDeLigne::lu).collect()
    }

    /// Le premier jour de la ligne (ses dates, puis son planning) ; `None` : elle n'en a pas.
    pub fn premier_jour(&self) -> Resultat<Option<Jour>> {
        self.dates().first().or(self.planning().first()).map(DateDeLigne::lu).transpose()
    }

    /// Une ligne en pourcentage, sa base et son pourcentage positifs ; une autre passe.
    pub fn verifier_pourcentage(&self) -> Resultat<()> {
        let LignePaie::Pourcentage(l) = self else { return Ok(()) };
        if l.base.is_some_and(|b| b < Decimal::ZERO) || l.taux_majoration.is_some_and(|p| p < Decimal::ZERO) {
            return Err(format!("Ligne << {} >> : base et pourcentage sont positifs.", l.rubrique));
        }
        Ok(())
    }

    /// SA FORME CANONIQUE, pour les empreintes et les oracles : ses nombres en texte normalise, sans ce que le moteur pose
    /// pour l'ecran (`Marques`).
    pub fn canon(&self) -> Resultat<LignePaie> {
        let o = |x: Option<Decimal>| x.map(canon).transpose();
        Ok(match self.clone() {
            LignePaie::Unites(l) => LignePaie::Unites(Unites { quantite: o(l.quantite)?, taux: o(l.taux)?, ..l }),
            LignePaie::UnitesEtHeures(l) => LignePaie::UnitesEtHeures(UnitesEtHeures { quantite: o(l.quantite)?, taux: o(l.taux)?, heures: o(l.heures)?, ..l }),
            LignePaie::Frais(l) => LignePaie::Frais(Frais { quantite: o(l.quantite)?, taux: o(l.taux)?, marques: Marques::default(), ..l }),
            LignePaie::Pourcentage(l) => LignePaie::Pourcentage(Pourcentage { base: o(l.base)?, taux_majoration: o(l.taux_majoration)?, ..l }),
            LignePaie::Montant(l) => LignePaie::Montant(Montant { montant: o(l.montant)?, ..l }),
            LignePaie::AvantageEnNature(l) => LignePaie::AvantageEnNature(AvantageEnNature { montant: o(l.montant)?, ..l }),
            LignePaie::Composee(l) => LignePaie::Composee(Composee { quantite: canon(l.quantite)?, taux: canon(l.taux)?, heures: o(l.heures)?, ..l }),
            LignePaie::RetenueArret(l) => LignePaie::RetenueArret(RetenueArret { quantite: canon(l.quantite)?, taux: canon(l.taux)?, ..l }),
            LignePaie::IndemniteJournaliere(l) => {
                LignePaie::IndemniteJournaliere(IndemniteJournaliere { jours: canon(l.jours)?, taux: canon(l.taux)?, assiette_pas: o(l.assiette_pas)?, ..l })
            }
            LignePaie::Maintien(l) => LignePaie::Maintien(Maintien { quantite: canon(l.quantite)?, taux: canon(l.taux)?, ..l }),
            LignePaie::Importee(l) => LignePaie::Importee(Importee { quantite: o(l.quantite)?, taux: o(l.taux)?, heures: o(l.heures)?, ..l }),
        })
    }
}

/// UN POURCENTAGE SUR CENT : le quotient exact, a l'exposant ideal, qui garde au moins l'echelle du pourcentage
/// (`12.5` -> `0.125`, `30.0` -> `0.30`).
fn sur_cent(pct: Decimal) -> Decimal {
    let mut taux = socle::num::div(pct, Decimal::ONE_HUNDRED).normalize();
    if taux.scale() < pct.scale() {
        taux.rescale(pct.scale());
    }
    taux
}

// ------------------------------------------------------------------ la porte

/// LES VALEURS QU'UNE LIGNE SAISIE PEUT DIRE, chacune une cle de sa forme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cle {
    Rubrique,
    Quantite,
    Taux,
    Heures,
    Base,
    TauxMajoration,
    Montant,
    Genre,
    Territoire,
    Cases,
    Lieu,
    Dates,
    Planning,
}

impl Cle {
    /// Sa cle, telle que la ligne l'ecrit.
    pub fn nom(self) -> &'static str {
        match self {
            Cle::Rubrique => "rubrique",
            Cle::Quantite => "quantite",
            Cle::Taux => "taux",
            Cle::Heures => "heures",
            Cle::Base => "base",
            Cle::TauxMajoration => "taux_majoration",
            Cle::Montant => "montant",
            Cle::Genre => "genre",
            Cle::Territoire => "territoire",
            Cle::Cases => "cases",
            Cle::Lieu => "lieu",
            Cle::Dates => "dates",
            Cle::Planning => "planning",
        }
    }
}

/// LA FORME QUE LE CATALOGUE DEMANDE D'UNE LIGNE DE SA RUBRIQUE : ce que dit sa `saisie`, et sa nature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormeSaisie {
    Unites,
    UnitesEtHeures,
    Frais,
    Pourcentage,
    Montant,
    AvantageEnNature,
}

impl FormeSaisie {
    /// LA FORME D'UNE LIGNE DE LA RUBRIQUE `r`, telle que son catalogue la declare (`saisie` ; `jours`, celle de la
    /// semaine, ce sont les dates de la ligne) ; `None` : une rubrique que le moteur pose (`saisie` vide), qui ne se saisit
    /// pas. Une `saisie` que le moteur ne sait pas ranger est une rubrique mal decrite : refus.
    pub fn de(r: &droit::catalogue::Rubrique) -> Resultat<Option<FormeSaisie>> {
        let mut dit: Vec<&str> = r.saisie.iter().map(String::as_str).filter(|s| *s != "jours").collect();
        dit.sort_unstable();
        let frais = r.nature == "frais";
        Ok(Some(match (dit.as_slice(), frais) {
            (["quantite", "taux"], false) => FormeSaisie::Unites,
            (["quantite", "taux"], true) => FormeSaisie::Frais,
            (["heures", "quantite", "taux"], false) => FormeSaisie::UnitesEtHeures,
            (["base", "taux_majoration"], _) => FormeSaisie::Pourcentage,
            (["montant"], _) => FormeSaisie::Montant,
            (["genre", "montant"], _) => FormeSaisie::AvantageEnNature,
            ([], _) => return Ok(None),
            _ => {
                return Err(format!(
                    "La rubrique « {} » est mal décrite : sa saisie ({}) ne dit pas une forme de ligne. Le signaler au développeur.",
                    r.libelle,
                    r.saisie.join(", ")
                ))
            }
        }))
    }

    /// Les noms des cles qu'une ligne de cette forme dit (la route etat les donne a la page, `rubriques[].cles`).
    pub fn noms(self) -> Vec<&'static str> {
        self.cles().iter().map(|c| c.nom()).collect()
    }

    /// Les cles qu'une ligne de cette forme dit, toutes, dans l'ordre de la ligne.
    pub fn cles(self) -> &'static [Cle] {
        use Cle::*;
        match self {
            FormeSaisie::Unites => &[Rubrique, Quantite, Taux, Lieu, Dates, Planning],
            FormeSaisie::UnitesEtHeures => &[Rubrique, Quantite, Taux, Heures, Lieu, Dates, Planning],
            FormeSaisie::Frais => &[Rubrique, Quantite, Taux, Territoire, Cases, Lieu, Dates, Planning],
            FormeSaisie::Pourcentage => &[Rubrique, Base, TauxMajoration],
            FormeSaisie::Montant => &[Rubrique, Montant],
            FormeSaisie::AvantageEnNature => &[Rubrique, Montant, Genre],
        }
    }
}

/// UNE LIGNE TELLE QUE L'APPELANT LA DONNE (la page, un LLM : le changement de la case `lignes`, `carte::Changement`) :
/// sa rubrique et les valeurs qu'elle dit, chacune une `Cle` lue a son type (absente : `None` ; `null` : une case vide).
/// Une cle inconnue ou repetee refuse a la lecture ; les cles que la rubrique demande se verifient a la porte
/// (`LigneDonnee::ranger`), avec le catalogue.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LigneDonnee {
    pub rubrique: Option<String>,
    pub quantite: Option<Case>,
    pub taux: Option<Case>,
    pub heures: Option<Case>,
    pub base: Option<Case>,
    pub taux_majoration: Option<Case>,
    pub montant: Option<Case>,
    pub genre: Option<Option<String>>,
    pub territoire: Option<Option<String>>,
    pub cases: Option<Vec<CaseDeFrais>>,
    pub lieu: Option<Option<String>>,
    pub dates: Option<Vec<DateDeLigne>>,
    pub planning: Option<Vec<DateDeLigne>>,
}

impl<'de> Deserialize<'de> for LigneDonnee {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<LigneDonnee, D::Error> {
        struct Visiteur;
        impl<'de> serde::de::Visitor<'de> for Visiteur {
            type Value = LigneDonnee;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("une ligne de paie : {\"rubrique\": …, et les valeurs que sa rubrique demande}")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut m: A) -> Result<LigneDonnee, A::Error> {
                /// Une valeur posee une fois : une cle repetee refuse.
                fn une<T, E: serde::de::Error>(case: &mut Option<T>, v: T, cle: Cle) -> Result<(), E> {
                    if case.is_some() {
                        return Err(E::custom(format!("la clé {} est répétée", cle.nom())));
                    }
                    *case = Some(v);
                    Ok(())
                }
                /// Un nombre, ou `null`.
                struct Nombre(Case);
                impl<'de> Deserialize<'de> for Nombre {
                    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Nombre, D::Error> {
                        socle::forme::decimal_opt(d).map(Nombre)
                    }
                }
                let mut l = LigneDonnee::default();
                while let Some(cle) = m.next_key::<Cle>()? {
                    match cle {
                        Cle::Rubrique => une(&mut l.rubrique, m.next_value::<String>()?, cle)?,
                        Cle::Quantite => une(&mut l.quantite, m.next_value::<Nombre>()?.0, cle)?,
                        Cle::Taux => une(&mut l.taux, m.next_value::<Nombre>()?.0, cle)?,
                        Cle::Heures => une(&mut l.heures, m.next_value::<Nombre>()?.0, cle)?,
                        Cle::Base => une(&mut l.base, m.next_value::<Nombre>()?.0, cle)?,
                        Cle::TauxMajoration => une(&mut l.taux_majoration, m.next_value::<Nombre>()?.0, cle)?,
                        Cle::Montant => une(&mut l.montant, m.next_value::<Nombre>()?.0, cle)?,
                        Cle::Genre => une(&mut l.genre, m.next_value::<Option<String>>()?, cle)?,
                        Cle::Territoire => une(&mut l.territoire, m.next_value::<Option<String>>()?, cle)?,
                        Cle::Cases => une(&mut l.cases, m.next_value::<Vec<CaseDeFrais>>()?, cle)?,
                        Cle::Lieu => une(&mut l.lieu, m.next_value::<Option<String>>()?, cle)?,
                        Cle::Dates => une(&mut l.dates, m.next_value::<Vec<DateDeLigne>>()?, cle)?,
                        Cle::Planning => une(&mut l.planning, m.next_value::<Vec<DateDeLigne>>()?, cle)?,
                    }
                }
                Ok(l)
            }
        }
        d.deserialize_map(Visiteur)
    }
}

impl LigneDonnee {
    /// Les cles qu'elle dit, dans l'ordre de `Cle`.
    fn dites(&self) -> Vec<Cle> {
        let mut out = Vec::new();
        let mut si = |dite: bool, c: Cle| {
            if dite {
                out.push(c);
            }
        };
        si(self.rubrique.is_some(), Cle::Rubrique);
        si(self.quantite.is_some(), Cle::Quantite);
        si(self.taux.is_some(), Cle::Taux);
        si(self.heures.is_some(), Cle::Heures);
        si(self.base.is_some(), Cle::Base);
        si(self.taux_majoration.is_some(), Cle::TauxMajoration);
        si(self.montant.is_some(), Cle::Montant);
        si(self.genre.is_some(), Cle::Genre);
        si(self.territoire.is_some(), Cle::Territoire);
        si(self.cases.is_some(), Cle::Cases);
        si(self.lieu.is_some(), Cle::Lieu);
        si(self.dates.is_some(), Cle::Dates);
        si(self.planning.is_some(), Cle::Planning);
        out
    }

    /// LE JOUR OU SA RUBRIQUE SE LIT : son premier jour (ses dates, puis son planning), sinon le dernier jour de `mois`,
    /// celui de la carte.
    pub fn jour(&self, mois: Mois) -> Resultat<Jour> {
        let premier = self.dates.iter().chain(self.planning.iter()).flatten().next();
        match premier {
            Some(d) => d.lu(),
            None => Ok(mois.dernier_jour()),
        }
    }

    /// LA LIGNE RANGEE DANS SA FORME (`FormeSaisie::de` de sa rubrique `r`, lue au catalogue a son jour, `jour`) : elle
    /// dit exactement les cles que cette forme demande, sinon le refus dit lesquelles donner et lesquelles retirer.
    pub fn ranger(self, r: &droit::catalogue::Rubrique) -> Resultat<LignePaie> {
        let Some(forme) = FormeSaisie::de(r)? else {
            return Err(format!("« {} » ne se saisit pas : le moteur la pose lui-même. Retirez la ligne.", r.libelle));
        };
        let voulues = forme.cles();
        let dites = self.dites();
        let manquent: Vec<&str> = voulues.iter().filter(|c| !dites.contains(c)).map(|c| c.nom()).collect();
        let de_trop: Vec<&str> = dites.iter().filter(|c| !voulues.contains(c)).map(|c| c.nom()).collect();
        if !manquent.is_empty() || !de_trop.is_empty() {
            let liste = |l: &[&str]| l.iter().map(|c| format!("« {} »", c)).collect::<Vec<_>>().join(", ");
            let mut quoi = Vec::new();
            if !manquent.is_empty() {
                quoi.push(format!("donnez {} (null tant que la case est vide)", liste(&manquent)));
            }
            if !de_trop.is_empty() {
                quoi.push(format!("retirez {}", liste(&de_trop)));
            }
            return Err(format!(
                "la ligne « {} » dit {} : {}.",
                r.libelle,
                liste(&voulues.iter().map(|c| c.nom()).collect::<Vec<_>>()),
                quoi.join(" ; ")
            ));
        }
        // Une case vide s'ecrit `null`, jamais un texte vide.
        for (cle, t) in [(Cle::Lieu, &self.lieu), (Cle::Territoire, &self.territoire), (Cle::Genre, &self.genre)] {
            if t.as_ref().is_some_and(|t| t.as_ref().is_some_and(|t| t.trim().is_empty())) {
                return Err(format!("la ligne « {} » : « {} » est vide ; écrivez null.", r.libelle, cle.nom()));
            }
        }
        let rubrique = r.code.clone();
        // Chaque cle voulue est dite : la verification vient d'etre faite.
        let dit = |x: Option<Case>| x.flatten();
        let jours = |x: Option<Vec<DateDeLigne>>| x.into_iter().flatten().collect::<Vec<_>>();
        let texte = |x: Option<Option<String>>| x.flatten();
        Ok(match forme {
            FormeSaisie::Unites => LignePaie::Unites(Unites {
                rubrique,
                quantite: dit(self.quantite),
                taux: dit(self.taux),
                lieu: texte(self.lieu),
                dates: jours(self.dates),
                planning: jours(self.planning),
            }),
            FormeSaisie::UnitesEtHeures => LignePaie::UnitesEtHeures(UnitesEtHeures {
                rubrique,
                quantite: dit(self.quantite),
                taux: dit(self.taux),
                heures: dit(self.heures),
                lieu: texte(self.lieu),
                dates: jours(self.dates),
                planning: jours(self.planning),
            }),
            FormeSaisie::Frais => {
                let mut cases = self.cases.into_iter().flatten().collect::<Vec<_>>();
                cases.sort();
                cases.dedup();
                LignePaie::Frais(Frais {
                    rubrique,
                    quantite: dit(self.quantite),
                    taux: dit(self.taux),
                    territoire: texte(self.territoire),
                    cases,
                    lieu: texte(self.lieu),
                    dates: jours(self.dates),
                    planning: jours(self.planning),
                    marques: Marques::default(),
                })
            }
            FormeSaisie::Pourcentage => LignePaie::Pourcentage(Pourcentage { rubrique, base: dit(self.base), taux_majoration: dit(self.taux_majoration) }),
            FormeSaisie::Montant => LignePaie::Montant(Montant { rubrique, montant: dit(self.montant) }),
            FormeSaisie::AvantageEnNature => LignePaie::AvantageEnNature(AvantageEnNature { rubrique, montant: dit(self.montant), genre: texte(self.genre) }),
        })
    }
}

/// LES LIGNES QU'UN APPELANT DONNE A UNE CARTE DE `mois`, RANGEES (`LigneDonnee::ranger`) : chacune sa rubrique lue au
/// catalogue de la compagnie a son jour (`LigneDonnee::jour`). Le premier refus nomme le salarie (`qui`) et la ligne ;
/// rien ne s'ecrit.
pub fn ranger(racine: &std::path::Path, mois: Mois, lignes: Vec<LigneDonnee>, qui: &str) -> Resultat<Vec<LignePaie>> {
    let mut out = Vec::new();
    for (i, l) in lignes.into_iter().enumerate() {
        let refus = |m: String| format!("{} : sa ligne {} : {} Rien n'est écrit.", qui, i + 1, m);
        let Some(code) = l.rubrique.clone() else {
            return Err(refus("elle dit sa rubrique (« rubrique »).".to_string()));
        };
        let r = droit::catalogue::rubrique(racine, &code, &l.jour(mois).map_err(refus)?).map_err(refus)?;
        out.push(l.ranger(&r).map_err(refus)?);
    }
    Ok(out)
}
