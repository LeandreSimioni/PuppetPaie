//! LE MOIS CIVIL `AAAA-MM` : la cle d'un mois de paie, d'un fichier d'engagements (`engagements/2026-11.json`), d'un
//! dossier de documents, d'une echeance. Il se lit une fois (`Mois::lire`, ou `serde` : la meme chaine dans le JSON) et
//! ne s'ecrit que sous cette forme ; son annee, son numero, son nom, le suivant et le precedent se lisent de lui.
//!
//! Quatre chiffres, un tiret, un numero de 01 a 12, rien d'autre : le mois d'une paie est celui que sa DSN declare, « le
//! premier jour du mois civil au titre duquel est etablie la paie », de forme `(01)(0[1-9]|1[0-2])(20)[0-9]{2}`
//! (S20.G00.05.005, `atelier/reference/norme-dsn/texte-2027/bloc-S20.G00.05.txt` l. 122-129). La norme n'admet « 00 »
//! qu'au mois d'une date de naissance inconnue (S21.G00.30.006, `bloc-S21.G00.30.txt` l. 143-148), ce que
//! `declarations::champs` lit du mois de naissance d'un NIR : jamais au mois d'une paie. Un mois nomme aussi un fichier ou
//! un dossier : `../config` ou `2026-06/envoi` n'en sont pas.

use crate::date::{jours_du_mois, repr_texte, Jour};
use crate::num::Resultat;

/// Un mois civil, de 0001-01 a 9999-12. L'ordre est celui du calendrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize, serde::Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct Mois {
    an: i32,
    numero: u32,
}

/// Les trois graphies d'un nom de mois que le moteur ecrit, chacune la ou elle s'ecrivait deja :
/// - `Titre` (« Aout ») : noms de fichiers (`DSN_Aout2026.dsn`), titres et sections des etats ;
/// - `Minuscule` (« aout ») : sans accent, dans une phrase (le libelle d'un mois d'engagement, d'un dossier) ;
/// - `Accentuee` (« août ») : dans une phrase en francais (courriels, contrats, avenants, ecrans).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Graphie {
    Titre,
    Minuscule,
    Accentuee,
}

const MOIS_TITRE: [&str; 12] = ["Janvier", "Fevrier", "Mars", "Avril", "Mai", "Juin", "Juillet", "Aout", "Septembre", "Octobre", "Novembre", "Decembre"];
const MOIS_MINUSCULE: [&str; 12] = ["janvier", "fevrier", "mars", "avril", "mai", "juin", "juillet", "aout", "septembre", "octobre", "novembre", "decembre"];
const MOIS_ACCENTUE: [&str; 12] = ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"];

/// Le refus d'un numero de mois hors de 1 a 12 : « 00 » n'est pas decembre, « 13 » n'est pas janvier.
pub fn mois_impossible(mois: impl std::fmt::Display) -> String {
    format!("mois impossible : {}", mois)
}

impl Mois {
    /// Le mois `numero` (1 a 12) de l'annee `an` (1 a 9999) ; refus sinon.
    pub fn nouveau(an: i32, numero: u32) -> Resultat<Mois> {
        if !(1..=9999).contains(&an) {
            return Err(crate::date::annee_impossible(an));
        }
        if !(1..=12).contains(&numero) {
            return Err(mois_impossible(numero));
        }
        Ok(Mois { an, numero })
    }

    /// Un mois ecrit `AAAA-MM` ; toute autre forme se refuse.
    pub fn lire(texte: &str) -> Resultat<Mois> {
        let b = texte.as_bytes();
        if !(b.len() == 7 && b[4] == b'-' && b[..4].iter().chain(&b[5..]).all(u8::is_ascii_digit)) {
            return Err(format!("mois illisible : {} (attendu : AAAA-MM)", repr_texte(texte)));
        }
        Mois::nouveau(texte[..4].parse().unwrap_or(0), texte[5..].parse().unwrap_or(0))
    }

    /// Le mois tel qu'une DSN le declare (S20.G00.05.005) : `01MMAAAA`, le premier jour du mois ; toute autre forme se
    /// refuse.
    pub fn de_la_dsn(texte: &str) -> Resultat<Mois> {
        let b = texte.as_bytes();
        if !(b.len() == 8 && b.starts_with(b"01") && b.iter().all(u8::is_ascii_digit)) {
            return Err(format!("mois de DSN illisible : {} (attendu : 01MMAAAA)", repr_texte(texte)));
        }
        Mois::nouveau(texte[4..].parse().unwrap_or(0), texte[2..4].parse().unwrap_or(0))
    }

    /// Le mois du jour `j`.
    pub fn du_jour(j: &Jour) -> Mois {
        Mois { an: j.an, numero: j.mois }
    }

    pub fn annee(self) -> i32 {
        self.an
    }

    /// Le numero du mois, de 1 a 12.
    pub fn numero(self) -> u32 {
        self.numero
    }

