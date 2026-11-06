//! LA LIGNE DE PAIE : une rubrique, sa quantite, son taux, ses jours. Une seule forme, declaree ici : celle que la carte
//! porte (`saisie::Carte::lignes`, ce que la personne tape), celle que le moteur compose pour un contrat paye au mois
//! (`conges::lignes_du_mois`, `maintien`, `imprevus`, `heures_en_plus`) et celle que la paie ecrite garde
//! (`relire::ecrire`). Elle se lit avec la carte, par `socle::forme` ; une cle inconnue refuse en se nommant.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use socle::forme::NombreLu;

use socle::date::Jour;
use socle::num::{Decimal, Resultat};

/// UN NOMBRE TEL QU'IL EST SAISI : un nombre, ou un nombre ecrit en texte. La page envoie `""` pour une case vide ; la
/// forme canonique ecrit ses nombres en texte (`LignePaie::canon`) ; un flottant garde son ecriture (`130.0`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Chiffre {
    Nombre(NombreLu),
    Texte(String),
}

impl Chiffre {
    /// Un decimal, en nombre (sa forme la plus courte : `12.5`, `15`).
    pub fn de_decimal(x: Decimal) -> Chiffre {
        let t = x.normalize().to_string();
        match t.parse::<NombreLu>() {
            Ok(n) => Chiffre::Nombre(n),
            Err(_) => Chiffre::Texte(t),
        }
    }

    /// Un decimal, en texte (`div(...).normalize().to_string()`).
    pub fn texte_de(x: Decimal) -> Chiffre {
        Chiffre::Texte(x.to_string())
    }

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

/// Le jour d'une date detaillee, et son lieu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JourEtLieu {
    pub jour: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lieu: Option<String>,
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
            DateDeLigne::Detaillee(JourEtLieu { lieu: Some(l), .. }) if !l.is_empty() => Some(l.clone()),
            _ => None,
        }
    }

    /// Le jour `jour`, au lieu `lieu` s'il est dit.
    pub fn au(jour: String, lieu: String) -> DateDeLigne {
        if lieu.is_empty() {
            DateDeLigne::Nue(jour)
        } else {
            DateDeLigne::Detaillee(JourEtLieu { jour, lieu: Some(lieu) })
        }
    }
}

/// UNE LIGNE DE PAIE. Absent et `null` se confondent : rien n'est dit.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LignePaie {
    /// Le code de sa rubrique au catalogue (`droit::catalogue::rubrique`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rubrique: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantite: Option<Chiffre>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub taux: Option<Chiffre>,
    /// Une ligne en pourcentage (`taux_majoration` dans la `saisie` de sa rubrique) : sa base, en euros, et son
    /// pourcentage ; elle se lit en quantite et taux (`LignePaie::en_pourcentage`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<Chiffre>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub taux_majoration: Option<Chiffre>,
    /// Ses heures, par unite (`Rubrique::heures`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heures: Option<Chiffre>,
    /// Le genre d'un avantage en nature (`droit::catalogue::code_54`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    /// Le lieu d'une nuitee ou d'un repas hors de la metropole (`crate::TERRITOIRES`, ou un code de `droit::missions`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub territoire: Option<String>,
    /// Les cases d'une ligne de frais (`crate::JUSTIFICATIF`, `PARIS`, `RESTAURANT`, `LOGE`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub justificatif: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paris: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restaurant: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loge: Option<bool>,
    /// Ou se joue la ligne, quand elle le dit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lieu: Option<String>,
    /// Ses jours du mois.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dates: Vec<DateDeLigne>,
    /// Ses jours des mois suivants.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub planning: Vec<DateDeLigne>,
    /// Ce que la ligne dit d'elle-meme sur le bulletin (`calcul::LigneBrut::precision`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precision: Option<String>,
    /// Une indemnite journaliere subrogee (`maintien::lignes`) : elle tombe d'un calcul sans indemnite (`maintien::au_net`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ij: Option<bool>,
    /// L'assiette du prelevement a la source des indemnites nettes reversees (`maintien::IJ_ASSIETTE_PAS`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assiette_pas: Option<Chiffre>,
    /// La retenue d'un arret de travail (`imprevus::lignes_du_mois`) : elle tombe du calcul du mois sans l'absence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arret: Option<bool>,
    /// La ligne du maintien garanti en net, ce qu'il faut pour le reconstituer (`maintien::au_net`) ; elle ne s'ecrit
    /// pas : le calcul la remplace par son montant (`maintien::avec_le_montant`).
    #[serde(skip)]
    pub maintien: Option<Box<crate::maintien::Reconstitution>>,
    /// Ce que le moteur pose sur une ligne de frais pour l'ecran ; rien ne s'ecrit.
    #[serde(skip)]
    pub marques: Marques,
}

/// CE QUE LE MOTEUR POSE SUR UNE LIGNE DE FRAIS CALCULEE, pour l'ecran (`frais_au_dela`) : son rang parmi les lignes de
/// la carte, les cases qui ont lieu d'etre et leur aide, et le taux saisi avant que sa part au-dela de la limite s'en
/// detache.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Marques {
    pub rang: Option<usize>,
    pub cases: Vec<&'static str>,
    pub aides: BTreeMap<&'static str, String>,
    pub taux_saisi: Option<Chiffre>,
}

impl LignePaie {
    /// Une ligne de `rubrique`, `quantite` x `taux`.
    pub fn de(rubrique: &str, quantite: Chiffre, taux: Chiffre) -> LignePaie {
        LignePaie { rubrique: Some(rubrique.to_string()), quantite: Some(quantite), taux: Some(taux), ..LignePaie::default() }
    }

    /// Son code de rubrique, vide s'il n'est pas dit.
    pub fn code(&self) -> &str {
        match &self.rubrique {
            Some(r) => r,
            None => "",
        }
    }

    /// La case `case` de la ligne de frais est-elle cochee ?
    pub fn cochee(&self, case: &str) -> bool {
        let dite = match case {
            crate::JUSTIFICATIF => self.justificatif,
            crate::PARIS => self.paris,
            crate::RESTAURANT => self.restaurant,
            crate::LOGE => self.loge,
            _ => None,
        };
        dite == Some(true)
    }

    /// Son territoire hors de la metropole, s'il est dit.
    pub fn territoire_dit(&self) -> Option<&str> {
        self.territoire.as_deref().filter(|t| !t.is_empty())
    }

    /// Ses jours du mois, lus.
    pub fn jours(&self) -> Resultat<Vec<Jour>> {
        self.dates.iter().map(DateDeLigne::lu).collect()
    }

    /// Le premier jour de la ligne (ses dates, puis son planning), s'il se lit : un jour illisible, le calcul le refuse en
    /// le nommant (`jours`).
    pub fn premier_jour(&self) -> Option<Jour> {
        match self.dates.first().or(self.planning.first()).map(DateDeLigne::lu) {
            Some(Ok(j)) => Some(j),
            Some(Err(_)) | None => None,
        }
    }

    /// Ses heures dites, en flottant (`None` : rien n'est dit).
    pub fn heures_lues(&self) -> Resultat<Option<f64>> {
        self.heures.as_ref().map(Chiffre::flottant).transpose()
    }

    /// LA LIGNE SAISIE EN POURCENTAGE (`base` x `taux_majoration` %) rendue en `quantite` x `taux` ; une autre telle
    /// quelle.
    pub fn en_pourcentage(&self) -> Resultat<LignePaie> {
        if self.base.is_none() && self.taux_majoration.is_none() {
            return Ok(self.clone());
        }
        let rub = match &self.rubrique {
            Some(r) => r.clone(),
            None => "None".to_string(),
        };
        let (base, pct) = match (&self.base, &self.taux_majoration) {
            (Some(b), Some(p)) => (b.dec_avec_echelle()?, p.dec_avec_echelle()?),
            _ => {
                return Err(socle::echec::cle(
                    format!("La ligne « {} » est en pourcentage : elle dit sa base (en euros) et son pourcentage.", rub),
                    "taux_majoration",
                ))
            }
        };
        if base.is_sign_negative() && !base.is_zero() || pct.is_sign_negative() && !pct.is_zero() {
            return Err(format!("Ligne << {} >> : base et pourcentage sont positifs.", rub));
        }
        // Decimal(pct) / Decimal(100) : le quotient exact, a l'exposant ideal.
        let mut taux = socle::num::div(pct, Decimal::ONE_HUNDRED).normalize();
        if taux.scale() < pct.scale() {
            taux.rescale(pct.scale());
        }
        for (champ, x, attendu) in [("quantite", &self.quantite, base), ("taux", &self.taux, taux)] {
            if let Some(x) = x.as_ref().filter(|x| !x.vide()) {
                if x.dec_avec_echelle()? != attendu {
                    return Err(socle::echec::cle(
                        format!(
                            "La ligne « {} » : sa quantité ou son taux ({}) contredit sa base et son pourcentage ({} x {} %). N'écrire que la base et le pourcentage.",
                            rub,
                            x.en_texte(),
                            base,
                            pct
                        ),
                        champ,
                    ));
                }
            }
        }
        Ok(LignePaie { quantite: Some(Chiffre::Texte(base.to_string())), taux: Some(Chiffre::Texte(taux.to_string())), ..self.clone() })
    }

    /// SA FORME CANONIQUE, pour les empreintes et les oracles : ses nombres en texte normalise, sans ce que le moteur pose
    /// pour l'ecran (`Marques`).
    pub fn canon(&self) -> Resultat<LignePaie> {
        let c = |x: &Option<Chiffre>| x.as_ref().map(Chiffre::canon).transpose();
        Ok(LignePaie {
            quantite: c(&self.quantite)?,
            taux: c(&self.taux)?,
            base: c(&self.base)?,
            taux_majoration: c(&self.taux_majoration)?,
            heures: c(&self.heures)?,
            assiette_pas: c(&self.assiette_pas)?,
            marques: Marques::default(),
            ..self.clone()
        })
    }
}