    /// Le nom du mois dans la graphie `g` : la seule table des noms de mois du moteur.
    pub fn nom(self, g: Graphie) -> &'static str {
        let table = match g {
            Graphie::Titre => &MOIS_TITRE,
            Graphie::Minuscule => &MOIS_MINUSCULE,
            Graphie::Accentuee => &MOIS_ACCENTUE,
        };
        table[(self.numero - 1) as usize]
    }

    /// « octobre 2026 », dans la graphie `g`.
    pub fn libelle(self, g: Graphie) -> String {
        format!("{} {:04}", self.nom(g), self.an)
    }

    /// « décembre 2026 », avec ses accents, du mois ecrit `texte`, pour une phrase qui ne peut pas refuser pour son mois (un
    /// bandeau, un conseil, un avertissement, un document) : un mois illisible s'ecrit tel quel (« 2026-00 »), jamais sous
    /// le nom d'un autre mois.
    pub fn en_clair(texte: &str) -> String {
        Mois::lire(texte).map_or_else(|_| texte.to_string(), |m| m.libelle(Graphie::Accentuee))
    }

    /// L'etiquette du mois dans le nom d'un fichier : « Aout2026 » (le nom en titre, sans accent).
    pub fn etiquette(self) -> String {
        format!("{}{:04}", self.nom(Graphie::Titre), self.an)
    }

    /// Le mois `n` mois plus loin (en arriere si `n` est negatif) ; refus hors des annees 1 a 9999.
    pub fn plus(self, n: i32) -> Resultat<Mois> {
        let rang = i64::from(self.an) * 12 + i64::from(self.numero) - 1 + i64::from(n);
        Mois::nouveau(i32::try_from(rang.div_euclid(12)).unwrap_or(0), (rang.rem_euclid(12) + 1) as u32).map_err(|_| crate::date::HORS_CALENDRIER.to_string())
    }

    /// Le mois suivant (decembre 9999 n'en a pas : il se rend lui-meme, aucun mois de paie n'y va).
    pub fn suivant(self) -> Mois {
        self.plus(1).unwrap_or(self)
    }

    /// Le mois precedent (janvier de l'an 1 n'en a pas : il se rend lui-meme).
    pub fn precedent(self) -> Mois {
        self.plus(-1).unwrap_or(self)
    }

    /// Le nombre de jours du mois.
    pub fn jours(self) -> u32 {
        jours_du_mois(self.an, self.numero)
    }

    /// Le premier jour du mois.
    pub fn premier_jour(self) -> Jour {
        Jour { an: self.an, mois: self.numero, jour: 1 }
    }

    /// Le dernier jour du mois.
    pub fn dernier_jour(self) -> Jour {
        Jour { an: self.an, mois: self.numero, jour: self.jours() }
    }

    /// Le jour `jour` du mois, s'il existe.
    pub fn jour(self, jour: u32) -> Option<Jour> {
        Jour::nouveau(self.an, self.numero, jour)
    }

    /// Le jour `j` est-il dans ce mois ?
    pub fn contient(self, j: &Jour) -> bool {
        Mois::du_jour(j) == self
    }

    /// Les mois de `debut` a `fin` inclus (`Engagement::mois_du_contrat`) ; aucun si `fin` precede `debut`.
    pub fn entre(debut: &Jour, fin: &Jour) -> Vec<Mois> {
        Mois::du_jour(debut).jusqu_a(Mois::du_jour(fin))
    }

    /// Les mois de celui-ci a `dernier` inclus ; aucun si `dernier` le precede.
    pub fn jusqu_a(self, dernier: Mois) -> Vec<Mois> {
        let mut m = self;
        let mut sortie = Vec::new();
        while m <= dernier {
            sortie.push(m);
            if m.suivant() == m {
                break;
            }
            m = m.suivant();
        }
        sortie
    }
}

impl std::fmt::Display for Mois {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04}-{:02}", self.an, self.numero)
    }
}

impl std::str::FromStr for Mois {
    type Err = String;
    fn from_str(s: &str) -> Resultat<Mois> {
        Mois::lire(s)
    }
}

impl TryFrom<String> for Mois {
    type Error = String;
    fn try_from(s: String) -> Resultat<Mois> {
        Mois::lire(&s)
    }
}

impl From<Mois> for String {
    fn from(m: Mois) -> String {
        m.to_string()
    }
}

impl From<Mois> for serde_json::Value {
    fn from(m: Mois) -> serde_json::Value {
        serde_json::Value::String(m.to_string())
    }
}

impl PartialEq<str> for Mois {
    fn eq(&self, autre: &str) -> bool {
        Mois::lire(autre).is_ok_and(|m| m == *self)
    }
}

impl PartialEq<&str> for Mois {
    fn eq(&self, autre: &&str) -> bool {
        *self == **autre
    }
}

impl PartialEq<String> for Mois {
    fn eq(&self, autre: &String) -> bool {
        *self == **autre
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un mois se lit `AAAA-MM`, de 01 a 12, et rien d'autre (S20.G00.05.005) ; le JSON garde la meme chaine.
    #[test]
    fn un_mois_se_lit_aaaa_mm_et_rien_d_autre() {
        let m = Mois::lire("2026-12").unwrap();
        assert_eq!((m.annee(), m.numero(), m.to_string()), (2026, 12, "2026-12".to_string()));
        assert_eq!(m.suivant().to_string(), "2027-01");
        assert_eq!(m.suivant().precedent(), m);
        for faux in ["2026-13", "2026-00", "2026-1", "../config", "2026-06/envoi", "2026-12-01", " 2026-12"] {
            assert!(Mois::lire(faux).is_err(), "{} n'est pas un mois", faux);
        }
        assert_eq!(serde_json::to_value(m).unwrap(), serde_json::json!("2026-12"));
        assert_eq!(serde_json::from_value::<Mois>(serde_json::json!("2026-02")).unwrap().dernier_jour().iso(), "2026-02-28");
        assert!(serde_json::from_value::<Mois>(serde_json::json!("2026-13")).is_err());
    }
}
